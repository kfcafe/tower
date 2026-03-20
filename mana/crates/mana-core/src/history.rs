use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AgentHistoryEntry {
    pub bean_id: String,
    pub title: String,
    pub attempt: u32,
    pub success: bool,
    pub duration_secs: u64,
    pub tokens: u64,
    pub cost: f64,
    pub tool_count: usize,
    pub error: Option<String>,
    pub model: String,
    pub timestamp: String,
}

/// Append a completion record to `.mana/agent_history.jsonl`.
///
/// Gracefully swallows errors — logging should never crash the agent.
pub fn append_history(mana_dir: &Path, entry: &AgentHistoryEntry) {
    let _ = try_append(mana_dir, entry);
}

fn try_append(
    mana_dir: &Path,
    entry: &AgentHistoryEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = mana_dir.join("agent_history.jsonl");
    let line = serde_json::to_string(entry)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_entry(success: bool) -> AgentHistoryEntry {
        AgentHistoryEntry {
            bean_id: "42".to_string(),
            title: "Test unit".to_string(),
            attempt: 1,
            success,
            duration_secs: 30,
            tokens: 5000,
            cost: 0.03,
            tool_count: 12,
            error: None,
            model: "default".to_string(),
            timestamp: "2026-03-03T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn append_creates_file_and_writes_valid_jsonl() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let entry = make_entry(true);
        append_history(&mana_dir, &entry);

        let content = fs::read_to_string(mana_dir.join("agent_history.jsonl")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["bean_id"], "42");
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["tokens"], 5000);
        assert_eq!(parsed["cost"], 0.03);
    }

    #[test]
    fn append_appends_multiple_lines() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        append_history(&mana_dir, &make_entry(true));
        append_history(&mana_dir, &make_entry(false));

        let content = fs::read_to_string(mana_dir.join("agent_history.jsonl")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(first["success"], true);
        assert_eq!(second["success"], false);
    }

    #[test]
    fn append_error_field_serialized_when_present() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let mut entry = make_entry(false);
        entry.error = Some("Exit code 1".to_string());
        append_history(&mana_dir, &entry);

        let content = fs::read_to_string(mana_dir.join("agent_history.jsonl")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["error"], "Exit code 1");
    }

    #[test]
    fn append_error_field_null_when_none() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        append_history(&mana_dir, &make_entry(true));

        let content = fs::read_to_string(mana_dir.join("agent_history.jsonl")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert!(parsed["error"].is_null());
    }

    #[test]
    fn append_swallows_errors_on_missing_dir() {
        let dir = TempDir::new().unwrap();
        let bogus = dir.path().join("nonexistent");

        // Should not panic
        append_history(&bogus, &make_entry(true));
    }
}
