//! Pennon Rush: a late boot clears the breach while a hook-bow folds a pennant.
//! First ten seconds: run, jump late, release for a low arc, shoot the first sentry.
//! F2: gravity=112, jump=1536, run=640 fixed units; coyote=4, buffer=5 ticks.
//! One 75-second rampart, three waves and one captain. See assets/pennon/BRIEF.md.
pub use super::rafterwake::{Beat, Q};
use super::rafterwake::{Plank, camera, landing, supports};
use super::{BTreeMap, HZ, Input, MAX_PLAYERS, RoomKind, Run, Spoil};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
/// Weight after the apex, fixed units per tick squared.
pub const GRAVITY: u32 = 112;
/// Held jump impulse, fixed units per tick.
pub const JUMP: u32 = 1536;
/// Horizontal control, fixed units per tick.
pub const RUN: u32 = 640;
/// Late boot allowance in ticks.
pub const COYOTE: u32 = 4;
/// Early jump storage in ticks.
pub const BUFFER: u32 = 5;
pub const LENGTH: u32 = 75 * HZ;
pub const GOLD_PER_SEAT: u32 = 10;
pub const WORK_PER_RECORD: u32 = 1;
pub const BAY: (f32, f32) = (24.0, 12.0);
pub const FLOOR: i32 = 112;
pub const WIDTH: i32 = 1200;
/// Same seed, same breaches, independent contacts in every seat.
pub fn ramparts(seed: u64) -> [Plank; 4] {
    let shift = (seed % 3) as i32 * 4;
    [
        Plank {
            x: 8,
            y: FLOOR,
            w: 188 + shift,
            lift: false,
        },
        Plank {
            x: 248 + shift,
            y: FLOOR,
            w: 220,
            lift: false,
        },
        Plank {
            x: 524 + shift,
            y: FLOOR,
            w: 220,
            lift: false,
        },
        Plank {
            x: 800 + shift,
            y: FLOOR,
            w: 392 - shift,
            lift: false,
        },
    ]
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Area {
    pub fn hits(self, x: i32, y: i32, height: i32) -> bool {
        x + 8 > self.x && x - 8 < self.x + self.w && y > self.y && y - height < self.y + self.h
    }
    fn valid(self) -> bool {
        (0..=WIDTH).contains(&self.x)
            && (-32..=FLOOR).contains(&self.y)
            && (0..=200).contains(&self.w)
            && (0..=144).contains(&self.h)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub x: i32,
    pub hp: u8,
    pub captain: bool,
    pub beat: Beat,
    pub ticks: u32,
    pub hook: bool,
    pub attack: u32,
    pub danger: Area,
    pub aim: (i32, i32),
    pub flinch: u32,
}
impl Foe {
    pub fn new(x: i32, captain: bool) -> Self {
        Self {
            x,
            hp: if captain { 18 } else { 3 },
            captain,
            beat: Beat::Walk,
            ticks: if captain { 45 } else { 6 },
            hook: false,
            attack: 0,
            danger: Area {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            aim: (0, 0),
            flinch: 0,
        }
    }
    pub fn stage(&self) -> usize {
        let full = if self.captain { 18 } else { 3 };
        if self.hp * 3 <= full {
            2
        } else if self.hp * 3 <= full * 2 {
            1
        } else {
            0
        }
    }
    pub fn tell_length(&self) -> u32 {
        33 - self.stage() as u32 * 4
    }
    pub fn tell_stage(&self) -> usize {
        ((self.tell_length() - self.ticks.min(self.tell_length())) * 4 / self.tell_length()).min(3)
            as usize
    }
    pub(super) fn step(&mut self, x: i32, y: i32) -> bool {
        self.flinch = self.flinch.saturating_sub(1);
        if self.hp == 0 {
            return false;
        }
        self.ticks = self.ticks.saturating_sub(1);
        match self.beat {
            Beat::Walk if self.ticks == 0 => {
                self.hook = self.attack % 2 == 1;
                self.attack += 1;
                self.aim = (x, y - 29);
                self.danger = if self.hook {
                    Area {
                        x: (self.x - 156).max(0),
                        y: FLOOR - 23,
                        w: 156,
                        h: 23,
                    }
                } else {
                    Area {
                        x: (x - 26).clamp(8, WIDTH - 60),
                        y: FLOOR - 100,
                        w: 52,
                        h: 100,
                    }
                };
                self.beat = Beat::Tell;
                self.ticks = self.tell_length();
            }
            Beat::Tell if self.ticks == 0 => {
                self.beat = Beat::Attack;
                self.ticks = 10;
                return true;
            }
            Beat::Attack if self.ticks == 0 => {
                self.beat = Beat::Rest;
                self.ticks = 60 - self.stage() as u32 * 10;
            }
            Beat::Rest if self.ticks == 0 => {
                self.beat = Beat::Walk;
                self.ticks = 45;
            }
            _ => {}
        }
        false
    }
    fn valid(&self) -> bool {
        (0..=WIDTH).contains(&self.x)
            && self.hp <= if self.captain { 18 } else { 3 }
            && self.ticks <= 60
            && self.flinch <= 12
            && self.attack <= LENGTH
            && self.danger.valid()
            && self.aim.0.abs() <= WIDTH
            && self.aim.1.abs() <= 256
            && (self.hp == 0) == (self.beat == Beat::Defeat)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shot {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub enemy: bool,
    pub life: u32,
}
impl Shot {
    fn aimed(x: i32, y: i32, tx: i32, ty: i32) -> Self {
        let dx = tx - x;
        let dy = ty - y;
        let length = dx.abs().max(dy.abs()).max(1);
        Self {
            x: x * Q,
            y: y * Q,
            vx: dx * 512 / length,
            vy: dy * 512 / length,
            enemy: true,
            life: 180,
        }
    }
    fn valid(&self) -> bool {
        self.x.abs() <= (WIDTH + 64) * Q
            && self.y.abs() <= 512 * Q
            && self.vx.abs() <= 2048
            && self.vy.abs() <= 2048
            && self.life <= 180
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sample {
    tick: u64,
    x: i32,
    y: i32,
    ground: bool,
    coyote: u32,
    dead: bool,
    duck: bool,
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
    pub jump_key: bool,
    pub fire_key: bool,
    pub duck: bool,
    pub aim: (i32, i32),
    pub last_jump: Option<u64>,
    pub last_shot: Option<u64>,
    pub cooldown: u32,
    pub foes: Vec<Foe>,
    pub shots: Vec<Shot>,
    pub pickups: [bool; 2],
    pub ward: u32,
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
            camera: 0,
            ground: Some(0),
            coyote: COYOTE,
            buffer: 0,
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
            jump_key: false,
            fire_key: false,
            duck: false,
            aim: (1, 0),
            last_jump: None,
            last_shot: None,
            cooldown: 0,
            foes: vec![
                Foe::new(158, false),
                Foe::new(422, false),
                Foe::new(698, false),
                Foe::new(1130, true),
            ],
            shots: Vec::new(),
            pickups: [false; 2],
            ward: 0,
            history: VecDeque::new(),
        }
    }
    pub fn valid(&self) -> bool {
        (16 * Q..=(WIDTH - 24) * Q).contains(&self.x)
            && (-80 * Q..=240 * Q).contains(&self.y)
            && (-2200..=4096).contains(&self.vy)
            && [-1, 1].contains(&self.dir)
            && (0..=WIDTH - 254).contains(&self.camera)
            && self.ground.is_none_or(|i| i < 4)
            && self.checkpoint < 4
            && self.height <= WIDTH as u32
            && self.coyote <= 8
            && self.buffer <= 10
            && self.pose < 12
            && self.pose_ticks <= 12
            && self.freeze <= 2
            && self.down <= 18
            && self.hurt <= 30
            && self.effect <= 12
            && self.cooldown <= 12
            && self.ward <= 1
            && self.falls <= LENGTH
            && self.chain <= LENGTH
            && self.score <= 1000
            && self.finish.is_none_or(|t| t <= LENGTH)
            && self.foes.len() == 4
            && self
                .foes
                .iter()
                .enumerate()
                .all(|(i, f)| f.valid() && f.captain == (i == 3))
            && self.shots.iter().all(Shot::valid)
            && (-1..=1).contains(&self.aim.0)
            && (-1..=1).contains(&self.aim.1)
            && self.aim != (0, 0)
    }
    fn sample(&self, tick: u64) -> Sample {
        Sample {
            tick,
            x: self.x,
            y: self.y,
            ground: self.ground.is_some(),
            coyote: self.coyote,
            dead: self.down > 0,
            duck: self.duck,
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
        if self.ward > 0 {
            self.ward = 0;
            self.hurt = 30;
            self.burst(self.x / Q, self.y / Q - 20);
            return;
        }
        self.falls += 1;
        self.chain = 0;
        self.down = 18;
        self.pose = 8;
        self.burst(self.x / Q, self.y / Q);
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
        let jump = input.interact && !self.jump_key;
        if !input.interact && self.jump_key && self.vy < 0 {
            self.vy /= 2;
        }
        self.jump_key = input.interact;
        self.fire_key = input.fire;
        if self.finish.is_some() {
            self.pose = 9;
            return;
        }
        self.effect = self.effect.saturating_sub(1);
        self.hurt = self.hurt.saturating_sub(1);
        self.cooldown = self.cooldown.saturating_sub(1);
        if jump
            && let Some(s) = sampled
                .as_ref()
                .filter(|s| !s.dead && self.last_jump.is_none_or(|v| s.tick > v))
        {
            self.buffer = t.pennon_buffer;
            self.last_jump = Some(s.tick);
            if s.ground || s.coyote > 0 {
                self.coyote = t.pennon_coyote.max(1);
            }
        }
        if self.freeze > 0 {
            self.freeze -= 1;
            return;
        }
        let ps = ramparts(self.seed);
        if self.down > 0 {
            self.down -= 1;
            if self.down == 0 {
                let p = ps[self.checkpoint];
                self.x = (p.x + 24) * Q;
                self.y = FLOOR * Q;
                self.vy = 0;
                self.ground = Some(self.checkpoint);
                self.hurt = 30;
                self.pose = 5;
                self.pose_ticks = 6;
                self.shots.retain(|s| !s.enemy);
            }
            return;
        }
        self.duck = input.move_y > 0 && input.move_x == 0 && self.ground.is_some();
        if input.move_x != 0 {
            self.dir = i32::from(input.move_x);
        }
        self.aim = if input.aim_x != 0 || input.aim_y != 0 {
            if input.aim_x != 0 {
                self.dir = i32::from(input.aim_x);
            }
            (i32::from(input.aim_x), i32::from(input.aim_y))
        } else if self.duck {
            (self.dir, 0)
        } else {
            (
                if input.move_x == 0 && input.move_y != 0 {
                    0
                } else {
                    self.dir
                },
                i32::from(input.move_y),
            )
        };
        if self.buffer > 0 && (self.ground.is_some() || self.coyote > 0) {
            self.vy = -(t.pennon_jump as i32);
            self.ground = None;
            self.coyote = 0;
            self.buffer = 0;
            self.pose = 3;
            self.duck = false;
        }
        self.buffer = self.buffer.saturating_sub(1);
        let old_y = self.y;
        self.x = (self.x + i32::from(input.move_x) * t.pennon_run as i32)
            .clamp(16 * Q, (WIDTH - 24) * Q);
        if let Some(i) = self.ground {
            if !supports(self.x / Q, &ps[i]) {
                self.ground = None;
                self.coyote = t.pennon_coyote;
            }
        }
        if self.ground.is_none() {
            self.vy = (self.vy + t.pennon_gravity as i32).min(4096);
            self.y += self.vy;
        }
        if self.vy >= 0
            && let Some((i, p)) = landing(old_y, self.y, self.x / Q, &ps)
        {
            let landed = self.ground.is_none();
            self.y = p.y * Q;
            self.vy = 0;
            self.ground = Some(i);
            self.coyote = t.pennon_coyote;
            self.checkpoint = self.checkpoint.max(i);
            if landed {
                self.pose = 5;
                self.pose_ticks = 4;
                self.burst(self.x / Q, FLOOR);
            }
        }
        if self.ground.is_none() {
            self.coyote = self.coyote.saturating_sub(1);
        }
        if self.y > (FLOOR + 70) * Q {
            self.ward = 0;
            self.fail();
        }
        self.height = self.height.max((self.x / Q - 42).max(0) as u32);
        self.camera = camera(self.x / Q, 64, WIDTH - 254);
        // Held fire repeats only when a fresh in-window client sample is available.
        if input.fire
            && self.cooldown == 0
            && let Some(s) = sampled
                .as_ref()
                .filter(|s| !s.dead && self.last_shot.is_none_or(|v| s.tick > v))
        {
            let (dx, dy) = self.aim;
            let speed = if dx != 0 && dy != 0 { 1448 } else { 2048 };
            let age = (now - s.tick) as i32;
            let sx = s.x / Q + self.dir * 18;
            let sy = s.y / Q
                - if s.duck {
                    10
                } else if dy < 0 {
                    29
                } else if dy > 0 {
                    12
                } else {
                    18
                };
            let mut shot = Shot {
                x: sx * Q,
                y: sy * Q,
                vx: dx * speed,
                vy: dy * speed,
                enemy: false,
                life: 30,
            };
            // Swept contacts below include travel since the client's sampled muzzle.
            shot.x += shot.vx * age;
            shot.y += shot.vy * age;
            self.shots.push(shot);
            self.cooldown = 12;
            self.last_shot = Some(s.tick);
        }
        for i in 0..self.foes.len() {
            if self.foes[i].hp == 0
                || ((self.foes[i].x - self.x / Q).abs() > 170 && self.foes[i].attack == 0)
            {
                continue;
            }
            // Later waves only wake once the previous pennant has fallen.
            if i > 0 && self.foes[i - 1].hp > 0 {
                continue;
            }
            let launch = self.foes[i].step(self.x / Q, self.y / Q);
            if launch {
                let f = &self.foes[i];
                if !f.captain {
                    self.shots.push(Shot::aimed(
                        f.x - 16,
                        FLOOR - if f.hook { 16 } else { 26 },
                        f.aim.0,
                        f.aim.1,
                    ));
                }
            }
            let f = &self.foes[i];
            if f.captain
                && f.beat == Beat::Attack
                && f.danger
                    .hits(self.x / Q, self.y / Q, if self.duck { 19 } else { 37 })
            {
                self.fail();
            }
        }
        let mut contact = None;
        let mut hurt = false;
        let active = self.foes.iter().position(|f| f.hp > 0);
        for shot in &mut self.shots {
            let old = (shot.x, shot.y);
            shot.x += shot.vx;
            shot.y += shot.vy;
            shot.life = shot.life.saturating_sub(1);
            // Integer swept segment/body contacts, including retroactive muzzle travel.
            let start = if !shot.enemy && shot.life == 29 {
                sampled.as_ref().map_or(old, |s| {
                    let age = (now - s.tick) as i32;
                    (old.0 - shot.vx * age, old.1 - shot.vy * age)
                })
            } else {
                old
            };
            if shot.enemy {
                if segment_hits(
                    start,
                    (shot.x, shot.y),
                    (
                        self.x / Q - 8,
                        self.y / Q - if self.duck { 19 } else { 37 },
                        16,
                        if self.duck { 19 } else { 37 },
                    ),
                ) {
                    hurt = true;
                    shot.life = 0;
                }
            } else {
                for (index, f) in self.foes.iter_mut().enumerate() {
                    if Some(index) == active
                        && ((f.x - self.x / Q).abs() <= 170 || f.attack > 0)
                        && f.hp > 0
                        && segment_hits(
                            start,
                            (shot.x, shot.y),
                            (
                                f.x - if f.captain { 26 } else { 16 },
                                FLOOR - if f.captain { 65 } else { 38 },
                                if f.captain { 52 } else { 32 },
                                if f.captain { 65 } else { 38 },
                            ),
                        )
                    {
                        f.hp -= 1;
                        f.flinch = 4;
                        if f.hp == 0 {
                            f.beat = Beat::Defeat;
                        }
                        shot.life = 0;
                        contact = Some((f.x, FLOOR - 30));
                        self.chain += 1;
                        break;
                    }
                }
            }
            if shot.x < 0 || shot.x > WIDTH * Q || shot.y < -80 * Q || shot.y > 200 * Q {
                shot.life = 0;
            }
        }
        self.shots.retain(|s| s.life > 0);
        if let Some((x, y)) = contact {
            self.burst(x, y);
        }
        if hurt {
            self.fail();
        }
        for (i, x) in [340, 848].into_iter().enumerate() {
            if !self.pickups[i] && (self.x / Q - x).abs() < 16 && self.y / Q > FLOOR - 28 {
                self.pickups[i] = true;
                self.ward = 1;
                self.burst(x, FLOOR - 15);
            }
        }
        let hits = self
            .foes
            .iter()
            .enumerate()
            .map(|(i, f)| {
                if i == 3 {
                    18 - f.hp as u32
                } else {
                    3 - f.hp as u32
                }
            })
            .sum::<u32>();
        self.score = (self.height.min(900) * 500 / 900 + hits * 500 / 27).min(999);
        if self.foes.iter().all(|f| f.hp == 0) {
            self.finish = Some(elapsed);
            self.score = 1000;
            self.pose = 9;
            self.shots.clear();
        }
        self.pose_ticks = self.pose_ticks.saturating_sub(1);
        if self.down == 0 && self.finish.is_none() && self.pose_ticks == 0 {
            self.pose = if self.duck {
                10
            } else if self.ground.is_none() {
                if self.vy < 0 { 3 } else { 4 }
            } else if input.fire {
                if self.aim.1 < 0 {
                    6
                } else if self.aim.1 > 0 {
                    7
                } else if input.move_x != 0 {
                    1 + (elapsed / 5 % 2) as u8
                } else {
                    11
                }
            } else if input.move_x != 0 {
                1 + (elapsed / 5 % 2) as u8
            } else {
                0
            };
        }
    }
    /// Visible gap edges and locked tells are sufficient; no future seed information.
    pub fn bot_input(&self, _elapsed: u32) -> Input {
        let ps = ramparts(self.seed);
        let x = self.x / Q;
        let foe = self.foes.iter().find(|f| f.hp > 0);
        let mut move_x = if foe.is_some_and(|f| f.x - x < 145 && f.x > x) {
            0
        } else {
            1
        };
        let imminent = self
            .ground
            .and_then(|i| ps.get(i))
            .is_some_and(|p| x > p.x + p.w - 12 && p.x + p.w < 1000);
        let danger = foe.is_some_and(|f| {
            f.captain
                && matches!(f.beat, Beat::Tell | Beat::Attack)
                && f.danger.hits(x, self.y / Q, 37)
        });
        if danger && foe.is_some_and(|f| !f.hook) {
            move_x = -1;
        }
        let jump = imminent
            || (danger && foe.is_some_and(|f| f.hook && (f.beat == Beat::Attack || f.ticks <= 12)));
        Input {
            move_x,
            move_y: 0,
            fire: foe.is_some_and(|f| f.x >= x),
            interact: (jump && self.ground.is_some()) || (self.jump_key && self.vy < 0),
            ..Default::default()
        }
    }
}
/// A fixed-point segment crosses a native pixel rectangle; no floating hot-path work.
pub fn segment_hits(a: (i32, i32), b: (i32, i32), r: (i32, i32, i32, i32)) -> bool {
    let (x, y, w, h) = r;
    let (l, t, rr, bb) = (x * Q, y * Q, (x + w) * Q, (y + h) * Q);
    if a.0.max(b.0) < l || a.0.min(b.0) > rr || a.1.max(b.1) < t || a.1.min(b.1) > bb {
        return false;
    }
    let cross = |p: (i32, i32)| {
        i64::from(b.0 - a.0) * i64::from(p.1 - a.1) - i64::from(b.1 - a.1) * i64::from(p.0 - a.0)
    };
    let c = [
        cross((l, t)),
        cross((rr, t)),
        cross((rr, bb)),
        cross((l, bb)),
    ];
    c.iter().any(|&v| v >= 0) && c.iter().any(|&v| v <= 0)
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
    pub fn pennon_showing(&self) -> bool {
        self.room().kind == RoomKind::Muster && self.pennon.is_some()
    }
    pub(super) fn tick_pennon(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Muster {
            self.pennon = None;
            return;
        }
        if self.pennon.is_none() {
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
                self.start_pennon();
            }
            return;
        }
        let mut s = self.pennon.take().unwrap();
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
                self.start_pennon();
            } else {
                s.key = take;
                self.pennon = Some(s);
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
            let record = &mut self.home.countryside.games.pennon;
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
        self.pennon = Some(s);
    }
    fn start_pennon(&mut self) {
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
        self.pennon = Some(s);
        self.home.countryside.games.pennon.plays =
            self.home.countryside.games.pennon.plays.saturating_add(1);
        self.sounds.push("play_card");
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn pennon_for_test() -> Run {
        let mut home = super::home::Home::default();
        home.levels.insert(super::home::Station::Wing, 1);
        let mut r = Run::at_home(41, 1, Some("Gawain"), home, Default::default());
        let at = r
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Muster)
            .unwrap();
        r.arrive(at, (BAY.0 / 2.0, BAY.1 / 2.0));
        r
    }
}
#[cfg(any(test, feature = "testing"))]
impl Run {
    pub fn pennon_review_for_test(moment: &str) -> Run {
        let mut r = Self::pennon_for_test();
        for (id, name) in [(2, "LYRA"), (3, "BRAN"), (4, "ELIN")] {
            r.join_seat(id, name);
        }
        if moment == "entrance" {
            return r;
        }
        r.start_pennon();
        let s = r.pennon.as_mut().unwrap();
        s.left = LENGTH - 900;
        for (&id, d) in &mut s.lanes {
            d.score = id * 160;
            d.height = id * 160;
        }
        let d = s.lanes.get_mut(&1).unwrap();
        d.x = 1028 * Q;
        d.camera = WIDTH - 254;
        d.ground = Some(3);
        d.checkpoint = 3;
        d.height = 900;
        d.score = 700;
        d.chain = 9;
        for f in &mut d.foes[..3] {
            f.hp = 0;
            f.beat = Beat::Defeat;
        }
        if let Some(cam) = moment
            .strip_prefix("rampart-")
            .and_then(|n| n.parse::<i32>().ok())
        {
            *d = Lane::new(s.seed);
            d.camera = cam;
            d.x = (cam + 94) * Q;
            d.pose = 1;
        }
        if moment == "start" || moment == "sentry" {
            *d = Lane::new(s.seed);
            d.x = 72 * Q;
        }
        let f = &mut d.foes[3];
        match moment {
            "idle" => {
                f.beat = Beat::Rest;
                f.ticks = 20;
            }
            "jump" => {
                d.y -= 35 * Q;
                d.ground = None;
                d.vy = -500;
                d.pose = 3;
            }
            "fall" => {
                d.y -= 20 * Q;
                d.ground = None;
                d.vy = 500;
                d.pose = 4;
            }
            "land" => {
                d.pose = 5;
                d.effect = 8;
                d.effect_x = d.x / Q;
                d.effect_y = FLOOR;
            }
            "hit" => {
                d.pose = 8;
                d.down = 18;
            }
            "duck" => {
                d.pose = 10;
                d.duck = true;
                d.shots.push(Shot {
                    x: d.x + 26 * Q,
                    y: (FLOOR - 29) * Q,
                    vx: -512,
                    vy: 0,
                    enemy: true,
                    life: 90,
                });
            }
            "aim-up" => {
                d.pose = 6;
                d.aim = (1, -1);
            }
            "aim-down" => {
                d.pose = 7;
                d.aim = (1, 1);
            }
            "shoot" => {
                d.pose = 11;
                d.shots.push(Shot {
                    x: 1080 * Q,
                    y: (FLOOR - 18) * Q,
                    vx: 2048,
                    vy: 0,
                    enemy: false,
                    life: 70,
                });
            }
            "drop-tell" | "drop" | "hook-tell" | "hook" => {
                f.hook = moment.starts_with("hook");
                f.beat = if moment.ends_with("tell") {
                    Beat::Tell
                } else {
                    Beat::Attack
                };
                f.ticks = if f.beat == Beat::Tell { 16 } else { 7 };
                f.danger = if f.hook {
                    Area {
                        x: f.x - 156,
                        y: FLOOR - 23,
                        w: 156,
                        h: 23,
                    }
                } else {
                    Area {
                        x: 984,
                        y: FLOOR - 100,
                        w: 52,
                        h: 100,
                    }
                };
            }
            "flinch" => {
                f.flinch = 12;
                d.effect = 10;
                d.effect_x = f.x;
                d.effect_y = FLOOR - 30;
            }
            "damage-1" => {
                f.hp = 12;
                f.beat = Beat::Rest;
                f.ticks = 20;
            }
            "damage-2" => {
                f.hp = 6;
                f.beat = Beat::Rest;
                f.ticks = 20;
            }
            "defeat" | "podium" => {
                f.hp = 0;
                f.beat = Beat::Defeat;
                d.finish = Some(1101);
                d.score = 1000;
                d.pose = 9;
            }
            "recover" => {
                f.beat = Beat::Rest;
                f.ticks = 8;
            }
            _ => {}
        }
        if moment == "sentry" {
            let f = &mut d.foes[0];
            f.beat = Beat::Tell;
            f.ticks = 16;
            f.aim = (d.x / Q, FLOOR - 29);
        }
        if moment == "podium" {
            s.left = 0;
            s.order = vec![1, 4, 3, 2];
            s.won = true;
        }
        r
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// All four frames use the real room/layers and the shared terminal painter.
    pub fn pennon_fx_review_for_test(kind: &str, n: usize) -> Run {
        let mut r = Self::pennon_review_for_test(kind);
        let d = r.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap();
        if kind == "drop" || kind == "hook" {
            d.foes[3].ticks = [10, 7, 5, 2][n];
        } else {
            d.effect = [12, 9, 6, 3][n];
            d.effect_x = if kind == "land" { d.x / Q } else { 1130 };
            d.effect_y = if kind == "land" { FLOOR } else { FLOOR - 30 };
        }
        r
    }
}
