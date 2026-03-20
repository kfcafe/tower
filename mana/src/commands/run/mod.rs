//! `mana run` — Dispatch ready units to agents.
//!
//! Finds ready units, groups them into waves by dependency order,
//! and spawns agents for each wave.
//!
//! Modes:
//! - `mana run` — one-shot: dispatch all ready units, then exit
//! - `mana run 5.1` — dispatch a single unit (or its ready children if parent)
//! - `mana run --dry-run` — show plan without spawning
//! - `mana run --loop` — keep running until no ready units remain
//! - `mana run --json-stream` — emit JSON stream events to stdout
//!
//! Spawning modes:
//! - **Template mode** (backward compat): If `config.run` is set, spawn via `sh -c <template>`.
//! - **Direct mode**: If no template is configured but `pi` is on PATH, spawn pi directly
//!   with `--mode json --print --no-session`, monitoring with timeouts and parsing events.

mod plan;
mod ready_queue;
mod wave;

pub use plan::{DispatchPlan, SizedBean};
pub use wave::Wave;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::commands::review::{cmd_review, ReviewArgs};
use crate::config::Config;
use crate::stream::{self, StreamEvent};

use plan::{plan_dispatch, print_plan, print_plan_json};
use ready_queue::run_ready_queue_direct;
use wave::run_wave;

/// Shared config passed to wave/ready-queue runners.
pub(super) struct RunConfig {
    pub max_jobs: usize,
    pub timeout_minutes: u32,
    pub idle_timeout_minutes: u32,
    pub json_stream: bool,
    pub file_locking: bool,
}

/// Arguments for cmd_run, matching the CLI definition.
pub struct RunArgs {
    pub id: Option<String>,
    pub jobs: u32,
    pub dry_run: bool,
    pub loop_mode: bool,
    pub auto_plan: bool,
    pub keep_going: bool,
    pub timeout: u32,
    pub idle_timeout: u32,
    pub json_stream: bool,
    /// If true, run adversarial review after each successful unit close.
    pub review: bool,
}

/// What action to take for a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeanAction {
    Implement,
}

impl fmt::Display for BeanAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BeanAction::Implement => write!(f, "implement"),
        }
    }
}

/// Result of a completed agent.
#[derive(Debug)]
#[allow(dead_code)]
struct AgentResult {
    id: String,
    title: String,
    action: BeanAction,
    success: bool,
    duration: Duration,
    total_tokens: Option<u64>,
    total_cost: Option<f64>,
    error: Option<String>,
    tool_count: usize,
    turns: usize,
    failure_summary: Option<String>,
}

/// Which spawning mode to use.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SpawnMode {
    /// Use shell template from config (backward compat).
    Template {
        run_template: String,
        plan_template: Option<String>,
    },
    /// Spawn pi directly with JSON output and monitoring.
    Direct,
}

/// Execute the `mana run` command.
pub fn cmd_run(mana_dir: &Path, args: RunArgs) -> Result<()> {
    // Determine spawn mode
    let config = Config::load_with_extends(mana_dir)?;
    let spawn_mode = determine_spawn_mode(&config);

    if spawn_mode == SpawnMode::Direct && !pi_available() {
        anyhow::bail!(
            "No agent configured and `pi` not found on PATH.\n\n\
             Either:\n  \
               1. Install pi: npm i -g @anthropic/pi\n  \
               2. Set a run template: mana config set run \"<command>\"\n\n\
             The command template uses {{id}} as a placeholder for the unit ID.\n\n\
             Examples:\n  \
               mana config set run \"pi @.mana/{{id}}-*.md 'implement and mana close {{id}}'\"\n  \
               mana config set run \"claude -p 'implement unit {{id}} and run mana close {{id}}'\""
        );
    }

    if let SpawnMode::Template {
        ref run_template, ..
    } = spawn_mode
    {
        // Validate template exists (kept for backward compat error message)
        let _ = run_template;
    }

    if args.loop_mode {
        run_loop(mana_dir, &config, &args, &spawn_mode)
    } else {
        run_once(mana_dir, &config, &args, &spawn_mode)
    }
}

/// Determine the spawn mode based on config.
fn determine_spawn_mode(config: &Config) -> SpawnMode {
    if let Some(ref run) = config.run {
        SpawnMode::Template {
            run_template: run.clone(),
            plan_template: config.plan.clone(),
        }
    } else {
        SpawnMode::Direct
    }
}

/// Check if `pi` is available on PATH.
fn pi_available() -> bool {
    Command::new("pi")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Single dispatch pass: plan → print/execute → report.
fn run_once(
    mana_dir: &Path,
    config: &Config,
    args: &RunArgs,
    spawn_mode: &SpawnMode,
) -> Result<()> {
    let plan = plan_dispatch(
        mana_dir,
        config,
        args.id.as_deref(),
        args.auto_plan,
        args.dry_run,
    )?;

    if plan.waves.is_empty() && plan.skipped.is_empty() {
        if args.json_stream {
            stream::emit_error("No ready units");
        } else {
            eprintln!("No ready units. Use `mana status` to see what's going on.");
        }
        return Ok(());
    }

    if args.dry_run {
        if args.json_stream {
            print_plan_json(&plan, args.id.as_deref());
        } else {
            print_plan(&plan);
        }
        return Ok(());
    }

    // Report blocked units (oversized/unscoped)
    if !plan.skipped.is_empty() && !args.json_stream {
        eprintln!("{} unit(s) blocked:", plan.skipped.len());
        for bb in &plan.skipped {
            eprintln!("  ⚠ {}  {}  ({})", bb.id, bb.title, bb.reason);
        }
        eprintln!();
    }

    let total_beans: usize = plan.waves.iter().map(|w| w.units.len()).sum();
    let total_waves = plan.waves.len();
    let parent_id = args.id.as_deref().unwrap_or("all");

    if args.json_stream {
        let beans_info: Vec<stream::BeanInfo> = plan
            .waves
            .iter()
            .enumerate()
            .flat_map(|(wave_idx, wave)| {
                wave.units.iter().map(move |b| stream::BeanInfo {
                    id: b.id.clone(),
                    title: b.title.clone(),
                    round: wave_idx + 1,
                })
            })
            .collect();
        stream::emit(&StreamEvent::RunStart {
            parent_id: parent_id.to_string(),
            total_beans,
            total_rounds: total_waves,
            units: beans_info,
        });
    }

    let run_cfg = RunConfig {
        max_jobs: args.jobs.min(config.max_concurrent) as usize,
        timeout_minutes: args.timeout,
        idle_timeout_minutes: args.idle_timeout,
        json_stream: args.json_stream,
        file_locking: config.file_locking,
    };
    let run_start = Instant::now();
    let total_done;
    let total_failed;
    let any_failed;
    let mut total_tokens: u64 = 0;
    let mut total_cost: f64 = 0.0;
    // Collect IDs of successfully closed units for --review post-processing
    let mut successful_ids: Vec<String> = Vec::new();

    match spawn_mode {
        SpawnMode::Direct => {
            if !args.json_stream {
                eprintln!("Dispatching {} unit(s)...", total_beans);
            }

            // Ready-queue: start each unit as soon as its specific deps finish.
            // Progress (▸ start, ✓/✗ done) is printed in real-time by the queue.
            let (results, had_failure) = run_ready_queue_direct(
                mana_dir,
                &plan.all_beans,
                &plan.index,
                &run_cfg,
                args.keep_going,
            )?;

            let mut done = 0u32;
            let mut failed = 0u32;
            for result in &results {
                total_tokens += result.total_tokens.unwrap_or(0);
                total_cost += result.total_cost.unwrap_or(0.0);
                if result.success {
                    if args.json_stream {
                        stream::emit(&StreamEvent::BeanDone {
                            id: result.id.clone(),
                            success: true,
                            duration_secs: result.duration.as_secs(),
                            error: None,
                            total_tokens: result.total_tokens,
                            total_cost: result.total_cost,
                            tool_count: Some(result.tool_count),
                            turns: Some(result.turns),
                            failure_summary: None,
                        });
                    }
                    done += 1;
                    successful_ids.push(result.id.clone());
                } else {
                    if args.json_stream {
                        stream::emit(&StreamEvent::BeanDone {
                            id: result.id.clone(),
                            success: false,
                            duration_secs: result.duration.as_secs(),
                            error: result.error.clone(),
                            total_tokens: result.total_tokens,
                            total_cost: result.total_cost,
                            tool_count: Some(result.tool_count),
                            turns: Some(result.turns),
                            failure_summary: result.failure_summary.clone(),
                        });
                    }
                    failed += 1;
                }
            }
            total_done = done;
            total_failed = failed;
            any_failed = had_failure;
        }

        SpawnMode::Template { .. } => {
            // Template mode: wave-based execution (legacy)
            let mut done = 0u32;
            let mut failed = 0u32;
            let mut had_failure = false;

            for (wave_idx, wave) in plan.waves.iter().enumerate() {
                if args.json_stream {
                    stream::emit(&StreamEvent::RoundStart {
                        round: wave_idx + 1,
                        total_rounds: total_waves,
                        bean_count: wave.units.len(),
                    });
                } else {
                    eprintln!("Wave {}: {} unit(s)", wave_idx + 1, wave.units.len());
                }

                let results = run_wave(mana_dir, &wave.units, spawn_mode, &run_cfg, wave_idx + 1)?;

                let mut wave_success = 0usize;
                let mut wave_failed = 0usize;

                for result in &results {
                    let duration = format_duration(result.duration);
                    if result.success {
                        if args.json_stream {
                            stream::emit(&StreamEvent::BeanDone {
                                id: result.id.clone(),
                                success: true,
                                duration_secs: result.duration.as_secs(),
                                error: None,
                                total_tokens: result.total_tokens,
                                total_cost: result.total_cost,
                                tool_count: Some(result.tool_count),
                                turns: Some(result.turns),
                                failure_summary: None,
                            });
                        } else {
                            eprintln!(
                                "  ✓ {}  {}  {}",
                                result.id, result.title, duration
                            );
                        }
                        done += 1;
                        wave_success += 1;
                        successful_ids.push(result.id.clone());
                    } else {
                        if args.json_stream {
                            stream::emit(&StreamEvent::BeanDone {
                                id: result.id.clone(),
                                success: false,
                                duration_secs: result.duration.as_secs(),
                                error: result.error.clone(),
                                total_tokens: result.total_tokens,
                                total_cost: result.total_cost,
                                tool_count: Some(result.tool_count),
                                turns: Some(result.turns),
                                failure_summary: result.failure_summary.clone(),
                            });
                        } else {
                            let err = result.error.as_deref().unwrap_or("failed");
                            eprintln!(
                                "  ✗ {}  {}  {} ({})",
                                result.id, result.title, duration, err
                            );
                        }
                        failed += 1;
                        wave_failed += 1;
                        had_failure = true;
                    }
                }

                if args.json_stream {
                    stream::emit(&StreamEvent::RoundEnd {
                        round: wave_idx + 1,
                        success_count: wave_success,
                        failed_count: wave_failed,
                    });
                }

                if had_failure && !args.keep_going {
                    break;
                }
            }

            total_done = done;
            total_failed = failed;
            any_failed = had_failure;
        }
    }

    // Trigger adversarial review for each successfully closed unit if --review is set.
    // Review runs synchronously after all units in this pass complete.
    if args.review && !successful_ids.is_empty() {
        for id in &successful_ids {
            if !args.json_stream {
                eprintln!("Review: checking {} ...", id);
            }
            if let Err(e) = cmd_review(
                mana_dir,
                ReviewArgs {
                    id: id.clone(),
                    model: None,
                    diff_only: false,
                },
            ) {
                eprintln!("Review: warning — review of {} failed: {}", id, e);
            }
        }
    }

    if args.json_stream {
        stream::emit(&StreamEvent::RunEnd {
            total_success: total_done as usize,
            total_failed: total_failed as usize,
            duration_secs: run_start.elapsed().as_secs(),
        });
    } else {
        let elapsed = format_duration(run_start.elapsed());
        let mut summary = format!(
            "\nDone: {} succeeded, {} failed, {} skipped  ({})",
            total_done,
            total_failed,
            plan.skipped.len(),
            elapsed,
        );
        if total_tokens > 0 || total_cost > 0.0 {
            let token_str = if total_tokens >= 1_000_000 {
                format!("{:.1}M tokens", total_tokens as f64 / 1_000_000.0)
            } else if total_tokens >= 1_000 {
                format!("{}k tokens", total_tokens / 1_000)
            } else {
                format!("{} tokens", total_tokens)
            };
            summary.push_str(&format!("  [{}, ${:.2}]", token_str, total_cost));
        }
        eprintln!("{}", summary);
    }

    if any_failed && !args.keep_going {
        anyhow::bail!("Some agents failed");
    }

    Ok(())
}

/// Loop mode: keep dispatching until no ready units remain.
fn run_loop(
    mana_dir: &Path,
    config: &Config,
    args: &RunArgs,
    _spawn_mode: &SpawnMode,
) -> Result<()> {
    let max_loops = if config.max_loops == 0 {
        u32::MAX
    } else {
        config.max_loops
    };

    for iteration in 0..max_loops {
        if iteration > 0 && !args.json_stream {
            eprintln!("\n--- Loop iteration {} ---\n", iteration + 1);
        }

        let plan = plan_dispatch(mana_dir, config, args.id.as_deref(), args.auto_plan, false)?;

        if plan.waves.is_empty() {
            if !args.json_stream {
                if iteration == 0 {
                    eprintln!("No ready units. Use `mana status` to see what's going on.");
                } else {
                    eprintln!("No more ready units. Stopping.");
                }
            }
            return Ok(());
        }

        // Run one pass (non-loop, non-dry-run)
        let inner_args = RunArgs {
            id: args.id.clone(),
            jobs: args.jobs,
            dry_run: false,
            loop_mode: false,
            auto_plan: args.auto_plan,
            keep_going: args.keep_going,
            timeout: args.timeout,
            idle_timeout: args.idle_timeout,
            json_stream: args.json_stream,
            review: args.review,
        };

        // Reload config each iteration (agents may have changed units)
        let config = Config::load_with_extends(mana_dir)?;
        let spawn_mode = determine_spawn_mode(&config);
        match run_once(mana_dir, &config, &inner_args, &spawn_mode) {
            Ok(()) => {}
            Err(e) => {
                if args.keep_going {
                    eprintln!("Warning: {}", e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    eprintln!("Reached max_loops ({}). Stopping.", max_loops);
    Ok(())
}

/// Format a duration as M:SS.
pub(super) fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Find the unit file path. Public wrapper for use in other commands.
pub fn find_unit_file(mana_dir: &Path, id: &str) -> Result<PathBuf> {
    crate::discovery::find_unit_file(mana_dir, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_beans_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    fn write_config(mana_dir: &std::path::Path, run: Option<&str>) {
        let run_line = match run {
            Some(r) => format!("run: \"{}\"\n", r),
            None => String::new(),
        };
        fs::write(
            mana_dir.join("config.yaml"),
            format!("project: test\nnext_id: 1\n{}", run_line),
        )
        .unwrap();
    }

    fn default_args() -> RunArgs {
        RunArgs {
            id: None,
            jobs: 4,
            dry_run: false,
            loop_mode: false,
            auto_plan: false,
            keep_going: false,
            timeout: 30,
            idle_timeout: 5,
            json_stream: false,
            review: false,
        }
    }

    #[test]
    fn cmd_run_errors_when_no_run_template_and_no_pi() {
        let (_dir, mana_dir) = make_beans_dir();
        write_config(&mana_dir, None);

        let args = default_args();

        let result = cmd_run(&mana_dir, args);
        // With no template and no pi on PATH, should error
        // (The exact error depends on whether pi is installed)
        // In CI/test without pi, it should bail
        if !pi_available() {
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("No agent configured") || err.contains("not found"),
                "Error should mention missing agent: {}",
                err
            );
        }
    }

    #[test]
    fn dry_run_does_not_spawn() {
        let (_dir, mana_dir) = make_beans_dir();
        write_config(&mana_dir, Some("echo {id}"));

        // Create a ready unit
        let mut unit = crate::unit::Unit::new("1", "Test unit");
        unit.verify = Some("echo ok".to_string());
        unit.to_file(mana_dir.join("1-test.md")).unwrap();

        let args = RunArgs {
            dry_run: true,
            ..default_args()
        };

        // dry_run should succeed without spawning any processes
        let result = cmd_run(&mana_dir, args);
        assert!(result.is_ok());
    }

    #[test]
    fn dry_run_with_json_stream() {
        let (_dir, mana_dir) = make_beans_dir();
        write_config(&mana_dir, Some("echo {id}"));

        let mut unit = crate::unit::Unit::new("1", "Test unit");
        unit.verify = Some("echo ok".to_string());
        unit.to_file(mana_dir.join("1-test.md")).unwrap();

        let args = RunArgs {
            dry_run: true,
            json_stream: true,
            ..default_args()
        };

        // Should succeed and emit JSON events (captured to stdout)
        let result = cmd_run(&mana_dir, args);
        assert!(result.is_ok());
    }

    #[test]
    fn format_duration_formats_correctly() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
        assert_eq!(format_duration(Duration::from_secs(32)), "0:32");
        assert_eq!(format_duration(Duration::from_secs(62)), "1:02");
        assert_eq!(format_duration(Duration::from_secs(600)), "10:00");
    }

    #[test]
    fn determine_spawn_mode_template_when_run_set() {
        let config = Config {
            project: "test".to_string(),
            next_id: 1,
            auto_close_parent: true,
            run: Some("echo {id}".to_string()),
            plan: Some("plan {id}".to_string()),
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
        let mode = determine_spawn_mode(&config);
        assert_eq!(
            mode,
            SpawnMode::Template {
                run_template: "echo {id}".to_string(),
                plan_template: Some("plan {id}".to_string()),
            }
        );
    }

    #[test]
    fn determine_spawn_mode_direct_when_no_run() {
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
        };
        let mode = determine_spawn_mode(&config);
        assert_eq!(mode, SpawnMode::Direct);
    }

    #[test]
    fn agent_result_tracks_tokens_and_cost() {
        let result = AgentResult {
            id: "1".to_string(),
            title: "Test".to_string(),
            action: BeanAction::Implement,
            success: true,
            duration: Duration::from_secs(10),
            total_tokens: Some(5000),
            total_cost: Some(0.03),
            error: None,
            tool_count: 5,
            turns: 2,
            failure_summary: None,
        };
        assert_eq!(result.total_tokens, Some(5000));
        assert_eq!(result.total_cost, Some(0.03));
    }
}
