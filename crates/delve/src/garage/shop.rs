//! The shop floor: where the walls, the lifts and the racks stand, and the
//! meshes of all of it that never move. x runs across the hall, y into it
//! (the camera looks along +y), z is up, one unit about a pace.
//!
//! - Six lifts in two rows of three, one per knight (`BAYS`).
//! - Racks round the back and side walls, a station to a slot, each holding
//!   every part of its slot on a shelf (`stations`); a bench at the front for
//!   the paint and the trim.
//! - The rest of a working shop: a forge, tool chests, drums, tyres, a
//!   gantry with its chain, rafters and hanging lamps.

use super::look::*;
use super::meshes;
use super::parts::{self, Slot};
use super::prim::*;
use super::raster::{Light, Lighting};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use std::sync::OnceLock;

/// Half the hall's width, its depth and the height of its roof.
pub const HX: f32 = 15.5;
pub const HY: f32 = 22.0;
pub const ROOF: f32 = 11.5;
/// How big a racer is on its lift, against the example's racers.
pub const RACER_SCALE: f32 = 0.55;
/// The height of the pad a racer stands on.
pub const PAD_TOP: f32 = 0.5;
/// The height of a rack's shelf, and of the panel over it.
pub const SHELF: f32 = 1.3;
pub const TOP: f32 = 4.7;
/// How far a knight can reach to a rack.
pub const REACH: f32 = 2.4;

/// The lifts, front row then back row, left to right.
pub const BAYS: [(f32, f32); 6] = [(-8.0, 8.6), (0.0, 8.6), (8.0, 8.6), (-8.0, 15.4), (0.0, 15.4), (8.0, 15.4)];
/// The order the lifts are given out: the middle first.
pub const BAY_ORDER: [usize; 6] = [1, 4, 0, 2, 3, 5];
/// Half the pad's size along x and y.
pub const PAD_HALF: (f32, f32) = (3.5, 2.6);

/// A rack, or a bench: a run of cells, one part to a cell.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub slot: Slot,
    /// Where the run starts, and which way it runs.
    pub origin: (f32, f32),
    pub along: (f32, f32),
    /// Which way a knight at it faces, into the room.
    pub out: (f32, f32),
    pub cell: f32,
    pub n: usize,
    /// Whether the parts point into the room (a side wall) or along the wall.
    pub nose_out: bool,
    /// A bench has a knight on either side.
    pub two_sided: bool,
}

impl Station {
    pub fn len(&self) -> f32 {
        self.cell * self.n as f32
    }

    /// The middle of cell `i` on the wall's line.
    pub fn cell_at(&self, i: usize) -> (f32, f32) {
        let t = (i as f32 + 0.5) * self.cell;
        (self.origin.0 + self.along.0 * t, self.origin.1 + self.along.1 * t)
    }

    /// Where a knight stands to take from cell `i`.
    pub fn stand_at(&self, i: usize) -> (f32, f32) {
        let (x, y) = self.cell_at(i);
        let d = if self.two_sided { 1.2 } else { self.depth() + 0.5 };
        (x + self.out.0 * d, y + self.out.1 * d)
    }

    /// How far the shelf comes out from the wall.
    pub fn depth(&self) -> f32 {
        if self.nose_out { 2.7 } else { 1.5 }
    }

    /// The cell a knight at (x, y) is at, if they are close enough.
    pub fn cell_for(&self, x: f32, y: f32) -> Option<usize> {
        let (dx, dy) = (x - self.origin.0, y - self.origin.1);
        let t = dx * self.along.0 + dy * self.along.1;
        let off = dx * self.out.0 + dy * self.out.1;
        let (lo, hi) = if self.two_sided { (-REACH * 0.8, REACH * 0.8) } else { (-0.2, self.depth() + 0.8) };
        if t < -0.2 || t > self.len() + 0.2 || off < lo || off > hi {
            return None;
        }
        Some(((t / self.cell).floor().max(0.0) as usize).min(self.n - 1))
    }
}

/// Every station. The back wall runs exhaust, engines, crests; the right wall
/// the running gear and the armour; the left the chariots; the bench at the
/// front the paint and the trim.
pub fn stations() -> &'static [Station] {
    static S: OnceLock<Vec<Station>> = OnceLock::new();
    S.get_or_init(|| {
        let n = parts::count;
        let back = |slot, x0: f32, cell: f32| Station { slot, origin: (x0, HY), along: (1.0, 0.0), out: (0.0, -1.0), cell, n: n(slot), nose_out: false, two_sided: false };
        let left = |slot, y0: f32, cell: f32| Station { slot, origin: (-HX, y0), along: (0.0, 1.0), out: (1.0, 0.0), cell, n: n(slot), nose_out: true, two_sided: false };
                let right = |slot, y0: f32, cell: f32| Station { slot, origin: (HX, y0), along: (0.0, 1.0), out: (-1.0, 0.0), cell, n: n(slot), nose_out: true, two_sided: false };
        let bench = |slot, x0: f32, cell: f32| Station { slot, origin: (x0, 2.4), along: (1.0, 0.0), out: (0.0, 1.0), cell, n: n(slot), nose_out: false, two_sided: true };
        vec![
            back(Slot::Exhaust, -15.3, 1.5),
            back(Slot::Engine, -7.35, 2.45),
            back(Slot::Crest, 7.8, 1.5),
            left(Slot::Chassis, 8.0, 2.9),
            right(Slot::Armour, 6.3, 1.3),
            right(Slot::Running, 13.0, 1.6),
            bench(Slot::Paint, -7.8, 1.25),
            bench(Slot::Trim, 0.4, 1.25),
        ]
    })
}

/// The station for a slot.
pub fn station(slot: Slot) -> &'static Station {
    stations().iter().find(|s| s.slot == slot).expect("every slot has a station")
}

/// A box a knight cannot walk through: (x0, y0, x1, y1).
pub fn obstacles() -> &'static [(f32, f32, f32, f32)] {
    static O: OnceLock<Vec<(f32, f32, f32, f32)>> = OnceLock::new();
    O.get_or_init(|| {
        let mut o = vec![];
        for (bx, by) in BAYS {
            o.push((bx - PAD_HALF.0, by - PAD_HALF.1, bx + PAD_HALF.0, by + PAD_HALF.1));
        }
        for s in stations() {
            if s.two_sided {
                let (x0, y0) = s.origin;
                o.push((x0, y0 - 0.55, x0 + s.len(), y0 + 0.55));
            } else {
                // the shelf, out to where the parts end
                let d = s.depth() + 0.2;
                let (a, b) = (s.origin, (s.origin.0 + s.along.0 * s.len(), s.origin.1 + s.along.1 * s.len()));
                let c = (a.0 + s.out.0 * d, a.1 + s.out.1 * d);
                let e = (b.0 + s.out.0 * d, b.1 + s.out.1 * d);
                o.push((a.0.min(e.0).min(b.0).min(c.0), a.1.min(e.1).min(b.1).min(c.1), a.0.max(e.0).max(b.0).max(c.0), a.1.max(e.1).max(b.1).max(c.1)));
            }
        }
        // the forge and the chests along the left wall's front, the drums at the right's back
        o.push((-HX, 0.6, -HX + 3.4, 5.2));
        o.push((-HX, 5.4, -HX + 1.8, 8.0));
        o.push((HX - 2.2, 0.6, HX, 7.0));
        o
    })
}

/// Where cell `i` of a station holds its part: the shelf under it.
pub fn item_spot(s: &Station, i: usize) -> V3 {
    let (x, y) = s.cell_at(i);
    let (v, z) = if s.two_sided { (0.0, 0.9) } else { (0.35 + (s.depth() - 0.35) / 2.0, SHELF) };
    v3(x + s.out.0 * v, y + s.out.1 * v, z)
}

/// The bay a lift stands in, as its pad's middle.
pub fn bay_centre(bay: usize) -> V3 {
    let (x, y) = BAYS[bay % 6];
    v3(x, y, PAD_TOP)
}

/// Where a racer on a lift sits: the pad's middle, turned a little toward the
/// camera so both engines show.
pub fn racer_pose(bay: usize) -> (V3, f32) {
    (bay_centre(bay), -0.3)
}

/// Where a knight stands when it arrives at a bay: in front of its pad.
pub fn bay_door(bay: usize) -> (f32, f32) {
    let (x, y) = BAYS[bay % 6];
    (x, y - PAD_HALF.1 - 1.6)
}

/// The one camera: static, high at the front of the hall, looking down it.
pub fn camera(w: usize, h: usize) -> super::raster::Camera {
    if w * 10 < h * 17 {
        // a squarer frame (a terminal's): closer in, on the lifts and the engine rack
        super::raster::Camera::look(v3(0.0, -5.5, 8.5), v3(0.0, 13.0, 0.6), 1.12, w, h)
    } else {
        super::raster::Camera::look(v3(0.0, -8.0, 9.5), v3(0.0, 11.0, -0.2), 1.42, w, h)
    }
}

/// The lamps that never change, built once.
pub fn lighting() -> &'static Lighting {
    static L: OnceLock<Lighting> = OnceLock::new();
    L.get_or_init(build_lighting)
}

fn build_lighting() -> Lighting {
    let warm = [1.0, 0.74, 0.46];
    let mut lamps = vec![];
    for (bx, by) in BAYS {
        lamps.push(Light { pos: v3(bx, by - 0.5, 7.0), colour: scale3(warm, 1.35), reach: 12.5, halo: 1.6 });
    }
    // lamps over the racks
    for s in stations() {
        if s.two_sided {
            continue;
        }
        let (x, y) = s.cell_at(s.n / 2);
        lamps.push(Light { pos: v3(x + s.out.0 * 1.6, y + s.out.1 * 1.6, 4.4), colour: scale3(warm, 1.1), reach: 8.5, halo: 0.0 });
    }
    // the front bench
    lamps.push(Light { pos: v3(-4.0, 2.4, 3.6), colour: scale3(warm, 0.9), reach: 8.0, halo: 0.9 });
    lamps.push(Light { pos: v3(4.0, 2.4, 3.6), colour: scale3(warm, 0.9), reach: 8.0, halo: 0.9 });
    // the forge
    lamps.push(Light { pos: v3(-HX + 2.4, 3.0, 1.8), colour: [1.3, 0.5, 0.14], reach: 11.0, halo: 0.0 });
    // the moon at the high window
    lamps.push(Light { pos: v3(0.0, HY - 2.0, 7.6), colour: [0.32, 0.44, 0.9], reach: 16.0, halo: 0.0 });
    // the sign
    lamps.push(Light { pos: v3(0.0, HY - 1.2, 8.4), colour: [0.9, 0.16, 0.3], reach: 7.0, halo: 0.0 });
    Lighting { ambient: [0.045, 0.05, 0.09], lamps }
}

fn scale3(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// The shop as it stands when empty, built once.
pub fn architecture() -> &'static Mesh {
    static M: OnceLock<Mesh> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = Mesh::new();
        shell(&mut m);
        for bay in 0..6 {
            lift(&mut m, bay);
        }
        for s in stations() {
            rack(&mut m, s);
        }
        fittings(&mut m);
        m
    })
}

fn shell(m: &mut Mesh) {
    // the floor, running out under the camera
    quad(m, v3(-HX, -16.0, 0.0), v3(HX, -16.0, 0.0), v3(HX, HY, 0.0), v3(-HX, HY, 0.0), FLOOR_PLATE);
    // the walls
    quad(m, v3(-HX, HY, 0.0), v3(HX, HY, 0.0), v3(HX, HY, ROOF), v3(-HX, HY, ROOF), BRICK);
    quad(m, v3(-HX, -16.0, 0.0), v3(-HX, HY, 0.0), v3(-HX, HY, ROOF), v3(-HX, -16.0, ROOF), BRICK);
    quad(m, v3(HX, -16.0, 0.0), v3(HX, HY, 0.0), v3(HX, HY, ROOF), v3(HX, -16.0, ROOF), BRICK);
    // the roof, which the camera sees only the edge of
    quad(m, v3(-HX, -16.0, ROOF), v3(HX, -16.0, ROOF), v3(HX, HY, ROOF), v3(-HX, HY, ROOF), TIMBER);
    // pilasters
    for i in 0..5 {
        let x = -14.0 + i as f32 * 7.0;
        cuboid(m, v3(x - 0.5, HY - 0.7, 0.0), v3(x + 0.5, HY, ROOF), BRICK);
        cuboid(m, v3(x - 0.6, HY - 0.8, 0.0), v3(x + 0.6, HY, 0.7), DARK_STEEL);
        cuboid(m, v3(x - 0.6, HY - 0.8, ROOF - 0.5), v3(x + 0.6, HY, ROOF), DARK_STEEL);
    }
    for sgn in [-1.0f32, 1.0] {
        for i in 0..3 {
            let y = 4.0 + i as f32 * 7.0;
            let x = sgn * HX;
            cuboid(m, v3(x.min(x - sgn * 0.7), y - 0.5, 0.0), v3(x.max(x - sgn * 0.7), y + 0.5, ROOF), BRICK);
        }
    }
    // rafters and the beams across them
    for i in 0..7 {
        let y = -4.0 + i as f32 * 5.0;
        cuboid(m, v3(-HX, y - 0.35, ROOF - 1.2), v3(HX, y + 0.35, ROOF - 0.4), TIMBER);
        for sgn in [-1.0f32, 1.0] {
            let x = sgn * HX;
            beam(m, v3(x, y, ROOF - 3.2), v3(x - sgn * 3.0, y, ROOF - 1.0), 0.18, TIMBER);
        }
    }
    cuboid(m, v3(-HX, 0.0, ROOF - 0.5), v3(HX, HY, ROOF), TIMBER);
    // the high window on the back wall, with its mullions and the moon behind
    let (w0, w1, z0, z1) = (-4.4, 4.4, 5.0, 8.4);
    quad(m, v3(w0, HY - 0.02, z0), v3(w1, HY - 0.02, z0), v3(w1, HY - 0.02, z1), v3(w0, HY - 0.02, z1), MOONGLASS);
    for i in 0..=6 {
        let x = w0 + (w1 - w0) * i as f32 / 6.0;
        cuboid(m, v3(x - 0.07, HY - 0.3, z0), v3(x + 0.07, HY, z1), IRONK);
    }
    for z in [z0, (z0 + z1) / 2.0, z1] {
        cuboid(m, v3(w0, HY - 0.3, z - 0.07), v3(w1, HY, z + 0.07), IRONK);
    }
    // the sign over the window
    cuboid(m, v3(-4.6, HY - 0.5, 8.7), v3(4.6, HY, 9.4), DARK_STEEL);
    for i in 0..11 {
        let x = -4.0 + i as f32 * 0.8;
        let h = if i % 2 == 0 { 0.46 } else { 0.3 };
        cuboid(m, v3(x - 0.14, HY - 0.6, 9.05 - h * 0.5), v3(x + 0.14, HY - 0.5, 9.05 + h * 0.5), NEON);
    }
    // banners
    for x in [-11.5f32, 11.5] {
        quad(m, v3(x - 0.9, HY - 0.75, 4.3), v3(x + 0.9, HY - 0.75, 4.3), v3(x + 0.9, HY - 0.75, 8.2), v3(x - 0.9, HY - 0.75, 8.2), mat::BANNER);
        spike(m, v3(x - 0.9, HY - 0.75, 4.3), v3(x, HY - 0.75, 3.5), 0.9, mat::BANNER);
        cuboid(m, v3(x - 1.1, HY - 0.85, 8.2), v3(x + 1.1, HY - 0.65, 8.4), DARK_STEEL);
        // an emblem: a gold cross-hilted blade
        cuboid(m, v3(x - 0.07, HY - 0.78, 5.0), v3(x + 0.07, HY - 0.72, 7.6), mat::CLOTH_GOLD);
        cuboid(m, v3(x - 0.5, HY - 0.78, 7.0), v3(x + 0.5, HY - 0.72, 7.14), mat::CLOTH_GOLD);
    }
}

/// Dark iron for the fittings.
const IRONK: u8 = mat::STONE_DARK;

/// One lift: a pad on two columns, a hazard border painted round it, a
/// control post with its number, and a lamp on a chain.
fn lift(m: &mut Mesh, bay: usize) {
    let (bx, by) = BAYS[bay];
    let (hx, hy) = PAD_HALF;
    // hazard border on the floor
    for (x0, y0, x1, y1) in [
        (bx - hx - 0.6, by - hy - 0.6, bx + hx + 0.6, by - hy),
        (bx - hx - 0.6, by + hy, bx + hx + 0.6, by + hy + 0.6),
        (bx - hx - 0.6, by - hy, bx - hx, by + hy),
        (bx + hx, by - hy, bx + hx + 0.6, by + hy),
    ] {
        quad(m, v3(x0, y0, 0.02), v3(x1, y0, 0.02), v3(x1, y1, 0.02), v3(x0, y1, 0.02), HAZARD);
    }
    // the pad and its skirt
    cuboid(m, v3(bx - hx, by - hy, PAD_TOP - 0.35), v3(bx + hx, by + hy, PAD_TOP), PAD);
    cuboid(m, v3(bx - hx - 0.12, by - hy - 0.12, PAD_TOP - 0.4), v3(bx + hx + 0.12, by + hy + 0.12, PAD_TOP - 0.3), DARK_STEEL);
    // the columns under it
    for sx in [-1.0f32, 1.0] {
        cuboid(m, v3(bx + sx * (hx - 0.9) - 0.45, by - 0.5, 0.0), v3(bx + sx * (hx - 0.9) + 0.45, by + 0.5, PAD_TOP - 0.4), DARK_STEEL);
        cuboid(m, v3(bx + sx * (hx - 0.9) - 0.6, by - 0.65, 0.0), v3(bx + sx * (hx - 0.9) + 0.6, by + 0.65, 0.14), IRONK);
    }
    // the two posts at the pad's back corners, each with an amber beacon
    for sx in [-1.0f32, 1.0] {
        let (px, py) = (bx + sx * (hx - 0.1), by + hy - 0.15);
        cuboid(m, v3(px - 0.22, py - 0.22, 0.0), v3(px + 0.22, py + 0.22, 2.2), DARK_STEEL);
        cuboid(m, v3(px - 0.3, py - 0.3, 2.2), v3(px + 0.3, py + 0.3, 2.4), IRONK);
        cuboid(m, v3(px - 0.16, py - 0.34, 1.2), v3(px + 0.16, py - 0.22, 1.9), mat::BRASS);
        cyl(m, v3(px, py, 2.4), v3(px, py, 2.75), [0.18, 0.15], 8, LAMP, true);
    }
    // wheel stops
    for sx in [-1.0f32, 1.0] {
        cuboid(m, v3(bx + sx * (hx - 0.3) - 0.12, by - hy + 0.3, PAD_TOP), v3(bx + sx * (hx - 0.3) + 0.12, by + hy - 0.3, PAD_TOP + 0.1), mat::BRASS);
    }
    // the control post at the front corner, a lit plate and the bay's number
    let (px, py) = (bx + hx + 0.6, by - hy - 0.2);
    cuboid(m, v3(px - 0.3, py - 0.3, 0.0), v3(px + 0.3, py + 0.3, 1.7), DARK_STEEL);
    cuboid(m, v3(px - 0.34, py - 0.34, 1.3), v3(px + 0.34, py - 0.28, 1.75), IRONK);
    digit(m, bay + 1, v3(px, py - 0.36, 1.52), 0.3);
    // the lamp: a shade on a chain from the rafters
    let (lx, ly, lz) = (bx, by - 0.5, 7.0);
    chain(m, v3(lx, ly, ROOF - 0.4), v3(lx, ly, lz + 0.9), 0.07, IRONK);
    cyl(m, v3(lx, ly, lz + 0.9), v3(lx, ly, lz + 0.35), [0.2, 0.95], 10, DARK_STEEL, false);
    cyl(m, v3(lx, ly, lz + 0.35), v3(lx, ly, lz + 0.3), [0.95, 0.95], 10, LAMP, true);
    cyl(m, v3(lx, ly, lz + 0.3), v3(lx, ly, lz - 0.1), [0.34, 0.34], 8, LAMP, true);
}

/// A numeral in lit bars, facing -y, centred on `at`, `s` tall.
pub fn digit(m: &mut Mesh, n: usize, at: V3, s: f32) {
    // segments: top, upper-left, upper-right, middle, lower-left, lower-right, bottom
    const SEG: [[bool; 7]; 10] = [
        [true, true, true, false, true, true, true],
        [false, false, true, false, false, true, false],
        [true, false, true, true, true, false, true],
        [true, false, true, true, false, true, true],
        [false, true, true, true, false, true, false],
        [true, true, false, true, false, true, true],
        [true, true, false, true, true, true, true],
        [true, false, true, false, false, true, false],
        [true, true, true, true, true, true, true],
        [true, true, true, true, false, true, true],
    ];
    let t = s * 0.14;
    let (w, h) = (s * 0.5, s * 0.5);
    let bar = |m: &mut Mesh, cx: f32, cz: f32, hw: f32, hh: f32| {
        cuboid(m, at + v3(cx - hw, -0.02, cz - hh), at + v3(cx + hw, 0.04, cz + hh), NEON);
    };
    for (i, on) in SEG[n % 10].iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => bar(m, 0.0, h, w, t),
            1 => bar(m, -w, h * 0.5, t, h * 0.5),
            2 => bar(m, w, h * 0.5, t, h * 0.5),
            3 => bar(m, 0.0, 0.0, w, t),
            4 => bar(m, -w, -h * 0.5, t, h * 0.5),
            5 => bar(m, w, -h * 0.5, t, h * 0.5),
            _ => bar(m, 0.0, -h, w, t),
        }
    }
}

/// A rack or a bench and the parts on it.
fn rack(m: &mut Mesh, s: &Station) {
    let yaw_wall = s.along.1.atan2(s.along.0);
    let n = s.n;
    let len = s.len();
    let d = s.depth();
    let shelf = if s.two_sided { 0.9 } else { SHELF };
    // the run's own frame: u along, v out
    let at = |u: f32, v: f32, z: f32| v3(s.origin.0 + s.along.0 * u + s.out.0 * v, s.origin.1 + s.along.1 * u + s.out.1 * v, z);
    let boxed = |m: &mut Mesh, u0: f32, u1: f32, v0: f32, v1: f32, z0: f32, z1: f32, k: u8| {
        let (a, b) = (at(u0, v0, z0), at(u1, v1, z1));
        cuboid(m, v3(a.x.min(b.x), a.y.min(b.y), z0), v3(a.x.max(b.x), a.y.max(b.y), z1), k);
    };
    if s.two_sided {
        // a bench: a stout top on trestles, a low rail round the edge
        boxed(m, -0.1, len + 0.1, -0.5, 0.5, shelf - 0.18, shelf, TIMBER);
        boxed(m, -0.1, len + 0.1, -0.55, -0.45, shelf - 0.5, shelf + 0.12, DARK_STEEL);
        boxed(m, -0.1, len + 0.1, 0.45, 0.55, shelf - 0.5, shelf + 0.12, DARK_STEEL);
        for i in 0..=3 {
            let u = len * i as f32 / 3.0;
            boxed(m, u - 0.12, u + 0.12, -0.45, 0.45, 0.0, shelf - 0.18, DARK_STEEL);
        }
        boxed(m, -0.1, 0.05, -0.2, 0.2, shelf, shelf + 0.9, DARK_STEEL);
        boxed(m, len - 0.05, len + 0.1, -0.2, 0.2, shelf, shelf + 0.9, DARK_STEEL);
    } else {
        // a cabinet of drawers below, the shelf on it, the back panel and the
        // header with its lit strip above
        boxed(m, -0.1, len + 0.1, 0.0, d - 0.1, 0.0, shelf - 0.16, DARK_STEEL);
        for r in 0..3 {
            let z = 0.15 + r as f32 * ((shelf - 0.5) / 3.0);
            boxed(m, -0.12, len + 0.12, d - 0.14, d - 0.08, z + (shelf - 0.5) / 3.0 - 0.06, z + (shelf - 0.5) / 3.0, IRONK);
        }
        for i in 0..n {
            let u = (i as f32 + 0.5) * s.cell;
            boxed(m, u - 0.3, u + 0.3, d - 0.2, d - 0.06, shelf * 0.35, shelf * 0.35 + 0.1, mat::BRASS);
            boxed(m, u - 0.3, u + 0.3, d - 0.2, d - 0.06, shelf * 0.7, shelf * 0.7 + 0.1, mat::BRASS);
        }
        boxed(m, -0.1, len + 0.1, 0.0, d, shelf - 0.16, shelf, TIMBER);
        boxed(m, -0.1, len + 0.1, d - 0.08, d + 0.04, shelf - 0.2, shelf + 0.1, DARK_STEEL);
        boxed(m, -0.1, len + 0.1, 0.0, 0.2, shelf, TOP, TIMBER);
        for i in 0..=n {
            let u = i as f32 * s.cell;
            boxed(m, u - 0.07, u + 0.07, 0.2, 0.34, shelf, TOP, DARK_STEEL);
        }
        boxed(m, -0.15, len + 0.15, 0.0, 0.5, TOP, TOP + 0.6, DARK_STEEL);
        boxed(m, 0.05, len - 0.05, 0.5, 0.56, TOP + 0.1, TOP + 0.5, LAMP);
        boxed(m, 0.05, len - 0.05, 0.5, 0.58, TOP + 0.25, TOP + 0.35, mat::BRASS);
    }
    // the parts
    let mut item = Mesh::new();
    for (i, p) in parts::options(s.slot).enumerate() {
        meshes::display(p, 0.0, &mut item);
        let yaw = if s.nose_out { s.out.1.atan2(s.out.0) } else if s.two_sided { 0.0 } else { yaw_wall };
        let first = 0;
        place(&mut item, first, 1.0, yaw, V3::ZERO);
        let (lo, hi) = bounds(&item).unwrap_or((V3::ZERO, V3::ZERO));
        // the extents along the run and out from it
        let ext = |a: V3, b: V3| {
            let (p0, p1) = (a.x * s.along.0 + a.y * s.along.1, b.x * s.along.0 + b.y * s.along.1);
            let (q0, q1) = (a.x * s.out.0 + a.y * s.out.1, b.x * s.out.0 + b.y * s.out.1);
            ((p0.min(p1), p0.max(p1)), (q0.min(q1), q0.max(q1)))
        };
        let ((a0, a1), (o0, o1)) = ext(lo, hi);
        let (width, thick, tall) = (a1 - a0, o1 - o0, hi.z - lo.z);
        let (max_u, max_v, max_z) = (s.cell * 0.9, if s.two_sided { 0.9 } else { d - 0.3 }, if s.two_sided { 1.1 } else { TOP - SHELF - 0.2 });
        let k = (max_u / width.max(0.01)).min(max_v / thick.max(0.01)).min(max_z / tall.max(0.01)).min(1.3);
        let u_mid = (i as f32 + 0.5) * s.cell;
        // where the scaled item's box must land
        let want_u = u_mid;
        let want_v = if s.two_sided { 0.0 } else { 0.35 + (d - 0.35) / 2.0 };
        let have_u = (a0 + a1) * 0.5 * k;
        let have_v = (o0 + o1) * 0.5 * k;
        let target = at(want_u, want_v, shelf);
        // the item's own origin is moved so its box centre falls on the target
        let shift = v3(
            target.x - (s.along.0 * have_u + s.out.0 * have_v),
            target.y - (s.along.1 * have_u + s.out.1 * have_v),
            shelf - lo.z * k,
        );
        let start = m.tris.len();
        m.tris.extend_from_slice(&item.tris);
        place(m, start, k, 0.0, shift);
        // a pedestal mark under each part
        let c = at(u_mid, want_v, shelf);
        let r = (s.cell * 0.45).min(1.5);
        cuboid(m, v3(c.x - r.min(if s.along.0 != 0.0 { r } else { 0.3 }), c.y - r.min(if s.along.1 != 0.0 { r } else { 0.3 }), shelf), v3(c.x + r.min(if s.along.0 != 0.0 { r } else { 0.3 }), c.y + r.min(if s.along.1 != 0.0 { r } else { 0.3 }), shelf + 0.03), mat::BRASS);
    }
}

/// The rest of the shop: a forge, a chest, drums, tyres, a gantry.
fn fittings(m: &mut Mesh) {
    // the forge, against the left wall at the front: a hearth, a hood and a flue
    let (fy0, fy1) = (1.0, 5.0);
    cuboid(m, v3(-HX, fy0, 0.0), v3(-HX + 3.0, fy1, 1.1), BRICK);
    cuboid(m, v3(-HX, fy0 + 0.6, 1.1), v3(-HX + 2.2, fy1 - 0.6, 3.4), BRICK);
    cuboid(m, v3(-HX, fy0 + 1.1, 3.4), v3(-HX + 1.2, fy1 - 1.1, ROOF - 0.5), BRICK);
    cuboid(m, v3(-HX + 2.18, fy0 + 1.1, 1.3), v3(-HX + 2.26, fy1 - 1.1, 2.3), mat::FIRE);
    for (y0, y1, z0, z1) in [(fy0 + 0.9, fy0 + 1.1, 1.2, 2.4), (fy1 - 1.1, fy1 - 0.9, 1.2, 2.4), (fy0 + 0.9, fy1 - 0.9, 2.3, 2.45)] {
        cuboid(m, v3(-HX + 2.1, y0, z0), v3(-HX + 2.3, y1, z1), IRONK);
    }
    spike(m, v3(-HX + 2.0, (fy0 + fy1) * 0.5, 1.2), v3(-HX + 2.0, (fy0 + fy1) * 0.5, 2.2), 0.5, mat::FLAME);
    // an anvil on a stump before it
    let (ax, ay) = (-HX + 4.9, 3.0);
    cyl(m, v3(ax, ay, 0.0), v3(ax, ay, 0.8), [0.5, 0.5], 8, TIMBER, true);
    cuboid(m, v3(ax - 0.3, ay - 0.6, 0.8), v3(ax + 0.3, ay + 0.6, 1.05), DARK_STEEL);
    cuboid(m, v3(ax - 0.25, ay - 0.35, 1.05), v3(ax + 0.25, ay + 0.35, 1.25), DARK_STEEL);
    // a tool chest: a red cabinet of drawers with brass pulls
    let (x0, x1, y) = (-HX, -HX + 1.6, 5.4);
    cuboid(m, v3(x0, y, 0.0), v3(x1, y + 2.6, 2.6), CHEST);
    cuboid(m, v3(x0, y - 0.05, 2.6), v3(x1, y + 2.65, 2.75), DARK_STEEL);
    for r in 0..4 {
        let z = 0.25 + r as f32 * 0.6;
        cuboid(m, v3(x1 - 0.04, y + 0.15, z + 0.52), v3(x1 + 0.04, y + 2.45, z + 0.56), IRONK);
        cuboid(m, v3(x1 - 0.11, y + 0.95, z + 0.22), v3(x1 + 0.11, y + 1.65, z + 0.3), mat::BRASS);
    }
    // oil drums and tyres at the front right
    for (x, y) in [(HX - 1.0, 1.5), (HX - 2.2, 2.2)] {
        cyl(m, v3(x, y, 0.0), v3(x, y, 1.6), [0.62, 0.62], 10, RUST, true);
        cyl(m, v3(x, y, 0.5), v3(x, y, 0.58), [0.65, 0.65], 10, DARK_STEEL, false);
        cyl(m, v3(x, y, 1.1), v3(x, y, 1.18), [0.65, 0.65], 10, DARK_STEEL, false);
    }
    for (x, y, h) in [(HX - 1.3, 4.6, 4usize), (HX - 1.3, 6.4, 3)] {
        for i in 0..h {
            let z = i as f32 * 0.5;
            cyl(m, v3(x, y, z), v3(x, y, z + 0.5), [0.95, 0.95], 12, RUBBER, true);
            cyl(m, v3(x, y, z + 0.5), v3(x, y, z + 0.52), [0.5, 0.5], 12, DARK_STEEL, true);
        }
    }
    // a crate
    let (x, y, s) = (-HX + 1.3, 8.2, 0.9f32);
    cuboid(m, v3(x - s * 0.5, y - s * 0.5, 0.0), v3(x + s * 0.5, y + s * 0.5, s), TIMBER);
    cuboid(m, v3(x - s * 0.52, y - s * 0.52, s * 0.45), v3(x + s * 0.52, y + s * 0.52, s * 0.55), DARK_STEEL);
    // the gantry: two rails down the hall, a hoist on a beam over the engine rack
    let rz = 8.0;
    for x in [-(HX - 1.4), HX - 1.4] {
        cuboid(m, v3(x - 0.22, 1.0, rz), v3(x + 0.22, HY - 1.0, rz + 0.5), DARK_STEEL);
        for y in [1.0f32, HY - 1.0] {
            cuboid(m, v3(x - 0.3, y - 0.25, rz - 1.4), v3(x + 0.3, y + 0.25, rz), IRONK);
        }
    }
    cuboid(m, v3(-0.25, 11.0, rz + 0.25), v3(0.25, HY - 0.6, rz + 0.75), DARK_STEEL);
    let (ty, tx) = (18.6, 0.0);
    cuboid(m, v3(tx - 0.5, ty - 0.5, rz - 0.55), v3(tx + 0.5, ty + 0.5, rz + 0.25), IRONK);
    cuboid(m, v3(tx - 0.3, ty - 0.3, rz - 1.2), v3(tx + 0.3, ty + 0.3, rz - 0.55), mat::CLOTH_GOLD);
    chain(m, v3(tx, ty, rz - 1.2), v3(tx, ty, 4.8), 0.09, IRONK);
    cuboid(m, v3(tx - 0.25, ty - 0.25, 4.3), v3(tx + 0.25, ty + 0.25, 4.8), IRONK);
    beam(m, v3(tx, ty, 4.3), v3(tx, ty, 3.7), 0.09, DARK_STEEL);
    beam(m, v3(tx, ty, 3.7), v3(tx + 0.4, ty, 3.45), 0.09, DARK_STEEL);
    // chains looped on the back wall
    for (x, z0) in [(-5.8f32, 7.6), (6.2, 7.4), (-11.0, 7.4)] {
        chain(m, v3(x, HY - 0.9, z0), v3(x + 0.5, HY - 0.9, z0 - 1.9), 0.07, IRONK);
        chain(m, v3(x + 0.5, HY - 0.9, z0 - 1.9), v3(x + 1.2, HY - 0.9, z0 - 1.3), 0.07, IRONK);
    }
}
