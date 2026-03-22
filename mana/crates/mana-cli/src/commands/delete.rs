use std::path::Path;

use anyhow::Result;
use mana_core::ops::delete as ops_delete;

/// Delete a unit and clean up all references to it in other units' dependencies.
///
/// 1. Load the unit to get its title (for printing)
/// 2. Delete the unit file
/// 3. Scan all remaining units and remove deleted_id from their dependencies
/// 4. Rebuild the index
pub fn cmd_delete(mana_dir: &Path, id: &str) -> Result<()> {
    let result = ops_delete::delete(mana_dir, id)?;
    println!("Deleted unit {}: {}", result.id, result.title);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::index::Index;
    use crate::unit::Unit;
    use crate::util::title_to_slug;
    use tempfile::TempDir;

    fn setup_test_mana_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn test_delete_unit() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let unit = Unit::new("1", "Task to delete");
        let slug = title_to_slug(&unit.title);
        let unit_path = mana_dir.join(format!("1-{}.md", slug));
        unit.to_file(&unit_path).unwrap();

        assert!(unit_path.exists());

        cmd_delete(&mana_dir, "1").unwrap();

        assert!(!unit_path.exists());
    }

    #[test]
    fn test_delete_nonexistent_unit() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let result = cmd_delete(&mana_dir, "99");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_cleans_dependencies() {
        let (_dir, mana_dir) = setup_test_mana_dir();

        // Create units with dependencies
        let unit1 = Unit::new("1", "Task 1");
        let mut unit2 = Unit::new("2", "Task 2");
        let mut unit3 = Unit::new("3", "Task 3");

        unit2.dependencies = vec!["1".to_string()];
        unit3.dependencies = vec!["1".to_string(), "2".to_string()];

        let slug1 = title_to_slug(&unit1.title);
        let slug2 = title_to_slug(&unit2.title);
        let slug3 = title_to_slug(&unit3.title);

        unit1
            .to_file(mana_dir.join(format!("1-{}.md", slug1)))
            .unwrap();
        unit2
            .to_file(mana_dir.join(format!("2-{}.md", slug2)))
            .unwrap();
        unit3
            .to_file(mana_dir.join(format!("3-{}.md", slug3)))
            .unwrap();

        // Delete unit 1
        cmd_delete(&mana_dir, "1").unwrap();

        // Verify unit 2 no longer depends on 1
        let unit2_updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "2").unwrap()).unwrap();
        assert!(!unit2_updated.dependencies.contains(&"1".to_string()));

        // Verify unit 3 no longer depends on 1, but still depends on 2
        let unit3_updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "3").unwrap()).unwrap();
        assert!(!unit3_updated.dependencies.contains(&"1".to_string()));
        assert!(unit3_updated.dependencies.contains(&"2".to_string()));
    }

    #[test]
    fn test_delete_rebuilds_index() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let unit1 = Unit::new("1", "Task 1");
        let unit2 = Unit::new("2", "Task 2");
        let slug1 = title_to_slug(&unit1.title);
        let slug2 = title_to_slug(&unit2.title);
        unit1
            .to_file(mana_dir.join(format!("1-{}.md", slug1)))
            .unwrap();
        unit2
            .to_file(mana_dir.join(format!("2-{}.md", slug2)))
            .unwrap();

        cmd_delete(&mana_dir, "1").unwrap();

        let index = Index::load(&mana_dir).unwrap();
        assert_eq!(index.units.len(), 1);
        assert_eq!(index.units[0].id, "2");
    }

    #[test]
    fn test_cleanup_does_not_modify_unrelated_units() {
        let (_dir, mana_dir) = setup_test_mana_dir();

        // Create units where only some depend on 1
        let unit1 = Unit::new("1", "Task 1");
        let mut unit2 = Unit::new("2", "Task 2");
        let unit3 = Unit::new("3", "Task 3"); // No dependencies

        unit2.dependencies = vec!["1".to_string()];

        let slug1 = title_to_slug(&unit1.title);
        let slug2 = title_to_slug(&unit2.title);
        let slug3 = title_to_slug(&unit3.title);

        unit1
            .to_file(mana_dir.join(format!("1-{}.md", slug1)))
            .unwrap();
        unit2
            .to_file(mana_dir.join(format!("2-{}.md", slug2)))
            .unwrap();
        unit3
            .to_file(mana_dir.join(format!("3-{}.md", slug3)))
            .unwrap();

        cmd_delete(&mana_dir, "1").unwrap();

        let unit3_check =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "3").unwrap()).unwrap();
        assert!(unit3_check.dependencies.is_empty());
    }

    #[test]
    fn test_delete_with_complex_dependency_graph() {
        let (_dir, mana_dir) = setup_test_mana_dir();

        // Create a diamond dependency: 4 <- [2, 3], 2 <- 1, 3 <- 1
        let unit1 = Unit::new("1", "Root");
        let mut unit2 = Unit::new("2", "Middle left");
        let mut unit3 = Unit::new("3", "Middle right");
        let mut unit4 = Unit::new("4", "Bottom");

        unit2.dependencies = vec!["1".to_string()];
        unit3.dependencies = vec!["1".to_string()];
        unit4.dependencies = vec!["2".to_string(), "3".to_string()];

        let slug1 = title_to_slug(&unit1.title);
        let slug2 = title_to_slug(&unit2.title);
        let slug3 = title_to_slug(&unit3.title);
        let slug4 = title_to_slug(&unit4.title);

        unit1
            .to_file(mana_dir.join(format!("1-{}.md", slug1)))
            .unwrap();
        unit2
            .to_file(mana_dir.join(format!("2-{}.md", slug2)))
            .unwrap();
        unit3
            .to_file(mana_dir.join(format!("3-{}.md", slug3)))
            .unwrap();
        unit4
            .to_file(mana_dir.join(format!("4-{}.md", slug4)))
            .unwrap();

        // Delete node 1
        cmd_delete(&mana_dir, "1").unwrap();

        // Verify cleanup
        let unit2_updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "2").unwrap()).unwrap();
        let unit3_updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "3").unwrap()).unwrap();
        let unit4_updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "4").unwrap()).unwrap();

        assert!(!unit2_updated.dependencies.contains(&"1".to_string()));
        assert!(!unit3_updated.dependencies.contains(&"1".to_string()));
        assert!(!unit4_updated.dependencies.contains(&"1".to_string()));
        assert!(unit4_updated.dependencies.contains(&"2".to_string()));
        assert!(unit4_updated.dependencies.contains(&"3".to_string()));
    }

    #[test]
    fn test_delete_ignores_excluded_files() {
        let (_dir, mana_dir) = setup_test_mana_dir();

        let unit1 = Unit::new("1", "Task 1");
        let slug1 = title_to_slug(&unit1.title);
        unit1
            .to_file(mana_dir.join(format!("1-{}.md", slug1)))
            .unwrap();

        // Create config.yaml with a fake reference to "1"
        fs::write(
            mana_dir.join("config.yaml"),
            "next_id: 2\nproject_name: test\n",
        )
        .unwrap();

        // This should not fail even though config.yaml exists
        cmd_delete(&mana_dir, "1").unwrap();
        assert!(!mana_dir.join(format!("1-{}.md", slug1)).exists());
    }
}
