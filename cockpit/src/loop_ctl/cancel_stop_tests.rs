use super::*;

#[test]
fn generic_stop_after_resume_during_drain_retracts_running_intent() {
    fixture("generic-stop", |root| {
        for stop in ["command", "escape"] {
            let (mut app, held, calls) = held_app(root);
            app.loop_command(Some("stop".into()));
            assert_eq!(app.loop_resume(), "loop resumed");
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            if stop == "command" {
                app.input = "/stop".into();
                app.submit();
            } else {
                app.interrupt_idle_safe();
            }
            assert_eq!(
                app.loop_ctl.status,
                LoopStatus::Paused,
                "{stop} parks resumed intent"
            );
            assert!(
                app.thinking.as_ref().is_some_and(
                    |turn| turn.is_draining() && Arc::ptr_eq(&turn.cancel, &held.cancel)
                )
            );
            assert!(app.loop_ctl.wake_at.is_none());
            drop(held);
            app.advance();
            app.advance();
            assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
            assert!(app.thinking.is_none());
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert_eq!(app.loop_ctl.tokens_spent, 77);
        }
    });
}

#[test]
fn retired_worker_approval_is_denied_even_with_blanket_approval_enabled() {
    // env-lock-exempt: fixture in cancel_tests.rs holds crate::tests::env_lock for the entire closure.
    fixture("retired-approval", |root| {
        for yolo in ["0", "1"] {
            let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", yolo);
            let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
            let (mut app, held, calls) = held_app(root);
            app.loop_command(Some("stop".into()));
            let (inbox_tx, inbox_rx) = std::sync::mpsc::channel();
            app.approval_rx = inbox_rx;
            let (reply, answer) = std::sync::mpsc::channel();
            inbox_tx
                .send(crate::approval::Request {
                    prompt: "retired worker requests a new action".into(),
                    scope: crate::approval::ApprovalScope::RemoteHost("owned.invalid".into()),
                    reply,
                })
                .unwrap();
            app.advance();
            assert_eq!(answer.try_recv().unwrap(), crate::approval::Decision::Deny);
            assert!(app.pending_approval.is_none());
            assert!(
                app.thinking.as_ref().is_some_and(
                    |turn| turn.is_draining() && Arc::ptr_eq(&turn.cancel, &held.cancel)
                )
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn approval_for_live_foreground_or_background_owner_still_surfaces() {
    // env-lock-exempt: fixture in cancel_tests.rs holds crate::tests::env_lock for the entire closure.
    fixture("live-approval", |root| {
        let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
        let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
        for background in [false, true] {
            let (mut app, held, _) = held_app(root);
            let (bg_reply, bg_job) =
                crate::app_control::BackgroundJob::channel("owned approval", "retry");
            if background {
                app.thinking = None; // held test channels have no actual worker
                app.loop_ctl.status = LoopStatus::Stopped;
                app.bg_job = Some(bg_job);
            }
            let (inbox_tx, inbox_rx) = std::sync::mpsc::channel();
            app.approval_rx = inbox_rx;
            let (reply, answer) = std::sync::mpsc::channel();
            inbox_tx
                .send(crate::approval::Request {
                    prompt: "owned live action".into(),
                    scope: crate::approval::ApprovalScope::SelfTest,
                    reply,
                })
                .unwrap();
            app.advance();
            assert!(app.pending_approval.is_some());
            assert!(matches!(
                answer.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));
            app.pending_approval
                .take()
                .unwrap()
                .reply
                .send(crate::approval::Decision::Deny)
                .unwrap();
            assert_eq!(answer.recv().unwrap(), crate::approval::Decision::Deny);
            drop((held, bg_reply));
        }
    });
}
