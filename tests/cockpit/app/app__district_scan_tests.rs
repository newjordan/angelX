use super::scan_workspace_districts;
use std::path::PathBuf;

fn scratch() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "angel-district-scan-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create district scan fixture");
    root
}

#[test]
fn district_scan_filters_sorts_caps_and_is_deterministic() {
    let root = scratch();
    for skipped in [
        ".git",
        ".hidden",
        "target",
        "node_modules",
        "vendor",
        "dist",
        "build",
    ] {
        std::fs::create_dir(root.join(skipped)).unwrap();
    }
    std::fs::write(root.join("ordinary-file"), "not a district").unwrap();
    for index in (0..14).rev() {
        std::fs::create_dir(root.join(format!("ward-{index:02}"))).unwrap();
    }

    let expected = (0..12)
        .map(|index| format!("ward-{index:02}"))
        .collect::<Vec<_>>();
    let first = scan_workspace_districts(&root);
    assert_eq!(first, expected);
    assert_eq!(first, scan_workspace_districts(&root));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn district_scan_of_missing_root_is_empty_and_deterministic() {
    let root = scratch();
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(scan_workspace_districts(&root), Vec::<String>::new());
    assert_eq!(scan_workspace_districts(&root), Vec::<String>::new());
}
