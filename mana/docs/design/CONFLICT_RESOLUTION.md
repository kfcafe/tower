# Unit Merge Conflict Resolution System

> **Status: Design Document (Not Implemented)**
> 
> This document describes a proposed conflict resolution system for units.
> The `mana resolve` command and merge logic described here do not yet exist.

## Overview

When multiple agents/users modify the same unit concurrently, conflicts arise. This system detects conflicts and resolves them intelligently while preserving all work.

## Problem Space

### Conflict Types

#### 1. **Non-Overlapping Field Conflict** (Auto-Resolvable)
- Agent A modifies `status: open → in_progress`
- Agent B modifies `description: "old" → "new"`
- Result: Both changes apply automatically

#### 2. **Overlapping Field Conflict** (Requires Resolution)
- Agent A modifies `status: open → in_progress`
- Agent B modifies `status: open → closed`
- Result: Cannot auto-merge; conflict detected

#### 3. **Append Conflict** (Auto-Resolvable)
- Agent A appends note: "Done part 1"
- Agent B appends note: "Done part 2"
- Result: Both notes combined

#### 4. **Collection Conflict** (Mostly Auto-Resolvable)
- Agent A adds label: `labels: [urgent]`
- Agent B removes label: same
- Result: Conflict on intent

## Solution: Field-Level Merge with Conflict Markers

### Core Approach

```
Read-Modify-Write Conflict Detection:
1. Load original unit (get version hash)
2. Modify unit locally
3. On write:
   - Check if file was modified since read (compare hash)
   - If modified: attempt 3-way merge
   - If merge succeeds: write merged result
   - If merge fails: preserve conflict, fail write
```

### Implementation Strategy

#### Phase 1: Detection & Conflict Markers

Add a conflict metadata structure to units:

```yaml
---
id: 5
title: Example Unit
status: open
# ... other fields ...
conflicts:
  - field: status
    versions:
      - value: in_progress
        agent: agent-a
        timestamp: 2026-01-31T10:30:00Z
      - value: closed
        agent: agent-b
        timestamp: 2026-01-31T10:30:01Z
    resolution: pending
---
```

#### Phase 2: Three-Way Merge

For each field:
1. **Base version** (original before any changes)
2. **Left version** (first agent's modification)
3. **Right version** (second agent's modification)

Merge rules:
- If `base == left`: take right (only right changed)
- If `base == right`: take left (only left changed)
- If `left == right`: take left (both same)
- If `base != left && base != right && left != right`: **CONFLICT**

#### Phase 3: Special Field Handling

**Append-only fields** (notes, design notes):
- Always append both versions with metadata
- No conflict possible

**Collection fields** (labels, dependencies):
- Set-based merge: combine additions, conflict on deletions
- Example: `labels: [urgent]` + remove `urgent` = CONFLICT

**Status field**:
- State machine validation: only valid transitions allowed
- If both states reachable from base: CONFLICT
- If one is valid transition, other isn't: take valid one

**Scalar fields** (priority, title, description):
- Direct 3-way merge rules above

## Implementation Plan

### Step 1: Add Conflict Tracking to Unit Model

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FieldConflict {
    pub field: String,
    pub versions: Vec<ConflictVersion>,
    pub resolution: ConflictResolution,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConflictVersion {
    pub value: String,  // JSON-serialized value
    pub agent: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ConflictResolution {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "resolved")]
    Resolved,
    #[serde(rename = "discarded")]
    Discarded,
}

pub struct Unit {
    // ... existing fields ...
    pub conflicts: Vec<FieldConflict>,
}
```

### Step 2: Add Version Hash Tracking

```rust
impl Unit {
    /// Calculate SHA256 hash of canonical form
    pub fn hash(&self) -> String {
        // Sort fields, serialize to JSON, SHA256 hash
    }

    /// Load unit with version hash
    pub fn from_file_with_hash(path: &Path) -> Result<(Self, String)> {
        let unit = Unit::from_file(path)?;
        let hash = unit.hash();
        Ok((unit, hash))
    }
}
```

### Step 3: Merge Logic

```rust
impl Unit {
    /// Three-way merge: base, left (ours), right (theirs)
    pub fn merge(&mut self, base: &Unit, right: &Unit) -> Result<Vec<String>> {
        let mut conflicts = Vec::new();

        // Merge each field
        self.merge_field("status", base.status, self.status, right.status, &mut conflicts)?;
        self.merge_field("priority", base.priority, self.priority, right.priority, &mut conflicts)?;
        // ... etc for each field

        Ok(conflicts)  // Empty if no conflicts
    }

    fn merge_field<T: PartialEq + Clone>(
        &mut self,
        field_name: &str,
        base: T,
        mut left: T,
        right: T,
        conflicts: &mut Vec<String>,
    ) -> Result<()> {
        match (base == left, base == right, left == right) {
            (true, true, _) => {},      // No change
            (true, false, _) => { left = right; },  // Only right changed
            (false, true, _) => {},     // Only left changed (keep left)
            (false, false, true) => {}, // Both same
            (false, false, false) => {  // Both changed differently - CONFLICT
                conflicts.push(format!("Field '{}' has conflicting changes", field_name));
            }
        }
        Ok(())
    }
}
```

### Step 4: Update Command with Conflict Detection

```rust
pub fn cmd_update_with_merge(
    beans_dir: &Path,
    id: &str,
    updates: UpdateRequest,
) -> Result<()> {
    // Load original with hash
    let bean_path = find_bean_file(beans_dir, id)?;
    let (mut unit, original_hash) = Unit::from_file_with_hash(&bean_path)?;
    let base = unit.clone();  // Save base version

    // Apply local modifications
    apply_updates(&mut unit, updates)?;

    // Before writing: check for concurrent modifications
    let (current, current_hash) = Unit::from_file_with_hash(&bean_path)?;

    if current_hash != original_hash {
        // File was modified! Attempt merge
        match unit.merge(&base, &current) {
            Ok(conflicts) if conflicts.is_empty() => {
                // Merge succeeded, write result
                unit.to_file(&bean_path)?;
            }
            Ok(conflicts) => {
                // Record conflicts in unit
                unit.conflicts.push(/* create conflict records */);
                unit.to_file(&bean_path)?;
                return Err(anyhow!("Conflicts: {}", conflicts.join(", ")));
            }
            Err(e) => {
                return Err(anyhow!("Merge failed: {}", e));
            }
        }
    } else {
        // No concurrent modification, write normally
        unit.to_file(&bean_path)?;
    }

    // Rebuild index
    let index = Index::build(beans_dir)?;
    index.save(beans_dir)?;

    Ok(())
}
```

### Step 5: Conflict Resolution Command

Add new command: `mana resolve <unit-id> <field> <choice>`

```rust
pub fn cmd_resolve(
    beans_dir: &Path,
    id: &str,
    field: &str,
    choice: usize,  // Which version to keep (0, 1, 2... or "ask")
) -> Result<()> {
    let bean_path = find_bean_file(beans_dir, id)?;
    let mut unit = Unit::from_file(&bean_path)?;

    // Find conflict
    let conflict = unit.conflicts.iter_mut()
        .find(|c| c.field == field)
        .ok_or_else(|| anyhow!("No conflict for field: {}", field))?;

    if choice >= conflict.versions.len() {
        return Err(anyhow!("Invalid choice: {}", choice));
    }

    let chosen_value = conflict.versions[choice].value.clone();

    // Apply chosen value to field
    unit.apply_value(field, &chosen_value)?;

    // Mark conflict as resolved
    conflict.resolution = ConflictResolution::Resolved;

    // Clear conflicts if all resolved
    unit.conflicts.retain(|c| c.resolution == ConflictResolution::Pending);

    // Save
    unit.to_file(&bean_path)?;

    // Rebuild index
    let index = Index::build(beans_dir)?;
    index.save(beans_dir)?;

    println!("Resolved conflict for unit {} field {}", id, field);
    Ok(())
}
```

## Conflict Detection Rules by Field Type

### Scalar Fields (title, description, priority, assignee)
- 3-way merge with conflict on divergent changes

### Status Field
- Validate state machine
- If both transitions valid from base: conflict
- If one invalid: take the valid one
- If both invalid: conflict

### Append Fields (notes)
- Always merge: append both with timestamps
- No conflicts possible

### Collection Fields (labels, dependencies)
- Additions: always merge (set union)
- Removals: conflict if both versions affected same element
- Example: A adds "urgent", B removes "urgent" = conflict

### Complex Fields (description, design)
- Line-based diff/merge (similar to git)
- Heuristic: if changes don't overlap by 3+ lines: auto-merge
- If overlapping: conflict

## User Workflows

### Scenario 1: Auto-Merge Success
```bash
$ mana update 5 --status in_progress  # Agent A
# ... Agent B updates description concurrently ...
$ mana show 5
Status: in_progress
Description: (Agent B's changes)
✓ Merged automatically
```

### Scenario 2: Conflict Detected
```bash
$ mana update 5 --status closed  # Agent A
# ... Agent B updates status to in_progress concurrently ...
$ mana update 5 --description "my change"  # Agent A gets conflict

! Conflict detected for unit 5
! Field 'status' has competing values:
  [0] in_progress (agent-b @ 2026-01-31 10:30:01)
  [1] closed (you @ 2026-01-31 10:30:02)

$ mana resolve 5 status 0  # Pick version 0
✓ Resolved: status = in_progress
```

### Scenario 3: View Conflicts
```bash
$ mana show 5 --conflicts
# Shows all conflicted fields with versions
```

## Testing Strategy

```rust
#[cfg(test)]
mod merge_tests {
    #[test]
    fn test_non_overlapping_merge() { }

    #[test]
    fn test_conflicting_scalar_merge() { }

    #[test]
    fn test_status_valid_transition_merge() { }

    #[test]
    fn test_notes_append_merge() { }

    #[test]
    fn test_labels_union_merge() { }

    #[test]
    fn test_concurrent_write_detection() { }

    #[test]
    fn test_resolve_command() { }
}
```

## Migration from Current System

1. Add `conflicts: []` field to all existing units
2. Existing units have no conflicts initially
3. New writes use merge logic
4. Graceful degradation: old units work as before

## Future Enhancements

1. **Automatic conflict resolution policies**
   - "last-write-wins"
   - "agent-priority" (some agents always take precedence)
   - "data-loss-minimizing" (preserve more data)

2. **Operational Transform (OT) for real-time collaboration**
   - Current approach is good for batch updates
   - OT would support live editing

3. **Conflict statistics and reporting**
   - Track conflict patterns
   - Alert on high-conflict units

4. **Audit trail**
   - Full history of all versions
   - Revert to any previous version

## References

- Git's three-way merge algorithm
- Operational Transformation (OT)
- CRDT (Conflict-free Replicated Data Types)
