//! ⠠ d6 — the long run's seats: the models a long run hands a task to beside
//! its iterations. The deep worker's recovery experiment, the RL campaign's
//! prompt optimizer (a connected seat: it reads the ledger through the ledger
//! reader), handoff-RL's forced restart with its sequence directive, and the
//! four roles of a swarm_compile proof graph (delegate seats: they read the
//! ledger through their own `read_file`). Each prompt is its route; its labels
//! are page addresses, and the objective, hypotheses, evidence, attempts,
//! board state, commands and diffs ride beside them as data.
//! Every page is its original sentence, verbatim. Volume V, the long run.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠠';
pub(crate) const RECOVERY: Route = Route::new(CELL, '⠁');
pub(crate) const RECOVERY_CONTRACT: Route = Route::new(CELL, '⠃');
pub(crate) const OPTIMIZER: Route = Route::new(CELL, '⠉');
pub(crate) const HANDOFF: Route = Route::new(CELL, '⠙');
pub(crate) const HANDOFF_STATE: Route = Route::new(CELL, '⠑');
pub(crate) const SEQUENCE: Route = Route::new(CELL, '⠋');
pub(crate) const INVESTIGATOR: Route = Route::new(CELL, '⠛');
pub(crate) const TEST_AUTHOR: Route = Route::new(CELL, '⠓');
pub(crate) const IMPLEMENTER: Route = Route::new(CELL, '⠊');
pub(crate) const REVIEWER: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "long-run",
    surface: "the long run's seats: recovery, optimizer, forced handoff, swarm_compile roles",
    subs: &[
        Sub {
            route: RECOVERY,
            name: "recovery-experiment",
            signal: "a loop recovery experiment's labels (each sent as its page address, its value beside it)",
            action: "",
            ideas: "",
            pages: &[
                "[LOOP RECOVERY EXPERIMENT] Parent objective:",
                "Hypothesis:",
                "Operator steering:",
                "Recent evidence:",
            ],
        },
        Sub {
            route: RECOVERY_CONTRACT,
            name: "recovery-contract",
            signal: "run one discriminating local experiment on the hypothesis above",
            action: "",
            ideas: "",
            pages: &[
                "Run one discriminating local experiment with the available file, shell, and evaluation tools.",
                "Inspect repository instructions and discover its actual benchmark or RL evaluation workflow.",
                "Use a fixed baseline and report comparable measurements, failed checks, and the next decision.",
                "Preserve an unfinished experiment and its logs.",
                "No competitive submissions, redraws, external publishing, model switching, or nested workers.",
                "The parent owns fast wins and any integration.",
                "Return artifact paths and an honest result; no objective credit for prose or process exit alone.",
            ],
        },
        Sub {
            route: OPTIMIZER,
            name: "prompt-optimizer",
            signal: "improve a policy note from a high- and a low-scoring attempt (labelled below)",
            action: "",
            ideas: "",
            pages: &[
                "You optimize system prompts for a coding agent.",
                "TASK the agent must do:",
                "CURRENT policy note:",
                "A HIGH-scoring attempt (measured by the objective's own verifier):",
                "A LOW-scoring attempt:",
                "Write an improved policy note that steers the agent toward what the high-scoring attempt did.",
                "Reply with ONLY the new note.",
                "physical verifier:",
            ],
        },
        Sub {
            route: HANDOFF,
            name: "forced-handoff",
            signal: "handoff-RL wiped the context and restarts the campaign (roll beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "[FORCED HANDOFF — CONTEXT WIPED BY COCKPIT · roll #{roll}]\nThis is a host-enforced context restart (prompt injection procedure).",
                "Prior conversation history has been erased.",
                "Do not renegotiate, summarize the wipe, or ask whether to continue.",
                "Obey the sequence directive.",
                "BEGIN IMMEDIATELY.",
                "Compete.",
                "Place victories on the board.",
                "After the next submission result is in, the cockpit will demand handoff again.\n[/FORCED HANDOFF]",
            ],
        },
        Sub {
            route: HANDOFF_STATE,
            name: "handoff-state",
            signal: "the campaign state a forced handoff carries (each label sent as its page address)",
            action: "",
            ideas: "",
            pages: &[
                "- Winning Baseline: {base} (score: {score})",
                "- Board Status:",
                "- Current Hot-Path Hypothesis:",
                "Campaign task:",
                "latest submission evidence:",
                "isolate hot-path bottlenecks and optimize execution speed/accuracy",
            ],
        },
        Sub {
            route: SEQUENCE,
            name: "sequence-directive",
            signal: "handoff-RL's forced sequence directive",
            action: "",
            ideas: "",
            pages: &[
                "[forced sequence directive]\nI want you to compete for me using this cockpit and its agentic tools/resources at your disposal.",
                "I want you to place victories on the board, remember to check the board before submitting.",
                "- ALWAYS BE IMPROVING: the revolving door never stops.",
                "The current BEST goes up to bat now.",
                "Submit a verified competitive candidate promptly; inspect pending results when useful.",
                "- Once a submission is in play, immediately improve the next best on the newest winning baseline: isolate the next hot-path hypothesis, price the phase, cross-compile locally for register/spill checks, assert zero-fallback correctness, and submit that bat.",
                "Check the exact submission result explicitly; automatic tracking is not connected to ordinary submissions.\n[/forced sequence directive]",
            ],
        },
        Sub {
            route: INVESTIGATOR,
            name: "investigator",
            signal: "swarm_compile's investigator: investigate the goal below, read-only",
            action: "",
            ideas: "",
            pages: &[
                "Read and obey the active repository scope instructions first.",
                "Investigate only; do not edit files.",
                "Goal:",
                "Targeted proof command:",
                "Full acceptance command:",
                "Return a compact evidence packet: relevant files/symbols, likely root cause or design seam, risks, and the smallest implementation boundary.",
                "Cite concrete paths.",
            ],
        },
        Sub {
            route: TEST_AUTHOR,
            name: "test-author",
            signal: "swarm_compile's test author: one red regression test (its marker beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "Read and obey the active repository scope instructions first.",
                "Add the smallest standalone regression test for the goal.",
                "Goal:",
                "Investigator evidence:",
                "You may edit ONLY these test paths/prefixes:",
                "The orchestrator will run:",
                "The test must fail for the missing behavior, include the exact diagnostic marker `{red_marker}` in that intentional failure, and pass once the behavior is correctly implemented.",
                "Do not edit production code, weaken existing assertions, or make unrelated changes.",
                "Run the targeted command once if useful, then summarize the proof you added.",
            ],
        },
        Sub {
            route: IMPLEMENTER,
            name: "implementer",
            signal: "swarm_compile's implementer: turn the protected red test green",
            action: "",
            ideas: "",
            pages: &[
                "Read and obey the active repository scope instructions first.",
                "Implement the requested behavior on top of an immutable failing regression-test commit.",
                "Goal:",
                "Investigator evidence:",
                "Protected test files (do not edit, replace, rename, or delete):",
                "Targeted command:",
                "Full acceptance command:",
                "Make the smallest production change that turns the protected test green.",
                "Preserve existing behavior and finish with a concise summary of changed production files and commands run.",
            ],
        },
        Sub {
            route: REVIEWER,
            name: "reviewer",
            signal: "swarm_compile's reviewer: review the candidate diff below, read-only",
            action: "",
            ideas: "",
            pages: &[
                "Read and obey the active repository scope instructions first.",
                "Review this candidate from its exact branch; do not edit files.",
                "Goal:",
                "Targeted proof command:",
                "Full acceptance command:",
                "Candidate diff from the original base:",
                "Look for correctness gaps, scope violations, weakened tests, unsafe behavior, and needless complexity.",
                "Cite paths and executable objections.",
                "End with exactly one final line: `SWARM_REVIEW: PASS` if no blocking issue remains, otherwise `SWARM_REVIEW: BLOCK`.",
            ],
        },
    ],
};
