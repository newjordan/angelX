//! The plain data the party's realm keeps between runs, shared by the rules
//! and the host's realm: spoils, wishes, a haul banked at once, a player's
//! avatar, and the stables' horses and the forge's weapons. The realm
//! itself (its treasury, its save) is the host's.

pub mod chivalry;
pub mod forge;
pub mod squires;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_WISHES: usize = 64;

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
    Hearthstone,
    Forgeheart,
    Bellmetal,
    Ledgerglass,
    Warhorn,
    Waystone,
    Wishmote,
}

impl Spoil {
    pub const ALL: [Spoil; 15] = [
        Spoil::Gold,
        Spoil::Bone,
        Spoil::Wax,
        Spoil::Ore,
        Spoil::Gem,
        Spoil::Ember,
        Spoil::Scale,
        Spoil::Bond,
        Spoil::Hearthstone,
        Spoil::Forgeheart,
        Spoil::Bellmetal,
        Spoil::Ledgerglass,
        Spoil::Warhorn,
        Spoil::Waystone,
        Spoil::Wishmote,
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
            Spoil::Hearthstone => "hearthstone",
            Spoil::Forgeheart => "forgeheart",
            Spoil::Bellmetal => "bellmetal",
            Spoil::Ledgerglass => "ledgerglass",
            Spoil::Warhorn => "warhorn",
            Spoil::Waystone => "waystone",
            Spoil::Wishmote => "wishmote",
        }
    }

    pub fn from_word(word: &str) -> Option<Spoil> {
        Spoil::ALL
            .into_iter()
            .find(|s| s.word() == word.trim_end_matches('s'))
    }

    pub fn source(self) -> &'static str {
        match self {
            Spoil::Gold => "anywhere",
            Spoil::Bone | Spoil::Wax => "the Crypt",
            Spoil::Ore | Spoil::Gem => "the Mines",
            Spoil::Ember => "Dragon Keep",
            Spoil::Scale => "slaying the dragon",
            Spoil::Bond => "clearing a room together",
            Spoil::Hearthstone | Spoil::Forgeheart | Spoil::Ledgerglass => "the flooded Archive",
            Spoil::Bellmetal | Spoil::Warhorn => "the Fungal deep",
            Spoil::Waystone => "the Unknown floor",
            Spoil::Wishmote => "floor bosses, and wishes left unspent",
        }
    }

    pub fn worth(self) -> u32 {
        match self {
            Spoil::Gold => 1,
            Spoil::Bone | Spoil::Wax | Spoil::Ore => 20,
            Spoil::Gem | Spoil::Ember => 60,
            Spoil::Bond => 80,
            Spoil::Scale => 600,
            Spoil::Wishmote => 150,
            Spoil::Hearthstone
            | Spoil::Forgeheart
            | Spoil::Bellmetal
            | Spoil::Ledgerglass
            | Spoil::Warhorn
            | Spoil::Waystone => 300,
        }
    }
}

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
    Asked,
    Drafted,
    Built,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wish {
    pub id: String,
    pub name: String,
    pub words: String,
    pub by: String,
    pub status: Status,
    #[serde(default)]
    pub price: Spoils,
    #[serde(default)]
    pub art: Vec<String>,
    #[serde(default)]
    pub near: String,
    #[serde(default)]
    pub paid: Vec<(String, Spoils)>,
    #[serde(default)]
    pub raised: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot: Option<(i32, i32)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Haul {
    pub hero: u32,
    pub spoils: Spoils,
    pub why: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Avatar {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<appearance::Animation>,
    pub sprite: Vec<u8>,
    pub portrait: Vec<u8>,
    pub colour: [u8; 3],
    pub pixels: Vec<[u8; 4]>,
}

pub mod appearance;
pub mod effects;
impl Avatar {
    pub fn pixels_at(&self, tick: u64) -> &[[u8; 4]] {
        self.animation
            .as_ref()
            .and_then(|animation| animation.pixels(tick))
            .unwrap_or(&self.pixels)
    }
}

#[cfg(all(test, feature = "bend-cases-world"))]
#[path = "../tests/rules/realm_bend_cases__tests.rs"]
mod realm_bend_cases_tests;
