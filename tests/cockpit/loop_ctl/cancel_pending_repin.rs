use super::*;

#[test]
fn goal_command_reports_queued_recapture_without_telling_user_to_reissue() {
    // env-lock-exempt: fixture in cancel_tests.rs holds crate::tests::env_lock for the entire closure.
    fixture("pending-repin-notice", |root| {
        let _goal_file = crate::tests::TestEnvGuard::set(
            "ANGEL_GOAL_FILE",
            root.join("goal.json").to_str().unwrap(),
        );
        let (mut app, _) = idle_loop_app(root);
        app.input = "/goal owned goal".into();
        app.submit();
        let persisted =
            crate::drive::goal::load_for(root).expect("real goal command persisted fixture");
        assert_eq!(persisted.text, "owned goal");
        app.loop_ctl.status = LoopStatus::Verifying;
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.input = "/goal cmd true".into();
        app.submit();
        let response = app.messages.last().unwrap().text.as_ref();
        assert!(response.contains("fresh baseline queued"), "{response}");
        assert!(!response.contains("re-issue"), "{response}");
        assert!(app.loop_pending.is_some());
        drop(terminal);
    });
}

#[test]
fn acceptance_repin_retires_live_verifier_and_preserves_new_capture_intent() {
    fixture("pending-verify-repin", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Verifying;
        app.loop_ctl.accept_cmd = Some("old predicate".into());
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        assert_eq!(app.repin_loop_accept_cmd("new predicate"), Some(false));
        assert!(app.loop_pending.is_some());
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Running));
        assert_eq!(app.loop_ctl.accept_cmd.as_deref(), Some("new predicate"));
        assert_eq!(app.repin_loop_accept_cmd("newest predicate"), Some(false));
        terminal
            .send(VerifyResult {
                passed: true,
                summary: "stale verifier green".into(),
                detail: String::new(),
            })
            .unwrap();
        app.loop_drain_pending();
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert_eq!(app.loop_ctl.accept_cmd.as_deref(), Some("newest predicate"));
        assert_eq!(app.loop_ctl.baseline_passed, None);
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Running));
        assert!(app.pending_approval.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn self_gate_repin_is_rejected_without_changing_hardcoded_acceptance_or_owner() {
    fixture("pending-self-repin", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Verifying;
        app.loop_ctl.self_edit = true;
        app.loop_ctl.accept_cmd = None;
        app.loop_ctl.baseline_passed = Some(7);
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        assert_eq!(app.repin_loop_accept_cmd("replacement self gate"), None);
        assert!(matches!(app.loop_pending, Some(LoopPending::Verify(_))));
        assert_eq!(app.loop_ctl.status, LoopStatus::Verifying);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
        assert_eq!(app.loop_ctl.accept_cmd, None);
        assert!(
            app.messages
                .iter()
                .any(|m| m.text.contains("does not re-pin it"))
        );
        // Retire before delivering the held self-gate green: no integration or
        // worktree effects are permitted in this ownership control.
        app.loop_command(Some("stop".into()));
        terminal
            .send(VerifyResult {
                passed: true,
                summary: "retired self gate green".into(),
                detail: String::new(),
            })
            .unwrap();
        app.loop_drain_pending();
        assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        assert!(app.pending_approval.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}
