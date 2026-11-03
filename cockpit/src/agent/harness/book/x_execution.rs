//! ⠭ x — calls and replies that did not execute.
//!
//! Tool calls that all fail, tool markup printed as text, replies cut off at
//! the output cap or spent on reasoning, empty replies, and a provider session
//! the harness had to roll. None of these stops a turn; each is one route.

use super::{Primary, Raise, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/x_execution.book");

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
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: MARKUP,
            name: "markup",
            signal: "reply text held tool-call markup; nothing in it ran",
            action: "",
            ideas: "- Results described in prose never happened.",
            pages: &super::catalog_pages::<2>(CATALOG, b'2'),
        },
        Sub {
            route: OUTPUT_CAP,
            name: "output-cap",
            signal: "the last reply was cut off at the output token limit; nothing in it ran",
            action: "",
            ideas: "- Usually one tool call carried too much text; do not restate large content.",
            pages: &super::catalog_pages::<3>(CATALOG, b'3'),
        },
        Sub {
            route: REASONING_CAP,
            name: "reasoning-cap",
            signal: "the last reply spent its whole output limit on reasoning; nothing ran",
            action: "",
            ideas: "- Run the tests, read the failing case, or make one small edit.",
            pages: &super::catalog_pages::<2>(CATALOG, b'4'),
        },
        Sub {
            route: EMPTY,
            name: "empty",
            signal: "the last reply arrived empty: no text and no tool calls",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'5'),
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
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: EMIT_TOOL_CALL,
            name: "emit-tool-call",
            signal: "the reply held reasoning only; tools were offered",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'8'),
        },
        Sub {
            route: EDIT_RECOVERY,
            name: "edit-recovery",
            signal: "no edit was applied; the recovery fields follow",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'9'),
        },
        Sub {
            route: REISSUE,
            name: "reissue",
            signal: "the call's arguments were not valid JSON",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'0'),
        },
    ],
};

/// Hops in a row where every call failed at dispatch: `ANGEL_ERROR_LIMIT` was 6
/// in a task turn at 0.1.6, its advisory at half of it.
pub(crate) const ERROR_NUDGE_HOPS: usize = 3;
pub(crate) const ERROR_CASCADE_HOPS: usize = 6;

/// Consecutive hops where every call failed at dispatch. The third in a row
/// throws `⠭⠁`, once per streak; the sixth throws `⠼⠓` and starts the count
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
