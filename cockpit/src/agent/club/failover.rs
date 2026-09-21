//! FallbackClub link failover and SOTA MoA role selection.

use super::*;

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
    /// whole chain — the bits are an optimization, never a brick.
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

    fn compatible_chat_chain(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
    ) -> Vec<&Arc<dyn Club>> {
        let required_tokens = messages
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
                .sum::<usize>();
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
        let mut last = Err("fallback chain is empty".to_string());
        let chain = self.live_chain();
        for (index, club) in chain.iter().enumerate() {
            match club.respond(prompt) {
                Ok(v) => {
                    self.note_resolved(&***club);
                    return Ok(v);
                }
                Err(e) => {
                    Self::note_transition(&chain, index, &e);
                    last = Err(format!("[{}] {e}", club.label()));
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
        let mut last = Err("fallback chain is empty".to_string());
        let chain = self.compatible_chat_chain(messages, tools);
        for (index, club) in chain.iter().enumerate() {
            match club.chat(messages, tools) {
                Ok(v) => {
                    self.note_resolved(&***club);
                    return Ok(v);
                }
                Err(e) => {
                    Self::note_transition(&chain, index, &e);
                    last = Err(format!("[{}] {e}", club.label()));
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
        let mut last = Err("fallback chain is empty".to_string());
        let chain = self.compatible_chat_chain(messages, tools);
        for (index, club) in chain.iter().enumerate() {
            match club.chat_with_effort(messages, tools, effort) {
                Ok(v) => {
                    self.note_resolved(&***club);
                    return Ok(v);
                }
                Err(e) => {
                    Self::note_transition(&chain, index, &e);
                    last = Err(format!("[{}] {e}", club.label()));
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
        let mut last = Err("fallback chain is empty".to_string());
        let chain = self.compatible_chat_chain(messages, tools);
        for (index, club) in chain.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".to_string());
            }
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
                    self.note_resolved(&***club);
                    return Ok(v);
                }
                Err(e) => {
                    // A partially-streamed reply can't be replayed onto the next
                    // club, and a cancel isn't a failure worth routing around.
                    if emitted || cancel.load(Ordering::Relaxed) {
                        return Err(e);
                    }
                    Self::note_transition(&chain, index, &e);
                    last = Err(format!("[{}] {e}", club.label()));
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

/// The single canonical intelligence ranking for every SOTA MoA seat, smartest
/// first. Every seat — and above all the aggregate seat, the tool-capable
/// DRIVER and final synthesizer (the deep thinker) — defaults to the smartest
/// configured link; a free/breadth-tier model never out-ranks a frontier one.
/// Local fleet models are absent from this table by design: a local model must
/// never hold a deep-think seat (docs/plans/sota-moa-smartest-first.md).
///
///   1. openai / codex-run — GPT-5.x-class via ChatGPT OAuth
///   2. kimi               — Kimi K3 via Moonshot API
///   3. deepseek           — deepseek-v4-pro
///   4. glm                — configurable/default glm-5.3, with glm-5.3-flash
///      and glm-5.2 as additional explicit options (z.ai coding plan)
///   5. qwen               — qwen3.7-plus (Alibaba coding plan)
///   6. grok               — account OAuth HTTP to api.x.ai (Grok 4-family)
///   7. longcat            — LongCat-2.0
///   8. meta               — Muse Spark 1.3 (Meta Model API, metered per token;
///      late seat by design so it is not burned on every MoA fan-out)
///   9. openrouter         — stealth/ox-alpha (breadth tier; extra :free
///      catalog seats sit beside it)
///  10. cerebras           — speed-first host, model varies
pub(crate) const SOTA_MOA_INTELLIGENCE_ORDER: &[&str] = &[
    "codex-run",
    "codex",
    "openai",
    "chatgpt-luna-on-high",
    "luna-on-high",
    "luna-high",
    "chatgpt-luna",
    "luna",
    "kimi",
    "kimi-k3",
    "moonshot",
    "deepseek",
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "deepseek-flash",
    "glm",
    "glm-5.3",
    "glm-5.3-flash",
    "glm-5.2",
    "qwen",
    "qwen3.7-plus",
    "grok",
    "grok-research",
    "grok-4",
    "grok-4-latest",
    "grok-4.7",
    "grok-4.5",
    "grok-4.3",
    "grok-4.20",
    "grok-4.20-fast",
    "grok-4.20-mini",
    "longcat",
    "LongCat-2.0",
    // Metered per-token (no plan): auto-seated only after the plan/OAuth and
    // cheap metered seats above are taken, never in the cheap profile.
    "meta",
    "muse-spark-1.3",
    "muse-spark-1.2",
    "openrouter",
    "stealth/union-alpha",
    "stealth/ox-alpha",
    "thinkingmachines/inkling:free",
    "thinkingmachines/inkling-small:free",
    "hy",
    "tencent/hy3:free",
    "tencent/hy3-preview:free",
    "hy3",
    "hy3-preview",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "openrouter/free",
    "poolside/laguna-s-2.1:free",
    "cohere/north-mini-code:free",
    "z-ai/glm-5.2:free",
    "nvidia/nemotron-3.5-lightning:free",
    "cerebras",
];
pub(crate) const SOTA_MOA_PROPOSE_PREFS: &[&str] = SOTA_MOA_INTELLIGENCE_ORDER;
pub(crate) const SOTA_MOA_JUDGE_PREFS: &[&str] = SOTA_MOA_INTELLIGENCE_ORDER;
pub(crate) const SOTA_MOA_VERIFY_PREFS: &[&str] = SOTA_MOA_INTELLIGENCE_ORDER;
pub(crate) const SOTA_MOA_AGG_PREFS: &[&str] = SOTA_MOA_INTELLIGENCE_ORDER;

/// A deliberately bounded everyday MoA. Gemma supplies free local breadth when
/// Spark is up, OpenRouter's free route verifies, and LongCat/DeepSeek-Flash/Luna
/// perform synthesis/proposal. Frontier/token-plan links are intentionally absent:
/// choosing this profile is a spend boundary, not merely a preference hint.
pub(crate) const SOTA_MOA_CHEAP_PROPOSE_PREFS: &[&str] = &[
    "chatgpt-luna-on-high",
    "luna-on-high",
    "luna",
    // Spark-local ds4 DeepSeek-V4-Flash (free fleet) before the metered cloud flash.
    "dsflash",
    "deepseek-v4-flash",
    "deepseek-flash",
    "glm",
    "glm-5.3-flash",
    "glm-5.2",
    "longcat",
    "LongCat-2.0",
    "openrouter",
    "stealth/ox-alpha",
    "thinkingmachines/inkling:free",
    "thinkingmachines/inkling-small:free",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "openrouter/free",
    "poolside/laguna-s-2.1:free",
    "cohere/north-mini-code:free",
    "z-ai/glm-5.2:free",
    "nvidia/nemotron-3.5-lightning:free",
    "tencent/hy3:free",
    "gemma",
];
pub(crate) const SOTA_MOA_CHEAP_JUDGE_PREFS: &[&str] = &[
    "chatgpt-luna-on-high",
    "luna-on-high",
    "luna",
    "dsflash",
    "deepseek-v4-flash",
    "deepseek-flash",
    "glm",
    "glm-5.3-flash",
    "glm-5.2",
    "longcat",
    "LongCat-2.0",
    "openrouter",
    "stealth/ox-alpha",
    "thinkingmachines/inkling:free",
    "thinkingmachines/inkling-small:free",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "openrouter/free",
    "poolside/laguna-s-2.1:free",
    "cohere/north-mini-code:free",
    "z-ai/glm-5.2:free",
    "nvidia/nemotron-3.5-lightning:free",
    "tencent/hy3:free",
    "gemma",
];
pub(crate) const SOTA_MOA_CHEAP_VERIFY_PREFS: &[&str] = &[
    "dsflash",
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
    "nvidia/nemotron-3.5-lightning:free",
    "tencent/hy3:free",
    "longcat",
    "LongCat-2.0",
    "gemma",
];
pub(crate) const SOTA_MOA_CHEAP_AGG_PREFS: &[&str] = &[
    "chatgpt-luna-on-high",
    "luna-on-high",
    "luna",
    "dsflash",
    "deepseek-v4-flash",
    "deepseek-flash",
    "glm",
    "glm-5.3-flash",
    "glm-5.2",
    "longcat",
    "LongCat-2.0",
    "gemma",
    "openrouter",
    "stealth/ox-alpha",
    "thinkingmachines/inkling:free",
    "thinkingmachines/inkling-small:free",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "openrouter/free",
    "poolside/laguna-s-2.1:free",
    "cohere/north-mini-code:free",
    "z-ai/glm-5.2:free",
    "nvidia/nemotron-3.5-lightning:free",
    "tencent/hy3:free",
];
const SOTA_MOA_CHEAP_FALLBACK_ORDER: &[&str] = &[
    "dsflash",
    "deepseek-v4-flash",
    "deepseek-flash",
    "glm",
    "glm-5.3-flash",
    "glm-5.2",
    "chatgpt-luna-on-high",
    "luna-on-high",
    "luna",
    "gemma",
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

pub(crate) fn ordered_cheap_links(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let mut ordered = Vec::new();
    for wanted in SOTA_MOA_CHEAP_FALLBACK_ORDER {
        for entry in links
            .iter()
            .filter(|entry| cheap_link_allowed(entry) && link_matches(entry, wanted))
        {
            if !ordered
                .iter()
                .any(|(alias, _, _): &(String, Arc<dyn Club>, Arc<AtomicBool>)| {
                    alias.eq_ignore_ascii_case(&entry.0)
                })
            {
                ordered.push((entry.0.clone(), Arc::clone(&entry.1), Arc::clone(&entry.2)));
            }
        }
    }
    ordered
}

fn cheap_role_with_fallbacks(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    env_key: &str,
    preferences: &[&str],
) -> Arc<dyn Club> {
    let primary = pick_sota_role(links, env_key, preferences);
    let mut chain = vec![Arc::clone(&primary)];
    let mut gates = vec![
        links
            .iter()
            .find(|(_, club, _)| Arc::ptr_eq(club, &primary))
            .map(|(_, _, gate)| Arc::clone(gate)),
    ];
    for (_, club, gate) in ordered_cheap_links(links) {
        if chain.iter().any(|candidate| Arc::ptr_eq(candidate, &club)) {
            continue;
        }
        chain.push(club);
        gates.push(Some(gate));
    }
    Arc::new(FallbackClub::with_gates(chain, gates))
}

/// Configured proposer-only seats requested for SOTA-MoA breadth. Gemma is on by
/// default; `ANGEL_SOTA_MOA_EXTRA_PROPOSERS=none` provides the calibration
/// control. Explicit remote seats still obey provider opt-in and the cheap allowlist.
pub(crate) fn extra_sota_proposers(
    local_breadth: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Vec<GatedClub> {
    let requested =
        std::env::var("ANGEL_SOTA_MOA_EXTRA_PROPOSERS").unwrap_or_else(|_| "gemma".to_string());
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
    for entry in local_breadth {
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

fn find_named_link(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    names: &[&str],
) -> Option<(Arc<dyn Club>, Arc<AtomicBool>)> {
    for name in names {
        if let Some((_, club, available)) = links.iter().find(|(alias, club, _)| {
            alias.eq_ignore_ascii_case(name)
                || club.label().eq_ignore_ascii_case(name)
                || club
                    .label()
                    .to_ascii_lowercase()
                    .contains(&name.to_ascii_lowercase())
                || club.live_model_name().is_some_and(|model| {
                    model.eq_ignore_ascii_case(name)
                        || model
                            .to_ascii_lowercase()
                            .contains(&name.to_ascii_lowercase())
                })
        }) {
            return Some((Arc::clone(club), Arc::clone(available)));
        }
    }
    None
}

/// One Sol@ultra head, GLM-5.3 + DeepSeek v4 Pro extra proposers, Grok xhigh
/// weigh-in. Absent Sol or Grok, the club is not built — same as a missing
/// HTTP SOTA pin. Mix seats are optional: a missing GLM or DeepSeek key still
/// yields the club, just narrower.
pub(crate) fn mathgod_club(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Option<(Arc<dyn Club>, Arc<AtomicBool>)> {
    let (sol, sol_avail) =
        find_named_link(links, &["openai", "codex-run", "codex", "gpt-5.6-sol"])?;
    let (grok, _) = find_named_link(links, &["grok", "grok-4.7", "grok-4.6", "grok-4.5", "xai"])?;
    let extra = mathgod_mix_seats(links, sol.as_ref(), grok.as_ref());
    Some((
        Arc::new(crate::agent::swarm::SwarmClub::mathgod(sol, grok, extra)),
        sol_avail,
    ))
}

pub(crate) fn mathgod_mix_seats(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    sol: &dyn Club,
    grok: &dyn Club,
) -> Vec<GatedClub> {
    let mut extra = Vec::new();
    for names in [
        ["glm", "glm-5.3"].as_slice(),
        ["deepseek-v4-pro", "deepseek"].as_slice(),
    ] {
        let Some((club, avail)) = find_named_link(links, names) else {
            continue;
        };
        let label = club.label();
        let lower = label.to_ascii_lowercase();
        if lower.contains("flash")
            || label.eq_ignore_ascii_case(sol.label())
            || label.eq_ignore_ascii_case(grok.label())
            || extra.iter().any(|(existing, _): &(Arc<dyn Club>, _)| {
                existing.label().eq_ignore_ascii_case(label)
            })
        {
            continue;
        }
        extra.push((club, Some(avail)));
    }
    extra
}

pub(crate) fn pick_sota_role(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    env_key: &str,
    preferred_aliases: &[&str],
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

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn sota_moa_club(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Option<Arc<dyn Club>> {
    sota_moa_club_with_breadth(links, &[])
}

/// Build the SOTA wrapper with optional local breadth candidates. Local links
/// are considered only by the explicit `cheap` cost profile; the normal
/// smartest-first formation and its failover order remain unchanged.
pub(crate) fn sota_moa_club_with_breadth(
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    local_breadth: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
) -> Option<Arc<dyn Club>> {
    let cheap = cheap_moa_enabled();
    let mut role_links = links.to_vec();
    if cheap {
        role_links.extend(
            local_breadth
                .iter()
                .map(|(alias, club, gate)| (alias.clone(), Arc::clone(club), Arc::clone(gate))),
        );
        role_links = ordered_cheap_links(&role_links);
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
                SOTA_MOA_CHEAP_PROPOSE_PREFS,
            ),
            cheap_role_with_fallbacks(
                &role_links,
                "ANGEL_SOTA_MOA_JUDGE_CLUB",
                SOTA_MOA_CHEAP_JUDGE_PREFS,
            ),
            cheap_role_with_fallbacks(
                &role_links,
                "ANGEL_SOTA_MOA_VERIFY_CLUB",
                SOTA_MOA_CHEAP_VERIFY_PREFS,
            ),
            cheap_role_with_fallbacks(
                &role_links,
                "ANGEL_SOTA_MOA_AGG_CLUB",
                SOTA_MOA_CHEAP_AGG_PREFS,
            ),
        )
    } else {
        (
            pick_sota_role(links, "ANGEL_SOTA_MOA_PROPOSE_CLUB", SOTA_MOA_PROPOSE_PREFS),
            pick_sota_role(links, "ANGEL_SOTA_MOA_JUDGE_CLUB", SOTA_MOA_JUDGE_PREFS),
            pick_sota_role(links, "ANGEL_SOTA_MOA_VERIFY_CLUB", SOTA_MOA_VERIFY_PREFS),
            pick_sota_role(links, "ANGEL_SOTA_MOA_AGG_CLUB", SOTA_MOA_AGG_PREFS),
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
    let mut proposer_links = local_breadth.to_vec();
    proposer_links.extend_from_slice(links);
    let extra_proposers = extra_sota_proposers(&proposer_links)
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
