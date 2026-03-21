use std::fs;
use tempfile::TempDir;

use mana::api::*;

/// Set up a temporary .mana/ directory with a sample unit.
fn setup_test_env() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let mana_dir = dir.path().join(".mana");
    fs::create_dir_all(&mana_dir).unwrap();

    // Write minimal config
    fs::write(mana_dir.join("config.yaml"), "next_id: 2\n").unwrap();

    // Write a sample unit
    let unit = Unit::new("1", "Sample task");
    let slug = mana::util::title_to_slug(&unit.title);
    unit.to_file(mana_dir.join(format!("1-{}.md", slug)))
        .unwrap();

    (dir, mana_dir)
}

#[test]
fn api_re_exports_core_types() {
    // This test verifies that core types are accessible via mana::api
    let _status = Status::Open;
    let _result = RunResult::Pass;
    let unit = Unit::new("1", "Test");
    assert_eq!(unit.id, "1");
    assert_eq!(unit.status, Status::Open);
}

#[test]
fn api_get_bean_loads_by_id() {
    let (_dir, mana_dir) = setup_test_env();
    let unit = get_bean(&mana_dir, "1").unwrap();
    assert_eq!(unit.id, "1");
    assert_eq!(unit.title, "Sample task");
    assert_eq!(unit.status, Status::Open);
}

#[test]
fn api_get_bean_not_found() {
    let (_dir, mana_dir) = setup_test_env();
    let result = get_bean(&mana_dir, "999");
    assert!(result.is_err());
}

#[test]
fn api_load_index_returns_entries() {
    let (_dir, mana_dir) = setup_test_env();
    let index = load_index(&mana_dir).unwrap();
    assert_eq!(index.units.len(), 1);
    assert_eq!(index.units[0].id, "1");
    assert_eq!(index.units[0].title, "Sample task");
}

#[test]
fn api_find_mana_dir_discovers_directory() {
    let (dir, _beans_dir) = setup_test_env();
    let found = find_mana_dir(dir.path()).unwrap();
    assert!(found.ends_with(".mana"));
    assert!(found.is_dir());
}

#[test]
fn api_types_are_serializable() {
    let unit = Unit::new("1", "Serializable");
    let json = serde_json::to_string(&unit).unwrap();
    assert!(json.contains("Serializable"));

    let entry = IndexEntry::from(&unit);
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("Serializable"));
}

#[test]
fn api_graph_functions_accessible() {
    let (_dir, mana_dir) = setup_test_env();
    let index = load_index(&mana_dir).unwrap();

    // No cycles in a single-unit graph
    let cycles = find_all_cycles(&index).unwrap();
    assert!(cycles.is_empty());

    // Full graph renders
    let graph = build_full_graph(&index).unwrap();
    assert!(graph.contains("Sample task"));
}
