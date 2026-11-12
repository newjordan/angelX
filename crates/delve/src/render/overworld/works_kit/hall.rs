//! Halls and housing, the yard, the quay, and the stone and brick of the wall.

use super::hands::{self, Act, Trade};
use super::{Ctx, Load, paint};
use paint::Front;
use crate::render::overworld::ink::Img;
use crate::render::overworld::material::{self, ramp};

/// Merlons along a wall's top: `n` teeth of `w` across.
fn merlons(im: &mut Img, x: i32, y: i32, n: i32, w: i32, ramp: &[char]) {
    for k in 0..n {
        let mx = x + k * (w * 2);
        im.rect(mx, y, w, 4, ramp[ramp.len() - 2]);
        im.rect(mx, y, w, 1, ramp[ramp.len() - 1]);
        im.rect(mx + w - 1, y + 1, 1, 3, ramp[1]);
    }
}

pub(super) fn lodge(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 32.0, (h - 11) as f32, 31.0, 10.5, seed);
    // A long hall: coursed stone under a steep timber roof, its gable to the front.
    paint::stone(im, 3, h - 33, 58, 31, seed);
    paint::roof(im, 0, 16, 64, 14, ramp::TIMBER, seed);
    // The gable's pediment carries the lodge's mark: a square and compasses.
    im.rect(24, 4, 16, 13, 'B');
    for k in 0..5 {
        im.line(24 + k, 16 - k, 40 - k, 16 - k, 'b');
    }
    im.line(29, 14, 32, 6, 'W');
    im.line(35, 14, 32, 6, 'W');
    im.line(28, 11, 36, 11, 'V');
    im.put(32, 5, '5');
    // Lit windows, a great door, mason's marks cut in the jambs.
    paint::window(im, 8, h - 25, 5, 7, work);
    paint::window(im, 18, h - 25, 5, 7, work);
    paint::window(im, 41, h - 25, 5, 7, work);
    paint::window(im, 51, h - 25, 5, 7, work);
    paint::door(im, 28, h - 3, 9, 16);
    for (k, x) in [27, 38].into_iter().enumerate() {
        im.put(x, h - 12 + k as i32, 'W');
        im.put(x, h - 11 + k as i32, 'W');
        im.put(x + 1, h - 12 + k as i32, 'V');
    }
    paint::chimney(im, 52, 18, 6, 12);
    if work {
        paint::smoke(im, 55, 4, t, 14, 4, false);
        hands::stamp(im, 13, h - 2, Trade::Setter, Act::Walk, t, false);
        hands::stamp(im, 48, h - 2, Trade::MasterMason, Act::Idle, 0, true);
    }
    paint::banner(im, 61, 6, h - 12, t * u32::from(work), 'Q', 'T');
    paint::pile(im, 6, h - 1, 10, 6, Load::Ashlar, seed);
    let _ = w;
}

pub(super) fn depot(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // A gravelled yard behind a rail fence, a lean-to shed on posts at the back.
    paint::earth(im, 24.0, (h - 13) as f32, 23.0, 12.5, seed);
    for k in 0..30 {
        im.put(4 + (k * 7) % 40, h - 22 + (k * 5) % 18, if k % 3 == 0 { 'h' } else { 'G' });
    }
    for x in [3, 44] {
        paint::post(im, x, h - 38, h - 18);
    }
    paint::post(im, 24, h - 38, h - 18);
    paint::lean_roof(im, 0, 3, 48, 12, ramp::THATCH, seed);
    // Under it: crates, sacks, barrels in rows.
    paint::pile(im, 10, h - 19, 12, 8, Load::Crate, seed);
    paint::pile(im, 33, h - 19, 14, 9, Load::Flour, seed ^ 1);
    material::barrel(im, 22, h - 18);
    // A signpost with arrows both ways, a lamp.
    im.line(45, h - 8, 45, h - 24, 'o');
    im.rect(40, h - 24, 8, 3, 'O');
    im.rect(40, h - 20, 7, 3, 'T');
    // The hand-cart and the carrier moving crates across the yard.
    im.rect(4, h - 8, 10, 5, 'o');
    im.line(2, h - 9, 14, h - 9, 'B');
    im.ellipse(8.0, (h - 3) as f32, 2.5, 2.5, 'b');
    im.put(8, h - 3, 'V');
    if work {
        let x = 16 + ((paint::turn(t).sin() + 1.0) * 10.0) as i32;
        hands::stamp(im, x, h - 2, Trade::Carter, Act::Carry(Load::Crate), t, false);
    } else {
        hands::stamp(im, 20, h - 2, Trade::Carter, Act::Idle, 0, false);
    }
    let _ = w;
}

pub(super) fn quay(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // Water along the foot of the picture, the quay wall rising from it.
    paint::stream(im, 0, h - 14, w, 14, if work { t } else { 0 }, 1, seed);
    paint::blockish(im, 0, h - 30, w, 18, ramp::STONE, seed);
    // The paving on top, bollards along the edge.
    for x in (4..w - 4).step_by(14) {
        im.rect(x, h - 33, 4, 4, 'u');
        im.rect(x, h - 33, 4, 1, 'v');
    }
    // A jib crane on the quay head: a post, a swinging arm, the hook and a crate.
    im.rect(38, h - 49, 3, 20, 'o');
    im.rect(39, h - 49, 1, 20, 'O');
    let swing = if work { ((paint::turn(t).sin() + 1.0) * 6.0) as i32 } else { 8 };
    im.line(39, h - 49, 22 + swing, h - 42, 'O');
    im.line(39, h - 48, 22 + swing, h - 41, 'B');
    im.line(39, h - 37, 30, h - 44, 'b');
    let hook = if work { h - 36 + ((paint::turn(t).cos() + 1.0) * 3.0) as i32 } else { h - 34 };
    paint::rope(im, 22 + swing, h - 41, 22 + swing, hook, 0.0, 't');
    im.rect(19 + swing, hook, 7, 6, 'r');
    im.rect(19 + swing, hook, 7, 1, 'O');
    im.line(19 + swing, hook, 25 + swing, hook + 5, 'b');
    paint::pile(im, 8, h - 31, 12, 9, Load::Crate, seed);
    paint::pile(im, 4, h - 15, 10, 6, Load::Planks, seed ^ 1);
    // A moored boat's bow, a mooring line.
    im.ellipse(12.0, (h - 6) as f32, 11.0, 3.5, 'B');
    im.ellipse(12.0, (h - 7) as f32, 10.0, 2.5, 'o');
    paint::rope(im, 8, h - 31, 8, h - 8, 1.0, 't');
    if work {
        hands::stamp(im, 30, h - 30, Trade::Bargeman, Act::Idle, 0, false);
    }
    let _ = w;
}

pub(super) fn hut(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 6) as f32, 15.5, 5.5, seed);
    // Wattle and daub under a low thatch, a smoke hole in the roof.
    im.rect(3, h - 12, 26, 11, 't');
    for x in 3..29 {
        if x % 4 == 0 {
            im.line(x, h - 12, x, h - 2, 'o');
        }
    }
    im.rect(3, h - 12, 26, 1, 'O');
    paint::roof(im, 0, 3, 32, h - 13 - 3, ramp::THATCH, seed);
    paint::door(im, 12, h - 2, 6, 9);
    paint::window(im, 22, h - 10, 4, 4, work);
    if work {
        paint::smoke(im, 17, 2, t, 14, 3, false);
    }
    let _ = w;
}

pub(super) fn cottage(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 6) as f32, 15.5, 5.5, seed);
    let wall_y = paint::dwelling(im, 0, 8, w, 15, h - 2 - 22, Front::Stone, ramp::THATCH, seed);
    let foot = h - 3;
    paint::door(im, 6, foot, 6, 11);
    paint::window(im, 17, wall_y + 4, 5, 5, work);
    paint::chimney(im, 22, 16, 4, 12);
    // A flower box under the window.
    im.rect(16, wall_y + 10, 7, 2, 'o');
    for x in [17, 19, 21] {
        im.put(x, wall_y + 9, 'm');
        im.put(x, wall_y + 8, if x % 2 == 0 { '4' } else { 'y' });
    }
    if work {
        paint::smoke(im, 24, 5, t, 14, 3, false);
        // Washing on a line that stirs.
        im.line(27, h - 3, 27, h - 15, 'o');
        let sway = (paint::turn(t).sin() * 1.2).round() as i32;
        im.rect(28 + sway.max(0), h - 15, 3, 5, 'H');
    }
}
pub(super) fn warehouse(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 32.0, (h - 10) as f32, 31.0, 9.5, seed);
    // A big timber hall: heavy boards, a steep roof, a wide double door.
    paint::boards(im, 3, h - 34, 58, 32, seed);
    paint::roof(im, 0, 14, 64, 15, ramp::TILE, seed);
    paint::door(im, 24, h - 3, 18, 20);
    im.line(33, h - 22, 33, h - 3, 'o');
    for k in 0..4 {
        im.line(25, h - 18 + k * 4, 31, h - 14 + k * 4, 'B');
        im.line(35, h - 14 + k * 4, 41, h - 18 + k * 4, 'B');
    }
    // The loft door and the hoist: a beam, a rope, a crate going up and down.
    im.rect(44, 22, 9, 10, 'k');
    im.line(44, 20, 58, 20, 'O');
    im.line(44, 21, 58, 21, 'B');
    let lift = if work { ((paint::turn(t).cos() + 1.0) * 8.0).round() as i32 } else { 16 };
    paint::rope(im, 57, 21, 57, 24 + lift, 0.0, 't');
    im.rect(53, 24 + lift, 8, 6, 'r');
    im.rect(53, 24 + lift, 8, 1, 'O');
    paint::pile(im, 10, h - 1, 14, 9, Load::Flour, seed);
    material::barrel(im, 50, h - 1);
    material::barrel(im, 56, h - 1);
    if work {
        hands::stamp(im, 20, h - 2, Trade::Carter, Act::Carry(Load::Grain), t, false);
    }
    let _ = w;
}

pub(super) fn caravanserai(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 40.0, (h - 11) as f32, 39.0, 10.5, seed);
    // A long inn of stone: a low arcaded front, a tiled roof, a tower at one end.
    let wall_y = paint::dwelling(im, 0, 16, 66, 16, h - 2 - 31, paint::Front::Stone, ramp::TILE, seed);
    let foot = h - 3;
    for k in 0..4 {
        let x = 6 + k * 15 + if k >= 2 { 8 } else { 0 };
        paint::door(im, x, foot, 7, 14);
        paint::window(im, x + 1, wall_y + 8, 5, 6, work);
    }
    // The gate: a great arch through the middle, the court dark beyond it, a lamp hung in it.
    let gx = 32;
    im.rect(gx - 8, foot - 25, 16, 26, 'k');
    im.ellipse(gx as f32, (foot - 25) as f32, 8.0, 4.0, 'k');
    im.line(gx - 9, foot - 26, gx - 9, foot, 'h');
    im.line(gx + 8, foot - 26, gx + 8, foot, 'g');
    for k in 0..4 {
        im.put(gx - 6 + k * 4, foot - 28 + (k % 2), 'i');
    }
    im.rect(gx - 1, foot - 20, 2, 3, if work { '5' } else { 'S' });
    if work {
        im.put(gx - 1 + (t % 2) as i32, foot - 21, '6');
        for k in 0..6 {
            im.put(gx - 3 + k, foot - 12, 'a');
        }
    }
    // The tower and its flag.
    paint::stone(im, 66, 8, 13, h - 10, seed ^ 1);
    paint::roof(im, 65, 2, 15, 8, ramp::SLATE, seed);
    paint::banner(im, 72, 0, 3, t * u32::from(work), ')', '5');
    paint::window(im, 70, 24, 4, 7, work);
    // A wagon in the yard, its tilt of cloth.
    im.rect(68, h - 12, 10, 7, 'O');
    im.rect(68, h - 12, 10, 1, 'T');
    im.rect(69, h - 19, 8, 7, 'H');
    im.rect(69, h - 19, 8, 1, 'h');
    for x in [71.0f32, 75.0] {
        paint::wheel(im, x, (h - 5) as f32, 3.0, if work { paint::turn(t) * 2.0 / 6.0 } else { 0.0 }, 6, false, ('B', 'o', 'U'));
    }
    paint::pile(im, 56, h - 1, 11, 7, Load::Crate, seed);
    if work {
        hands::stamp(im, 48, h - 2, Trade::Carter, Act::Walk, t, false);
    }
    let _ = w;
}
pub(super) fn wall(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 3) as f32, 7.5, 3.0, seed);
    // A length of curtain: coursed ashlar, a string course, crenellated.
    paint::stone(im, 0, h - 12, 16, 11, seed);
    paint::stone(im, 0, 5, 16, h - 18, seed);
    merlons(im, 0, 1, 4, 3, material::ramp::STONE);
    im.rect(0, 4, 16, 2, 'J');
    im.rect(0, 6, 16, 1, 'g');
    im.rect(0, h - 13, 16, 2, 'h');
    // A cresset on the wall-walk, burning when it is manned.
    if work {
        paint::flame(im, 12, 4, 3, t, seed);
    } else {
        im.rect(11, 3, 3, 2, 'k');
    }
    let _ = w;
}

pub(super) fn brick_cottage(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 6) as f32, 15.5, 5.5, seed);
    let wall_y = paint::dwelling(im, 0, 9, w, 15, h - 2 - 23, Front::Brick, ramp::TILE, seed);
    paint::chimney(im, 4, 18, 4, 14);
    paint::door(im, 7, h - 3, 6, 11);
    paint::window(im, 18, wall_y + 4, 6, 6, work);
    im.rect(17, wall_y + 3, 1, 8, 'B');
    im.rect(24, wall_y + 3, 1, 8, 'B');
    if work {
        paint::smoke(im, 6, 7, t, 14, 3, false);
    }
}
pub(super) fn brick_wall(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 3) as f32, 7.5, 3.0, seed);
    paint::bricks(im, 0, 6, 16, h - 8, seed);
    // Stone coping along the top, a brick merlon rhythm.
    im.rect(0, 3, 16, 3, 'i');
    im.rect(0, 3, 16, 1, 'H');
    im.rect(0, 5, 16, 1, 'g');
    for k in 0..3 {
        paint::bricks(im, k * 6, 0, 4, 4, seed ^ k as u32);
    }
    if work {
        // A cresset on a bracket, burning when the wall is manned.
        im.rect(9, 9, 5, 2, 'U');
        im.rect(10, 8, 3, 1, 'u');
        paint::flame(im, 11, 7, 4, t, seed);
    }
    let _ = w;
}

pub(super) fn brick_gate(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 6) as f32, 15.5, 5.5, seed);
    // A brick archway between two piers, a stone voussoir ring, a portcullis in the throat.
    paint::bricks(im, 2, 6, 28, h - 8, seed);
    im.rect(2, 4, 28, 3, 'i');
    im.rect(2, 4, 28, 1, 'H');
    for k in 0..4 {
        paint::bricks(im, 3 + k * 7, 0, 5, 4, seed ^ k as u32);
    }
    im.rect(8, h - 24, 16, 23, 'k');
    im.ellipse(16.0, (h - 24) as f32, 8.0, 4.0, 'k');
    for k in 0..12 {
        let a = k as f32 / 11.0 * std::f32::consts::PI;
        im.put(16 - (a.cos() * 9.5) as i32, h - 24 - (a.sin() * 5.0) as i32, 'v');
    }
    im.line(7, h - 24, 7, h - 2, 'i');
    im.line(24, h - 24, 24, h - 2, 'h');
    // The portcullis: iron bars, drawn up into the arch when the gate is worked.
    let up = if work { ((paint::turn(t).cos() + 1.0) * 8.0).round() as i32 } else { 0 };
    for x in (9..24).step_by(3) {
        im.line(x, h - 25, x, h - 3 - up, 'u');
    }
    for y in (h - 22..h - 3).step_by(5) {
        if y <= h - 3 - up {
            im.line(9, y - up, 23, y - up, 'U');
        }
    }
    let _ = w;
}
