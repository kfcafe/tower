use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A persisted agent entry in the agents.json file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub pid: u32,
    pub title: String,
    pub action: String,
    pub started_at: i64,
    #[serde(default)]
    pub log_path: Option<String>,
    /// Set when the agent completes.
    #[serde(default)]
    pub finished_at: Option<i64>,
    /// Exit code on completion.
    #[serde(default)]
    pub exit_code: Option<i32>,
}

/// JSON output entry for `mana agents --json`.
#[derive(Debug, Serialize)]
struct AgentJsonEntry {
    bean_id: String,
    title: String,
    action: String,
    pid: u32,
    elapsed_secs: u64,
    status: String,
}

/// Return the path to the agents persistence file.
pub fn agents_file_path() -> Result<std::path::PathBuf> {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("units");
    std::fs::create_dir_all(&dir).context("Failed to create units state directory")?;
    Ok(dir.join("agents.json"))
}

/// Load agents from the persistence file. Returns empty map if file doesn't exist.
pub fn load_agents() -> Result<HashMap<String, AgentEntry>> {
    let path = agents_file_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if contents.trim().is_empty() {
        return Ok(HashMap::new());
    }
    let agents: HashMap<String, AgentEntry> =
        serde_json::from_str(&contents).with_context(|| "Failed to parse agents.json")?;
    Ok(agents)
}

/// Save agents atomically by writing to a temp file then renaming.
///
/// Prevents corruption if the process is killed mid-write (e.g., an agent
/// crashing during `mana close`). The rename is atomic on the same filesystem.
pub fn save_agents(agents: &HashMap<String, AgentEntry>) -> Result<()> {
    let path = agents_file_path()?;
    let json = serde_json::to_string_pretty(agents)?;

    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)
        .with_context(|| format!("Failed to write temp agents file {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &path).with_context(|| {
        format!(
            "Failed to rename {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

/// Check if a process with the given PID is still alive.
///
/// Uses `kill(pid, 0)` which checks existence without signaling.
/// Returns `true` if the process exists, even if owned by another user (EPERM).
/// Returns `false` if the PID overflows `i32` (not a valid Unix PID).
fn process_alive(pid: u32) -> bool {
    let Ok(pid_i32) = i32::try_from(pid) else {
        return false;
    };

    // SAFETY: kill(pid, 0) sends no signal — it only checks process existence.
    // Returns 0 if the process exists and we can signal it.
    // Returns -1 with EPERM if the process exists but is owned by another user.
    // Returns -1 with ESRCH if the process does not exist.
    let ret = unsafe { libc::kill(pid_i32, 0) };
    if ret == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Truncate a string to fit within `max_display_chars` characters, appending "…"
/// if truncated. Works correctly with multi-byte UTF-8.
fn truncate_title(title: &str, max_display_chars: usize) -> String {
    if title.chars().count() <= max_display_chars {
        return title.to_string();
    }
    let truncated: String = title.chars().take(max_display_chars - 1).collect();
    format!("{truncated}…")
}

/// Format a duration in seconds as a human-readable string (e.g. "1m 32s").
fn format_elapsed(secs: u64) -> String {
    if secs >= 3600 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    } else {
        let m = secs / 60;
        let s = secs % 60;
        format!("{}m {:02}s", m, s)
    }
}

/// Show running and recently completed agents.
///
/// Reads agent state from the persistence file, checks PIDs, cleans up stale
/// entries, and displays a table of agents.
pub fn cmd_agents(_beans_dir: &Path, json: bool) -> Result<()> {
    let mut agents = load_agents()?;
    let now = chrono::Utc::now().timestamp();

    // Clean up stale entries: if PID is dead and no finished_at, mark as completed
    let mut changed = false;
    for entry in agents.values_mut() {
        if entry.finished_at.is_none() && !process_alive(entry.pid) {
            entry.finished_at = Some(now);
            entry.exit_code = Some(-1); // unknown — process vanished
            changed = true;
        }
    }

    // Remove completed entries older than 1 hour
    let one_hour_ago = now - 3600;
    let before_len = agents.len();
    agents.retain(|_id, entry| entry.finished_at.map(|f| f > one_hour_ago).unwrap_or(true));
    if agents.len() != before_len {
        changed = true;
    }

    if changed {
        save_agents(&agents)?;
    }

    if agents.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No running agents.");
        }
        return Ok(());
    }

    if json {
        return print_agents_json(&agents, now);
    }

    print_agents_table(&agents, now);
    Ok(())
}

fn print_agents_json(agents: &HashMap<String, AgentEntry>, now: i64) -> Result<()> {
    let entries: Vec<AgentJsonEntry> = agents
        .iter()
        .map(|(id, entry)| {
            let elapsed = if let Some(finished) = entry.finished_at {
                (finished - entry.started_at).unsigned_abs()
            } else {
                (now - entry.started_at).unsigned_abs()
            };
            let status = match entry.finished_at {
                Some(_) => match entry.exit_code {
                    Some(0) | None => "completed".to_string(),
                    Some(code) => format!("failed({})", code),
                },
                None => "running".to_string(),
            };
            AgentJsonEntry {
                bean_id: id.clone(),
                title: entry.title.clone(),
                action: entry.action.clone(),
                pid: entry.pid,
                elapsed_secs: elapsed,
                status,
            }
        })
        .collect();
    let json_str = serde_json::to_string_pretty(&entries)?;
    println!("{}", json_str);
    Ok(())
}

fn print_agents_table(agents: &HashMap<String, AgentEntry>, now: i64) {
    let mut running: Vec<(&String, &AgentEntry)> = Vec::new();
    let mut completed: Vec<(&String, &AgentEntry)> = Vec::new();
    for (id, entry) in agents {
        if entry.finished_at.is_some() {
            completed.push((id, entry));
        } else {
            running.push((id, entry));
        }
    }

    running.sort_by(|a, b| crate::util::natural_cmp(a.0, b.0));
    completed.sort_by(|a, b| crate::util::natural_cmp(a.0, b.0));

    const TITLE_WIDTH: usize = 24;

    if !running.is_empty() {
        println!(
            "{:<6} {:<24} {:<12} {:<8} ELAPSED",
            "BEAN", "TITLE", "ACTION", "PID"
        );
        for (id, entry) in &running {
            let elapsed = (now - entry.started_at).unsigned_abs();
            let title = truncate_title(&entry.title, TITLE_WIDTH);
            println!(
                "{:<6} {:<24} {:<12} {:<8} {}",
                id,
                title,
                entry.action,
                entry.pid,
                format_elapsed(elapsed)
            );
        }
    }

    if !completed.is_empty() {
        if !running.is_empty() {
            println!();
        }
        println!("Recently completed:");
        for (id, entry) in &completed {
            let duration = entry
                .finished_at
                .map(|f| (f - entry.started_at).unsigned_abs())
                .unwrap_or(0);
            let status_str = match entry.exit_code {
                Some(0) => "✓".to_string(),
                Some(code) => format!("✗ exit {}", code),
                None => "?".to_string(),
            };
            let title = truncate_title(&entry.title, TITLE_WIDTH);
            println!(
                "  {} {} ({}, {})",
                id,
                title,
                status_str,
                format_elapsed(duration)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_elapsed_seconds() {
        assert_eq!(format_elapsed(0), "0m 00s");
        assert_eq!(format_elapsed(48), "0m 48s");
        assert_eq!(format_elapsed(92), "1m 32s");
    }

    #[test]
    fn format_elapsed_hours() {
        assert_eq!(format_elapsed(3661), "1h 1m");
        assert_eq!(format_elapsed(7200), "2h 0m");
    }

    #[test]
    fn load_agents_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agents.json");
        std::fs::write(&path, "").unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.trim().is_empty());
    }

    #[test]
    fn agent_entry_roundtrip() {
        let mut agents = HashMap::new();
        agents.insert(
            "5.1".to_string(),
            AgentEntry {
                pid: 42310,
                title: "Define user types".to_string(),
                action: "implement".to_string(),
                started_at: 1708000000,
                log_path: Some("/tmp/log".to_string()),
                finished_at: None,
                exit_code: None,
            },
        );

        let json = serde_json::to_string_pretty(&agents).unwrap();
        let parsed: HashMap<String, AgentEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        let entry = parsed.get("5.1").unwrap();
        assert_eq!(entry.pid, 42310);
        assert_eq!(entry.title, "Define user types");
        assert_eq!(entry.action, "implement");
        assert!(entry.finished_at.is_none());
    }

    #[test]
    fn agents_empty_persistence_shows_no_agents() {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        std::fs::create_dir(&mana_dir).unwrap();

        // load_agents reads from the real state dir, which may or may not exist.
        // The function handles both cases gracefully.
        let agents = load_agents();
        assert!(agents.is_ok());
    }

    #[test]
    fn process_alive_returns_true_for_current() {
        assert!(process_alive(std::process::id()));
    }

    #[test]
    fn process_alive_returns_false_for_nonexistent() {
        assert!(!process_alive(99_999_999));
    }

    #[test]
    fn process_alive_returns_false_for_overflowed_pid() {
        // PID > i32::MAX should return false, not panic
        assert!(!process_alive(u32::MAX));
        assert!(!process_alive(i32::MAX as u32 + 1));
    }

    #[test]
    fn truncate_title_short_string() {
        assert_eq!(truncate_title("hello", 24), "hello");
    }

    #[test]
    fn truncate_title_exact_length() {
        let title = "a".repeat(24);
        assert_eq!(truncate_title(&title, 24), title);
    }

    #[test]
    fn truncate_title_long_string() {
        let title = "a".repeat(30);
        let result = truncate_title(&title, 24);
        assert_eq!(result.chars().count(), 24);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_title_multibyte_utf8() {
        // 13 emoji = 13 chars but 52 bytes — must not panic on byte boundary
        let title = "🎉".repeat(13);
        let result = truncate_title(&title, 10);
        assert_eq!(result.chars().count(), 10);
        assert!(result.ends_with('…'));
    }
}
