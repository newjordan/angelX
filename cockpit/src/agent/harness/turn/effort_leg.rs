//! The reasoning-effort leg. The native per-hop effort is offered to the turn's
//! pinned `effort` policy (`cockpit/live/effort`, else the generation shipped
//! in the binary), which may answer another level this seat accepts.
use super::*;
use crate::agent::harness::live_policy::{self, LegSource};

const HEAD_CHARS: usize = 2000;

pub(super) struct TaskHead {
    head: String,
    len: usize,
}

impl TaskHead {
    pub(super) fn new(task: &str) -> Self {
        Self {
            head: task.chars().take(HEAD_CHARS).collect(),
            len: task.len(),
        }
    }
}

pub(super) struct Hop {
    pub(super) index: usize,
    pub(super) friction: bool,
    pub(super) tool_errors: u64,
    pub(super) failed_checks: usize,
    pub(super) red: bool,
    pub(super) stuck: bool,
}

#[derive(Default)]
pub(super) struct RedSeen(usize);

impl RedSeen {
    pub(super) fn take(&mut self, failed_checks: usize) -> bool {
        let new = failed_checks > self.0;
        self.0 = failed_checks;
        new
    }
}

#[derive(serde::Deserialize)]
struct Choice {
    effort: Option<String>,
}

pub(super) fn decide(
    registry: &ToolRegistry,
    club: &dyn Club,
    task: &TaskHead,
    hop: &Hop,
    native: Option<String>,
) -> (Option<String>, Option<LegSource>) {
    let levels = club.reasoning_levels();
    if levels.is_empty() {
        return (native, None);
    }
    let resolved = native.clone().or_else(|| club.reasoning_effort());
    let input = serde_json::json!({
        "seat": club.model_identity().unwrap_or_else(|| club.label().to_string()),
        "native": resolved,
        "allowed": levels,
        "hop": hop.index,
        "friction": hop.friction,
        "tool_errors": hop.tool_errors,
        "failed_checks": hop.failed_checks,
        "red": hop.red,
        "stuck": hop.stuck,
        "task": task.head,
        "task_len": task.len,
    });
    let (choice, source) = live_policy::decide_pinned(
        registry,
        "effort",
        &input,
        |choice: &Choice| match &choice.effort {
            Some(effort) if Some(effort) != resolved.as_ref() && !levels.contains(effort) => {
                Err(format!("{effort} is not an effort this seat accepts"))
            }
            _ => Ok(()),
        },
    );
    let effort = if choice.effort == resolved {
        native
    } else {
        choice.effort
    };
    (effort, Some(source))
}

pub(super) fn receipt(source: &LegSource) -> (String, Option<String>) {
    let prefix = |digest: &str| digest.chars().take(8).collect::<String>();
    match source {
        LegSource::Native => (source.as_str().to_string(), None),
        LegSource::Live { digest } | LegSource::Shipped { digest } => {
            (format!("{}:{}", source.as_str(), prefix(digest)), None)
        }
        LegSource::LiveFallback { digest, error } => (
            format!("{}:{}", source.as_str(), prefix(digest)),
            Some(error.chars().take(160).collect()),
        ),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/effort_leg__tests.rs"]
mod tests;
