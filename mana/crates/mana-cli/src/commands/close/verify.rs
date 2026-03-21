use std::io::Read;
use std::path::Path;
use std::process::{Command as ShellCommand, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;

/// Result of running a verify command
pub(super) struct VerifyResult {
    pub(super) success: bool,
    pub(super) exit_code: Option<i32>,
    pub(super) stdout: String,
    #[allow(dead_code)]
    pub(super) stderr: String,
    pub(super) output: String, // combined stdout+stderr, for backward compat
    /// True when the process was killed due to verify_timeout being exceeded.
    pub(super) timed_out: bool,
}

/// Run a verify command for a unit.
///
/// Returns VerifyResult with success status, exit code, and combined stdout/stderr.
/// If `timeout_secs` is Some(n), the process is killed after n seconds and
/// the result has `timed_out = true`.
pub(super) fn run_verify(
    mana_dir: &Path,
    verify_cmd: &str,
    timeout_secs: Option<u64>,
) -> Result<VerifyResult> {
    // Run in the project root (parent of .mana/)
    let project_root = mana_dir
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine project root from units dir"))?;

    println!("Running verify: {}", verify_cmd);

    let mut child = ShellCommand::new("sh")
        .args(["-c", verify_cmd])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn verify command: {}", verify_cmd))?;

    // Drain stdout/stderr in background threads to prevent pipe buffer deadlock.
    let stdout_thread = {
        let stdout = child.stdout.take().expect("stdout is piped");
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut reader = std::io::BufReader::new(stdout);
            let _ = reader.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).trim().to_string()
        })
    };
    let stderr_thread = {
        let stderr = child.stderr.take().expect("stderr is piped");
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut reader = std::io::BufReader::new(stderr);
            let _ = reader.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).trim().to_string()
        })
    };

    // Poll for process exit, enforcing the timeout if set.
    let timeout = timeout_secs.map(Duration::from_secs);
    let start = Instant::now();

    let (timed_out, exit_status) = loop {
        match child
            .try_wait()
            .with_context(|| "Failed to poll verify process")?
        {
            Some(status) => break (false, Some(status)),
            None => {
                if let Some(limit) = timeout {
                    if start.elapsed() >= limit {
                        let _ = child.kill();
                        let _ = child.wait();
                        break (true, None);
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let stdout_str = stdout_thread.join().unwrap_or_default();
    let stderr_str = stderr_thread.join().unwrap_or_default();

    if timed_out {
        let secs = timeout_secs.unwrap_or(0);
        return Ok(VerifyResult {
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            output: format!("Verify timed out after {}s", secs),
            timed_out: true,
        });
    }

    let status = exit_status.expect("exit_status is Some when not timed_out");
    let combined_output = {
        let mut combined = stdout_str.clone();
        if !stderr_str.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&stderr_str);
        }
        combined
    };

    Ok(VerifyResult {
        success: status.success(),
        exit_code: status.code(),
        stdout: stdout_str,
        stderr: stderr_str,
        output: combined_output,
        timed_out: false,
    })
}

/// Truncate output to first N + last N lines.
/// If output has fewer than 2*N lines, return it unchanged.
pub(super) fn truncate_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();

    if lines.len() <= max_lines * 2 {
        return output.to_string();
    }

    let first = &lines[..max_lines];
    let last = &lines[lines.len() - max_lines..];

    format!(
        "{}\n\n... ({} lines omitted) ...\n\n{}",
        first.join("\n"),
        lines.len() - max_lines * 2,
        last.join("\n")
    )
}

/// Format a verify failure as a Markdown block to append to notes.
pub(super) fn format_failure_note(attempt: u32, exit_code: Option<i32>, output: &str) -> String {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let truncated = truncate_output(output, 50);
    let exit_str = exit_code
        .map(|c| format!("Exit code: {}\n", c))
        .unwrap_or_default();

    format!(
        "\n## Attempt {} — {}\n{}\n```\n{}\n```\n",
        attempt, timestamp, exit_str, truncated
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // Truncation Helper Tests
    // =====================================================================

    #[test]
    fn test_truncate_output_short() {
        let output = "line1\nline2\nline3";
        let result = truncate_output(output, 50);
        assert_eq!(result, output); // No truncation needed
    }

    #[test]
    fn test_truncate_output_exact_boundary() {
        // Exactly 100 lines (50*2), should not truncate
        let lines: Vec<String> = (1..=100).map(|i| format!("line{}", i)).collect();
        let output = lines.join("\n");
        let result = truncate_output(&output, 50);
        assert_eq!(result, output);
    }

    #[test]
    fn test_truncate_output_long() {
        // 150 lines, should truncate to first 50 + last 50
        let lines: Vec<String> = (1..=150).map(|i| format!("line{}", i)).collect();
        let output = lines.join("\n");
        let result = truncate_output(&output, 50);

        assert!(result.contains("line1"));
        assert!(result.contains("line50"));
        assert!(!result.contains("line51"));
        assert!(!result.contains("line100"));
        assert!(result.contains("line101"));
        assert!(result.contains("line150"));
        assert!(result.contains("(50 lines omitted)"));
    }

    #[test]
    fn test_format_failure_note() {
        let note = format_failure_note(1, Some(1), "error message");

        assert!(note.contains("## Attempt 1"));
        assert!(note.contains("Exit code: 1"));
        assert!(note.contains("error message"));
        assert!(note.contains("```")); // Fenced code block
    }
}
