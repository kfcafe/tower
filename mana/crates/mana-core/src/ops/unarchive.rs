use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::discovery::find_archived_unit;
use crate::index::{ArchiveIndex, Index};
use crate::unit::Unit;

/// Result of unarchiving a unit.
pub struct UnarchiveResult {
    pub unit: Unit,
    pub path: std::path::PathBuf,
}

/// Unarchive a unit by moving it from `.mana/archive/**/` back to `.mana/`.
///
/// 1. Find the unit in the archive
/// 2. Verify it's marked as archived
/// 3. Move to main units directory
/// 4. Clear is_archived flag, update timestamp
/// 5. Remove from archive index, rebuild main index
pub fn unarchive(mana_dir: &Path, id: &str) -> Result<UnarchiveResult> {
    let archived_path = find_archived_unit(mana_dir, id)
        .with_context(|| format!("Archived unit not found: {}", id))?;

    let mut unit = Unit::from_file(&archived_path)
        .with_context(|| format!("Failed to load archived unit: {}", id))?;

    if !unit.is_archived {
        anyhow::bail!("Unit {} is not marked as archived", id);
    }

    let slug = unit
        .slug
        .clone()
        .unwrap_or_else(|| crate::util::title_to_slug(&unit.title));

    let target_path = mana_dir.join(format!("{}-{}.md", id, slug));

    if target_path.exists() {
        anyhow::bail!(
            "Unit {} already exists in main directory at {}",
            id,
            target_path.display()
        );
    }

    std::fs::rename(&archived_path, &target_path)
        .with_context(|| format!("Failed to move unit {} from archive to main directory", id))?;

    unit.is_archived = false;
    unit.updated_at = Utc::now();

    unit.to_file(&target_path)
        .with_context(|| format!("Failed to save unarchived unit: {}", id))?;

    {
        let mut archive_index =
            ArchiveIndex::load(mana_dir).unwrap_or(ArchiveIndex { units: Vec::new() });
        archive_index.remove(id);
        let _ = archive_index.save(mana_dir);
    }

    let index = Index::build(mana_dir).with_context(|| "Failed to rebuild index")?;
    index
        .save(mana_dir)
        .with_context(|| "Failed to save index")?;

    Ok(UnarchiveResult {
        unit,
        path: target_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    fn create_archived_unit(mana_dir: &Path, id: &str, title: &str) -> std::path::PathBuf {
        let archive_dir = mana_dir.join("archive").join("2026").join("01");
        fs::create_dir_all(&archive_dir).unwrap();

        let mut unit = Unit::new(id, title);
        unit.is_archived = true;

        let slug = title_to_slug(title);
        let unit_path = archive_dir.join(format!("{}-{}.md", id, slug));
        unit.to_file(&unit_path).unwrap();

        unit_path
    }

    #[test]
    fn unarchive_basic() {
        let (_dir, mana_dir) = setup();

        let archived_path = create_archived_unit(&mana_dir, "1", "Task");
        assert!(archived_path.exists());

        let result = unarchive(&mana_dir, "1").unwrap();

        assert!(!archived_path.exists());
        assert!(result.path.exists());
        assert!(!result.unit.is_archived);
    }

    #[test]
    fn unarchive_nonexistent() {
        let (_dir, mana_dir) = setup();
        let result = unarchive(&mana_dir, "999");
        assert!(result.is_err());
    }

    #[test]
    fn unarchive_updates_index() {
        let (_dir, mana_dir) = setup();

        create_archived_unit(&mana_dir, "1", "Task");

        let initial_index = Index::build(&mana_dir).unwrap();
        assert!(initial_index.units.is_empty());

        unarchive(&mana_dir, "1").unwrap();

        let updated_index = Index::load(&mana_dir).unwrap();
        assert_eq!(updated_index.units.len(), 1);
        assert_eq!(updated_index.units[0].id, "1");
    }

    #[test]
    fn unarchive_preserves_data() {
        let (_dir, mana_dir) = setup();

        let archive_dir = mana_dir.join("archive").join("2026").join("01");
        fs::create_dir_all(&archive_dir).unwrap();

        let mut unit = Unit::new("1", "Complex Task");
        unit.is_archived = true;
        unit.description = Some("Detailed description".to_string());
        unit.priority = 1;
        unit.labels = vec!["label1".to_string()];

        let slug = title_to_slug("Complex Task");
        unit.to_file(archive_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        let result = unarchive(&mana_dir, "1").unwrap();

        assert_eq!(
            result.unit.description,
            Some("Detailed description".to_string())
        );
        assert_eq!(result.unit.priority, 1);
        assert_eq!(result.unit.labels, vec!["label1".to_string()]);
        assert!(!result.unit.is_archived);
    }

    #[test]
    fn unarchive_already_in_main_dir() {
        let (_dir, mana_dir) = setup();

        create_archived_unit(&mana_dir, "1", "Task");

        let main_path = mana_dir.join("1-task.md");
        let existing_unit = Unit::new("1", "Existing");
        existing_unit.to_file(&main_path).unwrap();

        let result = unarchive(&mana_dir, "1");
        assert!(result.is_err());
    }
}
