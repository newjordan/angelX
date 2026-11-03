//! ⠺ w — workflow: the tool protocol, batching, response and promise hints,
//! and the timed cues. Every hint page is its original sentence, verbatim.
//!
//! Cues are timed, never always-on. The system prompt carries the entry
//! personality type (`⠽`); the first tool result of a turn carries the
//! hygiene cue (`⠺⠁⠺⠉⠧⠊`: tool protocol, batching, verification posture);
//! after that a cue appears only when the model's latest actions make it
//! relevant — serial single reads (`⠺⠛` batch), a clean test run after edits
//! (`⠺⠓` finish) — once each until the workspace changes.

use super::{Primary, Raise, Route, Sub, v_verification};

const CATALOG: &str = include_str!("../../../../book/w_workflow.book");

pub(crate) const CELL: char = '⠺';
pub(crate) const PROTOCOL: Route = Route::new(CELL, '⠁');
pub(crate) const PROTOCOL_COMPACT: Route = Route::new(CELL, '⠃');
pub(crate) const BATCHING: Route = Route::new(CELL, '⠉');
pub(crate) const RESPONSE: Route = Route::new(CELL, '⠙');
pub(crate) const PROMISE: Route = Route::new(CELL, '⠑');
pub(crate) const PROMISE_COMPACT: Route = Route::new(CELL, '⠋');
pub(crate) const BATCH: Route = Route::new(CELL, '⠛');
pub(crate) const FINISH: Route = Route::new(CELL, '⠓');
pub(crate) const PAGING: Route = Route::new(CELL, '⠊');
pub(crate) const GREP_CAP: Route = Route::new(CELL, '⠚');

/// Consecutive single-read hops before the batch cue appears.
const SERIAL_READS: usize = 3;

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "workflow",
    surface: "tool protocol, batching, response and promise hints, timed cues, paging",
    subs: &[
        Sub {
            route: PROTOCOL,
            name: "protocol",
            signal: "the tool protocol",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: PROTOCOL_COMPACT,
            name: "protocol-compact",
            signal: "the tool protocol, compact core",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'2'),
        },
        Sub {
            route: BATCHING,
            name: "batching",
            signal: "batching tool calls",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'3'),
        },
        Sub {
            route: RESPONSE,
            name: "response",
            signal: "how to respond",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'4'),
        },
        Sub {
            route: PROMISE,
            name: "promise",
            signal: "ending a turn on a promise",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'5'),
        },
        Sub {
            route: PROMISE_COMPACT,
            name: "promise-compact",
            signal: "ending a turn on a promise, compact core",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'6'),
        },
        Sub {
            route: BATCH,
            name: "batch",
            signal: "a cue: several consecutive hops each read one thing",
            action: "the next reads can share one response, or one `code_mode` call",
            ideas: "- Every hop re-sends the conversation; three reads in one hop cost one \
                    round trip.",
            pages: &[],
        },
        Sub {
            route: FINISH,
            name: "finish",
            signal: "a cue: your test command passed after your edits",
            action: "finish the remaining deliverables and answer",
            ideas: "- Work past green (comment edits, lint the task did not ask for, repeat runs \
                    on unchanged code) adds time without adding evidence.\n\
                    - A deliverable the task names that is still missing is the remaining work.",
            pages: &[],
        },
        Sub {
            route: PAGING,
            name: "paging",
            signal: "the read stopped at a page boundary (next offset above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'9'),
        },
        Sub {
            route: GREP_CAP,
            name: "grep-cap",
            signal: "grep's output cap was reached",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'0'),
        },
    ],
};

/// The turn's cue scheduler: hygiene once at the first tool result, then
/// cues as the model's own actions make them relevant.
#[derive(Default)]
pub(crate) struct Cues {
    hygiene_shown: bool,
    serial_reads: usize,
    batch_shown: bool,
    edited: bool,
    pending: Vec<Raise>,
}

impl Cues {
    /// One hop's calls, as the model issued them.
    pub(crate) fn observe_calls<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        let names = names.into_iter().collect::<Vec<_>>();
        let single_read = names.len() == 1 && matches!(names[0], "read_file" | "grep" | "list_dir");
        self.serial_reads = if single_read {
            self.serial_reads + 1
        } else {
            0
        };
        if self.serial_reads >= SERIAL_READS && !self.batch_shown {
            self.batch_shown = true;
            self.pending.push(Raise::new(BATCH, None));
        }
    }

    /// A workspace edit: a later clean run is new evidence.
    pub(crate) fn note_edit(&mut self) {
        self.edited = true;
    }

    /// A test command came back clean (typed green, or a shell run that exited
    /// zero). After edits, that is the finish cue.
    pub(crate) fn observe_clean_run(&mut self, command: &str) {
        if std::mem::take(&mut self.edited) {
            self.pending.push(Raise::new(FINISH, command.to_string()));
        }
    }

    /// The cues for this hop: hygiene on the turn's first tool result, then
    /// any cues that came due.
    pub(crate) fn take(&mut self) -> Vec<Raise> {
        let mut out = std::mem::take(&mut self.pending);
        if !std::mem::replace(&mut self.hygiene_shown, true) {
            out.extend(
                [PROTOCOL, BATCHING, v_verification::POSTURE].map(|route| Raise::new(route, None)),
            );
        }
        out
    }
}
