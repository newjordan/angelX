use super::*;

#[test]
fn k5c_absent_control_files_have_no_pin_entries() {
    let root = std::env::temp_dir().join(format!("angel-k5c-controls-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let controls = CargoControls::capture(&root, &root).unwrap();
    // Ancestor controls belong to the host fixture; absent local candidates
    // must never acquire entries, even when the parent has a toolchain pin.
    assert!(
        controls
            .files
            .iter()
            .all(|file| !file.path.starts_with(&root))
    );
    assert!(controls.lookup.iter().any(|path| path.starts_with(&root)));
    controls.revalidate().unwrap();
    std::fs::write(root.join("rust-toolchain"), "stable").unwrap();
    let error = controls.revalidate().unwrap_err();
    assert!(error.contains("changed since pin"), "{error}");
    assert!(error.contains(&root.display().to_string()), "{error}");
    std::fs::remove_dir_all(&root).unwrap();
    let error = CargoControls::capture(&root, &root).err().unwrap();
    assert!(
        error.contains("resolve Cargo control lookup directory"),
        "{error}"
    );
    assert!(error.contains(&root.display().to_string()), "{error}");
}
