use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::unit::Unit;
use crate::discovery::find_archived_unit;
use crate::index::{ArchiveIndex, Index};

/// Unarchive a unit by moving it from `.mana/archive/**/` back to `.mana/`.
///
/// 1. Find the unit in the archive using find_archived_unit()
/// 2. Load the unit and extract its slug
/// 3. Compute the target path: `.mana/<id>-<slug>.md`
/// 4. Move the file using std::fs::rename
/// 5. Set is_archived = false
/// 6. Update updated_at timestamp
/// 7. Save the unit to its new location
/// 8. Rebuild the index
/// 9. Git commit the changes
pub fn cmd_unarchive(mana_dir: &Path, id: &str) -> Result<()> {
    // Find the archived unit
    let archived_path = find_archived_unit(mana_dir, id)
        .with_context(|| format!("Archived unit not found: {}", id))?;

    // Load the unit from archive
    let mut unit = Unit::from_file(&archived_path)
        .with_context(|| format!("Failed to load archived unit: {}", id))?;

    // Check if the unit is actually marked as archived
    if !unit.is_archived {
        anyhow::bail!("Unit {} is not marked as archived", id);
    }

    // Get the slug from the unit (should be set)
    let slug = unit
        .slug
        .clone()
        .unwrap_or_else(|| crate::util::title_to_slug(&unit.title));

    // Compute the target path in main units directory
    let target_path = mana_dir.join(format!("{}-{}.md", id, slug));

    // Check if unit already exists in main directory
    if target_path.exists() {
        anyhow::bail!(
            "Unit {} already exists in main directory at {}",
            id,
            target_path.display()
        );
    }

    // Move the file from archive to main units directory
    std::fs::rename(&archived_path, &target_path)
        .with_context(|| format!("Failed to move unit {} from archive to main directory", id))?;

    // Update unit metadata
    unit.is_archived = false;
    unit.updated_at = Utc::now();

    // Save the unit to its new location
    unit.to_file(&target_path)
        .with_context(|| format!("Failed to save unarchived unit: {}", id))?;

    // Remove from archive index
    {
        let mut archive_index =
            ArchiveIndex::load(mana_dir).unwrap_or(ArchiveIndex { units: Vec::new() });
        archive_index.remove(id);
        let _ = archive_index.save(mana_dir);
    }

    // Rebuild index
    let index = Index::build(mana_dir).with_context(|| "Failed to rebuild index")?;
    index
        .save(mana_dir)
        .with_context(|| "Failed to save index")?;

    println!("Unarchived unit {}: {}", id, unit.title);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_beans_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    fn create_archived_bean(
        mana_dir: &Path,
        id: &str,
        title: &str,
        year: &str,
        month: &str,
    ) -> std::path::PathBuf {
        let archive_dir = mana_dir.join("archive").join(year).join(month);
        fs::create_dir_all(&archive_dir).unwrap();

        let mut unit = Unit::new(id, title);
        unit.is_archived = true;

        let slug = title_to_slug(title);
        let bean_path = archive_dir.join(format!("{}-{}.md", id, slug));
        unit.to_file(&bean_path).unwrap();

        bean_path
    }

    #[test]
    fn test_unarchive_basic() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        // Create an archived unit
        let archived_path = create_archived_bean(&mana_dir, "1", "Task", "2026", "01");
        assert!(archived_path.exists());

        // Unarchive it
        cmd_unarchive(&mana_dir, "1").unwrap();

        // Verify the unit is no longer in archive
        assert!(!archived_path.exists());

        // Verify the unit is now in main directory
        let unarchived_path = mana_dir.join("1-task.md");
        assert!(unarchived_path.exists());

        // Verify is_archived is false
        let unarchived_bean = Unit::from_file(&unarchived_path).unwrap();
        assert!(!unarchived_bean.is_archived);
    }

    #[test]
    fn test_unarchive_preserves_slug() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        // Create an archived unit with a specific title/slug
        let archived_path =
            create_archived_bean(&mana_dir, "12", "Complex Unit Title", "2026", "01");

        cmd_unarchive(&mana_dir, "12").unwrap();

        // Verify the unit is now at the correct main path with the same slug
        let expected_path = mana_dir.join("12-complex-unit-title.md");
        assert!(expected_path.exists());
        assert!(!archived_path.exists());

        let unarchived_bean = Unit::from_file(&expected_path).unwrap();
        assert_eq!(unarchived_bean.id, "12");
        assert_eq!(unarchived_bean.title, "Complex Unit Title");
        assert!(!unarchived_bean.is_archived);
    }

    #[test]
    fn test_unarchive_nonexistent_bean() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let result = cmd_unarchive(&mana_dir, "999");
        assert!(result.is_err());
    }

    #[test]
    fn test_unarchive_updates_index() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        // Create an archived unit
        create_archived_bean(&mana_dir, "1", "Task", "2026", "01");

        // Build initial index (should be empty)
        let initial_index = Index::build(&mana_dir).unwrap();
        assert!(initial_index.units.is_empty());

        // Unarchive the unit
        cmd_unarchive(&mana_dir, "1").unwrap();

        // Verify index is rebuilt with the unarchived unit
        let updated_index = Index::load(&mana_dir).unwrap();
        assert_eq!(updated_index.units.len(), 1);
        assert_eq!(updated_index.units[0].id, "1");
    }

    #[test]
    fn test_unarchive_updates_updated_at() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        let archived_path = create_archived_bean(&mana_dir, "1", "Task", "2026", "01");
        let original_bean = Unit::from_file(&archived_path).unwrap();
        let original_updated_at = original_bean.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));

        cmd_unarchive(&mana_dir, "1").unwrap();

        let unarchived_path = mana_dir.join("1-task.md");
        let unarchived_bean = Unit::from_file(&unarchived_path).unwrap();
        assert!(unarchived_bean.updated_at > original_updated_at);
    }

    #[test]
    fn test_unarchive_already_in_main_dir() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        // Create an archived unit
        create_archived_bean(&mana_dir, "1", "Task", "2026", "01");

        // Pre-create the unit in the main directory
        let main_path = mana_dir.join("1-task.md");
        let existing_bean = Unit::new("1", "Existing");
        existing_bean.to_file(&main_path).unwrap();

        // Try to unarchive - should fail because unit already exists in main dir
        let result = cmd_unarchive(&mana_dir, "1");
        assert!(result.is_err());
    }

    #[test]
    fn test_unarchive_not_marked_archived() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        // Create a unit that's not marked as archived in the archive directory
        let archive_dir = mana_dir.join("archive").join("2026").join("01");
        fs::create_dir_all(&archive_dir).unwrap();

        let mut unit = Unit::new("1", "Task");
        unit.is_archived = false; // Not archived!
        let slug = title_to_slug("Task");
        let bean_path = archive_dir.join(format!("1-{}.md", slug));
        unit.to_file(&bean_path).unwrap();

        // Try to unarchive - should fail because unit is not marked as archived
        let result = cmd_unarchive(&mana_dir, "1");
        assert!(result.is_err());
    }

    #[test]
    fn test_unarchive_preserves_bean_data() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        let archive_dir = mana_dir.join("archive").join("2026").join("01");
        fs::create_dir_all(&archive_dir).unwrap();

        let mut unit = Unit::new("1", "Complex Task");
        unit.is_archived = true;
        unit.description = Some("This is a detailed description".to_string());
        unit.acceptance = Some("- Acceptance 1\n- Acceptance 2".to_string());
        unit.priority = 1;
        unit.labels = vec!["label1".to_string(), "label2".to_string()];

        let slug = title_to_slug("Complex Task");
        let bean_path = archive_dir.join(format!("1-{}.md", slug));
        unit.to_file(&bean_path).unwrap();

        cmd_unarchive(&mana_dir, "1").unwrap();

        let unarchived_path = mana_dir.join("1-complex-task.md");
        let unarchived_bean = Unit::from_file(&unarchived_path).unwrap();

        // Verify all data is preserved
        assert_eq!(unarchived_bean.id, "1");
        assert_eq!(unarchived_bean.title, "Complex Task");
        assert_eq!(
            unarchived_bean.description,
            Some("This is a detailed description".to_string())
        );
        assert_eq!(
            unarchived_bean.acceptance,
            Some("- Acceptance 1\n- Acceptance 2".to_string())
        );
        assert_eq!(unarchived_bean.priority, 1);
        assert_eq!(
            unarchived_bean.labels,
            vec!["label1".to_string(), "label2".to_string()]
        );
        assert!(!unarchived_bean.is_archived);
    }

    #[test]
    fn test_unarchive_nested_year_month_structure() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        // Create archived unit in deeply nested structure
        create_archived_bean(&mana_dir, "5", "Deep Task", "2025", "06");

        cmd_unarchive(&mana_dir, "5").unwrap();

        let unarchived_path = mana_dir.join("5-deep-task.md");
        assert!(unarchived_path.exists());

        let unarchived_bean = Unit::from_file(&unarchived_path).unwrap();
        assert_eq!(unarchived_bean.id, "5");
        assert!(!unarchived_bean.is_archived);
    }

    #[test]
    fn test_unarchive_removes_from_archive_yaml() {
        let (_dir, mana_dir) = setup_test_beans_dir();

        // Create two archived units
        create_archived_bean(&mana_dir, "1", "Task One", "2026", "01");
        create_archived_bean(&mana_dir, "2", "Task Two", "2026", "01");

        // Build and save archive index with both units
        let archive = ArchiveIndex::build(&mana_dir).unwrap();
        archive.save(&mana_dir).unwrap();
        assert_eq!(archive.units.len(), 2);

        // Unarchive unit 1
        cmd_unarchive(&mana_dir, "1").unwrap();

        // archive.yaml should now only contain unit 2
        let updated_archive = ArchiveIndex::load(&mana_dir).unwrap();
        assert_eq!(updated_archive.units.len(), 1);
        assert_eq!(updated_archive.units[0].id, "2");
    }
}
