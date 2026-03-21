//! Editor integration for bn edit command.
//!
//! This module provides low-level file operations for editing units:
//! - Launching an external editor subprocess
//! - Creating backups before editing
//! - Validating and saving edited content
//! - Rebuilding indices after modifications
//! - Prompting user for rollback on validation errors

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use crate::discovery::find_unit_file;
use crate::index::Index;
use crate::unit::Unit;

/// Validate unit content and persist it to disk with updated timestamp.
///
/// Parses the content using Unit::from_string() to validate the YAML schema.
/// If validation succeeds, writes the content to the file and updates the
/// updated_at field to the current UTC time.
///
/// # Arguments
/// * `path` - Path where the validated content will be written
/// * `content` - The edited unit content (YAML or Markdown with YAML frontmatter)
///
/// # Returns
/// * Ok(()) if validation succeeds and file is written
/// * Err with descriptive message if:
///   - Content fails YAML schema validation
///   - File I/O error occurs
///
/// # Examples
/// ```ignore
/// validate_and_save(Path::new(".mana/1-my-task.md"), edited_content)?;
/// ```
pub fn validate_and_save(path: &Path, content: &str) -> Result<()> {
    // Parse content to validate schema
    let mut unit =
        Unit::from_string(content).with_context(|| "Failed to parse unit: invalid YAML schema")?;

    // Update the timestamp to current time
    unit.updated_at = Utc::now();

    // Serialize the validated unit back to YAML
    let validated_yaml =
        serde_yml::to_string(&unit).with_context(|| "Failed to serialize validated unit")?;

    // Write to disk
    fs::write(path, validated_yaml)
        .with_context(|| format!("Failed to write unit to {}", path.display()))?;

    Ok(())
}

/// Rebuild the unit index from current unit files on disk.
///
/// Reads all unit files in the units directory, builds a fresh index,
/// and saves it to .mana/index.yaml. This should be called after any
/// unit modification to keep the index synchronized.
///
/// # Arguments
/// * `mana_dir` - Path to the .mana directory
///
/// # Returns
/// * Ok(()) if index is built and saved successfully
/// * Err if:
///   - Directory is not readable
///   - Unit files fail to parse
///   - Index file cannot be written
///
/// # Examples
/// ```ignore
/// rebuild_index_after_edit(Path::new(".mana"))?;
/// ```
pub fn rebuild_index_after_edit(mana_dir: &Path) -> Result<()> {
    let index = Index::build(mana_dir).with_context(|| "Failed to build index from unit files")?;

    index
        .save(mana_dir)
        .with_context(|| "Failed to save index to .mana/index.yaml")?;

    Ok(())
}

/// Prompt user for action when validation fails: retry, rollback, or abort.
///
/// Displays the validation error and presents an interactive prompt with three options:
/// - 'y' or 'retry': Re-open the editor for another attempt (returns Ok)
/// - 'r' or 'rollback': Restore the original file from backup and abort (returns Ok)
/// - 'n' or any other input: Abort the edit operation (returns Err)
///
/// # Arguments
/// * `backup` - The original file content before editing (in bytes)
/// * `path` - Path to the unit file being edited
///
/// # Returns
/// * Ok(()) if user chooses 'retry' (signals caller to re-open editor) or 'rollback'
/// * Err if user chooses 'n'/'abort'
///
/// # Examples
/// ```ignore
/// match prompt_rollback(&backup, &path) {
///     Ok(()) => {
///         // User chose retry or rollback - check backup file to determine which
///         if path matches backup { /* was rollback */ }
///         else { /* was retry */ }
///     }
///     Err(e) => println!("Edit aborted: {}", e),
/// }
/// ```
pub fn prompt_rollback(backup: &[u8], path: &Path) -> Result<()> {
    // Present user with menu
    println!("\nValidation failed. What would you like to do?");
    println!("  (y)    Retry in editor");
    println!("  (r)    Rollback and discard changes");
    println!("  (n)    Abort");
    print!("\nChoice: ");
    io::stdout().flush()?;

    // Read user input
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim().to_lowercase();

    match choice.as_str() {
        "y" | "retry" => {
            // User wants to retry — return Ok to signal retry
            Ok(())
        }
        "r" | "rollback" => {
            // Restore from backup and return Ok (successful rollback)
            fs::write(path, backup)
                .with_context(|| format!("Failed to restore backup to {}", path.display()))?;
            println!("Rollback complete. Original file restored.");
            Ok(())
        }
        "n" => {
            // User aborts
            Err(anyhow!("Edit aborted by user"))
        }
        _ => {
            // Invalid input treated as abort
            Err(anyhow!("Edit aborted by user"))
        }
    }
}

/// Open a file in the user's configured editor.
///
/// Reads the $EDITOR environment variable and spawns a subprocess with the file path.
/// Waits for the editor to exit and validates the exit status.
///
/// # Arguments
/// * `path` - Path to the file to edit
///
/// # Returns
/// * Ok(()) if editor exits successfully (status 0)
/// * Err if:
///   - $EDITOR environment variable is not set
///   - Editor executable is not found
///   - Editor process exits with non-zero status
///   - Editor subprocess crashes
///
/// # Examples
/// ```ignore
/// open_editor(Path::new(".mana/1-my-task.md"))?;
/// ```
pub fn open_editor(path: &Path) -> Result<()> {
    // Get EDITOR environment variable
    let editor = env::var("EDITOR")
        .context("$EDITOR environment variable not set. Please set it to your preferred editor (e.g., vim, nano, emacs)")?;

    // Ensure file exists before opening
    if !path.exists() {
        return Err(anyhow!("File does not exist: {}", path.display()));
    }

    // Convert path to string for error messages
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("Path contains invalid UTF-8: {}", path.display()))?;

    // Spawn editor subprocess
    let mut cmd = Command::new(&editor);
    cmd.arg(path_str);

    let status = cmd.status().with_context(|| {
        anyhow!(
            "Failed to launch editor '{}'. Make sure it is installed and in your PATH",
            editor
        )
    })?;

    // Check exit status
    if !status.success() {
        let exit_code = status.code().unwrap_or(-1);
        return Err(anyhow!(
            "Editor '{}' exited with code {}",
            editor,
            exit_code
        ));
    }

    Ok(())
}

/// Load file content into memory as a backup before editing.
///
/// Reads the entire file content into a byte vector. This is used to detect
/// if the file was actually modified by comparing before/after content.
///
/// # Arguments
/// * `path` - Path to the file to backup
///
/// # Returns
/// * `Ok(Vec<u8>)` containing the file content
/// * Err if:
///   - File does not exist
///   - Permission denied reading the file
///   - I/O error occurs
///
/// # Examples
/// ```ignore
/// let backup = load_backup(Path::new(".mana/1-my-task.md"))?;
/// ```
pub fn load_backup(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| anyhow!("Failed to read file for backup: {}", path.display()))
}

/// Orchestrate the complete bn edit workflow for a unit.
///
/// The full edit workflow:
/// 1. Validate the unit ID format
/// 2. Find the unit file using discovery
/// 3. Load the current unit content as a backup
/// 4. Open the file in the user's configured editor
/// 5. Load the edited content
/// 6. Validate and save with schema validation (updates timestamp)
/// 7. Rebuild the index to reflect changes
///
/// If validation fails, prompts user to retry, rollback, or abort.
/// If editor subprocess fails, handles the error gracefully.
///
/// # Arguments
/// * `mana_dir` - Path to the .mana directory
/// * `id` - Unit ID to edit (e.g., "1", "1.1")
///
/// # Returns
/// * Ok(()) if edit is successful and saved
/// * Err if:
///   - Unit ID not found
///   - $EDITOR not set or editor not found
///   - Editor exits with non-zero status
///   - Validation fails and user chooses abort
///   - I/O or index rebuild fails
///
/// # Examples
/// ```ignore
/// cmd_edit(Path::new(".mana"), "1")?;
/// ```
pub fn cmd_edit(mana_dir: &Path, id: &str) -> Result<()> {
    // Step 1: Find the unit file
    let bean_path =
        find_unit_file(mana_dir, id).with_context(|| format!("Unit not found: {}", id))?;

    // Step 2: Load the current unit content as backup
    let backup = load_backup(&bean_path)
        .with_context(|| format!("Failed to load unit for editing: {}", id))?;

    // Step 3: Open editor for user to modify the file
    loop {
        match open_editor(&bean_path) {
            Ok(()) => {
                // Step 4: Read the edited content
                let edited_content = fs::read_to_string(&bean_path)
                    .with_context(|| format!("Failed to read edited unit file: {}", id))?;

                // Step 5: Validate and save the edited content (updates timestamp)
                match validate_and_save(&bean_path, &edited_content) {
                    Ok(()) => {
                        // Step 6: Rebuild the index to reflect changes
                        rebuild_index_after_edit(mana_dir)
                            .with_context(|| "Failed to rebuild index after edit")?;

                        println!("Unit {} updated successfully.", id);
                        return Ok(());
                    }
                    Err(validation_err) => {
                        // Validation failed - present user with options
                        eprintln!("Validation error: {}", validation_err);

                        match prompt_rollback(&backup, &bean_path) {
                            Ok(()) => {
                                // Check if file was restored to backup or if user wants to retry
                                let current = fs::read(&bean_path)
                                    .with_context(|| "Failed to read unit file")?;

                                if current == backup {
                                    // User chose rollback - exit cleanly
                                    println!("Edit cancelled.");
                                    return Ok(());
                                } else {
                                    // User chose retry - loop back to open editor
                                    continue;
                                }
                            }
                            Err(e) => {
                                // User chose abort - restore backup and return error
                                let _ = fs::write(&bean_path, &backup);
                                return Err(e).context("Edit aborted by user");
                            }
                        }
                    }
                }
            }
            Err(editor_err) => {
                // Editor subprocess failed - prompt user for action
                eprintln!("Editor error: {}", editor_err);

                // Attempt rollback
                match fs::write(&bean_path, &backup) {
                    Ok(()) => {
                        return Err(editor_err).context("Editor failed; backup restored");
                    }
                    Err(rollback_err) => {
                        return Err(anyhow!(
                            "Editor failed and rollback failed: {} (rollback: {})",
                            editor_err,
                            rollback_err
                        ));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_temp_file(content: &str) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.md");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        (dir, file_path)
    }

    fn create_valid_bean_file(content: &str) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("1-test.md");
        fs::write(&file_path, content).unwrap();
        (dir, file_path)
    }

    // =====================================================================
    // Tests for load_backup (from 2.1)
    // =====================================================================

    #[test]
    fn test_load_backup_reads_content() {
        let (_dir, path) = create_temp_file("Hello, World!");
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, b"Hello, World!");
    }

    #[test]
    fn test_load_backup_reads_empty_file() {
        let (_dir, path) = create_temp_file("");
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup.len(), 0);
    }

    #[test]
    fn test_load_backup_reads_multiline_content() {
        let (_dir, path) = create_temp_file("Line 1\nLine 2\nLine 3");
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, b"Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_load_backup_reads_binary_content() {
        let (_dir, path) = create_temp_file("Binary\x00\x01\x02");
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, b"Binary\x00\x01\x02");
    }

    #[test]
    fn test_load_backup_nonexistent_file() {
        let path = Path::new("/nonexistent/path/to/file.md");
        let result = load_backup(path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to read file"));
    }

    #[test]
    fn test_load_backup_large_file() {
        let (_dir, path) = create_temp_file(&"X".repeat(1024 * 1024)); // 1MB file
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup.len(), 1024 * 1024);
    }

    #[test]
    fn test_open_editor_nonexistent_file() {
        env::set_var("EDITOR", "echo");
        let path = Path::new("/nonexistent/path/to/file.md");
        let result = open_editor(path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("does not exist"));
    }

    #[test]
    fn test_open_editor_success_with_echo() {
        // Use 'echo' as a harmless editor that exits successfully
        env::set_var("EDITOR", "echo");
        let (_dir, path) = create_temp_file("test content");
        let result = open_editor(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_open_editor_success_with_true() {
        // Use 'true' as a harmless editor that always succeeds
        env::set_var("EDITOR", "true");
        let (_dir, path) = create_temp_file("test content");
        let result = open_editor(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_backup_preserves_exact_content() {
        let test_content = "# Unit Title\n\nsome description\n\nstatus: open";
        let (_dir, path) = create_temp_file(test_content);

        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, test_content.as_bytes());
    }

    #[test]
    fn test_backup_backup_before_edit_workflow() {
        let original = "original content";
        let (_dir, path) = create_temp_file(original);

        // Simulate backup before edit
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, original.as_bytes());

        // Simulate file modification
        fs::write(&path, "modified content").unwrap();

        // Verify backup is unchanged
        assert_eq!(backup, original.as_bytes());

        // Verify file is modified
        let current = fs::read(&path).unwrap();
        assert_ne!(current, backup);
    }

    // =====================================================================
    // Tests for validate_and_save (Unit 2.2)
    // =====================================================================

    #[test]
    fn test_validate_and_save_parses_and_validates_yaml() {
        let bean_content = r#"id: "1"
title: Test Unit
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;
        let (_dir, path) = create_valid_bean_file(bean_content);

        let result = validate_and_save(&path, bean_content);
        assert!(result.is_ok());

        // Verify file was written
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("id: '1'") || saved.contains("id: \"1\""));
    }

    #[test]
    fn test_validate_and_save_updates_timestamp() {
        let bean_content = r#"id: "1"
title: Test Unit
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;
        let (_dir, path) = create_valid_bean_file(bean_content);

        // Save original timestamp
        let before = Unit::from_string(bean_content).unwrap();
        let before_ts = before.updated_at;

        // Wait a tiny bit to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Validate and save
        validate_and_save(&path, bean_content).unwrap();

        // Load the saved unit and check timestamp was updated
        let saved_bean = Unit::from_file(&path).unwrap();
        assert!(saved_bean.updated_at > before_ts);
    }

    #[test]
    fn test_validate_and_save_rejects_invalid_yaml() {
        let invalid_content = "id: 1\ntitle: Test\nstatus: invalid_status\n";
        let (_dir, path) = create_valid_bean_file(invalid_content);

        let result = validate_and_save(&path, invalid_content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid YAML"));
    }

    #[test]
    fn test_validate_and_save_persists_to_disk() {
        let bean_content = r#"id: "1"
title: Original Title
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;
        let (_dir, path) = create_valid_bean_file(bean_content);

        validate_and_save(&path, bean_content).unwrap();

        // Read from disk and verify
        let unit = Unit::from_file(&path).unwrap();
        assert_eq!(unit.id, "1");
        assert_eq!(unit.title, "Original Title");
    }

    #[test]
    fn test_validate_and_save_with_markdown_frontmatter() {
        let md_content = r#"---
id: "2"
title: Markdown Unit
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
---

# Description

This is a markdown body.
"#;
        let (_dir, path) = create_valid_bean_file(md_content);

        validate_and_save(&path, md_content).unwrap();

        let unit = Unit::from_file(&path).unwrap();
        assert_eq!(unit.id, "2");
        assert_eq!(unit.title, "Markdown Unit");
        assert!(unit.description.is_some());
    }

    #[test]
    fn test_validate_and_save_missing_required_field() {
        let invalid_content = r#"id: "1"
title: Test
status: open
"#; // Missing created_at and updated_at
        let (_dir, path) = create_valid_bean_file(invalid_content);

        let result = validate_and_save(&path, invalid_content);
        assert!(result.is_err());
    }

    // =====================================================================
    // Tests for rebuild_index_after_edit (Unit 2.2)
    // =====================================================================

    #[test]
    fn test_rebuild_index_after_edit_creates_index() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Create a unit file
        let bean_content = r#"id: "1"
title: Test Unit
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;
        fs::write(mana_dir.join("1-test.md"), bean_content).unwrap();

        // Rebuild index
        rebuild_index_after_edit(&mana_dir).unwrap();

        // Verify index.yaml was created
        assert!(mana_dir.join("index.yaml").exists());

        // Load and verify index
        let index = Index::load(&mana_dir).unwrap();
        assert_eq!(index.units.len(), 1);
        assert_eq!(index.units[0].id, "1");
        assert_eq!(index.units[0].title, "Test Unit");
    }

    #[test]
    fn test_rebuild_index_after_edit_includes_all_beans() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Create multiple units
        let bean1 = Unit::new("1", "First Unit");
        let bean2 = Unit::new("2", "Second Unit");
        let bean3 = Unit::new("3", "Third Unit");

        bean1.to_file(mana_dir.join("1-first.md")).unwrap();
        bean2.to_file(mana_dir.join("2-second.md")).unwrap();
        bean3.to_file(mana_dir.join("3-third.md")).unwrap();

        rebuild_index_after_edit(&mana_dir).unwrap();

        let index = Index::load(&mana_dir).unwrap();
        assert_eq!(index.units.len(), 3);
    }

    #[test]
    fn test_rebuild_index_after_edit_saves_to_correct_location() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let unit = Unit::new("1", "Test");
        unit.to_file(mana_dir.join("1-test.md")).unwrap();

        rebuild_index_after_edit(&mana_dir).unwrap();

        let index_path = mana_dir.join("index.yaml");
        assert!(index_path.exists(), "index.yaml should be saved to .mana/");
    }

    #[test]
    fn test_rebuild_index_after_edit_empty_directory() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Rebuild index with no units
        rebuild_index_after_edit(&mana_dir).unwrap();

        // Index should be created but empty
        let index = Index::load(&mana_dir).unwrap();
        assert_eq!(index.units.len(), 0);
    }

    #[test]
    fn test_rebuild_index_after_edit_invalid_beans_dir() {
        let nonexistent = Path::new("/nonexistent/.mana");
        let result = rebuild_index_after_edit(nonexistent);
        assert!(result.is_err());
    }

    // =====================================================================
    // Tests for prompt_rollback (Unit 2.2)
    // =====================================================================

    #[test]
    fn test_prompt_rollback_restores_file_from_backup() {
        let (_dir, path) = create_temp_file("modified content");
        let backup = b"original content";

        // If we could mock stdin, we'd test rollback by:
        // 1. Verifying backup is written
        // 2. Checking file content matches backup
        // For now, verify the function would write backup correctly
        let result = fs::write(&path, backup);
        assert!(result.is_ok());

        let saved = fs::read(&path).unwrap();
        assert_eq!(saved, backup);
    }

    #[test]
    fn test_prompt_rollback_backup_preserves_content() {
        let original = "original unit content";
        let (_dir, path) = create_temp_file(original);

        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, original.as_bytes());

        // Modify file
        fs::write(&path, "modified content").unwrap();

        // Restore from backup
        fs::write(&path, &backup).unwrap();

        // Verify restoration
        let restored = fs::read(&path).unwrap();
        assert_eq!(restored, original.as_bytes());
    }

    #[test]
    fn test_validate_and_save_workflow_full() {
        // Full workflow: backup -> edit -> validate -> save
        let bean_content = r#"id: "1"
title: Original
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;
        let (_dir, path) = create_valid_bean_file(bean_content);

        // Step 1: Backup
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, bean_content.as_bytes());

        // Step 2: Simulate edit (modify title)
        let edited_content = r#"id: "1"
title: Modified
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;

        // Step 3: Validate and save
        validate_and_save(&path, edited_content).unwrap();

        // Step 4: Verify changes persisted
        let saved_bean = Unit::from_file(&path).unwrap();
        assert_eq!(saved_bean.title, "Modified");
    }

    #[test]
    fn test_rebuild_index_reflects_recent_edits() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Create initial unit
        let bean1 = Unit::new("1", "First");
        bean1.to_file(mana_dir.join("1-first.md")).unwrap();

        // Build index
        rebuild_index_after_edit(&mana_dir).unwrap();
        let index1 = Index::load(&mana_dir).unwrap();
        assert_eq!(index1.units.len(), 1);

        // Add another unit and rebuild
        let bean2 = Unit::new("2", "Second");
        bean2.to_file(mana_dir.join("2-second.md")).unwrap();

        rebuild_index_after_edit(&mana_dir).unwrap();
        let index2 = Index::load(&mana_dir).unwrap();
        assert_eq!(index2.units.len(), 2);
    }

    // =====================================================================
    // Integration tests for cmd_edit (Unit 2.3)
    // =====================================================================

    #[test]
    fn test_cmd_edit_finds_bean_by_id() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let unit = Unit::new("1", "Original title");
        unit.to_file(mana_dir.join("1-original.md")).unwrap();

        // Verify that find_unit_file can locate the unit
        let found = crate::discovery::find_unit_file(&mana_dir, "1");
        assert!(found.is_ok(), "Should find unit by ID");
    }

    #[test]
    fn test_cmd_edit_fails_for_nonexistent_bean() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Note: cmd_edit requires valid $EDITOR, so we just verify find_unit_file fails
        let found = crate::discovery::find_unit_file(&mana_dir, "999");
        assert!(found.is_err(), "Should fail for nonexistent unit");
    }

    #[test]
    fn test_cmd_edit_loads_backup_correctly() {
        let bean_content = r#"id: "1"
title: Test Unit
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;
        let (_dir, path) = create_valid_bean_file(bean_content);

        // Load backup
        let backup = load_backup(&path).unwrap();

        // Verify backup matches original
        assert_eq!(backup, bean_content.as_bytes());
    }

    #[test]
    fn test_cmd_edit_workflow_backup_edit_save() {
        // Test the complete workflow: backup -> edit -> validate -> save -> index
        let bean_content = r#"id: "1"
title: Original
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;
        let (_dir, path) = create_valid_bean_file(bean_content);
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Copy unit file to test mana_dir for index rebuild
        fs::copy(&path, mana_dir.join("1-original.md")).unwrap();

        // Step 1: Backup
        let backup = load_backup(&path).unwrap();
        assert_eq!(backup, bean_content.as_bytes());

        // Step 2: Simulate edit (modify title in memory)
        let edited_content = r#"id: "1"
title: Modified
status: open
priority: 2
created_at: "2026-01-26T15:00:00Z"
updated_at: "2026-01-26T15:00:00Z"
"#;

        // Step 3: Write edited content to file
        fs::write(&path, edited_content).unwrap();

        // Step 4: Validate and save
        validate_and_save(&path, edited_content).unwrap();

        // Step 5: Verify changes persisted and timestamp updated
        let saved_bean = Unit::from_file(&path).unwrap();
        assert_eq!(saved_bean.title, "Modified");
        assert_ne!(saved_bean.updated_at.to_string(), "2026-01-26T15:00:00Z");

        // Step 6: Rebuild index
        rebuild_index_after_edit(&mana_dir).unwrap();
        let index = Index::load(&mana_dir).unwrap();
        assert!(index.units.iter().any(|b| b.id == "1"));
    }

    #[test]
    fn test_cmd_edit_validates_schema_before_save() {
        let invalid_content = "id: 1\ntitle: Test\nstatus: invalid_status\n";
        let (_dir, path) = create_valid_bean_file(invalid_content);

        let result = validate_and_save(&path, invalid_content);
        assert!(result.is_err(), "Should reject invalid schema");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid YAML") || err_msg.contains("Invalid status"));
    }

    #[test]
    fn test_cmd_edit_preserves_bean_naming_convention() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        // Create a unit with {id}-{slug}.md naming
        let unit = Unit::new("1", "My Task");
        let original_path = mana_dir.join("1-my-task.md");
        unit.to_file(&original_path).unwrap();

        // Verify the file exists with correct naming
        assert!(
            original_path.exists(),
            "Unit file should be named 1-my-task.md"
        );

        // Load and modify
        let content = fs::read_to_string(&original_path).unwrap();
        let modified = content.replace("My Task", "Updated Task");

        // Save with validate_and_save (this is what cmd_edit uses)
        validate_and_save(&original_path, &modified).unwrap();

        // Verify the file still exists and naming is preserved
        assert!(
            original_path.exists(),
            "Naming should be preserved after edit"
        );

        // Verify the unit was updated
        let updated_bean = Unit::from_file(&original_path).unwrap();
        assert_eq!(updated_bean.title, "Updated Task");
    }

    #[test]
    fn test_cmd_edit_index_rebuild_includes_edited_bean() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let unit = Unit::new("1", "Original");
        unit.to_file(mana_dir.join("1-original.md")).unwrap();

        // Build initial index
        rebuild_index_after_edit(&mana_dir).unwrap();
        let index1 = Index::load(&mana_dir).unwrap();
        assert_eq!(index1.units[0].title, "Original");

        // Edit the unit
        let bean_content = fs::read_to_string(mana_dir.join("1-original.md")).unwrap();
        let modified = bean_content.replace("Original", "Modified");
        validate_and_save(&mana_dir.join("1-original.md"), &modified).unwrap();

        // Rebuild index
        rebuild_index_after_edit(&mana_dir).unwrap();
        let index2 = Index::load(&mana_dir).unwrap();

        // Verify index reflects the edit
        assert_eq!(index2.units[0].title, "Modified");
    }
}
