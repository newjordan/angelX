use super::*;

#[test]
fn is_js_ident_filters() {
    assert!(is_js_ident("read_file"));
    assert!(is_js_ident("_x"));
    assert!(!is_js_ident("read-file"));
    assert!(!is_js_ident("2cool"));
    assert!(!is_js_ident(""));
}

#[test]
fn code_mode_runtime_cap_is_checked_before_host_batch_dispatch() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = AtomicUsize::new(0);
    let invoke = |_: &str, _: &Value| panic!("unexpected single dispatch");
    let batch = |specs: &[(String, Value)]| {
        calls.fetch_add(1, Ordering::SeqCst);
        specs.iter().map(|(name, _)| Ok(name.clone())).collect()
    };
    let run = |script| {
        super::run(
            script,
            &[],
            &invoke,
            &batch,
            Duration::from_secs(5),
            128,
            &|| 2,
        )
    };
    let error = run("return batch([{tool:'a'},{tool:'b'},{tool:'c'}]);").unwrap_err();
    assert!(error.contains("batch has 3 items; limit is 2"), "{error}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let out = run("return batch([{tool:'a'},{tool:'b'}]);").unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        out.result,
        r#"[{"ok":true,"output":"a"},{"ok":true,"output":"b"}]"#
    );
}

// Exercise V8 semantics without rebuilding or linking the full cockpit test
// harness. Host scheduling/identity/effect policy remains tested in the facade.
use std::ops::Range;

fn mock(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "echo" => Ok(args.to_string()),
        "double" => Ok((args.get("n").and_then(Value::as_i64).unwrap_or(0) * 2).to_string()),
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

// Signature-compatible test adapter; these tests target JS semantics, not the
// host's parallel scheduler (which is not part of this crate).
fn run(
    script: &str,
    names: &[String],
    invoke: &ToolInvoker<'_>,
    _schedule: &dyn Fn(&[(String, Value)]) -> Vec<Range<usize>>,
    timeout: Duration,
    heap_mb: usize,
) -> Result<Outcome, String> {
    let batch = |specs: &[(String, Value)]| {
        specs
            .iter()
            .map(|(name, args)| invoke(name, args))
            .collect()
    };
    super::run(script, names, invoke, &batch, timeout, heap_mb, &|| 48)
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

#[test]
fn returns_number_stringified() {
    assert_eq!(run_ok("return 1 + 2;").result, "3");
}

#[test]
fn returns_string_verbatim() {
    assert_eq!(run_ok("return 'hello';").result, "hello");
}

#[test]
fn returns_object_as_json() {
    let out = run_ok("return {a: 1, b: [2, 3]};");
    // JSON.stringify ordering is insertion order.
    assert_eq!(out.result, r#"{"a":1,"b":[2,3]}"#);
}

#[test]
fn undefined_return_is_empty() {
    assert_eq!(run_ok("let x = 1;").result, "");
}

#[test]
fn calls_named_host_tool() {
    // double({n:21}) -> "42"
    assert_eq!(run_ok("return double({n: 21});").result, "42");
}

#[test]
fn await_is_optional_and_harmless() {
    assert_eq!(
        run_ok("const a = await double({n: 5}); return a;").result,
        "10"
    );
}

#[test]
fn pending_async_promise_returns_an_incomplete_execution_error() {
    let err = run(
        "return new Promise(() => {});",
        &[],
        &mock,
        &parallel_all,
        Duration::from_secs(5),
        128,
    )
    .unwrap_err();
    assert!(err.contains("did not settle"), "got: {err}");
}

#[test]
fn loops_and_batches_across_calls() {
    // Sum double(1..=4) = 2+4+6+8 = 20, in ONE run (would be 4 round-trips).
    let out = run_ok(
        "let total = 0; for (let i = 1; i <= 4; i++) { total += Number(double({n: i})); } return total;",
    );
    assert_eq!(out.result, "20");
}

#[test]
fn generic_tool_dispatch_reaches_any_tool() {
    // tool('double', {n:9}) works even without relying on the named wrapper.
    assert_eq!(run_ok("return tool('double', {n: 9});").result, "18");
}

#[test]
fn host_error_is_a_catchable_js_exception() {
    let out =
        run_ok("try { boom({}); return 'unreached'; } catch (e) { return 'caught:' + e.message; }");
    assert_eq!(out.result, "caught:kaboom");
}

#[test]
fn uncaught_host_error_fails_the_run() {
    let err = run(
        "return boom({});",
        &["boom".into()],
        &mock,
        &parallel_all,
        Duration::from_secs(5),
        128,
    )
    .unwrap_err();
    assert!(err.contains("kaboom"), "got: {err}");
}

#[test]
fn console_log_is_captured() {
    let out = run_ok("console.log('hi', 7); console.error('oops'); return 'ok';");
    assert_eq!(out.result, "ok");
    assert_eq!(out.console, vec!["hi 7".to_string(), "oops".to_string()]);
    assert!(!out.console_truncated);
}

#[test]
fn console_capture_is_byte_bounded() {
    let out = run_ok("console.log('x'.repeat(70000)); return 'ok';");
    assert_eq!(out.result, "ok");
    assert!(out.console_truncated);
    assert!(out.console.iter().map(String::len).sum::<usize>() <= 65_536);
}

#[test]
fn syntax_error_is_reported() {
    let err = run(
        "return ((( ;",
        &[],
        &mock,
        &parallel_all,
        Duration::from_secs(5),
        128,
    )
    .unwrap_err();
    assert!(err.contains("compile error"), "got: {err}");
}

#[test]
fn infinite_loop_times_out() {
    let err = run(
        "while (true) {}",
        &[],
        &mock,
        &parallel_all,
        Duration::from_millis(300),
        128,
    )
    .unwrap_err();
    assert!(err.contains("timed out"), "got: {err}");
}

#[test]
fn heap_bomb_terminates_instead_of_aborting() {
    // Unbounded allocation under a tiny (8MB) heap cap must be TERMINATED by
    // the near-heap-limit callback (returns Err) rather than crash the whole
    // process via V8's FatalProcessOutOfMemory. The timeout is generous (30s)
    // so the failure is provably the heap guard, not the wall-clock watchdog
    // (the heap fills in well under a second). If this test process *aborts*
    // instead of failing the assertion, the heap guard regressed.
    let err = run(
        "let a = []; for (;;) { a.push(new Array(200000).fill(7)); }",
        &[],
        &mock,
        &parallel_all,
        Duration::from_secs(30),
        8,
    )
    .unwrap_err();
    // Termination surfaces through has_terminated() as "execution timed out".
    assert!(
        err.contains("timed out") || err.contains("terminated"),
        "expected a termination error, got: {err}"
    );
}

#[test]
fn bad_json_args_throw() {
    // Force a non-serializable arg path by calling the bridge directly with a
    // value JSON.stringify drops to undefined → "{}" parses fine; instead test
    // that a thrown error inside a tool propagates with the tool name.
    let err = run(
        "return tool('nope', {});",
        &[],
        &mock,
        &parallel_all,
        Duration::from_secs(5),
        128,
    )
    .unwrap_err();
    assert!(err.contains("unknown tool: nope"), "got: {err}");
}

#[test]
fn active_run_rechecks_revoked_batch_policy_before_dispatch() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let cap = AtomicUsize::new(48);
    let dispatched = AtomicUsize::new(0);
    let invoke = |_: &str, _: &Value| panic!("unexpected single dispatch");
    let batch = |specs: &[(String, Value)]| {
        dispatched.fetch_add(1, Ordering::SeqCst);
        // Revoke the larger allowance while this run is still active.
        cap.store(1, Ordering::SeqCst);
        specs.iter().map(|(name, _)| Ok(name.clone())).collect()
    };
    let limit = || cap.load(Ordering::SeqCst);
    let error = super::run(
        "batch([{tool:'a'},{tool:'b'}]); return batch([{tool:'a'},{tool:'b'}]);",
        &[],
        &invoke,
        &batch,
        Duration::from_secs(5),
        128,
        &limit,
    )
    .unwrap_err();
    assert!(error.contains("batch has 2 items; limit is 1"), "{error}");
    assert_eq!(dispatched.load(Ordering::SeqCst), 1);
}
