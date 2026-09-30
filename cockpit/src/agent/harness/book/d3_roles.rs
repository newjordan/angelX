//! ⠄ (dots 3) — Volume IV, the mixture: its roles. The brief each stage of a
//! mixture-of-agents turn is handed — the proposer, the aggregator, the
//! classifier, the critic, the judge, the chooser, the verifier — and the
//! folds a stage can carry: the hedge ladder, context checkpointing, the
//! delegator's test request. Every stage is connected (`book::connect`); the
//! problem, the drafts and the counts ride beside the routes as data.

use super::{DIGITS, Primary, Route, Sub};

pub(crate) const CELL: char = '⠄';
pub(crate) const PROPOSER: Route = Route::new(CELL, '⠁');
pub(crate) const AGGREGATOR: Route = Route::new(CELL, '⠃');
pub(crate) const CLASSIFIER: Route = Route::new(CELL, '⠉');
pub(crate) const CRITIC: Route = Route::new(CELL, '⠙');
pub(crate) const JUDGE: Route = Route::new(CELL, '⠑');
pub(crate) const CHOOSER: Route = Route::new(CELL, '⠋');
pub(crate) const VERIFIER: Route = Route::new(CELL, '⠛');
pub(crate) const HEDGE: Route = Route::new(CELL, '⠓');
pub(crate) const CHECKPOINT: Route = Route::new(CELL, '⠊');
pub(crate) const DELEGATOR: Route = Route::new(CELL, '⠚');

/// Page addresses on one route, in order: the route's cells, each followed by
/// a page digit (1-based) — `pages(SYNTHESIS, 1..=3)` is `⠐⠙⠁⠐⠙⠃⠐⠙⠉`. The
/// mixture sends a label, or a variant of a section, as its pages.
pub(crate) fn pages(route: Route, numbers: impl IntoIterator<Item = usize>) -> String {
    numbers
        .into_iter()
        .map(|n| format!("{}{}", route.cells(), DIGITS[n - 1]))
        .collect()
}

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "roles",
    surface: "the mixture's roles: each stage's brief, and the folds a stage carries",
    subs: &[
        Sub {
            route: PROPOSER,
            name: "proposer",
            signal: "a proposer of the mixture (its angle and wave frame beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "Analyze the user's problem from your assigned angle.",
                "Be dense and concise: terse bullet points and decisive claims, not polished prose.",
                "Your output is raw material another agent will synthesize, so prioritize distinct, non-obvious insights over completeness or readability.",
                "No preamble, no conclusion.",
                "This stage is TEXT ONLY: no tool calls, and no tool-call markup of any kind (`<tool_call>`, `<function=…>`, `<SHELL>{…}`).",
                "The surrounding system prompt documents a tool protocol for the driver, not for you — imitating it here discards your draft.",
                "Write the analysis itself, from what you already know.",
            ],
        },
        Sub {
            route: AGGREGATOR,
            name: "aggregator",
            signal: "an aggregator of the mixture (its task and the drafts below)",
            action: "",
            ideas: "",
            pages: &[
                "You are an aggregator in a mixture-of-agents.",
                "You will be shown several independent expert responses to the user's problem, each from a different angle.",
                "Produce a single response stronger than any one of them: integrate the best reasoning, reconcile contradictions, keep what is correct, discard what is wrong, and cover angles a single response missed.",
            ],
        },
        Sub {
            route: CLASSIFIER,
            name: "classifier",
            signal: "the adaptive gate: classify the request below",
            action: "",
            ideas: "",
            pages: &[
                "Classify the user's request.",
                "Answer with exactly one word: OPEN if it is open-ended, ambiguous, exploratory, or benefits from being attacked from multiple angles; TIGHT if it is a narrow, well-specified directive with one clear right answer or action.",
                "One word only.",
            ],
        },
        Sub {
            route: CRITIC,
            name: "critic",
            signal: "the critic between layers (draft count beside the route, drafts below)",
            action: "",
            ideas: "",
            pages: &[
                "You are a rigorous critic inside a mixture-of-agents.",
                "Your job is to find what is weak so the next revision is stronger.",
                "Be specific, skeptical, and brief.",
                "Critique these {n} drafts answering the problem above.",
                "List the most important weaknesses, errors, contradictions, and unexplored angles across them — the specific things the next revision must fix or add.",
                "Terse bullet points only.",
            ],
        },
        Sub {
            route: JUDGE,
            name: "judge",
            signal: "a judge scoring candidate responses (its scoring task below)",
            action: "",
            ideas: "",
            pages: &[
                "You are an impartial, calibrated judge scoring candidate responses.",
                "Be harsh; reserve high scores for genuinely strong answers.",
                "Follow the output format exactly.",
            ],
        },
        Sub {
            route: CHOOSER,
            name: "chooser",
            signal: "the chooser of the final candidates (count beside the route, candidates below)",
            action: "",
            ideas: "",
            pages: &[
                "You select the single best answer from a set of candidates.",
                "Output only the number of the best one.",
                "Below are {n} candidate final answers to the problem above.",
                "Reply with ONLY the number (1-{n}) of the single best — most correct, complete, and clear — answer.",
            ],
        },
        Sub {
            route: VERIFIER,
            name: "verifier",
            signal: "the adversarial verifier of the final answer (the answer below)",
            action: "",
            ideas: "",
            pages: &[
                "You are an adversarial verifier.",
                "Find concrete errors, unsupported claims, logical gaps, or missing considerations in the answer under review.",
                "Check the answer below against the problem above for concrete errors, unsupported claims, logical gaps, or missing considerations.",
                "If it is genuinely solid, reply with exactly OK.",
                "Otherwise list the specific problems to fix, terse.",
                "Answer:",
            ],
        },
        Sub {
            route: HEDGE,
            name: "hedge",
            signal: "the hedge ladder, folded into every stage's base voice",
            action: "",
            ideas: "",
            pages: &[
                "Calibrate every claim's strength to its evidence: write \"demonstrates\"/\"shows\" only for what is established, \"suggests\"/\"indicates\" for supported-but-partial, \"may\"/\"might\" for plausible, and \"hypothesize\" for speculation.",
                "Do not overstate.",
                "Flag what is uncertain or unverified rather than asserting it.",
            ],
        },
        Sub {
            route: CHECKPOINT,
            name: "checkpoint",
            signal: "context checkpointing for a stage (the loop cadence beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "Context checkpointing: assume this MoA thread may be compacted after every {every} loop(s).",
                "Keep each stage checkpoint-ready: include durable decisions, key evidence or source URLs, relevant paths and commands, open risks, and the next action.",
                "Do not rely on raw earlier context surviving compaction.",
            ],
        },
        Sub {
            route: DELEGATOR,
            name: "delegator",
            signal: "a delegator may request one verifying test or specialist consult",
            action: "",
            ideas: "",
            pages: &[
                "If one key claim can be settled by a quick test or by consulting a specialist model, you MAY request exactly one.",
                "Append a single fenced block:\n```swarm-test\nwhere: local            # local | remote:<host> | peer:<agent> | phone:<model>\ncmd: <a shell/cargo command — or, for phone, the question to ask>\nclaim: <the claim this checks>\nwhy: <why here>\n```",
                "phone targets are model specialists: phone:deepseek (the default — a strong external SOTA second opinion), phone:code, phone:reason, phone:fast.",
                "Use a sandboxed test for empirical/ runtime facts; phone a specialist for hard math/code/reasoning.",
                "Only if it genuinely helps; no destructive commands.",
            ],
        },
    ],
};
