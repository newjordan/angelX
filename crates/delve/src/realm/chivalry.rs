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
pub enum Mount {
    #[default]
    Bramble,
    Cinder,
    Mist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Steed {
    pub charge: u32,
    pub balance: u32,
    pub strike: u32,
    pub brace: u32,
    pub tell: u32,
    pub weight: u32,
}

impl Mount {
    pub const ALL: [Self; 3] = [Self::Bramble, Self::Cinder, Self::Mist];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Bramble => "Bramble",
            Self::Cinder => "Cinder",
            Self::Mist => "Mist",
        }
    }
    pub fn says(self) -> &'static str {
        match self {
            Self::Bramble => "steady: hard to unhorse, slow to the meeting",
            Self::Cinder => "fast and heavy: hits harder, less time",
            Self::Mist => "light and true: a wide strike, an early read",
        }
    }
    pub fn steed(self) -> Steed {
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
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }
}

pub const TENDED_BALANCE: u32 = 1;

pub const HERD_CAP: usize = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stable {
    #[serde(default)]
    pub selected: Mount,
    #[serde(default)]
    pub tended: [bool; 3],
    #[serde(default)]
    pub wins: BTreeMap<String, u32>,
    #[serde(default)]
    pub bouts: u32,
    #[serde(default)]
    pub unhorsed: u32,
    #[serde(default)]
    pub herd: Vec<Horse>,
    #[serde(default)]
    pub selected_herd: Option<usize>,
    #[serde(default)]
    pub yards: BTreeMap<String, u32>,
    #[serde(default)]
    pub foals: u32,
}

impl Stable {
    pub fn is_tended(&self, mount: Mount) -> bool {
        self.tended[mount.index()]
    }

    pub fn note_service(&mut self, house: &str, calls: u32) {
        if calls > 0 && !house.is_empty() {
            *self.yards.entry(house.to_string()).or_default() += calls;
        }
    }

    pub fn yard_tier(&self, house: &str) -> u8 {
        let calls = self.yards.get(house).copied().unwrap_or(0);
        (calls / 250).min(4) as u8
    }

    pub fn market(&self, seed: u64) -> Vec<Horse> {
        MARKET_HOUSES
            .iter()
            .enumerate()
            .map(|(i, house)| {
                let mut blood = bloodline(house);
                blood.season(self.yard_tier(house));
                Horse::stock(seed ^ (i as u64 + 1) * 0x9E37, house, blood)
            })
            .collect()
    }

    pub fn todays_market(&self) -> Vec<Horse> {
        let seed = self.yards.values().map(|&n| u64::from(n)).sum::<u64>()
            ^ (self.herd.len() as u64) << 8
            ^ u64::from(self.foals) << 16;
        self.market(seed)
    }

    pub fn buy(&mut self, horse: Horse, gold: u32) -> Option<u32> {
        let price = horse.value(self);
        (gold >= price && self.herd.len() < HERD_CAP && !horse.name.eq_ignore_ascii_case("Helios")).then(|| {
            self.herd.push(horse);
            price
        })
    }

    pub fn sell(&mut self, at: usize) -> Option<u32> {
        (at < self.herd.len()).then(|| {
            self.selected_herd = self.selected_herd.and_then(|i| {
                if i == at { None } else { Some(if i > at { i - 1 } else { i }) }
            });
            self.herd.remove(at).value(self) * 7 / 10
        })
    }

    pub fn mare(&self, wins: u32) -> Option<&Horse> {
        self.selected_herd.and_then(|i| self.herd.get(i))
            .filter(|h| h.mare && h.raceable(wins))
            .or_else(|| self.herd.iter().find(|h| h.mare && h.raceable(wins)))
    }

    pub fn breed(&mut self, sire: usize, dam: usize, seed: u64) -> Option<&Horse> {
        if sire == dam || sire >= self.herd.len() || dam >= self.herd.len() || self.herd.len() >= HERD_CAP
        {
            return None;
        }
        let (s, d) = (self.herd[sire].clone(), self.herd[dam].clone());
        let mut blood = Bloodline {
            pace: (u16::from(s.blood.pace) + u16::from(d.blood.pace)).div_ceil(2) as u8,
            stamina: (u16::from(s.blood.stamina) + u16::from(d.blood.stamina)).div_ceil(2) as u8,
            corner: (u16::from(s.blood.corner) + u16::from(d.blood.corner)).div_ceil(2) as u8,
            nerve: (u16::from(s.blood.nerve) + u16::from(d.blood.nerve)).div_ceil(2) as u8,
        };
        blood.mutate(seed ^ self.foals as u64);
        blood.season(self.yard_tier(&s.house).max(self.yard_tier(&d.house)) / 2);
        self.foals += 1;
        self.herd.push(Horse {
            name: foal_name(seed, &s.name, &d.name),
            house: s.house.clone(),
            sire: s.name,
            dam: d.name,
            blood,
            mare: seed.is_multiple_of(2),
            ..Horse::default()
        });
        self.herd.last()
    }
}

pub const MARKET_HOUSES: [&str; 4] = ["qwen", "grok", "glm", "deepseek"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bloodline {
    pub pace: u8,
    pub stamina: u8,
    pub corner: u8,
    pub nerve: u8,
}

impl Bloodline {
    pub fn season(&mut self, tier: u8) {
        for gift in [
            &mut self.pace,
            &mut self.stamina,
            &mut self.corner,
            &mut self.nerve,
        ] {
            if *gift < 6 {
                *gift = (*gift + tier).min(9);
            }
        }
    }
    fn mutate(&mut self, seed: u64) {
        let gifts = [
            &mut self.pace,
            &mut self.stamina,
            &mut self.corner,
            &mut self.nerve,
        ];
        let at = (seed % 4) as usize;
        let up = (seed / 4).is_multiple_of(2);
        for (i, gift) in gifts.into_iter().enumerate() {
            if i == at {
                *gift = if up {
                    (*gift + 1).min(9)
                } else {
                    gift.saturating_sub(1).max(3)
                };
            }
        }
    }
    pub fn sum(self) -> u32 {
        u32::from(self.pace + self.stamina + self.corner + self.nerve)
    }
}

pub fn bloodline(house: &str) -> Bloodline {
    let h = mix(house.bytes().fold(0xCBF2_9CE4_8422_2325u64, |a, b| {
        (a ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3)
    }));
    let gift = |shift: u64| 3 + ((h >> shift) % 7) as u8;
    Bloodline {
        pace: gift(0),
        stamina: gift(8),
        corner: gift(16),
        nerve: gift(24),
    }
}

fn mix(mut h: u64) -> u64 {
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ h >> 31
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Horse {
    pub name: String,
    pub house: String,
    #[serde(default)]
    pub sire: String,
    #[serde(default)]
    pub dam: String,
    pub blood: Bloodline,
    #[serde(default)]
    pub starts: u32,
    #[serde(default)]
    pub wins: u32,
    #[serde(default)]
    pub seconds: u32,
    #[serde(default)]
    pub earned: u32,
    #[serde(default = "legacy_mare")]
    pub mare: bool,
    #[serde(default)]
    pub born: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coat: Option<HorseCoat>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HorseCoat {
    White,
}

fn legacy_mare() -> bool { true }

impl Horse {
    fn stock(seed: u64, house: &str, blood: Bloodline) -> Self {
        Self {
            name: foal_name(seed, house, ""),
            house: house.into(),
            blood,
            mare: seed.is_multiple_of(2),
            ..Self::default()
        }
    }

    pub fn raceable(&self, wins: u32) -> bool {
        !self.name.eq_ignore_ascii_case("Helios")
            && self.born.is_none_or(|born| wins.saturating_sub(born) >= crate::rules::ranch::GROWN)
    }

    pub fn value(&self, stable: &Stable) -> u32 {
        40 + self.blood.sum() * 6
            + self.wins * 60
            + self.seconds * 25
            + self.starts * 4
            + u32::from(stable.yard_tier(&self.house)) * 15
    }

    pub fn lore(&self, stable: &Stable) -> String {
        let calls = stable.yards.get(&self.house).copied().unwrap_or(0);
        let bred = if self.sire.is_empty() {
            format!("foaled in the {} yard", self.house)
        } else {
            format!(
                "by {} out of {}, of the {} yard",
                self.sire, self.dam, self.house
            )
        };
        let road = if calls == 0 {
            "Its house has yet to serve, and the line is unproven.".into()
        } else {
            format!(
                "Its house has answered {calls} calls under its banner, and the road knows the name."
            )
        };
        let record = if self.starts == 0 {
            "Never yet put to a race.".into()
        } else {
            format!(
                "{} wins from {} starts, {} gold won.",
                self.wins, self.starts, self.earned
            )
        };
        format!("{}, {bred}. {road} {record}", self.name)
    }
}

const NAME_HEAD: [&str; 8] = [
    "Ash", "Bolt", "Cinder", "Dusk", "Ember", "Fleet", "Gale", "Hollow",
];
const NAME_TAIL: [&str; 8] = [
    "fall", "runner", "mane", "hoof", "song", "stride", "veil", "watch",
];

fn foal_name(seed: u64, sire: &str, _dam: &str) -> String {
    let h = mix(seed ^ sire.bytes().map(u64::from).sum::<u64>());
    format!(
        "{}{}",
        NAME_HEAD[(h % 8) as usize],
        NAME_TAIL[((h >> 8) % 8) as usize]
    )
}

pub fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
    Ok(serde_json::from_value::<T>(value).unwrap_or_default())
}

#[cfg(test)]
#[path = "../../tests/realm/chivalry__tests.rs"]
mod tests;
