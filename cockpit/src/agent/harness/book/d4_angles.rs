//! ⠈ (dots 4) — Volume IV, the mixture: its angles. The six persona lenses a
//! proposer is handed, the stances that overlay them, the frame of a proposer
//! wave (the research scout's context, the disagreement frontier), the frame
//! of a refine layer (its emphasis, the critic's weaknesses), and the live
//! research scout that grounds a wave. A persona is sent as its route, a
//! stance as its pages; the scout, a CLI seat with no tool channel, hears its
//! pages recited (`book::connect::recite`).

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/d4_angles.book");
pub(crate) const CELL: char = '⠈';
pub(crate) const FIRST_PRINCIPLES: Route = Route::new(CELL, '⠁');
pub(crate) const RED_TEAM: Route = Route::new(CELL, '⠃');
pub(crate) const PRAGMATIST: Route = Route::new(CELL, '⠉');
pub(crate) const LATERAL: Route = Route::new(CELL, '⠙');
pub(crate) const SYSTEMS: Route = Route::new(CELL, '⠑');
pub(crate) const EMPIRICIST: Route = Route::new(CELL, '⠋');
pub(crate) const STANCES: Route = Route::new(CELL, '⠛');
pub(crate) const WAVE: Route = Route::new(CELL, '⠓');
pub(crate) const REFINE: Route = Route::new(CELL, '⠊');
pub(crate) const SCOUT: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "angles",
    surface: "the mixture's angles: personas, stances, the wave and refine frames, the scout",
    subs: &[
        Sub {
            route: FIRST_PRINCIPLES,
            name: "first-principles",
            signal: "the first-principles lens",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'1'),
        },
        Sub {
            route: RED_TEAM,
            name: "red-team",
            signal: "the red-team lens",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'2'),
        },
        Sub {
            route: PRAGMATIST,
            name: "pragmatist",
            signal: "the pragmatist lens",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'3'),
        },
        Sub {
            route: LATERAL,
            name: "lateral",
            signal: "the lateral lens",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'4'),
        },
        Sub {
            route: SYSTEMS,
            name: "systems",
            signal: "the systems lens",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'5'),
        },
        Sub {
            route: EMPIRICIST,
            name: "empiricist",
            signal: "the empiricist lens",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'6'),
        },
        Sub {
            route: STANCES,
            name: "stances",
            signal: "a stance overlaid on a lens, sent as its pages",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'7'),
        },
        Sub {
            route: WAVE,
            name: "wave",
            signal: "a proposer wave's frame: the scout's sources, the disagreement frontier",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'8'),
        },
        Sub {
            route: REFINE,
            name: "refine",
            signal: "a refine layer's frame: its emphasis, the critic's weaknesses",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'9'),
        },
        Sub {
            route: SCOUT,
            name: "scout",
            signal: "the live research scout, recited to a seat with no tool channel",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'0'),
        },
    ],
};
