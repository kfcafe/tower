use std::path::Path;

use anyhow::Result;

use crate::index::{Index, IndexEntry};
use crate::unit::Status;
use crate::util::parse_status;

/// Parameters for listing/filtering units.
#[derive(Default)]
pub struct ListParams {
    pub status: Option<String>,
    pub priority: Option<u8>,
    pub parent: Option<String>,
    pub label: Option<String>,
    pub assignee: Option<String>,
    pub current_user: Option<String>,
    pub include_closed: bool,
}

/// Load the index, apply filters, and return matching entries.
pub fn list(mana_dir: &Path, params: &ListParams) -> Result<Vec<IndexEntry>> {
    let index = Index::load_or_rebuild(mana_dir)?;
    let status_filter = params.status.as_deref().and_then(parse_status);

    let mut entries = index.units.clone();

    if status_filter == Some(Status::Closed) || params.include_closed {
        if let Ok(archived) = Index::collect_archived(mana_dir) {
            entries.extend(archived);
        }
    }

    entries.retain(|entry| {
        if !params.include_closed
            && status_filter != Some(Status::Closed)
            && entry.status == Status::Closed
        {
            return false;
        }
        if let Some(s) = status_filter {
            if entry.status != s {
                return false;
            }
        }
        if let Some(p) = params.priority {
            if entry.priority != p {
                return false;
            }
        }
        if let Some(ref parent) = params.parent {
            if entry.parent.as_deref() != Some(parent.as_str()) {
                return false;
            }
        }
        if let Some(ref label) = params.label {
            if !entry.labels.contains(label) {
                return false;
            }
        }
        if let Some(ref assignee) = params.assignee {
            if entry.assignee.as_deref() != Some(assignee.as_str()) {
                return false;
            }
        }
        if let Some(ref user) = params.current_user {
            let claimed_match = entry
                .claimed_by
                .as_ref()
                .is_some_and(|c| c == user || c.starts_with(&format!("{}/", user)));
            let assignee_match = entry.assignee.as_deref() == Some(user.as_str());
            if !claimed_match && !assignee_match {
                return false;
            }
        }
        true
    });

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::create::{self, tests::minimal_params};
    use crate::ops::update;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let bd = dir.path().join(".mana");
        fs::create_dir(&bd).unwrap();
        crate::config::Config {
            project: "test".to_string(),
            next_id: 1,
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
            commit_template: None,
            research: None,
            run_model: None,
            plan_model: None,
            review_model: None,
            research_model: None,
            batch_verify: false,
            memory_reserve_mb: 0,
            notify: None,
        }
        .save(&bd)
        .unwrap();
        (dir, bd)
    }

    #[test]
    fn list_all() {
        let (_dir, bd) = setup();
        create::create(&bd, minimal_params("A")).unwrap();
        create::create(&bd, minimal_params("B")).unwrap();
        assert_eq!(list(&bd, &ListParams::default()).unwrap().len(), 2);
    }

    #[test]
    fn list_excludes_closed() {
        let (_dir, bd) = setup();
        create::create(&bd, minimal_params("Open")).unwrap();
        create::create(&bd, minimal_params("Closed")).unwrap();
        update::update(
            &bd,
            "2",
            update::UpdateParams {
                title: None,
                description: None,
                acceptance: None,
                notes: None,
                design: None,
                status: Some("closed".into()),
                priority: None,
                assignee: None,
                add_label: None,
                remove_label: None,
                decisions: vec![],
                resolve_decisions: vec![],
            },
        )
        .unwrap();
        let entries = list(&bd, &ListParams::default()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "1");
    }

    #[test]
    fn list_filter_priority() {
        let (_dir, bd) = setup();
        let mut p0 = minimal_params("Urgent");
        p0.priority = Some(0);
        create::create(&bd, p0).unwrap();
        create::create(&bd, minimal_params("Normal")).unwrap();
        let entries = list(
            &bd,
            &ListParams {
                priority: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Urgent");
    }

    #[test]
    fn list_filter_parent() {
        let (_dir, bd) = setup();
        create::create(&bd, minimal_params("Parent")).unwrap();
        let mut child = minimal_params("Child");
        child.parent = Some("1".to_string());
        create::create(&bd, child).unwrap();
        let entries = list(
            &bd,
            &ListParams {
                parent: Some("1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "1.1");
    }

    #[test]
    fn list_filter_assignee() {
        let (_dir, bd) = setup();
        let mut alice = minimal_params("Alice");
        alice.assignee = Some("alice".to_string());
        create::create(&bd, alice).unwrap();
        let mut bob = minimal_params("Bob");
        bob.assignee = Some("bob".to_string());
        create::create(&bd, bob).unwrap();

        let entries = list(
            &bd,
            &ListParams {
                assignee: Some("alice".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Alice");
    }

    #[test]
    fn list_filter_current_user_matches_claimed_or_assigned() {
        let (_dir, bd) = setup();
        let mut claimed = minimal_params("Claimed");
        claimed.assignee = Some("other".to_string());
        create::create(&bd, claimed).unwrap();
        let mut assigned = minimal_params("Assigned");
        assigned.assignee = Some("alice".to_string());
        create::create(&bd, assigned).unwrap();

        let first_path = crate::discovery::find_unit_file(&bd, "1").unwrap();
        let mut first_unit = crate::unit::Unit::from_file(&first_path).unwrap();
        first_unit.claimed_by = Some("alice/session".to_string());
        first_unit.to_file(&first_path).unwrap();
        let index = Index::build(&bd).unwrap();
        index.save(&bd).unwrap();

        let entries = list(
            &bd,
            &ListParams {
                current_user: Some("alice".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(entries.len(), 2);
    }
}
