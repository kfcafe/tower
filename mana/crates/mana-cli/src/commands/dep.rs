use std::path::Path;

use anyhow::Result;

use mana_core::ops::dep;

/// Add a dependency: `mana dep add <id> <depends-on-id>`
pub fn cmd_dep_add(mana_dir: &Path, id: &str, depends_on_id: &str) -> Result<()> {
    let result = dep::dep_add(mana_dir, id, depends_on_id)?;
    println!("{} now depends on {}", result.from_id, result.to_id);
    Ok(())
}

/// Remove a dependency: `mana dep remove <id> <depends-on-id>`
pub fn cmd_dep_remove(mana_dir: &Path, id: &str, depends_on_id: &str) -> Result<()> {
    let result = dep::dep_remove(mana_dir, id, depends_on_id)?;
    println!("{} no longer depends on {}", result.from_id, result.to_id);
    Ok(())
}

/// List dependencies and dependents: `mana dep list <id>`
pub fn cmd_dep_list(mana_dir: &Path, id: &str) -> Result<()> {
    let result = dep::dep_list(mana_dir, id)?;

    println!("Dependencies ({}):", result.dependencies.len());
    if result.dependencies.is_empty() {
        println!("  (none)");
    } else {
        for d in &result.dependencies {
            if d.found {
                println!("  {} {}", d.id, d.title);
            } else {
                println!("  {} (not found)", d.id);
            }
        }
    }

    println!("\nDependents ({}):", result.dependents.len());
    if result.dependents.is_empty() {
        println!("  (none)");
    } else {
        for d in &result.dependents {
            println!("  {} {}", d.id, d.title);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Index;
    use crate::unit::Unit;
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_beans_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    fn create_bean(mana_dir: &Path, unit: &Unit) {
        let slug = title_to_slug(&unit.title);
        let filename = format!("{}-{}.md", unit.id, slug);
        unit.to_file(mana_dir.join(filename)).unwrap();
    }

    #[test]
    fn test_dep_add_simple() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let bean1 = Unit::new("1", "Task 1");
        let bean2 = Unit::new("2", "Task 2");
        create_bean(&mana_dir, &bean1);
        create_bean(&mana_dir, &bean2);

        cmd_dep_add(&mana_dir, "1", "2").unwrap();

        let updated = Unit::from_file(mana_dir.join("1-task-1.md")).unwrap();
        assert_eq!(updated.dependencies, vec!["2".to_string()]);
    }

    #[test]
    fn test_dep_add_self_dependency_rejected() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let bean1 = Unit::new("1", "Task 1");
        create_bean(&mana_dir, &bean1);

        let result = cmd_dep_add(&mana_dir, "1", "1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("self-dependency"));
    }

    #[test]
    fn test_dep_add_nonexistent_bean() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let bean1 = Unit::new("1", "Task 1");
        create_bean(&mana_dir, &bean1);

        let result = cmd_dep_add(&mana_dir, "1", "999");
        assert!(result.is_err());
    }

    #[test]
    fn test_dep_add_cycle_detection() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let mut bean1 = Unit::new("1", "Task 1");
        let bean2 = Unit::new("2", "Task 2");
        bean1.dependencies = vec!["2".to_string()];
        create_bean(&mana_dir, &bean1);
        create_bean(&mana_dir, &bean2);

        Index::build(&mana_dir).unwrap().save(&mana_dir).unwrap();

        let result = cmd_dep_add(&mana_dir, "2", "1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cycle"));
    }

    #[test]
    fn test_dep_remove() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let mut bean1 = Unit::new("1", "Task 1");
        let bean2 = Unit::new("2", "Task 2");
        bean1.dependencies = vec!["2".to_string()];
        create_bean(&mana_dir, &bean1);
        create_bean(&mana_dir, &bean2);

        cmd_dep_remove(&mana_dir, "1", "2").unwrap();

        let updated = Unit::from_file(mana_dir.join("1-task-1.md")).unwrap();
        assert_eq!(updated.dependencies, Vec::<String>::new());
    }

    #[test]
    fn test_dep_remove_not_found() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let bean1 = Unit::new("1", "Task 1");
        create_bean(&mana_dir, &bean1);

        let result = cmd_dep_remove(&mana_dir, "1", "2");
        assert!(result.is_err());
    }

    #[test]
    fn test_dep_list_with_dependencies() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let mut bean1 = Unit::new("1", "Task 1");
        let bean2 = Unit::new("2", "Task 2");
        let mut bean3 = Unit::new("3", "Task 3");
        bean1.dependencies = vec!["2".to_string()];
        bean3.dependencies = vec!["1".to_string()];
        create_bean(&mana_dir, &bean1);
        create_bean(&mana_dir, &bean2);
        create_bean(&mana_dir, &bean3);

        let result = cmd_dep_list(&mana_dir, "1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_dep_add_duplicate_rejected() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let mut bean1 = Unit::new("1", "Task 1");
        let bean2 = Unit::new("2", "Task 2");
        bean1.dependencies = vec!["2".to_string()];
        create_bean(&mana_dir, &bean1);
        create_bean(&mana_dir, &bean2);

        let result = cmd_dep_add(&mana_dir, "1", "2");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already depends"));
    }
}
