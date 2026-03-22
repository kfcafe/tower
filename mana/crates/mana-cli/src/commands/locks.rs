//! `mana locks` — View and manage file locks.

use std::path::Path;

use anyhow::Result;

use crate::locks;

/// List all active file locks.
pub fn cmd_locks(mana_dir: &Path) -> Result<()> {
    let active = locks::list_locks(mana_dir)?;

    if active.is_empty() {
        eprintln!("No active file locks.");
        return Ok(());
    }

    eprintln!("{} active lock(s):\n", active.len());
    for lock in &active {
        let age = chrono::Utc::now().timestamp() - lock.info.locked_at;
        let age_str = if age < 60 {
            format!("{}s", age)
        } else if age < 3600 {
            format!("{}m", age / 60)
        } else {
            format!("{}h", age / 3600)
        };

        eprintln!(
            "  {} — unit {} (pid {}, {})",
            lock.info.file_path, lock.info.unit_id, lock.info.pid, age_str
        );
    }

    Ok(())
}

/// Force-clear all file locks.
pub fn cmd_locks_clear(mana_dir: &Path) -> Result<()> {
    let cleared = locks::clear_all(mana_dir)?;
    if cleared == 0 {
        eprintln!("No locks to clear.");
    } else {
        eprintln!("Cleared {} lock(s).", cleared);
    }
    Ok(())
}
