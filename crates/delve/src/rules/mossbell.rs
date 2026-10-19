//! Mossbell: catch a heavy brass marble and crack a seed-pod keeper.
//! First ten seconds: the falling ball meets a paddle and thumps into the crown.
//! Five live integers: moss_flipper=1250, moss_gravity=28, moss_bounce=85,
//! moss_nudge=180, moss_launch=1450 (256 fixed units/pixel, 30 Hz).
//! Version one leaves out multiball, scrolling and bullets. Full brief in assets/mossbell.
use super::{BTreeMap, HZ, Input, MAX_PLAYERS, RoomKind, Run, Spoil};
use serde::{Deserialize, Serialize};
use std::{cmp::Reverse, collections::VecDeque};
/// Higher lifts a caught ball more firmly (fixed units per tick).
pub const FLIPPER: u32 = 1250;
/// Higher gives a heavier, faster descent (fixed units per tick squared).
pub const GRAVITY: u32 = 28;
/// Higher retains more rebound speed, as an integer percentage.
pub const BOUNCE: u32 = 85;
/// Higher makes each fresh upward press a stronger sideways rescue.
pub const NUDGE: u32 = 180;
/// Higher sends the marble further up its dedicated launch lane.
pub const LAUNCH: u32 = 1450;
pub const LENGTH: u32 = 60 * HZ;
pub const GOLD_PER_SEAT: u32 = 10;
pub const BAY: (f32, f32) = (31.0, 18.0);
pub const Q: i32 = 256;
pub const W: i32 = 160;
pub const H: i32 = 140;
pub const BUMPERS: [(i32, i32, i32); 3] = [(44, 57, 10), (116, 57, 10), (80, 83, 11)];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Beat {
    #[default]
    Idle,
    RootTell,
    Root,
    CrownTell,
    Crown,
    Flinch,
    Defeat,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stroke {
    pub left: bool,
    pub right: bool,
    pub nudge: i8,
    pub launch: bool,
}
impl Stroke {
    fn merge(&mut self, other: Self) {
        self.left |= other.left;
        self.right |= other.right;
        if other.nudge != 0 {
            self.nudge = other.nudge;
        }
        self.launch |= other.launch;
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ball {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub trail: [(i32, i32); 4],
    pub score: u32,
    pub drains: u32,
    pub chain: u8,
    pub lit: u8,
    pub hp: u8,
    pub beat: Beat,
    pub beat_left: u32,
    pub attack: u32,
    pub flash: [u8; 3],
    pub hit: u8,
    pub freeze: u8,
    pub left: u8,
    pub right: u8,
    pub save: u8,
    pub burst: u8,
    pub impact: (i32, i32),
    pub jackpot_at: Option<u64>,
}
impl Ball {
    pub fn new() -> Self {
        Self {
            x: 145 * Q,
            y: 122 * Q,
            vx: 0,
            vy: 0,
            trail: [(145, 122); 4],
            score: 0,
            drains: 0,
            chain: 0,
            lit: 0,
            hp: 6,
            beat: Beat::Idle,
            beat_left: 120,
            attack: 0,
            flash: [0; 3],
            hit: 0,
            freeze: 0,
            left: 0,
            right: 0,
            save: 1,
            burst: 0,
            impact: (145, 122),
            jackpot_at: None,
        }
    }
    pub fn valid(&self) -> bool {
        (0..=W * Q).contains(&self.x)
            && (0..=(H + 6) * Q).contains(&self.y)
            && (-2400..=2400).contains(&self.vx)
            && (-2400..=2400).contains(&self.vy)
            && self.hp <= 6
            && self.chain <= 3
            && self.lit < 8
            && self.left <= 5
            && self.right <= 5
            && self.save <= 12
            && self.freeze <= 2
            && self.hit <= 10
            && self.burst <= 16
            && self.flash.iter().all(|&v| v <= 12)
            && self.beat_left <= 150
            && self.score <= 1000000
            && self.drains <= LENGTH
            && self
                .trail
                .iter()
                .all(|&(x, y)| (0..=W).contains(&x) && (0..=H + 6).contains(&y))
    }
    fn contact(&mut self, x: i32, y: i32) {
        self.impact = (x, y);
        self.burst = 12;
    }
    /// Closest point on an integer segment; no division by a moving normal.
    fn rail(&mut self, a: (i32, i32), b: (i32, i32), bounce: i32) -> bool {
        let (ax, ay) = (a.0 * Q, a.1 * Q);
        let (dx, dy) = ((b.0 - a.0) * Q, (b.1 - a.1) * Q);
        let length = i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy);
        let dot = (i64::from(self.x - ax) * i64::from(dx) + i64::from(self.y - ay) * i64::from(dy))
            .clamp(0, length);
        let cx = ax + (i64::from(dx) * dot / length) as i32;
        let cy = ay + (i64::from(dy) * dot / length) as i32;
        self.circle(cx, cy, 3 * Q, bounce)
    }
    fn circle(&mut self, cx: i32, cy: i32, r: i32, bounce: i32) -> bool {
        let (mut dx, mut dy) = (self.x - cx, self.y - cy);
        if dx == 0 && dy == 0 {
            dy = -Q;
            dx = 1;
        }
        let d2 = i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy);
        if d2 >= i64::from(r) * i64::from(r) {
            return false;
        }
        let d = d2.isqrt() as i32;
        let closing = i64::from(self.vx) * i64::from(dx) + i64::from(self.vy) * i64::from(dy);
        self.x = cx + dx * r / d.max(1);
        self.y = cy + dy * r / d.max(1);
        if closing < 0 {
            self.vx -= (closing * i64::from(dx) * i64::from(100 + bounce) / (d2 * 100)) as i32;
            self.vy -= (closing * i64::from(dy) * i64::from(100 + bounce) / (d2 * 100)) as i32;
            true
        } else {
            false
        }
    }
    pub fn step(
        &mut self,
        stroke: Stroke,
        tune: &super::tune::Tune,
        age: u32,
        seed: u64,
        tick: u64,
    ) {
        if stroke.left {
            self.left = 5;
        }
        if stroke.right {
            self.right = 5;
        }
        for f in &mut self.flash {
            *f = f.saturating_sub(1);
        }
        self.hit = self.hit.saturating_sub(1);
        self.burst = self.burst.saturating_sub(1);
        if self.hp == 0 {
            self.beat = Beat::Defeat;
        } else {
            self.beat_left = self.beat_left.saturating_sub(1);
            if self.beat_left == 0 {
                match self.beat {
                    Beat::Idle | Beat::Flinch => {
                        self.beat = if (self.attack + seed as u32) & 1 == 0 {
                            Beat::RootTell
                        } else {
                            Beat::CrownTell
                        };
                        self.attack += 1;
                        self.beat_left = if self.hp > 2 { 36 } else { 30 };
                    }
                    Beat::RootTell => {
                        self.beat = Beat::Root;
                        self.beat_left = 12;
                    }
                    Beat::CrownTell => {
                        self.beat = Beat::Crown;
                        self.beat_left = 12;
                    }
                    _ => {
                        self.beat = Beat::Idle;
                        self.beat_left = 90 + u32::from(self.hp) * 10;
                    }
                }
            }
        }
        if self.freeze > 0 {
            self.freeze -= 1;
            return;
        }
        if self.save > 0 {
            self.save -= 1;
            if self.save == 0 || stroke.launch {
                self.save = 0;
                self.x = 145 * Q;
                self.y = 122 * Q;
                self.vx = 0;
                self.vy = -(tune.moss_launch as i32);
                self.trail = [(145, 122); 4];
            }
            return;
        }
        self.trail.rotate_right(1);
        self.trail[0] = (self.x / Q, self.y / Q);
        self.vx += i32::from(stroke.nudge) * tune.moss_nudge as i32;
        if stroke.nudge != 0 {
            self.vy -= tune.moss_nudge as i32 / 2;
        }
        // The launch lane clears the return bank before the curved crown feed.
        if self.x > 140 * Q && self.y < 22 * Q && self.vx == 0 {
            self.vx = -(360 + (seed.rotate_left(age % 64) & 63) as i32);
        }
        // A paddle sweeps its full wedge, so a ball below the raised tip is caught.
        for (swing, left) in [(stroke.left, true), (stroke.right, false)] {
            let along = if left {
                self.x / Q - 37
            } else {
                123 - self.x / Q
            };
            if swing && (0..=33).contains(&along) && self.vy > 0 {
                let raised = 119 * Q - along * 8 * Q / 33;
                let resting = 119 * Q + along * 11 * Q / 33;
                if self.y >= raised - 3 * Q && self.y <= resting + 3 * Q {
                    self.y = raised - 4 * Q;
                    self.vy = -(tune.moss_flipper as i32);
                    self.vx = if left {
                        320 + along * 12
                    } else {
                        -320 - along * 12
                    };
                    self.contact(self.x / Q, self.y / Q);
                    self.freeze = 2;
                }
            }
        }
        self.vy += tune.moss_gravity as i32;
        let bounce = tune.moss_bounce as i32;
        for _ in 0..4 {
            self.x += self.vx / 4;
            self.y += self.vy / 4;
            if self.x < 7 * Q {
                self.x = 7 * Q;
                self.vx = self.vx.abs() * bounce / 100;
            }
            if self.x > 153 * Q {
                self.x = 153 * Q;
                self.vx = -self.vx.abs() * bounce / 100;
            }
            if self.y < 6 * Q {
                self.y = 6 * Q;
                self.vy = self.vy.abs() * bounce / 100;
            }
            // Sloping banks guide a fair return toward the paddle tips.
            self.rail((8, 98), (38, 123), bounce);
            if !(self.x > 140 * Q && self.vy < 0) {
                self.rail((152, 98), (122, 123), bounce);
            }
            let ltip = if self.left > 0 { (70, 111) } else { (70, 130) };
            let rtip = if self.right > 0 { (90, 111) } else { (90, 130) };
            for (left, a, b, active) in [
                (true, (37, 119), ltip, self.left),
                (false, (123, 119), rtip, self.right),
            ] {
                let falling = self.vy > 0;
                if self.rail(a, b, bounce) && active > 0 && falling {
                    self.vy = -(tune.moss_flipper as i32);
                    self.vx = if left {
                        320 + (self.x / Q - 37) * 12
                    } else {
                        -320 - (123 - self.x / Q) * 12
                    };
                    self.contact(self.x / Q, self.y / Q);
                    self.freeze = 2;
                }
            }
            for (i, (x, y, r)) in BUMPERS.iter().copied().enumerate() {
                if self.circle(x * Q, y * Q, (r + 3) * Q, bounce) && self.flash[i] == 0 {
                    self.flash[i] = 12;
                    self.score += 25;
                    self.lit |= 1 << i;
                    self.chain = self.lit.count_ones() as u8;
                    self.vx += if self.x > x * Q { 90 } else { -90 };
                    self.vy -= 100;
                    self.contact(x, y);
                }
            }
            if self.hp > 0 && self.circle(80 * Q, 27 * Q, 19 * Q, bounce) && self.hit == 0 {
                self.hp -= 1;
                self.score += 100;
                self.hit = 10;
                self.contact(80, 33);
                if self.hp == 0 {
                    self.score += 300;
                    self.beat = Beat::Defeat;
                } else {
                    self.beat = Beat::Flinch;
                    self.beat_left = 12;
                }
            }
            // Both attacks act only inside their identically rendered tell areas.
            if self.beat == Beat::Root && (72 * Q..=82 * Q).contains(&self.y) {
                self.vx = if self.attack & 1 == 0 { 850 } else { -850 };
                self.vy = 250;
            }
            if self.beat == Beat::Crown
                && (60 * Q..=100 * Q).contains(&self.x)
                && (20 * Q..=43 * Q).contains(&self.y)
            {
                self.vy = 900;
            }
            self.vx = self.vx.clamp(-2200, 2200);
            self.vy = self.vy.clamp(-2200, 2200);
            if self.y > (H + 3) * Q {
                self.drains += 1;
                self.chain = 0;
                self.lit = 0;
                self.save = 12;
                self.x = 145 * Q;
                self.y = 122 * Q;
                self.vx = 0;
                self.vy = 0;
                self.contact(80, 134);
                break;
            }
        }
        self.x = self.x.clamp(0, W * Q);
        self.y = self.y.clamp(0, (H + 6) * Q);
        self.left = self.left.saturating_sub(1);
        self.right = self.right.saturating_sub(1);
        if self.hp == 0 && self.lit == 7 && self.jackpot_at.is_none() {
            self.jackpot_at = Some(tick);
            self.burst = 16;
            self.impact = (80, 63);
        }
    }
}
impl Default for Ball {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Sample {
    tick: u64,
    age: u32,
    before: Ball,
    stroke: Stroke,
    tune: super::tune::Tune,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub ball: Ball,
    pub seed: u64,
    pub keys: (bool, bool, bool, bool),
    #[serde(skip)]
    history: VecDeque<Sample>,
}
impl Table {
    pub fn new(seed: u64) -> Self {
        Self {
            ball: Ball::new(),
            seed,
            keys: (false, false, false, false),
            history: VecDeque::new(),
        }
    }
    pub fn valid(&self) -> bool {
        self.ball.valid()
    }
    pub fn step(&mut self, input: Input, tune: &super::tune::Tune, age: u32, tick: u64, allowance: u64) {
        let i = if input.valid()
            && input.latch_tick.is_none_or(|sample| sample <= tick && tick - sample <= allowance)
        {
            input
        } else {
            Input::default()
        };
        let keys = (
            i.move_x < 0 || i.fire,
            i.move_x > 0 || i.fire,
            i.move_y < 0,
            i.fire,
        );
        let mut stroke = Stroke {
            left: keys.0 && !self.keys.0,
            right: keys.1 && !self.keys.1,
            nudge: if keys.2 && !self.keys.2 {
                if self.ball.x < 80 * Q { 1 } else { -1 }
            } else {
                0
            },
            launch: keys.3 && !self.keys.3,
        };
        self.keys = keys;
        if let Some(sample) = i.latch_tick {
            // A stamped press is applied to the picture's tick, never again on arrival.
            stroke = Stroke {
                left: keys.0,
                right: keys.1,
                nudge: if keys.2 {
                    if self.ball.x < 80 * Q { 1 } else { -1 }
                } else {
                    0
                },
                launch: keys.3,
            };
            if sample < tick {
                if let Some(at) = self.history.iter().position(|h| h.tick == sample) {
                    if keys.2 {
                        stroke.nudge = if self.history[at].before.x < 80 * Q {
                            1
                        } else {
                            -1
                        };
                    }
                    self.history[at].stroke.merge(stroke);
                    self.ball = self.history[at].before.clone();
                    for h in self.history.iter_mut().skip(at) {
                        h.before = self.ball.clone();
                        self.ball.step(h.stroke, &h.tune, h.age, self.seed, h.tick);
                    }
                }
                stroke = Stroke::default();
            } else if sample > tick {
                stroke = Stroke::default();
            }
        }
        self.history.push_back(Sample {
            tick,
            age,
            before: self.ball.clone(),
            stroke,
            tune: tune.clone(),
        });
        // One second of local collision history; omitted from network pictures.
        if self.history.len() > HZ as usize {
            self.history.pop_front();
        }
        self.ball.step(stroke, tune, age, self.seed, tick);
    }
    pub fn bot_input(&self) -> Input {
        let b = &self.ball;
        Input {
            fire: b.save > 0 || (b.vy > 0 && b.y > 103 * Q && b.y < 124 * Q && !self.keys.3),
            move_y: if b.y > 127 * Q && !self.keys.2 { -1 } else { 0 },
            ..Default::default()
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub tables: BTreeMap<u32, Table>,
    pub seed: u64,
    #[serde(default)]
    pub started: u64,
    pub left: u32,
    pub order: Vec<u32>,
    pub key: bool,
    pub jackpot: Vec<u32>,
    pub jackpot_tick: Option<u64>,
}
impl Session {
    pub fn new(seats: impl Iterator<Item = u32>, seed: u64) -> Self {
        Self {
            tables: seats.map(|id| (id, Table::new(seed))).collect(),
            seed,
            started: 0,
            left: LENGTH,
            order: vec![],
            key: true,
            jackpot: vec![],
            jackpot_tick: None,
        }
    }
    pub fn points(&self, id: u32) -> u32 {
        self.tables[&id].ball.score + if self.jackpot.contains(&id) { 500 } else { 0 }
    }
    pub fn score(&self, id: u32) -> (Reverse<u32>, u32, u64) {
        let b = &self.tables[&id].ball;
        (
            Reverse(self.points(id)),
            b.drains,
            b.jackpot_at.unwrap_or(u64::MAX),
        )
    }
    pub fn place(&self, id: u32) -> usize {
        1 + self
            .tables
            .keys()
            .filter(|&&r| self.score(r) < self.score(id))
            .count()
    }
    pub fn race(&mut self) {
        self.jackpot_tick = self.tables.values().filter_map(|t| t.ball.jackpot_at).min();
        self.jackpot = self
            .tables
            .iter()
            .filter(|(_, t)| t.ball.jackpot_at.is_some() && t.ball.jackpot_at == self.jackpot_tick)
            .map(|(&id, _)| id)
            .collect();
    }
    pub fn payout(&self, id: u32) -> u32 {
        let Some(&first) = self.order.first() else {
            return 0;
        };
        if self.left > 0 || self.score(id) != self.score(first) {
            return 0;
        }
        let tied = self
            .tables
            .keys()
            .filter(|&&r| self.score(r) == self.score(first))
            .count() as u32;
        let before = self
            .tables
            .range(..id)
            .filter(|&(r, _)| self.score(*r) == self.score(first))
            .count() as u32;
        let pot = GOLD_PER_SEAT * self.tables.len() as u32;
        pot / tied + u32::from(before < pot % tied)
    }
    pub fn valid(&self) -> bool {
        !self.tables.is_empty()
            && self.tables.len() <= MAX_PLAYERS as usize
            && self.left <= LENGTH
            && self
                .tables
                .iter()
                .all(|(&id, t)| (1..=MAX_PLAYERS).contains(&id) && t.seed == self.seed && t.valid())
            && self.order.len() <= self.tables.len()
            && self
                .order
                .iter()
                .enumerate()
                .all(|(n, id)| self.tables.contains_key(id) && !self.order[..n].contains(id))
            && (self.left > 0 || self.order.len() == self.tables.len())
            && self
                .jackpot
                .iter()
                .enumerate()
                .all(|(n, id)| self.tables.contains_key(id) && !self.jackpot[..n].contains(id))
            && self.jackpot_tick == self.tables.values().filter_map(|t| t.ball.jackpot_at).min()
            && self.jackpot
                == self
                    .tables
                    .iter()
                    .filter(|(_, t)| {
                        t.ball.jackpot_at.is_some() && t.ball.jackpot_at == self.jackpot_tick
                    })
                    .map(|(&id, _)| id)
                    .collect::<Vec<_>>()
    }
    pub fn repair(&mut self) {
        self.tables.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        if self.tables.is_empty() {
            self.tables.insert(1, Table::new(self.seed));
        }
        *self = Self::new(self.tables.keys().copied(), self.seed);
    }
}
impl Run {
    pub fn mossbell_showing(&self) -> bool {
        self.room().kind == RoomKind::Bathhouse && self.mossbell.is_some()
    }
    pub(super) fn tick_mossbell(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Bathhouse {
            self.mossbell = None;
            return;
        }
        let pressed = inputs.iter().any(|(id, i)| {
            i.valid()
                && i.interact
                && self.players.get(id).is_some_and(|h| {
                    h.hp > 0
                        && !h.stone
                        && !h.away
                        && !self.exits.outside.contains_key(id)
                        && (h.x - BAY.0).hypot(h.y - BAY.1) < 3.0
                })
        });
        if self.mossbell.is_none() {
            if pressed {
                self.start_mossbell();
            }
            return;
        }
        let mut s = self.mossbell.take().unwrap();
        s.tables
            .retain(|id, _| self.players.contains_key(id) && !self.exits.outside.contains_key(id));
        s.order.retain(|id| s.tables.contains_key(id));
        s.race();
        if s.tables.is_empty() {
            return;
        }
        if s.left == 0 {
            let take = inputs
                .iter()
                .any(|(id, i)| s.tables.contains_key(id) && i.valid() && i.interact);
            if take && !s.key {
                self.start_mossbell();
            } else {
                s.key = take;
                self.mossbell = Some(s);
            }
            return;
        }
        let age = LENGTH - s.left;
        s.left -= 1;
        let old_jackpot = s.jackpot.clone();
        for (&id, t) in &mut s.tables {
            if self.players.get(&id).is_none_or(|h| h.away || h.stone) {
                continue;
            }
            let old = t.ball.clone();
            t.step(
                inputs.get(&id).copied().unwrap_or_default(),
                &self.tune,
                age,
                self.tick,
                self.latch_input_age.get(&id).copied().unwrap_or(0),
            );
            if t.ball.score > old.score {
                self.sounds
                    .push(if t.ball.hp < old.hp { "hit" } else { "clang" });
            }
            if t.ball.drains > old.drains {
                self.sounds.push("play_card");
            }
        }
        s.race();
        if s.jackpot != old_jackpot {
            self.sounds.push("spoil_pickup");
        }
        if s.left == 0 {
            s.order = s.tables.keys().copied().collect();
            let mut order = std::mem::take(&mut s.order);
            order.sort_by_key(|&id| (s.score(id), id));
            s.order = order;
            s.key = true;
            let record = &mut self.home.countryside.games.mossbell;
            record.best = record
                .best
                .max(s.tables[&s.order[0]].ball.score.min(1000) as u16);
            if !s.jackpot.is_empty() {
                record.wins = record.wins.saturating_add(1).min(record.plays);
            }
            for (&id, h) in &mut self.players {
                if s.tables.contains_key(&id) {
                    h.laurel = s.place(id) == 1;
                    h.carried.add(Spoil::Gold, s.payout(id));
                    if h.laurel {
                        h.booby = None;
                    }
                }
            }
            self.sounds.push("spoil_pickup");
        }
        self.mossbell = Some(s);
    }
    fn start_mossbell(&mut self) {
        let seed = self.rng.next();
        let seats = self
            .players
            .iter()
            .filter(|(id, h)| {
                h.hp > 0 && !h.stone && !h.away && !self.exits.outside.contains_key(id)
            })
            .map(|(&id, _)| id);
        let mut s = Session::new(seats, seed);
        s.started = self.tick + 1;
        if s.tables.is_empty() {
            return;
        }
        self.mossbell = Some(s);
        self.home.countryside.games.mossbell.plays =
            self.home.countryside.games.mossbell.plays.saturating_add(1);
        self.sounds.push("play_card");
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn mossbell_for_test() -> Run {
        let mut r = Run::at_home(
            41,
            1,
            Some("Gawain"),
            Default::default(),
            Default::default(),
        );
        let at = r
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Bathhouse)
            .unwrap();
        r.arrive(at, (BAY.0 / 2.0, BAY.1 / 2.0));
        r
    }
}
