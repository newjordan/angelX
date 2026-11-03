//! The miniworld, from the realm host (`angel_realm_host::world_viz`), with the
//! two pieces that stay here: the arcade painters onto a ratatui frame
//! (`art`, which the shooter view draws with) and the world pane's drag state
//! (`world_camera`, which names the scryglass surface).

pub(crate) use angel_realm_host::world_viz::*;

pub(crate) mod art;
pub(crate) mod world_camera;

// The world's tests that need the cockpit (a `LoopState`, the scryglass, the
// realm's catalog, a guest's picture): they run here, against its public paths.
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/coupled__tests.rs"]
mod coupled_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__plates_tests.rs"]
mod plates_tests;
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__wish_placement_tests.rs"]
mod wish_placement_tests;
