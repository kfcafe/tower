use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;

use crate::config::Config;
use crate::discovery::{archive_path_for_bean, find_archived_unit, find_unit_file};
use crate::graph;
use crate::hooks::{
    current_git_branch, execute_config_hook, execute_hook, is_trusted, HookEvent, HookVars,
};
use crate::index::{ArchiveIndex, Index, IndexEntry};
use crate::ops::verify::run_verify_command;
use crate::unit::{AttemptOutcome, OnCloseAction, OnFailAction, RunRecord, RunResult, Status, Unit};
use crate::util::title_to_slug;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// What action was taken by `process_on_fail`.
#[derive(Debug)]
pub enum OnFailActionTaken {
    /// Claim released for retry (attempt N / max M).
    Retry {
        attempt: u32,
        max: u32,
        delay_secs: Option<u64>,
    },
    /// Max retries exhausted — claim kept.
    RetryExhausted { max: u32 },
    /// Priority escalated and/or message appended.
    Escalated,
    /// No on_fail configured.
    None,
}

/// Result of a circuit breaker check.
#[derive(Debug)]
pub struct CircuitBreakerStatus {
    pub tripped: bool,
    pub subtree_total: u32,
    pub max_loops: u32,
}

/// Metadata about a verify failure, used by `record_failure`.
#[derive(Debug)]
pub struct VerifyFailure {
    pub exit_code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
    pub duration_secs: f64,
    pub started_at: chrono::DateTime<Utc>,
    pub finished_at: chrono::DateTime<Utc>,
    pub agent: Option<String>,
}

/// Options for the full `close` lifecycle.
pub struct CloseOpts {
    pub reason: Option<String>,
    pub force: bool,
}

/// Outcome of attempting to close a single unit.
pub enum CloseOutcome {
    /// The unit was closed and archived.
    Closed(CloseResult),
    /// The verify command failed.
    VerifyFailed(VerifyFailureResult),
    /// The pre-close hook rejected the close.
    RejectedByHook,
    /// Feature unit requires interactive TTY confirmation.
    FeatureRequiresHuman,
    /// Circuit breaker tripped — too many attempts across the subtree.
    CircuitBreakerTripped {
        unit_id: String,
        total_attempts: u32,
        max: u32,
    },
    /// Worktree merge had conflicts — unit stays open.
    MergeConflict,
}

/// Details of a successful close.
pub struct CloseResult {
    pub unit: Unit,
    pub archive_path: PathBuf,
    pub auto_closed_parents: Vec<String>,
    pub on_close_results: Vec<OnCloseActionResult>,
}

/// Result of one on_close action execution.
#[derive(Debug)]
pub enum OnCloseActionResult {
    /// A `run` command was executed.
    RanCommand { command: String, success: bool },
    /// A `notify` message was emitted.
    Notified { message: String },
    /// A `run` command was skipped (not trusted).
    Skipped { command: String },
}

/// Details of a verify failure during close.
pub struct VerifyFailureResult {
    pub unit: Unit,
    pub attempt_number: u32,
    pub exit_code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
    pub on_fail_action_taken: Option<OnFailActionTaken>,
    pub verify_command: String,
    pub timeout_secs: Option<u64>,
}

/// Maximum stdout size to capture as outputs (64 KB).
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// Core close lifecycle
// ---------------------------------------------------------------------------

/// Close a single unit — the full lifecycle.
///
/// Steps: pre-close hook → verify → worktree merge → feature gate → mark closed
/// → archive → post-close cascade → auto-close parents → rebuild index.
///
/// Does NOT handle TTY confirmation for feature units — if the unit is a feature,
/// returns `CloseOutcome::FeatureRequiresHuman` and the caller decides.
pub fn close(mana_dir: &Path, id: &str, opts: CloseOpts) -> Result<CloseOutcome> {
    let project_root = mana_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project root from units dir"))?;

    let config = Config::load(mana_dir).ok();

    let bean_path =
        find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;
    let mut unit =
        Unit::from_file(&bean_path).with_context(|| format!("Failed to load unit: {}", id))?;

    // 1. Pre-close hook
    if !run_pre_close_hook(&unit, project_root, opts.reason.as_deref()) {
        return Ok(CloseOutcome::RejectedByHook);
    }

    // 2. Verify (if applicable and not force)
    if let Some(verify_cmd) = unit.verify.clone() {
        if !verify_cmd.trim().is_empty() && !opts.force {
            let timeout_secs =
                unit.effective_verify_timeout(config.as_ref().and_then(|c| c.verify_timeout));

            let started_at = Utc::now();
            let verify_result = run_verify_command(&verify_cmd, project_root, timeout_secs)?;
            let finished_at = Utc::now();
            let duration_secs = (finished_at - started_at).num_milliseconds() as f64 / 1000.0;
            let agent = std::env::var("BEANS_AGENT").ok();

            if !verify_result.passed {
                // Build combined output — on timeout, synthesize a message
                let combined_output = if verify_result.timed_out {
                    format!("Verify timed out after {}s", timeout_secs.unwrap_or(0))
                } else {
                    let stdout = verify_result.stdout.trim();
                    let stderr = verify_result.stderr.trim();
                    let sep = if !stdout.is_empty() && !stderr.is_empty() {
                        "\n"
                    } else {
                        ""
                    };
                    format!("{}{}{}", stdout, sep, stderr)
                };

                // Record the failure
                let failure = VerifyFailure {
                    exit_code: verify_result.exit_code,
                    output: combined_output,
                    timed_out: verify_result.timed_out,
                    duration_secs,
                    started_at,
                    finished_at,
                    agent,
                };
                record_failure_on_unit(&mut unit, &failure);

                // Circuit breaker
                let root_id = find_root_parent(mana_dir, &unit)?;
                let config_max = config.as_ref().map(|c| c.max_loops).unwrap_or(10);
                let max_loops_limit =
                    resolve_max_loops(mana_dir, &unit, &root_id, config_max);

                if max_loops_limit > 0 {
                    // Save unit first so subtree count is accurate
                    unit.to_file(&bean_path)
                        .with_context(|| format!("Failed to save unit: {}", id))?;

                    let cb = check_circuit_breaker(mana_dir, &mut unit, &root_id, max_loops_limit)?;
                    if cb.tripped {
                        unit.to_file(&bean_path)
                            .with_context(|| format!("Failed to save unit: {}", id))?;

                        // Rebuild index
                        rebuild_index(mana_dir)?;

                        return Ok(CloseOutcome::CircuitBreakerTripped {
                            unit_id: id.to_string(),
                            total_attempts: cb.subtree_total,
                            max: cb.max_loops,
                        });
                    }
                }

                // Process on_fail action
                let action_taken = process_on_fail(&mut unit);

                unit.to_file(&bean_path)
                    .with_context(|| format!("Failed to save unit: {}", id))?;

                // Fire on_fail config hook
                run_on_fail_hook(&unit, project_root, config.as_ref(), &failure.output);

                // Rebuild index
                rebuild_index(mana_dir)?;

                return Ok(CloseOutcome::VerifyFailed(VerifyFailureResult {
                    unit,
                    attempt_number: 0, // filled below
                    exit_code: failure.exit_code,
                    output: failure.output,
                    timed_out: failure.timed_out,
                    on_fail_action_taken: Some(action_taken),
                    verify_command: verify_cmd,
                    timeout_secs,
                }));
            }

            // Record success in history
            unit.history.push(RunRecord {
                attempt: unit.attempts + 1,
                started_at,
                finished_at: Some(finished_at),
                duration_secs: Some(duration_secs),
                agent,
                result: RunResult::Pass,
                exit_code: verify_result.exit_code,
                tokens: None,
                cost: None,
                output_snippet: None,
            });

            // Capture stdout as unit outputs
            capture_verify_outputs(&mut unit, &verify_result.stdout);
        }
    }

    // 3. Worktree merge (after verify passes, before archiving)
    let worktree_info = detect_valid_worktree(project_root);
    if let Some(ref wt_info) = worktree_info {
        if !handle_worktree_merge(wt_info, &unit)? {
            return Ok(CloseOutcome::MergeConflict);
        }
    }

    // 4. Feature gate — delegate to caller
    if unit.feature {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            return Ok(CloseOutcome::FeatureRequiresHuman);
        }
        // TTY is available, but we don't do the prompt here — let the caller handle it
        // Actually, for backward compat, we return FeatureRequiresHuman and let CLI prompt
        return Ok(CloseOutcome::FeatureRequiresHuman);
    }

    // 5. Mark the unit closed
    let now = Utc::now();
    unit.status = Status::Closed;
    unit.closed_at = Some(now);
    unit.close_reason = opts.reason.clone();
    unit.updated_at = now;

    // Finalize the current attempt as success
    if let Some(attempt) = unit.attempt_log.last_mut() {
        if attempt.finished_at.is_none() {
            attempt.outcome = AttemptOutcome::Success;
            attempt.finished_at = Some(now);
            attempt.notes = opts.reason.clone();
        }
    }

    // Update last_verified for facts
    if unit.bean_type == "fact" {
        unit.last_verified = Some(now);
    }

    unit.to_file(&bean_path)
        .with_context(|| format!("Failed to save unit: {}", id))?;

    // 6. Archive
    let archive_path = archive_unit(mana_dir, &mut unit, &bean_path)?;

    // 7. Post-close cascade
    let on_close_results = run_post_close_actions(&unit, project_root, opts.reason.as_deref(), config.as_ref());

    // Auto-commit if configured (skip in worktree mode — it already commits)
    if worktree_info.is_none() {
        let auto_commit_enabled = config.as_ref().map(|c| c.auto_commit).unwrap_or(false);
        if auto_commit_enabled {
            auto_commit_on_close(project_root, id, &unit.title);
        }
    }

    // Clean up worktree after successful close
    if let Some(ref wt_info) = worktree_info {
        cleanup_worktree(wt_info);
    }

    // 8. Auto-close parents
    let auto_closed_parents = if mana_dir.exists() {
        if let Some(parent_id) = &unit.parent {
            let auto_close_enabled = config.as_ref().map(|c| c.auto_close_parent).unwrap_or(true);
            if auto_close_enabled {
                auto_close_parents(mana_dir, parent_id)?
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // Rebuild index
    rebuild_index(mana_dir)?;

    Ok(CloseOutcome::Closed(CloseResult {
        unit,
        archive_path,
        auto_closed_parents,
        on_close_results,
    }))
}

/// Mark a unit as explicitly failed. Stays open with claim released.
///
/// Records the failure in attempt_log for episodic memory and appends
/// a structured failure summary to notes.
pub fn close_failed(mana_dir: &Path, id: &str, reason: Option<String>) -> Result<Unit> {
    let now = Utc::now();

    let bean_path =
        find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;
    let mut unit =
        Unit::from_file(&bean_path).with_context(|| format!("Failed to load unit: {}", id))?;

    // Finalize the current attempt as failed
    if let Some(attempt) = unit.attempt_log.last_mut() {
        if attempt.finished_at.is_none() {
            attempt.outcome = AttemptOutcome::Failed;
            attempt.finished_at = Some(now);
            attempt.notes = reason.clone();
        }
    }

    // Release the claim (unit stays open for retry)
    unit.claimed_by = None;
    unit.claimed_at = None;
    unit.status = Status::Open;
    unit.updated_at = now;

    // Generate structured failure summary and append to notes
    {
        let attempt_num = unit.attempt_log.len() as u32;
        let duration_secs = unit
            .attempt_log
            .last()
            .and_then(|a| a.started_at)
            .map(|started| (now - started).num_seconds().max(0) as u64)
            .unwrap_or(0);

        let ctx = crate::failure::FailureContext {
            bean_id: id.to_string(),
            bean_title: unit.title.clone(),
            attempt: attempt_num.max(1),
            duration_secs,
            tool_count: 0,
            turns: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost: 0.0,
            error: reason,
            tool_log: vec![],
            verify_command: unit.verify.clone(),
        };
        let summary = crate::failure::build_failure_summary(&ctx);

        match &mut unit.notes {
            Some(notes) => {
                notes.push('\n');
                notes.push_str(&summary);
            }
            None => unit.notes = Some(summary),
        }
    }

    unit.to_file(&bean_path)
        .with_context(|| format!("Failed to save unit: {}", id))?;

    // Rebuild index
    rebuild_index(mana_dir)?;

    Ok(unit)
}

// ---------------------------------------------------------------------------
// Public composable functions
// ---------------------------------------------------------------------------

/// Check if all children of a parent unit are closed.
///
/// Checks both active and archived units. Returns true if the parent has no
/// children, or if all children have status=closed.
pub fn all_children_closed(mana_dir: &Path, parent_id: &str) -> Result<bool> {
    let index = Index::build(mana_dir)?;
    let archived = Index::collect_archived(mana_dir).unwrap_or_default();

    let mut all_beans = index.units;
    all_beans.extend(archived);

    let children: Vec<_> = all_beans
        .iter()
        .filter(|b| b.parent.as_deref() == Some(parent_id))
        .collect();

    if children.is_empty() {
        return Ok(true);
    }

    for child in children {
        if child.status != Status::Closed {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Auto-close parent chain when all children are done.
///
/// Recursively walks up the parent chain, closing and archiving each parent
/// whose children are all closed. Feature parents are skipped. Returns the
/// list of parent IDs that were auto-closed.
pub fn auto_close_parents(mana_dir: &Path, parent_id: &str) -> Result<Vec<String>> {
    let mut closed = Vec::new();
    auto_close_parent_recursive(mana_dir, parent_id, &mut closed)?;
    Ok(closed)
}

fn auto_close_parent_recursive(
    mana_dir: &Path,
    parent_id: &str,
    closed: &mut Vec<String>,
) -> Result<()> {
    if !all_children_closed(mana_dir, parent_id)? {
        return Ok(());
    }

    let bean_path = match find_unit_file(mana_dir, parent_id) {
        Ok(path) => path,
        Err(_) => return Ok(()), // Already archived
    };

    let mut unit = Unit::from_file(&bean_path)
        .with_context(|| format!("Failed to load parent unit: {}", parent_id))?;

    if unit.status == Status::Closed {
        return Ok(());
    }

    // Feature units are never auto-closed
    if unit.feature {
        return Ok(());
    }

    let now = Utc::now();
    unit.status = Status::Closed;
    unit.closed_at = Some(now);
    unit.close_reason = Some("Auto-closed: all children completed".to_string());
    unit.updated_at = now;

    unit.to_file(&bean_path)
        .with_context(|| format!("Failed to save parent unit: {}", parent_id))?;

    archive_unit(mana_dir, &mut unit, &bean_path)?;
    closed.push(parent_id.to_string());

    // Recurse to grandparent
    if let Some(grandparent_id) = &unit.parent {
        auto_close_parent_recursive(mana_dir, grandparent_id, closed)?;
    }

    Ok(())
}

/// Archive a closed unit to the dated archive directory.
///
/// Moves the unit file, marks `is_archived = true`, and updates the archive index.
/// Returns the archive path.
pub fn archive_unit(mana_dir: &Path, unit: &mut Unit, bean_path: &Path) -> Result<PathBuf> {
    let id = &unit.id;
    let slug = unit
        .slug
        .clone()
        .unwrap_or_else(|| title_to_slug(&unit.title));
    let ext = bean_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md");
    let today = chrono::Local::now().naive_local().date();
    let archive_path = archive_path_for_bean(mana_dir, id, &slug, ext, today);

    if let Some(parent) = archive_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create archive directories for unit {}", id))?;
    }

    std::fs::rename(bean_path, &archive_path)
        .with_context(|| format!("Failed to move unit {} to archive", id))?;

    unit.is_archived = true;
    unit.to_file(&archive_path)
        .with_context(|| format!("Failed to save archived unit: {}", id))?;

    // Append to archive index
    {
        let mut archive_index =
            ArchiveIndex::load(mana_dir).unwrap_or(ArchiveIndex { units: Vec::new() });
        archive_index.append(IndexEntry::from(&*unit));
        let _ = archive_index.save(mana_dir);
    }

    Ok(archive_path)
}

/// Record a failed verify attempt on a unit.
///
/// Increments attempts, appends failure details to notes, and pushes
/// a structured history entry. Does not save to disk — caller decides when to write.
pub fn record_failure(unit: &mut Unit, failure: &VerifyFailure) {
    record_failure_on_unit(unit, failure);
}

/// Process on_fail actions (retry release, escalate).
///
/// Mutates unit in-place (releases claim for retry, escalates priority).
/// Returns what action was taken.
pub fn process_on_fail(unit: &mut Unit) -> OnFailActionTaken {
    let on_fail = match &unit.on_fail {
        Some(action) => action.clone(),
        None => return OnFailActionTaken::None,
    };

    match on_fail {
        OnFailAction::Retry { max, delay_secs } => {
            let max_retries = max.unwrap_or(unit.max_attempts);
            if unit.attempts < max_retries {
                unit.claimed_by = None;
                unit.claimed_at = None;
                OnFailActionTaken::Retry {
                    attempt: unit.attempts,
                    max: max_retries,
                    delay_secs,
                }
            } else {
                OnFailActionTaken::RetryExhausted { max: max_retries }
            }
        }
        OnFailAction::Escalate { priority, message } => {
            if let Some(p) = priority {
                unit.priority = p;
            }
            if let Some(msg) = &message {
                let note = format!(
                    "\n## Escalated — {}\n{}",
                    Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
                    msg
                );
                match &mut unit.notes {
                    Some(notes) => notes.push_str(&note),
                    None => unit.notes = Some(note),
                }
            }
            if !unit.labels.contains(&"escalated".to_string()) {
                unit.labels.push("escalated".to_string());
            }
            OnFailActionTaken::Escalated
        }
    }
}

/// Check circuit breaker for a unit.
///
/// If subtree attempts exceed `max_loops`, trips the breaker: adds
/// "circuit-breaker" label and sets priority to P0. Unit is mutated
/// but NOT saved — caller decides when to write.
pub fn check_circuit_breaker(
    mana_dir: &Path,
    unit: &mut Unit,
    root_id: &str,
    max_loops: u32,
) -> Result<CircuitBreakerStatus> {
    if max_loops == 0 {
        return Ok(CircuitBreakerStatus {
            tripped: false,
            subtree_total: 0,
            max_loops: 0,
        });
    }

    let subtree_total = graph::count_subtree_attempts(mana_dir, root_id)?;
    if subtree_total >= max_loops {
        if !unit.labels.contains(&"circuit-breaker".to_string()) {
            unit.labels.push("circuit-breaker".to_string());
        }
        unit.priority = 0;
        Ok(CircuitBreakerStatus {
            tripped: true,
            subtree_total,
            max_loops,
        })
    } else {
        Ok(CircuitBreakerStatus {
            tripped: false,
            subtree_total,
            max_loops,
        })
    }
}

/// Walk up the parent chain to find the root ancestor of a unit.
///
/// Returns the ID of the topmost parent (the unit with no parent).
/// If the unit itself has no parent, returns its own ID.
pub fn find_root_parent(mana_dir: &Path, unit: &Unit) -> Result<String> {
    let mut current_id = match &unit.parent {
        None => return Ok(unit.id.clone()),
        Some(pid) => pid.clone(),
    };

    loop {
        let path = find_unit_file(mana_dir, &current_id)
            .or_else(|_| find_archived_unit(mana_dir, &current_id));

        match path {
            Ok(p) => {
                let b = Unit::from_file(&p)
                    .with_context(|| format!("Failed to load parent unit: {}", current_id))?;
                match b.parent {
                    Some(parent_id) => current_id = parent_id,
                    None => return Ok(current_id),
                }
            }
            Err(_) => return Ok(current_id),
        }
    }
}

/// Resolve the effective max_loops for a unit, considering root parent overrides.
pub fn resolve_max_loops(
    mana_dir: &Path,
    unit: &Unit,
    root_id: &str,
    config_max: u32,
) -> u32 {
    if root_id == unit.id {
        unit.effective_max_loops(config_max)
    } else {
        let root_path =
            find_unit_file(mana_dir, root_id).or_else(|_| find_archived_unit(mana_dir, root_id));
        match root_path {
            Ok(p) => Unit::from_file(&p)
                .map(|b| b.effective_max_loops(config_max))
                .unwrap_or(config_max),
            Err(_) => config_max,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Record a verify failure on a unit (internal).
fn record_failure_on_unit(unit: &mut Unit, failure: &VerifyFailure) {
    unit.attempts += 1;
    unit.updated_at = Utc::now();

    // Append failure to notes
    let failure_note = format_failure_note(unit.attempts, failure.exit_code, &failure.output);
    match &mut unit.notes {
        Some(notes) => notes.push_str(&failure_note),
        None => unit.notes = Some(failure_note),
    }

    // Record structured history entry
    let output_snippet = if failure.output.is_empty() {
        None
    } else {
        Some(truncate_output(&failure.output, 20))
    };
    unit.history.push(RunRecord {
        attempt: unit.attempts,
        started_at: failure.started_at,
        finished_at: Some(failure.finished_at),
        duration_secs: Some(failure.duration_secs),
        agent: failure.agent.clone(),
        result: if failure.timed_out {
            RunResult::Timeout
        } else {
            RunResult::Fail
        },
        exit_code: failure.exit_code,
        tokens: None,
        cost: None,
        output_snippet,
    });
}

/// Capture verify stdout as unit outputs.
fn capture_verify_outputs(unit: &mut Unit, stdout: &str) {
    let stdout = stdout.trim();
    if stdout.is_empty() {
        return;
    }

    if stdout.len() > MAX_OUTPUT_BYTES {
        let end = truncate_to_char_boundary(stdout, MAX_OUTPUT_BYTES);
        let truncated = &stdout[..end];
        unit.outputs = Some(serde_json::json!({
            "text": truncated,
            "truncated": true,
            "original_bytes": stdout.len()
        }));
    } else {
        match serde_json::from_str::<serde_json::Value>(stdout) {
            Ok(json) => {
                unit.outputs = Some(json);
            }
            Err(_) => {
                unit.outputs = Some(serde_json::json!({
                    "text": stdout
                }));
            }
        }
    }
}

/// Find the largest byte index <= `max_bytes` that falls on a UTF-8 char boundary.
pub fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Truncate output to first N + last N lines.
pub fn truncate_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();

    if lines.len() <= max_lines * 2 {
        return output.to_string();
    }

    let first = &lines[..max_lines];
    let last = &lines[lines.len() - max_lines..];

    format!(
        "{}\n\n... ({} lines omitted) ...\n\n{}",
        first.join("\n"),
        lines.len() - max_lines * 2,
        last.join("\n")
    )
}

/// Format a verify failure as a Markdown block to append to notes.
pub fn format_failure_note(attempt: u32, exit_code: Option<i32>, output: &str) -> String {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let truncated = truncate_output(output, 50);
    let exit_str = exit_code
        .map(|c| format!("Exit code: {}\n", c))
        .unwrap_or_default();

    format!(
        "\n## Attempt {} — {}\n{}\n```\n{}\n```\n",
        attempt, timestamp, exit_str, truncated
    )
}

// ---------------------------------------------------------------------------
// Hook helpers
// ---------------------------------------------------------------------------

/// Run pre-close hook. Returns true if hook passes or doesn't exist.
fn run_pre_close_hook(unit: &Unit, project_root: &Path, reason: Option<&str>) -> bool {
    let result = execute_hook(
        HookEvent::PreClose,
        unit,
        project_root,
        reason.map(|s| s.to_string()),
    );

    match result {
        Ok(hook_passed) => hook_passed,
        Err(e) => {
            eprintln!("Unit {} pre-close hook error: {}", unit.id, e);
            true
        }
    }
}

/// Run post-close hook + on_close actions + config hooks.
fn run_post_close_actions(
    unit: &Unit,
    project_root: &Path,
    reason: Option<&str>,
    config: Option<&Config>,
) -> Vec<OnCloseActionResult> {
    // Fire post-close hook
    match execute_hook(
        HookEvent::PostClose,
        unit,
        project_root,
        reason.map(|s| s.to_string()),
    ) {
        Ok(false) => {
            eprintln!(
                "Warning: post-close hook returned non-zero for unit {}",
                unit.id
            );
        }
        Err(e) => {
            eprintln!("Warning: post-close hook error for unit {}: {}", unit.id, e);
        }
        Ok(true) => {}
    }

    // Process on_close actions
    let mut results = Vec::new();
    for action in &unit.on_close {
        match action {
            OnCloseAction::Run { command } => {
                if !is_trusted(project_root) {
                    eprintln!(
                        "on_close: skipping `{}` (not trusted — run `mana trust` to enable)",
                        command
                    );
                    results.push(OnCloseActionResult::Skipped {
                        command: command.clone(),
                    });
                    continue;
                }
                eprintln!("on_close: running `{}`", command);
                let status = std::process::Command::new("sh")
                    .args(["-c", command.as_str()])
                    .current_dir(project_root)
                    .status();
                let success = match status {
                    Ok(s) if s.success() => true,
                    Ok(s) => {
                        eprintln!("on_close run command failed: {}", command);
                        let _ = s;
                        false
                    }
                    Err(e) => {
                        eprintln!("on_close run command error: {}", e);
                        false
                    }
                };
                results.push(OnCloseActionResult::RanCommand {
                    command: command.clone(),
                    success,
                });
            }
            OnCloseAction::Notify { message } => {
                println!("[unit {}] {}", unit.id, message);
                results.push(OnCloseActionResult::Notified {
                    message: message.clone(),
                });
            }
        }
    }

    // Fire on_close config hook
    if let Some(config) = config {
        if let Some(ref on_close_template) = config.on_close {
            let vars = HookVars {
                id: Some(unit.id.clone()),
                title: Some(unit.title.clone()),
                status: Some("closed".into()),
                branch: current_git_branch(),
                ..Default::default()
            };
            execute_config_hook("on_close", on_close_template, &vars, project_root);
        }
    }

    results
}

/// Fire the on_fail config hook.
fn run_on_fail_hook(
    unit: &Unit,
    project_root: &Path,
    config: Option<&Config>,
    output: &str,
) {
    if let Some(config) = config {
        if let Some(ref on_fail_template) = config.on_fail {
            let vars = HookVars {
                id: Some(unit.id.clone()),
                title: Some(unit.title.clone()),
                status: Some(format!("{}", unit.status)),
                attempt: Some(unit.attempts),
                output: Some(output.to_string()),
                branch: current_git_branch(),
                ..Default::default()
            };
            execute_config_hook("on_fail", on_fail_template, &vars, project_root);
        }
    }
}

// ---------------------------------------------------------------------------
// Worktree helpers
// ---------------------------------------------------------------------------

/// Detect and validate worktree context.
fn detect_valid_worktree(project_root: &Path) -> Option<crate::worktree::WorktreeInfo> {
    let info = crate::worktree::detect_worktree().unwrap_or(None)?;

    let canonical_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    if canonical_root.starts_with(&info.worktree_path) {
        Some(info)
    } else {
        None
    }
}

/// Commit worktree changes and merge to main. Returns false on conflict.
fn handle_worktree_merge(wt_info: &crate::worktree::WorktreeInfo, unit: &Unit) -> Result<bool> {
    crate::worktree::commit_worktree_changes(&format!("Close unit {}: {}", unit.id, unit.title))?;

    match crate::worktree::merge_to_main(wt_info, &unit.id)? {
        crate::worktree::MergeResult::Success | crate::worktree::MergeResult::NothingToCommit => {
            Ok(true)
        }
        crate::worktree::MergeResult::Conflict { files } => {
            eprintln!("Merge conflict in files: {:?}", files);
            eprintln!("Resolve conflicts and run `mana close {}` again", unit.id);
            Ok(false)
        }
    }
}

/// Clean up worktree after successful close.
fn cleanup_worktree(wt_info: &crate::worktree::WorktreeInfo) {
    if let Err(e) = crate::worktree::cleanup_worktree(wt_info) {
        eprintln!("Warning: failed to clean up worktree: {}", e);
    }
}

/// Auto-commit changes on close (non-worktree mode).
fn auto_commit_on_close(project_root: &Path, id: &str, title: &str) {
    let message = format!("Close unit {}: {}", id, title);

    let add_status = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status();

    match add_status {
        Ok(s) if !s.success() => {
            eprintln!(
                "Warning: git add -A failed (exit {})",
                s.code().unwrap_or(-1)
            );
            return;
        }
        Err(e) => {
            eprintln!("Warning: git add -A failed: {}", e);
            return;
        }
        _ => {}
    }

    let commit_result = std::process::Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    match commit_result {
        Ok(output) if output.status.success() => {
            eprintln!("auto_commit: {}", message);
        }
        Ok(output) if output.status.code() == Some(1) => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "Warning: git commit failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }
        Err(e) => {
            eprintln!("Warning: git commit failed: {}", e);
        }
    }
}

/// Rebuild the index.
fn rebuild_index(mana_dir: &Path) -> Result<()> {
    if mana_dir.exists() {
        let index = Index::build(mana_dir).with_context(|| "Failed to rebuild index")?;
        index
            .save(mana_dir)
            .with_context(|| "Failed to save index")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use tempfile::TempDir;

    fn setup_mana_dir() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    fn setup_mana_dir_with_config() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        Config {
            project: "test".to_string(),
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
        }
        .save(&mana_dir)
        .unwrap();

        (dir, mana_dir)
    }

    fn write_unit(mana_dir: &Path, unit: &Unit) {
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("{}-{}.md", unit.id, slug)))
            .unwrap();
    }

    // =====================================================================
    // close() tests
    // =====================================================================

    #[test]
    fn close_single_unit() {
        let (_dir, mana_dir) = setup_mana_dir();
        let unit = Unit::new("1", "Task");
        write_unit(&mana_dir, &unit);

        let result = close(
            &mana_dir,
            "1",
            CloseOpts {
                reason: None,
                force: false,
            },
        )
        .unwrap();

        match result {
            CloseOutcome::Closed(r) => {
                assert_eq!(r.unit.status, Status::Closed);
                assert!(r.unit.closed_at.is_some());
                assert!(r.unit.is_archived);
                assert!(r.archive_path.exists());
            }
            _ => panic!("Expected Closed outcome"),
        }
    }

    #[test]
    fn close_with_reason() {
        let (_dir, mana_dir) = setup_mana_dir();
        let unit = Unit::new("1", "Task");
        write_unit(&mana_dir, &unit);

        let result = close(
            &mana_dir,
            "1",
            CloseOpts {
                reason: Some("Fixed".to_string()),
                force: false,
            },
        )
        .unwrap();

        match result {
            CloseOutcome::Closed(r) => {
                assert_eq!(r.unit.close_reason, Some("Fixed".to_string()));
            }
            _ => panic!("Expected Closed outcome"),
        }
    }

    #[test]
    fn close_with_passing_verify() {
        let (_dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.verify = Some("true".to_string());
        write_unit(&mana_dir, &unit);

        let result = close(
            &mana_dir,
            "1",
            CloseOpts {
                reason: None,
                force: false,
            },
        )
        .unwrap();

        match result {
            CloseOutcome::Closed(r) => {
                assert_eq!(r.unit.status, Status::Closed);
                assert!(r.unit.is_archived);
                assert_eq!(r.unit.history.len(), 1);
                assert_eq!(r.unit.history[0].result, RunResult::Pass);
            }
            _ => panic!("Expected Closed outcome"),
        }
    }

    #[test]
    fn close_with_failing_verify() {
        let (_dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.verify = Some("false".to_string());
        write_unit(&mana_dir, &unit);

        let result = close(
            &mana_dir,
            "1",
            CloseOpts {
                reason: None,
                force: false,
            },
        )
        .unwrap();

        match result {
            CloseOutcome::VerifyFailed(r) => {
                assert_eq!(r.unit.status, Status::Open);
                assert_eq!(r.unit.attempts, 1);
            }
            _ => panic!("Expected VerifyFailed outcome"),
        }
    }

    #[test]
    fn close_force_skips_verify() {
        let (_dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.verify = Some("false".to_string());
        write_unit(&mana_dir, &unit);

        let result = close(
            &mana_dir,
            "1",
            CloseOpts {
                reason: None,
                force: true,
            },
        )
        .unwrap();

        match result {
            CloseOutcome::Closed(r) => {
                assert_eq!(r.unit.status, Status::Closed);
                assert!(r.unit.is_archived);
                assert_eq!(r.unit.attempts, 0);
            }
            _ => panic!("Expected Closed outcome"),
        }
    }

    #[test]
    fn close_feature_returns_requires_human() {
        let (_dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Feature");
        unit.feature = true;
        write_unit(&mana_dir, &unit);

        let result = close(
            &mana_dir,
            "1",
            CloseOpts {
                reason: None,
                force: false,
            },
        )
        .unwrap();

        assert!(matches!(result, CloseOutcome::FeatureRequiresHuman));
    }

    #[test]
    fn close_nonexistent_unit() {
        let (_dir, mana_dir) = setup_mana_dir();
        let result = close(
            &mana_dir,
            "99",
            CloseOpts {
                reason: None,
                force: false,
            },
        );
        assert!(result.is_err());
    }

    // =====================================================================
    // close_failed() tests
    // =====================================================================

    #[test]
    fn close_failed_marks_unit_as_failed() {
        let (_dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.status = Status::InProgress;
        unit.claimed_by = Some("agent-1".to_string());
        unit.attempt_log.push(crate::unit::AttemptRecord {
            num: 1,
            outcome: AttemptOutcome::Abandoned,
            notes: None,
            agent: Some("agent-1".to_string()),
            started_at: Some(Utc::now()),
            finished_at: None,
        });
        write_unit(&mana_dir, &unit);

        let result = close_failed(&mana_dir, "1", Some("blocked".to_string())).unwrap();
        assert_eq!(result.status, Status::Open);
        assert!(result.claimed_by.is_none());
        assert_eq!(result.attempt_log[0].outcome, AttemptOutcome::Failed);
        assert!(result.attempt_log[0].finished_at.is_some());
    }

    // =====================================================================
    // all_children_closed() tests
    // =====================================================================

    #[test]
    fn all_children_closed_when_no_children() {
        let (_dir, mana_dir) = setup_mana_dir();
        let unit = Unit::new("1", "Parent");
        write_unit(&mana_dir, &unit);

        assert!(all_children_closed(&mana_dir, "1").unwrap());
    }

    #[test]
    fn all_children_closed_when_some_open() {
        let (_dir, mana_dir) = setup_mana_dir();
        let parent = Unit::new("1", "Parent");
        write_unit(&mana_dir, &parent);

        let mut child1 = Unit::new("1.1", "Child 1");
        child1.parent = Some("1".to_string());
        child1.status = Status::Closed;
        write_unit(&mana_dir, &child1);

        let mut child2 = Unit::new("1.2", "Child 2");
        child2.parent = Some("1".to_string());
        write_unit(&mana_dir, &child2);

        assert!(!all_children_closed(&mana_dir, "1").unwrap());
    }

    // =====================================================================
    // auto_close_parents() tests
    // =====================================================================

    #[test]
    fn auto_close_parents_when_all_children_closed() {
        let (_dir, mana_dir) = setup_mana_dir_with_config();
        let parent = Unit::new("1", "Parent");
        write_unit(&mana_dir, &parent);

        let mut child = Unit::new("1.1", "Child");
        child.parent = Some("1".to_string());
        write_unit(&mana_dir, &child);

        // Close the child first
        let _ = close(
            &mana_dir,
            "1.1",
            CloseOpts {
                reason: None,
                force: false,
            },
        )
        .unwrap();

        // Parent should be auto-closed
        let parent_archived = find_archived_unit(&mana_dir, "1");
        assert!(parent_archived.is_ok());
        let p = Unit::from_file(parent_archived.unwrap()).unwrap();
        assert_eq!(p.status, Status::Closed);
        assert!(p.close_reason.as_ref().unwrap().contains("Auto-closed"));
    }

    #[test]
    fn auto_close_skips_feature_parents() {
        let (_dir, mana_dir) = setup_mana_dir_with_config();
        let mut parent = Unit::new("1", "Feature Parent");
        parent.feature = true;
        write_unit(&mana_dir, &parent);

        let mut child = Unit::new("1.1", "Child");
        child.parent = Some("1".to_string());
        write_unit(&mana_dir, &child);

        let _ = close(
            &mana_dir,
            "1.1",
            CloseOpts {
                reason: None,
                force: false,
            },
        )
        .unwrap();

        // Feature parent should still be open
        let parent_still_open = find_unit_file(&mana_dir, "1");
        assert!(parent_still_open.is_ok());
        let p = Unit::from_file(parent_still_open.unwrap()).unwrap();
        assert_eq!(p.status, Status::Open);
    }

    // =====================================================================
    // archive_unit() tests
    // =====================================================================

    #[test]
    fn archive_unit_moves_and_marks() {
        let (_dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.status = Status::Closed;
        let slug = title_to_slug(&unit.title);
        let bean_path = mana_dir.join(format!("1-{}.md", slug));
        unit.to_file(&bean_path).unwrap();

        let archive_path = archive_unit(&mana_dir, &mut unit, &bean_path).unwrap();
        assert!(archive_path.exists());
        assert!(!bean_path.exists());
        assert!(unit.is_archived);
    }

    // =====================================================================
    // record_failure() tests
    // =====================================================================

    #[test]
    fn record_failure_increments_attempts() {
        let mut unit = Unit::new("1", "Task");
        let failure = VerifyFailure {
            exit_code: Some(1),
            output: "error".to_string(),
            timed_out: false,
            duration_secs: 1.0,
            started_at: Utc::now(),
            finished_at: Utc::now(),
            agent: None,
        };
        record_failure(&mut unit, &failure);
        assert_eq!(unit.attempts, 1);
        assert_eq!(unit.history.len(), 1);
        assert_eq!(unit.history[0].result, RunResult::Fail);
    }

    #[test]
    fn record_failure_timeout() {
        let mut unit = Unit::new("1", "Task");
        let failure = VerifyFailure {
            exit_code: None,
            output: "timed out".to_string(),
            timed_out: true,
            duration_secs: 30.0,
            started_at: Utc::now(),
            finished_at: Utc::now(),
            agent: None,
        };
        record_failure(&mut unit, &failure);
        assert_eq!(unit.history[0].result, RunResult::Timeout);
    }

    // =====================================================================
    // process_on_fail() tests
    // =====================================================================

    #[test]
    fn process_on_fail_retry_releases_claim() {
        let mut unit = Unit::new("1", "Task");
        unit.on_fail = Some(OnFailAction::Retry {
            max: Some(5),
            delay_secs: None,
        });
        unit.attempts = 1;
        unit.claimed_by = Some("agent-1".to_string());
        unit.claimed_at = Some(Utc::now());

        let result = process_on_fail(&mut unit);
        assert!(matches!(result, OnFailActionTaken::Retry { .. }));
        assert!(unit.claimed_by.is_none());
    }

    #[test]
    fn process_on_fail_escalate_sets_priority() {
        let mut unit = Unit::new("1", "Task");
        unit.on_fail = Some(OnFailAction::Escalate {
            priority: Some(0),
            message: None,
        });
        unit.priority = 2;

        let result = process_on_fail(&mut unit);
        assert!(matches!(result, OnFailActionTaken::Escalated));
        assert_eq!(unit.priority, 0);
        assert!(unit.labels.contains(&"escalated".to_string()));
    }

    #[test]
    fn process_on_fail_none() {
        let mut unit = Unit::new("1", "Task");
        let result = process_on_fail(&mut unit);
        assert!(matches!(result, OnFailActionTaken::None));
    }

    // =====================================================================
    // check_circuit_breaker() tests
    // =====================================================================

    #[test]
    fn circuit_breaker_zero_disabled() {
        let (_dir, mana_dir) = setup_mana_dir();
        let mut unit = Unit::new("1", "Task");
        let result = check_circuit_breaker(&mana_dir, &mut unit, "1", 0).unwrap();
        assert!(!result.tripped);
    }

    // =====================================================================
    // Helper tests
    // =====================================================================

    #[test]
    fn truncate_to_char_boundary_ascii() {
        let s = "hello world";
        assert_eq!(truncate_to_char_boundary(s, 5), 5);
    }

    #[test]
    fn truncate_to_char_boundary_multibyte() {
        let s = "😀😁😂";
        assert_eq!(truncate_to_char_boundary(s, 5), 4);
    }

    #[test]
    fn truncate_output_short() {
        let output = "line1\nline2\nline3";
        let result = truncate_output(output, 50);
        assert_eq!(result, output);
    }

    #[test]
    fn format_failure_note_includes_exit_code() {
        let note = format_failure_note(1, Some(1), "error message");
        assert!(note.contains("## Attempt 1"));
        assert!(note.contains("Exit code: 1"));
        assert!(note.contains("error message"));
    }
}
