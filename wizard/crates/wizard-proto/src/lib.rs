use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub total_memory_usage: u64,    // bytes
    pub total_cpu_usage: f32,       // percentage
    pub active_processes: usize,
    pub uptime: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    OpenProject { path: String },
    RunUnit { unit_id: String },
    RetryUnit { unit_id: String },
    StopAgent { agent_id: String },
    SubscribeRuntime,
    UnsubscribeRuntime,
    GetRuntimeState,
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
}
