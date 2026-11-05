//! ⠥ u — skills: the playbook catalog, the referral card and the per-task
//! playbook hint. Every page is its original sentence, verbatim; skill names
//! and the playbook count ride inline as data. The selection logic
//! (`relevant_skill_hint`) is unchanged. The last three sections are replies
//! (Volume VI): a loaded skill's labels, reading a parked handle, and the
//! operator switches that bound tool search and code_mode.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};

const BOOK_PAGES_CATALOG: &str = include_str!("../../../../book/u_skills.book");
pub(crate) const CELL: char = '⠥';
pub(crate) const CATALOG: Route = Route::new(CELL, '⠁');
pub(crate) const REFERRAL: Route = Route::new(CELL, '⠃');
pub(crate) const HINT: Route = Route::new(CELL, '⠉');
pub(crate) const TOOL_SEARCH: Route = Route::new(CELL, '⠙');
pub(crate) const HANDLE: Route = Route::new(CELL, '⠑');
pub(crate) const RUNNER: Route = Route::new(CELL, '⠋');
pub(crate) const TYPED_CARGO: Route = Route::new(CELL, '⠛');
pub(crate) const SKILL_LABELS: Route = Route::new(CELL, '⠓');
pub(crate) const HANDLE_READS: Route = Route::new(CELL, '⠊');
pub(crate) const TOOL_NOTES: Route = Route::new(CELL, '⠚');

// ⠥⠓ a loaded skill's labels
pub(crate) const SKILL_TRUNCATED: Page = Page::new(SKILL_LABELS, 1);
pub(crate) const SKILL_RESOURCES: Page = Page::new(SKILL_LABELS, 2);

// ⠥⠊ reading a parked handle
pub(crate) const HANDLE_URI: Page = Page::new(HANDLE_READS, 1);
pub(crate) const HANDLE_SLICE: Page = Page::new(HANDLE_READS, 2);
pub(crate) const HANDLE_BUDGET: Page = Page::new(HANDLE_READS, 3);
pub(crate) const HANDLE_WHOLE: Page = Page::new(HANDLE_READS, 4);

// ⠥⠚ the tool library's operator switches
pub(crate) const ACTIVATION_CAP: Page = Page::new(TOOL_NOTES, 1);
pub(crate) const CODE_MODE_EFFECTS: Page = Page::new(TOOL_NOTES, 2);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "skills",
    surface: "playbooks and the tool library: catalog, hint, tool search, handles, runners",
    subs: &[
        Sub {
            route: CATALOG,
            name: "catalog",
            signal: "the playbook catalog (names below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(BOOK_PAGES_CATALOG, b'1'),
        },
        Sub {
            route: REFERRAL,
            name: "referral",
            signal: "the playbook referral card (count below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(BOOK_PAGES_CATALOG, b'2'),
        },
        Sub {
            route: HINT,
            name: "hint",
            signal: "a harness-selected playbook for this task (name below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(BOOK_PAGES_CATALOG, b'3'),
        },
        Sub {
            route: TOOL_SEARCH,
            name: "tool-search",
            signal: "tool_search activated schemas for the next request",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_PAGES_CATALOG, b'4'),
        },
        Sub {
            route: HANDLE,
            name: "handle",
            signal: "a large output was parked under a handle",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_PAGES_CATALOG, b'5'),
        },
        Sub {
            route: RUNNER,
            name: "runner",
            signal: "the verifier the harness chose failed (its reason above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(BOOK_PAGES_CATALOG, b'6'),
        },
        Sub {
            route: TYPED_CARGO,
            name: "typed-cargo",
            signal: "typed Cargo verification refused workspace-controlled semantics",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(BOOK_PAGES_CATALOG, b'7'),
        },
        Sub {
            route: SKILL_LABELS,
            name: "skill-labels",
            signal: "a label on the loaded skill (its base and files below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(BOOK_PAGES_CATALOG, b'8'),
        },
        Sub {
            route: HANDLE_READS,
            name: "handle-reads",
            signal: "a parked result is read through agent:// or handle_read (the handle above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(BOOK_PAGES_CATALOG, b'9'),
        },
        Sub {
            route: TOOL_NOTES,
            name: "tool-notes",
            signal: "an operator switch bounds this tool (the fact above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(BOOK_PAGES_CATALOG, b'0'),
        },
    ],
};
