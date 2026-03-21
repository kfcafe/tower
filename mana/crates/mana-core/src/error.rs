use std::path::PathBuf;

/// Typed error for mana-core's public API surface.
///
/// Callers can match on variants to handle specific failure modes
/// (e.g., distinguishing "unit not found" from "file corrupt").
/// Internal functions still use `anyhow` — this type lives at the API boundary.
#[derive(Debug, thiserror::Error)]
pub enum ManaError {
    /// The requested unit ID does not exist.
    #[error("Unit {id} not found")]
    UnitNotFound { id: String },

    /// The unit ID is syntactically invalid (empty, special chars, path traversal).
    #[error("Invalid unit ID: {id} — {reason}")]
    InvalidId { id: String, reason: String },

    /// Adding a dependency would create a cycle in the graph.
    #[error("Dependency cycle detected: {details}")]
    CycleDetected { details: String },

    /// The verify command failed or timed out.
    #[error("Verify failed: {reason}")]
    VerifyFailed { reason: String },

    /// Problem reading or validating `config.yaml`.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Problem building, loading, or saving the index.
    #[error("Index error: {0}")]
    IndexError(String),

    /// Could not acquire the index lock (another process may hold it).
    #[error("Lock conflict: {0}")]
    LockConflict(String),

    /// YAML/frontmatter deserialization failed.
    #[error("Parse error in {path}: {reason}")]
    ParseError { path: PathBuf, reason: String },

    /// Filesystem I/O failure.
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    /// Catch-all for internal errors that don't fit a specific variant.
    /// Preserves the original `anyhow::Error` for debugging.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Convenience alias used throughout the public API.
pub type ManaResult<T> = std::result::Result<T, ManaError>;
