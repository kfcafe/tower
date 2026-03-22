//! File locking for concurrent agents.
//!
//! When `file_locking` is enabled in config, agents lock files they work on
//! to prevent concurrent writes. Locks are stored as JSON files in `.mana/locks/`.
//!
//! Lock lifecycle:
//! - Pre-emptive: `mana run` locks files listed in the unit's `paths` field on spawn.
//! - On-write: The pi extension locks files on first write (safety net).
//! - Release: Locks are released when the agent finishes or is killed.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Information stored in each lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub unit_id: String,
    pub pid: u32,
    pub file_path: String,
    pub locked_at: i64,
}

/// A lock with its file system path.
#[derive(Debug)]
pub struct ActiveLock {
    pub info: LockInfo,
    pub lock_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Lock directory
// ---------------------------------------------------------------------------

/// Return the locks directory, creating it if needed.
pub fn lock_dir(mana_dir: &Path) -> Result<PathBuf> {
    let dir = mana_dir.join("locks");
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create locks directory: {}", dir.display()))?;
    Ok(dir)
}

/// Hash a file path to a lock filename.
fn lock_filename(file_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}.lock", hash)
}

/// Full path to the lock file for a given file path.
fn lock_file_path(mana_dir: &Path, file_path: &str) -> Result<PathBuf> {
    let dir = lock_dir(mana_dir)?;
    Ok(dir.join(lock_filename(file_path)))
}

// ---------------------------------------------------------------------------
// Lock operations
// ---------------------------------------------------------------------------

/// Acquire a lock on a file for a unit agent.
///
/// Returns `Ok(true)` if the lock was acquired, `Ok(false)` if already locked
/// by another live process. Stale locks (dead PID) are automatically cleaned.
///
/// Uses atomic file creation (`O_CREAT | O_EXCL`) to prevent TOCTOU races
/// when multiple agents attempt to lock the same file concurrently.
pub fn acquire(mana_dir: &Path, unit_id: &str, pid: u32, file_path: &str) -> Result<bool> {
    let lock_path = lock_file_path(mana_dir, file_path)?;

    let info = LockInfo {
        unit_id: unit_id.to_string(),
        pid,
        file_path: file_path.to_string(),
        locked_at: chrono::Utc::now().timestamp(),
    };

    let content = serde_json::to_string_pretty(&info).context("Failed to serialize lock info")?;

    // Attempt atomic creation — retries once after cleaning a stale lock.
    for _ in 0..2 {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                file.write_all(content.as_bytes()).with_context(|| {
                    format!("Failed to write lock file: {}", lock_path.display())
                })?;
                return Ok(true);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                match read_lock(&lock_path) {
                    Some(existing) if existing.unit_id == unit_id && existing.pid == pid => {
                        // Same owner re-acquiring — idempotent success
                        return Ok(true);
                    }
                    Some(existing) if is_process_alive(existing.pid) => {
                        // Held by a live process
                        return Ok(false);
                    }
                    _ => {
                        // Stale or corrupt — remove and retry
                        let _ = fs::remove_file(&lock_path);
                    }
                }
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("Failed to create lock file: {}", lock_path.display())
                });
            }
        }
    }

    // Both attempts failed — another agent won the race
    Ok(false)
}

/// Release all locks held by a specific unit.
pub fn release_all_for_unit(mana_dir: &Path, unit_id: &str) -> Result<u32> {
    let mut released = 0;
    for lock in list_locks(mana_dir)? {
        if lock.info.unit_id == unit_id {
            let _ = fs::remove_file(&lock.lock_path);
            released += 1;
        }
    }
    Ok(released)
}

/// Force-clear all locks.
pub fn clear_all(mana_dir: &Path) -> Result<u32> {
    let mut cleared = 0;
    for lock in list_locks(mana_dir)? {
        let _ = fs::remove_file(&lock.lock_path);
        cleared += 1;
    }
    Ok(cleared)
}

/// List all active locks, cleaning stale ones (dead PIDs) along the way.
pub fn list_locks(mana_dir: &Path) -> Result<Vec<ActiveLock>> {
    let dir = lock_dir(mana_dir)?;
    let mut locks = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(locks),
        Err(e) => return Err(e).context("Failed to read locks directory"),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("lock") {
            continue;
        }

        match read_lock(&path) {
            Some(info) if is_process_alive(info.pid) => {
                locks.push(ActiveLock {
                    info,
                    lock_path: path,
                });
            }
            _ => {
                // Stale or corrupt — clean up
                let _ = fs::remove_file(&path);
            }
        }
    }

    Ok(locks)
}

/// Check if a file is currently locked.
///
/// Returns the lock info if locked by a live process, None otherwise.
/// Automatically cleans stale locks.
pub fn check_lock(mana_dir: &Path, file_path: &str) -> Result<Option<LockInfo>> {
    let lock_path = lock_file_path(mana_dir, file_path)?;

    if !lock_path.exists() {
        return Ok(None);
    }

    match read_lock(&lock_path) {
        Some(info) => {
            if is_process_alive(info.pid) {
                Ok(Some(info))
            } else {
                // Stale — clean it up
                let _ = fs::remove_file(&lock_path);
                Ok(None)
            }
        }
        None => {
            // Corrupt — clean it up
            let _ = fs::remove_file(&lock_path);
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_lock(path: &Path) -> Option<LockInfo> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn is_process_alive(pid: u32) -> bool {
    // signal 0 checks existence without actually signaling.
    // Returns 0 if alive and we have permission to signal.
    // Returns -1 with EPERM if alive but owned by another user.
    // Returns -1 with ESRCH if the process does not exist.
    let ret = unsafe { libc::kill(pid as i32, 0) };
    if ret == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_mana_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();
        (dir, mana_dir)
    }

    #[test]
    fn acquire_and_release_via_unit() {
        let (_dir, mana_dir) = temp_mana_dir();
        let pid = std::process::id();

        let acquired = acquire(&mana_dir, "1.1", pid, "/tmp/test.rs").unwrap();
        assert!(acquired);

        let info = check_lock(&mana_dir, "/tmp/test.rs").unwrap();
        assert!(info.is_some());
        assert_eq!(info.unwrap().unit_id, "1.1");

        release_all_for_unit(&mana_dir, "1.1").unwrap();

        let info = check_lock(&mana_dir, "/tmp/test.rs").unwrap();
        assert!(info.is_none());
    }

    #[test]
    fn release_all_for_unit_works() {
        let (_dir, mana_dir) = temp_mana_dir();
        let pid = std::process::id();

        acquire(&mana_dir, "2.1", pid, "/tmp/a.rs").unwrap();
        acquire(&mana_dir, "2.1", pid, "/tmp/b.rs").unwrap();
        acquire(&mana_dir, "2.2", pid, "/tmp/c.rs").unwrap();

        let released = release_all_for_unit(&mana_dir, "2.1").unwrap();
        assert_eq!(released, 2);

        // c.rs should still be locked
        assert!(check_lock(&mana_dir, "/tmp/c.rs").unwrap().is_some());
        assert!(check_lock(&mana_dir, "/tmp/a.rs").unwrap().is_none());
    }

    #[test]
    fn list_locks_returns_all() {
        let (_dir, mana_dir) = temp_mana_dir();
        let pid = std::process::id();

        acquire(&mana_dir, "3.1", pid, "/tmp/x.rs").unwrap();
        acquire(&mana_dir, "3.2", pid, "/tmp/y.rs").unwrap();

        let locks = list_locks(&mana_dir).unwrap();
        assert_eq!(locks.len(), 2);
    }

    #[test]
    fn clear_all_removes_everything() {
        let (_dir, mana_dir) = temp_mana_dir();
        let pid = std::process::id();

        acquire(&mana_dir, "4.1", pid, "/tmp/p.rs").unwrap();
        acquire(&mana_dir, "4.2", pid, "/tmp/q.rs").unwrap();

        let cleared = clear_all(&mana_dir).unwrap();
        assert_eq!(cleared, 2);

        let locks = list_locks(&mana_dir).unwrap();
        assert!(locks.is_empty());
    }

    #[test]
    fn stale_lock_is_cleaned() {
        let (_dir, mana_dir) = temp_mana_dir();

        // Write a lock with a dead PID
        let lock_path = lock_file_path(&mana_dir, "/tmp/stale.rs").unwrap();
        let info = LockInfo {
            unit_id: "5.1".to_string(),
            pid: 999_999_999, // almost certainly dead
            file_path: "/tmp/stale.rs".to_string(),
            locked_at: 0,
        };
        fs::write(&lock_path, serde_json::to_string(&info).unwrap()).unwrap();

        // check_lock should clean it
        let result = check_lock(&mana_dir, "/tmp/stale.rs").unwrap();
        assert!(result.is_none());
        assert!(!lock_path.exists());
    }

    #[test]
    fn acquire_cleans_stale_and_succeeds() {
        let (_dir, mana_dir) = temp_mana_dir();

        // Plant a stale lock
        let lock_path = lock_file_path(&mana_dir, "/tmp/stale2.rs").unwrap();
        let info = LockInfo {
            unit_id: "6.1".to_string(),
            pid: 999_999_999,
            file_path: "/tmp/stale2.rs".to_string(),
            locked_at: 0,
        };
        fs::write(&lock_path, serde_json::to_string(&info).unwrap()).unwrap();

        // Acquire should clean the stale lock and succeed
        let acquired = acquire(&mana_dir, "6.2", std::process::id(), "/tmp/stale2.rs").unwrap();
        assert!(acquired);
    }

    #[test]
    fn same_owner_reacquire_is_idempotent() {
        let (_dir, mana_dir) = temp_mana_dir();
        let pid = std::process::id();

        let first = acquire(&mana_dir, "7.1", pid, "/tmp/idem.rs").unwrap();
        assert!(first);

        // Same unit + PID re-acquiring should succeed, not block itself
        let second = acquire(&mana_dir, "7.1", pid, "/tmp/idem.rs").unwrap();
        assert!(second);

        // Lock should still be valid
        let info = check_lock(&mana_dir, "/tmp/idem.rs").unwrap();
        assert!(info.is_some());
        assert_eq!(info.unwrap().unit_id, "7.1");
    }

    #[test]
    fn different_owner_blocked_by_live_lock() {
        let (_dir, mana_dir) = temp_mana_dir();
        let pid = std::process::id();

        let first = acquire(&mana_dir, "8.1", pid, "/tmp/contested.rs").unwrap();
        assert!(first);

        // Different unit trying to acquire the same file should be blocked
        let second = acquire(&mana_dir, "8.2", pid + 1, "/tmp/contested.rs").unwrap();
        assert!(!second);
    }

    #[test]
    fn list_locks_filters_stale() {
        let (_dir, mana_dir) = temp_mana_dir();
        let pid = std::process::id();

        // One live lock
        acquire(&mana_dir, "9.1", pid, "/tmp/live.rs").unwrap();

        // One stale lock (manually planted)
        let stale_path = lock_file_path(&mana_dir, "/tmp/ghost.rs").unwrap();
        let stale = LockInfo {
            unit_id: "9.2".to_string(),
            pid: 999_999_999,
            file_path: "/tmp/ghost.rs".to_string(),
            locked_at: 0,
        };
        fs::write(&stale_path, serde_json::to_string(&stale).unwrap()).unwrap();

        // list_locks should return only the live one and clean the stale one
        let locks = list_locks(&mana_dir).unwrap();
        assert_eq!(locks.len(), 1);
        assert_eq!(locks[0].info.unit_id, "9.1");
        assert!(!stale_path.exists());
    }
}
