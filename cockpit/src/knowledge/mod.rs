//! What the cockpit records and recalls: ledgers, memory, and curated sources.

pub(crate) mod atlas;
pub(crate) mod atlas_clerk;
pub(crate) mod barrel;
// Deli (deep-over-time research loop) folds into the swarm rather than being its
// own selectable agent — kept pending that symbiotic integration.
pub(crate) mod caddy;
// The Cut: the authored-diff manifest + the machine verdict on every write
// (docs/plans/the-cut.md). Rust only appends JSONL; the Node tick folds it.
pub(crate) mod cut;
pub(crate) mod dossier;
pub(crate) mod evidence;
pub(crate) mod experience;
#[allow(dead_code)]
pub(crate) mod librarian;
pub(crate) mod library;
pub(crate) mod memory;
pub(crate) mod repos;
pub(crate) mod route_intelligence;
pub(crate) mod session;
pub(crate) mod skills;
