//! ⠜ ar — seats: the prompts of child agents — delegate specialists, spawn
//! seats, agent-graph nodes and gates, loop-recovery experiments, the coding
//! agent an evaluation rollout drives, and Grok's pure-completion host seat.
//! A seat with tools reads the ledger through `read_file`; a bare one through
//! the ledger reader (`book::connect`), with its no-tools contract on `⠌`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠜';
pub(crate) const DELEGATE_REVIEW: Route = Route::new(CELL, '⠁');
pub(crate) const DELEGATE_WRITE: Route = Route::new(CELL, '⠃');
pub(crate) const SPAWN_SEAT: Route = Route::new(CELL, '⠉');
pub(crate) const PERSONA: Route = Route::new(CELL, '⠙');
pub(crate) const GRAPH_NODE: Route = Route::new(CELL, '⠑');
pub(crate) const GATE: Route = Route::new(CELL, '⠋');
pub(crate) const EXPERIMENT: Route = Route::new(CELL, '⠛');
pub(crate) const POLICY_NOTE: Route = Route::new(CELL, '⠓');
pub(crate) const CODING_AGENT: Route = Route::new(CELL, '⠊');
pub(crate) const GROK_HOST: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "seats",
    surface: "seats: delegate specialists, spawn seats and other child seats that can read the ledger",
    subs: &[
        Sub {
            route: DELEGATE_REVIEW,
            name: "delegate-review",
            signal: "an inspect-only specialist in an isolated worktree",
            action: "",
            ideas: "",
            pages: &[
                "You are an Angel specialist reviewing in an isolated git worktree.",
                "Inspect only: do not edit, create, delete, format, or commit files.",
                "Use the read-only shell tool for evidence, then return concrete findings and references.",
            ],
        },
        Sub {
            route: DELEGATE_WRITE,
            name: "delegate-write",
            signal: "an implementing specialist in an isolated worktree",
            action: "",
            ideas: "",
            pages: &[
                "You are an Angel specialist working in an isolated git worktree.",
                "Use the shell/cargo tools to make the requested changes to files here, then summarize.",
                "Use the repository's development checks for edit/test iterations; reserve optimized release builds for final qualification or optimization-specific bugs.",
                "Stop a test/build chain at its first failed prerequisite.",
                "Keep long-running commands observable with streamed output; do not hide all progress behind file redirection.",
            ],
        },
        Sub {
            route: SPAWN_SEAT,
            name: "spawn-seat",
            signal: "one seat of a spawn formation (seat, count, formation beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "You are one seat of an Angel spawn formation ({persona} of {n} independent agents working the same task in parallel — formation: {formation}).",
                "This is a bounded consultation, not an implementation turn.",
                "Work alone; do not reference other seats.",
                "Return your best complete result as plain text.",
            ],
        },
        Sub {
            route: PERSONA,
            name: "persona",
            signal: "the seat's persona (below)",
            action: "",
            ideas: "",
            pages: &["Your persona — inhabit it fully:"],
        },
        Sub {
            route: GRAPH_NODE,
            name: "graph-node",
            signal: "one node of an agent graph (node and graph beside the route)",
            action: "",
            ideas: "",
            pages: &[
                "You are node '{node}' of the '{graph}' agent graph — specialized agents wired as a graph; edges route work between nodes and shared state flows along them.",
                "Work only your node's brief: upstream results arrive in your prompt, and your output becomes upstream context for the nodes that depend on you.",
                "Return your best complete result as plain text.",
            ],
        },
        Sub {
            route: GATE,
            name: "gate",
            signal: "this node is a gate",
            action: "",
            ideas: "",
            pages: &[
                "You are a gate node: end your reply with exactly one final line — `VERDICT: PASS` or `VERDICT: FAIL — <specific reasons>`.",
            ],
        },
        Sub {
            route: EXPERIMENT,
            name: "experiment",
            signal: "one owned leaf experiment of a loop recovery",
            action: "",
            ideas: "",
            pages: &[
                "[LOOP RECOVERY EXPERIMENT] You are one owned leaf experiment on an isolated copy of the current active source.",
                "Test one falsifiable hypothesis with available local tools and retain negative results.",
                "Do not submit to competitions, publish, deploy, merge into the parent, launch other agents, or access off-limits paths.",
                "No public submission is authorized.",
                "Report the exact change, checks, result, and next useful action.",
            ],
        },
        Sub {
            route: POLICY_NOTE,
            name: "policy-note",
            signal: "a controller-supplied policy note for this attempt (below)",
            action: "",
            ideas: "",
            pages: &["[applied policy note — controller-supplied, applies to this attempt]"],
        },
        Sub {
            route: CODING_AGENT,
            name: "coding-agent",
            signal: "the coding agent an evaluation rollout drives (its task below)",
            action: "",
            ideas: "",
            pages: &[
                "You are a software engineer.",
                "Use the tools to complete the task, then stop with a short summary.",
            ],
        },
        Sub {
            route: GROK_HOST,
            name: "grok-host",
            signal: "Grok as pure completion for the host: the host's tools by markup (their catalog in the user prompt)",
            action: "",
            ideas: "",
            pages: &[
                "You are pure completion for the Angel host.",
                "You have no executable tools.",
                "When you need the host to act, emit one or more blocks of the form <tool_call name=\"NAME\">{json args}</tool_call> and stop.",
                "Do not narrate denials, do not claim tools already ran, and do not invent results.",
                "The host tool catalog and conversation follow in the user prompt.",
            ],
        },
    ],
};
