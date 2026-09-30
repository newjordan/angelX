//! Model-facing access to the same native campaigns driven by `/rl`.

use crate::agent::{club::ToolDef, harness::Tool};
use crate::drive::rl_ctl::RlState;
use serde_json::{Value, json};
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
        ToolDef {
            name: self.name().into(),
            description: "Run and inspect native RL campaigns during an active loop. run launches asynchronous isolated coding attempts, verifier measurements, reflection and policy comparison on the current loop model. Defaults to the loop task/verifier; task and verify can select a research subproblem. results retains artifacts across iterations and resumes. Only independently audited promotion installs a learned policy; otherwise this is measured exploration. /rl shows the same campaign to the operator. ⠩⠊".into(),
            params: json!({"type":"object", "properties": {
                "action":{"type":"string", "enum":["run","status","results","stop"]},
                "task":{"type":"string", "description":"⠩⠊⠉"},
                "verify":{"type":"string", "description":"⠩⠊⠙"},
                "rounds":{"type":"integer", "minimum":1, "description":"⠩⠊⠑"},
                "group":{"type":"integer", "minimum":1, "description":"⠩⠊⠋"},
                "samples":{"type":"integer", "minimum":2, "description":"⠩⠊⠛"},
                "verifier_scope":{"type":"array", "items":{"type":"string"}, "description":"⠩⠊⠓"},
                "audit":{"type":"array", "items":{"type":"object", "properties":{
                    "task":{"type":"string"}, "verify":{"type":"string"}, "source":{"type":"string", "description":"⠩⠊⠊"}
                }, "required":["task","verify","source"], "additionalProperties":false}},
                "run_id":{"type":"string", "description":"⠩⠊⠚"},
                "limit":{"type":"integer", "minimum":1, "description":"⠾⠛⠁"}
            }, "required":["action"], "additionalProperties":false}),
        }
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
