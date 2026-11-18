//! Holdings: the realm's own castles and cleared floors, bought at the
//! Court of Deeds (the Land Office by the Keep) and run from there.
//!
//! Every seated knight shares one purse and one set of holdings. A holding
//! is a model castle on the March (once its house has served the realm) or
//! a floor of the delve the party has cleared. Bought outright (a claim) or
//! in fief from the King (cheaper, and he takes a tithe every rent day), a
//! holding has ten room bays to build out, a steward to hire, a charter to
//! raise (its stars) and a coffer that fills with rent.
//!
//! Its crew does the work, and the realm watches them do it: two hands and
//! one more for each room that quarters folk, walking a trip a shift from
//! the stockpile to the work — courses on the room going up, courses on the
//! walls, sacks into the granary. The steward's standing order (turned at
//! the Court's order plate) says which comes first; what there is none of,
//! they fall through to the next. Unrest halves a crew, a riot stops it.
//!
//! Gold only ever comes from play: rent falls due each time the party
//! clears a floor of a delve (a rent day), and a knight collects it at the
//! Court or from a floor's steward below. angelX's own work keeps the
//! holdings, never pays them:
//!
//! - cache hits fill the granary; fresh tokens spent pay wages out of it;
//! - a passing test run mends the most worn walls;
//! - a commit lays courses on the first room being built;
//! - a compaction copies a scroll in the scriptorium (charters need one);
//! - a subagent is a retainer hired to garrison a holding;
//! - a loop iteration is a patrol that calms unrest;
//! - a failed or slow turn stirs unrest, which halves rents and then stops
//!   them.
//!
//! And the holdings are lived in (`super::villagers`): named folk
//! migrate to a stewarded holding with a free bed and grain, eat from
//! the same granary, lay labour, guard walls, and leave when the
//! realm lets them go hungry or afraid; guests — merchants,
//! pilgrims, troubadours, hunters — lodge a few rent days and pay in
//! kind. The countryside closes the loop: each built larder plot
//! (bakery, bothy, harbour) has its own named keeper and sends a
//! sack to this granary every rent day (`note_country`), so building
//! in the upper world feeds the villages above and below.
//!
//! Each mapping is one fixed number below. The run only marks
//! (`holdings:*`); the cockpit settles through `settle`, `rent_day` and
//! `feed`, pure functions of the realm's state, so the rules are tested
//! without a cockpit.

use super::home::BUY_HOLD;
use super::*;
use crate::realm::{Spoil, Spoils};

pub const SLOTS: usize = 10;
pub const WALLS: u32 = 10;
pub const STORES_CAP: u32 = 40;
pub const SCROLLS_CAP: u32 = 12;
pub const RETAINERS_CAP: u32 = 8;
pub const PATROLS_CAP: u32 = 10;
pub const UNREST_CAP: u32 = 10;
pub const RESTLESS: u32 = 5;
pub const COFFER_CAP: u32 = 3000;
pub const ROOM_LABOUR: u32 = 6;
pub const COMMIT_LABOUR: u32 = 3;
pub const SHIFT_SECS: u32 = 10;
pub const HANDS: u32 = 2;
pub const QUARTERS: [&str; 4] = ["inn", "barracks", "greathall", "garrison"];
pub const CREW_CAP: u32 = 6;
pub const COURSE_TRIPS: u32 = 4;
pub const MEND_TRIPS: u32 = 12;
pub const STOCK_TRIPS: u32 = 12;
pub const TOKENS_PER_SACK: u64 = 50_000;
pub const CACHE_PER_SACK: u64 = 250_000;
pub const SLOW_TURN_SECS: u64 = 180;
pub const TITHE: u32 = 30;
pub const STAR_RENT: [u32; 6] = [0, 0, 20, 50, 90, 140];
pub const STAR_PRICE: [u32; 6] = [0, 0, 500, 1000, 2000, 4000];
pub const NEWS: usize = 6;
pub const FAVOUR_CAP: i32 = 5;
pub const ROYAL: i32 = 3;
pub const MASONS: u32 = 100;
pub const RAID_WALLS: u32 = 4;
pub const RAID_MAX: u8 = 3;
pub const RAIDERS: [u32; 4] = [0, 4, 6, 8];
pub const RAID_MEND: u32 = 3;
pub const RAID_PURSE: [u32; 4] = [0, 120, 200, 300];
pub const WORTH: usize = 3;
pub const WARN_DAYS: u8 = 2;
pub const CALM_DAYS: u32 = 4;
pub const BANDS: [u32; 8] = [6, 8, 10, 12, 14, 16, 18, 20];
pub const BATTER: u32 = 3;
pub const GARRISON: u32 = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Muster {
    pub key: String,
    pub days: u8,
    pub strength: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Work {
    #[default]
    Build,
    Mend,
    Stock,
}

impl Work {
    pub const ALL: [Work; 3] = [Work::Build, Work::Mend, Work::Stock];

    pub fn word(self) -> &'static str {
        match self {
            Work::Build => "BUILD",
            Work::Mend => "MEND",
            Work::Stock => "STOCK",
        }
    }

    pub fn doing(self) -> &'static str {
        match self {
            Work::Build => "BUILDING",
            Work::Mend => "MENDING",
            Work::Stock => "STOCKING",
        }
    }

    pub fn next(self) -> Work {
        match self {
            Work::Build => Work::Mend,
            Work::Mend => Work::Stock,
            Work::Stock => Work::Build,
        }
    }

    pub fn trips(self) -> u32 {
        match self {
            Work::Build => COURSE_TRIPS,
            Work::Mend => MEND_TRIPS,
            Work::Stock => STOCK_TRIPS,
        }
    }

    fn is_first(&self) -> bool {
        *self == Work::Build
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub along: f32,
    pub laden: bool,
    pub working: bool,
}

pub fn trip_offset(hand: u32, hands: u32) -> u64 {
    u64::from(hand) * u64::from(SHIFT_SECS * HZ) / u64::from(hands.max(1))
}

pub fn leg(tick: u64, hand: u32, hands: u32) -> Leg {
    let shift = u64::from(SHIFT_SECS * HZ);
    let s = ((tick + trip_offset(hand, hands)) % shift) as f32 / shift as f32;
    const SET: f32 = 0.15;
    const OUT: f32 = 0.55;
    const LOAD: f32 = 0.65;
    if s < SET {
        Leg { along: 0.0, laden: true, working: true }
    } else if s < OUT {
        Leg { along: (s - SET) / (OUT - SET), laden: false, working: false }
    } else if s < LOAD {
        Leg { along: 1.0, laden: false, working: true }
    } else {
        Leg { along: 1.0 - (s - LOAD) / (1.0 - LOAD), laden: true, working: false }
    }
}

pub fn lands(tick: u64, hand: u32, hands: u32) -> bool {
    (tick + trip_offset(hand, hands)).is_multiple_of(u64::from(SHIFT_SECS * HZ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Tavern,
    Forge,
    Chapel,
    Trade,
    Barracks,
}

impl Family {
    pub const ALL: [Family; 5] = [Family::Tavern, Family::Forge, Family::Chapel, Family::Trade, Family::Barracks];

    pub fn word(self) -> &'static str {
        match self {
            Family::Tavern => "tavern",
            Family::Forge => "forge",
            Family::Chapel => "chapel",
            Family::Trade => "trade",
            Family::Barracks => "barracks",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Family::Tavern => "TAVERN",
            Family::Forge => "FORGE",
            Family::Chapel => "CHAPEL",
            Family::Trade => "TRADE",
            Family::Barracks => "BARRACKS",
        }
    }

    pub fn from_word(word: &str) -> Option<Family> {
        Family::ALL.into_iter().find(|f| f.word() == word)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boon {
    Heal(u32),
    FullHeal,
    Bomb,
    Armor(u32),
    Mana,
    Raise(u32),
    Ward(u32),
    Gold(u32),
    Grain(u32),
    Brawler,
    Hound,
    None,
}

pub struct RoomDef {
    pub id: &'static str,
    pub name: &'static str,
    pub family: Family,
    pub price: u32,
    pub rent: u32,
    pub boon: Boon,
    pub says: &'static str,
    pub relic: Option<Spoil>,
}

pub const ROOMS: [RoomDef; 21] = [
    RoomDef { id: "taproom", name: "Taproom", family: Family::Tavern, price: 400, rent: 20, boon: Boon::Brawler, says: "BRAWLER", relic: None },
    RoomDef { id: "brewery", name: "Brewery", family: Family::Tavern, price: 450, rent: 25, boon: Boon::Bomb, says: "BOMB +1", relic: None },
    RoomDef { id: "inn", name: "Inn Rooms", family: Family::Tavern, price: 500, rent: 30, boon: Boon::FullHeal, says: "FULL HP", relic: None },
    RoomDef { id: "greathall", name: "Great Hall", family: Family::Tavern, price: 800, rent: 40, boon: Boon::FullHeal, says: "MEND ALL", relic: Some(Spoil::Hearthstone) },
    RoomDef { id: "smithy", name: "Smithy", family: Family::Forge, price: 400, rent: 20, boon: Boon::Armor(1), says: "MAIL +1", relic: None },
    RoomDef { id: "fletcher", name: "Fletcher", family: Family::Forge, price: 450, rent: 25, boon: Boon::Bomb, says: "FIREBOMB", relic: None },
    RoomDef { id: "armory", name: "Armory", family: Family::Forge, price: 500, rent: 30, boon: Boon::Armor(2), says: "MAIL +2", relic: None },
    RoomDef { id: "greatforge", name: "Great Forge", family: Family::Forge, price: 800, rent: 40, boon: Boon::Armor(3), says: "MAIL +3", relic: Some(Spoil::Forgeheart) },
    RoomDef { id: "shrine", name: "Shrine", family: Family::Chapel, price: 400, rent: 20, boon: Boon::Mana, says: "BLESS", relic: None },
    RoomDef { id: "infirmary", name: "Infirmary", family: Family::Chapel, price: 450, rent: 25, boon: Boon::Raise(40), says: "RAISE", relic: None },
    RoomDef { id: "belltower", name: "Bell Tower", family: Family::Chapel, price: 500, rent: 30, boon: Boon::Ward(150), says: "WARD 5S", relic: None },
    RoomDef { id: "greatnave", name: "Great Nave", family: Family::Chapel, price: 800, rent: 40, boon: Boon::Mana, says: "SPELLS", relic: Some(Spoil::Bellmetal) },
    RoomDef { id: "stall", name: "Market Stall", family: Family::Trade, price: 400, rent: 20, boon: Boon::Gold(15), says: "GOLD +15", relic: None },
    RoomDef { id: "counting", name: "Counting House", family: Family::Trade, price: 450, rent: 25, boon: Boon::Gold(30), says: "GOLD +30", relic: None },
    RoomDef { id: "warehouse", name: "Warehouse", family: Family::Trade, price: 500, rent: 30, boon: Boon::Grain(2), says: "GRAIN +2", relic: None },
    RoomDef { id: "bourse", name: "Bourse", family: Family::Trade, price: 800, rent: 40, boon: Boon::Gold(60), says: "GOLD +60", relic: Some(Spoil::Ledgerglass) },
    RoomDef { id: "waypoint", name: "Waypoint", family: Family::Trade, price: 1000, rent: 0, boon: Boon::None, says: "WAY DOWN", relic: Some(Spoil::Waystone) },
    RoomDef { id: "barracks", name: "Barracks", family: Family::Barracks, price: 400, rent: 15, boon: Boon::Heal(30), says: "RATIONS", relic: None },
    RoomDef { id: "kennels", name: "Kennels", family: Family::Barracks, price: 450, rent: 15, boon: Boon::Hound, says: "HOUND", relic: None },
    RoomDef { id: "range", name: "Archery Range", family: Family::Barracks, price: 500, rent: 20, boon: Boon::Armor(1), says: "BRACERS", relic: None },
    RoomDef { id: "garrison", name: "Garrison", family: Family::Barracks, price: 800, rent: 35, boon: Boon::Ward(300), says: "WARD 10S", relic: Some(Spoil::Warhorn) },
];

pub const NEIGHBOURS: [(&str, &str, i32, &str); 11] = [
    ("taproom", "brewery", 10, "BREWED"),
    ("taproom", "barracks", 10, "THIRSTY"),
    ("inn", "smithy", -10, "NOISY"),
    ("inn", "belltower", -10, "NOISY"),
    ("inn", "kennels", -10, "NOISY"),
    ("infirmary", "shrine", 10, "BLESSED"),
    ("shrine", "taproom", -10, "ROWDY"),
    ("stall", "warehouse", 10, "STOCKED"),
    ("counting", "warehouse", 10, "STOCKED"),
    ("armory", "fletcher", 10, "SUPPLIED"),
    ("range", "fletcher", 10, "SUPPLIED"),
];

pub fn beside(i: usize) -> impl Iterator<Item = usize> {
    let row = i / 5;
    [i.wrapping_sub(1), i + 1].into_iter().filter(move |&j| j < SLOTS && j / 5 == row)
}

pub fn neighbours(rooms: &[Built], i: usize, id: &str) -> Vec<(i32, &'static str)> {
    NEIGHBOURS
        .iter()
        .filter(|(feels, by, _, _)| {
            *feels == id && beside(i).any(|j| rooms.get(j).is_some_and(|r| r.standing() && r.id == *by))
        })
        .map(|&(_, _, n, word)| (n, word))
        .collect()
}

pub fn room_def(id: &str) -> Option<&'static RoomDef> {
    ROOMS.iter().find(|r| r.id == id)
}

pub struct Steward {
    pub id: &'static str,
    pub name: &'static str,
    pub way: &'static str,
    pub does: &'static str,
}

pub const STEWARDS: [Steward; 5] = [
    Steward { id: "wynn", name: "Wynn", way: "GREEDY", does: "RENT +30" },
    Steward { id: "aled", name: "Brother Aled", way: "PIOUS", does: "CALMS UNREST" },
    Steward { id: "gwen", name: "Gwen", way: "BRAVE", does: "WALLS HOLD" },
    Steward { id: "hob", name: "Old Hob", way: "SOT", does: "TAVERNS +10" },
    Steward { id: "vell", name: "Mistress Vell", way: "SHREWD", does: "BUILDS FAST" },
];

pub fn steward(id: &str) -> Option<&'static Steward> {
    STEWARDS.iter().find(|s| s.id == id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deed {
    Floor(Pack),
    Castle(u8),
}

pub const FLOORS: [Pack; 6] = [
    Pack::Crypt,
    Pack::Cavern,
    Pack::Hellforge,
    Pack::Archive,
    Pack::Fungal,
    Pack::Unknown,
];

const FLOOR_TERMS: [(u32, u32, u32); 6] = [
    (6000, 2400, 0),
    (6000, 2400, 0),
    (14000, 5600, 10),
    (20000, 8000, 20),
    (28000, 11200, 30),
    (40000, 16000, 40),
];
const CASTLE_TERMS: (u32, u32, u32) = (5000, 2000, 5);

impl Deed {
    pub fn key(self) -> String {
        match self {
            Deed::Floor(pack) => format!("floor:{pack:?}"),
            Deed::Castle(i) => format!("castle:{}", super::map::houses::get(super::map::houses::HouseId(i)).key),
        }
    }

    pub fn of_key(key: &str) -> Option<Deed> {
        if let Some(pack) = key.strip_prefix("floor:") {
            return FLOORS.into_iter().find(|p| format!("{p:?}") == pack).map(Deed::Floor);
        }
        let house = key.strip_prefix("castle:")?;
        super::map::houses::by_key(house).map(|id| Deed::Castle(id.0))
    }

    pub fn name(self) -> String {
        match self {
            Deed::Floor(pack) => pack.name().to_string(),
            Deed::Castle(i) => super::map::houses::get(super::map::houses::HouseId(i)).castle.clone(),
        }
    }

    fn terms(self) -> (u32, u32, u32) {
        match self {
            Deed::Floor(pack) => FLOOR_TERMS[FLOORS.iter().position(|&p| p == pack).unwrap_or(0)],
            Deed::Castle(_) => CASTLE_TERMS,
        }
    }

    pub fn price(self, fief: bool) -> u32 {
        let (claim, held, _) = self.terms();
        if fief { held } else { claim }
    }

    pub fn bonus(self) -> u32 {
        self.terms().2
    }
}

pub fn catalogue() -> Vec<Deed> {
    FLOORS
        .into_iter()
        .map(Deed::Floor)
        .chain(super::map::houses::ids().map(|id| Deed::Castle(id.0)))
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Built {
    pub id: String,
    #[serde(default)]
    pub labour: u32,
}

impl Built {
    pub fn standing(&self) -> bool {
        self.labour >= ROOM_LABOUR
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holding {
    pub key: String,
    #[serde(default)]
    pub fief: bool,
    #[serde(default)]
    pub rooms: Vec<Built>,
    #[serde(default)]
    pub steward: Option<String>,
    #[serde(default)]
    pub walls: u32,
    #[serde(default)]
    pub coffer: u32,
    #[serde(default)]
    pub stars: u8,
    #[serde(default)]
    pub raid: u8,
    #[serde(default, skip_serializing_if = "Work::is_first")]
    pub order: Work,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub toil: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folk: Vec<super::villagers::Villager>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visitors: Vec<super::villagers::Visitor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occasion: Option<super::occasions::Occasion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seized: Option<super::fey::Seizure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub works: Vec<super::fey::Masterwork>,
}

impl Holding {
    pub fn deed(&self) -> Option<Deed> {
        Deed::of_key(&self.key)
    }

    pub fn raidable(&self) -> bool {
        !(crate::world::plan::tabula() && matches!(self.deed(), Some(Deed::Castle(_))))
    }

    pub fn crew(&self) -> u32 {
        if self.raid > 0 {
            return 0;
        }
        let quartered = self.rooms.iter().filter(|r| r.standing() && QUARTERS.contains(&r.id.as_str())).count() as u32;
        (HANDS + quartered).min(CREW_CAP)
    }

    pub fn work(&self, stores: u32) -> Option<Work> {
        if self.raid > 0 {
            return None;
        }
        let can = |w: Work| match w {
            Work::Build => self.rooms.iter().any(|r| !r.standing()),
            Work::Mend => self.walls < WALLS,
            Work::Stock => stores < STORES_CAP,
        };
        std::iter::once(self.order).chain(Work::ALL).find(|&w| can(w))
    }

    pub fn rising(&self) -> Option<usize> {
        self.rooms.iter().position(|r| !r.standing())
    }

    pub fn standing(&self) -> usize {
        self.rooms.iter().filter(|r| r.standing()).count()
    }

    pub fn has(&self, id: &str) -> bool {
        self.rooms.iter().any(|r| r.id == id)
    }

    pub fn stands(&self, id: &str) -> bool {
        self.rooms.iter().any(|r| r.id == id && r.standing())
    }

    pub fn preview(&self, def: &RoomDef) -> Vec<(i32, &'static str)> {
        let i = self.rooms.len();
        let mut rooms = self.rooms.clone();
        rooms.push(Built { id: def.id.to_string(), labour: ROOM_LABOUR });
        let mut out = neighbours(&rooms, i, def.id);
        for j in beside(i) {
            if let Some(r) = rooms.get(j).filter(|r| r.standing()) {
                out.extend(
                    NEIGHBOURS
                        .iter()
                        .filter(|(feels, by, _, _)| *feels == r.id && *by == def.id)
                        .map(|&(_, _, n, w)| (n, w)),
                );
            }
        }
        out
    }

    pub fn next_of(&self, family: Family) -> Option<&'static RoomDef> {
        ROOMS.iter().find(|r| r.family == family && !self.has(r.id))
    }

    pub fn rent(&self) -> u32 {
        if self.walls == 0 {
            return 0;
        }
        let bonus = self.deed().map_or(0, Deed::bonus);
        let sot = self.steward.as_deref() == Some("hob");
        let rooms: u32 = self
            .rooms
            .iter()
            .enumerate()
            .filter(|(_, r)| r.standing())
            .filter_map(|(i, r)| room_def(&r.id).map(|d| (i, d)))
            .map(|(i, d)| {
                let near: i32 = neighbours(&self.rooms, i, d.id).iter().map(|&(n, _)| n).sum();
                let own = d.rent + bonus + if sot && d.family == Family::Tavern { 10 } else { 0 };
                (own as i32 + near).max(0) as u32
            })
            .sum();
        let greedy = if self.steward.as_deref() == Some("wynn") { 30 } else { 0 };
        rooms + STAR_RENT[usize::from(self.stars.min(5))] + greedy
    }

    pub fn charter(&self) -> Result<(u8, u32), &'static str> {
        let next = self.stars + 1;
        match next {
            2 if self.steward.is_none() => Err("NEEDS A STEWARD"),
            3 if self.standing() < 5 => Err("NEEDS 5 ROOMS"),
            4 if self.standing() < SLOTS => Err("NEEDS 10 ROOMS"),
            5 if self.walls < WALLS => Err("NEEDS FULL WALLS"),
            2..=5 => Ok((next, STAR_PRICE[usize::from(next)])),
            _ => Err("FIVE STARS"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holdings {
    #[serde(default)]
    pub owned: Vec<Holding>,
    #[serde(default)]
    pub pick: u32,
    #[serde(default)]
    pub cleared: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub served: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub stores: u32,
    #[serde(default)]
    pub scrolls: u32,
    #[serde(default)]
    pub retainers: u32,
    #[serde(default)]
    pub patrols: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub bells: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub blades: u32,
    #[serde(default)]
    pub unrest: u32,
    #[serde(default)]
    pub token_rem: u64,
    #[serde(default)]
    pub cache_rem: u64,
    #[serde(default)]
    pub feed: BTreeMap<String, u32>,
    #[serde(default)]
    pub news: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tidings: Vec<String>,
    #[serde(default)]
    pub favour: i32,
    #[serde(default)]
    pub card: Option<u8>,
    #[serde(default)]
    pub card_next: u8,
    #[serde(default)]
    pub foreman: super::foreman::Foreman,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub folk_next: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub visitor_next: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub larders: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muster: Option<Muster>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub calm: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sieges: u32,
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

impl Holdings {
    pub fn valid(&self) -> bool {
        self.owned.len() <= 64
            && self.owned.iter().all(|h| h.rooms.len() <= SLOTS && h.key.len() <= 64 && h.stars <= 5)
            && self.bells <= super::goods::BELLS_HUNG
            && self.blades <= super::goods::ARMOURY
            && self.cleared.len() <= 16
            && self.served.len() <= 32
            && self.feed.len() <= 16
            && self.news.len() <= NEWS
            && self.news.iter().all(|n| n.len() <= 128)
            && self.favour.abs() <= FAVOUR_CAP
            && self.card.is_none_or(|c| usize::from(c) < super::crown::CARDS.len())
            && self.owned.iter().all(|h| h.raid <= RAID_MAX)
            && self.owned.iter().all(|h| h.folk.len() <= super::villagers::MAX_VILLAGERS && h.folk.iter().all(|f| f.valid()))
            && self.owned.iter().all(|h| h.visitors.len() <= super::villagers::MAX_VISITORS && h.visitors.iter().all(|v| v.valid()))
            && self.owned.iter().all(|h| {
                h.occasion.as_ref().is_none_or(|o| o.valid())
                    && h.seized.as_ref().is_none_or(|s| s.valid())
                    && h.works.len() <= super::fey::MAX_WORKS
                    && h.works.iter().all(|m| m.valid())
            })
            && self.tidings.len() <= super::occasions::TIDINGS
            && self.tidings.iter().all(|t| t.len() <= super::occasions::TIDING_LEN * 4)
            && self.larders <= super::countryside::LARDERS.len() as u32
            && self.foreman.valid()
            && self.muster.as_ref().is_none_or(|m| m.key.len() <= 64 && m.days <= WARN_DAYS)
    }

    pub fn holding(&self, key: &str) -> Option<&Holding> {
        self.owned.iter().find(|h| h.key == key)
    }

    pub fn holding_mut(&mut self, key: &str) -> Option<&mut Holding> {
        self.owned.iter_mut().find(|h| h.key == key)
    }

    pub fn picked(&self) -> Deed {
        let all = catalogue();
        all[self.pick as usize % all.len()]
    }

    pub fn offered(&self, deed: Deed) -> Result<(), &'static str> {
        if self.holding(&deed.key()).is_some() {
            return Err("OURS");
        }
        match deed {
            Deed::Floor(pack) if !self.cleared.contains(pack.name()) => Err("NOT CLEARED"),
            Deed::Castle(i)
                if !self
                    .served
                    .contains(super::map::houses::get(super::map::houses::HouseId(i)).key) =>
            {
                Err("NEVER SERVED")
            }
            _ => Ok(()),
        }
    }

    pub fn building(&self) -> Option<(&Holding, &Built)> {
        self.owned
            .iter()
            .find_map(|h| h.rooms.iter().find(|r| !r.standing()).map(|r| (h, r)))
    }

    pub fn hands(&self, w: &Holding) -> u32 {
        let crew = w.crew();
        if self.unrest >= UNREST_CAP {
            0
        } else if self.unrest >= RESTLESS {
            crew.div_ceil(2)
        } else {
            crew
        }
    }

    pub fn crew_work(&self, w: &Holding) -> Option<Work> {
        if self.hands(w) == 0 {
            return None;
        }
        w.work(self.stores)
    }

    pub fn trip(&mut self, key: &str) -> Option<String> {
        let stores = self.stores;
        let w = self.holding_mut(key)?;
        let job = w.work(stores)?;
        let shrewd = w.steward.as_deref() == Some("vell");
        w.toil += if shrewd && job == Work::Build { 2 } else { 1 };
        if w.toil < job.trips() {
            return None;
        }
        w.toil = 0;
        match job {
            Work::Build => {
                let name = w.deed().map_or_else(|| w.key.clone(), Deed::name);
                let r = w.rooms.iter_mut().find(|r| !r.standing())?;
                r.labour += 1;
                if r.standing() {
                    let room = room_def(&r.id).map_or("A room", |d| d.name);
                    let line = format!("{room} stands at {name}");
                    self.say(line.clone());
                    return Some(line);
                }
            }
            Work::Mend => w.walls = (w.walls + 1).min(WALLS),
            Work::Stock => self.stores = (self.stores + 1).min(STORES_CAP),
        }
        None
    }

    pub fn garrison(&self, w: &Holding) -> u32 {
        let barracks = w
            .rooms
            .iter()
            .filter(|r| r.standing() && room_def(&r.id).is_some_and(|d| d.family == Family::Barracks))
            .count() as u32;
        GARRISON * (barracks + u32::from(w.steward.as_deref() == Some("gwen")))
    }

    pub fn defence(&self, w: &Holding) -> u32 {
        let men = self.retainers + self.garrison(w);
        w.walls
            + self.garrison(w)
            + self.retainers
            + self.bells * super::goods::BELL_CALL
            + self.blades.min(men) * super::goods::BLADE_EDGE
    }

    fn richest(&self) -> Option<String> {
        self.owned
            .iter()
            .enumerate()
            .filter(|(_, w)| w.raidable())
            .max_by_key(|&(i, w)| (w.standing(), w.coffer, std::cmp::Reverse(i)))
            .map(|(_, w)| w.key.clone())
    }

    fn storyteller(&mut self) -> Vec<String> {
        let mut said = Vec::new();
        if let Some(m) = self.muster.as_mut() {
            m.days = m.days.saturating_sub(1);
            if m.days == 0 {
                let m = self.muster.take().expect("on the road");
                said.extend(self.arrive(&m));
                self.calm = CALM_DAYS;
            }
            return said;
        }
        if self.owned.iter().any(|w| w.raid > 0) {
            return said;
        }
        if self.calm > 0 {
            self.calm -= 1;
            return said;
        }
        let worth: usize = self.owned.iter().map(Holding::standing).sum();
        if worth < WORTH {
            return said;
        }
        let Some(key) = self.richest() else {
            return said;
        };
        let strength = BANDS[(self.sieges as usize).min(BANDS.len() - 1)];
        self.sieges += 1;
        let name = Deed::of_key(&key).map_or_else(|| key.clone(), Deed::name);
        self.muster = Some(Muster { key, days: WARN_DAYS, strength });
        let line = format!("Scouts: {strength} raiders muster against {name}, {WARN_DAYS} rent days off");
        self.say(line.clone());
        said.push(line);
        said
    }

    fn arrive(&mut self, m: &Muster) -> Vec<String> {
        let Some(i) = self.owned.iter().position(|w| w.key == m.key).filter(|&i| self.owned[i].raidable()) else {
            return Vec::new();
        };
        let name = self.owned[i].deed().map_or_else(|| m.key.clone(), Deed::name);
        let garrison = self.garrison(&self.owned[i]);
        let held = self.owned[i].walls + garrison;
        let aid = m
            .strength
            .checked_sub(held + self.retainers)
            .filter(|&short| short > 0)
            .and_then(|short| {
                super::goods::cover(short, self.bells, self.blades.min(self.retainers + garrison))
            });
        let line = if self.owned[i].raid > 0 {
            let w = &mut self.owned[i];
            w.raid = (w.raid + 1).min(RAID_MAX);
            format!("{} more raiders join those holding {name}", m.strength)
        } else if held + self.retainers >= m.strength || aid.is_some() {
            let (rang, armed) = aid.unwrap_or((0, 0));
            let help = rang * super::goods::BELL_CALL + armed * super::goods::BLADE_EDGE;
            let spent = m.strength.saturating_sub(held + help).min(self.retainers);
            self.retainers -= spent;
            self.bells -= rang;
            self.blades -= armed;
            let w = &mut self.owned[i];
            w.walls = w.walls.saturating_sub(BATTER);
            let mut line = if spent > 0 {
                format!("{name} held: {} raiders broke on its walls and {spent} retainers", m.strength)
            } else {
                format!("{name} held: {} raiders broke on its walls", m.strength)
            };
            if rang > 0 {
                line.push_str(&format!("; {rang} {} rang the folk to the wall", if rang == 1 { "bell" } else { "bells" }));
            }
            if armed > 0 {
                line.push_str(&format!("; {armed} forge {} armed its men", if armed == 1 { "blade" } else { "blades" }));
            }
            line
        } else {
            let w = &mut self.owned[i];
            let lost = std::mem::take(&mut w.coffer);
            w.raid = 1;
            if lost > 0 {
                format!("{name} fell to {} raiders: they carried off {lost} gold", m.strength)
            } else {
                format!("{name} fell to {} raiders: fight them in its hall", m.strength)
            }
        };
        self.say(line.clone());
        vec![line]
    }

    pub(crate) fn say(&mut self, line: String) {
        self.news.push(line);
        let over = self.news.len().saturating_sub(NEWS);
        self.news.drain(..over);
    }

    pub(crate) fn tell(&mut self, line: &str) {
        self.tidings.push(super::occasions::tiding(line));
        let over = self.tidings.len().saturating_sub(super::occasions::TIDINGS);
        self.tidings.drain(..over);
    }

    fn count(&mut self, op: Op, n: u32) {
        if n > 0 {
            *self.feed.entry(op.word().to_string()).or_default() += n;
        }
    }

    fn stir(&mut self, n: u32) {
        self.unrest = (self.unrest + n).min(UNREST_CAP);
    }

    fn lay(&mut self, mut n: u32) -> Vec<String> {
        let mut said = Vec::new();
        while n > 0 {
            let Some(h) = self.owned.iter_mut().find(|h| h.rooms.iter().any(|r| !r.standing())) else {
                break;
            };
            let name = h.deed().map_or_else(|| h.key.clone(), Deed::name);
            let r = h.rooms.iter_mut().find(|r| !r.standing()).expect("found");
            let step = n.min(ROOM_LABOUR - r.labour);
            r.labour += step;
            n -= step;
            if r.standing() {
                let room = room_def(&r.id).map_or("A room", |d| d.name);
                said.push(format!("{room} stands at {name}"));
            }
        }
        for line in &said {
            self.say(line.clone());
        }
        said
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Cache,
    Tokens,
    Tests,
    Commits,
    Compactions,
    Subagents,
    Loops,
    Errors,
}

impl Op {
    pub const ALL: [Op; 8] = [
        Op::Cache,
        Op::Tokens,
        Op::Tests,
        Op::Commits,
        Op::Compactions,
        Op::Subagents,
        Op::Loops,
        Op::Errors,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Op::Cache => "GRAIN",
            Op::Tokens => "WAGES",
            Op::Tests => "WALLS",
            Op::Commits => "BUILDS",
            Op::Compactions => "SCROLLS",
            Op::Subagents => "RETAINERS",
            Op::Loops => "PATROLS",
            Op::Errors => "UNREST",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Op::Cache => "CACHE HITS",
            Op::Tokens => "TOKENS SPENT",
            Op::Tests => "TESTS PASS",
            Op::Commits => "COMMITS",
            Op::Compactions => "COMPACTIONS",
            Op::Subagents => "SUBAGENTS",
            Op::Loops => "LOOP ROUNDS",
            Op::Errors => "ERRORS, SLOW",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ops {
    pub cache: u64,
    pub tokens: u64,
    pub tests: u32,
    pub commits: u32,
    pub compactions: u32,
    pub subagents: u32,
    pub loops: u32,
    pub errors: u32,
    pub slow: u32,
}

impl Ops {
    pub fn is_empty(&self) -> bool {
        *self == Ops::default()
    }
}

impl Ops {
    pub fn note(&mut self, op: Op) {
        match op {
            Op::Tests => self.tests += 1,
            Op::Commits => self.commits += 1,
            Op::Compactions => self.compactions += 1,
            Op::Subagents => self.subagents += 1,
            Op::Loops => self.loops += 1,
            Op::Errors => self.errors += 1,
            Op::Cache | Op::Tokens => {}
        }
    }
}

pub fn feed(h: &mut Holdings, ops: &Ops) -> Vec<String> {
    let mut said = Vec::new();
    h.cache_rem += ops.cache;
    let sacks = (h.cache_rem / CACHE_PER_SACK) as u32;
    h.cache_rem %= CACHE_PER_SACK;
    if sacks > 0 {
        h.stores = (h.stores + sacks).min(STORES_CAP);
        h.count(Op::Cache, sacks);
    }
    h.token_rem += ops.tokens;
    let wages = (h.token_rem / TOKENS_PER_SACK) as u32;
    h.token_rem %= TOKENS_PER_SACK;
    if wages > 0 {
        let paid = wages.min(h.stores);
        h.stores -= paid;
        h.count(Op::Tokens, wages);
        if paid < wages {
            h.stir(wages - paid);
            said.push(format!("Wages unpaid: the granary is bare ({} short)", wages - paid));
        }
    }
    for _ in 0..ops.tests {
        if let Some(w) = h.owned.iter_mut().filter(|w| w.walls < WALLS).min_by_key(|w| w.walls) {
            w.walls += 1;
        }
    }
    h.count(Op::Tests, ops.tests);
    h.count(Op::Commits, ops.commits);
    said.extend(h.lay(ops.commits * COMMIT_LABOUR));
    if ops.compactions > 0 {
        h.scrolls = (h.scrolls + ops.compactions).min(SCROLLS_CAP);
        h.count(Op::Compactions, ops.compactions);
    }
    if ops.subagents > 0 {
        h.retainers = (h.retainers + ops.subagents).min(RETAINERS_CAP);
        h.count(Op::Subagents, ops.subagents);
    }
    if ops.loops > 0 {
        h.patrols = (h.patrols + ops.loops).min(PATROLS_CAP);
        h.count(Op::Loops, ops.loops);
    }
    let stirred = ops.errors + ops.slow;
    if stirred > 0 {
        h.stir(stirred);
        h.count(Op::Errors, stirred);
    }
    said
}

pub fn rent_day(h: &mut Holdings, floors: usize) -> Vec<String> {
    let mut said = Vec::new();
    for _ in 0..floors.min(8) {
        if h.larders > 0 && !h.owned.is_empty() && h.stores < STORES_CAP {
            let sent = h.larders.min(STORES_CAP - h.stores);
            h.stores += sent;
            let line = format!("The countryside's keepers send {sent} sacks to the granary");
            h.say(line.clone());
            said.push(line);
        }
        let calmed = h.patrols.min(h.unrest);
        h.patrols -= calmed;
        h.unrest -= calmed;
        let mut taken = 0;
        let mut breached = Vec::new();
        let mut raided = Vec::new();
        let kings_men = h.favour <= -ROYAL;
        for i in 0..h.owned.len() {
            let unrest = h.unrest;
            let w = &mut h.owned[i];
            let name = w.deed().map_or_else(|| w.key.clone(), Deed::name);
            if !w.raidable() {
                w.raid = 0;
            }
            if w.raid > 0 {
                if let Some(r) = w.rooms.iter_mut().rev().find(|r| r.standing()) {
                    r.labour = ROOM_LABOUR / 2;
                    let room = room_def(&r.id).map_or("a room", |d| d.name);
                    raided.push(format!("Raiders sack the {room} at {name}"));
                }
                w.raid = (w.raid + 1).min(RAID_MAX);
            }
            let mut rent = if w.raid > 0 { 0 } else { w.rent() };
            if unrest >= UNREST_CAP {
                rent = 0;
            } else if unrest >= RESTLESS {
                rent /= 2;
            }
            if w.fief || kings_men {
                let tithe = TITHE.min(rent);
                rent -= tithe;
                taken += tithe;
            }
            w.coffer = (w.coffer + rent).min(COFFER_CAP);
            let brave = w.steward.as_deref() == Some("gwen")
                || w.stands("barracks")
                || w.folk.iter().any(|f| f.trade == super::villagers::Trade::Guard && f.hunger == 0 && f.age < super::villagers::ELDER)
                || w.visitors.iter().any(|v| v.kind == super::villagers::VisitorKind::Hunter);
            let pious = w.steward.as_deref() == Some("aled");
            let greedy = w.steward.as_deref() == Some("wynn");
            let steward = w.steward.is_some();
            if !brave {
                if h.retainers > 0 {
                    h.retainers -= 1;
                } else if w.walls > 0 {
                    w.walls -= 1;
                    if w.walls == 0 {
                        breached.push(w.deed().map_or_else(|| w.key.clone(), Deed::name));
                    }
                }
            }
            if steward {
                if h.stores > 0 {
                    h.stores -= 1;
                } else {
                    h.stir(1);
                }
            }
            if pious {
                h.unrest = h.unrest.saturating_sub(1);
            }
            if greedy {
                h.stir(1);
            }
            let w = &mut h.owned[i];
            if w.raid == 0 && w.walls <= RAID_WALLS && w.raidable() {
                w.raid = 1;
                raided.push(format!("Raiders take {name}: fight them in its hall"));
            }
        }
        for line in raided {
            h.say(line.clone());
            said.push(line);
        }
        said.extend(super::villagers::keep(h));
        if h.card.is_none() && !h.owned.is_empty() {
            let n = super::crown::CARDS.len() as u8;
            h.card = Some(h.card_next % n);
            h.card_next = (h.card_next + 1) % n;
            if !crate::world::plan::tabula() {
                said.push("The King's herald waits at the Land Office".into());
            }
        }
        for name in breached {
            let line = format!("{name} is breached: its walls are down");
            h.say(line.clone());
            said.push(line);
        }
        if taken > 0 {
            h.say(format!("The King took {taken} gold in tithes"));
        }
        said.extend(h.storyteller());
    }
    let waiting: u32 = h.owned.iter().map(|w| w.coffer).sum();
    if !h.owned.is_empty() && floors > 0 {
        said.push(format!("Rent day: {waiting} gold waits in the coffers"));
    }
    super::foreman::draft(h);
    if !h.foreman.queue.is_empty() && !crate::world::plan::tabula() {
        said.push("The Foreman has petitions at the Land Office".into());
    }
    said
}

pub fn pick_mark(step: i32) -> String {
    format!("holdings:pick:{}", if step < 0 { "prev" } else { "next" })
}
pub fn buy_mark(deed: Deed, fief: bool) -> String {
    format!("holdings:{}:{}", if fief { "fief" } else { "claim" }, deed.key())
}
pub fn steward_mark(key: &str) -> String {
    format!("holdings:steward:{key}")
}
pub fn charter_mark(key: &str) -> String {
    format!("holdings:charter:{key}")
}
pub fn order_mark(key: &str) -> String {
    format!("holdings:order:{key}")
}
pub fn trip_mark(key: &str) -> String {
    format!("holdings:trip:{key}")
}
pub const COLLECT: &str = "holdings:collect";
pub const LABOUR: &str = "holdings:labour";
pub const GRAIN: &str = "holdings:grain";

pub fn buy_open(h: &Holdings, deed: Deed, fief: bool, gold: u32, marks: &BTreeMap<String, u32>) -> Result<u32, &'static str> {
    h.offered(deed)?;
    if marks.contains_key(&buy_mark(deed, false)) || marks.contains_key(&buy_mark(deed, true)) {
        return Err("OURS");
    }
    let price = deed.price(fief);
    if gold < price {
        return Err("PURSE SHORT");
    }
    Ok(price)
}

pub fn room_price(h: &Holdings, def: &RoomDef) -> u32 {
    if h.favour >= ROYAL { def.price - MASONS } else { def.price }
}

pub fn next_steward(h: &Holdings, key: &str) -> Option<&'static Steward> {
    let w = h.holding(key)?;
    let at = w.steward.as_deref().and_then(|s| STEWARDS.iter().position(|t| t.id == s));
    let start = at.map_or(0, |i| i + 1);
    (0..STEWARDS.len())
        .map(|k| &STEWARDS[(start + k) % STEWARDS.len()])
        .find(|s| w.steward.as_deref() != Some(s.id) && !h.owned.iter().any(|o| o.steward.as_deref() == Some(s.id)))
}

#[derive(Debug, Default, PartialEq)]
pub struct Settled {
    pub changed: bool,
    pub said: Vec<(String, String)>,
}

pub fn settle(h: &mut Holdings, treasury: &mut Spoils, marks: &BTreeMap<String, u32>) -> Settled {
    let mut out = Settled::default();
    let gold = |t: &Spoils| t.get(Spoil::Gold);
    let pay = |t: &mut Spoils, n: u32| {
        let mut cost = Spoils::default();
        cost.add(Spoil::Gold, n);
        t.take(&cost);
    };
    for (mark, &n) in marks {
        let Some(rest) = mark.strip_prefix("holdings:") else {
            continue;
        };
        let (verb, key) = rest.split_once(':').unwrap_or((rest, ""));
        match verb {
            "pick" => {
                let len = catalogue().len() as u32;
                let step = n % len;
                h.pick = if key == "prev" { (h.pick + len - step) % len } else { (h.pick + step) % len };
                out.changed = true;
            }
            "claim" | "fief" => {
                let fief = verb == "fief";
                let Some(deed) = Deed::of_key(key) else { continue };
                if h.offered(deed).is_err() {
                    continue;
                }
                let price = deed.price(fief);
                if gold(treasury) < price {
                    out.said.push(("cant_afford".into(), format!("{}: {price} gold, and the purse is short", deed.name())));
                    continue;
                }
                pay(treasury, price);
                h.owned.push(Holding { key: deed.key(), fief, walls: WALLS, stars: 1, ..Holding::default() });
                h.favour = (h.favour + if fief { 1 } else { -1 }).clamp(-FAVOUR_CAP, FAVOUR_CAP);
                let line = if fief {
                    format!("{} is ours in fief for {price} gold: the King takes his tithe", deed.name())
                } else {
                    format!("{} is ours outright for {price} gold", deed.name())
                };
                h.say(line.clone());
                out.said.push(("holding_bought".into(), line));
                out.changed = true;
            }
            "petition" => {
                let aye = key == "aye";
                for _ in 0..n.min(super::foreman::QUEUE as u32) {
                    match super::foreman::answer(h, treasury, aye) {
                        Ok(line) => {
                            h.say(line.clone());
                            out.said.push(("holding_petition".into(), line));
                            out.changed = true;
                        }
                        Err(why) if why != "NO PETITIONS" => {
                            if let Some(p) = h.foreman.queue.first().cloned() {
                                h.foreman.queue.remove(0);
                                let name = p.key.split_once(':').map_or(p.key.as_str(), |(_, k)| k);
                                let line = format!("The Foreman's petition for {name} lapses: {why}");
                                h.say(line.clone());
                                out.said.push(("holding_petition".into(), line));
                                out.changed = true;
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            "steward" => {
                let Some(next) = next_steward(h, key) else { continue };
                let w = h.holding_mut(key).expect("steward of a holding");
                w.steward = Some(next.id.to_string());
                let name = w.deed().map_or_else(|| key.to_string(), Deed::name);
                let line = format!("{} the {} keeps {name} now", next.name, next.way.to_lowercase());
                h.say(line.clone());
                out.said.push(("holding_steward".into(), line));
                out.changed = true;
            }
            "charter" => {
                let Some(w) = h.holding(key) else { continue };
                let Ok((star, price)) = w.charter() else { continue };
                if h.scrolls == 0 {
                    out.said.push(("cant_afford".into(), "The charter needs a scroll from the scriptorium".into()));
                    continue;
                }
                if gold(treasury) < price {
                    out.said.push(("cant_afford".into(), format!("The charter: {price} gold, and the purse is short")));
                    continue;
                }
                pay(treasury, price);
                h.scrolls -= 1;
                let w = h.holding_mut(key).expect("held");
                w.stars = star;
                let name = w.deed().map_or_else(|| key.to_string(), Deed::name);
                let line = format!("{name} is chartered: {star} stars");
                h.say(line.clone());
                out.said.push(("holding_charter".into(), line));
                out.changed = true;
            }
            "collect" => {
                let total: u32 = h.owned.iter().map(|w| w.coffer).sum();
                if total == 0 {
                    continue;
                }
                for w in &mut h.owned {
                    w.coffer = 0;
                }
                treasury.add(Spoil::Gold, total);
                let line = format!("Rents collected: {total} gold to the treasury");
                h.say(line.clone());
                out.said.push(("holding_rent".into(), line));
                out.changed = true;
            }
            "order" => {
                let Some(w) = h.holding_mut(key) else { continue };
                for _ in 0..n % Work::ALL.len() as u32 {
                    w.order = w.order.next();
                }
                let name = w.deed().map_or_else(|| key.to_string(), Deed::name);
                let line = format!("{name}'s crew will {} first", w.order.word().to_lowercase());
                h.say(line.clone());
                out.said.push(("holding_order".into(), line));
                out.changed = true;
            }
            "trip" => {
                if h.holding(key).is_none() {
                    continue;
                }
                for _ in 0..n.min(CREW_CAP * 8) {
                    if let Some(line) = h.trip(key) {
                        out.said.push(("holding_built".to_string(), line));
                    }
                }
                out.changed = true;
            }
            "labour" => {
                let shrewd = h.building().is_some_and(|(w, _)| w.steward.as_deref() == Some("vell"));
                let lines = h.lay(n * if shrewd { 2 } else { 1 });
                out.said.extend(lines.into_iter().map(|l| ("holding_built".to_string(), l)));
                out.changed = true;
            }
            "raid" => {
                let Some((how, key)) = key.split_once(':') else { continue };
                if how == "won"
                    && h.holding(key).is_some_and(|w| w.raid == 0)
                    && h.muster.as_ref().is_some_and(|m| m.key == key)
                {
                    h.muster = None;
                    h.calm = CALM_DAYS;
                    let purse = RAID_PURSE[1];
                    treasury.add(Spoil::Gold, purse);
                    let name = Deed::of_key(key).map_or_else(|| key.to_string(), Deed::name);
                    let line = format!("The raiders were met at {name}: the band scatters, {purse} gold to the treasury");
                    h.say(line.clone());
                    out.said.push(("holding_raid_won".into(), line));
                    out.changed = true;
                    continue;
                }
                let Some(w) = h.holding_mut(key).filter(|w| w.raid > 0) else { continue };
                let name = w.deed().map_or_else(|| key.to_string(), Deed::name);
                let line = if how == "won" {
                    let purse = RAID_PURSE[usize::from(w.raid.min(RAID_MAX))];
                    w.raid = 0;
                    w.walls = (w.walls + RAID_MEND).min(WALLS);
                    treasury.add(Spoil::Gold, purse);
                    format!("{name} is free: the raiders' purse, {purse} gold, to the treasury")
                } else {
                    if let Some(r) = w.rooms.iter_mut().rev().find(|r| r.standing()) {
                        r.labour = ROOM_LABOUR / 2;
                    }
                    format!("The raiders hold {name} and sack a room")
                };
                h.say(line.clone());
                out.said.push((format!("holding_raid_{how}"), line));
                out.changed = true;
            }
            "card" => {
                let Some((card, pick)) = key.split_once(':') else { continue };
                let (Ok(card), Ok(pick)) = (card.parse::<u8>(), pick.parse::<usize>()) else { continue };
                if h.card != Some(card) {
                    continue;
                }
                match super::crown::answer(h, treasury, card, pick) {
                    Ok(line) => {
                        h.say(line.clone());
                        out.said.push(("king_card".into(), line));
                        out.changed = true;
                    }
                    Err(why) => out.said.push(("cant_afford".into(), why.to_string())),
                }
            }
            "grain" => {
                h.stores = (h.stores + n * 2).min(STORES_CAP);
                out.changed = true;
            }
            _ => {}
        }
    }
    out
}

pub fn note_country(h: &mut Holdings, country: &super::countryside::Country) -> bool {
    let n = country.larders();
    let changed = h.larders != n;
    h.larders = n;
    changed
}

pub fn note_realm(h: &mut Holdings, reclaimed: &BTreeMap<String, u32>, yards: &BTreeMap<String, u32>) -> bool {
    let mut changed = false;
    for pack in FLOORS {
        if reclaimed.get(pack.name()).copied().unwrap_or(0) > 0 && h.cleared.insert(pack.name().to_string()) {
            changed = true;
        }
    }
    for (house, &calls) in yards {
        if calls > 0 && h.served.len() < 32 && h.served.insert(house.clone()) {
            changed = true;
        }
    }
    changed
}

pub const PREV: (i32, i32, i32, i32) = (7, 5, 2, 1);
pub const NEXT: (i32, i32, i32, i32) = (15, 5, 2, 1);
pub const CLAIM: (i32, i32, i32, i32) = (8, 7, 2, 1);
pub const FIEF: (i32, i32, i32, i32) = (11, 7, 2, 1);
pub const STEWARD: (i32, i32, i32, i32) = (14, 7, 2, 1);
pub const CHARTER: (i32, i32, i32, i32) = (17, 7, 2, 1);
pub const AYE: (i32, i32, i32, i32) = (8, 10, 2, 1);
pub const NAY: (i32, i32, i32, i32) = (11, 10, 2, 1);
pub const ORDER: (i32, i32, i32, i32) = (14, 10, 2, 1);
pub const LEDGE: (i32, i32, i32, i32) = (17, 9, 5, 2);
pub const STRONGBOX: (i32, i32, i32, i32) = (20, 7, 2, 1);
pub const BOARD: (i32, i32, i32, i32) = (2, 1, 20, 3);
pub const LEDGER: (i32, i32, i32, i32) = (1, 4, 6, 9);
pub const DESK: (i32, i32, i32, i32) = (10, 5, 4, 1);
pub const CHEST: (i32, i32, i32, i32) = (20, 5, 2, 2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plate {
    Prev,
    Next,
    Claim,
    Fief,
    Steward,
    Charter,
    Aye,
    Nay,
    Orders,
    Collect,
}

impl Plate {
    pub fn all() -> [(Plate, (i32, i32, i32, i32)); 10] {
        [
            (Plate::Prev, PREV),
            (Plate::Next, NEXT),
            (Plate::Claim, CLAIM),
            (Plate::Fief, FIEF),
            (Plate::Steward, STEWARD),
            (Plate::Charter, CHARTER),
            (Plate::Aye, AYE),
            (Plate::Nay, NAY),
            (Plate::Orders, ORDER),
            (Plate::Collect, STRONGBOX),
        ]
    }

    pub fn tapped(self) -> bool {
        matches!(self, Plate::Prev | Plate::Next)
    }
}

pub fn plate_open(run: &Run, plate: Plate) -> Result<(String, u32), &'static str> {
    let h = &run.home.holdings;
    let gold = run.treasury.get(Spoil::Gold);
    let deed = h.picked();
    let key = deed.key();
    let owned = h.holding(&key);
    match plate {
        Plate::Prev => Ok((pick_mark(-1), 0)),
        Plate::Next => Ok((pick_mark(1), 0)),
        Plate::Claim | Plate::Fief => {
            let fief = plate == Plate::Fief;
            buy_open(h, deed, fief, gold, &run.marks).map(|p| (buy_mark(deed, fief), p))
        }
        Plate::Steward => {
            owned.ok_or("NOT OURS")?;
            next_steward(h, &key).map(|_| (steward_mark(&key), 0)).ok_or("NONE FREE")
        }
        Plate::Charter => {
            let (_, price) = owned.ok_or("NOT OURS")?.charter()?;
            if h.scrolls == 0 {
                return Err("NO SCROLL");
            }
            if gold < price {
                return Err("PURSE SHORT");
            }
            Ok((charter_mark(&key), price))
        }
        Plate::Aye => {
            let (price, _) = super::foreman::open(h, &run.treasury)?;
            Ok((super::foreman::aye_mark(), price))
        }
        Plate::Nay => {
            h.foreman.queue.first().ok_or("NO PETITIONS")?;
            Ok((super::foreman::nay_mark(), 0))
        }
        Plate::Orders => {
            owned.ok_or("NOT OURS")?;
            Ok((order_mark(&key), 0))
        }
        Plate::Collect => {
            let total: u32 = h.owned.iter().map(|w| w.coffer).sum();
            if total == 0 { Err("COFFERS EMPTY") } else { Ok((COLLECT.into(), total)) }
        }
    }
}

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32);
    c >= col && c < col + w && r >= row && r < row + h
}

pub fn plate_under(run: &Run, id: u32) -> Option<(Plate, (i32, i32, i32, i32))> {
    if run.room().kind != RoomKind::LandOffice {
        return None;
    }
    let hero = run.players.get(&id).filter(|h| h.hp > 0 && !h.stone)?;
    Plate::all().into_iter().find(|&(_, rect)| inside(rect, hero.x, hero.y))
}

pub fn land_office_room(cell: (i32, i32)) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = RoomKind::LandOffice;
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set(col, row, Tile::Floor);
        }
    }
    for rect in [BOARD, LEDGER, DESK, CHEST, LEDGE] {
        let (c, r, w, h) = rect;
        for row in r..r + h {
            for col in c..c + w {
                room.set(col as usize, row as usize, Tile::Block);
            }
        }
    }
    room.open_door(2);
    room
}

pub const BAYS: [(i32, i32); SLOTS] = [
    (2, 1), (5, 1), (8, 1), (14, 1), (17, 1),
    (2, 10), (5, 10), (8, 10), (14, 10), (17, 10),
];
pub const TABLE: (i32, i32, i32, i32) = (11, 5, 2, 1);
pub const STEWARD_PLATE: (i32, i32, i32, i32) = (11, 7, 2, 1);

pub fn bay(i: usize) -> ((i32, i32, i32, i32), (i32, i32, i32, i32)) {
    let (c, r) = BAYS[i];
    if r < 5 {
        ((c, r, 3, 2), (c, r + 2, 3, 1))
    } else {
        ((c, r + 1, 3, 2), (c, r, 3, 1))
    }
}

pub fn holding_room(cell: (i32, i32), door: usize, held: &Holding) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = RoomKind::Holding;
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set(col, row, Tile::Floor);
        }
    }
    let mut solid = vec![TABLE];
    for i in 0..held.rooms.len().min(SLOTS) {
        solid.push(bay(i).0);
    }
    for (c, r, w, h) in solid {
        for row in r..r + h {
            for col in c..c + w {
                room.set(col as usize, row as usize, Tile::Block);
            }
        }
    }
    room.open_door(door);
    room.cleared = true;
    room
}

pub fn held_floor<'h>(h: &'h Holdings, pack: Pack) -> Option<&'h Holding> {
    h.holding(&Deed::Floor(pack).key())
}

pub fn add_holding_hall(floor: &mut Floor, h: &Holdings) -> bool {
    let Some(held) = held_floor(h, floor.pack) else {
        return false;
    };
    if floor.depth == 0 || floor.rooms.len() >= 64 || floor.rooms.iter().any(|r| r.kind == RoomKind::Holding) {
        return false;
    }
    let taken = |c: (i32, i32)| floor.rooms.iter().any(|r| r.cell == c);
    let mut pick = None;
    'hosts: for (i, room) in floor.rooms.iter().enumerate() {
        let host = matches!(
            room.kind,
            RoomKind::Start | RoomKind::Sanctuary | RoomKind::Hall | RoomKind::Treasure | RoomKind::Fight
        ) && !room.great();
        if !host {
            continue;
        }
        for dir in [1usize, 3, 2, 0] {
            let (dx, dy) = DIRS[dir];
            let cell = (room.cell.0 + dx, room.cell.1 + dy);
            if !taken(cell) && (-16..=16).contains(&cell.0) && (-16..=16).contains(&cell.1) {
                pick = Some((i, dir, cell));
                break 'hosts;
            }
        }
    }
    let Some((host, dir, cell)) = pick else {
        return false;
    };
    floor.rooms[host].open_door(dir);
    floor.rooms.push(holding_room(cell, (dir + 2) % 4, held));
    true
}

pub fn castle_here(run: &Run) -> Option<u8> {
    if run.room().kind != RoomKind::RealmHall {
        return None;
    }
    let back = run.kingdom.return_to.as_ref()?;
    let (tx, ty) = ((back.x / TILE_UNITS) as i32, (back.y / TILE_UNITS) as i32);
    super::map::houses::ids()
        .find(|&id| super::map::castle_stand(id) == (tx, ty))
        .map(|id| id.0)
}

pub const CASTLE_PLATE: (i32, i32, i32, i32) = (11, 10, 2, 1);

impl Run {
    pub(super) fn tick_holdings(&mut self, inputs: &BTreeMap<u32, Input>) {
        for (id, hero) in self.players.iter_mut() {
            if !inputs.get(id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact) {
                hero.buy_spent = false;
            }
        }
        let h = &self.home.holdings;
        let tick = self.tick;
        let landed: Vec<String> = h
            .owned
            .iter()
            .filter(|w| h.crew_work(w).is_some())
            .flat_map(|w| {
                let hands = h.hands(w);
                (0..hands).filter(move |&j| lands(tick, j, hands)).map(|_| trip_mark(&w.key))
            })
            .collect();
        for mark in landed {
            *self.marks.entry(mark).or_default() += 1;
        }
        self.tick_raid();
        self.tick_crown(inputs);
        if (!self.home.holdings.foreman.queue.is_empty() || self.home.holdings.muster.is_some())
            && self.kingdom.moments.errand.is_none()
            && self.on_the_way(RoomKind::LandOffice)
        {
            self.kingdom.moments.errand = Some(RoomKind::LandOffice);
        }
        match self.room().kind {
            RoomKind::LandOffice => self.court(inputs),
            RoomKind::Holding => {
                self.steward_hall(inputs);
                self.folk_talk(inputs);
            }
            RoomKind::RealmHall => {
                if let Some(i) = castle_here(self) {
                    let deed = Deed::Castle(i);
                    let key = deed.key();
                    let gold = self.treasury.get(Spoil::Gold);
                    let what = if self.home.holdings.holding(&key).is_some() {
                        (self.home.holdings.owned.iter().any(|w| w.coffer > 0)).then(|| COLLECT.to_string())
                    } else {
                        buy_open(&self.home.holdings, deed, false, gold, &self.marks).ok().map(|_| buy_mark(deed, false))
                    };
                    if let Some(what) = what
                        && !self.held_on(inputs, CASTLE_PLATE, false).is_empty()
                    {
                        *self.marks.entry(what).or_default() += 1;
                        self.sounds.push("play_card");
                    }
                }
            }
            _ => {}
        }
    }

    fn held_on(&mut self, inputs: &BTreeMap<u32, Input>, plate: (i32, i32, i32, i32), tap: bool) -> Vec<u32> {
        let mut done = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs.get(&id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact);
            let on = hero.hp > 0 && !hero.stone && inside(plate, hero.x, hero.y);
            if !on {
                continue;
            }
            if !pressing {
                hero.buy_spent = false;
                hero.buying = 0;
                continue;
            }
            if hero.buy_spent {
                continue;
            }
            hero.buying = hero.buying.saturating_add(1);
            if tap || hero.buying >= BUY_HOLD {
                hero.buying = 0;
                hero.buy_spent = true;
                done.push(id);
            }
        }
        done
    }

    fn court(&mut self, inputs: &BTreeMap<u32, Input>) {
        for (plate, rect) in Plate::all() {
            let Ok((mark, _)) = plate_open(self, plate) else {
                continue;
            };
            let n = self.held_on(inputs, rect, plate.tapped()).len() as u32;
            if n > 0 {
                let n = if plate.tapped() { n } else { 1 };
                *self.marks.entry(mark).or_default() += n;
                self.sounds.push(if plate.tapped() { "door_pass" } else { "play_card" });
            }
        }
    }

    fn steward_hall(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(held) = held_floor(&self.home.holdings, self.dungeon.pack).cloned() else {
            return;
        };
        if held.coffer > 0 || self.home.holdings.owned.iter().any(|w| w.coffer > 0) {
            if !self.held_on(inputs, STEWARD_PLATE, false).is_empty() {
                *self.marks.entry(COLLECT.into()).or_default() += 1;
                self.sounds.push("gold");
            }
        }
        for (i, built) in held.rooms.iter().enumerate().take(SLOTS) {
            if !built.standing() || self.holding_used & (1 << i) != 0 || super::fey::seized_bay(&held, i) {
                continue;
            }
            let Some(def) = room_def(&built.id) else { continue };
            if self.boon_open(def.boon).is_err() || self.held_on(inputs, bay(i).1, false).is_empty() {
                continue;
            }
            self.holding_used |= 1 << i;
            self.boon(def.boon);
            self.sounds.push("play_card");
            self.found = Some((self.tick, 0, format!("{}: {}", def.name, def.says)));
        }
    }

    pub fn boon_open(&self, boon: Boon) -> Result<(), &'static str> {
        match boon {
            Boon::Brawler | Boon::Hound
                if self.hireling.as_ref().is_some_and(|h| h.kind == super::hireling::AllyKind::Beaumains) =>
            {
                Err("BEAUMAINS IS HERE")
            }
            _ => Ok(()),
        }
    }

    fn boon(&mut self, boon: Boon) {
        if let Boon::Brawler | Boon::Hound = boon {
            use super::hireling::{AllyKind, Hireling};
            let kind = if boon == Boon::Brawler { AllyKind::Brawler } else { AllyKind::Hound };
            let lead = self.players.iter().find(|(_, h)| h.hp > 0 && !h.stone).map(|(&id, h)| (id, (h.x, h.y)));
            if let Some((id, at)) = lead {
                let grid = Grid { room: &self.dungeon.rooms[self.at], barred: false };
                let at = super::hireling::beside(&grid, at);
                let mut ally = Hireling::minion(kind, 0, self.dungeon.depth, at, id);
                if kind == AllyKind::Brawler {
                    ally.max_hp *= 2;
                    ally.hp = ally.max_hp;
                }
                self.hireling = Some(ally);
                self.cues.push(format!("summoned:{}", kind.word()));
            }
            return;
        }
        for hero in self.players.values_mut().filter(|h| !h.stone) {
            match boon {
                Boon::Heal(n) => {
                    hero.mend(n, stakes::Mend::Rest);
                }
                Boon::FullHeal => {
                    hero.mend(hero.max_hp, stakes::Mend::Rest);
                }
                Boon::Bomb => hero.give_bombs(1),
                Boon::Armor(n) => hero.armor += n,
                Boon::Mana => hero.mana = MAX_MANA,
                Boon::Raise(n) => {
                    if hero.hp == 0 {
                        hero.hp = n.min(hero.max_hp).max(1);
                    } else {
                        hero.mend(n, stakes::Mend::Rest);
                    }
                }
                Boon::Ward(t) => hero.invulnerable = hero.invulnerable.max(t),
                Boon::Gold(n) => hero.carried.add(Spoil::Gold, n),
                Boon::Grain(_) | Boon::Brawler | Boon::Hound | Boon::None => {}
            }
        }
        if let Boon::Grain(_) = boon {
            *self.marks.entry(GRAIN.into()).or_default() += 1;
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_holdings__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/together_crews__tests.rs"]
mod crew_tests;

#[cfg(test)]
#[path = "../../tests/rules/together_storyteller__tests.rs"]
mod storyteller_tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../tests/rules/together_holdings__fixtures.rs"]
pub mod testing;
