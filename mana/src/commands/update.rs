use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;

use crate::unit::Unit;
use crate::discovery::find_unit_file;
use crate::hooks::{execute_hook, HookEvent};
use crate::index::Index;
use crate::util::parse_status;

/// Update a unit's fields based on provided flags.
///
/// - title, description, acceptance, design, priority, assignee, status: replace
/// - notes: append with timestamp separator
/// - labels: add/remove operations
/// - updates updated_at and rebuilds index
#[allow(clippy::too_many_arguments)]
pub fn cmd_update(
    mana_dir: &Path,
    id: &str,
    title: Option<String>,
    description: Option<String>,
    acceptance: Option<String>,
    notes: Option<String>,
    design: Option<String>,
    status: Option<String>,
    priority: Option<u8>,
    assignee: Option<String>,
    add_label: Option<String>,
    remove_label: Option<String>,
) -> Result<()> {
    // Validate priority if provided
    if let Some(p) = priority {
        crate::unit::validate_priority(p)?;
    }

    // Load the unit using find_unit_file
    let bean_path =
        find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;

    let mut unit =
        Unit::from_file(&bean_path).with_context(|| format!("Failed to load unit: {}", id))?;

    // Get project root for hooks (parent of .mana)
    let project_root = mana_dir
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine project root from units dir"))?;

    // Call pre-update hook (blocking - abort if it fails)
    let pre_passed = execute_hook(HookEvent::PreUpdate, &unit, project_root, None)
        .context("Pre-update hook execution failed")?;

    if !pre_passed {
        return Err(anyhow!("Pre-update hook rejected unit update"));
    }

    // Apply updates
    if let Some(new_title) = title {
        unit.title = new_title;
    }

    if let Some(new_description) = description {
        unit.description = Some(new_description);
    }

    if let Some(new_acceptance) = acceptance {
        unit.acceptance = Some(new_acceptance);
    }

    if let Some(new_notes) = notes {
        // Append notes with timestamp separator
        let timestamp = Utc::now().to_rfc3339();
        if let Some(existing) = unit.notes {
            unit.notes = Some(format!("{}\n\n---\n{}\n{}", existing, timestamp, new_notes));
        } else {
            unit.notes = Some(format!("---\n{}\n{}", timestamp, new_notes));
        }
    }

    if let Some(new_design) = design {
        unit.design = Some(new_design);
    }

    if let Some(new_status) = status {
        unit.status =
            parse_status(&new_status).ok_or_else(|| anyhow!("Invalid status: {}", new_status))?;
    }

    if let Some(new_priority) = priority {
        unit.priority = new_priority;
    }

    if let Some(new_assignee) = assignee {
        unit.assignee = Some(new_assignee);
    }

    if let Some(label) = add_label {
        if !unit.labels.contains(&label) {
            unit.labels.push(label);
        }
    }

    if let Some(label) = remove_label {
        unit.labels.retain(|l| l != &label);
    }

    // Update timestamp
    unit.updated_at = Utc::now();

    // Write back to the discovered path (preserves slug)
    unit.to_file(&bean_path)
        .with_context(|| format!("Failed to save unit: {}", id))?;

    // Rebuild index
    let index = Index::build(mana_dir).with_context(|| "Failed to rebuild index")?;
    index
        .save(mana_dir)
        .with_context(|| "Failed to save index")?;

    println!("Updated unit {}: {}", id, unit.title);

    // Call post-update hook (non-blocking - log warning if it fails)
    if let Err(e) = execute_hook(HookEvent::PostUpdate, &unit, project_root, None) {
        eprintln!("Warning: post-update hook failed: {}", e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Status;
    use crate::util::title_to_slug;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_beans_dir() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn test_update_title() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Original title");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.title, "New title");
    }

    #[test]
    fn test_update_notes_appends() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let mut unit = Unit::new("1", "Test");
        unit.notes = Some("First note".to_string());
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            None,
            None,
            None,
            Some("Second note".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        let notes = updated.notes.unwrap();
        assert!(notes.contains("First note"));
        assert!(notes.contains("Second note"));
        assert!(notes.contains("---"));
    }

    #[test]
    fn test_update_notes_creates_with_timestamp() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Test");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            None,
            None,
            None,
            Some("First note".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        let notes = updated.notes.unwrap();
        assert!(notes.contains("First note"));
        assert!(notes.contains("---"));
        assert!(notes.contains("T")); // ISO 8601 has T for date-time
    }

    #[test]
    fn test_update_status() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Test");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            None,
            None,
            None,
            None,
            None,
            Some("in_progress".to_string()),
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.status, Status::InProgress);
    }

    #[test]
    fn test_update_priority() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Test");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            None,
            None,
            None,
            None,
            None,
            None,
            Some(1),
            None,
            None,
            None,
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.priority, 1);
    }

    #[test]
    fn test_update_add_label() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Test");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("urgent".to_string()),
            None,
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert!(updated.labels.contains(&"urgent".to_string()));
    }

    #[test]
    fn test_update_remove_label() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let mut unit = Unit::new("1", "Test");
        unit.labels = vec!["urgent".to_string(), "bug".to_string()];
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("urgent".to_string()),
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert!(!updated.labels.contains(&"urgent".to_string()));
        assert!(updated.labels.contains(&"bug".to_string()));
    }

    #[test]
    fn test_update_nonexistent_bean() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let result = cmd_update(
            &mana_dir,
            "99",
            Some("Title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_update_multiple_fields() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            Some("New desc".to_string()),
            None,
            None,
            None,
            Some("closed".to_string()),
            Some(0),
            None,
            None,
            None,
        )
        .unwrap();

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.title, "New title");
        assert_eq!(updated.description, Some("New desc".to_string()));
        assert_eq!(updated.status, Status::Closed);
        assert_eq!(updated.priority, 0);
    }

    #[test]
    fn test_update_rebuilds_index() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Index doesn't exist yet
        assert!(!mana_dir.join("index.yaml").exists());

        cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        // Index should be created
        assert!(mana_dir.join("index.yaml").exists());

        let index = Index::load(&mana_dir).unwrap();
        assert_eq!(index.units.len(), 1);
        assert_eq!(index.units[0].title, "New title");
    }

    #[test]
    fn test_update_rejects_priority_too_high() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Test");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        let result = cmd_update(
            &mana_dir,
            "1",
            None,
            None,
            None,
            None,
            None,
            None,
            Some(5),
            None,
            None,
            None,
        );
        assert!(result.is_err(), "Should reject priority > 4");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("priority"),
            "Error should mention priority"
        );
    }

    #[test]
    fn test_update_accepts_valid_priorities() {
        for priority in 0..=4 {
            let (_dir, mana_dir) = setup_test_beans_dir();
            let unit = Unit::new("1", "Test");
            let slug = title_to_slug(&unit.title);
            unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
                .unwrap();

            let result = cmd_update(
                &mana_dir,
                "1",
                None,
                None,
                None,
                None,
                None,
                None,
                Some(priority),
                None,
                None,
                None,
            );
            assert!(result.is_ok(), "Priority {} should be valid", priority);

            let updated =
                Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap())
                    .unwrap();
            assert_eq!(updated.priority, priority);
        }
    }

    // =====================================================================
    // Hook Tests
    // =====================================================================

    #[test]
    fn test_pre_update_hook_skipped_when_not_trusted() {
        let (_dir, mana_dir) = setup_test_beans_dir();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Update should succeed even without hooks (untrusted)
        let result = cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(
            result.is_ok(),
            "Update should succeed when hooks not trusted"
        );

        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.title, "New title");
    }

    #[test]
    fn test_pre_update_hook_rejects_update_when_fails() {
        use crate::hooks::create_trust;
        use std::os::unix::fs::PermissionsExt;

        let (dir, mana_dir) = setup_test_beans_dir();
        let project_root = dir.path();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Enable trust and create failing hook
        create_trust(project_root).unwrap();
        let hooks_dir = project_root.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_path = hooks_dir.join("pre-update");
        fs::write(&hook_path, "#!/bin/bash\nexit 1").unwrap();

        #[cfg(unix)]
        {
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        // Update should fail
        let result = cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(
            result.is_err(),
            "Update should fail when pre-update hook rejects"
        );
        assert!(result.unwrap_err().to_string().contains("rejected"));

        // Unit should not be modified
        let unchanged =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(unchanged.title, "Original");
    }

    #[test]
    fn test_pre_update_hook_allows_update_when_passes() {
        use crate::hooks::create_trust;
        use std::os::unix::fs::PermissionsExt;

        let (dir, mana_dir) = setup_test_beans_dir();
        let project_root = dir.path();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Enable trust and create passing hook
        create_trust(project_root).unwrap();
        let hooks_dir = project_root.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_path = hooks_dir.join("pre-update");
        fs::write(&hook_path, "#!/bin/bash\nexit 0").unwrap();

        #[cfg(unix)]
        {
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        // Update should succeed
        let result = cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(
            result.is_ok(),
            "Update should succeed when pre-update hook passes"
        );

        // Unit should be modified
        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.title, "New title");
    }

    #[test]
    fn test_post_update_hook_runs_after_successful_update() {
        use crate::hooks::create_trust;
        use std::os::unix::fs::PermissionsExt;

        let (dir, mana_dir) = setup_test_beans_dir();
        let project_root = dir.path();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Enable trust and create post-update hook that writes a marker file
        create_trust(project_root).unwrap();
        let hooks_dir = project_root.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_path = hooks_dir.join("post-update");
        let marker_path = project_root.join("post_update_ran");
        let marker_path_str = marker_path.to_string_lossy();

        fs::write(
            &hook_path,
            format!(
                "#!/bin/bash\necho 'post-update hook ran' > {}",
                marker_path_str
            ),
        )
        .unwrap();

        #[cfg(unix)]
        {
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        // Update unit
        cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        // Unit should be updated
        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.title, "New title");

        // Post-update hook should have run (marker file created)
        assert!(marker_path.exists(), "Post-update hook should have run");
    }

    #[test]
    fn test_post_update_hook_failure_does_not_prevent_update() {
        use crate::hooks::create_trust;
        use std::os::unix::fs::PermissionsExt;

        let (dir, mana_dir) = setup_test_beans_dir();
        let project_root = dir.path();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Enable trust and create failing post-update hook
        create_trust(project_root).unwrap();
        let hooks_dir = project_root.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_path = hooks_dir.join("post-update");
        fs::write(&hook_path, "#!/bin/bash\nexit 1").unwrap();

        #[cfg(unix)]
        {
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        // Update should still succeed even though post-hook fails
        let result = cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(
            result.is_ok(),
            "Update should succeed even if post-update hook fails"
        );

        // Unit should still be modified
        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.title, "New title");
    }

    #[test]
    fn test_update_with_multiple_fields_triggers_hooks() {
        use crate::hooks::create_trust;
        use std::os::unix::fs::PermissionsExt;

        let (dir, mana_dir) = setup_test_beans_dir();
        let project_root = dir.path();
        let unit = Unit::new("1", "Original");
        let slug = title_to_slug(&unit.title);
        unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
            .unwrap();

        // Enable trust and create hooks
        create_trust(project_root).unwrap();
        let hooks_dir = project_root.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        let pre_hook = hooks_dir.join("pre-update");
        fs::write(&pre_hook, "#!/bin/bash\nexit 0").unwrap();

        let post_hook = hooks_dir.join("post-update");
        fs::write(&post_hook, "#!/bin/bash\nexit 0").unwrap();

        #[cfg(unix)]
        {
            fs::set_permissions(&pre_hook, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&post_hook, fs::Permissions::from_mode(0o755)).unwrap();
        }

        // Update multiple fields
        let result = cmd_update(
            &mana_dir,
            "1",
            Some("New title".to_string()),
            Some("New desc".to_string()),
            None,
            None,
            None,
            Some("in_progress".to_string()),
            None,
            None,
            None,
            None,
        );
        assert!(result.is_ok());

        // Verify all changes applied
        let updated =
            Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
        assert_eq!(updated.title, "New title");
        assert_eq!(updated.description, Some("New desc".to_string()));
        assert_eq!(updated.status, Status::InProgress);
    }
}
