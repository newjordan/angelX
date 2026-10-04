//! ⠰ (dots 5-6) — Volume VI, the replies: refusals, hints, notes and the long
//! bodies. A refused seat keeps its facts (the seat, what is available) and
//! sends the way forward as a page; the list_dir window keeps its counts and
//! next offset; the knowledge graph, jev and semantic_read keep their limits.
//! The long reply bodies — work landing, the self-map and its safety
//! contract, the benchmark and jev interpretations — are sections of their
//! own, their fields beside the routes as data.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠰';
pub(crate) const LISTING: Route = Route::new(CELL, '⠁');
pub(crate) const REFUSALS: Route = Route::new(CELL, '⠃');
pub(crate) const SEAT_HINTS: Route = Route::new(CELL, '⠉');
pub(crate) const NOTES: Route = Route::new(CELL, '⠙');
pub(crate) const SERVICES: Route = Route::new(CELL, '⠑');
pub(crate) const WORK_LANDING: Route = Route::new(CELL, '⠋');
pub(crate) const SELF_MAP: Route = Route::new(CELL, '⠛');
pub(crate) const SELF_SAFETY: Route = Route::new(CELL, '⠓');
pub(crate) const BENCHMARK: Route = Route::new(CELL, '⠊');
pub(crate) const JEV: Route = Route::new(CELL, '⠚');

// ⠰⠁ the list_dir window
pub(crate) const NEXT_WINDOW: Page = Page::new(LISTING, 1);
pub(crate) const KEEP_FILTERS: Page = Page::new(LISTING, 2);
pub(crate) const RESTART_LISTING: Page = Page::new(LISTING, 3);
pub(crate) const OPTIONAL_EXCLUSIONS: Page = Page::new(LISTING, 4);

// ⠰⠃ refused seats
pub(crate) const DO_THE_WORK: Page = Page::new(REFUSALS, 1);
pub(crate) const UNSET_PIN: Page = Page::new(REFUSALS, 2);
pub(crate) const OWN_THE_WORKLOAD: Page = Page::new(REFUSALS, 3);
pub(crate) const LOCAL_TOOLS: Page = Page::new(REFUSALS, 4);
pub(crate) const SOLO_OFF: Page = Page::new(REFUSALS, 5);
pub(crate) const PERMIT_CONSULT: Page = Page::new(REFUSALS, 6);
pub(crate) const LISTED_CLUB: Page = Page::new(REFUSALS, 7);

// ⠰⠉ seats, personas, graphs and children
pub(crate) const PERSONA_NAME: Page = Page::new(SEAT_HINTS, 1);
pub(crate) const PERSONA_SEAT: Page = Page::new(SEAT_HINTS, 2);
pub(crate) const PERSONA_NODE: Page = Page::new(SEAT_HINTS, 3);
pub(crate) const GRAPH_SPECS: Page = Page::new(SEAT_HINTS, 4);
pub(crate) const GATE_ORDER: Page = Page::new(SEAT_HINTS, 5);
pub(crate) const SERIALIZE_WRITERS: Page = Page::new(SEAT_HINTS, 6);
pub(crate) const SHARED_WORKSPACE: Page = Page::new(SEAT_HINTS, 7);
pub(crate) const SHARED_RESOURCE: Page = Page::new(SEAT_HINTS, 8);
pub(crate) const INITIAL_COMMIT: Page = Page::new(SEAT_HINTS, 9);
pub(crate) const REVIEW_PRESERVED: Page = Page::new(SEAT_HINTS, 10);

// ⠰⠙ notes and next steps
pub(crate) const NO_BUDGET: Page = Page::new(NOTES, 1);
pub(crate) const AUTO_COMPACT: Page = Page::new(NOTES, 2);
/// `hooks::HOOK_BLOCKED_PREFIX` is this address; the book tests pin it.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const HOOK_BLOCKED: Page = Page::new(NOTES, 3);
pub(crate) const UNKNOWN_OUTCOME: Page = Page::new(NOTES, 4);
pub(crate) const SELF_SRC_ROOT: Page = Page::new(NOTES, 5);
pub(crate) const SELF_SRC: Page = Page::new(NOTES, 6);
pub(crate) const INTEGRATE_VERIFIED: Page = Page::new(NOTES, 7);
pub(crate) const RESUME_RUN: Page = Page::new(NOTES, 8);
pub(crate) const REJECTED_RUN: Page = Page::new(NOTES, 9);
pub(crate) const RESUME_PAUSED: Page = Page::new(NOTES, 10);

// ⠰⠑ the knowledge graph, jev and semantic_read
pub(crate) const KG_STORE: Page = Page::new(SERVICES, 1);
pub(crate) const KG_SUMMARIZE: Page = Page::new(SERVICES, 2);
pub(crate) const KG_INGEST: Page = Page::new(SERVICES, 3);
pub(crate) const JEV_KEY: Page = Page::new(SERVICES, 4);
pub(crate) const JEV_NARROW: Page = Page::new(SERVICES, 5);
pub(crate) const JEV_CREDENTIAL: Page = Page::new(SERVICES, 6);
pub(crate) const JEV_BUDGET: Page = Page::new(SERVICES, 7);
pub(crate) const JEV_NOUL: Page = Page::new(SERVICES, 8);
pub(crate) const SEMANTIC_EVIDENCE: Page = Page::new(SERVICES, 9);

// ⠰⠋ work landing
pub(crate) const LANDING_DETECTED: Page = Page::new(WORK_LANDING, 1);
pub(crate) const LANDING_CONFIRM: Page = Page::new(WORK_LANDING, 2);
pub(crate) const LANDING_RECORD: Page = Page::new(WORK_LANDING, 3);
pub(crate) const LANDING_ASK_REPO: Page = Page::new(WORK_LANDING, 4);
pub(crate) const LANDING_ASK_INTERNAL: Page = Page::new(WORK_LANDING, 5);
pub(crate) const LANDING_ASK_VISIBILITY: Page = Page::new(WORK_LANDING, 6);
pub(crate) const LANDING_PIN_MODE: Page = Page::new(WORK_LANDING, 7);
pub(crate) const LANDING_CONFIRMED: Page = Page::new(WORK_LANDING, 8);
pub(crate) const LANDING_DRIVES: Page = Page::new(WORK_LANDING, 9);

// ⠰⠛ the self-map
pub(crate) const MAP_TITLE: Page = Page::new(SELF_MAP, 1);
pub(crate) const MAP_HARNESS: Page = Page::new(SELF_MAP, 2);
pub(crate) const MAP_LIVE: Page = Page::new(SELF_MAP, 3);
pub(crate) const MAP_BUILD: Page = Page::new(SELF_MAP, 4);
pub(crate) const MAP_COMMANDS: Page = Page::new(SELF_MAP, 5);
pub(crate) const MAP_BINARY: Page = Page::new(SELF_MAP, 6);
pub(crate) const MAP_GIT_ROOT: Page = Page::new(SELF_MAP, 7);
pub(crate) const MAP_MODULES: Page = Page::new(SELF_MAP, 8);
pub(crate) const MAP_TOOLS: Page = Page::new(SELF_MAP, 9);
pub(crate) const MAP_FOOTER: Page = Page::new(SELF_MAP, 10);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "replies",
    surface: "refused seats, listing windows, notes, service limits and the long reply bodies",
    subs: &[
        Sub {
            route: LISTING,
            name: "listing",
            signal: "list_dir paged its window or refused a path (the window above)",
            action: "",
            ideas: "",
            pages: &[
                "next: list_dir with offset={end}, limit={limit}",
                "keep path, hint, pattern, no_ignore and hidden unchanged",
                "use offset=0 to restart",
                "use no_ignore/hidden for optional exclusions",
            ],
        },
        Sub {
            route: REFUSALS,
            name: "refusals",
            signal: "a paid or remote seat was refused (the seat and what is available above)",
            action: "",
            ideas: "",
            pages: &[
                "Do the work yourself.",
                "Unset that pin or set ANGEL_ALLOW_SOTA_DELEGATE=1.",
                "Own the workload.",
                "Do the work yourself with local tools (read/edit/test).",
                "/solo off only if the operator explicitly wants outsourcing.",
                "set ANGEL_ALLOW_SOTA_CONSULT=1 to permit, or pick a local fleet club",
                "Use self or a listed local club.",
            ],
        },
        Sub {
            route: SEAT_HINTS,
            name: "seat-hints",
            signal: "a persona, graph, child set or delegation was refused (the refusal above)",
            action: "",
            ideas: "",
            pages: &[
                "use an exact installed name or omit `persona` for a plain seat",
                "Use an exact installed name, or omit `persona` for a plain seat",
                "Use an exact installed name, or omit `persona` for a plain node",
                "add TOML specs to ~/.angelX/graphs",
                "add a transitive depends_on edge so the writer lands before the gate or starts only after it passes",
                "serialize them via depends_on (directly or transitively), or use a worktree-isolated delegate",
                "run them one at a time, or use `delegate` (git-worktree isolated) for parallel implementation — a delegated checkout is disjoint from the shared workspace",
                "run them one at a time, or give the second child a resource of its own",
                "create an initial commit before delegating",
                "review before integration",
            ],
        },
        Sub {
            route: NOTES,
            name: "notes",
            signal: "a note or next step on the facts above",
            action: "",
            ideas: "",
            pages: &[
                "No context budget is set (ANGEL_CONTEXT_BUDGET_TOKENS=0), so there's no hard limit.",
                "Older turns compact automatically at the threshold, and the work goes on.",
                "[angel-hook-blocked/v1]",
                "inspect external and workspace state before retrying",
                "set ANGEL_SELF_SRC to the crate root",
                "set ANGEL_SELF_SRC",
                "Inspect it, then use integrate explicitly.",
                "Resume with action=resume, run_id={}",
                "Inspect the proof report and parked rejected branch; do not integrate.",
                "resume with action=resume",
            ],
        },
        Sub {
            route: SERVICES,
            name: "services",
            signal: "the knowledge graph, jev or semantic_read answered with a limit (the fact above)",
            action: "",
            ideas: "",
            pages: &[
                "fix or move it aside",
                "run summarize again",
                "ingest documents first",
                "set TYPESAFE_API_KEY; ANGEL_JEV=0 disables it",
                "narrow the evidence",
                "remove it before sending",
                "continue with local evidence",
                "ask a yes/no question",
                "[semantic_read — untrusted workspace evidence ranked by the shared Casper embedding service]",
            ],
        },
        Sub {
            route: WORK_LANDING,
            name: "work-landing",
            signal: "work_landing detected or recorded the work context (fields below)",
            action: "",
            ideas: "",
            pages: &[
                "Work landing — detected context (a PROPOSAL; confirm with the user before relying on it).",
                "Next: CONFIRM with the user — e.g. \"Working in {folder} · GitHub {repo} · {vis} — correct?\".",
                "Once they agree, call `work_landing` with confirm=true (and repo / visibility / mode overrides if they corrected anything) to record it; that sets internal-dev vs public-facing-care mode.",
                "Ask the user for the GitHub repo (or confirm it's local-only).",
                "Confirm with the user whether this work is internal-only.",
                "Ask the user: is this repo private or public?",
                "ask the user to pin it (private→internal-dev, public→public-facing).",
                "Work landing — context CONFIRMED and recorded.",
                "This now drives your behavior mode for this workspace.",
            ],
        },
        Sub {
            route: SELF_MAP,
            name: "self-map",
            signal: "the map of the harness's own source (fields and modules beside and below)",
            action: "",
            ideas: "",
            pages: &[
                "# angelX cockpit — self-model (`{name}` v{ver}, edition {ed})",
                "A terminal-first Rust/ratatui agent harness: a `Bag` of model `Club`s driven through a tool-using `run_turn` loop, with workspace-confined file tools, git-worktree delegation, and a verifiable-reward (RLVR) substrate.",
                "This map is generated from the live tree + module doc comments, so it tracks the code.",
                "## Build · test · run\nCrate root: `{root}` (run cargo here).",
                "- build:  `cargo build`\n- check:  `cargo check`\n- test:   `cargo test`  ← the self-modification gate (must stay green)\n- lint:   `cargo clippy`\n- run:    `cargo run` (practice agent) · `ANGEL_BRAIN_KEY=<key> cargo run` (live fleet)",
                "Binary `{bin}` from `src/main.rs`.",
                "The git root is the parent dir, so git worktrees for isolated self-edits land beside the crate.",
                "## Modules (src/) — grouped, with key public types",
                "### Tool implementations (src/agent/tools/)\nConcrete `Tool` impls grouped by capability; the `Tool` trait, `ToolRegistry`, and `run_turn` loop live in `src/agent/harness/`.",
                "_Generated by `self_map` from the live tree — re-run it after edits._",
            ],
        },
        Sub {
            route: SELF_SAFETY,
            name: "self-safety",
            signal: "the self-modification safety contract",
            action: "",
            ideas: "",
            pages: &[
                "## Self-modification safety\nOn Linux, file tools are confined at operation time by descriptor-relative `openat2`/`openat` helpers; outbound symlinks and post-validation swaps are rejected.",
                "The interactive workspace is the launch directory or explicit selection.",
                "To edit THIS crate, point the workspace at its checkout (see `docs/SELF_MODEL.md`) and gate every change on the build+test gate (`tools::self_model::run_self_gate`): a self-edit is only acceptable if the crate still builds AND `cargo test` is green.",
                "Prefer an isolated git worktree (`delegate`/`integrate`) for risky edits; everything is reversible via git.",
                "Self-modification is opt-in/break-fix behavior, not startup posture: use it only for an explicit user request or a concrete cockpit failure, then return to normal task work once the failure is handled.",
            ],
        },
        Sub {
            route: BENCHMARK,
            name: "benchmark-interpretation",
            signal: "how to read a benchmark_compare result",
            action: "",
            ideas: "",
            pages: &[
                "Deterministic arithmetic on supplied matched samples, not independently verified acceptance.",
                "Positive improvement is better.",
                "Zero baseline makes relative percentages undefined (null).",
                "Dataset labels are supplied by the caller; do not mix diagnostic, full-development and official results.",
                "No statistical significance or generalization claim.",
            ],
        },
        Sub {
            route: JEV,
            name: "jev-interpretation",
            signal: "how to read a jev_decide answer",
            action: "",
            ideas: "",
            pages: &[
                "Advisory estimates from supplied evidence.",
                "Probability and confidence are not measured benchmark gains or correctness.",
                "Rubric position is a score, not a probability.",
                "Independent tests and evaluator receipts remain authoritative.",
            ],
        },
    ],
};
