//! The juice kit, drawing half: the loud little moves every mini-game wants,
//! as pure functions of an age in ticks (`rules::juice` keeps what happened).
//!
//! - Impact: `shake` (scaled to the blow), `impact_frame` (the struck sprite
//!   in one light ink), `squash_stretch` (and its `lurch`), `impact_burst`,
//!   `death_burst`, `pop_number` (damage that rises), `hitmarker`.
//! - Moments: `banner` (slam in, hold, dissolve up), `countdown` (3-2-1-GO
//!   with a shock ring), `fly_up` (a score flying to the HUD), `iris` (a
//!   dithered wipe between scenes).
//! - Finish: `podium` (steps rising, the winner crowned; `podium_stands` says
//!   where its words sit), `confetti`, `firework`.
//! - Pieces: `big_text`, `dissolve`, `vignette`.
//!
//! Everything is the realm's inks, an ordered dither and no glow that dims a
//! figure; nothing is kept between frames and nothing is rolled, so the host,
//! a friend's mirror and a browser guest draw the same pixels. Positions are
//! in the canvas's own pixels (384 by 224 for a mini-game's screen).

use super::pfx::{self, Pen, Ramp, Shape, Spray, ramps};
use super::enlarge;
use super::super::ink::{Img, bayer, hash, text_width};
use crate::motion::{self, Spring, ease};
use crate::rules::juice::Weight;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Warm,
    Steel,
    Hot,
}

impl Tone {
    fn inks(self) -> (char, char, char) {
        match self {
            Tone::Warm => ('t', 'B', 'T'),
            Tone::Steel => ('i', 'g', 'H'),
            Tone::Hot => ('R', 'n', 'c'),
        }
    }
}

pub(crate) fn shake(blows: impl IntoIterator<Item = (Weight, u32)>, tick: u64) -> (i32, i32) {
    blows
        .into_iter()
        .map(|(w, age)| {
            let full = w.shake_ticks();
            motion::shake(tick, full.saturating_sub(age), full, w.shake())
        })
        .max_by_key(|&(x, y)| x * x + y * y)
        .unwrap_or((0, 0))
}

pub(crate) fn impact_frame(sprite: &Img, ink: char) -> Img {
    let mut out = sprite.clone();
    let Some(c) = super::super::ink::ink(ink) else { return out };
    for y in 0..out.h {
        for x in 0..out.w {
            if out.get(x, y).is_some_and(|p| p != super::super::ink::BLACK) {
                out.set(x, y, c);
            }
        }
    }
    out
}

pub(crate) fn squash_stretch(sprite: &Img, age: u32, amount: f32) -> (Img, f32) {
    let t = age as f32;
    let k = amount * motion::math::exp(-t * 0.26) * motion::math::cos(t * 0.95);
    let (wide, high) = motion::squash(k);
    let grow = high.max(1.0);
    let (cw, ch) = ((sprite.w as f32 * wide).round().max(1.0) as i32, (sprite.h as f32 * grow).round() as i32);
    let content = (sprite.h as f32 * high).round().max(1.0) as i32;
    let mut out = Img::new(cw, ch);
    for y in 0..content {
        for x in 0..cw {
            let (sx, sy) = ((x as f32 / wide) as i32, (y as f32 / high) as i32);
            if let Some(c) = sprite.get(sx, sy) {
                out.set(x, ch - content + y, c);
            }
        }
    }
    (out, grow)
}

pub(crate) fn lurch(weight: Weight, age: u32) -> f32 {
    let reach = match weight {
        Weight::Graze => 0.0,
        Weight::Solid => 0.22,
        Weight::Crushing => 0.4,
        Weight::Kill | Weight::Finale => 0.6,
    };
    reach * motion::math::exp(-(age as f32) * 0.3)
}

pub(crate) fn impact_burst(im: &mut Img, at: (f32, f32), age: u32, seed: u32, weight: Weight, tick: u64) {
    let (n, len, speed) = match weight {
        Weight::Graze => (7, 6.0, 2.4),
        Weight::Solid => (13, 10.0, 3.0),
        Weight::Crushing => (20, 15.0, 3.6),
        Weight::Kill => (28, 20.0, 4.0),
        Weight::Finale => (38, 28.0, 4.6),
    };
    let mut pen = Pen::new(im, 900);
    if age < 7 {
        pfx::rays(&mut pen, at, 8, len, 1.0, seed as f32 * 0.37, ramps::GOLD, age as f32 / 7.0);
    }
    if age < 2 {
        let r = 2 + weight as i32 * 2 - age as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    pen.put(at.0 as i32 + dx, at.1 as i32 + dy, if dx * dx + dy * dy < r * r / 2 { 'c' } else { 'T' });
                }
            }
        }
    }
    let spray = Spray { gravity: 0.16, ..Spray::round(seed, n, at, (speed * 0.5, speed), (8.0, 16.0)) };
    spray.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::EMBER, 2, tick));
    if weight >= Weight::Crushing && age < 16 {
        let k = age as f32 / 16.0;
        pen.shock((at.0, at.1 + 6.0), 3.0 + age as f32 * len * 0.16, 1.4, 0.45, ramps::SMOKE, k);
    }
}

pub(crate) fn death_burst(im: &mut Img, at: (f32, f32), age: u32, seed: u32, tick: u64) {
    let mut pen = Pen::new(im, 1400);
    let a = age as f32;
    if age < 8 {
        pfx::rays(&mut pen, at, 10, 20.0, 1.0, seed as f32, ramps::GOLD, a / 8.0);
    }
    let shards = Spray { gravity: 0.24, ..Spray::round(seed, 16, at, (1.2, 3.8), (16.0, 30.0)) };
    shards.each(a, |m| pfx::mote(&mut pen, &m, Shape::Shard, ramps::BONE, 2, tick));
    let embers = Spray { gravity: -0.03, ..Spray::round(seed ^ 0x5a, 12, at, (0.6, 2.4), (14.0, 28.0)) };
    embers.each(a, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::EMBER, 1, tick));
    let drops = Spray { gravity: 0.28, ..Spray::round(seed ^ 0xb1, 8, at, (1.0, 3.0), (12.0, 22.0)) };
    drops.each(a, |m| pfx::mote(&mut pen, &m, Shape::Droplet, ramps::BLOOD, 1, tick));
    let smoke = Spray { gravity: -0.05, ..Spray::round(seed ^ 0xc3, 6, at, (0.3, 1.1), (18.0, 34.0)) };
    smoke.each(a, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::SMOKE, 2, tick));
    if age < 20 {
        pen.shock((at.0, at.1 + 10.0), 4.0 + a * 3.0, 1.6, 0.45, ramps::SMOKE, a / 20.0);
    }
}

pub(crate) fn hitmarker(im: &mut Img, c: (i32, i32), age: u32, kill: bool) {
    if age >= 7 {
        return;
    }
    let (reach, gap) = (if kill { 9 } else { 6 }, 3 + age as i32 / 2);
    let ink = if kill { '@' } else { 'H' };
    for o in gap..=reach {
        for (dx, dy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
            im.put(c.0 + dx * o + dx, c.1 + dy * o, 'k');
            im.put(c.0 + dx * o, c.1 + dy * o, ink);
        }
    }
}

pub(crate) fn pop_number(im: &mut Img, text: &str, at: (i32, i32), age: u32, life: u32, scale: i32, fill: char) {
    if age >= life {
        return;
    }
    let hop = (ease::out((age as f32 / 7.0).min(1.0)) * 12.0) as i32 + (age as i32 * 3 / 5);
    let mut num = big_text_img(text, scale, fill, 'k');
    let fade = ((age as f32 - life as f32 * 0.6) / (life as f32 * 0.4)).max(0.0);
    dissolve(&mut num, fade);
    im.stamp(&num, at.0 - num.w / 2, at.1 - hop - num.h);
}

pub(crate) fn big_text_img(s: &str, scale: i32, fill: char, edge: char) -> Img {
    let mut small = Img::new(text_width(s).max(1), 7);
    small.text(0, 0, s, 'h');
    let (w, h) = (small.w * scale + 4, 7 * scale + 4);
    let mut out = Img::new(w, h);
    for y in 0..7 {
        for x in 0..small.w {
            if small.get(x, y).is_some() {
                out.rect(2 + x * scale, 2 + y * scale, scale, scale, fill);
            }
        }
    }
    out.outline_outside(edge);
    out.outline_outside('k');
    out
}

pub(crate) fn big_text(im: &mut Img, cx: i32, y: i32, s: &str, scale: i32, fill: char, edge: char) {
    let t = big_text_img(s, scale, fill, edge);
    im.stamp(&t, cx - t.w / 2, y);
}

pub(crate) fn dissolve(im: &mut Img, k: f32) {
    if k <= 0.0 {
        return;
    }
    for y in 0..im.h {
        for x in 0..im.w {
            if bayer(x, y) < k {
                im.clear(x, y);
            }
        }
    }
}

pub(crate) fn banner(im: &mut Img, y: i32, title: &str, sub: &str, age: u32, life: u32, tone: Tone, cap: i32) {
    if age >= life {
        return;
    }
    let (fill, edge, flash) = tone.inks();
    let rest = ((im.w - 24) / text_width(title).max(1)).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / text_width(title).max(1)).max(rest);
    let tail = life.saturating_sub(14);
    let out = if age > tail { (age - tail) as f32 / 14.0 } else { 0.0 };
    let lift = (ease::in_quad(out) * 10.0) as i32;
    let mut text = big_text_img(title, scale, if age < 3 { flash } else { fill }, edge);
    dissolve(&mut text, out);
    let (x, top) = (im.w / 2 - text.w / 2, y - lift - (scale - rest) * 3);
    let grow = ease::out((age as f32 / 8.0).min(1.0));
    let half = (((text.w / 2 + 8) as f32) * grow) as i32;
    for (row, inks) in [(y - 5 - lift, ['o', 'R']), (y + 7 * rest + 8 - lift, ['R', 'o'])] {
        for dx in -half..=half {
            let fall = dx.abs() as f32 / half.max(1) as f32;
            if bayer(im.w / 2 + dx, row) < fall * fall * 0.9 + out {
                continue;
            }
            im.put(im.w / 2 + dx, row, inks[0]);
            im.put(im.w / 2 + dx, row + 1, inks[1]);
        }
    }
    im.stamp(&text, x, top);
    if !sub.is_empty() && age >= 4 {
        let mut small = big_text_img(sub, 1, 'h', 'k');
        dissolve(&mut small, out);
        im.stamp(&small, im.w / 2 - small.w / 2, y + 7 * rest + 12 - lift);
    }
}

pub(crate) fn countdown(im: &mut Img, at: (i32, i32), label: &str, age: u32, life: u32, go: bool) {
    if age >= life {
        return;
    }
    let width = text_width(label).max(5);
    let rest = (150 / width).clamp(4, 9);
    let pop = Spring::SNAPPY.at(age as f32, 1.7, 1.0, 0.0).clamp(0.9, 1.7);
    let out = ((age as f32 - life as f32 * 0.7) / (life as f32 * 0.3)).max(0.0);
    let scale = (((rest as f32 * pop) * (1.0 - out * 0.4)).round() as i32).max(1);
    let (fill, edge) = if go { ('5', 'p') } else { ('T', 'B') };
    let mut t = big_text_img(label, scale, if age < 2 { 'H' } else { fill }, edge);
    dissolve(&mut t, out);
    {
        let mut pen = Pen::new(im, 700);
        let a = age as f32;
        pen.shock((at.0 as f32, at.1 as f32 + 4.0), 6.0 + a * 11.0, 2.2, 0.55, if go { ramps::GOLD } else { ramps::BONE }, (a / 14.0).min(1.0));
        if go {
            pen.shock((at.0 as f32, at.1 as f32 + 4.0), 2.0 + a * 17.0, 1.6, 0.55, ramps::EMBER, (a / 12.0).min(1.0));
            pfx::rays(&mut pen, (at.0 as f32, at.1 as f32), 14, 70.0, 0.7, 0.2, ramps::GOLD, (a / 14.0).min(1.0));
        }
    }
    im.stamp(&t, at.0 - t.w / 2, at.1 - t.h / 2);
}

pub(crate) fn fly_up(im: &mut Img, text: &str, from: (i32, i32), to: (i32, i32), age: u32, life: u32, fill: char) {
    if age >= life {
        return;
    }
    let t = age as f32 / life as f32;
    let (x, y) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 26.0, ease::both(t));
    let img = big_text_img(text, 1 + i32::from(t < 0.5), fill, 'B');
    im.stamp(&img, x as i32 - img.w / 2, y as i32 - img.h / 2);
    for k in 1..4 {
        let (tx, ty) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 26.0, ease::both((t - k as f32 * 0.05).max(0.0)));
        im.put(tx as i32, ty as i32, ['@', 'o', 'R'][k as usize - 1]);
    }
}

pub(crate) fn bump(age: u32, life: u32) -> f32 {
    if age >= life { 0.0 } else { ease::bump(age as f32 / life as f32) }
}

pub(crate) fn iris(im: &mut Img, centre: (i32, i32), k: f32, fill: char) {
    if k <= 0.0 {
        return;
    }
    let reach = ((im.w * im.w + im.h * im.h) as f32).sqrt() * 0.55;
    for y in 0..im.h {
        for x in 0..im.w {
            let d = ((x - centre.0) as f32).hypot((y - centre.1) as f32) / reach;
            if d + bayer(x, y) * 0.18 > 1.0 - k * 1.18 {
                im.put(x, y, fill);
            }
        }
    }
}

pub(crate) fn vignette(im: &mut Img, (bx, by, bw, bh): (i32, i32, i32, i32), k: f32, depth: i32, fill: char) {
    if k <= 0.0 {
        return;
    }
    for y in by..by + bh {
        for x in bx..bx + bw {
            let edge = (x - bx).min(bx + bw - 1 - x).min(y - by).min(by + bh - 1 - y);
            if edge >= depth {
                continue;
            }
            let near = 1.0 - edge as f32 / depth as f32;
            if bayer(x, y) < near * near * k {
                im.put(x, y, fill);
            }
        }
    }
}

pub(crate) fn confetti(im: &mut Img, (bx, by, bw, bh): (i32, i32, i32, i32), tick: u32, seed: u32, n: u32) {
    const INKS: [char; 8] = ['t', 'T', 'O', 'i', 'h', 'R', 'c', '@'];
    for i in 0..n {
        let u = |s: u32| (hash(i as i32, seed as i32, s) & 0xffff) as f32 / 65536.0;
        let fall = 0.8 + u(1) * 1.5;
        let span = (bh + 24) as f32;
        let y = (u(2) * span + tick as f32 * fall).rem_euclid(span) - 12.0;
        let x = u(3) * bw as f32 + (tick as f32 * 0.11 + u(4) * 6.28).sin() * (4.0 + u(5) * 6.0);
        let flip = (tick / 3 + i) % 3;
        let ink = INKS[(hash(i as i32, seed as i32, 6) % INKS.len() as u32) as usize];
        let (px, py) = (bx + x as i32, by + y as i32);
        if py < by || py >= by + bh {
            continue;
        }
        match flip {
            0 => im.rect(px, py, 3, 2, ink),
            1 => im.rect(px, py, 2, 3, ink),
            _ => im.rect(px, py, 2, 2, ink),
        }
    }
}

pub(crate) fn firework(im: &mut Img, x: f32, base: f32, top: f32, age: u32, seed: u32, ramp: Ramp, tick: u64) {
    const RISE: f32 = 14.0;
    let a = age as f32;
    let mut pen = Pen::new(im, 700);
    if a < RISE {
        let y = base + (top - base) * ease::out(a / RISE);
        pen.streak((x, y), (0.0, -1.0), 7.0, ramps::GOLD, a / RISE);
        return;
    }
    let t = a - RISE;
    let burst = Spray { gravity: 0.05, drag: 0.07, ..Spray::round(seed, 52, (x, top), (2.0, 5.2), (28.0, 48.0)) };
    let glitter = Spray { gravity: 0.03, drag: 0.1, ..Spray::round(seed ^ 0x31, 26, (x, top), (0.8, 2.6), (30.0, 52.0)) };
    glitter.each(t, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, tick));
    burst.each(t, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramp, 2, tick));
    if t < 6.0 {
        pfx::rays(&mut pen, (x, top), 10, 26.0, 1.0, seed as f32, ramps::GOLD, t / 6.0);
    }
}

pub(crate) struct Place {
    pub sprite: Img,
    pub name: String,
    pub line: String,
}

pub(crate) type Rect = (i32, i32, i32, i32);

const STEP_HEIGHTS: [i32; 3] = [58, 40, 28];
const STEP_START: [u32; 3] = [16, 8, 0];
const RANK_SCALE: [i32; 3] = [5, 4, 4];
const STEP_W: i32 = 84;
const STEP_GAP: i32 = 4;
const MOST_LETTERS: usize = 13;
const WORDS_BELOW: i32 = 22;

pub(crate) struct Stand {
    pub rank: Rect,
    pub rank_scale: i32,
    pub name: Rect,
    pub name_text: String,
    pub line: Rect,
    pub line_text: String,
    pub words_at: u32,
}

fn fitted(s: &str) -> String {
    if s.chars().count() <= MOST_LETTERS {
        s.to_string()
    } else {
        s.chars().take(MOST_LETTERS - 1).chain(std::iter::once('.')).collect()
    }
}

fn words_box(s: &str, cx: i32, top: i32, frame_w: i32) -> Rect {
    let glyphs = text_width(s).max(1);
    let img_w = glyphs + 4;
    let x = (cx - img_w / 2).clamp(0, (frame_w - img_w).max(0));
    (x + 1, top + 1, glyphs + 2, 7 + 2)
}

pub(crate) fn podium_stands((fw, fh): (i32, i32), (bx, by, bw, bh): Rect, places: &[Place]) -> Vec<Stand> {
    let floor = by + bh - 6;
    let cx = bx + bw / 2;
    let xs = [cx, cx - STEP_W - STEP_GAP, cx + STEP_W + STEP_GAP];
    let below = (floor + WORDS_BELOW).min(fh) - WORDS_BELOW;
    places
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, p)| {
            let mut scale = RANK_SCALE[i];
            while scale > 1 && 6 + 7 * scale + 4 > STEP_HEIGHTS[i] - 2 {
                scale -= 1;
            }
            let img_w = 5 * scale + 4;
            let rank = (xs[i] - img_w / 2 + 1, floor - STEP_HEIGHTS[i] + 6 + 1, 5 * scale + 2, 7 * scale + 2);
            let (name_text, line_text) = (fitted(&p.name), fitted(&p.line));
            Stand {
                rank,
                rank_scale: scale,
                name: words_box(&name_text, xs[i], below + 2, fw),
                name_text,
                line: words_box(&line_text, xs[i], below + 11, fw),
                line_text,
                words_at: STEP_START[i] + 18,
            }
        })
        .collect()
}

pub(crate) fn podium(im: &mut Img, area: Rect, places: &[Place], age: u32) -> (i32, i32) {
    let (bx, by, bw, bh) = area;
    let floor = by + bh - 6;
    let cx = bx + bw / 2;
    let spots = [(cx, 0usize), (cx - STEP_W - STEP_GAP, 1), (cx + STEP_W + STEP_GAP, 2)];
    let stands = podium_stands((im.w, im.h), area, places);
    let mut crown = (cx, floor - STEP_HEIGHTS[0]);
    for &(x, i) in spots.iter().rev() {
        let (Some(p), Some(stand)) = (places.get(i), stands.get(i)) else { continue };
        let rise = ease::out(((age as f32 - STEP_START[i] as f32) / 18.0).clamp(0.0, 1.0));
        let h = (STEP_HEIGHTS[i] as f32 * rise) as i32;
        if h <= 0 {
            continue;
        }
        let top = floor - h;
        let left = x - STEP_W / 2;
        for yy in top..floor {
            for xx in left..left + STEP_W {
                let ink = if yy == top {
                    if i == 0 { 'T' } else { 'h' }
                } else if xx == left || xx == left + STEP_W - 1 {
                    'g'
                } else if (yy - top) < 3 {
                    if i == 0 { 'o' } else { 'J' }
                } else if bayer(xx, yy) < 0.18 {
                    'g'
                } else {
                    'j'
                };
                im.put(xx, yy, ink);
            }
        }
        if h + 2 >= STEP_HEIGHTS[i] {
            let rank = big_text_img(&(i + 1).to_string(), stand.rank_scale, if i == 0 { 'O' } else { 'G' }, 'g');
            im.stamp(&rank, stand.rank.0 - 1, stand.rank.1 - 1);
        }
        let drop = ((1.0 - ease::out(((age as f32 - STEP_START[i] as f32 - 12.0) / 12.0).clamp(0.0, 1.0))) * 70.0) as i32;
        if age as f32 >= STEP_START[i] as f32 + 8.0 {
            let tall = if i == 0 { 68.0 } else { 56.0 };
            let k = (tall / p.sprite.h as f32 * 4.0).round().max(1.0) / 4.0;
            let fig = enlarge(&p.sprite, k);
            let (fx, fy) = (x - fig.w / 2, top - fig.h - drop);
            im.stamp(&fig, fx, fy);
            if i == 0 {
                crown = (x, fy);
            }
        }
        if age >= stand.words_at {
            let name = big_text_img(&stand.name_text, 1, 'i', 'k');
            im.stamp(&name, stand.name.0 - 1, stand.name.1 - 1);
            let line = big_text_img(&stand.line_text, 1, if i == 0 { 't' } else { 'h' }, 'k');
            im.stamp(&line, stand.line.0 - 1, stand.line.1 - 1);
        }
    }
    crown
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__juice__tests.rs"]
mod tests;
