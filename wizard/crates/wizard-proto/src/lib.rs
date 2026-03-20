use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    OpenProject { path: String },
    RunUnit { unit_id: String },
    RetryUnit { unit_id: String },
    StopAgent { agent_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    ProjectLoaded { snapshot: ProjectSnapshot },
    RuntimeUpdated { snapshot: RuntimeSnapshot },
    AgentSpawned { agent_id: String, unit_id: String },
    AgentExited { agent_id: String, exit_code: Option<i32> },
}
