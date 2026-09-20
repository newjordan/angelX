//! State shared by the agent-panel control rail and its floating Brain Route
//! deck. Rendering stays in `draw.rs`; input behavior lives in
//! `app_control/agent_menu.rs`.

use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentMenuKind {
    Model,
    Thinking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AgentControlMenu {
    pub(crate) kind: AgentMenuKind,
    pub(crate) selected: usize,
    /// Concrete route whose effort list THINK is inspecting. MODEL keeps this
    /// empty and uses `selected` as its route cursor.
    pub(crate) route_target: Option<(usize, usize)>,
    /// A critically full route armed by the first commit gesture. A matching
    /// second gesture explicitly forces it; navigation clears the arm.
    pub(crate) confirm_target: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentMenuAction {
    ToggleDetails,
    ToggleUnavailable,
    SelectRoute {
        agent_index: usize,
        slot_index: usize,
    },
    InspectEffort {
        agent_index: usize,
        slot_index: usize,
    },
    SelectEffort {
        agent_index: usize,
        slot_index: usize,
        effort: String,
    },
}

pub(crate) type AgentMenuHits = Vec<(Rect, AgentMenuAction)>;

pub(crate) fn connection_label(choice: &crate::agent::club::RouteChoice) -> String {
    if choice.driver == "openai" {
        return "ChatGPT".to_string();
    }
    if choice.agent == choice.driver || choice.agent == "sota" {
        choice.driver.clone()
    } else {
        format!("{} / {}", choice.agent, choice.driver)
    }
}

pub(crate) fn query_matches(query: &str, fields: &[&str]) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let query = query.to_ascii_lowercase();
    fields
        .iter()
        .any(|field| field.to_ascii_lowercase().contains(&query))
}
