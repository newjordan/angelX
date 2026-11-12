use super::{ProofClub, git, init_repo, scratch};
use crate::agent::club::ClubReply;
use crate::agent::harness::{ChatMsg, Club, Tool, ToolDef};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct GatedProofClub {
    fail: Arc<AtomicBool>,
}

impl Club for GatedProofClub {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        ProofClub.respond(prompt)
    }

    fn label(&self) -> &str {
        "proof-club"
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("synthetic provider outage".to_string());
        }
        ProofClub.chat(messages, tools)
    }
}

#[test]
fn paused_run_resumes_from_durable_graph_without_restarting() {
    let _guard = crate::tests::env_lock();
    // This case exercises an operator-selected pause and later resume, not
    // automatic outage recovery (whose default now waits for recovery/cancel).
    let _retries = crate::tests::TestEnvGuard::set("ANGEL_PROVIDER_RETRIES", "0");
    if !crate::agent::sandbox::available() {
        eprintln!("skip: host-capability landlock unavailable");
        return;
    }
    let repo = scratch("resume");
    let store = scratch("resume_store");
    init_repo(&repo);
    let fail = Arc::new(AtomicBool::new(true));
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(GatedProofClub {
        fail: Arc::clone(&fail),
    })];
    let tool = super::SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let error = tool
        .call(&serde_json::json!({
            "goal": "add the enabled feature",
            "targeted_test_cmd": "sh test.sh",
            "accept_cmd": "sh baseline.sh",
            "test_scope": ["test.sh"],
            "club": "proof-club"
        }))
        .expect_err("first provider call must pause the graph");
    let run_id = error
        .split_whitespace()
        .find(|word| word.starts_with("swr-"))
        .expect("error carries resumable run id")
        .to_string();
    let paused: serde_json::Value = serde_json::from_str(
        &tool
            .call(&serde_json::json!({"action":"status", "run_id":run_id}))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(paused["run"]["state"], "paused");

    fail.store(false, Ordering::SeqCst);
    let resumed: serde_json::Value = serde_json::from_str(
        &tool
            .call(&serde_json::json!({"action":"resume", "run_id":run_id}))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(resumed["run"]["state"], "verified");
    let candidate = resumed["run"]["parked_branch"].as_str().unwrap();
    let _ = git(&repo, &["branch", "-D", candidate]);
    let _ = std::fs::remove_dir_all(tool.engine.delegate.worktree_base());
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

#[test]
fn dirty_workspace_is_rejected_before_any_agent_runs() {
    let _guard = crate::tests::env_lock();
    let repo = scratch("dirty");
    let store = scratch("dirty_store");
    init_repo(&repo);
    std::fs::write(repo.join("user-work.txt"), "mine").unwrap();
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ProofClub)];
    let tool = super::SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let error = tool
        .call(&serde_json::json!({
            "goal": "add the enabled feature",
            "targeted_test_cmd": "sh test.sh",
            "accept_cmd": "sh baseline.sh",
            "test_scope": ["test.sh"],
            "club": "proof-club"
        }))
        .expect_err("dirty workspace must not be silently omitted from the frozen base");
    assert!(error.contains("clean workspace"), "{error}");
    assert!(
        !store.exists(),
        "no run is minted before the cleanliness gate"
    );
    let _ = std::fs::remove_dir_all(repo);
}

fn prepared_campaign_run(tool: &super::SwarmCompilerTool, label: &str) -> String {
    let base = tool.engine.inspect_campaign_base().unwrap();
    let authorization = super::AuthorizedCampaignBase {
        campaign_id: format!("cmp-{label}"),
        campaign_revision: 1,
        round: 1,
        contract_digest: "e1".repeat(32),
        base,
    };
    tool.engine
        .campaign_prepare(
            super::CampaignCompileRequest {
                goal: "leave a run for status to read".into(),
                task_type: "campaign".into(),
                targeted_test_cmd: "sh baseline.sh".into(),
                accept_cmd: "sh baseline.sh".into(),
                quality_cmds: Vec::new(),
                test_scope: vec!["baseline.sh".into()],
                default_route: "proof-club".into(),
                role_routes: std::collections::BTreeMap::new(),
            },
            authorization,
        )
        .unwrap()
        .run_id
}

fn status_of(tool: &super::SwarmCompilerTool, run_id: &str) -> (String, String) {
    let status: serde_json::Value = serde_json::from_str(
        &tool
            .call(&serde_json::json!({"action": "status", "run_id": run_id}))
            .unwrap(),
    )
    .unwrap();
    (
        status["run"]["state"].as_str().unwrap().to_string(),
        status["next_step"].as_str().unwrap().to_string(),
    )
}

fn resume_step(run_id: &str) -> String {
    format!(
        "{} run_id={run_id}",
        crate::agent::harness::book::d56_replies::RESUME_RUN.cells()
    )
}

/// A process killed mid-stage never reaches the error path that parks the run
/// as paused, so the file keeps saying `running` with nobody executing it.
/// `status` is the only signal the model gets; it must point at resume, and
/// only stay "still active" while an executor really holds the run's lease.
#[test]
fn a_run_left_running_by_a_dead_executor_points_at_resume() {
    let _guard = crate::tests::env_lock();
    let repo = scratch("orphan_repo");
    let store = scratch("orphan_store");
    init_repo(&repo);
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ProofClub)];
    let tool = super::SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let run_id = prepared_campaign_run(&tool, "orphan");

    // Prepared and never started: nothing is executing it.
    let (state, next) = status_of(&tool, &run_id);
    assert_eq!(state, "planning");
    assert_eq!(
        next,
        resume_step(&run_id),
        "a prepared run that nobody runs"
    );

    // The file a killed executor leaves: running, with a node mid-stage.
    let mut run = tool.engine.store.load(&run_id).unwrap();
    run.state = super::super::schema::RunState::Running;
    run.nodes[0].state = super::super::schema::NodeState::Running;
    tool.engine.store.save(&run).unwrap();
    let (state, next) = status_of(&tool, &run_id);
    assert_eq!(state, "running");
    assert_eq!(next, resume_step(&run_id), "a run whose executor is gone");

    // A live executor holds the lease: the run really is active.
    let lease = tool.engine.store.acquire_lease(&run_id).unwrap();
    let (state, next) = status_of(&tool, &run_id);
    assert_eq!(state, "running");
    assert_eq!(next, "Run is still active.");
    drop(lease);

    // The lease is released, as the kernel does for a dead process.
    let (_, next) = status_of(&tool, &run_id);
    assert_eq!(next, resume_step(&run_id));

    // The probe leaves no lock behind: an executor can still start.
    let _lease = tool.engine.store.acquire_lease(&run_id).unwrap();
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

#[test]
fn a_paused_run_keeps_pointing_at_resume_and_a_finished_run_at_its_outcome() {
    let _guard = crate::tests::env_lock();
    let repo = scratch("paused_status_repo");
    let store = scratch("paused_status_store");
    init_repo(&repo);
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ProofClub)];
    let tool = super::SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let run_id = prepared_campaign_run(&tool, "paused-status");
    let mut run = tool.engine.store.load(&run_id).unwrap();
    run.state = super::super::schema::RunState::Paused;
    tool.engine.store.save(&run).unwrap();
    assert_eq!(status_of(&tool, &run_id).1, resume_step(&run_id));
    run.state = super::super::schema::RunState::Rejected;
    tool.engine.store.save(&run).unwrap();
    assert_eq!(
        status_of(&tool, &run_id).1,
        crate::agent::harness::book::d56_replies::REJECTED_RUN.cells()
    );
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

/// `failed` on a node has meant two things: the stage rejected the run for
/// good, or an error interrupted the stage and resume will run it again. The
/// JSON the model reads must tell them apart.
#[test]
fn a_stage_interrupted_mid_run_is_not_reported_as_a_failed_stage() {
    let _guard = crate::tests::env_lock();
    let repo = scratch("interrupted_node_repo");
    let store = scratch("interrupted_node_store");
    init_repo(&repo);
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let release = Arc::new(AtomicBool::new(false));
    let club: Arc<dyn Club> = Arc::new(super::MidStageCancellationClub {
        started: started_tx,
        observed: observed_tx,
        release: Arc::clone(&release),
    });
    let tool = super::SwarmCompilerTool::with_store(repo.clone(), vec![club], store.clone());
    let base = tool.engine.inspect_campaign_base().unwrap();
    let authorization = super::AuthorizedCampaignBase {
        campaign_id: "cmp-interrupted-node".into(),
        campaign_revision: 1,
        round: 1,
        contract_digest: "f1".repeat(32),
        base,
    };
    let prepared = tool
        .engine
        .campaign_prepare(
            super::CampaignCompileRequest {
                goal: "interrupt the first stage".into(),
                task_type: "campaign".into(),
                targeted_test_cmd: "sh baseline.sh".into(),
                accept_cmd: "sh baseline.sh".into(),
                quality_cmds: Vec::new(),
                test_scope: vec!["baseline.sh".into()],
                default_route: "mid-stage-cancel".into(),
                role_routes: std::collections::BTreeMap::new(),
            },
            authorization.clone(),
        )
        .unwrap();
    let engine = Arc::clone(&tool.engine);
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let run_id = prepared.run_id.clone();
    let worker = std::thread::spawn(move || {
        engine.campaign_execute(prepared.run_id, authorization, &worker_cancelled)
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the run reaches its first stage");
    cancelled.store(true, Ordering::Release);
    if observed_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .is_err()
    {
        release.store(true, Ordering::Release);
        let _ = worker.join();
        panic!("the stage did not settle after cancellation");
    }
    let receipt = worker.join().unwrap().unwrap();
    assert_eq!(
        receipt.outcome,
        crate::agent::harness::SwarmRunOutcome::Paused
    );

    let (state, _) = status_of(&tool, &run_id);
    assert_eq!(state, "paused");
    let status: serde_json::Value = serde_json::from_str(
        &tool
            .call(&serde_json::json!({"action": "status", "run_id": run_id}))
            .unwrap(),
    )
    .unwrap();
    let nodes = status["run"]["nodes"].as_array().unwrap();
    let investigator = nodes.iter().find(|n| n["role"] == "investigator").unwrap();
    assert_eq!(
        investigator["state"], "interrupted",
        "the stage was cut off, not rejected: {investigator}"
    );
    let _ = std::fs::remove_dir_all(tool.engine.delegate.worktree_base());
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

/// A stage that rejects the run leaves its node `failed` and the run
/// `rejected`: terminal, nothing for resume to redo.
#[test]
fn a_stage_that_rejects_the_run_reads_failed() {
    let _guard = crate::tests::env_lock();
    if !crate::agent::sandbox::available() {
        eprintln!("landlock unavailable; skipping rejected-stage test");
        return;
    }
    /// Writes outside `test_scope`, which rejects the run at the test author.
    struct RogueTestAuthor;
    impl Club for RogueTestAuthor {
        fn respond(&self, prompt: &str) -> Result<String, String> {
            ProofClub.respond(prompt)
        }
        fn label(&self) -> &str {
            "proof-club"
        }
        fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
            use crate::agent::harness::book::d6_long_run::TEST_AUTHOR;
            let task = messages
                .iter()
                .rev()
                .find(|m| m.role == crate::agent::harness::ChatRole::User)
                .map(|m| m.content.to_string())
                .unwrap_or_default();
            if task.starts_with(&TEST_AUTHOR.cells()) {
                if messages
                    .iter()
                    .any(|m| m.role == crate::agent::harness::ChatRole::Tool)
                {
                    return Ok(ClubReply::Text("wrote a file".into()));
                }
                return Ok(ClubReply::Calls(vec![crate::agent::harness::ToolCall {
                    id: "write-rogue".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"command": "printf '%s\\n' x > rogue.txt"}),
                }]));
            }
            ProofClub.chat(messages, tools)
        }
    }
    let repo = scratch("rejected_node_repo");
    let store = scratch("rejected_node_store");
    init_repo(&repo);
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(RogueTestAuthor)];
    let tool = super::SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let output = tool
        .call(&serde_json::json!({
            "goal": "add the enabled feature",
            "targeted_test_cmd": "sh test.sh",
            "accept_cmd": "sh baseline.sh",
            "test_scope": ["test.sh"],
            "club": "proof-club"
        }))
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["run"]["state"], "rejected", "{output}");
    let nodes = value["run"]["nodes"].as_array().unwrap();
    let author = nodes.iter().find(|n| n["role"] == "test_author").unwrap();
    assert_eq!(author["state"], "failed", "{author}");
    let _ = std::fs::remove_dir_all(tool.engine.delegate.worktree_base());
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

/// Run files written before the split say `failed` for both; they still load.
#[test]
fn a_run_file_written_with_failed_nodes_still_loads() {
    let mut value = serde_json::to_value(super::super::schema::SwarmRun::new(
        "swr-old-failed".to_string(),
        "/tmp/workspace".to_string(),
        "/tmp/repo".to_string(),
        ".".to_string(),
        "ba5e0123456789".to_string(),
        "old run".to_string(),
        "feature".to_string(),
        "./test.sh".to_string(),
        "./baseline.sh".to_string(),
        Vec::new(),
        vec!["test.sh".to_string()],
        vec!["proof-club".to_string(); 4],
        1,
    ))
    .unwrap();
    value["state"] = "paused".into();
    value["nodes"][1]["state"] = "failed".into();
    value["nodes"][2]["state"] = "running".into();
    let run: super::super::schema::SwarmRun = serde_json::from_value(value).unwrap();
    assert_eq!(run.nodes[1].state, super::super::schema::NodeState::Failed);
    assert_eq!(run.nodes[2].state, super::super::schema::NodeState::Running);
    assert_eq!(
        serde_json::to_value(&run).unwrap()["nodes"][1]["state"],
        "failed"
    );
}
