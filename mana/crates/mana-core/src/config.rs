//! Project and global configuration.
//!
//! Configuration is stored in `.mana/config.yaml` (project-level) and
//! `~/.config/mana/config.yaml` (global/user-level).
//!
//! ## Loading config
//!
//! ```rust,no_run
//! use mana_core::config::Config;
//! use std::path::Path;
//!
//! let mana_dir = Path::new("/project/.mana");
//!
//! // Load project config only
//! let config = Config::load(mana_dir).unwrap();
//!
//! // Load with inheritance from `extends` paths (recommended)
//! let config = Config::load_with_extends(mana_dir).unwrap();
//! println!("Project: {}", config.project);
//! println!("Max concurrent agents: {}", config.max_concurrent);
//! ```
//!
//! ## Config inheritance
//!
//! A project config can extend shared configs via the `extends` field:
//!
//! ```yaml
//! project: my-project
//! next_id: 42
//! extends:
//!   - ~/shared/mana-config.yaml
//!   - ../team-defaults.yaml
//! ```
//!
//! Extended configs are merged with the local config taking precedence.
//! The `project`, `next_id`, and `extends` fields are never inherited.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_COMMIT_TEMPLATE: &str = "feat(unit-{id}): {title}";

/// Notification configuration for human-facing alerts.
///
/// Commands are shell templates run via `sh -c` with variable interpolation.
/// All are fire-and-forget — failures are logged but never block operations.
///
/// ## Template variables
///
/// | Variable | Available in | Description |
/// |----------|-------------|-------------|
/// | `{id}` | all | Unit ID |
/// | `{title}` | all | Unit title |
/// | `{status}` | on_close, on_scheduled_complete | "pass" or "fail" |
/// | `{verify_output}` | on_close | First 200 chars of verify output |
/// | `{attempt}` | on_fail | Current attempt number |
/// | `{max_attempts}` | on_fail | Max attempts configured |
/// | `{output}` | on_fail | First 200 chars of verify output |
/// | `{schedule}` | on_scheduled_complete | Schedule expression |
/// | `{next_run_at}` | on_scheduled_complete | Next scheduled run time |
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct NotifyConfig {
    /// Command run when a unit closes successfully.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_close: Option<String>,

    /// Command run when a unit's verify fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_fail: Option<String>,

    /// Command run when a scheduled unit completes (pass or fail).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_scheduled_complete: Option<String>,
}

/// Configuration for the adversarial review feature (`mana review` / `mana run --review`).
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ReviewConfig {
    /// Shell command template for the review agent. Use `{id}` as placeholder for unit ID.
    /// If unset, falls back to the global `run` template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Maximum number of times review can reopen a unit before giving up (default: 2).
    #[serde(default = "default_max_reopens")]
    pub max_reopens: u32,
}

fn default_max_reopens() -> u32 {
    2
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            run: None,
            max_reopens: 2,
        }
    }
}

/// Project-level mana configuration, loaded from `.mana/config.yaml`.
///
/// All fields have sensible defaults; only `project` and `next_id` are
/// required in the YAML file. Optional fields are omitted when serialized
/// to keep config files minimal.
///
/// Use [`Config::load_with_extends`] to load with inherited values from
/// parent configs listed in the `extends` field.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub project: String,
    pub next_id: u32,
    /// Auto-close parent units when all children are closed/archived (default: true)
    #[serde(default = "default_auto_close_parent")]
    pub auto_close_parent: bool,
    /// Shell command template for `--run`. Use `{id}` as placeholder for unit ID.
    /// Example: `claude -p "implement unit {id} and run mana close {id}"`.
    /// If unset, `--run` will print an error asking the user to configure it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Shell command template for planning large units. Uses `{id}` placeholder.
    /// If unset, plan operations will print an error asking the user to configure it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Maximum agent loops before stopping (default: 10, 0 = unlimited)
    #[serde(default = "default_max_loops")]
    pub max_loops: u32,
    /// Maximum parallel agents for `mana run` (default: 4)
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
    /// Seconds between polls in --watch mode (default: 30)
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u32,
    /// Paths to parent config files to inherit from (lowest to highest priority).
    /// Supports `~/` for home directory. Paths are relative to the project root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extends: Vec<String>,
    /// Path to project rules file, relative to .mana/ directory (default: "RULES.md").
    /// Contents are injected into every `mana context` output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_file: Option<String>,
    /// Enable file locking for concurrent agents (default: false).
    /// When enabled, agents lock files listed in unit `paths` on spawn
    /// and lock-on-write during execution. Prevents concurrent agents
    /// from clobbering the same file.
    #[serde(default, skip_serializing_if = "is_false_bool")]
    pub file_locking: bool,
    /// Enable git worktree isolation for parallel agents (default: false).
    /// When enabled, `mana run` creates a separate git worktree for each agent.
    /// Each agent works in its own directory, preventing file contention.
    /// On `mana close`, the worktree branch is merged back to main.
    #[serde(default, skip_serializing_if = "is_false_bool")]
    pub worktree: bool,
    /// Shell command template to run after a unit is successfully closed.
    /// Supports template variables: {id}, {title}, {status}, {branch}.
    /// Runs asynchronously — failures are logged but don't affect the close.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_close: Option<String>,
    /// Shell command template to run after a verify attempt fails.
    /// Supports template variables: {id}, {title}, {attempt}, {output}, {branch}.
    /// Runs asynchronously — failures are logged but don't affect the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_fail: Option<String>,
    /// Shell command template to run after `mana plan` creates children.
    /// Supports template variables: {id}, {parent}, {children}, {branch}.
    /// Runs asynchronously — failures are logged but don't affect the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_plan: Option<String>,
    /// Default timeout in seconds for verify commands (default: None = no limit).
    /// Per-unit `verify_timeout` overrides this value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_timeout: Option<u64>,
    /// Adversarial review configuration (`mana review` / `mana run --review`).
    /// Optional — review is disabled if not configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewConfig>,
    /// User identity name (e.g., "alice"). Used for claimed_by and created_by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// User email (e.g., "alice@co"). Optional, for git integration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,
    /// Automatically commit all changes when a unit is closed (default: false).
    /// Creates a commit with message based on `commit_template`.
    /// Skipped in worktree mode (worktree already commits).
    #[serde(default, skip_serializing_if = "is_false_bool")]
    pub auto_commit: bool,
    /// Template for auto-commit messages. Placeholders: {id}, {title}, {parent_id}, {labels}.
    /// Default: "feat(unit-{id}): {title}"
    ///
    /// Keep `{id}` in the template so `mana diff <id>` can find the unit's commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_template: Option<String>,
    /// Shell command template for project-level research (`mana plan` with no ID).
    /// Uses `{parent_id}` as placeholder for the parent unit that groups findings.
    /// Falls back to `plan` template with a research-oriented prompt if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research: Option<String>,
    /// Model to use for implementing units (`mana run`). Substituted into `{model}` in templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_model: Option<String>,
    /// Model to use for planning/splitting (`mana plan`). Substituted into `{model}` in templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_model: Option<String>,
    /// Model to use for adversarial review (`mana run --review`). Substituted into `{model}` in templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_model: Option<String>,
    /// Model to use for project research (`mana plan` with no args). Substituted into `{model}` in templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_model: Option<String>,
    /// Enable runner-mediated batch verification (default: false).
    /// When enabled, `mana run` sets `MANA_BATCH_VERIFY=1` on spawned agents.
    /// Agents signal completion without running verify inline; the runner
    /// collects AwaitingVerify units and runs each unique verify command once.
    #[serde(default, skip_serializing_if = "is_false_bool")]
    pub batch_verify: bool,
    /// Minimum available system memory (MB) to keep free when spawning agents.
    /// When set to a non-zero value, `mana run` checks available system memory
    /// before each agent spawn. If available memory is below this threshold,
    /// dispatch pauses until a running agent finishes and frees memory.
    /// Default: 0 (disabled). Recommended: 2048–4096 on a 16GB machine.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub memory_reserve_mb: u64,
    /// Notification settings for human-facing alerts (push notifications,
    /// desktop alerts, webhook pings). Separate from on_close/on_fail workflow
    /// hooks — those are for automation, these are for humans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<NotifyConfig>,
}

fn default_auto_close_parent() -> bool {
    true
}

fn default_max_loops() -> u32 {
    10
}

fn default_max_concurrent() -> u32 {
    4
}

fn default_poll_interval() -> u32 {
    30
}

fn is_false_bool(v: &bool) -> bool {
    !v
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project: String::new(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
            max_loops: 10,
            max_concurrent: 4,
            poll_interval: 30,
            extends: Vec::new(),
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
            memory_reserve_mb: 0,
            notify: None,
        }
    }
}

impl Config {
    /// Load config from .mana/config.yaml inside the given units directory.
    pub fn load(mana_dir: &Path) -> Result<Self> {
        let path = mana_dir.join("config.yaml");
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let config: Config = serde_yml::from_str(&contents)
            .with_context(|| format!("Failed to parse config at {}", path.display()))?;
        Ok(config)
    }

    /// Load config with inheritance from extended configs.
    ///
    /// Resolves the `extends` field, loading parent configs and merging
    /// inheritable fields. Local values take precedence over extended values.
    /// Fields `project`, `next_id`, and `extends` are never inherited.
    pub fn load_with_extends(mana_dir: &Path) -> Result<Self> {
        let mut config = Self::load(mana_dir)?;

        if config.extends.is_empty() {
            return Ok(config);
        }

        let mut seen = HashSet::new();
        let mut stack: Vec<String> = config.extends.clone();
        let mut parents: Vec<Config> = Vec::new();

        while let Some(path_str) = stack.pop() {
            let resolved = Self::resolve_extends_path(&path_str, mana_dir)?;

            let canonical = resolved
                .canonicalize()
                .with_context(|| format!("Cannot resolve extends path: {}", path_str))?;

            if !seen.insert(canonical.clone()) {
                continue; // Cycle detection
            }

            let contents = fs::read_to_string(&canonical).with_context(|| {
                format!("Failed to read extends config: {}", canonical.display())
            })?;
            let parent: Config = serde_yml::from_str(&contents).with_context(|| {
                format!("Failed to parse extends config: {}", canonical.display())
            })?;

            for ext in &parent.extends {
                stack.push(ext.clone());
            }

            parents.push(parent);
        }

        // Merge: closest parent first (highest priority among parents).
        // Only override local values that are still at their defaults.
        for parent in &parents {
            if config.run.is_none() {
                config.run = parent.run.clone();
            }
            if config.plan.is_none() {
                config.plan = parent.plan.clone();
            }
            if config.max_loops == default_max_loops() {
                config.max_loops = parent.max_loops;
            }
            if config.max_concurrent == default_max_concurrent() {
                config.max_concurrent = parent.max_concurrent;
            }
            if config.poll_interval == default_poll_interval() {
                config.poll_interval = parent.poll_interval;
            }
            if config.auto_close_parent == default_auto_close_parent() {
                config.auto_close_parent = parent.auto_close_parent;
            }
            if config.rules_file.is_none() {
                config.rules_file = parent.rules_file.clone();
            }
            if !config.file_locking {
                config.file_locking = parent.file_locking;
            }
            if !config.worktree {
                config.worktree = parent.worktree;
            }
            if config.on_close.is_none() {
                config.on_close = parent.on_close.clone();
            }
            if config.on_fail.is_none() {
                config.on_fail = parent.on_fail.clone();
            }
            if config.post_plan.is_none() {
                config.post_plan = parent.post_plan.clone();
            }
            if config.verify_timeout.is_none() {
                config.verify_timeout = parent.verify_timeout;
            }
            if config.review.is_none() {
                config.review = parent.review.clone();
            }
            if config.user.is_none() {
                config.user = parent.user.clone();
            }
            if config.user_email.is_none() {
                config.user_email = parent.user_email.clone();
            }
            if !config.auto_commit {
                config.auto_commit = parent.auto_commit;
            }
            if config.commit_template.is_none() {
                config.commit_template = parent.commit_template.clone();
            }
            if config.research.is_none() {
                config.research = parent.research.clone();
            }
            if config.run_model.is_none() {
                config.run_model = parent.run_model.clone();
            }
            if config.plan_model.is_none() {
                config.plan_model = parent.plan_model.clone();
            }
            if config.review_model.is_none() {
                config.review_model = parent.review_model.clone();
            }
            if config.research_model.is_none() {
                config.research_model = parent.research_model.clone();
            }
            // Never inherit: project, next_id, extends
        }

        Ok(config)
    }

    /// Resolve an extends path to an absolute path.
    /// `~/` expands to the home directory; other paths are relative to the project root.
    fn resolve_extends_path(path_str: &str, mana_dir: &Path) -> Result<PathBuf> {
        if let Some(stripped) = path_str.strip_prefix("~/") {
            let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot resolve home directory"))?;
            Ok(home.join(stripped))
        } else {
            // Resolve relative to the project root (parent of .mana/)
            let project_root = mana_dir.parent().unwrap_or(Path::new("."));
            Ok(project_root.join(path_str))
        }
    }

    /// Save config to .mana/config.yaml inside the given units directory.
    pub fn save(&self, mana_dir: &Path) -> Result<()> {
        let path = mana_dir.join("config.yaml");
        let contents = serde_yml::to_string(self).context("Failed to serialize config")?;
        fs::write(&path, &contents)
            .with_context(|| format!("Failed to write config at {}", path.display()))?;
        Ok(())
    }

    /// Return the path to the project rules file.
    /// Defaults to `.mana/RULES.md` if `rules_file` is not set.
    /// The path is resolved relative to the units directory.
    pub fn rules_path(&self, mana_dir: &Path) -> PathBuf {
        match &self.rules_file {
            Some(custom) => {
                let p = Path::new(custom);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    mana_dir.join(custom)
                }
            }
            None => mana_dir.join("RULES.md"),
        }
    }

    /// Return the current next_id and increment it for the next call.
    pub fn increment_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

// ---------------------------------------------------------------------------
// Global config (~/.config/mana/config.yaml)
// ---------------------------------------------------------------------------

/// Minimal global config stored at `~/.config/mana/config.yaml`.
/// Only holds user identity fields — project-level config has everything else.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,
}

impl GlobalConfig {
    /// Path to the new global config file: `~/.config/mana/config.yaml`.
    pub fn path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot determine home directory"))?;
        Ok(home.join(".config").join("mana").join("config.yaml"))
    }

    /// Path to the legacy global config file (`~/.config/` + `units/config.yaml`).
    /// Used as a read-only fallback during migration.
    fn legacy_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot determine home directory"))?;
        Ok(home.join(".config").join("units").join("config.yaml"))
    }

    /// Load global config. Returns Default if file doesn't exist.
    ///
    /// Falls back to the legacy `units` config directory if the new path doesn't exist
    /// but the old one does, to support migration from the old location.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read global config at {}", path.display()))?;
            let config: GlobalConfig = serde_yml::from_str(&contents)
                .with_context(|| format!("Failed to parse global config at {}", path.display()))?;
            return Ok(config);
        }

        // Backwards-compatible fallback: read from legacy units config path.
        if let Ok(legacy) = Self::legacy_path() {
            if legacy.exists() {
                let contents = fs::read_to_string(&legacy).with_context(|| {
                    format!(
                        "Failed to read legacy global config at {}",
                        legacy.display()
                    )
                })?;
                let config: GlobalConfig = serde_yml::from_str(&contents).with_context(|| {
                    format!(
                        "Failed to parse legacy global config at {}",
                        legacy.display()
                    )
                })?;
                return Ok(config);
            }
        }

        Ok(Self::default())
    }

    /// Save global config to `~/.config/mana/config.yaml`, creating parent directories if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let contents = serde_yml::to_string(self).context("Failed to serialize global config")?;
        fs::write(&path, &contents)
            .with_context(|| format!("Failed to write global config at {}", path.display()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Identity resolution
// ---------------------------------------------------------------------------

/// Resolve the current user identity using a priority chain:
///
/// 1. Project config `user` field (from `.mana/config.yaml`)
/// 2. Global config `user` field (from `~/.config/mana/config.yaml`)
/// 3. `git config user.name` (fallback)
/// 4. `$USER` environment variable (last resort)
///
/// Returns `None` only if all sources fail.
pub fn resolve_identity(mana_dir: &Path) -> Option<String> {
    // 1. Project config
    if let Ok(config) = Config::load(mana_dir) {
        if let Some(ref user) = config.user {
            if !user.is_empty() {
                return Some(user.clone());
            }
        }
    }

    // 2. Global config
    if let Ok(global) = GlobalConfig::load() {
        if let Some(ref user) = global.user {
            if !user.is_empty() {
                return Some(user.clone());
            }
        }
    }

    // 3. git config user.name
    if let Some(git_user) = git_config_user_name() {
        return Some(git_user);
    }

    // 4. $USER env var
    std::env::var("USER").ok().filter(|u| !u.is_empty())
}

/// Try to get `git config user.name`. Returns None on failure.
fn git_config_user_name() -> Option<String> {
    Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn config_round_trips_through_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test-project".to_string(),
            next_id: 42,
            auto_close_parent: true,
            run: None,
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };

        config.save(dir.path()).unwrap();
        let loaded = Config::load(dir.path()).unwrap();

        assert_eq!(config, loaded);
    }

    #[test]
    fn increment_id_returns_current_and_bumps() {
        let mut config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };

        assert_eq!(config.increment_id(), 1);
        assert_eq!(config.increment_id(), 2);
        assert_eq!(config.increment_id(), 3);
        assert_eq!(config.next_id, 4);
    }

    #[test]
    fn load_returns_error_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = Config::load(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn load_returns_error_for_invalid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.yaml"), "not: [valid: yaml: config").unwrap();
        let result = Config::load(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn save_creates_file_that_is_valid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "my-project".to_string(),
            next_id: 100,
            auto_close_parent: true,
            run: None,
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let contents = fs::read_to_string(dir.path().join("config.yaml")).unwrap();
        assert!(contents.contains("project: my-project"));
        assert!(contents.contains("next_id: 100"));
    }

    #[test]
    fn auto_close_parent_defaults_to_true() {
        let dir = tempfile::tempdir().unwrap();
        // Write a config WITHOUT auto_close_parent field
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert!(loaded.auto_close_parent);
    }

    #[test]
    fn auto_close_parent_can_be_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: false,
            run: None,
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert!(!loaded.auto_close_parent);
    }

    #[test]
    fn max_tokens_in_yaml_silently_ignored() {
        let dir = tempfile::tempdir().unwrap();
        // Existing configs in the wild may have max_tokens — must not error
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\nmax_tokens: 50000\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.project, "test");
    }

    #[test]
    fn run_defaults_to_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.run, None);
    }

    #[test]
    fn run_can_be_set() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: Some("claude -p 'implement unit {id}'".to_string()),
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(
            loaded.run,
            Some("claude -p 'implement unit {id}'".to_string())
        );
    }

    #[test]
    fn run_not_serialized_when_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let contents = fs::read_to_string(dir.path().join("config.yaml")).unwrap();
        assert!(!contents.contains("run:"));
    }

    #[test]
    fn max_loops_defaults_to_10() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.max_loops, 10);
    }

    #[test]
    fn max_loops_can_be_customized() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
            max_loops: 25,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.max_loops, 25);
    }

    // --- extends tests ---

    /// Helper: write a YAML config file at the given path.
    fn write_yaml(path: &std::path::Path, yaml: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, yaml).unwrap();
    }

    /// Helper: write a minimal local config inside a units dir, with extends.
    fn write_local_config(mana_dir: &std::path::Path, extends: &[&str], extra: &str) {
        let extends_yaml: Vec<String> = extends.iter().map(|e| format!("  - \"{}\"", e)).collect();
        let extends_block = if extends.is_empty() {
            String::new()
        } else {
            format!("extends:\n{}\n", extends_yaml.join("\n"))
        };
        let yaml = format!("project: test\nnext_id: 1\n{}{}", extends_block, extra);
        write_yaml(&mana_dir.join("config.yaml"), &yaml);
    }

    #[test]
    fn extends_empty_loads_normally() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();
        write_local_config(&mana_dir, &[], "");

        let config = Config::load_with_extends(&mana_dir).unwrap();
        assert_eq!(config.project, "test");
        assert!(config.run.is_none());
    }

    #[test]
    fn extends_single_merges_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        // Parent config (outside .mana, at project root)
        let parent_path = dir.path().join("shared.yaml");
        write_yaml(
            &parent_path,
            "project: shared\nnext_id: 999\nrun: \"deli spawn {id}\"\nmax_loops: 20\n",
        );

        write_local_config(&mana_dir, &["shared.yaml"], "");

        let config = Config::load_with_extends(&mana_dir).unwrap();
        // Inherited
        assert_eq!(config.run, Some("deli spawn {id}".to_string()));
        assert_eq!(config.max_loops, 20);
        // Never inherited
        assert_eq!(config.project, "test");
        assert_eq!(config.next_id, 1);
    }

    #[test]
    fn extends_local_overrides_parent() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        let parent_path = dir.path().join("shared.yaml");
        write_yaml(
            &parent_path,
            "project: shared\nnext_id: 999\nrun: \"parent-run\"\nmax_loops: 20\n",
        );

        // Local config sets its own run
        write_local_config(
            &mana_dir,
            &["shared.yaml"],
            "run: \"local-run\"\nmax_loops: 5\n",
        );

        let config = Config::load_with_extends(&mana_dir).unwrap();
        // Local values win
        assert_eq!(config.run, Some("local-run".to_string()));
        assert_eq!(config.max_loops, 5);
    }

    #[test]
    fn extends_circular_detected_and_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        // A extends B, B extends A
        let a_path = dir.path().join("a.yaml");
        let b_path = dir.path().join("b.yaml");
        write_yaml(
            &a_path,
            "project: a\nnext_id: 1\nextends:\n  - \"b.yaml\"\nmax_loops: 40\n",
        );
        write_yaml(
            &b_path,
            "project: b\nnext_id: 1\nextends:\n  - \"a.yaml\"\nmax_loops: 50\n",
        );

        write_local_config(&mana_dir, &["a.yaml"], "");

        // Should not infinite loop; loads successfully
        let config = Config::load_with_extends(&mana_dir).unwrap();
        assert_eq!(config.project, "test");
        // Gets value from one of the parents
        assert!(config.max_loops == 40 || config.max_loops == 50);
    }

    #[test]
    fn extends_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        write_local_config(&mana_dir, &["nonexistent.yaml"], "");

        let result = Config::load_with_extends(&mana_dir);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("nonexistent.yaml"),
            "Error should mention the missing file: {}",
            err_msg
        );
    }

    #[test]
    fn extends_recursive_a_extends_b_extends_c() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        // C: base config
        let c_path = dir.path().join("c.yaml");
        write_yaml(
            &c_path,
            "project: c\nnext_id: 1\nrun: \"from-c\"\nmax_loops: 40\n",
        );

        // B extends C, overrides max_loops
        let b_path = dir.path().join("b.yaml");
        write_yaml(
            &b_path,
            "project: b\nnext_id: 1\nextends:\n  - \"c.yaml\"\nmax_loops: 50\n",
        );

        // Local extends B
        write_local_config(&mana_dir, &["b.yaml"], "");

        let config = Config::load_with_extends(&mana_dir).unwrap();
        // B's max_loops (50) should apply since it's the direct parent
        assert_eq!(config.max_loops, 50);
        // run comes from C (B doesn't set it, but C does)
        assert_eq!(config.run, Some("from-c".to_string()));
    }

    #[test]
    fn extends_project_and_next_id_never_inherited() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        let parent_path = dir.path().join("shared.yaml");
        write_yaml(
            &parent_path,
            "project: parent-project\nnext_id: 999\nmax_loops: 50\n",
        );

        write_local_config(&mana_dir, &["shared.yaml"], "");

        let config = Config::load_with_extends(&mana_dir).unwrap();
        assert_eq!(config.project, "test");
        assert_eq!(config.next_id, 1);
    }

    #[test]
    fn extends_tilde_resolves_to_home_dir() {
        // We can't fully test ~ expansion without writing to the real home dir,
        // but we can verify the path resolution logic.
        let mana_dir = std::path::Path::new("/tmp/fake-units");
        let resolved = Config::resolve_extends_path("~/shared/config.yaml", mana_dir).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(resolved, home.join("shared/config.yaml"));
    }

    #[test]
    fn extends_not_serialized_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let contents = fs::read_to_string(dir.path().join("config.yaml")).unwrap();
        assert!(!contents.contains("extends"));
    }

    #[test]
    fn extends_defaults_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert!(loaded.extends.is_empty());
    }

    // --- plan, max_concurrent, poll_interval tests ---

    #[test]
    fn plan_defaults_to_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.plan, None);
    }

    #[test]
    fn plan_can_be_set() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: Some("claude -p 'plan unit {id}'".to_string()),
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.plan, Some("claude -p 'plan unit {id}'".to_string()));
    }

    #[test]
    fn plan_not_serialized_when_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let contents = fs::read_to_string(dir.path().join("config.yaml")).unwrap();
        assert!(!contents.contains("plan:"));
    }

    #[test]
    fn max_concurrent_defaults_to_4() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.max_concurrent, 4);
    }

    #[test]
    fn max_concurrent_can_be_customized() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
            max_loops: 10,
            max_concurrent: 8,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.max_concurrent, 8);
    }

    #[test]
    fn poll_interval_defaults_to_30() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.poll_interval, 30);
    }

    #[test]
    fn poll_interval_can_be_customized() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: None,
            max_loops: 10,
            max_concurrent: 4,
            poll_interval: 60,
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
            memory_reserve_mb: 0,
            notify: None,
        };
        config.save(dir.path()).unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert_eq!(loaded.poll_interval, 60);
    }

    #[test]
    fn extends_inherits_plan() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        let parent_path = dir.path().join("shared.yaml");
        write_yaml(
            &parent_path,
            "project: shared\nnext_id: 999\nplan: \"plan-cmd {id}\"\n",
        );

        write_local_config(&mana_dir, &["shared.yaml"], "");

        let config = Config::load_with_extends(&mana_dir).unwrap();
        assert_eq!(config.plan, Some("plan-cmd {id}".to_string()));
    }

    #[test]
    fn extends_inherits_max_concurrent() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        let parent_path = dir.path().join("shared.yaml");
        write_yaml(
            &parent_path,
            "project: shared\nnext_id: 999\nmax_concurrent: 16\n",
        );

        write_local_config(&mana_dir, &["shared.yaml"], "");

        let config = Config::load_with_extends(&mana_dir).unwrap();
        assert_eq!(config.max_concurrent, 16);
    }

    #[test]
    fn extends_inherits_poll_interval() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        let parent_path = dir.path().join("shared.yaml");
        write_yaml(
            &parent_path,
            "project: shared\nnext_id: 999\npoll_interval: 120\n",
        );

        write_local_config(&mana_dir, &["shared.yaml"], "");

        let config = Config::load_with_extends(&mana_dir).unwrap();
        assert_eq!(config.poll_interval, 120);
    }

    #[test]
    fn extends_local_overrides_new_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        let parent_path = dir.path().join("shared.yaml");
        write_yaml(
            &parent_path,
            "project: shared\nnext_id: 999\nplan: \"parent-plan\"\nmax_concurrent: 16\npoll_interval: 120\n",
        );

        write_local_config(
            &mana_dir,
            &["shared.yaml"],
            "plan: \"local-plan\"\nmax_concurrent: 2\npoll_interval: 10\n",
        );

        let config = Config::load_with_extends(&mana_dir).unwrap();
        assert_eq!(config.plan, Some("local-plan".to_string()));
        assert_eq!(config.max_concurrent, 2);
        assert_eq!(config.poll_interval, 10);
    }

    #[test]
    fn new_fields_round_trip_through_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: None,
            plan: Some("plan {id}".to_string()),
            max_loops: 10,
            max_concurrent: 8,
            poll_interval: 60,
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
            memory_reserve_mb: 0,
            notify: None,
        };

        config.save(dir.path()).unwrap();
        let loaded = Config::load(dir.path()).unwrap();

        assert_eq!(config, loaded);
    }

    #[test]
    fn batch_verify_defaults_to_false() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert!(!loaded.batch_verify);
    }

    #[test]
    fn batch_verify_can_be_enabled() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\nbatch_verify: true\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert!(loaded.batch_verify);
    }

    #[test]
    fn batch_verify_not_serialized_when_false() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\n",
        )
        .unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert!(!loaded.batch_verify);

        loaded.save(dir.path()).unwrap();
        let contents = fs::read_to_string(dir.path().join("config.yaml")).unwrap();
        assert!(!contents.contains("batch_verify"));
    }
}
