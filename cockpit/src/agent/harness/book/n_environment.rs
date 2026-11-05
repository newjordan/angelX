//! ⠝ n — environment: what this session's surroundings hold — the verified
//! capabilities, the interactive cockpit's surfaces, the vision sidecar, the
//! workspace's context carriers and the turn's own frames.
//! Every page is its original sentence, verbatim; what a session probes (the
//! PATH, the gates it found) is that route's evidence.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/n_environment.book");
pub(crate) const CELL: char = '⠝';
pub(crate) const CAPABILITIES: Route = Route::new(CELL, '⠁');
pub(crate) const COCKPIT: Route = Route::new(CELL, '⠃');
pub(crate) const VISION: Route = Route::new(CELL, '⠉');
pub(crate) const WORK_UNSET: Route = Route::new(CELL, '⠙');
pub(crate) const WORK_INTERNAL: Route = Route::new(CELL, '⠑');
pub(crate) const WORK_PUBLIC: Route = Route::new(CELL, '⠋');
pub(crate) const PROJECT: Route = Route::new(CELL, '⠛');
pub(crate) const SELF_MODEL: Route = Route::new(CELL, '⠓');
pub(crate) const TURN_CONTEXT: Route = Route::new(CELL, '⠊');
pub(crate) const STEER: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "environment",
    surface: "verified capabilities, the cockpit, vision, workspace context and turn frames",
    subs: &[
        Sub {
            route: CAPABILITIES,
            name: "capabilities",
            signal: "the capabilities verified at session start (evidence below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'1'),
        },
        Sub {
            route: COCKPIT,
            name: "cockpit",
            signal: "the interactive cockpit's visual surfaces",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'2'),
        },
        Sub {
            route: VISION,
            name: "vision",
            signal: "a text-only driver with a vision sidecar",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'3'),
        },
        Sub {
            route: WORK_UNSET,
            name: "work-unset",
            signal: "no confirmed work context for this workspace (detected fields below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'4'),
        },
        Sub {
            route: WORK_INTERNAL,
            name: "work-internal",
            signal: "confirmed internal-dev work context (fields below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'5'),
        },
        Sub {
            route: WORK_PUBLIC,
            name: "work-public",
            signal: "confirmed public-facing work context (fields below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'6'),
        },
        Sub {
            route: PROJECT,
            name: "project",
            signal: "the project's AGENTS.md guidance below this line",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: SELF_MODEL,
            name: "self-model",
            signal: "the map of the cockpit's own source below this line (the tool modules beside their page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'8'),
        },
        Sub {
            route: TURN_CONTEXT,
            name: "turn-context",
            signal: "the operator's controls and standing context, between two of these",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'9'),
        },
        Sub {
            route: STEER,
            name: "steer",
            signal: "the User message(s) after this were sent mid-run",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'0'),
        },
    ],
};
