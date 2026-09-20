use super::*;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-session-private-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
    fn record(&self, content: &str) -> SessionRecord {
        SessionRecord {
            schema: SESSION_SCHEMA.into(),
            workspace: self.0.clone(),
            project_key: "private-fixture".into(),
            history: vec![ChatMsg::user(content)],
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exclusive_session_temporary_preserves_preexisting_entries_and_checkpoint() {
    let fixture = Fixture::new();
    let path = fixture.0.join("stable.json");
    write_snapshot(&path, &fixture.record("last good"), |_| {}).unwrap();
    let committed = std::fs::read(&path).unwrap();
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let target = fixture.0.join("untouched");
    std::fs::write(&target, b"untouched bytes").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    for kind in ["symlink", "hardlink", "regular"] {
        match kind {
            "symlink" => symlink(&target, &temporary).unwrap(),
            "hardlink" => std::fs::hard_link(&target, &temporary).unwrap(),
            _ => {
                std::fs::write(&temporary, b"foreign staging").unwrap();
                std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o664))
                    .unwrap();
            }
        }
        let before = std::fs::symlink_metadata(&temporary).unwrap();
        let mut entered_payload_write = false;
        let failure = write_snapshot(&path, &fixture.record("must not publish"), |phase| {
            entered_payload_write |= phase == SessionWritePhase::WriteTemporary;
        })
        .unwrap_err();
        assert!(matches!(failure, SessionSaveError::Write(_)), "{failure:?}");
        assert!(!entered_payload_write);
        let after = std::fs::symlink_metadata(&temporary).unwrap();
        assert_eq!((after.ino(), after.mode()), (before.ino(), before.mode()));
        assert_eq!(std::fs::read(&path).unwrap(), committed);
        assert_eq!(std::fs::read(&target).unwrap(), b"untouched bytes");
        assert_eq!(std::fs::metadata(&target).unwrap().mode() & 0o777, 0o640);
        std::fs::remove_file(&temporary).unwrap();
    }
    write_snapshot(&path, &fixture.record("retry publishes"), |_| {}).unwrap();
    assert_eq!(
        &*load_record_path(&path).unwrap().history[0].content,
        "retry publishes"
    );
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
}

#[test]
fn session_parent_swap_publishes_only_in_the_pinned_store() {
    let fixture = Fixture::new();
    let store = fixture.0.join("sessions");
    let held = fixture.0.join("held");
    let outside = fixture.0.join("outside");
    std::fs::create_dir(&outside).unwrap();
    let path = store.join("stable.json");
    write_snapshot(&path, &fixture.record("last good"), |_| {}).unwrap();
    write_snapshot(&path, &fixture.record("new snapshot"), |phase| {
        if phase == SessionWritePhase::WriteTemporary {
            std::fs::rename(&store, &held).unwrap();
            symlink(&outside, &store).unwrap();
        }
    })
    .unwrap();
    let record = load_record_path(&held.join("stable.json")).unwrap();
    assert_eq!(record.workspace, fixture.0);
    assert_eq!(record.project_key, "private-fixture");
    assert_eq!(&*record.history[0].content, "new snapshot");
    assert_eq!(std::fs::read_dir(outside).unwrap().count(), 0);
}

#[test]
fn session_rejects_substituted_temporary_without_following_or_unlinking_it() {
    let fixture = Fixture::new();
    let path = fixture.0.join("stable.json");
    write_snapshot(&path, &fixture.record("last good"), |_| {}).unwrap();
    let committed = std::fs::read(&path).unwrap();
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let moved = fixture.0.join("owned-moved-temp");
    let target = fixture.0.join("untouched");
    std::fs::write(&target, b"safe").unwrap();
    let result = write_snapshot(&path, &fixture.record("must not publish"), |phase| {
        if phase == SessionWritePhase::Rename {
            std::fs::rename(&temporary, &moved).unwrap();
            symlink(&target, &temporary).unwrap();
        }
    });
    assert!(matches!(result, Err(SessionSaveError::Rename(_))));
    assert!(
        std::fs::symlink_metadata(&temporary)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"safe");
    assert_eq!(std::fs::read(&path).unwrap(), committed);
}
