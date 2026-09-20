use super::*;
#[test]
fn startup_walk_deadline_is_partial_and_preserves_full_digest() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("startup-deadline-{}", super::super::now_ms()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a"), b"first").unwrap();
    let partial = hash_tree_with_deadline(&root, b"a\0", Some(Instant::now())).unwrap();
    assert!(!partial.complete);
    assert_eq!((partial.paths, partial.total), (0, 1));
    let full = hash_tree_with_deadline(&root, b"a\0", None).unwrap();
    assert!(full.complete);
    assert_eq!(full.paths, 1);
    assert_ne!(partial.sha256, full.sha256);
    let bytes = vec![0x5a; 150_000];
    std::fs::write(root.join("large"), &bytes).unwrap();
    assert_eq!(
        file_digest(&root.join("large"), None).unwrap().unwrap(),
        crate::knowledge::cut::sha256_hex(&bytes)
    );
    assert_eq!(
        file_digest(&root.join("large"), Some(Instant::now())).unwrap(),
        None
    );
    assert_eq!(
        fast_sha256(b"abc"),
        crate::knowledge::cut::sha256_hex(b"abc")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn startup_walk_honors_gitignore_and_info_exclude_and_labels_exhaustion() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("startup-ignore-{}", super::super::now_ms()));
    std::fs::create_dir_all(&root).unwrap();
    let git = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    };
    git(&["init", "-q"]);
    std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    std::fs::write(root.join("visible"), "tracked").unwrap();
    git(&["add", "."]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-qm",
        "fixture",
    ]);
    std::fs::write(root.join(".git/info/exclude"), "excluded/\n").unwrap();
    let before = workspace_state_inner(Some(&root), Some(Instant::now() + Duration::from_secs(10)));
    assert_eq!(before["startup_index"]["complete"], true);
    for directory in ["ignored", "excluded"] {
        std::fs::create_dir(root.join(directory)).unwrap();
        for n in 0..100 {
            std::fs::write(root.join(directory).join(n.to_string()), b"litter").unwrap();
        }
    }
    let after = workspace_state_inner(Some(&root), Some(Instant::now() + Duration::from_secs(10)));
    assert_eq!(before["tree_sha256"], after["tree_sha256"]);
    assert_eq!(before["startup_index"], after["startup_index"]);
    std::fs::write(root.join("visible"), "changed").unwrap();
    assert_ne!(
        workspace_state(Some(&root))["tree_sha256"],
        before["tree_sha256"]
    );
    let previous = std::env::var_os("ANGEL_STARTUP_WALK_MS");
    unsafe {
        std::env::set_var("ANGEL_STARTUP_WALK_MS", "0");
    }
    let scope = WorkspaceStartScope::enter(&root);
    let mut attached = json!({});
    attach_start(&mut attached);
    if let Some(value) = previous {
        unsafe {
            std::env::set_var("ANGEL_STARTUP_WALK_MS", value);
        }
    } else {
        unsafe {
            std::env::remove_var("ANGEL_STARTUP_WALK_MS");
        }
    }
    assert_eq!(attached["start_startup_index"]["complete"], false);
    assert_ne!(attached["start_tree_sha256"], before["tree_sha256"]);
    drop(scope);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn trace_schema_tree_digest_tracks_content_deletion_and_order() {
    let _guard = crate::tests::env_lock();
    let root =
        std::env::temp_dir().join(format!("trace-schema-content-{}", super::super::now_ms()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a"), b"first").unwrap();
    std::fs::write(root.join("b"), b"second").unwrap();
    let before = hash_tree(&root, b"a\0b\0").unwrap();
    assert_eq!(hash_tree(&root, b"b\0a\0a\0").unwrap(), before);
    std::fs::write(root.join("a"), b"changed").unwrap();
    let after = hash_tree(&root, b"a\0b\0").unwrap();
    assert_ne!(after, before);
    std::fs::remove_file(root.join("a")).unwrap();
    assert_ne!(hash_tree(&root, b"a\0b\0").unwrap(), after);
}

#[test]
fn trace_schema_start_survives_edits_commits_nested_turns_and_scope_exit() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("trace-start-{}", super::super::now_ms()));
    std::fs::create_dir_all(&root).unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q"]);
    std::fs::write(root.join("a"), "before").unwrap();
    git(&["add", "a"]);
    git(&["commit", "-qm", "fixture start"]);
    let before = workspace_state(Some(&root));
    let scope = WorkspaceStartScope::enter(&root);
    std::fs::write(root.join("a"), "after").unwrap();
    let mut after = workspace_state(Some(&root));
    attach_start(&mut after);
    assert_eq!(after["start_tree_sha256"], before["tree_sha256"]);
    assert_eq!(
        after["start_dirty_paths_sha256"],
        before["dirty_paths_sha256"]
    );
    assert_ne!(after["tree_sha256"], after["start_tree_sha256"]);
    assert_ne!(
        after["dirty_paths_sha256"],
        after["start_dirty_paths_sha256"]
    );
    {
        let _nested = WorkspaceStartScope::enter(&root);
        attach_start(&mut after);
        assert_eq!(after["start_tree_sha256"], after["tree_sha256"]);
    }
    git(&["add", "a"]);
    git(&["commit", "-qm", "fixture end"]);
    after = workspace_state(Some(&root));
    assert_ne!(after["head_tree"], before["head_tree"]);
    attach_start(&mut after);
    assert_eq!(after["head_tree"], before["head_tree"]);
    assert_eq!(after["start_tree_sha256"], before["tree_sha256"]);
    assert_ne!(after["head"], before["head"]);
    drop(scope);
    after = workspace_state(Some(&root));
    attach_start(&mut after);
    assert_eq!(after["start_tree_sha256"], "unbound");
    assert_eq!(after["head_tree"], "unbound");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn trace_schema_unknown_fields_are_explicit() {
    let mut record = json!({"ts_ms":1,"hops":2});
    defaults(&mut record);
    assert_eq!(record["turn"]["hop_count"], 2);
    assert!(record["cost"]["paid"].is_null());
    assert_eq!(record["verifier"], json!([]));
    assert_eq!(workspace_state(None)["head"], "unbound");
}
#[test]
fn trace_schema_workspace_non_git_and_unborn() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("trace-schema-nongit-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(
        workspace_state(Some(&root.join("absent")))["head"],
        "not a git repo"
    );
    let startup = startup_workspace_state(&root);
    assert_eq!(startup["head"], "not a git repo");
    assert_eq!(startup["startup_index"]["reason"], "not a git repo");
    assert!(WorkspaceStartScope::enter(&root).partial_notice().is_none());
    let git = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    };
    git(&["init", "-q"]);
    // Empty repository has no HEAD; use the existing object's commit only in
    // real runs. No test creates a commit (worker policy).
    assert_eq!(workspace_state(Some(&root))["head"], "unbound");
    assert_eq!(startup_workspace_state(&root)["head"], "unbound");
    assert!(WorkspaceStartScope::enter(&root).partial_notice().is_none());
}
