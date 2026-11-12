//! Pennon Rush's show: what lately happened on the ramparts, kept so the
//! picture can make a fuss of it (`render::overworld::arena::pennon` draws
//! it).
//!
//! The rules (`pennon`) tell it what happened in each knight's lane: a
//! sentry struck or felled, a knight knocked down, a ward found, a boot
//! landing, a bolt loosed. It answers with the calls to shout (`Call`), the
//! table's count of pennons felled (every third one rallies the whole table)
//! and who leads. Nothing here moves a knight: the lanes stay what they were
//! (the only rule it adds is the rally's ward, which `Run::tick_pennon`
//! hands out), and it is otherwise only memory, with the juice kit's own
//! weights (`juice::Weight`) saying how hard each moment hits.
//!
//! Time is the show's own tick count, the siege's (`Show::t`), so the host,
//! a friend's mirror and a browser guest draw the same moment. Every list is
//! capped (`valid`).

use super::{FLOOR, Foe, Lane, Q};
use crate::rules::juice::Weight;
use crate::rules::{HZ, MAX_PLAYERS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Ticks of title and 3-2-1 before the siege begins: the lanes wait, so the
/// whole table starts on the same breath.
pub const HOLD: u32 = 66;
/// Ticks the GO stands over the first strides.
pub const GO_LIFE: u32 = 26;
/// Every this many pennons felled at the table, every knight without a ward
/// is handed one.
pub const RALLY_EVERY: u32 = 3;
/// Ticks each kind of moment stays on the books for its effects.
pub const HIT_LIFE: u32 = 40;
pub const DOWN_LIFE: u32 = 44;
pub const PUFF_LIFE: u32 = 20;
pub const CALL_LIFE: u32 = 64;
/// The last seconds are called out loud.
pub const LAST_SECONDS: u32 = 5;
/// A lead must be held this long (ticks) before it is called again.
const LEAD_GAP: u32 = 2 * HZ;

const MOST_HITS: usize = 10;
const MOST_DOWNS: usize = 8;
const MOST_PUFFS: usize = 24;
const MOST_CALLS: usize = 8;

/// A pennon struck or felled in `seat`'s lane: which foe (`0..=3`, the
/// captain last), where (world pixels), how it weighed, what the hit was
/// worth, the knight's run of hits and whether the foe was felled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub seat: u32,
    pub foe: u8,
    pub x: i16,
    pub y: i16,
    pub weight: Weight,
    pub gain: u16,
    pub chain: u16,
    pub kill: bool,
    pub born: u32,
}

impl Hit {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

/// A knight knocked down, or a ward broken in their stead: where, whether
/// they fell into the moat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Down {
    pub seat: u32,
    pub x: i16,
    /// The run of hits the fall broke.
    pub lost: u32,
    pub pit: bool,
    pub ward: bool,
    pub born: u32,
}

/// What a little moment was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Puff {
    /// A boot leaves the stone.
    Jump,
    /// A boot lands.
    Land,
    /// A hook-bow loosed: where the bolt left, which way it went.
    Muzzle,
    /// A ward picked up (or handed round by the rally).
    Ward,
    /// A sentry wakes and takes aim.
    Wake,
    /// A foe's bolt leaves it, or the captain's blow lands.
    Strike,
    /// A knight set down on the safe stone after a fall.
    Rise,
}

/// A little moment in `seat`'s lane at `x` (world pixels).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moment {
    pub kind: Puff,
    pub seat: u32,
    pub x: i16,
    pub y: i16,
    /// A way (`-1`, `0` or `1` each) for those that have one; for `Strike`
    /// and `Wake` the foe's place.
    pub dx: i8,
    pub dy: i8,
    pub born: u32,
}

impl Moment {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

/// What a banner calls. Words are the picture's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cry {
    /// The n-th sentry (`1..=3`) felled.
    Wave(u8),
    /// A run of this many hits without a fall.
    Combo(u8),
    /// The captain has woken.
    Captain,
    /// The captain's banner is torn (`1`) or he makes his last stand (`2`).
    Torn(u8),
    /// The captain is felled: the siege is broken.
    Broken,
    /// Every third pennon at the table: a ward for all.
    Rally,
    /// The knight went ahead.
    Lead,
}

impl Cry {
    /// How loudly it is shown: the banner with the higher rank stays on top.
    pub fn rank(self) -> u8 {
        match self {
            Cry::Broken => 9,
            Cry::Captain => 7,
            Cry::Torn(_) => 6,
            Cry::Rally => 5,
            Cry::Combo(_) => 5,
            Cry::Wave(_) => 4,
            Cry::Lead => 3,
        }
    }
}

/// A banner called for a knight (`who` 0: for the whole table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub cry: Cry,
    pub who: u32,
    /// For a `Wave`: this knight was the table's first to fell it.
    pub first: bool,
    pub born: u32,
}

impl Call {
    pub fn age(&self, now: u32) -> u32 {
        now.saturating_sub(self.born)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    /// Ticks since the siege was called, the title included.
    pub t: u32,
    /// The best the table had made before this round, for a new best.
    pub best0: u16,
    pub hits: Vec<Hit>,
    pub downs: Vec<Down>,
    pub moments: Vec<Moment>,
    pub calls: Vec<Call>,
    /// Pennons felled at the table, every lane's.
    pub felled: u32,
    /// Rallies handed round.
    pub rallies: u32,
    /// The knight now ahead on their own.
    pub lead: Option<u32>,
    /// The tick the lead was last called.
    pub led: u32,
    /// The tick the siege ended (the time ran out or the last captain fell).
    pub end: Option<u32>,
    /// Who felled each of the four first.
    pub first: [u32; 4],
    /// Each knight's highest combo call, so it is called once.
    called: BTreeMap<u32, u16>,
    /// Each knight's captain stage last called.
    torn: BTreeMap<u32, u8>,
}

/// What a lane was before the tick, to read the tick's changes off it.
#[derive(Clone, Debug)]
pub struct Snap {
    pub hp: [u8; 4],
    pub stage: [usize; 4],
    pub attack: [u32; 4],
    pub strike: [bool; 4],
    pub x: i32,
    pub y: i32,
    pub grounded: bool,
    pub falls: u32,
    pub ward: u32,
    pub chain: u32,
    pub last_shot: Option<u64>,
    pub pickups: [bool; 2],
    pub down: u32,
}

impl Snap {
    pub fn of(d: &Lane) -> Snap {
        let f = |i: usize| d.foes.get(i);
        Snap {
            hp: std::array::from_fn(|i| f(i).map_or(0, |f| f.hp)),
            stage: std::array::from_fn(|i| f(i).map_or(0, Foe::stage)),
            attack: std::array::from_fn(|i| f(i).map_or(0, |f| f.attack)),
            strike: std::array::from_fn(|i| f(i).is_some_and(|f| f.beat == super::Beat::Attack)),
            x: d.x,
            y: d.y,
            grounded: d.ground.is_some(),
            falls: d.falls,
            ward: d.ward,
            chain: d.chain,
            last_shot: d.last_shot,
            pickups: d.pickups,
            down: d.down,
        }
    }
}

fn px(v: i32) -> i16 {
    v.clamp(-2000, 2000) as i16
}

/// The most hits each foe takes.
fn most(i: usize) -> u32 {
    if i == 3 { 18 } else { 3 }
}

impl Show {
    pub fn quiet(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let seat = |id: u32| (1..=MAX_PLAYERS).contains(&id);
        self.hits.len() <= MOST_HITS
            && self.downs.len() <= MOST_DOWNS
            && self.moments.len() <= MOST_PUFFS
            && self.calls.len() <= MOST_CALLS
            && self.hits.iter().all(|h| seat(h.seat) && h.foe < 4)
            && self.downs.iter().all(|d| seat(d.seat))
            && self.moments.iter().all(|m| seat(m.seat))
            && self.calls.iter().all(|c| c.who == 0 || seat(c.who))
            && self.lead.is_none_or(seat)
            && self.first.iter().all(|&f| f == 0 || seat(f))
            && self.called.len() <= MAX_PLAYERS as usize
            && self.torn.len() <= MAX_PLAYERS as usize
            && self.felled <= 4 * MAX_PLAYERS
            && self.rallies <= 4 * MAX_PLAYERS
    }

    /// One tick of the siege (call first, before anything records): let go
    /// what is old.
    pub fn tick(&mut self) {
        self.t = self.t.saturating_add(1);
        let now = self.t;
        self.hits.retain(|h| h.age(now) < HIT_LIFE);
        self.downs.retain(|d| now.saturating_sub(d.born) < DOWN_LIFE);
        self.moments.retain(|m| m.age(now) < PUFF_LIFE);
        self.calls.retain(|c| c.age(now) < CALL_LIFE);
    }

    fn call(&mut self, cry: Cry, who: u32, first: bool) {
        if self.calls.len() >= MOST_CALLS {
            self.calls.remove(0);
        }
        self.calls.push(Call { cry, who, first, born: self.t });
    }

    fn moment(&mut self, kind: Puff, seat: u32, x: i32, y: i32, way: (i32, i32)) {
        if self.moments.len() >= MOST_PUFFS {
            self.moments.remove(0);
        }
        self.moments.push(Moment {
            kind,
            seat,
            x: px(x),
            y: px(y),
            dx: way.0.clamp(-1, 1) as i8,
            dy: way.1.clamp(-1, 1) as i8,
            born: self.t,
        });
    }

    /// Read what `seat`'s lane did this tick off what it was (`before`).
    /// Returns the heaviest weight of a blow landed, for the room's hold.
    pub fn observe(&mut self, seat: u32, before: &Snap, d: &Lane) -> Option<Weight> {
        let mut heaviest = None;
        let now = self.t;
        let hits = |hp: &[u8; 4]| -> u32 { (0..4).map(|i| most(i).saturating_sub(u32::from(hp[i]))).sum() };
        let after: [u8; 4] = std::array::from_fn(|i| d.foes.get(i).map_or(0, |f| f.hp));
        let (was, is) = (hits(&before.hp) * 500 / 27, hits(&after) * 500 / 27);
        for i in 0..4 {
            let Some(f) = d.foes.get(i) else { continue };
            if before.hp[i] > f.hp {
                let kill = f.hp == 0;
                let weight = if kill {
                    Weight::of(1, most(i), true, i == 3)
                } else if f.stage() != before.stage[i] {
                    Weight::Crushing
                } else {
                    Weight::of(1, most(i), false, false)
                };
                if self.hits.len() >= MOST_HITS {
                    self.hits.remove(0);
                }
                self.hits.push(Hit {
                    seat,
                    foe: i as u8,
                    x: px(f.x),
                    y: px(FLOOR - 30),
                    weight,
                    gain: (is - was).min(999) as u16,
                    chain: d.chain.min(999) as u16,
                    kill,
                    born: now,
                });
                heaviest = heaviest.max(Some(weight));
                if kill {
                    self.felled += 1;
                    if i < 3 {
                        if self.first[i] == 0 {
                            self.first[i] = seat;
                        }
                        let first = self.first[i] == seat;
                        self.call(Cry::Wave(i as u8 + 1), seat, first);
                    } else {
                        if self.first[3] == 0 {
                            self.first[3] = seat;
                        }
                        self.call(Cry::Broken, seat, self.first[3] == seat);
                    }
                } else if f.stage() != before.stage[i] && i == 3 {
                    let stage = f.stage() as u8;
                    if self.torn.get(&seat).copied().unwrap_or(0) < stage {
                        self.torn.insert(seat, stage);
                        self.call(Cry::Torn(stage), seat, false);
                    }
                }
            }
            // A foe wakes: its first aimed beat.
            if before.attack[i] == 0 && f.attack > 0 {
                self.moment(Puff::Wake, seat, f.x, FLOOR - if f.captain { 70 } else { 44 }, (i as i32, 0));
                if f.captain {
                    self.call(Cry::Captain, seat, false);
                }
            }
            // A foe's blow leaves it.
            if !before.strike[i] && f.beat == super::Beat::Attack && f.hp > 0 {
                self.moment(Puff::Strike, seat, f.x, FLOOR, (i as i32, i32::from(f.hook)));
            }
        }
        // The run of hits, called at each fifth.
        if d.chain > before.chain && d.chain >= 5 && d.chain % 5 == 0 {
            let top = self.called.entry(seat).or_default();
            if u32::from(*top) < d.chain {
                *top = d.chain as u16;
                self.call(Cry::Combo(d.chain.min(250) as u8), seat, false);
            }
        }
        if d.chain == 0 {
            self.called.remove(&seat);
        }
        // Knocked down, or a ward broken for them.
        if d.falls > before.falls || (before.ward > 0 && d.ward == 0 && d.falls == before.falls) {
            if self.downs.len() >= MOST_DOWNS {
                self.downs.remove(0);
            }
            let pit = d.falls > before.falls && d.y / Q > FLOOR + 20;
            self.downs.push(Down {
                seat,
                x: px(d.x / Q),
                lost: before.chain,
                pit,
                ward: d.falls == before.falls,
                born: now,
            });
            heaviest = heaviest.max(Some(Weight::Crushing));
        }
        // A fall ends: the knight is set down on the safe stone.
        if before.down == 1 && d.down == 0 {
            self.moment(Puff::Rise, seat, d.x / Q, FLOOR, (0, 0));
        }
        // A boot leaves the stone, a boot lands.
        if before.grounded && d.ground.is_none() && d.vy < 0 {
            self.moment(Puff::Jump, seat, d.x / Q, FLOOR, (d.dir, 0));
        }
        if !before.grounded && d.ground.is_some() && d.down == 0 && before.down == 0 {
            self.moment(Puff::Land, seat, d.x / Q, FLOOR, (d.dir, 0));
        }
        // A bolt leaves the hook-bow.
        if d.last_shot != before.last_shot && d.last_shot.is_some() {
            let (ax, ay) = d.aim;
            let y = d.y / Q - if d.duck { 10 } else if ay < 0 { 29 } else if ay > 0 { 12 } else { 18 };
            self.moment(Puff::Muzzle, seat, d.x / Q + d.dir * 18, y, (ax, ay));
        }
        // A ward found.
        if d.pickups != before.pickups {
            self.moment(Puff::Ward, seat, d.x / Q, FLOOR - 20, (0, 0));
        }
        heaviest
    }

    /// The tick's tallies, after every lane has moved: who leads (judged on
    /// the board's own order), and whether the table has felled a third
    /// pennon since the last rally. Returns whether a rally is due.
    pub fn standings(&mut self, scores: &[(u32, u32)]) -> bool {
        let top = scores.iter().map(|&(_, s)| s).max().unwrap_or(0);
        let leaders: Vec<u32> = scores.iter().filter(|&&(_, s)| s == top && top > 0).map(|&(id, _)| id).collect();
        let lead = (leaders.len() == 1 && scores.len() > 1 && top >= 60).then(|| leaders[0]);
        if let Some(one) = lead
            && self.lead != Some(one)
        {
            if self.lead.is_some() && self.t.saturating_sub(self.led) >= LEAD_GAP {
                self.call(Cry::Lead, one, false);
                self.led = self.t;
            }
            self.lead = Some(one);
        }
        self.felled / RALLY_EVERY > self.rallies
    }

    /// The rally: `got` knights were handed a ward.
    pub fn rallied(&mut self, got: &[(u32, i32)]) {
        self.rallies += 1;
        for &(seat, x) in got {
            self.moment(Puff::Ward, seat, x, FLOOR - 20, (0, 1));
        }
        if !got.is_empty() {
            self.call(Cry::Rally, 0, false);
        }
    }

    /// The latest hit on `foe` in `seat`'s lane, if lately.
    pub fn struck(&self, seat: u32, foe: usize) -> Option<&Hit> {
        self.hits.iter().rev().find(|h| h.seat == seat && usize::from(h.foe) == foe)
    }

    /// Ticks since the most recent moment of `kind` in `seat`'s lane.
    pub fn since(&self, kind: Puff, seat: u32) -> Option<u32> {
        self.moments.iter().filter(|m| m.kind == kind && m.seat == seat).map(|m| m.age(self.t)).min()
    }
}
