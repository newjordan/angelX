use super::*;
use crate::tests::{TestEnvGuard, env_lock};

fn windows(stall_secs: u64, first_token_secs: u64) -> WireWindows {
    WireWindows {
        stall_secs,
        first_token_secs,
        hard_secs: 900,
        stall_source: "table 2026-09-23".into(),
    }
}

fn last_record(dir: &std::path::Path) -> Value {
    let path = dir.join(format!("wire-{}.jsonl", std::process::id()));
    let text = std::fs::read_to_string(&path).expect("a record was appended");
    serde_json::from_str(text.lines().last().expect("one line")).expect("JSON record")
}

/// The heartbeat follows each model's calibrated stall window: a quarter of it,
/// never more often than every 15 s nor less often than every 60 s.
#[test]
fn heartbeat_cadence_is_calibrated_per_model() {
    let _guard = env_lock();
    {
        let _unset = TestEnvGuard::unset("ANGEL_WIRE_HEARTBEAT_SECS");
        assert_eq!(heartbeat_every(45), Duration::from_secs(15));
        assert_eq!(heartbeat_every(120), Duration::from_secs(30));
        assert_eq!(heartbeat_every(240), Duration::from_secs(60));
        assert_eq!(
            heartbeat_every(0),
            Duration::from_secs(60),
            "no stall bound"
        );
    }
    {
        let _off = TestEnvGuard::set("ANGEL_WIRE_HEARTBEAT_SECS", "0");
        assert_eq!(heartbeat_every(240), Duration::ZERO);
    }
    {
        let _set = TestEnvGuard::set("ANGEL_WIRE_HEARTBEAT_SECS", "7");
        assert_eq!(heartbeat_every(240), Duration::from_secs(7));
    }
}

/// One record per call carries the calibrated windows, the wire counters and
/// every dispatched call with its arguments as the model sent them; arguments
/// that were not JSON are noted.
#[test]
fn a_call_record_keeps_windows_counters_and_raw_arguments() {
    let _guard = env_lock();
    let dir = std::env::temp_dir().join(format!("angel-wire-test-{}-record", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _log = TestEnvGuard::set("ANGEL_WIRE_LOG_DIR", dir.to_str().unwrap());
    let _beat = TestEnvGuard::set("ANGEL_WIRE_HEARTBEAT_SECS", "0");

    let wire = WireCall::new("t-club", "t-model", "chat");
    wire.arm(windows(240, 300));
    wire.set_retry_streak(1);
    wire.phase("streaming");
    wire.bytes(64);
    wire.keepalive();
    wire.event("chunk");
    wire.reasoning(7);
    wire.text(5);
    wire.set_finish_reason(Some("tool_calls"));
    wire.finish(&Ok(ClubReply::Calls(vec![ToolCall {
        id: "call_1".into(),
        name: "cargo".into(),
        args: json!({"_raw": "?"}),
    }])));

    let record = last_record(&dir);
    assert_eq!(record["model"], "t-model");
    assert_eq!(record["outcome"], "calls");
    assert_eq!(record["windows"]["stall_secs"], 240);
    assert_eq!(record["windows"]["first_token_secs"], 300);
    assert_eq!(record["windows"]["stall_source"], "table 2026-09-23");
    assert_eq!(record["retry_streak"], 1);
    assert_eq!(record["bytes"], 64);
    assert_eq!(record["keepalives"], 1);
    assert_eq!(record["event_types"]["chunk"], 1);
    assert_eq!(record["text_chars"], 5);
    assert_eq!(record["reasoning_chars"], 7);
    assert!(record["first_token_ms"].is_u64());
    assert_eq!(record["finish_reason"], "tool_calls");
    assert_eq!(record["calls"][0]["name"], "cargo");
    assert_eq!(record["calls"][0]["args"], "?");
    assert_eq!(record["calls"][0]["args_parsed"], false);
    assert!(
        record["notes"][0]
            .as_str()
            .unwrap()
            .starts_with("unparsed_args"),
        "{record}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_call_is_classified_from_its_error() {
    let _guard = env_lock();
    let dir = std::env::temp_dir().join(format!("angel-wire-test-{}-outcome", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _log = TestEnvGuard::set("ANGEL_WIRE_LOG_DIR", dir.to_str().unwrap());
    let _beat = TestEnvGuard::set("ANGEL_WIRE_HEARTBEAT_SECS", "0");
    for (error, outcome) in [
        (
            "stream stalled: server kept the connection alive",
            "stalled",
        ),
        ("stream hard deadline exceeded after 900s", "hard_deadline"),
        (super::super::INCOMPLETE_STREAM_ERR, "incomplete"),
        ("openai responses: HTTP 500", "error"),
    ] {
        WireCall::new("t", "m", "responses").finish(&Err(error.to_string()));
        let record = last_record(&dir);
        assert_eq!(record["outcome"], outcome, "{error}");
        assert_eq!(record["first_token_ms"], Value::Null);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The heartbeat names the phase a long call is stuck in: before the first
/// token it reports the first-token bound, afterwards the gap bound.
#[test]
fn the_heartbeat_names_the_phase_and_its_bound() {
    let wire = WireCall::new("t", "m", "chat");
    *wire.shared.windows.lock().unwrap() = windows(45, 300);
    assert!(
        wire.shared
            .status_line()
            .contains("waiting for response headers")
    );
    wire.phase("streaming");
    wire.keepalive();
    let waiting = wire.shared.status_line();
    assert!(
        waiting.contains("waiting for first token (bound 300s)"),
        "{waiting}"
    );
    assert!(waiting.contains("no bytes yet"), "{waiting}");
    wire.bytes(10);
    wire.text(3);
    let streaming = wire.shared.status_line();
    assert!(
        streaming.contains("streaming, first token at"),
        "{streaming}"
    );
    assert!(streaming.contains("gap bound 45s"), "{streaming}");
    assert!(streaming.contains("text 3"), "{streaming}");
}
