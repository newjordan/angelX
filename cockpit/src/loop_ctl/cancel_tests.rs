use super::*;
use std::sync::atomic::Ordering;

type TerminalSender = std::sync::mpsc::Sender<
    Result<
        (
            Vec<ChatMsg>,
            String,
            crate::club::RouteIdentity,
            crate::harness::TurnStopReason,
        ),
        String,
    >,
>;

struct Held {
    terminal: TerminalSender,
    events: std::sync::mpsc::Sender<crate::harness::TurnEvent>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    old_steers: Arc<crate::steer::SteerQueue>,
}

struct OfflineCounter(Arc<std::sync::atomic::AtomicUsize>);
impl Club for OfflineCounter {
    fn label(&self) -> &str {
        "practice"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok("DIRECTION: next owned check".into())
    }
}

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-loop-cancel-{tag}-{}-{}",
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

fn held_app(root: &Path) -> (crate::App, Held, Arc<std::sync::atomic::AtomicUsize>) {
    let mut app = crate::seed_preview_app();
    let mut registry = crate::harness::ToolRegistry::new();
    registry.set_workspace(root.to_path_buf());
    app.tools = Arc::new(registry);
    app.bag = crate::club::Bag::practice_for_test();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    app.bag
        .replace_in_hand_club_for_test(Arc::new(OfflineCounter(calls.clone())));
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        awaiting_turn: true,
        task: "owned old loop".into(),
        workspace: Some(root.to_path_buf()),
        tokens_spent: 77,
        ..Default::default()
    };
    let (terminal, rx) = std::sync::mpsc::channel();
    let (events, event_rx) = std::sync::mpsc::channel();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.thinking = Some(Thinking {
        started: Instant::now(),
        club_label: "practice".into(),
        club: None,
        spawn_usage: crate::turn::published_spawn_usage(None, crate::club::CacheUsage::default()),
        requested_route: app.bag.in_hand_with_fallback().route_identity(),
        cancel: cancel.clone(),
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
    let old_steers = app.steer_queue.clone();
    (
        app,
        Held {
            terminal,
            events,
            cancel,
            old_steers,
        },
        calls,
    )
}

fn settle_offline_turn(app: &mut crate::App) {
    let turn = app.thinking.take().expect("one fresh offline turn");
    assert!(!turn.is_draining());
    turn.rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
}

#[test]
fn stop_then_resume_cannot_rearm_until_the_held_worker_settles() {
    fixture("resume", |root| {
        let (mut app, held, calls) = held_app(root);
        let route = app.bag.in_hand_with_fallback().route_identity();
        app.loop_command(Some("stop".into()));
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        assert!(held.cancel.load(Ordering::Acquire));
        assert_eq!(app.loop_resume(), "loop resumed");
        app.loop_arm();
        assert!(
            app.thinking
                .as_ref()
                .is_some_and(|turn| turn.is_draining() && Arc::ptr_eq(&turn.cancel, &held.cancel)),
            "replacement must not acquire the still-owned worker slot"
        );
        assert_eq!(app.loop_ctl.tokens_spent, 77);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        held.events
            .send(crate::harness::TurnEvent::Token(
                "late retired token".into(),
            ))
            .unwrap();
        app.advance();
        assert!(app.thinking.as_ref().is_some_and(Thinking::is_draining));
        held.terminal
            .send(Ok((
                vec![ChatMsg::assistant("late retired answer")],
                "late retired answer".into(),
                route.clone(),
                crate::harness::TurnStopReason::Answer,
            )))
            .unwrap();
        app.advance();
        assert!(app.thinking.is_none());
        assert_eq!(app.loop_ctl.iteration, 0, "retired result is not harvested");
        assert!(
            app.history
                .iter()
                .all(|message| !message.content.contains("late retired"))
        );
        assert!(
            app.messages
                .iter()
                .all(|message| !message.text.contains("late retired"))
        );
        app.loop_arm();
        assert!(app.thinking.is_some());
        assert_eq!(app.bag.in_hand_with_fallback().route_identity(), route);
        settle_offline_turn(&mut app);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn new_input_after_stop_is_isolated_from_retired_steers_until_drain() {
    fixture("followup", |root| {
        let (mut app, held, calls) = held_app(root);
        held.old_steers.push(ChatMsg::user("old loop steer"));
        app.loop_command(Some("stop".into()));
        assert!(held.old_steers.is_empty(), "pre-stop steers are discarded");
        app.input = "fresh work after loop stop".into();
        app.submit();
        assert!(app.thinking.as_ref().is_some_and(Thinking::is_draining));
        assert_eq!(app.steer_queue.len(), 1);
        assert!(!Arc::ptr_eq(&app.steer_queue, &held.old_steers));
        assert!(
            held.old_steers.drain().is_empty(),
            "retired worker cannot consume new input even after its cancel check"
        );
        assert!(app.pending_turn.is_none());
        assert!(
            app.history
                .iter()
                .all(|message| !message.content.contains("fresh work after loop stop"))
        );
        app.advance();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        held.terminal
            .send(Err("retired worker stopped".into()))
            .unwrap();
        app.advance();
        assert!(app.thinking.is_none());
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        assert_eq!(
            app.steer_queue.len(),
            1,
            "post-stop input waits for the next idle frame"
        );
        app.advance();
        assert!(
            app.thinking.is_some(),
            "explicit new input may start after terminal settlement"
        );
        assert_eq!(
            app.loop_ctl.status,
            LoopStatus::Stopped,
            "new manual work does not revive the loop"
        );
        assert!(
            app.history
                .iter()
                .any(|message| message.content.contains("fresh work after loop stop"))
        );
        settle_offline_turn(&mut app);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn every_loop_cancel_caller_retains_the_same_foreground_owner() {
    fixture("callers", |root| {
        for action in ["pause", "stop", "clear", "restart", "interrupt", "detach"] {
            let (mut app, held, calls) = held_app(root);
            match action {
                "interrupt" => {
                    assert!(app.interrupt());
                }
                "detach" => {
                    app.loop_detach_for_workspace_change();
                }
                _ => {
                    app.loop_command(Some(action.into()));
                }
            }
            assert!(
                app.thinking.as_ref().is_some_and(
                    |turn| turn.is_draining() && Arc::ptr_eq(&turn.cancel, &held.cancel)
                ),
                "{action} cannot drop worker ownership"
            );
            assert!(held.cancel.load(Ordering::Acquire));
            assert!(!app.loop_ctl.awaiting_turn);
            let expected = match action {
                "pause" | "interrupt" => LoopStatus::Paused,
                "clear" | "detach" => LoopStatus::Idle,
                _ => LoopStatus::Stopped,
            };
            assert_eq!(app.loop_ctl.status, expected);
            drop(held);
            app.advance();
            assert!(
                app.thinking.is_none(),
                "disconnect also proves terminal settlement"
            );
            assert_eq!(app.loop_ctl.status, expected);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn repeated_stop_retracts_queued_followup_without_releasing_the_owner() {
    fixture("repeat", |root| {
        let (mut app, held, calls) = held_app(root);
        app.loop_command(Some("stop".into()));
        app.input = "cancel this followup too".into();
        app.submit();
        assert_eq!(app.steer_queue.len(), 1);
        app.loop_command(Some("stop".into()));
        assert!(app.steer_queue.is_empty());
        assert!(
            app.thinking
                .as_ref()
                .is_some_and(|turn| Arc::ptr_eq(&turn.cancel, &held.cancel))
        );
        drop(held);
        app.advance();
        app.advance();
        assert!(app.thinking.is_none());
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            app.messages
                .iter()
                .filter(|message| message.text.contains("stopped worker drained"))
                .count(),
            1
        );
    });
}

#[path = "cancel_baseline_tests.rs"]
mod baseline;

#[path = "cancel_stop_tests.rs"]
mod stop;

#[path = "cancel_exit_tests.rs"]
mod exit;

#[path = "cancel_pending_tests.rs"]
mod pending;
