//! Power and lifting: wheels, gins, mills, cranes, and the ley engines.

use super::hands::{self, Act, Pose, Trade};
use super::{Ctx, Load, paint};
use crate::render::overworld::ink::hash;
use crate::render::overworld::material::{self, ramp};

pub(super) fn waterwheel(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // The race: water across the front of the picture, the bank behind it.
    paint::earth(im, 24.0, 22.0 + 12.0, 23.0, 9.0, seed);
    paint::stream(im, 0, h - 14, w, 14, if work { t } else { 0 }, -1, seed);
    // Stone piers either side of the wheel, and its axle between them.
    let (cx, cy) = (17.0f32, (h - 22) as f32);
    for x in [1, 31] {
        for y in 0..24 {
            let ch = if y == 0 { 'h' } else if (y + x) % 5 == 0 { 'j' } else { 'G' };
            im.rect(x, h - 14 - 24 + y + 10, 6, 1, ch);
        }
    }
    // The wheel: eight spokes, paddles on its rim, its lower paddles in the water.
    let turn_by = if work { paint::turn(t) * 2.0 / 8.0 } else { 0.4 };
    paint::wheel(im, cx, cy, 13.0, turn_by, 8, true, ('o', 'O', 'B'));
    paint::ring(im, cx, cy, 6.0, 'B');
    // The wheel's foot in the water: spray and a shadow on the stream.
    for k in 0..6 {
        let x = 8 + k * 3;
        im.put(x, h - 14, if work && (k + t as i32) % 2 == 0 { 'W' } else { '3' });
    }
    // The gearbox shed on the right: the axle goes in through its wall, and a cog turns.
    paint::boards(im, 36, h - 36, 11, 20, seed);
    paint::roof(im, 34, h - 44, 15, 9, crate::render::overworld::material::ramp::SLATE, seed);
    im.line(23, cy as i32, 38, cy as i32, 'o');
    paint::wheel(im, 42.0, (h - 26) as f32, 4.0, if work { paint::turn(t) } else { 0.5 }, 8, false, ('U', 'U', 'V'));
    // Sacks at the door, waiting on what the power drives.
    paint::pile(im, 40, h - 1, 10, 6, Load::Flour, seed);
}

pub(super) fn treadwheel_crane(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 12) as f32, 15.5, 11.0, seed);
    // The base: a timber plate and the mast standing from it.
    im.rect(2, h - 7, 28, 3, 'O');
    im.rect(2, h - 4, 28, 1, 'B');
    for y in 9..h - 6 {
        im.put(25, y, 'o');
        im.put(26, y, 'O');
        im.put(27, y, 'B');
    }
    // The jib: a long arm to the left, its tie rope back to the mast's head.
    im.line(25, 9, 3, 14, 'o');
    im.line(25, 10, 3, 15, 'B');
    im.line(26, 8, 4, 12, 'O');
    im.line(26, 22, 14, 13, 'b');
    // The great treadwheel at the foot: a drum a man walks inside.
    let (cx, cy) = (13.0f32, (h - 19) as f32);
    let a = if work { paint::turn(t) * 2.0 / 8.0 } else { 0.2 };
    paint::wheel(im, cx, cy, 11.0, a, 8, false, ('o', 'B', 'O'));
    if work {
        hands::stamp(im, 15, h - 8, Trade::Labourer, Act::Bare(Pose::Walk), t, true);
    }
    // Rope up the mast, over the head, along the jib and down to the hook.
    let lift = if work { ((paint::turn(t).cos() + 1.0) * 5.0).round() as i32 } else { 12 };
    let hook = 22 + lift;
    paint::rope(im, 24, h - 22, 26, 9, 0.0, 't');
    paint::rope(im, 26, 9, 4, 13, 0.0, 't');
    paint::rope(im, 4, 14, 4, hook, 0.0, 't');
    // A cut block hanging in the sling.
    im.rect(1, hook, 7, 6, 'i');
    im.rect(1, hook, 7, 1, 'H');
    im.rect(1, hook + 5, 7, 1, 'j');
    im.line(4, hook - 1, 1, hook, 't');
    im.line(4, hook - 1, 7, hook, 't');
    paint::pile(im, 28, h - 1, 8, 7, Load::Ashlar, seed);
    let _ = w;
}

pub(super) fn mortar_mill(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 13) as f32, 15.5, 12.0, seed);
    // The pan: a round stone trough seen from the front and above, mortar in it.
    let (cx, cy) = (16.0f32, (h - 14) as f32);
    im.ellipse(cx, cy + 2.0, 14.0, 8.0, 'j');
    im.ellipse(cx, cy, 14.0, 8.0, 'J');
    im.ellipse(cx, cy, 12.0, 6.5, if work { 'o' } else { 'h' });
    im.ellipse(cx, cy, 9.0, 4.5, if work { 'P' } else { 'o' });
    // The post at its middle and the sweep that turns the runner round the pan.
    let a = if work { paint::turn(t) } else { 0.8 };
    let (rx, ry) = (a.cos() * 9.0, a.sin() * 4.0);
    let behind = a.sin() < 0.0;
    let wheel = |im: &mut crate::render::overworld::ink::Img| {
        let (wx, wy) = (cx + rx, cy + ry - 5.0);
        im.ellipse(wx, wy + 1.0, 4.5, 5.5, 'G');
        im.ellipse(wx, wy, 4.0, 5.0, 'J');
        im.ellipse(wx - 0.5, wy - 1.0, 2.0, 3.0, 'h');
    };
    if behind {
        wheel(im);
    }
    for y in (cy as i32 - 20)..(cy as i32) {
        im.put(cx as i32, y, 'o');
        im.put(cx as i32 + 1, y, 'B');
    }
    im.line(cx as i32, cy as i32 - 6, (cx + rx) as i32, (cy + ry) as i32 - 5, 'O');
    im.line(cx as i32, cy as i32 - 5, (cx + rx) as i32, (cy + ry) as i32 - 4, 'B');
    if !behind {
        wheel(im);
    }
    if work {
        hands::stamp(im, 3, h - 2, Trade::Labourer, Act::Work, t, false);
    }
    // Sand and lime beside it, the mortar tub.
    paint::pile(im, 26, h - 1, 10, 7, Load::Sand, seed);
    paint::pile(im, 6, h - 1, 9, 6, Load::Lime, seed ^ 3);
    let _ = w;
}

pub(super) fn intake(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 8) as f32, 15.5, 7.0, seed);
    // The river along the back, a stone wall holding its bank, the sluice cut through it.
    paint::stream(im, 0, 12, w, 10, if work { t } else { 0 }, 1, seed);
    paint::blockish(im, 0, 22, 12, 10, ramp::STONE, seed);
    paint::blockish(im, 20, 22, 12, 10, ramp::STONE, seed ^ 1);
    // The gate: a timber leaf in grooved posts, raised by a screw when it runs.
    let open = if work { 6 } else { 1 };
    paint::post(im, 12, 14, 31);
    paint::post(im, 18, 14, 31);
    im.rect(14, 18, 4, 12 - open, 'O');
    im.rect(14, 18, 4, 1, 'o');
    im.rect(14, 30 - open, 4, 1, 'B');
    im.line(16, 12, 16, 18, 'U');
    im.rect(14, 11, 4, 2, 'v');
    // The leat: a timber trough taking the water out toward the works.
    im.rect(12, 32, 8, h - 33, 'S');
    paint::stream(im, 13, 32, 6, h - 33, if work { t } else { 0 }, 1, seed ^ 2);
    if !work {
        im.rect(13, 32, 6, h - 33, 'S');
        for k in 0..4 {
            im.put(14 + k, 36 + k % 2, 'u');
        }
    }
    im.rect(11, 32, 1, h - 33, 'o');
    im.rect(19, 32, 1, h - 33, 'B');
    let _ = w;
}

pub(super) fn treadwheel(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 8) as f32, 15.5, 7.0, seed);
    // The great drum on its frame: spokes and a tread of boards, a man inside walking.
    paint::post(im, 2, h - 30, h - 2);
    paint::post(im, 28, h - 30, h - 2);
    let a = if work { paint::turn(t) * 2.0 / 10.0 } else { 0.1 };
    paint::wheel(im, 16.0, (h - 16) as f32, 13.0, a, 10, false, ('o', 'B', 'O'));
    paint::ring(im, 16.0, (h - 16) as f32, 10.0, 'B');
    if work {
        hands::stamp(im, 17, h - 7, Trade::Labourer, Act::Bare(Pose::Walk), t, true);
    } else {
        hands::stamp(im, 17, h - 7, Trade::Labourer, Act::Bare(Pose::Stand), 0, true);
    }
    // The axle, out through the frame to the shaft that carries the work off.
    im.rect(14, h - 18, 4, 4, 'u');
    im.line(18, h - 16, 31, h - 16, 'U');
    let _ = w;
}

pub(super) fn horse_gin(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // The round: a trodden ring of earth before the gin-house.
    let (cx, cy) = (24.0f32, (h - 16) as f32);
    paint::earth(im, cx, cy, 23.0, 14.0, seed);
    im.ellipse(cx, cy, 18.0, 10.5, 'I');
    im.ellipse(cx, cy, 8.0, 4.0, 'B');
    // The gin-house at the back: a timber drum under a thatched cone, a door.
    paint::boards(im, 14, h - 52, 20, 14, seed);
    paint::trapezoid(im, 6, h - 52, (23.0, 25.0), (10.0, 38.0), &|x, y, side| {
        if side < 0 { 'o' } else if side > 0 { 'r' } else if (x + y) % 3 == 0 { 'O' } else { 'P' }
    });
    im.rect(23, 3, 3, 3, 'B');
    paint::door(im, 21, h - 39, 6, 11);
    // The sweep: a long beam out of the drum at head height, the horse yoked to its end.
    let a = if work { paint::turn(t) } else { 2.2 };
    let (hx, hy) = (cx + a.cos() * 14.0, cy + a.sin() * 7.0);
    let draw_horse = |im: &mut crate::render::overworld::ink::Img| {
        let (x, y) = (hx as i32, hy as i32 + 4);
        let stride = if work { (t % 2) as i32 } else { 0 };
        let flip = a.cos() < 0.0;
        let dx = if flip { -1 } else { 1 };
        // A pale grey horse, so it stands out of the trodden brown.
        im.rect(x - 6, y - 9, 12, 5, 'h');
        im.rect(x - 6, y - 9, 12, 1, 'i');
        im.rect(x - 6, y - 5, 12, 1, 'G');
        let hxx = if flip { x - 9 } else { x + 6 };
        im.rect(hxx, y - 12, 3, 5, 'h');
        im.rect(hxx + if flip { -1 } else { 2 }, y - 11, 2, 3, 'J');
        im.rect(hxx, y - 13, 2, 2, 'g');
        im.line(x - 7 * dx, y - 9, x - 8 * dx, y - 4, 'b');
        for (lx, ph) in [(x - 5, stride), (x - 3, 1 - stride), (x + 3, stride), (x + 5, 1 - stride)] {
            im.rect(lx, y - 4, 1, 5 - ph, 'G');
        }
    };
    let behind = a.sin() < 0.0;
    if behind {
        draw_horse(im);
    }
    let by = h - 40;
    im.line(24, by, hx as i32, hy as i32 - 6, 'O');
    im.line(24, by + 1, hx as i32, hy as i32 - 5, 'B');
    if !behind {
        draw_horse(im);
    }
    if work {
        hands::stamp(im, 4, h - 3, Trade::Ostler, Act::Idle, 0, false);
    }
    paint::pile(im, 42, h - 1, 9, 7, Load::Grain, seed);
    let _ = w;
}
pub(super) fn windmill(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 8) as f32, 15.5, 7.0, seed);
    // The tower: tapering, whitewashed plaster over stone; a boat-shaped cap.
    paint::trapezoid(im, 34, h - 4, (9.0, 23.0), (5.0, 27.0), &|x, y, side| {
        let n = hash(x / 2, y / 3, seed) % 6;
        match side {
            -1 => 'i',
            1 => 'g',
            _ if x < 14 => 'H',
            _ if x > 20 => 'h',
            _ if n == 0 => 'i',
            _ => 'H',
        }
    });
    for y in (40..h - 6).step_by(7) {
        im.line(8 - (y - 40) / 10, y, 24 + (y - 40) / 10, y, 'i');
    }
    paint::trapezoid(im, 26, 34, (10.0, 22.0), (9.0, 23.0), &|_, y, side| if side != 0 { 'b' } else if y == 26 { 'O' } else { 'r' });
    im.ellipse(16.0, 26.0, 8.0, 3.0, 'R');
    paint::door(im, 12, h - 4, 8, 12);
    paint::window(im, 14, 46, 4, 6, work);
    // The sails: four arms on the hub, lattice frames with cloth on them.
    let (hx, hy) = (16.0f32, 25.0f32);
    let a0 = if work { paint::turn(t) * 0.5 } else { 0.5 };
    for k in 0..4 {
        let a = a0 + k as f32 * std::f32::consts::FRAC_PI_2;
        let (cs, sn) = (a.cos(), a.sin());
        for s in 3..24 {
            im.put((hx + cs * s as f32) as i32, (hy + sn * s as f32) as i32, 'O');
        }
        for s in 7..24 {
            for off in 1..5 {
                let (px, py) = (hx + cs * s as f32 - sn * off as f32, hy + sn * s as f32 + cs * off as f32);
                im.put(px as i32, py as i32, if off == 4 || s == 23 { 'o' } else if (s + off) % 3 == 0 { 'T' } else { 'H' });
            }
        }
    }
    im.rect(14, 23, 5, 5, 'B');
    im.rect(15, 24, 3, 3, 'O');
    paint::pile(im, 28, h - 1, 7, 6, Load::Flour, seed);
    let _ = (w, material::ramp::STONE);
}

pub(super) fn drive_shaft(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 5) as f32, 7.5, 4.0, seed);
    // Two posts with bearings and a long round shaft across them; a cog riding it.
    for x in [1, 12] {
        paint::post(im, x, h - 12, h - 2);
        im.rect(x - 1, h - 12, 4, 2, 'U');
    }
    im.rect(0, h - 11, w, 3, 'O');
    im.rect(0, h - 11, w, 1, 'T');
    im.rect(0, h - 9, w, 1, 'B');
    // A bar of light that marks the turn: stripes sliding along it.
    for x in 0..w {
        if work && (x + t as i32) % 4 == 0 {
            im.put(x, h - 10, 'o');
        }
    }
    let a = if work { paint::turn(t) / 4.0 } else { 0.0 };
    paint::wheel(im, 7.5, (h - 10) as f32, 4.5, a, 8, false, ('U', 'u', 'V'));
}

pub(super) fn leyline(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    // A bed of dark flags, a channel inlaid across it, runes down the channel.
    for y in h - 12..h - 1 {
        for x in 0..w {
            let n = hash(x / 4, y / 3, seed) % 4;
            im.put(x, y, if y == h - 12 { 'J' } else if n == 0 { 'G' } else { 'j' });
        }
    }
    im.rect(0, h - 8, w, 3, 's');
    for x in 0..w {
        let pulse = if work { (x + 16 - (t as i32 * 2) % 16) % 16 } else { 99 };
        let ch = match pulse {
            0..=1 => 'w',
            2..=3 => '3',
            4..=6 => '2',
            _ => if x % 5 == 0 { 'z' } else { '1' },
        };
        im.put(x, h - 7, ch);
        im.put(x, h - 6, if pulse < 6 { '2' } else { '0' });
    }
}

pub(super) fn mana_engine(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 9) as f32, 15.5, 8.0, seed);
    // A stepped dais, a ring of coils about a socket, the crystal floating over it.
    paint::blockish(im, 2, h - 14, 28, 10, ramp::SLATE, seed);
    paint::blockish(im, 6, h - 22, 20, 9, ramp::SLATE, seed ^ 1);
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        let (x, y) = (16.0 + a.cos() * 9.0, (h - 18) as f32 + a.sin() * 3.5);
        im.rect(x as i32 - 1, y as i32 - 3, 3, 4, 'U');
        im.rect(x as i32 - 1, y as i32 - 3, 3, 1, 'V');
        im.put(x as i32, y as i32 - 2, if work { '3' } else { 'S' });
    }
    let bob = if work { (paint::turn(t).sin() * 2.0).round() as i32 } else { 0 };
    let (cx, cy) = (16, h - 34 + bob);
    // The crystal: a tall gem, lit from within.
    for y in 0..14 {
        let half = if y < 5 { 1 + y } else { (14 - y) / 2 + 1 };
        for x in -half..=half {
            let ch = if work {
                if x < 0 { '3' } else if x == 0 { 'w' } else { '2' }
            } else if x < 0 { 'U' } else if x == 0 { 'v' } else { 'u' };
            im.put(cx + x, cy + y - 7, ch);
        }
    }
    if work {
        for k in 0..4 {
            let a = paint::turn(t) + k as f32 * std::f32::consts::FRAC_PI_2;
            im.put((cx as f32 + a.cos() * 9.0) as i32, (cy as f32 + a.sin() * 4.0) as i32, if k % 2 == 0 { 'w' } else { '3' });
        }
        im.line(cx, cy + 8, cx, h - 24, '2');
    }
    let _ = w;
}

pub(super) fn conduit(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 8.0, (h - 4) as f32, 7.5, 3.5, seed);
    // A squat stone pylon with a brass band, a crystal in a cage at its head.
    paint::blockish(im, 3, h - 10, 10, 8, ramp::SLATE, seed);
    im.rect(4, h - 18, 8, 9, 'x');
    im.rect(4, h - 18, 1, 9, 'S');
    im.rect(11, h - 18, 1, 9, 's');
    im.rect(4, h - 14, 8, 1, 'O');
    im.rect(3, h - 19, 10, 1, 'h');
    let (cx, cy) = (8, h - 25 + if work { (paint::turn(t).sin()).round() as i32 } else { 0 });
    for y in -3..=3i32 {
        let half = 3 - y.abs();
        for x in -half..=half {
            im.put(cx + x, cy + y, if work { if x <= 0 { '3' } else { '2' } } else if x <= 0 { 'U' } else { 'u' });
        }
    }
    if work {
        im.put(cx, cy, 'w');
        for k in 0..3 {
            let a = paint::turn(t) * 2.0 + k as f32 * 2.1;
            im.put((cx as f32 + a.cos() * 5.0) as i32, (cy as f32 + a.sin() * 2.5) as i32, '3');
        }
    }
    let _ = w;
}

pub(super) fn enchanter(c: &mut Ctx) {
    let (t, work, seed) = (c.t, c.work, c.seed);
    let (w, h) = (c.w, c.h);
    let im = &mut c.im;
    paint::earth(im, 16.0, (h - 9) as f32, 15.5, 8.0, seed);
    // A round tower of slate-blue stone, a cone of violet roof, an iron-banded door.
    paint::trapezoid(im, 22, h - 4, (7.0, 25.0), (5.0, 27.0), &|x, y, side| {
        let n = hash(x / 3, y / 4, seed) % 5;
        match side {
            -1 => 'U',
            1 => 'S',
            _ if x < 12 => if n == 0 { 'v' } else { 'U' },
            _ if x > 20 => 'u',
            _ if n == 0 => 'U',
            _ => 'Q' ,
        }
    });
    paint::trapezoid(im, 6, 22, (16.0, 17.0), (3.0, 29.0), &|x, y, side| {
        if side < 0 { ']' } else if side > 0 { '(' } else if (x + y) % 4 == 0 { '[' } else { ')' }
    });
    im.rect(2, 21, 28, 2, '(');
    paint::door(im, 12, h - 4, 8, 12);
    // A window high up, and the ring of runes round the middle.
    paint::window(im, 14, 28, 4, 7, work);
    im.rect(15, 28, 2, 7, if work { '[' } else { 'k' });
    for k in 0..7 {
        let x = 8 + k * 3;
        let on = work && (k as u32 + t) % 4 < 2;
        im.rect(x, 40, 2, 3, if on { '3' } else { 'S' });
    }
    // The crystal over the spire, bobbing and throwing light.
    let bob = if work { (paint::turn(t).sin() * 1.5).round() as i32 } else { 0 };
    let (cx, cy) = (16, 3 + bob);
    for y in -4..=4i32 {
        let half = 4 - y.abs();
        for x in -half..=half {
            im.put(cx + x, cy + y, if work { if x < 0 { '3' } else if x == 0 { 'w' } else { '2' } } else if x < 0 { 'U' } else { 'u' });
        }
    }
    if work {
        for k in 0..4 {
            let a = paint::turn(t) + k as f32 * std::f32::consts::FRAC_PI_2;
            im.put((cx as f32 + a.cos() * 8.0) as i32, (cy as f32 + a.sin() * 3.0) as i32, if k % 2 == 0 { '3' } else { 'w' });
        }
    }
    let _ = w;
}
