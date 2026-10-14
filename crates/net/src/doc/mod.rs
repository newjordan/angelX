//! Document replication: a whole game state as a serde value tree, sent as
//! patches against what the client last received. For a party-sized game
//! (the Delve); a crowd uses entity replication instead.

pub mod delta;
pub mod value;
pub mod wire;
