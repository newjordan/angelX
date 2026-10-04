//! ⠯ and — Volume II, the tool library: the session — the task list, the
//! standing goal, the context gauge, code review and the text utilities, then
//! the continued section of code_mode.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠯';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "session",
    surface: "the task list, the goal, the context gauge, reviews and text utilities; code_mode, continued",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "todo",
            signal: "using `todo`",
            action: "",
            ideas: "",
            pages: &[
                "default 'list'",
                "for action=add",
                "for action=complete",
                "for action=set",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "goal",
            signal: "using `goal`",
            action: "",
            ideas: "",
            pages: &[
                "This does NOT start an autonomous loop — that is the operator's call.",
                "default 'show'",
                "the objective (action=set)",
                "optional verifiable command whose success means the goal is met (action=set)",
                "optional acceptance criteria (action=set)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "get_context_remaining",
            signal: "using `get_context_remaining`",
            action: "",
            ideas: "",
            pages: &[
                "Report how many tokens the conversation holds against its compaction threshold; older turns compact automatically there, so the threshold never ends the work.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "code_review",
            signal: "using `code_review`",
            action: "",
            ideas: "",
            pages: &[
                "file path, git diff, or raw code snippet to review",
                "review focus area",
                "optional club (default: self / in-hand)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "reverse",
            signal: "using `reverse`",
            action: "",
            ideas: "",
            pages: &["input text"],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "word_count",
            signal: "using `word_count`",
            action: "",
            ideas: "",
            pages: &["input text"],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "code_mode, continued",
            signal: "using `code_mode`, continued",
            action: "",
            ideas: "",
            pages: &[
                "Task text used to condition `repo_recon`; encoded as data, never executable code.",
                "Request nested mutation or execution tools; also requires operator ANGEL_CODE_MODE_EFFECTS=1.",
            ],
        },
    ],
};
