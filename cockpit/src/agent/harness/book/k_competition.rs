//! ⠅ k — the competition loop, double-encoded: `⠅⠁` / `⠅⠃` engage the loop
//! at a pace, and the technique routes after them lay out how it runs.
//!
//! A competition turn opens with one warpath — rapid `⠅⠁⠅⠙⠅⠑` (engage, the
//! best goes up to bat, improve the next), deep `⠅⠃⠅⠑⠅⠙` (engage, build the
//! evidence, submit once the gate passes). A slot the watcher sees finish
//! arrives as `⠅⠉⠅⠙`, with its id, status and score beside the stamps and in the
//! ledger. `⠅⠁` and `⠅⠃` speak in full: their pages are the 0.1.6 pace posture,
//! word for word. The standing world card of 0.1.6 stays gone: standing status is
//! never a reply template.
//!
//! The shelf `⡅` (dot 7 on ⠅) holds the advisories only a competition hears: a
//! verified candidate banked, a candidate that changed after its green, the
//! rapid first write.

use super::{Primary, Raise, Route, Sub};
use crate::agent::harness::{TaskPace, WatchNotify};

pub(crate) const CELL: char = '⠅';
pub(crate) const RAPID: Route = Route::new(CELL, '⠁');
pub(crate) const DEEP: Route = Route::new(CELL, '⠃');
pub(crate) const WATCHER: Route = Route::new(CELL, '⠉');
pub(crate) const SUBMIT: Route = Route::new(CELL, '⠙');
pub(crate) const IMPROVE: Route = Route::new(CELL, '⠑');
pub(crate) const NO_TIMEOUT: Route = Route::new(CELL, '⠋');
// ⠅⠛ ⠅⠓ ⠅⠊ stay in the book; no code raises them since the GPU-comp path went.
pub(crate) const DOCTRINE: Route = Route::new(CELL, '⠛');
pub(crate) const LIVING_PEER: Route = Route::new(CELL, '⠓');
pub(crate) const FREE_TRAIN: Route = Route::new(CELL, '⠊');

/// The competition's shelf, ⡅ (dot 7 on ⠅): the 0.1.6 advisories that only a
/// competition turn hears. ⠅ has one cell left and these need five.
pub(crate) const SHELF_CELL: char = '⡅';
pub(crate) const WINNER_BANK: Route = Route::new(SHELF_CELL, '⠁');
pub(crate) const CANDIDATE_CHANGED: Route = Route::new(SHELF_CELL, '⠃');
pub(crate) const FIRST_WRITE_RAPID: Route = Route::new(SHELF_CELL, '⠉');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "competition",
    surface: "the competition loop: engage at a pace, then its techniques",
    subs: &[
        Sub {
            route: RAPID,
            name: "rapid",
            signal: "engage the competition loop at rapid pace — always be improving",
            action: "",
            ideas: "- Keep one bat in flight and the next best being prepped; sitting on a prepped \
                    submission is a competition failure.\n\
                    - Long monologues or passive waiting do not count.\n\
                    - Report only new results; do not restate the board, the slot or the plan.",
            pages: &[
                "COMPETITION CHALLENGE PACE — RAPID. ALWAYS BE IMPROVING.",
                "Revolving door: mutate → local preflight → the current BEST goes up to bat (SUBMIT, receipt/ID) → immediately improve the next best.",
                "Once a submission is in play the harness watcher owns that slot.",
                "A WATCHER NOTIFY arrives with id + status + score/reason; do not poll-wait.",
                "Sitting on a prepped submission is a competition failure.",
                "Constant output: one bat in flight, the next best being prepped.",
                "After notify, one receipt check is optional; then submit-next (best to bat) or improve-candidate.",
                "Long monologues or passive waiting do not count.",
                "Never wrap builds, engine boots, or benchmarks in `timeout`: the harness has no tool ceiling and reports long tools live; a self-imposed timeout that kills a load mid-flight is the most common cause of \"no verifier result\".",
            ],
        },
        Sub {
            route: DEEP,
            name: "deep",
            signal: "engage the competition loop at deep pace — a slow-burn solve",
            action: "",
            ideas: "- Hop count and watcher state are never instructions to submit; a candidate \
                    that passes the local gate is submitted (receipt/ID) and then improved.\n\
                    - Depth is a reason to measure more, never to withhold a measured candidate.\n\
                    - Report only new results; do not restate the board, the slot or the plan.",
            pages: &[
                "COMPETITION CHALLENGE PACE — DEEP.",
                "This is a slow-burn solve: build a coherent evidence chain, test distinct hypotheses, and converge only when the candidate is defensible.",
                "Hop count, first-write pressure, and watcher state are never instructions to submit — but a candidate that passes the local gate IS submitted (receipt/ID) and then improved; depth is a reason to measure more, never a reason to withhold a measured candidate.",
                "Never wrap builds, engine boots, or benchmarks in `timeout`: the harness has no tool ceiling, long tools are reported live, and a self-imposed timeout that kills a load mid-flight is the most common cause of \"no verifier result\".",
            ],
        },
        Sub {
            pages: &[],
            route: WATCHER,
            name: "watcher",
            signal: "the watcher saw a submission reach a terminal state (id, status, score below)",
            action: "one receipt check is optional; then submit the next best or improve",
            ideas: "- Once a submission is in play the watcher owns that slot: do not poll-wait.",
        },
        Sub {
            pages: &[],
            route: SUBMIT,
            name: "submit",
            signal: "technique: the current best goes up to bat",
            action: "submit the best candidate that passed the local gate and keep its receipt/ID",
            ideas: "",
        },
        Sub {
            pages: &[],
            route: IMPROVE,
            name: "improve",
            signal: "technique: improve the next best while one is in flight",
            action: "take the next measured step on the next candidate",
            ideas: "",
        },
        Sub {
            pages: &[],
            route: NO_TIMEOUT,
            name: "no-timeout",
            signal: "technique: let long loads finish",
            action: "never wrap builds, engine boots or benchmarks in `timeout`",
            ideas: "- The harness has no tool ceiling and reports long tools live; a \
                    self-imposed timeout that kills a load mid-flight is the most common cause \
                    of \"no verifier result\".",
        },
        Sub {
            route: DOCTRINE,
            name: "doctrine",
            signal: "the always-driving competition doctrine (local GPU MoA)",
            action: "",
            ideas: "",
            pages: &[
                "[always driving competition doctrine] ALWAYS BE IMPROVING.",
                "The competition loop is a revolving door of constant output: the current BEST goes up to bat immediately, and the next best is prepped while that slot is in flight.",
                "Sitting, polling, or waiting on a prepped submission is a failure.",
                "Once a submission is in play, immediately branch the winning baseline, formulate the next hypothesis, run local preflights, and push the frontier.",
                "The harness watcher owns in-flight status — do not poll-wait.",
            ],
        },
        Sub {
            route: LIVING_PEER,
            name: "living-peer",
            signal: "the living B200 peer frontier (evidence below)",
            action: "the live status, verbatim, is the latest evidence",
            ideas: "",
            pages: &[],
        },
        Sub {
            route: FREE_TRAIN,
            name: "free-train",
            signal: "the free-train LoRA cycle (evidence below)",
            action: "the live status, verbatim, is the latest evidence",
            ideas: "",
            pages: &[],
        },
    ],
};

/// The warpath that engages the competition loop at `pace`.
pub(crate) fn engage(pace: TaskPace) -> Vec<Raise> {
    let routes = match pace {
        TaskPace::Rapid => [RAPID, SUBMIT, IMPROVE],
        TaskPace::Deep => [DEEP, IMPROVE, SUBMIT],
    };
    routes
        .into_iter()
        .map(|route| Raise::new(route, None))
        .collect()
}

/// The turn a finished slot arrives as: the warpath, then its receipt (id,
/// status, score or reason) as data, which is what the 0.1.6 watcher notice
/// carried.
pub(crate) fn watcher_turn(workspace: &std::path::Path, notify: &WatchNotify) -> String {
    let raises = watcher(notify);
    let receipt = raises
        .first()
        .and_then(|raise| raise.evidence.clone())
        .unwrap_or_default();
    format!("{}\n{receipt}", super::warpath(workspace, &raises))
}

/// The warpath for a slot the watcher saw finish, with its receipt.
pub(crate) fn watcher(notify: &WatchNotify) -> Vec<Raise> {
    let mut receipt = format!("submission {} status={}", notify.id, notify.status);
    match (&notify.score, &notify.rejection_reason) {
        (Some(score), Some(reason)) => receipt.push_str(&format!(" score={score} reason={reason}")),
        (Some(score), None) => receipt.push_str(&format!(" score={score}")),
        (None, Some(reason)) => receipt.push_str(&format!(" reason={reason}")),
        (None, None) => receipt.push_str(" reason=unspecified-terminal"),
    }
    if let Some(note) = &notify.source_note {
        receipt.push_str("; ");
        receipt.push_str(note);
    }
    vec![Raise::new(WATCHER, receipt), Raise::new(SUBMIT, None)]
}

pub(crate) const SHELF: Primary = Primary {
    cell: SHELF_CELL,
    name: "competition-advisories",
    surface: "the competition's advisories: a verified candidate, a changed candidate, the first write",
    subs: &[
        Sub {
            route: WINNER_BANK,
            name: "winner-bank",
            signal: "a local check passed on the candidate in a competition turn",
            action: "",
            ideas: "",
            pages: &[
                "COMPETITION CANDIDATE VERIFIED. A local check passed on this candidate.",
                "Preserve the passing candidate and its evidence before speculative edits.",
                "If the operator has authorized submission and all required checks pass, submit through the agreed workflow and record the receipt.",
                "A local pass is not proof of a leaderboard win; complete remaining requested checks first.",
            ],
        },
        Sub {
            route: CANDIDATE_CHANGED,
            name: "candidate-changed",
            signal: "the workspace changed after a passing check in a competition turn",
            action: "",
            ideas: "",
            pages: &[
                "VERIFIED CANDIDATE CHANGED: The workspace changed after a passing check.",
                "Preserve the prior candidate if available and verify the new bytes before claiming success.",
                "Follow the operator-authorized submission plan; a local pass alone does not prove a competitive win.",
            ],
        },
        Sub {
            route: FIRST_WRITE_RAPID,
            name: "first-write-rapid",
            signal: "the armed inspection budget is spent in a rapid competition turn",
            action: "",
            ideas: "",
            pages: &[
                "RAPID COMPETITION CANDIDATE PROGRESS. Inspection has not yet produced candidate progress.",
                "The next tool must mutate the candidate or follow the explicitly armed submission contract; a board receipt remains legal.",
                "Do not wrap another source read in a build/check.",
                "If no defensible edit exists, report the concrete blocker.",
            ],
        },
    ],
};
