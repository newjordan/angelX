use super::*;

fn idle_loop_app(root: &Path) -> (crate::App, Arc<std::sync::atomic::AtomicUsize>) {
    let (mut app, held, calls) = held_app(root);
    app.thinking = None; // fixture channels have no running foreground worker
    drop(held);
    app.loop_ctl.awaiting_turn = false;
    (app, calls)
}

#[test]
fn stop_retains_verify_owner_and_discards_late_green_without_resuming() {
    fixture("pending-verify", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Verifying;
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_command(Some("stop".into()));
        assert!(
            app.loop_pending.is_some(),
            "stop must retain the actual verifier receiver"
        );
        app.advance();
        assert!(
            app.loop_pending.is_some(),
            "empty terminal channel is still owned"
        );
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        terminal
            .send(VerifyResult {
                passed: true,
                summary: "retired verifier green must not apply".into(),
                detail: String::new(),
            })
            .unwrap();
        app.advance();
        assert!(app.loop_pending.is_none());
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        assert!(
            !app.messages
                .iter()
                .any(|m| m.text.contains("retired verifier green must not apply"))
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn baseline_stop_resume_preserves_owner_until_terminal_settlement() {
    fixture("pending-baseline-resume", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Baselining;
        app.loop_ctl.baseline_resume_to = Some(LoopStatus::Running);
        app.loop_ctl.accept_cmd =
            Some("printf 'test result: ok. 7 passed; 0 failed; 0 ignored;\\n'".into());
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Baseline(rx, LoopStatus::Running));
        app.loop_command(Some("stop".into()));
        assert_eq!(app.loop_resume(), "loop resumed");
        app.loop_arm();
        assert!(app.loop_pending.is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        // Sender success proves this exact old receiver survived stop/resume.
        terminal
            .send(999)
            .expect("old baseline receiver remains owned until settlement");
        app.loop_drain_pending();
        assert!(app.loop_pending.is_none());
        assert_eq!(
            app.loop_ctl.baseline_passed, None,
            "retired baseline cannot become current"
        );
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
    });
}

#[test]
fn stopped_verifier_keeps_exit_open_until_its_terminal_channel_disconnects() {
    fixture("pending-exit", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.session = crate::session::Session::at_for(root.join("sessions"), "owned".into(), root);
        app.history = vec![ChatMsg::user("owned durable stop and exit")];
        app.session.checkpoint(&app.history).unwrap();
        app.loop_ctl.status = LoopStatus::Verifying;
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_command(Some("stop".into()));
        app.input = "/exit".into();
        app.submit();
        assert!(
            !app.should_quit,
            "retired verifier must remain an exit owner"
        );
        app.advance();
        assert!(!app.should_quit);
        drop(terminal);
        app.advance();
        assert!(app.should_quit);
        assert!(app.loop_pending.is_none());
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn repin_retains_old_baseline_receiver_and_queues_new_predicate() {
    fixture("pending-repin", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Baselining;
        app.loop_ctl.baseline_resume_to = Some(LoopStatus::Running);
        app.loop_ctl.accept_cmd = Some("old predicate".into());
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Baseline(rx, LoopStatus::Running));
        app.repin_loop_accept_cmd("printf 'test result: ok. 7 passed; 0 failed; 0 ignored;\\n'");
        terminal
            .send(999)
            .expect("repin cannot abandon the active old baseline receiver");
        app.loop_drain_pending();
        assert!(app.loop_pending.is_none());
        assert_eq!(app.loop_ctl.baseline_passed, None);
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Running));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn manual_input_waits_behind_the_stopped_verifier_owner() {
    fixture("pending-manual", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Verifying;
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_command(Some("stop".into()));
        app.input = "accepted followup after verifier stop".into();
        app.submit();
        assert!(
            app.thinking.is_none(),
            "manual work cannot overlap a retired verifier"
        );
        assert!(app.loop_pending.is_some());
        assert!(
            !app.history
                .iter()
                .any(|m| m.content.contains("accepted followup after verifier stop"))
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        drop(terminal);
        app.advance();
        assert!(
            app.thinking.is_some(),
            "queued manual work starts after settlement"
        );
        settle_offline_turn(&mut app);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

#[path = "cancel_pending_boundaries.rs"]
mod boundaries;

#[path = "cancel_pending_commands.rs"]
mod commands;

#[path = "cancel_pending_repin.rs"]
mod repin;

#[path = "cancel_pending_exit.rs"]
mod exit;
