//! ⡪ (dots 2-4-6-7) — the loop ledgers' overflow shelf: research. The
//! Sloptomizer's lifecycle (`loop_research`: the Pareto selector, the UCB
//! bandit and the MicroLearner) and `rl_campaign`'s verdicts, as the routes a
//! detector raises when the model's best next step changes: a candidate that
//! passed or beat its baseline, a run that failed, finished unverified or
//! lost its baseline, advice that could not learn or has nothing to learn
//! from yet, a stall with research idle, a campaign that settled.
//!
//! The detectors are pure functions of the run's record and the campaign's
//! status ([`research`], [`campaign`], [`cold_advice`]); the routes ride in
//! the record's `warpath` field, its facts beside them.

use super::{Primary, Route, Sub};
use serde_json::Value;

pub(crate) const CELL: char = '⡪';
pub(crate) const PASSED: Route = Route::new(CELL, '⠁');
pub(crate) const WON: Route = Route::new(CELL, '⠃');
pub(crate) const FAILED: Route = Route::new(CELL, '⠉');
pub(crate) const UNVERIFIED: Route = Route::new(CELL, '⠙');
pub(crate) const BASELINE_RED: Route = Route::new(CELL, '⠑');
pub(crate) const LEARNING_ERROR: Route = Route::new(CELL, '⠋');
pub(crate) const LIVE: Route = Route::new(CELL, '⠛');
pub(crate) const ADVICE: Route = Route::new(CELL, '⠓');
pub(crate) const STALL: Route = Route::new(CELL, '⠊');
pub(crate) const CAMPAIGN: Route = Route::new(CELL, '⠚');

// `⡪⠓`: the Sloptomizer's evidence note (verbatim, from its runner) and the
// cold-start page, each sent as its address.
pub(crate) const EVIDENCE_NOTE: &str = "⡪⠓⠁⡪⠓⠃";
pub(crate) const COLD: &str = "⡪⠓⠉";
// `⡪⠚`: a campaign round with no advantage spread.
pub(crate) const NO_SPREAD: &str = "⡪⠚⠁";

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "research",
    surface: "the Sloptomizer's and rl_campaign's lifecycle: verdicts, stalls, advice and settled campaigns",
    subs: &[
        Sub {
            route: PASSED,
            name: "passed",
            signal: "a loop_research candidate passed its verifier (its measurements beside it)",
            action: "apply its patch from results to the main workspace, then run the loop's verifier there",
            ideas: "- A passing isolated attempt is evidence, not a change: the main workspace has not moved.\n\
                - Read the candidate's patch in `results` before applying it.",
            pages: &[],
        },
        Sub {
            route: WON,
            name: "won",
            signal: "the candidate beat its paired baseline on the same snapshot (paired delta +1)",
            action: "apply this idea first: it is a measured improvement",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: FAILED,
            name: "failed",
            signal: "the candidate's verifier failed",
            action: "the advice has learned from it: suggest again before the next run",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: UNVERIFIED,
            name: "unverified",
            signal: "a research run finished without a verifier result, so nothing was learned",
            action: "rerun it with a verifier (compare: true) before trusting its patch",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: BASELINE_RED,
            name: "baseline-red",
            signal: "the paired baseline has no completed verifier evidence; the candidate never started",
            action: "fix the verifier or the baseline on the main workspace before comparing again",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: LEARNING_ERROR,
            name: "learning-error",
            signal: "the run's result stands, but the advice did not learn from it (the error beside it)",
            action: "do not expect suggest to reflect this run",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: LIVE,
            name: "live",
            signal: "a research run or an rl_campaign is still running",
            action: "keep doing useful work in the main workspace; check status only when its result would change your next step",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: ADVICE,
            name: "advice",
            signal: "loop_research's advice: rankings to inspect, never a forced choice",
            action: "",
            ideas: "",
            pages: &[
                "Past verifier outcomes guide exploration.",
                "Only paired experiments provide deltas; this does not validate an installed policy or train the provider model.",
                "The advice has no observations yet: its ranking is a seed, so run the top idea with compare to give it evidence.",
            ],
        },
        Sub {
            route: STALL,
            name: "stall",
            signal: "the loop stalled and no research is running",
            action: "loop_research suggest, then run its top idea with compare",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: CAMPAIGN,
            name: "campaign",
            signal: "an rl_campaign settled (its decision beside it)",
            action: "promoted or validated: its policy is installed, keep working under it; any other decision leaves the incumbent in place",
            ideas: "",
            pages: &[
                "Every attempt in the last round passed or every attempt failed, so the campaign had no advantage spread to learn from: vary the task set or the verifier.",
            ],
        },
    ],
};

/// The routes a research run's record raises: live while it runs; after it,
/// the verdict its measurements carry, or the baseline that never let the
/// candidate start, and whether its learning stuck.
pub(crate) fn research(record: &Value) -> Vec<Route> {
    if record["running"] == true {
        return vec![LIVE];
    }
    let status = record["status"].as_str().unwrap_or_default();
    match status {
        "failed" if !record["baseline"].is_null() && record["candidate"].is_null() => {
            vec![BASELINE_RED]
        }
        "completed" | "completed_with_learning_error" => {
            let measurements = &record["measurements"];
            let mut routes = match measurements["candidate_passed"].as_bool() {
                None => vec![UNVERIFIED],
                Some(true) if measurements["paired_delta"].as_i64().is_some_and(|d| d > 0) => {
                    vec![WON, PASSED]
                }
                Some(true) => vec![PASSED],
                Some(false) => vec![FAILED],
            };
            if status == "completed_with_learning_error" {
                routes.push(LEARNING_ERROR);
            }
            routes
        }
        _ => Vec::new(),
    }
}

/// The routes a campaign's status raises: live while it runs, its verdict
/// once it settled.
pub(crate) fn campaign(status: &Value) -> Vec<Route> {
    match status["status"].as_str() {
        Some("running") => vec![LIVE],
        Some("settled") => vec![CAMPAIGN],
        _ => Vec::new(),
    }
}

/// A settled campaign whose last round had no advantage spread. The outcome
/// is as a campaign's status carries it (`{"Ok": …}` or `{"Err": …}`), or bare.
pub(crate) fn no_spread(outcome: &Value) -> bool {
    outcome.get("Ok").unwrap_or(outcome)["advantage_variance"]
        .as_f64()
        .is_some_and(|variance| variance == 0.0)
}

/// Advice with no observations behind its ranking yet.
pub(crate) fn cold_advice(advice: &Value) -> bool {
    advice["observations"].as_u64() == Some(0)
}

/// Routes as the cells of one `warpath` field.
pub(crate) fn cells(routes: &[Route]) -> String {
    routes.iter().map(|route| route.cells()).collect()
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__research_tests.rs"]
mod tests;
