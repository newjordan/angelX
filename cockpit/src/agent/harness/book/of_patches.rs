//! ⠷ of — Volume II, the tool library: changing files — writes, batched
//! edits, patches, staged edits, commits and merges.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/of_patches.book");
pub(crate) const CELL: char = '⠷';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "patches",
    surface: "writes, batched edits, patches, staged edits, commits and merges",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "write_file",
            signal: "using `write_file`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "multi_edit",
            signal: "using `multi_edit`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "apply_patch",
            signal: "using `apply_patch`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "resolve_edit",
            signal: "using `resolve_edit`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "git_commit",
            signal: "using `git_commit`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "integrate",
            signal: "using `integrate`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'6'),
        },
    ],
};
