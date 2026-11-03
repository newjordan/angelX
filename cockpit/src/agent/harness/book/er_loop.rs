//! ⠻ er — the loop iteration (`/loop`): what one fresh-context worker is, the
//! ground it breaks, its output contract, the brief, and the tier it runs on.
//! Every page is its original sentence, verbatim; the problem, findings, leads
//! and directions are the data that ride beside the routes. The text-only
//! deli worker reads the same routes through the ledger reader, with its own
//! contract on `⠌⠑`.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/er_loop.book");
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
            pages: &super::catalog_pages::<7>(CATALOG, b'1'),
        },
        Sub {
            route: GROUND,
            name: "ground",
            signal: "break new ground on the problem above",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'2'),
        },
        Sub {
            route: OPEN_LEADS,
            name: "open-leads",
            signal: "the open leads above are unverified",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'3'),
        },
        Sub {
            route: PIVOT,
            name: "pivot",
            signal: "the recent directions have stalled (your option)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'4'),
        },
        Sub {
            route: CONTRACT,
            name: "contract",
            signal: "the iteration's output contract",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'5'),
        },
        Sub {
            route: BRIEF,
            name: "brief",
            signal: "the harness-gathered project brief (its age beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'6'),
        },
        Sub {
            route: TIER_SWARM,
            name: "tier-swarm",
            signal: "the single-agent tier stalled; a wider mixture runs",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'7'),
        },
        Sub {
            route: TIER_SOTA,
            name: "tier-sota",
            signal: "the operator-selected SOTA tier runs",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'8'),
        },
        Sub {
            route: COMPETITION_WORKER,
            name: "competition-worker",
            signal: "the active cartridge's loop worker",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'9'),
        },
        Sub {
            route: PODRACE,
            name: "podrace",
            signal: "podrace mode",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'0'),
        },
    ],
};
