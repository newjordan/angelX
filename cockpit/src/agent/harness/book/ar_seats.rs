//! ⠜ ar — seats: the prompts of child agents — delegate specialists, spawn
//! seats, agent-graph nodes and gates, loop-recovery experiments, the coding
//! agent an evaluation rollout drives, and Grok's pure-completion host seat.
//! A seat with tools reads the ledger through `read_file`; a bare one through
//! the ledger reader (`book::connect`), with its no-tools contract on `⠌`.

use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/ar_seats.book");
pub(crate) const CELL: char = '⠜';
pub(crate) const DELEGATE_REVIEW: Route = Route::new(CELL, '⠁');
pub(crate) const DELEGATE_WRITE: Route = Route::new(CELL, '⠃');
pub(crate) const SPAWN_SEAT: Route = Route::new(CELL, '⠉');
pub(crate) const PERSONA: Route = Route::new(CELL, '⠙');
pub(crate) const GRAPH_NODE: Route = Route::new(CELL, '⠑');
pub(crate) const GATE: Route = Route::new(CELL, '⠋');
pub(crate) const EXPERIMENT: Route = Route::new(CELL, '⠛');
pub(crate) const POLICY_NOTE: Route = Route::new(CELL, '⠓');
pub(crate) const CODING_AGENT: Route = Route::new(CELL, '⠊');
pub(crate) const GROK_HOST: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "seats",
    surface: "seats: delegate specialists, spawn seats and other child seats that can read the ledger",
    subs: &[
        Sub {
            route: DELEGATE_REVIEW,
            name: "delegate-review",
            signal: "an inspect-only specialist in an isolated worktree",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'1'),
        },
        Sub {
            route: DELEGATE_WRITE,
            name: "delegate-write",
            signal: "an implementing specialist in an isolated worktree",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'2'),
        },
        Sub {
            route: SPAWN_SEAT,
            name: "spawn-seat",
            signal: "one seat of a spawn formation (seat, count, formation beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: PERSONA,
            name: "persona",
            signal: "the seat's persona (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'4'),
        },
        Sub {
            route: GRAPH_NODE,
            name: "graph-node",
            signal: "one node of an agent graph (node and graph beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'5'),
        },
        Sub {
            route: GATE,
            name: "gate",
            signal: "this node is a gate",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'6'),
        },
        Sub {
            route: EXPERIMENT,
            name: "experiment",
            signal: "one owned leaf experiment of a loop recovery",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'7'),
        },
        Sub {
            route: POLICY_NOTE,
            name: "policy-note",
            signal: "a controller-supplied policy note for this attempt (below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'8'),
        },
        Sub {
            route: CODING_AGENT,
            name: "coding-agent",
            signal: "the coding agent an evaluation rollout drives (its task below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'9'),
        },
        Sub {
            route: GROK_HOST,
            name: "grok-host",
            signal: "Grok as pure completion for the host: the host's tools by markup (their catalog in the user prompt)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'0'),
        },
    ],
};
