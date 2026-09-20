use super::*;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};

#[test]
fn snapshot_publication_uses_the_companion_locks_pinned_parent() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = std::env::temp_dir().join(format!(
        "angel-atlas-private-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let fixture = Fixture(root);
    let store = fixture.0.join("store");
    let held = fixture.0.join("held");
    let outside = fixture.0.join("outside");
    std::fs::create_dir(&outside).unwrap();
    let path = store.join("snapshot.json");
    let lock = FileLock::acquire(&lock_path(&path)).unwrap();
    std::fs::rename(&store, &held).unwrap();
    symlink(&outside, &store).unwrap();
    write_snapshot(&lock.directory, &path, &serde_json::json!({"fixture":true})).unwrap();
    assert_eq!(
        std::fs::metadata(held.join("snapshot.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(held.join("snapshot.json.lock"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(held.join("snapshot.json")).unwrap()
        )
        .unwrap(),
        serde_json::json!({"fixture":true})
    );
    assert_eq!(std::fs::read_dir(outside).unwrap().count(), 0);
}
