//! One failed provider request, classified once and answered with one decision:
//! the class, whether to replay it, and how long to wait first. The native leg
//! below is the shipped policy. The live `errors` leg (`cockpit/live/errors`)
//! decides from the flags this file extracts, and may only be stricter.
use super::*;
use crate::agent::harness::live_policy::{LegSource, decide};

/// Longest wait between two provider replays. A live verdict may not exceed it.
pub(crate) const MAX_RETRY_DELAY_MS: u32 = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureClass {
    Permanent,
    Transient,
    Quota,
    Context,
}

impl FailureClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::Transient => "transient",
            Self::Quota => "quota",
            Self::Context => "context",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FailureVerdict {
    pub(crate) class: FailureClass,
    pub(crate) retry: bool,
    pub(crate) delay_ms: u32,
}

/// What the turn knows about the failed attempt besides its error text.
pub(crate) struct FailureContext {
    pub(crate) emitted_answer: bool,
    pub(crate) output_cap_exhausted: bool,
    /// Replays already spent on this hop.
    pub(crate) attempt: usize,
    /// `ANGEL_PROVIDER_RETRIES`; `None` is unbounded.
    pub(crate) budget: Option<usize>,
    pub(crate) backoff_ms: usize,
    pub(crate) elapsed: Duration,
}

/// The decision, which leg made it, and whether a live leg refused a replay the
/// native policy would have made.
pub(crate) struct FailureDecision {
    pub(crate) verdict: FailureVerdict,
    pub(crate) leg: LegSource,
    pub(crate) declined: bool,
}

/// The shipped policy: the classification and retry gates the turn used before
/// the decision became a leg.
pub(crate) fn native_verdict(error: &str, ctx: &FailureContext) -> FailureVerdict {
    let class = if is_context_overflow_error(error) {
        FailureClass::Context
    } else if crate::agent::club::error_indicates_quota_exhausted(error) {
        FailureClass::Quota
    } else if is_permanent_provider_error(error) {
        FailureClass::Permanent
    } else {
        FailureClass::Transient
    };
    let retry = retryable_provider_failure(
        error,
        ctx.emitted_answer,
        is_recoverable_stream_error(error),
    ) && !ctx.output_cap_exhausted
        && retry_budget_allows(ctx.budget, ctx.attempt);
    let delay_ms = if retry {
        ctx.backoff_ms
            .saturating_mul(ctx.attempt.saturating_add(1))
            .min(MAX_RETRY_DELAY_MS as usize) as u32
    } else {
        0
    };
    FailureVerdict {
        class,
        retry,
        delay_ms,
    }
}

fn shape(error: &str, status: Option<u16>) -> &'static str {
    if is_recoverable_stream_error(error) {
        "stream"
    } else if is_empty_reply_error(error) {
        "empty"
    } else if error.starts_with("transport error after ") {
        "transport"
    } else if status.is_some() {
        "status"
    } else {
        "other"
    }
}

/// Everything the leg may read. Text is matched here; the leg sees flags and
/// numbers only.
pub(crate) fn failure_facts(error: &str, ctx: &FailureContext) -> serde_json::Value {
    let lower = error.to_ascii_lowercase();
    let status = provider_error_status(&lower);
    let whole = |value: usize, ceiling: u32| value.min(ceiling as usize) as u32;
    serde_json::json!({
        "kind": shape(error, status),
        "status": status.unwrap_or(0),
        "veto": carries_provider_veto(error),
        "config": carries_permanent_transport_configuration(error),
        "cert": crate::agent::club::error_indicates_certificate_rejected(error),
        "auth": crate::agent::club::error_indicates_auth_failed(error),
        "quota": crate::agent::club::error_indicates_quota_exhausted(error),
        "rejected": rejected_request_text(&lower),
        "context": is_context_overflow_error(error),
        "answered": ctx.emitted_answer,
        "capped": ctx.output_cap_exhausted,
        "attempt": whole(ctx.attempt, u32::MAX - 1),
        "bounded": ctx.budget.is_some(),
        "budget": whole(ctx.budget.unwrap_or(0), u32::MAX),
        "backoff_ms": whole(ctx.backoff_ms, u32::MAX),
        "cap_ms": MAX_RETRY_DELAY_MS,
        "elapsed_ms": ctx.elapsed.as_millis().min(u32::MAX as u128) as u32,
    })
}

/// A live verdict is accepted only when it asks for no more than the native one.
fn within_native(live: &FailureVerdict, native: &FailureVerdict) -> Result<(), String> {
    if live.retry && !native.retry {
        return Err("the leg replays a failure the native policy stops on".into());
    }
    if live.delay_ms > MAX_RETRY_DELAY_MS {
        return Err(format!(
            "the leg waits {} ms past the {MAX_RETRY_DELAY_MS} ms cap",
            live.delay_ms
        ));
    }
    if live.retry && matches!(live.class, FailureClass::Permanent | FailureClass::Quota) {
        return Err("the leg replays a failure it classed as unrecoverable".into());
    }
    if live.class == FailureClass::Context && native.class != FailureClass::Context {
        return Err("the leg names a context overflow the flags do not show".into());
    }
    Ok(())
}

/// Classify and decide once for this failed attempt, through the turn's pinned
/// `errors` leg when there is one.
pub(crate) fn decide_failure(
    registry: &ToolRegistry,
    error: &str,
    ctx: &FailureContext,
) -> FailureDecision {
    let native = native_verdict(error, ctx);
    let (verdict, leg) = decide(
        registry,
        "errors",
        &failure_facts(error, ctx),
        |live: &FailureVerdict| within_native(live, &native),
        || native.clone(),
    );
    let declined = native.retry && !verdict.retry;
    FailureDecision {
        verdict,
        leg,
        declined,
    }
}

/// `LegSource::as_str()` and the first eight characters of the generation digest.
pub(crate) fn leg_parts(leg: &LegSource) -> (&'static str, Option<&str>) {
    match leg {
        LegSource::Native => (leg.as_str(), None),
        LegSource::Live { digest }
        | LegSource::Shipped { digest }
        | LegSource::LiveFallback { digest, .. } => {
            (leg.as_str(), Some(&digest[..digest.len().min(8)]))
        }
    }
}

/// Receipt suffix for a decision a live generation touched; native adds nothing.
pub(crate) fn leg_note(leg: &LegSource) -> String {
    match leg_parts(leg) {
        (_, None) => String::new(),
        (source, Some(digest)) => format!(" [errors leg {source}:{digest}]"),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/turn__provider_failure__tests.rs"]
mod tests;
