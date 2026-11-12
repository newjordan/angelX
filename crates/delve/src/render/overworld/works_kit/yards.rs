//! The three yards the hillside has no trade for: the carter's yard (the
//! road carts and their oxen), the castle's stockyard (dressed stone,
//! timber and mortar waiting in the Keep's yard) and the founding camp, one
//! pavilion and one fire at the realm's first tile.

use super::hands::{self, Act, Trade};
use super::{Ctx, Load, paint};
use crate::render::overworld::ink::{Img, hash};
use crate::render::overworld::material::{self, ramp};

// ---------------------------------------------------------- the carter's ----

/// How an ox stands in its stall.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stance {
    /// Head up, tail swishing.
    Stand,
    /// Head down in the hay.
    Graze,
    /// Folded down on the straw.
    Lie,
}

/// An ox facing right, 14 wide, its hooves on row `foot` and the tail's tip
/// on column `x`; `left` turns it about. `hide` is its lit, field and shade
/// and `spots` the dark patches of its coat; `tail` is 0 hanging, 1 swung.
fn ox(im: &mut Img, x: i32, foot: i32, stance: Stance, hide: [char; 3], spots: char, left: bool, tail: i32) {
    let [lit, mid, dark] = hide;
    let mut o = Img::new(14, 15);
    let f = 14;
    if stance == Stance::Lie {
        // Folded down: a heap of hide on the straw, one fore-leg tucked out in front.
        o.rect(1, f - 8, 9, 7, mid);
        o.rect(2, f - 9, 7, 1, lit);
        o.rect(4, f - 10, 3, 1, lit);
        o.rect(1, f - 3, 9, 2, dark);
        o.rect(3, f - 7, 3, 2, lit);
        o.put(6, f - 5, spots);
        o.put(7, f - 6, spots);
        o.rect(8, f - 3, 4, 2, mid);
        o.rect(11, f - 3, 1, 2, 'k');
        o.clear(1, f - 8);
        // The neck up, the head at the straw's height, chewing.
        o.rect(9, f - 11, 2, 7, mid);
        o.rect(10, f - 12, 4, 4, mid);
        o.rect(10, f - 12, 3, 1, lit);
        o.rect(12, f - 10, 2, 2, '$');
        o.put(13, f - 10, 'k');
        o.put(11, f - 11, 'k');
        o.put(11, f - 13, 'H');
        o.put(11, f - 14, 'H');
        o.put(12, f - 15 + 1, 'H');
        o.put(13, f - 15 + 1, 'H');
        o.line(0, f - 6, 0, f - 3, dark);
    } else {
        let down = if stance == Stance::Graze { 4 } else { 0 };
        // Four legs, the far ones in shade; hooves black.
        o.rect(2, f - 4, 2, 4, mid);
        o.rect(4, f - 4, 1, 4, dark);
        o.rect(7, f - 4, 1, 4, dark);
        o.rect(8, f - 4, 2, 4, mid);
        for hx in [2, 3, 4, 7, 8, 9] {
            o.put(hx, f, 'k');
        }
        // The barrel of the body, a hump over the shoulders, a shaded belly.
        o.rect(2, f - 9, 8, 5, mid);
        o.rect(3, f - 10, 7, 1, lit);
        o.rect(5, f - 11, 3, 1, lit);
        o.rect(2, f - 5, 8, 1, dark);
        o.rect(3, f - 9, 3, 2, lit);
        o.put(6, f - 7, spots);
        o.put(7, f - 6, spots);
        o.put(4, f - 6, spots);
        o.clear(2, f - 9);
        // The neck and head: up, or down to the hay; the dewlap swinging under it.
        o.rect(9, f - 10, 2, 6 - down / 4, mid);
        o.rect(9, f - 10, 2, 1, lit);
        if down == 0 {
            o.rect(9, f - 5, 2, 1, dark);
        }
        o.rect(10, f - 10 + down, 4, 4, mid);
        o.rect(10, f - 10 + down, 3, 1, lit);
        o.rect(12, f - 8 + down, 2, 2, '$');
        o.put(13, f - 8 + down, 'k');
        o.put(11, f - 9 + down, 'k');
        if down > 0 {
            // The hay under its nose.
            o.put(12, f - 1, 'T');
            o.put(13, f - 1, 't');
            o.put(11, f - 1, 't');
            o.put(12, f - 2, 'T');
        } else {
            o.put(11, f - 6, dark);
        }
        // The horn rises from the poll and curves out ahead.
        o.put(11, f - 11 + down, 'H');
        o.put(11, f - 12 + down, 'H');
        o.put(12, f - 13 + down, 'H');
        o.put(13, f - 13 + down, 'H');
        // The tail: a rope of hair, kicked out by turns.
        if tail == 0 {
            o.line(1, f - 9, 1, f - 5, dark);
            o.put(1, f - 4, 'k');
        } else {
            o.line(1, f - 9, 0, f - 8, dark);
            o.line(0, f - 8, 0, f - 5, dark);
            o.put(0, f - 4, 'k');
        }
    }
    let o = if left { o.flip_h() } else { o };
    im.stamp(&o, x, foot - f);
}

/// A collar hung on its peg: a padded ring of leather, its hole dark.
fn collar(im: &mut Img, x: i32, y: i32) {
    im.line(x + 3, y - 1, x + 3, y, 'O');
    im.ellipse(x as f32 + 3.0, y as f32 + 4.5, 3.4, 4.0, 'B');
    im.ellipse(x as f32 + 3.0, y as f32 + 4.5, 2.6, 3.2, 'O');
    im.ellipse(x as f32 + 3.0, y as f32 + 5.0, 1.4, 1.9, 'K');
    im.put(x + 1, y + 2, 'T');
    im.put(x + 2, y + 1, 'T');
    im.line(x + 5, y + 6, x + 5, y + 8, 'b');
}

/// A yoke on its peg: a beam with two bows hanging from it, 7 wide.
fn yoke(im: &mut Img, x: i32, y: i32) {
    im.rect(x, y, 7, 2, 'O');
    im.rect(x, y, 7, 1, 'T');
    im.rect(x + 6, y + 1, 1, 1, 'o');
    for bx in [x + 1, x + 4] {
        im.line(bx, y + 2, bx, y + 6, 'o');
        im.line(bx + 1, y + 2, bx + 1, y + 6, 'B');
        im.put(bx, y + 7, 'B');
        im.put(bx + 1, y + 7, 'B');
    }
}

/// A cart standing on its wheel with its pole propped on a fork, 31 wide,
/// the wheel's foot on row `foot`.
fn cart(im: &mut Img, x: i32, foot: i32, loaded: bool) {
    // The pole runs forward from under the bed to a forked prop.
    im.line(x + 17, foot - 11, x + 30, foot - 13, 'O');
    im.line(x + 17, foot - 10, x + 30, foot - 12, 'o');
    im.line(x + 28, foot - 12, x + 28, foot, 'O');
    im.line(x + 29, foot - 12, x + 29, foot, 'o');
    im.put(x + 27, foot - 13, 'O');
    im.put(x + 30, foot - 14, 't');
    // The bed: a top rail, slatted sides, a floor plank, the tailboard.
    im.rect(x, foot - 18, 21, 8, 'o');
    im.rect(x, foot - 18, 21, 1, 'O');
    for k in 0..7 {
        im.line(x + 1 + k * 3, foot - 17, x + 1 + k * 3, foot - 13, 'B');
    }
    im.rect(x, foot - 12, 21, 2, 'b');
    im.rect(x, foot - 12, 21, 1, 'B');
    im.rect(x + 20, foot - 18, 1, 8, 'B');
    if loaded {
        // A crate and a sack standing in it, over the rail.
        material::crate_box(im, x + 6, foot - 17, 3);
        paint::sack(im, x + 12, foot - 17, ['T', 't', 'O']);
    }
    // The great wheel: a wooden felloe, spokes, an iron tyre and hub.
    let (cx, cy) = ((x + 10) as f32, (foot - 6) as f32);
    paint::wheel(im, cx, cy, 6.0, 0.3, 8, false, ('B', 'O', 'U'));
    paint::ring(im, cx, cy, 6.0, 'u');
    im.put(x + 10, foot - 6, 'V');
}

/// A hanging lantern, 3 by 5: a chain over a caged glass, lit (a flame in
/// it) or cold.
fn lantern(im: &mut Img, x: i32, y: i32, lit: bool, t: u32) {
    im.line(x + 1, y - 3, x + 1, y - 1, 'U');
    im.rect(x, y, 3, 1, 'u');
    im.rect(x, y + 1, 3, 3, if lit { '5' } else { 'S' });
    im.rect(x, y + 4, 3, 1, 'u');
    im.line(x, y + 1, x, y + 3, 'u');
    im.line(x + 2, y + 1, x + 2, y + 3, 'u');
    if lit {
        im.put(x + 1, y + 2, '6');
        im.put(x + 1, y + 1, if t % 2 == 0 { '6' } else { '5' });
    }
}

/// A wheel painted on a sign board, 5 by 5: a rim round a hub.
fn wheel_mark(im: &mut Img, x: i32, y: i32) {
    for (r, row) in [".BBB.", "B...B", "B.b.B", "B...B", ".BBB."].iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch != '.' {
                im.put(x + c as i32, y + r as i32, ch);
            }
        }
    }
}

pub(super) fn carters_yard(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // Trodden ground with the grit of wheel-ruts, the long range of stalls at the back.
    paint::earth(im, 24.0, (h - 13) as f32, 23.5, 12.5, seed);
    for k in 0..26 {
        im.put(3 + (k * 11) % 42, h - 22 + (k * 7) % 20, if k % 3 == 0 { 'h' } else { 'G' });
    }
    // The range's back wall in shade, boarded; the straw on its floor.
    let floor = h - 23;
    im.rect(0, 12, w, floor - 10, 'K');
    for x in (5..w - 3).step_by(6) {
        im.line(x, 13, x, floor, 'X');
    }
    for x in 2..w - 2 {
        let n = hash(x, 3, seed) % 5;
        im.put(x, floor, if n == 0 { 'b' } else if n < 3 { 't' } else { 'O' });
        if n < 2 {
            im.put(x, floor - 1, 'T');
        }
    }
    // Posts between the stalls, the tie beam, a roof of oak shingles over all.
    for x in [0, 16, 32, 46] {
        paint::post(im, x, 12, floor + 1);
    }
    paint::beam(im, 0, w - 1, 10);
    paint::lean_roof(im, 0, 1, w, 10, ramp::TIMBER, seed);
    // A hay bundle on a peg over each ox's rump.
    for x in [4, 20] {
        im.put(x + 1, 13, 'o');
        for k in 0..6 {
            for r in 0..4 {
                if hash(x + k, r, seed) % 4 != 0 && !(r == 3 && (k == 0 || k == 5)) {
                    im.put(x + k, 14 + r, if (k + r) % 3 == 0 { 't' } else { 'T' });
                }
            }
        }
    }
    // Two oxen in their stalls. At rest one is folded down on the straw and
    // one has its head in the hay; at work they stand, tails swishing.
    if work {
        ox(im, 2, floor, Stance::Stand, ['o', 'r', 'B'], 'B', false, (t / 2 % 2) as i32);
        ox(im, 18, floor, Stance::Stand, ['H', 'i', 'h'], 'g', false, ((t + 2) / 2 % 2) as i32);
    } else {
        ox(im, 2, floor, Stance::Lie, ['o', 'r', 'B'], 'B', false, 0);
        ox(im, 18, floor, Stance::Graze, ['H', 'i', 'h'], 'g', false, 0);
    }
    // The tack stall: a collar and a yoke on pegs, the sign hung over them.
    im.rect(35, 20, 10, 1, 'B');
    for x in [37, 42] {
        im.put(x, 20, 'O');
    }
    collar(im, 34, 21);
    yoke(im, 39, 21);
    let sway = if work && (t / 2) % 2 == 0 { 1 } else { 0 };
    im.line(36, 12, 36, 14, 'U');
    im.line(43, 12, 43, 14, 'U');
    im.rect(34 + sway, 14, 11, 7, 'B');
    im.rect(35 + sway, 15, 9, 5, 'T');
    im.rect(35 + sway, 15, 9, 1, 'W');
    wheel_mark(im, 37 + sway, 15);
    // The lantern on its chain at the stall post.
    lantern(im, 14, 15, work, t);
    // The cart standing in the yard, goods in it now and then while it loads.
    cart(im, 1, h - 1, work && (t / 4) % 2 == 1);
    // Spare wheels leaning on a low rail against the stall post, the trough before them.
    im.rect(33, h - 8, 15, 2, 'B');
    im.rect(33, h - 8, 15, 1, 'o');
    paint::wheel(im, 37.0, (h - 12) as f32, 5.0, 0.5, 8, false, ('B', 'O', 'U'));
    paint::ring(im, 37.0, (h - 12) as f32, 5.0, 'u');
    paint::wheel(im, 43.0, (h - 11) as f32, 5.0, 0.0, 8, false, ('B', 'O', 'U'));
    paint::ring(im, 43.0, (h - 11) as f32, 5.0, 'u');
    // The water trough before it, the water glinting.
    im.rect(32, h - 6, 16, 6, 'o');
    im.rect(32, h - 6, 16, 1, 'O');
    im.rect(33, h - 5, 14, 2, 'Q');
    for k in 0..14 {
        if (k + 3 * (t as i32 % 8 / 2)) % 5 == 0 {
            im.put(33 + k, h - 5, 'z');
        }
    }
    im.rect(32, h - 1, 16, 1, 'B');
    im.rect(32, h - 6, 1, 6, 'B');
    im.rect(47, h - 6, 1, 6, 'B');
    // The carter, hauling a crate along the yard from the trough to the cart.
    if work {
        let phase = paint::turn(t);
        let x = 26 + (phase.sin() * 6.0).round() as i32;
        hands::stamp(im, x, h - 3, Trade::Carter, Act::Carry(Load::Crate), t, phase.cos() < 0.0);
    }
}

// --------------------------------------------------------- the stockyard ----

/// A stack of dressed stone: `courses` courses of ashlar, joints staggered,
/// a pale top slab over them and the right face in shade; `w` wide.
fn course_stack(im: &mut Img, x: i32, foot: i32, w: i32, courses: i32) {
    for c in 0..courses {
        let y = foot - (c + 1) * 3;
        let off = if c % 2 == 0 { 0 } else { 4 };
        for dx in 0..w {
            let joint = (dx + off) % 8 == 7;
            let ink = if dx == w - 1 { 'G' } else if joint { 'J' } else { 'i' };
            im.put(x + dx, y, if dx == w - 1 { 'J' } else { 'H' });
            im.put(x + dx, y + 1, ink);
            im.put(x + dx, y + 2, if dx == w - 1 { 'g' } else if joint { 'G' } else { 'h' });
        }
    }
    // The top slab.
    let top = foot - courses * 3 - 3;
    im.rect(x, top, w, 3, 'H');
    im.rect(x, top, w, 1, 'W');
    im.rect(x + w - 1, top + 1, 1, 2, 'J');
    im.rect(x, top + 2, w, 1, 'i');
    // Its shadow on the ground.
    im.rect(x + 1, foot, w, 1, 'g');
}

/// Quoins stacked on a pallet: long and short stones alternating, as they
/// will lie at a corner, `w` wide.
fn quoins(im: &mut Img, x: i32, foot: i32, rows: i32) {
    im.rect(x - 1, foot - 1, 11, 1, 'B');
    for r in 0..rows {
        let y = foot - 3 - r * 3;
        let long_first = r % 2 == 0;
        let (a, b) = if long_first { (6, 3) } else { (3, 6) };
        for (cx, cw) in [(x, a), (x + a, b)] {
            im.rect(cx, y, cw, 3, 'i');
            im.rect(cx, y, cw, 1, 'H');
            im.rect(cx + cw - 1, y + 1, 1, 2, 'J');
            im.rect(cx, y + 2, cw, 1, 'G');
        }
    }
}

pub(super) fn castle_yard(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    let wk = t * u32::from(work);
    paint::earth(im, 24.0, (h - 14) as f32, 23.5, 13.5, seed);
    // The low paling along the back, a gate gap in it, its leaf swung open on the right post.
    let back = h - 26;
    material::rail_fence(im, (1, 18), back, &[back - 5, back - 2], 12, ('r', 'B'), seed);
    material::rail_fence(im, (29, 46), back, &[back - 5, back - 2], 12, ('r', 'B'), seed ^ 1);
    for y in [back - 5, back - 2] {
        im.rect(25, y, 4, 1, 'o');
    }
    im.rect(25, back - 6, 1, 7, 'B');
    im.line(25, back, 28, back - 5, 'B');
    // The Keep's pennant on a tall pole at the corner, a little cairn at its foot.
    paint::banner(im, 2, 2, back + 1, wk, ')', '5');
    im.rect(0, back, 5, 2, 'j');
    im.rect(0, back, 5, 1, 'J');
    // The stack of dressed stone along the back, its courses laid true.
    course_stack(im, 3, h - 13, 16, 4);
    // The crane: a timber mast on a sill braced to it, a jib out over the yard, a tie rope, a strut.
    let (mast, foot) = (41, h - 18);
    paint::post(im, mast, 8, foot);
    im.rect(mast - 6, foot, 13, 2, 'B');
    im.rect(mast - 6, foot, 13, 1, 'o');
    im.line(mast - 5, foot, mast, foot - 6, 'o');
    let tip = (27, 13);
    im.line(mast, 8, tip.0, tip.1, 'O');
    im.line(mast, 9, tip.0, tip.1 + 1, 'B');
    im.line(mast, 24, 32, 11, 'o');
    paint::rope(im, mast + 1, 5, tip.0, tip.1 - 1, 0.0, 't');
    im.rect(mast - 1, 6, 4, 3, 'o');
    im.put(mast, 5, 'O');
    // The pulley at the jib's end, the rope down, the block hung in its sling.
    im.rect(tip.0 - 1, tip.1, 3, 3, 'U');
    im.put(tip.0, tip.1 + 1, 'V');
    let lift = if work { ((paint::turn(t).cos() + 1.0) * 4.0).round() as i32 } else { 6 };
    let by = 20 + lift;
    im.line(tip.0, tip.1 + 3, tip.0, by, 't');
    im.line(tip.0, by, tip.0 - 3, by + 3, 't');
    im.line(tip.0, by, tip.0 + 3, by + 3, 't');
    im.rect(tip.0 - 4, by + 3, 9, 5, 'i');
    im.rect(tip.0 - 4, by + 3, 9, 1, 'H');
    im.rect(tip.0 + 4, by + 4, 1, 4, 'J');
    im.rect(tip.0 - 4, by + 7, 9, 1, 'G');
    // The roller-board under it: a sledge on three rollers, a block already aboard.
    let sled = h - 16;
    im.rect(20, sled, 16, 2, 'o');
    im.rect(20, sled, 16, 1, 'O');
    for x in [22, 28, 33] {
        im.ellipse(x as f32, (sled + 3) as f32, 1.9, 1.9, 'B');
        im.put(x - 1, sled + 2, 'o');
    }
    course_stack(im, 23, sled, 11, 1);
    paint::rope(im, 20, sled + 1, 15, sled + 5, 1.0, 't');
    // Timber squared and stacked on bearers: three layers, the beams' ends pale.
    let tb = h - 3;
    for x in [35, 44] {
        im.rect(x, tb - 1, 3, 2, 'b');
    }
    for l in 0..3 {
        let y = tb - 4 - l * 4;
        let inset = if l == 1 { 1 } else { 0 };
        im.rect(34 + inset, y, 13 - inset, 3, 'o');
        im.rect(34 + inset, y, 13 - inset, 1, 'O');
        im.rect(34 + inset, y + 2, 13 - inset, 1, 'B');
        im.rect(46 - inset, y, 1, 3, 't');
        im.put(40 + inset, y + 1, 'r');
        im.put(37 + inset, y + 1, 'r');
        if l < 2 {
            im.rect(36, y - 1, 1, 1, 'b');
            im.rect(43, y - 1, 1, 1, 'b');
        }
    }
    // Quoins on a pallet at the front, a tally board beside them, the mortar tub.
    quoins(im, 2, h - 2, 3);
    im.line(14, h - 5, 14, h - 2, 'B');
    im.line(20, h - 5, 20, h - 2, 'B');
    im.rect(12, h - 12, 11, 8, 'B');
    im.rect(13, h - 11, 9, 6, 'n');
    for k in 0..3 {
        let y = h - 10 + k * 2;
        for g in 0..2 {
            let x = 14 + g * 4;
            if (k + g) % 3 != 2 {
                im.line(x, y, x + 2, y, 'H');
            }
        }
    }
    im.rect(24, h - 10, 9, 8, 'r');
    im.rect(24, h - 10, 1, 8, 'o');
    im.rect(32, h - 10, 1, 8, 'B');
    im.rect(24, h - 3, 9, 1, 'b');
    im.line(24, h - 8, 32, h - 8, 'U');
    im.line(24, h - 5, 32, h - 5, 'U');
    im.ellipse(28.5, (h - 10) as f32, 4.5, 1.8, 'B');
    im.ellipse(28.5, (h - 10) as f32, 3.6, 1.3, 'J');
    im.put(27, h - 11, 'h');
    im.line(30, h - 11, 32, h - 17, 'o');
    im.rect(31, h - 18, 3, 1, 'v');
    // A hand carries a block from the quoins to the sledge.
    if work {
        let phase = paint::turn(t);
        let x = 22 + (phase.sin() * 6.0).round() as i32;
        hands::stamp(im, x, h - 2, Trade::Setter, Act::Carry(Load::Ashlar), t, phase.cos() < 0.0);
    }
    let _ = w;
}

// -------------------------------------------------------------- the camp ----

/// A canvas stripe at row `y` and column `x` of the pavilion, shaded by
/// its side: cream and the realm's violet, lit from the north-west.
fn canvas(u: f32, n: i32) -> char {
    let cream = n % 2 == 0;
    let (lit, mid, shade) = if cream { ('T', 't', 'O') } else { ('[', ')', '(') };
    if u < -0.3 {
        lit
    } else if u < 0.35 {
        mid
    } else {
        shade
    }
}

pub(super) fn camp(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 8) as f32, 7.8, 7.5, seed);
    // The pavilion: a pointed roof of striped canvas over low walls, its eaves scalloped.
    let (apex_y, eave_y) = (9, 23);
    for y in apex_y..=eave_y {
        let half = 0.8 + (y - apex_y) as f32 / (eave_y - apex_y) as f32 * 7.2;
        for x in 0..w {
            let u = (x as f32 + 0.5 - 8.0) / half;
            if u.abs() > 1.0 {
                continue;
            }
            let n = ((u + 1.0) / 2.0 * 7.0).floor() as i32;
            im.put(x, y, canvas(u, n));
        }
        // The roof's edge, a thin dark line against the sky.
        let half_i = half.round() as i32;
        im.put(8 - half_i - 1, y, 'b');
        im.put(8 + half_i, y, 'b');
    }
    // The walls under the eave: the same stripes, a door standing open.
    for y in eave_y + 1..eave_y + 10 {
        for x in 1..15 {
            let u = (x as f32 + 0.5 - 8.0) / 7.0;
            let n = ((u + 1.0) / 2.0 * 7.0).floor() as i32;
            im.put(x, y, canvas(u, n));
        }
        im.put(0, y, 'b');
        im.put(15, y, 'b');
    }
    // The scalloped valance along the eave, each tongue shadowing the wall.
    for x in 0..16 {
        let depth = if (1..=2).contains(&(x % 4)) { 3 } else { 2 };
        let u = (x as f32 + 0.5 - 8.0) / 7.5;
        let ink = canvas(u, (x / 2) % 2);
        for r in 0..depth {
            im.put(x, eave_y + 1 + r, ink);
        }
        im.put(x, eave_y + 1 + depth, 'b');
    }
    // The open door: the dark inside, the flaps tied back, a bedroll glimpsed.
    let door_y = eave_y + 5;
    im.rect(6, door_y, 4, 6, 'k');
    im.rect(7, door_y, 2, 5, 'K');
    im.line(6, door_y, 5, door_y + 5, 'O');
    im.line(9, door_y, 10, door_y + 5, 'O');
    im.line(7, door_y + 5, 8, door_y + 5, 'X');
    // The corner poles' feet.
    im.put(0, eave_y + 10, 'o');
    im.put(15, eave_y + 10, 'o');
    // The banner pole through the apex, its pennant streaming.
    paint::banner(im, 7, 1, apex_y + 1, t * u32::from(work), '[', '5');
    // The crate to the left of the fire and the sack to the right of it.
    im.rect(0, h - 7, 6, 6, 'o');
    im.rect(0, h - 7, 6, 1, 'O');
    im.rect(0, h - 2, 6, 1, 'b');
    im.line(1, h - 6, 4, h - 3, 'B');
    im.rect(5, h - 7, 1, 5, 'B');
    paint::sack(im, 10, h - 2, ['T', 't', 'O']);
    // The cooking fire at the tent door: a ring of stones, crossed logs, a tripod of
    // poles with an iron pot hung over it.
    let (fx, fy) = (8, h - 3);
    im.line(fx, fy - 11, fx - 5, fy, 'O');
    im.line(fx, fy - 11, fx + 5, fy, 'o');
    im.line(fx + 1, fy - 11, fx + 6, fy, 'B');
    im.line(fx, fy - 11, fx, fy - 9, 'J');
    im.line(fx - 3, fy, fx + 3, fy - 1, 'B');
    im.line(fx - 3, fy - 1, fx + 3, fy, 'o');
    // The pot: a rim, a bellied body, the stew glimpsed.
    im.rect(fx - 3, fy - 8, 7, 1, 'v');
    im.rect(fx - 3, fy - 7, 7, 2, 'g');
    im.rect(fx - 2, fy - 5, 5, 1, 'g');
    im.put(fx - 3, fy - 7, 'J');
    im.put(fx - 3, fy - 6, 'J');
    im.put(fx + 3, fy - 7, 'j');
    im.rect(fx - 2, fy - 8, 5, 1, if work { 'O' } else { 'o' });
    for (sx, sy, ink) in [(fx - 4, fy, 'J'), (fx - 2, fy + 1, 'h'), (fx, fy + 1, 'G'), (fx + 2, fy + 1, 'J'), (fx + 4, fy, 'G')] {
        im.rect(sx, sy - 1, 2, 2, ink);
        im.put(sx, sy - 1, 'i');
    }
    if work {
        paint::flame(im, fx, fy, 5, t, seed);
        // Sparks leaping from the flames, steam curling off the pot.
        for k in 0..3u32 {
            let age = (t + k * 3) % 8;
            let (sx, sy) = (fx - 2 + k as i32 * 2 + (age as i32 % 2), fy - 5 - age as i32);
            if age < 6 {
                im.put(sx, sy, if k % 2 == 0 { '6' } else { '5' });
            }
        }
        for k in 0..5 {
            let y = fy - 9 - k;
            let x = fx + ((paint::turn(t) + k as f32 * 0.9).sin() * 1.2).round() as i32;
            im.put(x, y, if k < 3 { 'H' } else { 'i' });
        }
    } else {
        im.rect(fx - 2, fy - 1, 5, 1, 'g');
        paint::embers(im, fx - 2, fy - 1, 5, seed);
        im.put(fx, fy - 1, '8');
        im.put(fx + 2, fy, '8');
    }
    let _ = (w, hash(0, 0, 0));
}

// ------------------------------------------------------ the shaft heads ----

/// The Great Shaft's mouth in the footprint's 16 rows: trodden ground, a
/// lip of dressed stone round a black shaft; its middle is at `(8, h - 7)`.
fn shaft_mouth(im: &mut Img, h: i32, seed: u32) {
    paint::earth(im, 8.0, (h - 7) as f32, 7.8, 6.6, seed);
    im.ellipse(8.0, (h - 6) as f32, 6.6, 4.2, 'J');
    im.ellipse(8.0, (h - 6) as f32, 6.1, 3.7, 'h');
    im.ellipse(8.0, (h - 6) as f32, 4.6, 2.7, 'k');
    im.ellipse(8.0, (h - 5) as f32, 4.0, 2.0, 'K');
    for x in 2..14 {
        if x % 3 != 1 {
            im.put(x, h - 10 + (x - 8).abs() / 4, 'i');
        }
    }
    for x in 3..13 {
        im.put(x, h - 2, 'j');
    }
}

/// How far down the rope its bucket hangs at step `t`: up and down the
/// shaft in a loop when `work`, else resting low.
fn bucket_drop(t: u32, work: bool, lo: i32, hi: i32) -> i32 {
    if work { lo + ((((paint::turn(t) + 0.9).cos() + 1.0) / 2.0) * (hi - lo) as f32).round() as i32 } else { hi }
}

pub(super) fn shaft_hand(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    shaft_mouth(im, h, seed);
    // Two baskets by the mouth for the lifted stone.
    for (x, y) in [(0, h - 6), (11, h - 5)] {
        im.rect(x, y, 5, 4, 'P');
        im.rect(x, y, 5, 1, 'O');
        im.rect(x + 1, y + 2, 3, 1, 'b');
    }
    // A tripod of poles over the collar, a pulley at its head.
    im.line(8, 4, 1, h - 4, 'O');
    im.line(8, 4, 14, h - 4, 'o');
    im.line(9, 4, 15, h - 4, 'B');
    im.line(8, 4, 8, h - 8, 'B');
    im.rect(6, 2, 5, 4, 'h');
    im.rect(7, 3, 3, 2, 'k');
    // The rope and the bucket bobbing in the throat.
    let by = h - 14 + bucket_drop(t, work, 0, 5);
    im.line(8, 6, 8, by, 't');
    paint::bucket(im, 6, by, if work { 'J' } else { 'g' });
}

pub(super) fn shaft_windlass(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    shaft_mouth(im, h, seed);
    // Two posts, a beam, a drum on its axle with a crank at the right.
    for x in [2, 12] {
        paint::post(im, x, 6, h - 4);
    }
    im.rect(1, 4, 14, 3, 'r');
    im.rect(1, 4, 14, 1, 'o');
    im.rect(4, 11, 9, 5, 'P');
    im.rect(4, 11, 9, 1, 'O');
    for k in 0..4 {
        im.rect(5 + ((k + if work { t as i32 / 2 } else { 0 }) % 4) * 2, 11, 1, 5, 'b');
    }
    let a = if work { paint::turn(t) } else { 0.6 };
    let (hx, hy) = (14 + (a.cos() * 1.0) as i32, 13 + (a.sin() * 3.0) as i32);
    im.line(13, 13, hx, hy, 'O');
    im.rect(hx - 1, hy - 1, 2, 2, 'o');
    // The bucket rising and falling as the drum turns.
    let by = h - 14 + bucket_drop(t, work, 0, 5);
    im.line(8, 16, 8, by, 't');
    paint::bucket(im, 6, by, if work { 'J' } else { 'g' });
}

/// A horse walking, seen from the side at `(x, foot)`, 8 wide: body, neck, head, four legs.
fn little_horse(im: &mut Img, x: i32, foot: i32, step: i32, left: bool) {
    // A grey, so it shows on the dark ground of the ring.
    let mut o = Img::new(8, 8);
    o.rect(0, 2, 5, 3, 'i');
    o.rect(0, 2, 5, 1, 'H');
    o.rect(0, 4, 5, 1, 'h');
    o.rect(4, 0, 2, 3, 'i');
    o.rect(5, 0, 3, 2, 'i');
    o.put(5, 0, 'k');
    o.put(6, 0, 'k');
    o.put(7, 1, 'h');
    o.line(0, 2, 0, 5, 'k');
    for (k, lx) in [0, 1, 3, 4].into_iter().enumerate() {
        let lift = i32::from((k as i32 + step) % 2 == 0);
        o.rect(lx, 5, 1, 3 - lift, if k % 2 == 0 { 'h' } else { 'J' });
    }
    let o = if left { o.flip_h() } else { o };
    im.stamp(&o, x - 4, foot - 7);
}

pub(super) fn shaft_gin(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 6) as f32, 7.8, 5.6, seed);
    // The trodden ring the horse walks, and the drum-house over the shaft in its middle.
    for k in 0..30 {
        let a = k as f32 / 30.0 * std::f32::consts::TAU;
        im.put((7.5 + a.cos() * 6.2).round() as i32, (h - 5) + (a.sin() * 3.0).round() as i32, if k % 2 == 0 { 'P' } else { 'I' });
    }
    let a = if work { paint::turn(t) } else { 2.5 };
    let (sx, sy) = (a.cos(), a.sin());
    // The sweep leaves the drum-house at mid height; the horse is yoked to its end.
    let (ex, ey) = (7.5 + sx * 6.0, (h - 13) as f32 + sy * 1.5);
    let (hx, hy) = (7.5 + sx * 4.5, (h - 4) as f32 + sy * 2.6);
    let step = if work { (t / 2) as i32 } else { 0 };
    let far = sy < 0.0;
    let (exi, eyi, hxi, hyi) = (ex.round() as i32, ey.round() as i32, hx.round() as i32, hy.round() as i32);
    if far {
        little_horse(im, hxi, hyi, step, sx > 0.0);
        im.line(8, h - 13, exi, eyi, 'O');
        im.line(exi, eyi, hxi, hyi - 4, 't');
    }
    // The drum-house: boards round a drum, a thatched cone, a low black door onto the shaft.
    im.rect(4, h - 19, 8, 11, 'P');
    im.rect(4, h - 19, 2, 11, 'o');
    im.rect(10, h - 19, 2, 11, 'B');
    for y in (h - 17..h - 9).step_by(3) {
        im.line(4, y, 11, y, 'b');
    }
    paint::trapezoid(im, h - 29, h - 20, (7.0, 9.0), (2.0, 14.0), &|x, y, side| {
        let n = hash(x, y / 2, seed) % 5;
        if side < 0 { 'T' } else if side > 0 { 'B' } else if n == 0 { 'o' } else if x > 9 { 'r' } else { 'O' }
    });
    im.rect(6, h - 15, 4, 6, 'k');
    im.rect(8, h - 31, 1, 3, 'B');
    if !far {
        im.line(8, h - 13, exi, eyi, 'O');
        im.line(8, h - 12, exi, eyi + 1, 'o');
        im.line(exi, eyi, hxi, hyi - 4, 't');
        little_horse(im, hxi, hyi, step, sx > 0.0);
    }
}

pub(super) fn shaft_water(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    shaft_mouth(im, h, seed);
    // The wheel behind the headframe: a rim, spokes turning, paddles round it.
    let (wx, wy) = (11.0f32, (h - 15) as f32);
    let a = if work { paint::turn(t) / 4.0 } else { 0.4 };
    paint::wheel(im, wx, wy, 5.0, a, 8, true, ('O', 'B', 'k'));
    im.line(2, wy as i32, 11, wy as i32, 'o');
    // The race above it and the water falling on its paddles, the pool below.
    im.rect(8, 4, 8, 2, 'P');
    im.rect(8, 4, 8, 1, 'o');
    im.rect(9, 5, 6, 1, 'Q');
    if work {
        for k in 0..4 {
            im.put(14, 6 + k * 2 + (t as i32 + k) % 2, if k % 2 == 0 { 'z' } else { 'Q' });
        }
    }
    im.rect(9, h - 4, 7, 2, 'q');
    if work {
        im.put(10 + (t as i32 % 5), h - 4, 'z');
    }
    // The headframe in front over the shaft: two posts, a beam, a drum geared to the wheel.
    for x in [1, 6] {
        paint::post(im, x, 9, h - 5);
    }
    im.rect(0, 7, 9, 3, 'r');
    im.rect(0, 7, 9, 1, 'o');
    im.rect(2, wy as i32 - 2, 5, 5, 'P');
    im.rect(2, wy as i32 - 2, 5, 1, 'O');
    for k in 0..2 {
        im.rect(3 + ((k + if work { t as i32 / 2 } else { 0 }) % 2) * 2, wy as i32 - 2, 1, 5, 'b');
    }
    let by = h - 14 + bucket_drop(t, work, 0, 3);
    im.line(4, wy as i32 + 3, 4, by, 't');
    im.rect(3, by, 3, 3, 'o');
}

pub(super) fn shaft_ley(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    shaft_mouth(im, h, seed);
    // Two standing stones with crystal heads either side of the mouth.
    for (k, x) in [1, 12].into_iter().enumerate() {
        im.rect(x, h - 20, 3, 15, 'i');
        im.rect(x, h - 20, 1, 15, 'H');
        im.rect(x + 2, h - 20, 1, 15, 'J');
        let glow = if work && (t as usize / 2 + k) % 2 == 0 { 'w' } else if work { '3' } else { 'S' };
        im.rect(x, h - 25, 3, 5, glow);
        im.put(x + 1, h - 26, if work { '3' } else { 'u' });
    }
    // The column of light, bright at its core, and the block riding up it.
    if work {
        for y in 2..h - 8 {
            for x in 5..11 {
                let d = (x as f32 - 7.5).abs();
                let flick = (y + t as i32) % 4;
                if d < 1.0 {
                    im.put(x, y, if flick == 0 { 'w' } else { '3' });
                } else if d < 2.0 && (x + y + t as i32) % 3 == 0 {
                    im.put(x, y, '2');
                }
            }
        }
        let rise = (t as i32 * 3) % 22;
        let by = h - 16 - rise;
        im.rect(5, by, 6, 4, 'H');
        im.rect(5, by, 6, 1, 'W');
        im.rect(5, by + 3, 6, 1, 'h');
    } else {
        im.rect(5, h - 13, 6, 4, 'h');
        im.rect(5, h - 13, 6, 1, 'i');
    }
}
