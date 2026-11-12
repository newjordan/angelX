//! Rafterwake's effects on the tower: dust where boots land and leave, gold where a rung is caught
//! and a deck is won, sparks and chips where the keeper is struck, rings where its slam comes
//! down, the rim of red when a knight falls. Each is a pure function of a mark's age on the run's
//! tick (`rules::rafterwake::show`), so a friend's mirror and a browser guest draw the same pixels.
use crate::render::overworld::ink::{Img, bayer, hash};
use super::super::juice;
use super::super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::hud::{PAD, View};
use crate::motion::ease;
use crate::rules::juice::Weight;
use crate::rules::rafterwake::{
    Q, TOP,
    show::{Kind, Mark},
};

/// Wood chips: pale timber down to dark.
const CHIPS: Ramp = Ramp(['T', 'O', 'o', 'R', 'p', 'B']);

/// What an effect needs to place itself in the window.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub v: &'a View<'a>,
    pub cam: i32,
}

impl Ctx<'_> {
    /// A tower point as a window point.
    pub fn at(&self, x: i32, y: i32) -> (f32, f32) {
        ((x + PAD) as f32, (y - self.cam) as f32)
    }
    fn tick(&self) -> u64 {
        u64::from(self.v.now)
    }
}

fn seed(m: &Mark, extra: u32) -> u32 {
    m.born.wrapping_mul(0x9E37_79B1).wrapping_add(extra.wrapping_mul(0x85EB_CA77)) ^ 0x2c1b_3c6d
}

/// The marks of the viewer's own tower.
fn marks<'a>(c: &'a Ctx) -> impl Iterator<Item = (&'a Mark, u32)> {
    let now = c.v.now;
    c.v.d.show.marks.iter().map(move |m| (m, m.age(now)))
}

/// Dust thrown sideways from `(x, y)`: `n` motes each way.
fn puffs(pen: &mut Pen, at: (f32, f32), n: u32, speed: (f32, f32), life: (f32, f32), age: f32, sd: u32, tick: u64) {
    for (dir, side) in [(0.0, 0u32), (std::f32::consts::PI, 1)] {
        let spray = Spray {
            angle: dir - 0.12 * (1.0 - dir.signum().abs()),
            spread: 0.5,
            drag: 0.16,
            gravity: -0.012,
            squash: 0.4,
            ..Spray::round(sd ^ side.wrapping_mul(0x51), n, at, speed, life)
        };
        spray.each(age, |m| pfx::mote(pen, &m, Shape::Smoke, ramps::SMOKE, 2, tick));
    }
}

// ─── under the figures ───────────────────────────────────────────────────────

/// What lies behind the knight: dust at its feet, the shock of a landing and of a slam, a deck
/// won, debris of a brick.
pub(super) fn under(im: &mut Img, c: Ctx) {
    for (m, age) in marks(&c) {
        let (x, y) = c.at(m.x, m.y);
        let a = age as f32;
        match m.kind {
            Kind::Land { hard } if age < 22 => {
                let mut pen = Pen::new(im, 500);
                let reach = if hard { 2.0 } else { 1.3 };
                pen.shock((x, y + 1.0), 3.0 + a * reach, 1.3, 0.28, ramps::SMOKE, a / 16.0);
                puffs(&mut pen, (x, y - 1.0), if hard { 6 } else { 4 }, (0.5, if hard { 2.4 } else { 1.7 }), (10.0, 20.0), a, seed(m, 1), c.tick());
            }
            Kind::Hop if age < 14 => {
                let mut pen = Pen::new(im, 200);
                puffs(&mut pen, (x, y - 1.0), 2, (0.3, 1.1), (6.0, 12.0), a, seed(m, 2), c.tick());
            }
            Kind::Top { .. } if age < 18 => {
                let mut pen = Pen::new(im, 400);
                pen.shock((x, y), 4.0 + a * 3.4, 1.6, 0.26, ramps::GOLD, a / 18.0);
                let glints = Spray { gravity: -0.045, drag: 0.1, squash: 0.5, ..Spray::round(seed(m, 3), 9, (x, y - 2.0), (0.4, 1.5), (14.0, 24.0)) };
                glints.each(a, |g| pfx::mote(&mut pen, &g, Shape::Star, ramps::GOLD, 1, c.tick()));
            }
            Kind::Slam { sweep } if age < 26 => {
                let mut pen = Pen::new(im, 800);
                let r = if sweep { 5.0 + a * 8.0 } else { 4.0 + a * 3.2 };
                pen.shock((x, y), r, 2.0, if sweep { 0.1 } else { 0.3 }, ramps::SMOKE, a / 26.0);
                // Grit shaken from the beams over it.
                let grit = Spray { angle: std::f32::consts::FRAC_PI_2, spread: 2.6, gravity: 0.09, drag: 0.05, ..Spray::round(seed(m, 4), 14, (x, y - 70.0), (0.2, 0.9), (16.0, 30.0)) };
                grit.each(a, |g| pfx::mote(&mut pen, &g, Shape::Dot, ramps::SMOKE, 1, c.tick()));
            }
            Kind::Brick if age < 28 => {
                let mut pen = Pen::new(im, 700);
                pen.shock((x, y), 3.0 + a * 1.6, 1.4, 0.3, ramps::SMOKE, a / 22.0);
                let chips = Spray { angle: -std::f32::consts::FRAC_PI_2, spread: 2.4, gravity: 0.2, drag: 0.04, ..Spray::round(seed(m, 5), 14, (x, y - 3.0), (1.2, 3.4), (14.0, 26.0)) };
                chips.each(a, |g| pfx::mote(&mut pen, &g, Shape::Shard, CHIPS, 2, c.tick()));
                puffs(&mut pen, (x, y - 1.0), 5, (0.5, 2.0), (10.0, 20.0), a, seed(m, 6), c.tick());
            }
            _ => {}
        }
    }
}

// ─── over the figures ────────────────────────────────────────────────────────

/// Sparks and rings over the knight and the keeper: a rung caught, the keeper struck, a fall, a
/// chain, a deck won, the bell's glints at the boots.
pub(super) fn over(im: &mut Img, c: Ctx) {
    for (m, age) in marks(&c) {
        let (x, y) = c.at(m.x, m.y);
        let a = age as f32;
        match m.kind {
            Kind::Catch if age < 16 => {
                let mut pen = Pen::new(im, 300);
                pen.twinkle((x, y - 22.0), 3.0, a / 14.0, ramps::GOLD);
                pen.twinkle((x + 6.0, y - 14.0), 2.0, ((a - 3.0) / 12.0).max(-1.0), ramps::GOLD);
                let sparks = Spray { gravity: 0.1, ..Spray::round(seed(m, 7), 6, (x, y - 20.0), (0.8, 2.0), (8.0, 14.0)) };
                sparks.each(a, |g| pfx::mote(&mut pen, &g, Shape::Spark, ramps::GOLD, 1, c.tick()));
            }
            Kind::Strike { left } => {
                let weight = m.kind.weight();
                juice::impact_burst(im, (x, y), age, seed(m, 8), weight, c.tick());
                let mut pen = Pen::new(im, 900);
                if age < 24 {
                    let chips = Spray { gravity: 0.2, drag: 0.05, ..Spray::round(seed(m, 9), if left == 0 { 22 } else { 12 }, (x, y), (1.0, 3.8), (14.0, 28.0)) };
                    chips.each(a, |g| pfx::mote(&mut pen, &g, Shape::Shard, CHIPS, 2, c.tick()));
                }
                if left == 0 && age < 30 {
                    pen.shock((x, y + 4.0), 6.0 + a * 4.2, 2.2, 0.5, ramps::GOLD, a / 30.0);
                    if age < 10 {
                        pfx::rays(&mut pen, (x, y), 12, 38.0, 1.0, 0.3, ramps::GOLD, a / 10.0);
                    }
                }
            }
            Kind::Fall { .. } if age < 30 => {
                let mut pen = Pen::new(im, 700);
                let (cx, cy) = (x, y - 20.0);
                if age < 8 {
                    pfx::rays(&mut pen, (cx, cy), 8, 22.0, 1.0, 0.2, ramps::EMBER, a / 8.0);
                }
                let dust = Spray { gravity: 0.1, drag: 0.1, ..Spray::round(seed(m, 10), 12, (cx, cy), (0.8, 2.8), (12.0, 24.0)) };
                dust.each(a, |g| pfx::mote(&mut pen, &g, Shape::Smoke, ramps::SMOKE, 2, c.tick()));
                // Stars wheel over the crumpled knight.
                if (6..30).contains(&age) {
                    for i in 0..3 {
                        let t = a * 0.34 + i as f32 * 2.1;
                        pen.twinkle((cx + t.cos() * 10.0, cy - 14.0 + t.sin() * 3.0), 2.0, ((a - 6.0) / 24.0 + i as f32 * 0.0).clamp(0.0, 0.95), ramps::GOLD);
                    }
                }
            }
            Kind::Rise if age < 18 => {
                let mut pen = Pen::new(im, 300);
                pen.shock((x, y - 1.0), 3.0 + a * 1.8, 1.4, 0.3, ramps::GOLD, a / 18.0);
                let motes = Spray { gravity: -0.05, drag: 0.1, ..Spray::round(seed(m, 11), 8, (x, y - 14.0), (0.3, 1.2), (12.0, 20.0)) };
                motes.each(a, |g| pfx::mote(&mut pen, &g, Shape::Star, ramps::GOLD, 1, c.tick()));
            }
            Kind::Chain { .. } if age < 18 => {
                let mut pen = Pen::new(im, 300);
                let ring = Spray { gravity: 0.03, drag: 0.12, ..Spray::round(seed(m, 12), 10, (x, y - 24.0), (1.2, 2.8), (10.0, 18.0)) };
                ring.each(a, |g| pfx::mote(&mut pen, &g, Shape::Star, ramps::GOLD, 1, c.tick()));
            }
            _ => {}
        }
    }
    if c.v.s.bell > 0 {
        let d = c.v.d;
        let (x, y) = c.at(d.x / Q, d.y / Q);
        let mut pen = Pen::new(im, 200);
        let phase = (c.v.now / 3) % 4;
        for k in 0..2u32 {
            let t = ((c.v.now / 2 + k * 3) % 8) as f32 / 8.0;
            let side = if (k + phase) % 2 == 0 { -5.0 } else { 5.0 };
            pen.twinkle((x + side, y - 2.0 + t * 3.0), 2.0, t, ramps::GOLD);
        }
    }
}

// ─── the whole picture ──────────────────────────────────────────────────────

/// The shake of the window now: the heaviest of the tower's marks (`motion::shake` in the kit),
/// and a little more through a hit-stop so the held frame is not dead.
pub(super) fn shake(c: Ctx) -> (i32, i32) {
    let blows = marks(&c).filter_map(|(m, age)| {
        let w = m.kind.weight();
        (w >= Weight::Solid && age < w.shake_ticks()).then_some((w, age))
    });
    let held = u64::from(c.v.run.hitstop.left) * 7;
    juice::shake(blows, c.tick() + held)
}

/// How a mark should squash the knight's body: `(age, amount)` of the freshest that does.
pub(super) fn squash(c: Ctx) -> Option<(u32, f32)> {
    marks(&c)
        .filter_map(|(m, age)| match m.kind {
            Kind::Land { hard } if age < 14 => Some((age, if hard { 0.34 } else { 0.2 })),
            // A hop stretches first: the squash curve a few ticks along.
            Kind::Hop if age < 10 => Some((age + 3, 0.18)),
            Kind::Rise if age < 10 => Some((age + 3, 0.22)),
            _ => None,
        })
        .min_by_key(|&(age, _)| age)
}

/// Is the knight in its white impact frame?
pub(super) fn knight_flash(c: Ctx) -> bool {
    marks(&c).any(|(m, age)| match m.kind {
        Kind::Fall { .. } => age < 3,
        _ => false,
    })
}

/// Is the keeper in its white impact frame, and how far through the squash after a strike?
pub(super) fn keeper_strike(c: Ctx) -> Option<(u32, Weight)> {
    marks(&c)
        .filter_map(|(m, age)| match m.kind {
            Kind::Strike { .. } if age < 16 => Some((age, m.kind.weight())),
            _ => None,
        })
        .min_by_key(|&(age, _)| age)
}

/// The rim of danger and hurt over the window: red after a fall, pulsing while a tell closes on
/// the knight, warm gold through the bell.
pub(super) fn rim(over: &mut Img, c: Ctx, tell_on_knight: bool) {
    let (w, h) = (over.w, over.h);
    let mut k: f32 = 0.0;
    let mut ink = 'p';
    for (m, age) in marks(&c) {
        if matches!(m.kind, Kind::Fall { .. }) && age < 24 {
            k = k.max(0.6 * (1.0 - age as f32 / 24.0));
            ink = '7';
        }
    }
    if tell_on_knight && k < 0.3 {
        k = 0.22 + 0.12 * ((c.v.now / 3 % 2) as f32);
        ink = 'p';
    }
    if k > 0.0 {
        juice::vignette(over, (0, 0, w, h), k, 10, ink);
    }
}

/// A flash of gold dither over the window as the keeper falls.
pub(super) fn flash(over: &mut Img, c: Ctx) {
    for (m, age) in marks(&c) {
        if matches!(m.kind, Kind::Strike { left: 0 }) && age < 5 {
            let k = 0.55 * (1.0 - age as f32 / 5.0);
            for y in 0..over.h {
                for x in 0..over.w {
                    if bayer(x, y) < k {
                        over.put(x, y, if age < 2 { 'c' } else { 'T' });
                    }
                }
            }
        }
    }
}

/// A few motes of dust drifting up through the lamplight, behind everything.
pub(super) fn motes(im: &mut Img, now: u32, cam: i32) {
    let mut pen = Pen::new(im, 120);
    for i in 0..14u32 {
        let h = hash(i as i32, 7, 0x5eed);
        let x = (h % 270) as f32 + 4.0;
        let rise = (now as f32 * (0.06 + (h >> 8 & 7) as f32 * 0.012)) + cam as f32 * 0.0;
        let y = ((h >> 12) % 174) as f32 - rise;
        let y = y.rem_euclid(174.0);
        let sway = ((now as f32 * 0.04 + i as f32).sin()) * 3.0;
        let k = (0.35 + 0.35 * ((now as f32 * 0.05 + i as f32 * 1.7).sin())).clamp(0.0, 1.0);
        if bayer(x as i32, y as i32) < 0.8 {
            pen.light((x + sway) as i32, y as i32, ramps::EMBER.at(0.5 + k * 0.3));
        }
    }
}

/// An ease for the window's iris at the end of a round.
pub(super) fn out(t: f32) -> f32 {
    ease::out(t.clamp(0.0, 1.0))
}

/// The keeper's deck's top, for effects that want to stand on it.
#[allow(dead_code)]
pub(super) const DECK: i32 = TOP;
