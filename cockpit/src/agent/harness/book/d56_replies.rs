//! ⠰ (dots 5-6) — Volume VI, the replies: refusals, hints, notes and the long
//! bodies. A refused seat keeps its facts (the seat, what is available) and
//! sends the way forward as a page; the list_dir window keeps its counts and
//! next offset; the knowledge graph, jev and semantic_read keep their limits.
//! The long reply bodies — work landing, the self-map and its safety
//! contract, the benchmark and jev interpretations — are sections of their
//! own, their fields beside the routes as data.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/d56_replies.book");
pub(crate) const CELL: char = '⠰';
pub(crate) const LISTING: Route = Route::new(CELL, '⠁');
pub(crate) const REFUSALS: Route = Route::new(CELL, '⠃');
pub(crate) const SEAT_HINTS: Route = Route::new(CELL, '⠉');
pub(crate) const NOTES: Route = Route::new(CELL, '⠙');
pub(crate) const SERVICES: Route = Route::new(CELL, '⠑');
pub(crate) const WORK_LANDING: Route = Route::new(CELL, '⠋');
pub(crate) const SELF_MAP: Route = Route::new(CELL, '⠛');
pub(crate) const SELF_SAFETY: Route = Route::new(CELL, '⠓');
pub(crate) const BENCHMARK: Route = Route::new(CELL, '⠊');
pub(crate) const JEV: Route = Route::new(CELL, '⠚');

// ⠰⠁ the list_dir window
pub(crate) const NEXT_WINDOW: Page = Page::new(LISTING, 1);
pub(crate) const KEEP_FILTERS: Page = Page::new(LISTING, 2);
pub(crate) const RESTART_LISTING: Page = Page::new(LISTING, 3);
pub(crate) const OPTIONAL_EXCLUSIONS: Page = Page::new(LISTING, 4);

// ⠰⠃ refused seats
pub(crate) const DO_THE_WORK: Page = Page::new(REFUSALS, 1);
pub(crate) const UNSET_PIN: Page = Page::new(REFUSALS, 2);
pub(crate) const OWN_THE_WORKLOAD: Page = Page::new(REFUSALS, 3);
pub(crate) const LOCAL_TOOLS: Page = Page::new(REFUSALS, 4);
pub(crate) const SOLO_OFF: Page = Page::new(REFUSALS, 5);
pub(crate) const PERMIT_CONSULT: Page = Page::new(REFUSALS, 6);
pub(crate) const LISTED_CLUB: Page = Page::new(REFUSALS, 7);

// ⠰⠉ seats, personas, graphs and children
pub(crate) const PERSONA_NAME: Page = Page::new(SEAT_HINTS, 1);
pub(crate) const PERSONA_SEAT: Page = Page::new(SEAT_HINTS, 2);
pub(crate) const PERSONA_NODE: Page = Page::new(SEAT_HINTS, 3);
pub(crate) const GRAPH_SPECS: Page = Page::new(SEAT_HINTS, 4);
pub(crate) const GATE_ORDER: Page = Page::new(SEAT_HINTS, 5);
pub(crate) const SERIALIZE_WRITERS: Page = Page::new(SEAT_HINTS, 6);
pub(crate) const SHARED_WORKSPACE: Page = Page::new(SEAT_HINTS, 7);
pub(crate) const SHARED_RESOURCE: Page = Page::new(SEAT_HINTS, 8);
pub(crate) const INITIAL_COMMIT: Page = Page::new(SEAT_HINTS, 9);
pub(crate) const REVIEW_PRESERVED: Page = Page::new(SEAT_HINTS, 10);

// ⠰⠙ notes and next steps
pub(crate) const NO_BUDGET: Page = Page::new(NOTES, 1);
pub(crate) const AUTO_COMPACT: Page = Page::new(NOTES, 2);
/// `hooks::HOOK_BLOCKED_PREFIX` is this address; the book tests pin it.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const HOOK_BLOCKED: Page = Page::new(NOTES, 3);
pub(crate) const UNKNOWN_OUTCOME: Page = Page::new(NOTES, 4);
pub(crate) const SELF_SRC_ROOT: Page = Page::new(NOTES, 5);
pub(crate) const SELF_SRC: Page = Page::new(NOTES, 6);
pub(crate) const INTEGRATE_VERIFIED: Page = Page::new(NOTES, 7);
pub(crate) const RESUME_RUN: Page = Page::new(NOTES, 8);
pub(crate) const REJECTED_RUN: Page = Page::new(NOTES, 9);
pub(crate) const RESUME_PAUSED: Page = Page::new(NOTES, 10);

// ⠰⠑ the knowledge graph, jev and semantic_read
pub(crate) const KG_STORE: Page = Page::new(SERVICES, 1);
pub(crate) const KG_SUMMARIZE: Page = Page::new(SERVICES, 2);
pub(crate) const KG_INGEST: Page = Page::new(SERVICES, 3);
pub(crate) const JEV_KEY: Page = Page::new(SERVICES, 4);
pub(crate) const JEV_NARROW: Page = Page::new(SERVICES, 5);
pub(crate) const JEV_CREDENTIAL: Page = Page::new(SERVICES, 6);
pub(crate) const JEV_BUDGET: Page = Page::new(SERVICES, 7);
pub(crate) const JEV_NOUL: Page = Page::new(SERVICES, 8);
pub(crate) const SEMANTIC_EVIDENCE: Page = Page::new(SERVICES, 9);

// ⠰⠋ work landing
pub(crate) const LANDING_DETECTED: Page = Page::new(WORK_LANDING, 1);
pub(crate) const LANDING_CONFIRM: Page = Page::new(WORK_LANDING, 2);
pub(crate) const LANDING_RECORD: Page = Page::new(WORK_LANDING, 3);
pub(crate) const LANDING_ASK_REPO: Page = Page::new(WORK_LANDING, 4);
pub(crate) const LANDING_ASK_INTERNAL: Page = Page::new(WORK_LANDING, 5);
pub(crate) const LANDING_ASK_VISIBILITY: Page = Page::new(WORK_LANDING, 6);
pub(crate) const LANDING_PIN_MODE: Page = Page::new(WORK_LANDING, 7);
pub(crate) const LANDING_CONFIRMED: Page = Page::new(WORK_LANDING, 8);
pub(crate) const LANDING_DRIVES: Page = Page::new(WORK_LANDING, 9);

// ⠰⠛ the self-map
pub(crate) const MAP_TITLE: Page = Page::new(SELF_MAP, 1);
pub(crate) const MAP_HARNESS: Page = Page::new(SELF_MAP, 2);
pub(crate) const MAP_LIVE: Page = Page::new(SELF_MAP, 3);
pub(crate) const MAP_BUILD: Page = Page::new(SELF_MAP, 4);
pub(crate) const MAP_COMMANDS: Page = Page::new(SELF_MAP, 5);
pub(crate) const MAP_BINARY: Page = Page::new(SELF_MAP, 6);
pub(crate) const MAP_GIT_ROOT: Page = Page::new(SELF_MAP, 7);
pub(crate) const MAP_MODULES: Page = Page::new(SELF_MAP, 8);
pub(crate) const MAP_TOOLS: Page = Page::new(SELF_MAP, 9);
pub(crate) const MAP_FOOTER: Page = Page::new(SELF_MAP, 10);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "replies",
    surface: "refused seats, listing windows, notes, service limits and the long reply bodies",
    subs: &[
        Sub {
            route: LISTING,
            name: "listing",
            signal: "list_dir paged its window or refused a path (the window above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: REFUSALS,
            name: "refusals",
            signal: "a paid or remote seat was refused (the seat and what is available above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'2'),
        },
        Sub {
            route: SEAT_HINTS,
            name: "seat-hints",
            signal: "a persona, graph, child set or delegation was refused (the refusal above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'3'),
        },
        Sub {
            route: NOTES,
            name: "notes",
            signal: "a note or next step on the facts above",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'4'),
        },
        Sub {
            route: SERVICES,
            name: "services",
            signal: "the knowledge graph, jev or semantic_read answered with a limit (the fact above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'5'),
        },
        Sub {
            route: WORK_LANDING,
            name: "work-landing",
            signal: "work_landing detected or recorded the work context (fields below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'6'),
        },
        Sub {
            route: SELF_MAP,
            name: "self-map",
            signal: "the map of the harness's own source (fields and modules beside and below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'7'),
        },
        Sub {
            route: SELF_SAFETY,
            name: "self-safety",
            signal: "the self-modification safety contract",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'8'),
        },
        Sub {
            route: BENCHMARK,
            name: "benchmark-interpretation",
            signal: "how to read a benchmark_compare result",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'9'),
        },
        Sub {
            route: JEV,
            name: "jev-interpretation",
            signal: "how to read a jev_decide answer",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'0'),
        },
    ],
};
