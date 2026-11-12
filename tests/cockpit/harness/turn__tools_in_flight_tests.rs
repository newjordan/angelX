use super::*;
use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};

fn call(id: &str) -> TurnEvent {
    TurnEvent::ToolCall {
        id: ToolEventId(id.into()),
        name: "grep".into(),
        args_summary: String::new(),
    }
}

fn result(id: &str) -> TurnEvent {
    TurnEvent::ToolResult {
        id: ToolEventId(id.into()),
        name: "grep".into(),
        summary: String::new(),
        outcome: ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::NotApplicable,
        },
    }
}

#[test]
fn a_lone_call_is_in_flight_until_its_result() {
    let mut tools = ToolsInFlight::default();
    assert!(!tools.any());
    tools.note(&call("a"));
    assert!(tools.any());
    tools.note(&TurnEvent::Token("text between".into()));
    assert!(tools.any());
    tools.note(&result("a"));
    assert!(!tools.any());
}

#[test]
fn a_parallel_sibling_finishing_leaves_the_other_in_flight() {
    let mut tools = ToolsInFlight::default();
    tools.note(&call("a"));
    tools.note(&call("b"));
    tools.note(&result("a"));
    assert!(tools.any(), "b is still running");
    tools.note(&result("b"));
    assert!(!tools.any());
}

#[test]
fn a_nested_call_finishing_leaves_its_parent_in_flight() {
    let mut tools = ToolsInFlight::default();
    tools.note(&call("code"));
    tools.note(&call("code:1"));
    tools.note(&result("code:1"));
    assert!(tools.any(), "the code_mode call itself has not returned");
    tools.note(&result("code"));
    assert!(!tools.any());
}

#[test]
fn an_unannounced_result_cannot_owe_the_count() {
    let mut tools = ToolsInFlight::default();
    tools.note(&result("ghost"));
    assert!(!tools.any());
    tools.note(&call("a"));
    assert!(tools.any(), "the stray result left no debt behind");
}
