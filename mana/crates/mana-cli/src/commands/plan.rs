//! `mana plan <id>` — decompose a unit into smaller children.
//!
//! Takes a unit ID and spawns an agent to break it into child units.
//! When `config.plan` is set, spawns that template command.
//! Otherwise, builds a rich prompt and spawns `pi` directly.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::discovery::find_unit_file;
use crate::spawner::substitute_template_with_model;
use crate::unit::Unit;
use mana_core::ops::plan::{build_decomposition_prompt, shell_escape};

/// Arguments for the plan command.
pub struct PlanArgs {
    pub id: String,
    pub strategy: Option<String>,
    pub auto: bool,
    pub dry_run: bool,
}

/// Execute the `mana plan` command.
pub fn cmd_plan(mana_dir: &Path, args: PlanArgs) -> Result<()> {
    let config = Config::load_with_extends(mana_dir)?;
    let unit_path = find_unit_file(mana_dir, &args.id)?;
    let unit = Unit::from_file(&unit_path)?;

    spawn_plan(mana_dir, &config, &args.id, &unit, &args)
}

fn spawn_plan(
    mana_dir: &Path,
    config: &Config,
    id: &str,
    unit: &Unit,
    args: &PlanArgs,
) -> Result<()> {
    let effective_model = unit.model.as_deref().or(config.plan_model.as_deref());

    if let Some(ref template) = config.plan {
        return spawn_template(template, id, args, effective_model);
    }

    spawn_builtin(mana_dir, id, unit, args, effective_model)
}

#[must_use]
fn build_plan_template_command(
    template: &str,
    id: &str,
    strategy: Option<&str>,
    model: Option<&str>,
) -> String {
    let mut cmd = substitute_template_with_model(template, id, model);

    if let Some(strategy) = strategy {
        cmd = format!("{} --strategy {}", cmd, strategy);
    }

    cmd
}

fn spawn_template(template: &str, id: &str, args: &PlanArgs, model: Option<&str>) -> Result<()> {
    let cmd = build_plan_template_command(template, id, args.strategy.as_deref(), model);

    if args.dry_run {
        eprintln!("Would spawn: {}", cmd);
        return Ok(());
    }

    eprintln!("Spawning: {}", cmd);
    run_shell_command(&cmd, id, args.auto)
}

#[must_use]
fn build_builtin_plan_command(unit_path: &str, prompt: &str, model: Option<&str>) -> String {
    let escaped_prompt = shell_escape(prompt);
    match model {
        Some(model) => format!(
            "pi --model {} @{} {}",
            shell_escape(model),
            unit_path,
            escaped_prompt
        ),
        None => format!("pi @{} {}", unit_path, escaped_prompt),
    }
}

fn spawn_builtin(
    mana_dir: &Path,
    id: &str,
    unit: &Unit,
    args: &PlanArgs,
    model: Option<&str>,
) -> Result<()> {
    let prompt = build_decomposition_prompt(id, unit, args.strategy.as_deref());
    let unit_path = find_unit_file(mana_dir, id)?;
    let unit_path_str = unit_path.display().to_string();
    let cmd = build_builtin_plan_command(&unit_path_str, &prompt, model);

    if args.dry_run {
        eprintln!("Would spawn: {}", cmd);
        eprintln!("\n--- Decomposition prompt ---");
        eprintln!("{}", prompt);
        return Ok(());
    }

    eprintln!("Spawning decomposition for unit {}...", id);
    run_shell_command(&cmd, id, args.auto)
}

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
    use mana_core::ops::plan::build_decomposition_prompt;
    use std::fs;
    use tempfile::TempDir;

    fn setup_mana_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        fs::write(mana_dir.join("config.yaml"), "project: test\nnext_id: 10\n").unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn plan_dry_run_does_not_spawn() {
        let (dir, mana_dir) = setup_mana_dir();
        fs::write(
            mana_dir.join("config.yaml"),
            "project: test\nnext_id: 10\nplan: \"echo planning {id}\"\n",
        )
        .unwrap();
        let unit = Unit::new("1", "Some unit");
        unit.to_file(mana_dir.join("1-some-unit.md")).unwrap();
        let _ = mana_core::index::Index::build(&mana_dir);
        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: "1".to_string(),
                strategy: None,
                auto: false,
                dry_run: true,
            },
        );
        assert!(result.is_ok());
        drop(dir);
    }

    #[test]
    fn plan_template_substitutes_model_and_strategy() {
        let cmd = build_plan_template_command(
            "claude --model {model} -p 'plan {id}'",
            "7",
            Some("by-layer"),
            Some("haiku"),
        );
        assert_eq!(cmd, "claude --model haiku -p 'plan 7' --strategy by-layer");
    }

    #[test]
    fn builtin_plan_command_includes_model_when_set() {
        let cmd = build_builtin_plan_command(
            ".mana/7-plan.md",
            "plan this unit carefully",
            Some("sonnet"),
        );
        assert_eq!(
            cmd,
            "pi --model 'sonnet' @.mana/7-plan.md 'plan this unit carefully'"
        );
    }

    #[test]
    fn build_prompt_includes_decomposition_rules() {
        let unit = Unit::new("42", "Implement auth system");
        let prompt = build_decomposition_prompt("42", &unit, None);
        assert!(prompt.contains("Decompose unit 42"));
        assert!(prompt.contains("non-thinking model"));
        assert!(prompt.contains("Maximize parallelism"));
        assert!(prompt.contains("mana create"));
        assert!(prompt.contains("--parent 42"));
    }

    #[test]
    fn plan_builtin_dry_run_shows_prompt() {
        let (dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Big unit");
        unit.description = Some("x".repeat(2000));
        unit.to_file(mana_dir.join("1-big-unit.md")).unwrap();
        let _ = mana_core::index::Index::build(&mana_dir);
        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: "1".to_string(),
                strategy: None,
                auto: false,
                dry_run: true,
            },
        );
        assert!(result.is_ok());
        drop(dir);
    }
}
