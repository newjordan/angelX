//! ⠠ d6 — the long run's seats: the models a long run hands a task to beside
//! its iterations. The deep worker's recovery experiment, the RL campaign's
//! prompt optimizer (a connected seat: it reads the ledger through the ledger
//! reader), handoff-RL's forced restart with its sequence directive, and the
//! four roles of a swarm_compile proof graph (delegate seats: they read the
//! ledger through their own `read_file`). Each prompt is its route; its labels
//! are page addresses, and the objective, hypotheses, evidence, attempts,
//! board state, commands and diffs ride beside them as data.
//! Every page is its original sentence, verbatim. Volume V, the long run.

use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/d6_long_run.book");
pub(crate) const CELL: char = '⠠';
pub(crate) const RECOVERY: Route = Route::new(CELL, '⠁');
pub(crate) const RECOVERY_CONTRACT: Route = Route::new(CELL, '⠃');
pub(crate) const OPTIMIZER: Route = Route::new(CELL, '⠉');
pub(crate) const HANDOFF: Route = Route::new(CELL, '⠙');
pub(crate) const HANDOFF_STATE: Route = Route::new(CELL, '⠑');
pub(crate) const SEQUENCE: Route = Route::new(CELL, '⠋');
pub(crate) const INVESTIGATOR: Route = Route::new(CELL, '⠛');
pub(crate) const TEST_AUTHOR: Route = Route::new(CELL, '⠓');
pub(crate) const IMPLEMENTER: Route = Route::new(CELL, '⠊');
pub(crate) const REVIEWER: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "long-run",
    surface: "the long run's seats: recovery, optimizer, forced handoff, swarm_compile roles",
    subs: &[
        Sub {
            route: RECOVERY,
            name: "recovery-experiment",
            signal: "a loop recovery experiment's labels (each sent as its page address, its value beside it)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: RECOVERY_CONTRACT,
            name: "recovery-contract",
            signal: "run one discriminating local experiment on the hypothesis above",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'2'),
        },
        Sub {
            route: OPTIMIZER,
            name: "prompt-optimizer",
            signal: "improve a policy note from a high- and a low-scoring attempt (labelled below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'3'),
        },
        Sub {
            route: HANDOFF,
            name: "forced-handoff",
            signal: "handoff-RL wiped the context and restarts the campaign (roll beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'4'),
        },
        Sub {
            route: HANDOFF_STATE,
            name: "handoff-state",
            signal: "the campaign state a forced handoff carries (each label sent as its page address)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'5'),
        },
        Sub {
            route: SEQUENCE,
            name: "sequence-directive",
            signal: "handoff-RL's forced sequence directive",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'6'),
        },
        Sub {
            route: INVESTIGATOR,
            name: "investigator",
            signal: "swarm_compile's investigator: investigate the goal below, read-only",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'7'),
        },
        Sub {
            route: TEST_AUTHOR,
            name: "test-author",
            signal: "swarm_compile's test author: one red regression test (its marker beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'8'),
        },
        Sub {
            route: IMPLEMENTER,
            name: "implementer",
            signal: "swarm_compile's implementer: turn the protected red test green",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'9'),
        },
        Sub {
            route: REVIEWER,
            name: "reviewer",
            signal: "swarm_compile's reviewer: review the candidate diff below, read-only",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'0'),
        },
    ],
};
