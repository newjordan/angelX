//! ⠱ wh — Volume II, the tool library: the web, the literature, repositories,
//! endpoints and the rig fleet. One section per tool; its pages are the
//! sentences that told the model how to use that tool, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/wh_world.book");
pub(crate) const CELL: char = '⠱';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "world",
    surface: "the world beyond the workspace",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "web_search",
            signal: "using `web_search`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "web_fetch",
            signal: "using `web_fetch`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "science_search",
            signal: "using `science_search`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "repo_search",
            signal: "using `repo_search`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "grok_research",
            signal: "using `grok_research`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "http_request",
            signal: "using `http_request`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "fleet_status",
            signal: "using `fleet_status`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'7'),
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "vast_instances",
            signal: "using `vast_instances`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'8'),
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "llm_probe",
            signal: "using `llm_probe`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'9'),
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "llm_bench",
            signal: "using `llm_bench`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'0'),
        },
    ],
};
