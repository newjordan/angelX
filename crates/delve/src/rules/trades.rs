//! Trades: the work a knight's hands know (the Works, `rules::hands`).
//!
//! A knight holds trades from the kingdom economy's list, each an
//! Apprentice, a Journeyman or a Master, earned by doing the work. Each deed
//! (a block cut, a stone set, a load carried) is a mark on the run
//! (`trade:<trade>:<knight>`) the realm settles into its record of the
//! knight (`Home::trades`), the way experience is: the run marks it, the
//! realm keeps it. What the run shows is the realm's count and the marks not
//! yet settled, so a deed is never counted twice and never lost between.
//!
//! Trades are learned from a teacher at the Keep's site: Bomrekedan's seven
//! teachers (their questline files give the figures and their lines) each
//! teach a few trades, and the Squires' Guild's hired hands work for the
//! party (`hands`).
//!
//! A trade also means something below: a mason's eye finds the secret wall,
//! a smith keeps the blade keen, a carpenter braces the barricade.

use super::*;

/// How many deeds make a Journeyman, and a Master.
pub const JOURNEYMAN: u32 = 30;
pub const MASTER: u32 = 200;

/// The most knights a ledger keeps, and the most deeds in one trade.
pub const MAX_KNIGHTS: usize = 64;
pub const MAX_DEEDS: u32 = 1_000_000;

/// One trade of the economy's list (`ECONOMY.md`, Professions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trade {
    Labourer,
    Quarryman,
    BankerMason,
    Setter,
    MasterMason,
    LimeBurner,
    Collier,
    Woodsman,
    Sawyer,
    Carpenter,
    Smith,
    Bloomer,
    Roper,
    Carter,
    Ostler,
    Bargeman,
    Miller,
    Baker,
    Brewer,
    Herder,
    Tanner,
    Wheelwright,
    ClerkOfWorks,
}

impl Trade {
    /// Every trade, in the economy's order.
    pub const ALL: [Trade; 23] = [
        Trade::Labourer,
        Trade::Quarryman,
        Trade::BankerMason,
        Trade::Setter,
        Trade::MasterMason,
        Trade::LimeBurner,
        Trade::Collier,
        Trade::Woodsman,
        Trade::Sawyer,
        Trade::Carpenter,
        Trade::Smith,
        Trade::Bloomer,
        Trade::Roper,
        Trade::Carter,
        Trade::Ostler,
        Trade::Bargeman,
        Trade::Miller,
        Trade::Baker,
        Trade::Brewer,
        Trade::Herder,
        Trade::Tanner,
        Trade::Wheelwright,
        Trade::ClerkOfWorks,
    ];

    /// The id the economy's list and the sim's workers know it by.
    pub const fn id(self) -> &'static str {
        match self {
            Trade::Labourer => "labourer",
            Trade::Quarryman => "quarryman",
            Trade::BankerMason => "banker-mason",
            Trade::Setter => "setter",
            Trade::MasterMason => "master-mason",
            Trade::LimeBurner => "lime-burner",
            Trade::Collier => "collier",
            Trade::Woodsman => "woodsman",
            Trade::Sawyer => "sawyer",
            Trade::Carpenter => "carpenter",
            Trade::Smith => "smith",
            Trade::Bloomer => "bloomer",
            Trade::Roper => "roper",
            Trade::Carter => "carter",
            Trade::Ostler => "ostler",
            Trade::Bargeman => "bargeman",
            Trade::Miller => "miller",
            Trade::Baker => "baker",
            Trade::Brewer => "brewer",
            Trade::Herder => "herder",
            Trade::Tanner => "tanner",
            Trade::Wheelwright => "wheelwright",
            Trade::ClerkOfWorks => "clerk-of-works",
        }
    }

    pub fn from_id(id: &str) -> Option<Trade> {
        Trade::ALL.into_iter().find(|t| t.id() == id)
    }

    /// The name as a plate shows it.
    pub fn name(self) -> String {
        self.id().replace('-', " ").to_uppercase()
    }

    /// The tool in the hand of a knight at this trade.
    pub const fn tool(self) -> Tool {
        match self {
            Trade::Labourer => Tool::Hod,
            Trade::Quarryman => Tool::Pick,
            Trade::BankerMason => Tool::Chisel,
            Trade::Setter => Tool::Mallet,
            Trade::MasterMason => Tool::Dividers,
            Trade::LimeBurner | Trade::Collier => Tool::Rake,
            Trade::Woodsman => Tool::Axe,
            Trade::Sawyer => Tool::Saw,
            Trade::Carpenter | Trade::Wheelwright => Tool::Hammer,
            Trade::Smith | Trade::Bloomer => Tool::Tongs,
            Trade::Roper | Trade::Carter | Trade::Ostler => Tool::Rope,
            Trade::Bargeman | Trade::Herder => Tool::Crook,
            Trade::Miller | Trade::Baker => Tool::Sack,
            Trade::Brewer => Tool::Ladle,
            Trade::Tanner => Tool::Knife,
            Trade::ClerkOfWorks => Tool::Ledger,
        }
    }
}

/// What a knight carries for a trade, drawn in the knight's hand
/// (`Hero::tool`): a number from 1, 0 being bare hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Hod = 1,
    Pick,
    Chisel,
    Mallet,
    Hammer,
    Dividers,
    Ledger,
    Tongs,
    Axe,
    Saw,
    Rake,
    Rope,
    Ladle,
    Crook,
    Sack,
    Knife,
}

impl Tool {
    pub const ALL: [Tool; 16] = [
        Tool::Hod,
        Tool::Pick,
        Tool::Chisel,
        Tool::Mallet,
        Tool::Hammer,
        Tool::Dividers,
        Tool::Ledger,
        Tool::Tongs,
        Tool::Axe,
        Tool::Saw,
        Tool::Rake,
        Tool::Rope,
        Tool::Ladle,
        Tool::Crook,
        Tool::Sack,
        Tool::Knife,
    ];

    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.code() == code)
    }

    /// Where, in `Hero::tool`, what is on a hod is kept: the tool in the low
    /// bits and above them a load (0 nothing, 1 a stone, 2 mortar).
    pub const LOAD_SHIFT: u8 = 5;

    /// The tool and its load as one code.
    pub fn pack(self, load: u8) -> u8 {
        self.code() | (load.min(2) << Tool::LOAD_SHIFT)
    }

    /// The tool and the load a code names.
    pub fn unpack(code: u8) -> Option<(Tool, u8)> {
        Tool::from_code(code & ((1 << Tool::LOAD_SHIFT) - 1)).map(|t| (t, code >> Tool::LOAD_SHIFT))
    }
}

/// How far a knight has come in a trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    Apprentice,
    Journeyman,
    Master,
}

impl Rank {
    pub fn name(self) -> &'static str {
        match self {
            Rank::Apprentice => "APPRENTICE",
            Rank::Journeyman => "JOURNEYMAN",
            Rank::Master => "MASTER",
        }
    }

    /// The rank `deeds` make, or none while the trade is not taken up.
    pub fn of(deeds: u32) -> Option<Rank> {
        match deeds {
            0 => None,
            d if d >= MASTER => Some(Rank::Master),
            d if d >= JOURNEYMAN => Some(Rank::Journeyman),
            _ => Some(Rank::Apprentice),
        }
    }

    /// How many deeds the next rank is away, or none for a Master.
    pub fn next_at(deeds: u32) -> Option<u32> {
        match Rank::of(deeds) {
            None => Some(1),
            Some(Rank::Apprentice) => Some(JOURNEYMAN),
            Some(Rank::Journeyman) => Some(MASTER),
            Some(Rank::Master) => None,
        }
    }

    /// How many ticks either side of the beat still land a good blow, to the
    /// rank: a steadier hand has more room (`hands`).
    pub fn grace(self) -> u32 {
        match self {
            Rank::Apprentice => 3,
            Rank::Journeyman => 4,
            Rank::Master => 5,
        }
    }
}

/// What the realm keeps of one knight's trades: deeds by trade id. A trade
/// is held once it has a deed (the lesson itself is the first).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Knack(pub BTreeMap<String, u32>);

/// Every knight's trades, by the name the realm keeps a knight's records
/// under (`talents::profile_key`). Left out of a save while it is empty, and
/// a save from before trades loads with none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ledger(pub BTreeMap<String, Knack>);

impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// A ledger a realm may carry: names and counts in bounds, trades the
    /// economy's list knows.
    pub fn valid(&self) -> bool {
        self.0.len() <= MAX_KNIGHTS
            && self.0.iter().all(|(who, knack)| {
                !who.is_empty() && who.len() <= 64 && knack.0.iter().all(|(id, &n)| Trade::from_id(id).is_some() && n <= MAX_DEEDS)
            })
    }

    /// The deeds `who` has in `trade` (the realm's count).
    pub fn deeds(&self, who: &str, trade: Trade) -> u32 {
        self.0.get(who).and_then(|k| k.0.get(trade.id())).copied().unwrap_or(0)
    }

    /// Add `n` deeds to `who`'s `trade`.
    pub fn earn(&mut self, who: &str, trade: Trade, n: u32) {
        if self.0.len() >= MAX_KNIGHTS && !self.0.contains_key(who) {
            return;
        }
        let at = self.0.entry(who.to_string()).or_default().0.entry(trade.id().to_string()).or_default();
        *at = at.saturating_add(n).min(MAX_DEEDS);
    }

    /// Settle the run's marks into the record: every `trade:` mark is deeds
    /// to the knight it names. True if any was.
    pub fn settle(&mut self, marks: &BTreeMap<String, u32>) -> bool {
        let mut changed = false;
        for (key, &n) in marks {
            if let Some((trade, who)) = split_mark(key)
                && n > 0
            {
                self.earn(who, trade, n);
                changed = true;
            }
        }
        changed
    }
}

/// The run's mark for deeds `who` did at `trade`.
pub fn mark_key(trade: Trade, who: &str) -> String {
    format!("trade:{}:{}", trade.id(), who)
}

/// The trade and the knight a `trade:` mark names.
pub fn split_mark(key: &str) -> Option<(Trade, &str)> {
    let rest = key.strip_prefix("trade:")?;
    let (id, who) = rest.split_once(':')?;
    Some((Trade::from_id(id)?, who))
}

/// A school at the Keep: one of Bomrekedan's teachers, their questline
/// file's figure and lines, and the trades they teach.
pub struct School {
    /// The questline (`bom-*.txt`) that gives the teacher's name, look and art.
    pub power: &'static str,
    /// Where they stand, the castle's frame in metres (`x` east, `y` north).
    pub at: [f32; 2],
    pub teaches: &'static [Trade],
}

/// The seven teachers of Bomrekedan, at the Keep's bailey in two rows east
/// of the gate's way, south of the Delve's door. Twenty-three trades among them.
pub const SCHOOLS: [School; 7] = [
    School { power: "bomrekedan:kekkit", at: [3.4, -12.8], teaches: &[Trade::Quarryman, Trade::Woodsman, Trade::Sawyer] },
    School { power: "bomrekedan:machines", at: [5.9, -12.8], teaches: &[Trade::Labourer, Trade::Roper, Trade::Wheelwright, Trade::Carter, Trade::Ostler] },
    School { power: "bomrekedan:steel", at: [8.4, -12.8], teaches: &[Trade::Smith, Trade::Bloomer, Trade::LimeBurner, Trade::Collier] },
    School { power: "bomrekedan:still", at: [10.9, -12.8], teaches: &[Trade::Brewer, Trade::Baker, Trade::Miller, Trade::Herder] },
    School { power: "bomrekedan:ratfolk", at: [4.6, -10.5], teaches: &[Trade::Setter, Trade::BankerMason, Trade::Bargeman] },
    School { power: "bomrekedan:erinys", at: [7.4, -10.5], teaches: &[Trade::ClerkOfWorks, Trade::MasterMason] },
    School { power: "bomrekedan:ivy", at: [10.2, -10.5], teaches: &[Trade::Carpenter, Trade::Tanner] },
];

/// The school that teaches `trade`.
pub fn school_of(trade: Trade) -> Option<&'static School> {
    SCHOOLS.iter().find(|s| s.teaches.contains(&trade))
}

/// What a knight must already be to be taught `trade`, or none: the Master
/// Mason draws the plans, and a knight who cannot yet lay and cut is not
/// shown them.
pub fn needs(trade: Trade) -> &'static [(Trade, Rank)] {
    match trade {
        Trade::MasterMason => &[(Trade::Setter, Rank::Journeyman), (Trade::BankerMason, Rank::Journeyman)],
        _ => &[],
    }
}

/// How long each trade a teacher offers is shown before the next, ticks.
pub const OFFER_TICKS: u64 = 75;

/// How long E is held while a trade is offered to take it up, ticks.
pub const LEARN_HOLD: u32 = 30;

/// A knight's trades as the run reads them: the realm's count and the marks
/// not yet settled.
#[derive(Clone, Copy)]
pub struct Reading<'a> {
    pub home: &'a Ledger,
    pub marks: &'a BTreeMap<String, u32>,
}

impl Reading<'_> {
    pub fn deeds(&self, who: &str, trade: Trade) -> u32 {
        self.home.deeds(who, trade).saturating_add(self.marks.get(&mark_key(trade, who)).copied().unwrap_or(0))
    }

    pub fn rank(&self, who: &str, trade: Trade) -> Option<Rank> {
        Rank::of(self.deeds(who, trade))
    }

    pub fn holds(&self, who: &str, trade: Trade) -> bool {
        self.deeds(who, trade) > 0
    }

    /// The trades `who` holds, most deeds first (the first of a tie by the
    /// economy's order).
    pub fn held(&self, who: &str) -> Vec<(Trade, u32)> {
        let mut out: Vec<(Trade, u32)> = Trade::ALL.into_iter().map(|t| (t, self.deeds(who, t))).filter(|&(_, n)| n > 0).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1));
        out
    }

    /// Whether `who` has what a teacher asks before they teach `trade`.
    pub fn ready_for(&self, who: &str, trade: Trade) -> bool {
        needs(trade).iter().all(|&(t, rank)| self.rank(who, t).is_some_and(|r| r >= rank))
    }

    /// How much keener a smith keeps `hero`'s blade, percent.
    pub fn edge(&self, hero: &Hero) -> u32 {
        keen_edge(self.rank(&super::home::Home::profile_key(hero), Trade::Smith))
    }

    /// The best a knight's masonry is: the cutter's, the setter's and the
    /// Master Mason's, whichever stands highest.
    pub fn mason(&self, who: &str) -> Option<Rank> {
        [Trade::BankerMason, Trade::Setter, Trade::MasterMason].into_iter().filter_map(|t| self.rank(who, t)).max()
    }
}

/// How far a mason's eye reaches below, in the Delve's units: an Apprentice
/// knows a hollow wall only by laying a hand on it, a Journeyman hears one
/// across a room, a Master across the hall.
pub fn eye_reach(rank: Option<Rank>) -> f32 {
    match rank {
        None => 0.0,
        Some(Rank::Apprentice) => 3.0,
        Some(Rank::Journeyman) => 9.0,
        Some(Rank::Master) => 16.0,
    }
}

/// How much keener a smith keeps the blade, in percent of its damage.
pub fn keen_edge(rank: Option<Rank>) -> u32 {
    match rank {
        None => 0,
        Some(Rank::Apprentice) => 3,
        Some(Rank::Journeyman) => 6,
        Some(Rank::Master) => 10,
    }
}

/// How much stouter a carpenter braces a barricade, in percent of its
/// strength.
pub fn brace(rank: Option<Rank>) -> u32 {
    match rank {
        None => 0,
        Some(Rank::Apprentice) => 10,
        Some(Rank::Journeyman) => 20,
        Some(Rank::Master) => 40,
    }
}

impl Run {
    /// The run's reading of the knights' trades.
    pub fn trades(&self) -> Reading<'_> {
        Reading { home: &self.home.trades, marks: &self.marks }
    }

    /// The name the realm keeps knight `id`'s trades under.
    pub fn trade_key(&self, id: u32) -> Option<String> {
        self.players.get(&id).map(super::home::Home::profile_key)
    }

    /// Knight `id`'s deeds in `trade`.
    pub fn deeds_of(&self, id: u32, trade: Trade) -> u32 {
        self.trade_key(id).map_or(0, |who| self.trades().deeds(&who, trade))
    }

    /// Knight `id`'s rank in `trade`, if they hold it.
    pub fn rank_of(&self, id: u32, trade: Trade) -> Option<Rank> {
        Rank::of(self.deeds_of(id, trade))
    }

    /// Mark `n` deeds in `trade` for knight `id`, to be settled into the
    /// realm's record. Says so when the deed makes a new rank.
    pub fn earn_deeds(&mut self, id: u32, trade: Trade, n: u32) {
        let Some(who) = self.trade_key(id) else { return };
        if n == 0 {
            return;
        }
        let before = Rank::of(self.trades().deeds(&who, trade));
        let key = mark_key(trade, &who);
        if !self.marks.contains_key(&key) && self.marks.len() >= 2000 {
            return;
        }
        let mark = self.marks.entry(key).or_default();
        *mark = mark.saturating_add(n);
        let after = Rank::of(self.trades().deeds(&who, trade));
        if after != before
            && let Some(rank) = after
        {
            let said = if before.is_none() {
                format!("{} TAKES UP THE {}", who.to_uppercase(), trade.name())
            } else {
                format!("{} IS NOW A {} {}", who.to_uppercase(), rank.name(), trade.name())
            };
            self.found = Some((self.tick, id, said));
            self.sounds.push("level_up");
        }
    }

    /// A carpenter's brace for knight `id`, percent.
    pub(super) fn brace_for(&self, id: u32) -> u32 {
        self.trade_key(id).map_or(0, |who| brace(self.trades().rank(&who, Trade::Carpenter)))
    }

    /// A mason's eye for knight `id`, in the Delve's units.
    pub(super) fn eye_for(&self, id: u32) -> f32 {
        self.trade_key(id).map_or(0.0, |who| eye_reach(self.trades().mason(&who)))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/trades__tests.rs"]
mod tests;
