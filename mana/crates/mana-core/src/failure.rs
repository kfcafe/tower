/// Structured failure summaries for failed agent runs.
///
/// When an agent fails, this module generates a markdown summary capturing
/// what was tried, why it failed, which files were touched, and suggestions
/// for the next attempt. Designed to be appended as a unit note so context
/// survives across retries.
use std::collections::BTreeSet;
use std::fmt::Write;

/// Everything needed to produce a failure summary.
#[derive(Debug)]
pub struct FailureContext {
    pub unit_id: String,
    pub unit_title: String,
    pub attempt: u32,
    pub duration_secs: u64,
    pub tool_count: usize,
    pub turns: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
    pub error: Option<String>,
    /// Log lines in `[tool] ToolName path/or/args` format.
    pub tool_log: Vec<String>,
    pub verify_command: Option<String>,
}

/// Build a structured markdown summary of a failed agent run.
#[must_use]
pub fn build_failure_summary(ctx: &FailureContext) -> String {
    let mut sections: Vec<String> = Vec::new();

    // Header
    let duration = format_duration(ctx.duration_secs);
    let total_tokens = ctx.input_tokens + ctx.output_tokens;
    let tokens = format_tokens(total_tokens);
    sections.push(format!(
        "## Attempt {} Failed ({}, {} tokens, ${:.3})",
        ctx.attempt, duration, tokens, ctx.cost
    ));

    // What was tried
    let tried = build_tried_section(ctx);
    if !tried.is_empty() {
        sections.push("### What was tried".to_string());
        sections.push(tried.join("\n"));
    }

    // Why it failed
    sections.push("### Why it failed".to_string());
    sections.push(build_failure_reason(ctx));

    // Files touched
    let files = extract_files_from_logs(&ctx.tool_log);
    if !files.is_empty() {
        sections.push("### Files touched".to_string());
        let list = files.iter().map(|f| format!("- {f}")).collect::<Vec<_>>();
        sections.push(list.join("\n"));
    }

    // Verify command
    if let Some(ref verify) = ctx.verify_command {
        sections.push("### Verify command".to_string());
        sections.push(format!("`{verify}`"));
    }

    // Suggestion
    if let Some(suggestion) = build_suggestion(ctx.error.as_deref()) {
        sections.push("### Suggestion for next attempt".to_string());
        sections.push(suggestion.to_string());
    }

    sections.join("\n\n")
}

/// Extract unique file paths associated with a specific tool from log lines.
///
/// Looks for lines matching `[tool] <tool_name> <path>` and returns
/// deduplicated paths in the order first seen.
#[must_use]
pub fn extract_tool_paths(logs: &[String], tool_name: &str) -> Vec<String> {
    let prefix = format!("[tool] {tool_name} ");
    let mut seen = BTreeSet::new();
    let mut paths = Vec::new();
    for line in logs {
        if let Some(rest) = line.strip_prefix(&prefix) {
            let path = rest.trim().to_string();
            if seen.insert(path.clone()) {
                paths.push(path);
            }
        }
        // Also handle lines where [tool] appears after a timestamp/prefix
        if let Some(idx) = line.find(&prefix) {
            let rest = &line[idx + prefix.len()..];
            let path = rest.trim().to_string();
            if seen.insert(path.clone()) {
                paths.push(path);
            }
        }
    }
    paths
}

/// Count occurrences of a tool in log lines.
#[must_use]
pub fn count_tool(logs: &[String], tool_name: &str) -> usize {
    let marker = format!("[tool] {tool_name}");
    logs.iter().filter(|line| line.contains(&marker)).count()
}

/// Extract all unique file paths from log lines regardless of tool.
///
/// Matches `[tool] <name> <path>` where path contains no spaces (to
/// distinguish file paths from multi-word arguments).
#[must_use]
pub fn extract_files_from_logs(logs: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    for line in logs {
        if let Some(path) = parse_tool_path(line) {
            if !path.contains(' ') && seen.insert(path.clone()) {
                files.push(path);
            }
        }
    }
    files
}

/// Return the last `n` tool names from log lines.
#[must_use]
pub fn extract_last_tools(logs: &[String], n: usize) -> Vec<String> {
    let mut tools = Vec::new();
    for line in logs {
        if let Some(name) = parse_tool_name(line) {
            tools.push(name);
        }
    }
    let start = tools.len().saturating_sub(n);
    tools[start..].to_vec()
}

/// Show up to 3 paths, then "+N more".
#[must_use]
pub fn summarize_paths(paths: &[String]) -> String {
    if paths.len() <= 3 {
        return paths.join(", ");
    }
    let first_three = paths[..3].join(", ");
    let remaining = paths.len() - 3;
    format!("{first_three} +{remaining} more")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn build_tried_section(ctx: &FailureContext) -> Vec<String> {
    let mut lines = Vec::new();

    let reads = extract_tool_paths(&ctx.tool_log, "Read");
    let edits = extract_tool_paths(&ctx.tool_log, "Edit");
    let writes = extract_tool_paths(&ctx.tool_log, "Write");
    let bash_count = count_tool(&ctx.tool_log, "Bash");

    if !reads.is_empty() {
        lines.push(format!("- Read {}", summarize_paths(&reads)));
    }
    if !edits.is_empty() {
        lines.push(format!("- Edited {}", summarize_paths(&edits)));
    }
    if !writes.is_empty() {
        lines.push(format!("- Wrote {}", summarize_paths(&writes)));
    }
    if bash_count > 0 {
        let plural = if bash_count > 1 { "s" } else { "" };
        lines.push(format!("- Ran {bash_count} bash command{plural}"));
    }

    let duration = format_duration(ctx.duration_secs);
    lines.push(format!(
        "- {} tool calls over {} turns in {}",
        ctx.tool_count, ctx.turns, duration
    ));

    lines
}

fn build_failure_reason(ctx: &FailureContext) -> String {
    let mut lines = Vec::new();

    if let Some(ref error) = ctx.error {
        lines.push(format!("- {error}"));
    }

    let last_tools = extract_last_tools(&ctx.tool_log, 3);
    if !last_tools.is_empty() {
        lines.push(format!(
            "- Last tools before failure: {}",
            last_tools.join(", ")
        ));
    }

    if lines.is_empty() {
        lines.push("- Unknown failure (no error captured)".to_string());
    }

    lines.join("\n")
}

fn build_suggestion(error: Option<&str>) -> Option<&'static str> {
    let err = error?.to_lowercase();

    if err.contains("idle timeout") {
        return Some("- Agent went idle — it may be stuck in a loop or waiting for input. Try a more focused prompt or break the task into smaller steps.");
    }
    if err.contains("timeout") {
        return Some("- Agent ran out of time. Consider increasing the timeout or simplifying the task scope.");
    }
    if err.contains("aborted") {
        return Some("- Agent was manually aborted. Review progress so far before retrying.");
    }
    if err.contains("claim") {
        return Some("- Could not claim the unit. Check if another agent is working on it or if it's already closed.");
    }
    if err.contains("exit code") {
        return Some("- Agent exited with an error. Check the verify command output and ensure the approach is correct before retrying.");
    }

    None
}

/// Parse the tool name from a `[tool] ToolName ...` log line.
fn parse_tool_name(line: &str) -> Option<String> {
    let tag = "[tool] ";
    let idx = line.find(tag)?;
    let rest = &line[idx + tag.len()..];
    let name = rest.split_whitespace().next()?;
    Some(name.to_string())
}

/// Parse the path argument from a `[tool] ToolName path` log line.
fn parse_tool_path(line: &str) -> Option<String> {
    let tag = "[tool] ";
    let idx = line.find(tag)?;
    let rest = &line[idx + tag.len()..];
    let mut parts = rest.splitn(2, ' ');
    let _tool = parts.next()?;
    let path = parts.next()?.trim();
    if path.is_empty() {
        return None;
    }
    Some(path.to_string())
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    let m = secs / 60;
    let s = secs % 60;
    let mut out = String::new();
    write!(out, "{m}m").ok();
    if s > 0 {
        write!(out, "{s}s").ok();
    }
    out
}

fn format_tokens(total: u64) -> String {
    if total >= 1_000_000 {
        format!("{:.1}M", total as f64 / 1_000_000.0)
    } else if total >= 1_000 {
        format!("{:.1}k", total as f64 / 1_000.0)
    } else {
        total.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_logs() -> Vec<String> {
        vec![
            "[tool] Read src/main.rs".into(),
            "[tool] Read src/lib.rs".into(),
            "[tool] Edit src/main.rs".into(),
            "[tool] Bash cargo test".into(),
            "[tool] Write src/new_file.rs".into(),
            "[tool] Bash cargo check".into(),
            "[tool] Read src/main.rs".into(), // duplicate
        ]
    }

    fn sample_ctx() -> FailureContext {
        FailureContext {
            unit_id: "42".into(),
            unit_title: "Add widget".into(),
            attempt: 2,
            duration_secs: 185,
            tool_count: 7,
            turns: 4,
            input_tokens: 50_000,
            output_tokens: 12_000,
            cost: 0.045,
            error: Some("idle timeout after 300s".into()),
            tool_log: sample_logs(),
            verify_command: Some("cargo test widget".into()),
        }
    }

    // -- extract_tool_paths --

    #[test]
    fn extract_tool_paths_deduplicates() {
        let logs = sample_logs();
        let reads = extract_tool_paths(&logs, "Read");
        assert_eq!(reads, vec!["src/main.rs", "src/lib.rs"]);
    }

    #[test]
    fn extract_tool_paths_returns_empty_for_missing_tool() {
        let logs = sample_logs();
        let grepped = extract_tool_paths(&logs, "Grep");
        assert!(grepped.is_empty());
    }

    // -- count_tool --

    #[test]
    fn count_tool_counts_all_occurrences() {
        let logs = sample_logs();
        assert_eq!(count_tool(&logs, "Read"), 3);
        assert_eq!(count_tool(&logs, "Bash"), 2);
        assert_eq!(count_tool(&logs, "Write"), 1);
        assert_eq!(count_tool(&logs, "Grep"), 0);
    }

    // -- extract_files_from_logs --

    #[test]
    fn extract_files_deduplicates_across_tools() {
        let logs = sample_logs();
        let files = extract_files_from_logs(&logs);
        assert_eq!(files, vec!["src/main.rs", "src/lib.rs", "src/new_file.rs"]);
    }

    #[test]
    fn extract_files_skips_multi_word_args() {
        let logs = vec![
            "[tool] Bash cargo test --release".into(),
            "[tool] Read src/foo.rs".into(),
        ];
        let files = extract_files_from_logs(&logs);
        assert_eq!(files, vec!["src/foo.rs"]);
    }

    // -- extract_last_tools --

    #[test]
    fn extract_last_tools_returns_last_n() {
        let logs = sample_logs();
        let last = extract_last_tools(&logs, 3);
        assert_eq!(last, vec!["Write", "Bash", "Read"]);
    }

    #[test]
    fn extract_last_tools_returns_all_when_fewer_than_n() {
        let logs = vec!["[tool] Read src/a.rs".into()];
        let last = extract_last_tools(&logs, 5);
        assert_eq!(last, vec!["Read"]);
    }

    // -- summarize_paths --

    #[test]
    fn summarize_paths_three_or_fewer() {
        let paths: Vec<String> = vec!["a.rs".into(), "b.rs".into()];
        assert_eq!(summarize_paths(&paths), "a.rs, b.rs");
    }

    #[test]
    fn summarize_paths_more_than_three() {
        let paths: Vec<String> = vec![
            "a.rs".into(),
            "b.rs".into(),
            "c.rs".into(),
            "d.rs".into(),
            "e.rs".into(),
        ];
        assert_eq!(summarize_paths(&paths), "a.rs, b.rs, c.rs +2 more");
    }

    // -- format helpers --

    #[test]
    fn format_duration_seconds_only() {
        assert_eq!(format_duration(42), "42s");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(185), "3m5s");
    }

    #[test]
    fn format_duration_exact_minutes() {
        assert_eq!(format_duration(120), "2m");
    }

    #[test]
    fn format_tokens_raw() {
        assert_eq!(format_tokens(500), "500");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(62_000), "62.0k");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    // -- build_failure_summary integration --

    #[test]
    fn summary_contains_all_sections() {
        let ctx = sample_ctx();
        let summary = build_failure_summary(&ctx);

        assert!(summary.contains("## Attempt 2 Failed"));
        assert!(summary.contains("3m5s"));
        assert!(summary.contains("62.0k tokens"));
        assert!(summary.contains("$0.045"));

        assert!(summary.contains("### What was tried"));
        assert!(summary.contains("Read src/main.rs, src/lib.rs"));
        assert!(summary.contains("Edited src/main.rs"));
        assert!(summary.contains("Wrote src/new_file.rs"));
        assert!(summary.contains("Ran 2 bash commands"));
        assert!(summary.contains("7 tool calls over 4 turns"));

        assert!(summary.contains("### Why it failed"));
        assert!(summary.contains("idle timeout after 300s"));
        assert!(summary.contains("Last tools before failure:"));

        assert!(summary.contains("### Files touched"));
        assert!(summary.contains("- src/main.rs"));
        assert!(summary.contains("- src/lib.rs"));

        assert!(summary.contains("### Verify command"));
        assert!(summary.contains("`cargo test widget`"));

        assert!(summary.contains("### Suggestion for next attempt"));
        assert!(summary.contains("stuck in a loop"));
    }

    #[test]
    fn summary_without_error_shows_unknown() {
        let ctx = FailureContext {
            error: None,
            tool_log: vec![],
            verify_command: None,
            ..sample_ctx()
        };
        let summary = build_failure_summary(&ctx);
        assert!(summary.contains("Unknown failure (no error captured)"));
        // No suggestion section when error is None
        assert!(!summary.contains("### Suggestion for next attempt"));
    }

    #[test]
    fn suggestion_timeout_generic() {
        let suggestion = build_suggestion(Some("total timeout exceeded"));
        assert!(suggestion.unwrap().contains("ran out of time"));
    }

    #[test]
    fn suggestion_idle_timeout_more_specific() {
        // "idle timeout" should match before generic "timeout"
        let suggestion = build_suggestion(Some("idle timeout after 300s"));
        assert!(suggestion.unwrap().contains("stuck in a loop"));
    }

    #[test]
    fn suggestion_aborted() {
        let suggestion = build_suggestion(Some("process aborted by user"));
        assert!(suggestion.unwrap().contains("manually aborted"));
    }

    #[test]
    fn suggestion_claim() {
        let suggestion = build_suggestion(Some("failed to claim unit"));
        assert!(suggestion.unwrap().contains("another agent"));
    }

    #[test]
    fn suggestion_exit_code() {
        let suggestion = build_suggestion(Some("exit code 1"));
        assert!(suggestion.unwrap().contains("verify command output"));
    }

    #[test]
    fn suggestion_none_for_unknown_error() {
        let suggestion = build_suggestion(Some("something weird happened"));
        assert!(suggestion.is_none());
    }

    #[test]
    fn singular_bash_command() {
        let ctx = FailureContext {
            tool_log: vec!["[tool] Bash cargo test".into()],
            ..sample_ctx()
        };
        let summary = build_failure_summary(&ctx);
        assert!(summary.contains("Ran 1 bash command\n"));
    }
}
