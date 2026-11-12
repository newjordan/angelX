//! The kingdom's replaceable production game. No renderer, clock, files or
//! host APIs: content is a checked catalogue, play is commands and work shifts.
//! `rules::industry` adapts real Delve encounters and seat input to this model.
mod catalog;
mod construction;
mod claims;
mod economy;
mod energy;
mod transport;
mod nature;
pub mod haul;
pub mod rollers;
pub mod rails;
pub mod carting;
pub mod hillside;
pub mod migrate;
pub mod site;
pub mod clerk;
pub mod ghosts;
pub mod books;
pub mod numbers;
pub mod goals;
pub mod seam;
pub mod pace;
pub mod first_chain;
pub mod patterns;
pub mod machines;
pub mod pressure;
pub mod defence;
pub mod design;
pub mod population;
pub mod power;
pub mod citizen_motion;
mod settlement;
pub mod world;
pub use catalog::*;
pub use nature::rect_of;
pub use construction::Placement;
pub use economy::*;
pub use power::{ConsumerPower, MachinePower, PowerComponent, PowerComponentId, PowerGrid, PowerKind, PowerNode, PowerReport, PowerRole, PowerStatus, PowerTotals};
pub use haul::{Line, LineRow, LineState, LineStats, Phase, Vehicle};
pub use rails::{Act, Bay, Rail, RailJam, Rails, Route, RouteRow, Stop, Wagon, WagonState};
pub use rollers::{Cell as RollerCell, Dir, Jam, Piece, RollerRow, RollerState, Rollers};
pub use carting::{Cart, CartPhase, RoadReport, Roadwork, Unserved, YardRow};
pub use hillside::{Deposit, HILL_CAMP, HILL_GATE, KEEP_CENTRE, KEEP_FOOTPRINT, REALM_HEIGHT, REALM_WIDTH, keep_rect, in_rect as hillside_in_rect};
pub use site::{Allowance, Hold, Site};
pub use clerk::{Cause, Chain, Clerk};
pub use ghosts::{Crew, Ghost, Laid, Leg, Plans};
pub use numbers::{Card, Choke, GoodLine, Numbers, Production, Row, Tone};
pub use goals::{Goals, Milestone};
pub use pressure::{Pressure, Stir};
pub use defence::{Blow, Havoc, Lure, LureKind, Tower};
pub use machines::MachineRow;
pub use world::{ExpansionEvidence, ExpansionOffer, ExpansionRequirement, Parcel, Terrain, World};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod village_tests;
#[cfg(test)]
mod food_tests;
#[cfg(test)]
mod citizen_scene_tests;
#[cfg(test)]
mod works_tests;
#[cfg(test)]
mod economy_tests;
#[cfg(test)]
mod roads_tests;
#[cfg(test)]
mod works_layout;
#[cfg(test)]
mod works_play;
#[cfg(test)]
mod seam_tests;
#[cfg(test)]
mod migrate_tests;
#[cfg(test)]
mod ux_tests;
#[cfg(test)]
mod tiers_tests;
#[cfg(test)]
mod rollers_tests;
#[cfg(test)]
mod rails_tests;
#[cfg(test)]
mod transport_bench;
#[cfg(test)]
mod defence_tests;
