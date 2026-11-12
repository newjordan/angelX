//! Wayfarers: the travellers on the kingdom's roads.
//!
//! A party that keeps walking the roads is met, now and then, by somebody
//! coming the other way: a pilgrim with a blessing left to spend, a peddler
//! with a cart, a minstrel who has heard of the party, a toll gang with a
//! rope across the road, a squire who has lost her company, a drover with
//! a tale. They are seeded, named and walk the road graph (`world::roads`,
//! read-only) toward the knights, then stop and wait. E beside one opens a
//! talk on the knight's own screen, as a village of the Deep does
//! (`deep::village`): their greeting, then what they offer, answered with
//! 1, 2 and 3. Every answer does something the words said it would.
//!
//! Rarity is a clock, not a dice roll: a traveller is sent after so many
//! seconds of the party walking a road, the same kind never twice running,
//! and only one is ever on the road. Who was met is kept in the realm
//! (`Ledger`, written through the marks at the save).
//!
//! The party's gold is the realm's treasury: a price is a mark the save
//! charges (`PAID`), and the run counts the marks not yet charged so a
//! second purchase cannot spend the same coin.

use super::activity::{Realm, Settled};
use super::map::{self, roads};
use super::villagers::{MAX_VILLAGERS, Trade, Villager};
use super::*;
use std::sync::OnceLock;

/// Seconds of the party walking a road before the first traveller of a
/// run, and between the rest.
pub const FIRST_AFTER: u64 = 20 * HZ as u64;
pub const THEN_AFTER: u64 = 75 * HZ as u64;
/// How far along the road ahead of the party a traveller is first seen
/// coming (tiles), and how near the knights it must not start (tiles).
pub const AHEAD: f32 = 34.0;
pub const OUT_OF_SIGHT: f32 = 15.0;
/// A traveller's walk (two tiles a second, as the realm's folk), how near a knight must stand to
/// speak with it (tiles), how far the knights may be before it gives up
/// the road, and how long it waits for someone to speak.
pub const WALK: f32 = 2.0;
pub const REACH: f32 = 3.2;
pub const LOSE: f32 = 40.0;
pub const IDLE: u32 = 45 * HZ;
/// How long a traveller who has said its piece takes to go out of sight.
pub const LEAVE: u64 = 10 * HZ as u64;
/// How near the Village's door the squire must be led to be home (tiles).
pub const HOME_REACH: f32 = 5.0;

/// A pilgrim's blessing: this much more damage and pace, for this long on
/// the whole party, or for this long on one knight who asks it for
/// themselves (and a full mend besides).
pub const BLESS_PERCENT: u32 = 15;
pub const BLESS_TICKS: u32 = 60 * HZ;
pub const BLESS_ALONE_TICKS: u32 = 180 * HZ;
/// The peddler's prices, in gold, and how many things are on his cart.
pub const POTION: u32 = 26;
pub const BOMB: u32 = 18;
pub const SCROLL: u32 = 34;
pub const STOCK: u8 = 4;
/// A song mends each knight by this share of their health and carries the
/// party's name this far (renown); a tip for it costs this much gold and
/// carries it further.
pub const SONG_MEND: u32 = 20;
pub const SONG_RENOWN: u32 = 10;
pub const TIP: u32 = 12;
pub const TIP_RENOWN: u32 = 30;
/// The toll, the purse of a gang beaten, and what the drover's better
/// tale costs; and the renown a true rumour pays when its place is reached.
pub const TOLL: u32 = 40;
pub const PURSE: u32 = 60;
pub const BETTER_TALE: u32 = 15;
pub const RUMOUR_RENOWN: u32 = 15;
/// The gang that draws steel: bodies of the dungeon's own roster, at the
/// health they have anywhere.
pub const GANG: [EnemyKind; 3] = [EnemyKind::Pavise, EnemyKind::Hob, EnemyKind::Skeleton];

/// The marks the save reads: gold the road is owed by the treasury, gold
/// the treasury is owed by the road, who was met, and a squire home.
pub const PAID: &str = "wayfarer:paid";
pub const OWED: &str = "wayfarer:purse";
const MET: &str = "wayfarer:met:";
const SQUIRE: &str = "wayfarer:squire:";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Pilgrim,
    Merchant,
    Troubadour,
    Tollkeeper,
    Squire,
    Gossip,
}

pub const KINDS: [Kind; 6] = [
    Kind::Pilgrim,
    Kind::Merchant,
    Kind::Troubadour,
    Kind::Tollkeeper,
    Kind::Squire,
    Kind::Gossip,
];

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Pilgrim => "pilgrim",
            Kind::Merchant => "peddler",
            Kind::Troubadour => "minstrel",
            Kind::Tollkeeper => "toll gang",
            Kind::Squire => "lost squire",
            Kind::Gossip => "drover",
        }
    }

    pub fn from_word(word: &str) -> Option<Kind> {
        KINDS.into_iter().find(|k| k.word() == word)
    }

    /// What they are called before their name.
    pub fn title(self) -> &'static str {
        match self {
            Kind::Pilgrim => "Brother",
            Kind::Merchant => "Peddler",
            Kind::Troubadour => "Minstrel",
            Kind::Tollkeeper => "Reeve",
            Kind::Squire => "Squire",
            Kind::Gossip => "Drover",
        }
    }

    fn hello(self) -> &'static str {
        match self {
            Kind::Pilgrim => "Peace on the road, knights. I carry a candle to the Chapel, and one blessing I have not yet spent.",
            Kind::Merchant => "Salt, string and a few things that go bang. Roadside prices, knights, and I do not haggle.",
            Kind::Troubadour => "A song for the road, knights? I have heard a good deal about you in the taverns.",
            Kind::Tollkeeper => "Hold there, knights! This road is tolled, and the rope stays up till it is paid.",
            Kind::Squire => "Please, knights! I lost my company on the road and my feet are done. Is there a town near?",
            Kind::Gossip => "Mind the ruts. I have driven cattle past half the realm, and you hear things.",
        }
    }

    fn ask(self) -> &'static str {
        match self {
            Kind::Pilgrim => "I may spend the blessing only once. On whom?",
            Kind::Merchant => "What will it be? I take the realm's gold.",
            Kind::Troubadour => "Shall I sing what I have heard?",
            Kind::Tollkeeper => "Pay, or show us you are someone to fear. Whoever has felled a guardian walks free. Or try our steel.",
            Kind::Squire => "Take me to the Village, and the realm will find me a bed. I will not forget it.",
            Kind::Gossip => "Two kinds of talk: what I know for nothing, and what I know for a price.",
        }
    }
}

const NAMES: [&str; 24] = [
    "Aldous", "Berenice", "Corwin", "Dilys", "Eadric", "Fenella", "Garrick", "Hesper", "Ivor", "Joss",
    "Kestrel", "Lowen", "Merrin", "Nesta", "Osric", "Perrin", "Quilla", "Rhodri", "Sorrel", "Tamsin",
    "Ulric", "Verity", "Wystan", "Yarrow",
];

fn mix(seed: u64, n: u32) -> u64 {
    let mut z = seed ^ (u64::from(n).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Where a traveller is in its errand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Coming along the road toward the knights, or waiting for one.
    Walking,
    /// Said its piece: going off along the road, since this tick.
    Leaving(u64),
    /// The squire, at the knights' heels toward the Village.
    Following,
    /// The gang, steel drawn, since this tick.
    Fighting(u64),
}

/// The one traveller on the road.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wayfarer {
    pub kind: Kind,
    pub name: String,
    /// Arena units, the feet.
    pub x: f32,
    pub y: f32,
    pub east: bool,
    /// Which road (`world::roads::ROADS`), how far along its course
    /// (tiles) and which way it is going (1 or -1).
    pub road: u16,
    pub along: f32,
    pub dir: i8,
    pub born: u64,
    pub stage: Stage,
    /// The last tick it took a step, for its stride.
    #[serde(default)]
    pub moved: u64,
    /// Ticks stood waiting for somebody to speak.
    #[serde(default)]
    pub still: u32,
    /// A knight has spoken with it; it is in the realm's ledger.
    #[serde(default)]
    pub met: bool,
    /// It has done what it came to do and leaves when the talk is over.
    #[serde(default)]
    pub done: bool,
    /// Things sold from the cart.
    #[serde(default)]
    pub sold: u8,
    /// The gang's bodies in the room.
    #[serde(default)]
    pub gang: Vec<u32>,
}

impl Wayfarer {
    /// Its name as the plate over it reads.
    pub fn plate(&self) -> String {
        match self.kind {
            Kind::Tollkeeper => format!("{} {} and Co.", self.kind.title(), self.name),
            _ => format!("{} {}", self.kind.title(), self.name),
        }
        .to_uppercase()
    }

    /// Whether a knight may speak with it now.
    pub fn talkable(&self) -> bool {
        matches!(self.stage, Stage::Walking | Stage::Following)
    }

    /// Whether it is moving this tick, for its stride.
    pub fn walking(&self, tick: u64) -> bool {
        tick.saturating_sub(self.moved) <= 1
    }
}

/// A knight's talk with the traveller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Talk {
    /// 0: their greeting; 1: what they offer; 2: what came of it.
    pub page: u8,
    pub opened: u64,
    /// The last thing that came of an answer.
    #[serde(default)]
    pub said: String,
}

/// The road's traffic, in the run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Ticks of the party walking a road since the last traveller: a clock
    /// of the moment, never saved nor sent.
    #[serde(skip)]
    pub walked: u64,
    /// Travellers sent this run: with the realm's ledger, the seed of the
    /// next one's name and kind.
    pub sent: u32,
    /// The kind sent last, never sent again straight after.
    pub last: Option<Kind>,
    pub on: Option<Wayfarer>,
    pub talks: BTreeMap<u32, Talk>,
    /// The way the knights on the road were last walking, for the same.
    #[serde(skip)]
    pub heading: (i8, i8),
    /// The place the last rumour marked, until a knight steps into it.
    pub rumoured: Option<RoomKind>,
    /// The answer key each knight holds, so one press is one answer.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<u32, u8>,
}

impl State {
    /// Nothing on the road, nobody talking: saves and streams stay as they
    /// were (the clock is not written).
    pub fn quiet(&self) -> bool {
        self.sent == 0 && self.last.is_none() && self.on.is_none() && self.talks.is_empty() && self.rumoured.is_none() && self.keys.is_empty()
    }

    pub fn valid(&self) -> bool {
        self.talks.len() <= MAX_PLAYERS as usize
            && self.talks.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.talks.values().all(|t| t.page <= 2 && t.said.len() <= 256)
            && self.keys.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.on.as_ref().is_none_or(|w| {
                w.x.is_finite()
                    && w.y.is_finite()
                    && w.along.is_finite()
                    && w.name.len() <= 24
                    && w.gang.len() <= GANG.len()
                    && w.sold <= STOCK
                    && w.dir.abs() == 1
            })
    }
}

/// What the realm remembers of the roads: whom the party has met, by kind,
/// and the latest of them, newest last. Written through the marks at the
/// save (`settle`), never by the run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub met: BTreeMap<String, u32>,
    pub lately: Vec<String>,
}

/// The most names the ledger keeps.
pub const LATELY: usize = 12;

impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.met.is_empty() && self.lately.is_empty()
    }

    pub fn total(&self) -> u32 {
        self.met.values().sum()
    }

    pub fn valid(&self) -> bool {
        self.met.len() <= KINDS.len()
            && self.met.keys().all(|k| Kind::from_word(k).is_some())
            && self.lately.len() <= LATELY
            && self.lately.iter().all(|l| l.len() <= 40)
    }

    fn record(&mut self, kind: Kind, name: &str, n: u32) {
        *self.met.entry(kind.word().to_string()).or_default() += n;
        self.lately.push(format!("{} {}", kind.title(), name));
        let over = self.lately.len().saturating_sub(LATELY);
        self.lately.drain(..over);
    }

    /// The kind met last, if the ledger remembers.
    pub fn last(&self) -> Option<Kind> {
        let line = self.lately.last()?;
        KINDS.into_iter().find(|k| line.starts_with(k.title()))
    }
}

/// A road's course as the walkers walk it: the tile centres, smoothed so a
/// diagonal is not a stair, and the distance along (tiles).
pub struct Path {
    pts: Vec<(f32, f32)>,
    cum: Vec<f32>,
}

impl Path {
    pub fn len(&self) -> f32 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    pub fn is_empty(&self) -> bool {
        self.pts.len() < 2
    }

    /// The point `along` tiles in (tile coordinates).
    pub fn at(&self, along: f32) -> (f32, f32) {
        let along = along.clamp(0.0, self.len());
        let i = self.cum.partition_point(|&c| c <= along).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let span = (self.cum[i] - self.cum[i - 1]).max(1e-4);
        let k = ((along - self.cum[i - 1]) / span).clamp(0.0, 1.0);
        (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
    }

    /// How far along the course the point nearest `(x, y)` (tiles) lies,
    /// and how far off it.
    pub fn nearest(&self, (x, y): (f32, f32)) -> (f32, f32) {
        self.pts
            .iter()
            .zip(&self.cum)
            .map(|(p, &c)| (c, (p.0 - x).hypot(p.1 - y)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0.0, f32::MAX))
    }
}

/// Every road, in `ROADS` order.
pub fn paths() -> &'static [Path] {
    static PATHS: OnceLock<Vec<Path>> = OnceLock::new();
    PATHS.get_or_init(|| {
        roads::ROADS
            .iter()
            .map(|road| {
                let tiles = if road.points.len() >= 2 { roads::course(road.points) } else { Vec::new() };
                let pts: Vec<(f32, f32)> = (0..tiles.len())
                    .map(|i| {
                        let (lo, hi) = (i.saturating_sub(3), (i + 3).min(tiles.len() - 1));
                        let n = (hi - lo + 1) as f32;
                        let (sx, sy) = tiles[lo..=hi].iter().fold((0.0, 0.0), |a, t| (a.0 + t.0 as f32, a.1 + t.1 as f32));
                        (sx / n + 0.5, sy / n + 0.5)
                    })
                    .collect();
                let mut cum = vec![0.0];
                for w in pts.windows(2) {
                    cum.push(cum[cum.len() - 1] + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1));
                }
                if pts.len() < 2 {
                    return Path { pts: vec![(0.0, 0.0), (1.0, 0.0)], cum: vec![0.0, 1.0] };
                }
                Path { pts, cum }
            })
            .collect()
    })
}

/// A choice on offer to a knight in a talk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Bless,
    Mend,
    Potion,
    Bomb,
    Scroll,
    Sing,
    Tip,
    Toll,
    Deeds,
    Steel,
    Follow,
    Hear,
    Better,
}

/// What a knight is shown in a talk: who, their words, the answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Speech {
    pub kind: Kind,
    pub who: String,
    pub words: String,
    pub answers: Vec<String>,
}

fn pay_label(what: &str, gold: u32) -> String {
    format!("{what}  {gold} GOLD")
}

impl Run {
    /// The traveller knight `id` could speak with right now, for the key
    /// the screen shows beside them.
    pub fn wayfarer_in_reach(&self, id: u32) -> Option<&Wayfarer> {
        let w = self.wayfarers.on.as_ref().filter(|w| w.talkable())?;
        let h = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone)?;
        ((h.x - w.x).hypot(h.y - w.y) < REACH * TILE_UNITS && !self.wayfarers.talks.contains_key(&id)).then_some(w)
    }

    /// The gold the treasury can still spare the road: what it holds, less
    /// what the road has been promised and the save has not yet charged.
    pub fn gold_spare(&self) -> u32 {
        self.treasury
            .get(Spoil::Gold)
            .saturating_sub(self.marks.get(PAID).copied().unwrap_or(0))
    }

    fn pay_road(&mut self, gold: u32) -> bool {
        if self.gold_spare() < gold {
            return false;
        }
        *self.marks.entry(PAID.to_string()).or_default() += gold;
        true
    }

    /// The knights standing: where each is (units).
    fn knight_spots(&self) -> Vec<(f32, f32)> {
        self.players
            .values()
            .filter(|h| h.hp > 0 && !h.stone && !h.absent && !h.away)
            .map(|h| (h.x, h.y))
            .collect()
    }

    /// Where the realm has a bed for a squire.
    fn squire_bed(&self) -> Option<String> {
        self.home
            .holdings
            .owned
            .iter()
            .find(|w| w.steward.is_some() && w.raid == 0 && w.folk.len() < w.standing().min(MAX_VILLAGERS))
            .map(|w| w.deed().map_or_else(|| w.key.clone(), super::holdings::Deed::name))
    }

    /// The Village's door, if the kingdom has one.
    fn village_stand(geo: &map::Geography) -> Option<(i32, i32)> {
        geo.sites.iter().find(|s| s.room == RoomKind::Village).map(|s| s.stand)
    }

    /// The places a drover might tell of: not yet stepped into this outing,
    /// nearest to `from` first.
    fn unseen_places<'g>(&self, geo: &'g map::Geography, from: (f32, f32)) -> Vec<&'g map::Site> {
        let mut sites: Vec<&map::Site> = geo
            .sites
            .iter()
            .filter(|s| !matches!(s.room, RoomKind::Gate | RoomKind::RealmHall) && !self.kingdom.seen.contains(&s.room))
            .collect();
        let far = |s: &&map::Site| {
            let (x, y) = ((s.stand.0 as f32 + 0.5) * TILE_UNITS, (s.stand.1 as f32 + 0.5) * TILE_UNITS);
            (from.0 - x).hypot(from.1 - y)
        };
        sites.sort_by(|a, b| far(a).total_cmp(&far(b)));
        sites
    }

    /// The guardian the party has felled most, if it has felled any.
    fn top_guardian(&self) -> Option<(&'static str, u32)> {
        self.home
            .trophies
            .iter()
            .filter(|&(_, &n)| n > 0)
            .filter_map(|(id, &n)| super::trophies::PLINTHS.iter().find(|p| p.id == id).map(|p| (p.name, n)))
            .max_by_key(|&(name, n)| (n, std::cmp::Reverse(name)))
    }

    /// What the party has done, as a minstrel would sing it: the guardian
    /// felled most, the deepest floor, the monsters felled. At most two.
    pub fn verses(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some((name, n)) = self.top_guardian() {
            let times = if n == 1 { "once".to_string() } else { format!("{n} times") };
            out.push(format!("{name}, felled {times}"));
        }
        if self.home.deepest > 0 {
            out.push(format!("floor {}, reached", self.home.deepest));
        }
        let slain: u32 = self.home.bestiary.values().sum();
        if slain > 0 {
            out.push(format!("{slain} monsters felled"));
        }
        out.truncate(2);
        out
    }

    /// Whether a traveller of `kind` could be sent to the party now.
    fn could_send(&self, kind: Kind, geo: &map::Geography, from: (f32, f32)) -> bool {
        match kind {
            Kind::Pilgrim | Kind::Merchant => true,
            Kind::Troubadour => !self.verses().is_empty(),
            Kind::Tollkeeper => self.home.deepest >= 1,
            Kind::Squire => self.squire_bed().is_some() && Self::village_stand(geo).is_some(),
            Kind::Gossip => !self.unseen_places(geo, from).is_empty(),
        }
    }

    /// The clock: the party walking a road counts toward the next
    /// traveller, only while none is on the road.
    fn road_clock(&mut self, inputs: &BTreeMap<u32, Input>, geo: &map::Geography) {
        if self.wayfarers.on.is_some() {
            return;
        }
        let mut heading = (0i32, 0i32);
        let mut walking = false;
        for (id, hero) in &self.players {
            let Some(input) = inputs.get(id).filter(|i| i.valid() && (i.move_x != 0 || i.move_y != 0)) else {
                continue;
            };
            if hero.hp > 0 && !hero.stone && !hero.away && super::kingdom::on_road(geo, hero.x, hero.y) {
                walking = true;
                heading.0 += i32::from(input.move_x);
                heading.1 += i32::from(input.move_y);
            }
        }
        if walking {
            self.wayfarers.walked += 1;
            self.wayfarers.heading = (heading.0.signum() as i8, heading.1.signum() as i8);
        }
    }

    /// Send the next traveller, when the clock says so: a seeded kind that
    /// is not the last and a seeded name, on the road ahead of the party and
    /// out of its sight, walking toward it.
    fn maybe_send(&mut self, geo: &map::Geography) {
        let due = if self.wayfarers.sent == 0 { FIRST_AFTER } else { THEN_AFTER };
        if self.wayfarers.on.is_some() || self.wayfarers.walked < due || self.tick % 15 != 0 {
            return;
        }
        let spots = self.knight_spots();
        // The knight on a road who sets the scene: the first by seat.
        let lead = self
            .players
            .values()
            .find(|h| h.hp > 0 && !h.stone && !h.away && !h.absent && super::kingdom::on_road(geo, h.x, h.y))
            .map(|h| (h.x, h.y));
        let Some(lead) = lead else {
            return;
        };
        let n = self.wayfarers.sent.wrapping_add(self.home.wayfarers.total());
        let roll = mix(self.seed ^ 0x5741_5946_4152_4552, n);
        let last = self.wayfarers.last.or_else(|| self.home.wayfarers.last());
        let kinds: Vec<Kind> = KINDS.into_iter().filter(|&k| Some(k) != last && self.could_send(k, geo, lead)).collect();
        let Some(&kind) = kinds.get((roll >> 8) as usize % kinds.len().max(1)) else {
            return;
        };
        let name = NAMES[(roll >> 24) as usize % NAMES.len()].to_string();
        // The road nearest the lead knight, and the way to walk it.
        let tiles = (lead.0 / TILE_UNITS, lead.1 / TILE_UNITS);
        let Some((road, at)) = paths()
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.nearest(tiles)))
            .filter(|(_, (_, off))| *off <= 4.0)
            .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
            .map(|(i, (along, _))| (i, along))
        else {
            return;
        };
        let path = &paths()[road];
        // Ahead of where the knights are going; if they stand, a seeded side.
        let tangent = {
            let (a, b) = (path.at(at - 2.0), path.at(at + 2.0));
            (b.0 - a.0, b.1 - a.1)
        };
        let (hx, hy) = self.wayfarers.heading;
        let pull = tangent.0 * f32::from(hx) + tangent.1 * f32::from(hy);
        let side: f32 = if pull.abs() > 0.2 {
            pull.signum()
        } else if roll & 1 == 0 {
            1.0
        } else {
            -1.0
        };
        // The full way ahead if the road has it, else as much as it has.
        let tries = [AHEAD, 28.0, 22.0, 17.0].into_iter().flat_map(|d| [side * d, -side * d]);
        let start = tries.into_iter().find_map(|s| {
            let along = at + s;
            let (px, py) = path.at(along);
            let seen = spots.iter().any(|&(kx, ky)| (kx / TILE_UNITS - px).hypot(ky / TILE_UNITS - py) < OUT_OF_SIGHT);
            let floor = geo.room.tile(px.floor() as i32, py.floor() as i32) == Tile::Floor;
            ((0.0..=path.len()).contains(&along) && !seen && floor).then_some((along, if s > 0.0 { -1i8 } else { 1i8 }))
        });
        let Some((along, dir)) = start else {
            return;
        };
        let (px, py) = path.at(along);
        self.wayfarers.sent += 1;
        self.wayfarers.last = Some(kind);
        self.sounds.push("play_card");
        let come = Wayfarer {
            kind,
            name,
            x: px * TILE_UNITS,
            y: py * TILE_UNITS,
            east: dir > 0,
            road: road as u16,
            along,
            dir,
            born: self.tick,
            stage: Stage::Walking,
            moved: 0,
            still: 0,
            met: false,
            done: false,
            sold: 0,
            gang: Vec::new(),
        };
        self.found = Some((self.tick, 0, format!("{} COMES ALONG THE ROAD", come.plate())));
        self.wayfarers.on = Some(come);
    }

    /// One tick of the road in the kingdom (`activity::FOLLIES`).
    pub(super) fn tick_wayfarers(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Kingdom {
            // Indoors or below, the road forgets everyone who was not
            // following the party; a gang's fight ended with the room.
            self.wayfarers.talks.clear();
            self.wayfarers.keys.clear();
            if self.wayfarers.on.as_ref().is_some_and(|w| w.stage != Stage::Following) {
                self.wayfarers.on = None;
                self.wayfarers.walked = 0;
            }
            return;
        }
        let geo = map::geography(&self.kingdom.wishes);
        self.road_clock(inputs, &geo);
        self.maybe_send(&geo);
        self.move_wayfarer(&geo);
        // Whoever has walked off has ended their talk.
        let far: Vec<u32> = self
            .wayfarers
            .talks
            .keys()
            .copied()
            .filter(|&id| {
                let (Some(w), Some(h)) = (&self.wayfarers.on, self.players.get(&id)) else {
                    return true;
                };
                h.hp == 0 || (h.x - w.x).hypot(h.y - w.y) > REACH * 1.7 * TILE_UNITS
            })
            .collect();
        for id in far {
            self.wayfarers.talks.remove(&id);
        }
    }

    fn move_wayfarer(&mut self, geo: &map::Geography) {
        let Some(mut w) = self.wayfarers.on.take() else {
            return;
        };
        let spots = self.knight_spots();
        let tick = self.tick;
        let nearest = spots
            .iter()
            .map(|&(x, y)| ((x - w.x).hypot(y - w.y) / TILE_UNITS, (x, y)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let step = WALK / HZ as f32;
        let path = paths().get(usize::from(w.road));
        let mut keep = path.is_some();
        match (w.stage, path) {
            (Stage::Walking, Some(path)) => match nearest {
                None => {}
                Some((d, _)) if d > LOSE => keep = false,
                Some((d, spot)) => {
                    w.east = spot.0 >= w.x;
                    let target = path.nearest((spot.0 / TILE_UNITS, spot.1 / TILE_UNITS)).0;
                    if d <= REACH - 0.6 || (target - w.along).abs() <= step {
                        w.still += 1;
                        if w.still >= IDLE && self.wayfarers.talks.is_empty() {
                            w.stage = Stage::Leaving(tick);
                        }
                    } else {
                        w.still = 0;
                        let s = (target - w.along).signum();
                        w.dir = s as i8;
                        let to = w.along + s * step;
                        self.walk_on(&mut w, to, geo);
                    }
                }
            },
            (Stage::Leaving(since), Some(path)) => {
                let away = nearest.map_or(1.0, |(_, spot)| {
                    let d = |along: f32| {
                        let (px, py) = path.at(along);
                        (spot.0 / TILE_UNITS - px).hypot(spot.1 / TILE_UNITS - py)
                    };
                    if d(w.along + 3.0) >= d(w.along - 3.0) { 1.0 } else { -1.0 }
                });
                w.dir = away as i8;
                w.east = away > 0.0;
                let to = (w.along + away * step).clamp(0.0, path.len());
                self.walk_on(&mut w, to, geo);
                keep = tick.saturating_sub(since) < LEAVE;
            }
            (Stage::Following, _) => keep = self.squire_follows(&mut w, nearest, geo),
            (Stage::Fighting(_), _) => keep = self.gang_fights(&mut w),
            _ => {}
        }
        // It has done what it came to: it goes when the talk is closed.
        if keep && w.done && w.stage == Stage::Walking && self.wayfarers.talks.is_empty() {
            w.stage = Stage::Leaving(tick);
        }
        if keep {
            self.wayfarers.on = Some(w);
        } else {
            self.wayfarers.walked = 0;
            self.wayfarers.talks.clear();
        }
    }

    /// Step along the road to `along`, if the ground there is open.
    fn walk_on(&self, w: &mut Wayfarer, along: f32, geo: &map::Geography) {
        let Some(path) = paths().get(usize::from(w.road)) else {
            return;
        };
        let (px, py) = path.at(along);
        if geo.room.tile(px.floor() as i32, py.floor() as i32) != Tile::Floor {
            return;
        }
        w.along = along;
        (w.x, w.y) = (px * TILE_UNITS, py * TILE_UNITS);
        w.moved = self.tick;
    }

    /// The squire at the knights' heels, and home when she reaches the
    /// Village's door. False when she is home.
    fn squire_follows(&mut self, w: &mut Wayfarer, nearest: Option<(f32, (f32, f32))>, geo: &map::Geography) -> bool {
        let door = Self::village_stand(geo).map(|(x, y)| ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.7) * TILE_UNITS));
        if let Some((hx, hy)) = door
            && (w.x - hx).hypot(w.y - hy) < HOME_REACH * TILE_UNITS
        {
            self.squire_home(w);
            return false;
        }
        let Some((d, spot)) = nearest else {
            return true;
        };
        // Left behind (in a house, a coach ride): back at the knights' heels.
        if d > 20.0 {
            (w.x, w.y) = (spot.0 - 1.5 * TILE_UNITS, spot.1);
            return true;
        }
        if d > 2.6 {
            // A knight's walk, then a trot to catch up (tiles a second).
            let pace = if d > 8.0 { 13.0 } else if d > 4.0 { 6.5 } else { 3.5 } * TILE_UNITS * DT;
            let (ux, uy) = unit(spot.0 - w.x, spot.1 - w.y);
            let grid = Grid { room: self.room(), barred: false };
            let to = grid.slide((w.x, w.y), (ux * pace, uy * pace), HERO_RADIUS, Mover::Walker);
            if (to.0 - w.x).abs() + (to.1 - w.y).abs() > 1e-4 {
                w.moved = self.tick;
            }
            (w.x, w.y) = to;
            w.east = ux >= 0.0;
        }
        true
    }

    /// The squire is led to the Village: she takes a bed there.
    fn squire_home(&mut self, w: &Wayfarer) {
        *self.marks.entry(format!("{SQUIRE}{}", w.name)).or_default() += 1;
        if self.kingdom.moments.errand == Some(RoomKind::Village) {
            self.kingdom.moments.errand = None;
        }
        let bed = self.squire_bed().unwrap_or_else(|| "the Village".into());
        self.sounds.push("wish_grant");
        self.cues.push("wayfarer_home".into());
        self.found = Some((self.tick, 0, format!("{} TAKES A BED AT {}", w.name.to_uppercase(), bed.to_uppercase())));
    }

    /// The gang's fight: false when it is won (the purse is the realm's).
    fn gang_fights(&mut self, w: &mut Wayfarer) -> bool {
        if self.enemies.iter().any(|e| e.hp > 0 && w.gang.contains(&e.id)) {
            return true;
        }
        *self.marks.entry(OWED.to_string()).or_default() += PURSE;
        self.sounds.push("wheel_land");
        self.cues.push("wayfarer_won".into());
        self.found = Some((self.tick, 0, format!("THE TOLL GANG IS ROUTED: THEIR PURSE IS THE REALM'S, {PURSE} GOLD")));
        false
    }

    /// The gang draws steel: their bodies stand in the road round the
    /// reeve, at the health any such body has.
    fn draw_steel(&mut self, w: &mut Wayfarer) {
        let offsets = [(-3.2, 0.4), (3.2, 0.4), (0.0, -3.0)];
        for (kind, (dx, dy)) in GANG.into_iter().zip(offsets) {
            let grid = Grid { room: self.room(), barred: false };
            let (mut x, mut y) = (w.x + dx, w.y + dy);
            if !grid.clear(x, y, kind.radius(), Mover::Walker) {
                (x, y) = (w.x, w.y);
            }
            if let Some(id) = self.spawn_body(kind, x, y) {
                w.gang.push(id);
            }
        }
        w.stage = Stage::Fighting(self.tick);
        self.wayfarers.talks.clear();
        self.sounds.push("door_seal");
        self.cues.push("wayfarer_steel".into());
        self.found = Some((self.tick, 0, "THE TOLL GANG DRAWS STEEL".into()));
    }

    /// Beaten on the road: the knights are knocked down, not killed. The
    /// gang takes its toll anyway, as much as the treasury can spare, and
    /// goes. True if it was this (and not a wipe).
    pub(super) fn wayfarer_rout(&mut self) -> bool {
        let Some(w) = self.wayfarers.on.as_ref().filter(|w| matches!(w.stage, Stage::Fighting(_))) else {
            return false;
        };
        if self.room().kind != RoomKind::Kingdom
            || !(self.players.values().all(|h| h.hp == 0 || h.stone) && self.players.values().any(|h| h.hp == 0))
        {
            return false;
        }
        let gang = w.gang.clone();
        self.enemies.retain(|e| !gang.contains(&e.id));
        self.projectiles.retain(|p| !p.hostile);
        for hero in self.players.values_mut() {
            if hero.hp == 0 {
                hero.hp = (hero.max_hp / 4).max(1);
            }
        }
        let toll = TOLL.min(self.gold_spare());
        *self.marks.entry(PAID.to_string()).or_default() += toll;
        self.wayfarers.on = None;
        self.wayfarers.walked = 0;
        self.sounds.push("door_open");
        self.cues.push("wayfarer_lost".into());
        self.found = Some((self.tick, 0, format!("THE GANG TAKES ITS TOLL, {toll} GOLD, AND LEAVES YOU IN THE DUST")));
        true
    }

    /// E by knight `id` on the road: an open talk goes on, then away; a
    /// traveller in reach is spoken to. Returns whether the press was theirs.
    pub(super) fn use_wayfarer(&mut self, id: u32) -> bool {
        if let Some(talk) = self.wayfarers.talks.get(&id).cloned() {
            if talk.page == 0 {
                self.wayfarers.talks.insert(id, Talk { page: 1, ..talk });
            } else {
                self.wayfarers.talks.remove(&id);
            }
            return true;
        }
        let tick = self.tick;
        let Some(w) = self.wayfarer_in_reach(id).map(|w| (w.stage, w.met, w.kind, w.name.clone())) else {
            return false;
        };
        // The squire at the party's heels has said all she has.
        if w.0 == Stage::Following {
            self.found = Some((tick, id, "THE SQUIRE KEEPS CLOSE. LEAD HER TO THE VILLAGE".into()));
            return true;
        }
        if !w.1 {
            if let Some(on) = self.wayfarers.on.as_mut() {
                on.met = true;
            }
            *self.marks.entry(format!("{MET}{}:{}", w.2.word(), w.3)).or_default() += 1;
        }
        self.wayfarers.talks.insert(id, Talk { page: 0, opened: tick, said: String::new() });
        self.sounds.push("play_card");
        true
    }

    /// The choices on offer in the traveller's talk.
    pub fn wayfarer_choices(&self) -> Vec<(Choice, String)> {
        let Some(w) = self.wayfarers.on.as_ref() else {
            return Vec::new();
        };
        match w.kind {
            Kind::Pilgrim => vec![
                (Choice::Bless, format!("BLESS THE PARTY: +{BLESS_PERCENT}% DAMAGE AND PACE FOR {}S", BLESS_TICKS / HZ)),
                (Choice::Mend, format!("BLESS ME ALONE: FULL HEALTH, AND +{BLESS_PERCENT}% FOR {}S", BLESS_ALONE_TICKS / HZ)),
            ],
            Kind::Merchant => vec![
                (Choice::Potion, pay_label("A POTION", POTION)),
                (Choice::Bomb, pay_label("A BOMB", BOMB)),
                (Choice::Scroll, pay_label("A THUNDER SCROLL", SCROLL)),
            ],
            Kind::Troubadour => vec![
                (Choice::Sing, format!("SING OF US: MENDS EACH KNIGHT BY {SONG_MEND}%, +{SONG_RENOWN} RENOWN")),
                (Choice::Tip, format!("TIP HIM {TIP} GOLD: +{TIP_RENOWN} RENOWN")),
            ],
            Kind::Tollkeeper => vec![
                (Choice::Toll, format!("PAY THE TOLL: {TOLL} GOLD")),
                (Choice::Deeds, "NAME YOUR DEEDS".into()),
                (Choice::Steel, "DRAW STEEL".into()),
            ],
            Kind::Squire => vec![(Choice::Follow, "COME WITH US TO THE VILLAGE".into())],
            Kind::Gossip => vec![
                (Choice::Hear, "HEAR WHAT HE KNOWS: MARKS A PLACE".into()),
                (Choice::Better, format!("A BETTER TALE: {BETTER_TALE} GOLD, MARKS A PLACE WITH WORK")),
            ],
        }
    }

    /// What knight `id`'s talk shows now.
    pub fn wayfarer_speech(&self, id: u32) -> Option<Speech> {
        let w = self.wayfarers.on.as_ref()?;
        let talk = self.wayfarers.talks.get(&id)?;
        let (words, answers) = match talk.page {
            0 => {
                let mut words = w.kind.hello().to_string();
                // What the realm remembers of the road: the last who was met.
                if let Some(last) = self.home.wayfarers.lately.last().filter(|l| !l.eq_ignore_ascii_case(&format!("{} {}", w.kind.title(), w.name))) {
                    words = format!("{words} Word on the road is that you met {last}.");
                }
                (words, vec!["E: GO ON".to_string()])
            }
            1 => {
                let mut words = w.kind.ask().to_string();
                if w.kind == Kind::Merchant {
                    words = format!("{words} The treasury can spare {} gold.", self.gold_spare());
                }
                if !talk.said.is_empty() {
                    words = format!("{words} {}", talk.said);
                }
                let mut answers: Vec<String> = self
                    .wayfarer_choices()
                    .into_iter()
                    .enumerate()
                    .map(|(i, (_, label))| format!("{}: {label}", i + 1))
                    .collect();
                answers.push("E: WALK ON".into());
                (words, answers)
            }
            _ => (talk.said.clone(), vec!["E: FAREWELL".to_string()]),
        };
        Some(Speech { kind: w.kind, who: w.plate(), words, answers })
    }

    /// The keys a talk takes before the rest of the tick sees them: a knight
    /// in conversation answers with 1, 2 and 3 rather than casting, one
    /// answer to a press.
    pub(super) fn wayfarer_inputs(&mut self, inputs: &BTreeMap<u32, Input>) -> Option<BTreeMap<u32, Input>> {
        self.wayfarers.keys.retain(|id, _| inputs.contains_key(id));
        if self.wayfarers.talks.is_empty() && self.wayfarers.keys.is_empty() {
            return None;
        }
        let mut out = inputs.clone();
        for (&id, input) in out.iter_mut() {
            if !input.valid() {
                continue;
            }
            let talking = self.wayfarers.talks.contains_key(&id);
            let was = self.wayfarers.keys.get(&id).copied().unwrap_or(0);
            if !talking && was == 0 {
                continue;
            }
            if talking && input.cast != 0 && input.cast != was {
                self.answer_wayfarer(id, input.cast);
            }
            if input.cast != 0 {
                self.wayfarers.keys.insert(id, input.cast);
                input.cast = 0;
            } else {
                self.wayfarers.keys.remove(&id);
            }
        }
        Some(out)
    }

    /// Knight `id` answers with key `pick` (1 to 3).
    pub(super) fn answer_wayfarer(&mut self, id: u32, pick: u8) {
        if self.wayfarers.talks.get(&id).is_none_or(|t| t.page != 1) {
            return;
        }
        let Some((choice, _)) = self.wayfarer_choices().get(usize::from(pick).wrapping_sub(1)).cloned() else {
            return;
        };
        let Some(mut w) = self.wayfarers.on.take() else {
            return;
        };
        let said = self.apply_choice(id, choice, &mut w);
        self.wayfarers.on = Some(w);
        let Some((words, done)) = said else {
            return;
        };
        self.found = Some((self.tick, id, words.to_uppercase()));
        if let Some(talk) = self.wayfarers.talks.get_mut(&id) {
            talk.said = words;
            if done {
                talk.page = 2;
            }
        }
        if done && let Some(w) = self.wayfarers.on.as_mut() {
            w.done = true;
        }
    }

    /// What the choice does. Its words, and whether it settles the meeting;
    /// none when the gang took up steel instead.
    fn apply_choice(&mut self, id: u32, choice: Choice, w: &mut Wayfarer) -> Option<(String, bool)> {
        match choice {
            Choice::Bless => {
                let mut n = 0;
                for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone && !h.absent) {
                    hero.blessed = BLESS_TICKS;
                    n += 1;
                }
                self.sounds.push("wish_grant");
                self.cues.push("wayfarer_blessing".into());
                let who = if n == 1 { "knight" } else { "knights" };
                Some((format!("The blessing falls on {n} {who}: {BLESS_PERCENT}% more damage and pace for {} seconds.", BLESS_TICKS / HZ), true))
            }
            Choice::Mend => {
                let hero = self.players.get_mut(&id)?;
                hero.hp = hero.max_hp;
                hero.blessed = BLESS_ALONE_TICKS;
                self.sounds.push("wish_grant");
                self.cues.push("wayfarer_blessing".into());
                Some((format!("A pilgrim's hand on your brow: you are whole, and {BLESS_PERCENT}% quicker and harder for {} seconds.", BLESS_ALONE_TICKS / HZ), true))
            }
            Choice::Potion | Choice::Bomb | Choice::Scroll => {
                let (price, what) = match choice {
                    Choice::Potion => (POTION, "a potion"),
                    Choice::Bomb => (BOMB, "a bomb"),
                    _ => (SCROLL, "a thunder scroll"),
                };
                let hero = self.players.get_mut(&id)?;
                let room = if choice == Choice::Bomb { hero.bombs < hero.bomb_limit() } else { hero.hand.len() < cards::HAND };
                if !room {
                    return Some((if choice == Choice::Bomb { "Your belt is full." } else { "Your hand is full." }.into(), false));
                }
                if !self.pay_road(price) {
                    return Some((format!("The treasury cannot spare {price} gold."), false));
                }
                let hero = self.players.get_mut(&id)?;
                match choice {
                    Choice::Bomb => hero.give_bombs(1),
                    Choice::Potion => hero.hand.push("potion".into()),
                    _ => hero.hand.push("thunder-scroll".into()),
                }
                w.sold += 1;
                self.sounds.push("gold");
                Some((format!("Sold: {what}, {price} gold from the treasury."), w.sold >= STOCK))
            }
            Choice::Sing => {
                let verses = self.verses();
                let mut mended = 0;
                for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone && !h.absent) {
                    let gain = (hero.max_hp * SONG_MEND / 100).min(hero.max_hp.saturating_sub(hero.hp));
                    hero.hp += gain;
                    mended += gain;
                }
                self.score = self.score.saturating_add(SONG_RENOWN);
                self.sounds.push("wish_grant");
                self.cues.push("wayfarer_song".into());
                let after = if mended > 0 { "The song mends the party" } else { "Nobody is hurt, so it mends no one" };
                Some((format!("He sings of {}. {after}, and carries your name: +{SONG_RENOWN} renown.", verses.join("; and of ")), true))
            }
            Choice::Tip => {
                if !self.pay_road(TIP) {
                    return Some((format!("The treasury cannot spare {TIP} gold."), false));
                }
                self.score = self.score.saturating_add(TIP_RENOWN);
                self.sounds.push("gold");
                Some((format!("He bows. Your name goes into his songs: +{TIP_RENOWN} renown."), true))
            }
            Choice::Toll => {
                if !self.pay_road(TOLL) {
                    return Some((format!("The treasury cannot spare {TOLL} gold."), false));
                }
                self.sounds.push("gold");
                Some((format!("{TOLL} gold from the treasury. The rope drops; they step aside."), true))
            }
            Choice::Deeds => {
                if let Some((name, _)) = self.top_guardian() {
                    self.sounds.push("wheel_land");
                    Some((format!("They have heard of {name}. The rope drops; they step aside."), true))
                } else {
                    self.draw_steel(w);
                    None
                }
            }
            Choice::Steel => {
                self.draw_steel(w);
                None
            }
            Choice::Follow => {
                w.stage = Stage::Following;
                self.kingdom.moments.errand = Some(RoomKind::Village);
                self.sounds.push("play_card");
                Some(("Lead on. I will keep close. The arrow points to the Village.".into(), true))
            }
            Choice::Hear | Choice::Better => {
                let geo = map::geography(&self.kingdom.wishes);
                let (room, line) = if choice == Choice::Hear {
                    let Some(site) = self.unseen_places(&geo, (w.x, w.y)).first().copied() else {
                        return Some(("He knows of nothing you have not seen.".into(), false));
                    };
                    (site.room, format!("They say something stirs at {}.", site.name))
                } else {
                    let Some(&(room, tale)) = self.open_errands().first() else {
                        return Some(("He has nothing better than the first tale.".into(), false));
                    };
                    if !self.pay_road(BETTER_TALE) {
                        return Some((format!("The treasury cannot spare {BETTER_TALE} gold."), false));
                    }
                    let name = geo.sites.iter().find(|s| s.room == room).map_or("a place nearby", |s| s.name.as_str());
                    (room, format!("{tale}, at {name}."))
                };
                self.kingdom.moments.errand = Some(room);
                self.wayfarers.rumoured = Some(room);
                self.sounds.push("play_card");
                Some((format!("{line} The arrow on your road points there."), true))
            }
        }
    }

    /// A knight steps into the place the goal arrow points at: a rumour that
    /// sent them there was true.
    pub(super) fn rumour_comes_true(&mut self, geo: &map::Geography, site: &map::Site) {
        let Some(room) = self.wayfarers.rumoured else {
            return;
        };
        let told = geo.sites.iter().find(|s| s.room == room).or_else(|| self.gateway(room, geo));
        if !told.is_some_and(|s| s.stand == site.stand) {
            return;
        }
        self.wayfarers.rumoured = None;
        self.score = self.score.saturating_add(RUMOUR_RENOWN);
        self.sounds.push("gold");
        self.found = Some((self.tick, 0, format!("THE DROVER WAS RIGHT: +{RUMOUR_RENOWN} RENOWN")));
    }
}

/// The save: what the road owes the treasury and the treasury the road,
/// who was met, and a squire home to a bed in a holding that has one.
pub fn settle(realm: &mut Realm<'_>, marks: &BTreeMap<String, u32>) -> Settled {
    let mut out = Settled::default();
    if !marks.keys().any(|m| m.starts_with("wayfarer:")) {
        return out;
    }
    let owed = marks.get(PAID).copied().unwrap_or(0).min(realm.treasury.get(Spoil::Gold));
    if owed > 0 {
        let mut price = Spoils::default();
        price.add(Spoil::Gold, owed);
        realm.treasury.take(&price);
        out.changed = true;
    }
    if let Some(&purse) = marks.get(OWED) {
        realm.treasury.add(Spoil::Gold, purse);
        out.changed = true;
    }
    let before = realm.home.wayfarers.clone();
    for (mark, &n) in marks {
        if let Some((kind, name)) = mark
            .strip_prefix(MET)
            .and_then(|rest| rest.split_once(':'))
            .and_then(|(kind, name)| Kind::from_word(kind).map(|k| (k, name)))
            .filter(|(_, name)| !name.is_empty() && name.len() <= 24)
        {
            realm.home.wayfarers.record(kind, name, n);
        }
        if let Some(name) = mark.strip_prefix(SQUIRE).filter(|n| !n.is_empty() && n.len() <= 24) {
            let spot = realm
                .home
                .holdings
                .owned
                .iter_mut()
                .find(|w| w.steward.is_some() && w.raid == 0 && w.folk.len() < w.standing().min(MAX_VILLAGERS));
            if let Some(held) = spot {
                let place = held.deed().map_or_else(|| held.key.clone(), super::holdings::Deed::name);
                held.folk.push(Villager {
                    name: name.to_string(),
                    trade: Trade::Labourer,
                    mood: 2,
                    hunger: 0,
                    skill: 0,
                    practiced: 0,
                    age: 17,
                    seasons: 0,
                });
                let line = format!("{name} the squire comes to {place}, led there from the road");
                realm.home.holdings.say(line.clone());
                out.said.push(("wayfarers".into(), line));
                out.changed = true;
            }
        }
    }
    out.changed |= realm.home.wayfarers != before;
    out
}

#[cfg(test)]
#[path = "../../tests/rules/wayfarers__tests.rs"]
mod tests;
