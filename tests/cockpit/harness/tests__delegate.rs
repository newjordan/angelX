//! Delegate modes, worktree integration, and spark/orchestrator handoffs.
//!
//! Extracted from `harness/tests.rs` without behavior change so the monolith can
//! shrink while preserving the full harness test inventory.

use super::*;

// --- delegate / orchestrator suite ---

#[test]
fn worktree_git_metadata_becomes_writable_for_confined_shells() {
    use crate::agent::sandbox::SandboxPolicy;

    let root = scratch("wt_git_roots");
    // A linked-worktree layout: main repo .git with worktrees/<name>/commondir
    // pointing back at the common dir, and the worktree's .git gitfile.
    let common = root.join("repo/.git");
    let gitdir = common.join("worktrees/wt");
    std::fs::create_dir_all(&gitdir).unwrap();
    std::fs::write(gitdir.join("commondir"), "../..\n").unwrap();
    let wt = root.join("wt");
    std::fs::create_dir_all(&wt).unwrap();
    std::fs::write(wt.join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();

    let roots = SandboxPolicy::git_worktree_roots(&wt);
    let common_canon = common.canonicalize().unwrap();
    assert!(
        roots.contains(&common_canon),
        "the common .git (object store, refs) must be writable: {roots:?}"
    );
    assert!(
        roots.contains(&gitdir),
        "the per-worktree gitdir (index, HEAD) must be writable: {roots:?}"
    );

    // A regular repository keeps its .git inside the workspace: nothing extra.
    let plain = root.join("plain");
    std::fs::create_dir_all(plain.join(".git")).unwrap();
    assert!(SandboxPolicy::git_worktree_roots(&plain).is_empty());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn orchestrator_prompt_is_the_entry_warpath_and_hygiene_is_in_the_ledger() {
    let ws = std::env::temp_dir().join(format!("angel_orch_sign_{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    let prompt = orchestrator_system_prompt(&ws, &["coder".to_string()]);
    assert_eq!(prompt, "⠽⠃", "the Driver with teammates, and no words");
    let ledger = |cells: &str| crate::agent::harness::book::ledger::read(&ws, cells).unwrap();
    assert!(
        ledger("⠕⠃").contains("- coder:"),
        "configured routes are ledger evidence"
    );
    // The type renders the whole prompt it replaces, verbatim.
    let driver = ledger("⠽⠃");
    for taught in [
        "structured tool-call interface",
        "Do not print raw",
        "<server>__mcp",
        "sandbox",
        "approval",
        "Batch independent tool calls",
        "`code_mode` batch",
        "Do not end a turn with a promise",
        "Local seats",
        "optional",
        "ask the user what they would like to do",
        "dedicated repository tools when available",
    ] {
        assert!(driver.contains(taught), "{taught}:\n{driver}");
    }
    // Naming the forbidden artifacts hands a degraded model a token to chase.
    let lower = driver.to_ascii_lowercase();
    for banned in ["gauntlet", "health checkup", "repair mode"] {
        assert!(!lower.contains(banned), "must not name '{banned}'");
    }
    let _ = std::fs::remove_dir_all(&ws);
}

/// Both delegate seats carry the shared batching advisory, and the seat
/// contract names no tool the worktree registry does not register.
#[test]
fn delegate_prompts_teach_batching_without_naming_unregistered_tools() {
    let _lock = crate::tests::env_lock();
    for mode in [DelegateMode::ReadOnly, DelegateMode::Write] {
        let cells = delegate_system_prompt(mode);
        assert!(
            cells.ends_with("⠺⠉⠁"),
            "delegate {mode:?} must carry the shared batching page:\n{cells}"
        );
        // The seat reads its words off the ledger (its registry carries
        // `read_file`); what they name must exist there.
        let prompt = super::book::ledger::read(Path::new("."), &cells).unwrap();
        assert!(prompt.contains("Batch independent tool calls"), "{prompt}");
        for unavailable in ["proc_run", "proc_status", "code_mode", "apply_patch"] {
            assert!(
                !prompt.contains(unavailable),
                "delegate {mode:?} cannot call {unavailable}:\n{prompt}"
            );
        }
    }
    let write =
        super::book::ledger::read(Path::new("."), &delegate_system_prompt(DelegateMode::Write))
            .unwrap();
    assert!(
        write.contains("Keep long-running commands observable"),
        "the observable-command guidance stays for the tools it has:\n{write}"
    );
}

// A scripted club: first chat → call `reverse`, second chat → final text.

/// Scenario: git_status / git_log parse LIVE git output on the real repo
/// (not a synthetic fixture) without erroring, surfacing the branch + history.

#[test]
fn delegate_mode_accepts_review_aliases() {
    assert_eq!(DelegateMode::parse(None).unwrap(), DelegateMode::Write);
    assert_eq!(
        DelegateMode::parse(Some(" REVIEW ")).unwrap(),
        DelegateMode::ReadOnly
    );
    assert_eq!(
        DelegateMode::parse(Some("readonly")).unwrap(),
        DelegateMode::ReadOnly
    );
    assert!(DelegateMode::parse(Some("delete_everything")).is_err());
}

#[test]
fn delegate_schema_exposes_read_only_mode() {
    let ws = std::env::temp_dir().join(format!("angel_delegate_schema_{}", std::process::id()));
    let delegate = DelegateTool::new(ws, Vec::new());
    let def = delegate.def();
    let modes = def.params["properties"]["mode"]["enum"]
        .as_array()
        .expect("mode enum");
    assert!(modes.iter().any(|v| v == "write"));
    assert!(modes.iter().any(|v| v == "review"));
    assert!(modes.iter().any(|v| v == "read_only"));
}

#[test]
fn delegate_worktree_base_is_scoped_per_workspace() {
    let parent = std::env::temp_dir();
    let a = DelegateTool::new(parent.join("angel_ws_a"), Vec::new()).worktree_base();
    let b = DelegateTool::new(parent.join("angel_ws_b"), Vec::new()).worktree_base();
    assert_ne!(a, b);
    assert!(
        a.parent().is_some_and(|p| p.ends_with(".angel-worktrees")),
        "worktrees stay grouped under .angel-worktrees; got {}",
        a.display()
    );
}

/// A scripted specialist that writes one file via the `shell` tool.
struct ScribeClub {
    name: String,
    file: String,
    content: String,
    hops: AtomicUsize,
}
impl ScribeClub {
    fn new(name: &str, file: &str, content: &str) -> Self {
        Self {
            name: name.into(),
            file: file.into(),
            content: content.into(),
            hops: AtomicUsize::new(0),
        }
    }
}
impl Club for ScribeClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Ok("unused".into())
    }
    fn label(&self) -> &str {
        &self.name
    }
    fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
        if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "w".into(),
                name: "shell".into(),
                args: serde_json::json!({
                    "command": format!("printf '%s' '{}' > {}", self.content, self.file)
                }),
            }]))
        } else {
            Ok(ClubReply::Text(format!("wrote {}", self.file)))
        }
    }
}

struct FailingDelegateClub;
impl Club for FailingDelegateClub {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Err("HTTP 401: synthetic delegate failure".to_string())
    }
    fn label(&self) -> &str {
        "failing"
    }
    fn chat(&self, _m: &[ChatMsg], _t: &[ToolDef]) -> Result<ClubReply, String> {
        // Cleanup after an actionable failure, not an ongoing recoverable
        // outage: routine outages now wait for recovery or cancellation.
        Err("HTTP 401: synthetic delegate failure".to_string())
    }
}

struct InterruptedScribe {
    scribe: ScribeClub,
    cancel: Arc<AtomicBool>,
}
impl Club for InterruptedScribe {
    fn label(&self) -> &str {
        "interrupted-scribe"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("unused".into())
    }
    fn chat(&self, history: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.scribe.hops.load(Ordering::SeqCst) == 0 {
            self.scribe.chat(history, tools)
        } else {
            self.cancel.store(true, Ordering::Release);
            Err("operator interrupted after write".into())
        }
    }
}

struct DarkTurbo;
impl Club for DarkTurbo {
    fn respond(&self, _p: &str) -> Result<String, String> {
        Err("turbo model not available".to_string())
    }
    fn label(&self) -> &str {
        "turbo"
    }
    fn is_available(&self) -> bool {
        false
    }
}

#[test]
fn delegate_skips_a_down_local_instead_of_failing() {
    let ws = std::env::temp_dir().join(format!(
        "angel_delegate_dark_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let tool = DelegateTool::new(ws.clone(), vec![Arc::new(DarkTurbo)]);
    let text = tool
        .call(&serde_json::json!({
            "club": "turbo",
            "task": "write hello"
        }))
        .unwrap();
    assert!(text.contains("delegate skipped"), "{text}");
    assert!(text.contains("turbo is not reachable"), "{text}");
    assert!(
        text.contains(&crate::agent::harness::book::o_orchestration::SEAT_SKIPPED.cells()),
        "{text}"
    );
    assert!(
        !text.contains("turbo model not available"),
        "must not chat a down local: {text}"
    );
    let _ = std::fs::remove_dir_all(&ws);
}

/// LIVE end-to-end: drive the real tool-loop against spark (skips cleanly
/// when the fleet is down, so the suite stays green offline). Proves
/// HttpClub.chat tool-calling + run_turn dispatch against a real model.
#[test]
fn live_spark_drives_the_tool_loop() {
    let _guard = crate::tests::env_lock();
    use crate::agent::club::HttpClub;
    let Ok(url) = std::env::var("ANGEL_SPARK_URL") else {
        eprintln!("set ANGEL_SPARK_URL to run the live tool-loop test; skipping");
        return;
    };
    let model = std::env::var("ANGEL_SPARK_MODEL").unwrap_or_else(|_| "qwopus-coder".to_string());
    let club = HttpClub::new("spark", url, model, None);
    if !club.is_ready() {
        eprintln!("spark not reachable; skipping live tool-loop test");
        return;
    }
    let reg = ToolRegistry::with_defaults(); // includes `reverse`
    let mut history = vec![
        ChatMsg::system("You have tools; use them when asked."),
        ChatMsg::user("Use the reverse tool on the text 'hello', then report the result."),
    ];
    let answer = run_turn(
        &club,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(6),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("live turn should complete");
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::Tool && m.content.as_ref() == "olleh"),
        "expected the reverse tool to have run (result 'olleh'); history: {:?}",
        history
            .iter()
            .map(|m| (&m.role, &m.content))
            .collect::<Vec<_>>()
    );
    assert!(
        !answer.trim().is_empty(),
        "final answer should be non-empty"
    );
}

/// LIVE multi-agent: spark (Driver) delegates to turbo over the shared
/// workspace. Heavy (two real models + worktrees) — opt-in via
/// ANGEL_LIVE_ORCH=1, and needs ANGEL_BRAIN_KEY for turbo.
#[test]
fn live_spark_orchestrates_turbo() {
    let _guard = crate::tests::env_lock();
    use crate::agent::club::HttpClub;
    if std::env::var("ANGEL_LIVE_ORCH").is_err() {
        eprintln!("set ANGEL_LIVE_ORCH=1 (+ ANGEL_BRAIN_KEY) to run; skipping");
        return;
    }
    let spark_url = std::env::var("ANGEL_SPARK_URL")
        .expect("ANGEL_LIVE_ORCH requires an explicit ANGEL_SPARK_URL");
    let spark = HttpClub::new("spark", spark_url, "qwopus-coder", None);
    if !spark.is_ready() {
        eprintln!("spark down; skipping");
        return;
    }
    let turbo_url = std::env::var("ANGEL_TURBO_URL")
        .expect("ANGEL_LIVE_ORCH requires an explicit ANGEL_TURBO_URL");
    let turbo: Arc<dyn Club> = Arc::new(HttpClub::new(
        "turbo",
        turbo_url,
        std::env::var("ANGEL_TURBO_MODEL").unwrap_or_default(),
        std::env::var("ANGEL_BRAIN_KEY").ok(),
    ));

    let ws = std::env::temp_dir().join(format!("angel_orch_live_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    let reg = ToolRegistry::with_team(ws.clone(), vec![turbo]);

    let mut history = vec![
        ChatMsg::system(orchestrator_system_prompt(&ws, &["turbo".to_string()])),
        ChatMsg::user(
            "Delegate to the turbo specialist: have turbo create a file named hello.txt whose \
                 contents are exactly the word 'angel'. Then integrate the resulting branch.",
        ),
    ];
    let answer = run_turn(
        &spark,
        &reg,
        &mut history,
        &AtomicBool::new(false),
        Some(16),
        &mpsc::channel::<TurnEvent>().0,
    )
    .expect("orchestration turn");
    eprintln!("orchestration answer: {answer}");
    eprintln!("hello.txt integrated: {}", ws.join("hello.txt").exists());

    // Core claim: spark actually delegated to turbo (the delegate tool result
    // embeds "branch=angel/turbo").
    assert!(
        history
            .iter()
            .any(|m| m.role == ChatRole::Tool && m.content.contains("branch=angel/turbo")),
        "expected spark to delegate to turbo"
    );

    let _ = std::fs::remove_dir_all(ws.parent().unwrap_or(&ws).join(".angel-worktrees"));
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn orchestrator_delegates_in_worktrees_and_integrates_serially() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping orchestrator test");
        return;
    }
    let ws = std::env::temp_dir().join(format!("angel_orch_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);

    let roster: Vec<Arc<dyn Club>> = vec![
        Arc::new(ScribeClub::new("scribe1", "note.txt", "hello-1")),
        Arc::new(ScribeClub::new("scribe2", "note2.txt", "hello-2")),
    ];
    let delegate = DelegateTool::new(ws.clone(), roster);
    let integrate = IntegrateTool::new(ws.clone());

    // First specialist works in its worktree; the diff is captured, but the
    // shared workspace is untouched until we integrate.
    let o1 = delegate
        .run("scribe1", "write note 1", DelegateMode::Write)
        .unwrap();
    assert!(
        o1.diff.contains("note.txt"),
        "delegate diff should add note.txt; got: {}",
        o1.diff
    );
    assert!(
        !ws.join("note.txt").exists(),
        "shared workspace must be untouched before integrate"
    );

    integrate.run(&o1.branch).unwrap();
    assert!(
        ws.join("note.txt").exists(),
        "note.txt present after integrate"
    );

    // Second specialist, serialized through the same workspace.
    let o2 = delegate
        .run("scribe2", "write note 2", DelegateMode::Write)
        .unwrap();
    integrate.run(&o2.branch).unwrap();
    assert!(
        ws.join("note.txt").exists() && ws.join("note2.txt").exists(),
        "both notes present after serialized integration"
    );

    // Unknown club is a clean error.
    assert!(delegate.run("ghost", "x", DelegateMode::Write).is_err());

    let _ = std::fs::remove_dir_all(delegate.worktree_base());
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn concurrent_integrations_are_serialized_and_both_land() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping concurrent integration test");
        return;
    }
    let ws = std::env::temp_dir().join(format!("angel_integrate_race_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    let roster: Vec<Arc<dyn Club>> = vec![
        Arc::new(ScribeClub::new("left", "left.txt", "left")),
        Arc::new(ScribeClub::new("right", "right.txt", "right")),
    ];
    let delegate = DelegateTool::new(ws.clone(), roster);
    let left = delegate
        .run("left", "write left", DelegateMode::Write)
        .unwrap();
    let right = delegate
        .run("right", "write right", DelegateMode::Write)
        .unwrap();

    let barrier = Arc::new(std::sync::Barrier::new(3));
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for branch in [left.branch, right.branch] {
            let workspace = ws.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(scope.spawn(move || {
                barrier.wait();
                IntegrateTool::new(workspace).run(&branch)
            }));
        }
        barrier.wait();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(
        results.iter().all(Result::is_ok),
        "serialized merges should both succeed: {results:?}"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join("left.txt")).unwrap(),
        "left"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join("right.txt")).unwrap(),
        "right"
    );

    let _ = std::fs::remove_dir_all(delegate.worktree_base());
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn integrate_lock_waits_observe_cancellation_without_releasing_the_held_lock() {
    let _guard = crate::tests::env_lock();
    let process_guard = acquire_integrate_process_lock_for_test(None).unwrap();
    let process_cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_cancel = Arc::clone(&process_cancel);
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (process_result_tx, process_result_rx) = std::sync::mpsc::channel();
    let process_waiter = std::thread::spawn(move || {
        let _ = started_tx.send(());
        let result = acquire_integrate_process_lock_for_test(Some(&worker_cancel));
        let _ = process_result_tx.send(result.err());
    });
    started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    process_cancel.store(true, std::sync::atomic::Ordering::Release);
    let process_error = match process_result_rx.recv_timeout(Duration::from_secs(1)) {
        Ok(error) => error,
        Err(_) => {
            drop(process_guard);
            process_result_rx.recv_timeout(Duration::from_secs(1)).unwrap()
        }
    };
    let process_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !process_waiter.is_finished() && std::time::Instant::now() < process_deadline {
        std::thread::yield_now();
    }
    let process_finished = process_waiter.is_finished();
    if process_finished {
        process_waiter.join().unwrap();
    } else {
        drop(process_waiter);
    }
    assert!(process_finished, "process lock waiter must settle");
    assert!(
        process_error.as_ref().is_some_and(|error| error.contains("integrate cancelled")),
        "{process_error:?}"
    );

    let ws = std::env::temp_dir().join(format!(
        "angel_integrate_cancel_flock_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&ws);
    let repo = ensure_git_workspace(&ws).unwrap();
    let repo_guard = acquire_repo_integrate_lock_for_test(&repo, Duration::from_secs(2)).unwrap();
    let repo_cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_cancel = Arc::clone(&repo_cancel);
    let repo_path = repo.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (repo_result_tx, repo_result_rx) = std::sync::mpsc::channel();
    let repo_waiter = std::thread::spawn(move || {
        let _ = started_tx.send(());
        let result = acquire_repo_integrate_lock_for_test_with_cancel(
            &repo_path,
            None,
            Some(&worker_cancel),
        );
        let _ = repo_result_tx.send(result.err());
    });
    started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    repo_cancel.store(true, std::sync::atomic::Ordering::Release);
    let repo_error = match repo_result_rx.recv_timeout(Duration::from_secs(1)) {
        Ok(error) => error,
        Err(_) => {
            drop(repo_guard);
            repo_result_rx.recv_timeout(Duration::from_secs(1)).unwrap()
        }
    };
    let repo_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !repo_waiter.is_finished() && std::time::Instant::now() < repo_deadline {
        std::thread::yield_now();
    }
    let repo_finished = repo_waiter.is_finished();
    if repo_finished {
        repo_waiter.join().unwrap();
    } else {
        drop(repo_waiter);
    }
    assert!(repo_finished, "repository lock waiter must settle");
    assert!(
        repo_error.as_ref().is_some_and(|error| error.contains("integrate cancelled")),
        "{repo_error:?}"
    );
    let _ = std::fs::remove_dir_all(ws);
}

#[test]
fn repo_integration_flock_blocks_a_second_file_description() {
    let ws = std::env::temp_dir().join(format!("angel_integrate_flock_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    let repo = ensure_git_workspace(&ws).unwrap();
    let first = acquire_repo_integrate_lock_for_test(&repo, Duration::from_millis(100)).unwrap();
    let started = Instant::now();
    let err = acquire_repo_integrate_lock_for_test(&repo, Duration::from_millis(60))
        .err()
        .expect("a separately opened descriptor must contend on flock");
    assert!(
        err.contains("timed out waiting for integration lock"),
        "{err}"
    );
    assert!(started.elapsed() < Duration::from_millis(500));

    drop(first);
    let reacquired =
        acquire_repo_integrate_lock_for_test(&repo, Duration::from_millis(100)).unwrap();
    drop(reacquired);
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn failed_delegate_cleans_worktree_and_drops_partial_branch() {
    let ws = std::env::temp_dir().join(format!("angel_delegate_fail_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(FailingDelegateClub)];
    let delegate = DelegateTool::new(ws.clone(), roster);
    let base = delegate.worktree_base();

    let err = delegate
        .run("FAILING", "fail cleanly", DelegateMode::Write)
        .err()
        .expect("provider failure must not be reported as a completed delegate");
    assert!(err.contains("specialist loop failed"), "{err}");
    assert!(
        !base.exists() || std::fs::read_dir(&base).unwrap().next().is_none(),
        "failed delegate leaked a worktree under {}",
        base.display()
    );
    let branches = run_git(&ws, &["branch", "--list", "angel/failing-*"]).unwrap();
    assert!(
        branches.trim().is_empty(),
        "failed delegate leaked branch: {branches}"
    );

    let _ = std::fs::remove_dir_all(base);
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn interrupted_write_delegate_preserves_work_and_checkpoint_without_integrating() {
    let _guard = crate::tests::env_lock();
    if !sandbox::available() {
        return;
    }
    let ws = scratch("interrupted-delegate-work");
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let delegate = DelegateTool::new(
        ws.clone(),
        vec![Arc::new(InterruptedScribe {
            scribe: ScribeClub::new("interrupted-scribe", "candidate.txt", "keep this work"),
            cancel: Arc::clone(&cancel),
        })],
    );
    let error = delegate
        .call_with_cancel(
            &serde_json::json!({"club":"interrupted-scribe", "task":"write then interrupt", "mode":"write"}),
            Some(&cancel),
        )
        .expect_err("must remain incomplete");
    assert!(error.contains("incomplete work preserved"), "{error}");
    let worktree = std::fs::read_dir(delegate.worktree_base())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        std::fs::read_to_string(worktree.join("candidate.txt")).unwrap(),
        "keep this work"
    );
    assert!(
        !ws.join("candidate.txt").exists(),
        "incomplete work must not integrate"
    );
    let run_id = worktree.file_name().unwrap().to_str().unwrap();
    let checkpoint = delegate_session_dir().join(format!("{run_id}.json"));
    let saved: Value = serde_json::from_slice(&std::fs::read(&checkpoint).unwrap()).unwrap();
    assert!(
        saved["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["role"] == "Tool" && m["tool_call_id"] == "w"),
        "tool result must survive interruption"
    );
    // A clean, committed candidate is equally recoverable after cancellation.
    let base = run_git(&ws, &["rev-parse", "HEAD"]).unwrap();
    run_git(&worktree, &["add", "candidate.txt"]).unwrap();
    run_git(&worktree, &["commit", "-qm", "preserved candidate"]).unwrap();
    assert!(super::super::orchestrator::delegate_has_recoverable_work(
        &worktree,
        base.trim()
    ));
    let branch = run_git(&worktree, &["branch", "--show-current"]).unwrap();
    run_git(
        &ws,
        &["worktree", "remove", "--force", worktree.to_str().unwrap()],
    )
    .unwrap();
    run_git(&ws, &["branch", "-D", branch.trim()]).unwrap();
    std::fs::remove_file(checkpoint).unwrap();
    let _ = std::fs::remove_dir_all(delegate.worktree_base());
    let _ = std::fs::remove_dir_all(ws);
}

#[test]
fn nested_workspace_delegates_inside_existing_repo_without_nested_git() {
    if !sandbox::available() {
        eprintln!("landlock unavailable; skipping nested workspace test");
        return;
    }
    let root = std::env::temp_dir().join(format!("angel_nested_repo_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo = ensure_git_workspace(&root).unwrap();
    let workspace = root.join("packages/widget");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join("seed.txt"), "seed\n").unwrap();
    run_git(&repo, &["add", "packages/widget/seed.txt"]).unwrap();
    run_git(&repo, &["commit", "-q", "-m", "add nested workspace"]).unwrap();

    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ScribeClub::new(
        "Nested Writer",
        "note.txt",
        "nested",
    ))];
    let delegate = DelegateTool::new(workspace.clone(), roster);
    let integrate = IntegrateTool::new(workspace.clone());
    let outcome = delegate
        .run(
            " nested writer ",
            "write in nested root",
            DelegateMode::Write,
        )
        .unwrap();
    assert!(
        outcome.diff.contains("packages/widget/note.txt"),
        "delegate wrote outside nested workspace: {}",
        outcome.diff
    );
    integrate.run(&outcome.branch).unwrap();
    assert_eq!(
        std::fs::read_to_string(workspace.join("note.txt")).unwrap(),
        "nested"
    );
    assert!(
        !workspace.join(".git").exists(),
        "nested workspace must not be initialized as a separate repository"
    );

    let _ = std::fs::remove_dir_all(delegate.worktree_base());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn delegate_ref_component_rejects_git_metacharacters() {
    assert_eq!(normalize_club_name("  MIXED Case "), "mixed case");
    assert_eq!(safe_ref_component("../../Spark:QA@{x}"), "spark-qa--x");
    assert_eq!(safe_ref_component("///"), "delegate");
}

#[test]
fn delegate_large_document_result_returns_artifact_preview() {
    let ws = std::env::temp_dir().join(format!("angel_delegate_artifact_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let html = format!(
        "<!doctype html><html><body><canvas id=\"game\"></canvas><script>{}</script></body></html>",
        "requestAnimationFrame(()=>{});".repeat(180)
    );

    let section = delegate_result_section(&ws, "angel/gemma-7", "answer", &html).unwrap();
    assert!(section.contains("answer artifact:"), "{section}");
    assert!(
        !section.contains(&"requestAnimationFrame(()=>{});".repeat(120)),
        "delegate result should only include a bounded preview"
    );
    let artifact = std::fs::read_dir(ws.join("angel_test_output/delegates"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(artifact.extension().and_then(|e| e.to_str()), Some("html"));
    let body = std::fs::read_to_string(&artifact).unwrap();
    assert!(body.contains("<!doctype html>"));
    assert!(body.contains("<canvas id=\"game\">"));

    let _ = std::fs::remove_dir_all(&ws);
}

// --- A1: harness failure nudges are telemetry, excluded from summarizer input ---
