//! The racer's meshes: one world3d mesh for every part, and the racer they
//! make together. Nose at +x, the engine pair either side of y = 0, the
//! chariot towed behind at -x, z up, the racer's wheels on z = 0 and its box
//! centred on the origin. A racer is a handful of hundred triangles built
//! fresh each frame into buffers that are kept (`RacerMeshes`); nothing is
//! cached because nothing needs to be.
//!
//! This is the hook the race reads: [`racer`] gives a build's whole racer in
//! the world3d materials the race's own rasterizer already paints. The
//! shapes are the knight racers example's, copied and then grown into the
//! catalogue's other parts.

use super::parts::{ArmourKind, ChassisKind, ChassisShape, CrestShape, EngineKind, EngineShape, ExhaustShape, Part, RunKind, Shape, Slot};
use super::prim::*;
use super::racer::Build;
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use std::f32::consts::{PI, TAU};

/// Structural steel.
pub const STEEL: u8 = mat::ARMOR;
/// Dark iron under the paint.
pub const IRON: u8 = mat::STONE_DARK;
/// The knight's visor, and the cold arc between the engines.
pub const GLOW: u8 = mat::CRYSTAL;
/// Exhaust fire, cool and hot.
pub const FIRE: u8 = mat::EMBER;
pub const FIRE_HOT: u8 = mat::FLAME;

/// The two colours a build paints with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ink {
    pub paint: u8,
    pub trim: u8,
}

impl Ink {
    pub fn of(b: &Build) -> Ink {
        let paint = match b.part(Slot::Paint).shape {
            Shape::Paint(m) => m,
            _ => IRON,
        };
        let trim = match b.part(Slot::Trim).shape {
            Shape::Trim(m) => m,
            _ => GLOW,
        };
        Ink { paint, trim }
    }
}

/// A racer's parts, one mesh to a slot, and the rig (arc and tethers) that
/// belongs to no slot.
#[derive(Clone, Debug, Default)]
pub struct RacerMeshes {
    pub slot: [Mesh; 8],
    pub rig: Mesh,
}

impl RacerMeshes {
    pub fn new() -> RacerMeshes {
        RacerMeshes::default()
    }

    pub fn clear(&mut self) {
        self.slot.iter_mut().for_each(|m| m.tris.clear());
        self.rig.tris.clear();
    }

    pub fn tris(&self) -> usize {
        self.slot.iter().map(|m| m.tris.len()).sum::<usize>() + self.rig.tris.len()
    }
}

/// Where the racer's parts sit, given the engine and chassis it wears.
#[derive(Clone, Copy, Debug)]
pub struct Lay {
    pub e: EngineShape,
    pub c: ChassisShape,
    /// Each engine's distance from the middle.
    pub gy: f32,
    /// The engines' axis height.
    pub ez: f32,
    /// The chariot's middle, and the height of its floor.
    pub cx: f32,
    pub cz: f32,
}

impl Lay {
    pub fn of(b: &Build) -> Lay {
        let e = match b.part(Slot::Engine).shape {
            Shape::Engine(e) => e,
            _ => unreachable!("the engine slot holds engines"),
        };
        let c = match b.part(Slot::Chassis).shape {
            Shape::Chassis(c) => c,
            _ => unreachable!("the chassis slot holds chariots"),
        };
        let width = if e.kind == EngineKind::Twin { e.r * 2.1 } else { e.r };
        Lay { e, c, gy: e.gap + width, ez: (e.r + 0.3).max(1.2), cx: -e.len * 0.5 - 2.0 * c.size, cz: 0.34 * c.size }
    }
}

// ---------------------------------------------------------------- engines

/// One engine, nose at +x, its axis at (y, z).
fn engine(m: &mut Mesh, e: &EngineShape, ink: Ink, y: f32, z: f32, t: f32) {
    match e.kind {
        EngineKind::Barrel => barrel_engine(m, e, ink, y, z, t, e.r, 1.0),
        EngineKind::Twin => {
            for sgn in [-1.0f32, 1.0] {
                barrel_engine(m, e, ink, y + sgn * e.r * 1.1, z, t + sgn, e.r, 0.8);
            }
            for x in [-0.12, 0.28] {
                let x = e.len * x;
                cuboid(m, v3(x - 0.1, y - e.r * 1.1, z - e.r * 0.35), v3(x + 0.1, y + e.r * 1.1, z + e.r * 0.35), STEEL);
            }
            cuboid(m, v3(-e.len * 0.5, y - e.r * 0.2, z + e.r * 0.5), v3(e.len * 0.2, y + e.r * 0.2, z + e.r * 0.75), ink.trim);
        }
        EngineKind::Turbine => turbine(m, e, ink, y, z, t),
    }
}

/// The barrel of the example's engines: a body, a collar, a band, a nose, a
/// ram and a ring of spikes, with its flame out the back.
#[allow(clippy::too_many_arguments)]
fn barrel_engine(m: &mut Mesh, s: &EngineShape, ink: Ink, y: f32, z: f32, t: f32, r: f32, spikes: f32) {
    let l = s.len;
    let n = s.sides;
    barrel(m, [-l * 0.5, l * 0.25], [y, z], [r * 0.85, r], n, ink.paint, true);
    barrel(m, [l * 0.25, l * 0.4], [y, z], [r * 1.08; 2], n, STEEL, false);
    barrel(m, [-l * 0.18, -l * 0.12], [y, z], [r * 1.06; 2], n, ink.trim, false);
    barrel(m, [l * 0.4, l * 0.5], [y, z], [r, r * 0.6], n, IRON, false);
    spike(m, v3(l * 0.5, y, z), v3(l * 0.5 + r * 2.2, y, z), r * 0.55, STEEL);
    let count = (s.spikes as f32 * spikes).round() as usize;
    for i in 0..count {
        let a = i as f32 / count as f32 * TAU;
        let d = v3(0.0, a.cos(), a.sin());
        let base = v3(l * 0.33, y, z) + d * r;
        spike(m, base, base + d * r * 0.9 + v3(r * 0.5, 0.0, 0.0), r * 0.18, STEEL);
    }
    plume(m, l, y, z, r, n, t, 2.6);
}

/// The fire out of an engine's tail, flickering with `t`.
#[allow(clippy::too_many_arguments)]
fn plume(m: &mut Mesh, l: f32, y: f32, z: f32, r: f32, n: usize, t: f32, long: f32) {
    let fl = 1.0 + 0.25 * (t * 23.0 + y * 3.0).sin();
    barrel(m, [-l * 0.5 - 0.02, -l * 0.5 - 0.15], [y, z], [r * 0.7; 2], n, FIRE, true);
    spike(m, v3(-l * 0.5 - 0.15, y, z), v3(-l * 0.5 - r * long * fl, y, z), r * 0.6, FIRE_HOT);
}

/// A short fat drum with a halo of blades round its throat and an intake
/// cone for a nose.
fn turbine(m: &mut Mesh, s: &EngineShape, ink: Ink, y: f32, z: f32, t: f32) {
    let (l, r, n) = (s.len, s.r, s.sides);
    barrel(m, [-l * 0.5, l * 0.3], [y, z], [r * 0.9, r], n, ink.paint, true);
    barrel(m, [l * 0.05, l * 0.3], [y, z], [r * 1.1; 2], n, STEEL, false);
    barrel(m, [-l * 0.3, -l * 0.22], [y, z], [r * 1.05; 2], n, ink.trim, false);
    // the intake
    cone(m, v3(l * 0.3, y, z), v3(l * 0.5 + r * 1.1, y, z), r * 0.85, n, IRON);
    barrel(m, [l * 0.3, l * 0.36], [y, z], [r * 0.86; 2], n, ink.trim, false);
    // blades: a ring of fins swept back from the throat, and a halo to hold them
    let blades = 12;
    for i in 0..blades {
        let (a0, a1) = (i as f32 / blades as f32 * TAU, (i as f32 + 0.55) / blades as f32 * TAU);
        let at = |a: f32, rr: f32, x: f32| v3(x, y + rr * a.cos(), z + rr * a.sin());
        quad(m, at(a0, r * 1.05, l * 0.1), at(a1, r * 1.05, l * 0.1), at(a1 + 0.12, r * 1.8, l * 0.24), at(a0 + 0.12, r * 1.8, l * 0.24), STEEL);
    }
    for i in 0..blades {
        let (a0, a1) = (i as f32 / blades as f32 * TAU, (i + 1) as f32 / blades as f32 * TAU);
        let at = |a: f32, rr: f32, x: f32| v3(x, y + rr * a.cos(), z + rr * a.sin());
        quad(m, at(a0, r * 1.72, l * 0.23), at(a1, r * 1.72, l * 0.23), at(a1, r * 1.86, l * 0.25), at(a0, r * 1.86, l * 0.25), ink.trim);
    }
    plume(m, l, y, z, r, n, t, 3.4);
}

// ---------------------------------------------------------------- the rest of the racer

/// Exhaust stacks along an engine's spine (or out its flank).
fn exhaust(m: &mut Mesh, x: &ExhaustShape, e: &EngineShape, ink: Ink, y: f32, z: f32, sgn: f32) {
    let r = e.r * if e.kind == EngineKind::Twin { 2.1 } else { 1.0 };
    for i in 0..x.pipes {
        if x.side {
            let px = -e.len * 0.4 + i as f32 * (e.len * 0.75 / x.pipes as f32);
            let w = e.r * 0.2 + 0.05;
            let base = v3(px, y + sgn * r * 0.85, z - e.r * 0.15);
            let out = base + v3(-0.5, sgn * (0.5 + x.height * 0.4), 0.18);
            cyl(m, base, out, [w, w * 0.8], 6, STEEL, false);
            cyl(m, out, out + v3(-0.28, sgn * 0.12, 0.0), [w * 1.35, w * 1.1], 6, ink.trim, false);
            cyl(m, out + v3(-0.28, sgn * 0.12, 0.0), out + v3(-0.36, sgn * 0.14, 0.0), [w * 1.0, w * 0.5], 6, FIRE, true);
        } else {
            let h = x.height * (1.0 - 0.18 * (i as f32 - (x.pipes as f32 - 1.0) / 2.0).abs());
            let (px, pz, w) = (-e.len * 0.42 + i as f32 * (e.len * 0.5 / x.pipes as f32), z + r * 0.8, e.r * 0.16);
            cuboid(m, v3(px - w, y - w, pz), v3(px + w, y + w, pz + h), STEEL);
            cuboid(m, v3(px - w * 1.3, y - w * 1.3, pz + h * 0.82), v3(px + w * 1.3, y + w * 1.3, pz + h * 0.9), ink.trim);
            spike(m, v3(px, y, pz + h), v3(px, y, pz + h + w * 3.0), w * 1.4, IRON);
        }
    }
}

/// Horns, fins or a mast on an engine.
fn crest(m: &mut Mesh, c: &CrestShape, e: &EngineShape, ink: Ink, y: f32, z: f32) {
    let r = e.r;
    let l = e.len;
    for sgn in [-1.0f32, 1.0] {
        if c.fins {
            let b = v3(-l * 0.3, y + sgn * r * 0.7, z);
            quad(m, b, b + v3(l * 0.45, 0.0, 0.0), b + v3(l * 0.2, sgn * r * 1.6, r * 0.6), b + v3(-l * 0.1, sgn * r * 1.6, r * 0.6), STEEL);
        }
        if c.horns {
            let b = v3(l * 0.42, y + sgn * r * 0.6, z + r * 0.6);
            spike(m, b, b + v3(r * 1.4, sgn * r * 1.3, r * 1.6), r * 0.22, STEEL);
        }
    }
    if c.mast {
        let (px, h) = (-l * 0.36, 2.4 + r);
        let base = v3(px, y, z + r * 0.9);
        cyl(m, base, base + v3(0.0, 0.0, h), [0.05, 0.035], 5, STEEL, false);
        spike(m, base + v3(0.0, 0.0, h), base + v3(0.0, 0.0, h + 0.3), 0.07, ink.trim);
        let top = base + v3(0.0, 0.0, h - 0.1);
        quad(m, top, top + v3(0.0, 0.0, -0.6), top + v3(-1.1, 0.0, -0.5), top + v3(-1.1, 0.0, -0.15), ink.trim);
        barrel(m, [px - 0.12, px + 0.12], [y, z + r * 0.9], [0.14, 0.14], 6, ink.paint, true);
    }
}

/// The knight in the chariot, from the example.
fn knight(m: &mut Mesh, k: V3, s: f32, lance: bool, ink: Ink, sitting: bool) {
    let k = if sitting { k - v3(0.0, 0.0, 0.1 * s) } else { k };
    cuboid(m, k + v3(-0.15 * s, -0.2 * s, 0.0), k + v3(0.15 * s, 0.2 * s, 0.45 * s), STEEL);
    cuboid(m, k + v3(-0.18 * s, -0.3 * s, 0.32 * s), k + v3(0.12 * s, 0.3 * s, 0.45 * s), STEEL);
    cuboid(m, k + v3(-0.11 * s, -0.11 * s, 0.47 * s), k + v3(0.11 * s, 0.11 * s, 0.7 * s), STEEL);
    cuboid(m, k + v3(0.11 * s, -0.09 * s, 0.57 * s), k + v3(0.12 * s, 0.09 * s, 0.6 * s), GLOW);
    spike(m, k + v3(0.0, 0.0, 0.7 * s), k + v3(-0.25 * s, 0.0, 1.05 * s), 0.07 * s, ink.trim);
    cuboid(m, k + v3(0.0, -0.28 * s, 0.2 * s), k + v3(0.35 * s, -0.2 * s, 0.3 * s), IRON);
    cuboid(m, k + v3(0.0, 0.2 * s, 0.2 * s), k + v3(0.35 * s, 0.28 * s, 0.3 * s), IRON);
    if lance {
        let base = k + v3(-0.2 * s, -0.35 * s, 0.25 * s);
        barrel(m, [base.x, base.x + 1.4 * s], [base.y, base.z + 0.35 * s], [0.03 * s; 2], 4, STEEL, false);
        quad(m, base + v3(-0.1 * s, 0.0, 0.0), base + v3(-0.1 * s, 0.0, 0.9 * s), base + v3(-0.5 * s, 0.0, 0.8 * s), base + v3(-0.5 * s, 0.0, 0.5 * s), ink.trim);
        barrel(m, [base.x - 0.1 * s, base.x - 0.08 * s], [base.y, base.z + 0.45 * s], [0.02 * s; 2], 4, IRON, false);
        cuboid(m, base + v3(-0.11 * s, -0.01 * s, 0.0), base + v3(-0.09 * s, 0.01 * s, 0.95 * s), IRON);
    }
}

/// The chariot body and its knight: `x` is its middle, `z` its floor.
/// `with_knight` is false for the empty chariot on a rack.
fn chassis(m: &mut Mesh, c: &ChassisShape, ink: Ink, x: f32, z: f32, with_knight: bool) {
    let s = c.size;
    let at = |dx: f32, dy: f32, dz: f32| v3(x + dx * s, dy * s, z + dz * s);
    let rim_h = match c.kind {
        ChassisKind::Wedge => {
            cuboid(m, at(-0.5, -0.4, 0.0), at(0.3, 0.4, 0.45), ink.paint);
            let (a, b) = (at(0.3, -0.4, 0.0), at(0.3, 0.4, 0.0));
            let tip = at(0.85, 0.0, 0.1);
            let (at_, bt) = (a + v3(0.0, 0.0, 0.45 * s), b + v3(0.0, 0.0, 0.45 * s));
            tri(m, a, at_, tip, STEEL);
            tri(m, b, bt, tip, STEEL);
            tri(m, at_, bt, tip, STEEL);
            cuboid(m, at(-0.52, -0.42, 0.42), at(0.32, 0.42, 0.5), ink.trim);
            0.45
        }
        ChassisKind::Wagon => {
            // high iron sides with plank bands, a ram plate for a nose
            cuboid(m, at(-0.55, -0.42, 0.0), at(0.4, 0.42, 0.18), ink.paint);
            for sgn in [-1.0f32, 1.0] {
                cuboid(m, at(-0.55, sgn * 0.42 - 0.04, 0.0), at(0.4, sgn * 0.42 + 0.04, 0.7), ink.paint);
                for i in 0..4 {
                    let x0 = -0.5 + i as f32 * 0.24;
                    cuboid(m, at(x0, sgn * 0.42 - 0.06, 0.1), at(x0 + 0.04, sgn * 0.42 + 0.06, 0.66), STEEL);
                }
                cuboid(m, at(-0.57, sgn * 0.42 - 0.06, 0.66), at(0.42, sgn * 0.42 + 0.06, 0.74), ink.trim);
            }
            cuboid(m, at(-0.57, -0.44, 0.0), at(-0.51, 0.44, 0.7), ink.paint);
            cuboid(m, at(0.4, -0.44, 0.0), at(0.5, 0.44, 0.66), STEEL);
            spike(m, at(0.5, 0.0, 0.33), at(0.9, 0.0, 0.33), 0.2 * s, STEEL);
            0.2
        }
        ChassisKind::Skiff => {
            cuboid(m, at(-0.6, -0.24, 0.0), at(0.35, 0.24, 0.3), ink.paint);
            let (a, b) = (at(0.35, -0.24, 0.0), at(0.35, 0.24, 0.0));
            let tip = at(1.0, 0.0, 0.18);
            let (at_, bt) = (a + v3(0.0, 0.0, 0.3 * s), b + v3(0.0, 0.0, 0.3 * s));
            tri(m, a, at_, tip, ink.paint);
            tri(m, b, bt, tip, ink.paint);
            tri(m, at_, bt, tip, STEEL);
            tri(m, a, b, tip, IRON);
            cuboid(m, at(-0.62, -0.26, 0.27), at(0.38, 0.26, 0.33), ink.trim);
            for sgn in [-1.0f32, 1.0] {
                quad(m, at(-0.6, sgn * 0.24, 0.3), at(-0.9, sgn * 0.5, 0.52), at(-0.9, sgn * 0.5, 0.4), at(-0.5, sgn * 0.24, 0.1), STEEL);
            }
            0.3
        }
        ChassisKind::Throne => {
            cuboid(m, at(-0.5, -0.4, 0.0), at(0.3, 0.4, 0.4), ink.paint);
            let (a, b) = (at(0.3, -0.4, 0.0), at(0.3, 0.4, 0.0));
            let tip = at(0.7, 0.0, 0.1);
            let (at_, bt) = (a + v3(0.0, 0.0, 0.4 * s), b + v3(0.0, 0.0, 0.4 * s));
            tri(m, a, at_, tip, ink.paint);
            tri(m, b, bt, tip, ink.paint);
            tri(m, at_, bt, tip, STEEL);
            // the throne: a tall back with a spired crown, and a gilt rail round the tub
            cuboid(m, at(-0.56, -0.3, 0.3), at(-0.44, 0.3, 1.25), ink.paint);
            cuboid(m, at(-0.58, -0.32, 1.1), at(-0.42, 0.32, 1.2), ink.trim);
            for i in 0..5 {
                let dy = -0.28 + i as f32 * 0.14;
                spike(m, at(-0.5, dy, 1.2), at(-0.5, dy, 1.2 + if i % 2 == 0 { 0.34 } else { 0.2 }), 0.045 * s, ink.trim);
            }
            for sgn in [-1.0f32, 1.0] {
                cuboid(m, at(-0.5, sgn * 0.41 - 0.025, 0.4), at(0.3, sgn * 0.41 + 0.025, 0.5), ink.trim);
                for i in 0..4 {
                    let x0 = -0.45 + i as f32 * 0.24;
                    cuboid(m, at(x0, sgn * 0.41 - 0.02, 0.32), at(x0 + 0.03, sgn * 0.41 + 0.02, 0.5), ink.trim);
                }
            }
            0.4
        }
    };
    if with_knight {
        let k = v3(x - 0.1 * s, 0.0, z + rim_h * s);
        knight(m, k, s, c.lance, ink, c.kind == ChassisKind::Throne);
    }
}

/// The running gear under a chariot whose middle is `x`, on the floor.
fn running(m: &mut Mesh, kind: RunKind, ink: Ink, x: f32, s: f32, t: f32) {
    let z0 = 0.34 * s;
    for sgn in [-1.0f32, 1.0] {
        let y = sgn * 0.5 * s;
        match kind {
            RunKind::Hover => {
                let c = v3(x - 0.15 * s, y, z0 * 0.5);
                cyl(m, c - v3(0.0, 0.0, 0.07 * s), c + v3(0.0, 0.0, 0.07 * s), [0.34 * s, 0.34 * s], 8, STEEL, true);
                cyl(m, c - v3(0.0, 0.0, 0.1 * s), c - v3(0.0, 0.0, 0.065 * s), [0.3 * s, 0.3 * s], 8, ink.trim, true);
                // the cushion it rides on, a little alive
                let lift = 0.02 * (t * 14.0 + sgn).sin();
                disc(m, c - v3(0.0, 0.0, 0.11 * s - lift), 0.4 * s, 8, GLOW);
                cuboid(m, c + v3(-0.04 * s, -sgn * 0.12 * s, 0.0), c + v3(0.04 * s, -sgn * 0.0, z0 * 0.6), STEEL);
            }
            RunKind::Wheels => {
                for dx in [-0.34f32, 0.3] {
                    let c = v3(x + dx * s, y, 0.3 * s);
                    let w = 0.09 * s;
                    cyl(m, c - v3(0.0, w, 0.0), c + v3(0.0, w, 0.0), [0.3 * s, 0.3 * s], 10, IRON, true);
                    cyl(m, c - v3(0.0, w * 1.1, 0.0), c + v3(0.0, w * 1.1, 0.0), [0.16 * s, 0.16 * s], 6, ink.trim, true);
                    for i in 0..10 {
                        let a = i as f32 / 10.0 * TAU;
                        let d = v3(a.cos(), 0.0, a.sin());
                        spike(m, c + d * 0.29 * s, c + d * 0.4 * s, 0.035 * s, STEEL);
                    }
                    // spokes
                    for i in 0..4 {
                        let a = i as f32 / 4.0 * PI;
                        let d = v3(a.cos(), 0.0, a.sin()) * 0.28 * s;
                        beam(m, c - d + v3(0.0, sgn * w, 0.0), c + d + v3(0.0, sgn * w, 0.0), 0.012 * s, STEEL);
                    }
                }
                cuboid(m, v3(x - 0.45 * s, y - 0.03 * s, 0.28 * s), v3(x + 0.4 * s, y + 0.03 * s, 0.36 * s), STEEL);
            }
            RunKind::Treads => {
                let (xa, xb) = (x - 0.55 * s, x + 0.5 * s);
                let w = 0.11 * s;
                let h = 0.32 * s;
                for xe in [xa, xb] {
                    cyl(m, v3(xe, y - w, h * 0.5), v3(xe, y + w, h * 0.5), [h * 0.5, h * 0.5], 8, IRON, true);
                }
                cuboid(m, v3(xa, y - w, 0.0), v3(xb, y + w, 0.03 * s), IRON);
                cuboid(m, v3(xa, y - w, h - 0.03 * s), v3(xb, y + w, h), IRON);
                let n = 11;
                for i in 0..=n {
                    let px = xa + (xb - xa) * i as f32 / n as f32;
                    cuboid(m, v3(px - 0.02 * s, y - w * 1.08, 0.0), v3(px + 0.02 * s, y + w * 1.08, 0.05 * s), STEEL);
                    cuboid(m, v3(px - 0.02 * s, y - w * 1.08, h - 0.05 * s), v3(px + 0.02 * s, y + w * 1.08, h), STEEL);
                }
                cuboid(m, v3(xa + 0.1 * s, y - w * 0.4, h * 0.3), v3(xb - 0.1 * s, y + w * 0.4, h * 0.7), ink.trim);
            }
            RunKind::Runners => {
                let (xa, xb) = (x - 0.85 * s, x + 0.75 * s);
                let w = 0.045 * s;
                cuboid(m, v3(xa, y - w, 0.0), v3(xb, y + w, 0.05 * s), STEEL);
                quad(m, v3(xb, y - w, 0.0), v3(xb, y + w, 0.0), v3(xb + 0.3 * s, y + w, 0.2 * s), v3(xb + 0.3 * s, y - w, 0.2 * s), STEEL);
                for dx in [-0.4f32, 0.0, 0.4] {
                    cuboid(m, v3(x + dx * s - 0.02 * s, y - w * 0.8, 0.05 * s), v3(x + dx * s + 0.02 * s, y + w * 0.8, z0), IRON);
                }
                cuboid(m, v3(xa, y - w * 1.2, 0.05 * s), v3(xb - 0.1 * s, y + w * 1.2, 0.075 * s), ink.trim);
            }
        }
    }
}

/// Plating over the engines and the chariot.
#[allow(clippy::too_many_arguments)]
fn armour(m: &mut Mesh, kind: ArmourKind, e: &EngineShape, c: &ChassisShape, ink: Ink, lay: &Lay, y: f32, z: f32, sgn: f32) {
    let (l, r) = (e.len, e.r * if e.kind == EngineKind::Twin { 2.1 } else { 1.0 });
    match kind {
        ArmourKind::Bare => {}
        ArmourKind::Studs => {
            for k in 0..3 {
                let x = -l * 0.3 + k as f32 * l * 0.28;
                barrel(m, [x - 0.07, x + 0.07], [y, z], [r * 1.07; 2], e.sides.max(5), mat::TACK_LEATHER, false);
                for i in 0..e.sides.max(5) {
                    let a = (i as f32 + 0.5) / e.sides.max(5) as f32 * TAU;
                    let p = v3(x, y + r * 1.08 * a.cos(), z + r * 1.08 * a.sin());
                    spike(m, p, p + v3(0.0, a.cos(), a.sin()) * 0.11, 0.045, mat::BRASS);
                }
            }
        }
        ArmourKind::Scales => {
            for row in 0..4 {
                for k in 0..6 {
                    let x = -l * 0.42 + k as f32 * l * 0.15 + if row % 2 == 0 { 0.0 } else { l * 0.075 };
                    let a = -0.9 + row as f32 * 0.6;
                    for side in [-1.0f32, 1.0] {
                        let ang = if side < 0.0 { PI - a } else { a };
                        let _ = ang;
                    }
                    let a_out = if sgn < 0.0 { PI + 0.9 - row as f32 * 0.6 } else { 0.9 - row as f32 * 0.6 + PI };
                    let _ = a;
                    let dir = v3(0.0, a_out.cos(), a_out.sin());
                    // scales fan over the flank facing out and over the top
                    let p = v3(x, y, z) + dir * r * 1.04;
                    let tang = v3(0.0, -dir.z, dir.y);
                    quad(m, p - tang * 0.2, p + tang * 0.2, p + tang * 0.2 + dir * 0.06 + v3(-0.2, 0.0, 0.0), p - tang * 0.2 + dir * 0.06 + v3(-0.2, 0.0, 0.0), STEEL);
                }
            }
        }
        ArmourKind::Bastion | ArmourKind::Barding => {
            // slab plates on the outer flank and the top, bolted on
            for k in 0..3 {
                let x = -l * 0.38 + k as f32 * l * 0.3;
                cuboid(m, v3(x, y + sgn * r * 0.9 - 0.06, z - r * 0.55), v3(x + l * 0.26, y + sgn * r * 0.9 + 0.06, z + r * 0.55), STEEL);
                cuboid(m, v3(x, y - r * 0.5, z + r * 0.92), v3(x + l * 0.26, y + r * 0.5, z + r * 0.98), STEEL);
                for dz in [-0.4f32, 0.4] {
                    let p = v3(x + l * 0.13, y + sgn * (r * 0.9 + 0.06), z + dz * r);
                    cuboid(m, p - v3(0.05, 0.0, 0.05), p + v3(0.05, sgn * 0.05, 0.05), ink.trim);
                }
                if kind == ArmourKind::Barding {
                    for dz in [-0.45f32, 0.0, 0.45] {
                        let p = v3(x + l * 0.13, y + sgn * (r * 0.96 + 0.06), z + dz * r);
                        spike(m, p, p + v3(0.0, sgn * 0.5, 0.0), 0.07, IRON);
                    }
                }
            }
        }
    }
    // the chariot's side: the mirror of the engine's, once for each side
    let cs = c.size;
    let cx = lay.cx;
    let cz = lay.cz;
    match kind {
        ArmourKind::Bare => {}
        ArmourKind::Studs => {
            if sgn > 0.0 {
                for s2 in [-1.0f32, 1.0] {
                    for i in 0..4 {
                        let p = v3(cx - 0.4 * cs + i as f32 * 0.24 * cs, s2 * 0.42 * cs, cz + 0.2 * cs);
                        spike(m, p, p + v3(0.0, s2 * 0.1, 0.0), 0.045, mat::BRASS);
                    }
                }
            }
        }
        ArmourKind::Scales | ArmourKind::Bastion | ArmourKind::Barding => {
            if sgn > 0.0 {
                let h = if kind == ArmourKind::Scales { 0.3 } else { 0.42 };
                for s2 in [-1.0f32, 1.0] {
                    cuboid(m, v3(cx - 0.5 * cs, s2 * 0.43 * cs - 0.03, cz + 0.02 * cs), v3(cx + 0.3 * cs, s2 * 0.43 * cs + 0.03, cz + h * cs), STEEL);
                    if kind == ArmourKind::Barding {
                        for i in 0..3 {
                            let p = v3(cx - 0.3 * cs + i as f32 * 0.3 * cs, s2 * 0.46 * cs, cz + 0.2 * cs);
                            spike(m, p, p + v3(0.0, s2 * 0.5, 0.0), 0.06, IRON);
                        }
                    }
                }
                if kind != ArmourKind::Scales {
                    cuboid(m, v3(cx + 0.3 * cs, -0.3 * cs, cz + 0.05 * cs), v3(cx + 0.36 * cs, 0.3 * cs, cz + 0.4 * cs), STEEL);
                }
            }
        }
    }
}

/// The cold arc between the engines and the tethers that tow the chariot.
fn rig(m: &mut Mesh, lay: &Lay, ink: Ink, t: f32) {
    let (e, gy, z) = (&lay.e, lay.gy, lay.ez);
    let r = e.r;
    let x = -e.len * 0.05;
    arc(m, v3(x, -gy + r * 0.9, z + 0.1), v3(x, gy - r * 0.9, z + 0.1), t, ink.trim);
    let s = lay.c.size;
    let w = v3(0.0, 0.0, 0.035);
    for sgn in [-1.0f32, 1.0] {
        let (a, b) = (v3(-e.len * 0.45, sgn * gy, z), v3(lay.cx + 0.2 * s, sgn * 0.3 * s, lay.cz + 0.2 * s));
        quad(m, a - w, b - w, b + w, a + w, ink.trim);
    }
}

/// A jagged arc between two points, re-rolled each frame.
fn arc(m: &mut Mesh, a: V3, b: V3, t: f32, k: u8) {
    let mut prev = a;
    for i in 1..=5 {
        let mut p = a.lerp(b, i as f32 / 5.0);
        if i < 5 {
            let j = ((t * 31.0 + i as f32 * 1.7).sin() * 43758.5).fract() - 0.5;
            let q = ((t * 17.0 + i as f32 * 2.9).sin() * 24634.6).fract() - 0.5;
            p += v3(j * 0.25, 0.0, q * 0.3);
        }
        let w = v3(0.0, 0.0, 0.07);
        quad(m, prev - w, p - w, p + w, prev + w, k);
        prev = p;
    }
}

/// A build's racer, part by part, into `out` (cleared first), centred and
/// standing on z = 0.
pub fn build_parts(b: &Build, t: f32, out: &mut RacerMeshes) {
    out.clear();
    let lay = Lay::of(b);
    let ink = Ink::of(b);
    let (e, c) = (lay.e, lay.c);
    let ex = match b.part(Slot::Exhaust).shape {
        Shape::Exhaust(x) => x,
        _ => unreachable!("the exhaust slot holds exhaust"),
    };
    let cr = match b.part(Slot::Crest).shape {
        Shape::Crest(x) => x,
        _ => unreachable!("the crest slot holds crests"),
    };
    let ar = match b.part(Slot::Armour).shape {
        Shape::Armour(x) => x,
        _ => unreachable!("the armour slot holds armour"),
    };
    let rk = match b.part(Slot::Running).shape {
        Shape::Running(x) => x,
        _ => unreachable!("the running slot holds running gear"),
    };
    for sgn in [-1.0f32, 1.0] {
        let (y, z) = (sgn * lay.gy, lay.ez + if sgn > 0.0 { 0.03 } else { 0.0 });
        engine(&mut out.slot[Slot::Engine.index()], &e, ink, y, z, t);
        exhaust(&mut out.slot[Slot::Exhaust.index()], &ex, &e, ink, y, z, sgn);
        crest(&mut out.slot[Slot::Crest.index()], &cr, &e, ink, y, z);
        armour(&mut out.slot[Slot::Armour.index()], ar, &e, &c, ink, &lay, y, z, sgn);
    }
    chassis(&mut out.slot[Slot::Chassis.index()], &c, ink, lay.cx, lay.cz, true);
    running(&mut out.slot[Slot::Running.index()], rk, ink, lay.cx, c.size, t);
    rig(&mut out.rig, &lay, ink, t);
    // the paint and the trim are the colours of the others
    // centre the whole on the origin, standing on the floor
    let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
    for m in out.slot.iter().chain([&out.rig]) {
        if let Some((a, b)) = bounds(m) {
            lo = v3(lo.x.min(a.x), lo.y.min(a.y), lo.z.min(a.z));
            hi = v3(hi.x.max(b.x), hi.y.max(b.y), hi.z.max(b.z));
        }
    }
    let shift = v3(-(lo.x + hi.x) * 0.5, 0.0, -lo.z.min(0.0));
    for m in out.slot.iter_mut().chain([&mut out.rig]) {
        place(m, 0, 1.0, 0.0, shift);
    }
}

/// The hook the race reads: a build's whole racer, merged into `out`
/// (cleared first), at the scale of the knight racers example.
pub fn racer(b: &Build, t: f32, out: &mut Mesh) {
    out.tris.clear();
    let mut parts = RacerMeshes::new();
    build_parts(b, t, &mut parts);
    for m in parts.slot.iter().chain([&parts.rig]) {
        out.tris.extend_from_slice(&m.tris);
    }
}

/// How big a part's display model is: the box its mesh fits in, which the
/// shop scales to its rack.
pub fn display_size(p: &Part) -> V3 {
    let mut m = Mesh::new();
    display(p, 0.0, &mut m);
    bounds(&m).map_or(v3(1.0, 1.0, 1.0), |(lo, hi)| hi - lo)
}

/// A part on its own, as a rack shows it: centred on the origin in x and y,
/// standing on z = 0, in the colours of the default racer.
pub fn display(p: &Part, t: f32, out: &mut Mesh) {
    out.tris.clear();
    let base = Build::kit(0);
    // on a rack every part is shown in the same pale iron, so it can be read
    let ink = Ink { paint: mat::STONE, trim: GLOW };
    let kit_lay = Lay::of(&base);
    match p.shape {
        Shape::Engine(e) => {
            let z = (e.r + 0.3).max(1.2);
            engine(out, &e, ink, 0.0, z, t);
            // the cradle it sits in
            for x in [-0.3f32, 0.3] {
                let cx = e.len * x;
                cuboid(out, v3(cx - 0.18, -e.r * 1.6 - 0.1, 0.0), v3(cx + 0.18, e.r * 1.6 + 0.1, 0.2), IRON);
            }
        }
        Shape::Chassis(c) => {
            let lay = Lay { c, cx: 0.0, cz: 0.34 * c.size, ..kit_lay };
            chassis(out, &c, ink, 0.0, lay.cz, false);
            running(out, RunKind::Hover, ink, 0.0, c.size, t);
        }
        Shape::Running(k) => {
            let s = 1.8;
            running(out, k, ink, 0.0, s, t);
            // the axle that joins the pair
            cuboid(out, v3(-0.1, -0.5 * s, 0.34 * s), v3(0.1, 0.5 * s, 0.34 * s + 0.12), IRON);
        }
        Shape::Exhaust(x) => {
            let e = EngineShape { kind: EngineKind::Barrel, len: 2.4, r: 0.7, sides: 6, spikes: 0, gap: 1.0 };
            let z = 1.2;
            barrel(out, [-e.len * 0.5, e.len * 0.5], [0.0, z], [e.r * 0.85, e.r], 6, IRON, true);
            exhaust(out, &x, &e, ink, 0.0, z, 1.0);
        }
        Shape::Crest(c) => {
            let e = EngineShape { kind: EngineKind::Barrel, len: 2.4, r: 0.7, sides: 6, spikes: 0, gap: 1.0 };
            let z = 1.2;
            barrel(out, [-e.len * 0.5, e.len * 0.5], [0.0, z], [e.r * 0.85, e.r], 6, IRON, true);
            crest(out, &c, &e, ink, 0.0, z);
        }
        Shape::Armour(k) => {
            let e = EngineShape { kind: EngineKind::Barrel, len: 2.4, r: 0.7, sides: 6, spikes: 0, gap: 1.0 };
            let z = 1.2;
            barrel(out, [-e.len * 0.5, e.len * 0.5], [0.0, z], [e.r * 0.85, e.r], 6, IRON, true);
            let lay = Lay { e, gy: 0.0, ..kit_lay };
            armour(out, k, &e, &lay.c, ink, &lay, 0.0, z, 1.0);
        }
        Shape::Paint(k) => {
            // a tin of paint, its lid off
            cyl(out, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.8), [0.5, 0.5], 10, STEEL, true);
            cyl(out, v3(0.0, 0.0, 0.8), v3(0.0, 0.0, 0.84), [0.42, 0.42], 10, k, true);
            cyl(out, v3(0.0, 0.0, 0.22), v3(0.0, 0.0, 0.5), [0.52, 0.52], 10, k, false);
            beam(out, v3(-0.5, 0.0, 0.9), v3(0.5, 0.0, 0.9), 0.025, IRON);
        }
        Shape::Trim(k) => {
            // a spool of trim
            cyl(out, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.1), [0.5, 0.5], 10, IRON, true);
            cyl(out, v3(0.0, 0.0, 0.1), v3(0.0, 0.0, 0.7), [0.36, 0.36], 10, k, false);
            cyl(out, v3(0.0, 0.0, 0.7), v3(0.0, 0.0, 0.8), [0.5, 0.5], 10, IRON, true);
        }
    }
    // stand it on the shelf, whatever hangs below its middle
    if let Some((lo, _)) = bounds(out) {
        place(out, 0, 1.0, 0.0, v3(0.0, 0.0, -lo.z));
    }
}
