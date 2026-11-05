//! ⠘ d45 — the iteration: what one `/loop` iteration is told about the last
//! one. Its setbacks ride under the setback header (`⠳⠑`), its notes under the
//! note header (`⠳⠚`). A one-sentence setback is sent as its page address, a
//! longer one as its route; the counts, errors and summaries ride beside them
//! as data. The curated state an iteration works from (`⠘⠋`) and the project
//! brief it is handed (`⠘⠛`, `⠘⠓`) are labelled by page address: the labels
//! are the pages, the problem, findings, machine facts and paths are the data.
//! The loop's own research and campaign tools name their notes here too
//! (`⠘⠊`), and the calls they refuse for their arguments (`⠘⠚`).
//! Every page is its original sentence, verbatim. Volume V, the long run.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/d45_iteration.book");
pub(crate) const CELL: char = '⠘';
pub(crate) const PROGRESS: Route = Route::new(CELL, '⠁');
pub(crate) const FAULT: Route = Route::new(CELL, '⠃');
pub(crate) const PODRACE_STALL: Route = Route::new(CELL, '⠉');
pub(crate) const RECEIPT: Route = Route::new(CELL, '⠙');
pub(crate) const FIRST_CANDIDATE: Route = Route::new(CELL, '⠑');
pub(crate) const CURATED: Route = Route::new(CELL, '⠋');
pub(crate) const BRIEF: Route = Route::new(CELL, '⠛');
pub(crate) const BRIEF_BENCHMARK: Route = Route::new(CELL, '⠓');
pub(crate) const LOOP_TOOLS: Route = Route::new(CELL, '⠊');
pub(crate) const LOOP_TOOL_REFUSALS: Route = Route::new(CELL, '⠚');

// `⠘⠁`: each page is sent alone, as its address.
pub(crate) const FALLBACK: &str = "⠘⠁⠁";
pub(crate) const TOOL_ERRORS: &str = "⠘⠁⠃";
pub(crate) const UNVERIFIED: &str = "⠘⠁⠉";
pub(crate) const DONE_UNVERIFIED: &str = "⠘⠁⠙";
pub(crate) const STALLED: &str = "⠘⠁⠑";
pub(crate) const SUBMIT_DUE: &str = "⠘⠁⠋";

// `⠘⠃`: each page is sent alone, as its address.
pub(crate) const ERRORED: &str = "⠘⠃⠁";
pub(crate) const HOP_HORIZON: &str = "⠘⠃⠃";
pub(crate) const PROVIDER_BLOCKED: &str = "⠘⠃⠉";
pub(crate) const BASELINE_DIED: &str = "⠘⠃⠙";
pub(crate) const VERIFY_DIED: &str = "⠘⠃⠑";
pub(crate) const ACCEPT_FAILED: &str = "⠘⠃⠋";
pub(crate) const TURN_STOPPED: &str = "⠘⠃⠛";

// `⠘⠊`: each page is sent alone, as its address (a label with its value).
pub(crate) const RESEARCH_PANICKED: &str = "⠘⠊⠁";
pub(crate) const RESEARCH_UNSTARTED: &str = "⠘⠊⠃";
pub(crate) const RESEARCH_APPROACH: &str = "⠘⠊⠉";
pub(crate) const RESEARCH_SNIPPETS: &str = "⠘⠊⠙";
pub(crate) const CAMPAIGN_NEEDS_VERIFIER: &str = "⠘⠊⠑";
pub(crate) const CAMPAIGN_UNKNOWN_ACTION: &str = "⠘⠊⠋";
pub(crate) const RESEARCH_NEEDS_LOOP: &str = "⠘⠊⠓";
pub(crate) const RESEARCH_NEEDS_VERIFIER: &str = "⠘⠊⠊";
pub(crate) const RESEARCH_UNKNOWN_ACTION: &str = "⠘⠊⠚";
pub(crate) const CAMPAIGN_NEEDS_LOOP: &str = "⠘⠊⠛";

// `⠘⠚`: each page is sent alone, as its address. A value of the wrong JSON
// type stays English, as in every tool; these are a required field missing or
// a value the tool itself rejects.
pub(crate) const CANDIDATE_NEEDS_IDEA: &str = "⠘⠚⠁";
pub(crate) const RUN_NEEDS_IDEA: &str = "⠘⠚⠃";
pub(crate) const RESEARCH_ID_INVALID: &str = "⠘⠚⠉";
pub(crate) const RESEARCH_ID_UNKNOWN: &str = "⠘⠚⠙";
pub(crate) const SAMPLES_TOO_FEW: &str = "⠘⠚⠑";
pub(crate) const AUDIT_NEEDS_TASK: &str = "⠘⠚⠋";
pub(crate) const AUDIT_NEEDS_VERIFY: &str = "⠘⠚⠛";
pub(crate) const AUDIT_NEEDS_SOURCE: &str = "⠘⠚⠓";
pub(crate) const CAMPAIGN_ID_INVALID: &str = "⠘⠚⠊";
pub(crate) const CAMPAIGN_ID_UNKNOWN: &str = "⠘⠚⠚";

/// `⠘⠑` without its blocked-attempt page: no attempt was blocked.
pub(crate) const NO_CANDIDATE: &str = "⠘⠑⠁⠘⠑⠉⠘⠑⠙";

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "iteration",
    surface: "what one loop iteration is told: setbacks, notes, its curated state, the brief",
    subs: &[
        Sub {
            route: PROGRESS,
            name: "progress",
            signal: "what the last iteration's reply did not establish (one page, its counts beside it)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'1'),
        },
        Sub {
            route: FAULT,
            name: "fault",
            signal: "the last iteration ended in a fault, not a result (one page, the error beside it)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'2'),
        },
        Sub {
            route: PODRACE_STALL,
            name: "podrace-stall",
            signal: "podrace stalled with no comparable objective improvement recorded",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'3'),
        },
        Sub {
            route: RECEIPT,
            name: "receipt",
            signal: "a podrace measurement or submission receipt was recorded",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'4'),
        },
        Sub {
            route: FIRST_CANDIDATE,
            name: "first-candidate",
            signal: "no measured candidate yet (iterations beside the route, a blocked attempt below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'5'),
        },
        Sub {
            route: CURATED,
            name: "curated",
            signal: "the curated state's labels (each sent as its page address, the state beside it)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'6'),
        },
        Sub {
            route: BRIEF,
            name: "brief",
            signal: "the project brief's headings and workspace labels (each sent as its page address)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'7'),
        },
        Sub {
            route: BRIEF_BENCHMARK,
            name: "brief-benchmark",
            signal: "the project brief's benchmark labels (each sent as its page address)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'8'),
        },
        Sub {
            route: LOOP_TOOLS,
            name: "loop-tools",
            signal: "a note from the loop's research or campaign tool (one page, its value beside it)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'9'),
        },
        Sub {
            route: LOOP_TOOL_REFUSALS,
            name: "loop-tool-refusals",
            signal: "loop_research or rl_campaign refused the call: a required field is missing or a value it rejects (one page)",
            action: "correct the named argument and call again",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'0'),
        },
    ],
};
