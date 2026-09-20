use super::*;

fn visible_summary(events: Vec<serde_json::Value>) -> String {
    let mut visible = String::new();
    for event in events {
        if let ResponseEvent::ReasoningSummary(delta) = parse_responses_event(&event.to_string()) {
            visible.push_str(&delta);
        }
    }
    visible
}

fn summary_delta(text: &str) -> serde_json::Value {
    serde_json::json!({"type":"response.reasoning_summary_text.delta","delta":text})
}

fn summary_done(text: &str) -> serde_json::Value {
    serde_json::json!({"type":"response.reasoning_summary_part.done","part":{"type":"summary_text","text":text}})
}

#[test]
fn public_summary_parts_separate_without_splitting_deltas_or_replaying_done_text() {
    let visible = visible_summary(vec![
        serde_json::json!({"type":"response.reasoning_summary_part.added","summary_index":0,"part":{"type":"summary_text","text":""}}),
        summary_delta("**Pre"),
        summary_delta("paring**\n\nChecking inputs."),
        serde_json::json!({"type":"response.reasoning_summary_text.done","text":"**Preparing**\n\nChecking inputs."}),
        summary_done("**Preparing**\n\nChecking inputs."),
        serde_json::json!({"type":"response.output_item.done","item":{"type":"reasoning","summary":[{"type":"summary_text","text":"**Preparing**\n\nChecking inputs."}]}}),
        summary_delta("**Running**"),
        summary_done("**Running**"),
    ]);
    assert_eq!(
        visible,
        "**Preparing**\n\nChecking inputs.\n\n**Running**\n\n"
    );
}

#[test]
fn summary_boundary_keeps_existing_paragraph_breaks() {
    for suffix in ["", "\n", "\n\n"] {
        let text = format!("completed{suffix}");
        assert_eq!(
            visible_summary(vec![summary_delta(&text), summary_done(&text)]),
            "completed\n\n"
        );
    }
}

#[test]
fn empty_or_unrelated_summary_done_events_do_not_create_visible_content() {
    assert_eq!(
        visible_summary(vec![
            summary_done(""),
            summary_done(" \n"),
            serde_json::json!({"type":"response.reasoning_summary_part.done"}),
            serde_json::json!({"type":"response.reasoning_summary_part.done","part":{"type":"output_text","text":"answer"}}),
            serde_json::json!({"type":"response.reasoning_summary_text.done","text":"completed"}),
        ]),
        ""
    );
}

#[test]
fn parse_responses_event_covers_done_ignore_and_error_fallbacks() {
    // A bare `[DONE]` (data-prefixed or not) is a usage-less completion.
    assert!(matches!(
        parse_responses_event("data: [DONE]"),
        ResponseEvent::Done(None)
    ));
    assert!(matches!(
        parse_responses_event("[DONE]"),
        ResponseEvent::Done(None)
    ));
    // Malformed JSON and a blank line are ignored, never an error.
    assert!(matches!(
        parse_responses_event("data: {not json"),
        ResponseEvent::Ignore
    ));
    assert!(matches!(
        parse_responses_event("   "),
        ResponseEvent::Ignore
    ));
    // A text delta with no `delta` field degrades to Ignore.
    assert!(matches!(
        parse_responses_event(r#"data: {"type":"response.output_text.delta"}"#),
        ResponseEvent::Ignore
    ));
    // The plain `response.reasoning_text.delta` variant also maps to Reasoning.
    assert!(matches!(
        parse_responses_event(r#"data: {"type":"response.reasoning_text.delta","delta":"r"}"#),
        ResponseEvent::Reasoning(r) if r == "r"
    ));
    // An `error`-type frame reads `/error/message`.
    assert!(matches!(
        parse_responses_event(r#"data: {"type":"error","error":{"message":"boom"}}"#),
        ResponseEvent::Failed(m, _) if m == "boom"
    ));
    // A failure with a top-level `message`.
    assert!(matches!(
        parse_responses_event(r#"data: {"type":"response.failed","message":"top"}"#),
        ResponseEvent::Failed(m, _) if m == "top"
    ));
    // A failure with no message at all falls back to the default text.
    assert!(matches!(
        parse_responses_event(r#"data: {"type":"response.failed"}"#),
        ResponseEvent::Failed(m, _) if m == "request failed"
    ));
}
