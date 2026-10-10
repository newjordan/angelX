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
        let mut campaign_spec = crate::drive::labyrinth::entry::spec_schema();
        campaign_spec["description"] = json!("⠾⠋⠑");
        ToolDef {
            name:self.name().into(),
            description:"Sloptomizer research. context(task) recalls live evidence relationships across models, including outside /loop. During a loop: suggest ranks ideas with Pareto, UCB and MicroLearner without a model call; run tests your chosen idea asynchronously in an isolated copy on the current model/effort; compare=true measures a paired baseline. Completion arrives in the running turn automatically. Advice never forces a choice. rl_campaign handles measured policy promotion. ⠩⠓".into(),
            params:json!({"type":"object","properties":{
                "action":{"type":"string","enum":["options","context","suggest","run","campaign","status","results","stop"]},
                "campaign_action":{"type":"string","enum":["start","run","status","cancel","check","recover","recheck"],"description":"⠾⠋⠉"},
                "campaign_id":{"type":"string","maxLength":80,"description":"⠾⠋⠙"},
                "campaign_spec":campaign_spec,
                "campaign_bundle":{"type":"string","maxLength":512,"description":"⠾⠋⠋"},
                "task":{"type":"string","description":"⠩⠓⠉"},
                "verify":{"type":["string","null"],"description":"⠩⠓⠙"},
                "methods":{"type":"array","items":{"type":"string","enum":["pareto","bandit","memory"]},"description":"⠩⠓⠑"},
                "candidates":{"type":"array","items":{"type":"object","properties":{"idea":{"type":"string"},"approach":{"type":"string"}},"required":["idea"],"additionalProperties":false},"description":"⠩⠓⠃"},
                "seed":{"type":"integer","description":"⠩⠓⠋"},
                "idea":{"type":"string","description":"⠩⠓⠛"},
                "approach":{"type":"string","description":"⠩⠓⠓"},
                "compare":{"type":"boolean","description":"⠩⠓⠊"},
                "use_memory":{"type":"boolean","description":"⠩⠓⠚"},
                "run_id":{"type":"string","description":"⠾⠋⠁"},
                "limit":{"type":"integer","minimum":1,"description":"⠾⠋⠃"}
            },"required":["action"],"additionalProperties":false}),
        }
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
