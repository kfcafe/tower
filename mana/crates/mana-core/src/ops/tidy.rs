use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::unit::{Unit, Status};
use crate::discovery::{archive_path_for_bean, find_unit_file};
use crate::index::{ArchiveIndex, Index};
use crate::util::title_to_slug;

/// A record of one unit that was archived during tidy.
#[derive(Debug, Clone)]
pub struct TidiedBean {
    pub id: String,
    pub title: String,
    pub archive_path: String,
}

/// A record of one unit that was released during tidy.
#[derive(Debug, Clone)]
pub struct ReleasedBean {
    pub id: String,
    pub title: String,
    pub reason: String,
}

/// Result of a tidy operation.
pub struct TidyResult {
    pub tidied: Vec<TidiedBean>,
    pub released: Vec<ReleasedBean>,
    pub skipped_parent_ids: Vec<String>,
    pub index_count: usize,
    pub agents_running: bool,
    pub in_progress_count: usize,
}

/// Format a chrono Duration as a human-readable string like "3 days ago".
fn format_duration(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds();
    if secs < 0 {
        return "just now".to_string();
    }
    let minutes = secs / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("claimed {} day(s) ago", days)
    } else if hours > 0 {
        format!("claimed {} hour(s) ago", hours)
    } else if minutes > 0 {
        format!("claimed {} minute(s) ago", minutes)
    } else {
        "claimed just now".to_string()
    }
}

/// Tidy the units directory: archive closed units, release stale in-progress
/// units, and rebuild the index.
///
/// The `check_agents` function is injectable for testability. It returns true
/// if agent processes are currently running.
///
/// With `dry_run = true`, reports what would change without touching any files.
pub fn tidy(
    mana_dir: &Path,
    dry_run: bool,
    check_agents: fn() -> bool,
) -> Result<TidyResult> {
    let index = Index::build(mana_dir).context("Failed to build index")?;

    let closed: Vec<&crate::index::IndexEntry> = index
        .units
        .iter()
        .filter(|entry| entry.status == Status::Closed)
        .collect();

    let mut tidied: Vec<TidiedBean> = Vec::new();
    let mut skipped_parent_ids: Vec<String> = Vec::new();

    for entry in &closed {
        let bean_path = match find_unit_file(mana_dir, &entry.id) {
            Ok(path) => path,
            Err(_) => continue,
        };

        let mut unit = Unit::from_file(&bean_path)
            .with_context(|| format!("Failed to load unit: {}", entry.id))?;

        if unit.is_archived {
            continue;
        }

        let has_open_children = index
            .units
            .iter()
            .any(|b| b.parent.as_deref() == Some(entry.id.as_str()) && b.status != Status::Closed);

        if has_open_children {
            skipped_parent_ids.push(entry.id.clone());
            continue;
        }

        let archive_date = unit
            .closed_at
            .unwrap_or(unit.updated_at)
            .with_timezone(&chrono::Local)
            .date_naive();

        let slug = unit
            .slug
            .clone()
            .unwrap_or_else(|| title_to_slug(&unit.title));
        let ext = bean_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("md");
        let archive_path = archive_path_for_bean(mana_dir, &entry.id, &slug, ext, archive_date);

        let relative = archive_path
            .strip_prefix(mana_dir)
            .unwrap_or(&archive_path);
        tidied.push(TidiedBean {
            id: entry.id.clone(),
            title: entry.title.clone(),
            archive_path: relative.display().to_string(),
        });

        if dry_run {
            continue;
        }

        if let Some(parent) = archive_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create archive directory for unit {}", entry.id)
            })?;
        }

        std::fs::rename(&bean_path, &archive_path)
            .with_context(|| format!("Failed to move unit {} to archive", entry.id))?;

        unit.is_archived = true;
        unit.to_file(&archive_path)
            .with_context(|| format!("Failed to save archived unit: {}", entry.id))?;
    }

    // Release stale in-progress units
    let in_progress: Vec<&crate::index::IndexEntry> = index
        .units
        .iter()
        .filter(|entry| entry.status == Status::InProgress)
        .collect();

    let mut released: Vec<ReleasedBean> = Vec::new();
    let agents_running;
    let in_progress_count = in_progress.len();

    if !in_progress.is_empty() {
        agents_running = check_agents();

        if !agents_running {
            for entry in &in_progress {
                let bean_path = match find_unit_file(mana_dir, &entry.id) {
                    Ok(path) => path,
                    Err(_) => continue,
                };

                let mut unit = match Unit::from_file(&bean_path) {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                let reason = if let Some(claimed_at) = unit.claimed_at {
                    let age = Utc::now().signed_duration_since(claimed_at);
                    format_duration(age)
                } else {
                    "never properly claimed".to_string()
                };

                released.push(ReleasedBean {
                    id: entry.id.clone(),
                    title: entry.title.clone(),
                    reason,
                });

                if dry_run {
                    continue;
                }

                let now = Utc::now();
                unit.status = Status::Open;
                unit.claimed_by = None;
                unit.claimed_at = None;
                unit.updated_at = now;

                unit.to_file(&bean_path)
                    .with_context(|| format!("Failed to release stale unit: {}", entry.id))?;
            }
        }
    } else {
        agents_running = false;
    }

    // Rebuild the index
    let final_index = Index::build(mana_dir).context("Failed to rebuild index after tidy")?;
    final_index
        .save(mana_dir)
        .context("Failed to save index")?;

    // Rebuild archive index if units were archived
    if !dry_run && !tidied.is_empty() {
        let archive_index =
            ArchiveIndex::build(mana_dir).context("Failed to rebuild archive index after tidy")?;
        archive_index
            .save(mana_dir)
            .context("Failed to save archive index")?;
    }

    Ok(TidyResult {
        tidied,
        released,
        skipped_parent_ids,
        index_count: final_index.units.len(),
        agents_running,
        in_progress_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    fn no_agents() -> bool {
        false
    }

    fn agents_running_fn() -> bool {
        true
    }

    fn write_bean(mana_dir: &Path, unit: &Unit) {
        let slug = title_to_slug(&unit.title);
        let path = mana_dir.join(format!("{}-{}.md", unit.id, slug));
        unit.to_file(path).unwrap();
    }

    #[test]
    fn tidy_archives_closed_beans() {
        let (_dir, mana_dir) = setup();

        let mut unit = Unit::new("1", "Done task");
        unit.status = Status::Closed;
        unit.closed_at = Some(chrono::Utc::now());
        write_bean(&mana_dir, &unit);

        let result = tidy(&mana_dir, false, no_agents).unwrap();

        assert_eq!(result.tidied.len(), 1);
        assert_eq!(result.tidied[0].id, "1");
        assert!(find_unit_file(&mana_dir, "1").is_err());
        let archived = crate::discovery::find_archived_unit(&mana_dir, "1");
        assert!(archived.is_ok());
    }

    #[test]
    fn tidy_leaves_open_beans_alone() {
        let (_dir, mana_dir) = setup();

        let unit = Unit::new("1", "Open task");
        write_bean(&mana_dir, &unit);

        let result = tidy(&mana_dir, false, no_agents).unwrap();

        assert!(result.tidied.is_empty());
        assert!(find_unit_file(&mana_dir, "1").is_ok());
    }

    #[test]
    fn tidy_dry_run_does_not_move_files() {
        let (_dir, mana_dir) = setup();

        let mut unit = Unit::new("1", "Done task");
        unit.status = Status::Closed;
        unit.closed_at = Some(chrono::Utc::now());
        write_bean(&mana_dir, &unit);

        let result = tidy(&mana_dir, true, no_agents).unwrap();

        assert_eq!(result.tidied.len(), 1);
        assert!(find_unit_file(&mana_dir, "1").is_ok());
    }

    #[test]
    fn tidy_skips_closed_parent_with_open_children() {
        let (_dir, mana_dir) = setup();

        let mut parent = Unit::new("1", "Parent");
        parent.status = Status::Closed;
        parent.closed_at = Some(chrono::Utc::now());
        write_bean(&mana_dir, &parent);

        let mut child = Unit::new("1.1", "Child");
        child.parent = Some("1".to_string());
        write_bean(&mana_dir, &child);

        let result = tidy(&mana_dir, false, no_agents).unwrap();

        assert!(result.tidied.is_empty());
        assert_eq!(result.skipped_parent_ids, vec!["1"]);
        assert!(find_unit_file(&mana_dir, "1").is_ok());
    }

    #[test]
    fn tidy_releases_stale_in_progress_beans() {
        let (_dir, mana_dir) = setup();

        let mut unit = Unit::new("1", "Stale WIP");
        unit.status = Status::InProgress;
        unit.claimed_at = Some(
            chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        );
        write_bean(&mana_dir, &unit);

        let result = tidy(&mana_dir, false, no_agents).unwrap();

        assert_eq!(result.released.len(), 1);
        let updated = Unit::from_file(find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.status, Status::Open);
        assert!(updated.claimed_by.is_none());
    }

    #[test]
    fn tidy_skips_in_progress_when_agents_running() {
        let (_dir, mana_dir) = setup();

        let mut unit = Unit::new("1", "Active WIP");
        unit.status = Status::InProgress;
        unit.claimed_at = Some(chrono::Utc::now());
        write_bean(&mana_dir, &unit);

        let result = tidy(&mana_dir, false, agents_running_fn).unwrap();

        assert!(result.released.is_empty());
        assert!(result.agents_running);
        let updated = Unit::from_file(find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.status, Status::InProgress);
    }

    #[test]
    fn tidy_empty_project() {
        let (_dir, mana_dir) = setup();
        let result = tidy(&mana_dir, false, no_agents).unwrap();
        assert!(result.tidied.is_empty());
        assert!(result.released.is_empty());
    }
}
