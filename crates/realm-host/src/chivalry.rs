//! Horsemanship as the cockpit keeps it: the commands' names for the two
//! places of horsemanship, and the practice game's old save. The mounts,
//! the stable and the horses are the Delve's (`angel_delve::realm::chivalry`).

pub use angel_delve::realm::chivalry::*;
use serde::{Deserialize, Serialize};

/// The two places of horsemanship in the world, by the names commands use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Stables,
    Tournament,
}

impl Place {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "stable" | "stables" => Some(Self::Stables),
            "tournament" | "knights" | "lists" | "joust" => Some(Self::Tournament),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Stables => "THE STABLES",
            Self::Tournament => "THE LISTS",
        }
    }
}

/// The practice game's old save: only the mount and the tending are kept,
/// and they move into the realm's stable once (`Realm::beside`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chivalry {
    pub selected: Mount,
    pub tended: [bool; 3],
}

/// The old practice subtree, decoded leniently.
pub fn deserialize_saved<'de, D>(deserializer: D) -> Result<Chivalry, D::Error>
where
    D: serde::Deserializer<'de>,
{
    lenient(deserializer)
}

#[cfg(test)]
#[path = "../tests/chivalry__tests.rs"]
mod tests;
