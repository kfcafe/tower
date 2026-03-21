use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::discovery::find_unit_file;
use crate::index::Index;
use crate::unit::Unit;

/// Find the next available child number for a parent.
/// Scans .mana/ for existing children ({parent_id}.{N}-*.md or {parent_id}.{N}.yaml),
/// finds highest N, returns N+1.
fn next_child_number(mana_dir: &Path, parent_id: &str) -> Result<u32> {
    let mut max_child: u32 = 0;

    let dir_entries = fs::read_dir(mana_dir)
        .with_context(|| format!("Failed to read directory: {}", mana_dir.display()))?;

    for entry in dir_entries {
        let entry = entry?;
        let path = entry.path();

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // Look for files matching "{parent_id}.{N}-*.md" (new format)
        if let Some(name_without_ext) = filename.strip_suffix(".md") {
            if let Some(name_without_parent) = name_without_ext.strip_prefix(parent_id) {
                if let Some(after_dot) = name_without_parent.strip_prefix('.') {
                    // Extract the number part before the hyphen
                    let num_part = after_dot.split('-').next().unwrap_or_default();
                    if let Ok(child_num) = num_part.parse::<u32>() {
                        if child_num > max_child {
                            max_child = child_num;
                        }
                    }
                }
            }
        }

        // Also support legacy format for backward compatibility: {parent_id}.{N}.yaml
        if let Some(name_without_ext) = filename.strip_suffix(".yaml") {
            if let Some(name_without_parent) = name_without_ext.strip_prefix(parent_id) {
                if let Some(after_dot) = name_without_parent.strip_prefix('.') {
                    if let Ok(child_num) = after_dot.parse::<u32>() {
                        if child_num > max_child {
                            max_child = child_num;
                        }
                    }
                }
            }
        }
    }

    Ok(max_child + 1)
}

/// Adopt existing units as children of a parent unit.
///
/// This command:
/// 1. Validates that the parent unit exists
/// 2. For each child ID:
///    - Loads the unit
///    - Assigns a new ID: `{parent_id}.{N}` (where N is sequential)
///    - Sets the unit's `parent` field to `parent_id`
///    - Renames the file to match the new ID
/// 3. Updates all dependency references across ALL units
/// 4. Rebuilds the index
///
/// # Arguments
/// * `mana_dir` - Path to the `.mana/` directory
/// * `parent_id` - The ID of the parent unit
/// * `child_ids` - List of unit IDs to adopt as children
///
/// # Returns
/// A map of old_id -> new_id for the adopted units
pub fn cmd_adopt(
    mana_dir: &Path,
    parent_id: &str,
    child_ids: &[String],
) -> Result<HashMap<String, String>> {
    // Validate parent exists
    let parent_path = find_unit_file(mana_dir, parent_id)
        .with_context(|| format!("Parent unit '{}' not found", parent_id))?;
    let _parent_bean = Unit::from_file(&parent_path)
        .with_context(|| format!("Failed to load parent unit '{}'", parent_id))?;

    // Track ID mappings: old_id -> new_id
    let mut id_map: HashMap<String, String> = HashMap::new();

    // Find the starting child number
    let mut next_num = next_child_number(mana_dir, parent_id)?;

    // Process each child
    for old_id in child_ids {
        // Load the child unit
        let old_path = find_unit_file(mana_dir, old_id)
            .with_context(|| format!("Child unit '{}' not found", old_id))?;
        let mut unit = Unit::from_file(&old_path)
            .with_context(|| format!("Failed to load child unit '{}'", old_id))?;

        // Compute new ID
        let new_id = format!("{}.{}", parent_id, next_num);
        next_num += 1;

        // Update unit fields
        unit.id = new_id.clone();
        unit.parent = Some(parent_id.to_string());
        unit.updated_at = Utc::now();

        // Compute new file path
        let slug = unit.slug.clone().unwrap_or_else(|| "unnamed".to_string());
        let new_filename = format!("{}-{}.md", new_id, slug);
        let new_path = mana_dir.join(&new_filename);

        // Write the updated unit to the new path
        unit.to_file(&new_path)
            .with_context(|| format!("Failed to write unit to {}", new_path.display()))?;

        // Remove the old file (if it's different from the new path)
        if old_path != new_path {
            fs::remove_file(&old_path).with_context(|| {
                format!("Failed to remove old unit file {}", old_path.display())
            })?;
        }

        // Track the mapping
        id_map.insert(old_id.clone(), new_id.clone());
        println!("Adopted {} -> {} (under {})", old_id, new_id, parent_id);
    }

    // Update dependencies across all units
    if !id_map.is_empty() {
        update_all_dependencies(mana_dir, &id_map)?;
    }

    // Rebuild the index
    let index = Index::build(mana_dir)?;
    index.save(mana_dir)?;

    Ok(id_map)
}

/// Update dependency references in all units based on the ID mapping.
///
/// Scans all unit files in the directory and replaces any dependency IDs
/// that appear in the id_map with their new values.
fn update_all_dependencies(mana_dir: &Path, id_map: &HashMap<String, String>) -> Result<()> {
    let dir_entries = fs::read_dir(mana_dir)
        .with_context(|| format!("Failed to read directory: {}", mana_dir.display()))?;

    for entry in dir_entries {
        let entry = entry?;
        let path = entry.path();

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // Only process unit files (.md with hyphen or .yaml)
        let is_bean_file = (filename.ends_with(".md") && filename.contains('-'))
            || (filename.ends_with(".yaml")
                && filename != "config.yaml"
                && filename != "index.yaml"
                && filename != "unit.yaml");

        if !is_bean_file {
            continue;
        }

        // Load the unit
        let mut unit = match Unit::from_file(&path) {
            Ok(b) => b,
            Err(_) => continue, // Skip files that can't be parsed
        };

        // Check if any dependencies need updating
        let mut modified = false;
        let mut new_deps = Vec::new();

        for dep in &unit.dependencies {
            if let Some(new_id) = id_map.get(dep) {
                new_deps.push(new_id.clone());
                modified = true;
            } else {
                new_deps.push(dep.clone());
            }
        }

        // Also check and update the parent field if it was remapped
        if let Some(ref parent) = unit.parent {
            if let Some(new_parent) = id_map.get(parent) {
                unit.parent = Some(new_parent.clone());
                modified = true;
            }
        }

        // Save if modified
        if modified {
            unit.dependencies = new_deps;
            unit.updated_at = Utc::now();
            unit.to_file(&path)
                .with_context(|| format!("Failed to update unit {}", path.display()))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    fn setup_beans_dir_with_config() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let config = Config {
            project: "test".to_string(),
            next_id: 10,
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
            research: None,
        };
        config.save(&mana_dir).unwrap();

        (dir, mana_dir)
    }

    #[test]
    fn adopt_single_bean() {
        let (_dir, mana_dir) = setup_beans_dir_with_config();

        // Create parent unit
        let mut parent = Unit::new("1", "Parent task");
        parent.slug = Some("parent-task".to_string());
        parent.acceptance = Some("Children complete".to_string());
        parent.to_file(mana_dir.join("1-parent-task.md")).unwrap();

        // Create child unit
        let mut child = Unit::new("2", "Child task");
        child.slug = Some("child-task".to_string());
        child.verify = Some("cargo test".to_string());
        child.to_file(mana_dir.join("2-child-task.md")).unwrap();

        // Adopt
        let result = cmd_adopt(&mana_dir, "1", &["2".to_string()]).unwrap();

        // Verify mapping
        assert_eq!(result.get("2"), Some(&"1.1".to_string()));

        // Verify old file is gone
        assert!(!mana_dir.join("2-child-task.md").exists());

        // Verify new file exists
        assert!(mana_dir.join("1.1-child-task.md").exists());

        // Verify unit content
        let adopted = Unit::from_file(mana_dir.join("1.1-child-task.md")).unwrap();
        assert_eq!(adopted.id, "1.1");
        assert_eq!(adopted.parent, Some("1".to_string()));
        assert_eq!(adopted.title, "Child task");
    }

    #[test]
    fn adopt_multiple_beans() {
        let (_dir, mana_dir) = setup_beans_dir_with_config();

        // Create parent
        let mut parent = Unit::new("1", "Parent");
        parent.slug = Some("parent".to_string());
        parent.acceptance = Some("All done".to_string());
        parent.to_file(mana_dir.join("1-parent.md")).unwrap();

        // Create children
        for i in 2..=4 {
            let mut child = Unit::new(i.to_string(), format!("Child {}", i));
            child.slug = Some(format!("child-{}", i));
            child.verify = Some("true".to_string());
            child
                .to_file(mana_dir.join(format!("{}-child-{}.md", i, i)))
                .unwrap();
        }

        // Adopt all three
        let result = cmd_adopt(
            &mana_dir,
            "1",
            &["2".to_string(), "3".to_string(), "4".to_string()],
        )
        .unwrap();

        // Verify mappings (should be sequential)
        assert_eq!(result.get("2"), Some(&"1.1".to_string()));
        assert_eq!(result.get("3"), Some(&"1.2".to_string()));
        assert_eq!(result.get("4"), Some(&"1.3".to_string()));

        // Verify files
        assert!(mana_dir.join("1.1-child-2.md").exists());
        assert!(mana_dir.join("1.2-child-3.md").exists());
        assert!(mana_dir.join("1.3-child-4.md").exists());
    }

    #[test]
    fn adopt_with_existing_children() {
        let (_dir, mana_dir) = setup_beans_dir_with_config();

        // Create parent with existing child
        let mut parent = Unit::new("1", "Parent");
        parent.slug = Some("parent".to_string());
        parent.acceptance = Some("Done".to_string());
        parent.to_file(mana_dir.join("1-parent.md")).unwrap();

        let mut existing_child = Unit::new("1.1", "Existing child");
        existing_child.slug = Some("existing-child".to_string());
        existing_child.parent = Some("1".to_string());
        existing_child.verify = Some("true".to_string());
        existing_child
            .to_file(mana_dir.join("1.1-existing-child.md"))
            .unwrap();

        // Create new unit to adopt
        let mut new_bean = Unit::new("5", "New unit");
        new_bean.slug = Some("new-unit".to_string());
        new_bean.verify = Some("true".to_string());
        new_bean.to_file(mana_dir.join("5-new-unit.md")).unwrap();

        // Adopt - should get 1.2, not 1.1
        let result = cmd_adopt(&mana_dir, "1", &["5".to_string()]).unwrap();

        assert_eq!(result.get("5"), Some(&"1.2".to_string()));
        assert!(mana_dir.join("1.2-new-unit.md").exists());
    }

    #[test]
    fn adopt_updates_dependencies() {
        let (_dir, mana_dir) = setup_beans_dir_with_config();

        // Create parent
        let mut parent = Unit::new("1", "Parent");
        parent.slug = Some("parent".to_string());
        parent.acceptance = Some("Done".to_string());
        parent.to_file(mana_dir.join("1-parent.md")).unwrap();

        // Create unit to adopt
        let mut to_adopt = Unit::new("2", "To adopt");
        to_adopt.slug = Some("to-adopt".to_string());
        to_adopt.verify = Some("true".to_string());
        to_adopt.to_file(mana_dir.join("2-to-adopt.md")).unwrap();

        // Create unit that depends on the one being adopted
        let mut dependent = Unit::new("3", "Dependent");
        dependent.slug = Some("dependent".to_string());
        dependent.verify = Some("true".to_string());
        dependent.dependencies = vec!["2".to_string()];
        dependent.to_file(mana_dir.join("3-dependent.md")).unwrap();

        // Adopt unit 2 under parent 1
        cmd_adopt(&mana_dir, "1", &["2".to_string()]).unwrap();

        // Verify dependent unit's dependencies were updated
        let dependent_updated = Unit::from_file(mana_dir.join("3-dependent.md")).unwrap();
        assert_eq!(dependent_updated.dependencies, vec!["1.1".to_string()]);
    }

    #[test]
    fn adopt_fails_for_missing_parent() {
        let (_dir, mana_dir) = setup_beans_dir_with_config();

        // Create only the child, no parent
        let mut child = Unit::new("2", "Child");
        child.slug = Some("child".to_string());
        child.verify = Some("true".to_string());
        child.to_file(mana_dir.join("2-child.md")).unwrap();

        // Try to adopt under non-existent parent
        let result = cmd_adopt(&mana_dir, "99", &["2".to_string()]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Parent unit '99' not found"));
    }

    #[test]
    fn adopt_fails_for_missing_child() {
        let (_dir, mana_dir) = setup_beans_dir_with_config();

        // Create only the parent
        let mut parent = Unit::new("1", "Parent");
        parent.slug = Some("parent".to_string());
        parent.acceptance = Some("Done".to_string());
        parent.to_file(mana_dir.join("1-parent.md")).unwrap();

        // Try to adopt non-existent child
        let result = cmd_adopt(&mana_dir, "1", &["99".to_string()]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Child unit '99' not found"));
    }

    #[test]
    fn adopt_rebuilds_index() {
        let (_dir, mana_dir) = setup_beans_dir_with_config();

        // Create parent and child
        let mut parent = Unit::new("1", "Parent");
        parent.slug = Some("parent".to_string());
        parent.acceptance = Some("Done".to_string());
        parent.to_file(mana_dir.join("1-parent.md")).unwrap();

        let mut child = Unit::new("2", "Child");
        child.slug = Some("child".to_string());
        child.verify = Some("true".to_string());
        child.to_file(mana_dir.join("2-child.md")).unwrap();

        // Adopt
        cmd_adopt(&mana_dir, "1", &["2".to_string()]).unwrap();

        // Load index and verify
        let index = Index::load(&mana_dir).unwrap();

        // Should have 2 units: parent (1) and adopted child (1.1)
        assert_eq!(index.units.len(), 2);

        // Find the adopted unit in the index
        let adopted = index.units.iter().find(|b| b.id == "1.1");
        assert!(adopted.is_some());
        assert_eq!(adopted.unwrap().parent, Some("1".to_string()));

        // Old ID should not be in index
        assert!(!index.units.iter().any(|b| b.id == "2"));
    }

    #[test]
    fn next_child_number_empty() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let num = next_child_number(&mana_dir, "1").unwrap();
        assert_eq!(num, 1);
    }

    #[test]
    fn next_child_number_with_existing() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Create existing children
        fs::write(mana_dir.join("1.1-child-one.md"), "test").unwrap();
        fs::write(mana_dir.join("1.2-child-two.md"), "test").unwrap();
        fs::write(mana_dir.join("1.5-child-five.md"), "test").unwrap();

        let num = next_child_number(&mana_dir, "1").unwrap();
        assert_eq!(num, 6); // Next after 5
    }

    #[test]
    fn next_child_number_ignores_other_parents() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Create children under different parents
        fs::write(mana_dir.join("1.1-child.md"), "test").unwrap();
        fs::write(mana_dir.join("2.1-child.md"), "test").unwrap();
        fs::write(mana_dir.join("2.2-child.md"), "test").unwrap();

        // Should only count children of parent "1"
        let num = next_child_number(&mana_dir, "1").unwrap();
        assert_eq!(num, 2);
    }
}
