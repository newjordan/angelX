fn scope_for(call: ToolCall, workspace: &Path) -> crate::approval::ApprovalScope {
    crate::approval::ApprovalScope::ActionBatch(
        ActionBatch::from_calls(&[call], ActionCapsuleMode::Approve, workspace)
            .unwrap()
            .approval_key()
            .to_owned(),
    )
}

#[test]
fn adversarial_approval_cargo_suffix_and_manifest_reuse_denied() {
    let _lock = crate::tests::env_lock();
    let workspace = Path::new("/fixture/workspace");
    let baseline = scope_for(
        call("shell", serde_json::json!({"command":"cargo test"})),
        workspace,
    );
    assert_eq!(
        crate::approval::test_reuse(baseline.clone(), baseline.clone()),
        crate::approval::Decision::Approve
    );
    for command in [
        "cargo test; rm -rf ../victim",
        "cargo test --manifest-path ../elsewhere/Cargo.toml",
    ] {
        let next = scope_for(
            call("shell", serde_json::json!({"command":command})),
            workspace,
        );
        assert_eq!(
            crate::approval::test_reuse(baseline.clone(), next),
            crate::approval::Decision::Deny
        );
    }
}

#[test]
fn adversarial_approval_tool_name_and_payload_are_not_transferable() {
    let _lock = crate::tests::env_lock();
    let workspace = Path::new("/fixture/workspace");
    let shell = scope_for(
        call("shell", serde_json::json!({"command":"cargo test"})),
        workspace,
    );
    // Typed verifiers do not consume capsule grants at all. Their own
    // pinned-argv policy authorizes execution independently of shell results.
    assert!(
        ActionBatch::from_calls(
            &[call("run_tests", serde_json::json!({}))],
            ActionCapsuleMode::Approve,
            workspace
        )
        .is_none()
    );
    let patch = scope_for(
        call(
            "apply_patch",
            serde_json::json!({"diff":"*** Begin Patch\n*** Add File: src/a\n+x\n*** End Patch"}),
        ),
        workspace,
    );
    assert_eq!(
        crate::approval::test_reuse(shell.clone(), patch),
        crate::approval::Decision::Deny
    );
    let renamed = scope_for(
        call("cargo", serde_json::json!({"command":"cargo test"})),
        workspace,
    );
    assert_eq!(
        crate::approval::test_reuse(shell, renamed),
        crate::approval::Decision::Deny
    );
}

#[cfg(unix)]
#[test]
fn adversarial_approval_write_reuse_cannot_escape_via_parent_or_symlink() {
    use crate::harness::Tool;
    let _lock = crate::tests::env_lock();
    let _full = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
    let fixture = std::env::temp_dir().join(format!(
        "angel-s03-reuse-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let root = fixture.join("workspace");
    let outside = fixture.join("outside");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let tool = crate::tools::file::WriteFileTool { root: root.clone() };
    let args = serde_json::json!({"path":"src/a", "content":"allowed"});
    let baseline = scope_for(call("write_file", args.clone()), &root);
    assert!(tool.call(&args).is_ok());
    let escape = serde_json::json!({"path":"../outside/a", "content":"allowed"});
    assert_eq!(
        crate::approval::test_reuse(
            baseline.clone(),
            scope_for(call("write_file", escape.clone()), &root)
        ),
        crate::approval::Decision::Deny
    );
    assert!(tool.call(&escape).is_err());
    // Same lexical arguments and cached approval after an ancestor changes:
    // the actual descriptor-based write must still reject the escape.
    std::fs::rename(root.join("src"), root.join("original-src")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("src")).unwrap();
    assert_eq!(
        crate::approval::test_reuse(
            baseline.clone(),
            scope_for(call("write_file", args.clone()), &root)
        ),
        crate::approval::Decision::Approve
    );
    assert!(tool.call(&args).is_err());
    assert!(!outside.join("a").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("original-src/a")).unwrap(),
        "allowed"
    );
    std::fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn adversarial_approval_command_workspace_target_and_effect_changes_invalidate_batch() {
    let _lock = crate::tests::env_lock();
    let original = call(
        "shell",
        serde_json::json!({"command":"printf local", "write_paths":["a.rs"]}),
    );
    let key = |calls: &[ToolCall], workspace: &Path| {
        ActionBatch::from_calls(calls, ActionCapsuleMode::Approve, workspace)
            .unwrap()
            .approval_key()
            .to_owned()
    };
    let workspace = Path::new("/fixture/one");
    let baseline = key(std::slice::from_ref(&original), workspace);
    assert_eq!(baseline, key(std::slice::from_ref(&original), workspace));
    assert_ne!(
        baseline,
        key(std::slice::from_ref(&original), Path::new("/fixture/two"))
    );
    for args in [
        serde_json::json!({"command":"printf changed", "write_paths":["a.rs"]}),
        serde_json::json!({"command":"printf local", "write_paths":["b.rs"]}),
        serde_json::json!({"command":"printf local", "write_paths":[]}),
        serde_json::json!({"command":"printf local"}),
    ] {
        assert_ne!(baseline, key(&[call("shell", args)], workspace));
    }
}

#[test]
fn adversarial_approval_profile_change_invalidates_batch() {
    let _lock = crate::tests::env_lock();
    let calls = [call("shell", serde_json::json!({"command":"printf local"}))];
    let _guarded = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "0");
    let _smart_off = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "0");
    let guarded = ActionBatch::from_calls(
        &calls,
        ActionCapsuleMode::Approve,
        Path::new("/fixture/one"),
    )
    .unwrap();
    let _full = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let full = ActionBatch::from_calls(
        &calls,
        ActionCapsuleMode::Approve,
        Path::new("/fixture/one"),
    )
    .unwrap();
    assert_ne!(guarded.approval_key(), full.approval_key());
}

use super::*;

fn call(name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: "call_1".to_string(),
        name: name.to_string(),
        args,
    }
}

#[test]
fn mode_defaults_to_approval_and_accepts_explicit_off_or_observe() {
    assert_eq!(ActionCapsuleMode::parse(None), ActionCapsuleMode::Approve);
    assert_eq!(ActionCapsuleMode::parse(Some("0")), ActionCapsuleMode::Off);
    assert_eq!(
        ActionCapsuleMode::parse(Some("observe")),
        ActionCapsuleMode::Observe
    );
    assert_eq!(
        ActionCapsuleMode::parse(Some("on")),
        ActionCapsuleMode::Approve
    );
    assert_eq!(
        ActionCapsuleMode::parse(Some("approve")),
        ActionCapsuleMode::Approve
    );
    assert_eq!(
        ActionCapsuleMode::parse(Some("unexpected")),
        ActionCapsuleMode::Approve
    );
}

#[test]
fn yolo_disables_interactive_action_capsules() {
    let _guard = crate::tests::env_lock();
    let previous = std::env::var_os("ANGEL_YOLO");
    let previous_smart = std::env::var_os("ANGEL_YOLO_SMART");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_YOLO", "1") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_YOLO_SMART") };
    let mut registry = ToolRegistry::new();
    registry.enable_action_capsules();
    assert_eq!(mode_for(&registry), ActionCapsuleMode::Off);
    match previous {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var("ANGEL_YOLO", value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_YOLO") },
    }
    match previous_smart {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var("ANGEL_YOLO_SMART", value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_YOLO_SMART") },
    }
}

#[test]
fn smart_yolo_also_disables_interactive_action_capsules() {
    let _guard = crate::tests::env_lock();
    let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "1");
    let mut registry = ToolRegistry::new();
    registry.enable_action_capsules();
    assert_eq!(mode_for(&registry), ActionCapsuleMode::Off);
    assert!(!crate::yolo::enabled());
    assert!(crate::yolo::smart_enabled());
}

#[test]
fn previews_are_argument_only_and_reads_stay_unclassified() {
    let write = ActionPreview::from_call(&call(
        "write_file",
        serde_json::json!({"path":"src/lib.rs","content":"abc"}),
    ))
    .unwrap();
    assert_eq!(write.scope, "write src/lib.rs (3 B payload)");
    assert!(
        ActionPreview::from_call(&call("read_file", serde_json::json!({"path":"src/lib.rs"}),))
            .is_none()
    );

    let shell = ActionPreview::from_call(&call(
        "shell",
        serde_json::json!({"command":"API_KEY=secret cargo test"}),
    ))
    .unwrap();
    assert!(!shell.scope.contains("secret"));
    assert!(shell.scope.contains("API_KEY=…"));

    let legacy_shell =
        ActionPreview::from_call(&call("shell", serde_json::json!({"cmd":"cargo check"}))).unwrap();
    assert!(legacy_shell.scope.contains("cargo check"));

    let machine_test = ActionPreview::from_call(&call(
        "machine_test",
        serde_json::json!({"command":"swift test --filter SchedulerTests"}),
    ))
    .unwrap();
    assert!(machine_test.scope.contains("queue remote machine test"));
    assert!(machine_test.scope.contains("SchedulerTests"));

    assert!(
        ActionPreview::from_call(&call(
            "code_mode",
            serde_json::json!({"script":"return grep({pattern:'x'});"}),
        ))
        .is_none(),
        "bounded read-only reconnaissance must not request action approval"
    );
    let effectful_code = ActionPreview::from_call(&call(
        "code_mode",
        serde_json::json!({"script":"return shell({command:'true'});","allow_effects":true}),
    ))
    .unwrap();
    assert!(effectful_code.scope.contains("effectful programmatic"));

    assert!(
        ActionPreview::from_call(&call(
            "http_request",
            serde_json::json!({"method":"GET","url":"https://example.com"}),
        ))
        .is_none(),
        "read-only HTTP must not request mutation approval"
    );
    let mutating_http = ActionPreview::from_call(&call(
        "http_request",
        serde_json::json!({"method":"POST","url":"https://api.example.com/jobs"}),
    ))
    .unwrap();
    assert!(mutating_http.scope.contains("api.example.com/jobs"));
}

#[test]
fn batch_is_scoped_to_exact_effectful_calls() {
    let _lock = crate::tests::env_lock();
    let calls = vec![
        call("read_file", serde_json::json!({"path":"a.rs"})),
        call(
            "str_replace",
            serde_json::json!({"path":"a.rs","old":"a","new":"b"}),
        ),
        call("shell", serde_json::json!({"command":"cargo test"})),
    ];
    let batch = ActionBatch::from_calls(
        &calls,
        ActionCapsuleMode::Approve,
        Path::new("/fixture/one"),
    )
    .unwrap();
    assert_eq!(batch.count(), 2);
    assert!(batch.contains(0).is_none());
    assert!(batch.contains(1).is_some());
    assert!(batch.contains(2).is_some());
    assert!(batch.approval_prompt().contains("replace once in a.rs"));
    assert!(batch.approval_prompt().contains("run shell command"));

    let changed = vec![
        calls[0].clone(),
        call(
            "str_replace",
            serde_json::json!({"path":"a.rs","old":"a","new":"c"}),
        ),
        calls[2].clone(),
    ];
    let changed_batch = ActionBatch::from_calls(
        &changed,
        ActionCapsuleMode::Approve,
        Path::new("/fixture/one"),
    )
    .unwrap();
    assert_ne!(batch.approval_key(), changed_batch.approval_key());
}

#[test]
fn receipt_never_claims_command_success() {
    let shell =
        ActionPreview::from_call(&call("shell", serde_json::json!({"command":"false"}))).unwrap();
    assert!(shell.receipt("exit 1", 7).contains("ran"));
    assert!(
        shell
            .receipt("tool error: spawn failed", 7)
            .contains("dispatch error")
    );
}

#[test]
fn metrics_aggregate_without_per_action_storage() {
    let mut metrics = ActionCapsuleMetrics::default();
    metrics.note_preflight(Duration::from_micros(12), 3);
    metrics.note_execution(Duration::from_millis(8));
    metrics.note_denied(2);
    metrics.note_approval_wait(Duration::from_millis(25));
    assert_eq!(metrics.previews, 1);
    assert_eq!(metrics.operations, 3);
    assert_eq!(metrics.preflight_us, 12);
    assert_eq!(metrics.action_exec_ms, 8);
    assert_eq!(metrics.denied, 2);
    assert_eq!(metrics.approval_wait_ms, 25);
}

#[test]
fn observe_keeps_disjoint_write_parallelism_but_approve_serializes_modals() {
    let boundary = WorkspaceBoundary::new(Path::new("."));
    let calls = vec![
        call(
            "write_file",
            serde_json::json!({"path":"a.rs","content":"a"}),
        ),
        call(
            "write_file",
            serde_json::json!({"path":"b.rs","content":"b"}),
        ),
    ];
    assert!(parallel_allowed(ActionCapsuleMode::Off, &boundary, &calls));
    assert!(parallel_allowed(
        ActionCapsuleMode::Observe,
        &boundary,
        &calls
    ));
    assert!(!parallel_allowed(
        ActionCapsuleMode::Approve,
        &boundary,
        &calls
    ));
}
