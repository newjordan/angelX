use super::*;

#[test]
fn loop_stop_then_exit_checkpoints_fresh_followup_after_terminal_settlement_once() {
    // env-lock-exempt: fixture in cancel_tests.rs holds crate::tests::env_lock for the entire closure.
    fixture("exit", |root| {
        let _env = [
            crate::tests::TestEnvGuard::set("HOME", root.to_str().unwrap()),
            crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
        ];
        let (mut app, held, calls) = held_app(root);
        app.session =
            crate::session::Session::at_for(root.join("sessions"), "owned-loop-exit".into(), root);
        app.history = vec![
            ChatMsg::system("static policy"),
            ChatMsg::user("old durable operator"),
        ];
        app.session.checkpoint(&app.history).unwrap();
        let before_disk = std::fs::read(app.session.path()).unwrap();
        let before_history = serde_json::to_value(&app.history).unwrap();
        held.old_steers
            .push(ChatMsg::user("discarded pre-stop steer"));
        app.input = "/loop stop".into();
        app.submit();
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        assert!(held.cancel.load(Ordering::Acquire));
        assert!(held.old_steers.is_empty());

        let sentinel = "EXACT_LOOP_STOP_EXIT_FOLLOWUP\n東京 🧭 e\u{301}";
        app.input = sentinel.into();
        app.submit();
        assert_eq!(app.steer_queue.len(), 1);
        assert!(!Arc::ptr_eq(&app.steer_queue, &held.old_steers));
        assert!(held.old_steers.drain().is_empty());

        // Exercise actual deferred baseline admission as well as queued model work.
        let mut goal = crate::goal::Goal::new("owned deferred baseline");
        goal.accept_cmd = Some("printf 'capture\\n' >> baseline-runs; printf 'test result: ok. 7 passed; 0 failed; 0 ignored;\\n'".into());
        app.goal = Some(goal);
        let started = app.loop_start_immediate("owned deferred loop".into(), 0, false, false);
        assert!(started.contains("loop started"), "{started}");
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Running));
        assert!(app.loop_pending.is_none());
        assert!(!root.join("baseline-runs").exists());

        app.input = "/exit".into();
        app.submit();
        assert_eq!(
            app.exit_request,
            Some(crate::app::ExitRequest::WaitingForIdle)
        );
        held.events
            .send(crate::harness::TurnEvent::Token(
                "late retired token".into(),
            ))
            .unwrap();
        for _ in 0..2 {
            app.advance();
            assert!(!app.should_quit);
            assert!(app.thinking.as_ref().is_some_and(|turn| {
                turn.is_draining() && Arc::ptr_eq(&turn.cancel, &held.cancel)
            }));
            assert!(app.pending_turn.is_none() && app.loop_pending.is_none());
            assert_eq!(app.steer_queue.len(), 1);
            assert_eq!(serde_json::to_value(&app.history).unwrap(), before_history);
            assert_eq!(std::fs::read(app.session.path()).unwrap(), before_disk);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert!(!root.join("baseline-runs").exists());
        }

        held.terminal
            .send(Ok((
                vec![ChatMsg::assistant("late retired answer")],
                "late retired answer".into(),
                app.bag.in_hand_with_fallback().route_identity(),
                crate::harness::TurnStopReason::Answer,
            )))
            .unwrap();
        app.advance();
        assert!(app.thinking.is_none());
        assert!(!app.should_quit);
        assert_eq!(app.steer_queue.len(), 1);
        assert_eq!(std::fs::read(app.session.path()).unwrap(), before_disk);
        // Both admission entry points must respect retained exit intent now idle.
        app.flush_queued_steers();
        app.loop_arm();
        assert!(app.thinking.is_none() && app.pending_turn.is_none());
        assert!(app.loop_pending.is_none());
        app.advance();

        assert!(app.should_quit);
        assert!(app.thinking.is_none() && app.pending_turn.is_none());
        assert!(app.loop_pending.is_none());
        assert!(app.steer_queue.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!root.join("baseline-runs").exists());
        let expected = vec![
            ChatMsg::system("static policy"),
            ChatMsg::user("old durable operator"),
            ChatMsg::user(sentinel),
        ];
        assert_eq!(
            serde_json::to_value(&app.history).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(app.session.path()).unwrap()).unwrap();
        assert_eq!(record["history"], serde_json::to_value(&expected).unwrap());
        assert_eq!(
            record["history"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["content"] == sentinel)
                .count(),
            1
        );
        assert_eq!(
            app.session.save_status(),
            crate::session::SessionSaveStatus::Healthy
        );
        assert!(
            app.messages
                .iter()
                .all(|m| !m.text.contains("late retired"))
        );
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
    });
}
