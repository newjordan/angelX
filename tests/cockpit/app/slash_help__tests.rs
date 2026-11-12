use super::*;

#[test]
fn embedded_help_matches_pre_migration_bytes() {
    let help = capture(None).unwrap();
    for (topic, bullet, disabled, expected) in [
        (
            None,
            "·",
            false,
            "be32741ee5a664deaf5b7872cbd88589b927c483b434d69afddbe1fb88ac457f",
        ),
        (
            None,
            "*",
            false,
            "df5ee29cf7d726c97895c5bbdbbcc580307d7d3f664a2f6ea334e63f6f35f8d9",
        ),
        (
            Some("all"),
            "·",
            false,
            "551dae6727cc4ea83b89c5e23f90989f5b2479a7acc7be320f45222fe385b0da",
        ),
        (
            Some("all"),
            "·",
            true,
            "551dae6727cc4ea83b89c5e23f90989f5b2479a7acc7be320f45222fe385b0da",
        ),
    ] {
        assert_eq!(
            crate::knowledge::cut::sha256_hex(help.render(topic, bullet, disabled).as_bytes()),
            expected
        );
    }
    let all = help.render(Some("all"), "·", false);
    assert_eq!(
        help.render(Some(" GrApH "), "·", false),
        format!("commands · graph\n{all}")
    );
    assert_eq!(
        help.render(Some("missing"), "*", false),
        format!(
            "commands · unknown topic missing\n{}",
            help.render(None, "*", false)
        )
    );
}

#[test]
fn override_reloads_per_request_and_old_capture_is_unchanged() {
    let temp = crate::tests::TestGitWorkspace::new("slash-help-reload");
    let path = temp.path().join("help.json");
    std::fs::write(&path, EMBEDDED).unwrap();
    let old = capture(Some(&path)).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(EMBEDDED).unwrap();
    value["before_atlas"] = value["before_atlas"]
        .as_str()
        .unwrap()
        .replace("/graph [list", "/graph [edited")
        .into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let next = capture(Some(&path)).unwrap();
    assert!(old.render(Some("all"), "·", false).contains("/graph [list"));
    assert!(
        next.render(Some("all"), "·", false)
            .contains("/graph [edited")
    );
    assert_eq!(next.render(Some("all"), "·", true), next.render(Some("all"), "·", false));
    std::fs::write(&path, b"invalid").unwrap();
    assert!(capture(Some(&path)).is_err());
    assert!(capture(Some(Path::new("relative.json"))).is_err());
    std::fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
    assert!(capture(Some(&path)).is_err());
}

#[cfg(unix)]
#[test]
fn override_does_not_follow_symlinks() {
    let temp = crate::tests::TestGitWorkspace::new("slash-help-symlink");
    let file = temp.path().join("help.json");
    let link = temp.path().join("link.json");
    std::fs::write(&file, EMBEDDED).unwrap();
    std::os::unix::fs::symlink(file, &link).unwrap();
    assert!(capture(Some(&link)).is_err());
}
