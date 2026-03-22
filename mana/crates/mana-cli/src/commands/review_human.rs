//! `mana review` — Human code review for agent-produced work.
//!
//! ## Modes
//! - `mana review` — show review queue (closed units ranked by risk)
//! - `mana review <id>` — open HTML review page in browser
//! - `mana review <id> --approve` — approve the unit
//! - `mana review <id> --request-changes "feedback"` — request changes
//! - `mana review <id> --reject "reason"` — reject the unit

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use chrono::Utc;

use mana_review::types::*;

/// Entry point for `mana review` (human review mode).
pub fn cmd_review_human(
    mana_dir: &Path,
    id: Option<&str>,
    approve: bool,
    request_changes: Option<&str>,
    reject: Option<&str>,
) -> Result<()> {
    match id {
        None => show_queue(mana_dir),
        Some(id) => {
            if approve {
                record_decision(mana_dir, id, ReviewDecision::Approved, None)
            } else if let Some(feedback) = request_changes {
                record_decision(
                    mana_dir,
                    id,
                    ReviewDecision::ChangesRequested,
                    Some(feedback),
                )
            } else if let Some(reason) = reject {
                record_decision(mana_dir, id, ReviewDecision::Rejected, Some(reason))
            } else {
                open_review_page(mana_dir, id)
            }
        }
    }
}

/// Show the review queue: closed units ranked by risk.
fn show_queue(mana_dir: &Path) -> Result<()> {
    let entries = mana_review::queue::build(mana_dir, project_root(mana_dir))?;

    if entries.is_empty() {
        println!("No units awaiting review.");
        return Ok(());
    }

    println!("Review Queue ({} units)\n", entries.len());
    println!(
        "  {:<8} {:<6} {:<40} {:>6} {:>5} {:>5}",
        "ID", "RISK", "TITLE", "FILES", "+", "-"
    );
    println!("  {}", "─".repeat(78));

    for entry in &entries {
        let risk_marker = match entry.risk_level {
            RiskLevel::Critical => "▲ CRIT",
            RiskLevel::High => "▲ HIGH",
            RiskLevel::Normal => "○ NORM",
            RiskLevel::Low => "· LOW ",
        };

        let title = if entry.title.len() > 38 {
            format!("{}…", &entry.title[..37])
        } else {
            entry.title.clone()
        };

        println!(
            "  {:<8} {:<6} {:<40} {:>6} {:>5} {:>5}",
            entry.unit_id,
            risk_marker,
            title,
            entry.file_count,
            format!("+{}", entry.additions),
            format!("-{}", entry.deletions),
        );

        // Show risk flags on next line if any
        for flag in &entry.risk_flags {
            println!("           └─ {} — {}", flag.kind, flag.message);
        }
    }

    println!("\nRun: mana review <id>  to open the review page");

    Ok(())
}

/// Open the HTML review page for a unit.
fn open_review_page(mana_dir: &Path, id: &str) -> Result<()> {
    let root = project_root(mana_dir);

    // Load the unit
    let unit =
        mana_core::api::get_unit(mana_dir, id).with_context(|| format!("Unit not found: {id}"))?;

    // Compute diff
    let (diff, file_changes) = mana_review::diff::compute(root, unit.checkpoint.as_deref())
        .unwrap_or_else(|_| (String::new(), vec![]));

    // Score risk
    let (risk_level, risk_flags) = mana_review::risk::score(&unit, &file_changes);

    // Load prior reviews
    let prior_reviews = mana_review::state::load_all(mana_dir).unwrap_or_default();
    let prior_reviews: Vec<_> = prior_reviews
        .into_iter()
        .filter(|r| r.unit_id == id)
        .collect();

    // Build the review candidate
    let candidate = ReviewCandidate {
        unit,
        file_changes,
        diff,
        risk_level,
        risk_flags,
        prior_reviews,
    };

    // Generate HTML
    let html = mana_review::render::generate_html(&candidate);

    // Write to temp file and open
    let review_dir = mana_dir.join("reviews");
    fs::create_dir_all(&review_dir)?;
    let html_path = review_dir.join(format!("review-{}.html", id.replace('.', "-")));
    fs::write(&html_path, &html)?;

    eprintln!("Opening review page: {}", html_path.display());

    // Try to open in browser
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(&html_path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(&html_path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(["/c", "start", &html_path.to_string_lossy()])
            .spawn();
    }

    println!("Review: {}", html_path.display());
    println!(
        "\nAfter reviewing, run one of:\n  \
         mana review {id} --approve\n  \
         mana review {id} --request-changes \"your feedback\"\n  \
         mana review {id} --reject \"reason\""
    );

    Ok(())
}

/// Record a review decision.
fn record_decision(
    mana_dir: &Path,
    id: &str,
    decision: ReviewDecision,
    message: Option<&str>,
) -> Result<()> {
    // Load unit to get attempt count
    let unit =
        mana_core::api::get_unit(mana_dir, id).with_context(|| format!("Unit not found: {id}"))?;

    let review = Review {
        unit_id: id.to_string(),
        attempt: unit.attempts,
        decision: decision.clone(),
        summary: message.map(|s| s.to_string()),
        annotations: vec![], // TODO: support inline annotations
        reviewed_at: Utc::now(),
        reviewer: "human".to_string(),
    };

    // Save the review
    mana_review::state::save(mana_dir, &review)?;

    match decision {
        ReviewDecision::Approved => {
            eprintln!("✓ Approved unit {id}");
            // Add reviewed label
            let unit_path = mana_core::discovery::find_unit_file(mana_dir, id)?;
            let mut unit = mana_core::unit::Unit::from_file(&unit_path)?;
            if !unit.labels.contains(&"reviewed".to_string()) {
                unit.labels.push("reviewed".to_string());
            }
            unit.labels.retain(|l| l != "review-failed");
            unit.updated_at = Utc::now();
            unit.to_file(&unit_path)?;
        }
        ReviewDecision::ChangesRequested => {
            eprintln!("↺ Changes requested for unit {id}");
            if let Some(msg) = message {
                eprintln!("  Feedback: {msg}");
            }
            // Reopen the unit so it can be retried
            let unit_path = mana_core::discovery::find_unit_file(mana_dir, id)?;
            let mut unit = mana_core::unit::Unit::from_file(&unit_path)?;
            unit.status = mana_core::unit::Status::Open;
            unit.closed_at = None;
            unit.close_reason = None;
            if !unit.labels.contains(&"review-failed".to_string()) {
                unit.labels.push("review-failed".to_string());
            }
            unit.labels.retain(|l| l != "reviewed");

            // Append review feedback to notes so the next agent sees it
            let feedback_note = format!(
                "\n---\n**Review: changes requested** ({})\n\n{}\n",
                Utc::now().format("%Y-%m-%d %H:%M UTC"),
                message.unwrap_or("(no details)")
            );
            match unit.notes {
                Some(ref mut existing) => existing.push_str(&feedback_note),
                None => unit.notes = Some(feedback_note),
            }
            unit.updated_at = Utc::now();
            unit.to_file(&unit_path)?;
        }
        ReviewDecision::Rejected => {
            eprintln!("✕ Rejected unit {id}");
            if let Some(msg) = message {
                eprintln!("  Reason: {msg}");
            }
            // Add rejection label but keep it closed
            let unit_path = mana_core::discovery::find_unit_file(mana_dir, id)?;
            let mut unit = mana_core::unit::Unit::from_file(&unit_path)?;
            if !unit.labels.contains(&"rejected".to_string()) {
                unit.labels.push("rejected".to_string());
            }
            unit.updated_at = Utc::now();
            unit.to_file(&unit_path)?;
        }
    }

    // Rebuild index
    let index =
        mana_core::index::Index::build(mana_dir).context("Failed to rebuild index after review")?;
    index
        .save(mana_dir)
        .context("Failed to save index after review")?;

    Ok(())
}

/// Get the project root from the .mana/ directory.
fn project_root(mana_dir: &Path) -> &Path {
    mana_dir.parent().unwrap_or(mana_dir)
}
