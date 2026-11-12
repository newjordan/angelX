//! The loud little moves of the race, drawn in the course's own space: wind
//! streaming past a fast ride, the dust and sparks a wheel throws, debris
//! flying in true 3D off a wreck, the pop of a gem or a ring, the burst a
//! griffin leaves. Each is a pure function of the race (its tick, its
//! riders, the records `show` keeps) and the lens it is seen through, so a
//! friend's mirror and a browser guest draw the same pixels.
//!
//! Two layers come out: `back` goes down under the knights (wind, dust,
//! rings on the floor) and `front` over them (sparks, chips, feathers,
//! bursts, paper). Both are transparent pictures stamped by the caller.
use super::super::pfx::{self, Mote, Pen, Ramp, Shape, ramps};
use crate::render::overworld::ink::{Img, bayer, hash};
use crate::render::world3d::raster::{self, View3};
use crate::rules::juice::Weight;
use crate::rules::race3d::{self as r3, Course, Race3d, Rider, Wreck, show::{Fx, Kind}};
use std::f32::consts::TAU;

/// The most motes a layer paints in a frame, however much happens at once.
const BUDGET: u32 = 1300;
/// Timber that stays readable on black paper as it cools, and cut stone.
const WOOD: Ramp = Ramp(['t', 'O', 'o', 'R', 'r', 'P']);
const STONE: Ramp = Ramp(['V', 'v', 'U', 'u', 'S', 'x']);

/// What the lens sees through: the camera, the canvas, and the course.
pub(super) struct Lens<'a> {
    pub view: &'a View3,
    pub w: i32,
    pub h: i32,
    pub race: &'a Race3d,
    pub seat: Option<u32>,
    pub track: &'a r3::Track,
}

impl Lens<'_> {
    /// A course point on the canvas: `(x, y, depth)`.
    pub fn at(&self, p: [f32; 3]) -> Option<(f32, f32, f32)> {
        raster::project_point(self.view, self.w as usize, self.h as usize, super::world(self.track, p[0], p[1], p[2]))
    }
    /// How many dots a course unit is at `depth`.
    pub fn per_unit(&self, depth: f32) -> f32 {
        raster::dots_tall(self.view, self.w as usize, self.h as usize, 1.0, depth)
    }
    fn me(&self) -> Option<&Rider> {
        self.seat.and_then(|id| self.race.riders.get(&id))
    }
}

/// The top pace a course's ride is judged against, units a second.
pub(super) fn top_pace(course: Course) -> f32 {
    match course {
        Course::Shaft => 11.0,
        Course::Cart => 11.0,
        Course::Griffin => 15.0,
        Course::Touge => 20.0,
    }
}

/// How fast a ride goes, 0 (crawling) to 1 (flat out).
pub(super) fn pace(course: Course, r: &Rider) -> f32 {
    ((r.vs.abs() - 3.0) / (top_pace(course) - 3.0)).clamp(0.0, 1.0)
}

// ─── things thrown ──────────────────────────────────────────────────────────

/// A handful of pieces thrown from one point of the course, flying on the
/// course's own axes: along, across and up, in units a second.
#[derive(Clone, Copy)]
struct Throw {
    seed: u32,
    n: u32,
    at: [f32; 3],
    /// The pace along the course the pieces keep (negative: thrown back).
    carry: f32,
    /// Speed away from the point, and up, each between the bounds.
    out: (f32, f32),
    up: (f32, f32),
    /// Pull down, units a second a second.
    g: f32,
    /// Ticks each lives.
    life: (f32, f32),
    shape: Shape,
    ramp: Ramp,
    size: u8,
    /// Lowest a piece falls to: a floor it comes to rest on.
    floor: Option<f32>,
    /// A solid piece this many course units across, drawn as a block that
    /// grows as it comes near the lens (0: a mote of the kit's).
    chunk: f32,
}

impl Throw {
    fn new(seed: u32, n: u32, at: [f32; 3], shape: Shape, ramp: Ramp) -> Throw {
        Throw {
            seed,
            n,
            at,
            carry: 0.0,
            out: (2.0, 5.0),
            up: (1.0, 4.0),
            g: 14.0,
            life: (12.0, 24.0),
            shape,
            ramp,
            size: 1,
            floor: Some(0.0),
            chunk: 0.0,
        }
    }
    fn chunk(mut self, units: f32) -> Throw {
        self.chunk = units;
        self
    }
    fn carry(mut self, c: f32) -> Throw {
        self.carry = c;
        self
    }
    fn out(mut self, a: f32, b: f32) -> Throw {
        self.out = (a, b);
        self
    }
    fn up(mut self, a: f32, b: f32) -> Throw {
        self.up = (a, b);
        self
    }
    fn g(mut self, g: f32) -> Throw {
        self.g = g;
        self
    }
    fn life(mut self, a: f32, b: f32) -> Throw {
        self.life = (a, b);
        self
    }
    fn size(mut self, s: u8) -> Throw {
        self.size = s;
        self
    }
    fn floor(mut self, f: Option<f32>) -> Throw {
        self.floor = f;
        self
    }
}

/// Every piece of `th`, `age` ticks after the throw.
fn throw(pen: &mut Pen, lens: &Lens, th: &Throw, age: f32, tick: u64) {
    for i in 0..th.n.min(pfx::MOST_MOTES) {
        let u = |salt: u32| pfx::unit(i as i32, th.seed as i32, salt);
        let life = th.life.0 + (th.life.1 - th.life.0) * u(1);
        if age >= life {
            continue;
        }
        let a = u(2) * TAU;
        let sp = th.out.0 + (th.out.1 - th.out.0) * u(3);
        let up = th.up.0 + (th.up.1 - th.up.0) * u(4);
        let v = [th.carry + a.cos() * sp * 0.7, a.sin() * sp, up];
        let pos = |age: f32| {
            let t = age / 30.0;
            let mut p = [th.at[0] + v[0] * t, th.at[1] + v[1] * t, th.at[2] + v[2] * t - 0.5 * th.g * t * t];
            if let Some(f) = th.floor {
                p[2] = p[2].max(f);
            }
            p
        };
        let (Some(a0), Some(a1)) = (lens.at(pos(age)), lens.at(pos(age + 1.0))) else { continue };
        if th.chunk > 0.0 {
            // A solid piece: a block of two inks, turning end over end.
            let side = (lens.per_unit(a0.2) * th.chunk).round().clamp(2.0, 7.0) as i32;
            let (bw, bh) = if (tick as u32 / 3 + i) % 2 == 0 { (side, (side * 2 / 3).max(2)) } else { ((side * 2 / 3).max(2), side) };
            let k = age / life;
            let (x0, y0) = (a0.0.round() as i32 - bw / 2, a0.1.round() as i32 - bh / 2);
            for dy in 0..bh {
                for dx in 0..bw {
                    let lit = dx == 0 || dy == 0;
                    pen.put(x0 + dx, y0 + dy, th.ramp.at(k * 0.7 + if lit { 0.0 } else { 0.22 }));
                }
            }
            continue;
        }
        let m = Mote { i, at: (a0.0, a0.1), vel: (a1.0 - a0.0, a1.1 - a0.1), k: age / life, seed: hash(i as i32, th.seed as i32, 0x99) };
        pfx::mote(pen, &m, th.shape, th.ramp, th.size, tick);
    }
}

/// A ring flat on the floor round `c`, `r` units across, flattened as the
/// lens sees the floor.
fn floor_ring(pen: &mut Pen, lens: &Lens, c: [f32; 3], r: f32, thick: f32, ramp: Ramp, fade: f32) {
    let Some(mid) = lens.at(c) else { return };
    let (Some(side), Some(far)) = (lens.at([c[0], c[1] + r, c[2]]), lens.at([c[0] + r, c[1], c[2]])) else { return };
    let rx = (side.0 - mid.0).abs().max(1.0);
    let ry = (far.1 - mid.1).abs();
    pen.shock((mid.0, mid.1), rx, thick, (ry / rx).clamp(0.12, 1.0), ramp, fade);
}

/// A ring standing across the course round `c` (a ring flown through, a
/// burst): `r` units, drawn as the lens sees its edge.
fn loop_ring(pen: &mut Pen, lens: &Lens, c: [f32; 3], r: f32, ramp: Ramp, k: f32) {
    let pts: Vec<(f32, f32)> = (0..20)
        .filter_map(|j| {
            let a = j as f32 / 20.0 * TAU;
            lens.at([c[0], c[1] + a.cos() * r, c[2] + a.sin() * r]).map(|p| (p.0, p.1))
        })
        .collect();
    if pts.len() < 20 {
        return;
    }
    for j in 0..20 {
        let (a, b) = (pts[j], pts[(j + 1) % 20]);
        let ch = ramp.dithered(0.1 + k * 0.8, j as i32, (k * 40.0) as i32);
        pen.line(a, b, ch);
    }
}

// ─── the two layers ─────────────────────────────────────────────────────────

/// The race's world effects for the lens's seat, as two transparent layers
/// (`None` for a layer nothing was painted on).
pub(super) fn layers(lens: &Lens, surge: f32) -> (Option<Img>, Option<Img>) {
    let mut back = Img::new(lens.w, lens.h);
    let mut front = Img::new(lens.w, lens.h);
    let race = lens.race;
    let now = race.t;
    let (back_used, front_used) = {
        let mut pb = Pen::new(&mut back, BUDGET);
        let mut pf = Pen::new(&mut front, BUDGET);
        if let Some(me) = lens.me() {
            wind(&mut pb, lens, me, surge);
            tow(&mut pb, lens, me);
        }
        for (&id, r) in &race.riders {
            if super::build::hidden(race, lens.seat, id) {
                continue;
            }
            trail(&mut pb, &mut pf, lens, id, r);
        }
        for f in race.show.fx.iter().filter(|f| f.age(now) < crate::rules::race3d::show::LIFE) {
            event(&mut pb, &mut pf, lens, f);
        }
        (pb.left() < BUDGET, pf.left() < BUDGET)
    };
    (back_used.then_some(back), front_used.then_some(front))
}

// ─── wind and slipstream ────────────────────────────────────────────────────

/// Where the course vanishes on the canvas.
fn vanishing(lens: &Lens, me: &Rider) -> (f32, f32) {
    lens.at([me.s + 60.0, me.x * 0.5, me.z]).map_or((lens.w as f32 / 2.0, lens.h as f32 * 0.42), |p| {
        (p.0.clamp(lens.w as f32 * 0.3, lens.w as f32 * 0.7), p.1.clamp(lens.h as f32 * 0.3, lens.h as f32 * 0.55))
    })
}

/// Streaks of wind out of the vanishing point, more and longer with pace; a
/// start or a burst drives a surge of them.
fn wind(pen: &mut Pen, lens: &Lens, me: &Rider, surge: f32) {
    let course = lens.race.course;
    let boosted = me.burst > 0 || me.drift.boost > 0;
    let k = if me.crash > 0 || me.finish.is_some() { 0.0 } else { pace(course, me) };
    let power = (k * 0.8 + surge + if boosted { 0.5 } else { 0.0 }).min(1.4);
    if power < 0.08 {
        return;
    }
    let now = lens.race.t as f32;
    let (cx, cy) = vanishing(lens, me);
    let (w, h) = (lens.w as f32, lens.h as f32);
    let n = (4.0 + 24.0 * power) as u32;
    let ramp = if boosted { ramps::GOLD.cooler(2) } else { ramps::SMOKE };
    for i in 0..n {
        let life = 10.0;
        let phase = pfx::unit(i as i32, 0x51ee, 1) * life;
        let lap = ((now + phase) / life).floor() as i32;
        let age = (now + phase) % life;
        let u = |salt: u32| pfx::unit(i as i32, lap ^ 0x77, salt);
        let a = u(2) * TAU;
        let (ux, uy) = (a.cos(), a.sin());
        // How far the ray runs before it leaves the canvas.
        let to_x = if ux > 0.01 { (w - cx) / ux } else if ux < -0.01 { -cx / ux } else { f32::MAX };
        let to_y = if uy > 0.01 { (h - cy) / uy } else if uy < -0.01 { -cy / uy } else { f32::MAX };
        let edge = to_x.min(to_y);
        let inner = 0.34 + 0.2 * u(3);
        let f = (age / life).powf(1.4);
        let r = (inner + (1.0 - inner) * f) * edge;
        let (hx, hy) = (cx + ux * r, cy + uy * r);
        // Keep the middle of the road and the knight's own spot clear.
        if (hx - cx).abs() < 46.0 && hy > cy + 2.0 {
            continue;
        }
        let len = (2.0 + (5.0 + 9.0 * power) * f).round();
        let k = (0.42 - 0.3 * power - 0.1 * f).max(0.02);
        pen.streak((hx, hy), (ux, uy), len, ramp, k);
        if power > 0.55 && f > 0.3 {
            // A fast ride's lines are two dots thick.
            pen.streak((hx + uy.abs().max(0.4), hy - ux.abs().max(0.4) * ux.signum()), (ux, uy), (len * 0.7).round(), ramp, k + 0.1);
        }
    }
}

/// Chevrons of wind streaming off the rival ahead whose wake this ride is
/// in: the picture of a tow (it moves nothing).
fn tow(pen: &mut Pen, lens: &Lens, me: &Rider) {
    let race = lens.race;
    if me.crash > 0 || me.finish.is_some() || race.titled() {
        return;
    }
    let Some(seat) = lens.seat else { return };
    let now = race.t as f32;
    for (&id, r) in &race.riders {
        if id == seat || r.crash > 0 || r.finish.is_some() {
            continue;
        }
        let gap = r.s - me.s;
        let tall = if race.course == Course::Griffin { (r.z - me.z).abs() } else { 0.0 };
        if !(2.5..=11.0).contains(&gap) || (r.x - me.x).abs() > 1.5 || tall > 1.3 {
            continue;
        }
        let k = 1.0 - (gap - 2.5) / 8.5;
        let lift = if race.course == Course::Shaft { 0.5 } else { 0.35 };
        for j in 0..6 {
            let side = (j as f32 - 2.5) * 0.26;
            let high = lift + (j % 3) as f32 * 0.16;
            let phase = ((now * 0.7 + j as f32 * 1.9) % 5.0) / 5.0;
            let head = r.s - 0.5 - phase * (gap - 1.0);
            let a = lens.at([head, r.x + side, r.z + high]);
            let b = lens.at([head - 0.9, r.x + side, r.z + high]);
            if let (Some(a), Some(b)) = (a, b) {
                let fade = (1.0 - phase) * k;
                if fade > 0.12 {
                    pen.line((a.0, a.1), (b.0, b.1), ramps::FROST.at(0.1 + (1.0 - fade) * 0.5));
                }
            }
        }
    }
}

// ─── what a wheel throws ────────────────────────────────────────────────────

/// A puff of dust or spray that stays where it was kicked up.
fn puff(pen: &mut Pen, lens: &Lens, at: [f32; 3], age: f32, life: f32, i: u32, ramp: Ramp, size: u8, tick: u64) {
    let Some(p) = lens.at([at[0], at[1], at[2] + age * 0.012]) else { return };
    let m = Mote { i, at: (p.0, p.1), vel: (0.0, -0.2), k: (age / life).clamp(0.0, 0.99), seed: hash(i as i32, at[0] as i32, 0x3d) };
    pfx::mote(pen, &m, Shape::Smoke, ramp, size, tick);
}

/// What a ride leaves behind it: dust off a ball's floor, sparks off a
/// cart's wheels and a lodestone's rail, spray in the sluice, embers behind
/// a burst, sparks and flame behind the Touge's slide.
fn trail(back: &mut Pen, front: &mut Pen, lens: &Lens, id: u32, r: &Rider) {
    let race = lens.race;
    let course = race.course;
    let now = race.t;
    let tick = u64::from(now);
    let revving = race.titled() && now + 45 >= r3::TITLE;
    let pace_k = if revving { (now + 45 - r3::TITLE) as f32 / 45.0 * 0.7 } else { pace(course, r) };
    let live = r.crash == 0 && r.finish.is_none();
    if !live || pace_k < 0.12 {
        return;
    }
    let v = if revving { 5.0 * pace_k } else { r.vs };
    let seed = id.wrapping_mul(0x9e37);
    let ground = race.course != Course::Griffin;
    // Dust: five puffs a ride streams, each where it was kicked up.
    if ground && (r.z <= 0.05 || course == Course::Touge) {
        let at = r3::track(course).at(r.s);
        let (ramp, size) = match at.ground {
            r3::Ground::Sluice => (ramps::WATER, 1),
            r3::Ground::Collapse => (ramps::EARTH, 2),
            _ => (ramps::SMOKE, 1),
        };
        let count = (2.0 + 3.0 * pace_k) as u32;
        for j in 0..count {
            let age = j as f32 * 3.0 + (now % 3) as f32;
            let life = 14.0;
            if age >= life {
                continue;
            }
            let wob = (pfx::unit(j as i32, seed as i32 ^ (now / 3) as i32, 5) - 0.5) * 0.5;
            let back_s = r.s - v * age / 30.0 - 0.5;
            puff(back, lens, [back_s, r.x + wob - r.vx * age / 60.0, 0.12], age, life, j, ramp, size, tick);
        }
    }
    match course {
        Course::Cart if r.z <= 0.05 && pace_k > 0.4 => {
            // Sparks off the rear wheels, flicking back along the rails.
            for side in [-0.5f32, 0.5] {
                let seed = seed ^ u32::from(side > 0.0);
                let age = (now % 6) as f32;
                let th = Throw::new(seed.wrapping_add(now / 6), 3, [r.s - 0.7, r.x + side, 0.12], Shape::Spark, ramps::FIRE)
                    .carry(v * 0.3)
                    .out(0.5, 1.8)
                    .up(0.8, 2.6)
                    .g(12.0)
                    .life(5.0, 9.0);
                throw(front, lens, &th, age, tick);
            }
        }
        Course::Shaft => {
            let g = r3::track(course).at(r.s).ground;
            if g == r3::Ground::Lodestone && r.z <= 0.05 && (r.x - 1.3).abs() < 0.45 && pace_k > 0.1 {
                let th = Throw::new(seed.wrapping_add(now / 4), 4, [r.s - 0.3, r.x, 0.1], Shape::Spark, ramps::STORM)
                    .carry(v * 0.2)
                    .out(0.6, 2.2)
                    .up(1.0, 3.2)
                    .g(10.0)
                    .life(6.0, 12.0);
                throw(front, lens, &th, (now % 4) as f32 * 1.5, tick);
            }
        }
        Course::Griffin if r.burst > 0 => {
            // Embers streaming off a burst.
            for j in 0..6u32 {
                let age = j as f32 * 2.0 + (now % 2) as f32;
                let back_s = r.s - r.vs * age / 30.0;
                let wob = (pfx::unit(j as i32, seed as i32, 7) - 0.5) * 0.6;
                let Some(p) = lens.at([back_s, r.x + wob, r.z + 0.2 + wob * 0.4]) else { continue };
                let m = Mote { i: j, at: (p.0, p.1), vel: (0.0, 0.0), k: (age / 14.0).min(0.99), seed: j };
                pfx::mote(front, &m, Shape::Ember, ramps::EMBER, 2, tick);
            }
        }
        Course::Touge => {
            let d = &r.drift;
            let stage = if d.boost > 0 { usize::from(d.boost_stage) } else { d.stage() };
            let ramp = [ramps::SMOKE, ramps::STORM, ramps::FIRE, ramps::ARCANE][stage.min(3)];
            if d.sliding && pace_k > 0.15 {
                for side in [-0.6f32, 0.6] {
                    let th = Throw::new(seed ^ (now / 2) ^ u32::from(side > 0.0), 4, [r.s - 0.8, r.x + side, 0.12], Shape::Spark, ramp)
                        .carry(v * 0.35)
                        .out(0.8, 3.2)
                        .up(1.0, 3.4)
                        .g(11.0)
                        .life(6.0, 14.0);
                    throw(front, lens, &th, (now % 2) as f32 * 2.0, tick);
                }
            }
            if d.boost > 0 {
                for j in 0..6u32 {
                    let age = j as f32 * 2.0 + (now % 2) as f32;
                    let Some(p) = lens.at([r.s - 1.2 - v * age / 30.0, r.x, 0.5 + age * 0.015]) else { continue };
                    let m = Mote { i: j, at: (p.0, p.1), vel: (0.0, 0.0), k: (age / 14.0).min(0.99), seed: j };
                    pfx::mote(front, &m, Shape::Ember, ramps::EMBER, 2, tick);
                }
            }
        }
        _ => {}
    }
}

// ─── what happened ──────────────────────────────────────────────────────────

/// How many pieces a blow of `weight` throws.
fn pieces(weight: Weight) -> u32 {
    match weight {
        Weight::Graze => 5,
        Weight::Solid => 9,
        Weight::Crushing => 15,
        Weight::Kill | Weight::Finale => 22,
    }
}

fn event(back: &mut Pen, front: &mut Pen, lens: &Lens, f: &Fx) {
    let race = lens.race;
    let age = f.age(race.t) as f32;
    let tick = u64::from(race.t);
    let seed = f.born.wrapping_mul(0x2545) ^ u32::from(f.seat) << 20 ^ u32::from(f.other) << 12;
    let ground = [f.at[0], f.at[1], 0.0];
    match f.kind {
        Kind::Wreck => wreck(back, front, lens, f, age, seed, tick),
        Kind::Bump => {
            let th = Throw::new(seed, 8, [f.at[0], f.at[1], f.at[2] + 0.5], Shape::Spark, ramps::FIRE).out(1.5, 4.5).up(0.5, 3.0).life(7.0, 14.0);
            throw(front, lens, &th, age, tick);
            if age < 8.0 {
                floor_ring(back, lens, ground, 0.4 + age * 0.22, 1.4, ramps::BONE, age / 8.0);
            }
        }
        Kind::Scrape => {
            let th = Throw::new(seed, 7, [f.at[0] - 0.2, f.at[1] + f.at[1].signum() * 0.4, 0.3], Shape::Spark, ramps::FIRE)
                .carry(-2.0)
                .out(1.0, 3.5)
                .up(0.8, 3.0)
                .life(6.0, 12.0);
            let th = if race.course == Course::Griffin { Throw { shape: Shape::Petal, ramp: ramps::BONE, floor: None, ..th } } else { th };
            throw(front, lens, &th, age, tick);
        }
        Kind::Hop => {
            if age < 10.0 {
                for j in 0..3u32 {
                    puff(back, lens, [f.at[0] - 0.3 - j as f32 * 0.25, f.at[1] + (j as f32 - 1.0) * 0.3, 0.1], age, 10.0, j, ramps::SMOKE, 1, tick);
                }
            }
        }
        Kind::Land => {
            let hard = (f.n as f32 / 10.0 / 6.0).clamp(0.3, 1.0);
            if age < 12.0 {
                floor_ring(back, lens, ground, 0.3 + age * 0.2 * hard.max(0.5), 1.6, ramps::SMOKE, age / 12.0);
            }
            let th = Throw::new(seed, (4.0 + 6.0 * hard) as u32, [f.at[0], f.at[1], 0.15], Shape::Smoke, ramps::SMOKE)
                .out(0.6, 2.2)
                .up(0.2, 1.0)
                .g(-0.5)
                .life(10.0, 18.0)
                .size(1);
            throw(back, lens, &th, age, tick);
            if race.course == Course::Cart {
                let th = Throw::new(seed ^ 9, 5, [f.at[0] - 0.3, f.at[1], 0.15], Shape::Spark, ramps::FIRE).out(0.8, 2.6).up(1.0, 3.0).life(6.0, 11.0);
                throw(front, lens, &th, age, tick);
            }
        }
        Kind::Pick => pick(front, lens, f, age, seed, tick),
        Kind::Burst => {
            let c = [f.at[0], f.at[1], f.at[2] + 0.35];
            if age < 16.0 {
                loop_ring(front, lens, c, 0.8 + age * 0.1, if f.n > 0 { ramps::FIRE } else { ramps::GOLD }, age / 16.0);
                if age < 7.0 {
                    loop_ring(front, lens, c, 0.4 + age * 0.17, ramps::BONE, age / 7.0);
                }
            }
            let th = Throw::new(seed, 10, c, Shape::Spark, ramps::GOLD).carry(-3.0).out(2.0, 5.5).up(-1.0, 2.0).g(2.0).life(8.0, 16.0).floor(None);
            throw(front, lens, &th, age, tick);
        }
        Kind::Kick => {
            if age < 12.0 {
                loop_ring(front, lens, [f.at[0], f.at[1], 0.6], 0.7 + age * 0.12, ramps::GOLD, age / 12.0);
            }
        }
        Kind::Roll => {
            let th = Throw::new(seed, 8, [f.at[0], f.at[1], f.at[2]], Shape::Petal, ramps::BONE).carry(-2.0).out(1.5, 3.5).up(-1.5, 1.5).g(1.0).life(10.0, 18.0).floor(None);
            throw(front, lens, &th, age, tick);
        }
        Kind::Ruffle => {
            let th = Throw::new(seed, 12, [f.at[0], f.at[1], f.at[2] + 0.4], Shape::Petal, ramps::BONE).out(1.0, 3.8).up(-1.0, 2.5).g(3.0).life(14.0, 26.0).floor(None).size(2);
            throw(front, lens, &th, age, tick);
            if age < 5.0 {
                pfx::rays(front, project(lens, f.at, 0.4), 8, 12.0, 0.8, 0.3, ramps::BONE, age / 5.0);
            }
        }
        Kind::Finish => {
            let own = lens.seat == Some(u32::from(f.seat));
            let c = [f.at[0], f.at[1], f.at[2] + 0.6];
            let th = Throw::new(seed, if own { 22 } else { 10 }, c, Shape::Star, if f.n == 1 { ramps::GOLD } else { ramps::BONE })
                .carry(r3_pace(race, f) * 0.5)
                .out(1.0, 4.5)
                .up(1.5, 5.0)
                .g(9.0)
                .life(16.0, 34.0)
                .floor(None);
            throw(front, lens, &th, age, tick);
        }
        Kind::Pass | Kind::Lead | Kind::Near | Kind::Photo => {}
    }
}

fn r3_pace(race: &Race3d, f: &Fx) -> f32 {
    race.riders.get(&u32::from(f.seat)).map_or(0.0, |r| r.vs)
}

/// A course point on the canvas, as a bare pair (the centre if it is not
/// on screen).
fn project(lens: &Lens, at: [f32; 3], lift: f32) -> (f32, f32) {
    lens.at([at[0], at[1], at[2] + lift]).map_or((lens.w as f32 / 2.0, lens.h as f32 / 2.0), |p| (p.0, p.1))
}

/// A wreck: a flash, a ring, and the wreck's own debris in the air.
fn wreck(back: &mut Pen, front: &mut Pen, lens: &Lens, f: &Fx, age: f32, seed: u32, tick: u64) {
    let race = lens.race;
    let n = pieces(f.weight);
    let pace = f.n as f32 / 10.0;
    let c = [f.at[0], f.at[1], f.at[2].max(0.0) + 0.45];
    let ground = [f.at[0], f.at[1], 0.0];
    let hard = f.weight >= Weight::Crushing;
    let big = f.weight >= Weight::Kill;
    // The instant of contact.
    if age < 6.0 && hard {
        pfx::rays(front, project(lens, f.at, 0.5), 10, if big { 30.0 } else { 20.0 }, 0.8, seed as f32, ramps::GOLD, age / 6.0);
    }
    if age < 14.0 {
        floor_ring(back, lens, ground, 0.4 + age * if hard { 0.16 } else { 0.1 }, 1.8, ramps::SMOKE, age / 14.0);
    }
    let carry = pace * 0.4;
    let size = if big { 3 } else { 2 };
    // Dust always, in a cloud that drifts up and thins.
    let dust = Throw::new(seed ^ 1, n + 4, [f.at[0], f.at[1], 0.2], Shape::Smoke, ramps::SMOKE)
        .carry(carry * 0.5)
        .out(0.5, 2.8)
        .up(0.3, 1.8)
        .g(-0.6)
        .life(16.0, 34.0)
        .size(size);
    if race.course != Course::Griffin {
        throw(back, lens, &dust, age, tick);
    }
    let wood = |extra: u32| {
        Throw::new(seed ^ 2, n + extra, c, Shape::Shard, WOOD).carry(carry).out(2.0, 6.5).up(2.0, 7.0).life(18.0, 34.0).size(3).chunk(0.3)
    };
    let sparks = |col: Ramp| Throw::new(seed ^ 3, n, c, Shape::Spark, col).carry(carry).out(2.5, 7.0).up(1.5, 6.0).g(12.0).life(10.0, 20.0).size(2);
    match f.wreck {
        Wreck::Bonk => {
            throw(front, lens, &sparks(ramps::FIRE), age, tick);
            throw(front, lens, &wood(0).up(2.5, 7.0), age, tick);
            // Stars about a bonked head.
            let stars = Throw::new(seed ^ 4, 5, [f.at[0], f.at[1], 1.4], Shape::Star, ramps::GOLD).out(0.3, 1.0).up(0.0, 0.8).g(0.0).life(16.0, 28.0).floor(None).size(2);
            throw(front, lens, &stars, age, tick);
        }
        Wreck::Buffer => {
            throw(front, lens, &wood(6), age, tick);
            throw(front, lens, &sparks(ramps::FIRE), age, tick);
        }
        Wreck::Timber => {
            throw(front, lens, &wood(8).out(2.5, 7.0), age, tick);
            throw(front, lens, &sparks(ramps::GOLD), age, tick);
        }
        Wreck::Gap => {
            // Planks and chips falling through the broken rail.
            let falling = Throw::new(seed ^ 5, n, c, Shape::Shard, WOOD).carry(carry * 0.3).out(0.5, 2.5).up(0.0, 2.5).g(22.0).life(18.0, 32.0).size(3).floor(None).chunk(0.32);
            throw(front, lens, &falling, age, tick);
        }
        Wreck::Fall => {
            let falling = Throw::new(seed ^ 6, n / 2 + 3, c, Shape::Shard, WOOD).carry(carry * 0.3).out(0.3, 1.8).up(0.0, 2.5).g(20.0).life(20.0, 36.0).size(2).floor(None).chunk(0.26);
            throw(front, lens, &falling, age, tick);
            // The wail: a thin streak of dust falling after the ball.
            let streak = Throw::new(seed ^ 7, 6, c, Shape::Smoke, ramps::SMOKE).out(0.1, 0.5).up(-2.0, 0.0).g(10.0).life(14.0, 28.0).floor(None).size(2);
            throw(back, lens, &streak, age, tick);
        }
        Wreck::Tumble => {
            let feathers = Throw::new(seed ^ 8, n + 8, c, Shape::Petal, ramps::BONE).carry(carry * 0.6).out(1.0, 5.0).up(-2.0, 3.5).g(2.0).life(20.0, 44.0).floor(None).size(2);
            throw(front, lens, &feathers, age, tick);
            let chips = Throw::new(seed ^ 9, n, c, Shape::Shard, STONE).carry(carry).out(2.0, 6.5).up(-1.0, 5.0).g(14.0).life(14.0, 30.0).floor(None).size(3).chunk(0.3);
            throw(front, lens, &chips, age, tick);
            let puffs = Throw::new(seed ^ 10, 8, c, Shape::Smoke, ramps::SMOKE).carry(carry * 0.5).out(0.5, 2.5).up(-0.5, 1.5).g(-0.4).life(14.0, 28.0).floor(None).size(2);
            throw(front, lens, &puffs, age, tick);
        }
        Wreck::SpinOut => {
            throw(front, lens, &sparks(ramps::FIRE), age, tick);
        }
        Wreck::None => {}
    }
}

/// A pickup: a flash and a handful of sparks where it was, in its own ink.
fn pick(front: &mut Pen, lens: &Lens, f: &Fx, age: f32, seed: u32, tick: u64) {
    let course = lens.race.course;
    let c = [f.at[0], f.at[1], f.at[2]];
    match f.n {
        2 => {
            // A gargoyle smashed: stone, embers and a puff.
            let chips = Throw::new(seed, 12, c, Shape::Shard, STONE).out(1.5, 5.5).up(-2.0, 3.5).g(10.0).life(14.0, 28.0).floor(None).size(3).chunk(0.24);
            throw(front, lens, &chips, age, tick);
            let embers = Throw::new(seed ^ 1, 8, c, Shape::Spark, ramps::EMBER).out(2.0, 6.0).up(-1.5, 3.0).g(4.0).life(8.0, 16.0).floor(None);
            throw(front, lens, &embers, age, tick);
            if age < 10.0 {
                pfx::rays(front, project(lens, c, 0.0), 8, 14.0, 0.8, 0.2, ramps::EMBER, age / 10.0);
            }
        }
        1 => {
            // A ring flown: a gold ring that swells and goes.
            if age < 14.0 {
                loop_ring(front, lens, c, 1.1 + age * 0.12, ramps::GOLD, age / 14.0);
            }
            let sparks = Throw::new(seed, 10, c, Shape::Spark, ramps::GOLD).out(1.0, 4.0).up(-2.0, 2.5).g(2.0).life(8.0, 16.0).floor(None);
            throw(front, lens, &sparks, age, tick);
        }
        _ => {
            let ramp = if course == Course::Shaft { ramps::FROST } else { ramps::GOLD };
            if age < 14.0 {
                front.twinkle(project(lens, c, 0.0), 4.0, age / 14.0, ramp);
            }
            let sparks = Throw::new(seed, 7, c, Shape::Spark, ramp).out(0.8, 3.0).up(0.5, 3.2).g(9.0).life(8.0, 14.0).floor(None);
            throw(front, lens, &sparks, age, tick);
        }
    }
}

/// A sprite flash for the knight `id` right now: the white impact frame's
/// ink, if a blow landed on them within its flash.
pub(super) fn struck(race: &Race3d, id: u32) -> Option<(Weight, u32)> {
    let now = race.t;
    race.show
        .fx
        .iter()
        .filter(|f| u32::from(f.seat) == id && matches!(f.kind, Kind::Wreck | Kind::Ruffle) || f.kind == Kind::Bump && (u32::from(f.seat) == id || u32::from(f.other) == id))
        .filter(|f| f.age(now) < 8)
        .map(|f| (f.weight, f.age(now)))
        .min_by_key(|&(_, a)| a)
}

/// A thin dither of ink over a box for one tick: the pale of a big blow, or
/// the flash of a photograph. Only the cells of the ordered dither that are
/// lit are visited, so the cost goes with the density and not the canvas.
pub(super) fn pale(im: &mut Img, rect: (i32, i32, i32, i32), density: f32, ink: char) {
    for cy in 0..4 {
        for cx in 0..4 {
            if bayer(cx, cy) >= density {
                continue;
            }
            let mut y = rect.1 + (cy - rect.1).rem_euclid(4);
            while y < rect.1 + rect.3 {
                let mut x = rect.0 + (cx - rect.0).rem_euclid(4);
                while x < rect.0 + rect.2 {
                    im.put(x, y, ink);
                    x += 4;
                }
                y += 4;
            }
        }
    }
}
