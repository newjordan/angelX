//! Starfall drawn (`rules::starfall`): a shooting star crossing the dark
//! over a quiet room, its fall and its landing, the shard of starlight it
//! leaves on the floor, the knight who takes it, and a wish made upon it
//! landing on its knight.
//!
//! The star glints high in a corner of the view, then crosses it: a
//! white-hot head in a gold halo, a cross of light about it, a long tail
//! cooling through lilac and violet into the dark, sparks of rose and gold
//! shed off it and falling. It bends down at the end and dives, and where it
//! lands the floor rings, light is thrown up in rays and stars are flung
//! out. What it leaves is a five-pointed shard hovering over a pool of its
//! own light, breathing, glinting, letting motes go up; it fades if no one
//! comes. Taken, it flares and its light leaps to the knight. A star wish
//! landing is the star's own: a little star falls out of the dark onto the
//! knight, and a five-pointed star of gold opens about them, turning, with
//! a shower of stars — never the lamp's gold ring.
//!
//! The path is worked out for each seat's own view (a great hall's camera
//! sees its own sky), and ends where the rules put the star down, so every
//! seat sees it land in the same place.

use super::super::chrome::{self, Tone};
use super::super::ink::{Img, bayer, hash, text_width};
use super::pfx::{self, Pen, Ramp, Shape, Spray, ease, ramps};
use super::{NATIVE_W, UNIT, wishfx};
use crate::rules::starfall::{self as sf, Fall, Star};
use crate::rules::Run;
use std::f32::consts::{PI, TAU};

/// The most motes a pass paints.
const BUDGET: u32 = 2600;

/// Starlight: white-hot through a warm white and blush to lilac and violet.
const CORE: Ramp = Ramp(['w', '9', '+', '^', ']', '[']);
/// A fallen star's own gold, from its white heart to its rim.
const STARGOLD: Ramp = Ramp(['w', '9', '6', '5', '4', 'O']);
/// The tail, cooling into the dark.
const TAIL: Ramp = ramps::ARCANE;
/// The sparks it sheds, and a shard's motes.
const BLUSH: Ramp = ramps::ROSE;
const GILT: Ramp = ramps::HOLY;

/// The star's head at `age` ticks after it glinted, in room pixels, for a
/// seat whose view's top left is `view`: none before it glints or once it
/// is down.
fn head(star: &Star, view: (i32, i32), age: f32) -> Option<(f32, f32)> {
    if age < 0.0 || age > sf::LANDS as f32 {
        return None;
    }
    let (s, p, c, l) = path(star, view);
    let glint = sf::GLINT as f32;
    let dive = sf::DIVE as f32;
    if age < glint {
        return Some(s);
    }
    if age < dive {
        // Across the top of the view, falling a little more as it goes.
        let u = (age - glint) / (dive - glint);
        let x = s.0 + (p.0 - s.0) * u;
        let bow = 10.0 + (star.seed >> 3) as f32 % 8.0;
        let y = s.1 + (p.1 - s.1) * u * u - bow * (u * PI).sin() * (1.0 - u);
        return Some((x, y));
    }
    // The dive: on along its line, bending down onto where it lands, ever
    // faster.
    let v = ((age - dive) / (sf::LANDS as f32 - dive)).clamp(0.0, 1.0);
    let e = 0.4 * v + 0.6 * v * v;
    let w = 1.0 - e;
    Some((w * w * p.0 + 2.0 * w * e * c.0 + e * e * l.0, w * w * p.1 + 2.0 * w * e * c.1 + e * e * l.1))
}

/// Where the star glints, where its dive begins, the dive's bend, and
/// where it comes down: it crosses from the side of the view away from
/// where it lands, so its crossing is long.
fn path(star: &Star, view: (i32, i32)) -> ((f32, f32), (f32, f32), (f32, f32), (f32, f32)) {
    let (vx, vy) = (view.0 as f32, view.1 as f32);
    let w = NATIVE_W as f32;
    let l = (star.land.0 * UNIT, star.land.1 * UNIT - 7.0);
    let from_left = l.0 > vx + w / 2.0 || (l.0 - (vx + w / 2.0)).abs() < 4.0 && star.from_left();
    let dir = if from_left { 1.0 } else { -1.0 };
    // Below the room's top wall and its signs: the dark over the floor.
    let jitter = (star.seed % 13) as f32;
    let s = (if from_left { vx + 20.0 } else { vx + w - 20.0 }, vy + 66.0 + jitter);
    let px = (l.0 - dir * (52.0 + (star.seed >> 5) as f32 % 18.0)).clamp(vx + 40.0, vx + w - 40.0);
    let py = (vy + 90.0 + (star.seed >> 9) as f32 % 14.0).min(l.1 - 18.0).max(vy + 70.0);
    let p = (px, py);
    let c = (p.0 + dir * 40.0, p.1 + (l.1 - p.1) * 0.25);
    (s, p, c, l)
}

/// The star whose room the seat looks into.
fn star_here(run: &Run) -> Option<&Star> {
    run.starfall.star.as_ref().filter(|s| s.room == run.at)
}

/// Under the figures: the floor lit where the star will land as it dives,
/// the ring its landing throws, the pool of light under its shard, and the
/// rings under a catch and a star wish's landing.
pub(super) fn ground(cv: &mut Img, run: &Run) {
    let mut pen = Pen::new(cv, BUDGET / 2);
    let tick = run.tick;
    if let Some(star) = star_here(run) {
        let age = star.age(tick) as f32;
        let (lx, ly) = (star.land.0 * UNIT, star.land.1 * UNIT);
        // The floor brightens where it will come down.
        if (sf::DIVE as f32 - 14.0..sf::LANDS as f32).contains(&age) {
            let k = (age - (sf::DIVE as f32 - 14.0)) / (sf::LANDS as f32 - sf::DIVE as f32 + 14.0);
            pen.glow((lx, ly), 6.0 + 16.0 * ease::inward(k), 0.45, CORE.cooler(1), 0.3 + 0.6 * k);
        }
        // Down: the floor rings.
        let since = age - sf::LANDS as f32;
        if (0.0..34.0).contains(&since) {
            pen.shock((lx, ly), 4.0 + ease::out(since / 22.0) * 46.0, 2.2, 0.45, CORE, ease::inward((since / 30.0).min(1.0)));
            pen.ring((lx, ly), 3.0 + ease::out(since / 16.0) * 26.0, 1.4, 0.45, BLUSH, ease::inward((since / 24.0).min(1.0)));
        }
        if let Some((_, life)) = run.starfall.shard(tick) {
            let fade = shard_fade(life);
            let breath = 0.5 + 0.5 * ((tick as f32) * 0.11).sin();
            pen.glow((lx, ly), 16.0 + 3.0 * breath, 0.42, CORE.cooler(1), (0.55 + 0.25 * breath) * (1.0 - fade));
            // A star's ring on the floor, turning slowly, coming on as it lands.
            let open = ease::back(((age - sf::LANDS as f32) / 14.0).min(1.0));
            pen.glyph((lx, ly + 0.5), 13.0 * open, 0.42, tick as f32 * 0.012, open, 5, TAIL, 0.25 + fade * 0.75, star.seed);
        }
        if let Fall::Taken { knight, tick: at } = star.fall
            && let Some(hero) = run.players.get(&knight)
        {
            let since = tick.saturating_sub(at) as f32;
            if since < sf::CAUGHT_TICKS as f32 {
                let (feet, _) = wishfx::body(run, hero);
                pen.shock((feet.0, feet.1 + 1.0), 3.0 + ease::out(since / 18.0) * 30.0, 1.8, 0.45, GILT, ease::inward((since / 26.0).min(1.0)));
            }
        }
    }
    for g in &run.starfall.granted {
        let Some(hero) = run.players.get(&g.knight).filter(|h| h.hp > 0 && !h.absent) else { continue };
        let since = tick.saturating_sub(g.tick) as f32 - FALLS;
        if since < 0.0 {
            continue;
        }
        let (feet, _) = wishfx::body(run, hero);
        let open = ease::back((since / 9.0).min(1.0));
        let fade = ((since - 24.0) / 36.0).clamp(0.0, 1.0);
        pen.glyph((feet.0, feet.1 + 1.0), 15.0 * open, 0.42, -since * 0.05, open, 5, CORE, fade, g.knight ^ 0x57a2);
        pen.ring((feet.0, feet.1 + 1.0), 4.0 + ease::out(since / 16.0) * 28.0, 1.5, 0.42, BLUSH, ease::inward((since / 22.0).min(1.0)));
    }
}

/// How far a shard has faded at `life` (0 to 1 of its life): nothing until
/// its last seconds.
fn shard_fade(life: f32) -> f32 {
    let from = 1.0 - sf::SHARD_FADE as f32 / sf::SHARD_LIFE as f32;
    ((life - from) / (1.0 - from)).clamp(0.0, 1.0)
}

/// Over the figures: the star in the sky and its landing, the shard, a
/// catch, a star wish landing, the little stars over every knight who holds
/// a star's wish, and the banner that calls it.
pub(super) fn over(cv: &mut Img, run: &Run, legible: bool, view: (i32, i32), focus: Option<u32>) {
    let tick = run.tick;
    {
        let mut pen = Pen::new(cv, BUDGET);
        if let Some(star) = star_here(run) {
            let age = star.age(tick) as f32;
            sky(&mut pen, star, view, age, tick);
            landing(&mut pen, star, age, tick);
            if let Some((_, life)) = run.starfall.shard(tick) {
                shard(&mut pen, star, life, tick);
            }
            if let Fall::Taken { knight, tick: at } = star.fall
                && let Some(hero) = run.players.get(&knight)
            {
                caught(&mut pen, run, star, hero, tick.saturating_sub(at) as f32, tick);
            }
        }
        for g in &run.starfall.granted {
            if let Some(hero) = run.players.get(&g.knight).filter(|h| h.hp > 0 && !h.absent) {
                granted(&mut pen, run, hero, view, tick.saturating_sub(g.tick) as f32, g.knight, tick);
            }
        }
        // Who holds a star's wish: a little star of their own over them.
        for (&id, hero) in run.players.iter().filter(|(_, h)| h.star_wish && h.hp > 0 && !h.absent) {
            let (_, mid) = wishfx::body(run, hero);
            let bob = ((tick as f32) * 0.15 + id as f32).sin() * 1.2;
            let at = (mid.0 - 7.0, mid.1 - 18.0 + bob);
            pen.glow(at, 4.0, 1.0, TAIL, 0.55);
            let phase = ((tick + u64::from(id) * 7) % 36) as f32 / 36.0;
            pen.twinkle(at, 2.6, 0.2 + phase * 0.6, CORE);
        }
    }
    if legible && let Some(star) = star_here(run) {
        banner(cv, run, star, view, focus);
    }
    if legible
        && let Some(star) = star_here(run)
        && let Fall::Taken { knight, tick: at } = star.fall
        && let Some(hero) = run.players.get(&knight)
    {
        yours(cv, run, hero, view, tick.saturating_sub(at));
    }
}

/// The star in the sky: its glint, its head and halo, the cross of light
/// about it, its tail and the sparks it sheds.
fn sky(pen: &mut Pen, star: &Star, view: (i32, i32), age: f32, tick: u64) {
    let Some(at) = head(star, view, age) else { return };
    let glint = sf::GLINT as f32;
    if age < glint {
        // A glint high in the dark, growing.
        let k = age / glint;
        pen.glow(at, 3.0 + 7.0 * k, 1.0, CORE, 0.4 + 0.5 * k);
        pen.twinkle(at, 2.0 + 5.0 * k, 0.5, CORE);
        return;
    }
    // Its tail: back along the way it came, a set length of it however
    // slowly it goes, thick and hot at the head and cooling to nothing.
    let pts = trail(star, view, age, TAIL_LEN);
    tail(pen, &pts, 4.6);
    // Little stars left in its wake, twinkling out where it passed.
    let first = ((age - 40.0).max(sf::GLINT as f32) / 5.0).ceil() as i32;
    for e in first..=(age / 5.0) as i32 {
        let Some(p) = head(star, view, e as f32 * 5.0) else { continue };
        let h = hash(e, star.seed as i32, 0x57f3);
        let off = ((h % 9) as f32 - 4.0, ((h >> 4) % 9) as f32 - 4.0);
        let phase = (age - e as f32 * 5.0) / 40.0;
        pen.twinkle((p.0 + off.0, p.1 + off.1), 2.0 + (h % 2) as f32, phase, if h % 3 == 0 { GILT } else { CORE });
    }
    // Sparks shed off it as it went, drifting down and going out.
    let first = (age - 34.0).max(glint).ceil() as i32;
    for e in (first..=age as i32).filter(|e| e % 2 == 0) {
        let Some(from) = head(star, view, e as f32) else { continue };
        let a = age - e as f32;
        let k = a / 34.0;
        let h = hash(e, star.seed as i32, 0x57f1);
        let drift = ((h % 200) as f32 / 100.0 - 1.0) * 0.35;
        let p = (from.0 + drift * a, from.1 + 0.4 * a + 0.012 * a * a);
        let m = pfx::Mote { i: e as u32, at: p, vel: (drift, 0.4 + 0.024 * a), k, seed: h };
        let (shape, ramp) = match h % 5 {
            0 => (Shape::Star, GILT),
            1 | 2 => (Shape::Dot, BLUSH),
            3 => (Shape::Star, BLUSH),
            _ => (Shape::Ember, TAIL),
        };
        pfx::mote(pen, &m, shape, ramp, 1, tick);
    }
    // The head: a halo of gold, a cross of light, and a white-hot core.
    let pulse = 0.5 + 0.5 * (age * 0.6).sin();
    // Hottest first: a glow only lights what is darker than it.
    pen.glow(at, 5.0, 1.0, CORE, 1.0);
    pen.glow(at, 10.0 + 2.0 * pulse, 1.0, GILT, 1.0);
    pen.glow(at, 16.0 + 3.0 * pulse, 1.0, TAIL, 0.65);
    let (x, y) = (at.0.round() as i32, at.1.round() as i32);
    let arm = 8 + (pulse * 5.0) as i32;
    for d in 1..=arm {
        let ch = CORE.step((d as usize * 5 / arm as usize).min(5));
        for (dx, dy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
            pen.light(x + dx, y + dy, ch);
        }
    }
    for d in 1..=arm / 2 {
        let ch = CORE.step(2 + d as usize * 3 / (arm as usize / 2).max(1));
        for (dx, dy) in [(d, d), (-d, d), (d, -d), (-d, -d)] {
            pen.light(x + dx, y + dy, ch);
        }
    }
    for dy in -1..=1 {
        for dx in -1..=1 {
            pen.put(x + dx, y + dy, if dx == 0 || dy == 0 { 'w' } else { '9' });
        }
    }
}

/// How long the star's tail is, in pixels along its way.
const TAIL_LEN: f32 = 120.0;

/// The way behind the star at `age`, head first, a pixel or so apart, as
/// far back as `len` pixels (or to its glint).
fn trail(star: &Star, view: (i32, i32), age: f32, len: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(len as usize + 2);
    let Some(mut last) = head(star, view, age) else { return out };
    out.push(last);
    let mut gone = 0.0;
    let mut a = age;
    while gone < len && a > sf::GLINT as f32 {
        a -= 0.25;
        let Some(p) = head(star, view, a.max(sf::GLINT as f32)) else { break };
        let d = (p.0 - last.0).hypot(p.1 - last.1);
        if d >= 1.0 {
            gone += d;
            out.push(p);
            last = p;
        }
        if out.len() > 400 {
            break;
        }
    }
    out
}

/// A tail along `pts` (head first): `thick` pixels to each side at the
/// head, thinning to a thread, its middle hot, its edges cooling into the
/// dark and dissolving toward its end.
fn tail(pen: &mut Pen, pts: &[(f32, f32)], thick: f32) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    for i in 0..n - 1 {
        let k = i as f32 / (n - 1) as f32;
        let (a, b) = (pts[i], pts[i + 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy).max(1e-3);
        let (nx, ny) = (-dy / l, dx / l);
        let th = thick * (1.0 - k).powf(0.9) + 0.6;
        let reach = th.ceil() as i32 + 1;
        for o in -reach..=reach {
            let off = o as f32;
            let across = (off.abs() / (th + 1.0)).min(1.0);
            let i_ = (1.0 - across).powf(1.3) * (1.0 - k).powf(0.7);
            let (x, y) = ((a.0 + nx * off).round() as i32, (a.1 + ny * off).round() as i32);
            if i_ < 0.06 || (i_ < 0.4 && bayer(x, y) > (i_ - 0.06) / 0.34) {
                continue;
            }
            let ramp = if across < 0.3 && k < 0.45 { CORE } else { TAIL };
            pen.light(x, y, ramp.dithered(1.0 - i_, x, y));
        }
    }
}

/// Ticks after a landing its burst is drawn.
const LANDING_TICKS: f32 = 40.0;

/// Where it comes down: a flash, rays of light thrown up, stars flung out
/// and falling.
fn landing(pen: &mut Pen, star: &Star, age: f32, tick: u64) {
    let since = age - sf::LANDS as f32;
    if !(0.0..LANDING_TICKS).contains(&since) {
        return;
    }
    let at = (star.land.0 * UNIT, star.land.1 * UNIT - 7.0);
    if since < 10.0 {
        pen.glow(at, 22.0 * (1.0 - since / 10.0) + 4.0, 1.0, CORE, 1.0 - since / 12.0);
    }
    pfx::rays(pen, at, 10, 30.0, 0.85, star.seed as f32 * 0.01, GILT, (since / 22.0).min(1.0));
    pfx::rays(pen, at, 5, 18.0, 0.85, star.seed as f32 * 0.01 + PI / 5.0, CORE, (since / 16.0).min(1.0));
    let spray = Spray {
        gravity: 0.09,
        angle: -PI / 2.0,
        spread: 4.2,
        drag: 0.06,
        ..Spray::round(star.seed ^ 0x1a4d, if pen.calm { 12 } else { 24 }, at, (1.4, 3.8), (18.0, 32.0))
    };
    spray.each(since, |m| {
        let (shape, ramp) = match m.i % 4 {
            0 => (Shape::Star, GILT),
            1 => (Shape::Star, BLUSH),
            2 => (Shape::Spark, CORE),
            _ => (Shape::Dot, TAIL),
        };
        pfx::mote(pen, &m, shape, ramp, 1, tick);
    });
}

/// The outer points of a five-pointed star round `c`, its top point up
/// when `rot` is zero, `r` out and `squash` up and down.
fn points(c: (f32, f32), r: f32, inner: f32, rot: f32, squash: f32) -> [(f32, f32); 10] {
    std::array::from_fn(|i| {
        let a = rot - PI / 2.0 + i as f32 * PI / 5.0;
        let d = if i % 2 == 0 { r } else { r * inner };
        (c.0 + a.cos() * d, c.1 + a.sin() * d * squash)
    })
}

/// Whether `(x, y)` lies inside the polygon `poly`.
fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut inn = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.1 > y) != (b.1 > y) && x < (b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0 {
            inn = !inn;
        }
        j = i;
    }
    inn
}

/// A solid five-pointed star: lit from its middle out down `ramp`, its
/// rim a shade down, dissolving by `fade`.
fn solid_star(pen: &mut Pen, c: (f32, f32), r: f32, rot: f32, ramp: Ramp, fade: f32) {
    let poly = points(c, r, 0.46, rot, 1.0);
    let ri = r.ceil() as i32 + 1;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (x, y) = (c.0.round() as i32 + dx, c.1.round() as i32 + dy);
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if !inside(&poly, fx, fy) || bayer(x, y) < fade {
                continue;
            }
            let d = (fx - c.0).hypot(fy - c.1) / r;
            // The edge: a pixel with open paper beside it, in the star's shape.
            let edge = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].iter().any(|&(ox, oy)| !inside(&poly, fx + ox, fy + oy));
            let k = if edge { 0.45 + d * 0.4 } else { d * 0.55 };
            pen.put(x, y, ramp.at(k));
        }
    }
}

/// The outline of a five-pointed star, drawn on as `open` goes 0 to 1.
fn star_lines(pen: &mut Pen, c: (f32, f32), r: f32, rot: f32, squash: f32, open: f32, ramp: Ramp, fade: f32) {
    let pts = points(c, r, 0.42, rot, squash);
    let n = (open.clamp(0.0, 1.0) * 10.0).ceil() as usize;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % 10]);
        let part = if i + 1 == n { (open * 10.0 - i as f32).clamp(0.0, 1.0) } else { 1.0 };
        let b = (a.0 + (b.0 - a.0) * part, a.1 + (b.1 - a.1) * part);
        let len = (b.0 - a.0).hypot(b.1 - a.1).ceil().max(1.0) as i32;
        for s in 0..=len {
            let t = s as f32 / len as f32;
            let (x, y) = ((a.0 + (b.0 - a.0) * t).round() as i32, (a.1 + (b.1 - a.1) * t).round() as i32);
            if bayer(x, y) < fade {
                continue;
            }
            // Brightest at the points, cooler toward the hollows.
            let k = if i % 2 == 0 { t } else { 1.0 - t };
            pen.light(x, y, ramp.at(0.05 + k * 0.5 + fade * 0.4));
        }
    }
}

/// The shard of starlight where the star came down: a five-pointed star
/// hovering and breathing over its pool, a glint running over it, little
/// stars coming and going about it and motes going up; fading at its end.
fn shard(pen: &mut Pen, star: &Star, life: f32, tick: u64) {
    let t = tick as f32;
    let fade = shard_fade(life);
    let bob = (t * 0.09).sin() * 1.5;
    let c = (star.land.0 * UNIT, star.land.1 * UNIT - 9.0 + bob);
    let breath = 0.5 + 0.5 * (t * 0.11).sin();
    let rot = (t * 0.02).sin() * 0.22;
    // The star first, then its aura about it: violet against its gold, so
    // its five points read.
    solid_star(pen, c, 7.5 + breath * 0.8, rot, STARGOLD, fade);
    pen.glow(c, 15.0 + 3.0 * breath, 1.0, TAIL, (0.45 + 0.25 * breath) * (1.0 - fade));
    // A glint running over it now and then.
    let g = (tick % 70) as f32;
    if g < 10.0 && fade < 0.5 {
        let k = g / 10.0;
        pen.twinkle((c.0 - 4.0 + k * 8.0, c.1 - 3.0 + k * 4.0), 4.0, k, CORE);
    }
    pen.light(c.0.round() as i32, c.1.round() as i32, 'w');
    // Little stars about it, each on a loop of its own.
    for i in 0..6u32 {
        let life_i = 34.0 + (i % 3) as f32 * 6.0;
        let start = (hash(i as i32, 2, 0x5ad) % 1000) as f32 / 1000.0 * life_i;
        let k = ((t + start) % life_i) / life_i;
        let round = ((t + start) / life_i) as i32;
        let a = (hash(i as i32 + round * 11, 3, 0x5ae) % 628) as f32 / 100.0;
        let d = 9.0 + (hash(i as i32 + round * 11, 4, 0x5af) % 7) as f32;
        let at = (c.0 + a.cos() * d, c.1 + a.sin() * d * 0.7);
        if bayer(at.0 as i32, at.1 as i32) >= fade {
            pen.twinkle(at, 1.6 + (i % 2) as f32, k, if i % 2 == 0 { CORE } else { BLUSH });
        }
    }
    // Motes going up off it, slowly.
    for i in 0..10u32 {
        let life_i = 50.0;
        let start = (hash(i as i32, 5, 0x5b0) % 1000) as f32 / 1000.0 * life_i;
        let k = ((t + start) % life_i) / life_i;
        let round = ((t + start) / life_i) as i32;
        let x = c.0 - 7.0 + (hash(i as i32 + round * 7, 6, 0x5b1) % 15) as f32;
        let at = (x + (k * 6.0 + i as f32).sin() * 2.0, c.1 + 4.0 - k * 26.0);
        if bayer(at.0 as i32, at.1 as i32) < fade.max(k * 0.6) {
            continue;
        }
        let m = pfx::Mote { i, at, vel: (0.0, -0.5), k, seed: i };
        pfx::mote(pen, &m, if i % 4 == 0 { Shape::Star } else { Shape::Dot }, if i % 3 == 0 { GILT } else { BLUSH }, 1, tick);
    }
}

/// The shard taken: it flares, its light leaps in an arc to the knight
/// and bursts over their head in stars.
fn caught(pen: &mut Pen, run: &Run, star: &Star, hero: &crate::rules::Hero, since: f32, tick: u64) {
    if since >= sf::CAUGHT_TICKS as f32 {
        return;
    }
    let from = (star.land.0 * UNIT, star.land.1 * UNIT - 9.0);
    let (_, mid) = wishfx::body(run, hero);
    let to = (mid.0, mid.1 - 10.0);
    // The flare where it lay.
    if since < 12.0 {
        let k = since / 12.0;
        pen.glow(from, 16.0 * (1.0 - k) + 3.0, 1.0, CORE, 1.0 - k * 0.6);
        pfx::rays(pen, from, 10, 22.0, 0.9, 0.3, CORE, k);
    }
    // It rises off the floor and goes once round the knight, closing in,
    // and into them: a ribbon of its light behind it.
    const LEAP: f32 = 18.0;
    let along = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        let a = PI * 0.5 + t * TAU * 1.15;
        let r = 30.0 * (1.0 - ease::inward(t)) + 2.0;
        let base = (from.0 + (to.0 - from.0) * ease::out(t), from.1 + (to.1 - from.1) * ease::out(t));
        (base.0 + a.cos() * r, base.1 + a.sin() * r * 0.5 - 10.0 * (t * PI).sin())
    };
    if since < LEAP + 2.0 {
        let mut pts = Vec::with_capacity(48);
        let mut last = along(since / LEAP);
        pts.push(last);
        let mut b = since;
        while b > 0.0 && pts.len() < 48 {
            b -= 0.25;
            let p = along(b.max(0.0) / LEAP);
            if (p.0 - last.0).hypot(p.1 - last.1) >= 1.0 {
                pts.push(p);
                last = p;
            }
        }
        tail(pen, &pts, 3.4);
        let head = pts[0];
        pen.glow(head, 4.0, 1.0, CORE, 1.0);
        pen.glow(head, 9.0, 1.0, GILT, 0.9);
        solid_star(pen, head, 4.5, since * 0.35, STARGOLD, 0.0);
        // Sparks shed off it as it goes round.
        for (j, &p) in pts.iter().enumerate().skip(4).step_by(7) {
            let h = hash(j as i32, since as i32 / 3, 0xca7d);
            pen.twinkle((p.0 + (h % 5) as f32 - 2.0, p.1 + 2.0 + ((h >> 3) % 4) as f32), 2.0, (j as f32 / pts.len() as f32).min(0.95), if h % 2 == 0 { GILT } else { BLUSH });
        }
    }
    // Over the knight: a burst of stars, a twinkle, light raining off.
    let burst = since - LEAP;
    if burst >= 0.0 {
        if burst < 16.0 {
            pen.glow(to, 6.0, 1.0, CORE, 1.0 - burst / 16.0);
            pen.glow(to, 20.0 * (1.0 - burst / 16.0) + 4.0, 1.0, GILT, 0.95 - burst / 20.0);
            pen.twinkle(to, 7.0, (burst / 16.0).min(0.99), CORE);
            pfx::rays(pen, to, 10, 20.0, 0.9, 0.2, GILT, burst / 16.0);
        }
        let spray = Spray {
            gravity: 0.07,
            angle: -PI / 2.0,
            spread: 5.0,
            drag: 0.08,
            ..Spray::round(star.seed ^ 0xca7c, if pen.calm { 8 } else { 16 }, to, (1.0, 3.0), (16.0, 30.0))
        };
        spray.each(burst, |m| pfx::mote(pen, &m, Shape::Star, if m.i % 2 == 0 { GILT } else { BLUSH }, 1, tick));
    }
}

/// Ticks a star wish's little star takes to fall onto its knight.
const FALLS: f32 = 14.0;

/// A star wish landing on its knight: a little star falls out of the dark
/// onto them, and a five-pointed star of gold opens about them, turning,
/// light thrown off its points and a shower of stars.
fn granted(pen: &mut Pen, run: &Run, hero: &crate::rules::Hero, view: (i32, i32), age: f32, id: u32, tick: u64) {
    if age >= sf::GRANT_TICKS as f32 {
        return;
    }
    let (_, mid) = wishfx::body(run, hero);
    let c = (mid.0, mid.1 - 2.0);
    if age < FALLS {
        // It falls: from high over them in the dark, a streak to their head.
        let top = (c.0 + 34.0 * if id % 2 == 0 { 1.0 } else { -1.0 }, view.1 as f32 + 6.0);
        let at = |a: f32| {
            let t = ease::inward((a / FALLS).clamp(0.0, 1.0));
            (top.0 + (c.0 - top.0) * t, top.1 + (c.1 - 10.0 - top.1) * t)
        };
        let mut pts = Vec::with_capacity(40);
        let mut last = at(age);
        pts.push(last);
        let mut b = age;
        while b > -6.0 && pts.len() < 40 {
            b -= 0.3;
            let p = at(b.max(0.0));
            if (p.0 - last.0).hypot(p.1 - last.1) >= 1.0 {
                pts.push(p);
                last = p;
            }
            if b <= 0.0 {
                break;
            }
        }
        tail(pen, &pts, 2.6);
        pen.glow(pts[0], 4.0, 1.0, CORE, 1.0);
        pen.glow(pts[0], 8.0, 1.0, GILT, 0.9);
        pen.twinkle(pts[0], 4.0, 0.5, CORE);
        return;
    }
    let since = age - FALLS;
    let open = ease::back((since / 10.0).min(1.0));
    let fade = ((since - 26.0) / 34.0).clamp(0.0, 1.0);
    let rot = since * 0.035;
    if since < 14.0 {
        pen.glow(c, 18.0 * (1.0 - since / 14.0) + 4.0, 1.0, CORE, 1.0 - since / 16.0);
    }
    pen.glow(c, 12.0, 1.0, TAIL.cooler(1), 0.5 * (1.0 - fade));
    star_lines(pen, c, 22.0 * open, rot, 1.0, open, GILT, fade);
    star_lines(pen, c, 15.0 * open, -rot * 1.4 + PI / 5.0, 1.0, open, CORE, (fade + 0.2).min(1.0));
    // Light thrown off its five points.
    for p in points(c, 22.0 * open, 0.42, rot, 1.0).iter().step_by(2) {
        pen.twinkle(*p, 3.0, ((since - 4.0) / 24.0).clamp(0.0, 0.99), CORE);
    }
    // Its rays go out along its points, the short ones between them.
    pfx::rays(pen, c, 10, 30.0, 1.0, rot - PI / 2.0, GILT, (since / 20.0).min(1.0));
    // A shower of stars, thrown up off them and falling all round.
    let spray = Spray {
        gravity: 0.08,
        angle: -PI / 2.0,
        spread: 4.6,
        drag: 0.05,
        ..Spray::round(id ^ 0x57a2 ^ tick.saturating_sub(age as u64) as u32, if pen.calm { 10 } else { 22 }, (c.0, c.1 - 6.0), (1.2, 3.4), (20.0, 36.0))
    };
    spray.each(since, |m| {
        let (shape, ramp) = match m.i % 3 {
            0 => (Shape::Star, GILT),
            1 => (Shape::Star, BLUSH),
            _ => (Shape::Dot, TAIL),
        };
        pfx::mote(pen, &m, shape, ramp, 1, tick);
    });
}

/// The banner over the top of the view while the star is up: "A SHOOTING
/// STAR!", and the knight's own key under it while they hold its wish.
fn banner(cv: &mut Img, run: &Run, star: &Star, view: (i32, i32), focus: Option<u32>) {
    let age = star.age(run.tick);
    if !(8..sf::BANNER).contains(&age) {
        return;
    }
    let head = "A SHOOTING STAR!";
    let holds = run.players.get(&focus.unwrap_or(1)).is_some_and(|h| h.star_wish);
    let line = if holds { "MAKE A WISH  T" } else { "MAKE A WISH" };
    let w = text_width(head).max(text_width(line)) + 18;
    let h = 23;
    // In from above, out the same way.
    let k_in = ease::back(((age - 8) as f32 / 12.0).min(1.0));
    let k_out = ease::inward(((age as f32 - (sf::BANNER as f32 - 12.0)) / 12.0).clamp(0.0, 1.0));
    let drop = (h as f32 + 8.0) * (1.0 - k_in + k_out);
    // Under the room's top wall, clear of its signs and its way-on mark.
    let x = view.0 + (NATIVE_W - w) / 2;
    let y = view.1 + 27 - drop.round() as i32;
    chrome::panel(cv, (x, y, w, h), Tone::FRAMED.rimmed(chrome::rim_of(']')));
    let shine = (run.tick / 4) % 2 == 0;
    cv.text(x + (w - text_width(head)) / 2, y + 3, head, if shine { 'w' } else { '9' });
    cv.text(x + (w - text_width(line)) / 2, y + 12, line, '^');
    if holds {
        // The key in gold, as every key is.
        let kx = x + (w - text_width(line)) / 2 + text_width("MAKE A WISH  ");
        cv.text(kx, y + 12, "T", '6');
    }
    let mut pen = Pen::new(cv, 120);
    for (i, (dx, dy)) in [(-4, 4), (w + 3, 6), (-2, h - 4), (w + 2, h - 6)].into_iter().enumerate() {
        let phase = ((run.tick + i as u64 * 9) % 30) as f32 / 30.0;
        pen.twinkle(((x + dx) as f32, (y + dy) as f32), 2.0, phase, if i % 2 == 0 { CORE } else { GILT });
    }
}

/// "THE STAR IS YOURS!" over the knight who took it, a moment.
fn yours(cv: &mut Img, run: &Run, hero: &crate::rules::Hero, view: (i32, i32), since: u64) {
    if !(10..sf::CAUGHT_TICKS + 20).contains(&since) {
        return;
    }
    let (_, mid) = wishfx::body(run, hero);
    let head = "THE STAR IS YOURS!";
    let line = "+1 WISH";
    let w = text_width(head) + 8;
    let h = 21;
    let rise = (ease::out(((since - 10) as f32 / 14.0).min(1.0)) * 6.0) as i32;
    let x = super::in_view_x(mid.0 as i32, w, view, cv.w);
    let y = (mid.1 as i32 - 44 - rise).max(view.1 + 2);
    chrome::panel(cv, (x, y, w, h), Tone::FRAMED.rimmed(chrome::rim_of(']')));
    cv.text(x + (w - text_width(head)) / 2, y + 3, head, if (run.tick / 4) % 2 == 0 { 'w' } else { '9' });
    cv.text(x + (w - text_width(line)) / 2, y + 11, line, '6');
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__starfall__shots.rs"]
mod shots;
