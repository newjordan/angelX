//! ⠞ t — personality: the Driver's identity and standing posture. Every page
//! is its original sentence, verbatim; the compact core keeps its own wording
//! in its own sections. Personality types (`⠽`) bundle these sections.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/t_personality.book");
pub(crate) const CELL: char = '⠞';
pub(crate) const IDENTITY: Route = Route::new(CELL, '⠁');
pub(crate) const IDENTITY_COMPACT: Route = Route::new(CELL, '⠃');
pub(crate) const POSTURE: Route = Route::new(CELL, '⠉');
pub(crate) const POSTURE_COMPACT: Route = Route::new(CELL, '⠙');
pub(crate) const FLEET: Route = Route::new(CELL, '⠑');
pub(crate) const PROVENANCE: Route = Route::new(CELL, '⠋');
pub(crate) const TIER_FAST: Route = Route::new(CELL, '⠛');
pub(crate) const TIER_PRO: Route = Route::new(CELL, '⠓');
pub(crate) const PLAN: Route = Route::new(CELL, '⠊');
pub(crate) const STYLE: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "personality",
    surface: "the Driver's identity and standing posture",
    subs: &[
        Sub {
            route: IDENTITY,
            name: "identity",
            signal: "who the model is",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'1'),
        },
        Sub {
            route: IDENTITY_COMPACT,
            name: "identity-compact",
            signal: "who the model is, compact core",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'2'),
        },
        Sub {
            route: POSTURE,
            name: "posture",
            signal: "the default posture",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: POSTURE_COMPACT,
            name: "posture-compact",
            signal: "the default posture, compact core",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'4'),
        },
        Sub {
            route: FLEET,
            name: "fleet",
            signal: "local seats",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'5'),
        },
        Sub {
            route: PROVENANCE,
            name: "provenance",
            signal: "workspace context provenance",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'6'),
        },
        Sub {
            route: TIER_FAST,
            name: "tier-fast",
            signal: "this turn runs on a fast tier with a stronger seat behind it",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'7'),
        },
        Sub {
            route: TIER_PRO,
            name: "tier-pro",
            signal: "this turn runs on the top tier available",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: PLAN,
            name: "plan",
            signal: "the operator armed plan mode",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'9'),
        },
        Sub {
            route: STYLE,
            name: "style",
            signal: "the operator selected a communication style (its label below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'0'),
        },
    ],
};
