use super::*;

#[test]
fn installed_and_relocated_resources_do_not_depend_on_the_build_checkout() {
    let temp = crate::tests::TestGitWorkspace::new("resource-paths");
    let prefix = temp.path().join("prefix");
    let bundle = prefix.join("share/angelX/bundles/source-id");
    std::fs::create_dir_all(bundle.join("cockpit")).unwrap();
    std::fs::create_dir_all(bundle.join("scripts")).unwrap();
    let executable = prefix.join("bin/angel");
    let absent_build = Path::new("/absent/build/cockpit");
    assert_eq!(
        resolve_root(None, Some(&executable), Some(absent_build), "source-id"),
        bundle
    );
    let moved = temp.path().join("moved-source");
    std::fs::create_dir_all(moved.join("cockpit/target/release")).unwrap();
    std::fs::create_dir_all(moved.join("scripts")).unwrap();
    std::fs::write(moved.join("cockpit/Cargo.toml"), "").unwrap();
    assert_eq!(
        resolve_root(
            None,
            Some(&moved.join("cockpit/target/release/angel")),
            Some(absent_build),
            "source-id"
        ),
        moved
    );
    assert_eq!(
        resolve_root(
            Some(bundle.clone()),
            Some(&executable),
            Some(absent_build),
            "other-id"
        ),
        bundle
    );
}

#[test]
fn a_portable_build_without_a_source_tree_falls_back_to_the_working_folder() {
    assert_eq!(resolve_root(None, None, None, "source-id"), PathBuf::from("."));
}
