use std::path::Path;

use anyhow::Result;

use crate::index::Index;
use crate::unit::Status;
use crate::util::natural_cmp;

/// Show hierarchical tree of units with status indicators
/// If id provided: show subtree rooted at that unit
/// If no id: show full project tree
pub fn cmd_tree(mana_dir: &Path, id: Option<&str>) -> Result<()> {
    let index = Index::load_or_rebuild(mana_dir)?;

    if let Some(unit_id) = id {
        // Show subtree rooted at unit_id
        print_subtree(&index, unit_id)?;
    } else {
        // Show full project tree
        print_full_tree(&index);
    }

    Ok(())
}

fn print_full_tree(index: &Index) {
    // Find root units (those with no parent)
    let root_units: Vec<_> = index.units.iter().filter(|e| e.parent.is_none()).collect();

    if root_units.is_empty() {
        println!("No units found.");
        return;
    }

    let mut visited = std::collections::HashSet::new();
    for root in root_units {
        print_tree_node(index, &root.id, "", &mut visited);
    }
}

fn print_subtree(index: &Index, unit_id: &str) -> Result<()> {
    let _entry = index
        .units
        .iter()
        .find(|e| e.id == unit_id)
        .ok_or_else(|| anyhow::anyhow!("Unit {} not found", unit_id))?;

    let mut visited = std::collections::HashSet::new();
    print_tree_node(index, unit_id, "", &mut visited);

    Ok(())
}

fn print_tree_node(
    index: &Index,
    unit_id: &str,
    prefix: &str,
    visited: &mut std::collections::HashSet<String>,
) {
    if visited.contains(unit_id) {
        return;
    }
    visited.insert(unit_id.to_string());

    // Find the unit
    if let Some(entry) = index.units.iter().find(|e| e.id == unit_id) {
        let status_indicator = match entry.status {
            Status::Open => "[ ]",
            Status::InProgress | Status::AwaitingVerify => "[-]",
            Status::Closed => "[x]",
        };

        println!(
            "{}{} {} {}",
            prefix, status_indicator, entry.id, entry.title
        );
    } else {
        println!("{}[!] {}", prefix, unit_id);
        return;
    }

    // Find children (units with this unit as parent)
    let children: Vec<_> = index
        .units
        .iter()
        .filter(|e| e.parent.as_ref() == Some(&unit_id.to_string()))
        .collect();

    // Also find dependents (units that depend on this one)
    let dependents: Vec<_> = index
        .units
        .iter()
        .filter(|e| e.dependencies.contains(&unit_id.to_string()))
        .collect();

    // Combine and deduplicate
    let mut all_children = children;
    for dep in dependents {
        if !all_children.iter().any(|e| e.id == dep.id) {
            all_children.push(dep);
        }
    }

    // Sort by natural order
    all_children.sort_by(|a, b| natural_cmp(&a.id, &b.id));

    for (i, child) in all_children.iter().enumerate() {
        let is_last_child = i == all_children.len() - 1;
        let connector = if is_last_child {
            "└── "
        } else {
            "├── "
        };
        let new_prefix = if is_last_child {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        print!("{}{}", prefix, connector);
        print_tree_node(index, &child.id, &new_prefix, visited);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_units() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Create a hierarchy:
        // 1 (root)
        // ├── 1.1 (child)
        // ├── 1.2 (child)
        // 2 (root)
        // 3 (depends on 1)

        let unit1 = Unit::new("1", "Root task");
        let mut unit1_1 = Unit::new("1.1", "Subtask");
        unit1_1.parent = Some("1".to_string());
        let mut unit1_2 = Unit::new("1.2", "Another subtask");
        unit1_2.parent = Some("1".to_string());
        let unit2 = Unit::new("2", "Another root");
        let mut unit3 = Unit::new("3", "Depends on 1");
        unit3.dependencies = vec!["1".to_string()];

        unit1.to_file(mana_dir.join("1.yaml")).unwrap();
        unit1_1.to_file(mana_dir.join("1.1.yaml")).unwrap();
        unit1_2.to_file(mana_dir.join("1.2.yaml")).unwrap();
        unit2.to_file(mana_dir.join("2.yaml")).unwrap();
        unit3.to_file(mana_dir.join("3.yaml")).unwrap();

        (dir, mana_dir)
    }

    #[test]
    fn full_tree_displays() {
        let (_dir, mana_dir) = setup_test_units();
        let index = Index::load_or_rebuild(&mana_dir).unwrap();

        // Just verify no panic
        print_full_tree(&index);
    }

    #[test]
    fn subtree_works() {
        let (_dir, mana_dir) = setup_test_units();
        let index = Index::load_or_rebuild(&mana_dir).unwrap();

        // Just verify no panic
        let _ = print_subtree(&index, "1");
    }

    #[test]
    fn subtree_not_found() {
        let (_dir, mana_dir) = setup_test_units();
        let index = Index::load_or_rebuild(&mana_dir).unwrap();

        let result = print_subtree(&index, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn status_indicators() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let b1 = Unit::new("1", "Open task");
        let mut b2 = Unit::new("2", "In progress");
        b2.status = crate::unit::Status::InProgress;
        let mut b3 = Unit::new("3", "Closed");
        b3.status = crate::unit::Status::Closed;

        b1.to_file(mana_dir.join("1.yaml")).unwrap();
        b2.to_file(mana_dir.join("2.yaml")).unwrap();
        b3.to_file(mana_dir.join("3.yaml")).unwrap();

        let index = Index::load_or_rebuild(&mana_dir).unwrap();
        print_full_tree(&index);
    }
}
