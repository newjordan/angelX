//! ⠍ m — method: the task pace, the coding repair discipline, the frames
//! of the workspace map and the recon snapshot, and the Treebeard lane. Every page is its original
//! sentence, verbatim; a section's pages rebuild the block it came from.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/m_method.book");
pub(crate) const CELL: char = '⠍';
pub(crate) const RAPID: Route = Route::new(CELL, '⠁');
pub(crate) const DEEP: Route = Route::new(CELL, '⠃');
pub(crate) const REPAIR_MAP: Route = Route::new(CELL, '⠉');
pub(crate) const REPAIR_VERIFY: Route = Route::new(CELL, '⠙');
pub(crate) const REPAIR_BATCH: Route = Route::new(CELL, '⠑');
pub(crate) const REPAIR_BACKGROUND: Route = Route::new(CELL, '⠋');
pub(crate) const REPAIR_REPORT: Route = Route::new(CELL, '⠛');
pub(crate) const MAP_NOTE: Route = Route::new(CELL, '⠓');
pub(crate) const RECON: Route = Route::new(CELL, '⠊');
pub(crate) const TREEBEARD: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "method",
    surface: "task pace, coding repair discipline, workspace map and recon frames, the Treebeard lane",
    subs: &[
        Sub {
            route: RAPID,
            name: "rapid",
            signal: "the rapid task pace",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'1'),
        },
        Sub {
            route: DEEP,
            name: "deep",
            signal: "the deep task pace",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'2'),
        },
        Sub {
            route: REPAIR_MAP,
            name: "repair-map",
            signal: "coding repair: mapping and editing",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: REPAIR_VERIFY,
            name: "repair-verify",
            signal: "coding repair: verification",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'4'),
        },
        Sub {
            route: REPAIR_BATCH,
            name: "repair-batch",
            signal: "coding repair: batching, evidence and narration",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'5'),
        },
        Sub {
            route: REPAIR_BACKGROUND,
            name: "repair-background",
            signal: "coding repair: background work",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'6'),
        },
        Sub {
            route: REPAIR_REPORT,
            name: "repair-report",
            signal: "coding repair: prerequisites and reporting",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'7'),
        },
        Sub {
            route: MAP_NOTE,
            name: "workspace-map",
            signal: "the task workspace map below this line",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'8'),
        },
        Sub {
            route: RECON,
            name: "recon",
            signal: "the repository reconnaissance snapshot below this line",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'9'),
        },
        Sub {
            route: TREEBEARD,
            name: "treebeard",
            signal: "the Treebeard harness lane (RLM / Hi/Q)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'0'),
        },
    ],
};
