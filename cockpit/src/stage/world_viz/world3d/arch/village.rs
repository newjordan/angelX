//! Hamlet-scale buildings and civil works: cottages, chapel, market stall,
//! well, and the arched stone bridge.
//!
//! These are the pieces that give a vista foreground and mid-ground. Each one
//! is built around a single bold roof shape plus one vertical accent (chimney,
//! bell tower, canopy ridge) so a row of them reads as a rhythm of gables
//! rather than a smear of dots.

use super::super::math::{V3, v3};
use super::super::mesh::{Mesh, mat};
use super::prim::{self, F_SIDES, F_SIDES_TOP};
use super::rng::{Rng, sub_seed};

/// Cottage. **Footprint ~4.2 × 2.8 tiles** (3.44 × 2.76 under the roof, plus a
/// lean-to shed off one gable end — which end is seeded), **~3.6 tiles tall**
/// to the chimney cap. Centred on the origin, door on the −y face.
pub(crate) fn cottage(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x43_4F_54_54)); // "COTT"
    let mut m = Mesh::new();

    let hx = 1.50_f32;
    let hy = 1.10_f32;
    let z_eave = 1.85 + r.jitter(0.12);
    let z_ridge = z_eave + 1.25 + r.jitter(0.12);

    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        0.28,
        hx + 0.12,
        hy + 0.12,
        hx,
        hy,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-hx, -hy, 0.28),
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
        0.10,
        mat::ROOF,
    );

    // Chimney: off-centre, tall enough to break the ridge line.
    let ch_x = r.range(0.55, 1.05) * if r.chance(0.5) { -1.0 } else { 1.0 };
    prim::boxed(
        &mut m,
        v3(ch_x - 0.19, 0.30, 1.30),
        v3(ch_x + 0.19, 0.68, z_ridge + 0.50),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );

    // Door and windows.
    prim::rect_panel(
        &mut m,
        v3(-0.34, -hy - 0.02, 0.28),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        0.68,
        1.20,
        mat::DOOR,
    );
    for &(x, sy) in &[
        (-0.95_f32, -1.0_f32),
        (0.95, -1.0),
        (-0.60, 1.0),
        (0.60, 1.0),
    ] {
        prim::rect_panel(
            &mut m,
            v3(x - 0.22, sy * (hy + 0.02), 0.85),
            v3(1.0, 0.0, 0.0),
            V3::UP,
            0.44,
            0.52,
            mat::WINDOW,
        );
    }

    // Lean-to shed on one gable end.
    let sx = if r.chance(0.5) { -1.0_f32 } else { 1.0 };
    let x_in = sx * hx;
    let x_out = sx * (hx + 0.95);
    let shed_h = 1.20;
    prim::boxed(
        &mut m,
        v3(x_in.min(x_out), -0.85, 0.0),
        v3(x_in.max(x_out), 0.85, shed_h),
        mat::WOOD,
        F_SIDES,
    );
    prim::quad(
        &mut m,
        v3(x_out, -0.92, shed_h),
        v3(x_out, 0.92, shed_h),
        v3(x_in, 0.92, shed_h + 0.55),
        v3(x_in, -0.92, shed_h + 0.55),
        mat::ROOF,
    );
    prim::quad(
        &mut m,
        v3(x_out, -0.92, shed_h - 0.10),
        v3(x_out, 0.92, shed_h - 0.10),
        v3(x_out, 0.92, shed_h),
        v3(x_out, -0.92, shed_h),
        mat::ROOF,
    );

    m
}

/// Chapel. **Footprint ~7.4 × 3.6 tiles**, **~6.9 tiles tall** to the spire.
/// Centred on the origin (the nave sits off-centre so the bell tower balances
/// the mass), entrance on the −y face.
pub(crate) fn chapel(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x43_48_50_4C)); // "CHPL"
    let mut m = Mesh::new();

    let cx = 0.0_f32;
    let hx = 2.50_f32;
    let hy = 1.50_f32;
    let z_eave = 2.60 + r.jitter(0.10);
    let z_ridge = z_eave + 1.75 + r.jitter(0.12);

    prim::battered(
        &mut m,
        cx,
        0.0,
        0.0,
        0.40,
        hx + 0.18,
        hy + 0.18,
        hx,
        hy,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(cx - hx, -hy, 0.40),
        v3(cx + hx, hy, z_eave),
        mat::STONE,
        F_SIDES,
    );
    for &sx in &[-1.0_f32, 1.0] {
        prim::gable_end(&mut m, cx + sx * hx, 0.0, hy, z_eave, z_ridge, mat::STONE);
    }
    prim::gable_roof(
        &mut m,
        cx,
        0.0,
        hx + 0.25,
        hy + 0.30,
        z_eave,
        z_ridge,
        0.14,
        0.12,
        mat::ROOF,
    );
    for &sy in &[-1.0_f32, 1.0] {
        let y = sy * (hy + 0.02);
        let right = v3(sy, 0.0, 0.0);
        for &x in &[-1.45_f32, 0.10, 1.65] {
            prim::arched_panel(
                &mut m,
                v3(cx + x - sy * 0.30, y, 1.10),
                right,
                V3::UP,
                0.60,
                1.00,
                5,
                mat::WINDOW,
            );
        }
    }
    prim::poly_fan(
        &mut m,
        v3(cx + hx + 0.02, 0.0, z_eave + 0.70),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        0.52,
        8,
        0.0,
        mat::WINDOW,
    );

    // Bell tower with a pyramidal spire.
    let tx = cx - hx - 1.05;
    let z_belfry = 5.00 + r.jitter(0.15);
    prim::battered(
        &mut m,
        tx,
        0.0,
        0.0,
        z_belfry,
        0.78,
        0.78,
        0.66,
        0.66,
        mat::STONE,
    );
    prim::boxed(
        &mut m,
        v3(tx - 0.80, -0.80, z_belfry),
        v3(tx + 0.80, 0.80, z_belfry + 0.32),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    for &(dx, dy, rx, ry) in &[
        (0.0_f32, -0.73_f32, 1.0_f32, 0.0_f32),
        (0.0, 0.73, 1.0, 0.0),
        (-0.73, 0.0, 0.0, 1.0),
        (0.73, 0.0, 0.0, 1.0),
    ] {
        prim::rect_panel(
            &mut m,
            v3(tx + dx - rx * 0.22, dy - ry * 0.22, z_belfry - 1.30),
            v3(rx, ry, 0.0),
            V3::UP,
            0.44,
            0.90,
            mat::WINDOW,
        );
    }
    prim::cone(
        &mut m,
        tx,
        0.0,
        z_belfry + 0.32,
        z_belfry + 1.90,
        1.10,
        4,
        std::f32::consts::FRAC_PI_4,
        mat::ROOF,
    );

    prim::arched_panel(
        &mut m,
        v3(cx - 0.62, -hy - 0.02, 0.40),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        1.24,
        1.10,
        5,
        mat::STONE_DARK,
    );
    prim::arched_panel(
        &mut m,
        v3(cx - 0.44, -hy - 0.05, 0.40),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        0.88,
        0.95,
        5,
        mat::DOOR,
    );

    // Re-centre: the tower hangs off −x, so slide the whole thing back.
    let (lo, hi) = prim::bbox(&m).unwrap_or((V3::ZERO, V3::ZERO));
    m.translated(v3(-(lo.x + hi.x) * 0.5, 0.0, 0.0))
}

/// Market stall. **Footprint ~2.8 × 2.0 tiles**, **~2.3 tiles tall**.
/// Centred on the origin, counter facing −y.
pub(crate) fn market_stall(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x53_54_4C_4C)); // "STLL"
    let mut m = Mesh::new();

    prim::boxed(
        &mut m,
        v3(-1.10, -0.48, 0.0),
        v3(1.10, 0.42, 0.85),
        mat::WOOD,
        F_SIDES_TOP,
    );
    for &(px, py) in &[
        (-1.15_f32, -0.68_f32),
        (1.15, -0.68),
        (-1.15, 0.68),
        (1.15, 0.68),
    ] {
        prim::boxed(
            &mut m,
            v3(px - 0.10, py - 0.10, 0.0),
            v3(px + 0.10, py + 0.10, 1.78),
            mat::WOOD,
            F_SIDES,
        );
    }
    prim::gable_roof(
        &mut m,
        0.0,
        0.0,
        1.42,
        1.00,
        1.72,
        2.28,
        0.08,
        0.0,
        mat::BANNER,
    );
    let crates = 2 + r.below(2);
    for i in 0..crates {
        let x = -0.70 + i as f32 * 0.70;
        let s = 0.22 + r.range(0.0, 0.08);
        prim::boxed(
            &mut m,
            v3(x - s, -0.24, 0.85),
            v3(x + s, 0.24, 0.85 + s * 1.4),
            mat::WOOD,
            F_SIDES_TOP,
        );
    }

    m
}

/// Draw well. **Footprint ~1.9 × 1.5 tiles**, **~2.5 tiles tall**.
/// Centred on the origin.
pub(crate) fn well() -> Mesh {
    let mut m = Mesh::new();
    prim::prism(&mut m, 0.0, 0.0, 0.0, 0.75, 0.66, 0.62, 8, 0.0, mat::STONE);
    prim::annulus(&mut m, 0.0, 0.0, 0.75, 0.62, 0.78, 8, 0.0, mat::STONE_DARK);
    prim::disc(&mut m, 0.0, 0.0, 0.18, 0.58, 8, 0.0, mat::WATER);
    for &sx in &[-1.0_f32, 1.0] {
        prim::boxed(
            &mut m,
            v3(sx * 0.62 - 0.09, -0.09, 0.60),
            v3(sx * 0.62 + 0.09, 0.09, 2.05),
            mat::WOOD,
            F_SIDES,
        );
    }
    prim::gable_roof(
        &mut m,
        0.0,
        0.0,
        0.95,
        0.72,
        1.98,
        2.48,
        0.08,
        0.0,
        mat::ROOF,
    );
    m
}

/// Arched stone bridge spanning along **+x**, centred on the origin.
/// **Footprint (`length` + 1.4) × 2.6 tiles**, **~2.4 tiles tall** to the
/// parapet crown. The deck cambers up to the middle and a single faceted
/// segmental arch springs from z = 0 at ±0.40 × `length`, so the void under the
/// crown is a clean dark lozenge at dot scale.
pub(crate) fn bridge(length: f32) -> Mesh {
    let mut m = Mesh::new();

    let len = length.max(3.0);
    let half = len * 0.5;
    let hw = 1.30_f32;
    let n = 8usize;
    let span = len * 0.40;
    let rise = 1.10_f32;

    let deck = |x: f32| -> f32 {
        let t = (x / half).clamp(-1.0, 1.0);
        1.30 + 0.50 * (1.0 - t * t)
    };
    let soffit = |x: f32| -> f32 {
        if x.abs() >= span {
            0.0
        } else {
            let t = x / span;
            rise * (1.0 - t * t).max(0.0).sqrt()
        }
    };

    for i in 0..n {
        let x0 = -half + len * (i as f32) / n as f32;
        let x1 = -half + len * ((i + 1) as f32) / n as f32;
        let (d0, d1) = (deck(x0), deck(x1));
        let (s0, s1) = (soffit(x0), soffit(x1));

        // Roadway.
        prim::quad(
            &mut m,
            v3(x0, -hw, d0),
            v3(x1, -hw, d1),
            v3(x1, hw, d1),
            v3(x0, hw, d0),
            mat::PATH,
        );
        // Spandrel faces — these carry the arch silhouette.
        for &sy in &[-1.0_f32, 1.0] {
            prim::quad(
                &mut m,
                v3(x0, sy * hw, s0),
                v3(x1, sy * hw, s1),
                v3(x1, sy * hw, d1),
                v3(x0, sy * hw, d0),
                mat::STONE,
            );
        }
        // Arch barrel underside (skipped where the arch has landed).
        if s0 > 1e-3 || s1 > 1e-3 {
            prim::quad(
                &mut m,
                v3(x0, -hw, s0),
                v3(x1, -hw, s1),
                v3(x1, hw, s1),
                v3(x0, hw, s0),
                mat::STONE_DARK,
            );
        }
        // Parapets: outer face + coping only.
        for &sy in &[-1.0_f32, 1.0] {
            let y_out = sy * hw;
            let y_in = sy * (hw - 0.26);
            prim::quad(
                &mut m,
                v3(x0, y_out, d0),
                v3(x1, y_out, d1),
                v3(x1, y_out, d1 + 0.58),
                v3(x0, y_out, d0 + 0.58),
                mat::STONE,
            );
            prim::quad(
                &mut m,
                v3(x0, y_out, d0 + 0.58),
                v3(x1, y_out, d1 + 0.58),
                v3(x1, y_in, d1 + 0.58),
                v3(x0, y_in, d0 + 0.58),
                mat::STONE_DARK,
            );
        }
    }

    // Abutments at both ends.
    for &sx in &[-1.0_f32, 1.0] {
        let x_in = sx * half;
        let x_out = sx * (half + 0.70);
        prim::boxed(
            &mut m,
            v3(x_in.min(x_out), -hw, 0.0),
            v3(x_in.max(x_out), hw, deck(half)),
            mat::STONE_DARK,
            F_SIDES_TOP,
        );
    }

    m
}
