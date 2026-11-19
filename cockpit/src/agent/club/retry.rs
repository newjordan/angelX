//! HTTP policy, env knobs, retry/backoff, and quota-exhaustion detection.

use super::*;

const DEFAULT_HTTP_READ_TIMEOUT_SECS: u64 = 60;
const DEFAULT_HTTP_RETRIES: u32 = 1;
const DEFAULT_HTTP_RATELIMIT_RETRIES: u32 = 5;
const DEFAULT_HTTP_RATELIMIT_BACKOFF_MS: u64 = 1_000;
const DEFAULT_HTTP_RATELIMIT_BACKOFF_CAP_SECS: u64 = 30;

thread_local! {
    static READ_FLOOR: std::cell::Cell<Option<Duration>> = const { std::cell::Cell::new(None) };
}

pub(crate) fn with_read_timeout_at_least<T>(floor: Duration, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Duration>);
    impl Drop for Restore {
        fn drop(&mut self) {
            READ_FLOOR.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(READ_FLOOR.with(|slot| slot.replace(Some(floor))));
    f()
}

pub(crate) fn read_timeout_on_this_thread(policy: Duration) -> Duration {
    match READ_FLOOR.with(std::cell::Cell::get) {
        Some(floor) if !policy.is_zero() => policy.max(floor),
        _ => policy,
    }
}

pub(crate) fn env_secs(key: &str, default_secs: u64) -> Duration {
    let secs = std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(default_secs);
    Duration::from_secs(secs)
}

pub(crate) fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

pub(crate) fn env_millis(key: &str, default_ms: u64) -> Duration {
    let ms = std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(default_ms);
    Duration::from_millis(ms)
}

pub(crate) fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

pub(crate) fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(default)
}

#[derive(Clone, Debug)]
pub(crate) struct HttpPolicy {
    pub(crate) connect_timeout: Duration,
    pub(crate) read_timeout: Duration,
    pub(crate) write_timeout: Duration,
    pub(crate) retries: u32,
    pub(crate) backoff_base: Duration,
    pub(crate) backoff_cap: Duration,
    pub(crate) rate_limit_retries: u32,
    pub(crate) rate_limit_backoff_base: Duration,
    pub(crate) rate_limit_backoff_cap: Duration,
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

pub(crate) const PROVIDER_RETRY_VETO: &str = "[x-should-retry: false]";

pub(crate) const PERMANENT_TRANSPORT_CONFIGURATION: &str = "[permanent transport configuration]";

pub(crate) fn is_permanent_transport_configuration(kind: ureq::ErrorKind) -> bool {
    matches!(
        kind,
        ureq::ErrorKind::InvalidUrl
            | ureq::ErrorKind::UnknownScheme
            | ureq::ErrorKind::InsecureRequestHttpsOnly
            | ureq::ErrorKind::InvalidProxyUrl
            | ureq::ErrorKind::ProxyUnauthorized
    )
}

pub(crate) fn provider_vetoes_retry(header: Option<&str>) -> bool {
    header.is_some_and(|value| value.trim().eq_ignore_ascii_case("false"))
}

pub(crate) fn provider_status_label(code: u16, vetoed: bool) -> String {
    if vetoed {
        format!("HTTP {code} {PROVIDER_RETRY_VETO}")
    } else {
        format!("HTTP {code}")
    }
}

pub(crate) fn is_retryable_status(code: u16) -> bool {
    matches!(code, 408 | 425 | 429 | 500 | 502 | 503 | 504 | 529)
}

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

pub(crate) fn error_indicates_quota_exhausted(err: &str) -> bool {
    err.contains("quota exhausted") || (err.contains("429") && is_quota_exhausted(err))
}

/// The provider declined the request itself (a Messages `refusal` stop, as
/// rendered by the Anthropic wire). Distinct from transport or quota faults:
/// the same request sent again earns the same refusal.
pub(crate) fn error_indicates_refusal(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("stop_reason refusal")
        || e.contains("stop_reason\":\"refusal\"")
        || e.contains("\"type\":\"refusal\"")
        || e.contains("\"type\": \"refusal\"")
}

/// A metering gateway's recovery contract, carried in its error body:
/// `run_action` says whether the run may go on at all, `retryable` whether
/// the same request may be sent again, and a reservation code says the
/// request's worst-case hold exceeds the credit left. That last one is
/// correctable only by sending a smaller request: the turn compacts the
/// conversation with the agent's own compactor, in the open, and the seat
/// lowers its own output limit. The same bytes are never sent twice.
pub(crate) fn error_indicates_gateway_stop(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("\"run_action\":\"stop\"") || e.contains("\"run_action\": \"stop\"")
}

pub(crate) fn error_indicates_gateway_retryable(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("\"retryable\":true") || e.contains("\"retryable\": true")
}

/// True for a reservation refusal the gateway says waiting will not fix:
/// the request itself has to change. Matched on the gateway's error code
/// (`…_reservation_…`), not on the word appearing somewhere in prose.
pub(crate) fn error_indicates_reservation_exceeds(err: &str) -> bool {
    (reservation_refusal(err).is_some() || err.to_ascii_lowercase().contains("_reservation_"))
        && !error_indicates_gateway_stop(err)
        && !error_indicates_gateway_retryable(err)
}

/// Appended by the seat to a reservation refusal when it lowered its own
/// output limit for the next send. With a smaller conversation, this is the
/// only other way a corrected request differs from the one refused.
pub(crate) const RESERVATION_LIMIT_LOWERED: &str = "[output limit lowered to ";

pub(crate) fn error_notes_lower_output_limit(err: &str) -> bool {
    err.contains(RESERVATION_LIMIT_LOWERED)
}

/// What a metering gateway reported when it would not hold credit for a
/// request. The request was not sent upstream and cost nothing; the numbers
/// say how far it has to shrink before it can be.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReservationRefusal {
    /// Credit the run can still commit, in the gateway's currency.
    pub(crate) available: f64,
    /// Credit the refused request's worst case would hold.
    pub(crate) required: f64,
    /// The worst-case size the gateway priced (its input bound plus the
    /// output limit), when it says.
    pub(crate) reserved_tokens: Option<u64>,
    /// The gateway says the same request can be admitted once other calls
    /// settle. Such a refusal is waited out, never shrunk.
    pub(crate) retryable: bool,
}

/// Share of the credit left that a corrected request is sized to hold. The
/// rest is room for the conversation to grow and for the credit to fall
/// before the next refusal: each correction rewrites the provider's cache,
/// so a few deep cuts cost less than many shallow ones.
const RESERVATION_FIT_SHARE: f64 = 0.5;
const RESERVATION_MIN_KEEP: f64 = 0.05;
const RESERVATION_OUTPUT_FLOOR: u64 = 8192;

fn reservation_fit_share() -> f64 {
    std::env::var("ANGEL_RESERVATION_FIT_SHARE")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|share| share.is_finite() && *share > 0.0 && *share <= 1.0)
        .unwrap_or(RESERVATION_FIT_SHARE)
}

/// Lowest output limit a reservation refusal may push a seat down to. The
/// output limit is the small term of a reservation; below this, replies are
/// cut mid tool call and the request is paid for twice.
pub(crate) fn reservation_output_floor() -> u64 {
    std::env::var("ANGEL_RESERVATION_OUTPUT_FLOOR")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|floor| *floor > 0)
        .unwrap_or(RESERVATION_OUTPUT_FLOOR)
}

/// Reads a reservation refusal out of a provider error (`HTTP 400: {json}`).
/// `None` unless the body carries a reservation code and both amounts.
pub(crate) fn reservation_refusal(err: &str) -> Option<ReservationRefusal> {
    let start = err.find('{')?;
    let body = serde_json::Deserializer::from_str(&err[start..])
        .into_iter::<serde_json::Value>()
        .next()?
        .ok()?;
    // The gateway repeats its diagnostic at the top level and under
    // `diagnostic`; some relays nest it under `error`.
    let layers = [Some(&body), body.get("diagnostic"), body.get("error")];
    let field = |key: &str| {
        layers
            .iter()
            .flatten()
            .find_map(|layer| layer.get(key).filter(|value| !value.is_null()))
    };
    let code = field("code")?.as_str()?.to_ascii_lowercase();
    if !code.contains("reservation") {
        return None;
    }
    let amount = |key: &str| {
        field(key)
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0)
    };
    let required = amount("required_reservation_usd").filter(|value| *value > 0.0)?;
    Some(ReservationRefusal {
        available: amount("available_usd")?,
        required,
        reserved_tokens: field("reserved_tokens").and_then(serde_json::Value::as_u64),
        retryable: field("retryable")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

impl ReservationRefusal {
    /// How much of the refused request's size the corrected one may keep so
    /// that its hold fits inside the fit share of the credit left. A
    /// reservation is linear in input size and output limit, so scaling both
    /// by this fraction fits whatever the two prices are.
    pub(crate) fn keep_fraction(&self) -> f64 {
        (reservation_fit_share() * self.available / self.required).clamp(RESERVATION_MIN_KEEP, 1.0)
    }

    /// The output limit to ask for after a refusal of one that asked `sent`.
    pub(crate) fn output_limit(&self, sent: u64, floor: u64) -> u64 {
        let scaled = (sent as f64 * self.keep_fraction()) as u64;
        scaled.max(floor).min(sent)
    }

    /// The conversation budget (estimated tokens, tools included) for the
    /// corrected request, given the refused one measured `estimated`.
    pub(crate) fn context_budget(&self, estimated: usize) -> usize {
        ((estimated as f64 * self.keep_fraction()) as usize).max(256)
    }

    /// Whether this refusal is for a smaller request than `earlier`. A
    /// correction that did not shrink the request has nothing left to give:
    /// sending it again would be refused again, for ever.
    pub(crate) fn smaller_than(&self, earlier: &Self) -> bool {
        match (self.reserved_tokens, earlier.reserved_tokens) {
            (Some(now), Some(before)) => now < before,
            _ => self.required < earlier.required,
        }
    }
}

pub(crate) fn error_indicates_certificate_rejected(err: &str) -> bool {
    err.contains("invalid peer certificate")
}

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

pub(crate) fn error_requires_provider_action(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("http 402")
        || e.contains("\"billing_not_configured\"")
        || e.contains("\"billing_error\"")
        || error_indicates_auth_failed(err)
        || error_indicates_quota_exhausted(err)
}

pub(crate) fn quota_cooldown_duration() -> Duration {
    env_secs("ANGEL_QUOTA_COOLDOWN_SECS", 3600)
}

const MAX_TRUNCATION_RETRIES: u32 = 4;

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

pub(crate) fn backoff_delay(base: Duration, cap: Duration, n: u32) -> Duration {
    let factor: u128 = 1u128.checked_shl(n).unwrap_or(u128::MAX);
    let scaled = base.as_millis().saturating_mul(factor).min(cap.as_millis());
    Duration::from_millis(scaled as u64)
}

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

pub(crate) fn retry_after(header: Option<&str>, cap: Duration) -> Option<Duration> {
    let secs: u64 = header?.trim().parse().ok()?;
    Some(Duration::from_secs(secs).min(cap))
}

pub(crate) fn parse_reset_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(secs) = s.parse::<f64>() {
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
        return None;
    }
    Duration::try_from_secs_f64(total).ok()
}

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

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/retry__reservation_tests.rs"]
mod reservation_tests;
