//! The Undercroft: the company's home under the Delve's gate.
//!
//! Every delve begins here, and every delve comes home here. It is one
//! screen of vaulted cellar: the Winding Stair down into the Delve in its
//! middle, behind a low parapet open to the south, and along its walls the
//! stations the party builds out with the spoils it carries up — Tobbin's
//! forge and rack, Old Blaise's hearth, the Chapel of Bonds, Wren's map
//! table. What is built stays built: the realm keeps it (`Realm::home`), and
//! the run carries a copy, so a friend's mirror sees the same cellar.
//!
//! A station's upgrade is either a *home card* dealt into every knight's
//! deck (the forge's edge, the hearth's warmth), so the card screen shows it
//! and the mirror carries it, or a gift handed out at the stair (the rack's
//! kit, the chapel's second wind, a landing further down).
//!
//! Buying is done in the world: stand on a station's engraved plate, read
//! its ledger, hold E. The run only asks (`orders`); the cockpit pays from
//! the realm's treasury and tells the run what now stands.

use super::*;
use crate::realm::{Spoil, Spoils};

pub const BUY_HOLD: u32 = HZ * 3 / 4;

pub fn hold_bar(buying: u32, hold: u32) -> String {
    let filled = (buying.min(hold) * 10 / hold.max(1)) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled))
}
pub const DESCEND_HOLD: u32 = HZ;
const NPC_REACH: f32 = 3.6;
const WIND_HP: u32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Station {
    Forge,
    Hearth,
    Rack,
    Chapel,
    Map,
    Wheel,
    Coffer,
    StallA,
    StallB,
    StallC,
    LessonA,
    LessonB,
    Wing,
    TapA,
    TapB,
    TapC,
    SongA,
    SongB,
    SongC,
    Hire,
}

impl Station {
    pub fn word(self) -> &'static str {
        match self {
            Station::Forge => "forge",
            Station::Hearth => "hearth",
            Station::Rack => "rack",
            Station::Chapel => "chapel",
            Station::Map => "map",
            Station::Wheel => "wheel",
            Station::Coffer => "coffer",
            Station::StallA | Station::StallB | Station::StallC => "stall",
            Station::LessonA | Station::LessonB => "lesson",
            Station::Wing => "wing",
            Station::TapA | Station::TapB | Station::TapC => "tap",
            Station::SongA | Station::SongB | Station::SongC => "song",
            Station::Hire => "hire",
        }
    }

    pub fn ladder(self) -> &'static Ladder {
        LADDERS
            .iter()
            .find(|l| l.station == self)
            .expect("every station has a ladder")
    }
}

pub struct Rung {
    pub price: &'static [(Spoil, u32)],
    pub says: &'static str,
    pub needs: u32,
}

pub struct Ladder {
    pub station: Station,
    pub name: &'static str,
    pub rungs: &'static [Rung],
}

use Spoil::{Bond, Bone, Ember, Gem, Gold, Ore, Scale, Wax};

pub const LADDERS: &[Ladder] = &[
    Ladder {
        station: Station::Forge,
        name: "Tobbin's Edge",
        rungs: &[
            Rung {
                price: &[(Gold, 150), (Ore, 4)],
                says: "+8% damage, all knights",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 300), (Ore, 8), (Gem, 2)],
                says: "+16% damage, all knights",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 500), (Ore, 12), (Gem, 4)],
                says: "+24% damage, all knights",
                needs: 2,
            },
            Rung {
                price: &[(Gold, 800), (Ember, 8), (Gem, 6)],
                says: "+32% damage, all knights",
                needs: 3,
            },
            Rung {
                price: &[(Gold, 1200), (Ember, 12), (Scale, 1)],
                says: "+40% damage, all knights",
                needs: 3,
            },
        ],
    },
    Ladder {
        station: Station::Hearth,
        name: "Blaise's Hearth",
        rungs: &[
            Rung {
                price: &[(Gold, 120), (Wax, 4)],
                says: "+10 health, all knights",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 250), (Wax, 8), (Bone, 6)],
                says: "+20 health, all knights",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 450), (Bone, 12), (Wax, 10)],
                says: "+30 health, all knights",
                needs: 2,
            },
            Rung {
                price: &[(Gold, 700), (Ember, 8), (Bond, 4)],
                says: "+40 health, all knights",
                needs: 3,
            },
        ],
    },
    Ladder {
        station: Station::Rack,
        name: "Tobbin's Rack",
        rungs: &[
            Rung {
                price: &[(Gold, 100), (Bone, 4)],
                says: "a potion for each knight",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 200), (Ore, 6)],
                says: "and a bomb more each",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 350), (Wax, 6), (Gem, 4)],
                says: "and a second potion",
                needs: 2,
            },
            Rung {
                price: &[(Gold, 600), (Ember, 6), (Bond, 2)],
                says: "and a thunder scroll",
                needs: 3,
            },
        ],
    },
    Ladder {
        station: Station::Chapel,
        name: "Chapel of Bonds",
        rungs: &[
            Rung {
                price: &[(Gold, 150), (Bond, 6)],
                says: "fallen rise once a delve",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 400), (Bond, 14), (Gem, 4)],
                says: "fallen rise once a floor",
                needs: 2,
            },
        ],
    },
    Ladder {
        station: Station::Map,
        name: "Wren's Landings",
        rungs: &[
            Rung {
                price: &[(Gold, 200), (Gem, 4)],
                says: "a landing at floor 2",
                needs: 2,
            },
            Rung {
                price: &[(Gold, 400), (Ember, 8)],
                says: "a landing at floor 3",
                needs: 3,
            },
            Rung {
                price: &[(Gold, 700), (Ember, 10), (Scale, 1)],
                says: "a landing at floor 4",
                needs: 4,
            },
            Rung {
                price: &[(Gold, 1000), (Gem, 10), (Scale, 2)],
                says: "a landing at floor 5",
                needs: 5,
            },
        ],
    },
    Ladder {
        station: Station::Coffer,
        name: "The Herald's Coffer",
        rungs: &[],
    },
    Ladder {
        station: Station::Wing,
        name: "The West Wing",
        rungs: &[Rung {
            price: &[(Gold, 300), (Ore, 20), (Bone, 10)],
            says: "dig out Maud's tavern",
            needs: 1,
        }],
    },
    Ladder {
        station: Station::Wheel,
        name: "Fortune's Wheel",
        rungs: &[
            Rung {
                price: &[(Gold, 250), (Gem, 4)],
                says: "Collapse and Glass Jaw",
                needs: 0,
            },
            Rung {
                price: &[(Gold, 500), (Ember, 6), (Bond, 2)],
                says: "Horde and Gauntlet",
                needs: 2,
            },
            Rung {
                price: &[(Gold, 800), (Gem, 6), (Scale, 1)],
                says: "Turbo, Rune Rush, All Random",
                needs: 3,
            },
            Rung {
                price: &[(Gold, 1200), (Gem, 8), (Scale, 2)],
                says: "Ironman, Hollows",
                needs: 4,
            },
        ],
    },
];

enum SavedHomeSquires {
    Yard(super::squires::Yard),
    Lodge(super::squires_lodge::Guild),
    Academy(super::squire_academy::Academy),
    Guild(super::squire_guild::Guild),
}
impl<'de> Deserialize<'de> for SavedHomeSquires {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("companies").is_some() {
            serde_json::from_value(value).map(Self::Yard).map_err(serde::de::Error::custom)
        } else if value.get("clock").is_some() || value.get("revision").is_some() || value.get("hired").is_some() {
            serde_json::from_value(value).map(Self::Guild).map_err(serde::de::Error::custom)
        } else if value.get("departures").is_some() || value.get("paid").is_some() {
            serde_json::from_value(value).map(Self::Academy).map_err(serde::de::Error::custom)
        } else if value.get("rosters").is_some() || value.get("recruiting").is_some() {
            serde_json::from_value(value).map(Self::Lodge).map_err(serde::de::Error::custom)
        } else {
            serde_json::from_value(value).map(Self::Academy).map_err(serde::de::Error::custom)
        }
    }
}
impl Default for SavedHomeSquires {
    fn default() -> Self { Self::Yard(Default::default()) }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Home {
    #[serde(default, skip_serializing_if = "crate::kingdom::Kingdom::is_empty")]
    pub industry: crate::kingdom::Kingdom,
    #[serde(default, skip_serializing_if = "super::kingdom_gear::Ledger::is_empty")]
    pub kingdom_gear: super::kingdom_gear::Ledger,
    #[serde(default, skip_serializing_if = "super::stockyard::Stockyard::is_empty")]
    pub stockyard: super::stockyard::Stockyard,
    #[serde(default)]
    pub squires: super::squires::Yard,
    #[serde(default, skip_serializing_if = "super::lane_squires::Company::is_default")]
    pub lane_squires: super::lane_squires::Company,
    #[serde(default, skip_serializing_if = "super::squire_company::Company::is_empty")]
    pub squire_company: super::squire_company::Company,
    #[serde(default, skip_serializing_if = "crate::realm::squires::Company::is_empty")]
    pub squire_hall: crate::realm::squires::Company,
    #[serde(default)]
    pub squire_academy: super::squire_academy::Academy,
    #[serde(default)]
    pub squire_guild: super::squire_guild::Guild,
    #[serde(default, skip_serializing_if = "super::squires_lodge::Guild::is_empty")]
    pub squires_lodge: super::squires_lodge::Guild,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub siege_campaign_begun: bool,
    #[serde(default, skip_serializing_if = "super::roundtable::unsaved")]
    pub roundtable: [u32; 5],
    #[serde(default = "super::gear::legacy_wardrobe", deserialize_with = "crate::realm::chivalry::lenient")]
    pub wardrobe: super::gear::Wardrobe,
    #[serde(default)]
    pub privy_wall: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "super::freedom::Wished::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    pub wished: super::freedom::Wished,
    #[serde(
        default,
        skip_serializing_if = "super::rewards::Sky::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    pub sky: super::rewards::Sky,
    #[serde(default)]
    pub college: super::college::College,
    #[serde(default)]
    pub levels: BTreeMap<Station, u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raised: Vec<super::raisings::Raised>,
    #[serde(
        default,
        skip_serializing_if = "super::world::bomrekedan::works::Works::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    pub bomrekedan: super::world::bomrekedan::works::Works,
    #[serde(default)]
    pub deepest: u32,
    #[serde(default)]
    pub landing: u32,
    #[serde(default)]
    pub feats: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub boxes: Vec<super::feats::Tier>,
    #[serde(default)]
    pub opened: u32,
    #[serde(default)]
    pub goblins: u32,
    #[serde(default)]
    pub trophies: BTreeMap<String, u32>,
    #[serde(default)]
    pub bestiary: BTreeMap<String, u32>,
    #[serde(default)]
    pub residents: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub knights: BTreeMap<String, super::talents::Prowess>,
    #[serde(default)]
    pub questlines: super::questline::Progress,
    #[serde(default)]
    pub bounties: Vec<super::bounties::Pinned>,
    #[serde(default)]
    pub bounties_paid: BTreeMap<String, u32>,
    #[serde(default)]
    pub raids: Vec<String>,
    #[serde(default)]
    pub round: Option<String>,
    #[serde(default)]
    pub song: Option<String>,
    #[serde(default)]
    pub hire: Option<String>,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub stable: crate::realm::chivalry::Stable,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub ranch: super::ranch::Ranch,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub barony: super::barony::Barony,
    #[serde(default, skip_serializing_if = "super::goods::Goods::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub goods: super::goods::Goods,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub holdings: super::holdings::Holdings,
    #[serde(
        default,
        skip_serializing_if = "super::salvage::Salvage::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    pub salvage: super::salvage::Salvage,
    #[serde(default, deserialize_with = "super::countryside::deserialize")]
    pub countryside: super::countryside::Country,
    #[serde(default, skip_serializing_if = "super::deep::Chronicle::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub deep: super::deep::Chronicle,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub recruit_ults: super::recruit_ults::RecruitUlts,
    #[serde(default, skip_serializing_if = "super::rite::Rites::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub rites: super::rite::Rites,
    #[serde(default, skip_serializing_if = "super::wayfarers::Ledger::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub wayfarers: super::wayfarers::Ledger,
    #[serde(default, skip_serializing_if = "super::trades::Ledger::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub trades: super::trades::Ledger,
    #[serde(default, skip_serializing_if = "u64_is_zero")]
    pub cave_seed: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mines: BTreeMap<u8, super::caves::mines::SunkMine>,
    #[serde(skip)]
    pub delve_paid: bool,
}

fn u64_is_zero(n: &u64) -> bool {
    *n == 0
}

#[derive(Deserialize)]
struct SavedHome {
    #[serde(default)]
    industry: crate::kingdom::Kingdom,
    #[serde(default)]
    kingdom_gear: super::kingdom_gear::Ledger,
    #[serde(default)]
    stockyard: super::stockyard::Stockyard,
    #[serde(default)]
    squires_lodge: super::squires_lodge::Guild,
    #[serde(default)]
    squire_guild: super::squire_guild::Guild,
    #[serde(default)]
    squires: SavedHomeSquires,
    #[serde(default, skip_serializing_if = "super::lane_squires::Company::is_default")]
    pub lane_squires: super::lane_squires::Company,
    #[serde(default, skip_serializing_if = "super::squire_company::Company::is_empty")]
    pub squire_company: super::squire_company::Company,
    #[serde(default, skip_serializing_if = "crate::realm::squires::Company::is_empty")]
    pub squire_hall: crate::realm::squires::Company,
    #[serde(default)]
    pub squire_academy: super::squire_academy::Academy,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub siege_campaign_begun: bool,
    #[serde(default, skip_serializing_if = "super::roundtable::unsaved")]
    pub roundtable: [u32; 5],
    #[serde(default = "super::gear::legacy_wardrobe", deserialize_with = "crate::realm::chivalry::lenient")]
    pub wardrobe: super::gear::Wardrobe,
    #[serde(default)]
    pub privy_wall: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "super::freedom::Wished::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    pub wished: super::freedom::Wished,
    #[serde(
        default,
        skip_serializing_if = "super::rewards::Sky::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    pub sky: super::rewards::Sky,
    #[serde(default)]
    pub college: super::college::College,
    #[serde(default)]
    pub levels: BTreeMap<Station, u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    raised: Vec<super::raisings::Raised>,
    #[serde(
        default,
        skip_serializing_if = "super::world::bomrekedan::works::Works::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    bomrekedan: super::world::bomrekedan::works::Works,
    #[serde(default)]
    pub deepest: u32,
    #[serde(default)]
    pub landing: u32,
    #[serde(default)]
    pub feats: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub boxes: Vec<super::feats::Tier>,
    #[serde(default)]
    pub opened: u32,
    #[serde(default)]
    pub goblins: u32,
    #[serde(default)]
    pub trophies: BTreeMap<String, u32>,
    #[serde(default)]
    pub bestiary: BTreeMap<String, u32>,
    #[serde(default)]
    pub residents: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub knights: BTreeMap<String, super::talents::Prowess>,
    #[serde(default)]
    pub questlines: super::questline::Progress,
    #[serde(default)]
    pub bounties: Vec<super::bounties::Pinned>,
    #[serde(default)]
    pub bounties_paid: BTreeMap<String, u32>,
    #[serde(default)]
    pub raids: Vec<String>,
    #[serde(default)]
    pub round: Option<String>,
    #[serde(default)]
    pub song: Option<String>,
    #[serde(default)]
    pub hire: Option<String>,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub stable: crate::realm::chivalry::Stable,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub ranch: super::ranch::Ranch,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub barony: super::barony::Barony,
    #[serde(default, skip_serializing_if = "super::goods::Goods::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub goods: super::goods::Goods,
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub holdings: super::holdings::Holdings,
    #[serde(
        default,
        skip_serializing_if = "super::salvage::Salvage::is_empty",
        deserialize_with = "crate::realm::chivalry::lenient"
    )]
    pub salvage: super::salvage::Salvage,
    #[serde(default, deserialize_with = "super::countryside::deserialize")]
    pub countryside: super::countryside::Country,
    #[serde(default, skip_serializing_if = "super::deep::Chronicle::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub deep: super::deep::Chronicle,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub recruit_ults: super::recruit_ults::RecruitUlts,
    #[serde(default, skip_serializing_if = "super::rite::Rites::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub rites: super::rite::Rites,
    #[serde(default, skip_serializing_if = "super::wayfarers::Ledger::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub wayfarers: super::wayfarers::Ledger,
    #[serde(default, skip_serializing_if = "super::trades::Ledger::is_empty", deserialize_with = "crate::realm::chivalry::lenient")]
    pub trades: super::trades::Ledger,
    #[serde(default, skip_serializing_if = "u64_is_zero")]
    pub cave_seed: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mines: BTreeMap<u8, super::caves::mines::SunkMine>,
}

impl Home {
    pub(crate) fn keep_cave(
        &mut self,
        seed: u64,
        mines: &BTreeMap<u8, super::caves::mines::SunkMine>,
    ) {
        if self.cave_seed == 0 {
            self.cave_seed = seed;
        }
        for (key, mine) in mines {
            self.mines.entry(*key).or_insert_with(|| mine.clone());
        }
    }
}

impl<'de> Deserialize<'de> for Home {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let SavedHome { industry, kingdom_gear, stockyard, raised, bomrekedan, mut squires_lodge, squires, mut squire_guild, mut squire_academy, lane_squires, squire_company, squire_hall, siege_campaign_begun, roundtable, wardrobe, privy_wall, wished, sky, college, levels, deepest, landing, feats, boxes, opened, goblins, trophies, bestiary, residents, knights, questlines, bounties, bounties_paid, raids, round, song, hire, stable, ranch, barony, goods, holdings, salvage, countryside, deep, recruit_ults, rites, wayfarers, trades, cave_seed, mines } = SavedHome::deserialize(deserializer)?;
        let squires = match squires {
            SavedHomeSquires::Guild(guild) => { squire_guild = guild; Default::default() },
            SavedHomeSquires::Yard(yard) => yard,
            SavedHomeSquires::Lodge(lodge) => { if squires_lodge.is_empty() { squires_lodge = lodge; } Default::default() },
            SavedHomeSquires::Academy(academy) => {
                if squire_academy == Default::default() { squire_academy = academy; }
                Default::default()
            }
        };
        Ok(Self { industry, kingdom_gear, stockyard, raised, bomrekedan, squires_lodge, squires, squire_guild, squire_academy, lane_squires, squire_company, squire_hall, siege_campaign_begun, roundtable, wardrobe, privy_wall, wished, sky, college, levels, deepest, landing, feats, boxes, opened, goblins, trophies, bestiary, residents, knights, questlines, bounties, bounties_paid, raids, round, song, hire, stable, ranch, barony, goods, holdings, salvage, countryside, deep, recruit_ults, rites, wayfarers, trades, cave_seed, mines, delve_paid: false })
    }
}

pub fn price(rung: &Rung) -> Spoils {
    let mut spoils = Spoils::default();
    for &(spoil, n) in rung.price {
        spoils.add(spoil, n);
    }
    spoils
}

pub fn numeral(level: u8) -> &'static str {
    ["", "I", "II", "III", "IV", "V", "VI"][usize::from(level).min(6)]
}

impl Home {
    pub fn profile_key(hero: &Hero) -> String {
        super::talents::profile_key(hero)
    }

    pub fn adopt(&mut self, kit: &str, player: &str) -> bool {
        if kit == player
            || self.knights.contains_key(player)
            || self.questlines.knights.contains_key(player)
        {
            return false;
        }
        let prowess = self.knights.remove(kit);
        let walks = self.questlines.knights.remove(kit);
        let found = prowess.is_some() || walks.is_some();
        if let Some(prowess) = prowess {
            self.knights.insert(player.to_owned(), prowess);
        }
        if let Some(walks) = walks {
            self.questlines.knights.insert(player.to_owned(), walks);
        }
        found
    }

    pub fn knight_key(hero: &Hero) -> String {
        super::talents::knight_key(hero)
    }

    pub fn level(&self, station: Station) -> u8 {
        self.levels.get(&station).copied().unwrap_or(0)
    }

    pub fn next(&self, station: Station) -> Option<(u8, &'static Rung)> {
        let level = self.level(station);
        station
            .ladder()
            .rungs
            .get(usize::from(level))
            .map(|rung| (level + 1, rung))
    }

    pub fn buy(&mut self, station: Station, treasury: &mut Spoils) -> Result<u8, String> {
        let ladder = station.ladder();
        let Some((level, rung)) = self.next(station) else {
            return Err(format!("{} is built in full", ladder.name));
        };
        if self.deepest < rung.needs {
            return Err(format!(
                "{} {} needs the party to have reached floor {} first",
                ladder.name,
                numeral(level),
                rung.needs
            ));
        }
        let cost = price(rung);
        if !treasury.covers(&cost) {
            return Err(format!(
                "{} {}: {}",
                ladder.name,
                numeral(level),
                treasury.shortfall(&cost)
            ));
        }
        treasury.take(&cost);
        self.levels.insert(station, level);
        Ok(level)
    }

    pub fn landings(&self) -> Vec<u32> {
        let mut landings: Vec<u32> = (1..=1 + u32::from(self.level(Station::Map))).collect();
        for w in &self.holdings.owned {
            if w.stands("waypoint")
                && let Some(super::holdings::Deed::Floor(pack)) = w.deed()
                && let Some(at) = super::holdings::FLOORS.iter().position(|&p| p == pack)
            {
                let depth = at as u32 + 1;
                if !landings.contains(&depth) {
                    landings.push(depth);
                }
            }
        }
        landings.extend(self.deep_landings());
        landings
    }

    pub fn landing(&self) -> u32 {
        let landings = self.landings();
        if landings.contains(&self.landing) {
            self.landing
        } else {
            1
        }
    }

    pub fn edge(&self) -> u32 {
        u32::from(self.level(Station::Forge)) * 8
    }

    pub fn warmth(&self) -> u32 {
        u32::from(self.level(Station::Hearth)) * 10
    }

    pub fn winds(&self) -> u8 {
        self.level(Station::Chapel).min(1)
    }
}

pub struct Spot {
    pub station: Station,
    pub furniture: (i32, i32, i32, i32),
    pub plate: (i32, i32, i32, i32),
    pub keeper: Option<(&'static str, (f32, f32))>,
}

pub const SPOTS: [Spot; 5] = [
    Spot {
        station: Station::Forge,
        furniture: (2, 1, 4, 2),
        plate: (3, 3, 2, 1),
        keeper: Some(("tobbin", (6.9, 3.0))),
    },
    Spot {
        station: Station::Map,
        furniture: (18, 1, 4, 2),
        plate: (19, 3, 2, 1),
        keeper: Some(("wren", (17.1, 3.0))),
    },
    Spot {
        station: Station::Hearth,
        furniture: (2, 11, 4, 2),
        plate: (3, 10, 2, 1),
        keeper: Some(("blaise", (6.9, 12.2))),
    },
    Spot {
        station: Station::Rack,
        furniture: (18, 11, 4, 2),
        plate: (19, 10, 2, 1),
        keeper: None,
    },
    Spot {
        station: Station::Chapel,
        furniture: (1, 5, 1, 4),
        plate: (2, 6, 1, 2),
        keeper: None,
    },
];

pub const BOUNTY_BOARD: (i32, i32, i32, i32) = (15, 9, 3, 1);

pub const STAIRWELL: (i32, i32, i32, i32) = (10, 5, 4, 4);
pub const STAIR_MOUTH: (i32, i32, i32, i32) = (11, 8, 2, 1);

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

pub fn plate_at(kind: RoomKind, x: f32, y: f32) -> Option<Station> {
    match kind {
        RoomKind::Home => SPOTS
            .iter()
            .find(|spot| inside(spot.plate, x, y))
            .map(|spot| spot.station),
        RoomKind::Fortune => {
            if inside(super::fortune::FORTUNE_PLATE, x, y) {
                Some(Station::Wheel)
            } else {
                inside(super::fortune::COFFER_PLATE, x, y).then_some(Station::Coffer)
            }
        }
        RoomKind::Trophies => inside(super::tavern::WING_PLATE, x, y).then_some(Station::Wing),
        _ => None,
    }
}

#[cfg(any(test, feature = "testing"))]
pub fn keeper_at(who: &str) -> Option<(f32, f32)> {
    SPOTS.iter().find_map(|spot| {
        spot.keeper
            .filter(|(name, _)| *name == who)
            .map(|(_, (c, r))| (c * TILE_UNITS, r * TILE_UNITS))
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Order {
    pub knight: u32,
    pub station: Station,
}

impl Run {
    pub fn at_home(
        seed: u64,
        raid_id: u64,
        guest: Option<&str>,
        home: Home,
        treasury: Spoils,
    ) -> Run {
        let mut run = Run::new(seed, raid_id, guest);
        run.home = home;
        if run.home.wardrobe.legacy {
            run.home.wardrobe.knights.entry("You".into()).or_insert_with(super::gear::Kept::equipped);
            run.home.wardrobe.legacy = false;
        }
        super::ranch::migrate(&mut run.home.ranch, &mut run.home.stable);
        run.treasury = treasury;
        run.dungeon = layout::undercroft(run.dungeon.pack);
        run.boss_gates = None;
        if run.home.level(Station::Wing) >= 1 {
            layout::dig_tavern(&mut run.dungeon);
        }
        run.raise_home_world();
        run.cues = vec!["home".into()];
        run.stock_stall();
        run.enter(0, None);
        let ids: Vec<u32> = run.players.keys().copied().collect();
        for id in ids {
            run.restore_gear(id);
            run.deal_home(id);
        }
        run.stock_rites();
        run
    }

    pub fn enter_settlement(&mut self, site: super::exhibits::Admission<'_>) -> Result<(), String> {
        if !self.at_home_now() || self.at != 0 || self.tick != 0 || self.settlement_site.is_some() {
            return Err("settlement admission requires a new run in the PLAYER HALL".into());
        }
        self.dungeon = site.floor.clone();
        self.raise_home_world();
        self.boss_gates = None;
        self.settlement_site = Some(site.id.to_string());
        self.settlement_exhibits = site.exhibits;
        self.players.get_mut(&1).expect("host seat").name = site.player.to_string();
        self.enter(0, None);
        Ok(())
    }

    pub fn at_home_now(&self) -> bool {
        self.dungeon.depth == 0
    }

    pub fn rebuild_home(&mut self, home: Home, treasury: Spoils) {
        let landing = self.home.landing;
        let barony = std::mem::take(&mut self.home.barony);
        let salvage = std::mem::take(&mut self.home.salvage);
        let industry = std::mem::take(&mut self.home.industry);
        let kingdom_gear = std::mem::take(&mut self.home.kingdom_gear);
        let stockyard = std::mem::take(&mut self.home.stockyard);
        let rites = std::mem::take(&mut self.home.rites);
        let cave_seed = self.home.cave_seed;
    let mines = self.home.mines.clone();
    self.home = home;
    self.home.keep_cave(cave_seed, &mines);
        self.home.rites.merge(&rites);
        if industry.rev > self.home.industry.rev { self.home.industry = industry; }
        if kingdom_gear.rev > self.home.kingdom_gear.rev { self.home.kingdom_gear = kingdom_gear; }
        if stockyard.rev > self.home.stockyard.rev { self.home.stockyard = stockyard; }
        for hero in self.players.values() {
            self.home.adopt(&super::talents::knight_key(hero), &super::talents::profile_key(hero));
        }
        if salvage.rev > self.home.salvage.rev {
            self.home.salvage = salvage;
        }
        super::ranch::migrate(&mut self.home.ranch, &mut self.home.stable);
        self.barony_news(&barony);
        self.home.landing = landing;
        self.treasury = treasury;
        self.sync_squires();
        if self.at_home_now()
            && self.home.level(Station::Wing) >= 1
            && layout::dig_tavern(&mut self.dungeon)
        {
            self.cues.push("dug".into());
            self.fx.push(Fx::Shake(10));
            self.sounds.push("rock_land");
        }
        if self.at_home_now() {
            self.raise_home_world();
        }
        if self.stall.is_empty() && self.home.goblins > 0 && self.at_home_now() {
            self.stock_stall();
        }
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            self.deal_home(id);
        }
    }

    pub fn deal_home(&mut self, id: u32) {
        self.deal_college_spell(id);
        let mut cards = Vec::new();
        let edge = self.home.edge();
        if edge > 0 {
            let level = self.home.level(Station::Forge);
            cards.push(home_card(
                "home-edge",
                &format!("Tobbin's Edge {}", numeral(level)),
                "Honed at the Undercroft forge, by a smith with opinions.",
                &format!("damage {edge}"),
                EDGE_ART,
            ));
        }
        let hearth = self.home.warmth();
        if hearth > 0 {
            let level = self.home.level(Station::Hearth);
            cards.push(home_card(
                "home-hearth",
                &format!("Blaise's Hearth {}", numeral(level)),
                "Warmth from the Undercroft fire, carried down.",
                &format!("max_hp {hearth}"),
                HEARTH_ART,
            ));
        }
        let stew = if self.home.residents.contains("mabel") {
            STEW
        } else {
            0
        };
        if stew > 0 {
            cards.push(home_card(
                "home-stew",
                "Mabel's Stew",
                "You'll eat it. It's good for you. She'll know if you don't.",
                &format!("max_hp {stew}"),
                STEW_ART,
            ));
        }
        if self.home.barony.is_lit("ore-forge") {
            cards.push(home_card(
                "home-mail",
                "Dwarf-Forged Mail",
                "Out of the Ore-Forge, relit. King Brannoc says it'll turn a troll. He has not tried.",
                &format!("armor {}", super::barony::FORGED_MAIL),
                MAIL_ART,
            ));
        }
        if self.round.as_deref() == Some("stout") {
            cards.push(home_card(
                "home-round",
                "Dragon's Breath Stout",
                "Maud's strongest. It's good for you. Mostly.",
                &format!("damage {}", super::tavern::STOUT),
                STEW_ART,
            ));
        }
        let warmth = hearth + stew;
        for card in &cards {
            self.book.insert(card.clone());
        }
        let Some(who) = self.players.get(&id).map(super::talents::profile_key) else {
            return;
        };
        let prowess = self.home.prowess(&who);
        let lessons: Vec<Card> = prowess
            .talents()
            .map(|(lesson, talent)| {
                home_card(
                    &format!("home-talent-{}", lesson + 1),
                    talent.name,
                    &format!(
                        "Sir Ector's lesson {}: {}",
                        numeral(lesson as u8 + 1),
                        talent.says
                    ),
                    talent.effect,
                    LESSON_ART,
                )
            })
            .collect();
        for card in &lessons {
            self.book.insert(card.clone());
        }
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.deck.retain(|c| !c.starts_with("home-"));
        hero.deck
            .extend(cards.iter().chain(&lessons).map(|c| c.id.clone()));
        hero.rebonus(&self.book);
        if warmth > hero.home_hp {
            let more = warmth - hero.home_hp;
            hero.max_hp += more;
            hero.hp = (hero.hp + more).min(hero.max_hp);
        } else if warmth < hero.home_hp {
            let less = hero.home_hp - warmth;
            hero.max_hp = hero.max_hp.saturating_sub(less).max(1);
            hero.hp = hero.hp.min(hero.max_hp);
        }
        hero.home_hp = warmth;
    }

    pub(super) fn kit_out(&mut self) {
        let rack = self.home.level(Station::Rack);
        let winds = self.home.winds();
        for hero in self.players.values_mut() {
            let mut gifts: Vec<&str> = Vec::new();
            if rack >= 1 {
                gifts.push("potion");
            }
            if rack >= 3 {
                gifts.push("potion");
            }
            if rack >= 4 {
                gifts.push("thunder-scroll");
            }
            for gift in gifts {
                if hero.hand.len() < cards::HAND {
                    hero.hand.push(gift.to_string());
                }
            }
            if rack >= 2 {
                hero.give_bombs(1);
            }
            if hero.laurel {
                hero.give_bombs(1);
            }
            hero.winds = winds;
        }
        self.freedom_kit_out();
    }

    pub(super) fn refill_winds(&mut self) {
        if self.mode == super::fortune::Mode::Ironman {
            return self.iron();
        }
        if self.home.level(Station::Chapel) >= 2 {
            for hero in self.players.values_mut() {
                hero.winds = hero.winds.max(1);
            }
        }
    }

    pub(super) fn second_winds(&mut self) {
        let mut rose = false;
        let mut talisman = false;
        for hero in self.players.values_mut() {
            if hero.hp == 0 && hero.talisman && !hero.stone {
                hero.talisman = false;
                hero.hp = hero.max_hp;
                hero.invulnerable = 2 * HZ;
                talisman = true;
                if self.blasts.len() < 16 {
                    self.blasts.push((hero.x, hero.y, BLAST_TICKS));
                }
                continue;
            }
            if hero.hp == 0 && hero.winds > 0 && !hero.stone {
                hero.winds -= 1;
                hero.hp = (hero.max_hp * WIND_HP / 100).max(1);
                hero.invulnerable = 2 * HZ;
                rose = true;
                if self.blasts.len() < 16 {
                    self.blasts.push((hero.x, hero.y, BLAST_TICKS));
                }
            }
        }
        if rose {
            self.cues.push("second_wind".into());
            self.sounds.push("wall_up");
            self.notice("not_yet");
        }
        if talisman {
            self.cues.push("talisman_used".into());
            self.sounds.push("wheel_land");
        }
    }

    pub(super) fn tick_home(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.tick_squire_company(inputs);
        self.tick_lane_squire_yard(inputs);
        if self.tick_home_room(inputs) {
            return;
        }
        let landings = self.home.landings();
        let mut asked = Vec::new();
        let mut turned = false;
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.interact);
            if !pressing {
                hero.buy_spent = false;
            }
            let standing = hero.hp > 0 && !hero.stone && hero.privy == 0;
            let on = plate_at(RoomKind::Home, hero.x, hero.y).filter(|_| standing);
            match on {
                Some(station) if pressing && !hero.buy_spent => {
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        asked.push(Order {
                            knight: id,
                            station,
                        });
                    }
                }
                _ => hero.buying = 0,
            }
            if standing && pressing && !hero.buy_spent && inside(STAIR_MOUTH, hero.x, hero.y) {
                hero.buy_spent = true;
                turned = true;
            }
        }
        if turned && landings.len() > 1 {
            let now = self.home.landing();
            let next = landings
                .iter()
                .copied()
                .find(|&l| l > now)
                .unwrap_or(landings[0]);
            self.home.landing = next;
            self.found = Some((
                self.tick,
                0,
                format!("The stair will let you off at floor {next}"),
            ));
            self.sounds.push("door_open");
        }
        if !asked.is_empty() {
            self.sounds.push("play_card");
        }
        self.orders.extend(asked);
        let mut near = self.greeted & (1 << 7);
        for (bit, spot) in SPOTS.iter().enumerate() {
            let Some((who, (c, r))) = spot.keeper else {
                continue;
            };
            let (kx, ky) = (c * TILE_UNITS, r * TILE_UNITS);
            let close = self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - kx).hypot(h.y - ky) < NPC_REACH);
            if close {
                near |= 1 << bit;
                if self.greeted & (1 << bit) == 0 {
                    self.cues.push(format!("npc:{who}"));
                }
            }
        }
        near |= self.greet_merlin();
        self.greeted = near;
        self.greet_residents();
    }
}

fn home_card(id: &str, name: &str, text: &str, effect: &str, art: &str) -> Card {
    let raw = format!(
        "name {name}\nkind hold\nrarity relic\nby the Undercroft\ntext {text}\ndrop 0\nchest 0\n{effect}\nart\n{art}\n"
    );
    cards::check(id, &raw)
        .expect("home cards pass their own checker")
        .card
}

const MAIL_ART: &str = "\
...hhhhhh...
..hiHiiHih..
.hiiiiiiiih.
.hjhjhjhjhh.
.hjhjhjhjhh.
.hhjhjhjhjh.
..hjhjhjhh..
..hhjhjhjh..
..jjjjjjjj..
............";

const STEW: u32 = 15;

const STEW_ART: &str = "\
....h..h....
...h..h.....
....h..h....
............
.OOOOOOOOOO.
.ORrRrRrRrO.
..ORrRrRrO..
...OOOOOO...
....OOOO....
............
............";

const LESSON_ART: &str = "\
.....HH.....
.....HH.....
.....HH.....
...OOHHOO...
.....HH.....
.TTTTbbTTTT.
.TccTbbTccT.
.TccTbbTccT.
.TTTTbbTTTT.
..BBBBBBBB..
............";

const EDGE_ART: &str = "\
....HH......
...HiiH.....
....HH......
....bb......
....bb......
....bb......
..GGGGGGGG..
..JhhhhhhJ..
....GGGG....
....GGGG....
...GGGGGG...";

const HEARTH_ART: &str = "\
.....6......
....656.....
...65@56....
..6@7@7@6...
..6@777@6...
...@777@....
....@7@.....
..bBBpBBb...
.bBBpppBBb..";

#[cfg(test)]
#[path = "../../tests/rules/together_home__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/home_bend_cases__tests.rs"]
pub(crate) mod bend_cases;

