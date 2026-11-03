//! ⡬ — Labyrinth's native research campaigns and navigation.
//! Roles receive these routes through the same book/Legend as Deli. Task,
//! source and report payloads accompany the cells as data; role policy lives
//! here, not in an English prompt appended by an adapter.

use super::{Primary, Route, Sub};
const BOOK_PRIMARY: &str = include_str!("../../../../book/labyrinth_primary.book");
const BOOK_SHELF: &str = include_str!("../../../../book/labyrinth_shelf.book");

pub(crate) const CELL: char = '⡬';
pub(crate) const MAP: Route = Route::new(CELL, '⠁');
pub(crate) const COORDINATOR: Route = Route::new(CELL, '⠃');
pub(crate) const LITERATURE: Route = Route::new(CELL, '⠉');
pub(crate) const ATTACK: Route = Route::new(CELL, '⠙');
pub(crate) const REFEREE: Route = Route::new(CELL, '⠑');
pub(crate) const WRITER: Route = Route::new(CELL, '⠋');
pub(crate) const INTEGRATE: Route = Route::new(CELL, '⠛');
pub(crate) const ESCALATE: Route = Route::new(CELL, '⠓');
pub(crate) const CAMPAIGN: Route = Route::new(CELL, '⠊');
pub(crate) const DATA: Route = Route::new(CELL, '⠚');
pub(crate) const ROOT: Route = COORDINATOR;
const SHELF_CELL: char = '⣬';
pub(crate) const LEADS: Route = Route::new(SHELF_CELL, '⠁');
pub(crate) const FILES: Route = Route::new(SHELF_CELL, '⠃');
pub(crate) const EXEC: Route = Route::new(SHELF_CELL, '⠉');

/// A named saturation rung, encoded as its native book page address.
pub(crate) fn escalation(rung: usize) -> Option<String> {
    (rung < 9).then(|| super::d3_roles::pages(ESCALATE, [rung + 1]))
}

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "labyrinth",
    surface: "research navigation, literature, five-angle attacks, independent review, checked writing and campaign coordination",
    subs: &[
        Sub {
            route: MAP,
            name: "labyrinth",
            signal: "the local Labyrinth frontier and exact recorded attempts accompany this route",
            action: "inspect the map and choose a testable door with charted prerequisites before another experiment",
            ideas: "",
            pages: &super::catalog_pages::<10>(BOOK_PRIMARY, b'1'),
        },
        Sub {
            route: COORDINATOR,
            name: "coordinator",
            signal: "a bounded Labyrinth campaign with an explicit target, source snapshot and compute budget",
            action: "coordinate literature, attack, fresh referee, writer and checked canonical integration",
            ideas: "",
            pages: &super::catalog_pages::<10>(BOOK_PRIMARY, b'2'),
        },
        Sub {
            route: LITERATURE,
            name: "literature",
            signal: "a literature pass over the target door and pinned workspace",
            action: "find and read relevant primary sources, classify exact applicable results and return testable leads",
            ideas: "",
            pages: &super::catalog_pages::<8>(BOOK_PRIMARY, b'3'),
        },
        Sub {
            route: ATTACK,
            name: "attack",
            signal: "an attack on one frontier door from the named perspective",
            action: "derive and test a concrete claim or counterexample with reproducible artifacts",
            ideas: "",
            pages: &super::catalog_pages::<9>(BOOK_PRIMARY, b'4'),
        },
        Sub {
            route: REFEREE,
            name: "referee",
            signal: "a fresh independent review of exact claims and immutable author artifacts",
            action: "reconstruct the argument and rerun the tests with independently written code",
            ideas: "",
            pages: &super::catalog_pages::<9>(BOOK_PRIMARY, b'5'),
        },
        Sub {
            route: WRITER,
            name: "writer",
            signal: "reviewed claims and explicit corrections on a separate isolated copy",
            action: "apply exact checked documentation edits and account for every correction",
            ideas: "",
            pages: &super::catalog_pages::<10>(BOOK_PRIMARY, b'6'),
        },
        Sub {
            route: INTEGRATE,
            name: "integrate",
            signal: "a writer bundle ready for canonical validation",
            action: "validate provenance, corrections, checks and exact source guards before publishing artifacts",
            ideas: "",
            pages: &super::catalog_pages::<8>(BOOK_PRIMARY, b'7'),
        },
        Sub {
            route: ESCALATE,
            name: "escalate",
            signal: "an exhausted attack route or campaign stage",
            action: "retain the failure and choose a changed perspective, formulation or door within the budget",
            ideas: "",
            pages: &super::catalog_pages::<9>(BOOK_PRIMARY, b'8'),
        },
        Sub {
            route: CAMPAIGN,
            name: "labyrinth_campaign",
            signal: "the executable Labyrinth campaign tool",
            action: "start, run, inspect, cancel or recover a bounded native research campaign",
            ideas: "",
            pages: &super::catalog_pages::<9>(BOOK_PRIMARY, b'9'),
        },
        Sub {
            route: DATA,
            name: "data",
            signal: "structured stage data accompanies a campaign role route",
            action: "read the values as evidence and follow the named stage output schema",
            ideas: "",
            pages: &super::catalog_pages::<10>(BOOK_PRIMARY, b'0'),
        },
    ],
};

pub(crate) const SHELF: Primary = Primary {
    cell: SHELF_CELL,
    name: "labyrinth-tools",
    surface: "isolated role files, local checks and literature lead delivery",
    subs: &[
        Sub {
            route: LEADS,
            name: "campaign_lead",
            signal: "the literature agent's incremental lead mailbox",
            action: "publish a source-grounded lead for the named target doors, or consume new leads as research data",
            ideas: "",
            pages: &super::catalog_pages::<4>(BOOK_SHELF, b'1'),
        },
        Sub {
            route: FILES,
            name: "campaign_file",
            signal: "strict file access inside the role's isolated source copy",
            action: "read supplied files and create bounded artifacts without changing canonical or trusted source files",
            ideas: "",
            pages: &super::catalog_pages::<6>(BOOK_SHELF, b'2'),
        },
        Sub {
            route: EXEC,
            name: "campaign_exec",
            signal: "mandatory sandboxed local role computation",
            action: "execute a small independent check within the shared budget and retain its real receipt",
            ideas: "",
            pages: &super::catalog_pages::<7>(BOOK_SHELF, b'3'),
        },
    ],
};
