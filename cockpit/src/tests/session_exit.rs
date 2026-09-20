//! Ordinary exit keeps failed history available and final cleanup preserves errors.
use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-session-exit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(path.join("workspace")).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exit_failed_save_keeps_exact_history_available_for_export_and_retry() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let mut app = seed_preview_app();
    let workspace = fixture.0.join("workspace");
    app.tools = Arc::new(harness::ToolRegistry::with_team(
        workspace.clone(),
        Vec::new(),
    ));
    app.session = session::Session::at_for(fixture.0.join("sessions"), "owned".into(), &workspace);
    app.history = vec![
        ChatMsg::system("static policy"),
        ChatMsg::user("old durable operator"),
    ];
    app.session.checkpoint(&app.history).unwrap();
    let old = std::fs::read(app.session.path()).unwrap();
    let pending_text = "EXACT_UNLAUNCHED_EXIT_OPERATOR\nnaïve 日本語 e\u{301} END".repeat(257);
    let queued_text = "EXACT_QUEUED_EXIT_OPERATOR\n🧭 final byte".repeat(257);
    app.submit_deferral = true;
    app.input = pending_text.clone();
    app.submit();
    assert!(app.pending_turn.is_some());
    app.input = queued_text.clone();
    app.submit();
    assert_eq!(app.steer_queue.len(), 1);
    let mut expected = app.history.clone();
    expected.push(ChatMsg::user(pending_text.trim()));
    expected.push(ChatMsg::user(queued_text.as_str()));
    let history = serde_json::to_value(&expected).unwrap();
    let original_id = app.session.id.clone();
    let temporary = app
        .session
        .path()
        .with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::create_dir(&temporary).unwrap();
    app.input = "/exit".into();
    app.submit();
    assert!(!app.should_quit);
    assert!(app.pending_turn.is_none());
    assert!(app.thinking.is_none());
    assert!(app.steer_queue.is_empty());
    assert_eq!(serde_json::to_value(&app.history).unwrap(), history);
    assert_eq!(app.session.id, original_id);
    assert_eq!(std::fs::read(app.session.path()).unwrap(), old);
    let response = &app.messages.last().unwrap().text;
    assert!(
        response.contains("boundary blocked")
            && response.contains("/raw")
            && response.contains("retry /exit"),
        "{response}"
    );
    assert!(matches!(
        app.session.save_status(),
        session::SessionSaveStatus::Failed(session::SessionSaveError::Write(_))
    ));
    app.input = "/raw".into();
    app.submit();
    let export = fixture.0.join(".angel0/transcript.txt");
    let exported = std::fs::read_to_string(&export).unwrap();
    assert_eq!(exported.matches(pending_text.trim()).count(), 1);
    assert_eq!(exported.matches(queued_text.as_str()).count(), 1);
    assert_eq!(serde_json::to_value(&app.history).unwrap(), history);
    std::fs::remove_dir(&temporary).unwrap();
    for _ in 0..3 {
        app.advance();
        assert!(!app.should_quit, "failed exit requires explicit retry");
        assert_eq!(std::fs::read(app.session.path()).unwrap(), old);
        assert!(app.thinking.is_none() && app.pending_turn.is_none());
    }
    app.input = "/exit".into();
    app.submit();
    assert!(app.should_quit);
    assert!(app.pending_turn.is_none() && app.thinking.is_none());
    assert!(app.steer_queue.is_empty());
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(app.session.path()).unwrap()).unwrap();
    assert_eq!(record["history"], history);
    assert_eq!(
        app.session.save_status(),
        session::SessionSaveStatus::Healthy
    );
}

#[test]
fn final_flush_error_names_checkpoint_and_uncommitted_staging() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let _env = TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let session = session::Session::at_for(
        fixture.0.join("sessions"),
        "owned".into(),
        &fixture.0.join("workspace"),
    );
    std::fs::create_dir_all(session.path().parent().unwrap()).unwrap();
    let temporary = session
        .path()
        .with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::create_dir(&temporary).unwrap();
    session
        .save_async(&[ChatMsg::user("pending exact user")])
        .unwrap();
    let error = flush_session_for_exit(&session).unwrap_err().to_string();
    assert!(
        error.contains("exit flush failed")
            && error.contains("committed checkpoint")
            && error.contains("uncommitted staging"),
        "{error}"
    );
    assert!(
        error.contains(session.path().to_str().unwrap())
            && error.contains(temporary.to_str().unwrap()),
        "{error}"
    );
    std::fs::remove_dir(&temporary).unwrap();
    session
        .checkpoint_for_exit(&[ChatMsg::user("pending exact user")])
        .unwrap();
    flush_session_for_exit(&session).unwrap();
}

#[test]
fn final_flush_failure_never_masks_the_original_run_loop_error() {
    let original = std::io::Error::from_raw_os_error(libc::EIO);
    let original_text = original.to_string();
    let error = finish_exit_results(
        Err(original),
        Err(std::io::Error::other("session flush timed out")),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        std::io::Error::from_raw_os_error(libc::EIO).kind()
    );
    assert!(
        error.to_string().contains(&original_text)
            && error.to_string().contains("session flush timed out")
    );
    let cause = error
        .get_ref()
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.raw_os_error(), Some(libc::EIO));
    assert!(finish_exit_results(Ok(()), Ok(())).is_ok());
    assert_eq!(
        finish_exit_results(Err(std::io::Error::from_raw_os_error(libc::EIO)), Ok(()))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EIO)
    );
    assert_eq!(
        finish_exit_results(Ok(()), Err(std::io::Error::from_raw_os_error(libc::ENOSPC)))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOSPC)
    );
}

fn bind_owned_session(app: &mut App, fixture: &Fixture) {
    let workspace = fixture.0.join("workspace");
    app.tools = Arc::new(harness::ToolRegistry::with_team(
        workspace.clone(),
        Vec::new(),
    ));
    app.session = session::Session::at_for(fixture.0.join("sessions"), "owned".into(), &workspace);
    app.history = vec![
        ChatMsg::system("static policy"),
        ChatMsg::user("old durable operator"),
    ];
    app.session.checkpoint(&app.history).unwrap();
}

#[test]
fn exit_preserves_live_writer_and_its_unpublished_queue() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let (mut app, worker) = seed_live_streaming_app(Vec::new());
    bind_owned_session(&mut app, &fixture);
    let sentinel = "EXACT_LIVE_QUEUED_OPERATOR\n東京 🧭 e\u{301}";
    app.input = sentinel.into();
    app.submit();
    assert_eq!(app.steer_queue.len(), 1);
    let before_history = serde_json::to_value(&app.history).unwrap();
    let before_disk = std::fs::read(app.session.path()).unwrap();
    let cancel = Arc::clone(&app.thinking.as_ref().unwrap().cancel);
    // Queueing a steer may already interrupt a provider wait. Exit must not
    // change that cancellation state or discard the still-owned writer.
    let cancelled_before_exit = cancel.load(std::sync::atomic::Ordering::Relaxed);
    app.input = "/exit".into();
    app.submit();
    assert!(!app.should_quit);
    assert!(app.thinking.is_some());
    assert_eq!(
        cancel.load(std::sync::atomic::Ordering::Relaxed),
        cancelled_before_exit
    );
    assert_eq!(app.steer_queue.len(), 1);
    assert_eq!(serde_json::to_value(&app.history).unwrap(), before_history);
    assert_eq!(std::fs::read(app.session.path()).unwrap(), before_disk);
    assert_eq!(
        app.session.save_status(),
        session::SessionSaveStatus::Healthy
    );
    let response = &app.messages.last().unwrap().text;
    assert!(
        response.contains("turn is still active")
            && response.contains("1 accepted message(s) still queued"),
        "{response}"
    );
    assert!(
        response.contains("Will save and close when existing work finishes")
            && response.contains("/raw exports current history only"),
        "{response}"
    );
    // No model is spawned: the held sender is the actual App worker channel.
    // Taking the queue here is test inspection only, after the guarded operation.
    let queued = app.steer_queue.drain();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].content.as_ref(), sentinel);
    drop(worker);
}

#[test]
fn exit_after_stop_and_real_drain_retains_followup_without_a_new_turn() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let (mut app, worker) = seed_live_streaming_app(Vec::new());
    bind_owned_session(&mut app, &fixture);
    for _ in 0..2 {
        app.input = "/stop".into();
        app.submit();
    }
    assert!(app.thinking.as_ref().is_some_and(Thinking::is_draining));
    let sentinel = "EXACT_POST_STOP_FOLLOWUP\n東京 🧭 e\u{301}";
    app.input = sentinel.into();
    app.submit();
    assert_eq!(app.steer_queue.len(), 1);
    let before = serde_json::to_value(&app.history).unwrap();
    app.input = "/exit".into();
    app.submit();
    assert!(!app.should_quit);
    assert!(app.thinking.as_ref().is_some_and(Thinking::is_draining));
    assert_eq!(app.steer_queue.len(), 1);
    assert_eq!(serde_json::to_value(&app.history).unwrap(), before);
    assert!(
        app.messages
            .last()
            .unwrap()
            .text
            .contains("worker is still draining")
    );
    // A real channel terminal receipt releases the existing App lifecycle;
    // this does not replace it with a model or manually clear the flight slot.
    worker
        .send(Err("owned stopped worker terminal receipt".into()))
        .unwrap();
    app.advance();
    assert!(app.thinking.is_none());
    assert_eq!(app.steer_queue.len(), 1);
    app.advance(); // The retained exit request owns the next idle boundary.
    assert!(app.should_quit);
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(app.steer_queue.is_empty());
    assert_eq!(
        app.history
            .iter()
            .filter(|m| m.role == ChatRole::User && m.content.as_ref() == sentinel)
            .count(),
        1
    );
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(app.session.path()).unwrap()).unwrap();
    assert_eq!(
        record["history"],
        serde_json::to_value(&app.history).unwrap()
    );
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
        session::SessionSaveStatus::Healthy
    );
}

#[test]
fn fully_idle_exit_checkpoints_without_requiring_work_or_a_model() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let mut app = seed_preview_app();
    bind_owned_session(&mut app, &fixture);
    app.history
        .push(ChatMsg::user("EXACT_IDLE_UNSAVED_OPERATOR"));
    let expected = serde_json::to_value(&app.history).unwrap();
    app.input = "/exit".into();
    app.submit();
    assert!(app.should_quit);
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(app.session.path()).unwrap()).unwrap();
    assert_eq!(record["history"], expected);
    flush_session_for_exit(&app.session).unwrap();
}

#[test]
fn graceful_exit_saves_the_completed_history_before_any_followup_or_loop_launch() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let loop_path = fixture.0.join("loop.json");
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
        TestEnvGuard::set("ANGEL_LOOP_FILE", loop_path.to_str().unwrap()),
    ];
    let (mut app, worker) = seed_live_streaming_app(Vec::new());
    bind_owned_session(&mut app, &fixture);
    let sentinel = "EXACT_GRACEFUL_FOLLOWUP\n東京 🧭 e\u{301}";
    app.input = sentinel.into();
    app.submit();
    assert_eq!(app.steer_queue.len(), 1);
    app.input = "/exit".into();
    app.submit();
    assert!(!app.should_quit);
    assert_eq!(
        app.exit_request,
        Some(crate::app::ExitRequest::WaitingForIdle)
    );
    let mut completed = app.history.clone();
    completed.push(ChatMsg::assistant("owned completed answer"));
    worker
        .send(Ok((
            completed.clone(),
            "owned completed answer".into(),
            crate::club::RouteIdentity {
                driver: "practice".into(),
                model: None,
                reasoning_effort: None,
            },
            crate::harness::TurnStopReason::Answer,
        )))
        .unwrap();
    app.advance();
    assert!(app.thinking.is_none());
    assert_eq!(
        serde_json::to_value(&app.history).unwrap(),
        serde_json::to_value(&completed).unwrap()
    );
    assert!(!app.should_quit);
    // Exercise both automatic admission entry points with the slot now free.
    app.flush_queued_steers();
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    app.loop_ctl.status = loop_ctl::LoopStatus::Running;
    app.loop_ctl.task = "must not launch during exit".into();
    app.loop_ctl.wake_at = Some(Instant::now());
    app.loop_arm();
    assert!(app.thinking.is_none() && !app.loop_ctl.awaiting_turn);
    app.advance();
    assert!(app.should_quit);
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(app.steer_queue.is_empty());
    completed.push(ChatMsg::user(sentinel));
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(app.session.path()).unwrap()).unwrap();
    assert_eq!(record["history"], serde_json::to_value(&completed).unwrap());
    assert_eq!(
        app.loop_ctl.status,
        loop_ctl::LoopStatus::Running,
        "exit intent is process-local; it does not rewrite saved loop policy"
    );
}

#[test]
fn graceful_exit_keeps_status_available_and_new_submitted_input_cancels_it() {
    let _lock = env_lock();
    let fixture = Fixture::new();
    let _env = [
        TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0"),
    ];
    let (mut app, worker) = seed_live_streaming_app(Vec::new());
    bind_owned_session(&mut app, &fixture);
    let cancel = Arc::clone(&app.thinking.as_ref().unwrap().cancel);
    app.input = "/exit".into();
    app.submit();
    app.input = "/status".into();
    app.submit();
    assert_eq!(
        app.exit_request,
        Some(crate::app::ExitRequest::WaitingForIdle)
    );
    assert!(!cancel.load(std::sync::atomic::Ordering::Acquire));
    app.input = "continue with this newly accepted input".into();
    app.submit();
    assert!(app.exit_request.is_none() && !app.should_quit);
    assert!(app.thinking.is_some());
    assert_eq!(app.steer_queue.len(), 1);
    assert!(
        app.messages
            .iter()
            .any(|m| m.text.contains("exit cancelled by new input"))
    );
    drop(worker);
}
