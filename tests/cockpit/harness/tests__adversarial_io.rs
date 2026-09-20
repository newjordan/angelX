//! Local filesystem attacks. Assertions distinguish pathname confinement from
//! inode confidentiality: an existing hard link is readable, but edits replace
//! its directory entry and must leave the external inode unchanged.
use super::*;

fn request(name: &str, path: &str) -> Value {
    match name {
        "write_file" => serde_json::json!({"path":path,"content":"replacement\n"}),
        "str_replace" => serde_json::json!({"path":path,"old":"original","new":"replacement"}),
        "multi_edit" => {
            serde_json::json!({"path":path,"edits":[{"old":"original","new":"replacement"}]})
        }
        "apply_patch" => {
            serde_json::json!({"diff":format!("*** Begin Patch\n*** Update File: {path}\n@@\n-original\n+replacement\n*** End Patch")})
        }
        _ => serde_json::json!({"path":path}),
    }
}

const DIRECT: &[&str] = &[
    "read_file",
    "write_file",
    "str_replace",
    "multi_edit",
    "apply_patch",
];

#[test]
fn adversarial_io_profiles_match_mandatory_shell_scope_and_direct_paths() {
    let _lock = crate::tests::env_lock();
    let _experience = EnvGuard::set("ANGEL_EXPERIENCE", "0");
    let base = scratch("adversarial-profiles");
    let root = base.join("workspace");
    std::fs::create_dir(&root).unwrap();
    let protected = base.join("protected.rs");
    std::fs::write(&protected, "original\n").unwrap();
    for profile in ["guarded", "smart", "full"] {
        let _yolo = EnvGuard::set("ANGEL_YOLO", if profile == "full" { "1" } else { "0" });
        let _smart = EnvGuard::set(
            "ANGEL_YOLO_SMART",
            if profile == "smart" { "1" } else { "0" },
        );
        let _sandbox = EnvGuard::set("ANGEL_SANDBOX", "1");
        assert_eq!(crate::yolo::profile().label(), profile);
        assert_eq!(crate::sandbox::enabled(), profile != "full");
        let status = crate::yolo::status_text();
        if profile == "full" {
            assert!(status.contains("sandboxing") && status.contains("bypassed"));
        }
        assert!(
            ReadFileTool { root: root.clone() }
                .call(&serde_json::json!({"path":protected}))
                .is_err()
        );
        let shell = ShellTool::in_dir(root.clone());
        std::fs::write(root.join("source.rs"), "original\n").unwrap();
        for scope in [
            serde_json::json!({"read_only":true}),
            serde_json::json!({"write_paths":[]}),
        ] {
            let mut args = scope;
            args["command"] = "printf forbidden > source.rs".into();
            let error = shell
                .call(&args)
                .expect_err("explicit inspection must deny writes");
            assert!(
                error.contains("effective shell scope: filesystem read-only; network disabled"),
                "{profile}: {error}"
            );
            assert_eq!(
                std::fs::read_to_string(root.join("source.rs")).unwrap(),
                "original\n"
            );
        }
        shell.call(&serde_json::json!({"command":"printf replacement > source.rs", "write_paths":["source.rs"]})).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("source.rs")).unwrap(),
            "replacement"
        );
        assert!(shell.call(&serde_json::json!({"command":"printf forbidden > new.rs", "write_paths":["source.rs"]})).is_err());
        assert!(!root.join("new.rs").exists());
        for path in ["../protected.rs", "source.rs\0suffix"] {
            assert!(
                shell
                    .call(&serde_json::json!({"command":"printf forbidden", "write_paths":[path]}))
                    .is_err()
            );
        }
        assert_eq!(std::fs::read_to_string(&protected).unwrap(), "original\n");
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn adversarial_io_file_registry_inventory() {
    let _lock = crate::tests::env_lock();
    let root = scratch("adversarial-inventory");
    let mut registry = ToolRegistry::new();
    register_file_tools(&mut registry, root.clone());
    let mut actual = registry.bindable_tool_names();
    actual.sort_unstable();
    let mut expected = vec![
        "read_file",
        "write_file",
        "str_replace",
        "multi_edit",
        "apply_patch",
        "resolve_edit",
        "outline",
        "git_diff",
        "git_status",
        "git_log",
        "git_commit",
        "tool_repair",
        "grep",
        "find_files",
        "file_search",
        "defs",
        "list_dir",
    ];
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "new file registry tool requires an adversarial coverage decision"
    );
    println!("file registry inventory: {}", actual.join(", "));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn adversarial_io_direct_tools_escape_and_name_matrix() {
    let _lock = crate::tests::env_lock();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "0");
    let _anchors = EnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let base = scratch("adversarial-paths");
    let root = base.join("workspace");
    std::fs::create_dir(&root).unwrap();
    let protected = base.join("protected");
    std::fs::write(&protected, "original\n").unwrap();
    std::os::unix::fs::symlink(&protected, root.join("escape")).unwrap();
    let mut registry = ToolRegistry::new();
    register_file_tools(&mut registry, root.clone());
    for name in DIRECT {
        for path in [
            "../protected".to_string(),
            protected.to_string_lossy().into_owned(),
            "escape".into(),
            "bad\0name".into(),
            "x".repeat(300),
        ] {
            let outcome = registry.dispatch(name, &request(name, &path));
            assert!(outcome.is_err(), "{name} accepted {path:?}: {outcome:?}");
            assert_eq!(std::fs::read_to_string(&protected).unwrap(), "original\n");
        }
        for path in [
            "é界🦀.txt".to_string(),
            root.join("absolute.txt").to_string_lossy().into_owned(),
            "null-adjacent␀.txt".into(),
        ] {
            let full = root.join(&path);
            std::fs::write(&full, "original\n").unwrap();
            let outcome = registry.dispatch(name, &request(name, &path)).unwrap();
            let expected = if *name == "read_file" {
                "original\n"
            } else {
                "replacement\n"
            };
            assert_eq!(
                std::fs::read_to_string(full).unwrap(),
                expected,
                "{name}: {outcome}"
            );
        }
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
#[cfg(unix)]
fn adversarial_io_hardlinks_preserve_external_inode_on_mutation() {
    let _lock = crate::tests::env_lock();
    let _anchors = EnvGuard::set("ANGEL_HASHLINE_ANCHORS", "0");
    let base = scratch("adversarial-hardlink");
    let root = base.join("workspace");
    std::fs::create_dir(&root).unwrap();
    let protected = base.join("protected");
    std::fs::write(&protected, "original\n").unwrap();
    let mut registry = ToolRegistry::new();
    register_file_tools(&mut registry, root.clone());
    for name in DIRECT {
        let link = root.join(name);
        std::fs::hard_link(&protected, &link).unwrap();
        let output = registry.dispatch(name, &request(name, name)).unwrap();
        assert_eq!(std::fs::read_to_string(&protected).unwrap(), "original\n");
        if *name == "read_file" {
            assert_eq!(
                output, "original\n",
                "hard links are not a confidentiality boundary"
            );
        } else {
            assert_eq!(std::fs::read_to_string(link).unwrap(), "replacement\n");
        }
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
#[cfg(unix)]
fn adversarial_io_directory_swap_after_check_is_denied() {
    let _lock = crate::tests::env_lock();
    let base = scratch("adversarial-swap");
    let root = base.join("workspace");
    let outside = base.join("outside");
    std::fs::create_dir_all(root.join("parent")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("file"), "original\n").unwrap();
    let checked = safe_path(&root, "parent/file").unwrap();
    let swap_root = root.clone();
    let swap_outside = outside.clone();
    std::thread::spawn(move || {
        std::fs::rename(swap_root.join("parent"), swap_root.join("saved")).unwrap();
        std::os::unix::fs::symlink(swap_outside, swap_root.join("parent")).unwrap();
    })
    .join()
    .unwrap();
    // Reuse the previously accepted pathname: the actual descriptor operation
    // must resolve beneath the root again, never trust the earlier check.
    assert!(confined_fs::confined_read(&root, &checked).is_err());
    assert!(confined_fs::confined_write(&root, &checked, b"replacement\n").is_err());
    let mut registry = ToolRegistry::new();
    register_file_tools(&mut registry, root.clone());
    for name in DIRECT {
        assert!(
            registry
                .dispatch(name, &request(name, "parent/file"))
                .is_err()
        );
    }
    assert_eq!(
        std::fs::read_to_string(outside.join("file")).unwrap(),
        "original\n"
    );
    std::fs::remove_dir_all(base).unwrap();
}
