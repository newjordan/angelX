//! The pixel motion-graphics kit: what every spell, wish and wished arm is
//! drawn with.
//!
//! Small pieces that compose: an effect's beats (a wind-up, the burst, an
//! overshoot that settles, a linger), colour ramps out of the realm's inks,
//! a pen that keeps count of what it paints and never dims a figure with a
//! glow, and shapes that read at the room's own size — rings that open
//! with their thickness falling away, sparks that streak, rune circles that
//! draw themselves on and turn, lightning that forks, smoke that curls up
//! and thins through an ordered dither, shock rings with a dark wake,
//! twinkles, ribbons, and sprays of motes thrown, slowed and dropped.
//!
//! Everything is a pure function of what it is handed: a tick, an age, a
//! seed hashed from things the run already holds. Nothing is kept between
//! frames and nothing is rolled, so the host, a friend's mirror and a
//! browser guest draw the same pixels from the same run.

use super::super::ink::{Img, Rgb, bayer, hash, ink};
use std::f32::consts::{PI, TAU};

// ─── timing ──────────────────────────────────────────────────────────────────

/// Easing curves and an effect's beat sheet: the motion foundation's own
/// (`crate::motion`), so the magic kit times its effects as the rest of
/// the game does.
pub(crate) use crate::motion::{Beats, ease};

// ─── colour ──────────────────────────────────────────────────────────────────

/// Six inks from hottest to darkest: a core, its burn, the body, the rim,
/// the cooling, and what it dies to on black.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ramp(pub [char; 6]);

impl Ramp {
    /// The ink `k` (0 the core, 1 the dark) of the way down.
    pub fn at(&self, k: f32) -> char {
        self.0[((k.clamp(0.0, 0.999)) * 6.0) as usize]
    }

    /// Step `i` (0 the core).
    pub fn step(&self, i: usize) -> char {
        self.0[i.min(5)]
    }

    /// `at`, dithered between neighbouring steps by the world pixel, so a
    /// falloff is a gradient and not a staircase.
    pub fn dithered(&self, k: f32, x: i32, y: i32) -> char {
        let f = k.clamp(0.0, 1.0) * 5.0;
        let i = f.floor();
        let step = i as usize + usize::from(bayer(x, y) < f - i);
        self.0[step.min(5)]
    }

    /// Every ink `by` steps down: the same ramp, cooler, for a far layer.
    pub fn cooler(&self, by: usize) -> Ramp {
        Ramp(std::array::from_fn(|i| self.0[(i + by).min(5)]))
    }
}

/// The realm's magics in its own inks. The arcane, the void and a brew
/// burn down the violet dye (a white-hot core for the arcane, a pale lilac
/// for a brew, the void sinking from amethyst to the dark), and a rose
/// down the pink, from cream through blush to wine.
pub(crate) mod ramps {
    use super::Ramp;
    pub const FIRE: Ramp = Ramp(['9', '6', '@', 'R', 'p', 'B']);
    pub const EMBER: Ramp = Ramp(['6', '@', 'o', 'R', 'p', 'b']);
    pub const FROST: Ramp = Ramp(['w', 'W', 'V', 'z', 'Q', 'q']);
    pub const HOLY: Ramp = Ramp(['9', '6', 'T', 't', 'O', 'o']);
    pub const GOLD: Ramp = Ramp(['9', '6', '5', '4', 'O', 'r']);
    pub const ARCANE: Ramp = Ramp(['w', '^', ']', '[', ')', '(']);
    pub const GENIE: Ramp = Ramp(['w', '3', '2', '1', 'q', '0']);
    pub const STORM: Ramp = Ramp(['w', 'W', '3', 'z', '1', 'q']);
    pub const POISON: Ramp = Ramp(['Y', 'C', 'y', 'A', 'l', 'F']);
    pub const NATURE: Ramp = Ramp(['Y', 'y', 'M', 'L', 'l', 'E']);
    pub const BLOOD: Ramp = Ramp(['c', '7', '7', 'p', 'B', 'n']);
    pub const ROSE: Ramp = Ramp(['9', '+', '%', ';', '?', '!']);
    pub const POOP: Ramp = Ramp(['t', 'o', 'P', 'r', 'B', 'b']);
    pub const BREW: Ramp = Ramp(['^', ']', '[', ')', '(', 'd']);
    pub const VOID: Ramp = Ramp(['^', '[', ')', '(', 'd', 'K']);
    pub const WATER: Ramp = Ramp(['w', 'z', 'Q', 'q', 'S', 's']);
    pub const BONE: Ramp = Ramp(['H', 'i', 'h', 'J', 'G', 'j']);
    pub const SMOKE: Ramp = Ramp(['i', 'h', 'J', 'G', 'j', 'g']);
    pub const EARTH: Ramp = Ramp(['T', 'O', 'o', 'P', 'I', 'b']);
}

/// How bright an ink is, for a glow that must never dim what it lands on.
pub(crate) fn lum(c: Rgb) -> u32 {
    299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2])
}

/// A share in `0..1` hashed from `(a, b, s)`.
pub(crate) fn unit(a: i32, b: i32, s: u32) -> f32 {
    (hash(a, b, s) & 0xffff) as f32 / 65536.0
}

// ─── the pen ─────────────────────────────────────────────────────────────────

/// A canvas and the motes it has left to paint. Every shape goes through
/// it, so no frame paints more than its budget however much magic is in
/// the air; and under the calm-screen preference it paints softer.
pub(crate) struct Pen<'a> {
    pub cv: &'a mut Img,
    left: u32,
    pub calm: bool,
    /// Keep what is drawn: light lands only on bare paper, so a burst
    /// thrown up off a knight goes behind them and never swallows them.
    pub keep: bool,
}

impl<'a> Pen<'a> {
    pub fn new(cv: &'a mut Img, budget: u32) -> Pen<'a> {
        Pen { cv, left: budget, calm: crate::render::viewer::calm(), keep: false }
    }

    /// Motes left.
    pub fn left(&self) -> u32 {
        self.left
    }

    /// Whether something is already drawn at `(x, y)`: a figure the light
    /// should go behind.
    pub fn has(&self, x: i32, y: i32) -> bool {
        self.cv.get(x, y).is_some()
    }

    fn take(&mut self) -> bool {
        let room = self.left > 0;
        self.left = self.left.saturating_sub(1);
        room
    }

    /// `ch` at `(x, y)`, over whatever is there.
    pub fn put(&mut self, x: i32, y: i32, ch: char) {
        if self.take() {
            self.cv.put(x, y, ch);
        }
    }

    /// `ch` at `(x, y)` where it brightens what is there: light adds, so a
    /// glow passing over a knight leaves the knight.
    pub fn light(&mut self, x: i32, y: i32, ch: char) {
        let Some(c) = ink(ch) else { return };
        if self.cv.get(x, y).is_some_and(|was| self.keep || lum(was) >= lum(c)) {
            return;
        }
        if self.take() {
            self.cv.set(x, y, c);
        }
    }

    /// `ch` at `(x, y)` only on bare paper: smoke and wakes that go behind
    /// everything already drawn.
    pub fn under(&mut self, x: i32, y: i32, ch: char) {
        if self.cv.get(x, y).is_none() && self.take() {
            self.cv.put(x, y, ch);
        }
    }

    /// A glow: a bright core, a halo ramping down the inks, its edge
    /// dithered away to bare paper so the floor still shows through.
    /// `squash` flattens it onto the floor (1 round, 0.5 a floor pool).
    pub fn glow(&mut self, (cx, cy): (f32, f32), r: f32, squash: f32, ramp: Ramp, power: f32) {
        let power = if self.calm { power * 0.55 } else { power };
        if power <= 0.02 || r < 0.5 {
            return;
        }
        let ry = (r * squash).max(0.5);
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                let dx = (x as f32 + 0.5 - cx) / r;
                let dy = (y as f32 + 0.5 - cy) / ry;
                let d = (dx * dx + dy * dy).sqrt();
                if d >= 1.0 {
                    continue;
                }
                let i = power * (1.0 - d).powf(1.6);
                if i < 0.07 || (i < 0.32 && bayer(x, y) > (i - 0.07) / 0.25) {
                    continue;
                }
                // The halo goes behind what is drawn; only the hot core
                // burns over it, so a glow never swallows a figure.
                if i < 0.8 && self.cv.get(x, y).is_some() {
                    continue;
                }
                self.light(x, y, ramp.dithered(1.0 - i.min(1.0), x, y));
            }
        }
    }

    /// A ring of radius `r` (squashed by `squash` up and down), `thick`
    /// pixels each side of its line, brightest on the line and falling
    /// away; `fade` (0 to 1) dims and dissolves it.
    pub fn ring(&mut self, (cx, cy): (f32, f32), r: f32, thick: f32, squash: f32, ramp: Ramp, fade: f32) {
        let fade = fade.clamp(0.0, 1.0);
        if fade >= 1.0 || r <= 0.0 {
            return;
        }
        let thick = thick.max(0.6);
        let reach = r + thick;
        let ry = (reach * squash).max(1.0);
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - reach).floor() as i32..=(cx + reach).ceil() as i32 {
                let dx = x as f32 + 0.5 - cx;
                let dy = (y as f32 + 0.5 - cy) / squash;
                let e = ((dx * dx + dy * dy).sqrt() - r).abs() / thick;
                if e >= 1.0 {
                    continue;
                }
                let i = (1.0 - e) * (1.0 - fade);
                if i < 0.1 || (i < 0.4 && bayer(x, y) > (i - 0.1) / 0.3) {
                    continue;
                }
                self.light(x, y, ramp.dithered(1.0 - i, x, y));
            }
        }
    }

    /// A shockwave: a dark wake inside a bright edge, so the floor seems
    /// to bend as it passes.
    pub fn shock(&mut self, c: (f32, f32), r: f32, thick: f32, squash: f32, ramp: Ramp, fade: f32) {
        let (cx, cy) = c;
        let inner = (r - thick * 2.5).max(0.0);
        let ry = (r * squash).max(1.0);
        if fade < 0.8 {
            for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
                for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                    let dx = x as f32 + 0.5 - cx;
                    let dy = (y as f32 + 0.5 - cy) / squash;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < inner || d > r - thick * 0.6 {
                        continue;
                    }
                    if bayer(x, y) < 0.5 * (1.0 - fade) {
                        self.under(x, y, 'k');
                    }
                }
            }
        }
        self.ring(c, r, thick, squash, ramp, fade);
    }

    /// A line in `ch` from `a` to `b`, lit (`light`).
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), ch: char) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = dx.abs().max(dy.abs()).ceil().max(1.0) as i32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.light((a.0 + dx * t).round() as i32, (a.1 + dy * t).round() as i32, ch);
        }
    }

    /// A spark: its head at `at`, a streak `len` pixels back along `dir`
    /// cooling down `ramp`; `k` (0 to 1) how far through its life.
    pub fn streak(&mut self, at: (f32, f32), dir: (f32, f32), len: f32, ramp: Ramp, k: f32) {
        let l = dir.0.hypot(dir.1).max(1e-4);
        let (ux, uy) = (dir.0 / l, dir.1 / l);
        let n = len.round().max(1.0) as i32;
        for j in 0..n {
            let t = j as f32 / n as f32;
            let x = (at.0 - ux * j as f32).round() as i32;
            let y = (at.1 - uy * j as f32).round() as i32;
            if j > 1 && bayer(x, y) < t * 0.7 {
                continue;
            }
            self.light(x, y, ramp.at(k * 0.6 + t * 0.55));
        }
    }

    /// A four-pointed twinkle at `at`, `size` arms at its height; `phase`
    /// 0 to 1 is its life: it swells, holds a moment and goes.
    pub fn twinkle(&mut self, at: (f32, f32), size: f32, phase: f32, ramp: Ramp) {
        if !(0.0..1.0).contains(&phase) {
            return;
        }
        let s = (ease::bump(phase) * size + 0.3).floor() as i32;
        let (x, y) = (at.0.round() as i32, at.1.round() as i32);
        self.light(x, y, ramp.step(0));
        for a in 1..=s {
            let ch = ramp.step((a as usize).min(4));
            for (dx, dy) in [(a, 0), (-a, 0), (0, a), (0, -a)] {
                self.light(x + dx, y + dy, ch);
            }
        }
        if s >= 2 {
            for (dx, dy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
                self.light(x + dx, y + dy, ramp.step(3));
            }
        }
    }

    /// A rune circle: an outer ring drawn on clockwise as `open` goes 0 to
    /// 1, an inner one counter to it, a star of `sides` points between
    /// them once it is half open, and runes round the band; turned by
    /// `rot`, dissolving by `fade`.
    #[allow(clippy::too_many_arguments)]
    pub fn glyph(&mut self, (cx, cy): (f32, f32), r: f32, squash: f32, rot: f32, open: f32, sides: u32, ramp: Ramp, fade: f32, seed: u32) {
        if r < 2.0 || fade >= 1.0 {
            return;
        }
        let open = open.clamp(0.0, 1.0);
        let shown = |x: i32, y: i32| bayer(x, y) >= fade;
        let lit = |k: f32| (k + fade * 0.6).min(1.0);
        // The outer ring, drawn on.
        let n = (TAU * r * 1.1) as i32;
        for i in 0..n {
            let t = i as f32 / n as f32;
            if t > open {
                break;
            }
            let a = rot + t * TAU;
            let (x, y) = ((cx + a.cos() * r).round() as i32, (cy + a.sin() * r * squash).round() as i32);
            if shown(x, y) {
                self.light(x, y, ramp.at(lit(if i % 7 == 0 { 0.0 } else { 0.25 })));
            }
        }
        // The inner ring, the other way.
        let ri = r * 0.72;
        let m = (TAU * ri) as i32;
        for i in 0..m {
            let t = i as f32 / m as f32;
            if t > open || i % 2 == 1 {
                continue;
            }
            let a = -rot * 1.3 - t * TAU;
            let (x, y) = ((cx + a.cos() * ri).round() as i32, (cy + a.sin() * ri * squash).round() as i32);
            if shown(x, y) {
                self.light(x, y, ramp.at(lit(0.45)));
            }
        }
        // The star, its lines growing out of their points.
        let grow = ((open - 0.45) / 0.55).clamp(0.0, 1.0);
        if grow > 0.0 && sides >= 3 {
            let skip = if sides >= 5 { 2 } else { 1 };
            let vert = |k: u32| {
                let a = -rot * 1.3 + k as f32 / sides as f32 * TAU - PI / 2.0;
                (cx + a.cos() * ri, cy + a.sin() * ri * squash)
            };
            for k in 0..sides {
                let (a, b) = (vert(k), vert((k + skip) % sides));
                let b = (a.0 + (b.0 - a.0) * grow, a.1 + (b.1 - a.1) * grow);
                let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.0) as i32;
                for s in 0..=steps {
                    let t = s as f32 / steps as f32;
                    let (x, y) = ((a.0 + (b.0 - a.0) * t).round() as i32, (a.1 + (b.1 - a.1) * t).round() as i32);
                    if shown(x, y) && (s % 3 != 2) {
                        self.light(x, y, ramp.at(lit(0.55)));
                    }
                }
            }
        }
        // The runes round the band, each a little mark of its own.
        if open > 0.3 && r >= 6.0 {
            let rr = r * 0.86;
            let count = (sides * 2).max(6);
            for k in 0..count {
                let a = rot + k as f32 / count as f32 * TAU;
                let (x, y) = ((cx + a.cos() * rr).round() as i32, (cy + a.sin() * rr * squash).round() as i32);
                let mark = RUNES[(hash(k as i32, seed as i32, 0x7e) % RUNES.len() as u32) as usize];
                for (dy, row) in mark.iter().enumerate() {
                    for (dx, on) in row.iter().enumerate() {
                        let (px, py) = (x + dx as i32 - 1, y + dy as i32 - 1);
                        if *on && shown(px, py) {
                            self.light(px, py, ramp.at(lit(if dy == 0 { 0.1 } else { 0.3 })));
                        }
                    }
                }
            }
        }
    }

    /// A bolt of lightning from `a` to `b`: jagged by `jag` pixels, a
    /// white core with a coloured flank, forking once off its middle.
    pub fn arc(&mut self, a: (f32, f32), b: (f32, f32), seed: u32, jag: f32, ramp: Ramp, fade: f32) {
        let mut pts = [(0.0f32, 0.0f32); 17];
        bolt_points(&mut pts, a, b, seed, jag);
        self.polyline(&pts, ramp, fade, true);
        // The fork: off the middle, a shorter run toward one side.
        let m = pts[8];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let side = if hash(seed as i32, 3, 0xa5) & 1 == 0 { 1.0 } else { -1.0 };
        let end = (m.0 + dx * 0.35 - dy * 0.3 * side, m.1 + dy * 0.35 + dx * 0.3 * side);
        let mut fork = [(0.0f32, 0.0f32); 17];
        bolt_points(&mut fork, m, end, seed ^ 0x55, jag * 0.6);
        self.polyline(&fork[..9], ramp.cooler(1), (fade + 0.2).min(1.0), false);
    }

    fn polyline(&mut self, pts: &[(f32, f32)], ramp: Ramp, fade: f32, flank: bool) {
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.0) as i32;
            for i in 0..=n {
                let t = i as f32 / n as f32;
                let (x, y) = ((a.0 + (b.0 - a.0) * t).round() as i32, (a.1 + (b.1 - a.1) * t).round() as i32);
                if bayer(x, y) < fade {
                    continue;
                }
                self.light(x, y, ramp.at(fade * 0.5));
                if flank && fade < 0.5 {
                    self.light(x + 1, y, ramp.step(2));
                    if (x + y) % 2 == 0 {
                        self.light(x, y + 1, ramp.step(3));
                    }
                }
            }
        }
    }

    /// A tendril of smoke out of `base`: puffs let go from `emit.0` to
    /// `emit.1` ticks, each rising `rise` pixels over its `life`, curling
    /// `curl` pixels side to side and swelling as it thins through the
    /// dither; behind everything already drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn wisp(&mut self, base: (f32, f32), age: f32, emit: (f32, f32), life: f32, rise: f32, curl: f32, ramp: Ramp, seed: u32) {
        let phase = unit(seed as i32, 1, 0x3b) * TAU;
        let first = (age - life).max(emit.0).ceil() as i32;
        let last = age.min(emit.1).floor() as i32;
        for e in first..=last {
            let a = age - e as f32;
            let t = a / life;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let x = base.0 + (a * 0.32 + phase).sin() * curl * (0.2 + t) + (t * t) * curl * 0.6;
            let y = base.1 - rise * ease::out(t);
            let r = 1.0 + t * 3.0;
            let fill = (1.0 - t).powf(1.3);
            let ch = ramp.at(0.15 + t * 0.85);
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if (dx * dx + dy * dy) as f32 > r * r {
                        continue;
                    }
                    let (px, py) = ((x + dx as f32).round() as i32, (y + dy as f32).round() as i32);
                    if bayer(px, py) < fill {
                        self.under(px, py, ch);
                    }
                }
            }
        }
    }

    /// A ribbon along `pts`, head first: thick and hot at its head,
    /// thinning and dotted toward its tail.
    pub fn ribbon(&mut self, pts: &[(f32, f32)], ramp: Ramp, width: f32) {
        let len = pts.len().max(2) as f32;
        for (i, w) in pts.windows(2).enumerate() {
            let k = i as f32 / (len - 1.0);
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dx.hypot(dy).max(1e-3);
            let (nx, ny) = (-dy / l, dx / l);
            let n = l.ceil().max(1.0) as i32;
            for s in 0..n {
                let t = s as f32 / n as f32;
                let (x, y) = (a.0 + dx * t, a.1 + dy * t);
                let (px, py) = (x.round() as i32, y.round() as i32);
                if k > 0.55 && bayer(px, py) < (k - 0.55) * 2.0 {
                    continue;
                }
                self.light(px, py, ramp.at(0.05 + k * 0.9));
                let wide = width * (1.0 - k);
                if wide >= 0.8 {
                    self.light((x + nx).round() as i32, (y + ny).round() as i32, ramp.at(0.3 + k * 0.7));
                }
                if wide >= 1.6 {
                    self.light((x - nx).round() as i32, (y - ny).round() as i32, ramp.at(0.4 + k * 0.6));
                }
            }
        }
    }
}

/// Little runes, three by three: the marks round a rune circle's band.
const RUNES: [[[bool; 3]; 3]; 8] = {
    const O: bool = false;
    const X: bool = true;
    [
        [[X, X, X], [O, X, O], [O, X, O]],
        [[X, O, X], [O, X, O], [X, O, X]],
        [[O, X, O], [X, X, X], [O, X, O]],
        [[X, X, O], [X, O, O], [X, X, X]],
        [[O, X, X], [O, X, O], [X, X, O]],
        [[X, O, X], [X, X, X], [O, O, X]],
        [[O, X, O], [X, O, X], [O, X, O]],
        [[X, X, X], [X, O, X], [O, O, O]],
    ]
};

/// A jagged run from `a` to `b` into `pts`, by midpoint displacement:
/// worst in the middle, pinned at both ends.
fn bolt_points(pts: &mut [(f32, f32); 17], a: (f32, f32), b: (f32, f32), seed: u32, jag: f32) {
    pts[0] = a;
    pts[16] = b;
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx.hypot(dy).max(1e-3);
    let (nx, ny) = (-dy / l, dx / l);
    let mut step = 16;
    let mut amp = jag;
    while step > 1 {
        let half = step / 2;
        let mut i = half;
        while i < 16 {
            let (p, q) = (pts[i - half], pts[i + half]);
            let off = (unit(i as i32, seed as i32, step as u32) - 0.5) * 2.0 * amp;
            pts[i] = ((p.0 + q.0) / 2.0 + nx * off, (p.1 + q.1) / 2.0 + ny * off);
            i += step;
        }
        step = half;
        amp *= 0.55;
    }
}

// ─── sprays ──────────────────────────────────────────────────────────────────

/// The most motes one spray throws.
pub(crate) const MOST_MOTES: u32 = 48;

/// A spray of motes thrown from one point: `n` of them along `angle`,
/// `spread` radians wide, each at a speed and a life of its own between
/// the bounds (pixels a tick, ticks), slowed by `drag`, pulled by
/// `gravity` (pixels a tick a tick; less than nothing floats up), flattened
/// by `squash` onto the floor, and let go `stagger` ticks apart.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spray {
    pub seed: u32,
    pub n: u32,
    pub at: (f32, f32),
    pub angle: f32,
    pub spread: f32,
    pub speed: (f32, f32),
    pub life: (f32, f32),
    pub drag: f32,
    pub gravity: f32,
    pub squash: f32,
    pub stagger: f32,
}

/// One mote of a spray as it is now.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mote {
    pub i: u32,
    pub at: (f32, f32),
    pub vel: (f32, f32),
    /// How far through its life, 0 to 1.
    pub k: f32,
    pub seed: u32,
}

impl Spray {
    /// A spray thrown every way at once.
    pub fn round(seed: u32, n: u32, at: (f32, f32), speed: (f32, f32), life: (f32, f32)) -> Spray {
        Spray { seed, n, at, angle: 0.0, spread: TAU, speed, life, drag: 0.12, gravity: 0.0, squash: 1.0, stagger: 0.0 }
    }

    /// Each mote alive `age` ticks after the throw.
    pub fn each(&self, age: f32, mut f: impl FnMut(Mote)) {
        for i in 0..self.n.min(MOST_MOTES) {
            let u = |s: u32| unit(i as i32, self.seed as i32, s);
            let t = age - i as f32 * self.stagger;
            let life = self.life.0 + (self.life.1 - self.life.0) * u(1);
            if t < 0.0 || t >= life {
                continue;
            }
            let even = (i as f32 + 0.5) / self.n.max(1) as f32 - 0.5;
            let a = self.angle + (even * 0.6 + (u(2) - 0.5) * 0.4) * self.spread;
            let v = self.speed.0 + (self.speed.1 - self.speed.0) * u(3);
            let (run, now) = if self.drag > 0.0 {
                let f = crate::motion::math::exp(-self.drag * t);
                (v * (1.0 - f) / self.drag, v * f)
            } else {
                (v * t, v)
            };
            let at = (
                self.at.0 + a.cos() * run,
                self.at.1 + a.sin() * run * self.squash + 0.5 * self.gravity * t * t,
            );
            let vel = (a.cos() * now, a.sin() * now * self.squash + self.gravity * t);
            f(Mote { i, at, vel, k: t / life, seed: hash(i as i32, self.seed as i32, 0x99) });
        }
    }
}

/// The shapes a mote may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Spark,
    Dot,
    Droplet,
    Shard,
    Bubble,
    Note,
    Star,
    Flake,
    Petal,
    Fly,
    Coin,
    Heart,
    Worm,
    Smoke,
    Ember,
}

/// The eighth note a song's mote is drawn as, four by five.
const NOTE: [&str; 5] = ["..##", "..#.", "..#.", "###.", "##.."];
const HEART: [&str; 3] = ["#.#", "###", ".#."];

/// Draw mote `m` as `shape` in `ramp`, `size` 1 to 3, at `tick`.
pub(crate) fn mote(pen: &mut Pen, m: &Mote, shape: Shape, ramp: Ramp, size: u8, tick: u64) {
    let (x, y) = (m.at.0.round() as i32, m.at.1.round() as i32);
    let k = m.k;
    let t = tick as u32;
    let spin = (t / 3 + m.seed) % 4;
    match shape {
        Shape::Spark => {
            let speed = m.vel.0.hypot(m.vel.1);
            let len = (1.0 + speed * 1.6).min(3.0 + size as f32 * 2.0);
            pen.streak(m.at, m.vel, len, ramp, k);
        }
        Shape::Ember => {
            pen.light(x, y, ramp.at(k * 0.8));
            if k < 0.4 && (t + m.seed) % 3 != 0 {
                pen.light(x, y - 1, ramp.at(0.5 + k));
            }
        }
        Shape::Dot => {
            pen.light(x, y, ramp.at(k * 0.9));
            if size >= 2 && k < 0.6 {
                for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
                    pen.light(x + dx, y + dy, ramp.at(0.25 + k * 0.7));
                }
            }
        }
        Shape::Droplet => {
            let (vx, vy) = m.vel;
            let l = vx.hypot(vy).max(1e-3);
            pen.light(x, y, ramp.at(0.1 + k * 0.7));
            pen.light((m.at.0 - vx / l).round() as i32, (m.at.1 - vy / l).round() as i32, ramp.at(0.35 + k * 0.6));
            if size >= 2 {
                pen.light(x + 1, y, ramp.at(0.3 + k * 0.6));
                pen.light(x, y + 1, ramp.at(0.5 + k * 0.5));
            }
        }
        Shape::Shard => {
            let dirs = [(1, -1), (1, 0), (1, 1), (0, 1)];
            let (dx, dy) = dirs[spin as usize];
            pen.light(x, y, ramp.at(k * 0.6));
            for j in 1..=i32::from(size.max(1)) {
                pen.light(x + dx * j, y + dy * j, ramp.at(0.3 + k * 0.6));
            }
            if k < 0.3 {
                pen.light(x - dx, y - dy, ramp.step(0));
            }
        }
        Shape::Bubble => {
            let r = i32::from(size.max(1));
            let ring: &[(i32, i32)] = if r == 1 {
                &[(-1, 0), (1, 0), (0, -1), (0, 1)]
            } else {
                &[(-2, 0), (2, 0), (0, -2), (0, 2), (-1, -1), (1, -1), (-1, 1), (1, 1)]
            };
            for &(dx, dy) in ring {
                pen.light(x + dx, y + dy, ramp.at(0.25 + k * 0.6));
            }
            pen.light(x - r + 1, y - r + 1 - i32::from(r == 1), ramp.step(0));
        }
        Shape::Note => {
            for (dy, row) in NOTE.iter().enumerate() {
                for (dx, c) in row.chars().enumerate() {
                    if c == '#' {
                        pen.light(x + dx as i32 - 2, y + dy as i32 - 4, ramp.at(if dy == 0 { 0.0 } else { 0.2 + k * 0.6 }));
                    }
                }
            }
        }
        Shape::Star => {
            pen.twinkle(m.at, 1.0 + f32::from(size), k, ramp);
        }
        Shape::Flake => {
            pen.light(x, y, ramp.step(0));
            let arm = ramp.at(0.25 + k * 0.6);
            if spin % 2 == 0 {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    pen.light(x + dx, y + dy, arm);
                }
            } else {
                for (dx, dy) in [(1, 1), (-1, -1), (1, -1), (-1, 1)] {
                    pen.light(x + dx, y + dy, arm);
                }
            }
        }
        Shape::Petal => {
            let (dx, dy) = [(1, 0), (1, 1), (0, 1), (-1, 1)][spin as usize];
            pen.light(x, y, ramp.at(0.15 + k * 0.6));
            pen.light(x + dx, y + dy, ramp.at(0.35 + k * 0.6));
        }
        Shape::Fly => {
            // A fly never goes straight: a jitter a tick about its line.
            let jx = (hash(m.seed as i32, t as i32, 0xf1) % 3) as i32 - 1;
            let jy = (hash(m.seed as i32, t as i32, 0xf2) % 3) as i32 - 1;
            let (x, y) = (x + jx, y + jy);
            pen.put(x, y, 'K');
            let wing = if t.is_multiple_of(2) { 'V' } else { 'h' };
            pen.light(x - 1, y - 1, wing);
            pen.light(x + 1, y - 1, wing);
        }
        Shape::Coin => {
            let face = (t / 2 + m.seed) % 4;
            pen.light(x, y, ramp.step(if face == 0 { 0 } else { 2 }));
            pen.light(x, y + 1, ramp.step(4));
            if face != 2 {
                pen.light(x + 1, y, ramp.step(3));
                pen.light(x + 1, y + 1, ramp.step(4));
            }
        }
        Shape::Heart => {
            for (dy, row) in HEART.iter().enumerate() {
                for (dx, c) in row.chars().enumerate() {
                    if c == '#' {
                        pen.light(x + dx as i32 - 1, y + dy as i32 - 1, ramp.at(if dx == 0 && dy == 0 { 0.0 } else { 0.25 + k * 0.6 }));
                    }
                }
            }
        }
        Shape::Worm => {
            let (vx, vy) = m.vel;
            let l = vx.hypot(vy).max(1e-3);
            let (ux, uy) = if l > 0.05 { (vx / l, vy / l) } else { (1.0, 0.0) };
            for j in 0..(3 + i32::from(size)) {
                let w = ((j as f32 * 1.7) + tick as f32 * 0.7 + m.seed as f32).sin();
                let px = m.at.0 - ux * j as f32 - uy * w;
                let py = m.at.1 - uy * j as f32 + ux * w;
                pen.light(px.round() as i32, py.round() as i32, ramp.at(if j == 0 { 0.1 } else { 0.3 + k * 0.5 }));
            }
        }
        Shape::Smoke => {
            // A puff: round from the start, swelling and thinning.
            let r = 1.4 + k * (1.5 + f32::from(size));
            let ri = r.ceil() as i32;
            let fill = 0.15 + 0.85 * (1.0 - k).powf(0.8);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if (dx * dx + dy * dy) as f32 <= r * r && bayer(x + dx, y + dy) < fill {
                        pen.under(x + dx, y + dy, ramp.at(0.3 + k * 0.7));
                    }
                }
            }
        }
    }
}

/// A star burst: `n` rays from `c`, each `len` pixels at its longest,
/// shooting out with an overshoot and drawing back as `k` goes 0 to 1.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rays(pen: &mut Pen, c: (f32, f32), n: u32, len: f32, squash: f32, rot: f32, ramp: Ramp, k: f32) {
    if k >= 1.0 {
        return;
    }
    let reach = len * ease::back((k * 2.2).min(1.0));
    let tail = len * ease::out(((k - 0.25) / 0.75).max(0.0));
    for i in 0..n {
        let a = rot + i as f32 / n as f32 * TAU;
        let long = if i % 2 == 0 { 1.0 } else { 0.55 };
        let (ux, uy) = (a.cos(), a.sin() * squash);
        let (from, to) = (tail * long, reach * long);
        let steps = (to - from).ceil().max(0.0) as i32;
        for s in 0..=steps {
            let d = from + s as f32;
            let t = d / (len * long).max(1.0);
            let (x, y) = ((c.0 + ux * d).round() as i32, (c.1 + uy * d).round() as i32);
            if t > 0.6 && bayer(x, y) < (t - 0.6) * 2.5 {
                continue;
            }
            pen.light(x, y, ramp.at(k * 0.5 + t * 0.6));
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__pfx__tests.rs"]
mod tests;
