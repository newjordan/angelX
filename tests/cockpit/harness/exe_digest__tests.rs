use super::*;
use std::cell::Cell;
use std::os::unix::fs::PermissionsExt;

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-exe-digest-{tag}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn memo(&self) -> PathBuf {
        self.0.join("memo")
    }
    fn image(&self, bytes: &[u8]) -> File {
        std::fs::write(self.0.join("image"), bytes).unwrap();
        File::open(self.0.join("image")).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Far enough ahead that a file made a moment ago counts as quiet.
fn later() -> SystemTime {
    SystemTime::now() + Duration::from_secs(60)
}

fn sha(file: &mut File) -> std::io::Result<String> {
    crate::agent::harness::workspace_state::sha256_reader_hex(file)
}

fn counted<'a>(calls: &'a Cell<u32>) -> impl FnOnce(&mut File) -> std::io::Result<String> + 'a {
    move |file| {
        calls.set(calls.get() + 1);
        sha(file)
    }
}

fn entries(memo: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(memo)
        .map(|rows| rows.flatten().map(|row| row.path()).collect())
        .unwrap_or_default()
}

#[test]
fn a_second_run_on_the_same_image_reuses_the_digest_without_reading_it() {
    let dir = Scratch::new("hit");
    let calls = Cell::new(0);
    let mut file = dir.image(b"first build");
    let cold = digest_at(Some(&dir.memo()), &mut file, later(), counted(&calls)).unwrap();
    assert_eq!(cold, crate::knowledge::cut::sha256_hex(b"first build"));
    assert_eq!(calls.get(), 1);

    let mut again = File::open(dir.0.join("image")).unwrap();
    let warm = digest_at(Some(&dir.memo()), &mut again, later(), counted(&calls)).unwrap();
    assert_eq!(warm, cold);
    assert_eq!(calls.get(), 1, "the warm run must not hash the image");
}

#[test]
fn a_rebuilt_image_misses_and_gets_its_own_digest() {
    let dir = Scratch::new("rebuilt");
    let calls = Cell::new(0);
    let mut old = dir.image(b"old build");
    let old_digest = digest_at(Some(&dir.memo()), &mut old, later(), counted(&calls)).unwrap();

    // Same path, same length, new bytes: the change time moves, so the key does.
    std::thread::sleep(Duration::from_millis(20));
    let mut new = dir.image(b"new build");
    let new_digest = digest_at(Some(&dir.memo()), &mut new, later(), counted(&calls)).unwrap();
    assert_eq!(calls.get(), 2);
    assert_ne!(old_digest, new_digest);
    assert_eq!(new_digest, crate::knowledge::cut::sha256_hex(b"new build"));
    assert_eq!(entries(&dir.memo()).len(), 2);
}

#[test]
fn an_image_changed_a_moment_ago_is_hashed_but_not_remembered() {
    let dir = Scratch::new("racy");
    let calls = Cell::new(0);
    let mut file = dir.image(b"just written");
    // The real clock: the image was written well inside the quiet period.
    digest_at(
        Some(&dir.memo()),
        &mut file,
        SystemTime::now(),
        counted(&calls),
    )
    .unwrap();
    assert!(entries(&dir.memo()).is_empty());
    let mut again = File::open(dir.0.join("image")).unwrap();
    digest_at(
        Some(&dir.memo()),
        &mut again,
        SystemTime::now(),
        counted(&calls),
    )
    .unwrap();
    assert_eq!(calls.get(), 2);
}

#[test]
fn an_image_that_moved_while_it_was_read_is_not_remembered() {
    let dir = Scratch::new("moved");
    let mut file = dir.image(b"before");
    let path = dir.0.join("image");
    let digest = digest_at(Some(&dir.memo()), &mut file, later(), |file| {
        let digest = sha(file)?;
        std::fs::write(&path, b"rewritten while hashing, longer").unwrap();
        Ok(digest)
    })
    .unwrap();
    assert_eq!(digest, crate::knowledge::cut::sha256_hex(b"before"));
    assert!(entries(&dir.memo()).is_empty());
}

#[test]
fn an_unlinked_image_is_not_remembered() {
    let dir = Scratch::new("unlinked");
    let mut file = dir.image(b"running image");
    std::fs::remove_file(dir.0.join("image")).unwrap();
    digest_at(Some(&dir.memo()), &mut file, later(), sha).unwrap();
    assert!(entries(&dir.memo()).is_empty());
}

fn warm(dir: &Scratch) -> (String, PathBuf) {
    let mut file = dir.image(b"image bytes");
    let digest = digest_at(Some(&dir.memo()), &mut file, later(), sha).unwrap();
    let entry = entries(&dir.memo()).pop().expect("an entry was stored");
    (digest, entry)
}

fn assert_ignored(dir: &Scratch, why: &str) {
    let calls = Cell::new(0);
    let mut file = File::open(dir.0.join("image")).unwrap();
    let digest = digest_at(Some(&dir.memo()), &mut file, later(), counted(&calls)).unwrap();
    assert_eq!(calls.get(), 1, "{why}: the entry must not be trusted");
    assert_eq!(
        digest,
        crate::knowledge::cut::sha256_hex(b"image bytes"),
        "{why}"
    );
}

#[test]
fn a_damaged_or_foreign_entry_is_never_trusted() {
    let wrong = "0".repeat(64);
    let cases: [(&str, Box<dyn Fn(&Path, &str)>); 6] = [
        (
            "wrong digest length",
            Box::new(|entry, key| {
                std::fs::write(entry, format!("{SCHEMA} {key} abc\n")).unwrap();
            }),
        ),
        (
            "uppercase digest",
            Box::new(|entry, key| {
                std::fs::write(entry, format!("{SCHEMA} {key} {}\n", "A".repeat(64))).unwrap();
            }),
        ),
        (
            "names another key",
            Box::new(|entry, _| {
                std::fs::write(entry, format!("{SCHEMA} other-key {}\n", "1".repeat(64))).unwrap();
            }),
        ),
        (
            "another schema",
            Box::new(|entry, key| {
                std::fs::write(
                    entry,
                    format!("angel-exe-digest/v0 {key} {}\n", "1".repeat(64)),
                )
                .unwrap();
            }),
        ),
        (
            "trailing fields",
            Box::new(|entry, key| {
                std::fs::write(entry, format!("{SCHEMA} {key} {} extra\n", "1".repeat(64)))
                    .unwrap();
            }),
        ),
        (
            "open to group and others",
            Box::new(|entry, key| {
                std::fs::write(entry, format!("{SCHEMA} {key} {}\n", "1".repeat(64))).unwrap();
                std::fs::set_permissions(entry, std::fs::Permissions::from_mode(0o644)).unwrap();
            }),
        ),
    ];
    for (why, damage) in cases {
        let dir = Scratch::new("damaged");
        let (_, entry) = warm(&dir);
        let key = entry.file_name().unwrap().to_string_lossy().into_owned();
        damage(&entry, &key);
        assert_ignored(&dir, why);
    }
    // The wrong-but-well-formed digest for the right key would be believed:
    // that is what the private directory and the owner/mode checks defend.
    assert!(valid_digest(&wrong));
}

#[test]
fn an_entry_reached_through_a_symlink_is_never_trusted() {
    let dir = Scratch::new("symlink");
    let (_, entry) = warm(&dir);
    let forged = dir.0.join("forged");
    std::fs::write(&forged, format!("{SCHEMA} x {}\n", "1".repeat(64))).unwrap();
    std::fs::remove_file(&entry).unwrap();
    std::os::unix::fs::symlink(&forged, &entry).unwrap();
    assert_ignored(&dir, "entry is a symlink");
}

#[test]
fn a_directory_open_to_others_or_replaced_by_a_symlink_is_never_used() {
    let open = Scratch::new("open-dir");
    warm(&open);
    std::fs::set_permissions(open.memo(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_ignored(&open, "directory readable by others");
    assert_eq!(
        entries(&open.memo()).len(),
        1,
        "no new entry in a shared directory"
    );

    let linked = Scratch::new("link-dir");
    warm(&linked);
    let real = linked.0.join("real");
    std::fs::rename(linked.memo(), &real).unwrap();
    std::os::unix::fs::symlink(&real, linked.memo()).unwrap();
    assert_ignored(&linked, "directory is a symlink");
}

#[test]
fn the_memo_is_created_private() {
    let dir = Scratch::new("private");
    let (_, entry) = warm(&dir);
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir.memo()) & 0o077, 0);
    assert_eq!(mode(&entry) & 0o077, 0);
}

#[test]
fn no_directory_means_every_run_hashes() {
    let dir = Scratch::new("off");
    let calls = Cell::new(0);
    for _ in 0..2 {
        let mut file = dir.image(b"image bytes");
        digest_at(None, &mut file, later(), counted(&calls)).unwrap();
    }
    assert_eq!(calls.get(), 2);
}

#[test]
fn only_the_newest_entries_are_kept() {
    let dir = Scratch::new("prune");
    for round in 0..(KEEP + 4) {
        let mut file = dir.image(format!("build {round}").as_bytes());
        digest_at(Some(&dir.memo()), &mut file, later(), sha).unwrap();
        std::thread::sleep(Duration::from_millis(3));
    }
    assert_eq!(entries(&dir.memo()).len(), KEEP);
}

#[test]
fn the_memo_is_off_in_tests_unless_a_directory_is_named_and_can_be_disabled() {
    let _env = crate::tests::env_lock();
    let dir = Scratch::new("env");
    let _clear = crate::tests::TestEnvGuard::unset("ANGEL_EXE_DIGEST_DIR");
    let _on = crate::tests::TestEnvGuard::unset("ANGEL_EXE_DIGEST_CACHE");
    assert_eq!(default_dir(), None);
    let named = dir.memo();
    let _named = crate::tests::TestEnvGuard::set("ANGEL_EXE_DIGEST_DIR", named.to_str().unwrap());
    assert_eq!(default_dir(), Some(named));
    let _off = crate::tests::TestEnvGuard::set("ANGEL_EXE_DIGEST_CACHE", "0");
    assert_eq!(default_dir(), None);
}
