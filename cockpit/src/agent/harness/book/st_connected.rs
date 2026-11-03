//! ⠌ st — the connected seats: models the harness calls without workspace
//! tools. Each one is offered the ledger reader alone ([`super::connect`]), so
//! its prompt is routes like everyone else's: the bare spawn seat and graph
//! node, the aggregator, code review, the text-only worker's
//! contract, the compaction summarizer, the final-answer advisor, and a
//! mixture stage that answered with a tool call. The task, drafts, code and
//! excerpts ride beside the routes as data.
//!
//! ⡌ (dots 3-4-7) is this chapter's overflow shelf: the Treebeard compactor, a
//! local seat that digests the bulk the lane parks under a handle, and the
//! frame its digest rides under in the root's receipt (or, when it lands after
//! that receipt was sent, under its handle at the tail). The same seat reads a
//! foreground call the harness handed to the background after it ran past its
//! limit, and that read rides the hand-off receipt the same way. The loop
//! watchdog's seat shelves here too: it reads a wedged iteration's facts and
//! names how to free the harness, never how to do the loop's task.

use super::{Primary, Route, Sub};
const BOOK_PRIMARY: &str = include_str!("../../../../book/connected_primary.book");
const BOOK_SHELF: &str = include_str!("../../../../book/connected_shelf.book");

pub(crate) const CELL: char = '⠌';
pub(crate) const BARE_SEAT: Route = Route::new(CELL, '⠁');
pub(crate) const BARE_NODE: Route = Route::new(CELL, '⠃');
pub(crate) const AGGREGATOR: Route = Route::new(CELL, '⠉');
pub(crate) const CODE_REVIEW: Route = Route::new(CELL, '⠙');
pub(crate) const REASONING_CONTRACT: Route = Route::new(CELL, '⠑');
pub(crate) const SUMMARIZER: Route = Route::new(CELL, '⠋');
pub(crate) const SUMMARY_SECTIONS: Route = Route::new(CELL, '⠛');
pub(crate) const MAP_NOTES: Route = Route::new(CELL, '⠓');
pub(crate) const ADVISOR: Route = Route::new(CELL, '⠊');
pub(crate) const TEXT_ONLY: Route = Route::new(CELL, '⠚');

pub(crate) const SHELF_CELL: char = '⡌';
pub(crate) const COMPACTOR: Route = Route::new(SHELF_CELL, '⠁');
pub(crate) const COMPACTOR_DIGEST: Route = Route::new(SHELF_CELL, '⠃');
pub(crate) const COMPACTOR_LATE: Route = Route::new(SHELF_CELL, '⠉');
pub(crate) const HANDOFF: Route = Route::new(SHELF_CELL, '⠙');
pub(crate) const OVERSEER: Route = Route::new(SHELF_CELL, '⠑');
pub(crate) const OVERSEER_READ: Route = Route::new(SHELF_CELL, '⠋');
pub(crate) const WATCHDOG: Route = Route::new(SHELF_CELL, '⠛');
pub(crate) const WATCHDOG_STOP: Route = Route::new(SHELF_CELL, '⠓');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "connected",
    surface: "seats without workspace tools, connected through the ledger reader",
    subs: &[
        Sub {
            route: BARE_SEAT,
            name: "bare-seat",
            signal: "a spawn seat with no workspace tools (the ledger reader only)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(BOOK_PRIMARY, b'1'),
        },
        Sub {
            route: BARE_NODE,
            name: "bare-node",
            signal: "a graph node with no workspace tools (the ledger reader only)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_PRIMARY, b'2'),
        },
        Sub {
            route: AGGREGATOR,
            name: "aggregator",
            signal: "the aggregator of a spawn formation (task and drafts below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(BOOK_PRIMARY, b'3'),
        },
        Sub {
            route: CODE_REVIEW,
            name: "code-review",
            signal: "an adversarial code review (focus beside the route, code below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(BOOK_PRIMARY, b'4'),
        },
        Sub {
            route: REASONING_CONTRACT,
            name: "reasoning-contract",
            signal: "the text-only worker's output contract",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(BOOK_PRIMARY, b'5'),
        },
        Sub {
            route: SUMMARIZER,
            name: "summarizer",
            signal: "distill the conversation excerpt below into the sections",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(BOOK_PRIMARY, b'6'),
        },
        Sub {
            route: SUMMARY_SECTIONS,
            name: "summary-sections",
            signal: "the summary's sections, in order",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(BOOK_PRIMARY, b'7'),
        },
        Sub {
            route: MAP_NOTES,
            name: "map-notes",
            signal: "note the conversation excerpt below densely",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(BOOK_PRIMARY, b'8'),
        },
        Sub {
            route: ADVISOR,
            name: "advisor",
            signal: "the final-answer advisor (the task and the proposed answer below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(BOOK_PRIMARY, b'9'),
        },
        Sub {
            route: TEXT_ONLY,
            name: "text-only",
            signal: "a text-only mixture stage: no workspace tools; after a tool call, the correction",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(BOOK_PRIMARY, b'0'),
        },
    ],
};

pub(crate) const SHELF: Primary = Primary {
    cell: SHELF_CELL,
    name: "compactor",
    surface: "the Treebeard compactor: a local seat that digests parked bulk so the root carries only what it needs",
    subs: &[
        Sub {
            route: COMPACTOR,
            name: "compactor",
            signal: "digest the parked tool output below for the root (its call beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(BOOK_SHELF, b'1'),
        },
        Sub {
            route: COMPACTOR_DIGEST,
            name: "compactor-digest",
            signal: "the local compactor's digest of the parked output above; its handle keeps the full body",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_SHELF, b'2'),
        },
        Sub {
            route: COMPACTOR_LATE,
            name: "compactor-late",
            signal: "the local compactor's digest of an earlier parked output, named by the handle beside the route",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_SHELF, b'3'),
        },
        Sub {
            route: HANDOFF,
            name: "handoff",
            signal: "a foreground call ran past its limit and now runs as the background job named above; nothing was stopped",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_SHELF, b'4'),
        },
        Sub {
            route: OVERSEER,
            name: "overseer",
            signal: "read the handed-off job below for the root (its facts beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(BOOK_SHELF, b'5'),
        },
        Sub {
            route: OVERSEER_READ,
            name: "overseer-read",
            signal: "the local compactor's read of a handed-off job: the receipt above, or the job named beside the route",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_SHELF, b'6'),
        },
        Sub {
            route: WATCHDOG,
            name: "watchdog",
            signal: "a loop iteration has made no harness progress for a long time; say whether the harness is wedged and how to free it (its facts below the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(BOOK_SHELF, b'7'),
        },
        Sub {
            route: WATCHDOG_STOP,
            name: "watchdog-stop",
            signal: "the loop watchdog saw no harness progress on this call for a long time and stopped it; the turn continues",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_SHELF, b'8'),
        },
    ],
};
