//! Core tool types and the registry: `Tool`, `ToolRegistry`, `TurnEvent`, `ContextGauge`.

use super::interception::Interception;
use super::registration::Disposable;
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolEventId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Succeeded,
    /// The harness rejected the call before dispatch, so no tool execution
    /// occurred and no mutation or verification credit may be attributed.
    NotStarted,
    Failed,
    Denied,
    Cancelled,
    Panicked,
}

impl ExecutionOutcome {
    /// Stable machine projection. Keep this explicit so multi-word variants do
    /// not inherit Rust's `Debug` spelling (for example, `NotStarted` must not
    /// leak as the ambiguous `notstarted`).
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::NotStarted => "not_started",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::Cancelled => "cancelled",
            Self::Panicked => "panicked",
        }
    }
}

/// Verifier truth is independent from dispatch truth. A verifier process may
/// run successfully while reporting a red or inconclusive project state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationOutcome {
    NotApplicable,
    Passed,
    Failed,
    Inconclusive,
}

/// Which child-process pipe produced a live tool-output chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessStream {
    Stdout,
    Stderr,
}

/// Thread-safe sink used by process-owning tools to expose bounded live output
/// to an operator surface without changing their final tool receipt.
pub(crate) type ToolOutputProgress = dyn Fn(ProcessStream, &[u8]) + Send + Sync + 'static;

impl VerificationOutcome {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not-applicable",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolOutcome {
    pub execution: ExecutionOutcome,
    pub verification: VerificationOutcome,
}

impl ToolOutcome {
    pub(crate) const fn not_started() -> Self {
        Self {
            execution: ExecutionOutcome::NotStarted,
            verification: VerificationOutcome::NotApplicable,
        }
    }

    pub(crate) fn attributable_success(self) -> bool {
        self.execution == ExecutionOutcome::Succeeded
            && matches!(
                self.verification,
                VerificationOutcome::NotApplicable | VerificationOutcome::Passed
            )
    }
}

/// Operator-visible state of the competition submission slot. This is typed
/// telemetry from the harness watcher, not text parsed back out of the UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SubmissionSlotPhase {
    #[default]
    Dormant,
    Empty,
    InFlight,
    Accepted,
    Rejected,
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

/// Live events emitted during `run_turn` so the UI can show tool-call activity
/// as it happens — not just the final answer seconds/minutes later.
#[derive(Clone, Debug)]
pub enum TurnEvent {
    /// A streamed assistant text delta (token chunk) as it arrives — so the UI
    /// can show the reply forming live instead of all at once at the end.
    Token(String),
    /// A streamed chunk of the model's *private reasoning* (`reasoning_content`
    /// from reasoning models like SIQ) — shown live in the agent's canvas pane,
    /// never folded into the answer or the saved transcript.
    Reasoning(String),
    /// Zero-payload liveness from a bounded provider transport recovery. It
    /// refreshes the foreground idle watchdog without entering the transcript
    /// or pretending the provider emitted a token.
    Heartbeat,
    /// The streamed text was a prose/tool envelope that got recovered into real
    /// tool calls. Drop it from the visible partial answer before tool activity
    /// is rendered.
    SuppressPartial,
    /// The agent requested one or more tool calls (name + brief args summary).
    ToolCall {
        id: ToolEventId,
        name: String,
        args_summary: String,
    },
    /// A tool call completed (name + first line of the result, truncated).
    ToolResult {
        id: ToolEventId,
        name: String,
        summary: String,
        outcome: ToolOutcome,
    },
    /// An out-of-band status note (e.g. context compaction) — shown in the
    /// activity trace, never part of the assistant's reply text.
    Notice(String),
    /// Persistence failure: always visible and retained in task JSON, even when
    /// optional capture leaves the policy turn running.
    RolloutCaptureError(String),
    /// A metered SOTA turn crossed another whole-million input-token mark.
    /// Display-only telemetry: this never enters model history or stops work.
    SpendMilestone { input_tokens: u64 },
    /// Competition submission-slot telemetry for the loop trench HUD. This is
    /// display-only and never enters model history.
    SubmissionSlot(SubmissionSlotTelemetry),
    /// The agent presented a rich-media card to the carousel via the `present`
    /// tool. kind ∈ image | link | graph | resource.
    Media {
        kind: String,
        label: String,
        url: String,
    },
}

pub const RELENTLESS_EXECUTION_DIRECTIVE: &str = "[relentless execution active] Relentless \
execution to the details: keep taking concrete tool-backed actions until the user's request is \
actually advanced; ensure every action benefits the user; produce logical, evidence-grounded \
output. Do not stop at status prose. Deliver a useful final answer, then this mode can turn off.";

/// A capability the agent can invoke.
pub trait Tool: Send + Sync {
    /// The tool's dispatch name — a cheap borrow (literal or owned field), no
    /// allocation. Used on the hot path (`dispatch`/name-listing) so we don't
    /// build a whole `ToolDef` (name + description + JSON schema) per candidate
    /// just to read `.name`. MUST equal `self.def().name`.
    fn name(&self) -> &str;
    /// Name + description + JSON-schema params advertised to the club.
    fn def(&self) -> ToolDef;
    /// The coeffect spec this tool declares — `d ⊆ K`, the keys that must hold
    /// before it is worth advertising (§3.2 Def 21). The default is the
    /// always-available tool; a provider-backed registration takes its spec from
    /// the provider instead, so override this only for a precondition the tool
    /// itself knows (a binary it shells out to, a workspace binding it reads).
    fn requires(&self) -> &[coeffect::Key] {
        &[]
    }
    /// Reviewed opaque executors can declare a workspace-write capability.
    /// The default is not a universal immutability proof for arbitrary tools.
    fn workspace_write_scope_is_opaque(&self, _args: &Value) -> bool {
        false
    }
    /// Run the tool against the (already-parsed) call arguments.
    fn call(&self, args: &Value) -> Result<String, String>;
    /// Run with optional turn cancellation authority. Pure/in-process tools use
    /// the default implementation; process-owning tools override it so an
    /// operator interrupt can kill and reap their child group promptly.
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

    /// Run with cancellation plus an optional live child-output sink. Tools
    /// without a streaming process retain the ordinary cancellation path.
    fn call_with_cancel_and_progress(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
        _progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        self.call_with_cancel(args, cancel)
    }

    /// Native metadata stays out of model-authored arguments and tool text.
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

/// Shared token gauge: `run_turn` writes the current estimate + budget each hop;
/// the `get_context_remaining` tool reads it so the model can pace itself. Atomics
/// give interior mutability through the `&ToolRegistry` the loop holds.
#[derive(Default)]
pub struct ContextGauge {
    pub used_tokens: AtomicUsize,
    /// 0 = no budget configured (the tool then reports usage only).
    pub budget_tokens: AtomicUsize,
    /// Last project+query signature auto-recalled from long-term memory. This
    /// prevents duplicate inserts for the same turn while still letting a later,
    /// genuinely different task recall fresh compacted memory.
    pub recall_key: AtomicU64,
    /// Harness-authored skill hints generated immediately before the next turn.
    pub pending_skill_hints: AtomicUsize,
    /// Provider-free code-mode work performed immediately before a headless
    /// turn. `run_turn` atomically adopts these counters into its one receipt.
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

/// The set of tools available this turn.
pub struct ToolRegistry {
    pub(crate) tools: Vec<Box<dyn Tool>>,
    /// Parallel to `tools`: a deferred tool stays dispatchable but is hidden from
    /// `defs()` (the advertised set) until `tool_search` surfaces it — keeps the
    /// always-loaded schema list lean when many tools (e.g. MCP) are registered.
    deferred: Vec<bool>,
    /// Parallel to `tools`/`deferred`: the inverse each tracked registration
    /// handed back. `None` is the degenerate registration — an untracked tool
    /// carries the identity inverse and has nothing to revert (§3.1.1).
    inverses: Vec<Option<Arc<Disposable>>>,
    /// Per-seat narrowing tables: a seat's effective policy is `grant(root) ⋈
    /// grant(seat)` under the narrowing-only monoid, so a descendant can never widen
    /// an ancestor's denials. The root context is the reserved seat `ROOT_SEAT`.
    seat_grants: std::sync::Mutex<std::collections::BTreeMap<String, Interception>>,
    /// Non-zero while a competition loop worker's tool allowlist is in force.
    loop_worker_allowlist: std::sync::Mutex<Option<(&'static str, &'static [&'static str])>>,
    /// Denial receipts, bounded, newest last: what was refused, under which policy,
    /// for which seat (Def 27's consulted-at-use metadata, made observable).
    policy_denials: std::sync::Mutex<Vec<PolicyDenial>>,
    pub(crate) gauge: Arc<ContextGauge>,
    /// Long-form memory backend (the "palace"). Auto-compaction deposits distilled
    /// drawers here, and recall reads them back. Defaults to a no-op store, so a
    /// registry built without one behaves exactly as before; `bootstrap` installs
    /// the real (MemPalace-backed) store when `ANGEL_MEMPALACE_CMD` is set.
    pub(crate) store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    /// This run's session id, stamped onto every drawer's provenance so a deposit
    /// is attributable to the session that produced it. Empty until `bootstrap`
    /// sets it (a bare registry — e.g. in tests — deposits with no session tag).
    pub(crate) session_id: String,
    /// The active workspace root the scoped tools were built against — the single
    /// source of truth read by `/sandbox`, the goal-loop's `accept_cmd`, and the
    /// status display so none of them can diverge from where the tools actually
    /// operate. `/cd` rebuilds the whole registry at a new root, swapping this.
    workspace: PathBuf,
    pub(super) routed_verifications:
        std::sync::Mutex<std::collections::HashMap<String, super::shell_verifier::RoutedExecution>>,
    pub(crate) mutation_targets: Arc<crate::agent::tools::build::MutationTargets>,
    /// Repository-bound Living Atlas shared by the cockpit UI and deferred
    /// model-facing tool. Rebuilt with the registry at every `/cd`.
    atlas: Arc<crate::knowledge::atlas::AtlasService>,
    /// Process-local idle worker over Atlas's persisted harvest queue.
    clerk: Arc<crate::knowledge::atlas_clerk::AtlasClerkWorker>,
    /// Canonicalized once with the registry and reused for filesystem and Git
    /// confinement instead of repeatedly resolving the same root per call.
    boundary: WorkspaceBoundary,
    /// Whether this registry belongs to the interactive root cockpit. Action
    /// capsules deliberately require this explicit capability in addition to
    /// their runtime mode, so task mode, delegates, evals, and worktree seats
    /// never acquire UI approval waits merely because they inherit the process
    /// environment.
    action_capsules: bool,
    /// A confined recovery evaluator owns measurement after the native turn.
    pub(crate) external_evaluator_only: bool,
    /// Local plain-model clubs for background utility work (compaction
    /// summaries): never a token-metered SOTA link, never the practice echo,
    /// never a fan-out pipeline. Empty (registries built without a bag) means
    /// utility work stays on the in-hand club, exactly as before.
    pub(crate) aux_clubs: Vec<Arc<dyn Club>>,
    pub(crate) auxiliary: super::auxiliary::AuxiliaryTracker,
    native_context: super::delegated_lineage::NativeToolContext,
    /// The session roster the team tools (`delegate`/`spawn`/`consult_model`)
    /// were built over, kept so harness-level seat lookups resolve an operator's
    /// club name exactly the way those tools do. Empty for bare registries,
    /// which then simply have no seat to name.
    roster: Vec<Arc<dyn Club>>,
    /// The one in-flight background compaction pass, if any (see
    /// [`super::compact::maybe_start_bg_compact`]). Registry-homed so a pass
    /// started late in one turn can splice at the next turn's first boundary;
    /// only the unbounded driver turn arms it, so delegate seats sharing this
    /// registry never contend for it.
    pub(crate) bg_compact: std::sync::Mutex<super::compact::BgCompactState>,
    /// Name/description-only index for deterministic per-turn skill hints.
    skill_index: Vec<SkillSummary>,
    /// Per-turn hidden tools surfaced by `tool_search` for the next request.
    tool_activations: Arc<ToolActivations>,
    /// Name/description/schema catalogue assembled once when discovery is
    /// enabled. Local tool-bubble routing reuses it without rebuilding or
    /// presenting the catalogue to the root model.
    tool_search_index: Vec<ToolDef>,
    /// Advertised (non-deferred) schemas, keyed by `(store version, activation
    /// generation)`: rebuilt when the tool set, the deferred flags, `Σ`, or the
    /// activated set move — and not on every hop of a multi-tool turn.
    advertised_defs: std::sync::Mutex<Option<(u64, u64, Arc<Vec<ToolDef>>, usize)>>,
    /// Shared model/resource graph. It projects the Bag and specialist-head
    /// catalog without replacing either protocol-specific dispatcher.
    backplane: Arc<crate::agent::backplane::BackplaneRegistry>,
    /// Shared internal service behind the model-facing `swarm_compile` adapter.
    swarm_compiler: Option<Arc<crate::agent::harness::swarm_compile::SwarmCompilerEngine>>,
    /// One controller shared by `/rl`, the stage, and loop-native tools.
    rl: Arc<std::sync::Mutex<crate::drive::rl_ctl::RlState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolSchemaProfile {
    Auto,
    Essential,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolBubbleSource {
    Disabled,
    Heuristic,
    Local,
    LocalFallback,
}

impl ToolBubbleSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Heuristic => "heuristic",
            Self::Local => "local",
            Self::LocalFallback => "local-fallback",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolBubbleDecision {
    pub(crate) source: ToolBubbleSource,
    pub(crate) tools: Vec<String>,
}

/// Launch-config schema profile. Release caches once; tests re-read so the
/// env-locked profile suite can force lean/full without a process restart.
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

/// Launch-config: bounded-task schema lean set. Default on.
fn bounded_task_schemas_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| env_flag("ANGEL_BOUNDED_TASK_SCHEMAS", true))
    }
    #[cfg(test)]
    env_flag("ANGEL_BOUNDED_TASK_SCHEMAS", true)
}

/// Whether this turn should advertise the discoverable essential set.
///
/// Local unbounded interactive (no hop cap, no competition, comp-mode off)
/// stays on the full schema list. Metered root routes are handled separately by
/// [`ToolRegistry::defs_for_driver_turn`]. Bounded coding tasks, competition,
/// and comp/lean mode recover hidden tools through `tool_search`.
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
            routed_verifications: Default::default(),
            seat_grants: Default::default(),
            loop_worker_allowlist: Default::default(),
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
            skill_index: Vec::new(),
            tool_activations: Arc::new(ToolActivations::default()),
            auxiliary: super::auxiliary::AuxiliaryTracker::default(),
            native_context: super::delegated_lineage::NativeToolContext::default(),
            tool_search_index: Vec::new(),
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

    /// Generation of the activation set — turn hops reuse defs while this is
    /// stable (no `tool_search` surfaces since the last rebuild).
    pub(crate) fn tool_activation_generation(&self) -> u64 {
        self.tool_activations.generation()
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

    /// Install the local utility clubs (called once at startup by `bootstrap`).
    pub fn set_aux_clubs(&mut self, clubs: Vec<Arc<dyn Club>>) {
        self.aux_clubs = clubs;
    }

    /// The session roster the team tools were built over.
    pub(crate) fn roster(&self) -> &[Arc<dyn Club>] {
        &self.roster
    }

    /// The active workspace root the scoped tools operate in. Read for display
    /// (`/sandbox`, status) and to root the goal-loop's acceptance command.
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

    /// Seed a small, sticky hidden-tool set once at the beginning of a metered
    /// turn. The deterministic shortlist is always available; an explicitly
    /// enabled local router may rerank it and fails open to BM25. Activated
    /// names use the same per-turn set as `tool_search`, so later hops remain
    /// byte-stable until discovery genuinely adds another tool.
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
        let docs = self
            .tool_search_index
            .iter()
            .enumerate()
            .filter(|(_, definition)| self.tool_activations.admits(&definition.name))
            .map(|(index, definition)| (index, tool_search_text(definition)))
            .collect::<Vec<_>>();
        let hits = bm25_rank(task, &docs, candidate_limit);
        if hits.is_empty() {
            return ToolBubbleDecision {
                source: ToolBubbleSource::Heuristic,
                tools: Vec::new(),
            };
        }
        let candidates = hits
            .iter()
            .map(|index| &self.tool_search_index[*index])
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
        let mut source = ToolBubbleSource::Heuristic;
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
                if let Ok(reply) = local.respond(&prompt)
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

    fn with_activated_tools(&self, mut definitions: Vec<ToolDef>) -> Vec<ToolDef> {
        let active = self.tool_activations.snapshot();
        let loop_on = self.rl().loop_enabled();
        for tool in &self.tools {
            if (active.contains(tool.name())
                || (loop_on
                    && matches!(
                        tool.name(),
                        "rl_campaign"
                            | "loop_research"
                            | "consult_model"
                            | "spawn"
                            | "continual_harness"
                    )))
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

    fn register_rl_campaign(&mut self) {
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

    /// Activated schemas that are absent from the exact definitions about to be
    /// sent to the provider. Zero is the protocol invariant; exposing the count
    /// makes a discovery/result success insufficient to hide next-hop drift.
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

    /// Mark this registry as the interactive cockpit root. Interactive roots
    /// default to approval; unattended registries never opt in here.
    pub(crate) fn enable_action_capsules(&mut self) {
        self.action_capsules = true;
    }

    /// Whether this registry is allowed to honor the action-capsule runtime
    /// mode. Kept separate from the environment to preserve unattended paths.
    pub(crate) fn action_capsules_enabled(&self) -> bool {
        self.action_capsules
    }

    /// A handle to this registry's context gauge (for the loop to update it).
    pub fn gauge(&self) -> Arc<ContextGauge> {
        Arc::clone(&self.gauge)
    }

    /// The per-turn activation set — hidden tools surfaced by `tool_search`.
    /// Handed out because retraction is observable: a provider that leaves must
    /// take its activation with it, or `activated_schema_failures` goes
    /// non-zero on the next request.
    #[allow(dead_code)]
    pub(crate) fn tool_activations(&self) -> Arc<ToolActivations> {
        Arc::clone(&self.tool_activations)
    }

    /// Atomically transfer provider-free startup work into exactly one turn.
    /// A second turn cannot double-count a prior task's reconnaissance.
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

    /// Install the long-form memory store (called once at startup by `bootstrap`).
    pub fn set_memory_store(
        &mut self,
        store: Arc<dyn crate::knowledge::memory::store::MemoryStore>,
    ) {
        self.store = store;
    }

    /// A handle to the long-form memory store (for recall outside the loop).
    pub fn memory_store(&self) -> Arc<dyn crate::knowledge::memory::store::MemoryStore> {
        Arc::clone(&self.store)
    }

    /// Stamp this registry with the run's session id (called once at startup by
    /// `bootstrap`). Used as the provenance tag on every deposited drawer
    /// (read directly as `registry.session_id` inside the turn loop).
    pub fn set_session_id(&mut self, id: impl Into<String>) {
        self.session_id = id.into();
    }

    /// Single-agent kit: utility tools + the sandboxed `shell`/`cargo` tools.
    pub fn with_defaults() -> Self {
        let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        // Resolve the repository-selected Rust toolchain before any model can
        // mutate this workspace; all five typed build tools share this exact
        // executable identity for the registry's lifetime.
        let (cargo, native) = crate::agent::tools::build::capture_verifier_runtimes(&workspace);
        let mut r = Self::new();
        r.set_workspace(workspace.clone());
        r.register_rl_campaign();
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
        r.register(Box::new(ContextTool::new(r.gauge())));
        register_file_tools(&mut r, workspace.clone());
        maybe_register_web_search(&mut r);
        maybe_register_web_fetch(&mut r);
        maybe_register_science(&mut r);
        maybe_register_repos(&mut r);
        maybe_register_grok_research(&mut r);
        maybe_register_http_request(&mut r);
        maybe_register_proc_tools(&mut r, Some(workspace.clone()));
        maybe_register_video_tools(&mut r, workspace.clone());
        maybe_register_vision_tools(&mut r, workspace.clone());
        maybe_register_fleet_tools(&mut r);
        maybe_register_llm_tools(&mut r);
        crate::agent::tools::jev::maybe_register_jev(&mut r);
        maybe_register_embedding_tools(&mut r, workspace);
        maybe_register_code_mode(&mut r);
        maybe_register_handle_read(&mut r);
        if r.atlas.enabled() {
            let atlas = r.atlas();
            r.register_deferred(Box::new(crate::knowledge::atlas::AtlasTool::new(atlas)));
        }
        r
    }

    /// Orchestrator kit: the single-agent tools, scoped to the shared
    /// `workspace`, plus `delegate`/`integrate` over the club `roster`.
    #[cfg(test)]
    pub fn with_team(workspace: PathBuf, roster: Vec<Arc<dyn Club>>) -> Self {
        Self::with_team_self(workspace, roster, None)
    }

    /// `with_team` plus the caller's own club pinned so the `spawn` tool can
    /// resolve `club:"self"` to true copies of the in-hand model.
    pub fn with_team_self(
        workspace: PathBuf,
        roster: Vec<Arc<dyn Club>>,
        self_club: Option<Arc<dyn Club>>,
    ) -> Self {
        // Pin once before constructing any task-facing tools. Delegates and
        // the root verifier lane cannot drift to different PATH resolutions.
        let (cargo, native) = crate::agent::tools::build::capture_verifier_runtimes(&workspace);
        let mut r = Self::new();
        r.set_workspace(workspace.clone());
        r.register_rl_campaign();
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
        crate::agent::tools::consult::maybe_register_leanstral(&mut r, &roster);
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
        maybe_register_video_tools(&mut r, workspace.clone());
        maybe_register_vision_tools(&mut r, workspace.clone());
        maybe_register_fleet_tools(&mut r);
        maybe_register_llm_tools(&mut r);
        crate::agent::tools::jev::maybe_register_jev(&mut r);
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

    /// Register a tool that's dispatchable but hidden from `defs()` until
    /// `tool_search` surfaces it. Call [`Self::enable_tool_search`] afterwards.
    pub fn register_deferred(&mut self, tool: Box<dyn Tool>) {
        self.declare_spec(tool.as_ref());
        self.tools.push(tool);
        self.deferred.push(true);
        self.inverses.push(None);
        self.invalidate_advertised_defs();
    }

    /// Register a provider together with the inverse that reverts it — the
    /// revertible-effect pairing of arXiv 2608.25512 §3.1.2, Definition 8. The
    /// provider hands back an inverse where it is registered, and firing that
    /// inverse reclaims whatever the provider installed (a child process, a
    /// scratch tree, a socket). Retracting the schema and the deferred flag is
    /// the registry's half of the same effect: [`Self::unregister`] and
    /// [`Self::dispose_registrations`] do both together.
    ///
    /// The returned handle is the one the registry keeps, so firing it early
    /// runs the same inverse. See [`super::registration`] for the once-only and
    /// LIFO guarantees, and `harness/tests/registration_inverse.rs` for the
    /// witness — the runtime does not check that an inverse reverts its effect
    /// (§5.1.1, §6.1), so the test is the proof.
    #[allow(dead_code)]
    pub fn register_tracked(
        &mut self,
        tool: Box<dyn Tool>,
        undo: impl FnOnce() + Send + Sync + 'static,
    ) -> Arc<Disposable> {
        self.register_tracked_with(tool, false, Arc::new(Disposable::new(undo)))
    }

    /// A tracked registration that stays hidden from `defs()` until
    /// `tool_search` surfaces it — the shape MCP servers register in.
    #[allow(dead_code)]
    pub fn register_tracked_deferred(
        &mut self,
        tool: Box<dyn Tool>,
        undo: impl FnOnce() + Send + Sync + 'static,
    ) -> Arc<Disposable> {
        self.register_tracked_with(tool, true, Arc::new(Disposable::new(undo)))
    }

    /// Track a tool against an inverse several registrations can share — a
    /// provider's per-tool inverse from
    /// [`crate::agent::harness::registration::ProviderScope`]. Retracting any one tool
    /// retracts that tool; the provider unloads when the last one goes.
    #[allow(dead_code)]
    pub fn track_registration(
        &mut self,
        tool: Box<dyn Tool>,
        inverse: Arc<Disposable>,
    ) -> Arc<Disposable> {
        self.register_tracked_with(tool, false, inverse)
    }

    /// Track a registration that belongs to a provider: the spec declared for it
    /// is the provider's liveness, not anything the tool knows about itself, so
    /// the schema is advertised only while that provider is in `Σ`.
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

    #[allow(dead_code)]
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
                // A provider-scoped registration must not discard what the tool
                // declares about itself: the effective spec is the provider's
                // liveness *and* the tool's own keys — a conjunction, which is what
                // a spec is (Def 21). Dropping one silently was the alternative.
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

    /// Adopt a tool's own declared spec (§3.2 Def 21). An empty spec is not
    /// recorded: admitting is the default, and a declaration that says nothing
    /// does not deserve a map entry.
    fn declare_spec(&self, tool: &dyn Tool) {
        let requirement = coeffect::Requirement::any(tool.requires().iter().cloned());
        if !requirement.is_empty() {
            self.tool_activations.declare(tool.name(), requirement);
        }
    }

    /// Bind a provider's liveness into `Σ` and hand back the lease that releases
    /// it. Every registration made with [`Self::track_registration_in_provider`]
    /// for this provider is advertised only while the lease is live: releasing it
    /// reclassifies those tools as deactivating, so their schemas leave the
    /// advertisement in the same turn — nothing to remember, no restart.
    pub fn bind_provider(&self, provider: &str) -> Arc<ProviderLease> {
        self.bind_coeffect(coeffect::Key::provider(provider))
    }

    /// Bind an arbitrary key in `Σ` — a capability, a resource, a session — and
    /// hand back the lease that releases it. Whatever declares this key is
    /// advertised only while that lease is live.
    pub fn bind_coeffect(&self, key: coeffect::Key) -> Arc<ProviderLease> {
        let change = self
            .tool_activations
            .store()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bind(key.clone(), coeffect::Value::Flag(true));
        // The provider *arriving* is itself a change: a tool already selected for
        // advertisement comes back on its own once its spec holds again.
        self.tool_activations.observe(&change);
        Arc::new(ProviderLease {
            key,
            store: self.tool_activations.store(),
            activations: Arc::clone(&self.tool_activations),
            released: Arc::new(Disposable::new(|| {})),
        })
    }

    /// Live tracked registrations — providers that still owe recovery.
    #[allow(dead_code)]
    pub fn tracked_registrations(&self) -> usize {
        self.inverses.iter().filter(|entry| entry.is_some()).count()
    }

    /// Retract one registration by name: drop the tool, its deferred flag, and
    /// the schema it contributed, then fire its inverse exactly once. Returns
    /// whether a tool by that name was registered.
    ///
    /// The name also leaves the activation set: `activated_schema_failures` is a
    /// zero invariant, and a name left behind by a departed provider would break
    /// it on the next provider request.
    #[allow(dead_code)]
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

    /// Retract every registration whose name starts with `prefix`, in reverse
    /// registration order (LIFO), firing each inverse once. This is the
    /// mid-session provider unload: one call drops a whole server's tools and
    /// lets its process go when the last inverse fires, with no cockpit restart.
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

    /// Unload every tracked provider: fire its inverse in reverse order of
    /// registration (the LIFO fold of §5.1.1) and retract exactly those tools,
    /// leaving untracked builtins in place. Returns how many inverses ran.
    ///
    /// Deliberately explicit rather than a `Drop` impl on the registry: a *rebuild*
    /// (the workspace/session swap constructs a fresh registry rather than reusing
    /// this one) fires the same inverses through `Disposable`'s own `Drop`, so no
    /// tracked provider leaks in that path either. This method is for a caller that
    /// keeps the registry and wants the composition unloaded now.
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

    /// Register (or refresh) `tool_search` over deferred tools and anything
    /// hidden from the coding-hot-path advertised set (non-essential plus
    /// essential-but-dropped adapters/memory). Discovery must include them
    /// even though they were registered normally.
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
            .filter(|(tool, deferred)| **deferred || !is_coding_hot_path_tool(tool.name()))
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

    /// The advertised tool schemas — every registered tool except deferred ones.
    /// Cached across hops; invalidated on register/deferred changes only.
    pub fn defs(&self) -> Vec<ToolDef> {
        self.defs_arc().as_ref().clone()
    }

    /// Shared handle to the advertised set — hop reuse without cloning the
    /// whole schema list when activations haven't moved. Token estimate is
    /// computed once with the cache so budget math doesn't re-serialize every
    /// schema JSON blob on every hop.
    pub(crate) fn defs_arc(&self) -> Arc<Vec<ToolDef>> {
        self.defs_arc_and_tokens().0
    }

    fn defs_arc_and_tokens(&self) -> (Arc<Vec<ToolDef>>, usize) {
        // The derivation key: `Σ`'s version and the activation generation. A
        // neutral change moves neither, so the memo survives it — the "no reload"
        // property expressed as a cache hit rather than as a comment.
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

    /// Choose schemas for one run, sized to the in-hand model's context window.
    /// The full schema set can exceed a small local
    /// model's whole window — so when the schemas would eat more than ~40% of
    /// `window` we drop to the essential coding/memory loop
    /// ([`is_essential_tool`]). Trimmed tools stay dispatchable by name and via
    /// `tool_search`; they're only removed from the always-sent schema list so a
    /// request can't overflow on schemas alone. In automatic mode, bounded
    /// turns use the essential set even on large-window models:
    /// they resend the schema payload every hop and can recover every hidden
    /// capability through `tool_search`, which returns the matching full
    /// schemas. Local unbounded interactive turns retain the complete set unless
    /// the context window itself is tight. Metered routes use
    /// [`Self::defs_for_driver_turn`] and default to the lean set even with a
    /// roomy window, because their schema payload is paid and resent per hop.
    /// `ANGEL_TOOL_SCHEMA_PROFILE=essential` forces the discoverable lean set
    /// for latency/cost-sensitive action loops; `full` forces every advertised
    /// schema; `auto` or unset retains the context-window heuristic.
    #[cfg(test)]
    pub fn defs_for_run(&self, window: Option<usize>, bounded_task: bool) -> Vec<ToolDef> {
        self.defs_for_turn(window, bounded_task, false)
    }

    /// Schema set for one turn. `competition` is additional: default
    /// interactive stays full unless the window is tight.
    pub(crate) fn defs_for_turn(
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
            && full_tokens > w * 2 / 5
        {
            return self.lean_defs(full.as_ref(), false);
        }
        self.with_activated_tools(full.as_ref().clone())
    }

    /// Route-aware schema policy used by the real driver loop. Explicit `full`
    /// remains authoritative; automatic metered routes start from the compact
    /// coding core plus the sticky per-turn bubble.
    pub(crate) fn defs_for_driver_turn(
        &self,
        window: Option<usize>,
        bounded_task: bool,
        competition: bool,
        metered_sota: bool,
    ) -> Vec<ToolDef> {
        if metered_sota && tool_schema_profile() == ToolSchemaProfile::Auto {
            let (full, _) = self.defs_arc_and_tokens();
            return self.lean_defs(full.as_ref(), true);
        }
        self.defs_for_turn(window, bounded_task, competition)
    }

    fn lean_defs(&self, full: &[ToolDef], coding_hot_path: bool) -> Vec<ToolDef> {
        self.with_activated_tools(
            full.iter()
                .filter(|definition| {
                    if coding_hot_path {
                        is_coding_hot_path_tool(&definition.name)
                    } else {
                        is_essential_tool(&definition.name)
                    }
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
        // A test run in task mode is held to the test-run budget whichever tool
        // starts it. Through `shell` or `cargo` a suite spinning on an infinite
        // loop otherwise keeps the 900 s busy ceiling, past the task's wall
        // (polyglot-v1 py-forth: `python3 -m pytest` via shell, 590 s).
        let call = ToolCall {
            id: String::new(),
            name: name.to_string(),
            args: args.clone(),
        };
        let budget = super::turn::is_verification_call(&call)
            .then(crate::agent::tools::build::test_run_budget)
            .flatten();
        let result = super::exec::with_call_budget(budget, || {
            self.dispatch_within_budget(name, args, cancel, progress)
        });
        match budget {
            Some(budget) => result
                .map(|text| budgeted_test_report(budget, text))
                .map_err(|text| budgeted_test_report(budget, text)),
            None => result,
        }
    }

    fn dispatch_within_budget(
        &self,
        name: &str,
        args: &Value,
        cancel: Option<&AtomicBool>,
        progress: Option<Arc<ToolOutputProgress>>,
    ) -> Result<String, String> {
        if let Some(error) = crate::agent::club::invalid_tool_args_error(args) {
            return Err(format!("{error}; reissue `{name}` with valid JSON"));
        }
        // Loop-worker scoping: while a competition loop worker holds the
        // LOOP_WORKER_SEAT grant, only that package's allowlist dispatches.
        // The denial is a receipt naming the seat and package — the worker
        // sees exactly what was refused and why, then continues on a
        // permitted action. It never blocks, interrupts, or asks permission.
        if self.loop_worker_scoped() && !self.loop_worker_allows(name) {
            let package = crate::agent::harness::comp_packages::active_package();
            return Err(format!(
                "policy denied tool {name}: deny:tool:{name} (seat loop_worker, package {}); \
choose a permitted action and continue",
                package.id
            ));
        }
        // §3.2.3: policy is consulted *here*, at invocation, so a grant tightened
        // mid-session binds the very next call with no reload. Checked before the
        // shell re-route below, so a denied command cannot be rewritten into a
        // different call and slip past the table.
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
                let result = self.dispatch_with_cancel_and_progress(
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
                let presented = route.present(text);
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
                // Panic-isolate the tool boundary. A tool — or any library it
                // calls — must never unwind through the turn loop and take down
                // the cockpit. A panic is folded into a plain `tool error:`
                // result (the same shape as a returned Err), so the existing
                // error classification (`is_error_result`) and the error-nudge
                // machinery see it and the agent can recover on the next hop
                // instead of losing the whole session.
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
        // Record declared paths even on partial-write errors; Git also observes
        // shell/formatter mutations before each Rust check.
        self.mutation_targets.record(name, args);
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

    /// Names of the tools `code_mode` should bind as host functions: every
    /// registered tool except `code_mode` itself (no self-recursion) and the
    /// `tool_search` discovery shim (meaningless inside a script). Deferred tools
    /// are *included* — a script can call them by name even while they're hidden
    /// from the advertised schema list. Order matches registration.
    pub fn bindable_tool_names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|t| t.name())
            .filter(|n| *n != "code_mode" && *n != "tool_search")
            .map(|n| n.to_string())
            .collect()
    }
}

/// One policy denial, as recorded for the operator (§3.2.3 Def 27): what was
/// refused, under which policy, for which seat. A denial that does not name its
/// policy is unauditable, so the receipt carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PolicyDenial {
    pub seat: Option<String>,
    pub tool: String,
    pub policy: String,
}

/// Receipts are for the session, not an archive.
const POLICY_DENIAL_LOG: usize = 256;

/// The reserved seat every other seat's table is merged under: the root context.
pub(crate) const ROOT_SEAT: &str = "root";

/// A seat grant held for a scope: dropping it revokes the grant, so an unwinding
/// context cannot leave its own attenuation behind (T1's inverse discipline — the
/// release is held by the runtime, not by the caller remembering to undo it).
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
    /// Bind the loop-worker allowlist for the duration of one worker's run.
    /// `package` names the owning competition family for denial receipts.
    pub(crate) fn bind_loop_worker_scope_static(
        &self,
        package: &'static crate::agent::harness::comp_packages::CompetitionPackage,
        profile: &'static crate::agent::harness::comp_packages::WorkerProfile,
    ) {
        self.bind_loop_worker_scope(package.id, profile.allowed_tools);
    }

    pub(crate) fn bind_loop_worker_scope(
        &self,
        package: &'static str,
        allowed: &'static [&'static str],
    ) {
        *self
            .loop_worker_allowlist
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some((package, allowed));
    }

    /// Release the loop-worker allowlist (worker finished or unwound).
    pub(crate) fn release_loop_worker_scope(&self) {
        *self
            .loop_worker_allowlist
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn loop_worker_scoped(&self) -> bool {
        self.loop_worker_allowlist
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    fn loop_worker_allows(&self, name: &str) -> bool {
        self.loop_worker_allowlist
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .map(|(_, allowed)| allowed.contains(&name))
            .unwrap_or(true)
    }

    /// Grant (or retune) a seat's narrowing table.
    pub(crate) fn grant_seat(&self, seat: &str, grant: Interception) {
        self.seat_grants
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(seat.to_string(), grant);
    }

    /// Drop a seat's grant: it falls back to the root table alone (the monoid
    /// identity on the seat side).
    pub(crate) fn revoke_seat(&self, seat: &str) {
        self.seat_grants
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(seat);
    }

    /// The table a context actually runs under: `grant(root) ⋈ grant(seat)` — or the
    /// root grant alone for the root context and for any seat without its own grant.
    /// A table write, never a re-registration, so a retune binds the next call.
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

    /// Consult the effective table for one invocation; a denial is recorded as a
    /// receipt that names its policy and its seat.
    fn invocation_denial(&self, seat: Option<&str>, name: &str) -> Option<String> {
        let table = self.effective_interception(seat);
        if table.is_empty() {
            return None;
        }
        let policy = table.consult(name).denial()?.to_string();
        Some(self.record_policy_denial(seat, name, &policy))
    }

    /// Invoke `name` on behalf of `seat`: the seat's table is consulted first, then
    /// the call runs through the ordinary path (whose own consult sees the ancestor
    /// grant, which the seat's table already includes — denials are monotone, so
    /// re-checking cannot widen anything).
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

/// Register the file toolset (`read_file`, `write_file`, `str_replace`,
/// `list_dir`) confined to `root`.
///
/// The four mutation tools — and only those four — are wrapped in The Cut's
/// capture ([`crate::knowledge::cut::capture_writes`]): this is the single write path in
/// the process, so wrapping it here is what makes "everything angel authored"
/// a claim rather than a hope. The wrapper is transparent (same name, schema,
/// and result), so dispatch, `code_mode` bindings, and the essential-tool trim
/// are unaffected. Plan: docs/plans/the-cut.md (T1).
/// A provider that is live in `Σ`, with the release that takes it out of service.
///
/// §5.1.3 asks for **inertial teardown**: mark the provider out of service, then
/// let its dependents drain, *then* reap it. Keeping the store change and the
/// physical teardown separate is what makes that order expressible — and the order
/// is what an owner gets for free when it releases the lease first and drops the
/// process second.
pub struct ProviderLease {
    key: coeffect::Key,
    store: Arc<std::sync::Mutex<coeffect::CoeffectStore>>,
    activations: Arc<super::recall::ToolActivations>,
    released: Arc<Disposable>,
}

impl ProviderLease {
    /// The key this provider occupies in `Σ`.
    #[allow(dead_code)]
    pub fn key(&self) -> &coeffect::Key {
        &self.key
    }

    /// Take the provider out of service: one change to `Σ`, then the reaction of
    /// every tool that declared it. Returns the classification log, empty when the
    /// lease had already been released.
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

    /// The same release as an inverse, so a provider's teardown can carry it.
    #[allow(dead_code)]
    pub fn as_disposable(self: &Arc<Self>) -> Arc<Disposable> {
        let lease = Arc::clone(self);
        Arc::new(Disposable::new(move || {
            lease.release();
        }))
    }
}

impl Drop for ProviderLease {
    /// The last hand drops the provider. An owner that keeps the lease (as
    /// `bootstrap` does, by moving it into the registration scope's inverse) sees
    /// no difference; an owner that lets it go without an explicit release would
    /// otherwise leave a dead provider bound in `Σ` — advertised, with nothing
    /// behind it. `Disposable` is once-only, so releasing first and dropping later
    /// stays a no-op.
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

/// The tools always advertised, even on a tiny-context model — the core
/// read/edit/shell/search loop plus memory + tool discovery. Everything else is
/// dropped from the always-sent schema set when the window can't afford it (still
/// dispatchable by name and discoverable via `tool_search`). Kept deliberately
/// small so the advertised schemas fit an 8K-class local model with room to work.
pub(crate) fn is_essential_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
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
    )
}

/// Bounded / competition / forced-essential advertised set. Orchestration
/// adapters (`swarm_compile`), the skill-name catalog, session-memory
/// (`handoff`, `recall`), glob inventory (`find_files`), the context
/// gauge, batched `multi_edit` (str_replace + apply_patch stay), and
/// `outline` (grep + read_file cover the map) remain on the tiny-window
/// essential list (team 8K routes) and stay discoverable via `tool_search`
/// on the coding hop path.
/// Short advertised `apply_patch` blurb for lean hops. The hashline/envelope
/// essay stays on the default interactive set.
pub(crate) fn lean_apply_patch_description() -> &'static str {
    "Apply a workspace patch: unified diff, *** Begin Patch envelope, or \
     hashline [path#tag] line ops (SWAP/INS/DEL/REM/MV). Confined and \
     preflight-checked. Hashline stage=true queues the plan for resolve_edit. \
     Receipts return the new tag."
}

/// Short advertised `apply_patch` param blurbs for lean hops. Required keys
/// stay identical to the full schema; the hashline stage/resolve essay does
/// not.
pub(crate) fn lean_apply_patch_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "diff": { "type": "string", "description": "unified, envelope, or hashline patch" },
            "strip": { "type": "integer", "description": "path strip -pN (default 1)" },
            "stage": {
                "type": "boolean",
                "description": "hashline: stage without writing"
            },
        },
        "required": ["diff"],
    })
}

/// Short advertised `read_file` blurb for lean hops. The virtual-URL /
/// hashline-recovery essay stays on the default interactive set.
pub(crate) fn lean_read_file_description() -> &'static str {
    "Read one bounded UTF-8 workspace page. offset is 1-based (default 1); \
     limit is complete lines (default 200, max 400). Truncation names the next \
     offset. Pages are headed [path#tag] for hashline apply_patch. Also \
     conflict://, skill://, agent://, outline://, pr://, issue://."
}

/// Short advertised `read_file` param blurbs for lean hops. Numeric bounds
/// stay identical to the full schema; the virtual-URL / truncation essay
/// does not.
pub(crate) fn lean_read_file_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "workspace path or virtual URL" },
            "offset": {
                "type": "integer",
                "minimum": 1,
                "description": "1-based first line (default 1)"
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 400,
                "description": "lines to return (default 200, max 400)"
            }
        },
        "required": ["path"],
    })
}

/// Short advertised `write_file` blurb for lean hops. The conflict-resolve
/// essay stays on the default interactive set.
pub(crate) fn lean_write_file_description() -> &'static str {
    "Create or overwrite a workspace text file (parents created). Returns the \
     new content tag. Also conflict://N with @ours/@theirs/@base/@both or a \
     custom marker-region body."
}

/// Short advertised `write_file` param blurbs for lean hops. Required keys
/// stay identical to the full schema; the conflict://* / resolve essay does
/// not.
pub(crate) fn lean_write_file_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "workspace path or conflict://N" },
            "content": {
                "type": "string",
                "description": "file contents or @ours/@theirs/@base/@both"
            },
        },
        "required": ["path", "content"],
    })
}

/// Short advertised `shell` blurb for lean hops. The sandbox / sudo /
/// package-manager essay stays on the default interactive set.
pub(crate) fn lean_shell_description() -> &'static str {
    "Run bash -c with pipefail; returns stdout+stderr. Omit scope options for builds, benchmarks, \
     installs and background jobs. read_only and write_paths restrict all children, temporary \
     files and network; write_paths is NOT an output-file list. Act on actual errors: keep the \
     earliest prerequisite failure, check usable input before dependent measurements, use allowed \
     scratch, and do not score failed input as zero performance. Explicit restrictions stay \
     authoritative; request a user-visible scope change instead of omitting them."
}

/// Short advertised `tool_search` blurb for lean hops. The deferred-count /
/// "not in your base tool list" essay stays on the default interactive set.
pub(crate) fn lean_tool_search_description() -> &'static str {
    "Search additional tools by capability. Matching native schemas activate on the next request."
}

/// Short advertised `list_dir` blurb for lean hops. The default-root /
/// suffix essay stays on the default interactive set.
pub(crate) fn lean_list_dir_description() -> &'static str {
    "List directory entries; optional hint/pattern ranks first. Truncated pages report counts and next offset. Directories end with /."
}

/// Short advertised `list_dir` param blurbs for lean hops. Required keys
/// stay identical to the full schema; the workspace-relative essay does not.
pub(crate) fn lean_list_dir_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "directory (default '.')" },
            "hint": { "type": "string", "description": "filename fragment to rank first" },
            "pattern": { "type": "string", "description": "basename glob to rank first" },
            "offset": { "type": "integer", "minimum": 0, "description": "sorted entry offset (default 0)" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 700, "description": "page entries (default 700); 24000-byte window" },
            "no_ignore": { "type": "boolean", "description": "include ignored paths; hard exclusions remain" },
            "hidden": { "type": "boolean", "description": "include hidden paths; hard exclusions remain" },
        },
        "required": [],
    })
}

/// Short advertised `str_replace` blurb for lean hops. The whitespace /
/// CRLF / smart-quote fallback essay stays on the default interactive set.
pub(crate) fn lean_str_replace_description() -> &'static str {
    "Replace an exact unique substring in a workspace file. Fails if 'old' is \
     missing or not unique. Returns the new content tag."
}

/// Short advertised `str_replace` param blurbs for lean hops. Required keys
/// stay identical to the full schema; the stale-edit / [path#tag] essay does
/// not.
pub(crate) fn lean_str_replace_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "workspace path" },
            "old": { "type": "string", "description": "exact unique text to replace" },
            "new": { "type": "string", "description": "replacement text" },
            "expect_tag": {
                "type": "string",
                "description": "optional [path#tag] stale-edit guard"
            },
        },
        "required": ["path", "old", "new"],
    })
}

/// Short advertised `grep` blurb for lean hops. The diversity / ignore-rule
/// essay stays on the default interactive set.
pub(crate) fn lean_grep_description() -> &'static str {
    "Search up to eight workspace paths for a regex. Honors gitignore; skips \
     hidden/credential/quarantine files. Receipts include after_file when \
     another page exists."
}

/// Short advertised `grep` param blurbs for lean hops. Numeric bounds stay
/// identical to the full schema; the continuation/union essay does not.
pub(crate) fn lean_grep_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": { "type": "string", "description": "regular expression" },
            "path": { "type": "string", "description": "file or directory (default '.')" },
            "paths": {
                "type": "array",
                "minItems": 1,
                "maxItems": 8,
                "items": { "type": "string" },
                "description": "1-8 paths; exclusive with path"
            },
            "after_file": {
                "type": "string",
                "description": "continuation cursor from a prior receipt"
            },
            "skip_files": {
                "type": "array",
                "maxItems": 128,
                "items": { "type": "string" },
                "description": "paths to omit (max 128)"
            },
            "ignore_case": { "type": "boolean", "description": "case-insensitive (default false)" },
            "context": {
                "type": "integer",
                "minimum": 0,
                "maximum": 10,
                "description": "lines around each match (0-10)"
            },
        },
        "required": ["pattern"],
    })
}

/// Slim advertised schemas on lean hops. Default interactive keeps full defs.
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
            "read_file" => lean_read_file_params(),
            "write_file" => lean_write_file_params(),
            "str_replace" => lean_str_replace_params(),
            "apply_patch" => lean_apply_patch_params(),
            "code_mode" => lean_code_mode_params(),
            "list_dir" => lean_list_dir_params(),
            _ => definition.params.clone(),
        },
    }
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

/// `ANGEL_SKILL_HINT` is launch config. The first hop of every turn consults
/// it; product reads once. Tests keep the live getenv so EnvGuard stays visible.
fn skill_hint_enabled() -> bool {
    #[cfg(not(test))]
    {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| env_flag("ANGEL_SKILL_HINT", true))
    }
    #[cfg(test)]
    env_flag("ANGEL_SKILL_HINT", true)
}

/// A test run killed at its budget reads as a hang the model can act on, not a
/// plain timeout. Other results pass through untouched.
fn budgeted_test_report(budget: std::time::Duration, text: String) -> String {
    // Only a kill counts: a slow suite that finished still prints Rust's
    // "running for over 60 seconds" lines.
    let killed_at_budget = text.to_ascii_lowercase().contains("timed out after");
    if killed_at_budget && !text.starts_with("tests: still running after") {
        crate::agent::tools::build::hung_suite_report(budget, &text)
    } else {
        text
    }
}
