//! What ails a figure, drawn over it so it reads at a glance: a burning
//! monster has flames licking up it, embers going off and a smudge of
//! smoke; a chilled one a crust of ice at its feet, glints coming and going
//! on it and flakes falling; a poisoned one green bubbles rising off it and
//! popping, drips and a sick glow; a stunned one stars going round its
//! head; a sleeper drifting Zs; one the Butcher holds wounds that bleed and
//! a pool under it. A hexed knight has the hex's sigil turning over the
//! frog; a mired one stands sunk in bog with the mud bubbling round.
//!
//! Each mark is a pure function of the figure's feet, its height, the tick
//! and a seed of the figure's own, paints a bounded handful of motes, and
//! stays inside a box a little bigger than the figure.

use super::super::ink::{Img, bayer, hash};
use super::pfx::{self, Pen, Ramp, Shape, ease, ramps, unit};
use super::{UNIT, wishfx};
use crate::rules::{Run, skills};
use std::f32::consts::TAU;

/// The most motes the marks paint in a pass, every figure together.
const BUDGET: u32 = 1600;

/// A hex: the frog's own green into the genie's teal.
const HEX: Ramp = Ramp(['Y', 'C', '3', '2', '1', '0']);
/// Bog: the dark of the mud, its wet sheen.
const MUD: Ramp = Ramp(['M', 'A', 'P', 'I', 'b', 'n']);
/// A dream: the night's teal to its dark.
const DREAM: Ramp = Ramp(['w', '3', '2', '1', 'q', '0']);

/// What ails a figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ail {
    Burn,
    Chill,
    Poison,
    Stun,
    Sleep,
    Bleed,
    Hex,
    Mire,
}

impl Ail {
    pub(super) const ALL: [Ail; 8] = [Ail::Burn, Ail::Chill, Ail::Poison, Ail::Stun, Ail::Sleep, Ail::Bleed, Ail::Hex, Ail::Mire];
}

/// A figure as the marks see it: its feet, how tall and wide it stands, a
/// seed of its own.
#[derive(Clone, Copy, Debug)]
pub(super) struct Body {
    pub feet: (f32, f32),
    pub h: f32,
    pub w: f32,
    pub seed: u32,
}

impl Body {
    fn mid(&self) -> (f32, f32) {
        (self.feet.0, self.feet.1 - self.h * 0.5)
    }
    fn head(&self) -> (f32, f32) {
        (self.feet.0, self.feet.1 - self.h)
    }
}

/// Over the figures: each monster's ailments and each knight's.
pub(super) fn over(cv: &mut Img, run: &Run) {
    let mut pen = Pen::new(cv, BUDGET);
    let tick = run.tick;
    for e in run.enemies.iter().filter(|e| e.hp > 0) {
        let (x, y, h) = super::works::foe_box(run, e);
        let body = Body { feet: (x as f32, y as f32), h: h as f32, w: (e.radius() * 2.0 * UNIT).clamp(8.0, 40.0), seed: e.id.wrapping_mul(0x9e37) };
        let mut ails = [None; 6];
        let mut n = 0;
        let mut add = |a: Ail| {
            if n < ails.len() && !ails.contains(&Some(a)) {
                ails[n] = Some(a);
                n += 1;
            }
        };
        for bane in run.works.banes.iter().filter(|b| b.foe == e.id) {
            add(match bane.kind {
                skills::Bane::Burn => Ail::Burn,
                skills::Bane::Poison => Ail::Poison,
            });
        }
        if e.chilled > 0 {
            add(Ail::Chill);
        }
        if e.asleep(tick) {
            add(Ail::Sleep);
        } else if e.frozen > 0 && !held_in_time(run, e) {
            add(Ail::Stun);
        }
        if run.champions.grips.iter().any(|g| g.foe == e.id) {
            add(Ail::Bleed);
        }
        for a in ails.into_iter().flatten() {
            mark(&mut pen, a, &body, tick);
        }
    }
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.privy == 0 && !h.absent) {
        let (feet, _) = wishfx::body(run, hero);
        let body = Body { feet, h: if hero.frog() { 10.0 } else { 20.0 }, w: 14.0, seed: id.wrapping_mul(0x85eb) };
        if hero.frog() {
            mark(&mut pen, Ail::Hex, &body, tick);
        }
        if hero.chilled > 0 {
            mark(&mut pen, Ail::Chill, &body, tick);
        }
        if hero.mired > 0 {
            mark(&mut pen, Ail::Mire, &body, tick);
        }
    }
}

/// A monster held in a Stillhour: stood still in time, not stunned.
pub(super) fn held_in_time(run: &Run, e: &crate::rules::Enemy) -> bool {
    run.spheres.iter().any(|s| (e.x - s.x).hypot(e.y - s.y) < s.r)
}

/// Draw ailment `a` on `body` at `tick`.
pub(super) fn mark(pen: &mut Pen, a: Ail, body: &Body, tick: u64) {
    match a {
        Ail::Burn => burn(pen, body, tick),
        Ail::Chill => chill(pen, body, tick),
        Ail::Poison => poison(pen, body, tick),
        Ail::Stun => stun(pen, body, tick),
        Ail::Sleep => sleep(pen, body, tick),
        Ail::Bleed => bleed(pen, body, tick),
        Ail::Hex => hex(pen, body, tick),
        Ail::Mire => mire(pen, body, tick),
    }
}

/// Where `i` of a figure's loops is, `life` ticks round: how far through
/// it (0 to 1) and which time round.
fn looped(i: u32, seed: u32, life: f32, tick: u64) -> (f32, i32) {
    let start = unit(i as i32, seed as i32, 0x10f) * life;
    let t = tick as f32 + start;
    ((t % life) / life, (t / life) as i32)
}

/// Burning: three or four tongues of flame licking up the figure's sides
/// and middle, each its own height and flicker, white-gold at the root and
/// red at the tip; a glow at its foot, and embers going up off it.
fn burn(pen: &mut Pen, b: &Body, tick: u64) {
    let r = ramps::FIRE;
    let t = tick as f32;
    let flick = 0.8 + 0.2 * (t * 0.7 + b.seed as f32).sin();
    pen.glow((b.feet.0, b.feet.1 - b.h * 0.25), b.w * 0.55 + 4.0, 0.8, ramps::EMBER, 0.45 * flick);
    let n = if b.w > 18.0 { 4 } else { 3 };
    for i in 0..n {
        let f = (i as f32 + 0.5) / n as f32 - 0.5;
        let root = (b.feet.0 + f * b.w * 0.8, b.feet.1 - b.h * (0.12 + 0.28 * unit(i, b.seed as i32, 0xb1)));
        let tall = 4.0 + b.h * 0.22 + (hash(i, (tick / 2) as i32, b.seed) % 4) as f32;
        let steps = tall.ceil() as i32;
        for j in 0..steps {
            let u = j as f32 / tall;
            let half = (1.0 - u).powf(0.7) * 1.8;
            let sway = ((j as f32 * 0.7 - t * 0.55 + i as f32 * 2.0).sin()) * u * 1.6;
            let y = (root.1 - j as f32).round() as i32;
            let hw = half.round() as i32;
            for dx in -hw..=hw {
                let x = (root.0 + sway).round() as i32 + dx;
                let edge = dx.abs() as f32 / half.max(0.6);
                let k = (u * 0.75 + edge * 0.35).min(1.0);
                if u > 0.6 && bayer(x, y + (tick / 2) as i32) < (u - 0.6) * 2.2 {
                    continue;
                }
                pen.put(x, y, r.at(k));
            }
        }
    }
    for i in 0..5u32 {
        let (k, n) = looped(i, b.seed, 18.0, tick);
        let x = b.feet.0 + (unit(i as i32 + n * 5, b.seed as i32, 0xb2) - 0.5) * b.w * 0.9 + (k * 7.0 + i as f32).sin() * 1.5;
        let y = b.feet.1 - b.h * 0.4 - k * (b.h * 0.7 + 8.0);
        let m = pfx::Mote { i, at: (x, y), vel: (0.0, -1.0), k, seed: b.seed ^ i };
        pfx::mote(pen, &m, Shape::Ember, ramps::EMBER, 1, tick);
    }
}

/// Chilled: a crust of ice pushed up round the figure's feet, glints
/// coming and going on it, and flakes falling about it.
fn chill(pen: &mut Pen, b: &Body, tick: u64) {
    let r = ramps::FROST;
    let half = (b.w * 0.55).round() as i32;
    for dx in -half..=half {
        let x = b.feet.0.round() as i32 + dx;
        let up = 1 + (hash(dx, 3, b.seed) % 3) as i32 - i32::from(dx.abs() > half - 2);
        for j in 0..up.max(0) {
            pen.put(x, b.feet.1.round() as i32 - j, if j == up - 1 { 'W' } else if (dx + j) % 3 == 0 { 'V' } else { 'z' });
        }
        if hash(dx, 4, b.seed) % 5 == 0 {
            pen.put(x, b.feet.1.round() as i32 - up, 'w');
        }
    }
    for i in 0..3u32 {
        let (k, n) = looped(i, b.seed, 26.0, tick);
        if k > 0.45 {
            continue;
        }
        let at = (
            b.feet.0 + (unit(i as i32 + n * 3, b.seed as i32, 0xc1) - 0.5) * b.w * 0.8,
            b.feet.1 - b.h * unit(i as i32 + n * 3, b.seed as i32, 0xc2) * 0.9,
        );
        pen.twinkle(at, 2.0, k / 0.45, r);
    }
    for i in 0..3u32 {
        let (k, n) = looped(i + 7, b.seed, 34.0, tick);
        let x = b.feet.0 + (unit(i as i32 + n * 9, b.seed as i32, 0xc3) - 0.5) * (b.w + 10.0) + (k * 6.0 + i as f32).sin() * 2.0;
        let y = b.feet.1 - b.h - 4.0 + k * (b.h + 4.0);
        let m = pfx::Mote { i, at: (x, y), vel: (0.0, 0.5), k: k * 0.7, seed: b.seed ^ i ^ n as u32 };
        pfx::mote(pen, &m, Shape::Flake, r, 1, tick);
    }
}

/// Poisoned: green bubbles rising off the figure and popping at the top
/// of their rise, a drip or two falling off it, a sick glow at its feet.
fn poison(pen: &mut Pen, b: &Body, tick: u64) {
    let r = ramps::POISON;
    pen.glow((b.feet.0, b.feet.1), b.w * 0.7 + 6.0, 0.4, r, 0.75);
    pen.glow(b.mid(), b.w * 0.6 + 4.0, 1.0, r.cooler(1), 0.35);
    for i in 0..5u32 {
        let (k, n) = looped(i, b.seed, 22.0, tick);
        let x = b.feet.0 + (unit(i as i32 + n * 11, b.seed as i32, 0xd1) - 0.5) * b.w * 0.9 + (k * 8.0 + i as f32).sin() * 1.5;
        let y = b.feet.1 - b.h * 0.3 - k * (b.h * 0.8 + 6.0);
        if k > 0.86 {
            // It pops: four flecks out from where it was.
            let s = ((k - 0.86) / 0.14 * 3.0) as i32 + 1;
            for (dx, dy) in [(-s, 0), (s, 0), (0, -s), (0, s)] {
                pen.light(x.round() as i32 + dx, y.round() as i32 + dy, r.step(1));
            }
            continue;
        }
        let m = pfx::Mote { i, at: (x, y), vel: (0.0, -1.0), k: k * 0.4, seed: b.seed ^ i };
        pfx::mote(pen, &m, Shape::Bubble, r, if i % 3 == 0 { 1 } else { 2 }, tick);
    }
    for i in 0..2u32 {
        let (k, n) = looped(i + 5, b.seed, 20.0, tick);
        let x = b.feet.0 + (unit(i as i32 + n * 13, b.seed as i32, 0xd2) - 0.5) * b.w * 0.7;
        let y = b.feet.1 - b.h * 0.45 + ease::inward(k) * b.h * 0.45;
        let m = pfx::Mote { i, at: (x, y), vel: (0.0, 1.0), k: k * 0.5, seed: b.seed ^ i };
        pfx::mote(pen, &m, Shape::Droplet, r, 1, tick);
    }
}

/// A five-pixel star, its arms in `arm`.
fn star(pen: &mut Pen, x: i32, y: i32, core: char, arm: char, big: bool) {
    pen.put(x, y, core);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        pen.put(x + dx, y + dy, arm);
    }
    if big {
        for (dx, dy) in [(2, 0), (-2, 0), (0, -2)] {
            pen.light(x + dx, y + dy, arm);
        }
    }
}

/// Stunned: three stars going round over its head, the near ones big and
/// bright, the far ones small, on a faint ring of their way.
fn stun(pen: &mut Pen, b: &Body, tick: u64) {
    let c = (b.head().0, b.head().1 - 3.0);
    let (rx, ry) = (b.w * 0.35 + 4.0, 2.2);
    let t = tick as f32 * 0.22 + unit(b.seed as i32, 1, 0xe1) * TAU;
    for i in 0..16 {
        if i % 4 != 0 {
            continue;
        }
        let a = i as f32 / 16.0 * TAU + t * 0.5;
        pen.light((c.0 + a.cos() * rx).round() as i32, (c.1 + a.sin() * ry).round() as i32, '4');
    }
    for i in 0..3 {
        let a = t + i as f32 * TAU / 3.0;
        let (x, y) = ((c.0 + a.cos() * rx).round() as i32, (c.1 + a.sin() * ry).round() as i32);
        if a.sin() > 0.0 {
            star(pen, x, y, '9', '6', a.sin() > 0.7);
        } else {
            pen.put(x, y, '5');
            pen.light(x + 1, y, '4');
        }
    }
}

/// The Zs a sleeper breathes out, small and big.
const SMALL_Z: [&str; 4] = ["####", "..#.", ".#..", "####"];
const BIG_Z: [&str; 5] = ["#####", "...#.", "..#..", ".#...", "#####"];

/// Asleep: three Zs drifting up off its head by turns, swaying, growing
/// and cooling as they rise, each in a dark rim so it reads on any floor,
/// thinning away at the top.
fn sleep(pen: &mut Pen, b: &Body, tick: u64) {
    let head = b.head();
    for i in 0..3u32 {
        let (k, _) = looped(i, b.seed, 36.0, tick);
        let k = (k + i as f32 / 3.0) % 1.0;
        let x = head.0 + 2.0 + k * 9.0 + (k * TAU * 1.5).sin() * 1.5;
        let y = head.1 - 2.0 - k * 16.0;
        let glyph: &[&str] = if k < 0.4 { &SMALL_Z } else { &BIG_Z };
        let ink = DREAM.at(k * 0.75);
        let (x0, y0) = (x.round() as i32, y.round() as i32);
        for (dy, row) in glyph.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                if ch != '#' {
                    continue;
                }
                let (px, py) = (x0 + dx as i32, y0 + dy as i32);
                if k > 0.75 && bayer(px, py) < (k - 0.75) * 4.0 {
                    continue;
                }
                pen.put(px, py, ink);
                for (ox, oy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1)] {
                    pen.under(px + ox, py + oy, 'k');
                }
            }
        }
    }
}

/// Bleeding: two cuts across it, drops running down from them and falling
/// to a pool spreading under its feet.
fn bleed(pen: &mut Pen, b: &Body, tick: u64) {
    let r = ramps::BLOOD;
    let cuts = [(-0.2, 0.62, 1.0), (0.18, 0.4, -1.0)];
    for (i, &(fx, fy, dir)) in cuts.iter().enumerate() {
        let at = (b.feet.0 + fx * b.w, b.feet.1 - b.h * fy);
        for j in -2..=2 {
            let (x, y) = ((at.0 + j as f32).round() as i32, (at.1 + j as f32 * 0.6 * dir).round() as i32);
            pen.put(x, y, if j == 0 { '7' } else { 'p' });
            pen.put(x, y + 1, '8');
        }
        // A drop running down from it, then off and falling.
        let (k, _) = looped(i as u32, b.seed, 24.0, tick);
        let fall = ease::inward(k) * (b.feet.1 - at.1);
        let (x, y) = (at.0.round() as i32, (at.1 + 1.0 + fall).round() as i32);
        pen.put(x, y, '7');
        if k < 0.5 {
            pen.put(x, y - 1, 'p');
        }
    }
    // The pool under it, spreading and still.
    let grow = 0.6 + 0.4 * ((tick as f32 * 0.02 + b.seed as f32).sin() * 0.5 + 0.5);
    let (rx, ry) = (b.w * 0.45 * grow + 2.0, 2.0 * grow + 0.5);
    for dy in -(ry.ceil() as i32)..=(ry.ceil() as i32) {
        for dx in -(rx.ceil() as i32)..=(rx.ceil() as i32) {
            let d = (dx as f32 / rx).hypot(dy as f32 / ry);
            if d > 1.0 {
                continue;
            }
            let (x, y) = (b.feet.0.round() as i32 + dx, b.feet.1.round() as i32 + 1 + dy);
            pen.under(x, y, if d < 0.4 && dy < 0 { r.step(3) } else { r.step(4) });
        }
    }
}

/// Hexed: the hex's sigil turning over the frog, motes of it going round,
/// flickering and breaking as the hex wears out.
fn hex(pen: &mut Pen, b: &Body, tick: u64) {
    let c = (b.head().0, b.head().1 - 6.0);
    let t = tick as f32;
    pen.glyph(c, 6.0, 1.0, t * 0.06, 1.0, 5, HEX, 0.15 + 0.15 * (t * 0.2).sin().abs(), b.seed);
    for i in 0..3u32 {
        let a = t * 0.12 + i as f32 * TAU / 3.0;
        let at = (b.feet.0 + a.cos() * (b.w * 0.5 + 3.0), b.feet.1 - b.h * 0.5 + a.sin() * 3.0);
        pen.twinkle(at, 1.5, ((t * 0.04 + i as f32 / 3.0) % 1.0).min(0.99), HEX);
    }
}

/// Mired: sunk in a bog to the ankles — mud over the feet, its sheen, bubbles
/// coming up and popping, and fingers of it reaching up the legs.
fn mire(pen: &mut Pen, b: &Body, tick: u64) {
    let (rx, ry) = (b.w * 0.6 + 2.0, 3.0);
    let base = (b.feet.0, b.feet.1 - 1.0);
    for dy in -3..=3 {
        for dx in -(rx.ceil() as i32)..=(rx.ceil() as i32) {
            let d = (dx as f32 / rx).hypot(dy as f32 / ry);
            if d > 1.0 {
                continue;
            }
            let (x, y) = (base.0.round() as i32 + dx, base.1.round() as i32 + dy);
            let wave = ((dx as f32 * 0.6 + tick as f32 * 0.15).sin() * 0.5 + 0.5) > 0.82 && dy < 0;
            pen.put(x, y, if wave { MUD.step(1) } else { MUD.dithered(0.3 + d * 0.6, x, y) });
        }
    }
    for side in [-1.0f32, 1.0] {
        let x = (base.0 + side * 3.0).round() as i32;
        let up = 2 + ((tick / 6 + b.seed as u64) % 2) as i32;
        for j in 0..up {
            pen.put(x, base.1.round() as i32 - 3 - j, MUD.step(2));
        }
    }
    for i in 0..3u32 {
        let (k, n) = looped(i, b.seed, 16.0, tick);
        let x = base.0 + (unit(i as i32 + n * 7, b.seed as i32, 0xf1) - 0.5) * rx * 1.6;
        let y = base.1 - k * 4.0;
        let (px, py) = (x.round() as i32, y.round() as i32);
        if k < 0.7 {
            pen.light(px, py, MUD.step(0));
            pen.light(px + 1, py, MUD.step(1));
        } else {
            for (dx, dy) in [(-1, 0), (2, 0), (0, -1), (1, -1)] {
                pen.light(px + dx, py + dy, MUD.step(1));
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__ailments__tests.rs"]
mod tests;
