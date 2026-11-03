//! ⠄ (dots 3) — Volume IV, the mixture: its roles. The brief each stage of a
//! mixture-of-agents turn is handed — the proposer, the aggregator, the
//! classifier, the critic, the judge, the chooser, the verifier — and the
//! folds a stage can carry: the hedge ladder, context checkpointing, the
//! delegator's test request. Every stage is connected (`book::connect`); the
//! problem, the drafts and the counts ride beside the routes as data.

use super::{DIGITS, Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/d3_roles.book");

pub(crate) const CELL: char = '⠄';
pub(crate) const PROPOSER: Route = Route::new(CELL, '⠁');
pub(crate) const AGGREGATOR: Route = Route::new(CELL, '⠃');
pub(crate) const CLASSIFIER: Route = Route::new(CELL, '⠉');
pub(crate) const CRITIC: Route = Route::new(CELL, '⠙');
pub(crate) const JUDGE: Route = Route::new(CELL, '⠑');
pub(crate) const CHOOSER: Route = Route::new(CELL, '⠋');
pub(crate) const VERIFIER: Route = Route::new(CELL, '⠛');
pub(crate) const HEDGE: Route = Route::new(CELL, '⠓');
pub(crate) const CHECKPOINT: Route = Route::new(CELL, '⠊');
pub(crate) const DELEGATOR: Route = Route::new(CELL, '⠚');

/// Page addresses on one route, in order: the route's cells, each followed by
/// a page digit (1-based) — `pages(SYNTHESIS, 1..=3)` is `⠐⠙⠁⠐⠙⠃⠐⠙⠉`. The
/// mixture sends a label, or a variant of a section, as its pages.
pub(crate) fn pages(route: Route, numbers: impl IntoIterator<Item = usize>) -> String {
    numbers
        .into_iter()
        .map(|n| format!("{}{}", route.cells(), DIGITS[n - 1]))
        .collect()
}

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "roles",
    surface: "the mixture's roles: each stage's brief, and the folds a stage carries",
    subs: &[
        Sub {
            route: PROPOSER,
            name: "proposer",
            signal: "a proposer of the mixture (its angle and wave frame beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'1'),
        },
        Sub {
            route: AGGREGATOR,
            name: "aggregator",
            signal: "an aggregator of the mixture (its task and the drafts below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'2'),
        },
        Sub {
            route: CLASSIFIER,
            name: "classifier",
            signal: "the adaptive gate: classify the request below",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'3'),
        },
        Sub {
            route: CRITIC,
            name: "critic",
            signal: "the critic between layers (draft count beside the route, drafts below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'4'),
        },
        Sub {
            route: JUDGE,
            name: "judge",
            signal: "a judge scoring candidate responses (its scoring task below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'5'),
        },
        Sub {
            route: CHOOSER,
            name: "chooser",
            signal: "the chooser of the final candidates (count beside the route, candidates below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'6'),
        },
        Sub {
            route: VERIFIER,
            name: "verifier",
            signal: "the adversarial verifier of the final answer (the answer below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'7'),
        },
        Sub {
            route: HEDGE,
            name: "hedge",
            signal: "the hedge ladder, folded into every stage's base voice",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: CHECKPOINT,
            name: "checkpoint",
            signal: "context checkpointing for a stage (the loop cadence beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'9'),
        },
        Sub {
            route: DELEGATOR,
            name: "delegator",
            signal: "a delegator may request one verifying test or specialist consult",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'0'),
        },
    ],
};
