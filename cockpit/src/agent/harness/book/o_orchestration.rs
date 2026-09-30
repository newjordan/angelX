//! ⠕ o — orchestration: delegate routes, `delegate`, `spawn` and
//! `swarm_compile`, with a campaign's final alignment gate (`⠕⠊`, its verdict
//! contract `⠕⠚`). Every page is its original sentence, verbatim. The
//! configured routes are the `⠕⠃` / `⠕⠋` evidence.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠕';
pub(crate) const NO_ROUTES: Route = Route::new(CELL, '⠁');
pub(crate) const ROUTES: Route = Route::new(CELL, '⠃');
pub(crate) const DELEGATE: Route = Route::new(CELL, '⠉');
pub(crate) const SPAWN: Route = Route::new(CELL, '⠙');
pub(crate) const SWARM_COMPILE: Route = Route::new(CELL, '⠑');
pub(crate) const ROUTES_COMPACT: Route = Route::new(CELL, '⠋');
pub(crate) const SEAT_SKIPPED: Route = Route::new(CELL, '⠛');
pub(crate) const DELI_CONSULT: Route = Route::new(CELL, '⠓');
pub(crate) const ALIGNMENT: Route = Route::new(CELL, '⠊');
pub(crate) const ALIGNMENT_VERDICT: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "orchestration",
    surface: "delegate routes, delegate, spawn, swarm_compile, seats, consults, the alignment gate",
    subs: &[
        Sub {
            route: NO_ROUTES,
            name: "no-routes",
            signal: "no delegate routes are configured",
            action: "",
            ideas: "",
            pages: &[
                "No delegate routes are configured for this session; do the work yourself.",
                "Be concise.",
            ],
        },
        Sub {
            route: ROUTES,
            name: "routes",
            signal: "the configured delegate routes (evidence below)",
            action: "",
            ideas: "",
            pages: &[
                "Configured delegate routes — use the structured `delegate` tool with `club`, `task`, and optional `mode` args only when that route is appropriate and reachable:",
            ],
        },
        Sub {
            route: DELEGATE,
            name: "delegate",
            signal: "the delegate tool",
            action: "",
            ideas: "",
            pages: &[
                "`delegate` runs that teammate in an isolated git worktree and returns a summary, a branch name, and a diff.",
                "Use `mode=review` or `mode=read_only` for inspection-only reviewers.",
                "Use `mode=write` for implementation, then call `integrate` with the returned branch afterwards (one branch at a time).",
                "Be concise.",
            ],
        },
        Sub {
            route: SPAWN,
            name: "spawn",
            signal: "the spawn tool",
            action: "",
            ideas: "",
            pages: &[
                "`spawn` fans a question across parallel sub-agents mid-turn — copies of yourself (club=self) or the fleet (club=auto) — in a formation (panel / moa / quorum), each seat optionally wearing a persona and a scoped tool grant.",
                "Reach for it when independent perspectives, adversarial review, or breadth beat working alone; it returns a labeled digest, never file changes.",
            ],
        },
        Sub {
            route: SWARM_COMPILE,
            name: "swarm-compile",
            signal: "the swarm_compile tool",
            action: "",
            ideas: "",
            pages: &[
                "`swarm_compile` is the proof-carrying coding path: it freezes a base commit, isolates investigation/test/implementation/review contributions, requires an intentional red regression marker followed by green targeted and full gates, parks the candidate branch, and learns routing only from post-action outcomes.",
                "Prefer direct action when a trustworthy red verifier already exists.",
                "Use `swarm_compile` when a green base needs an independently authored regression, adversarial review, or a durable audit trail, and the task has a concrete test scope plus a green baseline acceptance command.",
                "Integration remains an explicit later action.",
            ],
        },
        Sub {
            route: ROUTES_COMPACT,
            name: "routes-compact",
            signal: "the configured delegate routes, compact core (evidence below)",
            action: "",
            ideas: "",
            pages: &[
                "Delegate routes: {}.",
                "Use structured `delegate` calls; integrate returned implementation branches one at a time.",
            ],
        },
        Sub {
            route: SEAT_SKIPPED,
            name: "seat-skipped",
            signal: "the requested seat is not reachable (live seats above)",
            action: "",
            ideas: "",
            pages: &["Do the work yourself; do not retry {wanted}."],
        },
        Sub {
            route: DELI_CONSULT,
            name: "deli-consult",
            signal: "a deli-method consult",
            action: "",
            ideas: "",
            pages: &["[method=deli; proposals require actual checks]"],
        },
        Sub {
            route: ALIGNMENT,
            name: "alignment-gate",
            signal: "the campaign's final alignment gate for the parked candidate (contract below)",
            action: "",
            ideas: "",
            pages: &[
                "Read and obey the repository scope instructions.",
                "Review only; never edit, create, delete, format, commit, or change refs.",
                "You are the final alignment gate for the exact parked candidate below.",
                "Contract digest:",
                "Candidate OID:",
                "Target criteria:",
                "Authorized technical proofs:",
            ],
        },
        Sub {
            route: ALIGNMENT_VERDICT,
            name: "alignment-verdict",
            signal: "the alignment gate's verdict contract",
            action: "",
            ideas: "",
            pages: &[
                "Inspect the candidate and decide whether it actually satisfies every criterion without scope regressions or proof gaming.",
                "Return exactly one raw JSON object (no markdown fence or surrounding prose) with keys: schema, contract_digest, candidate_oid, criteria, cited_proof_sha256, summary.",
                "`schema` must be `campaign-review/v1`.",
                "Each criteria item must contain criterion_id, verdict (`pass` or `block`), and non-empty concrete citations.",
                "Cite every listed criterion and every proof SHA exactly once.",
                "Then end with exactly one final line `CAMPAIGN_REVIEW: PASS` only if every criterion passes, otherwise `CAMPAIGN_REVIEW: BLOCK`.",
            ],
        },
    ],
};
