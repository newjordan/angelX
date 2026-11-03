//! ⠮ the — Volume II, the tool library: sight — the language servers and the
//! cockpit screen, then the continued section of video_cut.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/the_sight.book");
pub(crate) const CELL: char = '⠮';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "sight",
    surface: "language servers and the cockpit screen; video_cut, continued",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "lsp_definition",
            signal: "using `lsp_definition`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "lsp_references",
            signal: "using `lsp_references`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "lsp_hover",
            signal: "using `lsp_hover`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "lsp_symbols",
            signal: "using `lsp_symbols`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "lsp_workspace_symbol",
            signal: "using `lsp_workspace_symbol`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "ui_inspect",
            signal: "using `ui_inspect`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "ui_verify",
            signal: "using `ui_verify`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "video_cut, continued",
            signal: "using `video_cut`, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'8'),
        },
    ],
};
