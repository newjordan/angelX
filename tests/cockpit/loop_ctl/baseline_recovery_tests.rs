use super::*;

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-baseline-recovery-{tag}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    test(&root);
    std::fs::remove_dir_all(root).unwrap();
}

fn state(root: &Path) -> LoopState {
    LoopState {
        status: LoopStatus::Baselining,
        workspace: Some(root.to_path_buf()),
        task: "retain the baseline before improving the candidate".into(),
        accept_cmd: Some("printf 'capture\\n' >> baseline-runs; printf 'test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\\n'".into()),
        iteration: 2,
        tokens_spent: 77,
        findings: vec!["retained evidence".into()],
        ..Default::default()
    }
}

fn drain_capture(app: &mut crate::App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.loop_pending.is_some() {
        assert!(
            Instant::now() < deadline,
            "baseline capture did not terminate"
        );
        app.loop_drain_pending();
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn persisted_baseline_recaptures_actual_command_and_preserves_active_or_paused_intent() {
    fixture("restore", |root| {
        for podrace in [false, true] {
            for intent in [Some(LoopStatus::Running), Some(LoopStatus::Paused), None] {
                let mut saved = state(root);
                saved.podrace = podrace;
                saved.baseline_resume_to = intent;
                std::fs::write(root.join("loop.json"), serde_json::to_vec(&saved).unwrap())
                    .unwrap();
                let mut app = crate::seed_preview_app();
                app.loop_ctl = load_from_path(root.join("loop.json")).unwrap();
                assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
                assert!(app.loop_pending.is_none());
                app.loop_arm();
                assert!(matches!(
                    app.loop_pending,
                    Some(LoopPending::Baseline(_, _))
                ));
                assert!(app.thinking.is_none() && !app.loop_ctl.awaiting_turn);
                app.loop_arm(); // the live receiver owns the slot
                assert!(app.thinking.is_none());
                drain_capture(&mut app);
                assert_eq!(app.loop_ctl.baseline_passed, Some(7));
                assert_eq!(app.loop_ctl.status, intent.unwrap_or(LoopStatus::Paused));
                assert_eq!(app.loop_ctl.baseline_resume_to, None);
                assert_eq!(app.loop_ctl.iteration, 2);
                assert_eq!(app.loop_ctl.tokens_spent, 77);
                assert_eq!(app.loop_ctl.findings, ["retained evidence"]);
                assert_eq!(
                    std::fs::read_to_string(root.join("baseline-runs")).unwrap(),
                    "capture\n"
                );
                std::fs::remove_file(root.join("baseline-runs")).unwrap();
            }
        }
    });
}

#[test]
fn paused_repin_persists_paused_capture_intent_before_the_worker_completes() {
    fixture("repin", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = state(root);
        app.loop_ctl.status = LoopStatus::Paused;
        let command = app.loop_ctl.accept_cmd.clone().unwrap();
        assert_eq!(app.repin_loop_accept_cmd(&command), Some(true));
        let saved = load_from_path(root.join("loop.json")).unwrap();
        assert_eq!(saved.baseline_resume_to, Some(LoopStatus::Paused));
        assert_eq!(saved.status, LoopStatus::Baselining);
        drain_capture(&mut app);
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));

        // A restored capture has no receiver from the former process. Another
        // re-pin must use persisted paused intent instead of assuming Running.
        app.loop_ctl = saved;
        assert!(app.loop_pending.is_none());
        assert_eq!(app.repin_loop_accept_cmd(&command), Some(true));
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Paused));
        drain_capture(&mut app);
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
    });
}

#[test]
fn interrupted_capture_requires_recapture_after_explicit_resume_and_honors_budget() {
    fixture("resume", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = state(root);
        app.loop_ctl.baseline_resume_to = Some(LoopStatus::Running);
        app.loop_ctl.max_iters = 2;
        app.loop_arm();
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert!(app.loop_pending.is_none() && app.thinking.is_none());
        assert!(!root.join("baseline-runs").exists());
        app.loop_ctl.max_iters = 3;
        assert_eq!(app.loop_resume(), "loop resumed");
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        app.loop_command(Some("pause".into()));
        let saved = load_from_path(root.join("loop.json")).unwrap();
        assert_eq!(saved.status, LoopStatus::Paused);
        assert!(saved.wake_at.is_none());
        app.loop_ctl = saved;
        app.loop_arm();
        assert!(app.loop_pending.is_none());
        assert_eq!(app.loop_resume(), "loop resumed");
        app.loop_arm();
        drain_capture(&mut app);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
        assert_eq!(app.loop_ctl.iteration, 2);
    });
}

#[test]
fn disconnected_baseline_retains_required_capture_and_waits_without_fabricated_result() {
    fixture("disconnected", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = state(root);
        app.loop_ctl.baseline_resume_to = Some(LoopStatus::Paused);
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        app.loop_pending = Some(LoopPending::Baseline(rx, LoopStatus::Paused));
        let before = Instant::now();
        app.loop_drain_pending();
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert_eq!(app.loop_ctl.baseline_resume_to, Some(LoopStatus::Paused));
        assert_eq!(app.loop_ctl.baseline_passed, None);
        assert!(app.loop_ctl.wake_at.unwrap() >= before + Duration::from_secs(60));
        let before_reload = Instant::now();
        app.loop_ctl = load_from_path(root.join("loop.json")).unwrap();
        assert_eq!(app.loop_ctl.status, LoopStatus::Baselining);
        assert!(app.loop_ctl.wake_at.unwrap() >= before_reload + Duration::from_secs(60));
        app.loop_arm();
        assert!(app.loop_pending.is_none() && app.thinking.is_none());
        app.loop_ctl.wake_at = Some(Instant::now());
        app.loop_arm();
        drain_capture(&mut app);
        assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
        assert_eq!(app.loop_ctl.baseline_passed, Some(7));
    });
}

#[test]
fn repeated_missing_candidate_pressure_is_one_directive_per_prompt_after_every_round() {
    fixture("pressure", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = state(root);
        app.loop_ctl.status = LoopStatus::Running;
        app.loop_ctl.accept_cmd = None;
        app.loop_ctl.iteration = 0;
        app.loop_ctl.podrace = true;
        app.loop_ctl.first_candidate_iters = 3;
        app.loop_ctl.stall_stop = 2;
        let mut lengths = Vec::new();
        for iteration in 1..=64 {
            app.loop_harvest(format!("DIRECTION: bounded candidate check {iteration:03}"));
            let prompt = app
                .loop_iteration_convo()
                .pop()
                .unwrap()
                .content
                .to_string();
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert_eq!(app.loop_ctl.measured_candidates, 0);
            assert_eq!(
                prompt.matches("no measured candidate yet after ").count(),
                usize::from(iteration >= 3)
            );
            if iteration >= 3 {
                assert!(prompt.contains(&format!(
                    "no measured candidate yet after {iteration} iterations"
                )));
            }
            if iteration >= 30 {
                lengths.push(prompt.len());
            }
        }
        // Once the direction window fills, neither old notices nor transcript
        // messages are injected. The periodic evidence-review block is bounded.
        assert!(lengths.iter().max().unwrap() - lengths.iter().min().unwrap() < 1_024);
        assert!(app.thinking.is_none());
    });
}
