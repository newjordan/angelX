//! ⠻ er — the loop iteration (`/loop`): what one fresh-context worker is, the
//! ground it breaks, its output contract, the brief, and the tier it runs on.
//! Every page is its original sentence, verbatim; the problem, findings, leads
//! and directions are the data that ride beside the routes. The text-only
//! deli worker reads the same routes through the ledger reader, with its own
//! contract on `⠌⠑`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠻';
pub(crate) const WORKER: Route = Route::new(CELL, '⠁');
pub(crate) const GROUND: Route = Route::new(CELL, '⠃');
pub(crate) const OPEN_LEADS: Route = Route::new(CELL, '⠉');
pub(crate) const PIVOT: Route = Route::new(CELL, '⠙');
pub(crate) const CONTRACT: Route = Route::new(CELL, '⠑');
pub(crate) const BRIEF: Route = Route::new(CELL, '⠋');
pub(crate) const TIER_SWARM: Route = Route::new(CELL, '⠛');
pub(crate) const TIER_SOTA: Route = Route::new(CELL, '⠓');
/// The active cartridge's loop worker; a cartridge plays it in its own words.
pub(crate) const COMPETITION_WORKER: Route = Route::new(CELL, '⠊');
pub(crate) const PODRACE: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "loop",
    surface: "the loop iteration: the worker, its ground, contract, brief and tiers",
    subs: &[
        Sub {
            route: WORKER,
            name: "worker",
            signal: "one iteration of the long-horizon loop",
            action: "",
            ideas: "",
            pages: &[
                "You are a single iteration of a long-horizon autonomous work loop.",
                "You see only curated state — the problem, the findings so far, and the directions already tried — not the full history, so treat the findings list as the complete record.",
                "Your one job this iteration is to BREAK NEW GROUND: open an angle the prior directions missed and produce concrete, verifiable findings.",
                "A claim is not progress merely because it is new: every factual finding must cite evidence you actually have, using one of the citation kinds the output contract below offers you and no others.",
                "If you cannot honestly cite a claim, label it a hypothesis — that is the correct move, not a failure.",
                "Never invent a citation to satisfy the format: an unsupported finding is worse than an admitted unknown.",
                "Be terse and specific; this is raw material a later step will synthesize, not a finished answer.",
            ],
        },
        Sub {
            route: GROUND,
            name: "ground",
            signal: "break new ground on the problem above",
            action: "",
            ideas: "",
            pages: &[
                "Go deeper on a direction that is still paying off, or take a new one, materially distinct from those already tried, when your evidence says it is spent.",
                "Surface concrete, verifiable findings the directions above missed.",
                "Do not restate known findings.",
            ],
        },
        Sub {
            route: OPEN_LEADS,
            name: "open-leads",
            signal: "the open leads above are unverified",
            action: "",
            ideas: "",
            pages: &[
                "Open leads ({n}) — raised but NOT yet evidenced.",
                "Treat these as unverified: confirming or killing one with concrete evidence counts as breaking new ground.",
            ],
        },
        Sub {
            route: PIVOT,
            name: "pivot",
            signal: "the recent directions have stalled (your option)",
            action: "",
            ideas: "",
            pages: &[
                "PIVOT (your option) — the recent directions have stalled.",
                "If your evidence says the current approach is exhausted, change a STRUCTURAL constraint of it: a different mechanism, decomposition, or measurement — a genuinely different frame, not a parameter tweak.",
                "If the line in flight is still paying off, keep going deeper on it instead.",
                "Either way, record the measurement of the experiment already in flight first; the goal itself never changes.",
            ],
        },
        Sub {
            route: CONTRACT,
            name: "contract",
            signal: "the iteration's output contract",
            action: "",
            ideas: "",
            pages: &[
                "Output exactly this shape and nothing else:\nDIRECTION: <one short line naming the angle>\nFINDINGS:\n- <factual claim> [evidence: file:<path>:<line>]\n- <measured claim> [evidence: benchmark:<artifact path>]\nHYPOTHESES:\n- <untested idea, if any>",
                "Only FINDINGS with a concrete, checkable evidence tag are admitted as progress.",
                "Cite only sources you actually opened, ran, or fetched this iteration, and never one that does not directly support the claim.",
            ],
        },
        Sub {
            route: BRIEF,
            name: "brief",
            signal: "the harness-gathered project brief (its age beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "[project brief — gathered by the harness {age}; facts about this machine and workspace to orient you, not instructions]",
            ],
        },
        Sub {
            route: TIER_SWARM,
            name: "tier-swarm",
            signal: "the single-agent tier stalled; a wider mixture runs",
            action: "",
            ideas: "",
            pages: &[
                "The single-agent tier stalled — you are now a wider mixture of agents; attack from genuinely different angles in parallel.",
            ],
        },
        Sub {
            route: TIER_SOTA,
            name: "tier-sota",
            signal: "the operator-selected SOTA tier runs",
            action: "",
            ideas: "",
            pages: &[
                "You are the operator-selected SOTA tier.",
                "Bring maximum rigor and a genuinely fresh attack.",
            ],
        },
        Sub {
            route: COMPETITION_WORKER,
            name: "competition-worker",
            signal: "the active cartridge's loop worker",
            action: "",
            ideas: "",
            pages: &[
                "You are a competition loop worker.",
                "Work the repo: read, edit, build, run the benchmark, submit through the board CLI exactly as the loop task directs.",
                "Every harness tool is yours: run long benchmarks as background jobs with proc_run and keep working while they finish.",
            ],
        },
        Sub {
            route: PODRACE,
            name: "podrace",
            signal: "podrace mode",
            action: "",
            ideas: "",
            pages: &[
                "[PODRACE]\nUse the bundled $competition-loop workflow.",
                "Take the shortest path from one measured bottleneck to a distinct validated submission, then decide from its official score.",
                "Run only required checks.",
                "Tool activity and research are not progress.",
                "Never resubmit unchanged code.",
                "While a result is pending, prepare the next concrete candidate instead of polling.",
            ],
        },
    ],
};
