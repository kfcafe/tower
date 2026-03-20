use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserPanel {
    pub id: String,
    pub url: String,
    pub scope: BrowserPanelScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrowserPanelScope {
    Global,
    Room { room_id: String },
    Unit { unit_id: String },
    Fact { fact_id: String },
}
