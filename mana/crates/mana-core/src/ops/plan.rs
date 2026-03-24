use crate::unit::Unit;

/// Build a decomposition prompt for a unit.
///
/// Embeds the unit's description, produces/requires, and decomposition rules
/// so an agent can create well-scoped child units. Each child should be
/// completable by a fast, non-thinking model in a single pass.
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

## Goal
Each child unit must be **completable by a fast, non-thinking model in a single pass**.
This means: no design decisions, no ambiguity, no exploration. Every child should
read like a recipe — follow the steps, pass verify, done.

## What Makes a Unit One-Shottable
- **Specific instructions** — exact file paths, function signatures, concrete steps
- **Embedded context** — relevant types, patterns, and existing code in the description itself
- **Small scope** — touches 1-3 files, writes 1-5 functions
- **Clear acceptance** — the verify command tests exactly what matters
- **No decisions left** — the "what" and "how" are both fully specified

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
2. Examine referenced source files to understand the code
3. Decide on a split strategy
4. Create 2-5 child units using `mana create` commands
5. Ensure every child has a verify command and enough embedded context
   that a fast model can implement it without exploring the codebase
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

    #[test]
    fn build_prompt_includes_rules() {
        let unit = Unit::new("42", "Implement auth system");
        let prompt = build_decomposition_prompt("42", &unit, None);

        assert!(prompt.contains("Decompose unit 42"));
        assert!(prompt.contains("Implement auth system"));
        assert!(prompt.contains("non-thinking model"));
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
    fn build_prompt_custom_strategy_passed_through() {
        let unit = Unit::new("1", "Task");
        let prompt = build_decomposition_prompt("1", &unit, Some("my custom approach"));
        assert!(prompt.contains("my custom approach"));
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
