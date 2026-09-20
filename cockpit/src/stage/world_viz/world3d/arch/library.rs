//! The library — the hero building of the realm — plus its interior variant.
//!
//! Art direction: one long hall, one very steep roof, and a rhythm of buttress
//! / window / buttress along the flank. At dot scale the building has to read
//! from three things only — the ridge line, the lantern spike breaking it, and
//! the bright ladder of WINDOW bays under the eave — so everything else is kept
//! deliberately plain. The portico exists to give the −y approach a dark
//! shadowed mouth instead of a flat wall.

use super::super::math::{V3, v3};
use super::super::mesh::{Mesh, mat};
use super::prim::{self, F_ALL, F_NX, F_NZ, F_PX, F_SIDES, F_SIDES_TOP};
use super::rng::{Rng, sub_seed};

/// Long-hall library, exterior. **Footprint 13.0 × 8.7 tiles**
/// (x −6.5..6.5 including the roof oversail, y −5.0..3.7 including the portico
/// steps and the flank buttresses), **~9.8 tiles tall** to the lantern finial.
/// Centred on the origin, ground at z = 0, entrance portico on the −y face.
///
/// Battered plinth → aisle walls with corner pilasters and eight weathered
/// buttresses → twelve tall arched WINDOW bays → steep pitched ROOF with a
/// fascia'd eave and a ridge crest → an octagonal crossing lantern that breaks
/// the ridge → a four-column portico over an arched DOOR.
pub(crate) fn library(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x4C_49_42_52)); // "LIBR"
    let mut m = Mesh::new();

    let hx = 6.0_f32;
    let hy = 3.0_f32;
    let z_plinth = 0.50;
    let z_eave = 4.60 + r.jitter(0.10);
    let z_ridge = z_eave + 2.80 + r.jitter(0.15);

    // Plinth + walls.
    prim::battered(
        &mut m,
        0.0,
        0.0,
        0.0,
        z_plinth,
        hx + 0.30,
        hy + 0.30,
        hx,
        hy,
        mat::STONE_DARK,
    );
    prim::boxed(
        &mut m,
        v3(-hx, -hy, z_plinth),
        v3(hx, hy, z_eave),
        mat::STONE,
        F_SIDES,
    );

    // Corner pilasters — they thicken the corners so the mass does not look
    // like a folded sheet in silhouette.
    for &(sx, sy) in &[(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let cx = sx * (hx - 0.30);
        let cy = sy * (hy - 0.30);
        prim::battered(
            &mut m,
            cx,
            cy,
            0.0,
            z_eave + 0.45,
            0.52,
            0.52,
            0.44,
            0.44,
            mat::STONE,
        );
        prim::boxed(
            &mut m,
            v3(cx - 0.44, cy - 0.44, z_eave + 0.45),
            v3(cx + 0.44, cy + 0.44, z_eave + 0.52),
            mat::STONE_DARK,
            prim::F_PZ,
        );
    }

    // Flank buttresses with a sloped weathering top.
    for &sy in &[-1.0_f32, 1.0] {
        for &x in &[-5.10_f32, -2.55, 2.55, 5.10] {
            let y_in = sy * hy;
            let y_out = sy * (hy + 0.70);
            let top = 3.55 + r.jitter(0.12);
            prim::boxed(
                &mut m,
                v3(x - 0.31, y_in.min(y_out), 0.0),
                v3(x + 0.31, y_in.max(y_out), top),
                mat::STONE,
                F_SIDES,
            );
            // Weathering: the outer top edge slopes back into the wall.
            prim::quad(
                &mut m,
                v3(x - 0.31, y_out, top),
                v3(x + 0.31, y_out, top),
                v3(x + 0.31, y_in, top + 0.55),
                v3(x - 0.31, y_in, top + 0.55),
                mat::STONE_DARK,
            );
        }
    }

    // Tall arched window bays, one per structural bay.
    let win_w = 0.95;
    let bays = [-5.55_f32, -3.83, -1.28, 1.28, 3.83, 5.55];
    for &sy in &[-1.0_f32, 1.0] {
        let y = sy * (hy + 0.02);
        let right = v3(sy, 0.0, 0.0); // keeps UV handedness consistent per face
        for &x in &bays {
            prim::arched_panel(
                &mut m,
                v3(x - sy * win_w * 0.5, y, 1.50),
                right,
                V3::UP,
                win_w,
                2.00,
                5,
                mat::WINDOW,
            );
        }
    }

    // Roof, gable ends, rose windows.
    prim::gable_roof(
        &mut m,
        0.0,
        0.0,
        hx + 0.50,
        hy + 0.55,
        z_eave,
        z_ridge,
        0.16,
        0.14,
        mat::ROOF,
    );
    for &sx in &[-1.0_f32, 1.0] {
        prim::gable_end(&mut m, sx * hx, 0.0, hy, z_eave, z_ridge, mat::STONE);
        prim::poly_fan(
            &mut m,
            v3(sx * (hx + 0.02), 0.0, z_eave + 0.95),
            v3(0.0, sx, 0.0),
            V3::UP,
            0.72,
            8,
            0.0,
            mat::WINDOW,
        );
    }

    // Crossing lantern — the spike that breaks the ridge line.
    let lan_r = 1.15;
    let lan_base = z_ridge - 0.80;
    let lan_top = lan_base + 1.90;
    prim::prism(
        &mut m,
        0.0,
        0.0,
        lan_base,
        lan_top,
        lan_r,
        lan_r,
        8,
        0.0,
        mat::STONE,
    );
    for i in 0..8 {
        let a = std::f32::consts::TAU * (i as f32 + 0.5) / 8.0;
        let out = v3(a.cos(), a.sin(), 0.0);
        let right = v3(-a.sin(), a.cos(), 0.0);
        let w = lan_r * 0.52;
        prim::rect_panel(
            &mut m,
            out * (lan_r * 0.96) - right * (w * 0.5) + v3(0.0, 0.0, lan_base + 0.35),
            right,
            V3::UP,
            w,
            1.10,
            mat::WINDOW,
        );
    }
    prim::annulus(
        &mut m,
        0.0,
        0.0,
        lan_top,
        lan_r,
        lan_r * 1.30,
        8,
        0.0,
        mat::ROOF,
    );
    prim::cone(
        &mut m,
        0.0,
        0.0,
        lan_top,
        lan_top + 1.35,
        lan_r * 1.30,
        8,
        0.0,
        mat::ROOF,
    );

    // Portico: steps, four columns, entablature, pediment.
    for i in 0..3 {
        let f = i as f32;
        let z0 = f * 0.18;
        let y_front = -5.00 + f * 0.22;
        prim::boxed(
            &mut m,
            v3(-3.50 + f * 0.10, y_front, z0),
            v3(3.50 - f * 0.10, -3.00, z0 + 0.18),
            mat::STONE_DARK,
            prim::F_NY | prim::F_PZ,
        );
    }
    let col_top = 3.70;
    for &cx in &[-2.75_f32, -0.95, 0.95, 2.75] {
        prim::prism(
            &mut m,
            cx,
            -4.25,
            0.54,
            col_top,
            0.30,
            0.27,
            6,
            0.0,
            mat::STONE,
        );
    }
    prim::boxed(
        &mut m,
        v3(-3.35, -4.75, col_top),
        v3(3.35, -2.90, col_top + 0.55),
        mat::STONE_DARK,
        F_ALL,
    );
    let ped_eave = col_top + 0.55;
    let ped_ridge = ped_eave + 1.25;
    for &fy in &[-4.75_f32, -2.90] {
        prim::tri(
            &mut m,
            [
                v3(-3.35, fy, ped_eave),
                v3(3.35, fy, ped_eave),
                v3(0.0, fy, ped_ridge),
            ],
            [[0.0, 0.0], [6.70, 0.0], [3.35, 1.25]],
            mat::STONE,
        );
    }
    for &sx in &[-1.0_f32, 1.0] {
        prim::quad(
            &mut m,
            v3(sx * 3.35, -4.75, ped_eave),
            v3(sx * 3.35, -2.90, ped_eave),
            v3(0.0, -2.90, ped_ridge),
            v3(0.0, -4.75, ped_ridge),
            mat::ROOF,
        );
    }

    // Main door under the portico, with a heavy stone surround.
    prim::arched_panel(
        &mut m,
        v3(-1.05, -3.03, 0.54),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        2.10,
        2.30,
        5,
        mat::STONE_DARK,
    );
    prim::arched_panel(
        &mut m,
        v3(-0.82, -3.06, 0.54),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        1.64,
        2.10,
        5,
        mat::DOOR,
    );

    m
}

/// Library interior — the reading hall. **Footprint 11.4 × 5.4 tiles**
/// (the inside face of the shell), **6.8 tiles to the ceiling ridge**, centred
/// on the origin with the FLOOR at z = 0.
///
/// Everything faces inward: FLOOR slab, four wall panels with ten arched
/// WINDOW bays, an open timber ceiling on five tie beams, six free-standing
/// BOOKS cases in two rows with WOOD carcasses, wall shelving on both end
/// walls, two long reading tables, a hearth and a few candle glows.
///
/// Detail here runs finer than the exterior builders (down to ~0.2 tiles)
/// because interiors are only ever seen from inside the room.
pub(crate) fn library_interior(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x49_4E_54_52)); // "INTR"
    let mut m = Mesh::new();

    let hx = 5.70_f32;
    let hy = 2.70_f32;
    let z_wall = 4.40;
    let z_ridge = 6.80;

    // Floor and shell.
    prim::quad(
        &mut m,
        v3(-hx, -hy, 0.0),
        v3(hx, -hy, 0.0),
        v3(hx, hy, 0.0),
        v3(-hx, hy, 0.0),
        mat::FLOOR,
    );
    prim::boxed(
        &mut m,
        v3(-hx, -hy, 0.0),
        v3(hx, hy, z_wall),
        mat::STONE,
        F_SIDES,
    );
    // Open timber ceiling.
    prim::quad(
        &mut m,
        v3(-hx, -hy, z_wall),
        v3(hx, -hy, z_wall),
        v3(hx, 0.0, z_ridge),
        v3(-hx, 0.0, z_ridge),
        mat::WOOD,
    );
    prim::quad(
        &mut m,
        v3(-hx, hy, z_wall),
        v3(hx, hy, z_wall),
        v3(hx, 0.0, z_ridge),
        v3(-hx, 0.0, z_ridge),
        mat::WOOD,
    );
    for &x in &[-4.40_f32, -2.20, 0.0, 2.20, 4.40] {
        prim::boxed(
            &mut m,
            v3(x - 0.13, -hy, z_wall - 0.15),
            v3(x + 0.13, hy, z_wall + 0.15),
            mat::WOOD,
            F_NZ | F_NX | F_PX,
        );
    }

    // Arched window bays on both flanks.
    let win_w = 0.85;
    for &sy in &[-1.0_f32, 1.0] {
        let y = sy * (hy - 0.02);
        let right = v3(-sy, 0.0, 0.0);
        for &x in &[-4.60_f32, -2.30, 0.0, 2.30, 4.60] {
            prim::arched_panel(
                &mut m,
                v3(x + sy * win_w * 0.5, y, 1.35),
                right,
                V3::UP,
                win_w,
                1.75,
                5,
                mat::WINDOW,
            );
        }
    }

    // Shelf stacks projecting off the −y wall, like a real library.
    //
    // They used to run *along* the room in two rows either side of a centre
    // aisle, and that arrangement has exactly one camera: down the aisle. From
    // there both rows are edge-on, so the hall rendered as a brown corridor
    // with a bright hole at the end and not a single book face in it — the
    // whole reason the hero interior came back for another pass. Turned
    // ninety degrees they present their long faces to anyone standing toward
    // the −x end, and because each one hides part of the next they telescope
    // into a receding rhythm instead of a flat wall of spines. The near half
    // of the room stays open, which is what the hearth needs to be seen past
    // them.
    let mut stack = Mesh::new();
    let (sw, sd, sh) = (1.60_f32, 0.56_f32, 2.50_f32); // reach into the room, thickness, height
    prim::boxed(
        &mut stack,
        v3(-sd * 0.5, -sw, 0.0),
        v3(sd * 0.5, 0.0, sh),
        mat::WOOD,
        F_SIDES_TOP,
    );
    for shelf in 0..3 {
        let z0 = 0.22 + shelf as f32 * 0.76;
        for &sx in &[-1.0_f32, 1.0] {
            prim::rect_panel(
                &mut stack,
                v3(sx * (sd * 0.5 + 0.02), -sw + 0.14, z0),
                v3(0.0, 1.0, 0.0),
                V3::UP,
                sw - 0.28,
                0.60,
                mat::BOOKS,
            );
        }
    }
    prim::stamp(
        &mut m,
        &stack,
        &[
            (-1.20, -hy + sw, 0.0),
            (1.30, -hy + sw, 0.0),
            (3.80, -hy + sw, 0.0),
        ],
    );

    // A low run of shelving under the +y bays, so the wall the stacks do not
    // reach is still a library wall and not bare masonry.
    prim::boxed(
        &mut m,
        v3(-4.70, hy - 0.54, 0.0),
        v3(4.70, hy - 0.02, 1.15),
        mat::WOOD,
        F_SIDES_TOP,
    );
    for shelf in 0..2 {
        prim::rect_panel(
            &mut m,
            v3(-4.56, hy - 0.57, 0.18 + shelf as f32 * 0.52),
            v3(1.0, 0.0, 0.0),
            V3::UP,
            9.12,
            0.42,
            mat::BOOKS,
        );
    }

    // Wall shelving on both end walls.
    for &sx in &[-1.0_f32, 1.0] {
        let x = sx * (hx - 0.28);
        prim::boxed(
            &mut m,
            v3(x - 0.28, -1.70, 0.0),
            v3(x + 0.28, 1.70, 2.60),
            mat::WOOD,
            F_SIDES_TOP,
        );
        for shelf in 0..3 {
            let z0 = 0.24 + shelf as f32 * 0.78;
            prim::rect_panel(
                &mut m,
                v3(x - sx * 0.30, -1.55, z0),
                v3(0.0, 1.0, 0.0),
                V3::UP,
                3.10,
                0.62,
                mat::BOOKS,
            );
        }
    }

    // Two long reading tables down the open half of the room, plus candle
    // glows. Shifted +x off the old symmetric pair: the staged eye stands at
    // x ≈ −4, and a table centred on −2.6 put the camera *inside* it.
    for &tx in &[-1.10_f32, 2.90] {
        prim::boxed(
            &mut m,
            v3(tx - 1.80, -0.55, 0.76),
            v3(tx + 1.80, 0.55, 0.88),
            mat::WOOD,
            F_ALL,
        );
        for &sx in &[-1.0_f32, 1.0] {
            prim::boxed(
                &mut m,
                v3(tx + sx * 1.35 - 0.10, -0.42, 0.0),
                v3(tx + sx * 1.35 + 0.10, 0.42, 0.76),
                mat::WOOD,
                F_SIDES,
            );
        }
        for &sx in &[-1.0_f32, 1.0] {
            prim::rect_panel(
                &mut m,
                v3(tx + sx * 0.90 - 0.11, 0.0, 0.88),
                v3(1.0, 0.0, 0.0),
                V3::UP,
                0.22,
                0.34,
                mat::FIRE,
            );
        }
    }

    // Hearth on the +x end wall.
    let hearth_x = hx - 0.05;
    prim::boxed(
        &mut m,
        v3(hearth_x - 0.85, -1.05, 0.0),
        v3(hearth_x, 1.05, 2.20),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    prim::arched_panel(
        &mut m,
        v3(hearth_x - 0.88, -0.70, 0.05),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        1.40,
        0.80,
        5,
        mat::FIRE,
    );

    // Door out on −y behind the eye, nudged by seed so two halls are not
    // twins. It sits west of the first stack rather than under one of them.
    let door_x = -4.30 + r.jitter(0.35);
    prim::rect_panel(
        &mut m,
        v3(door_x - 0.62, -hy + 0.02, 0.0),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        1.24,
        2.10,
        mat::DOOR,
    );

    // Moonlight through a +y bay, landing on the flags between the stacks and
    // the hearth. The hall is otherwise lit entirely by candle and fire, and a
    // room with only warm light in it has no depth cue left but distance — one
    // cold source is what puts air back in it.
    let shaft_x = 2.30_f32;
    prim::quad(
        &mut m,
        v3(shaft_x - 1.00, 0.55, 0.03),
        v3(shaft_x + 0.80, 0.55, 0.03),
        v3(shaft_x + 0.95, 2.05, 0.03),
        v3(shaft_x - 0.85, 2.05, 0.03),
        mat::PATH,
    );
    prim::quad(
        &mut m,
        v3(shaft_x - 0.58, 0.92, 0.05),
        v3(shaft_x + 0.40, 0.92, 0.05),
        v3(shaft_x + 0.50, 1.90, 0.05),
        v3(shaft_x - 0.48, 1.90, 0.05),
        mat::MOONLIGHT,
    );
    prim::light_shaft(
        &mut m,
        shaft_x - 0.06,
        (hy - 0.12, 2.10),
        (1.40, 0.07),
        0.40,
        1.30,
        mat::MOONLIGHT,
    );

    m
}
