//! FallbackClub link failover and SOTA MoA role selection.

use super::*;

mod preferences;
pub(crate) use preferences::{MoaPreferences, moa_preferences};
#[cfg(test)]
pub(crate) use preferences::{baseline_preferences, test_preferences};

type GatedClub = (Arc<dyn Club>, Option<Arc<AtomicBool>>);

// ---------------------------------------------------------------------------
// Bag — a set of clubs with one in hand
// ---------------------------------------------------------------------------

/// An ordered failover chain of clubs. Tries the primary; if it errors *before
/// emitting any output* (endpoint down, connect timeout, a fatal status like a
/// key-less 401), it transparently falls through to the next club — the eager
/// failover Hermes ships — so one dead endpoint doesn't sink the turn. A stream
/// that has already emitted tokens can't be replayed, so a mid-stream failure
/// surfaces as-is; a user cancel is honored immediately and never treated as a
/// failure to route around. Reports the primary's label so the UI is unchanged.
pub struct FallbackClub {
    label: String,
    chain: Vec<Arc<dyn Club>>,
    /// Index-parallel to `chain`: the prober-maintained availability bit for
    /// links that have one (`None` = always try). A dead LAN box used to cost
    /// its full connect-timeout × retries inside the chain walk even though the
    /// prober already knew it was down (refreshed every ~5s for free).
    gates: Vec<Option<Arc<AtomicBool>>>,
    resolved: Mutex<Option<RouteIdentity>>,
}

impl FallbackClub {
    /// Ungated chain — every link is always tried. Production builds the gated
    /// form via [`Self::with_gates`]; tests exercising pure walk order use this.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(chain: Vec<Arc<dyn Club>>) -> Self {
        let gates = vec![None; chain.len()];
        Self::with_gates(chain, gates)
    }

    pub fn with_gates(chain: Vec<Arc<dyn Club>>, gates: Vec<Option<Arc<AtomicBool>>>) -> Self {
        let label = chain
            .first()
            .map(|c| c.label().to_string())
            .unwrap_or_else(|| "fallback".to_string());
        debug_assert_eq!(chain.len(), gates.len());
        Self {
            label,
            chain,
            gates,
            resolved: Mutex::new(None),
        }
    }

    /// The chain filtered by live availability bits. If the prober currently
    /// marks *everything* down (a stale sweep, a cold start), fall back to the
    /// whole chain — the bits are an optimization, never a brick. This is the
    /// oracle the `seat-order` leg is checked against: test builds only.
    #[cfg(test)]
    fn live_chain(&self) -> Vec<&Arc<dyn Club>> {
        let live: Vec<&Arc<dyn Club>> = self
            .chain
            .iter()
            .zip(&self.gates)
            .filter(|(_, gate)| gate.as_ref().is_none_or(|g| g.load(Ordering::Relaxed)))
            .map(|(club, _)| club)
            .collect();
        if live.is_empty() {
            self.chain.iter().collect()
        } else {
            live
        }
    }

    /// The leg's parity oracle: test builds only.
    #[cfg(test)]
    fn compatible_chat_chain(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
    ) -> Vec<&Arc<dyn Club>> {
        let required_tokens = required_tokens(messages, tools);
        self.live_chain()
            .into_iter()
            .filter(|club| {
                club.metadata().is_none_or(|metadata| {
                    (tools.is_empty() || metadata.supports_tools)
                        && (metadata.context_window == 0
                            || metadata.context_window > required_tokens + 256)
                })
            })
            .collect()
    }

    fn note_transition(chain: &[&Arc<dyn Club>], index: usize, reason: &str) {
        if let Some(next) = chain.get(index + 1) {
            crate::knowledge::experience::note_failover(chain[index].label(), next.label(), reason);
        }
    }

    fn note_resolved(&self, club: &dyn Club) {
        crate::agent::harness::run_identity::publish_answer_route(
            self as *const Self as usize,
            club.resolved_route_identity(),
        );
        if let Ok(mut resolved) = self.resolved.lock() {
            *resolved = Some(club.resolved_route_identity());
        }
    }

    /// The per-link features a walk decision is made from: gate presence and
    /// position, and the detected metadata `compatible_chat_chain` consults.
    fn walk_input(
        &self,
        chat: bool,
        messages: &[ChatMsg],
        tools: &[ToolDef],
    ) -> serde_json::Value {
        // Only the links this walk can reach are worth a probe, exactly as
        // before: an unreachable link reports whatever is already cached, so a
        // shut gate costs nothing and the native read pattern is unchanged.
        let open: Vec<bool> = (0..self.chain.len())
            .map(|index| {
                self.gates
                    .get(index)
                    .and_then(|gate| gate.as_ref())
                    .is_none_or(|gate| gate.load(Ordering::Relaxed))
            })
            .collect();
        let any_open = open.iter().any(|open| *open);
        let metadata: Vec<Option<Metadata>> = self
            .chain
            .iter()
            .enumerate()
            .map(|(index, club)| {
                if any_open && !open[index] {
                    club.metadata_cached()
                } else {
                    club.metadata()
                }
            })
            .collect();
        let chain: Vec<serde_json::Value> = (0..self.chain.len())
            .map(|index| {
                let gate = self.gates.get(index).and_then(|gate| gate.as_ref());
                let detected = metadata[index];
                serde_json::json!({
                    "index": index,
                    "gated": gate.is_some(),
                    "gate_open": gate.is_some_and(|gate| gate.load(Ordering::Relaxed)),
                    "has_metadata": detected.is_some(),
                    "supports_tools": detected.is_some_and(|metadata| metadata.supports_tools),
                    "context_window": detected.map_or(0, |metadata| metadata.context_window),
                })
            })
            .collect();
        serde_json::json!({
            "mode": "walk",
            "chat": chat,
            "has_tools": !tools.is_empty(),
            "required_tokens": required_tokens(messages, tools).min(u32::MAX as usize),
            "chain": chain,
        })
    }

    /// The chain positions this walk will attempt, as decided by the
    /// `seat-order` leg (newest live generation, else the shipped one): a
    /// generation that names an attempt outside the chain, or out of order,
    /// keeps the shipped walk.
    fn walk_plan(
        &self,
        chat: bool,
        messages: &[ChatMsg],
        tools: &[ToolDef],
    ) -> Vec<usize> {
        self.walk_plan_decided(chat, messages, tools).0
    }

    /// The same decision with its source, so a test can see which generation
    /// answered when no live one was offered.
    fn walk_plan_decided(
        &self,
        chat: bool,
        messages: &[ChatMsg],
        tools: &[ToolDef],
    ) -> (Vec<usize>, crate::agent::harness::live_policy::LegSource) {
        let input = self.walk_input(chat, messages, tools);
        let limit = self.chain.len();
        let (plan, source) = crate::agent::harness::live_policy::decide_shipped::<SeatPlan>(
            SEAT_ORDER_LEG,
            &input,
            move |plan| seat_plan_check(&plan.plan, limit, true),
        );
        (plan.plan, source)
    }

    /// The plan's club handles, for the walk and its transition notes.
    fn plan_chain(&self, plan: &[usize]) -> Vec<&Arc<dyn Club>> {
        plan.iter()
            .filter_map(|&index| self.chain.get(index))
            .collect()
    }

    /// A failed attempt: note the transition and ask the `seat-order` leg which
    /// link is next. `abort` carries the attempt's own error back untouched —
    /// the stream already spoke, or the user cancelled.
    fn after_failure<E>(
        &self,
        chain: &[&Arc<dyn Club>],
        step: usize,
        failure: String,
        streaming: bool,
        emitted: bool,
        cancelled: bool,
    ) -> Walk<E> {
        let decided = self.next_step(step, chain.len(), streaming, emitted, cancelled);
        if decided.abort {
            // A partially streamed reply or a cancel surfaces this attempt's own
            // error, unprefixed: it is not a chain failure to route around.
            return Walk::Stop(Err(failure));
        }
        Self::note_transition(chain, step, &failure);
        let error = Err(format!("[{}] {failure}", chain[step].label()));
        if decided.proceed {
            Walk::Retry {
                error,
                next: decided.next,
            }
        } else {
            Walk::Stop(error)
        }
    }

    /// Today's walk, unchanged, as the leg's parity oracle: test builds only.
    #[cfg(test)]
    fn walk_plan_native(
        &self,
        chat: bool,
        messages: &[ChatMsg],
        tools: &[ToolDef],
    ) -> Vec<usize> {
        let chain = if chat {
            self.compatible_chat_chain(messages, tools)
        } else {
            self.live_chain()
        };
        chain
            .iter()
            .filter_map(|club| {
                self.chain
                    .iter()
                    .position(|candidate| Arc::ptr_eq(candidate, club))
            })
            .collect()
    }

    /// Which link to try after a failure, as decided by the `seat-order` leg:
    /// continue with `next`, stop and report the last failure, or abort the
    /// attempt's own error because the stream already spoke or the user
    /// cancelled. A generation that does not move forward inside the plan keeps
    /// the shipped step.
    fn next_step(
        &self,
        step: usize,
        plan_len: usize,
        streaming: bool,
        emitted: bool,
        cancelled: bool,
    ) -> AdvanceStep {
        let input = serde_json::json!({
            "mode": "advance",
            "step": step,
            "plan_len": plan_len,
            "streaming": streaming,
            "emitted": emitted,
            "cancelled": cancelled,
        });
        let (decision, _source) =
            crate::agent::harness::live_policy::decide_shipped::<AdvanceStep>(
                SEAT_ORDER_LEG,
                &input,
                move |decision| advance_check(decision, step, plan_len),
            );
        decision
    }
}

impl Club for FallbackClub {
    fn supports_formation_budget(&self) -> bool {
        self.chain
            .iter()
            .all(|club| club.supports_formation_budget())
    }
    fn bind_run_identity(&self, _effort: Option<&str>) -> Result<(), String> {
        Ok(())
    }

    fn label(&self) -> &str {
        &self.label
    }

    /// Per-club knobs address the primary, for the same reason its metadata is
    /// the one reported: it is the club that answers unless the chain moves.
    fn env_namespace(&self) -> Option<&str> {
        self.chain.first().and_then(|c| c.env_namespace())
    }

    /// Report the primary's metadata — compaction budgets against whichever club
    /// would actually answer first, and the UI label is already the primary's.
    fn metadata(&self) -> Option<Metadata> {
        self.chain.first().and_then(|c| c.metadata())
    }

    fn metadata_cached(&self) -> Option<Metadata> {
        self.chain.first().and_then(|c| c.metadata_cached())
    }

    fn model_identity(&self) -> Option<String> {
        self.chain.first().and_then(|club| club.model_identity())
    }

    fn reasoning_effort(&self) -> Option<String> {
        self.chain.first().and_then(|club| club.reasoning_effort())
    }

    fn reasoning_levels(&self) -> &[String] {
        self.chain
            .first()
            .map(|club| club.reasoning_levels())
            .unwrap_or(&[])
    }

    /// Validate the effort on the primary first; only if it accepts
    /// (returns Some) broadcast that canonical value to every remaining chain
    /// member. A rejection anywhere else is ignored — members keep their own
    /// state and only the primary's answer is authoritative.
    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let canonical = self.chain.first()?.set_reasoning_effort(requested)?;
        for club in self.chain.iter().skip(1) {
            club.set_reasoning_effort(&canonical);
        }
        Some(canonical)
    }

    fn resolved_route_identity(&self) -> RouteIdentity {
        crate::agent::harness::run_identity::answer_route(self as *const Self as usize)
            .unwrap_or_default()
    }

    fn resolved_route_if_known(&self) -> Option<RouteIdentity> {
        self.resolved
            .try_lock()
            .ok()
            .and_then(|route| route.clone())
    }

    fn route_metadata(&self) -> RouteMetadata {
        self.chain
            .first()
            .map(|club| club.route_metadata())
            .unwrap_or_default()
    }

    fn header_route_metadata(&self) -> RouteMetadata {
        self.chain
            .first()
            .map(|club| club.header_route_metadata())
            .unwrap_or_default()
    }

    fn route_state_revision(&self) -> u64 {
        self.chain
            .first()
            .map(|club| club.route_state_revision())
            .unwrap_or(0)
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        crate::agent::harness::run_identity::publish_answer_route(
            self as *const Self as usize,
            Default::default(),
        );
        let plan = self.walk_plan(false, &[], &[]);
        let chain = self.plan_chain(&plan);
        let mut last = Err("fallback chain is empty".to_string());
        let mut step = 0;
        while step < chain.len() {
            let club = chain[step];
            match club.respond(prompt) {
                Ok(v) => {
                    self.note_resolved(&**club);
                    return Ok(v);
                }
                Err(e) => {
                    match self.after_failure(&chain, step, e, false, false, false) {
                        Walk::Retry { error, next } => {
                            last = error;
                            step = next;
                        }
                        Walk::Stop(error) => return error,
                    }
                }
            }
        }
        last
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        crate::agent::harness::run_identity::publish_answer_route(
            self as *const Self as usize,
            Default::default(),
        );
        let plan = self.walk_plan(true, messages, tools);
        let chain = self.plan_chain(&plan);
        let mut last = Err("fallback chain is empty".to_string());
        let mut step = 0;
        while step < chain.len() {
            let club = chain[step];
            match club.chat(messages, tools) {
                Ok(v) => {
                    self.note_resolved(&**club);
                    return Ok(v);
                }
                Err(e) => {
                    match self.after_failure(&chain, step, e, false, false, false) {
                        Walk::Retry { error, next } => {
                            last = error;
                            step = next;
                        }
                        Walk::Stop(error) => return error,
                    }
                }
            }
        }
        last
    }

    /// Mirror of [`Club::chat`] threading a seat's per-call effort request to
    /// whichever chain member actually answers — a failed-over seat keeps its
    /// roster effort instead of silently reverting to the member's own state.
    fn chat_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        crate::agent::harness::run_identity::publish_answer_route(
            self as *const Self as usize,
            Default::default(),
        );
        let plan = self.walk_plan(true, messages, tools);
        let chain = self.plan_chain(&plan);
        let mut last = Err("fallback chain is empty".to_string());
        let mut step = 0;
        while step < chain.len() {
            let club = chain[step];
            match club.chat_with_effort(messages, tools, effort) {
                Ok(v) => {
                    self.note_resolved(&**club);
                    return Ok(v);
                }
                Err(e) => {
                    match self.after_failure(&chain, step, e, false, false, false) {
                        Walk::Retry { error, next } => {
                            last = error;
                            step = next;
                        }
                        Walk::Stop(error) => return error,
                    }
                }
            }
        }
        last
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.chat_streaming_with_effort(messages, tools, None, cancel, on_delta)
    }

    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        crate::agent::harness::run_identity::publish_answer_route(
            self as *const Self as usize,
            Default::default(),
        );
        let plan = self.walk_plan(true, messages, tools);
        let chain = self.plan_chain(&plan);
        let mut last = Err("fallback chain is empty".to_string());
        let mut step = 0;
        while step < chain.len() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".to_string());
            }
            let club = chain[step];
            let mut emitted = false;
            let mut wrapped = |d: StreamDelta| {
                // Only visible content counts as a committed partial reply (a club
                // that streamed only reasoning then failed can still fall through).
                if matches!(d, StreamDelta::Content(_)) {
                    emitted = true;
                }
                on_delta(d);
            };
            match club.chat_streaming_with_effort(messages, tools, effort, cancel, &mut wrapped) {
                Ok(v) => {
                    self.note_resolved(&**club);
                    return Ok(v);
                }
                Err(e) => {
                    // A partially-streamed reply can't be replayed onto the next
                    // club, and a cancel isn't a failure worth routing around.
                    let cancelled = cancel.load(Ordering::Relaxed);
                    match self.after_failure(&chain, step, e, true, emitted, cancelled) {
                        Walk::Retry { error, next } => {
                            last = error;
                            step = next;
                        }
                        Walk::Stop(error) => return error,
                    }
                }
            }
        }
        last
    }

    fn usage_accounting(&self) -> super::AccountingView {
        let mut view = super::AccountingView::default();
        for club in &self.chain {
            view.extend(club.usage_accounting());
        }
        view
    }

    fn token_usage(&self) -> Option<TokenUsage> {
        let mut total = TokenUsage::default();
        let mut any = false;
        for club in &self.chain {
            let Some(usage) = club.token_usage() else {
                continue;
            };
            any = true;
            total.turns = total.turns.saturating_add(usage.turns);
            total.last_input = total.last_input.saturating_add(usage.last_input);
            total.last_output = total.last_output.saturating_add(usage.last_output);
            total.last_reasoning = total.last_reasoning.saturating_add(usage.last_reasoning);
            total.total_input = total.total_input.saturating_add(usage.total_input);
            total.total_output = total.total_output.saturating_add(usage.total_output);
            total.total_reasoning = total.total_reasoning.saturating_add(usage.total_reasoning);
        }
        any.then_some(total)
    }

    fn cache_usage(&self) -> CacheUsage {
        self.chain
            .iter()
            .fold(CacheUsage::default(), |mut total, club| {
                let usage = club.cache_usage();
                total.control_requests = total
                    .control_requests
                    .saturating_add(usage.control_requests);
                total.read_input_tokens = total
                    .read_input_tokens
                    .saturating_add(usage.read_input_tokens);
                total.write_input_tokens = total
                    .write_input_tokens
                    .saturating_add(usage.write_input_tokens);
                total.read_accounting_responses = total
                    .read_accounting_responses
                    .saturating_add(usage.read_accounting_responses);
                total.write_accounting_responses = total
                    .write_accounting_responses
                    .saturating_add(usage.write_accounting_responses);
                total
            })
    }

    fn truncation_usage(&self) -> TruncationUsage {
        self.chain
            .iter()
            .fold(TruncationUsage::default(), |mut total, club| {
                let usage = club.truncation_usage();
                total.episodes = total.episodes.saturating_add(usage.episodes);
                total.retained_partials = total
                    .retained_partials
                    .saturating_add(usage.retained_partials);
                total.retries = total.retries.saturating_add(usage.retries);
                total.recoveries = total.recoveries.saturating_add(usage.recoveries);
                total.failures = total.failures.saturating_add(usage.failures);
                total
            })
    }

    fn effort_gate_usage(&self) -> EffortGateUsage {
        self.chain
            .iter()
            .fold(EffortGateUsage::default(), |mut total, club| {
                let usage = club.effort_gate_usage();
                total.withheld = total.withheld.saturating_add(usage.withheld);
                total.rejections = total.rejections.saturating_add(usage.rejections);
                if usage.last.is_some() {
                    total.last = usage.last;
                }
                total
            })
    }
}

// Baseline intelligence and cheap-profile orders live in one versioned
// preference generation. Native validation permits reordering only, so the
// cheap pool and its explicit spend boundary cannot grow through an override.

fn cheap_link_allowed(entry: &(String, Arc<dyn Club>, Arc<AtomicBool>)) -> bool {
    let Ok(raw) = std::env::var("ANGEL_SOTA_MOA_CHEAP_LINKS") else {
        return true;
    };
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .any(|wanted| link_matches(entry, wanted))
}

fn cheap_moa_enabled() -> bool {
    std::env::var("ANGEL_SOTA_MOA_COST_PROFILE")
        .map(|value| {
            let v = value.trim().to_ascii_lowercase();
            v == "cheap" || v == "sota_swarm" || v == "sota-swarm"
        })
        .unwrap_or(false)
}

fn link_matches(entry: &(String, Arc<dyn Club>, Arc<AtomicBool>), wanted: &str) -> bool {
    let (alias, club, _) = entry;
    alias.eq_ignore_ascii_case(wanted)
        || club.label().eq_ignore_ascii_case(wanted)
        || club
            .live_model_name()
            .is_some_and(|model| model.eq_ignore_ascii_case(wanted))
}

/// The live policy leg that seats links and steps the failover walk.
const SEAT_ORDER_LEG: &str = "seat-order";

/// The seating/walk decision the `seat-order` leg returns: link positions.
#[derive(Clone, serde::Deserialize)]
struct SeatPlan {
    plan: Vec<usize>,
}

/// One after-failure step, decided by the `seat-order` leg.
#[derive(Clone, serde::Deserialize)]
struct AdvanceStep {
    proceed: bool,
    next: usize,
    abort: bool,
}

/// What a failed attempt means for the chain walk.
enum Walk<E> {
    /// Try the link at `next`; `error` is what the chain reports if nothing else answers.
    Retry {
        error: Result<E, String>,
        next: usize,
    },
    /// Stop the walk and surface `error`: the chain is exhausted, the stream
    /// already spoke, or the user cancelled.
    Stop(Result<E, String>),
}

/// The context a chat's tool schemas and messages owe the model, in tokens.
fn required_tokens(messages: &[ChatMsg], tools: &[ToolDef]) -> usize {
    messages
        .iter()
        .map(|message| message.content.chars().count().div_ceil(4) + 8)
        .sum::<usize>()
        + tools
            .iter()
            .map(|tool| {
                (tool.name.len() + tool.description.len() + tool.params.to_string().len())
                    .div_ceil(4)
                    + 8
            })
            .sum::<usize>()
}

/// What a loaded generation may return: positions inside the chain, once each,
/// and in walk order when the decision is a plan.
fn seat_plan_check(plan: &Vec<usize>, limit: usize, ordered: bool) -> Result<(), String> {
    let mut previous = None;
    for (position, &index) in plan.iter().enumerate() {
        if index >= limit {
            return Err(format!("live seat-order named link {index} of {limit}"));
        }
        if plan[..position].contains(&index) {
            return Err(format!("live seat-order named link {index} twice"));
        }
        if ordered && previous.is_some_and(|previous: usize| index <= previous) {
            return Err("live seat-order reordered the failover chain".into());
        }
        previous = Some(index);
    }
    Ok(())
}

fn advance_check(decision: &AdvanceStep, step: usize, plan_len: usize) -> Result<(), String> {
    if decision.proceed {
        if decision.next <= step || decision.next >= plan_len {
            return Err("live seat-order stepped outside the remaining chain".into());
        }
    } else if decision.next != 0 {
        return Err("live seat-order stopped on a non-zero link".into());
    }
    Ok(())
}

/// The per-link features a seating decision is made from. Both text questions
/// stay here: which cheap-order slots a link matches, and whether the allowlist
/// admits it. Links sharing an alias share a group id, which is the seating's
/// case-insensitive de-duplication.
#[cfg(test)]
const SOTA_MOA_CHEAP_FALLBACK_ORDER: &[&str] = &[
    "deepseek-v4-flash",
    "deepseek-flash",
    "glm",
    "glm-5.3-flash",
    "glm-5.2",
    "chatgpt-luna-on-high",
    "luna-on-high",
    "luna",
    "openrouter",
    "stealth/ox-alpha",
    "thinkingmachines/inkling:free",
    "thinkingmachines/inkling-small:free",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "openrouter/free",
    "poolside/laguna-s-2.1:free",
    "cohere/north-mini-code:free",
    "z-ai/glm-5.2:free",
    "longcat",
];

#[cfg(test)]
fn seat_order_input(links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)]) -> serde_json::Value {
    seat_order_input_with(links, &moa_preferences())
}

fn seat_order_input_with(links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)], preferences: &MoaPreferences) -> serde_json::Value {
    let features: Vec<serde_json::Value> = links
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let matches: Vec<usize> = preferences.cheap_fallback
                .iter()
                .enumerate()
                .filter(|(_, wanted)| link_matches(entry, wanted))
                .map(|(slot, _)| slot)
                .collect();
            let group = links
                .iter()
                .position(|earlier| earlier.0.eq_ignore_ascii_case(&entry.0))
                .unwrap_or(index);
            serde_json::json!({
                "index": index,
                "group": group,
                "allowed": cheap_link_allowed(entry),
                "matches": matches,
            })
        })
        .collect();
    serde_json::json!({
        "mode": "order",
        "order": (0..preferences.cheap_fallback.len()).collect::<Vec<usize>>(),
        "links": features,
    })
}

/// The cheap-profile seating, decided by the `seat-order` leg. Rust keeps the
/// club objects, their availability gates and the two text questions (which
/// cheap-order slots a link's alias/label/live model name matches, and whether
/// the allowlist admits it); the leg owns the order and the de-duplication.
pub(crate) fn ordered_cheap_links(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let preferences = moa_preferences();
    ordered_cheap_links_with(links, &preferences)
}

fn ordered_cheap_links_with(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    preferences: &MoaPreferences,
) -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let input = seat_order_input_with(links, preferences);
    let limit = links.len();
    let (plan, _source) = crate::agent::harness::live_policy::decide_shipped::<SeatPlan>(
        SEAT_ORDER_LEG,
        &input,
        move |plan| seat_plan_check(&plan.plan, limit, false),
    );
    plan.plan
        .into_iter()
        .filter_map(|index| links.get(index))
        .map(|(alias, club, gate)| (alias.clone(), Arc::clone(club), Arc::clone(gate)))
        .collect()
}

/// Today's seating as chain positions, which is what the leg returns: the
/// parity oracle for [`ordered_cheap_links`], test builds only.
#[cfg(test)]
fn ordered_cheap_plan_native(links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)]) -> Vec<usize> {
    let mut ordered: Vec<usize> = Vec::new();
    for wanted in &moa_preferences().cheap_fallback {
        for (index, entry) in links
            .iter()
            .enumerate()
            .filter(|(_, entry)| cheap_link_allowed(entry) && link_matches(entry, wanted))
        {
            if !ordered
                .iter()
                .any(|seated| links[*seated].0.eq_ignore_ascii_case(&entry.0))
            {
                ordered.push(index);
            }
        }
    }
    ordered
}

#[cfg(test)]
pub(crate) fn ordered_cheap_links_for_test(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    preferences: &MoaPreferences,
) -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    ordered_cheap_links_with(links, preferences)
}

fn cheap_role_with_fallbacks(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    env_key: &str,
    order: &[String],
    preferences: &MoaPreferences,
) -> Arc<dyn Club> {
    let primary = pick_sota_role(links, env_key, order);
    let mut chain = vec![Arc::clone(&primary)];
    let mut gates = vec![
        links
            .iter()
            .find(|(_, club, _)| Arc::ptr_eq(club, &primary))
            .map(|(_, _, gate)| Arc::clone(gate)),
    ];
    for (_, club, gate) in ordered_cheap_links_with(links, preferences) {
        if chain.iter().any(|candidate| Arc::ptr_eq(candidate, &club)) {
            continue;
        }
        chain.push(club);
        gates.push(Some(gate));
    }
    Arc::new(FallbackClub::with_gates(chain, gates))
}

/// Configured proposer-only seats requested for SOTA-MoA breadth
/// (`ANGEL_SOTA_MOA_EXTRA_PROPOSERS`, off by default). Explicit seats still obey
/// provider opt-in and the cheap allowlist.
pub(crate) fn extra_sota_proposers(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Vec<GatedClub> {
    let requested = std::env::var("ANGEL_SOTA_MOA_EXTRA_PROPOSERS").unwrap_or_default();
    if matches!(
        requested.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "off" | "false" | "none"
    ) {
        return Vec::new();
    }
    let wanted = requested
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    for entry in links {
        if (cheap_moa_enabled() && !cheap_link_allowed(entry))
            || !wanted.iter().any(|name| link_matches(entry, name))
            || out
                .iter()
                .any(|(club, _): &(Arc<dyn Club>, Option<Arc<AtomicBool>>)| {
                    club.label().eq_ignore_ascii_case(entry.1.label())
                })
        {
            continue;
        }
        out.push((Arc::clone(&entry.1), Some(Arc::clone(&entry.2))));
    }
    out
}

pub(crate) fn pick_sota_role<T: AsRef<str>>(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    env_key: &str,
    preferred_aliases: &[T],
) -> Arc<dyn Club> {
    if let Some(wanted) = env_first(&[env_key]) {
        if let Some((_, club, _)) = links.iter().find(|(alias, club, _)| {
            alias.eq_ignore_ascii_case(&wanted)
                || club.label().eq_ignore_ascii_case(&wanted)
                || club
                    .live_model_name()
                    .is_some_and(|model| model.eq_ignore_ascii_case(&wanted))
        }) {
            return Arc::clone(club);
        }
        eprintln!("[sota-moa] {env_key}={wanted:?} did not match a SOTA link");
    }
    for preferred in preferred_aliases {
        let preferred = preferred.as_ref();
        if let Some((_, club, _)) = links.iter().find(|(alias, club, _)| {
            alias.eq_ignore_ascii_case(preferred)
                || club.label().eq_ignore_ascii_case(preferred)
                || club
                    .label()
                    .to_ascii_lowercase()
                    .contains(&preferred.to_ascii_lowercase())
                || club.live_model_name().is_some_and(|model| {
                    model.eq_ignore_ascii_case(preferred)
                        || model
                            .to_ascii_lowercase()
                            .contains(&preferred.to_ascii_lowercase())
                })
        }) {
            return Arc::clone(club);
        }
    }
    Arc::clone(&links[0].1)
}

/// Build the SOTA wrapper over the configured links: the smartest-first
/// formation and its failover order, or the explicit `cheap` cost profile.
pub(crate) fn sota_moa_club(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Option<Arc<dyn Club>> {
    let cheap = cheap_moa_enabled();
    let preferences = moa_preferences();
    let mut role_links = links.to_vec();
    if cheap {
        role_links = ordered_cheap_links_with(&role_links, &preferences);
    }
    if role_links.is_empty() {
        return None;
    }
    if role_links.len() < 2
        && !cheap
        && !std::env::var("ANGEL_SOTA_MOA_ALLOW_SINGLE")
            .map(|v| {
                matches!(
                    v.trim(),
                    "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
                )
            })
            .unwrap_or(false)
    {
        return None;
    }
    let (propose, judge, verify, aggregate) = if cheap {
        (
            cheap_role_with_fallbacks(
                &role_links,
                "ANGEL_SOTA_MOA_PROPOSE_CLUB",
                &preferences.cheap_propose,
                &preferences,
            ),
            cheap_role_with_fallbacks(
                &role_links,
                "ANGEL_SOTA_MOA_JUDGE_CLUB",
                &preferences.cheap_judge,
                &preferences,
            ),
            cheap_role_with_fallbacks(
                &role_links,
                "ANGEL_SOTA_MOA_VERIFY_CLUB",
                &preferences.cheap_verify,
                &preferences,
            ),
            cheap_role_with_fallbacks(
                &role_links,
                "ANGEL_SOTA_MOA_AGG_CLUB",
                &preferences.cheap_aggregate,
                &preferences,
            ),
        )
    } else {
        (
            pick_sota_role(links, "ANGEL_SOTA_MOA_PROPOSE_CLUB", &preferences.standard),
            pick_sota_role(links, "ANGEL_SOTA_MOA_JUDGE_CLUB", &preferences.standard),
            pick_sota_role(links, "ANGEL_SOTA_MOA_VERIFY_CLUB", &preferences.standard),
            pick_sota_role(links, "ANGEL_SOTA_MOA_AGG_CLUB", &preferences.standard),
        )
    };
    // Normal mode carries every configured SOTA link in intelligence order.
    // Cheap mode carries only its bounded Gemma/OpenRouter/LongCat pool, so a
    // failed free route cannot silently escalate into frontier/token-plan spend.
    let fallbacks = sota_moa_fallbacks(cheap, &role_links, links);
    if fallbacks.is_empty() && env_flag("ANGEL_SOTA_MOA_STRICT_ROUTES", false) {
        eprintln!(
            "[sota-moa] strict routes: propose={} judge={} verify={} aggregate={}; cross-seat failover disabled",
            propose.label(),
            judge.label(),
            verify.label(),
            aggregate.label()
        );
    }
    let extra_proposers = extra_sota_proposers(links)
        .into_iter()
        .filter(|(club, _)| !club.label().eq_ignore_ascii_case(propose.label()))
        .collect::<Vec<_>>();
    if !extra_proposers.is_empty() {
        eprintln!(
            "[sota-moa] additional proposer(s): {}",
            extra_proposers
                .iter()
                .map(|(club, _)| club.label())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Some(Arc::new(
        crate::agent::swarm::SwarmClub::from_sota_env_roles(
            "sota-moa", propose, judge, verify, aggregate,
        )
        .with_extra_proposers(extra_proposers)
        .with_quota_fallbacks(fallbacks),
    ))
}

/// Build the stage failover bench. Strict routes are an evaluator/campaign
/// contract: a failed assigned seat must surface as a failure instead of being
/// credited to another provider or model behind the same logical formation.
pub(crate) fn sota_moa_fallbacks(
    cheap: bool,
    role_links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Vec<Arc<dyn Club>> {
    if env_flag("ANGEL_SOTA_MOA_STRICT_ROUTES", false) {
        return Vec::new();
    }
    let source = if cheap { role_links } else { links };
    source.iter().map(|(_, club, _)| Arc::clone(club)).collect()
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/failover__seat_order_tests.rs"]
mod seat_order_tests;
