//! Owned queue/phase faults; diagnostic timeout never replaces an I/O receipt.
use super::*;

fn load_for_path_for_test(path: &Path, workspace: &Path) -> Result<Vec<ChatMsg>, String> {
    let record = load_record_path(path)?;
    matches_workspace(&record, workspace)
        .then_some(record.history)
        .ok_or_else(|| "project mismatch".to_string())
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("angel_writer_diag_{}", gen_id()));
        std::fs::create_dir_all(root.join("workspace")).unwrap();
        Self(root)
    }
    fn session(&self) -> Session {
        Session::at_for(
            self.0.join("sessions"),
            "owned".into(),
            &self.0.join("workspace"),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn timeout(attempt: &SaveAttempt) -> SessionWriterTimeout {
    match attempt.wait(Duration::from_millis(10)) {
        Err(SessionSaveError::WriterTimeout(detail)) => detail,
        other => panic!("expected held receipt timeout, got {other:?}"),
    }
}

#[test]
fn queued_timeout_names_its_exact_attempt_and_contains_no_payload_or_path() {
    let _env_lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, _receiver) = mpsc::sync_channel(1);
    let attempt = session
        .queue_history(&sender, Arc::from(vec![ChatMsg::user("PRIVATE_PAYLOAD界")]))
        .unwrap();
    let detail = timeout(&attempt);
    assert_eq!(detail.attempt_id, attempt.id);
    assert_eq!(detail.phase, SessionWritePhase::Queued);
    assert_eq!(detail.queue_ms, detail.elapsed_ms);
    assert_eq!(detail.phase_ms, detail.elapsed_ms);
    assert!(detail.elapsed_ms >= 10);
    assert_eq!(attempt.status(), SessionSaveStatus::Pending);
    let text = SessionSaveError::WriterTimeout(detail).to_string();
    for expected in [
        "flush timed out",
        "attempt=",
        "elapsed_ms=",
        "queue_ms=",
        "phase=queued",
        "phase_ms=",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(!text.contains("PRIVATE_PAYLOAD"));
    assert!(!text.contains(fixture.0.to_str().unwrap()));
    assert!(!session.path().exists());
}

#[test]
fn each_real_writer_phase_can_be_distinguished_from_queue_wait() {
    let _env_lock = crate::tests::env_lock();
    let phases = [
        SessionWritePhase::Assemble,
        SessionWritePhase::CreateDirectory,
        SessionWritePhase::Serialize,
        SessionWritePhase::CheckBinding,
        SessionWritePhase::OpenTemporary,
        SessionWritePhase::WriteTemporary,
        SessionWritePhase::SyncFile,
        SessionWritePhase::Rename,
        #[cfg(unix)]
        SessionWritePhase::SyncDirectory,
    ];
    for held_phase in phases {
        let fixture = Fixture::new();
        let session = fixture.session();
        let (sender, receiver) = mpsc::sync_channel(1);
        let attempt = session
            .queue_history(
                &sender,
                Arc::from(vec![ChatMsg::user("original exact receipt界")]),
            )
            .unwrap();
        let queued = timeout(&attempt);
        let WriterJob::Save(snapshot) = receiver.recv().unwrap() else {
            panic!("save expected")
        };
        let (reached, phase_ready) = mpsc::sync_channel(1);
        let (release, resume) = mpsc::sync_channel(1);
        let writer = std::thread::Builder::new()
            .name("session-saver".into())
            .spawn(move || {
                write_queued_snapshot_observed(snapshot, |phase| {
                    if phase == held_phase {
                        reached.send(()).unwrap();
                        resume.recv_timeout(Duration::from_secs(2)).unwrap();
                    }
                });
            })
            .unwrap();
        phase_ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let active = timeout(&attempt);
        assert_eq!(active.attempt_id, queued.attempt_id);
        assert_eq!(active.phase, held_phase);
        assert!(active.queue_ms >= queued.queue_ms);
        assert!(active.phase_ms >= 10);
        assert!(active.elapsed_ms >= active.queue_ms + active.phase_ms);
        assert_eq!(attempt.status(), SessionSaveStatus::Pending);
        release.send(()).unwrap();
        writer.join().unwrap();
        attempt.wait(Duration::ZERO).unwrap();
        let history = load_for_path_for_test(session.path(), &fixture.0.join("workspace")).unwrap();
        assert_eq!(&*history[0].content, "original exact receipt界");
        assert_eq!(
            active.phase, held_phase,
            "late completion cannot alter a returned timeout"
        );
    }
}

#[test]
fn frozen_timeout_survives_newer_save_and_late_original_failure() {
    let _env_lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, receiver) = mpsc::sync_channel(2);
    let first = session
        .queue_history(&sender, Arc::from(vec![ChatMsg::user("first")]))
        .unwrap();
    let original_error = SessionSaveError::WriterTimeout(timeout(&first));
    let frozen_text = original_error.to_string();
    let second = session
        .queue_history(&sender, Arc::from(vec![ChatMsg::user("second")]))
        .unwrap();
    assert_ne!(first.id, second.id);
    let WriterJob::Save(first_snapshot) = receiver.recv().unwrap() else {
        panic!("save expected")
    };
    std::fs::create_dir_all(session.path()).unwrap();
    std::thread::Builder::new()
        .name("session-saver".into())
        .spawn(move || write_queued_snapshot(first_snapshot))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(
        first.wait(Duration::ZERO),
        Err(SessionSaveError::BindingMismatch)
    );
    assert_eq!(session.save_status(), SessionSaveStatus::Pending);
    assert_eq!(original_error.to_string(), frozen_text);
    std::fs::remove_dir(session.path()).unwrap();
    let WriterJob::Save(second_snapshot) = receiver.recv().unwrap() else {
        panic!("save expected")
    };
    std::thread::Builder::new()
        .name("session-saver".into())
        .spawn(move || write_queued_snapshot(second_snapshot))
        .unwrap()
        .join()
        .unwrap();
    second.wait(Duration::ZERO).unwrap();
    assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
    assert_eq!(
        first.wait(Duration::ZERO),
        Err(SessionSaveError::BindingMismatch)
    );
    assert_eq!(original_error.to_string(), frozen_text);
    let history = load_for_path_for_test(session.path(), &fixture.0.join("workspace")).unwrap();
    assert_eq!(&*history[0].content, "second");
}

#[test]
fn actual_queued_timeout_reaches_harness_notice_without_starting_provider() {
    let _env_lock = crate::tests::env_lock();
    use crate::agent::club::{Club, ClubReply, ToolDef};
    use crate::agent::harness::{ToolRegistry, TurnEvent, run_turn_steered_checkpointed};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct CountingClub(AtomicUsize);
    impl Club for CountingClub {
        fn respond(&self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn label(&self) -> &str {
            "owned-checkpoint-counting-club"
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ClubReply::Text("must not start".into()))
        }
    }
    let fixture = Fixture::new();
    let session = fixture.session();
    let (sender, receiver) = mpsc::sync_channel(1);
    let club = CountingClub(AtomicUsize::new(0));
    let mut history = vec![ChatMsg::user("owned pre-provider checkpoint")];
    let (events, notices) = mpsc::channel();
    let failure = run_turn_steered_checkpointed(
        &club,
        &ToolRegistry::new(),
        &mut history,
        &AtomicBool::new(false),
        Some(1),
        &events,
        None,
        &|prefix| {
            session
                .queue_history(&sender, Arc::from(prefix))
                .and_then(|attempt| attempt.wait(Duration::from_millis(10)))
                .map_err(|error| error.to_string())
        },
    )
    .unwrap_err();
    assert_eq!(club.0.load(Ordering::SeqCst), 0);
    assert!(
        failure.contains("provider request not started") && failure.contains("phase=queued"),
        "{failure}"
    );
    assert!(notices.try_iter().any(|event| matches!(event,
        TurnEvent::Notice(message) if message.contains("phase=queued") && message.contains("attempt="))));
    let WriterJob::Save(snapshot) = receiver.recv().unwrap() else {
        panic!("save expected")
    };
    let exact_receipt = Arc::clone(&snapshot.attempt);
    std::thread::Builder::new()
        .name("session-saver".into())
        .spawn(move || write_queued_snapshot(snapshot))
        .unwrap()
        .join()
        .unwrap();
    exact_receipt.wait(Duration::ZERO).unwrap();
    assert_eq!(
        club.0.load(Ordering::SeqCst),
        0,
        "late disk success cannot dispatch a stopped turn"
    );
    let durable = load_for_path_for_test(session.path(), &fixture.0.join("workspace")).unwrap();
    assert!(
        durable
            .iter()
            .any(|m| m.content.as_ref() == "owned pre-provider checkpoint")
    );
}
