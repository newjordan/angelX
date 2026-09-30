//! ⠣ gh — Volume II, the tool library: finding your way around the code —
//! outlines, directory listings, search, definitions, git status and the self
//! map.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠣';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "navigation",
    surface: "outlines, listings, search, definitions, status and the self map",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "outline",
            signal: "using `outline`",
            action: "",
            ideas: "",
            pages: &["path inside the workspace (relative or absolute)"],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "list_dir",
            signal: "using `list_dir`",
            action: "",
            ideas: "",
            pages: &[
                "filename fragment to rank first (exact, substring, then fuzzy); does not filter",
                "basename glob to rank first; does not filter",
                "zero-based entry offset in the sorted listing",
                "page size (default 700); 24000-byte page window also applies; omitted counts and continuation are reported",
                "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
                "include hidden paths (default false); .git and credential files remain excluded",
                "workspace-relative directory (default '.')",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "grep",
            signal: "using `grep`",
            action: "",
            ideas: "",
            pages: &[
                "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
                "include hidden paths (default false); .git and credential files remain excluded",
                "regular expression",
                "file or directory inside the workspace, relative or absolute (default '.'); may be combined with `paths` — both are merged and deduplicated",
                "1-8 files/directories to search as one sorted, deduplicated union; both `path` and `paths` together are accepted (merged, deduplicated)",
                "deterministic continuation cursor from a prior grep receipt; only files lexically after this workspace-relative path are considered",
                "workspace-relative files to omit (maximum 128); useful for bounded explicit resumption/exclusion",
                "case-insensitive (default false)",
                "lines before and after each match (default 0, maximum 10); overlapping windows are merged",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "find_files",
            signal: "using `find_files`",
            action: "",
            ideas: "",
            pages: &[
                "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
                "include hidden paths (default false); .git and credential files remain excluded",
                "glob pattern, matched against workspace-relative paths",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "defs",
            signal: "using `defs`",
            action: "",
            ideas: "",
            pages: &[
                "Set include_source with 1-8 discovered file paths to collect bounded definition and reference windows with file hashes in one read per file.",
                "Provide exactly one of name/names.",
                "optional workspace file or directory to scope; outside targets are refused",
                "one symbol name to locate",
                "1-8 symbol names to resolve in one workspace scan",
                "case-insensitive symbol matching (default false)",
                "return JSON source windows from explicit paths (default false)",
                "discovered source files for include_source; no directories",
                "total returned source text bytes, excluding JSON metadata (default 16384)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "git_status",
            signal: "using `git_status`",
            action: "",
            ideas: "",
            pages: &[
                "A compact orientation before editing or committing; complements git_diff (which shows the actual changes).",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "self_map",
            signal: "using `self_map`",
            action: "",
            ideas: "",
            pages: &[
                "Use this to understand or plan changes to yourself.",
                "source module stem, e.g. agent/harness/registry (the layer prefix is optional)",
                "a module stem (e.g. \"harness\", \"swarm\", \"tools/nav\") for its outline only",
                "also persist the full map to SELF.md at the crate root",
            ],
        },
    ],
};
