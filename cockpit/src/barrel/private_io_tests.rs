use super::*;
use std::os::unix::fs::{DirBuilderExt, symlink};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-barrel-private-{}-{}",
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cap_eviction_and_append_stay_in_the_same_pinned_directory() {
    let fixture = Fixture::new();
    let store = fixture.0.join("store");
    let held = fixture.0.join("held");
    let outside = fixture.0.join("outside");
    let directory = crate::workspace_store::private_io::PrivateDirectory::open(&store).unwrap();
    std::fs::write(store.join("barrel-20000101.jsonl"), b"old\n").unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("barrel-20000101.jsonl"), b"untouched\n").unwrap();
    std::fs::rename(&store, &held).unwrap();
    symlink(&outside, &store).unwrap();
    let mut evicted = 0;
    assert!(matches!(
        append_capped_directory(
            &directory,
            "barrel-",
            std::ffi::OsStr::new("barrel-20000102.jsonl"),
            "new",
            4,
            &mut evicted
        ),
        Append::Ok
    ));
    assert_eq!(evicted, 1);
    assert!(!held.join("barrel-20000101.jsonl").exists());
    assert_eq!(
        std::fs::read(held.join("barrel-20000102.jsonl")).unwrap(),
        b"new\n"
    );
    assert_eq!(
        std::fs::read(outside.join("barrel-20000101.jsonl")).unwrap(),
        b"untouched\n"
    );
    assert!(!outside.join("barrel-20000102.jsonl").exists());
}

#[test]
fn cap_refuses_symlinked_shard_before_eviction_or_append() {
    let fixture = Fixture::new();
    let target = fixture.0.join("untouched");
    std::fs::write(&target, b"untouched\n").unwrap();
    let link = fixture.0.join("authored-20000101.jsonl");
    symlink(&target, &link).unwrap();
    let shard = fixture.0.join("authored-20000102.jsonl");
    let mut evicted = 0;
    assert!(matches!(
        append_capped_shard(&fixture.0, "authored-", &shard, "new", 64, &mut evicted),
        Append::IoError
    ));
    assert_eq!(evicted, 0);
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(target).unwrap(), b"untouched\n");
    assert!(!shard.exists());
}
