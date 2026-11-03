//! ⠐ (dots 5) — Volume IV, the mixture: its frames. The tasks the judge and
//! the aggregator seats are handed with their data — scoring, revising,
//! holding an answer to its sources, the final synthesis and an intermediate
//! merge, the output-length guidance — the labels on the payload (the drafts,
//! a trimmed context, a transcript made text-only), the formation's preflight,
//! a delegator's consults and the evidence they bring back. A label is sent as
//! its page address with its value beside it; a frame as its route.

use super::d46_recovery::Page;
use super::{Primary, Route, Sub};


const CATALOG: &str = include_str!("../../../../book/d5_frames.book");
pub(crate) const CELL: char = '⠐';
pub(crate) const SCORING: Route = Route::new(CELL, '⠁');
pub(crate) const REVISE: Route = Route::new(CELL, '⠃');
pub(crate) const CITE: Route = Route::new(CELL, '⠉');
pub(crate) const SYNTHESIS: Route = Route::new(CELL, '⠙');
pub(crate) const MERGE: Route = Route::new(CELL, '⠑');
pub(crate) const GUIDANCE: Route = Route::new(CELL, '⠋');
pub(crate) const LABELS: Route = Route::new(CELL, '⠛');
pub(crate) const PREFLIGHT: Route = Route::new(CELL, '⠓');
pub(crate) const CONSULTS: Route = Route::new(CELL, '⠊');
pub(crate) const EVIDENCE: Route = Route::new(CELL, '⠚');

// `⠐⠛`'s grounding headings and `⠐⠚`'s unstated claim, each sent alone as
// its address.
pub(crate) const GROK_SCOUT_HEADING: Page = Page::new(LABELS, 8);
pub(crate) const SEARX_HEADING: Page = Page::new(LABELS, 9);
pub(crate) const UNSTATED_CLAIM: Page = Page::new(EVIDENCE, 4);

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "frames",
    surface: "the mixture's frames: stage tasks, payload labels, preflight, consults, evidence",
    subs: &[
        Sub {
            route: SCORING,
            name: "scoring",
            signal: "the judge's scoring task, one score or per dimension (counts beside, responses below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'1'),
        },
        Sub {
            route: REVISE,
            name: "revise",
            signal: "revise an answer to fix the verifier's problems (both below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'2'),
        },
        Sub {
            route: CITE,
            name: "cite",
            signal: "hold an answer's claims to the gathered sources (both below)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(CATALOG, b'3'),
        },
        Sub {
            route: SYNTHESIS,
            name: "synthesis",
            signal: "the final synthesis of the drafts below",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(CATALOG, b'4'),
        },
        Sub {
            route: MERGE,
            name: "merge",
            signal: "an intermediate merge of the drafts below",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'5'),
        },
        Sub {
            route: GUIDANCE,
            name: "guidance",
            signal: "a target length for the delivered answer (the target beside the route)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(CATALOG, b'6'),
        },
        Sub {
            route: LABELS,
            name: "labels",
            signal: "the labels on a stage's payload, each sent with its value",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(CATALOG, b'7'),
        },
        Sub {
            route: PREFLIGHT,
            name: "preflight",
            signal: "the formation's planning preflight before the first tool action",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(CATALOG, b'8'),
        },
        Sub {
            route: CONSULTS,
            name: "consults",
            signal: "a delegator's consults: a peer's second opinion, a phoned specialist",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(CATALOG, b'9'),
        },
        Sub {
            route: EVIDENCE,
            name: "evidence",
            signal: "executed test evidence from the delegators (results below; a request with no claim marked by its page)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(CATALOG, b'0'),
        },
    ],
};
