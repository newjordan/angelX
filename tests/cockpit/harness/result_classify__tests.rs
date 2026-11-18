//! The live `result-classify` leg against the native classifiers it replaced.
//!
//! The host reduces every tool result to flags — the tool-error prefix, the
//! call kind, the exit-code class and the two no-content markers — and the leg
//! combines them. These tests compare the shipped generation with the native
//! oracle on the results a turn really sees, prove a broken generation cannot
//! change an answer, and report the per-decision cost.

use super::*;
use crate::agent::harness::live_policy::{LegSource, decide_with, shipped_leg};

const LEG: &str = "result-classify";

fn call(name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: "result-classify".into(),
        name: name.into(),
        args,
    }
}

fn verifier(name: &str) -> ToolCall {
    call(name, serde_json::json!({}))
}

fn shell(command: &str) -> ToolCall {
    call("shell", serde_json::json!({"command": command}))
}

fn result_cases() -> Vec<(ToolCall, String)> {
    let tests = || verifier("run_tests");
    vec![
        (
            tests(),
            "tool error: run_tests failed (exit 101)\n\nfailures:\n  ---- parser ----".to_string(),
        ),
        (tests(), "tool error: run_tests failed (exit 0)".to_string()),
        (tests(), "tool error: run_tests failed (exit 1)".to_string()),
        (
            tests(),
            "tool error: run_tests failed (exit 126)".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit 127)".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit -9)".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit +5)".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit 007)".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit 12abc)".to_string(),
        ),
        (tests(), "tool error: run_tests failed (exit )".to_string()),
        (
            tests(),
            "tool error: run_tests failed (exit 2147483648)".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit 101) on retry".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit 130)\n[no output on stdout or stderr]".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit 255)\nno such test".to_string(),
        ),
        (
            tests(),
            "tool error: run_tests failed (exit 1)\nsecond line failed (exit 0)".to_string(),
        ),
        (
            tests(),
            "tool error\nfailed (exit 101) on the next line".to_string(),
        ),
        (tests(), "tool error: worker panicked".to_string()),
        (tests(), "tool error: unknown tool: foo".to_string()),
        (tests(), "failed (exit 101) without the prefix".to_string()),
        (tests(), "3 tests failed, 10 passed".to_string()),
        (
            tests(),
            "TOOL ERROR: run_tests failed (exit 101)".to_string(),
        ),
        (
            tests(),
            " tool error: run_tests failed (exit 101)".to_string(),
        ),
        (tests(), String::new()),
        (
            shell("cargo test --lib"),
            "tool error: shell command failed (exit 101)".to_string(),
        ),
        (
            shell("ls -la"),
            "tool error: shell command failed (exit 2)".to_string(),
        ),
        (
            call("cargo", serde_json::json!({"args": "test --offline"})),
            "tool error: cargo failed (exit 101)".to_string(),
        ),
        (
            call("cargo", serde_json::json!({"args": "build"})),
            "tool error: cargo failed (exit 101)".to_string(),
        ),
        (
            call("grep", serde_json::json!({"pattern": "x"})),
            "no files match \"*.zzz\"".to_string(),
        ),
        (
            verifier("read_file"),
            "tool error: read_file failed (exit 1)".to_string(),
        ),
        (
            verifier("write_file"),
            "tool error: write_file failed (exit 2)".to_string(),
        ),
        (
            verifier("apply_patch"),
            "tool error: apply_patch failed (exit 128)".to_string(),
        ),
        (
            call("code_mode", serde_json::json!({})),
            "tool error: code_mode failed (exit 1)".to_string(),
        ),
        (
            verifier("run_tests"),
            "tool error: run_tests failed (exit 101)\n\ntool error: nested".to_string(),
        ),
        (
            verdict_call(),
            "tool error: delegate failed (exit 3)".to_string(),
        ),
    ]
}

fn verdict_call() -> ToolCall {
    call("delegate", serde_json::json!({"task": "check"}))
}

fn reply_cases() -> Vec<&'static str> {
    vec![
        "empty reply (no text and no tool calls)",
        "HTTP 200: empty reply (no text and no tool calls) from muse-spark",
        "club stream produced no data",
        "club stream produced no data after 3 chunks",
        "Empty reply (no text and no tool calls)",
        "empty reply",
        "club stream produced no dat",
        "transport error after 2 attempt(s): connection reset",
        "",
        "provider returned empty stdout",
    ]
}

fn live_answer(
    leg: &angel_live::Program,
    features: &ResultFeatures,
    native: impl FnOnce() -> ResultVerdict,
) -> (ResultVerdict, LegSource) {
    let input = features.input();
    decide_with(
        Some(leg.clone()),
        LEG,
        &input,
        |verdict: &ResultVerdict| verdict.check(features),
        native,
    )
}

#[test]
fn live_result_classify_matches_every_native_verdict() {
    let leg = shipped_leg(LEG);
    let mut checked = 0;
    for (call, result) in result_cases() {
        let features = ResultFeatures::of_call(&call, &result);
        let native = native_of_call(&call, &result);
        let (verdict, source) = live_answer(&leg, &features, || native);
        assert_eq!(verdict, native, "{:?} => {result:?}", call.name);
        assert!(
            matches!(source, LegSource::Live { .. }),
            "{:?} => {result:?} kept the {} leg: {source:?}",
            call.name,
            source.as_str()
        );
        assert_eq!(verdict.error, is_error_result_native(&result));
        assert_eq!(
            verdict.red_verifier,
            is_red_verifier_run_native(&call, &result)
        );
        assert_eq!(
            verdict.dispatch_failure,
            is_dispatch_failure_native(&call, &result)
        );
        let text_features = ResultFeatures::of_result(&result);
        let (text_verdict, text_source) =
            live_answer(&leg, &text_features, || native_of_result(&result));
        assert_eq!(
            text_verdict.error,
            is_error_result_native(&result),
            "{result:?}"
        );
        assert!(matches!(text_source, LegSource::Live { .. }), "{result:?}");
        assert_eq!(text_verdict, native_of_result(&result), "{result:?}");
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} results were compared");
}

#[test]
fn the_shipped_leg_answers_every_production_wrapper() {
    for (call, result) in result_cases() {
        assert_eq!(
            is_error_result(&result),
            is_error_result_native(&result),
            "{result:?}"
        );
        assert_eq!(
            is_red_verifier_run(&call, &result),
            is_red_verifier_run_native(&call, &result),
            "{:?} => {result:?}",
            call.name
        );
        assert_eq!(
            is_dispatch_failure(&call, &result),
            is_dispatch_failure_native(&call, &result),
            "{:?} => {result:?}",
            call.name
        );
    }
    for error in reply_cases() {
        assert_eq!(
            is_empty_reply_error(error),
            is_empty_reply_error_native(error),
            "{error:?}"
        );
    }
}

#[test]
fn live_result_classify_matches_every_native_reply_verdict() {
    let leg = shipped_leg(LEG);
    let mut checked = 0;
    for error in reply_cases() {
        let features = ResultFeatures::of_error_text(error);
        let native = native_of_error_text(error);
        let (verdict, source) = live_answer(&leg, &features, || native);
        assert_eq!(verdict, native, "{error:?}");
        assert_eq!(
            verdict.empty_reply,
            is_empty_reply_error_native(error),
            "{error:?}"
        );
        assert!(
            matches!(source, LegSource::Live { .. }),
            "{error:?} kept the {} leg: {source:?}",
            source.as_str()
        );
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} replies were compared");
}

#[test]
fn a_broken_result_classify_generation_keeps_the_native_verdict() {
    let root = std::env::temp_dir().join(format!(
        "angel-result-classify-leg-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(LEG)).unwrap();
    std::fs::write(
        root.join(LEG).join("module.json"),
        format!(
            r#"{{"api_version":1,"id":"{LEG}","kind":"policy","entry":"index.js","probe":{{"error":false,"verifier":false,"has_exit":false,"exit":0,"empty_marker":false,"club_stream":false}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(
        root.join(LEG).join("index.js"),
        "module.exports={apiVersion:1,render:()=>({error:false,red_verifier:false,dispatch_failure:true,empty_reply:false})}",
    )
    .unwrap();
    let mut registry =
        angel_live::Registry::validated(root.clone(), crate::agent::hotload::evaluate, |_, _| {
            Ok(())
        });
    registry.refresh(true);
    let broken = registry
        .snapshot()
        .policies
        .get(LEG)
        .cloned()
        .expect("the broken generation must still publish");
    let clean = "3 tests failed, 10 passed".to_string();
    let features = ResultFeatures::of_result(&clean);
    let (verdict, source) = live_answer(&broken, &features, || native_of_result(&clean));
    assert_eq!(verdict, native_of_result(&clean));
    assert_eq!(verdict.dispatch_failure, false);
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "the native classifiers must answer: {source:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn live_result_classify_reports_a_median_decision_cost() {
    let leg = shipped_leg(LEG);
    let cases = result_cases();
    let mut samples = Vec::with_capacity(200);
    for index in 0..200 {
        let (call, result) = &cases[index % cases.len()];
        let features = ResultFeatures::of_call(call, result);
        let input = features.input();
        let started = std::time::Instant::now();
        let (verdict, source) = decide_with(
            Some(leg.clone()),
            LEG,
            &input,
            |verdict: &ResultVerdict| verdict.check(&features),
            || native_of_call(call, result),
        );
        let elapsed = started.elapsed().as_secs_f64() * 1e6;
        assert!(matches!(source, LegSource::Live { .. }));
        assert_eq!(verdict, native_of_call(call, result));
        samples.push(elapsed);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!(
        "result-classify leg: {:.1} µs median, {:.1} µs p95 over 200 decisions",
        samples[100], samples[190]
    );
}

/// Three commands with the shape of the ones a debugging agent really issued in a recorded
/// run: an edit through a heredoc, then the test it wants to see, behind `timeout`, a
/// memory-cap wrapper, a pipeline into `grep`, a subshell or a loop. Each came back red.
/// After the third, the turn was told "Every tool call in your last several turns failed"
/// and the agent dropped a design whose values were already right.
fn debugging_commands() -> Vec<String> {
    vec![
        "cd /w/repo/src/lib; python3 - <<'EOF'\ns=open('a.rs').read()\nopen('a.rs','w').write(s.replace('x','y'))\nEOF\n\
         cd /w/repo; (timeout 1200 memcap 2539 cargo test --release --offline t_ -- --nocapture 2>&1 \
         | grep -E \"^error|-->|test |panicked|assert|left|right\" -A3 | head -40)"
            .to_string(),
        "cd /w/repo; cat >> src/lib/tests.rs <<'EOF'\n#[test]\nfn t_add_only() { assert_eq!(1, 1); }\nEOF\n\
         (timeout 1200 memcap 2539 cargo test --release --offline t_add_only 2>&1 \
         | grep -E \"^error|-->|test |panicked\" -A3 | head -30); \
         (timeout 600 cargo test --release --offline t_carry_only 2>&1 | grep -E \"test |panicked\" | head)"
            .to_string(),
        "cd /w/repo; sed -i 's/let m = 1;/let m = 2;/' src/lib/a.rs; python3 - <<'EOF'\nprint('edit')\nEOF\n\
         for c in 256 128 64 16 2; do echo chunk $c; CHUNK=$c timeout 600 cargo test --release --offline \
         t_carry_only 2>&1 | grep -E \"^test |left|right\" | head -4; done"
            .to_string(),
    ]
}

#[test]
fn three_red_tests_in_a_row_do_not_raise_the_every_call_failed_notice() {
    let red = "tool error: shell command failed (exit 101)\ntest lib::tests::t_divide ... FAILED\n\
               assertion `left == right` failed: phase";
    let mut errors = crate::agent::harness::book::x_execution::ErrorStreak::default();
    for command in debugging_commands() {
        let call = shell(&command);
        assert!(is_red_test_shell_run(&call, red), "red test not recognised: {command}");
        let counted = is_dispatch_failure(&call, red) && !is_red_test_shell_run(&call, red);
        assert!(errors.observe(counted).is_none(), "the notice fired on a red test: {command}");
    }
    assert_eq!(errors.streak(), 0);
}

#[test]
fn only_a_test_that_ran_is_kept_out_of_the_failed_call_streak() {
    let wrapped = shell("timeout 600 cargo test --release t_");
    // The wrapper or the test program was not found: the call did not run a test.
    for missing in [
        "tool error: shell command failed (exit 127)\nbash: timeout: command not found",
        "tool error: shell command failed (exit 126)\nbash: ./t: Permission denied",
    ] {
        assert!(!is_red_test_shell_run(&wrapped, missing));
    }
    // A command with no test in it that exits non-zero is still a failed call.
    let plain = shell("cat /w/repo/missing.txt");
    let failed = "tool error: shell command failed (exit 1)\ncat: /w/repo/missing.txt: No such file or directory";
    assert!(!is_red_test_shell_run(&plain, failed));
    assert!(is_dispatch_failure(&plain, failed));
    // A test name inside a heredoc body is text being written, not a test being run.
    let writing = shell("cat > notes.md <<'EOF'\nlater: cargo test t_\nEOF\nfalse");
    assert!(!is_red_test_shell_run(&writing, "tool error: shell command failed (exit 1)"));
    // Other tools are untouched.
    assert!(!is_red_test_shell_run(&verifier("run_tests"), "tool error: run_tests failed (exit 101)"));
    // Wrappers seen in real sessions.
    for command in [
        "CHUNK=2 timeout 600 cargo test --release t_carry_only",
        "(cargo test --release t_)",
        "for c in 2 4; do CHUNK=$c cargo test t_; done",
        "wrapped-cargo test --release t_profile -- --nocapture",
        "timeout 900 python3 -m pytest -x tests/",
        "cd /w/repo && timeout 300 go test ./...",
    ] {
        assert!(
            is_red_test_shell_run(&shell(command), "tool error: shell command failed (exit 1)\nFAILED"),
            "not recognised as a test run: {command}"
        );
    }
}
