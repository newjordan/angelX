//! Exit retries reuse one exact writer request; ordinary checkpoint ordering stays separate.
use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-exit-ticket-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(path.join("alpha")).unwrap();
        std::fs::create_dir(path.join("beta")).unwrap();
        Self(path)
    }
    fn session(&self) -> Session {
        Session::at_for(
            self.0.join("sessions"),
            "owned".into(),
            &self.0.join("alpha"),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn perform(job: WriterJob) {
    std::thread::Builder::new()
        .name("session-saver".into())
        .spawn(move || match job {
            WriterJob::Save(snapshot) => write_queued_snapshot(snapshot),
            _ => panic!("unexpected barrier"),
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn exit_identical_pending_and_successful_retry_has_one_actual_writer_request() {
    let _lock = crate::tests::env_lock();
    let _env = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, receiver) = mpsc::sync_channel(2);
    let history: Arc<[ChatMsg]> = Arc::from([ChatMsg::user("exact pending operator")]);
    let first = session
        .queue_history(&sender, Arc::clone(&history))
        .unwrap();
    let only_job = receiver.try_recv().expect("one ordinary writer request");
    let retry = session
        .queue_history_with_reuse(&sender, Arc::clone(&history), true)
        .unwrap();
    assert!(Arc::ptr_eq(&first, &retry));
    assert!(
        matches!(receiver.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "pending retry cannot enqueue a duplicate"
    );
    perform(only_job);
    assert_eq!(first.status(), SessionSaveStatus::Healthy);
    let completed = session
        .queue_history_with_reuse(&sender, Arc::clone(&history), true)
        .unwrap();
    assert!(Arc::ptr_eq(&first, &completed));
    assert!(
        matches!(receiver.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "successful retry cannot enqueue a duplicate"
    );
    let record = load_record_path(session.path()).unwrap();
    assert_eq!(record.history[0].content.as_ref(), "exact pending operator");
    assert!(
        Arc::ptr_eq(
            &session
                .save_status
                .lock()
                .unwrap()
                .latest_snapshot
                .as_ref()
                .unwrap()
                .history,
            &history
        ),
        "publication retains one shared snapshot, not serialized prompt copies"
    );
    // Ordinary saves still create a fresh ticket, even for identical content.
    let ordinary = session
        .queue_history(&sender, Arc::clone(&history))
        .unwrap();
    assert!(!Arc::ptr_eq(&first, &ordinary));
    perform(receiver.try_recv().unwrap());
}

#[test]
fn exit_failed_identical_attempt_retries_and_publishes_exact_new_snapshot() {
    let _lock = crate::tests::env_lock();
    let _env = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, receiver) = mpsc::sync_channel(2);
    let history: Arc<[ChatMsg]> = Arc::from([ChatMsg::user("retry me exactly")]);
    std::fs::create_dir_all(session.path().parent().unwrap()).unwrap();
    let temporary = session
        .path()
        .with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::create_dir(&temporary).unwrap();
    let first = session
        .queue_history_with_reuse(&sender, Arc::clone(&history), true)
        .unwrap();
    perform(receiver.try_recv().unwrap());
    assert!(matches!(
        first.status(),
        SessionSaveStatus::Failed(SessionSaveError::Write(_))
    ));
    std::fs::remove_dir(&temporary).unwrap();
    let retry = session
        .queue_history_with_reuse(&sender, history, true)
        .unwrap();
    assert!(!Arc::ptr_eq(&first, &retry));
    perform(
        receiver
            .try_recv()
            .expect("failed identical save needs an actual new writer request"),
    );
    assert_eq!(retry.status(), SessionSaveStatus::Healthy);
    assert_eq!(
        load_record_path(session.path()).unwrap().history[0]
            .content
            .as_ref(),
        "retry me exactly"
    );
}

#[test]
fn exit_reuse_rejects_changed_history_and_same_text_in_another_project() {
    let _lock = crate::tests::env_lock();
    let _env = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, receiver) = mpsc::sync_channel(2);
    let history: Arc<[ChatMsg]> = Arc::from([ChatMsg::user("same text")]);
    let first = session
        .queue_history_with_reuse(&sender, Arc::clone(&history), true)
        .unwrap();
    drop(receiver.try_recv().unwrap());
    let changed = session
        .queue_history_with_reuse(&sender, Arc::from([ChatMsg::user("new text")]), true)
        .unwrap();
    assert!(!Arc::ptr_eq(&first, &changed));
    drop(
        receiver
            .try_recv()
            .expect("different history creates a new writer request"),
    );
    let before_project = session
        .queue_history_with_reuse(&sender, Arc::clone(&history), true)
        .unwrap();
    drop(receiver.try_recv().unwrap());
    let mut other = session.clone();
    other.bind(&fixture.0.join("beta"));
    let foreign = other
        .queue_history_with_reuse(&sender, history, true)
        .unwrap();
    assert!(!Arc::ptr_eq(&before_project, &foreign));
    match receiver
        .try_recv()
        .expect("different project creates a new writer request")
    {
        WriterJob::Save(snapshot) => assert_eq!(snapshot.workspace, fixture.0.join("beta")),
        _ => panic!("unexpected barrier"),
    }
}

#[test]
fn exit_comparison_covers_every_persisted_message_field() {
    let mut original = ChatMsg::user("operator");
    original.tool_calls = Arc::from([crate::agent::club::ToolCall {
        id: "call".into(),
        name: "check".into(),
        args: serde_json::json!({"scope":"owned"}),
    }]);
    original.attachments = Arc::from([crate::agent::club::Media::Image {
        mime: "image/png".into(),
        b64: "AA==".into(),
    }]);
    original.tool_call_id = Some("result".into());
    original
        .recovery_context
        .push(crate::agent::club::owned_recovery_context_ref());
    let encoded = serde_json::to_value(&original).unwrap();
    let fields: std::collections::BTreeSet<_> = encoded
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        fields,
        [
            "role",
            "content",
            "attachments",
            "tool_calls",
            "tool_call_id",
            "recovery_context"
        ]
        .into_iter()
        .collect(),
        "new persisted fields require exit-comparison review"
    );
    let cloned: ChatMsg = serde_json::from_value(encoded.clone()).unwrap();
    assert!(same_persisted_history(
        std::slice::from_ref(&original),
        &[cloned]
    ));
    for field in [
        "role",
        "content",
        "attachments",
        "tool_calls",
        "tool_call_id",
        "recovery_context",
    ] {
        let mut changed = encoded.clone();
        changed[field] = match field {
            "role" => serde_json::json!("Assistant"),
            "content" => serde_json::json!("different operator"),
            "attachments" => serde_json::json!([{"Audio":{"format":"wav","b64":"AQ=="}}]),
            "tool_calls" => {
                serde_json::json!([{"id":"call","name":"check","args":{"scope":"different"}}])
            }
            "tool_call_id" => serde_json::json!("other result"),
            "recovery_context" => serde_json::json!([]),
            _ => unreachable!(),
        };
        let changed: ChatMsg = serde_json::from_value(changed).unwrap();
        assert!(
            !same_persisted_history(std::slice::from_ref(&original), &[changed]),
            "{field}"
        );
    }
    let mut transient = original.clone();
    transient.private_reasoning = Some(Arc::from("provider-private transient reasoning"));
    assert!(same_persisted_history(&[original], &[transient]));
}

#[test]
fn exit_equal_json_numbers_with_different_bytes_require_a_new_writer_request() {
    let _lock = crate::tests::env_lock();
    let _env = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, receiver) = mpsc::sync_channel(2);
    let mut original = ChatMsg::user("preserve exact persisted tool arguments");
    original.tool_calls = Arc::from([crate::agent::club::ToolCall {
        id: "owned-call".into(),
        name: "owned-tool".into(),
        args: serde_json::from_str(r#"{"nested":[{"zero":-0.0}]}"#).unwrap(),
    }]);
    let old_history: Arc<[ChatMsg]> = Arc::from([original.clone()]);
    let first = session
        .queue_history_with_reuse(&sender, old_history, true)
        .unwrap();
    perform(receiver.try_recv().expect("original actual writer request"));
    assert_eq!(first.status(), SessionSaveStatus::Healthy);
    let old_bytes = std::fs::read(session.path()).unwrap();
    let mut changed = original.clone();
    changed.tool_calls = Arc::from([crate::agent::club::ToolCall {
        id: "owned-call".into(),
        name: "owned-tool".into(),
        args: serde_json::from_str(r#"{"nested":[{"zero":0.0}]}"#).unwrap(),
    }]);
    assert_eq!(
        original.tool_calls[0].args, changed.tool_calls[0].args,
        "negative control must pass Value's cheap structural equality"
    );
    assert_ne!(
        serde_json::to_vec(&original.tool_calls[0].args).unwrap(),
        serde_json::to_vec(&changed.tool_calls[0].args).unwrap()
    );
    let expected_args = serde_json::to_vec(&changed.tool_calls[0].args).unwrap();
    let changed_history: Arc<[ChatMsg]> = Arc::from([changed]);
    let second = session
        .queue_history_with_reuse(&sender, Arc::clone(&changed_history), true)
        .unwrap();
    assert!(
        !Arc::ptr_eq(&first, &second),
        "different persisted number bytes must not reuse the healthy old ticket"
    );
    perform(
        receiver
            .try_recv()
            .expect("changed serialized args require an actual writer request"),
    );
    assert_eq!(second.status(), SessionSaveStatus::Healthy);
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    let new_bytes = std::fs::read(session.path()).unwrap();
    assert_ne!(old_bytes, new_bytes);
    let recovered = load_record_path(session.path()).unwrap();
    assert_eq!(
        serde_json::to_vec(&recovered.history[0].tool_calls[0].args).unwrap(),
        expected_args
    );
    // Decode into distinct Arc allocations so exact-equal arguments exercise
    // serialization equality as well as the ordinary pointer fast path.
    let exact_clone: Vec<ChatMsg> =
        serde_json::from_slice(&serde_json::to_vec(changed_history.as_ref()).unwrap()).unwrap();
    let third = session
        .queue_history_with_reuse(&sender, Arc::from(exact_clone), true)
        .unwrap();
    assert!(Arc::ptr_eq(&second, &third));
    assert!(
        matches!(receiver.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "exact persisted encoding reuses one successful writer request"
    );
}

#[test]
fn exit_recovery_context_change_requires_new_durable_checkpoint() {
    let _lock = crate::tests::env_lock();
    let _env = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, receiver) = mpsc::sync_channel(2);
    let original = ChatMsg::user("same visible recovery summary");
    let first = session
        .queue_history_with_reuse(&sender, Arc::from([original.clone()]), true)
        .unwrap();
    perform(receiver.try_recv().unwrap());
    assert_eq!(first.status(), SessionSaveStatus::Healthy);
    let reference = crate::agent::club::owned_recovery_context_ref();
    let mut changed_reference = reference.clone();
    changed_reference.summary_sha256 = crate::knowledge::cut::sha256_hex(b"new origin binding");
    let mut previous = first;
    for references in [vec![reference], vec![changed_reference], vec![]] {
        let mut changed = original.clone();
        changed.recovery_context = references.clone();
        let history: Arc<[ChatMsg]> = Arc::from([changed]);
        let next = session
            .queue_history_with_reuse(&sender, Arc::clone(&history), true)
            .unwrap();
        assert!(
            !Arc::ptr_eq(&previous, &next),
            "changed serialized origin cannot reuse the old healthy receipt"
        );
        let pending_retry = session
            .queue_history_with_reuse(&sender, Arc::clone(&history), true)
            .unwrap();
        assert!(
            Arc::ptr_eq(&next, &pending_retry),
            "identical pending retry still reuses exactly one write"
        );
        perform(
            receiver
                .try_recv()
                .expect("changed origin requires actual disk write"),
        );
        assert_eq!(next.status(), SessionSaveStatus::Healthy);
        let recovered = load_record_path(session.path()).unwrap();
        assert_eq!(recovered.history[0].content, original.content);
        assert_eq!(recovered.history[0].recovery_context, references);
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        previous = next;
    }
}
