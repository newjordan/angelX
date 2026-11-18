//! Castle pieces: the great keep, round towers, curtain wall runs, gatehouse.
//!
//! Dot-scale art direction for this family: one heavy battered mass per
//! building, exactly one strong horizontal (the corbel table under the
//! parapet), crenellation teeth ~0.7 tiles wide, and cone-capped turrets so the
//! skyline has a spike to catch the eye. Nothing here is smaller than ~0.3
//! tiles because anything finer dissolves into the Bayer screen.

use std::f32::consts::PI;

use super::super::math::{V3, v3};
use super::super::mesh::{Mesh, mat};
use super::prim::{self, F_SIDES, F_SIDES_TOP};
use super::rng::{Rng, sub_seed};

pub fn keep(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x4B_45_45_50));
    let mut m = Mesh::new();

    let half = 3.0_f32;
    let batter_h = half + 0.35;
    let z_batter = 1.05 + r.jitter(0.08);
    let z_wall = 4.10 + r.jitter(0.12);
    let z_corbel = z_wall + 0.35;
    let deck_half = half + 0.20;

    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_batter,
        batter_h,
        batter_h,
        half,
        half,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-half, -half, z_batter),
        v3(half, half, z_wall),
        mat::STONE,
        F_SIDES,
    );
    prim::boxed(
        &mut m,
        v3(-deck_half, -deck_half, z_wall),
        v3(deck_half, deck_half, z_corbel),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );

    let mut face = Mesh::new();
    let wall_y = -half - 0.02;
    let right = v3(1.0, 0.0, 0.0);
    let up = V3::UP;
    let win_w = 0.42;
    let win_h = 0.95;
    for &x in &[-1.75_f32, 1.75] {
        prim::rect_panel(
            &mut face,
            v3(x - win_w * 0.5, wall_y, 1.70),
            right,
            up,
            win_w,
            win_h,
            mat::WINDOW,
        );
    }
    for &x in &[-1.75_f32, 0.0, 1.75] {
        prim::rect_panel(
            &mut face,
            v3(x - win_w * 0.5, wall_y, 2.95),
            right,
            up,
            win_w,
            win_h,
            mat::WINDOW,
        );
    }
    prim::merlons(
        &mut face,
        v3(-2.55, -deck_half, z_corbel),
        v3(2.55, -deck_half, z_corbel),
        v3(0.0, 1.0, 0.0),
        0.70,
        0.42,
        1.28,
        6,
        mat::STONE,
    );
    for k in 0..4 {
        m.merge(face.clone().rotated_z(k as f32 * PI * 0.5));
    }

    let t_off = 2.60;
    let t_r = 0.85;
    let t_base = 2.80;
    let t_top = z_corbel + 0.55;
    let mut turret = Mesh::new();
    prim::prism(
        &mut turret,
        0.0,
        0.0,
        t_base,
        t_top,
        t_r,
        t_r,
        8,
        PI / 8.0,
        mat::STONE,
    );
    prim::annulus(
        &mut turret,
        0.0,
        0.0,
        t_top,
        t_r,
        t_r * 1.35,
        8,
        PI / 8.0,
        mat::STONE_DARK,
    );
    prim::cone(
        &mut turret,
        0.0,
        0.0,
        t_top,
        t_top + 0.90,
        t_r * 1.35,
        8,
        PI / 8.0,
        mat::ROOF,
    );
    prim::rect_panel(
        &mut turret,
        v3(-0.16, -t_r - 0.02, t_base + 0.9),
        right,
        up,
        0.32,
        0.62,
        mat::WINDOW,
    );
    prim::stamp(
        &mut m,
        &turret,
        &[
            (-t_off, -t_off, 0.0),
            (t_off, -t_off, 0.0),
            (t_off, t_off, 0.0),
            (-t_off, t_off, 0.0),
        ],
    );

    let porch_y = -half - 0.70;
    prim::boxed(
        &mut m,
        v3(-1.15, porch_y, 0.0),
        v3(1.15, -half + 0.05, 2.30),
        mat::STONE,
        F_SIDES_TOP,
    );
    prim::arched_panel(
        &mut m,
        v3(-0.85, porch_y - 0.03, 0.0),
        right,
        up,
        1.70,
        0.95,
        5,
        mat::STONE_DARK,
    );
    prim::arched_panel(
        &mut m,
        v3(-0.62, porch_y - 0.06, 0.0),
        right,
        up,
        1.24,
        0.82,
        5,
        mat::DOOR,
    );

    m
}

pub fn round_tower(height: f32, radius: f32, seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x54_4F_57_52));
    let mut m = Mesh::new();

    let h = height.max(1.5);
    let rad = radius.max(0.4);
    let sides = 10 + r.below(3);
    let rot = r.range(0.0, PI * 0.25);
    let z_batter = h * 0.22;
    let z_eave = h + 0.16;
    let eave_r = rad * 1.34;

    prim::prism(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_batter,
        rad * 1.12,
        rad,
        sides,
        rot,
        mat::STONE_DARK,
    );
    prim::prism(
        &mut m,
        0.0,
        0.0,
        z_batter,
        h,
        rad,
        rad,
        sides,
        rot,
        mat::STONE,
    );
    prim::prism(
        &mut m,
        0.0,
        0.0,
        h,
        z_eave,
        rad,
        rad * 1.14,
        sides,
        rot,
        mat::STONE_DARK,
    );
    prim::annulus(
        &mut m,
        0.0,
        0.0,
        z_eave,
        rad * 1.14,
        eave_r,
        sides,
        rot,
        mat::ROOF,
    );
    let tip = z_eave + 1.5 * rad + 0.40;
    prim::cone(&mut m, 0.0, 0.0, z_eave, tip, eave_r, sides, rot, mat::ROOF);

    let win_w = (rad * 0.34).clamp(0.24, 0.44);
    let win_h = win_w * 2.1;
    let count = 5;
    for i in 0..count {
        let a = rot + (i as f32) * (PI * 2.0 / 3.0) + r.jitter(0.12);
        let z = 0.9 + (i as f32) * (h - 1.9).max(0.4) / (count as f32 - 1.0);
        let out = v3(a.cos(), a.sin(), 0.0);
        let right = v3(-a.sin(), a.cos(), 0.0);
        let o = out * (rad + 0.02) - right * (win_w * 0.5) + v3(0.0, 0.0, z);
        prim::rect_panel(&mut m, o, right, V3::UP, win_w, win_h, mat::WINDOW);
    }

    if r.chance(0.55) {
        let mast_a = rot + r.range(0.0, PI * 2.0);
        let bx = mast_a.cos() * eave_r * 0.15;
        let by = mast_a.sin() * eave_r * 0.15;
        prim::boxed(
            &mut m,
            v3(bx - 0.05, by - 0.05, tip),
            v3(bx + 0.05, by + 0.05, tip + 1.10),
            mat::WOOD,
            F_SIDES,
        );
        let right = v3(-mast_a.sin(), mast_a.cos(), 0.0);
        prim::rect_panel(
            &mut m,
            v3(bx, by, tip + 0.30),
            right,
            V3::UP,
            0.85,
            0.62,
            mat::BANNER,
        );
    }

    m
}

pub fn rookery_dressing(height: f32, radius: f32, seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x52_4F_4F_4B));
    let mut m = Mesh::new();

    let h = height.max(1.5);
    let rad = radius.max(0.4);
    let z_eave = h + 0.16;
    let eave_r = rad * 1.34;
    let tip = z_eave + 1.5 * rad + 0.40;
    let sides = 10usize;
    let rot = r.range(0.0, PI * 0.25);

    let band_lo = h - 1.60;
    let band_hi = h - 0.88;
    let band_r = rad + 0.46;
    prim::annulus(
        &mut m,
        0.0,
        0.0,
        band_lo,
        rad,
        band_r,
        sides,
        rot,
        mat::STONE_DARK,
    );
    prim::prism(
        &mut m,
        0.0,
        0.0,
        band_lo,
        band_hi,
        band_r,
        band_r,
        sides,
        rot,
        mat::STONE,
    );
    prim::annulus(
        &mut m,
        0.0,
        0.0,
        band_hi,
        band_r - 0.16,
        band_r + 0.08,
        sides,
        rot,
        mat::STONE,
    );

    for i in 0..6 {
        let a = rot + PI * 0.15 + TAU_6 * i as f32;
        let out = v3(a.cos(), a.sin(), 0.0);
        let across = v3(-a.sin(), a.cos(), 0.0);
        prim::rect_panel(
            &mut m,
            out * (band_r + 0.02) - across * 0.13 + v3(0.0, 0.0, band_lo + 0.18),
            across,
            V3::UP,
            0.26,
            0.38,
            mat::STONE_DARK,
        );
        let ledge = out * band_r + v3(0.0, 0.0, band_lo + 0.08);
        prim::quad(
            &mut m,
            ledge - across * 0.21,
            ledge + across * 0.21,
            ledge + across * 0.21 + out * 0.20,
            ledge - across * 0.21 + out * 0.20,
            mat::STONE,
        );
    }

    let broadside = v3(-ROOK_VIEW.sin(), ROOK_VIEW.cos(), 0.0);
    for &offset in &[
        std::f32::consts::FRAC_PI_2,
        -std::f32::consts::FRAC_PI_2,
        2.36,
        -2.36,
    ] {
        let a = ROOK_VIEW + offset + r.jitter(0.10);
        prim::raven(
            &mut m,
            v3(eave_r * a.cos(), eave_r * a.sin(), z_eave + 0.06),
            broadside,
            0.52,
            mat::STONE,
        );
    }
    prim::raven(
        &mut m,
        v3(0.10 * rot.cos(), 0.10 * rot.sin(), tip + 0.02),
        broadside,
        0.52,
        mat::STONE,
    );

    m
}

const TAU_6: f32 = std::f32::consts::TAU / 6.0;

const ROOK_VIEW: f32 = 2.663;

pub fn curtain_wall(length: f32, seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x57_41_4C_4C));
    let mut m = Mesh::new();

    let hl = (length.max(1.0)) * 0.5;
    let half_y = 0.55;
    let plinth_y = 0.68;
    let z_plinth = 0.55 + r.jitter(0.06);
    let z_walk = 2.95 + r.jitter(0.10);

    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_plinth,
        hl,
        plinth_y,
        hl,
        half_y,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-hl, -half_y, z_plinth),
        v3(hl, half_y, z_walk),
        mat::STONE,
        F_SIDES_TOP,
    );
    prim::boxed(
        &mut m,
        v3(-hl, half_y - 0.20, z_walk),
        v3(hl, half_y, z_walk + 0.34),
        mat::STONE,
        prim::F_NY | prim::F_PZ,
    );
    prim::merlons(
        &mut m,
        v3(-hl, -half_y, z_walk),
        v3(hl, -half_y, z_walk),
        v3(0.0, 1.0, 0.0),
        0.80,
        0.36,
        1.30,
        14,
        mat::STONE,
    );

    m
}

pub fn gatehouse(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x47_41_54_45));
    let mut m = Mesh::new();

    let block_hx = 1.55;
    let block_hy = 1.40;
    let z_block = 4.55 + r.jitter(0.12);
    let z_corbel = z_block + 0.38;
    let gate_hw = 1.20;
    let springing = 2.35;
    let segs = 5;
    let head_top = springing + gate_hw + 0.25;
    let arch = prim::arch_profile(segs, gate_hw, gate_hw);

    for &y in &[-block_hy, block_hy] {
        let right = v3(1.0, 0.0, 0.0);
        let o = v3(-block_hx, y, 0.0);
        prim::rect_panel(
            &mut m,
            v3(-block_hx, y, 0.0),
            right,
            V3::UP,
            block_hx - gate_hw,
            z_corbel,
            mat::STONE,
        );
        prim::rect_panel(
            &mut m,
            v3(gate_hw, y, 0.0),
            right,
            V3::UP,
            block_hx - gate_hw,
            z_corbel,
            mat::STONE,
        );
        for i in 0..segs {
            let a = arch[i];
            let b = arch[i + 1];
            let pa = v3(a[0], y, springing + a[1]);
            let pb = v3(b[0], y, springing + b[1]);
            let qa = v3(a[0], y, head_top);
            let qb = v3(b[0], y, head_top);
            prim::tri_planar(&mut m, o, right, V3::UP, [pa, pb, qb], mat::STONE_DARK);
            prim::tri_planar(&mut m, o, right, V3::UP, [pa, qb, qa], mat::STONE_DARK);
        }
        prim::rect_panel(
            &mut m,
            v3(-gate_hw, y, head_top),
            right,
            V3::UP,
            gate_hw * 2.0,
            z_corbel - head_top,
            mat::STONE,
        );
    }

    for i in 0..segs {
        let a = arch[i];
        let b = arch[i + 1];
        prim::quad(
            &mut m,
            v3(a[0], -block_hy, springing + a[1]),
            v3(b[0], -block_hy, springing + b[1]),
            v3(b[0], block_hy, springing + b[1]),
            v3(a[0], block_hy, springing + a[1]),
            mat::STONE_DARK,
        );
    }
    for &x in &[-gate_hw, gate_hw] {
        prim::quad(
            &mut m,
            v3(x, -block_hy, 0.0),
            v3(x, block_hy, 0.0),
            v3(x, block_hy, springing),
            v3(x, -block_hy, springing),
            mat::STONE_DARK,
        );
    }
    prim::quad(
        &mut m,
        v3(-gate_hw, -block_hy, 0.01),
        v3(gate_hw, -block_hy, 0.01),
        v3(gate_hw, block_hy, 0.01),
        v3(-gate_hw, block_hy, 0.01),
        mat::PATH,
    );
    prim::rect_panel(
        &mut m,
        v3(-gate_hw + 0.06, 0.35, 0.0),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        (gate_hw - 0.06) * 2.0,
        springing - 0.15,
        mat::DOOR,
    );

    prim::boxed(
        &mut m,
        v3(-block_hx - 0.18, -block_hy - 0.18, z_block),
        v3(block_hx + 0.18, block_hy + 0.18, z_corbel),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    for &(y, inward) in &[(-block_hy - 0.18, 1.0_f32), (block_hy + 0.18, -1.0)] {
        prim::merlons(
            &mut m,
            v3(-block_hx - 0.18, y, z_corbel),
            v3(block_hx + 0.18, y, z_corbel),
            v3(0.0, inward, 0.0),
            0.72,
            0.36,
            1.15,
            4,
            mat::STONE,
        );
    }

    let drum_x = 2.30;
    let drum_r = 1.30;
    let drum_h = 4.90 + r.jitter(0.15);
    let mut drum = round_tower(drum_h, drum_r, sub_seed(seed, 0x44_52_55_4D));
    drum.tris
        .retain(|t| t.mat != mat::BANNER && t.mat != mat::WOOD);
    prim::stamp(&mut m, &drum, &[(-drum_x, 0.0, 0.0), (drum_x, 0.0, 0.0)]);

    m
}
