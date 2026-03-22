use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::discovery::find_unit_file;
use crate::index::Index;
use crate::unit::Unit;

/// Reopen a closed unit.
///
/// Sets status=open, clears closed_at and close_reason.
/// Updates updated_at and rebuilds index.
pub fn cmd_reopen(mana_dir: &Path, id: &str) -> Result<()> {
    let unit_path =
        find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;

    let mut unit =
        Unit::from_file(&unit_path).with_context(|| format!("Failed to load unit: {}", id))?;

    unit.status = crate::unit::Status::Open;
    unit.closed_at = None;
    unit.close_reason = None;
    unit.updated_at = Utc::now();

    unit.to_file(&unit_path)
        .with_context(|| format!("Failed to save unit: {}", id))?;

    // Rebuild index
    let index = Index::build(mana_dir).with_context(|| "Failed to rebuild index")?;
    index
        .save(mana_dir)
        .with_context(|| "Failed to save index")?;

    println!("Reopened unit {}: {}", id, unit.title);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Status;
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_mana_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn test_reopen_closed_unit() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.status = Status::Closed;
        unit.closed_at = Some(Utc::now());
        unit.close_reason = Some("Done".to_string());
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_reopen(&mana_dir, "1").unwrap();

        let reopened =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(reopened.status, Status::Open);
        assert!(reopened.closed_at.is_none());
        assert!(reopened.close_reason.is_none());
    }

    #[test]
    fn test_reopen_nonexistent_unit() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let result = cmd_reopen(&mana_dir, "99");
        assert!(result.is_err());
    }

    #[test]
    fn test_reopen_updates_updated_at() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.status = Status::Closed;
        unit.closed_at = Some(Utc::now());
        let original_updated_at = unit.updated_at;
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        cmd_reopen(&mana_dir, "1").unwrap();

        let reopened =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert!(reopened.updated_at > original_updated_at);
    }

    #[test]
    fn test_reopen_rebuilds_index() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let mut unit = Unit::new("1", "Task");
        unit.status = Status::Closed;
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_reopen(&mana_dir, "1").unwrap();

        let index = Index::load(&mana_dir).unwrap();
        let entry = index.units.iter().find(|e| e.id == "1").unwrap();
        assert_eq!(entry.status, Status::Open);
    }

    #[test]
    fn test_reopen_open_unit() {
        let (_dir, mana_dir) = setup_test_mana_dir();
        let unit = Unit::new("1", "Task");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Should work fine even if already open
        cmd_reopen(&mana_dir, "1").unwrap();

        let reopened =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(reopened.status, Status::Open);
    }
}
