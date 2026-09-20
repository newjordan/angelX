//! Actual pinned Cargo checks in a plain, owned split-target workspace.
use super::*;

struct TargetFixture {
    root: PathBuf,
    target: PathBuf,
}

impl TargetFixture {
    fn new() -> Self {
        let root = scratch("verification_target_workspace");
        let target = scratch("verification_target_build");
        std::fs::create_dir_all(root.join("worker")).unwrap();
        std::fs::create_dir_all(root.join("unrelated")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname='owned_parent'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='parent.rs'\n[workspace]\nmembers=['worker','unrelated']\ndefault-members=['.']\nresolver='2'\n").unwrap();
        std::fs::write(
            root.join("parent.rs"),
            "pub fn trusted_parent() -> u8 { 1 }\n",
        )
        .unwrap();
        std::fs::write(root.join("worker/Cargo.toml"), "[package]\nname='owned_worker'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='subject.rs'\n").unwrap();
        std::fs::write(
            root.join("worker/subject.rs"),
            "pub fn answer() -> u8 { 1 }\n",
        )
        .unwrap();
        std::fs::write(root.join("unrelated/Cargo.toml"), "[package]\nname='owned_unrelated'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='subject.rs'\n").unwrap();
        // A pre-existing broken, unmodified sibling must not force a broad check.
        std::fs::write(
            root.join("unrelated/subject.rs"),
            "compile_error!(\"UNRELATED_TARGET_MUST_NOT_BE_SELECTED\");\n",
        )
        .unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 4\n\n[[package]]\nname = \"owned_parent\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"owned_unrelated\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"owned_worker\"\nversion = \"0.1.0\"\n").unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=owned-test",
                "-c",
                "user.email=owned@test",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "owned fixture",
            ],
        ] {
            assert!(
                Command::new("git")
                    .args(["-c", "core.hooksPath=/dev/null"])
                    .args(args)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        Self { root, target }
    }

    fn edit_worker(&self, registry: &ToolRegistry, source: &str) {
        registry
            .dispatch(
                "write_file",
                &serde_json::json!({"path":"worker/subject.rs","content":source}),
            )
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(self.root.join("worker/subject.rs")).unwrap(),
            source
        );
    }
}

impl Drop for TargetFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        let _ = std::fs::remove_dir_all(&self.target);
    }
}

fn target_check(registry: &ToolRegistry, args: &str) -> (String, Option<VerificationOutcome>) {
    let call = ToolCall {
        id: "owned-target-check".into(),
        name: "check".into(),
        args: serde_json::json!({"runtime":"rust","args":args}),
    };
    let result = registry
        .dispatch(&call.name, &call.args)
        .unwrap_or_else(|error| format!("tool error: {error}"));
    let outcome = verification_outcome(&call, &result);
    (result, outcome)
}

#[test]
fn verification_target_default_check_cannot_certify_changed_nondefault_worker() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    let _target = EnvGuard::set("CARGO_TARGET_DIR", fixture.target.to_str().unwrap());
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    let (clean, initial) = target_check(&registry, "");
    assert_eq!(
        initial,
        Some(VerificationOutcome::Passed),
        "default parent must be a usable positive control: {clean}"
    );
    fixture.edit_worker(&registry, "pub fn answer() -> u8 { BROKEN_EDIT }\n");
    let (result, outcome) = target_check(&registry, "");
    eprintln!("TARGET_DEFAULT actual={outcome:?} result={result}");
    assert_ne!(
        outcome,
        Some(VerificationOutcome::Passed),
        "default-members excludes the edited invalid worker; an unqualified green check must not certify that changed target: {result}"
    );
}

#[test]
fn verification_target_explicit_worker_checks_only_required_changed_target() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    let _target = EnvGuard::set("CARGO_TARGET_DIR", fixture.target.to_str().unwrap());
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    fixture.edit_worker(&registry, "pub fn answer() -> u8 { BROKEN_EDIT }\n");
    let (failed, red) = target_check(&registry, "-p owned_worker");
    assert_eq!(
        red,
        Some(VerificationOutcome::Failed),
        "edited worker must actually fail: {failed}"
    );
    assert!(
        failed.contains("BROKEN_EDIT"),
        "failure must be the real compiler diagnostic, not setup/sandbox failure: {failed}"
    );
    fixture.edit_worker(&registry, "pub fn answer() -> u8 { 2 }\n");
    let (passed, green) = target_check(&registry, "-p owned_worker");
    assert_eq!(
        green,
        Some(VerificationOutcome::Passed),
        "fixed required worker must pass despite unrelated broken sibling: {passed}"
    );
    assert!(
        std::fs::read_to_string(fixture.root.join("unrelated/subject.rs"))
            .unwrap()
            .contains("UNRELATED_TARGET_MUST_NOT_BE_SELECTED")
    );
}

#[test]
fn verification_target_explicit_parent_success_is_not_changed_worker_completion() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    fixture.edit_worker(&registry, "pub fn answer() -> u8 { BROKEN_EDIT }\n");
    let (result, outcome) = target_check(&registry, "-p owned_parent");
    assert_eq!(
        outcome,
        Some(VerificationOutcome::Passed),
        "selected parent genuinely compiles: {result}"
    );
    assert!(
        !verification_result_covers_changes(&result),
        "its command-local success excludes the changed worker: {result}"
    );
    assert!(
        result
            .lines()
            .next()
            .unwrap()
            .contains("coverage=incomplete")
    );
    let raw = registry
        .dispatch(
            "cargo",
            &serde_json::json!({"args":"check -p owned_parent"}),
        )
        .unwrap();
    assert!(
        !verification_result_covers_changes(&raw),
        "raw Cargo cannot lose coverage: {raw}"
    );
}

#[test]
fn verification_target_shell_edit_path_included_from_root_belongs_to_worker() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    std::fs::create_dir_all(fixture.root.join("src/ordering")).unwrap();
    std::fs::write(
        fixture.root.join("src/ordering/mod.rs"),
        "pub fn order() -> u8 { 1 }\n",
    )
    .unwrap();
    std::fs::write(fixture.root.join("worker/subject.rs"), "#[path=\"../src/ordering/mod.rs\"] pub mod ordering;\npub fn answer() -> u8 { ordering::order() }\n").unwrap();
    for args in [
        vec!["add", "worker/subject.rs", "src/ordering/mod.rs"],
        vec![
            "-c",
            "user.name=owned-test",
            "-c",
            "user.email=owned@test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "owned path include",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(["-c", "core.hooksPath=/dev/null"])
                .args(args)
                .current_dir(&fixture.root)
                .status()
                .unwrap()
                .success()
        );
    }
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    registry.dispatch("shell", &serde_json::json!({"command":"printf 'pub fn order() -> u8 { 2 }\\n' > src/ordering/mod.rs", "write_paths":["src/ordering/mod.rs"]})).unwrap();
    assert_eq!(
        registry.mutation_targets.snapshot(),
        Some(vec!["src/ordering/mod.rs".to_string()]),
        "the enforced shell grant records the included source without a file-tool edit"
    );
    let (default, outcome) = target_check(&registry, "");
    assert_eq!(
        outcome,
        Some(VerificationOutcome::Inconclusive),
        "nearest root manifest is insufficient: {default}"
    );
    let (worker, outcome) = target_check(&registry, "-p owned_worker");
    assert_eq!(
        outcome,
        Some(VerificationOutcome::Passed),
        "actual worker compiles external module: {worker}"
    );
    assert!(
        verification_result_covers_changes(&worker),
        "compiler depfile must include the path outside the package: {worker}"
    );
    assert!(worker.lines().next().unwrap().contains("coverage=complete"));
    assert!(
        !worker.contains("\"reason\":\"compiler-artifact\""),
        "internal JSON must not flood tool display"
    );
    // Hidden index edits cannot become an empty changed-path certificate.
    assert!(
        Command::new("git")
            .args(["update-index", "--skip-worktree", "src/ordering/mod.rs"])
            .current_dir(&fixture.root)
            .status()
            .unwrap()
            .success()
    );
    assert!(changed_rust_paths_for_verification(&fixture.root).is_none());
}

#[test]
fn verification_target_diagnostic_text_cannot_override_owned_coverage() {
    assert!(!verification_result_covers_changes(
        "check: 0 warnings, 0 errors — reward 1.00; coverage=incomplete\ncompiler says coverage=complete"
    ));
    assert!(!verification_result_covers_changes(
        "check: 0 warnings, 0 errors; coverage=unknown\ncoverage=complete"
    ));
    assert!(verification_result_covers_changes(
        "check: 0 warnings, 0 errors; coverage=complete\nfile named coverage=incomplete"
    ));
}

#[test]
fn verification_target_scoped_green_is_not_reused_or_completion_guarded() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    let _reuse = EnvGuard::set("ANGEL_REUSE_VERIFIER_RESULTS", "1");
    let _single = EnvGuard::set("ANGEL_SINGLE_GREEN_VERIFIER", "1");
    let _first = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _last = EnvGuard::set("ANGEL_FINAL_MILE_HOPS", "0");
    let _post = EnvGuard::set("ANGEL_POST_GREEN_TOOL_BATCHES", "0");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    struct Sequence(AtomicUsize);
    impl Club for Sequence {
        fn label(&self) -> &str {
            "owned-target-scope-sequence"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let i = self.0.fetch_add(1, Ordering::SeqCst);
            let (name, args) = match i {
                0 => (
                    "write_file",
                    serde_json::json!({"path":"worker/subject.rs","content":"pub fn answer() -> u8 { 2 }\n"}),
                ),
                1 | 2 => (
                    "check",
                    serde_json::json!({"runtime":"rust","args":"-p owned_parent"}),
                ),
                3 => (
                    "check",
                    serde_json::json!({"runtime":"rust","args":"-p owned_worker"}),
                ),
                _ => return Ok(ClubReply::Text("Observed the worker check.".into())),
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("scope-{i}"),
                name: name.into(),
                args,
            }]))
        }
    }
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    let mut history = vec![ChatMsg::user(
        "Edit the worker and compile the required changed target. Parent compilation is only scoped evidence.",
    )];
    let (tx, rx) = mpsc::channel::<TurnEvent>();
    run_turn(
        &Sequence(AtomicUsize::new(0)),
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(7),
        &tx,
    )
    .unwrap();
    drop(tx);
    let checks: Vec<_> = rx
        .try_iter()
        .filter_map(|event| match event {
            TurnEvent::ToolResult { name, summary, .. } if name == "check" => Some(summary),
            _ => None,
        })
        .collect();
    assert_eq!(
        checks.len(),
        3,
        "a scoped parent check cannot stop the needed worker check: {checks:?}"
    );
    assert!(checks[0].contains("coverage=incomplete"), "{checks:?}");
    assert!(
        checks[1].contains("coverage=incomplete"),
        "repeat scoped check must execute, not reuse a bare green outcome: {checks:?}"
    );
    assert!(
        checks[2].contains("coverage=complete"),
        "required changed worker actually compiled: {checks:?}"
    );
}

#[test]
fn verification_target_ignored_shell_worker_cannot_hide_behind_changed_parent() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    // Turn only this owned fixture's existing worker into ignored input before
    // constructing the registry. It remains the manifest's actual lib source.
    std::fs::write(
        fixture.root.join(".gitignore"),
        "target/\nworker/subject.rs\n",
    )
    .unwrap();
    for args in [
        vec!["rm", "--cached", "--quiet", "--", "worker/subject.rs"],
        vec!["add", ".gitignore"],
        vec![
            "-c",
            "user.name=owned-test",
            "-c",
            "user.email=owned@test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "owned ignored worker input",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(["-c", "core.hooksPath=/dev/null"])
                .args(args)
                .current_dir(&fixture.root)
                .status()
                .unwrap()
                .success()
        );
    }
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    registry.dispatch("write_file", &serde_json::json!({"path":"parent.rs", "content":"pub fn trusted_parent() -> u8 { 2 }\n"})).unwrap();
    registry.dispatch("shell", &serde_json::json!({"command":"printf 'pub fn answer() -> u8 { BROKEN_IGNORED_EDIT }\\n' > worker/subject.rs"})).unwrap();
    assert!(
        std::fs::read_to_string(fixture.root.join("worker/subject.rs"))
            .unwrap()
            .contains("BROKEN_IGNORED_EDIT")
    );
    let (parent, parent_outcome) = target_check(&registry, "");
    let (worker, worker_outcome) = target_check(&registry, "-p owned_worker");
    assert_eq!(
        worker_outcome,
        Some(VerificationOutcome::Failed),
        "actual omitted worker is invalid: {worker}"
    );
    assert!(
        worker.contains("BROKEN_IGNORED_EDIT"),
        "must be the actual compiler diagnostic, not a setup failure: {worker}"
    );
    eprintln!("IGNORED_TARGET parent={parent_outcome:?} receipt={parent}");
    assert!(
        parent_outcome != Some(VerificationOutcome::Passed)
            || !verification_result_covers_changes(&parent),
        "the valid parent edit cannot cover an opaque shell edit to ignored worker source: {parent}"
    );
}

fn target_ignore_worker(fixture: &TargetFixture) {
    std::fs::write(
        fixture.root.join(".gitignore"),
        "target/\nworker/subject.rs\n",
    )
    .unwrap();
    for args in [
        vec!["rm", "--cached", "--quiet", "--", "worker/subject.rs"],
        vec!["add", ".gitignore"],
        vec![
            "-c",
            "user.name=owned-test",
            "-c",
            "user.email=owned@test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "owned finite source scope",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(["-c", "core.hooksPath=/dev/null"])
                .args(args)
                .current_dir(&fixture.root)
                .status()
                .unwrap()
                .success()
        );
    }
}

#[test]
fn verification_target_finite_shell_grant_covers_ignored_worker_and_denies_other_files() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    target_ignore_worker(&fixture);
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    let parent = std::fs::read(fixture.root.join("parent.rs")).unwrap();
    // File grants are actually enforced even with the ordinary YOLO bypass on.
    {
        let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
        registry.dispatch("shell", &serde_json::json!({"command": "python3 -c \"from pathlib import Path; Path('worker/subject.rs').write_text('pub fn answer() -> u8 { 7 }\\n')\"", "write_paths":["worker/subject.rs"]})).unwrap();
        let denied = registry.dispatch("shell", &serde_json::json!({"command":"printf 'outside grant' > parent.rs", "write_paths":["worker/subject.rs"]}));
        assert!(
            denied.is_err(),
            "out-of-grant source write must actually fail: {denied:?}"
        );
    }
    assert_eq!(
        std::fs::read(fixture.root.join("parent.rs")).unwrap(),
        parent
    );
    assert_eq!(
        registry.mutation_targets.snapshot(),
        Some(vec!["worker/subject.rs".into()])
    );
    let (result, outcome) = target_check(&registry, "-p owned_worker");
    assert_eq!(outcome, Some(VerificationOutcome::Passed), "{result}");
    assert!(
        verification_result_covers_changes(&result),
        "enforced ignored source grant has a useful exact package check: {result}"
    );
}

#[test]
fn verification_target_read_only_preserves_scope_and_partial_failure_never_resets() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    crate::agent::sandbox::prime_helper();
    let fixture = TargetFixture::new();
    let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
    let original = std::fs::read(fixture.root.join("parent.rs")).unwrap();
    let _yolo = EnvGuard::set("ANGEL_YOLO", "1");
    assert!(
        registry
            .dispatch(
                "shell",
                &serde_json::json!({"command":"cat parent.rs", "read_only":true})
            )
            .unwrap()
            .contains("trusted_parent")
    );
    assert!(
        registry
            .dispatch(
                "shell",
                &serde_json::json!({"command":"printf 'denied' > parent.rs", "read_only":true})
            )
            .is_err()
    );
    assert_eq!(
        std::fs::read(fixture.root.join("parent.rs")).unwrap(),
        original
    );
    assert_eq!(registry.mutation_targets.snapshot(), Some(Vec::new()));
    let failure = registry.dispatch("shell", &serde_json::json!({"command":"printf 'pub fn answer() -> u8 { 9 }\\n' > worker/subject.rs; exit 3"}));
    assert!(failure.is_err());
    assert!(
        std::fs::read_to_string(fixture.root.join("worker/subject.rs"))
            .unwrap()
            .contains("{ 9 }")
    );
    let generation = registry.mutation_targets.opaque_generation();
    assert!(
        generation > 0,
        "partial failure still had unrestricted write capability"
    );
    registry
        .dispatch(
            "shell",
            &serde_json::json!({"command":"cat worker/subject.rs", "read_only":true}),
        )
        .unwrap();
    registry.dispatch("shell", &serde_json::json!({"command":"printf 'pub fn answer() -> u8 { 10 }\\n' > worker/subject.rs", "write_paths":["worker/subject.rs"]})).unwrap();
    assert_eq!(registry.mutation_targets.opaque_generation(), generation);
    assert!(
        registry.mutation_targets.snapshot().is_none(),
        "later scoped/inspection calls cannot attest earlier opaque writes"
    );
}

#[test]
fn verification_target_scope_rejects_redirects_controls_and_read_only_escalation() {
    let _lock = crate::tests::env_lock();
    let fixture = TargetFixture::new();
    let tool = crate::agent::tools::shell::ShellTool::in_dir(fixture.root.clone());
    #[cfg(unix)]
    std::os::unix::fs::symlink("worker", fixture.root.join("redirected")).unwrap();
    for paths in [
        serde_json::json!(["worker"]),
        serde_json::json!(["worker/missing.rs"]),
        serde_json::json!([".git/config"]),
        serde_json::json!(["Cargo.toml"]),
        serde_json::json!(["../outside.rs"]),
        serde_json::json!(["off-limits/unused.rs"]),
        serde_json::json!(["redirected/subject.rs"]),
        serde_json::json!("parent.rs"),
    ] {
        assert!(
            tool.call(&serde_json::json!({"command":"true", "write_paths":paths}))
                .is_err(),
            "invalid source grant must fail before execution: {paths}"
        );
    }
    assert!(
        tool.call(&serde_json::json!({"command":"true", "read_only":"true"}))
            .is_err()
    );
    let reviewer = crate::agent::tools::shell::ShellTool::read_only_in_dir(fixture.root.clone());
    assert!(reviewer.call(&serde_json::json!({"command":"printf 'bad' > parent.rs", "read_only":false, "write_paths":["parent.rs"]})).is_err());
}

#[test]
fn verification_target_spawn_scope_hook_uses_resolved_grants_without_model_calls() {
    let _lock = crate::tests::env_lock();
    let fixture = TargetFixture::new();
    let tool = crate::agent::harness::spawn::SpawnTool::new(fixture.root.clone(), None, Vec::new());
    for args in [
        serde_json::json!({}),
        serde_json::json!({"tools":"none"}),
        serde_json::json!({"tools":"read_only"}),
        serde_json::json!({"tools":"research"}),
    ] {
        assert!(
            !tool.workspace_write_scope_is_opaque(&args),
            "inspection/research grants do not write workspace sources: {args}"
        );
    }
    assert!(tool.workspace_write_scope_is_opaque(&serde_json::json!({"tools":"code"})));
    let reviewer = tool.nested_for_test(crate::agent::harness::spawn::Grant::ReadOnly);
    assert!(
        !reviewer.workspace_write_scope_is_opaque(&serde_json::json!({"tools":"code"})),
        "a rejected grant cannot escalate the reviewer"
    );
    struct FailedWriter(crate::agent::harness::spawn::SpawnTool);
    impl Tool for FailedWriter {
        fn name(&self) -> &str {
            "owned_failed_spawn"
        }
        fn def(&self) -> ToolDef {
            ToolDef {
                name: self.name().into(),
                description: "owned no-model capability seam".into(),
                params: serde_json::json!({}),
            }
        }
        fn workspace_write_scope_is_opaque(&self, args: &Value) -> bool {
            self.0.workspace_write_scope_is_opaque(args)
        }
        fn call(&self, _: &Value) -> Result<String, String> {
            Err("owned failure after capability dispatch; no model invoked".into())
        }
    }
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(FailedWriter(tool)));
    assert!(
        registry
            .dispatch(
                "owned_failed_spawn",
                &serde_json::json!({"tools":"read_only"})
            )
            .is_err()
    );
    assert!(!registry.mutation_targets.is_opaque());
    assert!(
        registry
            .dispatch("owned_failed_spawn", &serde_json::json!({"tools":"code"}))
            .is_err()
    );
    assert!(
        registry.mutation_targets.is_opaque(),
        "a write-capable dispatch error must not erase its uncertainty"
    );
}

#[test]
fn verification_target_graph_scope_uses_loaded_node_grants_without_model_calls() {
    use crate::agent::harness::agent_graph::{GraphSpec, graph_has_workspace_writes};
    for (grant, expected) in [
        (None, false),
        (Some("read_only"), false),
        (Some("research"), false),
        (Some("code"), true),
        (Some("write"), true),
    ] {
        let spec: GraphSpec = serde_json::from_value(serde_json::json!({"name":"owned-scope", "node":[{"id":"owned-node", "prompt":"unused", "tools":grant}]})).unwrap();
        assert_eq!(graph_has_workspace_writes(&spec).unwrap(), expected);
    }
}

#[test]
fn verification_target_turn_scoped_success_and_opaque_failure_remain_finite() {
    let _lock = crate::tests::env_lock();
    let _mcp = EnvGuard::set("ANGEL_MCP_CONFIG", "/nonexistent/owned-target-mcp.json");
    let _first = EnvGuard::set("ANGEL_FIRST_WRITE_CALLS", "0");
    let _last = EnvGuard::set("ANGEL_FINAL_MILE_HOPS", "0");
    let _verify = EnvGuard::set("ANGEL_VERIFY_BEFORE_DONE", "1");
    crate::agent::sandbox::prime_helper();
    let _offline = EnvGuard::set("CARGO_NET_OFFLINE", "true");
    let _jobs = EnvGuard::set("CARGO_BUILD_JOBS", "1");
    struct Sequence {
        step: AtomicUsize,
        opaque: bool,
    }
    impl Club for Sequence {
        fn label(&self) -> &str {
            "owned-scope-workflow"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn chat(&self, _: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            let i = self.step.fetch_add(1, Ordering::SeqCst);
            let (name, args) = match i {
                0 if self.opaque => (
                    "shell",
                    serde_json::json!({"command":"printf 'pub fn answer() -> u8 { BROKEN_SCOPE_WORKFLOW }\\n' > worker/subject.rs"}),
                ),
                0 => (
                    "shell",
                    serde_json::json!({"command":"printf 'pub fn answer() -> u8 { 7 }\\n' > worker/subject.rs", "write_paths":["worker/subject.rs"]}),
                ),
                1 => (
                    "check",
                    serde_json::json!({"runtime":"rust", "args":"-p owned_worker"}),
                ),
                _ => {
                    return Ok(ClubReply::Text(
                        if self.opaque {
                            "Blocked: the actual worker compiler reports BROKEN_SCOPE_WORKFLOW."
                        } else {
                            "The required worker check passed."
                        }
                        .into(),
                    ));
                }
            };
            Ok(ClubReply::Calls(vec![ToolCall {
                id: format!("workflow-{i}"),
                name: name.into(),
                args,
            }]))
        }
    }
    for opaque in [false, true] {
        let fixture = TargetFixture::new();
        let registry = ToolRegistry::with_team(fixture.root.clone(), Vec::new());
        let club = Sequence {
            step: AtomicUsize::new(0),
            opaque,
        };
        let mut history = vec![ChatMsg::user(
            "Make the worker edit and run its required compiler check. Report the actual result or blocker.",
        )];
        let (tx, rx) = mpsc::channel::<TurnEvent>();
        let answer = run_turn(
            &club,
            &registry,
            &mut history,
            &AtomicBool::new(false),
            Some(5),
            &tx,
        )
        .unwrap();
        assert_eq!(
            club.step.load(Ordering::SeqCst),
            3,
            "scope workflow must finish after the actual check, without unchanged verifier requests: {answer}"
        );
        drop(tx);
        let checks: Vec<_> = rx
            .try_iter()
            .filter_map(|event| match event {
                TurnEvent::ToolResult { name, outcome, .. } if name == "check" => Some(outcome),
                _ => None,
            })
            .collect();
        assert_eq!(checks.len(), 1);
        assert_eq!(
            checks[0].verification,
            if opaque {
                VerificationOutcome::Failed
            } else {
                VerificationOutcome::Passed
            }
        );
        assert_eq!(registry.mutation_targets.is_opaque(), opaque);
    }
}
