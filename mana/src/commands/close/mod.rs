mod archive;
mod failure;
mod hooks;
mod parent;
mod verify;
mod worktree_merge;

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;

use crate::unit::{Unit, RunRecord, RunResult, Status};
use crate::config::Config;
use crate::discovery::find_unit_file;
use crate::index::Index;

use verify::run_verify;

#[cfg(test)]
use std::fs;

/// Maximum stdout size to capture as outputs (64 KB).
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Find the largest byte index <= `max_bytes` that falls on a UTF-8 char boundary.
fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Close one or more units.
///
/// Sets status=closed, closed_at=now, and optionally close_reason.
/// If the unit has a verify command, it must pass before closing (unless force=true).
/// Calls pre-close hook before verify (can block close if hook fails).
/// Auto-closes parent units when all children are closed (if enabled in config).
/// Rebuilds the index.
pub fn cmd_close(
    mana_dir: &Path,
    ids: Vec<String>,
    reason: Option<String>,
    force: bool,
) -> Result<()> {
    if ids.is_empty() {
        return Err(anyhow!("At least one unit ID is required"));
    }

    let now = Utc::now();
    let mut any_closed = false;
    let mut rejected_beans = Vec::new();

    let project_root = mana_dir
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine project root from units dir"))?;

    let config = Config::load(mana_dir).ok();

    for id in &ids {
        let bean_path =
            find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;

        let mut unit =
            Unit::from_file(&bean_path).with_context(|| format!("Failed to load unit: {}", id))?;

        // 1. Pre-close hook
        if !hooks::run_pre_close(&unit, project_root, reason.as_deref()) {
            eprintln!("Unit {} rejected by pre-close hook", id);
            rejected_beans.push(id.clone());
            continue;
        }

        // 2. Verify (if applicable and not --force)
        if let Some(verify_cmd) = unit.verify.clone() {
            if verify_cmd.trim().is_empty() {
                eprintln!("Warning: unit {} has empty verify command, skipping", id);
            } else if force {
                println!("Skipping verify for unit {} (--force)", id);
            } else {
                let started_at = Utc::now();
                let timeout_secs =
                    unit.effective_verify_timeout(config.as_ref().and_then(|c| c.verify_timeout));
                let verify_result = run_verify(mana_dir, &verify_cmd, timeout_secs)?;
                let finished_at = Utc::now();
                let duration_secs = (finished_at - started_at).num_milliseconds() as f64 / 1000.0;
                let agent = std::env::var("BEANS_AGENT").ok();

                if !verify_result.success {
                    // Surface timeout prominently
                    if verify_result.timed_out {
                        let secs = timeout_secs.unwrap_or(0);
                        println!("Verify timed out after {}s for unit {}", secs, id);
                    }

                    // Record the failure
                    failure::record_failure(
                        &mut unit,
                        &failure::FailureRecord {
                            exit_code: verify_result.exit_code,
                            output: verify_result.output.clone(),
                            timed_out: verify_result.timed_out,
                            duration_secs,
                            started_at,
                            finished_at,
                            agent: agent.clone(),
                        },
                    );

                    // Circuit breaker
                    let root_id = parent::find_root_parent(mana_dir, &unit)?;
                    let config_max = config.as_ref().map(|c| c.max_loops).unwrap_or(10);
                    let max_loops_limit =
                        failure::resolve_max_loops(mana_dir, &unit, &root_id, config_max);

                    if max_loops_limit > 0 {
                        // Save unit first so subtree count is accurate
                        unit.to_file(&bean_path)
                            .with_context(|| format!("Failed to save unit: {}", id))?;

                        let cb = failure::check_circuit_breaker(
                            mana_dir,
                            &mut unit,
                            &root_id,
                            max_loops_limit,
                        )?;
                        if cb.tripped {
                            unit.to_file(&bean_path)
                                .with_context(|| format!("Failed to save unit: {}", id))?;
                            eprintln!(
                                "⚡ Circuit breaker tripped for unit {} \
                                 (subtree total {} >= max_loops {} across root {})",
                                id, cb.subtree_total, cb.max_loops, root_id
                            );
                            eprintln!(
                                "Unit {} escalated to P0 with 'circuit-breaker' label. \
                                 Manual intervention required.",
                                id
                            );
                            continue;
                        }
                    }

                    // Process on_fail action
                    let action_taken = failure::process_on_fail(&mut unit);
                    match action_taken {
                        failure::OnFailActionTaken::Retry {
                            attempt,
                            max,
                            delay_secs,
                        } => {
                            println!(
                                "on_fail: will retry (attempt {}/{})",
                                attempt, max
                            );
                            if let Some(delay) = delay_secs {
                                println!(
                                    "on_fail: retry delay {}s (enforced by orchestrator)",
                                    delay
                                );
                            }
                        }
                        failure::OnFailActionTaken::RetryExhausted { max } => {
                            println!("on_fail: max retries ({}) exhausted", max);
                        }
                        failure::OnFailActionTaken::Escalated => {
                            if let Some(crate::unit::OnFailAction::Escalate {
                                priority,
                                message,
                            }) = &unit.on_fail
                            {
                                if let Some(p) = priority {
                                    // priority was already updated by process_on_fail;
                                    // print with old priority approximated from context
                                    println!(
                                        "on_fail: escalated priority → P{}",
                                        p
                                    );
                                }
                                if let Some(msg) = message {
                                    println!("on_fail: {}", msg);
                                }
                            }
                        }
                        failure::OnFailActionTaken::None => {}
                    }

                    unit.to_file(&bean_path)
                        .with_context(|| format!("Failed to save unit: {}", id))?;

                    // Display detailed failure feedback
                    if verify_result.timed_out {
                        println!("✗ Verify timed out for unit {}", id);
                    } else {
                        println!("✗ Verify failed for unit {}", id);
                    }
                    println!();
                    println!("Command: {}", verify_cmd);
                    if verify_result.timed_out {
                        println!("Timed out after {}s", timeout_secs.unwrap_or(0));
                    } else if let Some(code) = verify_result.exit_code {
                        println!("Exit code: {}", code);
                    }
                    if !verify_result.output.is_empty() {
                        println!("Output:");
                        for line in verify_result.output.lines() {
                            println!("  {}", line);
                        }
                    }
                    println!();
                    println!("Attempt {}. Unit remains open.", unit.attempts);
                    println!("Tip: Run `mana verify {}` to test without closing.", id);
                    println!("Tip: Use `mana close {} --force` to skip verify.", id);

                    // Fire on_fail config hook
                    hooks::run_on_fail_hook(
                        &unit,
                        project_root,
                        config.as_ref(),
                        &verify_result.output,
                    );

                    continue;
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
                let stdout = &verify_result.stdout;
                if !stdout.is_empty() {
                    if stdout.len() > MAX_OUTPUT_BYTES {
                        let end = truncate_to_char_boundary(stdout, MAX_OUTPUT_BYTES);
                        let truncated = &stdout[..end];
                        eprintln!(
                            "Warning: verify stdout ({} bytes) exceeds 64KB, truncating",
                            stdout.len()
                        );
                        unit.outputs = Some(serde_json::json!({
                            "text": truncated,
                            "truncated": true,
                            "original_bytes": stdout.len()
                        }));
                    } else {
                        match serde_json::from_str::<serde_json::Value>(stdout.trim()) {
                            Ok(json) => {
                                unit.outputs = Some(json);
                            }
                            Err(_) => {
                                unit.outputs = Some(serde_json::json!({
                                    "text": stdout.trim()
                                }));
                            }
                        }
                    }
                }

                println!("Verify passed for unit {}", id);
            }
        }

        // 3. Worktree merge (after verify passes, before archiving)
        let worktree_info = worktree_merge::detect_valid_worktree(project_root);
        if let Some(ref wt_info) = worktree_info {
            if !worktree_merge::handle_merge(wt_info, &unit)? {
                return Ok(()); // Conflict — don't archive
            }
        }

        // 4. Feature gate
        if unit.feature {
            use std::io::IsTerminal;
            if !std::io::stdin().is_terminal() {
                println!(
                    "Feature \"{}\" requires human review to close.",
                    unit.title
                );
                continue;
            }
            eprintln!(
                "Feature: \"{}\" — mark as complete? [y/N] ",
                unit.title
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap_or(0);
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Skipped feature \"{}\"", unit.title);
                continue;
            }
        }

        // 5. Close the unit
        unit.status = Status::Closed;
        unit.closed_at = Some(now);
        unit.close_reason = reason.clone();
        unit.updated_at = now;

        // Finalize the current attempt as success
        if let Some(attempt) = unit.attempt_log.last_mut() {
            if attempt.finished_at.is_none() {
                attempt.outcome = crate::unit::AttemptOutcome::Success;
                attempt.finished_at = Some(now);
                attempt.notes = reason.clone();
            }
        }

        // Update last_verified for facts
        if unit.bean_type == "fact" {
            unit.last_verified = Some(now);
        }

        unit.to_file(&bean_path)
            .with_context(|| format!("Failed to save unit: {}", id))?;

        // 6. Archive
        archive::archive_bean(mana_dir, &mut unit, &bean_path)?;
        println!("Closed unit {}: {}", id, unit.title);
        any_closed = true;

        // 7. Post-close cascade
        hooks::run_post_close(&unit, project_root, reason.as_deref(), config.as_ref());

        // Auto-commit if configured (skip in worktree mode — it already commits)
        if worktree_info.is_none() {
            let auto_commit_enabled = config.as_ref().map(|c| c.auto_commit).unwrap_or(false);
            if auto_commit_enabled {
                worktree_merge::auto_commit_on_close(project_root, id, &unit.title);
            }
        }

        // Clean up worktree after successful close
        if let Some(ref wt_info) = worktree_info {
            worktree_merge::cleanup(wt_info);
        }

        // 8. Check if parent should be auto-closed
        if mana_dir.exists() {
            if let Some(parent_id) = &unit.parent {
                let auto_close_enabled =
                    config.as_ref().map(|c| c.auto_close_parent).unwrap_or(true);
                if auto_close_enabled && parent::all_children_closed(mana_dir, parent_id)? {
                    parent::auto_close_parent(mana_dir, parent_id)?;
                }
            }
        }
    }

    // Report rejected units
    if !rejected_beans.is_empty() {
        eprintln!(
            "Failed to close {} unit(s) due to pre-close hook rejection: {}",
            rejected_beans.len(),
            rejected_beans.join(", ")
        );
    }

    // Rebuild index once after all updates
    if (any_closed || !ids.is_empty()) && mana_dir.exists() {
        let index = Index::build(mana_dir).with_context(|| "Failed to rebuild index")?;
        index
            .save(mana_dir)
            .with_context(|| "Failed to save index")?;
    }

    Ok(())
}

/// Mark an attempt as explicitly failed.
///
/// The unit stays open and the claim is released so another agent can retry.
/// Records the failure in attempt_log for episodic memory.
pub fn cmd_close_failed(mana_dir: &Path, ids: Vec<String>, reason: Option<String>) -> Result<()> {
    if ids.is_empty() {
        return Err(anyhow!("At least one unit ID is required"));
    }

    let now = Utc::now();

    for id in &ids {
        let bean_path =
            find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;

        let mut unit =
            Unit::from_file(&bean_path).with_context(|| format!("Failed to load unit: {}", id))?;

        // Finalize the current attempt as failed
        if let Some(attempt) = unit.attempt_log.last_mut() {
            if attempt.finished_at.is_none() {
                attempt.outcome = crate::unit::AttemptOutcome::Failed;
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
                bean_id: id.clone(),
                bean_title: unit.title.clone(),
                attempt: attempt_num.max(1),
                duration_secs,
                tool_count: 0,
                turns: 0,
                input_tokens: 0,
                output_tokens: 0,
                cost: 0.0,
                error: reason.clone(),
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

        let attempt_count = unit.attempt_log.len();
        println!(
            "Marked unit {} as failed (attempt #{}): {}",
            id, attempt_count, unit.title
        );
        if let Some(ref reason_text) = reason {
            println!("  Reason: {}", reason_text);
        }
        println!("  Unit remains open for retry.");
    }

    // Rebuild index
    let index = Index::build(mana_dir).with_context(|| "Failed to rebuild index")?;
    index
        .save(mana_dir)
        .with_context(|| "Failed to save index")?;

    Ok(())
}


#[cfg(test)]
#[path = "tests_close.rs"]
mod tests;

#[cfg(test)]
#[path = "tests_verify_timeout.rs"]
mod verify_timeout_tests;
