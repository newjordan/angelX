//! The plans the first kit left undrawn, in the monolith language (the flat ones are
//! `render::overworld::works_kit::gaps`): the hods and ladders, the great crane, the vineyard, the
//! wine press, the aqueduct and the swape. Each is blocks only, in its own frame (front toward `+z`),
//! over the footprint the sim gives it (a tile each, the aqueduct seven by one).

use std::f32::consts::{PI, TAU};

use angel_stage::set::Kind;

use super::W;
use crate::render::overworld::works_kit::Load;
use crate::world3d::blockout::Shape;

/// A beam of `kind` from the point `a` to `b` (x, y, z), built of short boxes stepped along it:
/// what a slanted pole or brace is when every block is square to the ground.
fn strut(b: &mut W, kind: Kind, a: [f32; 3], c: [f32; 3], thick: f32) {
    let n = (((c[0] - a[0]).abs().max((c[1] - a[1]).abs()).max((c[2] - a[2]).abs()) / (thick * 0.9)).ceil() as i32).max(1);
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let p = [a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t, a[2] + (c[2] - a[2]) * t];
        let h = thick / 2.0;
        b.bx(kind, [p[0] - h, p[0] + h], [(p[1] - h).max(0.0), p[1] + h], [p[2] - h, p[2] + h]);
    }
}

/// Hods and ladders: a stage of boards on trestles with cut stone on it, a sheerleg with a pulley
/// at its head over it, a ladder leant against the stage, a hod riding the rope up and down.
pub(super) fn hod_gang(b: &mut W) {
    let work = b.work;
    // The stage and its four trestle legs.
    b.bx(Kind::Floor, [2.0, 11.0], [4.0, 5.2], [3.5, 11.5]);
    for (x, z) in [(2.6, 4.2), (10.4, 4.2), (2.6, 10.8), (10.4, 10.8)] {
        b.bx(Kind::Trunk, [x - 0.5, x + 0.5], [0.0, 4.0], [z - 0.5, z + 0.5]);
    }
    b.heap(Load::Ashlar, 6.0, 7.5, 8.0, 5.2, 5.0);
    // The sheerleg: two poles leaning in to a head beam over the stage, a pulley at its middle, a guy to the ground.
    for z in [4.6f32, 10.4] {
        strut(b, Kind::Wood, [13.6, 0.5, z + if z < 7.5 { -0.8 } else { 0.8 }], [12.6, 31.0, 7.4], 1.3);
    }
    b.bx(Kind::Wood, [11.0, 14.2], [30.0, 31.8], [5.0, 10.0]);
    b.wheel(Kind::Metal, 12.2, 29.2, 7.5, 1.7, if work { b.turn() } else { 0.4 }, 4, 1.0);
    strut(b, Kind::Cloth, [12.6, 30.0, 7.5], [15.2, 0.4, 14.5], 0.5);
    // The ladder leant on the stage from the west.
    for z in [4.0f32, 11.0] {
        strut(b, Kind::Wood, [0.6, 0.4, z], [2.4, 16.0, z], 0.9);
    }
    for k in 0..6 {
        let y = 2.0 + k as f32 * 2.4;
        let x = 0.6 + (y / 16.0) * 1.8;
        b.bx(Kind::Floor, [x - 0.4, x + 0.4], [y, y + 0.6], [4.0, 11.0]);
    }
    // The hod on the rope: down at the stage's foot when stopped, riding up and down at work.
    let lift = if work { (b.turn().cos() + 1.0) * 8.0 } else { 0.0 };
    let hy = 6.0 + lift;
    b.bx(Kind::Cloth, [12.0, 12.5], [hy + 3.0, 29.0], [7.2, 7.7]);
    b.bx(Kind::Wood, [9.8, 14.6], [hy, hy + 3.0], [5.6, 9.4]);
    b.bx(Kind::Road, [10.4, 14.0], [hy + 3.0, hy + 5.0], [6.2, 8.8]);
}

/// The great crane: a timber mast of four posts in three tapering stages, ringed and braced, a head
/// deck with a pennon, a jib out over the tile with its counterweight behind, the treadwheel in
/// front, the hook's block riding a rope with a stone in the sling.
pub(super) fn great_crane(b: &mut W) {
    let work = b.work;
    let (stages, top) = ([(0.0f32, 20.0f32, 4.0f32), (20.0, 40.0, 5.0), (40.0, 58.0, 5.9)], 58.0f32);
    // The sill and footing.
    b.bx(Kind::Stone, [2.0, 14.0], [0.0, 2.0], [2.5, 13.5]);
    for &(y0, y1, edge) in &stages {
        let (lo, hi) = (edge, 16.0 - edge);
        for (x, z) in [(lo, lo), (hi, lo), (hi, hi), (lo, hi)] {
            b.bx(Kind::Wood, [x - 0.8, x + 0.8], [y0, y1], [z - 0.8, z + 0.8]);
        }
        // A ring of beams at the stage's head and an X on the front face.
        b.bx(Kind::Floor, [lo - 0.8, hi + 0.8], [y1 - 1.0, y1], [lo - 0.8, lo + 0.4]);
        b.bx(Kind::Floor, [lo - 0.8, hi + 0.8], [y1 - 1.0, y1], [hi - 0.4, hi + 0.8]);
        b.bx(Kind::Floor, [lo - 0.8, lo + 0.4], [y1 - 1.0, y1], [lo - 0.8, hi + 0.8]);
        b.bx(Kind::Floor, [hi - 0.4, hi + 0.8], [y1 - 1.0, y1], [lo - 0.8, hi + 0.8]);
        strut(b, Kind::Trunk, [lo, y0 + 1.0, hi], [hi, y1 - 1.0, hi], 0.7);
        strut(b, Kind::Trunk, [hi, y0 + 1.0, hi], [lo, y1 - 1.0, hi], 0.7);
    }
    // The head: a deck, the post the jib turns on, a pennon pole with its cloth.
    b.bx(Kind::Wood, [4.0, 12.0], [top, top + 1.4], [4.0, 12.0]);
    b.bx(Kind::Trunk, [7.4, 8.6], [top + 1.4, top + 12.0], [7.4, 8.6]);
    b.bx(Kind::Cloth, [8.6, 12.6], [top + 8.6, top + 11.4], [7.8, 8.2]);
    // The jib out to the west, its tie to the post's head, the counterweight behind.
    b.bx(Kind::Wood, [-2.4, 11.0], [top + 1.4, top + 2.8], [7.2, 8.8]);
    strut(b, Kind::Cloth, [8.0, top + 12.0, 8.0], [-2.2, top + 2.6, 8.0], 0.45);
    strut(b, Kind::Cloth, [8.0, top + 12.0, 8.0], [14.2, top + 2.6, 8.0], 0.45);
    b.bx(Kind::Rock, [12.0, 15.6], [top + 1.4, top + 5.4], [6.4, 9.6]);
    // The treadwheel in front of the mast: a drum a man walks in, turning at work.
    b.wheel(Kind::Floor, 8.0, 5.2, 14.0, 4.6, if work { b.turn() / 4.0 } else { 0.3 }, 8, 2.4);
    // The hook's rope from the jib's tip, its block and a cut stone in the sling.
    let lift = if work { (b.turn().cos() + 1.0) * 11.0 } else { 30.0 };
    let hook = top - 4.0 - lift.min(46.0);
    b.bx(Kind::Cloth, [-1.9, -1.4], [hook + 3.0, top + 1.4], [7.75, 8.25]);
    b.bx(Kind::Metal, [-2.6, -0.6], [hook + 2.0, hook + 3.4], [7.0, 9.0]);
    b.bx(Kind::Road, [-2.8, -0.4], [hook, hook + 2.0], [6.8, 9.2]);
    b.heap(Load::Ashlar, 14.0, 13.0, 4.0, 0.0, 3.0);
}

/// The vineyard: three terraces of vine, each a wire on stakes with its leaves heaped along it and
/// dark bunches hanging under, a picking basket at the foot; a barrow of grapes trundling along the
/// front row at work.
pub(super) fn vineyard(b: &mut W) {
    let (w, work) = (b.w, b.work);
    for (k, z) in [3.6f32, 7.8, 12.0].into_iter().enumerate() {
        b.bx(Kind::Mud, [0.6, w - 0.6], [0.0, 0.7], [z - 1.9, z + 1.9]);
        for x in [1.6f32, 5.6, 9.6, 13.6] {
            b.bx(Kind::Trunk, [x - 0.4, x + 0.4], [0.7, 6.0], [z - 0.4, z + 0.4]);
        }
        b.bx(Kind::Wood, [1.2, w - 1.2], [4.8, 5.2], [z - 0.2, z + 0.2]);
        // The leaf: a hedge of boxes of uneven height along the wire, the bunches under it.
        for i in 0..7 {
            let x = 1.4 + i as f32 * 1.95;
            let up = ((i * 3 + k as i32 * 5) % 4) as f32 * 0.5;
            b.bx(Kind::Crop, [x, x + 1.7], [1.8, 5.4 + up], [z - 1.1, z + 1.1]);
            if i as usize % 2 == k % 2 {
                b.bx(Kind::Slate, [x + 0.3, x + 1.4], [0.9, 2.2], [z + 1.0, z + 1.8]);
            }
        }
    }
    b.bx(Kind::Floor, [11.0, 15.0], [0.0, 2.6], [13.2, 15.4]);
    b.bx(Kind::Slate, [11.6, 14.4], [2.6, 3.4], [13.8, 14.8]);
    if work {
        // A barrow of grapes along the front row.
        let x = 2.0 + b.turn() / TAU * 8.0;
        b.bx(Kind::Wood, [x - 1.6, x + 1.6], [1.4, 2.6], [14.0, 15.4]);
        b.bx(Kind::Slate, [x - 1.2, x + 1.2], [2.6, 3.4], [14.3, 15.1]);
        b.bx(Kind::Trunk, [x + 1.6, x + 3.4], [1.8, 2.2], [14.4, 15.0]);
    }
}

/// The wine press: a stone vat with the must in it, a timber frame over it with the screw down its
/// middle and the long bar that hauls it round, a tub for the wine, casks at its foot.
pub(super) fn wine_press(b: &mut W) {
    let work = b.work;
    // The vat: a stone ring with the must filling it, and the tub at its lip.
    b.cyl(Kind::Stone, 7.0, 7.2, 6.4, 0.0, 5.0);
    b.cyl(Kind::Brick, 7.0, 7.2, 5.4, 4.6, 0.8);
    b.bx(Kind::Wood, [11.6, 15.4], [0.0, 2.6], [10.0, 14.0]);
    b.bx(Kind::Brick, [12.0, 15.0], [2.4, 2.9], [10.4, 13.6]);
    // The frame: two uprights and a head beam, the screw down between them to the press board.
    for z in [3.0f32, 11.4] {
        b.bx(Kind::Wood, [5.8, 8.2], [5.0, 22.0], [z - 0.7, z + 0.7]);
    }
    b.bx(Kind::Wood, [5.0, 9.0], [22.0, 24.4], [2.0, 12.4]);
    let sink = if work { (b.turn().sin() + 1.0) * 1.6 } else { 1.5 };
    b.bx(Kind::Trunk, [6.6, 7.4], [9.0 - sink, 22.0], [6.8, 7.6]);
    b.bx(Kind::Floor, [4.2, 9.8], [7.6 - sink, 9.0 - sink], [4.2, 10.2]);
    // The bar through the screw's head, swinging as it is hauled.
    let a = if work { b.turn() * 0.5 } else { 0.7 };
    b.shape(Shape::Box, Kind::Wood, [7.0, 25.0, 7.2], [11.0, 1.2, 1.2], a.sin().atan2(a.cos()).to_degrees());
    // Casks at the foot, hooped.
    for (x, z) in [(1.8f32, 12.6f32), (4.6, 13.4)] {
        b.cyl(Kind::Floor, x, z, 1.6, 0.0, 3.4);
        b.cyl(Kind::Metal, x, z, 1.7, 1.0, 0.5);
    }
}

/// The aqueduct: a stone arcade, an arch in each tile between piers, a timber trough along the top
/// with water in it that runs at work, a lantern post at each end.
pub(super) fn aqueduct(b: &mut W) {
    let (w, work) = (b.w, b.work);
    let tiles = (w / 16.0).round() as i32;
    for k in 0..tiles {
        let cx = k as f32 * 16.0 + 8.0;
        b.arch(Kind::Stone, cx, 8.0, 16.0, 19.0, 7.0, 0.0);
        // The keystone and the course under the trough.
        b.bx(Kind::Bone, [cx - 1.0, cx + 1.0], [17.0, 18.6], [4.4, 11.6]);
    }
    b.bx(Kind::Stone, [0.0, w], [19.0, 21.0], [4.0, 12.0]);
    // The trough: two timber sides and the water between, ripples travelling at work.
    b.bx(Kind::Wood, [0.0, w], [21.0, 24.0], [4.4, 5.4]);
    b.bx(Kind::Wood, [0.0, w], [21.0, 24.0], [10.6, 11.6]);
    b.bx(Kind::Water, [0.0, w], [21.0, 23.0], [5.4, 10.6]);
    let shift = if work { (b.t % 8) as f32 * 2.0 } else { 0.0 };
    let mut x = (shift % 14.0) + 1.0;
    while x < w - 3.0 {
        b.bx(Kind::Plaster, [x, x + 2.4], [23.0, 23.3], [6.4, 7.0]);
        x += 14.0;
    }
    for x in [1.5f32, w - 1.5] {
        b.bx(Kind::Trunk, [x - 0.5, x + 0.5], [24.0, 31.0], [7.5, 8.5]);
        b.bx(if work { Kind::Window } else { Kind::Door }, [x - 1.0, x + 1.0], [31.0, 33.0], [7.0, 9.0]);
    }
}

/// The swape: a footing of boards, a post, the long pivot arm on it with its counterweight and its
/// hooked basket, standing facing south (the way the goods go); a built one is turned to its own way
/// (`works_kit::stand`). At work the arm swings from over what it lifts from (behind it) to over what
/// it lifts to (in front) and back, in the eight steps of the loop; a rest hangs it over the front.
pub(super) fn swape(b: &mut W) {
    let work = b.work;
    // The footing, a stout post with a cap, the arm hung on it: pale timber so it stands off the pad it is on.
    b.bx(Kind::Floor, [1.5, 14.5], [0.0, 1.2], [1.5, 14.5]);
    b.bx(Kind::Floor, [6.4, 9.6], [1.2, 13.0], [6.4, 9.6]);
    b.bx(Kind::Road, [5.8, 10.2], [13.0, 14.0], [5.8, 10.2]);
    // The arm: phi 0 is over the front (south, +z), pi over the back.
    let phi = if work {
        let k = (b.t % 8) as f32;
        let t = if k <= 4.0 { k / 4.0 } else { (8.0 - k) / 4.0 };
        PI * t
    } else {
        0.0
    };
    let (dx, dz) = (phi.sin(), phi.cos());
    let (px, pz) = (8.0, 8.0);
    let reach = 6.6;
    let tip = [px + dx * reach, 15.4, pz + dz * reach];
    let heel = [px - dx * 3.0, 15.4, pz - dz * 3.0];
    strut(b, Kind::Floor, [heel[0], 14.6, heel[2]], [tip[0], 14.6, tip[2]], 1.7);
    b.bx(Kind::Rock, [heel[0] - 1.9, heel[0] + 1.9], [11.6, 15.0], [heel[2] - 1.9, heel[2] + 1.9]);
    // The hook's rope and its basket, lower when the arm is over a place to take or leave a good.
    let down = if work && ((b.t % 8) == 0 || (b.t % 8) == 4) { 6.0 } else { 3.6 };
    b.bx(Kind::Cloth, [tip[0] - 0.3, tip[0] + 0.3], [14.6 - down, 14.6], [tip[2] - 0.3, tip[2] + 0.3]);
    b.bx(Kind::Road, [tip[0] - 2.0, tip[0] + 2.0], [14.6 - down - 2.4, 14.6 - down], [tip[2] - 2.0, tip[2] + 2.0]);
    if work && (1..=3).contains(&(b.t % 8)) {
        b.bx(Kind::Bone, [tip[0] - 1.3, tip[0] + 1.3], [14.6 - down, 14.6 - down + 1.8], [tip[2] - 1.3, tip[2] + 1.3]);
    }
}
