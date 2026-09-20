//! Agent-facing goal tool — the model's controlled entry point to the same
//! durable, structured objective store the operator's `/goal` verb drives
//! ([`crate::goal`]). A goal the agent sets is re-injected into subsequent
//! non-casual turns (see `App::goal_context_block`), so the session keeps
//! steering toward it.
//!
//! Deliberately absent: *loop launch*. Starting an autonomous multi-turn run is
//! an operator act (`/loop`, `/self`) with its own budget / escalation guards —
//! a model must not mint its own runaway loop. This tool only persists / reads /
//! clears the objective; the human still owns running a loop against it.

use crate::club::ToolDef;
use crate::harness::Tool;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) struct GoalTool {
    workspace: PathBuf,
}

impl GoalTool {
    pub(crate) fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
        }
    }
}

impl Tool for GoalTool {
    fn name(&self) -> &str {
        "goal"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "goal".to_string(),
            description: "Read or set the durable standing objective for this project (the same \
                store the operator's /goal drives). action=show (default) reads it; action=set \
                replaces it with `text`, plus optional `accept_cmd` (a command whose success \
                means the goal is truly met) and optional `criteria` (acceptance criteria); \
                action=clear removes it. A set goal is re-injected into future turns so the \
                session keeps steering toward it. This does NOT start an autonomous loop — \
                that is the operator's call."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["show", "set", "clear"],
                        "description": "default 'show'"
                    },
                    "text": { "type": "string", "description": "the objective (action=set)" },
                    "accept_cmd": {
                        "type": "string",
                        "description": "optional verifiable command whose success means the goal is met (action=set)"
                    },
                    "criteria": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "optional acceptance criteria (action=set)"
                    }
                },
                "required": []
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        match args["action"].as_str().unwrap_or("show") {
            "show" => match crate::goal::load_for(&self.workspace) {
                Some(goal) => Ok(format_goal(&goal)),
                None => Ok("(no goal set)".to_string()),
            },
            "set" => {
                let text = args["text"]
                    .as_str()
                    .ok_or("missing 'text' for action=set")?
                    .trim();
                if text.is_empty() {
                    return Err("'text' must not be empty".to_string());
                }
                let mut goal = crate::goal::Goal::new(text);
                if let Some(command) = args["accept_cmd"].as_str() {
                    let command = command.trim();
                    if !command.is_empty() {
                        goal.accept_cmd = Some(command.to_string());
                    }
                }
                if let Some(criteria) = args["criteria"].as_array() {
                    for item in criteria {
                        if let Some(criterion) = item.as_str() {
                            let criterion = criterion.trim();
                            if !criterion.is_empty() {
                                goal.acceptance.push(criterion.to_string());
                            }
                        }
                    }
                }
                crate::goal::save_for(&mut goal, &self.workspace)?;
                Ok(format!("goal set → {text}"))
            }
            "clear" => {
                crate::goal::clear_for(&self.workspace)?;
                Ok("goal cleared".to_string())
            }
            other => Err(format!("unknown action {other:?} (use show|set|clear)")),
        }
    }
}

/// Compact, bounded rendering of a goal for the `show` action — the essentials
/// of `App::goal_show` without needing whole-cockpit state.
fn format_goal(goal: &crate::goal::Goal) -> String {
    let mut out = String::new();
    out.push_str(&format!("goal ({:?}): {}\n", goal.status, goal.text.trim()));
    if !goal.acceptance.is_empty() {
        out.push_str("acceptance criteria:\n");
        for criterion in &goal.acceptance {
            out.push_str(&format!("- {criterion}\n"));
        }
    }
    if let Some(command) = &goal.accept_cmd {
        out.push_str(&format!("verifiable check (must pass): {command}\n"));
    }
    if goal.rounds > 0 || goal.max_rounds.is_some() {
        match goal.max_rounds {
            Some(cap) => out.push_str(&format!("progress: round {}/{cap}\n", goal.rounds)),
            None => out.push_str(&format!("progress: round {}\n", goal.rounds)),
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
#[path = "../../../tests/cockpit/tools/goal__tests.rs"]
mod tests;
