//! ⠾ with — Volume II, the tool library: building and running — tests,
//! checks, formatting and stopping background jobs, then the continued
//! sections of the shell and loop tools.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/with_build.book");
pub(crate) const CELL: char = '⠾';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "build",
    surface: "tests, checks, formatting and stopping jobs; the shell and loop tools, continued",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "run_tests",
            signal: "using `run_tests`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "check",
            signal: "using `check`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "fmt",
            signal: "using `fmt`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "proc_stop",
            signal: "using `proc_stop`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "shell, continued",
            signal: "using `shell`, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "loop_research, continued",
            signal: "using `loop_research`, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "rl_campaign, continued",
            signal: "using `rl_campaign`, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'7'),
        },
    ],
};
