//! ⠗ r — what the operator armed: relentless execution (`/relentless`,
//! `ANGEL_RELENTLESS_EXECUTION`) rides the operator controls as `⠗⠁`; the
//! standing goal, solo mode and the magic keywords ride beside it, with the
//! bounds of the goal and memory blocks (`⠗⠓`). The goal block's field labels
//! are pages of `⠗⠃`, each field's value beside its address; the `goal` tool's
//! `show` reply reads the same pages. The reinforcement loop the
//! operator arms has two seats of its own: the grader and the prompt
//! optimizer.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠗';
pub(crate) const ARMED: Route = Route::new(CELL, '⠁');
pub(crate) const GOAL: Route = Route::new(CELL, '⠃');
pub(crate) const SOLO: Route = Route::new(CELL, '⠉');
pub(crate) const ULTRATHINK: Route = Route::new(CELL, '⠙');
pub(crate) const ORCHESTRATE: Route = Route::new(CELL, '⠑');
pub(crate) const WORKFLOWZ: Route = Route::new(CELL, '⠋');
pub(crate) const HANDOFF_RL: Route = Route::new(CELL, '⠛');
pub(crate) const BOUNDS: Route = Route::new(CELL, '⠓');
pub(crate) const GRADER: Route = Route::new(CELL, '⠊');
pub(crate) const OPTIMIZER: Route = Route::new(CELL, '⠚');

// `⠗⠃`'s field labels: each page is sent as its address, the field beside it.
pub(crate) const GOAL_OBJECTIVE: Page = Page::new(GOAL, 2);
pub(crate) const GOAL_CRITERIA: Page = Page::new(GOAL, 3);
pub(crate) const GOAL_CHECK: Page = Page::new(GOAL, 4);
pub(crate) const GOAL_STATE: Page = Page::new(GOAL, 5);
pub(crate) const GOAL_BUDGET: Page = Page::new(GOAL, 6);
pub(crate) const GOAL_BLOCKED: Page = Page::new(GOAL, 7);
pub(crate) const GOAL_NOTES: Page = Page::new(GOAL, 8);
pub(crate) const GOAL_SHOW: Page = Page::new(GOAL, 9);
pub(crate) const GOAL_ROUND: Page = Page::new(GOAL, 10);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "relentless",
    surface: "what the operator armed: relentless execution, the goal, solo, magic keywords, the RL seats",
    subs: &[
        Sub {
            route: ARMED,
            name: "armed",
            signal: "the operator armed relentless execution for this request",
            action: "",
            ideas: "- Every action should benefit the user; ground the output in evidence.\n\
                - Status prose is not progress; deliver a useful final answer, then the mode ends.",
            pages: &[
                "[relentless execution active]",
                "Relentless execution to the details: keep taking concrete tool-backed actions until the user's request is actually advanced; ensure every action benefits the user; produce logical, evidence-grounded output.",
                "Do not stop at status prose.",
                "Deliver a useful final answer, then this mode can turn off.",
            ],
        },
        Sub {
            route: GOAL,
            name: "goal",
            signal: "the operator's standing goal, fenced (each field's value beside its page)",
            action: "",
            ideas: "",
            pages: &[
                "[goal — standing objective; keep every action aligned to it]",
                "objective: {objective}",
                "acceptance criteria:",
                "verifiable check (must pass): {command}",
                "progress state: {state}",
                "round budget: {rounds}/{cap}",
                "blocked ({streak}/{min_rounds}): {reason}",
                "progress so far:",
                "goal ({status}): {text}",
                "progress: round {rounds}/{cap}",
            ],
        },
        Sub {
            route: SOLO,
            name: "solo",
            signal: "the operator armed solo mode",
            action: "",
            ideas: "",
            pages: &[
                "[solo mode active] You own this workload.",
                "Do the work yourself with local tools.",
                "Do not consult_model, code_review, delegate, or spawn to openai, codex, chatgpt, grok, sota-moa, sota-swarm, or any other paid/remote SOTA seat.",
                "Self-test: mutate, run the smallest relevant verifier, read diagnostics, fix from evidence.",
                "Outsourcing reasoning is a failure of this mode.",
                "If blocked, name the blocker — do not call Codex.",
            ],
        },
        Sub {
            route: ULTRATHINK,
            name: "ultrathink",
            signal: "the operator wrote ultrathink",
            action: "",
            ideas: "",
            pages: &[
                "[magic-keywords]\nultrathink: take careful multi-step reasoning; prefer the highest available thinking effort; do not rush to a final answer.",
            ],
        },
        Sub {
            route: ORCHESTRATE,
            name: "orchestrate",
            signal: "the operator wrote orchestrate",
            action: "",
            ideas: "",
            pages: &[
                "orchestrate: fan substantial independent work through parallel subagents (spawn/delegate), verify each phase, and merge only after checks pass.",
            ],
        },
        Sub {
            route: WORKFLOWZ,
            name: "workflowz",
            signal: "the operator wrote workflowz",
            action: "",
            ideas: "",
            pages: &[
                "workflowz: build a deterministic multi-subagent workflow with ordered phases and explicit handoffs; prefer structured yields over free-form prose between workers.",
            ],
        },
        Sub {
            route: HANDOFF_RL,
            name: "handoff-rl",
            signal: "the operator wrote handoff_rl",
            action: "",
            ideas: "",
            pages: &[
                "handoff_rl: compete using cockpit tools/resources; place victories on the board (check board before submitting); poll live candidate score, promote/reset from evidence, isolate next hot-path hypothesis on newest winning baseline, run focused correctness checks, and immediately submit next candidate.",
                "After a submission RESULT is in (score/status), the cockpit DEMANDS handoff: it wipes conversation context and prompt-injects a forced restart starting with 'hit it chewy' — not optional.",
            ],
        },
        Sub {
            route: BOUNDS,
            name: "bounds",
            signal: "the goal or memory block was cut to fit (the count beside the page)",
            action: "",
            ideas: "",
            pages: &[
                "[harness omitted {omitted} goal field(s) outside the bounded context]",
                "[harness omitted {omitted} memory item(s) outside the bounded context]",
            ],
        },
        Sub {
            route: GRADER,
            name: "grader",
            signal: "the reinforcement loop's grader (the task and the response below)",
            action: "",
            ideas: "",
            pages: &[
                "You are a strict grader.",
                "Rate from 0 to 10 how well the RESPONSE accomplishes the TASK.",
                "Reply with ONLY the number.",
            ],
        },
        Sub {
            route: OPTIMIZER,
            name: "optimizer",
            signal: "the reinforcement loop's prompt optimizer (the task, the prompt and two rollouts below)",
            action: "",
            ideas: "",
            pages: &[
                "You optimize system prompts for an AI assistant.",
                "TASK the assistant must do:",
                "CURRENT system prompt:",
                "A HIGH-scoring response:",
                "A LOW-scoring response:",
                "Write an improved system prompt that steers the assistant toward the high-scoring style.",
                "Reply with ONLY the new system prompt.",
            ],
        },
    ],
};
