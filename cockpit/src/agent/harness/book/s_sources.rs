//! ⠎ s — sources: the research contract, and the fences around every body of
//! recalled or carried evidence. A fence opens and closes on its own route;
//! the words that frame it are the pages.
//!
//! A research turn (selected by the skill router or a declared research
//! origin) opens with `⠎⠁`. The turn ends on the model's own answer like any
//! other; `turn/research.rs` renders citations and a missing-evidence
//! disclosure from it.

use super::{Primary, Raise, Route, Sub};

const CATALOG: &str = include_str!("../../../../book/s_sources.book");

pub(crate) const CELL: char = '⠎';
pub(crate) const CONTRACT: Route = Route::new(CELL, '⠁');
pub(crate) const RECALLED: Route = Route::new(CELL, '⠃');
pub(crate) const BROKER: Route = Route::new(CELL, '⠉');
pub(crate) const ATLAS_LENS: Route = Route::new(CELL, '⠙');
pub(crate) const MEMORY: Route = Route::new(CELL, '⠑');
pub(crate) const CONTINUAL: Route = Route::new(CELL, '⠋');
pub(crate) const RECALL_NOTES: Route = Route::new(CELL, '⠛');
pub(crate) const CAMPAIGN_LENS: Route = Route::new(CELL, '⠓');
pub(crate) const HANDOFF: Route = Route::new(CELL, '⠊');
pub(crate) const HANDOFF_STALE: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "sources",
    surface: "the research contract, and the fences around recalled evidence",
    subs: &[
        Sub {
            pages: &[],
            route: CONTRACT,
            name: "contract",
            signal: "this is a research turn (declared origin below, if any)",
            action: "answer with a citation for each supported claim, or decline citing nothing",
            ideas: "- Search with `web_search`, fetch documents with `web_fetch`; do not probe the \
                host with shell.\n\
                - If evidence is missing or insufficient, say so and cite nothing: no citations, \
                background links, document identifiers or sources appendix.\n\
                - Answer as soon as the requested claims are supported; batch independent reads.\n\
                - Source tracking is automatic and needs no extra calls. Do not invent a verifier \
                run.",
        },
        Sub {
            route: RECALLED,
            name: "recalled",
            signal: "recalled evidence from local memory stores, fenced",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'2'),
        },
        Sub {
            route: BROKER,
            name: "broker",
            signal: "reviewed background evidence from the knowledge broker, fenced",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'3'),
        },
        Sub {
            route: ATLAS_LENS,
            name: "atlas-lens",
            signal: "the living Atlas task lens, fenced",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'4'),
        },
        Sub {
            route: MEMORY,
            name: "memory",
            signal: "the operator's persistent memories, fenced",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'5'),
        },
        Sub {
            route: CONTINUAL,
            name: "continual",
            signal: "supplemental continual-harness state, fenced",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'6'),
        },
        Sub {
            route: RECALL_NOTES,
            name: "recall-notes",
            signal: "notes recalled from long-term memory for this project below",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'7'),
        },
        Sub {
            route: CAMPAIGN_LENS,
            name: "campaign-lens",
            signal: "the active campaign's objective receipt, fenced",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: HANDOFF,
            name: "handoff",
            signal: "the agent's own handoff, carried across compaction",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'9'),
        },
        Sub {
            route: HANDOFF_STALE,
            name: "handoff-stale",
            signal: "the carried handoff is stale (its age below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(CATALOG, b'0'),
        },
    ],
};

/// The contract route for a research turn, with its declared origin.
pub(crate) fn contract(origin: Option<&str>) -> Raise {
    let evidence = match origin {
        Some(origin) => format!("Research origin: {origin}"),
        None => "No research origin was declared; do not invent one.".to_string(),
    };
    Raise::new(CONTRACT, evidence)
}
