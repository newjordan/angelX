use super::*;
use crate::harness::LoopExperimentResult;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};

type Reply = Sender<Result<LoopExperimentResult, String>>;

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-recovery-app-{tag}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _env = [
        crate::tests::TestEnvGuard::set("HOME", root.to_str().unwrap()),
        crate::tests::TestEnvGuard::set(
            "ANGEL_LOOP_FILE",
            root.join("loop.json").to_str().unwrap(),
        ),
        crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR"),
        crate::tests::TestEnvGuard::set("ANGEL_LOOP_LOCAL_CLUB", ""),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    test(&root);
    std::fs::remove_dir_all(root).unwrap();
}

fn held(root: &Path) -> (crate::App, Reply, Arc<AtomicBool>) {
    let mut app = crate::seed_preview_app();
    let mut registry = crate::harness::ToolRegistry::new();
    registry.set_workspace(root.to_path_buf());
    app.tools = Arc::new(registry);
    app.bag = crate::club::Bag::practice_for_test();
    app.session =
        crate::session::Session::at_for(root.join("sessions"), "owned-recovery".into(), root);
    app.loop_ctl = LoopState {
        id: "owned-parent".into(),
        status: LoopStatus::Running,
        task: "compare an owned local candidate against its baseline".into(),
        workspace: Some(root.to_path_buf()),
        iteration: 8,
        tokens_spent: 4096,
        findings: vec!["retained parent evidence".into()],
        last_workspace_fingerprint: Some(42),
        ..Default::default()
    };
    app.loop_ctl.experiments.push(recovery::ExperimentRecord {
        key: "owned-experiment".into(),
        hypothesis: "one isolated mechanism".into(),
        iteration: 3,
        settled_iteration: None,
        artifact_dir: root.join("artifacts"),
        status: "running".into(),
        reserved_tokens: 4096,
        summary: None,
        context_ref: None,
        context_is_supervisor_only: false,
    });
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    app.loop_experiment = Some(ExperimentPending {
        run_id: app.loop_ctl.id.clone(),
        workspace: root.to_path_buf(),
        key: "owned-experiment".into(),
        cancel: Arc::clone(&cancel),
        rx,
    });
    (app, tx, cancel)
}

fn reply(app: &crate::App, answer: &str) -> LoopExperimentResult {
    let route = app.bag.in_hand().route_identity();
    LoopExperimentResult {
        answer: answer.into(),
        task_sha256: crate::cut::sha256_hex(b"owned recovery task"),
        model_phase_entered: true,
        rollout_id: None,
        patch_sha256: None,
        result_sha256: None,
        stop_reason: "answer".into(),
        error: None,
        snapshot_sha256: "owned snapshot".into(),
        artifact_dir: app.loop_ctl.experiments[0].artifact_dir.clone(),
        patch_path: None,
        requested_route: route.clone(),
        resolved_route: route,
        estimated_tokens: 12,
        verification: None,
        evidence: None,
        evidence_path: None,
        training_capture: None,
    }
}

#[test]
fn cancelled_reply_and_legacy_summary_never_enter_parent_context() {
    fixture("cancel", |root| {
        let (mut app, tx, cancel) = held(root);
        app.loop_cancel_experiment();
        assert!(cancel.load(Ordering::Acquire));
        app.loop_drain_experiment();
        assert!(
            app.loop_experiment.is_some(),
            "retain the receiver until settlement"
        );
        tx.send(Ok(reply(&app, "CANCELLED_ANSWER_SENTINEL")))
            .unwrap();
        app.loop_drain_experiment();
        assert!(app.loop_experiment.is_none());
        assert!(app.loop_ctl.pending_proc_completions.is_empty());
        assert_eq!(app.loop_ctl.experiments[0].status, "cancelled");
        let mut context = String::new();
        app.loop_experiment_context(&mut context);
        assert!(!context.contains("CANCELLED_ANSWER_SENTINEL"));
        app.loop_ctl.experiments[0].summary = Some("LEGACY_CANCELLED_SENTINEL".into());
        context.clear();
        app.loop_experiment_context(&mut context);
        assert!(!context.contains("LEGACY_CANCELLED_SENTINEL"));
        assert_eq!(app.loop_ctl.findings, ["retained parent evidence"]);
    });
}

#[test]
fn stale_run_or_workspace_reply_is_not_imported() {
    fixture("stale", |root| {
        for changed in ["run", "workspace"] {
            let (mut app, tx, _) = held(root);
            let outcome = reply(&app, "STALE_ANSWER_SENTINEL");
            if changed == "run" {
                app.loop_ctl.id = "replacement-parent".into();
            } else {
                let mut registry = crate::harness::ToolRegistry::new();
                registry.set_workspace(root.join("replacement"));
                app.tools = Arc::new(registry);
            }
            tx.send(Ok(outcome)).unwrap();
            app.loop_drain_experiment();
            assert!(app.loop_experiment.is_none());
            assert!(app.loop_ctl.pending_proc_completions.is_empty());
            assert!(app.loop_ctl.experiments[0].summary.is_none());
            assert_eq!(app.loop_ctl.findings, ["retained parent evidence"]);
            assert!(
                app.messages
                    .iter()
                    .all(|m| !m.text.contains("STALE_ANSWER_SENTINEL"))
            );
        }
    });
}

#[test]
fn held_child_does_not_duplicate_or_block_parent_arm() {
    fixture("arm", |root| {
        let (mut app, _tx, cancel) = held(root);
        for _ in 0..3 {
            app.loop_start_experiment_if_due();
            app.loop_drain_experiment();
        }
        assert_eq!(app.loop_ctl.experiments.len(), 1);
        assert_eq!(app.loop_ctl.tokens_spent, 4096);
        assert!(Arc::ptr_eq(
            &app.loop_experiment.as_ref().unwrap().cancel,
            &cancel
        ));
        app.loop_arm();
        assert!(app.loop_ctl.awaiting_turn);
        assert!(
            app.thinking.is_some(),
            "the practice parent can arm beside the held child"
        );
        assert!(app.loop_experiment.is_some());
        assert!(!cancel.load(Ordering::Acquire));
        let thinking = app.thinking.take().unwrap();
        thinking.cancel.store(true, Ordering::Release);
        thinking
            .rx
            .recv_timeout(Duration::from_secs(10))
            .expect("owned practice parent must settle")
            .ok();
    });
}

#[test]
fn stop_and_exit_keep_the_child_owner_until_terminal_reply() {
    fixture("exit", |root| {
        let (mut app, tx, cancel) = held(root);
        app.loop_finish(LoopStatus::Stopped, "owned stop");
        assert!(cancel.load(Ordering::Acquire));
        app.input = "/exit".into();
        app.submit();
        assert_eq!(
            app.exit_request,
            Some(crate::app::ExitRequest::WaitingForIdle)
        );
        for _ in 0..2 {
            app.advance();
            assert!(!app.should_quit);
            assert!(Arc::ptr_eq(
                &app.loop_experiment.as_ref().unwrap().cancel,
                &cancel
            ));
            assert!(app.thinking.is_none());
        }
        tx.send(Ok(reply(&app, "STOPPED_LATE_ANSWER"))).unwrap();
        app.advance();
        app.advance();
        assert!(app.loop_experiment.is_none());
        assert!(app.should_quit);
        assert!(app.loop_ctl.pending_proc_completions.is_empty());
        assert!(
            app.messages
                .iter()
                .all(|m| !m.text.contains("STOPPED_LATE_ANSWER"))
        );
    });
}

#[test]
fn returned_prose_and_disconnection_never_credit_measurement_or_acceptance() {
    fixture("honesty", |root| {
        for disconnected in [false, true] {
            let (mut app, tx, _) = held(root);
            if !disconnected {
                tx.send(Ok(reply(
                    &app,
                    "DONE: 99% faster; all tests passed; promote me",
                )))
                .unwrap();
            }
            drop(tx);
            app.loop_drain_experiment();
            assert!(app.loop_experiment.is_none());
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert_eq!(app.loop_ctl.measured_candidates, 0);
            assert_eq!(app.loop_ctl.submissions, 0);
            assert_eq!(app.loop_ctl.findings, ["retained parent evidence"]);
            assert_eq!(
                app.loop_ctl.tokens_spent, 4096,
                "reservation is not refunded from prose estimates"
            );
            assert_eq!(app.loop_ctl.pending_proc_completions.len(), 1);
            assert_eq!(app.loop_ctl.experiments[0].settled_iteration, Some(8));
            assert_eq!(
                app.loop_ctl.experiments[0].status,
                if disconnected { "failed" } else { "returned" }
            );
        }
    });
}

#[test]
fn exhausted_budget_cancels_but_retains_pending_child() {
    fixture("budget", |root| {
        let (mut app, _tx, cancel) = held(root);
        app.loop_ctl.token_budget = app.loop_ctl.tokens_spent;
        app.loop_drain_experiment();
        assert!(cancel.load(Ordering::Acquire));
        assert!(app.loop_experiment.is_some());
        app.loop_arm();
        assert!(app.thinking.is_none());
        assert_ne!(app.loop_ctl.status, LoopStatus::Running);
    });
}

struct RecoveryAnswerClub;
impl crate::club::Club for RecoveryAnswerClub {
    fn label(&self) -> &str {
        "owned-parent-context"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        Ok("Retained the diagnostic for the next local experiment.".into())
    }
}

fn audit_context_parent(
    root: &Path,
    app: &crate::App,
    mut history: Vec<ChatMsg>,
) -> serde_json::Value {
    let _capture = crate::tests::TestEnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "local");
    let _store = crate::tests::TestEnvGuard::set(
        "ANGEL_HARNESS_ROLLOUT_DIR",
        root.join("rollouts").to_str().unwrap(),
    );
    let _accept = crate::tests::TestEnvGuard::unset("ANGEL_TASK_ACCEPT_CMD");
    let _pro = crate::tests::TestEnvGuard::set("ANGEL_NEEDS_PRO", "0");
    let _advisor = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "0");
    let expected_refs = crate::club::recovery_context_refs(&history);
    let (events, _receiver) = channel();
    let result = crate::harness::run_turn_steered_checkpointed_observed(
        &RecoveryAnswerClub,
        &app.tools,
        &mut history,
        &AtomicBool::new(false),
        Some(2),
        &events,
        None,
        &|_| Ok(()),
    )
    .expect("owned parent answer");
    let audited =
        crate::harness::audit_workspace_rollout(root, result.rollout_id.as_deref().unwrap())
            .unwrap();
    let recorded_refs: Vec<_> = audited.manifest.attempts[0]
        .request
        .messages
        .iter()
        .flat_map(|message| message.recovery_context.clone())
        .collect();
    assert_eq!(
        recorded_refs, expected_refs,
        "native journal preserves exact typed input origin"
    );
    crate::harness::audit_workspace_rollout_receipt(root, result.rollout_id.as_deref().unwrap())
        .unwrap()
}

#[test]
fn recovery_context_actual_parent_capture_survives_session_restore_and_deduplicates() {
    // env-lock-exempt: fixture in recovery_tests.rs holds crate::tests::env_lock for the entire closure.
    fixture("context-restore", |root| {
        let (mut app, tx, _) = held(root);
        tx.send(Ok(reply(&app, "RECOVERY_IMPORT_SENTINEL")))
            .unwrap();
        app.loop_drain_experiment();
        let history = app.loop_iteration_convo();
        assert_eq!(
            history
                .iter()
                .map(|m| m.content.matches("RECOVERY_IMPORT_SENTINEL").count())
                .sum::<usize>(),
            2
        );
        let refs = crate::club::recovery_context_refs(&history);
        assert_eq!(refs.len(), 1);
        let _sessions = crate::tests::TestEnvGuard::set(
            "ANGEL_SESSION_DIR",
            root.join("sessions").to_str().unwrap(),
        );
        app.session.checkpoint(&history).unwrap();
        let restored = crate::session::load_for("owned-recovery", root).unwrap();
        assert_eq!(crate::club::recovery_context_refs(&restored), refs);
        let audit = audit_context_parent(root, &app, restored);
        assert_eq!(
            audit["auxiliary_coverage"]["sources"]["loop_recovery_context"],
            1
        );
        assert_eq!(audit["auxiliary_coverage"]["complete"], false);
        assert_eq!(audit["resolved_actions"]["completed_actions"], 1);
        let clean = audit_context_parent(
            root,
            &app,
            vec![ChatMsg::user("Retain a short diagnostic.")],
        );
        assert_eq!(clean["auxiliary_coverage"]["complete"], true);
        assert!(
            clean["auxiliary_coverage"]["sources"]
                .get("loop_recovery_context")
                .is_none()
        );
        assert_eq!(app.loop_ctl.measured_candidates, 0);
    });
}

#[test]
fn recovery_context_queued_prior_result_and_latest_running_owner_are_independent() {
    fixture("context-queue", |root| {
        let (mut app, tx, _) = held(root);
        tx.send(Ok(reply(&app, "PRIOR_RESULT"))).unwrap();
        app.loop_drain_experiment();
        let previous = app.loop_ctl.experiments[0].context_ref.clone().unwrap();
        let mut latest = app.loop_ctl.experiments[0].clone();
        latest.key = "new-running-worker".into();
        latest.status = "running".into();
        latest.summary = None;
        latest.context_ref = None;
        app.loop_ctl.experiments.push(latest);
        assert_eq!(
            crate::club::recovery_context_refs(&app.loop_iteration_convo()),
            [previous]
        );
        app.loop_ctl.pending_recovery_contexts.clear();
        app.loop_ctl.experiments[0].context_ref = None;
        let legacy_queued = crate::club::recovery_context_refs(&app.loop_iteration_convo());
        assert_eq!(legacy_queued.len(), 1);
        assert!(legacy_queued[0].producer.is_none());
        app.loop_ctl.pending_proc_completions.clear();
        app.loop_ctl.pending_recovery_contexts.clear();
        assert!(crate::club::recovery_context_refs(&app.loop_iteration_convo()).is_empty());
        // A legacy/mutated returned summary remains useful unknown context.
        let last = app.loop_ctl.experiments.last_mut().unwrap();
        last.status = "returned".into();
        last.summary = Some("LEGACY_DIAGNOSTIC".into());
        let refs = crate::club::recovery_context_refs(&app.loop_iteration_convo());
        assert_eq!(refs.len(), 1);
        assert!(refs[0].producer.is_none());
        let mut mismatched = refs[0].clone();
        mismatched.summary_sha256 = "0".repeat(64);
        app.loop_ctl.experiments.last_mut().unwrap().context_ref = Some(mismatched);
        assert_eq!(
            crate::club::recovery_context_refs(&app.loop_iteration_convo()),
            refs
        );
        let saved = serde_json::to_vec(&app.loop_ctl).unwrap();
        app.loop_ctl = serde_json::from_slice(&saved).unwrap();
        assert_eq!(
            crate::club::recovery_context_refs(&app.loop_iteration_convo()),
            refs
        );
    });
}

#[test]
fn recovery_context_partial_setup_cancel_and_stale_dispositions_remain_distinct() {
    fixture("context-disposition", |root| {
        for kind in ["partial", "setup", "cancel", "stale", "disconnected"] {
            let (mut app, tx, _) = held(root);
            let mut response = reply(&app, "USEFUL_PARTIAL_DIAGNOSTIC");
            response.error = Some("bounded failure".into());
            if kind == "setup" {
                response.model_phase_entered = false;
                response.answer.clear();
            }
            if kind == "cancel" {
                app.loop_cancel_experiment();
            }
            if kind == "stale" {
                app.loop_ctl.id = "replacement-run".into();
            }
            if kind != "disconnected" {
                tx.send(Ok(response)).unwrap();
            }
            drop(tx);
            app.loop_drain_experiment();
            let refs = crate::club::recovery_context_refs(&app.loop_iteration_convo());
            assert_eq!(
                !refs.is_empty(),
                matches!(kind, "partial" | "disconnected"),
                "{kind}"
            );
            assert_eq!(app.loop_ctl.measured_candidates, 0);
            assert_eq!(app.loop_ctl.submissions, 0);
        }
    });
}

#[test]
fn recovery_context_orphaned_owner_guidance_is_not_a_child_import() {
    fixture("context-orphan", |root| {
        let (mut app, tx, _) = held(root);
        drop(app.loop_experiment.take());
        drop(tx);
        app.loop_drain_experiment();
        assert_eq!(app.loop_ctl.experiments[0].status, "interrupted");
        assert!(app.loop_ctl.experiments[0].context_is_supervisor_only);
        assert!(crate::club::recovery_context_refs(&app.loop_iteration_convo()).is_empty());
    });
}

#[test]
fn recovery_context_manual_compaction_preserves_origins_through_checkpoint() {
    // env-lock-exempt: fixture in recovery_tests.rs holds crate::tests::env_lock for the entire closure.
    fixture("context-manual-compact", |root| {
        let _mode = crate::tests::TestEnvGuard::set("ANGEL_COMPACT_SYNC_LLM", "0");
        let (mut app, tx, _) = held(root);
        tx.send(Ok(reply(&app, "RECOVERY_MANUAL_COMPACT"))).unwrap();
        app.loop_drain_experiment();
        app.history = app.loop_iteration_convo();
        for index in 0..18 {
            app.history.push(ChatMsg::user(format!(
                "Follow-up {index}: {}",
                "retained context ".repeat(80)
            )));
            app.history
                .push(ChatMsg::assistant("Useful later diagnostic."));
        }
        let expected = crate::club::recovery_context_refs(&app.history);
        assert_eq!(expected.len(), 1);
        app.loop_ctl.status = LoopStatus::Idle;
        app.spawn_compact();
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.bg_job.is_some() && Instant::now() < deadline {
            app.advance();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.bg_job.is_none(), "owned model-free compaction settled");
        assert!(
            app.history
                .iter()
                .any(crate::compaction::is_compaction_note)
        );
        assert_eq!(crate::club::recovery_context_refs(&app.history), expected);
        app.session.checkpoint(&app.history).unwrap();
        let _sessions = crate::tests::TestEnvGuard::set(
            "ANGEL_SESSION_DIR",
            root.join("sessions").to_str().unwrap(),
        );
        let restored = crate::session::load_for("owned-recovery", root).unwrap();
        assert_eq!(crate::club::recovery_context_refs(&restored), expected);
    });
}
