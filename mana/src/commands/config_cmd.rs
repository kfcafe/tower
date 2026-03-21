use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::{Config, GlobalConfig};

/// Get a configuration value by key
pub fn cmd_config_get(mana_dir: &Path, key: &str) -> Result<()> {
    let config = Config::load(mana_dir)?;

    let value = match key {
        "project" => config.project,
        "next_id" => config.next_id.to_string(),
        "auto_close_parent" => config.auto_close_parent.to_string(),
        "run" => config.run.unwrap_or_default(),
        "plan" => config.plan.unwrap_or_default(),
        "max_concurrent" => config.max_concurrent.to_string(),
        "poll_interval" => config.poll_interval.to_string(),
        "rules_file" => config.rules_file.unwrap_or_else(|| "RULES.md".to_string()),
        "auto_commit" => config.auto_commit.to_string(),
        "research" => config.research.unwrap_or_default(),
        "on_close" => config.on_close.unwrap_or_default(),
        "on_fail" => config.on_fail.unwrap_or_default(),
        "post_plan" => config.post_plan.unwrap_or_default(),
        "user" => {
            if let Some(user) = config.user {
                user
            } else if let Ok(global) = GlobalConfig::load() {
                global.user.unwrap_or_default()
            } else {
                String::new()
            }
        }
        "user.email" => {
            if let Some(email) = config.user_email {
                email
            } else if let Ok(global) = GlobalConfig::load() {
                global.user_email.unwrap_or_default()
            } else {
                String::new()
            }
        }
        _ => return Err(anyhow!("Unknown config key: {}", key)),
    };

    println!("{}", value);
    Ok(())
}

/// Set a configuration value by key
pub fn cmd_config_set(mana_dir: &Path, key: &str, value: &str) -> Result<()> {
    let mut config = Config::load(mana_dir)?;

    match key {
        "project" => {
            config.project = value.to_string();
        }
        "next_id" => {
            config.next_id = value
                .parse()
                .map_err(|_| anyhow!("Invalid value for next_id: {}", value))?;
        }
        "auto_close_parent" => {
            config.auto_close_parent = value.parse().map_err(|_| {
                anyhow!(
                    "Invalid value for auto_close_parent: {} (expected true/false)",
                    value
                )
            })?;
        }
        "run" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.run = None;
            } else {
                config.run = Some(value.to_string());
            }
        }
        "plan" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.plan = None;
            } else {
                config.plan = Some(value.to_string());
            }
        }
        "max_concurrent" => {
            config.max_concurrent = value.parse().map_err(|_| {
                anyhow!(
                    "Invalid value for max_concurrent: {} (expected positive integer)",
                    value
                )
            })?;
        }
        "poll_interval" => {
            config.poll_interval = value.parse().map_err(|_| {
                anyhow!(
                    "Invalid value for poll_interval: {} (expected positive integer)",
                    value
                )
            })?;
        }
        "rules_file" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.rules_file = None;
            } else {
                config.rules_file = Some(value.to_string());
            }
        }
        "auto_commit" => {
            config.auto_commit = value.parse().map_err(|_| {
                anyhow!(
                    "Invalid value for auto_commit: {} (expected true/false)",
                    value
                )
            })?;
        }
        "research" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.research = None;
            } else {
                config.research = Some(value.to_string());
            }
        }
        "on_close" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.on_close = None;
            } else {
                config.on_close = Some(value.to_string());
            }
        }
        "on_fail" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.on_fail = None;
            } else {
                config.on_fail = Some(value.to_string());
            }
        }
        "post_plan" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.post_plan = None;
            } else {
                config.post_plan = Some(value.to_string());
            }
        }
        "user" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.user = None;
            } else {
                config.user = Some(value.to_string());
            }
        }
        "user.email" => {
            if value.is_empty() || value == "none" || value == "unset" {
                config.user_email = None;
            } else {
                config.user_email = Some(value.to_string());
            }
        }
        _ => return Err(anyhow!("Unknown config key: {}", key)),
    }

    config.save(mana_dir)?;
    println!("Set {} = {}", key, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "project: test\nnext_id: 1\nauto_close_parent: true\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn get_unknown_key_returns_error() {
        let dir = setup_test_dir();
        let result = cmd_config_get(dir.path(), "unknown_key");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown config key"));
    }

    #[test]
    fn set_unknown_key_returns_error() {
        let dir = setup_test_dir();
        let result = cmd_config_set(dir.path(), "unknown_key", "value");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown config key"));
    }

    #[test]
    fn get_run_returns_empty_when_unset() {
        let dir = setup_test_dir();
        let result = cmd_config_get(dir.path(), "run");
        assert!(result.is_ok());
    }

    #[test]
    fn set_run_stores_command_template() {
        let dir = setup_test_dir();
        cmd_config_set(dir.path(), "run", "claude -p 'implement unit {id}'").unwrap();

        let config = Config::load(dir.path()).unwrap();
        assert_eq!(
            config.run,
            Some("claude -p 'implement unit {id}'".to_string())
        );
    }

    #[test]
    fn set_run_to_none_clears_it() {
        let dir = setup_test_dir();
        cmd_config_set(dir.path(), "run", "some command").unwrap();
        cmd_config_set(dir.path(), "run", "none").unwrap();

        let config = Config::load(dir.path()).unwrap();
        assert_eq!(config.run, None);
    }

    #[test]
    fn set_run_to_empty_clears_it() {
        let dir = setup_test_dir();
        cmd_config_set(dir.path(), "run", "some command").unwrap();
        cmd_config_set(dir.path(), "run", "").unwrap();

        let config = Config::load(dir.path()).unwrap();
        assert_eq!(config.run, None);
    }
}
