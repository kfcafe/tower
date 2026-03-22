//! Review queue — list and rank units awaiting review.
//!
//! Scans `.mana/` for units that are closed and haven't been reviewed,
//! scores them, and returns a ranked queue.

use anyhow::Result;
use std::path::Path;

use crate::risk;
use crate::types::{FileChange, QueueEntry};

/// Build the review queue from the current `.mana/` state.
///
/// Returns units that are closed (verify passed) but haven't been
/// reviewed yet, ranked by risk level (highest first).
pub fn build(mana_dir: &Path, project_root: &Path) -> Result<Vec<QueueEntry>> {
    let index = mana_core::api::load_index(mana_dir)?;

    let mut entries = Vec::new();

    for unit_entry in &index.units {
        // Only show closed units (verify passed)
        if unit_entry.status != mana_core::api::Status::Closed {
            continue;
        }

        // Skip features — they're human-reviewed at a higher level
        if unit_entry.feature {
            continue;
        }

        // TODO: skip units that already have a review recorded
        // For now, include all closed units

        // Load the full unit for risk scoring
        let unit = mana_core::api::get_unit(mana_dir, &unit_entry.id)?;

        // For now, create placeholder file changes
        // Real implementation will use diff::compute with the unit's checkpoint
        let file_changes = get_file_changes_for_unit(project_root, &unit)?;

        let (risk_level, risk_flags) = risk::score(&unit, &file_changes);

        let total_additions: u32 = file_changes.iter().map(|fc| fc.additions).sum();
        let total_deletions: u32 = file_changes.iter().map(|fc| fc.deletions).sum();

        entries.push(QueueEntry {
            unit_id: unit.id.clone(),
            title: unit.title.clone(),
            risk_level,
            risk_flags,
            attempt: unit.attempts,
            file_count: file_changes.len(),
            additions: total_additions,
            deletions: total_deletions,
        });
    }

    // Sort: Critical first, then High, Normal, Low
    entries.sort_by(|a, b| b.risk_level.cmp(&a.risk_level));

    Ok(entries)
}

/// Get file changes for a unit.
///
/// Uses the unit's checkpoint (if available) to compute the diff
/// against the state before work began.
fn get_file_changes_for_unit(
    _project_root: &Path,
    _unit: &mana_core::unit::Unit,
) -> Result<Vec<FileChange>> {
    // TODO: implement using diff::compute with unit.checkpoint
    // For now, return empty — the queue will show all closed units
    // with no risk flags until diff integration is complete.
    Ok(vec![])
}
