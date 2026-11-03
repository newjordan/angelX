//! The campaign the kingdom mirrors: real work, counted outside the realm.
//!
//! The heart questline (`docs/HEART-QUEST.md`) walks on seven counters the
//! `vec` cartridge keeps in its board (`board.json`, schema `vec.board/1`,
//! the cartridge's `CONTRACT.md`): data staged, heart boards measured,
//! submitted and scored above their floors. They count work done on the
//! Virtual Embryo heart boards, and nothing in the realm can raise them.
//!
//! This module only reads. `Campaign::parse` turns a board's bytes into
//! the seven counts, bounded: a board over `MAX_BYTES`, nested past what
//! `serde_json` will read, of another schema, or with any of the seven
//! that is not a whole number from 0 to `MAX_COUNT`, reads as all zeros,
//! whole, never in part. A counter the realm does not know is ignored; a
//! missing one is 0. There is no way to add to a count: a `Campaign` is
//! made by reading a board (the host's, off the sim thread), or by the
//! wire carrying the host's to a client for drawing.

use serde::{Deserialize, Serialize};

/// The schema this reader knows. Another schema is another contract.
pub const SCHEMA: &str = "vec.board/1";

/// The counters, in the board's `quest` object. A questline names them by
/// these words; nothing else in the realm may (`questline` checks).
pub const COUNTERS: [&str; 7] = [
    "vec_data_staged",
    "vec_heart_proxy_runs",
    "vec_heart_submitted",
    "vec_heart_interp_above_floor",
    "vec_heart_extrap_above_floor",
    "vec_gata4_above_floor",
    "vec_heart_top10",
];

/// The prefix the campaign's counters own: a questline naming a counter
/// with it names the campaign's, and must name one of `COUNTERS`.
pub const PREFIX: &str = "vec_";

/// The most a board may weigh: it is a few kilobytes.
pub const MAX_BYTES: usize = 256 * 1024;

/// The most a counter may read. A count past it is not a count.
pub const MAX_COUNT: u64 = 1_000_000;

/// The campaign's counters as the board last said them. Zeros until a
/// board is read; zeros again when the board goes missing or wrong.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Campaign {
    counts: [u32; COUNTERS.len()],
}

impl Campaign {
    /// A board's bytes, read. Never panics: anything wrong reads as zeros.
    pub fn parse(bytes: &[u8]) -> Campaign {
        Campaign::read(bytes).unwrap_or_default()
    }

    fn read(bytes: &[u8]) -> Option<Campaign> {
        if bytes.len() > MAX_BYTES {
            return None;
        }
        // serde_json stops at 128 levels of nesting, with an error.
        let board: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        if board.get("schema")?.as_str()? != SCHEMA {
            return None;
        }
        let quest = board.get("quest")?.as_object()?;
        let mut counts = [0; COUNTERS.len()];
        for (count, name) in counts.iter_mut().zip(COUNTERS) {
            if let Some(value) = quest.get(name) {
                // Only a whole number: a negative, a fraction, a string, a
                // bool or a null is a board gone wrong, and so is the rest.
                let n = value.as_u64().filter(|&n| n <= MAX_COUNT)?;
                *count = n as u32;
            }
        }
        Some(Campaign { counts })
    }

    /// The count of `counter`: 0 for one the campaign does not keep.
    pub fn count(&self, counter: &str) -> u32 {
        COUNTERS
            .iter()
            .position(|&c| c == counter)
            .map_or(0, |i| self.counts[i])
    }

    /// Every counter with its count, in the board's order.
    pub fn counts(&self) -> impl Iterator<Item = (&'static str, u32)> + '_ {
        COUNTERS.iter().copied().zip(self.counts.iter().copied())
    }

    /// Nothing counted yet (or no board to read).
    pub fn is_empty(&self) -> bool {
        self.counts.iter().all(|&n| n == 0)
    }
}

/// Whether `counter` is one of the campaign's.
pub fn is_counter(counter: &str) -> bool {
    COUNTERS.contains(&counter)
}

/// Whether `counter` is in the campaign's reserved words (its prefix).
pub fn reserved(counter: &str) -> bool {
    counter.starts_with(PREFIX)
}

#[cfg(test)]
#[path = "../../tests/rules/campaign__tests.rs"]
mod tests;
