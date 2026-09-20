use super::policy::PolicyStore;
use super::store::RunStore;
use crate::agent::harness::{
    Club, DelegateTool, PinnedCargo, Tool, ToolDef, Value, normalize_club_name,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) struct SwarmCompilerTool {
    pub(super) engine: Arc<SwarmCompilerEngine>,
}

pub(crate) struct SwarmCompilerEngine {
    pub(super) workspace: PathBuf,
    pub(super) clubs: HashMap<String, Arc<dyn Club>>,
    pub(super) self_label: Option<String>,
    pub(super) delegate: DelegateTool,
    pub(super) store: RunStore,
    pub(super) policy: PolicyStore,
}

#[derive(Clone, Debug)]
pub(crate) struct CompileRequest {
    pub goal: String,
    pub task_type: String,
    pub targeted_test_cmd: String,
    pub accept_cmd: String,
    pub quality_cmds: Vec<String>,
    pub test_scope: Vec<String>,
    pub default_route: String,
    pub role_routes: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub(crate) struct CampaignCompileRequest {
    pub(crate) goal: String,
    pub(crate) task_type: String,
    pub(crate) targeted_test_cmd: String,
    pub(crate) accept_cmd: String,
    pub(crate) quality_cmds: Vec<String>,
    pub(crate) test_scope: Vec<String>,
    pub(crate) default_route: String,
    pub(crate) role_routes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignBase {
    pub(crate) workspace: PathBuf,
    pub(crate) repo_root: PathBuf,
    pub(crate) workspace_rel: String,
    pub(crate) base_oid: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthorizedCampaignBase {
    pub(crate) campaign_id: String,
    pub(crate) campaign_revision: u64,
    pub(crate) round: u32,
    pub(crate) contract_digest: String,
    pub(crate) base: CampaignBase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PreparedSwarmRun {
    pub(crate) run_id: String,
    pub(crate) base_oid: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SwarmRunOutcome {
    Paused,
    Rejected,
    Verified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SwarmRunReceipt {
    pub(crate) run_id: String,
    pub(crate) outcome: SwarmRunOutcome,
    pub(crate) base_oid: String,
    pub(crate) parked_branch: Option<String>,
    pub(crate) candidate_oid: Option<String>,
    pub(crate) changed_paths: Vec<String>,
    pub(crate) technical_pass: bool,
    pub(crate) code_review_pass: bool,
    pub(crate) proof_path: PathBuf,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignAlignmentRequest {
    pub(crate) campaign_id: String,
    pub(crate) campaign_revision: u64,
    pub(crate) round: u32,
    pub(crate) contract_digest: String,
    pub(crate) swarm_run_id: String,
    pub(crate) candidate_branch: String,
    pub(crate) candidate_oid: String,
    pub(crate) criteria: Vec<AlignmentCriterion>,
    pub(crate) proofs: Vec<AlignmentProof>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AlignmentCriterion {
    pub(crate) id: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AlignmentProof {
    pub(crate) sha256: String,
    pub(crate) summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AlignmentVerdict {
    Pass,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AlignmentIndependence {
    DifferentRouteAndRevision,
    DifferentRevision,
    SameRoute,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CampaignAlignmentReceipt {
    pub(crate) schema: String,
    pub(crate) verdict: AlignmentVerdict,
    pub(crate) reviewer_route: String,
    pub(crate) reviewer_model_revision: String,
    pub(crate) implementer_route: String,
    pub(crate) implementer_model_revision: String,
    pub(crate) technical_reviewer_route: String,
    pub(crate) technical_reviewer_model_revision: String,
    pub(crate) technical_review_independence: AlignmentIndependence,
    pub(crate) independence: AlignmentIndependence,
    pub(crate) reviewed_contract_digest: String,
    pub(crate) reviewed_candidate_oid: String,
    pub(crate) cited_criteria: Vec<String>,
    pub(crate) cited_proof_sha256: Vec<String>,
    pub(crate) summary: String,
    pub(crate) response_sha256: String,
}

impl From<CampaignCompileRequest> for CompileRequest {
    fn from(request: CampaignCompileRequest) -> Self {
        Self {
            goal: request.goal,
            task_type: request.task_type,
            targeted_test_cmd: request.targeted_test_cmd,
            accept_cmd: request.accept_cmd,
            quality_cmds: request.quality_cmds,
            test_scope: request.test_scope,
            default_route: request.default_route,
            role_routes: request.role_routes,
        }
    }
}

impl CompileRequest {
    pub(super) fn route_for(&self, role: &str) -> &str {
        self.role_routes
            .get(role)
            .map(String::as_str)
            .unwrap_or(&self.default_route)
    }
}

impl SwarmCompilerTool {
    #[cfg(test)]
    pub(crate) fn new(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        self_club: Option<Arc<dyn Club>>,
    ) -> Self {
        Self::from_engine(Arc::new(SwarmCompilerEngine::new(
            workspace, roster, self_club,
        )))
    }

    pub(crate) fn from_engine(engine: Arc<SwarmCompilerEngine>) -> Self {
        Self { engine }
    }

    #[cfg(test)]
    pub(super) fn with_store(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        store_root: PathBuf,
    ) -> Self {
        Self::from_engine(Arc::new(SwarmCompilerEngine::with_store(
            workspace, roster, store_root,
        )))
    }

    #[cfg(test)]
    pub(super) fn resolve_route(
        &self,
        task_type: &str,
        role: &str,
        requested: &str,
    ) -> Result<String, String> {
        self.engine.resolve_route(task_type, role, requested)
    }
}

impl SwarmCompilerEngine {
    #[cfg(test)]
    pub(crate) fn new(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        self_club: Option<Arc<dyn Club>>,
    ) -> Self {
        let cargo = PinnedCargo::capture(&workspace);
        Self::new_with_cargo(workspace, roster, self_club, cargo)
    }

    pub(crate) fn new_with_cargo(
        workspace: PathBuf,
        mut roster: Vec<Arc<dyn Club>>,
        self_club: Option<Arc<dyn Club>>,
        cargo: PinnedCargo,
    ) -> Self {
        let self_label = self_club
            .as_ref()
            .map(|club| normalize_club_name(club.label()));
        if let Some(club) = self_club
            && !roster.iter().any(|candidate| {
                normalize_club_name(candidate.label()) == normalize_club_name(club.label())
            })
        {
            roster.push(club);
        }
        let clubs = roster
            .iter()
            .map(|club| (normalize_club_name(club.label()), Arc::clone(club)))
            .collect::<HashMap<_, _>>();
        let store = RunStore::for_workspace(&workspace);
        let policy = PolicyStore::new(store.policy_path());
        Self {
            workspace: workspace.clone(),
            clubs,
            self_label,
            delegate: DelegateTool::new_with_cargo(workspace, roster, cargo),
            store,
            policy,
        }
    }

    #[cfg(test)]
    pub(super) fn with_store(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        store_root: PathBuf,
    ) -> Self {
        let mut engine = Self::new(workspace, roster, None);
        engine.store = RunStore::new(store_root);
        engine.policy = PolicyStore::new(engine.store.policy_path());
        engine
    }

    pub(super) fn resolve_route(
        &self,
        task_type: &str,
        role: &str,
        requested: &str,
    ) -> Result<String, String> {
        let requested = normalize_club_name(requested);
        let requested = if requested.is_empty() {
            "auto"
        } else {
            &requested
        };
        if requested == "self" {
            let label = self
                .self_label
                .as_ref()
                .ok_or_else(|| "club=self is unavailable in this registry".to_string())?;
            let club = self
                .clubs
                .get(label)
                .ok_or_else(|| "in-hand club disappeared from the swarm roster".to_string())?;
            return club
                .is_available()
                .then(|| label.clone())
                .ok_or_else(|| format!("club '{label}' is not reachable"));
        }
        if requested != "auto" {
            let club = self
                .clubs
                .get(requested)
                .ok_or_else(|| format!("unknown swarm compiler club '{requested}'"))?;
            if club.is_available() {
                return Ok(requested.to_string());
            }
            if !crate::agent::tools::consult::is_optional_local_label(club.label()) {
                return Err(format!("club '{requested}' is not reachable"));
            }
            // Optional local is down — pick among live seats instead of failing.
        }
        let candidates = self
            .clubs
            .iter()
            .filter(|(_, club)| club.is_available())
            .map(|(label, _)| label.clone())
            .collect::<Vec<_>>();
        self.policy.select(task_type, role, &candidates)
    }
}

impl Tool for SwarmCompilerTool {
    fn name(&self) -> &str {
        "swarm_compile"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "swarm_compile".to_string(),
            description: "Compile a green-base coding goal into a durable proof graph: isolated investigator, independent regression-test author, implementer, adversarial reviewer, differential/full verification, causal credit, and learned routing. Use direct action instead when a trustworthy red verifier already exists. A verified result is parked on a branch and never auto-merged. Actions: run, resume, status, policy."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {"type":"string", "enum":["run","resume","status","policy"], "default":"run"},
                    "run_id": {"type":"string", "description":"required for resume/status"},
                    "goal": {"type":"string", "description":"coding outcome to produce"},
                    "task_type": {"type":"string", "description":"optional stable class used by learned routing"},
                    "targeted_test_cmd": {"type":"string", "description":"must fail on the immutable test-only branch with the supplied run marker and pass on the candidate"},
                    "accept_cmd": {"type":"string", "description":"full acceptance command; must pass on base and candidate"},
                    "quality_cmds": {"type":"array", "items":{"type":"string"}, "description":"optional candidate-only lint/build/typecheck commands (max 4)"},
                    "test_scope": {"type":"array", "items":{"type":"string"}, "description":"allowed test-file paths/prefixes; implementation may not edit files the test author changes"},
                    "club": {"type":"string", "default":"auto", "description":"auto, self, or one explicit route for all roles"},
                    "routes": {"type":"object", "description":"optional per-role route overrides for investigator/test_author/implementer/reviewer"}
                },
                "required": []
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        match args.get("action").and_then(Value::as_str).unwrap_or("run") {
            "run" => self.engine.start(parse_request(args)?),
            "resume" => self.engine.resume(required_string(args, "run_id", 96)?),
            "status" => self.engine.status(required_string(args, "run_id", 96)?),
            "policy" => serde_json::to_string_pretty(&self.engine.policy.load()?)
                .map_err(|e| format!("render routing policy: {e}")),
            other => Err(format!("unknown swarm_compile action '{other}'")),
        }
    }
}

fn parse_request(args: &Value) -> Result<CompileRequest, String> {
    let goal = required_string(args, "goal", 4_000)?;
    let targeted_test_cmd = required_string(args, "targeted_test_cmd", 2_000)?;
    let accept_cmd = required_string(args, "accept_cmd", 2_000)?;
    let task_type = args
        .get("task_type")
        .and_then(Value::as_str)
        .map(safe_task_type)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| infer_task_type(&goal).to_string());
    let quality_cmds = string_array(args, "quality_cmds", 4, 2_000)?;
    let test_scope = string_array(args, "test_scope", 16, 400)?;
    if test_scope.is_empty() {
        return Err("test_scope needs at least one allowed test path/prefix".to_string());
    }
    if test_scope.iter().any(|path| {
        path.starts_with('/') || path.split('/').any(|part| part == "..") || path.contains('\0')
    }) {
        return Err("test_scope entries must be relative paths without '..'".to_string());
    }
    let default_route = args
        .get("club")
        .and_then(Value::as_str)
        .unwrap_or("auto")
        .trim()
        .to_string();
    let mut role_routes = BTreeMap::new();
    if let Some(routes) = args.get("routes").and_then(Value::as_object) {
        for role in ["investigator", "test_author", "implementer", "reviewer"] {
            if let Some(route) = routes.get(role).and_then(Value::as_str) {
                role_routes.insert(role.to_string(), route.trim().to_string());
            }
        }
    }
    Ok(CompileRequest {
        goal,
        task_type,
        targeted_test_cmd,
        accept_cmd,
        quality_cmds,
        test_scope,
        default_route,
        role_routes,
    })
}

fn required_string(args: &Value, key: &str, cap: usize) -> Result<String, String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} is required"))?;
    (value.chars().count() <= cap)
        .then(|| value.to_string())
        .ok_or_else(|| format!("{key} exceeds {cap} characters"))
}

fn string_array(args: &Value, key: &str, max: usize, cap: usize) -> Result<Vec<String>, String> {
    let Some(values) = args.get(key) else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| format!("{key} must be an array of strings"))?;
    if values.len() > max {
        return Err(format!("{key} accepts at most {max} entries"));
    }
    values
        .iter()
        .map(|value| {
            let text = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| format!("{key} entries must be non-empty strings"))?;
            (text.chars().count() <= cap)
                .then(|| text.to_string())
                .ok_or_else(|| format!("a {key} entry exceeds {cap} characters"))
        })
        .collect()
}

fn safe_task_type(raw: &str) -> String {
    raw.chars()
        .take(80)
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_ascii_lowercase()
}

pub(super) fn infer_task_type(goal: &str) -> &'static str {
    let lower = goal.to_ascii_lowercase();
    if ["bug", "fix", "broken", "regression", "error"]
        .iter()
        .any(|word| lower.contains(word))
    {
        "bugfix"
    } else if ["speed", "performance", "latency", "optimize"]
        .iter()
        .any(|word| lower.contains(word))
    {
        "performance"
    } else if ["refactor", "cleanup", "simplify"]
        .iter()
        .any(|word| lower.contains(word))
    {
        "refactor"
    } else if ["test", "coverage", "assert"]
        .iter()
        .any(|word| lower.contains(word))
    {
        "testing"
    } else if ["document", "readme", "docs"]
        .iter()
        .any(|word| lower.contains(word))
    {
        "documentation"
    } else {
        "feature"
    }
}
