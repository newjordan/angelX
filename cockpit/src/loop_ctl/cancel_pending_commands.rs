use super::*;

struct OwnedCommand {
    root: std::path::PathBuf,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Drop for OwnedCommand {
    fn drop(&mut self) {
        // Release and join even during assertion unwinding: no fixture may
        // disappear while its command thread still owns the workspace.
        let _ = std::fs::write(self.root.join("release-old"), "release\n");
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn held_baseline_command(app: &mut crate::App, root: &Path) -> OwnedCommand {
    let workspace = root.to_path_buf();
    let (terminal, rx) = std::sync::mpsc::channel();
    // Use the actual baseline command path with a test-owned terminal sender.
    // Completion publication is deliberately controlled by an owned file.
    let worker = std::thread::spawn(move || {
        let result = count_passed_in(
            "printf 'started\\n' > old-started; while [ ! -f release-old ]; do sleep 0.01; done; printf 'completed\\n' > old-completed; printf 'test result: ok. 999 passed; 0 failed; 0 ignored;\\n'",
            &workspace,
        );
        let _ = terminal.send(result);
    });
    let guard = OwnedCommand {
        root: root.to_path_buf(),
        worker: Some(worker),
    };
    app.loop_ctl.status = LoopStatus::Baselining;
    app.loop_ctl.baseline_resume_to = Some(LoopStatus::Running);
    app.loop_pending = Some(LoopPending::Baseline(rx, LoopStatus::Running));
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::read_to_string(root.join("old-started"))
        .ok()
        .as_deref()
        != Some("started\n")
    {
        assert!(
            Instant::now() < deadline,
            "owned baseline command did not start"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    guard
}

fn drain_commands(app: &mut crate::App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.loop_pending.is_some() {
        assert!(
            Instant::now() < deadline,
            "owned baseline command did not settle"
        );
        app.loop_drain_pending();
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn pause_then_repin_waits_for_old_command_and_restores_paused_intent() {
    fixture("pending-paused-repin", |root| {
        let (mut app, calls) = idle_loop_app(root);
        let guard = held_baseline_command(&mut app, root);
        app.loop_command(Some("pause".into()));
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Running));
        assert_eq!(
            app.repin_loop_accept_cmd(
                "printf 'test result: ok. 7 passed; 0 failed; 0 ignored;\\n'"
            ),
            Some(false)
        );
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Paused));
        drop(guard);
        drain_commands(&mut app);
        app.loop_arm();
        drain_commands(&mut app);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        app.loop_arm();
        assert!(app.thinking.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn real_baseline_repin_waits_for_old_command_then_captures_new_predicate_once() {
    fixture("pending-command-repin", |root| {
        let (mut app, calls) = idle_loop_app(root);
        let guard = held_baseline_command(&mut app, root);
        let command = "printf 'capture\\n' >> new-baseline-runs; printf 'test result: ok. 7 passed; 0 failed; 0 ignored;\\n'";
        assert_eq!(app.repin_loop_accept_cmd(command), Some(false));
        assert_eq!(app.repin_loop_accept_cmd(command), Some(false));
        app.loop_arm();
        app.advance();
        assert!(!root.join("new-baseline-runs").exists());
        assert!(!root.join("old-completed").exists());
        assert!(app.loop_pending.is_some());
        drop(guard); // release command, join its thread, then consume its actual terminal result
        drain_commands(&mut app);
        assert_eq!(
            app.loop_ctl.baseline_passed, None,
            "old 999 count is retired"
        );
        app.loop_arm();
        assert!(matches!(app.loop_pending, Some(LoopPending::Baseline(..))));
        drain_commands(&mut app);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(
            std::fs::read_to_string(root.join("new-baseline-runs")).unwrap(),
            "capture\n"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn real_baseline_stop_resume_and_exit_waits_for_actual_terminal_settlement() {
    fixture("pending-command-exit", |root| {
        let (mut app, calls) = idle_loop_app(root);
        app.session = crate::session::Session::at_for(root.join("sessions"), "owned".into(), root);
        app.history = vec![ChatMsg::user("owned baseline exit receipt")];
        app.session.checkpoint(&app.history).unwrap();
        let guard = held_baseline_command(&mut app, root);
        app.loop_ctl.accept_cmd = Some("touch forbidden-replacement".into());
        app.loop_command(Some("stop".into()));
        assert_eq!(app.loop_resume(), "loop resumed");
        app.loop_arm();
        assert!(!root.join("forbidden-replacement").exists());
        app.input = "/exit".into();
        app.submit();
        app.advance();
        assert!(!app.should_quit);
        assert!(!root.join("old-completed").exists());
        drop(guard);
        app.advance();
        assert!(app.should_quit);
        assert!(app.loop_pending.is_none());
        assert_eq!(app.loop_ctl.baseline_passed, None);
        assert!(!root.join("forbidden-replacement").exists());
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(app.session.path()).unwrap()).unwrap();
        assert_eq!(
            saved["history"],
            serde_json::to_value(&app.history).unwrap()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}
