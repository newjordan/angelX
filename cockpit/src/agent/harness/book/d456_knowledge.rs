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


const CATALOG: &str = include_str!("../../../../book/d456_knowledge.book");
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
            pages: &super::catalog_pages::<5>(CATALOG, b'1'),
        },
        Sub {
            route: CADDY,
            name: "caddy",
            signal: "the caddy card's headings, in order (its entries under `recipes:` and `hazards:`); its identity line and its doors, each value beside its page",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'2'),
        },
        Sub {
            route: MARKS,
            name: "marks",
            signal: "a mark on the scoped project docs: a doc's middle elided, the block's byte count",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'3'),
        },
        Sub {
            route: VISION,
            name: "vision",
            signal: "the vision sidecar: its describe question (the operator's question beside it), its frame, its absence, the operator's question after the description",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'4'),
        },
        Sub {
            route: KG_EXTRACT,
            name: "kg-extract",
            signal: "extract a typed knowledge graph from the document below (each value beside its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'5'),
        },
        Sub {
            route: KG_RESOLVE,
            name: "kg-resolve",
            signal: "deduplicate one entity type's surface forms (the type and the entities below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'6'),
        },
        Sub {
            route: KG_PROFILE,
            name: "kg-profile",
            signal: "profile one hub entity from its edges (the entity and its facts below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'7'),
        },
        Sub {
            route: KG_QUERY,
            name: "kg-query",
            signal: "answer the question below from the knowledge-graph facts below it",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: BUBBLE_ROUTER,
            name: "bubble-router",
            signal: "pick the tools the task below needs from the candidates below (each value beside its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'9'),
        },
        Sub {
            route: ATLAS_CLERK,
            name: "atlas-clerk",
            signal: "the Living Atlas teacher: draft review candidates from the harvest below (the shape beside its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'0'),
        },
    ],
};
