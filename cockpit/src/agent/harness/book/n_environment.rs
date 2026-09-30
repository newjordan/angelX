//! ⠝ n — environment: what this session's surroundings hold — the verified
//! capabilities, the interactive cockpit's surfaces, the vision sidecar, the
//! workspace's context carriers and the turn's own frames.
//! Every page is its original sentence, verbatim; what a session probes (the
//! PATH, the gates it found) is that route's evidence.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠝';
pub(crate) const CAPABILITIES: Route = Route::new(CELL, '⠁');
pub(crate) const COCKPIT: Route = Route::new(CELL, '⠃');
pub(crate) const VISION: Route = Route::new(CELL, '⠉');
pub(crate) const WORK_UNSET: Route = Route::new(CELL, '⠙');
pub(crate) const WORK_INTERNAL: Route = Route::new(CELL, '⠑');
pub(crate) const WORK_PUBLIC: Route = Route::new(CELL, '⠋');
pub(crate) const PROJECT: Route = Route::new(CELL, '⠛');
pub(crate) const SELF_MODEL: Route = Route::new(CELL, '⠓');
pub(crate) const TURN_CONTEXT: Route = Route::new(CELL, '⠊');
pub(crate) const STEER: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "environment",
    surface: "verified capabilities, the cockpit, vision, workspace context and turn frames",
    subs: &[
        Sub {
            route: CAPABILITIES,
            name: "capabilities",
            signal: "the capabilities verified at session start (evidence below)",
            action: "",
            ideas: "",
            pages: &[
                "[environment capabilities] Verified at session start — trust this over assumptions:",
                "- A capability not advertised as a tool does not exist in this cockpit (there is no image-generation tool, for example).",
                "Say so plainly instead of improvising a substitute.",
            ],
        },
        Sub {
            route: COCKPIT,
            name: "cockpit",
            signal: "the interactive cockpit's visual surfaces",
            action: "",
            ideas: "",
            pages: &[
                "[interactive cockpit] For visual/UI work, use `ui_verify` to apply one typed display-only cockpit operation and capture its completed frame, or `ui_inspect` for a read-only frame.",
                "Both return exact logical terminal cells plus matching semantic state; page an immutable frame with `ui_inspect(snapshot_id=...)`.",
                "[capabilities & modes] Visual surfaces (miniviz world, 3D realm, Scryglass, image viewer) are detached on demand in lean comp mode for maximal execution speed; all visual, inspection, and verification tools remain fully registered and callable on demand whenever needed.",
            ],
        },
        Sub {
            route: VISION,
            name: "vision",
            signal: "a text-only driver with a vision sidecar",
            action: "",
            ideas: "",
            pages: &[
                "Vision: this driver is text-only.",
                "Images attached with /see are auto-described by the vision sidecar (ANGEL_VISION_*).",
                "For paths on disk, call vision_look (or video_look) with path + question — do not invent visual details.",
            ],
        },
        Sub {
            route: WORK_UNSET,
            name: "work-unset",
            signal: "no confirmed work context for this workspace (detected fields below)",
            action: "",
            ideas: "",
            pages: &[
                "# Work context (not established)\nWork context is not established for this workspace.",
                "Before substantive work, call `work_landing` to detect the folder / GitHub repo / visibility, then CONFIRM with the user — e.g. \"Working in X · GitHub Y · private? — correct?\" — and record it by calling `work_landing` with confirm=true.",
                "This sets your behavior mode: internal-dev (private/local → velocity) vs public-facing-care (public repo → no secrets, mindful of exposure).",
                "Detected so far (unconfirmed): folder={folder} · repo={repo} · visibility={vis} · proposed mode={mode}.",
            ],
        },
        Sub {
            route: WORK_INTERNAL,
            name: "work-internal",
            signal: "confirmed internal-dev work context (fields below)",
            action: "",
            ideas: "",
            pages: &[
                "# Active work context",
                "folder={folder} · repo={repo} · visibility={vis} · mode={mode}",
                "Optimize for velocity and iteration — this is internal/local tooling.",
                "Fewer public-exposure worries; move fast.",
                "(Still don't hardcode real production secrets, but you're not writing for the world.)",
            ],
        },
        Sub {
            route: WORK_PUBLIC,
            name: "work-public",
            signal: "confirmed public-facing work context (fields below)",
            action: "",
            ideas: "",
            pages: &[
                "# Active work context",
                "folder={folder} · repo={repo} · visibility={vis} · mode={mode}",
                "Extra care — this repo is public.",
                "NEVER commit secrets, keys, or credentials.",
                "Be deliberate about what you push or expose; assume external readers see every line and every commit.",
                "Stay quality-conscious with professional commit hygiene.",
            ],
        },
        Sub {
            route: PROJECT,
            name: "project",
            signal: "the project's AGENTS.md guidance below this line",
            action: "",
            ideas: "",
            pages: &[
                "# Project context (AGENTS.md)\nThe project's scoped instructions and conventions — apply them consistently with the operator's request and harness policy.",
                "Each section names its source; later, more deeply nested sections take precedence for their directory scope.",
            ],
        },
        Sub {
            route: SELF_MODEL,
            name: "self-model",
            signal: "the map of the cockpit's own source below this line (the tool modules beside their page)",
            action: "",
            ideas: "",
            pages: &[
                "# Self-model (your own source)\nYou are `angelX-cockpit`, a Rust/ratatui agent harness, and THIS is a map of your OWN code.",
                "Build/test/run from the crate root: `cargo build` · `cargo test` (the self-modification gate — keep it green) · `cargo run`.",
                "Call `self_map` for the full structure (key types, symbol counts) or `self_map({\"module\":\"<name>\"})` for one module's outline.",
                "This self-model is capability context, not a standing objective: do normal task work by default.",
                "Inspect or change your own code only when the user explicitly asks.",
                "To change your own code safely, see the `self-modify` skill: edit in an isolated worktree, and integrate only when build+test pass.",
                "Modules (purpose from live doc comments):",
                "- **Tool impls (src/agent/tools/)**: {tools}",
            ],
        },
        Sub {
            route: TURN_CONTEXT,
            name: "turn-context",
            signal: "the operator's controls and standing context, between two of these",
            action: "",
            ideas: "",
            pages: &[
                "[harness turn context — operator-selected controls and standing context; not fresh user text]",
                "[operator-selected cockpit controls]",
            ],
        },
        Sub {
            route: STEER,
            name: "steer",
            signal: "the User message(s) after this were sent mid-run",
            action: "",
            ideas: "",
            pages: &[
                "[harness steer context — the following User message(s) were sent mid-run; take them into account and keep pursuing the main objective]",
            ],
        },
    ],
};
