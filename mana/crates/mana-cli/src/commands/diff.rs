use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::discovery::find_unit_file;
use crate::unit::Unit;

/// Output mode for the diff command.
pub enum DiffOutput {
    /// Full unified diff (default).
    Full,
    /// --stat: file-level summary.
    Stat,
    /// --name-only: just filenames.
    NameOnly,
}

/// Show git diff of what changed for a specific unit.
///
/// Strategy:
/// 1. Look for commits with `unit-{id}` in the message (preferred auto-commit
///    convention), plus legacy `Close unit {id}` messages. If found, show the
///    combined diff for those commits.
/// 2. Fall back to timestamp-based diffing: find the commit closest to
///    claimed_at and diff to closed_at (or HEAD if still open).
/// 3. If the unit has a checkpoint SHA, use that as the base.
pub fn cmd_diff(mana_dir: &Path, id: &str, output: DiffOutput, no_color: bool) -> Result<()> {
    let unit_path =
        find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;
    let unit =
        Unit::from_file(&unit_path).with_context(|| format!("Failed to load unit: {}", id))?;

    let project_root = mana_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project root from .mana/ dir"))?;

    // Ensure we're in a git repo
    if !is_git_repo(project_root) {
        anyhow::bail!("Not a git repository. bn diff requires git.");
    }

    // Strategy 1: Find commits by message convention (auto_commit)
    let tagged_commits = find_commits_for_unit(project_root, id)?;
    if !tagged_commits.is_empty() {
        return show_commit_diff(project_root, &tagged_commits, &output, no_color);
    }

    // Strategy 2: Use checkpoint SHA if available (recorded at claim time)
    if let Some(ref checkpoint) = unit.checkpoint {
        let end_ref = resolve_end_ref(&unit, project_root)?;
        return show_range_diff(project_root, checkpoint, &end_ref, &output, no_color);
    }

    // Strategy 3: Timestamp-based diffing
    let start_time = unit
        .claimed_at
        .or(Some(unit.created_at))
        .ok_or_else(|| anyhow::anyhow!("Unit has no claim or creation timestamp"))?;

    let start_commit = find_commit_at_time(project_root, &start_time.to_rfc3339())?;
    match start_commit {
        Some(sha) => {
            let end_ref = resolve_end_ref(&unit, project_root)?;
            show_range_diff(project_root, &sha, &end_ref, &output, no_color)
        }
        None => {
            eprintln!("No changes found for unit {}", id);
            Ok(())
        }
    }
}

/// Check if a directory is inside a git repository.
fn is_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Find commits whose message references a unit ID.
///
/// Looks for the preferred auto-commit convention `unit-{id}` plus legacy
/// `Close unit {id}: ...` messages for backward compatibility.
fn find_commits_for_unit(project_root: &Path, id: &str) -> Result<Vec<String>> {
    // Search for commits mentioning this unit ID in the message
    let patterns = [
        format!("Close unit {}: ", id),
        format!("Close unit {}:", id),
        format!("unit-{}", id),
    ];

    let mut commits = Vec::new();
    for pattern in &patterns {
        let output = Command::new("git")
            .args(["log", "--all", "--format=%H", "--grep", pattern])
            .current_dir(project_root)
            .output()
            .context("Failed to run git log")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let sha = line.trim();
                if !sha.is_empty() && !commits.contains(&sha.to_string()) {
                    commits.push(sha.to_string());
                }
            }
        }
    }

    Ok(commits)
}

/// Determine the end ref for a diff range.
///
/// - Closed units: use the commit at closed_at time (or HEAD).
/// - Open/in-progress units: use HEAD (shows working tree changes).
fn resolve_end_ref(unit: &Unit, project_root: &Path) -> Result<String> {
    if let Some(closed_at) = &unit.closed_at {
        // Find the commit closest to close time
        match find_commit_at_time(project_root, &closed_at.to_rfc3339())? {
            Some(sha) => Ok(sha),
            None => Ok("HEAD".to_string()),
        }
    } else {
        Ok("HEAD".to_string())
    }
}

/// Find the commit closest to (at or before) a given timestamp.
fn find_commit_at_time(project_root: &Path, timestamp: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .args([
            "log",
            "-1",
            "--format=%H",
            &format!("--before={}", timestamp),
        ])
        .current_dir(project_root)
        .output()
        .context("Failed to run git log")?;

    if output.status.success() {
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if sha.is_empty() {
            Ok(None)
        } else {
            Ok(Some(sha))
        }
    } else {
        Ok(None)
    }
}

/// Show diff for specific commits (auto_commit mode).
///
/// When there's a single commit, shows that commit's diff.
/// When there are multiple commits, shows the combined range.
fn show_commit_diff(
    project_root: &Path,
    commits: &[String],
    output: &DiffOutput,
    no_color: bool,
) -> Result<()> {
    if commits.is_empty() {
        return Ok(());
    }

    let mut args = vec!["diff".to_string()];
    add_output_flags(&mut args, output, no_color);

    if commits.len() == 1 {
        // Single commit: show its diff
        args = vec!["show".to_string()];
        add_output_flags(&mut args, output, no_color);
        if matches!(output, DiffOutput::Full) {
            args.push("--format=".to_string()); // suppress commit header for clean diff
        }
        args.push(commits[0].clone());
    } else {
        // Multiple commits: find the range from earliest parent to latest
        // Sort by commit date and diff from earliest^..latest
        let earliest = commits.last().unwrap(); // git log returns newest first
        let latest = &commits[0];
        args.push(format!("{}^..{}", earliest, latest));
    }

    run_git_to_stdout(project_root, &args)
}

/// Show diff between two refs.
fn show_range_diff(
    project_root: &Path,
    from: &str,
    to: &str,
    output: &DiffOutput,
    no_color: bool,
) -> Result<()> {
    let mut args = vec!["diff".to_string()];
    add_output_flags(&mut args, output, no_color);
    args.push(from.to_string());
    args.push(to.to_string());
    run_git_to_stdout(project_root, &args)
}

/// Add output mode flags to a git command.
fn add_output_flags(args: &mut Vec<String>, output: &DiffOutput, no_color: bool) {
    match output {
        DiffOutput::Stat => args.push("--stat".to_string()),
        DiffOutput::NameOnly => args.push("--name-only".to_string()),
        DiffOutput::Full => {}
    }

    if no_color {
        args.push("--no-color".to_string());
    } else {
        args.push("--color=auto".to_string());
    }
}

/// Run a git command and pipe output to stdout.
fn run_git_to_stdout(project_root: &Path, args: &[String]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(project_root)
        .status()
        .context("Failed to run git")?;

    if !status.success() {
        // Non-zero exit from git diff usually means "no differences" — not an error
        if status.code() == Some(1) {
            return Ok(());
        }
        anyhow::bail!("git exited with code {}", status.code().unwrap_or(-1));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Set up a temp dir with a git repo and a .mana/ directory containing a unit.
    fn setup_git_repo() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path();
        let mana_dir = project_root.join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Init git repo
        run_git(project_root, &["init"]);
        run_git(project_root, &["config", "user.email", "test@test.com"]);
        run_git(project_root, &["config", "user.name", "Test"]);

        // Initial commit
        fs::write(project_root.join("initial.txt"), "initial").unwrap();
        run_git(project_root, &["add", "-A"]);
        run_git(project_root, &["commit", "-m", "Initial commit"]);

        (dir, mana_dir)
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    }

    fn write_unit(mana_dir: &Path, unit: &Unit) {
        let path = mana_dir.join(format!("{}-test.md", unit.id));
        unit.to_file(&path).unwrap();
    }

    #[test]
    fn is_git_repo_true_for_git_dir() {
        let (dir, _) = setup_git_repo();
        assert!(is_git_repo(dir.path()));
    }

    #[test]
    fn is_git_repo_false_for_non_git_dir() {
        let dir = TempDir::new().unwrap();
        assert!(!is_git_repo(dir.path()));
    }

    #[test]
    fn find_commits_for_unit_finds_matching_commits() {
        let (dir, mana_dir) = setup_git_repo();
        let project_root = mana_dir.parent().unwrap();

        // Create a commit with the auto_commit convention
        fs::write(project_root.join("feature.txt"), "new feature").unwrap();
        run_git(project_root, &["add", "-A"]);
        run_git(project_root, &["commit", "-m", "feat(unit-5): add feature"]);

        let commits = find_commits_for_unit(project_root, "5").unwrap();
        assert_eq!(commits.len(), 1);

        // Should NOT match unrelated units
        let commits_other = find_commits_for_unit(project_root, "99").unwrap();
        assert!(commits_other.is_empty());

        drop(dir);
    }

    #[test]
    fn find_commits_ignores_partial_id_matches() {
        let (dir, mana_dir) = setup_git_repo();
        let project_root = mana_dir.parent().unwrap();

        // Commit for unit 5 should NOT match unit 50
        fs::write(project_root.join("f.txt"), "content").unwrap();
        run_git(project_root, &["add", "-A"]);
        run_git(project_root, &["commit", "-m", "feat(unit-5): something"]);

        let commits = find_commits_for_unit(project_root, "50").unwrap();
        assert!(commits.is_empty());

        drop(dir);
    }

    #[test]
    fn cmd_diff_no_git_repo_fails() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let unit = Unit::new("1", "Test");
        let path = mana_dir.join("1-test.md");
        unit.to_file(&path).unwrap();

        let result = cmd_diff(&mana_dir, "1", DiffOutput::Full, false);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("git"), "Expected git error, got: {}", err);
    }

    #[test]
    fn cmd_diff_with_tagged_commit_succeeds() {
        let (dir, mana_dir) = setup_git_repo();
        let project_root = mana_dir.parent().unwrap();

        // Create unit
        let unit = Unit::new("3", "Add login");
        write_unit(&mana_dir, &unit);

        // Make a change and commit with auto_commit convention
        fs::write(project_root.join("login.rs"), "fn login() {}").unwrap();
        run_git(project_root, &["add", "-A"]);
        run_git(project_root, &["commit", "-m", "feat(unit-3): Add login"]);

        // Should succeed (output goes to stdout)
        let result = cmd_diff(&mana_dir, "3", DiffOutput::Stat, true);
        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn cmd_diff_with_checkpoint_succeeds() {
        let (dir, mana_dir) = setup_git_repo();
        let project_root = mana_dir.parent().unwrap();

        // Get current HEAD as checkpoint
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(project_root)
            .output()
            .unwrap();
        let checkpoint = String::from_utf8_lossy(&head.stdout).trim().to_string();

        // Create unit with checkpoint
        let mut unit = Unit::new("7", "Refactor auth");
        unit.checkpoint = Some(checkpoint);
        write_unit(&mana_dir, &unit);

        // Make a change and commit
        fs::write(project_root.join("auth.rs"), "fn auth() {}").unwrap();
        run_git(project_root, &["add", "-A"]);
        run_git(project_root, &["commit", "-m", "refactor auth"]);

        let result = cmd_diff(&mana_dir, "7", DiffOutput::Full, true);
        assert!(result.is_ok());

        drop(dir);
    }

    #[test]
    fn cmd_diff_nonexistent_unit_fails() {
        let (_dir, mana_dir) = setup_git_repo();
        let result = cmd_diff(&mana_dir, "999", DiffOutput::Full, false);
        assert!(result.is_err());
    }

    #[test]
    fn find_commit_at_time_returns_none_for_future() {
        let (dir, _) = setup_git_repo();
        // Far future — should still return the latest commit
        let result = find_commit_at_time(dir.path(), "2099-01-01T00:00:00Z").unwrap();
        assert!(result.is_some());

        drop(dir);
    }

    #[test]
    fn add_output_flags_stat() {
        let mut args = Vec::new();
        add_output_flags(&mut args, &DiffOutput::Stat, false);
        assert!(args.contains(&"--stat".to_string()));
        assert!(args.contains(&"--color=auto".to_string()));
    }

    #[test]
    fn add_output_flags_name_only_no_color() {
        let mut args = Vec::new();
        add_output_flags(&mut args, &DiffOutput::NameOnly, true);
        assert!(args.contains(&"--name-only".to_string()));
        assert!(args.contains(&"--no-color".to_string()));
    }

    #[test]
    fn add_output_flags_full_default() {
        let mut args = Vec::new();
        add_output_flags(&mut args, &DiffOutput::Full, false);
        assert!(!args.contains(&"--stat".to_string()));
        assert!(!args.contains(&"--name-only".to_string()));
        assert!(args.contains(&"--color=auto".to_string()));
    }
}
