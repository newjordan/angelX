//! ⠳ ou — the loop's checkpoints: what outranks the next direction (a blocked
//! verifier, an overdue submission, the evidence review), how an iteration may
//! end, and the deep experiment in flight. The facts ride beside the route.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/ou_checkpoints.book");
pub(crate) const CELL: char = '⠳';
pub(crate) const BLOCKED: Route = Route::new(CELL, '⠁');
pub(crate) const OVERDUE: Route = Route::new(CELL, '⠃');
pub(crate) const WINDOW: Route = Route::new(CELL, '⠉');
pub(crate) const REVIEW: Route = Route::new(CELL, '⠙');
pub(crate) const SETBACK: Route = Route::new(CELL, '⠑');
pub(crate) const DONE_SELF: Route = Route::new(CELL, '⠋');
pub(crate) const DONE_ACCEPT: Route = Route::new(CELL, '⠛');
pub(crate) const DONE_OPEN: Route = Route::new(CELL, '⠓');
pub(crate) const DEEP: Route = Route::new(CELL, '⠊');
pub(crate) const NOTE: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "checkpoints",
    surface: "loop checkpoints: blocked, overdue, window, review, setback, done, experiments",
    subs: &[
        Sub {
            route: BLOCKED,
            name: "blocked",
            signal: "the verification path is blocked (diagnostic below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: OVERDUE,
            name: "overdue",
            signal: "measured candidates not yet submitted (counts beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'2'),
        },
        Sub {
            route: WINDOW,
            name: "window",
            signal: "the oldest findings and directions are elided (counts beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'3'),
        },
        Sub {
            route: REVIEW,
            name: "review",
            signal: "the 15-iteration evidence review",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'4'),
        },
        Sub {
            route: SETBACK,
            name: "setback",
            signal: "the previous iteration's setback (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'5'),
        },
        Sub {
            route: DONE_SELF,
            name: "done-self-edit",
            signal: "the loop improves the cockpit's own source",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'6'),
        },
        Sub {
            route: DONE_ACCEPT,
            name: "done-accept",
            signal: "a verifiable acceptance check is bound",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'7'),
        },
        Sub {
            route: DONE_OPEN,
            name: "done-open",
            signal: "no acceptance check is bound",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: DEEP,
            name: "deep-experiment",
            signal: "a deep experiment is in flight (its record below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'9'),
        },
        Sub {
            route: NOTE,
            name: "loop-note",
            signal: "a loop note (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'0'),
        },
    ],
};
