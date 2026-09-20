use super::*;
/// Prefix for harness-authored runtime commentary (error/spin/churn/false-start
/// nudges) that is pushed into live history. Live turns see these messages in the
/// tail, but the compaction summarizer must not — transient failure chatter must
/// never become a durable "open thread" that later reads as evidence the harness
/// is broken. [`render_transcript`] drops any message whose (whitespace-trimmed)
/// content starts with this mark. Baked into the nudge consts below so the pushed
/// message carries it; the tests pin every nudge string against it.
pub(crate) const TELEMETRY_MARK: &str = "[harness-telemetry] ";
const SOTA_INPUT_MILESTONE_TOKENS: u64 = 1_000_000;

/// Return the newest whole-million boundary crossed by a metered turn.
///
/// Large provider-side fan-out can jump across several boundaries in one
/// request. Emit one current receipt instead of flooding the display queue.
pub(crate) fn crossed_sota_input_milestone(previous: u64, current: u64) -> Option<u64> {
    let previous_millions = previous / SOTA_INPUT_MILESTONE_TOKENS;
    let current_millions = current / SOTA_INPUT_MILESTONE_TOKENS;
    (current_millions > previous_millions)
        .then(|| current_millions.saturating_mul(SOTA_INPUT_MILESTONE_TOKENS))
}

/// Resolve the per-hop reasoning effort under adaptive reasoning mode (`ANGEL_ADAPTIVE_REASONING=1`).
///
/// - Hop 0 (Turn intake & architectural planning): use configured/highest baseline effort.
/// - Hops > 0 under normal execution (clean tool return, no error/spin): downgrade to lowest
///   viable effort (e.g. "low" or "none") to eliminate thinking latency during mechanical tool chains.
/// - Friction / Error hops (tool errors, spin nudges, action starvation): escalate back to
///   high baseline effort for root-cause diagnosis.
/// - Locked-thinking GLM-5.3(*) seats are NOT auto-armed (they were until
///   2026-09-05): z.ai keys its prefix cache on `reasoning_effort`, so a rung
///   that changes between hops re-prefills the whole prompt every flip. Those
///   seats hold one rung per turn — the idle rung (Flash: field omitted =
///   provider `auto`; glm-5.3: `low`) or the operator pin — unless the operator
///   arms this policy explicitly.
pub(crate) fn resolve_adaptive_reasoning_effort(
    club: &dyn Club,
    hop: usize,
    has_friction: bool,
) -> Option<String> {
    let levels = club.reasoning_levels();
    if levels.is_empty() {
        return None;
    }
    let base_effort = club.reasoning_effort().unwrap_or_else(|| {
        levels
            .iter()
            .find(|l| l.eq_ignore_ascii_case("high") || l.eq_ignore_ascii_case("max"))
            .cloned()
            .unwrap_or_else(|| levels.last().cloned().unwrap_or_default())
    });

    if hop == 0 || has_friction {
        Some(base_effort)
    } else if levels.iter().any(|l| l.eq_ignore_ascii_case("low")) {
        Some("low".to_string())
    } else if levels.iter().any(|l| l.eq_ignore_ascii_case("none")) {
        Some("none".to_string())
    } else {
        levels.first().cloned()
    }
}

/// Opt-in reasoning-budget policy for locked-thinking seats (GLM-5.3*: thinking
/// cannot be disabled, and an open-ended `high` hop can burn the whole
/// max_tokens budget on reasoning alone — measured 2026-08-28: 4096/4096
/// reasoning tokens on a single uncapped hop). Returns the demotion effort
/// for the next hop once cumulative reasoning burn crosses `burn_limit`
/// without a submit. `0` (the default) disables it. Non-locked seats return `None`:
/// everywhere else the operator can just turn thinking off.
pub(crate) fn glm_thinking_burn_demotion(
    club: &dyn Club,
    cumulative_reasoning: u64,
    submitted: bool,
    burn_limit: u64,
) -> Option<String> {
    if burn_limit == 0 || submitted || cumulative_reasoning < burn_limit {
        return None;
    }
    if !crate::agent::club::glm_thinking_locked_for_club(&club.model_identity().unwrap_or_default())
    {
        return None;
    }
    club.reasoning_levels()
        .iter()
        .find(|l| l.eq_ignore_ascii_case("low"))
        .cloned()
        .or_else(|| club.reasoning_levels().first().cloned())
}

/// Whether a turn has exceeded its wall-clock budget. `budget_secs == 0` means
/// "off" (the default) — interactive extended-reasoning turns stay unbounded.
pub(crate) fn turn_expired(elapsed_secs: u64, budget_secs: usize) -> bool {
    budget_secs > 0 && elapsed_secs >= budget_secs as u64
}
