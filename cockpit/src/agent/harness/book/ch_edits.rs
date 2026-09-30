//! ⠡ ch — Volume II, the tool library: reading, editing and committing files,
//! and the notes that outlive a session. One section per tool; its pages are
//! the sentences that told the model how to use that tool, verbatim, moved off
//! the schema. The schema keeps what the tool does and ends with the section's
//! route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠡';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "edits",
    surface: "files, edits, git and notes",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "read_file",
            signal: "using `read_file`",
            action: "",
            ideas: "",
            pages: &[
                "Use `agent://hnd_…` or handle_read for opaque handles; `outline://` always requires a workspace source path.",
                "Reuse the next offset named by a truncated page",
                "Each page is headed with `[path#tag]` (whole-file content tag) and absolute 1-based line numbers so you can author token-cheap hashline `apply_patch` edits that emit only the NEW lines.",
                "workspace path, or conflict:// / skill:// / agent:// / outline:// / pr:// / issue:// / ledger:// virtual URL",
                "one-based first source line; default 1.",
                "complete source lines to return; default 200, maximum 400",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "str_replace",
            signal: "using `str_replace`",
            action: "",
            ideas: "",
            pages: &[
                "Fails if 'old' is absent or appears more than once — include enough surrounding context to make it unique.",
                "An indentation-only miss can return exact `old` and `expect_tag` recovery fields; copy them and author `new` with the intended indentation.",
                "path inside the workspace (relative or absolute)",
                "exact text to replace (must be unique)",
                "replacement text",
                "optional stale-edit guard: the file's content tag from read_file's [path#tag] header.",
                "The edit is rejected if the live file no longer matches it.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "git_diff",
            signal: "using `git_diff`",
            action: "",
            ideas: "",
            pages: &[
                "Review your own edits before committing.",
                "diff the index (--cached) instead of the worktree",
                "compare the current worktree against one verified branch, tag, or commit",
                "show a compact file/change summary instead of the full patch",
                "limit the diff to this in-workspace path (relative or absolute)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "git_log",
            signal: "using `git_log`",
            action: "",
            ideas: "",
            pages: &[
                "Orient on what changed recently and why.",
                "how many commits (default 15, max 100)",
                "limit history to this in-workspace path (relative or absolute)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "file_search",
            signal: "using `file_search`",
            action: "",
            ideas: "",
            pages: &[
                "Use when you half-remember a filename; complements find_files (exact glob) and grep (by content).",
                "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
                "include hidden paths (default false); .git and credential files remain excluded",
                "path fragment to fuzzy-match",
                "max results (default 20)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "semantic_read",
            signal: "using `semantic_read`",
            action: "",
            ideas: "",
            pages: &[
                "This is useful after file_search/grep finds plausible files but reading all of them would waste model context.",
                "natural-language question or implementation concept to retrieve",
                "workspace files selected by file_search, grep, find_files, or prior evidence",
                "ranked passages to return (default 6, maximum 12)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "lsp_diagnostics",
            signal: "using `lsp_diagnostics`",
            action: "",
            ideas: "",
            pages: &[
                "Use to verify an edit compiles/type-checks instead of guessing.",
                "File to analyze (workspace-relative or absolute).",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "notes",
            signal: "using `notes`",
            action: "",
            ideas: "",
            pages: &[
                "Use for durable findings and decisions.",
                "default 'list'",
                "for action=add",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "handoff",
            signal: "using `handoff`",
            action: "",
            ideas: "",
            pages: &[
                "Write the state a cold successor needs to resume this work: current goal, what is done (with evidence), what is in flight, exact next steps in order, and load-bearing facts (paths, hashes, commands, decisions with reasons).",
                "Refresh it before long or risky stretches and before ending a run.",
                "default: 'write' when 'note' is present, else 'show'",
                "for action=write — the complete replacement resume brief",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "recall",
            signal: "using `recall`",
            action: "",
            ideas: "",
            pages: &[
                "Use it when prior context would help and isn't in the current conversation.",
                "What to recall.",
                "Max notes (default 5).",
            ],
        },
    ],
};

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__tools_tests.rs"]
mod tests;
