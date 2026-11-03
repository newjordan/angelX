//! Model-facing access to the same native campaigns driven by `/rl`.

use crate::agent::{club::ToolDef, harness::Tool};
use crate::drive::rl_ctl::RlState;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub(crate) struct RlCampaignTool {
    workspace: PathBuf,
    state: Arc<Mutex<RlState>>,
}

impl RlCampaignTool {
    pub(crate) fn new(workspace: PathBuf, state: Arc<Mutex<RlState>>) -> Self {
        Self { workspace, state }
    }
}

impl Tool for RlCampaignTool {
    fn name(&self) -> &str {
        "rl_campaign"
    }
    fn def(&self) -> ToolDef {
        crate::agent::harness::core_catalog::baseline_def("rl_campaign")
    }
    fn workspace_write_scope_is_opaque(&self, args: &Value) -> bool {
        args["action"].as_str() == Some("run")
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.state
            .lock()
            .map_err(|_| "RL controller lock poisoned")?
            .tool_call(&self.workspace, args)
    }
}
