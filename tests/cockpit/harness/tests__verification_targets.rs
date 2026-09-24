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
