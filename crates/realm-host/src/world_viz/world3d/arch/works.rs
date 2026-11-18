//! The realm's working buildings: the smithy's open forge and the astronomer's
//! star-tower.
//!
//! # Why these exist
//! A staged vantage has to be *about* something. The court has four walls, four
//! drums, a keep and a gate; the realm has eight civic offices. Photograph
//! eight offices against the same four walls and eight vantages come back the
//! same picture — which is exactly what the first staging pass produced for the
//! Smithy and the Observatory. The fix is not a better camera, it is a mass
//! nothing else can be mistaken for.
//!
//! # Dot-scale art direction
//! - The **forge** reads from its fire. A dark stone mouth with a hot EMBER
//!   throat, a spark-lit anvil on a pool of firelight, and a chimney breaking
//!   the roofline. The rest of the shed is kept deliberately plain — every
//!   extra highlight steals from the one thing that says *smithy*.
//! - The **observatory** reads from its dome. Ten flat facets, so the moon lays
//!   a hard light/dark split across it that no other roof in the realm has, a
//!   warm slit prised open down the face, and a sighting tube poking through
//!   it. A drum with a cone on top would just be another tower.

use std::f32::consts::{PI, TAU};

use super::super::math::{V3, v3};
use super::super::mesh::{Mesh, mat};
use super::prim::{self, F_PZ, F_SIDES_TOP};
use super::rng::{Rng, sub_seed};

pub fn forge(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x46_4F_52_47));
    let mut m = Mesh::new();

    let hx = 1.85_f32;
    let hy = 1.70_f32;
    let z_sill = 0.30_f32;
    let z_eave = 2.30 + r.jitter(0.08);
    let z_ridge = z_eave + 1.05 + r.jitter(0.06);
    let right = v3(1.0, 0.0, 0.0);

    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_sill,
        hx + 0.16,
        hy + 0.16,
        hx,
        hy,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-hx, -hy, z_sill),
        v3(hx, hy, z_eave),
        mat::STONE,
        prim::F_PY | prim::F_NX | prim::F_PX,
    );
    prim::rect_panel(
        &mut m,
        v3(-hx + 0.05, hy - 0.06, z_sill),
        right,
        V3::UP,
        hx * 2.0 - 0.10,
        z_eave - z_sill,
        mat::STONE_DARK,
    );
    for &sx in &[-1.0_f32, 1.0] {
        prim::rect_panel(
            &mut m,
            v3(sx * (hx - 0.06), -hy + 0.05, z_sill),
            v3(0.0, 1.0, 0.0),
            V3::UP,
            hy * 2.0 - 0.10,
            z_eave - z_sill,
            mat::STONE_DARK,
        );
    }
    for &sx in &[-1.0_f32, 1.0] {
        prim::gable_end(&mut m, sx * hx, 0.0, hy, z_eave, z_ridge, mat::STONE);
    }
    prim::gable_roof(
        &mut m,
        0.0,
        0.0,
        hx + 0.28,
        hy + 0.42,
        z_eave,
        z_ridge,
        0.22,
        0.0,
        mat::WOOD,
    );
    prim::boxed(
        &mut m,
        v3(-(hx + 0.28), -0.13, z_ridge - 0.06),
        v3(hx + 0.28, 0.13, z_ridge + 0.13),
        mat::ROOF,
        F_SIDES_TOP,
    );

    let hearth_x = -0.75_f32;
    prim::boxed(
        &mut m,
        v3(hearth_x - 0.70, 0.50, z_sill),
        v3(hearth_x + 0.70, hy, 1.10),
        mat::STONE,
        prim::F_NY | prim::F_NX | prim::F_PX | F_PZ,
    );
    prim::arched_panel(
        &mut m,
        v3(hearth_x - 0.55, 0.46, 1.10),
        right,
        V3::UP,
        1.10,
        0.55,
        5,
        mat::EMBER,
    );
    prim::battered(
        &mut m,
        hearth_x,
        1.10,
        1.95,
        2.60,
        0.80,
        0.62,
        0.46,
        0.42,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(hearth_x - 0.46, 0.68, 2.60),
        v3(hearth_x + 0.46, 1.52, z_ridge + 0.95),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    prim::quad(
        &mut m,
        v3(hearth_x - 0.82, -0.35, z_sill + 0.03),
        v3(hearth_x + 0.90, -0.35, z_sill + 0.03),
        v3(hearth_x + 0.90, 0.46, z_sill + 0.03),
        v3(hearth_x - 0.82, 0.46, z_sill + 0.03),
        mat::EMBER,
    );

    let (ax, ay) = (0.88_f32, -0.58_f32);
    prim::prism(
        &mut m,
        ax,
        ay,
        z_sill,
        0.78,
        0.27,
        0.24,
        6,
        r.range(0.0, PI),
        mat::WOOD,
    );
    prim::boxed(
        &mut m,
        v3(ax - 0.30, ay - 0.21, 0.78),
        v3(ax + 0.30, ay + 0.21, 1.04),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    prim::quad(
        &mut m,
        v3(ax - 0.26, ay - 0.17, 1.06),
        v3(ax + 0.26, ay - 0.17, 1.06),
        v3(ax + 0.26, ay + 0.17, 1.06),
        v3(ax - 0.26, ay + 0.17, 1.06),
        mat::EMBER,
    );
    for i in 0..3 {
        let s = 0.13 + r.range(0.0, 0.05);
        prim::rect_panel(
            &mut m,
            v3(
                ax + r.jitter(0.42) - s * 0.5,
                ay - 0.22,
                1.25 + i as f32 * 0.26 + r.jitter(0.08),
            ),
            right,
            V3::UP,
            s,
            s,
            mat::EMBER,
        );
    }

    prim::rect_panel(
        &mut m,
        v3(0.50, hy + 0.02, z_sill),
        right,
        V3::UP,
        1.15,
        1.50,
        mat::DOOR,
    );
    prim::rect_panel(
        &mut m,
        v3(0.90, hy + 0.025, 1.95),
        right,
        V3::UP,
        0.35,
        0.25,
        mat::WINDOW,
    );

    for &(inset, z0, z1) in &[
        (0.00_f32, 0.00_f32, 0.42_f32),
        (0.08, 0.42, 0.80),
        (0.22, 0.80, 1.10),
    ] {
        let jitter = r.jitter(0.05);
        prim::boxed(
            &mut m,
            v3(hx + 0.25 + inset, -0.95 + inset + jitter, z0),
            v3(hx + 1.55 - inset, 0.95 - inset + jitter, z1),
            mat::WOOD,
            F_SIDES_TOP,
        );
    }

    m
}

pub fn observatory(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x4F_42_53_56));
    let mut m = Mesh::new();

    let sides = 10usize;
    let rot = r.range(0.0, TAU / sides as f32);
    let z_podium = 0.45_f32;
    let z_drum = 4.72 + r.jitter(0.15);
    let z_gallery = z_drum + 0.52;
    let z_ring = z_gallery + 0.62;
    let z_shoulder = z_ring + 0.58;
    let z_tip = z_shoulder + 0.52;

    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_podium,
        2.60,
        2.60,
        2.30,
        2.30,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-2.30, -2.30, z_podium - 0.15),
        v3(2.30, 2.30, z_podium),
        mat::STONE,
        F_PZ,
    );

    prim::prism(
        &mut m,
        0.0,
        0.0,
        z_podium,
        z_drum,
        1.78,
        1.62,
        sides,
        rot,
        mat::STONE,
    );
    prim::annulus(
        &mut m,
        0.0,
        0.0,
        z_drum,
        1.62,
        2.06,
        sides,
        rot,
        mat::STONE_DARK,
    );
    prim::prism(
        &mut m,
        0.0,
        0.0,
        z_drum,
        z_gallery,
        2.06,
        1.98,
        sides,
        rot,
        mat::STONE,
    );

    for i in 0..3 {
        let a = rot + PI * 0.75 + (i as f32) * (TAU / 3.0) + r.jitter(0.10);
        let out = v3(a.cos(), a.sin(), 0.0);
        let across = v3(-a.sin(), a.cos(), 0.0);
        let w = 0.30;
        prim::rect_panel(
            &mut m,
            out * 1.68 - across * (w * 0.5) + v3(0.0, 0.0, 1.30 + i as f32 * 1.05),
            across,
            V3::UP,
            w,
            0.64,
            mat::WINDOW,
        );
    }
    prim::arched_panel(
        &mut m,
        v3(-0.44, -1.74, z_podium),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        0.88,
        0.95,
        5,
        mat::DOOR,
    );

    let (r_low, r_mid, r_high) = (1.80_f32, 1.62_f32, 1.14_f32);
    prim::prism(
        &mut m,
        0.0,
        0.0,
        z_gallery,
        z_ring,
        r_low,
        r_mid,
        sides,
        rot,
        mat::ROOF,
    );
    prim::prism(
        &mut m,
        0.0,
        0.0,
        z_ring,
        z_shoulder,
        r_mid,
        r_high,
        sides,
        rot,
        mat::ROOF,
    );
    prim::cone(
        &mut m,
        0.0,
        0.0,
        z_shoulder,
        z_tip,
        r_high,
        sides,
        rot,
        mat::ROOF,
    );

    let slit = 0.21_f32;
    prim::quad(
        &mut m,
        v3(-slit, -(r_low + 0.03), z_gallery),
        v3(slit, -(r_low + 0.03), z_gallery),
        v3(slit, -(r_mid + 0.03), z_ring),
        v3(-slit, -(r_mid + 0.03), z_ring),
        mat::WINDOW,
    );
    prim::quad(
        &mut m,
        v3(-slit, -(r_mid + 0.03), z_ring),
        v3(slit, -(r_mid + 0.03), z_ring),
        v3(slit * 0.72, -(r_high + 0.03), z_shoulder),
        v3(-slit * 0.72, -(r_high + 0.03), z_shoulder),
        mat::WINDOW,
    );

    let base = v3(0.0, -0.55, z_gallery + 0.15);
    let tip = v3(0.0, -2.42, z_tip + 0.55);
    let axis = (tip - base).normalize();
    let across = v3(1.0, 0.0, 0.0);
    let over = axis.cross(across).normalize();
    for k in 0..4 {
        let a0 = PI * 0.25 + TAU * (k as f32) / 4.0;
        let a1 = PI * 0.25 + TAU * ((k + 1) as f32) / 4.0;
        let o0 = across * a0.cos() + over * a0.sin();
        let o1 = across * a1.cos() + over * a1.sin();
        prim::quad(
            &mut m,
            base + o0 * 0.19,
            base + o1 * 0.19,
            tip + o1 * 0.12,
            tip + o0 * 0.12,
            mat::WOOD,
        );
    }

    m
}
