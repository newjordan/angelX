//! Executable Labyrinth campaigns. Every role owns a copied workspace and a
//! fresh conversation; only the coordinator may hand checked edits to the
//! canonical-artifact integrator. Reports and execution receipts are immutable.

use crate::agent::club::{ChatMsg, Club, ClubReply, RouteIdentity, StreamDelta, ToolDef};
use crate::agent::harness::book::{self, labyrinth_campaign as chapter};
use crate::agent::harness::{
    DescendantBudgetScope, SubcallDepthGuard, Tool, ToolRegistry, TurnStopReason,
    confined_open_read_no_symlinks, confined_publish_new_no_symlinks, confined_read_dir,
    confined_read_limited_no_symlinks, confined_write, current_descendant_budget,
    reserve_descendant_calls, reserve_spawn_seats, run_sandboxed_observed_cancellable,
    run_turn_observed, with_call_budget,
};
use crate::agent::sandbox::SandboxPolicy;
use crate::knowledge::cut::sha256_hex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FILE_BYTES: usize = 8 * 1024 * 1024;
const REPORT_BYTES: usize = 128 * 1024;
pub(crate) const DOCUMENT_EDIT_BYTES: usize = 128 * 1024;
const SNAPSHOT_BYTES: usize = 128 * 1024 * 1024;
const MAX_FILES: usize = 1024;
const MAX_STATES: usize = 512;
const MAX_LEADS: usize = 128;
const SOURCE_INDEX: &str = ".campaign-inputs.json";

const ESCALATION_RUNGS: usize = 9;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CampaignSpec {
    pub id: String,
    pub task: String,
    pub doors: Vec<CampaignDoor>,
    /// Exact files supplied to literature/attack/writer roles. No implicit repo copy.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Definitions and raw data for the fresh referee, without author code.
    pub referee_inputs: Vec<String>,
    pub canonical_documents: Vec<String>,
    /// Coordinator-owned checks, copied from the user's configuration, never a draft.
    pub checks: Vec<CheckCommand>,
    pub spot_checks: Vec<CheckCommand>,
    #[serde(default)]
    pub closed_routes: Vec<ClosedRoute>,
    #[serde(default)]
    pub side_projects: Vec<String>,
    #[serde(default = "default_compute_rules")]
    pub compute_rules: String,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_hops")]
    pub max_hops: usize,
    #[serde(default = "default_role_secs")]
    pub role_timeout_secs: u64,
    #[serde(default = "default_campaign_secs")]
    pub campaign_timeout_secs: u64,
    /// One initial attack plus at most the nine saturation rungs.
    #[serde(default = "default_rounds")]
    pub max_rounds: usize,
    #[serde(default = "default_retries")]
    pub writer_retries: usize,
}

fn default_compute_rules() -> String {
    book::d3_roles::pages(chapter::COORDINATOR, [7])
}
fn default_concurrency() -> usize {
    2
}
fn default_hops() -> usize {
    64
}
fn default_role_secs() -> u64 {
    1800
}
fn default_campaign_secs() -> u64 {
    7200
}
fn default_rounds() -> usize {
    1
}
fn default_retries() -> usize {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CampaignDoor {
    pub id: String,
    pub statement: String,
    pub missing: String,
    /// At least five concrete attack plans, not five decorative labels.
    pub perspectives: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClosedRoute {
    pub door_id: String,
    pub route: String,
    pub lesson: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckCommand {
    pub argv: Vec<String>,
    #[serde(default = "default_check_secs")]
    pub timeout_secs: u64,
}
fn default_check_secs() -> u64 {
    120
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ArtifactDigest {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CanonicalSource {
    pub path: String,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RoleEvidence {
    pub id: String,
    pub role: String,
    pub report_path: String,
    pub report_sha256: String,
    pub result_path: String,
    pub result_sha256: String,
    /// Immutable source copy; the adjacent source-manifest.json binds the check.
    pub workspace_path: String,
    pub artifacts: Vec<ArtifactDigest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DocumentEdit {
    pub path: String,
    pub old: String,
    pub new: String,
    pub before_sha256: String,
    pub after_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Correction {
    pub id: String,
    pub claim_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<CorrectionField>,
    pub old: String,
    pub new: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CorrectionField {
    Statement,
    Test,
    Lesson,
}

impl CorrectionField {
    fn key(self) -> &'static str {
        match self {
            Self::Statement => "statement",
            Self::Test => "test",
            Self::Lesson => "lesson",
        }
    }
}

/// Legacy reports omit the field. Accept them only when their exact OLD anchor
/// identifies one occurrence across the three permitted claim text fields.
pub(crate) fn apply_claim_correction(claim: &mut Value, fix: &Correction) -> Result<(), String> {
    if !bounded_text(&fix.old, 16_000) || !bounded_text(&fix.new, 16_000) {
        return Err("correction requires bounded OLD and NEW text".into());
    }
    let fields = [
        CorrectionField::Statement,
        CorrectionField::Test,
        CorrectionField::Lesson,
    ];
    let candidates: Vec<_> = fields
        .into_iter()
        .filter(|field| fix.field.is_none_or(|selected| selected == *field))
        .collect();
    let matches: Vec<_> = candidates
        .into_iter()
        .filter_map(|field| {
            claim[field.key()]
                .as_str()
                .map(|text| (field, text.matches(&fix.old).count()))
        })
        .collect();
    if matches.iter().map(|(_, count)| count).sum::<usize>() != 1 {
        return Err(
            "correction OLD must occur exactly once in its selected claim text field".into(),
        );
    }
    let field = matches
        .into_iter()
        .find(|(_, count)| *count == 1)
        .unwrap()
        .0;
    let text = claim[field.key()]
        .as_str()
        .unwrap()
        .replacen(&fix.old, &fix.new, 1);
    if !bounded_text(&text, 16_000) {
        return Err("corrected claim text exceeds its bound".into());
    }
    claim[field.key()] = json!(text);
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CheckReceipt {
    pub id: String,
    pub stage: String,
    pub workspace_path: String,
    pub command: Vec<String>,
    pub command_sha256: String,
    pub source_sha256: String,
    pub source_after_sha256: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub output_path: String,
    pub output_sha256: String,
    pub elapsed_ms: u128,
    pub code_path: Option<String>,
    pub code_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_after_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_artifacts: Vec<ArtifactDigest>,
    /// Set by this coordinator after a new execution, never parsed from model text.
    pub fresh: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct IntegrationBundle {
    pub schema: u32,
    pub id: String,
    pub campaign_id: String,
    pub task: String,
    pub door_id: String,
    pub coordinator_id: String,
    pub reviewed_at: String,
    pub literature: RoleEvidence,
    pub author: RoleEvidence,
    pub referee: RoleEvidence,
    pub writer_before: RoleEvidence,
    pub writer: RoleEvidence,
    pub coordinator: RoleEvidence,
    pub canonical_sources: Vec<CanonicalSource>,
    pub canonical_sources_path: String,
    pub canonical_sources_sha256: String,
    pub claims: Vec<Value>,
    pub nodes: Vec<Value>,
    pub sota: Vec<Value>,
    pub frontier: Option<Value>,
    pub edits: Vec<DocumentEdit>,
    pub corrections: Vec<Correction>,
    pub applied_corrections: Vec<String>,
    pub checks: Vec<CheckReceipt>,
}

pub(crate) trait CampaignIntegrator: Send + Sync {
    /// Trusted canonical-owner merge preview, materialized in isolated copies
    /// before any post-edit check. Models cannot provide these bytes.
    fn preview(&self, _workspace: &Path, _bundle: &IntegrationBundle) -> Result<Value, String> {
        Ok(json!({"files":[]}))
    }
    /// The canonical owner must independently check hashes, exact replacements,
    /// map consistency and tier evidence before publishing the transaction.
    fn integrate(
        &self,
        workspace: &Path,
        bundle: &IntegrationBundle,
        cancel: &AtomicBool,
    ) -> Result<Value, String>;
    /// Append-only progress hook; a role message never carries authority.
    fn event(&self, _workspace: &Path, _event: &Value) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct CampaignRuntime {
    pub literature: Arc<dyn Club>,
    pub attack: Arc<dyn Club>,
    pub referee: Arc<dyn Club>,
    pub writer: Arc<dyn Club>,
    pub integrator: Arc<dyn CampaignIntegrator>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RoleState {
    id: String,
    role: String,
    attempt: usize,
    state: String,
    outcome: Option<RoleOutput>,
    error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RoleOutput {
    evidence: RoleEvidence,
    report: Value,
    stop_reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct DoorState {
    door_id: String,
    attack: RoleState,
    referee: RoleState,
    writer: RoleState,
    state: String,
    corrected_claims: Vec<Value>,
    corrections: Vec<Correction>,
    checks: Vec<CheckReceipt>,
    integration: Option<Value>,
    error: Option<String>,
    #[serde(default)]
    referee_rechecks: usize,
    #[serde(default)]
    integration_rechecks: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CampaignState {
    schema: u32,
    id: String,
    sequence: usize,
    spec_sha256: String,
    run_id: String,
    status: String,
    round: usize,
    literature: RoleState,
    doors: Vec<DoorState>,
    leads: Vec<Value>,
    closed_routes: Vec<ClosedRoute>,
    integrations: Vec<Value>,
    escalations: Vec<String>,
    last_error: Option<String>,
    /// Committed with the state before its separately indexed event is exposed.
    #[serde(default)]
    last_event: Value,
}

fn bounded_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.len() <= max && !text.contains('\0')
}
fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn map_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 240 && !id.chars().any(char::is_control)
}
fn door_slug(id: &str) -> String {
    if identifier(id) && id.len() <= 48 {
        id.to_string()
    } else {
        format!("door-{}", &sha256_hex(id.as_bytes())[..24])
    }
}
fn source_path(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    if raw.is_empty()
        || raw.len() > 512
        || raw.chars().any(char::is_control)
        || raw.contains('\\')
        || raw.split('/').any(|part| matches!(part, "" | "." | ".."))
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || matches!(
            path.components()
                .next()
                .and_then(|c| c.as_os_str().to_str()),
            Some("artifacts" | ".campaign-checks" | ".git")
        )
        || raw.starts_with("labyrinth/angel/")
    {
        return Err(format!("invalid campaign source path: {raw}"));
    }
    Ok(path.to_path_buf())
}
fn artifact_path(raw: &str) -> Result<PathBuf, String> {
    let path = source_path(
        raw.strip_prefix("artifacts/")
            .ok_or("role artifacts must be under artifacts/")?,
    )?;
    Ok(Path::new("artifacts").join(path))
}
fn validate_command(command: &CheckCommand) -> Result<(), String> {
    if command.argv.is_empty()
        || command.argv.len() > 64
        || command
            .argv
            .iter()
            .any(|s| s.is_empty() || s.len() > 16_000 || s.contains('\0'))
        || !(1..=14_400).contains(&command.timeout_secs)
    {
        return Err("invalid bounded campaign check argv/deadline".into());
    }
    Ok(())
}
impl CampaignSpec {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !identifier(&self.id)
            || !bounded_text(&self.task, 16_000)
            || !(1..=3).contains(&self.doors.len())
            || !(1..=4).contains(&self.concurrency)
            || !(1..=128).contains(&self.max_hops)
            || !(1..=14_400).contains(&self.role_timeout_secs)
            || !(1..=86_400).contains(&self.campaign_timeout_secs)
            || !(1..=10).contains(&self.max_rounds)
            || self.writer_retries > 3
            || self.inputs.len() + self.canonical_documents.len() > MAX_FILES
            || self.checks.is_empty()
            || self.spot_checks.is_empty()
            || self.checks.len() + self.spot_checks.len() > 16
            || self.closed_routes.len() > 128
            || self.side_projects.len() > 32
            || !bounded_text(&self.compute_rules, 16_000)
        {
            return Err("invalid campaign objective, files, role limits or pinned checks".into());
        }
        let mut ids = BTreeSet::new();
        for door in &self.doors {
            if !map_id(&door.id)
                || !ids.insert(&door.id)
                || !bounded_text(&door.statement, 16_000)
                || !bounded_text(&door.missing, 8000)
                || !(5..=12).contains(&door.perspectives.len())
                || door.perspectives.iter().any(|s| !bounded_text(s, 4000))
            {
                return Err("each selected door needs a unique id, missing ingredient and five concrete perspectives".into());
            }
        }
        for path in self
            .inputs
            .iter()
            .chain(&self.referee_inputs)
            .chain(&self.canonical_documents)
            .chain(&self.side_projects)
        {
            source_path(path)?;
        }
        if self.referee_inputs.len() > MAX_FILES || self.canonical_documents.is_empty() {
            return Err(
                "a campaign needs bounded referee definitions/raw data and canonical documents"
                    .into(),
            );
        }
        for route in &self.closed_routes {
            if !ids.contains(&route.door_id)
                || !bounded_text(&route.route, 2000)
                || !bounded_text(&route.lesson, 4000)
            {
                return Err("closed routes need a selected door and a lesson".into());
            }
        }
        for command in self.checks.iter().chain(&self.spot_checks) {
            validate_command(command)?;
        }
        Ok(())
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn base(id: &str) -> PathBuf {
    Path::new("labyrinth/angel/campaigns").join(id)
}
fn read(root: &Path, rel: &Path, max: usize) -> Result<Vec<u8>, String> {
    confined_read_limited_no_symlinks(root, rel, max)?
        .ok_or_else(|| format!("{} exceeds campaign byte budget", rel.display()))
}
fn publish(root: &Path, rel: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::symlink_metadata(root.join(rel)).is_ok() {
        if read(root, rel, bytes.len().max(1))? == bytes {
            return Ok(());
        }
        return Err(format!(
            "immutable campaign artifact already differs: {}",
            rel.display()
        ));
    }
    confined_publish_new_no_symlinks(root, rel, bytes)
}
fn publish_json(root: &Path, rel: &Path, value: &impl Serialize) -> Result<(), String> {
    publish(
        root,
        rel,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}
fn role(_id: &str, door: &str, name: &str, round: usize, attempt: usize) -> RoleState {
    RoleState {
        id: format!("{name}-{}-r{round}-a{attempt}", door_slug(door)),
        role: name.into(),
        attempt,
        state: "pending".into(),
        outcome: None,
        error: None,
    }
}
fn new_doors(spec: &CampaignSpec, round: usize) -> Vec<DoorState> {
    spec.doors
        .iter()
        .map(|d| DoorState {
            door_id: d.id.clone(),
            attack: role(&spec.id, &d.id, "attack", round, 0),
            referee: role(&spec.id, &d.id, "referee", round, 0),
            writer: role(&spec.id, &d.id, "writer", round, 0),
            state: "pending".into(),
            corrected_claims: Vec::new(),
            corrections: Vec::new(),
            checks: Vec::new(),
            integration: None,
            error: None,
            referee_rechecks: 0,
            integration_rechecks: 0,
        })
        .collect()
}
macro_rules! checkpoint {
    ($root:expr,$state:ident,$event:expr) => {{
        let event = $event;
        save($root, &mut $state, event)
    }};
}

fn journal_sequence(root: &Path, id: &str, directory: &str) -> Result<usize, String> {
    let rel = base(id).join(directory);
    if !root.join(&rel).exists() {
        return Ok(0);
    }
    let entries = confined_read_dir(root, &rel)?;
    if entries.len() > MAX_STATES {
        return Err("campaign journal entry budget exceeded".into());
    }
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let name = entry.name.to_str()?;
            (name.len() == 11
                && name.ends_with(".json")
                && name[..6].bytes().all(|b| b.is_ascii_digit()))
            .then(|| name[..6].parse::<usize>().ok())
            .flatten()
        })
        .max()
        .unwrap_or(0))
}
fn save(root: &Path, state: &mut CampaignState, event: Value) -> Result<(), String> {
    // Older coordinators exposed an event before its corresponding state. Skip
    // such orphan indices without deleting or rewriting append-only history.
    state.sequence = state
        .sequence
        .max(journal_sequence(root, &state.id, "events")?)
        .max(journal_sequence(root, &state.id, "states")?)
        + 1;
    if state.sequence >= MAX_STATES {
        return Err("campaign journal state budget exhausted".into());
    }
    let event = json!({"schema":1,"campaign":state.id,"sequence":state.sequence,"time_ms":now_ms(),"event":event});
    state.last_event = event.clone();
    publish_json(
        root,
        &base(&state.id).join(format!("states/{:06}.json", state.sequence)),
        state,
    )?;
    publish_json(
        root,
        &base(&state.id).join(format!("events/{:06}.json", state.sequence)),
        &event,
    )
}
fn load_state(root: &Path, id: &str) -> Result<(CampaignSpec, CampaignState), String> {
    if !identifier(id) {
        return Err("invalid campaign id".into());
    }
    let bytes = read(root, &base(id).join("spec.json"), 2 * 1024 * 1024)?;
    let spec: CampaignSpec = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    spec.validate()?;
    let entries = confined_read_dir(root, &base(id).join("states"))?;
    if entries.len() > MAX_STATES {
        return Err("campaign state budget exceeded".into());
    }
    let latest = entries
        .into_iter()
        .filter_map(|e| e.name.to_str().map(str::to_string))
        .filter(|s| {
            s.len() == 11 && s.ends_with(".json") && s[..6].bytes().all(|b| b.is_ascii_digit())
        })
        .max()
        .ok_or("campaign has no durable state")?;
    let state: CampaignState = serde_json::from_slice(&read(
        root,
        &base(id).join("states").join(latest),
        8 * 1024 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    if state.spec_sha256 != sha256_hex(&bytes) || state.id != id {
        return Err("campaign spec/state identity mismatch".into());
    }
    if !state.last_event.is_null() {
        if state.last_event["sequence"] != state.sequence || state.last_event["campaign"] != id {
            return Err("campaign committed event/state identity mismatch".into());
        }
        // A crash after state publication is repaired from the exact committed
        // event, including its original timestamp, rather than inventing one.
        publish_json(
            root,
            &base(id).join(format!("events/{:06}.json", state.sequence)),
            &state.last_event,
        )?;
    }
    Ok((spec, state))
}

pub(crate) fn start(workspace: &Path, spec: CampaignSpec) -> Result<Value, String> {
    spec.validate()?;
    super::initialize(workspace)?;
    let _lock = runner_lock(workspace, &spec.id)?;
    let spec_bytes = serde_json::to_vec_pretty(&spec).map_err(|e| e.to_string())?;
    let spec_path = base(&spec.id).join("spec.json");
    if std::fs::symlink_metadata(workspace.join(&spec_path)).is_ok() {
        if read(workspace, &spec_path, 2 * 1024 * 1024)? != spec_bytes {
            return Err("campaign id already has a different immutable specification".into());
        }
        if journal_sequence(workspace, &spec.id, "states")? > 0 {
            return status(workspace, Some(&spec.id));
        }
    }
    // Validate/capture the briefing files before admitting any model calls.
    let mut names: BTreeSet<String> = spec
        .inputs
        .iter()
        .chain(&spec.referee_inputs)
        .chain(&spec.canonical_documents)
        .chain(&spec.side_projects)
        .cloned()
        .collect();
    let mut total = 0usize;
    for name in std::mem::take(&mut names) {
        let baseline = base(&spec.id).join("baseline").join(&name);
        let bytes = if workspace.join(&baseline).exists() {
            read(workspace, &baseline, FILE_BYTES)?
        } else {
            read(workspace, &source_path(&name)?, FILE_BYTES)?
        };
        total = total
            .checked_add(bytes.len())
            .ok_or("campaign snapshot overflow")?;
        if total > SNAPSHOT_BYTES {
            return Err("campaign snapshot byte budget exceeded".into());
        }
        publish(
            workspace,
            &base(&spec.id).join("baseline").join(&name),
            &bytes,
        )?;
    }
    publish(workspace, &spec_path, &spec_bytes)?;
    let canonical_receipt = base(&spec.id).join("canonical-sources.json");
    if !workspace.join(&canonical_receipt).exists() {
        capture_canonical(workspace, &canonical_receipt)?;
    }
    let mut state = CampaignState {
        schema: 1,
        id: spec.id.clone(),
        sequence: 0,
        spec_sha256: sha256_hex(&spec_bytes),
        run_id: format!("{}-{}", std::process::id(), now_ms()),
        status: "ready".into(),
        round: 0,
        literature: role(&spec.id, "all", "literature", 0, 0),
        doors: new_doors(&spec, 0),
        leads: Vec::new(),
        closed_routes: spec.closed_routes.clone(),
        integrations: Vec::new(),
        escalations: Vec::new(),
        last_error: None,
        last_event: Value::Null,
    };
    checkpoint!(
        workspace,
        state,
        json!({"type":"campaign-created","doors":spec.doors.iter().map(|d|&d.id).collect::<Vec<_>>()})
    )?;
    serde_json::to_value(state).map_err(|e| e.to_string())
}

pub(crate) fn status(workspace: &Path, id: Option<&str>) -> Result<Value, String> {
    if let Some(id) = id {
        return serde_json::to_value(load_state(workspace, id)?.1).map_err(|e| e.to_string());
    }
    let root = Path::new("labyrinth/angel/campaigns");
    if !workspace.join(root).exists() {
        return Ok(json!({"campaigns":[]}));
    }
    let entries = confined_read_dir(workspace, root)?;
    if entries.len() > 128 {
        return Err("campaign catalog budget exceeded".into());
    }
    let mut campaigns = Vec::new();
    for entry in entries {
        let Some(id) = entry.name.to_str() else {
            continue;
        };
        if !entry.is_dir || !identifier(id) {
            continue;
        }
        let (_, state) = load_state(workspace, id)?;
        campaigns.push(json!({"id":id,"status":state.status,"round":state.round,"integrations":state.integrations.len(),"last_error":state.last_error}));
    }
    campaigns.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(json!({"campaigns":campaigns}))
}

pub(crate) fn cancel(workspace: &Path, id: &str) -> Result<Value, String> {
    let (_, state) = load_state(workspace, id)?;
    publish_json(
        workspace,
        &base(id)
            .join("signals")
            .join(format!("{}.json", state.run_id)),
        &json!({"cancel":true,"run_id":state.run_id}),
    )?;
    Ok(json!({"id":id,"cancel_requested":true,"run_id":state.run_id}))
}

struct RunnerLock(std::fs::File);
fn runner_lock(root: &Path, id: &str) -> Result<RunnerLock, String> {
    let rel = base(id).join("runner.lock");
    publish(root, &rel, b"")?;
    let file = confined_open_read_no_symlinks(root, &rel)?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // Advisory lock ownership is released by the kernel after a crash.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("this campaign already has an active coordinator".into());
        }
    }
    #[cfg(not(unix))]
    {
        return Err("campaign coordination needs descriptor locks on this platform".into());
    }
    Ok(RunnerLock(file))
}
impl Drop for RunnerLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            unsafe {
                libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}

fn role_work(id: &str, role: &str) -> PathBuf {
    base(id).join("workspaces").join(role)
}
fn role_archive(id: &str, role: &str) -> PathBuf {
    base(id).join("agents").join(role)
}

fn workflow_briefs(role: &str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = vec![
        (
            "labyrinth/workflow/SKILL.md",
            include_bytes!("../../../research/labyrinth/upstream/SKILL.md").as_slice(),
        ),
        (
            "labyrinth/workflow/references/loop.md",
            include_bytes!("../../../research/labyrinth/upstream/references/loop.md").as_slice(),
        ),
        (
            "labyrinth/workflow/references/campaigns.md",
            include_bytes!("../../../research/labyrinth/upstream/references/campaigns.md")
                .as_slice(),
        ),
        (
            "labyrinth/workflow/references/compute.md",
            include_bytes!("../../../research/labyrinth/upstream/references/compute.md").as_slice(),
        ),
        (
            "labyrinth/workflow/references/saturation.md",
            include_bytes!("../../../research/labyrinth/upstream/references/saturation.md")
                .as_slice(),
        ),
        (
            "labyrinth/workflow/references/schema.md",
            include_bytes!("../../../research/labyrinth/upstream/references/schema.md").as_slice(),
        ),
    ];
    match role {
        "attack" => files.push((
            "labyrinth/workflow/templates/briefs/attack.md",
            include_bytes!("../../../research/labyrinth/upstream/templates/briefs/attack.md")
                .as_slice(),
        )),
        "referee" => files.push((
            "labyrinth/workflow/templates/briefs/referee.md",
            include_bytes!("../../../research/labyrinth/upstream/templates/briefs/referee.md")
                .as_slice(),
        )),
        "writer" => files.push((
            "labyrinth/workflow/templates/briefs/writer.md",
            include_bytes!("../../../research/labyrinth/upstream/templates/briefs/writer.md")
                .as_slice(),
        )),
        _ => {}
    }
    files
}

fn capture_canonical(root: &Path, receipt: &Path) -> Result<Vec<CanonicalSource>, String> {
    let mut sources = Vec::new();
    for path in [
        "labyrinth/knowledge.json",
        "labyrinth/sota.json",
        "labyrinth/frontier.json",
    ] {
        let sha = if std::fs::symlink_metadata(root.join(path)).is_ok() {
            let bytes = read(root, Path::new(path), FILE_BYTES)?;
            publish(
                root,
                &receipt
                    .parent()
                    .unwrap()
                    .join("canonical-baseline")
                    .join(path),
                &bytes,
            )?;
            Some(sha256_hex(&bytes))
        } else {
            None
        };
        sources.push(CanonicalSource {
            path: path.into(),
            sha256: sha,
        });
    }
    publish_json(root, receipt, &sources)?;
    Ok(sources)
}

fn list_files(
    root: &Path,
    rel: &Path,
    files: &mut Vec<PathBuf>,
    exclude_runtime: bool,
) -> Result<(), String> {
    if files.len() > MAX_FILES {
        return Err("role file-count budget exceeded".into());
    }
    for entry in confined_read_dir(root, rel)? {
        let name = entry.name.to_str().ok_or("non-UTF-8 role filename")?;
        if exclude_runtime
            && rel.as_os_str().is_empty()
            && (name == "artifacts"
                || name == ".campaign-checks"
                || name == SOURCE_INDEX
                || name == ".git"
                || name.starts_with(".angel"))
        {
            continue;
        }
        let path = rel.join(name);
        // The metadata check rejects aliases even before the confined open.
        let meta = std::fs::symlink_metadata(root.join(&path)).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(format!("role symlink is not admitted: {}", path.display()));
        }
        if entry.is_dir {
            list_files(root, &path, files, exclude_runtime)?;
        } else if meta.is_file() {
            files.push(path);
        } else {
            return Err("role artifact is not a regular file".into());
        }
    }
    if files.len() > MAX_FILES {
        return Err("role file-count budget exceeded".into());
    }
    Ok(())
}
fn source_manifest(root: &Path) -> Result<(String, Vec<(String, String)>), String> {
    // Freeze the supplied input names once. Build products do not become new
    // trusted inputs merely because a checker created them in its private copy.
    let mut paths: Vec<PathBuf> = if root.join(SOURCE_INDEX).exists() {
        let names: Vec<String> =
            serde_json::from_slice(&read(root, Path::new(SOURCE_INDEX), REPORT_BYTES)?)
                .map_err(|e| e.to_string())?;
        if names.len() > MAX_FILES {
            return Err("role source inventory exceeds file budget".into());
        }
        names
            .iter()
            .map(|name| source_path(name))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut paths = Vec::new();
        list_files(root, Path::new(""), &mut paths, true)?;
        paths.sort();
        publish_json(
            root,
            Path::new(SOURCE_INDEX),
            &paths
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
        )?;
        paths
    };
    paths.sort();
    if paths.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("role source inventory contains aliases".into());
    }
    let mut total = 0usize;
    let mut items = Vec::new();
    for path in paths {
        let bytes = read(root, &path, FILE_BYTES)?;
        total += bytes.len();
        if total > SNAPSHOT_BYTES {
            return Err("role source byte budget exceeded".into());
        }
        items.push((path.to_string_lossy().into_owned(), sha256_hex(&bytes)));
    }
    let digest = sha256_hex(&serde_json::to_vec(&items).map_err(|e| e.to_string())?);
    Ok((digest, items))
}
fn prepare_role(root: &Path, spec: &CampaignSpec, role: &RoleState) -> Result<PathBuf, String> {
    let rel = role_work(&spec.id, &role.id);
    let names: BTreeSet<_> = if role.role == "referee" {
        spec.referee_inputs.iter().cloned().collect()
    } else {
        spec.inputs
            .iter()
            .chain(&spec.canonical_documents)
            .chain(&spec.side_projects)
            .cloned()
            .collect()
    };
    // A new marker makes even an empty referee definition set an owned directory.
    publish(root, &rel.join("artifacts/.owner"), role.id.as_bytes())?;
    for name in names {
        let bytes = if matches!(role.role.as_str(), "writer" | "coordinator")
            && spec.canonical_documents.contains(&name)
        {
            read(root, Path::new(&name), FILE_BYTES)?
        } else {
            read(
                root,
                &base(&spec.id).join("baseline").join(&name),
                FILE_BYTES,
            )?
        };
        publish(root, &rel.join(name), &bytes)?;
    }
    for (path, embedded) in workflow_briefs(&role.role) {
        let bytes = if root.join(path).exists() {
            read(root, Path::new(path), FILE_BYTES)?
        } else {
            embedded.to_vec()
        };
        publish(root, &rel.join(path), &bytes)?;
    }
    if role.role != "referee" {
        for path in [
            "labyrinth/knowledge.json",
            "labyrinth/sota.json",
            "labyrinth/frontier.json",
        ] {
            if root.join(&rel).join(path).exists() {
                continue;
            }
            let source = if matches!(role.role.as_str(), "writer" | "coordinator") {
                PathBuf::from(path)
            } else {
                base(&spec.id).join("canonical-baseline").join(path)
            };
            if root.join(&source).exists() {
                publish(root, &rel.join(path), &read(root, &source, FILE_BYTES)?)?;
            }
        }
    }
    if role.role == "writer" {
        capture_canonical(
            root,
            &role_archive(&spec.id, &role.id).join("canonical-sources.json"),
        )?;
    }
    Ok(root.join(rel))
}

/// Strict role-local file capability. Writing a canonical input is impossible
/// through this tool; drafts, own-code and data are confined to artifacts/.
struct RoleFiles {
    root: PathBuf,
}
impl Tool for RoleFiles {
    fn name(&self) -> &str {
        "campaign_file"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().into(),
            description: chapter::FILES.cells(),
            params: json!({"type":"object","properties":{"action":{"enum":["read","write","list"]},"path":{"type":"string"},"text":{"type":"string"},"offset":{"type":"integer","minimum":0},"max_bytes":{"type":"integer","minimum":1,"maximum":8192}},"required":["action","path"],"additionalProperties":false}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let path = args["path"].as_str().ok_or("campaign_file needs path")?;
        match args["action"].as_str() {
            Some("write") => {
                let rel = artifact_path(path)?;
                let text = args["text"].as_str().ok_or("write needs text")?;
                if text.len() > REPORT_BYTES {
                    return Err("role write byte budget exceeded".into());
                }
                // New files use strict publication. Existing owned artifacts may
                // be rewritten, after rejecting every alias component by a read.
                if self.root.join(&rel).exists() {
                    read(&self.root, &rel, FILE_BYTES)?;
                    confined_write(&self.root, &rel, text.as_bytes())?;
                } else {
                    publish(&self.root, &rel, text.as_bytes())?;
                }
                Ok("owned artifact written".into())
            }
            Some("read") => {
                let rel = if path.starts_with("artifacts/") {
                    artifact_path(path)?
                } else {
                    source_path(path)?
                };
                let text = String::from_utf8(read(&self.root, &rel, FILE_BYTES)?)
                    .map_err(|e| e.to_string())?;
                let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
                if offset > text.len() || !text.is_char_boundary(offset) {
                    return Err("read offset must be an in-range UTF-8 byte boundary".into());
                }
                let limit = args
                    .get("max_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or(8192) as usize;
                if !(1..=8192).contains(&limit) {
                    return Err("read max_bytes must be between 1 and 8192".into());
                }
                if offset == 0 && text.len() <= limit && args.get("offset").is_none() {
                    return Ok(text);
                }
                let mut end = offset.saturating_add(limit).min(text.len());
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                if end == offset && offset < text.len() {
                    return Err("read max_bytes must cover the next UTF-8 character".into());
                }
                let next = (end < text.len()).then_some(end);
                Ok(format!(
                    "{}\n{}",
                    json!({"read_slice":{"path":path,"offset":offset,"next_offset":next,"total_bytes":text.len(),"sha256":sha256_hex(text.as_bytes())}}),
                    &text[offset..end]
                ))
            }
            Some("list") => {
                let rel = if path == "." {
                    PathBuf::new()
                } else if path == "artifacts" {
                    PathBuf::from("artifacts")
                } else if path.starts_with("artifacts/") {
                    artifact_path(path)?
                } else {
                    source_path(path)?
                };
                let names = confined_read_dir(&self.root, &rel)?
                    .into_iter()
                    .take(MAX_FILES)
                    .map(|e| e.name.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                Ok(json!(names).to_string())
            }
            _ => Err("unknown campaign_file action".into()),
        }
    }
}

struct RoleExec {
    root: PathBuf,
    timeout: Duration,
}
impl Tool for RoleExec {
    fn name(&self) -> &str {
        "campaign_exec"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().into(),
            description: chapter::EXEC.cells(),
            params: json!({"type":"object","properties":{"argv":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":64}},"required":["argv"],"additionalProperties":false}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }
    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        let argv: Vec<String> =
            serde_json::from_value(args["argv"].clone()).map_err(|e| e.to_string())?;
        validate_command(&CheckCommand {
            argv: argv.clone(),
            timeout_secs: self.timeout.as_secs().max(1).min(1800),
        })?;
        let policy = role_policy(&self.root, false)?;
        let args: Vec<&str> = argv[1..].iter().map(String::as_str).collect();
        let observation = campaign_subprocess(self.timeout, cancel, || {
            run_sandboxed_observed_cancellable(&argv[0], &args, Some(&self.root), &policy, cancel)
        })?;
        Ok(json!({"exit_code":observation.exit,"timed_out":observation.timed_out,"cancelled":observation.cancelled,"output":observation.output}).to_string())
    }
}
static SUBPROCESS: Mutex<()> = Mutex::new(());
struct SubprocessPermit {
    _guard: std::sync::MutexGuard<'static, ()>,
}
impl SubprocessPermit {
    fn acquire(deadline: Instant, cancel: Option<&AtomicBool>) -> Result<Self, String> {
        loop {
            if cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire)) {
                return Err("campaign subprocess admission cancelled before execution".into());
            }
            if Instant::now() >= deadline {
                return Err(
                    "campaign subprocess admission deadline exhausted before execution".into(),
                );
            }
            match SUBPROCESS.try_lock() {
                Ok(permit) => return Ok(Self { _guard: permit }),
                Err(std::sync::TryLockError::Poisoned(permit)) => {
                    return Ok(Self {
                        _guard: permit.into_inner(),
                    });
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(10))
                }
            }
        }
    }
}
fn campaign_subprocess<T>(
    timeout: Duration,
    cancel: Option<&AtomicBool>,
    execute: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let timeout =
        crate::agent::harness::call_budget().map_or(timeout, |budget| budget.min(timeout));
    let deadline = Instant::now() + timeout;
    let _permit = SubprocessPermit::acquire(deadline, cancel)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err("campaign subprocess deadline exhausted at admission".into());
    }
    with_call_budget(Some(remaining), execute)
}
fn role_policy(root: &Path, check: bool) -> Result<SandboxPolicy, String> {
    // Per-role S02: host secrets and sibling author/referee work have no read
    // grant. A parent/global sealed profile would still expose the parent repo.
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut policy = crate::agent::sandbox::sealed::build(&canonical, None).policy;
    let project = canonical
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "labyrinth"))
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    policy.sealed_reads.retain(|read| {
        if *read == canonical {
            return true;
        }
        !project
            .as_ref()
            .is_some_and(|project| read.starts_with(project) || project.starts_with(read))
    });
    policy.writable_roots = vec![canonical.join("artifacts")];
    if check {
        let (_, files) = source_manifest(root)?;
        let sources: Vec<PathBuf> = files.iter().map(|(name, _)| PathBuf::from(name)).collect();
        let mut outputs = BTreeSet::from([
            PathBuf::from(".campaign-checks"),
            PathBuf::from("target"),
            PathBuf::from("build"),
            PathBuf::from("_build"),
        ]);
        for source in &sources {
            let parent = source.parent().unwrap_or(Path::new(""));
            if source.file_name().is_some_and(|name| name == "Cargo.toml") {
                outputs.insert(parent.join("target"));
            }
        }
        // Cache markers beside arbitrary inputs change exact candidate-tree
        // membership before its checker runs. Python imports can run without
        // bytecode caches; owned artifacts and derived build outputs remain
        // writable without adding files under copied source directories.
        for output in outputs {
            if sources.iter().any(|source| source.starts_with(&output)) {
                continue;
            }
            // Strict native creation rejects aliases. Inputs remain read-only
            // for the entire process, including transient rewrite-and-restore.
            publish(
                root,
                &output.join(".campaign-owner"),
                b"bounded check output directory",
            )?;
            policy.writable_roots.push(canonical.join(output));
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        policy
            .deny_reads
            .extend([home.join(".aws"), home.join(".codex"), home.join(".agents")]);
    }
    Ok(policy)
}

/// A nominal argument match is insufficient: `true code.py` never executes
/// code.py. Referee checks must directly run their retained program or pass it
/// as the interpreter's first actual script argument, without eval wrappers.
pub(crate) fn validate_independent_command(argv: &[String], code: &str) -> Result<(), String> {
    artifact_path(code)?;
    if argv.is_empty() {
        return Err("independent check has no executable argv".into());
    }
    if argv[0] == code {
        return Ok(());
    }
    let program = Path::new(&argv[0]);
    if program.components().count() > 1 && !program.is_absolute() {
        return Err("independent interpreter cannot be a role-relative shim".into());
    }
    let name = program
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid independent interpreter")?;
    let resolved = if program.is_absolute() {
        program.to_path_buf()
    } else {
        let path = crate::agent::harness::sandbox_command_path()
            .ok_or("independent interpreter PATH is unavailable")?;
        std::env::split_paths(&path)
            .map(|dir| dir.join(program))
            .find(|path| path.is_file())
            .ok_or("independent interpreter does not resolve to a file")?
    }
    .canonicalize()
    .map_err(|e| format!("independent interpreter resolution failed: {e}"))?;
    let trusted = ["/usr", "/bin", "/sbin", "/opt"]
        .iter()
        .any(|prefix| resolved.starts_with(prefix))
        || std::env::var_os("HOME")
            .map(PathBuf::from)
            .is_some_and(|home| resolved.starts_with(home.join(".nvm")));
    let role_owned = resolved.ancestors().any(|parent| {
        parent.file_name().is_some_and(|name| name == "campaigns")
            && parent.parent().is_some_and(|parent| {
                parent.file_name().is_some_and(|name| name == "angel")
                    && parent.parent().is_some_and(|parent| {
                        parent.file_name().is_some_and(|name| name == "labyrinth")
                    })
            })
    });
    if !trusted || role_owned {
        return Err("independent interpreter must resolve to a read-only runtime outside every role workspace".into());
    }
    let safe_flags: &[&str] = if name == "python"
        || name == "python3"
        || name
            .strip_prefix("python3.")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        &["-I", "-B", "-u", "-E", "-s", "-S", "-O", "-OO", "-q"]
    } else if matches!(name, "bash" | "sh") {
        &["-e", "-u", "-x", "--noprofile", "--norc"]
    } else if name == "node" {
        &[]
    } else if name == "ruby" {
        &["-w", "--disable-gems"]
    } else {
        return Err(
            "independent check must directly execute own code with a supported interpreter".into(),
        );
    };
    let mut index = 1;
    while index < argv.len() && safe_flags.contains(&argv[index].as_str()) {
        index += 1;
    }
    if name.starts_with("python")
        && (!argv[1..index].iter().any(|flag| flag == "-I")
            || !argv[1..index].iter().any(|flag| flag == "-S"))
    {
        return Err("independent Python checks require -I -S startup isolation".into());
    }
    if argv.get(index).is_none_or(|argument| argument != code) {
        return Err("independent check must execute retained code as the first script argument; eval/module/check-only wrappers are refused".into());
    }
    Ok(())
}

type Leads = Arc<Mutex<Vec<Value>>>;
/// The existing GET-only connector, with immutable role-local provenance.
/// Recording retrieval never upgrades the truth of a research claim.
struct RetainedRead {
    inner: Box<dyn Tool>,
    canonical: PathBuf,
    work: PathBuf,
    archive: PathBuf,
    requests: Arc<AtomicUsize>,
}
impl Tool for RetainedRead {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn def(&self) -> ToolDef {
        let mut definition = self.inner.def();
        definition.description = format!(
            "{}\n{}",
            chapter::LITERATURE.cells(),
            if self.name() == "web_search" {
                "⠱⠁"
            } else {
                "⠱⠃"
            }
        );
        definition
    }
    fn requires(&self) -> &[crate::agent::harness::coeffect::Key] {
        self.inner.requires()
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }
    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        if cancel.is_some_and(|signal| signal.load(Ordering::Acquire)) {
            return Err("literature retrieval cancelled before dispatch".into());
        }
        let request = serde_json::to_vec_pretty(args).map_err(|e| e.to_string())?;
        if request.len() > 16_000 {
            return Err("literature retrieval request byte budget exceeded".into());
        }
        let index = self
            .requests
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < 32).then_some(current + 1)
            })
            .map_err(|_| "literature retrieval count budget exhausted")?;
        let result = self.inner.call_with_cancel(args, cancel);
        let (response, succeeded) = match result {
            Ok(response) => (response, true),
            Err(error) => (error, false),
        };
        if response.len() > 256_000 {
            return Err("literature retrieval response byte budget exceeded".into());
        }
        let prefix = self
            .archive
            .join("retrievals")
            .join(format!("{}-{index:03}", self.name()));
        let request_path = prefix.join("request.json");
        let response_path = prefix.join("response.txt");
        let receipt_path = prefix.join("receipt.json");
        publish(&self.canonical, &request_path, &request)?;
        publish(&self.canonical, &response_path, response.as_bytes())?;
        let http_status = if self.name() == "web_fetch" {
            response
                .strip_prefix('[')
                .and_then(|text| text.split_whitespace().next())
                .and_then(|status| status.parse::<u16>().ok())
        } else {
            None
        };
        let source_checked =
            succeeded && http_status.is_some_and(|status| (200..300).contains(&status));
        let receipt = json!({"schema":1,"tool":self.name(),"request_path":request_path,"request_sha256":sha256_hex(&request),"response_path":response_path,"response_sha256":sha256_hex(response.as_bytes()),"succeeded":succeeded,"http_status":http_status,"source_checked":source_checked,"claim_verified":false});
        let bytes = serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?;
        publish(&self.canonical, &receipt_path, &bytes)?;
        // Reopening a fetched source uses this actor's own file capability;
        // authority remains bound to the immutable canonical receipt above.
        let local = Path::new("artifacts/retrievals").join(format!("{}-{index:03}", self.name()));
        for (name, content) in [
            ("request.json", request.as_slice()),
            ("response.txt", response.as_bytes()),
            ("receipt.json", bytes.as_slice()),
        ] {
            publish(&self.work, &local.join(name), content)?;
        }
        let payload=json!({"text":response,"retrieval_receipt":{"path":receipt_path,"sha256":sha256_hex(&bytes)},"local_response_path":local.join("response.txt"),"source_checked":source_checked,"claim_verified":false}).to_string();
        if succeeded { Ok(payload) } else { Err(payload) }
    }
}

struct LeadTool {
    campaign_root: PathBuf,
    archive: PathBuf,
    mailbox: Leads,
}
impl Tool for LeadTool {
    fn name(&self) -> &str {
        "campaign_lead"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().into(),
            description: chapter::LEADS.cells(),
            params: json!({"type":"object","properties":{"id":{"type":"string"},"doors":{"type":"array","items":{"type":"string"}},"statement":{"type":"string"},"source":{"type":"string"},"source_url":{"type":"string"},"source_receipt":{"type":"object","properties":{"path":{"type":"string"},"sha256":{"type":"string"}},"required":["path","sha256"],"additionalProperties":false}},"required":["id","doors","statement","source"],"additionalProperties":false}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        validate_lead(args)?;
        let mut lead = args.clone();
        lead["source_checked"] = json!(false);
        lead["claim_verified"] = json!(false);
        if let Some(reference) = args.get("source_receipt") {
            let path = reference["path"]
                .as_str()
                .ok_or("literature source receipt requires a path")?;
            let digest = reference["sha256"]
                .as_str()
                .ok_or("literature source receipt requires a SHA")?;
            let rel = Path::new(path);
            if path.split('/').any(|part| matches!(part, "" | "." | ".."))
                || path.contains('\\')
                || path.chars().any(char::is_control)
                || !rel.starts_with(self.archive.join("retrievals"))
                || rel.file_name().is_none_or(|name| name != "receipt.json")
            {
                return Err(
                    "source receipt is outside this literature role's immutable retrievals".into(),
                );
            }
            let bytes = read(&self.campaign_root, rel, REPORT_BYTES)?;
            if sha256_hex(&bytes) != digest {
                return Err("literature retrieval receipt changed".into());
            }
            let receipt: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let request_path = receipt["request_path"]
                .as_str()
                .ok_or("retrieval has no request")?;
            let response_path = receipt["response_path"]
                .as_str()
                .ok_or("retrieval has no response")?;
            let prefix = rel.parent().unwrap();
            if Path::new(request_path).parent() != Some(prefix)
                || Path::new(response_path).parent() != Some(prefix)
            {
                return Err("retrieval request/response ownership differs".into());
            }
            let request = read(&self.campaign_root, Path::new(request_path), REPORT_BYTES)?;
            let response = read(&self.campaign_root, Path::new(response_path), 256_000)?;
            if receipt["request_sha256"] != sha256_hex(&request)
                || receipt["response_sha256"] != sha256_hex(&response)
            {
                return Err("retrieval source bytes changed".into());
            }
            let request: Value = serde_json::from_slice(&request).map_err(|e| e.to_string())?;
            let source = args
                .get("source_url")
                .and_then(Value::as_str)
                .or_else(|| args["source"].as_str())
                .unwrap_or_default();
            if receipt["tool"] == "web_fetch"
                && receipt["source_checked"] == true
                && request["url"] == source
            {
                lead["source_checked"] = json!(true);
                lead["source_excerpt"] = json!(
                    String::from_utf8(response)
                        .map_err(|e| e.to_string())?
                        .chars()
                        .take(4000)
                        .collect::<String>()
                );
            }
        }
        let digest = sha256_hex(lead.to_string().as_bytes());
        publish_json(
            &self.campaign_root,
            &self.archive.join("leads").join(format!("{digest}.json")),
            &lead,
        )?;
        let mut leads = self.mailbox.lock().unwrap_or_else(|e| e.into_inner());
        if !leads.contains(&lead) {
            if leads.len() >= MAX_LEADS {
                return Err("literature lead budget exceeded".into());
            }
            leads.push(lead);
        }
        Ok("lead archived and forwarded as unverified source material".into())
    }
}
fn validate_lead(lead: &Value) -> Result<(), String> {
    if lead["id"].as_str().is_none_or(|s| !map_id(s))
        || lead["statement"]
            .as_str()
            .is_none_or(|s| !bounded_text(s, 8000))
        || lead["source"]
            .as_str()
            .is_none_or(|s| !bounded_text(s, 4000))
        || lead["doors"].as_array().is_none_or(|a| {
            a.is_empty() || a.len() > 3 || a.iter().any(|d| d.as_str().is_none_or(|s| !map_id(s)))
        })
    {
        return Err("invalid literature lead".into());
    }
    Ok(())
}

/// Literature arrives between model hops without inheriting author reasoning
/// into the referee. Streaming/effort/cancellation remain the inner Club's.
struct LeadClub {
    inner: Arc<dyn Club>,
    mailbox: Leads,
    door: String,
}
impl LeadClub {
    fn messages(&self, messages: &[ChatMsg]) -> Vec<ChatMsg> {
        let mut copy = messages.to_vec();
        let leads = self
            .mailbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|lead| {
                lead["doors"]
                    .as_array()
                    .is_some_and(|doors| doors.iter().any(|d| d == &self.door))
            })
            .cloned()
            .collect::<Vec<_>>();
        if !leads.is_empty() {
            copy.push(ChatMsg::harness(format!(
                "{}\n{}",
                book::sign_lines(&[chapter::LITERATURE, chapter::DATA]),
                json!({"literature_leads":leads})
            )));
        }
        copy
    }
}
impl Club for LeadClub {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        self.inner.respond(prompt)
    }
    fn label(&self) -> &str {
        self.inner.label()
    }
    fn route_identity(&self) -> RouteIdentity {
        self.inner.route_identity()
    }
    fn model_identity(&self) -> Option<String> {
        self.inner.model_identity()
    }
    fn supports_formation_budget(&self) -> bool {
        self.inner.supports_formation_budget()
    }
    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.inner.chat(&self.messages(messages), tools)
    }
    fn chat_streaming_captured(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        origin: Option<(&str, &'static str)>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.inner.chat_streaming_captured(
            &self.messages(messages),
            tools,
            effort,
            origin,
            cancel,
            on_delta,
        )
    }
}

fn role_registry(
    root: &Path,
    role: &str,
    spec: &CampaignSpec,
    canonical: &Path,
    role_id: &str,
    leads: &Leads,
) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.set_workspace(root.to_path_buf());
    registry.register(Box::new(book::connect::LedgerReader {
        workspace: root.to_path_buf(),
    }));
    registry.register(Box::new(RoleFiles {
        root: root.to_path_buf(),
    }));
    if role == "literature" {
        let requests = Arc::new(AtomicUsize::new(0));
        if crate::agent::harness::env_flag("ANGEL_WEB_SEARCH", true) {
            registry.register(Box::new(RetainedRead {
                inner: Box::new(crate::agent::tools::web::WebSearchTool),
                canonical: canonical.to_path_buf(),
                work: root.to_path_buf(),
                archive: role_archive(&spec.id, role_id),
                requests: Arc::clone(&requests),
            }));
        }
        if crate::agent::harness::env_flag("ANGEL_WEB_FETCH", true) {
            registry.register(Box::new(RetainedRead {
                inner: Box::new(crate::agent::tools::web::WebFetchTool),
                canonical: canonical.to_path_buf(),
                work: root.to_path_buf(),
                archive: role_archive(&spec.id, role_id),
                requests,
            }));
        }
        registry.register(Box::new(LeadTool {
            campaign_root: canonical.to_path_buf(),
            archive: role_archive(&spec.id, role_id),
            mailbox: Arc::clone(leads),
        }));
    } else {
        registry.register(Box::new(RoleExec {
            root: root.to_path_buf(),
            timeout: Duration::from_secs(spec.role_timeout_secs.min(1800)),
        }));
    }
    registry
}

fn briefing(
    spec: &CampaignSpec,
    state: &CampaignState,
    role: &RoleState,
    door: Option<&DoorState>,
    review_materials: Option<&Value>,
) -> String {
    let role_route = match role.role.as_str() {
        "literature" => chapter::LITERATURE,
        "attack" => chapter::ATTACK,
        "referee" => chapter::REFEREE,
        "writer" => chapter::WRITER,
        _ => chapter::COORDINATOR,
    };
    let mut data = json!({"campaign_id":spec.id,"task":spec.task,"role_id":role.id,"role":role.role,
        "inputs":if role.role=="referee"{&spec.referee_inputs}else{&spec.inputs},
        "canonical_documents":spec.canonical_documents,"compute_rules":spec.compute_rules,
        "budgets":{"concurrency":spec.concurrency,"max_hops":spec.max_hops,"role_timeout_secs":spec.role_timeout_secs,"campaign_timeout_secs":spec.campaign_timeout_secs},
        "checks":spec.checks,"spot_checks":spec.spot_checks,"side_projects":spec.side_projects,
        "closed_routes":state.closed_routes,"escalation_routes":state.escalations,"round":state.round,
        "writable_scope":"artifacts/"});
    data["workflow_brief_paths"] = json!(
        workflow_briefs(&role.role)
            .iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>()
    );
    if let Some(ds) = door {
        data["door"] = json!(spec.doors.iter().find(|d| d.id == ds.door_id).unwrap());
        if role.role == "referee" {
            data["author_report"] = ds.attack.outcome.as_ref().unwrap().report.clone();
        }
        if role.role == "writer" {
            data["claims"] = json!(ds.corrected_claims);
            data["corrections"] = json!(ds.corrections);
            data["author"] = json!(ds.attack.outcome.as_ref().unwrap().evidence);
            data["referee"] = json!(ds.referee.outcome.as_ref().unwrap().evidence);
            data["referee_report"] = ds.referee.outcome.as_ref().unwrap().report.clone();
            data["previous_draft_diagnostic"] = json!(ds.writer.error);
            data["review_materials"] = review_materials.cloned().unwrap_or(Value::Null);
        }
    } else {
        data["doors"] = json!(spec.doors);
    }
    data["output_shape"] = match role.role.as_str() {
        "literature" => {
            json!({"leads":[{"id":"lead-id","doors":["door-id"],"statement":"source statement","source":"citation/path/URL"}]})
        }
        "attack" => {
            json!({"claims":[{"id":"claim-id","statement":"claim statement","status":["PROVED","PROVED-CONDITIONAL","COMPUTED","EVIDENCE","CONJECTURE","REFUTED","DEAD END"],"test":"test","lesson":"lesson","proof":"artifacts/proof.md","code":"artifacts/test.py","counterexample":"artifacts/counterexample.json","evidence":["artifacts/output.json"]}],"closed_routes":[{"door_id":"door-id","route":"route","lesson":"lesson"}]})
        }
        "referee" => {
            json!({"verdicts":[{"claim_id":"claim-id","verdict":["ESTABLISHED","ESTABLISHED WITH CORRECTIONS","GAP","FALSE"],"reason":"reason","lesson":"closed route lesson","counterexample":"artifacts/counterexample.json","corrections":[{"id":"correction-id","claim_id":"claim-id","field":["statement","test","lesson"],"old":"exact old fragment","new":"exact replacement"}]}],"independent_check":{"code":"artifacts/independent.py","argv":["python3","-I","-S","-B","artifacts/independent.py"]}})
        }
        "writer" => {
            json!({"claims":data["claims"],"applied_corrections":door.unwrap().corrections.iter().map(|c|&c.id).collect::<Vec<_>>(),"edits":[{"path":"configured canonical path","old":"exact old text","new":"exact new text"}],"nodes":[{"id":"node-id","claim_id":"claim-id","statement":"complete corrected claim statement","kind":["evidence","theorem","exhaustive","conjecture","deadend"],"tier":[null,"T2","T3","T4","T5"]}],"sota":[],"frontier":null})
        }
        _ => json!({}),
    };
    format!(
        "{}\n{}\n{}",
        book::sign_lines(&[chapter::COORDINATOR, role_route, chapter::ESCALATE]),
        book::d3_roles::pages(chapter::DATA, 1..=10),
        data
    )
}

fn archive_role(
    root: &Path,
    spec: &CampaignSpec,
    role: &RoleState,
    work: &Path,
    raw: &str,
    result: Value,
) -> Result<RoleOutput, String> {
    if raw.len() > REPORT_BYTES {
        return Err("role report byte budget exceeded".into());
    }
    let archive = role_archive(&spec.id, &role.id);
    let mut artifacts = Vec::new();
    let (source_sha, source_files) = source_manifest(work)?;
    let mut manifest = Vec::new();
    for (name, hash) in source_files {
        let rel = archive.join("source").join(&name);
        publish(root, &rel, &read(work, Path::new(&name), FILE_BYTES)?)?;
        manifest.push(json!({"path":rel,"source_path":name,"sha256":hash}));
    }
    publish_json(
        root,
        &archive.join("source-manifest.json"),
        &json!({"schema":1,"source_sha256":source_sha,"source_after_sha256":source_sha,"files":manifest}),
    )?;
    let mut paths = Vec::new();
    list_files(work, Path::new("artifacts"), &mut paths, false)?;
    paths.sort();
    let mut total = 0usize;
    for path in paths {
        let bytes = read(work, &path, FILE_BYTES)?;
        total += bytes.len();
        if total > SNAPSHOT_BYTES {
            return Err("role artifact byte budget exceeded".into());
        }
        let rel = archive.join(&path);
        publish(root, &rel, &bytes)?;
        artifacts.push(ArtifactDigest {
            path: rel.to_string_lossy().into_owned(),
            sha256: sha256_hex(&bytes),
        });
    }
    if role.role == "literature" && root.join(archive.join("retrievals")).exists() {
        let mut originals = Vec::new();
        list_files(root, &archive.join("retrievals"), &mut originals, false)?;
        for path in originals {
            let bytes = read(root, &path, FILE_BYTES)?;
            total += bytes.len();
            if total > SNAPSHOT_BYTES {
                return Err("literature retrieval archive byte budget exceeded".into());
            }
            artifacts.push(ArtifactDigest {
                path: path.to_string_lossy().into_owned(),
                sha256: sha256_hex(&bytes),
            });
        }
    }
    let report_path = archive.join("report.md");
    publish(root, &report_path, raw.as_bytes())?;
    let mut result = result;
    result["report_sha256"] = json!(sha256_hex(raw.as_bytes()));
    result["source_sha256"] = json!(source_sha);
    let result_bytes = serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?;
    let result_path = archive.join("result.json");
    publish(root, &result_path, &result_bytes)?;
    let report = serde_json::from_str(raw)
        .map_err(|e| format!("role final report must be JSON; verbatim report retained: {e}"))?;
    let evidence = RoleEvidence {
        id: role.id.clone(),
        role: role.role.clone(),
        report_path: report_path.to_string_lossy().into_owned(),
        report_sha256: sha256_hex(raw.as_bytes()),
        result_path: result_path.to_string_lossy().into_owned(),
        result_sha256: sha256_hex(&result_bytes),
        workspace_path: archive.join("source").to_string_lossy().into_owned(),
        artifacts,
    };
    let output = RoleOutput {
        evidence,
        report,
        stop_reason: result["stop_reason"].as_str().unwrap_or("unknown").into(),
    };
    publish_json(root, &archive.join("outcome.json"), &output)?;
    Ok(output)
}

struct ActiveRole {
    index: Option<usize>,
    kind: String,
    id: String,
    started: Instant,
    cancel: Arc<AtomicBool>,
}
impl Drop for ActiveRole {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
struct LandedRole {
    id: String,
    result: Result<RoleOutput, String>,
}

struct CampaignSeat(Arc<AtomicUsize>);
fn campaign_seats(root: &Path, id: &str) -> Result<Arc<AtomicUsize>, String> {
    static LIVE: OnceLock<Mutex<BTreeMap<PathBuf, std::sync::Weak<AtomicUsize>>>> = OnceLock::new();
    let key = root
        .canonicalize()
        .map_err(|e| e.to_string())?
        .join(base(id));
    let mut live = LIVE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    live.retain(|_, counter| counter.strong_count() > 0);
    if let Some(counter) = live.get(&key).and_then(std::sync::Weak::upgrade) {
        return Ok(counter);
    }
    let counter = Arc::new(AtomicUsize::new(0));
    live.insert(key, Arc::downgrade(&counter));
    Ok(counter)
}
impl CampaignSeat {
    fn reserve(counter: &Arc<AtomicUsize>, limit: usize) -> Result<Self, String> {
        counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < limit).then_some(current + 1)
            })
            .map_err(|_| {
                "campaign physical role capacity remains occupied by unfinished workers".to_string()
            })?;
        Ok(Self(Arc::clone(counter)))
    }
}
impl Drop for CampaignSeat {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn copy_review(
    root: &Path,
    work: &Path,
    evidence: &RoleEvidence,
    prefix: &str,
) -> Result<Value, String> {
    let mut mapping = Vec::new();
    let archive = Path::new(&evidence.report_path)
        .parent()
        .ok_or("role report lacks archive parent")?;
    let items = std::iter::once(ArtifactDigest {
        path: evidence.report_path.clone(),
        sha256: evidence.report_sha256.clone(),
    })
    .chain(std::iter::once(ArtifactDigest {
        path: evidence.result_path.clone(),
        sha256: evidence.result_sha256.clone(),
    }))
    .chain(evidence.artifacts.clone());
    let mut total = 0usize;
    for item in items {
        let rel = Path::new(&item.path)
            .strip_prefix(archive)
            .map_err(|_| "review artifact escapes its original role archive")?;
        let bytes = read(root, Path::new(&item.path), FILE_BYTES)?;
        total += bytes.len();
        if total > SNAPSHOT_BYTES || sha256_hex(&bytes) != item.sha256 {
            return Err("writer review artifact budget/hash mismatch".into());
        }
        let local = Path::new(prefix).join(rel);
        source_path(&local.to_string_lossy())?;
        publish(work, &local, &bytes)?;
        mapping.push(json!({"original_path":item.path,"path":local,"sha256":item.sha256}));
    }
    Ok(
        json!({"role_id":evidence.id,"report_path":Path::new(prefix).join("report.md"),"artifacts":mapping}),
    )
}

fn launch(
    root: &Path,
    spec: &CampaignSpec,
    state: &CampaignState,
    role: &RoleState,
    door: Option<&DoorState>,
    runtime: &CampaignRuntime,
    leads: &Leads,
    tx: &mpsc::Sender<LandedRole>,
    seats: &Arc<AtomicUsize>,
) -> Result<ActiveRole, String> {
    let seat = CampaignSeat::reserve(seats, spec.concurrency)?;
    let work = prepare_role(root, spec, role)?;
    if role.role == "referee" {
        let author = door.unwrap().attack.outcome.as_ref().unwrap();
        let claims = validate_attack(root, author)?;
        publish(
            &work,
            Path::new("author-evidence/report.md"),
            &read(root, Path::new(&author.evidence.report_path), REPORT_BYTES)?,
        )?;
        for claim in &claims {
            let mut evidence = claim["evidence"].as_array().cloned().unwrap_or_default();
            for key in ["proof", "counterexample"] {
                if claim[key].is_object() {
                    evidence.push(claim[key].clone());
                }
            }
            for artifact in evidence {
                let path = artifact["path"]
                    .as_str()
                    .ok_or("author evidence needs retained path")?;
                let relative = Path::new(path)
                    .strip_prefix(Path::new(&author.evidence.report_path).parent().unwrap())
                    .map_err(|_| "author evidence not in author archive")?;
                publish(
                    &work,
                    &Path::new("author-evidence").join(relative),
                    &read(root, Path::new(path), FILE_BYTES)?,
                )?;
            }
        }
    }
    let review_materials = if role.role == "writer" {
        let ds = door.unwrap();
        Some(json!({
            "author":copy_review(root,&work,&ds.attack.outcome.as_ref().unwrap().evidence,"writer-review/author")?,
            "referee":copy_review(root,&work,&ds.referee.outcome.as_ref().unwrap().evidence,"writer-review/referee")?
        }))
    } else {
        None
    };
    let registry = role_registry(&work, &role.role, spec, root, &role.id, leads);
    let club = match role.role.as_str() {
        "literature" => Arc::clone(&runtime.literature),
        "attack" => Arc::new(LeadClub {
            inner: Arc::clone(&runtime.attack),
            mailbox: Arc::clone(leads),
            door: door.unwrap().door_id.clone(),
        }) as Arc<dyn Club>,
        "referee" => Arc::clone(&runtime.referee),
        "writer" => Arc::clone(&runtime.writer),
        _ => return Err("invalid campaign role".into()),
    };
    let prompt = briefing(spec, state, role, door, review_materials.as_ref());
    publish_json(
        root,
        &role_archive(&spec.id, &role.id).join("plan.json"),
        &json!({
        "schema":1,"id":role.id,"role":role.role,"round":state.round,"attempt":role.attempt,
        "door":door.map(|d|&d.door_id),"brief":prompt,"brief_sha256":sha256_hex(prompt.as_bytes()),
        "source_sha256":source_manifest(&work)?.0,"route":club.route_identity(),
        "max_hops":spec.max_hops,"role_timeout_secs":spec.role_timeout_secs,"shared_concurrency":spec.concurrency}),
    )?;
    let budget = current_descendant_budget()?;
    reserve_descendant_calls(&budget, 1, "labyrinth campaign role")?;
    let mut permits = reserve_spawn_seats(1, 64)?;
    let permit = permits.pop().unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let child_cancel = Arc::clone(&cancel);
    let root = root.to_path_buf();
    let spec = spec.clone();
    let role = role.clone();
    let id = role.id.clone();
    let tx = tx.clone();
    let kind = role.role.clone();
    let active_id = id.clone();
    std::thread::Builder::new().name(format!("labyrinth-{id}")).spawn(move||{
        let _permit=permit;let _budget=DescendantBudgetScope::inherit(budget);let _depth=SubcallDepthGuard::enter();
        let started=Instant::now();let (events,event_rx)=mpsc::channel();
        // Drain streamed tokens while the worker is alive. Retained tool
        // receipts live in the outcome, not an unbounded telemetry backlog.
        let _drain=std::thread::spawn(move||{for _ in event_rx{}});
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
        let mut history=vec![ChatMsg::system(book::sign_lines(&[chapter::ROOT,chapter::CAMPAIGN])),ChatMsg::user(prompt)];
        let outcome=run_turn_observed(&*club,&registry,&mut history,&child_cancel,Some(spec.max_hops),&events);
        drop(events);
        match outcome {
            Ok(outcome)=>{
                let raw=outcome.answer;
                let result=json!({"schema":1,"id":role.id,"role":role.role,"club":club.route_identity(),"stop_reason":outcome.stop_reason.as_str(),"rollout_id":outcome.rollout_id,"tools":outcome.tools,"elapsed_ms":started.elapsed().as_millis()});
                match archive_role(&root,&spec,&role,&work,&raw,result){
                    Ok(output) if outcome.stop_reason==TurnStopReason::Answer=>Ok(output),
                    Ok(_)=>Err(format!("role stopped without a complete answer: {}",outcome.stop_reason.as_str())),Err(e)=>Err(e)
                }
            }
            Err(failure)=>{let error=failure.message;let _=publish_json(&root,&role_archive(&spec.id,&role.id).join("failure.json"),&json!({"id":role.id,"role":role.role,"error":error,"elapsed_ms":started.elapsed().as_millis()}));Err(error)}
        }
        })).unwrap_or_else(|panic|{
            let message=panic.downcast_ref::<String>().map(String::as_str).or_else(||panic.downcast_ref::<&str>().copied()).unwrap_or("unknown panic");
            let error=format!("campaign role worker panicked: {}",message.chars().take(4000).collect::<String>());
            let _=publish_json(&root,&role_archive(&spec.id,&role.id).join("failure.json"),&json!({"id":role.id,"role":role.role,"error":error,"elapsed_ms":started.elapsed().as_millis()}));Err(error)
        });
        drop(seat);
        let _=tx.send(LandedRole{id,result});
    }).map_err(|e|format!("could not start campaign role: {e}"))?;
    Ok(ActiveRole {
        index: door.and_then(|d| state.doors.iter().position(|s| s.door_id == d.door_id)),
        kind,
        id: active_id,
        started: Instant::now(),
        cancel,
    })
}

fn role_mut<'a>(
    state: &'a mut CampaignState,
    index: Option<usize>,
    kind: &str,
) -> &'a mut RoleState {
    let Some(index) = index else {
        return &mut state.literature;
    };
    match kind {
        "attack" => &mut state.doors[index].attack,
        "referee" => &mut state.doors[index].referee,
        _ => &mut state.doors[index].writer,
    }
}

fn validate_attack(root: &Path, output: &RoleOutput) -> Result<Vec<Value>, String> {
    let claims = output.report["claims"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 32)
        .ok_or("attack needs 1..32 claims")?;
    let mut ids = BTreeSet::new();
    let mut result = Vec::new();
    for claim in claims {
        let id = claim["id"]
            .as_str()
            .filter(|s| map_id(s) && ids.insert(s.to_string()))
            .ok_or("claim id must be unique and bounded")?;
        let status = claim["status"]
            .as_str()
            .ok_or("claim needs upstream status")?;
        if !matches!(
            status,
            "PROVED"
                | "PROVED-CONDITIONAL"
                | "COMPUTED"
                | "EVIDENCE"
                | "CONJECTURE"
                | "REFUTED"
                | "DEAD END"
        ) || claim["statement"]
            .as_str()
            .is_none_or(|s| !bounded_text(s, 16_000))
        {
            return Err(format!("invalid claim {id}"));
        }
        if status == "CONJECTURE"
            && claim["test"]
                .as_str()
                .is_none_or(|s| !bounded_text(s, 8000))
        {
            return Err("every conjecture needs a concrete test".into());
        }
        if matches!(status, "REFUTED" | "DEAD END")
            && claim["lesson"]
                .as_str()
                .is_none_or(|s| !bounded_text(s, 8000))
        {
            return Err("every closed route needs a lesson".into());
        }
        let mut claim = claim.clone();
        for key in ["proof", "code", "counterexample"] {
            if let Some(raw) = claim[key].as_str().filter(|s| !s.is_empty()) {
                let rel = artifact_path(raw)?;
                let rel = Path::new(&output.evidence.report_path)
                    .parent()
                    .unwrap()
                    .join(rel);
                let bytes = read(root, &rel, FILE_BYTES)?;
                claim[key] = json!({"path":rel,"sha256":sha256_hex(&bytes)});
            }
        }
        if matches!(status, "PROVED" | "PROVED-CONDITIONAL") && !claim["proof"].is_object() {
            return Err("claimed proof must have a retained proof artifact".into());
        }
        if matches!(status, "COMPUTED" | "EVIDENCE") && !claim["code"].is_object() {
            return Err("computed evidence needs retained author code".into());
        }
        let mut evidence = Vec::new();
        if let Some(items) = claim["evidence"].as_array() {
            if items.len() > 32 {
                return Err("claim evidence budget exceeded".into());
            }
            for item in items {
                let rel = artifact_path(item.as_str().ok_or("evidence needs artifact paths")?)?;
                let rel = Path::new(&output.evidence.report_path)
                    .parent()
                    .unwrap()
                    .join(rel);
                let bytes = read(root, &rel, FILE_BYTES)?;
                evidence.push(json!({"path":rel,"sha256":sha256_hex(&bytes)}));
            }
        }
        claim["evidence"] = json!(evidence);
        result.push(claim);
    }
    Ok(result)
}

fn referee_claims(
    root: &Path,
    claims: &[Value],
    report: &Value,
    referee: &RoleEvidence,
) -> Result<(Vec<Value>, Vec<Correction>), String> {
    let verdicts = report["verdicts"]
        .as_array()
        .filter(|a| a.len() == claims.len())
        .ok_or("referee must return exactly one verdict per claim")?;
    let mut seen = BTreeSet::new();
    let mut corrections = Vec::new();
    let mut corrected = Vec::new();
    for original in claims {
        let id = original["id"].as_str().unwrap();
        let matches = verdicts
            .iter()
            .filter(|v| v["claim_id"] == id)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!("missing/duplicate independent verdict for {id}"));
        }
        let verdict = matches[0];
        let label = verdict["verdict"].as_str().ok_or("referee needs verdict")?;
        if !matches!(
            label,
            "ESTABLISHED" | "ESTABLISHED WITH CORRECTIONS" | "GAP" | "FALSE"
        ) || verdict["reason"]
            .as_str()
            .is_none_or(|s| !bounded_text(s, 8000))
        {
            return Err("invalid referee verdict/reason".into());
        }
        let mut claim = original.clone();
        if label == "GAP" {
            let test = verdict
                .get("test")
                .and_then(Value::as_str)
                .or_else(|| original["test"].as_str())
                .filter(|test| bounded_text(test, 8000))
                .ok_or("GAP requires a concrete next test before downgrading to a conjecture")?;
            claim["status"] = json!("CONJECTURE");
            claim["test"] = json!(test);
            claim["lesson"] = verdict["reason"].clone();
        }
        if label == "FALSE" {
            let raw = verdict["counterexample"]
                .as_str()
                .ok_or("FALSE verdict requires a retained referee-owned counterexample")?;
            let rel = Path::new(&referee.report_path)
                .parent()
                .unwrap()
                .join(artifact_path(raw)?);
            let digest = sha256_hex(&read(root, &rel, FILE_BYTES)?);
            if !referee
                .artifacts
                .iter()
                .any(|artifact| Path::new(&artifact.path) == rel && artifact.sha256 == digest)
            {
                return Err(
                    "FALSE counterexample is not a retained independently checked referee artifact"
                        .into(),
                );
            }
            claim["status"] = json!("REFUTED");
            claim["counterexample"] = json!({"path":rel,"sha256":digest});
            claim["lesson"] = verdict
                .get("lesson")
                .filter(|v| v.as_str().is_some_and(|s| bounded_text(s, 8000)))
                .cloned()
                .unwrap_or_else(|| verdict["reason"].clone());
        }
        let fixes = verdict["corrections"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if (label == "ESTABLISHED WITH CORRECTIONS") != !fixes.is_empty() {
            return Err("correction verdict must contain exact corrections".into());
        }
        for fix in fixes {
            let correction: Correction = serde_json::from_value(fix).map_err(|e| e.to_string())?;
            if !map_id(&correction.id)
                || !seen.insert(correction.id.clone())
                || correction.claim_id != id
                || !bounded_text(&correction.old, 16_000)
                || !bounded_text(&correction.new, 16_000)
            {
                return Err("invalid or duplicate exact referee correction".into());
            }
            apply_claim_correction(&mut claim, &correction)?;
            corrections.push(correction);
        }
        claim["referee"] = json!({"id":referee.id,"verdict":label,"reason":verdict["reason"]});
        corrected.push(claim);
    }
    Ok((corrected, corrections))
}

fn run_check(
    root: &Path,
    spec: &CampaignSpec,
    role_id: &str,
    stage: &str,
    work: &Path,
    command: &CheckCommand,
    number: usize,
    code: Option<&str>,
    cancel: &AtomicBool,
) -> Result<CheckReceipt, String> {
    validate_command(command)?;
    let (source_sha256, _) = source_manifest(work)?;
    let receipt_path = role_archive(&spec.id, role_id)
        .join("checks")
        .join(format!("{stage}-{number}.json"));
    if root.join(&receipt_path).exists() {
        let receipt: CheckReceipt =
            serde_json::from_slice(&read(root, &receipt_path, REPORT_BYTES)?)
                .map_err(|e| e.to_string())?;
        if receipt.command != command.argv
            || receipt.source_sha256 != source_sha256
            || receipt.source_sha256 != receipt.source_after_sha256
            || receipt.exit_code != Some(0)
            || !receipt.fresh
            || receipt.cancelled
            || receipt.timed_out
            || sha256_hex(&read(root, Path::new(&receipt.output_path), FILE_BYTES)?)
                != receipt.output_sha256
        {
            return Err("recovered check receipt is stale, failed or hash-mismatched".into());
        }
        if let (Some(path), Some(hash)) = (&receipt.code_path, &receipt.code_sha256) {
            if sha256_hex(&read(root, Path::new(path), FILE_BYTES)?) != *hash {
                return Err("recovered independent-code artifact changed".into());
            }
        }
        if receipt
            .code_after_sha256
            .as_ref()
            .is_some_and(|hash| Some(hash) != receipt.code_sha256.as_ref())
        {
            return Err("recovered independent code changed during execution".into());
        }
        for artifact in &receipt.output_artifacts {
            if !Path::new(&artifact.path).starts_with(role_archive(&spec.id, role_id))
                || sha256_hex(&read(root, Path::new(&artifact.path), FILE_BYTES)?)
                    != artifact.sha256
            {
                return Err(
                    "recovered check output artifact is outside its role or hash-mismatched".into(),
                );
            }
        }
        return Ok(receipt);
    }
    let mut code_sha256 = None;
    let mut code_path = None;
    if let Some(code) = code {
        let rel = artifact_path(code)?;
        let bytes = read(work, &rel, FILE_BYTES)?;
        let archived = role_archive(&spec.id, role_id).join(&rel);
        publish(root, &archived, &bytes)?;
        code_sha256 = Some(sha256_hex(&bytes));
        code_path = Some(archived.to_string_lossy().into_owned());
    }
    let args: Vec<_> = command.argv[1..].iter().map(String::as_str).collect();
    let started = Instant::now();
    let outcome = campaign_subprocess(
        Duration::from_secs(command.timeout_secs),
        Some(cancel),
        || {
            run_sandboxed_observed_cancellable(
                &command.argv[0],
                &args,
                Some(work),
                &role_policy(work, true)?,
                Some(cancel),
            )
        },
    )?;
    let (source_after_sha256, _) = source_manifest(work)?;
    // Preserve the model's precheck artifacts and freeze each replay's outputs
    // separately. Timings/counters may differ across genuinely fresh runs.
    let code_after_sha256 = code
        .map(|name| read(work, Path::new(name), FILE_BYTES).map(|bytes| sha256_hex(&bytes)))
        .transpose()?;
    let mut generated = Vec::new();
    list_files(work, Path::new("artifacts"), &mut generated, false)?;
    let mut output_artifacts = Vec::new();
    let mut output_bytes = 0usize;
    for path in generated {
        let bytes = read(work, &path, FILE_BYTES)?;
        output_bytes += bytes.len();
        if output_bytes > SNAPSHOT_BYTES {
            return Err("check output byte budget exceeded".into());
        }
        let original = role_archive(&spec.id, role_id).join(&path);
        let retained = if !root.join(&original).exists() {
            publish(root, &original, &bytes)?;
            original
        } else if read(root, &original, FILE_BYTES)? == bytes {
            original
        } else {
            let retained = role_archive(&spec.id, role_id)
                .join("checks")
                .join(format!("{stage}-{number}"))
                .join(&path);
            publish(root, &retained, &bytes)?;
            retained
        };
        output_artifacts.push(ArtifactDigest {
            path: retained.to_string_lossy().into_owned(),
            sha256: sha256_hex(&bytes),
        });
    }
    let id = format!("{stage}-{number}");
    let output_path = role_archive(&spec.id, role_id)
        .join("checks")
        .join(format!("{id}.txt"));
    publish(root, &output_path, outcome.output.as_bytes())?;
    let receipt = CheckReceipt {
        id: id.clone(),
        stage: stage.into(),
        workspace_path: role_archive(&spec.id, role_id)
            .join("source")
            .to_string_lossy()
            .into_owned(),
        command: command.argv.clone(),
        command_sha256: sha256_hex(&serde_json::to_vec(&command.argv).map_err(|e| e.to_string())?),
        source_sha256,
        source_after_sha256,
        exit_code: outcome.exit,
        timed_out: outcome.timed_out,
        cancelled: outcome.cancelled,
        output_path: output_path.to_string_lossy().into_owned(),
        output_sha256: sha256_hex(outcome.output.as_bytes()),
        elapsed_ms: started.elapsed().as_millis(),
        code_path,
        code_sha256,
        code_after_sha256,
        output_artifacts,
        fresh: true,
    };
    publish_json(
        root,
        &role_archive(&spec.id, role_id)
            .join("checks")
            .join(format!("{id}.json")),
        &receipt,
    )?;
    if receipt.exit_code != Some(0)
        || receipt.timed_out
        || receipt.cancelled
        || receipt.source_sha256 != receipt.source_after_sha256
        || receipt
            .code_after_sha256
            .as_ref()
            .is_some_and(|hash| Some(hash) != receipt.code_sha256.as_ref())
    {
        return Err(format!(
            "{stage} check rejected: exit={:?} timeout={} cancelled={} source_stable={}; receipt retained at {}",
            receipt.exit_code,
            receipt.timed_out,
            receipt.cancelled,
            receipt.source_sha256 == receipt.source_after_sha256,
            receipt.output_path
        ));
    }
    Ok(receipt)
}

fn check_referee(
    root: &Path,
    spec: &CampaignSpec,
    door: &mut DoorState,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let author = door.attack.outcome.as_ref().unwrap();
    let claims = validate_attack(root, author)?;
    let referee = door.referee.outcome.as_ref().unwrap();
    let code = referee.report["independent_check"]["code"]
        .as_str()
        .ok_or("referee must provide its own independently written check code")?;
    let code_rel = artifact_path(code)?;
    let work = root.join(role_work(&spec.id, &door.referee.id));
    let code_sha = sha256_hex(&read(&work, &code_rel, FILE_BYTES)?);
    if claims
        .iter()
        .any(|c| c["code"]["sha256"].as_str() == Some(&code_sha))
    {
        return Err("referee reused author code; replay is not independent verification".into());
    }
    let argv: Vec<String> =
        serde_json::from_value(referee.report["independent_check"]["argv"].clone())
            .map_err(|e| e.to_string())?;
    validate_independent_command(&argv, code)?;
    let receipt = run_check(
        root,
        spec,
        &door.referee.id,
        "referee",
        &work,
        &CheckCommand {
            argv,
            timeout_secs: spec.role_timeout_secs.min(1800),
        },
        door.referee_rechecks,
        Some(code),
        cancel,
    )?;
    // Coordinator-generated raw outputs join the retained artifact inventory;
    // original model report/result/source identities remain immutable.
    let referee = door.referee.outcome.as_mut().unwrap();
    for item in &receipt.output_artifacts {
        if !referee
            .evidence
            .artifacts
            .iter()
            .any(|prior| prior.path == item.path)
        {
            referee.evidence.artifacts.push(item.clone());
        }
    }
    let (claims, corrections) = referee_claims(root, &claims, &referee.report, &referee.evidence)?;
    door.corrected_claims = claims;
    door.corrections = corrections;
    door.checks.push(receipt);
    Ok(())
}

fn checked_edits(
    root: &Path,
    spec: &CampaignSpec,
    door: &DoorState,
    report: &Value,
) -> Result<Vec<DocumentEdit>, String> {
    let claims = report["claims"]
        .as_array()
        .ok_or("writer must retain the exact corrected claims")?;
    if claims != &door.corrected_claims {
        return Err(
            "writer changed a referee-corrected statement/status or dropped provenance".into(),
        );
    }
    let ids: Vec<String> = serde_json::from_value(report["applied_corrections"].clone())
        .map_err(|_| "writer must enumerate all applied correction ids")?;
    let supplied: BTreeSet<_> = ids.iter().collect();
    let required: BTreeSet<_> = door.corrections.iter().map(|c| &c.id).collect();
    if supplied != required || supplied.len() != ids.len() {
        return Err("writer omitted or invented mandatory corrections".into());
    }
    let raw = report["edits"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 32)
        .ok_or("writer needs bounded exact document edits")?;
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    let mut originals = BTreeMap::new();
    let mut edits = Vec::new();
    let mut prose = String::new();
    for edit in raw {
        let path = edit["path"]
            .as_str()
            .filter(|p| spec.canonical_documents.iter().any(|name| name == *p))
            .ok_or("writer target is outside configured canonical documents")?;
        if !texts.contains_key(path) {
            let bytes = read(
                root,
                &Path::new(
                    &door
                        .writer
                        .outcome
                        .as_ref()
                        .unwrap()
                        .evidence
                        .workspace_path,
                )
                .join(path),
                FILE_BYTES,
            )?;
            originals.insert(path.to_string(), bytes.clone());
            texts.insert(
                path.to_string(),
                String::from_utf8(bytes).map_err(|e| e.to_string())?,
            );
        }
        let old = edit["old"]
            .as_str()
            .filter(|s| bounded_text(s, DOCUMENT_EDIT_BYTES))
            .ok_or("exact OLD text required")?;
        let new = edit["new"]
            .as_str()
            .filter(|s| bounded_text(s, DOCUMENT_EDIT_BYTES))
            .ok_or("exact NEW text required")?;
        let text = texts.get_mut(path).unwrap();
        if text.matches(old).count() != 1 {
            return Err("writer OLD must occur exactly once, re-anchor overlapping edits".into());
        }
        let before = sha256_hex(text.as_bytes());
        *text = text.replacen(old, new, 1);
        let after = sha256_hex(text.as_bytes());
        edits.push(DocumentEdit {
            path: path.into(),
            old: old.into(),
            new: new.into(),
            before_sha256: before,
            after_sha256: after,
        });
        prose.push_str(new);
        prose.push('\n');
    }
    for claim in &door.corrected_claims {
        if !prose.contains(claim["statement"].as_str().unwrap()) {
            return Err(
                "writer prose does not contain every exact corrected claim statement".into(),
            );
        }
    }
    // A check cannot quietly validate against a newer canonical source.
    for (path, bytes) in originals {
        if read(root, Path::new(&path), FILE_BYTES)? != bytes {
            return Err(format!("canonical source changed during campaign: {path}"));
        }
    }
    if let Some(nodes) = report["nodes"].as_array() {
        for node in nodes {
            if node["tier"] == "T1" {
                return Err(
                    "campaign cannot automatically create or modify published T1 claims".into(),
                );
            }
            if node["tier"] == "T3"
                && (!node["certification"].is_object()
                    || node["certification"]["exhaustive"] != true)
            {
                return Err("T3 requires explicit exhaustive certification for independent canonical-owner validation".into());
            }
            if node["tier"] == "T2" && node.get("proof_artifact").is_none() {
                return Err("T2 proposal needs an explicit retained proof artifact".into());
            }
        }
    }
    Ok(edits)
}

fn pending_evidence(
    spec: &CampaignSpec,
    id: &str,
    role: &str,
    original: &RoleEvidence,
) -> RoleEvidence {
    let archive = role_archive(&spec.id, id);
    let mut evidence = original.clone();
    evidence.id = id.into();
    evidence.role = role.into();
    evidence.report_path = archive.join("report.md").to_string_lossy().into_owned();
    evidence.result_path = archive.join("result.json").to_string_lossy().into_owned();
    evidence.result_sha256 = String::new();
    evidence.workspace_path = archive.join("source").to_string_lossy().into_owned();
    if role == "coordinator" {
        evidence.report_sha256 = sha256_hex(b"{}");
        evidence.artifacts.clear();
    }
    evidence
}

fn materialize_preview(work: &Path, preview: &Value) -> Result<(), String> {
    let files = preview["files"]
        .as_array()
        .filter(|files| files.len() <= 3)
        .ok_or("canonical owner preview needs at most three files")?;
    let mut names = if work.join(SOURCE_INDEX).exists() {
        serde_json::from_slice::<Vec<String>>(&read(work, Path::new(SOURCE_INDEX), REPORT_BYTES)?)
            .map_err(|e| e.to_string())?
    } else {
        source_manifest(work)?
            .1
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    };
    let mut seen = BTreeSet::new();
    for file in files {
        let path = file["path"]
            .as_str()
            .filter(|path| {
                matches!(
                    *path,
                    "labyrinth/knowledge.json" | "labyrinth/sota.json" | "labyrinth/frontier.json"
                )
            })
            .ok_or("canonical preview target is outside map/SOTA/frontier")?;
        let content = file["content"]
            .as_str()
            .filter(|text| text.len() <= FILE_BYTES)
            .ok_or("preview requires bounded UTF-8 content")?;
        if !seen.insert(path)
            || file["sha256"].as_str() != Some(sha256_hex(content.as_bytes()).as_str())
        {
            return Err("canonical preview contains duplicate files or inconsistent hashes".into());
        }
        if work.join(path).exists() {
            read(work, Path::new(path), FILE_BYTES)?;
            confined_write(work, Path::new(path), content.as_bytes())?;
        } else {
            publish(work, Path::new(path), content.as_bytes())?;
        }
        if !names.iter().any(|name| name == path) {
            names.push(path.into());
        }
    }
    names.sort();
    names.dedup();
    if names.len() > MAX_FILES {
        return Err("canonical preview exceeds source inventory budget".into());
    }
    confined_write(
        work,
        Path::new(SOURCE_INDEX),
        &serde_json::to_vec_pretty(&names).map_err(|e| e.to_string())?,
    )?;
    Ok(())
}

fn prepare_frozen_writer(
    root: &Path,
    spec: &CampaignSpec,
    id: &str,
    writer: &RoleEvidence,
) -> Result<PathBuf, String> {
    let work = root.join(role_work(&spec.id, id));
    publish(
        root,
        &role_work(&spec.id, id).join("artifacts/.owner"),
        writer.id.as_bytes(),
    )?;
    let manifest: Value = serde_json::from_slice(&read(
        root,
        &Path::new(&writer.workspace_path)
            .parent()
            .ok_or("writer source has no archive parent")?
            .join("source-manifest.json"),
        FILE_BYTES,
    )?)
    .map_err(|error| error.to_string())?;
    let files = manifest["files"]
        .as_array()
        .filter(|files| files.len() <= MAX_FILES)
        .ok_or("writer frozen source inventory exceeds its budget")?;
    for file in files {
        let path = source_path(
            file["source_path"]
                .as_str()
                .ok_or("writer source path missing")?,
        )?;
        let bytes = read(
            root,
            Path::new(file["path"].as_str().ok_or("writer archive path missing")?),
            FILE_BYTES,
        )?;
        if file["sha256"] != sha256_hex(&bytes) {
            return Err("writer frozen source changed".into());
        }
        publish(&work, &path, &bytes)?;
    }
    source_manifest(&work)?;
    let archive = Path::new(&writer.report_path)
        .parent()
        .ok_or("writer report has no archive parent")?;
    for item in &writer.artifacts {
        let path = Path::new(&item.path)
            .strip_prefix(archive)
            .map_err(|_| "writer artifact is outside its immutable archive")?;
        artifact_path(path.to_str().ok_or("writer artifact path is not UTF-8")?)?;
        let bytes = read(root, Path::new(&item.path), FILE_BYTES)?;
        if sha256_hex(&bytes) != item.sha256 {
            return Err("writer frozen artifact changed".into());
        }
        publish(&work, path, &bytes)?;
    }
    Ok(work)
}

fn integrate_writer(
    root: &Path,
    spec: &CampaignSpec,
    state: &CampaignState,
    index: usize,
    runtime: &CampaignRuntime,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    let door = &state.doors[index];
    let writer = door.writer.outcome.as_ref().unwrap();
    let report = &writer.report;
    let retry_suffix = if door.integration_rechecks == 0 {
        String::new()
    } else {
        format!("-i{}", door.integration_rechecks)
    };
    let bundle_path = base(&spec.id).join("integrations").join(format!(
        "{}-{}-r{}-a{}{}.json",
        spec.id,
        door_slug(&door.door_id),
        state.round,
        door.writer.attempt,
        retry_suffix
    ));
    if root.join(&bundle_path).exists() {
        let bundle: IntegrationBundle =
            serde_json::from_slice(&read(root, &bundle_path, 2 * 1024 * 1024)?)
                .map_err(|e| e.to_string())?;
        if bundle.writer.report_sha256 != writer.evidence.report_sha256 {
            return Err("recovered integration writer identity differs".into());
        }
        return runtime.integrator.integrate(root, &bundle, cancel);
    }
    let edits = checked_edits(root, spec, door, report)?;
    let checked_id = format!("{}-checked{}", door.writer.id, retry_suffix);
    let work = prepare_frozen_writer(root, spec, &checked_id, &writer.evidence)?;
    let coordinator_id = format!(
        "coordinator-{}-r{}-a{}{}",
        door_slug(&door.door_id),
        state.round,
        door.writer.attempt,
        retry_suffix
    );
    let canonical_sources_path =
        role_archive(&spec.id, &door.writer.id).join("canonical-sources.json");
    let source_bytes = read(root, &canonical_sources_path, FILE_BYTES)?;
    let mut bundle = IntegrationBundle {
        schema: 1,
        campaign_id: spec.id.clone(),
        id: format!(
            "{}-{}-r{}-a{}{}",
            spec.id,
            door_slug(&door.door_id),
            state.round,
            door.writer.attempt,
            retry_suffix
        ),
        task: spec.task.clone(),
        door_id: door.door_id.clone(),
        coordinator_id: coordinator_id.clone(),
        reviewed_at: super::workflow::now(),
        literature: state
            .literature
            .outcome
            .as_ref()
            .ok_or("literature has no complete archived report")?
            .evidence
            .clone(),
        author: door.attack.outcome.as_ref().unwrap().evidence.clone(),
        referee: door.referee.outcome.as_ref().unwrap().evidence.clone(),
        writer_before: writer.evidence.clone(),
        writer: pending_evidence(spec, &checked_id, "writer", &writer.evidence),
        coordinator: pending_evidence(spec, &coordinator_id, "coordinator", &writer.evidence),
        canonical_sources: serde_json::from_slice(&source_bytes).map_err(|e| e.to_string())?,
        canonical_sources_path: canonical_sources_path.to_string_lossy().into_owned(),
        canonical_sources_sha256: sha256_hex(&source_bytes),
        claims: door.corrected_claims.clone(),
        nodes: report["nodes"].as_array().cloned().unwrap_or_default(),
        sota: report["sota"].as_array().cloned().unwrap_or_default(),
        frontier: report.get("frontier").filter(|v| !v.is_null()).cloned(),
        edits: edits.clone(),
        corrections: door.corrections.clone(),
        applied_corrections: serde_json::from_value(report["applied_corrections"].clone())
            .map_err(|e| e.to_string())?,
        checks: door.checks.clone(),
    };
    let mut checks = door.checks.clone();
    for (n, command) in spec.checks.iter().enumerate() {
        checks.push(run_check(
            root,
            spec,
            &door.writer.id,
            "writer-before",
            &work,
            command,
            door.integration_rechecks * spec.checks.len() + n,
            None,
            cancel,
        )?);
    }
    for edit in &edits {
        let text = String::from_utf8(read(&work, Path::new(&edit.path), FILE_BYTES)?)
            .map_err(|e| e.to_string())?;
        if sha256_hex(text.as_bytes()) != edit.before_sha256 || text.matches(&edit.old).count() != 1
        {
            return Err("writer copy source/anchor changed before applying exact edit".into());
        }
        confined_write(
            &work,
            Path::new(&edit.path),
            text.replacen(&edit.old, &edit.new, 1).as_bytes(),
        )?;
    }
    let draft_path = base(&spec.id)
        .join("drafts")
        .join(format!("{}.json", bundle.id));
    if root.join(&draft_path).exists() {
        let previous: IntegrationBundle =
            serde_json::from_slice(&read(root, &draft_path, 2 * 1024 * 1024)?)
                .map_err(|e| e.to_string())?;
        bundle.reviewed_at = previous.reviewed_at;
    }
    publish_json(root, &draft_path, &bundle)?;
    let preview = runtime.integrator.preview(root, &bundle)?;
    materialize_preview(&work, &preview)?;
    // Writer input copy is archived after exact coordinator-applied edits; the
    // model's original report and result already remain immutable separately.
    let checked_role = RoleState {
        id: checked_id.clone(),
        ..door.writer.clone()
    };
    let raw = String::from_utf8(read(
        root,
        Path::new(&writer.evidence.report_path),
        REPORT_BYTES,
    )?)
    .map_err(|e| e.to_string())?;
    let checked = archive_role(
        root,
        spec,
        &checked_role,
        &work,
        &raw,
        json!({"schema":1,"role":"writer","id":checked_id,"stop_reason":"answer","draft_role_id":door.writer.id,"draft_result_sha256":writer.evidence.result_sha256}),
    )?;
    let mut n = 0;
    for _ in 0..2 {
        for command in &spec.checks {
            checks.push(run_check(
                root,
                spec,
                &checked_id,
                "writer",
                &work,
                command,
                n,
                None,
                cancel,
            )?);
            n += 1;
        }
    }
    let coordinator_role = RoleState {
        id: coordinator_id.clone(),
        role: "coordinator".into(),
        attempt: 0,
        state: "finished".into(),
        outcome: None,
        error: None,
    };
    let coordinator_work = prepare_role(root, spec, &coordinator_role)?;
    for edit in &edits {
        let text = String::from_utf8(read(&coordinator_work, Path::new(&edit.path), FILE_BYTES)?)
            .map_err(|e| e.to_string())?;
        if sha256_hex(text.as_bytes()) != edit.before_sha256 || text.matches(&edit.old).count() != 1
        {
            return Err("coordinator source copy mismatch".into());
        }
        confined_write(
            &coordinator_work,
            Path::new(&edit.path),
            text.replacen(&edit.old, &edit.new, 1).as_bytes(),
        )?;
    }
    materialize_preview(&coordinator_work, &preview)?;
    let coordinator = archive_role(
        root,
        spec,
        &coordinator_role,
        &coordinator_work,
        "{}",
        json!({"schema":1,"id":coordinator_id,"role":"coordinator","stop_reason":"answer"}),
    )?;
    for (n, command) in spec.spot_checks.iter().enumerate() {
        checks.push(run_check(
            root,
            spec,
            &coordinator_id,
            "coordinator",
            &coordinator_work,
            command,
            n,
            None,
            cancel,
        )?);
    }
    // Re-read canonical preimages after all tests and before crossing the sole
    // canonical-write boundary; the integrator repeats this inside its lock.
    let mut first = BTreeSet::new();
    for edit in &edits {
        if first.insert(&edit.path)
            && sha256_hex(&read(root, Path::new(&edit.path), FILE_BYTES)?) != edit.before_sha256
        {
            return Err("canonical source changed after isolated checks; no integration".into());
        }
    }
    bundle.writer = checked.evidence;
    bundle.coordinator = coordinator.evidence;
    bundle.checks = checks;
    publish_json(
        root,
        &base(&spec.id)
            .join("integrations")
            .join(format!("{}.json", bundle.id)),
        &bundle,
    )?;
    if cancel.load(Ordering::Acquire) {
        return Err("campaign cancelled before canonical integration".into());
    }
    runtime.integrator.integrate(root, &bundle, cancel)
}

fn recover_role(root: &Path, spec: &CampaignSpec, role: &mut RoleState) -> Result<(), String> {
    if role.state != "running" {
        return Ok(());
    }
    let rel = role_archive(&spec.id, &role.id).join("outcome.json");
    if root.join(&rel).exists() {
        let output: RoleOutput = serde_json::from_slice(&read(root, &rel, 2 * REPORT_BYTES)?)
            .map_err(|e| e.to_string())?;
        if output.evidence.id != role.id
            || sha256_hex(&read(
                root,
                Path::new(&output.evidence.report_path),
                REPORT_BYTES,
            )?) != output.evidence.report_sha256
            || sha256_hex(&read(
                root,
                Path::new(&output.evidence.result_path),
                REPORT_BYTES,
            )?) != output.evidence.result_sha256
            || serde_json::from_slice::<Value>(&read(
                root,
                Path::new(&output.evidence.report_path),
                REPORT_BYTES,
            )?)
            .map_err(|e| e.to_string())?
                != output.report
        {
            return Err("recovered role provenance mismatch".into());
        }
        if output.stop_reason == "answer" {
            role.state = "finished".into();
            role.outcome = Some(output);
            return Ok(());
        }
    }
    // A saved 'running' is not evidence of live work after a process restart.
    role.attempt += 1;
    let prefix = role
        .id
        .rsplit_once("-a")
        .map_or(role.id.as_str(), |(prefix, _)| prefix);
    role.id = format!("{prefix}-a{}", role.attempt);
    role.state = "pending".into();
    role.error=Some("interrupted role has no complete archived result; replacement owns a fresh role id/workspace".into());
    Ok(())
}

fn verify_retained_role(root: &Path, spec: &CampaignSpec, role: &RoleState) -> Result<(), String> {
    let output = role
        .outcome
        .as_ref()
        .filter(|output| output.stop_reason == "answer")
        .ok_or("recheck requires a complete retained model answer")?;
    let evidence = &output.evidence;
    if evidence.id != role.id || evidence.role != role.role {
        return Err("retained role identity mismatch".into());
    }
    for (path, digest) in [
        (&evidence.report_path, &evidence.report_sha256),
        (&evidence.result_path, &evidence.result_sha256),
    ] {
        if sha256_hex(&read(root, Path::new(path), REPORT_BYTES)?) != *digest {
            return Err("retained model report/result changed before recheck".into());
        }
    }
    let report: Value =
        serde_json::from_slice(&read(root, Path::new(&evidence.report_path), REPORT_BYTES)?)
            .map_err(|error| error.to_string())?;
    if report != output.report {
        return Err("retained report differs from campaign outcome".into());
    }
    for artifact in &evidence.artifacts {
        if !Path::new(&artifact.path).starts_with(role_archive(&spec.id, &role.id))
            || sha256_hex(&read(root, Path::new(&artifact.path), FILE_BYTES)?) != artifact.sha256
        {
            return Err("retained role artifact changed before recheck".into());
        }
    }
    let manifest: Value = serde_json::from_slice(&read(
        root,
        &role_archive(&spec.id, &role.id).join("source-manifest.json"),
        FILE_BYTES,
    )?)
    .map_err(|error| error.to_string())?;
    let source = root.join(&evidence.workspace_path);
    let mut paths = Vec::new();
    list_files(&source, Path::new(""), &mut paths, true)?;
    paths.sort();
    let mut total = 0usize;
    let mut pairs = Vec::new();
    for path in paths {
        let bytes = read(&source, &path, FILE_BYTES)?;
        total += bytes.len();
        if total > SNAPSHOT_BYTES {
            return Err("retained source exceeds recheck budget".into());
        }
        pairs.push((path.to_string_lossy().into_owned(), sha256_hex(&bytes)));
    }
    let archive_sha = sha256_hex(&serde_json::to_vec(&pairs).map_err(|error| error.to_string())?);
    if manifest["source_sha256"] != archive_sha || manifest["source_after_sha256"] != archive_sha {
        return Err("retained role source changed before recheck".into());
    }
    if role.role != "writer" {
        let work = root.join(role_work(&spec.id, &role.id));
        read(&work, Path::new(SOURCE_INDEX), REPORT_BYTES)?;
        if source_manifest(&work)?.0 != archive_sha {
            return Err("retained role source changed before recheck".into());
        }
    }
    Ok(())
}

/// Re-run trusted checks of a complete, immutable referee answer after an
/// operational/protocol failure. This does not synthesize or rewrite a model
/// result, repeat completed model roles, or clear the recorded closed route.
pub(crate) fn recheck(workspace: &Path, id: &str) -> Result<Value, String> {
    let _lock = runner_lock(workspace, id)?;
    let (spec, mut state) = load_state(workspace, id)?;
    if state.status != "budget_exhausted"
        || !state.integrations.is_empty()
        || state.literature.state != "finished"
        || campaign_seats(workspace, id)?.load(Ordering::Acquire) != 0
    {
        return Err("recheck requires an idle, unintegrated blocked campaign".into());
    }
    let eligible: Vec<_> = state
        .doors
        .iter()
        .enumerate()
        .filter(|(_, door)| {
            door.state == "blocked"
                && door.attack.state == "finished"
                && door.integration.is_none()
                && ((door.referee.state == "failed"
                    && door.referee.outcome.is_some()
                    && door.writer.state == "pending"
                    && door.corrected_claims.is_empty()
                    && door.referee_rechecks < 3)
                    || (door.referee.state == "finished"
                        && door.writer.state == "finished"
                        && door.writer.outcome.is_some()
                        && !door.corrected_claims.is_empty()
                        && door.integration_rechecks < 3))
        })
        .map(|(index, _)| index)
        .collect();
    if eligible.is_empty() {
        return Err("no retained referee or writer answer eligible for a bounded recheck".into());
    }
    verify_retained_role(workspace, &spec, &state.literature)?;
    for &index in &eligible {
        verify_retained_role(workspace, &spec, &state.doors[index].attack)?;
        verify_retained_role(workspace, &spec, &state.doors[index].referee)?;
        if state.doors[index].writer.state == "finished" {
            verify_retained_role(workspace, &spec, &state.doors[index].writer)?;
        }
    }
    let failures: Vec<_> = eligible
        .iter()
        .map(|&index| {
            let door = &mut state.doors[index];
            let integration = door.writer.state == "finished";
            let failure = json!({"door_id":door.door_id,"role_id":if integration {&door.writer.id} else {&door.referee.id},
                "previous_error":if integration {&door.error} else {&door.referee.error},
                "kind":if integration {"integration"} else {"referee"}});
            if integration {
                door.integration_rechecks += 1;
                door.state = "refereed".into();
            } else {
                door.referee_rechecks += 1;
                door.referee.state = "finished".into();
                door.referee.error = None;
                door.state = "pending".into();
            }
            door.error = None;
            failure
        })
        .collect();
    state.status = "ready".into();
    state.last_error = None;
    checkpoint!(
        workspace,
        state,
        json!({"type":"campaign-recheck-requested",
        "failures":failures,"model_reports_rewritten":false})
    )?;
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub(crate) fn run(
    workspace: &Path,
    id: &str,
    runtime: CampaignRuntime,
    parent_cancel: &AtomicBool,
) -> Result<Value, String> {
    let _lock = runner_lock(workspace, id)?;
    let (spec, mut state) = load_state(workspace, id)?;
    if matches!(
        state.status.as_str(),
        "completed" | "saturated" | "budget_exhausted"
    ) {
        return serde_json::to_value(state).map_err(|e| e.to_string());
    }
    let seats = campaign_seats(workspace, id)?;
    if seats.load(Ordering::Acquire) > 0 {
        state.status = "execution_blocked".into();
        state.last_error=Some("unfinished workers from a previous coordinator still own this campaign's physical capacity".into());
        checkpoint!(
            workspace,
            state,
            json!({"type":"physical-capacity-blocked","live_workers":seats.load(Ordering::Acquire)})
        )?;
        return serde_json::to_value(state).map_err(|e| e.to_string());
    }
    recover_role(workspace, &spec, &mut state.literature)?;
    for door in &mut state.doors {
        recover_role(workspace, &spec, &mut door.attack)?;
        recover_role(workspace, &spec, &mut door.referee)?;
        recover_role(workspace, &spec, &mut door.writer)?;
    }
    state.run_id = format!("{}-{}", std::process::id(), now_ms());
    state.status = "running".into();
    checkpoint!(workspace, state, json!({"type":"coordinator-resumed"}))?;
    let _budget = DescendantBudgetScope::enter_root();
    let started = Instant::now();
    let leads = Arc::new(Mutex::new(state.leads.clone()));
    let (tx, rx) = mpsc::channel::<LandedRole>();
    let mut active = BTreeMap::<String, ActiveRole>::new();
    let signal = workspace
        .join(base(id))
        .join("signals")
        .join(format!("{}.json", state.run_id));
    let run_cancel = Arc::new(AtomicBool::new(false));
    std::thread::scope(|scope| {
        struct FinishWatchdog(Arc<AtomicBool>);
        impl Drop for FinishWatchdog {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let watchdog_done = Arc::new(AtomicBool::new(false));
        let _finish = FinishWatchdog(Arc::clone(&watchdog_done));
        let watchdog_cancel = Arc::clone(&run_cancel);
        let watchdog_signal = signal.clone();
        let campaign_limit = Duration::from_secs(spec.campaign_timeout_secs);
        scope.spawn(move || {
            while !watchdog_done.load(Ordering::Acquire) {
                if parent_cancel.load(Ordering::Acquire)
                    || watchdog_signal.exists()
                    || started.elapsed() >= campaign_limit
                {
                    watchdog_cancel.store(true, Ordering::Release);
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        loop {
            let externally_cancelled = parent_cancel.load(Ordering::Acquire) || signal.exists();
            if externally_cancelled
                || started.elapsed() >= Duration::from_secs(spec.campaign_timeout_secs)
            {
                run_cancel.store(true, Ordering::Release);
                for role in active.values() {
                    role.cancel.store(true, Ordering::Release);
                }
                state.status = if externally_cancelled {
                    "cancelled"
                } else {
                    "deadline"
                }
                .into();
                for role in active.values() {
                    let saved = role_mut(&mut state, role.index, &role.kind);
                    saved.state = "running".into();
                    saved.error = Some(
                        "cancelled/deadline; late results remain archived for recovery".into(),
                    );
                }
                checkpoint!(workspace, state, json!({"type":state.status.clone()}))?;
                break;
            }
            for landed in rx.try_iter() {
                let Some(job) = active.remove(&landed.id) else {
                    continue;
                };
                let discovered=landed.result.as_ref().ok().map(|output|{
                    let kind=match job.kind.as_str(){"literature"=>"literature","referee"=>"reviewed","writer"=>"documented",_=>{
                        match output.report["claims"].as_array().and_then(|claims|claims.first()).and_then(|claim|claim["status"].as_str()) {
                            Some("PROVED"|"PROVED-CONDITIONAL")=>"proved",Some("REFUTED"|"DEAD END")=>"refuted",_=>"computed"
                        }
                    }};
                    let mut evidence=vec![output.evidence.report_path.clone()];
                    evidence.extend(output.evidence.artifacts.iter().take(126).map(|artifact|artifact.path.clone()));
                    json!({"type":kind,"summary":format!("{} report {} retained; coordinator verification pending",job.kind,landed.id),"campaign":spec.id,"role_id":landed.id,"review_status":"under-review","nodes":job.index.map(|i|vec![state.doors[i].door_id.clone()]).unwrap_or_default(),"evidence":evidence})
                });
                let role = role_mut(&mut state, job.index, &job.kind);
                match landed.result {
                    Ok(output) => {
                        role.state = "finished".into();
                        role.outcome = Some(output);
                        role.error = None;
                    }
                    Err(error) => {
                        role.state = "failed".into();
                        role.error = Some(error);
                    }
                }
                if let Some(event) = discovered {
                    runtime.integrator.event(workspace, &event)?;
                }
                checkpoint!(
                    workspace,
                    state,
                    json!({"type":"role-finished","role_id":landed.id,"role":job.kind})
                )?;
            }
            let expired = active
                .values()
                .filter(|r| r.started.elapsed() >= Duration::from_secs(spec.role_timeout_secs))
                .map(|r| r.id.clone())
                .collect::<Vec<_>>();
            for id in expired {
                let job = active.remove(&id).unwrap();
                job.cancel.store(true, Ordering::Release);
                let role = role_mut(&mut state, job.index, &job.kind);
                role.state = "failed".into();
                role.error = Some(
                    "role deadline exhausted; physical seat permit retained until worker exits"
                        .into(),
                );
                checkpoint!(
                    workspace,
                    state,
                    json!({"type":"role-deadline","role_id":id})
                )?;
            }
            let current_leads = leads.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if state.leads != current_leads {
                state.leads = current_leads;
                runtime.integrator.event(workspace,&json!({"type":"literature","summary":format!("{} literature leads forwarded as unverified source material",state.leads.len()),"campaign":spec.id,"nodes":spec.doors.iter().map(|door|door.id.clone()).collect::<Vec<_>>(),"evidence":[role_archive(&spec.id,&state.literature.id).join("leads").to_string_lossy()]}))?;
                checkpoint!(
                    workspace,
                    state,
                    json!({"type":"literature-forwarded","count":state.leads.len()})
                )?;
            }
            if state.literature.state == "finished" {
                if let Some(items) = state
                    .literature
                    .outcome
                    .as_ref()
                    .and_then(|o| o.report["leads"].as_array())
                {
                    let mut mailbox = leads.lock().unwrap_or_else(|e| e.into_inner());
                    for lead in items {
                        if validate_lead(lead).is_ok()
                            && !mailbox.iter().any(|retained| retained["id"] == lead["id"])
                            && mailbox.len() < MAX_LEADS
                        {
                            let mut lead = lead.clone();
                            lead["source_checked"] = json!(false);
                            lead["claim_verified"] = json!(false);
                            if !mailbox.contains(&lead) {
                                mailbox.push(lead);
                            }
                        }
                    }
                }
            }
            for index in 0..state.doors.len() {
                if matches!(state.doors[index].state.as_str(), "integrated" | "blocked") {
                    continue;
                }
                let failure = if state.doors[index].attack.state == "failed" {
                    state.doors[index].attack.error.clone()
                } else if state.doors[index].referee.state == "failed" {
                    state.doors[index].referee.error.clone()
                } else if state.doors[index].writer.state == "failed" {
                    state.doors[index].writer.error.clone()
                } else if state.literature.state == "failed" {
                    state.literature.error.clone()
                } else {
                    None
                };
                if let Some(error) = failure {
                    state.doors[index].state = "blocked".into();
                    state.doors[index].error = Some(error.clone());
                    state.closed_routes.push(ClosedRoute {
                        door_id: state.doors[index].door_id.clone(),
                        route: format!("round {} rejected attack/referee", state.round),
                        lesson: error,
                    });
                    checkpoint!(
                        workspace,
                        state,
                        json!({"type":"door-blocked","door":state.doors[index].door_id})
                    )?;
                    continue;
                }
                if state.doors[index].attack.state == "finished"
                    && state.doors[index].referee.state == "pending"
                {
                    let output = state.doors[index].attack.outcome.as_ref().unwrap();
                    if let Err(error) = validate_attack(workspace, output) {
                        state.doors[index].attack.state = "failed".into();
                        state.doors[index].attack.error = Some(error);
                        continue;
                    }
                    if let Some(routes) = output.report["closed_routes"].as_array() {
                        for route in routes {
                            if let Ok(route) = serde_json::from_value::<ClosedRoute>(route.clone())
                            {
                                if route.door_id == state.doors[index].door_id
                                    && bounded_text(&route.lesson, 4000)
                                    && state.closed_routes.len() < 128
                                    && !state.closed_routes.contains(&route)
                                {
                                    state.closed_routes.push(route);
                                }
                            }
                        }
                    }
                }
                if state.doors[index].referee.state == "finished"
                    && state.doors[index].corrected_claims.is_empty()
                {
                    if let Err(error) = with_call_budget(
                        Some(
                            Duration::from_secs(spec.campaign_timeout_secs)
                                .saturating_sub(started.elapsed()),
                        ),
                        || check_referee(workspace, &spec, &mut state.doors[index], &run_cancel),
                    ) {
                        state.doors[index].referee.state = "failed".into();
                        state.doors[index].referee.error = Some(error);
                    } else {
                        state.doors[index].state = "refereed".into();
                        for claim in state.doors[index]
                            .corrected_claims
                            .clone()
                            .into_iter()
                            .filter(|claim| {
                                matches!(
                                    claim["referee"]["verdict"].as_str(),
                                    Some("FALSE" | "GAP")
                                )
                            })
                        {
                            let route = ClosedRoute {
                                door_id: state.doors[index].door_id.clone(),
                                route: format!(
                                    "claim {}: attempted route",
                                    claim["id"].as_str().unwrap()
                                ),
                                lesson: claim["lesson"]
                                    .as_str()
                                    .unwrap()
                                    .chars()
                                    .take(4000)
                                    .collect(),
                            };
                            if state.closed_routes.len() < 128
                                && !state.closed_routes.contains(&route)
                            {
                                state.closed_routes.push(route);
                            }
                            let mut evidence = vec![
                                state.doors[index]
                                    .referee
                                    .outcome
                                    .as_ref()
                                    .unwrap()
                                    .evidence
                                    .report_path
                                    .clone(),
                            ];
                            if let Some(path) = claim["counterexample"]["path"].as_str() {
                                evidence.push(path.to_string())
                            }
                            runtime.integrator.event(workspace,&json!({"type":if claim["status"]=="REFUTED"{"refuted"}else{"question"},"summary":format!("Independent review of claim {}: {}",claim["id"].as_str().unwrap(),claim["referee"]["verdict"].as_str().unwrap()),"campaign":spec.id,"nodes":[state.doors[index].door_id.clone()],"evidence":evidence}))?;
                        }
                    }
                    checkpoint!(
                        workspace,
                        state,
                        json!({"type":"referee-checked","door":state.doors[index].door_id})
                    )?;
                }
                if state.doors[index].writer.state == "finished" {
                    match with_call_budget(
                        Some(
                            Duration::from_secs(spec.campaign_timeout_secs)
                                .saturating_sub(started.elapsed()),
                        ),
                        || integrate_writer(workspace, &spec, &state, index, &runtime, &run_cancel),
                    ) {
                        Ok(receipt) => {
                            state.doors[index].state = "integrated".into();
                            state.doors[index].integration = Some(receipt.clone());
                            state.integrations.push(receipt);
                        }
                        Err(error) => {
                            let attempt = state.doors[index].writer.attempt;
                            if attempt < spec.writer_retries {
                                let mut replacement = role(
                                    &spec.id,
                                    &state.doors[index].door_id,
                                    "writer",
                                    state.round,
                                    attempt + 1,
                                );
                                replacement.error = Some(error);
                                state.doors[index].writer = replacement;
                            } else {
                                state.doors[index].state = "blocked".into();
                                state.doors[index].error = Some(error);
                            }
                        }
                    }
                    checkpoint!(
                        workspace,
                        state,
                        json!({"type":"writer-reviewed","door":state.doors[index].door_id,"state":state.doors[index].state})
                    )?;
                }
            }
            // Referees begin as soon as an attack lands; writers never run before
            // their independent check. Literature and attacks share this same cap.
            let mut ready = Vec::<(Option<usize>, String)>::new();
            if state.literature.state == "pending" {
                ready.push((None, "literature".into()));
            }
            for (i, door) in state.doors.iter().enumerate() {
                if matches!(door.state.as_str(), "blocked" | "integrated") {
                    continue;
                }
                if door.referee.state == "pending" && door.attack.state == "finished" {
                    ready.push((Some(i), "referee".into()));
                } else if door.writer.state == "pending"
                    && !door.corrected_claims.is_empty()
                    && state.literature.state == "finished"
                {
                    ready.push((Some(i), "writer".into()));
                } else if door.attack.state == "pending" {
                    ready.push((Some(i), "attack".into()));
                }
            }
            for (index, kind) in ready {
                if active.len() >= spec.concurrency {
                    break;
                }
                if kind == "writer" && active.values().any(|job| job.kind == "writer") {
                    continue;
                }
                let role = role_mut(&mut state, index, &kind).clone();
                let door = index.map(|i| &state.doors[i]);
                match launch(
                    workspace, &spec, &state, &role, door, &runtime, &leads, &tx, &seats,
                ) {
                    Ok(job) => {
                        role_mut(&mut state, index, &kind).state = "running".into();
                        active.insert(role.id.clone(), job);
                    }
                    Err(error) if error.contains("campaign physical role capacity") => {
                        state.status = "execution_blocked".into();
                        state.last_error = Some(error);
                        checkpoint!(
                            workspace,
                            state,
                            json!({"type":"physical-capacity-blocked"})
                        )?;
                        return serde_json::to_value(state).map_err(|e| e.to_string());
                    }
                    Err(error) => {
                        let role = role_mut(&mut state, index, &kind);
                        role.state = "failed".into();
                        role.error = Some(error);
                    }
                }
                checkpoint!(
                    workspace,
                    state,
                    json!({"type":"role-started","role_id":role.id,"role":kind})
                )?;
            }
            if active.is_empty()
                && state
                    .doors
                    .iter()
                    .all(|d| matches!(d.state.as_str(), "blocked" | "integrated"))
            {
                if state.doors.iter().any(|d| d.state == "integrated") {
                    state.status = "completed".into();
                    checkpoint!(
                        workspace,
                        state,
                        json!({"type":"campaign-completed","integrations":state.integrations.len()})
                    )?;
                    break;
                }
                if state.round + 1 >= spec.max_rounds {
                    state.status = if state.escalations.len() == ESCALATION_RUNGS {
                        "saturated"
                    } else {
                        "budget_exhausted"
                    }
                    .into();
                    checkpoint!(
                        workspace,
                        state,
                        json!({"type":state.status.clone(),"unclimbed_rungs":ESCALATION_RUNGS-state.escalations.len()})
                    )?;
                    break;
                }
                if state.round >= ESCALATION_RUNGS {
                    return Err("campaign escalation ladder exhausted".into());
                }
                let rung = chapter::escalation(state.round)
                    .ok_or("campaign escalation ladder exhausted")?;
                state.escalations.push(rung.clone());
                state.round += 1;
                state.doors = new_doors(&spec, state.round);
                state.literature = role(&spec.id, "all", "literature", state.round, 0);
                runtime.integrator.event(
                workspace,
                &json!({"type":"monitor","summary":rung,"campaign":spec.id,"round":state.round,"nodes":spec.doors.iter().map(|door|door.id.clone()).collect::<Vec<_>>(),"evidence":[]}),
            )?;
                checkpoint!(
                    workspace,
                    state,
                    json!({"type":"escalation","rung":rung,"round":state.round})
                )?;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        serde_json::to_value(state).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/labyrinth/campaign__tests.rs"]
mod tests;
