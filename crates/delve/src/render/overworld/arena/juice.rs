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

/// How a banner is inked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    /// Cream on amber: the usual call.
    Warm,
    /// Grey steel: a quiet call for another knight's moment.
    Steel,
    /// Blood: danger, a streak ended.
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

// ─── impact ──────────────────────────────────────────────────────────────────

/// The screen's shake for blows `(weight, age)`: the strongest of them.
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

/// The impact frame: every opaque pixel of `sprite` in the ink `ink`, its
/// black outline kept, so a struck figure reads as one bright silhouette.
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

/// A struck sprite squashed wide and low, then stretched tall and thin, then
/// settling (`age` ticks since the blow). The result keeps the feet at its
/// bottom edge; its canvas is `grow` times as tall as the sprite, and the
/// caller stands it `grow` times as tall in the world so the pixels keep
/// their size. `amount` is 0 for none; a crushing blow is about 0.4.
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

/// How far a struck body is knocked back along the blow, in world units,
/// `age` ticks after it landed: a lurch that eases home.
pub(crate) fn lurch(weight: Weight, age: u32) -> f32 {
    let reach = match weight {
        Weight::Graze => 0.0,
        Weight::Solid => 0.22,
        Weight::Crushing => 0.4,
        Weight::Kill | Weight::Finale => 0.6,
    };
    reach * motion::math::exp(-(age as f32) * 0.3)
}

/// The spark of a blow at `at` (`age` ticks old, `weight` heavy): a star at
/// the instant of contact, sparks thrown and falling, and for the heavy
/// blows a ring of dust on the floor.
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
    // The instant of contact: a hot disc that is gone in two ticks.
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

/// A knight going down at `at`: shards and smoke thrown wide, embers, a few
/// dark drops and a shock ring.
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

/// The mark of a blow you landed, at the crosshair: four ticks that flick
/// in and out; a kill's are long and warm.
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

/// A number thrown up off a struck knight at `at`: it hops, then drifts up
/// and dissolves. `scale` 1 for a scratch, 2 for a big one.
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

// ─── text ────────────────────────────────────────────────────────────────────

/// `s` in the 5x7 font `scale` times as big, `fill` inside a two-pixel edge
/// (`edge` then black), as its own picture.
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

/// `s` centred on `cx` at row `y`, the same way up.
pub(crate) fn big_text(im: &mut Img, cx: i32, y: i32, s: &str, scale: i32, fill: char, edge: char) {
    let t = big_text_img(s, scale, fill, edge);
    im.stamp(&t, cx - t.w / 2, y);
}

/// Take `k` (0 to 1) of `im`'s pixels away, by the ordered dither.
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

// ─── moments ─────────────────────────────────────────────────────────────────

/// A banner `age` ticks into its `life`: it slams in big and settles to its
/// size, light rails run out either side of it, a line of small words sits
/// under it, and it slides up and dissolves at the end. `y` is the row of
/// its top at rest; `cap` the biggest scale it may rest at (3 for the call
/// of the moment, 2 for a second one under it).
pub(crate) fn banner(im: &mut Img, y: i32, title: &str, sub: &str, age: u32, life: u32, tone: Tone, cap: i32) {
    if age >= life {
        return;
    }
    let (fill, edge, flash) = tone.inks();
    // The biggest whole scale that fits the width, never above 3.
    let rest = ((im.w - 24) / text_width(title).max(1)).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / text_width(title).max(1)).max(rest);
    let tail = life.saturating_sub(14);
    let out = if age > tail { (age - tail) as f32 / 14.0 } else { 0.0 };
    let lift = (ease::in_quad(out) * 10.0) as i32;
    let mut text = big_text_img(title, scale, if age < 3 { flash } else { fill }, edge);
    dissolve(&mut text, out);
    let (x, top) = (im.w / 2 - text.w / 2, y - lift - (scale - rest) * 3);
    // The rails: two dithered bars of light running out from the middle.
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

/// One beat of a countdown (a digit or a word) `age` ticks into its `life`:
/// it drops in huge, springs to size, a shock ring leaves it, and it
/// shrinks away. `at` is the centre. `go` rings it twice and runs rays.
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

/// A figure flying from `from` to `to` over `life` ticks (a score to the
/// HUD): it arcs up first, then dives home, small and warm. Draws nothing
/// before it begins or after it lands (`age >= life`, which is also when
/// the HUD should pulse).
pub(crate) fn fly_up(im: &mut Img, text: &str, from: (i32, i32), to: (i32, i32), age: u32, life: u32, fill: char) {
    if age >= life {
        return;
    }
    let t = age as f32 / life as f32;
    let (x, y) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 26.0, ease::both(t));
    let img = big_text_img(text, 1 + i32::from(t < 0.5), fill, 'B');
    im.stamp(&img, x as i32 - img.w / 2, y as i32 - img.h / 2);
    // A few sparks trail it.
    for k in 1..4 {
        let (tx, ty) = motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 26.0, ease::both((t - k as f32 * 0.05).max(0.0)));
        im.put(tx as i32, ty as i32, ['@', 'o', 'R'][k as usize - 1]);
    }
}

/// How far a pulse of `age` ticks has swelled, 0 to 1 and gone: for a HUD
/// number that bumps when a flying score lands.
pub(crate) fn bump(age: u32, life: u32) -> f32 {
    if age >= life { 0.0 } else { ease::bump(age as f32 / life as f32) }
}

/// A dithered iris of ink `fill` closing over the picture from its edges to
/// `centre` (`k` 0 open, 1 shut) or, going the other way, opening.
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

/// The danger of a blow taken, as a dithered rim of `fill` along the edges of
/// a `(x, y, w, h)` box, `k` (0 to 1) how strong, `depth` pixels deep.
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

// ─── finish ──────────────────────────────────────────────────────────────────

/// A fall of paper in the realm's warm and grey inks over the box `(x, y, w,
/// h)` at `tick`: `n` pieces, each its own speed, sway and flutter.
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

/// A firework `age` ticks after it was fired from `(x, base)`: a streak
/// rises to `(x, top)`, bursts into sparks that fall, and fades.
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

/// One of the three on a podium.
pub(crate) struct Place {
    pub sprite: Img,
    pub name: String,
    pub line: String,
}

/// A box on the canvas: `(x, y, w, h)`.
pub(crate) type Rect = (i32, i32, i32, i32);

/// The steps' heights (first, second, third) and the tick each starts to rise.
const STEP_HEIGHTS: [i32; 3] = [58, 40, 28];
const STEP_START: [u32; 3] = [16, 8, 0];
/// The size each step's rank digit would like; it is cut down until it fits
/// its own step's face, so the low step never gets the high step's digit.
const RANK_SCALE: [i32; 3] = [5, 4, 4];
/// A step's width, and the gap between steps.
const STEP_W: i32 = 84;
const STEP_GAP: i32 = 4;
/// The most letters a name or a score line may have under a step: the steps
/// stand `STEP_W + STEP_GAP` apart, and the longest words must stay clear of
/// the next step's words. Longer ones are cut and end in a full stop.
const MOST_LETTERS: usize = 13;
/// Rows the words under the steps take below the floor line.
const WORDS_BELOW: i32 = 22;

/// Where one step's words sit on the canvas, as the boxes the letters (and
/// the one-pixel ring the fill wears) really cover, so a test or a ledger of
/// claims can see that none meets another. The words are already cut to fit.
pub(crate) struct Stand {
    /// The rank digit, at the size that fits the step.
    pub rank: Rect,
    pub rank_scale: i32,
    /// The name under the step, and the score line under the name.
    pub name: Rect,
    pub name_text: String,
    pub line: Rect,
    pub line_text: String,
    /// The tick of the scene from which the name and line are drawn.
    pub words_at: u32,
}

/// `s`, cut to `MOST_LETTERS` with a full stop when it was longer.
fn fitted(s: &str) -> String {
    if s.chars().count() <= MOST_LETTERS {
        s.to_string()
    } else {
        s.chars().take(MOST_LETTERS - 1).chain(std::iter::once('.')).collect()
    }
}

/// The box of `s` as `big_text_img(s, 1, ..)` stamped centred on `cx` with
/// its top at `top`, kept inside a canvas `frame_w` wide: the stamped
/// picture's own corner, then the letters and their ring.
fn words_box(s: &str, cx: i32, top: i32, frame_w: i32) -> Rect {
    let glyphs = text_width(s).max(1);
    let img_w = glyphs + 4;
    let x = (cx - img_w / 2).clamp(0, (frame_w - img_w).max(0));
    (x + 1, top + 1, glyphs + 2, 7 + 2)
}

/// Where the words of a podium on the box `area` stand, for as many of
/// `places` as the podium shows (three), on a canvas of `frame` size. Each
/// is the full-grown step's; the rise only reveals them.
pub(crate) fn podium_stands((fw, fh): (i32, i32), (bx, by, bw, bh): Rect, places: &[Place]) -> Vec<Stand> {
    let floor = by + bh - 6;
    let cx = bx + bw / 2;
    let xs = [cx, cx - STEP_W - STEP_GAP, cx + STEP_W + STEP_GAP];
    // The words need `WORDS_BELOW` rows under the floor: lift them if the box is low.
    let below = (floor + WORDS_BELOW).min(fh) - WORDS_BELOW;
    places
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, p)| {
            let mut scale = RANK_SCALE[i];
            // Picture 6 rows into the face, 7 rows per size and the edge, 2 rows to spare.
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

/// A podium on the box `(x, y, w, h)` (at least 264 wide; leave 16 rows under
/// it for the words): the first place in the middle on the tallest step, the
/// second at their left, the third at their right. Steps rise in turn (third,
/// second, first), each knight dropping onto theirs; `age` runs from the
/// first tick of the scene. Every rank digit is sized to fit its step, and
/// every name and score line is cut and placed (`podium_stands`) so it stays
/// clear of the digits, of its neighbours and of the canvas edge. Returns the
/// top-centre of the first step's knight, where a crown or a confetti gun
/// belongs.
pub(crate) fn podium(im: &mut Img, area: Rect, places: &[Place], age: u32) -> (i32, i32) {
    let (bx, by, bw, bh) = area;
    let floor = by + bh - 6;
    let cx = bx + bw / 2;
    let spots = [(cx, 0usize), (cx - STEP_W - STEP_GAP, 1), (cx + STEP_W + STEP_GAP, 2)];
    let stands = podium_stands((im.w, im.h), area, places);
    let mut crown = (cx, floor - STEP_HEIGHTS[0]);
    // Back to front: the lower steps first, the winner's last.
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
        // The rank, big and dark on the step's face, at the size that fits it.
        if h + 2 >= STEP_HEIGHTS[i] {
            let rank = big_text_img(&(i + 1).to_string(), stand.rank_scale, if i == 0 { 'O' } else { 'G' }, 'g');
            im.stamp(&rank, stand.rank.0 - 1, stand.rank.1 - 1);
        }
        // The knight: stands on the step after it has risen.
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
