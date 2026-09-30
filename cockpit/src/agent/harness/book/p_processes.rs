//! ⠏ p — background jobs the task started (`proc_run`).
//!
//! Headless tasks have no UI consumer for job completions, so a finished job
//! reaches the model as `⠏⠃` (clean exit) or `⠏⠉` (failed or unknown exit)
//! at the next hop boundary, with the job's receipt in the ledger. A task that
//! answers while it still owns running jobs carries `⠏⠁` to the checkpoint.

use super::{Primary, Raise, Route, Sub};
use crate::agent::tools::proc::ProcCompletion;

pub(crate) const CELL: char = '⠏';
pub(crate) const LIVE: Route = Route::new(CELL, '⠁');
pub(crate) const DONE: Route = Route::new(CELL, '⠃');
pub(crate) const FAILED: Route = Route::new(CELL, '⠉');
pub(crate) const PROC_STARTED: Route = Route::new(CELL, '⠙');
pub(crate) const PROC_RUNNING: Route = Route::new(CELL, '⠑');
pub(crate) const TOOL_IDLE: Route = Route::new(CELL, '⠋');
pub(crate) const GIT_GUARD: Route = Route::new(CELL, '⠛');
pub(crate) const SLEEP_GUARD: Route = Route::new(CELL, '⠓');
pub(crate) const DETACH_GUARD: Route = Route::new(CELL, '⠊');
pub(crate) const SANDBOX: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "processes",
    surface: "background jobs, the shell's guards and the sandbox",
    subs: &[
        Sub {
            route: LIVE,
            name: "live",
            signal: "jobs this task started are still running",
            action: "",
            ideas: "- `proc_wait` for the outcome; a task that exits stops the jobs it owns.\n\
                    - `proc_status` shows a job without blocking.",
            pages: &[
                "Your final answer was deferred because this task still owned running background work.",
                "Inspect proc_status and finish from its actual outcome.",
                "If a job is no longer needed, explicitly stop it with proc_stop before answering.",
                "Do not report an in-flight build as complete; task exit stops remaining jobs.",
            ],
        },
        Sub {
            route: DONE,
            name: "done",
            signal: "a background job exited cleanly (receipt below)",
            action: "",
            ideas: "",
            pages: &[
                "Inspect proc_status id={id} for captured output.",
                "Process exit is not benchmark acceptance or a verified solve.",
            ],
        },
        Sub {
            route: FAILED,
            name: "failed",
            signal: "a background job failed or its exit is unknown (receipt below)",
            action: "",
            ideas: "",
            pages: &[
                "Inspect proc_status id={id} for captured output.",
                "Process exit is not benchmark acceptance or a verified solve.",
                "Background work finished while your answer was being generated.",
                "Inspect its proc_status outcome and incorporate it before finishing.",
            ],
        },
        Sub {
            route: PROC_STARTED,
            name: "proc-started",
            signal: "a background job just started (its id above)",
            action: "",
            ideas: "",
            pages: &[
                "Read captured output through `proc_status` id={id}; use contains to filter errors.",
                "Snapshot with `proc_status` (no wait) while you keep working; check later.",
                "Use `proc_stop` to kill the whole tree.",
            ],
        },
        Sub {
            route: PROC_RUNNING,
            name: "proc-running",
            signal: "the job is still running",
            action: "",
            ideas: "",
            pages: &[
                "still running — advance useful independent work.",
                "If this job blocks all remaining work, use proc_wait; do not substitute shell sleep.",
                "still running — snapshot only.",
                "wait_ms was ignored.",
                "Advance independent work; if this job blocks all remaining work, use proc_wait instead of shell sleep.",
            ],
        },
        Sub {
            route: TOOL_IDLE,
            name: "tool-idle",
            signal: "the tool went silent past its limit and its child tree was stopped",
            action: "",
            ideas: "",
            pages: &["retry with explicit input or use proc_run"],
        },
        Sub {
            route: GIT_GUARD,
            name: "git-guard",
            signal: "this sealed task keeps git inspection-only",
            action: "",
            ideas: "",
            pages: &[
                "Git is inspection-only: use status/diff/log/show/grep/ls-files/rev-parse, and edit source files directly.",
                "Do not stash, add, reset, restore, checkout, clean, commit, switch, fetch, merge, rebase, or push.",
            ],
        },
        Sub {
            route: SLEEP_GUARD,
            name: "sleep-guard",
            signal: "a sleep inside a tool call was refused",
            action: "",
            ideas: "",
            pages: &[
                "Run the foreground build/benchmark directly and let it own its wait, or do other useful work before taking one later status snapshot.",
                "To check on a running background command or build, inspect its log or status directly without a long sleep.",
            ],
        },
        Sub {
            route: DETACH_GUARD,
            name: "detach-guard",
            signal: "this sealed task keeps process ownership in the foreground",
            action: "",
            ideas: "",
            pages: &[
                "Run builds and benchmarks in the foreground; do not use nohup/disown/setsid or leave an `&` job without `wait`.",
                "The runner supplies a long foreground tool deadline, so detached polling is unnecessary.",
                "(Opt out: ANGEL_TASK_SHELL_NO_DETACH=0.)",
            ],
        },
        Sub {
            route: SANDBOX,
            name: "sandbox",
            signal: "the command met the sandbox (the page named on the result applies)",
            action: "",
            ideas: "",
            pages: &[
                "[sandbox: this command tried to start its own macOS sandbox-exec inside Angel's confinement, which the kernel refuses (sandbox_apply: Operation not permitted). Angel already confines writes, so run SwiftPM with `--disable-sandbox` (swift build/test --disable-sandbox …) or give the tool the equivalent flag; the code you changed did not cause this.]",
                "[sandbox: this command's own bubblewrap sandbox could not start inside Angel's confinement. That is a harness/launch setting, not your change: the operator must launch with ANGEL_SANDBOX_BACKEND=bwrap on a namespace-capable host. Do not retry the same command or disable the project's sandbox; report the blocker.]",
                "[sandbox: privilege escalation is disabled (no_new_privs), so sudo/system package managers cannot work here — this is not a missing password. Self-serve user-level instead: pip/npm/cargo installs, or download a static binary into ~/.local/bin (writable, on PATH). Network is available. The operator can lift confinement with ANGEL_SANDBOX=0 or /yolo.]",
                "[sandbox: this bootstrap installer tried to write user state/configuration outside the sanctioned install roots. Retrying, chmod, or copy workarounds cannot widen Landlock. Use an already-installed binary, or ask the operator to run the installer outside the cockpit / enable `/yolo on` for this explicit install.]",
                "[workspace boundary: current workspace is {workspace}. Creating a different home-level project or editing a symlink target outside the granted roots is not permitted by this session. Restart in the intended project or use an explicitly operator-approved scope change; retrying cp/chmod does not change the grant. No successful host change may be claimed without checking the actual destination.]",
            ],
        },
    ],
};

/// Raises for jobs that just finished: one route per outcome, every receipt in
/// the evidence. A failed or unknown exit is `⠏⠉`; clean exits are `⠏⠃`.
pub(crate) fn completions(finished: &[ProcCompletion]) -> Vec<Raise> {
    let receipts = |failed: bool| {
        finished
            .iter()
            .filter(|job| (job.exit_code != Some(0)) == failed)
            .map(ProcCompletion::task_message)
            .collect::<Vec<_>>()
    };
    [(FAILED, receipts(true)), (DONE, receipts(false))]
        .into_iter()
        .filter(|(_, receipts)| !receipts.is_empty())
        .map(|(route, receipts)| Raise::new(route, receipts.join("\n")))
        .collect()
}

/// Whether any of these completions failed or ended without a known exit.
/// The facts of each finished job, beside the stamps as data: `[id] name state`.
pub(crate) fn receipt_lines(finished: &[ProcCompletion]) -> String {
    finished
        .iter()
        .map(|job| format!("[{}] {} {}", job.id, job.name, job.state))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn any_failed(finished: &[ProcCompletion]) -> bool {
    finished.iter().any(|job| job.exit_code != Some(0))
}
