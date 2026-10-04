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
                "Start from the current promoted leader source, preserving useful local work in a separate branch or checkout. Compare optimized baseline and candidate builds on the same hardware and evaluator protocol; retain the source identities and measured results.",
                "Keep the exact submission ID and check its official result; automatic tracking is not connected to ordinary submissions.",
                "Use the platform CLI for status, score, rejection reason and metrics. A configured watcher is supplementary; a missing notification is not evidence that a job is pending.",
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
            pages: &[
                "Read officialMetrics and rejectionReason as well as officialScore. Compare verified work with self-reported work when both exist. A discrepancy needs investigation; self-reported peak throughput is not proof of tested candidates or lost hits.",
                "Historical briefs and handoffs are evidence, not new restrictions. Recheck inherited prohibitions against current user instructions and actual tool capabilities. Inspect the leader source before deciding to keep an older local lineage.",
                "Use the optimized build and only the validation needed for the changed candidate. Reuse a valid receipt for unchanged bytes and inputs; do not delay an eligible winner with broad repeated checks.",
            ],
            route: WATCHER,
            name: "submission-status",
            signal: "inspect the official submission result or current frontier",
            action: "correlate the exact terminal UUID with its dispatched candidate and durable receipt; queued is pending, rejection or failure feeds rerooting at the latest frontier, and only a promoted source plus a verified score/frontier is a win; then improve or submit-next",
            ideas: "- Inspect the exact submission result when it can change your next decision; continue useful work while it runs.",
        },
        Sub {
            pages: &[],
            route: SUBMIT,
            name: "submit",
            signal: "technique: the current best goes up to bat",
            action: "when authorized, submit only after correctness/preflight checks and preserve the preflight receipt; record the exact candidate commit, source/archive and evaluated-binary SHA256 when available, predecessor, protocol, and terminal receipt/ID. Deduplicate the same in-flight job, but allow controlled rule-compliant repeated measurements with a stated reason",
            ideas: "",
        },
        Sub {
            pages: &[],
            route: IMPROVE,
            name: "improve",
            signal: "technique: improve the next best while one is in flight",
            action: "form one measurable hypothesis, reroot any rejected mechanism at the latest frontier, make the smallest distinct patch, preserve passing bytes and receipt before edits, compare same-binary A/B against a fresh baseline, credit the original author, and record correctness/noise/protocol identity in the durable measurement ledger (`⠪⠃`)",
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
                "Submit a verified competitive candidate promptly; inspect pending results when useful.",
                "Once a submission is in play, immediately branch the winning baseline, formulate the next hypothesis, run local preflights, and push the frontier.",
                "The agent owns result follow-through. Inspect the exact ID with the platform CLI.",
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

/// The route order for the competition loop at `pace`.
pub(crate) fn engage(pace: TaskPace) -> Vec<Raise> {
    engage_routes(pace, None)
}

/// Competition entry plus the persisted brief, carried inline so the model
/// receives evidence rather than only recording it in a ledger.
pub(crate) fn engage_for_workspace(pace: TaskPace, workspace: &std::path::Path) -> Vec<Raise> {
    engage_routes(pace, Some(workspace))
}

fn engage_routes(pace: TaskPace, workspace: Option<&std::path::Path>) -> Vec<Raise> {
    let routes = match pace {
        TaskPace::Rapid => [RAPID, SUBMIT, IMPROVE],
        TaskPace::Deep => [DEEP, IMPROVE, SUBMIT],
    };
    let mut raises = routes
        .into_iter()
        .map(|route| Raise::new(route, None))
        .collect::<Vec<_>>();
    if let (Some(raise), Some(workspace)) = (raises.first_mut(), workspace)
        && let Some(brief) = crate::agent::harness::cartridges::active()
            .and_then(|cartridge| cartridge.hooks().seat_entry_focus(workspace))
    {
        *raise = Raise::inline(raise.route, brief);
    }
    raises
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
    let persistence = crate::agent::harness::cartridges::active()
        .and_then(|cartridge| cartridge.hooks().record_terminal(workspace, notify).err())
        .map(|error| format!("\nTerminal evidence persistence unavailable: {error}"))
        .unwrap_or_default();
    format!(
        "{}\n{receipt}{persistence}",
        super::warpath(workspace, &raises)
    )
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
                "Preserve the passing candidate and its existing preflight receipt before further edits.",
                "Check eligibility against the current promoted leader and submit promptly within the operator's instructions. Additional checks need a specific unresolved concern; do not repeat passing checks on unchanged bytes and inputs.",
                "A local pass is not proof of a leaderboard win; read the official submission result.",
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
                "Check whether candidate code, inputs, or the evaluator changed. Reuse the existing receipt when only notes or unrelated files changed; rerun only the affected required check.",
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
