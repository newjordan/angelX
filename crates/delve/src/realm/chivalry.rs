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

/// How a mount rides at the lists. Ticks are the Delve's (30 a second).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Steed {
    /// Ticks from the spur to the meeting at the middle of the tilt.
    pub charge: u32,
    /// How many knocks the rider takes before falling.
    pub balance: u32,
    /// How far either side of the meeting a strike still lands, in ticks.
    pub strike: u32,
    /// How long, in ticks, the horse holds its rider steady at the clash:
    /// with the strike, how long the lances stay crossed.
    pub brace: u32,
    /// How long before the meeting the rival's guard can be read, in ticks.
    pub tell: u32,
    /// Extra knocks a clean hit deals.
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
    /// What the mount is like under a rider, in a few words.
    pub fn says(self) -> &'static str {
        match self {
            Self::Bramble => "steady: hard to unhorse, slow to the meeting",
            Self::Cinder => "fast and heavy: hits harder, less time",
            Self::Mist => "light and true: a wide strike, an early read",
        }
    }
    /// The numbers behind `says`: a mount changes the joust's timing, not a
    /// score on a menu.
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

/// A tended mount carries its rider one knock longer.
pub const TENDED_BALANCE: u32 = 1;

/// The stable as the realm keeps it: the mount saddled for the lists, which
/// are tended (brushed, watered, tack checked: one bout's worth), and what
/// the realm's knights have done at the lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stable {
    #[serde(default)]
    pub selected: Mount,
    #[serde(default)]
    pub tended: [bool; 3],
    /// Bouts won, by rival id.
    #[serde(default)]
    pub wins: BTreeMap<String, u32>,
    #[serde(default)]
    pub bouts: u32,
    /// Rivals put on the sand.
    #[serde(default)]
    pub unhorsed: u32,
    /// The herd: horses owned, traded and bred, beyond the three mounts.
    #[serde(default)]
    pub herd: Vec<Horse>,
    /// Herd selection is independent of the three jousting mounts.
    #[serde(default)]
    pub selected_herd: Option<usize>,
    /// Calls served under each house's banner: the usage bloodlines are
    /// seasoned by, and horse lore is told from.
    #[serde(default)]
    pub yards: BTreeMap<String, u32>,
    /// Foals bred in this stable, to vary their naming and mutation.
    #[serde(default)]
    pub foals: u32,
}

impl Stable {
    pub fn is_tended(&self, mount: Mount) -> bool {
        self.tended[mount.index()]
    }

    /// Service under a house's banner, tallied in calls served. This is the
    /// usage a bloodline's lore is earned from: a house whose models work
    /// seasons its horses, and an idle house's line goes soft.
    pub fn note_service(&mut self, house: &str, calls: u32) {
        if calls > 0 && !house.is_empty() {
            *self.yards.entry(house.to_string()).or_default() += calls;
        }
    }

    /// How seasoned a house's yard is, 0 (never served) to 4 (a great house
    /// of the road). One tier per 250 calls under the banner.
    pub fn yard_tier(&self, house: &str) -> u8 {
        let calls = self.yards.get(house).copied().unwrap_or(0);
        (calls / 250).min(4) as u8
    }

    /// The horse market on a given day: four horses whose blood is seasoned
    /// by how much their houses have actually served. Pure in the seed.
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

    /// Today's market: the four horses on offer, by the yard as it stands.
    /// The stables' pens and the yard's ledger show the same four.
    pub fn todays_market(&self) -> Vec<Horse> {
        let seed = self.yards.values().map(|&n| u64::from(n)).sum::<u64>()
            ^ (self.herd.len() as u64) << 8
            ^ u64::from(self.foals) << 16;
        self.market(seed)
    }

    /// Buy a market horse for gold, if the purse and the yard allow it.
    /// Returns what was spent, or `None` and nothing changes.
    pub fn buy(&mut self, horse: Horse, gold: u32) -> Option<u32> {
        let price = horse.value(self);
        (gold >= price && self.herd.len() < 12 && !horse.name.eq_ignore_ascii_case("Helios")).then(|| {
            self.herd.push(horse);
            price
        })
    }

    /// Sell a herd horse back to the road at seven-tenths of its value.
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

    /// Breed two herd horses. The foal carries the mean of its parents'
    /// blood, seasoned by the busier parent's house, and is named from
    /// both. The stable keeps at most twelve horses.
    pub fn breed(&mut self, sire: usize, dam: usize, seed: u64) -> Option<&Horse> {
        if sire == dam || sire >= self.herd.len() || dam >= self.herd.len() || self.herd.len() >= 12
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
            mare: seed % 2 == 0,
            ..Horse::default()
        });
        self.herd.last()
    }
}

/// The houses whose yards sell at the realm's horse market.
pub const MARKET_HOUSES: [&str; 4] = ["qwen", "grok", "glm", "deepseek"];

/// A house's gift, in the blood: what its models are like, as horseflesh.
/// Deterministic in the house key, so every realm agrees on a line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bloodline {
    pub pace: u8,
    pub stamina: u8,
    pub corner: u8,
    pub nerve: u8,
}

impl Bloodline {
    /// Seasoned blood runs truer: service under the banner lifts the low
    /// gifts first, which is why a worked line feels more honest.
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
        let up = seed / 4 % 2 == 0;
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

/// The blood a house breeds, from its key alone (3..=9 in each gift).
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

/// One horse of the realm: its blood, its house, and what the road and the
/// hippodrome have made of it. The lore is read off this record, never
/// written beside it, so a horse is exactly what it has done.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Horse {
    pub name: String,
    /// The model house in whose yard it was foaled (a key in `houses`).
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
    /// Gold won at the hippodrome.
    #[serde(default)]
    pub earned: u32,
    /// Older herd records predate sex; they remain eligible mares.
    #[serde(default = "legacy_mare")]
    pub mare: bool,
    /// Delves won at birth, for horses growing in the flower fields.
    #[serde(default)]
    pub born: Option<u32>,
}

fn legacy_mare() -> bool { true }

impl Horse {
    fn stock(seed: u64, house: &str, blood: Bloodline) -> Self {
        Self {
            name: foal_name(seed, house, ""),
            house: house.into(),
            blood,
            mare: seed % 2 == 0,
            ..Self::default()
        }
    }

    pub fn raceable(&self, wins: u32) -> bool {
        !self.name.eq_ignore_ascii_case("Helios")
            && self.born.is_none_or(|born| wins.saturating_sub(born) >= crate::rules::ranch::GROWN)
    }

    /// What the horse is worth in gold: blood, record, and a yard made
    /// honest by service all raise the price; a green horse is cheap.
    pub fn value(&self, stable: &Stable) -> u32 {
        40 + self.blood.sum() * 6
            + self.wins * 60
            + self.seconds * 25
            + self.starts * 4
            + u32::from(stable.yard_tier(&self.house)) * 15
    }

    /// The horse's story, told from its record and its house's service.
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

/// Decode a saved subtree on its own: a malformed or newer one resets to
/// its default instead of failing the whole realm (`Realm::beside` would
/// otherwise start the realm over).
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
