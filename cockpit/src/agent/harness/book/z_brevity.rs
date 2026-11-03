//! ⠵ z — brevity: the caveman mode, its levels and its rules. Every page is its
//! original sentence, verbatim. The link layer splices the marker, one level
//! and the rules, in the original order (`⠵⠁⠵⠉⠵⠋` for full).

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/z_brevity.book");
pub(crate) const CELL: char = '⠵';
pub(crate) const MARKER: Route = Route::new(CELL, '⠁');
pub(crate) const LITE: Route = Route::new(CELL, '⠃');
pub(crate) const FULL: Route = Route::new(CELL, '⠉');
pub(crate) const ULTRA: Route = Route::new(CELL, '⠙');
pub(crate) const WENYAN: Route = Route::new(CELL, '⠑');
pub(crate) const RULES: Route = Route::new(CELL, '⠋');
pub(crate) const AGED: Route = Route::new(CELL, '⠛');
pub(crate) const COMPACTED: Route = Route::new(CELL, '⠓');
pub(crate) const PLAN: Route = Route::new(CELL, '⠊');
pub(crate) const CONSTRAINTS: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "brevity",
    surface: "brevity: caveman rules and levels, and the compacted context",
    subs: &[
        Sub {
            route: MARKER,
            name: "marker",
            signal: "the brevity mode and its level",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'1'),
        },
        Sub {
            route: LITE,
            name: "lite",
            signal: "the lite level",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'2'),
        },
        Sub {
            route: FULL,
            name: "full",
            signal: "the full level",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'3'),
        },
        Sub {
            route: ULTRA,
            name: "ultra",
            signal: "the ultra level",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'4'),
        },
        Sub {
            route: WENYAN,
            name: "wenyan",
            signal: "the wenyan levels",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'5'),
        },
        Sub {
            route: RULES,
            name: "rules",
            signal: "the brevity rules",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'6'),
        },
        Sub {
            route: AGED,
            name: "aged",
            signal: "a tool output was elided from context (its identity and handle inside the bracket)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'7'),
        },
        Sub {
            route: COMPACTED,
            name: "compacted",
            signal: "earlier conversation, compacted to the notes below",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: PLAN,
            name: "plan",
            signal: "the model's own working plan, carried across compaction",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'9'),
        },
        Sub {
            route: CONSTRAINTS,
            name: "constraints",
            signal: "the operator's protected task and constraints follow as anchors",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'0'),
        },
    ],
};
