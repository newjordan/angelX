//! Motion: one clock for everything that moves on the screen.
//!
//! The Delve steps at 30 ticks a second (`rules::HZ`) and every picture is
//! a pure function of the run: an effect's age in ticks goes in, a pose
//! comes out, and nothing is kept between frames. This is the shared
//! vocabulary for that (docs/MOTION.md):
//!
//! - [`ease`] and [`Ease`]: curves over `0..=1` — even, in, out, both, an
//!   overshoot, an anticipation's pull-back, a spring, a bounce, a light
//!   elastic, and the bump that rises and falls.
//! - [`Beats`]: an effect's beat sheet as data — anticipation, action,
//!   follow-through, settle — and where in it an age falls ([`Beat`]).
//! - [`Key`] and [`track`]: a value keyed through time, each key reached
//!   on its own ease; a key that repeats the last value is a hold.
//! - [`Spring`]: a damped spring stepped at the game's own rate, for a
//!   value released, or kicked from rest (secondary motion).
//! - [`squash`], [`arc`], [`steps`]: squash and stretch that keep a
//!   sprite's area, a thrown arc, and the held steps pixel art moves in.
//! - [`shake`], [`impact`], [`flash`]: the camera's shake from its
//!   countdown, and a hit's impact and flash over its first ticks.
//!
//! It is the same on every machine: only `+ - * /`, `sqrt`, `floor` and
//! `round` (exact under IEEE 754 on every platform, wasm included) and its
//! own sine, cosine and exponent ([`math`]) in place of the platform's, so
//! a host, a friend's mirror and a browser guest compute every value bit
//! for bit alike. Nothing here reads the wire, a clock or a die.

/// Ticks a second: the rules' own rate (`rules::HZ`, checked by a test).
pub const HZ: u32 = 30;

fn c(t: f32) -> f32 {
    if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) }
}

/// How far `age` is through a span `len` ticks long that starts at
/// `start`, clamped to `0..=1`.
pub fn share(age: f32, start: f32, len: f32) -> f32 {
    if len <= 0.0 {
        return if age >= start { 1.0 } else { 0.0 };
    }
    c((age - start) / len)
}

/// `a` to `b` by `t` (not clamped: an overshooting ease goes past `b`).
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `a` to `b` as `t` goes 0 to 1 along `ease`.
pub fn tween(a: f32, b: f32, t: f32, ease: Ease) -> f32 {
    lerp(a, b, ease.at(t))
}

// ─── math ────────────────────────────────────────────────────────────────────

/// Sine, cosine and the exponent from `+ - * /` alone: the same bits on
/// every platform, where the system's own may differ in the last place.
/// Good to about 1e-12 in `f64` before the result is rounded to `f32`.
pub mod math {
    use std::f64::consts::{FRAC_PI_2, LN_2, PI, TAU};

    /// `sin x`.
    pub fn sin(x: f32) -> f32 {
        sin64(f64::from(x)) as f32
    }

    /// `cos x`.
    pub fn cos(x: f32) -> f32 {
        sin64(f64::from(x) + FRAC_PI_2) as f32
    }

    /// `e^x`, for `x` in about `-87..88`; outside it, its nearest end.
    pub fn exp(x: f32) -> f32 {
        exp64(f64::from(x)) as f32
    }

    /// `sin x` in `f64`: reduced exactly to `-pi/2..=pi/2`, then a Taylor
    /// polynomial to the 15th power (its error under 3e-12 there).
    pub fn sin64(x: f64) -> f64 {
        if !x.is_finite() {
            return 0.0;
        }
        let mut r = x - (x / TAU).round() * TAU;
        if r > FRAC_PI_2 {
            r = PI - r;
        } else if r < -FRAC_PI_2 {
            r = -PI - r;
        }
        let r2 = r * r;
        r * (1.0
            + r2 * (-1.0 / 6.0
                + r2 * (1.0 / 120.0
                    + r2 * (-1.0 / 5040.0
                        + r2 * (1.0 / 362_880.0
                            + r2 * (-1.0 / 39_916_800.0
                                + r2 * (1.0 / 6_227_020_800.0 + r2 * (-1.0 / 1_307_674_368_000.0))))))))
    }

    /// `cos x` in `f64`.
    pub fn cos64(x: f64) -> f64 {
        sin64(x + FRAC_PI_2)
    }

    /// `e^x` in `f64`: `x = k ln 2 + r`, `e^r` by its series, times `2^k`
    /// laid straight into the exponent's bits.
    pub fn exp64(x: f64) -> f64 {
        if x.is_nan() {
            return 1.0;
        }
        let x = x.clamp(-700.0, 700.0);
        let k = (x / LN_2).round();
        let r = x - k * LN_2;
        let (mut term, mut sum) = (1.0f64, 1.0f64);
        for n in 1..=14 {
            term *= r / f64::from(n);
            sum += term;
        }
        sum * f64::from_bits(((k as i64 + 1023) as u64) << 52)
    }
}

// ─── eases ───────────────────────────────────────────────────────────────────

/// Easing curves over `0..=1`, as plain functions. Each takes any number
/// and clamps it; each starts at 0 and ends at 1 (`bump` ends where it
/// began, at 0).
pub mod ease {
    use super::{c, math};

    /// Even all the way.
    pub fn linear(t: f32) -> f32 {
        c(t)
    }

    /// Slow away, fast in (cubic): what is drawn in toward a point.
    pub fn inward(t: f32) -> f32 {
        let t = c(t);
        t * t * t
    }

    /// Fast away, gentle to rest (cubic): the shape of most releases.
    pub fn out(t: f32) -> f32 {
        let u = 1.0 - c(t);
        1.0 - u * u * u
    }

    /// Slow at both ends (smoothstep).
    pub fn both(t: f32) -> f32 {
        let t = c(t);
        t * t * (3.0 - 2.0 * t)
    }

    /// A gentler in (quadratic).
    pub fn in_quad(t: f32) -> f32 {
        let t = c(t);
        t * t
    }

    /// A gentler out (quadratic).
    pub fn out_quad(t: f32) -> f32 {
        let u = 1.0 - c(t);
        1.0 - u * u
    }

    /// Past the mark and back: an opening that overshoots (by about an
    /// eighth) and settles.
    pub fn back(t: f32) -> f32 {
        back_by(t, 1.9)
    }

    /// `back` with its own overshoot `s` (1.70158 goes a tenth past).
    pub fn back_by(t: f32, s: f32) -> f32 {
        let u = c(t) - 1.0;
        1.0 + u * u * ((s + 1.0) * u + s)
    }

    /// The anticipation: a pull the other way first (to about -0.1), then
    /// away fast to the mark. A wind-up before a throw.
    pub fn anticipate(t: f32) -> f32 {
        let t = c(t);
        let s = 1.70158;
        t * t * ((s + 1.0) * t - s)
    }

    /// A snap that rings once and stills.
    pub fn spring(t: f32) -> f32 {
        let t = c(t);
        if t >= 1.0 {
            return 1.0;
        }
        1.0 - math::exp(-6.0 * t) * math::cos(t * 11.0)
    }

    /// A light elastic: out past the mark by about a quarter, back under
    /// it a little, a last small swing, and exactly at rest on 1.
    pub fn elastic(t: f32) -> f32 {
        let t = c(t);
        1.0 - (1.0 - t) * math::exp(-4.0 * t) * math::cos(t * 2.0 * std::f32::consts::TAU)
    }

    /// A fall that bounces to rest: three hops, each smaller.
    pub fn bounce(t: f32) -> f32 {
        let t = c(t);
        let (n, d) = (7.5625, 2.75);
        if t < 1.0 / d {
            n * t * t
        } else if t < 2.0 / d {
            let t = t - 1.5 / d;
            n * t * t + 0.75
        } else if t < 2.5 / d {
            let t = t - 2.25 / d;
            n * t * t + 0.9375
        } else {
            let t = t - 2.625 / d;
            n * t * t + 0.984375
        }
    }

    /// Up and down again: zero at both ends, one in the middle.
    pub fn bump(t: f32) -> f32 {
        math::sin(c(t) * std::f32::consts::PI)
    }
}

/// An ease by name, for an effect described as data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Ease {
    Linear,
    /// Cubic in.
    In,
    /// Cubic out: most releases.
    #[default]
    Out,
    /// Smoothstep.
    Both,
    InQuad,
    OutQuad,
    /// Overshoots and settles.
    Back,
    /// Pulls back first, then goes.
    Anticipate,
    Spring,
    Elastic,
    Bounce,
    /// Steps straight to the end: a cut, or a hold followed by a jump.
    Step,
}

impl Ease {
    pub const ALL: [Ease; 12] = [
        Ease::Linear,
        Ease::In,
        Ease::Out,
        Ease::Both,
        Ease::InQuad,
        Ease::OutQuad,
        Ease::Back,
        Ease::Anticipate,
        Ease::Spring,
        Ease::Elastic,
        Ease::Bounce,
        Ease::Step,
    ];

    pub fn at(self, t: f32) -> f32 {
        match self {
            Ease::Linear => ease::linear(t),
            Ease::In => ease::inward(t),
            Ease::Out => ease::out(t),
            Ease::Both => ease::both(t),
            Ease::InQuad => ease::in_quad(t),
            Ease::OutQuad => ease::out_quad(t),
            Ease::Back => ease::back(t),
            Ease::Anticipate => ease::anticipate(t),
            Ease::Spring => ease::spring(t),
            Ease::Elastic => ease::elastic(t),
            Ease::Bounce => ease::bounce(t),
            Ease::Step => {
                if c(t) >= 1.0 { 1.0 } else { 0.0 }
            }
        }
    }

    /// Whether it goes past its mark (or behind its start).
    pub fn overshoots(self) -> bool {
        matches!(self, Ease::Back | Ease::Anticipate | Ease::Spring | Ease::Elastic)
    }
}

// ─── beats ───────────────────────────────────────────────────────────────────

/// An effect's beat sheet, in ticks: the anticipation that gathers, the
/// action that throws, the follow-through that carries past the mark and
/// back, and the settle as it lingers and goes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Beats {
    pub anticipation: f32,
    pub action: f32,
    pub follow: f32,
    pub settle: f32,
}

/// Where in its beats an effect is, and how far (`0..1`) through that beat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Beat {
    Anticipation(f32),
    Action(f32),
    Follow(f32),
    Settle(f32),
    Done,
}

impl Beats {
    pub const fn new(anticipation: f32, action: f32, follow: f32, settle: f32) -> Beats {
        Beats { anticipation, action, follow, settle }
    }

    pub fn total(&self) -> f32 {
        self.anticipation + self.action + self.follow + self.settle
    }

    pub fn at(&self, age: f32) -> Beat {
        let mut t = age;
        if t < 0.0 || t.is_nan() {
            return Beat::Done;
        }
        for (len, beat) in [
            (self.anticipation, Beat::Anticipation as fn(f32) -> Beat),
            (self.action, Beat::Action),
            (self.follow, Beat::Follow),
            (self.settle, Beat::Settle),
        ] {
            if t < len {
                return beat(t / len.max(1e-3));
            }
            t -= len;
        }
        Beat::Done
    }

    /// The tick the action begins.
    pub fn action_at(&self) -> f32 {
        self.anticipation
    }

    /// The tick the follow-through begins.
    pub fn follow_at(&self) -> f32 {
        self.anticipation + self.action
    }

    /// The tick the settle begins.
    pub fn settle_at(&self) -> f32 {
        self.anticipation + self.action + self.follow
    }

    /// Ticks since the action began (negative in the anticipation).
    pub fn since_action(&self, age: f32) -> f32 {
        age - self.anticipation
    }

    /// How big the effect stands: drawn in a little through the
    /// anticipation, past full at the end of the action, back to full
    /// through the follow-through, full through the settle.
    pub fn size(&self, age: f32) -> f32 {
        match self.at(age) {
            Beat::Anticipation(t) => 0.25 * ease::inward(1.0 - t),
            Beat::Action(t) => 1.22 * ease::out(t),
            Beat::Follow(t) => 1.22 - 0.22 * ease::both(t),
            Beat::Settle(_) => 1.0,
            Beat::Done => 0.0,
        }
    }

    /// How bright: rising through the anticipation, full in the action,
    /// holding through the follow-through, and going through the settle.
    pub fn power(&self, age: f32) -> f32 {
        match self.at(age) {
            Beat::Anticipation(t) => 0.35 + 0.4 * t,
            Beat::Action(_) => 1.0,
            Beat::Follow(t) => 1.0 - 0.15 * t,
            Beat::Settle(t) => 0.85 * (1.0 - ease::both(t)),
            Beat::Done => 0.0,
        }
    }

    /// A pose through the beats: from rest pulled back to `-pull` in the
    /// anticipation, out past 1 by the action's end (`back`), back to 1
    /// through the follow-through, and held at 1 to the end. 0 when done.
    pub fn pose(&self, age: f32, pull: f32) -> f32 {
        match self.at(age) {
            Beat::Anticipation(t) => -pull * ease::out(t),
            Beat::Action(t) => lerp(-pull, 1.15, ease::out(t)),
            Beat::Follow(t) => 1.15 - 0.15 * ease::both(t),
            Beat::Settle(_) => 1.0,
            Beat::Done => 0.0,
        }
    }
}

// ─── tracks ──────────────────────────────────────────────────────────────────

/// A value at tick `at`, reached from the key before it along `ease`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub at: f32,
    pub v: f32,
    pub ease: Ease,
}

/// A key: `v` at tick `at`, arrived at along `ease`.
pub const fn key(at: f32, v: f32, ease: Ease) -> Key {
    Key { at, v, ease }
}

/// The value of `keys` (in order of `at`) at `age`: the first key's value
/// before it, the last's after it, and between two keys the later one's
/// ease. Two keys with the same value hold it still between them.
pub fn track(keys: &[Key], age: f32) -> f32 {
    let Some(first) = keys.first() else {
        return 0.0;
    };
    if age <= first.at {
        return first.v;
    }
    for w in keys.windows(2) {
        let (a, b) = (w[0], w[1]);
        if age < b.at {
            return tween(a.v, b.v, share(age, a.at, b.at - a.at), b.ease);
        }
    }
    keys[keys.len() - 1].v
}

// ─── springs ─────────────────────────────────────────────────────────────────

/// A damped spring, stepped at the game's rate (four steps a tick,
/// semi-implicit Euler) from a known start, so the pose at any age is a
/// pure function of the age and the same on every seat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Swings a second, undamped.
    pub hz: f32,
    /// 1 comes to rest without passing the mark; under 1 rings.
    pub damping: f32,
}

impl Spring {
    /// Quick and barely past: a pickup landing, a panel popping in.
    pub const SNAPPY: Spring = Spring { hz: 3.2, damping: 0.62 };
    /// Lazy and soft: cloth, a hanging sign.
    pub const SOFT: Spring = Spring { hz: 1.4, damping: 0.5 };
    /// Rings a few times: a jelly, a struck bell, a wobbling hat.
    pub const WOBBLY: Spring = Spring { hz: 2.6, damping: 0.22 };

    /// Steps past this are taken to be at rest (ten seconds).
    const MOST: u32 = 10 * HZ;

    /// Where a value let go at `from`, moving `v0` a tick, stands `age`
    /// ticks later as the spring pulls it to `to`.
    pub fn at(&self, age: f32, from: f32, to: f32, v0: f32) -> f32 {
        if age <= 0.0 || age.is_nan() {
            return from;
        }
        const SUB: u32 = 4;
        let dt = 1.0 / (HZ * SUB) as f32;
        let w = std::f32::consts::TAU * self.hz;
        let (k, d) = (w * w, 2.0 * self.damping * w);
        let (mut x, mut v) = (from - to, v0 * HZ as f32);
        let tick = |x: &mut f32, v: &mut f32| {
            for _ in 0..SUB {
                *v += (-k * *x - d * *v) * dt;
                *x += *v * dt;
            }
        };
        let whole = (age.floor() as u32).min(Self::MOST);
        for _ in 0..whole {
            tick(&mut x, &mut v);
        }
        let f = age.fract();
        if whole >= Self::MOST || f == 0.0 {
            return to + x;
        }
        let was = x;
        tick(&mut x, &mut v);
        to + lerp(was, x, f)
    }

    /// A body at rest kicked with `kick` a tick at age 0: how far off its
    /// rest it is `age` ticks later (secondary motion: a jiggle, a sway).
    pub fn kick(&self, age: f32, kick: f32) -> f32 {
        self.at(age, 0.0, 0.0, kick)
    }

    /// `at` from 0 to 1 at rest: the spring as an ease over ticks.
    pub fn ease(&self, age: f32) -> f32 {
        self.at(age, 0.0, 1.0, 0.0)
    }
}

// ─── shape and step ──────────────────────────────────────────────────────────

/// Squash and stretch that keep a body's area: `k` above 0 squashes it
/// wide and low, below 0 stretches it tall and thin. `(width, height)`
/// scales.
pub fn squash(k: f32) -> (f32, f32) {
    let k = k.clamp(-0.6, 0.6);
    let wide = 1.0 + k;
    (wide, 1.0 / wide)
}

/// A thrown arc from `a` to `b`, `height` pixels over the straight line at
/// its top: where it is at `t` (0 thrown, 1 landed). Up is minus.
pub fn arc(a: (f32, f32), b: (f32, f32), height: f32, t: f32) -> (f32, f32) {
    let t = c(t);
    (lerp(a.0, b.0, t), lerp(a.1, b.1, t) - height * 4.0 * t * (1.0 - t))
}

/// `t` held to `n` even steps: the way pixel art moves, a pose at a time
/// rather than a smear of in-betweens. `steps(t, 1)` is a cut.
pub fn steps(t: f32, n: u32) -> f32 {
    let n = n.max(1) as f32;
    (c(t) * n).floor() / n
}

/// An age held on twos (or threes...): each pose kept `every` ticks.
pub fn on(age: u64, every: u64) -> u64 {
    let every = every.max(1);
    age / every * every
}

// ─── shake and impact ────────────────────────────────────────────────────────

/// The camera's shake, in pixels, from its countdown: `left` ticks of a
/// `full` shake, at most `reach` pixels. Trauma squared, so small blows
/// barely move and the last ticks settle instead of buzzing; two waves an
/// axis that never line up, so it wanders rather than ticks side to side.
pub fn shake(tick: u64, left: u32, full: u32, reach: f32) -> (i32, i32) {
    let trauma = (left as f32 / full.max(1) as f32).min(1.0);
    let amp = reach * trauma * trauma;
    let t = tick as f64;
    let x = math::sin64(t * 0.47) * 0.7 + math::sin64(t * 0.19 + 1.3) * 0.3;
    let y = math::sin64(t * 0.37 + 2.1) * 0.7 + math::sin64(t * 0.23) * 0.3;
    ((x as f32 * amp).round() as i32, (y as f32 * amp).round() as i32)
}

/// A hit's weight over its first ticks: 1 at contact, held for `hold`
/// ticks (the frames a hit-stop keeps), then falling away over `fall`.
pub fn impact(age: f32, hold: f32, fall: f32) -> f32 {
    if age < 0.0 {
        return 0.0;
    }
    if age < hold {
        return 1.0;
    }
    1.0 - ease::out(share(age, hold, fall))
}

/// A hit flash, a tick at a time: lit for the first `hold` ticks, then
/// every other tick for `blink` more, then dark.
pub fn flash(age: u32, hold: u32, blink: u32) -> bool {
    age < hold || (age < hold + blink && (age - hold) % 2 == 1)
}

#[cfg(test)]
#[path = "../../tests/render/motion__tests.rs"]
mod tests;
