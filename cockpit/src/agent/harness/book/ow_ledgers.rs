//! ⠪ ow — the loop's ledgers: what this run changed, measured and was told,
//! what background jobs returned, and the RL campaigns and loop research
//! beside it. The ledger bodies ride beside the routes; an outcome's notes
//! (`⠪⠊`) ride beside its facts as page addresses.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠪';
pub(crate) const FILES: Route = Route::new(CELL, '⠁');
pub(crate) const MEASUREMENTS: Route = Route::new(CELL, '⠃');
pub(crate) const STEER_STANDING: Route = Route::new(CELL, '⠉');
pub(crate) const STEER_FRESH: Route = Route::new(CELL, '⠙');
pub(crate) const OUTCOMES: Route = Route::new(CELL, '⠑');
pub(crate) const RL: Route = Route::new(CELL, '⠋');
pub(crate) const RESEARCH_PATHS: Route = Route::new(CELL, '⠛');
pub(crate) const LOOP_RESEARCH: Route = Route::new(CELL, '⠓');
pub(crate) const OUTCOME_NOTES: Route = Route::new(CELL, '⠊');
pub(crate) const RESEARCH_NOTES: Route = Route::new(CELL, '⠚');

// `⠪⠊`: each page is sent alone, as its address, beside an outcome's facts.
pub(crate) const PROC_NOTES: &str = "⠪⠊⠁⠪⠊⠃";
pub(crate) const OWNER_INTERRUPTED: &str = "⠪⠊⠉";
pub(crate) const INSPECT_CANDIDATE: &str = "⠪⠊⠙";
pub(crate) const RESEARCH_HISTORY_UNAVAILABLE: &str = "⠪⠊⠑";
pub(crate) const HISTORY_UNAVAILABLE: &str = "⠪⠊⠋";
pub(crate) const WORKER_DISCONNECTED: &str = "⠪⠊⠛";
pub(crate) const CAMPAIGN_ACTIVE: &str = "⠪⠊⠓";

// `⠪⠚`: loop_research's notes, as runs of page addresses in its results.
pub(crate) const RESEARCH_EXECUTION: &str = "⠪⠚⠁⠪⠚⠃⠪⠚⠉";
pub(crate) const RESEARCH_LEARNING: &str = "⠪⠚⠙⠪⠚⠑";
pub(crate) const RESEARCH_RUNTIME: &str = "⠪⠚⠋⠪⠚⠛";
pub(crate) const RESEARCH_RUNNING: &str = "⠪⠚⠓⠪⠚⠊";
pub(crate) const RESEARCH_ACTIVE: &str = "⠪⠚⠚";

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "ledgers",
    surface: "loop ledgers: changes, measurements, steering, outcomes, RL campaigns",
    subs: &[
        Sub {
            route: FILES,
            name: "files",
            signal: "files changed so far this run (diff stat below)",
            action: "",
            ideas: "",
            pages: &[
                "[files changed so far this run (git diff --stat) — build on these; do not re-make or blindly revert them]",
            ],
        },
        Sub {
            route: MEASUREMENTS,
            name: "measurements",
            signal: "measurements and submissions this run (below)",
            action: "",
            ideas: "",
            pages: &[
                "[measurements and submissions this run — newest first, as the harness recorded them]",
            ],
        },
        Sub {
            route: STEER_STANDING,
            name: "steer-standing",
            signal: "operator notes already answered (below)",
            action: "",
            ideas: "",
            pages: &[
                "[operator steering — notes the user sent earlier in this run, already answered; keep honoring them, do not answer or act on them again]",
            ],
        },
        Sub {
            route: STEER_FRESH,
            name: "steer-fresh",
            signal: "operator notes not yet answered (below)",
            action: "",
            ideas: "",
            pages: &[
                "[operator message — sent mid-run and not yet answered; answer it once while pursuing the task]",
            ],
        },
        Sub {
            route: OUTCOMES,
            name: "outcomes",
            signal: "background process outcomes (below)",
            action: "",
            ideas: "",
            pages: &[
                "[background process outcomes — inspect the retained logs and actual verifier; an exit is not a solve]",
            ],
        },
        Sub {
            route: RL,
            name: "rl-campaigns",
            signal: "rl_campaign is available in this loop (history below)",
            action: "",
            ideas: "",
            pages: &[
                "[RL campaigns]\nrl_campaign is available throughout this loop: run starts asynchronous measured attempts on the current route; status, results, and stop manage them.",
                "Use it when comparing approaches or improving a policy would help.",
                "Continue useful work while it runs.",
                "Inspect actual verifier outcomes and retained attempt artifacts; apply a useful candidate to the main workspace and verify it there.",
                "Submit a verified winner when ready.",
                "Campaign availability does not require you to launch one.",
                "Campaigns without an independent audit are measured exploration; they do not install validated learning.",
            ],
        },
        Sub {
            route: RESEARCH_PATHS,
            name: "research-paths",
            signal: "optional research paths",
            action: "",
            ideas: "",
            pages: &[
                "Optional research paths: consult_model(method=\"deli\", club=\"self\") explores directions and returns a synthesis to this turn; spawn(formation=\"moa\") compares parallel approaches; continual_harness retains useful supplemental notes.",
                "Choose them when helpful, return to ordinary tools when ready, and check proposals against actual evidence.",
            ],
        },
        Sub {
            route: LOOP_RESEARCH,
            name: "loop-research",
            signal: "loop_research is available in this loop (history below)",
            action: "",
            ideas: "",
            pages: &[
                "Optional loop_research: Sloptomizer suggest offers pareto, bandit and memory advice; run tries your chosen idea asynchronously on this route.",
                "compare=true measures a baseline and candidate from the same source.",
                "status/results/stop let you step out and back into ordinary work.",
                "Verifier receipts update exploratory memory; no forced research step or policy install.",
                "Submit a verified winner when ready.",
            ],
        },
        Sub {
            route: OUTCOME_NOTES,
            name: "outcome-notes",
            signal: "notes on a background or experiment outcome (one page, beside its facts)",
            action: "",
            ideas: "",
            pages: &[
                "Inspect proc_status id={id} for captured output (retained log {log}).",
                "Process exit is not benchmark acceptance or a verified solve.",
                "Owner was interrupted; inspect retained working source and logs before choosing another experiment.",
                "inspect candidate and evaluation receipts before integration",
                "Research history unavailable:",
                "History unavailable:",
                "experiment worker disconnected; inspect retained artifacts",
                "an rl_campaign is already running; continue useful work or request stop and inspect status",
            ],
        },
        Sub {
            route: RESEARCH_NOTES,
            name: "research-notes",
            signal: "loop_research's notes on its options and runs (page addresses in its results)",
            action: "",
            ideas: "",
            pages: &[
                "Optional isolated attempt on the current loop route.",
                "compare=true adds a baseline attempt from the same frozen source.",
                "No extra hop, time or thinking caps; explicit loop budgets and cancellation apply.",
                "Physical verifier receipts update original Sloptomizer UCB, Pareto and MicroLearner state.",
                "Exploratory advice, not an audited policy install or provider weight training.",
                "Bundled algorithms; requires Python 3 standard library.",
                "suggest checks the runtime without a model call.",
                "Continue useful work.",
                "status/results expose evidence and learning; stop exits this research run without ending the main loop.",
                "a research run is already active; continue useful work or request stop and inspect status",
            ],
        },
    ],
};
