//! One mounted knight's body: momentum along a curve, the tuck roll, the
//! charge dash, the spur, the leap and the lance-lock. This file knows the
//! terrain and nothing else; the foes, springs and posts are the session's
//! (`world.rs`), which calls the few methods here that change a rider.
//!
//! Numbers are fixed point: positions in 1/256 pixel, speeds in 1/256 pixel
//! a tick, accelerations in 1/256 pixel a tick squared, on the 30 Hz sim.
use super::math::{self, ONE};
use super::terrain::{Hit, Terrain};
use super::host::Input;
use serde::{Deserialize, Serialize};

pub const TOP: i32 = 1280;
pub const ACC: i32 = 26;
pub const DEC: i32 = 130;
pub const FRIC: i32 = 12;
pub const DRAG: i32 = 6;
pub const SLOPE: i32 = 28;
pub const ROLL_DOWN: i32 = 250;
pub const ROLL_UP: i32 = 60;
pub const ROLL_FRIC: i32 = 6;
pub const ROLL_DEC: i32 = 40;
pub const ROLL_MIN: i32 = 192;
pub const ROLL_STOP: i32 = 80;
pub const ROLL_UNCURL: i32 = 640;
pub const GRAVITY: i32 = 100;
pub const FALL_MAX: i32 = 4096;
pub const AIR_ACC: i32 = 60;
pub const JUMP: i32 = 1560;
pub const JUMP_CUT: i32 = 1024;
pub const COYOTE: u32 = 4;
pub const BUFFER: u32 = 5;
pub const SPUR_ACC: i32 = 96;
pub const SPUR_KICK: i32 = 256;
pub const SPUR_DRAIN: i32 = 12;
pub const GAUGE_MAX: i32 = 1000;
pub const GAUGE_START: i32 = 500;
pub const CHARGE_STEP: i32 = 320;
pub const CHARGE_MAX: i32 = 1280;
pub const CHARGE_BASE: i32 = 768;
pub const DRAFT_ACC: i32 = 26;
pub const DRAFT_CAP: i32 = 128;
pub const LOCK_SPEED: i32 = 3072;
pub const LOCK_TICKS: u8 = 14;
pub const LOCK_BOUNCE: i32 = 1280;
pub const LUNGE_SPEED: i32 = 2304;
pub const LUNGE_TICKS: u8 = 8;
pub const BOUNCE: i32 = 1100;
pub const BOUNCE_HELD: i32 = 320;
pub const SPEED_MAX: i32 = 4096;
pub const SCATTER_MAX: u32 = 16;
pub const LOOSE_LIFE: u16 = 120;
pub const LOOSE_DELAY: u16 = 24;
pub const HURT_TICKS: u32 = 90;
pub const KNOCK_TICKS: u32 = 18;
pub const DOWN_TICKS: u32 = 24;
pub const WORLD_PX: i32 = 20000;
pub const SPARK_WORDS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Numbers {
    pub slope: i32,
    pub loop_min: i32,
    pub spur_cap: i32,
    pub lock_reach: i32,
    pub dimming: i32,
}

impl Default for Numbers {
    fn default() -> Self {
        Numbers { slope: 100, loop_min: 512, spur_cap: 2048, lock_reach: 96, dimming: 1000 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub x: i32,
    pub down: bool,
    pub jump: bool,
    pub spur: bool,
}

impl Keys {
    pub fn of(i: &Input) -> Keys {
        Keys {
            x: i32::from(i.move_x),
            down: i.move_y > 0 || i.aim_y > 0,
            jump: i.interact || i.move_y < 0,
            spur: i.dash || i.fire,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ground {
    pub path: u8,
    pub s: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    pub target: u16,
    pub ticks: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loose {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub age: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ev {
    Jump,
    Land(i32),
    Roll,
    Rev(i32),
    Launch,
    Spur,
    Skid,
    Loop,
    Fell,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rider {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub ground: Option<Ground>,
    pub gsp: i32,
    pub dir: i32,
    pub roll: bool,
    pub ball: bool,
    pub crouch: bool,
    pub skid: bool,
    pub charge: i32,
    pub spur: bool,
    pub gauge: i32,
    pub lock: Option<Lock>,
    pub lock_cd: u8,
    pub spring_cd: u8,
    pub freeze: u8,
    pub shake: u8,
    pub lunge: u8,
    pub lunged: bool,
    pub chain: u32,
    pub best_chain: u32,
    pub sparks: u32,
    pub shield: bool,
    pub hurt: u32,
    pub ctrl: u32,
    pub down: u32,
    pub coyote: u32,
    pub buffer: u32,
    pub jump_key: bool,
    pub spur_key: bool,
    pub jumping: bool,
    pub air: u32,
    pub draft: bool,
    pub ang: i32,
    pub cam_x: i32,
    pub cam_y: i32,
    pub zoom: i32,
    pub looping: bool,
    pub want_lock: bool,
    pub loose: Vec<Loose>,
    pub taken: [u64; SPARK_WORDS],
    pub crates: u32,
    pub collected: u32,
    pub falls: u32,
    pub hits: u32,
    pub kills: u32,
    pub finish: Option<u32>,
    pub score: u32,
}

impl Rider {
    pub fn new_on(t: &Terrain, path: usize, s: i32) -> Self {
        let at = t.paths[path].sample(s);
        Rider {
            x: at.x,
            y: at.y,
            vx: 0,
            vy: 0,
            ground: Some(Ground { path: path as u8, s }),
            gsp: 0,
            dir: 1,
            roll: false,
            ball: false,
            crouch: false,
            skid: false,
            charge: 0,
            spur: false,
            gauge: GAUGE_START,
            lock: None,
            lock_cd: 0,
            spring_cd: 0,
            freeze: 0,
            shake: 0,
            lunge: 0,
            lunged: false,
            chain: 0,
            best_chain: 0,
            sparks: 0,
            shield: false,
            hurt: 0,
            ctrl: 0,
            down: 0,
            coyote: COYOTE,
            buffer: 0,
            jump_key: false,
            spur_key: false,
            jumping: false,
            air: 0,
            draft: false,
            ang: 0,
            cam_x: 0,
            cam_y: 0,
            zoom: 256,
            looping: false,
            want_lock: false,
            loose: Vec::new(),
            taken: [0; SPARK_WORDS],
            crates: 0,
            collected: 0,
            falls: 0,
            hits: 0,
            kills: 0,
            finish: None,
            score: 0,
        }
    }

    pub fn place(&mut self, t: &Terrain, path: usize, s: i32) {
        let at = t.paths[path].sample(s);
        let keep = Rider {
            collected: self.collected,
            falls: self.falls,
            hits: self.hits,
            kills: self.kills,
            best_chain: self.best_chain,
            taken: self.taken,
            crates: self.crates,
            finish: self.finish,
            gauge: self.gauge.max(GAUGE_START * 3 / 5),
            ..Rider::new_on(t, path, s)
        };
        *self = Rider { hurt: HURT_TICKS, ..keep };
        self.snap_camera(at.x / 256, at.y / 256);
    }

    pub fn snap_camera(&mut self, px: i32, py: i32) {
        self.zoom = 256;
        self.cam_x = px - 384 / 2 + 40;
        self.cam_y = py - 224 * 60 / 100;
    }

    pub fn grounded(&self) -> bool {
        self.ground.is_some()
    }

    pub fn attacking(&self) -> bool {
        self.roll || self.ball || self.lock.is_some() || self.lunge > 0
    }

    pub fn speed(&self) -> i32 {
        if self.grounded() {
            self.gsp.abs()
        } else {
            math::isqrt((i64::from(self.vx).pow(2) + i64::from(self.vy).pow(2)) as u64) as i32
        }
    }

    pub fn body(&self) -> (i32, i32) {
        if self.roll || self.ball || self.crouch || self.lock.is_some() {
            (7, 14)
        } else {
            (7, 28)
        }
    }

    pub fn centre(&self, t: &Terrain) -> (i32, i32) {
        let h = self.body().1 * 128;
        match self.ground {
            Some(g) => {
                let s = t.paths[g.path as usize].sample(g.s);
                (self.x + (i64::from(s.ty) * i64::from(h) / i64::from(ONE)) as i32,
                 self.y - (i64::from(s.tx) * i64::from(h) / i64::from(ONE)) as i32)
            }
            None => (self.x, self.y - h),
        }
    }

    pub fn step(&mut self, k: Keys, t: &Terrain, n: &Numbers, ev: &mut Vec<Ev>) {
        self.hurt = self.hurt.saturating_sub(1);
        self.ctrl = self.ctrl.saturating_sub(1);
        self.lock_cd = self.lock_cd.saturating_sub(1);
        self.spring_cd = self.spring_cd.saturating_sub(1);
        self.shake = self.shake.saturating_sub(1);
        self.skid = false;
        if self.freeze > 0 {
            self.freeze -= 1;
            if k.jump && !self.jump_key {
                self.buffer = BUFFER;
            } else {
                self.buffer = self.buffer.saturating_sub(1);
            }
            self.jump_key = k.jump;
            self.spur_key = k.spur;
            self.after(t);
            return;
        }
        let jump_edge = k.jump && !self.jump_key;
        let spur_edge = k.spur && !self.spur_key;
        self.jump_key = k.jump;
        self.spur_key = k.spur;
        if jump_edge {
            self.buffer = BUFFER;
        }
        if self.finish.is_some() {
            let k = Keys::default();
            if self.grounded() {
                self.ground_tick(k, t, n, ev, false, false);
            } else {
                self.air_tick(k, t, n, ev, false, false);
            }
            self.after(t);
            return;
        }
        if self.grounded() {
            self.ground_tick(k, t, n, ev, jump_edge, spur_edge);
        } else {
            self.air_tick(k, t, n, ev, jump_edge, spur_edge);
        }
        self.after(t);
    }

    fn pull(&self, ty: i32, n: &Numbers) -> i32 {
        let base = (i64::from(SLOPE) * i64::from(n.slope) * i64::from(ty) / (100 * i64::from(ONE))) as i32;
        if !self.roll || base == 0 || self.gsp == 0 {
            return base;
        }
        let downhill = (base > 0) == (self.gsp > 0);
        base * if downhill { ROLL_DOWN } else { ROLL_UP } / 100
    }

    fn ground_tick(&mut self, k: Keys, t: &Terrain, n: &Numbers, ev: &mut Vec<Ev>, jump_edge: bool, spur_edge: bool) {
        let Some(g) = self.ground else { return };
        let smp = t.paths[g.path as usize].sample(g.s);
        self.coyote = COYOTE;
        self.air = 0;
        let locked = self.ctrl > 0 || self.finish.is_some();
        let kx = if locked { 0 } else { k.x };
        let down = k.down && !locked;
        let slow = self.gsp.abs() < ROLL_MIN;

        if !self.roll && down && slow {
            self.crouch = true;
            if jump_edge {
                self.charge = (self.charge + CHARGE_STEP).min(CHARGE_MAX);
                self.buffer = 0;
                ev.push(Ev::Rev(self.charge));
            }
        }
        if self.crouch && (!down || !slow) {
            self.crouch = false;
            if self.charge > 0 {
                self.gsp = self.dir * (CHARGE_BASE + self.charge);
                self.roll = true;
                self.charge = 0;
                ev.push(Ev::Launch);
            }
        }
        if !self.crouch {
            self.charge = 0;
        } else {
            self.charge -= self.charge >> 5;
        }
        if down && !self.crouch && !self.roll && !slow {
            self.roll = true;
            ev.push(Ev::Roll);
        }
        if self.roll && self.gsp.abs() < ROLL_STOP && self.pull(smp.ty, n).abs() < FRIC {
            self.roll = false;
        }
        if self.roll && !down && self.gsp.abs() < ROLL_UNCURL {
            self.roll = false;
        }

        let spurring = k.spur && self.gauge > 0 && !self.crouch && !locked;
        self.spur = spurring;
        let mut cap = TOP + if self.draft { DRAFT_CAP } else { 0 };
        if spurring {
            cap = n.spur_cap;
            self.gauge = (self.gauge - SPUR_DRAIN).max(0);
            if spur_edge && self.gsp * self.dir < cap {
                self.gsp += self.dir * SPUR_KICK;
                ev.push(Ev::Spur);
            }
        }

        if self.buffer > 0 && !self.crouch && !locked {
            let (c, d) = (i64::from(smp.tx), i64::from(smp.ty));
            let up = (smp.ty, -smp.tx);
            self.vx = ((i64::from(self.gsp) * c + i64::from(JUMP) * i64::from(up.0)) / i64::from(ONE)) as i32;
            self.vy = ((i64::from(self.gsp) * d + i64::from(JUMP) * i64::from(up.1)) / i64::from(ONE)) as i32;
            self.ground = None;
            self.buffer = 0;
            self.coyote = 0;
            self.jumping = true;
            self.ball = true;
            self.roll = false;
            self.air = 0;
            self.looping = false;
            ev.push(Ev::Jump);
            return;
        }

        self.buffer = self.buffer.saturating_sub(1);

        let grip = if smp.tx < 0 { 0 } else { (ONE - smp.ty.abs() * 3 / 2).max(0) };
        let acc = ACC * grip / ONE;
        let mut gsp = self.gsp;
        gsp += self.pull(smp.ty, n);
        if self.roll {
            if kx != 0 && kx * gsp < 0 {
                gsp += kx * ROLL_DEC;
            }
            gsp = toward_zero(gsp, ROLL_FRIC * n.slope / 100);
        } else if self.crouch || self.finish.is_some() {
            gsp = toward_zero(gsp, if self.finish.is_some() { DEC / 2 } else { DEC });
        } else {
            if spurring && gsp * self.dir < cap {
                gsp = (gsp + self.dir * (SPUR_ACC * grip.max(ONE / 2) / ONE)).clamp(-cap, cap);
            }
            if kx != 0 {
                if gsp * kx < 0 {
                    if gsp.abs() >= 768 {
                        self.skid = true;
                        ev.push(Ev::Skid);
                    }
                    gsp += kx * DEC;
                    if gsp * kx > 0 {
                        gsp = kx * acc.max(1);
                        self.dir = kx;
                    }
                } else {
                    if !self.skid {
                        self.dir = kx;
                    }
                    if gsp * kx < cap {
                        gsp += kx * acc;
                        if gsp * kx > cap {
                            gsp = kx * cap;
                        }
                    } else if !spurring {
                        gsp -= kx * DRAG.min(gsp * kx - cap);
                    }
                    if self.draft && !spurring && gsp * kx > 0 && gsp * kx < cap {
                        gsp += kx * DRAFT_ACC * grip / ONE;
                    }
                }
            } else if !spurring {
                gsp = toward_zero(gsp, FRIC * n.slope / 100);
                if gsp.abs() > 512 {
                    self.dir = gsp.signum();
                }
            } else if self.draft {
                gsp += self.dir * DRAFT_ACC;
            }
        }
        self.gsp = gsp.clamp(-SPEED_MAX, SPEED_MAX);

        let (mut path, mut s) = (g.path as usize, g.s);
        let was_ring = t.paths[path].kind == super::terrain::Kind::Ring;
        match advance(t, &mut path, &mut s, self.gsp) {
            Leave::Stay => {}
            Leave::Wall => {
                self.gsp = 0;
            }
            Leave::End => {
                let at = t.paths[path].sample(s);
                self.vx = (i64::from(self.gsp) * i64::from(at.tx) / i64::from(ONE)) as i32;
                self.vy = (i64::from(self.gsp) * i64::from(at.ty) / i64::from(ONE)) as i32;
                self.x = at.x;
                self.y = at.y;
                self.ground = None;
                self.air = 0;
                self.coyote = COYOTE;
                self.roll = false;
                return;
            }
        }
        let at = t.paths[path].sample(s);
        let on_ring = t.paths[path].kind == super::terrain::Kind::Ring;
        if at.tx < 0 && self.gsp.abs() < n.loop_min {
            self.vx = (i64::from(self.gsp) * i64::from(at.tx) / i64::from(ONE)) as i32;
            self.vy = (i64::from(self.gsp) * i64::from(at.ty) / i64::from(ONE)) as i32;
            self.x = at.x;
            self.y = at.y;
            self.ground = None;
            self.air = 0;
            self.coyote = 0;
            self.roll = false;
            self.ball = false;
            self.ctrl = self.ctrl.max(12);
            ev.push(Ev::Fell);
            return;
        }
        if on_ring && !was_ring {
            ev.push(Ev::Loop);
        }
        self.looping = on_ring;
        self.ground = Some(Ground { path: path as u8, s });
        self.x = at.x;
        self.y = at.y;
    }

    fn air_tick(&mut self, k: Keys, t: &Terrain, n: &Numbers, ev: &mut Vec<Ev>, jump_edge: bool, spur_edge: bool) {
        self.air += 1;
        self.coyote = self.coyote.saturating_sub(1);
        self.crouch = false;
        self.charge = 0;
        let locked = self.ctrl > 0 || self.finish.is_some();
        let kx = if locked { 0 } else { k.x };

        if self.buffer > 0 && self.coyote > 0 && !locked && !self.jumping {
            self.vy = -JUMP;
            self.buffer = 0;
            self.coyote = 0;
            self.jumping = true;
            self.ball = true;
            ev.push(Ev::Jump);
        }
        self.want_lock = jump_edge && self.buffer > 0 && self.lock.is_none() && !locked;
        self.buffer = self.buffer.saturating_sub(1);

        let spurring = k.spur && self.gauge > 0 && !locked;
        self.spur = spurring;
        let mut cap = TOP + if self.draft { DRAFT_CAP } else { 0 };
        if spurring {
            cap = n.spur_cap;
            self.gauge = (self.gauge - SPUR_DRAIN).max(0);
            if spur_edge {
                ev.push(Ev::Spur);
            }
        }
        if self.lock.is_none() {
            if self.lunge > 0 {
                self.lunge -= 1;
                self.vx = self.dir * LUNGE_SPEED;
                self.vy = (self.vy + GRAVITY / 3).min(256);
            } else {
                if kx != 0 {
                    self.dir = kx;
                    if self.vx * kx < cap {
                        self.vx += kx * AIR_ACC;
                        if self.vx * kx > cap {
                            self.vx = kx * cap;
                        }
                    }
                } else if spurring && self.vx * self.dir < cap {
                    self.vx += self.dir * SPUR_ACC / 2;
                }
                if self.draft && self.vx * self.dir > 0 {
                    self.vx += self.dir * DRAFT_ACC / 2;
                }
                self.vy = (self.vy + GRAVITY).min(FALL_MAX);
            }
            if self.jumping {
                if !k.jump && self.vy < -JUMP_CUT {
                    self.vy = -JUMP_CUT;
                }
                if self.vy >= 0 {
                    self.jumping = false;
                }
            }
        }
        self.vx = self.vx.clamp(-SPEED_MAX, SPEED_MAX);

        let from = (self.x, self.y);
        let mut to = (self.x + self.vx, self.y + self.vy);
        let (lo, hi) = (12 * 256, WORLD_PX * 256 - 8 * 256);
        if to.0 < lo || to.0 > hi {
            to.0 = to.0.clamp(lo, hi);
            self.vx = if to.0 == lo { self.vx.max(0) } else { self.vx.min(0) };
        }
        if let Some(h) = t.land(from, to) {
            self.land(h, t, k, ev);
            return;
        }
        self.x = to.0;
        self.y = to.1;
    }

    fn land(&mut self, h: Hit, t: &Terrain, k: Keys, ev: &mut Vec<Ev>) {
        let s = t.paths[h.path].sample(h.s);
        let along = (i64::from(self.vx) * i64::from(s.tx) + i64::from(self.vy) * i64::from(s.ty)) / i64::from(ONE);
        let normal = (i64::from(self.vx) * i64::from(s.ty) - i64::from(self.vy) * i64::from(s.tx)) / i64::from(ONE);
        self.gsp = along as i32;
        self.ground = Some(Ground { path: h.path as u8, s: h.s });
        self.x = h.x;
        self.y = h.y;
        self.vx = 0;
        self.vy = 0;
        self.jumping = false;
        self.ball = false;
        self.lock = None;
        self.lunge = 0;
        self.lunged = false;
        self.chain = 0;
        self.air = 0;
        self.coyote = COYOTE;
        self.roll = k.down && self.gsp.abs() >= ROLL_MIN;
        ev.push(Ev::Land(normal.unsigned_abs() as i32));
    }

    fn after(&mut self, t: &Terrain) {
        let (target, ease, slew) = match self.ground {
            Some(g) => {
                let s = t.paths[g.path as usize].sample(g.s);
                (math::atan2(s.ty, s.tx), 2, 40)
            }
            None if self.lock.is_some() => (math::atan2(self.vy, self.vx.abs().max(1) * self.dir), 3, 30),
            None => (0, 5, 16),
        };
        self.ang = math::turn_toward(self.ang, target, ease, slew);
        self.camera();
    }

    fn camera(&mut self) {
        let (px, py) = (self.x / 256, self.y / 256);
        let want = 256 - ((self.speed() - 1200).clamp(0, 800) * 64 / 800);
        self.zoom += ((want - self.zoom) / 8).clamp(-4, 4);
        if self.zoom == want || (want - self.zoom).abs() < 8 {
            self.zoom += (want - self.zoom).signum();
        }
        let (vw, vh) = (384 * 256 / self.zoom, 224 * 256 / self.zoom);
        let lead = (self.dir * self.speed() * 22 / 256).clamp(-120, 120);
        let tx = px + lead - vw / 2;
        self.cam_x += ((tx - self.cam_x) / 6).clamp(-24, 24);
        if (tx - self.cam_x).abs() > vw {
            self.cam_x = tx;
        }
        let ty = py - vh * 60 / 100;
        let d = ty - self.cam_y;
        let dead = 22;
        let follow = if self.looping { 14 } else { 5 };
        if d.abs() > dead {
            self.cam_y += ((d - d.signum() * dead) / follow).clamp(-20, 20);
        }
    }

    pub fn strike(&mut self) -> bool {
        self.kills += 1;
        if self.lock.take().is_some() {
            self.vy = -LOCK_BOUNCE;
            self.vx = (self.vx / 2).clamp(-1536, 1536);
            self.lock_cd = 2;
            self.lunge = 0;
            self.lunged = false;
            self.chain += 1;
            self.best_chain = self.best_chain.max(self.chain);
            return true;
        }
        if !self.grounded() {
            self.vy = -(BOUNCE + if self.jump_key { BOUNCE_HELD } else { 0 });
            self.jumping = false;
            self.lunge = 0;
            self.chain += 1;
            self.best_chain = self.best_chain.max(self.chain);
            return true;
        }
        false
    }

    pub fn knock(&mut self, from: i32) -> Blow {
        if self.hurt > 0 || self.down > 0 {
            return Blow::None;
        }
        self.hits += 1;
        let away = if self.x >= from { 1 } else { -1 };
        self.ground = None;
        self.roll = false;
        self.ball = false;
        self.crouch = false;
        self.charge = 0;
        self.lock = None;
        self.lunge = 0;
        self.vx = away * 640;
        self.vy = -1280;
        self.ctrl = KNOCK_TICKS;
        self.jumping = false;
        self.chain = 0;
        self.freeze = 3;
        self.shake = 8;
        if self.shield {
            self.shield = false;
            self.hurt = HURT_TICKS;
            return Blow::Shielded;
        }
        if self.sparks > 0 {
            let n = self.sparks.min(SCATTER_MAX);
            self.sparks = 0;
            self.hurt = HURT_TICKS;
            self.scatter(n);
            return Blow::Scattered(n);
        }
        self.down = DOWN_TICKS;
        self.falls += 1;
        self.vy = -1600;
        Blow::Down
    }

    fn scatter(&mut self, n: u32) {
        self.loose.clear();
        let (cx, cy) = (self.x, self.y - 14 * 256);
        for i in 0..n as i32 {
            let a = -96 + (i * 37) % 64 + (i % 2) * 3;
            let pace = 1100 + (i * 97) % 900;
            self.loose.push(Loose {
                x: cx,
                y: cy,
                vx: (i64::from(math::cos(a)) * i64::from(pace) / i64::from(ONE)) as i32,
                vy: (i64::from(math::sin(a)) * i64::from(pace) / i64::from(ONE)) as i32,
                age: 0,
            });
        }
    }

    pub fn tumble(&mut self) {
        self.vy = (self.vy + GRAVITY).min(FALL_MAX);
        self.x += self.vx;
        self.y += self.vy;
        self.down = self.down.saturating_sub(1);
        self.zoom = (self.zoom + 8).min(256);
    }

    pub fn tick_loose(&mut self, t: &Terrain) {
        for l in &mut self.loose {
            l.age += 1;
            l.vy = (l.vy + GRAVITY).min(FALL_MAX);
            let to = (l.x + l.vx, l.y + l.vy);
            if let Some(h) = t.land((l.x, l.y), to) {
                l.x = h.x;
                l.y = h.y - 128;
                l.vy = -(l.vy * 3 / 4).max(300);
                l.vx = l.vx * 9 / 10;
            } else {
                l.x = to.0;
                l.y = to.1;
            }
        }
        self.loose.retain(|l| l.age < LOOSE_LIFE);
    }

    pub fn valid(&self, t: &Terrain) -> bool {
        let bound = |v: i32, m: i32| (-m..=m).contains(&v);
        self.ground.is_none_or(|g| {
            (g.path as usize) < t.paths.len() && (0..=t.paths[g.path as usize].len()).contains(&g.s)
        }) && bound(self.vx, SPEED_MAX)
            && bound(self.vy, SPEED_MAX)
            && bound(self.gsp, SPEED_MAX)
            && [-1, 1].contains(&self.dir)
            && (0..=20000 * 256).contains(&self.x)
            && (-200 * 256..=1400 * 256).contains(&self.y)
            && (0..=GAUGE_MAX).contains(&self.gauge)
            && (0..=CHARGE_MAX).contains(&self.charge)
            && self.chain <= 1000
            && self.best_chain <= 1000
            && self.sparks <= 9999
            && self.hurt <= HURT_TICKS * 2
            && self.ctrl <= 60
            && self.down <= DOWN_TICKS
            && self.coyote <= COYOTE
            && self.buffer <= BUFFER
            && (0..1024).contains(&self.ang)
            && (128..=256).contains(&self.zoom)
            && self.loose.len() <= SCATTER_MAX as usize
            && self.loose.iter().all(|l| l.age <= LOOSE_LIFE)
            && self.lock.is_none_or(|l| l.ticks <= LOCK_TICKS)
            && self.lunge <= LUNGE_TICKS
            && self.freeze <= 8
            && self.shake <= 16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    None,
    Shielded,
    Scattered(u32),
    Down,
}

fn toward_zero(v: i32, by: i32) -> i32 {
    if v > 0 {
        (v - by).max(0)
    } else {
        (v + by).min(0)
    }
}

pub enum Leave {
    Stay,
    End,
    Wall,
}

pub fn advance(t: &Terrain, path: &mut usize, s: &mut i32, d: i32) -> Leave {
    let mut left = d;
    for _ in 0..4 {
        let p = &t.paths[*path];
        let target = *s + left;
        if left > 0 {
            if let Some(q) = p.portals.iter().find(|q| q.dir > 0 && q.s > *s && q.s <= target && d >= q.min_speed) {
                left = target - q.s;
                *path = q.to.path;
                *s = q.to.s;
                continue;
            }
            if target > p.len() {
                match p.next {
                    Some(l) => {
                        left = target - p.len();
                        *path = l.path;
                        *s = l.s;
                        continue;
                    }
                    None => {
                        *s = p.len();
                        return Leave::End;
                    }
                }
            }
            *s = target;
            return Leave::Stay;
        } else if left < 0 {
            if let Some(q) = p.portals.iter().find(|q| q.dir < 0 && q.s < *s && q.s >= target && -d >= q.min_speed) {
                left = target - q.s;
                *path = q.to.path;
                *s = q.to.s;
                continue;
            }
            if target < 0 {
                match p.prev {
                    Some(l) => {
                        left = target;
                        *path = l.path;
                        *s = l.s;
                        continue;
                    }
                    None => {
                        *s = 0;
                        return Leave::Wall;
                    }
                }
            }
            *s = target;
            return Leave::Stay;
        } else {
            return Leave::Stay;
        }
    }
    Leave::Stay
}
