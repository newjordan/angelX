//! The lie of the land as the rules see it: the one door from the sim into
//! the world's map data (its places, roads, gates and castles, and the
//! great houses). Rules read the world through this module only, never a
//! renderer's module. Nothing here draws.

pub use crate::world::houses;
pub use crate::world::houses::stand as castle_stand;
pub use crate::world::kingdom::{Geography, Site, gate_stand, geography, is_road, outdoor};
pub use crate::world::map::{MAP_H, MAP_W};
