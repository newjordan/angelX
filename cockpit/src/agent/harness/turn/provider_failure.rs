//! One failed provider request, classified once and answered with one decision:
//! the class, whether to replay it, and how long to wait first. The native leg
//! below is the shipped policy. The live `errors` leg (`cockpit/live/errors`)
//! decides from the flags this file extracts, and may only be stricter.
use super::*;
use crate::agent::harness::live_policy::{LegSource, decide};

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

pub(crate) struct FailureContext {
    pub(crate) emitted_answer: bool,
    pub(crate) output_cap_exhausted: bool,
    pub(crate) attempt: usize,
    pub(crate) budget: Option<usize>,
    pub(crate) backoff_ms: usize,
    pub(crate) elapsed: Duration,
}

pub(crate) struct FailureDecision {
    pub(crate) verdict: FailureVerdict,
    pub(crate) leg: LegSource,
    pub(crate) declined: bool,
}

pub(crate) fn native_verdict(error: &str, ctx: &FailureContext) -> FailureVerdict {
    // A gateway reservation refusal is the credit form of an oversized
    // context: the request as built cannot be afforded, and the answer is
    // to send a smaller one. The turn does that itself, sized from the
    // amounts the gateway reports (see the reservation correction in
    // `turn/mod.rs`), and asks again only with a request it has changed.
    // So the verdict here is never "replay": the same bytes would be
    // refused again, for nothing, for as long as they were sent.
    let reservation = crate::agent::club::error_indicates_reservation_exceeds(error);
    let class = if is_context_overflow_error(error) || reservation {
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
        && !reservation
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
        "gw_stop": crate::agent::club::error_indicates_gateway_stop(error),
        "gw_retry": crate::agent::club::error_indicates_gateway_retryable(error),
        "gw_resv": crate::agent::club::error_indicates_reservation_exceeds(error),
        "context": is_context_overflow_error(error)
            || crate::agent::club::error_indicates_reservation_exceeds(error),
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

pub(crate) fn decide_failure(
    registry: &ToolRegistry,
    error: &str,
    ctx: &FailureContext,
) -> FailureDecision {
    let native = native_verdict(error, ctx);
    // A metering gateway states its recovery decision in the error body
    // (`run_action`, `retryable`, a reservation code). That is the
    // provider's own contract for this failure, not a policy preference:
    // a leg verdict that contradicts the native reading of it falls back
    // to native instead of overriding the gateway.
    let gateway_contract = crate::agent::club::error_indicates_gateway_stop(error)
        || crate::agent::club::error_indicates_gateway_retryable(error)
        || crate::agent::club::error_indicates_reservation_exceeds(error);
    let (verdict, leg) = decide(
        registry,
        "errors",
        &failure_facts(error, ctx),
        |live: &FailureVerdict| {
            within_native(live, &native)?;
            if gateway_contract && live.retry != native.retry {
                return Err(
                    "the leg overrides the gateway's stated recovery contract".into(),
                );
            }
            Ok(())
        },
        || native.clone(),
    );
    let declined = native.retry && !verdict.retry;
    FailureDecision {
        verdict,
        leg,
        declined,
    }
}

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

pub(crate) fn leg_note(leg: &LegSource) -> String {
    match leg_parts(leg) {
        (_, None) => String::new(),
        (source, Some(digest)) => format!(" [errors leg {source}:{digest}]"),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/turn__provider_failure__tests.rs"]
mod tests;
