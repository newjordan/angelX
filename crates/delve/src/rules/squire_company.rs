//! The squires' company: kept by knight name in the realm, never by seat.
//! Each knight brings one companion; summoned minions keep their own places.

use super::*;

#[path = "squires/company.rs"]
pub mod company;
#[path = "squires/field.rs"]
pub mod field;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Ninja,
    Healer,
    Ranger,
    Shield,
    Alchemist,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ninja => "Ninja",
            Self::Healer => "Healer",
            Self::Ranger => "Ranger",
            Self::Shield => "Shield-bearer",
            Self::Alchemist => "Alchemist",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squire {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub level: u8,
    #[serde(default)]
    pub trained_on: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster {
    #[serde(default)]
    pub squires: Vec<Squire>,
    #[serde(default)]
    pub active: Option<u32>,
    #[serde(default)]
    pub claimed: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    #[serde(default)]
    pub board: u8,
    #[serde(default)]
    pub hall: u8,
    #[serde(default)]
    pub yard: u8,
    #[serde(default)]
    pub delves: u32,
    #[serde(default)]
    pub next_id: u32,
    #[serde(default)]
    pub rosters: BTreeMap<String, Roster>,
}

/// The actor uses the same movement, damage and grace as the old allies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Companion {
    pub squire: Squire,
    pub actor: hireling::Hireling,
    #[serde(default)]
    pub ability: u32,
    #[serde(default)]
    pub effect: u32,
    #[serde(default)]
    pub mark: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    #[serde(default)]
    pub companions: BTreeMap<u32, Companion>,
}

impl Company {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

impl Field {
    pub fn is_empty(&self) -> bool {
        self.companions.is_empty()
    }
}
