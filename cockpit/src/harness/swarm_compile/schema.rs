use serde::{Deserialize, Serialize};

pub(super) const SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum RunState {
    Planning,
    Running,
    Paused,
    Rejected,
    Verified,
}

impl RunState {
    pub(super) fn terminal(self) -> bool {
        matches!(self, Self::Rejected | Self::Verified)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum NodeState {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TaskNode {
    pub id: String,
    pub role: String,
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub artifact_kind: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    pub route: String,
    pub state: NodeState,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Contribution {
    pub task_id: String,
    pub role: String,
    pub club: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_revision: Option<String>,
    pub base_oid: String,
    pub branch: Option<String>,
    pub diff_hash: Option<String>,
    pub changed_paths: Vec<String>,
    pub summary: String,
    pub elapsed_ms: u128,
    pub tokens: u64,
    pub review_verdict: Option<ReviewVerdict>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReviewVerdict {
    Pass,
    Block,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct CommandProof {
    pub label: String,
    pub git_ref: String,
    pub command: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub marker_seen: bool,
    #[serde(default)]
    pub workspace_clean: bool,
    pub has_test_summary: bool,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub output_tail: String,
    pub elapsed_ms: u128,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct VerificationReport {
    pub baseline_accept: Option<CommandProof>,
    pub test_only_target: Option<CommandProof>,
    pub candidate_target: Option<CommandProof>,
    pub candidate_accept: Option<CommandProof>,
    pub quality: Vec<CommandProof>,
    pub differential: bool,
    pub regression_guard: bool,
    pub technical_pass: bool,
    pub review_pass: bool,
    pub passed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct CreditRow {
    pub role: String,
    pub club: String,
    pub reward: f64,
    pub basis: String,
    pub tokens: u64,
    pub elapsed_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct SwarmRun {
    pub kind: String,
    pub version: u64,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<CampaignOwnership>,
    pub state: RunState,
    pub workspace: String,
    pub repo_root: String,
    pub workspace_rel: String,
    pub base_oid: String,
    pub goal: String,
    pub task_type: String,
    pub targeted_test_cmd: String,
    pub accept_cmd: String,
    pub quality_cmds: Vec<String>,
    pub test_scope: Vec<String>,
    pub red_marker: String,
    pub nodes: Vec<TaskNode>,
    pub contributions: Vec<Contribution>,
    pub verification: VerificationReport,
    pub credits: Vec<CreditRow>,
    pub parked_branch: Option<String>,
    pub last_error: Option<String>,
    pub created_ms: u64,
    pub updated_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct CampaignOwnership {
    pub campaign_id: String,
    pub campaign_revision: u64,
    pub round: u32,
    pub contract_digest: String,
}

impl SwarmRun {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        id: String,
        workspace: String,
        repo_root: String,
        workspace_rel: String,
        base_oid: String,
        goal: String,
        task_type: String,
        targeted_test_cmd: String,
        accept_cmd: String,
        quality_cmds: Vec<String>,
        test_scope: Vec<String>,
        routes: Vec<String>,
        now_ms: u64,
    ) -> Self {
        let roles = ["investigator", "test_author", "implementer", "reviewer"];
        let mut nodes = Vec::with_capacity(5);
        for (index, role) in roles.into_iter().enumerate() {
            let (artifact_kind, acceptance) = role_contract(role);
            nodes.push(TaskNode {
                id: format!("task-{role}"),
                role: role.to_string(),
                depends_on: if index == 0 {
                    Vec::new()
                } else {
                    vec![format!("task-{}", roles[index - 1])]
                },
                artifact_kind: artifact_kind.to_string(),
                acceptance,
                route: routes.get(index).cloned().unwrap_or_default(),
                state: NodeState::Pending,
            });
        }
        nodes.push(TaskNode {
            id: "task-verifier".to_string(),
            role: "verifier".to_string(),
            depends_on: vec!["task-reviewer".to_string()],
            artifact_kind: "proof_bundle".to_string(),
            acceptance: vec![
                "base green".to_string(),
                "marker-bearing red test-only proof".to_string(),
                "candidate targeted/full/quality green".to_string(),
                "review pass".to_string(),
            ],
            route: "deterministic-local".to_string(),
            state: NodeState::Pending,
        });
        Self {
            kind: "angel.swarm_run".to_string(),
            version: SCHEMA_VERSION,
            red_marker: format!("ANGEL_SWARM_RED:{id}"),
            id,
            campaign: None,
            state: RunState::Planning,
            workspace,
            repo_root,
            workspace_rel,
            base_oid,
            goal,
            task_type,
            targeted_test_cmd,
            accept_cmd,
            quality_cmds,
            test_scope,
            nodes,
            contributions: Vec::new(),
            verification: VerificationReport::default(),
            credits: Vec::new(),
            parked_branch: None,
            last_error: None,
            created_ms: now_ms,
            updated_ms: now_ms,
        }
    }

    pub(super) fn node_mut(&mut self, role: &str) -> Option<&mut TaskNode> {
        self.nodes.iter_mut().find(|node| node.role == role)
    }

    pub(super) fn contribution(&self, role: &str) -> Option<&Contribution> {
        self.contributions.iter().find(|item| item.role == role)
    }
}

fn role_contract(role: &str) -> (&'static str, Vec<String>) {
    match role {
        "investigator" => (
            "evidence_packet",
            vec!["bounded path-and-symbol evidence".to_string()],
        ),
        "test_author" => (
            "immutable_test_patch",
            vec![
                "changes stay inside test_scope".to_string(),
                "targeted command fails with the run marker".to_string(),
            ],
        ),
        "implementer" => (
            "production_patch",
            vec![
                "protected test files unchanged".to_string(),
                "targeted and full gates pass".to_string(),
            ],
        ),
        "reviewer" => (
            "adversarial_review",
            vec!["anchored pass-or-block verdict".to_string()],
        ),
        _ => ("artifact", Vec::new()),
    }
}
