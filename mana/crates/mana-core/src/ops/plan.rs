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

/// Detect the project's language/stack by looking for marker files.
///
/// Returns a list of (language, config_file) pairs found in the project root.
pub fn detect_project_stack(project_root: &Path) -> Vec<(&'static str, &'static str)> {
    let markers: &[(&str, &str)] = &[
        ("Rust", "Cargo.toml"),
        ("JavaScript/TypeScript", "package.json"),
        ("Python", "pyproject.toml"),
        ("Python", "setup.py"),
        ("Go", "go.mod"),
        ("Ruby", "Gemfile"),
        ("Java", "pom.xml"),
        ("Java", "build.gradle"),
        ("Elixir", "mix.exs"),
        ("Swift", "Package.swift"),
        ("C/C++", "CMakeLists.txt"),
        ("Zig", "build.zig"),
    ];

    markers
        .iter()
        .filter(|(_, file)| project_root.join(file).exists())
        .copied()
        .collect()
}

/// Run static analysis commands for the detected stack.
///
/// Returns a string with the combined output of all checks (best-effort).
/// Commands that aren't installed or fail are skipped gracefully.
pub fn run_static_checks(project_root: &Path) -> String {
    let stack = detect_project_stack(project_root);
    let mut output = String::new();

    for (lang, _) in &stack {
        let checks: Vec<(&str, &[&str])> = match *lang {
            "Rust" => vec![
                ("cargo clippy", &["cargo", "clippy", "--", "-D", "warnings"]),
                ("cargo test (check)", &["cargo", "test", "--no-run"]),
            ],
            "JavaScript/TypeScript" => vec![
                ("npm run lint", &["npm", "run", "lint"]),
                ("npx tsc --noEmit", &["npx", "tsc", "--noEmit"]),
            ],
            "Python" => vec![
                ("ruff check .", &["ruff", "check", "."]),
                ("mypy .", &["mypy", "."]),
            ],
            "Go" => vec![
                ("go vet ./...", &["go", "vet", "./..."]),
                ("golangci-lint run", &["golangci-lint", "run"]),
            ],
            _ => vec![],
        };

        for (name, args) in checks {
            let result = std::process::Command::new(args[0])
                .args(&args[1..])
                .current_dir(project_root)
                .output();

            match result {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    if !o.status.success() {
                        output.push_str(&format!("### {} (exit {})\n", name, o.status));
                        if !stdout.is_empty() {
                            // Truncate to keep prompt reasonable
                            let truncated: String = stdout.chars().take(2000).collect();
                            output.push_str(&truncated);
                            output.push('\n');
                        }
                        if !stderr.is_empty() {
                            let truncated: String = stderr.chars().take(2000).collect();
                            output.push_str(&truncated);
                            output.push('\n');
                        }
                    } else {
                        output.push_str(&format!("### {} — ✓ passed\n", name));
                    }
                }
                Err(_) => {
                    // Tool not installed, skip silently
                }
            }
        }
    }

    output
}

/// Build a research prompt for project-level analysis.
///
/// Includes detected stack info, static check results, and instructions
/// for the agent to create units from findings.
pub fn build_research_prompt(
    project_root: &Path,
    parent_id: &str,
    mana_cmd: &str,
) -> String {
    let stack = detect_project_stack(project_root);
    let stack_info = if stack.is_empty() {
        "Could not detect project stack.".to_string()
    } else {
        stack
            .iter()
            .map(|(lang, file)| format!("- {} ({})", lang, file))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let static_output = run_static_checks(project_root);
    let static_section = if static_output.is_empty() {
        "No static analysis tools were available or all passed.".to_string()
    } else {
        static_output
    };

    format!(
        r#"Analyze this project for improvements and create units for each finding.

## Project Stack
{stack_info}

## Static Analysis Results
{static_section}

## Your Task
1. Review the static analysis output above for errors, warnings, and issues
2. Examine the codebase for:
   - **Bugs**: Logic errors, edge cases, error handling gaps
   - **Tests**: Missing test coverage, untested error paths
   - **Refactors**: Code duplication, complexity, unclear naming
   - **Security**: Input validation, auth issues, data exposure
   - **Performance**: Unnecessary allocations, N+1 queries, blocking I/O
3. For each finding, create a unit:

```
{mana_cmd} create "category: description" \
  --parent {parent_id} \
  --verify "test command" \
  --description "What's wrong, where it is, how to fix it"
```

## Categories
Use these prefixes for unit titles:
- `bug:` for bugs and logic errors
- `test:` for missing tests
- `refactor:` for code quality improvements
- `security:` for security issues
- `perf:` for performance improvements

## Rules
- Focus on actionable, concrete findings (not style nits)
- Every unit must have a verify command that proves the fix works
- Include file paths and line numbers when possible
- Prioritize: critical bugs > security > missing tests > refactors > perf
- Create 3-10 units (don't overwhelm with trivial issues)
- After creating units, run `{mana_cmd} tree {parent_id}` to show the result"#,
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
