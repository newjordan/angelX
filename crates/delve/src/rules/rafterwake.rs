//! Rafterwake: a late boot catches the lift in the bellwright's loft.
//! First ten seconds: run, hop, release for a short arc, catch a ladder.
//! Five F2 integers: gravity=112, jump=1536, run=640 (1/256 px units),
//! coyote=4 ticks, buffer=5 ticks. Ladder pace=384. See assets/rafterwake/BRIEF.md.
//! One seeded 75-second scaffold, no campaign. Each seat owns contacts and camera.
use super::{BTreeMap, HZ, Input, MAX_PLAYERS, RoomKind, Run, Spoil};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
/// Larger makes the descent heavier, in fixed units per tick squared.
pub const GRAVITY: u32 = 112;
/// Larger clears a higher deck, in fixed units per tick.
pub const JUMP: u32 = 1536;
/// Larger increases horizontal control, in fixed units per tick.
pub const RUN: u32 = 640;
/// Late presses still belong to the plank for this many ticks.
pub const COYOTE: u32 = 4;
/// Early presses wait this many ticks for a landing.
pub const BUFFER: u32 = 5;
pub const Q: i32 = 256;
pub const LENGTH: u32 = 75 * HZ;
pub const GOLD_PER_SEAT: u32 = 10;
pub const WORK_PER_RECORD: u32 = 1;
pub const BAY: (f32, f32) = (24.0, 12.0);
pub const FLOOR: i32 = 540;
pub const TOP: i32 = 108;
pub const LADDER: i32 = 384;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Beat {
    Walk,
    Tell,
    Attack,
    Rest,
    Defeat,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Danger {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Danger {
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y > self.y && y <= self.y + self.h
    }
    fn valid(self) -> bool {
        (0..=240).contains(&self.x)
            && (0..=FLOOR).contains(&self.y)
            && (0..=240).contains(&self.w)
            && (0..=128).contains(&self.h)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plank {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub lift: bool,
}
/// Shared native-pixel support contact; boots retain the original four-pixel lip.
pub fn supports(x: i32, p: &Plank) -> bool {
    x >= p.x - 4 && x < p.x + p.w + 4
}
/// Shared descending crossing contact, in the same fixed-point units as the body.
pub fn landing(old_y: i32, y: i32, x: i32, ps: &[Plank]) -> Option<(usize, &Plank)> {
    ps.iter()
        .enumerate()
        .rev()
        .find(|(_, p)| old_y <= p.y * Q && y >= p.y * Q && supports(x, p))
}
/// Either axis follows its own seated body and clamps to its authored room extent.
pub fn camera(position: i32, lead: i32, extent: i32) -> i32 {
    (position - lead).clamp(0, extent)
}
/// A triangular integer track; every seat sees the same phase for a round.
pub fn planks(seed: u64, elapsed: u32) -> [Plank; 13] {
    std::array::from_fn(|i| {
        let phase = (elapsed / 2 + seed as u32 % 80 + i as u32 * 11) % 80;
        let shift = if phase < 40 {
            phase as i32 - 20
        } else {
            60 - phase as i32
        };
        let lift = i > 0 && i < 12 && i % 3 == 2;
        Plank {
            x: if i == 0 || i == 12 {
                8
            } else if lift {
                88 + shift
            } else if (i as u64 + seed) % 2 == 0 {
                8
            } else {
                96
            },
            y: FLOOR - i as i32 * 36,
            w: if i == 0 || i == 12 {
                232
            } else if lift {
                72
            } else {
                140
            },
            lift,
        }
    })
}
pub fn ladder(deck: usize) -> Option<i32> {
    (deck > 0 && deck < 12 && deck % 3 == 1).then_some(120)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keeper {
    pub x: i32,
    pub hp: u8,
    pub beat: Beat,
    pub ticks: u32,
    pub sweep: bool,
    pub attack: u32,
    pub danger: Danger,
    pub flinch: u32,
}
impl Keeper {
    pub fn new() -> Self {
        Self {
            x: 184,
            hp: 6,
            beat: Beat::Walk,
            ticks: 45,
            sweep: false,
            attack: 0,
            danger: Danger {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            flinch: 0,
        }
    }
    pub fn stage(&self) -> usize {
        if self.hp <= 2 {
            2
        } else if self.hp <= 4 {
            1
        } else {
            0
        }
    }
    pub(super) fn step(&mut self, x: i32) -> bool {
        self.flinch = self.flinch.saturating_sub(1);
        if self.hp == 0 {
            return false;
        }
        self.ticks = self.ticks.saturating_sub(1);
        match self.beat {
            Beat::Walk => {
                if self.ticks == 0 {
                    self.sweep = self.attack % 2 == 1;
                    self.attack += 1;
                    self.danger = if self.sweep {
                        Danger {
                            x: 16,
                            y: TOP - 19,
                            w: 216,
                            h: 19,
                        }
                    } else {
                        Danger {
                            x: (x - 24).clamp(8, 192),
                            y: TOP - 100,
                            w: 48,
                            h: 100,
                        }
                    };
                    self.beat = Beat::Tell;
                    self.ticks = 33 - self.stage() as u32 * 4;
                }
            }
            Beat::Tell => {
                if self.ticks == 0 {
                    self.beat = Beat::Attack;
                    self.ticks = 10;
                    return true;
                }
            }
            Beat::Attack => {
                if self.ticks == 0 {
                    self.beat = Beat::Rest;
                    self.ticks = 48 - self.stage() as u32 * 8;
                }
            }
            Beat::Rest => {
                if self.ticks == 0 {
                    self.beat = Beat::Walk;
                    self.ticks = 30;
                }
            }
            Beat::Defeat => {}
        }
        false
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sample {
    tick: u64,
    ground: bool,
    coyote: u32,
    ladder: Option<usize>,
    y: i32,
    dead: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lane {
    pub seed: u64,
    pub x: i32,
    pub y: i32,
    pub vy: i32,
    pub dir: i32,
    pub camera: i32,
    pub ground: Option<usize>,
    pub coyote: u32,
    pub buffer: u32,
    pub climbing: bool,
    pub checkpoint: usize,
    pub height: u32,
    pub falls: u32,
    pub score: u32,
    pub chain: u32,
    pub pose: u8,
    pub pose_ticks: u32,
    pub freeze: u32,
    pub down: u32,
    pub hurt: u32,
    pub effect: u32,
    pub effect_x: i32,
    pub effect_y: i32,
    pub finish: Option<u32>,
    pub fire_key: bool,
    pub keeper: Keeper,
    pub brick: Danger,
    pub brick_tell: u32,
    pub brick_burst: u32,
    pub last_jump: Option<u64>,
    pub cooldown: u32,
    #[serde(skip)]
    history: VecDeque<Sample>,
}
impl Lane {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            x: 42 * Q,
            y: FLOOR * Q,
            vy: 0,
            dir: 1,
            camera: FLOOR - 132,
            ground: Some(0),
            coyote: COYOTE,
            buffer: 0,
            climbing: false,
            checkpoint: 0,
            height: 0,
            falls: 0,
            score: 0,
            chain: 0,
            pose: 0,
            pose_ticks: 0,
            freeze: 0,
            down: 0,
            hurt: 0,
            effect: 0,
            effect_x: 0,
            effect_y: 0,
            finish: None,
            fire_key: false,
            keeper: Keeper::new(),
            brick: Danger {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            brick_tell: 0,
            brick_burst: 0,
            last_jump: None,
            cooldown: 0,
            history: VecDeque::new(),
        }
    }
    pub fn valid(&self) -> bool {
        (16 * Q..=224 * Q).contains(&self.x)
            && (-64 * Q..=(FLOOR + 120) * Q).contains(&self.y)
            && (-2200..=4096).contains(&self.vy)
            && [-1, 1].contains(&self.dir)
            && (0..=FLOOR - 132).contains(&self.camera)
            && self.ground.is_none_or(|i| i <= 12)
            && self.checkpoint <= 12
            && self.height <= 432
            && self.coyote <= 8
            && self.buffer <= 10
            && self.pose < 10
            && self.pose_ticks <= 12
            && self.freeze <= 2
            && self.down <= 18
            && self.hurt <= 30
            && self.effect <= 12
            && self.cooldown <= 24
            && self.falls <= LENGTH
            && self.chain <= LENGTH
            && self.score <= 1000
            && self.finish.is_none_or(|t| t <= LENGTH)
            && self.brick.valid()
            && self.brick_tell <= 30
            && self.brick_burst <= 12
            && (152..=208).contains(&self.keeper.x)
            && self.keeper.hp <= 6
            && self.keeper.ticks <= 48
            && self.keeper.flinch <= 12
            && self.keeper.attack <= LENGTH
            && self.keeper.danger.valid()
            && ((self.keeper.hp == 0) == (self.keeper.beat == Beat::Defeat))
    }
    fn near_ladder(&self) -> Option<usize> {
        (1..12).find(|&i| {
            ladder(i).is_some_and(|x| {
                (self.x / Q - x).abs() <= 14
                    && self.y / Q >= FLOOR - i as i32 * 36 - 4
                    && self.y / Q <= FLOOR - (i as i32 - 1) * 36 + 4
            })
        })
    }
    fn sample(&self, tick: u64) -> Sample {
        Sample {
            tick,
            ground: self.ground.is_some(),
            coyote: self.coyote,
            ladder: self.near_ladder(),
            y: self.y,
            dead: self.down > 0,
        }
    }
    fn burst(&mut self, x: i32, y: i32) {
        self.effect = 12;
        self.effect_x = x;
        self.effect_y = y;
        self.freeze = 2;
    }
    fn fail(&mut self) {
        if self.hurt > 0 || self.down > 0 {
            return;
        }
        self.falls += 1;
        self.chain = 0;
        self.down = 18;
        self.pose = 8;
        self.burst(self.x / Q, self.y / Q);
        self.climbing = false;
    }
    pub fn step_at(
        &mut self,
        input: Input,
        t: &super::tune::Tune,
        now: u64,
        allowance: u64,
        elapsed: u32,
    ) {
        let input = if input.valid() {
            input
        } else {
            Input::default()
        };
        self.history.push_back(self.sample(now));
        while self
            .history
            .front()
            .is_some_and(|s| now.saturating_sub(s.tick) > allowance)
        {
            self.history.pop_front();
        }
        let sampled = match input.latch_tick {
            Some(tick) if tick <= now && now - tick <= allowance => {
                self.history.iter().rev().find(|s| s.tick == tick).cloned()
            }
            Some(_) => None,
            None => Some(self.sample(now)),
        };
        let press = input.fire && !self.fire_key;
        if !input.fire && self.fire_key && self.vy < 0 {
            self.vy /= 2;
        }
        self.fire_key = input.fire;
        if self.finish.is_some() {
            self.pose = 9;
            return;
        }
        self.effect = self.effect.saturating_sub(1);
        self.hurt = self.hurt.saturating_sub(1);
        self.cooldown = self.cooldown.saturating_sub(1);
        if press
            && sampled.as_ref().is_some_and(|s| !s.dead)
            && self
                .last_jump
                .is_none_or(|v| sampled.as_ref().unwrap().tick > v)
        {
            let s = sampled.as_ref().unwrap();
            self.buffer = t.rafter_buffer;
            self.last_jump = Some(s.tick);
            if s.ground || s.coyote > 0 || s.ladder.is_some() {
                self.coyote = t.rafter_coyote.max(1);
            }
        }
        if self.freeze > 0 {
            self.freeze -= 1;
            return;
        }
        let ps = planks(self.seed, elapsed);
        if self.down > 0 {
            self.down -= 1;
            if self.down == 0 {
                let p = ps[self.checkpoint];
                self.x = (p.x + p.w / 2) * Q;
                self.y = p.y * Q;
                self.vy = 0;
                self.ground = Some(self.checkpoint);
                self.hurt = 30;
                self.pose = 5;
                self.pose_ticks = 6;
            }
            return;
        }
        if input.move_y != 0
            && let Some(s) = sampled.as_ref().filter(|s| s.ladder.is_some() && !s.dead)
        {
            if !self.climbing {
                // Catch the rung the knight saw, even if transit carried the body past it.
                self.x = 120 * Q;
                self.y = s.y;
            }
            self.climbing = true;
            self.ground = None;
        }
        if self.buffer > 0 && (self.ground.is_some() || self.coyote > 0 || self.climbing) {
            self.vy = -(t.rafter_jump as i32);
            self.ground = None;
            self.coyote = 0;
            self.climbing = false;
            self.buffer = 0;
            self.pose = 3;
        }
        self.buffer = self.buffer.saturating_sub(1);
        let old_y = self.y;
        if let Some(i) = self.ground {
            let prev = planks(self.seed, elapsed.saturating_sub(1))[i];
            self.x += (ps[i].x - prev.x) * Q;
        }
        self.x = (self.x + i32::from(input.move_x) * t.rafter_run as i32).clamp(16 * Q, 224 * Q);
        if input.move_x != 0 {
            self.dir = i32::from(input.move_x);
        }
        if self.climbing {
            if let Some(i) = self.near_ladder() {
                self.x = 120 * Q;
                self.vy = 0;
                self.y += i32::from(input.move_y) * LADDER;
                if self.y <= ps[i].y * Q {
                    self.y = ps[i].y * Q;
                    self.ground = Some(i);
                    self.checkpoint = self.checkpoint.max(i);
                    self.climbing = false;
                }
            } else {
                self.climbing = false;
            }
        } else {
            if let Some(i) = self.ground {
                if !supports(self.x / Q, &ps[i]) {
                    self.ground = None;
                    self.coyote = t.rafter_coyote;
                }
            }
            if self.ground.is_none() {
                self.vy = (self.vy + t.rafter_gravity as i32).min(4096);
                self.y += self.vy;
            }
            if self.vy >= 0 {
                if let Some((i, p)) = landing(old_y, self.y, self.x / Q, &ps) {
                    let landed = self.ground.is_none();
                    self.y = p.y * Q;
                    self.vy = 0;
                    self.ground = Some(i);
                    self.coyote = t.rafter_coyote;
                    if landed {
                        self.pose = 5;
                        self.pose_ticks = 4;
                        self.chain += 1;
                        self.burst(self.x / Q, p.y);
                    }
                    if i > self.checkpoint {
                        self.checkpoint = i;
                    }
                }
            }
        }
        if self.ground.is_none() {
            self.coyote = self.coyote.saturating_sub(1);
        }
        self.height = self.height.max((FLOOR - self.y / Q).clamp(0, 432) as u32);
        self.score = self.height + (6 - u32::from(self.keeper.hp)) * 70;
        self.camera = camera(self.y / Q, 110, FLOOR - 132);
        if self.y > (FLOOR + 60) * Q || self.y / Q > ps[self.checkpoint].y + 110 {
            self.fail();
        }
        if self.height >= 370 {
            if self.keeper.step(self.x / Q) && self.keeper.danger.contains(self.x / Q, self.y / Q) {
                self.fail();
            }
            if self.cooldown == 0
                && self.keeper.hp > 0
                && self.vy < 0
                && self.y / Q <= TOP - 22
                && self.y / Q >= TOP - 60
                && (self.x / Q - self.keeper.x).abs() < 40
                && self.keeper.beat != Beat::Tell
            {
                self.keeper.hp -= 1;
                self.keeper.flinch = 12;
                self.cooldown = 24;
                self.vy = 640;
                self.burst(self.keeper.x, TOP - 36);
                if self.keeper.hp == 0 {
                    self.keeper.beat = Beat::Defeat;
                    self.finish = Some(elapsed);
                    self.score = 1000;
                    self.pose = 9;
                }
            }
        }
        if self.brick_tell > 0 {
            self.brick_tell -= 1;
            if self.brick_tell == 0 {
                self.brick_burst = 12;
                if self.brick.contains(self.x / Q, self.y / Q) {
                    self.fail();
                }
            }
        } else {
            self.brick_burst = self.brick_burst.saturating_sub(1);
            if elapsed % 150 == 90 && self.height > 72 && self.height < 370 {
                self.brick = Danger {
                    x: (self.x / Q - 14).clamp(8, 204),
                    y: ps[self.checkpoint].y - 70,
                    w: 28,
                    h: 70,
                };
                self.brick_tell = 30;
            }
        }
        self.pose_ticks = self.pose_ticks.saturating_sub(1);
        if self.down == 0 && self.finish.is_none() && self.pose_ticks == 0 {
            self.pose = if self.climbing {
                6 + (elapsed / 6 % 2) as u8
            } else if self.ground.is_none() {
                if self.vy < 0 { 3 } else { 4 }
            } else if input.move_x != 0 {
                1 + (elapsed / 5 % 2) as u8
            } else {
                0
            };
        }
    }
    /// Uses visible planks and keeper tells, no knowledge of future random events.
    pub fn bot_input(&self, elapsed: u32) -> Input {
        let ps = planks(self.seed, elapsed);
        let current = self.checkpoint;
        let mut tx = if current < 12 {
            let p = ps[current + 1];
            (p.x + p.w / 2).clamp(ps[current].x + 8, ps[current].x + ps[current].w - 8)
        } else {
            self.keeper.x - 12
        };
        if current < 12 && ladder(current + 1).is_some() {
            tx = 120;
        }
        let close = (tx * Q - self.x).abs() < 14 * Q;
        Input {
            move_x: if close {
                0
            } else {
                (tx * Q - self.x).signum() as i8
            },
            move_y: if current < 12
                && ladder(current + 1).is_some()
                && (self.x / Q - 120).abs() < 14
            {
                -1
            } else {
                0
            },
            // A complete held arc survives stale grounded snapshots on the link.
            // The release belongs to the round clock, not the echoed button state.
            fire: elapsed % 45 < 24
                && ((self.ground.is_some() && (current == 12 || close))
                    || (self.fire_key && self.vy < 0)),
            ..Default::default()
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub lanes: BTreeMap<u32, Lane>,
    pub seed: u64,
    pub left: u32,
    pub order: Vec<u32>,
    pub key: bool,
    pub won: bool,
}
impl Session {
    pub fn new(seats: impl Iterator<Item = u32>, seed: u64) -> Self {
        Self {
            lanes: seats.map(|id| (id, Lane::new(seed))).collect(),
            seed,
            left: LENGTH,
            order: Vec::new(),
            key: true,
            won: false,
        }
    }
    pub fn score(&self, id: u32) -> (std::cmp::Reverse<u32>, u32, u32) {
        let d = &self.lanes[&id];
        (
            std::cmp::Reverse(d.score),
            d.falls,
            d.finish.unwrap_or(LENGTH + 1),
        )
    }
    pub fn place(&self, id: u32) -> usize {
        1 + self
            .lanes
            .keys()
            .filter(|&&r| self.score(r) < self.score(id))
            .count()
    }
    pub fn payout(&self, id: u32) -> u32 {
        let Some(&first) = self.order.first() else {
            return 0;
        };
        if self.left > 0 || self.score(id) != self.score(first) {
            return 0;
        }
        let tied = self
            .lanes
            .keys()
            .filter(|&&r| self.score(r) == self.score(first))
            .count() as u32;
        let before = self
            .lanes
            .range(..id)
            .filter(|&(r, _)| self.score(*r) == self.score(first))
            .count() as u32;
        let total = GOLD_PER_SEAT * self.lanes.len() as u32;
        total / tied + u32::from(before < total % tied)
    }
    pub fn valid(&self) -> bool {
        !self.lanes.is_empty()
            && self.lanes.len() <= MAX_PLAYERS as usize
            && self
                .lanes
                .iter()
                .all(|(&id, d)| (1..=MAX_PLAYERS).contains(&id) && d.valid() && d.seed == self.seed)
            && self.left <= LENGTH
            && self.order.len() <= self.lanes.len()
            && self
                .order
                .iter()
                .enumerate()
                .all(|(n, id)| self.lanes.contains_key(id) && !self.order[..n].contains(id))
            && (self.left > 0 || self.order.len() == self.lanes.len())
    }
    pub fn repair(&mut self) {
        self.lanes.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        for d in self.lanes.values_mut() {
            if !d.valid() || d.seed != self.seed {
                *d = Lane::new(self.seed);
            }
        }
        if self.lanes.is_empty() {
            self.lanes.insert(1, Lane::new(self.seed));
        }
        self.left = LENGTH;
        self.order.clear();
        self.key = true;
        self.won = false;
    }
}
impl Run {
    pub fn rafterwake_showing(&self) -> bool {
        self.room().kind == RoomKind::Workshop && self.rafterwake.is_some()
    }
    pub(super) fn tick_rafterwake(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Workshop {
            self.rafterwake = None;
            return;
        }
        if self.rafterwake.is_none() {
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
            if pressed {
                self.start_rafterwake();
            }
            return;
        }
        let mut s = self.rafterwake.take().unwrap();
        s.lanes
            .retain(|id, _| self.players.contains_key(id) && !self.exits.outside.contains_key(id));
        s.order.retain(|id| s.lanes.contains_key(id));
        if s.left > 0 {
            for (&id, hero) in &self.players {
                if hero.hp > 0 && !hero.stone && !hero.away && !self.exits.outside.contains_key(&id)
                {
                    s.lanes.entry(id).or_insert_with(|| Lane::new(s.seed));
                }
            }
        }
        if s.lanes.is_empty() {
            return;
        }
        if s.left == 0 {
            let take = inputs
                .iter()
                .any(|(id, i)| s.lanes.contains_key(id) && i.valid() && i.interact);
            if take && !s.key {
                self.start_rafterwake();
            } else {
                s.key = take;
                self.rafterwake = Some(s);
            }
            return;
        }
        s.left -= 1;
        let elapsed = LENGTH - s.left;
        for (&id, d) in &mut s.lanes {
            if self.players.get(&id).is_some_and(|h| !h.away && !h.stone) {
                let chain = d.chain;
                let falls = d.falls;
                d.step_at(
                    inputs.get(&id).copied().unwrap_or_default(),
                    &self.tune,
                    self.tick,
                    self.latch_input_age.get(&id).copied().unwrap_or(0),
                    elapsed,
                );
                if d.chain > chain {
                    self.sounds.push("clang");
                }
                if d.falls > falls {
                    self.sounds.push("hit");
                }
            }
        }
        if s.left == 0 || s.lanes.values().all(|d| d.finish.is_some()) {
            s.left = 0;
            s.order = s.lanes.keys().copied().collect();
            let mut order = std::mem::take(&mut s.order);
            order.sort_by_key(|id| (s.score(*id), *id));
            s.order = order;
            s.key = true;
            s.won = s.lanes.values().any(|d| d.finish.is_some());
            let record = &mut self.home.countryside.games.rafterwake;
            record.best = record.best.max(
                s.lanes
                    .values()
                    .map(|d| (d.score / WORK_PER_RECORD).min(1000) as u16)
                    .max()
                    .unwrap_or(0),
            );
            if s.won {
                record.wins = record.wins.saturating_add(1).min(record.plays);
            }
            for (&id, h) in &mut self.players {
                if s.lanes.contains_key(&id) {
                    h.laurel = s.place(id) == 1;
                    h.carried.add(Spoil::Gold, s.payout(id));
                    if h.laurel {
                        h.booby = None;
                    }
                }
            }
            self.sounds.push("spoil_pickup");
        }
        self.rafterwake = Some(s);
    }
    fn start_rafterwake(&mut self) {
        let seed = self.rng.next();
        let seats = self
            .players
            .iter()
            .filter(|(id, h)| {
                h.hp > 0 && !h.stone && !h.away && !self.exits.outside.contains_key(id)
            })
            .map(|(&id, _)| id);
        let s = Session::new(seats, seed);
        if s.lanes.is_empty() {
            return;
        }
        self.rafterwake = Some(s);
        self.home.countryside.games.rafterwake.plays = self
            .home
            .countryside
            .games
            .rafterwake
            .plays
            .saturating_add(1);
        self.sounds.push("play_card");
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn rafterwake_for_test() -> Run {
        let mut home = super::home::Home::default();
        home.levels.insert(super::home::Station::Wing, 1);
        let mut r = Run::at_home(41, 1, Some("Gawain"), home, Default::default());
        let at = r
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Workshop)
            .unwrap();
        r.arrive(at, (BAY.0 / 2.0, BAY.1 / 2.0));
        r
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    pub fn rafterwake_review_for_test(moment: &str) -> Run {
        let mut r = Self::rafterwake_for_test();
        for (id, n) in [(2, "LYRA"), (3, "BRAN"), (4, "ELIN")] {
            r.join_seat(id, n);
        }
        if moment == "entrance" {
            return r;
        }
        r.start_rafterwake();
        let s = r.rafterwake.as_mut().unwrap();
        s.left = LENGTH - 900;
        for (&id, d) in &mut s.lanes {
            d.height = id * 72;
            d.score = d.height;
        }
        let d = s.lanes.get_mut(&1).unwrap();
        d.x = 140 * Q;
        d.y = TOP * Q;
        d.ground = Some(12);
        d.checkpoint = 12;
        d.height = 432;
        d.score = 432;
        d.camera = 0;
        if let Some(cam) = moment
            .strip_prefix("tower-")
            .and_then(|n| n.parse::<i32>().ok())
        {
            d.camera = cam;
            d.y = (cam + 110) * Q;
            d.pose = 4;
            d.ground = None;
        }
        match moment {
            "bottom" => {
                *d = Lane::new(s.seed);
            }
            "middle" => {
                d.checkpoint = 6;
                d.x = 130 * Q;
                d.y = 324 * Q;
                d.camera = 214;
                d.ground = Some(6);
                d.pose = 1;
            }
            "climb" => {
                d.x = 120 * Q;
                d.y = 418 * Q;
                d.camera = 308;
                d.climbing = true;
                d.ground = None;
                d.pose = 6;
            }
            "jump" => {
                d.y -= 30 * Q;
                d.vy = -700;
                d.ground = None;
                d.pose = 3;
            }
            "fall" => {
                d.y -= 20 * Q;
                d.vy = 700;
                d.ground = None;
                d.pose = 4;
            }
            "land" => {
                d.pose = 5;
                d.effect = 8;
                d.effect_x = d.x / Q;
                d.effect_y = d.y / Q;
            }
            "hit" => {
                d.pose = 8;
                d.down = 18;
            }
            "drop-tell" | "drop" => {
                d.keeper.beat = if moment == "drop" {
                    Beat::Attack
                } else {
                    Beat::Tell
                };
                d.keeper.ticks = if moment == "drop" { 7 } else { 20 };
                d.keeper.danger = Danger {
                    x: 76,
                    y: 8,
                    w: 48,
                    h: 100,
                };
            }
            "sweep-tell" | "sweep" => {
                d.keeper.sweep = true;
                d.keeper.beat = if moment == "sweep" {
                    Beat::Attack
                } else {
                    Beat::Tell
                };
                d.keeper.ticks = if moment == "sweep" { 7 } else { 20 };
                d.keeper.danger = Danger {
                    x: 16,
                    y: 89,
                    w: 216,
                    h: 19,
                };
            }
            "flinch" => {
                d.keeper.flinch = 12;
                d.effect = 10;
                d.effect_x = 184;
                d.effect_y = 72;
            }
            "damage-1" => {
                d.keeper.hp = 4;
                d.keeper.beat = Beat::Rest;
                d.keeper.ticks = 20;
            }
            "damage-2" => {
                d.keeper.hp = 2;
                d.keeper.beat = Beat::Rest;
                d.keeper.ticks = 20;
            }
            "defeat" | "podium" => {
                d.keeper.hp = 0;
                d.keeper.beat = Beat::Defeat;
                d.finish = Some(900);
                d.score = 1000;
                d.pose = 9;
            }
            "recover" => {
                d.keeper.beat = Beat::Rest;
                d.keeper.ticks = 8;
            }
            _ => {}
        }
        if moment == "podium" {
            s.left = 0;
            s.order = vec![1, 4, 3, 2];
            s.won = true;
        }
        r
    }
}
