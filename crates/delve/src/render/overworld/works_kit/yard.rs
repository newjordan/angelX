//! Open ground works: pits, ponds, pasture, the tanyard, the ditch.

use super::hands::{self, Act, Pose, Trade};
use super::{Ctx, Load, paint};
use crate::render::overworld::ink::{Img, hash};
use crate::render::overworld::material;

/// A timber-lined pit: boards round a rectangle, the inside by `fill`.
fn lined_pit(im: &mut Img, x: i32, y: i32, w: i32, h: i32, fill: &dyn Fn(i32, i32) -> char) {
    for yy in y..y + h {
        for xx in x..x + w {
            im.put(xx, yy, fill(xx, yy));
        }
    }
    for xx in x - 1..x + w + 1 {
        im.put(xx, y - 1, 'O');
        im.put(xx, y - 2, 'o');
        im.put(xx, y + h, 'B');
        im.put(xx, y + h + 1, 'b');
    }
    for yy in y - 1..y + h + 1 {
        im.put(x - 1, yy, 'o');
        im.put(x + w, yy, 'B');
    }
}

pub(super) fn slaking_pit(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 12) as f32, 15.5, 11.0, seed);
    // Lime putty in a boarded pit: milk-white, with a skin; boiling when the water goes on.
    lined_pit(im, 4, h - 22, 23, 14, &|x, y| {
        let n = hash(x, y, seed) % 9;
        let boil = work && ((x * 5 + y * 3 + t as i32 * 2) % 11 == 0);
        if boil { 'W' } else if n == 0 { 'h' } else if n < 4 { 'i' } else { 'H' }
    });
    if work {
        for k in 0..5 {
            let (x, y) = (7 + k * 4, h - 18 + (k * 3 + t as i32) % 8);
            im.ellipse(x as f32, y as f32, 1.5, 1.2, 'w');
        }
        paint::smoke(im, 10, h - 22, t, 14, 3, true);
        paint::smoke(im, 21, h - 22, t + 4, 14, 3, true);
        hands::stamp(im, 6, h - 3, Trade::LimeBurner, Act::Work, t, false);
    } else {
        im.line(6, h - 17, 22, h - 17, 'G');
    }
    // A bucket of water and a sack of quicklime by the lip.
    paint::bucket(im, 24, h - 7, 'z');
    paint::pile(im, 27, h - 1, 8, 7, Load::Lime, seed);
    let _ = w;
}

pub(super) fn sand_pit(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 13) as f32, 15.5, 12.0, seed);
    // A bright cut in the bank: sand, its face banded.
    for y in h - 28..h - 10 {
        for x in 3..25 {
            let n = hash(x, y / 2, seed) % 6;
            let band = (y - (h - 28)) / 4 % 2;
            let (dx, dy) = ((x as f32 - 14.0) / 12.0, (y as f32 - (h - 14) as f32) / 10.0);
            if dx * dx + dy * dy < 1.0 {
                im.put(x, y, if n == 0 { 'B' } else if band == 0 { 'O' } else { 't' });
            }
        }
    }
    im.ellipse(14.0, (h - 12) as f32, 9.0, 3.0, 'P');
    im.ellipse(14.0, (h - 12) as f32, 8.0, 2.0, 'o');
    paint::pile(im, 26, h - 2, 12, 10, Load::Sand, seed);
    // A barrow with a load.
    im.line(2, h - 3, 9, h - 3, 'B');
    im.rect(3, h - 7, 6, 4, 'o');
    im.rect(3, h - 8, 6, 2, 'T');
    im.ellipse(5.0, (h - 2) as f32, 2.0, 2.0, 'b');
    if work {
        hands::stamp(im, 16, h - 5, Trade::Labourer, Act::Work, t, false);
        paint::dust(im, 20, h - 14, t, 't');
    } else {
        im.line(14, h - 14, 14, h - 5, 'B');
        im.rect(12, h - 15, 5, 2, 'v');
    }
    let _ = w;
}

pub(super) fn clay_pit(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 13) as f32, 15.5, 12.0, seed);
    // A wet red-brown bank, a standing puddle in the cut, sliced faces.
    for y in h - 28..h - 9 {
        for x in 3..26 {
            let (dx, dy) = ((x as f32 - 14.0) / 12.5, (y as f32 - (h - 15) as f32) / 10.5);
            if dx * dx + dy * dy < 1.0 {
                let n = hash(x / 2, y / 2, seed) % 6;
                im.put(x, y, if n == 0 { 'O' } else if n < 3 { 'R' } else { 'r' });
            }
        }
    }
    im.ellipse(13.0, (h - 11) as f32, 7.0, 2.5, 'S');
    im.ellipse(12.0, (h - 11) as f32, 5.0, 1.5, if work && t % 4 < 2 { 'z' } else { 'Q' });
    for k in 0..3 {
        im.line(5 + k * 5, h - 22 + k, 8 + k * 5, h - 16 + k, 'p');
    }
    paint::pile(im, 27, h - 2, 10, 8, Load::Clay, seed);
    if work {
        hands::stamp(im, 8, h - 4, Trade::Labourer, Act::Work, t, false);
    } else {
        im.line(8, h - 15, 8, h - 6, 'B');
        im.rect(6, h - 16, 5, 2, 'v');
    }
    let _ = w;
}

pub(super) fn retting_pond(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    paint::pond(im, 22.0, (h - 14) as f32, 18.0, 9.0, if work { t } else { 0 }, seed);
    // Sheaves of hemp sunk in it under stones: pale-green butts breaking the surface.
    for (x, y) in [(12, h - 18), (20, h - 12), (28, h - 17), (16, h - 8)] {
        im.rect(x, y, 6, 3, 'M');
        im.rect(x, y, 6, 1, 'y');
        im.rect(x + 1, y + 3, 4, 1, 'N');
        im.put(x + 2, y + 1, 'J');
        im.put(x + 3, y + 1, 'G');
    }
    // Bubbles rising off the rot.
    if work {
        for k in 0..5 {
            let age = ((t + k * 2) % 8) as i32;
            im.put(12 + k as i32 * 5, h - 12 - age, 'W');
        }
    }
    // Hemp stooked to dry on a rack, and the retter.
    for k in 0..4 {
        im.line(38 + k * 2, h - 5, 40 + k * 2, h - 18, 'N');
        im.line(40 + k * 2, h - 5, 40 + k * 2, h - 18, 'A');
    }
    im.line(37, h - 6, 47, h - 6, 'o');
    paint::pile(im, 6, h - 1, 11, 7, Load::Hemp, seed);
    if work {
        hands::stamp(im, 5, h - 6, Trade::Roper, Act::Bare(Pose::Thrust), t, false);
    }
    let _ = w;
}

/// A cow, side on, facing right: 15 by 10, `bob` its head dipping to graze.
fn cow(im: &mut Img, x: i32, foot: i32, bob: i32, hide: [char; 3]) {
    let [lit, mid, dark] = hide;
    im.rect(x, foot - 8, 11, 5, mid);
    im.rect(x + 1, foot - 9, 9, 1, lit);
    im.rect(x + 1, foot - 3, 9, 1, dark);
    for (lx, ch) in [(x + 1, dark), (x + 3, mid), (x + 7, dark), (x + 9, mid)] {
        im.rect(lx, foot - 3, 1, 3, ch);
    }
    // The head, down at the grass by turns.
    im.rect(x + 11, foot - 9 + bob, 3, 4, lit);
    im.put(x + 13, foot - 9 + bob, 'k');
    im.put(x + 12, foot - 10 + bob, 'T');
    im.put(x + 11, foot - 10 + bob, 'T');
    im.put(x - 1, foot - 7, dark);
    im.put(x - 1, foot - 6, dark);
    im.put(x + 4, foot - 7, 'H');
    im.put(x + 8, foot - 6, 'H');
}

/// A sheep, 8 wide: fleece and a dark face.
fn sheep(im: &mut Img, x: i32, foot: i32, bob: i32) {
    im.ellipse(x as f32 + 3.5, (foot - 4) as f32, 3.8, 2.8, 'H');
    im.ellipse(x as f32 + 3.0, (foot - 5) as f32, 2.5, 1.5, 'i');
    im.rect(x + 1, foot - 2, 1, 2, 'k');
    im.rect(x + 5, foot - 2, 1, 2, 'k');
    im.rect(x + 7, foot - 5 + bob, 2, 2, 'g');
    im.put(x + 8, foot - 5 + bob, 'k');
}

pub(super) fn pasture(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // Grazed turf: lighter tufts on the paddock, a worn path to the gate.
    for k in 0..40 {
        let (x, y) = (4 + (k * 13) % (w - 8), h - 40 + (k * 17) % 36);
        im.put(x, y, 'm');
        im.put(x + 1, y - 1, 'l');
    }
    // The fence round three sides, open at the front.
    material::rail_fence(im, (1, w - 3), h - 38, &[h - 45, h - 42], 12, ('r', 'B'), seed);
    material::rail_fence(im, (1, 20), h - 3, &[h - 10, h - 7], 12, ('r', 'B'), seed ^ 1);
    material::rail_fence(im, (w - 20, w - 3), h - 3, &[h - 10, h - 7], 12, ('r', 'B'), seed ^ 2);
    for y in [h - 36, h - 28, h - 20] {
        im.put(1, y, 'o');
        im.put(w - 3, y, 'o');
        im.rect(1, y, 2, 6, 'o');
        im.rect(w - 4, y, 2, 6, 'o');
    }
    // Stock: two cows and a few sheep, grazing in turn.
    let step = (paint::turn(t) * 1.0).sin();
    let move_x = if work { (step * 3.0).round() as i32 } else { 0 };
    cow(im, 8 + move_x, h - 24, if work && (t / 2) % 2 == 0 { 2 } else { 0 }, ['O', 'o', 'B']);
    cow(im, 36 - move_x, h - 14, if work && (t / 2) % 2 == 1 { 2 } else { 0 }, ['T', 'O', 'o']);
    for (k, (x, y)) in [(24, h - 30), (44, h - 24), (50, h - 12), (16, h - 11)].into_iter().enumerate() {
        sheep(im, x + if work && k % 2 == 0 { move_x } else { 0 }, y, if work && (t + k as u32) % 4 < 2 { 1 } else { 0 });
    }
    // A trough, a rick of hay, the herder and his crook.
    im.rect(51, h - 36, 10, 3, 'o');
    im.rect(52, h - 36, 8, 1, 'z');
    im.rect(51, h - 33, 2, 2, 'B');
    im.rect(59, h - 33, 2, 2, 'B');
    paint::pile(im, 56, h - 22, 11, 9, Load::Grain, seed);
    if work {
        hands::stamp(im, 30, h - 4, Trade::Herder, Act::Walk, t, false);
    } else {
        hands::stamp(im, 30, h - 4, Trade::Herder, Act::Idle, 0, false);
    }
}

pub(super) fn tanyard(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 12) as f32, 23.0, 11.5, seed);
    // Two rows of tan pits, timber-edged, dark with bark liquor.
    for (i, (x, y)) in [(3, h - 24), (19, h - 24), (3, h - 13), (19, h - 13)].into_iter().enumerate() {
        lined_pit(im, x, y, 12, 6, &|xx, yy| {
            let n = hash(xx, yy, seed ^ i as u32) % 8;
            let bubble = work && (xx * 3 + yy * 5 + t as i32 * 2 + i as i32 * 3) % 13 == 0;
            if bubble { 'O' } else if n == 0 { 'P' } else if n < 3 { 'b' } else { 'I' }
        });
    }
    // Drying racks: hides stretched taut on frames, tan and pale.
    for (k, x) in [35, 42].into_iter().enumerate() {
        im.rect(x, h - 38, 1, 22, 'B');
        im.rect(x + 6, h - 38, 1, 22, 'B');
        im.rect(x, h - 38, 7, 1, 'O');
        im.rect(x + 1, h - 36, 5, 15, if k == 0 { 'O' } else { 't' });
        im.rect(x + 1, h - 36, 5, 1, 'T');
        im.line(x + 2, h - 30, x + 4, h - 26, 'o');
    }
    // The scraping beam: a half-round log, a hide over it, the fleshing knife.
    im.rect(34, h - 12, 12, 3, 'B');
    im.rect(34, h - 12, 12, 1, 'o');
    im.rect(35, h - 10, 2, 8, 'B');
    im.rect(44, h - 10, 2, 8, 'B');
    paint::pile(im, 8, h - 1, 10, 7, Load::Hides, seed);
    paint::pile(im, 24, h - 1, 10, 6, Load::Timber, seed ^ 3);
    if work {
        hands::stamp(im, 40, h - 3, Trade::Tanner, Act::Work, t, true);
    }
    let _ = w;
}

pub(super) fn ditch_gang(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 13) as f32, 23.0, 12.0, seed);
    // The cut: a dark trench across the whole width, its sides stepped.
    for y in h - 24..h - 15 {
        for x in 1..w - 1 {
            let depth = y - (h - 24);
            im.put(x, y, if depth < 2 { 'B' } else if depth < 4 { 'b' } else if (x + y) % 5 == 0 { 'X' } else { 'k' });
        }
    }
    im.line(1, h - 24, w - 2, h - 24, 'O');
    // Water seeping in at the trench's foot.
    for x in (4..w - 4).step_by(7) {
        im.put(x, h - 16, 'u');
        im.put(x + 1, h - 16, 'S');
    }
    // Spoil heaped on the bank in front, a peg-and-line along the edge.
    paint::spoil(im, 10, h - 3, 9.0, 7.0, seed);
    paint::spoil(im, 33, h - 3, 10.0, 6.0, seed ^ 2);
    for x in [3, 24, 44] {
        paint::post(im, x, h - 30, h - 24);
    }
    im.line(3, h - 29, 44, h - 29, 't');
    if work {
        let throw = |im: &mut Img, x: i32, k: u32| {
            let s = ((t + k) % 8) as f32;
            for j in 0..3 {
                let a = s - j as f32;
                if (0.0..6.0).contains(&a) {
                    im.put(x + (a * 2.2) as i32, h - 22 - (a * (6.0 - a) * 0.6) as i32, 'B');
                }
            }
        };
        hands::stamp(im, 14, h - 14, Trade::Labourer, Act::Work, t, false);
        hands::stamp(im, 32, h - 14, Trade::Labourer, Act::Work, t + 4, false);
        throw(im, 18, 0);
        throw(im, 36, 4);
    } else {
        im.line(22, h - 14, 22, h - 6, 'B');
        im.rect(20, h - 15, 5, 2, 'v');
    }
}
