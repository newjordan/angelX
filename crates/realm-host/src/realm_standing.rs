//! The wishes standing in the realm, as last loaded or saved.
//!
//! The overworld reads built wishes from here without file access. The
//! cockpit's realm save publishes into the slot (`publish`) whenever it loads
//! or saves; the old `drive::together_realm::standing_wishes` path re-exports
//! the reader.

use angel_delve::realm::Wish;
use std::sync::{Arc, OnceLock, RwLock};

fn standing() -> &'static RwLock<Arc<Vec<Wish>>> {
    static STANDING: OnceLock<RwLock<Arc<Vec<Wish>>>> = OnceLock::new();
    STANDING.get_or_init(Default::default)
}

/// The wishes standing in the realm, as last loaded or saved.
pub fn standing_wishes() -> Arc<Vec<Wish>> {
    standing().read().map(|w| w.clone()).unwrap_or_default()
}

/// Replace the standing wishes.
pub fn publish(built: Arc<Vec<Wish>>) {
    if let Ok(mut slot) = standing().write() {
        *slot = built;
    }
}
