//! ⠌ st — the connected seats: models the harness calls without workspace
//! tools. Each one is offered the ledger reader alone ([`super::connect`]), so
//! its prompt is routes like everyone else's: the bare spawn seat and graph
//! node, the aggregator, code review, the text-only worker's
//! contract, the compaction summarizer, the final-answer advisor, and a
//! mixture stage that answered with a tool call. The task, drafts, code and
//! excerpts ride beside the routes as data.

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
