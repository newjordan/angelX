//! ⠕ o — orchestration: delegate routes, `delegate`, `spawn` and
//! `swarm_compile`, with a campaign's final alignment gate (`⠕⠊`, its verdict
//! contract `⠕⠚`). Every page is its original sentence, verbatim. The
//! configured routes are the `⠕⠃` / `⠕⠋` evidence.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/o_orchestration.book");
pub(crate) const CELL: char = '⠕';
pub(crate) const NO_ROUTES: Route = Route::new(CELL, '⠁');
pub(crate) const ROUTES: Route = Route::new(CELL, '⠃');
pub(crate) const DELEGATE: Route = Route::new(CELL, '⠉');
pub(crate) const SPAWN: Route = Route::new(CELL, '⠙');
pub(crate) const SWARM_COMPILE: Route = Route::new(CELL, '⠑');
pub(crate) const ROUTES_COMPACT: Route = Route::new(CELL, '⠋');
pub(crate) const SEAT_SKIPPED: Route = Route::new(CELL, '⠛');
pub(crate) const DELI_CONSULT: Route = Route::new(CELL, '⠓');
pub(crate) const ALIGNMENT: Route = Route::new(CELL, '⠊');
pub(crate) const ALIGNMENT_VERDICT: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "orchestration",
    surface: "delegate routes, delegate, spawn, swarm_compile, seats, consults, the alignment gate",
    subs: &[
        Sub {
            route: NO_ROUTES,
            name: "no-routes",
            signal: "no delegate routes are configured",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'1'),
        },
        Sub {
            route: ROUTES,
            name: "routes",
            signal: "the configured delegate routes (evidence below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'2'),
        },
        Sub {
            route: DELEGATE,
            name: "delegate",
            signal: "the delegate tool",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: SPAWN,
            name: "spawn",
            signal: "the spawn tool",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'4'),
        },
        Sub {
            route: SWARM_COMPILE,
            name: "swarm-compile",
            signal: "the swarm_compile tool",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'5'),
        },
        Sub {
            route: ROUTES_COMPACT,
            name: "routes-compact",
            signal: "the configured delegate routes, compact core (evidence below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'6'),
        },
        Sub {
            route: SEAT_SKIPPED,
            name: "seat-skipped",
            signal: "the requested seat is not reachable (live seats above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'7'),
        },
        Sub {
            route: DELI_CONSULT,
            name: "deli-consult",
            signal: "a deli-method consult",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'8'),
        },
        Sub {
            route: ALIGNMENT,
            name: "alignment-gate",
            signal: "the campaign's final alignment gate for the parked candidate (contract below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'9'),
        },
        Sub {
            route: ALIGNMENT_VERDICT,
            name: "alignment-verdict",
            signal: "the alignment gate's verdict contract",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'0'),
        },
    ],
};
