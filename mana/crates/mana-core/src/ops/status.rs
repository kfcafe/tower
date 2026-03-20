use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::unit::Status;
use crate::blocking::check_blocked;
use crate::index::{Index, IndexEntry};
use crate::util::natural_cmp;

/// Categorized view of project status.
#[derive(Debug, Serialize)]
pub struct StatusSummary {
    pub features: Vec<IndexEntry>,
    pub claimed: Vec<IndexEntry>,
    pub ready: Vec<IndexEntry>,
    pub goals: Vec<IndexEntry>,
    pub blocked: Vec<BlockedEntry>,
}

/// An entry that is blocked with its reason.
#[derive(Debug, Serialize)]
pub struct BlockedEntry {
    #[serde(flatten)]
    pub entry: IndexEntry,
    pub block_reason: String,
}

/// Compute the project status summary: categorize units into claimed, ready,
/// goals (need decomposition), and blocked.
pub fn status(mana_dir: &Path) -> Result<StatusSummary> {
    let index = Index::load_or_rebuild(mana_dir)?;

    let mut features: Vec<IndexEntry> = Vec::new();
    let mut claimed: Vec<IndexEntry> = Vec::new();
    let mut ready: Vec<IndexEntry> = Vec::new();
    let mut goals: Vec<IndexEntry> = Vec::new();
    let mut blocked: Vec<BlockedEntry> = Vec::new();

    for entry in &index.units {
        if entry.feature {
            features.push(entry.clone());
            continue;
        }
        match entry.status {
            Status::InProgress => {
                claimed.push(entry.clone());
            }
            Status::Open => {
                if let Some(reason) = check_blocked(entry, &index) {
                    blocked.push(BlockedEntry {
                        entry: entry.clone(),
                        block_reason: reason.to_string(),
                    });
                } else if entry.has_verify {
                    ready.push(entry.clone());
                } else {
                    goals.push(entry.clone());
                }
            }
            Status::Closed => {}
        }
    }

    sort_entries(&mut features);
    sort_entries(&mut claimed);
    sort_entries(&mut ready);
    sort_entries(&mut goals);
    blocked.sort_by(|a, b| match a.entry.priority.cmp(&b.entry.priority) {
        std::cmp::Ordering::Equal => natural_cmp(&a.entry.id, &b.entry.id),
        other => other,
    });

    Ok(StatusSummary {
        features,
        claimed,
        ready,
        goals,
        blocked,
    })
}

fn sort_entries(entries: &mut [IndexEntry]) {
    entries.sort_by(|a, b| match a.priority.cmp(&b.priority) {
        std::cmp::Ordering::Equal => natural_cmp(&a.id, &b.id),
        other => other,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    fn write_bean(mana_dir: &Path, unit: &Unit) {
        let slug = title_to_slug(&unit.title);
        let path = mana_dir.join(format!("{}-{}.md", unit.id, slug));
        unit.to_file(path).unwrap();
    }

    #[test]
    fn status_categorizes_beans() {
        let (_dir, mana_dir) = setup();

        // Open unit with verify -> ready
        let mut ready_bean = Unit::new("1", "Ready task");
        ready_bean.verify = Some("cargo test".to_string());
        write_bean(&mana_dir, &ready_bean);

        // Open unit without verify -> goals
        let goal_bean = Unit::new("2", "Goal task");
        write_bean(&mana_dir, &goal_bean);

        // In progress -> claimed
        let mut claimed_bean = Unit::new("3", "Claimed task");
        claimed_bean.status = Status::InProgress;
        write_bean(&mana_dir, &claimed_bean);

        let result = status(&mana_dir).unwrap();

        assert_eq!(result.ready.len(), 1);
        assert_eq!(result.ready[0].id, "1");

        assert_eq!(result.goals.len(), 1);
        assert_eq!(result.goals[0].id, "2");

        assert_eq!(result.claimed.len(), 1);
        assert_eq!(result.claimed[0].id, "3");

        assert!(result.blocked.is_empty());
    }

    #[test]
    fn status_detects_blocked() {
        let (_dir, mana_dir) = setup();

        // Create a dependency that's still open
        let mut dep = Unit::new("1", "Dependency");
        dep.verify = Some("true".to_string());
        write_bean(&mana_dir, &dep);

        // Create unit depending on the open dep
        let mut blocked_bean = Unit::new("2", "Blocked task");
        blocked_bean.verify = Some("true".to_string());
        blocked_bean.dependencies = vec!["1".to_string()];
        write_bean(&mana_dir, &blocked_bean);

        let result = status(&mana_dir).unwrap();

        assert_eq!(result.blocked.len(), 1);
        assert_eq!(result.blocked[0].entry.id, "2");
    }

    #[test]
    fn status_empty_project() {
        let (_dir, mana_dir) = setup();

        let result = status(&mana_dir).unwrap();

        assert!(result.features.is_empty());
        assert!(result.claimed.is_empty());
        assert!(result.ready.is_empty());
        assert!(result.goals.is_empty());
        assert!(result.blocked.is_empty());
    }

    #[test]
    fn status_skips_closed() {
        let (_dir, mana_dir) = setup();

        let mut unit = Unit::new("1", "Closed task");
        unit.status = Status::Closed;
        write_bean(&mana_dir, &unit);

        let result = status(&mana_dir).unwrap();

        assert!(result.claimed.is_empty());
        assert!(result.ready.is_empty());
        assert!(result.goals.is_empty());
        assert!(result.blocked.is_empty());
    }
}
