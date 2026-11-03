//! ⠏ p — background jobs the task started (`proc_run`).
//!
//! Headless tasks have no UI consumer for job completions, so a finished job
//! reaches the model as `⠏⠃` (clean exit) or `⠏⠉` (failed or unknown exit)
//! at the next hop boundary, with the job's receipt in the ledger. A task that
//! answers while it still owns running jobs carries `⠏⠁` to the checkpoint.

use super::{Primary, Raise, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/p_processes.book");
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
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: DONE,
            name: "done",
            signal: "a background job exited cleanly (receipt below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'2'),
        },
        Sub {
            route: FAILED,
            name: "failed",
            signal: "a background job failed or its exit is unknown (receipt below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'3'),
        },
        Sub {
            route: PROC_STARTED,
            name: "proc-started",
            signal: "a background job just started (its id above)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'4'),
        },
        Sub {
            route: PROC_RUNNING,
            name: "proc-running",
            signal: "the job is still running",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'5'),
        },
        Sub {
            route: TOOL_IDLE,
            name: "tool-idle",
            signal: "the tool went silent past its limit and its child tree was stopped",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'6'),
        },
        Sub {
            route: GIT_GUARD,
            name: "git-guard",
            signal: "this sealed task keeps git inspection-only",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: SLEEP_GUARD,
            name: "sleep-guard",
            signal: "a sleep inside a tool call was refused",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'8'),
        },
        Sub {
            route: DETACH_GUARD,
            name: "detach-guard",
            signal: "this sealed task keeps process ownership in the foreground",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'9'),
        },
        Sub {
            route: SANDBOX,
            name: "sandbox",
            signal: "the command met the sandbox (the page named on the result applies)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'0'),
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
