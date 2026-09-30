//! ⠸ (dots 456) — Volume VII, the seats and the preamble: the knowledge. The
//! frames of the pinned preamble — the repo dossier, the caddy card's headings,
//! the marks on the scoped project docs — and the seats that work the
//! knowledge stores: the vision sidecar, the knowledge graph's four calls, the
//! tool-bubble router and the Living Atlas clerk. Every seat is connected
//! (`book::connect`), so it can read its route; a sentence that carries a
//! value — an allowed vocabulary, a reply shape, a count — is sent as its page
//! address with the value beside it, so the value stays inline.
//!
//! The dossier and the caddy card ride inside the `⠎⠃` evidence fence, where
//! recalled braille is broken with `·`; a line that opens with addresses on
//! the store's own section is the harness's, and stays readable
//! ([`store_route`]). The facts inside stay data.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠸';
pub(crate) const DOSSIER: Route = Route::new(CELL, '⠁');
pub(crate) const CADDY: Route = Route::new(CELL, '⠃');
pub(crate) const MARKS: Route = Route::new(CELL, '⠉');
pub(crate) const VISION: Route = Route::new(CELL, '⠙');
pub(crate) const KG_EXTRACT: Route = Route::new(CELL, '⠑');
pub(crate) const KG_RESOLVE: Route = Route::new(CELL, '⠋');
pub(crate) const KG_PROFILE: Route = Route::new(CELL, '⠛');
pub(crate) const KG_QUERY: Route = Route::new(CELL, '⠓');
pub(crate) const BUBBLE_ROUTER: Route = Route::new(CELL, '⠊');
pub(crate) const ATLAS_CLERK: Route = Route::new(CELL, '⠚');

// `⠸⠁`'s fact pages and `⠸⠃`'s identity and doors: each is sent as its
// address, the values beside it.
pub(crate) const DOSSIER_TRAP: Page = Page::new(DOSSIER, 4);
pub(crate) const DOSSIER_THREAD: Page = Page::new(DOSSIER, 5);
pub(crate) const CADDY_IDENTITY: Page = Page::new(CADDY, 4);
pub(crate) const CADDY_RECIPES_DEGRADED: Page = Page::new(CADDY, 5);
pub(crate) const CADDY_HAZARDS_DEGRADED: Page = Page::new(CADDY, 6);
pub(crate) const CADDY_DOORS: Page = Page::new(CADDY, 7);

/// The section whose route lines a harness-rendered evidence store opens with:
/// the dossier's frame, the caddy card's headings. Recalled text never renders
/// as a whole line of braille in these stores.
pub(crate) fn store_route(store: &str) -> Option<Route> {
    match store {
        "dossier" => Some(DOSSIER),
        "caddy" => Some(CADDY),
        _ => None,
    }
}

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "knowledge",
    surface: "the pinned preamble's frames, and the seats that work the knowledge stores",
    subs: &[
        Sub {
            route: DOSSIER,
            name: "dossier",
            signal: "the repo dossier opens and closes on this route (its facts inside; a trap, the last session and a withheld count each beside its page)",
            action: "",
            ideas: "",
            pages: &[
                "[repo dossier — what angel has verified about this workspace]",
                "[/dossier]",
                "({withheld} fact(s) below {min_belief} belief withheld pending re-verification)",
                "trap: `{text}` fails here ({evidence})",
                "last session here: {age} ago, ended with {stop} ({driver})",
            ],
        },
        Sub {
            route: CADDY,
            name: "caddy",
            signal: "the caddy card's headings, in order (its entries under `recipes:` and `hazards:`); its identity line and its doors, each value beside its page",
            action: "",
            ideas: "",
            pages: &[
                "recipes (verified on this workspace):",
                "recipes (historical; source unbound; rerun before relying):",
                "hazards (what failed here and why):",
                "[caddy · {repo} · HEAD {head} · {dirty} dirty · {langs} · host: {host}]",
                "recipes degraded",
                "hazards degraded",
                "doors: skills {skills} · dossier rituals: {rituals}",
            ],
        },
        Sub {
            route: MARKS,
            name: "marks",
            signal: "a mark on the scoped project docs: a doc's middle elided, the block's byte count",
            action: "",
            ideas: "",
            pages: &[
                "[scoped project doc truncated: middle elided]",
                "<!-- angel-project-doc-bytes: {n} -->",
            ],
        },
        Sub {
            route: VISION,
            name: "vision",
            signal: "the vision sidecar: its describe question (the operator's question beside it), its frame, its absence, the operator's question after the description",
            action: "",
            ideas: "",
            pages: &[
                "Describe this image in detail.",
                "Include any visible text (OCR), layout, objects, and anything needed to answer a follow-up question about it.",
                "The operator's question about this image:",
                "Describe the image carefully (OCR any text) so a text-only model can answer that question without seeing the pixels.",
                "[vision sidecar · {backend} · {n} image(s)]",
                "[vision sidecar unavailable — image attachments dropped; configure ANGEL_VISION_URL + ANGEL_VISION_MODEL or use vision_look once a backend is up]",
                "Operator question: {question}",
            ],
        },
        Sub {
            route: KG_EXTRACT,
            name: "kg-extract",
            signal: "extract a typed knowledge graph from the document below (each value beside its page)",
            action: "",
            ideas: "",
            pages: &[
                "You extract a typed knowledge graph from one document.",
                "Entity types — the ONLY allowed values for \"type\": PERSON, ORGANIZATION, LOCATION, EVENT, ARTIFACT.",
                "Rules:\n- Extract only entities central to what the document is about; skip incidental mentions.",
                "- For each entity write a one-sentence description grounded in THIS document (it is used later to disambiguate entities with similar names).",
                "- Predicates are short verb phrases: \"commanded\", \"launched from\", \"part of\".",
                "- Every relation must connect two entities you extracted.",
                "Reply with STRICT JSON only — no prose, no code fence — matching exactly:\n{\"entities\":[{\"name\":\"…\",\"type\":\"PERSON\",\"description\":\"…\"}],\n \"relations\":[{\"source\":\"…\",\"predicate\":\"…\",\"target\":\"…\"}]}",
                "Document (source id: {source_doc}):",
                "(The document was truncated to fit; extract from what is shown.)",
            ],
        },
        Sub {
            route: KG_RESOLVE,
            name: "kg-resolve",
            signal: "deduplicate one entity type's surface forms (the type and the entities below)",
            action: "",
            ideas: "",
            pages: &[
                "You deduplicate entities in a knowledge graph.",
                "Below are {type} entities, one per line as `name — description`.",
                "Cluster entries that refer to the SAME real-world {type}.",
                "Use the descriptions — do NOT merge entities that merely share a name (\"Armstrong — walked on the Moon\" and \"Armstrong — jazz trumpeter\" stay separate), and DO merge different surface forms of one thing (\"Edwin Aldrin\" and \"Buzz Aldrin\").",
                "The canonical name is the most complete, unambiguous form and MUST be one of the listed names.",
                "Only output clusters with 2+ members; singletons are implied.",
                "Reply with STRICT JSON only — no prose, no code fence:\n{\"clusters\":[{\"canonical\":\"…\",\"aliases\":[\"…\"]}]}",
            ],
        },
        Sub {
            route: KG_PROFILE,
            name: "kg-profile",
            signal: "profile one hub entity from its edges (the entity and its facts below)",
            action: "",
            ideas: "",
            pages: &[
                "Write a profile for one entity in a knowledge graph, grounded ONLY in the facts below.",
                "Do not add outside knowledge; if the facts do not support a claim, leave it out.",
                "Reply with STRICT JSON only — no prose, no code fence:\n{\"summary\":\"2-3 sentences\",\"key_facts\":[\"…\"],\"time_range\":{\"start\":\"YYYY or YYYY-MM or unknown\",\"end\":\"YYYY or YYYY-MM or ongoing or unknown\"}}",
            ],
        },
        Sub {
            route: KG_QUERY,
            name: "kg-query",
            signal: "answer the question below from the knowledge-graph facts below it",
            action: "",
            ideas: "",
            pages: &[
                "Answer the question using ONLY the knowledge-graph facts below.",
                "Every claim must cite at least one edge, written as (source —predicate→ target [doc]).",
                "If the graph does not contain the answer, say exactly what is missing — do not guess.",
            ],
        },
        Sub {
            route: BUBBLE_ROUTER,
            name: "bubble-router",
            signal: "pick the tools the task below needs from the candidates below (each value beside its page)",
            action: "",
            ideas: "",
            pages: &[
                "Select 0 to {max} tools that the coding agent is likely to need for the current task.",
                "Return ONLY JSON in this exact shape: {\"tools\":[\"tool_name\"]}.",
                "Use only names from the candidate list.",
                "Prefer fewer tools; the base read/edit/shell/search tools are already present.",
            ],
        },
        Sub {
            route: ATLAS_CLERK,
            name: "atlas-clerk",
            signal: "the Living Atlas teacher: draft review candidates from the harvest below (the shape beside its page)",
            action: "",
            ideas: "",
            pages: &[
                "You are the Living Atlas teacher.",
                "Draft zero to three review candidates from the bounded harvest below.",
                "Output only strict JSON matching {\"schema\":\"angel-atlas-clerk/v1\",\"action\":\"propose|merge|abstain\",\"candidates\":[{\"kind\":\"fact|decision|procedure|note|preference|open_thread|entity|artifact\",\"content\":\"...\",\"confidence\":0.0,\"sources\":[{\"id\":\"...\",\"kind\":\"...\",\"digest\":\"...\",\"excerpt\":null,\"independent\":true,\"influenced_by\":null}],\"merge_into\":null}]}.",
                "Never activate, accept, share, or train on a claim.",
                "Copy source ids and digests exactly from receipt_sources; independence and influence are owned by the harness.",
                "A merge is a proposed revision linked to merge_into, never an automatic edit.",
                "Abstain when bound independent receipts are absent or evidence is insufficient.",
            ],
        },
    ],
};
