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
            pages: &[
                "Tool protocol: tools are available through the structured tool-call interface advertised by the host.",
                "Do not print raw `<tool_call>`, `<function=...>`, `shell(...)`, or `delegate(...)` markup as chat text.",
                "Use built-in tools for local work, `skill(name)` for reusable playbooks, `<server>__mcp` tools for MCP resources/prompts, and `delegate` only when a specialist should work in an isolated subagent workspace.",
                "Tool execution is still bounded by the active workspace, sandbox, hooks, and approval gates.",
            ],
        },
        Sub {
            route: PROTOCOL_COMPACT,
            name: "protocol-compact",
            signal: "the tool protocol, compact core",
            action: "",
            ideas: "",
            pages: &[
                "Tool protocol: use the structured tool-call interface advertised by the host.",
                "Never print `<tool_call>`, `<function=...>`, `shell(...)`, or `delegate(...)` markup as chat text.",
                "Tool execution is bounded by the active workspace, sandbox, hooks, and approval gates.",
            ],
        },
        Sub {
            route: BATCHING,
            name: "batching",
            signal: "batching tool calls",
            action: "",
            ideas: "",
            pages: &[
                "Batch independent tool calls in one assistant response instead of waiting between them; combine related read/filter work into fewer calls and consume the compact result.",
                "For multi-file reading or filtering, prefer one `code_mode` batch or the dedicated repository tools when available, over a serial chain of shell reconnaissance.",
            ],
        },
        Sub {
            route: RESPONSE,
            name: "response",
            signal: "how to respond",
            action: "",
            ideas: "",
            pages: &[
                "Act on clear requests immediately.",
                "Precede every tool call with its caveman line, not a prose preamble.",
                "Report new findings, blockers, or changes of plan briefly.",
                "Keep the final answer to the outcome, verification, and remaining work; give more detail when the user asks.",
            ],
        },
        Sub {
            route: PROMISE,
            name: "promise",
            signal: "ending a turn on a promise",
            action: "",
            ideas: "",
            pages: &[
                "Do not end a turn with a promise to inspect, check, brief the council, or pull context.",
                "If you say you need reconnaissance or council input, make the corresponding tool calls in that same turn, then answer from the results.",
            ],
        },
        Sub {
            route: PROMISE_COMPACT,
            name: "promise-compact",
            signal: "ending a turn on a promise, compact core",
            action: "",
            ideas: "",
            pages: &[
                "Do not end a turn with a promise to inspect, check, or pull context: make the corresponding tool calls in that same turn, then answer from the results.",
            ],
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
            pages: &["…[more content; re-call read_file with offset={next_offset}]"],
        },
        Sub {
            route: GREP_CAP,
            name: "grep-cap",
            signal: "grep's output cap was reached",
            action: "",
            ideas: "",
            pages: &[
                "narrow the path or pattern",
                "resume without duplicates with \"after_file\":{cursor}",
            ],
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
