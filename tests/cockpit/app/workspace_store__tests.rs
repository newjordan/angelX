#[cfg(unix)]
#[test]
fn r04e_contended_store_lock_waits_and_never_drops_the_write() {
    use std::os::fd::AsRawFd;
    let _guard = crate::tests::env_lock();
    let path = std::env::temp_dir().join(format!("r04e-lock-{}", std::process::id()));
    let held = std::fs::File::create(&path).unwrap();
    let contender = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    super::lock_store(&held, "r04e_test_lock").unwrap();
    let held_fd = held.as_raw_fd();
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        // SAFETY: the descriptor stays open in `held` until this thread has released it.
        assert_eq!(unsafe { libc::flock(held_fd, libc::LOCK_UN) }, 0);
    });
    let started = std::time::Instant::now();
    // The contender waits through one contention receipt and then acquires;
    // the write is never dropped.
    super::lock_store(&contender, "r04e_test_lock").unwrap();
    assert!(started.elapsed() >= std::time::Duration::from_millis(2000));
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    releaser.join().unwrap();
    drop(contender);
    drop(held);
    std::fs::remove_file(path).unwrap();
}

use super::*;

#[test]
fn stdout_drain_retains_a_bounded_prefix_and_marks_overflow() {
    let input = vec![b'x'; 128 * 1024];
    let captured = drain_stdout(std::io::Cursor::new(input), 32 * 1024)
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(captured.bytes.len(), 32 * 1024);
    assert!(captured.bytes.iter().all(|byte| *byte == b'x'));
    assert!(captured.overflow);
}

#[cfg(unix)]
#[test]
fn command_capture_enforces_its_own_deadline_and_kills_descendants() {
    let started = Instant::now();
    let captured = capture_with_deadline(
        "sh",
        &["-c", "sleep 30"],
        Path::new("."),
        Duration::from_millis(50),
        1024,
    )
    .unwrap();
    assert!(captured.is_none(), "deadline should report a timeout");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "deadline cleanup must not wait for the child tree"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn capture_exit_notification_observes_real_child_and_preserves_exit_status() {
    let mut child = Command::new("sh")
        .args(["-c", "sleep 0.02; exit 7"])
        .spawn_owned()
        .unwrap();
    let wake = CaptureExitWake::new(child.id());
    if wake.fd.is_none() {
        child.wait().unwrap();
        eprintln!("pidfd unavailable; bounded polling fallback remains active");
        return;
    }
    wake.wait(Duration::from_secs(2));
    assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(7));
    CaptureExitWake { fd: None }.wait(Duration::ZERO);
}

#[cfg(unix)]
#[test]
fn command_capture_reaps_successful_parents_background_pipe_holders() {
    let started = Instant::now();
    let (status, captured) = capture_with_deadline(
        "sh",
        &["-c", "sleep 30 & printf okay"],
        Path::new("."),
        Duration::from_secs(1),
        1024,
    )
    .unwrap()
    .expect("the direct child should finish before its deadline");
    assert!(status.success());
    assert_eq!(captured.bytes, b"okay");
    assert!(!captured.overflow);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a grandchild holding stdout must not stall reader join"
    );
}

#[test]
fn workspace_key_is_deterministic_and_distinct() {
    let a = workspace_key(Path::new("/home/u/alpha"));
    let a2 = workspace_key(Path::new("/home/u/alpha"));
    let b = workspace_key(Path::new("/home/u/beta"));
    assert_eq!(a, a2, "same path -> same stable file key");
    assert_ne!(a, b, "different paths get distinct keys");
    assert!(!a.contains('/'), "key is filesystem-safe");
}

#[test]
fn workspace_json_path_uses_key_under_namespace_dir() {
    let dir = Path::new("/tmp/angel/loops");
    let path = workspace_json_path_in(dir, Path::new("/home/u/alpha"));
    assert_eq!(path.parent(), Some(dir));
    assert_eq!(path.extension().and_then(|e| e.to_str()), Some("json"));
}

#[test]
fn private_atomic_write_publishes_complete_replacement() {
    static TEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = TEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "angel-private-atomic-{}-{serial}",
        std::process::id()
    ));
    let path = dir.join("state.json");
    write_private_atomic(&path, b"first").unwrap();
    write_private_atomic(&path, b"second").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"second");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn project_match_requires_both_canonical_root_and_key() {
    let workspace = Path::new("/home/u/alpha");
    let identity = repo_identity(workspace);
    assert!(matches_project(workspace, &identity.root, &identity.key));
    assert!(!matches_project(
        workspace,
        Path::new("/home/u/beta"),
        &identity.key
    ));
    assert!(!matches_project(workspace, &identity.root, "wrong-key"));
}

// --- repo identity: the main worktree is the repository. ---

#[test]
fn main_worktree_is_the_first_porcelain_entry() {
    // `git worktree list --porcelain` from ANY worktree prints the main one
    // first — that is what makes it the canonical identity.
    let out = "worktree /home/u/repo\nHEAD abc123\nbranch refs/heads/main\n\n\
               worktree /tmp/cut-forge/item-3\nHEAD abc123\nbranch refs/heads/item-3\n";
    assert_eq!(
        main_worktree_from_porcelain(out),
        Some(PathBuf::from("/home/u/repo"))
    );
}

#[test]
fn main_worktree_handles_bare_and_detached_and_garbage() {
    // Bare main repo: the first entry is the bare dir (+ a `bare` line).
    assert_eq!(
        main_worktree_from_porcelain("worktree /srv/git/repo.git\nbare\n"),
        Some(PathBuf::from("/srv/git/repo.git"))
    );
    // Detached worktree: `detached` instead of `branch` — still an entry.
    assert_eq!(
        main_worktree_from_porcelain("worktree /home/u/repo\nHEAD abc\ndetached\n"),
        Some(PathBuf::from("/home/u/repo"))
    );
    // Never panics on nonsense / empty output.
    assert_eq!(main_worktree_from_porcelain(""), None);
    assert_eq!(main_worktree_from_porcelain("HEAD abc\n"), None);
    assert_eq!(main_worktree_from_porcelain("worktree \n"), None);
}

#[test]
fn a_non_repo_path_keeps_its_own_identity() {
    // Not a git repo → the workspace IS the identity (the legacy behavior),
    // and nothing crashes.
    let ws = std::env::temp_dir().join(format!("angel-ws-store-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&ws);
    let id = repo_identity(&ws);
    assert_eq!(id.root, ws, "no git → fall back to the workspace path");
    assert_eq!(id.key, workspace_key(&ws));
    assert_eq!(id.slug, None);
    let _ = std::fs::remove_dir_all(&ws);
}

/// `git worktree add` a real linked worktree of a throwaway repo and prove it
/// inherits the main checkout's identity — the whole point of this module.
/// Uses a fresh temp repo, so the source tree's `.git` is never touched.
#[test]
fn a_linked_worktree_inherits_the_main_repos_identity() {
    let _guard = crate::tests::env_lock();
    let base = std::env::temp_dir().join(format!("angel-wt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let repo = base.join("main");
    let linked = base.join("linked");
    std::fs::create_dir_all(&repo).unwrap();

    let git = |args: &[&str], cwd: &Path| {
        Command::new("git")
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    };
    // A repo needs one commit before `worktree add` will work.
    let setup = git(&["init", "-q"], &repo)
        .map(|s| s.success())
        .unwrap_or(false)
        && git(&["config", "user.email", "t@t"], &repo).is_ok()
        && git(&["config", "user.name", "t"], &repo).is_ok()
        && std::fs::write(repo.join("f.txt"), "x").is_ok()
        && git(&["add", "-A"], &repo).is_ok()
        && git(&["commit", "-qm", "c"], &repo)
            .map(|s| s.success())
            .unwrap_or(false)
        && git(
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "angel-linked-test",
                linked.to_str().unwrap(),
                "HEAD",
            ],
            &repo,
        )
        .map(|s| s.success())
        .unwrap_or(false);
    if !setup {
        let _ = std::fs::remove_dir_all(&base); // no git available — skip
        return;
    }

    // The linked worktree — a different directory, and a DIFFERENT
    // workspace_key — resolves to the main checkout, so its evidence lands
    // in the main repo's dossier instead of a key that dies with it.
    assert_ne!(workspace_key(&linked), workspace_key(&repo));
    let id = repo_identity(&linked);
    assert_eq!(id.root.canonicalize().ok(), repo.canonicalize().ok());
    assert_eq!(id.key, repo_identity(&repo).key);
    // …and a subdirectory of the worktree resolves there too.
    let sub = linked.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    assert_eq!(repo_identity(&sub).key, id.key);

    // Durable knowledge identity is deliberately repository-wide, but live
    // workspace/Git state must remain checkout-specific. This is the
    // branch-cache boundary: consulting any process-local identity or
    // confinement cache must never retarget one checkout to another.
    let main_branch = git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &repo, 5).unwrap();
    assert_eq!(
        git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &linked, 5).as_deref(),
        Some("angel-linked-test")
    );
    let main_boundary = crate::harness::WorkspaceBoundary::cached(&repo);
    let linked_boundary = crate::harness::WorkspaceBoundary::cached(&linked);
    assert_eq!(main_boundary.repository.key, linked_boundary.repository.key);
    assert_ne!(
        main_boundary.canonical_root, linked_boundary.canonical_root,
        "linked worktrees must retain distinct live filesystem boundaries"
    );
    std::fs::write(repo.join("f.txt"), "main dirty").unwrap();
    std::fs::write(linked.join("f.txt"), "linked dirty").unwrap();
    assert_ne!(
        crate::harness::workspace_fingerprint(&repo),
        crate::harness::workspace_fingerprint(&linked),
        "branch-local dirty state must never share a cached fingerprint"
    );
    assert_eq!(
        git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &repo, 5).as_deref(),
        Some(main_branch.as_str())
    );
    assert_eq!(
        git_capture(&["rev-parse", "--abbrev-ref", "HEAD"], &linked, 5).as_deref(),
        Some("angel-linked-test")
    );

    let _ = git(
        &["worktree", "remove", "--force", linked.to_str().unwrap()],
        &repo,
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn identity_can_be_switched_off() {
    // The escape hatch restores pure path-keying (no git spawn at all).
    let _guard = crate::tests::env_lock();
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_REPO_IDENTITY", "0") };
    let root = canonical_repo_root(&crate_dir);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_REPO_IDENTITY") };
    assert_eq!(root, crate_dir, "off → the workspace is its own repo again");
}
