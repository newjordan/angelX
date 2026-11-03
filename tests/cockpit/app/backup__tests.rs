//! Tiered snapshot / restore behavior for `platform::backup`.
use super::*;

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    target: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "angel-backup-test-{}-{}-{serial}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("state");
        let target = root.join("vault");
        std::fs::create_dir_all(source.join("sessions")).unwrap();
        std::fs::create_dir_all(source.join("trajectories")).unwrap();
        std::fs::create_dir_all(source.join("proc")).unwrap();
        std::fs::write(source.join("sessions/one.json"), b"{\"turn\":1}").unwrap();
        std::fs::write(source.join("trajectories/big.bin"), vec![7_u8; 4096]).unwrap();
        std::fs::write(source.join("proc/live.json"), b"runtime").unwrap();
        std::fs::write(source.join("cockpit.err"), b"noise").unwrap();
        std::fs::write(source.join("settings.toml"), b"key = 1").unwrap();
        Self {
            root,
            source,
            target,
        }
    }

    fn snapshot_dir(&self, name: &str) -> PathBuf {
        self.target.join("snapshots").join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn inode(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata(path).unwrap().ino()
}

#[test]
fn core_tier_excludes_bulk_runtime_and_droppings() {
    let fixture = Fixture::new();
    let (name, stats) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    let dir = fixture.snapshot_dir(&name);
    assert!(dir.join("sessions/one.json").is_file());
    assert!(dir.join("settings.toml").is_file());
    assert!(!dir.join("trajectories/big.bin").exists());
    assert!(!dir.join("proc/live.json").exists());
    assert!(!dir.join("cockpit.err").exists());
    assert_eq!(stats.linked, 0);
    let manifest = verify_snapshot(&dir).unwrap();
    assert_eq!(manifest.tier, "core");
    assert_eq!(manifest.files.len(), stats.files);
}

#[test]
fn core_backup_restores_pinned_session_and_its_private_book_generation() {
    use crate::agent::club::ChatMsg;
    use crate::agent::harness::book::{catalog, continuity, generation_store};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new();
    let workspace = crate::tests::TestGitWorkspace::new("backup-book-generation");
    let source_store = fixture.source.join("book-generations");
    let _source_store = generation_store::TestRootGuard::enter(source_store);
    let original = catalog::Catalog::baseline().bytes();
    let digest = generation_store::publish(&original).unwrap();
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user("task")];
    continuity::bind_generation(&mut history, &digest).unwrap();
    let sessions = fixture.source.join("sessions");
    let session = crate::knowledge::session::Session::at_for(
        sessions,
        "pinned-session".into(),
        workspace.path(),
    );
    session.save(&history).unwrap();

    let (name, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    let snapshot = fixture.snapshot_dir(&name);
    let stored = format!("book-generations/{digest}.json");
    assert!(snapshot.join(&stored).is_file());
    let restored = fixture.root.join("restored");
    restore_local(&snapshot, &restored, false).unwrap();
    let generation = restored.join(&stored);
    assert_eq!(std::fs::read(&generation).unwrap(), original);
    assert_eq!(std::fs::metadata(&generation).unwrap().nlink(), 1);
    assert_eq!(
        std::fs::metadata(&generation).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(restored.join("book-generations"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    let _restored_store = generation_store::TestRootGuard::enter(restored.join("book-generations"));
    let _sessions = crate::tests::TestEnvGuard::set(
        "ANGEL_SESSION_DIR",
        restored.join("sessions").to_str().unwrap(),
    );
    let mut recovered =
        crate::knowledge::session::load_for("pinned-session", workspace.path()).unwrap();
    assert_eq!(
        continuity::generation_digest(&recovered),
        Some(digest.as_str())
    );
    assert_eq!(
        catalog::bind_turn(&mut recovered).unwrap().bytes(),
        original
    );
}

#[test]
fn backup_captures_generation_published_after_source_inventory() {
    use crate::agent::club::ChatMsg;
    use crate::agent::harness::book::{catalog, continuity, generation_store};

    let _guard = crate::tests::env_lock();
    let fixture = Fixture::new();
    let workspace = crate::tests::TestGitWorkspace::new("backup-book-concurrent");
    let source_store = fixture.source.join("book-generations");
    let _source_store = generation_store::TestRootGuard::enter(source_store);
    let session = crate::knowledge::session::Session::at_for(
        fixture.source.join("sessions"),
        "pinned-session".into(),
        workspace.path(),
    );
    let mut history = vec![ChatMsg::system("system"), ChatMsg::user("task")];
    // The session filename is already inventoried, but its generation does
    // not exist until after the backup has finished discovering source paths.
    session.save(&history).unwrap();
    let original = catalog::Catalog::baseline().bytes();
    let mut published = String::new();
    let (name, _) =
        snapshot_local_after_collection(&fixture.source, &fixture.target, Tier::Core, || {
            published = generation_store::publish(&original).unwrap();
            continuity::bind_generation(&mut history, &published).unwrap();
            session.save(&history).unwrap();
        })
        .unwrap();
    let snapshot = fixture.snapshot_dir(&name);
    verify_snapshot(&snapshot).unwrap();
    assert_eq!(
        std::fs::read(snapshot.join(format!("book-generations/{published}.json"))).unwrap(),
        original
    );
    let restored = fixture.root.join("restored-after-race");
    restore_local(&snapshot, &restored, false).unwrap();
    let _store = generation_store::TestRootGuard::enter(restored.join("book-generations"));
    let _sessions = crate::tests::TestEnvGuard::set(
        "ANGEL_SESSION_DIR",
        restored.join("sessions").to_str().unwrap(),
    );
    let mut recovered =
        crate::knowledge::session::load_for("pinned-session", workspace.path()).unwrap();
    assert_eq!(
        continuity::generation_digest(&recovered),
        Some(published.as_str())
    );
    assert_eq!(
        catalog::bind_turn(&mut recovered).unwrap().bytes(),
        original
    );
}

#[test]
fn backup_rejects_session_with_missing_generation() {
    use crate::agent::club::ChatMsg;
    use crate::agent::harness::book::continuity;

    let fixture = Fixture::new();
    let workspace = crate::tests::TestGitWorkspace::new("backup-book-missing");
    let session = crate::knowledge::session::Session::at_for(
        fixture.source.join("sessions"),
        "missing-generation".into(),
        workspace.path(),
    );
    let mut history = vec![ChatMsg::user("task")];
    continuity::bind_generation(&mut history, &"a".repeat(64)).unwrap();
    session.save(&history).unwrap();
    let error = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap_err();
    assert!(error.contains("session generation"), "{error}");
    assert!(latest_snapshot_dir(&fixture.target).is_none());
}

#[test]
fn restore_rejects_a_symlinked_book_generation_directory() {
    let fixture = Fixture::new();
    let source = fixture.source.join("book-generations");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("generation.json"), b"saved generation").unwrap();
    let (name, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    let outside = fixture.root.join("outside");
    let destination = fixture.root.join("restored");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(&destination).unwrap();
    std::os::unix::fs::symlink(&outside, destination.join("book-generations")).unwrap();
    let error = restore_local(&fixture.snapshot_dir(&name), &destination, true).unwrap_err();
    assert!(error.contains("book generation directory"), "{error}");
    assert!(!outside.join("generation.json").exists());
}

#[test]
fn unchanged_files_hardlink_and_changed_files_recopy() {
    let fixture = Fixture::new();
    let (first, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Bulk).unwrap();
    // Snapshot names embed creation ms; force a distinct second name.
    std::thread::sleep(std::time::Duration::from_millis(2));
    std::fs::write(fixture.source.join("sessions/one.json"), b"{\"turn\":2}").unwrap();
    let (second, stats) = snapshot_local(&fixture.source, &fixture.target, Tier::Bulk).unwrap();
    assert_ne!(first, second);
    let first_dir = fixture.snapshot_dir(&first);
    let second_dir = fixture.snapshot_dir(&second);
    // trajectories/big.bin is unchanged: same inode across snapshots.
    assert_eq!(
        inode(&first_dir.join("trajectories/big.bin")),
        inode(&second_dir.join("trajectories/big.bin"))
    );
    assert!(
        stats.linked >= 1,
        "unchanged file should hardlink: {stats:?}"
    );
    // sessions/one.json changed: new inode, new content; old snapshot intact.
    assert_ne!(
        inode(&first_dir.join("sessions/one.json")),
        inode(&second_dir.join("sessions/one.json"))
    );
    assert_eq!(
        std::fs::read(first_dir.join("sessions/one.json")).unwrap(),
        b"{\"turn\":1}"
    );
    assert_eq!(
        std::fs::read(second_dir.join("sessions/one.json")).unwrap(),
        b"{\"turn\":2}"
    );
    verify_snapshot(&second_dir).unwrap();
    let latest = latest_snapshot_dir(&fixture.target).unwrap();
    assert_eq!(latest, second_dir);
}

#[test]
fn tampered_snapshot_fails_verification() {
    let fixture = Fixture::new();
    let (name, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    let dir = fixture.snapshot_dir(&name);
    // Hardlinked/owned file: rewrite through a fresh inode to simulate bitrot
    // without touching the source.
    let path = dir.join("sessions/one.json");
    let replacement = dir.join("sessions/.tampered");
    std::fs::write(&replacement, b"{\"turn\":999}").unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    let error = verify_snapshot(&dir).unwrap_err();
    assert!(error.contains("hash mismatch"), "{error}");
}

#[test]
fn restore_refuses_differing_files_without_force() {
    let fixture = Fixture::new();
    let (name, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    let dir = fixture.snapshot_dir(&name);
    let live = fixture.source.join("sessions/one.json");
    std::fs::write(&live, b"{\"turn\":50}").unwrap();
    std::fs::remove_file(fixture.source.join("settings.toml")).unwrap();

    let stats = restore_local(&dir, &fixture.source, false).unwrap();
    assert_eq!(stats.refused, 1);
    assert_eq!(std::fs::read(&live).unwrap(), b"{\"turn\":50}");
    // A missing file restores even without force.
    assert_eq!(
        std::fs::read(fixture.source.join("settings.toml")).unwrap(),
        b"key = 1"
    );

    let stats = restore_local(&dir, &fixture.source, true).unwrap();
    assert!(stats.restored >= 1);
    assert_eq!(std::fs::read(&live).unwrap(), b"{\"turn\":1}");

    // Second forced pass: everything already identical.
    let stats = restore_local(&dir, &fixture.source, true).unwrap();
    assert_eq!(stats.restored, 0);
    assert!(stats.identical >= 2);
}

#[test]
fn prune_keeps_newest_and_never_latest() {
    let fixture = Fixture::new();
    let mut names = Vec::new();
    for turn in 1..=3 {
        std::fs::write(
            fixture.source.join("sessions/one.json"),
            format!("{{\"turn\":{turn}}}"),
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let (name, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
        names.push(name);
    }
    let removed = prune_local(&fixture.target, 1).unwrap();
    assert_eq!(removed.len(), 2);
    let remaining = list_snapshots(&fixture.target);
    assert_eq!(remaining, vec![names[2].clone()]);
    verify_snapshot(&fixture.snapshot_dir(&names[2])).unwrap();
}

#[test]
fn target_parsing_distinguishes_local_and_remote() {
    assert!(matches!(
        parse_target("/var/tmp/vault").unwrap(),
        Target::Local(_)
    ));
    assert!(matches!(
        parse_target("backups/angel").unwrap(),
        Target::Local(_)
    ));
    match parse_target("jordan@casper:/srv/angel-backups").unwrap() {
        Target::Remote { ssh, path } => {
            assert_eq!(ssh, "jordan@casper");
            assert_eq!(path, "/srv/angel-backups");
        }
        other => panic!("expected remote, got {other:?}"),
    }
    // A bare ssh alias (user/hostname live in ssh_config) is remote too.
    match parse_target("casper:/home/user/angel-backups").unwrap() {
        Target::Remote { ssh, path } => {
            assert_eq!(ssh, "casper");
            assert_eq!(path, "/home/user/angel-backups");
        }
        other => panic!("expected remote, got {other:?}"),
    }
    match parse_target("rsync://jordan@casper:/srv/angel-backups").unwrap() {
        Target::Remote { ssh, path } => {
            assert_eq!(ssh, "jordan@casper");
            assert_eq!(path, "/srv/angel-backups");
        }
        other => panic!("expected remote, got {other:?}"),
    }
}

#[test]
fn fetch_latest_file_verifies_against_the_manifest() {
    let fixture = Fixture::new();
    snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    // Point the slot at the fixture vault through the config file.
    std::fs::write(
        fixture.source.join("backup.json"),
        serde_json::json!({
            "schema": BACKUP_CONFIG_SCHEMA,
            "target": fixture.target.to_string_lossy(),
        })
        .to_string(),
    )
    .unwrap();
    let staging = fixture.root.join("fetch-staging");
    let fetched = fetch_latest_file(&fixture.source, "sessions/one.json", &staging).unwrap();
    assert_eq!(std::fs::read(&fetched).unwrap(), b"{\"turn\":1}");
    // A file the snapshot does not hold is refused, not invented.
    let error = fetch_latest_file(&fixture.source, "sessions/missing.json", &staging).unwrap_err();
    assert!(
        error.contains("fetch") || error.contains("not in"),
        "{error}"
    );
}

#[test]
fn unconfigured_backup_is_dormant_not_an_error() {
    let fixture = Fixture::new();
    // No backup.json in the fixture source, no flags: status reports dormant.
    let text = dispatch(&fixture.source, &["status".to_string()]).unwrap();
    assert!(text.contains("not configured"), "{text}");
}

#[test]
fn a_hash_mismatched_snapshot_is_rejected_and_never_restored() {
    let fixture = Fixture::new();
    let (name, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    let dir = fixture.snapshot_dir(&name);
    // Bitrot in the snapshot: swap a new inode in so the source is untouched.
    let replacement = dir.join("sessions/.bitrot");
    std::fs::write(&replacement, b"{\"turn\":999}").unwrap();
    std::fs::rename(&replacement, dir.join("sessions/one.json")).unwrap();
    // Live state differs from the snapshot, so --force would overwrite it if
    // the snapshot were trusted at all.
    std::fs::write(fixture.source.join("sessions/one.json"), b"{\"turn\":50}").unwrap();
    std::fs::remove_file(fixture.source.join("settings.toml")).unwrap();

    let error = restore_local(&dir, &fixture.source, true).unwrap_err();
    assert!(error.contains("hash mismatch"), "{error}");
    assert_eq!(
        std::fs::read(fixture.source.join("sessions/one.json")).unwrap(),
        b"{\"turn\":50}"
    );
    assert!(!fixture.source.join("settings.toml").exists());
}

#[test]
fn fetch_never_hands_back_a_snapshot_file_with_the_wrong_hash() {
    let fixture = Fixture::new();
    let (name, _) = snapshot_local(&fixture.source, &fixture.target, Tier::Core).unwrap();
    std::fs::write(
        fixture.source.join("backup.json"),
        serde_json::json!({
            "schema": BACKUP_CONFIG_SCHEMA,
            "target": fixture.target.to_string_lossy(),
        })
        .to_string(),
    )
    .unwrap();
    let snapshot_file = fixture.snapshot_dir(&name).join("sessions/one.json");
    std::fs::write(&snapshot_file, b"{\"turn\":666}").unwrap();

    let staging = fixture.root.join("fetch-staging");
    let error = fetch_latest_file(&fixture.source, "sessions/one.json", &staging).unwrap_err();
    assert!(error.contains("manifest verification"), "{error}");
    assert!(!staging.join("one.json").exists());
    assert_eq!(
        std::fs::read(fixture.source.join("sessions/one.json")).unwrap(),
        b"{\"turn\":1}"
    );
}
