//! The kingdom's replaceable production game. No renderer, clock, files or
//! host APIs: content is a checked catalogue, play is commands and work shifts.
//! `rules::industry` adapts real Delve encounters and seat input to this model.
mod catalog;
mod construction;
mod claims;
mod economy;
mod energy;
mod transport;
pub mod population;
pub mod power;
pub mod citizen_motion;
mod settlement;
pub mod world;
pub use catalog::*;
pub use economy::*;
pub use power::{ConsumerPower, MachinePower, PowerComponent, PowerComponentId, PowerGrid, PowerKind, PowerNode, PowerReport, PowerRole, PowerStatus, PowerTotals};
pub use world::{ExpansionEvidence, ExpansionOffer, ExpansionRequirement, Parcel, Terrain, World};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod village_tests;
#[cfg(test)]
mod food_tests;
#[cfg(test)]
mod citizen_scene_tests;
