//! ⠗ r — what the operator armed: relentless execution (`/relentless`,
//! `ANGEL_RELENTLESS_EXECUTION`) rides the operator controls as `⠗⠁`; the
//! standing goal, solo mode and the magic keywords ride beside it, with the
//! bounds of the goal and memory blocks (`⠗⠓`). The goal block's field labels
//! are pages of `⠗⠃`, each field's value beside its address; the `goal` tool's
//! `show` reply reads the same pages. The reinforcement loop the
//! operator arms has two seats of its own: the grader and the prompt
//! optimizer.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/r_relentless.book");
pub(crate) const CELL: char = '⠗';
pub(crate) const ARMED: Route = Route::new(CELL, '⠁');
pub(crate) const GOAL: Route = Route::new(CELL, '⠃');
pub(crate) const SOLO: Route = Route::new(CELL, '⠉');
pub(crate) const ULTRATHINK: Route = Route::new(CELL, '⠙');
pub(crate) const ORCHESTRATE: Route = Route::new(CELL, '⠑');
pub(crate) const WORKFLOWZ: Route = Route::new(CELL, '⠋');
pub(crate) const HANDOFF_RL: Route = Route::new(CELL, '⠛');
pub(crate) const BOUNDS: Route = Route::new(CELL, '⠓');
pub(crate) const GRADER: Route = Route::new(CELL, '⠊');
pub(crate) const OPTIMIZER: Route = Route::new(CELL, '⠚');

// `⠗⠃`'s field labels: each page is sent as its address, the field beside it.
pub(crate) const GOAL_OBJECTIVE: Page = Page::new(GOAL, 2);
pub(crate) const GOAL_CRITERIA: Page = Page::new(GOAL, 3);
pub(crate) const GOAL_CHECK: Page = Page::new(GOAL, 4);
pub(crate) const GOAL_STATE: Page = Page::new(GOAL, 5);
pub(crate) const GOAL_BUDGET: Page = Page::new(GOAL, 6);
pub(crate) const GOAL_BLOCKED: Page = Page::new(GOAL, 7);
pub(crate) const GOAL_NOTES: Page = Page::new(GOAL, 8);
pub(crate) const GOAL_SHOW: Page = Page::new(GOAL, 9);
pub(crate) const GOAL_ROUND: Page = Page::new(GOAL, 10);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "relentless",
    surface: "what the operator armed: relentless execution, the goal, solo, magic keywords, the RL seats",
    subs: &[
        Sub {
            route: ARMED,
            name: "armed",
            signal: "the operator armed relentless execution for this request",
            action: "",
            ideas: "- Every action should benefit the user; ground the output in evidence.\n\
                - Status prose is not progress; deliver a useful final answer, then the mode ends.",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: GOAL,
            name: "goal",
            signal: "the operator's standing goal, fenced (each field's value beside its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(CATALOG, b'2'),
        },
        Sub {
            route: SOLO,
            name: "solo",
            signal: "the operator armed solo mode",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(CATALOG, b'3'),
        },
        Sub {
            route: ULTRATHINK,
            name: "ultrathink",
            signal: "the operator wrote ultrathink",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'4'),
        },
        Sub {
            route: ORCHESTRATE,
            name: "orchestrate",
            signal: "the operator wrote orchestrate",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'5'),
        },
        Sub {
            route: WORKFLOWZ,
            name: "workflowz",
            signal: "the operator wrote workflowz",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'6'),
        },
        Sub {
            route: HANDOFF_RL,
            name: "handoff-rl",
            signal: "the operator wrote handoff_rl",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: BOUNDS,
            name: "bounds",
            signal: "the goal or memory block was cut to fit (the count beside the page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'8'),
        },
        Sub {
            route: GRADER,
            name: "grader",
            signal: "the reinforcement loop's grader (the task and the response below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'9'),
        },
        Sub {
            route: OPTIMIZER,
            name: "optimizer",
            signal: "the reinforcement loop's prompt optimizer (the task, the prompt and two rollouts below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'0'),
        },
    ],
};
