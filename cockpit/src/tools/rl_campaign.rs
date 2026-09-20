//! Model-facing access to the same native campaigns driven by `/rl`.

use crate::{club::ToolDef, harness::Tool, rl_ctl::RlState};
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
            description: "Run and inspect native RL campaigns during an active loop. run launches asynchronous isolated coding attempts, verifier measurements, reflection and policy comparison on the current loop model. Defaults to the loop task/verifier; task and verify can select a research subproblem. Continue useful work while it runs; results retains artifacts across iterations and resumes. Only independently audited promotion installs a learned policy; otherwise this is measured exploration. Use real checks, inspect candidates, and submit a verified winner when ready. /rl shows the same campaign to the operator.".into(),
            params: json!({"type":"object", "properties": {
                "action":{"type":"string", "enum":["run","status","results","stop"]},
                "task":{"type":"string", "description":"Optional research objective; defaults to loop task"},
                "verify":{"type":"string", "description":"Actual verifier command; defaults to loop acceptance command"},
                "rounds":{"type":"integer", "minimum":1, "description":"Campaign rounds; default 1"},
                "group":{"type":"integer", "minimum":1, "description":"Samples per training group; default 3"},
                "samples":{"type":"integer", "minimum":2, "description":"Samples per measured comparison case; at least 2, default 2"},
                "verifier_scope":{"type":"array", "items":{"type":"string"}, "description":"Existing verifier-owned paths relative to each case source"},
                "audit":{"type":"array", "items":{"type":"object", "properties":{
                    "task":{"type":"string"}, "verify":{"type":"string"}, "source":{"type":"string", "description":"Existing independently authored held-out source workspace"}
                }, "required":["task","verify","source"], "additionalProperties":false}},
                "run_id":{"type":"string", "description":"results: optional exact campaign id, within this workspace"},
                "limit":{"type":"integer", "minimum":1, "description":"results: number of retained campaigns; default 5"}
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
