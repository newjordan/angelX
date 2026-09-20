//! Realm dressing: a small stable yard on the north-east meadow — the
//! lived-in fringe of the demesne, merged once at the end of
//! [`super::compose::court_scene`].
//!
//! Two compact outbuildings (a stable with a wide doorway and a warm window,
//! a plainer byre), a low yard wall with a gated gap flanked by bannered
//! waymarkers, and one haycock. Everything is built from the existing
//! primitive/material vocabulary — no new materials, no new primitives.
//!
//! # Placement, and why the north-east meadow
//! Every staged camera (`world3d::VANTAGES`) was checked before this ground
//! was chosen. The yard occupies **x 18.5..26.7, y 21.5..27.3**:
//! - The parent composition reserves the yard in its tree-scatter exclusion;
//!   the original scatter put a tree through the stable. Existing landmark
//!   footprints and the river remain outside the yard.
//! - The nearest vantage eye is the Chapel's at `(26, -1)`, **~23 tiles** from
//!   the nearest yard masonry — no mark is engulfed, and the eye-to-hero
//!   corridors remain separate from this north-east corner.
//! - From the Keep's mark it lies dead in frame but fully **occluded by the
//!   north curtain** (sightline crosses the wall at ~3.0 tiles high against a
//!   3.75 parapet; the tallest mass here is ~2.9).
//! - From the Rookery's mark the yard's nearest corner is ~46 tiles out at
//!   the extreme frame edge — deep background, where a distant farm roof
//!   reads as settlement, not clutter.
//!
//! All coordinates stay inside `|x|, |y| ≤ 28` (`COURT_EXTENT`).
//!
//! Parts sit on `court_ground_z(seed, …) − 0.12`, exactly as `compose::place`
//! sinks the rest of the scene. Pure function of `seed` (determinism law).
//!
//! **~117 triangles** (cap 160 for the whole returned mesh).

use std::f32::consts::FRAC_PI_2;

use super::super::math::{V3, v3};
use super::super::mesh::{Mesh, mat};
use super::compose::court_ground_z;
use super::prim::{self, F_SIDES, F_SIDES_TOP};
use super::rng::{Rng, sub_seed};

/// The stable yard, placed in world coordinates, ready to merge into the
/// court scene with the same `seed` the scene was built with.
pub(crate) fn dressing(seed: u64) -> Mesh {
    let mut m = Mesh::new();

    // Sit a part on the court's own ground, sunk slightly so no seam shows —
    // the same rule `court_scene`'s `place` closure follows.
    let place = |out: &mut Mesh, part: Mesh, x: f32, y: f32, yaw: f32| {
        let z = court_ground_z(seed, x, y) - 0.12;
        out.merge(part.rotated_z(yaw).translated(v3(x, y, z)));
    };

    // The two outbuildings form an L around a small yard. The stable faces
    // south (−y): its doorway and warm window look back toward the court and
    // take the south-west moon on the face, and the yard gate leads to them.
    place(&mut m, stable(sub_seed(seed, 70)), 24.0, 23.6, 0.0);
    place(&mut m, byre(sub_seed(seed, 71)), 19.8, 25.4, FRAC_PI_2);

    // Low yard wall: a south run with a gated gap, and a short east return.
    place(&mut m, wall_run(3.8), 20.5, 21.5, 0.0);
    place(&mut m, wall_run(2.8), 25.2, 21.5, 0.0);
    place(&mut m, wall_run(3.0), 26.6, 23.0, FRAC_PI_2);

    // Waymarkers flanking the gate — stone posts with BANNER pennants.
    place(&mut m, waymarker(sub_seed(seed, 72)), 22.4, 21.5, 0.0);
    place(&mut m, waymarker(sub_seed(seed, 73)), 23.8, 21.5, 0.0);

    // One haycock in the yard: the cheapest possible "this place is worked".
    place(&mut m, haycock(sub_seed(seed, 74)), 21.6, 22.7, 0.0);

    m
}

/// Stable. **Footprint ~4.3 × 3.2 tiles** (3.8 × 2.6 of wall, plus roof
/// overhang), **~2.9 tiles tall** to the ridge. Centred on the origin, wide
/// doorway and warm window on the −y face, a loft light in the +x gable.
fn stable(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x53_54_42_4C)); // "STBL"
    let mut m = Mesh::new();

    let hx = 1.90_f32;
    let hy = 1.30_f32;
    let z_sill = 0.26_f32;
    let z_eave = 1.70 + r.jitter(0.08);
    let z_ridge = z_eave + 1.15 + r.jitter(0.08);

    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_sill,
        hx + 0.14,
        hy + 0.14,
        hx,
        hy,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-hx, -hy, z_sill),
        v3(hx, hy, z_eave),
        mat::STONE,
        F_SIDES,
    );
    for &sx in &[-1.0_f32, 1.0] {
        prim::gable_end(&mut m, sx * hx, 0.0, hy, z_eave, z_ridge, mat::STONE);
    }
    prim::gable_roof(
        &mut m,
        0.0,
        0.0,
        hx + 0.22,
        hy + 0.28,
        z_eave,
        z_ridge,
        0.12,
        0.0,
        mat::ROOF,
    );

    // The wide doorway is what says *stable* and not *cottage*: nearly a tile
    // and a quarter across, with one warm window beside it.
    prim::rect_panel(
        &mut m,
        v3(-0.62, -hy - 0.02, z_sill),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        1.24,
        1.22,
        mat::DOOR,
    );
    prim::rect_panel(
        &mut m,
        v3(0.98, -hy - 0.02, 0.88),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        0.44,
        0.52,
        mat::WINDOW,
    );
    // Loft light in the +x gable.
    prim::rect_panel(
        &mut m,
        v3(hx + 0.02, -0.20, z_eave + 0.30),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        0.40,
        0.42,
        mat::WINDOW,
    );

    m
}

/// Byre. **Footprint ~3.6 × 2.6 tiles** (3.2 × 2.1 of wall, plus roof
/// overhang), **~2.4 tiles tall** to the ridge. Centred on the origin, door
/// on the −y face. Deliberately plainer than the stable — one accent per
/// skyline, and the stable already has it.
fn byre(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x42_59_52_45)); // "BYRE"
    let mut m = Mesh::new();

    let hx = 1.60_f32;
    let hy = 1.05_f32;
    let z_sill = 0.24_f32;
    let z_eave = 1.45 + r.jitter(0.08);
    let z_ridge = z_eave + 0.95 + r.jitter(0.08);

    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_sill,
        hx + 0.12,
        hy + 0.12,
        hx,
        hy,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-hx, -hy, z_sill),
        v3(hx, hy, z_eave),
        mat::STONE,
        F_SIDES,
    );
    for &sx in &[-1.0_f32, 1.0] {
        prim::gable_end(&mut m, sx * hx, 0.0, hy, z_eave, z_ridge, mat::STONE);
    }
    prim::gable_roof(
        &mut m,
        0.0,
        0.0,
        hx + 0.20,
        hy + 0.25,
        z_eave,
        z_ridge,
        0.10,
        0.0,
        mat::ROOF,
    );
    prim::rect_panel(
        &mut m,
        v3(-0.40, -hy - 0.02, z_sill),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        0.80,
        1.05,
        mat::DOOR,
    );

    m
}

/// A low dry-stone yard wall, `len` tiles long along +x, centred on the
/// origin. **0.55 tiles tall** — a boundary you can see over, so it divides
/// the meadow without ever becoming a facade.
fn wall_run(len: f32) -> Mesh {
    let mut m = Mesh::new();
    let half = len.max(0.5) * 0.5;
    prim::boxed(
        &mut m,
        v3(-half, -0.09, 0.0),
        v3(half, 0.09, 0.55),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    m
}

/// Waymarker: a tapered stone post with a small BANNER pennant, centred on
/// the origin. **~1.0 tile tall** including the pennant.
fn waymarker(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x57_41_59_4D)); // "WAYM"
    let mut m = Mesh::new();
    let lean = r.jitter(0.05);
    prim::battered(
        &mut m,
        lean,
        0.0,
        0.0,
        0.85,
        0.15,
        0.15,
        0.10,
        0.10,
        mat::STONE,
    );
    prim::rect_panel(
        &mut m,
        v3(lean, -0.02, 0.48),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        0.34,
        0.30,
        mat::BANNER,
    );
    m
}

/// Haycock: one seven-facet cone, centred on the origin, **~1.15 tiles
/// tall**. WOOD rather than FOLIAGE — warm and dry, not green.
fn haycock(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x48_41_59_43)); // "HAYC"
    let mut m = Mesh::new();
    prim::cone(
        &mut m,
        0.0,
        0.0,
        0.0,
        1.15,
        0.78,
        7,
        r.range(0.0, std::f32::consts::TAU),
        mat::WOOD,
    );
    m
}

#[cfg(test)]
#[path = "../../../../../../tests/cockpit/world_viz/world3d__arch__realm_dressing__tests.rs"]
mod tests;
