//! ⠬ ing — drivers around a turn: the deli frame the in-turn deliberation hands
//! its action seat, and the tutor's framing of an educational question; the
//! deli's synthesis, the tutor's lesson handoff and its recall practice, and
//! the operator's stand-ins — the words the harness writes in the operator's
//! place for `/review` and `/quest gauntlet`.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/ing_drivers.book");
pub(crate) const CELL: char = '⠬';
pub(crate) const DELI: Route = Route::new(CELL, '⠁');
pub(crate) const TUTOR: Route = Route::new(CELL, '⠃');
pub(crate) const PREFLIGHT: Route = Route::new(CELL, '⠉');
pub(crate) const REVIEW: Route = Route::new(CELL, '⠙');
pub(crate) const OUTPUT_CONTRACT: Route = Route::new(CELL, '⠑');
pub(crate) const DELI_SYNTHESIS: Route = Route::new(CELL, '⠋');
pub(crate) const TUTOR_HANDOFF: Route = Route::new(CELL, '⠛');
pub(crate) const TUTOR_RECALL: Route = Route::new(CELL, '⠓');
pub(crate) const TUTOR_FALLBACK: Route = Route::new(CELL, '⠊');
pub(crate) const STAND_INS: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "drivers",
    surface: "drivers: the deli frame and synthesis, the tutor, the operator's stand-ins",
    subs: &[
        Sub {
            route: DELI,
            name: "deli",
            signal: "the deli deliberation (findings and open leads below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'1'),
        },
        Sub {
            route: TUTOR,
            name: "tutor",
            signal: "an educational question (context and question below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'2'),
        },
        Sub {
            route: PREFLIGHT,
            name: "preflight",
            signal: "the formation's preflight advisory (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'3'),
        },
        Sub {
            route: REVIEW,
            name: "review",
            signal: "the operator's review request (worktree snapshot below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'4'),
        },
        Sub {
            route: OUTPUT_CONTRACT,
            name: "output-contract",
            signal: "an extraction-shaped ask: return only what it asks for",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'5'),
        },
        Sub {
            route: DELI_SYNTHESIS,
            name: "deli-synthesis",
            signal: "fuse the deli's findings and open leads below into the final answer",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'6'),
        },
        Sub {
            route: TUTOR_HANDOFF,
            name: "tutor-handoff",
            signal: "the tutor's lesson handoff (each value beside its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'7'),
        },
        Sub {
            route: TUTOR_RECALL,
            name: "tutor-recall",
            signal: "a lesson's recall practice (the tutor beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'8'),
        },
        Sub {
            route: TUTOR_FALLBACK,
            name: "tutor-fallback",
            signal: "a tutor question with no lesson open and no reference retrieved",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'9'),
        },
        Sub {
            route: STAND_INS,
            name: "stand-ins",
            signal: "the harness's words in the operator's place: the `/review` task, the `/quest gauntlet` steer",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'0'),
        },
    ],
};
