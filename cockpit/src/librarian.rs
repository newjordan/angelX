//! The Librarian — curation layer between agent/moa *reports* and the long-form
//! memory palace ([`crate::memory_store`]).
//!
//! Compaction files its own structured drawers directly (it already produces
//! clean, sectioned notes). The moa is different: a fan-out produces free-form
//! synthesized **reports**, and filing those raw would flood the palace with
//! near-duplicate, low-signal chatter. The Librarian is the gate — it decides what
//! a report actually contributes, dedups against what's already filed, routes each
//! kept item to the right wing/room, and only then deposits.
//!
//! **Status: interface + a minimal pass-through only.** The moa is NOT yet wired
//! to it — that integration (`moa.rs::synthesize` → Librarian → palace) is
//! deferred until the curation policy below (dedup / importance / splitting) is
//! real, so the moa can't pollute the store in the meantime. The cockpit's
//! compaction path deposits to the store directly today, bypassing the Librarian.

use crate::memory_store::{Drawer, MemoryStore};
use std::sync::Arc;

/// A free-form contribution from an agent or a moa synthesis, before curation.
/// `topic` becomes the drawer `room`; `wing`/`source` tag provenance as usual.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub wing: String,
    pub topic: String,
    pub body: String,
    pub source: String,
}

/// Curates reports into the palace. Holds the store plus (eventually) the dedup
/// index and routing/importance policy.
pub struct Librarian {
    store: Arc<dyn MemoryStore>,
}

impl Librarian {
    pub fn new(store: Arc<dyn MemoryStore>) -> Self {
        Self { store }
    }

    /// File a report into the palace after curation.
    ///
    /// v0 is a **pass-through**: a non-empty report becomes one drawer, with no
    /// dedup, importance gating, or splitting yet — those are the work that gates
    /// the moa wiring (see [`curate`]). Returns whether anything was filed.
    pub fn file(&self, report: &Report) -> bool {
        let drawers = curate(report);
        let mut filed = 0usize;
        for d in &drawers {
            if self.store.deposit(d).is_ok() {
                filed += 1;
            }
        }
        filed > 0
    }
}

/// Turn a report into the drawers that should actually be filed.
///
/// TODO(librarian): this is where the moa-grade curation lives — dedup against
/// the existing palace (`store.search` the topic first), score importance and
/// drop low-signal reports, and split a multi-topic synthesis into one drawer per
/// finding. Until that exists, the moa stays unwired. v0: one drawer, verbatim.
fn curate(report: &Report) -> Vec<Drawer> {
    if report.body.trim().is_empty() {
        return Vec::new();
    }
    vec![Drawer {
        wing: report.wing.clone(),
        room: report.topic.clone(),
        content: report.body.trim().to_string(),
        source: report.source.clone(),
    }]
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/librarian__tests.rs"]
mod tests;
