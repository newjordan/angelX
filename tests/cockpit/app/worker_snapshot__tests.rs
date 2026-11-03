use super::*;

#[test]
fn snapshot_pins_all_files_and_cleans_up() {
    let root = std::env::temp_dir().join(format!("angel-snapshot-test-{}", std::process::id()));
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(root.join("entry.mjs"), "entry").unwrap();
    std::fs::write(root.join("lib/rules.mjs"), "old").unwrap();
    let snapshot = Snapshot::capture(&root, &["entry.mjs", "lib/rules.mjs"], "fixture").unwrap();
    std::fs::write(root.join("lib/rules.mjs"), "new").unwrap();
    assert_eq!(
        std::fs::read(snapshot.dir.join("lib/rules.mjs")).unwrap(),
        b"old"
    );
    let dir = snapshot.dir.clone();
    drop(snapshot);
    assert!(!dir.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_untrusted_paths_and_oversized_files() {
    let root = std::env::temp_dir().join(format!("angel-snapshot-bad-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    assert!(Snapshot::capture(&root, &["../outside"], "fixture").is_err());
    assert!(Snapshot::capture(&root, &["/absolute"], "fixture").is_err());
    assert!(Snapshot::capture(&root, &["same", "same"], "fixture").is_err());
    std::fs::write(root.join("large"), vec![0; MAX_MODULE as usize + 1]).unwrap();
    assert!(
        Snapshot::capture(&root, &["large"], "fixture")
            .unwrap_err()
            .contains("byte limit")
    );
    std::fs::remove_dir_all(root).unwrap();
}
