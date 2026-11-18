//! Hands: the Keep's wall built by hand (the Works, `world::works`).
//!
//! A knight who has taken up a trade (`trades`) does that trade's work at the
//! site with their own hands, with weight and rhythm, and the stone chain it
//! feeds is the wall's:
//!
//!   rough block -> banker (cut) -> yard -> hod or crane -> the course -> setter
//!
//! The *quarryman* swings a pick at the outcrop and blocks come off. The
//! *banker mason* cuts a rough block to ashlar, a quoin or a voussoir at a
//! bench, four chisel strokes on the beat. The *labourer* carries stone and
//! mortar to the wall, slower for the weight, and climbs the scaffold to set
//! the load down; or walks the treadwheel so the crane lifts a stone. The
//! *setter* sets the next stone in lay order at the pegged course with a
//! plumb-and-level timing game: a true set is a stronger stone (stored), and
//! every set writes the knight's mason's mark to the site's tally. The
//! *carpenter* raises scaffold bays and arch centring ahead of the masons,
//! who may not lay past an unraised bay. The *Master Mason* draws the
//! templates arch stones are cut by and pegs out new castles; the *Clerk of
//! Works* reads the whole line and says what is holding it up.
//!
//! The hired hands of the site do the same jobs at a slower, steady rate, and
//! a knight beside them speeds the gang. Until the production sim and the
//! Delve's stockyard are integrated three seams stand in for them, each one
//! function: [`hired`] (the sim's workers by trade), [`MILL`] (the mortar
//! mill's stub) and the rough stone's one store, the Delve gate's stockyard
//! (`stockyard`): the benches cut from what stands there, the outcrop adds
//! rubble to it ([`Run::stock_rough`]) and the Delve's carry sets quarry
//! blocks down in it. Nothing here needs the sim to work.
//!
//! Everything is keyed on the tick and on the keys held, with no randomness:
//! the host and a watching client step it the same.

use super::site::Site;
use super::trades::{LEARN_HOLD, OFFER_TICKS, Rank, SCHOOLS, Trade};
use super::*;
use crate::world::works::{self, Castle, PX_PER_M};
use crate::world::works_life as life;

pub const QUARRY: [f32; 2] = [-12.8, -6.4];
pub const TUB: [f32; 2] = [-9.6, -8.0];
pub const FLOOR: [f32; 2] = [-12.6, -12.8];
pub const BOARD: [f32; 2] = [-12.0, -8.2];

pub const REACH_QUARRY: f32 = 2.2;
pub const REACH_BENCH: f32 = 1.7;
pub const REACH_YARD: f32 = 2.4;
pub const REACH_TUB: f32 = 2.0;
pub const REACH_WHEEL: f32 = 1.7;
pub const REACH_FLOOR: f32 = 2.2;
pub const REACH_BOARD: f32 = 2.2;
pub const REACH_SCHOOL: f32 = 1.9;
pub const REACH_WALL: f32 = 4.6;

const BENCH_STAND: f32 = 1.15;

pub const CLAIMS: u32 = 3;

pub const LOOKAHEAD: u32 = 320;

pub const CUT_MAX: u32 = 40;
pub const COURSE_MAX: u32 = 12;
pub const MORTAR_MAX: u32 = 30;
pub const TUB_MAX: u32 = 18;

pub const LOAD_DABS: u32 = 6;

pub const TEMPLATE_USES: u32 = 12;

const TEMPLATED: [&str; 2] = ["voussoir", "chevron"];

pub const MILL: u32 = 90;

pub const fn beat(job: Job) -> (u32, u8) {
    match job {
        Job::Quarry => (24, 5),
        Job::Bench => (20, 4),
        Job::Bay => (22, 6),
        Job::Floor => (26, 3),
        _ => (24, 1),
    }
}

pub const PERFECT: u32 = 1;

pub const PLUMB_SWING: u32 = 36;
pub const LEVEL_DRIFT: u32 = 45;
pub const FLINCH: u64 = 6;
pub const LETGO: u64 = 4 * HZ as u64;
pub const SETTLE: u64 = 9;
const LOSE_BEATS: u32 = 3;

pub const CLIMB: u32 = 36;

const STROKE: u32 = 6;

pub const PACE_STONE: f32 = 0.62;
pub const PACE_MORTAR: f32 = 0.8;

pub const CRANE_CYCLE: f32 = 9.0;
const CRANE_DOWN: f32 = 0.78;

pub const CRANE_HIRED: f32 = 0.35;

pub const BESIDE: f32 = 9.0;
pub const BESIDE_MAX: usize = 3;

const CLERKING: u64 = 20 * HZ as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Job {
    Quarry,
    Bench,
    Yard,
    Tub,
    Ladder,
    Wheel,
    Course,
    Bay,
    Floor,
    Board,
}

impl Job {
    pub const fn trade(self) -> Trade {
        match self {
            Job::Quarry => Trade::Quarryman,
            Job::Bench => Trade::BankerMason,
            Job::Yard | Job::Tub | Job::Ladder | Job::Wheel => Trade::Labourer,
            Job::Course => Trade::Setter,
            Job::Bay => Trade::Carpenter,
            Job::Floor => Trade::MasterMason,
            Job::Board => Trade::ClerkOfWorks,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Job::Quarry => "THE OUTCROP",
            Job::Bench => "THE BANKER",
            Job::Yard => "THE YARD",
            Job::Tub => "THE MORTAR TUB",
            Job::Ladder => "THE SCAFFOLD",
            Job::Wheel => "THE TREADWHEEL",
            Job::Course => "THE COURSE",
            Job::Bay => "THE NEXT BAY",
            Job::Floor => "THE TRACING FLOOR",
            Job::Board => "THE TALLY",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Perfect,
    Good,
    Off,
}

impl Grade {
    pub fn of(dev: u32, rank: Rank) -> Grade {
        if dev <= PERFECT {
            Grade::Perfect
        } else if dev <= rank.grace() {
            Grade::Good
        } else {
            Grade::Off
        }
    }

    pub fn lands(self) -> bool {
        self != Grade::Off
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Work {
    Strike { job: Job, began: u64, hits: u8, flaws: u8, last: u64 },
    Set { index: u32, began: u64, plumb: Option<Grade>, tapped: u64, level: Option<Grade>, lands: u64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Carry {
    #[default]
    Nothing,
    Stone,
    Mortar,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hand {
    pub trade: Option<Trade>,
    pub carry: Carry,
    pub work: Option<Work>,
    pub hold: u32,
    pub holding: Option<Job>,
    pub asking: Option<Trade>,
    pub struck: u64,
    pub swing_was: bool,
    pub use_was: bool,
    pub clerked: u64,
    pub said: Option<(u64, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Yard {
    pub cut: u32,
    pub course: u32,
    pub mortar: u32,
    pub tub: u32,
    pub timber: u32,
    pub templates: u32,
    pub crane_extra: f32,
    #[serde(skip)]
    pub acc: [u32; 8],
    pub done: BTreeMap<u32, (u8, bool)>,
    pub settled: Option<(u32, u64)>,
}

impl Default for Yard {
    fn default() -> Self {
        Yard {
            cut: 0,
            course: 0,
            mortar: 0,
            tub: TUB_MAX / 2,
            timber: super::site::START,
            templates: 0,
            crane_extra: 0.0,
            acc: [0; 8],
            done: BTreeMap::new(),
            settled: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hands {
    pub seats: BTreeMap<u32, Hand>,
    pub yard: Yard,
    #[serde(skip)]
    pub busy: std::collections::BTreeSet<u32>,
    #[serde(skip)]
    pub striking: std::collections::BTreeSet<u32>,
    #[serde(skip)]
    pub seen: super::works_seam::Seen,
}

impl Hands {
    pub fn is_default(&self) -> bool {
        self == &Hands::default()
    }

    pub fn valid(&self) -> bool {
        let y = &self.yard;
        self.seats.len() <= MAX_PLAYERS as usize
            && self.seats.keys().all(|&id| (1..=MAX_PLAYERS).contains(&id))
            && self.seats.values().all(|h| h.said.as_ref().is_none_or(|(_, t)| t.len() <= 160))
            && y.cut <= CUT_MAX
            && y.course <= COURSE_MAX
            && y.mortar <= MORTAR_MAX
            && y.tub <= TUB_MAX
            && y.templates <= 100 * TEMPLATE_USES
            && y.crane_extra.is_finite()
            && y.crane_extra >= 0.0
            && y.done.len() <= 16
    }

    pub fn pace(&self, id: u32) -> f32 {
        match self.seats.get(&id).map(|h| h.carry) {
            Some(Carry::Stone) => PACE_STONE,
            Some(Carry::Mortar) => PACE_MORTAR,
            _ => 1.0,
        }
    }

    pub fn trade_of(&self, id: u32) -> Option<Trade> {
        self.seats.get(&id).and_then(|h| h.trade)
    }

    fn fits(job: Job, carry: Carry) -> bool {
        match job {
            Job::Yard | Job::Tub | Job::Wheel => carry == Carry::Nothing,
            Job::Ladder => carry != Carry::Nothing,
            Job::Course | Job::Quarry | Job::Bench | Job::Bay | Job::Floor | Job::Board => carry == Carry::Nothing,
        }
    }
}

pub fn hired(trade: Trade) -> u32 {
    match trade {
        Trade::Quarryman => 2,
        Trade::BankerMason => 3,
        Trade::Labourer => 3,
        Trade::Setter => 2,
        Trade::Carpenter => 1,
        Trade::MasterMason => 1,
        _ => 0,
    }
}

const fn cost(slot: usize) -> u32 {
    match slot {
        SLOT_QUARRY => 20 * HZ,
        SLOT_BENCH => 15 * HZ,
        SLOT_HAUL => 20 * HZ,
        SLOT_MORTAR => 6 * HZ,
        SLOT_SET => 12 * HZ,
        SLOT_BAY => 30 * HZ,
        SLOT_TEMPLATE => 300 * HZ,
        _ => u32::MAX,
    }
}

const SLOT_QUARRY: usize = 0;
const SLOT_BENCH: usize = 1;
const SLOT_HAUL: usize = 2;
const SLOT_MORTAR: usize = 3;
const SLOT_SET: usize = 4;
const SLOT_BAY: usize = 5;
const SLOT_TEMPLATE: usize = 6;

pub fn units(p: [f32; 2]) -> (f32, f32) {
    let (ox, oz) = works::origin_px();
    let k = 16.0 / TILE_UNITS;
    ((ox + p[0] * PX_PER_M) / k, (oz - p[1] * PX_PER_M) / k)
}

pub fn metres((x, y): (f32, f32)) -> [f32; 2] {
    let (ox, oz) = works::origin_px();
    let k = 16.0 / TILE_UNITS;
    [(x * k - ox) / PX_PER_M, (oz - y * k) / PX_PER_M]
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

pub fn stone_at(castle: &Castle, index: u32) -> [f32; 2] {
    let b = castle.stones[(index as usize).min(castle.stones.len().saturating_sub(1))].aabb();
    [f32::from(b[0] + b[3]) / 200.0, f32::from(b[1] + b[4]) / 200.0]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bay {
    pub appears: u32,
    pub at: [f32; 2],
}

pub fn bays(castle: &Castle) -> Vec<Bay> {
    static KEEP: std::sync::OnceLock<Vec<Bay>> = std::sync::OnceLock::new();
    if castle.id == works::KEEP {
        return KEEP.get_or_init(|| bays_of(castle)).clone();
    }
    bays_of(castle)
}

fn bays_of(castle: &Castle) -> Vec<Bay> {
    use crate::world::works::Work as Frame;
    let mut sums: BTreeMap<u32, ([f32; 2], u32)> = BTreeMap::new();
    for f in castle.falsework.iter().filter(|f| matches!(f.kind, Frame::Scaffold | Frame::Centring)) {
        let [x0, y0, _, x1, y1, _] = f.bx.map(|v| f32::from(v) / 100.0);
        let e = sums.entry(f.appears).or_insert(([0.0; 2], 0));
        e.0[0] += (x0 + x1) / 2.0;
        e.0[1] += (y0 + y1) / 2.0;
        e.1 += 1;
    }
    sums.into_iter().map(|(appears, (s, n))| Bay { appears, at: [s[0] / n as f32, s[1] / n as f32] }).collect()
}

pub fn limit(castle: &Castle, yard: &Yard, laid: u32) -> u32 {
    let raised = bays(castle).iter().filter(|b| b.appears < laid).map(|b| b.appears).max().unwrap_or(0).max(yard.timber);
    bays(castle).iter().map(|b| b.appears).find(|&a| a > raised).unwrap_or(castle.len())
}

pub fn next_bay(castle: &Castle, yard: &Yard, laid: u32) -> Option<Bay> {
    let lim = limit(castle, yard, laid);
    if lim >= castle.len() || lim.saturating_sub(laid) > LOOKAHEAD {
        return None;
    }
    bays(castle).into_iter().find(|b| b.appears == lim)
}

pub fn crane_clock(tick: u64, extra: f32) -> f32 {
    tick as f32 / HZ as f32 * CRANE_HIRED + extra
}

pub fn wheel_stand(castle: &Castle, laid: u32) -> Option<[f32; 2]> {
    life::crane(castle, laid, 0.0).map(|c| [c.at[0] - 1.4, c.at[1]])
}

pub fn jobs_near(castle: &Castle, yard: &Yard, laid: u32, at: [f32; 2]) -> Vec<Job> {
    let mut out = Vec::new();
    if dist(at, BOARD) <= REACH_BOARD {
        out.push(Job::Board);
    }
    if dist(at, FLOOR) <= REACH_FLOOR {
        out.push(Job::Floor);
    }
    if dist(at, TUB) <= REACH_TUB {
        out.push(Job::Tub);
    }
    if dist(at, life::YARD) <= REACH_YARD {
        out.push(Job::Yard);
    }
    if dist(at, QUARRY) <= REACH_QUARRY {
        out.push(Job::Quarry);
    }
    if bench_near(at).is_some() {
        out.push(Job::Bench);
    }
    if wheel_stand(castle, laid).is_some_and(|w| dist(at, w) <= REACH_WHEEL) {
        out.push(Job::Wheel);
    }
    if laid < castle.len() {
        if (laid..laid.saturating_add(CLAIMS).min(castle.len())).any(|i| dist(at, stone_at(castle, i)) <= REACH_WALL) {
            out.push(Job::Course);
            out.push(Job::Ladder);
        }
        if next_bay(castle, yard, laid).is_some_and(|b| dist(at, b.at) <= REACH_WALL) {
            out.push(Job::Bay);
        }
    }
    out
}

pub fn job_point(castle: &Castle, yard: &Yard, laid: u32, job: Job, at: [f32; 2]) -> [f32; 2] {
    match job {
        Job::Quarry => QUARRY,
        Job::Bench => bench_near(at).map_or(at, |k| life::BANKERS[k]),
        Job::Yard => life::YARD,
        Job::Tub => TUB,
        Job::Wheel => wheel_stand(castle, laid).unwrap_or(at),
        Job::Course | Job::Ladder => stone_at(castle, laid),
        Job::Bay => next_bay(castle, yard, laid).map_or(at, |b| b.at),
        Job::Floor => FLOOR,
        Job::Board => BOARD,
    }
}

pub fn bench_near(at: [f32; 2]) -> Option<usize> {
    life::BANKERS
        .iter()
        .enumerate()
        .map(|(k, b)| (k, dist(at, [b[0], b[1] + BENCH_STAND])))
        .filter(|&(_, d)| d <= REACH_BENCH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
}

pub fn school_near(at: [f32; 2]) -> Option<usize> {
    SCHOOLS
        .iter()
        .enumerate()
        .map(|(k, s)| (k, dist(at, s.at)))
        .filter(|&(_, d)| d <= REACH_SCHOOL)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
}

pub fn offer(school: usize, tick: u64, holds: impl Fn(Trade) -> bool) -> Option<Trade> {
    let open: Vec<Trade> = SCHOOLS[school].teaches.iter().copied().filter(|&t| !holds(t)).collect();
    (!open.is_empty()).then(|| open[(tick / OFFER_TICKS) as usize % open.len()])
}

pub fn bob(since: u64) -> f32 {
    (std::f32::consts::TAU * since as f32 / PLUMB_SWING as f32).sin()
}

pub fn bubble(since: u64) -> f32 {
    let at = since % u64::from(2 * LEVEL_DRIFT);
    let half = u64::from(LEVEL_DRIFT);
    let t = if at < half { at as f32 / half as f32 } else { 2.0 - at as f32 / half as f32 };
    t * 2.0 - 1.0
}

pub fn plumb_dev(since: u64) -> u32 {
    let half = u64::from(PLUMB_SWING / 2);
    let r = since % half;
    r.min(half - r) as u32
}

pub fn level_dev(since: u64) -> u32 {
    let drift = LEVEL_DRIFT as f32;
    let x = (since as f32 + drift / 2.0) % drift;
    x.min(drift - x).round() as u32
}

pub fn beat_dev(job: Job, since: u64) -> u32 {
    let p = u64::from(beat(job).0);
    let r = since % p;
    r.min(p - r) as u32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Meter {
    Beat { job: Job, off: f32, hits: u8, need: u8 },
    Hold { frac: f32 },
    Set { bob: f32, bubble: Option<f32>, plumb: Option<Grade>, level: Option<Grade>, landed: bool },
}

impl Hand {
    pub fn meter(&self, now: u64) -> Option<Meter> {
        let Some(work) = self.work else {
            let target = if self.asking.is_some() {
                LEARN_HOLD
            } else if self.holding == Some(Job::Ladder) {
                CLIMB
            } else {
                return None;
            };
            return (self.hold > 0).then(|| Meter::Hold { frac: (self.hold as f32 / target as f32).min(1.0) });
        };
        match work {
            Work::Strike { job, began, hits, .. } => {
                let (p, need) = beat(job);
                let r = now.saturating_sub(began) % u64::from(p);
                let to_next = (u64::from(p) - r) % u64::from(p);
                Some(Meter::Beat { job, off: to_next as f32 / p as f32, hits, need })
            }
            Work::Set { began, plumb, tapped, level, lands, .. } => {
                let bubble = plumb.map(|_| bubble(now.saturating_sub(tapped)));
                Some(Meter::Set {
                    bob: if plumb.is_some() { 0.0 } else { bob(now.saturating_sub(began)) },
                    bubble,
                    plumb,
                    level,
                    landed: lands != 0 && now >= lands,
                })
            }
        }
    }

    pub fn climb(&self) -> f32 {
        if self.holding == Some(Job::Ladder) { (self.hold as f32 / CLIMB as f32).min(1.0) } else { 0.0 }
    }

    pub fn striking(&self, now: u64) -> bool {
        now.saturating_sub(self.struck) < u64::from(STROKE) && self.struck != 0
    }
}

struct Who {
    id: u32,
    key: String,
    at: [f32; 2],
    swing: bool,
    press: bool,
    held: bool,
}

impl Run {
    fn works_here(h: &Hero) -> bool {
        h.hp > 0 && !h.away && !h.stone && !h.idle && !h.absent
    }

    pub(super) fn tick_hands(&mut self, inputs: &BTreeMap<u32, Input>, castle: &'static Castle) {
        if self.site_replay.is_some() {
            self.hands.busy.clear();
            return;
        }
        let tick = self.tick;
        let laid = self.sites.keep().map_or(0, |s| s.laid);
        {
            let passed = bays(castle).iter().filter(|b| b.appears < laid).map(|b| b.appears).max().unwrap_or(0);
            self.hands.yard.timber = self.hands.yard.timber.max(passed);
        }
        let live: Vec<u32> = self.players.iter().filter(|(_, h)| Run::works_here(h)).map(|(&id, _)| id).collect();
        let gone: Vec<u32> = self.hands.seats.keys().copied().filter(|id| !live.contains(id)).collect();
        for id in gone {
            self.release_hand(id);
            if !self.players.contains_key(&id) {
                self.hands.seats.remove(&id);
            }
        }
        let mut whos = Vec::new();
        for &id in &live {
            let hero = &self.players[&id];
            let input = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
            let hand = self.hands.seats.entry(id).or_default();
            let (swing_was, use_was) = (hand.swing_was, hand.use_was);
            hand.swing_was = input.swing;
            hand.use_was = input.interact;
            whos.push(Who {
                id,
                key: super::home::Home::profile_key(hero),
                at: metres((hero.x, hero.y)),
                swing: input.swing && !swing_was,
                press: input.interact && !use_was,
                held: input.interact,
            });
            let key = super::home::Home::profile_key(hero);
            if self.hands.seats[&id].trade.is_none() {
                let best = self.trades().held(&key).first().map(|&(t, _)| t);
                if let Some(t) = best {
                    self.hands.seats.entry(id).or_default().trade = Some(t);
                }
            }
        }
        let mut busy = std::collections::BTreeSet::new();
        self.hands.striking.clear();
        for who in &whos {
            if self.hand_acts(who, castle) {
                busy.insert(who.id);
            }
        }
        self.hands.busy = busy;
        self.tick_gang(&whos, castle);
        self.settle_done(castle);
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let (trade, struck, load, on_wheel) = self.hands.seats.get(&id).map_or((None, 0, 0, false), |h| {
                (h.trade, h.struck, match h.carry { Carry::Nothing => 0, Carry::Stone => 1, Carry::Mortar => 2 }, h.holding == Some(Job::Wheel))
            });
            let Some(hero) = self.players.get_mut(&id) else { continue };
            hero.tool = match trade {
                Some(t) if hero.hp > 0 && !on_wheel => t.tool().pack(load),
                _ => 0,
            };
            if struck == tick && struck != 0 {
                hero.swing = hero.swing.max(STROKE);
            }
        }
    }

    pub(super) fn release_hand(&mut self, id: u32) {
        let Some(hand) = self.hands.seats.get_mut(&id) else { return };
        match std::mem::take(&mut hand.carry) {
            Carry::Stone => self.hands.yard.cut = (self.hands.yard.cut + 1).min(CUT_MAX),
            Carry::Mortar => self.hands.yard.tub = (self.hands.yard.tub + LOAD_DABS).min(TUB_MAX),
            Carry::Nothing => {}
        }
        hand.work = None;
        hand.hold = 0;
        hand.holding = None;
        hand.asking = None;
        self.hands.busy.remove(&id);
        if let Some(hero) = self.players.get_mut(&id) {
            hero.tool = 0;
        }
    }

    pub(super) fn release_all_hands(&mut self) {
        self.hands.busy.clear();
        self.hands.striking.clear();
        if self.hands.seats.values().all(|h| h.carry == Carry::Nothing && h.work.is_none()) && self.players.values().all(|h| h.tool == 0) {
            return;
        }
        let ids: Vec<u32> = self.hands.seats.keys().copied().collect();
        for id in ids {
            self.release_hand(id);
        }
        for h in self.players.values_mut() {
            h.tool = 0;
        }
    }

    fn face(&mut self, id: u32, toward: [f32; 2]) {
        let (tx, ty) = units(toward);
        if let Some(h) = self.players.get_mut(&id) {
            let (dx, dy) = (tx - h.x, ty - h.y);
            let n = dx.hypot(dy);
            if n > 0.05 {
                (h.aim_x, h.aim_y) = (dx / n, dy / n);
            }
        }
    }

    fn hand_says(&mut self, id: u32, text: impl Into<String>) {
        let tick = self.tick;
        if let Some(h) = self.hands.seats.get_mut(&id) {
            h.said = Some((tick, text.into()));
        }
    }

    fn hand_acts(&mut self, who: &Who, castle: &'static Castle) -> bool {
        let (id, tick) = (who.id, self.tick);
        let laid = self.sites.keep().map_or(0, |s| s.next());
        if let Some(s) = school_near(who.at) {
            return self.lesson(who, s);
        }
        {
            let hand = self.hands.seats.entry(id).or_default();
            if hand.asking.take().is_some() {
                hand.hold = 0;
                hand.holding = None;
            }
        }
        let near = jobs_near(castle, &self.hands.yard, laid, who.at);
        let hand = self.hands.seats.entry(id).or_default().clone();
        let active = hand.trade;
        let job = near.iter().copied().find(|&j| Some(j.trade()) == active && Hands::fits(j, hand.carry));
        let Some(job) = job else {
            if (who.swing || who.press)
                && let Some(other) = near.iter().copied().find(|j| self.trades().holds(&who.key, j.trade()))
            {
                self.take_up_tool(id, other.trade());
            }
            let h = self.hands.seats.entry(id).or_default();
            if h.work.take().is_some() || h.hold != 0 {
                h.hold = 0;
                h.holding = None;
            }
            if who.press && near.is_empty() && h.carry != Carry::Nothing {
                self.release_hand(id);
                self.sounds.push("step_wood");
            }
            return false;
        };
        if matches!(job, Job::Quarry | Job::Bench | Job::Bay | Job::Floor | Job::Course) {
            self.hands.striking.insert(id);
        }
        match job {
            Job::Quarry | Job::Bench | Job::Bay | Job::Floor => self.strike_job(who, job, castle),
            Job::Course => self.course_job(who, castle),
            Job::Yard => {
                if who.press {
                    if self.hands.yard.cut > 0 {
                        self.hands.yard.cut -= 1;
                        self.hands.seats.entry(id).or_default().carry = Carry::Stone;
                        self.sounds.push("spoil_pickup");
                        self.hand_says(id, "A STONE ON THE HOD");
                    } else {
                        self.hand_says(id, "NO CUT STONE IN THE YARD");
                    }
                }
            }
            Job::Tub => {
                if who.press {
                    let tub = self.tub_stock();
                    if tub >= LOAD_DABS {
                        if self.keep_live().is_none() {
                            self.hands.yard.tub -= LOAD_DABS;
                        }
                        self.hands.seats.entry(id).or_default().carry = Carry::Mortar;
                        self.sounds.push("spoil_pickup");
                        self.hand_says(id, "A LOAD OF MORTAR");
                    } else {
                        self.hand_says(id, "THE TUB IS EMPTY: THE MILL IS SLOW");
                    }
                }
            }
            Job::Ladder => self.climb(who),
            Job::Wheel => self.wheel(who),
            Job::Board => {
                if who.press {
                    let said = self.clerk_line(castle);
                    self.found = Some((tick, id, said));
                    let ready = self.hands.seats.get(&id).is_some_and(|h| tick >= h.clerked + CLERKING || h.clerked == 0);
                    if ready {
                        self.hands.seats.entry(id).or_default().clerked = tick.max(1);
                        self.earn_deeds(id, Trade::ClerkOfWorks, 1);
                    }
                }
            }
        }
        matches!(job, Job::Ladder | Job::Wheel) || self.hands.seats.get(&id).is_some_and(|h| h.work.is_some())
    }

    fn take_up_tool(&mut self, id: u32, trade: Trade) {
        let hand = self.hands.seats.entry(id).or_default();
        if hand.trade == Some(trade) {
            return;
        }
        let had = hand.carry;
        hand.work = None;
        if had != Carry::Nothing {
            self.release_hand(id);
        }
        self.hands.seats.entry(id).or_default().trade = Some(trade);
        self.sounds.push("spoil_pickup");
        self.hand_says(id, format!("THE {} IN HAND", trade.name()));
    }

    fn lesson(&mut self, who: &Who, school: usize) -> bool {
        let (id, tick) = (who.id, self.tick);
        let key = who.key.clone();
        let reading = self.trades();
        let offered = offer(school, tick, |t| reading.holds(&key, t));
        let Some(trade) = offered else {
            self.hands.seats.entry(id).or_default().asking = None;
            return false;
        };
        let ready = self.trades().ready_for(&who.key, trade);
        let hand = self.hands.seats.entry(id).or_default();
        hand.work = None;
        if !who.held || !ready {
            hand.hold = 0;
            hand.asking = None;
            return false;
        }
        if hand.hold > LEARN_HOLD {
            return true;
        }
        if hand.asking != Some(trade) {
            hand.asking = Some(trade);
            hand.hold = 0;
        }
        hand.hold += 1;
        if hand.hold < LEARN_HOLD {
            return true;
        }
        hand.hold = LEARN_HOLD + 1;
        hand.asking = None;
        hand.trade = Some(trade);
        self.earn_deeds(id, trade, 1);
        let q = super::questline::book().get(SCHOOLS[school].power);
        let line = q.map_or(String::new(), |q| format!("{}: {}", q.teacher.to_uppercase(), q.teach.split('.').next().unwrap_or("").trim()));
        self.sounds.push("card_pickup");
        self.hand_says(id, format!("YOU TAKE UP THE {}", trade.name()));
        if !line.is_empty() {
            self.found = Some((tick, id, line));
        }
        true
    }

    fn strike_job(&mut self, who: &Who, job: Job, castle: &'static Castle) {
        let (id, tick) = (who.id, self.tick);
        let (period, need) = beat(job);
        let rank = self.rank_of(id, job.trade()).unwrap_or(Rank::Apprentice);
        {
            let hand = self.hands.seats.entry(id).or_default();
            if let Some(Work::Strike { job: j, last, .. }) = hand.work
                && (j != job || tick.saturating_sub(last) > u64::from(period * LOSE_BEATS))
            {
                hand.work = None;
            }
        }
        if !who.swing {
            return;
        }
        let laid = self.sites.keep().map_or(0, |s| s.next());
        let fresh = self.hands.seats.get(&id).is_none_or(|h| h.work.is_none());
        if fresh {
            let blocked = match job {
                Job::Quarry => (self.keep_live().is_none() && !self.home.stockyard.wants_rubble()).then_some("THE YARD HAS RUBBLE ENOUGH: THE DELVE'S BLOCKS NEED ROOM"),
                Job::Bench if !self.rough_ready() => Some("NO ROUGH STONE TO CUT"),
                Job::Bench if self.hands.yard.cut >= CUT_MAX => Some("THE YARD IS FULL OF CUT STONE"),
                Job::Bench if self.needs_template(castle, laid) => Some("A VOUSSOIR NEEDS A TEMPLATE: FROM THE MASTER MASON"),
                Job::Bay if next_bay(castle, &self.hands.yard, laid).is_none() => Some("THE BAYS STAND AHEAD: OF THE MASONS"),
                _ => None,
            };
            if let Some(text) = blocked {
                self.hand_says(id, text);
                return;
            }
        }
        let (began, mut hits, mut flaws, last) = match self.hands.seats[&id].work {
            Some(Work::Strike { began, hits, flaws, last, .. }) => (began, hits, flaws, last),
            _ => (tick, 0, 0, 0),
        };
        let early = began != tick && tick - last < u64::from(period / 2);
        let grade = if began == tick {
            Grade::Perfect
        } else if early {
            Grade::Off
        } else {
            Grade::of(beat_dev(job, tick - began), rank)
        };
        if grade.lands() {
            hits += 1;
        } else {
            hits = hits.saturating_sub(1);
            flaws = flaws.saturating_add(1);
        }
        let point = match job {
            Job::Quarry => QUARRY,
            Job::Bench => bench_near(who.at).map_or(who.at, |k| life::BANKERS[k]),
            Job::Bay => next_bay(castle, &self.hands.yard, laid).map_or(who.at, |b| b.at),
            _ => FLOOR,
        };
        self.face(id, point);
        self.sounds.push(match (job, grade.lands()) {
            (_, false) => "hit",
            (Job::Quarry, _) => "hit_axe",
            (Job::Bay, _) => "step_wood",
            _ => "clang",
        });
        {
            let hand = self.hands.seats.entry(id).or_default();
            hand.struck = tick.max(1);
            hand.work = Some(Work::Strike { job, began, hits, flaws, last: tick });
        }
        if hits < need {
            return;
        }
        self.hands.seats.entry(id).or_default().work = None;
        let clean = flaws == 0;
        match job {
            Job::Quarry => {
                if self.stock_rough(geology::RUBBLE, 1) == 0 {
                    self.hand_says(id, "THE STORE IS FULL: NO ROOM FOR RUBBLE");
                    return;
                }
                self.sounds.push("rock_land");
                self.earn_deeds(id, Trade::Quarryman, 1 + u32::from(clean));
                self.hand_says(id, "RUBBLE COMES OFF");
            }
            Job::Bench => {
                let role = self.next_cut_role(castle, laid);
                if let Some(site) = self.keep_live() {
                    let good = match role.as_str() {
                        "voussoir" | "chevron" => "voussoir",
                        "quoin" => "quoin",
                        _ => "ashlar",
                    };
                    if let Err(why) = self.home.industry.hand_dress(crate::kingdom::catalog(), site, good) {
                        self.hand_says(id, format!("THE BLOCK WAS TAKEN: {}", why.to_uppercase()));
                        return;
                    }
                } else if self.home.stockyard.take_for_cut().is_none() {
                    self.hand_says(id, "THE BLOCK WAS TAKEN");
                    return;
                }
                self.hands.yard.cut = (self.hands.yard.cut + 1).min(CUT_MAX);
                if TEMPLATED.contains(&role.as_str()) {
                    self.hands.yard.templates = self.hands.yard.templates.saturating_sub(1);
                }
                self.sounds.push("spoil_pickup");
                self.earn_deeds(id, Trade::BankerMason, 1 + u32::from(clean));
                self.hand_says(id, format!("CUT: {}", role.to_uppercase().replace('-', " ")));
            }
            Job::Bay => {
                if let Some(site) = self.keep_live() {
                    let c = crate::kingdom::catalog();
                    match self.home.industry.stand_poles(c, site) {
                        Ok(n) => {
                            self.sounds.push("wall_up");
                            self.earn_deeds(id, Trade::Carpenter, 2 + u32::from(clean));
                            self.hand_says(id, format!("{n} POLES STAND IN THE YARD"));
                        }
                        Err(why) => self.hand_says(id, why.to_uppercase()),
                    }
                } else {
                    if let Some(b) = next_bay(castle, &self.hands.yard, laid) {
                        self.hands.yard.timber = self.hands.yard.timber.max(b.appears);
                    }
                    self.sounds.push("wall_up");
                    self.earn_deeds(id, Trade::Carpenter, 2 + u32::from(clean));
                    self.hand_says(id, "THE BAY STANDS");
                }
            }
            Job::Floor => {
                self.hands.yard.templates += TEMPLATE_USES;
                self.sounds.push("card_pickup");
                self.earn_deeds(id, Trade::MasterMason, 2);
                self.hand_says(id, "A TEMPLATE DRAWN: VOUSSOIRS AND CHEVRON MAY BE CUT");
                if self.rank_of(id, Trade::MasterMason) == Some(Rank::Master)
                    && let Err(why) = self.peg_out(id)
                {
                    self.found = Some((tick, id, why));
                }
            }
            _ => {}
        }
    }

    pub fn next_cut_role(&self, castle: &Castle, laid: u32) -> String {
        let y = &self.hands.yard;
        let at = (laid + y.cut + y.course).min(castle.len().saturating_sub(1));
        castle.stones.get(at as usize).map_or_else(String::new, |s| castle.role(s.role).to_string())
    }

    fn needs_template(&self, castle: &Castle, laid: u32) -> bool {
        TEMPLATED.contains(&self.next_cut_role(castle, laid).as_str()) && self.hands.yard.templates == 0
    }

    pub fn peg_out(&mut self, id: u32) -> Result<String, String> {
        if self.rank_of(id, Trade::MasterMason) != Some(Rank::Master) {
            return Err("ONLY A MASTER MASON PEGS OUT A CASTLE".into());
        }
        let next = works::PLANS.iter().find(|p| !self.sites.0.iter().any(|s| s.plan == **p));
        let Some(&plan) = next else {
            return Err("NO NEW PLAN IS BAKED TO PEG OUT: THE HOUSE CASTLES COME FROM BLOCKS".into());
        };
        if self.sites.0.len() >= site::MAX_SITES {
            return Err("THE MAP HAS ROOM FOR NO MORE SITES".into());
        }
        self.sites.0.push(Site { plan: plan.to_string(), laid: 0, marks: Vec::new(), trues: Vec::new(), lost: Vec::new(), bands: Vec::new() });
        Ok(plan.to_string())
    }

    fn course_job(&mut self, who: &Who, castle: &'static Castle) {
        let (id, tick) = (who.id, self.tick);
        let laid = self.sites.keep().map_or(0, |s| s.next());
        let rank = self.rank_of(id, Trade::Setter).unwrap_or(Rank::Apprentice);
        match self.hands.seats.entry(id).or_default().work {
            Some(Work::Set { index, began, plumb, tapped, level, lands }) => {
                if lands != 0 {
                    if tick >= lands {
                        self.hands.seats.entry(id).or_default().work = None;
                        self.finish_set(id, index, plumb, level);
                    }
                    return;
                }
                let (now_since, expired) = if plumb.is_none() { (tick - began, tick - began > LETGO) } else { (tick - tapped, tick - tapped > LETGO) };
                if expired {
                    self.hands.seats.entry(id).or_default().work = None;
                    self.hand_says(id, "YOU LET THE STONE GO");
                    return;
                }
                if !who.swing {
                    return;
                }
                self.face(id, stone_at(castle, index));
                if plumb.is_none() {
                    let grade = if now_since < FLINCH { Grade::Off } else { Grade::of(plumb_dev(now_since), rank) };
                    self.sounds.push("clang");
                    let hand = self.hands.seats.entry(id).or_default();
                    hand.work = Some(Work::Set { index, began, plumb: Some(grade), tapped: tick, level, lands });
                    hand.struck = tick.max(1);
                } else {
                    let grade = Grade::of(level_dev(now_since), rank);
                    self.sounds.push("clang");
                    let hand = self.hands.seats.entry(id).or_default();
                    hand.work = Some(Work::Set { index, began, plumb, tapped, level: Some(grade), lands: tick + SETTLE });
                    hand.struck = tick.max(1);
                }
            }
            _ => {
                if !who.swing {
                    return;
                }
                let yard = &self.hands.yard;
                let lim = limit(castle, yard, laid);
                let claimed: Vec<u32> = self
                    .hands
                    .seats
                    .iter()
                    .filter_map(|(_, h)| match h.work {
                        Some(Work::Set { index, .. }) => Some(index),
                        _ => None,
                    })
                    .collect();
                let keep = self.sites.keep();
                let index = (laid..laid.saturating_add(CLAIMS).min(castle.len()))
                    .find(|i| !keep.is_some_and(|s| s.stands(*i)) && !claimed.contains(i) && !yard.done.contains_key(i) && dist(who.at, stone_at(castle, *i)) <= REACH_WALL);
                let Some(index) = index else {
                    self.hand_says(id, "THE NEXT STONES ARE TAKEN");
                    return;
                };
                if index >= lim {
                    self.hand_says(id, "WAITING ON THE CARPENTER: THE BAY IS NOT RAISED");
                    return;
                }
                if let Some(why) = self.yard_holds(claimed.len() as u32 + yard.done.len() as u32 + 1) {
                    self.hand_says(id, why);
                    return;
                }
                let taken = claimed.len() as u32;
                if yard.course <= taken || yard.mortar <= taken {
                    self.hand_says(id, if yard.course <= taken { "NO STONE AT THE WALL: HOD ONE UP" } else { "NO MORTAR AT THE WALL" });
                    return;
                }
                self.sounds.push("spoil_pickup");
                self.face(id, stone_at(castle, index));
                let hand = self.hands.seats.entry(id).or_default();
                hand.work = Some(Work::Set { index, began: tick, plumb: None, tapped: 0, level: None, lands: 0 });
                hand.struck = 0;
            }
        }
    }

    fn finish_set(&mut self, id: u32, index: u32, plumb: Option<Grade>, level: Option<Grade>) {
        let (p, l) = (plumb.unwrap_or(Grade::Off), level.unwrap_or(Grade::Off));
        let tick = self.tick;
        if !p.lands() && !l.lands() {
            self.sounds.push("hit");
            self.hand_says(id, "THE STONE SLIPS: TRY AGAIN");
            return;
        }
        let truth = p.lands() && l.lands();
        let perfect = p == Grade::Perfect && l == Grade::Perfect;
        if self.hands.yard.course == 0 || self.hands.yard.mortar == 0 {
            self.hand_says(id, "THE STONE WAS TAKEN");
            return;
        }
        self.hands.yard.course -= 1;
        self.hands.yard.mortar -= 1;
        self.hands.yard.done.insert(index, (id.min(u32::from(u8::MAX)) as u8, truth));
        self.hands.yard.settled = Some((index, tick));
        self.sounds.push("rock_land");
        if truth {
            self.sounds.push("card_pickup");
        }
        self.earn_deeds(id, Trade::Setter, 1 + u32::from(truth) + u32::from(perfect));
        self.hand_says(id, if perfect { "A PERFECT SET: THE STONE IS TRUE" } else if truth { "A TRUE SET" } else { "A ROUGH SET" });
    }

    fn climb(&mut self, who: &Who) {
        let (id, tick) = (who.id, self.tick);
        let hand = self.hands.seats.entry(id).or_default();
        if !who.held {
            hand.hold = 0;
            hand.holding = None;
            return;
        }
        hand.holding = Some(Job::Ladder);
        hand.hold += 1;
        if hand.hold % 9 == 1 {
            self.sounds.push("step_wood");
        }
        let hand = self.hands.seats.entry(id).or_default();
        if hand.hold < CLIMB {
            return;
        }
        hand.hold = 0;
        hand.holding = None;
        let carry = std::mem::take(&mut hand.carry);
        match carry {
            Carry::Stone => {
                if self.hands.yard.course >= COURSE_MAX {
                    self.hands.seats.entry(id).or_default().carry = Carry::Stone;
                    self.hand_says(id, "THE COURSE IS FULL: NO ROOM FOR STONE");
                    return;
                }
                self.hands.yard.course += 1;
            }
            Carry::Mortar => {
                if self.hands.yard.mortar >= MORTAR_MAX {
                    self.hands.seats.entry(id).or_default().carry = Carry::Mortar;
                    self.hand_says(id, "THE WALL TUB IS FULL: MORTAR WAITS");
                    return;
                }
                self.hands.yard.mortar = (self.hands.yard.mortar + LOAD_DABS).min(MORTAR_MAX);
            }
            Carry::Nothing => return,
        }
        self.sounds.push("rock_land");
        let _ = tick;
        self.earn_deeds(id, Trade::Labourer, 1);
        self.hand_says(id, "THE LOAD IS SET DOWN: AT THE WALL");
    }

    fn wheel(&mut self, who: &Who) {
        let id = who.id;
        let hand = self.hands.seats.entry(id).or_default();
        if !who.held {
            hand.holding = None;
            hand.hold = 0;
            return;
        }
        hand.holding = Some(Job::Wheel);
        hand.hold = hand.hold.saturating_add(1);
        let walked = hand.hold;
        if let Some(h) = self.players.get_mut(&id) {
            h.stride += 0.115;
        }
        if walked % 12 == 1 {
            self.sounds.push("step_wood");
        }
        if walked % (4 * HZ) == 0 {
            self.earn_deeds(id, Trade::Labourer, 1);
        }
    }

    fn tick_gang(&mut self, whos: &[Who], castle: &'static Castle) {
        let site_at = [0.0f32, -4.0];
        let near_site = self.players.values().any(|h| {
            Run::works_here(h) && (h.x - units(site_at).0).hypot(h.y - units(site_at).1) <= site::NEAR_TILES * TILE_UNITS
        });
        let (laid, tick) = (self.sites.keep().map_or(0, |s| s.next()), self.tick);
        let wheel_knights = self.hands.seats.values().filter(|h| h.holding == Some(Job::Wheel)).count() as f32;
        let live = self.sim_hillside();
        if !live && tick.is_multiple_of(u64::from(MILL)) {
            self.hands.yard.tub = (self.hands.yard.tub + 1).min(TUB_MAX);
        }
        let before = crane_clock(tick.saturating_sub(1), self.hands.yard.crane_extra);
        self.hands.yard.crane_extra += wheel_knights.min(3.0) / HZ as f32;
        let now = crane_clock(tick, self.hands.yard.crane_extra);
        let turn = |t: f32| ((t / CRANE_CYCLE) + (1.0 - CRANE_DOWN)).floor();
        if near_site && life::crane(castle, laid, 0.0).is_some() && turn(now) > turn(before) && self.hands.yard.cut > 0 && self.hands.yard.course < COURSE_MAX {
            self.hands.yard.cut -= 1;
            self.hands.yard.course += 1;
            self.sounds.push("rock_land");
        }
        if !near_site {
            return;
        }
        let beside = |at: [f32; 2]| whos.iter().filter(|w| dist(w.at, at) <= BESIDE).count().min(BESIDE_MAX) as u32;
        let squires = self.squire_hands();
        let yard_at = life::YARD;
        let lim = limit(castle, &self.hands.yard, laid);
        let course_at = if laid < castle.len() { stone_at(castle, laid) } else { [0.0, 0.0] };
        let bay_at = next_bay(castle, &self.hands.yard, laid).map_or([0.0, 0.0], |b| b.at);
        let stub = |n: u32| if live { 0 } else { n };
        let plan: [(usize, u32, [f32; 2]); 7] = [
            (SLOT_QUARRY, stub(self.hired_hands(Trade::Quarryman)), QUARRY),
            (SLOT_BENCH, stub(self.hired_hands(Trade::BankerMason)), [life::BANKERS[1][0], life::BANKERS[1][1]]),
            (SLOT_HAUL, stub(self.hired_hands(Trade::Labourer) + squires), yard_at),
            (SLOT_MORTAR, stub(1), TUB),
            (SLOT_SET, stub(self.hired_hands(Trade::Setter)), course_at),
            (SLOT_BAY, stub(self.hired_hands(Trade::Carpenter)), bay_at),
            (SLOT_TEMPLATE, self.hired_hands(Trade::MasterMason), FLOOR),
        ];
        for (slot, hands, at) in plan {
            if hands == 0 {
                continue;
            }
            self.hands.yard.acc[slot] += hands * (100 + 50 * beside(at));
            let one = cost(slot).saturating_mul(100);
            while self.hands.yard.acc[slot] >= one {
                if !self.gang_unit(slot, castle, laid, lim) {
                    self.hands.yard.acc[slot] = self.hands.yard.acc[slot].min(one);
                    break;
                }
                self.hands.yard.acc[slot] -= one;
            }
        }
    }

    fn gang_unit(&mut self, slot: usize, castle: &'static Castle, laid: u32, lim: u32) -> bool {
        match slot {
            SLOT_QUARRY => return self.stock_rough(geology::RUBBLE, 1) > 0,
            SLOT_BENCH if !self.home.stockyard.can_cut() => return false,
            _ => {}
        }
        let y = &mut self.hands.yard;
        match slot {
            SLOT_BENCH => {
                if y.cut >= CUT_MAX {
                    return false;
                }
                let at = (laid + y.cut + y.course).min(castle.len().saturating_sub(1));
                let role = castle.stones.get(at as usize).map_or("", |s| castle.role(s.role));
                if TEMPLATED.contains(&role) {
                    if y.templates == 0 {
                        return false;
                    }
                    y.templates -= 1;
                }
                y.cut += 1;
                self.home.stockyard.take_for_cut();
                true
            }
            SLOT_HAUL => {
                if y.cut == 0 || y.course >= COURSE_MAX {
                    return false;
                }
                y.cut -= 1;
                y.course += 1;
                true
            }
            SLOT_MORTAR => {
                if y.mortar >= MORTAR_MAX {
                    return false;
                }
                y.mortar += 1;
                true
            }
            SLOT_SET => {
                if laid >= castle.len() || laid >= lim || y.course == 0 || y.mortar == 0 || y.done.contains_key(&laid) {
                    return false;
                }
                let claimed = self.hands.seats.values().any(|h| matches!(h.work, Some(Work::Set { index, .. }) if index == laid));
                if claimed {
                    return false;
                }
                let y = &mut self.hands.yard;
                y.course -= 1;
                y.mortar -= 1;
                y.done.insert(laid, (0, false));
                true
            }
            SLOT_BAY => {
                let Some(b) = next_bay(castle, y, laid) else { return false };
                y.timber = y.timber.max(b.appears);
                true
            }
            SLOT_TEMPLATE => {
                y.templates += TEMPLATE_USES;
                true
            }
            _ => false,
        }
    }

    fn settle_done(&mut self, castle: &'static Castle) {
        loop {
            let Some(site) = self.sites.keep() else { return };
            let laid = site.next();
            if laid >= castle.len() {
                self.hands.yard.done.clear();
                return;
            }
            let lim = limit(castle, &self.hands.yard, laid);
            let Some(&(seat, truth)) = self.hands.yard.done.get(&laid) else {
                self.hands.yard.done.retain(|&i, _| i > laid);
                return;
            };
            if laid >= lim {
                return;
            }
            self.hands.yard.done.remove(&laid);
            if seat != 0
                && let Some(site) = self.keep_live()
                && let Err(why) = self.home.industry.site_hand_set(crate::kingdom::catalog(), site)
            {
                self.hands.yard.course = (self.hands.yard.course + 1).min(COURSE_MAX);
                self.hands.yard.mortar = (self.hands.yard.mortar + 1).min(MORTAR_MAX);
                self.hand_says(u32::from(seat), format!("THE YARD COULD NOT PAY: {}", why.to_uppercase()));
                continue;
            }
            if let Some(s) = self.sites.keep_mut() {
                s.lay_set(seat, truth);
            }
        }
    }

    pub fn squire_hands(&self) -> u32 {
        self.players
            .values()
            .filter(|h| Run::works_here(h))
            .map(|h| {
                self.home.squire_guild.rosters.get(&h.name).map_or(0, |r| r.members.len().min(super::squire_guild::ROSTER_MAX) as u32)
            })
            .sum()
    }

    pub fn clerk_line(&self, castle: &Castle) -> String {
        let (laid, y) = (self.sites.keep().map_or(0, |s| s.next()), &self.hands.yard);
        if laid >= castle.len() {
            return "THE KEEP STANDS".into();
        }
        let lim = limit(castle, y, laid);
        if laid >= lim {
            return format!("WAITING ON THE CARPENTER: A BAY AT STONE {lim}");
        }
        if self.needs_template(castle, laid) {
            return "WAITING ON A TEMPLATE: THE MASTER MASON HAS NONE DRAWN".into();
        }
        if let Some(sentence) = self.keep_clerk() {
            return sentence;
        }
        let rough = self.home.stockyard.can_cut();
        if y.course == 0 && y.cut == 0 && !rough {
            return "NO STONE: THE STOCKYARD IS BARE. CUT BLOCKS BELOW AND CARRY THEM UP".into();
        }
        if y.course == 0 && y.cut == 0 {
            return "THE BANKERS WAIT ON ROUGH STONE".into();
        }
        if y.course == 0 {
            return "STONE LIES IN THE YARD: THE HOD AND THE CRANE ARE BEHIND".into();
        }
        if y.mortar == 0 {
            return "THE MORTAR IS OUT: THE MILL IS SLOW".into();
        }
        "THE SETTERS ARE THE LINE: MORE HANDS AT THE COURSE".into()
    }

    pub fn crane_seconds(&self) -> f32 {
        crane_clock(self.tick, self.hands.yard.crane_extra)
    }

    pub fn hand_prompt(&self, id: u32) -> Option<String> {
        if self.room().kind != RoomKind::Kingdom {
            return None;
        }
        let hero = self.players.get(&id).filter(|h| Run::works_here(h))?;
        let castle = self.sites.keep()?.castle()?;
        let laid = self.sites.keep()?.next();
        let at = metres((hero.x, hero.y));
        let key = super::home::Home::profile_key(hero);
        let reading = self.trades();
        let hand = self.hands.seats.get(&id);
        if let Some(s) = school_near(at) {
            let teacher = super::questline::book().get(SCHOOLS[s].power).and_then(|q| q.teacher.split_whitespace().next()).unwrap_or("THE TEACHER").to_uppercase();
            return Some(match offer(s, self.tick, |t| reading.holds(&key, t)) {
                None => format!("{teacher}: I HAVE TAUGHT YOU ALL I CAN"),
                Some(t) if !reading.ready_for(&key, t) => {
                    let asks: Vec<String> = super::trades::needs(t).iter().map(|(t, r)| format!("{} {}", r.name(), t.name())).collect();
                    format!("{teacher}: THE {}? COME BACK A {}", t.name(), asks.join(" AND "))
                }
                Some(t) => format!("{teacher}: HOLD E TO TAKE UP THE {}", t.name()),
            });
        }
        let near = jobs_near(castle, &self.hands.yard, laid, at);
        let (active, carry) = hand.map_or((None, Carry::Nothing), |h| (h.trade, h.carry));
        let job = near.iter().copied().find(|&j| Some(j.trade()) == active && Hands::fits(j, carry));
        if let Some(job) = job {
            let y = &self.hands.yard;
            let line = match job {
                Job::Quarry => "SWING ON THE BEAT: FIVE BLOWS FREE RUBBLE".to_string(),
                Job::Bench => {
                    if !self.rough_ready() {
                        "NO ROUGH STONE TO CUT: THE STOCKYARD IS BARE".to_string()
                    } else {
                        format!("SWING ON THE BEAT: CUT {}", self.next_cut_role(castle, laid).to_uppercase().replace('-', " "))
                    }
                }
                Job::Yard => if y.cut > 0 { "E: TAKE A STONE ON THE HOD".to_string() } else { "NO CUT STONE IN THE YARD".to_string() },
                Job::Tub => "E: TAKE A LOAD OF MORTAR".to_string(),
                Job::Ladder => "HOLD E TO CLIMB AND SET THE LOAD DOWN".to_string(),
                Job::Wheel => "HOLD E TO WALK THE WHEEL".to_string(),
                Job::Course => match hand.and_then(|h| h.work) {
                    Some(Work::Set { plumb: None, .. }) => "SWING WHEN THE PLUMB HANGS TRUE".to_string(),
                    Some(Work::Set { .. }) => "SWING WHEN THE BUBBLE RIDES THE MARK".to_string(),
                    _ if y.course == 0 => "NO STONE AT THE WALL: HOD ONE UP".to_string(),
                    _ if y.mortar == 0 => "NO MORTAR AT THE WALL".to_string(),
                    _ => "SWING TO SET THE NEXT STONE".to_string(),
                },
                Job::Bay => "SWING ON THE BEAT: SIX KNOCKS RAISE THE BAY".to_string(),
                Job::Floor => "SWING ON THE BEAT: DRAW A TEMPLATE".to_string(),
                Job::Board => "E: MAKE A ROUND OF THE LEDGER".to_string(),
            };
            return Some(line);
        }
        if carry != Carry::Nothing && !near.contains(&Job::Ladder) {
            return Some("CARRY IT TO THE WALL: HOLD E THERE TO CLIMB".to_string());
        }
        if active == Some(Trade::Labourer) && carry == Carry::Nothing && near.contains(&Job::Ladder) {
            return Some("FETCH A LOAD: STONE FROM THE YARD, MORTAR FROM THE TUB".to_string());
        }
        let nearest = |j: &Job| dist(at, job_point(castle, &self.hands.yard, laid, *j, at));
        let other = near.iter().copied().filter(|j| *j != Job::Ladder).min_by(|a, b| nearest(a).total_cmp(&nearest(b)))?;
        let t = other.trade();
        Some(if reading.holds(&key, t) {
            format!("{}: SWING OR PRESS E TO TAKE UP THE {}", other.name(), t.name())
        } else {
            let who = super::trades::school_of(t).and_then(|s| super::questline::book().get(s.power)).map_or("A TEACHER", |q| q.teacher.as_str());
            format!("{} NEEDS A {}: {} TEACHES IT", other.name(), t.name(), who.to_uppercase())
        })
    }

    pub fn teach_trade(&mut self, id: u32, trade: Trade, deeds: u32) {
        let Some(who) = self.trade_key(id) else { return };
        let have = self.trades().deeds(&who, trade);
        if deeds > have {
            self.earn_deeds(id, trade, deeds - have);
        }
        self.hands.seats.entry(id).or_default().trade = Some(trade);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/hands__tests.rs"]
mod tests;
