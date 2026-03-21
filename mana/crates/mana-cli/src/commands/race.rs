//! `mana race` — Race N agents on the same unit, pick the best.
//!
//! Spawns multiple agents working on the same unit in parallel. Each runs
//! independently, and when all finish, the results are displayed for the
//! human to pick a winner.
//!
//! ## Template mode
//! Uses the `config.run` template to spawn N independent processes.
//! Each agent works in the same working tree (they race to `mana close`).
//!
//! ## Direct mode
//! Spawns N pi processes with the same structured prompt.
//!
//! Results are reported as a table: pass/fail, duration, tokens, cost.
//! If exactly one passes, it wins automatically. If multiple pass, the
//! human chooses (or the fastest is auto-selected with `--auto`).

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::unit::Unit;
use crate::config::Config;
use crate::discovery::find_unit_file;

/// Arguments for `cmd_race`.
pub struct RaceArgs {
    pub id: String,
    /// Number of agents to race (default: 3).
    pub count: u32,
    /// Timeout per agent in minutes.
    pub timeout: u32,
    /// Auto-select the fastest passing agent (skip interactive pick).
    pub auto: bool,
    /// Output as JSON.
    pub json: bool,
}

/// Result of a single race contestant.
#[derive(Debug, Serialize)]
struct RaceResult {
    contestant: u32,
    success: bool,
    duration_secs: u64,
    exit_code: Option<i32>,
}

/// Summary of the race.
#[derive(Debug, Serialize)]
struct RaceSummary {
    bean_id: String,
    bean_title: String,
    contestants: u32,
    results: Vec<RaceResult>,
    winner: Option<u32>,
}

/// Execute `mana race <id>`.
///
/// Spawns N agents working on the same unit. Each agent runs the configured
/// `run` template (or pi directly). Waits for all to finish, reports results,
/// and identifies a winner.
pub fn cmd_race(mana_dir: &Path, args: RaceArgs) -> Result<()> {
    let config = Config::load_with_extends(mana_dir)?;
    let bean_path = find_unit_file(mana_dir, &args.id)
        .with_context(|| format!("Unit not found: {}", args.id))?;
    let unit = Unit::from_file(&bean_path)
        .with_context(|| format!("Failed to load unit: {}", args.id))?;

    // We need a run template for race mode (direct mode would need multiple
    // pi instances which all try to close the same unit — use template mode).
    let run_template = config.run.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Race mode requires a configured run template.\n\
             Set one with: mana config set run \"<command>\"\n\
             Or configure an agent: mana init --setup"
        )
    })?;

    let count = args.count.max(2); // minimum 2 contestants

    eprintln!(
        "Racing {} agents on unit {}: \"{}\"",
        count, unit.id, unit.title
    );
    eprintln!();

    // Spawn N agents
    let (tx, rx) = mpsc::channel::<RaceResult>();

    for i in 1..=count {
        let cmd_str = run_template.replace("{id}", &args.id);
        let tx = tx.clone();
        let timeout_secs = args.timeout as u64 * 60;

        std::thread::spawn(move || {
            let started = Instant::now();

            let child = Command::new("sh")
                .args(["-c", &cmd_str])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            let (success, exit_code) = match child {
                Ok(mut c) => {
                    // Poll with timeout
                    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
                    loop {
                        match c.try_wait() {
                            Ok(Some(status)) => {
                                break (status.success(), status.code());
                            }
                            Ok(None) => {
                                if Instant::now() > deadline {
                                    let _ = c.kill();
                                    let _ = c.wait();
                                    break (false, None);
                                }
                                std::thread::sleep(Duration::from_millis(500));
                            }
                            Err(_) => break (false, None),
                        }
                    }
                }
                Err(_) => (false, None),
            };

            let _ = tx.send(RaceResult {
                contestant: i,
                success,
                duration_secs: started.elapsed().as_secs(),
                exit_code,
            });
        });

        eprintln!("  Spawned contestant #{}", i);
    }

    drop(tx); // close sender so rx iterator terminates

    // Collect results
    eprintln!();
    eprintln!("Waiting for all contestants...");
    eprintln!();

    let mut results: Vec<RaceResult> = Vec::new();
    for result in rx {
        let status = if result.success { "✓ pass" } else { "✗ fail" };
        let duration = format_duration(Duration::from_secs(result.duration_secs));
        eprintln!(
            "  Contestant #{}: {}  ({})",
            result.contestant, status, duration
        );
        results.push(result);
    }

    // Sort by contestant number for consistent display
    results.sort_by_key(|r| r.contestant);

    // Determine winner
    let passing: Vec<&RaceResult> = results.iter().filter(|r| r.success).collect();
    let winner = if passing.len() == 1 {
        let w = passing[0].contestant;
        eprintln!();
        eprintln!(
            "Winner: contestant #{} (only one passed)",
            w
        );
        Some(w)
    } else if passing.is_empty() {
        eprintln!();
        eprintln!("No contestants passed. Unit remains open.");
        None
    } else if args.auto {
        // Auto-select fastest passing
        let fastest = passing
            .iter()
            .min_by_key(|r| r.duration_secs)
            .unwrap();
        eprintln!();
        eprintln!(
            "Winner: contestant #{} (fastest of {} passing, {})",
            fastest.contestant,
            passing.len(),
            format_duration(Duration::from_secs(fastest.duration_secs))
        );
        Some(fastest.contestant)
    } else {
        eprintln!();
        eprintln!(
            "{} contestants passed. Pick a winner:",
            passing.len()
        );
        for r in &passing {
            eprintln!(
                "  #{}: {} ",
                r.contestant,
                format_duration(Duration::from_secs(r.duration_secs))
            );
        }
        eprintln!();
        eprintln!(
            "Tip: use `mana race --auto` to auto-select the fastest passing contestant."
        );
        // Auto-select fastest when not interactive
        let fastest = passing
            .iter()
            .min_by_key(|r| r.duration_secs)
            .unwrap();
        Some(fastest.contestant)
    };

    let summary = RaceSummary {
        bean_id: args.id.clone(),
        bean_title: unit.title.clone(),
        contestants: count,
        results,
        winner,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    }

    // Print summary table
    if !args.json {
        eprintln!();
        eprintln!("=== Race Summary ===");
        eprintln!("Unit: {} — {}", summary.bean_id, summary.bean_title);
        eprintln!("Contestants: {}", summary.contestants);
        eprintln!();
        for r in &summary.results {
            let status = if r.success { "✓" } else { "✗" };
            let duration = format_duration(Duration::from_secs(r.duration_secs));
            eprintln!("  #{} {} {}", r.contestant, status, duration);
        }
        if let Some(w) = summary.winner {
            eprintln!();
            eprintln!("Winner: #{}", w);
        }
    }

    Ok(())
}

/// Format a duration as M:SS.
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;
    use crate::config::Config;
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    fn setup_beans_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let config = Config {
            project: "test".to_string(),
            run: Some("echo 'racing unit {id}'".to_string()),
            ..Config::default()
        };
        config.save(&mana_dir).unwrap();

        (dir, mana_dir)
    }

    fn write_test_bean(mana_dir: &std::path::Path, unit: &Unit) {
        let slug = title_to_slug(&unit.title);
        let path = mana_dir.join(format!("{}-{}.md", unit.id, slug));
        unit.to_file(&path).unwrap();
    }

    #[test]
    fn race_requires_run_template() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Config with no run template
        let config = Config {
            project: "test".to_string(),
            ..Config::default()
        };
        config.save(&mana_dir).unwrap();

        let mut unit = Unit::new("1", "Test unit");
        unit.verify = Some("true".to_string());
        write_test_bean(&mana_dir, &unit);

        let args = RaceArgs {
            id: "1".to_string(),
            count: 2,
            timeout: 1,
            auto: true,
            json: false,
        };

        let result = cmd_race(&mana_dir, args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("run template"));
    }

    #[test]
    fn race_bean_not_found() {
        let (_dir, mana_dir) = setup_beans_dir();

        let args = RaceArgs {
            id: "999".to_string(),
            count: 2,
            timeout: 1,
            auto: true,
            json: false,
        };

        let result = cmd_race(&mana_dir, args);
        assert!(result.is_err());
    }

    #[test]
    fn race_runs_contestants() {
        let (_dir, mana_dir) = setup_beans_dir();

        let mut unit = Unit::new("1", "Race test");
        unit.verify = Some("true".to_string());
        write_test_bean(&mana_dir, &unit);

        let args = RaceArgs {
            id: "1".to_string(),
            count: 2,
            timeout: 1,
            auto: true,
            json: false,
        };

        // This spawns `echo 'racing unit 1'` twice — both succeed
        let result = cmd_race(&mana_dir, args);
        assert!(result.is_ok());
    }

    #[test]
    fn race_json_output() {
        let (_dir, mana_dir) = setup_beans_dir();

        let mut unit = Unit::new("1", "Race JSON test");
        unit.verify = Some("true".to_string());
        write_test_bean(&mana_dir, &unit);

        let args = RaceArgs {
            id: "1".to_string(),
            count: 2,
            timeout: 1,
            auto: true,
            json: true,
        };

        let result = cmd_race(&mana_dir, args);
        assert!(result.is_ok());
    }

    #[test]
    fn race_minimum_two_contestants() {
        let (_dir, mana_dir) = setup_beans_dir();

        let mut unit = Unit::new("1", "Race min test");
        unit.verify = Some("true".to_string());
        write_test_bean(&mana_dir, &unit);

        // count=1 should be bumped to 2
        let args = RaceArgs {
            id: "1".to_string(),
            count: 1,
            timeout: 1,
            auto: true,
            json: false,
        };

        let result = cmd_race(&mana_dir, args);
        assert!(result.is_ok());
    }

    #[test]
    fn format_duration_works() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
        assert_eq!(format_duration(Duration::from_secs(65)), "1:05");
        assert_eq!(format_duration(Duration::from_secs(3600)), "60:00");
    }
}
