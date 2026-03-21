use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WizardLocalState {
    pub open_views: Vec<String>,
    pub last_project: Option<String>,
}

pub fn state_path(root: &Path) -> PathBuf {
    root.join(".wizard").join("state.json")
}

/// Load local state from `.wizard/state.json`
/// Returns default state if file doesn't exist or is invalid
pub fn load_state(root: &Path) -> WizardLocalState {
    let path = state_path(root);

    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => WizardLocalState::default(),
    }
}

/// Save local state to `.wizard/state.json`
/// Creates parent directories as needed
pub fn save_state(root: &Path, state: &WizardLocalState) -> io::Result<()> {
    let path = state_path(root);

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(state)?;
    fs::write(&path, content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_default_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let state = load_state(root);

        assert_eq!(state.open_views, Vec::<String>::new());
        assert_eq!(state.last_project, None);
    }

    #[test]
    fn test_save_and_load_state() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let original_state = WizardLocalState {
            open_views: vec!["view1".to_string(), "view2".to_string()],
            last_project: Some("my_project".to_string()),
        };

        // Save state
        save_state(root, &original_state).unwrap();

        // Verify file was created
        let state_file = state_path(root);
        assert!(state_file.exists());

        // Load state back
        let loaded_state = load_state(root);

        assert_eq!(loaded_state.open_views, original_state.open_views);
        assert_eq!(loaded_state.last_project, original_state.last_project);
    }

    #[test]
    fn test_load_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create .wizard directory and write invalid JSON
        fs::create_dir_all(root.join(".wizard")).unwrap();
        fs::write(state_path(root), "invalid json").unwrap();

        // Should return default state for invalid JSON
        let state = load_state(root);

        assert_eq!(state.open_views, Vec::<String>::new());
        assert_eq!(state.last_project, None);
    }

    #[test]
    fn test_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Ensure .wizard directory doesn't exist initially
        assert!(!root.join(".wizard").exists());

        let state = WizardLocalState::default();
        save_state(root, &state).unwrap();

        // Verify directory was created
        assert!(root.join(".wizard").exists());
        assert!(state_path(root).exists());
    }
}
