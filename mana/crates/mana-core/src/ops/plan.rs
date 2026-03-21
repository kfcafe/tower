use std::path::Path;

use anyhow::Result;

use crate::blocking::{MAX_PATHS, MAX_PRODUCES};
use crate::discovery::find_unit_file;
use crate::index::Index;
use crate::unit::{Status, Unit};
use crate::util::natural_cmp;

/// A candidate unit that needs planning (decomposition).
pub struct PlanCandidate {
    pub id: String,
    pub title: String,
    pub priority: u8,
}

/// Whether a unit is considered oversized and needs planning.
pub fn is_oversized(unit: &Unit) -> bool {
    unit.produces.len() > MAX_PRODUCES || unit.paths.len() > MAX_PATHS
}

/// Find all open, unclaimed units that are oversized.
///
/// Returns candidates sorted by priority (ascending P0 first), then by ID.
pub fn find_plan_candidates(mana_dir: &Path) -> Result<Vec<PlanCandidate>> {
    let index = Index::load_or_rebuild(mana_dir)?;
    let mut candidates: Vec<PlanCandidate> = Vec::new();

    for entry in &index.units {
        if entry.status != Status::Open {
            continue;
        }
        if entry.claimed_by.is_some() {
            continue;
        }

        let bean_path = match find_unit_file(mana_dir, &entry.id) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let unit = match Unit::from_file(&bean_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if is_oversized(&unit) {
            candidates.push(PlanCandidate {
                id: entry.id.clone(),
                title: entry.title.clone(),
                priority: entry.priority,
            });
        }
    }

    candidates.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| natural_cmp(&a.id, &b.id))
    });

    Ok(candidates)
}

/// Build a rich decomposition prompt for a unit.
///
/// Embeds the unit's description, produces/requires, and decomposition rules
/// so an agent can create well-scoped child units.
pub fn build_decomposition_prompt(id: &str, unit: &Unit, strategy: Option<&str>) -> String {
    let strategy_guidance = match strategy {
        Some("feature") | Some("by-feature") => {
            "Split by feature — each child is a vertical slice (types + impl + tests for one feature)."
        }
        Some("layer") | Some("by-layer") => {
            "Split by layer — types/interfaces first, then implementation, then tests."
        }
        Some("file") | Some("by-file") => {
            "Split by file — each child handles one file or closely related file group."
        }
        Some("phase") => {
            "Split by phase — scaffold first, then core logic, then edge cases, then polish."
        }
        Some(other) => {
            return build_prompt_text(id, unit, other);
        }
        None => "Choose the best strategy: by-feature (vertical slices), by-layer, or by-file.",
    };

    build_prompt_text(id, unit, strategy_guidance)
}

fn build_prompt_text(id: &str, unit: &Unit, strategy_guidance: &str) -> String {
    let title = &unit.title;
    let priority = unit.priority;
    let description = unit.description.as_deref().unwrap_or("(no description)");

    let mut dep_context = String::new();
    if !unit.produces.is_empty() {
        dep_context.push_str(&format!("\nProduces: {}\n", unit.produces.join(", ")));
    }
    if !unit.requires.is_empty() {
        dep_context.push_str(&format!("Requires: {}\n", unit.requires.join(", ")));
    }

    format!(
        r#"Decompose unit {id} into smaller child units.

## Parent Unit
- **ID:** {id}
- **Title:** {title}
- **Priority:** P{priority}
{dep_context}
## Strategy
{strategy_guidance}

## Sizing Rules
- A unit is **atomic** if it requires ≤5 functions to write and ≤10 to read
- Each child should have at most 3 `produces` artifacts and 5 `paths`
- Count functions concretely by examining the code — don't estimate

## Splitting Rules
- Create **2-4 children** for medium units, **3-5** for large ones
- **Maximize parallelism** — prefer independent units over sequential chains
- Each child must have a **verify command** that exits 0 on success
- Children should be independently testable where possible
- Use `--produces` and `--requires` to express dependencies between siblings

## Context Embedding Rules
- **Embed context into descriptions** — don't reference files, include the relevant types/signatures
- Include: concrete file paths, function signatures, type definitions
- Include: specific steps, edge cases, error handling requirements
- Be specific: "Add `fn validate_email(s: &str) -> bool` to `src/util.rs`" not "add validation"

## How to Create Children
Use `mana create` for each child unit:

```
mana create "child title" \
  --parent {id} \
  --priority {priority} \
  --verify "test command that exits 0" \
  --produces "artifact_name" \
  --requires "artifact_from_sibling" \
  --description "Full description with:
- What to implement
- Which files to modify (with paths)
- Key types/signatures to use or create
- Acceptance criteria
- Edge cases to handle"
```

## Description Template
A good child unit description includes:
1. **What**: One clear sentence of what this child does
2. **Files**: Specific file paths with what changes in each
3. **Context**: Embedded type definitions, function signatures, patterns to follow
4. **Acceptance**: Concrete criteria the verify command checks
5. **Edge cases**: What could go wrong, what to handle

## Your Task
1. Read the parent unit's description below
2. Examine referenced source files to count functions accurately
3. Decide on a split strategy
4. Create 2-5 child units using `mana create` commands
5. Ensure every child has a verify command
6. After creating children, run `mana tree {id}` to show the result

## Parent Unit Description
{description}"#,
    )
}

/// Escape a string for safe use as a single shell argument.
pub fn shell_escape(s: &str) -> String {
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_beans_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        fs::write(mana_dir.join("config.yaml"), "project: test\nnext_id: 10\n").unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn is_oversized_with_many_produces() {
        let mut unit = Unit::new("1", "Big unit");
        unit.produces = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        assert!(is_oversized(&unit));
    }

    #[test]
    fn is_oversized_false_for_small() {
        let unit = Unit::new("1", "Small unit");
        assert!(!is_oversized(&unit));
    }

    #[test]
    fn find_candidates_returns_oversized() {
        let (_dir, mana_dir) = setup_beans_dir();

        let mut big = Unit::new("1", "Big unit");
        big.produces = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        big.to_file(mana_dir.join("1-big-unit.md")).unwrap();

        let small = Unit::new("2", "Small unit");
        small.to_file(mana_dir.join("2-small-unit.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let candidates = find_plan_candidates(&mana_dir).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "1");
    }

    #[test]
    fn find_candidates_empty_when_all_small() {
        let (_dir, mana_dir) = setup_beans_dir();

        let unit = Unit::new("1", "Small");
        unit.to_file(mana_dir.join("1-small.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let candidates = find_plan_candidates(&mana_dir).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn build_prompt_includes_rules() {
        let unit = Unit::new("42", "Implement auth system");
        let prompt = build_decomposition_prompt("42", &unit, None);

        assert!(prompt.contains("Decompose unit 42"));
        assert!(prompt.contains("Implement auth system"));
        assert!(prompt.contains("≤5 functions"));
        assert!(prompt.contains("Maximize parallelism"));
        assert!(prompt.contains("Embed context"));
        assert!(prompt.contains("verify command"));
        assert!(prompt.contains("mana create"));
        assert!(prompt.contains("--parent 42"));
        assert!(prompt.contains("--produces"));
        assert!(prompt.contains("--requires"));
    }

    #[test]
    fn build_prompt_with_strategy() {
        let unit = Unit::new("1", "Big task");
        let prompt = build_decomposition_prompt("1", &unit, Some("by-feature"));
        assert!(prompt.contains("vertical slice"));
    }

    #[test]
    fn build_prompt_includes_produces_requires() {
        let mut unit = Unit::new("5", "Task with deps");
        unit.produces = vec!["auth_types".to_string(), "auth_middleware".to_string()];
        unit.requires = vec!["db_connection".to_string()];

        let prompt = build_decomposition_prompt("5", &unit, None);
        assert!(prompt.contains("auth_types"));
        assert!(prompt.contains("db_connection"));
    }

    #[test]
    fn shell_escape_simple() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn shell_escape_with_quotes() {
        assert_eq!(shell_escape("it's here"), "'it'\\''s here'");
    }
}
