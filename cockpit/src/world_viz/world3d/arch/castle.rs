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

/// Great square keep. **Footprint 7.5 × 7.5 tiles** (6.0 × 6.0 main block plus
/// four corner turrets whose eaves reach ±3.75), **~5.9 tiles tall** to the
/// turret finials. Centred on the origin, ground at z = 0, entrance porch on
/// the −y face.
///
/// Battered base → shaft → corbel table → crenellated deck, with two rows of
/// narrow punched windows per face and cone-capped corner turrets.
pub(crate) fn keep(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x4B_45_45_50)); // "KEEP"
    let mut m = Mesh::new();

    let half = 3.0_f32;
    let batter_h = half + 0.35;
    let z_batter = 1.05 + r.jitter(0.08);
    let z_wall = 4.10 + r.jitter(0.12);
    let z_corbel = z_wall + 0.35;
    let deck_half = half + 0.20;

    // Battered base, shaft, corbel table, deck.
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

    // One decorated face, then four rotations — a keep reads best symmetric,
    // and the rotate-4 trick keeps the triangle stream tiny and deterministic.
    let mut face = Mesh::new();
    let wall_y = -half - 0.02;
    let right = v3(1.0, 0.0, 0.0);
    let up = V3::UP;
    let win_w = 0.42;
    let win_h = 0.95;
    // Lower row skips the centre bay (the porch stands there on the front).
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
    // Crenellations, inset so the corner turrets own the corners cleanly.
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

    // Corner turrets: octagonal shaft, flared eave, conical cap.
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

    // Entrance porch on −y: a projecting block with an arched door.
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

/// Round tower. **Footprint 2.7 × `radius` across** (the roof overhang is the
/// widest thing), **total height `height` + 1.5 × `radius` + 0.56**, plus 1.10
/// more when the seed grants a banner mast. Centred on the origin, ground at
/// z = 0.
///
/// A 10–12 sided battered prism with a corbelled eave, a steep conical ROOF
/// that oversails the shaft (the overhang is the read at dot scale), staggered
/// punched windows spiralling up the shaft, and — for some seeds — a BANNER on
/// a short mast.
pub(crate) fn round_tower(height: f32, radius: f32, seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x54_4F_57_52)); // "TOWR"
    let mut m = Mesh::new();

    let h = height.max(1.5);
    let rad = radius.max(0.4);
    let sides = 10 + r.below(3); // 10..=12
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

    // Staggered windows: each one a third of the way round from the last, so
    // the tower reads as turning even when only one face is lit.
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

/// Rookery dressing for a [`round_tower`] built with the same `height` and
/// `radius`: merge it onto the tower and the tower becomes the rookery.
/// **Footprint 2.9 × `radius` across** (the rim birds hang a hand's width past
/// the eave), **top 0.42 above the tower's finial**. Centred on the origin.
///
/// Three things, and they are the three a bird tower has that a drum tower
/// does not:
/// 1. a **corbelled dovecote band** under the crown — a ring standing 0.46
///    proud of the shaft, bracketed by a dark corbel soffit below and a lit
///    drip course above. Those two horizontals are the whole read at 12.8
///    tiles: the soffit is a down-face, so it takes neither moon nor sky and
///    goes almost black, while the drip's top face takes both. A band that
///    only projected 0.36 threw a shadow one dot deep and dithered away into
///    the shaft, which is what sent this back for another pass;
/// 2. **bird boxes** punched into that band as dark holes with pale landing
///    ledges under them, so the band reads as pierced rather than as a moulding;
/// 3. **perched rooks** hanging just past the eave rim and one on the finial —
///    [`prim::raven`] profiles crossed in two vertical planes, so a bird reads
///    as a bird from every bearing instead of vanishing edge-on.
///
/// Nothing here is finer than ~0.2 tiles, and two deliberate cheats earn their
/// keep at dot scale: the birds are oversized (a life-sized rook is a sub-dot
/// at vista range, and a roofline of sub-dots just looks noisy), and they are
/// tagged STONE rather than STONE_DARK. A black bird against a night sky is
/// black on black; a moonlit one is a pale notch on the rim, which is the only
/// version of it anybody can actually see.
pub(crate) fn rookery_dressing(height: f32, radius: f32, seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x52_4F_4F_4B)); // "ROOK"
    let mut m = Mesh::new();

    // Mirror `round_tower`'s own geometry so the dressing lands on the tower
    // rather than near it.
    let h = height.max(1.5);
    let rad = radius.max(0.4);
    let z_eave = h + 0.16;
    let eave_r = rad * 1.34;
    let tip = z_eave + 1.5 * rad + 0.40;
    let sides = 10usize;
    let rot = r.range(0.0, PI * 0.25);

    // Dovecote band, bracketed above and below.
    let band_lo = h - 1.60;
    let band_hi = h - 0.88;
    let band_r = rad + 0.46;
    // The corbel soffit: a flat ring facing straight down, which is the darkest
    // surface the shader can make (no moon key, minimum sky bounce). Its depth
    // *is* the shadow's width.
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
    // ...and the drip course over it, an up-facing ring that takes both moon
    // and sky. Dark line, pale band, bright line: three horizontals stacked in
    // half a tile, which is what survives the Bayer screen at vista range.
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

    // Bird boxes: dark holes with a pale ledge under each.
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
        // A short, pale landing ledge. Short on purpose: the band already
        // stands 0.46 proud, and the tower's own eave is the outer limit
        // everything on it has to live inside.
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

    // Perched rooks, on the eave rim and one on the finial. A bird inside the
    // tower's own outline is a dark shape on a dark shaft and disappears; one
    // sitting on the rim notches the silhouette, which is the only place a
    // viewer can find it. Tagged STONE, not STONE_DARK — see the module note:
    // a black bird against a night sky is black on black.
    // The rooks are **staged**, not scattered, and both halves of that are
    // deliberate.
    //
    // *Where*: a bird standing anywhere but the drum's limb projects onto the
    // cone behind it, and the cone's moonward facets shade to almost exactly
    // the value a billboard turned toward the camera can reach — so it
    // vanishes into the roof. On the limb it hangs against sky, which is the
    // only backdrop in this frame a rook can be read against. Two of the four
    // therefore take the limbs of the staged bearing, and the other two fill
    // in for a camera that has walked off it.
    //
    // *Facing*: all of them, and the finial bird, lie broadside to that same
    // bearing. The module note above already admits this tower is built for
    // one camera — the north-west drum is the only corner whose lit limb
    // faces open ground — and a bird is far too small to hedge on.
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
    // The finial bird is the skyline note, and the one thing on this tower
    // nothing else can stand in for.
    prim::raven(
        &mut m,
        v3(0.10 * rot.cos(), 0.10 * rot.sin(), tip + 0.02),
        broadside,
        0.52,
        mat::STONE,
    );

    m
}

/// A sixth of a turn — the dovecote band's hole pitch.
const TAU_6: f32 = std::f32::consts::TAU / 6.0;

/// Bearing from the north-west drum to the mark the Rookery is staged from
/// (`world3d::VANTAGES[2]`: eye (−22.5, 16.5) looking at (−11, 11)), in
/// radians. The rooks are placed and turned against it — see the note beside
/// them for why a bird this small cannot be built camera-agnostic.
const ROOK_VIEW: f32 = 2.663;

/// Curtain wall segment. Runs along **+x**, centred on the origin, outer face
/// toward **−y**. **Footprint `length` × 1.36 tiles**, **3.75 tiles tall** to
/// the top of the teeth. Rotate/translate to build a run; `rotated_z(PI/2)`
/// turns it into the east wall of a court.
///
/// Battered plinth, wall body, walkway deck, an outer crenellated parapet and a
/// plain inner breastwork. Tooth count is derived from the length and capped so
/// any length stays inside the triangle budget.
pub(crate) fn curtain_wall(length: f32, seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x57_41_4C_4C)); // "WALL"
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
    // Inner breastwork: low, plain, no teeth — it must not compete with the
    // outer silhouette.
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

/// Gatehouse: twin drum towers flanking a faceted round-arched gate passage.
/// **Footprint 8.1 × 3.5 tiles**, **~7.4 tiles tall** to the drum finials.
/// Centred on the origin; the passage runs along **y** and the approach face is
/// **−y**, matching [`curtain_wall`]'s outer face so the two butt together.
pub(crate) fn gatehouse(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x47_41_54_45)); // "GATE"
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

    // Front and back faces: piers, arch spandrels, and the wall above.
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

    // Passage: intrados barrel, side walls, floor.
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
    // Gate leaves, set back inside the passage.
    prim::rect_panel(
        &mut m,
        v3(-gate_hw + 0.06, 0.35, 0.0),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        (gate_hw - 0.06) * 2.0,
        springing - 0.15,
        mat::DOOR,
    );

    // Machicolated crown + crenellations on the approach and inner edges.
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

    // The two drums. Taller than the block so the gate reads as a gate.
    let drum_x = 2.30;
    let drum_r = 1.30;
    let drum_h = 4.90 + r.jitter(0.15);
    let mut drum = round_tower(drum_h, drum_r, sub_seed(seed, 0x44_52_55_4D));
    // Drums never carry banners — the mast clutters the gate silhouette.
    drum.tris
        .retain(|t| t.mat != mat::BANNER && t.mat != mat::WOOD);
    prim::stamp(&mut m, &drum, &[(-drum_x, 0.0, 0.0), (drum_x, 0.0, 0.0)]);

    m
}
