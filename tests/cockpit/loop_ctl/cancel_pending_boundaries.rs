use super::*;

#[test]
fn repeated_cancel_finish_and_detach_keep_exact_receiver_and_newer_status() {
    fixture("pending-callers", |root| {
        for action in [
            "stop",
            "pause",
            "clear",
            "restart",
            "interrupt",
            "finish",
            "detach",
        ] {
            let (mut app, calls) = idle_loop_app(root);
            app.loop_ctl.status = LoopStatus::Verifying;
            let (terminal, rx) = std::sync::mpsc::channel();
            app.loop_pending = Some(LoopPending::Verify(rx));
            for _ in 0..2 {
                match action {
                    "interrupt" => {
                        assert!(app.interrupt());
                    }
                    "finish" => app.loop_finish(LoopStatus::Failed, "owned terminal intent"),
                    "detach" => {
                        app.loop_detach_for_workspace_change();
                    }
                    _ => {
                        app.loop_command(Some(action.into()));
                    }
                }
                assert!(app.loop_pending.is_some(), "{action}");
            }
            let status = app.loop_ctl.status;
            terminal
                .send(VerifyResult {
                    passed: true,
                    summary: "late old green".into(),
                    detail: String::new(),
                })
                .unwrap();
            app.loop_drain_pending();
            app.loop_drain_pending();
            assert!(app.loop_pending.is_none());
            assert_eq!(
                app.loop_ctl.status, status,
                "{action} terminal result cannot change newer intent"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert_eq!(
                app.messages
                    .iter()
                    .filter(|m| m.text.contains("verifier slot released"))
                    .count(),
                1
            );
        }
    });
}

#[test]
fn late_retired_verifier_cannot_complete_a_new_loop() {
    fixture("pending-new-loop", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Verifying;
        app.goal = None;
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_command(Some("stop".into()));
        let started = app.loop_start_immediate("newer owned loop".into(), 0, false, false);
        assert!(started.contains("loop started"), "{started}");
        app.loop_arm();
        assert!(app.thinking.is_none());
        assert!(app.loop_pending.is_some());
        let new_id = app.loop_ctl.id.clone();
        terminal
            .send(VerifyResult {
                passed: true,
                summary: "old work passed".into(),
                detail: String::new(),
            })
            .unwrap();
        app.loop_drain_pending();
        assert_eq!(app.loop_ctl.id, new_id);
        assert_eq!(app.loop_ctl.task, "newer owned loop");
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(app.loop_ctl.iteration, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn generic_stop_parks_resume_intent_and_retracts_input_while_verifier_drains() {
    fixture("pending-generic-stop", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.loop_ctl.status = LoopStatus::Verifying;
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_command(Some("stop".into()));
        assert_eq!(app.loop_resume(), "loop resumed");
        app.steer_queue.push(ChatMsg::user("retract this input"));
        app.input = "/stop".into();
        app.submit();
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert!(app.steer_queue.is_empty());
        drop(terminal);
        app.advance();
        app.advance();
        assert!(app.thinking.is_none());
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn retired_owner_blocks_direct_sota_self_and_workspace_admission() {
    fixture("pending-direct-admission", |root| {
        let (mut app, calls) = idle_loop_app(root);
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_command(Some("stop".into()));
        assert!(app.loop_command(Some("sota".into())).contains("busy"));
        app.loop_request_sota();
        app.loop_spawn_verify("touch forbidden-admission".into());
        app.loop_spawn_self_gate();
        assert!(
            app.self_command(Some("owned new self task".into()))
                .contains("busy")
        );
        assert!(app.change_workspace(Some("/tmp")).contains("busy"));
        app.loop_ctl.self_edit = true;
        assert!(app.self_command(Some("discard".into())).contains("busy"));
        app.self_request_integrate("must not integrate");
        app.self_integrate_now();
        assert!(app.loop_pending.is_some());
        assert!(app.pending_approval.is_none());
        assert!(!root.join("forbidden-admission").exists());
        terminal
            .send(VerifyResult {
                passed: false,
                summary: "held owner survives every admission gate".into(),
                detail: String::new(),
            })
            .unwrap();
        app.loop_drain_pending();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn cancel_local_approvals_does_not_leave_a_retired_worker_and_live_approval_still_applies() {
    fixture("pending-approvals", |root| {
        for integration in [false, true] {
            let (mut app, _) = idle_loop_app(root);
            let (reply, rx) = std::sync::mpsc::channel();
            let retained_reply = reply.clone();
            app.loop_ctl.status = LoopStatus::AwaitingApproval;
            app.pending_approval = Some(crate::PendingApproval {
                prompt: "owned decision".into(),
                scope_label: None,
                reply,
            });
            app.loop_pending = Some(if integration {
                LoopPending::SelfIntegrate(rx)
            } else {
                LoopPending::Approval(rx)
            });
            app.loop_command(Some("stop".into()));
            assert!(app.loop_pending.is_none() && app.pending_approval.is_none());
            assert!(retained_reply.send(Decision::ApproveAll).is_err());
            app.loop_drain_pending();
            assert_eq!(app.loop_ctl.status, LoopStatus::Stopped);
        }
        let (mut app, calls) = idle_loop_app(root);
        let (reply, rx) = std::sync::mpsc::channel();
        app.loop_ctl.status = LoopStatus::AwaitingApproval;
        app.loop_pending = Some(LoopPending::Approval(rx));
        reply.send(Decision::ApproveAll).unwrap();
        app.loop_drain_pending();
        assert_eq!(app.loop_ctl.tier, EscalationTier::Sota);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn accepted_deferred_manual_input_does_not_starve_the_retired_owner_drain() {
    fixture("pending-deferred", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.submit_deferral = true;
        app.input = "owned accepted deferred message".into();
        app.submit();
        app.pending_turn.as_mut().unwrap().echo_drawn = true;
        let (terminal, rx) = std::sync::mpsc::channel();
        app.loop_pending = Some(LoopPending::Verify(rx));
        app.loop_command(Some("stop".into()));
        app.advance();
        assert!(app.pending_turn.is_some() && app.thinking.is_none());
        assert!(app.loop_pending.is_some());
        drop(terminal);
        app.advance();
        assert!(app.loop_pending.is_none() && app.pending_turn.is_none());
        settle_offline_turn(&mut app);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}
