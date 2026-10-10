//! ⠌ st — the connected seats: models the harness calls without workspace
//! tools. Each one is offered the ledger reader alone ([`super::connect`]), so
//! its prompt is routes like everyone else's: the bare spawn seat and graph
//! node, the aggregator, code review, the text-only worker's
//! contract, the compaction summarizer, the final-answer advisor, and a
//! mixture stage that answered with a tool call. The task, drafts, code and
//! excerpts ride beside the routes as data.
//!
//! ⡌ (dots 3-4-7) is this chapter's overflow shelf: the Treebeard compactor, a
//! local seat that digests the bulk the lane parks under a handle, and the
//! frame its digest rides under in the root's receipt (or, when it lands after
//! that receipt was sent, under its handle at the tail). The same seat reads a
//! foreground call the harness handed to the background after it ran past its
//! limit, and that read rides the hand-off receipt the same way. The loop
//! watchdog's seat shelves here too: it reads a wedged iteration's facts and
//! names how to free the harness, never how to do the loop's task.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠌';
pub(crate) const BARE_SEAT: Route = Route::new(CELL, '⠁');
pub(crate) const BARE_NODE: Route = Route::new(CELL, '⠃');
pub(crate) const AGGREGATOR: Route = Route::new(CELL, '⠉');
pub(crate) const CODE_REVIEW: Route = Route::new(CELL, '⠙');
pub(crate) const REASONING_CONTRACT: Route = Route::new(CELL, '⠑');
pub(crate) const SUMMARIZER: Route = Route::new(CELL, '⠋');
pub(crate) const SUMMARY_SECTIONS: Route = Route::new(CELL, '⠛');
pub(crate) const MAP_NOTES: Route = Route::new(CELL, '⠓');
pub(crate) const ADVISOR: Route = Route::new(CELL, '⠊');
pub(crate) const TEXT_ONLY: Route = Route::new(CELL, '⠚');

pub(crate) const SHELF_CELL: char = '⡌';
pub(crate) const COMPACTOR: Route = Route::new(SHELF_CELL, '⠁');
pub(crate) const COMPACTOR_DIGEST: Route = Route::new(SHELF_CELL, '⠃');
pub(crate) const COMPACTOR_LATE: Route = Route::new(SHELF_CELL, '⠉');
pub(crate) const HANDOFF: Route = Route::new(SHELF_CELL, '⠙');
pub(crate) const OVERSEER: Route = Route::new(SHELF_CELL, '⠑');
pub(crate) const OVERSEER_READ: Route = Route::new(SHELF_CELL, '⠋');
pub(crate) const WATCHDOG: Route = Route::new(SHELF_CELL, '⠛');
pub(crate) const WATCHDOG_STOP: Route = Route::new(SHELF_CELL, '⠓');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "connected",
    surface: "seats without workspace tools, connected through the ledger reader",
    subs: &[
        Sub {
            route: BARE_SEAT,
            name: "bare-seat",
            signal: "a spawn seat with no workspace tools (the ledger reader only)",
            action: "",
            ideas: "",
            pages: &[
                "You have no tools this run: answer from reasoning alone.",
                "Tool use and workspace mutation are intentionally out of scope for this consult; return the completed answer directly.",
            ],
        },
        Sub {
            route: BARE_NODE,
            name: "bare-node",
            signal: "a graph node with no workspace tools (the ledger reader only)",
            action: "",
            ideas: "",
            pages: &["You have no tools this run: answer from reasoning alone."],
        },
        Sub {
            route: AGGREGATOR,
            name: "aggregator",
            signal: "the aggregator of a spawn formation (task and drafts below)",
            action: "",
            ideas: "",
            pages: &[
                "You are the aggregator of a spawn formation.",
                "Fold the drafts into one best answer: keep every well-supported point, drop contradictions and filler, resolve disagreements explicitly.",
                "Output only the final answer.",
            ],
        },
        Sub {
            route: CODE_REVIEW,
            name: "code-review",
            signal: "an adversarial code review (focus beside the route, code below)",
            action: "",
            ideas: "",
            pages: &[
                "You are an expert adversarial code reviewer focusing on {focus}.",
                "Review the provided code carefully and structure your response with:\n1. **Summary & Verdict**: Clean / Issues Found / Critical Bugs\n2. **Key Findings**: Specific lines, invariant breaks, logic bugs, or security flaws\n3. **Actionable Fixes**: Precise replacement snippets or refactor advice.",
                "Please review the following code ({focus} focus):",
            ],
        },
        Sub {
            route: REASONING_CONTRACT,
            name: "reasoning-contract",
            signal: "the text-only worker's output contract",
            action: "",
            ideas: "",
            pages: &[
                "You are reasoning only.",
                "You have NO repository, NO filesystem, NO shell, and NO network this iteration: you cannot open a file, run a command or benchmark, or fetch a URL.",
                "Therefore you must NOT emit file:, benchmark:, test:, command:, tool: or url: citations — a citation of that kind would be fabricated, and a fabricated citation is worse than no finding at all.",
                "It will be rejected and counted against progress.",
                "Any claim about specific code, concrete file contents, or a measured number is a HYPOTHESIS here, not a finding — no matter how confident you are.",
                "Output exactly this shape and nothing else:\nDIRECTION: <one short line naming the angle>\nFINDINGS:\n- <claim that follows from the problem statement as given> [evidence: premise:<the part of the problem it rests on>]\n- <claim that follows logically from a premise or an earlier finding> [evidence: derivation:<the step>]\nHYPOTHESES:\n- <anything needing code, measurement, or an external source to settle>",
                "Only FINDINGS carrying a premise: or derivation: tag are admitted as progress.",
                "Putting a real uncertainty under HYPOTHESES costs you nothing and is the correct move; dressing one up as a finding is the one thing that fails.",
            ],
        },
        Sub {
            route: SUMMARIZER,
            name: "summarizer",
            signal: "distill the conversation excerpt below into the sections",
            action: "",
            ideas: "",
            pages: &[
                "Distill the earlier portion of an assistant/tool conversation below into dense, durable notes for later reference (these REPLACE the excerpt as background — not instructions).",
                "Use EXACTLY these sections, each introduced by its `## ` header, in this order.",
                "Under each, write terse bullet points; if a section has nothing, write `(none)`.",
                "Do not add other sections or any preamble.",
                "Do not record transient tool errors, retries, crashes, or harness warnings as tasks, open threads, or facts unless the user explicitly asked to investigate or fix them.",
            ],
        },
        Sub {
            route: SUMMARY_SECTIONS,
            name: "summary-sections",
            signal: "the summary's sections, in order",
            action: "",
            ideas: "",
            pages: &[
                "## Task\nthe current goal — what the user is ultimately trying to achieve",
                "## Decisions\nchoices made and the reasoning; approaches considered and rejected",
                "## Files\nfiles created or changed and the purpose of each change",
                "## Facts\ndurable facts established — versions, paths, config values, constraints",
                "## OpenThreads\nunfinished work and next steps for what the user asked for; never transient tool or runtime errors",
                "## Entities\nkey named things — functions, modules, endpoints, commands, people",
            ],
        },
        Sub {
            route: MAP_NOTES,
            name: "map-notes",
            signal: "note the conversation excerpt below densely",
            action: "",
            ideas: "",
            pages: &[
                "Densely note the key decisions, files changed, durable facts, open threads, and named entities in this conversation excerpt.",
                "Terse bullet points, no preamble.",
                "Do not record transient tool errors, retries, crashes, or harness warnings as tasks, open threads, or facts unless the user explicitly asked to investigate or fix them.",
            ],
        },
        Sub {
            route: ADVISOR,
            name: "advisor",
            signal: "the final-answer advisor (the task and the proposed answer below)",
            action: "",
            ideas: "",
            pages: &[
                "You are a terse senior reviewer.",
                "You are shown a TASK and a proposed ANSWER.",
                "Reply with exactly one verdict line and nothing else:\n- `CLEAR` if the answer is sound and complete.\n- `NOTE: <one sentence>` for a real but non-blocking concern.\n- `BLOCK: <one sentence>` if the answer is wrong, unsafe, or misses the task's core requirement.",
                "Judge substance, not style.",
                "Do not rewrite the answer; add nothing after the verdict line.",
                "TASK:",
                "ANSWER:",
                "Your verdict line:",
            ],
        },
        Sub {
            route: TEXT_ONLY,
            name: "text-only",
            signal: "a text-only mixture stage: no workspace tools; after a tool call, the correction",
            action: "",
            ideas: "",
            pages: &[
                "This is an analysis-only stage.",
                "You have no tools here and no tool call will execute — a reply containing tool-call markup is discarded unread.",
                "Answer in prose from what you already know.",
                "Your previous reply was a tool call.",
                "No tools are available in this stage and nothing was executed — a tool call here is discarded.",
                "Answer now in plain prose, from what you already know: no tool calls and no tool-call markup of any kind (`[TOOL_CALLS]`, `<tool_call>`, `<function=…>`, `<SHELL>{…}`).",
            ],
        },
    ],
};

pub(crate) const SHELF: Primary = Primary {
    cell: SHELF_CELL,
    name: "compactor",
    surface: "the Treebeard compactor: a local seat that digests parked bulk so the root carries only what it needs",
    subs: &[
        Sub {
            route: COMPACTOR,
            name: "compactor",
            signal: "digest the parked tool output below for the root (its call beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "The root model sees a handle to the tool output below, not the output itself.",
                "Write the digest it needs to act without opening the handle: what the output is, and the paths, line numbers, names, values and errors that matter.",
                "Quote a line exactly only when its exact text matters.",
                "Terse bullet points, no preamble.",
            ],
        },
        Sub {
            route: COMPACTOR_DIGEST,
            name: "compactor-digest",
            signal: "the local compactor's digest of the parked output above; its handle keeps the full body",
            action: "",
            ideas: "",
            pages: &["read the exact bytes with handle_read when the digest is not enough"],
        },
        Sub {
            route: COMPACTOR_LATE,
            name: "compactor-late",
            signal: "the local compactor's digest of an earlier parked output, named by the handle beside the route",
            action: "",
            ideas: "",
            pages: &["read the exact bytes with handle_read when the digest is not enough"],
        },
        Sub {
            route: HANDOFF,
            name: "handoff",
            signal: "a foreground call ran past its limit and now runs as the background job named above; nothing was stopped",
            action: "",
            ideas: "",
            pages: &["The job keeps running; its output so far and its process tree are above."],
        },
        Sub {
            route: OVERSEER,
            name: "overseer",
            signal: "read the handed-off job below for the root (its facts beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "A foreground tool call ran past its limit and was moved to the background, still running.",
                "From its command, process tree, CPU, state and output, say what it is doing and whether it looks like progress or a stall.",
                "Terse bullet points, no preamble.",
            ],
        },
        Sub {
            route: OVERSEER_READ,
            name: "overseer-read",
            signal: "the local compactor's read of a handed-off job: the receipt above, or the job named beside the route",
            action: "",
            ideas: "",
            pages: &["check the job with proc_status before relying on this read"],
        },
        Sub {
            route: WATCHDOG,
            name: "watchdog",
            signal: "a loop iteration has made no harness progress for a long time; say whether the harness is wedged and how to free it (its facts below the route)",
            action: "",
            ideas: "",
            pages: &[
                "You are the loop watchdog: your only job is to keep the harness moving, not to judge or advance the loop's task.",
                "The facts below say what the iteration is blocked on: the tool call in flight, its process activity, and how long since the model last streamed.",
                "Choose wait when the blocked work is visibly advancing (fresh output, a delegate still making calls), handoff to move a running process to the background so the turn continues, stop_call when the call itself is hung, or restart_turn when the turn is stuck outside any call.",
                "Earlier watchdog actions in this stretch are listed; when one already failed to free the turn, escalate.",
                "Answer with one short reason line, then a last line of exactly `verdict: wait`, `verdict: handoff`, `verdict: stop_call` or `verdict: restart_turn`.",
            ],
        },
        Sub {
            route: WATCHDOG_STOP,
            name: "watchdog-stop",
            signal: "the loop watchdog saw no harness progress on this call for a long time and stopped it; the turn continues",
            action: "",
            ideas: "",
            pages: &[
                "For long work, start it with proc_run and check it with proc_status instead of holding the turn on it.",
            ],
        },
    ],
};
