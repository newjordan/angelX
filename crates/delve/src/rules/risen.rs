//! Humphrey the Risen: the countryside's very persistent former neighbor.
//!
//! Once the festival is properly underway he claws up through the compost,
//! shambling after the party with a hedge grudge and a two-handed shove.
//! Felling him pays a bounty and buys quiet — never peace: he reassembles,
//! each time a little larger. He never spawns in the Delve; below the gate
//! is somebody else's problem.
//!
//! His spawning is retired (the surface keeps no zombies); the countryside
//! still keeps his compost timers.

use super::*;

/// How much round-free lingering in one room before he claws in.
pub const FOLLOW: u32 = 8 * HZ;
/// Compost time after he is felled.
pub const RISE_AGAIN: u32 = 45 * HZ;
