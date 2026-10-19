//! The lie of the land: the realm's map, its doors, where the castles, the
//! college and the party's wishes stand, the kingdom's geography the rules
//! walk on, the great houses, and the tunnels' routing. Nothing here draws;
//! the renderer reads it (and lends it the font's widths, to measure names).

pub mod castles;
pub mod college;
pub mod doors;
pub mod houses;
pub mod kingdom;
pub mod map;
pub mod routing;
pub mod wild;
pub mod biome;
pub mod roads;
pub mod signs;
pub mod finds;
pub mod dressing;
pub mod wishes;

pub use castles::castles;
