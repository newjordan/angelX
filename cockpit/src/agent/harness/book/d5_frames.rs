//! ⠐ (dots 5) — Volume IV, the mixture: its frames. The tasks the judge and
//! the aggregator seats are handed with their data — scoring, revising,
//! holding an answer to its sources, the final synthesis and an intermediate
//! merge, the output-length guidance — the labels on the payload (the drafts,
//! a trimmed context, a transcript made text-only), the formation's preflight,
//! a delegator's consults and the evidence they bring back. A label is sent as
//! its page address with its value beside it; a frame as its route.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠐';
pub(crate) const SCORING: Route = Route::new(CELL, '⠁');
pub(crate) const REVISE: Route = Route::new(CELL, '⠃');
pub(crate) const CITE: Route = Route::new(CELL, '⠉');
pub(crate) const SYNTHESIS: Route = Route::new(CELL, '⠙');
pub(crate) const MERGE: Route = Route::new(CELL, '⠑');
pub(crate) const GUIDANCE: Route = Route::new(CELL, '⠋');
pub(crate) const LABELS: Route = Route::new(CELL, '⠛');
pub(crate) const PREFLIGHT: Route = Route::new(CELL, '⠓');
pub(crate) const CONSULTS: Route = Route::new(CELL, '⠊');
pub(crate) const EVIDENCE: Route = Route::new(CELL, '⠚');

// `⠐⠛`'s grounding headings and `⠐⠚`'s unstated claim, each sent alone as
// its address.
pub(crate) const GROK_SCOUT_HEADING: Page = Page::new(LABELS, 8);
pub(crate) const SEARX_HEADING: Page = Page::new(LABELS, 9);
pub(crate) const UNSTATED_CLAIM: Page = Page::new(EVIDENCE, 4);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "frames",
    surface: "the mixture's frames: stage tasks, payload labels, preflight, consults, evidence",
    subs: &[
        Sub {
            route: SCORING,
            name: "scoring",
            signal: "the judge's scoring task, one score or per dimension (counts beside, responses below)",
            action: "",
            ideas: "",
            pages: &[
                "Score each of the {n} responses below from 0 to 10 for combined correctness, insight, and rigor.",
                "Output one line per response as 'N: score' (e.g. '1: 7'), nothing else.",
                "Score each of the {n} responses below on these dimensions, each 0 to 10: {dims}.",
                "Output one line per response as 'N: a b c d' where a b c d are the scores for {dims} in that exact order (e.g. '1: 8 6 7 5'), nothing else.",
            ],
        },
        Sub {
            route: REVISE,
            name: "revise",
            signal: "revise an answer to fix the verifier's problems (both below)",
            action: "",
            ideas: "",
            pages: &[
                "Revise the answer below to fix these problems, keeping everything already correct.",
                "Output the full corrected answer directly to the user — no preamble, no mention of the revision.",
                "Problems:",
                "Answer:",
            ],
        },
        Sub {
            route: CITE,
            name: "cite",
            signal: "hold an answer's claims to the gathered sources (both below)",
            action: "",
            ideas: "",
            pages: &[
                "Revise the answer below so every nontrivial factual claim is either supported by one of the sources listed here — attribute it inline (e.g. 'per [source]') — or explicitly hedged or removed when the sources don't support it.",
                "Never invent a citation or cite a source not in this list.",
                "Keep everything already correct and output the full answer directly, no preamble, no meta-commentary.",
                "Sources:",
                "Answer:",
            ],
        },
        Sub {
            route: SYNTHESIS,
            name: "synthesis",
            signal: "the final synthesis of the drafts below",
            action: "",
            ideas: "",
            pages: &[
                "Synthesize the single strongest answer to the problem above, drawing on the responses below.",
                "Keep what is correct, discard what is wrong, reconcile conflicts, and cover anything a single response missed.",
                "Answer the user directly and in full — do not mention this synthesis step or the responses.",
                "Use all internal evidence needed.",
                "Deliver a complete answer in the format and level of detail the user requested.",
                "Be direct and non-repetitive; remove repetition before evidence, caveats, code, or required detail.",
                "Do not shorten merely to meet an unstated length.",
            ],
        },
        Sub {
            route: MERGE,
            name: "merge",
            signal: "an intermediate merge of the drafts below",
            action: "",
            ideas: "",
            pages: &[
                "Merge the responses below into a single, stronger set of dense bullet points.",
                "Keep what is correct, drop what is wrong, reconcile conflicts, and add what they missed.",
                "This is intermediate material for a later step, not the final answer — no preamble, no prose.",
                "If the inputs conflict, state both positions; do not resolve that conflict at this stage.",
            ],
        },
        Sub {
            route: GUIDANCE,
            name: "guidance",
            signal: "a target length for the delivered answer (the target beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "Use all internal evidence needed, but shape the delivered answer rather than constraining the panel's reasoning.",
                "Aim for at most about {target} characters unless completeness or an explicit user format requires more; remove repetition before removing evidence, caveats, or required detail.",
            ],
        },
        Sub {
            route: LABELS,
            name: "labels",
            signal: "the labels on a stage's payload, each sent with its value",
            action: "",
            ideas: "",
            pages: &[
                "{n} independent responses:",
                "{total} independent responses represented by {n} item(s):",
                "({weight} of {total} drafts converged here)",
                "[MOA draft truncated before synthesis: {omitted} chars omitted]",
                "[context trimmed: {omitted} chars omitted]",
                "Assistant requested tools:",
                "[tool result for {id}]",
                "### Grok research scout",
                "### SearXNG web search",
            ],
        },
        Sub {
            route: PREFLIGHT,
            name: "preflight",
            signal: "the formation's planning preflight before the first tool action",
            action: "",
            ideas: "",
            pages: &[
                "[FORMATION PREFLIGHT: Produce an independent, concrete action plan and risk review for the coordinator.",
                "This is planning only: no tool is available, nothing has executed, and you must not claim edits, tests, or measurements.]",
                "[formation advisory truncated]",
            ],
        },
        Sub {
            route: CONSULTS,
            name: "consults",
            signal: "a delegator's consults: a peer's second opinion, a phoned specialist",
            action: "",
            ideas: "",
            pages: &[
                "Second opinion requested.",
                "Claim:",
                "Proposed test:",
                "Evaluate the claim (run the test if you can) and reply with a verdict and brief reasoning.",
                "You are being consulted by a peer agent for your expertise.",
                "Answer concisely and decisively.",
                "Context — the claim under consideration:",
                "Question:",
            ],
        },
        Sub {
            route: EVIDENCE,
            name: "evidence",
            signal: "executed test evidence from the delegators (results below; a request with no claim marked by its page)",
            action: "",
            ideas: "",
            pages: &[
                "Executed test evidence — ground your answer in these real results.",
                "A delegator agent proposed each; angel approved and routed it.",
                "Trust passing/failing tests and consulted specialists over the drafts' bare assertions:",
                "(unstated)",
            ],
        },
    ],
};
