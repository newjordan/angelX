//! ⠡ ch — Volume II, the tool library: reading, editing and committing files,
//! and the notes that outlive a session. One section per tool; its pages are
//! the sentences that told the model how to use that tool, verbatim, moved off
//! the schema. The schema keeps what the tool does and ends with the section's
//! route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/ch_edits.book");
pub(crate) const CELL: char = '⠡';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "edits",
    surface: "files, edits, git and notes",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "read_file",
            signal: "using `read_file`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "str_replace",
            signal: "using `str_replace`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "git_diff",
            signal: "using `git_diff`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "git_log",
            signal: "using `git_log`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "file_search",
            signal: "using `file_search`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "semantic_read",
            signal: "using `semantic_read`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "lsp_diagnostics",
            signal: "using `lsp_diagnostics`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "notes",
            signal: "using `notes`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "handoff",
            signal: "using `handoff`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'9'),
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "recall",
            signal: "using `recall`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'0'),
        },
    ],
};

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__tools_tests.rs"]
mod tests;
