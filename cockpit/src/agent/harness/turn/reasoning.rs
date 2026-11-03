use super::*;
/// Prefix for harness-authored runtime commentary (error/spin/churn/false-start
/// nudges) that is pushed into live history. Live turns see these messages in the
/// tail, but the compaction summarizer must not — transient failure chatter must
/// never become a durable "open thread" that later reads as evidence the harness
/// is broken. [`render_transcript`] drops any message whose (whitespace-trimmed)
/// content starts with this mark. Baked into the nudge consts below so the pushed
/// message carries it; the tests pin every nudge string against it.
pub(crate) const TELEMETRY_MARK: &str = "[harness-telemetry] ";
/// The boundary a metered-input receipt reports: whole millions of tokens.
const SOTA_INPUT_MILESTONE_TOKENS: u64 = 1_000_000;

// -- The authored leg -------------------------------------------------------
//
// Every decision below is the `reasoning` leg: `cockpit/live/reasoning`
// (`reasoning.bend`) when a hot generation is active, else the generation
// compiled into this binary. The Rust ladder that used to answer these is the
// parity oracle in this module's tests and nothing else. Rust keeps only the
// text work (which model identities are GLM-locked) and hands the leg numbers,
// booleans and the seat's own level list.

/// The shipped Bend leg that answers every decision below.
const LEG: &str = "reasoning";

#[derive(serde::Deserialize)]
struct Choice {
    effort: Option<String>,
}

#[derive(serde::Deserialize)]
struct Milestone {
    tokens: Option<u64>,
}

#[derive(serde::Deserialize)]
struct Budget {
    expired: bool,
}

/// The contract a milestone answer must satisfy: `None` or a whole-million
/// boundary.
fn milestone_check(milestone: &Milestone) -> Result<(), String> {
    match milestone.tokens {
        Some(tokens) if tokens == 0 || tokens % SOTA_INPUT_MILESTONE_TOKENS != 0 => {
            Err(format!("{tokens} is not a whole-million boundary"))
        }
        _ => Ok(()),
    }
}

/// The contract an effort answer must satisfy: `None` or a level this seat
/// accepts. `pin` is the configured wire spelling the ladder passes through
/// untouched, admitted only on the hops that can answer it (intake and
/// friction), so a clean hop cannot pin itself there.
fn effort_check<'a>(
    levels: &'a [String],
    pin: Option<&'a str>,
) -> impl Fn(&Choice) -> Result<(), String> + 'a {
    move |choice| match choice.effort.as_deref() {
        Some(effort)
            if Some(effort) != pin
                && !levels.iter().any(|level| level.eq_ignore_ascii_case(effort)) =>
        {
            Err(format!("{effort} is not an effort this seat accepts"))
        }
        _ => Ok(()),
    }
}

/// The adaptive ladder through the leg. A seat with no effort levels is never
/// consulted: no rung exists to choose.
pub(crate) fn adaptive_effort(club: &dyn Club, hop: usize, has_friction: bool) -> Option<String> {
    let levels = club.reasoning_levels();
    if levels.is_empty() {
        return None;
    }
    let pin = club.reasoning_effort();
    let intake = hop == 0 || has_friction;
    let input = serde_json::json!({
        "op": "ladder",
        "levels": levels,
        "effort": pin,
        "hop": hop,
        "friction": has_friction,
    });
    let (choice, _source) = crate::agent::harness::live_policy::decide_shipped(
        LEG,
        &input,
        effort_check(levels, intake.then_some(pin.as_deref()).flatten()),
    );
    choice.effort
}

/// The locked-seat burn demotion through the leg. Which model identities are
/// GLM-locked is text work and stays here as one boolean feature.
pub(crate) fn burn_demotion(
    club: &dyn Club,
    cumulative_reasoning: u64,
    submitted: bool,
    burn_limit: u64,
) -> Option<String> {
    let levels = club.reasoning_levels();
    if levels.is_empty() {
        return None;
    }
    let locked = crate::agent::club::glm_thinking_locked_for_club(
        &club.model_identity().unwrap_or_default(),
    );
    let input = serde_json::json!({
        "op": "burn",
        "levels": levels,
        "cumulative": cumulative_reasoning,
        "submitted": submitted,
        "limit": burn_limit,
        "locked": locked,
    });
    let (choice, _source) =
        crate::agent::harness::live_policy::decide_shipped(LEG, &input, effort_check(levels, None));
    choice.effort
}

/// The metered-input milestone through the leg.
pub(crate) fn input_milestone(previous: u64, current: u64) -> Option<u64> {
    let input = serde_json::json!({
        "op": "milestone",
        "previous": previous,
        "current": current,
    });
    let (milestone, _source) =
        crate::agent::harness::live_policy::decide_shipped(LEG, &input, milestone_check);
    milestone.tokens
}

/// The turn's wall-clock budget through the leg. Both booleans are in contract,
/// so the check admits either answer.
pub(crate) fn budget_expired(elapsed_secs: u64, budget_secs: usize) -> bool {
    let input = serde_json::json!({
        "op": "budget",
        "elapsed": elapsed_secs,
        "budget": budget_secs,
    });
    let (answer, _source) =
        crate::agent::harness::live_policy::decide_shipped(LEG, &input, |_: &Budget| Ok(()));
    answer.expired
}

/// The native ladder the shipped leg must reproduce; other test modules still
/// ask it directly for their own ladders.
#[cfg(test)]
pub(crate) use self::tests::{
    crossed_sota_input_milestone, resolve_adaptive_reasoning_effort, turn_expired,
};

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/reasoning_leg__tests.rs"]
mod tests;
