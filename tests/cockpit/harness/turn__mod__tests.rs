use super::*;

#[test]
fn metered_unproductive_default_arms_redirection_and_stop() {
    let _lock = crate::tests::env_lock();
    let _escalate = crate::tests::TestEnvGuard::unset("ANGEL_UNPRODUCTIVE_STREAK_ESCALATE");
    let _stop = crate::tests::TestEnvGuard::unset("ANGEL_UNPRODUCTIVE_STREAK_STOP");
    let _task = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACTIVE");

    let policy = configured_unproductive_policy(true, false);
    assert_eq!(policy.escalate, 8);
    assert_eq!(policy.stop, 16);
}

#[test]
fn metered_unproductive_overrides_and_competition_are_authoritative() {
    let _lock = crate::tests::env_lock();
    let _escalate = crate::tests::TestEnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_ESCALATE", "0");
    let _stop = crate::tests::TestEnvGuard::set("ANGEL_UNPRODUCTIVE_STREAK_STOP", "17");
    let _task = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACTIVE");

    let policy = configured_unproductive_policy(true, false);
    assert_eq!(policy.escalate, 0);
    assert_eq!(policy.stop, 17);

    let competition = configured_unproductive_policy(true, true);
    assert_eq!(competition.escalate, 0);
    assert_eq!(competition.stop, 0);
}

#[test]
fn non_metered_unproductive_defaults_remain_off() {
    let _lock = crate::tests::env_lock();
    let _escalate = crate::tests::TestEnvGuard::unset("ANGEL_UNPRODUCTIVE_STREAK_ESCALATE");
    let _stop = crate::tests::TestEnvGuard::unset("ANGEL_UNPRODUCTIVE_STREAK_STOP");
    let _task = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACTIVE");

    let policy = configured_unproductive_policy(false, false);
    assert_eq!(policy.escalate, 0);
    assert_eq!(policy.stop, 0);
}

#[test]
fn provider_death_acceptance_requires_passed_post_verifier() {
    for result in [None, Some("failed"), Some("not_run"), Some("passed")] {
        let receipt = task_acceptance_snapshot(
            Some("fixture verifier"),
            true,
            Some("passed"),
            1,
            u64::from(result.is_some()),
            1,
            result,
            "provider_error",
            None,
            || None,
        )
        .unwrap();
        assert_eq!(receipt.terminal_passed, result == Some("passed"));
        let json = serde_json::to_value(receipt).unwrap();
        assert_eq!(json["terminal_passed"], result == Some("passed"));
    }
}

#[test]
fn rendered_output_stays_unverified_under_external_only_and_stale_on_revision_mismatch() {
    let _guard = crate::tests::env_lock();
    let prior = std::env::var_os("ANGEL_TASK_RENDERED_REQUIREMENT");
    unsafe { std::env::set_var("ANGEL_TASK_RENDERED_REQUIREMENT", "1") };
    let receipt = task_acceptance_snapshot(
        Some("true"),
        true,
        Some("passed"),
        1,
        1,
        1,
        Some("passed"),
        "accept_cmd",
        None,
        || None,
    )
    .expect("armed snapshot");
    assert!(receipt.terminal_passed);
    assert_eq!(
        receipt.rendered_output.as_ref().map(|r| r.state),
        Some("unverified")
    );
    assert!(!receipt.task_accepted());

    let without_cmd =
        task_acceptance_snapshot(None, false, None, 0, 0, 0, None, "answer", None, || None)
            .expect("rendered requirement emits acceptance without accept_cmd");
    assert_eq!(
        without_cmd.rendered_output.as_ref().map(|r| r.state),
        Some("unverified")
    );
    assert!(!without_cmd.task_accepted());

    let checked = crate::knowledge::cut::sha256_hex(b"checked-workspace");
    let current = crate::knowledge::cut::sha256_hex(b"current-workspace");
    let stale = task_acceptance_snapshot(
        None,
        false,
        None,
        0,
        0,
        0,
        None,
        "answer",
        Some(checked.clone()),
        || Some(current.clone()),
    )
    .expect("stale via production snapshot");
    assert_eq!(
        stale.rendered_output.as_ref().map(|r| r.state),
        Some("stale")
    );
    assert_eq!(
        stale
            .rendered_output
            .as_ref()
            .and_then(|r| r.checked_revision_sha256.as_deref()),
        Some(checked.as_str())
    );
    assert_eq!(
        stale
            .rendered_output
            .as_ref()
            .and_then(|r| r.current_revision_sha256.as_deref()),
        Some(current.as_str())
    );
    assert!(!stale.task_accepted());

    unsafe { std::env::remove_var("ANGEL_TASK_RENDERED_REQUIREMENT") };
    let off = task_acceptance_snapshot(None, false, None, 0, 0, 0, None, "answer", None, || None);
    assert!(off.is_none());
    if let Some(value) = prior {
        unsafe { std::env::set_var("ANGEL_TASK_RENDERED_REQUIREMENT", value) }
    }
}

#[test]
fn revision_hasher_stays_idle_when_rendered_requirement_is_off() {
    let _guard = crate::tests::env_lock();
    let prior = std::env::var_os("ANGEL_TASK_RENDERED_REQUIREMENT");
    unsafe { std::env::remove_var("ANGEL_TASK_RENDERED_REQUIREMENT") };
    let mut off_hits = 0u32;
    let off = task_acceptance_snapshot(None, false, None, 0, 0, 0, None, "answer", None, || {
        off_hits += 1;
        panic!("workspace hasher must not run when rendered requirement is off");
    });
    assert!(off.is_none());
    assert_eq!(off_hits, 0);

    unsafe { std::env::set_var("ANGEL_TASK_RENDERED_REQUIREMENT", "1") };
    let mut on_hits = 0u32;
    let on = task_acceptance_snapshot(None, false, None, 0, 0, 0, None, "answer", None, || {
        on_hits += 1;
        Some("rev-on".into())
    })
    .expect("requirement on hashes once");
    assert_eq!(on_hits, 1);
    assert_eq!(
        on.rendered_output
            .as_ref()
            .and_then(|r| r.current_revision_sha256.as_deref()),
        Some("rev-on")
    );
    match prior {
        Some(value) => unsafe { std::env::set_var("ANGEL_TASK_RENDERED_REQUIREMENT", value) },
        None => unsafe { std::env::remove_var("ANGEL_TASK_RENDERED_REQUIREMENT") },
    }
}

#[test]
fn task_timing_parallel_wall_shares_and_overhead() {
    assert_eq!(tool_wall_shares(&[80, 80], 81), vec![40, 41]);
    assert_eq!(tool_wall_shares(&[0, 0], 3), vec![0, 0]);
    assert_eq!(tool_wall_shares(&[9, 3], 17), vec![9, 3]);
    let mut timing = TaskTimingAccumulator::default();
    timing.note_tool_batch(Duration::from_millis(100));
    timing.tool_member_ms = tool_wall_shares(&[80, 80], 81).iter().sum();
    let t = timing.finish(100);
    assert_eq!(t.tool_overhead_ms, 19);
    assert_eq!(t.tool_ms, timing.tool_member_ms + t.tool_overhead_ms);
}

#[test]
fn task_timing_calls_have_durations_and_stream_idle_observations() {
    let mut timing = TaskTimingAccumulator {
        call_start_ms: 20,
        ..Default::default()
    };
    timing.note_visible(30);
    timing.note_visible(60);
    timing.note_model_wait(Duration::from_millis(80));
    timing.note_model_span(20, 100);
    let t = timing.finish(100);
    let call = &t.calls["model_calls"][0];
    assert_eq!(call["ms"], 80);
    assert_eq!(call["first_delta_ms"], 10);
    assert_eq!(call["stream_ms"], 70);
    assert_eq!(call["idle_max_ms"], 40);
    assert!(call["retry_of"].is_null());
    assert_eq!(
        TaskTimingAccumulator::default().finish(0).calls["model_calls"],
        serde_json::json!([])
    );
}

#[test]
fn p06c_accumulator_background_is_an_overlapping_work_breakdown() {
    let mut timing = TaskTimingAccumulator::default();
    timing.background.note_for_test("aging_ms", 750_000);
    timing.background.note_for_test("aging_ms", 750_000);
    timing.background.note_for_test("compaction_ms", 90_000_000);
    timing.note_model_wait(Duration::from_millis(80));
    let receipt = timing.finish(100);
    assert_eq!(receipt.background.aging_ms, 1);
    assert_eq!(receipt.background.compaction_ms, 90);
    assert_eq!(receipt.model_ms, 80);
    assert_eq!(receipt.other_ms, 20);
    assert_eq!(
        receipt.wall_ms,
        receipt.model_ms + receipt.tool_ms + receipt.other_ms
    );
}

#[test]
fn task_timing_accumulator_splits_elapsed_without_underflow() {
    let mut timing = TaskTimingAccumulator::default();
    timing.note_model_wait(Duration::from_millis(1_200));
    timing.note_model_wait(Duration::from_millis(800));
    timing.note_model_retry_wait(Duration::from_millis(150));
    timing.note_tool_batch(Duration::from_millis(300));
    timing.note_tool_call("shell", Duration::from_millis(250));
    timing.note_tool_call("read_file", Duration::from_millis(40));
    timing.note_tool_result(true, false);
    timing.note_tool_result(true, true);
    timing.note_tool_result(false, false);
    let telemetry = timing.finish(3_000);
    assert_eq!(telemetry.schema, "angel-task-timing/v2");
    assert_eq!(telemetry.model_ms, 2_150);
    assert_eq!(telemetry.model_calls, 2);
    assert_eq!(telemetry.model_retry_ms, 150);
    assert_eq!(telemetry.model_retries, 1);
    assert_eq!(telemetry.tool_ms, 300);
    assert_eq!(telemetry.tool_calls, 2);
    assert_eq!(telemetry.tool_errors, 1);
    assert_eq!(telemetry.tool_max_ms, 250);
    assert_eq!(telemetry.tool_max_name.as_deref(), Some("shell"));
    assert_eq!(telemetry.other_ms, 550);
}

#[test]
fn trace_schema_timing_retry_samples_reconcile_once() {
    let mut timing = TaskTimingAccumulator::default();
    timing.note_model_wait(Duration::from_millis(100));
    timing.note_model_span(20, 120);
    timing.note_model_retry_wait(Duration::from_millis(30));
    timing.note_model_wait(Duration::from_millis(200));
    timing.note_model_span(150, 350);
    timing.note_tool_batch(Duration::from_millis(80)); // two parallel 80ms tools
    timing.note_tool_call("a", Duration::from_millis(80));
    timing.note_tool_call("b", Duration::from_millis(80));
    timing.note_visible(170);
    timing.note_visible(200);
    let t = timing.finish(500);
    assert_eq!(
        t.calls["model_calls"][0]["retry_of"],
        serde_json::Value::Null
    );
    assert_eq!(t.calls["model_calls"][1]["retry_of"], 0);
    assert_eq!(t.calls["model_calls"].as_array().unwrap().len(), 2);
    assert_eq!(t.model_ms, 330); // backoff is included exactly once
    assert_eq!(t.tool_ms, 80); // parallel members are not summed
    assert_eq!(t.residual_ms, 90);
    assert_eq!(
        t.wall_ms,
        t.model_ms + t.tool_ms + t.serial_overhead_ms + t.residual_ms
    );
    assert_eq!(t.overlap_ms, 0);
    assert_eq!(t.spans["first_visible_output"], 170);
}

#[test]
fn p05b_reasoning_and_answer_timestamps_are_separate() {
    let mut timing = TaskTimingAccumulator {
        call_start_ms: 100,
        ..Default::default()
    };
    timing.note_visible(150);
    timing.note_visible(180);
    timing.note_answer(500);
    timing.note_answer(550);
    timing.note_model_span(100, 600);
    let result = timing.finish(600);
    assert_eq!(result.spans["first_visible_ms"], 150);
    assert_eq!(result.spans["ttft_ms"], 150);
    assert_eq!(result.spans["first_answer_token_ms"], 500);
    assert!(
        result.spans["ttft_ms"].as_u64().unwrap()
            < result.spans["first_answer_token_ms"].as_u64().unwrap()
    );
    assert_eq!(result.calls["model_calls"][0]["first_visible_ms"], 50);
    assert_eq!(result.calls["model_calls"][0]["ttft_ms"], 50);
    assert_eq!(result.calls["model_calls"][0]["first_answer_token_ms"], 400);

    let mut plain = TaskTimingAccumulator::default();
    plain.note_answer(200);
    plain.note_model_span(0, 300);
    let result = plain.finish(300);
    assert_eq!(
        result.spans["first_visible_ms"],
        result.spans["first_answer_token_ms"]
    );
    let empty = TaskTimingAccumulator::default().finish(10);
    assert!(empty.spans["first_visible_ms"].is_null());
    assert!(empty.spans["first_answer_token_ms"].is_null());
}

#[test]
fn p05b_scripted_sse_separates_progress_from_answer_in_timing_ledger() {
    let _guard = crate::tests::env_lock();
    let mut acc = crate::agent::club::StreamAccumulator::default();
    let mut timing = TaskTimingAccumulator::default();
    let mut events = Vec::new();
    for (elapsed, line) in [
        (
            50,
            r#"data: {"choices":[{"delta":{"reasoning_content":"fixture progress"}}]}"#,
        ),
        (400, r#"data: {"choices":[{"delta":{"content":"answer"}}]}"#),
    ] {
        let crate::agent::club::SseEvent::Chunk(chunk) = crate::agent::club::parse_sse_line(line)
        else {
            panic!("expected scripted SSE chunk")
        };
        let delta = acc.apply_chunk(&chunk);
        if let Some(text) = delta.reasoning {
            timing.note_visible(elapsed);
            events.push(TurnEvent::Reasoning(text));
        }
        if let Some(text) = delta.content {
            timing.note_answer(elapsed);
            events.push(TurnEvent::Token(text));
        }
    }
    assert!(matches!(&events[0], TurnEvent::Reasoning(text) if text == "fixture progress"));
    assert!(matches!(&events[1], TurnEvent::Token(text) if text == "answer"));
    assert!(
        matches!(acc.into_reply(false), crate::agent::club::ClubReply::Text(text) if text == "answer")
    );
    timing.note_model_span(0, 500);
    let ledger = timing.finish(500);
    assert_eq!(ledger.spans["first_visible_ms"], 50);
    assert_eq!(ledger.spans["first_answer_token_ms"], 400);
    assert_eq!(ledger.calls["model_calls"][0]["first_visible_ms"], 50);
    assert_eq!(ledger.calls["model_calls"][0]["first_answer_token_ms"], 400);
}

#[test]
fn task_timing_accumulator_never_underflows_other_ms() {
    // Clock skew or tiered reruns can hand finalize a turn elapsed smaller
    // than the accumulated waits: `other_ms` must clamp at zero, not wrap.
    let mut timing = TaskTimingAccumulator::default();
    timing.note_model_wait(Duration::from_millis(5_000));
    timing.note_tool_batch(Duration::from_millis(2_000));
    let telemetry = timing.finish(1_000);
    assert_eq!(telemetry.other_ms, 0);
    let empty = TaskTimingAccumulator::default().finish(0);
    assert_eq!(empty.other_ms, 0);
    assert_eq!(empty.model_ms, 0);
    assert_eq!(empty.tool_calls, 0);
    assert!(empty.tool_max_name.is_none());
}
