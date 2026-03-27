use regex::Regex;
use std::sync::OnceLock;

/// Severity of a verify command lint finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyLintLevel {
    Error,
    Warning,
}

/// A lint finding for a verify command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyLintResult {
    pub level: VerifyLintLevel,
    pub message: String,
}

impl VerifyLintResult {
    fn error(message: impl Into<String>) -> Self {
        Self {
            level: VerifyLintLevel::Error,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            level: VerifyLintLevel::Warning,
            message: message.into(),
        }
    }
}

/// Lint a verify command for known anti-patterns.
#[must_use]
pub fn lint_verify(cmd: &str) -> Vec<VerifyLintResult> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return vec![VerifyLintResult::error(
            "Verify command is empty. Use a command that can fail when the unit is incomplete.",
        )];
    }

    let segments: Vec<&str> = shell_segment_splitter()
        .split(trimmed)
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect();
    let meaningful_segments: Vec<&str> = segments
        .iter()
        .copied()
        .filter(|segment| !is_setup_segment(segment))
        .collect();
    let target_segment = meaningful_segments
        .last()
        .copied()
        .or_else(|| segments.last().copied())
        .unwrap_or(trimmed);

    let mut findings = Vec::new();

    if meaningful_segments.len() <= 1 && is_always_pass_command(target_segment) {
        findings.push(VerifyLintResult::error(
            "Verify command always exits successfully (`true`, `echo ...`, or `exit 0`). Replace it with a focused check that can fail.",
        ));
    }

    if is_bare_cargo_test(target_segment) {
        findings.push(VerifyLintResult::error(
            "Bare `cargo test` runs the entire suite — it can pass without proving this unit. Use a targeted filter like `cargo test auth::login`.",
        ));
    }

    if is_npm_test_without_filter(target_segment) {
        findings.push(VerifyLintResult::error(
            "Bare `npm test` runs the entire suite. Use `npm test -- --grep login` or `npm test -- -t login`.",
        ));
    }

    if is_pytest_without_filter(target_segment) {
        findings.push(VerifyLintResult::error(
            "Bare `pytest` runs the entire suite. Use `pytest -k login` and pair it with a grep existence check.",
        ));
    }

    if is_go_test_without_run(target_segment) {
        findings.push(VerifyLintResult::error(
            "Bare `go test` without `-run` runs the entire suite. Use `go test ./... -run TestLogin`.",
        ));
    }

    if cargo_test_filter_arg(target_segment).is_some() && !contains_grep(trimmed) {
        findings.push(VerifyLintResult::warning(
            "`cargo test <filter>` exits 0 when the filter matches no tests. Prepend an existence check like `grep -rq '<filter>' tests && cargo test <filter>`.",
        ));
    }

    if has_pytest_k_filter(target_segment) && !contains_grep(trimmed) {
        findings.push(VerifyLintResult::warning(
            "`pytest -k <filter>` exits 0 when the filter matches no tests. Prepend an existence check like `grep -rq 'test_login' tests && pytest -k login`.",
        ));
    }

    if is_existence_only_check(trimmed) {
        findings.push(VerifyLintResult::warning(
            "A file existence check does not verify correctness. Chain it with a real assertion, for example `test -f file && grep -q 'expected text' file`.",
        ));
    }

    findings
}

fn shell_segment_splitter() -> &'static Regex {
    static SPLITTER: OnceLock<Regex> = OnceLock::new();
    SPLITTER
        .get_or_init(|| Regex::new(r"\s*(?:&&|\|\||;|\n)\s*").expect("valid shell splitter regex"))
}

fn is_setup_segment(segment: &str) -> bool {
    let trimmed = segment.trim();
    trimmed == "cd" || trimmed.starts_with("cd ")
}

fn is_always_pass_command(segment: &str) -> bool {
    let trimmed = segment.trim();
    trimmed == "true" || trimmed == "exit 0" || trimmed == "echo" || trimmed.starts_with("echo ")
}

fn tokens(segment: &str) -> Vec<&str> {
    segment.split_whitespace().collect()
}

fn is_bare_cargo_test(segment: &str) -> bool {
    matches!(tokens(segment).as_slice(), ["cargo", "test"])
}

fn is_npm_test_without_filter(segment: &str) -> bool {
    let tokens = tokens(segment);
    tokens.len() >= 2
        && tokens[0] == "npm"
        && tokens[1] == "test"
        && !segment.contains("-- --grep")
        && !segment.contains("-- -t")
}

fn is_pytest_without_filter(segment: &str) -> bool {
    let tokens = tokens(segment);
    tokens.first() == Some(&"pytest") && !has_pytest_k_filter(segment)
}

fn is_go_test_without_run(segment: &str) -> bool {
    let tokens = tokens(segment);
    tokens.len() >= 2
        && tokens[0] == "go"
        && tokens[1] == "test"
        && !tokens
            .iter()
            .any(|token| *token == "-run" || token.starts_with("-run="))
}

fn cargo_test_filter_arg(segment: &str) -> Option<&str> {
    let tokens = tokens(segment);
    if tokens.first() != Some(&"cargo") || tokens.get(1) != Some(&"test") {
        return None;
    }

    let mut skip_next = false;
    for token in tokens.iter().skip(2) {
        if skip_next {
            skip_next = false;
            continue;
        }

        if *token == "--" {
            break;
        }

        if takes_cargo_test_value(token) {
            skip_next = true;
            continue;
        }

        if has_inline_cargo_test_value(token) || token.starts_with('-') {
            continue;
        }

        return Some(token);
    }

    None
}

fn takes_cargo_test_value(token: &str) -> bool {
    matches!(
        token,
        "-p" | "--package"
            | "--manifest-path"
            | "--message-format"
            | "--target"
            | "--target-dir"
            | "--color"
            | "-j"
            | "--jobs"
            | "--profile"
            | "--config"
            | "--test"
            | "--bench"
            | "--example"
            | "--bin"
            | "--features"
    )
}

fn has_inline_cargo_test_value(token: &str) -> bool {
    [
        "--package=",
        "--manifest-path=",
        "--message-format=",
        "--target=",
        "--target-dir=",
        "--color=",
        "--jobs=",
        "--profile=",
        "--config=",
        "--test=",
        "--bench=",
        "--example=",
        "--bin=",
        "--features=",
    ]
    .iter()
    .any(|prefix| token.starts_with(prefix))
}

fn has_pytest_k_filter(segment: &str) -> bool {
    let tokens = tokens(segment);
    tokens.windows(2).any(|window| matches!(window, ["-k", _]))
        || tokens.iter().any(|token| token.starts_with("-k="))
}

fn contains_grep(command: &str) -> bool {
    command.split_whitespace().any(|token| token == "grep")
}

fn is_existence_only_check(command: &str) -> bool {
    let tokens = tokens(command);
    matches!(tokens.as_slice(), ["test", "-f", _] | ["[", "-f", _, "]"])
}

#[cfg(test)]
mod tests {
    use super::{lint_verify, VerifyLintLevel};

    #[test]
    fn verify_lint_rejects_empty_commands() {
        let findings = lint_verify("   ");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].level, VerifyLintLevel::Error);
    }

    #[test]
    fn verify_lint_rejects_always_pass_commands() {
        let findings = lint_verify("cd mana && exit 0");
        assert!(findings.iter().any(|finding| {
            finding.level == VerifyLintLevel::Error && finding.message.contains("always exits")
        }));
    }

    #[test]
    fn verify_lint_rejects_bare_test_runners() {
        let cargo = lint_verify("cargo test");
        assert!(cargo.iter().any(|finding| {
            finding.level == VerifyLintLevel::Error && finding.message.contains("cargo test")
        }));

        let pytest = lint_verify("pytest");
        assert!(pytest.iter().any(|finding| {
            finding.level == VerifyLintLevel::Error && finding.message.contains("pytest")
        }));

        let go = lint_verify("go test ./...");
        assert!(go.iter().any(|finding| {
            finding.level == VerifyLintLevel::Error && finding.message.contains("go test")
        }));
    }

    #[test]
    fn verify_lint_accepts_targeted_test_commands() {
        // Targeted commands should not produce errors
        let cargo = lint_verify("cargo test auth::login");
        assert!(!cargo.iter().any(|f| f.level == VerifyLintLevel::Error));

        let cargo_p = lint_verify("cargo test -p mana-core verify_lint");
        assert!(!cargo_p.iter().any(|f| f.level == VerifyLintLevel::Error));

        let pytest = lint_verify("pytest -k test_login");
        assert!(!pytest.iter().any(|f| f.level == VerifyLintLevel::Error));

        let go = lint_verify("go test ./... -run TestLogin");
        assert!(!go.iter().any(|f| f.level == VerifyLintLevel::Error));
    }

    #[test]
    fn verify_lint_warns_on_filtered_commands_without_grep() {
        let cargo = lint_verify("cargo test create::tests::lint");
        assert!(cargo.iter().any(|finding| {
            finding.level == VerifyLintLevel::Warning
                && finding.message.contains("matches no tests")
        }));

        let pytest = lint_verify("pytest -k login");
        assert!(pytest.iter().any(|finding| {
            finding.level == VerifyLintLevel::Warning
                && finding.message.contains("matches no tests")
        }));
    }

    #[test]
    fn verify_lint_accepts_filtered_commands_with_grep_guard() {
        let findings = lint_verify("grep -rq 'test_login' tests && pytest -k login");
        assert!(findings.is_empty());
    }

    #[test]
    fn verify_lint_warns_on_existence_only_checks() {
        let findings = lint_verify("test -f README.md");
        assert!(findings.iter().any(|finding| {
            finding.level == VerifyLintLevel::Warning && finding.message.contains("existence check")
        }));
    }
}
