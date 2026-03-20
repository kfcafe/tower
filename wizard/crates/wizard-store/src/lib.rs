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
