use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use super::archive;
use crate::discovery::{find_archived_unit, find_unit_file};
use crate::index::Index;
use crate::unit::{Status, Unit};

/// Check if all children of a parent unit are closed (in archive or with status=closed).
///
/// Returns true if:
/// - The parent has no children, OR
/// - All children are either in the archive (closed) or have status=closed
pub(crate) fn all_children_closed(mana_dir: &Path, parent_id: &str) -> Result<bool> {
    // Always rebuild the index fresh - we can't rely on staleness check because
    // files may have just been moved to archive (which isn't tracked in staleness)
    let index = Index::build(mana_dir)?;
    let archived = Index::collect_archived(mana_dir).unwrap_or_default();

    // Combine active and archived units
    let mut all_beans = index.units;
    all_beans.extend(archived);

    // Find children of this parent
    let children: Vec<_> = all_beans
        .iter()
        .filter(|b| b.parent.as_deref() == Some(parent_id))
        .collect();

    // If no children, return true (nothing to check)
    if children.is_empty() {
        return Ok(true);
    }

    // Check if all children are closed
    for child in children {
        if child.status != Status::Closed {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Close a parent unit automatically when all its children are closed.
/// This is called recursively to close ancestor units.
///
/// Unlike normal close, auto-close:
/// - Skips verify command (children already verified)
/// - Sets close_reason to indicate auto-close
/// - Recursively checks grandparent
pub(crate) fn auto_close_parent(mana_dir: &Path, parent_id: &str) -> Result<()> {
    // Find the parent unit
    let bean_path = match find_unit_file(mana_dir, parent_id) {
        Ok(path) => path,
        Err(_) => {
            // Parent might already be archived, skip
            return Ok(());
        }
    };

    let mut unit = Unit::from_file(&bean_path)
        .with_context(|| format!("Failed to load parent unit: {}", parent_id))?;

    // Skip if already closed
    if unit.status == Status::Closed {
        return Ok(());
    }

    // Feature units are never auto-closed — they require human review
    if unit.feature {
        return Ok(());
    }

    let now = Utc::now();

    // Close the parent (skip verify - children already verified)
    unit.status = Status::Closed;
    unit.closed_at = Some(now);
    unit.close_reason = Some("Auto-closed: all children completed".to_string());
    unit.updated_at = now;

    unit.to_file(&bean_path)
        .with_context(|| format!("Failed to save parent unit: {}", parent_id))?;

    // Archive the closed unit
    archive::archive_bean(mana_dir, &mut unit, &bean_path)?;

    println!("Auto-closed parent unit {}: {}", parent_id, unit.title);

    // Recursively check if this unit's parent should also be auto-closed
    if let Some(grandparent_id) = &unit.parent {
        if all_children_closed(mana_dir, grandparent_id)? {
            auto_close_parent(mana_dir, grandparent_id)?;
        }
    }

    Ok(())
}

/// Walk up the parent chain to find the root ancestor of a unit.
///
/// Returns the ID of the topmost parent (the unit with no parent).
/// If the unit itself has no parent, returns its own ID.
/// Handles archived parents gracefully by checking both active and archived units.
pub(crate) fn find_root_parent(mana_dir: &Path, unit: &Unit) -> Result<String> {
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
            Err(_) => return Ok(current_id), // Can't find parent, assume it's root
        }
    }
}
