use super::*;
use crate::unit::{RunResult, Status, Unit};
use crate::util::title_to_slug;
use std::fs;
use tempfile::{Builder, TempDir};

fn setup_test_mana_dir() -> (TempDir, std::path::PathBuf) {
    let dir = Builder::new()
        .prefix("mana-close-verify-timeout-")
        .tempdir()
        .unwrap();
    let project_root = fs::canonicalize(dir.path()).unwrap();
    let mana_dir = project_root.join(".mana");
    fs::create_dir(&mana_dir).unwrap();
    (dir, mana_dir)
}

#[test]
fn verify_timeout_kills_slow_process_and_records_timeout() {
    let (_dir, mana_dir) = setup_test_mana_dir();

    let mut unit = Unit::new("1", "Slow verify task");
    unit.verify = Some("sleep 60".to_string());
    unit.verify_timeout = Some(1);
    let slug = title_to_slug(&unit.title);
    unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
        .unwrap();

    cmd_close(&mana_dir, vec!["1".to_string()], None, false).unwrap();

    let updated =
        Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
    assert_eq!(updated.status, Status::Open);
    assert_eq!(updated.attempts, 1);
    assert!(updated.closed_at.is_none());

    assert_eq!(updated.history.len(), 1);
    assert_eq!(updated.history[0].result, RunResult::Timeout);
    assert!(updated.history[0].exit_code.is_none());

    let snippet = updated.history[0].output_snippet.as_deref().unwrap_or("");
    assert!(
        snippet.contains("timed out"),
        "expected snippet to contain 'timed out', got: {:?}",
        snippet
    );
}

#[test]
fn verify_timeout_does_not_affect_fast_commands() {
    let (_dir, mana_dir) = setup_test_mana_dir();

    let mut unit = Unit::new("1", "Fast verify task");
    unit.verify = Some("true".to_string());
    unit.verify_timeout = Some(30);
    let slug = title_to_slug(&unit.title);
    unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
        .unwrap();

    cmd_close(&mana_dir, vec!["1".to_string()], None, false).unwrap();

    let archived = crate::discovery::find_archived_unit(&mana_dir, "1").unwrap();
    let updated = Unit::from_file(&archived).unwrap();
    assert_eq!(updated.status, Status::Closed);
    assert!(updated.is_archived);
}

#[test]
fn verify_timeout_unit_level_overrides_config() {
    let (_dir, mana_dir) = setup_test_mana_dir();

    let config_yaml = "project: test\nnext_id: 2\nverify_timeout: 60\n";
    fs::write(mana_dir.join("config.yaml"), config_yaml).unwrap();

    let mut unit = Unit::new("1", "Unit timeout overrides config");
    unit.verify = Some("sleep 60".to_string());
    unit.verify_timeout = Some(1);
    let slug = title_to_slug(&unit.title);
    unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
        .unwrap();

    cmd_close(&mana_dir, vec!["1".to_string()], None, false).unwrap();

    let updated =
        Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
    assert_eq!(updated.status, Status::Open);
    assert_eq!(updated.history[0].result, RunResult::Timeout);
}

#[test]
fn verify_timeout_config_level_applies_when_unit_has_none() {
    let (_dir, mana_dir) = setup_test_mana_dir();

    let config_yaml = "project: test\nnext_id: 2\nverify_timeout: 1\n";
    fs::write(mana_dir.join("config.yaml"), config_yaml).unwrap();

    let mut unit = Unit::new("1", "Config timeout applies");
    unit.verify = Some("sleep 60".to_string());
    let slug = title_to_slug(&unit.title);
    unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
        .unwrap();

    cmd_close(&mana_dir, vec!["1".to_string()], None, false).unwrap();

    let updated =
        Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
    assert_eq!(updated.status, Status::Open);
    assert_eq!(updated.history[0].result, RunResult::Timeout);
}

#[test]
fn verify_timeout_appends_to_notes() {
    let (_dir, mana_dir) = setup_test_mana_dir();

    let mut unit = Unit::new("1", "Timeout notes test");
    unit.verify = Some("sleep 60".to_string());
    unit.verify_timeout = Some(1);
    let slug = title_to_slug(&unit.title);
    unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
        .unwrap();

    cmd_close(&mana_dir, vec!["1".to_string()], None, false).unwrap();

    let updated =
        Unit::from_file(crate::discovery::find_unit_file(&mana_dir, "1").unwrap()).unwrap();
    let notes = updated.notes.unwrap_or_default();
    assert!(
        notes.contains("timed out"),
        "expected notes to contain 'timed out', got: {:?}",
        notes
    );
}

#[test]
fn effective_verify_timeout_unit_wins_over_config() {
    let unit = {
        let mut b = Unit::new("1", "Test");
        b.verify_timeout = Some(5);
        b
    };
    assert_eq!(unit.effective_verify_timeout(Some(30)), Some(5));
}

#[test]
fn effective_verify_timeout_config_fallback() {
    let unit = Unit::new("1", "Test");
    assert_eq!(unit.effective_verify_timeout(Some(30)), Some(30));
}

#[test]
fn effective_verify_timeout_both_none() {
    let unit = Unit::new("1", "Test");
    assert_eq!(unit.effective_verify_timeout(None), None);
}
