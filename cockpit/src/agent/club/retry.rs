//! HTTP policy, env knobs, retry/backoff, and quota-exhaustion detection.

use super::*;

const DEFAULT_HTTP_READ_TIMEOUT_SECS: u64 = 60;
const DEFAULT_HTTP_RETRIES: u32 = 1;
const DEFAULT_HTTP_RATELIMIT_RETRIES: u32 = 5;
const DEFAULT_HTTP_RATELIMIT_BACKOFF_MS: u64 = 1_000;
const DEFAULT_HTTP_RATELIMIT_BACKOFF_CAP_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// HttpClub — a real OpenAI-compatible club (chat + tool calls)
// ---------------------------------------------------------------------------

/// Read an env var as whole seconds, falling back to `default_secs`.
pub(crate) fn env_secs(key: &str, default_secs: u64) -> Duration {
    let secs = std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(default_secs);
    Duration::from_secs(secs)
}

/// Read an env var as a whole `usize`, falling back to `default`.
pub(crate) fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

/// Read an env var as whole milliseconds, falling back to `default_ms`.
pub(crate) fn env_millis(key: &str, default_ms: u64) -> Duration {
    let ms = std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(default_ms);
    Duration::from_millis(ms)
}

/// Read an env var as a `u32`, falling back to `default`.
pub(crate) fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

/// Read an env var as an `f64`, falling back to `default`.
pub(crate) fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(default)
}

/// Transport policy for an [`HttpClub`] — timeouts + retry/backoff, read from the
/// environment once at construction. The defaults are liveness-bounded: a short
/// connect timeout fails fast on a host that's actually down, a one-minute read
/// timeout allows ordinary non-streamed generations, and one bounded retry rides
/// out a transient reset/503 without turning a silent provider into a many-minute
/// foreground hang.
///
/// Knobs (all optional): `ANGEL_HTTP_CONNECT_TIMEOUT`, `ANGEL_HTTP_TIMEOUT`
/// (read), `ANGEL_HTTP_WRITE_TIMEOUT` — seconds; `ANGEL_HTTP_RETRIES` — count;
/// `ANGEL_HTTP_BACKOFF_MS` — base backoff. Transient 429s have a separate,
/// longer budget because a large homogeneous swarm can briefly exhaust an
/// otherwise healthy provider's request window:
/// `ANGEL_HTTP_RATELIMIT_RETRIES`, `ANGEL_HTTP_RATELIMIT_BACKOFF_MS`, and
/// `ANGEL_HTTP_RATELIMIT_BACKOFF_CAP` (seconds). Long-window quota exhaustion
/// still fails immediately. Set both retry counts to `0` for fail-fast behavior.
#[derive(Clone, Debug)]
pub(crate) struct HttpPolicy {
    /// TCP connect deadline — fail fast on a dead/unreachable host.
    pub(crate) connect_timeout: Duration,
    /// Socket read deadline. A buffered call must finish within it; on an SSE
    /// call it bounds one blocking read. The stream layer may retry timed-out
    /// reads only for its narrow, bounded local-tool parser recovery window.
    pub(crate) read_timeout: Duration,
    /// Write deadline — large multimodal bodies (base64 images) over a slow LAN.
    pub(crate) write_timeout: Duration,
    /// Extra attempts after the first, for *retryable* failures only.
    pub(crate) retries: u32,
    /// Base backoff; the wait doubles each retry up to `backoff_cap`.
    pub(crate) backoff_base: Duration,
    /// Upper bound on any single backoff / honored `Retry-After`.
    pub(crate) backoff_cap: Duration,
    /// Extra attempts after a transient HTTP 429. Kept separate from transport
    /// and 5xx retries so a silent socket remains bounded to two read windows
    /// while a short provider request window can drain without dropping seats.
    pub(crate) rate_limit_retries: u32,
    /// Base/cap for transient-429 backoff. A wider jittered ladder prevents a
    /// whole swarm from waking in lockstep and immediately colliding again.
    pub(crate) rate_limit_backoff_base: Duration,
    pub(crate) rate_limit_backoff_cap: Duration,
    /// Jitter ratio in `[0,1]` applied to computed backoff (not `Retry-After`):
    /// the wait becomes a random draw in `[(1-r)·d, d]`. Decorrelates the swarm's
    /// many agents so they don't all wake and re-hit one endpoint in lockstep.
    /// `0` = deterministic backoff (the old behavior).
    pub(crate) jitter: f64,
}

impl HttpPolicy {
    pub(crate) fn from_env() -> Self {
        Self {
            connect_timeout: env_secs("ANGEL_HTTP_CONNECT_TIMEOUT", 10),
            read_timeout: env_secs("ANGEL_HTTP_TIMEOUT", DEFAULT_HTTP_READ_TIMEOUT_SECS),
            write_timeout: env_secs("ANGEL_HTTP_WRITE_TIMEOUT", 60),
            retries: env_u32("ANGEL_HTTP_RETRIES", DEFAULT_HTTP_RETRIES),
            backoff_base: env_millis("ANGEL_HTTP_BACKOFF_MS", 500),
            backoff_cap: Duration::from_secs(8),
            rate_limit_retries: env_u32(
                "ANGEL_HTTP_RATELIMIT_RETRIES",
                DEFAULT_HTTP_RATELIMIT_RETRIES,
            ),
            rate_limit_backoff_base: env_millis(
                "ANGEL_HTTP_RATELIMIT_BACKOFF_MS",
                DEFAULT_HTTP_RATELIMIT_BACKOFF_MS,
            ),
            rate_limit_backoff_cap: env_secs(
                "ANGEL_HTTP_RATELIMIT_BACKOFF_CAP",
                DEFAULT_HTTP_RATELIMIT_BACKOFF_CAP_SECS,
            ),
            jitter: env_f64("ANGEL_HTTP_JITTER", 0.5).clamp(0.0, 1.0),
        }
    }
}

/// HTTP status codes worth retrying: transient overload / warming / gateway
/// hiccups. Everything else (400/401/403/404/422 …) is a real client/config
/// error where retrying just wastes time, so it surfaces immediately.
pub(crate) fn is_retryable_status(code: u16) -> bool {
    matches!(code, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

/// A 429 body that signals a *long-window* quota exhaustion (a weekly/monthly
/// plan cap or spent balance) rather than a transient per-minute rate limit.
/// Retrying can't help — the window won't reset for hours or days — so the caller
/// fails fast instead of burning the retry budget and backoff sleeps on it. Only
/// consulted for 429s, where this vocabulary unambiguously means "come back
/// later", never "slow down". Example seen in the wild:
/// `{"error":{"code":"1310","message":"Weekly/Monthly Limit Exhausted..."}}`.
pub(crate) fn is_quota_exhausted(body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    b.contains("exhaust")
        || b.contains("weekly")
        || b.contains("monthly")
        || b.contains("quota")
        || b.contains("usage limit")
        || b.contains("insufficient balance")
        || b.contains("\"1310\"")
}

/// Whether a club *error string* carries a quota-exhaustion 429 (the raw body is
/// embedded in the error) or a cooldown short-circuit from [`HttpClub`]'s quota
/// gate. This is the swarm's failover trigger: exactly the errors where the link
/// is down for hours/days and another SOTA link should take the call. A plain
/// transient 429 (no exhaustion vocabulary) stays non-matching — the retry loop
/// already handled it, and failing over would double-spend on a healthy link.
pub(crate) fn error_indicates_quota_exhausted(err: &str) -> bool {
    err.contains("quota exhausted") || (err.contains("429") && is_quota_exhausted(err))
}

/// Whether an error means this provider link is unusable for the current call
/// because credentials were rejected. Unlike transient 5xx/timeout failures,
/// retrying the same link cannot fix this; a SOTA role should hand off to another
/// configured link when available. Covers HTTP auth statuses and the Z.ai/GLM
/// payload observed in production (`身份验证失败`, code 1000).
pub(crate) fn error_indicates_auth_failed(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("http 401")
        || e.contains("http 403")
        || e.contains("unauthorized")
        || e.contains("forbidden")
        || e.contains("authentication failed")
        || e.contains("auth failed")
        || e.contains("invalid api key")
        || e.contains("invalid_api_key")
        || e.contains("身份验证失败")
        || (e.contains("\"code\":\"1000\"") && e.contains("验证"))
}

/// An account/configuration failure is not a research stall. Stop the outer
/// loop before it re-arms or widens into a swarm against the same broken route.
pub(crate) fn error_requires_provider_action(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("http 402")
        || e.contains("\"billing_not_configured\"")
        || e.contains("\"billing_error\"")
        || error_indicates_auth_failed(err)
        || error_indicates_quota_exhausted(err)
}

/// How long a quota-exhausted link sits out before we let a request probe it
/// again. The provider window is typically hours-to-days; re-probing hourly is
/// one cheap failed request and self-corrects a wrong guess about the reset.
pub(crate) fn quota_cooldown_duration() -> Duration {
    env_secs("ANGEL_QUOTA_COOLDOWN_SECS", 3600)
}

const MAX_TRUNCATION_RETRIES: u32 = 4;

/// Progressive `max_tokens` asks for unusable truncated replies: 2×, 4×, 8×,
/// then 16× the cap the original request carried, each clamped to `retry_cap`.
/// Duplicate capped values are omitted, so a low provider ceiling stops the
/// ladder early instead of resending an identical request. An empty schedule
/// means retrying cannot help or has been disabled.
pub(crate) fn truncation_retry_schedule(
    sent: Option<u64>,
    enabled: bool,
    retry_cap: u64,
    retries: u32,
) -> Vec<u64> {
    if !enabled {
        return Vec::new();
    }
    let Some(sent) = sent else {
        return Vec::new();
    };
    if sent >= retry_cap {
        return Vec::new();
    }
    let mut schedule = Vec::with_capacity(retries.min(MAX_TRUNCATION_RETRIES) as usize);
    let mut previous = sent;
    for retry in 0..retries.min(MAX_TRUNCATION_RETRIES) {
        let multiplier = 1u64.checked_shl(retry + 1).unwrap_or(u64::MAX);
        let next = sent.saturating_mul(multiplier).min(retry_cap);
        if next <= previous {
            break;
        }
        schedule.push(next);
        previous = next;
    }
    schedule
}

/// [`truncation_retry_schedule`] on its env knobs: `ANGEL_TRUNCATION_RETRY`
/// (default on; `0`/`off`/`false`/`no` disables) decides whether a truncated
/// reply is retried at all, `ANGEL_TRUNCATION_RETRIES` (default/hard max 4)
/// bounds extra attempts, and `ANGEL_TRUNCATION_RETRY_MAX` (default 65536) caps
/// how high the escalated ask may go.
pub(crate) fn truncation_retry_schedule_from_env(sent: Option<u64>) -> Vec<u64> {
    let enabled = !std::env::var("ANGEL_TRUNCATION_RETRY")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "off" | "false" | "no"
            )
        })
        .unwrap_or(false);
    let retries = env_u32("ANGEL_TRUNCATION_RETRIES", MAX_TRUNCATION_RETRIES);
    let retry_cap = std::env::var("ANGEL_TRUNCATION_RETRY_MAX")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(65536);
    truncation_retry_schedule(sent, enabled, retry_cap, retries)
}

/// Whether a reasoning-only reply (private reasoning, no visible answer) earns
/// its single direct-answer retry. `ANGEL_REASONING_ONLY_RETRY` defaults on;
/// `0`/`off`/`false`/`no` disables it. The retry itself is always depth-one —
/// a second reasoning-only reply fails closed with the original error.
pub(crate) fn reasoning_only_retry_enabled() -> bool {
    !std::env::var("ANGEL_REASONING_ONLY_RETRY")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "off" | "false" | "no"
            )
        })
        .unwrap_or(false)
}

/// Backoff for retry `n` (0-based): `base * 2^n`, clamped to `cap`. Computed in
/// `u128` millis and saturating, so a large `n` or `base` can never overflow —
/// it just pins to `cap`.
pub(crate) fn backoff_delay(base: Duration, cap: Duration, n: u32) -> Duration {
    let factor: u128 = 1u128.checked_shl(n).unwrap_or(u128::MAX);
    let scaled = base.as_millis().saturating_mul(factor).min(cap.as_millis());
    Duration::from_millis(scaled as u64)
}

/// A pseudo-random fraction in `[0,1)` for backoff jitter. Seeded from the wall
/// clock's sub-nanos run through the splitmix64 finalizer — not crypto, just
/// enough entropy to decorrelate retries across the swarm's many agents/processes
/// (each calls at a slightly different instant). No `rand` dependency.
pub(crate) fn jitter_fraction() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Apply equal jitter to a deterministic backoff: keep `(1-ratio)` of it as a
/// floor and randomize the rest, so the result lands in `[(1-ratio)·d, d]`.
/// Bounded by the original `d` (so the `cap` still holds) with a floor (so
/// connection-refused retries never busy-spin). `ratio <= 0` is the identity —
/// keeps the deterministic path testable. Split from [`backoff_delay`] so that
/// function stays pure for its exact-value test.
pub(crate) fn jittered(delay: Duration, ratio: f64) -> Duration {
    if ratio <= 0.0 {
        return delay;
    }
    let ratio = ratio.min(1.0);
    let ms = delay.as_millis() as f64;
    let floor = ms * (1.0 - ratio);
    let span = ms * ratio;
    Duration::from_millis((floor + span * jitter_fraction()) as u64)
}

/// Parse a `Retry-After` header in delta-seconds form into a Duration, clamped
/// to `cap`. The HTTP-date form is intentionally ignored (returns `None`) — we
/// fall back to computed backoff for that rarer case.
pub(crate) fn retry_after(header: Option<&str>, cap: Duration) -> Option<Duration> {
    let secs: u64 = header?.trim().parse().ok()?;
    Some(Duration::from_secs(secs).min(cap))
}

/// Parse an OpenAI-style reset value into a `Duration`: bare seconds (`"6"`), or
/// Go-duration suffix forms (`"20ms"`, `"1s"`, `"6m0s"`, `"1h2m3s"`). The reset
/// headers come in this mixed shape across providers.
pub(crate) fn parse_reset_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(secs) = s.parse::<f64>() {
        // A hostile/broken upstream can send `inf`, `1e400` (parses to +inf), or a
        // huge finite value; `Duration::from_secs_f64` PANICS on non-finite or
        // overflowing input. `try_from_secs_f64` returns Err there, and the caller
        // already falls back to a conservative 1s when this yields `None`.
        return Duration::try_from_secs_f64(secs.max(0.0)).ok();
    }
    let bytes = s.as_bytes();
    let mut total = 0f64;
    let mut num = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            i += 1;
        } else {
            let mut unit = String::new();
            while i < bytes.len() && (bytes[i] as char).is_ascii_alphabetic() {
                unit.push(bytes[i] as char);
                i += 1;
            }
            let val: f64 = num.parse().ok()?;
            num.clear();
            total += match unit.as_str() {
                "h" => val * 3600.0,
                "m" => val * 60.0,
                "s" => val,
                "ms" => val / 1000.0,
                _ => return None,
            };
        }
    }
    if !num.is_empty() {
        return None; // a trailing number with no unit is malformed
    }
    // Same guard as the bare-seconds branch: a very long digit run can push
    // `total` to +inf, which would panic `from_secs_f64`.
    Duration::try_from_secs_f64(total).ok()
}

/// Pull `(requests_remaining, window_reset)` from the standard OpenAI rate-limit
/// header pair (`x-ratelimit-remaining-requests` + `x-ratelimit-reset-requests`).
/// `get` is a header lookup, so this stays pure and testable. Missing reset → a
/// conservative 1s. `None` when the remaining header is absent (most local vLLM
/// servers send nothing — proactive backoff then simply never engages).
pub(crate) fn parse_rate_limit_headers(
    get: impl Fn(&str) -> Option<String>,
) -> Option<(u64, Duration)> {
    let remaining =
        get("x-ratelimit-remaining-requests").and_then(|v| v.trim().parse::<u64>().ok())?;
    let reset = get("x-ratelimit-reset-requests")
        .and_then(|v| parse_reset_duration(&v))
        .unwrap_or_else(|| Duration::from_secs(1));
    Some((remaining, reset))
}

/// Some OpenAI-compatible servers answer `200 OK` with an `{"error": …}` body
/// instead of an error status. Pull a human message out of such an envelope so
/// it surfaces, instead of being parsed as an empty (blank) reply.
pub(crate) fn extract_api_error(v: &serde_json::Value) -> Option<String> {
    let e = v.get("error")?;
    if let Some(msg) = e.get("message").and_then(|m| m.as_str()) {
        Some(msg.to_string())
    } else if let Some(s) = e.as_str() {
        Some(s.to_string())
    } else {
        Some(e.to_string())
    }
}

/// Sleep `total`, waking every 100ms to honor a user interrupt. Backoff and
/// proactive rate-limit waits used to be opaque `thread::sleep`s — an Esc during
/// a 30s wait did nothing until the sleep ended and the turn felt hung.
pub(crate) fn cancellable_sleep(total: Duration, cancel: Option<&std::sync::atomic::AtomicBool>) {
    let total = crate::agent::harness::formation_budget::request_wall_remaining()
        .map(|remaining| remaining.min(total))
        .unwrap_or(total);
    let Some(cancel) = cancel else {
        std::thread::sleep(total);
        return;
    };
    let slice = Duration::from_millis(100);
    let deadline = std::time::Instant::now() + total;
    while std::time::Instant::now() < deadline {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(
            slice.min(deadline.saturating_duration_since(std::time::Instant::now())),
        );
    }
}

/// Compact a JSON value to a single-line string capped at `max` chars, for error
/// messages (so a giant/garbled body can't flood the UI).
pub(crate) fn truncate_json(v: &serde_json::Value, max: usize) -> String {
    let s = v.to_string();
    if s.chars().count() > max {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    } else {
        s
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/retry__reset_duration_stress.rs"]
mod reset_duration_stress;

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/retry__liveness_defaults.rs"]
mod liveness_defaults;
