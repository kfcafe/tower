use std::path::Path;

use anyhow::Result;
use mana_core::ops::verify as ops_verify;

use crate::output::Output;

/// Run the verify command for a unit without closing it.
///
/// Returns `Ok(true)` if the command exits 0, `Ok(false)` if non-zero or timed out.
/// If no verify command is set, prints a message and returns `Ok(true)`.
/// Respects `verify_timeout` from the unit or project config.
pub fn cmd_verify(mana_dir: &Path, id: &str, out: &Output) -> Result<bool> {
    let result = ops_verify::run_verify(mana_dir, id)?;

    let Some(result) = result else {
        out.info(&format!("no verify command set for unit {}", id));
        return Ok(true);
    };

    out.info(&format!("Running: {}", result.command));
    if let Some(secs) = result.timeout_secs {
        out.info(&format!("Timeout: {}s", secs));
    }

    if !result.stdout.trim().is_empty() {
        print!("{}", result.stdout);
    }
    if !result.stderr.trim().is_empty() {
        eprint!("{}", result.stderr);
    }

    if result.timed_out {
        out.warn(&format!(
            "Verify timed out after {}s for unit {}",
            result.timeout_secs.unwrap_or(0),
            id
        ));
        return Ok(false);
    }

    if result.passed {
        out.success(id, "Verify passed");
        Ok(true)
    } else {
        out.error(&format!("Verify failed for unit {}", id));
        Ok(false)
    }
}
