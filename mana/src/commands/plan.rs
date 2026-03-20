//! `mana plan` — interactively plan a large unit into children.
//!
//! Without an ID, picks the highest-priority ready unit that is oversized or unscoped.
//! When `config.plan` is set, spawns that template command.
//! Otherwise, builds a rich decomposition prompt and spawns `pi` directly.

use std::path::Path;

use anyhow::Result;

use crate::unit::Unit;
use crate::config::Config;
use crate::discovery::find_unit_file;
use crate::index::Index;
use mana_core::ops::plan::{
    build_decomposition_prompt, find_plan_candidates, is_oversized, shell_escape,
};

/// Arguments for the plan command.
pub struct PlanArgs {
    pub id: Option<String>,
    pub strategy: Option<String>,
    pub auto: bool,
    pub force: bool,
    pub dry_run: bool,
}

/// Execute the `mana plan` command.
pub fn cmd_plan(mana_dir: &Path, args: PlanArgs) -> Result<()> {
    let config = Config::load_with_extends(mana_dir)?;

    let _index = Index::load_or_rebuild(mana_dir)?;

    match args.id {
        Some(ref id) => plan_specific(mana_dir, &config, id, &args),
        None => plan_auto_pick(mana_dir, &config, &args),
    }
}

/// Plan a specific unit by ID.
fn plan_specific(mana_dir: &Path, config: &Config, id: &str, args: &PlanArgs) -> Result<()> {
    let bean_path = find_unit_file(mana_dir, id)?;
    let unit = Unit::from_file(&bean_path)?;

    if !is_oversized(&unit) && !args.force {
        eprintln!("Unit {} is small enough to run directly.", id);
        eprintln!("  Use mana run {} to dispatch it.", id);
        eprintln!("  Use mana plan {} --force to plan anyway.", id);
        return Ok(());
    }

    spawn_plan(mana_dir, config, id, &unit, args)
}

/// Auto-pick the highest-priority ready unit that is oversized.
fn plan_auto_pick(mana_dir: &Path, config: &Config, args: &PlanArgs) -> Result<()> {
    let candidates = find_plan_candidates(mana_dir)?;

    if candidates.is_empty() {
        eprintln!("✓ All ready units are small enough to run directly.");
        eprintln!("  Use mana run to dispatch them.");
        return Ok(());
    }

    // Show all candidates
    eprintln!("{} units need planning:", candidates.len());
    for c in &candidates {
        eprintln!("  P{}  {:6}  {}", c.priority, c.id, c.title);
    }
    eprintln!();

    // Pick first (highest priority, lowest ID)
    let first = &candidates[0];
    eprintln!("Planning: {} — {}", first.id, first.title);

    let bean_path = find_unit_file(mana_dir, &first.id)?;
    let unit = Unit::from_file(&bean_path)?;

    spawn_plan(mana_dir, config, &first.id, &unit, args)
}

/// Spawn the plan command for a unit.
fn spawn_plan(
    mana_dir: &Path,
    config: &Config,
    id: &str,
    unit: &Unit,
    args: &PlanArgs,
) -> Result<()> {
    if let Some(ref template) = config.plan {
        return spawn_template(template, id, args);
    }

    spawn_builtin(mana_dir, id, unit, args)
}

/// Spawn the plan using a user-configured template command.
fn spawn_template(template: &str, id: &str, args: &PlanArgs) -> Result<()> {
    let mut cmd = template.replace("{id}", id);

    if let Some(ref strategy) = args.strategy {
        cmd = format!("{} --strategy {}", cmd, strategy);
    }

    if args.dry_run {
        eprintln!("Would spawn: {}", cmd);
        return Ok(());
    }

    eprintln!("Spawning: {}", cmd);
    run_shell_command(&cmd, id, args.auto)
}

/// Build a decomposition prompt and spawn `pi` with it directly.
fn spawn_builtin(mana_dir: &Path, id: &str, unit: &Unit, args: &PlanArgs) -> Result<()> {
    let prompt = build_decomposition_prompt(id, unit, args.strategy.as_deref());

    let bean_path = find_unit_file(mana_dir, id)?;
    let bean_path_str = bean_path.display().to_string();

    let escaped_prompt = shell_escape(&prompt);
    let cmd = format!("pi @{} {}", bean_path_str, escaped_prompt);

    if args.dry_run {
        eprintln!("Would spawn: {}", cmd);
        eprintln!("\n--- Built-in decomposition prompt ---");
        eprintln!("{}", prompt);
        return Ok(());
    }

    eprintln!("Spawning built-in decomposition for unit {}...", id);
    run_shell_command(&cmd, id, args.auto)
}

/// Execute a shell command, either interactively or non-interactively.
fn run_shell_command(cmd: &str, id: &str, auto: bool) -> Result<()> {
    if auto {
        let status = std::process::Command::new("sh").args(["-c", cmd]).status();
        match status {
            Ok(s) if s.success() => {
                eprintln!("Planning complete. Use mana tree {} to see children.", id);
            }
            Ok(s) => {
                anyhow::bail!("Plan command exited with code {}", s.code().unwrap_or(-1));
            }
            Err(e) => {
                anyhow::bail!("Failed to run plan command: {}", e);
            }
        }
    } else {
        let status = std::process::Command::new("sh")
            .args(["-c", cmd])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
        match status {
            Ok(s) if s.success() => {
                eprintln!("Planning complete. Use mana tree {} to see children.", id);
            }
            Ok(s) => {
                let code = s.code().unwrap_or(-1);
                if code != 0 {
                    anyhow::bail!("Plan command exited with code {}", code);
                }
            }
            Err(e) => {
                anyhow::bail!("Failed to run plan command: {}", e);
            }
        }
    }
    Ok(())
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
        fs::write(
            mana_dir.join("config.yaml"),
            "project: test\nnext_id: 10\n",
        )
        .unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn plan_help_contains_plan() {
        // Verified by the unit's verify command
    }

    #[test]
    fn plan_no_template_without_auto_errors() {
        let (dir, mana_dir) = setup_beans_dir();

        let mut unit = Unit::new("1", "Big unit");
        unit.produces = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        unit.to_file(mana_dir.join("1-big-unit.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: Some("1".to_string()),
                strategy: None,
                auto: false,
                force: true,
                dry_run: true,
            },
        );

        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn plan_small_bean_suggests_run() {
        let (dir, mana_dir) = setup_beans_dir();

        let unit = Unit::new("1", "Small unit");
        unit.to_file(mana_dir.join("1-small-unit.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: Some("1".to_string()),
                strategy: None,
                auto: false,
                force: false,
                dry_run: false,
            },
        );

        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn plan_force_overrides_size_check() {
        let (dir, mana_dir) = setup_beans_dir();

        fs::write(
            mana_dir.join("config.yaml"),
            "project: test\nnext_id: 10\nplan: \"true\"\n",
        )
        .unwrap();

        let unit = Unit::new("1", "Small unit");
        unit.to_file(mana_dir.join("1-small-unit.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: Some("1".to_string()),
                strategy: None,
                auto: false,
                force: true,
                dry_run: false,
            },
        );

        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn plan_dry_run_does_not_spawn() {
        let (dir, mana_dir) = setup_beans_dir();

        fs::write(
            mana_dir.join("config.yaml"),
            "project: test\nnext_id: 10\nplan: \"echo planning {id}\"\n",
        )
        .unwrap();

        let mut unit = Unit::new("1", "Big unit");
        unit.produces = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        unit.to_file(mana_dir.join("1-big-unit.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: Some("1".to_string()),
                strategy: None,
                auto: false,
                force: false,
                dry_run: true,
            },
        );

        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn plan_auto_pick_finds_oversized() {
        let (dir, mana_dir) = setup_beans_dir();

        fs::write(
            mana_dir.join("config.yaml"),
            "project: test\nnext_id: 10\nplan: \"true\"\n",
        )
        .unwrap();

        let mut big = Unit::new("1", "Big unit");
        big.produces = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        big.to_file(mana_dir.join("1-big-unit.md")).unwrap();

        let small = Unit::new("2", "Small unit");
        small.to_file(mana_dir.join("2-small-unit.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: None,
                strategy: None,
                auto: true,
                force: false,
                dry_run: false,
            },
        );

        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn plan_auto_pick_none_needed() {
        let (dir, mana_dir) = setup_beans_dir();

        let unit = Unit::new("1", "Small");
        unit.to_file(mana_dir.join("1-small.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: None,
                strategy: None,
                auto: false,
                force: false,
                dry_run: false,
            },
        );

        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn build_prompt_includes_decomposition_rules() {
        let unit = Unit::new("42", "Implement auth system");
        let prompt = build_decomposition_prompt("42", &unit, None);

        assert!(prompt.contains("Decompose unit 42"), "missing header");
        assert!(prompt.contains("Implement auth system"), "missing title");
        assert!(prompt.contains("≤5 functions"), "missing sizing rules");
        assert!(
            prompt.contains("Maximize parallelism"),
            "missing parallelism rule"
        );
        assert!(
            prompt.contains("Embed context"),
            "missing context embedding rule"
        );
        assert!(
            prompt.contains("verify command"),
            "missing verify requirement"
        );
        assert!(prompt.contains("mana create"), "missing create syntax");
        assert!(prompt.contains("--parent 42"), "missing parent flag");
        assert!(prompt.contains("--produces"), "missing produces flag");
        assert!(prompt.contains("--requires"), "missing requires flag");
    }

    #[test]
    fn build_prompt_with_strategy() {
        let unit = Unit::new("1", "Big task");
        let prompt = build_decomposition_prompt("1", &unit, Some("by-feature"));

        assert!(
            prompt.contains("vertical slice"),
            "missing feature strategy guidance"
        );
    }

    #[test]
    fn build_prompt_includes_produces_requires() {
        let mut unit = Unit::new("5", "Task with deps");
        unit.produces = vec!["auth_types".to_string(), "auth_middleware".to_string()];
        unit.requires = vec!["db_connection".to_string()];

        let prompt = build_decomposition_prompt("5", &unit, None);

        assert!(prompt.contains("auth_types"), "missing produces");
        assert!(prompt.contains("db_connection"), "missing requires");
    }

    #[test]
    fn shell_escape_simple() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn shell_escape_with_quotes() {
        assert_eq!(shell_escape("it's here"), "'it'\\''s here'");
    }

    #[test]
    fn plan_builtin_dry_run_shows_prompt() {
        let (dir, mana_dir) = setup_beans_dir();

        let mut unit = Unit::new("1", "Big unit");
        unit.description = Some("x".repeat(2000));
        unit.to_file(mana_dir.join("1-big-unit.md")).unwrap();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: Some("1".to_string()),
                strategy: None,
                auto: false,
                force: true,
                dry_run: true,
            },
        );

        assert!(result.is_ok());

        drop(dir);
    }
}
