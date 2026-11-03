//! ⠯ and — Volume II, the tool library: the session — the task list, the
//! standing goal, the context gauge, code review and the text utilities, then
//! the continued section of code_mode.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/and_session.book");
pub(crate) const CELL: char = '⠯';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "session",
    surface: "the task list, the goal, the context gauge, reviews and text utilities; code_mode, continued",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "todo",
            signal: "using `todo`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "goal",
            signal: "using `goal`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "get_context_remaining",
            signal: "using `get_context_remaining`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "code_review",
            signal: "using `code_review`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "reverse",
            signal: "using `reverse`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "word_count",
            signal: "using `word_count`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "code_mode, continued",
            signal: "using `code_mode`, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
    ],
};
