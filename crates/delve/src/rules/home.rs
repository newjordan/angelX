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

/// How long E is held on a plate to buy: long enough that a stray press in
/// passing buys nothing.
pub const BUY_HOLD: u32 = HZ * 3 / 4;

/// A hold's progress as ten cells of a bar, filled and still to fill, for a
/// plate's board and the sidebar alike.
pub fn hold_bar(buying: u32, hold: u32) -> String {
    let filled = (buying.min(hold) * 10 / hold.max(1)) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled))
}
/// How long a knight stands on the Winding Stair before the party goes down.
pub const DESCEND_HOLD: u32 = HZ;
/// How near an NPC a knight comes before they speak.
const NPC_REACH: f32 = 3.6;
/// What a fallen knight's second wind brings them back with, in percent.
const WIND_HP: u32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Station {
    /// Tobbin's forge: every knight's edge.
    Forge,
    /// Old Blaise's hearth: every knight's health.
    Hearth,
    /// Tobbin's rack: what every knight carries down.
    Rack,
    /// The Chapel of Bonds: a fallen knight's second wind.
    Chapel,
    /// Wren's map table: landings further down the Winding Stair.
    Map,
    /// Dame Fortune's wheel: more wedges, more ways down.
    Wheel,
    /// The Herald's coffer in Fortune's hall: the boxes achievements earn,
    /// opened before the audience.
    Coffer,
    /// Grubbins' stall in the Training Yard: its three wares.
    StallA,
    StallB,
    StallC,
    /// Sir Ector's lecterns in the Training Yard: the left talent, and the
    /// right.
    LessonA,
    LessonB,
    /// Tobbin's crew digs the west wing out of the Trophy Hall's wall:
    /// Maud's tavern.
    Wing,
    /// Maud's three taps: a round for the next delve.
    TapA,
    TapB,
    TapC,
    /// Sir Dinadan's three songs: an aura for the next delve.
    SongA,
    SongB,
    SongC,
    /// Beaumains' table: his wage, for the next delve.
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

/// One step up a station's ladder.
pub struct Rung {
    pub price: &'static [(Spoil, u32)],
    /// What it does, on the ledger.
    pub says: &'static str,
    /// The deepest floor the party must have reached first (0: none).
    pub needs: u32,
}

/// A station's upgrades, in order.
pub struct Ladder {
    pub station: Station,
    /// The ledger's title, before its numeral.
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
                says: "Ironman, Sponsors, Hollows",
                needs: 4,
            },
        ],
    },
];

/// The Undercroft as built: each station's level, the deepest floor the
/// party has reached, and the landing the stair takes them to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Home {
    #[serde(default = "super::gear::legacy_wardrobe", deserialize_with = "crate::realm::chivalry::lenient")]
    pub wardrobe: super::gear::Wardrobe,
    /// What knights have scratched on the privy wall, newest last; it stays
    /// from delve to delve (`pitstop::WALL`).
    #[serde(default)]
    pub privy_wall: Vec<String>,
    #[serde(default)]
    pub college: super::college::College,
    #[serde(default)]
    pub levels: BTreeMap<Station, u8>,
    #[serde(default)]
    pub deepest: u32,
    /// The floor the Winding Stair lets the party off at (0 and 1: the top).
    #[serde(default)]
    pub landing: u32,
    /// Achievements earned, each once; the boxes they brought, unopened;
    /// and how many boxes have been opened.
    #[serde(default)]
    pub feats: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub boxes: Vec<super::feats::Tier>,
    #[serde(default)]
    pub opened: u32,
    /// Loot goblins that got away: one is enough for Grubbins to come back
    /// and open his stall.
    #[serde(default)]
    pub goblins: u32,
    /// The realm's best show: the most viewers a delve has had, in
    /// thousands.
    #[serde(default)]
    pub best_show: u32,
    /// The Trophy Hall: how often the party has felled each guardian (by
    /// its id; the dragon is `dragon`) and found the Grail (`grail`).
    #[serde(default)]
    pub trophies: BTreeMap<String, u32>,
    /// The Herald's Bestiary: how many of each kind the realm has felled.
    #[serde(default)]
    pub bestiary: BTreeMap<String, u32>,
    /// The people rescued from the delve's cages, who live here now.
    #[serde(default)]
    pub residents: std::collections::BTreeSet<String>,
    /// Each knight's prowess, by knight: experience and the talents Sir
    /// Ector has taught them.
    #[serde(default)]
    pub knights: BTreeMap<String, super::talents::Prowess>,
    /// Wren's board: the bounties pinned to it, and how often she has paid
    /// each.
    #[serde(default)]
    pub bounties: Vec<super::bounties::Pinned>,
    #[serde(default)]
    pub bounties_paid: BTreeMap<String, u32>,
    /// The round Maud has poured for the next delve, by its id.
    #[serde(default)]
    pub round: Option<String>,
    /// The song Sir Dinadan will sing for the next delve, by its id.
    #[serde(default)]
    pub song: Option<String>,
    /// Who is hired for the next delve: Beaumains, or nobody.
    #[serde(default)]
    pub hire: Option<String>,
    /// The stables and the lists: the saddled mount, which are tended, the
    /// record of bouts. Decoded on its own, so a bad subtree never costs
    /// the realm.
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub stable: crate::realm::chivalry::Stable,
    /// The horse ranch: stud-book attempts and growth progress. Legacy
    /// foals migrate into the herd. Decoded on its own, like the stable.
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub ranch: super::ranch::Ranch,
    /// King Brannoc's barony: his court, the works paid for, his missions,
    /// the forges relit and the ledger. Decoded on its own, like the
    /// stable.
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub barony: super::barony::Barony,
    /// The realm's holdings: castles and cleared floors bought at the
    /// Court of Deeds, and the stores angelX's work keeps them with.
    /// Decoded on its own, like the stable.
    #[serde(default, deserialize_with = "crate::realm::chivalry::lenient")]
    pub holdings: super::holdings::Holdings,
    #[serde(default, deserialize_with = "super::countryside::deserialize")]
    pub countryside: super::countryside::Country,
}

/// A price as spoils.
pub fn price(rung: &Rung) -> Spoils {
    let mut spoils = Spoils::default();
    for &(spoil, n) in rung.price {
        spoils.add(spoil, n);
    }
    spoils
}

/// Roman numerals for a ledger's rungs.
pub fn numeral(level: u8) -> &'static str {
    ["", "I", "II", "III", "IV", "V", "VI"][usize::from(level).min(6)]
}

impl Home {
    pub fn level(&self, station: Station) -> u8 {
        self.levels.get(&station).copied().unwrap_or(0)
    }

    /// The next rung up a station's ladder, and its number, if any is left.
    pub fn next(&self, station: Station) -> Option<(u8, &'static Rung)> {
        let level = self.level(station);
        station
            .ladder()
            .rungs
            .get(usize::from(level))
            .map(|rung| (level + 1, rung))
    }

    /// Pay for a station's next rung out of `treasury`. The new level, or
    /// what stands in the way.
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

    /// The floors the stair can let the party off at: the top, and each
    /// landing Wren has drawn.
    pub fn landings(&self) -> Vec<u32> {
        (1..=1 + u32::from(self.level(Station::Map))).collect()
    }

    /// Where the stair goes now.
    pub fn landing(&self) -> u32 {
        let landings = self.landings();
        if landings.contains(&self.landing) {
            self.landing
        } else {
            1
        }
    }

    /// The forge's edge: percent more damage for every knight.
    pub fn edge(&self) -> u32 {
        u32::from(self.level(Station::Forge)) * 8
    }

    /// The hearth's warmth: more health for every knight.
    pub fn warmth(&self) -> u32 {
        u32::from(self.level(Station::Hearth)) * 10
    }

    /// Second winds each knight carries down: none, once a delve (1), or
    /// once a floor (refilled at every stair).
    pub fn winds(&self) -> u8 {
        self.level(Station::Chapel).min(1)
    }
}

/// Where a station stands in the Undercroft, in tiles: its furniture (which
/// stops feet and shots), its engraved plate (which a knight stands on to
/// buy), and its keeper.
pub struct Spot {
    pub station: Station,
    /// Furniture: column, row, width, height in tiles.
    pub furniture: (i32, i32, i32, i32),
    /// The plate: column, row, width, height in tiles.
    pub plate: (i32, i32, i32, i32),
    /// The keeper, if one stands here: who, and where (tile coordinates of
    /// their feet, fractional).
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

/// Wren's bounty board, on its posts south-east of the stair (column, row,
/// width, height in tiles).
pub const BOUNTY_BOARD: (i32, i32, i32, i32) = (15, 9, 3, 1);

/// The Winding Stair's parapet (column, row, width, height in tiles); the
/// stair is its middle two by two, its mouth the two tiles under them.
pub const STAIRWELL: (i32, i32, i32, i32) = (10, 5, 4, 4);
pub const STAIR_MOUTH: (i32, i32, i32, i32) = (11, 8, 2, 1);

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

/// The plate a point stands on in a home room, if any.
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

/// A keeper's feet, in arena units.
#[cfg(any(test, feature = "testing"))]
pub fn keeper_at(who: &str) -> Option<(f32, f32)> {
    SPOTS.iter().find_map(|spot| {
        spot.keeper
            .filter(|(name, _)| *name == who)
            .map(|(_, (c, r))| (c * TILE_UNITS, r * TILE_UNITS))
    })
}

/// A purchase a knight asked for at a plate, for the cockpit to pay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Order {
    pub knight: u32,
    pub station: Station,
}

impl Run {
    /// A new delve, begun in the Undercroft: `home` is what the realm has
    /// built, `treasury` what it can spend.
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
        super::world::raise_world(&mut run.dungeon);
        run.cues = vec!["home".into()];
        run.stock_stall();
        run.enter(0, None);
        let ids: Vec<u32> = run.players.keys().copied().collect();
        for id in ids {
            run.restore_gear(id);
            run.deal_home(id);
        }
        run
    }

    /// Explicit admission of a saved loop site. Never called during run ticks,
    /// and only before leaving home: passive work cannot rewrite a live map.
    pub fn enter_settlement(&mut self, site: super::exhibits::Admission<'_>) -> Result<(), String> {
        if !self.at_home_now() || self.at != 0 || self.tick != 0 || self.settlement_site.is_some() {
            return Err("settlement admission requires a new run in the PLAYER HALL".into());
        }
        self.dungeon = site.floor.clone();
        super::world::raise_world(&mut self.dungeon);
        self.boss_gates = None;
        self.settlement_site = Some(site.id.to_string());
        self.settlement_exhibits = site.exhibits;
        self.players.get_mut(&1).expect("host seat").name = site.player.to_string();
        self.enter(0, None);
        Ok(())
    }

    /// The party stands in the Undercroft.
    pub fn at_home_now(&self) -> bool {
        self.dungeon.depth == 0
    }

    /// The cockpit's word on what the realm has built, after it paid (or
    /// banked): every knight's home cards follow.
    pub fn rebuild_home(&mut self, home: Home, treasury: Spoils) {
        let landing = self.home.landing;
        let barony = std::mem::take(&mut self.home.barony);
        self.home = home;
        super::ranch::migrate(&mut self.home.ranch, &mut self.home.stable);
        self.barony_news(&barony);
        // The stair keeps the landing chosen in this run.
        self.home.landing = landing;
        self.treasury = treasury;
        // The west wing dug: the Trophy Hall's wall opens on the tavern.
        if self.at_home_now()
            && self.home.level(Station::Wing) >= 1
            && layout::dig_tavern(&mut self.dungeon)
        {
            self.cues.push("dug".into());
            self.fx.push(Fx::Shake(10));
            self.sounds.push("rock_land");
        }
        // A checkpoint from before the world: it rises around the party.
        if self.at_home_now() {
            super::world::raise_world(&mut self.dungeon);
        }
        if self.stall.is_empty() && self.home.goblins > 0 && self.at_home_now() {
            self.stock_stall();
        }
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            self.deal_home(id);
        }
    }

    /// Hand knight `id` the home's cards: the forge's edge and the hearth's
    /// warmth, as held cards in their deck, at their current rung.
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
        // Mabel's stew, once she's been brought home.
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
        // The Ore-Forge's work: dwarf-forged mail for every knight.
        if self.home.barony.is_lit("ore-forge") {
            cards.push(home_card(
                "home-mail",
                "Dwarf-Forged Mail",
                "Out of the Ore-Forge, relit. King Brannoc says it'll turn a troll. He has not tried.",
                &format!("armor {}", super::barony::FORGED_MAIL),
                MAIL_ART,
            ));
        }
        // Maud's stout, drunk at the top of the stair: this delve's.
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
        // Sir Ector's lessons: this knight's own.
        let Some(who) = self.players.get(&id).map(super::talents::knight_key) else {
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
        // The hearth's health is held apart, so a new rung adds only the
        // difference.
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

    /// What the rack and the chapel hand each knight at the top of the
    /// stair: potions, bombs, a scroll, second winds.
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
                hero.bombs = (hero.bombs + 1).min(MAX_BOMBS);
            }
            // The countryside's crown is worth a bomb below.
            if hero.laurel {
                hero.bombs = (hero.bombs + 1).min(MAX_BOMBS);
            }
            hero.winds = winds;
        }
    }

    /// The chapel's refill at each new floor: once a floor at its second rung.
    pub(super) fn refill_winds(&mut self) {
        if self.mode == super::fortune::Mode::Ironman {
            return self.iron();
        }
        if self.home.level(Station::Chapel) >= 2 {
            for hero in self.players.values_mut() {
                // At least one again; Brother Anselm's, if unspent, stays.
                hero.winds = hero.winds.max(1);
            }
        }
    }

    /// A knight at zero health with a second wind left rises again.
    pub(super) fn second_winds(&mut self) {
        let mut rose = false;
        let mut talisman = false;
        for hero in self.players.values_mut() {
            if hero.hp == 0 && hero.talisman && !hero.stone {
                // The Talisman: up at once, whole.
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

    /// One tick in the Undercroft: plates read and held, the stair's mouth
    /// turns the landing, keepers greet whoever walks up.
    pub(super) fn tick_home(&mut self, inputs: &BTreeMap<u32, Input>) {
        // The room's own game, from the registry (`activity::HOME_ROOMS`).
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
            // A tap at the stair's mouth turns the landing.
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
        // Keepers speak as a knight walks up to them, once per approach.
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

/// A home card, checked like any other card.
fn home_card(id: &str, name: &str, text: &str, effect: &str, art: &str) -> Card {
    let raw = format!(
        "name {name}\nkind hold\nrarity relic\nby the Undercroft\ntext {text}\ndrop 0\nchest 0\n{effect}\nart\n{art}\n"
    );
    cards::check(id, &raw)
        .expect("home cards pass their own checker")
        .card
}

/// A mail shirt, its rings bright at the collar.
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

/// Mabel's stew: what it adds to every knight's health.
const STEW: u32 = 15;

/// A bowl of Mabel's stew, steaming.
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

/// Sir Ector's seal: a sword over an open book.
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

/// Tobbin's hammer over the anvil.
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

/// Blaise's fire, a heart in it.
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
