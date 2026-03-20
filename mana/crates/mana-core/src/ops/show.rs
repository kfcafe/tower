use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::unit::Unit;
use crate::discovery::find_unit_file;

/// Result of loading a unit.
pub struct GetResult {
    pub unit: Unit,
    pub path: PathBuf,
}

/// Load a unit by ID and return its full data.
pub fn get(mana_dir: &Path, id: &str) -> Result<GetResult> {
    let bean_path = find_unit_file(mana_dir, id)
        .with_context(|| format!("Unit not found: {}", id))?;
    let unit = Unit::from_file(&bean_path)
        .with_context(|| format!("Failed to load unit: {}", id))?;
    Ok(GetResult { unit, path: bean_path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::create::{self, tests::minimal_params};
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let bd = dir.path().join(".mana");
        fs::create_dir(&bd).unwrap();
        crate::config::Config {
            project: "test".to_string(), next_id: 1, auto_close_parent: true,
            run: None, plan: None, max_loops: 10, max_concurrent: 4,
            poll_interval: 30, extends: vec![], rules_file: None,
            file_locking: false, worktree: false, on_close: None,
            on_fail: None, post_plan: None, verify_timeout: None,
            review: None, user: None, user_email: None, auto_commit: false,
        }.save(&bd).unwrap();
        (dir, bd)
    }

    #[test]
    fn get_existing() {
        let (_dir, bd) = setup();
        create::create(&bd, minimal_params("My task")).unwrap();
        let r = get(&bd, "1").unwrap();
        assert_eq!(r.unit.title, "My task");
        assert!(r.path.exists());
    }

    #[test]
    fn get_nonexistent() {
        let (_dir, bd) = setup();
        assert!(get(&bd, "99").is_err());
    }
}
