use super::*;

#[test]
fn hostile_tool_call_index_does_not_blow_up_allocation() {
    let mut acc = StreamAccumulator::default();
    let chunk = serde_json::json!({
        "choices": [{ "delta": { "tool_calls": [
            { "index": 100_000_000u64, "id": "x", "function": { "name": "f" } }
        ] } }]
    });
    let _ = acc.apply_chunk(&chunk);
    assert!(
        acc.tool_calls.len() <= 1024,
        "index cap must bound tool_calls growth, got {}",
        acc.tool_calls.len()
    );

    let mut acc = StreamAccumulator::default();
    let chunk = serde_json::json!({
        "choices": [{ "delta": { "tool_calls": [
            { "index": 2u64, "id": "y", "function": { "name": "g" } }
        ] } }]
    });
    let _ = acc.apply_chunk(&chunk);
    assert_eq!(acc.tool_calls.len(), 3);
}
