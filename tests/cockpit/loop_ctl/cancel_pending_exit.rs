use super::*;

fn owned_exit_session(app: &mut crate::App, root: &Path) {
    app.session =
        crate::knowledge::session::Session::at_for(root.join("sessions"), "owned".into(), root);
    app.history = vec![ChatMsg::user("owned self gate exit")];
    app.session.checkpoint(&app.history).unwrap();
    app.loop_ctl.self_edit = true;
    app.loop_ctl.self_branch = Some("owned-unmerged-branch".into());
    // Omit integration roots: a regression must fail before any Git effects.
    app.loop_ctl.self_root = None;
}

#[test]
fn live_self_gate_green_during_exit_neither_prompts_nor_auto_integrates() {
    // env-lock-exempt: fixture in cancel_tests.rs holds crate::tests::env_lock for the entire closure.
    fixture("pending-self-exit", |root| {
        for yolo in ["0", "1"] {
            let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", yolo);
            let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
            let (mut app, calls) = idle_loop_app(root);
            owned_exit_session(&mut app, root);
            app.loop_ctl.status = LoopStatus::Verifying;
            let (terminal, rx) = std::sync::mpsc::channel();
            app.loop_pending = Some(LoopPending::Verify(rx));
            app.input = "/exit".into();
            app.submit();
            assert!(!app.should_quit);
            terminal
                .send(VerifyResult {
                    passed: true,
                    summary: "owned actual gate result".into(),
                    detail: String::new(),
                })
                .unwrap();
            app.advance();
            assert!(app.should_quit);
            assert!(app.loop_pending.is_none() && app.pending_approval.is_none());
            assert_eq!(
                app.loop_ctl.status,
                LoopStatus::Done,
                "completed gate remains truthful"
            );
            assert_eq!(
                app.loop_ctl.self_branch.as_deref(),
                Some("owned-unmerged-branch")
            );
            assert!(
                app.messages
                    .iter()
                    .any(|m| m.text.contains("gate green")
                        && m.text.contains("branch kept unmerged"))
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn existing_self_integration_approval_cannot_dispatch_merge_after_exit_request() {
    fixture("pending-approved-exit", |root| {
        let (mut app, calls) = idle_loop_app(root);
        owned_exit_session(&mut app, root);
        app.loop_ctl.status = LoopStatus::AwaitingApproval;
        let (decision, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::SelfIntegrate(rx));
        app.input = "/exit".into();
        app.submit();
        assert!(!app.should_quit);
        decision.send(Decision::ApproveAll).unwrap();
        app.advance();
        assert!(app.should_quit);
        assert!(app.loop_pending.is_none() && app.pending_approval.is_none());
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert!(
            app.messages
                .iter()
                .any(|m| m.text.contains("integration held by exit"))
        );
        assert_eq!(
            app.loop_ctl.self_branch.as_deref(),
            Some("owned-unmerged-branch")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}
