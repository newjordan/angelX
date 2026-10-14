//! The plain data the party's realm keeps between runs, shared by the rules
//! and the host's realm: spoils, wishes, a haul banked at once, a player's
//! avatar, and the stables' horses and the forge's weapons. The realm
//! itself (its treasury, its save) is the host's.

pub mod chivalry;
pub mod forge;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_WISHES: usize = 64;

/// What a delve yields. Each pack has its own; gold is everywhere; a bond is
/// earned only by clearing a room with two knights standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spoil {
    Gold,
    Bone,
    Wax,
    Ore,
    Gem,
    Ember,
    Scale,
    Bond,
}

impl Spoil {
    pub const ALL: [Spoil; 8] = [
        Spoil::Gold,
        Spoil::Bone,
        Spoil::Wax,
        Spoil::Ore,
        Spoil::Gem,
        Spoil::Ember,
        Spoil::Scale,
        Spoil::Bond,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Spoil::Gold => "gold",
            Spoil::Bone => "bone",
            Spoil::Wax => "wax",
            Spoil::Ore => "ore",
            Spoil::Gem => "gem",
            Spoil::Ember => "ember",
            Spoil::Scale => "scale",
            Spoil::Bond => "bond",
        }
    }

    pub fn from_word(word: &str) -> Option<Spoil> {
        Spoil::ALL
            .into_iter()
            .find(|s| s.word() == word.trim_end_matches('s'))
    }

    /// Where it comes from, for a player deciding where to delve.
    pub fn source(self) -> &'static str {
        match self {
            Spoil::Gold => "anywhere",
            Spoil::Bone | Spoil::Wax => "the Crypt",
            Spoil::Ore | Spoil::Gem => "the Mines",
            Spoil::Ember => "Dragon Keep",
            Spoil::Scale => "slaying the dragon",
            Spoil::Bond => "clearing a room together",
        }
    }

    /// Worth in gold, for a wish's floor price.
    pub fn worth(self) -> u32 {
        match self {
            Spoil::Gold => 1,
            Spoil::Bone | Spoil::Wax | Spoil::Ore => 20,
            Spoil::Gem | Spoil::Ember => 60,
            Spoil::Bond => 80,
            Spoil::Scale => 600,
        }
    }
}

/// An amount of each spoil.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spoils(pub BTreeMap<Spoil, u32>);

impl Spoils {
    pub fn get(&self, spoil: Spoil) -> u32 {
        self.0.get(&spoil).copied().unwrap_or(0)
    }

    pub fn add(&mut self, spoil: Spoil, n: u32) {
        if n > 0 {
            let slot = self.0.entry(spoil).or_insert(0);
            *slot = slot.saturating_add(n);
        }
    }

    pub fn merge(&mut self, other: &Spoils) {
        for (&spoil, &n) in &other.0 {
            self.add(spoil, n);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.values().all(|&n| n == 0)
    }

    /// Half of each, rounded down: what a wiped party carries home.
    pub fn half(&self) -> Spoils {
        Spoils(
            self.0
                .iter()
                .map(|(&s, &n)| (s, n / 2))
                .filter(|&(_, n)| n > 0)
                .collect(),
        )
    }

    pub fn covers(&self, price: &Spoils) -> bool {
        price.0.iter().all(|(&s, &n)| self.get(s) >= n)
    }

    pub fn take(&mut self, price: &Spoils) {
        for (&s, &n) in &price.0 {
            if let Some(slot) = self.0.get_mut(&s) {
                *slot = slot.saturating_sub(n);
            }
        }
        self.0.retain(|_, n| *n > 0);
    }

    pub fn worth(&self) -> u32 {
        self.0.iter().map(|(&s, &n)| s.worth() * n).sum()
    }

    /// `12 bone · 3 wax · 150 gold`, in the order spoils are listed.
    pub fn label(&self) -> String {
        let parts: Vec<String> = Spoil::ALL
            .into_iter()
            .filter(|&s| self.get(s) > 0)
            .map(|s| format!("{} {}", self.get(s), s.word()))
            .collect();
        if parts.is_empty() {
            "nothing".into()
        } else {
            parts.join(" · ")
        }
    }

    /// `need 4 more bone · 1 scale`, or empty when covered.
    pub fn shortfall(&self, price: &Spoils) -> String {
        let mut missing = Spoils::default();
        for (&s, &n) in &price.0 {
            missing.add(s, n.saturating_sub(self.get(s)));
        }
        if missing.is_empty() {
            String::new()
        } else {
            format!("need {}", missing.label())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Plain words, waiting for the host's angelX to draft it.
    Asked,
    /// Drafted with art and a price; waiting for the treasury.
    Drafted,
    /// Paid for and standing in the realm.
    Built,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wish {
    pub id: String,
    pub name: String,
    /// The wish in the words of whoever made it.
    pub words: String,
    pub by: String,
    pub status: Status,
    #[serde(default)]
    pub price: Spoils,
    /// Ink rows, at most 32 by 32.
    #[serde(default)]
    pub art: Vec<String>,
    /// The overworld place it should stand near (`Place` name), if any.
    #[serde(default)]
    pub near: String,
    /// Who paid what, once built.
    #[serde(default)]
    pub paid: Vec<(String, Spoils)>,
    /// Built wishes keep the order they were raised in.
    #[serde(default)]
    pub raised: u32,
}

/// One knight's spoils banked at once, with why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Haul {
    pub hero: u32,
    pub spoils: Spoils,
    pub why: String,
}

/// A player's own knight, as their image model drew it (checked and
/// normalised by the host).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Avatar {
    /// 32×32 PNG for the arena.
    pub sprite: Vec<u8>,
    /// 128×128 PNG for the player cards.
    pub portrait: Vec<u8>,
    /// The sprite's most common colour, for the terminal arena.
    pub colour: [u8; 3],
    /// Decoded, normalized sprite; avoids PNG decoding on every game frame.
    pub pixels: Vec<[u8; 4]>,
}
