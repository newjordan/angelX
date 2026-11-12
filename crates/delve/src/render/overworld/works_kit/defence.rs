//! What stands against the Deep: the palisade, the timber gate, the watchtower and the brazier,
//! and what happens to a thing the Deep reaches, a fire and a crack (`rules::deep_raid`).
//! Timber and iron on the realm's palette; a manned or lit piece moves.

use super::{Ctx, paint};
use crate::render::overworld::ink::{Img, hash};

/// A sharpened stake of timber standing on row `foot` with its point at `top`, three across, lit on
/// its left.
fn stake(im: &mut Img, x: i32, top: i32, foot: i32, seed: i32) {
    for y in top + 1..=foot {
        let grain = hash(x, y, seed as u32) % 9 == 0;
        im.put(x, y, if grain { 'O' } else { 'o' });
        im.put(x + 1, y, if grain { 'b' } else { 'B' });
        im.put(x + 2, y, 'b');
    }
    // The point.
    im.put(x + 1, top, 'O');
    im.put(x, top + 1, 'O');
    im.put(x + 2, top + 1, 'B');
    im.put(x + 1, top - 1, 'o');
}

pub(super) fn palisade(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 3) as f32, 7.5, 3.0, seed);
    // Four stakes standing shoulder to shoulder, no two the same height, so a row of tiles is one wall.
    for (i, x) in [0, 4, 8, 12].into_iter().enumerate() {
        let top = 4 + (hash(i as i32, seed as i32, 91) % 5) as i32;
        stake(im, x, top, h - 4, seed as i32 + i as i32);
        im.put(x + 3, h - 6, 'k');
    }
    // Two rails bind them.
    im.rect(0, h - 15, 16, 2, 'O');
    im.rect(0, h - 14, 16, 1, 'b');
    im.rect(0, h - 9, 16, 2, 'O');
    im.rect(0, h - 8, 16, 1, 'b');
    im.rect(0, h - 4, 16, 2, 'k');
    // A torch on the middle stake: lit when the line is manned.
    if work {
        im.rect(7, 6, 2, 3, 'U');
        paint::flame(im, 8, 6, 3, t, seed);
    } else {
        im.rect(7, 6, 2, 3, 'k');
    }
    let _ = w;
}

pub(super) fn wood_gate(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 6) as f32, 15.5, 5.5, seed);
    // Two heavy posts with pointed heads, a lintel between them.
    for x in [1, 27] {
        for y in 6..h - 3 {
            im.put(x, y, 'o');
            im.put(x + 1, y, 'B');
            im.put(x + 2, y, 'B');
            im.put(x + 3, y, 'b');
        }
        im.put(x + 1, 4, 'O');
        im.put(x + 2, 4, 'B');
        im.rect(x, 5, 4, 1, 'O');
    }
    im.rect(1, 8, 30, 3, 'O');
    im.rect(1, 10, 30, 1, 'b');
    // The leaves: planks with iron straps, shut; swung open a little when carts go through.
    let gap = if work { 2 + ((paint::turn(t).cos() + 1.0) * 4.0).round() as i32 } else { 0 };
    let (lo, hi) = (5 - gap.min(4), 27 + gap.min(4));
    for (x0, x1) in [(5, 15 - gap), (17 + gap, 27)] {
        if x1 <= x0 {
            continue;
        }
        for y in 12..h - 3 {
            for x in x0..x1 {
                let seam = (x - x0) % 4 == 0;
                im.put(x, y, if seam { 'b' } else if (x - x0) % 4 == 1 { 'o' } else { 'B' });
            }
        }
        for y in [16, h - 10] {
            im.rect(x0, y, x1 - x0, 2, 'U');
            im.rect(x0, y, x1 - x0, 1, 'u');
        }
    }
    if gap > 0 {
        // The way through, dark.
        im.rect(15 - gap + 1, 12, 2 * gap - 1, h - 15, 'k');
    }
    let _ = (w, lo, hi);
}

pub(super) fn watchtower(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 8) as f32, 15.5, 7.0, seed);
    // A footing of stone, four legs of timber leaning in, cross braces.
    paint::stone(im, 4, h - 11, 24, 9, seed);
    let foot = h - 11;
    let deck = 24;
    for (x, lean) in [(5, 1), (10, 0), (19, 0), (25, -1)] {
        for y in deck..foot {
            let s = (y - deck) as f32 / (foot - deck) as f32;
            let off = (lean as f32 * (1.0 - s) * 3.0).round() as i32;
            im.put(x + off, y, 'o');
            im.put(x + 1 + off, y, 'B');
        }
    }
    for (y, x0, x1) in [(deck + 10, 7, 24), (deck + 22, 6, 25)] {
        im.line(x0, y, x1, y - 8, 'b');
        im.line(x1, y, x0, y - 8, 'b');
        im.rect(x0, y, x1 - x0, 1, 'O');
    }
    // The deck, and a palisade rail round it.
    im.rect(2, deck - 2, 28, 3, 'O');
    im.rect(2, deck, 28, 1, 'b');
    for (i, x) in (3..29).step_by(4).enumerate() {
        stake(im, x, deck - 10 - (i as i32 % 2), deck - 3, seed as i32 + i as i32);
    }
    // The roof: a steep little hip of shingles on two posts.
    im.rect(5, 6, 2, deck - 14, 'B');
    im.rect(25, 6, 2, deck - 14, 'B');
    paint::lean_roof(im, 2, 2, 28, 7, crate::render::overworld::material::ramp::TIMBER, seed);
    // A ladder up the front.
    paint::ladder(im, 15, deck, foot);
    // The archer: a silhouette at the rail when it is manned, with a cresset beside; a still, dark one when not.
    if work {
        let sway = ((paint::turn(t) * 1.0).sin() * 1.2).round() as i32;
        im.rect(13 + sway, deck - 15, 4, 7, 'H');
        im.rect(14 + sway, deck - 18, 3, 3, 'j');
        im.line(11 + sway, deck - 12, 18 + sway, deck - 12, 'o');
        im.rect(21, deck - 9, 3, 2, 'U');
        paint::flame(im, 22, deck - 9, 3, t, seed);
    } else {
        im.rect(21, deck - 9, 3, 2, 'k');
    }
    let _ = w;
}

pub(super) fn brazier(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 3) as f32, 7.0, 3.0, seed);
    // Three iron legs, a bowl of iron, a ring of coals or ash.
    im.line(4, h - 5, 6, h - 14, 'u');
    im.line(11, h - 5, 9, h - 14, 'u');
    im.line(8, h - 4, 8, h - 14, 'U');
    im.ellipse(8.0, (h - 15) as f32, 6.0, 2.4, 'U');
    im.ellipse(8.0, (h - 16) as f32, 5.0, 1.6, if work { '8' } else { 'k' });
    im.rect(2, h - 14, 12, 1, 'u');
    if work {
        paint::flame(im, 8, h - 16, 6, t, seed);
        paint::smoke(im, 8, h - 24, t, 10, 2, false);
    } else {
        paint::embers(im, 5, h - 16, 6, seed);
    }
    let _ = w;
}

// ---------------------------------------------------------- what it takes ----

/// A works on fire: flames leaping off its whole width and a column of smoke. `w` by `h`
/// pixels, the flames along the middle of the height.
pub fn blaze(w: i32, h: i32, t: u32) -> Img {
    let mut im = Img::new(w, h);
    let seed = (w * 31 + h) as u32;
    let n = (w / 7).max(2);
    for k in 0..n {
        let x = (k * 2 + 1) * w / (2 * n);
        let foot = h - 3 - (hash(k, h, 71) % (h / 4).max(2) as u32) as i32;
        paint::flame(&mut im, x, foot, 5 + (hash(k, w, 72) % 4) as i32, t + k as u32 * 3, seed ^ k as u32);
    }
    paint::smoke(&mut im, w / 2, h / 3, t, h / 2, 3, false);
    im
}

/// Cracks across a battered defence: more of them the more it has lost (`lost` of `max`).
pub fn cracks(w: i32, h: i32, lost: u32, max: u32) -> Img {
    let mut im = Img::new(w, h);
    let n = (lost * 6 / max.max(1)).clamp(1, 6) as i32;
    for k in 0..n {
        let x = 2 + (hash(k, 5, 81) % (w - 4).max(1) as u32) as i32;
        let y = h / 3 + (hash(k, 7, 82) % (h / 2).max(1) as u32) as i32;
        let (mut cx, mut cy) = (x, y);
        for s in 0..4 + n {
            im.put(cx, cy, 'k');
            cx += (hash(k, s, 83) % 3) as i32 - 1;
            cy += 1;
        }
    }
    im
}
