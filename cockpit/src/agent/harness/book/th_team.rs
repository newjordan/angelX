//! ⠹ th — Volume II, the tool library: delegates, consults, formations and
//! graphs, and the harness tools that batch, park and find. One section per
//! tool; its pages are the sentences that told the model how to use that tool,
//! verbatim, moved off the schema. The schema keeps what the tool does and ends
//! with the section's route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠹';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "team",
    surface: "the team and the harness's own tools",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "delegate",
            signal: "using `delegate`",
            action: "",
            ideas: "",
            pages: &[
                "Use mode=review/read_only for inspection-only reviewers; use mode=write for implementation, then call `integrate` with that branch to apply it.",
                "specialist club name",
                "what the specialist should do",
                "write implements changes; review/read_only inspects with workspace writes blocked",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "swarm_compile",
            signal: "using `swarm_compile`",
            action: "",
            ideas: "",
            pages: &[
                "Use direct action instead when a trustworthy red verifier already exists.",
                "required for resume/status",
                "coding outcome to produce",
                "optional stable class used by learned routing",
                "must fail on the immutable test-only branch with the supplied run marker and pass on the candidate",
                "full acceptance command; must pass on base and candidate",
                "optional candidate-only lint/build/typecheck commands (max 4)",
                "allowed test-file paths/prefixes; implementation may not edit files the test author changes",
                "auto, self, or one explicit route for all roles",
                "optional per-role route overrides for investigator/test_author/implementer/reviewer",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "consult_model",
            signal: "using `consult_model`",
            action: "",
            ideas: "",
            pages: &[
                "method=deli optionally runs fresh-context Deli deliberation and returns its findings to this turn; choose rounds when useful, then continue ordinary work.",
                "Deli reasons over the supplied material; use normal tools or RL campaigns to measure its proposals.",
                "Prefer local seats; paid SOTA needs operator allow.",
                "club or model name (default auto=self).",
                "the task, code snippet, or question to consult on",
                "optional role instructions for the consulted model",
                "direct (default): one consultation; deli: iterative deliberation, then return to this task",
                "Deli rounds for this call; defaults to the operator's ANGEL_DELI_ROUNDS (6)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "agent_graph",
            signal: "using `agent_graph`",
            action: "",
            ideas: "",
            pages: &[
                "Use for work that genuinely splits into specialties with handoffs; for ad-hoc parallel seats use `spawn`, and for most tasks just do the work yourself.",
                "graph name from the installed catalog",
                "the task the graph works",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "spawn",
            signal: "using `spawn`",
            action: "",
            ideas: "",
            pages: &[
                "For direct single-call model questions use `consult_model` or `code_review`; for git-isolated implementation use `delegate`.",
                "seat tool grant; code requires n=1 (use delegate for parallel writes)",
                "the task every seat works (or use tasks[])",
                "one task per seat (sets n)",
                "default: panel when n>1, else solo",
                "seat count (default 3, capped by ANGEL_SPAWN_MAX)",
                "optional exact persona name or list cycling across seats; installed names: {}; omit this field for a plain seat",
                "self/auto = your own model replicated (default, self-same panel) | smart = the designated escalation seat (ANGEL_SOTA_SMART_CLUB, default luna) | fleet = spread across reachable fleet clubs (opt-in) | an explicit club label",
                "optional formation deadline in seconds; default 0 (unbounded)",
                "K for quorum formation (default ceil(n/2))",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "code_mode",
            signal: "using `code_mode`",
            action: "",
            ideas: "",
            pages: &[
                "Handle intermediates with `handle_put(body, {producer?, identity?})` → opaque `hnd_…` and `handle_get(id, {offset?, max_bytes?})` for a capped slice — keep bulk out of the script return value.",
                "Use handle_put/handle_get for bulk.",
                "`return` your final value (objects are JSON-stringified).",
                "Use `recipe:'repo_recon', query:'the task'` for the fixed, read-only task-conditioned repository map, or fan out custom inspection.",
                "Example: `const r = batch(files.map(f => ({tool:'outline', args:{path:f}}))); return r.filter(x => !x.ok).length + ' failing inspections';`",
                "`return` the result.",
                "JavaScript to execute.",
                "Call bound tools as functions; `return` the final value.",
                "Trusted built-in orchestration recipe.",
                "Mutually exclusive with `script`.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "handle_read",
            signal: "using `handle_read`",
            action: "",
            ideas: "",
            pages: &[
                "Prefer strategy over bulk: only call when the receipt is insufficient.",
                "Opaque handle id from a prior receipt (hnd_…).",
                "Byte offset into the stored body.",
                "Requested slice size; further capped by store policy.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "tool_search",
            signal: "using `tool_search`",
            action: "",
            ideas: "",
            pages: &[
                "Use this when you need a capability not in your base tool list.",
                "Returns matching tool names + summaries; then call the tool directly by name.",
                "capability keywords",
                "max results (default 5)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "skill",
            signal: "using `skill`",
            action: "",
            ideas: "",
            pages: &[
                "Load a skill's full instructions by name before doing that kind of task.",
                "skill name from the catalog",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "jev_decide",
            signal: "using `jev_decide`",
            action: "",
            ideas: "",
            pages: &[
                "Batch independent questions about the same state.",
                "Never use as a verifier or as measured benchmark improvement; use benchmark_compare for measured percentages.",
                "Sends only supplied state/questions to TypeSafe; omit secrets.",
                "Include dataset, source and uncertainty.",
                "No credentials.",
                "Concise evidence, code excerpts or measurement records.",
                "Explicit question; the model does not see the question id.",
                "For choice: distinct candidate descriptions.",
                "For score: ordered rubric levels from 0 upwards.",
                "Omit for noul.",
            ],
        },
    ],
};
