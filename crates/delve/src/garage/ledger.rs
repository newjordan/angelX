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
use crate::rules::knight_racers::Career;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const VERSION: u32 = 1;
pub const MAX_NAME: usize = 28;
pub const MAX_KNIGHTS: usize = 512;
pub const MAX_GHOSTS: usize = 12;

pub fn key(name: &str) -> String {
    name.trim().chars().take(MAX_NAME).collect::<String>().to_lowercase()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub name: String,
    pub build: Build,
    pub swaps: u32,
    pub seq: u64,
    pub career: Career,
    pub ghosts: BTreeMap<String, Ghost>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ghost {
    pub ms: u32,
    pub seq: u64,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swap {
    pub slot: Slot,
    pub from: &'static Part,
    pub to: &'static Part,
    pub before: Stats,
    pub after: Stats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrant {
    pub name: String,
    pub build: Build,
    pub stats: Stats,
}

impl Entrant {
    pub fn mesh(&self, t: f32, out: &mut crate::render::world3d::mesh::Mesh) {
        super::meshes::racer(&self.build, t, out);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwapError {
    Unknown,
    Already,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub v: u32,
    pub seq: u64,
    pub knights: BTreeMap<String, Entry>,
    #[serde(skip)]
    dirty: bool,
}

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

    pub fn build(&self, name: &str) -> Build {
        self.knights.get(&key(name)).map_or_else(|| Build::starting(name), |e| e.build.clone().sound())
    }

    pub fn entrant(&self, name: &str) -> Entrant {
        let build = self.build(name);
        Entrant { name: name.trim().chars().take(MAX_NAME).collect(), stats: build.stats(), build }
    }

    pub fn knows(&self, name: &str) -> bool {
        self.knights.contains_key(&key(name))
    }

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

    pub fn set(&mut self, name: &str, build: Build) {
        if self.build(name) != build.clone().sound() {
            self.keep(name, build);
        }
    }

    fn keep(&mut self, name: &str, build: Build) {
        self.stamp(name, true, |e| e.build = build.sound());
    }

    fn stamp(&mut self, name: &str, swap: bool, change: impl FnOnce(&mut Entry)) {
        self.seq += 1;
        let seq = self.seq;
        let e = self.knights.entry(key(name)).or_default();
        e.name = name.trim().chars().take(MAX_NAME).collect();
        change(e);
        if swap { e.swaps = e.swaps.saturating_add(1); }
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

    pub fn career(&self, name: &str) -> Career {
        let ids = self.build(name).ids();
        match self.knights.get(&key(name)) {
            Some(e) => {
                let mut c = e.career.clone();
                c.build = ids;
                c.own_what_is_worn();
                c
            }
            None => Career::with_build(ids),
        }
    }

    pub fn set_career(&mut self, name: &str, career: &Career) {
        let build = Build::from_ids(&career.build);
        let career = Career { build: Career::default().build, ..career.clone() };
        self.stamp(name, false, |e| {
            e.build = build;
            e.career = career;
        });
    }

    pub fn ghost(&self, name: &str, key_: &str) -> Option<&Ghost> {
        self.knights.get(&key(name))?.ghosts.get(key_)
    }

    pub fn set_ghost(&mut self, name: &str, key_: &str, ghost: Ghost) -> bool {
        if self.ghost(name, key_).is_some_and(|g| g.ms > 0 && g.ms <= ghost.ms) {
            return false;
        }
        let key_ = key_.to_string();
        let seq = self.seq + 1;
        self.stamp(name, false, |e| {
            e.ghosts.insert(key_, Ghost { seq, ..ghost });
            while e.ghosts.len() > MAX_GHOSTS {
                let oldest = e.ghosts.iter().min_by_key(|(_, g)| g.seq).map(|(k, _)| k.clone());
                match oldest {
                    Some(k) => e.ghosts.remove(&k),
                    None => break,
                };
            }
        });
        true
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub(super) fn clean(&mut self) {
        self.dirty = false;
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a ledger is plain data")
    }

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
