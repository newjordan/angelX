use super::*;

// WriterJob::Save carries a bound history snapshot; SessionRecord is
// assembled on the session-saver thread.
const _: fn(SaveSnapshot) -> WriterJob = WriterJob::Save;

#[test]
fn checkpoint_returns_its_own_failure_with_a_newer_save_pending() {
    let root = std::env::temp_dir().join(format!("angel_sess_receipt_{}", gen_id()));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(root.clone(), "receipt".into(), &workspace);
    std::fs::create_dir(session.path()).unwrap();
    let (sender, receiver) = mpsc::sync_channel(2);
    let checkpoint_session = session.clone();
    let checkpoint_sender = sender.clone();
    let checkpoint = std::thread::spawn(move || {
        checkpoint_session.checkpoint_to(&checkpoint_sender, &[ChatMsg::user("checkpoint A")])
    });
    let WriterJob::Save(first) = receiver.recv_timeout(Duration::from_secs(1)).unwrap() else {
        panic!("expected first save");
    };
    let second = session
        .queue_history(&sender, Arc::from(vec![ChatMsg::user("save B")]))
        .unwrap();
    assert_eq!(session.save_status(), SessionSaveStatus::Pending);
    std::thread::Builder::new()
        .name("session-saver".into())
        .spawn(move || write_queued_snapshot(first))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(
        checkpoint.join().unwrap(),
        Err(SessionSaveError::BindingMismatch)
    );
    assert!(matches!(
        second.wait(Duration::ZERO),
        Err(SessionSaveError::WriterTimeout(_))
    ));
    assert_eq!(session.save_status(), SessionSaveStatus::Pending);
    std::fs::remove_dir(session.path()).unwrap();
    let WriterJob::Save(next) = receiver.recv_timeout(Duration::from_secs(1)).unwrap() else {
        panic!("expected second save");
    };
    std::thread::Builder::new()
        .name("session-saver".into())
        .spawn(move || write_queued_snapshot(next))
        .unwrap()
        .join()
        .unwrap();
    second.wait(Duration::ZERO).unwrap();
    session.flush_pending().unwrap();
    assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(&*retained[0].content, "save B");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn older_write_cannot_clear_a_newer_backlog_failure() {
    let root = std::env::temp_dir().join(format!("angel_sess_backlog_{}", gen_id()));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(root.clone(), "ordered-status".into(), &workspace);
    let (sender, receiver) = mpsc::sync_channel(1);
    let save = |text| {
        session
            .queue_history(&sender, Arc::from(vec![ChatMsg::user(text)]))
            .map(|_| ())
    };
    let drain_one = || {
        let WriterJob::Save(snapshot) = receiver.recv().unwrap() else {
            panic!("expected an actual queued snapshot");
        };
        std::thread::Builder::new()
            .name("session-saver".into())
            .spawn(move || write_queued_snapshot(snapshot))
            .unwrap()
            .join()
            .unwrap();
    };
    save("older accepted history").unwrap();
    assert_eq!(
        save("newer lost history"),
        Err(SessionSaveError::WriterBacklog)
    );
    drain_one();
    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(&*retained[0].content, "older accepted history");
    assert_eq!(
        session.save_status(),
        SessionSaveStatus::Failed(SessionSaveError::WriterBacklog)
    );
    save("successful retry of newest history").unwrap();
    drain_one();
    assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(&*retained[0].content, "successful retry of newest history");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn writer_queue_saturation_fails_fast_and_typed() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (first_reply, _first_done) = mpsc::channel();
    assert_eq!(
        enqueue_writer(&sender, WriterJob::Barrier(first_reply)),
        Ok(())
    );

    let (overflow_reply, _overflow_done) = mpsc::channel();
    assert_eq!(
        enqueue_writer(&sender, WriterJob::Barrier(overflow_reply)),
        Err(SessionSaveError::WriterBacklog),
        "a full writer queue must not block or retain another job"
    );

    drop(receiver);
    let (disconnected_reply, _disconnected_done) = mpsc::channel();
    assert_eq!(
        enqueue_writer(&sender, WriterJob::Barrier(disconnected_reply)),
        Err(SessionSaveError::WriterUnavailable)
    );
}

#[test]
fn session_roundtrip_and_list() {
    let dir = std::env::temp_dir().join(format!("angel_sess_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let workspace = dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let s = Session::at_for(dir.clone(), "0000000000001-1".to_string(), &workspace);
    let hist = vec![
        ChatMsg::system("orchestrator prompt"),
        ChatMsg::user("hello there"),
        ChatMsg::assistant("hi back"),
    ];
    s.save(&hist).unwrap();

    let loaded = load_for_path_for_test(s.path(), &workspace).unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(&*loaded[1].content, "hello there");
    assert_eq!(loaded[2].role, ChatRole::Assistant);

    let infos = list_dir_for(&dir, &workspace);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].id, "0000000000001-1");
    assert_eq!(infos[0].turns, 1);
    assert!(
        infos[0].preview.contains("hello there"),
        "got: {}",
        infos[0].preview
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saved_history_redacts_secrets_and_resumes_with_typed_tool_arguments() {
    let _lock = crate::tests::env_lock();
    let secret = "fixture-escaped-\"secret\"\\01234567";
    let _key = crate::tests::TestEnvGuard::set("ANGEL_T_SESSION_SECRET", secret);
    let dir = std::env::temp_dir().join(format!("angel_sess_redact_{}", std::process::id()));
    let workspace = dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(dir.clone(), "redacted".to_string(), &workspace);
    let mut message = ChatMsg::assistant("hf_abcdefghijklmnopqrstuvwxyz01234567");
    message.tool_calls = vec![crate::club::ToolCall {
            id: "call-1".to_string(),
            name: "fixture".to_string(),
            args: serde_json::json!({"access_token": "opaque-value", "count": 17, "ok": true, "nested": [secret, null]}),
        }].into();
    let history = vec![ChatMsg::user(secret), message];
    session.save(&history).unwrap();
    let loaded = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(&*loaded[0].content, "«redacted:ANGEL_T_SESSION_SECRET»");
    assert_eq!(&*loaded[1].content, crate::secrets::REDACTED);
    let args = &loaded[1].tool_calls[0].args;
    assert_eq!(args["access_token"], crate::secrets::REDACTED);
    assert_eq!(args["count"], 17);
    assert_eq!(args["ok"], true);
    assert_eq!(args["nested"][1], serde_json::Value::Null);
    assert_eq!(
        &*history[0].content, secret,
        "redaction does not edit live history"
    );
    session.save(&history).unwrap();
    assert!(load_for_path_for_test(session.path(), &workspace).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn session_binding_rejects_cross_project_and_legacy_records() {
    let dir = std::env::temp_dir().join(format!("angel_sess_scope_{}", std::process::id()));
    let alpha = dir.join("alpha");
    let beta = dir.join("beta");
    std::fs::create_dir_all(&alpha).unwrap();
    std::fs::create_dir_all(&beta).unwrap();

    let alpha_session = Session::at_for(dir.clone(), "alpha".to_string(), &alpha);
    alpha_session
        .save(&[ChatMsg::system("ALPHA_ONLY"), ChatMsg::user("alpha work")])
        .unwrap();
    assert_eq!(list_dir_for(&dir, &alpha).len(), 1);
    assert!(list_dir_for(&dir, &beta).is_empty());

    // Even an explicit/colliding sink id cannot overwrite a snapshot that
    // is already bound to another project.
    let colliding_beta = Session::at_for(dir.clone(), "alpha".to_string(), &beta);
    let mismatch = colliding_beta.save(&[
        ChatMsg::system("BETA_OVERWRITE"),
        ChatMsg::user("beta work"),
    ]);
    assert_eq!(mismatch, Err(SessionSaveError::BindingMismatch));
    let retained = load_for_path_for_test(alpha_session.path(), &alpha).unwrap();
    assert!(
        retained
            .iter()
            .any(|message| message.content.as_ref() == "ALPHA_ONLY")
    );
    assert!(
        !retained
            .iter()
            .any(|message| message.content.as_ref() == "BETA_OVERWRITE")
    );

    std::fs::write(
        dir.join("legacy.json"),
        serde_json::to_vec(&vec![ChatMsg::system("LEGACY_GLOBAL")]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        list_dir_for(&dir, &alpha).len(),
        1,
        "legacy global sessions must be inert"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

fn load_for_path_for_test(path: &Path, workspace: &Path) -> Result<Vec<ChatMsg>, String> {
    let record = load_record_path(path)?;
    matches_workspace(&record, workspace)
        .then_some(record.history)
        .ok_or_else(|| "project mismatch".to_string())
}

#[test]
fn empty_thread_writes_nothing() {
    let dir = std::env::temp_dir().join(format!("angel_sess_empty_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let s = Session::at(dir.clone(), "x".to_string());
    s.save(&[]).unwrap();
    assert!(!s.path().exists(), "empty thread must not create a file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn disabled_session_writes_nothing() {
    let s = Session::disabled();
    s.save(&[ChatMsg::system("preview only")]).unwrap();
    assert!(
        !s.path().exists(),
        "disabled session must not create a file"
    );
}

#[test]
fn nonempty_snapshot_requires_a_project_binding() {
    let dir = std::env::temp_dir().join(format!("angel_sess_unbound_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let session = Session::at(dir, "unbound".to_string());
    let result = session.save(&[ChatMsg::user("retain in memory")]);
    assert_eq!(result, Err(SessionSaveError::Unbound));
    assert_eq!(
        session.save_status(),
        SessionSaveStatus::Failed(SessionSaveError::Unbound)
    );
    assert!(!session.path().exists());
}

#[test]
fn failed_rewrite_retains_the_previous_good_snapshot() {
    let dir = std::env::temp_dir().join(format!("angel_sess_io_{}", std::process::id()));
    let workspace = dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(dir.clone(), "stable".to_string(), &workspace);
    session.save(&[ChatMsg::user("last good")]).unwrap();

    let tmp = session
        .path()
        .with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::create_dir(&tmp).unwrap();
    let result = session.save(&[ChatMsg::user("must not replace")]);
    assert!(matches!(result, Err(SessionSaveError::Write(_))));
    assert!(matches!(
        session.save_status(),
        SessionSaveStatus::Failed(SessionSaveError::Write(_))
    ));
    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(&*retained[0].content, "last good");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
#[test]
fn file_size_failure_reports_failed_retains_snapshot_and_cleans_temp() {
    const CHILD_ROOT: &str = "ANGEL_T_SESSION_EFBIG_ROOT";
    const FIXTURE_MARKER: &str = "owned-session-efbig-probe/v1";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("angel_sess_efbig_")
        );
        assert_eq!(
            std::fs::read_to_string(root.join("fixture-marker")).unwrap(),
            FIXTURE_MARKER
        );
        let workspace = root.join("workspace");
        let session = Session::at_for(root.join("sessions"), "probe".into(), &workspace);
        let original = std::fs::read(session.path()).unwrap();
        // This is kernel EFBIG injection, not a claim of a full filesystem.
        // Retain the hard ceiling and restore the original soft limit even
        // on panic, so coverage/profile output can complete during exit.
        struct RestoreLimit {
            limit: libc::rlimit,
            signal: libc::sighandler_t,
        }
        impl Drop for RestoreLimit {
            fn drop(&mut self) {
                unsafe {
                    let _ = libc::setrlimit(libc::RLIMIT_FSIZE, &self.limit);
                    libc::signal(libc::SIGXFSZ, self.signal);
                }
            }
        }
        let _restore = unsafe {
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE, &mut limit), 0);
            let signal = libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            assert_ne!(signal, libc::SIG_ERR);
            RestoreLimit { limit, signal }
        };
        let limited = libc::rlimit {
            rlim_cur: 65_536,
            rlim_max: _restore.limit.rlim_max,
        };
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limited) }, 0);
        let error = session
            .checkpoint(&[ChatMsg::user("new snapshot ".repeat(30_000))])
            .unwrap_err();
        let expected = std::io::Error::from_raw_os_error(libc::EFBIG).to_string();
        assert!(
            matches!(&error, SessionSaveError::Write(detail) if detail == &expected),
            "{error}"
        );
        assert_eq!(
            session.save_status(),
            SessionSaveStatus::Failed(error.clone())
        );
        assert_eq!(session.flush_pending(), Err(error));
        assert_eq!(
            std::fs::read(session.path()).unwrap(),
            original,
            "failed snapshot must not replace committed bytes"
        );
        let temporary = session
            .path()
            .with_extension(format!("json.{}.tmp", std::process::id()));
        assert!(
            !temporary.exists(),
            "EFBIG left a partial temporary snapshot: {}",
            temporary.display()
        );
        return;
    }

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    struct ReapChild(std::process::Child);
    impl Drop for ReapChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!("angel_sess_efbig_{}", gen_id())));
    let workspace = fixture.0.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(fixture.0.join("fixture-marker"), FIXTURE_MARKER).unwrap();
    let session = Session::at_for(fixture.0.join("sessions"), "probe".into(), &workspace);
    session
        .save(&[ChatMsg::user("last complete checkpoint")])
        .unwrap();
    let original = std::fs::read(session.path()).unwrap();
    let log = std::fs::File::create(fixture.0.join("child.log")).unwrap();
    let mut child = ReapChild(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::file_size_failure_reports_failed_retains_snapshot_and_cleans_temp",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CHILD_ROOT, &fixture.0)
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap(),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "owned EFBIG probe timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(
        status.success(),
        "owned EFBIG probe failed: {}",
        std::fs::read_to_string(fixture.0.join("child.log")).unwrap()
    );
    assert_eq!(std::fs::read(session.path()).unwrap(), original);
    assert!(
        std::fs::read_dir(session.path().parent().unwrap())
            .unwrap()
            .flatten()
            .all(|entry| entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "tmp"))
    );
}

#[test]
fn background_writer_publishes_io_failure() {
    let root = std::env::temp_dir().join(format!("angel_sess_bg_io_{}", std::process::id()));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let blocked = root.join("not-a-directory");
    std::fs::write(&blocked, "file").unwrap();
    let session = Session::at_for(blocked, "sink".to_string(), &workspace);
    session
        .save_async(&[ChatMsg::user("conversation remains live")])
        .unwrap();
    let result = session.flush_pending();
    assert!(matches!(result, Err(SessionSaveError::CreateDirectory(_))));
    assert!(matches!(
        session.save_status(),
        SessionSaveStatus::Failed(SessionSaveError::CreateDirectory(_))
    ));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn save_async_enqueues_a_bound_snapshot_not_a_session_record() {
    let root = std::env::temp_dir().join(format!(
        "angel_sess_off_ui_{}_{}",
        std::process::id(),
        gen_id()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(root.clone(), "off-ui".to_string(), &workspace);
    let history = vec![
        ChatMsg::user("queued without a caller-built record"),
        ChatMsg::assistant("writer assembled the document"),
    ];
    session.save_async(&history).unwrap();
    session.flush_pending().unwrap();

    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(retained.len(), 2);
    assert_eq!(
        &*retained[0].content,
        "queued without a caller-built record"
    );
    assert_eq!(&*retained[1].content, "writer assembled the document");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn save_history_roundtrips_a_shared_arc_snapshot() {
    let root = std::env::temp_dir().join(format!(
        "angel_sess_shared_arc_{}_{}",
        std::process::id(),
        gen_id()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(root.clone(), "shared-arc".to_string(), &workspace);
    let history: Arc<[ChatMsg]> = Arc::from(vec![
        ChatMsg::user("shared with spawn"),
        ChatMsg::assistant("same snapshot"),
    ]);
    session.save_history(Arc::clone(&history)).unwrap();
    session.flush_pending().unwrap();

    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(retained.len(), 2);
    assert_eq!(&*retained[0].content, &*history[0].content);
    assert_eq!(&*retained[1].content, &*history[1].content);
    assert_eq!(retained[0].role, ChatRole::User);
    assert_eq!(retained[1].role, ChatRole::Assistant);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn async_flush_barrier_makes_the_latest_snapshot_immediately_loadable() {
    let root = std::env::temp_dir().join(format!(
        "angel_sess_flush_{}_{}",
        std::process::id(),
        gen_id()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(root.clone(), "ordered".to_string(), &workspace);
    session
        .save_async(&[
            ChatMsg::user("latest queued user turn"),
            ChatMsg::assistant("latest queued assistant turn"),
        ])
        .unwrap();

    session.flush_pending().unwrap();

    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(retained.len(), 2);
    assert_eq!(&*retained[0].content, "latest queued user turn");
    assert_eq!(&*retained[1].content, "latest queued assistant turn");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn checkpoint_lands_after_an_older_queued_snapshot() {
    let root = std::env::temp_dir().join(format!(
        "angel_sess_checkpoint_{}_{}",
        std::process::id(),
        gen_id()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(root.clone(), "checkpoint".to_string(), &workspace);
    session
        .save_async(&[ChatMsg::user("older queued snapshot")])
        .unwrap();

    session
        .checkpoint(&[
            ChatMsg::user("explicit checkpoint"),
            ChatMsg::assistant("confirmed durable"),
        ])
        .unwrap();

    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(retained.len(), 2);
    assert_eq!(&*retained[0].content, "explicit checkpoint");
    assert_eq!(&*retained[1].content, "confirmed durable");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn blocking_save_lands_after_an_older_queued_snapshot() {
    let root = std::env::temp_dir().join(format!(
        "angel_sess_ordered_save_{}_{}",
        std::process::id(),
        gen_id()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let session = Session::at_for(root.clone(), "ordered-save".to_string(), &workspace);
    session
        .save_async(&[ChatMsg::user("older queued snapshot")])
        .unwrap();

    session
        .save(&[
            ChatMsg::user("newer boundary snapshot"),
            ChatMsg::assistant("must remain newest"),
        ])
        .unwrap();

    let retained = load_for_path_for_test(session.path(), &workspace).unwrap();
    assert_eq!(retained.len(), 2);
    assert_eq!(&*retained[0].content, "newer boundary snapshot");
    assert_eq!(&*retained[1].content, "must remain newest");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resume_repairs_a_trailing_tool_intent_as_outcome_unknown() {
    let calls = vec![crate::club::ToolCall {
        id: "effect-1".to_string(),
        name: "shell".to_string(),
        args: serde_json::json!({"cmd": "external-side-effect"}),
    }];
    let repaired = repair_interrupted_tool_batch(vec![
        ChatMsg::user("perform it"),
        ChatMsg::assistant_calls(calls),
    ]);

    assert_eq!(repaired.len(), 3);
    assert_eq!(repaired[2].role, ChatRole::Tool);
    assert_eq!(repaired[2].tool_call_id.as_deref(), Some("effect-1"));
    assert!(repaired[2].content.contains("outcome unknown"));
    assert!(repaired[2].content.contains("before retrying"));

    let unchanged = vec![ChatMsg::user("hello"), ChatMsg::assistant("done")];
    let retained = repair_interrupted_tool_batch(unchanged);
    assert_eq!(retained.len(), 2);
    assert_eq!(&*retained[0].content, "hello");
    assert_eq!(&*retained[1].content, "done");
}
