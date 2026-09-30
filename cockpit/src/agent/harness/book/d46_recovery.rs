//! ⠨ (dots 4-6) — Volume VI, the replies: recovery. A tool result that failed
//! keeps its facts inline — the error, the path, the id, the tags, the counts,
//! the seconds — and names the way back as a page address (`⠨⠉⠁`), on its own
//! line after the facts or beside the clause it replaced. Virtual URLs, merge
//! conflicts, replace misses, hashline patches, staged edits, the shell's
//! redirection scope, stopped processes, the pinned toolchain, Cargo coverage
//! and background-job output.

use super::{DIGITS, Primary, Route, Sub};

pub(crate) const CELL: char = '⠨';
pub(crate) const VIRTUAL: Route = Route::new(CELL, '⠁');
pub(crate) const CONFLICTS: Route = Route::new(CELL, '⠃');
pub(crate) const REPLACE: Route = Route::new(CELL, '⠉');
pub(crate) const HASHLINE: Route = Route::new(CELL, '⠙');
pub(crate) const STAGED: Route = Route::new(CELL, '⠑');
pub(crate) const SHELL_SCOPE: Route = Route::new(CELL, '⠋');
pub(crate) const STOPPED: Route = Route::new(CELL, '⠛');
pub(crate) const TOOLCHAIN: Route = Route::new(CELL, '⠓');
pub(crate) const COVERAGE: Route = Route::new(CELL, '⠊');
pub(crate) const JOB_OUTPUT: Route = Route::new(CELL, '⠚');

/// One page on its own: its route's cells and its digit, three cells. A reply
/// sends a one-sentence directive as its page; the values it names ride
/// beside the address as data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Page {
    pub(crate) route: Route,
    pub(crate) number: usize,
}

impl Page {
    pub(crate) const fn new(route: Route, number: usize) -> Self {
        Self { route, number }
    }

    pub(crate) fn cells(self) -> String {
        format!("{}{}", self.route.cells(), DIGITS[self.number - 1])
    }

    /// The verbatim sentence this address decodes to.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn text(self) -> &'static str {
        self.route.sub().pages[self.number - 1]
    }
}

/// Several pages as one run of addresses, in order.
pub(crate) fn run(pages: &[Page]) -> String {
    pages.iter().map(|page| page.cells()).collect()
}

// ⠨⠁ virtual URLs
pub(crate) const SKILL_TOOL: Page = Page::new(VIRTUAL, 1);
pub(crate) const GH_MUTATE: Page = Page::new(VIRTUAL, 2);
pub(crate) const SKILL_BODY: Page = Page::new(VIRTUAL, 3);

// ⠨⠃ conflicts
pub(crate) const CONFLICT_READ_FIRST: Page = Page::new(CONFLICTS, 1);
pub(crate) const CONFLICT_UNKNOWN: Page = Page::new(CONFLICTS, 2);
pub(crate) const CONFLICT_WRITE_ONE: Page = Page::new(CONFLICTS, 3);
pub(crate) const CONFLICT_READ_FILE: Page = Page::new(CONFLICTS, 4);
pub(crate) const CONFLICT_SCOPE_ONE: Page = Page::new(CONFLICTS, 5);
pub(crate) const CONFLICT_SCOPES: Page = Page::new(CONFLICTS, 6);
pub(crate) const CONFLICT_REREAD: Page = Page::new(CONFLICTS, 7);
pub(crate) const CONFLICT_REREAD_CAREFULLY: Page = Page::new(CONFLICTS, 8);
pub(crate) const CONFLICT_FOOTER: Page = Page::new(CONFLICTS, 9);
pub(crate) const CONFLICT_RESOLVE: Page = Page::new(CONFLICTS, 10);

// ⠨⠉ replace misses
pub(crate) const STALE_EDIT: Page = Page::new(REPLACE, 1);
pub(crate) const NOT_UNIQUE: Page = Page::new(REPLACE, 2);
pub(crate) const ENCLOSING: Page = Page::new(REPLACE, 3);
pub(crate) const TRIMMED_REGIONS: Page = Page::new(REPLACE, 4);
pub(crate) const NO_LINE_MATCHES: Page = Page::new(REPLACE, 5);
pub(crate) const INDENTATION: Page = Page::new(REPLACE, 6);
pub(crate) const CLOSEST_REGION: Page = Page::new(REPLACE, 7);
pub(crate) const MORE_CONTEXT: Page = Page::new(REPLACE, 8);

// ⠨⠙ hashline
pub(crate) const LINE_ID: Page = Page::new(HASHLINE, 1);
pub(crate) const STALE_REM: Page = Page::new(HASHLINE, 2);
pub(crate) const RECOVERY_FAILED: Page = Page::new(HASHLINE, 3);
pub(crate) const NO_SNAPSHOT: Page = Page::new(HASHLINE, 4);
pub(crate) const PLAIN_OPS: Page = Page::new(HASHLINE, 5);
pub(crate) const BLOCK_OPENER: Page = Page::new(HASHLINE, 6);

// ⠨⠑ staged edits and patches
pub(crate) const STAGED_ACCEPT: Page = Page::new(STAGED, 1);
pub(crate) const STAGED_REJECT: Page = Page::new(STAGED, 2);
pub(crate) const STAGE_ONE: Page = Page::new(STAGED, 3);
pub(crate) const STAGED_LIST: Page = Page::new(STAGED, 4);
pub(crate) const RESTAGE: Page = Page::new(STAGED, 5);
pub(crate) const COMBINE_HUNKS: Page = Page::new(STAGED, 6);
pub(crate) const RESOLVE_ACTIONS: Page = Page::new(STAGED, 7);
pub(crate) const STAGE_HASHLINE_SECTIONS: Page = Page::new(STAGED, 8);
pub(crate) const STAGE_HASHLINE_ONLY: Page = Page::new(STAGED, 9);

// ⠨⠋ the shell's redirection scope
pub(crate) const LITERAL_PATH: Page = Page::new(SHELL_SCOPE, 1);
pub(crate) const SCRATCH_PATH: Page = Page::new(SHELL_SCOPE, 2);
pub(crate) const SCRATCH_OUTPUT: Page = Page::new(SHELL_SCOPE, 3);

// ⠨⠛ a stopped or unstartable process
pub(crate) const TIMEOUT_KNOB: Page = Page::new(STOPPED, 1);
pub(crate) const IDLE_FLOOR: Page = Page::new(STOPPED, 2);
pub(crate) const LIVE_DIRECTORY: Page = Page::new(STOPPED, 3);

// ⠨⠓ the pinned toolchain and the runners
pub(crate) const REPIN: Page = Page::new(TOOLCHAIN, 1);
pub(crate) const CARGO_FALLBACK: Page = Page::new(TOOLCHAIN, 2);
pub(crate) const OMIT_FLAG: Page = Page::new(TOOLCHAIN, 3);
pub(crate) const HUNG_SUITE: Page = Page::new(TOOLCHAIN, 4);
pub(crate) const RUNNERS: Page = Page::new(TOOLCHAIN, 5);
pub(crate) const SUITE_WITH_SHELL: Page = Page::new(TOOLCHAIN, 6);
pub(crate) const PLAN_WITH_SHELL: Page = Page::new(TOOLCHAIN, 7);

// ⠨⠊ Cargo coverage
pub(crate) const SELECT_PACKAGE: Page = Page::new(COVERAGE, 1);
pub(crate) const EARLIER_WRITES: Page = Page::new(COVERAGE, 2);
pub(crate) const REPORT_SCOPE: Page = Page::new(COVERAGE, 3);
pub(crate) const FUTURE_SCOPE: Page = Page::new(COVERAGE, 4);
pub(crate) const MISSING_INVENTORY: Page = Page::new(COVERAGE, 5);
pub(crate) const MISSING_EVIDENCE: Page = Page::new(COVERAGE, 6);

// ⠨⠚ background-job output
pub(crate) const RETAINED_HANDLE: Page = Page::new(JOB_OUTPUT, 1);
pub(crate) const NO_COMPLETION_NOTICE: Page = Page::new(JOB_OUTPUT, 2);
pub(crate) const CAPTURED_OUTPUT: Page = Page::new(JOB_OUTPUT, 3);
pub(crate) const INSPECT_RECEIPT: Page = Page::new(JOB_OUTPUT, 4);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "recovery",
    surface: "failed tool results and the way back: edits, conflicts, patches, the shell, processes, the toolchain",
    subs: &[
        Sub {
            route: VIRTUAL,
            name: "virtual-urls",
            signal: "a virtual URL cannot serve that call (the scheme above)",
            action: "",
            ideas: "",
            pages: &[
                "use the skill tool or edit the skill files on disk",
                "use gh CLI or the browser to mutate",
                "read skill://NAME for full body",
            ],
        },
        Sub {
            route: CONFLICTS,
            name: "conflicts",
            signal: "a merge-conflict id or region (the conflict:// fact above)",
            action: "",
            ideas: "",
            pages: &[
                "read a conflicted file first so conflict:// ids are assigned",
                "read a conflicted file first, or call read_file path=conflict:// to list active ids",
                "write conflict://N or conflict://* with a resolution",
                "read the conflicted file first",
                "use conflict://N/ours|theirs|base",
                "use ours, theirs, or base",
                "re-read",
                "re-read and resolve carefully",
                "resolve with write_file path=conflict://N content=@ours|@theirs|@base|custom:",
                "Resolve: write_file path=conflict://{} content=@ours|@theirs|@base|@both|custom",
            ],
        },
        Sub {
            route: REPLACE,
            name: "replace",
            signal: "str_replace or multi_edit found no unique match (the miss above)",
            action: "",
            ideas: "",
            pages: &[
                "re-read the file and retry",
                "add more context that distinguishes the target.",
                "Include surrounding unique lines (e.g. the enclosing function) in `old`.",
                "Add unique surrounding context.",
                "Re-read the file before editing.",
                "Copy the exact indentation shown above.",
                "Correct `old` to the exact lines above (they may have changed since your last read).",
                "add more context",
            ],
        },
        Sub {
            route: HASHLINE,
            name: "hashline",
            signal: "a hashline patch was refused (the reason above)",
            action: "",
            ideas: "",
            pages: &[
                "a line id is the bare 1-based number shown in the read_file hashline gutter (e.g. `255`), and a range is `A.=B` (e.g. `255.=257`); `=`, `-`, `:` and `#hash` suffixes are not ranges",
                "Re-read the file before REM.",
                "Re-read the file and rebuild the edit.",
                "re-read the file and rebuild the edit",
                "use plain SWAP/DEL/INS",
                "re-read and use plain line ranges, or point at a real block opener ({, [, (, heading, or indented suite)",
            ],
        },
        Sub {
            route: STAGED,
            name: "staged",
            signal: "a staged edit or a patch needs its next step (the id above)",
            action: "",
            ideas: "",
            pages: &[
                "Accept: resolve_edit id={} action=accept",
                "Reject: resolve_edit id={} action=reject",
                "apply_patch with stage=true to propose one",
                "resolve_edit action=list",
                "re-stage",
                "combine its hunks into one operation",
                "use accept, reject, or list",
                "stage=true is only supported for hashline patches (sections with [path#tag])",
                "stage=true is only supported for hashline patches",
            ],
        },
        Sub {
            route: SHELL_SCOPE,
            name: "shell-scope",
            signal: "a sealed task refused a shell output redirection (the target above)",
            action: "",
            ideas: "",
            pages: &[
                "Use a literal editable workspace path or a literal /tmp/... path for scratch output.",
                "use a literal /tmp/... path for scratch output",
                "Use a literal /tmp/... path for scratch output.",
            ],
        },
        Sub {
            route: STOPPED,
            name: "stopped",
            signal: "the tool's process was stopped or could not start (the receipt above)",
            action: "",
            ideas: "",
            pages: &[
                "raise/disable via ANGEL_TOOL_TIMEOUT",
                "if this was a legitimate wait (remote validation, polling, downloads) raise ANGEL_TOOL_IDLE_FLOOR_SECS or use proc_run",
                "cd to a live directory and retry",
            ],
        },
        Sub {
            route: TOOLCHAIN,
            name: "toolchain",
            signal: "the pinned toolchain or a runner could not serve this call (the reason above)",
            action: "",
            ideas: "",
            pages: &[
                "restart Angel to re-pin the toolchain",
                "Fallback: run `cargo test` through the `shell` tool (the toolchain on PATH is not affected by this pin)",
                "omit that flag",
                "Inspect for an infinite loop or a blocking wait; legitimate slow tests may need a larger timeout.",
                "use rust|js|python|go|swift",
                "Run the suite with `shell` and its own command.",
                "run the suite with `shell`",
            ],
        },
        Sub {
            route: COVERAGE,
            name: "coverage",
            signal: "Cargo passed, but its coverage of the changed sources is not established (coverage above)",
            action: "",
            ideas: "",
            pages: &[
                "Select the required package; unrelated packages need not be rebuilt.",
                "This result is useful command-local evidence; repeating a check or selecting another package cannot attest those earlier writes.",
                "Report the scope limitation or use independently pinned task acceptance.",
                "Use read_only/write_paths for future commands; neither resets prior uncertainty.",
                "Keep this result command-local and report the scope limitation; package selection alone cannot restore missing inventory evidence.",
                "Keep this result command-local and report the evidence limitation; do not repeat unchanged checks as a cure.",
            ],
        },
        Sub {
            route: JOB_OUTPUT,
            name: "job-output",
            signal: "a background job's captured output is behind proc_status (the id above)",
            action: "",
            ideas: "",
            pages: &[
                "retained process handle/receipt requires proc_status.",
                "Automatic completion notification is unavailable for this session.",
                "captured output: proc_status id={id}, optional contains filter",
                "inspect with proc_status id={}",
            ],
        },
    ],
};
