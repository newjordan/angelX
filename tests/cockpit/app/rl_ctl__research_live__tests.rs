use super::*;

fn wait_for(live: &LiveResearch, count: u64) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let out = live.output.lock().unwrap();
        if let Some(error) = &out.error {
            panic!("{error}");
        }
        if out.advice["relation_count"].as_u64() == Some(count) {
            return;
        }
        assert!(Instant::now() < until, "live observer did not publish");
        drop(out);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn live_sloptomizer_persists_relationships_and_restores_them_on_another_model() {
    let _guard = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_SLOPTOMIZER_LIVE", "1");
    let root = std::env::temp_dir().join(format!("angel-live-{}", new_run_id()));
    let _dir = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.to_str().unwrap());
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let (send, _) = mpsc::channel();
    let mut live = LiveResearch::new(&workspace, "fixture objective", None, "fixture-a", send);
    let mut call = ToolCall {
        id: "1".into(),
        name: "run_tests".into(),
        args: json!({"command":"sensitive argument"}),
    };
    let mut route = RouteIdentity {
        driver: "fixture".into(),
        model: Some("model-a".into()),
        ..Default::default()
    };
    live.observe(
        &call,
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::Failed,
        },
        "red receipt",
        "red → boundary missing → run edges → green",
        &route,
    );
    live.flush();
    wait_for(&live, 1);
    let state_path = live.root.join("state.json");
    drop(live);
    let (send, _) = mpsc::channel();
    let mut restored = LiveResearch::new(&workspace, "fixture objective", None, "fixture-b", send);
    wait_for(&restored, 1);
    call.id = "2".into();
    route.model = Some("model-b".into());
    restored.observe(
        &call,
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::Passed,
        },
        "green receipt",
        "red → boundary missing → run edges → green",
        &route,
    );
    restored.flush();
    wait_for(&restored, 2);
    let mut history = vec![ChatMsg::user("fixture objective")];
    restored.poll(&workspace, &mut history, false);
    assert!(history.last().unwrap().content.starts_with("⚠⡫⠁⡫⠉"));
    let state: Value = serde_json::from_slice(&bridge::read(&state_path).unwrap()).unwrap();
    assert_eq!(state["relations"].as_array().unwrap().len(), 2);
    assert_eq!(state["relations"][0]["session"], "fixture-a");
    assert_eq!(state["relations"][1]["session"], "fixture-b");
    assert_eq!(state["observations"], json!([]));
    assert!(state["learner"].is_null());
    assert!(!state.to_string().contains("sensitive argument"));
    drop(restored);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn caveman_relations_are_optional_bounded_and_preserve_unicode() {
    let frame =
        caveman_frame("2 fail → boundary unchecked → run edge case → expect raise").unwrap();
    assert_eq!(frame[1], "boundary unchecked");
    assert!(caveman_frame("just run the experiment").is_none());
    assert!(caveman_frame("a → b → c").is_none());
    assert!(caveman_frame("a → b → c → d → e").is_none());
    assert_eq!(
        caveman_frame(&format!("x → {} → check → expect", "🦀".repeat(400))).unwrap()[1]
            .chars()
            .count(),
        240
    );
}

#[test]
fn different_failure_receipts_raise_a_decodable_verdict_run_without_claiming_identity() {
    let _guard = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_SLOPTOMIZER_LIVE", "1");
    let root = std::env::temp_dir().join(format!("angel-live-{}", new_run_id()));
    let _dir = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.to_str().unwrap());
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let (send, _) = mpsc::channel();
    let mut live = LiveResearch::new(&workspace, "fixture", None, "fixture", send);
    let route = RouteIdentity {
        driver: "fixture".into(),
        model: Some("glm".into()),
        ..Default::default()
    };
    for n in 1..=4 {
        live.observe(
            &ToolCall {
                id: n.to_string(),
                name: "run_tests".into(),
                args: json!({}),
            },
            ToolOutcome {
                execution: ExecutionOutcome::Failed,
                verification: VerificationOutcome::Failed,
            },
            &format!("failure {n}"),
            "",
            &route,
        );
    }
    live.flush();
    wait_for(&live, 4);
    let mut history = vec![ChatMsg::user("fixture")];
    live.poll(&workspace, &mut history, false);
    let note = &history.last().unwrap().content;
    assert!(note.starts_with("⚠⡫⠁⡫⠛"));
    let data: Value = serde_json::from_str(note.split_once('\n').unwrap().1).unwrap();
    assert_eq!(data["checks"][0]["run"], 4);
    assert_eq!(data["checks"][0]["same"], 1);
    let definitions = book::introduction::introductions(&history, None);
    assert!(
        definitions
            .last()
            .unwrap()
            .1
            .contains("changing failures may show progress")
    );
    live.poll(&workspace, &mut history, false);
    assert_eq!(
        history.len(),
        2,
        "one notice without a model call or forced pivot"
    );
    drop(live);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn model_free_observer_admits_only_executed_typed_checks() {
    let _guard = crate::tests::env_lock();
    let _enabled = crate::tests::TestEnvGuard::set("ANGEL_SLOPTOMIZER_LIVE", "1");
    let root = std::env::temp_dir().join(format!("angel-live-{}", new_run_id()));
    let _dir = crate::tests::TestEnvGuard::set("ANGEL_RL_DIR", root.to_str().unwrap());
    let (send, _) = mpsc::channel();
    let mut live = LiveResearch::new(&root, "fixture", None, "fixture", send);
    let call = ToolCall {
        id: "1".into(),
        name: "shell".into(),
        args: json!({"command":"private fixture"}),
    };
    let route = RouteIdentity {
        driver: "fixture".into(),
        ..Default::default()
    };
    for execution in [
        ExecutionOutcome::NotStarted,
        ExecutionOutcome::Denied,
        ExecutionOutcome::Cancelled,
    ] {
        live.observe(
            &call,
            ToolOutcome {
                execution,
                verification: VerificationOutcome::Passed,
            },
            "not proof",
            "",
            &route,
        );
    }
    live.observe(
        &call,
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::NotApplicable,
        },
        "poll complete",
        "",
        &route,
    );
    assert!(live.pending.is_empty());
    live.observe(
        &call,
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::Inconclusive,
        },
        "exit zero",
        "x → hypothesis → check → expect",
        &route,
    );
    assert_eq!(live.pending.len(), 1);
    assert_eq!(live.pending[0]["verdict"], "inconclusive");
    assert_eq!(live.pending[0]["hypothesis"], "hypothesis");
    assert!(!live.pending[0].to_string().contains("private fixture"));
    assert_eq!(live.pending[0]["receipt_excerpt"], "exit zero");
    assert_ne!(live.pending[0]["receipt"], "exit zero");
    live.observe(
        &call,
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::Inconclusive,
        },
        "exit zero",
        "x → hypothesis → check → expect",
        &route,
    );
    assert_ne!(
        live.pending[0]["id"], live.pending[1]["id"],
        "reused provider call IDs still describe separate executions"
    );
    // Pure admission test: avoid starting the optional worker during Drop.
    live.pending.clear();
}

#[test]
fn excerpts_redact_before_truncation_and_slow_consumers_never_hold_turn_exit() {
    assert_eq!(
        excerpt("API_KEY=never-retain-this\nassertion failed", true),
        "«redacted»\nassertion failed"
    );
    assert_eq!(excerpt(&"🦀".repeat(500), false).chars().count(), 240);
    let frame = caveman_frame(&format!(
        "observed → {}API_KEY=never-retain-this → check → expect",
        "x".repeat(238)
    ))
    .unwrap();
    assert_eq!(
        frame[1], "«redacted»",
        "redact the whole claim before cutting it"
    );
    let root = PathBuf::from("/fixture/slow-live-consumer");
    let (events, receive_events) = mpsc::channel();
    let mut live = LiveResearch::new(&root, "fixture", None, "fixture", events);
    let (queue, _blocked_consumer) = mpsc::sync_channel(1);
    queue.try_send(Vec::new()).unwrap();
    live.send = Some(queue);
    live.pending = (0..300).map(|i| json!({"id":i})).collect();
    live.flush();
    assert_eq!(live.pending.len(), 256);
    assert!(live.output.lock().unwrap().error.is_some());
    drop(live);
    assert!(
        matches!(
            receive_events.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ),
        "background memory cannot retain a turn's event sender"
    );
}

#[test]
fn later_quiet_updates_cannot_swallow_a_reactive_warning() {
    let mut output = Output::default();
    let root = Path::new("/fixture/publish");
    output.publish(
        json!({"relation_count":2,"signals":[{"kind":"contrast"}],"checks":[]}),
        root,
    );
    output.publish(json!({"relation_count":3,"signals":[],"checks":[]}), root);
    assert_eq!(output.advice["relation_count"], 3);
    assert_eq!(
        output.pending_notice.as_ref().unwrap()["signals"][0]["kind"],
        "contrast"
    );
}

#[test]
fn steady_evidence_is_quiet_but_compaction_restores_current_context() {
    let root = PathBuf::from("/nonexistent/angel-live-poll-fixture");
    let (send, _) = mpsc::channel();
    let mut live = LiveResearch::new(&root, "fixture", None, "fixture", send);
    {
        let mut out = live.output.lock().unwrap();
        out.generation = 1;
        out.advice = json!({"relation_count":1,"checks":[],"signals":[]});
    }
    let mut history = vec![ChatMsg::user("task")];
    live.poll(&root, &mut history, false);
    assert_eq!(history.len(), 2);
    live.poll(&root, &mut history, false);
    assert_eq!(history.len(), 2, "no repeated cue or model call");
    // Recreating the observer at the next turn does not spend the same card
    // again, including when it originally arrived with a historical warning.
    history[1].content = history[1].content.replacen("⚠⡫⠁", "⚠⡫⠁⡫⠉", 1).into();
    let (send, _) = mpsc::channel();
    let mut next = LiveResearch::new(&root, "fixture", None, "fixture", send);
    {
        let mut out = next.output.lock().unwrap();
        out.generation = 1;
        out.advice = json!({"relation_count":1,"checks":[],"signals":[]});
    }
    next.poll(&root, &mut history, false);
    assert_eq!(
        history.len(),
        2,
        "unchanged context survives turn boundaries"
    );
    history.truncate(1);
    live.poll(&root, &mut history, false);
    assert_eq!(history.len(), 2);
    live.poll(&root, &mut history, true);
    assert_eq!(history.len(), 3, "new model receives current evidence");
}
