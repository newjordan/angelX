//! ⠾ with — Volume II, the tool library: building and running — tests,
//! checks, formatting and stopping background jobs, then the continued
//! sections of the shell and loop tools.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠾';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "build",
    surface: "tests, checks, formatting and stopping jobs; the shell and loop tools, continued",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "run_tests",
            signal: "using `run_tests`",
            action: "",
            ideas: "",
            pages: &[
                "typed runtime selection; an explicit entrypoint selects its runtime, otherwise auto requires one root language (optional)",
                "Explicit native test entrypoint; node bypasses package scripts, unittest accepts discover/modules/files, pytest requires an immutable system installation.",
                "alias of runtime for the scan fallback: rust|js|python|go|swift (optional)",
                "Rust: cargo test flags.",
                "Node: confined test files/globs and test-name/skip-pattern.",
                "Python: unittest -s directory, -p pattern, -k.",
                "Other runners: appended verbatim (optional)",
                "run the suite of this subdirectory of the workspace (mono-repos: e.g. `sidecar/forge` for its python tests, `cockpit` for that crate); the runner is chosen from that directory's own files (optional)",
                "alias of `dir` for repos whose crates live one directory down (e.g. cockpit/, harness/); default = the largest (optional)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "check",
            signal: "using `check`",
            action: "",
            ideas: "",
            pages: &[
                "Runtime selection.",
                "Auto requires one root language; mixed projects need explicit selection.",
                "Rust: cargo check flags.",
                "Node/Python: confined source files or simple filename globs; default scans up to 256 visible source files, excluding generated/vendor directories.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "fmt",
            signal: "using `fmt`",
            action: "",
            ideas: "",
            pages: &["check only, don't modify (default false)"],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "proc_stop",
            signal: "using `proc_stop`",
            action: "",
            ideas: "",
            pages: &["handle id from proc_run"],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "shell, continued",
            signal: "using `shell`, continued",
            action: "",
            ideas: "",
            pages: &[
                "Do not inventory `/`, `$HOME`, or unrelated repos; prefer `read_file`/`grep` inside the workspace.",
                "YOLO SMART is active: workspace shell/write batches do not wait for interactive approval — act decisively with tools (edit, build, test, fix).",
                "Prefer concrete tool-backed code changes over status prose.",
                "shell command.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "loop_research, continued",
            signal: "using `loop_research`, continued",
            action: "",
            ideas: "",
            pages: &[
                "results: exact retained run within this workspace",
                "results: retained run count; default 5",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "rl_campaign, continued",
            signal: "using `rl_campaign`, continued",
            action: "",
            ideas: "",
            pages: &["results: number of retained campaigns; default 5"],
        },
    ],
};
