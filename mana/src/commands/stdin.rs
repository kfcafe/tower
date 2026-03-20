//! Utilities for reading from stdin in pipe-friendly commands.
//!
//! Supports the `-` convention: `--description -` reads from stdin.
//! Also supports reading unit IDs from stdin, one per line.

use std::io::{self, IsTerminal, Read};

use anyhow::{Context, Result};

/// Read all of stdin into a string.
/// Returns an error if stdin is a terminal (not piped).
pub fn read_stdin() -> Result<String> {
    if io::stdin().is_terminal() {
        anyhow::bail!(
            "Expected piped input but stdin is a terminal.\n\
             Use: echo \"content\" | mana ... --description -"
        );
    }

    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .context("Failed to read from stdin")?;
    Ok(buf)
}

/// Resolve a value that might be "-" (meaning read from stdin).
/// If the value is "-", reads stdin. Otherwise returns the value as-is.
pub fn resolve_stdin_value(value: String) -> Result<String> {
    if value == "-" {
        read_stdin()
    } else {
        Ok(value)
    }
}

/// Resolve an `Option<String>` that might contain "-".
pub fn resolve_stdin_opt(value: Option<String>) -> Result<Option<String>> {
    match value {
        Some(v) if v == "-" => Ok(Some(read_stdin()?)),
        other => Ok(other),
    }
}

/// Read unit IDs from stdin, one per line.
/// Trims whitespace and skips empty lines.
pub fn read_ids_from_stdin() -> Result<Vec<String>> {
    let content = read_stdin()?;
    let ids: Vec<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if ids.is_empty() {
        anyhow::bail!("No unit IDs found on stdin");
    }

    Ok(ids)
}
