//! Existing session status semantics expose unsupported path serialization.
#![cfg(unix)]
use super::*;
use std::os::unix::ffi::OsStringExt as _;

#[test]
fn invalid_workspace_serialization_preserves_checkpoint_and_publishes_failure() {
    let _lock = crate::tests::env_lock();
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
            "angel-session-invalid-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    let valid = fixture.0.join("valid");
    let raw = fixture
        .0
        .join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    std::fs::create_dir_all(&valid).unwrap();
    std::fs::create_dir(&raw).unwrap();
    let _env = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
    let directory = fixture.0.join("sessions");
    let prior = Session::at_for(directory.clone(), "owned".into(), &valid);
    prior
        .save(&[ChatMsg::user("last durable operator")])
        .unwrap();
    let old = std::fs::read(prior.path()).unwrap();
    let mut session = Session::at_for(directory.clone(), "owned".into(), &raw);
    let history = [ChatMsg::user("new operator must remain live")];
    session.save_async(&history).unwrap();
    let error = session.flush_pending().unwrap_err();
    assert!(
        matches!(&error, SessionSaveError::Serialize(detail) if detail.contains("invalid UTF-8"))
    );
    assert_eq!(
        session.save_status(),
        SessionSaveStatus::Failed(error.clone())
    );
    assert_eq!(std::fs::read(session.path()).unwrap(), old);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    assert_eq!(history[0].content.as_ref(), "new operator must remain live");
    // Returning to a supported binding is an ordinary successful retry; failure
    // does not poison the shared writer or falsely overwrite its later status.
    session.bind(&valid);
    session.checkpoint(&history).unwrap();
    assert_eq!(session.save_status(), SessionSaveStatus::Healthy);
    assert_ne!(std::fs::read(session.path()).unwrap(), old);
}
