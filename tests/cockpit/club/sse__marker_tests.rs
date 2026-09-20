use super::*;

fn chunk(content: &str) -> serde_json::Value {
    serde_json::json!({ "choices": [{ "delta": { "content": content } }] })
}

fn split_feed(pieces: &[&str]) -> (String, String) {
    let mut splitter = MarkerSplitter::default();
    let mut reasoning = String::new();
    let mut content = String::new();
    for piece in pieces {
        let sp = splitter.feed(piece);
        reasoning.push_str(&sp.reasoning);
        content.push_str(&sp.content);
    }
    let tail = splitter.finish();
    reasoning.push_str(&tail.reasoning);
    content.push_str(&tail.content);
    (reasoning, content)
}

#[test]
fn full_markers_in_one_chunk_split() {
    let (r, c) = split_feed(&["x<think>why</think>answer"]);
    assert_eq!(r, "why");
    assert_eq!(c, "xanswer");
}

#[test]
fn markers_across_chunk_boundaries() {
    // Every marker is cut mid-tag at a chunk boundary.
    let (r, c) = split_feed(&["pre<thi", "nk>why</thi", "nk>post"]);
    assert_eq!(r, "why");
    assert_eq!(c, "prepost");
}

#[test]
fn no_markers_pass_through_verbatim() {
    let (r, c) = split_feed(&["plain answer", " with no tags"]);
    assert_eq!(r, "");
    assert_eq!(c, "plain answer with no tags");
}

#[test]
fn unicode_content_and_reasoning_are_chunk_boundary_safe() {
    let (r, c) = split_feed(&[
        "plain — 界 🧭 ",
        "<thi",
        "nk>reasoning — 思考 🧠</thi",
        "nk>answer — 完了 ✅",
    ]);
    assert_eq!(r, "reasoning — 思考 🧠");
    assert_eq!(c, "plain — 界 🧭 answer — 完了 ✅");
}

#[test]
fn unicode_before_a_partial_close_marker_is_safe() {
    let (r, c) = split_feed(&["<think>思考 🧠</th", "ink>答え ✅"]);
    assert_eq!(r, "思考 🧠");
    assert_eq!(c, "答え ✅");
}

#[test]
fn unclosed_think_ends_as_reasoning_never_content() {
    let (r, c) = split_feed(&["<think>partial reasoning cut off"]);
    assert_eq!(r, "partial reasoning cut off");
    assert_eq!(c, "");
}

#[test]
fn lenient_first_close_without_open_tag() {
    // Ollama/llama.cpp style: the open tag lives in the prefill, so the
    // first marker seen is the close tag.
    let (r, c) = split_feed(&["step by step reasoning</think>final answer"]);
    assert_eq!(r, "step by step reasoning");
    assert_eq!(c, "final answer");
}

#[test]
fn response_wrapper_tags_are_stripped() {
    let (r, c) = split_feed(&["<think>why</think>\n<response>answer</response>"]);
    assert_eq!(r, "why");
    assert_eq!(c, "answer");
}

#[test]
fn stray_close_after_answer_stays_verbatim() {
    let (r, c) = split_feed(&["<think>a</think>answer b</think> tail"]);
    assert_eq!(r, "a");
    assert_eq!(c, "answer b</think> tail");
}

#[test]
fn multiple_think_cycles_accumulate() {
    let (r, c) = split_feed(&["<think>a</think>x<think>b</think>y"]);
    assert_eq!(r, "ab");
    assert_eq!(c, "xy");
}

#[test]
fn partial_marker_tail_is_held_then_flushed_as_content() {
    let (r, c) = split_feed(&["abc</th", "unknowable"]);
    assert_eq!(r, "");
    assert_eq!(c, "abc</thunknowable");
}

#[test]
fn split_marker_text_handles_complete_body() {
    let sp = split_marker_text("<think>why</think>\n\nanswer");
    assert_eq!(sp.reasoning, "why");
    assert_eq!(sp.content, "answer");
}

#[test]
fn protocol_field_locks_out_marker_splitting() {
    let mut acc = StreamAccumulator::default();
    let _ = acc.apply_chunk(&serde_json::json!({
        "choices": [{ "delta": { "reasoning_content": "proto reason" } }]
    }));
    let d = acc.apply_chunk(&serde_json::json!({
        "choices": [{ "delta": { "content": "<think>not a marker</think>real" } }]
    }));
    assert_eq!(
        d.content.as_deref(),
        Some("<think>not a marker</think>real")
    );
    let reasoning = acc.take_reasoning();
    assert_eq!(reasoning, "proto reason");
    assert_eq!(acc.content, "<think>not a marker</think>real");
}

#[test]
fn vllm_reasoning_streams_without_entering_the_answer() {
    for shape in ["delta", "message"] {
        let mut acc = StreamAccumulator::default();
        let first = acc.apply_chunk(&serde_json::json!({
            "choices": [{shape: {"reasoning": "Compute 思考 "}}]
        }));
        assert_eq!(first.reasoning.as_deref(), Some("Compute 思考 "));
        assert_eq!(first.content, None);
        let second = acc.apply_chunk(&serde_json::json!({
            "choices": [{shape: {"reasoning": "17 × 23", "content": "391"}}]
        }));
        assert_eq!(second.reasoning.as_deref(), Some("17 × 23"));
        assert_eq!(second.content.as_deref(), Some("391"));
        assert_eq!(acc.take_reasoning(), "Compute 思考 17 × 23");
        match acc.into_reply(false) {
            ClubReply::Text(text) => assert_eq!(text, "391"),
            other => panic!("expected an answer, got {other:?}"),
        }
    }
}

#[test]
fn zai_openrouter_reasoning_details_and_object_never_enter_answer_or_tools() {
    let mut acc = StreamAccumulator::default();
    let d1 = acc.apply_chunk(&serde_json::json!({
        "choices": [{ "delta": { "reasoning_details": [
            {"type": "reasoning.text", "text": "think "},
            {"type": "reasoning.text", "content": "first"}
        ] } }]
    }));
    assert_eq!(d1.reasoning.as_deref(), Some("think first"));
    assert_eq!(d1.content, None);
    let d2 = acc.apply_chunk(&serde_json::json!({
        "choices": [{ "delta": { "reasoning": {"text": " more"} } }]
    }));
    assert_eq!(d2.reasoning.as_deref(), Some(" more"));
    let d3 = acc.apply_chunk(&serde_json::json!({
        "choices": [{ "delta": {
            "content": "answer",
            "tool_calls": [{ "index": 0, "id": "c1", "function": { "name": "read_file", "arguments": "{\"path\":\"x\"}" } }]
        } }]
    }));
    assert_eq!(d3.content.as_deref(), Some("answer"));
    assert_eq!(d3.reasoning, None);
    assert_eq!(acc.take_reasoning(), "think first more");
    match acc.into_reply(true) {
        ClubReply::Calls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "read_file");
            assert_eq!(calls[0].args["path"], "x");
            assert!(!format!("{:?}", calls[0].args).contains("think"));
        }
        other => panic!("expected calls, got {other:?}"),
    }
}

#[test]
fn vllm_reasoning_aliases_do_not_duplicate_or_split_literal_answer_tags() {
    for fields in [
        serde_json::json!({"reasoning_content": "working", "reasoning": "working"}),
        serde_json::json!({"reasoning_content": "", "reasoning": "working"}),
        serde_json::json!({"reasoning_content": null, "reasoning": "working"}),
        serde_json::json!({"reasoning_content": {}, "reasoning": "working"}),
        serde_json::json!({"reasoning_content": "working", "reasoning": "other"}),
    ] {
        let mut delta = fields;
        delta["content"] = serde_json::json!("<think>literal documentation</think>");
        let mut acc = StreamAccumulator::default();
        let observed = acc.apply_chunk(&serde_json::json!({"choices": [{"delta": delta}]}));
        assert_eq!(observed.reasoning.as_deref(), Some("working"));
        assert_eq!(
            observed.content.as_deref(),
            Some("<think>literal documentation</think>")
        );
        assert_eq!(acc.take_reasoning(), "working");
    }
}

#[test]
fn accumulator_splits_raw_markers_and_surfaces_live_deltas() {
    let _lock = crate::tests::env_lock();
    {
        let _var = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_MARKERS");
        resync_marker_splitting_from_env();
        let mut acc = StreamAccumulator::default();
        let d1 = acc.apply_chunk(&chunk("<think>why"));
        assert_eq!(d1.content, None);
        assert_eq!(d1.reasoning.as_deref(), Some("why"));
        let d2 = acc.apply_chunk(&chunk("</think>answer"));
        assert_eq!(d2.reasoning, None);
        assert_eq!(d2.content.as_deref(), Some("answer"));
        assert_eq!(acc.take_reasoning(), "why");
        match acc.into_reply(false) {
            ClubReply::Text(t) => assert_eq!(t, "answer"),
            other => panic!("expected text reply, got {other:?}"),
        }
    }
    resync_marker_splitting_from_env();
}

#[test]
fn kill_switch_disables_marker_splitting() {
    let mut acc = StreamAccumulator {
        markers_on: Some(false),
        ..StreamAccumulator::default()
    };
    let _ = acc.apply_chunk(&chunk("<think>why</think>answer"));
    assert_eq!(acc.take_reasoning(), "");
    assert_eq!(acc.content, "<think>why</think>answer");
}

#[test]
fn marker_kill_switch_values_are_parsed_without_global_env_mutation() {
    for value in ["0", "false", "NO", " off "] {
        assert!(!marker_splitting_enabled_from(Some(value)), "{value}");
    }
    for value in ["", "1", "true", "unexpected"] {
        assert!(marker_splitting_enabled_from(Some(value)), "{value}");
    }
    assert!(marker_splitting_enabled_from(None));
}

#[test]
fn marker_splitting_defaults_on() {
    let _lock = crate::tests::env_lock();
    {
        let _var = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_MARKERS");
        resync_marker_splitting_from_env();
        assert!(marker_splitting_enabled());
        let mut acc = StreamAccumulator::default();
        let _ = acc.apply_chunk(&chunk("<think>why</think>answer"));
        assert_eq!(acc.take_reasoning(), "why");
        assert_eq!(acc.content, "answer");
    }
    resync_marker_splitting_from_env();
}

#[test]
fn marker_splitting_env_zero_disables() {
    let _lock = crate::tests::env_lock();
    {
        let _on = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_MARKERS");
        resync_marker_splitting_from_env();
        assert!(marker_splitting_enabled());
        let _off = crate::tests::TestEnvGuard::set("ANGEL_REASONING_MARKERS", "0");
        assert!(
            marker_splitting_enabled(),
            "cache must not re-read env until seed reset"
        );
        set_marker_splitting_enabled(false);
        assert!(!marker_splitting_enabled());
        resync_marker_splitting_from_env();
        assert!(!marker_splitting_enabled());
        let mut acc = StreamAccumulator::default();
        let _ = acc.apply_chunk(&chunk("<think>why</think>answer"));
        assert_eq!(acc.take_reasoning(), "");
        assert_eq!(acc.content, "<think>why</think>answer");
    }
    resync_marker_splitting_from_env();
}

#[test]
fn response_tag_split_across_chunks() {
    let (r, c) = split_feed(&["<think>r</think><respo", "nse>answer"]);
    assert_eq!(r, "r");
    assert_eq!(c, "answer");
}
