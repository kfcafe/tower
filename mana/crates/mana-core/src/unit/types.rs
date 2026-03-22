use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    InProgress,
    /// Agent has finished work; runner will run the verify command on its behalf.
    AwaitingVerify,
    Closed,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Open => write!(f, "open"),
            Status::InProgress => write!(f, "in_progress"),
            Status::AwaitingVerify => write!(f, "awaiting_verify"),
            Status::Closed => write!(f, "closed"),
        }
    }
}

// ---------------------------------------------------------------------------
// RunResult / RunRecord (verification history)
// ---------------------------------------------------------------------------

/// Outcome of a verification run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
    Pass,
    Fail,
    Timeout,
    Cancelled,
}

/// A single verification run record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub attempt: u32,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub result: RunResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_snippet: Option<String>,
}

// ---------------------------------------------------------------------------
// OnCloseAction
// ---------------------------------------------------------------------------

/// Declarative action to run when a unit's verify command fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum OnFailAction {
    /// Retry with optional max attempts and delay.
    Retry {
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        delay_secs: Option<u64>,
    },
    /// Bump priority and add message.
    Escalate {
        #[serde(skip_serializing_if = "Option::is_none")]
        priority: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

/// Declarative actions to run when a unit is closed.
/// Processed after the unit is archived and post-close hook fires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum OnCloseAction {
    /// Run a shell command in the project root.
    Run { command: String },
    /// Print a notification message.
    Notify { message: String },
}

// ---------------------------------------------------------------------------
// AttemptRecord (for memory system attempt tracking)
// ---------------------------------------------------------------------------

/// Outcome of a claim→close cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptOutcome {
    Success,
    Failed,
    Abandoned,
}

/// A single attempt record (claim→close cycle).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub num: u32,
    pub outcome: AttemptOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serializes_as_lowercase() {
        let open = serde_yml::to_string(&Status::Open).unwrap();
        let in_progress = serde_yml::to_string(&Status::InProgress).unwrap();
        let closed = serde_yml::to_string(&Status::Closed).unwrap();

        assert_eq!(open.trim(), "open");
        assert_eq!(in_progress.trim(), "in_progress");
        assert_eq!(closed.trim(), "closed");
    }

    #[test]
    fn awaiting_verify_serializes_as_snake_case() {
        let yaml = serde_yml::to_string(&Status::AwaitingVerify).unwrap();
        assert_eq!(yaml.trim(), "awaiting_verify");
    }

    #[test]
    fn awaiting_verify_deserializes_from_snake_case() {
        let status: Status = serde_yml::from_str("awaiting_verify").unwrap();
        assert_eq!(status, Status::AwaitingVerify);
    }

    #[test]
    fn awaiting_verify_display() {
        assert_eq!(Status::AwaitingVerify.to_string(), "awaiting_verify");
    }

    #[test]
    fn run_result_serializes_as_snake_case() {
        assert_eq!(
            serde_yml::to_string(&RunResult::Pass).unwrap().trim(),
            "pass"
        );
        assert_eq!(
            serde_yml::to_string(&RunResult::Fail).unwrap().trim(),
            "fail"
        );
        assert_eq!(
            serde_yml::to_string(&RunResult::Timeout).unwrap().trim(),
            "timeout"
        );
        assert_eq!(
            serde_yml::to_string(&RunResult::Cancelled).unwrap().trim(),
            "cancelled"
        );
    }

    #[test]
    fn run_record_minimal_round_trip() {
        let now = Utc::now();
        let record = RunRecord {
            attempt: 1,
            started_at: now,
            finished_at: None,
            duration_secs: None,
            agent: None,
            result: RunResult::Pass,
            exit_code: None,
            tokens: None,
            cost: None,
            output_snippet: None,
        };

        let yaml = serde_yml::to_string(&record).unwrap();
        let restored: RunRecord = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(record, restored);

        // Optional fields should be omitted
        assert!(!yaml.contains("finished_at:"));
        assert!(!yaml.contains("duration_secs:"));
        assert!(!yaml.contains("agent:"));
        assert!(!yaml.contains("exit_code:"));
        assert!(!yaml.contains("tokens:"));
        assert!(!yaml.contains("cost:"));
        assert!(!yaml.contains("output_snippet:"));
    }

    #[test]
    fn run_record_full_round_trip() {
        let now = Utc::now();
        let record = RunRecord {
            attempt: 3,
            started_at: now,
            finished_at: Some(now),
            duration_secs: Some(12.5),
            agent: Some("agent-42".to_string()),
            result: RunResult::Fail,
            exit_code: Some(1),
            tokens: Some(5000),
            cost: Some(0.03),
            output_snippet: Some("FAILED: assertion error".to_string()),
        };

        let yaml = serde_yml::to_string(&record).unwrap();
        let restored: RunRecord = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(record, restored);
    }

    #[test]
    fn history_with_cancelled_result() {
        let now = Utc::now();
        let record = RunRecord {
            attempt: 1,
            started_at: now,
            finished_at: None,
            duration_secs: None,
            agent: None,
            result: RunResult::Cancelled,
            exit_code: None,
            tokens: None,
            cost: None,
            output_snippet: None,
        };

        let yaml = serde_yml::to_string(&record).unwrap();
        assert!(yaml.contains("cancelled"));
        let restored: RunRecord = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(restored.result, RunResult::Cancelled);
    }
}
