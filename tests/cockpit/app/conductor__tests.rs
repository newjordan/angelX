use super::*;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("angel-conductor-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_json(path: &Path, value: serde_json::Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
}

fn init_repo(base: &Path) -> PathBuf {
    let repo = base.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("file.txt"), "base\n").unwrap();
    run_git(&repo, &["init", "-q"]).unwrap();
    run_git(&repo, &["add", "-A"]).unwrap();
    run_git(&repo, &["commit", "-q", "-m", "init"]).unwrap();
    repo
}

fn parked_branch(repo: &Path, base: &Path, branch: &str, text: &str) -> PathBuf {
    let wt = base.join(branch.replace('/', "-"));
    run_git(
        repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            branch,
            &wt.to_string_lossy(),
            "HEAD",
        ],
    )
    .unwrap();
    std::fs::write(wt.join("file.txt"), text).unwrap();
    run_git(&wt, &["add", "-A"]).unwrap();
    run_git(&wt, &["commit", "-q", "-m", "parked"]).unwrap();
    wt
}

fn queue_one(state: &Path, id: &str, branch: &str, wt: &Path) {
    let wt = wt.to_string_lossy().to_string();
    write_json(
        &state.join("queue.json"),
        serde_json::json!([
            {
                "id": id,
                "branch": branch,
                "goal": "Change file.txt",
                "agendaNode": "conductor_code_file",
                "worktree": wt,
                "worktreeTop": wt
            }
        ]),
    );
}

#[test]
fn status_renders_agenda_queue_heartbeat_and_arming_state() {
    let base = scratch("status");
    write_json(
        &base.join("status.json"),
        serde_json::json!({
            "ranking": [
                { "rung": "code", "goal": "Investigate bench regression.", "priority": 8.25 },
                { "rung": "config", "goal": "Review a Tier-B proposal.", "priority": 2.0 }
            ]
        }),
    );
    write_json(
        &base.join("queue.json"),
        serde_json::json!([
            { "id": "q1", "branch": "angel/conductor-1", "goal": "Investigate bench regression." }
        ]),
    );
    write_json(
        &base.join("heartbeat.json"),
        serde_json::json!({
            "ts": "2026-07-07T02:05:00.000Z",
            "action": "skip",
            "reason": "disabled"
        }),
    );

    let text = status_text_in(&base, Some("1"), Some("practice"));
    assert!(
        text.contains("state: armed (code driver: practice)"),
        "{text}"
    );
    assert!(text.contains("1. [code] Investigate bench regression. · priority 8.250"));
    assert!(text.contains("queue: 1 gated branch(es) pending"));
    assert!(text.contains("q1 · angel/conductor-1"));
    assert!(text.contains("heartbeat: skip at 2026-07-07T02:05:00.000Z - disabled"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn status_is_quiet_when_artifacts_are_absent_or_disarmed() {
    let base = scratch("empty");
    let text = status_text_in(&base, None, None);
    assert!(text.contains("state: disarmed"), "{text}");
    assert!(text.contains("agenda: no status export yet"), "{text}");
    assert!(text.contains("queue: empty"), "{text}");
    assert!(text.contains("heartbeat: none"), "{text}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn measure_mode_reports_itself_and_a_typo_falls_through_to_disarmed() {
    let base = scratch("measure");
    // Measure is a real state, not "disarmed" — reporting it as disarmed is how
    // you stop trusting your own dashboard.
    let text = status_text_in(&base, Some("measure"), None);
    assert!(text.contains("state: measure"), "{text}");
    assert!(text.contains("dispatches nothing"), "{text}");

    // A typo must fail into the state that cannot act — never into armed.
    for typo in ["measur", "Measure ", "yes", ""] {
        let text = status_text_in(&base, Some(typo), None);
        let armed = text.contains("state: armed");
        assert!(!armed, "typo {typo:?} must never read as armed: {text}");
    }
    assert!(
        status_text_in(&base, Some("Measure "), None).contains("state: measure"),
        "measure is trimmed and case-insensitive, like the JS side",
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn startup_notice_fires_once_when_queue_count_grows() {
    let base = scratch("notice");
    let queue = base.join("queue.json");
    let stamp = base.join("last-launch");
    write_json(
        &queue,
        serde_json::json!([
            { "id": "q1", "branch": "angel/conductor-1" },
            { "id": "q2", "branch": "angel/conductor-2" }
        ]),
    );

    let first = startup_notice_in(&queue, &stamp);
    assert_eq!(
        first.as_deref(),
        Some("conductor: 2 gated branches await /conductor review")
    );
    assert!(startup_notice_in(&queue, &stamp).is_none());

    write_json(
        &queue,
        serde_json::json!([
            { "id": "q1", "branch": "angel/conductor-1" },
            { "id": "q2", "branch": "angel/conductor-2" },
            { "id": "q3", "branch": "angel/conductor-3" }
        ]),
    );
    let grown = startup_notice_in(&queue, &stamp);
    assert_eq!(
        grown.as_deref(),
        Some("conductor: 3 gated branches await /conductor review")
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn brief_renders_tick_briefing_and_parked_branch_diffstats() {
    let base = scratch("brief");
    std::fs::write(base.join("briefing.md"), "# Morning\n- ran code item\n").unwrap();
    write_json(
        &base.join("queue.json"),
        serde_json::json!([
            {
                "id": "q1",
                "branch": "angel/conductor-1",
                "goal": "Fix bench regression",
                "diffstat": " cockpit/src/lib.rs | 2 ++"
            }
        ]),
    );
    let text = brief_text_in(&base);
    assert!(text.contains("# Morning"));
    assert!(text.contains("parked branches awaiting review"));
    assert!(text.contains("q1 · angel/conductor-1 · Fix bench regression"));
    assert!(text.contains("cockpit/src/lib.rs | 2 ++"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn approve_merges_clean_live_tree_removes_worktree_and_spools() {
    let base = scratch("approve");
    let state = base.join("state");
    let repo = init_repo(&base);
    let branch = "angel/conductor-test-approve";
    let wt = parked_branch(&repo, &base, branch, "approved\n");
    queue_one(&state, "q-approve", branch, &wt);

    let msg = approve_in(&state, &repo, "q-approve");
    assert!(msg.contains("conductor approved"), "{msg}");
    assert_eq!(
        std::fs::read_to_string(repo.join("file.txt")).unwrap(),
        "approved\n"
    );
    assert!(!wt.exists(), "worktree removed: {msg}");
    let queue = std::fs::read_to_string(state.join("queue.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<serde_json::Value>>(&queue)
            .unwrap()
            .len(),
        0
    );
    let spool = std::fs::read_to_string(state.join("verdicts.jsonl")).unwrap();
    assert!(spool.contains("\"action\":\"approve\""));
    assert!(spool.contains("conductor_code_file"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn approve_refuses_dirty_live_tree_and_keeps_queue() {
    let base = scratch("dirty");
    let state = base.join("state");
    let repo = init_repo(&base);
    let branch = "angel/conductor-test-dirty";
    let wt = parked_branch(&repo, &base, branch, "approved\n");
    queue_one(&state, "q-dirty", branch, &wt);
    std::fs::write(repo.join("local.txt"), "uncommitted\n").unwrap();

    let msg = approve_in(&state, &repo, "q-dirty");
    assert!(msg.contains("live tree is dirty"), "{msg}");
    let queue = std::fs::read_to_string(state.join("queue.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<serde_json::Value>>(&queue)
            .unwrap()
            .len(),
        1
    );
    assert!(wt.exists(), "worktree kept");
    let _ = run_git(
        &repo,
        &["worktree", "remove", "--force", &wt.to_string_lossy()],
    );
    let _ = run_git(&repo, &["branch", "-D", branch]);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn reject_removes_worktree_deletes_branch_and_spools() {
    let base = scratch("reject");
    let state = base.join("state");
    let repo = init_repo(&base);
    let branch = "angel/conductor-test-reject";
    let wt = parked_branch(&repo, &base, branch, "rejected\n");
    queue_one(&state, "q-reject", branch, &wt);

    let msg = reject_in(&state, &repo, "q-reject");
    assert!(msg.contains("conductor rejected"), "{msg}");
    assert_eq!(
        std::fs::read_to_string(repo.join("file.txt")).unwrap(),
        "base\n"
    );
    assert!(!wt.exists(), "worktree removed");
    assert!(run_git(&repo, &["rev-parse", "--verify", branch]).is_err());
    let queue = std::fs::read_to_string(state.join("queue.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<serde_json::Value>>(&queue)
            .unwrap()
            .len(),
        0
    );
    let spool = std::fs::read_to_string(state.join("verdicts.jsonl")).unwrap();
    assert!(spool.contains("\"action\":\"reject\""));
    assert!(spool.contains("conductor_code_file"));
    let _ = std::fs::remove_dir_all(&base);
}
