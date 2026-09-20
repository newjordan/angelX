use super::{ProofClub, git, init_repo, scratch};
use crate::club::ClubReply;
use crate::harness::{ChatMsg, Club, Tool, ToolDef};
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
    if !crate::sandbox::available() {
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
