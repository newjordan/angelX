//! Read-only research navigation; claim edits use ordinary reviewed file tools.
use crate::agent::harness::book::labyrinth_campaign as legend;
use crate::agent::{club::ToolDef, harness::Tool};
use crate::drive::labyrinth;
use serde_json::{Value, json};
use std::path::PathBuf;

pub(crate) struct LabyrinthTool {
    workspace: PathBuf,
}
impl LabyrinthTool {
    pub(crate) fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}
impl Tool for LabyrinthTool {
    fn name(&self) -> &str {
        "labyrinth"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().into(),
            description: legend::MAP.cells(),
            params: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["status","check","frontier","plan","route"]},
                "task":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":24},
                "from":{"type":"string"},"to":{"type":"string"},
                "policy":{"type":"string","enum":["explore","established"]}
            },"required":["action"],"additionalProperties":false}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let Some(map) = labyrinth::load(&self.workspace)? else {
            return Ok(json!({"initialized":false,"enable":"/labyrinth init"}).to_string());
        };
        let result = match args["action"].as_str().unwrap_or("status") {
            "status" | "check" => map.status(),
            "frontier" => map.frontier(
                args["task"].as_str(),
                args["limit"].as_u64().unwrap_or(12).min(24) as usize,
            ),
            "plan" => map.plan(
                args["task"].as_str(),
                args["limit"].as_u64().unwrap_or(6).min(24) as usize,
            ),
            "route" => {
                let explore = match args["policy"].as_str().unwrap_or("explore") {
                    "explore" => true,
                    "established" => false,
                    _ => return Err("policy must be explore or established".into()),
                };
                map.route(
                    args["from"].as_str().ok_or("route needs from")?,
                    args["to"].as_str().ok_or("route needs to")?,
                    explore,
                )?
            }
            _ => return Err("unknown labyrinth action".into()),
        };
        serde_json::to_string(&result).map_err(|error| error.to_string())
    }
}
