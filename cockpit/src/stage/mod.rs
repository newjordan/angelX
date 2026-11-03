//! The miniworld and its ceremonies — the gamified systems view.

// The miniworld and the rest of the stage's realm live in the realm host
// (`crates/realm-host`); each keeps its old path here.
pub(crate) use angel_realm_host::{identity, knight_cast, village};
pub(crate) mod houses;
pub(crate) mod knight_journey;
pub(crate) mod questmap;
pub(crate) mod raytrace;
pub(crate) mod realm_hooks;
pub(crate) mod world_viz;
