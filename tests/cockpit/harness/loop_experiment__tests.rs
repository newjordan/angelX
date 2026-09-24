use super::*;
use crate::agent::club::{ClubReply, StreamDelta};

pub(super) fn fixture(test: impl FnOnce(&Path, &Path)) {
    let _lock = crate::tests::env_lock();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "angel-loop-experiment-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    let cancel = AtomicBool::new(false);
    git_output(&source, &["init", "-q"], &cancel).unwrap();
    std::fs::write(source.join("candidate.txt"), "committed\n").unwrap();
    git_output(&source, &["add", "."], &cancel).unwrap();
    git_output(&source, &["commit", "-q", "-m", "owned fixture"], &cancel).unwrap();
    test(&root, &source);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_snapshot_preserves_dirty_untracked_and_deletion_without_parent_writes() {
    fixture(|root, source| {
        std::fs::write(source.join("candidate.txt"), "dirty parent\n").unwrap();
        std::fs::write(source.join("new.txt"), "untracked\n").unwrap();
        let before =
            git_output(source, &["status", "--porcelain"], &AtomicBool::new(false)).unwrap();
        let copy = root.join("copy");
        std::fs::create_dir(&copy).unwrap();
        let manifest = snapshot_live(source, Some(&copy), &AtomicBool::new(false)).unwrap();
        assert_eq!(
            std::fs::read_to_string(copy.join("candidate.txt")).unwrap(),
            "dirty parent\n"
        );
        assert!(manifest.contains_key(Path::new("new.txt")));
        assert!(!copy.join(".git").exists());
        std::fs::write(copy.join("candidate.txt"), "child only\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(source.join("candidate.txt")).unwrap(),
            "dirty parent\n"
        );
        assert_eq!(
            git_output(source, &["status", "--porcelain"], &AtomicBool::new(false)).unwrap(),
            before
        );
        std::fs::remove_file(source.join("candidate.txt")).unwrap();
        assert!(
            !snapshot_live(source, None, &AtomicBool::new(false))
                .unwrap()
                .contains_key(Path::new("candidate.txt"))
        );
    });
}

#[cfg(unix)]
#[test]
fn live_snapshot_preserves_initialized_submodule_and_refuses_missing_or_escaping_checkout() {
    fixture(|root, source| {
        let cancel = AtomicBool::new(false);
        let dependency = root.join("local-dependency");
        std::fs::create_dir(&dependency).unwrap();
        git_output(&dependency, &["init", "-q"], &cancel).unwrap();
        std::fs::write(dependency.join("live.txt"), "committed dependency\n").unwrap();
        std::fs::write(dependency.join("deleted.txt"), "delete me\n").unwrap();
        git_output(&dependency, &["add", "."], &cancel).unwrap();
        git_output(
            &dependency,
            &["commit", "-q", "-m", "local dependency"],
            &cancel,
        )
        .unwrap();
        git_output(
            source,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "--",
                dependency.to_str().unwrap(),
                "nested/submodule",
            ],
            &cancel,
        )
        .unwrap();
        let checkout = source.join("nested/submodule");
        assert!(checkout.join(".git").is_file());
        std::fs::write(checkout.join("live.txt"), "dirty live dependency\n").unwrap();
        std::fs::write(checkout.join("new.txt"), "untracked dependency\n").unwrap();
        std::fs::remove_file(checkout.join("deleted.txt")).unwrap();
        let before = git_output(source, &["status", "--porcelain"], &cancel).unwrap();
        let copy = root.join("submodule-copy");
        std::fs::create_dir(&copy).unwrap();
        let manifest = snapshot_live(source, Some(&copy), &cancel).unwrap();
        assert_eq!(
            std::fs::read_to_string(copy.join("nested/submodule/live.txt")).unwrap(),
            "dirty live dependency\n"
        );
        assert_eq!(
            std::fs::read_to_string(copy.join("nested/submodule/new.txt")).unwrap(),
            "untracked dependency\n"
        );
        assert!(!manifest.contains_key(Path::new("nested/submodule/deleted.txt")));
        assert!(!copy.join("nested/submodule/.git").exists());
        assert_eq!(
            git_output(source, &["status", "--porcelain"], &cancel).unwrap(),
            before
        );

        // Leave the indexed gitlink in place but remove its checkout. An
        // empty directory must not cause Git to inventory the parent again.
        let parked = root.join("parked-submodule");
        std::fs::rename(&checkout, &parked).unwrap();
        std::fs::create_dir(&checkout).unwrap();
        assert!(
            active_files(source, &cancel)
                .unwrap_err()
                .contains("uninitialized")
        );
        std::fs::remove_dir(&checkout).unwrap();
        assert!(active_files(source, &cancel).is_err());
        std::os::unix::fs::symlink(&dependency, &checkout).unwrap();
        assert!(
            active_files(source, &cancel)
                .unwrap_err()
                .contains("directory refused")
        );
    });
}

#[cfg(unix)]
#[test]
fn snapshot_prunes_owned_excluded_fixture_and_rejects_symlink_escape() {
    fixture(|root, source| {
        // This is an owned temporary fixture, never the real archive.
        let excluded = source.join("off-limits");
        std::fs::create_dir(&excluded).unwrap();
        std::os::unix::fs::symlink(
            root.join("absent-unreadable-target"),
            excluded.join("never-follow"),
        )
        .unwrap();
        let copy = root.join("copy");
        std::fs::create_dir(&copy).unwrap();
        snapshot_live(source, Some(&copy), &AtomicBool::new(false)).unwrap();
        assert!(!copy.join("off-limits").exists());
        std::os::unix::fs::symlink(root, source.join("escape")).unwrap();
        assert!(snapshot_live(source, None, &AtomicBool::new(false)).is_err());
        assert!(!active_relative(Path::new("../escape")));
        assert!(!active_relative(Path::new("nested/off-limits/file")));
    });
}

#[cfg(unix)]
#[test]
fn snapshot_reproduces_links_that_cannot_alias_the_live_tree() {
    fixture(|root, source| {
        // A build tree's link to a system library (apollo's
        // lib/libcrypto.so -> /usr/lib/.../libcrypto.so.3) and an in-tree
        // soname link are reproduced as links, never followed.
        let system = root.join("system");
        std::fs::create_dir(&system).unwrap();
        std::fs::write(system.join("libcrypto.so.3"), "system library\n").unwrap();
        std::fs::create_dir(source.join("lib")).unwrap();
        std::os::unix::fs::symlink(
            system.join("libcrypto.so.3"),
            source.join("lib/libcrypto.so"),
        )
        .unwrap();
        std::fs::write(source.join("lib/libfoo.so.1"), "in-tree library\n").unwrap();
        std::os::unix::fs::symlink("libfoo.so.1", source.join("lib/libfoo.so")).unwrap();
        let copy = root.join("copy");
        std::fs::create_dir(&copy).unwrap();
        let manifest = snapshot_live(source, Some(&copy), &AtomicBool::new(false)).unwrap();
        assert_eq!(
            std::fs::read_link(copy.join("lib/libcrypto.so")).unwrap(),
            system.join("libcrypto.so.3")
        );
        assert_eq!(
            std::fs::read_link(copy.join("lib/libfoo.so")).unwrap(),
            Path::new("libfoo.so.1")
        );
        assert_eq!(
            std::fs::read_to_string(copy.join("lib/libfoo.so")).unwrap(),
            "in-tree library\n"
        );
        assert_eq!(
            manifest[Path::new("lib/libcrypto.so")].symlink.as_deref(),
            Some(system.join("libcrypto.so.3").as_path())
        );
        // A link-free entry serializes as before, so existing digests hold.
        let entry = serde_json::to_string(&manifest[Path::new("candidate.txt")]).unwrap();
        assert!(!entry.contains("symlink"), "{entry}");

        // Links that could lead the copy back into the live tree are refused.
        for (name, target) in [
            ("up", PathBuf::from("../..")),
            ("quarantine", PathBuf::from("off-limits/secret")),
            ("live", source.join("candidate.txt")),
            ("parent", root.to_path_buf()),
        ] {
            let link = source.join(name);
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let error = snapshot_live(source, None, &AtomicBool::new(false)).unwrap_err();
            assert!(
                error.contains("snapshot refuses symlink"),
                "{name}: {error}"
            );
            std::fs::remove_file(&link).unwrap();
        }
    });
}

pub(super) struct SelectedLeaf {
    pub(super) calls: AtomicUsize,
    pub(super) held: bool,
    pub(super) entered: Arc<AtomicBool>,
}
impl Club for SelectedLeaf {
    fn label(&self) -> &str {
        "owned-glm-route"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("unexpected unobserved call".into())
    }
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        _delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.entered.store(true, Ordering::Release);
        assert!(
            messages
                .iter()
                .any(|message| message.content.contains("[LOOP RECOVERY EXPERIMENT]"))
        );
        assert!(!tools.iter().any(|tool| {
            ["spawn", "delegate", "agent_graph", "proc_run"].contains(&tool.name.as_str())
        }));
        if self.held {
            while !cancel.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(5));
            }
            return Err("owned provider observed cancellation".into());
        }
        if self.calls.fetch_add(1, Ordering::Relaxed) == 0 {
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "owned-edit".into(),
                name: "write_file".into(),
                args: serde_json::json!({"path":"candidate.txt", "content":"child candidate\n"}),
            }]));
        }
        Ok(ClubReply::Text(
            "DIRECTION: the isolated candidate is ready for the pinned evaluator".into(),
        ))
    }
}

pub(super) fn request(root: &Path, source: &Path) -> LoopExperimentRequest {
    LoopExperimentRequest {
        workspace: source.to_path_buf(),
        artifact_dir: root.join("artifacts"),
        task: "Make one local candidate edit and report evidence".into(),
        max_hops: 3,
        deadline_secs: 10,
        token_budget: 32_768,
        verify_command: Some("test \"$(cat candidate.txt)\" = 'child candidate'".into()),
        policy_note: None,
    }
}

#[test]
fn git_capture_preserves_large_output_and_refuses_output_over_bound() {
    fixture(|_root, source| {
        let bytes = vec![b'x'; 2 * 1024 * 1024];
        std::fs::write(source.join("large.txt"), &bytes).unwrap();
        let cancel = AtomicBool::new(false);
        git_output(source, &["add", "large.txt"], &cancel).unwrap();
        git_output(
            source,
            &["commit", "-q", "-m", "owned large output"],
            &cancel,
        )
        .unwrap();
        assert_eq!(
            git_output(source, &["show", "HEAD:large.txt"], &cancel).unwrap(),
            bytes
        );
        assert!(git_output_with_limit(source, &["show", "HEAD:large.txt"], &cancel, 1024).is_err());
    });
}

#[test]
fn nested_git_inventory_copies_live_source_and_preserves_tracked_ignored_baseline() {
    fixture(|root, source| {
        let nested = source.join("nested");
        std::fs::create_dir(&nested).unwrap();
        let cancel = AtomicBool::new(false);
        git_output(&nested, &["init", "-q"], &cancel).unwrap();
        std::fs::write(nested.join("tracked.txt"), "committed").unwrap();
        git_output(&nested, &["add", "."], &cancel).unwrap();
        git_output(
            &nested,
            &["commit", "-q", "-m", "nested owned base"],
            &cancel,
        )
        .unwrap();
        std::fs::write(nested.join("tracked.txt"), "nested dirty").unwrap();
        std::fs::write(nested.join("untracked.txt"), "nested untracked").unwrap();
        std::fs::write(source.join(".gitignore"), "candidate.txt\n").unwrap();
        std::fs::create_dir(nested.join("off-limits")).unwrap();
        std::fs::write(
            nested.join("off-limits/owned-excluded.txt"),
            "fixture excluded",
        )
        .unwrap();
        let copy = root.join("copy");
        std::fs::create_dir(&copy).unwrap();
        let manifest = snapshot_live(source, Some(&copy), &cancel).unwrap();
        assert!(manifest.contains_key(Path::new("candidate.txt")));
        assert!(manifest.contains_key(Path::new("nested/untracked.txt")));
        assert_eq!(
            std::fs::read_to_string(copy.join("nested/tracked.txt")).unwrap(),
            "nested dirty"
        );
        assert!(!copy.join("nested/.git").exists());
        assert!(!copy.join("nested/off-limits").exists());
        git_output(&copy, &["init", "-q"], &cancel).unwrap();
        git_output(&copy, &["add", "-f", "-A", "--", "."], &cancel).unwrap();
        let tracked = git_output(&copy, &["ls-files", "candidate.txt"], &cancel).unwrap();
        assert_eq!(tracked, b"candidate.txt\n");
    });
}

/// Explicit local read-only source acceptance. Set the source path only for
/// an authorized active workspace; the copied fixture always lives in /tmp.
#[test]
#[ignore = "requires an explicitly scoped active source via ANGEL_EXPERIMENT_SNAPSHOT_SOURCE"]
fn actual_active_source_snapshot_without_model() {
    let source =
        std::env::var_os("ANGEL_EXPERIMENT_SNAPSHOT_SOURCE").expect("explicit source path");
    let source = PathBuf::from(source).canonicalize().unwrap();
    assert!(
        !source
            .components()
            .any(|part| part.as_os_str() == "off-limits")
    );
    let root = std::env::temp_dir().join(format!(
        "angel-active-snapshot-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let cancel = AtomicBool::new(false);
    let result = (|| {
        let first = snapshot_live(&source, Some(&root), &cancel)?;
        let second = snapshot_live(&source, None, &cancel)?;
        if first != second {
            return Err("active source changed during snapshot acceptance".to_string());
        }
        let bytes = first
            .keys()
            .map(|path| std::fs::metadata(root.join(path)).unwrap().len())
            .sum::<u64>();
        println!(
            "actual active snapshot: files={} bytes={bytes} sha256={}",
            first.len(),
            manifest_hash(&first)?
        );
        assert!(!root.join("off-limits").exists());
        assert!(!root.join(".git").exists());
        Ok::<(), String>(())
    })();
    std::fs::remove_dir_all(root).unwrap();
    result.unwrap();
}

#[test]
fn native_worker_uses_selected_leaf_and_returns_reconstructible_evaluated_patch() {
    fixture(|root, source| {
        std::fs::write(source.join("candidate.txt"), "dirty parent\n").unwrap();
        let entered = Arc::new(AtomicBool::new(false));
        let club = Arc::new(SelectedLeaf {
            calls: AtomicUsize::new(0),
            held: false,
            entered,
        });
        let result = run_loop_experiment(
            request(root, source),
            club.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(result.error.is_none(), "{result:?}");
        assert!(club.calls.load(Ordering::Acquire) > 0);
        assert_eq!(result.requested_route.driver, "owned-glm-route");
        assert_eq!(result.verification.as_ref().unwrap().exit_code, Some(0));
        assert!(result.patch_path.as_ref().unwrap().is_file());
        assert_eq!(
            std::fs::read_to_string(root.join("artifacts/baseline/candidate.txt")).unwrap(),
            "dirty parent\n"
        );
        assert_eq!(
            std::fs::read_to_string(source.join("candidate.txt")).unwrap(),
            "dirty parent\n"
        );
        assert!(root.join("artifacts/working").is_dir());
        assert!(root.join("artifacts/result.json").is_file());
        let bytes = std::fs::read(root.join("artifacts/result.json")).unwrap();
        assert_eq!(
            result.result_sha256,
            Some(crate::knowledge::cut::sha256_hex(&bytes))
        );
        assert_eq!(
            result.patch_sha256,
            Some(crate::knowledge::cut::sha256_hex(
                &std::fs::read(result.patch_path.as_ref().unwrap()).unwrap()
            ))
        );
        assert_eq!(
            result.task_sha256,
            crate::knowledge::cut::sha256_hex(request(root, source).task.as_bytes())
        );
        let persisted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(persisted["snapshot_sha256"], result.snapshot_sha256);
        assert!(result.model_phase_entered);
    });
}

#[test]
fn held_native_provider_cancels_and_preserves_partial_source() {
    fixture(|root, source| {
        let entered = Arc::new(AtomicBool::new(false));
        let club = Arc::new(SelectedLeaf {
            calls: AtomicUsize::new(0),
            held: true,
            entered: Arc::clone(&entered),
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let req = request(root, source);
        let worker = std::thread::spawn(move || run_loop_experiment(req, club, worker_cancel));
        let start = Instant::now();
        while !entered.load(Ordering::Acquire) && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        cancel.store(true, Ordering::Release);
        let result = worker.join().unwrap().unwrap();
        assert!(entered.load(Ordering::Acquire), "{result:?}");
        assert_eq!(result.stop_reason, "interrupt");
        assert!(root.join("artifacts/working/candidate.txt").is_file());
        assert!(result.verification.is_none());
    });
}

#[test]
fn tiny_reserved_budget_prevents_provider_dispatch() {
    fixture(|root, source| {
        let entered = Arc::new(AtomicBool::new(false));
        let club = Arc::new(SelectedLeaf {
            calls: AtomicUsize::new(0),
            held: false,
            entered: Arc::clone(&entered),
        });
        let mut req = request(root, source);
        req.token_budget = 1;
        let result = run_loop_experiment(req, club, Arc::new(AtomicBool::new(false))).unwrap();
        assert!(!entered.load(Ordering::Acquire));
        assert_eq!(result.stop_reason, "token_budget");
        assert!(root.join("artifacts/baseline/candidate.txt").is_file());
    });
}

#[test]
fn held_verifier_observes_stop_and_keeps_tool_and_working_evidence() {
    fixture(|root, source| {
        let entered = Arc::new(AtomicBool::new(false));
        let club = Arc::new(SelectedLeaf {
            calls: AtomicUsize::new(0),
            held: false,
            entered,
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let mut req = request(root, source);
        req.verify_command = Some("touch verifier-entered; sleep 30".into());
        let worker = std::thread::spawn(move || run_loop_experiment(req, club, worker_cancel));
        let start = Instant::now();
        let marker = root.join("artifacts/working/verifier-entered");
        while !marker.exists() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        cancel.store(true, Ordering::Release);
        let result = worker.join().unwrap().unwrap();
        assert!(marker.exists(), "{result:?}");
        assert_eq!(result.stop_reason, "interrupt");
        assert!(result.verification.as_ref().unwrap().cancelled);
        let trace = std::fs::read_to_string(root.join("artifacts/tool-events.json")).unwrap();
        assert!(trace.contains("write_file") && trace.contains("tool_result"));
        assert!(root.join("artifacts/working/candidate.txt").is_file());
    });
}
