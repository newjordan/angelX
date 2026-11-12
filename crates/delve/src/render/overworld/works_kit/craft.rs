//! Farm and household trades: the farmstead, the mill, the oven, the
//! brewery, the loom.

use super::hands::{self, Act, Pose, Trade};
use super::{Ctx, Load, paint};
use paint::Front;
use crate::render::overworld::ink::hash;
use crate::render::overworld::material::{self, ramp};

pub(super) fn farmstead(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // Ploughed strips across the front, in furrows: ripe grain, hemp.
    for (k, (x, y, fw, fh, grain)) in [(2, h - 24, 28, 10, true), (33, h - 24, 29, 10, false), (2, h - 13, 60, 11, true)].into_iter().enumerate() {
        for yy in y..y + fh {
            for xx in x..x + fw {
                let (u, v) = ((xx - x) % 4, (yy - y) % 4);
                let sway = work && ((xx / 4) as u32 + t) % 8 < 3;
                let ch = if u == 3 {
                    'I'
                } else if grain {
                    match (u, v) {
                        (1, 0) => if sway { 'O' } else { 'T' },
                        (1, 1) => 't',
                        (1, _) => 'm',
                        (0, 0) => 'o',
                        (2, 1) => 'T',
                        (0, _) | (2, _) => 'B',
                        _ => 'B',
                    }
                } else {
                    match (u, v) {
                        (1, 0) | (1, 1) => if sway { 'y' } else { 'M' },
                        (1, _) => 'A',
                        (0, 1) | (2, 2) => 'N',
                        _ => 'B',
                    }
                };
                im.put(xx, yy, if hash(xx, yy, seed ^ k as u32) % 23 == 0 { 'P' } else { ch });
            }
        }
        im.rect(x, y + fh, fw, 1, 'b');
    }
    // The farmhouse: stone below, thatch, a chimney smoking.
    let wall_y = paint::dwelling(im, 0, h - 62, 32, 17, 20, Front::Stone, ramp::THATCH, seed);
    paint::door(im, 12, wall_y + 19, 6, 11);
    paint::window(im, 5, wall_y + 4, 4, 4, work);
    paint::window(im, 22, wall_y + 4, 4, 4, work);
    paint::chimney(im, 24, h - 58, 4, 10);
    if work {
        paint::smoke(im, 26, h - 70, t, 12, 4, false);
    }
    // The barn: tall boards, a broad door, a loft hatch.
    let barn_y = paint::dwelling(im, 34, h - 64, 30, 15, 24, Front::Board, ramp::THATCH, seed ^ 1);
    paint::door(im, 44, barn_y + 23, 11, 16);
    im.rect(46, barn_y + 3, 6, 5, 'k');
    // A rick of hay.
    paint::pile(im, 58, h - 25, 11, 9, Load::Grain, seed);
    if work {
        hands::stamp(im, 20, h - 14, Trade::Herder, Act::Work, t, false);
    }
    let _ = w;
}
pub(super) fn mill(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 9) as f32, 15.5, 8.5, seed);
    // The mill house: square, stone, a steep slate roof.
    paint::stone(im, 3, h - 36, 24, 33, seed);
    paint::roof(im, 0, 8, 30, 14, ramp::SLATE, seed);
    // The stone's window: the runner stone seen turning, its furrows spinning.
    im.rect(10, h - 30, 10, 9, 'b');
    im.ellipse(15.0, (h - 25) as f32, 4.0, 3.0, 'J');
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU + if work { paint::turn(t) * 0.5 } else { 0.0 };
        im.put((15.0 + a.cos() * 3.2) as i32, (h - 25) as f32 as i32 + (a.sin() * 2.4) as i32, 'g');
    }
    // The hoist: a beam out of the loft with a rope, a sack riding it.
    im.line(21, 16, 31, 16, 'o');
    im.line(21, 17, 31, 17, 'B');
    let lift = if work { ((paint::turn(t).cos() + 1.0) * 6.0).round() as i32 } else { 14 };
    paint::rope(im, 30, 17, 30, 20 + lift, 0.0, 't');
    paint::sack(im, 27, 27 + lift, ['H', 'i', 'h']);
    // The door at the foot, flour dust drifting out of it.
    paint::door(im, 12, h - 4, 7, 11);
    if work {
        paint::dust(im, 16, h - 12, t, 'H');
        paint::dust(im, 20, h - 14, t + 3, 'i');
        hands::stamp(im, 6, h - 2, Trade::Miller, Act::Work, t, false);
    }
    paint::pile(im, 26, h - 1, 11, 8, Load::Flour, seed);
    paint::pile(im, 5, h - 1, 9, 6, Load::Grain, seed ^ 2);
    let _ = w;
}

pub(super) fn bakery(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 9) as f32, 15.5, 8.5, seed);
    // A brick house under tile, a counter in its window, a chimney.
    let wall_y = paint::dwelling(im, 0, 14, 26, 16, h - 2 - 29, Front::Brick, ramp::TILE, seed);
    paint::chimney(im, 17, 20, 5, 14);
    paint::window(im, 5, wall_y + 5, 8, 7, work);
    im.rect(4, wall_y + 12, 10, 2, 'O');
    for k in 0..3 {
        im.ellipse(6.0 + k as f32 * 3.0, (wall_y + 11) as f32, 1.6, 1.0, 'O');
    }
    paint::door(im, 16, h - 3, 6, 13);
    // The domed oven bulging out of the side, its mouth glowing.
    let keep = |_: i32, _: i32| true;
    material::lobe(im, (26.0, (h - 13) as f32), (6.5, 9.0), &['b', 'B', 'r', 'R', 'o', 'O'], seed, 1.8, 0.1, &keep);
    paint::mouth(im, 23, h - 12, 6, 7, work, t);
    if work {
        paint::smoke(im, 19, 12, t, 14, 4, false);
        hands::stamp(im, 9, h - 2, Trade::Baker, Act::Work, t, false);
        for k in 0..3 {
            im.ellipse(6.0 + k as f32 * 3.0, (wall_y + 11) as f32, 1.8, 1.2, if (t + k) % 4 < 2 { 't' } else { 'O' });
        }
    }
    paint::pile(im, 27, h - 1, 7, 6, Load::Bread, seed);
    let _ = w;
}
pub(super) fn brewery(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // A big timber hall, a steep roof with a louvred vent at the ridge.
    let wall_y = paint::dwelling(im, 0, 10, 48, 18, h - 2 - 27, Front::Board, ramp::TILE, seed);
    im.rect(19, 3, 10, 8, 'B');
    for k in 0..3 {
        im.line(20, 4 + k * 2, 28, 4 + k * 2, 'k');
    }
    im.rect(18, 2, 12, 2, 'r');
    if work {
        paint::smoke(im, 24, 1, t, 12, 5, true);
    }
    // The copper kettle in the open bay: a riveted dome, a pipe to the vent.
    im.rect(6, wall_y + 4, 22, h - 6 - wall_y, 'K');
    let foot = h - 4;
    let keep = |_: i32, _: i32| true;
    material::lobe(im, (16.0, (foot - 9) as f32), (9.5, 9.5), &['I', 'p', 'R', 'o', 'O', 't'], seed, 3.0, 0.05, &keep);
    for k in 0..6 {
        im.put(9 + k * 3, foot - 8, 'T');
    }
    im.rect(8, foot - 12, 17, 1, 'p');
    im.line(24, foot - 14, 25, wall_y + 3, 'o');
    if work {
        for k in 0..3 {
            let x = 11 + k * 5;
            im.put(x, foot - 20 - ((t + k as u32 * 3) % 4) as i32, 'W');
        }
        hands::stamp(im, 29, h - 3, Trade::Brewer, Act::Bare(Pose::Slash), t, true);
    }
    // Barrels stacked by the door.
    paint::door(im, 34, h - 3, 7, 13);
    for (x, y) in [(43, h - 1), (47, h - 1), (45, h - 9)] {
        material::barrel(im, x, y);
    }
    paint::pile(im, 8, h - 1, 11, 7, Load::Ale, seed);
    im.line(2, wall_y + 2, 2, wall_y + 12, 'o');
    im.rect(0, wall_y + 12, 5, 4, 'r');
    let _ = w;
}
pub(super) fn weaving_shed(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // A shed with its front open: a boarded back in shadow, thatch over it.
    let wall_y = paint::dwelling(im, 0, 6, 34, 16, h - 2 - 21, Front::Board, ramp::THATCH, seed);
    im.rect(4, wall_y + 2, 26, h - 6 - wall_y, 'K');
    for x in [3, 31] {
        paint::post(im, x, wall_y, h - 3);
    }
    // The loom: a tall frame strung with warp, cloth woven in stripes at its foot.
    im.rect(8, wall_y + 2, 2, h - 8 - wall_y, 'O');
    im.rect(24, wall_y + 2, 2, h - 8 - wall_y, 'O');
    im.rect(8, wall_y + 2, 18, 2, 'O');
    im.rect(8, h - 8, 18, 2, 'B');
    for x in 11..24 {
        im.line(x, wall_y + 4, x, h - 9, if x % 2 == 0 { 'T' } else { 't' });
    }
    let shuttle = if work { 11 + ((t * 2) % 13) as i32 } else { 14 };
    let cloth_top = h - 18 - if work { (t / 4) as i32 } else { 0 };
    for y in cloth_top..h - 9 {
        for x in 11..24 {
            im.put(x, y, match ((y - cloth_top) / 2) % 3 { 0 => ')', 1 => ']', _ => 'Q' });
        }
    }
    im.rect(shuttle, cloth_top - 2, 4, 2, 'B');
    im.line(11, cloth_top - 1, 24, cloth_top - 1, 'o');
    if work {
        hands::stamp(im, 17, h - 2, Trade::Roper, Act::Bare(Pose::Slash), t, false);
    }
    // Dyed cloth hung on a line outside: violet, blue, ochre.
    im.line(36, h - 40, 47, h - 40, 'o');
    im.line(36, h - 40, 36, h - 4, 'B');
    for (k, col) in [')', 'Q', 'O', 'z'].into_iter().enumerate() {
        let x = 37 + (k as i32 % 2) * 5;
        let y = h - 39 + (k as i32 / 2) * 14;
        im.rect(x, y, 5, 12, col);
        im.line(x, y + 11, x + 4, y + 11, 'b');
    }
    paint::wheel(im, 40.0, (h - 6) as f32, 4.0, if work { paint::turn(t) } else { 0.0 }, 6, false, ('o', 'O', 'B'));
    paint::pile(im, 8, h - 1, 9, 7, Load::Cloth, seed);
    let _ = w;
}
