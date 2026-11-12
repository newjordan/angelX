//! The Great Shaft's head in the monolith language (`realm_mono`): the
//! machine on the crown of the Delve gate's heap, one for each way the shaft
//! lifts what the Delve gives up (`crate::rules::stockyard::Tier`).
//!
//! - **Hand carry**: two posts and a beam with a pulley, a rope, a bucket
//!   and a pair of baskets.
//! - **Windlass**: a taller frame with a drum on its axle, a crank and a
//!   bucket on the rope.
//! - **Horse gin**: a capstan with its sweep and a horse at the end of it,
//!   a ring of stones round the track, a headframe for the rope.
//! - **Water hoist**: a headframe and drum, a stepped wheel standing in its
//!   race with the flume falling on it.
//! - **Ley lift**: a ring of four pale stones with lit crystal heads and a
//!   column of light with a block riding up it.
//!
//! Each stands on the heap's top, `top` high, inside `HALF_W` by `HALF_D`
//! of its middle, so nothing stands past the heap.
use angel_stage::set::Kind;

use super::realm_mono::Mono;
use super::realm_place::Stood;
use crate::rules::stockyard::Tier;

/// How far the head reaches either side of the heap's crown, across.
pub const HALF_W: f32 = 14.0;
/// And front to back.
pub const HALF_D: f32 = 12.0;
/// The crown's middle, back from the footprint's middle.
const CROWN_Z: f32 = -2.0;

/// A post: a thin standing box.
fn post(m: &mut Mono, kind: Kind, x: f32, z: f32, y0: f32, y1: f32, t: f32) {
    m.block(kind, [x - t / 2.0, x + t / 2.0], [y0, y1], [z - t / 2.0, z + t / 2.0]);
}

/// A disc standing in the x-y plane at depth `z`: rows of boxes, so a round
/// wheel reads as the monolith's stepped one.
fn disc(m: &mut Mono, kind: Kind, cx: f32, cy: f32, z: f32, r: f32, thick: f32) {
    let rows = (r * 2.0 / 2.0).ceil() as i32;
    for i in 0..rows {
        let y = cy - r + i as f32 * 2.0;
        let dy = (y + 1.0 - cy).abs();
        let half = (r * r - dy * dy).max(0.0).sqrt();
        if half > 0.5 {
            m.block(kind, [cx - half, cx + half], [y, y + 2.0], [z - thick / 2.0, z + thick / 2.0]);
        }
    }
}

/// The head of `tier` on a heap whose crown is `top` high, with `s` for the
/// ley lift's light.
pub fn head(m: &mut Mono, top: f32, s: &mut Stood, tier: Tier) {
    let z = CROWN_Z;
    match tier {
        Tier::Hand => {
            // Two posts, a beam, a pulley, a rope, a bucket, baskets.
            for x in [-6.0, 6.0] {
                post(m, Kind::Wood, x, z, top, top + 22.0, 2.0);
            }
            m.block(Kind::Wood, [-8.0, 8.0], [top + 22.0, top + 24.0], [z - 1.5, z + 1.5]);
            m.block(Kind::Metal, [-1.5, 1.5], [top + 17.0, top + 22.0], [z - 1.0, z + 1.0]);
            post(m, Kind::Wood, 0.0, z, top + 7.0, top + 17.0, 0.7);
            m.block(Kind::Wood, [-2.5, 2.5], [top + 3.0, top + 7.0], [z - 2.5, z + 2.5]);
            for (x, zz) in [(-10.0, z + 6.0), (9.0, z + 7.0)] {
                m.block(Kind::Thatch, [x - 2.5, x + 2.5], [top, top + 4.0], [zz - 2.5, zz + 2.5]);
            }
        }
        Tier::Windlass => {
            for x in [-10.0, 10.0] {
                post(m, Kind::Wood, x, z, top, top + 28.0, 2.5);
            }
            m.block(Kind::Wood, [-12.0, 12.0], [top + 28.0, top + 30.5], [z - 2.0, z + 2.0]);
            // The drum, a crank on the right post's outside, a bucket.
            m.block(Kind::Trunk, [-8.0, 8.0], [top + 17.0, top + 22.0], [z - 2.5, z + 2.5]);
            m.block(Kind::Wood, [10.0, 13.0], [top + 19.0, top + 20.0], [z - 0.5, z + 0.5]);
            m.block(Kind::Wood, [12.5, 13.5], [top + 15.0, top + 20.0], [z - 0.5, z + 0.5]);
            post(m, Kind::Wood, 0.0, z, top + 7.0, top + 17.0, 0.7);
            m.block(Kind::Wood, [-2.5, 2.5], [top + 3.0, top + 7.0], [z - 2.5, z + 2.5]);
            m.block(Kind::Wood, [11.0, 15.0], [top, top + 4.0], [z + 5.0, z + 9.0]);
        }
        Tier::HorseGin => {
            // The capstan and its sweep, the horse, the ring of stones.
            let (cx, cz) = (4.0, z + 1.0);
            m.block(Kind::Trunk, [cx - 1.5, cx + 1.5], [top, top + 16.0], [cz - 1.5, cz + 1.5]);
            m.block(Kind::Wood, [cx - 2.5, cx + 2.5], [top + 16.0, top + 18.0], [cz - 2.5, cz + 2.5]);
            m.put("box", Kind::Wood, [cx, top + 9.0, cz], [20.0, 1.4, 1.4], 30.0);
            // The horse, at the end of the sweep.
            let (hx, hz) = (cx + 8.7, cz + 5.0);
            m.block(Kind::Trunk, [hx - 2.5, hx + 2.5], [top + 3.5, top + 7.5], [hz - 1.2, hz + 1.2]);
            m.block(Kind::Trunk, [hx + 2.0, hx + 3.5], [top + 6.0, top + 9.5], [hz - 0.9, hz + 0.9]);
            for (dx, dz) in [(-2.0, -0.8), (-2.0, 0.8), (2.0, -0.8), (2.0, 0.8)] {
                post(m, Kind::Wood, hx + dx, hz + dz, top, top + 3.5, 0.6);
            }
            // The ring: eight low stones.
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                let (sx, sz) = (cx + a.cos() * 11.0, cz + a.sin() * 9.0);
                if sx.abs() <= HALF_W && (sz - z).abs() <= HALF_D {
                    m.block(Kind::Stone, [sx - 1.0, sx + 1.0], [top, top + 1.5], [sz - 1.0, sz + 1.0]);
                }
            }
            // The headframe on the left and the rope across to it.
            for zz in [z - 3.0, z + 3.0] {
                post(m, Kind::Wood, -11.0, zz, top, top + 20.0, 1.6);
            }
            m.block(Kind::Wood, [-12.5, -9.5], [top + 20.0, top + 22.0], [z - 4.0, z + 4.0]);
            m.block(Kind::Metal, [-12.0, -10.0], [top + 17.0, top + 20.0], [z - 1.0, z + 1.0]);
            m.block(Kind::Wood, [-11.0, 4.0], [top + 15.5, top + 16.2], [z - 0.3, z + 0.3]);
            post(m, Kind::Wood, -11.0, z, top + 5.0, top + 17.0, 0.5);
            m.block(Kind::Wood, [-13.0, -9.0], [top + 2.0, top + 5.0], [z - 2.0, z + 2.0]);
        }
        Tier::WaterHoist => {
            // The headframe and drum on the left.
            for zz in [z - 3.0, z + 3.0] {
                post(m, Kind::Wood, -9.0, zz, top, top + 22.0, 1.8);
            }
            m.block(Kind::Wood, [-11.0, -7.0], [top + 22.0, top + 24.0], [z - 4.5, z + 4.5]);
            m.block(Kind::Trunk, [-11.0, -7.0], [top + 14.0, top + 18.0], [z - 3.0, z + 3.0]);
            post(m, Kind::Wood, -9.0, z, top + 5.0, top + 14.0, 0.6);
            m.block(Kind::Wood, [-11.0, -7.0], [top + 2.0, top + 5.0], [z - 2.0, z + 2.0]);
            // The wheel, upright, with its axle through to the drum.
            let (wx, wy) = (6.0, top + 9.0);
            disc(m, Kind::Wood, wx, wy, z, 8.0, 2.0);
            disc(m, Kind::Trunk, wx, wy, z, 3.0, 2.6);
            m.block(Kind::Metal, [-8.0, wx], [wy - 0.5, wy + 0.5], [z - 0.5, z + 0.5]);
            // The flume above it, the water falling and a pool below.
            m.block(Kind::Wood, [wx - 2.0, wx + 12.0 - 2.0], [top + 19.0, top + 20.0], [z - 1.8, z + 1.8]);
            m.block(Kind::Water, [wx - 1.5, wx + 9.0], [top + 20.0, top + 21.0], [z - 1.2, z + 1.2]);
            m.block(Kind::Water, [wx - 1.5, wx + 0.5], [top + 3.0, top + 19.0], [z - 1.2, z + 1.2]);
            m.block(Kind::Water, [wx - 3.0, wx + 3.0], [top, top + 1.2], [z - 3.0, z + 3.0]);
        }
        Tier::LeyLift => {
            // Four pale stones, lit crystal heads, a column of light with a
            // block riding in it.
            for (x, zz) in [(-9.0, z - 8.0), (9.0, z - 8.0), (-9.0, z + 8.0), (9.0, z + 8.0)] {
                post(m, Kind::Stone, x, zz, top, top + 14.0, 3.0);
                m.block(Kind::Window, [x - 1.0, x + 1.0], [top + 14.0, top + 17.5], [zz - 1.0, zz + 1.0]);
            }
            m.block(Kind::Stone, [-7.0, 7.0], [top, top + 1.2], [z - 7.0, z + 7.0]);
            m.block(Kind::Window, [-1.5, 1.5], [top + 1.2, top + 46.0], [z - 1.5, z + 1.5]);
            m.block(Kind::Stone, [-3.0, 3.0], [top + 22.0, top + 27.0], [z - 3.0, z + 3.0]);
            s.lamp(m, [0.0, top + 30.0, z], 80.0, 1.0, 3.0);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/world3d/realm_shaft__tests.rs"]
mod tests;
