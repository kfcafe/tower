use std::env;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::config::Config;

/// Parameters for initializing a units project.
#[derive(Debug, Default)]
pub struct InitParams {
    pub project_name: Option<String>,
    pub run: Option<String>,
    pub plan: Option<String>,
}

/// Result of initialization.
pub struct InitResult {
    pub project: String,
    pub already_existed: bool,
    pub run: Option<String>,
    pub plan: Option<String>,
}

/// Initialize a .mana/ directory with config and default files.
///
/// Creates .mana/, config.yaml, RULES.md stub, and .gitignore.
/// Preserves existing project name and next_id on re-init.
/// Returns structured result so callers can format output.
pub fn init(path: Option<&Path>, params: InitParams) -> Result<InitResult> {
    let cwd = if let Some(p) = path {
        p.to_path_buf()
    } else {
        env::current_dir()?
    };
    let mana_dir = cwd.join(".mana");
    let already_existed = mana_dir.exists() && mana_dir.is_dir();

    if !mana_dir.exists() {
        fs::create_dir(&mana_dir).with_context(|| {
            format!("Failed to create .mana directory at {}", mana_dir.display())
        })?;
    } else if !mana_dir.is_dir() {
        anyhow::bail!(".mana exists but is not a directory");
    }

    let project = if let Some(ref name) = params.project_name {
        name.clone()
    } else if already_existed {
        Config::load(&mana_dir)
            .map(|c| c.project)
            .unwrap_or_else(|_| auto_detect_project_name(&cwd))
    } else {
        auto_detect_project_name(&cwd)
    };

    let next_id = if already_existed {
        Config::load(&mana_dir).map(|c| c.next_id).unwrap_or(1)
    } else {
        1
    };

    let config = Config {
        project: project.clone(),
        next_id,
        auto_close_parent: true,
        run: params.run.clone(),
        plan: params.plan.clone(),
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
        commit_template: None,
        research: None,
        run_model: None,
        plan_model: None,
        review_model: None,
        research_model: None,
        batch_verify: false,
    };

    config.save(&mana_dir)?;

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

    let gitignore_path = mana_dir.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(
            &gitignore_path,
            "# Regenerable cache — rebuilt automatically by mana sync\nindex.yaml\narchive.yaml\n\n# File lock\nindex.lock\n",
        )
        .with_context(|| format!("Failed to create .gitignore at {}", gitignore_path.display()))?;
    }

    Ok(InitResult {
        project,
        already_existed,
        run: params.run,
        plan: params.plan,
    })
}

/// Auto-detect project name from directory name.
pub fn auto_detect_project_name(cwd: &Path) -> String {
    cwd.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "project".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn init_creates_beans_dir() {
        let dir = TempDir::new().unwrap();
        let result = init(Some(dir.path()), InitParams::default()).unwrap();

        assert!(!result.already_existed);
        assert!(dir.path().join(".mana").exists());
        assert!(dir.path().join(".mana").is_dir());
    }

    #[test]
    fn init_creates_config() {
        let dir = TempDir::new().unwrap();
        init(
            Some(dir.path()),
            InitParams {
                project_name: Some("my-project".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.project, "my-project");
        assert_eq!(config.next_id, 1);
    }

    #[test]
    fn init_preserves_next_id_on_reinit() {
        let dir = TempDir::new().unwrap();
        init(Some(dir.path()), InitParams::default()).unwrap();

        let mana_dir = dir.path().join(".mana");
        let mut config = Config::load(&mana_dir).unwrap();
        config.next_id = 42;
        config.save(&mana_dir).unwrap();

        let result = init(Some(dir.path()), InitParams::default()).unwrap();

        assert!(result.already_existed);
        let config = Config::load(&mana_dir).unwrap();
        assert_eq!(config.next_id, 42);
    }

    #[test]
    fn init_creates_rules_md() {
        let dir = TempDir::new().unwrap();
        init(Some(dir.path()), InitParams::default()).unwrap();

        let rules_path = dir.path().join(".mana").join("RULES.md");
        assert!(rules_path.exists());
        let content = fs::read_to_string(&rules_path).unwrap();
        assert!(content.contains("# Project Rules"));
    }

    #[test]
    fn init_does_not_overwrite_rules_md() {
        let dir = TempDir::new().unwrap();
        init(Some(dir.path()), InitParams::default()).unwrap();

        let rules_path = dir.path().join(".mana").join("RULES.md");
        fs::write(&rules_path, "# Custom rules").unwrap();

        init(Some(dir.path()), InitParams::default()).unwrap();

        let content = fs::read_to_string(&rules_path).unwrap();
        assert!(content.contains("# Custom rules"));
    }

    #[test]
    fn init_with_run_and_plan() {
        let dir = TempDir::new().unwrap();
        let result = init(
            Some(dir.path()),
            InitParams {
                run: Some("pi run {id}".to_string()),
                plan: Some("pi plan {id}".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.run, Some("pi run {id}".to_string()));
        let config = Config::load(&dir.path().join(".mana")).unwrap();
        assert_eq!(config.run, Some("pi run {id}".to_string()));
        assert_eq!(config.plan, Some("pi plan {id}".to_string()));
    }
}
