use super::*;

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-loop-autonomy-{tag}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    test(&root);
    std::fs::remove_dir_all(root).unwrap();
}

fn running(root: &Path) -> LoopState {
    LoopState {
        status: LoopStatus::Running,
        task: "improve Swift candidate and preserve the evidence".into(),
        workspace: Some(root.to_path_buf()),
        podrace: true,
        first_candidate_iters: 3,
        stall_stop: 2,
        findings: vec!["operator sentinel".into()],
        ..Default::default()
    }
}

#[test]
fn three_iteration_swift_receipt_gap_steers_without_claiming_no_execution() {
    fixture("swift", |root| {
        for tier in [
            EscalationTier::Local,
            EscalationTier::Swarm,
            EscalationTier::Sota,
        ] {
            let mut app = crate::seed_preview_app();
            let model = app.bag.in_hand_with_fallback().label().to_string();
            app.loop_ctl = running(root);
            app.loop_ctl.tier = tier;
            for i in 1..=5 {
                app.loop_harvest_with_tools(
                    format!("DIRECTION: Swift check {i}"),
                    ToolStripSnapshot {
                        calls: 1,
                        outcome_actions: vec![format!(
                            "outcome:shell:swift test and time candidate {i}"
                        )],
                        ..Default::default()
                    },
                );
                assert_eq!(app.loop_ctl.status, LoopStatus::Running);
                assert_eq!(app.loop_ctl.tier, tier);
                assert!(app.loop_ctl.wake_at.is_some());
                assert!(app.loop_pending.is_none() && app.pending_approval.is_none());
            }
            assert_eq!(
                app.loop_ctl.measured_candidates, 0,
                "no synthetic verification credit"
            );
            assert_eq!(app.loop_ctl.tool_calls_total, 5);
            assert_eq!(app.loop_ctl.findings, ["operator sentinel"]);
            assert_eq!(app.bag.in_hand_with_fallback().label(), model);
            let prompt = app
                .loop_iteration_convo()
                .pop()
                .unwrap()
                .content
                .to_string();
            assert!(prompt.contains("[loop note — information, not an order]"));
            assert!(prompt.contains("no measured candidate yet after"));
            assert!(
                !app.messages
                    .iter()
                    .any(|m| m.text.contains("no benchmark or verify command was run"))
            );
        }
    });
}

#[test]
fn all_real_budgets_take_precedence_over_watchdog_and_blocker_continuation() {
    fixture("budget", |root| {
        for budget in ["iterations", "tokens", "deadline"] {
            for ending in ["reply", "provider", "execution"] {
                let mut app = crate::seed_preview_app();
                app.loop_ctl = running(root);
                app.loop_ctl.iteration = 2;
                match budget {
                    "iterations" => app.loop_ctl.max_iters = 3,
                    "tokens" => {
                        app.loop_ctl.token_budget = 1;
                        app.loop_ctl.tokens_spent = 1;
                    }
                    _ => {
                        app.loop_ctl.deadline_secs = 1;
                        app.loop_ctl.started_ms = now_ms().saturating_sub(2_000);
                    }
                }
                match ending {
                    "reply" => app.loop_harvest("DIRECTION: another check".into()),
                    "provider" => app.loop_harvest_error_with_tools(
                        "HTTP 401: Unauthorized".into(),
                        ToolStripSnapshot::default(),
                    ),
                    _ => {
                        crate::agent::harness::exec::set_sandbox_receipt(Some(serde_json::json!({
                            "helper_error": "Landlock unavailable; run angel --doctor",
                            "helper_phase": "landlock",
                            "helper_exit": 1,
                        })));
                        let error = crate::agent::harness::execution_blocker(
                            "shell",
                            "tool error: shell command failed (exit 1)\nbwrap: setting up uid map: Permission denied",
                        )
                        .unwrap();
                        app.loop_harvest_error_with_tools(error, ToolStripSnapshot::default());
                    }
                }
                assert_eq!(app.loop_ctl.status, LoopStatus::Paused, "{budget}/{ending}");
                assert!(app.loop_ctl.wake_at.is_none());
                assert!(app.messages.last().unwrap().text.contains("budget reached"));
                app.loop_arm();
                assert!(app.thinking.is_none());
            }
        }
    });
}

#[test]
fn repeated_red_acceptance_preserves_failure_and_selected_route_until_real_budget() {
    fixture("red", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = running(root);
        app.loop_ctl.podrace = false;
        app.loop_ctl.tier = EscalationTier::Sota;
        app.loop_ctl.stall_stop = 1;
        for i in 1..=5 {
            app.loop_ctl.status = LoopStatus::Verifying;
            app.loop_apply_verify_result(
                VerifyResult {
                    passed: false,
                    summary: "red".into(),
                    detail: "exact failing predicate".into(),
                },
                i > 1,
            );
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert_eq!(app.loop_ctl.tier, EscalationTier::Sota);
            assert_eq!(app.loop_ctl.stale_count, i);
            assert!(
                app.loop_ctl
                    .last_setback
                    .as_deref()
                    .unwrap()
                    .contains("exact failing predicate")
            );
            assert!(app.loop_pending.is_none() && app.pending_approval.is_none());
            assert!(app.loop_ctl.wake_at.is_some());
        }
        app.loop_ctl.max_iters = 1;
        app.loop_ctl.iteration = 1;
        app.loop_apply_verify_result(
            VerifyResult {
                passed: false,
                summary: "red".into(),
                detail: "exact failing predicate".into(),
            },
            true,
        );
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert!(
            app.loop_ctl
                .last_setback
                .as_deref()
                .unwrap()
                .contains("exact failing predicate")
        );
    });
}

#[test]
fn blocked_retry_respects_pacing_manual_pause_stop_and_interrupt() {
    fixture("manual", |root| {
        for command in ["pause", "stop", "interrupt"] {
            let mut app = crate::seed_preview_app();
            app.loop_ctl = running(root);
            app.loop_ctl.iteration = 2; // provider error coincides with the watchdog threshold
            let before = Instant::now();
            app.loop_harvest_error_with_tools(
                "HTTP 402: billing not configured".into(),
                ToolStripSnapshot::default(),
            );
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert!(app.loop_ctl.wake_at.unwrap() >= before + Duration::from_secs(60));
            app.loop_arm();
            assert!(app.thinking.is_none(), "the retry must not arm immediately");
            assert_eq!(
                app.loop_ctl.last_error.as_deref(),
                Some("HTTP 402: billing not configured")
            );
            let restored = load_from_path(root.join("loop.json")).unwrap();
            assert!(restored.wake_at.unwrap() > Instant::now() + Duration::from_secs(59));
            if command == "interrupt" {
                app.loop_on_interrupt();
            } else {
                app.loop_command(Some(command.into()));
            }
            let expected = if command == "stop" {
                LoopStatus::Stopped
            } else {
                LoopStatus::Paused
            };
            assert_eq!(app.loop_ctl.status, expected);
            assert!(app.loop_ctl.wake_at.is_none());
            app.loop_arm();
            assert!(app.thinking.is_none());
            let saved = load_from_path(root.join("loop.json")).unwrap();
            assert_eq!(saved.status, expected);
            assert!(
                saved.wake_at.is_none(),
                "stale blocker text must not override manual intent"
            );
        }
    });
}

#[test]
fn explicit_resume_recovers_terminal_states_without_resetting_task_evidence_or_caps() {
    fixture("resume", |root| {
        for status in [LoopStatus::Stopped, LoopStatus::Failed, LoopStatus::Paused] {
            let mut app = crate::seed_preview_app();
            app.loop_ctl = running(root);
            app.loop_ctl.status = status;
            app.loop_ctl.iteration = 3;
            app.loop_ctl.max_iters = 4;
            app.loop_ctl.tokens_spent = 123;
            app.loop_ctl.tier = EscalationTier::Swarm;
            app.loop_ctl.last_error = Some("legacy watchdog".into());
            let task = app.loop_ctl.task.clone();
            assert_eq!(app.loop_resume(), "loop resumed");
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert_eq!(app.loop_ctl.task, task);
            assert_eq!(app.loop_ctl.findings, ["operator sentinel"]);
            assert_eq!(app.loop_ctl.iteration, 3);
            assert_eq!(app.loop_ctl.tokens_spent, 123);
            assert_eq!(app.loop_ctl.tier, EscalationTier::Swarm);
            assert!(app.loop_pending.is_none() && app.pending_approval.is_none());
            app.loop_ctl.status = status;
            app.loop_ctl.iteration = 4;
            app.loop_ctl.wake_at = None;
            assert!(app.loop_resume().contains("at budget"));
            assert_eq!(app.loop_ctl.status, status);
            assert!(app.loop_ctl.wake_at.is_none());
        }
    });
}

#[test]
fn restart_preserves_active_intent_but_never_approves_or_unpauses_user_states() {
    fixture("restore", |root| {
        for podrace in [false, true] {
            for status in [
                LoopStatus::Running,
                LoopStatus::Verifying,
                LoopStatus::AwaitingApproval,
                LoopStatus::Paused,
                LoopStatus::Stopped,
                LoopStatus::Failed,
            ] {
                let mut state = running(root);
                state.podrace = podrace;
                state.status = status;
                state.tier = EscalationTier::Local;
                state.last_error = Some("no measured candidate in 3 iterations".into());
                state.awaiting_turn = true;
                std::fs::write(root.join("loop.json"), serde_json::to_vec(&state).unwrap())
                    .unwrap();
                let loaded = load_from_path(root.join("loop.json")).unwrap();
                let active = matches!(status, LoopStatus::Running | LoopStatus::Verifying);
                let expected = if active {
                    LoopStatus::Running
                } else if status == LoopStatus::AwaitingApproval {
                    LoopStatus::Paused
                } else {
                    status
                };
                assert_eq!(loaded.status, expected);
                assert_eq!(loaded.wake_at.is_some(), active);
                assert!(!loaded.awaiting_turn);
                assert_eq!(loaded.tier, EscalationTier::Local);
                assert_eq!(loaded.findings, ["operator sentinel"]);
                assert_eq!(loaded.task, state.task);
            }
        }
        let mut state = running(root);
        state.last_error = Some("HTTP 401: Unauthorized".into());
        std::fs::write(root.join("loop.json"), serde_json::to_vec(&state).unwrap()).unwrap();
        let before = Instant::now();
        let loaded = load_from_path(root.join("loop.json")).unwrap();
        assert_eq!(loaded.status, LoopStatus::Running);
        assert!(loaded.wake_at.unwrap() >= before + Duration::from_secs(60));
        let mut app = crate::seed_preview_app();
        app.loop_ctl = loaded;
        app.loop_arm();
        assert!(app.thinking.is_none());
    });
}

#[test]
fn pending_worker_and_inflight_turn_block_arming_before_budget_gate() {
    fixture("single-flight", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = running(root);
        app.loop_ctl.iteration = 1;
        app.loop_ctl.max_iters = 1;
        app.loop_ctl.tokens_spent = 77;
        let (_tx, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_arm();
        assert!(app.thinking.is_none() && app.loop_pending.is_some());
        assert_eq!(app.loop_ctl.tokens_spent, 77);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        app.loop_pending = None;
        app.loop_ctl.awaiting_turn = true;
        app.loop_arm();
        assert!(app.thinking.is_none());
        assert_eq!(app.loop_ctl.tokens_spent, 77);
        app.loop_ctl.awaiting_turn = false;
        app.loop_arm();
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert!(app.thinking.is_none());
        assert_eq!(app.loop_ctl.tokens_spent, 77);
    });
}

#[test]
fn explicit_resume_loads_active_saved_state_but_does_not_rearm_an_inflight_loop() {
    fixture("loaded-resume", |root| {
        let mut app = crate::seed_preview_app();
        let mut saved = running(root);
        saved.workspace = Some(app.tools.current_workspace().to_path_buf());
        saved.iteration = 2;
        saved.max_iters = 3;
        saved.tokens_spent = 123;
        std::fs::write(root.join("loop.json"), serde_json::to_vec(&saved).unwrap()).unwrap();
        app.loop_ctl = LoopState::default();
        assert_eq!(app.loop_ctl.status, LoopStatus::Idle);
        assert_eq!(app.loop_resume(), "loop resumed");
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(app.loop_ctl.findings, ["operator sentinel"]);
        assert_eq!(app.loop_ctl.iteration, 2);
        assert_eq!(app.loop_ctl.tokens_spent, 123);
        let wake = app.loop_ctl.wake_at;
        app.loop_ctl.awaiting_turn = true;
        assert!(app.loop_resume().contains("saved loop is running"));
        assert!(app.loop_ctl.awaiting_turn);
        assert_eq!(app.loop_ctl.wake_at, wake);
        assert!(app.thinking.is_none());
    });
}

#[test]
fn disconnected_verifier_retries_with_backoff_without_fabricating_acceptance() {
    fixture("verify-disconnect", |root| {
        for spent in [false, true] {
            let mut app = crate::seed_preview_app();
            app.loop_ctl = running(root);
            app.loop_ctl.status = LoopStatus::Verifying;
            app.loop_ctl.iteration = 1;
            app.loop_ctl.max_iters = if spent { 1 } else { 2 };
            app.loop_ctl.pending_acceptance = Some(PendingAcceptanceRun {
                command: "predicate".into(),
                config_identity: "config".into(),
                workspace_fingerprint: Some("candidate".into()),
            });
            let (tx, rx) = std::sync::mpsc::channel();
            drop(tx);
            app.loop_pending = Some(LoopPending::Verify(rx));
            let before = Instant::now();
            app.loop_drain_pending();
            assert_eq!(
                app.loop_ctl.status,
                if spent {
                    LoopStatus::Paused
                } else {
                    LoopStatus::Running
                }
            );
            assert!(app.loop_pending.is_none() && app.loop_ctl.pending_acceptance.is_none());
            assert!(app.loop_ctl.last_failed_acceptance.is_none());
            assert_eq!(app.loop_ctl.measured_candidates, 0);
            assert_eq!(
                app.loop_ctl.last_error.as_deref(),
                Some("verify worker died")
            );
            assert!(
                app.loop_ctl
                    .last_setback
                    .as_deref()
                    .unwrap()
                    .contains("without an acceptance result")
            );
            if spent {
                assert!(app.loop_ctl.wake_at.is_none());
            } else {
                assert!(app.loop_ctl.wake_at.unwrap() >= before + Duration::from_secs(60));
                let loaded = load_from_path(root.join("loop.json")).unwrap();
                assert!(loaded.wake_at.unwrap() > Instant::now() + Duration::from_secs(59));
            }
            app.loop_arm();
            assert!(app.thinking.is_none());
        }
    });
}
