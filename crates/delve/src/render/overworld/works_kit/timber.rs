//! The wood trades: felling, sawing, joinery, the wheelwright, the ropewalk.

use super::hands::{self, Act, Pose, Trade};
use super::{Ctx, Load, paint};
use crate::render::overworld::ink::hash;
use crate::render::overworld::material::ramp;

/// A felled log lying along `x0..x1` with its top on `y`, `r` thick, its
/// right end showing the rings.
fn log(im: &mut crate::render::overworld::ink::Img, x0: i32, x1: i32, y: i32, r: i32) {
    for yy in 0..r {
        let ch = match yy {
            0 => 'O',
            1 => 'o',
            _ if yy == r - 1 => 'b',
            _ if yy >= r - 3 => 'I',
            _ => 'B',
        };
        for x in x0..=x1 {
            let rough = if (x / 3 + yy) % 7 == 0 { 'P' } else { ch };
            im.put(x, y + yy, if yy > 1 && yy < r - 3 { rough } else { ch });
        }
    }
    // The cut end: rings round a pale heart.
    im.ellipse(x1 as f32, y as f32 + r as f32 / 2.0, 1.8, r as f32 / 2.0, 'O');
    im.ellipse(x1 as f32, y as f32 + r as f32 / 2.0, 1.0, r as f32 / 3.0, 't');
}

pub(super) fn sawpit(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let im = &mut c.im;
    paint::earth(im, 24.0, 40.0, 23.0, 11.0, seed);
    // The pit: a dark cut in the ground between the trestles, boarded at its lip.
    im.rect(11, 30, 28, 16, 'k');
    im.rect(11, 30, 28, 3, 'K');
    for y in 33..46 {
        im.put(11, y, 'X');
        im.put(38, y, 'X');
    }
    // The trestles: two A-frames and their brace.
    for (cx, dir) in [(8, 1), (41, -1)] {
        im.line(cx, 24, cx - 4 * dir, 46, 'o');
        im.line(cx + 1, 24, cx - 4 * dir + 1, 46, 'B');
        im.line(cx, 24, cx + 4 * dir, 46, 'o');
        im.line(cx + 1, 24, cx + 4 * dir + 1, 46, 'B');
        im.line(cx - 2, 36, cx + 3, 36, 'O');
    }
    // The pit sawyer first, then the log over his head, then the lip over his feet.
    let mut bottom_grip = (26, 36);
    let mut top_grip = (26, 12);
    if work {
        bottom_grip = hands::stamp(im, 30, 46, Trade::Sawyer, Act::Bare(Pose::Thrust), t + 3, true);
    }
    log(im, 3, 44, 22, 8);
    // A cut already run along the log's length: the kerf, and planks off it.
    if !work {
        im.line(8, 25, 36, 25, 'b');
    }
    if work {
        top_grip = hands::stamp(im, 22, 22, Trade::Sawyer, Act::Bare(Pose::Thrust), t, false);
    }
    // The saw: handles top and bottom, the blade through the log.
    let (sx, y_top, y_bot) = if work { (26, top_grip.1, bottom_grip.1) } else { (26, 12, 36) };
    for y in y_top..=y_bot {
        im.put(sx, y, 'V');
        im.put(sx + 1, y, if y % 3 == 0 { 'u' } else { 'v' });
    }
    im.rect(sx - 2, y_top - 1, 6, 2, 'o');
    im.rect(sx - 2, y_bot, 6, 2, 'o');
    // The lip of the pit, over the pit sawyer's feet.
    im.rect(10, 44, 30, 3, 'O');
    im.rect(10, 46, 30, 1, 'B');
    // What it makes: sawdust under the cut, planks stacked and leaning.
    let fall = if work { 3 } else { 1 };
    for k in 0..fall {
        im.put(25 + (k * 2) % 4, 47, 'T');
        im.put(24 + (k * 3) % 5, 48, 't');
    }
    paint::pile(im, 8, 50, 14, 7, Load::Planks, seed);
    paint::pile(im, 40, 50, 14, 8, Load::Timber, seed ^ 3);
    for k in 0..3 {
        im.line(33 + k, 24, 39 + k, 47, if k == 1 { 'T' } else { 'O' });
    }
}

pub(super) fn ropewalk(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, w as f32 / 2.0, (h - 8) as f32, w as f32 * 0.48, 9.0, seed);
    // The long low shed: a boarded back wall in shadow, a thin thatched roof the whole length.
    let floor = h - 7;
    paint::boards(im, 3, 12, w - 6, floor - 12, seed);
    im.rect(3, 12, w - 6, floor - 12, 'K');
    paint::boards(im, 3, 12, w - 6, 4, seed);
    paint::lean_roof(im, 0, 2, w, 11, ramp::THATCH, seed);
    // Hanks of hemp hung up along the back wall, waiting to be spun.
    for k in 0..8 {
        let x = 22 + k * 8;
        im.rect(x, 17, 3, 8, if k % 2 == 0 { 'M' } else { 'A' });
        im.rect(x, 17, 3, 1, 'o');
        im.put(x + 1, 25, 'N');
    }
    // The strands: three yarns paying out along the lane, twisting as they go.
    let y = floor - 12;
    let (x0, x1) = (16, w - 18);
    for (n, yy) in [y, y + 4, y + 8].into_iter().enumerate() {
        for i in x0..x1 {
            let phase = if work { (i + t as i32 * 2 + n as i32 * 3) % 8 } else { (i + n as i32 * 3) % 8 };
            let (a, b) = if phase < 4 { ('T', 't') } else { ('t', 'O') };
            im.put(i, yy, a);
            im.put(i, yy + 1, b);
        }
    }
    // The front posts, over the lane.
    for k in 0..=8 {
        paint::post(im, 4 + k * (w - 10) / 8, 12, floor + 2);
    }
    // The jack at the left end: a great spinning wheel, a hand at its crank.
    let a = if work { paint::turn(t) * 2.0 / 8.0 } else { 0.3 };
    paint::wheel(im, 10.0, (y + 6) as f32, 8.0, a, 8, false, ('o', 'O', 'B'));
    let (hx, hy) = (10.0 + (a * 8.0).cos() * 8.0, (y + 6) as f32 + (a * 8.0).sin() * 8.0);
    im.rect(hx as i32 - 1, hy as i32 - 1, 3, 3, 'B');
    if work {
        hands::stamp(im, 20, floor + 3, Trade::Roper, Act::Bare(Pose::Slash), t, true);
    }
    // The traveller, a sled on the lane, walked back as the strand lengthens.
    let back = if work { (paint::turn(t).sin() * 3.0).round() as i32 } else { 0 };
    let tx = x1 + back;
    im.rect(tx, y - 2, 6, 14, 'o');
    im.rect(tx, y - 2, 6, 1, 'O');
    for yy in [y, y + 4, y + 8] {
        im.put(tx - 1, yy, 'V');
    }
    if work {
        hands::stamp(im, tx + 12, floor + 3, Trade::Roper, Act::Bare(Pose::Walk), t, true);
    }
    // Finished rope coiled at the end, hemp hanks at the start.
    paint::pile(im, w - 8, h - 1, 14, 10, Load::Rope, seed);
    paint::pile(im, 6, h - 1, 10, 7, Load::Hemp, seed ^ 7);
}
/// A stump with its rings showing, `r` across.
fn stump(im: &mut crate::render::overworld::ink::Img, x: i32, foot: i32, r: i32) {
    im.rect(x - r, foot - 4, 2 * r, 5, 'B');
    im.rect(x - r, foot - 4, 1, 5, 'o');
    im.ellipse(x as f32, (foot - 4) as f32, r as f32, 2.2, 'O');
    im.ellipse(x as f32, (foot - 4) as f32, r as f32 * 0.6, 1.3, 't');
    im.put(x, foot - 4, 'o');
}

pub(super) fn woodsman_camp(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // A brush shelter: a ridge of poles thatched with leafy boughs, a dark mouth, a bedroll.
    paint::trapezoid(im, h - 34, h - 4, (13.0, 14.0), (0.0, 27.0), &|x, y, side| {
        let n = hash(x, y / 2, seed) % 6;
        if side != 0 { 'E' } else if n == 0 { 'N' } else if n == 1 { 'F' } else if (x + y) % 3 == 0 { 'l' } else { 'm' }
    });
    paint::trapezoid(im, h - 18, h - 4, (13.0, 14.0), (7.0, 20.0), &|_, y, side| if side != 0 { 'b' } else if y > h - 8 { 'I' } else { 'k' });
    im.rect(9, h - 8, 8, 3, 'Q');
    im.line(13, h - 34, 13, h - 36, 'o');
    // The block, a log on it half split, the axe; split billets heaped, a bundle of coppice poles.
    stump(im, 33, h - 3, 5);
    im.rect(30, h - 12, 6, 6, 'B');
    im.rect(30, h - 12, 6, 1, 'o');
    im.line(33, h - 12, 33, h - 7, 'b');
    for k in 0..6 {
        im.line(42 + k, h - 3, 44 + k / 2, h - 30, if k % 2 == 0 { 'o' } else { 'O' });
    }
    im.line(42, h - 18, 47, h - 18, 't');
    paint::pile(im, 21, h - 1, 11, 8, Load::Timber, seed);
    if work {
        hands::stamp(im, 26, h - 3, Trade::Woodsman, Act::Work, t, true);
        paint::chips(im, 31, h - 12, t, 4, 'T');
    } else {
        im.line(34, h - 12, 37, h - 20, 'B');
        im.rect(35, h - 23, 5, 4, 'v');
    }
    let _ = w;
}
pub(super) fn carpentry(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // A boarded shop with its front open and a roof of oak shingles over it.
    let wall_top = h - 30;
    paint::boards(im, 3, wall_top, 30, 28, seed);
    paint::roof(im, 0, wall_top - 15, 36, 16, ramp::TIMBER, seed);
    im.rect(6, wall_top + 5, 24, 22, 'K');
    // Inside: the bench along the back, a plane on it, joists racked above.
    im.rect(6, h - 15, 24, 2, 'O');
    im.rect(6, h - 13, 2, 11, 'B');
    im.rect(28, h - 13, 2, 11, 'B');
    for k in 0..4 {
        im.rect(8, wall_top + 7 + k * 2, 20, 1, if k % 2 == 0 { 'O' } else { 'o' });
    }
    im.rect(14, h - 18, 6, 3, 'V');
    // The great arch rib standing outside, half-built: a centring for a vault.
    for k in 0..16 {
        let a = std::f32::consts::PI * k as f32 / 15.0;
        let (x, y) = (40.0 + a.cos() * 7.0, (h - 6) as f32 - a.sin() * 13.0);
        im.rect(x as i32, y as i32, 2, 2, if k % 2 == 0 { 'O' } else { 'o' });
    }
    im.line(33, h - 6, 47, h - 6, 'o');
    im.line(40, h - 6, 40, h - 20, 'B');
    paint::pile(im, 7, h - 1, 12, 7, Load::Planks, seed);
    if work {
        hands::stamp(im, 20, h - 3, Trade::Carpenter, Act::Work, t, false);
        paint::chips(im, 28, h - 13, t, 3, 'T');
    } else {
        im.line(24, h - 15, 24, h - 6, 'B');
        im.rect(22, h - 16, 5, 3, 'v');
    }
    let _ = w;
}

pub(super) fn wheelwright(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // A shop of heavy boards, a lean-to roof, its walls hung with wheels.
    paint::boards(im, 3, h - 30, 28, 28, seed);
    paint::lean_roof(im, 0, h - 44, 34, 15, ramp::TIMBER, seed);
    for (x, y, r) in [(10.0f32, (h - 21) as f32, 7.0f32), (23.0, (h - 17) as f32, 8.0)] {
        paint::wheel(im, x, y, r, 0.3, 8, false, ('B', 'O', 'U'));
        paint::ring(im, x, y, r - 2.0, 'o');
    }
    // A wheel on its stand in the yard: the iron tyre dropped on it red-hot, steam and all.
    im.rect(40, h - 18, 2, 14, 'B');
    im.rect(34, h - 5, 14, 2, 'B');
    let (wx, wy) = (41.0f32, (h - 14) as f32);
    paint::wheel(im, wx, wy, 6.0, 0.7, 8, false, ('B', 'o', 'O'));
    let rim = if work { '@' } else { 'U' };
    paint::ring(im, wx, wy, 7.5, rim);
    paint::ring(im, wx, wy, 8.5, if work { '4' } else { 'u' });
    if work {
        paint::smoke(im, 41, h - 24, t, 12, 3, true);
        hands::stamp(im, 30, h - 3, Trade::Wheelwright, Act::Work, t, true);
        if t % 4 < 2 {
            for k in 0..4 {
                im.put(35 + k, h - 18 + k % 2, '6');
            }
        }
    }
    // A cart body waiting for its wheels, and spokes in a bundle.
    im.rect(4, h - 9, 18, 5, 'o');
    im.rect(4, h - 9, 18, 1, 'O');
    im.rect(4, h - 5, 18, 1, 'b');
    for x in [8, 14] {
        im.line(x, h - 4, x, h - 1, 'B');
    }
    let _ = w;
}
