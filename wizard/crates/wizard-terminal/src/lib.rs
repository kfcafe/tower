use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalSessionKind {
    Room { room_id: String },
    Agent { agent_id: String, unit_id: String },
    Verify { unit_id: String },
    Quick,
}

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("terminal integration is not implemented yet")]
    Unavailable,
}
