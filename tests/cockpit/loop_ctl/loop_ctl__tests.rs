use super::*;

#[test]
fn loop_checkpoints_are_distinct_for_sessions_in_the_same_workspace() {
    let _lock = crate::tests::env_lock();
    let _file = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_FILE");
    let workspace = Path::new("/tmp/angel-shared-workspace");

    let alpha = store_path_for_session(Some(workspace), Some("session-alpha"));
    let beta = store_path_for_session(Some(workspace), Some("session-beta"));
    let legacy = store_path_for_session(Some(workspace), None);

    assert_ne!(alpha, beta, "concurrent shells must not share loop state");
    assert_ne!(alpha, legacy);
    assert_ne!(beta, legacy);
    assert_eq!(alpha.parent(), beta.parent());
}

#[test]
fn default_caps_are_unbounded_and_prompt_names_no_cap() {
    let _lock = crate::tests::env_lock();
    let _iters = crate::tests::TestEnvGuard::set("ANGEL_LOOP_MAX_ITERS", "");
    let _time = crate::tests::TestEnvGuard::set("ANGEL_LOOP_DEADLINE_SECS", "");
    let _tokens = crate::tests::TestEnvGuard::set("ANGEL_LOOP_TOKEN_BUDGET", "");
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState::configured_from_env();
    assert_eq!(
        (
            app.loop_ctl.max_iters,
            app.loop_ctl.deadline_secs,
            app.loop_ctl.token_budget
        ),
        (0, 0, 0)
    );
    app.loop_ctl.iteration = 100_000;
    app.loop_ctl.tokens_spent = usize::MAX;
    assert!(budget_tripped(&app.loop_ctl).is_none());
    let prompt = app
        .loop_iteration_convo()
        .pop()
        .unwrap()
        .content
        .to_string();
    let header = prompt
        .split("[files changed so far")
        .next()
        .unwrap_or(&prompt);
    assert!(header.contains("iteration 100000 · no cap"));
    assert!(!header.to_lowercase().contains("budget"), "{header}");
}

#[test]
fn max_zero_reports_other_operator_caps() {
    let _lock = crate::tests::env_lock();
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState {
        max_iters: 2,
        deadline_secs: 900,
        token_budget: 123,
        ..Default::default()
    };
    let message = app.loop_command(Some("max 0".into()));
    assert_eq!(app.loop_ctl.max_iters, 0);
    assert!(message.contains("deadline 900s"), "{message}");
    assert!(message.contains("tokens ~123"), "{message}");
}

#[test]
fn endless_clears_every_cap_and_resumes_in_one_command() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-l00-endless-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let _dir = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "improve".into(),
        workspace: Some(root.clone()),
        max_iters: 2,
        iteration: 2,
        deadline_secs: 900,
        token_budget: 100,
        operator_caps: true,
        ..Default::default()
    };
    let why = budget_tripped(&app.loop_ctl).unwrap();
    assert_eq!(why, "max iterations (2)");
    app.loop_pause_for_budget(&why);
    assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
    assert!(
        app.messages
            .last()
            .unwrap()
            .text
            .contains("max iterations (2)")
    );
    let reply = app.loop_command(Some("endless".into()));
    assert!(reply.contains("resumed"), "{reply}");
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert_eq!(app.loop_ctl.cap_summary(), "no cap");
    assert!(budget_tripped(&app.loop_ctl).is_none());
    let saved = load().unwrap();
    assert_eq!(saved.cap_summary(), "no cap");
    let mut legacy = serde_json::to_value(&saved).unwrap();
    legacy.as_object_mut().unwrap().remove("operator_caps");
    legacy["status"] = serde_json::json!("paused");
    legacy["deadline_secs"] = serde_json::json!(900);
    legacy["token_budget"] = serde_json::json!(2_000_000);
    std::fs::write(root.join("loop.json"), serde_json::to_vec(&legacy).unwrap()).unwrap();
    let migrated = load().unwrap();
    assert_eq!(migrated.cap_summary(), "no cap");
    assert!(migrated.paused_for_cap);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn openai_family_labels_are_recognized() {
    assert!(is_openai_family_label("openai"));
    assert!(is_openai_family_label("openai-gpt"));
    assert!(is_openai_family_label("codex"));
    assert!(is_openai_family_label("gpt-5.6-luna"));
    assert!(!is_openai_family_label("deepseek-v4-flash"));
    assert!(!is_openai_family_label("dsflash"));
    assert!(!is_openai_family_label("glm-5.2"));
    assert!(!is_openai_family_label("kimi-k3"));
}

#[test]
fn loop_sota_prefers_in_hand_deepseek_over_openai() {
    let labels = vec![
        "openai".into(),
        "deepseek-v4-flash".into(),
        "glm".into(),
        "practice".into(),
    ];
    let avail = vec![true, true, true, true];
    let pick = preferred_loop_sota_label(
        "deepseek-v4-flash",
        &labels,
        &avail,
        /*allow_openai=*/ false,
        None,
    );
    assert_eq!(pick.as_deref(), Some("deepseek-v4-flash"));
}

#[test]
fn loop_sota_preserves_selected_api_route_under_default_policy() {
    let _g = crate::tests::env_lock();
    let _policy = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_SOTA_ALLOW_OPENAI");
    let labels = vec!["glm".into(), "openai-api".into()];
    assert!(loop_sota_allow_openai());
    let selected = preferred_loop_sota_label(
        "openai-api",
        &labels,
        &[true, true],
        loop_sota_allow_openai(),
        None,
    );
    assert_eq!(selected.as_deref(), Some("openai-api"));
}

#[test]
fn loop_sota_skips_openai_when_disallowed_even_if_first() {
    let labels = vec!["openai".into(), "glm".into(), "kimi".into()];
    let avail = vec![true, true, true];
    let pick = preferred_loop_sota_label("practice", &labels, &avail, false, None);
    assert_eq!(pick.as_deref(), Some("glm"));
    // Pin still wins when operator explicitly wants openai.
    let pinned = preferred_loop_sota_label("practice", &labels, &avail, false, Some("openai"));
    assert_eq!(pinned.as_deref(), Some("openai"));
    // Empty when only openai is available and disallowed.
    let only_oai = preferred_loop_sota_label("practice", &["openai".into()], &[true], false, None);
    assert_eq!(only_oai, None);
}

#[test]
fn loop_initial_tier_honors_selected_oauth_model_and_explicit_override() {
    let labels = vec![
        "openai".into(),
        "qwen3.8-27b".into(),
        "tiny-1b".into(),
        "practice".into(),
    ];
    let avail = vec![true, true, true, true];
    let pick = preferred_loop_local_label("openai", &labels, &avail, None);
    assert_eq!(pick.as_deref(), Some("openai"));
    let pinned = preferred_loop_local_label("openai", &labels, &avail, Some("tiny-1b"));
    assert_eq!(pinned.as_deref(), Some("tiny-1b"));
    let already_local = preferred_loop_local_label("tiny-1b", &labels, &avail, None);
    assert_eq!(already_local.as_deref(), Some("tiny-1b"));
    let selected = preferred_loop_local_label(
        "openai",
        &["openai".into(), "practice".into()],
        &[true, true],
        None,
    );
    assert_eq!(selected.as_deref(), Some("openai"));
    for model in ["openai", "grok-4.6", "glm-5.3-flash"] {
        assert_eq!(
            preferred_loop_local_label(model, &["meta".into(), model.into()], &[true, true], None)
                .as_deref(),
            Some(model),
            "an incidental Meta route must not hijack the selected model"
        );
    }
    assert_eq!(
        preferred_loop_local_label(
            "practice",
            &["meta".into(), "practice".into()],
            &[true, true],
            None
        ),
        None,
        "Meta must never masquerade as a local fleet seat"
    );
}

#[test]
fn provider_account_error_delays_retry_without_stall_or_swarm_escalation() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-provider-error-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    for error in [
        r#"HTTP 402: {"error":{"code":"billing_not_configured","type":"billing_error"}}"#,
        "HTTP 401: Unauthorized",
        "HTTP 403: Forbidden",
        "HTTP 429: monthly quota exhausted",
    ] {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = LoopState {
            status: LoopStatus::Running,
            awaiting_turn: true,
            stale_count: 3,
            task: "improve the candidate".into(),
            workspace: Some(root.clone()),
            podrace: true,
            ..Default::default()
        };
        app.loop_harvest_error_with_tools(error.into(), ToolStripSnapshot::default());
        assert_eq!(app.loop_ctl.status, LoopStatus::Running, "{error}");
        assert_eq!(app.loop_ctl.iteration, 1);
        assert_eq!(app.loop_ctl.tier, EscalationTier::Local);
        assert_eq!(app.loop_ctl.stale_count, 3);
        assert_eq!(app.loop_ctl.tool_calls_total, 0);
        assert_eq!(app.loop_ctl.last_error.as_deref(), Some(error));
        assert!(app.loop_ctl.wake_at.unwrap() > Instant::now() + Duration::from_secs(59));
        app.loop_arm();
        assert!(app.thinking.is_none(), "no immediate paid retry");
        assert!(!app.loop_ctl.awaiting_turn);
        assert!(app.loop_pending.is_none());
        let saved: LoopState =
            serde_json::from_slice(&std::fs::read(root.join("loop.json")).unwrap()).unwrap();
        assert_eq!(saved.status, LoopStatus::Running);
        assert_eq!(saved.last_error.as_deref(), Some(error));
    }
    for error in [
        "HTTP 429: Too Many Requests",
        "HTTP 503: temporarily unavailable",
        "transport timeout",
    ] {
        assert!(!crate::agent::club::error_requires_provider_action(error));
    }
    std::fs::remove_file(root.join("loop.json")).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn execution_blocker_pauses_once_without_retry_and_preserves_progress() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-execution-blocker-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        awaiting_turn: true,
        stale_count: 2,
        task: "improve candidate".into(),
        workspace: Some(root.clone()),
        podrace: true,
        ..Default::default()
    };
    crate::agent::harness::exec::set_sandbox_receipt(Some(
        serde_json::json!({"helper_error": "Landlock unavailable; run angel --doctor", "helper_phase": "landlock", "helper_exit": 1}),
    ));
    let error = crate::agent::harness::execution_blocker(
        "shell",
        "tool error: shell command failed (exit 1)\nbwrap: setting up uid map: Permission denied",
    )
    .unwrap();
    app.loop_harvest_error_with_tools(
        error.clone(),
        ToolStripSnapshot {
            calls: 20,
            errors: 1,
            outcome_actions: vec!["new synthetic probe".into()],
            ..Default::default()
        },
    );
    assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
    assert_eq!(app.loop_ctl.tier, EscalationTier::Local);
    assert_eq!(app.loop_ctl.stale_count, 2);
    assert!(app.loop_ctl.outcome_actions_seen.is_empty());
    assert!(app.loop_ctl.wake_at.is_none());
    assert!(!app.loop_ctl.retry_after_error);
    app.loop_arm();
    assert!(app.thinking.is_none());
    assert!(!app.loop_ctl.awaiting_turn);
    let saved: LoopState =
        serde_json::from_slice(&std::fs::read(root.join("loop.json")).unwrap()).unwrap();
    assert_eq!(saved.status, LoopStatus::Paused);
    assert_eq!(saved.execution_blocker.as_deref(), Some(error.as_str()));
    app.loop_ctl = saved;
    app.loop_command(Some("pause".into()));
    assert_eq!(app.loop_resume(), "loop resumed");
    assert!(app.loop_ctl.execution_blocker.is_none());
    assert_eq!(app.loop_ctl.last_setback.as_deref(), Some(error.as_str()));
    app.loop_harvest_error_with_tools(error, ToolStripSnapshot::default());
    assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
    assert_eq!(app.loop_ctl.iteration, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn podrace_submission_clock_steers_a_measured_but_unsubmitted_run() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-first-submission-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let podrace = || LoopState {
        status: LoopStatus::Running,
        task: "produce wins on the board".into(),
        workspace: Some(root.clone()),
        podrace: true,
        first_candidate_iters: 3,
        first_submission_iters: 2,
        ..Default::default()
    };
    let measured = |what: &str| ToolStripSnapshot {
        calls: 1,
        verified_outcome_actions: vec![format!("measured:shell:./benchmark.sh {what}:result=ab")],
        ..Default::default()
    };

    // Measured on iteration 1; two more iterations without a submission
    // make it overdue (directive), two beyond that reinforce the directive.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.terminal_focused = false;
    app.loop_harvest_with_tools("DIRECTION: measure the candidate".into(), measured("a"));
    assert_eq!(app.loop_ctl.first_measured_iteration, Some(1));
    app.loop_harvest("DIRECTION: tune".into());
    assert!(
        !app.loop_submission_overdue(),
        "one iteration since: not yet"
    );
    app.loop_harvest("DIRECTION: tune more".into());
    assert!(
        app.loop_submission_overdue(),
        "two iterations since the measurement"
    );
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    let prompt = app
        .loop_iteration_convo()
        .pop()
        .expect("iteration prompt")
        .content
        .to_string();
    assert!(
        prompt.contains("[SUBMISSION OVERDUE — 1 measured candidate(s), 0 submissions, 2"),
        "{prompt}"
    );
    app.loop_harvest_with_tools("DIRECTION: measure another".into(), measured("b"));
    assert_eq!(
        app.loop_ctl.status,
        LoopStatus::Running,
        "three since: still directing"
    );
    app.loop_harvest("DIRECTION: tune yet again".into());
    assert_eq!(
        app.loop_ctl.status,
        LoopStatus::Running,
        "4 iterations since: continue with submission steering"
    );
    let last_setback = app.loop_ctl.last_setback.clone().unwrap_or_default();
    assert!(
        last_setback.contains("2 measured candidate(s) and no submission in the 4 iterations"),
        "{last_setback}"
    );
    assert!(
        app.loop_ctl
            .last_setback
            .as_deref()
            .unwrap()
            .contains("submit the best measured candidate")
    );
    assert!(app.loop_ctl.wake_at.is_some());
    assert!(!app.take_attention_request());

    // A submission receipt silences the clock for good.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.loop_harvest_with_tools("DIRECTION: measure".into(), measured("a"));
    app.loop_harvest_with_tools(
        "DIRECTION: submit".into(),
        ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec!["submitted:shell:hilbert submit cand:result=cd".into()],
            ..Default::default()
        },
    );
    for i in 1..=6 {
        app.loop_harvest(format!("DIRECTION: next {i}"));
    }
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert!(
        !app.loop_iteration_convo()
            .pop()
            .expect("iteration prompt")
            .content
            .to_string()
            .contains("SUBMISSION OVERDUE")
    );

    // Knob 0: never fires.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.loop_ctl.first_submission_iters = 0;
    app.loop_harvest_with_tools("DIRECTION: measure".into(), measured("a"));
    for i in 1..=6 {
        app.loop_harvest(format!("DIRECTION: drift {i}"));
    }
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
}

fn scripted_verifier_failure(command: &str, output: &str) -> ToolStripSnapshot {
    let mut strip = crate::ui::toolstrip::ToolStrip::default();
    let id = crate::agent::harness::ToolEventId("blocked-fixture".into());
    let args = crate::agent::harness::summarize_args(&serde_json::json!({"command": command}));
    strip.call_event(id.clone(), "shell", &args);
    strip.result_event(
        &id,
        "shell",
        output,
        crate::agent::harness::ToolOutcome {
            execution: crate::agent::harness::ExecutionOutcome::Failed,
            verification: crate::agent::harness::VerificationOutcome::NotApplicable,
        },
    );
    strip.snapshot()
}

#[test]
fn loop_ctl_blocked_tail_redaction_and_stable_digest() {
    let _lock = crate::tests::env_lock();
    let _secret =
        crate::tests::TestEnvGuard::set("ANGEL_T_BLOCKED_SECRET", "fixture-credential-123456");
    let command = "./benchmark.sh --token fixture-credential-123456 --golden fixture.json";
    let snap = scripted_verifier_failure(
        command,
        "tool error: shell command failed (exit 1)\nold stdout\n[stderr] missing golden\n[stderr] fixture-credential-123456\nlast stdout",
    );
    let failure = &snap.verifier_failure_details[0];
    assert_eq!(failure.exit_code, "1");
    assert!(failure.tail.starts_with("[stderr] missing golden"));
    assert!(failure.tail.ends_with("last stdout"));
    assert!(!failure.diagnostic().contains("fixture-credential-123456"));
    assert!(failure.diagnostic().contains("redacted"));
    assert!(failure.command.ends_with("--golden fixture.json"));
    assert!(failure.diagnostic().contains("failure_digest:"));
    let lines = crate::ui::toolstrip::VerifierFailure::new(
        "./benchmark.sh",
        "shell command failed (exit 1)\n0\n1\n2\n3\n4\n5\n6\n7",
    );
    assert_eq!(lines.tail, "2\n3\n4\n5\n6\n7");
    let same = scripted_verifier_failure(
        command,
        "tool error: shell command failed (exit 1)\nold   stdout\n[stderr] missing   golden\n[stderr] fixture-credential-123456\nlast stdout",
    );
    assert_eq!(
        failure.failure_digest,
        same.verifier_failure_details[0].failure_digest
    );
    let long = crate::ui::toolstrip::VerifierFailure::new(
        &"é".repeat(250),
        &format!("shell command failed (exit 2)\n{}\nend", "é".repeat(500)),
    );
    assert!(long.command.chars().count() <= 200);
    assert!(long.tail.len() <= 600);
    assert!(long.tail.ends_with("end"));
    assert!(long.tail.lines().count() <= 6);
    let empty = crate::ui::toolstrip::VerifierFailure::new(
        "./benchmark.sh",
        "shell command failed (exit 1)",
    );
    assert!(empty.tail.contains("no output captured"));
}

#[test]
fn loop_ctl_blocked_repeat_escalates_and_continues_by_default() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-loop-blocked-default-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let _limit = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_BLOCKED_REPEAT_LIMIT");
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "benchmark candidate".into(),
        podrace: true,
        workspace: Some(root.clone()),
        ..Default::default()
    };
    let failed = scripted_verifier_failure(
        "./benchmark.sh --golden fixture.json",
        "tool error: shell command failed (exit 1)\n[stderr] missing fixture.json",
    );
    app.loop_harvest_with_tools("VERIFY blocked".into(), failed.clone());
    app.loop_ctl.blocked_prompt_digest =
        Some(failed.verifier_failure_details[0].failure_digest.clone());
    app.loop_harvest_error_with_tools("fixture tool failed".into(), failed.clone());
    app.loop_harvest("VERIFY blocked: please restore it".into());
    // Three identical blocked iterations: escalation recorded, instruction issued, loop still running.
    assert_ne!(app.loop_ctl.status, LoopStatus::Paused);
    assert_eq!(app.loop_ctl.escalations.len(), 1);
    assert_eq!(
        app.loop_ctl.escalations[0]["kind"],
        "verifier_blocked_repeat"
    );
    assert_eq!(app.loop_ctl.blocked_repeat_count, 0);
    let setback = app.loop_ctl.last_setback.clone().unwrap_or_default();
    assert!(
        setback.contains("do not run that command again"),
        "{setback}"
    );
    assert!(setback.contains("missing fixture.json"), "{setback}");
}

#[test]
fn loop_ctl_blocked_repeat_pause_ledger_and_gauge() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-loop-blocked-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let _limit = crate::tests::TestEnvGuard::set("ANGEL_LOOP_BLOCKED_REPEAT_LIMIT", "3"); // explicit operator cap: pause semantics under test
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "benchmark candidate".into(),
        podrace: true,
        workspace: Some(root.clone()),
        ..Default::default()
    };
    let failed = scripted_verifier_failure(
        "./benchmark.sh --golden fixture.json",
        "tool error: shell command failed (exit 1)\n[stderr] missing fixture.json",
    );
    app.loop_harvest_with_tools("VERIFY blocked".into(), failed.clone());
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert_eq!(app.loop_ctl.blocked_repeat_count, 1);
    let prompt = app.loop_iteration_convo();
    assert!(
        prompt
            .last()
            .unwrap()
            .content
            .to_string()
            .contains("VERIFICATION PATH BLOCKED")
    );
    app.loop_ctl.blocked_prompt_digest =
        Some(failed.verifier_failure_details[0].failure_digest.clone());
    let same_digest = scripted_verifier_failure(
        "./benchmark.sh --golden fixture.json",
        "tool error: shell command failed (exit 1)\n[stderr] missing   fixture.json",
    );
    app.loop_harvest_error_with_tools("fixture tool failed".into(), same_digest);
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert_eq!(app.loop_ctl.blocked_repeat_count, 2);
    assert!(
        !app.loop_iteration_convo()
            .last()
            .unwrap()
            .content
            .to_string()
            .contains("VERIFICATION PATH BLOCKED")
    );
    // Model-only repetition still counts as an unresolved blocked iteration.
    app.loop_harvest("VERIFY blocked: please restore it".into());
    assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
    assert!(app.loop_ctl.wake_at.is_none());
    let rows: Vec<_> = app
        .messages
        .iter()
        .filter(|m| matches!(m.role, Role::Activity) && m.text.contains("verification blocked —"))
        .collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].text.ends_with("×3"));
    assert_eq!(app.loop_ctl.escalations.len(), 1);
    let entry = &app.loop_ctl.escalations[0];
    assert_eq!(entry["kind"], "verifier_blocked_repeat");
    assert_eq!(entry["iteration_start"], 1);
    assert_eq!(entry["iteration_end"], 3);
    let reason = entry["diagnostic"].as_str().unwrap();
    assert!(reason.contains("exit code: 1"));
    assert!(reason.contains("missing fixture.json"));
    assert!(reason.contains(&format!(
        "run ./benchmark.sh --golden fixture.json in {}; /loop resume after it exits 0",
        root.display()
    )));
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("loop.json")).unwrap()).unwrap();
    assert_eq!(saved["escalations"][0], *entry);
    assert!(
        crate::agent::harness::progress_ledger_snapshot()["escalations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "verifier_blocked_repeat")
    );
    assert_eq!(app.loop_ctl.log.len(), 3);
    assert_eq!(app.loop_resume(), "loop resumed");
    assert_eq!(app.loop_ctl.blocked_repeat_count, 0);
    assert!(app.loop_ctl.blocked_prompt_digest.is_none());
    let _two = crate::tests::TestEnvGuard::set("ANGEL_LOOP_BLOCKED_REPEAT_LIMIT", "2");
    app.loop_harvest_with_tools("VERIFY blocked".into(), failed.clone());
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    app.loop_harvest_with_tools("VERIFY blocked".into(), failed);
    assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
    assert_eq!(app.loop_ctl.escalations.len(), 2);
    assert_eq!(app.loop_ctl.escalations[1]["iteration_start"], 4);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn loop_ctl_blocked_changed_failure_reset_verified_clear_and_opt_out() {
    let _lock = crate::tests::env_lock();
    let _limit = crate::tests::TestEnvGuard::set("ANGEL_LOOP_BLOCKED_REPEAT_LIMIT", "0");
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "benchmark candidate".into(),
        podrace: true,
        ..Default::default()
    };
    let a = scripted_verifier_failure(
        "./benchmark.sh",
        "shell command failed (exit 1)\n[stderr] missing fixture",
    );
    for _ in 0..4 {
        app.loop_harvest_with_tools("VERIFY blocked".into(), a.clone());
    }
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert_eq!(app.loop_ctl.blocked_repeat_count, 4);
    assert!(app.loop_ctl.escalations.is_empty());
    let b = scripted_verifier_failure(
        "./benchmark.sh",
        "shell command failed (exit 2)\n[stderr] invalid fixture",
    );
    app.loop_harvest_with_tools("VERIFY blocked".into(), b);
    assert_eq!(app.loop_ctl.blocked_repeat_count, 1);
    assert_eq!(app.loop_ctl.blocked_first_iteration, 5);
    app.loop_harvest_with_tools(
        "DIRECTION: verified".into(),
        ToolStripSnapshot {
            verified_outcome_actions: vec!["measured:shell:./benchmark.sh:result=ok".into()],
            ..Default::default()
        },
    );
    assert!(app.loop_ctl.verifier_blocked.is_none());
    assert!(app.loop_ctl.verifier_failure.is_none());
    assert_eq!(app.loop_ctl.blocked_repeat_count, 0);
}

#[test]
fn blocked_verifier_rides_first_in_the_iteration_prompt() {
    let mut app = crate::seed_preview_app();
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "win the kernel podrace".into(),
        podrace: true,
        verifier_blocked: Some("benchctl measure-job: missing required --golden".into()),
        steer_notes: vec!["keep the ledger current".into()],
        ..Default::default()
    };
    let prompt = app
        .loop_iteration_convo()
        .pop()
        .expect("iteration prompt")
        .content
        .to_string();
    let blocked_at = prompt
        .find("[VERIFICATION PATH BLOCKED — restore it before anything else]")
        .expect("the blocked block rides the prompt");
    assert!(
        prompt.contains("Do not propose a new optimization direction"),
        "{prompt}"
    );
    assert!(
            prompt
                .contains("Notes in the repository are not a blocker; missing inputs that a script in the repository can fetch are not a blocker"),
            "{prompt}"
        );
    let steer_at = prompt
        .find("[operator steering")
        .expect("steer block present for ordering");
    assert!(
        blocked_at < steer_at,
        "the blocked block must precede the steering block"
    );

    app.loop_ctl.verifier_blocked = None;
    let clean = app
        .loop_iteration_convo()
        .pop()
        .expect("iteration prompt")
        .content
        .to_string();
    assert!(!clean.contains("VERIFICATION PATH BLOCKED"));
}

#[test]
fn podrace_first_candidate_clock_steers_unmeasured_runs() {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-first-candidate-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let podrace = || LoopState {
        status: LoopStatus::Running,
        task: "improve the kernel".into(),
        workspace: Some(root.clone()),
        podrace: true,
        first_candidate_iters: 3,
        ..Default::default()
    };

    // Default clock: three unmeasured iterations steer the next measurement.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.terminal_focused = false;
    app.loop_harvest("DIRECTION: explore micro-opt 1".into());
    app.loop_harvest("DIRECTION: explore micro-opt 2".into());
    assert_eq!(
        app.loop_ctl.status,
        LoopStatus::Running,
        "the clock arms at 3, not 2"
    );
    app.loop_harvest("DIRECTION: explore micro-opt 3".into());
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    let last_setback = app.loop_ctl.last_setback.clone().unwrap_or_default();
    assert!(
        last_setback.contains("no measured candidate in 3 iterations"),
        "{last_setback}"
    );
    assert!(
        last_setback.contains("no verified measured-candidate receipt recorded"),
        "{last_setback}"
    );
    assert!(app.messages.iter().any(|m| matches!(m.role, Role::System)
        && m.text.contains("no measured candidate in 3 iterations")));
    assert!(app.loop_ctl.wake_at.is_some());
    assert!(
        app.loop_ctl
            .last_setback
            .as_deref()
            .unwrap()
            .contains("next action")
    );
    assert!(!app.take_attention_request());

    // An armed blocker diagnostic rides the continuation directive.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.loop_ctl.verifier_blocked = Some("benchctl measure-job: missing required --golden".into());
    app.loop_ctl.iteration = 2;
    app.loop_harvest("DIRECTION: explore micro-opt 3".into());
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert!(
        app.loop_ctl
            .last_setback
            .as_deref()
            .unwrap_or_default()
            .contains("missing required --golden")
    );

    // One measured candidate keeps the run alive past the clock.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.loop_harvest_with_tools(
        "DIRECTION: measure locally".into(),
        ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec![
                "measured:shell:./benchmark.sh --local-iterate:result=ab".into(),
            ],
            ..Default::default()
        },
    );
    app.loop_harvest("DIRECTION: tune the kernel".into());
    app.loop_harvest("DIRECTION: tune again".into());
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert_eq!(app.loop_ctl.measured_candidates, 1);
    assert!(app.loop_ctl.wake_at.is_some(), "continuation armed");

    // Knob 0: the clock never fires.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.loop_ctl.first_candidate_iters = 0;
    for i in 1..=5 {
        app.loop_harvest(format!("DIRECTION: drift {i}"));
    }
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);

    // Non-podrace loops are unaffected even with the knob armed.
    let mut app = crate::seed_preview_app();
    app.loop_ctl = podrace();
    app.loop_ctl.podrace = false;
    for i in 1..=5 {
        app.loop_harvest(format!("DIRECTION: build {i}"));
    }
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn loop_never_auto_picks_mathgod() {
    let labels = vec![
        "mathgod".into(),
        "qwen3.8-27b".into(),
        "glm".into(),
        "practice".into(),
    ];
    let avail = vec![true, true, true, true];
    assert_eq!(
        preferred_loop_local_label("openai", &labels, &avail, None).as_deref(),
        Some("qwen3.8-27b")
    );
    assert_eq!(
        preferred_loop_local_label("mathgod", &labels, &avail, None).as_deref(),
        Some("qwen3.8-27b"),
        "mathgod in hand must not count as a local loop seat"
    );
    assert_eq!(
        preferred_loop_sota_label("qwen3.8-27b", &labels, &avail, false, None).as_deref(),
        Some("glm"),
        "mathgod must not be the loop SOTA escalation"
    );
    assert_eq!(
        preferred_loop_sota_label("mathgod", &labels, &avail, false, None).as_deref(),
        Some("glm")
    );
}

#[test]
fn parse_interval_units() {
    assert_eq!(parse_interval("30s"), Some(30));
    assert_eq!(parse_interval("5m"), Some(300));
    assert_eq!(parse_interval("2h"), Some(7200));
    assert_eq!(parse_interval("90"), Some(90));
    assert_eq!(parse_interval("ship"), None);
}

#[test]
fn split_interval_separates_cadence_from_task() {
    assert_eq!(
        split_interval("5m fix the bug"),
        (300, "fix the bug".to_string())
    );
    assert_eq!(split_interval("just do it"), (0, "just do it".to_string()));
    assert_eq!(split_interval("30s"), (30, String::new()));
    assert_eq!(split_interval("ship"), (0, "ship".to_string()));
}

#[test]
fn loop_start_options_parse_iteration_ghost_and_endless() {
    assert_eq!(
        parse_loop_start_options("[iterations=25] harden the harness"),
        ("harden the harness".to_string(), Some(25))
    );
    assert_eq!(
        parse_loop_start_options("iterations=7 ship it"),
        ("ship it".to_string(), Some(7))
    );
    assert_eq!(
        parse_loop_start_options("endless keep improving"),
        ("keep improving".to_string(), Some(0))
    );
    assert_eq!(
        parse_loop_start_options("[iterations=endless] keep improving"),
        ("keep improving".to_string(), Some(0))
    );
    assert_eq!(
        parse_loop_start_options("max=∞ keep improving"),
        ("keep improving".to_string(), Some(0))
    );
}

#[test]
fn parse_usize_or_endless_accepts_infinite_aliases() {
    assert_eq!(parse_usize_or_endless("25"), Some(25));
    assert_eq!(parse_usize_or_endless("0"), Some(0));
    assert_eq!(parse_usize_or_endless("endless"), Some(0));
    assert_eq!(parse_usize_or_endless("infinity"), Some(0));
    assert_eq!(parse_usize_or_endless("no_limit"), Some(0));
    assert_eq!(parse_usize_or_endless("nope"), None);
}

#[test]
fn cycle_elapsed_is_runtime_only() {
    let mut st = LoopState {
        status: LoopStatus::Running,
        wake_at: Some(Instant::now() + Duration::from_secs(10)),
        cycle_started_ms: None,
        ..Default::default()
    };
    assert_eq!(cycle_elapsed_secs(&st), None);
    st.wake_at = None;
    st.cycle_started_ms = Some(now_ms().saturating_sub(2_000));
    assert!(cycle_elapsed_secs(&st).is_some_and(|secs| secs <= 3));
}

#[test]
fn persisted_active_podrace_and_ordinary_loops_rearm_after_restart() {
    let path = std::env::temp_dir().join(format!(
        "angel-loop-podrace-resume-{}-{}.json",
        std::process::id(),
        now_ms()
    ));
    let write = |state: &LoopState| {
        std::fs::write(&path, serde_json::to_vec(state).unwrap()).unwrap();
    };

    write(&LoopState {
        status: LoopStatus::Running,
        podrace: true,
        ..Default::default()
    });
    let resumed = load_from_path(path.clone()).unwrap();
    assert_eq!(resumed.status, LoopStatus::Running);
    assert!(resumed.wake_at.is_some());

    write(&LoopState {
        status: LoopStatus::Running,
        ..Default::default()
    });
    let ordinary = load_from_path(path.clone()).unwrap();
    assert_eq!(ordinary.status, LoopStatus::Running);
    assert!(ordinary.wake_at.is_some());

    let _ = std::fs::remove_file(path);
}

fn acceptance_memo_workspace(tag: &str) -> (PathBuf, PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "angel-loop-red-memo-{tag}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&workspace)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "angel@example.invalid"]);
    git(&["config", "user.name", "Angel Test"]);
    std::fs::write(workspace.join("candidate.txt"), "one\n").unwrap();
    // Keep the process sentinel inside the fixture workspace but ignored,
    // so counting executions does not itself mutate the candidate
    // fingerprint under test.
    std::fs::write(workspace.join(".gitignore"), ".gate-runs.txt\n").unwrap();
    git(&["add", "candidate.txt", ".gitignore"]);
    git(&["commit", "-q", "-m", "seed"]);
    (root, workspace)
}

fn shell_single_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

fn red_sentinel_command(sentinel: &Path, marker: &str) -> String {
    format!(
        "printf '{marker}\\n' >> {}; printf 'acceptance red\\n'; exit 7",
        shell_single_quote(sentinel)
    )
}

fn execute_acceptance_plan_for_test(
    state: &mut LoopState,
    command: &str,
    workspace: &Path,
) -> (VerifyResult, bool) {
    match prepare_acceptance_gate(state, command, workspace) {
        AcceptanceGatePlan::Replay(result) => (result, false),
        AcceptanceGatePlan::Execute(run) => {
            // A literal process sentinel proves memo admission controls
            // process creation. Production execution is independently
            // covered by `run_accept_cmd_*`; its evaluator sandbox is
            // intentionally read-only outside ephemeral scratch, so it
            // cannot publish a durable sentinel by design.
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(workspace)
                .output()
                .unwrap();
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let result = VerifyResult {
                passed: output.status.success(),
                summary: format!("sentinel exit {:?}", output.status.code()),
                detail: failure_detail(&combined),
            };
            record_acceptance_result(state, run, &result, workspace);
            (result, true)
        }
    }
}

#[test]
fn unchanged_red_acceptance_executes_once_and_survives_state_reload() {
    let _lock = crate::tests::env_lock();
    let _threshold = crate::tests::TestEnvGuard::set("ANGEL_LOOP_VERIFY_THRESHOLD", "1");
    let (root, workspace) = acceptance_memo_workspace("unchanged");
    let sentinel = workspace.join(".gate-runs.txt");
    let command = red_sentinel_command(&sentinel, "same");
    let mut state = LoopState {
        workspace: Some(workspace.clone()),
        accept_cmd: Some(command.clone()),
        status: LoopStatus::Paused,
        ..Default::default()
    };

    let (first, first_executed) =
        execute_acceptance_plan_for_test(&mut state, &command, &workspace);
    assert!(first_executed);
    assert!(!first.passed);
    assert_eq!(state.last_failed_acceptance.as_ref().unwrap().attempts, 1);
    assert_eq!(
        std::fs::read_to_string(&sentinel).unwrap().lines().count(),
        1
    );

    // Exercise the actual persisted-state loader, not just an in-memory
    // clone: a process callback/resume may retain the red receipt only when
    // its cryptographic workspace key still matches.
    let state_file = root.join("state.json");
    std::fs::write(&state_file, serde_json::to_vec(&state).unwrap()).unwrap();
    let mut reloaded = load_from_path(state_file).expect("memoized loop state reloads");
    let (replayed, second_executed) =
        execute_acceptance_plan_for_test(&mut reloaded, &command, &workspace);
    assert!(!second_executed, "unchanged red must not spawn a process");
    assert!(!replayed.passed);
    assert!(
        replayed.summary.contains("process skipped"),
        "{}",
        replayed.summary
    );
    assert_eq!(
        reloaded.last_failed_acceptance.as_ref().unwrap().attempts,
        2
    );
    assert_eq!(
        std::fs::read_to_string(&sentinel).unwrap().lines().count(),
        1,
        "the process sentinel proves the unchanged red ran only once"
    );

    reloaded.last_failed_acceptance.as_mut().unwrap().attempts = ACCEPTANCE_RED_MEMO_MAX_ATTEMPTS;
    let (_, capped_executed) =
        execute_acceptance_plan_for_test(&mut reloaded, &command, &workspace);
    assert!(!capped_executed);
    assert_eq!(
        reloaded.last_failed_acceptance.as_ref().unwrap().attempts,
        ACCEPTANCE_RED_MEMO_MAX_ATTEMPTS,
        "unchanged-red attempt accounting must saturate at its persisted cap"
    );
    assert_eq!(
        std::fs::read_to_string(&sentinel).unwrap().lines().count(),
        1
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn changed_workspace_command_or_config_reexecutes_red_acceptance() {
    let _lock = crate::tests::env_lock();
    let _threshold = crate::tests::TestEnvGuard::set("ANGEL_LOOP_VERIFY_THRESHOLD", "1");
    let (root, workspace) = acceptance_memo_workspace("invalidate");
    let sentinel = workspace.join(".gate-runs.txt");
    let first_command = red_sentinel_command(&sentinel, "first-command");
    let mut state = LoopState {
        workspace: Some(workspace.clone()),
        accept_cmd: Some(first_command.clone()),
        status: LoopStatus::Paused,
        ..Default::default()
    };

    assert!(execute_acceptance_plan_for_test(&mut state, &first_command, &workspace).1);
    assert!(!execute_acceptance_plan_for_test(&mut state, &first_command, &workspace).1);

    std::fs::write(workspace.join("candidate.txt"), "two\n").unwrap();
    assert!(
        execute_acceptance_plan_for_test(&mut state, &first_command, &workspace).1,
        "tracked workspace mutation must force a fresh process"
    );
    assert_eq!(state.last_failed_acceptance.as_ref().unwrap().attempts, 1);

    let second_command = red_sentinel_command(&sentinel, "second-command");
    state.accept_cmd = Some(second_command.clone());
    assert!(
        execute_acceptance_plan_for_test(&mut state, &second_command, &workspace).1,
        "an exact command change must force a fresh process"
    );
    assert_eq!(state.last_failed_acceptance.as_ref().unwrap().attempts, 1);

    state.baseline_passed = Some(9);
    assert!(
        execute_acceptance_plan_for_test(&mut state, &second_command, &workspace).1,
        "a verifier configuration change must force a fresh process"
    );
    assert_eq!(state.last_failed_acceptance.as_ref().unwrap().attempts, 1);
    assert_eq!(
        std::fs::read_to_string(&sentinel).unwrap().lines().count(),
        4,
        "initial, changed-workspace, changed-command, and changed-config executions only"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ambiguous_workspace_fingerprint_never_reuses_red_acceptance() {
    let _lock = crate::tests::env_lock();
    let _threshold = crate::tests::TestEnvGuard::set("ANGEL_LOOP_VERIFY_THRESHOLD", "1");
    let root = std::env::temp_dir().join(format!(
        "angel-loop-red-memo-ambiguous-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let workspace = root.join("not-a-git-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    // Stop Git discovery before an enclosing worktree when TMPDIR is local.
    std::fs::write(workspace.join(".git"), "invalid fixture git marker\n").unwrap();
    let sentinel = workspace.join("gate-runs.txt");
    let command = red_sentinel_command(&sentinel, "ambiguous");
    let mut state = LoopState {
        workspace: Some(workspace.clone()),
        accept_cmd: Some(command.clone()),
        ..Default::default()
    };

    assert!(execute_acceptance_plan_for_test(&mut state, &command, &workspace).1);
    assert!(state.last_failed_acceptance.is_none());
    assert!(execute_acceptance_plan_for_test(&mut state, &command, &workspace).1);
    assert_eq!(
        std::fs::read_to_string(&sentinel).unwrap().lines().count(),
        2
    );

    let _ = std::fs::remove_dir_all(root);
}

// Real evaluator checks need a Git-backed candidate, not the live angelX
// checkout. Keep the fingerprint small and independent of concurrent work.
struct AcceptanceFixture(PathBuf, PathBuf);

impl AcceptanceFixture {
    fn new(tag: &str) -> Self {
        let (root, workspace) = acceptance_memo_workspace(tag);
        Self(root, workspace)
    }

    fn workspace(&self) -> &Path {
        &self.1
    }
}

impl Drop for AcceptanceFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn run_accept_cmd_judges_exit_status() {
    let fixture = AcceptanceFixture::new("exit-status");
    let ws = fixture.workspace();
    assert!(run_accept_cmd("exit 0", None, ws, "loop-test").passed);
    assert!(!run_accept_cmd("exit 3", None, ws, "loop-test").passed);
    assert!(!run_accept_cmd("false", None, ws, "loop-test").passed);
    assert!(run_accept_cmd("true", None, ws, "loop-test").passed);
}

#[test]
fn run_accept_cmd_scores_test_output_via_rlvr() {
    let fixture = AcceptanceFixture::new("rlvr");
    // A libtest summary is scored through evaluator-owned LEVI/reinforce
    // evidence: both the parsed result and the command exit must be green.
    let green = run_accept_cmd(
        "echo 'test result: ok. 5 passed; 0 failed; 0 ignored;'",
        None,
        fixture.workspace(),
        "loop-test",
    );
    assert!(
        green.passed,
        "all-green RLVR reward is a pass: {}",
        green.summary
    );
    assert!(green.summary.contains("RLVR reward"));

    let red = run_accept_cmd(
        "echo 'test result: FAILED. 3 passed; 2 failed; 0 ignored;'",
        None,
        fixture.workspace(),
        "loop-test",
    );
    assert!(!red.passed, "partial reward is not a pass: {}", red.summary);

    let forged_green = run_accept_cmd(
        "echo 'test result: ok. 5 passed; 0 failed; 0 ignored;'; exit 1",
        None,
        fixture.workspace(),
        "loop-test",
    );
    assert!(
        !forged_green.passed,
        "green-looking output from a failing verifier is rejected: {}",
        forged_green.summary
    );

    // Reward-hack guard: a "green" with zero executed tests is NOT a pass,
    // even though the reward parser might read it as 1.0.
    let empty = run_accept_cmd(
        "echo 'test result: ok. 0 passed; 0 failed; 0 ignored;'",
        None,
        fixture.workspace(),
        "loop-test",
    );
    assert!(
        !empty.passed,
        "zero-test green is rejected: {}",
        empty.summary
    );
}

#[test]
fn run_accept_cmd_rejects_count_regression() {
    let fixture = AcceptanceFixture::new("count-regression");
    // Baseline was 5 passing; a later all-green with only 3 passing means tests
    // were deleted/disabled to fake a pass → rejected even though reward == 1.0.
    let regressed = run_accept_cmd(
        "echo 'test result: ok. 3 passed; 0 failed; 0 ignored;'",
        Some(5),
        fixture.workspace(),
        "loop-test",
    );
    assert!(
        !regressed.passed,
        "pass-count regression is rejected: {}",
        regressed.summary
    );
    assert!(regressed.summary.contains("REJECTED"));
    // Same count as baseline (or more) with all green is accepted.
    let ok = run_accept_cmd(
        "echo 'test result: ok. 5 passed; 0 failed; 0 ignored;'",
        Some(5),
        fixture.workspace(),
        "loop-test",
    );
    assert!(ok.passed, "no regression → pass: {}", ok.summary);
}

#[test]
fn count_passed_parses_summary_in_dir() {
    assert_eq!(
        count_passed_in(
            "echo 'test result: ok. 7 passed; 0 failed; 0 ignored;'",
            Path::new(".")
        ),
        7
    );
    assert_eq!(count_passed_in("echo 'no tests here'", Path::new(".")), 0);
    // The baseline runs in the given dir — the same root verify uses.
    assert_eq!(
        count_passed_in(
            "[ -e Cargo.toml ] && echo 'test result: ok. 1 passed; 0 failed;'",
            Path::new(env!("CARGO_MANIFEST_DIR"))
        ),
        1
    );
}

#[cfg(target_os = "linux")]
#[test]
fn count_passed_drains_noisy_stderr_before_the_summary() {
    assert_eq!(
        count_passed_in(
            "i=0; while [ \"$i\" -lt 40000 ]; do printf 'noise-%05d-xxxxxxxxxxxxxxxxxxxxxxxx\\n' \"$i\" >&2; i=$((i + 1)); done; printf 'test result: ok. 9 passed; 0 failed; 0 ignored;\\n'",
            Path::new("."),
        ),
        9
    );
}

#[cfg(target_os = "linux")]
#[test]
fn count_passed_timeout_returns_zero_instead_of_sticking_baselining() {
    let _lock = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
    let _timeout = crate::tests::TestEnvGuard::set("ANGEL_TOOL_TIMEOUT", "1");
    let started = Instant::now();
    assert_eq!(count_passed_in("sleep 30", Path::new(".")), 0);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "a timed-out baseline must return control to the loop"
    );
}

#[test]
fn steer_notes_persist_and_default() {
    // Mid-run steers recorded on a loop must survive a crash/resume, and a
    // pre-steering state file (no field) must still load.
    let mut st = LoopState::configured_from_env();
    st.steer_notes.push("focus the parser".to_string());
    st.status = LoopStatus::Running;
    let json = serde_json::to_string(&st).unwrap();
    let back: LoopState = serde_json::from_str(&json).unwrap();
    assert_eq!(back.steer_notes, vec!["focus the parser".to_string()]);

    let v = serde_json::to_value(LoopState::default()).unwrap();
    assert!(
        v.get("steer_notes").is_none(),
        "empty notes are skipped on disk"
    );
    let legacy: LoopState = serde_json::from_value(v).unwrap();
    assert!(legacy.steer_notes.is_empty());
}

#[test]
fn acceptance_red_memo_is_backward_compatible() {
    let value = serde_json::to_value(LoopState::default()).unwrap();
    assert!(
        value.get("last_failed_acceptance").is_none(),
        "empty memo stays absent from the persisted schema"
    );
    let legacy: LoopState = serde_json::from_value(value).unwrap();
    assert!(legacy.last_failed_acceptance.is_none());
    assert!(legacy.pending_acceptance.is_none());
}

#[test]
fn deli_flag_persists_and_loads() {
    let mut st = LoopState::configured_from_env();
    st.deli = true;
    st.status = LoopStatus::Running;
    // serde roundtrip keeps the deli flag.
    let json = serde_json::to_string(&st).unwrap();
    let back: LoopState = serde_json::from_str(&json).unwrap();
    assert!(back.deli);
}

#[test]
fn self_run_fields_persist_and_default() {
    // A self run's identity (worktree branch, live root, prior workspace)
    // must survive a crash so /self integrate|discard still work after it.
    let mut st = LoopState::configured_from_env();
    st.self_edit = true;
    st.self_branch = Some("angel/self-42".to_string());
    st.self_root = Some(PathBuf::from("/repo/cockpit"));
    st.self_prev_workspace = Some(PathBuf::from("/home/user/project"));
    st.status = LoopStatus::Running;
    let json = serde_json::to_string(&st).unwrap();
    let back: LoopState = serde_json::from_str(&json).unwrap();
    assert!(back.self_edit);
    assert_eq!(back.self_branch.as_deref(), Some("angel/self-42"));
    assert_eq!(back.self_root.as_deref(), Some(Path::new("/repo/cockpit")));
    assert_eq!(
        back.self_prev_workspace.as_deref(),
        Some(Path::new("/home/user/project"))
    );
    // A pre-self-loop state file (all the old fields, none of the self ones)
    // still loads: the self fields default off.
    let mut v = serde_json::to_value(LoopState::default()).unwrap();
    let obj = v.as_object_mut().unwrap();
    obj.remove("self_edit");
    assert!(obj.get("self_branch").is_none(), "None options are skipped");
    let legacy: LoopState = serde_json::from_value(v).unwrap();
    assert!(!legacy.self_edit);
    assert!(legacy.self_branch.is_none());
}

#[test]
fn linked_worktree_state_survives_recreated_link_path() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-loop-worktree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let main = root.join("main");
    let linked = root.join("linked");
    std::fs::create_dir_all(&main).unwrap();
    let git = |cwd: &Path, args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&main, &["init", "-q"]);
    git(&main, &["config", "user.email", "angel@example.invalid"]);
    git(&main, &["config", "user.name", "Angel Test"]);
    std::fs::write(main.join("seed"), "one").unwrap();
    git(&main, &["add", "seed"]);
    git(&main, &["commit", "-q", "-m", "seed"]);
    git(&main, &["worktree", "add", "-q", linked.to_str().unwrap()]);

    let loop_file = root.join("loop.json");
    let prior = std::env::var_os("ANGEL_LOOP_FILE");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LOOP_FILE", &loop_file) };
    let mut state = LoopState {
        workspace: Some(linked.clone()),
        status: LoopStatus::Paused,
        ..LoopState::default()
    };
    state.steer_notes.push("retain identity".into());
    save(&state);
    assert!(
        load_for(&main).is_some(),
        "linked and main paths share a repository identity"
    );

    git(
        &main,
        &["worktree", "remove", "--force", linked.to_str().unwrap()],
    );
    std::fs::create_dir_all(&linked).unwrap();
    std::fs::write(linked.join("unrelated"), "replacement path").unwrap();
    let loaded = load_for(&main).expect("stored canonical identity survives path recreation");
    assert_eq!(loaded.steer_notes, vec!["retain identity"]);

    match prior {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var("ANGEL_LOOP_FILE", value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_LOOP_FILE") },
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn adventure_quest_walks_the_regions_end_to_end() {
    let mut app = crate::seed_preview_app();
    assert_eq!(
        app.world.quest().region(),
        crate::stage::world_viz::Region::CastleTown
    );

    // A podrace competition loop starts.
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "optimize the kernel benchmark and submit".into(),
        podrace: true,
        pivot: 1,
        ..Default::default()
    };
    app.adventure_mirror_drain();
    assert_eq!(
        app.world.quest().region(),
        crate::stage::world_viz::Region::TheMines
    );

    // Stalled iterations sink the quest into the swamp (pivot 1 → the
    // first stale count already reads level 3).
    for n in 1..=3 {
        app.loop_harvest(format!("DIRECTION: drift {n}"));
    }
    app.adventure_mirror_drain();
    assert_eq!(
        app.world.quest().region(),
        crate::stage::world_viz::Region::Swamp
    );

    // A measurement receipt shows activity but does not establish
    // an objective improvement from the coordinator's prose.
    app.loop_harvest_with_tools(
        "DIRECTION: tuned the pipeline — improved latency".into(),
        ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec!["measured:shell:./bench.sh --a:result=1".into()],
            ..Default::default()
        },
    );
    app.adventure_mirror_drain();
    assert_eq!(
        app.world.quest().region(),
        crate::stage::world_viz::Region::TheMines
    );
    assert_eq!(app.world.quest().treasures(), 0);

    // Submitting puts the hero in the Dragon Keep.
    app.loop_harvest_with_tools(
        "DIRECTION: submit the best candidate".into(),
        ToolStripSnapshot {
            calls: 1,
            verified_outcome_actions: vec!["submitted:shell:hilbert submit cand:result=2".into()],
            ..Default::default()
        },
    );
    app.adventure_mirror_drain();
    assert_eq!(
        app.world.quest().region(),
        crate::stage::world_viz::Region::DragonKeep
    );

    // The run finishes ok — loot walks home, then idles back in town.
    app.loop_finish(LoopStatus::Done, "findings goal met");
    app.adventure_mirror_drain();
    assert_eq!(
        app.world.quest().region(),
        crate::stage::world_viz::Region::Homecoming
    );
    for _ in 0..120 {
        app.world.tick();
    }
    assert_eq!(
        app.world.quest().region(),
        crate::stage::world_viz::Region::CastleTown
    );
}
