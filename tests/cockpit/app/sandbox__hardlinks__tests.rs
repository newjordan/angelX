use super::*;

#[test]
fn landlock_only_hardlink_write_keeps_outside_unchanged_and_local_links_work() {
    let _guard = crate::tests::env_lock();
    if let Some(root) = std::env::var_os("ANGEL_T_ALIAS_CHILD") {
        let root = PathBuf::from(root);
        let policy = super::super::SandboxPolicy {
            writable_roots: vec![root.clone()],
            allow_network: false,
            enforce: true,
            mandatory: true,
            sealed_reads: vec![],
            deny_reads: vec![],
        };
        unsafe {
            std::env::set_var(
                super::super::HELPER_POLICY_ENV,
                serde_json::to_string(&policy).unwrap(),
            );
        }
        let args = [
            std::ffi::OsString::from("--"),
            std::ffi::OsString::from("/bin/sh"),
            std::ffi::OsString::from("-c"),
            std::ffi::OsString::from(
                "printf changed > \"$1/alias\" && ln \"$1/alias\" \"$1/new-local-link\" && test \"$(cat \"$1/new-local-link\")\" = changed && ! (printf escape > \"$1/../outside\")",
            ),
            std::ffi::OsString::from("alias-test"),
            root.into_os_string(),
        ];
        super::super::exec_helper_inner(args, || {
            super::super::compatibility::classify(
                true,
                true,
                true,
                false,
                "bwrap: setting up uid map: Permission denied",
            )
        })
        .unwrap();
        return;
    }
    let fixture = TestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "original").unwrap();
    std::fs::hard_link(&outside, root.join("alias")).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child.args(["--exact", "sandbox::hardlinks::tests::landlock_only_hardlink_write_keeps_outside_unchanged_and_local_links_work", "--nocapture"]);
    child.env("ANGEL_T_ALIAS_CHILD", &root);
    let channel = super::super::status::attach(&mut child).unwrap();
    let output = child.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = channel.receive().expect("helper status receipt");
    assert_eq!(receipt["sandbox_profile"], "landlock-only");
    assert!(
        receipt["notice"]
            .as_str()
            .unwrap()
            .contains("userns denied by AppArmor")
    );
    assert_eq!(
        receipt["aliases_copied"],
        serde_json::json!([root.join("alias")])
    );
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "original");
}

#[test]
fn landlock_alias_copy_preserves_outside_and_new_local_links() {
    let _guard = crate::tests::env_lock();
    let fixture = TestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let outside = fixture.path().join("outside");
    let alias = root.join("alias");
    std::fs::write(&outside, "original").unwrap();
    std::fs::hard_link(&outside, &alias).unwrap();
    let metadata = std::fs::metadata(&outside).unwrap();
    let aliases = scan(std::slice::from_ref(&root)).unwrap();
    let limited = privatize_with_limits(&aliases, 0, 0);
    assert_eq!(limited["aliases_unprotected"], serde_json::json!([alias]));
    let receipt = privatize(&aliases);
    assert_eq!(receipt["aliases_copied"], serde_json::json!([alias]));
    assert_eq!(std::fs::metadata(&alias).unwrap().mode(), metadata.mode());
    assert_eq!(
        std::fs::metadata(&alias).unwrap().modified().unwrap(),
        metadata.modified().unwrap()
    );
    std::fs::write(&alias, "changed").unwrap();
    std::fs::hard_link(&alias, root.join("local-link")).unwrap();
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "original");
    assert_eq!(
        std::fs::read_to_string(root.join("local-link")).unwrap(),
        "changed"
    );
    assert!(
        privatize(&scan(std::slice::from_ref(&root)).unwrap())["aliases_copied"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn complete_internal_alias_groups_remain_writable_but_external_aliases_do_not() {
    let fixture = TestRoot::new();
    let first = fixture.path().join("first");
    let second = fixture.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let local = first.join("local");
    let sibling = second.join("local-alias");
    std::fs::write(&local, "local").unwrap();
    std::fs::hard_link(&local, &sibling).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "outside").unwrap();
    let external = first.join("external");
    std::fs::hard_link(&outside, &external).unwrap();
    let partial_grant = scan(std::slice::from_ref(&first)).unwrap();
    assert_eq!(partial_grant.paths, vec![external.clone(), local.clone()]);
    let whole_grant = scan(&[first.clone(), second]).unwrap();
    assert_eq!(whole_grant.paths, vec![external]);
    // A bounded/incomplete walk cannot infer that unseen aliases are local.
    let incomplete = scan_with_limit(&[local], 1).unwrap();
    assert_eq!(incomplete.paths.len(), 1);
}

#[test]
fn hardlink_scan_reports_aliases_and_limits_without_following_symlinks() {
    let fixture = TestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "original").unwrap();
    std::fs::hard_link(&outside, root.join("alias")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("symlink")).unwrap();
    std::fs::write(root.join("ordinary"), "local").unwrap();
    let scan = scan(std::slice::from_ref(&root)).unwrap();
    assert_eq!(scan.paths, vec![root.join("alias")]);
    assert!(scan.limited.is_empty());
    let receipt = scan.receipt();
    assert_eq!(receipt["hardlink_readonly_count"], 1);
    assert!(!receipt.to_string().contains("original"));
    assert!(!receipt.to_string().contains(&root.display().to_string()));
    let limited = scan_with_limit(&[root], 1).unwrap();
    assert!(!limited.limited.is_empty());
}
