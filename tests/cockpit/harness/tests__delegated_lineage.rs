//! Owned native delegate → exact integration → sealed audit controls.
use super::*;
use crate::club::{RouteIdentity, StreamDelta};

struct Child {
    calls: AtomicUsize,
    write: bool,
}
impl Club for Child {
    fn label(&self) -> &str {
        "lineage-child"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        unreachable!()
    }
    fn resolved_route_identity(&self) -> RouteIdentity {
        RouteIdentity {
            driver: "scripted-child".into(),
            model: Some("child-observed-v1".into()),
            reasoning_effort: None,
        }
    }
    fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        assert!(
            messages
                .iter()
                .any(|m| m.role == ChatRole::User && m.content.as_ref() == "owned child task")
        );
        if self.write && self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "owned-write".into(),
                name: "shell".into(),
                args: serde_json::json!({"command":"printf 'owned child bytes\\n' > child.txt"}),
            }]))
        } else {
            Ok(ClubReply::Text("owned child answer".into()))
        }
    }
}

struct Parent {
    calls: AtomicUsize,
    workspace: PathBuf,
}
impl Club for Parent {
    fn label(&self) -> &str {
        "lineage-parent"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        unreachable!()
    }
    fn resolved_route_identity(&self) -> RouteIdentity {
        RouteIdentity {
            driver: "scripted-parent".into(),
            model: Some("parent-observed-v1".into()),
            reasoning_effort: None,
        }
    }
    fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
        match self.calls.fetch_add(1, Ordering::SeqCst) {
            0 => Ok(ClubReply::Calls(vec![ToolCall {
                id: "parent-delegate".into(),
                name: "delegate".into(),
                args: serde_json::json!({"club":"lineage-child","task":"owned child task"}),
            }])),
            1 => {
                assert!(
                    !self.workspace.join("child.txt").exists(),
                    "delegate must stay isolated until integration"
                );
                let result = messages
                    .iter()
                    .rev()
                    .find(|m| m.role == ChatRole::Tool)
                    .unwrap();
                let branch = branch(&result.content);
                Ok(ClubReply::Calls(vec![ToolCall {
                    id: "parent-integrate".into(),
                    name: "integrate".into(),
                    args: serde_json::json!({"branch":branch}),
                }]))
            }
            _ => Ok(ClubReply::Text("owned parent answer".into())),
        }
    }
}

fn branch(text: &str) -> String {
    text.split("branch=")
        .nth(1)
        .expect("actual delegate branch header")
        .split(']')
        .next()
        .unwrap()
        .to_string()
}

fn fixture_env(root: &Path) -> Vec<EnvGuard> {
    for directory in ["config", "cache", "state", "atlas", "trajectory"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    vec![
        EnvGuard::unset("HOME"),
        EnvGuard::set("XDG_CONFIG_HOME", root.join("config").to_str().unwrap()),
        EnvGuard::set("XDG_CACHE_HOME", root.join("cache").to_str().unwrap()),
        EnvGuard::set("XDG_STATE_HOME", root.join("state").to_str().unwrap()),
        EnvGuard::set("ANGEL_ATLAS_DIR", root.join("atlas").to_str().unwrap()),
        EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", "shadow"),
        EnvGuard::set(
            "ANGEL_HARNESS_ROLLOUT_DIR",
            root.join("rollouts").to_str().unwrap(),
        ),
        EnvGuard::set("ANGEL_TRAJECTORY_LOG", "1"),
        EnvGuard::set(
            "ANGEL_TRAJECTORY_DIR",
            root.join("trajectory").to_str().unwrap(),
        ),
        EnvGuard::set("ANGEL_REPO_IDENTITY", "1"),
        EnvGuard::set("ANGEL_ADVISOR", "0"),
        EnvGuard::set("ANGEL_TEACHER_WATCH", "0"),
        EnvGuard::set("ANGEL_TOOL_BUBBLE", "0"),
        EnvGuard::set("ANGEL_YOLO", "1"),
        EnvGuard::set("ANGEL_SOTA_SOLO", "0"),
        EnvGuard::set("ANGEL_AUTO_RECALL", "0"),
        EnvGuard::set("ANGEL_CUT", "0"),
        EnvGuard::set("ANGEL_TASK_ID", "unrelated-parent-task"),
        EnvGuard::set("ANGEL_RUN_ID", "unrelated-parent-run"),
    ]
}

fn registry(workspace: &Path, child: Arc<dyn Club>) -> ToolRegistry {
    ensure_git_workspace(workspace).unwrap();
    let mut registry = ToolRegistry::new();
    registry.set_workspace(workspace.to_path_buf());
    registry.register(Box::new(DelegateTool::new(
        workspace.to_path_buf(),
        vec![child],
    )));
    registry.register(Box::new(IntegrateTool::new(workspace.to_path_buf())));
    registry
}

#[test]
fn delegated_lineage_native_parent_reaudits_child_and_exact_applied_commit() {
    let _lock = crate::tests::env_lock();
    let root = scratch("delegated-lineage");
    let _env = fixture_env(&root);
    let workspace = root.join("repo");
    let registry = registry(
        &workspace,
        Arc::new(Child {
            calls: AtomicUsize::new(0),
            write: true,
        }),
    );
    let parent = Parent {
        calls: AtomicUsize::new(0),
        workspace: workspace.clone(),
    };
    let mut history = vec![ChatMsg::user("owned parent task")];
    let outcome = crate::harness::turn::run_turn_observed(
        &parent,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(6),
        &mpsc::channel().0,
    )
    .unwrap();
    assert!(!outcome.interrupted);
    assert_eq!(
        std::fs::read_to_string(workspace.join("child.txt")).unwrap(),
        "owned child bytes\n"
    );
    let id = outcome.rollout_id.unwrap();
    let receipt =
        crate::harness::rollout::audit_workspace_rollout_receipt(&workspace, &id).unwrap();
    let coverage = &receipt["auxiliary_coverage"];
    assert_eq!(coverage["schema"], "angel-native-auxiliary-coverage/v2");
    assert_eq!(coverage["complete"], true);
    assert_eq!(coverage["unlinked_auxiliary_operations"], 0);
    let links = coverage["linked_operations"].as_array().unwrap();
    assert_eq!(links.len(), 2);
    assert_ne!(links[0]["operation_id"], links[1]["operation_id"]);
    assert_eq!(links[0]["artifact"], links[1]["artifact"]);
    assert_eq!(
        links[0]["artifact"]["child_audit"]["resolved_actions"]["routes"][0]["route"]["model_revision"],
        "child-observed-v1"
    );
    assert_eq!(
        receipt["resolved_actions"]["routes"][0]["route"]["model_revision"],
        "parent-observed-v1"
    );
    assert_ne!(
        links[0]["artifact"]["child_audit"]["task_binding"]["task_id"],
        "unrelated-parent-task"
    );
    assert_eq!(
        links[0]["artifact"]["task_sha256"],
        crate::cut::sha256_hex(b"owned child task")
    );
    assert_eq!(
        links[1]["application"]["after_commit"],
        run_git(&workspace, &["rev-parse", "HEAD"]).unwrap().trim()
    );
    assert_eq!(
        links[1]["application"]["after_tree"],
        run_git(&workspace, &["rev-parse", "HEAD^{tree}"])
            .unwrap()
            .trim()
    );
    let worktrees = run_git(&workspace, &["worktree", "list", "--porcelain"]).unwrap();
    assert_eq!(
        worktrees
            .lines()
            .filter(|line| line.starts_with("worktree "))
            .count(),
        1
    );
    // Consumer parity can reuse these exact producer/auditor bytes. This is an
    // explicit owned test output, never a production environment knob.
    let export = std::env::var_os("ANGEL_DELEGATED_LINEAGE_FIXTURE").map(PathBuf::from);
    if let Some(path) = export.as_ref() {
        std::fs::write(path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
        std::fs::write(
            path.with_extension("fixture.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema":"angel-owned-delegated-lineage-fixture/v1",
                "workspace": workspace, "rollout_id": id, "root": root,
                "simulated_provider": true,
                "source_sha256": crate::harness::run_identity::source_sha256(),
            }))
            .unwrap(),
        )
        .unwrap();
    }
    // Even a newly self-hashed child manifest cannot replace its sealed journal.
    let repo_key = crate::workspace_store::repo_identity(&workspace).key;
    let child = links[0]["artifact"]["child_rollout_id"].as_str().unwrap();
    let path = root
        .join("rollouts")
        .join(repo_key)
        .join("runs")
        .join(child)
        .join("manifest.json");
    let original = std::fs::read(&path).unwrap();
    let store = crate::harness::rollout::RolloutStore::for_workspace(&workspace);
    let repo_key = crate::workspace_store::repo_identity(&workspace).key;
    let mut forged = store.load_manifest(child, &repo_key).unwrap();
    forged
        .auxiliary_coverage
        .as_mut()
        .unwrap()
        .observed_tool_calls = 9;
    forged.manifest_sha256 = None;
    forged.manifest_sha256 = Some(crate::cut::sha256_hex(
        &serde_json::to_vec(&forged).unwrap(),
    ));
    std::fs::write(&path, serde_json::to_vec(&forged).unwrap()).unwrap();
    assert_eq!(
        store
            .load_manifest(child, &repo_key)
            .unwrap()
            .auxiliary_coverage
            .unwrap()
            .observed_tool_calls,
        9,
        "forged typed manifest must pass its own digest before journal audit"
    );
    let error =
        crate::harness::rollout::audit_workspace_rollout_receipt(&workspace, &id).unwrap_err();
    assert!(
        error.contains("manifest does not match its journal"),
        "{error}"
    );
    std::fs::write(path, original).unwrap();
    std::fs::remove_dir_all(DelegateTool::new(workspace.clone(), vec![]).worktree_base()).unwrap();
    if export.is_none() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn delegated_lineage_branch_movement_cannot_apply_a_different_artifact() {
    let _lock = crate::tests::env_lock();
    let root = scratch("delegated-moved");
    let _env = fixture_env(&root);
    let workspace = root.join("repo");
    let registry = registry(
        &workspace,
        Arc::new(Child {
            calls: AtomicUsize::new(0),
            write: true,
        }),
    );
    let scope = registry.auxiliary.enter();
    let text = registry
        .dispatch(
            "delegate",
            &serde_json::json!({"club":"lineage-child","task":"owned child task"}),
        )
        .unwrap();
    assert!(scope.snapshot().complete);
    let branch = branch(&text);
    run_git(
        &workspace,
        &["update-ref", &format!("refs/heads/{branch}"), "HEAD"],
    )
    .unwrap();
    let before = run_git(&workspace, &["rev-parse", "HEAD"]).unwrap();
    let error = registry
        .dispatch("integrate", &serde_json::json!({"branch":branch}))
        .unwrap_err();
    assert!(error.contains("changed since capture"), "{error}");
    assert_eq!(run_git(&workspace, &["rev-parse", "HEAD"]).unwrap(), before);
    assert!(!workspace.join("child.txt").exists());
    let coverage = scope.snapshot();
    assert_eq!(coverage.linked_operations.len(), 1);
    assert_eq!(coverage.sources["model_tool"], 1);
    assert!(!coverage.complete);
    drop(scope);
    std::fs::remove_dir_all(DelegateTool::new(workspace, vec![]).worktree_base()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn delegated_lineage_missing_capture_and_overlapping_turns_remain_unknown() {
    let _lock = crate::tests::env_lock();
    let root = scratch("delegated-unknown");
    let _env = fixture_env(&root);
    let workspace = root.join("repo");
    let registry = registry(
        &workspace,
        Arc::new(Child {
            calls: AtomicUsize::new(0),
            write: false,
        }),
    );
    for capture in ["off", "shadow"] {
        let _capture = EnvGuard::set("ANGEL_HARNESS_ROLLOUTS", capture);
        let scope = registry.auxiliary.enter();
        let overlapping = (capture == "shadow").then(|| registry.auxiliary.enter());
        registry.dispatch("delegate", &serde_json::json!({"club":"lineage-child","task":"owned child task","mode":"read_only"})).unwrap();
        let actual = scope.snapshot();
        assert!(!actual.complete);
        assert_eq!(actual.unlinked_auxiliary_operations, 1);
        assert!(actual.linked_operations.is_empty());
        drop(overlapping);
    }
    std::fs::remove_dir_all(DelegateTool::new(workspace, vec![]).worktree_base()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

struct HeldChild {
    entered: mpsc::Sender<()>,
}

#[test]
fn delegated_lineage_worktree_excludes_synthetic_tree_before_population() {
    let _lock = crate::tests::env_lock();
    let root = scratch("delegated-sparse");
    let _env = fixture_env(&root);
    let repo = root.join("repo");
    ensure_git_workspace(&repo).unwrap();
    std::fs::create_dir_all(repo.join("owned-excluded")).unwrap();
    std::fs::write(
        repo.join("owned-excluded/synthetic.txt"),
        "owned excluded bytes\n",
    )
    .unwrap();
    std::fs::write(repo.join("live.txt"), "owned baseline\n").unwrap();
    run_git(&repo, &["add", "-A"]).unwrap();
    run_git(&repo, &["commit", "-qm", "owned sparse fixture"]).unwrap();
    std::fs::write(repo.join("live.txt"), "parent dirty bytes remain\n").unwrap();
    let worktree = root.join("child");
    crate::harness::orchestrator::prepare_delegate_worktree(
        &repo,
        &worktree,
        "owned-sparse-child",
        "HEAD",
        "owned-excluded",
        None,
    )
    .unwrap();
    assert!(!worktree.join("owned-excluded").exists());
    assert_eq!(
        std::fs::read_to_string(worktree.join("live.txt")).unwrap(),
        "owned baseline\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("live.txt")).unwrap(),
        "parent dirty bytes remain\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("owned-excluded/synthetic.txt")).unwrap(),
        "owned excluded bytes\n"
    );
    assert!(
        run_git(&worktree, &["status", "--porcelain"])
            .unwrap()
            .trim()
            .is_empty()
    );
    std::fs::write(worktree.join("live.txt"), "child edited bytes\n").unwrap();
    run_git(&worktree, &["add", "-A"]).unwrap();
    assert_eq!(
        run_git(&worktree, &["diff", "--cached", "--name-only"])
            .unwrap()
            .trim(),
        "live.txt",
        "sparse add must not stage deletion of excluded tracked content"
    );
    run_git(
        &repo,
        &["worktree", "remove", "--force", worktree.to_str().unwrap()],
    )
    .unwrap();
    assert!(!worktree.exists());
    std::fs::remove_dir_all(root).unwrap();
}
impl Club for HeldChild {
    fn label(&self) -> &str {
        "held-child"
    }
    fn respond(&self, _: &str) -> Result<String, String> {
        unreachable!()
    }
    fn chat_streaming(
        &self,
        _: &[ChatMsg],
        _: &[ToolDef],
        cancel: &AtomicBool,
        _: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.entered.send(()).unwrap();
        let limit = Instant::now() + Duration::from_secs(5);
        while !cancel.load(Ordering::Relaxed) && Instant::now() < limit {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !cancel.load(Ordering::Relaxed) {
            return Err("fixture cancellation not propagated".into());
        }
        Err("owned child observed cancellation".into())
    }
}

#[test]
fn delegated_lineage_cancellation_settles_child_without_resolving_parent_operation() {
    let _lock = crate::tests::env_lock();
    let root = scratch("delegated-cancel");
    let _env = fixture_env(&root);
    let workspace = root.join("repo");
    let (tx, rx) = mpsc::channel();
    let registry = registry(&workspace, Arc::new(HeldChild { entered: tx }));
    let scope = registry.auxiliary.enter();
    let cancel = AtomicBool::new(false);
    let error = std::thread::scope(|threads| {
        let worker = threads.spawn(|| {
            registry.dispatch_with_cancel(
                "delegate",
                &serde_json::json!({
            "club":"held-child", "task":"owned child task", "mode":"read_only"}),
                Some(&cancel),
            )
        });
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            owned_delegate_snapshot(&cancel as *const _ as usize).is_some(),
            "a running delegate must remain visible under its operator cancel owner"
        );
        cancel.store(true, Ordering::Relaxed);
        worker.join().unwrap().unwrap_err()
    });
    assert!(
        error.contains("rollout=rol-"),
        "failure retains child capture identity: {error}"
    );
    assert!(
        owned_delegate_snapshot(&cancel as *const _ as usize).is_none(),
        "cancelled delegate must release the live display entry"
    );
    let coverage = scope.snapshot();
    assert_eq!(coverage.unlinked_auxiliary_operations, 1);
    assert!(coverage.linked_operations.is_empty());
    assert!(!coverage.complete);
    assert_eq!(
        run_git(&workspace, &["worktree", "list", "--porcelain"])
            .unwrap()
            .lines()
            .filter(|line| line.starts_with("worktree "))
            .count(),
        1
    );
    std::fs::remove_dir_all(DelegateTool::new(workspace, vec![]).worktree_base()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
