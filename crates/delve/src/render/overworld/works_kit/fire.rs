//! The fire trades: kilns, clamps, furnaces, forges.

use super::hands::{self, Act, Pose, Trade};
use super::{Ctx, Load, paint};
use crate::render::overworld::material;

pub(super) fn lime_kiln(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 11) as f32, 15.5, 10.0, seed);
    // The bank the kiln is dug into: spoil heaped to either side.
    paint::spoil(im, 4, h - 14, 6.0, 7.0, seed);
    paint::spoil(im, w - 5, h - 14, 6.0, 7.0, seed ^ 1);
    // The pot: a squat tapering stack of rough stone, wider at its foot.
    let (top, foot) = (20, h - 6);
    paint::trapezoid(im, top, foot, (9.0, 23.0), (5.0, 27.0), &|x, y, side| {
        let shade = side > 0 || y > foot - 3;
        let n = crate::render::overworld::ink::hash(x / 2, y / 3, seed) % 5;
        match (side < 0, shade, n) {
            (true, _, _) => 'J',
            (_, true, _) => 'g',
            (_, _, 0) => 'h',
            (_, _, 1) => 'j',
            (_, _, 2) => 'g',
            _ => 'G',
        }
    });
    // Courses of iron hoops: a kiln is tied round.
    for y in [28, 36] {
        let inset = ((y - top) as f32 / (foot - top) as f32 * 4.0) as i32;
        im.line(9 - inset, y, 23 + inset, y, 'u');
    }
    // The top: a stone rim round the charging hole, dark, or red-hot.
    im.ellipse(16.0, 20.0, 7.0, 2.5, 'h');
    im.ellipse(16.0, 20.0, 5.5, 1.8, if work { '7' } else { 'k' });
    if work {
        im.ellipse(16.0, 20.0, 3.5, 1.0, '@');
        paint::flame(im, 16, 20, 5, t, seed);
        paint::smoke(im, 17, 14, t, 14, 4, true);
    }
    // The draw-arch at its foot: the fire's mouth.
    paint::mouth(im, 12, foot - 11, 8, 11, work, t);
    im.line(11, foot - 12, 20, foot - 12, 'i');
    if work {
        // Light spilling out on the ground.
        for k in 0..6 {
            im.put(12 + k, foot + 1, if k % 2 == 0 { '4' } else { 'a' });
        }
    } else {
        paint::embers(im, 13, foot - 1, 6, seed);
    }
    // The burner at the mouth with his rake, when it burns.
    if work {
        hands::stamp(im, 27, h - 3, Trade::LimeBurner, Act::Work, t, true);
    }
    // Limestone waiting to be burnt, and the white lime that comes out.
    paint::pile(im, 7, h - 1, 11, 7, Load::Ashlar, seed);
    paint::pile(im, 25, h - 1, 11, 7, Load::Lime, seed);
}

pub(super) fn clamp(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // Burnt ground: soot round the clamp.
    paint::earth(im, 16.0, (h - 10) as f32, 15.5, 9.0, seed);
    for k in 0..24 {
        let (x, y) = (3 + (k * 7) % 26, h - 6 + (k * 5) % 6);
        im.put(x, y, if k % 2 == 0 { 'K' } else { 'X' });
    }
    // The dome of turf over the stacked wood.
    let keep = |_: i32, _: i32| true;
    material::lobe(im, (16.0, (h - 16) as f32), (13.5, 9.5), &['b', 'B', 'F', 'E', 'l', 'm'], seed, 2.2, 0.28, &keep);
    // Turf sods in lines, and the earth between.
    for k in 0..16 {
        let (x, y) = (5 + (k * 5) % 22, h - 24 + (k * 3) % 14);
        if im.get(x, y).is_some() {
            im.put(x, y, 'N');
            im.put(x + 1, y, 'B');
        }
    }
    // Logs showing at the foot, ends out.
    for k in 0..6 {
        let x = 5 + k * 4;
        im.ellipse(x as f32, (h - 6) as f32, 1.6, 1.6, 'o');
        im.put(x, h - 6, 't');
    }
    // The chimney hole at the crown, and the vents round the skirt.
    im.rect(14, h - 26, 4, 2, 'k');
    for (x, y) in [(8, h - 14), (16, h - 11), (24, h - 14)] {
        im.rect(x, y, 2, 2, if work { '7' } else { 'k' });
        if work {
            im.put(x, y, '@');
        }
    }
    if work {
        paint::smoke(im, 16, h - 26, t, 16, 5, true);
        for (i, x) in [8, 24].into_iter().enumerate() {
            paint::smoke(im, x, h - 16, t + 2 + i as u32 * 3, 7, 2, true);
        }
    }
    // The collier's lean-to: poles and turf, his rake, a sack of charcoal.
    im.line(1, h - 3, 3, h - 14, 'o');
    im.line(7, h - 3, 3, h - 14, 'o');
    if work {
        hands::stamp(im, 6, h - 2, Trade::Collier, Act::Work, t, false);
    }
    paint::pile(im, 27, h - 1, 9, 8, Load::Charcoal, seed);
    let _ = w;
}

pub(super) fn bloomery(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 11) as f32, 15.5, 11.0, seed);
    // The stack: a tall clay-rendered shaft, tapering, hooped with iron.
    let (top, foot) = (8, h - 8);
    paint::trapezoid(im, top, foot, (11.0, 21.0), (7.0, 25.0), &|x, y, side| {
        let n = crate::render::overworld::ink::hash(x, y / 3, seed) % 6;
        match side {
            -1 => 'O',
            1 => 'p',
            _ if x < 15 => 'o',
            _ if x > 19 => 'r',
            _ if n == 0 => 'R',
            _ => 'o',
        }
    });
    for y in (top + 7..foot - 2).step_by(9) {
        let inset = ((foot - y) as f32 / (foot - top) as f32 * 4.0) as i32;
        im.line(11 - inset, y, 21 + inset, y, 'u');
        im.line(11 - inset, y + 1, 21 + inset, y + 1, 'S');
    }
    // The throat at the top: dark, or fire standing in it.
    im.rect(12, top - 1, 8, 2, 'h');
    im.rect(13, top, 6, 1, if work { '7' } else { 'k' });
    if work {
        paint::flame(im, 16, top, 6, t, seed);
        paint::smoke(im, 17, top - 8, t, 12, 4, false);
    }
    // The tuyere hole at its foot and the bellows pumping into it.
    let pump = if work { ((paint::turn(t).sin() + 1.0) * 2.0).round() as i32 } else { 0 };
    im.rect(13, foot - 8, 6, 6, 'k');
    if work {
        im.rect(14, foot - 6, 4, 3, '@');
        im.put(15, foot - 5, '6');
    }
    // Bellows: two boards hinged at the stack, leather folds between them.
    let bx = 0;
    for k in 0..pump + 5 {
        im.put(bx + 2 + k % 11, foot - 4 - (k % 2), 'B');
    }
    im.rect(bx + 1, foot - 9 - pump, 10, 2, 'o');
    im.rect(bx + 1, foot - 3, 10, 2, 'o');
    for y in foot - 7 - pump..foot - 3 {
        for x in bx + 2..bx + 10 {
            im.put(x, y, if (x + y) % 3 == 0 { 'I' } else { 'B' });
        }
    }
    im.line(bx + 11, foot - 5, 13, foot - 5, 'U');
    // The lever and the bloomer on it.
    if work {
        hands::stamp(im, 2, foot + 4, Trade::Bloomer, Act::Bare(Pose::Overhead), t, false);
    }
    // Ore to feed it, charcoal, and slag that comes out.
    paint::pile(im, 28, h - 1, 10, 7, Load::Ore, seed);
    paint::pile(im, 17, h - 1, 9, 6, Load::Charcoal, seed ^ 5);
    for k in 0..7 {
        let (x, y) = (4 + (k * 3) % 8, h - 2 - (k % 3));
        im.put(x, y, if k % 3 == 0 { 'u' } else { 'k' });
        im.put(x + 1, y, 'X');
    }
    let _ = w;
}

pub(super) fn ashery(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 13) as f32, 15.5, 12.0, seed);
    // The burn: logs and brush stacked in a cone, a grey crust of ash round its foot, fire in its mouth.
    for k in 0..6 {
        let y = h - 7 - k * 3;
        let half = 8 - k;
        im.line(2 + k, y, 2 + k + half, y - 3, 'B');
        im.line(18 - k, y, 18 - k - half, y - 3, 'P');
    }
    for k in 0..9 {
        let (x, y) = (3 + (k * 5) % 13, h - 5 - (k * 3) % 5);
        im.rect(x, y, 3, 2, if k % 2 == 0 { 'G' } else { 'J' });
    }
    im.rect(7, h - 12, 5, 6, if work { '7' } else { 'k' });
    if work {
        paint::flame(im, 9, h - 7, 5, t, seed);
        paint::smoke(im, 10, h - 20, t, 14, 4, false);
    } else {
        paint::embers(im, 8, h - 8, 3, seed);
    }
    // The leaching vats: two broad tubs on a stand, a spigot each, a pot below catching the lye.
    for (k, x) in [19, 25].into_iter().enumerate() {
        im.rect(x, h - 28, 6, 12, 'r');
        im.rect(x, h - 28, 6, 1, 'O');
        im.rect(x + 1, h - 27, 4, 1, 'T');
        for y in [h - 25, h - 20] {
            im.line(x, y, x + 5, y, 'U');
        }
        im.rect(x - 1, h - 16, 8, 2, 'o');
        im.rect(x + 1, h - 14, 1, 7, 'B');
        im.rect(x + 4, h - 14, 1, 7, 'B');
        im.rect(x + 1, h - 8, 4, 4, 'u');
        im.rect(x + 1, h - 8, 4, 1, 'U');
        let drop = ((t + k as u32 * 4) % 8) as i32;
        if work && drop < 5 {
            im.put(x + 3, h - 14 + drop, 'z');
        }
        if work {
            im.put(x + 2 + (drop % 2), h - 9, 'z');
        }
    }
    // Potash: grey-white cakes heaped at the edge.
    paint::pile(im, 8, h - 1, 10, 6, Load::Glass, seed);
    if work {
        hands::stamp(im, 14, h - 2, Trade::Labourer, Act::Work, t, false);
    }
    let _ = w;
}
pub(super) fn glassworks(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // The beehive furnace: a dome of clay brick, glory holes in its flank.
    let keep = |_: i32, _: i32| true;
    material::lobe(im, (18.0, (h - 22) as f32), (17.0, 17.0), &['b', 'p', 'r', 'R', 'o', 'O'], seed, 2.0, 0.1, &keep);
    for k in 0..4 {
        paint::bricks(im, 6 + k * 3, h - 14 + (k % 2) * 3, 5, 3, seed ^ k as u32);
    }
    for (i, (x, y)) in [(10, h - 20), (22, h - 20), (16, h - 11)].into_iter().enumerate() {
        paint::mouth(im, x, y, 5, 5, work, t + i as u32);
        im.line(x - 1, y - 1, x + 5, y - 1, 'U');
    }
    // The stack: a tall brick cone beside it, smoking.
    paint::trapezoid(im, 4, h - 14, (34.0, 40.0), (32.0, 43.0), &|x, y, side| {
        if side < 0 { 'o' } else if side > 0 { 'p' } else if (x + y) % 4 == 0 { 'B' } else { 'R' }
    });
    im.rect(34, 3, 7, 2, 'h');
    if work {
        paint::smoke(im, 37, 2, t, 14, 4, false);
    }
    // The blower at the glory hole with his pipe, a gather of glass swelling on it.
    if work {
        let g = hands::stamp(im, 36, h - 3, Trade::Smith, Act::Bare(hands::Pose::Thrust), t, true);
        im.line(g.0, g.1, g.0 - 12, g.1 + 1, 'U');
        let r = 1.6 + (paint::turn(t).sin() + 1.0) * 0.9;
        im.ellipse((g.0 - 13) as f32, (g.1 + 1) as f32, r, r, '6');
        im.ellipse((g.0 - 13) as f32, (g.1 + 1) as f32, (r - 1.0).max(0.6), (r - 1.0).max(0.6), '5');
    } else {
        im.line(30, h - 6, 40, h - 6, 'U');
    }
    paint::pile(im, 8, h - 1, 12, 7, Load::Sand, seed);
    paint::pile(im, 44, h - 1, 10, 8, Load::Glass, seed ^ 4);
    let _ = w;
}

pub(super) fn smithy(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 11) as f32, 15.5, 10.5, seed);
    // An open-fronted timber shed with a lean-to roof and a stone chimney.
    paint::boards(im, 2, h - 38, 28, 28, seed);
    im.rect(4, h - 34, 24, 26, 'K');
    paint::lean_roof(im, 0, 4, 32, 12, material::ramp::TIMBER, seed);
    paint::chimney(im, 24, 16, 5, 14);
    // On the back wall: the tools the trade is, hung on pegs.
    for (k, x) in [7, 12, 17].into_iter().enumerate() {
        im.line(x, h - 32, x, h - 27, 'B');
        match k {
            0 => im.rect(x - 2, h - 27, 5, 2, 'V'),
            1 => im.rect(x - 1, h - 27, 3, 5, 'U'),
            _ => im.line(x - 2, h - 27, x + 2, h - 22, 'V'),
        }
    }
    // The hearth, its hood and the glow.
    im.rect(20, h - 18, 8, 9, 'G');
    im.rect(20, h - 18, 8, 1, 'h');
    im.rect(21, h - 15, 6, 4, if work { '@' } else { 'k' });
    if work {
        im.rect(22, h - 14, 4, 2, '6');
        paint::flame(im, 24, h - 15, 4, t, seed);
        paint::smoke(im, 26, 6, t, 14, 3, false);
    } else {
        paint::embers(im, 22, h - 12, 4, seed);
    }
    // The anvil on its stump, the quench trough, the smith.
    im.rect(10, h - 10, 6, 6, 'B');
    im.rect(8, h - 13, 12, 3, 'U');
    im.rect(8, h - 13, 12, 1, 'W');
    im.rect(18, h - 12, 3, 2, 'u');
    im.rect(21, h - 7, 9, 4, 'r');
    im.rect(22, h - 7, 7, 2, 'z');
    if work {
        hands::stamp(im, 6, h - 3, Trade::Smith, Act::Work, t, false);
        if t % 4 < 2 {
            for k in 0..4 {
                im.put(14 + k * 2 - (t % 2) as i32, h - 15 - (k * 3) % 5, '6');
            }
        }
    }
    let _ = w;
}

pub(super) fn forge_shop(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 11) as f32, 23.0, 10.5, seed);
    // A closed stone forge hall, its slate roof and a great chimney.
    let wall_y = paint::dwelling(im, 0, 12, 48, 16, h - 2 - 27, paint::Front::Stone, material::ramp::SLATE, seed);
    paint::chimney(im, 36, 24, 8, 22);
    if work {
        paint::smoke(im, 40, 0, t, 16, 4, false);
    }
    // The wide arched bay: the hearth blazing inside, bellows beside.
    let foot = h - 3;
    paint::mouth(im, 8, foot - 21, 22, 22, work, t);
    im.line(7, foot - 22, 30, foot - 22, 'i');
    if work {
        im.ellipse(19.0, (foot - 9) as f32, 8.0, 5.0, '6');
        paint::flame(im, 19, foot - 7, 7, t, seed);
    } else {
        paint::embers(im, 12, foot - 2, 14, seed);
    }
    // The tilt-hammer on the right: a great helve on its post, the iron head over an anvil.
    let swing = if work { ((paint::turn(t).sin() + 1.0) * 3.5).round() as i32 } else { 5 };
    im.rect(40, foot - 14, 6, 15, 'B');
    im.line(33, foot - 18 - swing, 45, foot - 15, 'o');
    im.line(33, foot - 17 - swing, 45, foot - 14, 'B');
    im.rect(31, foot - 17 - swing, 5, 4, 'U');
    im.rect(31, foot - 17 - swing, 5, 1, 'W');
    im.rect(31, foot - 6, 7, 3, 'U');
    im.rect(30, foot - 3, 9, 2, 'B');
    if work && swing < 2 {
        for k in 0..4 {
            im.put(30 + k * 2, foot - 8 - k % 3, '6');
        }
    }
    if work {
        hands::stamp(im, 22, h - 2, Trade::Smith, Act::Work, t, false);
    }
    paint::pile(im, 4, h - 1, 9, 6, Load::Iron, seed);
    let _ = (w, wall_y);
}
pub(super) fn lead_works(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 12) as f32, 15.5, 11.0, seed);
    // A low smelting hearth under a conical hood, its flue taking the fume up.
    paint::blockish(im, 4, h - 22, 18, 14, material::ramp::STONE, seed);
    paint::trapezoid(im, 6, h - 20, (12.0, 14.0), (6.0, 20.0), &|x, y, side| {
        if side < 0 { 'u' } else if side > 0 { 'S' } else if (x + y) % 5 == 0 { 'S' } else { 'U' }
    });
    im.rect(10, 4, 6, 2, 'v');
    im.rect(8, h - 16, 6, 6, 'k');
    if work {
        im.rect(9, h - 14, 4, 3, '@');
        paint::flame(im, 11, h - 13, 3, t, seed);
        paint::smoke(im, 13, 4, t, 12, 4, true);
    } else {
        paint::embers(im, 9, h - 11, 4, seed);
    }
    // The channel and the moulds in the sand: a pour running, ingots cooling.
    for k in 0..3 {
        im.rect(24, h - 10 + k * 3, 6, 2, 'u');
        im.rect(25, h - 10 + k * 3, 4, 1, if work && k as u32 == t % 3 { '@' } else { 'U' });
    }
    if work {
        for k in 0..4 {
            im.put(20 + k, h - 12 + k / 2, if k % 2 == 0 { '@' } else { '5' });
        }
        hands::stamp(im, 10, h - 3, Trade::Bloomer, Act::Work, t, false);
    }
    paint::pile(im, 8, h - 1, 10, 6, Load::Ore, seed);
    paint::pile(im, 26, h - 1, 10, 6, Load::Lead, seed ^ 1);
    let _ = w;
}

pub(super) fn brick_kiln(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 24.0, (h - 12) as f32, 23.0, 11.5, seed);
    // A kiln of unburnt bricks stacked on a plastered base, fire-holes at its foot.
    paint::bricks(im, 4, h - 36, 32, 24, seed);
    im.rect(4, h - 36, 32, 2, 'O');
    for k in 0..4 {
        im.rect(4, h - 36 + k * 6, 1, 6, 'O');
        im.rect(35, h - 36 + k * 6, 1, 6, 'b');
    }
    // A daub of clay over the top, vents smoking through it.
    im.rect(3, h - 39, 34, 4, 'r');
    im.rect(3, h - 39, 34, 1, 'o');
    for x in [10, 20, 30] {
        im.rect(x, h - 41, 3, 2, 'k');
    }
    for (i, x) in [8, 17, 26].into_iter().enumerate() {
        paint::mouth(im, x, h - 20, 6, 8, work, t + i as u32 * 2);
        im.line(x - 1, h - 21, x + 6, h - 21, 'U');
    }
    // A brick chimney at its end.
    paint::chimney(im, 38, h - 12, 7, 38);
    if work {
        for x in [11, 21, 31] {
            paint::smoke(im, x, h - 42, t + x as u32, 10, 3, false);
        }
        paint::smoke(im, 41, h - 52, t, 14, 4, false);
    }
    paint::pile(im, 42, h - 1, 10, 8, Load::Brick, seed);
    if work {
        hands::stamp(im, 20, h - 2, Trade::Labourer, Act::Work, t, false);
    }
    let _ = w;
}

pub(super) fn royal_forge(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 32.0, (h - 11) as f32, 31.0, 10.5, seed);
    // A grand hall of dark stone, a steep slate roof between two chimneys.
    let wall_y = paint::dwelling(im, 0, 14, 64, 17, h - 2 - 30, paint::Front::Stone, material::ramp::SLATE, seed);
    paint::chimney(im, 8, 24, 9, 24);
    paint::chimney(im, 47, 24, 9, 24);
    if work {
        paint::smoke(im, 12, 0, t, 16, 4, false);
        paint::smoke(im, 51, 0, t + 3, 16, 4, false);
    }
    // The great doors thrown wide: the hearth roaring in an arch.
    let foot = h - 3;
    paint::mouth(im, 22, foot - 26, 20, 27, work, t);
    im.line(20, foot - 27, 43, foot - 27, 'W');
    im.line(21, foot - 27, 21, foot, 'i');
    im.line(42, foot - 27, 42, foot, 'h');
    if work {
        im.ellipse(32.0, (foot - 11) as f32, 7.0, 5.0, '6');
    }
    // The royal banner over the roof, and a shield over the door.
    paint::banner(im, 32, 2, 16, t * u32::from(work), ')', '5');
    im.rect(29, wall_y + 3, 6, 5, ']');
    im.rect(30, wall_y + 4, 4, 3, '5');
    // Anvils and the smiths about them.
    for x in [8, 54] {
        im.rect(x - 3, foot - 6, 7, 3, 'U');
        im.rect(x - 3, foot - 6, 7, 1, 'W');
        im.rect(x - 1, foot - 3, 3, 4, 'B');
    }
    if work {
        hands::stamp(im, 13, h - 2, Trade::Smith, Act::Work, t, false);
        hands::stamp(im, 50, h - 2, Trade::Smith, Act::Work, t + 4, true);
        if t % 4 < 2 {
            for x in [5, 51] {
                for k in 0..3 {
                    im.put(x + k * 2, foot - 9 - k % 2, '6');
                }
            }
        }
    }
    let _ = w;
}
