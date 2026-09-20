use super::*;

#[test]
fn fingerprint_git_probe_does_not_renew_an_expired_shared_budget() {
    let expired = Instant::now() - Duration::from_millis(1);
    assert!(git_output(Path::new("."), &["--version"], expired).is_none());
}

fn git(root: &Path, args: &[&str]) {
    let status = pinned_git_command(root, args).status().unwrap();
    assert!(status.success(), "git {args:?}");
}

fn commit(root: &Path, message: &str) {
    git(
        root,
        &[
            "-c",
            "user.name=Angel Test",
            "-c",
            "user.email=angel@example.invalid",
            "commit",
            "-qm",
            message,
        ],
    );
}

#[cfg(target_os = "linux")]
#[test]
fn evidence_ignores_scratch_and_sandbox_diagnostics_but_binds_candidate_bytes() {
    let _lock = crate::tests::env_lock();
    let fixture = crate::sandbox::HardlinkTestRoot::new();
    let root = fixture.path();
    git(root, &["init", "-q"]);
    std::fs::write(root.join("candidate.txt"), "before").unwrap();
    git(root, &["add", "candidate.txt"]);
    commit(root, "evidence baseline");
    let _confined = ConfinedRecoveryGit::new(root).unwrap();
    let before = workspace_evidence_sha256(root).unwrap();
    let paths_before = workspace_evidence_paths(root).unwrap();
    let probe_before = git_stream_output(root, &["rev-parse", "HEAD"], |_| true).unwrap();
    let scratch = root.join(".angel-experiment-tmp");
    std::fs::create_dir(&scratch).unwrap();
    std::fs::write(scratch.join("counter"), "x").unwrap();
    // Exclusion is a product contract, independent of .git/info/exclude.
    let probe_after = git_stream_output(root, &["rev-parse", "HEAD"], |_| true).unwrap();
    assert_eq!(probe_before.stdout.sha256, probe_after.stdout.sha256);
    assert_ne!(probe_before.stderr.sha256, probe_after.stderr.sha256);
    assert_eq!(workspace_evidence_sha256(root).unwrap(), before);
    assert_eq!(workspace_evidence_paths(root).unwrap(), paths_before);
    git(root, &["add", "-f", ".angel-experiment-tmp/counter"]);
    assert_eq!(workspace_evidence_sha256(root).unwrap(), before);
    std::fs::write(scratch.join("counter"), "xx").unwrap();
    assert_eq!(workspace_evidence_sha256(root).unwrap(), before);
    assert_eq!(workspace_evidence_paths(root).unwrap(), paths_before);

    std::fs::write(root.join("candidate.txt"), "after!").unwrap();
    assert_ne!(workspace_evidence_sha256(root).unwrap(), before);
    let paths_after = workspace_evidence_paths(root).unwrap();
    assert_ne!(
        paths_before[Path::new("candidate.txt")],
        paths_after[Path::new("candidate.txt")]
    );
    std::fs::write(root.join("new.txt"), "new").unwrap();
    assert!(
        workspace_evidence_paths(root)
            .unwrap()
            .contains_key(Path::new("new.txt"))
    );
    std::fs::remove_file(root.join("candidate.txt")).unwrap();
    assert_ne!(
        workspace_evidence_paths(root).unwrap()[Path::new("candidate.txt")],
        paths_after[Path::new("candidate.txt")]
    );
}

#[test]
fn sha256_padding_boundaries_preserve_exact_digests() {
    // Fixed zero-byte vectors cross both the padding and compression edges.
    for (len, expected) in [
        (
            55,
            "02779466cdec163811d078815c633f21901413081449002f24aa3e80f0b88ef7",
        ),
        (
            56,
            "d4817aa5497628e7c77e6b606107042bbba3130888c5f47a375e6179be789fbb",
        ),
        (
            63,
            "c7723fa1e0127975e49e62e753db53924c1bd84b8ac1ac08df78d09270f3d971",
        ),
        (
            64,
            "f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a92759fb4b",
        ),
        (
            65,
            "98ce42deef51d40269d542f5314bef2c7468d401ad5d85168bfab4c0108f75f7",
        ),
        (
            119,
            "f616b0d54e78571a9611f343c9f8e022e859e920381ab0e4d3da01e193a7bd7e",
        ),
        (
            120,
            "6edd9f6f9cc92cded36e6c4a580933f9c9f1b90562b46903b806f21902a1a54f",
        ),
        (
            127,
            "15dae5979058bfbf4f9166029b6e340ea3ca374fef578a11dc9e6e923860d7ae",
        ),
        (
            128,
            "38723a2e5e8a17aa7950dc008209944e898f69a7bd10a23c839d341e935fd5ca",
        ),
    ] {
        let bytes = vec![0; len];
        assert_eq!(crate::cut::sha256_hex(&bytes), expected, "len={len}");
        assert_eq!(sha256_reader_hex(&mut bytes.as_slice()).unwrap(), expected);
        for split in 0..=len {
            let mut streaming = StreamingSha256::new();
            streaming.update(&bytes[..split]);
            let mut cloned = streaming.clone();
            streaming.update(&[]);
            streaming.update(&bytes[split..]);
            cloned.update(&bytes[split..]);
            assert_eq!(
                hex_digest(streaming.finalize()),
                expected,
                "len={len}, split={split}"
            );
            assert_eq!(hex_digest(cloned.finalize()), expected);
        }
    }
}

#[test]
fn streaming_sha256_matches_one_shot_across_large_chunk_boundaries() {
    let bytes = (0..(MAX_EXACT_FINGERPRINT_BYTES as usize + HASH_BUFFER_BYTES + 17))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let expected = crate::cut::sha256_hex(&bytes);
    for chunk_size in [1, 7, 63, 64, 65, HASH_BUFFER_BYTES, HASH_BUFFER_BYTES + 13] {
        let mut streaming = StreamingSha256::new();
        for chunk in bytes.chunks(chunk_size) {
            streaming.update(chunk);
        }
        assert_eq!(
            hex_digest(streaming.finalize()),
            expected,
            "chunk={chunk_size}"
        );
    }
}

#[cfg(unix)]
#[test]
fn streamed_child_drains_large_stdout_and_stderr_exactly() {
    let chunk = "0123456789abcdef".repeat(4);
    let repetitions = 20_000_u64;
    let script = format!(
        "i=0; while [ \"$i\" -lt {repetitions} ]; do printf '%s' '{chunk}'; printf '%s' '{chunk}' >&2; i=$((i + 1)); done"
    );
    let child = Command::new("/bin/sh")
        .args(["-c", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_owned()
        .unwrap();
    let mut observed_stdout = 0_u64;
    let evidence = stream_child_output(
        child,
        |bytes| {
            observed_stdout += bytes.len() as u64;
            true
        },
        Instant::now() + Duration::from_secs(3),
        false,
    )
    .expect("large dual-stream child must complete without a pipe stall");
    let expected = repetitions * chunk.len() as u64;
    assert!(expected > MAX_EXACT_FINGERPRINT_BYTES);
    assert_eq!(evidence.status_code, 0);
    assert_eq!(evidence.stdout.bytes, expected);
    assert_eq!(evidence.stderr.bytes, expected);
    assert_eq!(observed_stdout, expected);
}

#[cfg(unix)]
#[test]
fn streamed_child_deadline_kills_a_pipe_holding_process_group() {
    use std::os::unix::process::CommandExt as _;

    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "sleep 30 & wait"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command.spawn_owned().unwrap();
    let started = Instant::now();
    assert!(
        stream_child_output(
            child,
            |_| true,
            Instant::now() + Duration::from_millis(50),
            true,
        )
        .is_none()
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a wedged Git tree must not retain evaluator evidence forever"
    );
}

#[cfg(unix)]
#[test]
fn streamed_child_deadline_also_bounds_wait_after_early_pipe_close() {
    use std::os::unix::process::CommandExt as _;

    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "exec >/dev/null 2>&1; sleep 30"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command.spawn_owned().unwrap();
    let started = Instant::now();
    assert!(
        stream_child_output(
            child,
            |_| true,
            Instant::now() + Duration::from_millis(50),
            true,
        )
        .is_none()
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "closing pipes early must not bypass the child deadline"
    );
}

#[test]
fn evidence_hashes_middle_only_change_in_oversized_tracked_diff() {
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-large-middle-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);

    // One very long line makes the binary diff exceed the historical 1 MiB
    // cap while keeping its first/last 64 KiB unchanged across the two
    // mutations. The old head/tail sampler therefore collided here.
    let mut contents = vec![b'a'; MAX_EXACT_FINGERPRINT_BYTES as usize + 256 * 1024];
    let middle = contents.len() / 2;
    std::fs::write(root.join("large.txt"), &contents).unwrap();
    git(&root, &["add", "large.txt"]);
    commit(&root, "large baseline");

    contents[middle] = b'b';
    std::fs::write(root.join("large.txt"), &contents).unwrap();
    let first_diff =
        git_stream_output(&root, &["diff", "--binary", "HEAD", "--"], |_| true).unwrap();
    assert!(first_diff.success());
    assert!(
        first_diff.stdout.bytes > MAX_EXACT_FINGERPRINT_BYTES,
        "fixture must exercise oversized Git output: {} bytes",
        first_diff.stdout.bytes
    );
    let first = workspace_evidence_sha256(&root).unwrap();

    contents[middle] = b'c';
    std::fs::write(root.join("large.txt"), &contents).unwrap();
    assert_eq!(
        std::fs::metadata(root.join("large.txt")).unwrap().len(),
        contents.len() as u64
    );
    let second = workspace_evidence_sha256(&root).unwrap();
    assert_ne!(
        first, second,
        "same-length middle-only tracked changes need distinct audit identities"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evidence_streams_middle_only_change_in_large_untracked_file() {
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-large-untracked-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("anchor.txt"), "anchor").unwrap();
    git(&root, &["add", "anchor.txt"]);
    commit(&root, "anchor");

    let mut contents = vec![b'x'; MAX_EXACT_FINGERPRINT_BYTES as usize + HASH_BUFFER_BYTES];
    let middle = contents.len() / 2;
    std::fs::write(root.join("untracked.bin"), &contents).unwrap();
    let first = workspace_evidence_sha256(&root).unwrap();
    contents[middle] = b'y';
    std::fs::write(root.join("untracked.bin"), &contents).unwrap();
    let second = workspace_evidence_sha256(&root).unwrap();
    assert_ne!(
        first, second,
        "large untracked middle bytes must participate in evidence identity"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evidence_fails_closed_for_hidden_tracked_index_flags() {
    let _env_lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-hidden-index-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("tracked.txt"), "baseline").unwrap();
    git(&root, &["add", "tracked.txt"]);
    commit(&root, "hidden-index baseline");
    assert!(workspace_evidence_sha256(&root).is_some());

    git(
        &root,
        &["update-index", "--assume-unchanged", "tracked.txt"],
    );
    std::fs::write(root.join("tracked.txt"), "changed!").unwrap();
    assert_eq!(
        workspace_evidence_sha256(&root),
        None,
        "assume-unchanged bytes must disable exact evidence/cache reuse"
    );

    git(
        &root,
        &["update-index", "--no-assume-unchanged", "tracked.txt"],
    );
    std::fs::write(root.join("tracked.txt"), "baseline").unwrap();
    git(&root, &["update-index", "--skip-worktree", "tracked.txt"]);
    std::fs::write(root.join("tracked.txt"), "changed!").unwrap();
    assert_eq!(
        workspace_evidence_sha256(&root),
        None,
        "skip-worktree bytes must disable exact evidence/cache reuse"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn evidence_hashes_actual_tracked_bytes_hidden_by_clean_filters() {
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-clean-filter-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(root.join(".gitattributes"), "probe filter=hide\n").unwrap();
    std::fs::write(root.join("probe"), "baseline\n").unwrap();
    git(&root, &["config", "filter.hide.clean", "sed s/.*/fixed/"]);
    git(&root, &["add", ".gitattributes", "probe"]);
    commit(&root, "filtered baseline");

    std::fs::write(root.join("probe"), "first dirty bytes\n").unwrap();
    let first = workspace_evidence_sha256(&root).unwrap();
    std::fs::write(root.join("probe"), "other dirty bytes\n").unwrap();
    let second = workspace_evidence_sha256(&root).unwrap();
    assert_ne!(
        first, second,
        "working-tree clean filters must not collapse distinct actual bytes"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evidence_ignores_git_repository_redirection_environment() {
    let _env_lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-env-root-{}",
        std::process::id()
    ));
    let decoy = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-env-decoy-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&decoy);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&decoy).unwrap();
    for (repo, body) in [(&root, "root"), (&decoy, "decoy")] {
        git(repo, &["init", "-q"]);
        std::fs::write(repo.join("tracked.txt"), body).unwrap();
        git(repo, &["add", "tracked.txt"]);
        commit(repo, body);
    }
    std::fs::write(root.join("fixture-only.txt"), "root only").unwrap();
    git(&root, &["add", "fixture-only.txt"]);
    commit(&root, "fixture-only");
    let expected = workspace_evidence_sha256(&root).unwrap();

    let _git_dir = crate::tests::TestEnvGuard::set("GIT_DIR", decoy.join(".git").to_str().unwrap());
    let _git_work_tree = crate::tests::TestEnvGuard::set("GIT_WORK_TREE", decoy.to_str().unwrap());
    let _git_index = crate::tests::TestEnvGuard::set(
        "GIT_INDEX_FILE",
        decoy.join(".git/index").to_str().unwrap(),
    );
    git(&root, &["add", "fixture-only.txt"]);
    assert_eq!(workspace_evidence_sha256(&root), Some(expected));

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(decoy);
}

#[cfg(unix)]
#[test]
fn evidence_distinguishes_non_utf8_symlink_targets() {
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-non-utf8-symlink-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("anchor.txt"), "anchor").unwrap();
    git(&root, &["add", "anchor.txt"]);
    commit(&root, "anchor");

    let link = root.join("untracked-link");
    let first_target = vec![b't', b'a', b'r', b'g', b'e', b't', b'-', 0x80];
    let second_target = vec![b't', b'a', b'r', b'g', b'e', b't', b'-', 0x81];
    assert_eq!(
        String::from_utf8_lossy(&first_target),
        String::from_utf8_lossy(&second_target),
        "fixture must collide under the former lossy encoding"
    );

    symlink(
        PathBuf::from(std::ffi::OsString::from_vec(first_target)),
        &link,
    )
    .unwrap();
    let first = workspace_evidence_sha256(&root).unwrap();
    std::fs::remove_file(&link).unwrap();
    symlink(
        PathBuf::from(std::ffi::OsString::from_vec(second_target)),
        &link,
    )
    .unwrap();
    let second = workspace_evidence_sha256(&root).unwrap();
    assert_ne!(
        first, second,
        "distinct raw symlink-target bytes need distinct audit identities"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evidence_streams_status_larger_than_memory_chunk() {
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-evidence-large-status-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("anchor.txt"), "anchor").unwrap();
    git(&root, &["add", "anchor.txt"]);
    commit(&root, "anchor");

    let suffix = "s".repeat(205);
    for index in 0..5_000 {
        std::fs::write(root.join(format!("{index:05}-{suffix}.txt")), b"x").unwrap();
    }
    let status = git_stream_output(
        &root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        |_| true,
    )
    .unwrap();
    assert!(status.success());
    assert!(
        status.stdout.bytes > MAX_EXACT_FINGERPRINT_BYTES,
        "fixture produced only {} status bytes",
        status.stdout.bytes
    );
    assert!(workspace_evidence_sha256(&root).is_some());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evaluator_execution_pinned_git_ignores_hostile_path() {
    let root =
        std::env::temp_dir().join(format!("angel-pinned-git-control-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("workspace");
    let hostile = root.join("hostile");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&hostile).unwrap();
    git(&workspace, &["init", "-q"]);
    std::fs::write(workspace.join("tracked.txt"), "fixture").unwrap();
    git(&workspace, &["add", "tracked.txt"]);
    let expected = workspace_evidence_sha256(&workspace).unwrap();

    let fake_git = hostile.join("git");
    std::fs::write(&fake_git, "#!/bin/sh\nprintf forged\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&fake_git).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_git, permissions).unwrap();
    }
    let hostile_path = format!("{}:/bin:/usr/bin", hostile.display());
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "harness::workspace_state::tests::evaluator_execution_pinned_git_child",
            "--nocapture",
        ])
        .env("PATH", &hostile_path)
        .env("ANGEL_PINNED_GIT_TEST_WORKSPACE", &workspace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!("PINNED_GIT_DIGEST={expected}")),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn broad_home_workspace_skips_optional_git_evidence() {
    let root = std::env::temp_dir().join(format!("angel-home-workspace-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("project")).unwrap();

    assert!(workspace_is_home(&root, Some(root.as_os_str())));
    assert!(!workspace_is_home(
        &root.join("project"),
        Some(root.as_os_str())
    ));
    assert!(!workspace_is_home(&root, None));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn evaluator_execution_pinned_git_child() {
    let Some(workspace) = std::env::var_os("ANGEL_PINNED_GIT_TEST_WORKSPACE") else {
        return;
    };
    let digest = workspace_evidence_sha256(Path::new(&workspace)).unwrap();
    println!("PINNED_GIT_DIGEST={digest}");
}

#[test]
fn fingerprint_tracks_real_changes_and_ignores_reverts() {
    let _env_lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-fingerprint-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("tracked.txt"), "base").unwrap();
    git(&root, &["add", "tracked.txt"]);
    git(
        &root,
        &[
            "-c",
            "user.name=Angel Test",
            "-c",
            "user.email=angel@example.invalid",
            "commit",
            "-qm",
            "base",
        ],
    );

    let baseline = workspace_fingerprint(&root).unwrap();
    let evidence_baseline = workspace_evidence_sha256(&root).unwrap();
    assert_eq!(workspace_fingerprint(&root), Some(baseline));
    assert_eq!(
        workspace_evidence_sha256(&root),
        Some(evidence_baseline.clone())
    );
    std::fs::write(root.join("tracked.txt"), "changed").unwrap();
    assert_ne!(workspace_fingerprint(&root), Some(baseline));
    assert_ne!(
        workspace_evidence_sha256(&root),
        Some(evidence_baseline.clone())
    );
    std::fs::write(root.join("tracked.txt"), "base").unwrap();
    assert_eq!(workspace_fingerprint(&root), Some(baseline));
    assert_eq!(
        workspace_evidence_sha256(&root),
        Some(evidence_baseline.clone())
    );

    std::fs::write(root.join("new.txt"), "one").unwrap();
    let untracked = workspace_fingerprint(&root).unwrap();
    assert_ne!(untracked, baseline);
    std::fs::write(root.join("new.txt"), "two").unwrap();
    assert_ne!(workspace_fingerprint(&root), Some(untracked));

    std::fs::remove_file(root.join("new.txt")).unwrap();
    std::fs::write(root.join("tracked.txt"), "next revision").unwrap();
    git(&root, &["add", "tracked.txt"]);
    git(
        &root,
        &[
            "-c",
            "user.name=Angel Test",
            "-c",
            "user.email=angel@example.invalid",
            "commit",
            "-qm",
            "next",
        ],
    );
    assert_ne!(
        workspace_evidence_sha256(&root),
        Some(evidence_baseline),
        "clean revisions must have distinct evaluator evidence identities"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn concurrent_fingerprint_equals_sequential_recomputation() {
    let _env_lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-fingerprint-equiv-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(root.join("tracked.txt"), "committed").unwrap();
    git(&root, &["add", "tracked.txt"]);
    commit(&root, "base");
    std::fs::write(root.join("staged.txt"), "staged").unwrap();
    git(&root, &["add", "staged.txt"]);
    std::fs::write(root.join("tracked.txt"), "modified after commit").unwrap();
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("untracked.txt"), "untracked").unwrap();
    std::fs::write(root.join("nested").join("deep.txt"), "deep untracked").unwrap();

    // Expected value: the same four probes, run strictly one after another
    // through git_output, hashed by the same helper the live path uses.
    let deadline = Instant::now() + Duration::from_secs(30);
    let [
        status_args,
        tracked_args,
        tracked_paths_args,
        untracked_args,
    ] = fingerprint_probe_args();
    let status = git_output(&root, &status_args, deadline).unwrap();
    let tracked = git_output(&root, &tracked_args, deadline).unwrap();
    let tracked_paths = git_output(&root, &tracked_paths_args, deadline).unwrap();
    let untracked = git_output(&root, &untracked_args, deadline).unwrap();
    let expected = hash_fingerprint_probes(&root, &status, &tracked, &tracked_paths, &untracked);

    assert_eq!(workspace_fingerprint(&root), Some(expected));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fingerprint_is_absent_outside_git() {
    let root = std::env::temp_dir().join(format!(
        "angel-workspace-fingerprint-nongit-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(workspace_fingerprint(&root), None);
    assert_eq!(workspace_evidence_sha256(&root), None);
    let _ = std::fs::remove_dir_all(root);
}
