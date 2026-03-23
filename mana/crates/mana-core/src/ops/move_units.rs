use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::config::Config;
use crate::discovery::find_unit_file;
use crate::index::Index;
use crate::unit::Unit;

/// Result of a move operation.
pub struct MoveResult {
    /// Map of old_id -> new_id.
    pub id_map: HashMap<String, String>,
}

/// Resolve a path to a `.mana/` directory.
///
/// Accepts either a path ending in `.mana/` or a project directory containing `.mana/`.
pub fn resolve_mana_dir(path: &Path) -> Result<PathBuf> {
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
/// For each unit ID, loads from source, assigns new sequential ID in destination,
/// writes to destination, removes from source, and rebuilds both indices.
/// Clears parent, dependencies, and claim fields on the moved unit.
///
/// Returns a map of old_id -> new_id.
pub fn move_units(source_dir: &Path, dest_dir: &Path, ids: &[String]) -> Result<MoveResult> {
    let source_canonical = source_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve source path: {}", source_dir.display()))?;
    let dest_canonical = dest_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve destination path: {}", dest_dir.display()))?;
    if source_canonical == dest_canonical {
        bail!("Source and destination are the same .mana/ directory");
    }

    let mut dest_config = Config::load(dest_dir).context("Failed to load destination config")?;

    let mut id_map: HashMap<String, String> = HashMap::new();
    let mut source_files_to_remove: Vec<PathBuf> = Vec::new();

    for old_id in ids {
        let source_path = find_unit_file(source_dir, old_id)
            .with_context(|| format!("Unit '{}' not found in {}", old_id, source_dir.display()))?;
        let mut unit = Unit::from_file(&source_path)
            .with_context(|| format!("Failed to load unit '{}' from source", old_id))?;

        let new_id = dest_config.increment_id().to_string();

        unit.id = new_id.clone();
        unit.updated_at = Utc::now();
        unit.parent = None;
        unit.dependencies.clear();
        unit.claimed_by = None;
        unit.claimed_at = None;

        let slug = unit.slug.clone().unwrap_or_else(|| "unnamed".to_string());
        let dest_filename = format!("{}-{}.md", new_id, slug);
        let dest_path = dest_dir.join(&dest_filename);
        unit.to_file(&dest_path)
            .with_context(|| format!("Failed to write unit to {}", dest_path.display()))?;

        source_files_to_remove.push(source_path);
        id_map.insert(old_id.clone(), new_id);
    }

    dest_config
        .save(dest_dir)
        .context("Failed to save destination config")?;

    for path in &source_files_to_remove {
        fs::remove_file(path)
            .with_context(|| format!("Failed to remove source file: {}", path.display()))?;
    }

    let dest_index = Index::build(dest_dir)?;
    dest_index.save(dest_dir)?;

    if source_dir.join("config.yaml").exists() {
        let source_index = Index::build(source_dir)?;
        source_index.save(source_dir)?;
    }

    Ok(MoveResult { id_map })
}

/// Move units from another project into this one.
pub fn move_from(mana_dir: &Path, from: &str, ids: &[String]) -> Result<MoveResult> {
    let from_path = PathBuf::from(from);
    let source_dir = resolve_mana_dir(&from_path)
        .with_context(|| format!("Failed to resolve --from: {}", from))?;
    move_units(&source_dir, mana_dir, ids)
}

/// Move units from this project into another one.
pub fn move_to(mana_dir: &Path, to: &str, ids: &[String]) -> Result<MoveResult> {
    let to_path = PathBuf::from(to);
    let dest_dir =
        resolve_mana_dir(&to_path).with_context(|| format!("Failed to resolve --to: {}", to))?;
    move_units(mana_dir, &dest_dir, ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::title_to_slug;
    use tempfile::TempDir;

    fn setup_mana_dir(name: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        Config {
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
            memory_reserve_mb: 0,
            notify: None,
        }
        .save(&mana_dir)
        .unwrap();

        (dir, mana_dir)
    }

    fn create_test_unit(mana_dir: &Path, id: &str, title: &str) {
        let mut unit = Unit::new(id, title);
        unit.slug = Some(title_to_slug(title));
        unit.verify = Some("true".to_string());
        let slug = unit.slug.clone().unwrap();
        unit.to_file(mana_dir.join(format!("{}-{}.md", id, slug)))
            .unwrap();
    }

    #[test]
    fn move_single_unit() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        create_test_unit(&src_units, "1", "Fix login bug");

        let result = move_units(&src_units, &dst_units, &["1".to_string()]).unwrap();

        assert_eq!(result.id_map.get("1"), Some(&"1".to_string()));
        assert!(!src_units.join("1-fix-login-bug.md").exists());
        assert!(dst_units.join("1-fix-login-bug.md").exists());
    }

    #[test]
    fn move_fails_for_same_directory() {
        let (_dir, mana_dir) = setup_mana_dir("same");
        create_test_unit(&mana_dir, "1", "Task");

        let result = move_units(&mana_dir, &mana_dir, &["1".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn move_clears_parent_and_deps() {
        let (_src_dir, src_units) = setup_mana_dir("source");
        let (_dst_dir, dst_units) = setup_mana_dir("dest");

        let mut unit = Unit::new("1.1", "Child task");
        unit.slug = Some("child-task".to_string());
        unit.verify = Some("true".to_string());
        unit.parent = Some("1".to_string());
        unit.dependencies = vec!["5".to_string()];
        unit.claimed_by = Some("agent-1".to_string());
        unit.to_file(src_units.join("1.1-child-task.md")).unwrap();

        let result = move_units(&src_units, &dst_units, &["1.1".to_string()]).unwrap();

        let new_id = result.id_map.get("1.1").unwrap();
        let moved = Unit::from_file(dst_units.join(format!("{}-child-task.md", new_id))).unwrap();

        assert!(moved.parent.is_none());
        assert!(moved.dependencies.is_empty());
        assert!(moved.claimed_by.is_none());
    }

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
}
