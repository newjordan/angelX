//! The stone trades: quarries, bankers, the tracing floor.

use super::hands::{self, Act, Pose, Trade};
use super::{Ctx, Load, paint};
use crate::render::overworld::ink::hash;
use crate::render::overworld::material::{self, ramp};

pub(super) fn field_quarry(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // The face: raw benches of grey rock, each cut a block deep, ragged on top.
    crag(im, 6, 2, 38, 16, seed);
    crag(im, 1, 13, 25, 15, seed ^ 1);
    crag(im, 27, 15, 20, 14, seed ^ 2);
    // Plug-and-feather marks where the next block will part: a row of
    // wedges along the bench's top.
    for k in 0..5 {
        let x = 30 + k * 3;
        im.put(x, 19, 'k');
        im.put(x, 18, 'V');
    }
    // Loose stone at the foot.
    paint::pile(im, 38, h - 1, 14, 9, Load::Stone, seed);
    paint::pile(im, 8, h - 1, 11, 6, Load::Rubble, seed ^ 4);
    if work {
        hands::stamp(im, 22, h - 3, Trade::Quarryman, Act::Work, t, false);
        paint::chips(im, 30, h - 14, t, 4, 'h');
        paint::dust(im, 30, h - 10, t, 'i');
    } else {
        // His pick left stuck in the bench.
        im.line(21, h - 12, 24, h - 5, 'B');
        im.line(19, h - 12, 24, h - 12, 'v');
    }
    let _ = w;
}

/// A bench of raw rock: packed stones, a ragged lit top, its right edge in shade.
fn crag(im: &mut crate::render::overworld::ink::Img, x: i32, y: i32, w: i32, h: i32, seed: u32) {
    let mut tmp = crate::render::overworld::ink::Img::new(w, h);
    material::rubble(&mut tmp, (0, 0, w, h), ramp::STONE, 5, seed);
    for yy in 0..h {
        for xx in 0..w {
            let top = (hash(xx / 3, 7, seed) % 3) as i32 + if xx < 2 || xx > w - 3 { 1 } else { 0 };
            if yy < top {
                continue;
            }
            if yy == top {
                im.put(x + xx, y + yy, 'i');
            } else if yy == top + 1 {
                im.put(x + xx, y + yy, 'h');
            } else if let Some(c) = tmp.get(xx, yy) {
                im.set(x + xx, y + yy, if xx >= w - 2 || yy >= h - 2 { material::darker(c) } else { c });
            }
        }
    }
    // A seam or two running down the face.
    for k in 0..2 {
        let sx = x + 6 + (hash(k, 3, seed) % (w as u32 - 10)) as i32;
        im.line(sx, y + 4, sx + 1, y + h - 2, 'k');
    }
}
pub(super) fn rubble_pit(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 15) as f32, 15.5, 13.0, seed);
    // The pit: a shallow oval cut, its rim of tossed stones.
    im.ellipse(12.0, (h - 15) as f32, 10.0, 6.0, 'b');
    im.ellipse(12.0, (h - 14) as f32, 8.5, 4.5, 'k');
    for k in 0..9 {
        let a = k as f32 / 9.0 * std::f32::consts::TAU;
        let (x, y) = (12.0 + a.cos() * 10.5, (h - 15) as f32 + a.sin() * 6.5);
        im.rect(x as i32, y as i32, 3, 2, if k % 2 == 0 { 'J' } else { 'G' });
    }
    // Stones in the pit's bottom.
    for k in 0..6 {
        im.put(8 + (k * 3) % 9, h - 16 + (k * 2) % 4, 'g');
    }
    paint::pile(im, 25, h - 3, 14, 11, Load::Rubble, seed);
    // A barrow, tipped.
    im.line(23, h - 3, 30, h - 3, 'B');
    im.rect(24, h - 8, 7, 4, 'o');
    im.line(24, h - 8, 30, h - 8, 'O');
    im.ellipse(27.0, (h - 2) as f32, 2.0, 2.0, 'b');
    if work {
        hands::stamp(im, 7, h - 6, Trade::Labourer, Act::Work, t, false);
        paint::chips(im, 14, h - 15, t + 2, 3, 'J');
    }
    let _ = w;
}

pub(super) fn claim_quarry(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 22) as f32, 23.0, 22.0, seed);
    // The collar: a square of dressed stone round the shaft's black mouth.
    paint::blockish(im, 11, h - 30, 26, 14, ramp::STONE, seed);
    im.rect(15, h - 27, 18, 6, 'k');
    im.rect(15, h - 27, 18, 2, 'K');
    // The headframe: two legs meeting over the shaft, a pulley at the apex.
    im.line(13, h - 27, 24, 6, 'o');
    im.line(14, h - 27, 25, 6, 'B');
    im.line(35, h - 27, 24, 6, 'o');
    im.line(34, h - 27, 25, 7, 'B');
    im.line(17, h - 38, 31, h - 38, 'O');
    paint::wheel(im, 24.0, 8.0, 3.0, if work { paint::turn(t) } else { 0.0 }, 4, false, ('U', 'V', 'u'));
    // The bucket on its rope: down into the dark and up with rock.
    let up = if work { ((paint::turn(t).cos() + 1.0) * 6.0).round() as i32 } else { 5 };
    let by = h - 38 + up;
    paint::rope(im, 24, 9, 24, by, 0.0, 't');
    im.rect(21, by, 7, 5, 'r');
    im.rect(21, by, 7, 1, 'O');
    im.rect(22, by - 1, 5, 1, 'J');
    // The windlass drum beside, its crank turned by a hand.
    im.rect(38, h - 24, 7, 4, 'o');
    paint::rope(im, 41, h - 24, 27, 10, 0.0, 't');
    if work {
        hands::stamp(im, 42, h - 3, Trade::Quarryman, Act::Bare(Pose::Slash), t, true);
    }
    // The charter banner on its pole, and rough blocks stacked for the cart.
    paint::banner(im, 4, 20, h - 3, t * u32::from(work), 'Q', 'T');
    paint::pile(im, 40, h - 1, 14, 9, Load::Stone, seed);
    paint::pile(im, 12, h - 1, 12, 7, Load::Rubble, seed ^ 3);
    let _ = w;
}

pub(super) fn banker(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // The shelter: four posts and a shingled lean-to over the bench.
    for x in [4, 38] {
        paint::post(im, x, 12, h - 4);
    }
    paint::post(im, 4, 12, h - 4);
    paint::lean_roof(im, 1, 3, 44, 11, ramp::TIMBER, seed);
    // Templates hung on the back post: boards cut to a curve.
    for k in 0..3 {
        im.rect(6 + k * 4, 16, 3, 8 + k * 2, 'O');
        im.put(7 + k * 4, 16, 'B');
    }
    // The bench: a heavy stone slab on two piers, a block half dressed on it.
    paint::blockish(im, 10, h - 20, 26, 10, ramp::STONE, seed);
    paint::blockish(im, 15, h - 28, 14, 9, ramp::SLATE, seed ^ 1);
    // Its dressed face and the rough, a line of drafting down it.
    im.rect(15, h - 25, 6, 5, 'i');
    im.line(21, h - 25, 21, h - 20, 'k');
    if work {
        hands::stamp(im, 12, h - 3, Trade::BankerMason, Act::Work, t, false);
        paint::chips(im, 20, h - 28, t, 4, 'H');
        paint::dust(im, 22, h - 28, t, 'i');
    } else {
        im.rect(30, h - 26, 4, 3, 'B');
        im.line(30, h - 27, 33, h - 27, 'v');
    }
    // Rough blocks in, dressed ashlar out.
    paint::pile(im, 40, h - 1, 14, 9, Load::Ashlar, seed);
    paint::pile(im, 6, h - 1, 11, 6, Load::Stone, seed ^ 2);
    let _ = w;
}

pub(super) fn tracing_floor(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // The floor: lime plaster laid on a raised bed, pale, edged in stone.
    let (fx, fy, fw, fh) = (2, h - 38, w - 4, 34);
    for y in fy..fy + fh {
        for x in fx..fx + fw {
            let n = hash(x / 2, y / 2, seed) % 7;
            im.put(x, y, if n == 0 { 'h' } else if n < 3 { 'i' } else { 'H' });
        }
    }
    for x in fx..fx + fw {
        im.put(x, fy, 'G');
        im.put(x, fy + fh - 1, 'g');
        im.put(x, fy + fh, 'k');
    }
    for y in fy..fy + fh {
        im.put(fx, y, 'J');
        im.put(fx + fw - 1, y, 'g');
    }
    // The scribed constructions: rings, an arch's arcs, a chevron's zigzag.
    let (cx, cy) = (40.0f32, (fy + 16) as f32);
    paint::ring(im, cx, cy, 12.0, 'G');
    paint::ring(im, cx, cy, 7.0, 'G');
    paint::ring(im, cx, cy, 3.0, 'J');
    im.line(26, fy + 16, 54, fy + 16, 'J');
    im.line(40, fy + 3, 40, fy + 29, 'J');
    for k in 0..6 {
        let x = 8 + k * 3;
        im.line(x, fy + 8 + (k % 2) * 6, x + 3, fy + 14 - (k % 2) * 6, 'G');
    }
    for r in [8.0f32, 13.0] {
        for k in 0..20 {
            let a = std::f32::consts::PI + k as f32 / 20.0 * std::f32::consts::PI;
            im.put((17.0 + a.cos() * r) as i32, fy + 24 + (a.sin() * r) as i32, 'g');
        }
    }
    // The compass at work: one leg on the ring's centre, the other sweeping
    // round it, and a heavy new arc laid behind it as it goes.
    let sweep = if work { paint::turn(t) } else { 2.4 };
    let r = 17.0;
    if work {
        for k in 0..=t * 4 {
            let a = k as f32 / 32.0 * std::f32::consts::TAU;
            im.put((cx + a.cos() * r) as i32, (cy + a.sin() * r * 0.8) as i32, 'k');
            im.put((cx + a.cos() * (r - 1.0)) as i32, (cy + a.sin() * (r - 1.0) * 0.8) as i32, 'g');
        }
    }
    let tip = (cx + sweep.cos() * r, cy + sweep.sin() * r * 0.8);
    im.line(cx as i32, cy as i32, tip.0 as i32, tip.1 as i32, 'W');
    im.line(cx as i32 + 1, cy as i32, tip.0 as i32 + 1, tip.1 as i32, 'V');
    im.line(cx as i32, cy as i32, cx as i32, cy as i32 - 12, 'U');
    im.line(cx as i32, cy as i32 - 12, tip.0 as i32, tip.1 as i32, 'U');
    im.rect(cx as i32 - 1, cy as i32 - 14, 3, 3, 'O');
    if work {
        hands::stamp(im, 56, fy + 31, Trade::MasterMason, Act::Bare(Pose::Slash), t, true);
    }
    // The trestle table with its parchment, and the template boards leaning.
    im.rect(w - 14, h - 22, 12, 2, 'O');
    im.line(w - 13, h - 20, w - 13, h - 12, 'B');
    im.line(w - 4, h - 20, w - 4, h - 12, 'B');
    im.rect(w - 12, h - 24, 8, 2, 'H');
    for k in 0..3 {
        im.rect(w - 24 + k * 3, h - 18, 2, 9 + k, 'o');
    }
}
pub(super) fn slater(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 11) as f32, 15.5, 10.0, seed);
    // A lean-to shed roofed in the slate it makes, the roof half covered.
    paint::post(im, 3, 14, h - 3);
    paint::post(im, 27, 14, h - 3);
    paint::lean_roof(im, 0, 3, 32, 13, ramp::SLATE, seed);
    for k in 0..7 {
        im.line(2 + k * 4, 12, 4 + k * 4, 15, 'x');
    }
    // The splitting bench: a thick slab on a stump, the slater's froe in it.
    im.rect(9, h - 17, 8, 12, 'B');
    im.rect(9, h - 17, 8, 2, 'O');
    paint::blockish(im, 7, h - 24, 14, 8, ramp::SLATE, seed);
    for k in 0..3 {
        im.line(9 + k * 4, h - 24 + 3, 9 + k * 4, h - 18, 'x');
    }
    if work {
        hands::stamp(im, 6, h - 4, Trade::BankerMason, Act::Work, t, false);
        paint::chips(im, 14, h - 24, t, 4, 'v');
    }
    // Stacks of split slate, and the roofs they will go on.
    paint::pile(im, 25, h - 1, 11, 8, Load::Slate, seed);
    for k in 0..3 {
        im.line(18 + k, h - 6, 23 + k, h - 22, 'u');
    }
    let _ = w;
}

pub(super) fn mason_yard(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // The yard's fence along the back, a gate gap, rows of dressed blocks.
    material::rail_fence(im, (1, 46), h - 26, &[h - 33, h - 30], 12, ('r', 'B'), seed);
    for (i, (x, y)) in [(4, h - 22), (18, h - 22), (32, h - 22), (6, h - 12), (20, h - 12)].into_iter().enumerate() {
        paint::blockish(im, x, y, 13, 9, if i % 2 == 0 { ramp::STONE } else { ramp::SLATE }, seed ^ i as u32);
    }
    // A carved block on trestles: the chevron cut across its face.
    im.rect(34, h - 12, 10, 9, 'i');
    for k in 0..4 {
        im.line(34 + k * 3, h - 12 + (k % 2) * 4, 36 + k * 3, h - 8 + (k % 2) * -4, 'g');
    }
    im.line(33, h - 3, 33, h - 1, 'B');
    im.line(44, h - 3, 44, h - 1, 'B');
    // Sheer legs: two poles lashed at the top, a rope and a block hung from them.
    im.line(41, h - 25, 36, h - 33, 'o');
    im.line(44, h - 25, 39, h - 33, 'o');
    paint::rope(im, 37, h - 33, 37, h - 27 + if work { (paint::turn(t).sin() * 1.5).round() as i32 } else { 0 }, 0.0, 't');
    if work {
        hands::stamp(im, 30, h - 3, Trade::BankerMason, Act::Work, t, true);
        paint::chips(im, 36, h - 11, t, 3, 'H');
    }
    let _ = w;
}
