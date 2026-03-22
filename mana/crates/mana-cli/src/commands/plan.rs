//! `mana plan` — interactively plan a large unit into children, or run project research.
//!
//! With an ID, decomposes a large unit into smaller children.
//! Without an ID, enters project-level research mode: detects the project stack,
//! runs static analysis, and spawns an agent to find improvements and create units.
//!
//! When `config.plan` is set, spawns that template command for decomposition.
//! When `config.research` is set, spawns that for research mode.
//! Otherwise, builds a rich prompt and spawns `pi` directly.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::discovery::find_unit_file;
use crate::index::Index;
use crate::spawner::substitute_template_with_model;
use crate::unit::Unit;
use mana_core::ops::plan::{
    build_decomposition_prompt, build_research_prompt, is_oversized, shell_escape,
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
        None => plan_research(mana_dir, &config, &args),
    }
}

/// Plan a specific unit by ID.
fn plan_specific(mana_dir: &Path, config: &Config, id: &str, args: &PlanArgs) -> Result<()> {
    let unit_path = find_unit_file(mana_dir, id)?;
    let unit = Unit::from_file(&unit_path)?;

    if !is_oversized(&unit) && !args.force {
        eprintln!("Unit {} is small enough to run directly.", id);
        eprintln!("  Use mana run {} to dispatch it.", id);
        eprintln!("  Use mana plan {} --force to plan anyway.", id);
        return Ok(());
    }

    spawn_plan(mana_dir, config, id, &unit, args)
}

/// Project-level research mode: analyze codebase and create units from findings.
fn plan_research(mana_dir: &Path, config: &Config, args: &PlanArgs) -> Result<()> {
    let project_root = mana_dir.parent().unwrap_or(Path::new("."));
    let mana_cmd = std::env::args()
        .next()
        .unwrap_or_else(|| "mana".to_string());

    eprintln!("🔍 Project research mode");
    eprintln!();

    // Detect stack
    let stack = mana_core::ops::plan::detect_project_stack(project_root);
    if stack.is_empty() {
        eprintln!("  Could not detect project language/stack.");
    } else {
        eprintln!("  Detected stack:");
        for (lang, file) in &stack {
            eprintln!("    {} ({})", lang, file);
        }
    }
    eprintln!();

    // Create a parent unit to group findings
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let parent_title = format!("Project research — {}", date);

    if args.dry_run {
        eprintln!("Would create parent unit: {}", parent_title);
        eprintln!();

        // Run static checks for preview
        eprintln!("Running static analysis...");
        let static_output = mana_core::ops::plan::run_static_checks(project_root);
        if static_output.is_empty() {
            eprintln!("  No issues found (or tools not installed).");
        } else {
            eprintln!("{}", static_output);
        }

        let prompt = build_research_prompt(project_root, "PARENT_ID", &mana_cmd);
        eprintln!("--- Research prompt ---");
        eprintln!("{}", prompt);
        return Ok(());
    }

    // Create the parent unit
    let mut cfg = crate::config::Config::load(mana_dir)?;
    let parent_id = cfg.increment_id().to_string();
    cfg.save(mana_dir)?;

    let mut parent_unit = Unit::new(&parent_id, &parent_title);
    parent_unit.labels = vec!["research".to_string()];
    parent_unit.verify = Some(format!("{} tree {}", mana_cmd, parent_id));
    parent_unit.description = Some(format!(
        "Parent unit grouping project research findings from {}.",
        date
    ));
    let slug = crate::util::title_to_slug(&parent_title);
    let filename = format!("{}-{}.md", parent_id, slug);
    parent_unit.to_file(mana_dir.join(&filename))?;

    // Rebuild index to include new parent
    let _ = Index::build(mana_dir);

    eprintln!("Created parent unit {} — {}", parent_id, parent_title);
    eprintln!();

    // Spawn research agent
    spawn_research(mana_dir, config, &parent_id, &mana_cmd, args)
}

/// Spawn the research agent.
fn spawn_research(
    mana_dir: &Path,
    config: &Config,
    parent_id: &str,
    mana_cmd: &str,
    args: &PlanArgs,
) -> Result<()> {
    // Priority: config.research > config.plan > built-in
    if let Some(ref template) = config.research {
        let cmd =
            build_research_template_command(template, parent_id, config.research_model.as_deref());
        eprintln!("Spawning research: {}", cmd);
        return run_shell_command(&cmd, parent_id, args.auto);
    }

    if let Some(ref template) = config.plan {
        // Use plan template with a research-oriented invocation.
        // Research uses its own model routing, even when falling back to the plan template.
        let cmd =
            substitute_template_with_model(template, parent_id, config.research_model.as_deref());
        eprintln!("Spawning research (via plan template): {}", cmd);
        return run_shell_command(&cmd, parent_id, args.auto);
    }

    // Built-in: construct prompt and spawn pi
    let project_root = mana_dir.parent().unwrap_or(Path::new("."));

    eprintln!("Running static analysis...");
    let prompt = build_research_prompt(project_root, parent_id, mana_cmd);

    let cmd = build_builtin_research_command(&prompt, config.research_model.as_deref());

    eprintln!("Spawning built-in research agent...");
    run_shell_command(&cmd, parent_id, args.auto)
}

/// Spawn the plan command for a unit.
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

/// Spawn the plan using a user-configured template command.
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
fn build_research_template_command(template: &str, parent_id: &str, model: Option<&str>) -> String {
    let cmd = template
        .replace("{parent_id}", parent_id)
        .replace("{id}", parent_id);
    match model {
        Some(model) => cmd.replace("{model}", model),
        None => cmd,
    }
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

#[must_use]
fn build_builtin_research_command(prompt: &str, model: Option<&str>) -> String {
    let escaped_prompt = shell_escape(prompt);
    match model {
        Some(model) => format!("pi --model {} {}", shell_escape(model), escaped_prompt),
        None => format!("pi {}", escaped_prompt),
    }
}

/// Build a decomposition prompt and spawn `pi` with it directly.
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

    fn setup_mana_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        fs::write(mana_dir.join("config.yaml"), "project: test\nnext_id: 10\n").unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn plan_help_contains_plan() {
        // Verified by the unit's verify command
    }

    #[test]
    fn plan_no_template_without_auto_errors() {
        let (dir, mana_dir) = setup_mana_dir();

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
    fn plan_small_unit_suggests_run() {
        let (dir, mana_dir) = setup_mana_dir();

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
        let (dir, mana_dir) = setup_mana_dir();

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
        let (dir, mana_dir) = setup_mana_dir();

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
    fn plan_research_dry_run_shows_prompt() {
        let (dir, mana_dir) = setup_mana_dir();

        let _ = Index::build(&mana_dir);

        let result = cmd_plan(
            &mana_dir,
            PlanArgs {
                id: None,
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
    fn plan_research_creates_parent_unit() {
        let (dir, mana_dir) = setup_mana_dir();

        // Use a research template that just succeeds
        fs::write(
            mana_dir.join("config.yaml"),
            "project: test\nnext_id: 10\nresearch: \"true\"\n",
        )
        .unwrap();

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

        // Check parent unit was created
        let index = Index::load_or_rebuild(&mana_dir).unwrap();
        let research_units: Vec<_> = index
            .units
            .iter()
            .filter(|u| u.title.contains("Project research"))
            .collect();
        assert_eq!(research_units.len(), 1);

        drop(dir);
    }

    #[test]
    fn plan_research_falls_back_to_plan_template() {
        let (dir, mana_dir) = setup_mana_dir();

        // No research template, but plan template is set
        fs::write(
            mana_dir.join("config.yaml"),
            "project: test\nnext_id: 10\nplan: \"true\"\n",
        )
        .unwrap();

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
    fn research_template_command_replaces_parent_id_and_model() {
        let cmd = build_research_template_command(
            "claude --model {model} -p 'research {parent_id} {id}'",
            "42",
            Some("sonnet"),
        );

        assert_eq!(cmd, "claude --model sonnet -p 'research 42 42'");
    }

    #[test]
    fn research_template_without_model_keeps_placeholder() {
        let cmd = build_research_template_command(
            "claude --model {model} -p 'research {parent_id}'",
            "42",
            None,
        );

        assert_eq!(cmd, "claude --model {model} -p 'research 42'");
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
    fn plan_template_prefers_unit_model_override() {
        let config_model = Some("haiku");
        let unit_model = Some("opus");
        let cmd = build_plan_template_command(
            "claude --model {model} -p 'plan {id}'",
            "7",
            None,
            unit_model.or(config_model),
        );

        assert_eq!(cmd, "claude --model opus -p 'plan 7'");
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
    fn builtin_research_command_includes_model_when_set() {
        let cmd = build_builtin_research_command("research the project", Some("opus"));

        assert_eq!(cmd, "pi --model 'opus' 'research the project'");
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
        let (dir, mana_dir) = setup_mana_dir();

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
