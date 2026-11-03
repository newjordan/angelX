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
    assert_eq!(
        resolve_root(None, None, None, "source-id"),
        PathBuf::from(".")
    );
}

#[test]
fn installed_bundle_supplies_reloadable_surface_manifests() {
    let temp = crate::tests::TestGitWorkspace::new("installed-module-resources");
    let prefix = temp.path().join("prefix");
    let bundle = prefix.join("share/angelX/bundles/source-id");
    let dir = bundle.join("cockpit/modules");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(bundle.join("scripts")).unwrap();
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("modules");
    for name in [
        "core.toml",
        "agent.toml",
        "artifacts.toml",
        "shell.toml",
        "image.toml",
        "graph.toml",
    ] {
        std::fs::copy(checkout.join(name), dir.join(name)).unwrap();
    }
    let path = dir.join("agent.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("Agent Identity", "Bundle agent");
    std::fs::write(path, text).unwrap();
    let resolved = resolve_root(
        None,
        Some(&prefix.join("bin/angel")),
        Some(Path::new("/absent/cockpit")),
        "source-id",
    );
    let mut host = crate::platform::runtime::ModuleHost::from_default_manifests().unwrap();
    host.reload_manifests(&resolved.join("cockpit/modules"))
        .unwrap();
    assert_eq!(host.status(&"agent".into()).unwrap().title, "Bundle agent");
}
