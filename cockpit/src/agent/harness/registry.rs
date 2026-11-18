//! Core tool types and the registry: `Tool`, `ToolRegistry`, `TurnEvent`, `ContextGauge`.

use super::interception::Interception;
use super::registration::Disposable;
use super::*;

pub use crate::telemetry::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessStream {
    Stdout,
    Stderr,
}

pub(crate) type ToolOutputProgress = dyn Fn(ProcessStream, &[u8]) + Send + Sync + 'static;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SubmissionSlotPhase {
    #[default]
    Dormant,
    Empty,
    InFlight,
    Accepted,
    Rejected,
    TimedOut,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SubmissionSlotTelemetry {
    pub(crate) phase: SubmissionSlotPhase,
    pub(crate) id: Option<String>,
    pub(crate) score: Option<String>,
}

impl SubmissionSlotTelemetry {
    pub(crate) fn payload_bytes(&self) -> usize {
        self.id
            .as_ref()
            .map_or(0, String::len)
            .saturating_add(self.score.as_ref().map_or(0, String::len))
    }
}

#[derive(Clone, Debug)]
pub enum TurnEvent {
    ResearchEvidence(
        angel_plug::feed::Evidence,
    ),
    Token(String),
    Reasoning(String),
    Heartbeat,
    SuppressPartial,
    ToolCall {
        id: ToolEventId,
        name: String,
        args_summary: String,
    },
    ToolResult {
        id: ToolEventId,
        name: String,
        summary: String,
        outcome: ToolOutcome,
    },
    GraphCrop {
        id: ToolEventId,
        event: Box<crate::knowledge::graph_crop::GraphEvent>,
    },
    Notice(String),
    RolloutCaptureError(String),
    SpendMilestone { input_tokens: u64 },
    SubmissionSlot(SubmissionSlotTelemetry),
    Media {
        kind: String,
        label: String,
        url: String,
    },
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn def(&self) -> ToolDef;
    fn requires(&self) -> &[coeffect::Key] {
        &[]
    }
    fn workspace_write_scope_is_opaque(&self, _args: &Value) -> bool {
        false
    }
    fn call(&self, args: &Value) -> Result<String, String>;
    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        if cancel.is_some_and(|signal| signal.load(std::sync::atomic::Ordering::Acquire)) {
            return Err("tool cancelled before dispatch".to_string());
        }
        crate::agent::tools::http_transport::with_cancel(cancel, || self.call(args))
    }

    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
        _progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        self.call_with_cancel(args, cancel)
    }

    fn call_with_native_context(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
        _context: &super::delegated_lineage::NativeToolContext,
    ) -> Result<super::delegated_lineage::NativeToolResult, String> {
        self.call_with_cancel_and_progress(args, cancel, progress)
            .map(super::delegated_lineage::NativeToolResult::plain)
    }
}

#[derive(Default)]
pub struct ContextGauge {
    pub used_tokens: AtomicUsize,
    pub budget_tokens: AtomicUsize,
    pub recall_key: AtomicU64,
    pub pending_skill_hints: AtomicUsize,
    pub preturn_code_mode_calls: AtomicUsize,
    pub preturn_code_mode_recipe_calls: AtomicUsize,
    pub preturn_code_mode_nested_calls: AtomicUsize,
    pub preturn_code_mode_nested_output_bytes: AtomicU64,
    pub preturn_code_mode_policy_rejections: AtomicUsize,
    pub preturn_task_recon_context_bytes: AtomicUsize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreturnCodeModeMetrics {
    pub calls: usize,
    pub recipe_calls: usize,
    pub nested_calls: usize,
    pub nested_output_bytes: u64,
    pub policy_rejections: usize,
    pub task_recon_context_bytes: usize,
}

pub struct ToolRegistry {
    pub(crate) vision: crate::agent::tools::vision::VisionBackend,
    pub(crate) tools: Vec<Box<dyn Tool>>,
    deferred: Vec<bool>,
    inverses: Vec<Option<Arc<Disposable>>>,
    seat_grants: std::sync::Mutex<std::collections::BTreeMap<String, Interception>>,
    policy_denials: std::sync::Mutex<Vec<PolicyDenial>>,
    pub(crate) gauge: Arc<ContextGauge>,
    pub(crate) store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    pub(crate) session_id: String,
    workspace: PathBuf,
    pub(super) routed_verifications:
        std::sync::Mutex<std::collections::HashMap<String, super::shell_verifier::RoutedExecution>>,
    pub(crate) mutation_targets: Arc<crate::agent::tools::build::MutationTargets>,
    atlas: Arc<crate::knowledge::atlas::AtlasService>,
    clerk: Arc<crate::knowledge::atlas_clerk::AtlasClerkWorker>,
    boundary: WorkspaceBoundary,
    action_capsules: bool,
    pub(crate) external_evaluator_only: bool,
    pub(crate) aux_clubs: Vec<Arc<dyn Club>>,
    pub(crate) auxiliary: super::auxiliary::AuxiliaryTracker,
    native_context: super::delegated_lineage::NativeToolContext,
    roster: Vec<Arc<dyn Club>>,
    pub(crate) bg_compact: std::sync::Mutex<super::compact::BgCompactState>,
    pub(crate) rolling_digests: std::sync::Mutex<Vec<super::compactor::Digesting>>,
    skill_index: Vec<SkillSummary>,
    tool_activations: Arc<ToolActivations>,
    tool_search_index: Vec<ToolDef>,
    live_tool_router: std::sync::Mutex<LiveToolRouter>,
    live_policies: std::sync::Mutex<LivePolicies>,
    pub(super) live_outcomes: std::sync::Mutex<Option<Arc<super::live_outcomes::Binding>>>,
    advertised_defs: std::sync::Mutex<Option<(u64, u64, Arc<Vec<ToolDef>>, usize)>>,
    backplane: Arc<crate::agent::backplane::BackplaneRegistry>,
    swarm_compiler: Option<Arc<crate::agent::harness::swarm_compile::SwarmCompilerEngine>>,
    rl: Arc<std::sync::Mutex<crate::drive::rl_ctl::RlState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolSchemaProfile {
    Auto,
    Essential,
    Full,
}

#[derive(Default)]
struct LivePolicies {
    offered: std::collections::BTreeMap<String, angel_live::Program>,
    pinned: std::collections::BTreeMap<String, angel_live::Program>,
}

#[derive(Default)]
struct LiveToolRouter {
    program: Option<angel_live::Program>,
    selected_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolBubbleSource {
    Disabled,
    Heuristic,
    Local,
    LocalFallback,
    Live,
    LiveFallback,
}

impl ToolBubbleSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Heuristic => "heuristic",
            Self::Local => "local",
            Self::LocalFallback => "local-fallback",
            Self::Live => "live",
            Self::LiveFallback => "live-fallback",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolBubbleDecision {
    pub(crate) source: ToolBubbleSource,
    pub(crate) tools: Vec<String>,
}

fn tool_schema_profile() -> ToolSchemaProfile {
    #[cfg(not(test))]
    {
        static PROFILE: std::sync::OnceLock<ToolSchemaProfile> = std::sync::OnceLock::new();
        *PROFILE.get_or_init(tool_schema_profile_from_env)
    }
    #[cfg(test)]
    tool_schema_profile_from_env()
}

fn tool_schema_profile_from_env() -> ToolSchemaProfile {
    match std::env::var("ANGEL_TOOL_SCHEMA_PROFILE")
        .unwrap_or_else(|_| "auto".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "essential" | "lean" => ToolSchemaProfile::Essential,
        "full" => ToolSchemaProfile::Full,
        _ => ToolSchemaProfile::Auto,
    }
}

fn bounded_task_schemas_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| env_flag("ANGEL_BOUNDED_TASK_SCHEMAS", true))
    }
    #[cfg(test)]
    env_flag("ANGEL_BOUNDED_TASK_SCHEMAS", true)
}

pub(crate) fn use_essential_schemas(bounded_task: bool, competition: bool) -> bool {
    if competition || crate::drive::comp_mode::enabled() {
        return true;
    }
    bounded_task && bounded_task_schemas_enabled()
}

impl ToolRegistry {
    pub fn new() -> Self {
        let workspace = current_dir_workspace();
        let atlas = crate::knowledge::atlas::AtlasService::open(&workspace);
        let clerk = crate::knowledge::atlas_clerk::AtlasClerkWorker::shared(Arc::clone(&atlas));
        Self {
            vision: crate::agent::tools::vision::VisionBackend::new(None),
            routed_verifications: Default::default(),
            seat_grants: Default::default(),
            policy_denials: Default::default(),
            tools: Vec::new(),
            deferred: Vec::new(),
            inverses: Vec::new(),
            gauge: Arc::new(ContextGauge::default()),
            store: Arc::new(crate::knowledge::memory::store::NullStore),
            session_id: String::new(),
            boundary: WorkspaceBoundary::cached(&workspace),
            atlas,
            clerk,
            workspace,
            mutation_targets: Arc::new(crate::agent::tools::build::MutationTargets::default()),
            action_capsules: false,
            external_evaluator_only: false,
            aux_clubs: Vec::new(),
            roster: Vec::new(),
            bg_compact: std::sync::Mutex::new(super::compact::BgCompactState::default()),
            rolling_digests: std::sync::Mutex::new(Vec::new()),
            skill_index: Vec::new(),
            tool_activations: Arc::new(ToolActivations::default()),
            auxiliary: super::auxiliary::AuxiliaryTracker::default(),
            native_context: super::delegated_lineage::NativeToolContext::default(),
            tool_search_index: Vec::new(),
            live_tool_router: Default::default(),
            live_policies: Default::default(),
            live_outcomes: Default::default(),
            advertised_defs: std::sync::Mutex::new(None),
            backplane: Arc::new(crate::agent::backplane::BackplaneRegistry::default()),
            swarm_compiler: None,
            rl: Arc::new(std::sync::Mutex::new(
                crate::drive::rl_ctl::RlState::default(),
            )),
        }
    }

    fn invalidate_advertised_defs(&self) {
        if let Ok(mut g) = self.advertised_defs.lock() {
            *g = None;
        }
    }

    pub(crate) fn tool_activation_generation(&self) -> u64 {
        self.tool_activations.generation()
    }

    pub(crate) fn has_tool_activations(&self) -> bool {
        !self.tool_activations.snapshot().is_empty()
    }

    pub(crate) fn set_backplane(&mut self, backplane: crate::agent::backplane::BackplaneRegistry) {
        self.backplane = Arc::new(backplane);
    }

    pub(crate) fn backplane(&self) -> Arc<crate::agent::backplane::BackplaneRegistry> {
        Arc::clone(&self.backplane)
    }

    pub(crate) fn swarm_compiler(
        &self,
    ) -> Option<Arc<crate::agent::harness::swarm_compile::SwarmCompilerEngine>> {
        self.swarm_compiler.as_ref().map(Arc::clone)
    }

    pub(crate) fn refresh_backplane(&self, bag: &crate::agent::club::Bag) {
        self.backplane.refresh_from_bag(bag);
    }

    pub fn set_aux_clubs(&mut self, clubs: Vec<Arc<dyn Club>>) {
        self.aux_clubs = clubs;
    }

    pub(crate) fn roster(&self) -> &[Arc<dyn Club>] {
        &self.roster
    }

    pub fn current_workspace(&self) -> &Path {
        &self.workspace
    }

    pub(crate) fn atlas(&self) -> Arc<crate::knowledge::atlas::AtlasService> {
        Arc::clone(&self.atlas)
    }

    pub(crate) fn clerk(&self) -> Arc<crate::knowledge::atlas_clerk::AtlasClerkWorker> {
        Arc::clone(&self.clerk)
    }

    pub(crate) fn workspace_boundary(&self) -> &WorkspaceBoundary {
        &self.boundary
    }

    pub(crate) fn set_workspace(&mut self, workspace: PathBuf) {
        self.boundary = WorkspaceBoundary::cached(&workspace);
        self.atlas = crate::knowledge::atlas::AtlasService::open(&workspace);
        self.clerk =
            crate::knowledge::atlas_clerk::AtlasClerkWorker::shared(Arc::clone(&self.atlas));
        self.workspace = workspace;
    }

    pub(crate) fn set_skill_index(&mut self, skills: &[Skill]) {
        self.skill_index = skill_summaries(skills);
    }

    pub(crate) fn relevant_skill_hint(&self, task: &str) -> Option<String> {
        if !skill_hint_enabled() {
            return None;
        }
        let hint = relevant_skill_hint(&self.skill_index, task);
        if hint.is_some() {
            self.gauge
                .pending_skill_hints
                .fetch_add(1, Ordering::Relaxed);
        }
        hint
    }

    pub(crate) fn reset_tool_activations(&self) {
        self.tool_activations.reset();
    }

    pub(crate) fn set_live_tool_router(&self, program: Option<angel_live::Program>) {
        self.live_tool_router
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .program = program;
    }

    pub(crate) fn set_live_policies(
        &self,
        policies: std::collections::BTreeMap<String, angel_live::Program>,
    ) {
        self.live_policies
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .offered = policies;
    }

    pub(crate) fn pin_live_policies(&self) {
        let mut legs = self.live_policies.lock().unwrap_or_else(|p| p.into_inner());
        legs.pinned = legs.offered.clone();
    }

    pub(crate) fn live_policy(&self, name: &str) -> Option<angel_live::Program> {
        self.live_policies
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pinned
            .get(name)
            .cloned()
    }

    pub(crate) fn tool_router_changed(&self) -> bool {
        let router = self
            .live_tool_router
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        router.program.as_ref().map(angel_live::Program::digest)
            != router.selected_digest.as_deref()
    }

    pub(crate) fn seed_tool_bubble(&self, task: &str) -> ToolBubbleDecision {
        if !env_flag("ANGEL_TOOL_BUBBLE", true)
            || tool_schema_profile() == ToolSchemaProfile::Full
            || task.trim().is_empty()
            || self.tool_search_index.is_empty()
        {
            return ToolBubbleDecision {
                source: ToolBubbleSource::Disabled,
                tools: Vec::new(),
            };
        }
        let activation_max = tool_search_activation_limit();
        let max = env_usize("ANGEL_TOOL_BUBBLE_MAX", 3)
            .min(activation_max)
            .min(8);
        if max == 0 {
            return ToolBubbleDecision {
                source: ToolBubbleSource::Disabled,
                tools: Vec::new(),
            };
        }
        let candidate_limit = env_usize("ANGEL_TOOL_BUBBLE_CANDIDATES", 16)
            .max(max)
            .min(48);
        let projected = super::core_catalog::current()
            .filter(|generation| !generation.is_baseline())
            .map(|generation| {
                let mut entries = self.tool_search_index.clone();
                for entry in &mut entries {
                    generation.overlay(entry);
                }
                entries
            });
        let search_index = projected.as_deref().unwrap_or(&self.tool_search_index);
        let docs = search_index
            .iter()
            .enumerate()
            .filter(|(_, definition)| self.tool_activations.admits(&definition.name))
            .map(|(index, definition)| (index, tool_search_text(definition)))
            .collect::<Vec<_>>();
        let program = {
            let mut router = self
                .live_tool_router
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let digest = router
                .program
                .as_ref()
                .map(|program| program.digest().to_string());
            if digest != router.selected_digest {
                self.tool_activations.reset();
            }
            router.selected_digest = digest;
            router.program.clone()
        };
        let mut source = ToolBubbleSource::Heuristic;
        let hits = if let Some(program) = program {
            match super::live_policy::rank_tools(
                &program,
                task,
                &docs,
                &self.tool_search_index,
                candidate_limit,
            ) {
                Ok(hits) => {
                    source = ToolBubbleSource::Live;
                    hits
                }
                Err(error) => {
                    tracing::debug!(%error, digest = program.digest(), "live tool router retained native ranking");
                    source = ToolBubbleSource::LiveFallback;
                    bm25_rank(task, &docs, candidate_limit)
                }
            }
        } else {
            bm25_rank(task, &docs, candidate_limit)
        };
        if hits.is_empty() {
            return ToolBubbleDecision {
                source,
                tools: Vec::new(),
            };
        }
        let candidates = hits
            .iter()
            .map(|index| &search_index[*index])
            .collect::<Vec<_>>();
        let fallback = candidates
            .iter()
            .take(max)
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        let router = std::env::var("ANGEL_TOOL_BUBBLE_ROUTER")
            .unwrap_or_else(|_| "heuristic".to_string())
            .trim()
            .to_ascii_lowercase();
        let wants_local = matches!(router.as_str(), "local" | "auto" | "model");
        let mut selected = fallback;
        if wants_local {
            source = ToolBubbleSource::LocalFallback;
            let pin = std::env::var("ANGEL_TOOL_BUBBLE_CLUB")
                .ok()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty());
            let local = self.aux_clubs.iter().find(|club| {
                if !club.is_available() {
                    return false;
                }
                let Some(pin) = pin.as_deref() else {
                    return true;
                };
                club.label().eq_ignore_ascii_case(pin)
                    || club
                        .model_identity()
                        .is_some_and(|model| model.eq_ignore_ascii_case(pin))
            });
            if let Some(local) = local {
                let allowed = candidates
                    .iter()
                    .map(|definition| definition.name.clone())
                    .collect::<Vec<_>>();
                let prompt = tool_bubble_router_prompt(task, &candidates, max);
                self.auxiliary.utility_entered("tool_bubble");
                if let Ok(reply) =
                    super::book::connect::respond(&**local, self.current_workspace(), &prompt)
                    && let Some(routed) = parse_tool_bubble_reply(&reply, &allowed, max)
                {
                    selected = routed;
                    source = ToolBubbleSource::Local;
                }
            }
        }
        self.tool_activations
            .activate(selected.iter().map(String::as_str), activation_max);
        let active = self.tool_activations.snapshot();
        selected.retain(|name| active.contains(name));
        ToolBubbleDecision {
            source,
            tools: selected,
        }
    }

    fn with_research_tools(&self, mut definitions: Vec<ToolDef>) -> Vec<ToolDef> {
        for tool in &self.tools {
            if is_research_tool(tool.name())
                && !definitions
                    .iter()
                    .any(|definition| definition.name == tool.name())
            {
                definitions.push(tool.def());
            }
        }
        definitions
    }

    fn with_activated_tools(&self, mut definitions: Vec<ToolDef>) -> Vec<ToolDef> {
        let active = self.tool_activations.snapshot();
        let loop_on = self.rl().loop_enabled();
        for tool in &self.tools {
            if (active.contains(tool.name())
                || (loop_on
                    && (is_research_tool(tool.name())
                        || matches!(
                            tool.name(),
                            "rl_campaign"
                                | "loop_research"
                                | "consult_model"
                                | "spawn"
                                | "continual_harness"
                        ))))
                && !definitions
                    .iter()
                    .any(|definition| definition.name == tool.name())
            {
                definitions.push(tool.def());
            }
        }
        definitions
    }

    pub(crate) fn rl(&self) -> std::sync::MutexGuard<'_, crate::drive::rl_ctl::RlState> {
        self.rl.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn register_rl_campaign(&mut self, club: Option<Arc<dyn Club>>) {
        self.register_deferred(Box::new(
            crate::agent::tools::labyrinth::LabyrinthTool::new(self.workspace.clone()),
        ));
        self.register_deferred(Box::new(
            crate::agent::tools::labyrinth_campaign::LabyrinthCampaignTool::new(
                self.workspace.clone(),
                club,
            ),
        ));
        self.register_deferred(Box::new(
            crate::agent::tools::loop_research::LoopResearchTool::new(
                self.workspace.clone(),
                Arc::clone(&self.rl),
            ),
        ));
        self.register_deferred(Box::new(
            crate::agent::tools::rl_campaign::RlCampaignTool::new(
                self.workspace.clone(),
                Arc::clone(&self.rl),
            ),
        ));
    }

    pub(crate) fn activated_schema_failures(&self, definitions: &[ToolDef]) -> usize {
        let advertised = definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.tool_activations
            .snapshot()
            .iter()
            .filter(|name| !advertised.contains(name.as_str()))
            .count()
    }

    pub(crate) fn enable_action_capsules(&mut self) {
        self.action_capsules = true;
    }

    pub(crate) fn action_capsules_enabled(&self) -> bool {
        self.action_capsules
    }

    pub fn gauge(&self) -> Arc<ContextGauge> {
        Arc::clone(&self.gauge)
    }

    #[allow(dead_code)]
    pub(crate) fn tool_activations(&self) -> Arc<ToolActivations> {
        Arc::clone(&self.tool_activations)
    }

    pub(crate) fn take_preturn_code_mode_metrics(&self) -> PreturnCodeModeMetrics {
        PreturnCodeModeMetrics {
            calls: self
                .gauge
                .preturn_code_mode_calls
                .swap(0, Ordering::Relaxed),
            recipe_calls: self
                .gauge
                .preturn_code_mode_recipe_calls
                .swap(0, Ordering::Relaxed),
            nested_calls: self
                .gauge
                .preturn_code_mode_nested_calls
                .swap(0, Ordering::Relaxed),
            nested_output_bytes: self
                .gauge
                .preturn_code_mode_nested_output_bytes
                .swap(0, Ordering::Relaxed),
            policy_rejections: self
                .gauge
                .preturn_code_mode_policy_rejections
                .swap(0, Ordering::Relaxed),
            task_recon_context_bytes: self
                .gauge
                .preturn_task_recon_context_bytes
                .swap(0, Ordering::Relaxed),
        }
    }

    pub fn set_memory_store(
        &mut self,
        store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    ) {
        self.store = store;
    }

    pub fn memory_store(&self) -> Arc<dyn crate::knowledge::memory::store::MemoryStore> {
        Arc::clone(&self.store)
    }

    pub fn set_session_id(&mut self, id: impl Into<String>) {
        self.session_id = id.into();
    }

    pub fn with_defaults() -> Self {
        let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (cargo, native) = crate::agent::tools::build::capture_verifier_runtimes(&workspace);
        let mut r = Self::new();
        r.set_workspace(workspace.clone());
        r.register_rl_campaign(None);
        r.register(Box::new(ReverseTool));
        r.register(Box::new(WordCountTool));
        r.register(Box::new(
            ShellTool::in_dir(workspace.clone())
                .with_mutation_targets(Arc::clone(&r.mutation_targets)),
        ));
        r.register(Box::new(
            CargoTool::in_dir_with_cargo(workspace.clone(), cargo.clone())
                .with_mutation_targets(Arc::clone(&r.mutation_targets)),
        ));
        r.register(Box::new(RunTestsTool::in_dir_with_runtimes(
            workspace.clone(),
            cargo.clone(),
            native.clone(),
        )));
        r.register(Box::new(LintTool::in_dir_with_runtimes(
            workspace.clone(),
            cargo.clone(),
            native.clone(),
        )));
        r.register(Box::new(
            CheckTool::in_dir_with_runtimes(workspace.clone(), cargo.clone(), native.clone())
                .with_mutation_targets(Arc::clone(&r.mutation_targets)),
        ));
        r.register(Box::new(FmtTool::in_dir_with_cargo(
            workspace.clone(),
            cargo.clone(),
        )));
        r.register(Box::new(TodoTool::new()));
        r.register(Box::new(HandoffTool::new(&workspace)));
        r.register(Box::new(NotesTool::new(&workspace)));
        r.register(Box::new(crate::agent::tools::goal::GoalTool::new(
            &workspace,
        )));
        r.register(Box::new(
            crate::drive::continual_harness::ContinualHarnessTool::new(&workspace),
        ));
        r.register(Box::new(PresentTool::new(&workspace)));
        r.register(Box::new(crate::agent::tools::graph::GraphTool::default()));
        r.register(Box::new(ContextTool::new(r.gauge())));
        register_file_tools(&mut r, workspace.clone());
        maybe_register_web_search(&mut r);
        maybe_register_web_fetch(&mut r);
        maybe_register_science(&mut r);
        maybe_register_repos(&mut r);
        maybe_register_grok_research(&mut r);
        maybe_register_http_request(&mut r);
        maybe_register_proc_tools(&mut r, Some(workspace.clone()));
        for tool in crate::agent::harness::cartridges::active()
            .map(|cartridge| cartridge.hooks().tools(&workspace))
            .unwrap_or_default()
        {
            r.register(tool);
        }
        maybe_register_video_tools(&mut r, workspace.clone());
        maybe_register_vision_tools(&mut r, workspace.clone());
        maybe_register_hands(&mut r, workspace.clone());
        maybe_register_fleet_tools(&mut r);
        maybe_register_llm_tools(&mut r);
        crate::agent::tools::jev::maybe_register_jev(&mut r);
        crate::agent::harness::cartridges::tools::register(&mut r, &workspace);
        maybe_register_embedding_tools(&mut r, workspace);
        maybe_register_code_mode(&mut r);
        maybe_register_handle_read(&mut r);
        if r.atlas.enabled() {
            let atlas = r.atlas();
            r.register_deferred(Box::new(crate::knowledge::atlas::AtlasTool::new(atlas)));
        }
        r
    }

    #[cfg(test)]
    pub fn with_team(workspace: PathBuf, roster: Vec<Arc<dyn Club>>) -> Self {
        Self::with_team_self(workspace, roster, None)
    }

    #[cfg(test)]
    pub fn with_team_self(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        self_club: Option<Arc<dyn Club>>,
    ) -> Self {
        Self::with_team_self_in_run(workspace, roster, self_club, None)
    }

    pub(crate) fn with_team_self_in_run(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        self_club: Option<Arc<dyn Club>>,
        run: Option<&crate::agent::codex_startup::CodexStartup>,
    ) -> Self {
        let (cargo, native) = crate::agent::tools::build::capture_verifier_runtimes(&workspace);
        let mut r = Self::new();
        r.set_workspace(workspace.clone());
        r.register_rl_campaign(self_club.clone().or_else(|| roster.first().cloned()));
        r.vision = crate::agent::tools::vision::VisionBackend::for_driver(
            run,
            self_club.as_ref(),
        );
        r.roster = roster.clone();
        r.register(Box::new(ReverseTool));
        r.register(Box::new(WordCountTool));
        r.register(Box::new(
            ShellTool::in_dir(workspace.clone())
                .with_mutation_targets(Arc::clone(&r.mutation_targets)),
        ));
        r.register(Box::new(
            CargoTool::in_dir_with_cargo(workspace.clone(), cargo.clone())
                .with_mutation_targets(Arc::clone(&r.mutation_targets)),
        ));
        r.register(Box::new(RunTestsTool::in_dir_with_runtimes(
            workspace.clone(),
            cargo.clone(),
            native.clone(),
        )));
        r.register(Box::new(LintTool::in_dir_with_runtimes(
            workspace.clone(),
            cargo.clone(),
            native.clone(),
        )));
        r.register(Box::new(
            CheckTool::in_dir_with_runtimes(workspace.clone(), cargo.clone(), native.clone())
                .with_mutation_targets(Arc::clone(&r.mutation_targets)),
        ));
        r.register(Box::new(FmtTool::in_dir_with_cargo(
            workspace.clone(),
            cargo.clone(),
        )));
        r.register(Box::new(TodoTool::new()));
        r.register(Box::new(HandoffTool::new(&workspace)));
        r.register(Box::new(NotesTool::new(&workspace)));
        r.register(Box::new(crate::agent::tools::goal::GoalTool::new(
            &workspace,
        )));
        r.register(Box::new(
            crate::drive::continual_harness::ContinualHarnessTool::new(&workspace),
        ));
        r.register(Box::new(PresentTool::new(&workspace)));
        r.register(Box::new(crate::agent::tools::graph::GraphTool::default()));
        r.register(Box::new(ContextTool::new(r.gauge())));
        register_file_tools(&mut r, workspace.clone());
        r.register(Box::new(DelegateTool::new_with_cargo(
            workspace.clone(),
            roster.clone(),
            cargo.clone(),
        )));
        r.register(Box::new(IntegrateTool::new(workspace.clone())));
        let swarm_compiler = Arc::new(
            crate::agent::harness::swarm_compile::SwarmCompilerEngine::new_with_cargo(
                workspace.clone(),
                roster.clone(),
                self_club.clone(),
                cargo.clone(),
            ),
        );
        r.swarm_compiler = Some(Arc::clone(&swarm_compiler));
        r.register(Box::new(SwarmCompilerTool::from_engine(swarm_compiler)));
        r.register(Box::new(
            crate::agent::tools::consult::ConsultModelTool::new(roster.clone(), self_club.clone()),
        ));
        r.register(Box::new(crate::agent::tools::consult::CodeReviewTool::new(
            roster.clone(),
            self_club.clone(),
        )));
        r.register(Box::new(
            AgentGraphTool::new_with_cargo(
                workspace.clone(),
                self_club.clone(),
                roster.clone(),
                cargo.clone(),
            )
            .with_mutation_targets(Arc::clone(&r.mutation_targets)),
        ));
        r.register(Box::new(KnowledgeGraphTool::new(
            workspace.clone(),
            self_club.clone(),
            roster.clone(),
        )));
        r.register(Box::new(SpawnTool::new_with_cargo(
            workspace.clone(),
            self_club,
            roster,
            cargo,
        )));
        maybe_register_web_search(&mut r);
        maybe_register_web_fetch(&mut r);
        maybe_register_science(&mut r);
        maybe_register_repos(&mut r);
        maybe_register_grok_research(&mut r);
        maybe_register_http_request(&mut r);
        maybe_register_proc_tools(&mut r, Some(workspace.clone()));
        for tool in crate::agent::harness::cartridges::active()
            .map(|cartridge| cartridge.hooks().tools(&workspace))
            .unwrap_or_default()
        {
            r.register(tool);
        }
        maybe_register_video_tools(&mut r, workspace.clone());
        maybe_register_vision_tools(&mut r, workspace.clone());
        maybe_register_hands(&mut r, workspace.clone());
        maybe_register_fleet_tools(&mut r);
        maybe_register_llm_tools(&mut r);
        crate::agent::tools::jev::maybe_register_jev(&mut r);
        crate::agent::harness::cartridges::tools::register(&mut r, &workspace);
        maybe_register_embedding_tools(&mut r, workspace);
        maybe_register_code_mode(&mut r);
        maybe_register_handle_read(&mut r);
        if r.atlas.enabled() {
            let atlas = r.atlas();
            r.register_deferred(Box::new(crate::knowledge::atlas::AtlasTool::new(atlas)));
        }
        r
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.declare_spec(tool.as_ref());
        self.tools.push(tool);
        self.deferred.push(false);
        self.inverses.push(None);
        self.invalidate_advertised_defs();
    }

    pub fn register_deferred(&mut self, tool: Box<dyn Tool>) {
        self.declare_spec(tool.as_ref());
        self.tools.push(tool);
        self.deferred.push(true);
        self.inverses.push(None);
        self.invalidate_advertised_defs();
    }

    #[allow(dead_code)]
    pub fn register_tracked(
        &mut self,
        tool: Box<dyn Tool>,
        undo: impl FnOnce() + Send + Sync + 'static,
    ) -> Arc<Disposable> {
        self.register_tracked_with(tool, false, Arc::new(Disposable::new(undo)))
    }

    #[allow(dead_code)]
    pub fn register_tracked_deferred(
        &mut self,
        tool: Box<dyn Tool>,
        undo: impl FnOnce() + Send + Sync + 'static,
    ) -> Arc<Disposable> {
        self.register_tracked_with(tool, true, Arc::new(Disposable::new(undo)))
    }

    #[allow(dead_code)]
    pub fn track_registration(
        &mut self,
        tool: Box<dyn Tool>,
        inverse: Arc<Disposable>,
    ) -> Arc<Disposable> {
        self.register_tracked_with(tool, false, inverse)
    }

    pub fn track_registration_in_provider(
        &mut self,
        tool: Box<dyn Tool>,
        inverse: Arc<Disposable>,
        provider: &str,
        deferred: bool,
    ) -> Arc<Disposable> {
        self.register_tracked_scoped(
            tool,
            deferred,
            inverse,
            Some(coeffect::Requirement::any([coeffect::Key::provider(
                provider,
            )])),
        )
    }

    fn register_tracked_with(
        &mut self,
        tool: Box<dyn Tool>,
        deferred: bool,
        disposable: Arc<Disposable>,
    ) -> Arc<Disposable> {
        self.register_tracked_scoped(tool, deferred, disposable, None)
    }

    fn register_tracked_scoped(
        &mut self,
        tool: Box<dyn Tool>,
        deferred: bool,
        disposable: Arc<Disposable>,
        requirement: Option<coeffect::Requirement>,
    ) -> Arc<Disposable> {
        match requirement {
            Some(requirement) => {
                let mut keys = requirement.keys().to_vec();
                keys.extend(tool.requires().iter().cloned());
                self.tool_activations
                    .declare(tool.name(), coeffect::Requirement::any(keys));
            }
            None => self.declare_spec(tool.as_ref()),
        }
        self.tools.push(tool);
        self.deferred.push(deferred);
        self.inverses.push(Some(Arc::clone(&disposable)));
        self.invalidate_advertised_defs();
        disposable
    }

    fn declare_spec(&self, tool: &dyn Tool) {
        let requirement = coeffect::Requirement::any(tool.requires().iter().cloned());
        if !requirement.is_empty() {
            self.tool_activations.declare(tool.name(), requirement);
        }
    }

    pub fn bind_provider(&self, provider: &str) -> Arc<ProviderLease> {
        self.bind_coeffect(coeffect::Key::provider(provider))
    }

    pub fn bind_coeffect(&self, key: coeffect::Key) -> Arc<ProviderLease> {
        let change = self
            .tool_activations
            .store()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bind(key.clone(), coeffect::Value::Flag(true));
        self.tool_activations.observe(&change);
        Arc::new(ProviderLease {
            key,
            store: self.tool_activations.store(),
            activations: Arc::clone(&self.tool_activations),
            released: Arc::new(Disposable::new(|| {})),
        })
    }

    #[allow(dead_code)]
    pub fn tracked_registrations(&self) -> usize {
        self.inverses.iter().filter(|entry| entry.is_some()).count()
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        let Some(index) = self.tools.iter().position(|tool| tool.name() == name) else {
            return false;
        };
        self.tools.remove(index);
        self.deferred.remove(index);
        let inverse = self.inverses.remove(index);
        self.tool_activations.withdraw(name);
        if let Some(inverse) = inverse {
            inverse.fire();
        }
        self.invalidate_advertised_defs();
        true
    }

    #[allow(dead_code)]
    pub fn unregister_prefix(&mut self, prefix: &str) -> usize {
        let names: Vec<String> = self
            .tools
            .iter()
            .map(|tool| tool.name().to_string())
            .filter(|name| name.starts_with(prefix))
            .collect();
        let mut retracted = 0;
        for name in names.iter().rev() {
            if self.unregister(name) {
                retracted += 1;
            }
        }
        retracted
    }

    #[allow(dead_code)]
    pub fn dispose_registrations(&mut self) -> usize {
        let tools = std::mem::take(&mut self.tools);
        let deferred = std::mem::take(&mut self.deferred);
        let inverses = std::mem::take(&mut self.inverses);
        let mut pairs: Vec<(Box<dyn Tool>, bool, Option<Arc<Disposable>>)> = tools
            .into_iter()
            .zip(deferred)
            .zip(inverses)
            .map(|((tool, deferred), inverse)| (tool, deferred, inverse))
            .collect();

        let mut fired = 0;
        let mut removed = Vec::new();
        for (tool, _, inverse) in pairs.iter().rev() {
            if let Some(inverse) = inverse {
                if inverse.fire() {
                    fired += 1;
                }
                removed.push(tool.name().to_string());
            }
        }
        for (tool, deferred, inverse) in pairs.drain(..) {
            if inverse.is_none() {
                self.tools.push(tool);
                self.deferred.push(deferred);
                self.inverses.push(None);
            }
        }
        for name in removed {
            self.tool_activations.withdraw(&name);
        }
        self.invalidate_advertised_defs();
        fired
    }

    pub fn enable_tool_search(&mut self) {
        if let Some(index) = self
            .tools
            .iter()
            .position(|tool| tool.name() == "tool_search")
        {
            self.tools.remove(index);
            self.deferred.remove(index);
            self.inverses.remove(index);
        }
        let entries: Vec<ToolDef> = self
            .tools
            .iter()
            .zip(&self.deferred)
            .filter(|(tool, deferred)| {
                **deferred
                    || !is_coding_hot_path_tool(tool.name())
                    || (small_workspace_route() && !is_small_workspace_tool(tool.name()))
            })
            .map(|(t, _)| t.def())
            .collect();
        self.tool_search_index.clone_from(&entries);
        if !entries.is_empty() {
            self.register(Box::new(ToolSearchTool {
                entries,
                activations: Arc::clone(&self.tool_activations),
            }));
        }
        self.invalidate_advertised_defs();
    }

    pub fn defs(&self) -> Vec<ToolDef> {
        self.defs_arc().as_ref().clone()
    }

    pub(crate) fn defs_arc(&self) -> Arc<Vec<ToolDef>> {
        self.defs_arc_and_tokens().0
    }

    fn defs_arc_and_tokens(&self) -> (Arc<Vec<ToolDef>>, usize) {
        let store_version = self
            .tool_activations
            .store()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .version();
        let activation = self.tool_activations.generation();
        if let Ok(guard) = self.advertised_defs.lock()
            && let Some((cached_store, cached_activation, defs, tokens)) = guard.as_ref()
            && *cached_store == store_version
            && *cached_activation == activation
        {
            return (Arc::clone(defs), *tokens);
        }
        let built = Arc::new(
            self.tools
                .iter()
                .zip(&self.deferred)
                .filter(|(_, d)| !**d)
                .filter(|(t, _)| self.tool_activations.admits(t.name()))
                .map(|(t, _)| t.def())
                .collect::<Vec<_>>(),
        );
        let tokens = estimate_tool_tokens(built.as_ref());
        if let Ok(mut guard) = self.advertised_defs.lock() {
            *guard = Some((store_version, activation, Arc::clone(&built), tokens));
        }
        (built, tokens)
    }

    #[cfg(test)]
    pub fn defs_for_run(&self, window: Option<usize>, bounded_task: bool) -> Vec<ToolDef> {
        self.defs_for_turn(window, bounded_task, false)
    }

    pub(crate) fn defs_for_turn(
        &self,
        window: Option<usize>,
        bounded_task: bool,
        competition: bool,
    ) -> Vec<ToolDef> {
        let defs = self.trimmed_defs_for_turn(window, bounded_task, competition);
        if competition {
            self.with_research_tools(defs)
        } else {
            defs
        }
    }

    fn trimmed_defs_for_turn(
        &self,
        window: Option<usize>,
        bounded_task: bool,
        competition: bool,
    ) -> Vec<ToolDef> {
        let (full, full_tokens) = self.defs_arc_and_tokens();
        match tool_schema_profile() {
            ToolSchemaProfile::Essential => {
                return self.lean_defs(full.as_ref(), true);
            }
            ToolSchemaProfile::Full => return self.with_activated_tools(full.as_ref().clone()),
            ToolSchemaProfile::Auto => {}
        }
        if use_essential_schemas(bounded_task, competition) {
            return self.lean_defs(full.as_ref(), true);
        }
        if let Some(w) = window
            && w > 0
            && full_tokens > (w / 5) * 2 + (w % 5) * 2 / 5
        {
            return self.lean_defs(full.as_ref(), false);
        }
        self.with_activated_tools(full.as_ref().clone())
    }

    pub(crate) fn defs_for_driver_turn(
        &self,
        window: Option<usize>,
        bounded_task: bool,
        competition: bool,
        metered_sota: bool,
    ) -> Vec<ToolDef> {
        if metered_sota && tool_schema_profile() == ToolSchemaProfile::Auto {
            let (full, _) = self.defs_arc_and_tokens();
            let defs = self.lean_defs(full.as_ref(), true);
            return if competition {
                self.with_research_tools(defs)
            } else {
                defs
            };
        }
        self.defs_for_turn(window, bounded_task, competition)
    }

    pub(crate) fn apply_core_catalog(
        &self,
        defs: &mut [ToolDef],
        generation: &super::core_catalog::Generation,
    ) {
        if generation.is_baseline() {
            return;
        }
        for def in defs {
            generation.overlay(def);
        }
    }

    fn lean_defs(&self, full: &[ToolDef], coding_hot_path: bool) -> Vec<ToolDef> {
        let campaign_literature = full
            .iter()
            .any(|definition| definition.name == "campaign_lead");
        self.with_activated_tools(
            full.iter()
                .filter(|definition| {
                    (if coding_hot_path && small_workspace_route() {
                        is_small_workspace_tool(&definition.name)
                    } else if coding_hot_path {
                        is_coding_hot_path_tool(&definition.name)
                    } else {
                        is_essential_tool(&definition.name)
                    }) || (campaign_literature
                        && matches!(definition.name.as_str(), "web_search" | "web_fetch"))
                })
                .map(lean_advertised_tool_def)
                .collect(),
        )
    }

    pub fn dispatch(&self, name: &str, args: &Value) -> Result<String, String> {
        self.dispatch_with_cancel(name, args, None)
    }

    pub(crate) fn dispatch_with_cancel(
        &self,
        name: &str,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        self.dispatch_with_cancel_and_progress(name, args, cancel, None)
    }

    pub(crate) fn dispatch_with_cancel_and_progress(
        &self,
        name: &str,
        args: &Value,
        cancel: Option<&AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        crate::agent::tools::build::settle_pending_pins();
        super::exec::with_call_budget(crate::agent::tools::build::task_call_budget(), || {
            let watch = self.live_watch(name, args);
            let result = self.dispatch_within_budget(name, args, cancel, progress);
            self.live_attach(watch, result, cancel)
        })
    }

    fn dispatch_within_budget(
        &self,
        name: &str,
        args: &Value,
        cancel: Option<&AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        if let Some(error) = crate::agent::club::invalid_tool_args_error(args) {
            return Err(format!(
                "{error}\n{}",
                super::book::x_execution::REISSUE.cells()
            ));
        }
        if let Some(denial) = self.invocation_denial(None, name) {
            return Err(denial);
        }
        if matches!(name, "shell" | "proc_run") {
            let key = serde_json::to_string(&(name, args)).map_err(|e| e.to_string())?;
            self.routed_verifications
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
            if let Ok(route) = super::shell_verifier::plan(name, args, &self.workspace) {
                let result = self.dispatch_within_budget(
                    &route.call.name,
                    &route.call.args,
                    cancel,
                    progress,
                );
                let text = result
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|e| format!("tool error: {e}"));
                let outcome = super::turn_event_outcome(&route.call, &text, false);
                let presented = route.present(text, result.is_err());
                let receipt = super::shell_verifier::RoutingReceipt {
                    routed_call: Some(route.call.clone()),
                    routed_cwd: Some(
                        self.workspace
                            .join(route.call.args["dir"].as_str().unwrap_or("."))
                            .canonicalize()
                            .map_err(|e| e.to_string())?,
                    ),
                    reason: "argv routed through typed adapter".into(),
                };
                self.routed_verifications
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(
                        key,
                        super::shell_verifier::RoutedExecution {
                            text: presented.clone(),
                            outcome,
                            receipt,
                        },
                    );
                return if result.is_ok() {
                    Ok(presented)
                } else {
                    Err(presented.trim_start_matches("tool error: ").to_string())
                };
            }
        }
        let influence = self.atlas.begin_tool_call(name, args);
        let result = match self.tools.iter().find(|t| t.name() == name) {
            Some(tool) => {
                let operation = self.auxiliary.tool_entered(name, args);
                if tool.workspace_write_scope_is_opaque(args) {
                    self.mutation_targets.mark_opaque();
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    tool.call_with_native_context(args, cancel, progress, &self.native_context)
                }))
                .unwrap_or_else(|payload| {
                    let message = payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                        .unwrap_or("non-string panic payload");
                    Err(format!("tool error: {name} panicked: {message}"))
                });
                result.map(|result| {
                    if let Some(artifact) = result.artifact {
                        if let Some(branch) = result.branch {
                            self.native_context.remember(branch, artifact.clone());
                        }
                        operation.resolve(artifact, result.application);
                    }
                    result.text
                })
            }
            None => Err(format!("unknown tool: {name}")),
        };
        self.mutation_targets.record(&self.workspace, name, args);
        self.atlas
            .record_tool_receipt(name, args, &result, influence);
        if matches!(name, "shell" | "proc_run") {
            let text = result
                .as_ref()
                .cloned()
                .unwrap_or_else(|e| format!("tool error: {e}"));
            let call = crate::agent::club::ToolCall {
                id: String::new(),
                name: name.into(),
                args: args.clone(),
            };
            let receipt = super::shell_verifier::RoutingReceipt {
                routed_call: None,
                routed_cwd: None,
                reason: super::shell_verifier::plan(name, args, &self.workspace)
                    .err()
                    .unwrap_or_else(|| "routing shape changed during shell execution".into()),
            };
            let entry = super::shell_verifier::RoutedExecution {
                outcome: super::turn_event_outcome(&call, &text, false),
                text,
                receipt,
            };
            self.routed_verifications
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(serde_json::to_string(&(name, args)).unwrap(), entry);
        }
        result
    }

    pub(crate) fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name() == name)
    }

    pub fn bindable_tool_names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|t| t.name())
            .filter(|n| *n != "code_mode" && *n != "tool_search")
            .map(|n| n.to_string())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PolicyDenial {
    pub seat: Option<String>,
    pub tool: String,
    pub policy: String,
}

const POLICY_DENIAL_LOG: usize = 256;

pub(crate) const ROOT_SEAT: &str = "root";

pub(crate) struct SeatGrant<'a> {
    registry: &'a ToolRegistry,
    seat: &'static str,
}

impl<'a> SeatGrant<'a> {
    pub(crate) fn install(
        registry: &'a ToolRegistry,
        seat: &'static str,
        table: Interception,
    ) -> Self {
        registry.grant_seat(seat, table);
        Self { registry, seat }
    }
}

impl Drop for SeatGrant<'_> {
    fn drop(&mut self) {
        self.registry.revoke_seat(self.seat);
    }
}

impl ToolRegistry {
    pub(crate) fn grant_seat(&self, seat: &str, grant: Interception) {
        self.seat_grants
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(seat.to_string(), grant);
    }

    pub(crate) fn revoke_seat(&self, seat: &str) {
        self.seat_grants
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(seat);
    }

    pub(crate) fn effective_interception(&self, seat: Option<&str>) -> Interception {
        let grants = self.seat_grants.lock().unwrap_or_else(|e| e.into_inner());
        let root = grants.get(ROOT_SEAT).cloned().unwrap_or_default();
        match seat {
            None => root,
            Some(seat) if seat == ROOT_SEAT => root,
            Some(seat) => match grants.get(seat) {
                Some(grant) => root.merge(grant),
                None => root,
            },
        }
    }

    pub(crate) fn policy_denials(&self) -> Vec<PolicyDenial> {
        self.policy_denials
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn record_policy_denial(&self, seat: Option<&str>, tool: &str, policy: &str) -> String {
        let mut log = self
            .policy_denials
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        log.push(PolicyDenial {
            seat: seat.map(str::to_string),
            tool: tool.to_string(),
            policy: policy.to_string(),
        });
        let overflow = log.len().saturating_sub(POLICY_DENIAL_LOG);
        if overflow > 0 {
            log.drain(..overflow);
        }
        let where_ = seat.map(|s| format!(" (seat {s})")).unwrap_or_default();
        format!("policy denied tool {tool}: {policy}{where_}")
    }

    fn invocation_denial(&self, seat: Option<&str>, name: &str) -> Option<String> {
        let table = self.effective_interception(seat);
        if table.is_empty() {
            return None;
        }
        let policy = table.consult(name).denial()?.to_string();
        Some(self.record_policy_denial(seat, name, &policy))
    }

    pub(crate) fn dispatch_with_cancel_in_seat(
        &self,
        seat: &str,
        name: &str,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        if let Some(denial) = self.invocation_denial(Some(seat), name) {
            return Err(denial);
        }
        self.dispatch_with_cancel(name, args, cancel)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

pub struct ProviderLease {
    key: coeffect::Key,
    store: Arc<std::sync::Mutex<coeffect::CoeffectStore>>,
    activations: Arc<super::recall::ToolActivations>,
    released: Arc<Disposable>,
}

impl ProviderLease {
    #[allow(dead_code)]
    pub fn key(&self) -> &coeffect::Key {
        &self.key
    }

    pub fn release(&self) -> Vec<(String, coeffect::Classification)> {
        if !self.released.fire() {
            return Vec::new();
        }
        let change = self
            .store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .release(&self.key);
        self.activations.observe(&change)
    }

    #[allow(dead_code)]
    pub fn as_disposable(self: &Arc<Self>) -> Arc<Disposable> {
        let lease = Arc::clone(self);
        Arc::new(Disposable::new(move || {
            lease.release();
        }))
    }
}

impl Drop for ProviderLease {
    fn drop(&mut self) {
        self.release();
    }
}

pub(crate) fn register_file_tools(r: &mut ToolRegistry, root: PathBuf) {
    use crate::knowledge::cut::capture_writes;
    r.register(Box::new(ReadFileTool { root: root.clone() }));
    r.register(capture_writes(
        Box::new(WriteFileTool { root: root.clone() }),
        root.clone(),
    ));
    r.register(capture_writes(
        Box::new(StrReplaceTool { root: root.clone() }),
        root.clone(),
    ));
    r.register(capture_writes(
        Box::new(MultiEditTool { root: root.clone() }),
        root.clone(),
    ));
    r.register(capture_writes(
        Box::new(ApplyPatchTool { root: root.clone() }),
        root.clone(),
    ));
    r.register(capture_writes(
        Box::new(ResolveEditTool { root: root.clone() }),
        root.clone(),
    ));
    r.register(Box::new(OutlineTool { root: root.clone() }));
    r.register(Box::new(GitDiffTool {
        workspace: root.clone(),
    }));
    r.register(Box::new(GitStatusTool {
        workspace: root.clone(),
    }));
    r.register(Box::new(GitLogTool {
        workspace: root.clone(),
    }));
    r.register(Box::new(GitCommitTool {
        workspace: root.clone(),
    }));
    r.register(Box::new(ToolRepairTool::new(root.clone())));
    r.register(Box::new(GrepTool { root: root.clone() }));
    r.register(Box::new(FindFilesTool { root: root.clone() }));
    r.register(Box::new(FileSearchTool { root: root.clone() }));
    r.register(Box::new(DefsTool { root: root.clone() }));
    r.register(Box::new(ListDirTool { root }));
}

pub(crate) fn is_essential_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "campaign_file"
            | "campaign_exec"
            | "campaign_lead"
            | "write_file"
            | "str_replace"
            | "multi_edit"
            | "apply_patch"
            | "shell"
            | "grep"
            | "find_files"
            | "list_dir"
            | "outline"
            | "get_context_remaining"
            | "handoff"
            | "code_mode"
            | "recall"
            | "swarm_compile"
            | "skill"
            | "tool_search"
    ) || crate::agent::harness::cartridges::tools::is_core(name)
}

pub(crate) fn lean_apply_patch_description() -> &'static str {
    "Apply a workspace patch: unified diff, *** Begin Patch envelope, or \
     hashline [path#tag] line ops (SWAP/INS/DEL/REM/MV). Confined and \
     preflight-checked. Hashline stage=true queues the plan for resolve_edit. \
     Receipts return the new tag. ⠷⠉"
}

pub(crate) fn lean_read_file_description() -> &'static str {
    static TEXT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        format!(
            "Read one bounded UTF-8 workspace page. offset is 1-based (default 1); \
             limit is complete lines (default 200, max 400). Truncation names the next \
             offset. Pages are headed [path#tag] for hashline apply_patch. Also \
             conflict://, skill://, agent://, outline://, pr://, issue://; {}. ⠡⠁",
            crate::agent::harness::book::DIRECTION
        )
    });
    &TEXT
}

pub(crate) fn lean_write_file_description() -> &'static str {
    "Create or overwrite a workspace text file (parents created). Returns the \
     new content tag. Also conflict://N with @ours/@theirs/@base/@both or a \
     custom marker-region body. ⠷⠁"
}

pub(crate) fn lean_shell_description() -> &'static str {
    "Run bash -c with pipefail; returns stdout+stderr. ⠩⠁"
}

pub(crate) fn lean_tool_search_description() -> &'static str {
    "Search additional tools by capability. Matching native schemas activate on the next request. ⠹⠓"
}

pub(crate) fn lean_list_dir_description() -> &'static str {
    "List directory entries; optional hint/pattern ranks first. Truncated pages report counts and next offset. Directories end with /. ⠣⠃"
}

pub(crate) fn lean_str_replace_description() -> &'static str {
    "Replace an exact unique substring in a workspace file. Fails if 'old' is \
     missing or not unique. Returns the new content tag. ⠡⠃"
}

pub(crate) fn lean_grep_description() -> &'static str {
    "Search up to eight workspace paths for a regex. Honors gitignore; skips \
     hidden/credential/quarantine files. Receipts include after_file when \
     another page exists. ⠣⠉"
}

pub(crate) fn lean_grep_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": { "type": "string", "description": "⠣⠉⠉" },
            "path": { "type": "string", "description": "⠣⠉⠙" },
            "paths": {
                "type": "array",
                "minItems": 1,
                "maxItems": 8,
                "items": { "type": "string" },
                "description": "⠣⠉⠑"
            },
            "after_file": {
                "type": "string",
                "description": "⠣⠉⠋"
            },
            "skip_files": {
                "type": "array",
                "maxItems": 128,
                "items": { "type": "string" },
                "description": "⠣⠉⠛"
            },
            "ignore_case": { "type": "boolean", "description": "⠣⠉⠓" },
            "context": {
                "type": "integer",
                "minimum": 0,
                "maximum": 10,
                "description": "⠣⠉⠊"
            },
        },
        "required": ["pattern"],
    })
}

pub(crate) fn lean_advertised_tool_def(definition: &ToolDef) -> ToolDef {
    let description = match definition.name.as_str() {
        "code_mode" => lean_code_mode_description(),
        "apply_patch" => lean_apply_patch_description(),
        "read_file" => lean_read_file_description(),
        "write_file" => lean_write_file_description(),
        "shell" => lean_shell_description(),
        "str_replace" => lean_str_replace_description(),
        "grep" => lean_grep_description(),
        "tool_search" => lean_tool_search_description(),
        "list_dir" => lean_list_dir_description(),
        _ => return definition.clone(),
    };
    ToolDef {
        name: definition.name.clone(),
        description: description.to_string(),
        params: match definition.name.as_str() {
            "grep" => lean_grep_params(),
            _ => definition.params.clone(),
        },
    }
}

pub(crate) fn is_research_tool(name: &str) -> bool {
    matches!(
        name,
        "web_search"
            | "web_fetch"
            | "science_search"
            | "repo_search"
            | "defs"
            | "find_files"
            | "outline"
            | "gpu_stat"
            | "labyrinth"
            | "labyrinth_campaign"
    ) || crate::agent::harness::cartridges::active()
        .is_some_and(|cartridge| cartridge.hooks().research_tools().contains(&name))
}

pub(crate) const SMALL_WORKSPACE_ROUTE_ENV: &str = "ANGEL_TASK_SMALL_WORKSPACE";

fn small_workspace_route() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| env_flag(SMALL_WORKSPACE_ROUTE_ENV, false))
    }
    #[cfg(test)]
    env_flag(SMALL_WORKSPACE_ROUTE_ENV, false)
}

fn is_small_workspace_tool(name: &str) -> bool {
    matches!(
        name,
        "shell" | "read_file" | "write_file" | "str_replace" | "tool_search"
    )
}

pub(crate) fn is_coding_hot_path_tool(name: &str) -> bool {
    is_essential_tool(name)
        && !matches!(
            name,
            "swarm_compile"
                | "skill"
                | "handoff"
                | "recall"
                | "find_files"
                | "get_context_remaining"
                | "multi_edit"
                | "outline"
        )
}

fn skill_hint_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| env_flag("ANGEL_SKILL_HINT", true))
    }
    #[cfg(test)]
    env_flag("ANGEL_SKILL_HINT", true)
}
