//! Git diff computation and file change analysis.
//!
//! Wraps `git diff` to compute what changed for a unit's work,
//! and parses the output into structured [`FileChange`] records.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::types::{ChangeType, FileChange};

/// Compute the diff for a unit's work.
///
/// Uses `git diff` against the unit's checkpoint (if available) or HEAD~1.
/// Returns the raw unified diff and parsed file changes.
pub fn compute(project_root: &Path, checkpoint: Option<&str>) -> Result<(String, Vec<FileChange>)> {
    let base_ref = checkpoint.unwrap_or("HEAD~1");

    // Get the raw diff
    let diff_output = Command::new("git")
        .args(["diff", base_ref, "HEAD"])
        .current_dir(project_root)
        .output()
        .context("failed to run git diff")?;

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    // Get the stat for structured file changes
    let stat_output = Command::new("git")
        .args(["diff", "--numstat", base_ref, "HEAD"])
        .current_dir(project_root)
        .output()
        .context("failed to run git diff --numstat")?;

    let stat = String::from_utf8_lossy(&stat_output.stdout);
    let file_changes = parse_numstat(&stat, project_root, base_ref)?;

    Ok((diff, file_changes))
}

/// Compute diff between two specific refs.
pub fn compute_between(
    project_root: &Path,
    from_ref: &str,
    to_ref: &str,
) -> Result<(String, Vec<FileChange>)> {
    let diff_output = Command::new("git")
        .args(["diff", from_ref, to_ref])
        .current_dir(project_root)
        .output()
        .context("failed to run git diff")?;

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    let stat_output = Command::new("git")
        .args(["diff", "--numstat", from_ref, to_ref])
        .current_dir(project_root)
        .output()
        .context("failed to run git diff --numstat")?;

    let stat = String::from_utf8_lossy(&stat_output.stdout);
    let file_changes = parse_numstat(&stat, project_root, from_ref)?;

    Ok((diff, file_changes))
}

/// Get the raw unified diff as a string.
pub fn raw_diff(project_root: &Path, base_ref: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", base_ref, "HEAD"])
        .current_dir(project_root)
        .output()
        .context("failed to run git diff")?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse `git diff --numstat` output into FileChange records.
fn parse_numstat(stat: &str, project_root: &Path, base_ref: &str) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();

    for line in stat.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }

        // Binary files show as "-" for additions/deletions
        let additions = parts[0].parse::<u32>().unwrap_or(0);
        let deletions = parts[1].parse::<u32>().unwrap_or(0);
        let path = parts[2].to_string();

        // Handle renames: "old => new" format
        let (path, change_type) = if path.contains(" => ") {
            let new_path = path.split(" => ").last().unwrap_or(&path);
            (new_path.to_string(), ChangeType::Renamed)
        } else {
            let ct = determine_change_type(project_root, base_ref, &path);
            (path, ct)
        };

        changes.push(FileChange {
            path,
            change_type,
            additions,
            deletions,
        });
    }

    Ok(changes)
}

/// Determine if a file was added, modified, or deleted.
fn determine_change_type(project_root: &Path, base_ref: &str, path: &str) -> ChangeType {
    // Check if the file exists in the base ref
    let in_base = Command::new("git")
        .args(["cat-file", "-e", &format!("{base_ref}:{path}")])
        .current_dir(project_root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Check if the file exists in HEAD
    let in_head = Command::new("git")
        .args(["cat-file", "-e", &format!("HEAD:{path}")])
        .current_dir(project_root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    match (in_base, in_head) {
        (false, true) => ChangeType::Added,
        (true, false) => ChangeType::Deleted,
        _ => ChangeType::Modified,
    }
}
