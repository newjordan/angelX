//! The Librarian — curation layer between agent/moa *reports* and the long-form
//! memory palace ([`crate::knowledge::memory::store`]).
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

use crate::knowledge::memory::store::{Drawer, MemoryStore};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub wing: String,
    pub topic: String,
    pub body: String,
    pub source: String,
}

pub struct Librarian {
    store: Arc<dyn MemoryStore>,
}

impl Librarian {
    pub fn new(store: Arc<dyn MemoryStore>) -> Self {
        Self { store }
    }

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
#[path = "../../../tests/cockpit/app/librarian__tests.rs"]
mod tests;
