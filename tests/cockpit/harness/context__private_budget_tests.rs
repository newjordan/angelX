use super::*;

fn private_call(bytes: usize) -> ChatMsg {
    ChatMsg::assistant_calls_with_reasoning(
        vec![ToolCall {
            id: "inspect".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path": "candidate.rs"}),
        }],
        Some("r".repeat(bytes)),
    )
}

#[test]
fn private_reasoning_counts_toward_context_without_serializing_or_rendering() {
    let plain = private_call(0);
    let private = private_call(48_000);
    assert_eq!(
        estimate_tokens(std::slice::from_ref(&private)),
        estimate_tokens(std::slice::from_ref(&plain)) + 12_000,
    );
    assert_eq!(
        serde_json::to_value(&private).unwrap(),
        serde_json::to_value(&plain).unwrap(),
    );
    assert_eq!(render_transcript(&[private]), render_transcript(&[plain]),);
}

#[test]
fn private_reasoning_resize_invalidates_rolling_context_estimate() {
    let mut history = vec![private_call(4_000)];
    let mut roll = HistoryTokenRoll::default();
    let before = roll.observe(&history);
    history[0].private_reasoning = Some("r".repeat(12_000).into());
    assert_eq!(roll.observe(&history), before + 2_000);
    assert_eq!(roll.observe(&history), estimate_tokens(&history));
    history.push(ChatMsg::tool("inspect", "fn solve() {}"));
    assert_eq!(roll.observe(&history), estimate_tokens(&history));
    history[0].private_reasoning = None;
    assert_eq!(roll.observe(&history), estimate_tokens(&history));
}

#[test]
fn private_reasoning_floor_prevents_futile_tool_evidence_erasure() {
    let mut history = vec![
        private_call(48_000),
        ChatMsg::tool("inspect", "e".repeat(8_000)),
    ];
    let evidence = history[1].content.clone();
    assert_eq!(fit_tool_results_to_budget(&mut history, 10_000, &[]), 0);
    assert_eq!(history[1].content, evidence);
    assert!(context_tokens(&history, &[]) > 10_000);
}
