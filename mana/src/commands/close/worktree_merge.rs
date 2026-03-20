use std::path::Path;

use anyhow::Result;

use crate::unit::Unit;
use crate::worktree;

/// Detect and validate worktree context for the current process.
///
/// Returns Some(WorktreeInfo) if we're in a valid secondary worktree that
/// belongs to the given project root. Returns None if we're not in a worktree,
/// or if the detected worktree doesn't match the project.
pub(crate) fn detect_valid_worktree(
    project_root: &Path,
) -> Option<worktree::WorktreeInfo> {
    let info = worktree::detect_worktree().unwrap_or(None)?;

    let canonical_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    if canonical_root.starts_with(&info.worktree_path) {
        Some(info)
    } else {
        None
    }
}

/// Commit worktree changes and merge to main.
///
/// Returns Ok(true) if merge succeeded or there was nothing to merge.
/// Returns Ok(false) if there was a conflict (caller should abort the close).
pub(crate) fn handle_merge(
    wt_info: &worktree::WorktreeInfo,
    unit: &Unit,
) -> Result<bool> {
    // Commit any uncommitted changes
    worktree::commit_worktree_changes(&format!("Close unit {}: {}", unit.id, unit.title))?;

    // Merge to main
    match worktree::merge_to_main(wt_info, &unit.id)? {
        worktree::MergeResult::Success | worktree::MergeResult::NothingToCommit => Ok(true),
        worktree::MergeResult::Conflict { files } => {
            eprintln!("Merge conflict in files: {:?}", files);
            eprintln!("Resolve conflicts and run `mana close {}` again", unit.id);
            Ok(false)
        }
    }
}

/// Clean up worktree after successful close.
pub(crate) fn cleanup(wt_info: &worktree::WorktreeInfo) {
    if let Err(e) = worktree::cleanup_worktree(wt_info) {
        eprintln!("Warning: failed to clean up worktree: {}", e);
    }
}

/// Auto-commit changes on close (non-worktree mode).
pub(crate) fn auto_commit_on_close(project_root: &Path, id: &str, title: &str) {
    let message = format!("Close unit {}: {}", id, title);

    // Stage all changes
    let add_status = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status();

    match add_status {
        Ok(s) if !s.success() => {
            eprintln!("Warning: git add -A failed (exit {})", s.code().unwrap_or(-1));
            return;
        }
        Err(e) => {
            eprintln!("Warning: git add -A failed: {}", e);
            return;
        }
        _ => {}
    }

    // Commit (exit code 1 = nothing to commit, which is fine)
    let commit_result = std::process::Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    match commit_result {
        Ok(output) if output.status.success() => {
            eprintln!("auto_commit: {}", message);
        }
        Ok(output) if output.status.code() == Some(1) => {
            // Nothing to commit — silent
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "Warning: git commit failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }
        Err(e) => {
            eprintln!("Warning: git commit failed: {}", e);
        }
    }
}
