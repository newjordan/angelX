//! HttpClub: a real OpenAI-compatible club (chat + tool calls) over HTTP.

use super::*;
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;

mod local_deepseek;

/// Supplies a Bearer token per request (OAuth seats with silent refresh).
pub(crate) type HttpTokenProvider =
    std::sync::Arc<dyn Fn() -> Result<String, String> + Send + Sync>;

enum PendingStreamDelta {
    Content(String),
    Reasoning(String),
}

const TOOL_REASONING_RECEIPT_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Build one cache-routing identity per live club instance. Reusing only the
/// human label (`angel-deepseek`, for example) makes concurrent cockpit
/// processes with different conversations compete for the same provider cache
/// lane. Keep the key stable for this instance, but distinct across processes
/// and separately constructed sessions. The compact shape stays below the
/// 64-character limit used by strict OpenAI-compatible gateways.
fn synth_prompt_cache_key(name: &str) -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    let label: String = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .take(20)
        .collect();
    let label = if label.is_empty() { "club" } else { &label };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed) as u32;
    format!(
        "angel-{label}-{:08x}-{nanos:016x}-{sequence:08x}",
        std::process::id()
    )
}

fn emit_pending_stream_deltas(
    pending: &mut Vec<PendingStreamDelta>,
    on_delta: &mut dyn FnMut(StreamDelta),
) {
    for delta in pending.drain(..) {
        match delta {
            PendingStreamDelta::Content(text) => on_delta(StreamDelta::Content(&text)),
            PendingStreamDelta::Reasoning(text) => on_delta(StreamDelta::Reasoning(&text)),
        }
    }
}

pub(crate) const INCOMPLETE_STREAM_ERR: &str = "provider stream ended before a terminal event";

fn retain_interrupted_stream(
    content: String,
    pending: &mut Vec<PendingStreamDelta>,
    on_delta: &mut dyn FnMut(StreamDelta),
) -> String {
    emit_pending_stream_deltas(pending, on_delta);
    on_delta(StreamDelta::Content(STREAM_INTERRUPTED_SUFFIX));
    format!(
        "{INCOMPLETE_STREAM_ERR}: {}",
        mark_stream_interrupted(&content)
    )
}

/// Error for an SSE stream that died on its data deadline. A stream that
/// produced nothing usable — only keep-alives, blank envelopes, or silence —
/// keeps the historical bare `stream stalled` text (the stall watchdog and the
/// long-session fault classifier read it). A stream that did produce something
/// becomes an *incomplete stream* instead: a half-assembled tool call is
/// discarded outright, streamed prose is kept and marked interrupted, and the
/// turn may replay the hop with the speculative display retracted. Without one
/// shared rule the identical fault was recoverable when the keep-alive branch
/// saw it and fatal when the read-timeout branch did.
fn stalled_stream_error(
    acc: &mut StreamAccumulator,
    stalled_secs: u64,
    bound_name: &str,
    pending: &mut Vec<PendingStreamDelta>,
    on_delta: &mut dyn FnMut(StreamDelta),
) -> String {
    if acc.has_usable_tool_call() {
        return format!("{INCOMPLETE_STREAM_ERR}; incomplete tool call discarded");
    }
    if !acc.content.is_empty() {
        return retain_interrupted_stream(std::mem::take(&mut acc.content), pending, on_delta);
    }
    format!(
        "stream stalled: server kept the connection alive but sent no data \
         for {stalled_secs}s (bound: {bound_name})"
    )
}

/// Process-wide stream hop knobs. `stream_body_with_rules` consults these on
/// every hop, so the env vars are read once. Tests resync under `env_lock`
/// the same way markdown syntax / stall pulse do.
const DEFAULT_STREAM_STALL_SECS: u64 = 45;
const DEFAULT_STREAM_HARD_SECS: u64 = 900;
const DEFAULT_STREAM_TOOL_SILENCE_SECS: u64 = 900;
const STREAM_HEARTBEAT_MIN_INTERVAL_SECS: u64 = 15;
const DEFAULT_STREAM_MAX_LINE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_STREAM_RULE_RETRIES: usize = 2;

static STREAM_STALL_SECS: AtomicU64 = AtomicU64::new(DEFAULT_STREAM_STALL_SECS);
static STREAM_HARD_SECS: AtomicU64 = AtomicU64::new(DEFAULT_STREAM_HARD_SECS);
static STREAM_TOOL_SILENCE_SECS: AtomicU64 = AtomicU64::new(DEFAULT_STREAM_TOOL_SILENCE_SECS);
static STREAM_MAX_LINE_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_STREAM_MAX_LINE_BYTES);
static STREAM_RULE_RETRIES: AtomicUsize = AtomicUsize::new(DEFAULT_STREAM_RULE_RETRIES);
static STREAM_KNOBS_SEEDED: AtomicBool = AtomicBool::new(false);

fn stream_knobs_from_env() -> (Duration, Duration, Duration, usize, usize) {
    (
        env_secs("ANGEL_STREAM_STALL_SECS", DEFAULT_STREAM_STALL_SECS),
        env_secs("ANGEL_STREAM_HARD_SECS", DEFAULT_STREAM_HARD_SECS),
        env_secs(
            "ANGEL_STREAM_TOOL_SILENCE_SECS",
            DEFAULT_STREAM_TOOL_SILENCE_SECS,
        ),
        env_usize("ANGEL_STREAM_MAX_LINE_BYTES", DEFAULT_STREAM_MAX_LINE_BYTES),
        env_usize("ANGEL_STREAM_RULE_RETRIES", DEFAULT_STREAM_RULE_RETRIES),
    )
}

fn seed_stream_knobs_from_env() {
    if STREAM_KNOBS_SEEDED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        let (stall, hard, tool_silence, max_line, retries) = stream_knobs_from_env();
        STREAM_STALL_SECS.store(stall.as_secs(), Ordering::Relaxed);
        STREAM_HARD_SECS.store(hard.as_secs(), Ordering::Relaxed);
        STREAM_TOOL_SILENCE_SECS.store(tool_silence.as_secs(), Ordering::Relaxed);
        STREAM_MAX_LINE_BYTES.store(max_line, Ordering::Relaxed);
        STREAM_RULE_RETRIES.store(retries, Ordering::Relaxed);
    }
}

fn stream_hop_knobs() -> (Duration, Duration, Duration, usize, usize) {
    seed_stream_knobs_from_env();
    (
        Duration::from_secs(STREAM_STALL_SECS.load(Ordering::Relaxed)),
        Duration::from_secs(STREAM_HARD_SECS.load(Ordering::Relaxed)),
        Duration::from_secs(STREAM_TOOL_SILENCE_SECS.load(Ordering::Relaxed)),
        STREAM_MAX_LINE_BYTES.load(Ordering::Relaxed),
        STREAM_RULE_RETRIES.load(Ordering::Relaxed),
    )
}

pub(crate) fn identity_stream_stall_secs() -> u64 {
    stream_hop_knobs().0.as_secs()
}

/// Re-read the five stream hop knobs into the cache. Tests that hold
/// `crate::tests::env_lock()` and mutate the vars must call this so the cache
/// observes the override; call again after the env guard drops to restore.
#[cfg(test)]
pub(crate) fn resync_stream_knobs_from_env() {
    STREAM_KNOBS_SEEDED.store(false, Ordering::Relaxed);
    seed_stream_knobs_from_env();
}

fn stream_read_timed_out(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    )
}

fn contains_qwen_hint(value: &str) -> bool {
    value
        .as_bytes()
        .windows(4)
        .any(|window| window.eq_ignore_ascii_case(b"qwen"))
}

/// SGLang's Qwen tool parser can withhold every byte of a large string
/// argument until its closing delimiter. Admit only a Qwen-named model/club on
/// a private endpoint which was offered tools; every other route keeps the
/// ordinary fail-fast timeout behavior.
fn local_qwen_tool_stream_eligible(
    base_url: &str,
    club_name: &str,
    model: Option<&str>,
    tools_offered: bool,
) -> bool {
    tools_offered
        && is_private_host(base_url)
        && (contains_qwen_hint(club_name) || model.is_some_and(contains_qwen_hint))
}

/// A parsed model delta is the strongest start signal. A partial first `data:`
/// line also counts: a tool-only Qwen response can cross the socket deadline
/// before its first giant JSON event reaches a newline and becomes parseable.
fn local_tool_stream_started(eligible: bool, acc: &StreamAccumulator, partial_line: &[u8]) -> bool {
    let partial_line = partial_line
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(partial_line);
    eligible && (acc.has_model_output() || partial_line.starts_with(b"data:"))
}

/// Heartbeats exist only for the foreground watchdog. Keep-alive-heavy servers
/// may produce many ignored SSE lines per second, so coalesce them here rather
/// than allowing an unbounded zero-byte event backlog.
fn emit_stream_heartbeat(
    last_heartbeat: &mut Option<Instant>,
    on_delta: &mut dyn FnMut(StreamDelta),
) {
    let due = last_heartbeat.is_none_or(|last| {
        last.elapsed() >= Duration::from_secs(STREAM_HEARTBEAT_MIN_INTERVAL_SECS)
    });
    if due {
        *last_heartbeat = Some(Instant::now());
        on_delta(StreamDelta::Heartbeat);
    }
}

/// Process-wide rate-limit wait knobs. `send_with_retry` consults these on
/// every stream send, so the env vars are read once. Tests resync under
/// `env_lock` the same way stream hop knobs / markdown syntax do.
const DEFAULT_RATELIMIT_MAX_WAIT_SECS: u64 = 30;

static RATELIMIT_PROACTIVE: AtomicBool = AtomicBool::new(true);
static RATELIMIT_MAX_WAIT_SECS: AtomicU64 = AtomicU64::new(DEFAULT_RATELIMIT_MAX_WAIT_SECS);
static RATELIMIT_KNOBS_SEEDED: AtomicBool = AtomicBool::new(false);

fn ratelimit_proactive_from_env() -> bool {
    !std::env::var("ANGEL_RATELIMIT_PROACTIVE")
        .map(|v| v == "0")
        .unwrap_or(false)
}

fn ratelimit_knobs_from_env() -> (bool, Duration) {
    (
        ratelimit_proactive_from_env(),
        env_secs("ANGEL_RATELIMIT_MAX_WAIT", DEFAULT_RATELIMIT_MAX_WAIT_SECS),
    )
}

fn seed_ratelimit_knobs_from_env() {
    if RATELIMIT_KNOBS_SEEDED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        let (proactive, max_wait) = ratelimit_knobs_from_env();
        RATELIMIT_PROACTIVE.store(proactive, Ordering::Relaxed);
        RATELIMIT_MAX_WAIT_SECS.store(max_wait.as_secs(), Ordering::Relaxed);
    }
}

fn ratelimit_send_knobs() -> (bool, Duration) {
    seed_ratelimit_knobs_from_env();
    (
        RATELIMIT_PROACTIVE.load(Ordering::Relaxed),
        Duration::from_secs(RATELIMIT_MAX_WAIT_SECS.load(Ordering::Relaxed)),
    )
}

/// Re-read the two rate-limit send knobs into the cache. Tests that hold
/// `crate::tests::env_lock()` and mutate the vars must call this so the cache
/// observes the override; call again after the env guard drops to restore.
#[cfg(test)]
pub(crate) fn resync_ratelimit_knobs_from_env() {
    RATELIMIT_KNOBS_SEEDED.store(false, Ordering::Relaxed);
    seed_ratelimit_knobs_from_env();
}

/// The provider contract a seat was *configured* for.
///
/// Carried on the club so provider-specific behavior — private-reasoning replay
/// and declared input modalities — survives an operator-set base URL (a local
/// forwarder or gateway that relays the real provider) without guessing from
/// the host or the model name. `HttpClub::new` (generic/ad-hoc construction:
/// phone targets, test doubles, one-off routes) carries `None`, so every such
/// club keeps provider-private state withheld.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderContract {
    /// A seat wired through the configured DeepSeek namespace
    /// (`ANGEL_DEEPSEEK_*`, including an `ANGEL_DEEPSEEK_URL` forwarder).
    DeepSeek,
}

pub struct HttpClub {
    name: String,
    base_url: String,
    /// Configured model id, or the id learned from the live `/models` response.
    /// `None` means dynamic: resolve it before the first request instead of
    /// hardcoding a checkpoint in source.
    model: Mutex<Option<String>>,
    api_key: Option<String>,
    /// First-class OAuth (or other rotating) credential. Wins over `api_key`
    /// when set so seats like Grok OAuth never bake a stale JWT into the club.
    token_provider: Option<HttpTokenProvider>,
    /// Derived once from the immutable name/base URL: the env namespace and the
    /// per-club knob names built from it. `reasoning_effort()` and
    /// `route_metadata()` run on the draw path every frame (header + agent
    /// rails); rebuilding these `format!` strings per call was allocator churn.
    env_prefix: String,
    effort_env_name: String,
    dialect_env_name: String,
    local_reasoning_profile: local_deepseek::ReasoningProfile,
    max_tokens_env_name: String,
    /// Lowercased base URL for hop-path dialect / openrouter checks (immutable).
    base_url_lc: String,
    /// Stable, instance-scoped default `prompt_cache_key` — built once.
    default_prompt_cache_key: String,
    /// Per-club pin env names precomputed so hops don't format! them.
    prompt_cache_pin_env: String,
    stream_usage_pin_env: String,
    /// Pooled connection with tuned timeouts — reused across hops so a slow LAN
    /// doesn't pay a fresh TCP + slow-start handshake on every single call.
    agent: ureq::Agent,
    policy: HttpPolicy,
    /// Last seen `(requests_remaining, window_reset_at)` from rate-limit headers.
    /// `run`-set on each response, read before the next to back off proactively.
    rate_limit: Mutex<Option<(u64, Instant)>>,
    /// Lazily-detected model metadata + when it was probed. `None` until the first
    /// `metadata()` call probes the backend; re-probed after a TTL so a server
    /// restarted with a new `--ctx-size` is picked up WITHOUT a cockpit restart. A
    /// transient probe failure during a server swap keeps the last good window.
    metadata: Mutex<Option<(Metadata, Instant)>>,
    /// Single-flight gate for metadata refreshes. An 80-way swarm shares one
    /// `HttpClub`; without this gate every cold caller can independently issue
    /// `/props` + `/models` probes before the first result reaches the cache.
    metadata_probe: Mutex<()>,

    /// In-memory, validated reasoning-effort override chosen from the agent
    /// panel's THINK deck. Wins over the per-club/global env fallback for
    /// HTTP-backed models; `None` keeps the env-only behavior byte-identical.
    effort_override: Mutex<Option<String>>,
    /// Test-injectable clock for the snapshot TTL. The real clock is the
    /// default; tests swap it so a >150ms scheduler stall can never expire a
    /// 'fresh' stamp mid-assert (the historical flake).
    now: Arc<dyn Fn() -> Instant + Send + Sync>,
    /// Revision+TTL snapshot of dialect/gate/override/env for the draw path
    /// (and body builds when the snapshot is armed). One lock replaces the
    /// ~5 mutex + env reads `reasoning_effort` used to pay every frame.
    /// Keyed on `route_state_revision` so THINK/capability mutations invalidate
    /// exactly, and on [`reasoning_env_generation`] so formation pin resyncs
    /// invalidate immediately. The TTL only rebuilds from the env cache.
    effort_snapshot: Mutex<Option<(u64, u64, Instant, EffortSnapshot)>>,
    /// OpenAI-compatible token usage from response `usage` fields.
    usage: UsageCell,
    accounting: super::AccountingCell,
    /// Explicit request breakpoints plus provider-reported cache read/write
    /// tokens. Kept cumulative so a turn can record an exact before/after delta.
    cache_usage: CacheUsageCell,
    /// Output-cap retry outcomes are tracked independently because truncated
    /// provider responses frequently omit token-usage fields.
    truncation: Mutex<TruncationUsage>,
    /// Sticky capability truth learned from the backend itself: the server's own
    /// words after it rejected the reasoning field we sent. Once set, the club
    /// stops sending reasoning controls and the THINK deck goes dark for it.
    /// Deliberately separate from `metadata` so a TTL re-probe can't launder the
    /// lesson away. Process-lifetime — a backend upgraded mid-session re-earns
    /// the field on the next cockpit start.
    reasoning_rejected: Mutex<Option<String>>,
    /// Highest output budget a truncation recovery actually succeeded at, in
    /// tokens (0 = nothing learned). Long tool-calling turns on GLM-5.x seats
    /// blew through a fixed 8k cap on every turn — each one paying a full
    /// doomed generation before the ladder rescued it. Remembering the level
    /// that worked starts the *next* turn there instead.
    learned_output_budget: AtomicU64,
    /// Monotonic invalidation key for the Bag's immutable route snapshot.
    route_state_revision: AtomicU64,
    /// Cumulative effort-gate events (withheld requests, learned rejections).
    /// The turn loop deltas this around a chat and voices the reason — the gate
    /// that strips an operator's effort must never do it silently.
    effort_gate: Mutex<EffortGateUsage>,
    /// Tolerate `finish_reason=length`: when the provider cuts a reply at its
    /// output-token cap but we still have usable prose (no half-written tool
    /// call), keep the partial text as material instead of failing closed. Set
    /// only for SOTA/metered links (see [`Self::sota_tuned`]) so a long MoA
    /// proposer draft is degraded, not discarded. Off by default → the main
    /// tool-calling loop still fails closed on a cut-off reply.
    keep_truncated: bool,
    /// Output-token budget sent as `max_tokens` when the caller hasn't set an
    /// env override. `None` → nothing is sent (byte-identical default body for
    /// local fleet clubs). SOTA links carry a generous default so a long
    /// instruction doesn't trip a provider's (sometimes low) completion cap.
    /// Whether this club may run pxpipe on eligible image-capable models. Set
    /// only by `sota_tuned`, then gated again by label/model/env at send time.
    pxpipe_candidate: bool,
    /// Whether this club should prepend SOTA brevity instructions. Set only by
    /// `sota_tuned`; env can still disable it globally.
    caveman_candidate: bool,
    /// Track the backend's live model id: when set, every `/models` probe that
    /// reports a *different* id than the cached one adopts it (and drops the
    /// cached metadata, which described the old checkpoint). Off (default), the
    /// first learned/configured id is kept for the life of the club — right for
    /// env-pinned models and cloud endpoints that list a whole catalog. Fleet
    /// boxes get this so swapping the checkpoint on a rig mid-session renames
    /// and re-scores the club without a cockpit restart.
    follow_backend: bool,
    /// Circuit breaker armed by a quota-exhausted 429 (weekly/monthly plan cap,
    /// spent balance): `(retry-at, provider message)`. While armed, every call
    /// fails immediately without touching the network — the window won't reset
    /// for hours, so re-sending just burns latency — and `is_available()` reports
    /// the link down so role pickers and the tab strip route around it. Expires
    /// on its own (see [`quota_cooldown_duration`]); the next call re-probes.
    quota_gate: Mutex<Option<(Instant, String)>>,
    /// Judge-issued provisioning for the *next* body build (see `provision.rs`).
    /// Staged by the chat entries right before `build_body_with_effort` consumes
    /// it, so a truncation retry rebuilds from deterministic heuristics instead
    /// of replaying a stale verdict.
    provision_directive: Mutex<Option<ProvisionDirective>>,
    /// Provider contract this seat was configured for (`None` = generic/ad-hoc).
    /// Explicit configuration, never inferred at request time from a URL or a
    /// model string, so a route that merely *looks* DeepSeek cannot inherit
    /// provider-private reasoning replay or the cloud model's capabilities.
    provider_contract: Option<ProviderContract>,
}

/// Authority attached to this exact built request, never sent as provider JSON.
/// Operator limits are ceilings; inferred judge defaults may adapt within the
/// existing bounded truncation ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestOutputBudget {
    ProviderNative,
    OperatorLimit(u32),
    InferredDefault(u32),
}
impl RequestOutputBudget {
    fn retry_schedule(self, sent: Option<u64>) -> Vec<u64> {
        match self {
            Self::InferredDefault(default) => {
                debug_assert!(sent.is_some_and(|tokens| tokens >= u64::from(default)));
                truncation_retry_schedule_from_env(sent)
            }
            Self::ProviderNative | Self::OperatorLimit(_) => Vec::new(),
        }
    }
}

/// One provider attempt may repeat cumulative `usage` on several stream
/// frames. Keep the latest reported value per counter and publish once when the attempt
/// leaves scope (success, retry, cancellation, or failure), matching the wire
/// contract without double-counting a single request.
struct StreamUsageCommit<'a> {
    club: &'a HttpClub,
    latest: Option<super::usage::ReportedUsage>,
    accounting: super::AccountingAttempt<'a>,
}

impl<'a> StreamUsageCommit<'a> {
    #[cfg(test)]
    fn new(club: &'a HttpClub) -> Self {
        Self::from_attempt(club, club.accounting.attempt())
    }
    fn from_attempt(club: &'a HttpClub, accounting: super::AccountingAttempt<'a>) -> Self {
        Self {
            club,
            latest: None,
            accounting,
        }
    }

    fn observe(&mut self, frame: &serde_json::Value) {
        if let Some(newer) = frame.get("usage").and_then(super::usage::parse_http_usage) {
            self.latest
                .get_or_insert_with(Default::default)
                .update(newer);
            if let Some(usage) = self.latest {
                self.accounting
                    .observe(Some(self.club.accounting_observation(usage)));
            }
        }
    }
}

impl Drop for StreamUsageCommit<'_> {
    fn drop(&mut self) {
        if let Some(usage) = self.latest.take() {
            self.club.record_usage_observation(usage);
            self.accounting
                .observe(Some(self.club.accounting_observation(usage)));
        }
    }
}

/// Fallback contract for ambiguous input/output aliases. Chat Completions
/// prompt/completion fields select their inclusive contract at observation time.
fn accounting_contract_for_url(base: &str) -> super::UsageContract {
    use super::{CacheConvention, ReasoningConvention, UsageContract};
    let Ok(url) = url::Url::parse(base) else {
        return UsageContract::default();
    };
    if url.scheme() != "https" {
        return UsageContract::default();
    }
    match url.host_str() {
        Some("api.openai.com" | "api.deepseek.com" | "openrouter.ai") => UsageContract {
            cache: CacheConvention::Included,
            reasoning: ReasoningConvention::Included,
        },
        Some("api.z.ai" | "open.bigmodel.cn") => UsageContract {
            cache: CacheConvention::Included,
            reasoning: ReasoningConvention::Unknown,
        },
        _ => UsageContract::default(),
    }
}

/// Provider-authored ids of DeepSeek's V4 Chat Completions family.
/// `deepseek-flash` is the live V4.1 Flash id; `deepseek-v4-flash` and the
/// experimental `deepseek-v4-flash-vision-exp` are the retired ids it
/// temporarily aliases. Matches are exact — the Spark-local V4 serve
/// (`deepseek-v4-flash-dspark`) is a different, text-only checkpoint.
pub(crate) fn is_deepseek_v4_flash(model_l: &str) -> bool {
    matches!(
        model_l,
        "deepseek-flash" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp"
    )
}

/// DeepSeek V4 Pro — the separate text-only Pro seat (service and billing
/// continue unchanged after the V4.1 Flash rename).
pub(crate) fn is_deepseek_v4_pro(model_l: &str) -> bool {
    model_l == "deepseek-v4-pro"
}

/// Either current DeepSeek V4 seat. The static window/cache map, the canonical
/// THINK ladder, and the provider-route gate all key off this one predicate so
/// the id set lives in exactly one place.
pub(crate) fn is_deepseek_v4_model(model_l: &str) -> bool {
    is_deepseek_v4_pro(model_l) || is_deepseek_v4_flash(model_l)
}

/// Input modalities the provider documents for a known cloud model id — the
/// same static model map as [`static_model_window`], so capability facts about
/// an id live in one place. `None` means *not declared*.
///
/// Current DeepSeek V4.1 Flash (`deepseek-flash`, plus the retired ids that
/// alias it) accepts native `image_url` base64 parts — PNG/JPEG/GIF/WebP in
/// user messages on Chat Completions — so it declares `text+image`; V4 Pro and
/// the retired `deepseek-chat`/`deepseek-reasoner` ids are `text`. Whether a
/// *route* may claim these is decided by the route itself, because the same id
/// string on a local or unknown endpoint is a different checkpoint.
pub(crate) fn static_model_input_modalities(model_l: &str) -> Option<&'static [&'static str]> {
    if is_deepseek_v4_flash(model_l) {
        return Some(&["text", "image"]);
    }
    if is_deepseek_v4_pro(model_l) || matches!(model_l, "deepseek-chat" | "deepseek-reasoner") {
        return Some(&["text"]);
    }
    None
}

/// Static context-window + prompt-cache support for known cloud models that
/// don't expose a live `/props` (llama.cpp) or `max_model_len` (vLLM). Returns
/// `(context_window, supports_cache)`. Conservative and easy to extend.
pub(crate) fn static_model_window(model_l: &str) -> Option<(usize, bool)> {
    let m = model_l;
    if is_deepseek_v4_model(m) {
        // DeepSeek's current V4 API models both publish a 1M-token context and
        // retain the provider's always-on context cache.
        return Some((1_000_000, true));
    }
    if matches!(m, "glm-5.3" | "glm-5.3-flash" | "glm-5.2") {
        // Z.ai GLM-5.3 / GLM-5.3-Flash / GLM-5.2: 1M context, measured prefix
        // cache on the coding-plan endpoint (see backend_prompt_cache_capable).
        return Some((1_000_000, true));
    }
    if m == "grok-4.6" {
        // xAI publishes a 500k context. Its Chat Completions cache affinity is
        // expressed through an x-grok-conv-id header rather than angel0's
        // OpenAI-style body key, so keep that separate capability conservative.
        return Some((500_000, false));
    }
    if m.contains("gpt-5") || m.contains("codex") {
        return Some((400_000, true));
    }
    if m.contains("o3") || m.contains("o1") {
        return Some((200_000, true));
    }
    if m.contains("claude") || m.contains("opus") || m.contains("sonnet") {
        return Some((200_000, true));
    }
    if m.contains("gpt-4o") || m.contains("gpt-4.1") || m.contains("gpt-4") {
        return Some((128_000, true));
    }
    if m.contains("tencent/hy3") || m.contains("hy3") {
        return Some((262_000, true));
    }
    if m.contains("nemotron-3-ultra")
        || m.contains("nemotron-3.5-lightning")
        || m.contains("ox-alpha")
        || m.contains("inkling")
    {
        // OpenRouter free / $0 breadth routes. Catalog context; no advertised
        // prompt cache on the free tier.
        return Some((1_000_000, false));
    }
    if m.contains("nemotron-3-super") || m.contains("laguna") || m.contains("union-alpha") {
        return Some((262_144, false));
    }
    if m.contains("north-mini-code") || m.contains("glm-5.2:free") {
        return Some((256_000, false));
    }
    if m == "openrouter/free" {
        return Some((200_000, false));
    }
    if m == "k3-256k" || m.contains("kimi-for-coding") {
        // Kimi Code plan routes: K2.7 Coding (both speeds) and the 256k K3.
        return Some((262_144, true));
    }
    if m == "k3" || m.contains("kimi-k3") {
        // Reported by Moonshot's authenticated model catalog; the Kimi Code
        // plan calls the same model `k3`. Cache support measured, not folklore:
        // a 2026-08-10 paired probe against api.kimi.com/coding/v1 reported
        // `cached_tokens: 2030/2030` on the second identical request — a full
        // automatic prefix hit, OpenAI-dialect accounting, and a usage frame
        // under `stream_options.include_usage`.
        return Some((1_048_576, true));
    }
    if m.contains("qwen") {
        // Alibaba DashScope Qwen / Qwen Coding Plan: 1M or 262k context, automatic prefix cache.
        if m.contains("qwen3.7")
            || m.contains("qwen3-coder")
            || m.contains("qwen-plus")
            || m.contains("qwen-max")
        {
            return Some((1_000_000, true));
        }
        return Some((262_144, true));
    }
    None
}

/// True when `base_url` points at a private/LAN/tailnet host (loopback, RFC-1918,
/// CGNAT 100.64/10 — which covers tailscale's 100.x addresses — or a bare
/// hostname with no dots). Used to pick a fail-fast connect timeout for fleet
/// boxes while WAN SOTA endpoints keep the conservative default.
pub(crate) fn is_private_host(base_url: &str) -> bool {
    let rest = base_url
        .trim()
        .strip_prefix("http://")
        .or_else(|| base_url.trim().strip_prefix("https://"))
        .unwrap_or(base_url.trim());
    let authority = rest
        .split(['/', '?'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    // Bracketed IPv6 must be peeled as a unit. Splitting `[::1]:8000` on `:`
    // first yields an empty host and incorrectly applies WAN timeouts/pooling.
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        bracketed
            .split_once(']')
            .map(|(host, _)| host)
            .unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    if host.is_empty() {
        return false;
    }
    if host == "localhost" {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                let [a, b, ..] = v4.octets();
                v4.is_loopback() || v4.is_private() || (a == 100 && (64..=127).contains(&b))
            }
            std::net::IpAddr::V6(v6) => {
                let first = v6.segments()[0];
                v6.is_loopback()
                    || (first & 0xfe00) == 0xfc00 // unique-local fc00::/7
                    || (first & 0xffc0) == 0xfe80 // link-local fe80::/10
            }
        };
    }
    if !host.contains('.') {
        return true; // bare LAN/tailnet hostname
    }
    let octets: Vec<u8> = host.split('.').filter_map(|o| o.parse().ok()).collect();
    match octets.as_slice() {
        [127, ..] | [10, ..] | [192, 168, ..] => true,
        [172, b, ..] => (16..=31).contains(b),
        [100, b, ..] => (64..=127).contains(b), // CGNAT incl. tailscale 100.64/10
        _ => false,
    }
}

/// Reusable sockets retained per backend. Local inference servers are routinely
/// driven by 80-way waves, so retaining only ureq's old 16 sockets forced most
/// agents to reconnect before every stage. WAN providers keep the conservative
/// pool; operators can override either with `ANGEL_HTTP_IDLE_CONNECTIONS`.
pub(crate) fn idle_connections_per_host(base_url: &str) -> usize {
    if let Some(n) = idle_connections_override() {
        return n;
    }
    if is_private_host(base_url) { 96 } else { 16 }
}

fn idle_connections_override() -> Option<usize> {
    #[cfg(not(test))]
    {
        static OVERRIDE: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
        *OVERRIDE.get_or_init(idle_connections_override_from_env)
    }
    #[cfg(test)]
    idle_connections_override_from_env()
}

fn idle_connections_override_from_env() -> Option<usize> {
    std::env::var("ANGEL_HTTP_IDLE_CONNECTIONS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
        .map(|n| n.min(512))
}

/// Bounded THINK ladder offered by the agent panel for HTTP-backed models —
/// the canonical lowercase values sent as `reasoning_effort`.
const HTTP_EFFORT_LEVELS: [&str; 4] = ["none", "low", "medium", "high"];
const GROK_46_HTTP_EFFORT_LEVELS: [&str; 4] = ["low", "medium", "high", "xhigh"];
/// Current DeepSeek V4 thinking-mode controls (verified 2026-09-17 against the
/// provider's thinking-mode guide): thinking is on by default at `high`, the
/// depth rungs are `low` / `high` / `max` (`minimal`→low, `medium`/`xhigh`→high,
/// `ultra`→max), and `none` is the documented `thinking: {type: disabled}`
/// toggle — a switch, never a `reasoning_effort` rung. An unset operator control
/// sends neither field, leaving the provider's own default in force.
const DEEPSEEK_V4_HTTP_EFFORT_LEVELS: [&str; 4] = ["none", "low", "high", "max"];
/// GLM-5.3 / GLM-5.3-Flash accept `reasoning_effort` rungs low / high / max
/// with thinking locked on (verified 2026-08-28 against the coding endpoint:
/// `low` bounds reasoning_tokens; `thinking.type: "disabled"` is rejected).
const GLM_53_LOCKED_HTTP_EFFORT_LEVELS: [&str; 3] = ["low", "high", "max"];

/// How a backend expects its reasoning/thinking control spelled on the wire.
/// One canonical operator ladder (`none/low/medium/high`) is translated into
/// each provider's native dialect at request-build time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReasoningDialect {
    /// OpenAI-style top-level `reasoning_effort` string — o-series,
    /// deepseek-reasoner, OpenRouter, and most compatible servers.
    OpenAiEffort,
    /// Zhipu/Z.ai GLM `thinking: {"type": "enabled"|"disabled"}` block.
    GlmThinking,
    /// Qwen3-family `chat_template_kwargs.enable_thinking` boolean
    /// (vLLM / llama.cpp OpenAI-compatible servers).
    QwenEnableThinking,
}

/// The THINK ladder a dialect can express. Binary thinking toggles offer two
/// honest rungs instead of pretending four distinct efforts exist.
fn dialect_effort_levels(dialect: ReasoningDialect, model_l: &str) -> &'static [&'static str] {
    match dialect {
        ReasoningDialect::OpenAiEffort if model_l == "grok-4.6" => &GROK_46_HTTP_EFFORT_LEVELS,
        ReasoningDialect::OpenAiEffort if is_deepseek_v4_model(model_l) => {
            &DEEPSEEK_V4_HTTP_EFFORT_LEVELS
        }
        ReasoningDialect::OpenAiEffort => &HTTP_EFFORT_LEVELS,
        ReasoningDialect::GlmThinking if glm_thinking_locked_on(model_l) => {
            &GLM_53_LOCKED_HTTP_EFFORT_LEVELS
        }
        ReasoningDialect::GlmThinking | ReasoningDialect::QwenEnableThinking => &["none", "high"],
    }
}

/// Name-list folklore, demoted to a *promotion-only* hint: a name that
/// guarantees a reasoning model (deepseek-r1, o-series, qwq…) declares
/// `Some(true)`; anything else is `None` — unknown, never a declared "no". The
/// old boolean form manufactured a "no" for every unrecognized model and
/// silently stripped operator-set effort, which reads as the control not
/// existing (the silent-gate law).
pub(crate) fn folklore_reasoning(model_l: &str) -> Option<bool> {
    [
        "r1",
        "deepseek-r",
        "o1",
        "o3",
        "qwq",
        "reason",
        "think",
        "siq",
    ]
    .iter()
    .any(|k| model_l.contains(k))
    .then_some(true)
}

/// Read capability truth out of an OpenAI-compatible `/models` catalog for one
/// model id. Single-model servers (vLLM, llama.cpp) report the loaded window at
/// `data[0]`; multi-model catalogs (OpenRouter) are searched by id, where an
/// entry's `context_length` gives the real window and its
/// `supported_parameters` list — when present — is a genuine capability
/// declaration for reasoning controls, not a guess.
pub(crate) fn catalog_capabilities(
    v: &serde_json::Value,
    model: &str,
) -> (Option<usize>, Option<bool>) {
    let by_id = v.get("data").and_then(|d| d.as_array()).and_then(|list| {
        list.iter()
            .find(|e| e.get("id").and_then(|i| i.as_str()) == Some(model))
    });
    let window = v
        .pointer("/data/0/max_model_len")
        .and_then(|x| x.as_u64())
        .or_else(|| v.pointer("/data/0/meta/n_ctx").and_then(|x| x.as_u64()))
        .or_else(|| {
            by_id
                .and_then(|e| e.get("context_length"))
                .and_then(|x| x.as_u64())
        })
        .filter(|&n| n > 0)
        .map(|n| n as usize);
    let declared = by_id
        .and_then(|e| e.get("supported_parameters"))
        .and_then(|p| p.as_array())
        .map(|params| {
            params
                .iter()
                .filter_map(|p| p.as_str())
                .any(|p| matches!(p, "reasoning" | "include_reasoning" | "reasoning_effort"))
        });
    (window, declared)
}

/// Does this error read as the backend rejecting the reasoning field itself,
/// rather than a transport, auth, or model failure? Conservative on purpose:
/// only client-rejection classes (HTTP 400/422 or a 200 `{"error":…}`
/// envelope), and only when the message names the field this dialect puts on
/// the wire.
pub(crate) fn is_reasoning_field_rejection(error: &str, dialect: ReasoningDialect) -> bool {
    let rejection_class = error.starts_with("HTTP 400")
        || error.starts_with("HTTP 422")
        || error.starts_with("api error:");
    if !rejection_class {
        return false;
    }
    let e = error.to_ascii_lowercase();
    match dialect {
        ReasoningDialect::OpenAiEffort => {
            e.contains("reasoning_effort") || e.contains("reasoning.effort")
        }
        ReasoningDialect::GlmThinking => e.contains("thinking"),
        ReasoningDialect::QwenEnableThinking => {
            e.contains("enable_thinking") || e.contains("chat_template_kwargs")
        }
    }
}

/// Stable env namespace for provider-specific request knobs. Aggregators use
/// model ids as their UI label (`tencent/hy3:free`), which is not a usable
/// shell-variable suffix; recognize the provider URL before falling back to
/// a sanitized club label.
fn env_prefix_for(name: &str, base_url: &str) -> String {
    if base_url.to_ascii_lowercase().contains("openrouter.ai") {
        return "OPENROUTER".to_string();
    }
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// How long a draw-path effort snapshot may serve after `route_state_revision`
/// and [`reasoning_env_generation`] last matched. Env strings are seed-once
/// ([`reasoning_env_var`]) until formation/test resync; this TTL only rebuilds
/// from that cache. THINK override and capability mutations bump the revision
/// and invalidate immediately.
const EFFORT_ENV_TTL: Duration = Duration::from_millis(150);

/// Wire-format tool array shared across hops while the schema fingerprint is
/// stable. Process-wide Arc: every HttpClub speaks the same OpenAI function
/// shape, and the fingerprint already keys the set. Callers still take a
/// Value clone for the body (serde owns it), but the rebuild of ~50 function
/// objects is skipped on cache hits.
fn tools_wire_json(tools: &[ToolDef]) -> serde_json::Value {
    use serde_json::json;
    let fp = crate::turn::defs_fingerprint(tools);
    static CACHE: Mutex<Option<(u64, Arc<serde_json::Value>)>> = Mutex::new(None);
    if let Ok(guard) = CACHE.lock()
        && let Some((cached_fp, value)) = guard.as_ref()
        && *cached_fp == fp
    {
        return value.as_ref().clone();
    }
    let defs: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            // Clone params by reference into the JSON tree once; params are
            // already owned Values on the ToolDef.
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.params,
                },
            })
        })
        .collect();
    let value = Arc::new(serde_json::Value::Array(defs));
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((fp, Arc::clone(&value)));
    }
    value.as_ref().clone()
}

/// Serialize a JSON body into a thread-local scratch buffer, then take ownership
/// of the bytes. Avoids the allocator churn of `serde_json::to_vec` growing a
/// fresh Vec on every hop of a multi-turn coding session.
fn encode_json_bytes(body: &serde_json::Value) -> Result<Vec<u8>, serde_json::Error> {
    thread_local! {
        static BUF: std::cell::RefCell<Vec<u8>> =
            std::cell::RefCell::new(Vec::with_capacity(64 * 1024));
    }
    BUF.with(|cell| {
        let mut buf = cell.borrow_mut();
        buf.clear();
        serde_json::to_writer(&mut *buf, body)?;
        Ok(std::mem::take(&mut *buf))
    })
}

/// Process-wide OpenRouter Claude cache knobs
/// (`ANGEL_OPENROUTER_ANTHROPIC_CACHE`, `ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR`,
/// `ANGEL_ANTHROPIC_CACHE_TTL`). Hop-start body-build consults these on every
/// request, so the env is read once. Tests resync under `env_lock` the same
/// way stream-usage / max-tokens / prompt-cache caches do. Breakpoint and TTL
/// send policy is unchanged.
static OPENROUTER_ANTHROPIC_CACHE_CFG: std::sync::OnceLock<Mutex<Option<(bool, usize, bool)>>> =
    std::sync::OnceLock::new();

fn openrouter_anthropic_cache_cfg_slot() -> &'static Mutex<Option<(bool, usize, bool)>> {
    OPENROUTER_ANTHROPIC_CACHE_CFG.get_or_init(|| Mutex::new(None))
}

fn openrouter_anthropic_cache_cfg() -> (bool, usize, bool) {
    let mut slot = openrouter_anthropic_cache_cfg_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(cfg) = *slot {
        return cfg;
    }
    let cfg = openrouter_anthropic_cache_cfg_from_env();
    *slot = Some(cfg);
    cfg
}

fn openrouter_anthropic_cache_cfg_from_env() -> (bool, usize, bool) {
    let enabled = std::env::var("ANGEL_OPENROUTER_ANTHROPIC_CACHE")
        .ok()
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "off" | "false" | "no"
            )
        })
        .unwrap_or(true);
    let floor = std::env::var("ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(1024)
        .clamp(256, 65_536);
    let ttl_1h = std::env::var("ANGEL_ANTHROPIC_CACHE_TTL")
        .ok()
        .is_some_and(|ttl| ttl.trim() == "1h");
    (enabled, floor, ttl_1h)
}

/// Re-read OpenRouter Claude cache knobs into the cache. Tests that hold
/// `crate::tests::env_lock()` and mutate those vars must call this so the
/// cache observes the override; call again after the env guard drops to restore.
#[cfg(test)]
pub(crate) fn resync_openrouter_anthropic_cache_from_env() {
    *openrouter_anthropic_cache_cfg_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PromptCacheMode {
    /// Unset: capability-gated (detected cache backends only).
    Capability,
    /// Explicit truthy: send unless known non-caching.
    ForceOn,
    /// Explicit off.
    Off,
}

fn prompt_cache_global_mode() -> PromptCacheMode {
    #[cfg(not(test))]
    {
        static MODE: std::sync::OnceLock<PromptCacheMode> = std::sync::OnceLock::new();
        *MODE.get_or_init(prompt_cache_global_mode_from_env)
    }
    #[cfg(test)]
    match prompt_cache_club_pin("ANGEL_PROMPT_CACHE") {
        Some(false) => PromptCacheMode::Off,
        Some(true) => PromptCacheMode::ForceOn,
        None => PromptCacheMode::Capability,
    }
}

fn prompt_cache_global_mode_from_env() -> PromptCacheMode {
    match std::env::var("ANGEL_PROMPT_CACHE")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .as_deref()
    {
        Some("0" | "off" | "false" | "no") => PromptCacheMode::Off,
        Some(_) => PromptCacheMode::ForceOn,
        None => PromptCacheMode::Capability,
    }
}

fn prompt_cache_explicit_key_from_env() -> Option<String> {
    std::env::var("ANGEL_PROMPT_CACHE_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Process-wide `ANGEL_PROMPT_CACHE_KEY`. Hop start consults this on every
/// request, so the env is read once. Tests resync under `env_lock` the same
/// way stream-usage / max-tokens caches do.
static PROMPT_CACHE_EXPLICIT_KEY: std::sync::OnceLock<Mutex<Option<Option<String>>>> =
    std::sync::OnceLock::new();

fn prompt_cache_explicit_key_slot() -> &'static Mutex<Option<Option<String>>> {
    PROMPT_CACHE_EXPLICIT_KEY.get_or_init(|| Mutex::new(None))
}

fn prompt_cache_explicit_key() -> Option<String> {
    let mut slot = prompt_cache_explicit_key_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(key) = slot.as_ref() {
        return key.clone();
    }
    let key = prompt_cache_explicit_key_from_env();
    *slot = Some(key.clone());
    key
}

/// Process-wide `ANGEL_<CLUB>_PROMPT_CACHE` pins. Hop-start body-build consults
/// the per-club pin on every hop, so each env name is read once. Tests resync
/// under `env_lock` the same way stream-usage / max-tokens caches do.
static PROMPT_CACHE_CLUB_PINS: std::sync::OnceLock<Mutex<HashMap<String, Option<bool>>>> =
    std::sync::OnceLock::new();

fn prompt_cache_club_pins() -> &'static Mutex<HashMap<String, Option<bool>>> {
    PROMPT_CACHE_CLUB_PINS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn prompt_cache_club_pin(name: &str) -> Option<bool> {
    let mut pins = prompt_cache_club_pins()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(&pin) = pins.get(name) {
        return pin;
    }
    let pin = env_truthy_pin(name);
    pins.insert(name.to_string(), pin);
    pin
}

/// Re-read global and per-club `PROMPT_CACHE` pins and `ANGEL_PROMPT_CACHE_KEY`
/// into the cache. Tests that hold `crate::tests::env_lock()` and mutate those
/// vars must call this so the cache observes the override; call again after
/// the env guard drops to restore.
#[cfg(test)]
pub(crate) fn resync_prompt_cache_from_env() {
    prompt_cache_club_pins()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    *prompt_cache_explicit_key_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// Global `ANGEL_STREAM_USAGE` pin: `Some(true/false)` when set, `None` when
/// unset (fall through to capability detection).
fn stream_usage_global() -> Option<bool> {
    #[cfg(not(test))]
    {
        static PIN: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
        *PIN.get_or_init(|| env_truthy_pin("ANGEL_STREAM_USAGE"))
    }
    #[cfg(test)]
    stream_usage_club_pin("ANGEL_STREAM_USAGE")
}

/// Parse a truthy/falsey env pin. `None` when unset or empty.
fn env_truthy_pin(name: &str) -> Option<bool> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .map(|v| !matches!(v.as_str(), "0" | "off" | "false" | "no"))
}

/// Process-wide `ANGEL_<CLUB>_STREAM_USAGE` pins. Streaming body-build consults
/// the per-club pin on every hop, so each env name is read once. Tests resync
/// under `env_lock` the same way stream hop knobs do.
static STREAM_USAGE_CLUB_PINS: std::sync::OnceLock<Mutex<HashMap<String, Option<bool>>>> =
    std::sync::OnceLock::new();

fn stream_usage_club_pins() -> &'static Mutex<HashMap<String, Option<bool>>> {
    STREAM_USAGE_CLUB_PINS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stream_usage_club_pin(name: &str) -> Option<bool> {
    let mut pins = stream_usage_club_pins()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(&pin) = pins.get(name) {
        return pin;
    }
    let pin = env_truthy_pin(name);
    pins.insert(name.to_string(), pin);
    pin
}

/// Re-read global and per-club `STREAM_USAGE` pins into the cache. Tests that
/// hold `crate::tests::env_lock()` and mutate either var must call this so the
/// cache observes the override; call again after the env guard drops to restore.
#[cfg(test)]
pub(crate) fn resync_stream_usage_pins_from_env() {
    stream_usage_club_pins()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// Process-wide `ANGEL_{CLUB}_MAX_TOKENS` / `ANGEL_CLUB_MAX_TOKENS`. Hop start
/// (`output_budget_policy`) consults these on every request, so each env name
/// is read once. Tests resync under `env_lock` the same way stream hop knobs do.
static MAX_TOKENS_ENV_CACHE: std::sync::OnceLock<Mutex<HashMap<String, Option<u32>>>> =
    std::sync::OnceLock::new();

fn max_tokens_env_cache() -> &'static Mutex<HashMap<String, Option<u32>>> {
    MAX_TOKENS_ENV_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parse_max_tokens_env(name: &str) -> Option<u32> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|tokens| *tokens > 0)
}

fn max_tokens_env(name: &str) -> Option<u32> {
    let mut cache = max_tokens_env_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(&tokens) = cache.get(name) {
        return tokens;
    }
    let tokens = parse_max_tokens_env(name);
    cache.insert(name.to_string(), tokens);
    tokens
}

/// Re-read club max-tokens env names into the cache. Tests that hold
/// `crate::tests::env_lock()` and mutate the vars must call this so the cache
/// observes the override; call again after the env guard drops to restore.
#[cfg(test)]
pub(crate) fn resync_max_tokens_env_from_env() {
    max_tokens_env_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// Process-wide `ANGEL_<CLUB>_REASONING_EFFORT` / `ANGEL_REASONING_EFFORT` /
/// `ANGEL_<CLUB>_REASONING_DIALECT` strings. FrameChrome → `Bag::reasoning_effort`
/// consults these on the draw path, so each name is read once until
/// [`resync_reasoning_effort_env_from_env`] (formation apply / tests). THINK
/// override and `route_state_revision` still invalidate the snapshot.
static REASONING_ENV_CACHE: std::sync::OnceLock<Mutex<HashMap<String, Option<String>>>> =
    std::sync::OnceLock::new();
static REASONING_ENV_GENERATION: AtomicU64 = AtomicU64::new(0);

fn reasoning_env_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    REASONING_ENV_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Seed-once getenv for a reasoning-effort or dialect knob.
pub(crate) fn reasoning_env_var(name: &str) -> Option<String> {
    let mut cache = reasoning_env_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(value) = cache.get(name) {
        return value.clone();
    }
    let value = std::env::var(name).ok();
    cache.insert(name.to_string(), value.clone());
    value
}

pub(crate) fn reasoning_env_generation() -> u64 {
    REASONING_ENV_GENERATION.load(Ordering::Relaxed)
}

/// Re-read reasoning-effort and dialect env strings into the cache. Formation
/// apply/leave call this after writing those knobs so Math God / Grok War pins
/// land on the next draw. Tests that hold `crate::tests::env_lock()` and mutate
/// the vars must call this so the cache observes the override; call again after
/// the env guard drops to restore.
pub(crate) fn resync_reasoning_effort_env_from_env() {
    reasoning_env_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    REASONING_ENV_GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Launch-config metadata re-probe TTL. `0` = probe once and never again.
fn metadata_ttl() -> Duration {
    #[cfg(not(test))]
    {
        static TTL: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
        *TTL.get_or_init(metadata_ttl_from_env)
    }
    #[cfg(test)]
    metadata_ttl_from_env()
}

fn metadata_ttl_from_env() -> Duration {
    Duration::from_secs(
        std::env::var("ANGEL_METADATA_TTL_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(120),
    )
}

/// Launch-config: whether GLM default thinking is forced on (rare escape hatch).
fn glm_thinking_default_on() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(glm_thinking_default_on_from_env)
    }
    #[cfg(test)]
    glm_thinking_default_on_from_env()
}

fn glm_thinking_default_on_from_env() -> bool {
    std::env::var("ANGEL_GLM_THINKING_DEFAULT")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled" | "high"
            )
        })
        .unwrap_or(false)
}

/// GLM-5.3 and GLM-5.3-Flash reject `thinking.type: "disabled"`. Depth is
/// `reasoning_effort` (`low` / `high` / `max`) with thinking left enabled.
fn glm_thinking_locked_on(model: &str) -> bool {
    let m = model.trim().as_bytes();
    m.eq_ignore_ascii_case(b"glm-5.3") || (m.len() >= 8 && m[..8].eq_ignore_ascii_case(b"glm-5.3-"))
}

/// Public policy hook: true when the seat's provider bills reasoning tokens on
/// every request regardless of the operator's wishes (GLM-5.3 / GLM-5.3-Flash
/// reject `thinking.type: "disabled"`). The turn loop uses this to default
/// adaptive reasoning ON for those seats — flat full effort is a thinking tax.
pub fn glm_thinking_locked_for_club(model: &str) -> bool {
    glm_thinking_locked_on(model)
}

fn glm_thinking_is_flash(model: &str) -> bool {
    model
        .as_bytes()
        .windows(13)
        .any(|window| window.eq_ignore_ascii_case(b"glm-5.3-flash"))
}

/// Idle `reasoning_effort` for models that cannot disable thinking. GLM-5.3
/// idles on the closest-to-off rung (`low`). GLM-5.3-Flash idles on `auto`:
/// the field is omitted and z.ai's own adaptive default applies — exactly what
/// opencode sends, and the reason it solved the 2026-09-05 arena tasks 2×
/// faster with the same model (angel0's former Flash idle of `high` made every
/// hop, even a 95-byte `read_file` call, think at full depth: 2–7× the
/// reasoning tokens, ~20 s per hop). `ANGEL_GLM_FLASH_IDLE_EFFORT`
/// (`auto` | `low` | `high` | `max`) pins it.
fn glm_locked_idle_effort(model: &str) -> Option<&'static str> {
    if glm_thinking_is_flash(model) {
        glm_flash_idle_effort_from_env()
    } else {
        Some("low")
    }
}

/// GLM-5.3-Flash idle rung. Default `low`, measured 2026-09-06 in the harness
/// arena (6 tasks × 2 interleaved rounds vs opencode on the same endpoint):
/// `auto` (field omitted) solved 12/12 at a 64 s median with 15.4 k reasoning
/// tokens; `low` held for the whole turn solved 11/11 at a 28 s median with
/// 0.7 k reasoning tokens, faster than opencode in 9 of 11 head-to-heads.
/// `ANGEL_GLM_FLASH_IDLE_EFFORT=auto` restores the omitted field; `high` /
/// `max` pin deeper rungs. One rung per turn: z.ai keys its prefix cache on
/// the effort value, so the rung must not move between hops.
fn glm_flash_idle_effort_from_env() -> Option<&'static str> {
    match std::env::var("ANGEL_GLM_FLASH_IDLE_EFFORT")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("auto") | Some("omit") | Some("none") => None,
        Some("high") => Some("high"),
        Some("max") => Some("max"),
        _ => Some("low"),
    }
}

/// Peer clubs (Grok) share the same TTL contract as the HTTP effort snapshot.
pub(crate) const EFFORT_ENV_TTL_FOR_PEERS: Duration = EFFORT_ENV_TTL;

/// Combined draw/request snapshot of every fact `reasoning_effort` /
/// `reasoning_levels` / body-build consult. Gate-reason strings are preserved
/// verbatim so effort gates keep speaking the same words.
#[derive(Clone, Debug)]
struct EffortSnapshot {
    dialect: ReasoningDialect,
    gate_reason: Option<String>,
    override_effort: Option<String>,
    env_effort: Option<String>,
}

/// Master switch for the revision-keyed effort snapshot. Default on; `0` /
/// `false` / `off` / `no` restore the historical per-call lock+env path.
fn effort_snapshot_enabled() -> bool {
    effort_snapshot_enabled_for_peers()
}

/// Shared with peer clubs (Grok) so one env knob arms every effort snapshot.
pub(crate) fn effort_snapshot_enabled_for_peers() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(effort_snapshot_enabled_from_env)
    }
    #[cfg(test)]
    effort_snapshot_enabled_from_env()
}

fn effort_snapshot_enabled_from_env() -> bool {
    !matches!(
        std::env::var("ANGEL_EFFORT_SNAPSHOT")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(|v| v.to_ascii_lowercase())
            .as_deref(),
        Some("0" | "false" | "off" | "no")
    )
}

impl HttpClub {
    /// Precomputed at construction (see [`env_prefix_for`]) — name and base URL
    /// are immutable for the life of the club.
    fn env_prefix(&self) -> &str {
        &self.env_prefix
    }

    /// The env-configured effort: the per-club name wins over the global,
    /// trimmed, empty filtered out. Seed-once process cache — the draw path
    /// and (when the snapshot is armed) body-build no longer getenv these
    /// knobs until [`resync_reasoning_effort_env_from_env`]. Precedence is
    /// unchanged, so the wire still carries the same effort once a name is
    /// seeded.
    fn effort_env_read(&self) -> Option<String> {
        reasoning_env_var(&self.effort_env_name)
            .or_else(|| reasoning_env_var("ANGEL_REASONING_EFFORT"))
            .map(|effort| effort.trim().to_string())
            .filter(|effort| !effort.is_empty())
            .or_else(|| {
                model_defaults::entry(&self.model_identity().unwrap_or_default())
                    .and_then(|e| e.default_effort)
            })
    }

    fn glm_flash_idle_effort(&self) -> Option<String> {
        let model = self
            .model
            .lock()
            .ok()
            .and_then(|g| g.as_ref().filter(|id| !id.trim().is_empty()).cloned())
            .unwrap_or_default();
        if glm_thinking_is_flash(&model) {
            glm_locked_idle_effort(&model).map(str::to_string)
        } else {
            None
        }
    }

    /// Resolve this club's effort dialect without consulting the snapshot
    /// cache. Used by [`Self::effort_snapshot_build`] and when the snapshot is
    /// disarmed.
    fn resolve_reasoning_dialect(&self) -> ReasoningDialect {
        if let Some(value) = reasoning_env_var(&self.dialect_env_name) {
            match value.trim().to_ascii_lowercase().as_str() {
                "glm" | "thinking" => return ReasoningDialect::GlmThinking,
                "qwen" | "enable-thinking" | "enable_thinking" => {
                    return ReasoningDialect::QwenEnableThinking;
                }
                "openai" | "effort" => return ReasoningDialect::OpenAiEffort,
                "" => {}
                other => eprintln!(
                    "[club:{}] unknown reasoning dialect '{other}' — using the openai spelling \
                     (valid: openai, glm, qwen)",
                    self.name
                ),
            }
        }
        let base = self.base_url_lc.as_str();
        // OpenRouter fronts many models but takes the OpenAI spelling for all.
        if base.contains("openrouter.ai") {
            return ReasoningDialect::OpenAiEffort;
        }
        let model = self.model_id().unwrap_or_default().to_ascii_lowercase();
        if base.contains("z.ai") || base.contains("bigmodel.cn") || model.starts_with("glm") {
            return ReasoningDialect::GlmThinking;
        }
        if model.contains("qwen3") {
            return ReasoningDialect::QwenEnableThinking;
        }
        ReasoningDialect::OpenAiEffort
    }

    /// Resolve this club's effort dialect: an explicit
    /// `ANGEL_<CLUB>_REASONING_DIALECT` (`openai`|`glm`|`qwen`) wins; otherwise
    /// the base URL / model name select the known dialects, defaulting to the
    /// OpenAI spelling. A resolved GLM/Qwen dialect is itself capability truth:
    /// those backends think, whatever the name-substring heuristic guessed — so
    /// dialect-known clubs bypass the `supports_reasoning` suppression gate.
    /// When the effort snapshot is armed, returns the revision-keyed cache.
    pub(crate) fn reasoning_dialect(&self) -> ReasoningDialect {
        if effort_snapshot_enabled() {
            self.effort_snapshot_load().dialect
        } else {
            self.resolve_reasoning_dialect()
        }
    }

    /// Build a fresh effort snapshot from live locks/env (no cache consult).
    fn effort_snapshot_build(&self) -> EffortSnapshot {
        let dialect = self.resolve_reasoning_dialect();
        EffortSnapshot {
            dialect,
            gate_reason: self.reasoning_gate_reason(dialect),
            override_effort: self.effort_override.lock().ok().and_then(|g| g.clone()),
            env_effort: self.effort_env_read(),
        }
    }

    /// Revision+TTL effort snapshot. Invalidates immediately when
    /// `route_state_revision` bumps (THINK override, rejection learn, model
    /// swap) or [`reasoning_env_generation`] bumps (formation pin resync).
    /// After [`EFFORT_ENV_TTL`] the snapshot rebuilds from the env cache.
    fn effort_snapshot_load(&self) -> EffortSnapshot {
        let rev = self.route_state_revision.load(Ordering::Relaxed);
        let env_gen = reasoning_env_generation();
        let Ok(mut g) = self.effort_snapshot.lock() else {
            return self.effort_snapshot_build();
        };
        if let Some((cached_rev, cached_gen, at, snap)) = g.as_ref()
            && *cached_rev == rev
            && *cached_gen == env_gen
            && (self.now)().saturating_duration_since(*at) < EFFORT_ENV_TTL
        {
            return snap.clone();
        }
        let snap = self.effort_snapshot_build();
        *g = Some((rev, env_gen, (self.now)(), snap.clone()));
        snap
    }

    fn output_budget_policy(&self) -> (OutputBudgetPolicy, Option<String>) {
        // Seed-once process cache — hop start no longer getenv's these knobs.
        if let Some(tokens) = max_tokens_env(&self.max_tokens_env_name) {
            return (
                OutputBudgetPolicy::Explicit {
                    tokens,
                    source: OutputBudgetSource::PerClubEnv,
                },
                Some(self.max_tokens_env_name.clone()),
            );
        }
        if let Some(tokens) = max_tokens_env("ANGEL_CLUB_MAX_TOKENS") {
            return (
                OutputBudgetPolicy::Explicit {
                    tokens,
                    source: OutputBudgetSource::GlobalEnv,
                },
                Some("ANGEL_CLUB_MAX_TOKENS".to_string()),
            );
        }
        (OutputBudgetPolicy::ProviderNative, None)
    }

    fn truncation_policy(&self, sent: Option<u64>) -> OutputBudgetPolicy {
        let Some(tokens) = sent.and_then(|tokens| u32::try_from(tokens).ok()) else {
            return OutputBudgetPolicy::ProviderNative;
        };
        let source = match self.output_budget_policy().0 {
            OutputBudgetPolicy::Explicit { source, .. } => source,
            _ => OutputBudgetSource::PerClubEnv,
        };
        OutputBudgetPolicy::Explicit { tokens, source }
    }

    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        let base_url = base_url.into();
        let mut policy = HttpPolicy::from_env();
        // A LAN/tailnet host that SYN-blackholes (crashed box) would hold each
        // attempt the full WAN-sized connect window, and that unit multiplies
        // through retries and the failover chain. Private hosts answer in
        // milliseconds when up — fail fast unless the operator pinned a timeout.
        if std::env::var_os("ANGEL_HTTP_CONNECT_TIMEOUT").is_none() && is_private_host(&base_url) {
            policy.connect_timeout = Duration::from_secs(3);
        }
        let agent = ureq::AgentBuilder::new()
            // Provider requests carry bearer credentials. Never replay them to
            // a redirect target; endpoint changes must be explicit config.
            .redirects(0)
            .timeout_connect(policy.connect_timeout)
            .timeout_read(policy.read_timeout)
            .timeout_write(policy.write_timeout)
            // Preserve a full local swarm wave across stages instead of making
            // 64+ agents reconnect; WAN links retain a smaller pool.
            .max_idle_connections_per_host(idle_connections_per_host(&base_url))
            .user_agent(concat!("angel0-cockpit/", env!("CARGO_PKG_VERSION")))
            .build();
        let model = model.into();
        let model = (!model.trim().is_empty()).then(|| model.trim().to_string());
        let name = name.into();
        let env_prefix = env_prefix_for(&name, &base_url);
        let local_reasoning_profile =
            local_deepseek::ReasoningProfile::from_env(&env_prefix, &base_url, model.as_deref());
        let base_url_lc = base_url.to_ascii_lowercase();
        Self {
            effort_env_name: format!("ANGEL_{env_prefix}_REASONING_EFFORT"),
            dialect_env_name: format!("ANGEL_{env_prefix}_REASONING_DIALECT"),
            local_reasoning_profile,
            max_tokens_env_name: format!("ANGEL_{env_prefix}_MAX_TOKENS"),
            prompt_cache_pin_env: format!("ANGEL_{env_prefix}_PROMPT_CACHE"),
            stream_usage_pin_env: format!("ANGEL_{env_prefix}_STREAM_USAGE"),
            default_prompt_cache_key: synth_prompt_cache_key(&name),
            base_url_lc,
            env_prefix,
            name,
            base_url,
            model: Mutex::new(model),
            api_key,
            token_provider: None,
            agent,
            policy,
            rate_limit: Mutex::new(None),
            metadata: Mutex::new(None),
            metadata_probe: Mutex::new(()),
            effort_override: Mutex::new(None),
            now: Arc::new(Instant::now),
            effort_snapshot: Mutex::new(None),
            usage: UsageCell::default(),
            accounting: super::AccountingCell::default(),
            cache_usage: CacheUsageCell::default(),
            truncation: Mutex::new(TruncationUsage::default()),
            reasoning_rejected: Mutex::new(None),
            learned_output_budget: AtomicU64::new(0),
            route_state_revision: AtomicU64::new(0),
            effort_gate: Mutex::new(EffortGateUsage::default()),
            keep_truncated: false,
            pxpipe_candidate: false,
            caveman_candidate: false,
            follow_backend: false,
            quota_gate: Mutex::new(None),
            provision_directive: Mutex::new(None),
            provider_contract: None,
        }
    }

    /// Bind provider controls independently from the displayed catalog model.
    /// Called during construction, before any effort or usage cache is read.
    pub(crate) fn with_env_namespace(mut self, namespace: &str) -> Self {
        let prefix = env_prefix_for(namespace, "");
        self.effort_env_name = format!("ANGEL_{prefix}_REASONING_EFFORT");
        self.dialect_env_name = format!("ANGEL_{prefix}_REASONING_DIALECT");
        self.max_tokens_env_name = format!("ANGEL_{prefix}_MAX_TOKENS");
        self.prompt_cache_pin_env = format!("ANGEL_{prefix}_PROMPT_CACHE");
        self.stream_usage_pin_env = format!("ANGEL_{prefix}_STREAM_USAGE");
        self.local_reasoning_profile = local_deepseek::ReasoningProfile::from_env(
            &prefix,
            &self.base_url,
            self.model
                .lock()
                .ok()
                .and_then(|model| model.clone())
                .as_deref(),
        );
        self.env_prefix = prefix;
        self
    }

    /// Declare the provider contract this seat was *configured* for. Only the
    /// seat constructors that read the provider's own namespace call this, so a
    /// forwarder/gateway URL set by the operator keeps the provider's contract
    /// while a club that merely reuses a provider id or name does not.
    pub(crate) fn with_provider_contract(mut self, contract: ProviderContract) -> Self {
        self.provider_contract = Some(contract);
        self
    }

    /// Attach a first-class rotating Bearer provider (OAuth access tokens).
    /// The provider is called on every request so silent refresh stays live.
    pub fn with_token_provider(mut self, provider: HttpTokenProvider) -> Self {
        self.token_provider = Some(provider);
        self
    }

    /// Resolve the Authorization Bearer value for this request.
    fn bearer_token(&self) -> Result<Option<String>, String> {
        if let Some(provider) = &self.token_provider {
            return Ok(Some(provider()?));
        }
        Ok(self.api_key.clone())
    }

    /// Follow the backend's live model id (see the `follow_backend` field). For
    /// fleet endpoints where the operator swaps checkpoints freely; never for
    /// env-pinned models or multi-model cloud catalogs.
    pub fn follow_backend(mut self) -> Self {
        self.follow_backend = true;
        self
    }

    /// Tune a metered SOTA link for the mixture-of-agents path: tolerate output
    /// truncation (keep the partial prose draft rather than fail closed). Output
    /// length remains provider-native unless an operator explicitly sets
    /// `ANGEL_{CLUB}_MAX_TOKENS` / `ANGEL_CLUB_MAX_TOKENS`.
    pub fn sota_tuned(mut self) -> Self {
        self.keep_truncated = true;
        self.pxpipe_candidate = true;
        self.caveman_candidate = true;
        self
    }

    fn accounting_observation(
        &self,
        usage: super::usage::ReportedUsage,
    ) -> super::UsageObservation {
        super::UsageObservation {
            raw: [
                usage.input,
                usage.output,
                usage.reasoning,
                usage.cache_read,
                usage.cache_write,
            ],
            paths: usage.paths,
            // Direct xAI Chat Completions is an explicit exception to the
            // inclusive completion convention: total = prompt + completion +
            // reasoning. Confirmed by real RIPEMD Grok receipts and xAI's REST
            // reference. Do not extend this to proxies based on model names.
            contract: if usage.paths[0] == Some("prompt_tokens")
                && usage.paths[1] == Some("completion_tokens")
            {
                super::UsageContract {
                    cache: super::CacheConvention::Included,
                    reasoning: if url::Url::parse(&self.base_url).is_ok_and(|url| {
                        url.scheme() == "https" && url.host_str() == Some("api.x.ai")
                    }) {
                        super::ReasoningConvention::Separate
                    } else {
                        super::ReasoningConvention::Included
                    },
                }
            } else {
                accounting_contract_for_url(&self.base_url)
            },
        }
    }

    pub(crate) fn record_usage_json(&self, v: &serde_json::Value) {
        let Some(usage) = v.get("usage") else {
            return;
        };
        self.record_usage_value(usage);
    }

    fn record_usage_value(&self, value: &serde_json::Value) {
        if let Some(usage) = super::usage::parse_http_usage(value) {
            self.record_usage_observation(usage);
        }
    }

    fn record_usage_observation(&self, usage: super::usage::ReportedUsage) {
        if let Some(tokens) = usage.cache_read {
            self.cache_usage.add_read(tokens);
        }
        if let Some(tokens) = usage.cache_write {
            self.cache_usage.add_write(tokens);
        }
        // A valid zero is an observation. Partial-field availability remains
        // explicit in the parser; the legacy meter stores observed totals.
        if usage.has_tokens() {
            let generation = self
                .accounting_observation(usage)
                .generation_output()
                .unwrap_or_else(|| usage.output.unwrap_or(0));
            self.usage.record_turn(
                usage.input.unwrap_or(0),
                generation,
                usage.reasoning.unwrap_or(0),
            );
        }
    }

    fn record_retained_truncation(&self) {
        if let Ok(mut stats) = self.truncation.lock() {
            stats.episodes = stats.episodes.saturating_add(1);
            stats.retained_partials = stats.retained_partials.saturating_add(1);
        }
    }

    fn record_truncation(&self, retries: usize, recovered: bool) {
        if let Ok(mut stats) = self.truncation.lock() {
            stats.episodes = stats.episodes.saturating_add(1);
            stats.retries = stats.retries.saturating_add(retries as u64);
            if recovered {
                stats.recoveries = stats.recoveries.saturating_add(1);
            } else {
                stats.failures = stats.failures.saturating_add(1);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn stage_inferred_output_budget_for_test(&self, tokens: u32) {
        *self.provision_directive.lock().unwrap() = Some(ProvisionDirective {
            max_tokens: Some(tokens),
            ..ProvisionDirective::default()
        });
    }

    /// Remember the output budget a truncation recovery succeeded at, so the
    /// next turn starts there instead of re-running a doomed capped ask. Keeps
    /// the maximum: a bigger successful ask supersedes a smaller one.
    pub(crate) fn learn_output_budget(&self, tokens: u64) {
        let mut current = self.learned_output_budget.load(Ordering::Relaxed);
        while tokens > current {
            match self.learned_output_budget.compare_exchange(
                current,
                tokens,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    eprintln!(
                        "[club:{}] learned output budget: {} tokens",
                        self.name, tokens
                    );
                    return;
                }
                Err(observed) => current = observed,
            }
        }
    }

    /// Learned defaults can grow; an operator ceiling always takes precedence.
    /// Provider-native requests remain uncapped.
    fn apply_learned_output_budget(
        &self,
        body: &mut serde_json::Value,
        budget: RequestOutputBudget,
    ) {
        match budget {
            RequestOutputBudget::OperatorLimit(tokens) => {
                body["max_tokens"] = serde_json::json!(tokens);
                return;
            }
            RequestOutputBudget::ProviderNative => return,
            RequestOutputBudget::InferredDefault(_) => {}
        }
        let learned = self.learned_output_budget.load(Ordering::Relaxed);
        if learned == 0 {
            return;
        }
        if let Some(current) = body.get("max_tokens").and_then(|v| v.as_u64())
            && learned > current
        {
            body["max_tokens"] = serde_json::json!(learned);
        }
    }

    /// Quick readiness probe: `GET {base_url}/models` returns 200 once the model
    /// is loaded. (turbo answers 503 "Loading model" while warming up.) Sends the
    /// key in case the server protects /models too. Uses a short fixed timeout so
    /// a probe never hangs the UI, regardless of the (longer) chat read timeout.
    pub fn is_ready(&self) -> bool {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let mut req = self.agent.get(&url).timeout(Duration::from_secs(2));
        let Ok(Some(key)) = self.bearer_token() else {
            // Unauthenticated probe is still valid for open local endpoints;
            // OAuth provider failure means the seat is not ready.
            if self.token_provider.is_some() {
                return false;
            }
            let Ok(resp) = req.call() else {
                return false;
            };
            if let Ok(v) = resp.into_json::<serde_json::Value>() {
                self.remember_reported_model(&v);
            }
            return true;
        };
        req = req.set("Authorization", &format!("Bearer {key}"));
        let Ok(resp) = req.call() else {
            return false;
        };
        if let Ok(v) = resp.into_json::<serde_json::Value>() {
            self.remember_reported_model(&v);
        }
        true
    }

    /// GET a JSON document from this club's backend with the short readiness
    /// timeout (never the long chat read timeout), carrying the API key. `None`
    /// on any transport/parse failure — callers treat that as "unknown".
    fn get_json(&self, url: &str) -> Option<serde_json::Value> {
        let mut req = self.agent.get(url).timeout(Duration::from_secs(2));
        if let Ok(Some(key)) = self.bearer_token() {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        req.call().ok()?.into_json().ok()
    }

    pub(crate) fn reported_model_id(v: &serde_json::Value) -> Option<String> {
        v.pointer("/data/0/id")
            .and_then(|x| x.as_str())
            .or_else(|| v.pointer("/models/0/name").and_then(|x| x.as_str()))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    pub(crate) fn remember_reported_model(&self, v: &serde_json::Value) -> Option<String> {
        let id = Self::reported_model_id(v)?;
        let mut changed = false;
        if let Ok(mut model) = self.model.lock() {
            let current = model.as_deref().unwrap_or("").trim();
            if current.is_empty() {
                *model = Some(id.clone());
                changed = true;
            } else if self.follow_backend && current != id {
                // The rig swapped checkpoints under us — adopt the new identity
                // and drop the cached metadata (window/caps described the old
                // model; the next metadata() call re-probes).
                *model = Some(id.clone());
                changed = true;
                if let Ok(mut m) = self.metadata.lock() {
                    *m = None;
                }
            }
        }
        if changed {
            self.route_state_revision.fetch_add(1, Ordering::Relaxed);
        }
        Some(id)
    }

    /// Model id for a request. Env/config wins; otherwise ask the backend's
    /// `/models` endpoint and cache the first reported id.
    fn model_id(&self) -> Result<String, String> {
        // Hold the model lock through the one cold `/models` request. This is a
        // deliberate one-time critical section: otherwise an N-way first wave
        // sees the empty cache N times and stampedes the control endpoint.
        let mut model = self
            .model
            .lock()
            .map_err(|_| "model id cache lock poisoned".to_string())?;
        if let Some(current) = model.as_ref().filter(|s| !s.trim().is_empty()) {
            return Ok(current.clone());
        }
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let v = self.get_json(&url).ok_or_else(|| {
            format!(
                "{} did not report a model id at /models; set ANGEL_{}_MODEL",
                self.base_url,
                self.name.to_ascii_uppercase().replace('-', "_")
            )
        })?;
        let learned = Self::reported_model_id(&v).ok_or_else(|| {
            format!(
                "{} /models did not include data[0].id or models[0].name",
                self.base_url
            )
        })?;
        *model = Some(learned.clone());
        Ok(learned)
    }

    /// Detect the model's real context window from the live backend. Shapes seen
    /// in the wild (validated against the live fleet):
    ///  - llama.cpp: `GET /props` → `default_generation_settings.n_ctx` (served at
    ///    the server root, not under `/v1`, so we strip a trailing `/v1` first).
    ///  - vLLM: `GET /v1/models` → `data[0].max_model_len`.
    ///  - llama.cpp `/v1/models`: `data[0].meta.n_ctx` (the *loaded* window, which
    ///    can be far below `n_ctx_train` — use the loaded one or requests overflow).
    ///
    /// `(None, …)` if none report (cloud endpoints, or a server that doesn't
    /// expose it); the second slot is the catalog's reasoning declaration when
    /// one exists (see [`catalog_capabilities`]).
    fn probe_backend_capabilities(&self, model: &str) -> (Option<usize>, Option<bool>) {
        let base = self.base_url.trim_end_matches('/');
        let root = base.strip_suffix("/v1").unwrap_or(base);
        // When base has no /v1 suffix, root == base and the two candidate URLs
        // are identical — probing the same dead endpoint twice doubles the stall.
        let mut urls = vec![format!("{root}/props")];
        if root != base {
            urls.push(format!("{base}/props"));
        }
        for url in urls {
            if let Some(v) = self.get_json(&url) {
                let n = v.get("n_ctx").and_then(|x| x.as_u64()).or_else(|| {
                    v.pointer("/default_generation_settings/n_ctx")
                        .and_then(|x| x.as_u64())
                });
                if let Some(n) = n
                    && n > 0
                {
                    // llama.cpp `/props` — a single-model server with no
                    // capability catalog; reasoning support stays unknown.
                    return (Some(n as usize), None);
                }
            }
        }
        if let Some(v) = self.get_json(&format!("{base}/models")) {
            return catalog_capabilities(&v, model);
        }
        (None, None)
    }

    /// Build the full [`Metadata`] for this club: probe the live window and
    /// capability declarations, fall back to a small static map for known cloud
    /// models. A `context_window` of `0` means genuinely unknown.
    fn probe_metadata(&self) -> Metadata {
        let model = self.model_id().unwrap_or_default();
        let model_l = model.to_ascii_lowercase();
        let (live, declared_reasoning) = self.probe_backend_capabilities(&model);
        let (static_ctx, static_cache) = static_model_window(&model_l).unwrap_or((0, false));
        let context_window = live.unwrap_or(static_ctx);
        // A backend that answered a live window probe (llama.cpp/vLLM) does
        // automatic prefix caching; cloud models in the static map declare it.
        let supports_cache = live.is_some() || static_cache;
        Metadata {
            context_window,
            supports_cache,
            // A catalog declaration is capability truth; the name folklore can
            // only *promote* to Some(true), never manufacture a "no".
            supports_reasoning: declared_reasoning.or_else(|| folklore_reasoning(&model_l)),
            supports_tools: true,
        }
    }

    /// Local truth for "this backend has a byte-exact automatic prefix cache":
    /// cached probe metadata first, then the static cloud-model map, then the
    /// DeepSeek family whose context cache is documented always-on (hits bill
    /// at a small fraction of a miss). Pure and network-free — callable from a
    /// request builder or a per-turn mode resolution without side effects.
    pub(crate) fn backend_prompt_cache_capable(&self) -> bool {
        if let Some((meta, _)) = self.metadata.lock().ok().and_then(|g| *g)
            && meta.supports_cache
        {
            return true;
        }
        let model_l = self.model_id().unwrap_or_default().to_ascii_lowercase();
        if let Some((_, cache)) = static_model_window(&model_l)
            && cache
        {
            return true;
        }
        // Documented/measured automatic prefix caches without a static-map
        // window entry (a wrong window guess would distort context budgets;
        // capability alone is safe). DeepSeek: documented always-on context
        // cache. GLM via z.ai/bigmodel: a 2026-08-10 paired probe against the
        // coding-plan endpoint reported `cached_tokens: 1920/1946` on the
        // second identical request, OpenAI-dialect accounting, and a usage
        // frame under `stream_options.include_usage`.
        let base_l = self.base_url.to_ascii_lowercase();
        model_l.contains("deepseek")
            || base_l.contains("deepseek.com")
            || model_l.contains("glm")
            || base_l.contains("z.ai")
            || base_l.contains("bigmodel")
            || model_l.contains("qwen")
            || base_l.contains("dashscope")
            || model_l.contains("kimi")
            || base_l.contains("moonshot")
            || base_l.contains("kimi.com")
    }

    /// Lazily detect and cache [`Metadata`]. Probes the backend at most once per
    /// process; a result (even an all-default one) is stored so a backend that
    /// can't report a window is never re-probed.
    fn ensure_metadata(&self) {
        // Re-probe after this TTL so a server relaunched with a different
        // `--ctx-size` is detected without restarting the cockpit. `0` = probe
        // once and never re-probe (the old behavior).
        let ttl = metadata_ttl();
        let is_fresh = || match self.metadata.lock() {
            Ok(g) => g
                .as_ref()
                .is_some_and(|(_, ts)| ttl.is_zero() || ts.elapsed() <= ttl),
            // Preserve the old poison behavior: don't perform network work when
            // the cache lock itself can no longer publish a result.
            Err(_) => true,
        };
        // Warm readers take only the short cache lock and never queue behind a
        // refresh already in progress.
        if is_fresh() {
            return;
        }
        // Serialize refreshers. The second cache check after acquiring the gate
        // lets followers consume the leader's newly published value.
        let Ok(_probe) = self.metadata_probe.lock() else {
            return;
        };
        let now = Instant::now();
        if is_fresh() {
            return;
        }
        let m = self.probe_metadata();
        if let Ok(mut g) = self.metadata.lock() {
            let old = g.as_ref().map(|(metadata, _)| *metadata);
            match *g {
                // A re-probe that came back "unknown" during a server swap must not
                // clobber a previously-good window — keep it, and retry next call.
                Some((old, _)) if m.context_window == 0 && old.context_window > 0 => {}
                _ => {
                    *g = Some((m, now));
                    if old != Some(m) {
                        self.route_state_revision.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    /// Whether this route is a DeepSeek V4 *provider* route: an exact current
    /// family id on a seat the operator wired through the configured DeepSeek
    /// namespace (`ANGEL_DEEPSEEK_*`, including an `ANGEL_DEEPSEEK_URL`
    /// forwarder that relays the real API) or on the provider's own host.
    ///
    /// Everything else — a local/fleet serve reusing a DeepSeek id, a club
    /// wearing the name on an unrelated endpoint — is *not* the provider, so its
    /// private reasoning stays withheld and it declares no cloud capability.
    fn is_deepseek_v4_provider_route(&self, model_l: &str) -> bool {
        is_deepseek_v4_model(model_l)
            && (self.provider_contract == Some(ProviderContract::DeepSeek)
                || self.official_deepseek_host())
    }

    /// The provider's own API host, parsed rather than substring-matched, so a
    /// URL carrying the host as userinfo or in its path cannot claim it.
    fn official_deepseek_host(&self) -> bool {
        url::Url::parse(&self.base_url).ok().is_some_and(|url| {
            url.host_str()
                .is_some_and(|host| host.eq_ignore_ascii_case("api.deepseek.com"))
        })
    }

    /// Build the chat-completions request body (model + messages + optional
    /// tools). `stream` toggles SSE token streaming.
    #[cfg(test)]
    pub(crate) fn build_body(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        stream: bool,
    ) -> Result<serde_json::Value, String> {
        self.build_body_with_effort(messages, tools, stream, None)
    }

    /// [`Self::build_body`] carrying a per-call effort request (a MoA seat's
    /// roster effort). Precedence: per-call > THINK-deck override > env.
    pub(crate) fn build_body_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        stream: bool,
        call_effort: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        self.build_body_and_budget(messages, tools, stream, call_effort)
            .map(|(body, _)| body)
    }

    fn build_body_and_budget(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        stream: bool,
        call_effort: Option<&str>,
    ) -> Result<(serde_json::Value, RequestOutputBudget), String> {
        use serde_json::json;
        let model = self.model_id()?;
        if let Some(error) = self
            .local_reasoning_profile
            .error(&model, self.reasoning_dialect())
        {
            return Err(error.to_string());
        }
        let mut outbound = messages_to_json(messages);
        // Combined 1:1 amendment of the outbound copy before the caveman
        // splice, while live messages still zip onto wire objects. Private
        // reasoning stays on the exact in-memory assistant tool-call
        // message for a DeepSeek V4 provider route only; generic
        // JSON/session/rollout serialization never sees it. Harness filler
        // whitespace is squeezed in the outbound copy only (operator text is
        // never touched).
        let replay_private_reasoning = self.is_deepseek_v4_provider_route(&model);
        let squeeze_harness_ws = self.caveman_candidate && econ_enabled();
        if replay_private_reasoning || squeeze_harness_ws {
            for (message, wire) in messages.iter().zip(outbound.iter_mut()) {
                if replay_private_reasoning
                    && message.role == ChatRole::Assistant
                    && !message.tool_calls.is_empty()
                    && let Some(reasoning) = &message.private_reasoning
                {
                    wire["reasoning_content"] = json!(reasoning.as_ref());
                }
                if squeeze_harness_ws
                    && message.role == ChatRole::Harness
                    && message.attachments.is_empty()
                    && let Some(squeezed) = econ_squeeze_ws(&message.content)
                {
                    wire["content"] = json!(squeezed);
                }
            }
        }
        // Brevity instruction for token-metered SOTA links: spliced into the
        // outbound JSON, so the history is never cloned to carry one message.
        if self.caveman_candidate
            && let Some((at, text)) = sota_caveman_insert(messages)
        {
            outbound.insert(at, json!({ "role": "system", "content": text }));
        }
        // Pre-provisioning (economizer + optimizer — `provision.rs`). A judge
        // directive staged by the chat entry wins per-field; deterministic
        // heuristics fill the gaps on metered links. Every splice is
        // tail-side so the pinned, cacheable prefix stays byte-identical.
        let mut prov = self
            .provision_directive
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .unwrap_or_default();
        let extraction_limit =
            if self.caveman_candidate && econ_enabled() && extraction_ask(messages) {
                econ_extract_budget()
            } else {
                None
            };
        if self.caveman_candidate && econ_enabled() && prov.contract.is_none() {
            prov.contract = econ_contract(messages);
        }
        // An explicit extraction limit outranks inferred judge provisioning.
        // It remains active when the output contract was already spliced.
        if let Some(tokens) = extraction_limit {
            prov.max_tokens = Some(tokens);
        }
        if let Some(text) = optimizer_dup(
            messages,
            // A resolved GLM/Qwen thinking dialect is stronger capability
            // truth than a catalog "no" (the effort gate's rule): never
            // duplicate at a backend the dialect says can reason.
            match self.reasoning_dialect() {
                ReasoningDialect::OpenAiEffort => self
                    .metadata
                    .lock()
                    .ok()
                    .and_then(|g| *g)
                    .and_then(|(m, _)| m.supports_reasoning),
                _ => Some(true),
            },
            self.caveman_candidate,
            !tools.is_empty(),
        ) {
            outbound.push(json!({ "role": "user", "content": text }));
        }
        if let Some(contract) = prov.contract.take() {
            outbound.push(json!({ "role": "system", "content": contract }));
        }
        // OpenRouter's Anthropic-compatible Chat Completions path supports
        // explicit per-content-block cache breakpoints across first-party,
        // Bedrock, and Vertex routes. Mark only a sufficiently large stable
        // system prefix for Claude-family models; local and non-Claude bodies
        // stay byte-identical. The first write can cost more, so operators may
        // disable it or raise the minimum for one-shot workloads.
        let model_l = model.to_ascii_lowercase();
        let openrouter_claude = self.base_url_lc.contains("openrouter.ai")
            && (model_l.contains("anthropic/") || model_l.contains("claude"));
        let (cache_enabled, cache_min_tokens, cache_ttl_1h) = openrouter_anthropic_cache_cfg();
        if openrouter_claude && cache_enabled {
            let cache_control = {
                let mut control = json!({"type":"ephemeral"});
                if cache_ttl_1h {
                    control["ttl"] = json!("1h");
                }
                control
            };
            let mut marked = false;
            // Total body size for the tail-marker floor, measured before any
            // marking rewrites string content into block arrays (an array
            // block would otherwise stop counting toward the total).
            let body_tokens: usize = outbound
                .iter()
                .map(|message| {
                    message
                        .get("content")
                        .and_then(|content| content.as_str())
                        .map_or(0, |content| content.chars().count() / 4)
                })
                .sum();
            if let Some(message) = outbound.iter_mut().find(|message| {
                message.get("role").and_then(|role| role.as_str()) == Some("system")
                    && message
                        .get("content")
                        .and_then(|content| content.as_str())
                        .is_some_and(|content| content.chars().count() / 4 >= cache_min_tokens)
            }) {
                let content = message["content"].as_str().unwrap_or_default().to_string();
                message["content"] = json!([{
                    "type": "text",
                    "text": content,
                    "cache_control": cache_control.clone(),
                }]);
                marked = true;
            }
            // Moving tail breakpoint: also mark the last plain-text message so
            // the growing conversation body caches, not just the fixed system
            // prefix. Anthropic checks earlier breakpoint positions on the next
            // request, so advancing this marker each hop turns the re-sent
            // conversation into a cache read plus a small incremental write —
            // with only the system marker, everything after the preamble was
            // re-billed at full price on every call. Tool-role messages are
            // skipped (their translation to tool_result blocks is the
            // gateway's business), and tiny one-shot bodies stay unmarked via
            // the same total-size floor the system marker uses.
            if body_tokens >= cache_min_tokens {
                // The find requires plain string content, so an
                // already-marked message (content rewritten to a block array)
                // can never be selected twice.
                if let Some(message) = outbound.iter_mut().rev().find(|message| {
                    matches!(
                        message.get("role").and_then(|role| role.as_str()),
                        Some("user") | Some("assistant") | Some("system")
                    ) && message
                        .get("content")
                        .and_then(|content| content.as_str())
                        .is_some_and(|content| !content.trim().is_empty())
                }) {
                    let content = message["content"].as_str().unwrap_or_default().to_string();
                    message["content"] = json!([{
                        "type": "text",
                        "text": content,
                        "cache_control": cache_control,
                    }]);
                    marked = true;
                }
            }
            if marked {
                self.cache_usage.add_control_request();
            }
        }
        let mut body = json!({
            "model": model,
            "messages": outbound,
            "stream": stream,
        });
        if !tools.is_empty() {
            // Tool schemas are byte-stable across most hops of a turn (activation
            // changes the fingerprint). Rebuilding the wire array every hop was
            // pure allocator churn on a multi-hop coding turn; cache by the same
            // fingerprint the hop ledger uses for prefix-breaker attribution.
            body["tools"] = tools_wire_json(tools);
            if crate::club::final_response_requested(messages) {
                body["tool_choice"] = json!("none");
            }
        }
        // Opt-in passthroughs — no-ops unless enabled, so default bodies stay
        // byte-identical and lenient servers (vLLM) see nothing new. Capability
        // flags from the detected metadata gate what we send, so a known
        // non-reasoning / non-caching backend never gets a field it might reject.
        let up = self.env_prefix();
        // Read *cached* metadata only — never probe here. Probing is a network call
        // and must not be a side effect of building a request body (it would add a
        // hop to every first chat and consume a single-connection test server). In
        // the live loop `compaction_budget` warms this cache before the first chat.
        let meta = self.metadata.lock().ok().and_then(|g| *g).map(|(m, _)| m);
        // Reasoning control, spelled in the club's resolved dialect. Precedence:
        // the per-call seat request wins, then the in-memory operator override
        // (THINK deck), then the env fallback — keeping the no-override body
        // byte-identical. Suppression requires *declared* truth — the backend's
        // own rejection of the field, or a catalog that declares no reasoning
        // controls (OpenAI spelling only; a resolved GLM/Qwen dialect IS
        // capability knowledge). Unknown always sends: a wrong guess here reads
        // as the control not existing, and a strict server teaches us instead
        // (see `learn_reasoning_rejection`). A withheld request moves the gate
        // counter so the turn can voice it — never a silent strip.
        // When the snapshot is armed, one load supplies dialect + gate +
        // override + env (the draw path already paid for it this frame).
        // Per-call seat effort still wins over the snapshot.
        let snap = effort_snapshot_enabled().then(|| self.effort_snapshot_load());
        let dialect = snap
            .as_ref()
            .map(|s| s.dialect)
            .unwrap_or_else(|| self.resolve_reasoning_dialect());
        let requested = call_effort
            .map(|s| s.to_string())
            .or_else(|| {
                if let Some(s) = snap.as_ref() {
                    s.override_effort.clone().or_else(|| s.env_effort.clone())
                } else {
                    self.effort_override
                        .lock()
                        .ok()
                        .and_then(|g| g.clone())
                        .or_else(|| self.effort_env_read())
                }
            })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let gate_reason = || {
            snap.as_ref()
                .and_then(|s| s.gate_reason.clone())
                .or_else(|| self.reasoning_gate_reason(dialect))
        };
        if let Some(effort) = requested {
            if let Some(reason) = gate_reason() {
                self.record_effort_withheld(&effort, &reason);
            } else {
                let off = effort.eq_ignore_ascii_case("none");
                match dialect {
                    ReasoningDialect::OpenAiEffort
                        if self.is_deepseek_v4_provider_route(&model_l) =>
                    {
                        // DeepSeek's current Chat Completions thinking contract:
                        // the toggle is `thinking: {type}` and depth is
                        // `reasoning_effort` (low/high/max). `none` is the
                        // documented disable toggle, not a rung — sending
                        // `reasoning_effort: "none"` is not a value the provider
                        // accepts.
                        if off {
                            body["thinking"] = json!({ "type": "disabled" });
                        } else {
                            body["thinking"] = json!({ "type": "enabled" });
                            body["reasoning_effort"] = json!(effort);
                        }
                    }
                    ReasoningDialect::OpenAiEffort => {
                        body["reasoning_effort"] = json!(effort);
                    }
                    ReasoningDialect::GlmThinking => {
                        let locked = glm_thinking_locked_on(&self.model_id().unwrap_or_default());
                        let mode = if off && !locked {
                            "disabled"
                        } else {
                            "enabled"
                        };
                        body["thinking"] = json!({ "type": mode });
                        body["clear_thinking"] = json!(true);
                        if locked {
                            body["reasoning_effort"] =
                                json!(if off { "low" } else { effort.as_str() });
                        }
                    }
                    ReasoningDialect::QwenEnableThinking => {
                        body["chat_template_kwargs"]["enable_thinking"] = json!(!off);
                    }
                }
            }
        } else if dialect == ReasoningDialect::GlmThinking {
            // Default OFF. Omitting the field lets Z.ai leave thinking open; that
            // has burned multi-hour hops with zero tools / zero submissions.
            // Operators who want deep think set ANGEL_GLM_REASONING_EFFORT=high
            // or the THINK chip. Escape: ANGEL_GLM_THINKING_DEFAULT=enabled.
            // GLM-5.3 / GLM-5.3-Flash reject `thinking.type: "disabled"`; keep
            // thinking on. glm-5.3 idles at `low`; Flash idles at `auto` (field
            // omitted) unless ANGEL_GLM_FLASH_IDLE_EFFORT pins a rung.
            let model = self.model_id().unwrap_or_default();
            let locked = glm_thinking_locked_on(&model);
            let default_on = glm_thinking_default_on();
            if locked {
                body["thinking"] = json!({ "type": "enabled" });
                body["clear_thinking"] = json!(true);
                // `None` (Flash `auto`) omits the field: the provider's adaptive
                // default decides depth per hop instead of a flat pin.
                if let Some(idle) = glm_locked_idle_effort(&model) {
                    body["reasoning_effort"] = json!(idle);
                }
            } else if !default_on {
                if let Some(reason) = gate_reason() {
                    self.record_effort_withheld("none", &reason);
                } else {
                    body["thinking"] = json!({ "type": "disabled" });
                    body["clear_thinking"] = json!(true);
                }
            }
        }
        // `prompt_cache_key` — an explicit key always wins; otherwise a stable
        // per-club-instance key is sent so prefix-caching servers (vLLM APC,
        // llama.cpp, OpenAI-compatible clouds) reuse the KV cache across this
        // club's turns without colliding with a concurrent Angel conversation.
        // The agent loop re-sends the whole history every hop and the swarm
        // re-sends the conversation to every role call, so cache reuse is the
        // single biggest per-call compute/cost saver. Default ON, but only for
        // backends *detected* cache-capable (a live window probe or the static
        // model map) — an unknown/strict backend keeps a byte-identical body.
        // `ANGEL_PROMPT_CACHE=0` disables; an explicit truthy value restores the
        // old opt-in semantics (send unless the backend is known non-caching).
        let explicit_key = prompt_cache_explicit_key();
        // Per-club pin, checked before the global gate: aggregators (OpenRouter)
        // never appear in the static model map, so detection alone leaves their
        // cache cold — `ANGEL_<CLUB>_PROMPT_CACHE=1` sends the key even without
        // detected metadata, `0` never sends it for this club, and unset keeps
        // the global capability-gated behavior byte-identical.
        // Seed-once process cache — hops no longer getenv the per-club pin.
        let club_pin = prompt_cache_club_pin(&self.prompt_cache_pin_env);
        let auto_cache = match club_pin {
            Some(pin) => pin,
            None => match prompt_cache_global_mode() {
                PromptCacheMode::Off => false,
                PromptCacheMode::ForceOn => meta.map(|m| m.supports_cache).unwrap_or(true),
                PromptCacheMode::Capability => meta.map(|m| m.supports_cache).unwrap_or(false),
            },
        };
        if let Some(key) =
            explicit_key.or_else(|| auto_cache.then(|| self.default_prompt_cache_key.clone()))
        {
            body["prompt_cache_key"] = json!(key);
        }
        // `stream_options.include_usage` — asks an OpenAI-compatible backend to
        // attach a usage frame (incl. cached-token counts) to the final stream
        // chunk. Some providers (DeepSeek, OpenRouter) volunteer usage; OpenAI
        // and most gateways stay silent unless asked, which leaves the cache
        // meter reading n/a and a cache-spend problem invisible. Default:
        // capability-gated — a backend with a detected/documented prefix cache
        // is asked (it demonstrably speaks this dialect), while an unknown or
        // strict server keeps a byte-identical body. The per-club
        // `ANGEL_<CLUB>_STREAM_USAGE` pin is checked first, then the global
        // `ANGEL_STREAM_USAGE`, either way explicitly overriding detection.
        if stream {
            // Seed-once process cache — hops no longer getenv the per-club pin.
            let usage_pin = stream_usage_club_pin(&self.stream_usage_pin_env);
            let armed = usage_pin.unwrap_or_else(|| {
                stream_usage_global().unwrap_or_else(|| self.backend_prompt_cache_capable())
            });
            if armed {
                body["stream_options"] = json!({ "include_usage": true });
            }
        }
        // `max_tokens` is opt-in. An operator env override wins; else a
        // judge/extraction budget (only ever paired with a spliced output
        // contract — see `provision.rs`); else provider-native.
        let budget =
            if let OutputBudgetPolicy::Explicit { tokens, .. } = self.output_budget_policy().0 {
                body["max_tokens"] = json!(tokens);
                RequestOutputBudget::OperatorLimit(tokens)
            } else if let Some(tokens) = prov.max_tokens {
                body["max_tokens"] = json!(tokens);
                if extraction_limit.is_some() {
                    RequestOutputBudget::OperatorLimit(tokens)
                } else {
                    RequestOutputBudget::InferredDefault(tokens)
                }
            } else {
                RequestOutputBudget::ProviderNative
            };
        // Stop sequences: judge verdict first, else operator env. Never
        // auto-derived — a guessed stop can amputate code or tool JSON.
        if prov.stop.is_empty() {
            prov.stop = econ_stop_seqs(up);
        }
        if !prov.stop.is_empty() {
            body["stop"] = json!(prov.stop);
        }
        self.apply_learned_output_budget(&mut body, budget);
        Ok((body, budget))
    }

    /// Record the rate-limit window from a successful response's headers, so the
    /// next call can back off proactively.
    fn record_rate_limit(&self, r: &ureq::Response) {
        if let Some((remaining, reset)) =
            parse_rate_limit_headers(|h| r.header(h).map(String::from))
            && let Ok(mut g) = self.rate_limit.lock()
        {
            *g = Some((remaining, Instant::now() + reset));
        }
    }

    /// If the last response left 0 requests in the window and it hasn't reset,
    /// sleep until it does (capped by `ANGEL_RATELIMIT_MAX_WAIT`, default 30s).
    /// Disabled with `ANGEL_RATELIMIT_PROACTIVE=0`.
    fn await_rate_limit(&self, cancel: Option<&AtomicBool>) {
        // Seed-once process cache — stream send no longer getenv's these knobs.
        let (proactive, cap) = ratelimit_send_knobs();
        if !proactive {
            return;
        }
        let wait = {
            match self.rate_limit.lock() {
                Ok(g) => match *g {
                    Some((0, reset_at)) => reset_at.checked_duration_since(Instant::now()),
                    _ => None,
                },
                Err(_) => None,
            }
        };
        if let Some(w) = wait {
            cancellable_sleep(w.min(cap), cancel);
        }
    }

    /// Remaining cooldown + provider message while the quota gate is armed.
    /// `None` once the cooldown lapses (the gate clears itself so the next call
    /// re-probes the provider — the reset may have happened early).
    fn quota_gate_remaining(&self) -> Option<(Duration, String)> {
        let mut g = self.quota_gate.lock().ok()?;
        match &*g {
            Some((until, msg)) => match until.checked_duration_since(Instant::now()) {
                Some(left) => Some((left, msg.clone())),
                None => {
                    *g = None;
                    None
                }
            },
            None => None,
        }
    }

    /// Arm the quota circuit breaker with the provider's own message (it usually
    /// names the reset time, e.g. "Your limit will reset at 2026-07-03 11:33:39").
    fn arm_quota_gate(&self, provider_msg: &str) {
        eprintln!(
            "[club:{}] quota exhausted — benched for {}s: {provider_msg}",
            self.name,
            quota_cooldown_duration().as_secs()
        );
        if let Ok(mut g) = self.quota_gate.lock() {
            *g = Some((
                Instant::now() + quota_cooldown_duration(),
                provider_msg.to_string(),
            ));
        }
    }

    /// The declared reason reasoning controls must stay off this club's wire,
    /// when one exists. `None` means send: the capability is affirmed or
    /// genuinely unknown — an unknown never gates. A backend rejection outranks
    /// dialect knowledge; a catalog "no" is honored only for the default OpenAI
    /// spelling, since a resolved GLM/Qwen dialect is itself capability truth.
    fn reasoning_gate_reason(&self, dialect: ReasoningDialect) -> Option<String> {
        if let Some(error) = self
            .local_reasoning_profile
            .error(&self.model_identity().unwrap_or_default(), dialect)
        {
            return Some(error.to_string());
        }
        if let Some(words) = self.reasoning_rejected.lock().ok().and_then(|g| g.clone()) {
            return Some(words);
        }
        if dialect != ReasoningDialect::OpenAiEffort {
            return None;
        }
        let meta = self.metadata.lock().ok().and_then(|g| *g).map(|(m, _)| m);
        if meta.and_then(|m| m.supports_reasoning) == Some(false) {
            return Some("the model catalog declares no reasoning controls".to_string());
        }
        None
    }

    /// Count one withheld effort and keep the operator-facing reason. One
    /// count per *new* fact: the same standing effort withheld on every later
    /// hop would re-notice every turn without adding information.
    fn record_effort_withheld(&self, effort: &str, reason: &str) {
        if let Ok(mut g) = self.effort_gate.lock() {
            let event = format!("{}: effort '{effort}' withheld — {reason}", self.name);
            if g.last.as_deref() != Some(event.as_str()) {
                g.withheld = g.withheld.saturating_add(1);
                g.last = Some(event);
            }
        }
    }

    /// Whether this request body carries a reasoning control in any dialect.
    fn body_carries_reasoning(body: &serde_json::Value) -> bool {
        body.get("reasoning_effort").is_some()
            || body.get("thinking").is_some()
            || body
                .pointer("/chat_template_kwargs/enable_thinking")
                .is_some()
    }

    /// Learn from a backend that rejected the reasoning control we sent: keep
    /// the server's own words, stop sending the field (sticky for this process
    /// — a metadata TTL re-probe can't launder the lesson away), and count it
    /// so the turn can voice what happened. Returns true when this call taught
    /// something new and the request deserves one immediate field-free retry.
    fn learn_reasoning_rejection(&self, error: &str, body_carried: bool) -> bool {
        if !body_carried || !is_reasoning_field_rejection(error, self.reasoning_dialect()) {
            return false;
        }
        {
            let Ok(mut g) = self.reasoning_rejected.lock() else {
                return false;
            };
            if g.is_some() {
                return false;
            }
            *g = Some(format!("the backend rejected it: {}", error.trim()));
        }
        // The control no longer exists on this club; keeping a THINK override
        // around would pretend otherwise on every later build.
        if let Ok(mut g) = self.effort_override.lock() {
            *g = None;
        }
        eprintln!(
            "[club:{}] backend rejected the reasoning field — learned, retrying without it: {}",
            self.name,
            error.trim()
        );
        if let Ok(mut g) = self.effort_gate.lock() {
            g.rejections = g.rejections.saturating_add(1);
            g.last = Some(format!(
                "{}: backend rejected reasoning controls — retried without them and stopped \
                 sending the field ({})",
                self.name,
                error.trim()
            ));
        }
        self.route_state_revision.fetch_add(1, Ordering::Relaxed);
        true
    }

    pub(crate) fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// Cache structured provider truth after an oversized request. This lets
    /// later turns pre-compact against the window that actually enforced the
    /// request instead of paying another 400 to relearn it.
    fn learn_context_window_from_error(&self, detail: &str) {
        let Some(context_window) = context_window_from_error_detail(detail) else {
            return;
        };
        if let Ok(mut cached) = self.metadata.lock() {
            let mut learned = cached
                .as_ref()
                .map(|(metadata, _)| *metadata)
                .unwrap_or(Metadata {
                    supports_tools: true,
                    ..Metadata::default()
                });
            let changed = learned.context_window != context_window;
            learned.context_window = context_window;
            *cached = Some((learned, (self.now)()));
            if changed {
                self.route_state_revision.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// POST the serialized body with retry/backoff and return the raw response on
    /// the first success — the caller decides whether to buffer it as JSON
    /// ([`Self::post_chat`]) or stream the body ([`Self::chat_streaming`]).
    ///
    /// Resilient to a flaky LAN: transient failures — a transport error (reset /
    /// refused / connect timeout) or a retryable HTTP status (429/503/5xx) — are
    /// retried with exponential backoff (honoring `Retry-After`), up to
    /// `policy.retries` times. A transient 429 uses the independent, longer
    /// rate-limit budget so bursty swarm seats can drain without multiplying the
    /// read-timeout budget for unrelated failures. Fatal statuses surface
    /// immediately rather than burning either retry budget. Retry covers
    /// *establishing* the response; a stream can't be replayed once its bytes
    /// have started flowing.
    fn send_with_retry(
        &self,
        bytes: &[u8],
        cancel: Option<&AtomicBool>,
        abort: Option<&ureq::AbortHandle>,
    ) -> Result<(ureq::Response, super::AccountingAttempt<'_>), String> {
        // Observation only: binding the run identity must never change the
        // request path. A body that is not JSON, or a capture that fails,
        // leaves the identity unbound (reported as such) and the request
        // proceeds exactly as before.
        if crate::harness::run_identity::current().is_none()
            && let Ok(wire) = serde_json::from_slice::<serde_json::Value>(bytes)
        {
            let _ = self.bind_wire_identity(&wire);
        }
        let url = self.chat_completions_url();
        let p = &self.policy;
        // Quota circuit breaker: while a weekly/monthly exhaustion is cooling
        // down, fail the call instantly (no network) with the provider's own
        // message — the swarm's failover keys off this error, and the user sees
        // when the window resets instead of a bare 429.
        if let Some((remaining, msg)) = self.quota_gate_remaining() {
            return Err(format!(
                "quota exhausted (cooling down, next probe in {}m): {msg}",
                remaining.as_secs().div_ceil(60).max(1)
            ));
        }
        // Proactive rate-limit backoff: if the last response said 0 requests
        // remained and the window hasn't reset yet, wait it out rather than fire a
        // request we already know will 429. Inert when the server sends no rate
        // headers (most local vLLM), or when ANGEL_RATELIMIT_PROACTIVE=0.
        self.await_rate_limit(cancel);
        let mut ordinary_attempt: u32 = 0;
        let mut rate_limit_attempt: u32 = 0;
        loop {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err("cancelled".to_string());
            }
            let mut req = self
                .agent
                .post(&url)
                .set("Content-Type", "application/json");
            if let Some(key) = self.bearer_token()? {
                req = req.set("Authorization", &format!("Bearer {key}"));
            }
            if let Some(abort) = abort {
                req = req.with_abort_handle(abort.clone());
            }
            // Fit output to the remaining shared allocation. Reservation below
            // remains atomic: concurrent callers cannot multiply this allowance.
            let mut requested_output = None;
            let fitted_bytes = if let Some(available) = crate::harness::formation_budget::current()
                .and_then(|budget| budget.available_tokens())
            {
                let mut wire: serde_json::Value = serde_json::from_slice(bytes)
                    .map_err(|e| format!("formation request is not JSON: {e}"))?;
                let key = if wire.get("max_completion_tokens").is_some() {
                    "max_completion_tokens"
                } else {
                    "max_tokens"
                };
                // Reserve extra framing room for inserting a previously absent cap.
                let room = available.saturating_sub((bytes.len() as u64).saturating_add(320));
                let requested = wire
                    .get(key)
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1024);
                requested_output = Some(requested);
                let share = crate::harness::formation_budget::current()
                    .and_then(|budget| budget.share_output(requested))
                    .unwrap_or(requested);
                wire[key] = serde_json::json!(
                    requested
                        .min(room.max(crate::harness::formation_budget::SHARE_FLOOR))
                        .min(share.max(1))
                        .max(1)
                );
                Some(serde_json::to_vec(&wire).map_err(|e| e.to_string())?)
            } else {
                None
            };
            let bytes = fitted_bytes.as_deref().unwrap_or(bytes);
            if let Some(remaining) = crate::harness::formation_budget::request_wall_remaining() {
                if remaining.is_zero() {
                    return Err("graph request deadline reached".into());
                }
                req = req.timeout(remaining);
            }
            let reservation = if let Some(budget) = crate::harness::formation_budget::current() {
                let wire: serde_json::Value = serde_json::from_slice(bytes)
                    .map_err(|e| format!("formation request is not JSON: {e}"))?;
                let max_output = wire
                    .get("max_tokens")
                    .or_else(|| wire.get("max_completion_tokens"))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                // Deliberately conservative byte estimate, including JSON/tool
                // framing. This is not a reported usage observation.
                let prompt = (bytes.len() as u64).saturating_add(256);
                let requested_output = requested_output.unwrap_or(max_output);
                let reservation = if requested_output == max_output {
                    budget.reserve(self.label(), prompt, max_output)?
                } else {
                    budget.reserve_fitted(self.label(), prompt, max_output, requested_output)?
                };
                Some(reservation)
            } else {
                None
            };
            let mut accounting = self.accounting.attempt();
            if let Some(reservation) = reservation {
                accounting.reserve_formation(reservation);
            }
            accounting.request_bytes(bytes.len());
            // Raw progress diagnostics must not overwrite the live cockpit.
            // Redirected/headless stderr still retains the timing trace.
            let grok_start = (self.base_url_lc.starts_with("https://api.x.ai/")
                && !std::io::IsTerminal::is_terminal(&std::io::stderr()))
            .then(Instant::now);
            if grok_start.is_some() {
                eprintln!(
                    "[grok] request-headers start 0 ms; connect/read/write bounds {}/{}/{} ms",
                    self.policy.connect_timeout.as_millis(),
                    self.policy.read_timeout.as_millis(),
                    self.policy.write_timeout.as_millis()
                );
            }
            let response = req.send_bytes(bytes);
            if let Some(started) = grok_start {
                eprintln!(
                    "[grok] request-headers {} {} ms",
                    if response.is_ok() {
                        "complete"
                    } else {
                        "failed"
                    },
                    started.elapsed().as_millis()
                );
            }
            match response {
                Ok(r) => {
                    self.record_rate_limit(&r);
                    return Ok((r, accounting));
                }
                Err(ureq::Error::Status(code, r)) => {
                    // Snapshot the header before the body read consumes the response.
                    let retry_hdr = r.header("retry-after").map(str::to_string);
                    let mut detail = String::new();
                    let _ = std::io::Read::read_to_string(
                        &mut accounting.response_reader(r.into_reader()),
                        &mut detail,
                    );
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&detail) {
                        accounting.observe(
                            value
                                .get("usage")
                                .and_then(super::usage::parse_http_usage)
                                .map(|usage| self.accounting_observation(usage)),
                        );
                        self.record_usage_json(&value);
                    }
                    drop(accounting); // Close the actual send before any retry sleep.
                    self.learn_context_window_from_error(&detail);
                    // A weekly/monthly quota exhaustion won't clear on retry, so
                    // surface it immediately rather than sleeping through the whole
                    // retry budget on a provider that's down until its window resets.
                    // Arm the circuit breaker so every call until the cooldown
                    // expires fails instantly instead of re-poking a dead window.
                    if code == 429 && is_quota_exhausted(&detail) {
                        self.arm_quota_gate(detail.trim());
                        return Err(crate::secrets::redact_error(&format!(
                            "HTTP {code}: {}",
                            detail.trim()
                        )));
                    }
                    let (attempt, retries, backoff_base, backoff_cap) = if code == 429 {
                        (
                            rate_limit_attempt,
                            p.rate_limit_retries,
                            p.rate_limit_backoff_base,
                            p.rate_limit_backoff_cap,
                        )
                    } else {
                        (ordinary_attempt, p.retries, p.backoff_base, p.backoff_cap)
                    };
                    if is_retryable_status(code) && attempt < retries {
                        let wait =
                            retry_after(retry_hdr.as_deref(), backoff_cap).unwrap_or_else(|| {
                                jittered(
                                    backoff_delay(backoff_base, backoff_cap, attempt),
                                    p.jitter,
                                )
                            });
                        if code == 429 {
                            rate_limit_attempt += 1;
                        } else {
                            ordinary_attempt += 1;
                        }
                        cancellable_sleep(wait, cancel);
                        continue;
                    }
                    return Err(crate::secrets::redact_error(&format!(
                        "HTTP {code}: {}",
                        detail.trim()
                    )));
                }
                Err(e) => {
                    drop(accounting);
                    // Transport-level: connection refused/reset, connect timeout, DNS.
                    // Exactly the slow-LAN blips worth riding out.
                    if ordinary_attempt < p.retries {
                        let wait = jittered(
                            backoff_delay(p.backoff_base, p.backoff_cap, ordinary_attempt),
                            p.jitter,
                        );
                        ordinary_attempt += 1;
                        cancellable_sleep(wait, cancel);
                        continue;
                    }
                    return Err(crate::secrets::redact_error(&format!(
                        "transport error after {} attempt(s): {e}",
                        ordinary_attempt + 1
                    )));
                }
            }
        }
    }

    /// POST a chat body and return the parsed JSON response (non-streaming).
    fn post_chat(&self, body: serde_json::Value) -> Result<serde_json::Value, String> {
        let bytes = self.encode_request_body(body)?;
        let (r, mut accounting) = self.send_with_retry(&bytes, None, None)?;
        let v: serde_json::Value =
            serde_json::from_reader(accounting.response_reader(r.into_reader()))
                .map_err(|e| format!("decode response: {e}"))?;
        accounting.observe(
            v.get("usage")
                .and_then(super::usage::parse_http_usage)
                .map(|usage| self.accounting_observation(usage)),
        );
        self.record_usage_json(&v);
        // A 200 + {"error":…} envelope is a real failure, not a blank reply.
        if let Some(msg) = extract_api_error(&v) {
            return Err(format!("api error: {msg}"));
        }
        Ok(v)
    }

    fn encode_request_body(&self, body: serde_json::Value) -> Result<Vec<u8>, String> {
        let model = body
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        // Reuse a thread-local buffer so multi-hop turns don't re-allocate the
        // multi-KB request scratch on every encode. pxpipe still takes ownership.
        let bytes = encode_json_bytes(&body).map_err(|e| format!("encode request: {e}"))?;
        if !self.pxpipe_candidate {
            return Ok(bytes);
        }
        maybe_pxpipe_transform(PxpipeApi::ChatCompletions, &self.name, &model, bytes)
    }

    /// Test-only hook: inject cached metadata WITHOUT a network probe, so the
    /// reasoning-controls tests can exercise the `!supports_reasoning` path on
    /// a club pointed at a non-responsive host.
    #[cfg(test)]
    pub(crate) fn inject_metadata_for_tests(&self, metadata: Metadata) {
        if let Ok(mut g) = self.metadata.lock() {
            let changed = g.as_ref().map(|(old, _)| *old) != Some(metadata);
            *g = Some((metadata, std::time::Instant::now()));
            if changed {
                self.route_state_revision.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

fn context_window_from_error_detail(detail: &str) -> Option<usize> {
    let body: serde_json::Value = serde_json::from_str(detail).ok()?;
    let error = body.get("error").unwrap_or(&body);
    let error_type = error
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let typed_overflow = error_type.contains("context") && error_type.contains("exceed");
    let described_overflow = message.contains("context")
        && (message.contains("exceed") || message.contains("too large"));
    if !typed_overflow && !described_overflow {
        return None;
    }
    error
        .get("n_ctx")
        .and_then(serde_json::Value::as_u64)
        .and_then(|window| usize::try_from(window).ok())
        .filter(|window| *window > 0)
}

impl Club for HttpClub {
    fn supports_formation_budget(&self) -> bool {
        true
    }
    fn bind_run_identity(&self, _effort: Option<&str>) -> Result<(), String> {
        Ok(())
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        match self.chat(&[ChatMsg::user(prompt)], &[])? {
            ClubReply::Text(t) => Ok(t),
            ClubReply::Calls(_) => Err("club requested a tool with no tools offered".to_string()),
        }
    }

    fn label(&self) -> &str {
        &self.name
    }

    fn env_namespace(&self) -> Option<&str> {
        Some(&self.env_prefix)
    }

    fn resolved_model_defaults(&self) -> serde_json::Value {
        let mut budgets = model_defaults::budgets(
            &self.model_identity().unwrap_or_default(),
            self.env_prefix(),
        );
        if let Some(effort) = self.effort_override.lock().ok().and_then(|g| g.clone()) {
            budgets["reasoning_effort"] = serde_json::json!(effort);
            budgets["reasoning_effort_source"] = serde_json::json!("env");
        }
        budgets
    }

    /// Reachable iff the OpenAI-compatible endpoint answers `GET /models`. Bounded
    /// by `is_ready`'s short fixed timeout, so the bag prober never stalls on a
    /// dead box. A quota-exhausted link reports unavailable without a probe: the
    /// endpoint would answer `/models` fine, but every chat is a guaranteed 429
    /// until its window resets, so for routing purposes it's down.
    fn is_available(&self) -> bool {
        if self.quota_gate_remaining().is_some() {
            return false;
        }
        self.is_ready()
    }

    fn warm_up(&self) {
        // Probe window/capability once so the first paid hop does not pay
        // cold `/models` + TLS setup on the operator's Enter key.
        self.ensure_metadata();
    }

    fn quota_cooldown(&self) -> Option<Duration> {
        self.quota_gate_remaining().map(|(left, _)| left)
    }

    /// The live checkpoint id, once learned — only for clubs that follow their
    /// backend (fleet endpoints); env-pinned and catalog clubs return `None`
    /// so their configured alias/label stays authoritative.
    fn live_model_name(&self) -> Option<String> {
        if !self.follow_backend {
            return None;
        }
        self.model
            .lock()
            .ok()
            .and_then(|g| g.as_ref().filter(|s| !s.trim().is_empty()).cloned())
    }

    fn model_identity(&self) -> Option<String> {
        self.model
            .lock()
            .ok()
            .and_then(|model| model.as_ref().filter(|id| !id.trim().is_empty()).cloned())
    }

    /// HTTP routes expose their configured process-level hint as read-only. The
    /// picker deliberately cannot mutate it because a global env write could
    /// change another club or a multi-hop turn already in flight.
    fn reasoning_effort(&self) -> Option<String> {
        if effort_snapshot_enabled() {
            let snap = self.effort_snapshot_load();
            // Only *declared* incapability hides the effort.
            if snap.gate_reason.is_some() {
                return None;
            }
            return snap.override_effort.or(snap.env_effort).or_else(|| {
                (snap.dialect == ReasoningDialect::GlmThinking)
                    .then(|| self.glm_flash_idle_effort())
                    .flatten()
            });
        }
        // Only *declared* incapability hides the effort: a backend rejection or
        // a catalog "no" (OpenAI spelling; a GLM/Qwen dialect is itself truth).
        if self
            .reasoning_gate_reason(self.resolve_reasoning_dialect())
            .is_some()
        {
            return None;
        }
        // The operator's THINK-deck override wins over the env fallback.
        if let Some(over) = self.effort_override.lock().ok().and_then(|g| g.clone()) {
            return Some(over);
        }
        // Process-lifetime env cache: this runs on the draw path every frame.
        self.effort_env_read().or_else(|| {
            (self.resolve_reasoning_dialect() == ReasoningDialect::GlmThinking)
                .then(|| self.glm_flash_idle_effort())
                .flatten()
        })
    }

    /// The THINK ladder the agent panel offers for this club, in the club's
    /// resolved dialect. Empty only on *declared* incapability — a backend that
    /// rejected the field, or a catalog "no" under the default OpenAI spelling.
    /// Unknown capability offers the ladder: the deck lights up, and a strict
    /// server corrects a wrong guess on the first turn. A cached read only
    /// (mirrors [`reasoning_effort`](Club::reasoning_effort)), never a probe.
    fn reasoning_levels(&self) -> &[String] {
        static OPENAI_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        static GROK_46_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        static DEEPSEEK_V4_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        static GLM_BINARY_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        static GLM_53_LOCKED_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        static QWEN_THINKING_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        static LOCAL_DEEPSEEK_LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

        let (dialect, gated) = if effort_snapshot_enabled() {
            let snap = self.effort_snapshot_load();
            (snap.dialect, snap.gate_reason.is_some())
        } else {
            let dialect = self.resolve_reasoning_dialect();
            (dialect, self.reasoning_gate_reason(dialect).is_some())
        };
        if gated {
            return &[];
        }
        if let Some(levels) = self.local_reasoning_profile.levels() {
            return LOCAL_DEEPSEEK_LEVELS
                .get_or_init(|| levels.iter().map(|level| (*level).to_string()).collect());
        }
        let model_l = self.model_id().unwrap_or_default().to_ascii_lowercase();
        match dialect {
            ReasoningDialect::OpenAiEffort if model_l == "grok-4.6" => GROK_46_LEVELS
                .get_or_init(|| vec!["low".into(), "medium".into(), "high".into(), "xhigh".into()]),
            ReasoningDialect::OpenAiEffort if is_deepseek_v4_model(&model_l) => DEEPSEEK_V4_LEVELS
                .get_or_init(|| {
                    DEEPSEEK_V4_HTTP_EFFORT_LEVELS
                        .iter()
                        .map(|level| (*level).to_string())
                        .collect()
                }),
            ReasoningDialect::OpenAiEffort => OPENAI_LEVELS
                .get_or_init(|| vec!["none".into(), "low".into(), "medium".into(), "high".into()]),
            ReasoningDialect::GlmThinking if glm_thinking_locked_on(&model_l) => {
                GLM_53_LOCKED_LEVELS.get_or_init(|| {
                    GLM_53_LOCKED_HTTP_EFFORT_LEVELS
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                })
            }
            ReasoningDialect::GlmThinking => {
                GLM_BINARY_LEVELS.get_or_init(|| vec!["none".into(), "high".into()])
            }
            ReasoningDialect::QwenEnableThinking => {
                QWEN_THINKING_LEVELS.get_or_init(|| vec!["none".into(), "high".into()])
            }
        }
    }

    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let (dialect, gated) = if effort_snapshot_enabled() {
            let snap = self.effort_snapshot_load();
            (snap.dialect, snap.gate_reason.is_some())
        } else {
            let dialect = self.resolve_reasoning_dialect();
            (dialect, self.reasoning_gate_reason(dialect).is_some())
        };
        if gated {
            return None;
        }
        let model_l = self.model_id().unwrap_or_default().to_ascii_lowercase();
        let canonical = self
            .local_reasoning_profile
            .levels()
            .unwrap_or_else(|| dialect_effort_levels(dialect, &model_l))
            .iter()
            .copied()
            .find(|level| level.eq_ignore_ascii_case(requested.trim()))?
            .to_string();
        if let Ok(mut g) = self.effort_override.lock() {
            *g = Some(canonical.clone());
        }
        self.route_state_revision.fetch_add(1, Ordering::Relaxed);
        Some(canonical)
    }

    fn effort_gate_usage(&self) -> EffortGateUsage {
        self.effort_gate
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    fn route_metadata(&self) -> RouteMetadata {
        let (output_budget, output_budget_provenance) = self.output_budget_policy();
        // Declared capability is the *provider's* contract for this route, so it
        // requires a route we can identify as the provider's plus an exact family
        // id. A local/fleet serve reusing a DeepSeek id declares nothing and
        // keeps its own text-only identity, and a club that merely looks
        // DeepSeek by name never inherits the cloud model's capabilities.
        let model_l = self
            .model
            .lock()
            .ok()
            .and_then(|model| model.as_ref().map(|id| id.to_ascii_lowercase()))
            .unwrap_or_default();
        let input_modalities = if self.is_deepseek_v4_provider_route(&model_l) {
            static_model_input_modalities(&model_l)
                .map(|modalities| modalities.iter().map(|m| (*m).to_string()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        RouteMetadata {
            output_budget,
            output_budget_provenance,
            input_modalities,
            ..RouteMetadata::default()
        }
    }

    fn header_route_metadata(&self) -> RouteMetadata {
        // Same as an empty `route_metadata()` window/speed view, without the
        // max-tokens env-cache lock the draw path used to pay every frame.
        RouteMetadata::default()
    }

    fn route_state_revision(&self) -> u64 {
        // The frame path must stay lock- and allocation-free. Product environment
        // controls are startup configuration; live model, capability, and effort
        // mutations increment this revision at their publication sites.
        self.route_state_revision.load(Ordering::Relaxed)
    }

    fn metadata(&self) -> Option<Metadata> {
        self.ensure_metadata();
        self.metadata.lock().ok().and_then(|g| *g).map(|(m, _)| m)
    }

    fn metadata_cached(&self) -> Option<Metadata> {
        self.metadata.lock().ok().and_then(|g| *g).map(|(m, _)| m)
    }

    fn usage_accounting(&self) -> super::AccountingView {
        self.accounting.view()
    }

    fn token_usage(&self) -> Option<TokenUsage> {
        // Lock-free: the Agent bay meter asks this every frame.
        let stats = self.usage.load();
        (stats.turns > 0).then_some(stats)
    }

    fn cache_usage(&self) -> CacheUsage {
        self.cache_usage.load()
    }

    fn prompt_cache_capable(&self) -> bool {
        self.backend_prompt_cache_capable()
    }

    fn truncation_usage(&self) -> TruncationUsage {
        self.truncation
            .lock()
            .map(|stats| *stats)
            .unwrap_or_default()
    }

    fn chat(&self, messages: &[ChatMsg], tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.chat_with_effort(messages, tools, None)
    }

    fn chat_with_effort(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolDef],
        effort: Option<&str>,
    ) -> Result<ClubReply, String> {
        set_pending_tool_reasoning(None);
        // Judge consult for metered, tool-less asks (fail-open, cached,
        // latency-bounded — see `provision.rs`). Staged, then consumed by the
        // body build below.
        if self.caveman_candidate
            && tools.is_empty()
            && let Some(d) = judge_directive(&self.name, messages)
            && let Ok(mut g) = self.provision_directive.lock()
        {
            *g = Some(d);
        }
        let (body, budget) = self.build_body_and_budget(messages, tools, false, effort)?;
        let sent = body.get("max_tokens").and_then(|v| v.as_u64());
        let carried_reasoning = Self::body_carries_reasoning(&body);
        let first = self.chat_body(body, tools);
        match &first {
            Err(e) if e.as_str() == TRUNCATED_OUTPUT_ERR => {}
            // The model produced reasoning but no answer (raw ` ` stream or a
            // `reasoning_content` field with empty content). One bounded,
            // history-preserving recovery: re-ask with a direct-answer
            // reminder. A second reasoning-only reply fails closed with the
            // original error — no recursion, no unbounded resend.
            Err(e) if e.as_str() == EMPTY_REPLY_REASONING_ONLY_ERR => {
                if !reasoning_only_retry_enabled() {
                    return first;
                }
                let Ok(mut retry) = self.build_body_with_effort(messages, tools, false, effort)
                else {
                    return first;
                };
                let reminder = if tools.is_empty() {
                    ANSWER_DIRECTLY_REMINDER
                } else {
                    EMIT_TOOL_CALL_REMINDER
                };
                inject_stream_reminder(&mut retry, reminder);
                eprintln!(
                    "[club:{}] reply was reasoning-only — retrying with a {} reminder",
                    self.name,
                    if tools.is_empty() {
                        "direct-answer"
                    } else {
                        "emit-tool-call"
                    }
                );
                return self.chat_body(retry, tools);
            }
            // A strict backend rejecting the reasoning field is capability
            // truth, not a model failure: learn it once, then re-enter with the
            // full ladder intact — the learned gate guarantees the rebuilt body
            // can't re-offend, so this recursion is depth-one.
            Err(e) if self.learn_reasoning_rejection(e, carried_reasoning) => {
                return self.chat_with_effort(messages, tools, effort);
            }
            _ => return first,
        }
        // Truncation recovery: only unusable output (typically a half-written tool
        // call) reaches this seam. Only inferred defaults gain output room,
        // bounded by the existing resend count/maximum. Explicit operator caps
        // and provider-native limits are never enlarged.
        let schedule = budget.retry_schedule(sent);
        for (retry_index, escalated) in schedule.iter().copied().enumerate() {
            eprintln!(
                "[club:{}] output truncated at max_tokens={} — retry {}/{} at {escalated}",
                self.name,
                sent.unwrap_or(0),
                retry_index + 1,
                schedule.len()
            );
            let Ok(mut retry) = self.build_body_with_effort(messages, tools, false, effort) else {
                self.record_truncation(retry_index, false);
                return Err(self.truncation_policy(sent).incomplete_message());
            };
            retry["max_tokens"] = serde_json::json!(escalated);
            match self.chat_body(retry, tools) {
                Ok(reply) => {
                    self.record_truncation(retry_index + 1, true);
                    self.learn_output_budget(escalated);
                    return Ok(reply);
                }
                Err(error) if error == TRUNCATED_OUTPUT_ERR => continue,
                // A transport/auth/config failure is not output-cap evidence.
                // Preserve the original deterministic truncation error and stop.
                Err(_) => {
                    self.record_truncation(retry_index + 1, false);
                    return Err(self.truncation_policy(sent).incomplete_message());
                }
            }
        }
        self.record_truncation(schedule.len(), false);
        Err(self.truncation_policy(sent).incomplete_message())
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
        set_pending_tool_reasoning(None);
        if self.caveman_candidate
            && tools.is_empty()
            && let Some(d) = judge_directive(&self.name, messages)
            && let Ok(mut g) = self.provision_directive.lock()
        {
            *g = Some(d);
        }
        let (body, budget) = self.build_body_and_budget(messages, tools, true, effort)?;
        let sent = body.get("max_tokens").and_then(|v| v.as_u64());
        let carried_reasoning = Self::body_carries_reasoning(&body);
        // Track whether any visible content reached the caller: an already-emitted
        // stream can't be replayed without duplicating output, so the retry below
        // only fires on a stream that produced nothing — the same rule
        // [`FallbackClub`] applies before routing around a failure.
        let mut emitted = false;
        let mut wrapped = |d: StreamDelta| {
            if matches!(d, StreamDelta::Content(_)) {
                emitted = true;
            }
            on_delta(d);
        };
        let first = self.stream_body(body, tools, cancel, &mut wrapped);
        match &first {
            Err(e) if e.as_str() == TRUNCATED_OUTPUT_ERR && !emitted => {}
            Err(e) if e.as_str() == TRUNCATED_OUTPUT_ERR => {
                self.record_truncation(0, false);
                return Err(self.truncation_policy(sent).incomplete_message());
            }
            // Reasoning-only reply (private thinking, no visible answer): one
            // bounded re-ask with a direct-answer reminder. No visible content
            // was emitted by definition, so replaying is safe. A second
            // reasoning-only stream fails closed with its error — no recursion.
            Err(e) if !emitted && e.as_str() == EMPTY_REPLY_REASONING_ONLY_ERR => {
                if !reasoning_only_retry_enabled() {
                    return first;
                }
                let Ok(mut retry) = self.build_body_with_effort(messages, tools, true, effort)
                else {
                    return first;
                };
                let reminder = if tools.is_empty() {
                    ANSWER_DIRECTLY_REMINDER
                } else {
                    EMIT_TOOL_CALL_REMINDER
                };
                inject_stream_reminder(&mut retry, reminder);
                eprintln!(
                    "[club:{}] stream was reasoning-only — retrying with a {} reminder",
                    self.name,
                    if tools.is_empty() {
                        "direct-answer"
                    } else {
                        "emit-tool-call"
                    }
                );
                let mut retry_wrapped = |delta: StreamDelta| on_delta(delta);
                return self.stream_body(retry, tools, cancel, &mut retry_wrapped);
            }
            // A rejection arrives before any SSE byte, so `!emitted` holds and
            // a re-entry duplicates nothing. Learn once; the rebuilt body drops
            // the field, so the recursion is depth-one.
            Err(e) if !emitted && self.learn_reasoning_rejection(e, carried_reasoning) => {
                return self.chat_streaming_with_effort(messages, tools, effort, cancel, on_delta);
            }
            _ => return first,
        }
        let schedule = budget.retry_schedule(sent);
        for (retry_index, escalated) in schedule.iter().copied().enumerate() {
            eprintln!(
                "[club:{}] output truncated at max_tokens={} — streaming retry {}/{} at {escalated}",
                self.name,
                sent.unwrap_or(0),
                retry_index + 1,
                schedule.len()
            );
            let Ok(mut retry) = self.build_body_with_effort(messages, tools, true, effort) else {
                self.record_truncation(retry_index, false);
                return Err(self.truncation_policy(sent).incomplete_message());
            };
            retry["max_tokens"] = serde_json::json!(escalated);
            let mut retry_emitted = false;
            let retry_result = {
                let mut wrapped_retry = |delta: StreamDelta| {
                    if matches!(delta, StreamDelta::Content(_)) {
                        retry_emitted = true;
                    }
                    on_delta(delta);
                };
                self.stream_body(retry, tools, cancel, &mut wrapped_retry)
            };
            match retry_result {
                Ok(reply) => {
                    self.record_truncation(retry_index + 1, true);
                    self.learn_output_budget(escalated);
                    return Ok(reply);
                }
                Err(error) if error == TRUNCATED_OUTPUT_ERR && !retry_emitted => continue,
                // Never replay after any visible retry output; doing so would
                // duplicate a partial answer in the live transcript.
                Err(_) => {
                    self.record_truncation(retry_index + 1, false);
                    return Err(self.truncation_policy(sent).incomplete_message());
                }
            }
        }
        self.record_truncation(schedule.len(), false);
        Err(self.truncation_policy(sent).incomplete_message())
    }
}

impl HttpClub {
    /// One buffered chat round: POST an already-built body and decode the reply.
    /// Observe only the final serialized controls, never prompt or credential fields.
    pub(crate) fn bind_wire_identity(&self, body: &serde_json::Value) -> Result<(), String> {
        crate::harness::run_identity::prepare_model_defaults(self.resolved_model_defaults());
        crate::harness::run_identity::bind(
            crate::harness::run_identity::Model {
                club: self.label().into(),
                id: body["model"].as_str().unwrap_or("unbound").into(),
                base_url: crate::harness::run_identity::endpoint_identity(&self.base_url),
                driver: self.label().into(),
            },
            crate::harness::run_identity::wire_effort(body),
            body.get("max_tokens")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("provider-native")),
            None,
        )
    }

    /// Split from [`Club::chat`] (whose body this was, verbatim) so the truncation
    /// retry can re-send an escalated body while an untruncated request stays
    /// byte-identical to before.
    fn chat_body(&self, body: serde_json::Value, tools: &[ToolDef]) -> Result<ClubReply, String> {
        let sent = body.get("max_tokens").and_then(|value| value.as_u64());
        let replay_private_reasoning = body
            .get("model")
            .and_then(|model| model.as_str())
            .is_some_and(|model| self.is_deepseek_v4_provider_route(model));
        let v = self.post_chat(body)?;
        // Validate the response shape rather than silently treating a malformed
        // or empty body as a blank reply (which shows up as Angel saying nothing).
        let choice = match v["choices"].as_array().and_then(|c| c.first()) {
            Some(choice) => choice,
            None => {
                return Err(format!(
                    "club returned no choices — {}",
                    truncate_json(&v, 200)
                ));
            }
        };
        // A cut-off reply is only safe to keep when it's plain prose: a truncated
        // tool call (structured or prose-wrapped) has half-written args, so those
        // paths always fail closed. Prose is kept for `keep_truncated` links and
        // errors otherwise — computed here, acted on after we know the shape.
        let truncated = choice.get("finish_reason").and_then(|v| v.as_str()) == Some("length");
        let msg = &choice["message"];

        if let Some(tcs) = msg["tool_calls"].as_array() {
            let calls: Vec<ToolCall> = tcs
                .iter()
                .enumerate()
                .filter_map(|(idx, tc)| {
                    let id = tc["id"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("call_{idx}"));
                    let name = tc["function"]["name"]
                        .as_str()
                        .or_else(|| tc["name"].as_str())?
                        .to_string();
                    let raw_val = if !tc["function"]["arguments"].is_null() {
                        &tc["function"]["arguments"]
                    } else {
                        &tc["arguments"]
                    };
                    let raw = if let Some(s) = raw_val.as_str() {
                        s.to_string()
                    } else if raw_val.is_object() || raw_val.is_array() {
                        raw_val.to_string()
                    } else {
                        "{}".to_string()
                    };
                    let args = parsed_tool_args(&id, &raw);
                    Some(ToolCall { id, name, args })
                })
                .collect();
            if !calls.is_empty() {
                if truncated {
                    return Err(TRUNCATED_OUTPUT_ERR.to_string());
                }
                let reasoning = response_reasoning(msg)
                    .filter(|reasoning| !reasoning.is_empty())
                    .filter(|reasoning| reasoning.len() <= TOOL_REASONING_RECEIPT_MAX_BYTES)
                    .map(str::to_string);
                set_pending_tool_reasoning(replay_private_reasoning.then_some(reasoning).flatten());
                return Ok(ClubReply::Calls(calls));
            }
        }

        // No structured tool calls — but recover one the model wrote into the text
        // (explicit wrappers only) before treating the content as a final answer.
        // Only when tools were actually offered: a tool-less request (swarm/deli
        // workers) can't execute anything, so wrapper-looking text stays prose.
        let raw_content = msg["content"].as_str().unwrap_or("");
        // Local R1/Qwen3 distills can carry the whole ` ` marker dialect
        // inside `content` (no protocol `reasoning_content`). Split it so the
        // answer is clean and chain-of-thought never becomes reply text or
        // persisted history. The reasoning half is private and, matching how
        // protocol reasoning on plain-text replies is treated, is not
        // surfaced here. Kill-switch: ANGEL_REASONING_MARKERS=0.
        let mut marker_reasoning = String::new();
        let content: std::borrow::Cow<'_, str> =
            if response_reasoning(msg).is_some() || !marker_splitting_enabled() {
                std::borrow::Cow::Borrowed(raw_content)
            } else {
                let sp = split_marker_text(raw_content);
                marker_reasoning = sp.reasoning;
                std::borrow::Cow::Owned(sp.content)
            };
        if !tools.is_empty() {
            let recovered = extract_prose_tool_calls(&content);
            if !recovered.is_empty() {
                if truncated {
                    return Err(TRUNCATED_OUTPUT_ERR.to_string());
                }
                return Ok(ClubReply::Calls(recovered));
            }
        }
        // No tool calls: expect text. An empty answer here means the turn would
        // render as a blank reply, so surface it as an error the user can see
        // (and the harness can retry) instead.
        if content.trim().is_empty() {
            let protocol_reasoning = response_reasoning(msg).is_some_and(|r| !r.trim().is_empty());
            return Err(if truncated {
                TRUNCATED_OUTPUT_ERR.to_string()
            } else if !marker_reasoning.is_empty() || protocol_reasoning {
                // Reasoning-only reply: the caller makes one bounded
                // direct-answer retry before failing closed.
                EMPTY_REPLY_REASONING_ONLY_ERR.to_string()
            } else {
                "club returned an empty reply (no text and no tool calls)".to_string()
            });
        }
        // Cut off at the token cap but we have usable prose: a fail-closed club
        // surfaces the error (the caller retries / shrinks the request); a SOTA
        // link keeps the partial draft as material, marked so it's never mistaken
        // for a complete answer.
        if truncated && !self.keep_truncated {
            return Err(TRUNCATED_OUTPUT_ERR.to_string());
        }
        if truncated {
            self.record_retained_truncation();
            return Ok(ClubReply::Text(mark_truncated(
                &content,
                self.truncation_policy(sent),
            )));
        }
        Ok(ClubReply::Text(content.into_owned()))
    }

    /// One streaming chat round: POST an already-built body and forward deltas.
    /// Split from [`Club::chat_streaming`] (whose body this was, verbatim) for the
    /// same reason as [`Self::chat_body`].
    fn stream_body(
        &self,
        body: serde_json::Value,
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
    ) -> Result<ClubReply, String> {
        // TTSR: opt-in stream rules. With none configured (the default) the
        // attempt loop below is a single `is_empty()` short-circuit, so the
        // hot path is untouched.
        self.stream_body_with_rules(
            body,
            tools,
            cancel,
            on_delta,
            crate::stream_rules::StreamRules::global(),
        )
    }

    /// [`Self::stream_body`] with the TTSR rule set passed explicitly. Only the
    /// wrapper above and tests call this: the global set is a `OnceLock` seeded
    /// once from env/file at first touch, so a test arming a rule through it
    /// would race whichever streaming test initializes it first.
    fn stream_body_with_rules(
        &self,
        body: serde_json::Value,
        tools: &[ToolDef],
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(StreamDelta),
        rules: &crate::stream_rules::StreamRules,
    ) -> Result<ClubReply, String> {
        std::thread::scope(|scope| {
            use std::io::Read as _;
            let sent = body.get("max_tokens").and_then(|value| value.as_u64());
            let replay_private_reasoning = body
                .get("model")
                .and_then(|model| model.as_str())
                .is_some_and(|model| self.is_deepseek_v4_provider_route(model));
            let local_qwen_tool_stream = local_qwen_tool_stream_eligible(
                &self.base_url,
                &self.name,
                body.get("model").and_then(|model| model.as_str()),
                !tools.is_empty(),
            );
            // Each read is bounded by the agent's read timeout — but while streaming
            // that is normally only an *idle* deadline, and keep-alive comments
            // defeat it:
            // every `: ping` line completes a read and resets the window, so a server
            // that queues forever while pinging (observed live: z.ai under load — one
            // socket held 35 minutes, zero data, zero CPU) hangs the turn. Hold the
            // stream to a *data* deadline too: a real chunk must arrive within
            // `ANGEL_STREAM_STALL_SECS` (default 45, `0` disables) of the last.
            // A second wall-clock bound catches a provider that evades the data
            // deadline by dribbling real SSE deltas indefinitely. The ordinary
            // default is 15 minutes; competition runners can set a tighter bound.
            // One narrow exception covers a real local-serving failure mode: Qwen
            // tool parsers can keep decoding while withholding a large string
            // argument until its closing delimiter. Once a private Qwen tool stream
            // has emitted a model delta or begun a `data:` line, tolerate read
            // timeouts for a bounded `ANGEL_STREAM_TOOL_SILENCE_SECS` window and
            // emit zero-payload liveness upstream. Cold streams, other models, and
            // public providers retain the short deadline.
            // Per-line cap: a server that drips bytes without ever sending `\n` would
            // otherwise grow the line buffer without bound (OOM), and neither the idle
            // read timeout nor the stall guard fires mid-line to stop it.
            // Seed-once process cache — hop start no longer getenv's these knobs.
            let (stall_window, hard_window, tool_silence_window, max_line, max_rule_retries) =
                stream_hop_knobs();
            let stall_window = model_defaults::budgets(
                body["model"].as_str().unwrap_or_default(),
                self.env_prefix(),
            )["stream_stall_secs"]
                .as_u64()
                .map(Duration::from_secs)
                .unwrap_or(stall_window);

            let mut body = body;
            let mut fired: std::collections::HashSet<usize> = std::collections::HashSet::new();
            let mut rule_attempts = 0usize;

            'attempt: loop {
                let attempt_started = Instant::now();
                // Only a TTSR rule retry ever needs `body` again (to inject its
                // reminder below), and the delta loop can only trip a rule under
                // this exact predicate. When no retry is possible — the default:
                // no rules configured — encode by move instead of deep-cloning the
                // full request tree (entire history, base64 attachments, tool
                // schemas) every attempt, matching the buffered
                // [`Self::post_chat`] path.
                let can_retry = !rules.is_empty() && rule_attempts < max_rule_retries;
                let bytes = if can_retry {
                    self.encode_request_body(body.clone())?
                } else {
                    self.encode_request_body(std::mem::take(&mut body))?
                };
                let abort = ureq::AbortHandle::default();
                let _cancel_read = super::sse::CancelReadGuard::new(scope, cancel, abort.clone());
                let (resp, mut accounting) =
                    self.send_with_retry(&bytes, Some(cancel), Some(&abort))?;
                let mut last_data = Instant::now();
                let mut last_heartbeat = None;
                let mut reader = BufReader::new(accounting.response_reader(resp.into_reader()));
                let mut acc = StreamAccumulator::default();
                // Which deadline applies to a frame that carried no model
                // output, and whether this stream is inside the narrow private
                // Qwen parser-recovery window (which also allows zero-payload
                // heartbeats upstream). One selector keeps the keep-alive,
                // read-timeout, and blank-frame paths on the same bound.
                let idle_bound = |acc: &StreamAccumulator| {
                    let grace = !tool_silence_window.is_zero()
                        && local_tool_stream_started(local_qwen_tool_stream, acc, &[]);
                    if grace {
                        (tool_silence_window, "ANGEL_STREAM_TOOL_SILENCE_SECS", true)
                    } else {
                        (stall_window, "ANGEL_STREAM_STALL_SECS", false)
                    }
                };
                let mut usage_commit = StreamUsageCommit::from_attempt(self, accounting);
                let mut saw_data = false;
                let mut saw_done = false;
                let mut raw: Vec<u8> = Vec::new();
                let mut rule_tripped: Option<(usize, String)> = None;
                // A retryable TTSR attempt is speculative. The caller cannot
                // retract already-emitted text, so hold its deltas until the
                // attempt commits; a tripped rule drops them before retrying.
                // The ordinary no-rules streaming path still emits immediately.
                let mut pending_deltas = Vec::new();

                'stream: loop {
                    if cancel.load(Ordering::Relaxed) {
                        // User interrupted mid-reply: keep the text so far, drop any
                        // half-assembled tool call (its args JSON would be incomplete).
                        emit_pending_stream_deltas(&mut pending_deltas, on_delta);
                        return Ok(ClubReply::Text(acc.content));
                    }
                    if !hard_window.is_zero() && attempt_started.elapsed() >= hard_window {
                        let elapsed = attempt_started.elapsed().as_secs();
                        eprintln!(
                            "[club:{}] stream exceeded its {elapsed}s wall-clock budget — giving up \
                         (ANGEL_STREAM_HARD_SECS)",
                            self.name
                        );
                        return Err(format!(
                            "stream hard deadline exceeded after {elapsed}s \
                         (bound: ANGEL_STREAM_HARD_SECS)"
                        ));
                    }
                    raw.clear();
                    // `read_until` leaves bytes already read in `raw` when the
                    // underlying socket times out. Retry in place so a timeout in
                    // the middle of one large SSE line cannot drop its prefix.
                    let reached_eof = loop {
                        if cancel.load(Ordering::Relaxed) {
                            emit_pending_stream_deltas(&mut pending_deltas, on_delta);
                            return Ok(ClubReply::Text(acc.content));
                        }
                        // Cap the single-line read at `max_line + 1` so an endless
                        // newline-free stream cannot grow `raw` without bound.
                        let remaining = max_line.saturating_add(1).saturating_sub(raw.len());
                        if remaining == 0 {
                            return Err(format!(
                                "stream line exceeded {max_line} bytes (ANGEL_STREAM_MAX_LINE_BYTES) \
                             — likely a non-SSE or runaway response"
                            ));
                        }
                        let read = {
                            let mut limited = (&mut reader).take(remaining as u64);
                            limited.read_until(b'\n', &mut raw)
                        };
                        if cancel.load(Ordering::Relaxed) {
                            emit_pending_stream_deltas(&mut pending_deltas, on_delta);
                            return Ok(ClubReply::Text(acc.content));
                        }
                        match read {
                            // EOF after a timed-out partial line still leaves a line
                            // to parse; an empty buffer is the real end of stream.
                            Ok(0) => {
                                if !stall_window.is_zero()
                                    && last_data.elapsed() >= stall_window
                                    && acc.finish_reason.is_none()
                                {
                                    let stalled = last_data.elapsed().as_secs();
                                    eprintln!(
                                        "[club:{}] stream stalled — connection went idle for {stalled}s, \
                                     giving up (ANGEL_STREAM_STALL_SECS)",
                                        self.name
                                    );
                                    return Err(stalled_stream_error(
                                        &mut acc,
                                        stalled,
                                        "ANGEL_STREAM_STALL_SECS",
                                        &mut pending_deltas,
                                        on_delta,
                                    ));
                                }
                                break raw.is_empty();
                            }
                            Ok(_) => break false,
                            Err(error) => {
                                // A provider may close immediately after its terminal
                                // finish chunk without an OpenAI `[DONE]` sentinel.
                                // The explicit finish reason is enough to commit.
                                if acc.finish_reason.is_some() {
                                    break 'stream;
                                }

                                let waiting_on_local_tool =
                                    local_tool_stream_started(local_qwen_tool_stream, &acc, &raw);
                                if stream_read_timed_out(&error)
                                    && waiting_on_local_tool
                                    && !tool_silence_window.is_zero()
                                {
                                    let silent_for = last_data.elapsed();
                                    if silent_for < tool_silence_window {
                                        // No complete SSE event arrived. This only
                                        // tells the foreground watchdog that the
                                        // transport is intentionally inside its
                                        // bounded parser recovery window; it is never
                                        // rendered or counted as model output.
                                        emit_stream_heartbeat(&mut last_heartbeat, on_delta);
                                        continue;
                                    }
                                    let stalled = silent_for.as_secs();
                                    eprintln!(
                                        "[club:{}] local tool stream silent for {stalled}s — \
                                     giving up (ANGEL_STREAM_TOOL_SILENCE_SECS)",
                                        self.name
                                    );
                                    return Err(format!(
                                        "stream stalled while waiting for a buffered local tool call: \
                                     no SSE data for {stalled}s \
                                     (bound: ANGEL_STREAM_TOOL_SILENCE_SECS)"
                                    ));
                                }

                                if stream_read_timed_out(&error)
                                    && !stall_window.is_zero()
                                    && last_data.elapsed() >= stall_window
                                {
                                    let stalled = last_data.elapsed().as_secs();
                                    eprintln!(
                                        "[club:{}] stream stalled — no data for {stalled}s, \
                                     giving up (ANGEL_STREAM_STALL_SECS)",
                                        self.name
                                    );
                                    return Err(stalled_stream_error(
                                        &mut acc,
                                        stalled,
                                        "ANGEL_STREAM_STALL_SECS",
                                        &mut pending_deltas,
                                        on_delta,
                                    ));
                                }

                                // Mid-stream read error/timeout. Keep already-streamed
                                // plain text (no pending tool call), but never present
                                // the severed response as an ordinary complete answer.
                                if !acc.content.is_empty() && acc.tool_calls.is_empty() {
                                    return Err(retain_interrupted_stream(
                                        acc.content,
                                        &mut pending_deltas,
                                        on_delta,
                                    ));
                                }
                                return Err(format!(
                                    "{INCOMPLETE_STREAM_ERR}: stream read error: {error}"
                                ));
                            }
                        }
                    };
                    if reached_eof {
                        break;
                    }
                    if raw.len() > max_line {
                        return Err(format!(
                            "stream line exceeded {max_line} bytes (ANGEL_STREAM_MAX_LINE_BYTES) \
                         — likely a non-SSE or runaway response"
                        ));
                    }
                    let decoded = String::from_utf8_lossy(&raw);
                    let line = decoded.trim_end_matches(['\n', '\r']);
                    match parse_sse_line(line) {
                        SseEvent::Done => {
                            saw_done = true;
                            break;
                        }
                        SseEvent::Ignore => {
                            // Keep-alives/blank lines are activity, not progress. When
                            // only these arrive for the active stall window, give up
                            // loudly (streamed prose survives, a half tool call fails).
                            // An already-generating private tool stream gets the same
                            // bounded parser grace whether its server is silent or
                            // emits transport keep-alives.
                            let (silence_window, bound_name, local_tool_grace) = idle_bound(&acc);
                            if !silence_window.is_zero() && last_data.elapsed() >= silence_window {
                                let stalled = last_data.elapsed().as_secs();
                                if acc.finish_reason.is_some() {
                                    // The model already finished; only the protocol
                                    // sentinel (and maybe a trailing usage frame) is
                                    // missing behind the keep-alives. Commit what it
                                    // finished instead of failing a complete answer.
                                    eprintln!(
                                        "[club:{}] finish reason received, keep-alives for {stalled}s \
                                     without [DONE] — committing ({bound_name})",
                                        self.name,
                                    );
                                    break;
                                }
                                eprintln!(
                                    "[club:{}] stream stalled — keep-alives but no data for {stalled}s, \
                                 giving up ({bound_name})",
                                    self.name,
                                );
                                return Err(stalled_stream_error(
                                    &mut acc,
                                    stalled,
                                    bound_name,
                                    &mut pending_deltas,
                                    on_delta,
                                ));
                            }
                            if local_tool_grace && line.trim_start().starts_with(':') {
                                emit_stream_heartbeat(&mut last_heartbeat, on_delta);
                            }
                            continue;
                        }
                        SseEvent::Chunk(chunk) => {
                            // Error frames can carry the attempt's final usage too.
                            usage_commit.observe(&chunk);
                            // Some servers stream an {"error":…} frame instead of a status.
                            if let Some(msg) = extract_api_error(&chunk) {
                                return Err(format!("api error: {msg}"));
                            }
                            saw_data = true;
                            let d = acc.apply_chunk(&chunk);
                            if d.model_activity {
                                last_data = Instant::now();
                            } else {
                                // A well-formed frame that carried no model
                                // output is a keep-alive wearing SSE clothes.
                                // Hold it to the same data deadline as the
                                // keep-alive and read-timeout paths, or a
                                // server can park a hop at the wall-clock
                                // ceiling with blank envelopes alone.
                                let (silence_window, bound_name, _) = idle_bound(&acc);
                                if !silence_window.is_zero()
                                    && last_data.elapsed() >= silence_window
                                {
                                    let stalled = last_data.elapsed().as_secs();
                                    eprintln!(
                                        "[club:{}] stream stalled — blank frames, no model \
                                     output for {stalled}s, giving up ({bound_name})",
                                        self.name,
                                    );
                                    return Err(stalled_stream_error(
                                        &mut acc,
                                        stalled,
                                        bound_name,
                                        &mut pending_deltas,
                                        on_delta,
                                    ));
                                }
                            }
                            if let Some(c) = d.content {
                                if can_retry {
                                    pending_deltas.push(PendingStreamDelta::Content(c));
                                } else {
                                    on_delta(StreamDelta::Content(&c));
                                }
                                // TTSR: has the answer drifted into a rule's pattern?
                                // (`can_retry` is this attempt's loop-top predicate,
                                // so a trip implies the body was cloned, not moved.)
                                if can_retry
                                    && let Some((idx, rule)) =
                                        rules.first_new_match(&acc.content, &fired)
                                {
                                    rule_tripped = Some((idx, rule.reminder.clone()));
                                    break;
                                }
                            }
                            if let Some(r) = d.reasoning {
                                if can_retry {
                                    pending_deltas.push(PendingStreamDelta::Reasoning(r));
                                } else {
                                    on_delta(StreamDelta::Reasoning(&r));
                                }
                            }
                        }
                    }
                }

                if let Some((idx, reminder)) = rule_tripped {
                    // A stream rule matched mid-generation: inject it as a system
                    // reminder and retry the request, discarding the drifted partial.
                    // (`body` still holds the original request here — a rule only
                    // trips under `can_retry`, which encoded from a clone above.)
                    // `fired` stops the same rule from looping; `max_rule_retries`
                    // bounds the total. The reminder rides in the request body rather
                    // than being taxed onto every turn.
                    fired.insert(idx);
                    rule_attempts += 1;
                    inject_stream_reminder(&mut body, &reminder);
                    continue 'attempt;
                }

                // No terminal frame means the response is incomplete, even when
                // the server closes cleanly or has sent no model data at all.
                // Never dispatch half-assembled tool calls or commit partial prose.
                if !saw_done && acc.finish_reason.is_none() {
                    if !acc.content.is_empty() && acc.tool_calls.is_empty() {
                        return Err(retain_interrupted_stream(
                            acc.content,
                            &mut pending_deltas,
                            on_delta,
                        ));
                    }
                    return Err(if acc.tool_calls.is_empty() {
                        INCOMPLETE_STREAM_ERR.to_string()
                    } else {
                        format!("{INCOMPLETE_STREAM_ERR}; incomplete tool call discarded")
                    });
                }

                if acc.finish_reason.as_deref() == Some("length") {
                    // Keep cut-off *prose* for a SOTA link (nothing half-written),
                    // fail closed everywhere else.
                    if self.keep_truncated
                        && !acc.content.trim().is_empty()
                        && acc.tool_calls.is_empty()
                    {
                        self.record_retained_truncation();
                        emit_pending_stream_deltas(&mut pending_deltas, on_delta);
                        return Ok(ClubReply::Text(mark_truncated(
                            &acc.content,
                            self.truncation_policy(sent),
                        )));
                    }
                    return Err(TRUNCATED_OUTPUT_ERR.to_string());
                }
                let reasoning = acc.take_reasoning();
                let had_reasoning = !reasoning.is_empty();
                let reply = acc.into_reply(!tools.is_empty());
                if matches!(reply, ClubReply::Calls(_)) {
                    let reasoning = (!reasoning.is_empty()
                        && reasoning.len() <= TOOL_REASONING_RECEIPT_MAX_BYTES
                        && replay_private_reasoning)
                        .then_some(reasoning);
                    set_pending_tool_reasoning(reasoning);
                }
                if let ClubReply::Text(t) = &reply
                    && t.trim().is_empty()
                {
                    // A reasoning model that only thought (raw ` ` stream or
                    // a `reasoning_content` field) produced private reasoning
                    // but no answer. Distinguish it from a plain empty reply
                    // so the caller can make one bounded direct-answer retry.
                    if had_reasoning {
                        return Err(EMPTY_REPLY_REASONING_ONLY_ERR.to_string());
                    }
                    return Err(if saw_data {
                        "club returned an empty reply (no text and no tool calls)".to_string()
                    } else {
                        "club stream produced no data".to_string()
                    });
                }
                emit_pending_stream_deltas(&mut pending_deltas, on_delta);
                return Ok(reply);
            }
        })
    }
}

/// Add a TTSR stream-rule reminder to the leading system block of the request
/// body, so the retried stream sees the correction. Strict Qwen chat templates
/// reject any system message after a user/assistant message, so appending the
/// reminder used to turn reasoning-only recovery into an HTTP 400. Merge into
/// the first system message when possible; otherwise insert one at the front.
/// A no-op if the body has no `messages` array (non-chat-completions shape).
fn inject_stream_reminder(body: &mut serde_json::Value, reminder: &str) {
    let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return;
    };
    let reminder = format!("[stream-rule reminder] {reminder}");
    let leading_system_content = messages.first_mut().and_then(|message| {
        if message.get("role").and_then(|role| role.as_str()) == Some("system") {
            message.get_mut("content")
        } else {
            None
        }
    });
    if let Some(content) = leading_system_content {
        match content {
            serde_json::Value::String(existing) => {
                existing.push_str("\n\n");
                existing.push_str(&reminder);
                return;
            }
            serde_json::Value::Array(parts) => {
                parts.push(serde_json::json!({ "type": "text", "text": reminder }));
                return;
            }
            _ => {}
        }
    }
    messages.insert(
        0,
        serde_json::json!({ "role": "system", "content": reminder }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_reminder_never_places_system_after_conversation_history() {
        let mut with_system = serde_json::json!({
            "messages": [
                { "role": "system", "content": "base policy" },
                { "role": "user", "content": "work" },
                { "role": "assistant", "content": "partial" }
            ]
        });
        inject_stream_reminder(&mut with_system, "act now");
        let messages = with_system["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3, "reuse the leading system message");
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert!(
            messages[0]["content"]
                .as_str()
                .unwrap()
                .ends_with("[stream-rule reminder] act now")
        );

        let mut without_system = serde_json::json!({
            "messages": [
                { "role": "user", "content": "work" },
                { "role": "assistant", "content": "partial" }
            ]
        });
        inject_stream_reminder(&mut without_system, "answer now");
        let messages = without_system["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
    }

    #[test]
    fn live_meters_read_usage_without_taking_the_hop_mutex() {
        let src = include_str!("http.rs");
        let token = src
            .find("fn token_usage(&self) -> Option<TokenUsage>")
            .expect("HttpClub token_usage");
        let cache = src
            .find("fn cache_usage(&self) -> CacheUsage")
            .expect("HttpClub cache_usage");
        let token_body = &src[token..cache];
        assert!(
            token_body.contains("self.usage.load()") && !token_body.contains(".lock("),
            "token_usage must stay lock-free for the draw path:\n{token_body}"
        );
        let cache_end = src[cache..]
            .find("\n    fn prompt_cache_capable")
            .map(|n| cache + n)
            .expect("prompt_cache_capable follows cache_usage");
        let cache_body = &src[cache..cache_end];
        assert!(
            cache_body.contains("self.cache_usage.load()") && !cache_body.contains(".lock("),
            "cache_usage must stay lock-free for the drain fold:\n{cache_body}"
        );
    }

    /// Restore-on-drop env guard (the `club/tests` one is module-private).
    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }
    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
        fn unset(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(key) };
            Self { key, previous }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                // TODO: Audit that the environment access only happens in single-threaded code.
                None => unsafe { std::env::remove_var(self.key) },
            }
            if self.key.ends_with("REASONING_EFFORT") || self.key.ends_with("REASONING_DIALECT") {
                resync_reasoning_effort_env_from_env();
            }
            if self.key.ends_with("MAX_TOKENS") {
                resync_max_tokens_env_from_env();
            }
            if self.key.ends_with("PROMPT_CACHE") || self.key.ends_with("PROMPT_CACHE_KEY") {
                resync_prompt_cache_from_env();
            }
            if self.key.contains("ANTHROPIC_CACHE") {
                resync_openrouter_anthropic_cache_from_env();
            }
        }
    }

    #[test]
    fn structured_context_overflow_teaches_the_http_metadata_cache() {
        let detail = r#"{"error":{"code":400,"message":"request (104239 tokens) exceeds the available context size (102400 tokens)","type":"exceed_context_size_error","n_prompt_tokens":104239,"n_ctx":102400}}"#;
        assert_eq!(context_window_from_error_detail(detail), Some(102_400));

        let club = HttpClub::new("casper", "http://127.0.0.1:9/v1", "local", None);
        club.inject_metadata_for_tests(Metadata {
            context_window: 0,
            supports_cache: true,
            supports_reasoning: Some(true),
            supports_tools: true,
        });
        club.learn_context_window_from_error(detail);
        let learned = club
            .metadata
            .lock()
            .expect("metadata lock")
            .as_ref()
            .expect("learned metadata")
            .0;
        assert_eq!(learned.context_window, 102_400);
        assert!(learned.supports_cache);
        assert_eq!(learned.supports_reasoning, Some(true));
        assert!(learned.supports_tools);

        club.learn_context_window_from_error(
            r#"{"error":{"type":"invalid_request_error","n_ctx":7}}"#,
        );
        let unchanged = club
            .metadata
            .lock()
            .expect("metadata lock")
            .as_ref()
            .expect("cached metadata")
            .0;
        assert_eq!(unchanged, learned);
    }

    #[test]
    fn deepseek_v4_replays_reasoning_on_exact_live_messages_without_cross_matching() {
        let club = HttpClub::new(
            "deepseek",
            "https://api.deepseek.com/v1",
            "deepseek-v4-pro",
            None,
        );
        let repaired = vec![ToolCall {
            id: "angel_call_1_0".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({"path": "src/main.rs"}),
        }];
        let first = ChatMsg::assistant_calls_with_reasoning(
            repaired.clone(),
            Some("first-private-reasoning".to_string()),
        );
        let second = ChatMsg::assistant_calls_with_reasoning(
            repaired.clone(),
            Some("second-private-reasoning".to_string()),
        );
        let body = club
            .build_body(
                &[
                    ChatMsg::user("inspect"),
                    first.clone(),
                    ChatMsg::tool(&repaired[0].id, "source"),
                    ChatMsg::user("inspect again"),
                    second,
                    ChatMsg::tool(&repaired[0].id, "source again"),
                ],
                &[],
                true,
            )
            .expect("DeepSeek body");
        assert_eq!(
            body["messages"][1]["reasoning_content"].as_str(),
            Some("first-private-reasoning")
        );
        assert_eq!(
            body["messages"][4]["reasoning_content"].as_str(),
            Some("second-private-reasoning")
        );
        let saved = serde_json::to_string(&first).expect("serialize live message");
        assert!(!saved.contains("private-reasoning"));
        assert!(!format!("{first:?}").contains("first-private-reasoning"));
        let restored: ChatMsg = serde_json::from_str(&saved).expect("restore message");
        assert!(restored.private_reasoning.is_none());

        // The extra field is provider-specific; compatible gateways may reject
        // it, and a URL containing the official host as userinfo is not trusted.
        let generic = HttpClub::new(
            "deepseek-proxy",
            "https://api.deepseek.com@127.0.0.1/v1",
            "deepseek-v4-pro",
            None,
        );
        let generic_body = generic
            .build_body(
                &[
                    ChatMsg::user("inspect"),
                    first,
                    ChatMsg::tool(&repaired[0].id, "source"),
                ],
                &[],
                true,
            )
            .expect("generic body");
        assert!(
            generic_body["messages"][1]
                .get("reasoning_content")
                .is_none()
        );
    }

    #[test]
    fn private_reasoning_handoff_is_thread_local_and_consumed_once() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers = ["session-a", "session-b"].map(|value| {
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                set_pending_tool_reasoning(Some(value.to_string()));
                barrier.wait();
                assert_eq!(take_pending_tool_reasoning().as_deref(), Some(value));
                assert!(take_pending_tool_reasoning().is_none());
            })
        });
        for worker in workers {
            worker.join().expect("reasoning handoff worker");
        }
        assert!(take_pending_tool_reasoning().is_none());
    }

    #[test]
    fn env_knob_names_are_precomputed_from_the_club_namespace() {
        let club = HttpClub::new("deepseek-v4-pro", "http://127.0.0.1:9/v1", "m", None);
        assert_eq!(club.env_prefix(), "DEEPSEEK_V4_PRO");
        assert_eq!(
            club.effort_env_name,
            "ANGEL_DEEPSEEK_V4_PRO_REASONING_EFFORT"
        );
        assert_eq!(
            club.dialect_env_name,
            "ANGEL_DEEPSEEK_V4_PRO_REASONING_DIALECT"
        );
        assert_eq!(club.max_tokens_env_name, "ANGEL_DEEPSEEK_V4_PRO_MAX_TOKENS");
        // Model-id labels on an aggregator resolve to the provider namespace.
        let openrouter = HttpClub::new(
            "tencent/hy3:free",
            "https://openrouter.ai/api/v1",
            "tencent/hy3:free",
            None,
        );
        assert_eq!(
            openrouter.effort_env_name,
            "ANGEL_OPENROUTER_REASONING_EFFORT"
        );
    }

    /// Seed-once cache: EnvGuard overrides of effort/dialect knobs stay
    /// invisible until resync, then restore after the guard drops.
    #[test]
    fn reasoning_effort_env_seeds_once_and_resyncs_under_env_lock() {
        use crate::club::Club;
        let _guard = crate::tests::env_lock();
        let per_club = "ANGEL_T_EFFORT_CACHE_REASONING_EFFORT";
        let dialect = "ANGEL_T_EFFORT_CACHE_REASONING_DIALECT";
        {
            let _global_clear = EnvGuard::unset("ANGEL_REASONING_EFFORT");
            let _club_clear = EnvGuard::unset(per_club);
            let _dial_clear = EnvGuard::unset(dialect);
            let _grok_clear = EnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
            resync_reasoning_effort_env_from_env();
            assert_eq!(reasoning_env_var("ANGEL_REASONING_EFFORT"), None);
            assert_eq!(reasoning_env_var(per_club), None);
            assert_eq!(reasoning_env_var(dialect), None);
            assert_eq!(reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"), None);

            let _global = EnvGuard::set("ANGEL_REASONING_EFFORT", "medium");
            let _club = EnvGuard::set(per_club, "low");
            let _dial = EnvGuard::set(dialect, "glm");
            let _grok = EnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "high");
            assert_eq!(
                reasoning_env_var("ANGEL_REASONING_EFFORT"),
                None,
                "cache must not re-read env until seed reset"
            );
            assert_eq!(reasoning_env_var(per_club), None);
            assert_eq!(reasoning_env_var(dialect), None);
            assert_eq!(reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"), None);

            resync_reasoning_effort_env_from_env();
            assert_eq!(
                reasoning_env_var("ANGEL_REASONING_EFFORT").as_deref(),
                Some("medium")
            );
            assert_eq!(reasoning_env_var(per_club).as_deref(), Some("low"));
            assert_eq!(reasoning_env_var(dialect).as_deref(), Some("glm"));
            assert_eq!(
                reasoning_env_var("ANGEL_GROK_REASONING_EFFORT").as_deref(),
                Some("high")
            );

            let _snap = EnvGuard::set("ANGEL_EFFORT_SNAPSHOT", "0");
            let club = HttpClub::new("t-effort-cache", "http://127.0.0.1:9/v1", "m", None);
            assert_eq!(club.effort_env_read().as_deref(), Some("low"));
            assert_eq!(club.reasoning_effort().as_deref(), Some("low"));
            assert_eq!(club.reasoning_dialect(), ReasoningDialect::GlmThinking);
        }
        resync_reasoning_effort_env_from_env();
        assert_eq!(
            reasoning_env_var("ANGEL_REASONING_EFFORT"),
            std::env::var("ANGEL_REASONING_EFFORT").ok()
        );
        assert_eq!(reasoning_env_var(per_club), std::env::var(per_club).ok());
        assert_eq!(reasoning_env_var(dialect), std::env::var(dialect).ok());
        assert_eq!(
            reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"),
            std::env::var("ANGEL_GROK_REASONING_EFFORT").ok()
        );
    }

    #[test]
    fn effort_snapshot_invalidates_on_revision_and_ttl() {
        use crate::club::Club;
        let _guard = crate::tests::env_lock();
        let _snap = EnvGuard::set("ANGEL_EFFORT_SNAPSHOT", "1");
        let _global = EnvGuard::unset("ANGEL_REASONING_EFFORT");
        let _per = EnvGuard::unset("ANGEL_T_EFFORT_SNAP_REASONING_EFFORT");
        let _dial = EnvGuard::unset("ANGEL_T_EFFORT_SNAP_REASONING_DIALECT");
        resync_reasoning_effort_env_from_env();
        let mut club = HttpClub::new("t-effort-snap", "http://127.0.0.1:9/v1", "m", None);
        // Same deterministic clock: TTL transitions are test-controlled.
        let clock = Arc::new(std::sync::Mutex::new(Instant::now()));
        let clock_reader = Arc::clone(&clock);
        club.now = Arc::new(move || *clock_reader.lock().unwrap());

        // Warm the snapshot at the default OpenAI dialect / no effort.
        assert!(club.reasoning_effort().is_none());
        assert_eq!(club.reasoning_dialect(), ReasoningDialect::OpenAiEffort);

        // THINK override bumps route_state_revision → snapshot invalidates now.
        assert_eq!(club.set_reasoning_effort("high").as_deref(), Some("high"));
        assert_eq!(club.reasoning_effort().as_deref(), Some("high"));

        // Dialect env change does not bump revision. Without resync the
        // process cache stays frozen even after TTL; after resync the expired
        // snapshot re-resolves from the new seed.
        let _dial_set = EnvGuard::set("ANGEL_T_EFFORT_SNAP_REASONING_DIALECT", "glm");
        let fresh = *clock.lock().unwrap();
        *club.effort_snapshot.lock().unwrap() = Some((
            club.route_state_revision(),
            reasoning_env_generation(),
            fresh,
            EffortSnapshot {
                dialect: ReasoningDialect::OpenAiEffort,
                gate_reason: None,
                override_effort: Some("high".into()),
                env_effort: None,
            },
        ));
        assert_eq!(
            club.reasoning_dialect(),
            ReasoningDialect::OpenAiEffort,
            "fresh snapshot must hold the prior dialect within TTL"
        );
        *club.effort_snapshot.lock().unwrap() = Some((
            club.route_state_revision(),
            reasoning_env_generation(),
            fresh - (EFFORT_ENV_TTL + Duration::from_millis(1)),
            EffortSnapshot {
                dialect: ReasoningDialect::OpenAiEffort,
                gate_reason: None,
                override_effort: Some("high".into()),
                env_effort: None,
            },
        ));
        assert_eq!(
            club.reasoning_dialect(),
            ReasoningDialect::OpenAiEffort,
            "expired snapshot must not getenv until the env cache is resynced"
        );

        resync_reasoning_effort_env_from_env();
        *club.effort_snapshot.lock().unwrap() = Some((
            club.route_state_revision(),
            reasoning_env_generation(),
            fresh - (EFFORT_ENV_TTL + Duration::from_millis(1)),
            EffortSnapshot {
                dialect: ReasoningDialect::OpenAiEffort,
                gate_reason: None,
                override_effort: Some("high".into()),
                env_effort: None,
            },
        ));
        assert_eq!(
            club.reasoning_dialect(),
            ReasoningDialect::GlmThinking,
            "expired snapshot must re-resolve dialect from the resynced cache"
        );
        // GLM ladder is binary; the high override still surfaces.
        assert_eq!(club.reasoning_effort().as_deref(), Some("high"));
    }

    fn assert_provider_cancel_disconnects(partial: &'static str, chunked: bool, headers: bool) {
        use std::io::{Read, Write};
        let _guard = crate::tests::env_lock();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (ready, received) = std::sync::mpsc::channel();
        let cancel = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let server = scope.spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request = read_http_request(&mut socket);
                assert!(request.to_ascii_lowercase().contains("connection: close"));
                if headers {
                    write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n{}\r\n{partial}",
                        if chunked {
                            "Transfer-Encoding: chunked\r\n"
                        } else {
                            ""
                        }
                    )
                    .unwrap();
                }
                ready.send(()).unwrap();
                let mut byte = [0];
                assert_eq!(
                    socket.read(&mut byte).unwrap(),
                    0,
                    "cancel must close the provider connection"
                );
            });
            let cancel_ref = &cancel;
            let canceller = scope.spawn(move || {
                received.recv_timeout(Duration::from_secs(2)).unwrap();
                std::thread::sleep(Duration::from_millis(100));
                let start = Instant::now();
                cancel_ref.store(true, Ordering::Relaxed);
                start
            });
            let club = HttpClub::new("cancel-fixture", url, "fixture", None);
            let body = serde_json::json!({"model":"fixture", "messages":[], "stream":true});
            let reply = club.stream_body_with_rules(
                body,
                &[],
                &cancel,
                &mut |_| {},
                &crate::stream_rules::StreamRules::from_json_for_test("[]"),
            );
            let signal = canceller.join().unwrap();
            if headers {
                assert!(reply.is_ok(), "{reply:?}");
            } else {
                assert!(reply.is_err(), "cancelled request must not yield a reply");
            }
            assert!(signal.elapsed() < Duration::from_secs(2));
            server.join().unwrap();
        });
    }

    #[test]
    fn provider_cancel_disconnects_hung_response_headers() {
        assert_provider_cancel_disconnects("", false, false);
    }

    #[test]
    fn provider_cancel_disconnects_hung_sse_read() {
        assert_provider_cancel_disconnects(": ready\n\ndata: {", false, true);
    }

    #[test]
    fn provider_cancel_disconnects_partial_http_chunk_header() {
        assert_provider_cancel_disconnects("a", true, true);
    }

    #[test]
    fn provider_cancel_disconnects_partial_http_chunk_body() {
        assert_provider_cancel_disconnects("100\r\ndata: {", true, true);
    }

    /// Read one full HTTP request (headers + `Content-Length` body) off a
    /// socket so a test can assert on what actually went over the wire.
    /// (A local twin of `club/tests.rs`'s helper, which is module-private.)
    fn read_http_request(sock: &mut std::net::TcpStream) -> String {
        use std::io::Read;
        let mut req = Vec::new();
        let mut buf = [0u8; 1024];
        while let Ok(n) = sock.read(&mut buf) {
            if n == 0 {
                break;
            }
            req.extend_from_slice(&buf[..n]);
            if let Some(pos) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&req[..pos]).to_ascii_lowercase();
                let need = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if req.len() >= pos + 4 + need {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&req).into_owned()
    }

    /// Serve a fixed sequence of canned HTTP responses (one connection each),
    /// capturing the raw request that preceded each. Once the sequence is
    /// exhausted the listener drops, so an over-asking retry loop fails visibly.
    fn serve_seq(
        responses: Vec<String>,
    ) -> (
        String,
        std::sync::Arc<Mutex<Vec<String>>>,
        std::thread::JoinHandle<()>,
    ) {
        serve_seq_with_accept_timeout(responses, std::time::Duration::from_secs(2))
    }

    /// As [`serve_seq`], with a bounded per-connection wait chosen by a test
    /// whose setup can be delayed independently of the fixture thread.
    fn serve_seq_with_accept_timeout(
        responses: Vec<String>,
        accept_timeout: std::time::Duration,
    ) -> (
        String,
        std::sync::Arc<Mutex<Vec<String>>>,
        std::thread::JoinHandle<()>,
    ) {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Instant;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener
            .set_nonblocking(true)
            .expect("fixture listener nonblocking");
        let addr = listener.local_addr().unwrap();
        let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
        let captured = std::sync::Arc::clone(&requests);
        let handle = std::thread::spawn(move || {
            for response in responses {
                let deadline = Instant::now() + accept_timeout;
                let mut sock = loop {
                    match listener.accept() {
                        Ok((sock, _)) => break sock,
                        Err(err)
                            if err.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        _ => return,
                    }
                };
                let _ = sock.set_nonblocking(false);
                let _ = sock.set_read_timeout(Some(std::time::Duration::from_secs(1)));
                let _ = sock.set_write_timeout(Some(std::time::Duration::from_secs(1)));
                let req = read_http_request(&mut sock);
                captured.lock().unwrap().push(req);
                let _ = sock.write_all(response.as_bytes());
                let _ = sock.flush();
                // Wait for the client to finish reading and hang up (read → 0).
                let mut buf = [0u8; 512];
                let _ = sock.read(&mut buf);
            }
        });
        (format!("http://{addr}"), requests, handle)
    }

    /// Send one SSE event, pause long enough to trip the client's socket read
    /// timeout, then finish the same response. Models the Qwen/SGLang parser
    /// gap where decoding continues but a large tool argument is withheld.
    fn serve_delayed_stream(
        first_event: &'static str,
        delay: Duration,
        remaining_events: &'static str,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let _ = read_http_request(&mut sock);
            sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
            sock.write_all(first_event.as_bytes()).unwrap();
            sock.flush().unwrap();
            std::thread::sleep(delay);
            let _ = sock.write_all(remaining_events.as_bytes());
            let _ = sock.flush();
        });
        (format!("http://{addr}"), handle)
    }

    /// Keep sending SSE comments during the parser gap. This prevents the
    /// socket deadline from firing and exercises the independent data-stall /
    /// foreground-watchdog path.
    fn serve_keepalive_stream(
        first_event: &'static str,
        ping_delay: Duration,
        ping_count: usize,
        remaining_events: &'static str,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let _ = read_http_request(&mut sock);
            sock.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
            sock.write_all(first_event.as_bytes()).unwrap();
            sock.flush().unwrap();
            for _ in 0..ping_count {
                std::thread::sleep(ping_delay);
                let _ = sock.write_all(b": ping\n\n");
                let _ = sock.flush();
            }
            let _ = sock.write_all(remaining_events.as_bytes());
            let _ = sock.flush();
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn local_tool_stream_survives_parser_silence_past_socket_timeout() {
        let _guard = crate::tests::env_lock();
        {
            let _http_timeout = EnvGuard::set("ANGEL_HTTP_TIMEOUT", "1");
            let _tool_silence = EnvGuard::set("ANGEL_STREAM_TOOL_SILENCE_SECS", "4");
            let _stall = EnvGuard::set("ANGEL_STREAM_STALL_SECS", "1");
            resync_stream_knobs_from_env();

            // A tool-only response can cross the socket deadline inside its
            // very first giant `data:` line, before the accumulator has seen a
            // parseable event.
            let first = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,";
            let rest = concat!(
                "\"id\":\"call_1\",\"function\":{\"name\":\"write_file\",",
                "\"arguments\":\"{\\\"path\\\":\\\"note.md\\\",",
                "\\\"content\\\":\\\"hello\\\"}\"}}]},",
                "\"finish_reason\":\"tool_calls\"}]}\n\n",
                "data: [DONE]\n\n",
            );
            let (base, handle) = serve_delayed_stream(first, Duration::from_millis(1_500), rest);
            let club = HttpClub::new("qwen-parser-gap", base, "qwen", None);
            let body = serde_json::json!({
                "model": "qwen",
                "messages": [{"role": "user", "content": "write the note"}],
                "stream": true,
            });
            let tools = [ToolDef {
                name: "write_file".to_string(),
                description: "write a file".to_string(),
                params: serde_json::json!({"type": "object"}),
            }];
            let rules = crate::stream_rules::StreamRules::from_json_for_test("[]");
            let cancel = AtomicBool::new(false);
            let mut heartbeats = 0usize;
            let reply = club
                .stream_body_with_rules(
                    body,
                    &tools,
                    &cancel,
                    &mut |delta| match delta {
                        StreamDelta::Content(_) => {}
                        StreamDelta::Reasoning(_) => {}
                        StreamDelta::Heartbeat => heartbeats += 1,
                    },
                    &rules,
                )
                .expect("a bounded local parser gap must not sever the tool call");
            handle.join().unwrap();

            assert!(
                heartbeats >= 1,
                "the watchdog must learn about the bounded wait"
            );
            match reply {
                ClubReply::Calls(calls) => {
                    assert_eq!(calls.len(), 1);
                    assert_eq!(calls[0].id, "call_1");
                    assert_eq!(calls[0].name, "write_file");
                    assert_eq!(calls[0].args["path"], "note.md");
                    assert_eq!(calls[0].args["content"], "hello");
                }
                other => panic!("expected tool call, got {other:?}"),
            }
        }
        resync_stream_knobs_from_env();
    }

    #[test]
    fn local_tool_stream_keepalives_refresh_the_foreground_watchdog() {
        let _guard = crate::tests::env_lock();
        {
            let _http_timeout = EnvGuard::set("ANGEL_HTTP_TIMEOUT", "3");
            let _tool_silence = EnvGuard::set("ANGEL_STREAM_TOOL_SILENCE_SECS", "4");
            let _stall = EnvGuard::set("ANGEL_STREAM_STALL_SECS", "1");
            resync_stream_knobs_from_env();

            let first = concat!(
                "data: {\"choices\":[{\"delta\":{\"reasoning_content\":",
                "\"assembling tool arguments\"}}]}\n\n",
            );
            let rest = concat!(
                "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,",
                "\"id\":\"call_ping\",\"function\":{\"name\":\"write_file\",",
                "\"arguments\":\"{\\\"path\\\":\\\"ping.md\\\"}\"}}]},",
                "\"finish_reason\":\"tool_calls\"}]}\n\n",
                "data: [DONE]\n\n",
            );
            let (base, handle) = serve_keepalive_stream(first, Duration::from_millis(300), 5, rest);
            let club = HttpClub::new("qwen-keepalive-gap", base, "qwen", None);
            let body = serde_json::json!({
                "model": "qwen",
                "messages": [{"role": "user", "content": "write the note"}],
                "stream": true,
            });
            let tools = [ToolDef {
                name: "write_file".to_string(),
                description: "write a file".to_string(),
                params: serde_json::json!({"type": "object"}),
            }];
            let rules = crate::stream_rules::StreamRules::from_json_for_test("[]");
            let cancel = AtomicBool::new(false);
            let mut heartbeats = 0usize;
            let reply = club
                .stream_body_with_rules(
                    body,
                    &tools,
                    &cancel,
                    &mut |delta| {
                        if matches!(delta, StreamDelta::Heartbeat) {
                            heartbeats += 1;
                        }
                    },
                    &rules,
                )
                .expect("keep-alives must not defeat local parser-gap recovery");
            handle.join().unwrap();

            assert_eq!(
                heartbeats, 1,
                "keep-alives must refresh the watchdog without flooding it"
            );
            match reply {
                ClubReply::Calls(calls) => {
                    assert_eq!(calls.len(), 1);
                    assert_eq!(calls[0].id, "call_ping");
                    assert_eq!(calls[0].args["path"], "ping.md");
                }
                other => panic!("expected tool call, got {other:?}"),
            }
        }
        resync_stream_knobs_from_env();
    }

    #[test]
    fn non_qwen_local_tool_stream_keeps_the_fail_fast_timeout() {
        let _guard = crate::tests::env_lock();
        {
            let _http_timeout = EnvGuard::set("ANGEL_HTTP_TIMEOUT", "1");
            let _tool_silence = EnvGuard::set("ANGEL_STREAM_TOOL_SILENCE_SECS", "4");
            resync_stream_knobs_from_env();

            let first = concat!(
                "data: {\"choices\":[{\"delta\":{\"reasoning_content\":",
                "\"still working\"}}]}\n\n",
            );
            let (base, handle) =
                serve_delayed_stream(first, Duration::from_millis(1_500), "data: [DONE]\n\n");
            let club = HttpClub::new("llama-local", base, "llama-4", None);
            let body = serde_json::json!({
                "model": "llama-4",
                "messages": [{"role": "user", "content": "answer"}],
                "stream": true,
            });
            let tools = [ToolDef {
                name: "write_file".to_string(),
                description: "write a file".to_string(),
                params: serde_json::json!({"type": "object"}),
            }];
            let rules = crate::stream_rules::StreamRules::from_json_for_test("[]");
            let cancel = AtomicBool::new(false);
            let mut heartbeats = 0usize;
            let error = club
                .stream_body_with_rules(
                    body,
                    &tools,
                    &cancel,
                    &mut |delta| {
                        if matches!(delta, StreamDelta::Heartbeat) {
                            heartbeats += 1;
                        }
                    },
                    &rules,
                )
                .expect_err("non-Qwen routes must retain the ordinary socket timeout");
            handle.join().unwrap();

            assert!(error.contains("stream read error"), "{error}");
            assert_eq!(heartbeats, 0);
        }
        resync_stream_knobs_from_env();
    }

    #[test]
    fn local_tool_stream_grace_requires_private_qwen_started_tool_request() {
        let eligible = local_qwen_tool_stream_eligible(
            "http://127.0.0.1:8000/v1",
            "qwen38",
            Some("Qwen3.8-Flash-Next-NVFP4"),
            true,
        );
        assert!(eligible);

        let mut acc = StreamAccumulator::default();
        assert!(!local_tool_stream_started(eligible, &acc, &[]));
        assert!(local_tool_stream_started(
            eligible,
            &acc,
            b"data: {\"choices\":[{\"delta\":{\"tool_calls\":["
        ));

        acc.apply_chunk(&serde_json::json!({
            "choices": [{"delta": {"reasoning_content": "working"}}]
        }));
        assert!(local_tool_stream_started(eligible, &acc, &[]));
        assert!(!local_qwen_tool_stream_eligible(
            "https://api.example.com/v1",
            "qwen38",
            Some("qwen"),
            true,
        ));
        assert!(!local_qwen_tool_stream_eligible(
            "http://127.0.0.1:8000/v1",
            "llama",
            Some("Llama-4"),
            true,
        ));
        assert!(!local_qwen_tool_stream_eligible(
            "http://127.0.0.1:8000/v1",
            "qwen38",
            Some("qwen"),
            false,
        ));
    }

    #[test]
    fn credentialed_provider_request_never_contacts_redirect_target() {
        use std::io::Write;
        use std::net::TcpListener;
        use std::time::{Duration, Instant};

        let redirect_target = TcpListener::bind("127.0.0.1:0").unwrap();
        redirect_target.set_nonblocking(true).unwrap();
        let target_url = format!("http://{}/stolen", redirect_target.local_addr().unwrap());
        let source = TcpListener::bind("127.0.0.1:0").unwrap();
        let source_url = format!(
            "http://{}/v1/chat/completions",
            source.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let (mut socket, _) = source.accept().unwrap();
            let request = read_http_request(&mut socket);
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: {target_url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            socket.write_all(response.as_bytes()).unwrap();
            request
        });

        let club = HttpClub::new("redirect-test", source_url, "model", Some("secret".into()));
        let result = club
            .agent
            .post(&club.base_url)
            .set("Authorization", "Bearer secret")
            .send_string("{}");
        assert_eq!(result.expect("302 response").status(), 302);
        let request = server.join().unwrap();
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret")
        );

        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match redirect_target.accept() {
                Ok(_) => panic!("credentialed client contacted the redirect target"),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("redirect target accept failed: {error}"),
            }
        }
    }

    #[test]
    fn p05b_glm_reasoning_is_delivered_before_answer_and_answer_bytes_are_preserved() {
        use std::io::Write;
        let _guard = crate::tests::env_lock();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (visible_tx, visible_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let request = read_http_request(&mut socket);
            socket.write_all(concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"fixture progress\"}}]}\n\n",
            ).as_bytes()).unwrap();
            socket.flush().unwrap();
            // A test-only handshake proves delivery while the answer is still
            // withheld. This is not a product deadline or a latency benchmark.
            visible_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            socket
                .write_all(
                    concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"  answer\\n\"}}]}\n\n",
                        "data: {\"choices\":[{\"delta\":{\"content\":\"雪  \"}}]}\n\n",
                        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                        "data: [DONE]\n\n",
                    )
                    .as_bytes(),
                )
                .unwrap();
            request
        });
        let club = HttpClub::new("openrouter", base, "glm-5.3-flash", None);
        let body = club
            .build_body_with_effort(&[ChatMsg::user("fixture")], &[], true, Some("low"))
            .unwrap();
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["reasoning_effort"], "low");
        let mut answer = String::new();
        let mut reasoning = String::new();
        let reply = club
            .stream_body_with_rules(
                body,
                &[],
                &AtomicBool::new(false),
                &mut |delta| match delta {
                    StreamDelta::Reasoning(text) => {
                        assert!(answer.is_empty());
                        reasoning.push_str(text);
                        visible_tx.send(()).unwrap();
                    }
                    StreamDelta::Content(text) => answer.push_str(text),
                    StreamDelta::Heartbeat => panic!("unexpected heartbeat"),
                },
                &crate::stream_rules::StreamRules::from_json_for_test("[]"),
            )
            .unwrap();
        let request = server.join().unwrap();
        let wire: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(wire["stream"], true);
        assert_eq!(wire["thinking"]["type"], "enabled");
        assert_eq!(reasoning, "fixture progress");
        assert_eq!(answer.as_bytes(), "  answer\n雪  ".as_bytes());
        assert!(matches!(reply, ClubReply::Text(text) if text.as_bytes() == answer.as_bytes()));
    }

    #[test]
    fn p05b_plain_answer_and_reasoning_tool_calls_keep_their_payloads() {
        let _guard = crate::tests::env_lock();
        let plain = concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"  plain\\n雪  \"}}]}\n\n",
            "data: [DONE]\n\n",
        )
        .to_string();
        let tool = concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"fixture progress\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_fixture\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"fixture.txt\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (base, _, server) = serve_seq(vec![plain, tool]);
        let club = HttpClub::new("fixture", base, "fixture", None);
        for is_tool in [false, true] {
            let mut content = String::new();
            let mut reasoning = String::new();
            let reply = club
                .stream_body_with_rules(
                    serde_json::json!({"model":"fixture", "stream":true, "messages":[]}),
                    &[],
                    &AtomicBool::new(false),
                    &mut |delta| match delta {
                        StreamDelta::Content(text) => content.push_str(text),
                        StreamDelta::Reasoning(text) => reasoning.push_str(text),
                        StreamDelta::Heartbeat => panic!("unexpected heartbeat"),
                    },
                    &crate::stream_rules::StreamRules::from_json_for_test("[]"),
                )
                .unwrap();
            if is_tool {
                assert!(content.is_empty());
                assert_eq!(reasoning, "fixture progress");
                let ClubReply::Calls(calls) = reply else {
                    panic!("expected tool calls")
                };
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id, "call_fixture");
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].args, serde_json::json!({"path":"fixture.txt"}));
            } else {
                assert!(reasoning.is_empty());
                assert_eq!(content.as_bytes(), "  plain\n雪  ".as_bytes());
                assert!(matches!(reply, ClubReply::Text(text) if text == content));
            }
        }
        server.join().unwrap();
    }

    #[test]
    fn p05c_cancel_during_zai_reasoning_tears_the_stream_down() {
        use std::io::Write;
        use std::sync::atomic::Ordering;
        let _guard = crate::tests::env_lock();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (visible_tx, visible_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let _request = read_http_request(&mut socket);
            socket.write_all(concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think first\"}}]}\n\n",
            ).as_bytes()).unwrap();
            socket.flush().unwrap();
            let _ = visible_rx.recv_timeout(Duration::from_secs(5));
            let _ = socket.write_all(
                concat!(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"must not land\"}}]}\n\n",
                    "data: [DONE]\n\n",
                )
                .as_bytes(),
            );
        });
        let club = HttpClub::new("openrouter", base, "glm-5.3-flash", None);
        let cancel = AtomicBool::new(false);
        let mut reasoning = String::new();
        let mut answer = String::new();
        let reply = club.stream_body_with_rules(
            serde_json::json!({"model":"glm-5.3-flash","stream":true,"messages":[]}),
            &[],
            &cancel,
            &mut |delta| match delta {
                StreamDelta::Reasoning(text) => {
                    reasoning.push_str(text);
                    cancel.store(true, Ordering::Relaxed);
                    let _ = visible_tx.send(());
                }
                StreamDelta::Content(text) => answer.push_str(text),
                StreamDelta::Heartbeat => {}
            },
            &crate::stream_rules::StreamRules::from_json_for_test("[]"),
        );
        let _ = server.join();
        assert_eq!(reasoning, "think first");
        assert!(!answer.contains("must not land"), "{answer}");
        match reply {
            Ok(ClubReply::Text(text)) => {
                assert!(!text.contains("must not land"), "{text}");
            }
            Err(err) => {
                let lower = err.to_ascii_lowercase();
                assert!(
                    lower.contains("cancel")
                        || lower.contains("interrupt")
                        || lower.contains("abort"),
                    "{err}"
                );
            }
            other => panic!("unexpected reply {other:?}"),
        }
    }

    /// Build a one-rule TTSR set without mutating process-global environment.
    /// A temporary `ANGEL_STREAM_RULES` override can race any streaming test
    /// that initializes the global `OnceLock`, permanently contaminating that
    /// test process even when the mutating test itself holds `env_lock`.
    fn probe_stream_rules() -> crate::stream_rules::StreamRules {
        let rules = crate::stream_rules::StreamRules::from_json_for_test(
            r#"[{"pattern":"TTSR-DRIFT-PROBE","reminder":"stay on script"}]"#,
        );
        assert!(!rules.is_empty(), "probe rule must parse");
        rules
    }

    /// The attempt loop encodes by move on the default path (no rules → no
    /// deep clone of the full request tree). This pins the `can_retry` side:
    /// a configured rule tripping mid-stream must still retry with the
    /// ORIGINAL body — proving the clone survives — plus the injected
    /// reminder.
    #[test]
    fn stream_rule_retry_discards_speculative_deltas_and_resends_original_body() {
        let _guard = crate::tests::env_lock();
        let rules = probe_stream_rules();
        let drifted = concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"discarded thought\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"heading TTSR-DRIFT-PROBE off script\"}}]}\n\n",
            "data: [DONE]\n\n",
        )
        .to_string();
        let clean = concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"clean thought\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"back on script\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        )
        .to_string();
        let (base, requests, handle) = serve_seq(vec![drifted, clean]);
        let club = HttpClub::new("ttsr-clone-probe", base, "m", None);
        let body = serde_json::json!({
            "model": "m",
            "messages": [{"role": "user", "content": "the original prompt"}],
            "stream": true,
        });
        let cancel = AtomicBool::new(false);
        let mut deltas = Vec::new();
        let reply = club
            .stream_body_with_rules(
                body,
                &[],
                &cancel,
                &mut |delta| match delta {
                    StreamDelta::Content(text) => {
                        deltas.push(("content", text.to_string()));
                    }
                    StreamDelta::Reasoning(text) => {
                        deltas.push(("reasoning", text.to_string()));
                    }
                    StreamDelta::Heartbeat => {}
                },
                &rules,
            )
            .expect("rule retry recovers a clean stream");
        // The reply comes from the retried stream, not the drifted partial.
        match reply {
            ClubReply::Text(t) => assert_eq!(t, "back on script"),
            other => panic!("expected text, got {other:?}"),
        }
        assert_eq!(
            deltas,
            vec![
                ("reasoning", "clean thought".to_string()),
                ("content", "back on script".to_string()),
            ],
            "speculative deltas from the discarded attempt must never leak"
        );
        let reqs = requests.lock().unwrap();
        assert_eq!(reqs.len(), 2, "one drifted attempt + one rule retry");
        assert!(
            reqs[1].contains("the original prompt"),
            "retry lost the original body: {}",
            reqs[1]
        );
        assert!(
            reqs[1].contains("[stream-rule reminder] stay on script"),
            "retry must carry the injected reminder: {}",
            reqs[1]
        );
        assert!(
            !reqs[0].contains("stream-rule reminder"),
            "first attempt is the untouched request"
        );
        drop(reqs);
        handle.join().unwrap();
    }

    /// The `can_retry = false` side of the same gate: with the retry budget at
    /// zero a matching rule must not fire, the body is encoded by move, and
    /// the stream completes in exactly one request carrying the same bytes.
    #[test]
    fn stream_rule_gate_exhausted_encodes_by_move_single_attempt() {
        let _guard = crate::tests::env_lock();
        {
            let rules = probe_stream_rules();
            let _retries = EnvGuard::set("ANGEL_STREAM_RULE_RETRIES", "0");
            resync_stream_knobs_from_env();
            let drifted = concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"heading TTSR-DRIFT-PROBE off script\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n",
            )
            .to_string();
            let (base, requests, handle) = serve_seq(vec![drifted]);
            let club = HttpClub::new("ttsr-move-probe", base, "m", None);
            let body = serde_json::json!({
                "model": "m",
                "messages": [{"role": "user", "content": "the original prompt"}],
                "stream": true,
            });
            let cancel = AtomicBool::new(false);
            let reply = club
                .stream_body_with_rules(body, &[], &cancel, &mut |_| {}, &rules)
                .expect("no retry budget keeps the single stream");
            handle.join().unwrap();
            match reply {
                ClubReply::Text(t) => assert_eq!(t, "heading TTSR-DRIFT-PROBE off script"),
                other => panic!("expected text, got {other:?}"),
            }
            let reqs = requests.lock().unwrap();
            assert_eq!(reqs.len(), 1, "an exhausted budget must not re-send");
            assert!(
                reqs[0].contains("the original prompt"),
                "the moved body still encodes the request: {}",
                reqs[0]
            );
        }
        resync_stream_knobs_from_env();
    }

    /// Seed-once cache: EnvGuard overrides are invisible until resync, then
    /// visible, then restored after the guard drops. Covers all five knobs.
    #[test]
    fn stream_hop_knobs_seed_once_and_resync_under_env_lock() {
        let _guard = crate::tests::env_lock();
        {
            let _stall_clear = EnvGuard::unset("ANGEL_STREAM_STALL_SECS");
            let _hard_clear = EnvGuard::unset("ANGEL_STREAM_HARD_SECS");
            let _tool_clear = EnvGuard::unset("ANGEL_STREAM_TOOL_SILENCE_SECS");
            let _line_clear = EnvGuard::unset("ANGEL_STREAM_MAX_LINE_BYTES");
            let _retries_clear = EnvGuard::unset("ANGEL_STREAM_RULE_RETRIES");
            resync_stream_knobs_from_env();
            let (stall, hard, tool_silence, max_line, retries) = stream_hop_knobs();
            assert_eq!(stall, Duration::from_secs(DEFAULT_STREAM_STALL_SECS));
            assert_eq!(hard, Duration::from_secs(DEFAULT_STREAM_HARD_SECS));
            assert_eq!(
                tool_silence,
                Duration::from_secs(DEFAULT_STREAM_TOOL_SILENCE_SECS)
            );
            assert_eq!(max_line, DEFAULT_STREAM_MAX_LINE_BYTES);
            assert_eq!(retries, DEFAULT_STREAM_RULE_RETRIES);

            let _stall = EnvGuard::set("ANGEL_STREAM_STALL_SECS", "1");
            let _hard = EnvGuard::set("ANGEL_STREAM_HARD_SECS", "2");
            let _tool = EnvGuard::set("ANGEL_STREAM_TOOL_SILENCE_SECS", "3");
            let _line = EnvGuard::set("ANGEL_STREAM_MAX_LINE_BYTES", "4096");
            let _retries = EnvGuard::set("ANGEL_STREAM_RULE_RETRIES", "0");
            let (stall, hard, tool_silence, max_line, retries) = stream_hop_knobs();
            assert_eq!(
                stall,
                Duration::from_secs(DEFAULT_STREAM_STALL_SECS),
                "cache must not re-read env until seed reset"
            );
            assert_eq!(
                hard,
                Duration::from_secs(DEFAULT_STREAM_HARD_SECS),
                "cache must not re-read env until seed reset"
            );
            assert_eq!(
                tool_silence,
                Duration::from_secs(DEFAULT_STREAM_TOOL_SILENCE_SECS),
                "cache must not re-read env until seed reset"
            );
            assert_eq!(max_line, DEFAULT_STREAM_MAX_LINE_BYTES);
            assert_eq!(retries, DEFAULT_STREAM_RULE_RETRIES);

            resync_stream_knobs_from_env();
            let (stall, hard, tool_silence, max_line, retries) = stream_hop_knobs();
            assert_eq!(stall, Duration::from_secs(1));
            assert_eq!(hard, Duration::from_secs(2));
            assert_eq!(tool_silence, Duration::from_secs(3));
            assert_eq!(max_line, 4096);
            assert_eq!(retries, 0);
        }
        resync_stream_knobs_from_env();
        let (stall, hard, tool_silence, max_line, retries) = stream_hop_knobs();
        assert_eq!(
            stall,
            env_secs("ANGEL_STREAM_STALL_SECS", DEFAULT_STREAM_STALL_SECS)
        );
        assert_eq!(
            hard,
            env_secs("ANGEL_STREAM_HARD_SECS", DEFAULT_STREAM_HARD_SECS)
        );
        assert_eq!(
            tool_silence,
            env_secs(
                "ANGEL_STREAM_TOOL_SILENCE_SECS",
                DEFAULT_STREAM_TOOL_SILENCE_SECS
            )
        );
        assert_eq!(
            max_line,
            env_usize("ANGEL_STREAM_MAX_LINE_BYTES", DEFAULT_STREAM_MAX_LINE_BYTES)
        );
        assert_eq!(
            retries,
            env_usize("ANGEL_STREAM_RULE_RETRIES", DEFAULT_STREAM_RULE_RETRIES)
        );
    }

    /// Seed-once cache: EnvGuard overrides of the global and per-club
    /// `STREAM_USAGE` pins stay invisible until resync, then restore after the
    /// guard drops.
    #[test]
    fn stream_usage_pins_seed_once_and_resync_under_env_lock() {
        let _guard = crate::tests::env_lock();
        let club_pin = "ANGEL_STREAM_USAGE_PIN_CACHE_PROBE_STREAM_USAGE";
        {
            let _global_clear = EnvGuard::unset("ANGEL_STREAM_USAGE");
            let _club_clear = EnvGuard::unset(club_pin);
            resync_stream_usage_pins_from_env();
            assert_eq!(stream_usage_global(), None);
            assert_eq!(stream_usage_club_pin(club_pin), None);

            let _global = EnvGuard::set("ANGEL_STREAM_USAGE", "1");
            let _club = EnvGuard::set(club_pin, "0");
            assert_eq!(
                stream_usage_global(),
                None,
                "cache must not re-read env until seed reset"
            );
            assert_eq!(stream_usage_club_pin(club_pin), None);

            resync_stream_usage_pins_from_env();
            assert_eq!(stream_usage_global(), Some(true));
            assert_eq!(stream_usage_club_pin(club_pin), Some(false));
        }
        resync_stream_usage_pins_from_env();
        assert_eq!(stream_usage_global(), env_truthy_pin("ANGEL_STREAM_USAGE"));
        assert_eq!(stream_usage_club_pin(club_pin), env_truthy_pin(club_pin));
    }

    /// Seed-once cache: EnvGuard overrides are invisible until resync, then
    /// visible, then restored after the guard drops. Covers both knobs.
    #[test]
    fn ratelimit_knobs_seed_once_and_resync_under_env_lock() {
        let _guard = crate::tests::env_lock();
        {
            let _proactive_clear = EnvGuard::unset("ANGEL_RATELIMIT_PROACTIVE");
            let _wait_clear = EnvGuard::unset("ANGEL_RATELIMIT_MAX_WAIT");
            resync_ratelimit_knobs_from_env();
            let (proactive, max_wait) = ratelimit_send_knobs();
            assert!(proactive);
            assert_eq!(
                max_wait,
                Duration::from_secs(DEFAULT_RATELIMIT_MAX_WAIT_SECS)
            );

            let _proactive = EnvGuard::set("ANGEL_RATELIMIT_PROACTIVE", "0");
            let _wait = EnvGuard::set("ANGEL_RATELIMIT_MAX_WAIT", "1");
            let (proactive, max_wait) = ratelimit_send_knobs();
            assert!(proactive, "cache must not re-read env until seed reset");
            assert_eq!(
                max_wait,
                Duration::from_secs(DEFAULT_RATELIMIT_MAX_WAIT_SECS)
            );

            resync_ratelimit_knobs_from_env();
            let (proactive, max_wait) = ratelimit_send_knobs();
            assert!(!proactive);
            assert_eq!(max_wait, Duration::from_secs(1));
        }
        resync_ratelimit_knobs_from_env();
        let (proactive, max_wait) = ratelimit_send_knobs();
        let (want_proactive, want_wait) = ratelimit_knobs_from_env();
        assert_eq!(proactive, want_proactive);
        assert_eq!(max_wait, want_wait);
    }

    /// Seed-once cache: EnvGuard overrides of `ANGEL_{CLUB}_MAX_TOKENS` /
    /// `ANGEL_CLUB_MAX_TOKENS` stay invisible until resync, then restore after
    /// the guard drops. Per-club still beats global; parse/zero-filter is
    /// unchanged.
    #[test]
    fn club_max_tokens_env_seeds_once_and_resyncs_under_env_lock() {
        use crate::club::Club;
        let _guard = crate::tests::env_lock();
        let per_club = "ANGEL_MAX_TOKENS_CACHE_PROBE_MAX_TOKENS";
        {
            let _global_clear = EnvGuard::unset("ANGEL_CLUB_MAX_TOKENS");
            let _club_clear = EnvGuard::unset(per_club);
            resync_max_tokens_env_from_env();
            assert_eq!(max_tokens_env("ANGEL_CLUB_MAX_TOKENS"), None);
            assert_eq!(max_tokens_env(per_club), None);
            let club = HttpClub::new("max-tokens-cache-probe", "http://127.0.0.1:9/v1", "m", None);
            assert_eq!(
                club.route_metadata().output_budget,
                OutputBudgetPolicy::ProviderNative
            );

            let _global = EnvGuard::set("ANGEL_CLUB_MAX_TOKENS", "4096");
            let _club = EnvGuard::set(per_club, "512");
            assert_eq!(
                max_tokens_env("ANGEL_CLUB_MAX_TOKENS"),
                None,
                "cache must not re-read env until seed reset"
            );
            assert_eq!(max_tokens_env(per_club), None);
            assert_eq!(
                club.route_metadata().output_budget,
                OutputBudgetPolicy::ProviderNative,
                "cache must not re-read env until seed reset"
            );

            resync_max_tokens_env_from_env();
            assert_eq!(max_tokens_env("ANGEL_CLUB_MAX_TOKENS"), Some(4096));
            assert_eq!(max_tokens_env(per_club), Some(512));
            assert_eq!(
                club.route_metadata().output_budget,
                OutputBudgetPolicy::Explicit {
                    tokens: 512,
                    source: OutputBudgetSource::PerClubEnv,
                }
            );
            assert_eq!(
                club.route_metadata().output_budget_provenance.as_deref(),
                Some(per_club)
            );
        }
        resync_max_tokens_env_from_env();
        assert_eq!(
            max_tokens_env("ANGEL_CLUB_MAX_TOKENS"),
            parse_max_tokens_env("ANGEL_CLUB_MAX_TOKENS")
        );
        assert_eq!(max_tokens_env(per_club), parse_max_tokens_env(per_club));
    }

    /// Seed-once cache: EnvGuard overrides of the global and per-club
    /// `PROMPT_CACHE` pins and `ANGEL_PROMPT_CACHE_KEY` stay invisible until
    /// resync, then restore after the guard drops. Send policy is unchanged.
    #[test]
    fn prompt_cache_pins_seed_once_and_resync_under_env_lock() {
        let _guard = crate::tests::env_lock();
        let club_pin = "ANGEL_PROMPT_CACHE_PIN_CACHE_PROBE_PROMPT_CACHE";
        {
            let _global_clear = EnvGuard::unset("ANGEL_PROMPT_CACHE");
            let _club_clear = EnvGuard::unset(club_pin);
            let _key_clear = EnvGuard::unset("ANGEL_PROMPT_CACHE_KEY");
            resync_prompt_cache_from_env();
            assert_eq!(prompt_cache_global_mode(), PromptCacheMode::Capability);
            assert_eq!(prompt_cache_club_pin(club_pin), None);
            assert_eq!(prompt_cache_explicit_key(), None);

            let _global = EnvGuard::set("ANGEL_PROMPT_CACHE", "1");
            let _club = EnvGuard::set(club_pin, "0");
            let _key = EnvGuard::set("ANGEL_PROMPT_CACHE_KEY", "explicit-key");
            assert_eq!(
                prompt_cache_global_mode(),
                PromptCacheMode::Capability,
                "cache must not re-read env until seed reset"
            );
            assert_eq!(prompt_cache_club_pin(club_pin), None);
            assert_eq!(
                prompt_cache_explicit_key(),
                None,
                "cache must not re-read env until seed reset"
            );

            resync_prompt_cache_from_env();
            assert_eq!(prompt_cache_global_mode(), PromptCacheMode::ForceOn);
            assert_eq!(prompt_cache_club_pin(club_pin), Some(false));
            assert_eq!(prompt_cache_explicit_key().as_deref(), Some("explicit-key"));
        }
        resync_prompt_cache_from_env();
        assert_eq!(
            prompt_cache_global_mode(),
            prompt_cache_global_mode_from_env()
        );
        assert_eq!(prompt_cache_club_pin(club_pin), env_truthy_pin(club_pin));
        assert_eq!(
            prompt_cache_explicit_key(),
            prompt_cache_explicit_key_from_env()
        );
    }

    /// Seed-once cache: EnvGuard overrides of OpenRouter Claude cache knobs
    /// stay invisible until resync, then restore after the guard drops.
    /// Breakpoint floor clamp and TTL `1h` match stay unchanged.
    #[test]
    fn openrouter_anthropic_cache_cfg_seeds_once_and_resyncs_under_env_lock() {
        let _guard = crate::tests::env_lock();
        {
            let _enabled_clear = EnvGuard::unset("ANGEL_OPENROUTER_ANTHROPIC_CACHE");
            let _floor_clear = EnvGuard::unset("ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR");
            let _ttl_clear = EnvGuard::unset("ANGEL_ANTHROPIC_CACHE_TTL");
            resync_openrouter_anthropic_cache_from_env();
            assert_eq!(openrouter_anthropic_cache_cfg(), (true, 1024, false));

            let _enabled = EnvGuard::set("ANGEL_OPENROUTER_ANTHROPIC_CACHE", "0");
            let _floor = EnvGuard::set("ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR", "100");
            let _ttl = EnvGuard::set("ANGEL_ANTHROPIC_CACHE_TTL", "1h");
            assert_eq!(
                openrouter_anthropic_cache_cfg(),
                (true, 1024, false),
                "cache must not re-read env until seed reset"
            );

            resync_openrouter_anthropic_cache_from_env();
            assert_eq!(
                openrouter_anthropic_cache_cfg(),
                (false, 256, true),
                "floor still clamps 256..=65536; only exact 1h requests 1h TTL"
            );
        }
        resync_openrouter_anthropic_cache_from_env();
        assert_eq!(
            openrouter_anthropic_cache_cfg(),
            openrouter_anthropic_cache_cfg_from_env()
        );
    }
    /// Operator-ordered F01 contract: a useful output floor may exceed remaining allocation.
    #[test]
    fn formation_graph_wire_cap_fits_shared_remaining_allocation() {
        let _lock = crate::tests::env_lock();
        let budget = crate::harness::formation_budget::Budget::new(Some(1000), None);
        let _scope = crate::harness::formation_budget::enter(Some(budget.clone()));
        let payload =
            serde_json::json!({"choices":[{"message":{"role":"assistant","content":"done"}}],
            "usage":{"prompt_tokens":100,"completion_tokens":10,
                "prompt_tokens_details":{"cached_tokens":20,"cache_write_tokens":0},
                "completion_tokens_details":{"reasoning_tokens":2}}})
            .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        );
        let (base, requests, server) =
            serve_seq_with_accept_timeout(vec![response], std::time::Duration::from_secs(30));
        let club = HttpClub::new("graph-budget-test", &base, "graph-budget-test", None);
        club.post_chat(
            serde_json::json!({"model":"graph-budget-test", "messages":[], "max_tokens":5000}),
        )
        .unwrap();
        server.join().unwrap();
        let requests = requests.lock().unwrap();
        let wire: serde_json::Value =
            serde_json::from_str(requests[0].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(wire["max_tokens"], 1024);
        let receipt = budget.snapshot();
        assert_eq!(receipt["reserved"], 0);
        assert_eq!(receipt["calls"][0]["max_output"], wire["max_tokens"]);
        assert!(receipt["calls"][0]["reserved"].as_u64().unwrap() > 1000);
        assert_eq!(receipt["over_allocation"], true);
        assert_eq!(receipt["paid_spent"], 90);
    }

    #[test]
    fn formation_budget_turn_abort_preserves_stream_observation_before_attempt_drop() {
        let _lock = crate::tests::env_lock();
        let _tokens = EnvGuard::set("ANGEL_FORMATION_TOKEN_BUDGET", "5000");
        let _wall = EnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
        let scope = crate::harness::formation_budget::start_turn().unwrap();
        let budget = crate::harness::formation_budget::current().unwrap();
        let club = HttpClub::new("glm-5.3", "http://127.0.0.1:9/v1", "glm-5.3", None);
        let mut accounting = club.accounting.attempt();
        accounting.reserve_formation(budget.reserve("glm-5.3", 1000, 100).unwrap());
        let mut attempt = StreamUsageCommit::from_attempt(&club, accounting);
        attempt.observe(
            &serde_json::json!({"usage":{"prompt_tokens":500,"completion_tokens":20,
            "total_tokens":520,"prompt_tokens_details":{"cached_tokens":100,"cache_write_tokens":0},
            "completion_tokens_details":{"reasoning_tokens":5}}}),
        );
        drop(scope); // The worker still owns its attempt when the turn exits.
        let receipt = crate::harness::formation_budget::snapshot().unwrap();
        assert_eq!(receipt["reserved"], 0);
        assert_eq!(receipt["spent"], 520);
        assert_eq!(receipt["calls"][0]["settled_reason"], "aborted");
        drop(attempt);
        assert_eq!(budget.snapshot(), receipt);
    }

    #[test]
    fn formation_graph_live_shaped_terminal_usage_releases_workers_before_fanin() {
        let _lock = crate::tests::env_lock();
        // G01c/root-live-b20 GLM nofault first attempts; fan-in is the
        // observed GLM worker_fail attempt (nofault fan-in usage is unknown).
        // This tests adapter accounting under both route labels, not a claim
        // that the hosted route reported these same token counts.
        for model in ["glm-5.3-flash", "deepseek-v4-flash"] {
            for total in [48_000, 288_000] {
                let budget = crate::harness::formation_budget::Budget::new(Some(total), None);
                budget.set_unstarted(6);
                let club = HttpClub::new(model, "http://127.0.0.1:9/v1", model, None);
                let finish = |role: &str, prompt, input, output, cached, reasoning| {
                    let _role = crate::harness::formation_budget::enter_role(role);
                    let mut accounting = club.accounting.attempt();
                    accounting.reserve_formation(budget.reserve(model, prompt, 1024).unwrap());
                    let mut stream = StreamUsageCommit::from_attempt(&club, accounting);
                    stream
                        .observe(&serde_json::json!({"choices":[{"delta":{"content":"fixture"}}]}));
                    assert!(budget.snapshot()["reserved"].as_u64().unwrap() > 0);
                    stream.observe(&serde_json::json!({"choices":[], "usage":{
                        "prompt_tokens":input,"completion_tokens":output,
                        "prompt_tokens_details":{"cached_tokens":cached},
                        "completion_tokens_details":{"reasoning_tokens":reasoning}}}));
                    // Observing the last chunk must not release a running stream.
                    assert!(budget.snapshot()["reserved"].as_u64().unwrap() > 0);
                    stream
                };
                drop(finish("planner", 5295, 1268, 528, 0, 24));
                let workers = [
                    finish("worker_alpha", 7840, 1848, 81, 384, 17),
                    finish("worker_beta", 7837, 1848, 42, 384, 9),
                    finish("worker_gamma", 7861, 1861, 80, 384, 54),
                ];
                let before = budget.available_tokens().unwrap();
                drop(workers);
                assert!(budget.available_tokens().unwrap() > before);
                drop(finish("reviewer", 10260, 2507, 428, 384, 12));
                drop(finish("fanin", 6710, 1695, 49, 960, 5));
                let receipt = budget.snapshot();
                assert_eq!(receipt["reserved"], 0);
                assert_eq!(receipt["spent"], 12_235);
                assert_eq!(receipt["paid_spent"], 9_739);
                assert_eq!(receipt["budget_exhausted"], false);
                println!("G01f terminal-frame fixture model={model} allocation={total} {receipt}");
            }
        }
    }

    #[test]
    fn usage_contract_scripted_frames_settle_one_shared_formation_budget() {
        let _lock = crate::tests::env_lock();
        let _tokens = EnvGuard::set("ANGEL_FORMATION_TOKEN_BUDGET", "5000");
        let _wall = EnvGuard::unset("ANGEL_FORMATION_WALL_SECS");
        let _scope = crate::harness::formation_budget::start_turn().unwrap();
        let budget = crate::harness::formation_budget::current().unwrap();
        for model in ["glm-5.3-flash", "glm-5.3"] {
            let club = HttpClub::new(model, "http://127.0.0.1:9/v1", model, None);
            let before = club.usage_accounting();
            let mut accounting = club.accounting.attempt();
            accounting.reserve_formation(budget.reserve(model, 1000, 100).unwrap());
            {
                let mut attempt = StreamUsageCommit::from_attempt(&club, accounting);
                let frame = serde_json::json!({"usage":{"prompt_tokens":500,"completion_tokens":20,
                    "total_tokens":520,"prompt_tokens_details":{"cached_tokens":100,"cache_write_tokens":0},
                    "completion_tokens_details":{"reasoning_tokens":5}}});
                attempt.observe(&frame);
                attempt.observe(&frame); // Repeated cumulative SSE frames charge once.
            }
            let usage = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
            assert_eq!(usage.total_prompt, Some(500));
            assert_eq!(usage.generation_output, Some(20));
            assert_eq!(usage.uncached_input, Some(400));
            assert!(usage.core_complete);
        }
        let snapshot = budget.snapshot();
        assert_eq!(snapshot["spent"], 1040);
        assert_eq!(snapshot["reserved"], 0);
        assert_eq!(snapshot["budget_exhausted"], false);
        for (call, model) in snapshot["calls"]
            .as_array()
            .unwrap()
            .iter()
            .zip(["glm-5.3-flash", "glm-5.3"])
        {
            assert_eq!(call["route"], model);
            assert_eq!(call["normalized_usage"]["source"], "reported");
        }
        let club = HttpClub::new("missing", "http://127.0.0.1:9/v1", "missing", None);
        let mut accounting = club.accounting.attempt();
        accounting.reserve_formation(budget.reserve("missing", 100, 100).unwrap());
        drop(StreamUsageCommit::from_attempt(&club, accounting));
        assert_eq!(budget.snapshot()["exhaustion_reason"], "unknown_usage");
        println!(
            "G02c scripted shared budget: two routes, 1040 tokens, no double counting; missing usage remains unknown"
        );
    }
}

#[cfg(test)]
mod usage_accounting_tests {
    use super::*;
    #[test]
    fn provider_usage_http_explicit_zero_is_observed() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        c.record_usage_json(&serde_json::json!({"usage":{"prompt_tokens":0,"completion_tokens":0,"completion_tokens_details":{"reasoning_tokens":0}}}));
        assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
        assert_eq!(c.token_usage().unwrap_or_default().total_input, 0);
    }

    #[test]
    fn provider_usage_http_null_aliases_do_not_hide_valid_responses_counts() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        c.record_usage_json(&serde_json::json!({"usage":{"prompt_tokens":null,"input_tokens":12,"completion_tokens":null,"output_tokens":5,"prompt_tokens_details":{"cached_tokens":null},"input_tokens_details":{"cached_tokens":7}}}));
        assert_eq!(c.token_usage().unwrap_or_default().total_input, 12);
        assert_eq!(c.token_usage().unwrap_or_default().total_output, 5);
        assert_eq!(c.cache_usage().read_input_tokens, 7);
    }

    #[test]
    fn provider_usage_deepseek_null_detail_keeps_native_cache_hit() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        c.record_usage_json(&serde_json::json!({"usage":{"prompt_tokens":100,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":null},"prompt_cache_hit_tokens":75}}));
        assert_eq!(c.cache_usage().read_input_tokens, 75);
    }

    #[test]
    fn provider_usage_anthropic_null_write_alias_keeps_cache_creation() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        c.record_usage_json(&serde_json::json!({"usage":{"input_tokens":100,"output_tokens":5,"cache_write_input_tokens":null,"cache_creation_input_tokens":4,"cache_read_input_tokens":20}}));
        assert_eq!(c.cache_usage().write_input_tokens, 4);
        assert_eq!(c.token_usage().unwrap_or_default().total_input, 100);
    }

    #[test]
    fn provider_usage_http_responses_style_cache_write_detail_is_retained() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        c.record_usage_json(&serde_json::json!({"usage":{"input_tokens":100,"output_tokens":5,"input_tokens_details":{"cached_tokens":20,"cache_write_tokens":10}}}));
        assert_eq!(c.cache_usage().write_input_tokens, 10);
    }

    #[test]
    fn provider_usage_http_metadata_only_final_frame_keeps_last_usage() {
        for metadata in [
            serde_json::Value::Null,
            serde_json::json!({}),
            serde_json::json!({"billing":"pending"}),
        ] {
            let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
            {
                let mut commit = StreamUsageCommit::new(&c);
                commit.observe(
                    &serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}),
                );
                commit.observe(&serde_json::json!({"usage":metadata}));
            }
            assert_eq!(c.token_usage().unwrap_or_default().total_input, 12);
            assert_eq!(c.token_usage().unwrap_or_default().total_output, 5);
            assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
        }
    }

    #[test]
    fn provider_usage_http_partial_cumulative_frame_keeps_previously_reported_input() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        {
            let mut commit = StreamUsageCommit::new(&c);
            commit
                .observe(&serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}));
            commit.observe(&serde_json::json!({"usage":{"completion_tokens":6}}));
        }
        assert_eq!(c.token_usage().unwrap_or_default().total_input, 12);
        assert_eq!(c.token_usage().unwrap_or_default().total_output, 6);
        assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
    }

    #[test]
    fn provider_usage_http_explicit_zero_final_frame_overrides_prior_cumulative_counts() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        {
            let mut commit = StreamUsageCommit::new(&c);
            commit
                .observe(&serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}));
            commit.observe(&serde_json::json!({"usage":{"prompt_tokens":0,"completion_tokens":0}}));
        }
        assert_eq!(c.token_usage().unwrap_or_default().turns, 1);
        assert_eq!(c.token_usage().unwrap_or_default().total_input, 0);
        assert_eq!(c.token_usage().unwrap_or_default().total_output, 0);
    }

    #[test]
    fn provider_usage_http_cumulative_corrections_and_attempts_do_not_double_count() {
        let c = HttpClub::new("usage-fixture", "http://127.0.0.1:9/v1", "fixture", None);
        for _ in 0..2 {
            let mut commit = StreamUsageCommit::new(&c);
            commit
                .observe(&serde_json::json!({"usage":{"prompt_tokens":12,"completion_tokens":5}}));
            commit
                .observe(&serde_json::json!({"usage":{"prompt_tokens":10,"completion_tokens":4}}));
        }
        assert_eq!(c.token_usage().unwrap_or_default().turns, 2);
        assert_eq!(c.token_usage().unwrap_or_default().total_input, 20);
        assert_eq!(c.token_usage().unwrap_or_default().total_output, 8);
    }
}

#[cfg(test)]
mod usage_projection_tests {
    use super::*;
    #[test]
    fn usage_projection_http_commits_partial_stream_paths_and_cache_writes() {
        let club = HttpClub::new("fixture", "https://api.openai.com/v1", "fixture", None);
        let before = club.usage_accounting();
        {
            let mut commit = StreamUsageCommit::new(&club);
            commit.observe(&serde_json::json!({"usage":{"prompt_tokens":100,"completion_tokens":1,"prompt_tokens_details":{"cached_tokens":20,"cache_write_tokens":10}}}));
            commit.observe(&serde_json::json!({"usage":{"completion_tokens":5}}));
            commit.observe(&serde_json::json!({"usage":null}));
        }
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(
            (report.input, report.output, report.reasoning),
            (Some(100), Some(5), None)
        );
        assert_eq!(
            (
                report.uncached_input,
                report.total_prompt,
                report.generation_output
            ),
            (Some(70), Some(100), Some(5))
        );
        assert_eq!(report.raw_field_reports["prompt_tokens"], 1);
        assert_eq!(
            report.raw_field_reports["prompt_tokens_details.cache_write_tokens"],
            1
        );
    }

    #[test]
    fn usage_projection_http_zero_retry_unknown_and_cache_only_survive() {
        let club = HttpClub::new("fixture", "https://openrouter.ai/api/v1", "fixture", None);
        let before = club.usage_accounting();
        {
            let _missing = StreamUsageCommit::new(&club);
        }
        {
            let mut commit = StreamUsageCommit::new(&club);
            commit.observe(&serde_json::json!({"usage":{"prompt_tokens":0,"completion_tokens":0}}));
        }
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(
            (
                report.attempts,
                report.input,
                report.output,
                report.reasoning
            ),
            (2, Some(0), Some(0), None)
        );
        assert_eq!(report.reported_attempts.input, 1);
        assert!(!report.core_complete);
        let before = club.usage_accounting();
        {
            let mut commit = StreamUsageCommit::new(&club);
            commit.observe(
                &serde_json::json!({"usage":{"prompt_tokens_details":{"cached_tokens":80}}}),
            );
        }
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(
            (report.input, report.output, report.cache_read),
            (None, None, Some(80))
        );
        assert_eq!(report.cache_hit_pct, None);
    }

    #[test]
    fn usage_projection_http_custom_endpoint_never_guesses_contract() {
        let club = HttpClub::new(
            "openai-looking-label",
            "http://127.0.0.1:9/v1",
            "fixture",
            None,
        );
        let before = club.usage_accounting();
        {
            let mut commit = StreamUsageCommit::new(&club);
            commit.observe(&serde_json::json!({"usage":{"input_tokens":100,"output_tokens":5,"cache_read_input_tokens":20,"cache_creation_input_tokens":4}}));
        }
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(
            (report.input, report.cache_read, report.cache_write),
            (Some(100), Some(20), Some(4))
        );
        assert_eq!(
            (
                report.uncached_input,
                report.total_prompt,
                report.cache_hit_pct
            ),
            (None, None, None)
        );
        assert_eq!(report.cache_convention_attempts["unknown"], 1);
    }

    #[test]
    fn usage_projection_http_fallback_alias_keeps_exact_numeric_provenance() {
        let club = HttpClub::new("fixture", "https://api.deepseek.com/v1", "fixture", None);
        let before = club.usage_accounting();
        {
            let mut commit = StreamUsageCommit::new(&club);
            commit.observe(&serde_json::json!({"usage":{"prompt_tokens":null,"input_tokens":100,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":null},"prompt_cache_hit_tokens":75}}));
        }
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(report.raw_field_reports.get("input_tokens"), Some(&1));
        assert_eq!(
            report.raw_field_reports.get("prompt_cache_hit_tokens"),
            Some(&1)
        );
        assert!(!report.raw_field_reports.contains_key("prompt_tokens"));
        assert!(
            !report
                .raw_field_reports
                .contains_key("prompt_tokens_details.cached_tokens")
        );
    }
    #[test]
    fn usage_projection_http_actual_retry_and_decode_failure_close_attempts() {
        let _lock = crate::tests::env_lock();
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            for (status, body) in [
                (
                    "500 Internal Server Error",
                    r#"{"usage":{"prompt_tokens":7},"error":{"message":"fixture retry"}}"#,
                ),
                (
                    "200 OK",
                    r#"{"usage":{"completion_tokens":3},"choices":[]}"#,
                ),
                ("200 OK", "invalid json fixture"),
            ] {
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("bounded fixture accept: {error}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut request = Vec::new();
                loop {
                    let mut chunk = [0u8; 1024];
                    let size = socket.read(&mut chunk).unwrap();
                    assert!(size > 0 && request.len() + size <= 65536);
                    request.extend_from_slice(&chunk[..size]);
                    if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                        let length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse::<usize>()
                            .unwrap();
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).unwrap();
            }
        });
        let mut club = HttpClub::new("fixture", format!("http://{address}"), "fixture", None);
        club.agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(2))
            .build();
        club.policy.retries = 1;
        club.policy.backoff_base = Duration::ZERO;
        club.policy.backoff_cap = Duration::ZERO;
        let before = club.usage_accounting();
        assert!(
            club.post_chat(serde_json::json!({"model":"fixture","messages":[]}))
                .is_ok()
        );
        assert!(
            club.post_chat(serde_json::json!({"model":"fixture","messages":[]}))
                .unwrap_err()
                .contains("decode response")
        );
        server.join().unwrap();
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert_eq!(report.attempts, 3);
        assert_eq!((report.input, report.output), (Some(7), Some(3)));
        assert_eq!(
            (
                report.reported_attempts.input,
                report.reported_attempts.output
            ),
            (1, 1)
        );
        assert_eq!(report.raw_field_reports["prompt_tokens"], 1);
        assert!(!report.core_complete);
    }
}

#[cfg(test)]
#[path = "http/native_usage_contract_fixture_tests.rs"]
mod native_usage_contract_fixture_tests;

#[cfg(test)]
mod supplemental_chat_usage_fixture_tests;

#[cfg(test)]
mod trajectory_byte_tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn trajectory_c03c_attempt_counts_utf8_partial_stream_and_unknown() {
        let _lock = crate::tests::env_lock();
        let club = HttpClub::new("fixture", "http://127.0.0.1:9/v1", "fixture", None);
        crate::harness::reset_turn_ledger(&club);
        crate::harness::note_timing_origin(Instant::now());
        crate::harness::begin_model_request();
        let wire = "data: π\n\ndata: [DONE]\n\n".as_bytes();
        {
            let mut attempt = club.accounting.attempt();
            attempt.request_bytes("{\"prompt\":\"π\"}".len());
            let mut reader = attempt.response_reader(wire);
            let mut prefix = [0; 7];
            reader.read_exact(&mut prefix).unwrap();
        }
        {
            let _unobserved = club.accounting.attempt();
        }
        let samples = crate::harness::provider_call_samples();
        assert_eq!(samples[0]["request_bytes"], "{\"prompt\":\"π\"}".len());
        assert_eq!(samples[0]["response_bytes"], 7);
        assert!(samples[1]["request_bytes"].is_null());
        assert!(samples[1]["response_bytes"].is_null());
        crate::harness::end_model_request();
    }

    #[test]
    fn trajectory_c03d_usage_missing_retry_is_unreported_and_not_a_partial_sum() {
        let _lock = crate::tests::env_lock();
        let club = HttpClub::new(
            "openrouter",
            "https://api.z.ai/api/coding/paas/v4",
            "fixture",
            None,
        );
        crate::harness::reset_turn_ledger(&club);
        crate::harness::note_timing_origin(Instant::now());
        crate::harness::begin_model_request();
        let before = club.usage_accounting();
        {
            let mut missing = club.accounting.attempt();
            missing.request_bytes(300_000);
        }
        {
            let mut accounting = club.accounting.attempt();
            accounting.request_bytes(300_000);
            let mut reader = accounting.response_reader("fixture".as_bytes());
            let mut body = Vec::new();
            reader.read_to_end(&mut body).unwrap();
            let mut commit = StreamUsageCommit::from_attempt(&club, accounting);
            commit.observe(
                &serde_json::json!({"usage":{"prompt_tokens":100,"completion_tokens":5,
                "prompt_tokens_details":{"cached_tokens":40}}}),
            );
            commit.observe(&serde_json::json!({"usage":{"completion_tokens":6}}));
            commit.observe(&serde_json::json!({"usage":null}));
        }
        let samples = crate::harness::provider_call_samples();
        assert_eq!(samples[0]["accounting_status"], "unreported");
        for key in [
            "raw_input",
            "paid_input",
            "cached_input",
            "output",
            "total_tokens",
            "response_bytes",
        ] {
            assert!(samples[0]["counters"][key].is_null());
        }
        assert_eq!(samples[1]["accounting_status"], "reported");
        assert_eq!(samples[1]["retry_of"], 0);
        assert_eq!(
            samples[1]["counters"],
            serde_json::json!({"raw_input":100,"paid_input":60,
            "cached_input":40,"output":6,"generation_output":6,"total_tokens":106,"response_bytes":7})
        );
        let report = crate::harness::task_usage_delta(before, club.usage_accounting()).unwrap();
        assert!(!report.core_complete);
        // The shared UI report intentionally retains observed subtotals;
        // only the durable ledger requires complete measurement coverage.
        assert_eq!(report.total_prompt, Some(100));
        let ledger_usage = crate::harness::complete_ledger_usage(&report);
        for field in [
            "input",
            "output",
            "cache_read",
            "total_prompt",
            "generation_output",
        ] {
            assert!(
                ledger_usage[field].is_null(),
                "{field} must not be a partial sum"
            );
        }
        assert_eq!(ledger_usage["accounting_status"], "partial");
        assert_eq!(ledger_usage["reported_attempts"]["input"], 1);
        crate::harness::end_model_request();
    }

    #[test]
    fn trajectory_c03c_zai_chat_contract_is_known_on_openrouter_dialect() {
        let _lock = crate::tests::env_lock();
        for base in [
            "https://api.z.ai/api/coding/paas/v4",
            "https://openrouter.ai/api/v1",
            "http://127.0.0.1:9/v1",
        ] {
            let club = HttpClub::new("openrouter", base, "z-ai/glm-5", None);
            let usage = super::super::usage::parse_http_usage(&serde_json::json!({
                "prompt_tokens":100,"completion_tokens":12,
                "prompt_tokens_details":{"cached_tokens":40},
                "completion_tokens_details":{"reasoning_tokens":3}
            }))
            .unwrap();
            let observation = club.accounting_observation(usage);
            assert_eq!(
                observation.contract.cache,
                super::super::CacheConvention::Included
            );
            assert_eq!(
                observation.contract.reasoning,
                super::super::ReasoningConvention::Included
            );
        }
    }
}
