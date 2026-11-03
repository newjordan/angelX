//! The Lawn Flamingo, drawn (`nooks::gnome::FLAMINGO`): the gnome's lawn
//! ornament, come alive on the garden's floor and very pleased about it.
//!
//! It is a tall pink bird on two grey wire legs, each still ending in the
//! clod of lawn it pulled itself out of, with a glossy body, a neck in a
//! proud S and a bent beak, pale with a black tip. It stands on one leg
//! with the other tucked up, its head bobbing, and now and then shakes its
//! feathers out. It walks with a strut, high-stepping, its head going
//! forward and back like a pigeon's.
//!
//! Its fight is its file's (`lawn-flamingo.boss`), each move its own look:
//!
//! - **The fan** is a wing-gust: wings up and spread, black flight feathers
//!   showing, then swept forward, and the wind goes out in a fan of lines
//!   with pink feathers in it.
//! - **STAB** (the charge): it crouches and coils its neck back, beak on
//!   the lane, the tip glinting; lunges down the lane neck out like a
//!   spear, feathers coming off it; and ends with its beak stuck in the
//!   floor, legs kicking, until it pulls free.
//! - **HONK** (the inhale): it rears and swells up like a balloon,
//!   drawing the air in; then honks, beak wide, the sound going out in
//!   rings, and goes down again.
//! - **The spiral** of its rage is a pirouette: up on one leg, wings out,
//!   turning, a spiral of pink feathers thrown off it as it spins.
//! - **A hit** makes it flinch, eyes wide, a feather or two knocked loose.
//! - **Felled**, it quivers, topples over and goes off in a cloud of its
//!   own feathers; what is left is what it always was, a plastic lawn
//!   flamingo stuck in the ground at an angle, wobbling to a stop in a heap
//!   of pink feathers, until the hand comes for it.
//!
//! Every pose is a pure function of the run: the guardian's stage and
//! timers, its age against its file's beats, the tick. Nothing is kept.

use super::super::super::ink::{Img, bayer, hash};
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ease, ramps, unit};
use super::super::at;
use crate::rules::bosses::{Stage, Windup};
use crate::rules::nooks::gnome::FLAMINGO;
use crate::rules::spectacle::{Beat, Corpse};
use crate::rules::{Boss, Enemy, Run, TELEGRAPH, signatures as fight};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

type V = (f32, f32);

/// Its pinks, deepest first: wine, rose, the body, the blush, the cream.
const PINK: [char; 5] = ['!', '?', ';', '%', '+'];
/// A feather off it, cream at the core to wine.
const FEATHER: Ramp = Ramp(['+', '+', '%', ';', '?', '!']);
/// Air moving: pale through the steel blues.
const GUST: Ramp = Ramp(['W', 'V', 'v', 'U', 'u', 'S']);
/// The sound of it.
const HONK: Ramp = Ramp(['+', '%', 'T', ';', '?', '!']);
/// Turned earth.
const SOD: Ramp = Ramp(['M', 'y', 'P', 'I', 'b', 'n']);

/// The picture it is drawn on, and where its foot stands in it.
const W: i32 = 92;
const H: i32 = 70;
const FOOT: V = (46.0, 69.0);

/// Where its hip stands over its foot, and its body over that.
const HIP: f32 = -21.0;
const BODY: V = (-1.0, -26.0);
/// The body's half-width and half-height.
const RX: f32 = 11.0;
const RY: f32 = 6.5;
/// Where its neck leaves the body, from the body's middle.
const SHOULDER: V = (8.0, -3.0);
/// Its head at rest, from the shoulder.
const REST_HEAD: V = (0.0, -16.0);

// ─── the pose ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
enum Eye {
    Open,
    Wide,
    Shut,
    Dizzy,
    Cross,
}

/// One leg: its knee and its foot, from the hip; whether its clod is on
/// the floor.
#[derive(Clone, Copy, Debug)]
struct Leg {
    knee: V,
    foot: V,
}

/// How the bird stands this tick.
#[derive(Clone, Copy, Debug)]
struct Pose {
    /// Which way it faces, turning about its standing leg: 0 to the
    /// right, a half turn to the left, a quarter toward us.
    turn: f32,
    /// How far its body sinks on bent legs, and its lean (forward, down).
    drop: f32,
    pitch: f32,
    /// Its head from the shoulder; how far forward the neck's middle bows;
    /// where the beak points (0 straight ahead, more is down); how far it
    /// is open.
    head: V,
    bow: f32,
    tilt: f32,
    gape: f32,
    /// Wings: 0 folded, 1 up and spread; swept forward 0 to 1; held out to
    /// the sides (a pirouette) instead.
    wings: f32,
    sweep: f32,
    out: bool,
    /// The near leg and the far one.
    legs: [Leg; 2],
    /// Swollen with air; its feathers stood up.
    puff: f32,
    ruffle: f32,
    eye: Eye,
    angry: bool,
    /// Shaken off its place a pixel or two.
    jitter: V,
    /// Its beak in the floor, a little way ahead.
    stuck: bool,
    /// What it is under the feathers: a moulded lawn ornament on two rods.
    plastic: bool,
    tick: u32,
    seed: u32,
}

impl Pose {
    fn rest(tick: u32, seed: u32) -> Pose {
        Pose {
            turn: 0.0,
            drop: 0.0,
            pitch: -0.06,
            head: REST_HEAD,
            bow: 0.0,
            tilt: 0.25,
            gape: 0.0,
            wings: 0.0,
            sweep: 0.0,
            out: false,
            legs: [STRAIGHT, TUCKED],
            puff: 0.0,
            ruffle: 0.0,
            eye: Eye::Open,
            angry: false,
            jitter: (0.0, 0.0),
            stuck: false,
            plastic: false,
            tick,
            seed,
        }
    }
}

const STRAIGHT: Leg = Leg { knee: (0.5, 11.0), foot: (0.0, 21.0) };
/// Folded up under it, the shin back and the clod tucked to its belly.
const TUCKED: Leg = Leg { knee: (-5.5, 7.0), foot: (1.5, 3.0) };

/// The phase of a beat `every` ticks long that fired at `age` ticks
/// `offset` ago: ticks since it last fired.
fn since(age: u32, offset: u32, every: u32) -> u32 {
    (age + offset) % every.max(1)
}

/// How many ticks apart a line of the file fires at this stage.
fn every_at(every: u32, stage: Stage) -> u32 {
    if stage == Stage::Rage { (every * 2 / 3).max(4) } else { every.max(4) }
}

/// A walking leg at `phase` (0 to 1): planted and pushing back for half,
/// then lifted high, knee up, and swung forward.
fn stride(phase: f32, reach: f32, lift: f32) -> Leg {
    let p = phase.rem_euclid(1.0);
    if p < 0.5 {
        let f = p / 0.5;
        let x = reach * (1.0 - 2.0 * f);
        Leg { knee: (x * 0.5 + 1.0, 11.0), foot: (x, 21.0) }
    } else {
        let f = (p - 0.5) / 0.5;
        let up = ease::bump(f);
        let x = -reach + 2.0 * reach * ease::both(f);
        // A strut: the knee goes up high and forward, the shin hanging
        // under it, the toe kicked out at the last.
        let knee = (x * 0.4 + 4.0 * up + 1.0, 11.0 - 8.0 * up * lift);
        let foot = (x - 2.0 * up + 3.0 * up * f, 21.0 - 9.0 * up * lift);
        Leg { knee, foot }
    }
}

/// The flamingo's pose this tick.
fn pose(run: &Run, enemy: &Enemy, boss: &Boss, tick: u32) -> Pose {
    let seed = hash(enemy.id as i32, 7, 0xf1a);
    let mut p = Pose::rest(tick, seed);
    let t = tick as f32;
    let stage = boss.stage(enemy.hp, enemy.max_hp);
    let rage = stage == Stage::Rage;
    p.angry = rage;
    // It faces the nearest knight, or the way it last went.
    let nearest = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .min_by(|a, b| (a.x - enemy.x).hypot(a.y - enemy.y).total_cmp(&(b.x - enemy.x).hypot(b.y - enemy.y)));
    let toward = nearest.map(|h| (h.x - enemy.x, h.y - enemy.y));
    let left = match toward {
        Some((dx, _)) if dx.abs() > 0.6 => dx < 0.0,
        _ => enemy.dir.0 < 0.0,
    };
    p.turn = if left { PI } else { 0.0 };
    let up = fight::winding(enemy, boss);
    match (enemy.stage, up.map(|u| u.kind)) {
        (fight::WIND, Some(Windup::Charge)) => {
            let k = (enemy.timer as f32 / up.map_or(30, |u| u.wind).max(1) as f32).min(1.0);
            p.turn = if enemy.dir.0 < 0.0 { PI } else { 0.0 };
            stab_wind(&mut p, k);
        }
        (fight::DASH, _) => {
            p.turn = if enemy.dir.0 < 0.0 { PI } else { 0.0 };
            lunge(&mut p, t);
        }
        (fight::DAZED, Some(Windup::Charge)) => {
            p.turn = if enemy.dir.0 < 0.0 { PI } else { 0.0 };
            stuck(&mut p, enemy.timer as f32 / fight::RECOVER as f32, t);
        }
        (fight::WIND, Some(Windup::Inhale)) => {
            let k = (enemy.timer as f32 / up.map_or(30, |u| u.wind).max(1) as f32).min(1.0);
            honk_wind(&mut p, k, t);
        }
        (fight::DAZED, Some(_)) => honked(&mut p, enemy.timer as f32, t),
        (fight::WIND | fight::JUMP, Some(_)) => {
            // Another charge-up, should a file give it one: it rears.
            let k = (enemy.timer as f32 / up.map_or(30, |u| u.wind).max(1) as f32).min(1.0);
            honk_wind(&mut p, k * 0.6, t);
        }
        _ => {
            let walking = toward.is_some_and(|(dx, dy)| dx.hypot(dy) > enemy.radius() + 2.4) && enemy.age >= TELEGRAPH;
            if walking {
                strut(&mut p, t);
            } else {
                idle(&mut p, t, seed);
            }
            // The fan: wings up before it, the gust on it.
            if let Some(fan) = boss.attacks.first().filter(|a| a.from <= stage) {
                let every = every_at(fan.every, stage);
                let after = since(enemy.age, 0, every);
                gust(&mut p, after, every);
            }
            // The rage's spiral: a pirouette round each volley.
            if rage && let Some(spin) = boss.attacks.get(1).filter(|a| a.from <= stage) {
                let every = every_at(spin.every, stage);
                let after = since(enemy.age, 11, every);
                pirouette(&mut p, after, every, t);
            }
        }
    }
    // Struck: it flinches, eyes wide, feathers up.
    if enemy.hurt > 0 && !p.stuck {
        let k = f32::from(enemy.hurt) / 6.0;
        p.head.0 -= 3.0 * k;
        p.head.1 += 1.5 * k;
        p.tilt -= 0.3 * k;
        p.ruffle = p.ruffle.max(0.8 * k);
        p.eye = Eye::Wide;
        p.jitter.0 -= k;
    }
    p
}

/// Standing about on one leg: the head bobbing and turning a little; now
/// and then it shakes its feathers out, and every so often it changes legs.
fn idle(p: &mut Pose, t: f32, seed: u32) {
    let bob = (t * 0.13).sin();
    p.head.0 += bob * 1.2;
    p.head.1 += (t * 0.26).sin().abs() * 1.2;
    p.tilt = 0.25 + (t * 0.07).sin() * 0.12;
    // Changing legs: both down a moment, then the other up.
    let shift = ((t as u32 + seed % 97) % 240) as f32;
    if shift < 14.0 {
        let k = ease::bump(shift / 14.0);
        p.legs[1] = Leg { knee: (lerp(TUCKED.knee.0, -1.5, k), lerp(TUCKED.knee.1, 11.0, k)), foot: (lerp(TUCKED.foot.0, -2.0, k), lerp(TUCKED.foot.1, 21.0, k)) };
        p.drop = k;
    } else if shift >= 120.0 {
        p.legs = [TUCKED, STRAIGHT];
    }
    // The ruffle.
    let r = ((t as u32 + seed % 61) % 150) as f32;
    if r < 16.0 {
        let k = ease::bump(r / 16.0);
        p.ruffle = k;
        p.wings = 0.18 * k;
        p.jitter.0 = if (t as u32) % 2 == 0 { k } else { -k };
        p.head.1 += 1.5 * k;
        p.eye = Eye::Shut;
    }
}

/// The strut: a high step each half second, its body bobbing on the
/// step and its head going forward and back like a pigeon's.
fn strut(p: &mut Pose, t: f32) {
    let phase = (t / 28.0).fract();
    p.legs = [stride(phase, 3.5, 1.0), stride(phase + 0.5, 3.5, 1.0)];
    let step = (phase * TAU * 2.0).sin();
    p.drop = 0.6 + step * 0.6;
    p.head.0 += 2.5 * (phase * TAU * 2.0 + 1.2).sin();
    p.head.1 += 0.8 * step.abs();
    p.pitch = 0.04;
    p.tilt = 0.28;
}

/// Ticks before the fan the wings rise, and after it they fold away.
const GUST_RISE: u32 = 14;
const GUST_FOLD: u32 = 16;

/// The wing-gust about the fan, `after` ticks since it last loosed.
fn gust(p: &mut Pose, after: u32, every: u32) {
    let before = every - after;
    if before <= GUST_RISE && after != 0 {
        // Up and spread, the flight feathers fanned.
        let k = ease::out(1.0 - before as f32 / GUST_RISE as f32);
        p.wings = p.wings.max(k);
        p.sweep = 0.0;
        p.head.0 -= 2.0 * k;
        p.tilt -= 0.1 * k;
        p.pitch -= 0.08 * k;
    } else if after < GUST_FOLD {
        // The beat: swept forward hard, then folded.
        let k = after as f32 / GUST_FOLD as f32;
        let sweep = ease::out((k * 3.0).min(1.0));
        p.wings = p.wings.max(1.0 - ease::both(((k - 0.35) / 0.65).max(0.0)));
        p.sweep = sweep * (1.0 - ease::both(((k - 0.5) / 0.5).max(0.0)));
        p.head.0 += 2.0 * (1.0 - k);
        p.pitch += 0.12 * (1.0 - k);
        p.ruffle = p.ruffle.max(0.4 * (1.0 - k));
    }
}

/// Ticks of the pirouette before its spiral looses, and after.
const SPIN_LEAD: u32 = 8;
const SPIN_TAIL: u32 = 14;

fn spinning(after: u32, every: u32) -> Option<f32> {
    let before = every - after;
    if after == 0 || before <= SPIN_LEAD {
        let into = if after == 0 { SPIN_LEAD } else { SPIN_LEAD - before };
        Some(into as f32 / (SPIN_LEAD + SPIN_TAIL) as f32)
    } else if after < SPIN_TAIL {
        Some((SPIN_LEAD + after) as f32 / (SPIN_LEAD + SPIN_TAIL) as f32)
    } else {
        None
    }
}

/// The pirouette: up on one leg, the other drawn up to its knee, wings out
/// to the sides, a turn and a half and back to facing where it was.
fn pirouette(p: &mut Pose, after: u32, every: u32, t: f32) {
    let Some(k) = spinning(after, every) else {
        // Between spins it is cross: feathers up, a shiver.
        p.ruffle = p.ruffle.max(0.25);
        p.jitter.0 += if (t as u32) % 4 < 2 { 0.4 } else { -0.4 };
        return;
    };
    p.turn += ease::both(k) * TAU * 1.5 + PI * 0.0;
    p.legs = [STRAIGHT, Leg { knee: (5.0, 6.0), foot: (0.5, 11.0) }];
    p.wings = 0.0;
    p.sweep = 0.0;
    p.out = true;
    p.head = (1.0, -17.0);
    p.tilt = -0.15;
    p.pitch = -0.1;
    p.drop = -1.0 * ease::bump(k);
    p.ruffle = 0.3;
    p.eye = Eye::Shut;
}

/// STAB, winding: down on bent legs, the neck coiling back like a spring
/// with the beak level along the lane, wings half up; shaking at the last.
fn stab_wind(p: &mut Pose, k: f32) {
    let e = ease::out(k);
    p.legs = [Leg { knee: (3.0 * e + 0.5, 11.0 - 1.0 * e), foot: (2.0 * e, 21.0) }, Leg { knee: (-0.5 + 2.0 * e, 11.0), foot: (-3.0, 21.0) }];
    p.drop = 5.0 * e;
    p.pitch = -0.18 * e;
    // The last of it: one more pull back before it lets go.
    let pull = ease::bump(((k - 0.7) / 0.3).clamp(0.0, 1.0)) * 2.0;
    p.head = (lerp(REST_HEAD.0, -9.0, e) - pull, lerp(REST_HEAD.1, -10.0, e));
    p.bow = 4.0 * e;
    p.tilt = lerp(0.25, -0.05, e);
    p.wings = 0.25 * e;
    p.ruffle = 0.3 * e;
    p.angry = true;
    if k > 0.6 {
        p.jitter = (if (p.tick % 2) == 0 { 1.0 } else { -1.0 }, 0.0);
    }
}

/// Down the lane: neck out flat like a spear, body leaning in, the legs
/// a blur under it, wings swept back.
fn lunge(p: &mut Pose, t: f32) {
    let phase = (t / 8.0).fract();
    p.legs = [stride(phase, 6.0, 0.7), stride(phase + 0.5, 6.0, 0.7)];
    p.drop = 2.0;
    p.pitch = 0.32;
    p.head = (15.0, -1.0);
    p.bow = -6.0;
    p.tilt = -0.25;
    p.wings = 0.45;
    p.sweep = -0.6;
    p.ruffle = 0.5;
    p.angry = true;
}

/// Its beak in the floor after the lunge, tail up and legs kicking, until
/// it yanks it out at the last and shakes its head.
fn stuck(p: &mut Pose, k: f32, t: f32) {
    if k < 0.72 {
        let kick = (t * 0.9).sin();
        p.legs = [Leg { knee: (1.0 + kick * 2.0, 10.0), foot: (-1.0 + kick * 4.0, 21.0 - (kick.max(0.0) * 6.0)) }, Leg { knee: (-1.0, 11.0), foot: (-2.0, 21.0) }];
        p.pitch = 0.75;
        p.drop = 2.0;
        p.stuck = true;
        p.wings = 0.25 + 0.2 * (t * 0.6).sin().abs();
        p.eye = Eye::Dizzy;
        p.jitter = (kick * 0.6, 0.0);
    } else {
        // Free, with a spring back up.
        let f = (k - 0.72) / 0.28;
        let s = ease::spring(f);
        p.pitch = 0.75 * (1.0 - s);
        p.head = (lerp(12.0, REST_HEAD.0, s), lerp(4.0, REST_HEAD.1, s));
        p.tilt = 0.25 + 0.6 * (1.0 - s);
        p.eye = if f < 0.6 { Eye::Dizzy } else { Eye::Open };
        p.ruffle = 0.6 * (1.0 - f);
        p.legs = [STRAIGHT, Leg { knee: (-1.0, 11.0), foot: (-2.0, 21.0) }];
    }
}

/// HONK, winding: it rears, swelling with the air it draws in, its beak
/// up and a little open, feathers standing; shaking at the last.
fn honk_wind(p: &mut Pose, k: f32, t: f32) {
    let e = ease::out(k);
    p.puff = e;
    p.ruffle = 0.3 + 0.5 * e;
    p.head = (lerp(REST_HEAD.0, -3.0, e), lerp(REST_HEAD.1, -18.0, e));
    p.bow = -2.0 * e;
    p.tilt = lerp(0.25, -0.7, e);
    p.gape = 0.35 * e;
    p.pitch = -0.2 * e;
    p.legs = [STRAIGHT, Leg { knee: (-1.0, 11.0), foot: (-2.5, 21.0) }];
    p.eye = if k > 0.5 { Eye::Shut } else { Eye::Open };
    p.wings = 0.15 * e;
    if k > 0.7 {
        p.jitter = (if (t as u32) % 2 == 0 { 0.8 } else { -0.8 }, 0.0);
    }
}

/// The honk itself, `a` ticks after: beak wide, neck thrust out, the air
/// going out of it; then down again, a little dazed.
fn honked(p: &mut Pose, a: f32, t: f32) {
    let out = (1.0 - a / 12.0).clamp(0.0, 1.0);
    p.puff = (1.0 - a / 8.0).clamp(0.0, 1.0);
    p.gape = if a < 14.0 { 1.0 } else { (1.0 - (a - 14.0) / 6.0).max(0.0) };
    p.head = (lerp(REST_HEAD.0, 9.0, out.max(0.4)), lerp(REST_HEAD.1, -14.0, out.max(0.4)));
    p.tilt = lerp(0.2, -0.35, out.max(0.5));
    p.bow = 2.0 * out;
    p.wings = 0.7 * out;
    p.sweep = 0.4 * out;
    p.ruffle = 0.5 * out;
    p.pitch = 0.1 * out;
    p.legs = [STRAIGHT, Leg { knee: (-1.0, 11.0), foot: (-2.5, 21.0) }];
    p.eye = if a < 14.0 { Eye::Shut } else { Eye::Dizzy };
    if a < 10.0 {
        p.jitter = (if (t as u32) % 2 == 0 { 1.0 } else { -1.0 }, if (t as u32) % 3 == 0 { -1.0 } else { 0.0 });
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ─── the drawing ─────────────────────────────────────────────────────────────

fn add(a: V, b: V) -> V {
    (a.0 + b.0, a.1 + b.1)
}

fn rot(v: V, a: f32) -> V {
    let (s, c) = a.sin_cos();
    (v.0 * c - v.1 * s, v.0 * s + v.1 * c)
}

fn bez(p: [V; 4], t: f32) -> V {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (a * p[0].0 + b * p[1].0 + c * p[2].0 + d * p[3].0, a * p[0].1 + b * p[1].1 + c * p[2].1 + d * p[3].1)
}

/// The picture and the turn it is drawn at: a point of the bird's own
/// side (`x` forward, `y` down, `z` toward us when it faces right) goes to
/// the picture turned about its standing leg.
struct Paint {
    im: Img,
    c: f32,
    s: f32,
    off: V,
}

impl Paint {
    fn new(turn: f32, off: V) -> Paint {
        Paint { im: Img::new(W, H), c: turn.cos(), s: turn.sin(), off }
    }

    /// A point of the bird to the picture.
    fn p(&self, x: f32, y: f32, z: f32) -> V {
        (x * self.c - z * self.s + self.off.0, y + self.off.1)
    }

    /// How near us a point of the bird stands.
    fn near(&self, x: f32, z: f32) -> f32 {
        x * self.s + z * self.c
    }

    fn px(&self, p: V) -> (i32, i32) {
        ((FOOT.0 + p.0).round() as i32, (FOOT.1 + p.1).round() as i32)
    }

    fn dot(&mut self, p: V, ch: char) {
        let (x, y) = self.px(p);
        self.im.put(x, y, ch);
    }

    fn seg(&mut self, a: V, b: V, ch: char) {
        let (a, b) = (self.px(a), self.px(b));
        self.im.line(a.0, a.1, b.0, b.1, ch);
    }

    fn blob(&mut self, c: V, rx: f32, ry: f32, ch: char) {
        self.im.ellipse(FOOT.0 + c.0, FOOT.1 + c.1, rx.max(0.5), ry.max(0.5), ch);
    }

    /// A shaded egg round `c` (half-sizes `rx`, `ry`, leaning `tilt`):
    /// lit from the upper left down `ramp`, dark at its far rim, and on a
    /// glossy one a hard white highlight, as paint on plastic shines.
    #[allow(clippy::too_many_arguments)]
    fn egg(&mut self, c: V, rx: f32, ry: f32, tilt: f32, ramp: &[char], gloss: bool, fluff: Option<(f32, u32)>) {
        let (cx, cy) = (FOOT.0 + c.0, FOOT.1 + c.1);
        let r = rx.max(ry) + 3.0;
        let (st, ct) = tilt.sin_cos();
        let l = norm(-0.5, -0.75, 0.6);
        let h = norm(l.0, l.1, l.2 + 1.0);
        let n = ramp.len();
        for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let (lx, ly) = (dx * ct + dy * st, -dx * st + dy * ct);
                // Feathers standing up break its edge into tufts.
                let edge = fluff.map_or(1.0, |(f, seed)| {
                    let a = ly.atan2(lx);
                    let tuft = ((a * 7.0 + (seed % 7) as f32).sin() * 0.5 + 0.5).powi(3);
                    1.0 + f * 0.28 * tuft
                });
                let (u, v) = (lx / (rx * edge), ly / (ry * edge));
                let d = u * u + v * v;
                if d > 1.0 {
                    continue;
                }
                let z = (1.0 - d).max(0.0).sqrt();
                // The light and the normal in the picture's frame.
                let (nx, ny) = (u * ct - v * st, u * st + v * ct);
                let lam = (nx * l.0 + ny * l.1 + z * l.2).max(0.0);
                let mut i = ((lam * 1.15) * (n as f32 - 0.01)) as usize;
                if d > 0.8 && lam < 0.5 {
                    i = i.min(1);
                } else if d > 0.9 {
                    i = i.min(n - 2);
                }
                if gloss && (nx * h.0 + ny * h.1 + z * h.2) > 0.975 {
                    self.im.put(x, y, 'H');
                    continue;
                }
                self.im.put(x, y, ramp[i.min(n - 1)]);
            }
        }
    }
}

fn norm(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let l = (x * x + y * y + z * z).sqrt().max(1e-4);
    (x / l, y / l, z / l)
}

/// Below this a wing lies folded, lifted a little; from it, it opens.
const OPEN: f32 = 0.32;

/// What is drawn, back to front.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Wing(f32),
    Leg(usize),
    Tail,
    Body,
    Fold(f32),
    Neck,
}

/// The flamingo in `p`, on its picture with its standing foot at the
/// bottom middle, trimmed to what is drawn.
fn draw(p: &Pose) -> Img {
    let mut pt = Paint::new(p.turn, p.jitter);
    let puff = 1.0 + p.puff * 0.45;
    let body = (BODY.0, BODY.1 + p.drop - p.puff * 2.0);
    let hip = (0.0, HIP + p.drop);
    let tilt = p.pitch;
    let shoulder = add(body, rot(SHOULDER, tilt));
    let mut parts = vec![
        (pt.near(0.0, -1.2), 1, Part::Leg(1)),
        (pt.near(0.0, 1.2), 1, Part::Leg(0)),
        (pt.near(-12.0, 0.0) - 0.5, 2, Part::Tail),
        (0.0, 3, Part::Body),
        (pt.near(-2.0, 6.0), 4, Part::Fold(1.0)),
        (pt.near(-2.0, -6.0), 4, Part::Fold(-1.0)),
        // In front of a raised wing, so the head shows through a flap.
        (pt.near(shoulder.0 + 2.0, 0.0).max(-3.0) + 4.6 * pt.c.abs(), 5, Part::Neck),
    ];
    if p.wings >= OPEN || p.out {
        parts.push((pt.near(0.0, 4.0) + 0.5, 6, Part::Wing(1.0)));
        parts.push((pt.near(0.0, -4.0) - 0.5, 0, Part::Wing(-1.0)));
    }
    parts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for &(_, _, part) in &parts {
        match part {
            Part::Leg(i) => leg(&mut pt, p, hip, i),
            Part::Tail => tail(&mut pt, p, body, tilt),
            Part::Body => {
                let c = pt.p(body.0, body.1, 0.0);
                let rx = ((RX * puff * pt.c).powi(2) + (5.5 * puff * pt.s).powi(2)).sqrt();
                let fluff = (p.ruffle > 0.05).then_some((p.ruffle, p.seed ^ (p.tick / 3)));
                pt.egg(c, rx, RY * puff, tilt * pt.c, &PINK, true, fluff);
                if p.plastic {
                    // Paint on plastic: a long shine along its back.
                    let a = pt.p(body.0 - 6.0, body.1 - RY + 1.6, 0.0);
                    let b = pt.p(body.0 + 3.0, body.1 - RY + 1.2, 0.0);
                    pt.seg(a, b, '+');
                    pt.seg(add(a, (2.0 * pt.c, 0.0)), add(b, (-2.0 * pt.c, 0.0)), 'H');
                }
                // Its belly, a shade deeper where the legs go in.
                let belly = pt.p(body.0 + 1.0, body.1 + RY * puff - 1.2, 0.0);
                pt.seg(add(belly, (-3.0 * pt.c.abs(), 0.0)), add(belly, (3.0 * pt.c.abs(), 0.0)), '?');
            }
            Part::Fold(side) => {
                if p.wings < OPEN && !p.out && pt.near(0.0, side * 6.0) > 0.3 {
                    folded(&mut pt, p, body, tilt, side, puff);
                }
            }
            Part::Neck => neck_and_head(&mut pt, p, shoulder),
            Part::Wing(side) => wing(&mut pt, p, body, tilt, side),
        }
    }
    let mut im = pt.im;
    im.outline_outside('k');
    trim(&im)
}

/// `im` cut to what is drawn on it, its foot column kept in the middle
/// and a pixel spare round it for a rim to light.
fn trim(im: &Img) -> Img {
    let (mut x0, mut x1, mut y0) = (im.w, 0, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some() {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
            }
        }
    }
    if x0 > x1 {
        return Img::new(1, 1);
    }
    let foot = FOOT.0 as i32;
    let half = (foot - x0).max(x1 - foot) + 1;
    let top = (y0 - 1).max(0);
    Img::from_view(im, foot - half, top, half * 2 + 1, im.h - top)
}

/// A wire leg from the hip: thigh to the knee, shin to the foot, the joint
/// a knob, and the clod of lawn it came out of on its end.
fn leg(pt: &mut Paint, p: &Pose, hip: V, i: usize) {
    let z = if i == 0 { 1.2 } else { -1.2 };
    let far = pt.near(0.0, z) < 0.0;
    if p.plastic {
        // Two plain rods down into the lawn.
        let x = if i == 0 { 1.5 } else { -1.5 };
        let (a, b) = (pt.p(hip.0 + x, hip.1, z), pt.p(x * 1.4, 0.0, z));
        pt.seg(a, b, if far { 'G' } else { 'h' });
        return;
    }
    let l = p.legs[i];
    let at = |pt: &Paint, v: V| pt.p(hip.0 + v.0, hip.1 + v.1, z);
    let (h, k, f) = (at(pt, (0.0, 0.0)), at(pt, l.knee), at(pt, l.foot));
    let (wire, lit) = if far { ('G', 'J') } else { ('h', 'i') };
    pt.seg(h, k, wire);
    pt.seg(k, f, wire);
    // A glint down the near side of the wire.
    if !far {
        let m = ((h.0 + k.0) / 2.0, (h.1 + k.1) / 2.0);
        pt.dot(m, lit);
    }
    pt.dot(k, if far { 'j' } else { 'J' });
    pt.dot(add(k, (0.0, -1.0)), lit);
    sod(pt, f, far, p.seed ^ i as u32);
}

/// A clod of lawn on a foot: earth, a lip of turf and a few blades.
fn sod(pt: &mut Paint, f: V, far: bool, seed: u32) {
    let (earth, dark, grass, blade) = if far { ('I', 'b', 'l', 'A') } else { ('P', 'I', 'A', 'y') };
    pt.blob(add(f, (0.0, -0.3)), 2.6, 1.4, dark);
    pt.blob(add(f, (-0.3, -0.7)), 2.0, 1.0, earth);
    pt.seg(add(f, (-2.0, -1.5)), add(f, (2.0, -1.5)), grass);
    for k in 0..3 {
        let x = -1.5 + k as f32 * 1.5 + (seed % 3) as f32 * 0.3;
        pt.dot(add(f, (x, -2.5 - (k % 2) as f32)), blade);
    }
}

/// Its tail: three plumes up off its back, the black of the folded flight
/// feathers under them.
fn tail(pt: &mut Paint, p: &Pose, body: V, tilt: f32) {
    let flick = (p.tick as f32 * 0.21 + p.seed as f32).sin() * 0.08 + p.ruffle * 0.2;
    let base = add(body, rot((-RX + 2.0, -1.5), tilt));
    let fan: &[(f32, f32)] = if p.plastic { &[(-2.75, 6.5), (-2.95, 6.0)] } else { &[(-2.6, 6.5), (-2.85, 7.5), (-3.1, 6.0), (-2.35, 5.0)] };
    for (k, &(a, len)) in fan.iter().enumerate() {
        let a = a + tilt - flick * (k as f32 - 1.0);
        // A plume: full at the root, curling to a point.
        for g in 0..=8 {
            let t = g as f32 / 8.0;
            let along = add(base, (a.cos() * len * t, a.sin() * len * t - t * t * 0.8));
            let q = pt.p(along.0, along.1, 0.0);
            let r = lerp(1.5, 0.4, t);
            pt.blob(q, r, r, if t > 0.7 { '%' } else if k % 2 == 0 { ';' } else { '?' });
        }
        let tip = add(base, (a.cos() * len, a.sin() * len - 0.8));
        pt.dot(pt.p(tip.0, tip.1, 0.0), '+');
    }
    // The flight feathers' black, below.
    let under = add(body, rot((-RX + 2.0, 1.5), tilt));
    let end = add(under, rot((-6.0, 0.5), tilt));
    let (a, b) = (pt.p(under.0, under.1, 0.0), pt.p(end.0, end.1, 0.0));
    pt.seg(a, b, 'g');
    pt.seg(add(a, (0.0, -1.0)), add(b, (0.5, -1.0)), 'j');
}

/// The wing folded on its side `side`: a teardrop of feathers, scalloped
/// in rows, its black tips down over the tail.
fn folded(pt: &mut Paint, p: &Pose, body: V, tilt: f32, side: f32, puff: f32) {
    let lift = (p.wings / OPEN).min(1.0) * 3.0;
    let z = side * 5.0;
    for s in 0..=26 {
        let f = s as f32 / 26.0;
        let x = 6.0 - f * 19.0;
        let half = 3.6 * (f * PI).sin().powf(0.6) * puff;
        let y = -1.0 - f * 1.0 - lift * (1.0 - f);
        for d in -(half.ceil() as i32)..=(half.ceil() as i32) {
            let dy = d as f32;
            if dy.abs() > half {
                continue;
            }
            let at = add(body, rot((x, y + dy), tilt));
            let q = pt.p(at.0, at.1, z);
            // Rows of feathers lying over each other, each edge a curve.
            let row = (x - 0.3 * dy * dy).rem_euclid(3.2) < 1.0;
            let ink = if p.plastic {
                // A moulded wing: one smooth piece, its rim raised.
                if dy < -half + 1.0 { '%' } else if dy > half - 1.0 || f > 0.92 { '?' } else { ';' }
            } else if f > 0.8 {
                if dy > 0.0 { 'g' } else { 'j' }
            } else if dy < -half + 1.0 {
                '%'
            } else if row {
                '?'
            } else {
                ';'
            };
            pt.dot(q, ink);
        }
    }
}

/// A wing open on its side `side`: a fan of feathers off the shoulder,
/// blush at the coverts and black at the flight feathers, raised by
/// `wings`, swept forward by `sweep`; or held straight out (a pirouette).
fn wing(pt: &mut Paint, p: &Pose, body: V, tilt: f32, side: f32) {
    let far = pt.near(0.0, side * 4.0) < 0.0;
    let root = add(body, rot((if p.out { 3.0 } else { -1.0 }, -4.0), tilt));
    // Its flight feathers are black, drawn in the charcoals so they read
    // against a dark floor too.
    let (inner, mid, light, tip, edge) = if far { ('!', '?', ';', 'X', 'g') } else { ('?', ';', '%', 'g', 'j') };
    let rays = 26;
    if p.out {
        // Out to the side, a little up, as a dancer holds her arms.
        for r in 0..=rays {
            let f = r as f32 / rays as f32;
            let reach = 16.0 * (0.55 + 0.45 * (f * PI).sin());
            let lift = -6.0 + f * 6.0;
            let back = (f - 0.5) * 9.0;
            let n = reach.ceil() as i32;
            for g in 0..=n {
                let k = g as f32 / reach;
                let (x, y, z) = (root.0 + back * k, root.1 + lift * k - 2.0 * (k * PI).sin(), side * (2.0 + reach * k));
                let q = pt.p(x, y, z);
                let ink = if k > 0.62 { if r % 4 == 0 { edge } else { tip } } else if r < 3 { light } else if k < 0.25 { inner } else { mid };
                pt.dot(q, ink);
            }
        }
        return;
    }
    // Up and spread: a fan from straight up (its leading edge) back to
    // along its back (its trailing one), folding down onto the back as it
    // closes and turning forward with the sweep; the far one behind and
    // higher.
    let raise = ((p.wings - OPEN) / (1.0 - OPEN)).clamp(0.0, 1.0) * 0.75 + 0.25;
    let lead = -FRAC_PI_2 - 0.1 - (1.0 - raise) * 1.1 + p.sweep * 1.45 + if far { -0.35 } else { 0.0 };
    let span = 0.35 + raise * 1.1;
    let len = 9.0 + raise * 10.0;
    for r in 0..=rays {
        let f = r as f32 / rays as f32;
        let a = lead - span * f + tilt;
        // Each flight feather's tip a little round, so the edge scallops.
        let notch = if r % 3 == 0 { 1.4 } else if r % 3 == 2 { 0.5 } else { 0.0 };
        let reach = len * (0.72 + 0.28 * (f * PI * 0.85).sin()) - notch;
        let n = reach.ceil() as i32;
        for g in 0..=n {
            let k = g as f32 / reach.max(1.0);
            let at = add(root, (a.cos() * reach * k, a.sin() * reach * k));
            let q = pt.p(at.0, at.1, side * (2.0 + k * 3.0));
            let ink = if k > 0.56 {
                if r % 3 == 1 { edge } else { tip }
            } else if r <= 2 || (k > 0.44 && r % 2 == 0) {
                light
            } else if k < 0.2 {
                inner
            } else {
                mid
            };
            pt.dot(q, ink);
        }
    }
}

/// The neck from the shoulder in its S to the head, the head, its eye,
/// and the beak.
fn neck_and_head(pt: &mut Paint, p: &Pose, shoulder: V) {
    let head = add(shoulder, p.head);
    let stuck = p.stuck;
    // The beak in the floor: the head down at the ground ahead of it.
    let head = if stuck { (shoulder.0 + 9.0, -4.5) } else { head };
    let ctrl = if stuck {
        [shoulder, add(shoulder, (6.0, -6.0)), add(head, (2.0, -9.0)), head]
    } else {
        let d = (head.0 - shoulder.0, head.1 - shoulder.1);
        let len = d.0.hypot(d.1).max(1.0);
        let (ux, uy) = (d.0 / len, d.1 / len);
        // Bowed out in front low down and back behind the head: the S a
        // lawn flamingo's neck is cast in.
        let (nx, ny) = (-uy, ux);
        let b = (6.0 + p.bow).max(0.0);
        [
            shoulder,
            add(shoulder, (d.0 * 0.25 + nx * b, d.1 * 0.25 + ny * b)),
            add(head, (-d.0 * 0.3 - nx * b * 1.25, -d.1 * 0.3 - ny * b * 1.25)),
            head,
        ]
    };
    let n = 28;
    let pts: Vec<V> = (0..=n).map(|i| bez(ctrl, i as f32 / n as f32)).collect();
    let r_at = |i: usize| lerp(2.4, 1.5, i as f32 / n as f32);
    // Its shadow side, the body of it, the lit side, a gloss down it.
    for (i, &q) in pts.iter().enumerate() {
        let q = pt.p(q.0, q.1, 0.0);
        pt.blob(add(q, (0.5, 0.6)), r_at(i), r_at(i), '?');
    }
    for (i, &q) in pts.iter().enumerate() {
        let q = pt.p(q.0, q.1, 0.0);
        pt.blob(q, r_at(i) - 0.35, r_at(i) - 0.35, ';');
    }
    for (i, &q) in pts.iter().enumerate() {
        let q = pt.p(q.0, q.1, 0.0);
        pt.blob(add(q, (-0.7, -0.5)), r_at(i) * 0.45, r_at(i) * 0.45, '%');
    }
    for (i, &q) in pts.iter().enumerate().filter(|(i, _)| i % 3 == 1 && *i < n - 3) {
        let q = pt.p(q.0, q.1, 0.0);
        pt.dot(add(q, (-r_at(i) * 0.6, -r_at(i) * 0.4)), '+');
    }
    // The head.
    let hq = pt.p(head.0, head.1, 0.0);
    let face = pt.c;
    pt.egg(hq, 3.4 + 0.3 * (1.0 - face.abs()), 3.0, 0.0, &PINK, true, None);
    // The beak: thick and pale at the face, bent down hard at its middle,
    // its tip black; its lower half dropping open.
    let a = if stuck { 1.35 } else { p.tilt };
    let dir = |a: f32| (a.cos() * pt.c, a.sin());
    let base = add(hq, (2.6 * pt.c, 0.7));
    let bend = add(base, (dir(a).0 * 3.6, dir(a).1 * 3.6));
    let tip = add(bend, (dir(a + 1.05).0 * 4.0, dir(a + 1.05).1 * 4.0));
    let gape = p.gape * 0.8;
    if gape > 0.05 {
        let lo_bend = add(base, (dir(a + gape).0 * 3.0, dir(a + gape).1 * 3.0 + 1.0));
        let lo_tip = add(lo_bend, (dir(a + gape + 0.9).0 * 2.8, dir(a + gape + 0.9).1 * 2.8));
        // The mouth inside, dark.
        pt.seg(add(base, (0.0, 1.0)), add(bend, (0.0, 1.0)), '!');
        pt.seg(base, lo_bend, 'h');
        pt.seg(lo_bend, lo_tip, 'K');
        pt.seg(add(base, (0.0, 0.5)), lo_bend, 'i');
    } else {
        pt.seg(add(base, (0.0, 1.0)), add(bend, (0.0, 1.0)), 'h');
    }
    pt.seg(base, bend, 'H');
    pt.seg(add(base, (0.0, -0.6)), add(bend, (-0.5 * pt.c, -0.6)), 'H');
    pt.seg(bend, tip, 'K');
    pt.seg(add(bend, (0.0, 1.0)), add(tip, (-0.6 * pt.c, 0.0)), 'k');
    pt.dot(bend, 'i');
    pt.dot(tip, 'k');
    // Its eye, on whichever side looks at us; both when it faces us.
    for side in [1.0f32, -1.0] {
        let z = side * 1.6;
        if pt.near(0.5, z) < -0.2 {
            continue;
        }
        let e = add(pt.p(head.0 + 0.6, head.1 - 0.6, z), (0.0, 0.0));
        eye(pt, e, p, pt.c.abs() < 0.5);
    }
}

fn eye(pt: &mut Paint, e: V, p: &Pose, front: bool) {
    let toward = if pt.c >= 0.0 { 1.0 } else { -1.0 };
    if p.plastic {
        // A painted eye: a black dot with its shine painted on.
        pt.blob(e, 1.0, 1.0, 'k');
        pt.dot(add(e, (-0.5 * toward, -0.5)), 'W');
        return;
    }
    match p.eye {
        Eye::Open | Eye::Wide => {
            let wide = p.eye == Eye::Wide;
            pt.blob(e, if wide { 1.5 } else { 1.1 }, if wide { 1.5 } else { 1.1 }, 'H');
            let look = if front { (0.0, 0.0) } else { (0.5 * toward, 0.0) };
            pt.dot(add(e, look), 'k');
            if wide {
                pt.dot(add(e, (-0.8 * toward, -0.8)), 'W');
            }
        }
        Eye::Shut => {
            pt.seg(add(e, (-1.0, 0.0)), add(e, (1.0, 0.0)), 'k');
            pt.dot(add(e, (-1.0 * toward, -1.0)), '!');
        }
        Eye::Dizzy => {
            pt.blob(e, 1.2, 1.2, 'H');
            let a = p.tick as f32 * 0.7;
            pt.dot(add(e, (a.cos() * 0.8, a.sin() * 0.8)), 'k');
        }
        Eye::Cross => {
            pt.dot(e, 'k');
            for d in [(-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0)] {
                pt.dot(add(e, d), 'k');
            }
        }
    }
    if p.angry && !matches!(p.eye, Eye::Cross) {
        // A brow, down toward the beak: it means it.
        pt.seg(add(e, (-1.6 * toward, -2.4)), add(e, (1.4 * toward, -1.4)), 'k');
    }
}

// ─── where its parts are ─────────────────────────────────────────────────────

/// Where the parts of a pose stand, from its foot, as drawn: its body's
/// middle, its head, its beak's tip, and which way it faces across the
/// picture (1 right, -1 left, near 0 at us).
#[derive(Clone, Copy, Debug)]
struct Joints {
    body: V,
    head: V,
    beak: V,
    face: f32,
}

fn joints(p: &Pose) -> Joints {
    let pt = Paint::new(p.turn, p.jitter);
    let body = (BODY.0, BODY.1 + p.drop - p.puff * 2.0);
    let shoulder = add(body, rot(SHOULDER, p.pitch));
    let head = if p.stuck { (shoulder.0 + 9.0, -4.5) } else { add(shoulder, p.head) };
    let a = if p.stuck { 1.35 } else { p.tilt };
    let dir = |a: f32| (a.cos() * pt.c, a.sin());
    let hq = pt.p(head.0, head.1, 0.0);
    let base = add(hq, (2.6 * pt.c, 0.7));
    let bend = add(base, (dir(a).0 * 3.6, dir(a).1 * 3.6));
    let tip = if p.stuck || p.gape < 0.3 { add(bend, (dir(a + 1.05).0 * 4.0, dir(a + 1.05).1 * 4.0)) } else { bend };
    Joints { body: pt.p(body.0, body.1, 0.0), head: hq, beak: tip, face: pt.c }
}

// ─── what moves round it ─────────────────────────────────────────────────────

/// The most motes the flamingo's own marks paint in a frame.
const BUDGET: u32 = 1400;

/// A feather loosed at `at`, `k` through its fall: a little quill turning
/// as it rocks down.
fn feather(pen: &mut Pen, at: V, k: f32, seed: u32, tick: u32) {
    if !(0.0..1.0).contains(&k) {
        return;
    }
    let rock = ((tick as f32 * 0.25) + seed as f32 * 0.7).sin();
    let (x, y) = ((at.0 + rock * 1.5).round() as i32, at.1.round() as i32);
    // Its quill along the way it hangs, its vane either side.
    let (dx, dy) = if rock > 0.35 { (1, -1) } else if rock < -0.35 { (1, 1) } else { (1, 0) };
    let (nx, ny) = (-dy, dx);
    let fade = |s: f32| FEATHER.at(s + k * 0.45);
    for j in -2..=2 {
        pen.put(x + dx * j, y + dy * j, fade(if j == 0 { 0.0 } else { 0.2 }));
    }
    for j in -1..=1 {
        pen.put(x + dx * j + nx, y + dy * j + ny, fade(0.35));
    }
    if k < 0.6 {
        pen.put(x - dx + -nx, y - dy - ny, fade(0.55));
        pen.put(x + -nx, y - ny, fade(0.5));
    }
}

/// Over the figures: the wind and feathers of its gust, the glint of the
/// beak as it winds up a stab, the lunge's streak and the feathers it
/// sheds, the dirt off its beak and the stars round its head when it is
/// stuck, the air it draws in and the honk going out in rings, the spiral
/// of feathers off its pirouette, and the feathers a hit knocks loose.
pub(in super::super) fn over(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for e in run.enemies.iter().filter(|e| e.hp > 0) {
        let Some(boss) = e.boss.and_then(|i| run.bosses.get(usize::from(i))).filter(|b| is(b)) else { continue };
        let p = pose(run, e, boss, tick);
        let j = joints(&p);
        let (fx, fy) = at(e.x, e.y);
        let w = |v: V| (fx as f32 + v.0, fy as f32 + v.1);
        let (body, head, beak) = (w(j.body), w(j.head), w(j.beak));
        let seed = hash(e.id as i32, e.signature.cycle as i32, 0xf1b);
        let mut pen = Pen::new(cv, BUDGET);
        let few = |n: u32, calm: bool| if calm { n / 2 } else { n };
        let stage = boss.stage(e.hp, e.max_hp);
        let up = fight::winding(e, boss);
        match (e.stage, up.map(|u| u.kind)) {
            (fight::WIND, Some(Windup::Charge)) => {
                let k = (e.timer as f32 / up.map_or(30, |u| u.wind).max(1) as f32).min(1.0);
                // The tip glints, quicker to the last.
                if k > 0.35 {
                    let every = if k > 0.75 { 6 } else { 12 };
                    let ph = (tick % every) as f32 / every as f32;
                    pen.twinkle(beak, 2.0 + k * 2.0, ph, ramps::HOLY);
                }
            }
            (fight::DASH, _) => {
                // A pink streak off the beak, speed lines past the body.
                let d = (e.dir.0, e.dir.1 * 0.9);
                let pts: Vec<V> = (0..14).map(|i| (beak.0 - d.0 * i as f32 * 2.2, beak.1 - d.1 * i as f32 * 2.2 + (i as f32 * 0.9 + tick as f32).sin() * 0.4)).collect();
                pen.ribbon(&pts, FEATHER, 2.2);
                for i in 0..5 {
                    let off = (i as f32 - 2.0) * 3.5;
                    let at = (body.0 - d.0 * (12.0 + (i % 2) as f32 * 4.0) - d.1 * off, body.1 - d.1 * 12.0 + d.0 * off * 0.6);
                    pen.streak(at, (d.0.max(0.01).copysign(d.0), d.1), 7.0 + (i % 3) as f32 * 2.0, GUST, 0.3);
                }
                shed(&mut pen, run, e, up.map_or(26.0, |u| u.speed), 0.0, tick);
            }
            (fight::DAZED, Some(Windup::Charge)) => {
                let k = e.timer as f32 / fight::RECOVER as f32;
                shed(&mut pen, run, e, up.map_or(26.0, |u| u.speed), e.timer as f32, tick);
                let ground = w((j.head.0 + 1.5 * j.face, 0.0));
                if k < 0.72 {
                    // Its beak in a little mound of turned earth.
                    let (gx, gy) = (ground.0 as i32, ground.1 as i32);
                    for (dx, ch) in [(-4, 'b'), (-3, 'I'), (-2, 'P'), (-1, 'P'), (1, 'P'), (2, 'P'), (3, 'I'), (4, 'b')] {
                        pen.put(gx + dx, gy, ch);
                    }
                    for (dx, ch) in [(-3, 'b'), (-2, 'I'), (-1, 'o'), (1, 'o'), (2, 'I'), (3, 'b')] {
                        pen.put(gx + dx, gy - 1, ch);
                    }
                    for (dx, ch) in [(-2, 'M'), (2, 'y')] {
                        pen.put(gx + dx, gy - 2, ch);
                    }
                    // Stars round its head.
                    for s in 0..3 {
                        let a = tick as f32 * 0.22 + s as f32 * TAU / 3.0;
                        let at = (body.0 + a.cos() * 9.0, body.1 - 9.0 + a.sin() * 3.0);
                        pen.twinkle(at, 1.6, ((tick / 2 + s * 5) % 12) as f32 / 12.0, ramps::GOLD);
                    }
                } else {
                    // Out it comes, and the earth with it.
                    let a = (k - 0.72) / 0.28 * fight::RECOVER as f32 * 0.28;
                    let spray = Spray { seed, n: few(12, pen.calm), at: ground, angle: -FRAC_PI_2 - 0.4 * j.face, spread: 2.0, speed: (1.2, 2.8), life: (8.0, 12.0), drag: 0.08, gravity: 0.25, squash: 1.0, stagger: 0.0 };
                    spray.each(a, |m| pfx::mote(&mut pen, &m, Shape::Dot, SOD, 1, run.tick));
                    pen.put(ground.0 as i32, ground.1 as i32, 'b');
                }
            }
            (fight::WIND, Some(Windup::Inhale)) => {
                let k = (e.timer as f32 / up.map_or(30, |u| u.wind).max(1) as f32).min(1.0);
                // The air going into it, streaming to its beak.
                for i in 0..few(18, pen.calm) {
                    let h = hash(i as i32, 3, seed);
                    let life = 10 + h % 8;
                    let ph = ((tick + h % life) % life) as f32 / life as f32;
                    let a = (h % 628) as f32 / 100.0;
                    let r = (34.0 + (h >> 10) as f32 % 18.0) * (1.0 - ease::inward(ph)) * (1.0 - k * 0.25);
                    let at = (beak.0 + a.cos() * r, beak.1 + a.sin() * r * 0.7);
                    pen.streak(at, (-a.cos(), -a.sin() * 0.7), 3.0 + k * 4.0, GUST, ph * 0.8);
                }
            }
            (fight::DAZED, Some(Windup::Inhale)) => honk_marks(&mut pen, &j, (beak, body, head), e.timer as f32, seed, tick, (fx, fy)),
            _ => {
                // The wing-gust about its fan.
                if let Some(fan) = boss.attacks.first().filter(|a| a.from <= stage) {
                    let every = every_at(fan.every, stage);
                    let after = since(e.age, 0, every) as f32;
                    if after < 16.0 {
                        let aim = run
                            .players
                            .values()
                            .filter(|h| h.hp > 0)
                            .min_by(|a, b| (a.x - e.x).hypot(a.y - e.y).total_cmp(&(b.x - e.x).hypot(b.y - e.y)))
                            .map_or(if j.face < 0.0 { PI } else { 0.0 }, |h| (h.y - e.y).atan2(h.x - e.x));
                        gust_marks(&mut pen, body, aim, fan.arc.to_radians(), after, seed, tick);
                    }
                }
                // The pirouette's spiral of feathers.
                if stage == Stage::Rage
                    && let Some(spin) = boss.attacks.get(1).filter(|a| a.from <= stage)
                {
                    let every = every_at(spin.every, stage);
                    let after = since(e.age, 11, every);
                    if let Some(k) = spinning(after, every) {
                        spin_marks(&mut pen, body, p.turn, k, seed, tick);
                    }
                    if after < 24 {
                        spiral_feathers(&mut pen, body, after as f32, seed ^ (e.age / every.max(1)), tick);
                    }
                }
                // Its ruffle: a feather or two let go, drifting down.
                let r = (tick + p.seed % 61) % 150;
                if r < 50 && !p.angry {
                    for i in 0..2u32 {
                        let h = hash(i as i32, (tick + p.seed % 61) as i32 / 150, seed);
                        let k = (r as f32 - i as f32 * 4.0) / 46.0;
                        let at = (body.0 + ((h % 15) as f32 - 7.0), body.1 - 2.0 + ease::out(k.max(0.0)) * 24.0);
                        feather(&mut pen, at, k, h, tick);
                    }
                }
            }
        }
        // A hit knocks a feather or two loose.
        if e.hurt > 0 {
            let k = 6.0 - f32::from(e.hurt);
            let hit = Spray { seed: hash(e.id as i32, e.hp as i32, 0xfe), n: 3, at: (body.0, body.1 - 2.0), angle: -FRAC_PI_2, spread: 3.6, speed: (1.5, 2.6), life: (6.0, 7.0), drag: 0.15, gravity: 0.15, squash: 1.0, stagger: 0.0 };
            hit.each(k, |m| feather(&mut pen, m.at, m.k * 0.7, m.seed, tick));
        }
    }
    fallen_over(cv, run);
}

/// The feathers a lunge sheds down the lane, each let go where it passed
/// and drifting down; `after` ticks since it stopped.
fn shed(pen: &mut Pen, run: &Run, e: &Enemy, speed: f32, after: f32, tick: u32) {
    let Some(r) = e.signature.rails.first() else { return };
    let len = (r.b.0 - r.a.0).hypot(r.b.1 - r.a.1).max(0.1);
    let gone = ((e.x - r.a.0) * (r.b.0 - r.a.0) + (e.y - r.a.1) * (r.b.1 - r.a.1)) / len;
    let per_tick = (speed / 30.0).max(0.05);
    for i in 0..8u32 {
        let f = (i as f32 + 0.5 + unit(i as i32, 5, e.signature.cycle) * 0.6) / 8.0;
        let d = f * len;
        if d > gone {
            continue;
        }
        let age = (gone - d) / per_tick + after;
        let k = age / 34.0;
        let (x, y) = at(r.a.0 + (r.b.0 - r.a.0) * f, r.a.1 + (r.b.1 - r.a.1) * f);
        let h = hash(i as i32, e.signature.cycle as i32, 0x5ed);
        let high = 16.0 + (h >> 8) as f32 % 14.0;
        let at = (x as f32 + ((h % 11) as f32 - 5.0), y as f32 - high + ease::both(k.min(1.0)) * (high - 2.0));
        feather(pen, at, k, h, tick);
    }
    let _ = run;
}

/// The gust off its wings, `after` ticks since its fan: a fan of wind
/// lines going out along `aim` `arc` wide, pink feathers carried in it.
fn gust_marks(pen: &mut Pen, from: V, aim: f32, arc: f32, after: f32, seed: u32, tick: u32) {
    let k = after / 16.0;
    let n = if pen.calm { 5 } else { 9 };
    for i in 0..n {
        let f = i as f32 / (n - 1) as f32 - 0.5;
        let a = aim + f * arc + (unit(i, 2, seed) - 0.5) * 0.12;
        let (ux, uy) = (a.cos(), a.sin() * 0.75);
        let r = 4.0 + ease::out((k * 1.15 - unit(i, 3, seed) * 0.25).max(0.0)) * 50.0 + unit(i, 4, seed) * 10.0;
        let at = (from.0 + ux * r, from.1 + uy * r + (f * 6.0 + after * 0.5).sin());
        pen.streak(at, (ux, uy), 8.0 + 10.0 * (1.0 - k), GUST, k * 0.9);
        // A little curl at the end of every other line.
        if i % 2 == 0 && k > 0.2 {
            let c = (at.0 + uy * 2.0, at.1 - ux * 2.0);
            pen.light(c.0.round() as i32, c.1.round() as i32, GUST.at(0.4 + k * 0.5));
        }
    }
    let feathers = Spray { seed, n: if pen.calm { 4 } else { 8 }, at: from, angle: aim, spread: arc, speed: (2.0, 3.6), life: (13.0, 16.0), drag: 0.1, gravity: 0.04, squash: 0.75, stagger: 0.4 };
    feathers.each(after, |m| feather(pen, m.at, m.k, m.seed, tick));
    // The air pushed off the wings, a soft crescent.
    if after < 8.0 {
        pen.ring((from.0 + aim.cos() * (6.0 + after * 3.0), from.1 + aim.sin() * (4.0 + after * 2.0)), 6.0 + after * 2.0, 1.0, 0.6, GUST, 0.3 + after / 10.0);
    }
}

/// Round the pirouette: ribbons of pink wound round its middle, turning
/// with it, and the air it stirs.
fn spin_marks(pen: &mut Pen, body: V, turn: f32, k: f32, seed: u32, tick: u32) {
    let power = ease::bump(k);
    for arm in 0..3 {
        let a0 = turn + arm as f32 * TAU / 3.0;
        let pts: Vec<V> = (0..12)
            .map(|i| {
                let a = a0 - i as f32 * 0.22;
                let r = 15.0 + i as f32 * 0.6;
                (body.0 + a.cos() * r, body.1 - 2.0 + a.sin() * r * 0.42 - i as f32 * 0.3)
            })
            .collect();
        if power > 0.15 {
            pen.ribbon(&pts, FEATHER, 1.6 * power + 0.6);
        }
    }
    pen.ring((body.0, body.1 + 22.0), 10.0 + 8.0 * power, 1.0, 0.4, FEATHER, 1.0 - power * 0.7);
    let _ = (seed, tick);
}

/// The spiral of feathers its pirouette throws, `after` ticks since its
/// spiral loosed: three arms of them, curling out.
fn spiral_feathers(pen: &mut Pen, body: V, after: f32, seed: u32, tick: u32) {
    for arm in 0..3u32 {
        for i in 0..if pen.calm { 3 } else { 5 } {
            let t = after - i as f32 * 1.5;
            if t < 0.0 {
                continue;
            }
            let k = t / 22.0;
            let a = arm as f32 * TAU / 3.0 + unit(arm as i32, 1, seed) * TAU + k * 2.2;
            let r = 6.0 + ease::out(k) * (34.0 + i as f32 * 3.0);
            let at = (body.0 + a.cos() * r, body.1 + a.sin() * r * 0.6 + k * k * 10.0);
            feather(pen, at, k, hash(arm as i32, i, seed), tick);
        }
    }
}

/// The honk, `a` ticks after: rings of sound out of its beak, the cry
/// over it, and the air knocked out of it all round.
fn honk_marks(pen: &mut Pen, j: &Joints, (beak, body, head): (V, V, V), a: f32, seed: u32, tick: u32, foot: (i32, i32)) {
    if a < 22.0 {
        let face = if j.face >= 0.0 { 1.0 } else { -1.0 };
        for wave in 0..3 {
            let t = a - wave as f32 * 4.0;
            if !(0.0..16.0).contains(&t) {
                continue;
            }
            let r = 4.0 + ease::out(t / 16.0) * 30.0;
            pen.shock((beak.0 + face * r * 0.35, beak.1), r, 1.6, 0.8, HONK, t / 16.0);
        }
        // The blast all round, where its burst goes.
        if a < 12.0 {
            pen.shock((body.0, foot.1 as f32), 8.0 + ease::out(a / 12.0) * 40.0, 2.2, 0.45, GUST, a / 12.0);
        }
        let blown = Spray { seed, n: if pen.calm { 5 } else { 10 }, at: body, angle: 0.0, spread: TAU, speed: (1.5, 3.2), life: (16.0, 22.0), drag: 0.1, gravity: 0.04, squash: 0.7, stagger: 0.0 };
        blown.each(a, |m| feather(pen, m.at, m.k, m.seed, tick));
    }
    if a < 20.0 {
        // HONK!, popping up over it.
        let lift = ease::back((a / 6.0).min(1.0)) * 6.0;
        let word = "HONK!";
        let w = super::super::super::ink::text_width(word);
        let face = if j.face >= 0.0 { 1.0 } else { -1.0 };
        let x = if face > 0.0 { beak.0 + 5.0 } else { beak.0 - 5.0 - w as f32 } as i32;
        let y = (head.1 - 4.0 - lift) as i32;
        let wob = if (tick / 2) % 2 == 0 { 0 } else { 1 };
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (1, 1)] {
            pen.cv.text(x + dx + wob, y + dy, word, 'k');
        }
        pen.cv.text(x + wob, y, word, if a < 10.0 { '+' } else { '%' });
    }
}

// ─── its fall ────────────────────────────────────────────────────────────────

/// Ticks it quivers, and where it hits the floor and goes off in feathers.
const QUIVER: f32 = 8.0;
const POOF: f32 = 24.0;

/// Its body as it lies, if this corpse is the flamingo's: quivering, then
/// toppling, then (from the poof) the plastic ornament it always was,
/// stuck in the ground at an angle, wobbling to a stop.
pub(in super::super) fn corpse(run: &Run, c: &Corpse) -> Option<Img> {
    let boss = c.boss.and_then(|i| run.bosses.get(usize::from(i)))?;
    if !is(boss) {
        return None;
    }
    let age = run.tick.saturating_sub(c.born) as f32;
    let seed = hash(c.born as i32, 3, 0xdea);
    let mut cv = Img::new(CW, CH);
    if age < POOF {
        let mut p = Pose::rest(run.tick as u32, seed);
        p.legs = [STRAIGHT, Leg { knee: (-1.0, 11.0), foot: (-2.0, 21.0) }];
        p.eye = Eye::Cross;
        p.ruffle = 0.4;
        p.head.0 -= 1.0;
        p.tilt = 0.6;
        p.wings = 0.2;
        let mut lean = 0.0;
        if age < QUIVER {
            p.jitter = (if (run.tick % 2) == 0 { 1.0 } else { -1.0 }, 0.0);
            p.eye = Eye::Wide;
        } else {
            // Over it goes, stiff as a board, faster as it falls.
            lean = -ease::inward((age - QUIVER) / (POOF - QUIVER)) * 1.5;
            p.legs = [STRAIGHT, Leg { knee: (0.5, 11.0), foot: (-1.0, 21.0) }];
        }
        let im = draw(&p);
        let pivot = (im.w / 2, im.h - 1);
        stamp_turned(&mut cv, &im, pivot, (CW / 2, CH - 1), lean);
    } else {
        // What is left: the ornament, at its angle, wobbling to rest.
        let t = age - POOF;
        let lean = 0.32 + 0.5 * (-t / 14.0).exp() * (t * 0.45).cos();
        let mut p = Pose::rest(0, seed);
        p.plastic = true;
        p.legs = [STRAIGHT, STRAIGHT];
        p.drop = 4.0;
        p.tilt = 0.3;
        p.pitch = 0.0;
        let im = draw(&p);
        let pivot = (im.w / 2, im.h - 1);
        stamp_turned(&mut cv, &im, pivot, (CW / 2, CH - 4), lean);
        // Stuck in a heap of its own feathers, growing as they come down.
        mound(&mut cv, ((t / 24.0).min(1.0) * 0.6 + 0.4).min(1.0), seed);
    }
    cv.outline_outside('k');
    Some(cv)
}

/// The heap at the stake's foot, `full` of the way piled: feathers over
/// feathers, the lit ones on top, the deep ones under.
fn mound(cv: &mut Img, full: f32, seed: u32) {
    let (cx, cy) = (CW / 2, CH - 1);
    let (rx, ry) = (12.0 * full + 2.0, 4.5 * full + 1.0);
    for y in (cy as f32 - ry * 1.4) as i32..=cy {
        for x in (cx as f32 - rx) as i32..=(cx as f32 + rx) as i32 {
            let (u, v) = ((x - cx) as f32 / rx, (cy - y) as f32 / (ry * 1.4));
            // Lumpy on top, a feather's tip here and there.
            let lump = 1.0 + 0.18 * ((x as f32 * 1.3 + seed as f32).sin() * (x as f32 * 0.7).cos());
            if u * u + v * v > lump {
                continue;
            }
            let h = hash(x, y, seed);
            let up = v + (h % 100) as f32 / 400.0;
            let ink = if up > 0.75 { '+' } else if up > 0.45 { ['%', '+', '%'][h as usize % 3] } else if up > 0.2 { ['%', ';', ';'][h as usize % 3] } else { ['?', ';', '!'][h as usize % 3] };
            cv.put(x, y, ink);
        }
    }
    // Quills sticking out of it.
    for k in 0..(4.0 * full) as i32 + 1 {
        let h = hash(k, 2, seed);
        let x = cx - (rx as i32) + 2 + (h % (rx as u32 * 2).max(1)) as i32;
        let y = cy - 1 - (h >> 8) as i32 % 3;
        cv.put(x, y - 1, '+');
        cv.put(x + if h & 1 == 0 { 1 } else { -1 }, y - 2, '%');
    }
}

/// The corpse's picture: as wide as it lies fallen, as tall as it stands.
const CW: i32 = 112;
const CH: i32 = 58;

/// `src` onto `cv` turned `a` about its pixel `pivot`, which lands on
/// `to`: nearest-neighbour, so its inks stay its own.
fn stamp_turned(cv: &mut Img, src: &Img, pivot: (i32, i32), to: (i32, i32), a: f32) {
    let (s, c) = a.sin_cos();
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (dx, dy) = ((x - to.0) as f32, (y - to.1) as f32);
            let (sx, sy) = (dx * c + dy * s, -dx * s + dy * c);
            if let Some(col) = src.get((sx + pivot.0 as f32).round() as i32, (sy + pivot.1 as f32).round() as i32) {
                cv.set(x, y, col);
            }
        }
    }
}

/// Each fallen flamingo in the room, how long since it fell, and where it
/// lies; while the hand has not yet shut the ground over it.
fn fallen(run: &Run) -> impl Iterator<Item = (f32, (i32, i32), Beat, u32)> + '_ {
    run.spectacle.corpses.iter().filter_map(move |c| {
        let boss = c.boss.and_then(|i| run.bosses.get(usize::from(i)))?;
        let beat = c.beat(run.tick);
        (is(boss) && c.room == run.at && beat != Beat::Gone).then(|| (run.tick.saturating_sub(c.born) as f32, at(c.x, c.y), beat, hash(c.born as i32, 3, 0xdea)))
    })
}

/// Whether a monster's fall at `(x, y)` is the flamingo's, which falls in
/// its own feathers (the flash of it is the room's own).
pub(in super::super) fn owns_fall(run: &Run, x: f32, y: f32) -> bool {
    run.spectacle.corpses.iter().any(|c| {
        c.boss.and_then(|i| run.bosses.get(usize::from(i))).is_some_and(is) && (c.x - x).abs() < 0.5 && (c.y - y).abs() < 0.5
    })
}

/// Under the figures: the heap of its feathers round where it fell, laid
/// down as they land; going as the ground shuts.
pub(in super::super) fn fallen_under(cv: &mut Img, run: &Run) {
    for (age, (x, y), beat, seed) in fallen(run) {
        if age < POOF {
            continue;
        }
        let keep = match beat {
            Beat::Closing(t) => 1.0 - t,
            _ => 1.0,
        };
        let landed = ((age - POOF) / 40.0).min(1.0);
        let n = (16.0 * landed * keep) as i32;
        let mut pen = Pen::new(cv, 400);
        for i in 0..n {
            let h = hash(i, 9, seed);
            // Strewn round the heap, thinning out away from it.
            let r = 10.0 + ((h % 1000) as f32 / 1000.0).powf(0.8) * 18.0;
            let a = ((h >> 10) % 628) as f32 / 100.0;
            let at = (x as f32 + a.cos() * r, y as f32 + a.sin() * r * 0.4 - 1.0);
            feather(&mut pen, at, 0.25 + (h >> 20) as f32 % 4.0 * 0.08, h, 0);
        }
    }
}

/// Over the figures: its last quiver, the crash and the cloud of its own
/// feathers, and the feathers coming down out of it.
fn fallen_over(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for (age, (x, y), _, seed) in fallen(run) {
        let mut pen = Pen::new(cv, BUDGET);
        let (x, y) = (x as f32, y as f32);
        if age < POOF {
            if age > QUIVER {
                // A last honk goes out of it as it goes over.
                let k = (age - QUIVER) / (POOF - QUIVER);
                pen.twinkle((x + 6.0, y - 46.0 + k * 20.0), 2.0, k, FEATHER);
            }
            continue;
        }
        let t = age - POOF;
        let side = 22.0;
        let mid = (x + side * 0.5, y - 8.0);
        if t < 22.0 {
            // The poof: a cloud of pink billowing out and thinning.
            let k = t / 22.0;
            for b in 0..7 {
                let a = b as f32 / 7.0 * TAU + unit(b, 1, seed) * 0.8;
                let r = ease::out(k) * (10.0 + unit(b, 2, seed) * 8.0);
                let c = (mid.0 + a.cos() * r, mid.1 + a.sin() * r * 0.55 - k * 6.0);
                puff(&mut pen, c, 4.0 + 5.0 * ease::out(k) + unit(b, 3, seed) * 2.0, k);
            }
            if t < 6.0 {
                pen.glow(mid, 18.0, 0.8, FEATHER, 1.0 - t / 6.0);
                pfx::rays(&mut pen, mid, 10, 26.0, 0.7, unit(seed as i32, 4, 1) * TAU, FEATHER, t / 6.0);
            }
            pen.shock((mid.0, y), 6.0 + ease::out(k) * 34.0, 2.0, 0.42, FEATHER, ease::inward(k));
        }
        // Feathers thrown up out of it, coming down slowly, rocking.
        let up = Spray { seed, n: if pen.calm { 12 } else { 24 }, at: mid, angle: -FRAC_PI_2, spread: 4.2, speed: (1.8, 4.6), life: (40.0, 64.0), drag: 0.16, gravity: 0.012, squash: 1.0, stagger: 0.0 };
        up.each(t, |m| feather(&mut pen, m.at, m.k, m.seed, tick));
    }
}

/// A billow of the poof: pale pink on its lit top, rose under, breaking
/// up through the dither as it goes.
fn puff(pen: &mut Pen, c: V, r: f32, k: f32) {
    let ri = r.ceil() as i32;
    let fill = (1.0 - k).powf(0.9);
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
            if d > 1.0 {
                continue;
            }
            let (px, py) = ((c.0 + dx as f32).round() as i32, (c.1 + dy as f32).round() as i32);
            if bayer(px, py) >= fill * (1.15 - d * 0.5) {
                continue;
            }
            let lit = -(dx as f32 + dy as f32 * 1.4) / r;
            let ink = if lit > 0.6 { '+' } else if lit > -0.1 { '%' } else if lit > -0.7 { ';' } else { '?' };
            pen.put(px, py, ink);
        }
    }
}

// ─── the hooks ───────────────────────────────────────────────────────────────

/// Whether `boss` is the Lawn Flamingo.
pub(in super::super) fn is(boss: &Boss) -> bool {
    boss.id == FLAMINGO
}

/// The flamingo `enemy` is, drawn, if it is the flamingo.
pub(in super::super) fn figure(run: &Run, enemy: &Enemy, boss: &Boss, tick: u32) -> Option<Img> {
    if !is(boss) {
        return None;
    }
    Some(draw(&pose(run, enemy, boss, tick)))
}

/// Every pose it has, a row of each, for the review sheet.
#[cfg(test)]
pub(in super::super) fn sheet() -> Img {
    let poses: Vec<Pose> = {
        let mut v = Vec::new();
        for t in [0u32, 12, 24, 36] {
            let mut p = Pose::rest(t, 3);
            idle(&mut p, t as f32, 3);
            v.push(p);
        }
        for k in 0..4 {
            let mut p = Pose::rest(k * 7, 3);
            strut(&mut p, (k * 7) as f32);
            v.push(p);
        }
        for after in [74u32, 79, 2, 8] {
            let mut p = Pose::rest(0, 3);
            gust(&mut p, after, 80);
            v.push(p);
        }
        for k in [0.0f32, 0.4, 0.8, 1.0] {
            let mut p = Pose::rest(0, 3);
            stab_wind(&mut p, k);
            v.push(p);
        }
        for (k, t) in [(0.0, 0.0), (0.0, 4.0), (0.3, 2.0), (0.86, 0.0)] {
            let mut p = Pose::rest(0, 3);
            if k == 0.0 { lunge(&mut p, t) } else { stuck(&mut p, k, t) }
            v.push(p);
        }
        for k in [0.2f32, 0.6, 1.0] {
            let mut p = Pose::rest(0, 3);
            honk_wind(&mut p, k, 1.0);
            v.push(p);
        }
        let mut p = Pose::rest(0, 3);
        honked(&mut p, 2.0, 0.0);
        v.push(p);
        for k in 0..8 {
            let mut p = Pose::rest(0, 3);
            p.turn = k as f32 * TAU / 8.0;
            pirouette(&mut p, 40, 46, 0.0);
            p.turn = k as f32 * TAU / 8.0;
            v.push(p);
        }
        v
    };
    let (cw, ch) = (W, H);
    let cols = 4;
    let rows = poses.len().div_ceil(cols) as i32;
    let mut out = Img::black(cw * cols as i32, ch * rows);
    for (i, p) in poses.iter().enumerate() {
        let im = draw(p);
        let (gx, gy) = ((i % cols) as i32 * cw, (i / cols) as i32 * ch);
        out.stamp(&im, gx + cw / 2 - im.w / 2, gy + ch - im.h);
    }
    out
}
