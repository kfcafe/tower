use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::config::Config;
use crate::discovery::find_unit_file;
use crate::index::Index;
use crate::unit::Unit;

/// Resolve a path to a `.mana/` directory.
///
/// Accepts either:
/// - A path ending in `.mana/` directly
/// - A project directory containing `.mana/`
fn resolve_mana_dir(path: &Path) -> Result<PathBuf> {
    if path.is_dir() && path.file_name().is_some_and(|n| n == ".mana") {
        return Ok(path.to_path_buf());
    }

    let candidate = path.join(".mana");
    if candidate.is_dir() {
        return Ok(candidate);
    }

    bail!(
        "No .mana/ directory found at '{}'\n\
         Pass the path to a .mana/ directory or the project directory containing it.",
        path.display()
    );
}

/// Move units between two `.mana/` directories.
///
/// For each unit ID:
/// 1. Loads the unit from the source directory
/// 2. Assigns a new sequential ID in the destination (via `config.next_id`)
/// 3. Writes the unit to the destination with the new ID
/// 4. Removes the unit from the source
/// 5. Updates both source and destination indices
///
/// Returns a map of old_id → new_id.
fn move_units(
    source_dir: &Path,
    dest_dir: &Path,
    ids: &[String],
) -> Result<HashMap<String, String>> {
    // Prevent moving units into the same directory
    let source_canonical = source_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve source path: {}", source_dir.display()))?;
    let dest_canonical = dest_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve destination path: {}", dest_dir.display()))?;
    if source_canonical == dest_canonical {
        bail!("Source and destination are the same .mana/ directory");
    }

    // Load destination config to get next_id
    let mut dest_config = Config::load(dest_dir).context("Failed to load destination config")?;

    let mut id_map: HashMap<String, String> = HashMap::new();
    let mut source_files_to_remove: Vec<PathBuf> = Vec::new();

    for old_id in ids {
        // Find and load the unit from source
        let source_path = find_unit_file(source_dir, old_id)
            .with_context(|| format!("Unit '{}' not found in {}", old_id, source_dir.display()))?;
        let mut unit = Unit::from_file(&source_path)
            .with_context(|| format!("Failed to load unit '{}' from source", old_id))?;

        // Assign a new ID in the destination
        let new_id = dest_config.increment_id().to_string();

        // Update unit fields
        unit.id = new_id.clone();
        unit.updated_at = Utc::now();

        // Clear source-specific fields that don't transfer cleanly
        unit.parent = None;
        unit.dependencies.clear();
        unit.claimed_by = None;
        unit.claimed_at = None;

        // Write to destination
        let slug = unit.slug.clone().unwrap_or_else(|| "unnamed".to_string());
        let dest_filename = format!("{}-{}.md", new_id, slug);
        let dest_path = dest_dir.join(&dest_filename);
        unit.to_file(&dest_path)
            .with_context(|| format!("Failed to write unit to {}", dest_path.display()))?;

        // Track for removal
        source_files_to_remove.push(source_path);
        id_map.insert(old_id.clone(), new_id.clone());

        eprintln!("Moved {} → {} ({})", old_id, new_id, unit.title);
    }

    // Save updated destination config (with incremented next_id)
    dest_config
        .save(dest_dir)
        .context("Failed to save destination config")?;

    // Remove source files
    for path in &source_files_to_remove {
        fs::remove_file(path)
            .with_context(|| format!("Failed to remove source file: {}", path.display()))?;
    }

    // Rebuild both indices
    let dest_index = Index::build(dest_dir)?;
    dest_index.save(dest_dir)?;

    if source_dir.join("config.yaml").exists() {
        let source_index = Index::build(source_dir)?;
        source_index.save(source_dir)?;
    }

    Ok(id_map)
}

/// Move units from another `.mana/` directory into the current project.
///
/// `mana_dir` is the current project's `.mana/` (the destination).
pub fn cmd_move_from(
    mana_dir: &Path,
    from: &str,
    ids: &[String],
) -> Result<HashMap<String, String>> {
    let from_path = PathBuf::from(from);
    let source_dir = resolve_mana_dir(&from_path)
        .with_context(|| format!("Failed to resolve --from: {}", from))?;

    let result = move_units(&source_dir, mana_dir, ids)?;

    eprintln!(
        "\nMoved {} unit{} from {} → {}",
        result.len(),
        if result.len() == 1 { "" } else { "s" },
        source_dir.display(),
        mana_dir.display(),
    );

    Ok(result)
}

/// Move units from the current project into another `.mana/` directory.
///
/// `mana_dir` is the current project's `.mana/` (the source).
pub fn cmd_move_to(mana_dir: &Path, to: &str, ids: &[String]) -> Result<HashMap<String, String>> {
    let to_path = PathBuf::from(to);
    let dest_dir =
        resolve_mana_dir(&to_path).with_context(|| format!("Failed to resolve --to: {}", to))?;

    let result = move_units(mana_dir, &dest_dir, ids)?;

    eprintln!(
        "\nMoved {} unit{} from {} → {}",
        result.len(),
        if result.len() == 1 { "" } else { "s" },
        mana_dir.display(),
        dest_dir.display(),
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_mana_dir(name: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let config = Config {
            project: name.to_string(),
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
        };
        config.save(&mana_dir).unwrap();

        (dir, mana_dir)
    }

    fn create_test_unit(mana_dir: &Path, id: &str, title: &str) {
        let mut unit = Unit::new(id, title);
        unit.slug = Some(crate::util::title_to_slug(title));
        unit.verify = Some("true".to_string());
        let slug = unit.slug.clone().unwrap();
        unit.to_file(mana_dir.join(format!("{}-{}.md", id, slug)))
            .unwrap();
    }

    // =====================================================================
    // move_units (core)
    // =====================================================================

    #[test]
    fn move_single_unit() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        create_test_unit(&src_units, "1", "Fix login bug");

        let result = move_units(&src_units, &dst_units, &["1".to_string()]).unwrap();

        assert_eq!(result.get("1"), Some(&"1".to_string()));
        assert!(!src_units.join("1-fix-login-bug.md").exists());
        assert!(dst_units.join("1-fix-login-bug.md").exists());

        let moved = Unit::from_file(dst_units.join("1-fix-login-bug.md")).unwrap();
        assert_eq!(moved.id, "1");
        assert_eq!(moved.title, "Fix login bug");
        assert!(moved.parent.is_none());
        assert!(moved.dependencies.is_empty());
    }

    #[test]
    fn move_multiple_units() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        let mut config = Config::load(&dst_units).unwrap();
        config.next_id = 10;
        config.save(&dst_units).unwrap();

        create_test_unit(&src_units, "1", "Task one");
        create_test_unit(&src_units, "2", "Task two");
        create_test_unit(&src_units, "3", "Task three");

        let result = move_units(
            &src_units,
            &dst_units,
            &["1".to_string(), "2".to_string(), "3".to_string()],
        )
        .unwrap();

        assert_eq!(result.get("1"), Some(&"10".to_string()));
        assert_eq!(result.get("2"), Some(&"11".to_string()));
        assert_eq!(result.get("3"), Some(&"12".to_string()));

        assert!(!src_units.join("1-task-one.md").exists());
        assert!(dst_units.join("10-task-one.md").exists());
        assert!(dst_units.join("11-task-two.md").exists());
        assert!(dst_units.join("12-task-three.md").exists());
    }

    #[test]
    fn move_clears_parent_and_deps() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        let mut unit = Unit::new("1.1", "Child task");
        unit.slug = Some("child-task".to_string());
        unit.verify = Some("true".to_string());
        unit.parent = Some("1".to_string());
        unit.dependencies = vec!["5".to_string(), "6".to_string()];
        unit.claimed_by = Some("agent-1".to_string());
        unit.to_file(src_units.join("1.1-child-task.md")).unwrap();

        let result = move_units(&src_units, &dst_units, &["1.1".to_string()]).unwrap();

        let new_id = result.get("1.1").unwrap();
        let moved = Unit::from_file(dst_units.join(format!("{}-child-task.md", new_id))).unwrap();

        assert!(moved.parent.is_none());
        assert!(moved.dependencies.is_empty());
        assert!(moved.claimed_by.is_none());
        assert!(moved.claimed_at.is_none());
        assert_eq!(moved.title, "Child task");
        assert_eq!(moved.verify, Some("true".to_string()));
    }

    #[test]
    fn move_preserves_unit_content() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        let mut unit = Unit::new("1", "Complex task");
        unit.slug = Some("complex-task".to_string());
        unit.verify = Some("cargo test auth".to_string());
        unit.description = Some("Do the thing with the stuff".to_string());
        unit.acceptance = Some("All tests pass".to_string());
        unit.notes = Some("Tried X, failed. Avoid Y.".to_string());
        unit.labels = vec!["bug".to_string(), "auth".to_string()];
        unit.priority = 0;
        unit.to_file(src_units.join("1-complex-task.md")).unwrap();

        let result = move_units(&src_units, &dst_units, &["1".to_string()]).unwrap();

        let new_id = result.get("1").unwrap();
        let moved = Unit::from_file(dst_units.join(format!("{}-complex-task.md", new_id))).unwrap();

        assert_eq!(moved.verify, Some("cargo test auth".to_string()));
        assert_eq!(
            moved.description,
            Some("Do the thing with the stuff".to_string())
        );
        assert_eq!(moved.acceptance, Some("All tests pass".to_string()));
        assert_eq!(moved.notes, Some("Tried X, failed. Avoid Y.".to_string()));
        assert_eq!(moved.labels, vec!["bug".to_string(), "auth".to_string()]);
        assert_eq!(moved.priority, 0);
    }

    #[test]
    fn move_fails_for_missing_unit() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        let result = move_units(&src_units, &dst_units, &["999".to_string()]);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unit '999' not found"));
    }

    #[test]
    fn move_fails_for_same_directory() {
        let (_dir, mana_dir) = setup_mana_dir("same");
        create_test_unit(&mana_dir, "1", "Task");

        let result = move_units(&mana_dir, &mana_dir, &["1".to_string()]);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Source and destination are the same"));
    }

    #[test]
    fn move_updates_destination_config_next_id() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        create_test_unit(&src_units, "1", "Task one");
        create_test_unit(&src_units, "2", "Task two");

        move_units(&src_units, &dst_units, &["1".to_string(), "2".to_string()]).unwrap();

        let config = Config::load(&dst_units).unwrap();
        assert_eq!(config.next_id, 3);
    }

    #[test]
    fn move_rebuilds_both_indices() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        create_test_unit(&src_units, "1", "Task one");
        create_test_unit(&src_units, "2", "Task two");

        move_units(&src_units, &dst_units, &["1".to_string()]).unwrap();

        let src_index = Index::load(&src_units).unwrap();
        assert_eq!(src_index.units.len(), 1);
        assert_eq!(src_index.units[0].id, "2");

        let dst_index = Index::load(&dst_units).unwrap();
        assert_eq!(dst_index.units.len(), 1);
        assert_eq!(dst_index.units[0].title, "Task one");
    }

    // =====================================================================
    // cmd_move_from (pull direction)
    // =====================================================================

    #[test]
    fn move_from_with_mana_dir_path() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        create_test_unit(&src_units, "1", "Some task");

        let result =
            cmd_move_from(&dst_units, src_units.to_str().unwrap(), &["1".to_string()]).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn move_from_with_project_dir_path() {
        let (src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        create_test_unit(&src_units, "1", "Some task");

        let result = cmd_move_from(
            &dst_units,
            src_dir.path().to_str().unwrap(),
            &["1".to_string()],
        )
        .unwrap();

        assert_eq!(result.len(), 1);
    }

    // =====================================================================
    // cmd_move_to (push direction)
    // =====================================================================

    #[test]
    fn move_to_pushes_units() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        let mut config = Config::load(&dst_units).unwrap();
        config.next_id = 50;
        config.save(&dst_units).unwrap();

        create_test_unit(&src_units, "1", "Push me");

        let result =
            cmd_move_to(&src_units, dst_units.to_str().unwrap(), &["1".to_string()]).unwrap();

        assert_eq!(result.get("1"), Some(&"50".to_string()));
        assert!(!src_units.join("1-push-me.md").exists());
        assert!(dst_units.join("50-push-me.md").exists());
    }

    #[test]
    fn move_to_with_project_dir_path() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (dst_dir, dst_units) = setup_mana_dir("dest");

        create_test_unit(&src_units, "1", "Push task");

        let result = cmd_move_to(
            &src_units,
            dst_dir.path().to_str().unwrap(),
            &["1".to_string()],
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        assert!(dst_units.join("1-push-task.md").exists());
    }

    // =====================================================================
    // resolve_mana_dir
    // =====================================================================

    #[test]
    fn resolve_with_mana_dir() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let result = resolve_mana_dir(&mana_dir).unwrap();
        assert_eq!(result, mana_dir);
    }

    #[test]
    fn resolve_with_project_dir() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let result = resolve_mana_dir(dir.path()).unwrap();
        assert_eq!(result, mana_dir);
    }

    #[test]
    fn resolve_fails_for_no_units() {
        let dir = TempDir::new().unwrap();
        let result = resolve_mana_dir(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn move_to_fails_for_invalid_dest() {
        let (_dir, src_units) = setup_mana_dir("source");
        create_test_unit(&src_units, "1", "Task");

        let result = cmd_move_to(&src_units, "/nonexistent/path", &["1".to_string()]);
        assert!(result.is_err());
    }
}
