use super::*;

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-loop-recovery-{tag}-{}-{}",
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

fn running(root: &Path) -> LoopState {
    LoopState {
        status: LoopStatus::Running,
        awaiting_turn: true,
        task: "continue the owned candidate".into(),
        workspace: Some(root.to_path_buf()),
        ..Default::default()
    }
}

#[test]
fn successful_reply_retires_provider_and_ordinary_errors_before_reload() {
    fixture("success", |root| {
        for error in [
            "HTTP 401: Unauthorized",
            "HTTP 503: temporarily unavailable",
            "transport timeout",
        ] {
            let mut app = crate::seed_preview_app();
            app.loop_ctl = running(root);
            let before_error = Instant::now();
            app.loop_harvest_error_with_tools(error.into(), ToolStripSnapshot::default());
            assert_eq!(app.loop_ctl.last_error.as_deref(), Some(error));
            assert!(app.loop_ctl.wake_at.unwrap() >= before_error + Duration::from_secs(60));
            assert!(
                app.messages
                    .iter()
                    .any(|message| message.text.contains(error))
            );
            app.loop_harvest_with_tools(
                "DIRECTION: continue with the recovered route".into(),
                ToolStripSnapshot {
                    calls: 20,
                    ..Default::default()
                },
            );
            assert!(app.loop_ctl.last_error.is_none());
            assert!(
                app.messages
                    .iter()
                    .any(|message| message.text.contains(error)),
                "failure transcript remains"
            );
            assert!(!app.loop_ctl.retry_after_error);
            assert!(app.loop_ctl.wake_at.unwrap() <= Instant::now());
            let loaded = load_from_path(root.join("loop.json")).unwrap();
            assert_eq!(loaded.status, LoopStatus::Running);
            assert_eq!(loaded.iteration, 2);
            assert!(loaded.last_error.is_none());
            assert!(
                loaded.wake_at.unwrap() <= Instant::now(),
                "recovery must survive restart: {error}"
            );
        }
    });
}

#[test]
fn old_checkpoints_without_retry_flag_keep_blocker_delay_until_success() {
    fixture("legacy", |root| {
        for error in [
            "HTTP 401: Unauthorized",
            "goal durability checkpoint failed (owned store unavailable)",
            "verify worker died",
        ] {
            let mut state = running(root);
            state.last_error = Some(error.into());
            state.tier = EscalationTier::Swarm;
            let mut wire = serde_json::to_value(&state).unwrap();
            wire.as_object_mut().unwrap().remove("retry_after_error");
            std::fs::write(root.join("loop.json"), serde_json::to_vec(&wire).unwrap()).unwrap();
            let before_reload = Instant::now();
            let loaded = load_from_path(root.join("loop.json")).unwrap();
            assert_eq!(loaded.last_error.as_deref(), Some(error));
            assert!(loaded.wake_at.unwrap() >= before_reload + Duration::from_secs(60));
            let mut app = crate::seed_preview_app();
            app.loop_ctl = loaded;
            app.loop_arm();
            assert!(
                app.thinking.is_none(),
                "legacy unresolved blocker still waits"
            );
            app.loop_harvest("DIRECTION: the next owned check".into());
            let recovered = load_from_path(root.join("loop.json")).unwrap();
            assert_eq!(recovered.tier, EscalationTier::Swarm);
            assert!(recovered.last_error.is_none());
            assert!(recovered.wake_at.unwrap() <= Instant::now());
        }
    });
}

#[test]
fn explicit_resume_clears_active_diagnostic_and_persists_immediate_intent() {
    fixture("resume", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = running(root);
        let error = "HTTP 401: Unauthorized";
        app.loop_harvest_error_with_tools(error.into(), ToolStripSnapshot::default());
        app.loop_command(Some("pause".into()));
        assert_eq!(app.loop_resume(), "loop resumed");
        assert!(app.loop_ctl.last_error.is_none());
        assert!(
            app.loop_ctl
                .last_setback
                .as_deref()
                .unwrap()
                .contains(error),
            "the next prompt still has the last setback"
        );
        let resumed = load_from_path(root.join("loop.json")).unwrap();
        assert_eq!(resumed.status, LoopStatus::Running);
        assert!(resumed.wake_at.unwrap() <= Instant::now());
        assert!(
            app.messages
                .iter()
                .any(|message| message.text.contains(error))
        );
    });
}

#[test]
fn successful_model_reply_does_not_resolve_an_execution_prerequisite() {
    fixture("execution", |root| {
        let mut app = crate::seed_preview_app();
        app.loop_ctl = running(root);
        crate::agent::harness::exec::set_sandbox_receipt(Some(serde_json::json!({
            "helper_error": "Landlock unavailable; run angel --doctor",
            "helper_phase": "landlock",
            "helper_exit": 1,
        })));
        let error = crate::agent::harness::execution_blocker(
            "shell", "tool error: shell command failed (exit 1)\nbwrap: setting up uid map: Permission denied"
        ).unwrap();
        app.loop_harvest_error_with_tools(error.clone(), ToolStripSnapshot::default());
        app.loop_harvest("DIRECTION: inspect the prerequisite next".into());
        assert_eq!(
            app.loop_ctl.execution_blocker.as_deref(),
            Some(error.as_str())
        );
        let loaded = load_from_path(root.join("loop.json")).unwrap();
        assert_eq!(loaded.execution_blocker.as_deref(), Some(error.as_str()));
        assert_eq!(loaded.status, LoopStatus::Paused);
        assert!(loaded.wake_at.is_none());
        assert!(!loaded.retry_after_error);
    });
}
