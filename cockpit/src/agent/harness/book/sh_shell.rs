//! ⠩ sh — Volume II, the tool library: the shell, background processes,
//! verifier runners, experiments and measured comparisons. One section per
//! tool; its pages are the sentences that told the model how to use that tool,
//! verbatim, moved off the schema. The schema keeps what the tool does and ends
//! with the section's route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠩';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "shell",
    surface: "the shell, processes, runs and measurements",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "shell",
            signal: "using `shell`",
            action: "",
            ideas: "",
            pages: &[
                "Act on actual tool evidence: preserve the earliest prerequisite failure, check usable input before dependent measurements, and discover optional dependencies from real errors.",
                "Do not score failed input as zero performance.",
                "Never use shell `sleep` to poll a job or submission; keep doing useful work and use one later status snapshot.",
                "Omit scope options for builds, benchmarks, installs and background jobs.",
                "Act on actual errors: keep the earliest prerequisite failure, check usable input before dependent measurements, use allowed scratch, and do not score failed input as zero performance.",
                "Explicit restrictions stay authoritative; report a concrete blocker if no permitted approach works.",
                "Judge from actual exit status and output; quoted text, a quiet success, and a valid `$url` are not empty-input or zero-frame evidence.",
                "This sealed task requires foreground process ownership: run long builds directly; nohup/disown/setsid and unmanaged `&` jobs are rejected.",
                "This sealed task permits read-only Git inspection only; edit files directly and do not stash/add/reset/restore/checkout/clean/commit/switch/fetch/merge/rebase/push.",
                "YOLO is active for this headless coding task: writes/network are unrestricted, but default cwd is the task workspace — start with local `ls`/`find .`/`tests` there.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "cargo",
            signal: "using `cargo`",
            action: "",
            ideas: "",
            pages: &[
                "Use this to pull crates and build/calibrate code.",
                "Pass subcommand + flags as `args`, e.g. build or test.",
                "Use available local dependencies; network fetches are unavailable in this confined experiment.",
                "cargo subcommand + flags",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "lint",
            signal: "using `lint`",
            action: "",
            ideas: "",
            pages: &[
                "Select runtime explicitly for mixed roots.",
                "Runtime selection.",
                "Auto requires one root language; mixed projects need explicit selection.",
                "Extra cargo clippy arguments on the Rust route; other lint adapters are unsupported.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "proc_run",
            signal: "using `proc_run`",
            action: "",
            ideas: "",
            pages: &[
                "Do not redirect output: proc_status reads the captured log.",
                "Use for daemons and slow jobs — an inference server (llama-server, vllm serve), a dev server, a long build — then keep working.",
                "Take one proc_status snapshot only when its result can change your next action; if no independent work remains, use proc_wait for a bounded wait that returns early on completion rather than shell sleep.",
                "Use `shell` for commands that finish quickly.",
                "command line (sh -c)",
                "short label for the handle/log (default: first word)",
                "working directory (default: the workspace)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "proc_status",
            signal: "using `proc_status`",
            action: "",
            ideas: "",
            pages: &[
                "If still running, do useful independent work.",
                "If all remaining work depends on this job, use proc_wait instead of shell sleep.",
                "Use contains with an id to search captured compiler errors by literal substring; captured host logs are not workspace files for grep/read_file.",
                "handle id from proc_run",
                "log lines to show for a single id (default 40)",
                "With id only: literal case-sensitive log filter, 1–256 UTF-8 bytes, no newlines.",
                "Searches trailing 8 MiB of each retained log file; tail_lines limits matching lines.",
                "ignored (legacy).",
                "Snapshot only; never parks the turn.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "proc_wait",
            signal: "using `proc_wait`",
            action: "",
            ideas: "",
            pages: &[
                "Wait for one background job when no useful independent work remains.",
                "Use instead of shell sleep; use proc_status for an immediate snapshot.",
                "handle id from proc_run",
                "captured log lines (default 40, maximum 400)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "tool_repair",
            signal: "using `tool_repair`",
            action: "",
            ideas: "",
            pages: &[
                "Call only when the user has explicitly asked to investigate or fix a broken tool in this session.",
                "Diagnose-only by default; pass repair=true only after the user approves making changes.",
                "Set true ONLY after the user approves making changes.",
                "command/profile to repair, e.g. auto or gh (default auto)",
                "apply local repairs (mutates ~/.local/bin symlinks and global git config); default false — diagnose only.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "loop_research",
            signal: "using `loop_research`",
            action: "",
            ideas: "",
            pages: &[
                "Continue ordinary work, inspect status/results, stop when desired, and apply/verify useful patches yourself.",
                "suggest: optional unmeasured ideas; no fabricated rewards",
                "Defaults to loop objective; selects a separate learning scope if changed",
                "Defaults to loop verifier; explicit null explores without learning rewards",
                "suggest: choose any methods; defaults to all",
                "suggest: deterministic exploration seed; default 0",
                "run: the experimental approach to try",
                "run: stable label for bandit comparisons; default direct",
                "run: opt into baseline plus candidate (two attempts); default false",
                "run: include retrieved experimental snippets as task context; default true",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "rl_campaign",
            signal: "using `rl_campaign`",
            action: "",
            ideas: "",
            pages: &[
                "Continue useful work while it runs; results retains artifacts across iterations and resumes.",
                "Use real checks, inspect candidates, and submit a verified winner when ready.",
                "Optional research objective; defaults to loop task",
                "Actual verifier command; defaults to loop acceptance command",
                "Campaign rounds; default 1",
                "Samples per training group; default 3",
                "Samples per measured comparison case; at least 2, default 2",
                "Existing verifier-owned paths relative to each case source",
                "Existing independently authored held-out source workspace",
                "results: optional exact campaign id, within this workspace",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "benchmark_compare",
            signal: "using `benchmark_compare`",
            action: "",
            ideas: "",
            pages: &[
                "Samples must be nonnegative and matched in order; separate diagnostic, full-development and official cohorts.",
                "Metric and unit, e.g. kernel latency (ms).",
            ],
        },
    ],
};
