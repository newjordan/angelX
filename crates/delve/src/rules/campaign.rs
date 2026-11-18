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

pub const SCHEMA: &str = "vec.board/1";

pub const COUNTERS: [&str; 7] = [
    "vec_data_staged",
    "vec_heart_proxy_runs",
    "vec_heart_submitted",
    "vec_heart_interp_above_floor",
    "vec_heart_extrap_above_floor",
    "vec_gata4_above_floor",
    "vec_heart_top10",
];

pub const PREFIX: &str = "vec_";

pub const MAX_BYTES: usize = 256 * 1024;

pub const MAX_COUNT: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Campaign {
    counts: [u32; COUNTERS.len()],
}

impl Campaign {
    pub fn parse(bytes: &[u8]) -> Campaign {
        Campaign::read(bytes).unwrap_or_default()
    }

    fn read(bytes: &[u8]) -> Option<Campaign> {
        if bytes.len() > MAX_BYTES {
            return None;
        }
        let board: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        if board.get("schema")?.as_str()? != SCHEMA {
            return None;
        }
        let quest = board.get("quest")?.as_object()?;
        let mut counts = [0; COUNTERS.len()];
        for (count, name) in counts.iter_mut().zip(COUNTERS) {
            if let Some(value) = quest.get(name) {
                let n = value.as_u64().filter(|&n| n <= MAX_COUNT)?;
                *count = n as u32;
            }
        }
        Some(Campaign { counts })
    }

    pub fn count(&self, counter: &str) -> u32 {
        COUNTERS
            .iter()
            .position(|&c| c == counter)
            .map_or(0, |i| self.counts[i])
    }

    pub fn counts(&self) -> impl Iterator<Item = (&'static str, u32)> + '_ {
        COUNTERS.iter().copied().zip(self.counts.iter().copied())
    }

    pub fn is_empty(&self) -> bool {
        self.counts.iter().all(|&n| n == 0)
    }
}

pub fn is_counter(counter: &str) -> bool {
    COUNTERS.contains(&counter)
}

pub fn reserved(counter: &str) -> bool {
    counter.starts_with(PREFIX)
}

#[cfg(test)]
#[path = "../../tests/rules/campaign__tests.rs"]
mod tests;
