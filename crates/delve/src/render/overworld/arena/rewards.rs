//! The steps up as a seat sees them (`rules::rewards`): every moment a
//! knight gets stronger lands where it can be seen, on the shared clock
//! (`crate::motion`, docs/MOTION.md), and goes.
//!
//! - **A level**: gold gathers at the knight's feet, a pillar of light
//!   throws up out of them, chevrons climb either side, and LEVEL with its
//!   number slammed down rises over their head.
//! - **A tier**: the wish's old numeral over the knight flips to the new
//!   one (twice for a super wish), sparks off the flip, and rays behind it
//!   from the third tier up, in the tier's own light.
//! - **A rung**: four corners close in on the knight like a lens finding
//!   them, a bright line sweeps them foot to crown, and the ladder's pips
//!   beside them fill to the new rung, its pixels named over them.
//! - **A relic**: light comes straight down onto the knight, a star flares
//!   on them with a ring of sparks, and RELIC and its name rise.
//! - **The party's**: a floor deeper than the realm has been (DEEPEST YET,
//!   the floor slammed on), a constellation of the realm's sky drawn star by
//!   star and lit (`sky`), and the sky's gift, a star falling to each knight.
//!
//! The light at a knight's feet is drawn under the figures (`under`), the
//! rest over them (`over`), the party's banners over the view (`screen`).
//! Every frame is the run and the tick: nothing is kept between frames.

use super::super::chrome::{self, Tone};
use super::super::ink::{Img, bayer, text_width};
use super::pfx::{Pen, Ramp, Spray, ramps, unit};
use super::*;
use crate::motion::{Ease, ease, key, share, track};
use crate::rules::Hero;
use crate::rules::rewards::{self as rules_rewards, Kind, Step, constellation, numeral};
use std::f32::consts::TAU;

/// The most motes the moments paint in a pass.
const BUDGET: u32 = 1400;

/// A knight's step and where they stand: their feet and their middle, in
/// room pixels.
fn knights_steps(run: &Run) -> impl Iterator<Item = (&Step, f32, &Hero, ((f32, f32), (f32, f32)))> {
    run.rewards.playing(run.tick).filter(|(s, _)| !s.kind.banner()).filter_map(move |(s, age)| {
        let hero = run.players.get(&s.knight).filter(|h| !h.absent && h.privy == 0)?;
        Some((s, age as f32, hero, super::wishfx::body(run, hero)))
    })
}

/// Under the figures: what lights the floor at a knight's feet.
pub(super) fn under(cv: &mut Img, run: &Run) {
    let mut pen = Pen::new(cv, BUDGET / 2);
    for (s, age, _, (feet, _)) in knights_steps(run) {
        match &s.kind {
            Kind::Level { .. } => level_floor(&mut pen, feet, age, s.knight),
            Kind::Rung { .. } => {
                let open = ease::out(share(age, 12.0, 10.0));
                let fade = share(age, 40.0, 50.0);
                pen.ring((feet.0, feet.1 + 1.0), 3.0 + 9.0 * open, 0.9, 0.42, ramps::FROST, fade.max(1.0 - open.min(1.0) * 2.0).min(1.0));
            }
            Kind::Relic { .. } => {
                let pool = 1.0 - ease::inward(share(age, 6.0, 40.0));
                pen.glow((feet.0, feet.1 + 1.0), 11.0, 0.4, ramps::HOLY, 0.9 * pool * share(age, 0.0, 3.0));
            }
            Kind::Tier { .. } | Kind::Record { .. } | Kind::Lit { .. } | Kind::Gift { .. } => {}
        }
    }
}

/// Over the figures: what rises from and stands over a knight.
pub(super) fn over(cv: &mut Img, run: &Run) {
    // A tier's numeral goes over the rest, so it is never under a pillar.
    let mut all: Vec<(Step, f32, (f32, f32), (f32, f32))> =
        knights_steps(run).map(|(s, age, _, (feet, mid))| (s.clone(), age, feet, mid)).collect();
    all.sort_by_key(|(s, ..)| matches!(s.kind, Kind::Tier { .. }));
    for (s, age, feet, mid) in all {
        match s.kind {
            Kind::Level { level } => level_over(cv, feet, mid, age, level, s.knight),
            Kind::Tier { from, tier, ref wish } => tier_over(cv, feet, age, from, tier, wish, s.knight),
            Kind::Rung { px } => rung_over(cv, feet, age, px),
            Kind::Relic { ref name } => relic_over(cv, feet, mid, age, name, s.knight),
            _ => {}
        }
    }
}

/// Over the view, after the camera: the party's banners.
pub(super) fn screen(cv: &mut Img, run: &Run, origin: (i32, i32)) {
    for (s, age) in run.rewards.playing(run.tick).filter(|(s, _)| s.kind.banner()) {
        let age = age as f32;
        match &s.kind {
            Kind::Record { floor } => record(cv, run, age, *floor),
            Kind::Lit { index, by } => lit(cv, run, age, *index, by),
            Kind::Gift { wishes } => gift(cv, run, origin, age, *wishes),
            _ => {}
        }
    }
}

// ─── letters ─────────────────────────────────────────────────────────────────

/// `text` in `ink`, `k` times the font's size.
fn letters(text: &str, ink: char, k: i32) -> Img {
    let mut small = Img::new(text_width(text).max(1), 7);
    small.text(0, 0, text, ink);
    if k <= 1 {
        return small;
    }
    let mut out = Img::new(small.w * k, small.h * k);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = small.get(x / k, y / k) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// A tier's numeral (from 0) in strokes of its own, seven pixels high: the
/// font's I has feet that run together into I:I:I and its V is round, so
/// the ones and fives here share one beam over and under, and the five is
/// sharp.
fn roman(tier: u8, ink: char) -> Img {
    const V: [&str; 7] = ["#...#", "#...#", ".#.#.", ".#.#.", ".#.#.", "..#..", "..#.."];
    let ones = |n: i32| -> Img {
        let w = 3 * n;
        let mut im = Img::new(w, 7);
        for x in 0..w {
            im.put(x, 0, ink);
            im.put(x, 6, ink);
        }
        for k in 0..n {
            for y in 1..6 {
                im.put(1 + 3 * k, y, ink);
            }
        }
        im
    };
    let five = || {
        let mut im = Img::new(5, 7);
        for (y, row) in V.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '#' {
                    im.put(x as i32, y as i32, ink);
                }
            }
        }
        im
    };
    match tier {
        0..=2 => ones(i32::from(tier) + 1),
        3 => beside(&ones(1), &five(), 1),
        _ => five(),
    }
}

/// `im` at `k` times its size.
fn bigger(im: &Img, k: i32) -> Img {
    let mut out = Img::new(im.w * k, im.h * k);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = im.get(x / k, y / k) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// A wish's name in at most `room` letters: whole when it fits, else its
/// last words that do, else cut.
pub(super) fn short_name(name: &str, room: usize) -> String {
    let name = name.to_uppercase();
    if name.chars().count() <= room {
        return name;
    }
    let words: Vec<&str> = name.split_whitespace().collect();
    for from in 1..words.len() {
        let tail = words[from..].join(" ");
        if tail.chars().count() <= room {
            return tail;
        }
    }
    name.chars().take(room).collect()
}

/// `im` lit from above: its top rows in `hi`, the rest in `lo`, a line of
/// `glint` across its top, so a numeral reads as cast metal.
fn cast(im: &Img, hi: char, lo: char, glint: char) -> Img {
    let mut out = Img::new(im.w, im.h);
    let top = (0..im.h).find(|&y| (0..im.w).any(|x| im.get(x, y).is_some())).unwrap_or(0);
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some() {
                let first = im.get(x, y - 1).is_none() && y <= top + 1;
                out.put(x, y, if first { glint } else if y < im.h / 2 { hi } else { lo });
            }
        }
    }
    out
}

/// `im` ringed in a pixel of `edge`, so it reads over any floor.
fn outlined(im: &Img, edge: char) -> Img {
    let mut out = Img::new(im.w + 2, im.h + 2);
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some() {
                for (dx, dy) in [(0, 1), (2, 1), (1, 0), (1, 2), (0, 0), (2, 2), (0, 2), (2, 0)] {
                    out.put(x + dx, y + dy, edge);
                }
            }
        }
    }
    out.stamp(im, 1, 1);
    out
}

/// `im` squashed to `s` of its height (0 to past 1) about its middle row.
fn squashed(im: &Img, s: f32) -> Img {
    let h = ((im.h as f32 * s).round() as i32).max(1);
    let mut out = Img::new(im.w, h);
    for y in 0..h {
        let sy = ((y as f32 + 0.5) / s).floor() as i32;
        for x in 0..im.w {
            if let Some(c) = im.get(x, sy.min(im.h - 1)) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// Two images side by side on one foot, `gap` apart.
fn beside(a: &Img, b: &Img, gap: i32) -> Img {
    let h = a.h.max(b.h);
    let mut out = Img::new(a.w + gap + b.w, h);
    out.stamp(a, 0, h - a.h);
    out.stamp(b, a.w + gap, h - b.h);
    out
}

/// One image over another, centred, `gap` apart.
fn over_under(a: &Img, b: &Img, gap: i32) -> Img {
    let w = a.w.max(b.w);
    let mut out = Img::new(w, a.h + gap + b.h);
    out.stamp(a, (w - a.w) / 2, 0);
    out.stamp(b, (w - b.w) / 2, a.h + gap);
    out
}

/// A label `im` over a knight: its foot `lift` pixels over their feet,
/// centred on them, popping up into place at `age` and lifting away at the
/// end of `life` (`notices::board_from`).
fn over_head(cv: &mut Img, im: &Img, feet: (f32, f32), lift: f32, age: f32, life: f32) {
    let x = (feet.0 - im.w as f32 / 2.0).round() as i32;
    let y = (feet.1 - lift).round() as i32 - im.h;
    super::notices::board_from(cv, im, (x, y), age, Some(life), 6.0);
}

// ─── a level ─────────────────────────────────────────────────────────────────

/// A level's beats: the gold drawn in, the throw of the pillar, its
/// follow-through as it thins, and the number standing to be read.
const LEVEL: crate::motion::Beats = crate::motion::Beats::new(9.0, 5.0, 22.0, 54.0);

fn level_floor(pen: &mut Pen, feet: (f32, f32), age: f32, seat: u32) {
    // Gold drawn in to the feet.
    if age < LEVEL.action_at() {
        let t = share(age, 0.0, LEVEL.action_at());
        for m in 0..10 {
            let a = m as f32 / 10.0 * TAU + seat as f32;
            let r = 16.0 * (1.0 - ease::inward(t)) + 1.0;
            let at = (feet.0 + a.cos() * r, feet.1 + a.sin() * r * 0.45);
            pen.light(at.0.round() as i32, at.1.round() as i32, if m % 2 == 0 { '6' } else { '5' });
            pen.light((at.0 + a.cos()).round() as i32, (at.1 + a.sin() * 0.45).round() as i32, 'O');
        }
        return;
    }
    // Thrown out: a ring past its size and an echo a beat behind it.
    let since = LEVEL.since_action(age);
    for (lag, ramp) in [(0.0, ramps::HOLY), (4.0, ramps::GOLD)] {
        let t = share(since - lag, 0.0, 16.0);
        if t <= 0.0 || t >= 1.0 {
            continue;
        }
        pen.ring((feet.0, feet.1 + 1.0), 3.0 + 15.0 * ease::out(t), 1.1 - 0.5 * t, 0.42, ramp, ease::in_quad(t));
    }
    pen.glow((feet.0, feet.1), 9.0, 0.45, ramps::HOLY, 1.0 - ease::inward(share(since, 0.0, 30.0)));
}

fn level_over(cv: &mut Img, feet: (f32, f32), mid: (f32, f32), age: f32, level: u32, seat: u32) {
    let since = LEVEL.since_action(age);
    {
        let mut pen = Pen::new(cv, BUDGET / 2);
        // The pillar: thrown up out of the feet, wide and white, thinning
        // through its follow-through.
        if (0.0..LEVEL.follow + LEVEL.action).contains(&since) {
            let reach = 52.0 * ease::out(share(since, 0.0, LEVEL.action));
            let thin = ease::inward(share(since, LEVEL.action, LEVEL.follow));
            let half = 4.0 * (1.0 - thin) + 0.6;
            for y in (feet.1 - reach).floor() as i32..=feet.1.ceil() as i32 {
                // Brightest at the foot, failing toward the top.
                let up = (feet.1 - y as f32) / reach.max(1.0);
                for x in (feet.0 - half).floor() as i32..=(feet.0 + half).ceil() as i32 {
                    let d = ((x as f32 + 0.5 - feet.0).abs() / half).min(1.0);
                    let i = (1.0 - d) * (1.0 - up * 0.7) * (1.0 - thin * 0.8);
                    if i < 0.12 || bayer(x, y) > i * 1.6 {
                        continue;
                    }
                    // The halo goes behind the knight; only the hot core
                    // burns over them, and only at the throw.
                    if pen.has(x, y) && (i < 0.55 || since > LEVEL.action + 2.0) {
                        continue;
                    }
                    pen.light(x, y, ramps::HOLY.at(1.0 - i));
                }
            }
        }
        // Chevrons climbing either side, a beat apart.
        for c in 0..3 {
            let t = share(since, 2.0 + c as f32 * 7.0, 26.0);
            if t <= 0.0 || t >= 1.0 {
                continue;
            }
            let y = feet.1 - 4.0 - 30.0 * ease::out(t);
            for side in [-1.0f32, 1.0] {
                let x = feet.0 + side * (9.0 + 2.0 * (c % 2) as f32);
                let ink = if t < 0.6 { '6' } else { '4' };
                if bayer(x as i32, y as i32) < t * t {
                    continue;
                }
                for d in -2i32..=2 {
                    pen.light(x.round() as i32 + d, y.round() as i32 + d.abs(), ink);
                }
            }
        }
        // Twinkles about the number once it stands.
        for m in 0..5 {
            let t = share(age, 18.0 + m as f32 * 6.0, 14.0);
            let at = (mid.0 + (unit(m, seat as i32, 51) - 0.5) * 40.0, feet.1 - 30.0 - unit(m, seat as i32, 52) * 14.0);
            pen.twinkle(at, 2.0, t, ramps::GOLD);
        }
    }
    // LEVEL and its number, slammed down: four times the font, then three,
    // then the twice it rests at.
    let at = LEVEL.action_at() + 3.0;
    if age >= at {
        let d = age - at;
        let k = if d < 1.0 { 4 } else if d < 2.0 { 3 } else { 2 };
        let number = outlined(&cast(&letters(&level.to_string(), '5', k), '6', '4', '9'), 'k');
        let word = outlined(&letters("LEVEL", 'T', 1), 'k');
        let plate = beside(&word, &number, 2);
        over_head(cv, &plate, feet, 24.0, d, LEVEL.total() - at);
    }
}

// ─── a tier ──────────────────────────────────────────────────────────────────

/// A tier's light: the genie's teal for the first two, gold for the third
/// and fourth, violet for the fifth; and its numeral's inks.
fn tier_light(tier: u8) -> (Ramp, (char, char, char)) {
    match tier {
        0 | 1 => (ramps::GENIE, ('3', '2', 'w')),
        2 | 3 => (ramps::GOLD, ('6', '4', '9')),
        _ => (ramps::ARCANE, ('^', ']', 'w')),
    }
}

/// Ticks between one flip of the numeral and the next.
const FLIP: f32 = 7.0;
/// When the first flip begins.
const FIRST_FLIP: f32 = 7.0;

fn tier_over(cv: &mut Img, feet: (f32, f32), age: f32, from: u8, tier: u8, wish: &str, seat: u32) {
    let life = 60.0;
    let flips = u32::from(tier.saturating_sub(from)).max(1);
    let done = FIRST_FLIP + flips as f32 * FLIP;
    // Which numeral stands now, and how far through its flip.
    let n = (((age - FIRST_FLIP) / FLIP).floor().max(-1.0) + 1.0).min(flips as f32) as u32;
    let showing = from + n as u8;
    let into = age - FIRST_FLIP - (n as f32 - 1.0) * FLIP;
    let (ramp, (hi, lo, glint)) = tier_light(showing);
    let s = if n == 0 {
        // Before the first flip it closes up into it.
        1.0 - ease::inward(share(age, FIRST_FLIP - 3.0, 3.0))
    } else {
        ease::back(share(into, 0.0, 5.0))
    };
    let foot = 26.0;
    let center = (feet.0, feet.1 - foot - 7.0);
    {
        let mut pen = Pen::new(cv, BUDGET / 2);
        // Rays behind it from the third tier on, turning; more at the fifth.
        if tier >= 2 && age >= FIRST_FLIP {
            let (rays, reach) = if tier >= 4 { (12, 16.0) } else { (8, 9.0) };
            let power = share(age, FIRST_FLIP, 6.0) * (1.0 - ease::inward(share(age, done + 18.0, 20.0)));
            let inner = 3.0 + 3.0 * i32::from(tier.min(3)) as f32 + 2.0;
            for r in 0..rays {
                let a = r as f32 / rays as f32 * TAU + age * 0.04;
                let len = reach + 5.0 * ((r % 2) as f32);
                for j in 0..(len * power) as i32 {
                    let d = inner + j as f32;
                    let (x, y) = (center.0 + a.cos() * d, center.1 + a.sin() * d * 0.8);
                    if bayer(x as i32, y as i32) < j as f32 / (len + 4.0) {
                        continue;
                    }
                    pen.light(x.round() as i32, y.round() as i32, ramp.at(0.15 + j as f32 / (len * 1.6)));
                }
            }
            if tier >= 4 {
                let t = share(age, FIRST_FLIP + flips as f32 * FLIP - 2.0, 18.0);
                if t > 0.0 && t < 1.0 {
                    pen.ring(center, 6.0 + 18.0 * ease::out(t), 1.0, 0.8, ramp, ease::in_quad(t));
                }
            }
        }
        // Sparks off each flip.
        for f in 1..=flips {
            let at = FIRST_FLIP + (f as f32 - 1.0) * FLIP;
            let spray = Spray { gravity: 0.08, ..Spray::round(seat * 31 + f, 12, center, (1.4, 2.8), (10.0, 18.0)) };
            spray.each(age - at, |m| pen.streak(m.at, m.vel, 2.0, ramp, m.k));
        }
    }
    if age < 2.0 {
        return;
    }
    let numeral = outlined(&cast(&bigger(&roman(showing, hi), 2), hi, lo, glint), 'k');
    let numeral = squashed(&numeral, s.max(0.05));
    let name = short_name(wish, 18);
    let words = outlined(&letters(&name, 'i', 1), 'k');
    let x = (feet.0 - numeral.w as f32 / 2.0).round() as i32;
    let y = (center.1 - numeral.h as f32 / 2.0).round() as i32;
    let exit = share(age, life - 8.0, 8.0);
    let lift = (-7.0 * ease::inward(exit)).round() as i32;
    let pop = if age < 6.0 { (6.0 * (1.0 - ease::back(share(age, 2.0, 4.0)))).round() as i32 } else { 0 };
    for yy in 0..numeral.h {
        for xx in 0..numeral.w {
            if let Some(c) = numeral.get(xx, yy)
                && bayer(x + xx, y + yy) >= ease::in_quad(exit)
            {
                cv.set(x + xx, y + yy + lift + pop, c);
            }
        }
    }
    if age >= done {
        over_head(cv, &words, feet, foot - 9.0, age - done, life - done);
    }
}

// ─── a rung ──────────────────────────────────────────────────────────────────

/// The figure's box a lens closes on: half its width and its height.
const LENS: (f32, f32) = (8.0, 18.0);

fn rung_over(cv: &mut Img, feet: (f32, f32), age: f32, px: u32) {
    let life = 105.0;
    let mut pen = Pen::new(cv, BUDGET / 2);
    // The corners close in, a little past, and settle; at the end they
    // open out again and go.
    let close = track(&[key(0.0, 2.6, Ease::Linear), key(12.0, 0.92, Ease::Out), key(17.0, 1.0, Ease::Both), key(88.0, 1.0, Ease::Linear), key(104.0, 2.2, Ease::In)], age);
    let gone = ease::in_quad(share(age, 90.0, 14.0));
    let (hw, top) = (LENS.0 * close + 2.0, LENS.1 * (0.5 + 0.5 * close));
    let (l, r, t, b) = (feet.0 - hw, feet.0 + hw, feet.1 - top - 2.0, feet.1 + 3.0);
    let locked = (12.0..15.0).contains(&age);
    let ink = if locked { 'w' } else { 'W' };
    for (cx, cy, sx, sy) in [(l, t, 1.0, 1.0), (r, t, -1.0, 1.0), (l, b, 1.0, -1.0), (r, b, -1.0, -1.0)] {
        for k in 0..5 {
            for (x, y) in [(cx + sx * k as f32, cy), (cx, cy + sy * k as f32)] {
                let (x, y) = (x.round() as i32, y.round() as i32);
                if bayer(x, y) >= gone {
                    pen.light(x, y, if k == 0 { 'w' } else { ink });
                    pen.under(x + 1, y + 1, 'q');
                }
            }
        }
    }
    // The line sweeping foot to crown, the figure glinting fine behind it.
    let sweep = share(age, 16.0, 14.0);
    if sweep > 0.0 && sweep < 1.0 {
        let y = (b - (b - t) * ease::both(sweep)).round() as i32;
        for x in (l + 1.0) as i32..r as i32 {
            pen.light(x, y, 'w');
            pen.light(x, y + 1, '3');
            if x % 2 == 0 {
                pen.light(x, y - 1, '3');
            }
        }
        for yy in y + 2..(b as i32).min(y + 8) {
            for x in (l + 1.0) as i32..r as i32 {
                if (x + yy) % 2 == 0 && bayer(x, yy) < 0.18 - (yy - y) as f32 * 0.02 {
                    pen.light(x, yy, 'W');
                }
            }
        }
    }
    // The ladder's pips beside the figure, the new rung filling.
    let pips = share(age, 26.0, 4.0);
    if pips > 0.0 && gone < 1.0 {
        let x0 = (r + 4.0).round() as i32;
        let reached = rules_rewards::RUNG_PIXELS.iter().position(|&p| p == px).unwrap_or(0);
        for (i, _) in rules_rewards::RUNG_PIXELS.iter().enumerate() {
            let y = b as i32 - 2 - i as i32 * 4;
            let fill = if i < reached {
                'z'
            } else if i == reached {
                if age < 32.0 { 'w' } else if age < 36.0 { '3' } else { '2' }
            } else {
                'x'
            };
            let w = (5.0 * ease::out(pips)).round() as i32;
            for dx in 0..w {
                for dy in 0..2 {
                    if bayer(x0 + dx, y + dy) >= gone {
                        pen.put(x0 + dx, y + dy, fill);
                    }
                }
            }
        }
    }
    drop(pen);
    // Its pixels named over the figure, a little after the lens finds it.
    let at = 30.0;
    if age >= at {
        let number = outlined(&cast(&letters(&px.to_string(), 'W', 2), 'w', 'V', 'w'), 'k');
        let unit = outlined(&letters("PX", 'V', 1), 'k');
        let caption = outlined(&letters("FINER", '3', 1), 'k');
        let plate = over_under(&caption, &beside(&number, &unit, 1), 0);
        over_head(cv, &plate, feet, LENS.1 + 6.0, age - at, life - at - 6.0);
    }
}

// ─── a relic ─────────────────────────────────────────────────────────────────

fn relic_over(cv: &mut Img, feet: (f32, f32), mid: (f32, f32), age: f32, name: &str, seat: u32) {
    let life = 75.0;
    {
        let mut pen = Pen::new(cv, BUDGET / 2);
        // Light straight down onto them, narrowing as it goes.
        let ray = 1.0 - ease::inward(share(age, 3.0, 18.0));
        if ray > 0.0 {
            let half = 2.0 + 4.0 * ray;
            let reach = 90.0 * ease::out(share(age, 0.0, 3.0));
            for y in (feet.1 - reach) as i32..feet.1 as i32 {
                for x in (feet.0 - half).floor() as i32..=(feet.0 + half).ceil() as i32 {
                    let d = ((x as f32 + 0.5 - feet.0).abs() / half).min(1.0);
                    let i = (1.0 - d) * ray;
                    if i < 0.15 || bayer(x, y) > i * 1.4 {
                        continue;
                    }
                    pen.light(x, y, ramps::HOLY.at(1.0 - i));
                }
            }
        }
        // A star flares on them: four arms out and back.
        let flare = ease::bump(share(age, 2.0, 14.0));
        if flare > 0.0 {
            let arm = (14.0 * flare).round() as i32;
            let (cx, cy) = (mid.0.round() as i32, mid.1.round() as i32);
            for k in 0..=arm {
                let ch = if k < arm / 3 { 'w' } else if k < arm * 2 / 3 { '9' } else { '6' };
                for (dx, dy) in [(k, 0), (-k, 0), (0, k), (0, -k)] {
                    pen.light(cx + dx, cy + dy, ch);
                }
            }
        }
        // A ring of sparks, thrown and falling.
        let spray = Spray { gravity: 0.07, ..Spray::round(seat * 17 + 5, 16, mid, (1.6, 3.2), (14.0, 24.0)) };
        spray.each(age - 3.0, |m| pen.streak(m.at, m.vel, 2.5, ramps::GOLD, m.k));
        for m in 0..4 {
            let t = share(age, 14.0 + m as f32 * 7.0, 12.0);
            let at = (mid.0 + (unit(m, seat as i32, 61) - 0.5) * 26.0, mid.1 - 6.0 - unit(m, seat as i32, 62) * 10.0);
            pen.twinkle(at, 2.0, t, ramps::HOLY);
        }
    }
    // RELIC, struck over the take line that names it (`marks::takes`).
    let _ = name;
    let at = 6.0;
    if age >= at {
        let d = age - at;
        let k = if d < 1.0 { 4 } else if d < 2.0 { 3 } else { 2 };
        let word = outlined(&cast(&letters("RELIC", '5', k), '6', '4', '9'), 'k');
        over_head(cv, &word, feet, 42.0, d, life - at);
    }
}

// ─── the party's banners ─────────────────────────────────────────────────────

/// The row a banner `h` tall stands on: the top band where no knight is,
/// else the lower, else the middle (`notices::board_row`).
fn banner_row(run: &Run, h: i32) -> i32 {
    let (ox, oy) = view_origin(run, None);
    let clear = |top: i32| {
        run.players.values().filter(|k| k.hp > 0 && !k.absent).all(|k| {
            let (x, y) = at(k.x, k.y);
            let (x, y) = (x - ox, y - oy);
            x < -12 || x > NATIVE_W + 12 || y + 6 < top || y - 26 > top + h
        })
    };
    [18, NATIVE_H - h - 34].into_iter().find(|&top| clear(top)).unwrap_or_else(|| super::notices::board_row(run, h, false))
}

/// A floor deeper than the realm has stood on.
fn record(cv: &mut Img, run: &Run, age: f32, floor: u32) {
    let life = 120.0;
    let slam = 9.0;
    let d = age - slam;
    let k = if d < 0.0 { 0 } else if d < 1.0 { 4 } else if d < 2.0 { 3 } else { 2 };
    let caption = letters("DEEPEST YET", 'O', 1);
    let line = letters("THE REALM HAS NEVER BEEN THIS DEEP", 'h', 1);
    let number = if k > 0 {
        cast(&letters(&format!("FLOOR {floor}"), '5', k), '6', '4', '9')
    } else {
        Img::new(text_width(&format!("FLOOR {floor}")) * 2, 14)
    };
    let inner = over_under(&over_under(&caption, &number, 3), &line, 3);
    let (w, h) = (inner.w.max(150) + 16, 7 + 3 + 14 + 3 + 7 + 10);
    let mut board = Img::new(w + 1, h + 1);
    chrome::panel(&mut board, (0, 0, w, h), Tone::FRAMED);
    // The number slams at its rest's size: the board stays the size it rests at.
    let rest_h = 7 + 3 + 14 + 3 + 7;
    board.stamp(&inner, (w - inner.w) / 2, 5 + (rest_h - inner.h) / 2);
    let y = banner_row(run, h);
    let x = (NATIVE_W - w) / 2;
    super::notices::board_in(cv, &board, (x, y), age, Some(life));
    // Its landing throws sparks off the board's edges, and it jolts.
    let mut pen = Pen::new(cv, BUDGET / 3);
    for (n, (sx, dir)) in [(x as f32 + 4.0, std::f32::consts::PI), (x as f32 + w as f32 - 4.0, 0.0)].into_iter().enumerate() {
        let spray = Spray { angle: dir, spread: 1.6, gravity: 0.1, ..Spray::round(40 + n as u32, 10, (sx, y as f32 + h as f32 / 2.0), (1.8, 3.4), (10.0, 18.0)) };
        spray.each(age - slam, |m| pen.streak(m.at, m.vel, 2.0, ramps::GOLD, m.k));
    }
    for m in 0..6 {
        let t = share(age, slam + 4.0 + m as f32 * 9.0, 16.0);
        let at = (x as f32 + 10.0 + unit(m, floor as i32, 71) * (w as f32 - 20.0), y as f32 + 4.0 + unit(m, floor as i32, 72) * (h as f32 - 8.0));
        pen.twinkle(at, 2.0, t, ramps::GOLD);
    }
}

// ─── the sky ─────────────────────────────────────────────────────────────────

/// A constellation's figure: strokes through its stars, on a 48 by 28
/// chart.
type Strokes = &'static [&'static [(f32, f32)]];

/// The eight figures, in `rules::rewards::CONSTELLATIONS` order: the lamp,
/// the crown, the grail, the dragon, the flamingo, the gnome, the key, the
/// comet.
const FIGURES: [Strokes; 8] = [
    &[&[(2.0, 8.0), (14.0, 15.0), (20.0, 21.0), (30.0, 21.0), (36.0, 15.0), (30.0, 13.0), (20.0, 13.0), (14.0, 15.0)], &[(36.0, 15.0), (43.0, 12.0), (41.0, 19.0), (36.0, 15.0)], &[(30.0, 13.0), (25.0, 8.0), (20.0, 13.0)]],
    &[&[(6.0, 24.0), (5.0, 9.0), (14.0, 17.0), (24.0, 4.0), (34.0, 17.0), (43.0, 9.0), (42.0, 24.0), (6.0, 24.0)], &[(14.0, 21.0), (24.0, 20.0), (34.0, 21.0)]],
    &[&[(11.0, 3.0), (13.0, 11.0), (19.0, 16.0), (24.0, 17.0), (29.0, 16.0), (35.0, 11.0), (37.0, 3.0)], &[(24.0, 17.0), (24.0, 23.0)], &[(16.0, 26.0), (24.0, 23.0), (32.0, 26.0)], &[(11.0, 3.0), (24.0, 5.0), (37.0, 3.0)]],
    &[&[(2.0, 21.0), (8.0, 15.0), (14.0, 19.0), (20.0, 13.0), (27.0, 17.0), (33.0, 11.0), (39.0, 12.0), (44.0, 6.0), (47.0, 10.0), (42.0, 12.0)], &[(20.0, 13.0), (23.0, 3.0), (30.0, 8.0), (27.0, 17.0)], &[(33.0, 11.0), (36.0, 4.0)]],
    &[&[(30.0, 2.0), (35.0, 4.0), (31.0, 6.0), (27.0, 10.0), (31.0, 14.0)], &[(22.0, 15.0), (31.0, 14.0), (38.0, 17.0), (31.0, 21.0), (21.0, 19.0), (22.0, 15.0)], &[(30.0, 21.0), (30.0, 27.0)], &[(30.0, 24.0), (25.0, 21.0)]],
    &[&[(24.0, 1.0), (15.0, 13.0), (33.0, 13.0), (24.0, 1.0)], &[(17.0, 13.0), (18.0, 20.0), (24.0, 27.0), (30.0, 20.0), (31.0, 13.0)], &[(20.0, 16.0), (28.0, 16.0)]],
    &[&[(4.0, 10.0), (9.0, 5.0), (14.0, 10.0), (9.0, 15.0), (4.0, 10.0)], &[(14.0, 10.0), (27.0, 10.0), (43.0, 10.0)], &[(36.0, 10.0), (36.0, 16.0)], &[(43.0, 10.0), (43.0, 17.0)]],
    &[&[(39.0, 4.0), (44.0, 7.0), (42.0, 12.0), (36.0, 10.0), (39.0, 4.0)], &[(36.0, 10.0), (21.0, 16.0), (5.0, 23.0)], &[(41.0, 12.0), (24.0, 21.0), (10.0, 27.0)], &[(37.0, 7.0), (19.0, 10.0), (2.0, 14.0)]],
];

/// Constellation `index`'s figure.
fn figure(index: u32) -> Strokes {
    FIGURES[(index % FIGURES.len() as u32) as usize]
}

/// Constellation `index`'s stars on its chart, in the order they fill: its
/// corners in the order its strokes are drawn, then as many more as it
/// takes along its longest runs.
pub(super) fn stars(index: u32) -> Vec<(f32, f32)> {
    let want = rules_rewards::size(index) as usize;
    let strokes = figure(index);
    // Each corner once, with where it lies along the drawing.
    let mut placed: Vec<((f32, f32), f32)> = Vec::new();
    let mut runs: Vec<((f32, f32), (f32, f32), f32)> = Vec::new();
    let mut along = 0.0f32;
    for stroke in strokes {
        for (i, &p) in stroke.iter().enumerate() {
            if i > 0 {
                let a = stroke[i - 1];
                let l = (p.0 - a.0).hypot(p.1 - a.1);
                runs.push((a, p, along));
                along += l;
            }
            if !placed.iter().any(|(q, _)| (q.0 - p.0).abs() < 1.5 && (q.1 - p.1).abs() < 1.5) {
                placed.push((p, along));
            }
        }
    }
    placed.truncate(want);
    // More along the longest runs, each run split as evenly as it may.
    let mut splits = vec![1u32; runs.len()];
    let length = |r: &((f32, f32), (f32, f32), f32)| (r.1.0 - r.0.0).hypot(r.1.1 - r.0.1);
    while placed.len() + splits.iter().map(|s| (s - 1) as usize).sum::<usize>() < want && !runs.is_empty() {
        let i = (0..runs.len())
            .max_by(|&a, &b| (length(&runs[a]) / splits[a] as f32).total_cmp(&(length(&runs[b]) / splits[b] as f32)))
            .unwrap();
        splits[i] += 1;
    }
    for (r, &n) in runs.iter().zip(&splits) {
        for k in 1..n {
            let t = k as f32 / n as f32;
            placed.push(((r.0.0 + (r.1.0 - r.0.0) * t, r.0.1 + (r.1.1 - r.0.1) * t), r.2 + length(r) * t));
        }
    }
    placed.sort_by(|a, b| a.1.total_cmp(&b.1));
    placed.into_iter().map(|(p, _)| p).collect()
}

/// A chart of constellation `index` at `(x, y)`, `k` pixels a chart unit:
/// its first `lit` stars bright (`glints`: some of them with a twinkle
/// `phase` each), the rest faint, and its strokes drawn on in `line` as
/// `drawn` goes 0 to 1.
#[allow(clippy::too_many_arguments)]
pub(super) fn chart(cv: &mut Img, (x, y): (i32, i32), k: f32, index: u32, lit: usize, drawn: f32, line: char, glints: &[(usize, f32)]) {
    let mut pen = Pen::new(cv, BUDGET);
    let p = |(cx, cy): (f32, f32)| (x as f32 + cx * k, y as f32 + cy * k);
    if drawn > 0.0 {
        let strokes = figure(index);
        let total: f32 = strokes.iter().flat_map(|s| s.windows(2)).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum();
        let mut left = total * drawn.min(1.0);
        for stroke in strokes {
            for w in stroke.windows(2) {
                let (a, b) = (p(w[0]), p(w[1]));
                let l = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
                let part = (left / l.max(0.01)).clamp(0.0, 1.0);
                left -= l;
                if part <= 0.0 {
                    continue;
                }
                let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * part).ceil() as i32;
                for i in 0..=n {
                    let t = i as f32 / ((b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1.0));
                    let (px, py) = ((a.0 + (b.0 - a.0) * t).round() as i32, (a.1 + (b.1 - a.1) * t).round() as i32);
                    if i % 2 == 0 {
                        pen.light(px, py, line);
                    }
                }
            }
        }
    }
    for (i, &s) in stars(index).iter().enumerate() {
        let (sx, sy) = p(s);
        let (sx, sy) = (sx.round() as i32, sy.round() as i32);
        if i < lit {
            pen.light(sx, sy, 'w');
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                pen.light(sx + dx, sy + dy, '2');
            }
        } else {
            pen.light(sx, sy, 'u');
        }
    }
    for &(i, phase) in glints {
        if let Some(&s) = stars(index).get(i) {
            pen.twinkle(p(s), 3.0, phase, ramps::GENIE);
        }
    }
}

/// A constellation whole: drawn star by star on a night board, its
/// strokes run through them, the whole of it flaring, and its name, who
/// lit it, and the gift it leaves.
fn lit(cv: &mut Img, run: &Run, age: f32, index: u32, by: &str) {
    let life = 150.0;
    let k = 2.0;
    let (cw, ch) = (48.0 * k, 28.0 * k);
    let name = format!("{} IS LIT", constellation(index));
    let who = format!("BY {}", by.to_uppercase().chars().take(16).collect::<String>());
    let gift = "A WISH MORE EACH, NEXT DELVE";
    let w = (cw as i32 + 16).max(text_width(&name) + 16).max(text_width(gift) + 16);
    let h = ch as i32 + 8 + 3 * 9 + 6;
    let mut board = Img::new(w + 1, h + 1);
    chrome::panel(&mut board, (0, 0, w, h), Tone::FRAMED);
    let n = stars(index).len();
    let shown = ((age - 8.0) / 1.0).clamp(0.0, n as f32) as usize;
    let drawn = share(age, 8.0 + n as f32, 22.0);
    let flare = share(age, 32.0 + n as f32, 20.0);
    let glints: Vec<(usize, f32)> = if flare > 0.0 && flare < 1.0 {
        (0..n).map(|i| (i, share(flare, i as f32 / n as f32 * 0.5, 0.5))).collect()
    } else {
        (0..shown.min(n)).map(|i| (i, share(age, 8.0 + i as f32, 6.0))).collect()
    };
    // A night of faint stars behind it.
    for sy in 3..ch as i32 + 5 {
        for sx in 3..w - 3 {
            if super::super::ink::hash(sx, sy, 0x5ca) % 61 == 0 {
                board.put(sx, sy, if super::super::ink::hash(sx, sy, 0x5cb) % 3 == 0 { 'u' } else { 'S' });
            }
        }
    }
    chart(&mut board, ((w - cw as i32) / 2, 5), k, index, shown, drawn, '2', &glints);
    let typed = |text: &str, from: f32| -> String { text.chars().take(((age - from) * 2.0).max(0.0) as usize).collect() };
    let ty = 5 + ch as i32 + 5;
    let line = typed(&name, 36.0 + n as f32);
    board.text((w - text_width(&name)) / 2, ty, &line, '3');
    let line = typed(&who, 52.0 + n as f32);
    board.text((w - text_width(&who)) / 2, ty + 9, &line, 'h');
    let line = typed(gift, 62.0 + n as f32);
    board.text((w - text_width(gift)) / 2, ty + 18, &line, 'O');
    let y = banner_row(run, h);
    super::notices::board_in(cv, &board, ((NATIVE_W - w) / 2, y), age, Some(life));
}

/// The sky's gift handed out: a board says so, and a star falls to each
/// knight.
fn gift(cv: &mut Img, run: &Run, (ox, oy): (i32, i32), age: f32, wishes: u8) {
    let life = 105.0;
    let words = if wishes == 1 { "A WISH MORE EACH".to_string() } else { format!("{wishes} WISHES MORE EACH") };
    let caption = letters("THE SKY'S GIFT", '3', 1);
    let line = letters(&words, 'O', 1);
    let inner = over_under(&caption, &line, 3);
    let (w, h) = (inner.w + 16, inner.h + 9);
    let mut board = Img::new(w + 1, h + 1);
    chrome::panel(&mut board, (0, 0, w, h), Tone::FRAMED);
    board.stamp(&inner, 8, 5);
    let y = banner_row(run, h);
    super::notices::board_in(cv, &board, ((NATIVE_W - w) / 2, y), age, Some(life));
    let mut pen = Pen::new(cv, BUDGET / 2);
    for (n, hero) in run.players.values().filter(|h| !h.absent && h.hp > 0).enumerate() {
        let (fx, fy) = at(hero.x, hero.y);
        let (fx, fy) = ((fx - ox) as f32, (fy - oy) as f32);
        let from = 12.0 + n as f32 * 5.0;
        let t = share(age, from, 20.0);
        if t <= 0.0 {
            continue;
        }
        let start = (fx + 18.0 - n as f32 * 9.0, -4.0);
        let end = (fx, fy - 10.0);
        if t < 1.0 {
            let now = crate::motion::arc(start, end, -10.0, ease::in_quad(t));
            let before = crate::motion::arc(start, end, -10.0, ease::in_quad((t - 0.08).max(0.0)));
            pen.streak(now, (now.0 - before.0, now.1 - before.1), 7.0, ramps::GENIE, 0.0);
            pen.twinkle(now, 2.0, 0.5, ramps::HOLY);
        }
        let landed = age - from - 20.0;
        pen.twinkle(end, 3.0, share(landed, 0.0, 14.0), ramps::GENIE);
        let ring = share(landed, 0.0, 14.0);
        if ring > 0.0 && ring < 1.0 {
            pen.ring((fx, fy + 1.0), 3.0 + 9.0 * ease::out(ring), 0.9, 0.42, ramps::GENIE, ease::in_quad(ring));
        }
    }
}

// ─── the road ahead ──────────────────────────────────────────────────────────

/// Where the road ahead's rows stand, between its headline and the
/// party's panel, and the most a row is high.
const ROAD_TOP: i32 = 37;
const ROAD_FOOT: i32 = 150;
const ROAD_ROW: i32 = 26;
/// The party's panel, and the line under it.
const PANEL: (i32, i32, i32, i32) = (8, 153, NATIVE_W - 16, 56);
const ROAD_HINT_Y: i32 = 214;
/// Steps a count takes to tick up.
const COUNT: f32 = 24.0;

/// A tier's numeral ink on the board.
fn tier_ink(tier: u8) -> char {
    tier_light(tier).1.0
}

/// One knight's ledger as words, each in its ink: the level (and the step
/// up to it), the experience (`xp_shown` of it, ticking up), the wishes,
/// the best tier, relics and the rung.
fn earned_words(e: &crate::rules::rewards::Earned, xp_shown: u32) -> Vec<(String, char)> {
    let mut out = Vec::new();
    if e.level > e.from_level && e.from_level > 0 {
        out.push((format!("LV {}>{}", e.from_level, e.level), 't'));
    } else {
        out.push((format!("LV {}", e.level.max(1)), 'i'));
    }
    if e.xp > 0 {
        out.push((format!("+{xp_shown} XP"), 'i'));
    }
    match e.wishes {
        0 => {}
        1 => out.push(("1 WISH".into(), 'i')),
        n => out.push((format!("{n} WISHES"), 'i')),
    }
    if let Some((wish, tier)) = &e.best
        && *tier >= 1
    {
        out.push((format!("{} {}", short_name(wish, 14), numeral(*tier)), tier_ink(*tier)));
    }
    match e.relics {
        0 => {}
        1 => out.push(("A RELIC".into(), '5')),
        n => out.push((format!("{n} RELICS"), '5')),
    }
    if e.rung > rules_rewards::RUNG_PIXELS[0] {
        out.push((format!("{} PX", e.rung), '3'));
    }
    out
}

/// `words` along a line from `x` to `right`, two spaces between, as many
/// as fit; the first `shown` letters of them.
fn lay(cv: &mut Img, (x, y): (i32, i32), right: i32, words: &[(String, char)], shown: usize) {
    let (mut cx, mut left) = (x, shown);
    for (text, ink) in words {
        let w = text_width(text);
        if cx + w > right {
            break;
        }
        let typed: String = text.chars().take(left).collect();
        left = left.saturating_sub(text.chars().count() + 2);
        cv.text(cx, y, &typed, *ink);
        cx += w + 13;
    }
}

/// A finished run's last word, `age` steps in: THE ROAD AHEAD. Each
/// knight's row slides in with what they earned this delve, its count
/// ticking up, and under it what comes next for them; then the party's
/// panel: the realm's sky with this delve's stars lighting on it, and what
/// lies ahead (a record, the Deep's next village or its road, the next
/// constellation). Black paper, as the results before it.
pub(super) fn road_page(run: &Run, age: u32) -> Img {
    let mut cv = Img::black(NATIVE_W, NATIVE_H);
    let a = age as f32;
    // The headline, its letters set down two a step.
    let text = "THE ROAD AHEAD";
    let set = (age as usize * 2).min(text.len());
    let head = letters(&text[..set], 'i', 2);
    cv.stamp(&head, (NATIVE_W - text_width(text) * 2) / 2, 7);
    if set > 0 && set < text.len() + 2 {
        let last = letters(&text[set.saturating_sub(1)..set], 't', 2);
        cv.stamp(&last, (NATIVE_W - text_width(text) * 2) / 2 + (set as i32 - 1) * 12, 7);
    }
    if age >= 6 {
        let sub = "WHAT EACH KNIGHT EARNED, AND WHAT COMES NEXT";
        cv.text((NATIVE_W - text_width(sub)) / 2, 25, sub, 'G');
    }
    let ids: Vec<u32> = run.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
    let n = ids.len().max(1) as i32;
    let rh = ((ROAD_FOOT - ROAD_TOP) / n).min(ROAD_ROW);
    let top = ROAD_TOP + (ROAD_FOOT - ROAD_TOP - rh * n) / 2;
    let tall = rh >= 24;
    for (i, &id) in ids.iter().enumerate() {
        let start = 8.0 + i as f32 * 5.0;
        if a < start {
            break;
        }
        let hero = &run.players[&id];
        let y = top + i as i32 * rh;
        let slide = (start + 6.0 - a).max(0.0);
        let x0 = -(slide * slide / 2.0) as i32;
        let (bx, bw, bh) = (x0 + 6, NATIVE_W - 12, rh - 2);
        let e = run.ledger_shown().earned.get(&id).cloned().unwrap_or_default();
        let up = e.level > e.from_level && e.from_level > 0;
        cv.rect(bx, y, bw, bh, if up { 'K' } else { 'k' });
        if up {
            for x in bx..bx + bw {
                cv.put(x, y, 'o');
            }
        }
        cv.rect(bx, y, 2, bh, super::party::seat_ink(id));
        let well_h = if tall { 22 } else { bh - 2 };
        chrome::shaped(&mut cv, (x0 + 12, y + 1, 26, well_h), &[1], Tone::WELL.rimmed(if up { chrome::BRASS_RIM } else { chrome::COLD_RIM }), false);
        let bust = super::party::bust(&super::knights::portrait(hero, id));
        if tall {
            cv.stamp(&bust, x0 + 13, y + 2);
        } else {
            let mut small = Img::new(bust.w / 2, bust.h / 2);
            for sy in 0..small.h {
                for sx in 0..small.w {
                    if let Some(c) = bust.get(sx * 2, sy * 2) {
                        small.set(sx, sy, c);
                    }
                }
            }
            cv.stamp(&small, x0 + 25 - small.w / 2, y + 1 + (well_h - small.h) / 2);
        }
        let name: String = hero.shown_name().to_uppercase().chars().take(10).collect();
        let (l1, l2) = if tall { (y + 4, y + 14) } else { (y + (bh - 16).max(0) / 2, y + (bh - 16).max(0) / 2 + 8) };
        cv.text(x0 + 44, l1, &name, if up { 't' } else { 'i' });
        // What they earned, the experience ticking up.
        let counted = (e.xp as f32 * ease::out(share(a, start + 3.0, COUNT))).round() as u32;
        let words = earned_words(&e, counted);
        lay(&mut cv, (x0 + 44 + text_width(&name).max(60) + 8, l1), NATIVE_W - 10, &words, usize::MAX);
        // What comes next, typed on once the counts have landed.
        let from = start + 3.0 + COUNT * 0.6;
        if a >= from {
            let mut next: Vec<(String, char)> = vec![("NEXT".into(), 'G')];
            next.extend(run.ahead(id).iter().map(|n| (n.line(), 'h')));
            let shown = ((a - from) * 3.0) as usize;
            lay(&mut cv, (x0 + 44, l2), NATIVE_W - 10, &next, shown);
        }
    }
    // The party's panel: the sky and the road.
    let at = 10.0 + n as f32 * 5.0 + COUNT;
    if a >= at {
        let d = a - at;
        let (px, py, pw, ph) = PANEL;
        let mut panel = Img::new(pw + 1, ph + 1);
        chrome::panel(&mut panel, (0, 0, pw, ph), Tone::FRAMED);
        let road = run.road();
        let sky = road.iter().find_map(|r| match r {
            crate::rules::rewards::Road::Sky { index, placed, .. } => Some((*index, *placed)),
            _ => None,
        });
        let text_x = if let Some((index, placed)) = sky {
            // This delve's stars light on it one after another.
            let new = run.ledger_shown().earned.values().map(|e| e.wishes as usize).sum::<usize>().min(placed as usize);
            let first = placed as usize - new;
            let lit_now = first + ((d - 6.0) / 2.0).clamp(0.0, new as f32) as usize;
            let glints: Vec<(usize, f32)> = (first..lit_now).map(|i| (i, share(d, 6.0 + (i - first) as f32 * 2.0, 8.0))).collect();
            chart(&mut panel, (8, 6), 1.5, index, lit_now, 1.0, '1', &glints);
            8 + 72 + 10
        } else {
            10
        };
        for (k, r) in road.iter().enumerate() {
            let line = r.line();
            let ink = match r {
                crate::rules::rewards::Road::Record { .. } => 't',
                crate::rules::rewards::Road::Sky { .. } => '3',
                _ => 'O',
            };
            let shown = ((d - k as f32 * 6.0) * 3.0).max(0.0) as usize;
            let room = ((pw - text_x - 6) / 6).max(0) as usize;
            let line: String = line.chars().take(room.min(shown)).collect();
            panel.text(text_x, 8 + k as i32 * 11, &line, ink);
        }
        super::notices::board_from(&mut cv, &panel, (px, py), d, None, 6.0);
    }
    if age >= 16 {
        let hint = super::again_hint(run);
        cv.text((NATIVE_W - text_width(&hint)) / 2, ROAD_HINT_Y, &hint, 'G');
    }
    cv
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__rewards__tests.rs"]
mod tests;
