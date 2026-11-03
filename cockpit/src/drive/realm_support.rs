//! The one door the Delve (realm) code left in the cockpit uses into platform
//! helpers.
//!
//! The game files under `ui/viz/` and `app/control/dungeon*` reach the digest
//! and the atomic stores only through here, so the cut that moves them out of
//! the cockpit crate has a single seam. Every
//! item below is a re-export of the Kernel's host services
//! (`angel_live::host`), which both crates stand on; nothing is wrapped or
//! changed.

#[cfg(test)]
pub(crate) use angel_live::host::sha256_hex;

pub(crate) mod workspace_store {
    /// Test-only fault injection for the atomic writers (the cockpit's storage
    /// regression tests own the arming helper).
    #[cfg(test)]
    pub(crate) use crate::platform::workspace_store::storage_atomic_tests;
    pub(crate) use angel_live::host::workspace_store::{
        try_private_lock, write_atomic, write_private_atomic_exact,
    };
}
