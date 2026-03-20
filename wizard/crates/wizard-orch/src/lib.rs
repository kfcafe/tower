use wizard_proto::{ProjectSnapshot, RuntimeSnapshot};

pub fn project_snapshot(project_name: &str) -> ProjectSnapshot {
    ProjectSnapshot {
        project_name: project_name.to_string(),
        unit_count: 0,
        open_unit_count: 0,
    }
}

pub fn runtime_snapshot() -> RuntimeSnapshot {
    RuntimeSnapshot {
        running_agents: 0,
        queued_units: 0,
    }
}
