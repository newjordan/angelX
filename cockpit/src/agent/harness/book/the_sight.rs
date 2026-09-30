//! ⠮ the — Volume II, the tool library: sight — the language servers and the
//! cockpit screen, then the continued section of video_cut.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠮';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "sight",
    surface: "language servers and the cockpit screen; video_cut, continued",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "lsp_definition",
            signal: "using `lsp_definition`",
            action: "",
            ideas: "",
            pages: &[
                "Give `symbol` (first occurrence is queried) or an explicit 1-based `line`(+`character`).",
                "Source file (workspace-relative or absolute).",
                "Symbol to locate (first occurrence in the file).",
                "Use this OR line/character.",
                "1-based line of the symbol (alternative to `symbol`).",
                "1-based column on `line` (default 1).",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "lsp_references",
            signal: "using `lsp_references`",
            action: "",
            ideas: "",
            pages: &[
                "Give `symbol` or 1-based `line`(+`character`).",
                "Source file (workspace-relative or absolute).",
                "Symbol to locate (first occurrence in the file).",
                "Use this OR line/character.",
                "1-based line of the symbol (alternative to `symbol`).",
                "1-based column on `line` (default 1).",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "lsp_hover",
            signal: "using `lsp_hover`",
            action: "",
            ideas: "",
            pages: &[
                "Give `symbol` or 1-based `line`(+`character`).",
                "Source file (workspace-relative or absolute).",
                "Symbol to locate (first occurrence in the file).",
                "Use this OR line/character.",
                "1-based line of the symbol (alternative to `symbol`).",
                "1-based column on `line` (default 1).",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "lsp_symbols",
            signal: "using `lsp_symbols`",
            action: "",
            ideas: "",
            pages: &["Source file to outline (workspace-relative or absolute)."],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "lsp_workspace_symbol",
            signal: "using `lsp_workspace_symbol`",
            action: "",
            ideas: "",
            pages: &[
                "A server must be warm first (run lsp_symbols/lsp_diagnostics on any project file).",
                "Symbol name (or prefix/substring) to search for.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "ui_inspect",
            signal: "using `ui_inspect`",
            action: "",
            ideas: "",
            pages: &[
                "Use format=cells for exact Ratatui symbols, color variants, underline color, modifiers, and skip flags.",
                "Page an immutable cached snapshot without drawing a new frame.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "ui_verify",
            signal: "using `ui_verify`",
            action: "",
            ideas: "",
            pages: &[
                "Page the immutable result later with ui_inspect(snapshot_id=...).",
                "Optional exact campaign id from the loaded report catalog.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "video_cut, continued",
            signal: "using `video_cut`, continued",
            action: "",
            ideas: "",
            pages: &[
                "output height (default: first clip's)",
                "output frame rate (default 24)",
            ],
        },
    ],
};
