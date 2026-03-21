use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::discovery::archive_path_for_bean;
use crate::index::{ArchiveIndex, IndexEntry};
use crate::unit::Unit;
use crate::util::title_to_slug;

/// Move a closed unit to the dated archive directory.
/// Updates the unit's `is_archived` flag and writes to the archive path.
/// Returns the archive path.
pub(crate) fn archive_bean(mana_dir: &Path, unit: &mut Unit, bean_path: &Path) -> Result<PathBuf> {
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

    // Create archive directories if needed
    if let Some(parent) = archive_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create archive directories for unit {}", id))?;
    }

    // Move the unit file to archive
    std::fs::rename(bean_path, &archive_path)
        .with_context(|| format!("Failed to move unit {} to archive", id))?;

    // Update unit metadata to mark as archived
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
