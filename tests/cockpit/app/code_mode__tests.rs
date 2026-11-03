use super::*;

/// A mock tool host: a few arithmetic/echo "tools" so the runtime can be
/// exercised end-to-end without a ToolRegistry.
fn mock(name: &str, args: &Value) -> Result<String, String> {
    match name {
        // echo: return the args back as compact JSON.
        "echo" => Ok(args.to_string()),
        // double: {n} -> n*2 as a string.
        "double" => {
            let n = args.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
            Ok((n * 2).to_string())
        }
        // boom: always errors (to test catchable throws).
        "boom" => Err("kaboom".to_string()),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn parallel_all(specs: &[(String, Value)]) -> Vec<Range<usize>> {
    if specs.is_empty() {
        Vec::new()
    } else {
        std::iter::once(0..specs.len()).collect()
    }
}

fn parallel_none(specs: &[(String, Value)]) -> Vec<Range<usize>> {
    (0..specs.len()).map(|index| index..index + 1).collect()
}

fn run_ok(script: &str) -> Outcome {
    run(
        script,
        &["echo".into(), "double".into(), "boom".into()],
        &mock,
        &parallel_all,
        Duration::from_secs(5),
        128,
    )
    .expect("run ok")
}

/// Like `mock`, plus a `sleep` tool so concurrency is observable.
fn slow_mock(name: &str, args: &Value) -> Result<String, String> {
    if name == "sleep" {
        let ms = args.get("ms").and_then(|v| v.as_u64()).unwrap_or(0);
        std::thread::sleep(Duration::from_millis(ms));
        return Ok(format!("slept {ms}"));
    }
    mock(name, args)
}

#[test]
fn batch_preserves_order_and_captures_errors() {
    let specs = vec![
        ("double".to_string(), serde_json::json!({ "n": 1 })),
        ("boom".to_string(), serde_json::json!({})),
        ("double".to_string(), serde_json::json!({ "n": 3 })),
    ];
    let out = run_batch(&mock, &specs, 4);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].as_ref().unwrap(), "2");
    assert!(out[1].is_err(), "middle item should carry its own error");
    assert_eq!(out[2].as_ref().unwrap(), "6");
}

#[test]
fn batch_runs_concurrently() {
    // 8 × 50ms run concurrently (cap 16) → well under the 400ms serial sum.
    let specs: Vec<(String, Value)> = (0..8)
        .map(|_| ("sleep".to_string(), serde_json::json!({ "ms": 50 })))
        .collect();
    let start = std::time::Instant::now();
    let out = run_batch(&slow_mock, &specs, 16);
    let elapsed = start.elapsed();
    assert_eq!(out.len(), 8);
    assert!(out.iter().all(|r| r.is_ok()));
    assert!(
        elapsed < Duration::from_millis(250),
        "expected concurrent execution, took {elapsed:?}"
    );
}

#[test]
fn batch_respects_concurrency_cap() {
    // cap 2 over 4 × 50ms → ~2 waves ≈ 100ms+, not the 200ms full-serial.
    let specs: Vec<(String, Value)> = (0..4)
        .map(|_| ("sleep".to_string(), serde_json::json!({ "ms": 50 })))
        .collect();
    let start = std::time::Instant::now();
    let out = run_batch(&slow_mock, &specs, 2);
    let elapsed = start.elapsed();
    assert_eq!(out.len(), 4);
    assert!(
        elapsed >= Duration::from_millis(90),
        "cap 2 should serialize into ~2 waves: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(350),
        "but still overlap within a wave: {elapsed:?}"
    );
}

#[test]
fn scheduled_batch_parallelizes_safe_runs_around_serial_barrier() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let active = [AtomicUsize::new(0), AtomicUsize::new(0)];
    let max_active = [AtomicUsize::new(0), AtomicUsize::new(0)];
    let barrier_seen = AtomicBool::new(false);
    let invoke = |name: &str, args: &Value| {
        if name == "barrier" {
            if active[0].load(Ordering::SeqCst) != 0 {
                return Err("barrier crossed the first safe run".to_string());
            }
            barrier_seen.store(true, Ordering::SeqCst);
            return Ok(name.to_string());
        }
        let phase = args["phase"].as_u64().unwrap() as usize;
        if phase == 1 && !barrier_seen.load(Ordering::SeqCst) {
            return Err("second safe run crossed the barrier".to_string());
        }
        let now = active[phase].fetch_add(1, Ordering::SeqCst) + 1;
        max_active[phase].fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(30));
        active[phase].fetch_sub(1, Ordering::SeqCst);
        Ok(name.to_string())
    };
    let specs = vec![
        ("a".to_string(), serde_json::json!({"phase": 0})),
        ("b".to_string(), serde_json::json!({"phase": 0})),
        ("barrier".to_string(), serde_json::json!({})),
        ("c".to_string(), serde_json::json!({"phase": 1})),
        ("d".to_string(), serde_json::json!({"phase": 1})),
    ];

    let results = run_scheduled_batch(&invoke, &specs, vec![0..2, 2..3, 3..5], 16);

    assert_eq!(
        results
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("all segments"),
        ["a", "b", "barrier", "c", "d"]
    );
    assert_eq!(max_active[0].load(Ordering::SeqCst), 2);
    assert_eq!(max_active[1].load(Ordering::SeqCst), 2);
}

#[test]
fn malformed_batch_schedule_fails_closed_to_declaration_order() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let active = AtomicUsize::new(0);
    let max_active = AtomicUsize::new(0);
    let invoke = |name: &str, _: &Value| {
        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
        max_active.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(10));
        active.fetch_sub(1, Ordering::SeqCst);
        Ok(name.to_string())
    };
    let specs = (0..4)
        .map(|index| (index.to_string(), serde_json::json!({})))
        .collect::<Vec<_>>();

    let results = run_scheduled_batch(&invoke, &specs, vec![0..2, 3..4], 16);

    assert_eq!(
        results
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("fallback serial calls"),
        ["0", "1", "2", "3"]
    );
    assert_eq!(max_active.load(Ordering::SeqCst), 1);
}

#[test]
fn batch_from_js_returns_structured_results() {
    // End-to-end through V8: batch() returns parsed objects, in order, with
    // per-item ok/error, all in one run.
    let out = run_ok(
        "const r = batch([{tool:'double',args:{n:5}},{tool:'boom'},{tool:'double',args:{n:6}}]); \
             return r.map(x => x.ok ? x.output : 'ERR').join(',');",
    );
    assert_eq!(out.result, "10,ERR,12");
}

#[test]
fn batch_scheduler_can_force_effects_serial_in_declaration_order() {
    let started = std::time::Instant::now();
    let out = run(
        "const r=batch([1,2,3,4].map(()=>({tool:'sleep',args:{ms:40}}))); \
             return r.length;",
        &["sleep".into()],
        &slow_mock,
        &parallel_none,
        Duration::from_secs(5),
        128,
    )
    .expect("serial batch");
    assert_eq!(out.result, "4");
    assert!(
        started.elapsed() >= Duration::from_millis(140),
        "scheduler-fenced batch unexpectedly overlapped: {:?}",
        started.elapsed()
    );
}

#[test]
fn batch_rejects_oversized_spec_before_dispatch() {
    // YOLO lifts the construction cap (`usize::MAX`); pin it off so this
    // checks the ordinary 48-item bound, not the inherited operator profile.
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _smart = crate::tests::TestEnvGuard::unset("ANGEL_YOLO_SMART");
    let _cap = crate::tests::TestEnvGuard::unset("ANGEL_CODE_MODE_MAX_BATCH_ITEMS");
    let err = run(
        "return batch(Array.from({length:49},()=>({tool:'double',args:{n:1}})));",
        &["double".into()],
        &mock,
        &parallel_all,
        Duration::from_secs(5),
        128,
    )
    .unwrap_err();
    assert!(
        err.contains("batch has 49 items; limit is 48"),
        "got: {err}"
    );
}

/// Scenario: a realistic "mass-test" program — map inputs to specs, run them
/// concurrently with `batch()` (one failure mixed in), aggregate the
/// successes, return JSON. Exercises loop + map + batch + per-item error +
/// JSON return in a single turn (the marquee code-mode use case).
#[test]
fn mass_test_aggregation_program() {
    let out = run_ok(
        "const inputs = [1,2,3,4,5];\n\
             const specs = inputs.map(n => ({tool:'double', args:{n}}));\n\
             specs.push({tool:'boom'});\n\
             const results = batch(specs);\n\
             let sum = 0, fails = 0;\n\
             for (const r of results) { if (r.ok) sum += parseInt(r.output,10); else fails++; }\n\
             return JSON.stringify({sum, fails, count: results.length});",
    );
    assert_eq!(
        out.result, r#"{"sum":30,"fails":1,"count":6}"#,
        "got: {}",
        out.result
    );
}

#[test]
fn extracted_bridge_preserves_worker_identity_and_error_order() {
    use crate::agent::harness::run_identity::{
        LiveModelScope, LiveTurnScope, live_model, live_turn,
    };
    let _model = LiveModelScope::enter(Some("code-mode-boundary-model".into()));
    let _turn = LiveTurnScope::enter(Some("code-mode-boundary-turn".into()));
    let invoke = |name: &str, _: &Value| {
        assert_eq!(live_model().as_deref(), Some("code-mode-boundary-model"));
        assert_eq!(live_turn().as_deref(), Some("code-mode-boundary-turn"));
        if name == "fail" {
            Err("expected-boundary-error".into())
        } else {
            Ok(name.to_string())
        }
    };
    let out = run(
        "return batch(['first','fail','last'].map(tool=>({tool})));",
        &[],
        &invoke,
        &parallel_all,
        Duration::from_secs(5),
        128,
    )
    .expect("bridge batch");
    assert_eq!(
        serde_json::from_str::<Value>(&out.result).unwrap(),
        serde_json::json!([
            {"ok":true,"output":"first"},
            {"ok":false,"error":"expected-boundary-error"},
            {"ok":true,"output":"last"},
        ])
    );
    assert_eq!(live_model().as_deref(), Some("code-mode-boundary-model"));
    assert_eq!(live_turn().as_deref(), Some("code-mode-boundary-turn"));
}
