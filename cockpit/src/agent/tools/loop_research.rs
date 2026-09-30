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
        ToolDef {
            name:self.name().into(),
            description:"Optional Sloptomizer/AngelX research during a loop. suggest returns original Pareto, UCB bandit and MicroLearner memory advice without a model call. run tests your chosen idea asynchronously in an isolated source copy on the current model/effort; verifier receipts update future advice. compare=true adds a baseline attempt for a real paired delta. Advice never forces a choice or installs a policy. Native rl_campaign handles audited policy promotion; consult_model(method=deli) and spawn(formation=moa) remain separate optional paths. ⠩⠓".into(),
            params:json!({"type":"object","properties":{
                "action":{"type":"string","enum":["options","suggest","run","status","results","stop"]},
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
