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
//! Gold only ever comes from play: rent falls due each time the party
//! clears a floor of a delve (a rent day), and a knight collects it at the
//! Court or from a floor's steward below. angelX's own work keeps the
//! holdings, never pays them:
//!
//! - cache hits fill the granary; fresh tokens spent pay wages out of it;
//! - a passing test run mends the most worn walls;
//! - a commit lays courses on the room being built;
//! - a compaction copies a scroll in the scriptorium (charters need one);
//! - a subagent is a retainer hired to garrison a holding;
//! - a loop iteration is a patrol that calms unrest;
//! - a failed or slow turn stirs unrest, which halves rents and then stops
//!   them.
//!
//! Each mapping is one fixed number below. The run only marks
//! (`holdings:*`); the cockpit settles through `settle`, `rent_day` and
//! `feed`, pure functions of the realm's state, so the rules are tested
//! without a cockpit.

use super::home::BUY_HOLD;
use super::*;
use crate::realm::{Spoil, Spoils};

/// Room bays per holding.
pub const SLOTS: usize = 10;
/// The walls a holding is bought with, and their most.
pub const WALLS: u32 = 10;
/// The granary's most sacks.
pub const STORES_CAP: u32 = 40;
/// The scriptorium's shelf, retainers at the Court, patrols out, unrest.
pub const SCROLLS_CAP: u32 = 12;
pub const RETAINERS_CAP: u32 = 8;
pub const PATROLS_CAP: u32 = 10;
pub const UNREST_CAP: u32 = 10;
/// Unrest from here halves every rent; at the cap a riot stops them.
pub const RESTLESS: u32 = 5;
/// The most a coffer holds before the steward stops counting.
pub const COFFER_CAP: u32 = 3000;
/// Labour a room takes to stand.
pub const ROOM_LABOUR: u32 = 6;
/// Labour one commit lays.
pub const COMMIT_LABOUR: u32 = 3;
/// Realm seconds between the builders' shifts in play (one labour each).
pub const SHIFT_SECS: u32 = 10;
/// Fresh tokens (input not read from cache, and output) a sack of wages pays for.
pub const TOKENS_PER_SACK: u64 = 50_000;
/// Cached input tokens that fill a sack in the granary.
pub const CACHE_PER_SACK: u64 = 250_000;
/// A turn this long is a slow turn (seconds).
pub const SLOW_TURN_SECS: u64 = 180;
/// The King's tithe from a fief, each rent day.
pub const TITHE: u32 = 30;
/// Rent by stars (index = stars), each rent day.
pub const STAR_RENT: [u32; 6] = [0, 0, 20, 50, 90, 140];
/// A charter's price by the star it raises to (gold; and one scroll).
pub const STAR_PRICE: [u32; 6] = [0, 0, 500, 1000, 2000, 4000];
/// The ledger lines the board keeps.
pub const NEWS: usize = 6;
/// The King's favour runs this far either way; from `ROYAL` up his masons
/// build every room for `MASONS` less, from `-ROYAL` down his men take the
/// tithe from every holding, claimed or not.
pub const FAVOUR_CAP: i32 = 5;
pub const ROYAL: i32 = 3;
pub const MASONS: u32 = 100;
/// Raiders come when a holding's walls are this low, hold it for at most
/// `RAID_MAX` rent days' worth, and bring this many to the fight by how
/// long they have held it; the party's win frees it, mends this much wall
/// and pays the raiders' purse.
pub const RAID_WALLS: u32 = 4;
pub const RAID_MAX: u8 = 3;
pub const RAIDERS: [u32; 4] = [0, 4, 6, 8];
pub const RAID_MEND: u32 = 3;
pub const RAID_PURSE: [u32; 4] = [0, 120, 200, 300];

/// A room family: what a holding is known for.
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

/// What a built room does for the party when they pass through the floor
/// (hold E at its bay, once a floor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boon {
    /// Every knight mended this much.
    Heal(u32),
    /// Every knight to full health.
    FullHeal,
    /// A bomb for every knight.
    Bomb,
    /// Mail for every knight, this many pieces.
    Armor(u32),
    /// Every knight's spells full.
    Mana,
    /// The fallen raised, and every knight mended this much.
    Raise(u32),
    /// Every knight untouchable this many ticks.
    Ward(u32),
    /// Gold in every knight's purse.
    Gold(u32),
    /// Sacks into the realm's granary.
    Grain(u32),
    /// A tavern brawler at the party's side for the rest of the delve.
    Brawler,
    /// A kennel hound at the party's side for the rest of the delve.
    Hound,
}

/// A room a holding can build.
pub struct RoomDef {
    pub id: &'static str,
    pub name: &'static str,
    pub family: Family,
    pub price: u32,
    /// Rent each rent day, once it stands.
    pub rent: u32,
    pub boon: Boon,
    /// The boon in a word and a number (eight letters at most), on its bay.
    pub says: &'static str,
}

/// Three rooms a family, built in this order: ten bays hold ten of the
/// fifteen, so a holding is what its owners chose to leave out.
pub const ROOMS: [RoomDef; 15] = [
    RoomDef { id: "taproom", name: "Taproom", family: Family::Tavern, price: 400, rent: 20, boon: Boon::Brawler, says: "BRAWLER" },
    RoomDef { id: "brewery", name: "Brewery", family: Family::Tavern, price: 450, rent: 25, boon: Boon::Bomb, says: "BOMB +1" },
    RoomDef { id: "inn", name: "Inn Rooms", family: Family::Tavern, price: 500, rent: 30, boon: Boon::FullHeal, says: "FULL HP" },
    RoomDef { id: "smithy", name: "Smithy", family: Family::Forge, price: 400, rent: 20, boon: Boon::Armor(1), says: "MAIL +1" },
    RoomDef { id: "fletcher", name: "Fletcher", family: Family::Forge, price: 450, rent: 25, boon: Boon::Bomb, says: "FIREBOMB" },
    RoomDef { id: "armory", name: "Armory", family: Family::Forge, price: 500, rent: 30, boon: Boon::Armor(2), says: "MAIL +2" },
    RoomDef { id: "shrine", name: "Shrine", family: Family::Chapel, price: 400, rent: 20, boon: Boon::Mana, says: "BLESS" },
    RoomDef { id: "infirmary", name: "Infirmary", family: Family::Chapel, price: 450, rent: 25, boon: Boon::Raise(40), says: "RAISE" },
    RoomDef { id: "belltower", name: "Bell Tower", family: Family::Chapel, price: 500, rent: 30, boon: Boon::Ward(150), says: "WARD 5S" },
    RoomDef { id: "stall", name: "Market Stall", family: Family::Trade, price: 400, rent: 20, boon: Boon::Gold(15), says: "GOLD +15" },
    RoomDef { id: "counting", name: "Counting House", family: Family::Trade, price: 450, rent: 25, boon: Boon::Gold(30), says: "GOLD +30" },
    RoomDef { id: "warehouse", name: "Warehouse", family: Family::Trade, price: 500, rent: 30, boon: Boon::Grain(2), says: "GRAIN +2" },
    RoomDef { id: "barracks", name: "Barracks", family: Family::Barracks, price: 400, rent: 15, boon: Boon::Heal(30), says: "RATIONS" },
    RoomDef { id: "kennels", name: "Kennels", family: Family::Barracks, price: 450, rent: 15, boon: Boon::Hound, says: "HOUND" },
    RoomDef { id: "range", name: "Archery Range", family: Family::Barracks, price: 500, rent: 20, boon: Boon::Armor(1), says: "BRACERS" },
];

/// What a room does for its neighbours in the next bay (the same row of
/// five): a brewery feeds a taproom, a smithy keeps an inn awake. The room
/// that feels it, the neighbour that does it, the rent it adds or takes, and
/// the word its bay shows.
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

/// The bays next to bay `i`: its row of five on either side.
pub fn beside(i: usize) -> impl Iterator<Item = usize> {
    let row = i / 5;
    [i.wrapping_sub(1), i + 1].into_iter().filter(move |&j| j < SLOTS && j / 5 == row)
}

/// What room `id` in bay `i` feels from its neighbours in `rooms`, standing
/// ones only: (rent, word) for each.
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

/// A steward: one of the folk who run a holding, each with a way about
/// them.
pub struct Steward {
    pub id: &'static str,
    pub name: &'static str,
    /// Their way, one word.
    pub way: &'static str,
    /// What it does, two or three words.
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

/// Something the realm can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deed {
    Floor(Pack),
    /// A model house's castle, by its place on the March.
    Castle(u8),
}

/// The floors, in the order the Court lists them.
pub const FLOORS: [Pack; 6] = [
    Pack::Crypt,
    Pack::Cavern,
    Pack::Hellforge,
    Pack::Archive,
    Pack::Fungal,
    Pack::Unknown,
];

/// A floor's deed (claim) and fief prices, and the rent each of its rooms
/// adds for the depth it sits at.
const FLOOR_TERMS: [(u32, u32, u32); 6] = [
    (6000, 2400, 0),
    (6000, 2400, 0),
    (14000, 5600, 10),
    (20000, 8000, 20),
    (28000, 11200, 30),
    (40000, 16000, 40),
];
/// A castle's deed and fief, and what each of its rooms adds.
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

    /// The price outright, or in fief.
    pub fn price(self, fief: bool) -> u32 {
        let (claim, held, _) = self.terms();
        if fief { held } else { claim }
    }

    /// What each standing room adds to its rent for where the holding is.
    pub fn bonus(self) -> u32 {
        self.terms().2
    }
}

/// Everything the Court lists: the six floors, then the castles.
pub fn catalogue() -> Vec<Deed> {
    FLOORS
        .into_iter()
        .map(Deed::Floor)
        .chain(super::map::houses::ids().map(|id| Deed::Castle(id.0)))
        .collect()
}

/// A room as built (or building) in a holding's bay.
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

/// One holding of the realm's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holding {
    pub key: String,
    #[serde(default)]
    pub fief: bool,
    /// Its bays, in order; at most `SLOTS`.
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
    /// Raiders at the holding: 0 none, then how long they have held it
    /// (1 to 3). A raid stops its rent and sacks a room each rent day until
    /// the party fights it off in the steward's hall.
    #[serde(default)]
    pub raid: u8,
}

impl Holding {
    pub fn deed(&self) -> Option<Deed> {
        Deed::of_key(&self.key)
    }

    pub fn standing(&self) -> usize {
        self.rooms.iter().filter(|r| r.standing()).count()
    }

    pub fn has(&self, id: &str) -> bool {
        self.rooms.iter().any(|r| r.id == id)
    }

    /// Whether a room of `id` stands here (not going up, not sacked).
    pub fn stands(&self, id: &str) -> bool {
        self.rooms.iter().any(|r| r.id == id && r.standing())
    }

    /// What the next room would feel, and do to its neighbour, if it went
    /// into the next free bay: (rent, word), summed per word.
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

    /// The next room of `family` this holding has not built.
    pub fn next_of(&self, family: Family) -> Option<&'static RoomDef> {
        ROOMS.iter().find(|r| r.family == family && !self.has(r.id))
    }

    /// Rent this rent day before unrest, the tithe and the coffer's cap.
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

    /// What the next star needs and costs, or why it can't be raised.
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

/// The realm's holdings and the stores that keep them, inside `Home` (so a
/// friend's mirror sees them). Every field defaults: an older save loads
/// as a realm that holds nothing yet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holdings {
    #[serde(default)]
    pub owned: Vec<Holding>,
    /// The deed the Court's board shows, by its place in `catalogue`.
    #[serde(default)]
    pub pick: u32,
    /// The floors the realm has cleared, by pack name (from the treasury's
    /// record), so a knight's own screen knows what is for sale.
    #[serde(default)]
    pub cleared: std::collections::BTreeSet<String>,
    /// The houses that have served the realm (their castles are for sale).
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
    #[serde(default)]
    pub unrest: u32,
    /// Tokens not yet a whole sack.
    #[serde(default)]
    pub token_rem: u64,
    #[serde(default)]
    pub cache_rem: u64,
    /// What angelX's work has done for the holdings, all told, by the
    /// board's word (`Op::word`).
    #[serde(default)]
    pub feed: BTreeMap<String, u32>,
    /// The latest lines of the Court's ledger.
    #[serde(default)]
    pub news: Vec<String>,
    /// The King's favour, from `-FAVOUR_CAP` to `FAVOUR_CAP`.
    #[serde(default)]
    pub favour: i32,
    /// The King's card waiting for the realm's answer, by its place in
    /// `crown::CARDS`, and the card he sends next.
    #[serde(default)]
    pub card: Option<u8>,
    #[serde(default)]
    pub card_next: u8,
}

impl Holdings {
    pub fn valid(&self) -> bool {
        self.owned.len() <= 64
            && self.owned.iter().all(|h| h.rooms.len() <= SLOTS && h.key.len() <= 64 && h.stars <= 5)
            && self.cleared.len() <= 16
            && self.served.len() <= 32
            && self.feed.len() <= 16
            && self.news.len() <= NEWS
            && self.news.iter().all(|n| n.len() <= 128)
            && self.favour.abs() <= FAVOUR_CAP
            && self.card.is_none_or(|c| usize::from(c) < super::crown::CARDS.len())
            && self.owned.iter().all(|h| h.raid <= RAID_MAX)
    }

    pub fn holding(&self, key: &str) -> Option<&Holding> {
        self.owned.iter().find(|h| h.key == key)
    }

    fn holding_mut(&mut self, key: &str) -> Option<&mut Holding> {
        self.owned.iter_mut().find(|h| h.key == key)
    }

    /// The deed the board shows now.
    pub fn picked(&self) -> Deed {
        let all = catalogue();
        all[self.pick as usize % all.len()]
    }

    /// Whether a deed may be bought at all: a floor once cleared, a castle
    /// once its house has served.
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

    /// The room being built now: the first unfinished one, in the order
    /// the holdings were bought.
    pub fn building(&self) -> Option<(&Holding, &Built)> {
        self.owned
            .iter()
            .find_map(|h| h.rooms.iter().find(|r| !r.standing()).map(|r| (h, r)))
    }

    fn say(&mut self, line: String) {
        self.news.push(line);
        let over = self.news.len().saturating_sub(NEWS);
        self.news.drain(..over);
    }

    fn count(&mut self, op: Op, n: u32) {
        if n > 0 {
            *self.feed.entry(op.word().to_string()).or_default() += n;
        }
    }

    fn stir(&mut self, n: u32) {
        self.unrest = (self.unrest + n).min(UNREST_CAP);
    }

    /// Lay `n` labour on the room being built.
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

/// One of angelX's own works, as the holdings feel it.
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

    /// What the holdings call it (the board's big word).
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

    /// Where it comes from in angelX (the board's small word).
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

/// angelX's work since the last settle, as counts. The cockpit gathers it
/// from the harness as it happens; `feed` folds it into the holdings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ops {
    /// Cached input tokens read.
    pub cache: u64,
    /// Fresh tokens spent: input not read from cache, and output.
    pub tokens: u64,
    pub tests: u32,
    pub commits: u32,
    pub compactions: u32,
    pub subagents: u32,
    pub loops: u32,
    /// Failed turns.
    pub errors: u32,
    /// Turns slower than `SLOW_TURN_SECS`.
    pub slow: u32,
}

impl Ops {
    pub fn is_empty(&self) -> bool {
        *self == Ops::default()
    }
}

impl Ops {
    /// One more of `op`.
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

/// Fold angelX's work into the holdings: stores, walls, labour, scrolls,
/// retainers, patrols and unrest. Never gold. The lines it says, for the
/// ledger and the notice.
pub fn feed(h: &mut Holdings, ops: &Ops) -> Vec<String> {
    let mut said = Vec::new();
    // The granary: cache hits fill it, fresh tokens pay wages from it. An
    // unpaid wage stirs unrest.
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
    // A passing test run: the masons mend the most worn walls.
    for _ in 0..ops.tests {
        if let Some(w) = h.owned.iter_mut().filter(|w| w.walls < WALLS).min_by_key(|w| w.walls) {
            w.walls += 1;
        }
    }
    h.count(Op::Tests, ops.tests);
    // A commit: courses laid on the room being built.
    h.count(Op::Commits, ops.commits);
    said.extend(h.lay(ops.commits * COMMIT_LABOUR));
    // The scriptorium copies a scroll for every compaction.
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

/// A rent day for every holding: the party has cleared `floors` floors.
/// Rents into the coffers (halved by unrest, stopped by a riot or a
/// breach), the King's tithe from fiefs, wear on the walls (unless a
/// retainer or a brave steward holds them), stewards' wages from the
/// granary, and patrols walking off unrest. Never paid to the treasury
/// here: a knight collects.
pub fn rent_day(h: &mut Holdings, floors: usize) -> Vec<String> {
    let mut said = Vec::new();
    for _ in 0..floors.min(8) {
        // Patrols out first: each calms one.
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
            // Raiders hold it: no rent, and a room sacked (back to half
            // built) for every rent day they stay.
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
            // A brave steward or a standing barracks keeps the walls.
            let brave = w.steward.as_deref() == Some("gwen") || w.stands("barracks");
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
            // Low walls bring raiders.
            let w = &mut h.owned[i];
            if w.raid == 0 && w.walls <= RAID_WALLS {
                w.raid = 1;
                raided.push(format!("Raiders take {name}: fight them in its hall"));
            }
        }
        for line in raided {
            h.say(line.clone());
            said.push(line);
        }
        // The King sends a card when the realm holds something and none
        // waits.
        if h.card.is_none() && !h.owned.is_empty() {
            let n = super::crown::CARDS.len() as u8;
            h.card = Some(h.card_next % n);
            h.card_next = (h.card_next + 1) % n;
            said.push("The King's herald waits at the Land Office".into());
        }
        for name in breached {
            let line = format!("{name} is breached: its walls are down");
            h.say(line.clone());
            said.push(line);
        }
        if taken > 0 {
            h.say(format!("The King took {taken} gold in tithes"));
        }
    }
    let waiting: u32 = h.owned.iter().map(|w| w.coffer).sum();
    if !h.owned.is_empty() && floors > 0 {
        said.push(format!("Rent day: {waiting} gold waits in the coffers"));
    }
    said
}

/// The marks a knight's hands make at the Court and below.
pub fn pick_mark(step: i32) -> String {
    format!("holdings:pick:{}", if step < 0 { "prev" } else { "next" })
}
pub fn buy_mark(deed: Deed, fief: bool) -> String {
    format!("holdings:{}:{}", if fief { "fief" } else { "claim" }, deed.key())
}
pub fn build_mark(key: &str, family: Family) -> String {
    format!("holdings:build:{}:{key}", family.word())
}
pub fn steward_mark(key: &str) -> String {
    format!("holdings:steward:{key}")
}
pub fn charter_mark(key: &str) -> String {
    format!("holdings:charter:{key}")
}
pub const COLLECT: &str = "holdings:collect";
pub const LABOUR: &str = "holdings:labour";
pub const GRAIN: &str = "holdings:grain";

/// Why a deed can't be bought on these terms, or its price.
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

/// Why a holding can't build the next room of `family`, or its price.
pub fn build_open(h: &Holdings, key: &str, family: Family, gold: u32) -> Result<&'static RoomDef, &'static str> {
    let w = h.holding(key).ok_or("NOT OURS")?;
    if w.rooms.len() >= SLOTS {
        return Err("BAYS FULL");
    }
    let def = w.next_of(family).ok_or("ALL THREE BUILT")?;
    if gold < room_price(h, def) {
        return Err("PURSE SHORT");
    }
    Ok(def)
}

/// A room's price: the King's masons build for less while his favour is
/// high.
pub fn room_price(h: &Holdings, def: &RoomDef) -> u32 {
    if h.favour >= ROYAL { def.price - MASONS } else { def.price }
}

/// The next free steward for a holding, after the one it has.
pub fn next_steward(h: &Holdings, key: &str) -> Option<&'static Steward> {
    let w = h.holding(key)?;
    let at = w.steward.as_deref().and_then(|s| STEWARDS.iter().position(|t| t.id == s));
    let start = at.map_or(0, |i| i + 1);
    (0..STEWARDS.len())
        .map(|k| &STEWARDS[(start + k) % STEWARDS.len()])
        .find(|s| w.steward.as_deref() != Some(s.id) && !h.owned.iter().any(|o| o.steward.as_deref() == Some(s.id)))
}

/// What a settle did, for the cockpit to say and save.
#[derive(Debug, Default, PartialEq)]
pub struct Settled {
    pub changed: bool,
    /// (chorus cue, notice line), in order.
    pub said: Vec<(String, String)>,
}

/// The cockpit's half: every `holdings:*` mark the run made, paid from the
/// treasury and saved. Pure: the realm's holdings and purse in, changed.
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
                // The King likes a vassal better than a freeholder.
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
            "build" => {
                let Some((family, key)) = key.split_once(':') else { continue };
                let Some(family) = Family::from_word(family) else { continue };
                for _ in 0..n.min(SLOTS as u32) {
                    let Ok(def) = build_open(h, key, family, gold(treasury)) else { break };
                    let price = room_price(h, def);
                    pay(treasury, price);
                    let w = h.holding_mut(key).expect("open");
                    w.rooms.push(Built { id: def.id.to_string(), labour: 0 });
                    let name = w.deed().map_or_else(|| key.to_string(), Deed::name);
                    let line = format!("A {} is going up at {name} ({price} gold)", def.name);
                    h.say(line.clone());
                    out.said.push(("holding_build".into(), line));
                    out.changed = true;
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
            "labour" => {
                // A shrewd steward on the room being built drives it twice
                // as hard.
                let shrewd = h.building().is_some_and(|(w, _)| w.steward.as_deref() == Some("vell"));
                let lines = h.lay(n * if shrewd { 2 } else { 1 });
                out.said.extend(lines.into_iter().map(|l| ("holding_built".to_string(), l)));
                out.changed = true;
            }
            "raid" => {
                let Some((how, key)) = key.split_once(':') else { continue };
                let Some(w) = h.holding_mut(key).filter(|w| w.raid > 0) else { continue };
                let name = w.deed().map_or_else(|| key.to_string(), Deed::name);
                let line = if how == "won" {
                    let purse = RAID_PURSE[usize::from(w.raid.min(RAID_MAX))];
                    w.raid = 0;
                    w.walls = (w.walls + RAID_MEND).min(WALLS);
                    treasury.add(Spoil::Gold, purse);
                    format!("{name} is free: the raiders' purse, {purse} gold, to the treasury")
                } else {
                    // Beaten: they sack a room and stay.
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

/// Bring the realm's record into the holdings: which floors are cleared,
/// which houses have served. True if anything changed.
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

// ── The Court of Deeds (the Land Office) ──────────────────────────────────

/// The Court's plates (tiles): the board's arrows, the four terms, the four
/// families, and the strongbox.
pub const PREV: (i32, i32, i32, i32) = (7, 5, 2, 1);
pub const NEXT: (i32, i32, i32, i32) = (15, 5, 2, 1);
pub const CLAIM: (i32, i32, i32, i32) = (8, 7, 2, 1);
pub const FIEF: (i32, i32, i32, i32) = (11, 7, 2, 1);
pub const STEWARD: (i32, i32, i32, i32) = (14, 7, 2, 1);
pub const CHARTER: (i32, i32, i32, i32) = (17, 7, 2, 1);
pub const BUILD: [(i32, i32, i32, i32); 5] = [(8, 10, 2, 1), (11, 10, 2, 1), (14, 10, 2, 1), (17, 10, 2, 1), (20, 10, 2, 1)];
pub const STRONGBOX: (i32, i32, i32, i32) = (20, 7, 2, 1);
/// Solid furniture: the great board, the ledger wall, the clerk's desk,
/// the strongbox.
pub const BOARD: (i32, i32, i32, i32) = (2, 1, 20, 3);
pub const LEDGER: (i32, i32, i32, i32) = (1, 4, 6, 9);
pub const DESK: (i32, i32, i32, i32) = (10, 5, 4, 1);
pub const CHEST: (i32, i32, i32, i32) = (20, 5, 2, 2);

/// A Court plate: what it is and what holding E there does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plate {
    Prev,
    Next,
    Claim,
    Fief,
    Steward,
    Charter,
    Build(Family),
    Collect,
}

impl Plate {
    pub fn all() -> [(Plate, (i32, i32, i32, i32)); 12] {
        [
            (Plate::Prev, PREV),
            (Plate::Next, NEXT),
            (Plate::Claim, CLAIM),
            (Plate::Fief, FIEF),
            (Plate::Steward, STEWARD),
            (Plate::Charter, CHARTER),
            (Plate::Build(Family::Tavern), BUILD[0]),
            (Plate::Build(Family::Forge), BUILD[1]),
            (Plate::Build(Family::Chapel), BUILD[2]),
            (Plate::Build(Family::Trade), BUILD[3]),
            (Plate::Build(Family::Barracks), BUILD[4]),
            (Plate::Collect, STRONGBOX),
        ]
    }

    /// A tap turns the board; the rest are held, like a station's.
    pub fn tapped(self) -> bool {
        matches!(self, Plate::Prev | Plate::Next)
    }
}

/// What a Court plate would do right now, or why it can't: the mark to
/// make and the price (0 for free), from the run's own copy of the realm.
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
        Plate::Build(family) => build_open(h, &key, family, gold).map(|d| (build_mark(&key, family), room_price(h, d))),
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

/// The plate knight `id` stands on, in the Court.
pub fn plate_under(run: &Run, id: u32) -> Option<(Plate, (i32, i32, i32, i32))> {
    if run.room().kind != RoomKind::LandOffice {
        return None;
    }
    let hero = run.players.get(&id).filter(|h| h.hp > 0 && !h.stone)?;
    Plate::all().into_iter().find(|&(_, rect)| inside(rect, hero.x, hero.y))
}

/// The Court's room on the home floor.
pub fn land_office_room(cell: (i32, i32)) -> Room {
    let mut room = Room::solid_rock(cell);
    room.kind = RoomKind::LandOffice;
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set(col, row, Tile::Floor);
        }
    }
    for rect in [BOARD, LEDGER, DESK, CHEST] {
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

// ── A held floor's steward's hall, below ───────────────────────────────────

/// The hall's ten bays (tiles): five along the north wall, five along the
/// south, clear of the middle columns and rows, where doors come in. Each
/// bay's furniture stands against the wall; its plate is the row in front.
pub const BAYS: [(i32, i32); SLOTS] = [
    (2, 1), (5, 1), (8, 1), (14, 1), (17, 1),
    (2, 10), (5, 10), (8, 10), (14, 10), (17, 10),
];
/// The steward's table in the middle, and the plate before it.
pub const TABLE: (i32, i32, i32, i32) = (11, 5, 2, 1);
pub const STEWARD_PLATE: (i32, i32, i32, i32) = (11, 7, 2, 1);

/// Bay `i`'s furniture (solid) and its plate.
pub fn bay(i: usize) -> ((i32, i32, i32, i32), (i32, i32, i32, i32)) {
    let (c, r) = BAYS[i];
    if r < 5 {
        ((c, r, 3, 2), (c, r + 2, 3, 1))
    } else {
        ((c, r + 1, 3, 2), (c, r, 3, 1))
    }
}

/// The hall a held floor has off its first hall, its bays as built.
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

/// The holding a floor is, if the realm holds it.
pub fn held_floor<'h>(h: &'h Holdings, pack: Pack) -> Option<&'h Holding> {
    h.holding(&Deed::Floor(pack).key())
}

/// A floor the realm holds gets its steward's hall off the entrance (or
/// the first hall with room beside it), chosen without dice like the
/// King's forge-hall. True if a room was added.
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

/// The castle a knight in the realm's shared hall came in from, if any.
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

/// In a castle's hall: its deed plate (buy it, or collect its rent).
pub const CASTLE_PLATE: (i32, i32, i32, i32) = (11, 10, 2, 1);

impl Run {
    /// One tick of the holdings: builders' shifts wherever the party is,
    /// the Court's plates, a held floor's bays and steward, a castle's
    /// deed plate.
    pub(super) fn tick_holdings(&mut self, inputs: &BTreeMap<u32, Input>) {
        // A hand off the key is ready for the next plate.
        for (id, hero) in self.players.iter_mut() {
            if !inputs.get(id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact) {
                hero.buy_spent = false;
            }
        }
        if self.home.holdings.building().is_some() && self.tick.is_multiple_of(u64::from(SHIFT_SECS * HZ)) {
            *self.marks.entry(LABOUR.into()).or_default() += 1;
        }
        self.tick_raid();
        self.tick_crown(inputs);
        match self.room().kind {
            RoomKind::LandOffice => self.court(inputs),
            RoomKind::Holding => self.steward_hall(inputs),
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

    /// The knights whose hold on `plate` filled this tick (or, `tap`, who
    /// just pressed on it).
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
                // One deed a hold: two knights at once still buy it once.
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
            if !built.standing() || self.holding_used & (1 << i) != 0 {
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

    /// Whether a bay's boon can be had now: a brawler or a hound needs the
    /// party's ally place, which Beaumains keeps once hired.
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

    /// A bay's boon, for the whole party.
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
                Boon::Bomb => hero.bombs += 1,
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
                Boon::Grain(_) | Boon::Brawler | Boon::Hound => {}
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

#[cfg(any(test, feature = "testing"))]
#[path = "../../tests/rules/together_holdings__fixtures.rs"]
pub mod testing;
