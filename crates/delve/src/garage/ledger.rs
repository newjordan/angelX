//! The garage's memory: every knight's racer, kept by name. The ledger is
//! plain data the host owns, like the realm's `brought`: it outlives games
//! and restarts because the host writes it down (`Store`) after every swap
//! and reads it back before the first knight walks in.
//!
//! A knight is known by their name, trimmed and lowercased (`key`), as the
//! host keeps seats. A name never seen gets a starting racer from the kit
//! (`Build::starting`), which is not written down until they swap a part.

use super::parts::{Part, Slot};
use super::racer::{Build, Stats};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The newest save format this build writes. Older ones load; anything with
/// a larger number was written by a newer garage and is read as far as it is
/// understood.
pub const VERSION: u32 = 1;
/// Names are cut to this many characters, as the realm's are.
pub const MAX_NAME: usize = 28;
/// The garage remembers this many knights; past it the one who swapped
/// longest ago is forgotten (and gets a starting racer if they return).
pub const MAX_KNIGHTS: usize = 512;

/// The key a name is kept under.
pub fn key(name: &str) -> String {
    name.trim().chars().take(MAX_NAME).collect::<String>().to_lowercase()
}

/// One knight's entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// The name as the knight last wrote it.
    pub name: String,
    pub build: Build,
    /// Swaps made, ever.
    pub swaps: u32,
    /// The ledger's `seq` at the last swap: a clock that needs no clock.
    pub seq: u64,
}

/// What a swap did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swap {
    pub slot: Slot,
    pub from: &'static Part,
    pub to: &'static Part,
    pub before: Stats,
    pub after: Stats,
}

/// A knight's racer as the race takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrant {
    pub name: String,
    pub build: Build,
    pub stats: Stats,
}

impl Entrant {
    /// The racer's whole body in world3d meshes, nose at +x and standing on
    /// z = 0, centred: `t` is seconds, for the flames and the arc. Written
    /// into `out`, which is cleared first.
    pub fn mesh(&self, t: f32, out: &mut crate::render::world3d::mesh::Mesh) {
        super::meshes::racer(&self.build, t, out);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwapError {
    /// The catalogue has no such part.
    Unknown,
    /// The knight already has it.
    Already,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub v: u32,
    /// Counts every swap ever made; each entry notes it at its last.
    pub seq: u64,
    pub knights: BTreeMap<String, Entry>,
    /// Set by a change, cleared by `Store::save`: not saved.
    #[serde(skip)]
    dirty: bool,
}

/// Two ledgers are the same when they remember the same: whether one has been
/// written down is not part of that.
impl PartialEq for Ledger {
    fn eq(&self, other: &Ledger) -> bool {
        (self.v, self.seq, &self.knights) == (other.v, other.seq, &other.knights)
    }
}

impl Eq for Ledger {}

impl Default for Ledger {
    fn default() -> Ledger {
        Ledger { v: VERSION, seq: 0, knights: BTreeMap::new(), dirty: false }
    }
}

impl Ledger {
    pub fn new() -> Ledger {
        Ledger::default()
    }

    /// The knight's racer: what they last built, or their starting racer.
    pub fn build(&self, name: &str) -> Build {
        self.knights.get(&key(name)).map_or_else(|| Build::starting(name), |e| e.build.clone().sound())
    }

    /// What the race reads for a knight: the racer they built, its numbers
    /// and (through [`Entrant::mesh`]) its parts as world3d meshes.
    pub fn entrant(&self, name: &str) -> Entrant {
        let build = self.build(name);
        Entrant { name: name.trim().chars().take(MAX_NAME).collect(), stats: build.stats(), build }
    }

    /// Whether the knight has swapped a part here before.
    pub fn knows(&self, name: &str) -> bool {
        self.knights.contains_key(&key(name))
    }

    /// Put a part on the knight's racer and write it into the ledger.
    pub fn swap(&mut self, name: &str, part_id: &str) -> Result<Swap, SwapError> {
        let to = super::parts::part(part_id).ok_or(SwapError::Unknown)?;
        let mut build = self.build(name);
        let before = build.stats();
        let from = build.part(to.slot);
        if !build.fit(part_id) {
            return Err(SwapError::Already);
        }
        let after = build.stats();
        self.keep(name, build);
        Ok(Swap { slot: to.slot, from, to, before, after })
    }

    /// Write a whole racer down for a knight (a kit racer, an undo).
    pub fn set(&mut self, name: &str, build: Build) {
        if self.build(name) != build.clone().sound() {
            self.keep(name, build);
        }
    }

    fn keep(&mut self, name: &str, build: Build) {
        self.seq += 1;
        let seq = self.seq;
        let e = self.knights.entry(key(name)).or_default();
        e.name = name.trim().chars().take(MAX_NAME).collect();
        e.build = build.sound();
        e.swaps = e.swaps.saturating_add(1);
        e.seq = seq;
        self.dirty = true;
        while self.knights.len() > MAX_KNIGHTS {
            let oldest = self.knights.iter().min_by_key(|(_, e)| e.seq).map(|(k, _)| k.clone());
            match oldest {
                Some(k) => self.knights.remove(&k),
                None => break,
            };
        }
    }

    /// Whether something changed since the last `Store::save`.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub(super) fn clean(&mut self) {
        self.dirty = false;
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a ledger is plain data")
    }

    /// Read a saved ledger. Missing fields take their defaults; unknown ones
    /// are ignored; every build is made sound, so a part the catalogue has
    /// dropped becomes the default racer's.
    pub fn from_json(text: &str) -> Result<Ledger, String> {
        let mut l: Ledger = serde_json::from_str(text).map_err(|e| e.to_string())?;
        l.v = l.v.max(1);
        for e in l.knights.values_mut() {
            e.build = std::mem::take(&mut e.build).sound();
        }
        l.dirty = false;
        Ok(l)
    }
}
