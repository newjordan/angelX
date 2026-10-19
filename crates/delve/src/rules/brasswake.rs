//! Brasswake: a brewery aisle moves under a planted punch and a heaved cask.
//! First ten seconds: close on a porter, land a three-blow kick, mark a rival aisle.
//! Five live integers: brass_stride=3 px/tick, brass_combo=12 ticks,
//! brass_tell=27 ticks, brass_shove=12 px, brass_stop=2 ticks.
//! One 75-second stage, three waves and one vat keeper. No campaign or bullets.
//! Full feel brief: assets/brasswake/BRIEF.md. Each seat owns its lane and camera.
use super::{BTreeMap, HZ, Input, MAX_PLAYERS, RoomKind, Run, Spoil};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Larger closes the aisle faster, in native pixels per tick.
pub const STRIDE: u32 = 3;
/// Larger leaves more breathing space between fresh blows.
pub const COMBO: u32 = 12;
/// Baseline wind-up; later phases keep at least 23 ticks.
pub const TELL: u32 = 27;
/// Larger makes contact move a body farther along the aisle.
pub const SHOVE: u32 = 12;
/// Local contact freeze; the other lanes keep playing.
pub const STOP: u32 = 2;
pub const LENGTH: u32 = 75 * HZ;
pub const GOLD_PER_SEAT: u32 = 10;
/// The shared best ledger stores five work points per unit, without losing any award.
pub const WORK_PER_RECORD: u32 = 5;
pub const BAY: (f32, f32) = (42.0, 10.0);
pub const END: i32 = 960;

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
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
    fn valid(self) -> bool {
        (-128..=END + 128).contains(&self.x)
            && (100..=180).contains(&self.y)
            && (0..=128).contains(&self.w)
            && (0..=32).contains(&self.h)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub x: i32,
    pub y: i32,
    pub hp: u8,
    pub boss: bool,
    pub beat: Beat,
    pub ticks: u32,
    pub attack: u32,
    pub rush: bool,
    pub dir: i32,
    pub danger: Danger,
    pub flinch: u32,
}
impl Foe {
    pub fn new(x: i32, y: i32, boss: bool) -> Self {
        Self {
            x,
            y,
            hp: if boss { 32 } else { 10 },
            boss,
            beat: Beat::Walk,
            ticks: 0,
            attack: 0,
            rush: false,
            dir: -1,
            danger: Danger {
                x,
                y: y - 10,
                w: 0,
                h: 20,
            },
            flinch: 0,
        }
    }
    pub fn stage(&self) -> usize {
        let max = if self.boss { 32 } else { 10 };
        if self.hp <= max / 3 {
            2
        } else if self.hp <= max * 2 / 3 {
            1
        } else {
            0
        }
    }
    fn valid(&self) -> bool {
        (-64..=END + 96).contains(&self.x)
            && (126..=174).contains(&self.y)
            && self.hp <= if self.boss { 32 } else { 10 }
            && [-1, 1].contains(&self.dir)
            && self.ticks <= 90
            && self.flinch <= 12
            && self.attack <= LENGTH
            && self.danger.valid()
            && ((self.hp == 0) == (self.beat == Beat::Defeat))
            && (!matches!(self.beat, Beat::Tell | Beat::Attack | Beat::Rest) || self.ticks > 0)
    }
    fn step(&mut self, x: i32, y: i32, seed: u64, tune: &super::tune::Tune) -> bool {
        self.flinch = self.flinch.saturating_sub(1);
        if self.hp == 0 {
            return false;
        }
        match self.beat {
            Beat::Walk => {
                self.dir = if x < self.x { -1 } else { 1 };
                if (x - self.x).abs() > 44 {
                    self.x += self.dir;
                }
                if (y - self.y).abs() > 3 {
                    self.y += (y - self.y).signum();
                }
                if (x - self.x).abs() <= 50 && (y - self.y).abs() <= 8 {
                    self.attack += 1;
                    self.rush = self.attack.wrapping_add(seed as u32) % 2 == 0;
                    self.beat = Beat::Tell;
                    self.ticks = tune
                        .brass_tell
                        .saturating_sub(self.stage() as u32 * 3)
                        .max(23);
                    self.danger = if self.rush {
                        Danger {
                            x: self.x.min(self.x + self.dir * 96),
                            y: self.y - 10,
                            w: 96,
                            h: 20,
                        }
                    } else {
                        Danger {
                            x: self.x + self.dir * 28 - 24,
                            y: self.y - 14,
                            w: 48,
                            h: 28,
                        }
                    };
                }
            }
            Beat::Tell => {
                self.ticks -= 1;
                if self.ticks == 0 {
                    self.beat = Beat::Attack;
                    self.ticks = 9;
                    return true;
                }
            }
            Beat::Attack => {
                self.ticks -= 1;
                if self.rush {
                    self.x = (self.x + self.dir * 8).clamp(0, END + 64);
                }
                if self.ticks == 0 {
                    self.beat = Beat::Rest;
                    self.ticks = 42 - self.stage() as u32 * 7;
                }
            }
            Beat::Rest => {
                self.ticks -= 1;
                if self.ticks == 0 {
                    self.beat = Beat::Walk;
                }
            }
            Beat::Defeat => {}
        }
        false
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cask {
    pub danger: Danger,
    pub tell: u32,
    pub burst: u32,
    pub from: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sample {
    tick: u64,
    wave: u8,
    holding: bool,
    x: i32,
    y: i32,
    dir: i32,
    down: bool,
    foes: Vec<(i32, i32, u8)>,
    cask: Option<i32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lane {
    pub seed: u64,
    pub x: i32,
    pub y: i32,
    pub dir: i32,
    pub camera: i32,
    pub wave: u8,
    pub foes: Vec<Foe>,
    pub hp: u8,
    pub falls: u32,
    pub score: u32,
    pub chain: u32,
    pub combo: u8,
    pub cooldown: u32,
    pub pose: u8,
    pub pose_ticks: u32,
    pub freeze: u32,
    pub hurt: u32,
    pub down: u32,
    pub guard: bool,
    pub holding: bool,
    pub cask: Option<i32>,
    pub bread: Option<i32>,
    pub incoming: Vec<Cask>,
    pub effect: u32,
    pub effect_x: i32,
    pub effect_y: i32,
    pub finish: Option<u32>,
    pub fire_key: bool,
    pub key: bool,
    pub last_strike: Option<u64>,
    #[serde(skip)]
    history: VecDeque<Sample>,
}
impl Lane {
    pub fn new(seed: u64) -> Self {
        let mut s = Self {
            seed,
            x: 40,
            y: 150,
            dir: 1,
            camera: 0,
            wave: 0,
            foes: Vec::new(),
            hp: 6,
            falls: 0,
            score: 0,
            chain: 0,
            combo: 0,
            cooldown: 0,
            pose: 0,
            pose_ticks: 0,
            freeze: 0,
            hurt: 0,
            down: 0,
            guard: false,
            holding: false,
            cask: Some(82),
            bread: None,
            incoming: Vec::new(),
            effect: 0,
            effect_x: 0,
            effect_y: 0,
            finish: None,
            fire_key: false,
            key: true,
            last_strike: None,
            history: VecDeque::new(),
        };
        s.spawn();
        s
    }
    fn spawn(&mut self) {
        let gate = (i32::from(self.wave) + 1) * 240;
        self.foes = if self.wave == 3 {
            vec![Foe::new(gate - 40, 150, true)]
        } else {
            vec![
                Foe::new(
                    gate - 64,
                    136 + ((self.seed >> self.wave) & 1) as i32 * 24,
                    false,
                ),
                Foe::new(
                    gate - 12,
                    160 - ((self.seed >> self.wave) & 1) as i32 * 24,
                    false,
                ),
            ]
        };
    }
    pub fn valid(&self) -> bool {
        (24..=END - 24).contains(&self.x)
            && (126..=174).contains(&self.y)
            && [-1, 1].contains(&self.dir)
            && (0..=END - 240).contains(&self.camera)
            && self.wave <= 3
            && self.foes.len() == if self.wave == 3 { 1 } else { 2 }
            && self.foes.iter().all(Foe::valid)
            && self.hp <= 6
            && self.falls <= LENGTH
            && self.score <= LENGTH * 300
            && self.chain <= LENGTH
            && self.combo <= 2
            && self.cooldown <= 30
            && self.pose < 12
            && self.pose_ticks <= 18
            && self.freeze <= 8
            && self.hurt <= 24
            && self.down <= 18
            && self.cask.is_none_or(|x| (0..=END).contains(&x))
            && self.bread.is_none_or(|x| (0..=END).contains(&x))
            && self.incoming.iter().all(|c| {
                c.danger.valid()
                    && c.tell <= 60
                    && c.burst <= 12
                    && (1..=MAX_PLAYERS).contains(&c.from)
            })
            && self.effect <= 12
            && self.finish.is_none_or(|t| t <= LENGTH)
    }
    fn sample(&self, tick: u64) -> Sample {
        Sample {
            tick,
            wave: self.wave,
            holding: self.holding,
            x: self.x,
            y: self.y,
            dir: self.dir,
            down: self.down > 0,
            foes: self.foes.iter().map(|e| (e.x, e.y, e.hp)).collect(),
            cask: self.cask,
        }
    }
    fn contact(&mut self, x: i32, y: i32, tune: &super::tune::Tune) {
        self.effect = 12;
        self.effect_x = x;
        self.effect_y = y - 20;
        self.freeze = tune.brass_stop;
    }
    fn hit(&mut self, x: i32, y: i32, tune: &super::tune::Tune) {
        if self.hurt > 0 || self.down > 0 || self.finish.is_some() {
            return;
        }
        self.contact(x, y, tune);
        if self.guard {
            self.pose = 10;
            self.pose_ticks = 10;
            return;
        }
        self.hp -= 1;
        self.chain = 0;
        self.combo = 0;
        self.hurt = 24;
        self.pose = 8;
        self.pose_ticks = 12;
        self.x = (self.x - (x - self.x).signum() * tune.brass_shove as i32).clamp(24, END - 24);
        if self.hp == 0 {
            self.falls += 1;
            self.down = 18;
            self.pose = 9;
            self.pose_ticks = 18;
            self.holding = false;
        }
    }
    /// Fresh attacks are judged against the frame sampled by the client. History
    /// lives only as long as the existing measured network allowance, never on wire.
    pub fn step_at(
        &mut self,
        input: Input,
        tune: &super::tune::Tune,
        now: u64,
        allowance: u64,
        elapsed: u32,
    ) -> bool {
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
        let fire = input.fire && !self.fire_key;
        let use_cask = input.interact && !self.key;
        self.fire_key = input.fire;
        self.key = input.interact;
        let sampled = match input.latch_tick {
            Some(tick) if tick <= now && now - tick <= allowance => {
                self.history.iter().rev().find(|s| s.tick == tick).cloned()
            }
            Some(_) => None,
            None => Some(self.sample(now)),
        };
        let mut thrown = false;
        if self.finish.is_none() {
            if let Some(s) = sampled.filter(|s| !s.down && s.wave == self.wave) {
                if use_cask {
                    if self.holding && s.holding {
                        self.holding = false;
                        self.pose = 7;
                        self.pose_ticks = 12;
                        thrown = true;
                    } else if self.cask == s.cask
                        && s.cask.is_some_and(|x| (x - s.x).abs() <= 28)
                        && (s.y - 150).abs() <= 20
                    {
                        self.holding = true;
                        self.cask = None;
                        self.pose = 6;
                        self.pose_ticks = 10;
                    }
                }
                if fire
                    && !self.holding
                    && self
                        .last_strike
                        .is_none_or(|tick| s.tick >= tick + u64::from(tune.brass_combo))
                {
                    self.last_strike = Some(s.tick);
                    self.cooldown = tune.brass_combo;
                    self.pose = 3 + self.combo;
                    self.pose_ticks = 10;
                    let reach = if self.combo == 2 { 48 } else { 38 };
                    if let Some(index) = s.foes.iter().position(|&(x, y, hp)| {
                        hp > 0
                            && (x - s.x).abs() <= reach
                            && (y - s.y).abs() <= 13
                            && (x - s.x) * s.dir >= -6
                    }) {
                        if let Some(e) = self.foes.get_mut(index).filter(|e| e.hp > 0) {
                            let damage = if self.combo == 2 { 3 } else { 2 };
                            e.hp = e.hp.saturating_sub(damage);
                            e.flinch = 10;
                            let margin = if e.boss { 32 } else { 24 };
                            e.x = (e.x + s.dir * tune.brass_shove as i32)
                                .clamp(margin, (i32::from(self.wave) + 1) * 240 - margin);
                            let (x, y) = (e.x, e.y);
                            self.score += 10;
                            self.chain += 1;
                            self.combo = (self.combo + 1) % 3;
                            if e.hp == 0 {
                                e.beat = Beat::Defeat;
                                self.score += if e.boss { 600 } else { 100 };
                            }
                            self.contact(x, y, tune);
                        }
                    } else {
                        self.combo = 0;
                    }
                }
            }
        }
        self.guard = input.interact && !self.holding && self.pose != 7;
        if self.freeze > 0 {
            self.freeze -= 1;
            return thrown;
        }
        self.effect = self.effect.saturating_sub(1);
        self.hurt = self.hurt.saturating_sub(1);
        self.cooldown = self.cooldown.saturating_sub(1);
        self.pose_ticks = self.pose_ticks.saturating_sub(1);
        if self.down > 0 {
            self.down -= 1;
            if self.down == 0 {
                self.hp = 6;
                self.pose = 11;
                self.pose_ticks = 12;
                self.hurt = 24;
            }
            return thrown;
        }
        if self.finish.is_some() {
            return thrown;
        }
        if self.pose_ticks == 0 {
            self.pose = if self.guard {
                10
            } else if self.holding {
                6
            } else if input.move_x != 0 || input.move_y != 0 {
                1 + (elapsed / 6 % 2) as u8
            } else {
                0
            };
        }
        if !self.guard && self.pose_ticks == 0 {
            self.x = (self.x + i32::from(input.move_x) * tune.brass_stride as i32)
                .clamp(24, ((i32::from(self.wave) + 1) * 240).min(END - 24));
            self.y = (self.y + i32::from(input.move_y) * tune.brass_stride as i32).clamp(126, 174);
            if input.move_x != 0 {
                self.dir = i32::from(input.move_x);
            }
        }
        self.camera = (self.x - 92).clamp(0, END - 240);
        if self
            .bread
            .is_some_and(|x| (self.x - x).abs() < 20 && (self.y - 150).abs() < 18)
        {
            self.bread = None;
            self.hp = (self.hp + 2).min(6);
            self.score += 25;
        }
        let mut contacts = Vec::new();
        for e in &mut self.foes {
            let landed = e.step(self.x, self.y, self.seed, tune);
            let margin = if e.boss { 32 } else { 24 };
            let edge = (i32::from(self.wave) + 1) * 240 - margin;
            e.x = e.x.clamp(margin, edge);
            let end = (e.danger.x + e.danger.w).clamp(24, edge);
            e.danger.x = e.danger.x.clamp(24, edge);
            e.danger.w = end.saturating_sub(e.danger.x);
            if landed && e.danger.contains(self.x, self.y) {
                contacts.push((e.x, e.y));
            }
        }
        for c in &mut self.incoming {
            if c.tell > 0 {
                c.tell -= 1;
                if c.tell == 0 {
                    c.burst = 12;
                    if c.danger.contains(self.x, self.y) {
                        contacts.push((c.danger.x + c.danger.w / 2, c.danger.y + c.danger.h / 2));
                    }
                }
            } else {
                c.burst = c.burst.saturating_sub(1);
            }
        }
        self.incoming.retain(|c| c.tell > 0 || c.burst > 0);
        for (x, y) in contacts {
            self.hit(x, y, tune);
        }
        if self.foes.iter().all(|e| e.hp == 0) && self.x >= ((i32::from(self.wave) + 1) * 240) - 40
        {
            if self.wave == 3 {
                self.finish = Some(elapsed);
                self.pose = 0;
            } else {
                self.wave += 1;
                self.bread = Some(self.x + 12);
                self.cask = Some(self.x + 34);
                self.spawn();
            }
        }
        thrown
    }
    /// Reads public positions and danger rectangles only; no hidden future state.
    pub fn bot_input(&self, elapsed: u32) -> Input {
        if self.finish.is_some() {
            return Input::default();
        }
        let target = self.foes.iter().find(|e| e.hp > 0);
        let (tx, ty) = target.map_or((((i32::from(self.wave) + 1) * 240), 150), |e| {
            (e.x - self.dir * 25, e.y)
        });
        let danger = target.filter(|e| e.beat == Beat::Tell).map(|e| e.danger);
        let evade = danger.is_some_and(|d| d.contains(self.x, self.y));
        Input {
            move_x: if (tx - self.x).abs() > 24 {
                (tx - self.x).signum() as i8
            } else {
                0
            },
            move_y: if evade {
                if self.y < 150 { -1 } else { 1 }
            } else if (ty - self.y).abs() > 6 {
                (ty - self.y).signum() as i8
            } else {
                0
            },
            fire: target.is_some_and(|e| (e.x - self.x).abs() <= 36 && (e.y - self.y).abs() <= 12)
                && !self.fire_key
                && self.cooldown == 0,
            interact: elapsed % 12 == 0
                && (self.holding || self.cask.is_some_and(|x| (x - self.x).abs() <= 28)),
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
    pub fn brasswake_showing(&self) -> bool {
        self.room().kind == RoomKind::Tavern && self.brasswake.is_some()
    }
    pub(super) fn tick_brasswake(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Tavern {
            self.brasswake = None;
            return;
        }
        if self.brasswake.is_none() {
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
                self.start_brasswake();
            }
            return;
        }
        let mut s = self.brasswake.take().unwrap();
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
                self.start_brasswake();
            } else {
                s.key = take;
                self.brasswake = Some(s);
            }
            return;
        }
        s.left -= 1;
        let elapsed = LENGTH - s.left;
        let mut thrown = Vec::new();
        for (&id, d) in &mut s.lanes {
            if self.players.get(&id).is_some_and(|h| !h.away && !h.stone) {
                let chain = d.chain;
                let falls = d.falls;
                if d.step_at(
                    inputs.get(&id).copied().unwrap_or_default(),
                    &self.tune,
                    self.tick,
                    self.latch_input_age.get(&id).copied().unwrap_or(0),
                    elapsed,
                ) {
                    thrown.push(id);
                }
                if d.chain > chain {
                    self.sounds.push("clang");
                }
                if d.falls > falls {
                    self.sounds.push("hit");
                }
            }
        }
        let ids = s.lanes.keys().copied().collect::<Vec<_>>();
        for from in thrown {
            let n = ids.iter().position(|id| *id == from).unwrap();
            let to = ids[(n + 1) % ids.len()];
            let d = s.lanes.get_mut(&to).unwrap();
            d.incoming.push(Cask {
                danger: Danger {
                    x: (d.x + 30).min(END - 40),
                    y: (d.y - 12).clamp(114, 150),
                    w: 40,
                    h: 24,
                },
                tell: self.tune.brass_tell,
                burst: 0,
                from,
            });
            self.sounds.push("play_card");
        }
        if s.left == 0 || s.lanes.values().all(|d| d.finish.is_some()) {
            s.left = 0;
            s.order = s.lanes.keys().copied().collect();
            let mut order = std::mem::take(&mut s.order);
            order.sort_by_key(|id| (s.score(*id), *id));
            s.order = order;
            s.key = true;
            s.won = s.lanes.values().any(|d| d.finish.is_some());
            let record = &mut self.home.countryside.games.brasswake;
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
        self.brasswake = Some(s);
    }
    fn start_brasswake(&mut self) {
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
        self.brasswake = Some(s);
        self.home.countryside.games.brasswake.plays = self
            .home
            .countryside
            .games
            .brasswake
            .plays
            .saturating_add(1);
        self.sounds.push("play_card");
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn brasswake_for_test() -> Run {
        let mut home = super::home::Home::default();
        home.levels.insert(super::home::Station::Wing, 1);
        let mut r = Run::at_home(41, 1, Some("Gawain"), home, Default::default());
        let at = r
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Tavern)
            .unwrap();
        r.arrive(at, (BAY.0 / 2.0, BAY.1 / 2.0));
        r
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// Shared staged scenes for the RGB and actual terminal review writers.
    pub fn brasswake_review_for_test(moment: &str) -> Run {
        let mut r = Self::brasswake_for_test();
        for (id, n) in [(2, "LYRA"), (3, "BRAN"), (4, "ELIN")] {
            r.join_seat(id, n);
        }
        r.players.get_mut(&1).unwrap().name = "YOU".into();
        if moment == "entrance" {
            return r;
        }
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        let s = r.brasswake.as_mut().unwrap();
        s.left = 39 * HZ;
        let d = s.lanes.get_mut(&1).unwrap();
        d.wave = 3;
        d.x = 808;
        d.y = 154;
        d.camera = 720;
        d.cask = Some(752);
        d.foes = vec![Foe::new(876, 154, true)];
        match moment {
            "overlap" => {
                d.wave = 0;
                d.foes = vec![Foe::new(d.x + 9, d.y + 3, false)];
            }
            "standings" => {
                for (&id, d) in &mut s.lanes {
                    d.score = 660;
                    d.falls = if id == 1 { 2 } else { 0 };
                    d.finish = if id == 3 { Some(54 * HZ) } else { None };
                }
            }
            "slam-tell" | "slam" => {
                let e = &mut d.foes[0];
                e.beat = if moment == "slam" {
                    Beat::Attack
                } else {
                    Beat::Tell
                };
                e.ticks = if moment == "slam" { 5 } else { 24 };
                e.danger = Danger {
                    x: 800,
                    y: 140,
                    w: 48,
                    h: 28,
                };
            }
            "rush-tell" | "rush" => {
                let e = &mut d.foes[0];
                e.rush = true;
                e.beat = if moment == "rush" {
                    Beat::Attack
                } else {
                    Beat::Tell
                };
                e.ticks = if moment == "rush" { 5 } else { 24 };
                e.danger = Danger {
                    x: 780,
                    y: 144,
                    w: 96,
                    h: 20,
                };
            }
            "flinch" => {
                d.foes[0].flinch = 10;
                d.effect = 9;
                d.effect_x = 847;
                d.effect_y = 132;
                d.pose = 4;
                d.pose_ticks = 10;
                d.chain = 2;
            }
            "damage-1" => {
                d.foes[0].hp = 20;
                d.foes[0].beat = Beat::Rest;
                d.foes[0].ticks = 20;
            }
            "damage-2" => {
                d.foes[0].hp = 8;
                d.foes[0].beat = Beat::Rest;
                d.foes[0].ticks = 20;
            }
            "defeat" => {
                d.foes[0].hp = 0;
                d.foes[0].beat = Beat::Defeat;
                d.chain = 5;
            }
            "throw" => {
                d.pose = 7;
                d.pose_ticks = 10;
                d.incoming.push(Cask {
                    danger: Danger {
                        x: 820,
                        y: 142,
                        w: 40,
                        h: 24,
                    },
                    tell: 20,
                    burst: 0,
                    from: 2,
                });
            }
            "down" => {
                d.down = 18;
                d.pose = 9;
                d.hp = 0;
                d.falls = 1;
            }
            "guard" => {
                d.pose = 10;
                d.guard = true;
            }
            "kick" => {
                d.pose = 5;
                d.pose_ticks = 10;
                d.chain = 3;
                d.foes[0].x = 848;
                d.effect = 6;
                d.effect_x = 841;
                d.effect_y = 134;
            }
            "wave-1" | "wave-2" | "wave-3" => {
                d.wave = moment.as_bytes()[5] - b'1';
                d.x = i32::from(d.wave) * 240 + 82;
                d.camera = i32::from(d.wave) * 240;
                d.cask = Some(d.x - 30);
                d.spawn();
            }
            "podium" => {
                s.left = 0;
                s.won = true;
                for (&id, d) in &mut s.lanes {
                    d.score = 1300 - id * 75;
                    d.falls = id - 1;
                    d.finish = Some((51 + id) * HZ);
                }
                s.order = vec![1, 2, 3, 4];
            }
            _ => {}
        }
        r
    }
}
