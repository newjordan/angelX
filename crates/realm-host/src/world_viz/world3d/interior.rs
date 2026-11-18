//! Interior stages for `/world enter` — OWNED BY THE INTERIORS AGENT
//! (Phase D, docs/plans/world3d-spec.md ownership rules). Camera staging and
//! scene selection for the inside of buildings; the ride seam calls in here
//! directly so world3d.rs stays the vista-staging file.
//!
//! # What an interior is
//! Eight rooms, one per civic landmark. The Scriptorium gets the hand-built
//! hero hall from [`arch::library_interior`]; the other seven share the
//! parameterized great hall in [`chamber_mesh`] and differ by **shell material,
//! room proportion, wall openings and one identity piece** — the hearth, the
//! portcullis, the perch row, the forge, the rose window, the round table, the
//! star slit. Distinct-at-a-glance beats detailed: at 96 × 72 braille dots a
//! room reads from three things only — where the light is, what mass sits in
//! front of it, and the shape of the ceiling.
//!
//! # Light
//! There is no volumetric light in the rasterizer, so an interior is lit by
//! what it can put on screen:
//! - `WINDOW` (emissive, fog-proof) — the window bays, and `FIRE`, its live
//!   twin: the hearth mouth, the candle flames, the brazier coals, the hung
//!   lamps. Identical at rest; the split is what lets the fire breathe
//!   ([`firelight`]) while the glazing holds still.
//! - the **lantern gain** on `FLOOR`/`PATH`/`GRASS`/`WATER` (raster.rs:509) —
//!   the rider's lantern pools on the ground for ~5.5 tiles, which is what
//!   keeps the near floor off black and gives the room its depth ramp.
//! - the **moon key**: the shader flips a face's normal toward the eye, so an
//!   interior wall the camera looks at has an inward normal, and a room whose
//!   far wall faces −x/−y catches the same moon lambert the exteriors do.
//! - real **night sky** through a punched opening (the gate mouth, the rookery
//!   loft hatch, the observatory slit). Rooms that open on the sky orient
//!   themselves by `yaw` so the moon lands inside the opening.
//!
//! # Determinism
//! Meshes are built once per building behind [`super::scene::interior_scene`]'s
//! fixed `OnceLock` array and keyed on nothing but the building index. The
//! camera is a pure function of the `RayView` the ride already built plus the
//! authored table below — the exact inputs `cinematic_key()`'s interior branch
//! (building + `tick / 2` + the world3d flag) already covers. The hearth
//! flicker rides that same `tick / 2` bucket and nothing else, which is why it
//! is cache-safe by construction: a fire can only change on a frame the ride
//! was already going to redraw.
#![allow(dead_code)]

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::arch;
use super::arch::prim::{self, F_ALL, F_NX, F_NY, F_NZ, F_PX, F_PY, F_PZ, F_SIDES, F_SIDES_TOP};
use super::arch::rng::{Rng, sub_seed};
use super::instruments;
use super::math::{V3, v3};
use super::mesh::{Mesh, mat};
use super::props;
use super::raster::{self, View3};
use super::scene::SCENE_SEED;

const MOON_AZIMUTH: f32 = super::MOON_BEARING * TAU;

const BASE_HEADING: f32 = -FRAC_PI_2;

const SWING_LIMIT: f32 = 1.60;

const SWING_GAIN: f32 = 0.55;

const PARALLAX: f32 = 0.55;

const VERT_FOV_MAX: f32 = 1.12;

struct Chamber {
    hx: f32,
    hy: f32,
    z_wall: f32,
    z_ridge: f32,
    shell: u8,
    bays: &'static [f32],
    bay_sill: f32,
    shafts: &'static [f32],
    yaw: f32,
    eye: (f32, f32),
    eye_h: f32,
    anchor: (f32, f32, f32),
    frame_x: f32,
    frame_y: f32,
    lens: f32,
    hero: &'static str,
}

const CHAMBERS: [Chamber; 8] = [
    Chamber {
        hx: 6.0,
        hy: 4.0,
        z_wall: 4.0,
        z_ridge: 5.9,
        shell: mat::STONE,
        bays: &[-3.0, 0.6],
        bay_sill: 2.05,
        shafts: &[0.6],
        yaw: 0.0,
        eye: (-3.9, -1.15),
        eye_h: 1.72,
        anchor: (5.1, 0.0, 1.30),
        frame_x: 0.61,
        frame_y: 0.50,
        lens: 0.94,
        hero: "the long hearth burning at the head of the hall",
    },
    Chamber {
        hx: 6.2,
        hy: 2.9,
        z_wall: 3.8,
        z_ridge: 5.1,
        shell: mat::STONE,
        bays: &[-3.4],
        bay_sill: 1.95,
        shafts: &[],
        yaw: MOON_AZIMUTH - 0.16,
        eye: (-4.2, 0.95),
        eye_h: 1.62,
        anchor: (6.2, 0.0, 1.70),
        frame_x: 0.44,
        frame_y: 0.53,
        lens: 0.86,
        hero: "the portcullis down across the moonlit gate mouth",
    },
    Chamber {
        hx: 5.0,
        hy: 3.4,
        z_wall: 3.3,
        z_ridge: 5.6,
        shell: mat::WOOD,
        bays: &[-3.2],
        bay_sill: 1.70,
        shafts: &[],
        yaw: MOON_AZIMUTH - 0.10,
        eye: (-2.9, -0.90),
        eye_h: 1.45,
        anchor: (5.0, 0.0, 2.30),
        frame_x: 0.57,
        frame_y: 0.46,
        lens: 0.80,
        hero: "the perch rows black against the open loft door",
    },
    Chamber {
        hx: 5.7,
        hy: 2.7,
        z_wall: 4.4,
        z_ridge: 6.8,
        shell: mat::STONE,
        bays: &[],
        bay_sill: 1.35,
        shafts: &[],
        yaw: 0.0,
        eye: (-3.95, -0.30),
        eye_h: 1.82,
        anchor: (4.85, 0.0, 1.00),
        frame_x: 0.62,
        frame_y: 0.55,
        lens: 0.86,
        hero: "the reading hall past the shelf stacks to the hearth",
    },
    Chamber {
        hx: 5.0,
        hy: 3.6,
        z_wall: 3.4,
        z_ridge: 5.2,
        shell: mat::WOOD,
        bays: &[-2.6],
        bay_sill: 1.90,
        shafts: &[],
        yaw: 0.0,
        eye: (-2.6, 1.05),
        eye_h: 1.58,
        anchor: (4.85, -0.45, 1.15),
        frame_x: 0.63,
        frame_y: 0.56,
        lens: 0.92,
        hero: "the forge fire in its hood over the anvil",
    },
    Chamber {
        hx: 6.0,
        hy: 3.2,
        z_wall: 4.6,
        z_ridge: 7.0,
        shell: mat::STONE,
        bays: &[-3.6, -1.2, 2.9],
        bay_sill: 2.30,
        shafts: &[2.9],
        yaw: 0.0,
        eye: (-4.2, 0.75),
        eye_h: 1.60,
        anchor: (5.9, 0.0, 3.25),
        frame_x: 0.45,
        frame_y: 0.33,
        lens: 0.94,
        hero: "the rose window over the altar tapers",
    },
    Chamber {
        hx: 6.3,
        hy: 5.2,
        z_wall: 4.4,
        z_ridge: 6.6,
        shell: mat::STONE,
        bays: &[-2.8, 2.8],
        bay_sill: 2.20,
        shafts: &[2.8],
        yaw: 0.0,
        eye: (-4.7, -2.60),
        eye_h: 2.62,
        anchor: (0.0, 0.0, 0.92),
        frame_x: 0.56,
        frame_y: 0.66,
        lens: 0.96,
        hero: "the lit ring on the round table and its empty seats",
    },
    Chamber {
        hx: 5.0,
        hy: 4.0,
        z_wall: 4.0,
        z_ridge: 6.6,
        shell: mat::STONE,
        bays: &[-3.0],
        bay_sill: 1.85,
        shafts: &[],
        yaw: MOON_AZIMUTH + 1.15,
        eye: (-3.2, 1.35),
        eye_h: 1.58,
        anchor: (1.55, -0.45, 1.92),
        frame_x: 0.41,
        frame_y: 0.53,
        lens: 0.94,
        hero: "the scrying glass in brass rings beneath the star slit",
    },
];

pub fn hero(index: u8) -> &'static str {
    CHAMBERS[(index as usize).min(7)].hero
}

pub fn render_interior_frame(
    building: super::super::Building,
    view: &super::super::raycast::RayView,
    dot_w: u32,
    dot_h: u32,
    bucket: u64,
) -> image::RgbaImage {
    let index = super::super::cinematics::building_index(building);
    raster::render_scene_lit(
        &super::scene::interior_scene(index),
        &staged_view(index, view, dot_w as usize, dot_h as usize),
        dot_w as usize,
        dot_h as usize,
        firelight(index, bucket),
    )
}

pub const BREATH_STEPS: u64 = 4;

pub const BUCKETS_PER_STEP: u64 = 8;

const BREATH: [(f32, f32); BREATH_STEPS as usize] = [
    (1.000, 0.00),
    (0.972, 0.05),
    (0.988, 0.02),
    (0.958, 0.08),
];

const BREATH_PHASE: [u64; 8] = [0, 2, 3, 1, 0, 3, 1, 2];

pub fn firelight(index: u8, bucket: u64) -> raster::Firelight {
    let room = (index as usize).min(BREATH_PHASE.len() - 1);
    let step = (bucket / BUCKETS_PER_STEP).wrapping_add(BREATH_PHASE[room]) % BREATH_STEPS;
    let (gain, warm) = BREATH[step as usize];
    raster::Firelight { gain, warm }
}

fn staged_view(
    index: u8,
    view: &super::super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> View3 {
    let chamber = &CHAMBERS[(index as usize).min(CHAMBERS.len() - 1)];

    let aspect = dot_h.max(1) as f32 / dot_w.max(1) as f32;
    let mut tan_h = (view.fov_rad.clamp(0.70, 1.40) * 0.5).tan();
    let tan_v_max = (VERT_FOV_MAX * 0.5).tan();
    if tan_h * aspect > tan_v_max {
        tan_h = tan_v_max / aspect.max(1e-3);
    }
    tan_h *= chamber.lens.clamp(0.35, 1.0);
    let tan_v = tan_h * aspect;

    let raw = wrap_pi(view.heading_rad - BASE_HEADING);
    let swing = SWING_LIMIT * (raw / SWING_LIMIT).tanh() * SWING_GAIN;

    let local = (
        chamber.eye.0,
        chamber.eye.1 + (swing * 0.5).clamp(-1.0, 1.0) * PARALLAX,
    );
    let eye = rotate(local, chamber.yaw);
    let anchor = rotate((chamber.anchor.0, chamber.anchor.1), chamber.yaw);

    let bearing = (anchor.1 - eye.1).atan2(anchor.0 - eye.0);
    let heading = bearing - (tan_h * (2.0 * chamber.frame_x - 1.0)).atan() + swing;

    let reach = ((anchor.0 - eye.0).powi(2) + (anchor.1 - eye.1).powi(2))
        .sqrt()
        .max(1e-3);
    let elevation = ((chamber.anchor.2 - chamber.eye_h) / reach).atan();
    let pitch = elevation - (tan_v * (1.0 - 2.0 * chamber.frame_y)).atan();
    let pitch = (pitch + view.bob * 0.03).clamp(-1.10, 1.10);

    View3 {
        pos: v3(eye.0, eye.1, chamber.eye_h),
        heading_rad: heading,
        pitch,
        fov_rad: 2.0 * tan_h.atan(),
    }
}

fn rotate(point: (f32, f32), yaw: f32) -> (f32, f32) {
    let (sin, cos) = yaw.sin_cos();
    (point.0 * cos - point.1 * sin, point.0 * sin + point.1 * cos)
}

fn wrap_pi(angle: f32) -> f32 {
    let wrapped = (angle + PI).rem_euclid(TAU);
    wrapped - PI
}

pub fn interior_mesh(index: u8) -> Mesh {
    let index = index.min(7);
    let chamber = &CHAMBERS[index as usize];
    let seed = sub_seed(SCENE_SEED, 0x494E_5445_0000_0000 | index as u64);
    let mut mesh = if index == 3 {
        arch::library_interior(seed)
    } else {
        chamber_mesh(chamber, seed, index)
    };
    if chamber.yaw != 0.0 {
        mesh = mesh.rotated_z(chamber.yaw);
    }
    mesh
}

fn chamber_mesh(chamber: &Chamber, seed: u64, index: u8) -> Mesh {
    let mut rng = Rng::new(sub_seed(seed, 0x484F_4C4C));
    let mut mesh = Mesh::new();
    let (hx, hy) = (chamber.hx, chamber.hy);

    prim::quad(
        &mut mesh,
        v3(-hx, -hy, 0.0),
        v3(hx, -hy, 0.0),
        v3(hx, hy, 0.0),
        v3(-hx, hy, 0.0),
        mat::FLOOR,
    );

    for &sy in &[-1.0_f32, 1.0] {
        prim::quad(
            &mut mesh,
            v3(-hx, sy * hy, 0.0),
            v3(hx, sy * hy, 0.0),
            v3(hx, sy * hy, chamber.z_wall),
            v3(-hx, sy * hy, chamber.z_wall),
            chamber.shell,
        );
    }
    punched_wall(
        &mut mesh,
        v3(-hx, -hy, 0.0),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        hy * 2.0,
        chamber.z_wall,
        (hy - 0.75, hy + 0.75, 0.0, 2.15),
        chamber.shell,
    );
    prim::rect_panel(
        &mut mesh,
        v3(-hx + 0.02, -0.72, 0.0),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        1.44,
        2.10,
        mat::DOOR,
    );

    const PLINTH: f32 = 0.22;
    const PLINTH_Z: f32 = 0.42;
    for &sy in &[-1.0_f32, 1.0] {
        let (y0, y1) = (
            sy * hy - sy.max(0.0) * PLINTH,
            sy * hy + (-sy).max(0.0) * PLINTH,
        );
        prim::boxed(
            &mut mesh,
            v3(-hx, y0.min(y1), 0.0),
            v3(hx, y0.max(y1), PLINTH_Z),
            mat::STONE_DARK,
            F_PZ | if sy < 0.0 { F_PY } else { F_NY },
        );
    }
    prim::boxed(
        &mut mesh,
        v3(-hx, -hy + PLINTH, 0.0),
        v3(-hx + PLINTH, hy - PLINTH, PLINTH_Z),
        mat::STONE_DARK,
        F_PZ | F_PX,
    );

    for &sy in &[-1.0_f32, 1.0] {
        prim::quad(
            &mut mesh,
            v3(-hx, sy * hy, chamber.z_wall),
            v3(hx, sy * hy, chamber.z_wall),
            v3(hx, 0.0, chamber.z_ridge),
            v3(-hx, 0.0, chamber.z_ridge),
            mat::WOOD,
        );
    }
    prim::gable_end(
        &mut mesh,
        -hx,
        0.0,
        hy,
        chamber.z_wall,
        chamber.z_ridge,
        chamber.shell,
    );
    let beams = 4;
    for i in 0..beams {
        let x = -hx + (i as f32 + 0.75) * (hx * 2.0 / beams as f32);
        prim::boxed(
            &mut mesh,
            v3(x - 0.14, -hy, chamber.z_wall - 0.18),
            v3(x + 0.14, hy, chamber.z_wall + 0.16),
            mat::WOOD,
            F_NZ | F_NX | F_PX,
        );
    }

    for &sy in &[-1.0_f32, 1.0] {
        let y = sy * (hy - 0.02);
        let right = v3(-sy, 0.0, 0.0);
        for &x in chamber.bays {
            prim::arched_panel(
                &mut mesh,
                v3(x + sy * 0.45, y, chamber.bay_sill),
                right,
                V3::UP,
                0.90,
                1.20,
                5,
                mat::WINDOW,
            );
        }
    }
    for &x in chamber.shafts {
        light_pool(&mut mesh, x, hy, chamber.bay_sill);
    }

    match index {
        0 => dress_keep(&mut mesh, chamber, &mut rng),
        1 => dress_gatehouse(&mut mesh, chamber),
        2 => dress_rookery(&mut mesh, chamber, &mut rng),
        4 => dress_smithy(&mut mesh, chamber),
        5 => dress_chapel(&mut mesh, chamber),
        6 => dress_round_table(&mut mesh, chamber),
        7 => dress_observatory(&mut mesh, chamber),
        _ => end_wall(&mut mesh, chamber),
    }
    mesh
}

fn end_wall(mesh: &mut Mesh, chamber: &Chamber) {
    prim::quad(
        mesh,
        v3(chamber.hx, -chamber.hy, 0.0),
        v3(chamber.hx, chamber.hy, 0.0),
        v3(chamber.hx, chamber.hy, chamber.z_wall),
        v3(chamber.hx, -chamber.hy, chamber.z_wall),
        chamber.shell,
    );
    prim::gable_end(
        mesh,
        chamber.hx,
        0.0,
        chamber.hy,
        chamber.z_wall,
        chamber.z_ridge,
        chamber.shell,
    );
}

fn dress_keep(mesh: &mut Mesh, chamber: &Chamber, rng: &mut Rng) {
    end_wall(mesh, chamber);
    let x = chamber.hx;

    prim::boxed(
        mesh,
        v3(x - 0.95, -1.85, 0.0),
        v3(x, 1.85, 2.60),
        mat::STONE_DARK,
        F_NX | F_NY | F_PY | F_PZ,
    );
    prim::battered(
        mesh,
        x - 0.48,
        0.0,
        2.60,
        chamber.z_wall + 0.30,
        0.48,
        1.85,
        0.30,
        0.75,
        mat::STONE_DARK,
    );

    prim::arched_panel(
        mesh,
        v3(x - 0.97, -1.15, 0.10),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        2.30,
        0.95,
        5,
        mat::FIRE,
    );
    prim::quad(
        mesh,
        v3(x - 2.60, -1.45, 0.03),
        v3(x - 0.97, -1.45, 0.03),
        v3(x - 0.97, 1.45, 0.03),
        v3(x - 2.60, 1.45, 0.03),
        mat::PATH,
    );

    for &sy in &[-1.0_f32, 1.0] {
        prim::rect_panel(
            mesh,
            v3(x - 0.04, sy * 3.35 - 0.55, 1.55),
            v3(0.0, 1.0, 0.0),
            V3::UP,
            1.10,
            2.30,
            mat::BANNER,
        );
    }

    if let Some(skull) = props::placed("dragon_skull", v3(x - 2.0, -2.75, 0.0), -FRAC_PI_2 + 0.45, 1.25) {
        mesh.merge(skull);
    }

    let top = 0.86;
    prim::boxed(
        mesh,
        v3(-1.25, -0.72, top - 0.12),
        v3(3.30, 0.72, top),
        mat::WOOD,
        F_ALL,
    );
    for &tx in &[-0.45_f32, 2.50] {
        prim::boxed(
            mesh,
            v3(tx - 0.14, -0.52, 0.0),
            v3(tx + 0.14, 0.52, top - 0.12),
            mat::WOOD,
            F_SIDES,
        );
    }
    for &sy in &[-1.0_f32, 1.0] {
        prim::boxed(
            mesh,
            v3(-1.05, sy * 1.30 - 0.22, 0.0),
            v3(3.10, sy * 1.30 + 0.22, 0.46),
            mat::WOOD,
            F_SIDES_TOP,
        );
    }
    for i in 0..3 {
        let cx = -0.20 + i as f32 * 1.55 + rng.jitter(0.10);
        prim::boxed(
            mesh,
            v3(cx - 0.07, -0.07, top),
            v3(cx + 0.07, 0.07, top + 0.34),
            mat::WOOD,
            F_SIDES,
        );
        prim::rect_panel(
            mesh,
            v3(cx - 0.11, 0.0, top + 0.34),
            v3(1.0, 0.0, 0.0),
            V3::UP,
            0.22,
            0.30,
            mat::FIRE,
        );
    }
}

fn dress_gatehouse(mesh: &mut Mesh, chamber: &Chamber) {
    let x = chamber.hx;
    let (oy, oz) = (1.95_f32, 3.20_f32);

    if let Some(guard) =
        props::placed("knight_statue", v3(x - 1.0, -chamber.hy + 0.55, 0.0), PI - 0.6, 1.0)
    {
        mesh.merge(guard);
    }

    punched_wall(
        mesh,
        v3(x, -chamber.hy, 0.0),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        chamber.hy * 2.0,
        chamber.z_wall,
        (chamber.hy - oy, chamber.hy + oy, 0.0, oz),
        chamber.shell,
    );
    for &sy in &[-1.0_f32, 1.0] {
        prim::tri(
            mesh,
            [
                v3(x, sy * oy, oz - 0.85),
                v3(x, sy * oy, oz),
                v3(x, sy * (oy - 0.85), oz),
            ],
            [[0.0, 0.0], [0.85, 0.0], [0.85, 0.85]],
            chamber.shell,
        );
    }
    prim::gable_end(
        mesh,
        x,
        0.0,
        chamber.hy,
        chamber.z_wall,
        chamber.z_ridge,
        chamber.shell,
    );
    prim::quad(
        mesh,
        v3(x, -14.0, 0.0),
        v3(x + 22.0, -14.0, 0.0),
        v3(x + 22.0, 14.0, 0.0),
        v3(x, 14.0, 0.0),
        mat::PATH,
    );

    let bar_x = x - 0.42;
    for i in 0..7 {
        let by = -oy + 0.30 + i as f32 * (2.0 * (oy - 0.30) / 6.0);
        prim::boxed(
            mesh,
            v3(bar_x - 0.09, by - 0.09, 0.30),
            v3(bar_x + 0.09, by + 0.09, oz - 0.10),
            mat::STONE_DARK,
            F_SIDES,
        );
    }
    for &bz in &[0.95_f32, 1.95] {
        prim::boxed(
            mesh,
            v3(bar_x - 0.07, -oy + 0.20, bz - 0.08),
            v3(bar_x + 0.07, oy - 0.20, bz + 0.08),
            mat::STONE_DARK,
            F_SIDES,
        );
    }

    for &sy in &[-1.0_f32, 1.0] {
        let (bx, by) = (x - 2.30, sy * (chamber.hy - 0.85));
        prim::prism(mesh, bx, by, 0.0, 0.86, 0.16, 0.13, 6, 0.0, mat::STONE_DARK);
        prim::prism(
            mesh,
            bx,
            by,
            0.86,
            1.24,
            0.20,
            0.44,
            8,
            0.0,
            mat::STONE_DARK,
        );
        prim::disc(mesh, bx, by, 1.25, 0.40, 8, 0.0, mat::EMBER);
        prim::cone(mesh, bx, by, 1.25, 1.86, 0.34, 4, 0.0, mat::FLAME);
    }
}

fn dress_rookery(mesh: &mut Mesh, chamber: &Chamber, rng: &mut Rng) {
    let (hx, hy) = (chamber.hx, chamber.hy);
    let half = 2.05_f32;

    punched_wall(
        mesh,
        v3(hx, -hy, 0.0),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        hy * 2.0,
        chamber.z_wall,
        (hy - half, hy + half, 1.22, chamber.z_wall),
        chamber.shell,
    );
    slit_gable(
        mesh,
        hx,
        hy,
        half,
        chamber.z_wall,
        chamber.z_ridge,
        chamber.shell,
    );
    prim::quad(
        mesh,
        v3(hx - 0.02, -half, 1.22),
        v3(hx - 1.15, -half - 0.46, 1.22),
        v3(hx - 1.15, -half - 0.46, 3.20),
        v3(hx - 0.02, -half, 3.20),
        mat::WOOD,
    );
    prim::boxed(
        mesh,
        v3(hx - 0.62, -half, 1.04),
        v3(hx, half, 1.22),
        mat::WOOD,
        F_NX | F_PZ,
    );
    prim::quad(
        mesh,
        v3(hx - 2.60, -1.30, 0.04),
        v3(hx - 0.10, -1.45, 0.04),
        v3(hx - 0.10, 1.45, 0.04),
        v3(hx - 2.60, 1.30, 0.04),
        mat::PATH,
    );

    let roosts: [(f32, f32, usize); 4] = [
        (1.10, 1.50, 1),
        (2.60, 1.64, 1),
        (3.80, 1.76, 2),
        (hx - 0.36, 1.52, 2),
    ];
    for (row, &(px, z, birds)) in roosts.iter().enumerate() {
        prim::boxed(
            mesh,
            v3(px - 0.12, -hy + 0.25, z),
            v3(px + 0.12, hy - 0.25, z + 0.16),
            mat::WOOD,
            F_SIDES_TOP,
        );
        if row < 3 {
            for &sy in &[-1.0_f32, 1.0] {
                prim::boxed(
                    mesh,
                    v3(px - 0.10, sy * (hy - 0.42), 0.0),
                    v3(px + 0.10, sy * (hy - 0.22), z),
                    mat::WOOD,
                    F_SIDES,
                );
            }
        }
        for slot in 0..birds {
            let span = 3.70_f32;
            let by = if birds <= 1 {
                rng.range(-1.35, 1.35)
            } else {
                -span * 0.5 + slot as f32 * span / (birds as f32 - 1.0) + rng.jitter(0.30)
            };
            prim::bird_profile(
                mesh,
                v3(px, by, z + 0.16),
                v3(0.0, 1.0, 0.0),
                0.78,
                mat::STONE_DARK,
            );
        }
    }

    for &rx in &[-3.60_f32, -1.70] {
        for &sy in &[-1.0_f32, 1.0] {
            prim::boxed(
                mesh,
                v3(rx - 0.11, sy * (hy - 0.05), chamber.z_wall - 0.10),
                v3(rx + 0.11, sy * 0.10, chamber.z_ridge - 0.30),
                mat::WOOD,
                F_NX | F_PX | F_NZ,
            );
        }
    }
    let (lx, ly) = (3.20_f32, 1.20_f32);
    prim::boxed(
        mesh,
        v3(lx - 0.04, ly - 0.04, 2.82),
        v3(lx + 0.04, ly + 0.04, chamber.z_wall),
        mat::WOOD,
        F_SIDES,
    );
    prim::boxed(
        mesh,
        v3(lx - 0.19, ly - 0.19, 2.42),
        v3(lx + 0.19, ly + 0.19, 2.76),
        mat::FIRE,
        F_SIDES,
    );
    prim::boxed(
        mesh,
        v3(lx - 0.25, ly - 0.25, 2.76),
        v3(lx + 0.25, ly + 0.25, 2.86),
        mat::WOOD,
        F_SIDES_TOP,
    );
    prim::prism(
        mesh,
        2.40,
        -1.35,
        0.0,
        0.72,
        0.17,
        0.14,
        6,
        0.0,
        mat::STONE_DARK,
    );
    prim::prism(
        mesh,
        2.40,
        -1.35,
        0.72,
        1.06,
        0.20,
        0.42,
        8,
        0.0,
        mat::STONE_DARK,
    );
    prim::disc(mesh, 2.40, -1.35, 1.07, 0.38, 8, 0.0, mat::EMBER);
    prim::cone(mesh, 2.40, -1.35, 1.07, 1.58, 0.32, 6, 0.0, mat::FLAME);

    let lad_x = -hx + 0.75;
    for &sy in &[-1.0_f32, 1.0] {
        prim::boxed(
            mesh,
            v3(lad_x - 0.08, 1.95 + sy * 0.42 - 0.08, 0.0),
            v3(lad_x + 0.08, 1.95 + sy * 0.42 + 0.08, 2.55),
            mat::WOOD,
            F_SIDES,
        );
    }
    for rung in 0..5 {
        let z = 0.42 + rung as f32 * 0.48;
        prim::boxed(
            mesh,
            v3(lad_x - 0.05, 1.45, z - 0.05),
            v3(lad_x + 0.05, 2.45, z + 0.05),
            mat::WOOD,
            F_SIDES,
        );
    }
}

fn dress_smithy(mesh: &mut Mesh, chamber: &Chamber) {
    end_wall(mesh, chamber);
    let x = chamber.hx;

    prim::boxed(
        mesh,
        v3(x - 1.20, -1.90, 0.0),
        v3(x, 0.70, 0.95),
        mat::STONE_DARK,
        F_NX | F_NY | F_PY | F_PZ,
    );
    prim::battered(
        mesh,
        x - 0.60,
        -0.60,
        1.95,
        chamber.z_wall + 0.20,
        0.60,
        1.30,
        0.34,
        0.55,
        mat::STONE_DARK,
    );
    prim::arched_panel(
        mesh,
        v3(x - 1.22, -1.55, 0.95),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        1.90,
        0.55,
        5,
        mat::FIRE,
    );
    prim::quad(
        mesh,
        v3(x - 1.22, -1.55, 0.96),
        v3(x - 0.05, -1.55, 0.96),
        v3(x - 0.05, 0.35, 0.96),
        v3(x - 1.22, 0.35, 0.96),
        mat::FIRE,
    );
    prim::quad(
        mesh,
        v3(x - 3.10, -2.10, 0.03),
        v3(x - 1.20, -2.10, 0.03),
        v3(x - 1.20, 0.95, 0.03),
        v3(x - 3.10, 0.95, 0.03),
        mat::PATH,
    );

    let (ax, ay) = (1.30_f32, -0.85_f32);
    prim::prism(mesh, ax, ay, 0.0, 0.62, 0.42, 0.38, 6, 0.0, mat::WOOD);
    prim::boxed(
        mesh,
        v3(ax - 0.52, ay - 0.22, 0.62),
        v3(ax + 0.44, ay + 0.22, 0.86),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    prim::boxed(
        mesh,
        v3(ax + 0.44, ay - 0.13, 0.70),
        v3(ax + 0.92, ay + 0.13, 0.84),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );

    prim::prism(mesh, 2.35, 1.85, 0.0, 0.78, 0.42, 0.40, 8, 0.0, mat::WOOD);
    prim::disc(mesh, 2.35, 1.85, 0.79, 0.38, 8, 0.0, mat::WATER);
    prim::boxed(
        mesh,
        v3(-1.40, chamber.hy - 0.24, 1.55),
        v3(1.40, chamber.hy - 0.06, 1.75),
        mat::WOOD,
        F_SIDES | F_NZ,
    );
    for i in 0..4 {
        let tx = -1.05 + i as f32 * 0.70;
        prim::rect_panel(
            mesh,
            v3(tx - 0.07, chamber.hy - 0.26, 0.85),
            v3(1.0, 0.0, 0.0),
            V3::UP,
            0.14,
            0.70,
            mat::STONE_DARK,
        );
    }
}

fn dress_chapel(mesh: &mut Mesh, chamber: &Chamber) {
    end_wall(mesh, chamber);
    let x = chamber.hx;

    prim::poly_fan(
        mesh,
        v3(x - 0.02, 0.0, 3.25),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        1.42,
        10,
        0.0,
        mat::STONE_DARK,
    );
    prim::poly_fan(
        mesh,
        v3(x - 0.06, 0.0, 3.25),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        1.08,
        10,
        0.0,
        mat::WINDOW,
    );
    for &sy in &[-1.0_f32, 1.0] {
        prim::arched_panel(
            mesh,
            v3(x - 0.05, sy * 1.55 - 0.24, 1.15),
            v3(0.0, 1.0, 0.0),
            V3::UP,
            0.48,
            0.95,
            5,
            mat::WINDOW,
        );
    }

    prim::boxed(
        mesh,
        v3(3.15, -1.35, 0.0),
        v3(4.45, 1.35, 0.14),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    prim::boxed(
        mesh,
        v3(3.45, -0.95, 0.14),
        v3(4.20, 0.95, 1.02),
        mat::STONE,
        F_SIDES_TOP,
    );
    for &sy in &[-1.0_f32, 1.0] {
        prim::boxed(
            mesh,
            v3(3.72, sy * 0.72 - 0.06, 1.02),
            v3(3.86, sy * 0.72 + 0.06, 1.52),
            mat::WOOD,
            F_SIDES,
        );
        prim::rect_panel(
            mesh,
            v3(3.68, sy * 0.72, 1.52),
            v3(1.0, 0.0, 0.0),
            V3::UP,
            0.22,
            0.36,
            mat::FIRE,
        );
    }

    for &cx in &[-1.60_f32, 1.10] {
        for &sy in &[-1.0_f32, 1.0] {
            prim::prism(
                mesh,
                cx,
                sy * (chamber.hy - 0.75),
                0.0,
                chamber.z_wall - 0.30,
                0.34,
                0.30,
                6,
                0.0,
                mat::STONE,
            );
        }
    }
    for i in 0..3 {
        let px = -1.70 + i as f32 * 1.30;
        for &sy in &[-1.0_f32, 1.0] {
            prim::boxed(
                mesh,
                v3(px - 0.13, sy * 1.85 - 0.85, 0.34),
                v3(px + 0.13, sy * 1.85 + 0.85, 0.48),
                mat::WOOD,
                F_SIDES_TOP,
            );
            prim::rect_panel(
                mesh,
                v3(px + 0.13, sy * 1.85 - 0.85, 0.48),
                v3(0.0, 1.0, 0.0),
                V3::UP,
                1.70,
                0.56,
                mat::WOOD,
            );
            for &end in &[-0.78_f32, 0.78] {
                prim::boxed(
                    mesh,
                    v3(px - 0.12, sy * 1.85 + end - 0.07, 0.0),
                    v3(px + 0.12, sy * 1.85 + end + 0.07, 0.34),
                    mat::WOOD,
                    F_SIDES,
                );
            }
        }
    }
}

fn dress_round_table(mesh: &mut Mesh, chamber: &Chamber) {
    end_wall(mesh, chamber);

    prim::prism(mesh, 0.0, 0.0, 0.0, 0.72, 1.00, 0.80, 6, 0.0, mat::WOOD);
    prim::prism(mesh, 0.0, 0.0, 0.72, 0.90, 2.55, 2.55, 12, 0.0, mat::WOOD);
    prim::disc(mesh, 0.0, 0.0, 0.90, 2.55, 12, 0.0, mat::WOOD);
    instruments::council_map(mesh);

    for i in 0..8 {
        let angle = TAU * i as f32 / 8.0 + 0.39;
        if i == 4 {
            continue;
        }
        let (sin, cos) = angle.sin_cos();
        let (sx, sy) = (cos * 3.35, sin * 3.35);
        if i == 0
            && let Some(siege) =
                props::placed("siege_perilous", v3(cos * 3.6, sin * 3.6, 0.0), angle - FRAC_PI_2, 1.15)
        {
            mesh.merge(siege);
            continue;
        }
        let seat = {
            let mut chair = Mesh::new();
            prim::boxed(
                &mut chair,
                v3(-0.44, -0.42, 0.40),
                v3(0.38, 0.42, 0.56),
                mat::WOOD,
                F_SIDES_TOP,
            );
            for &sy in &[-1.0_f32, 1.0] {
                prim::boxed(
                    &mut chair,
                    v3(0.22, sy * 0.34 - 0.09, 0.0),
                    v3(0.40, sy * 0.34 + 0.09, 1.62),
                    mat::WOOD,
                    F_SIDES,
                );
            }
            prim::rect_panel(
                &mut chair,
                v3(0.26, -0.30, 0.64),
                v3(0.0, 1.0, 0.0),
                V3::UP,
                0.60,
                0.46,
                mat::WOOD,
            );
            prim::boxed(
                &mut chair,
                v3(0.18, -0.50, 1.62),
                v3(0.44, 0.50, 1.82),
                mat::WOOD,
                F_SIDES_TOP,
            );
            chair
        };
        mesh.merge(seat.rotated_z(angle).translated(v3(sx, sy, 0.0)));
    }

    for i in 0..3 {
        let angle = TAU * (i as f32 + 0.5) / 3.0 + 0.6;
        let (sin, cos) = angle.sin_cos();
        let (cx, cy) = (cos * 1.95, sin * 1.95);
        prim::boxed(
            mesh,
            v3(cx - 0.08, cy - 0.08, 0.90),
            v3(cx + 0.08, cy + 0.08, 1.34),
            mat::WOOD,
            F_SIDES,
        );
        prim::rect_panel(
            mesh,
            v3(cx - 0.12, cy, 1.34),
            v3(1.0, 0.0, 0.0),
            V3::UP,
            0.24,
            0.34,
            mat::FIRE,
        );
    }

    for &(bx, by, yaw) in &[
        (chamber.hx - 0.05, -2.30_f32, 0.0_f32),
        (chamber.hx - 0.05, 1.90, 0.0),
    ] {
        let _ = yaw;
        prim::rect_panel(
            mesh,
            v3(bx, by, 1.70),
            v3(0.0, 1.0, 0.0),
            V3::UP,
            1.05,
            2.20,
            mat::BANNER,
        );
    }
}

fn dress_observatory(mesh: &mut Mesh, chamber: &Chamber) {
    let (x, hy) = (chamber.hx, chamber.hy);
    let slit = 1.40_f32;

    punched_wall(
        mesh,
        v3(x, -hy, 0.0),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        hy * 2.0,
        chamber.z_wall,
        (hy - slit, hy + slit, 1.05, chamber.z_wall),
        chamber.shell,
    );
    slit_gable(
        mesh,
        x,
        hy,
        slit,
        chamber.z_wall,
        chamber.z_ridge,
        chamber.shell,
    );
    prim::quad(
        mesh,
        v3(x - 3.40, -2.05, 0.03),
        v3(x - 0.06, -2.05, 0.03),
        v3(x - 0.06, 2.05, 0.03),
        v3(x - 3.40, 2.05, 0.03),
        mat::PATH,
    );
    prim::quad(
        mesh,
        v3(x - 0.10, -slit - 0.10, 0.05),
        v3(x - 0.10, slit + 0.10, 0.05),
        v3(x - 2.70, slit + 0.10, 0.05),
        v3(x - 2.70, -slit - 0.10, 0.05),
        mat::MOONLIGHT,
    );

    for &sy in &[-1.0_f32, 1.0] {
        prim::quad(
            mesh,
            v3(x - 0.03, sy * slit, 1.05),
            v3(x - 0.03, sy * slit, chamber.z_wall),
            v3(x - 0.50, sy * slit, chamber.z_wall),
            v3(x - 0.50, sy * slit, 1.05),
            mat::STONE_DARK,
        );
    }

    instruments::scrying_globe(mesh, v3(1.55, -0.45, 1.92));

    prim::boxed(
        mesh,
        v3(-1.30, 1.35, 0.0),
        v3(-0.55, 1.95, 0.92),
        mat::WOOD,
        F_SIDES,
    );
    prim::quad(
        mesh,
        v3(-1.42, 1.30, 0.92),
        v3(-0.45, 1.30, 0.92),
        v3(-0.45, 2.05, 1.32),
        v3(-1.42, 2.05, 1.32),
        mat::WOOD,
    );
    prim::rect_panel(
        mesh,
        v3(-1.12, 1.26, 1.34),
        v3(1.0, 0.0, 0.0),
        V3::UP,
        0.34,
        0.46,
        mat::FIRE,
    );
    let (lx, ly) = (2.05_f32, 2.05_f32);
    prim::boxed(
        mesh,
        v3(lx - 0.04, ly - 0.04, 2.54),
        v3(lx + 0.04, ly + 0.04, chamber.z_wall),
        mat::WOOD,
        F_SIDES,
    );
    prim::boxed(
        mesh,
        v3(lx - 0.17, ly - 0.17, 2.18),
        v3(lx + 0.17, ly + 0.17, 2.50),
        mat::FIRE,
        F_SIDES,
    );
    prim::boxed(
        mesh,
        v3(lx - 0.23, ly - 0.23, 2.50),
        v3(lx + 0.23, ly + 0.23, 2.60),
        mat::WOOD,
        F_SIDES_TOP,
    );
    prim::prism(mesh, 3.30, 2.55, 0.0, 0.95, 0.20, 0.16, 6, 0.0, mat::WOOD);
    instruments::armillary(mesh, v3(3.30, 2.55, 1.42));
}

#[allow(clippy::too_many_arguments)]
fn punched_wall(
    mesh: &mut Mesh,
    origin: V3,
    right: V3,
    up: V3,
    w: f32,
    h: f32,
    hole: (f32, f32, f32, f32),
    material: u8,
) {
    let (u0, u1, v0, v1) = (
        hole.0.clamp(0.0, w),
        hole.1.clamp(0.0, w),
        hole.2.clamp(0.0, h),
        hole.3.clamp(0.0, h),
    );
    let at = |u: f32, v: f32| origin + right * u + up * v;
    prim::quad(
        mesh,
        at(0.0, 0.0),
        at(w, 0.0),
        at(w, v0),
        at(0.0, v0),
        material,
    );
    prim::quad(mesh, at(0.0, v1), at(w, v1), at(w, h), at(0.0, h), material);
    prim::quad(
        mesh,
        at(0.0, v0),
        at(u0, v0),
        at(u0, v1),
        at(0.0, v1),
        material,
    );
    prim::quad(mesh, at(u1, v0), at(w, v0), at(w, v1), at(u1, v1), material);
}

fn light_pool(mesh: &mut Mesh, x: f32, hy: f32, sill: f32) {
    let cy = hy - 1.75;
    prim::quad(
        mesh,
        v3(x - 1.05, cy - 1.20, 0.03),
        v3(x + 0.75, cy - 1.20, 0.03),
        v3(x + 0.95, cy + 1.05, 0.03),
        v3(x - 0.85, cy + 1.05, 0.03),
        mat::PATH,
    );
    prim::quad(
        mesh,
        v3(x - 0.62, cy - 0.66, 0.05),
        v3(x + 0.34, cy - 0.66, 0.05),
        v3(x + 0.46, cy + 0.60, 0.05),
        v3(x - 0.50, cy + 0.60, 0.05),
        mat::MOONLIGHT,
    );
    prim::light_shaft(
        mesh,
        x - 0.08,
        (hy - 0.12, sill + 0.85),
        (cy + 0.30, 0.07),
        0.42,
        1.35,
        mat::MOONLIGHT,
    );
}

fn slit_gable(
    mesh: &mut Mesh,
    x: f32,
    hy: f32,
    half: f32,
    z_wall: f32,
    z_ridge: f32,
    material: u8,
) {
    let shoulder = z_wall + (z_ridge - z_wall) * (1.0 - (half / hy.max(1e-3)).clamp(0.0, 1.0));
    for &sy in &[-1.0_f32, 1.0] {
        prim::tri(
            mesh,
            [
                v3(x, sy * hy, z_wall),
                v3(x, sy * half, z_wall),
                v3(x, sy * half, shoulder),
            ],
            [[0.0, 0.0], [hy - half, 0.0], [hy - half, shoulder - z_wall]],
            material,
        );
    }
}

#[cfg(test)]
pub fn staged_eye(
    index: u8,
    view: &super::super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32, f32, f32, f32) {
    let camera = staged_view(index, view, dot_w, dot_h);
    (
        camera.pos.x,
        camera.pos.y,
        camera.pos.z,
        camera.heading_rad,
        camera.pitch,
        camera.fov_rad,
    )
}

#[cfg(test)]
pub fn composition_mix(
    index: u8,
    view: &super::super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    raster::composition_mix(
        &super::scene::interior_scene(index),
        &staged_view(index, view, dot_w, dot_h),
        dot_w,
        dot_h,
    )
}

#[cfg(test)]
pub fn standoff(
    index: u8,
    view: &super::super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> f32 {
    let camera = staged_view(index, view, dot_w, dot_h);
    let scene = super::scene::interior_scene(index);
    centre_reach(&scene, &camera, dot_w, dot_h)
}

#[cfg(test)]
pub fn centre_reach(scene: &Mesh, camera: &View3, dot_w: usize, dot_h: usize) -> f32 {
    let (sin_h, cos_h) = camera.heading_rad.sin_cos();
    let (sin_p, cos_p) = camera.pitch.sin_cos();
    let right = v3(-sin_h, cos_h, 0.0);
    let forward = v3(cos_h * cos_p, sin_h * cos_p, sin_p);
    let up = forward.cross(right).normalize();
    let tan_h = (camera.fov_rad * 0.5).tan();
    let tan_v = tan_h * dot_h.max(1) as f32 / dot_w.max(1) as f32;

    let mut nearest = f32::INFINITY;
    for step_y in -1i32..=1 {
        for step_x in -1i32..=1 {
            let direction = (forward
                + right * (step_x as f32 * 0.12 * tan_h)
                + up * (step_y as f32 * 0.12 * tan_v))
                .normalize();
            for tri in &scene.tris {
                if matches!(tri.mat, mat::GRASS | mat::PATH | mat::WATER | mat::FLOOR) {
                    continue;
                }
                if let Some(hit) = ray_triangle(camera.pos, direction, tri.v)
                    && hit < nearest
                {
                    nearest = hit;
                }
            }
        }
    }
    nearest
}

#[cfg(test)]
fn ray_triangle(origin: V3, direction: V3, v: [V3; 3]) -> Option<f32> {
    let (edge1, edge2) = (v[1] - v[0], v[2] - v[0]);
    let pvec = direction.cross(edge2);
    let determinant = edge1.dot(pvec);
    if determinant.abs() < 1e-8 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let tvec = origin - v[0];
    let u = tvec.dot(pvec) * inverse;
    if !(-1e-6..=1.000_001).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(edge1);
    let w = direction.dot(qvec) * inverse;
    if w < -1e-6 || u + w > 1.000_001 {
        return None;
    }
    let distance = edge2.dot(qvec) * inverse;
    (distance > 1e-4).then_some(distance)
}

#[cfg(test)]
#[path = "../../../tests/world_viz/world3d__interior__tests.rs"]
mod tests;

#[cfg(test)]
pub fn review_camera(index: u8, w: usize, h: usize) -> View3 {
    let view = super::super::raycast::RayView {
        x: 4.5,
        y: 7.25,
        heading_rad: BASE_HEADING,
        look_yaw: 0.0,
        fov_rad: 1.05,
        bob: 0.0,
        eye_h: 0.0,
    };
    staged_view(index, &view, w, h)
}
