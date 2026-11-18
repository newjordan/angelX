use super::*;
pub(crate) const TELEMETRY_MARK: &str = "[harness-telemetry] ";
const SOTA_INPUT_MILESTONE_TOKENS: u64 = 1_000_000;

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

fn milestone_check(milestone: &Milestone) -> Result<(), String> {
    match milestone.tokens {
        Some(tokens) if tokens == 0 || tokens % SOTA_INPUT_MILESTONE_TOKENS != 0 => {
            Err(format!("{tokens} is not a whole-million boundary"))
        }
        _ => Ok(()),
    }
}

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

#[cfg(test)]
pub(crate) use self::tests::{
    crossed_sota_input_milestone, resolve_adaptive_reasoning_effort, turn_expired,
};

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/reasoning_leg__tests.rs"]
mod tests;
