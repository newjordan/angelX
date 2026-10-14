//! Entity replication, for crowds: the muster's 200 knights, and any game
//! with more things moving than a document diff can afford.
//!
//! The game keeps a `Table` of small records: an id, a kind, a position for
//! interest, the bytes fixed when the thing appears (`spawn`: a name, a
//! colour) and the few bytes that change (`state`: where it is, what it is
//! doing). Once a tick it publishes the table as a `World`, shared by every
//! connection's writer. Each writer keeps a `Viewer`: what its client holds,
//! by id and version, so it sends only what is near enough to matter and
//! has changed since it last wrote to that client. The client keeps a
//! `Mirror` of what it was sent, with a few samples of each thing so it can
//! draw everyone between two ticks.
//!
//! The stream is a WebSocket over TCP, reliable and ordered, so what was
//! last written to a client is what it holds: no acks, and no baselines
//! kept per snapshot.
//!
//! A snapshot body, front to back:
//!
//! | part | bytes |
//! |---|---|
//! | flags | 1 byte: `DEFERRED` when something the client holds changed and is not in this snapshot |
//! | globals | 1 byte (0 unchanged, 1 whole, 2 patch), then the value or patch (`doc::wire`) |
//! | despawns | count, then ids |
//! | spawns | count, then for each: id, kind, spawn bytes, state bytes |
//! | updates | count, then for each: id, then the state whole or only its changed bytes |
//!
//! Ids in each list rise, and each is sent as the step from the one before.
//! An update's head is a varint: the state's length times two, and the
//! state follows whole; or a mask of the bytes that differ from what the
//! client holds (bit i, byte i) times two plus one, and those bytes follow.
//! A walking knight moves the low bytes of its place: two bytes, not five.
//! A snapshot marked whole starts the client over: an empty mirror, no
//! globals, and a fresh name table for the globals' field names.

mod mirror;
mod send;
mod world;

pub use mirror::{Mirror, Mirrored};
pub use send::{Interest, Viewer, Written};
pub use world::{Entity, State, Table, World};

/// Bytes one snapshot may carry, everything in it counted. What does not
/// fit carries to the next snapshot. At 20 snapshots a second this caps a
/// client near 24 KB/s however crowded its view.
pub const BUDGET: usize = 1200;
/// A thing's changing bytes: a position and a pose, not a document.
pub const MAX_STATE: usize = 24;
/// A thing's bytes fixed at its appearance: a colour and a name.
pub const MAX_SPAWN: usize = 64;
/// Samples a client keeps of each thing: enough to draw 100 ms behind
/// through a few late snapshots.
pub const HISTORY: usize = 8;

/// Something the client holds changed and was left for a later snapshot,
/// so the things this snapshot does not mention may not be as they were.
const DEFERRED: u8 = 1;

const GLOBALS_SAME: u8 = 0;
const GLOBALS_WHOLE: u8 = 1;
const GLOBALS_PATCH: u8 = 2;

/// Bytes a varint of `n` takes.
fn varint_len(n: u64) -> usize {
    let bits = 64 - n.leading_zeros() as usize;
    bits.max(1).div_ceil(7)
}

#[cfg(test)]
mod tests;
