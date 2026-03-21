use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectSnapshot {
    pub project_name: String,
    pub unit_count: usize,
    pub open_unit_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeSnapshot {
    pub running_agents: usize,
    pub queued_units: usize,
}

/// Extended runtime state with detailed process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub agents: HashMap<String, AgentInfo>,
    pub work_queue: Vec<QueuedWork>,
    pub process_metrics: ProcessMetrics,
    pub last_updated: std::time::SystemTime,
}

/// Information about a running agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub unit_id: String,
    pub status: AgentStatus,
    pub started_at: std::time::SystemTime,
    pub last_activity: std::time::SystemTime,
    pub pid: Option<u32>,
    pub memory_usage: Option<u64>, // bytes
    pub cpu_usage: Option<f32>,    // percentage
}

/// Status of an agent process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Starting,
    Running,
    Stopping,
    Failed { error: String },
}

/// Work that is queued for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedWork {
    pub unit_id: String,
    pub priority: WorkPriority,
    pub queued_at: std::time::SystemTime,
    pub estimated_duration: Option<std::time::Duration>,
}

/// Priority levels for queued work
#[derive(Debug, Clone, Serialize, Deserialize, PartialOrd, Ord, PartialEq, Eq)]
pub enum WorkPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// System-level process metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMetrics {
    pub total_memory_usage: u64, // bytes
    pub total_cpu_usage: f32,    // percentage
    pub active_processes: usize,
    pub uptime: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    OpenProject {
        path: String,
    },
    RunUnit {
        unit_id: String,
    },
    RetryUnit {
        unit_id: String,
    },
    StopAgent {
        agent_id: String,
    },
    SubscribeRuntime,
    UnsubscribeRuntime,
    GetRuntimeState,
    /// Review and artifact commands
    RequestReview {
        unit_id: String,
        review_type: ReviewType,
    },
    CompleteReview {
        review_id: String,
        decision: ReviewDecision,
        notes: Option<String>,
    },
    GetArtifacts {
        unit_id: Option<String>,
    },
    GetReviewHistory {
        unit_id: Option<String>,
    },
    VerifyUnit {
        unit_id: String,
    },
}

/// Types of artifacts that can be generated during unit execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    /// Code files generated or modified
    CodeFile { language: String },
    /// Documentation files
    Documentation,
    /// Configuration files
    Config,
    /// Test files
    Test,
    /// Build artifacts
    Build,
    /// Log files or output
    Log,
    /// Screenshots or images
    Image,
    /// Data files (JSON, CSV, etc.)
    Data { format: String },
    /// Other file types
    Other { description: String },
}

/// Types of reviews that can be requested
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewType {
    /// Review code changes
    Code,
    /// Review documentation
    Documentation,
    /// Review test coverage and quality
    Test,
    /// Review architecture or design
    Architecture,
    /// Review performance impact
    Performance,
    /// Review security implications
    Security,
    /// General unit completion review
    Completion,
    /// Review of generated artifacts
    Artifact { artifact_id: String },
}

/// Decisions that can be made during review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewDecision {
    /// Approve the changes/work
    Approve,
    /// Request modifications before approval
    RequestChanges { required_changes: Vec<String> },
    /// Reject the work entirely
    Reject { reason: String },
    /// Mark as needs more information
    NeedsInfo { questions: Vec<String> },
    /// Defer decision to later
    Defer {
        until: Option<std::time::SystemTime>,
    },
}

/// Verification status for unit completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// All verification checks passed
    Passed,
    /// Some checks failed
    Failed,
    /// Verification is still in progress
    InProgress,
    /// Verification was skipped
    Skipped { reason: String },
    /// Verification encountered an error
    Error { message: String },
}

/// Detailed verification results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDetails {
    /// Individual verification checks performed
    pub checks: Vec<VerificationCheck>,
    /// Overall verification summary
    pub summary: String,
    /// Files or artifacts verified
    pub verified_artifacts: Vec<String>,
    /// Any issues found during verification
    pub issues: Vec<VerificationIssue>,
}

/// Individual verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    /// Name of the check
    pub name: String,
    /// Status of this specific check
    pub status: VerificationStatus,
    /// Details about what was checked
    pub description: String,
    /// Time taken to perform this check
    pub duration: std::time::Duration,
    /// Any output or logs from the check
    pub output: Option<String>,
}

/// Issues found during verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationIssue {
    /// Severity of the issue
    pub severity: IssueSeverity,
    /// Description of the issue
    pub message: String,
    /// File path where issue was found (if applicable)
    pub file: Option<PathBuf>,
    /// Line number in file (if applicable)
    pub line: Option<usize>,
    /// Suggested fix or action
    pub suggestion: Option<String>,
}

/// Severity levels for verification issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Artifact metadata and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Unique identifier for the artifact
    pub id: String,
    /// Unit that generated this artifact
    pub unit_id: String,
    /// Type of artifact
    pub artifact_type: ArtifactType,
    /// File path (relative to project root)
    pub path: PathBuf,
    /// Size in bytes
    pub size: u64,
    /// When the artifact was created
    pub created_at: std::time::SystemTime,
    /// Hash or checksum for integrity
    pub checksum: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Whether this artifact has been reviewed
    pub reviewed: bool,
    /// Review status if applicable
    pub review_status: Option<ReviewDecision>,
}

/// Review session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    /// Unique review identifier
    pub id: String,
    /// Unit being reviewed
    pub unit_id: String,
    /// Type of review
    pub review_type: ReviewType,
    /// Current status
    pub status: ReviewStatus,
    /// When review was requested
    pub requested_at: std::time::SystemTime,
    /// When review was completed (if applicable)
    pub completed_at: Option<std::time::SystemTime>,
    /// Final decision (if completed)
    pub decision: Option<ReviewDecision>,
    /// Review notes or comments
    pub notes: Option<String>,
    /// Artifacts being reviewed
    pub artifacts: Vec<String>, // artifact IDs
    /// Review checklist or criteria
    pub checklist: Vec<ReviewChecklistItem>,
}

/// Status of a review session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewStatus {
    /// Review has been requested but not started
    Pending,
    /// Review is currently in progress
    InProgress,
    /// Review has been completed
    Completed,
    /// Review was cancelled
    Cancelled,
    /// Review is blocked waiting for something
    Blocked { reason: String },
}

/// Individual item in a review checklist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewChecklistItem {
    /// Description of what to check
    pub description: String,
    /// Whether this item has been checked
    pub checked: bool,
    /// Notes on this specific item
    pub notes: Option<String>,
    /// Whether this item is required or optional
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    ProjectLoaded {
        snapshot: ProjectSnapshot,
    },
    RuntimeUpdated {
        snapshot: RuntimeSnapshot,
    },
    ProjectRefreshed {
        snapshot: ProjectSnapshot,
    },
    AgentSpawned {
        agent_id: String,
        unit_id: String,
    },
    AgentExited {
        agent_id: String,
        exit_code: Option<i32>,
    },
    /// New detailed runtime state updates
    RuntimeStateChanged {
        state: RuntimeState,
    },
    /// Agent status change notifications
    AgentStatusChanged {
        agent_id: String,
        status: AgentStatus,
        timestamp: std::time::SystemTime,
    },
    /// Work queue changes
    WorkQueued {
        unit_id: String,
        priority: WorkPriority,
        timestamp: std::time::SystemTime,
    },
    WorkDequeued {
        unit_id: String,
        timestamp: std::time::SystemTime,
    },
    /// Review and artifact events
    ArtifactGenerated {
        artifact_id: String,
        unit_id: String,
        artifact_type: ArtifactType,
        path: PathBuf,
        timestamp: std::time::SystemTime,
    },
    ReviewRequested {
        review_id: String,
        unit_id: String,
        review_type: ReviewType,
        timestamp: std::time::SystemTime,
    },
    ReviewCompleted {
        review_id: String,
        decision: ReviewDecision,
        notes: Option<String>,
        timestamp: std::time::SystemTime,
    },
    VerificationResult {
        unit_id: String,
        verification_id: String,
        result: VerificationStatus,
        details: VerificationDetails,
        timestamp: std::time::SystemTime,
    },
}
