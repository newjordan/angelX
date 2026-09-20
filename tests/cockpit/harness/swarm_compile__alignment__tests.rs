use super::*;
use crate::club::{ClubReply, RouteIdentity};
use crate::harness::swarm_compile::schema::{
    CampaignOwnership, Contribution, ReviewVerdict as SwarmVerdict, SwarmRun,
};
use crate::harness::swarm_compile::{policy::PolicyStore, store::RunStore};
use crate::harness::{AlignmentCriterion, AlignmentProof, ChatMsg, Club, ToolCall, ToolDef};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

fn request() -> CampaignAlignmentRequest {
    CampaignAlignmentRequest {
        campaign_id: "cmp-test".to_string(),
        campaign_revision: 3,
        round: 1,
        contract_digest: "ab".repeat(32),
        swarm_run_id: "swr-test".to_string(),
        candidate_branch: "angel/candidate".to_string(),
        candidate_oid: "caad1da7e012345".to_string(),
        criteria: vec![AlignmentCriterion {
            id: "AC-1".to_string(),
            text: "the feature works".to_string(),
        }],
        proofs: vec![AlignmentProof {
            sha256: "cd".repeat(32),
            summary: "technical gates passed".to_string(),
        }],
    }
}

fn valid_output(request: &CampaignAlignmentRequest) -> String {
    format!(
        "{{\"schema\":\"campaign-review/v1\",\"contract_digest\":\"{}\",\
         \"candidate_oid\":\"{}\",\"criteria\":[{{\"criterion_id\":\"AC-1\",\
         \"verdict\":\"pass\",\"citations\":[\"src/lib.rs:10\"]}}],\
         \"cited_proof_sha256\":[\"{}\"],\"summary\":\"criterion is implemented\"}}\n\
         CAMPAIGN_REVIEW: PASS",
        request.contract_digest, request.candidate_oid, request.proofs[0].sha256
    )
}

#[test]
fn strict_parser_accepts_one_exact_bound_verdict() {
    let request = request();
    let parsed = parse_alignment_output(&valid_output(&request), &request).unwrap();
    assert_eq!(parsed.verdict, AlignmentVerdict::Pass);
    assert_eq!(parsed.cited_criteria, vec!["AC-1"]);
}

#[test]
fn strict_parser_rejects_mismatch_malformed_and_duplicate_verdicts() {
    let request = request();
    let mut mismatch = valid_output(&request);
    mismatch = mismatch.replace(&request.candidate_oid, "deadbeef");
    assert!(parse_alignment_output(&mismatch, &request).is_err());
    assert!(parse_alignment_output("not json\nCAMPAIGN_REVIEW: PASS", &request).is_err());
    let duplicate = format!("{}\nCAMPAIGN_REVIEW: BLOCK", valid_output(&request));
    assert!(
        parse_alignment_output(&duplicate, &request)
            .unwrap_err()
            .contains("exactly one")
    );
}

#[test]
fn independence_fails_closed_for_same_or_unverifiable_identity() {
    assert_eq!(
        classify_independence("impl", "v1", "impl", "v1"),
        AlignmentIndependence::SameRoute
    );
    assert_eq!(
        classify_independence("review", "v1", "impl", "v1"),
        AlignmentIndependence::Unavailable
    );
    assert_eq!(
        classify_independence("review", "v2", "impl", "v1"),
        AlignmentIndependence::DifferentRouteAndRevision
    );
}

struct ReviewClub {
    answer: String,
    write_attempt: bool,
    hops: AtomicUsize,
}

impl Club for ReviewClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(self.answer.clone())
    }

    fn label(&self) -> &str {
        "alignment"
    }

    fn model_identity(&self) -> Option<String> {
        Some("review-v2".to_string())
    }

    fn resolved_route_identity(&self) -> RouteIdentity {
        RouteIdentity {
            driver: "alignment".to_string(),
            model: Some("review-v2".to_string()),
            reasoning_effort: None,
        }
    }

    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        if self.write_attempt && self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "forbidden-write".to_string(),
                name: "shell".to_string(),
                args: serde_json::json!({
                    "command": "printf bad > forbidden-review-write.txt"
                }),
            }]));
        }
        Ok(ClubReply::Text(self.answer.clone()))
    }
}

struct RootClub;

impl Club for RootClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("root must never review".to_string())
    }

    fn label(&self) -> &str {
        "root"
    }

    fn model_identity(&self) -> Option<String> {
        Some("root-v1".to_string())
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn engine_uses_durable_non_root_identity_and_blocks_denied_write_pass() {
    // `campaign_alignment_review` starts a rollout recorder when a concurrent
    // fixture temporarily enables local capture. Its backing directory is
    // process-global configuration, so serialize this reader with fixtures
    // that set and later remove the configured rollout root.
    let _env_guard = crate::tests::env_lock();
    let base = std::env::temp_dir().join(format!("angel-alignment-engine-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let repo = base.join("repo");
    let store_root = base.join("store");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "review@example.invalid"]);
    git(&repo, &["config", "user.name", "Review Test"]);
    std::fs::write(repo.join("baseline.txt"), "green\n").unwrap();
    git(&repo, &["add", "baseline.txt"]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    let oid = git(&repo, &["rev-parse", "HEAD"]);
    let candidate_branch = "angel/swarm/alignment-candidate";
    git(&repo, &["branch", candidate_branch, &oid]);

    let request = CampaignAlignmentRequest {
        campaign_id: "cmp-engine-review".to_string(),
        campaign_revision: 3,
        round: 1,
        contract_digest: "ab".repeat(32),
        swarm_run_id: "swr-engine-review".to_string(),
        candidate_branch: candidate_branch.to_string(),
        candidate_oid: oid.clone(),
        criteria: vec![AlignmentCriterion {
            id: "AC-1".to_string(),
            text: "baseline remains green".to_string(),
        }],
        proofs: vec![AlignmentProof {
            sha256: "cd".repeat(32),
            summary: "technical proof".to_string(),
        }],
    };
    let answer = valid_output(&request);
    let reviewer: Arc<dyn Club> = Arc::new(ReviewClub {
        answer: answer.clone(),
        write_attempt: false,
        hops: AtomicUsize::new(0),
    });
    let root: Arc<dyn Club> = Arc::new(RootClub);
    let mut engine = SwarmCompilerEngine::new(repo.clone(), vec![reviewer], Some(root));
    engine.store = RunStore::new(store_root);
    engine.policy = PolicyStore::new(engine.store.policy_path());

    let mut run = SwarmRun::new(
        request.swarm_run_id.clone(),
        repo.canonicalize().unwrap().to_string_lossy().into_owned(),
        repo.canonicalize().unwrap().to_string_lossy().into_owned(),
        ".".to_string(),
        oid.clone(),
        "review candidate".to_string(),
        "campaign".to_string(),
        "true".to_string(),
        "true".to_string(),
        Vec::new(),
        vec!["baseline.txt".to_string()],
        vec!["alignment".to_string(); 4],
        1,
    );
    run.campaign = Some(CampaignOwnership {
        campaign_id: request.campaign_id.clone(),
        campaign_revision: request.campaign_revision,
        round: request.round,
        contract_digest: request.contract_digest.clone(),
    });
    run.state = RunState::Verified;
    run.verification.technical_pass = true;
    run.verification.review_pass = true;
    run.verification.passed = true;
    run.parked_branch = Some(candidate_branch.to_string());
    run.contributions = vec![
        Contribution {
            task_id: "task-implementer".to_string(),
            role: "implementer".to_string(),
            club: "implementer".to_string(),
            resolved_route: Some("implementer".to_string()),
            model_revision: Some("impl-v1".to_string()),
            base_oid: oid.clone(),
            branch: Some(candidate_branch.to_string()),
            diff_hash: None,
            changed_paths: vec!["baseline.txt".to_string()],
            summary: "implementation".to_string(),
            elapsed_ms: 1,
            tokens: 1,
            review_verdict: None,
        },
        Contribution {
            task_id: "task-reviewer".to_string(),
            role: "reviewer".to_string(),
            club: "technical".to_string(),
            resolved_route: Some("technical".to_string()),
            model_revision: Some("tech-v1".to_string()),
            base_oid: oid.clone(),
            branch: None,
            diff_hash: None,
            changed_paths: Vec::new(),
            summary: "technical pass".to_string(),
            elapsed_ms: 1,
            tokens: 1,
            review_verdict: Some(SwarmVerdict::Pass),
        },
    ];
    engine.store.save(&run).unwrap();
    let receipt = engine
        .campaign_alignment_review(request.clone(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(receipt.verdict, AlignmentVerdict::Pass);
    assert_eq!(receipt.reviewer_route, "alignment");
    assert_eq!(receipt.reviewer_model_revision, "review-v2");
    assert_ne!(receipt.reviewer_route, "root");

    let malicious: Arc<dyn Club> = Arc::new(ReviewClub {
        answer,
        write_attempt: true,
        hops: AtomicUsize::new(0),
    });
    let malicious_root: Arc<dyn Club> = Arc::new(RootClub);
    let mut malicious_engine =
        SwarmCompilerEngine::new(repo.clone(), vec![malicious], Some(malicious_root));
    malicious_engine.store = engine.store.clone();
    malicious_engine.policy = PolicyStore::new(malicious_engine.store.policy_path());
    let error = malicious_engine
        .campaign_alignment_review(request, &AtomicBool::new(false))
        .unwrap_err();
    assert!(error.contains("failed tool attempt"), "{error}");
    assert!(!repo.join("forbidden-review-write.txt").exists());
    assert_eq!(
        git(&repo, &["rev-parse", candidate_branch]),
        oid,
        "the parked candidate ref must not move"
    );
    let _ = std::fs::remove_dir_all(malicious_engine.delegate.worktree_base());
    let _ = std::fs::remove_dir_all(base);
}
