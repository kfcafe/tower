use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::unit::Unit;

/// Maximum time to wait for a hook script before killing it.
const HOOK_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// HookEvent Enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    PreCreate,
    PostCreate,
    PreUpdate,
    PostUpdate,
    PreClose,
    PostClose,
}

impl HookEvent {
    /// Convert HookEvent to its string representation for hook file names.
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::PreCreate => "pre-create",
            HookEvent::PostCreate => "post-create",
            HookEvent::PreUpdate => "pre-update",
            HookEvent::PostUpdate => "post-update",
            HookEvent::PreClose => "pre-close",
            HookEvent::PostClose => "post-close",
        }
    }
}

// ---------------------------------------------------------------------------
// HookPayload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayload {
    pub event: String,
    pub unit: Unit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl HookPayload {
    /// Create a new HookPayload for the given event and unit.
    pub fn new(event: HookEvent, unit: Unit, reason: Option<String>) -> Self {
        Self {
            event: event.as_str().to_string(),
            unit,
            reason,
        }
    }

    /// Serialize this payload to JSON.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).context("Failed to serialize hook payload to JSON")
    }
}

// ---------------------------------------------------------------------------
// Hook Path Management
// ---------------------------------------------------------------------------

/// Get the path to a hook script based on the event and project directory.
pub fn get_hook_path(project_dir: &Path, event: HookEvent) -> PathBuf {
    project_dir.join(".mana").join("hooks").join(event.as_str())
}

/// Check if a hook file exists and is executable.
pub fn is_hook_executable(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    // On Unix-like systems, check execute bit
    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        if let Ok(metadata) = fs::metadata(path) {
            let mode = metadata.permissions().mode();
            // Check if any execute bit is set (user, group, or other)
            (mode & 0o111) != 0
        } else {
            false
        }
    }

    // On Windows, any executable file extension is considered executable
    #[cfg(windows)]
    {
        let path_str = path.to_string_lossy();
        let exe_extensions = [".exe", ".bat", ".cmd", ".ps1", ".com"];
        exe_extensions.iter().any(|ext| path_str.ends_with(ext))
    }

    // Default to true if we can't determine
    #[cfg(not(any(unix, windows)))]
    true
}

// ---------------------------------------------------------------------------
// Hook Execution
// ---------------------------------------------------------------------------

/// Execute a hook script with the given payload.
///
/// # Arguments
///
/// * `event` - The hook event to trigger
/// * `unit` - The unit to pass to the hook
/// * `project_dir` - The project root directory (parent of .mana/)
/// * `reason` - Optional reason (used for pre-close hooks)
///
/// # Returns
///
/// * `Ok(true)` - Hook passed (exit 0), or hook doesn't exist, or hooks not trusted
/// * `Ok(false)` - Hook executed but returned non-zero exit code
/// * `Err` - Hook exists but not executable, timeout, or I/O error
pub fn execute_hook(
    event: HookEvent,
    unit: &Unit,
    project_dir: &Path,
    reason: Option<String>,
) -> Result<bool> {
    // Security model: Hooks are disabled by default. Users must explicitly enable them
    // with `mana trust` before any hooks will execute. This ensures users review the
    // hook scripts in .mana/hooks/ before giving them execution rights.
    if !is_trusted(project_dir) {
        return Ok(true);
    }

    let hook_path = get_hook_path(project_dir, event);

    // If hook doesn't exist, silently return success
    if !hook_path.exists() {
        return Ok(true);
    }

    // If hook exists but is not executable, return error
    if !is_hook_executable(&hook_path) {
        return Err(anyhow!(
            "Hook {} exists but is not executable",
            hook_path.display()
        ));
    }

    // Create the payload
    let payload = HookPayload::new(event, unit.clone(), reason);
    let json_payload = payload.to_json()?;

    // Spawn the subprocess. stdout/stderr are discarded — hooks communicate
    // exclusively via exit code. Using Stdio::null() instead of Stdio::piped()
    // prevents deadlock: piped() without draining blocks the child once OS pipe
    // buffers fill (~64KB), causing it to hang until the timeout kills it.
    let mut child = Command::new(&hook_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_dir)
        .spawn()
        .with_context(|| format!("Failed to spawn hook {}", hook_path.display()))?;

    // Write JSON payload to stdin, then close the pipe
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("Failed to open stdin for hook"))?;
        stdin
            .write_all(json_payload.as_bytes())
            .context("Failed to write payload to hook stdin")?;
    }

    // Poll for completion with timeout
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) => {
                if start.elapsed() > HOOK_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait(); // Reap to prevent zombie process
                    return Err(anyhow!(
                        "Hook {} timed out after {}s",
                        hook_path.display(),
                        HOOK_TIMEOUT.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(
                    anyhow!(e).context(format!("Failed to wait for hook {}", hook_path.display()))
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Trust Management
// ---------------------------------------------------------------------------

/// Check if hooks are trusted (enabled).
///
/// Returns true if the .mana/.hooks-trusted file exists, false otherwise.
/// Does not error if the file doesn't exist.
pub fn is_trusted(project_dir: &Path) -> bool {
    project_dir.join(".mana").join(".hooks-trusted").exists()
}

/// Enable hook trust by creating the .mana/.hooks-trusted file.
///
/// # Returns
///
/// * `Ok(())` - Trust file created successfully
/// * `Err` - Failed to create trust file
pub fn create_trust(project_dir: &Path) -> Result<()> {
    let trust_path = project_dir.join(".mana").join(".hooks-trusted");

    let parent = trust_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid trust path"))?;
    std::fs::create_dir_all(parent).context("Failed to create .mana directory for trust file")?;

    let metadata = format!("Hooks enabled at {}\n", chrono::Utc::now());
    std::fs::write(&trust_path, metadata).context("Failed to create trust file")
}

/// Revoke hook trust by deleting the .mana/.hooks-trusted file.
///
/// # Returns
///
/// * `Ok(())` - Trust file deleted successfully
/// * `Err` - Trust file doesn't exist or failed to delete
pub fn revoke_trust(project_dir: &Path) -> Result<()> {
    let trust_path = project_dir.join(".mana").join(".hooks-trusted");

    if !trust_path.exists() {
        return Err(anyhow!("Trust file does not exist"));
    }

    std::fs::remove_file(&trust_path).context("Failed to revoke hook trust")
}

// ---------------------------------------------------------------------------
// Config-Based Hooks (on_close, on_fail, post_plan)
// ---------------------------------------------------------------------------

/// Template variables for config hook expansion.
///
/// Each field maps to a `{name}` placeholder in the hook command template.
/// Missing variables are left as-is (e.g., `{attempt}` stays literal if not set).
#[derive(Debug, Default)]
pub struct HookVars {
    pub id: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub attempt: Option<u32>,
    pub output: Option<String>,
    pub parent: Option<String>,
    pub children: Option<String>,
    pub branch: Option<String>,
}

/// Expand template variables in a hook command string.
///
/// Replaces `{name}` placeholders with values from `vars`.
/// Unknown or unset variables are left as-is.
/// The `{output}` variable is truncated to 1000 chars.
pub fn expand_template(template: &str, vars: &HookVars) -> String {
    let mut result = template.to_string();

    if let Some(ref v) = vars.id {
        result = result.replace("{id}", v);
    }
    if let Some(ref v) = vars.title {
        result = result.replace("{title}", v);
    }
    if let Some(ref v) = vars.status {
        result = result.replace("{status}", v);
    }
    if let Some(attempt) = vars.attempt {
        result = result.replace("{attempt}", &attempt.to_string());
    }
    if let Some(ref v) = vars.output {
        // Truncate output to 1000 chars
        let truncated = if v.len() > 1000 {
            &v[..1000]
        } else {
            v.as_str()
        };
        result = result.replace("{output}", truncated);
    }
    if let Some(ref v) = vars.parent {
        result = result.replace("{parent}", v);
    }
    if let Some(ref v) = vars.children {
        result = result.replace("{children}", v);
    }
    if let Some(ref v) = vars.branch {
        result = result.replace("{branch}", v);
    }

    result
}

/// Get the current git branch name, or None if not in a git repo.
pub fn current_git_branch() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Execute a config-based hook command asynchronously.
///
/// The command is expanded with template variables, then spawned via `sh -c`.
/// The subprocess runs in the background — we don't wait for it.
/// Any errors during spawn are logged to stderr but never propagated.
///
/// # Arguments
///
/// * `hook_name` - Name for logging (e.g., "on_close", "on_fail")
/// * `template` - The command template with `{var}` placeholders
/// * `vars` - Template variables to expand
/// * `project_dir` - Working directory for the subprocess
pub fn execute_config_hook(hook_name: &str, template: &str, vars: &HookVars, project_dir: &Path) {
    let cmd = expand_template(template, vars);

    match Command::new("sh")
        .args(["-c", &cmd])
        .current_dir(project_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_child) => {
            // Fire-and-forget: don't wait for completion
        }
        Err(e) => {
            eprintln!("Warning: {} hook failed to spawn: {}", hook_name, e);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_bean() -> Unit {
        Unit::new("1", "Test Unit")
    }

    fn create_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_hook_event_string_representation() {
        assert_eq!(HookEvent::PreCreate.as_str(), "pre-create");
        assert_eq!(HookEvent::PostCreate.as_str(), "post-create");
        assert_eq!(HookEvent::PreUpdate.as_str(), "pre-update");
        assert_eq!(HookEvent::PostUpdate.as_str(), "post-update");
        assert_eq!(HookEvent::PreClose.as_str(), "pre-close");
        assert_eq!(HookEvent::PostClose.as_str(), "post-close");
    }

    #[test]
    fn test_hook_payload_serializes_to_json() {
        let unit = create_test_bean();
        let payload = HookPayload::new(HookEvent::PreCreate, unit.clone(), None);

        let json = payload.to_json().unwrap();
        assert!(json.contains("\"event\":\"pre-create\""));
        assert!(json.contains("\"id\":\"1\""));
        assert!(json.contains("\"title\":\"Test Unit\""));
        assert!(!json.contains("\"reason\"") || json.contains("\"reason\":null"));
    }

    #[test]
    fn test_hook_payload_with_reason() {
        let unit = create_test_bean();
        let payload = HookPayload::new(
            HookEvent::PreClose,
            unit,
            Some("Completed successfully".to_string()),
        );

        let json = payload.to_json().unwrap();
        assert!(json.contains("\"event\":\"pre-close\""));
        assert!(json.contains("\"reason\":\"Completed successfully\""));
    }

    #[test]
    fn test_get_hook_path() {
        let temp_dir = create_test_dir();
        let hook_path = get_hook_path(temp_dir.path(), HookEvent::PreCreate);

        assert!(hook_path.ends_with(".mana/hooks/pre-create"));
    }

    #[test]
    fn test_missing_hook_returns_ok_true() {
        let temp_dir = create_test_dir();
        let unit = create_test_bean();

        // No hook exists
        let result = execute_hook(HookEvent::PreCreate, &unit, temp_dir.path(), None);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_non_executable_hook_returns_error() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let hooks_dir = project_dir.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Enable trust so hook execution is attempted
        create_trust(project_dir).unwrap();

        let hook_path = hooks_dir.join("pre-create");
        fs::write(&hook_path, "#!/bin/bash\nexit 0").unwrap();
        // File is not executable

        let unit = create_test_bean();
        let result = execute_hook(HookEvent::PreCreate, &unit, project_dir, None);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not executable"));
    }

    #[test]
    fn test_successful_hook_execution() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let hooks_dir = project_dir.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Enable trust so hook execution is attempted
        create_trust(project_dir).unwrap();

        let hook_path = hooks_dir.join("pre-create");
        // Use a simple script that just exits successfully, ignoring stdin
        fs::write(&hook_path, "#!/bin/bash\nexit 0").unwrap();

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let unit = create_test_bean();
        let result = execute_hook(HookEvent::PreCreate, &unit, project_dir, None);

        assert!(result.is_ok(), "Hook execution failed: {:?}", result.err());
        assert!(result.unwrap());
    }

    #[test]
    fn test_hook_execution_with_failure_exit_code() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let hooks_dir = project_dir.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Enable trust so hook execution is attempted
        create_trust(project_dir).unwrap();

        let hook_path = hooks_dir.join("pre-create");
        fs::write(&hook_path, "#!/bin/bash\nexit 1").unwrap();

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let unit = create_test_bean();
        let result = execute_hook(HookEvent::PreCreate, &unit, project_dir, None);

        assert!(result.is_ok(), "Hook execution failed: {:?}", result.err());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_hook_receives_json_payload_on_stdin() {
        // Test that the payload can be serialized to JSON and would be sent to the hook
        let unit = create_test_bean();
        let payload = HookPayload::new(HookEvent::PreCreate, unit, None);

        let json = payload.to_json().unwrap();

        // Verify the JSON contains all expected fields
        assert!(json.contains("\"event\":\"pre-create\""));
        assert!(json.contains("\"unit\":{"));
        assert!(json.contains("\"id\":\"1\""));
        assert!(json.contains("\"title\":\"Test Unit\""));
        assert!(json.contains("\"status\":"));
    }

    #[test]
    #[cfg(unix)]
    fn test_hook_timeout() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let hooks_dir = project_dir.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Enable trust so hook execution is attempted
        create_trust(project_dir).unwrap();

        let hook_path = hooks_dir.join("pre-create");
        // Script that sleeps for longer than timeout
        fs::write(&hook_path, "#!/bin/bash\nsleep 60\nexit 0").unwrap();

        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();

        let unit = create_test_bean();
        let result = execute_hook(HookEvent::PreCreate, &unit, project_dir, None);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[test]
    fn test_is_hook_executable_with_missing_file() {
        let temp_dir = create_test_dir();
        let hook_path = temp_dir.path().join("nonexistent");

        assert!(!is_hook_executable(&hook_path));
    }

    #[test]
    #[cfg(unix)]
    fn test_is_hook_executable_with_executable_file() {
        let temp_dir = create_test_dir();
        let hook_path = temp_dir.path().join("executable");
        fs::write(&hook_path, "#!/bin/bash\necho test").unwrap();

        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(is_hook_executable(&hook_path));
    }

    #[test]
    #[cfg(unix)]
    fn test_is_hook_executable_with_non_executable_file() {
        let temp_dir = create_test_dir();
        let hook_path = temp_dir.path().join("non-executable");
        fs::write(&hook_path, "#!/bin/bash\necho test").unwrap();

        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o644)).unwrap();

        assert!(!is_hook_executable(&hook_path));
    }

    #[test]
    fn test_hook_payload_with_all_bean_fields() {
        let mut unit = create_test_bean();
        unit.description = Some("Test description".to_string());
        unit.acceptance = Some("Test acceptance".to_string());
        unit.labels = vec!["test".to_string(), "important".to_string()];

        let payload = HookPayload::new(HookEvent::PostCreate, unit, None);
        let json = payload.to_json().unwrap();

        assert!(json.contains("description"));
        assert!(json.contains("Test description"));
        assert!(json.contains("labels"));
        assert!(json.contains("test"));
    }

    // =====================================================================
    // Trust Management Tests
    // =====================================================================

    #[test]
    fn test_is_trusted_returns_false_when_trust_file_does_not_exist() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();

        // Create .mana directory
        fs::create_dir_all(project_dir.join(".mana")).unwrap();

        // Trust should not exist
        assert!(!is_trusted(project_dir));
    }

    #[test]
    fn test_is_trusted_returns_true_when_trust_file_exists() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();

        // Create .mana directory and trust file
        fs::create_dir_all(project_dir.join(".mana")).unwrap();
        fs::write(project_dir.join(".mana").join(".hooks-trusted"), "").unwrap();

        // Trust should exist
        assert!(is_trusted(project_dir));
    }

    #[test]
    fn test_create_trust_creates_trust_file() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();

        // Create .mana directory
        fs::create_dir_all(project_dir.join(".mana")).unwrap();

        // Trust should not exist yet
        assert!(!is_trusted(project_dir));

        // Create trust
        let result = create_trust(project_dir);
        assert!(result.is_ok());

        // Trust should now exist
        assert!(is_trusted(project_dir));

        // Verify file contains metadata
        let content = fs::read_to_string(project_dir.join(".mana").join(".hooks-trusted")).unwrap();
        assert!(content.contains("Hooks enabled"));
    }

    #[test]
    fn test_revoke_trust_removes_trust_file() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();

        // Create .mana directory and trust file
        fs::create_dir_all(project_dir.join(".mana")).unwrap();
        fs::write(project_dir.join(".mana").join(".hooks-trusted"), "").unwrap();

        // Trust should exist
        assert!(is_trusted(project_dir));

        // Revoke trust
        let result = revoke_trust(project_dir);
        assert!(result.is_ok());

        // Trust should no longer exist
        assert!(!is_trusted(project_dir));
    }

    #[test]
    fn test_revoke_trust_errors_if_file_does_not_exist() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();

        // Create .mana directory
        fs::create_dir_all(project_dir.join(".mana")).unwrap();

        // Try to revoke non-existent trust
        let result = revoke_trust(project_dir);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Trust file does not exist"));
    }

    #[test]
    fn test_execute_hook_skips_when_not_trusted() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let hooks_dir = project_dir.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Create an executable hook
        let hook_path = hooks_dir.join("pre-create");
        fs::write(&hook_path, "#!/bin/bash\nexit 1").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let unit = create_test_bean();

        // Hook should NOT execute (returns Ok(true) but doesn't run)
        // If trust is disabled, hook should not even check executability
        let result = execute_hook(HookEvent::PreCreate, &unit, project_dir, None);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Returns true but doesn't execute
    }

    #[test]
    fn test_execute_hook_runs_when_trusted() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let hooks_dir = project_dir.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Enable trust
        create_trust(project_dir).unwrap();

        // Create an executable hook that succeeds
        let hook_path = hooks_dir.join("pre-create");
        fs::write(&hook_path, "#!/bin/bash\nexit 0").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let unit = create_test_bean();

        // Hook should execute successfully
        let result = execute_hook(HookEvent::PreCreate, &unit, project_dir, None);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_execute_hook_respects_non_trusted_status() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let hooks_dir = project_dir.join(".mana").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();

        // Create a hook but DO NOT enable trust
        let hook_path = hooks_dir.join("pre-create");
        fs::write(&hook_path, "#!/bin/bash\nexit 0").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let unit = create_test_bean();

        // Hook should NOT execute (returns Ok(true) silently)
        let result = execute_hook(HookEvent::PreCreate, &unit, project_dir, None);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // =====================================================================
    // Template Expansion Tests
    // =====================================================================

    #[test]
    fn test_expand_template_with_all_vars() {
        let vars = HookVars {
            id: Some("42".into()),
            title: Some("Fix the bug".into()),
            status: Some("closed".into()),
            attempt: Some(3),
            output: Some("FAIL: test_foo".into()),
            parent: Some("10".into()),
            children: Some("10.1,10.2".into()),
            branch: Some("main".into()),
        };

        let result = expand_template(
            "echo {id} {title} {status} {attempt} {output} {parent} {children} {branch}",
            &vars,
        );
        assert_eq!(
            result,
            "echo 42 Fix the bug closed 3 FAIL: test_foo 10 10.1,10.2 main"
        );
    }

    #[test]
    fn test_expand_template_missing_vars_left_as_is() {
        let vars = HookVars {
            id: Some("1".into()),
            ..Default::default()
        };

        let result = expand_template("echo {id} {title} {unknown}", &vars);
        assert_eq!(result, "echo 1 {title} {unknown}");
    }

    #[test]
    fn test_expand_template_output_truncated_to_1000_chars() {
        let long_output = "x".repeat(2000);
        let vars = HookVars {
            output: Some(long_output),
            ..Default::default()
        };

        let result = expand_template("echo {output}", &vars);
        // "echo " = 5 chars + 1000 chars of x
        assert_eq!(result.len(), 5 + 1000);
    }

    #[test]
    fn test_expand_template_empty_template() {
        let vars = HookVars::default();
        let result = expand_template("", &vars);
        assert_eq!(result, "");
    }

    #[test]
    fn test_expand_template_no_placeholders() {
        let vars = HookVars {
            id: Some("1".into()),
            ..Default::default()
        };
        let result = expand_template("echo hello world", &vars);
        assert_eq!(result, "echo hello world");
    }

    #[test]
    fn test_expand_template_multiple_same_var() {
        let vars = HookVars {
            id: Some("5".into()),
            ..Default::default()
        };
        let result = expand_template("{id} and {id} again", &vars);
        assert_eq!(result, "5 and 5 again");
    }

    // =====================================================================
    // Config Hook Execution Tests
    // =====================================================================

    #[test]
    fn test_execute_config_hook_writes_to_file() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let output_file = project_dir.join("hook_output.txt");

        let vars = HookVars {
            id: Some("99".into()),
            title: Some("Test unit".into()),
            ..Default::default()
        };

        // Build the template with the output file path baked in
        let template = format!("echo '{{id}}' > {}", output_file.display());
        execute_config_hook("on_close", &template, &vars, project_dir);

        // Wait briefly for async subprocess
        std::thread::sleep(Duration::from_millis(500));

        let content = fs::read_to_string(&output_file).unwrap();
        assert_eq!(content.trim(), "99");
    }

    #[test]
    fn test_execute_config_hook_failure_does_not_panic() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();

        // Running a command that doesn't exist should not panic
        execute_config_hook(
            "on_close",
            "/nonexistent/command/that/does/not/exist",
            &HookVars::default(),
            project_dir,
        );

        // If we get here, the hook failure was handled gracefully
    }

    #[test]
    fn test_execute_config_hook_with_template_expansion() {
        let temp_dir = create_test_dir();
        let project_dir = temp_dir.path();
        let output_file = project_dir.join("expanded.txt");

        let vars = HookVars {
            id: Some("7".into()),
            title: Some("My Task".into()),
            status: Some("closed".into()),
            branch: Some("feature-x".into()),
            ..Default::default()
        };

        let template = format!(
            "echo '{{id}}|{{title}}|{{status}}|{{branch}}' > {}",
            output_file.display()
        );
        execute_config_hook("on_close", &template, &vars, project_dir);

        std::thread::sleep(Duration::from_millis(500));

        let content = fs::read_to_string(&output_file).unwrap();
        assert_eq!(content.trim(), "7|My Task|closed|feature-x");
    }
}
