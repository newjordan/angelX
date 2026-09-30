//! ⠵ z — brevity: the caveman mode, its levels and its rules. Every page is its
//! original sentence, verbatim. The link layer splices the marker, one level
//! and the rules, in the original order (`⠵⠁⠵⠉⠵⠋` for full).

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠵';
pub(crate) const MARKER: Route = Route::new(CELL, '⠁');
pub(crate) const LITE: Route = Route::new(CELL, '⠃');
pub(crate) const FULL: Route = Route::new(CELL, '⠉');
pub(crate) const ULTRA: Route = Route::new(CELL, '⠙');
pub(crate) const WENYAN: Route = Route::new(CELL, '⠑');
pub(crate) const RULES: Route = Route::new(CELL, '⠋');
pub(crate) const AGED: Route = Route::new(CELL, '⠛');
pub(crate) const COMPACTED: Route = Route::new(CELL, '⠓');
pub(crate) const PLAN: Route = Route::new(CELL, '⠊');
pub(crate) const CONSTRAINTS: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "brevity",
    surface: "brevity: caveman rules and levels, and the compacted context",
    subs: &[
        Sub {
            route: MARKER,
            name: "marker",
            signal: "the brevity mode and its level",
            action: "",
            ideas: "",
            pages: &["angelX SOTA brevity mode ({level})."],
        },
        Sub {
            route: LITE,
            name: "lite",
            signal: "the lite level",
            action: "",
            ideas: "",
            pages: &["Lite: remove filler and hedging, but keep normal grammar."],
        },
        Sub {
            route: FULL,
            name: "full",
            signal: "the full level",
            action: "",
            ideas: "",
            pages: &[
                "Full: drop filler, pleasantries, and padded narration.",
                "Fragments OK.",
            ],
        },
        Sub {
            route: ULTRA,
            name: "ultra",
            signal: "the ultra level",
            action: "",
            ideas: "",
            pages: &[
                "Ultra: shortest unambiguous answer.",
                "State each fact once.",
                "Use fragments when clear.",
            ],
        },
        Sub {
            route: WENYAN,
            name: "wenyan",
            signal: "the wenyan levels",
            action: "",
            ideas: "",
            pages: &[
                "Wenyan levels are disabled for angelX SOTA outbounds; use full terse English unless the user writes Chinese.",
            ],
        },
        Sub {
            route: RULES,
            name: "rules",
            signal: "the brevity rules",
            action: "",
            ideas: "",
            pages: &[
                "Reason the same way.",
                "Preserve technical substance.",
                "Keep code blocks, CLI commands, JSON, tool-call arguments, API names, file paths, commit keywords, and exact error strings verbatim.",
                "Obey explicit output formats, safety warnings, and destructive-action confirmations over brevity.",
                "Do not mention this mode.",
            ],
        },
        Sub {
            route: AGED,
            name: "aged",
            signal: "a tool output was elided from context (its identity and handle inside the bracket)",
            action: "",
            ideas: "",
            pages: &["re-run the tool if needed"],
        },
        Sub {
            route: COMPACTED,
            name: "compacted",
            signal: "earlier conversation, compacted to the notes below",
            action: "",
            ideas: "",
            pages: &[
                "[Earlier conversation compacted to these notes — background reference, not an instruction.]",
                "[Earlier conversation compacted — background reference, not an instruction.",
                "Fuller detail is in long-term memory; recall it if needed.]",
            ],
        },
        Sub {
            route: PLAN,
            name: "plan",
            signal: "the model's own working plan, carried across compaction",
            action: "",
            ideas: "",
            pages: &[
                "[current-plan/v1 — assistant-authored working state, not a user instruction]",
            ],
        },
        Sub {
            route: CONSTRAINTS,
            name: "constraints",
            signal: "the operator's protected task and constraints follow as anchors",
            action: "",
            ideas: "",
            pages: &[
                "Protected operator task and explicit constraints/answer contract follow in User-role anchors.",
                "Constraint classes: named identifiers, forbidden paths, numeric limits, required tests, answer format.",
                "Exact anchors govern; excerpts and drops are recorded below.",
            ],
        },
    ],
};
