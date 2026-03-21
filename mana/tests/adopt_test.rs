//! Integration tests for the `bn adopt` command.
//!
//! These tests verify the public API for adopting existing units as children
//! of a parent unit.

use std::fs;

use mana::commands::cmd_adopt;
use mana::config::Config;
use mana::index::Index;
use mana::unit::Unit;
use tempfile::TempDir;

/// Setup a test environment with a .mana directory and config.
fn setup_test_env() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let mana_dir = dir.path().join(".mana");
    fs::create_dir(&mana_dir).unwrap();

    let config = Config {
        project: "test-adopt".to_string(),
        next_id: 100,
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
    };
    config.save(&mana_dir).unwrap();

    (dir, mana_dir)
}

/// Helper to create a unit with standard fields.
fn create_bean(mana_dir: &std::path::Path, id: &str, title: &str, is_parent: bool) {
    let mut unit = Unit::new(id, title);
    let slug = title.to_lowercase().replace(' ', "-");
    unit.slug = Some(slug.clone());

    if is_parent {
        unit.acceptance = Some("All children complete".to_string());
    } else {
        unit.verify = Some("true".to_string());
    }

    let filename = format!("{}-{}.md", id, slug);
    unit.to_file(mana_dir.join(filename)).unwrap();
}

#[test]
fn test_adopt_basic_single() {
    let (_dir, mana_dir) = setup_test_env();

    // Create parent (100) and child to adopt (101)
    create_bean(&mana_dir, "100", "Parent Task", true);
    create_bean(&mana_dir, "101", "Child Task", false);

    // Adopt: 101 should become 100.1
    let result = cmd_adopt(&mana_dir, "100", &["101".to_string()]).unwrap();

    // Verify the ID mapping
    assert_eq!(result.get("101"), Some(&"100.1".to_string()));

    // Old file should be gone
    assert!(!mana_dir.join("101-child-task.md").exists());

    // New file should exist
    assert!(mana_dir.join("100.1-child-task.md").exists());

    // Verify unit content
    let adopted = Unit::from_file(mana_dir.join("100.1-child-task.md")).unwrap();
    assert_eq!(adopted.id, "100.1");
    assert_eq!(adopted.parent, Some("100".to_string()));
    assert_eq!(adopted.title, "Child Task");
}

#[test]
fn test_adopt_multiple_children() {
    let (_dir, mana_dir) = setup_test_env();

    // Create parent (100) and three children to adopt (101, 102, 103)
    create_bean(&mana_dir, "100", "Parent", true);
    create_bean(&mana_dir, "101", "First", false);
    create_bean(&mana_dir, "102", "Second", false);
    create_bean(&mana_dir, "103", "Third", false);

    // Adopt all three: they should become 100.1, 100.2, 100.3
    let result = cmd_adopt(
        &mana_dir,
        "100",
        &["101".to_string(), "102".to_string(), "103".to_string()],
    )
    .unwrap();

    // Verify sequential numbering
    assert_eq!(result.get("101"), Some(&"100.1".to_string()));
    assert_eq!(result.get("102"), Some(&"100.2".to_string()));
    assert_eq!(result.get("103"), Some(&"100.3".to_string()));

    // All new files should exist
    assert!(mana_dir.join("100.1-first.md").exists());
    assert!(mana_dir.join("100.2-second.md").exists());
    assert!(mana_dir.join("100.3-third.md").exists());

    // All old files should be removed
    assert!(!mana_dir.join("101-first.md").exists());
    assert!(!mana_dir.join("102-second.md").exists());
    assert!(!mana_dir.join("103-third.md").exists());
}

#[test]
fn test_adopt_files_renamed_correctly() {
    let (_dir, mana_dir) = setup_test_env();

    create_bean(&mana_dir, "100", "Parent", true);

    // Create a unit with a specific slug
    let mut unit = Unit::new("101", "My Complex Task Name");
    unit.slug = Some("my-complex-task-name".to_string());
    unit.verify = Some("echo ok".to_string());
    unit.to_file(mana_dir.join("101-my-complex-task-name.md"))
        .unwrap();

    cmd_adopt(&mana_dir, "100", &["101".to_string()]).unwrap();

    // Verify new filename preserves the slug
    assert!(mana_dir.join("100.1-my-complex-task-name.md").exists());

    // Verify content is preserved
    let adopted = Unit::from_file(mana_dir.join("100.1-my-complex-task-name.md")).unwrap();
    assert_eq!(adopted.slug, Some("my-complex-task-name".to_string()));
    assert_eq!(adopted.verify, Some("echo ok".to_string()));
}

#[test]
fn test_adopt_updates_dependency_references() {
    let (_dir, mana_dir) = setup_test_env();

    // Create parent
    create_bean(&mana_dir, "100", "Parent", true);

    // Create unit to adopt (101)
    create_bean(&mana_dir, "101", "Task A", false);

    // Create unit that depends on 101
    let mut dependent = Unit::new("102", "Task B");
    dependent.slug = Some("task-b".to_string());
    dependent.verify = Some("true".to_string());
    dependent.dependencies = vec!["101".to_string()];
    dependent.to_file(mana_dir.join("102-task-b.md")).unwrap();

    // Adopt 101 under 100
    cmd_adopt(&mana_dir, "100", &["101".to_string()]).unwrap();

    // Dependency in unit 102 should now point to 100.1
    let updated = Unit::from_file(mana_dir.join("102-task-b.md")).unwrap();
    assert_eq!(updated.dependencies, vec!["100.1".to_string()]);
}

#[test]
fn test_adopt_updates_index() {
    let (_dir, mana_dir) = setup_test_env();

    create_bean(&mana_dir, "100", "Parent", true);
    create_bean(&mana_dir, "101", "Child", false);

    cmd_adopt(&mana_dir, "100", &["101".to_string()]).unwrap();

    // Load and verify the index
    let index = Index::load(&mana_dir).unwrap();

    // Should have 2 units: parent and adopted child
    assert_eq!(index.units.len(), 2);

    // Adopted unit should have new ID in index
    let adopted = index.units.iter().find(|b| b.id == "100.1");
    assert!(adopted.is_some());
    assert_eq!(adopted.unwrap().parent, Some("100".to_string()));

    // Old ID should not exist in index
    assert!(!index.units.iter().any(|b| b.id == "101"));
}

#[test]
fn test_adopt_error_missing_parent() {
    let (_dir, mana_dir) = setup_test_env();

    // Only create the child, no parent
    create_bean(&mana_dir, "101", "Orphan", false);

    // Try to adopt under non-existent parent
    let result = cmd_adopt(&mana_dir, "999", &["101".to_string()]);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Parent unit '999' not found"),
        "Error should mention missing parent, got: {}",
        err_msg
    );
}

#[test]
fn test_adopt_error_missing_child() {
    let (_dir, mana_dir) = setup_test_env();

    // Only create the parent, no child
    create_bean(&mana_dir, "100", "Parent", true);

    // Try to adopt non-existent child
    let result = cmd_adopt(&mana_dir, "100", &["999".to_string()]);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Child unit '999' not found"),
        "Error should mention missing child, got: {}",
        err_msg
    );
}

#[test]
fn test_adopt_continues_numbering_after_existing_children() {
    let (_dir, mana_dir) = setup_test_env();

    // Create parent with existing children
    create_bean(&mana_dir, "100", "Parent", true);

    let mut child1 = Unit::new("100.1", "Existing Child 1");
    child1.slug = Some("existing-child-1".to_string());
    child1.parent = Some("100".to_string());
    child1.verify = Some("true".to_string());
    child1
        .to_file(mana_dir.join("100.1-existing-child-1.md"))
        .unwrap();

    let mut child2 = Unit::new("100.2", "Existing Child 2");
    child2.slug = Some("existing-child-2".to_string());
    child2.parent = Some("100".to_string());
    child2.verify = Some("true".to_string());
    child2
        .to_file(mana_dir.join("100.2-existing-child-2.md"))
        .unwrap();

    // Create new unit to adopt
    create_bean(&mana_dir, "103", "New Child", false);

    // Adopt - should become 100.3, not 100.1
    let result = cmd_adopt(&mana_dir, "100", &["103".to_string()]).unwrap();

    assert_eq!(result.get("103"), Some(&"100.3".to_string()));
    assert!(mana_dir.join("100.3-new-child.md").exists());
}

#[test]
fn test_adopt_bean_already_has_parent() {
    let (_dir, mana_dir) = setup_test_env();

    // Create two potential parent units
    create_bean(&mana_dir, "100", "Parent A", true);
    create_bean(&mana_dir, "200", "Parent B", true);

    // Create a child that already belongs to Parent A
    let mut child = Unit::new("100.1", "Existing Child");
    child.slug = Some("existing-child".to_string());
    child.parent = Some("100".to_string());
    child.verify = Some("true".to_string());
    child
        .to_file(mana_dir.join("100.1-existing-child.md"))
        .unwrap();

    // Adopt this child under Parent B - this re-parents the unit
    let result = cmd_adopt(&mana_dir, "200", &["100.1".to_string()]).unwrap();

    // Unit should be re-parented to 200.1
    assert_eq!(result.get("100.1"), Some(&"200.1".to_string()));

    // Verify the new parent is set
    let reparented = Unit::from_file(mana_dir.join("200.1-existing-child.md")).unwrap();
    assert_eq!(reparented.parent, Some("200".to_string()));
    assert_eq!(reparented.id, "200.1");

    // Old file should be gone
    assert!(!mana_dir.join("100.1-existing-child.md").exists());
}

#[test]
fn test_adopt_preserves_bean_fields() {
    let (_dir, mana_dir) = setup_test_env();

    create_bean(&mana_dir, "100", "Parent", true);

    // Create a unit with lots of fields
    let mut unit = Unit::new("101", "Complex Unit");
    unit.slug = Some("complex-unit".to_string());
    unit.description = Some("A detailed description".to_string());
    unit.acceptance = Some("All criteria met".to_string());
    unit.verify = Some("cargo test".to_string());
    unit.dependencies = vec![];
    unit.priority = 1;
    unit.to_file(mana_dir.join("101-complex-unit.md")).unwrap();

    cmd_adopt(&mana_dir, "100", &["101".to_string()]).unwrap();

    // Verify all fields are preserved
    let adopted = Unit::from_file(mana_dir.join("100.1-complex-unit.md")).unwrap();
    assert_eq!(adopted.title, "Complex Unit");
    assert_eq!(
        adopted.description,
        Some("A detailed description".to_string())
    );
    assert_eq!(adopted.acceptance, Some("All criteria met".to_string()));
    assert_eq!(adopted.verify, Some("cargo test".to_string()));
    assert_eq!(adopted.priority, 1);
}
