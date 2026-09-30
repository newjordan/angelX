//! ⠘ d45 — the iteration: what one `/loop` iteration is told about the last
//! one. Its setbacks ride under the setback header (`⠳⠑`), its notes under the
//! note header (`⠳⠚`). A one-sentence setback is sent as its page address, a
//! longer one as its route; the counts, errors and summaries ride beside them
//! as data. The curated state an iteration works from (`⠘⠋`) and the project
//! brief it is handed (`⠘⠛`, `⠘⠓`) are labelled by page address: the labels
//! are the pages, the problem, findings, machine facts and paths are the data.
//! The loop's own research and campaign tools name their notes here too
//! (`⠘⠊`), and the calls they refuse for their arguments (`⠘⠚`).
//! Every page is its original sentence, verbatim. Volume V, the long run.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠘';
pub(crate) const PROGRESS: Route = Route::new(CELL, '⠁');
pub(crate) const FAULT: Route = Route::new(CELL, '⠃');
pub(crate) const PODRACE_STALL: Route = Route::new(CELL, '⠉');
pub(crate) const RECEIPT: Route = Route::new(CELL, '⠙');
pub(crate) const FIRST_CANDIDATE: Route = Route::new(CELL, '⠑');
pub(crate) const CURATED: Route = Route::new(CELL, '⠋');
pub(crate) const BRIEF: Route = Route::new(CELL, '⠛');
pub(crate) const BRIEF_BENCHMARK: Route = Route::new(CELL, '⠓');
pub(crate) const LOOP_TOOLS: Route = Route::new(CELL, '⠊');
pub(crate) const LOOP_TOOL_REFUSALS: Route = Route::new(CELL, '⠚');

// `⠘⠁`: each page is sent alone, as its address.
pub(crate) const FALLBACK: &str = "⠘⠁⠁";
pub(crate) const TOOL_ERRORS: &str = "⠘⠁⠃";
pub(crate) const UNVERIFIED: &str = "⠘⠁⠉";
pub(crate) const DONE_UNVERIFIED: &str = "⠘⠁⠙";
pub(crate) const STALLED: &str = "⠘⠁⠑";
pub(crate) const SUBMIT_DUE: &str = "⠘⠁⠋";

// `⠘⠃`: each page is sent alone, as its address.
pub(crate) const ERRORED: &str = "⠘⠃⠁";
pub(crate) const HOP_HORIZON: &str = "⠘⠃⠃";
pub(crate) const PROVIDER_BLOCKED: &str = "⠘⠃⠉";
pub(crate) const BASELINE_DIED: &str = "⠘⠃⠙";
pub(crate) const VERIFY_DIED: &str = "⠘⠃⠑";
pub(crate) const ACCEPT_FAILED: &str = "⠘⠃⠋";
pub(crate) const TURN_STOPPED: &str = "⠘⠃⠛";

// `⠘⠊`: each page is sent alone, as its address (a label with its value).
pub(crate) const RESEARCH_PANICKED: &str = "⠘⠊⠁";
pub(crate) const RESEARCH_UNSTARTED: &str = "⠘⠊⠃";
pub(crate) const RESEARCH_APPROACH: &str = "⠘⠊⠉";
pub(crate) const RESEARCH_SNIPPETS: &str = "⠘⠊⠙";
pub(crate) const CAMPAIGN_NEEDS_VERIFIER: &str = "⠘⠊⠑";
pub(crate) const CAMPAIGN_UNKNOWN_ACTION: &str = "⠘⠊⠋";
pub(crate) const RESEARCH_NEEDS_LOOP: &str = "⠘⠊⠓";
pub(crate) const RESEARCH_NEEDS_VERIFIER: &str = "⠘⠊⠊";
pub(crate) const RESEARCH_UNKNOWN_ACTION: &str = "⠘⠊⠚";
pub(crate) const CAMPAIGN_NEEDS_LOOP: &str = "⠘⠊⠛";

// `⠘⠚`: each page is sent alone, as its address. A value of the wrong JSON
// type stays English, as in every tool; these are a required field missing or
// a value the tool itself rejects.
pub(crate) const CANDIDATE_NEEDS_IDEA: &str = "⠘⠚⠁";
pub(crate) const RUN_NEEDS_IDEA: &str = "⠘⠚⠃";
pub(crate) const RESEARCH_ID_INVALID: &str = "⠘⠚⠉";
pub(crate) const RESEARCH_ID_UNKNOWN: &str = "⠘⠚⠙";
pub(crate) const SAMPLES_TOO_FEW: &str = "⠘⠚⠑";
pub(crate) const AUDIT_NEEDS_TASK: &str = "⠘⠚⠋";
pub(crate) const AUDIT_NEEDS_VERIFY: &str = "⠘⠚⠛";
pub(crate) const AUDIT_NEEDS_SOURCE: &str = "⠘⠚⠓";
pub(crate) const CAMPAIGN_ID_INVALID: &str = "⠘⠚⠊";
pub(crate) const CAMPAIGN_ID_UNKNOWN: &str = "⠘⠚⠚";

/// `⠘⠑` without its blocked-attempt page: no attempt was blocked.
pub(crate) const NO_CANDIDATE: &str = "⠘⠑⠁⠘⠑⠉⠘⠑⠙";

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "iteration",
    surface: "what one loop iteration is told: setbacks, notes, its curated state, the brief",
    subs: &[
        Sub {
            route: PROGRESS,
            name: "progress",
            signal: "what the last iteration's reply did not establish (one page, its counts beside it)",
            action: "",
            ideas: "",
            pages: &[
                "the previous coordinator reply was an error/status fallback, not a result; restore the driver before claiming progress",
                "the previous iteration had {errors} error/incomplete result(s) across {calls} tool call(s); its prose did not reset stall detection — resolve the failed evidence chain first",
                "the previous iteration reported {unverified} unverified claim(s); validate them with concrete file/artifact/command/test/URL evidence before treating them as findings",
                "unverified done claim: continue the task and produce concrete verification evidence; no acceptance command is bound",
                "stalled without new verified progress: finish the line in flight, or change one concrete constraint if your evidence says it is exhausted; run the smallest discriminating check and use its result to choose the next action",
                "{measured} measured candidate(s) and no submission in the {since} iterations since the first measurement — the board is the instrument; submit the best measured candidate or identify and resolve the exact blocking command",
            ],
        },
        Sub {
            route: FAULT,
            name: "fault",
            signal: "the last iteration ended in a fault, not a result (one page, the error beside it)",
            action: "",
            ideas: "",
            pages: &[
                "the previous iteration ended in an error, not a result:",
                "the prior turn hit an explicitly configured hop horizon; do not restart reconnaissance — continue from the current workspace and make validation, submission, or score retrieval the next action",
                "provider account/configuration blocked: {err}; retry on the selected route after the backoff",
                "baseline worker died; retrying the pinned baseline capture before model work",
                "verify worker died without an acceptance result; retry the check after the backoff, without treating it as a red receipt",
                "the acceptance check FAILED:",
                "inner turn stopped ({reason}); automatic continuation paused — inspect the retained evidence before `/loop resume`",
            ],
        },
        Sub {
            route: PODRACE_STALL,
            name: "podrace-stall",
            signal: "podrace stalled with no comparable objective improvement recorded",
            action: "",
            ideas: "",
            pages: &[
                "no comparable objective improvement recorded yet; progress remains unknown.",
                "Inspect existing evidence against the fixed baseline.",
                "Preserve an unfinished discriminating experiment until its result is available; do not abandon a sustained deep-cut hypothesis merely because this review is due.",
                "Retire only a disproved hypothesis, then choose the next concrete mechanism and smallest available check.",
                "Repeated competitive submissions of unchanged candidates are banned; do not redraw to obtain a new receipt.",
            ],
        },
        Sub {
            route: RECEIPT,
            name: "receipt",
            signal: "a podrace measurement or submission receipt was recorded",
            action: "",
            ideas: "",
            pages: &[
                "measurement/submission execution recorded; a receipt is not comparable objective improvement.",
                "Compare the retained result with the fixed baseline using the available verifier.",
                "Repeated competitive submissions of unchanged candidates are banned; a new receipt ID, timestamp, or note does not authorize a redraw.",
                "Use the comparison to retire the hypothesis or choose the next concrete mechanism.",
            ],
        },
        Sub {
            route: FIRST_CANDIDATE,
            name: "first-candidate",
            signal: "no measured candidate yet (iterations beside the route, a blocked attempt below)",
            action: "",
            ideas: "",
            pages: &[
                "no measured candidate yet after {iterations} iterations.",
                "The last measurement attempt was blocked: {why}.",
                "Take the time the problem needs to understand it; when a candidate is ready, measure it with the benchmark so the loop can record it.",
                "A supervised deep worker can own a long check while you keep working.",
            ],
        },
        Sub {
            route: CURATED,
            name: "curated",
            signal: "the curated state's labels (each sent as its page address, the state beside it)",
            action: "",
            ideas: "",
            pages: &[
                "Problem:",
                "Findings so far ({n}):",
                "Open leads ({n}):",
                "Directions already tried:",
                "none yet",
            ],
        },
        Sub {
            route: BRIEF,
            name: "brief",
            signal: "the project brief's headings and workspace labels (each sent as its page address)",
            action: "",
            ideas: "",
            pages: &[
                "machine:",
                "benchmark:",
                "workspace:",
                "notes in the workspace, newest first:",
                "recent commits:",
                "toolchains:",
                "{n} repositories inside the workspace (their changes do not show in the top-level git diff):",
                "top level:",
                "build and run files:",
                "(mapped the first {WALK_LIMIT} entries, {WALK_DEPTH} levels deep)",
            ],
        },
        Sub {
            route: BRIEF_BENCHMARK,
            name: "brief-benchmark",
            signal: "the project brief's benchmark labels (each sent as its page address)",
            action: "",
            ideas: "",
            pages: &[
                "the same spec is in {n} more copies:",
                "editable:",
                "setup:",
                "run:",
                "a submission must beat the current score by {bips} bips ({percent}%)",
                "score file",
            ],
        },
        Sub {
            route: LOOP_TOOLS,
            name: "loop-tools",
            signal: "a note from the loop's research or campaign tool (one page, its value beside it)",
            action: "",
            ideas: "",
            pages: &[
                "research worker panicked; inspect retained artifacts",
                "could not start research worker; launch record retained:",
                "[Selected experimental approach — task context]",
                "[Optional historical research snippets — evidence to assess]",
                "supply a real verifier in 'verify', or bind /goal cmd; RL needs a measurement",
                "unknown rl_campaign action; use run, status, results, or stop",
                "rl_campaign run needs an active /loop; /rl run remains available to the operator",
                "loop_research needs an active /loop",
                "compare requires a verifier to measure a difference",
                "unknown loop_research action",
            ],
        },
        Sub {
            route: LOOP_TOOL_REFUSALS,
            name: "loop-tool-refusals",
            signal: "loop_research or rl_campaign refused the call: a required field is missing or a value it rejects (one page)",
            action: "correct the named argument and call again",
            ideas: "",
            pages: &[
                "candidate requires idea",
                "run requires the idea you want to try",
                "run_id must be an identifier returned by loop_research",
                "no research run with that id in this workspace",
                "samples must be at least 2 for measured policy comparison",
                "audit requires task",
                "audit requires verify",
                "audit requires independent source",
                "run_id must be a campaign identifier returned by this tool",
                "no retained campaign with that run_id in this workspace",
            ],
        },
    ],
};
