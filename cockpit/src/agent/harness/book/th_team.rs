//! ⠹ th — Volume II, the tool library: delegates, consults, formations and
//! graphs, and the harness tools that batch, park and find. One section per
//! tool; its pages are the sentences that told the model how to use that tool,
//! verbatim, moved off the schema. The schema keeps what the tool does and ends
//! with the section's route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/th_team.book");
pub(crate) const CELL: char = '⠹';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "team",
    surface: "the team and the harness's own tools",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "delegate",
            signal: "using `delegate`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "swarm_compile",
            signal: "using `swarm_compile`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "consult_model",
            signal: "using `consult_model`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "agent_graph",
            signal: "using `agent_graph`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "spawn",
            signal: "using `spawn`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "code_mode",
            signal: "using `code_mode`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "handle_read",
            signal: "using `handle_read`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'7'),
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "tool_search",
            signal: "using `tool_search`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'8'),
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "skill",
            signal: "using `skill`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'9'),
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "jev_decide",
            signal: "using `jev_decide`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'0'),
        },
    ],
};
