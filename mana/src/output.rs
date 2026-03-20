/// Thin output abstraction for consistent CLI messaging.
///
/// All human-readable messages go to stderr, keeping stdout clean for
/// machine-readable output (JSON, piped IDs). Supports quiet mode
/// to suppress informational messages while preserving warnings and errors.
pub struct Output {
    quiet: bool,
}

impl Output {
    /// Create a new Output with default settings (not quiet).
    pub fn new() -> Self {
        Self { quiet: false }
    }

    /// Create an Output from a quiet flag.
    pub fn with_quiet(quiet: bool) -> Self {
        Self { quiet }
    }

    /// Informational message. Suppressed in quiet mode.
    pub fn info(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{}", msg);
        }
    }

    /// Success message with a unit ID prefix. Suppressed in quiet mode.
    pub fn success(&self, id: &str, msg: &str) {
        if !self.quiet {
            eprintln!("  ✓ {}  {}", id, msg);
        }
    }

    /// Warning message. Never suppressed.
    pub fn warn(&self, msg: &str) {
        eprintln!("  ⚠ {}", msg);
    }

    /// Error message. Never suppressed.
    pub fn error(&self, msg: &str) {
        eprintln!("  ✗ {}", msg);
    }
}

impl Default for Output {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_new_is_not_quiet() {
        let out = Output::new();
        assert!(!out.quiet);
    }

    #[test]
    fn output_default_is_not_quiet() {
        let out = Output::default();
        assert!(!out.quiet);
    }

    #[test]
    fn output_with_quiet_true() {
        let out = Output::with_quiet(true);
        assert!(out.quiet);
    }

    #[test]
    fn output_with_quiet_false() {
        let out = Output::with_quiet(false);
        assert!(!out.quiet);
    }
}
