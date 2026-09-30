//! ⠟ q — the stop checkpoint and the if-stuck strategies.
//!
//! When the model answers while the turn holds stop facts, the harness shows
//! it one warpath: the two most pressing facts, then `⠟⠁` (the deli route).
//! Once per turn; the model's next answer stands. The other subs are the
//! strategies a stuck model can pick from `ledger://⠟`.
//!
//! When a turn stops on the harness's word, not the model's, the answer the
//! harness writes is `⠟⠛`: its reader is the operator or an evaluator, who
//! cannot read the ledger, so the answer carries the page itself. A local seat
//! that faults mid-session is recovered by the teacher-watch: its notes are
//! `⠟⠓` and `⠟⠊`, and the cheap teacher it may ask is `⠟⠚`.

use super::{
    Primary, Raise, Route, Sub, WARPATH_ROUTES, d3456_advisories, l_loops, p_processes,
    v_verification, x_execution,
};
use std::path::Path;

pub(crate) const CELL: char = '⠟';
pub(crate) const DELI: Route = Route::new(CELL, '⠁');
pub(crate) const RESEARCH: Route = Route::new(CELL, '⠃');
pub(crate) const LOOP_RESEARCH: Route = Route::new(CELL, '⠉');
pub(crate) const RESTATE: Route = Route::new(CELL, '⠙');
pub(crate) const NEIGHBOR: Route = Route::new(CELL, '⠑');
pub(crate) const FINISH: Route = Route::new(CELL, '⠋');
pub(crate) const ANSWERS: Route = Route::new(CELL, '⠛');
pub(crate) const TEACHER_NOTES: Route = Route::new(CELL, '⠓');
pub(crate) const TEACHER_NOTES_MORE: Route = Route::new(CELL, '⠊');
pub(crate) const TEACHER: Route = Route::new(CELL, '⠚');

/// `⠟⠛⠁`: the acceptance gate passed and the turn ends on it.
pub(crate) const ACCEPTED: &str = "Acceptance gate passed after {hop} tool hop(s); the verified workspace is ready for inspection.";
/// `⠟⠛⠃`: a research turn hit its hop cap with no draft.
pub(crate) const NO_DRAFT: &str =
    "Evidence is missing; no research draft was produced before the hop cap.";
/// `⠟⠛⠉`: the research contract's whole-question decline.
pub(crate) const DISCLOSURE: &str =
    "No evidence in the corpus supports an answer to this question.";

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "stop",
    surface: "the stop checkpoint, the strategies for a stuck turn, the harness's own answers, the teacher-watch",
    subs: &[
        Sub {
            pages: &[],
            route: DELI,
            name: "deli",
            signal: "the stop checkpoint: the facts before this route were seen when you answered",
            action: "if stuck: `consult_model` with `{\"method\":\"deli\",\"prompt\":\"<the blocker>\"}` \
                     runs a fresh-context deliberation loop and returns here",
            ideas: "- Otherwise answer again: the next answer stands (⠟⠋).\n\
                    - Every fact seen at this stop is in the evidence below.",
        },
        Sub {
            pages: &[],
            route: RESEARCH,
            name: "research",
            signal: "a strategy: research the topic before the next attempt",
            action: "`science_search` (arxiv) or `web_search` (exa) on the blocker",
            ideas: "",
        },
        Sub {
            pages: &[],
            route: LOOP_RESEARCH,
            name: "loop-research",
            signal: "a strategy inside a /loop: test one idea in an isolated copy, measured against a baseline",
            action: "`loop_research` suggest, then run its top idea with compare",
            ideas: "",
        },
        Sub {
            pages: &[],
            route: RESTATE,
            name: "restate",
            signal: "a strategy: shrink the blocker",
            action: "restate it in one sentence, then take the smallest check that would disprove it",
            ideas: "",
        },
        Sub {
            pages: &[],
            route: NEIGHBOR,
            name: "neighbor",
            signal: "a strategy: copy the shape that works",
            action: "read a working neighbor — a passing test, a sibling module",
            ideas: "",
        },
        Sub {
            pages: &[],
            route: FINISH,
            name: "finish",
            signal: "the answer stands",
            action: "answer again to finish the turn",
            ideas: "",
        },
        Sub {
            route: ANSWERS,
            name: "answers",
            signal: "an answer the harness writes when the turn stops on its word (the reader cannot read the ledger, so it carries the page)",
            action: "",
            ideas: "",
            pages: &[ACCEPTED, NO_DRAFT, DISCLOSURE],
        },
        Sub {
            route: TEACHER_NOTES,
            name: "teacher-notes",
            signal: "the teacher-watch recovered a faulted local seat: context overflow, an empty reply, a dark transport",
            action: "",
            ideas: "",
            pages: &[
                "teacher-watch: context overflow — rolled the tail into a ledger.",
                "Continue from the current workspace and the compact note.",
                "Do not re-read the whole transcript.",
                "teacher-watch: local seat returned empty — context rolled.",
                "Answer or tool-call now; do not replay the same empty hop.",
                "teacher-watch: local seat went dark (transport).",
                "Context rolled.",
                "Do not retry the same dead endpoint this hop; continue from the ledger on the next iteration.",
            ],
        },
        Sub {
            route: TEACHER_NOTES_MORE,
            name: "teacher-notes-more",
            signal: "the teacher-watch recovered a faulted local seat: a named local unreachable, a long generation timed out",
            action: "",
            ideas: "",
            pages: &[
                "teacher-watch: named local is not reachable.",
                "Skipped.",
                "Continue yourself from the ledger; do not retry the dark seat.",
                "teacher-watch: local seat timed out after a long generation.",
                "Context rolled.",
                "Continue with a smaller next action.",
            ],
        },
        Sub {
            route: TEACHER,
            name: "teacher",
            signal: "the cheap teacher-monitor, asked about one fault (the fault, the hop and the error below)",
            action: "",
            ideas: "",
            pages: &[
                "You are a cheap teacher-monitor for a long local-model coding session.",
                "One fault just happened during a calibration/proving run.",
                "Reply with exactly one line:\nROLL: <why> | RETRY: <why> | CATCH: <bug> | CONTINUE: <next action>",
            ],
        },
    ],
};

/// Which stop facts lead the warpath, most pressing first.
const PRIORITY: [Route; 14] = [
    v_verification::ACCEPTANCE,
    v_verification::RED,
    v_verification::POST_WRITE,
    v_verification::FLAKY,
    v_verification::TESTS_EDITED,
    d3456_advisories::NO_EDIT,
    x_execution::MARKUP,
    v_verification::UNTESTED,
    p_processes::FAILED,
    p_processes::LIVE,
    l_loops::SAME_BATCH,
    l_loops::STORM,
    l_loops::POLL,
    x_execution::ERRORS,
];

fn rank(route: Route) -> usize {
    PRIORITY
        .iter()
        .position(|candidate| *candidate == route)
        .unwrap_or(PRIORITY.len())
}

/// The once-per-turn stop checkpoint.
#[derive(Default)]
pub(crate) struct Checkpoint {
    spent: bool,
}

impl Checkpoint {
    pub(crate) fn spent(&self) -> bool {
        self.spent
    }

    /// The checkpoint warpath for these facts, or `None` when there are none,
    /// it was already shown this turn, or too little budget is left to act.
    pub(crate) fn engage(
        &mut self,
        workspace: &Path,
        facts: &[Raise],
        budget_left: bool,
    ) -> Option<(String, Vec<Raise>)> {
        if facts.is_empty() || self.spent || !budget_left {
            return None;
        }
        self.spent = true;
        // Evidence for every fact reaches the ledger, even past the cap.
        for raise in facts {
            if let Some(evidence) = raise.evidence.as_deref() {
                super::ledger::record(workspace, raise.route, evidence);
            }
        }
        let mut ordered = facts.to_vec();
        ordered.sort_by_key(|raise| rank(raise.route));
        let seen = ordered
            .iter()
            .map(|raise| format!("{} {}", raise.route.cells(), raise.route.name()))
            .collect::<Vec<_>>()
            .join("\n");
        let mut shown = ordered;
        shown.truncate(WARPATH_ROUTES - 1);
        shown.push(Raise::new(DELI, format!("Facts at this stop:\n{seen}")));
        let cells = super::warpath(workspace, &shown);
        Some((cells, shown))
    }
}

/// The checkpoint as the model reads it: the warpath, then what the facts
/// carry inline — the failing run's tail, the changed files, the receipt — as
/// the 0.1.6 advisories quoted them, and what is left to act on. The facts
/// are data; the sentences that go with them arrive through the legend. The
/// checkpoint's own evidence (`⠟⠁`, the list of facts) stays in the ledger.
pub(crate) fn stop_turn(cells: &str, shown: &[Raise], left: Option<&str>) -> String {
    let mut turn = cells.to_string();
    if let Some(left) = left {
        turn.push('\n');
        turn.push_str(left);
    }
    for raise in shown.iter().filter(|raise| raise.route != DELI) {
        if let Some(evidence) = raise
            .evidence
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            turn.push('\n');
            turn.push_str(evidence);
        }
    }
    turn
}

/// What a task turn still has after a stop checkpoint, or `None` when too
/// little is left for the checkpoint to be worth a hop: fewer than three steps
/// (an edit, a test run, the answer), or less than a quarter of the task's
/// wall clock. `wall_secs` 0 means no wall.
pub(crate) fn task_budget_left(
    hop: usize,
    max_hops: Option<usize>,
    elapsed_secs: u64,
    wall_secs: usize,
) -> Option<String> {
    let steps = match max_hops {
        Some(max) if max.saturating_sub(hop) < 3 => return None,
        Some(max) => Some(max - hop),
        None => None,
    };
    let secs = match wall_secs as u64 {
        0 => None,
        wall if wall.saturating_sub(elapsed_secs).saturating_mul(4) < wall => return None,
        wall => Some(wall - elapsed_secs.min(wall)),
    };
    Some(match (secs, steps) {
        (Some(secs), Some(steps)) => format!("About {secs} s and {steps} steps are left."),
        (Some(secs), None) => format!("About {secs} s are left."),
        (None, Some(steps)) => format!("{steps} steps are left."),
        (None, None) => "This task has no time or step limit.".to_string(),
    })
}
