//! Full campaigns use selected native clubs and the same encoded book as Deli.
use crate::agent::{
    club::{Club, ToolDef},
    harness::{Tool, book::labyrinth_campaign as legend},
};
use crate::drive::labyrinth;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

pub(crate) struct LabyrinthCampaignTool {
    workspace: PathBuf,
    club: Option<Arc<dyn Club>>,
}
impl LabyrinthCampaignTool {
    pub(crate) fn new(workspace: PathBuf, club: Option<Arc<dyn Club>>) -> Self {
        Self { workspace, club }
    }
}
impl Tool for LabyrinthCampaignTool {
    fn name(&self) -> &str {
        "labyrinth_campaign"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().into(),
            description: legend::CAMPAIGN.cells(),
            params: json!({
                "type":"object","properties":{
                    "action":{"type":"string","enum":["start","run","status","cancel","check","recover","recheck"]},
                    "id":{"type":"string","maxLength":80},"spec":labyrinth::entry::spec_schema(),
                    "bundle":{"type":"string","maxLength":512}
                },"required":["action"],"additionalProperties":false
            }),
        }
    }
    fn workspace_write_scope_is_opaque(&self, args: &Value) -> bool {
        matches!(
            args["action"].as_str(),
            Some("start" | "run" | "cancel" | "recover" | "recheck")
        )
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }
    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        let quiet = AtomicBool::new(false);
        labyrinth::entry::dispatch(
            &self.workspace,
            args,
            self.club.clone(),
            cancel.unwrap_or(&quiet),
        )
        .map(|value| value.to_string())
    }
}
