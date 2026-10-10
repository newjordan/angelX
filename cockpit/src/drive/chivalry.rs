//! Horsemanship: the realm's three mounts, the stable that keeps them, and
//! what a bout at the lists has made of the realm's knights.
//!
//! The stables and the lists are rooms of the world (see
//! `together_shooter::world`); the joust itself is played in the run
//! (`together_shooter::joust`). This module holds only what the realm keeps:
//! which mount is saddled, which are tended, and the record of bouts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mount {
    #[default]
    Bramble,
    Cinder,
    Mist,
}

/// How a mount rides at the lists. Ticks are the Delve's (30 a second).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Steed {
    /// Ticks from the spur to the meeting at the middle of the tilt.
    pub(crate) charge: u32,
    /// How many knocks the rider takes before falling.
    pub(crate) balance: u32,
    /// How far either side of the meeting a strike still lands, in ticks.
    pub(crate) strike: u32,
    /// How far either side of the meeting a brace still holds, in ticks.
    pub(crate) brace: u32,
    /// How long before the meeting the rival's guard can be read, in ticks.
    pub(crate) tell: u32,
    /// Extra knocks a clean hit deals.
    pub(crate) weight: u32,
}

impl Mount {
    pub(crate) const ALL: [Self; 3] = [Self::Bramble, Self::Cinder, Self::Mist];
    pub(crate) fn index(self) -> usize {
        self as usize
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Bramble => "Bramble",
            Self::Cinder => "Cinder",
            Self::Mist => "Mist",
        }
    }
    /// What the mount is like under a rider, in a few words.
    pub(crate) fn says(self) -> &'static str {
        match self {
            Self::Bramble => "steady: hard to unhorse, a wide brace",
            Self::Cinder => "fast and heavy: hits harder, less time",
            Self::Mist => "light and true: a wide strike, an early read",
        }
    }
    /// The numbers behind `says`: a mount changes the joust's timing, not a
    /// score on a menu.
    pub(crate) fn steed(self) -> Steed {
        match self {
            Self::Bramble => Steed {
                charge: 84,
                balance: 5,
                strike: 7,
                brace: 12,
                tell: 36,
                weight: 0,
            },
            Self::Cinder => Steed {
                charge: 63,
                balance: 4,
                strike: 6,
                brace: 8,
                tell: 30,
                weight: 1,
            },
            Self::Mist => Steed {
                charge: 75,
                balance: 4,
                strike: 10,
                brace: 8,
                tell: 54,
                weight: 0,
            },
        }
    }
    pub(crate) fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }
}

/// A tended mount carries its rider one knock longer.
pub(crate) const TENDED_BALANCE: u32 = 1;

/// The stable as the realm keeps it: the mount saddled for the lists, which
/// are tended (brushed, watered, tack checked: one bout's worth), and what
/// the realm's knights have done at the lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct Stable {
    #[serde(default)]
    pub(crate) selected: Mount,
    #[serde(default)]
    pub(crate) tended: [bool; 3],
    /// Bouts won, by rival id.
    #[serde(default)]
    pub(crate) wins: BTreeMap<String, u32>,
    #[serde(default)]
    pub(crate) bouts: u32,
    /// Rivals put on the sand.
    #[serde(default)]
    pub(crate) unhorsed: u32,
}

impl Stable {
    pub(crate) fn is_tended(&self, mount: Mount) -> bool {
        self.tended[mount.index()]
    }
}

/// The two places of horsemanship in the world, by the names commands use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Place {
    Stables,
    Tournament,
}

impl Place {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "stable" | "stables" => Some(Self::Stables),
            "tournament" | "knights" | "lists" | "joust" => Some(Self::Tournament),
            _ => None,
        }
    }
    pub(crate) fn label(self) -> &'static str {
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
pub(crate) struct Chivalry {
    pub(crate) selected: Mount,
    pub(crate) tended: [bool; 3],
}

/// Decode a saved subtree on its own: a malformed or newer one resets to
/// its default instead of failing the whole realm (`Realm::beside` would
/// otherwise start the realm over).
pub(crate) fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
    Ok(serde_json::from_value::<T>(value).unwrap_or_default())
}

/// The old practice subtree, decoded leniently.
pub(crate) fn deserialize_saved<'de, D>(deserializer: D) -> Result<Chivalry, D::Error>
where
    D: serde::Deserializer<'de>,
{
    lenient(deserializer)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/drive/chivalry__tests.rs"]
mod tests;
