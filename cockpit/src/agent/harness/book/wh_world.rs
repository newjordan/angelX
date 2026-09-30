//! ⠱ wh — Volume II, the tool library: the web, the literature, repositories,
//! endpoints and the rig fleet. One section per tool; its pages are the
//! sentences that told the model how to use that tool, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route.
//!
//! Each parameter's description is the address of its page; a tool whose
//! pages pass ten continues in a later chapter's section named
//! `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠱';

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "world",
    surface: "the world beyond the workspace",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "web_search",
            signal: "using `web_search`",
            action: "",
            ideas: "",
            pages: &[
                "Use for current information, documentation, error messages, or anything outside the workspace.",
                "Research search origin: {origin}; fetch document URLs returned by web_search with web_fetch.",
                "Do not probe the host with shell.",
                "Research search origin: {origin}; fetch document URLs with web_fetch.",
                "search query",
                "max results (default 5)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "web_fetch",
            signal: "using `web_fetch`",
            action: "",
            ideas: "",
            pages: &[
                "Use to read documentation, APIs, or pages found via web_search.",
                "Research search origin: {origin}; fetch document URLs returned by web_search with web_fetch.",
                "Do not probe the host with shell.",
                "Research search origin: {origin}; fetch document URLs with web_fetch.",
                "http(s) URL to fetch",
                "cap on returned text bytes (default 20000, max 200000)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "science_search",
            signal: "using `science_search`",
            action: "",
            ideas: "",
            pages: &[
                "Use for papers, prior work, citation counts, or a literature review in ML, biology, physics or chemistry.",
                "the research question or topic",
                "papers per source (default 8, max 25)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "repo_search",
            signal: "using `repo_search`",
            action: "",
            ideas: "",
            pages: &[
                "Use when a report or answer should point at real, maintained projects: reference implementations, libraries, tools, or the freshest work in a fast-moving area.",
                "topic or GitHub search query (supports GitHub qualifiers like language:rust, stars:>1000)",
                "best = most-starred (default); latest = most-recently-pushed",
                "repositories to return (default 10, max 25)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "grok_research",
            signal: "using `grok_research`",
            action: "",
            ideas: "",
            pages: &[
                "Prefer over web_search when recency, breaking news, or social/X signal matters — it runs Grok's own agentic web + X search and brings back cited context.",
                "what to research (a question or topic)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "http_request",
            signal: "using `http_request`",
            action: "",
            ideas: "",
            pages: &[
                "Use for JSON APIs — local inference servers, webhooks, REST services; use web_fetch for reading pages.",
                "HTTP method (default GET)",
                "http(s) URL",
                "header name → value",
                "request body (verbatim)",
                "cap on returned body bytes (default 20000, max 200000)",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "fleet_status",
            signal: "using `fleet_status`",
            action: "",
            ideas: "",
            pages: &[
                "Use to find which rigs are reachable before SSHing or probing an endpoint on one.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠓'),
            name: "vast_instances",
            signal: "using `vast_instances`",
            action: "",
            ideas: "",
            pages: &[
                "Recon only — report an idle instance, never destroy or restart one; that is the operator's call.",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠊'),
            name: "llm_probe",
            signal: "using `llm_probe`",
            action: "",
            ideas: "",
            pages: &[
                "Use after starting a server with proc_run to confirm it's ready, or to discover what a fleet box serves.",
                "endpoint base, e.g. http://host:8000 (with or without /v1)",
                "authentication credential if required",
            ],
        },
        Sub {
            route: Route::new(CELL, '⠚'),
            name: "llm_bench",
            signal: "using `llm_bench`",
            action: "",
            ideas: "",
            pages: &[
                "Meant for local/self-hosted endpoints — pointing it at a paid API spends real tokens.",
                "endpoint base, e.g. http://host:8000 (with or without /v1)",
                "model id (default: first id from /v1/models)",
                "fixed prompt (default: a ~40-token instruction)",
                "per run (default 128)",
                "measured runs after warmup (default 3, max 10)",
                "authentication credential if required",
            ],
        },
    ],
};
