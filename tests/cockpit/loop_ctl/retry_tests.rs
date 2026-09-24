use super::*;
use std::sync::atomic::Ordering;

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-loop-retry-{tag}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let _local = crate::tests::TestEnvGuard::set("ANGEL_LOOP_LOCAL_CLUB", "");
    test(&root);
    std::fs::remove_dir_all(root).unwrap();
}

fn running(root: &Path, podrace: bool) -> LoopState {
    LoopState {
        status: LoopStatus::Running,
        awaiting_turn: true,
        task: "continue the owned candidate".into(),
        workspace: Some(root.to_path_buf()),
        podrace,
        first_candidate_iters: 1,
        stall_stop: 1,
        ..Default::default()
    }
}

fn assert_paced(app: &mut crate::App, before: Instant) {
    assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    assert!(app.loop_ctl.wake_at.unwrap() >= before + Duration::from_secs(60));
    let tokens = app.loop_ctl.tokens_spent;
    app.loop_arm();
    assert!(
        app.thinking.is_none(),
        "an error must not arm another paid turn immediately"
    );
    assert!(!app.loop_ctl.awaiting_turn);
    assert_eq!(app.loop_ctl.tokens_spent, tokens);
}

#[test]
fn exhausted_transient_errors_are_paced_across_profiles_and_stall_thresholds() {
    fixture("errors", |root| {
        for podrace in [false, true] {
            for (index, error) in [
                "HTTP 503: temporarily unavailable",
                "HTTP 429: Too Many Requests",
                "transport error after 1 attempt(s): connection refused",
                "worker vanished",
            ]
            .iter()
            .enumerate()
            {
                assert!(!crate::agent::club::error_requires_provider_action(error));
                let mut app = crate::seed_preview_app();
                app.bag = crate::agent::club::Bag::practice_for_test();
                let selected = app.bag.in_hand_with_fallback().route_identity();
                app.loop_ctl = running(root, podrace);
                let tier = [
                    EscalationTier::Local,
                    EscalationTier::Swarm,
                    EscalationTier::Sota,
                ][index % 3];
                app.loop_ctl.tier = tier;
                let before = Instant::now();
                app.loop_harvest_error_with_tools((*error).into(), ToolStripSnapshot::default());
                assert_paced(&mut app, before);
                assert_eq!(app.loop_ctl.iteration, 1);
                assert_eq!(app.loop_ctl.tier, tier);
                assert_eq!(app.bag.in_hand_with_fallback().route_identity(), selected);
                assert_eq!(app.loop_ctl.last_error.as_deref(), Some(*error));
                let before_reload = Instant::now();
                app.loop_ctl = load_from_path(root.join("loop.json")).unwrap();
                assert_paced(&mut app, before_reload);
                assert_eq!(app.loop_ctl.tier, tier);
            }
        }
    });
}

#[test]
fn completed_worker_error_reaches_pacing_through_actual_advance() {
    fixture("worker", |root| {
        let mut app = crate::seed_preview_app();
        app.bag = crate::agent::club::Bag::practice_for_test();
        app.loop_ctl = running(root, false);
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Err("HTTP 503: temporarily unavailable".to_string()))
            .unwrap();
        drop(tx);
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        drop(event_tx);
        app.thinking = Some(Thinking {
            started: Instant::now(),
            club_label: "practice".into(),
            club: None,
            spawn_usage: crate::agent::turn::published_spawn_usage(
                None,
                crate::agent::club::CacheUsage::default(),
            ),
            requested_route: app.bag.in_hand_with_fallback().route_identity(),
            cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            rx,
            event_rx,
            last_stream_at: Instant::now(),
            idle_timeout_secs: Some(600),
            idle_warned_50: false,
            idle_warned_80: false,
            steer_idle_interrupt_secs: 60,
            steer_interrupt_fired: false,
            draining: false,
        });
        let before = Instant::now();
        app.advance();
        assert_paced(&mut app, before);
        assert_eq!(app.loop_ctl.iteration, 1);
    });
}

struct OfflineCounter(Arc<std::sync::atomic::AtomicUsize>);

impl Club for OfflineCounter {
    fn label(&self) -> &str {
        "practice"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok("DIRECTION: inspect the owned candidate next".into())
    }
}

#[test]
fn expired_retry_arms_one_offline_turn_and_success_restores_zero_interval() {
    fixture("expiry", |root| {
        let workspace = crate::tests::TestGitWorkspace::new("retry-expiry");
        let mut app = crate::seed_preview_app();
        app.tools = Arc::new(workspace.registry());
        app.bag = crate::agent::club::Bag::practice_for_test();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        app.bag
            .replace_in_hand_club_for_test(Arc::new(OfflineCounter(calls.clone())));
        app.loop_ctl = running(root, false);
        app.loop_harvest_error_with_tools(
            "HTTP 503: unavailable".into(),
            ToolStripSnapshot::default(),
        );
        app.loop_ctl.wake_at = Some(Instant::now()); // advance virtual deadline, never sleep a minute
        app.loop_arm();
        assert!(app.thinking.is_some());
        let tokens = app.loop_ctl.tokens_spent;
        app.loop_arm();
        assert_eq!(
            app.loop_ctl.tokens_spent, tokens,
            "single-flight input charged once"
        );
        let thinking = app.thinking.take().unwrap();
        let (_, reply, _, _) = thinking
            .rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        app.loop_harvest_with_tools(
            reply,
            ToolStripSnapshot {
                calls: 20,
                ..Default::default()
            },
        );
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert!(app.loop_ctl.wake_at.unwrap() <= Instant::now());
        // The transcript retains the failure; successful recovery clears its active diagnostic.
        assert!(app.loop_ctl.last_error.is_none());
        let loaded = load_from_path(root.join("loop.json")).unwrap();
        assert!(loaded.wake_at.unwrap() <= Instant::now());
    });
}

#[test]
fn retry_respects_longer_interval_and_successful_round_cadence() {
    fixture("interval", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = running(root, false);
        app.loop_ctl.interval_secs = 180;
        let before = Instant::now();
        app.loop_harvest_error_with_tools("transport timeout".into(), ToolStripSnapshot::default());
        assert!(app.loop_ctl.wake_at.unwrap() >= before + Duration::from_secs(180));
        let before_reload = Instant::now();
        app.loop_ctl = load_from_path(root.join("loop.json")).unwrap();
        assert!(app.loop_ctl.wake_at.unwrap() >= before_reload + Duration::from_secs(180));
        app.loop_ctl.interval_secs = 3;
        let before_success = Instant::now();
        app.loop_harvest_with_tools(
            "DIRECTION: next owned check".into(),
            ToolStripSnapshot {
                calls: 20,
                ..Default::default()
            },
        );
        let wake = app.loop_ctl.wake_at.unwrap();
        assert!(wake >= before_success + Duration::from_secs(3));
        assert!(wake < Instant::now() + Duration::from_secs(4));
    });
}

#[test]
fn retry_cannot_override_user_budgets_or_manual_parking() {
    fixture("caps", |root| {
        for cap in ["iterations", "tokens", "deadline"] {
            let mut app = crate::seed_preview_app();
            app.loop_ctl = running(root, false);
            match cap {
                "iterations" => app.loop_ctl.max_iters = 1,
                "tokens" => {
                    app.loop_ctl.token_budget = 10;
                    app.loop_ctl.tokens_spent = 10;
                }
                _ => {
                    app.loop_ctl.deadline_secs = 1;
                    app.loop_ctl.started_ms = now_ms().saturating_sub(2_000);
                }
            }
            app.loop_harvest_error_with_tools(
                "HTTP 503: unavailable".into(),
                ToolStripSnapshot::default(),
            );
            assert_eq!(app.loop_ctl.status, LoopStatus::Paused, "{cap}");
            assert!(app.loop_ctl.wake_at.is_none());
            app.loop_arm();
            assert!(app.thinking.is_none());
        }
        for command in ["pause", "stop", "interrupt"] {
            let mut app = crate::seed_preview_app();
            app.loop_ctl = running(root, false);
            app.loop_harvest_error_with_tools(
                "HTTP 503: unavailable".into(),
                ToolStripSnapshot::default(),
            );
            if command == "interrupt" {
                app.loop_on_interrupt();
            } else {
                app.loop_command(Some(command.into()));
            }
            let parked = load_from_path(root.join("loop.json")).unwrap();
            assert!(matches!(
                parked.status,
                LoopStatus::Paused | LoopStatus::Stopped
            ));
            assert!(parked.wake_at.is_none());
            assert_eq!(app.loop_resume(), "loop resumed");
            assert!(app.loop_ctl.wake_at.unwrap() <= Instant::now());
            let resumed = load_from_path(root.join("loop.json")).unwrap();
            assert_eq!(resumed.status, LoopStatus::Running);
            assert!(
                resumed.wake_at.unwrap() <= Instant::now(),
                "manual resume clears retry pacing"
            );
        }
    });
}

#[test]
fn explicit_hop_horizon_keeps_zero_interval_continuation() {
    fixture("horizon", |root| {
        for podrace in [false, true] {
            let mut app = crate::seed_preview_app();
            app.loop_ctl = running(root, podrace);
            app.loop_harvest_error_with_tools(
                "tool loop hit the 17-hop runaway guard without answering".into(),
                ToolStripSnapshot::default(),
            );
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert_eq!(app.loop_ctl.tier, EscalationTier::Local);
            assert!(app.loop_ctl.wake_at.unwrap() <= Instant::now());
            let loaded = load_from_path(root.join("loop.json")).unwrap();
            assert!(loaded.wake_at.unwrap() <= Instant::now());
        }
    });
}
