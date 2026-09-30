//! ⠷ of — Volume II, the tool library: changing files — writes, batched
//! edits, patches, staged edits, commits and merges.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠷';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "patches",
    surface: "writes, batched edits, patches, staged edits, commits and merges",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "write_file",
            signal: "using `write_file`",
            action: "",
            ideas: "",
            pages: &[
                "workspace path, or conflict://N / conflict://* virtual URL",
                "full file contents, or @ours/@theirs/@base/@both for conflict resolve",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "multi_edit",
            signal: "using `multi_edit`",
            action: "",
            ideas: "",
            pages: &[
                "Fewer hops than repeated str_replace for a multi-site refactor.",
                "path inside the workspace (relative or absolute)",
                "optional stale-edit guard: the file's content tag from read_file's [path#tag] header.",
                "All edits are rejected if the live file no longer matches it.",
                "ordered edits, each applied to the result of the previous",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "apply_patch",
            signal: "using `apply_patch`",
            action: "",
            ideas: "",
            pages: &[
                "Hashline also supports stage=true: preflight and queue the plan without writing; call resolve_edit to accept or reject.",
                "unified diff (a/ b/ headers)",
                "path strip level -pN (default 1)",
                "hashline only: when true, preflight and stage the plan (no disk write); resolve with resolve_edit",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "resolve_edit",
            signal: "using `resolve_edit`",
            action: "",
            ideas: "",
            pages: &[
                "accept | reject | list (default list)",
                "staged edit id from the proposal card (required for accept/reject)",
                "optional note recorded on accept/reject",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "git_commit",
            signal: "using `git_commit`",
            action: "",
            ideas: "",
            pages: &[
                "Set execute=true to stage each unit and commit in that order.",
                "false (default) = print plan only; true = stage+commit each unit",
                "true (default) = one commit per file class; false = single commit",
                "commit subject (and optional blank-line + body).",
                "Used for the first/only unit; further split units get class-scoped subjects",
                "include untracked files in the plan (default true)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "integrate",
            signal: "using `integrate`",
            action: "",
            ideas: "",
            pages: &["branch from delegate"],
        },
    ],
};
