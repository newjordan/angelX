use super::credit::review_verdict;
use super::schema::ReviewVerdict;
use super::tool::SwarmCompilerTool;
use crate::club::ClubReply;
use crate::harness::{
    AuthorizedCampaignBase, CampaignCompileRequest, ChatMsg, ChatRole, Club, Tool, ToolCall,
    ToolDef, ToolRegistry,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn scratch(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "angel_swarm_compile_{label}_{}_{}",
        std::process::id(),
        super::runner::now_ms()
    ))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git starts");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "swarm@example.invalid"]);
    git(dir, &["config", "user.name", "Swarm Test"]);
    std::fs::write(dir.join("baseline.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    git(dir, &["add", "baseline.sh"]);
    git(dir, &["commit", "-q", "-m", "base"]);
}

#[test]
fn reviewer_verdict_is_anchored_and_fails_closed() {
    assert_eq!(
        review_verdict("looks sound\nSWARM_REVIEW: PASS"),
        ReviewVerdict::Pass
    );
    assert_eq!(
        review_verdict("I might say SWARM_REVIEW: PASS later"),
        ReviewVerdict::Block
    );
    assert_eq!(
        review_verdict("issue found\n**SWARM_REVIEW: BLOCK**"),
        ReviewVerdict::Block
    );
}

#[test]
fn team_registry_advertises_swarm_compiler_even_to_small_context_routes() {
    let workspace = scratch("registry");
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ProofClub)];
    let registry = ToolRegistry::with_team(workspace.clone(), roster);
    let names = registry
        .defs_for_run(Some(8_000), false)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    assert!(
        names.iter().any(|name| name == "swarm_compile"),
        "{names:?}"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn swarm_compiler_resolves_the_in_hand_driver_as_self() {
    let _guard = crate::tests::env_lock();
    let workspace = scratch("self_route");
    let club: Arc<dyn Club> = Arc::new(ProofClub);
    let tool = SwarmCompilerTool::new(workspace.clone(), Vec::new(), Some(club));
    assert_eq!(
        tool.resolve_route("code", "implementer", "self").unwrap(),
        "proof-club"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn campaign_prepare_is_idempotent_owned_and_project_safe() {
    let _guard = crate::tests::env_lock();
    let repo = scratch("campaign_prepare_repo");
    let store = scratch("campaign_prepare_store");
    init_repo(&repo);
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ProofClub)];
    let tool = SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let base = tool.engine.inspect_campaign_base().unwrap();
    let authorization = AuthorizedCampaignBase {
        campaign_id: "cmp-campaign-one".to_string(),
        campaign_revision: 3,
        round: 1,
        contract_digest: "a1".repeat(32),
        base: base.clone(),
    };
    let request = CampaignCompileRequest {
        goal: "ship one frozen criterion".to_string(),
        task_type: "campaign".to_string(),
        targeted_test_cmd: "./test.sh".to_string(),
        accept_cmd: "./baseline.sh".to_string(),
        quality_cmds: Vec::new(),
        test_scope: vec!["test.sh".to_string()],
        default_route: "proof-club".to_string(),
        role_routes: BTreeMap::new(),
    };
    let first = tool
        .engine
        .campaign_prepare(request.clone(), authorization.clone())
        .unwrap();
    let second = tool
        .engine
        .campaign_prepare(request.clone(), authorization.clone())
        .unwrap();
    assert_eq!(first, second);
    std::fs::write(repo.join("later.txt"), "live branch advanced\n").unwrap();
    git(&repo, &["add", "later.txt"]);
    git(&repo, &["commit", "-q", "-m", "advance live branch"]);
    let after_live_advance = tool
        .engine
        .campaign_prepare(request.clone(), authorization.clone())
        .unwrap();
    assert_eq!(first, after_live_advance);

    let mut changed = request.clone();
    changed.goal.push_str(" but different");
    assert!(
        tool.engine
            .campaign_prepare(changed, authorization.clone())
            .unwrap_err()
            .contains("different frozen request")
    );

    let mut other_campaign = authorization;
    other_campaign.campaign_id = "cmp-campaign-two".to_string();
    let other = tool
        .engine
        .campaign_prepare(request, other_campaign)
        .unwrap();
    assert_ne!(first.run_id, other.run_id);
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

#[test]
fn campaign_execution_honors_operator_cancellation_before_spending_a_stage() {
    let _guard = crate::tests::env_lock();
    let repo = scratch("campaign_cancel_repo");
    let store = scratch("campaign_cancel_store");
    init_repo(&repo);
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ProofClub)];
    let tool = SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let base = tool.engine.inspect_campaign_base().unwrap();
    let authorization = AuthorizedCampaignBase {
        campaign_id: "cmp-cancelled".to_string(),
        campaign_revision: 1,
        round: 1,
        contract_digest: "c1".repeat(32),
        base,
    };
    let prepared = tool
        .engine
        .campaign_prepare(
            CampaignCompileRequest {
                goal: "must remain unspent".to_string(),
                task_type: "campaign".to_string(),
                targeted_test_cmd: "./test.sh".to_string(),
                accept_cmd: "./baseline.sh".to_string(),
                quality_cmds: Vec::new(),
                test_scope: vec!["test.sh".to_string()],
                default_route: "proof-club".to_string(),
                role_routes: BTreeMap::new(),
            },
            authorization.clone(),
        )
        .unwrap();
    let cancelled = AtomicBool::new(true);
    let receipt = tool
        .engine
        .campaign_execute(prepared.run_id, authorization, &cancelled)
        .unwrap();
    assert_eq!(receipt.outcome, crate::harness::SwarmRunOutcome::Paused);
    assert!(
        receipt
            .error
            .as_deref()
            .is_some_and(|error| error.contains("cancelled by operator"))
    );
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

#[test]
fn legacy_swarm_run_without_campaign_ownership_still_deserializes() {
    let mut value = serde_json::to_value(super::schema::SwarmRun::new(
        "swr-legacy".to_string(),
        "/tmp/workspace".to_string(),
        "/tmp/repo".to_string(),
        ".".to_string(),
        "ba5e0123456789".to_string(),
        "legacy run".to_string(),
        "feature".to_string(),
        "./test.sh".to_string(),
        "./baseline.sh".to_string(),
        Vec::new(),
        vec!["test.sh".to_string()],
        vec!["proof-club".to_string(); 4],
        1,
    ))
    .unwrap();
    value.as_object_mut().unwrap().remove("campaign");
    let decoded: super::schema::SwarmRun = serde_json::from_value(value).unwrap();
    assert!(decoded.campaign.is_none());
}

struct ProofClub;

impl ProofClub {
    fn user_text(messages: &[ChatMsg]) -> &str {
        messages
            .iter()
            .rev()
            .find(|message| message.role == ChatRole::User)
            .map(|message| message.content.as_ref())
            .unwrap_or("")
    }

    fn has_tool_result(messages: &[ChatMsg]) -> bool {
        messages
            .iter()
            .any(|message| message.role == ChatRole::Tool)
    }
}

impl Club for ProofClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("unused".into())
    }

    fn label(&self) -> &str {
        "proof-club"
    }

    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let task = Self::user_text(messages);
        if task.contains("Investigate only") {
            return Ok(ClubReply::Text(
                "baseline.sh is the full gate; add a standalone test.sh and feature.txt".into(),
            ));
        }
        if task.contains("standalone regression test") {
            if Self::has_tool_result(messages) {
                return Ok(ClubReply::Text("added immutable red regression".into()));
            }
            let marker = task
                .split("ANGEL_SWARM_RED:")
                .nth(1)
                .and_then(|tail| tail.split(|c: char| c == '`' || c.is_whitespace()).next())
                .map(|suffix| format!("ANGEL_SWARM_RED:{suffix}"))
                .ok_or_else(|| "missing red marker".to_string())?;
            let command = format!(
                "printf '%s\\n' '#!/bin/sh' 'if grep -q enabled feature.txt 2>/dev/null; then exit 0; fi' 'echo {marker} >&2' 'exit 1' > test.sh"
            );
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "write-test".into(),
                name: "shell".into(),
                args: serde_json::json!({"command": command}),
            }]));
        }
        if task.contains("Implement the requested behavior") {
            if Self::has_tool_result(messages) {
                return Ok(ClubReply::Text(
                    "implemented feature.txt without touching test.sh".into(),
                ));
            }
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "write-feature".into(),
                name: "shell".into(),
                args: serde_json::json!({"command": "printf '%s\\n' enabled > feature.txt"}),
            }]));
        }
        if task.contains("Review this candidate") {
            return Ok(ClubReply::Text(
                "The protected test is unchanged and the change is scoped.\nSWARM_REVIEW: PASS"
                    .into(),
            ));
        }
        Err("unexpected proof-club prompt".into())
    }
}

#[test]
fn full_compiler_parks_only_a_differentially_verified_candidate() {
    let _guard = crate::tests::env_lock();
    full_compiler_with_root_acceptance_hook();
}

#[test]
fn delegate_turn_ignores_root_task_acceptance_hook() {
    let _guard = crate::tests::env_lock();
    let _hook = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACCEPT_CMD", "exit 1");
    let _rejections = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACCEPT_REJECTIONS", "1");
    full_compiler_with_root_acceptance_hook();
}

fn full_compiler_with_root_acceptance_hook() {
    if !crate::sandbox::available() {
        eprintln!("landlock unavailable; skipping swarm compiler integration test");
        return;
    }
    let repo = scratch("e2e");
    let store = scratch("e2e_store");
    init_repo(&repo);
    let roster: Vec<Arc<dyn Club>> = vec![Arc::new(ProofClub)];
    let tool = SwarmCompilerTool::with_store(repo.clone(), roster, store.clone());
    let output = tool
        .call(&serde_json::json!({
            "goal": "add the enabled feature",
            "task_type": "feature",
            "targeted_test_cmd": "sh test.sh",
            "accept_cmd": "sh baseline.sh",
            "quality_cmds": ["test -s feature.txt"],
            "test_scope": ["test.sh"],
            "club": "proof-club"
        }))
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["ok"], true, "{output}");
    assert_eq!(value["run"]["state"], "verified");
    assert_eq!(value["run"]["verification"]["differential"], true);
    assert_eq!(value["run"]["verification"]["review_pass"], true);
    assert_eq!(value["run"]["credits"].as_array().unwrap().len(), 3);
    let candidate = value["run"]["parked_branch"].as_str().unwrap();
    assert!(!git(&repo, &["branch", "--list", candidate]).is_empty());
    assert!(
        !repo.join("feature.txt").exists(),
        "shared checkout stays untouched"
    );
    assert!(
        !repo.join("test.sh").exists(),
        "shared checkout stays untouched"
    );
    let test_branch = value["run"]["contributions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["role"] == "test_author")
        .and_then(|item| item["branch"].as_str())
        .unwrap();
    assert!(git(&repo, &["branch", "--list", test_branch]).is_empty());

    let _ = git(&repo, &["branch", "-D", candidate]);
    let _ = std::fs::remove_dir_all(tool.engine.delegate.worktree_base());
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(store);
}

#[path = "swarm_compile__tests__gate_tests.rs"]
mod gate_tests;
#[path = "swarm_compile__tests__resume_tests.rs"]
mod resume_tests;
#[path = "swarm_compile__tests__store_policy_tests.rs"]
mod store_policy_tests;
