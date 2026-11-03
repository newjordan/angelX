//! On-demand original Sloptomizer primitives and isolated experiments.
use crate::agent::{club::ToolDef, harness::Tool};
use crate::drive::rl_ctl::RlState;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

pub(crate) struct LoopResearchTool {
    workspace: PathBuf,
    state: Arc<Mutex<RlState>>,
}
impl LoopResearchTool {
    pub(crate) fn new(workspace: PathBuf, state: Arc<Mutex<RlState>>) -> Self {
        Self { workspace, state }
    }
}
impl Tool for LoopResearchTool {
    fn name(&self) -> &str {
        "loop_research"
    }
    fn def(&self) -> ToolDef {
        let mut definition = crate::agent::harness::core_catalog::baseline_def("loop_research");
        let mut spec = crate::drive::labyrinth::entry::spec_schema();
        spec["description"] = json!("⠾⠋⠑");
        definition.params["properties"]["campaign_spec"] = spec;
        definition
    }
    fn workspace_write_scope_is_opaque(&self, args: &Value) -> bool {
        args["action"] == "run"
            || (args["action"] == "campaign"
                && matches!(
                    args["campaign_action"].as_str(),
                    Some("start" | "run" | "cancel" | "recover" | "recheck")
                ))
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }
    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        let no_cancel = AtomicBool::new(false);
        self.state
            .lock()
            .map_err(|_| "RL controller lock poisoned")?
            .research_call(&self.workspace, args, cancel.unwrap_or(&no_cancel))
    }
}

#[cfg(test)]
pub(crate) fn catalog_current_loop_research() -> ToolDef {
    LoopResearchTool::new(PathBuf::new(), Arc::new(Mutex::new(RlState::default()))).def()
}
