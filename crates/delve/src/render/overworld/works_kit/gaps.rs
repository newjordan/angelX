//! The plans the first kit left undrawn: the hods and ladders, the great
//! crane, the vineyard, the wine press, the aqueduct, and the swape as a plan
//! (the swape's own pivoting arm, turned to the way it faces, is
//! `rollers::swape_flat`). Each sits on the footprint the sim gives it
//! (`delve/kingdom/frontier.json`: one tile, the aqueduct seven by one), a
//! pure function of `(state, step)` like the rest.

use super::hands::{self, Act, Pose, Trade};
use super::{Ctx, Load, State, paint, rollers};
use crate::render::overworld::ink::hash;

/// Hods and ladders: a timber sheerleg with a pulley at its head, a ladder
/// leant on a stage of boards, a hod of stone riding the rope while a labourer
/// goes up and down. The first lift, before any crane.
pub(super) fn hod_gang(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 8) as f32, 7.5, 6.5, seed);
    // The stage: three boards on two trestles, a stack of cut stone on it.
    im.rect(1, h - 14, 12, 2, 'o');
    im.rect(1, h - 12, 12, 1, 'B');
    for x in [2, 11] {
        im.rect(x, h - 11, 1, 7, 'I');
    }
    // The sheerleg: two poles leaning together to a head over the stage, a pulley, a tie to the ground.
    paint::post(im, 12, h - 31, h - 4);
    im.line(14, h - 4, 12, h - 31, 'B');
    im.rect(10, h - 32, 5, 2, 'b');
    im.put(11, h - 31, 'T');
    // The ladder, leant on the stage's edge, its rungs every third row.
    im.line(3, h - 4, 6, h - 22, 'O');
    im.line(5, h - 4, 8, h - 22, 'o');
    for y in (h - 20..h - 5).step_by(3) {
        let x = 3 + (h - 4 - y) / 6;
        im.rect(x, y, 3, 1, 'B');
    }
    // The hod rides the rope up and down the sheerleg while they work, hangs at its foot otherwise.
    let lift = if work { ((paint::turn(t).cos() + 1.0) * 6.0).round() as i32 } else { 12 };
    let hook = h - 27 + lift;
    paint::rope(im, 11, h - 30, 11, hook, 0.0, 't');
    im.rect(8, hook, 7, 5, 'B');
    im.rect(8, hook, 7, 1, 'o');
    im.rect(9, hook - 2, 5, 3, 'i');
    im.rect(9, hook - 2, 5, 1, 'H');
    if work {
        // A labourer climbs under a load on the ladder's foot.
        hands::stamp(im, 5, h - 3, Trade::Labourer, Act::Carry(Load::Ashlar), t, false);
    } else {
        hands::stamp(im, 5, h - 3, Trade::Labourer, Act::Idle, 0, false);
    }
    paint::pile(im, 13, h - 1, 5, 5, Load::Ashlar, seed);
}

/// The great crane: a tall timber mast braced in the X, a long jib with its counterweight, the
/// stays, a treadwheel at the foot big enough for four, the hook's block riding the rope.
pub(super) fn great_crane(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 6) as f32, 7.8, 5.5, seed);
    // The footing: dressed stone under a timber sill.
    im.rect(1, h - 6, 14, 3, 'J');
    im.rect(1, h - 6, 14, 1, 'h');
    im.rect(0, h - 4, 16, 2, 'o');
    // The mast: two posts a little closer at the head than the foot, a cross brace in each bay.
    let top = 6;
    for y in top..h - 6 {
        let lean = (h - 6 - y) / 14;
        im.put(6 + lean / 2, y, 'O');
        im.put(7 + lean / 2, y, 'B');
        im.put(10 - lean / 2, y, 'o');
        im.put(11 - lean / 2, y, 'B');
    }
    for k in 0..5 {
        let (y0, y1) = (top + 2 + k * 9, top + 11 + k * 9);
        if y1 >= h - 6 {
            break;
        }
        im.line(7, y0, 10, y1, 'P');
        im.line(10, y0, 7, y1, 'P');
        im.rect(6, y1, 6, 1, 'b');
    }
    // The head: a deck, the jib out to the left with its counterweight right, the stays to the mast's top.
    im.rect(4, top - 1, 10, 2, 'b');
    im.line(8, top - 1, 0, top + 3, 'o');
    im.line(8, top, 0, top + 4, 'B');
    im.line(9, top, 15, top + 1, 'o');
    im.rect(13, top + 1, 3, 4, 'G');
    im.rect(13, top + 1, 3, 1, 'h');
    paint::rope(im, 8, 1, 0, top + 3, 0.0, 't');
    paint::rope(im, 8, 1, 14, top + 1, 0.0, 't');
    // A pennon on the mast's tip.
    im.rect(8, 0, 1, 3, 'I');
    im.rect(9, 0, 3, 2, '4');
    // The treadwheel at the mast's foot, a quarter of it showing past the sill, turning at work.
    let a = if work { paint::turn(t) * 2.0 / 8.0 } else { 0.3 };
    paint::wheel(im, 3.0, (h - 11) as f32, 5.0, a, 8, false, ('o', 'B', 'O'));
    // The hook's rope from the jib's tip, the block riding it and a cut stone in the sling.
    let lift = if work { ((paint::turn(t).cos() + 1.0) * 9.0).round() as i32 } else { 28 };
    let hook = top + 10 + lift;
    paint::rope(im, 0, top + 4, 0, hook, 0.0, 't');
    im.rect(0, hook, 4, 4, 'i');
    im.rect(0, hook, 4, 1, 'H');
    im.put(0, hook - 1, 'T');
    paint::pile(im, 13, h - 1, 5, 4, Load::Ashlar, seed);
}

/// The vineyard: rows of vines on stakes along a wire, grapes dark in the leaf, a basket at the
/// row's end; a picker stooping at the vines while they work.
pub(super) fn vineyard(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    // Ground the vines grow on: a ragged patch.
    paint::earth(im, 8.0, (h - 8) as f32, 8.0, 7.0, seed);
    // Three terraces of vine, each a wire on stakes with its leaves heaped along it.
    for (k, row) in [h - 16, h - 11, h - 6].into_iter().enumerate() {
        for x in (1..15).step_by(4) {
            im.rect(x, row - 5, 1, 6, 'I');
        }
        im.line(1, row - 4, 14, row - 4, 'p');
        for x in 1..15 {
            let n = hash(x, k as i32, seed) % 5;
            let (leaf, dark) = if n < 2 { ('m', 'l') } else if n < 4 { ('l', 'E') } else { ('A', 'm') };
            im.rect(x, row - 4, 1, 4, leaf);
            im.put(x, row - 5 + (n as i32 % 2), dark);
        }
        // Grapes in dark bunches under the leaf, a few on each row; paler in the first row's sun.
        for x in (2..14).step_by(3) {
            let low = (hash(x, k as i32, seed ^ 5) % 2) as i32;
            im.rect(x, row - 1 + low, 2, 2, if k == 0 { '[' } else { ')' });
            im.put(x, row - 1 + low, ']');
        }
    }
    // A picking basket at the foot of the rows, and a picker at work.
    im.rect(11, h - 3, 4, 3, 'o');
    im.rect(11, h - 3, 4, 1, 'O');
    im.rect(12, h - 4, 2, 1, ')');
    if work {
        hands::stamp(im, 8, h - 2, Trade::Herder, Act::Bare(if t % 4 < 2 { Pose::Overhead } else { Pose::Stand }), t, false);
    }
}

/// The wine press: a screw press in a timber frame over a stone vat, the long bar turning the
/// screw, red wine running into a tub, casks at its side.
pub(super) fn wine_press(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let h = c.h;
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 6) as f32, 7.8, 5.8, seed);
    // The vat: a stone ring, the must in it.
    im.rect(2, h - 9, 12, 7, 'G');
    im.rect(2, h - 9, 12, 1, 'h');
    im.rect(3, h - 8, 10, 3, 'a');
    im.rect(3, h - 8, 10, 1, '7');
    // The frame: two uprights, a head beam, the screw down the middle to the press board.
    for x in [2, 13] {
        im.rect(x, h - 24, 2, 17, 'B');
        im.put(x, h - 24, 'o');
    }
    im.rect(1, h - 26, 14, 3, 'o');
    im.rect(1, h - 26, 14, 1, 'O');
    let sink = if work { ((paint::turn(t).sin() + 1.0) * 1.5).round() as i32 } else { 1 };
    for y in h - 23..h - 11 + sink {
        im.put(7, y, if y % 2 == 0 { 'I' } else { 'P' });
        im.put(8, y, 'p');
    }
    im.rect(4, h - 12 + sink, 8, 2, 'O');
    im.rect(4, h - 12 + sink, 8, 1, 't');
    // The bar through the screw's head, swinging as it is hauled round.
    let swing = if work { ((paint::turn(t).cos()) * 5.0).round() as i32 } else { 4 };
    im.line(7 - swing, h - 21, 8 + swing, h - 21, 'B');
    im.line(7 - swing, h - 22, 8 + swing, h - 22, 'o');
    // The tub the wine runs into at the vat's lip, and the drip.
    im.rect(12, h - 5, 4, 3, 'B');
    im.rect(12, h - 5, 4, 1, 'o');
    im.rect(13, h - 4, 2, 1, 'a');
    if work {
        let y = h - 7 + (t % 3) as i32;
        im.put(14, y, '7');
    }
    // Casks at the foot, hooped.
    for (x, foot) in [(0, h - 1), (4, h - 1)] {
        im.rect(x, foot - 5, 4, 5, 'r');
        im.rect(x, foot - 4, 4, 1, 'I');
        im.rect(x, foot - 2, 4, 1, 'I');
        im.rect(x + 1, foot - 5, 2, 1, 'R');
    }
}

/// The aqueduct: an arcade of dressed stone seven tiles long, round arches between piers, a water
/// channel along its top that runs at work. Its picture is `7 * 16` wide.
pub(super) fn aqueduct(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // The wall of the arcade, coursed stone, from the channel's floor down to the ground.
    let (top, floor) = (h - 28, h - 1);
    for y in top..=floor {
        for x in 0..w {
            let course = (y - top) / 4;
            let n = hash(x / 5 + course * 7, course, seed) % 6;
            let joint = (y - top) % 4 == 0 || (x + course * 2) % 10 == 0;
            let ch = if y == top { 'h' } else if joint { 'j' } else if n == 0 { 'i' } else if n < 3 { 'h' } else { 'J' };
            im.put(x, y, ch);
        }
    }
    // The arches cut through it: a round head over each bay between the piers.
    for k in 0..w / 16 {
        let (x0, x1) = (k * 16 + 3, k * 16 + 13);
        let (cx, cy) = ((x0 + x1) as f32 / 2.0, (top + 12) as f32);
        for y in top + 8..=floor {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let inside = if (y as f32) < cy { (dx * dx) / 25.0 + ((y as f32 - cy) * (y as f32 - cy)) / 25.0 <= 1.0 } else { true };
                if inside {
                    im.clear(x, y);
                }
            }
        }
        // The keystone and the shadow under the arch.
        im.put(k * 16 + 8, top + 7, 'H');
    }
    // The channel: a trough of timber and puddled clay along the top, water in it, running at work.
    im.rect(0, top - 6, w, 6, 'B');
    im.rect(0, top - 6, w, 1, 'o');
    for x in 1..w - 1 {
        let ripple = if work { (x + (t as i32) * 2) % 8 < 2 } else { x % 11 == 0 };
        im.put(x, top - 4, if ripple { 'W' } else { 'z' });
        im.put(x, top - 3, if ripple { 'z' } else { 'Q' });
        im.put(x, top - 2, 'q');
    }
    // A post and a lantern at each end of it.
    for x in [1, w - 3] {
        im.rect(x, top - 14, 1, 8, 'I');
        im.rect(x - 1, top - 15, 3, 2, if work { '6' } else { 'o' });
    }
}

/// The swape as a plan: the post and its pivot arm, standing facing south; a built swape's own
/// arm is turned to the way it faces (`rollers::swape_flat`).
pub(super) fn swape(c: &mut Ctx) {
    let state = if c.work { State::Working } else { State::Idle };
    let pic = rollers::swape_flat(2, state, c.t);
    let y = c.h - pic.h;
    c.im.stamp(&pic, 0, y);
}
