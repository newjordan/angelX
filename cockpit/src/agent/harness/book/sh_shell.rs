//! ⠩ sh — Volume II, the tool library: the shell, background processes,
//! verifier runners, experiments and measured comparisons. One section per
//! tool; its pages are the sentences that told the model how to use that tool,
//! verbatim, moved off the schema. The schema keeps what the tool does and ends
//! with the section's route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/sh_shell.book");
pub(crate) const CELL: char = '⠩';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "shell",
    surface: "the shell, processes, runs and measurements",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "shell",
            signal: "using `shell`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "cargo",
            signal: "using `cargo`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "lint",
            signal: "using `lint`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "proc_run",
            signal: "using `proc_run`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "proc_status",
            signal: "using `proc_status`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "proc_wait",
            signal: "using `proc_wait`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "tool_repair",
            signal: "using `tool_repair`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'7'),
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "loop_research",
            signal: "using `loop_research`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'8'),
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "rl_campaign",
            signal: "using `rl_campaign`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'9'),
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "benchmark_compare",
            signal: "using `benchmark_compare`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'0'),
        },
    ],
};
