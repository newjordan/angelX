//! ⠪ ow — the loop's ledgers: what this run changed, measured and was told,
//! what background jobs returned, and the RL campaigns and loop research
//! beside it. The ledger bodies ride beside the routes; an outcome's notes
//! (`⠪⠊`) ride beside its facts as page addresses.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/ow_ledgers.book");
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
            pages: &super::catalog_pages::<1>(CATALOG, b'1'),
        },
        Sub {
            route: MEASUREMENTS,
            name: "measurements",
            signal: "measurements and submissions this run (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'2'),
        },
        Sub {
            route: STEER_STANDING,
            name: "steer-standing",
            signal: "operator notes already answered (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'3'),
        },
        Sub {
            route: STEER_FRESH,
            name: "steer-fresh",
            signal: "operator notes not yet answered (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'4'),
        },
        Sub {
            route: OUTCOMES,
            name: "outcomes",
            signal: "background process outcomes (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'5'),
        },
        Sub {
            route: RL,
            name: "rl-campaigns",
            signal: "rl_campaign is available in this loop (history below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'6'),
        },
        Sub {
            route: RESEARCH_PATHS,
            name: "research-paths",
            signal: "optional research paths",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: LOOP_RESEARCH,
            name: "loop-research",
            signal: "loop_research is available in this loop (history below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'8'),
        },
        Sub {
            route: OUTCOME_NOTES,
            name: "outcome-notes",
            signal: "notes on a background or experiment outcome (one page, beside its facts)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'9'),
        },
        Sub {
            route: RESEARCH_NOTES,
            name: "research-notes",
            signal: "loop_research's notes on its options and runs (page addresses in its results)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'0'),
        },
    ],
};
