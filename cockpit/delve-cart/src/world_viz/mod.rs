//! The miniworld, from the realm host (`angel_realm_host::world_viz`), with the
//! piece that stays here: the arcade painters onto a ratatui frame (`art`,
//! which the shooter view draws with).

pub use angel_realm_host::world_viz::*;

pub mod art;

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__plates_tests.rs"]
mod plates_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__wish_placement_tests.rs"]
mod wish_placement_tests;
