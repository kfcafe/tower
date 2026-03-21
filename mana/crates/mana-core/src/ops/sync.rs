use std::path::Path;

use anyhow::Result;

use crate::index::{count_bean_formats, ArchiveIndex, Index};

/// Result of a sync (index rebuild) operation.
pub struct SyncResult {
    pub bean_count: usize,
    pub archive_count: usize,
    pub md_count: usize,
    pub yaml_count: usize,
    pub mixed_formats: bool,
}

/// Rebuild index from unit files on disk.
///
/// Force rebuilds both the main index and archive index unconditionally.
/// Returns structured counts so callers can format output.
pub fn sync(mana_dir: &Path) -> Result<SyncResult> {
    let (md_count, yaml_count) = count_bean_formats(mana_dir)?;

    let index = Index::build(mana_dir)?;
    let bean_count = index.units.len();
    index.save(mana_dir)?;

    let archive_index = ArchiveIndex::build(mana_dir)?;
    let archive_count = archive_index.units.len();
    if archive_count > 0 || mana_dir.join("archive.yaml").exists() {
        archive_index.save(mana_dir)?;
    }

    Ok(SyncResult {
        bean_count,
        archive_count,
        md_count,
        yaml_count,
        mixed_formats: md_count > 0 && yaml_count > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::{Status, Unit};
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn sync_rebuilds_index() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let bean1 = Unit::new("1", "Task one");
        let bean2 = Unit::new("2", "Task two");
        let slug1 = title_to_slug(&bean1.title);
        let slug2 = title_to_slug(&bean2.title);
        bean1
            .to_file(mana_dir.join(format!("1-{}.md", slug1)))
            .unwrap();
        bean2
            .to_file(mana_dir.join(format!("2-{}.md", slug2)))
            .unwrap();

        let result = sync(&mana_dir).unwrap();

        assert_eq!(result.bean_count, 2);
        assert!(!result.mixed_formats);
    }

    #[test]
    fn sync_empty_project() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let result = sync(&mana_dir).unwrap();

        assert_eq!(result.bean_count, 0);
        assert_eq!(result.archive_count, 0);
    }

    #[test]
    fn sync_rebuilds_archive() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let archive_dir = mana_dir.join("archive").join("2026").join("03");
        fs::create_dir_all(&archive_dir).unwrap();

        let mut unit = Unit::new("10", "Archived ten");
        unit.status = Status::Closed;
        unit.is_archived = true;
        let slug = title_to_slug(&unit.title);
        unit.to_file(archive_dir.join(format!("10-{}.md", slug)))
            .unwrap();

        let result = sync(&mana_dir).unwrap();

        assert_eq!(result.archive_count, 1);
        assert!(mana_dir.join("archive.yaml").exists());
    }
}
