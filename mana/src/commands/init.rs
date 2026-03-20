use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write as _};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::Config;

/// Known agent presets with their run/plan templates and detection info.
#[derive(Debug, Clone)]
struct AgentPreset {
    /// Display name (e.g. "pi")
    name: &'static str,
    /// Shell command template for `run`. `{id}` is replaced with unit ID.
    run: &'static str,
    /// Shell command template for `plan`. `{id}` is replaced with unit ID.
    plan: &'static str,
    /// Command to check if the agent is installed (e.g. "pi --version").
    version_cmd: &'static str,
    /// CLI binary name to search for in PATH.
    binary: &'static str,
}

const PRESETS: &[AgentPreset] = &[
    AgentPreset {
        name: "pi",
        run: "pi run {id}",
        plan: "pi plan {id}",
        version_cmd: "pi --version",
        binary: "pi",
    },
    AgentPreset {
        name: "claude",
        run: "claude -p 'Implement unit {id}. Read unit with mana show {id}. Read referenced files with mana context {id}. When done run mana close {id}.'",
        plan: "claude -p 'Decompose unit {id}. Read unit with mana show {id}. Break into child units with mana create --parent {id}.'",
        version_cmd: "claude --version",
        binary: "claude",
    },
    AgentPreset {
        name: "aider",
        run: "aider --message 'Implement unit {id}. Read unit with mana show {id}. Read referenced files with mana context {id}. When done run mana close {id}.'",
        plan: "aider --message 'Decompose unit {id}. Read unit with mana show {id}. Break into child units with mana create --parent {id}.'",
        version_cmd: "aider --version",
        binary: "aider",
    },
];

/// Arguments for `mana init`.
#[derive(Debug, Default)]
pub struct InitArgs {
    pub project_name: Option<String>,
    pub agent: Option<String>,
    pub run: Option<String>,
    pub plan: Option<String>,
    pub setup: bool,
    pub no_agent: bool,
}

/// Find a preset by name (case-insensitive).
fn find_preset(name: &str) -> Option<&'static AgentPreset> {
    let lower = name.to_lowercase();
    PRESETS.iter().find(|p| p.name == lower)
}

/// Check if a binary exists in PATH using `which`.
fn binary_exists(name: &str) -> Option<String> {
    Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// Detect which agent CLIs are installed.
/// Returns a list of (preset, Option<path>).
fn detect_agents() -> Vec<(&'static AgentPreset, Option<String>)> {
    PRESETS
        .iter()
        .map(|p| (p, binary_exists(p.binary)))
        .collect()
}

/// Run the agent's version command and return the output.
fn verify_agent(preset: &AgentPreset) -> Option<String> {
    let parts: Vec<&str> = preset.version_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8(o.stdout)
                .or_else(|_| String::from_utf8(o.stderr.clone()))
                .ok()
        })
        .map(|s| s.trim().to_string())
}

/// Interactive agent setup wizard (for TTY).
/// Returns (run_template, plan_template) or None if user skips.
fn interactive_agent_setup() -> Result<Option<(String, String)>> {
    let detected = detect_agents();

    eprintln!("Agent setup");
    eprintln!("  Checking for agent CLIs...");

    for (preset, path) in &detected {
        if let Some(p) = path {
            eprintln!("  ✓ {} found ({})", preset.name, p);
        } else {
            eprintln!("  ✗ {} not found", preset.name);
        }
    }
    eprintln!();

    // Build menu
    let mut options: Vec<String> = Vec::new();
    for (i, (preset, path)) in detected.iter().enumerate() {
        let marker = if path.is_some() { "✓" } else { " " };
        options.push(format!("[{}] {} {}", i + 1, marker, preset.name));
    }
    options.push(format!("[{}] custom", PRESETS.len() + 1));
    options.push(format!("[{}] skip", PRESETS.len() + 2));

    eprintln!("Which agent?  {}", options.join("  "));
    eprint!("> ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    // Parse choice
    let choice: usize = match input.parse() {
        Ok(n) => n,
        Err(_) => {
            // Try matching by name
            if let Some(preset) = find_preset(input) {
                return finish_preset_selection(preset);
            }
            eprintln!("Skipping agent setup.");
            return Ok(None);
        }
    };

    if choice == 0 || choice > PRESETS.len() + 2 {
        eprintln!("Skipping agent setup.");
        return Ok(None);
    }

    // Skip
    if choice == PRESETS.len() + 2 {
        return Ok(None);
    }

    // Custom
    if choice == PRESETS.len() + 1 {
        eprint!("Run command template (use {{id}} for unit ID): ");
        io::stderr().flush()?;
        let mut run_input = String::new();
        io::stdin().read_line(&mut run_input)?;
        let run_cmd = run_input.trim().to_string();

        eprint!("Plan command template (use {{id}} for unit ID, Enter to skip): ");
        io::stderr().flush()?;
        let mut plan_input = String::new();
        io::stdin().read_line(&mut plan_input)?;
        let plan_cmd = plan_input.trim().to_string();

        if run_cmd.is_empty() {
            eprintln!("No run command provided. Skipping agent setup.");
            return Ok(None);
        }

        let plan = if plan_cmd.is_empty() {
            run_cmd.clone()
        } else {
            plan_cmd
        };

        return Ok(Some((run_cmd, plan)));
    }

    // Preset selection (1-indexed)
    let preset = &PRESETS[choice - 1];
    finish_preset_selection(preset)
}

/// Apply a preset: verify agent and return templates.
fn finish_preset_selection(preset: &AgentPreset) -> Result<Option<(String, String)>> {
    eprintln!();
    eprintln!("Verifying {}...", preset.name);
    match verify_agent(preset) {
        Some(version) => eprintln!("  ✓ {} → {}", preset.version_cmd, version),
        None => eprintln!(
            "  ⚠ {} not responding (you can still configure it)",
            preset.name
        ),
    }

    Ok(Some((preset.run.to_string(), preset.plan.to_string())))
}

/// Initialize a .mana/ directory with a config.yaml file.
///
/// Supports agent setup via presets, custom commands, or interactive wizard.
pub fn cmd_init(path: Option<&Path>, args: InitArgs) -> Result<()> {
    let cwd = if let Some(p) = path {
        p.to_path_buf()
    } else {
        env::current_dir()?
    };
    let mana_dir = cwd.join(".mana");
    let already_exists = mana_dir.exists() && mana_dir.is_dir();

    // Re-init without --setup: show current config and hint
    if already_exists && !args.setup && args.agent.is_none() && args.run.is_none() {
        if let Ok(config) = Config::load(&mana_dir) {
            eprintln!("Project: {}", config.project);
            match &config.run {
                Some(run) => eprintln!("Run: {}", run),
                None => eprintln!("Run: (not configured)"),
            }
            match &config.plan {
                Some(plan) => eprintln!("Plan: {}", plan),
                None => eprintln!("Plan: (not configured)"),
            }
            eprintln!();
            eprintln!("To reconfigure: mana init --setup");
            return Ok(());
        }
        // Config missing/corrupt — fall through to create it
    }

    // Create .mana/ directory if it doesn't exist
    if !mana_dir.exists() {
        fs::create_dir(&mana_dir).with_context(|| {
            format!(
                "Failed to create .mana directory at {}",
                mana_dir.display()
            )
        })?;
    } else if !mana_dir.is_dir() {
        anyhow::bail!(".mana exists but is not a directory");
    }

    // Determine project name
    let project = if let Some(ref name) = args.project_name {
        name.clone()
    } else if already_exists {
        // Preserve existing project name on --setup
        Config::load(&mana_dir)
            .map(|c| c.project)
            .unwrap_or_else(|_| auto_detect_project_name(&cwd))
    } else {
        auto_detect_project_name(&cwd)
    };

    // Preserve next_id on re-init
    let next_id = if already_exists {
        Config::load(&mana_dir).map(|c| c.next_id).unwrap_or(1)
    } else {
        1
    };

    // Determine agent config (run/plan)
    let (run, plan) = resolve_agent_config(&args)?;

    // Create config
    let config = Config {
        project: project.clone(),
        next_id,
        auto_close_parent: true,
        run,
        plan,
        max_loops: 10,
        max_concurrent: 4,
        poll_interval: 30,
        extends: vec![],
        rules_file: None,
        file_locking: false,
        worktree: false,
        on_close: None,
        on_fail: None,
        post_plan: None,
        verify_timeout: None,
        review: None,
        user: None,
        user_email: None,
        auto_commit: false,
    };

    config.save(&mana_dir)?;

    // Create stub RULES.md if it doesn't exist
    let rules_path = mana_dir.join("RULES.md");
    if !rules_path.exists() {
        fs::write(
            &rules_path,
            "\
# Project Rules

<!-- These rules are automatically injected into every agent context.
     Define coding standards, conventions, and constraints here.
     Delete these comments and add your own rules. -->

<!-- Example rules:

## Code Style
- Use `snake_case` for functions and variables
- Maximum line length: 100 characters
- All public functions must have doc comments

## Architecture
- No direct database access outside the `db` module
- All errors must use the `anyhow` crate

## Forbidden Patterns
- No `.unwrap()` in production code
- No `println!` for logging (use `tracing` instead)
-->
",
        )
        .with_context(|| format!("Failed to create RULES.md at {}", rules_path.display()))?;
    }

    // Create .mana/.gitignore if it doesn't exist — index.yaml is a regenerable cache
    let gitignore_path = mana_dir.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(
            &gitignore_path,
            "# Regenerable cache — rebuilt automatically by mana sync\nindex.yaml\narchive.yaml\n\n# File lock\nindex.lock\n",
        )
        .with_context(|| format!("Failed to create .gitignore at {}", gitignore_path.display()))?;
    }

    if already_exists && args.setup {
        eprintln!("Reconfigured units in .mana/");
    } else if !already_exists {
        eprintln!("Initialized units in .mana/");
    }

    // Print next steps
    if config.run.is_some() {
        eprintln!();
        eprintln!("Next steps:");
        eprintln!("  mana create \"my first task\" --verify \"test command\"");
    } else {
        eprintln!();
        eprintln!("Next steps:");
        eprintln!("  mana init --setup          # configure an agent");
        eprintln!("  mana config set run \"...\"  # or set run command directly");
        eprintln!("  mana create \"task\" --verify \"test command\"");
    }

    Ok(())
}

/// Auto-detect project name from directory.
fn auto_detect_project_name(cwd: &Path) -> String {
    cwd.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "project".to_string())
}

/// Resolve the run/plan config from InitArgs.
///
/// Priority: --run/--plan flags > --agent preset > interactive wizard > None
fn resolve_agent_config(args: &InitArgs) -> Result<(Option<String>, Option<String>)> {
    // --no-agent: skip entirely
    if args.no_agent {
        return Ok((None, None));
    }

    // --run/--plan provided directly
    if args.run.is_some() || args.plan.is_some() {
        return Ok((args.run.clone(), args.plan.clone()));
    }

    // --agent <name>: look up preset
    if let Some(ref agent_name) = args.agent {
        let preset = find_preset(agent_name).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown agent '{}'. Known agents: {}",
                agent_name,
                PRESETS
                    .iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        eprintln!("Verifying {}...", preset.name);
        match verify_agent(preset) {
            Some(version) => eprintln!("  ✓ {} → {}", preset.version_cmd, version),
            None => eprintln!("  ⚠ {} not responding (configured anyway)", preset.name),
        }

        return Ok((Some(preset.run.to_string()), Some(preset.plan.to_string())));
    }

    // Interactive (TTY only)
    if io::stderr().is_terminal() && (args.setup || !args.no_agent) {
        if let Some((run, plan)) = interactive_agent_setup()? {
            return Ok((Some(run), Some(plan)));
        }
    }

    // Non-interactive or user skipped
    Ok((None, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create InitArgs with defaults.
    fn default_args() -> InitArgs {
        InitArgs {
            project_name: None,
            agent: None,
            run: None,
            plan: None,
            setup: false,
            no_agent: true, // Skip interactive in tests
        }
    }

    #[test]
    fn init_creates_beans_dir() {
        let dir = TempDir::new().unwrap();
        let result = cmd_init(Some(dir.path()), default_args());

        assert!(result.is_ok());
        assert!(dir.path().join(".mana").exists());
        assert!(dir.path().join(".mana").is_dir());
    }

    #[test]
    fn init_creates_config_with_explicit_name() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.project_name = Some("my-project".to_string());
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.project, "my-project");
        assert_eq!(config.next_id, 1);
    }

    #[test]
    fn init_auto_detects_project_name_from_dir() {
        let dir = TempDir::new().unwrap();
        let result = cmd_init(Some(dir.path()), default_args());

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        let dir_name = dir
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");
        assert_eq!(config.project, dir_name);
    }

    #[test]
    fn init_idempotent() {
        let dir = TempDir::new().unwrap();

        let mut args1 = default_args();
        args1.project_name = Some("test-project".to_string());
        let result1 = cmd_init(Some(dir.path()), args1);
        assert!(result1.is_ok());

        // Second init with --setup so it actually re-writes
        let mut args2 = default_args();
        args2.project_name = Some("test-project".to_string());
        args2.setup = true;
        let result2 = cmd_init(Some(dir.path()), args2);
        assert!(result2.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.project, "test-project");
    }

    #[test]
    fn init_config_is_valid_yaml() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.project_name = Some("yaml-test".to_string());
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config_path = dir.path().join(".mana").join("config.yaml");
        assert!(config_path.exists());

        let contents = fs::read_to_string(&config_path).unwrap();
        assert!(contents.contains("project: yaml-test"));
        assert!(contents.contains("next_id: 1"));
    }

    #[test]
    fn init_with_agent_pi_sets_run_and_plan() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.agent = Some("pi".to_string());
        args.no_agent = false;
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert!(config.run.is_some());
        assert!(config.plan.is_some());
        assert!(config.run.unwrap().contains("pi"));
        assert!(config.plan.unwrap().contains("pi"));
    }

    #[test]
    fn init_with_agent_claude_sets_run_and_plan() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.agent = Some("claude".to_string());
        args.no_agent = false;
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert!(config.run.is_some());
        assert!(config.plan.is_some());
        assert!(config.run.unwrap().contains("claude"));
        assert!(config.plan.unwrap().contains("claude"));
    }

    #[test]
    fn init_with_agent_aider_sets_run_and_plan() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.agent = Some("aider".to_string());
        args.no_agent = false;
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert!(config.run.is_some());
        assert!(config.plan.is_some());
        assert!(config.run.unwrap().contains("aider"));
        assert!(config.plan.unwrap().contains("aider"));
    }

    #[test]
    fn init_with_unknown_agent_errors() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.agent = Some("unknown-agent".to_string());
        args.no_agent = false;
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Unknown agent"));
        assert!(err.contains("unknown-agent"));
    }

    #[test]
    fn init_with_custom_run_and_plan() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.run = Some("my-agent run {id}".to_string());
        args.plan = Some("my-agent plan {id}".to_string());
        args.no_agent = false;
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.run, Some("my-agent run {id}".to_string()));
        assert_eq!(config.plan, Some("my-agent plan {id}".to_string()));
    }

    #[test]
    fn init_with_run_only() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.run = Some("my-agent {id}".to_string());
        args.no_agent = false;
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.run, Some("my-agent {id}".to_string()));
        assert_eq!(config.plan, None);
    }

    #[test]
    fn init_with_no_agent_skips_setup() {
        let dir = TempDir::new().unwrap();
        let mut args = default_args();
        args.no_agent = true;
        let result = cmd_init(Some(dir.path()), args);

        assert!(result.is_ok());

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.run, None);
        assert_eq!(config.plan, None);
    }

    #[test]
    fn init_setup_on_existing_reconfigures() {
        let dir = TempDir::new().unwrap();

        // First init — no agent
        let mut args1 = default_args();
        args1.project_name = Some("my-project".to_string());
        cmd_init(Some(dir.path()), args1).unwrap();

        let config1 = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config1.run, None);

        // Bump next_id to simulate usage
        let mut config_modified = config1;
        config_modified.next_id = 5;
        config_modified.save(&dir.path().join(".mana")).unwrap();

        // Re-init with --setup --agent pi
        let mut args2 = default_args();
        args2.setup = true;
        args2.agent = Some("pi".to_string());
        args2.no_agent = false;
        cmd_init(Some(dir.path()), args2).unwrap();

        let config2 = Config::load(&dir.path().join(".mana")).unwrap();
        // Agent configured
        assert!(config2.run.is_some());
        assert!(config2.run.unwrap().contains("pi"));
        // Preserved
        assert_eq!(config2.project, "my-project");
        assert_eq!(config2.next_id, 5);
    }

    #[test]
    fn reinit_without_setup_shows_config() {
        let dir = TempDir::new().unwrap();

        // First init
        let mut args1 = default_args();
        args1.project_name = Some("show-test".to_string());
        cmd_init(Some(dir.path()), args1).unwrap();

        // Second init without --setup (no flags that would trigger re-write)
        let args2 = default_args();
        let result = cmd_init(Some(dir.path()), args2);
        assert!(result.is_ok());

        // Config unchanged
        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.project, "show-test");
    }

    #[test]
    fn find_preset_is_case_insensitive() {
        assert!(find_preset("Pi").is_some());
        assert!(find_preset("PI").is_some());
        assert!(find_preset("pi").is_some());
        assert!(find_preset("Claude").is_some());
        assert!(find_preset("AIDER").is_some());
        assert!(find_preset("unknown").is_none());
    }

    #[test]
    fn detect_agents_returns_all_presets() {
        let agents = detect_agents();
        assert_eq!(agents.len(), PRESETS.len());
        // Each entry maps to a known preset
        for (preset, _) in &agents {
            assert!(PRESETS.iter().any(|p| p.name == preset.name));
        }
    }

    #[test]
    fn init_creates_rules_md_stub() {
        let dir = TempDir::new().unwrap();
        cmd_init(Some(dir.path()), default_args()).unwrap();

        let rules_path = dir.path().join(".mana").join("RULES.md");
        assert!(rules_path.exists(), "RULES.md should be created by init");

        let content = fs::read_to_string(&rules_path).unwrap();
        assert!(content.contains("# Project Rules"));
    }

    #[test]
    fn init_does_not_overwrite_existing_rules_md() {
        let dir = TempDir::new().unwrap();
        cmd_init(Some(dir.path()), default_args()).unwrap();

        // Overwrite RULES.md with custom content
        let rules_path = dir.path().join(".mana").join("RULES.md");
        fs::write(&rules_path, "# Custom rules\nNo panics allowed.").unwrap();

        // Re-init with --setup
        let mut args = default_args();
        args.setup = true;
        cmd_init(Some(dir.path()), args).unwrap();

        // Custom content preserved
        let content = fs::read_to_string(&rules_path).unwrap();
        assert!(content.contains("No panics allowed."));
    }

    #[test]
    fn init_preserves_next_id_on_setup() {
        let dir = TempDir::new().unwrap();

        // Create initial config with bumped next_id
        let mut args1 = default_args();
        args1.project_name = Some("preserve-test".to_string());
        cmd_init(Some(dir.path()), args1).unwrap();

        let mana_dir = dir.path().join(".mana");
        let mut config = Config::load(&mana_dir).unwrap();
        config.next_id = 42;
        config.save(&mana_dir).unwrap();

        // Re-init with --setup
        let mut args2 = default_args();
        args2.setup = true;
        cmd_init(Some(dir.path()), args2).unwrap();

        let config2 = Config::load(&mana_dir).unwrap();
        assert_eq!(config2.next_id, 42);
    }
}
