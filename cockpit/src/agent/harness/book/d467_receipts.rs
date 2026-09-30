//! ⡨ (dots 4-6-7) — Volume VI, the replies, on its overflow shelf: receipts.
//! The handoff brief's requirements and the todo/notes action lists, and the
//! marks the harness leaves where it elided something to fit the context —
//! an aged or duplicate tool output, a shrunk argument, recalled notes past
//! the budget, the middle of a long task or output, the tool-aging boundary.
//! A mark is its page address inside the bracket it always had; its counts
//! ride beside it as data (`…[⡨⠃⠋ dropped=… shown=… total=…]`). The
//! harness's notes on a result (`⡨⠉`) are marks the same way: the operator
//! task kept out of a summary, a worker killed by a signal, a grandchild
//! holding a pipe, a script run over a recipe.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⡨';
pub(crate) const HANDOFF: Route = Route::new(CELL, '⠁');
pub(crate) const ELISION: Route = Route::new(CELL, '⠃');
pub(crate) const NOTES: Route = Route::new(CELL, '⠉');

// ⡨⠁ the handoff brief and the action lists
pub(crate) const HANDOFF_WRITE: Page = Page::new(HANDOFF, 1);
pub(crate) const BRIEF_NAMES: Page = Page::new(HANDOFF, 2);
pub(crate) const FULL_BRIEF: Page = Page::new(HANDOFF, 3);
pub(crate) const DENSE_BRIEF: Page = Page::new(HANDOFF, 4);
pub(crate) const TODO_ACTIONS: Page = Page::new(HANDOFF, 5);
pub(crate) const HANDOFF_ACTIONS: Page = Page::new(HANDOFF, 6);
pub(crate) const NOTES_ACTIONS: Page = Page::new(HANDOFF, 7);

// ⡨⠃ elision marks. The aged, duplicate, argument, task-middle and
// context-fit marks are `&str` consts beside their detectors (`compact.rs`,
// `context.rs`); the book tests pin each one to its page here.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const AGED_OUTPUT: Page = Page::new(ELISION, 1);
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const DUPLICATE_OUTPUT: Page = Page::new(ELISION, 2);
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const SHRUNK_ARGUMENT: Page = Page::new(ELISION, 3);
pub(crate) const RECALLED_NOTES: Page = Page::new(ELISION, 4);
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const TASK_MIDDLE: Page = Page::new(ELISION, 5);
pub(crate) const MIDDLE_LINES: Page = Page::new(ELISION, 6);
pub(crate) const MIDDLE_BYTES: Page = Page::new(ELISION, 7);
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const CONTEXT_FIT: Page = Page::new(ELISION, 8);
pub(crate) const MATCH_CAP: Page = Page::new(ELISION, 9);
pub(crate) const AGING_BOUNDARY: Page = Page::new(ELISION, 10);

// ⡨⠉ the harness's notes on a result: a mark inside its bracket, its values
// beside the address.
pub(crate) const TASK_RETAINED: Page = Page::new(NOTES, 1);
pub(crate) const WORKER_KILLED: Page = Page::new(NOTES, 2);
pub(crate) const STDOUT_HELD: Page = Page::new(NOTES, 3);
pub(crate) const STDERR_HELD: Page = Page::new(NOTES, 4);
pub(crate) const SCRIPT_OVER_RECIPE: Page = Page::new(NOTES, 5);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "receipts",
    surface: "the handoff brief and the action lists; the marks left where context was elided; the harness's notes on a result",
    subs: &[
        Sub {
            route: HANDOFF,
            name: "handoff",
            signal: "the handoff note or an action list refused the call (the fact above)",
            action: "",
            ideas: "",
            pages: &[
                "action=write {note} to set one",
                "A resume brief must name the goal, completed work with evidence, in-flight state, ordered next steps, and load-bearing facts.",
                "Write the full brief.",
                "keep it a dense resume brief, not a transcript",
                "use list|add|complete|set",
                "use show|write",
                "use list|add|clear",
            ],
        },
        Sub {
            route: ELISION,
            name: "elision",
            signal: "the harness elided part of the context here (counts beside the mark)",
            action: "",
            ideas: "",
            pages: &[
                "tool output elided",
                "[duplicate inspection output elided ({original_bytes} bytes) — newest identical result retained at {newer_call}]",
                "[tool argument elided: {} bytes]",
                "…[{omitted} additional recalled note(s) omitted to fit context]",
                "…[middle of active user task omitted by bounded compaction anchor]…",
                "…[{dropped} middle line(s) elided — {} of {} lines shown]",
                "…[{dropped} middle byte(s) elided — ~{} of {} bytes shown]",
                "tool output elided for context fit",
                "…[truncated at {max} matches]",
                "[tool-aging boundary: aged {results} result(s), dropped {excerpts} excerpt(s), saved {bytes} payload bytes]",
            ],
        },
        Sub {
            route: NOTES,
            name: "notes",
            signal: "the harness noted what happened to a result or a request (values beside the mark)",
            action: "",
            ideas: "",
            pages: &[
                "[Operator task retained separately; sha256:{sha}]",
                "[worker killed by {signal}; reason=signal:{signal}; signal {number}; verification inconclusive]",
                "[grandchild_holds_stdout: drain deadline reached; inherited output pipe closed; capture incomplete]",
                "[grandchild_holds_stderr: drain deadline reached; inherited error pipe closed; capture incomplete]",
                "[code_mode: both script and recipe given; ran script]",
            ],
        },
    ],
};
