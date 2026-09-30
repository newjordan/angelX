//! ⠳ ou — the loop's checkpoints: what outranks the next direction (a blocked
//! verifier, an overdue submission, the evidence review), how an iteration may
//! end, and the deep experiment in flight. The facts ride beside the route.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠳';
pub(crate) const BLOCKED: Route = Route::new(CELL, '⠁');
pub(crate) const OVERDUE: Route = Route::new(CELL, '⠃');
pub(crate) const WINDOW: Route = Route::new(CELL, '⠉');
pub(crate) const REVIEW: Route = Route::new(CELL, '⠙');
pub(crate) const SETBACK: Route = Route::new(CELL, '⠑');
pub(crate) const DONE_SELF: Route = Route::new(CELL, '⠋');
pub(crate) const DONE_ACCEPT: Route = Route::new(CELL, '⠛');
pub(crate) const DONE_OPEN: Route = Route::new(CELL, '⠓');
pub(crate) const DEEP: Route = Route::new(CELL, '⠊');
pub(crate) const NOTE: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "checkpoints",
    surface: "loop checkpoints: blocked, overdue, window, review, setback, done, experiments",
    subs: &[
        Sub {
            route: BLOCKED,
            name: "blocked",
            signal: "the verification path is blocked (diagnostic below)",
            action: "",
            ideas: "",
            pages: &[
                "[VERIFICATION PATH BLOCKED — restore it before anything else] {diagnostic}.",
                "Do not propose a new optimization direction.",
                "This iteration's only acceptable outcomes: (1) the official benchmark/verify command runs to completion, or (2) a direct local measurement via the repository's own benchmark script, or (3) an exact, minimal operator action (command + why) if neither is possible.",
                "Notes in the repository are not a blocker; missing inputs that a script in the repository can fetch are not a blocker.",
            ],
        },
        Sub {
            route: OVERDUE,
            name: "overdue",
            signal: "measured candidates, no submission yet (counts beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "[SUBMISSION OVERDUE — {measured} measured candidate(s), 0 submissions, {since} iterations since the first measurement; continue with a concrete submission action]",
                "The board is the instrument.",
                "This iteration's only acceptable outcomes: (1) submit the best measured candidate through the official submit command (a candidate that measures at or ahead of the leader on the same local corpus goes in NOW), or (2) if every measured candidate measures behind the leader, say so in one line (score vs leader, same corpus) and produce and measure a new candidate this iteration, or (3) the exact blocking command and its error for the operator.",
                "Do not open a new research direction, do not re-measure what is already measured, and do not treat local-vs-hidden corpus doubt as a reason to withhold: the board settles it.",
            ],
        },
        Sub {
            route: WINDOW,
            name: "window",
            signal: "the oldest findings and directions are elided (counts beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "[window: the {f_skip} oldest findings and {d_skip} oldest directions are elided; restating them still counts as stale ground, not new]",
            ],
        },
        Sub {
            route: REVIEW,
            name: "review",
            signal: "the 15-iteration evidence review",
            action: "",
            ideas: "",
            pages: &[
                "[EVIDENCE REVIEW CHECKPOINT] This is the mandatory 15-iteration audit.",
                "Reconcile every active claim against primary artifacts, identify contradictions and measured negative results, and state the literal target delta.",
                "Do not launch another costly benchmark/submit/deploy action until the existing evidence is reconciled.",
                "Unsupported novelty is not progress.",
            ],
        },
        Sub {
            route: SETBACK,
            name: "setback",
            signal: "the previous iteration's setback (below)",
            action: "",
            ideas: "",
            pages: &[
                "[previous iteration setback — address the cause below first; do not re-declare done until it is fixed]",
            ],
        },
        Sub {
            route: DONE_SELF,
            name: "done-self-edit",
            signal: "the loop improves the cockpit's own source",
            action: "",
            ideas: "",
            pages: &[
                "You are improving the cockpit's OWN source code in an isolated git worktree (the current workspace root is that worktree's crate).",
                "Use the file and cargo tools to make the change, keep the crate building and its test suite green, and when the goal is fully achieved end your reply with a line containing exactly: LOOP_DONE\nA build+test gate with a pass-count regression guard verifies that claim; only a green gate can be integrated into the live tree, so never delete or disable tests to get there.",
            ],
        },
        Sub {
            route: DONE_ACCEPT,
            name: "done-accept",
            signal: "a verifiable acceptance check is bound",
            action: "",
            ideas: "",
            pages: &[
                "If the goal is fully achieved, run the verifiable check and confirm it passes, then end your reply with a line containing exactly: LOOP_DONE",
            ],
        },
        Sub {
            route: DONE_OPEN,
            name: "done-open",
            signal: "no acceptance check is bound",
            action: "",
            ideas: "",
            pages: &[
                "No verifiable acceptance command is bound.",
                "Do not declare the loop complete on your own; keep surfacing concrete progress, blockers, or the next necessary action.",
                "If the operator has asked you to wrap up or stop, finish what is in flight and end your reply with a line containing exactly: LOOP_DONE",
            ],
        },
        Sub {
            route: DEEP,
            name: "deep-experiment",
            signal: "a deep experiment is in flight (its record below)",
            action: "",
            ideas: "",
            pages: &[
                "[DEEP EXPERIMENT — {status}] Hypothesis: {hypothesis}.",
                "Artifacts: {artifacts}.",
                "Cancelled reply discarded; inspect retained source and logs before resuming the hypothesis.",
                "An isolated worker owns this hypothesis; preserve its unfinished work.",
                "Keep the fast lane moving with distinct validated candidates.",
                "Do not duplicate or replace the active experiment.",
                "On return, inspect its patch and actual evaluation receipts; integrate only after checking against the current parent workspace.",
                "A child result does not establish acceptance or a percentage improvement.",
            ],
        },
        Sub {
            route: NOTE,
            name: "loop-note",
            signal: "a loop note (below)",
            action: "",
            ideas: "",
            pages: &["[loop note — information, not an order]"],
        },
    ],
};
