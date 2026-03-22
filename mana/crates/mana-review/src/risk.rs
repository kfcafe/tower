//! Heuristic risk scoring for review triage.
//!
//! Assigns a [`RiskLevel`] and [`RiskFlag`]s to a completed unit based on
//! cheap, fast, deterministic signals — no LLM required.

use crate::types::*;
use mana_core::unit::Unit;

/// Security-sensitive path patterns.
const SECURITY_PATTERNS: &[&str] = &[
    "auth",
    "crypto",
    "secret",
    "token",
    "password",
    "credential",
    "payment",
    "billing",
    "session",
    "permission",
    "rbac",
    "acl",
    "oauth",
    "jwt",
    "saml",
    "tls",
    "ssl",
    "cert",
];

/// Test file patterns.
const TEST_PATTERNS: &[&str] = &[
    "test", "tests", "spec", "specs", "_test.", ".test.", ".spec.",
];

/// Large diff threshold (total lines changed).
const LARGE_DIFF_THRESHOLD: u32 = 300;

/// Score a completed unit for review risk.
pub fn score(unit: &Unit, file_changes: &[FileChange]) -> (RiskLevel, Vec<RiskFlag>) {
    let mut flags = Vec::new();

    check_scope_creep(unit, file_changes, &mut flags);
    check_test_modifications(file_changes, &mut flags);
    check_many_attempts(unit, &mut flags);
    check_large_diff(file_changes, &mut flags);
    check_security_sensitive(file_changes, &mut flags);
    check_files_deleted(file_changes, &mut flags);

    let level = flags_to_level(&flags);
    (level, flags)
}

/// Agent touched files not mentioned in the unit's description or paths.
fn check_scope_creep(unit: &Unit, file_changes: &[FileChange], flags: &mut Vec<RiskFlag>) {
    if unit.paths.is_empty() && unit.description.is_none() {
        return; // no scope defined, can't detect creep
    }

    // Build known-good path prefixes from unit.paths
    let known_prefixes: Vec<String> = unit
        .paths
        .iter()
        .filter_map(|p| {
            std::path::Path::new(p)
                .parent()
                .map(|parent| parent.to_string_lossy().to_lowercase())
        })
        .filter(|p| !p.is_empty())
        .collect();

    let desc_lower = format!(
        "{} {}",
        unit.title,
        unit.description.as_deref().unwrap_or("")
    )
    .to_lowercase();

    let out_of_scope: Vec<String> = file_changes
        .iter()
        .filter(|fc| {
            let path_lower = fc.path.to_lowercase();

            // File is directly listed in unit.paths
            if unit.paths.iter().any(|p| p.to_lowercase() == path_lower) {
                return false;
            }

            // File is under a known path prefix (e.g. src/auth/)
            if known_prefixes
                .iter()
                .any(|prefix| path_lower.starts_with(prefix))
            {
                return false;
            }

            // File's directory is mentioned in the description (segment > 3 chars)
            if let Some(parent) = std::path::Path::new(&fc.path).parent() {
                if parent
                    .to_string_lossy()
                    .to_lowercase()
                    .split('/')
                    .any(|seg| seg.len() > 3 && desc_lower.contains(seg))
                {
                    return false;
                }
            }

            true
        })
        .map(|fc| fc.path.clone())
        .collect();

    if !out_of_scope.is_empty() {
        flags.push(RiskFlag {
            kind: RiskFlagKind::ScopeCreep,
            message: format!(
                "{} file(s) changed outside apparent unit scope",
                out_of_scope.len()
            ),
            files: out_of_scope,
        });
    }
}

/// Agent modified test files — might have weakened tests to pass.
fn check_test_modifications(file_changes: &[FileChange], flags: &mut Vec<RiskFlag>) {
    let test_files: Vec<String> = file_changes
        .iter()
        .filter(|fc| {
            fc.change_type == ChangeType::Modified
                && TEST_PATTERNS
                    .iter()
                    .any(|p| fc.path.to_lowercase().contains(p))
        })
        .map(|fc| fc.path.clone())
        .collect();

    if !test_files.is_empty() {
        flags.push(RiskFlag {
            kind: RiskFlagKind::TestModified,
            message: format!(
                "{} test file(s) were modified — verify tests weren't weakened",
                test_files.len()
            ),
            files: test_files,
        });
    }
}

/// Unit took 3+ attempts — something was hard or the agent struggled.
fn check_many_attempts(unit: &Unit, flags: &mut Vec<RiskFlag>) {
    if unit.attempts >= 3 {
        flags.push(RiskFlag {
            kind: RiskFlagKind::ManyAttempts,
            message: format!("Unit took {} attempts to complete", unit.attempts),
            files: vec![],
        });
    }
}

/// Unusually large diff for a single unit.
fn check_large_diff(file_changes: &[FileChange], flags: &mut Vec<RiskFlag>) {
    let total_lines: u32 = file_changes
        .iter()
        .map(|fc| fc.additions + fc.deletions)
        .sum();

    if total_lines > LARGE_DIFF_THRESHOLD {
        flags.push(RiskFlag {
            kind: RiskFlagKind::LargeDiff,
            message: format!(
                "{} total lines changed (threshold: {})",
                total_lines, LARGE_DIFF_THRESHOLD
            ),
            files: vec![],
        });
    }
}

/// Files in security-sensitive paths.
fn check_security_sensitive(file_changes: &[FileChange], flags: &mut Vec<RiskFlag>) {
    let sensitive_files: Vec<String> = file_changes
        .iter()
        .filter(|fc| {
            let path_lower = fc.path.to_lowercase();
            SECURITY_PATTERNS.iter().any(|p| path_lower.contains(p))
        })
        .map(|fc| fc.path.clone())
        .collect();

    if !sensitive_files.is_empty() {
        flags.push(RiskFlag {
            kind: RiskFlagKind::SecuritySensitive,
            message: format!(
                "{} file(s) in security-sensitive paths",
                sensitive_files.len()
            ),
            files: sensitive_files,
        });
    }
}

/// Agent deleted files.
fn check_files_deleted(file_changes: &[FileChange], flags: &mut Vec<RiskFlag>) {
    let deleted: Vec<String> = file_changes
        .iter()
        .filter(|fc| fc.change_type == ChangeType::Deleted)
        .map(|fc| fc.path.clone())
        .collect();

    if !deleted.is_empty() {
        flags.push(RiskFlag {
            kind: RiskFlagKind::FilesDeleted,
            message: format!("{} file(s) deleted", deleted.len()),
            files: deleted,
        });
    }
}

/// Map risk flags to an overall risk level.
fn flags_to_level(flags: &[RiskFlag]) -> RiskLevel {
    if flags.is_empty() {
        return RiskLevel::Low;
    }

    let has_critical = flags.iter().any(|f| {
        matches!(
            f.kind,
            RiskFlagKind::TestModified
                | RiskFlagKind::SecuritySensitive
                | RiskFlagKind::VerifyModified
        )
    });

    if has_critical {
        return RiskLevel::Critical;
    }

    let has_high = flags.iter().any(|f| {
        matches!(
            f.kind,
            RiskFlagKind::ScopeCreep | RiskFlagKind::ManyAttempts | RiskFlagKind::FilesDeleted
        )
    });

    if has_high {
        return RiskLevel::High;
    }

    RiskLevel::Normal
}

#[cfg(test)]
mod tests {
    use super::*;
    use mana_core::unit::Unit;

    fn make_unit(title: &str, attempts: u32) -> Unit {
        let mut unit = Unit::new("1.3", title);
        unit.attempts = attempts;
        unit.description = Some("Implement auth middleware for JWT validation".into());
        unit.paths = vec!["src/auth/middleware.rs".into()];
        unit
    }

    fn make_change(path: &str, change_type: ChangeType, add: u32, del: u32) -> FileChange {
        FileChange {
            path: path.into(),
            change_type,
            additions: add,
            deletions: del,
        }
    }

    #[test]
    fn no_flags_for_clean_unit() {
        let unit = make_unit("Auth middleware", 1);
        let changes = vec![make_change(
            "src/auth/middleware.rs",
            ChangeType::Modified,
            30,
            5,
        )];
        let (_, flags) = score(&unit, &changes);
        // auth/ is security-sensitive, so it gets flagged — that's correct
        // but scope is clean (file matches unit paths)
        assert!(!flags.iter().any(|f| f.kind == RiskFlagKind::ScopeCreep));
        assert!(flags
            .iter()
            .any(|f| f.kind == RiskFlagKind::SecuritySensitive));
    }

    #[test]
    fn scope_creep_detected() {
        let mut unit = make_unit("Auth middleware", 1);
        unit.paths = vec!["src/auth/middleware.rs".into()];
        unit.description = Some("Implement auth middleware".into());
        let changes = vec![
            make_change("src/auth/middleware.rs", ChangeType::Modified, 30, 5),
            make_change("src/unrelated/stuff.rs", ChangeType::Modified, 10, 2),
        ];
        let (level, flags) = score(&unit, &changes);
        assert!(flags.iter().any(|f| f.kind == RiskFlagKind::ScopeCreep));
        assert!(level >= RiskLevel::High);
    }

    #[test]
    fn test_modification_is_critical() {
        let unit = make_unit("Auth middleware", 1);
        let changes = vec![
            make_change("src/auth/middleware.rs", ChangeType::Modified, 30, 5),
            make_change("tests/auth_test.rs", ChangeType::Modified, 5, 3),
        ];
        let (level, flags) = score(&unit, &changes);
        assert!(flags.iter().any(|f| f.kind == RiskFlagKind::TestModified));
        assert_eq!(level, RiskLevel::Critical);
    }

    #[test]
    fn many_attempts_flagged() {
        let unit = make_unit("Auth middleware", 4);
        let changes = vec![make_change(
            "src/auth/middleware.rs",
            ChangeType::Modified,
            30,
            5,
        )];
        let (_, flags) = score(&unit, &changes);
        assert!(flags.iter().any(|f| f.kind == RiskFlagKind::ManyAttempts));
    }

    #[test]
    fn large_diff_flagged() {
        let unit = make_unit("Auth middleware", 1);
        let changes = vec![
            make_change("src/auth/middleware.rs", ChangeType::Added, 200, 0),
            make_change("src/auth/types.rs", ChangeType::Added, 150, 0),
        ];
        let (_, flags) = score(&unit, &changes);
        assert!(flags.iter().any(|f| f.kind == RiskFlagKind::LargeDiff));
    }

    #[test]
    fn security_sensitive_is_critical() {
        let unit = make_unit("Auth middleware", 1);
        let changes = vec![make_change(
            "src/auth/middleware.rs",
            ChangeType::Modified,
            30,
            5,
        )];
        let (level, flags) = score(&unit, &changes);
        assert!(flags
            .iter()
            .any(|f| f.kind == RiskFlagKind::SecuritySensitive));
        assert_eq!(level, RiskLevel::Critical);
    }

    #[test]
    fn deleted_files_flagged() {
        let unit = make_unit("Auth middleware", 1);
        let changes = vec![make_change(
            "src/auth/old_middleware.rs",
            ChangeType::Deleted,
            0,
            50,
        )];
        let (_, flags) = score(&unit, &changes);
        assert!(flags.iter().any(|f| f.kind == RiskFlagKind::FilesDeleted));
    }
}
