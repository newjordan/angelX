#[test]
fn explicit_helper_pin_does_not_require_procfs_but_self_discovery_does() {
    let absent = || {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "procfs absent",
        ))
    };
    assert_eq!(
        resolve_helper_path(absent, Some(" /opt/angel ")).unwrap(),
        PathBuf::from("/opt/angel")
    );
    assert!(
        resolve_helper_path(absent, Some("self"))
            .unwrap_err()
            .contains("procfs absent")
    );
    assert!(
        resolve_helper_path(absent, None)
            .unwrap_err()
            .contains("procfs absent")
    );
    assert_eq!(
        resolve_helper_path(|| Ok(PathBuf::from("/owned/angel")), Some("self")).unwrap(),
        PathBuf::from("/owned/angel")
    );
}
use super::*;

#[test]
fn deleted_exe_suffix_is_stripped_from_helper_path() {
    assert_eq!(
        sanitize_deleted_exe_path(PathBuf::from("/opt/angel (deleted)")),
        PathBuf::from("/opt/angel"),
        "a live-rebuilt binary must resolve to its on-disk replacement"
    );
    assert_eq!(
        sanitize_deleted_exe_path(PathBuf::from("/opt/angel")),
        PathBuf::from("/opt/angel"),
        "a clean path passes through untouched"
    );
}

#[test]
fn choose_helper_honours_pin_then_sibling_then_self() {
    assert_eq!(
        choose_helper(Path::new("/opt/angel"), true, Some("/pinned/helper")),
        PathBuf::from("/pinned/helper"),
        "an explicit path pin wins even when a sibling is deployed"
    );
    assert_eq!(
        choose_helper(
            Path::new("/opt/angel"),
            false,
            Some("/opt/other/angel (deleted)")
        ),
        PathBuf::from("/opt/other/angel"),
        "a pinned path gets the same (deleted)-suffix sanitation"
    );
    assert_eq!(
        choose_helper(Path::new("/opt/angel"), true, None),
        PathBuf::from("/opt/angel-sandbox"),
        "an unpinned spawn prefers the deployed sibling helper"
    );
    assert_eq!(
        choose_helper(Path::new("/opt/angel"), false, None),
        PathBuf::from("/opt/angel"),
        "without sibling or pin the cockpit re-execs itself"
    );
    assert_eq!(
        choose_helper(Path::new("/opt/angel"), false, Some("self")),
        PathBuf::from("/opt/angel"),
        "the self pin restores the cockpit re-exec explicitly"
    );
    assert_eq!(
        choose_helper(Path::new("/opt/angel"), true, Some("  ")),
        PathBuf::from("/opt/angel-sandbox"),
        "a blank pin is treated as unset"
    );
}

#[test]
fn cargo_install_grants_only_bounded_metadata_paths() {
    let home = Path::new("/operator-home");
    let paths = cargo_install_metadata_paths(home);
    assert!(paths.contains(&home.join(".cargo/.crates.toml")));
    assert!(paths.contains(&home.join(".cargo/.crates2.json")));
    assert!(!paths.contains(&home.join(".cargo")));
    assert!(!paths.contains(&home.join(".cargo/config.toml")));
    assert!(!paths.contains(&home.join(".cargo/credentials.toml")));
}
