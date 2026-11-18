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

pub const JOURNEYMAN: u32 = 30;
pub const MASTER: u32 = 200;

pub const MAX_KNIGHTS: usize = 64;
pub const MAX_DEEDS: u32 = 1_000_000;

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

    pub fn name(self) -> String {
        self.id().replace('-', " ").to_uppercase()
    }

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

    pub const LOAD_SHIFT: u8 = 5;

    pub fn pack(self, load: u8) -> u8 {
        self.code() | (load.min(2) << Tool::LOAD_SHIFT)
    }

    pub fn unpack(code: u8) -> Option<(Tool, u8)> {
        Tool::from_code(code & ((1 << Tool::LOAD_SHIFT) - 1)).map(|t| (t, code >> Tool::LOAD_SHIFT))
    }
}

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

    pub fn of(deeds: u32) -> Option<Rank> {
        match deeds {
            0 => None,
            d if d >= MASTER => Some(Rank::Master),
            d if d >= JOURNEYMAN => Some(Rank::Journeyman),
            _ => Some(Rank::Apprentice),
        }
    }

    pub fn next_at(deeds: u32) -> Option<u32> {
        match Rank::of(deeds) {
            None => Some(1),
            Some(Rank::Apprentice) => Some(JOURNEYMAN),
            Some(Rank::Journeyman) => Some(MASTER),
            Some(Rank::Master) => None,
        }
    }

    pub fn grace(self) -> u32 {
        match self {
            Rank::Apprentice => 3,
            Rank::Journeyman => 4,
            Rank::Master => 5,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Knack(pub BTreeMap<String, u32>);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ledger(pub BTreeMap<String, Knack>);

impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn valid(&self) -> bool {
        self.0.len() <= MAX_KNIGHTS
            && self.0.iter().all(|(who, knack)| {
                !who.is_empty() && who.len() <= 64 && knack.0.iter().all(|(id, &n)| Trade::from_id(id).is_some() && n <= MAX_DEEDS)
            })
    }

    pub fn deeds(&self, who: &str, trade: Trade) -> u32 {
        self.0.get(who).and_then(|k| k.0.get(trade.id())).copied().unwrap_or(0)
    }

    pub fn earn(&mut self, who: &str, trade: Trade, n: u32) {
        if self.0.len() >= MAX_KNIGHTS && !self.0.contains_key(who) {
            return;
        }
        let at = self.0.entry(who.to_string()).or_default().0.entry(trade.id().to_string()).or_default();
        *at = at.saturating_add(n).min(MAX_DEEDS);
    }

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

pub fn mark_key(trade: Trade, who: &str) -> String {
    format!("trade:{}:{}", trade.id(), who)
}

pub fn split_mark(key: &str) -> Option<(Trade, &str)> {
    let rest = key.strip_prefix("trade:")?;
    let (id, who) = rest.split_once(':')?;
    Some((Trade::from_id(id)?, who))
}

pub struct School {
    pub power: &'static str,
    pub at: [f32; 2],
    pub teaches: &'static [Trade],
}

pub const SCHOOLS: [School; 7] = [
    School { power: "bomrekedan:kekkit", at: [3.4, -12.8], teaches: &[Trade::Quarryman, Trade::Woodsman, Trade::Sawyer] },
    School { power: "bomrekedan:machines", at: [5.9, -12.8], teaches: &[Trade::Labourer, Trade::Roper, Trade::Wheelwright, Trade::Carter, Trade::Ostler] },
    School { power: "bomrekedan:steel", at: [8.4, -12.8], teaches: &[Trade::Smith, Trade::Bloomer, Trade::LimeBurner, Trade::Collier] },
    School { power: "bomrekedan:still", at: [10.9, -12.8], teaches: &[Trade::Brewer, Trade::Baker, Trade::Miller, Trade::Herder] },
    School { power: "bomrekedan:ratfolk", at: [4.6, -10.5], teaches: &[Trade::Setter, Trade::BankerMason, Trade::Bargeman] },
    School { power: "bomrekedan:erinys", at: [7.4, -10.5], teaches: &[Trade::ClerkOfWorks, Trade::MasterMason] },
    School { power: "bomrekedan:ivy", at: [10.2, -10.5], teaches: &[Trade::Carpenter, Trade::Tanner] },
];

pub fn school_of(trade: Trade) -> Option<&'static School> {
    SCHOOLS.iter().find(|s| s.teaches.contains(&trade))
}

pub fn needs(trade: Trade) -> &'static [(Trade, Rank)] {
    match trade {
        Trade::MasterMason => &[(Trade::Setter, Rank::Journeyman), (Trade::BankerMason, Rank::Journeyman)],
        _ => &[],
    }
}

pub const OFFER_TICKS: u64 = 75;

pub const LEARN_HOLD: u32 = 30;

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

    pub fn held(&self, who: &str) -> Vec<(Trade, u32)> {
        let mut out: Vec<(Trade, u32)> = Trade::ALL.into_iter().map(|t| (t, self.deeds(who, t))).filter(|&(_, n)| n > 0).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1));
        out
    }

    pub fn ready_for(&self, who: &str, trade: Trade) -> bool {
        needs(trade).iter().all(|&(t, rank)| self.rank(who, t).is_some_and(|r| r >= rank))
    }

    pub fn edge(&self, hero: &Hero) -> u32 {
        keen_edge(self.rank(&super::home::Home::profile_key(hero), Trade::Smith))
    }

    pub fn mason(&self, who: &str) -> Option<Rank> {
        [Trade::BankerMason, Trade::Setter, Trade::MasterMason].into_iter().filter_map(|t| self.rank(who, t)).max()
    }
}

pub fn eye_reach(rank: Option<Rank>) -> f32 {
    match rank {
        None => 0.0,
        Some(Rank::Apprentice) => 3.0,
        Some(Rank::Journeyman) => 9.0,
        Some(Rank::Master) => 16.0,
    }
}

pub fn keen_edge(rank: Option<Rank>) -> u32 {
    match rank {
        None => 0,
        Some(Rank::Apprentice) => 3,
        Some(Rank::Journeyman) => 6,
        Some(Rank::Master) => 10,
    }
}

pub fn brace(rank: Option<Rank>) -> u32 {
    match rank {
        None => 0,
        Some(Rank::Apprentice) => 10,
        Some(Rank::Journeyman) => 20,
        Some(Rank::Master) => 40,
    }
}

impl Run {
    pub fn trades(&self) -> Reading<'_> {
        Reading { home: &self.home.trades, marks: &self.marks }
    }

    pub fn trade_key(&self, id: u32) -> Option<String> {
        self.players.get(&id).map(super::home::Home::profile_key)
    }

    pub fn deeds_of(&self, id: u32, trade: Trade) -> u32 {
        self.trade_key(id).map_or(0, |who| self.trades().deeds(&who, trade))
    }

    pub fn rank_of(&self, id: u32, trade: Trade) -> Option<Rank> {
        Rank::of(self.deeds_of(id, trade))
    }

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

    pub(super) fn brace_for(&self, id: u32) -> u32 {
        self.trade_key(id).map_or(0, |who| brace(self.trades().rank(&who, Trade::Carpenter)))
    }

    pub(super) fn eye_for(&self, id: u32) -> f32 {
        self.trade_key(id).map_or(0.0, |who| eye_reach(self.trades().mason(&who)))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/trades__tests.rs"]
mod tests;
