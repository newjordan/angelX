use super::*;

fn settle_baseline(app: &mut crate::App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.loop_pending.is_some() {
        assert!(Instant::now() < deadline, "owned baseline did not settle");
        app.loop_drain_pending();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn pin_owned_goal(app: &mut crate::App, root: &Path) {
    let mut registry = crate::agent::harness::ToolRegistry::new();
    registry.set_workspace(root.to_path_buf());
    app.tools = Arc::new(registry);
    let mut goal = crate::drive::goal::Goal::new("owned pinned follow-up");
    goal.accept_cmd = Some("printf 'capture\n' >> baseline-runs; printf 'test result: ok. 7 passed; 0 failed; 0 ignored;\n'".into());
    app.goal = Some(goal);
}

#[test]
fn new_pinned_loop_defers_actual_baseline_until_cancelled_worker_drains() {
    fixture("baseline", |root| {
        let (mut app, held, calls) = held_app(root);
        let mut registry = crate::agent::harness::ToolRegistry::new();
        registry.set_workspace(root.to_path_buf());
        app.tools = Arc::new(registry);
        app.loop_command(Some("stop".into()));
        let mut goal = crate::drive::goal::Goal::new("owned pinned follow-up");
        goal.accept_cmd = Some("printf 'capture\n' >> baseline-runs; printf 'test result: ok. 7 passed; 0 failed; 0 ignored;\n'".into());
        app.goal = Some(goal);
        let started = app.loop_start_immediate("owned new loop".into(), 0, false, false);
        assert!(started.contains("loop started"), "{started}");
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Running));
        assert!(
            app.loop_pending.is_none(),
            "no verifier may race the retiring worker"
        );
        assert!(!root.join("baseline-runs").exists());
        app.advance();
        assert!(app.thinking.as_ref().is_some_and(Thinking::is_draining));
        assert!(app.loop_pending.is_none());
        held.terminal
            .send(Err("old worker settled".into()))
            .unwrap();
        app.advance();
        assert!(app.thinking.is_none());
        assert!(app.loop_pending.is_none());
        app.advance();
        assert!(matches!(app.loop_pending, Some(LoopPending::Baseline(..))));
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.loop_pending.is_some() {
            assert!(Instant::now() < deadline, "owned baseline did not settle");
            app.loop_drain_pending();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(
            std::fs::read_to_string(root.join("baseline-runs")).unwrap(),
            "capture\n"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "baseline precedes model work"
        );
    });
}

#[test]
fn pinned_start_preserves_an_accepted_manual_turn_before_baseline() {
    fixture("pending-input", |root| {
        let (mut app, held, calls) = held_app(root);
        app.thinking = None; // no real worker was spawned by the held-channel fixture
        drop(held);
        app.loop_ctl.status = LoopStatus::Stopped;
        pin_owned_goal(&mut app, root);
        app.submit_deferral = true;
        app.input = "accepted manual work first".into();
        app.submit();
        assert!(app.pending_turn.is_some());
        app.loop_start_immediate("owned new loop".into(), 0, false, false);
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert!(app.loop_pending.is_none());
        let pending = app.pending_turn.as_mut().unwrap();
        assert_eq!(pending.raw.as_ref(), "accepted manual work first");
        pending.echo_drawn = true;
        let deadline = Instant::now() + Duration::from_secs(5);
        app.advance();
        while app.thinking.is_some() {
            assert!(
                Instant::now() < deadline,
                "offline manual turn did not settle"
            );
            app.advance();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!root.join("baseline-runs").exists());
        app.advance();
        assert!(matches!(app.loop_pending, Some(LoopPending::Baseline(..))));
        settle_baseline(&mut app);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
        assert_eq!(
            std::fs::read_to_string(root.join("baseline-runs")).unwrap(),
            "capture\n"
        );
    });
}

#[test]
fn pinned_start_waits_for_an_existing_background_owner() {
    fixture("background", |root| {
        let (mut app, held, calls) = held_app(root);
        app.thinking = None;
        drop(held);
        app.loop_ctl.status = LoopStatus::Stopped;
        pin_owned_goal(&mut app, root);
        let (reply, job) = crate::app::control::BackgroundJob::channel("owned held job", "retry");
        app.bg_job = Some(job);
        app.loop_start_immediate("owned new loop".into(), 0, false, false);
        assert!(app.loop_pending.is_none());
        app.advance();
        assert!(app.bg_job.is_some());
        assert!(app.loop_pending.is_none());
        assert!(!root.join("baseline-runs").exists());
        drop(reply);
        app.advance();
        assert!(app.bg_job.is_none());
        assert!(matches!(app.loop_pending, Some(LoopPending::Baseline(..))));
        settle_baseline(&mut app);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}
