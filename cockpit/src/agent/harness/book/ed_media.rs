//! ⠫ ed — Volume II, the tool library: the stage, video and vision, and the
//! records that carry a workspace forward. One section per tool; its pages are
//! the sentences that told the model how to use that tool, verbatim, moved off
//! the schema. The schema keeps what the tool does and ends with the section's
//! route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/ed_media.book");
pub(crate) const CELL: char = '⠫';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "media",
    surface: "media and the workspace record",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "present",
            signal: "using `present`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "video_probe",
            signal: "using `video_probe`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "video_beats",
            signal: "using `video_beats`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "video_cut",
            signal: "using `video_cut`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "video_contact_sheet",
            signal: "using `video_contact_sheet`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "video_look",
            signal: "using `video_look`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "vision_look",
            signal: "using `vision_look`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'7'),
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "knowledge_graph",
            signal: "using `knowledge_graph`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'8'),
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "continual_harness",
            signal: "using `continual_harness`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'9'),
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "work_landing",
            signal: "using `work_landing`",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'0'),
        },
    ],
};
