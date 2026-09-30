//! ⠭ x — calls and replies that did not execute.
//!
//! Tool calls that all fail, tool markup printed as text, replies cut off at
//! the output cap or spent on reasoning, empty replies, and a provider session
//! the harness had to roll. None of these stops a turn; each is one route.

use super::{Primary, Raise, Route, Sub};

pub(crate) const CELL: char = '⠭';
pub(crate) const ERRORS: Route = Route::new(CELL, '⠁');
pub(crate) const MARKUP: Route = Route::new(CELL, '⠃');
pub(crate) const OUTPUT_CAP: Route = Route::new(CELL, '⠉');
pub(crate) const REASONING_CAP: Route = Route::new(CELL, '⠙');
pub(crate) const EMPTY: Route = Route::new(CELL, '⠑');
pub(crate) const SESSION: Route = Route::new(CELL, '⠋');
pub(crate) const ANSWER_DIRECTLY: Route = Route::new(CELL, '⠛');
pub(crate) const EMIT_TOOL_CALL: Route = Route::new(CELL, '⠓');
pub(crate) const EDIT_RECOVERY: Route = Route::new(CELL, '⠊');
pub(crate) const REISSUE: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "execution",
    surface: "calls and replies that did not execute",
    subs: &[
        Sub {
            route: ERRORS,
            name: "errors",
            signal: "every call failed, the third hop in a row",
            action: "",
            ideas: "- Later errors are often echoes of the first.\n\
                    - Prerequisites: path, arguments, dependency, state (`list_dir` / `find_files` \
                    before reading).",
            pages: &[
                "Every tool call in your last several turns failed.",
                "Stop and read the actual error messages — they usually name the cause (wrong path, missing file, bad arguments, wrong state).",
                "Verify the precondition first (does the file/dir exist? `list_dir`/`find_files` before reading; check the working state) and then retry differently.",
                "Repeating failing calls won't help.",
            ],
        },
        Sub {
            route: MARKUP,
            name: "markup",
            signal: "reply text held tool-call markup; nothing in it ran",
            action: "",
            ideas: "- Results described in prose never happened.",
            pages: &[
                "Your previous message printed raw tool markup as plain text — no tool was executed, and any results it described were invented.",
                "Re-issue the action through the structured tool-call interface now (no tool markup in chat text), then answer from the real output.",
            ],
        },
        Sub {
            route: OUTPUT_CAP,
            name: "output-cap",
            signal: "the last reply was cut off at the output token limit; nothing in it ran",
            action: "",
            ideas: "- Usually one tool call carried too much text; do not restate large content.",
            pages: &[
                "Your last reply was cut off at the output token limit before it finished, so nothing in it ran.",
                "Usually one tool call carried too much text.",
                "Split the work: write a large file in parts (create it with the first part, then add the rest with further edits), keep each tool call well under the limit, and do not restate large content.",
            ],
        },
        Sub {
            route: REASONING_CAP,
            name: "reasoning-cap",
            signal: "the last reply spent its whole output limit on reasoning; nothing ran",
            action: "",
            ideas: "- Run the tests, read the failing case, or make one small edit.",
            pages: &[
                "Your last reply spent its whole output limit on private reasoning and was cut off before it said or did anything, so nothing ran.",
                "Do not work the problem out in your head: take the next concrete step now with one tool call (run the tests, read the failing case, or make one small edit) and keep your reasoning short.",
            ],
        },
        Sub {
            route: EMPTY,
            name: "empty",
            signal: "the last reply arrived empty: no text and no tool calls",
            action: "",
            ideas: "",
            pages: &[
                "Your previous reply arrived empty — no text and no tool calls were received.",
                "Respond now with either structured tool calls or answer text.",
            ],
        },
        Sub {
            pages: &[],
            route: SESSION,
            name: "session",
            signal: "the provider session faulted and the harness rolled history (note below)",
            action: "continue from the current workspace and the compact note",
            ideas: "- Do not re-read the whole transcript or replay the same hop.",
        },
        Sub {
            route: ANSWER_DIRECTLY,
            name: "answer-directly",
            signal: "the reply held reasoning only; no tools were offered",
            action: "",
            ideas: "",
            pages: &[
                "Your previous turn produced only internal reasoning and no final answer.",
                "Skip the reasoning this time: answer the request directly, without any think block.",
            ],
        },
        Sub {
            route: EMIT_TOOL_CALL,
            name: "emit-tool-call",
            signal: "the reply held reasoning only; tools were offered",
            action: "",
            ideas: "",
            pages: &[
                "Your previous turn produced only internal reasoning and no action.",
                "Do not reason further.",
                "Execute the next step NOW as a structured tool call through the tool interface — no prose, no think block.",
                "Only if no tool applies, give the final answer directly.",
            ],
        },
        Sub {
            route: EDIT_RECOVERY,
            name: "edit-recovery",
            signal: "no edit was applied; the recovery fields follow",
            action: "",
            ideas: "",
            pages: &[
                "Exact source is JSON-escaped below; copy `old` and author `new` with the intended indentation.",
                "Trimming is only a discovery hint, including for Python/YAML; it does not authorize re-indentation.",
                "Retry str_replace on the same path with these old/expect_tag fields.",
                "Retry the ENTIRE multi_edit list with this outer expect_tag; replace only the identified edit's old.",
                "This old describes the buffer AFTER preceding edits, not the unchanged live file.",
                "edit_index is one-based recovery metadata, not a tool argument.",
                "Read the indicated region and choose a smaller unique edit.",
                "add surrounding context before retrying.",
                "Read this path with offset={offset} limit={limit} and choose a smaller unique edit.",
            ],
        },
        Sub {
            route: REISSUE,
            name: "reissue",
            signal: "the call's arguments were not valid JSON",
            action: "",
            ideas: "",
            pages: &["reissue `{name}` with valid JSON"],
        },
    ],
};

/// Hops in a row where every call failed at dispatch: `ANGEL_ERROR_LIMIT` was 6
/// in a task turn at 0.1.6, its advisory at half of it.
pub(crate) const ERROR_NUDGE_HOPS: usize = 3;
pub(crate) const ERROR_CASCADE_HOPS: usize = 6;

/// Consecutive hops where every call failed at dispatch. The third in a row
/// throws `⠭⠁`, once per streak; the sixth throws `⠼⠊` and starts the count
/// again, as the 0.1.6 error breaker did before its stop was taken away.
#[derive(Default)]
pub(crate) struct ErrorStreak {
    streak: usize,
}

impl ErrorStreak {
    pub(crate) fn observe(&mut self, all_errored: bool) -> Option<Raise> {
        self.streak = if all_errored { self.streak + 1 } else { 0 };
        if self.streak == ERROR_CASCADE_HOPS {
            self.streak = 0;
            return Some(Raise::new(
                super::d3456_advisories::CASCADE,
                format!("{ERROR_CASCADE_HOPS} hops in a row, every call failed"),
            ));
        }
        (self.streak == ERROR_NUDGE_HOPS)
            .then(|| Raise::new(ERRORS, "3 hops in a row, every call failed".to_string()))
    }

    pub(crate) fn streak(&self) -> usize {
        self.streak
    }
}

/// Cut-off replies in a row, after which a re-send would only repeat itself.
pub(crate) const OUTPUT_CAP_LIMIT: usize = 3;

/// A provider error meaning the reply hit its output-token cap: re-sending the
/// same request hits the same cap (polyglot-v1 rust-decimal, DeepSeek: 25
/// identical 8192-token cut-offs until the task wall).
pub(crate) fn is_output_cap_truncation(error: &str) -> bool {
    error.starts_with("response incomplete:")
        || error == crate::agent::club::TRUNCATED_OUTPUT_ERR
        || error.contains("finish_reason=length")
}

/// The route for a reply that failed before it could act, if it has one.
pub(crate) fn failed_reply(error: &str, reasoning_only: bool) -> Option<Route> {
    if is_output_cap_truncation(error) {
        Some(if reasoning_only {
            REASONING_CAP
        } else {
            OUTPUT_CAP
        })
    } else if super::super::is_empty_reply_error(error) {
        Some(EMPTY)
    } else {
        None
    }
}
