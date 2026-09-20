//! The bag of clubs: model backends the cockpit can swing.
//!
//! We think of models as golf clubs. Each model is a [`Club`]; the [`Bag`] holds
//! several and keeps one "in hand". The primary club is the **Driver**
//! (`ANGEL_DRIVER`) — unset, it prefers the OpenAI ChatGPT-OAuth link (when a
//! token is on disk), then a configured LongCat link, then the smartest
//! reachable fleet model. A configured OpenRouter catalog is selectable on the
//! `sota` box and is not auto-elected. Local models
//! are loaded from each endpoint's live `/models` response; env vars may pin a
//! model id, but source never has to know which checkpoint a rig is serving.
//! [`PracticeClub`] is the practice swing (offline echo).
//!
//! Club endpoints are resolved **dynamically from the tailnet** at startup
//! (`tailscale status --json`, keyed by DNSName label), so an offline box drops
//! out on its own. `ANGEL_<LABEL>_URL/_MODEL` may override an endpoint/model.
//!
//! A club talks to the agent loop through [`Club::chat`], which speaks the
//! OpenAI-style tool-calling protocol (messages + tool defs in, either a text
//! answer or tool calls out). `respond` is the simple single-turn fallback.

pub(crate) use base64::Engine as _;
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use std::io::{BufRead, BufReader};
pub(crate) use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
pub(crate) use std::sync::{Arc, Mutex};
pub(crate) use std::time::{Duration, Instant};

mod accounting;
mod bag;
mod caveman;
pub(crate) mod codex_selection;
mod discovery;
mod failover;
mod grok;
mod http;
pub(crate) mod model_defaults;
mod prober;
mod provision;
mod pxpipe;
mod recovery_context;
mod retry;
mod sse;
mod tool_parse;
mod types;
mod usage;
pub(crate) use accounting::*;

pub(crate) use bag::*;
pub(crate) use caveman::*;
pub(crate) use discovery::*;
pub(crate) use failover::*;
pub(crate) use grok::*;
pub(crate) use http::*;
pub(crate) use prober::*;
pub(crate) use provision::*;
pub(crate) use pxpipe::*;
pub(crate) use recovery_context::*;
pub(crate) use retry::*;
pub(crate) use sse::*;
pub(crate) use tool_parse::*;
pub(crate) use types::*;

thread_local! {
    /// Private provider reasoning associated with the most recent tool-call
    /// reply on this execution thread. The turn loop consumes it immediately
    /// when it appends that exact assistant message, before another request can
    /// run on the thread. It is never serialized or shared across sessions.
    static PENDING_TOOL_REASONING: std::cell::RefCell<Option<String>> = const {
        std::cell::RefCell::new(None)
    };
}

pub(crate) fn set_pending_tool_reasoning(reasoning: Option<String>) {
    PENDING_TOOL_REASONING.with(|slot| *slot.borrow_mut() = reasoning);
}

pub(crate) fn take_pending_tool_reasoning() -> Option<String> {
    PENDING_TOOL_REASONING.with(|slot| slot.borrow_mut().take())
}

pub trait Club: Send + Sync {
    /// Can every underlying provider attempt reserve and settle a turn budget?
    /// Unknown adapters are reported as untracked; bookkeeping never denies dispatch.
    fn supports_formation_budget(&self) -> bool {
        false
    }

    /// Single-turn, no tools — the simple path and the base every club implements.
    fn respond(&self, prompt: &str) -> Result<String, String>;

    /// Like [`respond`](Self::respond) but abandons the call when `cancel` is
    /// raised. The default ignores the flag — `respond` is uninterruptible for
    /// most clubs. A club that drives a killable external resource (e.g. a CLI
    /// subprocess) overrides this to tear it down promptly, so an interruptible
    /// caller (the swarm's research scout) can honor Esc mid-call instead of
    /// blocking out the full timeout.
    fn respond_cancellable(&self, prompt: &str, _cancel: &AtomicBool) -> Result<String, String> {
        self.respond(prompt)
    }

    /// Short label shown in the UI so it's always clear which club is in hand.
    fn label(&self) -> &str;

    /// Env namespace for this club's per-club knobs (`ANGEL_<CLUB>_*`), when it
    /// has one. Not derivable from [`Club::label`]: aggregators label themselves
    /// with a model id (`tencent/hy3:free`) that is not a shell-variable suffix,
    /// and resolve to their provider namespace instead (see `env_prefix_for`).
    /// `None` for logical clubs with no request-level knobs; callers then read
    /// only the global control.
    fn env_namespace(&self) -> Option<&str> {
        None
    }

    /// Whether this club's backend is reachable *right now*. Default `true`:
    /// local/logical clubs (the practice swing) are always in play. HTTP-backed
    /// clubs override this with a short, bounded readiness probe so a dead
    /// endpoint can be dropped from the bag (hidden from the tab strip, skipped
    /// by `Tab`) without the cockpit ever blocking on it — the [`Bag`] prober
    /// calls this off the UI thread.
    fn is_available(&self) -> bool {
        true
    }

    /// The model id this club is *currently* serving, for clubs that track their
    /// backend dynamically (`None` for fixed-identity clubs like the practice
    /// swing and env-pinned SOTA links). The bag prefers this over the static
    /// slot label, so the tab strip and header show the real checkpoint even
    /// after the operator swaps models on a box mid-session.
    fn live_model_name(&self) -> Option<String> {
        None
    }

    /// Exact configured-or-live model identity for operator route inspection.
    /// Unlike `live_model_name`, this may expose an env-pinned catalog model
    /// without forcing the compact tab strip to replace its useful mode alias.
    fn model_identity(&self) -> Option<String> {
        self.live_model_name()
    }

    fn resolved_model_defaults(&self) -> serde_json::Value {
        model_defaults::budgets(
            &self.model_identity().unwrap_or_default(),
            self.env_namespace().unwrap_or(self.label()),
        )
    }

    /// Bind non-HTTP request identity; HTTP binds from its final serialized body.
    fn bind_run_identity(&self, effort: Option<&str>) -> Result<(), String> {
        if is_logical_wrapper_label(self.label()) {
            return Ok(());
        }
        let route = self.route_identity();
        crate::harness::run_identity::bind(
            crate::harness::run_identity::Model {
                club: self.label().into(),
                id: route.model.unwrap_or_else(|| "unbound".into()),
                base_url: "unbound".into(),
                driver: route.driver,
            },
            // The generic adapter cannot certify a provider's dialect. Never
            // label an unvalidated request as a wire value.
            serde_json::json!(if effort.is_some() || route.reasoning_effort.is_some() {
                "unbound"
            } else {
                "none"
            }),
            serde_json::json!("unbound"),
            None,
        )
    }

    fn route_identity(&self) -> RouteIdentity {
        RouteIdentity {
            driver: self.label().to_string(),
            model: self.model_identity(),
            reasoning_effort: self.reasoning_effort(),
        }
    }

    /// Route that actually produced the most recent answer. Plain clubs are
    /// their own resolved route; failover wrappers override after a successful
    /// chain hop.
    fn resolved_route_identity(&self) -> RouteIdentity {
        self.route_identity()
    }

    /// Already-published failover identity, if any. The draw path uses this
    /// instead of allocating a fresh [`Self::route_identity`] every frame:
    /// `None` means keep the spawn `requested_route`. Default is never-known.
    fn resolved_route_if_known(&self) -> Option<RouteIdentity> {
        None
    }

    /// Agent-header capability facts. Default is [`Self::route_metadata`];
    /// HTTP clubs override to skip the per-frame output-budget env-cache lock
    /// (their header never showed a window or speed tier).
    fn header_route_metadata(&self) -> RouteMetadata {
        self.route_metadata()
    }

    /// Best-effort connection / metadata warm so the first real hop does not
    /// pay cold DNS+TLS+probe on the submit path. Default is a no-op; HTTP
    /// clubs probe `/models` once (cached). Safe to call from a background
    /// thread — never blocks the UI loop.
    fn warm_up(&self) {}

    /// Cached operator-facing model/capability facts. Backends with no trusted
    /// catalog metadata return an empty value rather than invented claims.
    fn route_metadata(&self) -> RouteMetadata {
        RouteMetadata::default()
    }

    /// Allocation-free revision for mutable facts projected by `route_choices`.
    /// Clubs with a live model identity, effort override, or capability gate bump
    /// this after publishing a change so the Bag can reuse immutable snapshots
    /// without hiding an update.
    fn route_state_revision(&self) -> u64 {
        0
    }

    /// Current backend-owned reasoning effort, when this route exposes a
    /// selectable effort. Logical/mixed routes return `None` and the UI labels
    /// them model-native rather than pretending a global knob controls them.
    fn reasoning_effort(&self) -> Option<String> {
        None
    }

    /// Effort values accepted by this exact model, in increasing order.
    fn reasoning_levels(&self) -> &[String] {
        &[]
    }

    /// Select one exact supported effort. The default is read-only; selectable
    /// backends validate the value against their own model capability list.
    fn set_reasoning_effort(&self, _effort: &str) -> Option<String> {
        None
    }

    /// Whether this club's final answer is a *synthesized report* worth filing to
    /// long-form memory (via the Librarian → palace). Default `false`: a plain
    /// chat turn is ephemeral and is captured by compaction if it matters. The
    /// swarm overrides this — its answer is a distilled multi-agent synthesis, the
    /// kind of artifact the palace exists to keep. See `harness::run_turn`.
    fn reports_to_palace(&self) -> bool {
        false
    }

    /// Remaining cooldown while this club's provider quota is exhausted (weekly/
    /// monthly plan cap), `None` when the club is spendable. A cheap in-memory
    /// check, never a network probe — the swarm's failover and the status line
    /// consult it to route around a link that's down until its window resets.
    /// Default `None`: local clubs have no metered quota.
    fn quota_cooldown(&self) -> Option<Duration> {
        None
    }

    /// A human-readable usage report for `/status`, when the backend reports one
    /// (token counts, plan rate-limits). Default `None`: most clubs don't expose
    /// usage. The ChatGPT/Codex club overrides this with the figures the Responses
    /// API returns. Returns `None` until at least one turn has run.
    fn usage_report(&self) -> Option<String> {
        None
    }

    /// Structured token usage for compact live UI meters. Backends that only have
    /// text reports can keep returning `None`; the cockpit will show an unknown
    /// SOTA meter rather than parsing display prose.
    fn token_usage(&self) -> Option<TokenUsage> {
        None
    }

    /// Coherent optional accounting; unsupported backends remain explicitly untracked.
    fn usage_accounting(&self) -> AccountingView {
        AccountingView::untracked()
    }

    /// Cumulative prompt-cache economics for causal benchmarking. Backends
    /// without explicit cache controls or usage fields report zero.
    fn cache_usage(&self) -> CacheUsage {
        CacheUsage::default()
    }

    /// Whether this backend keeps a byte-exact automatic prefix cache — a live
    /// window probe (llama.cpp/vLLM APC), the static cloud-model map, or a
    /// documented provider family (DeepSeek caches every request and bills a
    /// hit at a small fraction of a miss). Drives the cache-first defaults:
    /// append-only history while a turn is in flight and asking streaming
    /// responses to carry usage accounting. `false` when unknown, so an
    /// undetected backend keeps every historical behavior byte-identical.
    fn prompt_cache_capable(&self) -> bool {
        false
    }

    /// Cumulative unusable-output truncation recovery for causal benchmarking.
    /// Always available and zero for clubs without this transport behavior.
    fn truncation_usage(&self) -> TruncationUsage {
        TruncationUsage::default()
    }

    /// Cumulative reasoning-effort gate events: requested efforts withheld from
    /// the wire and backend rejections that taught the club to stop sending the
    /// field. The turn loop deltas this around a chat and voices `last` as a
    /// notice — a gate that strips operator intent must say so. Default zero:
    /// clubs without reasoning controls never gate one.
    fn effort_gate_usage(&self) -> EffortGateUsage {
        EffortGateUsage::default()
    }

    /// The model's real context window + capability flags, detected once from the
    /// backend. `None` for logical clubs (the practice swing) and any club that
    /// can't report — callers then fall back to their own default budget. HTTP
    /// clubs override this with a lazy, cached `/props` (or `/v1/models`) probe.
    /// Compaction budgets against `context_window`; `build_body` gates the
    /// prompt-cache key and `reasoning_effort` on the capability flags.
    fn metadata(&self) -> Option<Metadata> {
        None
    }

    /// Like [`Club::metadata`], but never performs network work: returns only
    /// what is already cached (possibly stale, possibly `None`). For UI-thread
    /// callers — `metadata()` can block on a backend probe with multi-second
    /// timeouts, which must never sit on the submit path.
    fn metadata_cached(&self) -> Option<Metadata> {
        None
    }

    /// Tool-enabled chat. Default impl ignores tools and treats the latest user
    /// message as a prompt — so simple clubs (the practice swing) work inside the agent
    /// loop, they just never call tools. Real clubs override this.
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        let prompt = messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.as_ref())
            .unwrap_or("");
        self.respond(prompt).map(ClubReply::Text)
    }

    /// [`Club::chat`] with a per-call reasoning-effort request. Seat-scoped
    /// fan-outs (the MoA roster) use this so ONE shared club can serve several
    /// seats at different efforts without mutating route state — the request
    /// rides the call, never the club. The value is a request, not a command:
    /// backends apply their own validation/dialect, and this default ignores it
    /// entirely (model-native routes, simple clubs, test doubles).
    fn chat_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        let _ = effort;
        self.chat(messages, tools)
    }

    /// Streaming counterpart to [`Club::chat_with_effort`]. A seat-scoped
    /// caller uses this instead of mutating shared route state before launch.
    /// The default preserves correctness for simple clubs by taking the
    /// non-streaming per-call path and emitting its answer as one delta; real
    /// streaming providers override it so cancellation and token deltas remain
    /// live while the effort request rides the individual call.
    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        // Model-native/simple clubs have no effort implementation at all; keep
        // their streaming and cancellation behavior intact. Clubs that opt in
        // to a reasoning ladder but not this method still get the safe
        // non-streaming per-call fallback below.
        if self.reasoning_levels().is_empty() {
            return self.chat_streaming(messages, tools, cancel, on_delta);
        }
        let reply = self.chat_with_effort(messages, tools, effort)?;
        if let ClubReply::Text(t) = &reply
            && !t.is_empty()
        {
            on_delta(StreamDelta::Content(t));
        }
        Ok(reply)
    }

    /// Streaming tool-calling chat. Real HTTP clubs override this to stream the
    /// reply token-by-token (`on_delta` gets each text chunk as it arrives),
    /// check `cancel` between chunks for a prompt mid-reply interrupt, and bound
    /// reads by an *idle* (per-chunk) timeout — so a long but steady generation
    /// over a slow LAN never trips a total deadline.
    ///
    /// The default is non-streaming: run the blocking [`Club::chat`] and emit the
    /// whole answer as a single delta. So simple clubs (the practice swing, test doubles)
    /// keep working through the streaming path with no extra code — they just
    /// deliver their reply in one piece.
    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        _cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        let reply = self.chat(messages, tools)?;
        if let ClubReply::Text(t) = &reply
            && !t.is_empty()
        {
            on_delta(StreamDelta::Content(t));
        }
        Ok(reply)
    }
}

/// Immutable per-seat view of a shared club. The wrapper pins route identity
/// and threads its effort through each chat call; it never changes the inner
/// club's operator/UI state, so concurrent graph or spawn seats cannot overwrite
/// one another's request just before dispatch.
struct EffortScopedClub {
    inner: Arc<dyn Club>,
    effort: String,
}

impl EffortScopedClub {
    fn route_with_effort(&self, mut route: RouteIdentity) -> RouteIdentity {
        route.reasoning_effort = Some(self.effort.clone());
        route
    }

    fn effective_effort(&self, _requested: Option<&str>) -> Option<&str> {
        // A scoped view is an immutable seat contract, not another mutable
        // selection surface. Nested wrappers/callers cannot override the pin
        // and make the wire request disagree with the projected route receipt.
        Some(self.effort.as_str())
    }
}

impl Club for EffortScopedClub {
    fn supports_formation_budget(&self) -> bool {
        self.inner.supports_formation_budget()
    }
    fn respond(&self, prompt: &str) -> Result<String, String> {
        self.respond_cancellable(prompt, &AtomicBool::new(false))
    }

    fn respond_cancellable(&self, prompt: &str, cancel: &AtomicBool) -> Result<String, String> {
        match self.inner.chat_streaming_with_effort(
            &[ChatMsg::user(prompt)],
            &[],
            Some(&self.effort),
            cancel,
            &mut |_| {},
        )? {
            ClubReply::Text(text) => Ok(text),
            ClubReply::Calls(_) => Err("club requested a tool with no tools offered".to_string()),
        }
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn env_namespace(&self) -> Option<&str> {
        self.inner.env_namespace()
    }

    fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    fn live_model_name(&self) -> Option<String> {
        self.inner.live_model_name()
    }

    fn model_identity(&self) -> Option<String> {
        self.inner.model_identity()
    }

    fn bind_run_identity(&self, _effort: Option<&str>) -> Result<(), String> {
        self.inner.bind_run_identity(Some(&self.effort))
    }

    fn route_identity(&self) -> RouteIdentity {
        self.route_with_effort(self.inner.route_identity())
    }

    fn resolved_route_identity(&self) -> RouteIdentity {
        self.route_with_effort(self.inner.resolved_route_identity())
    }

    fn resolved_route_if_known(&self) -> Option<RouteIdentity> {
        self.inner
            .resolved_route_if_known()
            .map(|route| self.route_with_effort(route))
    }

    fn warm_up(&self) {
        self.inner.warm_up();
    }

    fn route_metadata(&self) -> RouteMetadata {
        self.inner.route_metadata()
    }

    fn header_route_metadata(&self) -> RouteMetadata {
        self.inner.header_route_metadata()
    }

    fn route_state_revision(&self) -> u64 {
        self.inner.route_state_revision()
    }

    fn reasoning_effort(&self) -> Option<String> {
        Some(self.effort.clone())
    }

    fn reasoning_levels(&self) -> &[String] {
        self.inner.reasoning_levels()
    }

    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        requested
            .trim()
            .eq_ignore_ascii_case(&self.effort)
            .then(|| self.effort.clone())
    }

    fn reports_to_palace(&self) -> bool {
        self.inner.reports_to_palace()
    }

    fn quota_cooldown(&self) -> Option<Duration> {
        self.inner.quota_cooldown()
    }

    fn usage_report(&self) -> Option<String> {
        self.inner.usage_report()
    }

    fn token_usage(&self) -> Option<TokenUsage> {
        self.inner.token_usage()
    }

    fn usage_accounting(&self) -> AccountingView {
        self.inner.usage_accounting()
    }

    fn cache_usage(&self) -> CacheUsage {
        self.inner.cache_usage()
    }

    fn prompt_cache_capable(&self) -> bool {
        self.inner.prompt_cache_capable()
    }

    fn truncation_usage(&self) -> TruncationUsage {
        self.inner.truncation_usage()
    }

    fn effort_gate_usage(&self) -> EffortGateUsage {
        self.inner.effort_gate_usage()
    }

    fn metadata(&self) -> Option<Metadata> {
        self.inner.metadata()
    }

    fn metadata_cached(&self) -> Option<Metadata> {
        self.inner.metadata_cached()
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.inner
            .chat_with_effort(messages, tools, Some(&self.effort))
    }

    fn chat_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        self.inner
            .chat_with_effort(messages, tools, self.effective_effort(effort))
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.inner
            .chat_streaming_with_effort(messages, tools, Some(&self.effort), cancel, on_delta)
    }

    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        self.inner.chat_streaming_with_effort(
            messages,
            tools,
            self.effective_effort(effort),
            cancel,
            on_delta,
        )
    }
}

/// Resolve and pin one supported reasoning effort without mutating `club`.
/// The returned `Option` is the exact canonical value applied; a model-native
/// or rejecting route is returned unchanged with `None`.
pub(crate) fn scoped_reasoning_effort(
    club: Arc<dyn Club>,
    requested: &str,
) -> (Arc<dyn Club>, Option<String>) {
    let canonical = club
        .reasoning_levels()
        .iter()
        .find(|level| level.eq_ignore_ascii_case(requested.trim()))
        .cloned();
    match canonical {
        Some(effort) => (
            Arc::new(EffortScopedClub {
                inner: club,
                effort: effort.clone(),
            }),
            Some(effort),
        ),
        None => (club, None),
    }
}

/// Logical wrappers that must not be auto-elected as a brain, loop seat, or
/// Tab stop. `mathgod` is selected only by `/moa math` or `ANGEL_DRIVER=mathgod`.
pub(crate) fn is_logical_wrapper_label(label: &str) -> bool {
    let label = label.trim();
    eq_ascii_ignore_case(label, "practice")
        || eq_ascii_ignore_case(label, "swarm")
        || contains_ascii_ignore_case(label, "moa")
        || is_mathgod_label(label)
        || starts_with_ascii_ignore_case(label, "gpu-comp")
}

pub(crate) fn is_mathgod_label(label: &str) -> bool {
    let label = label.trim();
    eq_ascii_ignore_case(label, "mathgod")
        || eq_ascii_ignore_case(label, "math-god")
        || eq_ascii_ignore_case(label, "math god")
}

fn contains_ascii_ignore_case(hay: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return true;
    }
    hay.as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn eq_ascii_ignore_case(value: &str, expected: &str) -> bool {
    value.as_bytes().eq_ignore_ascii_case(expected.as_bytes())
}

fn starts_with_ascii_ignore_case(value: &str, prefix: &str) -> bool {
    let value = value.as_bytes();
    let prefix = prefix.as_bytes();
    value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Token-metered / remote SOTA labels. These clubs are useful for explicit
/// escalation, but should never be pulled into local-only fallback/delegation by
/// accident.
///
/// The Agent bay meter asks this every frame, so matching must not allocate.
pub(crate) fn is_sota_label(label: &str) -> bool {
    contains_ascii_ignore_case(label, "codex")
        || contains_ascii_ignore_case(label, "openai")
        || contains_ascii_ignore_case(label, "gpt")
        || contains_ascii_ignore_case(label, "kimi")
        || contains_ascii_ignore_case(label, "moonshot")
        || contains_ascii_ignore_case(label, "glm")
        || contains_ascii_ignore_case(label, "deepseek")
        || contains_ascii_ignore_case(label, "cerebras")
        || eq_ascii_ignore_case(label, "meta")
        || starts_with_ascii_ignore_case(label, "muse-")
        || contains_ascii_ignore_case(label, "grok")
        || contains_ascii_ignore_case(label, "xai")
        || contains_ascii_ignore_case(label, "openrouter")
        || contains_ascii_ignore_case(label, ":free")
        || eq_ascii_ignore_case(label, "hy")
        || contains_ascii_ignore_case(label, "tencent/hy3")
        || contains_ascii_ignore_case(label, "hy3-preview")
        // The OpenRouter breadth seat wears its model id as its label, so the
        // free route has to be recognized by slug or it looks local.
        || contains_ascii_ignore_case(label, "nemotron")
        || contains_ascii_ignore_case(label, "union-alpha")
        || contains_ascii_ignore_case(label, "ox-alpha")
        || contains_ascii_ignore_case(label, "inkling")
        || contains_ascii_ignore_case(label, "thinkingmachines")
        || contains_ascii_ignore_case(label, "laguna")
        || contains_ascii_ignore_case(label, "north-mini-code")
        // Alibaba's coding-plan seat. Matched exactly, never by `contains`:
        // the fleet runs local Qwen checkpoints (qwen3.6-27b, qwen3.5:4b) that
        // must stay local-eligible.
        || eq_ascii_ignore_case(label, "qwen")
        || eq_ascii_ignore_case(label, "qwen3.7-plus")
        || contains_ascii_ignore_case(label, "longcat")
        || contains_ascii_ignore_case(label, "sota")
        || eq_ascii_ignore_case(label, "mathgod")
        || eq_ascii_ignore_case(label, "math-god")
}

/// The designated smart-escalation seat — what `smart`/`sota` means in
/// `consult_model` and `spawn`.
pub(crate) struct SmartSeat {
    pub(crate) club: String,
    pub(crate) effort: String,
}

/// Resolve the smart seat. Defaults: gpt-5.6 **Sol at max** effort, with
/// **Kimi-K3 at high** as the graceful alternative (operator order
/// 2026-08-02). When a TUI approval surface is attached, the operator gets a
/// popup: approve/approve-all = the primary, deny = the alternative — a choice
/// between two sanctioned seats, never a veto that silently downgrades to a
/// cheap model. Headless runs take the primary silently: escalation must never
/// stall an unattended lane on a modal nobody will answer.
pub(crate) fn smart_seat() -> SmartSeat {
    let env_or = |name: &str, default: &str| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default.to_string())
    };
    let primary = env_or("ANGEL_SOTA_SMART_CLUB", "sol");
    let primary_effort = env_or("ANGEL_SOTA_SMART_EFFORT", "max");
    let alt = env_or("ANGEL_SOTA_SMART_ALT", "kimi");
    let alt_effort = env_or("ANGEL_SOTA_SMART_ALT_EFFORT", "high");
    if crate::approval::ui_installed() {
        let decision = crate::approval::ask(
            crate::approval::ApprovalScope::PhoneModel(format!(
                "smart escalation · {primary}@{primary_effort}"
            )),
            &format!(
                "Smart escalation seat: [y] {primary}@{primary_effort} (default) · \
                 [n] {alt}@{alt_effort}"
            ),
        );
        if decision == crate::approval::Decision::Deny {
            return SmartSeat {
                club: alt,
                effort: alt_effort,
            };
        }
    }
    SmartSeat {
        club: primary,
        effort: primary_effort,
    }
}

/// The smart seat's alternative, for availability fallback when the primary is
/// not in the roster (e.g. no ChatGPT OAuth token on this box).
pub(crate) fn smart_seat_alt() -> SmartSeat {
    let env_or = |name: &str, default: &str| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default.to_string())
    };
    SmartSeat {
        club: env_or("ANGEL_SOTA_SMART_ALT", "kimi"),
        effort: env_or("ANGEL_SOTA_SMART_ALT_EFFORT", "high"),
    }
}

// ---------------------------------------------------------------------------
// PracticeClub — the practice swing
// ---------------------------------------------------------------------------

/// Offline echo. Simulates a little latency (on the worker thread, so the UI
/// stays smooth) so the loading bar is visible even with no real club.
pub struct PracticeClub {
    latency: Duration,
}

impl PracticeClub {
    pub fn new() -> Self {
        Self {
            latency: Duration::from_millis(800),
        }
    }
}

impl Default for PracticeClub {
    fn default() -> Self {
        Self::new()
    }
}

impl Club for PracticeClub {
    fn model_identity(&self) -> Option<String> {
        Some(
            if std::env::var("ANGEL_PRACTICE").as_deref() == Ok("1") {
                "practice (ANGEL_PRACTICE=1)"
            } else {
                "practice"
            }
            .to_string(),
        )
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        if !self.latency.is_zero() {
            std::thread::sleep(self.latency);
        }
        let p = prompt.trim();
        if p.is_empty() {
            return Ok("…".to_string());
        }
        Ok(format!("practice: you said: {p}"))
    }
    fn label(&self) -> &str {
        "practice"
    }
}

// ---------------------------------------------------------------------------
// GpuCompLocalMoaClub — logical cockpit configuration
// ---------------------------------------------------------------------------

/// A permanent, always-available selector entry for the overnight GPU competition
/// formation. It is a coordinator surface: visible in the cockpit as a stable
/// logical agent, with Turbo doing the actual language-model work underneath.
pub struct GpuCompLocalMoaClub {
    driver: Arc<dyn Club>,
}

impl GpuCompLocalMoaClub {
    pub fn new(driver: Arc<dyn Club>) -> Self {
        Self { driver }
    }

    fn coordinator_prompt() -> &'static str {
        "You are GPU Comp Local MoA, the angel0 overnight GPU competition coordinator. \
Your primary objective is to drive the local angel0 GPU-comp fleet toward stronger kernel candidates by coordinating the local models and keeping the pipeline efficient. \
Your role is architecture, model coordination, debugging, validation discipline, and operational routing. \
You are not the active competition driver unless explicitly asked; Turbo x12 is the driver/self-agent pool, Leanstral handles formula and numerical math, DICE is an advisory checker, and OpenAI audits submission.py algorithm/math at most every 45 minutes. \
This cockpit club is one Turbo coordinator conversation, not proof that twelve lanes, Leanstral, DICE, or OpenAI actually ran. Claim a lane or verifier participated only when its runner artifact exists. Use the explicit GPU-comp runner for fan-out; never describe a single coordinator turn as an MoA dispatch. \
Answer the user's actual prompt directly. Do not repeat setup instructions unless the user asks how to start, activate, bootstrap, or diagnose the configuration. \
When asked your objective, state the coordination objective plainly. \
When discussing kernel improvements, require evidence-first validation: correctness before timing, proxy GPU results only as rejection/calibration evidence, and authoritative target benchmarks before submission claims. \
Work Treebeard-style when the lane is active: park large kernels and popcorn logs under handles, batch candidate edits via code_mode, keep the root trajectory strategy-only (handle receipts + plan), and feed measured B200 outcomes back as Hi/Q goldens rather than bulk CUDA dumps."
    }

    fn status_card() -> &'static str {
        "GPU Comp Local MoA coordinator: this chat is one Turbo coordinator lane. The separate runner can dispatch Turbo lanes and record Leanstral/DICE/OpenAI participation; only its artifacts prove those lanes ran. Activate the cockpit formation with `/moa gpu`; run the explicit runner with `node scripts/gpu-comp-local-moa.mjs watch --submission <submission.py>`."
    }

    fn with_coordinator_prompt(messages: &[ChatMsg]) -> Vec<ChatMsg> {
        let prompt = Self::coordinator_prompt();
        match messages.split_first() {
            Some((first, rest)) if first.role == ChatRole::System => {
                let mut out = Vec::with_capacity(messages.len());
                out.push(ChatMsg::system(format!(
                    "{prompt}\n\nCockpit context:\n{}",
                    first.content
                )));
                out.extend(rest.iter().cloned());
                out
            }
            _ => {
                let mut out = Vec::with_capacity(messages.len() + 1);
                out.push(ChatMsg::system(prompt));
                out.extend(messages.iter().cloned());
                out
            }
        }
    }

    fn driver_error_text(error: &str) -> String {
        format!(
            "Turbo driver is not reachable from the GPU Comp Local MoA coordinator yet: {error}\n\n{}",
            Self::status_card()
        )
    }
}

impl Club for GpuCompLocalMoaClub {
    fn model_identity(&self) -> Option<String> {
        self.driver.model_identity()
    }

    fn resolved_route_identity(&self) -> RouteIdentity {
        self.driver.resolved_route_identity()
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        match self.chat(&[ChatMsg::user(prompt)], &[])? {
            ClubReply::Text(text) => Ok(text),
            ClubReply::Calls(_) => Ok(
                "GPU Comp Local MoA needs the cockpit tool loop for that request; ask from the normal chat path so I can execute tools."
                    .to_string(),
            ),
        }
    }

    fn label(&self) -> &str {
        "gpu-comp-local-moa"
    }

    fn is_available(&self) -> bool {
        self.driver.is_available()
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        let messages = Self::with_coordinator_prompt(messages);
        self.driver
            .chat(&messages, tools)
            .map_err(|error| Self::driver_error_text(&error))
    }

    fn chat_streaming(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        let messages = Self::with_coordinator_prompt(messages);
        self.driver
            .chat_streaming(&messages, tools, cancel, on_delta)
            .map_err(|error| Self::driver_error_text(&error))
    }

    fn quota_cooldown(&self) -> Option<Duration> {
        self.driver.quota_cooldown()
    }

    fn usage_report(&self) -> Option<String> {
        self.driver.usage_report()
    }

    fn token_usage(&self) -> Option<TokenUsage> {
        self.driver.token_usage()
    }

    fn usage_accounting(&self) -> AccountingView {
        self.driver.usage_accounting()
    }

    fn cache_usage(&self) -> CacheUsage {
        self.driver.cache_usage()
    }

    fn truncation_usage(&self) -> TruncationUsage {
        self.driver.truncation_usage()
    }

    fn effort_gate_usage(&self) -> EffortGateUsage {
        self.driver.effort_gate_usage()
    }

    fn metadata(&self) -> Option<Metadata> {
        self.driver.metadata()
    }

    fn metadata_cached(&self) -> Option<Metadata> {
        self.driver.metadata_cached()
    }
}

#[cfg(test)]
mod tests;

pub(crate) const FINAL_MILE_ANSWER_NUDGE: &str = "[harness-telemetry] FINAL RESPONSE WINDOW. Tool calls are now disabled for the \
    last bounded policy calls. Return the final answer now; do not announce future work or print \
    tool markup. State the retained change and verification evidence, or state that no candidate \
    remains and name the concrete blocker or failed check.";

/// Only a harness-owned final-window directive in the current user turn may
/// disable calls. User/tool text cannot impersonate this transport policy, and
/// a new operator turn resets it without rewriting the retained history.
pub(crate) fn final_response_requested(messages: &[ChatMsg]) -> bool {
    messages
        .iter()
        .rev()
        .take_while(|m| m.role != ChatRole::User)
        .any(|m| {
            m.role == ChatRole::Harness
                && matches!(
                    m.content.as_ref(),
                    FINAL_MILE_ANSWER_NUDGE | crate::harness::research::COMPOSE
                )
        })
}
