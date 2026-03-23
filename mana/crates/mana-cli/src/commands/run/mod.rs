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

pub(super) mod memory;
mod plan;
mod ready_queue;
mod wave;

pub use plan::{DispatchPlan, SizedUnit};
pub use wave::Wave;

use std::fmt;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::commands::review::{cmd_review, ReviewArgs};
use crate::config::Config;
use crate::stream::{self, StreamEvent};
use crate::unit::Unit;

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
    /// Config-level model for run/implement (substituted into `{model}` in templates).
    pub run_model: Option<String>,
    /// When true, agents defer verify by exiting with AwaitingVerify status.
    /// The runner collects all deferred units and runs each unique verify command once.
    pub batch_verify: bool,
    /// Minimum available system memory (MB) to reserve. 0 = disabled.
    pub memory_reserve_mb: u64,
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
pub enum UnitAction {
    Implement,
}

impl fmt::Display for UnitAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnitAction::Implement => write!(f, "implement"),
        }
    }
}

/// Result of a completed agent.
#[derive(Debug)]
#[allow(dead_code)]
struct AgentResult {
    id: String,
    title: String,
    action: UnitAction,
    success: bool,
    duration: Duration,
    total_tokens: Option<u64>,
    total_cost: Option<f64>,
    error: Option<String>,
    tool_count: usize,
    turns: usize,
    failure_summary: Option<String>,
}

// ---------------------------------------------------------------------------
// Signal handling for clean agent shutdown
// ---------------------------------------------------------------------------

/// Global flag set by SIGINT/SIGTERM signal handlers to request clean shutdown.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// PIDs of running child agent processes, for cleanup on shutdown.
static CHILD_PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Returns true if a shutdown signal (SIGINT/SIGTERM) has been received.
fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Install signal handlers for SIGINT and SIGTERM.
///
/// Instead of immediately terminating, the handlers set a flag that's checked
/// in the execution loops. This allows clean shutdown: kill child agents,
/// release claims, and print a summary.
fn install_signal_handlers() {
    unsafe {
        libc::signal(
            libc::SIGINT,
            signal_handler as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            signal_handler as *const () as libc::sighandler_t,
        );
    }
}

extern "C" fn signal_handler(_sig: libc::c_int) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Register a child process PID for shutdown tracking.
fn register_child_pid(pid: u32) {
    if let Ok(mut pids) = CHILD_PIDS.lock() {
        pids.push(pid);
    }
}

/// Unregister a child process PID after it exits.
fn unregister_child_pid(pid: u32) {
    if let Ok(mut pids) = CHILD_PIDS.lock() {
        pids.retain(|&p| p != pid);
    }
}

/// Send SIGTERM to all tracked child processes for graceful shutdown.
fn kill_all_children() {
    if let Ok(pids) = CHILD_PIDS.lock() {
        for &pid in pids.iter() {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        }
    }
}

/// Send SIGKILL to all tracked child processes (forced shutdown).
fn force_kill_all_children() {
    if let Ok(pids) = CHILD_PIDS.lock() {
        for &pid in pids.iter() {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecisionWarning {
    id: String,
    title: String,
    decisions: Vec<String>,
}

fn collect_decision_warnings(
    mana_dir: &Path,
    units: &[SizedUnit],
    index: &crate::index::Index,
) -> Result<Vec<DecisionWarning>> {
    let mut warnings = Vec::new();

    for unit in units {
        let Some(entry) = index.units.iter().find(|entry| entry.id == unit.id) else {
            continue;
        };

        if !entry.has_decisions {
            continue;
        }

        let unit_path = crate::discovery::find_unit_file(mana_dir, &unit.id)?;
        let unit = Unit::from_file(&unit_path)?;
        if unit.decisions.is_empty() {
            continue;
        }

        warnings.push(DecisionWarning {
            id: unit.id,
            title: unit.title,
            decisions: unit.decisions,
        });
    }

    warnings.sort_by(|a, b| crate::util::natural_cmp(&a.id, &b.id));
    Ok(warnings)
}

fn format_decision_warning_message(warnings: &[DecisionWarning]) -> String {
    let mut message = String::new();

    if warnings.len() == 1 {
        let warning = &warnings[0];
        message.push_str(&format!(
            "⚠ Unit {} has {} unresolved decision{} — agent may make wrong choices:\n",
            warning.id,
            warning.decisions.len(),
            if warning.decisions.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
        for (idx, decision) in warning.decisions.iter().enumerate() {
            message.push_str(&format!("  {}: {}\n", idx, decision));
        }
        return message;
    }

    message.push_str(&format!(
        "⚠ {} units have unresolved decisions — agents may make wrong choices:\n",
        warnings.len()
    ));
    for warning in warnings {
        message.push_str(&format!(
            "Unit {}: {} ({} unresolved)\n",
            warning.id,
            warning.title,
            warning.decisions.len()
        ));
        for (idx, decision) in warning.decisions.iter().enumerate() {
            message.push_str(&format!("  {}: {}\n", idx, decision));
        }
    }

    message
}

fn confirm_dispatch_with_decisions(
    warnings: &[DecisionWarning],
    json_stream: bool,
) -> Result<bool> {
    if warnings.is_empty() {
        return Ok(true);
    }

    eprint!("{}", format_decision_warning_message(warnings));

    if json_stream || !std::io::stdin().is_terminal() {
        return Ok(true);
    }

    eprint!("Dispatch anyway? [y/N] ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

/// Execute the `mana run` command.
pub fn cmd_run(mana_dir: &Path, args: RunArgs) -> Result<()> {
    // Install signal handlers for clean shutdown on Ctrl+C / SIGTERM
    install_signal_handlers();

    // Determine spawn mode
    let config = Config::load_with_extends(mana_dir)?;
    let spawn_mode = determine_spawn_mode(&config);

    if spawn_mode == SpawnMode::Direct && !imp_available() && !pi_available() {
        anyhow::bail!(
            "No agent configured and neither `imp` nor `pi` found on PATH.\n\n\
             Either:\n  \
               1. Install imp (Rust): cargo install imp-cli\n  \
               2. Install pi (Node): npm i -g @mariozechner/pi-coding-agent\n  \
               3. Set a run template: mana config set run \"<command>\"\n\n\
             The command template uses {{id}} as a placeholder for the unit ID.\n\n\
             Examples:\n  \
               mana config set run \"imp run {{id}} && mana close {{id}}\"\n  \
               mana config set run \"pi @.mana/{{id}}-*.md 'implement and mana close {{id}}'\""
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

/// Check if `imp` is available on PATH.
fn imp_available() -> bool {
    Command::new("imp")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
    // Check for shutdown before starting execution
    if shutdown_requested() {
        if !args.json_stream {
            eprintln!("\nShutdown signal received, aborting.");
        }
        return Ok(());
    }

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

    let decision_warnings = collect_decision_warnings(mana_dir, &plan.all_units, &plan.index)?;
    if !confirm_dispatch_with_decisions(&decision_warnings, args.json_stream)? {
        if !args.json_stream {
            eprintln!("Dispatch cancelled.");
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

    let total_units: usize = plan.waves.iter().map(|w| w.units.len()).sum();
    let total_waves = plan.waves.len();
    let parent_id = args.id.as_deref().unwrap_or("all");

    if args.json_stream {
        let units_info: Vec<stream::UnitInfo> = plan
            .waves
            .iter()
            .enumerate()
            .flat_map(|(wave_idx, wave)| {
                wave.units.iter().map(move |b| stream::UnitInfo {
                    id: b.id.clone(),
                    title: b.title.clone(),
                    round: wave_idx + 1,
                })
            })
            .collect();
        stream::emit(&StreamEvent::RunStart {
            parent_id: parent_id.to_string(),
            total_units,
            total_rounds: total_waves,
            units: units_info,
        });
    }

    let run_cfg = RunConfig {
        max_jobs: args.jobs.min(config.max_concurrent) as usize,
        timeout_minutes: args.timeout,
        idle_timeout_minutes: args.idle_timeout,
        json_stream: args.json_stream,
        file_locking: config.file_locking,
        run_model: config.run_model.clone(),
        batch_verify: config.batch_verify,
        memory_reserve_mb: config.memory_reserve_mb,
    };
    let run_start = Instant::now();
    let total_done;
    let mut total_failed;
    let mut any_failed;
    let mut total_tokens: u64 = 0;
    let mut total_cost: f64 = 0.0;
    // Collect IDs of successfully closed units for --review post-processing
    let mut successful_ids: Vec<String> = Vec::new();

    match spawn_mode {
        SpawnMode::Direct => {
            if !args.json_stream {
                eprintln!("Dispatching {} unit(s)...", total_units);
            }

            // Ready-queue: start each unit as soon as its specific deps finish.
            // Progress (▸ start, ✓/✗ done) is printed in real-time by the queue.
            let (results, had_failure) = run_ready_queue_direct(
                mana_dir,
                &plan.all_units,
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
                        stream::emit(&StreamEvent::UnitDone {
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
                        stream::emit(&StreamEvent::UnitDone {
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

            // After all agents complete, run batch verification if enabled.
            // Each agent exits with AwaitingVerify status; the runner now resolves them.
            if run_cfg.batch_verify {
                match mana_core::ops::batch_verify::batch_verify(mana_dir) {
                    Ok(bv) => {
                        // Promote agent successes that passed verify into successful_ids
                        for id in &bv.passed {
                            if !successful_ids.contains(id) {
                                successful_ids.push(id.clone());
                            }
                        }
                        // Failures from batch verify count as failed units
                        total_failed += bv.failed.len() as u32;
                        if !bv.failed.is_empty() {
                            any_failed = true;
                        }

                        if args.json_stream {
                            stream::emit(&StreamEvent::BatchVerify {
                                commands_run: bv.commands_run,
                                passed: bv.passed.clone(),
                                failed: bv.failed.iter().map(|f| f.unit_id.clone()).collect(),
                            });
                        } else {
                            print_batch_verify_result(&bv);
                        }
                    }
                    Err(e) => {
                        eprintln!("Batch verify error: {}", e);
                        any_failed = true;
                    }
                }
            }
        }

        SpawnMode::Template { .. } => {
            // Template mode: wave-based execution (legacy)
            let mut done = 0u32;
            let mut failed = 0u32;
            let mut had_failure = false;

            for (wave_idx, wave) in plan.waves.iter().enumerate() {
                // Check for shutdown signal between waves
                if shutdown_requested() {
                    if !args.json_stream {
                        eprintln!("\nShutdown signal received, stopping.");
                    }
                    had_failure = true;
                    break;
                }

                if args.json_stream {
                    stream::emit(&StreamEvent::RoundStart {
                        round: wave_idx + 1,
                        total_rounds: total_waves,
                        unit_count: wave.units.len(),
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
                            stream::emit(&StreamEvent::UnitDone {
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
                            eprintln!("  ✓ {}  {}  {}", result.id, result.title, duration);
                        }
                        done += 1;
                        wave_success += 1;
                        successful_ids.push(result.id.clone());
                    } else {
                        if args.json_stream {
                            stream::emit(&StreamEvent::UnitDone {
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
        // Check for shutdown signal between loop iterations
        if shutdown_requested() {
            if !args.json_stream {
                eprintln!("\nShutdown signal received, stopping.");
            }
            return Ok(());
        }

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

/// Print a human-readable summary of a batch verify run.
///
/// Example output:
///   Batch verify: 2 commands, 3/4 units passed
///     ✓ cargo check -p mana-cli  (units: 1.1, 1.2, 1.3)
///     ✗ cargo test -p mana-core  (unit: 1.4) — exit code 1
fn print_batch_verify_result(result: &mana_core::ops::batch_verify::BatchVerifyResult) {
    let total = result.passed.len() + result.failed.len();
    eprintln!(
        "\nBatch verify: {} command{}, {}/{} unit{} passed",
        result.commands_run,
        if result.commands_run == 1 { "" } else { "s" },
        result.passed.len(),
        total,
        if total == 1 { "" } else { "s" },
    );

    if !result.passed.is_empty() {
        eprintln!(
            "  ✓ {} unit{} passed",
            result.passed.len(),
            if result.passed.len() == 1 { "" } else { "s" }
        );
    }

    // Group failures by verify command for compact display.
    let mut by_cmd: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for failure in &result.failed {
        by_cmd
            .entry(&failure.verify_command)
            .or_default()
            .push(&failure.unit_id);
    }

    // Sort for deterministic output.
    let mut cmd_entries: Vec<(&str, Vec<&str>)> = by_cmd.into_iter().collect();
    cmd_entries.sort_by_key(|(cmd, _)| *cmd);

    for (cmd, ids) in cmd_entries {
        let ids_str = ids.join(", ");
        let unit_word = if ids.len() == 1 { "unit" } else { "units" };
        // Find exit code for this command from the first matching failure
        let exit_info = result
            .failed
            .iter()
            .find(|f| f.verify_command == cmd)
            .map(|f| {
                if f.timed_out {
                    " — timed out".to_string()
                } else if let Some(code) = f.exit_code {
                    format!(" — exit code {}", code)
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        eprintln!("  ✗ {}  ({}: {}){}", cmd, unit_word, ids_str, exit_info);
    }
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

    fn make_mana_dir() -> (TempDir, std::path::PathBuf) {
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
        let (_dir, mana_dir) = make_mana_dir();
        write_config(&mana_dir, None);

        let args = default_args();

        let result = cmd_run(&mana_dir, args);
        // With no template and no pi on PATH, should error
        // (The exact error depends on whether pi is installed)
        // In CI/test without pi, it should bail
        if !pi_available() && !imp_available() {
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
        let (_dir, mana_dir) = make_mana_dir();
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
        let (_dir, mana_dir) = make_mana_dir();
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
        let mode = determine_spawn_mode(&config);
        assert_eq!(mode, SpawnMode::Direct);
    }

    #[test]
    fn agent_result_tracks_tokens_and_cost() {
        let result = AgentResult {
            id: "1".to_string(),
            title: "Test".to_string(),
            action: UnitAction::Implement,
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

    #[test]
    fn collect_decision_warnings_only_returns_dispatch_units_with_decisions() {
        let (_dir, mana_dir) = make_mana_dir();
        write_config(&mana_dir, Some("echo {id}"));

        let mut unit1 = crate::unit::Unit::new("1", "Has decisions");
        unit1.verify = Some("echo ok".to_string());
        unit1.decisions = vec!["JWT or session cookies?".to_string()];
        unit1.to_file(mana_dir.join("1-has-decisions.md")).unwrap();

        let mut unit2 = crate::unit::Unit::new("2", "No decisions");
        unit2.verify = Some("echo ok".to_string());
        unit2.to_file(mana_dir.join("2-no-decisions.md")).unwrap();

        let index = crate::index::Index::build(&mana_dir).unwrap();
        let units = vec![
            SizedUnit {
                id: "1".to_string(),
                title: "Has decisions".to_string(),
                action: UnitAction::Implement,
                priority: 2,
                dependencies: Vec::new(),
                parent: None,
                produces: Vec::new(),
                requires: Vec::new(),
                paths: Vec::new(),
                model: None,
            },
            SizedUnit {
                id: "2".to_string(),
                title: "No decisions".to_string(),
                action: UnitAction::Implement,
                priority: 2,
                dependencies: Vec::new(),
                parent: None,
                produces: Vec::new(),
                requires: Vec::new(),
                paths: Vec::new(),
                model: None,
            },
        ];

        let warnings = collect_decision_warnings(&mana_dir, &units, &index).unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].id, "1");
        assert_eq!(warnings[0].decisions, vec!["JWT or session cookies?"]);
    }

    #[test]
    fn format_decision_warning_message_matches_single_unit_prompt() {
        let message = format_decision_warning_message(&[DecisionWarning {
            id: "42".to_string(),
            title: "Implement auth".to_string(),
            decisions: vec![
                "JWT or session cookies?".to_string(),
                "Which JWT library?".to_string(),
            ],
        }]);

        assert!(message.contains("⚠ Unit 42 has 2 unresolved decisions"));
        assert!(message.contains("0: JWT or session cookies?"));
        assert!(message.contains("1: Which JWT library?"));
    }

    #[test]
    fn signal_flag_defaults_to_false() {
        SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
        assert!(!shutdown_requested());
    }

    #[test]
    fn signal_flag_can_be_toggled() {
        SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        assert!(shutdown_requested());
        // Reset for other tests
        SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
        assert!(!shutdown_requested());
    }

    #[test]
    fn child_pid_tracking() {
        // Clear any existing PIDs
        if let Ok(mut pids) = CHILD_PIDS.lock() {
            pids.clear();
        }

        register_child_pid(1234);
        register_child_pid(5678);

        let count = CHILD_PIDS.lock().unwrap().len();
        assert_eq!(count, 2);

        unregister_child_pid(1234);
        let count = CHILD_PIDS.lock().unwrap().len();
        assert_eq!(count, 1);

        // Unregister non-existent PID is a no-op
        unregister_child_pid(9999);
        let count = CHILD_PIDS.lock().unwrap().len();
        assert_eq!(count, 1);

        unregister_child_pid(5678);
        let count = CHILD_PIDS.lock().unwrap().len();
        assert_eq!(count, 0);
    }
}
