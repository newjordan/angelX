//! The feel kit: what makes the race read as 600 km/h a hand's breadth off the road.
//!
//! Two halves. The **mood** is the camera's memory: springs for the lag on a steer and a bend,
//! the roll into it, the boost's kick, a landing's dip and bounce, the shakes and the rumble
//! (`camera` reads it). The **draw** is a small pixel kit painted over the raster before the
//! instruments: exhaust plumes, ignition bursts, ground wash and rooster tails, wall sparks,
//! landing dust, speed streaks and radial lines, the boost vignette, a whoosh when a rival is
//! passed close. Every mark is an ink of the realm, dithered on the 4x4 Bayer, and tested
//! against the raster's depth, so it is crisp at 384x224 and sits behind a crest.
//!
//! Everything keys on the race's tick and the cars' state. A seat's memory starts over from
//! the car whenever the clock goes back, the car jumps or the course changes, so a late join,
//! a rematch or a replay frames the same race. Particles are few (`CAP`) and live in the road's
//! own frame (`s` along it, `y` across, `z` up), so a bend carries them round with it.
//!
//! Other lanes use it through `emit` (a puff, a spark, a flame, a smoke from anywhere) and
//! `nozzles` (where a car's engines breathe); see `docs`'s `FX-API.md` in the proof folder.
use super::*;
use super::camera::{tune, Lens, H, W};
use super::damage_fx;
use super::pods::{look_of, pod, throats};
use crate::ink::{self, bayer, Rgb};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const DT: f32 = 1. / 30.;

// Heat shimmer over Emberfall's lava and hot straights (the world lane's): see `shimmer.rs`.
#[path = "shimmer.rs"]
mod shimmer;

/// Test builds can switch stages of `draw` off by bit, to cost them one at a time.
#[cfg(test)]
thread_local! { pub static SKIP: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
fn skipped(bit: u32) -> bool { SKIP.with(|s| s.get() & bit != 0) }
#[cfg(not(test))]
#[inline]
fn skipped(_bit: u32) -> bool { false }
/// Particles a seat keeps at once.
pub const CAP: usize = 240;
/// The lowest row the marks may reach: the hint bar below it is the instruments'.
const FLOOR: i32 = 215;

// ---------------------------------------------------------------- small helpers

fn hash32(mut x: u32) -> u32 {
    x ^= x >> 16; x = x.wrapping_mul(0x7feb_352d); x ^= x >> 15; x = x.wrapping_mul(0x846c_a68b); x ^ (x >> 16)
}
/// 0..1 from a seed.
fn rnd(seed: u32) -> f32 { (hash32(seed) >> 8) as f32 / 16_777_216. }
/// -1..1 from a seed.
fn rsg(seed: u32) -> f32 { rnd(seed) * 2. - 1. }

fn ink_rgb(ch: char) -> Rgb { ink::ink(ch).unwrap_or([255, 0, 255]) }
fn ramp(r: &[char], t: f32) -> char { r[((t.clamp(0., 0.999)) * r.len() as f32) as usize] }

/// Fire from white-hot to ash.
const FIRE: [char; 9] = ['w', '9', '6', '4', 'O', 'R', '7', '8', 'Z'];
/// A spark's life: white, gold, orange-red.
const SPARK: [char; 7] = ['9', '6', '5', '4', 'O', 'R', '7'];
/// The shock of a boost: white to teal.
const SHOCK: [char; 5] = ['w', '3', '2', '1', '0'];
/// Wind: pale stone to slate.
const WIND: [char; 5] = ['W', 'V', 'v', 'U', 'u'];
/// Smoke from a hurt engine.
const SMOKE: [char; 5] = ['X', 'g', 'j', 'G', 'J'];
/// Soot to ash: the black smoke off a hurt engine, going pale as it climbs and spreads (pale enough
/// at the top to show on a dark road, black enough at the nozzle to show on a bright one).
const SOOT: [char; 6] = ['K', 'X', 'j', 'G', 'h', 'i'];
/// The pale steam off a blown pod.
const STEAM: [char; 4] = ['H', 'i', 'h', 'J'];

fn bank_slot() -> &'static ink::RgbMap<(u8, u8)> {
    static T: OnceLock<ink::RgbMap<(u8, u8)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = ink::RgbMap::default();
        for bank in 0..ink::BANKS {
            if bank == ink::SIGNAL_BANK { continue; }
            for (i, &v) in ink::bank_colours(bank).iter().enumerate() { m.insert(ink::rgb(v), (bank as u8, i as u8)); }
        }
        m
    })
}

/// `c` `by` steps up (or down, below zero) its own bank's ramp: a lighter or deeper step of the
/// same hue. A colour that is not on a ramp (black, a signal) stays as it is.
pub fn step(c: Rgb, by: i32) -> Rgb {
    match bank_slot().get(&c) {
        Some(&(b, i)) => {
            let cols = ink::bank_colours(b as usize);
            ink::rgb(cols[(i as i32 + by).clamp(0, cols.len() as i32 - 1) as usize])
        }
        None => c,
    }
}

/// `c` on the realm's plain inks, by a 16-step cube built once (the palette's nearest plain ink for each cell):
/// a see-through mark asks it for every dot, so it is a table read and not a walk down the palette.
fn snap_plain(c: [f32; 3]) -> Rgb {
    static CUBE: OnceLock<Vec<Rgb>> = OnceLock::new();
    let cube = CUBE.get_or_init(|| {
        let mut v = Vec::with_capacity(16 * 16 * 16);
        for r in 0..16 { for g in 0..16 { for b in 0..16 {
            let mid = |x: i32| (x * 16 + 8) as f32;
            v.push(ink::nearest_plain([mid(r), mid(g), mid(b)]));
        } } }
        v
    });
    let q = |x: f32| (x.clamp(0., 255.) as usize) >> 4;
    cube[(q(c[0]) << 8) | (q(c[1]) << 4) | q(c[2])]
}

// ---------------------------------------------------------------- the brush

/// The picture and its depth, and what marks may be set on them.
struct Brush<'a> {
    im: &'a mut Img,
    depth: &'a [f32],
    lens: Lens,
}

impl Brush<'_> {
    fn open(&self, x: i32, y: i32, z: f32) -> bool {
        x >= 0 && x < 384 && y >= 0 && y < FLOOR && self.depth.get((y * 384 + x) as usize).is_none_or(|&d| d + 0.6 >= z)
    }
    fn dot(&mut self, x: i32, y: i32, z: f32, c: Rgb) {
        if self.open(x, y, z) { self.im.set(x, y, c); }
    }
    /// A dot that is there `a` of the time, by the 4x4 dither.
    fn dot_a(&mut self, x: i32, y: i32, z: f32, c: Rgb, a: f32) {
        if a > bayer(x, y) { self.dot(x, y, z, c); }
    }
    /// A disc whose edge fades by dither: solid inside `0.65 r`, `a` of it past.
    fn disc(&mut self, cx: f32, cy: f32, r: f32, z: f32, c: Rgb, a: f32) {
        if r < 0.9 { self.dot_a(cx.round() as i32, cy.round() as i32, z, c, a.max(0.5)); return; }
        let r2 = r * r;
        for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d2 = dx * dx + dy * dy;
                if d2 > r2 { continue; }
                let edge = ((1. - d2 / r2) * 2.5).clamp(0., 1.);
                self.dot_a(x, y, z, c, a * edge);
            }
        }
    }
    /// A one-dot line, `a` of it by dither.
    fn seg(&mut self, (x0, y0): (f32, f32), (x1, y1): (f32, f32), z: f32, c: Rgb, a: f32) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).ceil().clamp(1., 400.) as i32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.dot_a((x0 + (x1 - x0) * t).round() as i32, (y0 + (y1 - y0) * t).round() as i32, z, c, a);
        }
    }
    /// A line whose colour and weight change along it: `f(t)` is the colour, the dot's radius and its alpha.
    fn trail(&mut self, p0: (f32, f32, f32, f32), p1: (f32, f32, f32, f32), f: &dyn Fn(f32) -> (Rgb, f32)) {
        let len = (p1.0 - p0.0).hypot(p1.1 - p0.1);
        let n = (len / (1.2f32).max(p0.3.max(p1.3) * 0.45)).ceil().clamp(1., 260.) as i32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (c, a) = f(t);
            let r = p0.3 + (p1.3 - p0.3) * t;
            self.disc(p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t, r, p0.2 + (p1.2 - p0.2) * t, c, a);
        }
    }
    /// A dot laid over the picture, see-through: it takes `rgb` by `k` of what is under it (so the road and
    /// the pods show through), snapped to the realm's inks, and is there `a` of the time by the dither.
    fn tint(&mut self, x: i32, y: i32, z: f32, rgb: Rgb, k: f32, a: f32) {
        if a <= bayer(x, y) || !self.open(x, y, z) { return; }
        let Some(under) = self.im.get(x, y) else { return };
        let mixed = [0, 1, 2].map(|i| under[i] as f32 + (rgb[i] as f32 - under[i] as f32) * k);
        self.im.set(x, y, snap_plain(mixed));
    }
    /// A lumpy puff of `r` dots across (three overlapping lobes, so it is no disc), tinted `rgb` by `k`,
    /// thickest at its middle and thinning to nothing at its rim.
    fn cloud(&mut self, cx: f32, cy: f32, r: f32, z: f32, seed: u32, rgb: Rgb, k: f32, a: f32) {
        if r < 1.2 { self.tint(cx.round() as i32, cy.round() as i32, z, rgb, k, a.max(0.4)); return; }
        let turn = rnd(seed) * TAU;
        let lobes = [(0., 0., 1.), (0.5 * turn.cos(), 0.42 * turn.sin(), 0.7), (-0.5 * (turn + 1.9).cos(), -0.4 * (turn + 1.9).sin(), 0.62)];
        for (ox, oy, lr) in lobes {
            let (lx, ly, rr) = (cx + ox * r, cy + oy * r, r * lr);
            for y in (ly - rr).floor() as i32..=(ly + rr).ceil() as i32 {
                for x in (lx - rr).floor() as i32..=(lx + rr).ceil() as i32 {
                    let (dx, dy) = ((x as f32 + 0.5 - lx) / rr, (y as f32 + 0.5 - ly) / rr);
                    let d2 = dx * dx + dy * dy;
                    if d2 > 1. { continue; }
                    self.tint(x, y, z, rgb, k, a * (1. - d2).powf(0.8));
                }
            }
        }
    }
    fn lighten(&mut self, x: i32, y: i32, by: i32, a: f32) {
        if a > bayer(x, y) && x >= 0 && x < 384 && y >= 0 && y < FLOOR {
            if let Some(c) = self.im.get(x, y) { self.im.set(x, y, step(c, by)); }
        }
    }
}

// ---------------------------------------------------------------- the mood (what the lens reads)

/// What the springs add to the chase rig this tick. `Mood::default()` is nothing.
#[derive(Clone, Copy, Debug)]
pub struct Mood {
    /// How far the lens' lane trails the pod's (metres across the road).
    pub lag_y: f32,
    /// Radians the lens' heading trails the road's.
    pub heading: f32,
    pub pitch: f32,
    /// Roll into a bend, with a wall's jolt on it (radians, positive clockwise).
    pub roll: f32,
    /// Field of view added by a boost's kick and the burn that follows it.
    pub fov: f32,
    /// Metres the surge throws the lens back.
    pub pull: f32,
    /// Metres the lens has dropped (a landing); negative on the bounce.
    pub dip: f32,
    /// Shake in the lens' own (right, up, forward), metres.
    pub shake: V3,
    /// How hard the engines burn, 0 to 1: the vignette and the plumes read it.
    pub boost: f32,
}

impl Default for Mood {
    fn default() -> Mood { Mood { lag_y: 0., heading: 0., pitch: 0., roll: 0., fov: 0., pull: 0., dip: 0., shake: V3::ZERO, boost: 0. } }
}

#[derive(Clone, Copy, Default)]
struct Snap { d: f32, y: f32, v: f32, vy: f32, boosting: bool, flying: bool, impact: u32, vz: f32, fell: u32, slams: u32 }
impl Snap {
    fn of(c: &rules::Car) -> Snap {
        Snap { d: c.d, y: c.y, v: c.v, vy: c.vy, boosting: c.boosting, flying: c.flying, impact: c.impact, vz: c.vz, fell: c.fell, slams: c.slams }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind { Spark, Dust, Flame, Ember, Smoke }

/// One mark in the air, in the road's frame: where (`s` along, `y` across, `z` up), how it moves
/// (metres a second in the same), how long it has and has had to live.
#[derive(Clone, Copy, Debug)]
pub struct P {
    pub kind: Kind,
    pub at: (f32, f32, f32),
    pub vel: (f32, f32, f32),
    pub age: u8,
    pub life: u8,
    /// Metres across at its fullest.
    pub size: f32,
    pub tint: Rgb,
}

/// What this screen remembers of one car between its frames: its last state, the path of each
/// engine behind it, and when it last lit.
#[derive(Default)]
struct Seen {
    snap: Snap,
    trail: [Vec<(f32, f32, f32)>; 2],
    ignite: u32,
}

#[derive(Default)]
struct Mem {
    course: Option<rules::CourseId>,
    ticks: u32,
    snap: Option<Snap>,
    // The lens' springs.
    lat: f32, head: f32, roll: f32, kick: f32, boost: f32,
    dip: f32, dip_v: f32, pitch: f32, pitch_v: f32, rj: f32, rj_v: f32,
    shake: f32,
    jitter: [f32; 3],
    near: BTreeMap<u32, u32>,
    // The marks.
    ptick: u32,
    parts: Vec<P>,
    cars: BTreeMap<u32, Seen>,
    pending: Vec<P>,
    ground: Option<Rgb>,
}

thread_local! { static MEM: RefCell<BTreeMap<u32, Mem>> = const { RefCell::new(BTreeMap::new()) }; }

fn along(lap: f32, d: f32, s: f32) -> f32 {
    let mut ds = (d - s).rem_euclid(lap);
    if ds > lap * 0.5 { ds = lap - ds; }
    ds
}

impl Mem {
    /// Bring the springs to this tick of the race, from the car as it is.
    fn sync(&mut self, race: &rules::Session, car: &rules::Car) {
        let ticks = race.ticks;
        let fresh = match &self.snap {
            None => true,
            Some(s) => ticks < self.ticks || ticks > self.ticks + 24 || (car.d - s.d).abs() > 40. || self.course != Some(race.course),
        };
        if fresh {
            let pending = std::mem::take(&mut self.pending);
            *self = Mem { course: Some(race.course), ticks, snap: Some(Snap::of(car)), lat: car.y, boost: if car.boosting { 1. } else { 0. }, ptick: ticks, pending, ..Mem::default() };
            return;
        }
        if ticks == self.ticks { return; }
        let steps = (ticks - self.ticks).min(3);
        for i in 0..steps { self.step(race, car, i + 1 == steps); }
        self.ticks = ticks;
    }

    /// One tick of the springs. `last` is the tick the car is at: only it can see an event.
    fn step(&mut self, race: &rules::Session, car: &rules::Car, last: bool) {
        let prev = self.snap.unwrap_or_else(|| Snap::of(car));
        let course = race.course();
        let fr = course.frame(car.route, car.d.rem_euclid(course.lap()));
        let rate = |tau: f32| 1. - (-DT / tau).exp();
        let sp = (car.v / 58.).clamp(0., 1.);
        // Lag: the lens' lane trails the pod's, and its heading trails the road's turn.
        self.lat += (car.y - self.lat) * rate(tune("TAU_LAT", 0.22));
        let turn = fr.k * car.v;
        self.head += (-turn * tune("TAU_HEAD", 0.2) - self.head) * rate(0.12);
        // Roll into a bend: the centripetal push, the steer and the road's bank.
        let lean = -(fr.k * car.v * car.v * tune("ROLL_K", 0.0013) + car.vy * tune("ROLL_V", 0.0022) + fr.bank * tune("ROLL_B", 0.12));
        self.roll += (lean.clamp(-0.1, 0.1) - self.roll) * rate(0.16);
        // Boost: the kick at ignition eases back to the burn.
        self.kick *= (-DT / 0.42).exp();
        self.boost += ((car.boosting as u8 as f32) - self.boost) * if car.boosting { 0.4 } else { 0.1 };
        if last && car.boosting && !prev.boosting { self.kick = 1.; self.shake += 0.1; }
        if last && !car.boosting && prev.boosting { self.kick *= 0.5; }
        // A landing, a wall, a hazard, a near miss.
        if last {
            let landed = prev.flying && !car.flying && car.fell == prev.fell;
            if landed {
                let hard = (-prev.vz / 14.).clamp(0.3, 1.3);
                self.dip_v += 11. * hard;
                self.pitch_v -= 0.6 * hard;
                self.shake += 0.35 * hard;
            }
            if car.fell != prev.fell { self.shake += 1.0; self.dip_v += 8.; }
            if car.impact > prev.impact && !landed && car.fell == prev.fell {
                let (gain, side) = match car.impact { 12 => (1.0, car.y.signum()), 10 => (0.8, 0.), 8 => (0.7, 0.), _ => (0.22, 0.) };
                self.shake += gain;
                self.rj_v += side * 1.6 + if side == 0. { 0.5 * rsg(race.ticks * 3 + 1) } else { 0. };
            }
            // A slam from another pod (the law's): a jolt thrown away from the side it came from.
            if car.slams > prev.slams {
                let hard = (0.55 + car.kick.abs() / 14.).clamp(0.6, 1.2);
                self.shake += hard;
                self.rj_v += f32::from(car.slam_side) * 1.6 * hard + if car.slam_side == 0 { 0.5 * rsg(race.ticks * 3 + 9) } else { 0. };
                self.dip_v += 3. * hard;
            }
            self.near_miss(race, car);
        }
        self.shake = (self.shake * (-DT / 0.24).exp()).min(1.5);
        // The springs: a dip and its bounce, the nose's pitch, a wall's roll.
        let spring = |x: &mut f32, v: &mut f32, om: f32, zeta: f32| { let a = -om * om * *x - 2. * zeta * om * *v; *v += a * DT; *x += *v * DT; };
        spring(&mut self.dip, &mut self.dip_v, 13., 0.28);
        spring(&mut self.pitch, &mut self.pitch_v, 11., 0.3);
        spring(&mut self.rj, &mut self.rj_v, 20., 0.22);
        // Shake and rumble, fresh each tick.
        let (t, amp) = (race.ticks, self.shake * self.shake);
        let rum = tune("RUMBLE", 0.05) * sp * sp + 0.05 * self.boost;
        self.jitter = [
            amp * 0.6 * rsg(t * 5 + 1) + rum * 0.7 * rsg(t * 5 + 2),
            amp * 0.5 * rsg(t * 5 + 3) + rum * rsg(t * 5 + 4),
            amp * 0.4 * rsg(t * 5 + 5),
        ];
        self.snap = Some(Snap::of(car));
    }

    /// A rival or a hazard just missed, a shake and a whoosh to come: once each, by a cooldown.
    fn near_miss(&mut self, race: &rules::Session, car: &rules::Car) {
        if car.flying || car.finish.is_some() { return; }
        let lap = race.course().lap();
        let mut hit = false;
        for (&id, o) in &race.cars {
            if o.route != car.route || o.finish.is_some() || std::ptr::eq(o, car) { continue; }
            let (dd, dy) = (along(lap, o.d, car.d), (o.y - car.y).abs());
            if dd < 5. && (2.1..4.6).contains(&dy) && (car.v - o.v).abs() > 3. && race.ticks >= self.near.get(&id).copied().unwrap_or(0) + 24 {
                self.near.insert(id, race.ticks);
                hit = true;
            }
        }
        let hazards = if car.route == 0 { race.course().hazards() } else { &[][..] };
        for (i, h) in hazards.iter().enumerate() {
            let id = 1000 + i as u32;
            let (dd, dy) = (along(lap, car.d, h.s), (car.y - h.y).abs());
            if h.kind != rules::HazardKind::Slick && h.live(race.ticks) && dd < h.half_l + 3. && (h.half_w + 1.05..h.half_w + 2.3).contains(&dy)
                && race.ticks >= self.near.get(&id).copied().unwrap_or(0) + 24 {
                self.near.insert(id, race.ticks);
                hit = true;
            }
        }
        if hit { self.shake += 0.2; self.rj_v += 0.4 * rsg(race.ticks * 7 + 3); }
    }

    fn mood(&self, car: &rules::Car) -> Mood {
        Mood {
            lag_y: self.lat - car.y,
            heading: self.head,
            pitch: self.pitch + self.jitter[1] * 0.004,
            roll: self.roll + self.rj,
            fov: tune("FOV_KICK", 0.2) * self.kick + tune("FOV_BOOST", 0.08) * self.boost + self.shake * 0.04,
            pull: tune("PULL", 2.4) * self.kick,
            dip: self.dip,
            shake: v3(self.jitter[0], self.jitter[1], self.jitter[2]),
            boost: self.boost,
        }
    }
}

/// Every seat's memory wiped, so a still starts from the car as it is (the proofs').
#[cfg(test)]
pub fn forget_all() { MEM.with(|m| m.borrow_mut().clear()); }

/// The mood of `seat`'s camera at this tick of the race (the springs advanced to it).
pub fn mood(race: &rules::Session, seat: u32, car: &rules::Car) -> Mood {
    MEM.with(|m| {
        let mut all = m.borrow_mut();
        let mem = all.entry(seat).or_default();
        mem.sync(race, car);
        mem.mood(car)
    })
}

// ---------------------------------------------------------------- where an engine is

/// Where a car's two engines breathe, in the road's frame: (s, y, z) for the left and right. The
/// throats are the pods' own (`pods::throats`, in the pod's frame, footprint and all), so a plume
/// leaves the nozzle the mesh draws. `pods::throats` lists the lesser-`y` engine first; the left
/// of the road is the greater `y`, so the pair is turned about.
pub fn nozzles(race: &rules::Session, seat: u32, c: &rules::Car) -> [(f32, f32, f32); 2] {
    let a = c.yaw - c.vy * 0.012;
    let (sa, ca) = a.sin_cos();
    let _ = race;
    let t = throats(look_of(seat, c));
    [t[1], t[0]].map(|(at, _)| (c.d + at.x * ca - at.y * sa, c.y + at.x * sa + at.y * ca, c.lift.max(-30.) + at.z))
}

/// The height of a car's engines, for what is thrown off its flank.
fn nz_z(n: &[(f32, f32, f32); 2]) -> f32 { (n[0].2 + n[1].2) * 0.5 }

fn world(at: (f32, f32, f32)) -> V3 { point(at.0, at.1, at.2) }

// ---------------------------------------------------------------- the marks

/// Hand a mark to `viewer`'s screen: it joins the particles at that seat's next frame. The
/// position and velocity are in the road's frame: (s along it, y across it to the left, z up),
/// metres and metres a second, so a mark left at rest stays where it was set and the car runs
/// on from it.
pub fn emit(viewer: u32, kind: Kind, at: (f32, f32, f32), vel: (f32, f32, f32), life: u8, size: f32, tint: Option<Rgb>) {
    MEM.with(|m| {
        let mut all = m.borrow_mut();
        let mem = all.entry(viewer).or_default();
        if mem.pending.len() < 64 {
            mem.pending.push(P { kind, at, vel, age: 0, life: life.max(1), size, tint: tint.unwrap_or([0, 0, 0]) });
        }
    });
}

impl Mem {
    fn push(&mut self, p: P) {
        if self.parts.len() < CAP { self.parts.push(p); }
    }

    fn step_parts(&mut self) {
        for p in &mut self.parts {
            let (g, drag) = match p.kind { Kind::Spark => (14., 0.985), Kind::Ember => (-2., 0.96), Kind::Dust => (7., 0.93), Kind::Flame => (-3., 0.9), Kind::Smoke => (-3.5, 0.94) };
            p.vel.2 -= g * DT;
            p.vel = (p.vel.0 * drag, p.vel.1 * drag, p.vel.2 * if p.kind == Kind::Dust { 0.97 } else { drag });
            p.at = (p.at.0 + p.vel.0 * DT, p.at.1 + p.vel.1 * DT, p.at.2 + p.vel.2 * DT);
            if p.at.2 < 0. {
                p.at.2 = 0.;
                p.vel.2 = if p.kind == Kind::Spark { -p.vel.2 * 0.35 } else { 0. };
                if p.kind == Kind::Dust { p.vel.0 *= 0.8; p.vel.1 *= 0.8; }
            }
            p.age += 1;
        }
        self.parts.retain(|p| p.age < p.life);
    }

    /// The ground's colour near the viewer's pod, a few dots off its flanks, for the dust to take.
    fn sample_ground(&mut self, b: &Brush, car: &rules::Car, gap: f32) {
        let mut best: Option<(u32, Rgb)> = None;
        for (ds, dy) in [(1.0f32, gap + 2.6), (1., -(gap + 2.6)), (-3., gap + 3.4), (-3., -(gap + 3.4))] {
            let Some((x, y, _)) = b.lens.project(point(car.d + ds, car.y + dy, 0.)) else { continue };
            let Some(c) = b.im.get(x as i32, y as i32) else { continue };
            let lum = c[0] as u32 * 3 + c[1] as u32 * 6 + c[2] as u32;
            if lum > 120 && best.is_none_or(|(l, _)| lum > l) { best = Some((lum, c)); }
        }
        if let Some((_, c)) = best { self.ground = Some(c); }
    }

    /// One tick of marks: the cars' events spawn them, then they move.
    fn tick_parts(&mut self, race: &rules::Session, seat: u32, car: &rules::Car, b: &Brush) {
        let ticks = race.ticks;
        if ticks < self.ptick || ticks > self.ptick + 24 {
            self.parts.clear();
            self.cars.clear();
            self.ptick = ticks;
        }
        let steps = (ticks - self.ptick).min(3);
        self.ptick = ticks;
        let own = pod(look_of(seat, car));
        self.sample_ground(b, car, own.gap);
        let dust = self.ground.filter(|c| ink::bank_of(*c).is_some_and(|k| k != ink::SIGNAL_BANK)).unwrap_or_else(|| ink_rgb('J'));
        for _ in 0..steps.saturating_sub(1) { self.step_parts(); }
        if steps > 0 { self.step_parts(); }
        for p in std::mem::take(&mut self.pending) { self.push(p); }
        if steps == 0 { return; }
        let lap = race.course().lap();
        let course = race.course();
        // The viewer's own pod first, so when the marks are at their cap it is the rivals' that are dropped.
        let mut order: Vec<u32> = race.cars.keys().copied().collect();
        order.sort_by_key(|&id| id != seat);
        for id in order {
            let c = &race.cars[&id];
            let dd = c.d - car.d;
            let near = id == seat || (-25. ..70.).contains(&dd);
            let look = look_of(id, c);
            let pd = pod(look);
            let a = c.yaw - c.vy * 0.012;
            let (sa, ca) = a.sin_cos();
            let now = nozzles(race, id, c);
            let seen = self.cars.entry(id).or_insert_with(|| Seen { snap: Snap::of(c), ..Seen::default() });
            let was = seen.snap;
            seen.snap = Snap::of(c);
            for e in 0..2 {
                seen.trail[e].insert(0, now[e]);
                seen.trail[e].truncate(16);
            }
            if !near || c.finish.is_some() { continue; }
            let salt = ticks.wrapping_mul(131).wrapping_add(id * 977);
            let thr = (c.v / 50.).clamp(0., 1.2);
            let (v, boosting) = (c.v, c.boosting);
            let fr = course.frame(c.route, c.d.rem_euclid(lap));
            let mut fresh: Vec<P> = Vec::new();
            // Ignition: a burst of flame off both engines.
            if boosting && !was.boosting {
                seen.ignite = ticks;
                for (e, n) in now.iter().enumerate() {
                    for k in 0..9u32 {
                        let sd = salt + e as u32 * 50 + k * 7;
                        fresh.push(P { kind: Kind::Flame, at: (n.0, n.1, n.2),
                            vel: (v * 0.45 - 4. - 14. * rnd(sd), (e as f32 * 2. - 1.) * -1. * (1. + 4. * rnd(sd + 1)) + rsg(sd + 2) * 2., 0.6 + 4. * rsg(sd + 3)),
                            age: 0, life: 7 + (rnd(sd + 4) * 6.) as u8, size: pd.r * (0.4 + 0.5 * rnd(sd + 5)), tint: [0, 0, 0] });
                    }
                }
            }
            // The engines' wash: dust kicked off the road behind them, a rooster tail at speed.
            if !c.flying && v > 14. {
                let n = 1 + boosting as u32;
                for (e, nz) in now.iter().enumerate() {
                    for k in 0..n {
                        let sd = salt + e as u32 * 31 + k * 13;
                        if rnd(sd) > 0.2 + 0.5 * thr.min(1.) { continue; }
                        fresh.push(P { kind: Kind::Dust, at: (nz.0 - 0.6, nz.1 + rsg(sd + 1) * 0.4, 0.2 + 0.2 * rnd(sd + 2)),
                            vel: (v * (0.4 + 0.25 * rnd(sd + 3)), (e as f32 * 2. - 1.) * -(0.8 + 2.2 * rnd(sd + 4)) + rsg(sd + 5), 2.5 + 4.5 * rnd(sd + 6) + boosting as u32 as f32 * 2.),
                            age: 0, life: 8 + (rnd(sd + 7) * 6.) as u8, size: 0.22 + 0.4 * rnd(sd + 8), tint: dust });
                    }
                }
            }
            // Embers drift off an engine on the burn.
            if v > 20. && rnd(salt) < 0.5 {
                let e = (salt & 1) as usize;
                let nz = now[e];
                fresh.push(P { kind: Kind::Ember, at: nz, vel: (v * 0.6, rsg(salt + 3) * 2.5, 1.5 + 3. * rnd(salt + 4)), age: 0, life: 7 + (rnd(salt + 5) * 6.) as u8, size: 0.2, tint: [0, 0, 0] });
            }
            // The wall: sparks off the engine on that side.
            let edge = if c.y >= 0. { fr.wl } else { fr.wr };
            let scrape = c.y.abs() > edge - 0.45 && v > 6. && !c.flying;
            let hit = c.impact > was.impact && c.impact >= 12;
            if scrape || hit {
                let side = c.y.signum();
                let e = if side > 0. { 0 } else { 1 };
                let nz = now[e];
                let n = if hit { 24 } else { 4 };
                for k in 0..n {
                    let sd = salt + 17 + k * 11;
                    let spread = rsg(sd + 1);
                    fresh.push(P { kind: Kind::Spark, at: (nz.0 + pd.len * 0.5 * rsg(sd + 6) + pd.len * 0.4, c.y + side * pd.r, nz.2 - 0.2 + 0.7 * rnd(sd + 2)),
                        vel: (v * (0.55 + 0.4 * rnd(sd + 3)), -side * (1.5 + 5. * rnd(sd + 4)), 2. + 6. * rnd(sd + 5) + spread),
                        age: 0, life: 8 + (rnd(sd + 7) * 8.) as u8, size: 0.12, tint: [0, 0, 0] });
                }
            }
            // A slam from another pod throws sparks off the side that took it.
            if c.slams > was.slams {
                let side = if c.slam_side == 0 { -1. } else { f32::from(c.slam_side) };
                let n = if c.slam_side == 0 { 10 } else { 16 };
                for k in 0..n {
                    let sd = salt + 61 + k * 9;
                    fresh.push(P { kind: Kind::Spark, at: (c.d + pd.len * 0.3 * rsg(sd + 6), c.y + side * c.spec().half_w, nz_z(&now) - 0.2 + 0.7 * rnd(sd + 2)),
                        vel: (v * (0.45 + 0.4 * rnd(sd + 3)) + rsg(sd) * 3., side * (1. + 5. * rnd(sd + 4)), 2. + 6. * rnd(sd + 5)),
                        age: 0, life: 8 + (rnd(sd + 7) * 8.) as u8, size: 0.12, tint: [0, 0, 0] });
                }
            }
            // A hazard's blow throws sparks and dust.
            if c.impact > was.impact && c.impact == 8 && !(was.flying && !c.flying) {
                for k in 0..10u32 {
                    let sd = salt + 91 + k * 7;
                    fresh.push(P { kind: Kind::Spark, at: (c.d - 1., c.y + rsg(sd) * 1.2, 0.8), vel: (v * 0.5, rsg(sd + 1) * 6., 2. + 6. * rnd(sd + 2)), age: 0, life: 9 + (rnd(sd + 3) * 6.) as u8, size: 0.12, tint: [0, 0, 0] });
                }
            }
            // Landing: a ring of dust and a few sparks.
            if was.flying && !c.flying && c.fell == was.fell {
                let hard = (-was.vz / 14.).clamp(0.3, 1.3);
                let n = (20. * hard) as u32;
                for k in 0..n {
                    let sd = salt + 201 + k * 5;
                    let th = rnd(sd) * TAU;
                    let (sn, cs) = th.sin_cos();
                    let out = 3. + 8. * rnd(sd + 1) * hard;
                    fresh.push(P { kind: Kind::Dust, at: (c.d - 1.5 + cs * 1.4, c.y + sn * 1.8, 0.2),
                        vel: (v * 0.55 + cs * out, sn * out, 1.5 + 5. * rnd(sd + 2) * hard), age: 0, life: 10 + (rnd(sd + 3) * 8.) as u8, size: 0.35 + 0.55 * rnd(sd + 4), tint: dust });
                }
                for k in 0..(10. * hard) as u32 {
                    let sd = salt + 301 + k * 5;
                    fresh.push(P { kind: Kind::Spark, at: (c.d - 1.2, c.y + rsg(sd) * 1.6, 0.3), vel: (v * 0.6 + rsg(sd + 1) * 3., rsg(sd + 2) * 6., 3. + 6. * rnd(sd + 3)), age: 0, life: 8 + (rnd(sd + 4) * 7.) as u8, size: 0.12, tint: [0, 0, 0] });
                }
            }
            // Smoke from an engine that is hurt is the law's to hand in through `emit`.
            let _ = (ca, sa);
            // A rival's marks thin out with its distance, and leave room in the cap for the viewer's own.
            let keep = if id == seat { 1. } else { (1.15 - dd.abs() / 60.).clamp(0.15, 1.) };
            for (k, p) in fresh.into_iter().enumerate() {
                if rnd(salt.wrapping_add(k as u32 * 29)) <= keep && (id == seat || self.parts.len() + 70 < CAP) { self.push(p); }
            }
        }
    }
}

// ---------------------------------------------------------------- the picture

/// What a screen paints over the raster, in this order: the road's wash, speed streaks
/// in the air, the engines' plumes, the marks, the ignition's shock, a rival's whoosh,
/// the radial lines and the vignette at the edges. `im` is the picture, `depth` the raster's.
pub fn draw(im: &mut Img, race: &rules::Session, car: &rules::Car, seat: u32, view: &View3, depth: &[f32]) {
    shimmer::draw(im, race, car, view, depth);
    if race.finished || race.ticks < rules::race::COUNTDOWN { return; }
    let lens = Lens::new(view);
    let mut b = Brush { im, depth, lens };
    MEM.with(|m| {
        let mut all = m.borrow_mut();
        let mem = all.entry(seat).or_default();
        mem.sync(race, car);
        if !skipped(256) { mem.tick_parts(race, seat, car, &b); }
        let boost = mem.boost;
        let sp = (car.v / 58.).clamp(0., 1.);
        if !skipped(1) { wash(&mut b, mem, race, seat); }
        if !skipped(2) { streaks(&mut b, race, car, sp, boost); }
        if !skipped(4) {
            // The viewer's own plumes first, then the rest from the farthest in.
            let mut order: Vec<u32> = race.cars.keys().copied().collect();
            order.sort_by_key(|&id| id != seat);
            for id in order {
                let c = &race.cars[&id];
                if let Some(seen) = mem.cars.get(&id) { plume(&mut b, race, id, c, seen, id == seat, if id == seat { boost } else { c.boosting as u8 as f32 }); }
            }
        }
        if !skipped(1024) { damage(&mut b, race, seat); }
        if !skipped(8) { let hold = Hold::of(&b, seat, car); marks(&mut b, mem, hold); }
        if !skipped(16) {
            for (&id, c) in &race.cars {
                if let Some(seen) = mem.cars.get(&id) { shock(&mut b, race, id, c, seen.ignite); }
            }
        }
        if !skipped(32) { whoosh(&mut b, race, car, seat); }
        if !skipped(512) { draft(&mut b, race); }
        if car.finish.is_none() {
            if !skipped(64) { radial(&mut b, race, sp, boost); }
            if !skipped(128) { vignette(&mut b, race.ticks, sp, boost); }
        }
    });
}

/// The road blown pale under and behind the engines: the ground's own next lighter step.
fn wash(b: &mut Brush, mem: &Mem, race: &rules::Session, seat: u32) {
    let Some(c) = race.cars.get(&seat) else { return };
    if c.v < 12. || c.flying { return; }
    let k = (c.v / 50.).clamp(0., 1.2);
    for seen in mem.cars.get(&seat).into_iter() {
        for tr in &seen.trail {
            for (i, &p) in tr.iter().enumerate().take(7) {
                let Some((x, y, z)) = b.lens.project(world((p.0, p.1, 0.))) else { continue };
                let r = (1.5 + 0.2 * i as f32) * b.lens.scale(z);
                let r = r.clamp(2., 22.);
                let a = (0.4 - 0.035 * i as f32) * k.min(1.) * tune("WASH", 1.0);
                for yy in (y - r * 0.5) as i32..=(y + r * 0.5) as i32 {
                    for xx in (x - r) as i32..=(x + r) as i32 {
                        let (dx, dy) = ((xx as f32 - x) / r, (yy as f32 - y) / (r * 0.5));
                        let d2 = dx * dx + dy * dy;
                        if d2 > 1. { continue; }
                        // Only the road: what the raster put at the road's own depth.
                        if a * (1. - d2) <= bayer(xx, yy) { continue; }
                        let ok = b.depth.get((yy.clamp(0, 223) * 384 + xx.clamp(0, 383)) as usize).is_some_and(|&d| (d - z).abs() < 1.5 + z * 0.06);
                        if ok { b.lighten(xx, yy, 1, 1.); }
                    }
                }
            }
        }
    }
}

/// Specks of wind that stand in the air and stream past the lens, drawn as lines along the road:
/// the radial streaks of a fast ride, with parallax. Fixed by the camera's place on the road, so
/// there is nothing to remember.
fn streaks(b: &mut Brush, race: &rules::Session, car: &rules::Car, sp: f32, boost: f32) {
    let v = car.v;
    if v < 20. { return; }
    let heat = ((v - 20.) / 38.).clamp(0., 1.);
    let n = (8. + 44. * heat * heat + 12. * boost) as u32;
    let period = 80.;
    let back = tune("BACK", camera::BACK);
    let cam_s = car.d - back;
    let len = (2.6 + 7. * heat) * tune("STREAK", 1.0);
    let ticks = race.ticks;
    for i in 0..n {
        let phase = rnd(i * 17 + 5) * period;
        let k = ((cam_s + 14. - phase) / period).ceil();
        let s = phase + k * period;
        let seed = i.wrapping_mul(977).wrapping_add((k as i32 as u32).wrapping_mul(31));
        let y = car.y * 0.5 + rsg(seed) * 15.;
        let z = 0.3 + rnd(seed + 1) * 7.5;
        let a = point(s, y, z);
        let t = point(s + len * (0.7 + 0.6 * rnd(seed + 2)), y, z);
        let (Some((x0, y0, z0)), Some((x1, y1, _))) = (b.lens.project(a), b.lens.project(t)) else { continue };
        if z0 > 90. { continue; }
        // Closer is brighter and longer; the far ones are slate. A few drop out each tick: the flicker of wind.
        let near = (1. - (z0 - 8.) / 80.).clamp(0., 1.);
        if (ticks + seed) % 5 == 0 { continue; }
        let ch = if boost > 0.4 && rnd(seed + 3) < 0.5 { ramp(&SHOCK, 0.4 - 0.4 * near) } else { ramp(&WIND, 0.85 - 0.8 * near) };
        b.seg((x0, y0), (x0 + (x1 - x0) * (0.5 + 0.5 * near), y0 + (y1 - y0) * (0.5 + 0.5 * near)), z0, ink_rgb(ch), 1.);
    }
    let _ = sp;
}

/// How far in front of the lens a point is along its view.
fn z_of(lens: &Lens, p: V3) -> f32 { (p - lens.pos()).dot(lens.fwd()) }

/// An engine's plume: the path it has flown, a ribbon that lengthens and brightens with the
/// burn, white-hot at the nozzle and cooling to red.
fn plume(b: &mut Brush, race: &rules::Session, id: u32, c: &rules::Car, seen: &Seen, own: bool, boost: f32) {
    if c.finish.is_some() || seen.trail[0].len() < 2 { return; }
    // A rival's plume is smaller and shorter the farther it is (a speck at that range, and the flame of five
    // pods at once is what buried a battle); past 130 m it is left to the engine's own light.
    let far = if own { 1. } else { ((130. - (world(seen.trail[0][0]) - b.lens.pos()).length()) / 70.).clamp(0., 1.) };
    if far <= 0. { return; }
    let thr = (c.v / 50.).clamp(0., 1.2);
    let long = (tune("PLUME", 1.0)) * (1.8 + 2.8 * thr + 6. * boost) * (0.55 + 0.45 * far);
    let r0 = (throats(look_of(id, c))[0].1 * 0.75).clamp(0.22, 0.62) * (1. + 0.45 * boost) * (0.6 + 0.4 * far);
    let heat = (0.5 + 0.5 * boost + 0.15 * thr).min(1.);
    let flick = 1. + 0.08 * rsg(race.ticks * 3 + id);
    for e in 0..2 {
        let tr = &seen.trail[e];
        let mut pts: Vec<(V3, f32)> = Vec::with_capacity(tr.len() + 1);
        let mut acc = 0.;
        let mut prev = world(tr[0]);
        pts.push((prev, 0.));
        for &p in &tr[1..] {
            let w = world(p);
            let seg = (w - prev).length();
            if acc + seg >= long {
                pts.push((prev.lerp(w, (long - acc) / seg.max(1e-3)), long));
                acc = long;
                break;
            }
            acc += seg;
            pts.push((w, acc));
            prev = w;
        }
        // At a crawl the path has no length: the plume stands straight out the back instead.
        if acc < long.min(0.9 + 2.2 * thr) {
            let a = c.yaw - c.vy * 0.012;
            let tail = world((tr[0].0 - a.cos() * (0.9 + 2.2 * thr), tr[0].1 - a.sin() * (0.9 + 2.2 * thr), tr[0].2));
            pts.push((tail, 0.9 + 2.2 * thr));
            acc = 0.9 + 2.2 * thr;
        }
        let total = acc.max(0.5);
        let proj: Vec<Option<(f32, f32, f32, f32)>> = pts.iter().map(|&(p, at)| {
            let t = at / total;
            // A rival's flame is slimmer as it comes up on the lens: five fat ribbons at 6 to 14 m are what hid the pack.
            let slim = if own { 1. } else { ((z_of(&b.lens, p) - 4.) / 10.).clamp(0.4, 1.) };
            b.lens.project(p).map(|(x, y, z)| (x, y, z, (r0 * (1. - 0.78 * t) * flick * slim * b.lens.scale(z)).min(if own { 26. } else { 17. })))
        }).collect();
        for w in 0..pts.len() - 1 {
            let (Some(a), Some(z)) = (proj[w], proj[w + 1]) else { continue };
            // Past the lens' nose the ribbon is too fat to be a flame: it ends there.
            if a.2 < 5. || z.2 < 5. { continue; }
            let (t0, t1) = (pts[w].1 / total, pts[w + 1].1 / total);
            // The core, then the flame round it: two passes down the same path.
            let shade = |t: f32, hot: f32| ink_rgb(ramp(&FIRE, (t * 0.75 + (1. - hot) * 0.3).clamp(0., 0.6)));
            b.trail((a.0, a.1, a.2, a.3), (z.0, z.1, z.2, z.3), &|u| (shade(t0 + (t1 - t0) * u, heat * 0.7), 1. - 0.75 * (t0 + (t1 - t0) * u)));
            b.trail((a.0, a.1, a.2, a.3 * 0.55), (z.0, z.1, z.2, z.3 * 0.55), &|u| (shade(t0 + (t1 - t0) * u, heat + 0.25), 1.));
        }
    }
}

/// What a hurt engine puts in the air: black smoke that thins as it rises and ages, steam off a
/// blown pod, flame at a burning nozzle, coals, the blowout's shards. They are dots through the
/// dither tinted from the picture under them, so the pods and the road show through. The viewer's
/// own pod is drawn first and the rest from the farthest in, so a near pod's marks lie over a far
/// one's; a far pod's are fewer and smaller (they are specks at that range) and gone past 110 m.
fn damage(b: &mut Brush, race: &rules::Session, seat: u32) {
    let eye = b.lens.pos();
    let mut order: Vec<(u32, f32)> = race.cars.iter().filter(|(_, c)| c.finish.is_none())
        .map(|(&id, c)| (id, (point(c.d, c.y, 0.) - eye).length())).collect();
    order.sort_by(|a, c| (a.0 != seat).cmp(&(c.0 != seat)).then(c.1.total_cmp(&a.1)));
    // The own pod is first; the sort above puts it ahead, then the far ones before the near ones.
    for (id, dist) in order {
        let own = id == seat;
        let reach = if own { 1. } else { ((110. - dist) / 70.).clamp(0., 1.) };
        if reach <= 0. { continue; }
        let c = &race.cars[&id];
        for (n, p) in damage_fx::puffs(race, id, c, eye).into_iter().enumerate() {
            // Far pods keep some of their puffs, by a fixed roll so they do not flicker.
            if !own && rnd(id.wrapping_mul(131).wrapping_add(n as u32 * 7)) > 0.25 + 0.75 * reach { continue; }
            let Some((x, y, z)) = b.lens.project(p.at) else { continue };
            let px = (p.r * b.lens.scale(z) * (0.55 + 0.45 * reach)).clamp(0., 15.);
            let seed = (id << 8).wrapping_add(n as u32 * 13);
            let thin = (1. - p.age).clamp(0., 1.);
            match p.stuff {
                damage_fx::Stuff::Smoke => {
                    // Black at the nozzle, going to ash grey as it climbs; thick at first and then see-through.
                    let rgb = ink_rgb(ramp(&SOOT, 0.05 + 0.9 * p.age));
                    b.cloud(x, y, px, z, seed, rgb, 0.85 - 0.3 * p.age, (1.1 * thin.powf(1.1) * (0.7 + 0.3 * p.weight)).min(0.95));
                }
                damage_fx::Stuff::Steam => {
                    b.cloud(x, y, px * 1.1, z, seed, ink_rgb(ramp(&STEAM, p.age)), 0.55, 0.7 * thin);
                }
                damage_fx::Stuff::Flame => {
                    // White-hot at the nozzle, down through gold and orange to red at the tip.
                    let rgb = ink_rgb(ramp(&FIRE, 0.62 * p.age));
                    b.cloud(x, y, px, z, seed, rgb, 1., 1. - 0.35 * p.age);
                }
                damage_fx::Stuff::Spark => {
                    let rgb = ink_rgb(ramp(&SPARK, p.age));
                    b.dot(x as i32, y as i32, z, rgb);
                    if px > 1.5 { b.dot(x as i32 + 1, y as i32, z, rgb); }
                }
                damage_fx::Stuff::Shard(k) => {
                    let rgb = match k { 0 => ink_rgb(ramp(&FIRE, 0.1 + 0.5 * p.age)), 1 => ink_rgb(ramp(&FIRE, 0.4 + 0.5 * p.age)), _ => ink_rgb(ramp(&SOOT, 0.2 + 0.5 * p.age)) };
                    b.cloud(x, y, px, z, seed, rgb, if k == 2 { 0.7 } else { 1. }, 0.95 * thin.max(0.2));
                }
            }
        }
    }
}

/// The screen the viewer's own pod holds: the rectangle its footprint covers, and how far from the lens its
/// farthest part is. A cloud of dust, flame or smoke in front of that depth and inside that rectangle is
/// between the lens and the pod, and would sit over it; the marks leave it out.
#[derive(Clone, Copy)]
struct Hold { x0: f32, y0: f32, x1: f32, y1: f32, depth: f32 }

impl Hold {
    fn of(b: &Brush, seat: u32, car: &rules::Car) -> Option<Hold> {
        let (ahead, tail, half) = super::pods::foot(look_of(seat, car));
        let lift = car.lift.max(0.);
        let (mut x0, mut y0, mut x1, mut y1, mut depth) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN, 0f32);
        for s in [car.d - tail, car.d + ahead] {
            for y in [car.y - half, car.y + half] {
                for z in [0., 3.4] {
                    let (px, py, pz) = b.lens.project(point(s, y, lift + z))?;
                    x0 = x0.min(px); y0 = y0.min(py); x1 = x1.max(px); y1 = y1.max(py);
                    depth = depth.max(pz);
                }
            }
        }
        Some(Hold { x0, y0, x1, y1, depth })
    }
    fn covers(&self, x: f32, y: f32, z: f32) -> bool { x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1 && z < self.depth }
}

/// The marks in the air.
fn marks(b: &mut Brush, mem: &Mem, hold: Option<Hold>) {
    for p in &mem.parts {
        let af = p.age as f32 / p.life as f32;
        let w = world(p.at);
        let Some((x, y, z)) = b.lens.project(w) else { continue };
        let sc = b.lens.scale(z);
        // Marks of pods far up the road are smaller: the viewer's own are 5 to 17 m off, the pack's twice that.
        let shrink = 1. - 0.45 * ((z - 18.) / 40.).clamp(0., 1.);
        // Clouds between the lens and the viewer's own pod are not drawn over it.
        if let Some(h) = hold { if p.kind != Kind::Spark && h.covers(x, y, z) { continue; } }
        match p.kind {
            Kind::Spark => {
                let back = (p.at.0 - p.vel.0 * DT * 2.2, p.at.1 - p.vel.1 * DT * 2.2, p.at.2 - p.vel.2 * DT * 2.2);
                let Some((x0, y0, _)) = b.lens.project(world(back)) else { continue };
                let c = ink_rgb(ramp(&SPARK, af));
                b.seg((x0, y0), (x, y), z, c, 1.);
                if sc > 11. { b.dot(x as i32, y as i32, z, ink_rgb('9')); }
            }
            Kind::Ember => {
                let c = ink_rgb(ramp(&FIRE, 0.2 + af * 0.7));
                b.dot(x as i32, y as i32, z, c);
            }
            Kind::Flame => {
                let r = (p.size * sc * (1. - 0.6 * af) * shrink).clamp(0.8, 12.);
                b.disc(x, y, r, z, ink_rgb(ramp(&FIRE, af * 0.9)), 1.);
            }
            Kind::Dust => {
                let r = (p.size * sc * (0.5 + 0.9 * af) * shrink).clamp(0.8, 7.);
                let base = if p.tint == [0, 0, 0] { ink_rgb('J') } else { p.tint };
                let c = step(base, 1 + (af < 0.3) as i32);
                b.disc(x, y, r, z, c, (1. - af) * 0.65);
            }
            Kind::Smoke => {
                let r = (p.size * sc * (0.5 + 1.1 * af)).clamp(0.8, 14.);
                b.disc(x, y, r, z, ink_rgb(ramp(&SMOKE, 0.15 + af * 0.7)), (1. - af * af) * 0.85);
            }
        }
    }
}

/// The shock of an ignition: a flash at each nozzle, a ring that opens round the engines.
fn shock(b: &mut Brush, race: &rules::Session, id: u32, c: &rules::Car, at: u32) {
    if at == 0 || race.ticks < at || race.ticks - at > 9 { return; }
    let age = (race.ticks - at) as f32;
    let pd = pod(look_of(id, c));
    let nz = nozzles(race, id, c);
    // The flash.
    if age < 3. {
        for n in nz {
            if let Some((x, y, z)) = b.lens.project(world(n)) {
                let r = (pd.r * (1.9 - 0.5 * age) * b.lens.scale(z)).clamp(2., 34.) * (1. - 0.4 * ((z - 18.) / 40.).clamp(0., 1.));
                b.disc(x, y, r, z, ink_rgb(if age < 1. { 'w' } else { '9' }), 1.);
                b.disc(x, y, r * 1.5, z, ink_rgb('6'), 0.5);
            }
        }
    }
    // The ring, about the engines' axis, opening as it goes.
    let mid = ((nz[0].0 + nz[1].0) * 0.5, (nz[0].1 + nz[1].1) * 0.5, (nz[0].2 + nz[1].2) * 0.5);
    let centre = world(mid);
    let yaw = pose(c.d).2 + c.yaw;
    let right = v3(-yaw.sin(), yaw.cos(), 0.);
    let r = tune("RING", 1.0) * (1.3 + age * 1.1);
    let fade = (1. - age / 10.).clamp(0., 1.);
    let n = 40;
    let mut last: Option<(f32, f32, f32)> = None;
    for i in 0..=n {
        let th = i as f32 / n as f32 * TAU;
        let p = centre + right * (th.cos() * r * 1.25) + V3::UP * (th.sin() * r * 0.8);
        let here = b.lens.project(p);
        if let (Some(a), Some(z)) = (last, here) {
            let ch = ink_rgb(ramp(&SHOCK, age / 10. * 0.8));
            // Solid while it is young, a thread on the dither as it opens.
            b.seg((a.0, a.1), (z.0, z.1), z.2, ch, fade * 1.7);
            if age < 5. { b.seg((a.0 + 1., a.1), (z.0 + 1., z.1), z.2, ch, fade * 1.7); }
        }
        last = here;
    }
}

/// The slipstream, drawn in the air: threads of wind along the cone the law gives a wake (from a
/// following pod's nose back to the tail of the pod ahead whose wake it rides, half a width of
/// `0.55 half_w + 0.8 + 0.06 gap`), running down it toward the follower, more of them and brighter as
/// the draft deepens. It reads `Car::draft` (0..1) and `Car::wake` (the leader's seat), and draws
/// the same cone for every pod in a wake, so a rival's tow can be seen too.
fn draft(b: &mut Brush, race: &rules::Session) {
    use rules::race::contact;
    let lap = race.course().lap();
    for (&id, me) in &race.cars {
        if me.draft < 0.08 || me.wake == 0 || me.finish.is_some() || me.blown() || me.flying { continue; }
        let Some(lead) = race.cars.get(&u32::from(me.wake)) else { continue };
        let gap = (contact::ahead(lap, me.d, lead.d) - contact::nose(me) - contact::tail(lead)).max(0.5);
        let nose = me.d + contact::nose(me);
        let half = lead.spec().half_w * 0.55 + 0.8 + gap * 0.06;
        let n = (3. + 15. * me.draft) as u32;
        for i in 0..n {
            let sd = i.wrapping_mul(2971).wrapping_add(id * 977);
            // Each thread runs from the leader's tail down to the follower, then comes round again.
            let u = (rnd(sd) + race.ticks as f32 * (0.034 + 0.03 * rnd(sd + 1))).fract();
            let s_at = nose + gap * (1. - u);
            let lane = lead.y + (me.y - lead.y) * u;
            let y = lane + rsg(sd + 2) * half * (0.35 + 0.65 * (1. - u));
            let z = 0.35 + rnd(sd + 3) * 1.7;
            let len = (1.6 + 4. * me.draft) * (0.6 + 0.8 * rnd(sd + 4));
            let (Some((x0, y0, z0)), Some((x1, y1, _))) = (b.lens.project(point(s_at + len, y, z)), b.lens.project(point(s_at, y, z))) else { continue };
            if z0 > 120. || z0 < 4. { continue; }
            let strong = me.draft > 0.55 && rnd(sd + 5) < 0.55;
            let ch = if strong { ramp(&SHOCK, 0.3 + 0.4 * u) } else { ramp(&WIND, 0.05 + 0.5 * rnd(sd + 6)) };
            b.seg((x0, y0), (x1, y1), z0, ink_rgb(ch), 0.5 + 0.5 * me.draft);
        }
    }
}

/// A pod passing close in the next lane: a bundle of streaks along the road at it, thick as
/// it is near and as the two ride at different speeds.
fn whoosh(b: &mut Brush, race: &rules::Session, car: &rules::Car, seat: u32) {
    let lap = race.course().lap();
    for (&id, o) in &race.cars {
        if id == seat || o.route != car.route || o.finish.is_some() || car.flying { continue; }
        let dd = along(lap, o.d, car.d);
        let dy = (o.y - car.y).abs();
        let rel = (car.v - o.v).abs();
        if dd > 9. || !(1.8..6.5).contains(&dy) { continue; }
        let k = (1. - dd / 9.) * (1. - (dy - 1.8) / 4.7) * (rel / 9.).clamp(0.25, 1.);
        let n = (8. + 26. * k) as u32;
        for i in 0..n {
            let sd = i.wrapping_mul(131).wrapping_add(race.ticks.wrapping_mul(7)).wrapping_add(id * 977);
            let y = o.y + rsg(sd) * 1.9;
            let z = 0.2 + rnd(sd + 1) * 2.6;
            let s0 = o.d - 5. + rnd(sd + 2) * 9.;
            let len = (3. + 6. * k) * (0.6 + 0.8 * rnd(sd + 3));
            let (Some((x0, y0, z0)), Some((x1, y1, _))) = (b.lens.project(point(s0, y, z)), b.lens.project(point(s0 + len, y, z))) else { continue };
            b.seg((x0, y0), (x1, y1), z0, ink_rgb(ramp(&WIND, 0.02 + 0.45 * rnd(sd + 4))), 1.);
        }
    }
}

/// What the instruments hold, as (left, top, right, bottom): no radial line crosses these.
const PLATES: [(f32, f32, f32, f32); 6] = [(0., 0., 70., 56.), (66., 0., 282., 44.), (280., 0., 384., 80.), (340., 70., 384., 140.), (0., 184., 384., 224.), (20., 68., 106., 170.)];

/// Streaks that fly off the edges of the picture from its vanishing point: more and longer as
/// the engines run hotter, kept off the plates.
fn radial(b: &mut Brush, race: &rules::Session, sp: f32, boost: f32) {
    let heat = ((sp - 0.4) / 0.6).clamp(0., 1.);
    if heat <= 0. { return; }
    let n = (5. + 16. * heat * heat + 10. * boost) as u32;
    let t = race.ticks;
    let (cx, cy) = (192., 108.);
    for i in 0..n {
        let sd = i.wrapping_mul(2_654_435_761).wrapping_add(t.wrapping_mul(40_503));
        let th = rnd(sd) * TAU;
        let (sn, cs) = th.sin_cos();
        // Out to the edge of the picture's ellipse, from a third of the way to it.
        let ex = cs * 200. * (0.62 + 0.3 * rnd(sd + 1));
        let ey = sn * 118. * (0.62 + 0.3 * rnd(sd + 1));
        let len = (0.10 + 0.22 * heat + 0.12 * boost) * (0.5 + rnd(sd + 2));
        let (x0, y0) = (cx + ex, cy + ey);
        let (x1, y1) = (cx + ex * (1. + len * 2.), cy + ey * (1. + len * 2.));
        let hid = PLATES.iter().any(|&(l, t, r, bt)| (x0 > l && x0 < r && y0 > t && y0 < bt) || (x1 > l && x1 < r && y1 > t && y1 < bt));
        if hid { continue; }
        let ch = if boost > 0.3 && rnd(sd + 3) < 0.6 { 'v' } else { 'U' };
        b.seg((x0, y0), (x1, y1), 0., ink_rgb(ch), 1.);
    }
}

/// A small direct-mapped memory of what each packed colour becomes one, two and three steps
/// down its ramp, so a pass over the picture asks the palette once per colour, not per dot.
struct Darker { keys: [u32; 1024], vals: [[u32; 3]; 1024] }
impl Darker {
    fn new() -> Darker { Darker { keys: [0; 1024], vals: [[0; 3]; 1024] } }
    fn by(&mut self, p: u32, lvl: usize) -> u32 {
        let h = (p.wrapping_mul(0x9e37_79b1) >> 22) as usize;
        if self.keys[h] != p {
            let c = [(p >> 16) as u8, (p >> 8) as u8, p as u8];
            for l in 0..3 {
                let d = step(c, -(l as i32 + 1));
                self.vals[h][l] = 0xff00_0000 | (d[0] as u32) << 16 | (d[1] as u32) << 8 | d[2] as u32;
            }
            self.keys[h] = p;
        }
        self.vals[h][lvl - 1]
    }
}

/// How much of the vignette each dot of the picture is under, 0 to 255: nothing in the middle.
fn vignette_shape() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        let mut v = vec![0u8; 384 * FLOOR as usize];
        for y in 0..FLOOR {
            let ny = (y as f32 - 108.) / 112.;
            for x in 0..384 {
                let nx = (x as f32 - 192.) / 192.;
                let r = nx * nx * 0.8 + ny * ny * 1.05;
                v[(y * 384 + x) as usize] = (((r - 0.6) / 0.55).clamp(0., 1.) * 255.) as u8;
            }
        }
        v
    })
}

/// The edges dimmed by dither as the speed climbs, hard on a boost: tunnel vision on the road.
fn vignette(b: &mut Brush, ticks: u32, sp: f32, boost: f32) {
    let amt = (0.4 * sp * sp + 0.95 * boost) * tune("VIGNETTE", 1.0);
    if amt < 0.03 || b.im.w != 384 || b.im.h < FLOOR { return; }
    let flutter = 1. + 0.06 * rsg(ticks);
    // Fixed point: a level is 256, the shape's share 0..255, the dither a threshold of 0..255.
    let k = (amt * flutter * 3. * 256. / 255. * 256.) as i32;
    let shape = vignette_shape();
    let im = &mut *b.im;
    let mut darker = Darker::new();
    // A dot is touched only where the shape's share is enough to reach the first step: the middle
    // of each row, where it is not, is never visited.
    let reach = ((128 * 256) / k.max(1) + 1).min(256) as u32;
    const BAYER16: [[i32; 4]; 4] = [[8, 136, 40, 168], [200, 72, 232, 104], [56, 184, 24, 152], [248, 120, 216, 88]];
    for y in 0..FLOOR {
        let row = (y * 384) as usize;
        let m_at = |x: i32| shape[row + x as usize] as u32;
        let mut left = 0;
        while left < 384 && m_at(left) >= reach { left += 1; }
        let mut right = 384;
        while right > left && m_at(right - 1) >= reach { right -= 1; }
        let thr = &BAYER16[(y & 3) as usize];
        for x in (0..left).chain(right..384) {
            // Up to three steps down the ink's ramp, the levels dithered into one another: no contour lines.
            let a = (m_at(x) as i32 * k) >> 8;
            let lvl = ((a + thr[(x & 3) as usize] - 128) >> 8).clamp(0, 3) as usize;
            if lvl == 0 { continue; }
            let p = im.words()[row + x as usize];
            if p >> 24 != 0 {
                let d = darker.by(p, lvl);
                im.set(x, y, [(d >> 16) as u8, (d >> 8) as u8, d as u8]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn race() -> rules::Session {
        let mut r = rules::Session::blank_on(rules::CourseId::Emberfall);
        r.join_ids(1..=6);
        r.ticks = rules::race::COUNTDOWN + 40;
        r
    }

    #[test]
    fn a_car_at_rest_is_a_lens_without_mood() {
        let mut r = race();
        r.cars.get_mut(&1).unwrap().v = 0.;
        let car = r.cars[&1].clone();
        let m = mood(&r, 1, &car);
        assert!(m.lag_y.abs() < 1e-4 && m.dip.abs() < 1e-4 && m.fov.abs() < 1e-3 && m.shake.length() < 1e-3, "{m:?}");
    }

    #[test]
    fn the_springs_start_over_when_the_clock_goes_back() {
        let mut r = race();
        let mut car = r.cars[&1].clone();
        car.v = 50.; car.boosting = true;
        for t in 0..40 { r.ticks += 1; let _ = mood(&r, 7, &car); let _ = t; }
        let hot = mood(&r, 7, &car);
        assert!(hot.boost > 0.5);
        r.ticks = rules::race::COUNTDOWN + 10;
        car.boosting = false; car.v = 0.;
        let cold = mood(&r, 7, &car);
        assert!(cold.boost < 0.01 && cold.fov.abs() < 0.01, "{cold:?}");
    }

    #[test]
    fn a_boost_kicks_the_field_wide_and_eases_back() {
        let mut r = race();
        let mut car = r.cars[&1].clone();
        car.v = 46.;
        r.ticks += 1; let _ = mood(&r, 3, &car);
        car.boosting = true; car.v = 52.;
        r.ticks += 1;
        let at = mood(&r, 3, &car).fov;
        for _ in 0..40 { r.ticks += 1; let _ = mood(&r, 3, &car); }
        let later = mood(&r, 3, &car).fov;
        assert!(at > later + 0.05 && later > 0., "kick {at}, burn {later}");
    }

    #[test]
    fn the_marks_are_bounded_and_the_frame_is_the_same_twice() {
        let mut r = race();
        for c in r.cars.values_mut() { c.v = 50.; c.boosting = true; }
        let mut run = crate::rules::Run::roundtable_for_test("knight_racers").unwrap();
        run.knight_racers = None;
        let mut frames = Vec::new();
        for _ in 0..2 {
            let mut rr = r.clone();
            let mut last = Img::new(384, 224);
            for _ in 0..60 {
                rr.ticks += 1;
                for c in rr.cars.values_mut() { c.d += 1.6; }
                let car = rr.cars[&1].clone();
                let view = camera::shot(&rr, &rr.cars[&1], 1).view;
                let mut im = Img::black(384, 224);
                draw(&mut im, &rr, &car, 1, &view, &vec![f32::INFINITY; 384 * 224]);
                last = im;
                MEM.with(|m| assert!(m.borrow().get(&1).is_none_or(|x| x.parts.len() <= CAP)));
            }
            frames.push(last);
            MEM.with(|m| m.borrow_mut().clear());
        }
        let _ = run;
        assert!(frames[0] == frames[1], "the same ticks drew two pictures");
    }

    #[test]
    fn a_landing_dips_the_lens_and_it_bounces() {
        let mut r = race();
        let mut car = r.cars[&1].clone();
        car.v = 44.; car.flying = true; car.vz = -12.; car.lift = 1.;
        r.ticks += 1; let _ = mood(&r, 5, &car);
        car.flying = false; car.vz = 0.; car.lift = 0.; car.impact = 8;
        let (mut lowest, mut highest) = (0f32, 0f32);
        for _ in 0..24 {
            r.ticks += 1;
            let m = mood(&r, 5, &car);
            lowest = lowest.max(m.dip);
            highest = highest.min(m.dip);
            car.impact = car.impact.saturating_sub(1);
        }
        assert!(lowest > 0.3, "the lens dipped {lowest} m");
        assert!(highest < -0.02, "and bounced to {highest}");
    }

    #[test]
    fn a_wall_shakes_the_lens_and_it_settles() {
        let mut r = race();
        let mut car = r.cars[&1].clone();
        car.v = 44.;
        r.ticks += 1; let _ = mood(&r, 6, &car);
        car.impact = 12; car.y = 6.;
        let (mut worst, mut last) = (0f32, 0f32);
        for i in 0..40 {
            r.ticks += 1;
            let m = mood(&r, 6, &car);
            worst = worst.max(m.shake.length());
            if i == 39 { last = m.shake.length(); }
            car.impact = car.impact.saturating_sub(1);
        }
        assert!(worst > 0.15 && last < 0.12, "shake {worst} then {last}");
    }

    #[test]
    fn the_lens_leans_into_a_bend() {
        // The sharpest bends of the course, one to each hand.
        let course = rules::course(rules::CourseId::Glassway);
        let (mut left, mut right) = ((0., 0.), (0., 0.));
        let mut s = 200.;
        while s < 4000. {
            let k = course.frame(0, s).k;
            if k > left.1 { left = (s, k); }
            if k < right.1 { right = (s, k); }
            s += 5.;
        }
        for (at, sign) in [(left.0, -1.), (right.0, 1.)] {
            let mut r = rules::Session::blank_on(rules::CourseId::Glassway);
            r.join_ids(1..=6);
            r.ticks = rules::race::COUNTDOWN + 40;
            let mut car = r.cars[&1].clone();
            car.d = at; car.v = 46.;
            let mut roll = 0.;
            for _ in 0..30 { r.ticks += 1; roll = mood(&r, 8, &car).roll; }
            assert!(roll * sign > 0.02, "at {at} the lens rolled {roll}, wanted the sign {sign}");
        }
    }

    /// A cloud between the lens and the viewer's own pod is left out; the same cloud behind the pod is drawn.
    #[test]
    fn marks_do_not_sit_over_the_viewers_own_pod() {
        let mut r = race();
        for c in r.cars.values_mut() { c.d += 400.; c.v = 40.; }
        let lit_with = |cloud: Option<(f32, f32)>| {
            MEM.with(|m| m.borrow_mut().clear());
            let mut rr = r.clone();
            rr.ticks += 6;
            let car = rr.cars[&1].clone();
            let view = camera::shot(&rr, &car, 1).view;
            let depth = vec![f32::INFINITY; 384 * 224];
            let mut warm = Img::black(384, 224);
            draw(&mut warm, &rr, &car, 1, &view, &depth);
            if let Some((ds, z)) = cloud { emit(1, Kind::Flame, (car.d + ds, car.y, z), (0., 0., 0.), 30, 0.7, None); }
            let mut im = Img::black(384, 224);
            draw(&mut im, &rr, &car, 1, &view, &depth);
            (0..224).flat_map(|y| (0..384).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y).is_some_and(|c| c != [0, 0, 0])).count()
        };
        let base = lit_with(None);
        let over = lit_with(Some((-12.5, 4.0)));
        let behind = lit_with(Some((14., 1.5)));
        assert_eq!(over, base, "a cloud between the lens and the pod, over it, was drawn: {over} dots against {base}");
        assert!(behind > base + 8, "the same cloud up the road past the pod is drawn: {behind} against {base}");
    }

    #[test]
    fn a_hold_covers_what_lies_in_front_of_it_inside_its_rectangle() {
        let h = Hold { x0: 100., y0: 100., x1: 200., y1: 200., depth: 10. };
        assert!(h.covers(150., 150., 5.));
        assert!(!h.covers(150., 150., 12.), "farther than the pod is behind it");
        assert!(!h.covers(250., 150., 5.), "outside the rectangle is beside it");
    }

    #[test]
    fn engines_sit_either_side_of_the_pod() {
        let r = race();
        let c = r.cars[&1].clone();
        let n = nozzles(&r, 1, &c);
        assert!(n[0].1 > c.y && n[1].1 < c.y && n[0].0 < c.d);
    }
}
