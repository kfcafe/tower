use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Review decision made by a human reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    /// Unit is approved — close it, unblock downstream.
    Approved,
    /// Unit needs changes — keep open, feedback becomes next attempt context.
    ChangesRequested,
    /// Unit is rejected — close it with reason, do not retry.
    Rejected,
}

impl std::fmt::Display for ReviewDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewDecision::Approved => write!(f, "approved"),
            ReviewDecision::ChangesRequested => write!(f, "changes_requested"),
            ReviewDecision::Rejected => write!(f, "rejected"),
        }
    }
}

/// A review annotation on a specific location in a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    /// File path relative to project root.
    pub file: String,
    /// Optional line range (e.g. "42-58"). None = whole file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<String>,
    /// The reviewer's comment.
    pub comment: String,
    /// Optional concrete suggestion for what to change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Severity of this annotation.
    #[serde(default)]
    pub severity: AnnotationSeverity,
}

/// Severity of a review annotation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationSeverity {
    /// Must be addressed before approval.
    #[default]
    Required,
    /// Should be addressed but not blocking.
    Recommended,
    /// Minor nit, informational.
    Minor,
}

/// A complete review record stored in `.mana/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Review {
    /// Unit ID being reviewed.
    pub unit_id: String,
    /// Which attempt this review covers.
    pub attempt: u32,
    /// The decision.
    pub decision: ReviewDecision,
    /// High-level summary from the reviewer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// File-level annotations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    /// When the review was recorded.
    pub reviewed_at: DateTime<Utc>,
    /// Who reviewed (human name or "human").
    #[serde(default = "default_reviewer")]
    pub reviewer: String,
}

fn default_reviewer() -> String {
    "human".to_string()
}

/// Risk level for review triage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Rubber-stampable: config, docs, scaffolding.
    Low,
    /// Normal review: standard completed unit.
    Normal,
    /// Needs careful review: scope creep, many attempts, core logic.
    High,
    /// Immediate attention: test modifications, security-sensitive files, repeated failures.
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "LOW"),
            RiskLevel::Normal => write!(f, "NORMAL"),
            RiskLevel::High => write!(f, "HIGH"),
            RiskLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A risk flag — a specific concern identified by the risk scorer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskFlag {
    /// What kind of risk this is.
    pub kind: RiskFlagKind,
    /// Human-readable description.
    pub message: String,
    /// Relevant file paths, if any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

/// Categories of risk flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskFlagKind {
    /// Agent touched files not mentioned in unit description or paths.
    ScopeCreep,
    /// Agent modified test assertions or expectations.
    TestModified,
    /// Unit took 3+ attempts to complete.
    ManyAttempts,
    /// Unusually large diff for the unit scope.
    LargeDiff,
    /// Files in security-sensitive paths (auth, crypto, payments, etc.).
    SecuritySensitive,
    /// Agent deleted files.
    FilesDeleted,
    /// Verify command was changed by the agent.
    VerifyModified,
}

impl std::fmt::Display for RiskFlagKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskFlagKind::ScopeCreep => write!(f, "scope-creep"),
            RiskFlagKind::TestModified => write!(f, "test-modified"),
            RiskFlagKind::ManyAttempts => write!(f, "many-attempts"),
            RiskFlagKind::LargeDiff => write!(f, "large-diff"),
            RiskFlagKind::SecuritySensitive => write!(f, "security-sensitive"),
            RiskFlagKind::FilesDeleted => write!(f, "files-deleted"),
            RiskFlagKind::VerifyModified => write!(f, "verify-modified"),
        }
    }
}

/// Summary of a file change in the diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    /// File path relative to project root.
    pub path: String,
    /// What happened to this file.
    pub change_type: ChangeType,
    /// Lines added.
    pub additions: u32,
    /// Lines removed.
    pub deletions: u32,
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// A unit ready for review with all context assembled.
#[derive(Debug, Clone)]
pub struct ReviewCandidate {
    /// The unit being reviewed.
    pub unit: mana_core::unit::Unit,
    /// Files changed (from git diff).
    pub file_changes: Vec<FileChange>,
    /// Raw unified diff output.
    pub diff: String,
    /// Computed risk level.
    pub risk_level: RiskLevel,
    /// Specific risk flags.
    pub risk_flags: Vec<RiskFlag>,
    /// Previous reviews for this unit (if any).
    pub prior_reviews: Vec<Review>,
}

/// Entry in the review queue.
#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub unit_id: String,
    pub title: String,
    pub risk_level: RiskLevel,
    pub risk_flags: Vec<RiskFlag>,
    pub attempt: u32,
    pub file_count: usize,
    pub additions: u32,
    pub deletions: u32,
}
