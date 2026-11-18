//! Dotmax adventure-region stages, using the shared authored navigation grid.
//!
//! Five places the quest goes when it leaves Castle Town — the Mines, the Dark
//! Forest, the Swamp, the Dragon Keep and the Homecoming road. Each one is a
//! **pure seeded generator** built out of the pieces this library already has
//! (terrain patches, trees, curtain walls, drums, a gatehouse, the whole castle
//! court) plus the handful of props a region needs and nothing else does.
//!
//! # Contract (inherited from `arch`)
//! Pure functions of `(stage, seed, danger, treasures)`: no clock, no ambient
//! randomness, no map iteration. Same arguments ⇒ byte-identical triangles.
//!
//! Mines are roofed galleries with a crystal working and a descending side
//! shaft. They and the Dragon Keep use the interior composition checks;
//! forest, swamp and homecoming use the outdoor vista checks.
//!
//! # Triangle budgets (measured; `region::tests` asserts the 8k ceiling)
//! | stage | tris | footprint (tiles) |
//! |---|---|---|
//! | [`mines`] | ~1.6k | 40 × 14, gallery with a working and lower shaft |
//! | [`dark_forest`] | ~3.0k | 52 × 52, clearing r = 13 |
//! | [`swamp`] | ~2.0k | 52 × 52, water pan r = 16 |
//! | [`dragon_keep`] | ~1.7k | 30 × 18 hall |
//! | [`homecoming`] | ~4.9k | the castle court + lamp-lit gate road |

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::super::math::{V3, v3};
use super::super::mesh::{Mesh, mat};
use super::compose::{COURT_EXTENT, court_scene, court_terrain};
use super::land::{Strip, TerrainOpts, ground_z, terrain_patch_with, tree};
use super::prim::{self, F_ALL, F_NX, F_NZ, F_PX, F_SIDES, F_SIDES_TOP};
use super::rng::{Rng, sub_seed};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Mines,
    DarkForest,
    Swamp,
    DragonKeep,
    Homecoming,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Mines,
        Stage::DarkForest,
        Stage::Swamp,
        Stage::DragonKeep,
        Stage::Homecoming,
    ];

    pub fn index(self) -> usize {
        match self {
            Stage::Mines => 0,
            Stage::DarkForest => 1,
            Stage::Swamp => 2,
            Stage::DragonKeep => 3,
            Stage::Homecoming => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Stage::Mines => "the Mines",
            Stage::DarkForest => "the Dark Forest",
            Stage::Swamp => "the Swamp",
            Stage::DragonKeep => "the Dragon Keep",
            Stage::Homecoming => "the Homecoming road",
        }
    }
}

pub const MINE_ROOMS: (usize, usize) = match crate::world_viz::region_walk::room_grid(
    crate::world_viz::adventure::Region::TheMines,
) {
    Some(grid) => grid,
    None => panic!("the Mines are the room-locked region; they must have an authored grid"),
};

pub const MINE_STATIONS: usize = MINE_ROOMS.0 * MINE_ROOMS.1;

const MINE_LEN: f32 = 40.0;
const MINE_HALF_W: f32 = 3.4;
const MINE_SPRING: f32 = 2.05;
const MINE_ROOF: f32 = 3.25;
const MINE_CAP: f32 = 2.45;
const MINE_FRAME_PITCH: f32 = 3.2;

const KEEP_HX: f32 = 15.0;
const KEEP_HY: f32 = 6.5;
const KEEP_WALL: f32 = 5.4;
const KEEP_RIDGE: f32 = 8.2;

const CLEARING_R: f32 = 10.5;
const FOREST_FRAME_R: f32 = 7.6;
const SWAMP_CLEAR_R: f32 = 6.6;

pub const WISP_PHASES: u8 = 4;

pub fn stage_extent(stage: Stage) -> f32 {
    match stage {
        Stage::DarkForest | Stage::Swamp => 26.0,
        Stage::Mines => MINE_LEN * 0.5 + 1.0,
        Stage::DragonKeep => KEEP_HX + 1.0,
        Stage::Homecoming => COURT_EXTENT,
    }
}

pub fn stage_terrain(stage: Stage) -> TerrainOpts {
    match stage {
        Stage::Mines | Stage::DragonKeep => TerrainOpts {
            cell: 4.0,
            relief: 0.0,
            path: None,
            river: None,
            water_depth: 0.0,
        },
        Stage::DarkForest => TerrainOpts {
            cell: 2.9,
            relief: 0.34,
            path: None,
            river: None,
            water_depth: 0.0,
        },
        Stage::Swamp => TerrainOpts {
            cell: 2.9,
            relief: 0.12,
            path: None,
            river: Some(Strip::along_x(0.0, 16.0)),
            water_depth: 0.10,
        },
        Stage::Homecoming => court_terrain(),
    }
}

pub fn stage_ground_z(stage: Stage, seed: u64, x: f32, y: f32) -> f32 {
    ground_z(seed, x, y, &stage_terrain(stage))
}

pub fn region_stage(stage: Stage, seed: u64, danger: u8, treasures: u8, phase: u8) -> Mesh {
    let danger = danger.min(3);
    let chests = treasures.min(MAX_CHESTS as u8) as usize;
    let phase = phase % WISP_PHASES;
    match stage {
        Stage::Mines => mines(seed, danger, chests),
        Stage::DarkForest => dark_forest(seed, danger, chests),
        Stage::Swamp => swamp(seed, danger, chests, phase),
        Stage::DragonKeep => dragon_keep(seed, danger, chests),
        Stage::Homecoming => homecoming(seed, danger, chests),
    }
}

pub fn stage_animates(stage: Stage) -> bool {
    stage == Stage::Swamp
}

pub const MAX_CHESTS: usize = 3;

fn chest_spots(stage: Stage) -> [(f32, f32); MAX_CHESTS] {
    match stage {
        Stage::Mines => [(-9.5, 2.1), (2.0, -2.2), (13.0, 2.0)],
        Stage::DarkForest => [(10.0, 5.0), (-9.0, 8.0), (4.0, 11.0)],
        Stage::Swamp => [(9.0, 6.0), (-8.0, 9.0), (6.0, -9.5)],
        Stage::DragonKeep => [(-2.5, -3.4), (3.5, 3.3), (8.0, -3.2)],
        Stage::Homecoming => [(2.4, -13.2), (-2.4, -16.6), (2.4, -19.8)],
    }
}

fn place(out: &mut Mesh, stage: Stage, seed: u64, part: Mesh, x: f32, y: f32, yaw: f32) {
    let z = stage_ground_z(stage, seed, x, y) - 0.12;
    out.merge(part.rotated_z(yaw).translated(v3(x, y, z)));
}

fn chest() -> Mesh {
    let mut m = Mesh::new();
    prim::boxed(
        &mut m,
        v3(-0.55, -0.40, 0.0),
        v3(0.55, 0.40, 0.46),
        mat::WOOD,
        F_SIDES,
    );
    prim::rect_panel(
        &mut m,
        v3(-0.55, 0.34, 0.50),
        v3(1.0, 0.0, 0.0),
        v3(0.0, 0.42, 0.60).normalize(),
        1.10,
        0.46,
        mat::WOOD,
    );
    prim::rect_panel(
        &mut m,
        v3(-0.48, -0.34, 0.44),
        v3(1.0, 0.0, 0.0),
        v3(0.0, 1.0, 0.0),
        0.96,
        0.68,
        mat::EMBER,
    );
    prim::rect_panel(
        &mut m,
        v3(-0.48, -0.40, 0.10),
        v3(1.0, 0.0, 0.0),
        v3(0.0, 0.0, 1.0),
        0.96,
        0.30,
        mat::EMBER,
    );
    m
}

fn torch(_hot: bool) -> Mesh {
    let mut m = Mesh::new();
    prim::boxed(
        &mut m,
        v3(-0.07, -0.26, -0.34),
        v3(0.07, 0.0, 0.06),
        mat::WOOD,
        F_SIDES,
    );
    prim::cone(&mut m, 0.0, -0.20, 0.02, 0.52, 0.19, 5, 0.0, mat::FLAME);
    m
}

fn lamp_post() -> Mesh {
    let mut m = Mesh::new();
    prim::prism(&mut m, 0.0, 0.0, 0.0, 2.45, 0.11, 0.08, 5, 0.0, mat::WOOD);
    prim::boxed(
        &mut m,
        v3(-0.22, -0.22, 2.45),
        v3(0.22, 0.22, 2.92),
        mat::WINDOW,
        F_ALL,
    );
    prim::cone(&mut m, 0.0, 0.0, 2.92, 3.18, 0.30, 5, 0.0, mat::ROOF);
    m
}

fn dead_tree(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x44_45_41_44));
    let mut m = Mesh::new();
    let h = r.range(3.0, 5.0);
    let lean = r.range(0.0, TAU);
    let tilt = r.range(0.10, 0.34);
    let (tx, ty) = (tilt * lean.cos(), tilt * lean.sin());
    prim::prism(
        &mut m,
        0.0,
        0.0,
        0.0,
        h * 0.5,
        0.30,
        0.21,
        5,
        lean,
        mat::TRUNK,
    );
    prim::prism(
        &mut m,
        tx * h * 0.5,
        ty * h * 0.5,
        h * 0.5,
        h,
        0.25,
        0.10,
        5,
        lean,
        mat::TRUNK,
    );
    for i in 0..3 {
        let a = lean + TAU * (i as f32) / 3.0 + r.jitter(0.4);
        let z = h * (0.52 + 0.13 * i as f32);
        let reach = r.range(0.7, 1.15);
        prim::prism(
            &mut m,
            tx * z + reach * 0.5 * a.cos(),
            ty * z + reach * 0.5 * a.sin(),
            z,
            z + r.range(0.25, 0.6),
            0.11,
            0.05,
            4,
            a,
            mat::TRUNK,
        );
    }
    m
}

fn wisp(r: f32) -> Mesh {
    let mut m = Mesh::new();
    prim::cone(&mut m, 0.0, 0.0, 0.0, r * 1.5, r, 5, 0.0, mat::MOONLIGHT);
    prim::cone(&mut m, 0.0, 0.0, 0.0, -r * 1.5, r, 5, 0.0, mat::MOONLIGHT);
    m
}

fn path_span(m: &mut Mesh, stage: Stage, seed: u64, a: (f32, f32), b: (f32, f32), half: f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-3 {
        return;
    }
    let (nx, ny) = (-dy / len * half, dx / len * half);
    let lift = 0.035;
    let z = |x: f32, y: f32| stage_ground_z(stage, seed, x, y) + lift;
    let p = [
        (a.0 - nx, a.1 - ny),
        (a.0 + nx, a.1 + ny),
        (b.0 + nx, b.1 + ny),
        (b.0 - nx, b.1 - ny),
    ];
    prim::quad(
        m,
        v3(p[0].0, p[0].1, z(p[0].0, p[0].1)),
        v3(p[1].0, p[1].1, z(p[1].0, p[1].1)),
        v3(p[2].0, p[2].1, z(p[2].0, p[2].1)),
        v3(p[3].0, p[3].1, z(p[3].0, p[3].1)),
        mat::PATH,
    );
}

fn stand_chests(m: &mut Mesh, stage: Stage, seed: u64, n: usize) {
    let spots = chest_spots(stage);
    for (i, &(x, y)) in spots.iter().take(n.min(MAX_CHESTS)).enumerate() {
        place(
            m,
            stage,
            seed,
            chest(),
            x,
            y,
            FRAC_PI_2 * 0.5 + i as f32 * 0.7,
        );
    }
}

fn mines(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::Mines;
    let mut r = Rng::new(sub_seed(seed, 0x4D_49_4E_45));
    let mut m = Mesh::new();
    let half = MINE_LEN * 0.5;

    prim::quad(
        &mut m,
        v3(-half, -MINE_HALF_W - 0.4, 0.0),
        v3(half, -MINE_HALF_W - 0.4, 0.0),
        v3(half, MINE_HALF_W + 0.4, 0.0),
        v3(-half, MINE_HALF_W + 0.4, 0.0),
        mat::FLOOR,
    );

    let segs = (MINE_LEN / 2.0) as usize;
    fn wall(m: &mut Mesh, x0: f32, x1: f32, y0: f32, y1: f32, r: &mut Rng) -> (f32, f32, f32) {
        let z0 = MINE_SPRING + r.jitter(0.12);
        let z1 = MINE_SPRING + r.jitter(0.12);
        let roof = MINE_ROOF + r.jitter(0.10);
        let opening = y0 < 0.0 && ((x0 >= -12.0 && x1 <= -8.0) || (x0 >= 2.0 && x1 <= 8.0));
        if !opening {
            prim::quad(
                m,
                v3(x0, y0, 0.0),
                v3(x1, y1, 0.0),
                v3(x1, y1, z1),
                v3(x0, y0, z0),
                mat::ROCK,
            );
        }
        prim::quad(
            m,
            v3(x0, y0, z0),
            v3(x1, y1, z1),
            v3(x1, y1 * 0.42, roof),
            v3(x0, y0 * 0.42, roof),
            mat::ROCK,
        );
        (y0 * 0.42, y1 * 0.42, roof)
    }
    for i in 0..segs {
        let x0 = -half + 2.0 * i as f32;
        let x1 = x0 + 2.0;
        let spread0 = MINE_HALF_W + r.jitter(0.30);
        let spread1 = MINE_HALF_W + r.jitter(0.30);
        let (cl0, cl1, roof_l) = wall(&mut m, x0, x1, -spread0, -spread1, &mut r);
        let (cr0, cr1, roof_r) = wall(&mut m, x0, x1, spread0, spread1, &mut r);
        prim::quad(
            &mut m,
            v3(x0, cl0, roof_l),
            v3(x1, cl1, roof_l),
            v3(x1, cr1, roof_r),
            v3(x0, cr0, roof_r),
            mat::ROCK,
        );
    }
    for &x in &[-half, half] {
        prim::quad(
            &mut m,
            v3(x, -MINE_HALF_W - 0.4, 0.0),
            v3(x, MINE_HALF_W + 0.4, 0.0),
            v3(x, MINE_HALF_W + 0.4, MINE_ROOF),
            v3(x, -MINE_HALF_W - 0.4, MINE_ROOF),
            mat::ROCK,
        );
    }

    let frames = (MINE_LEN / MINE_FRAME_PITCH) as usize;
    for i in 0..frames {
        let x = -half + 1.4 + MINE_FRAME_PITCH * i as f32;
        for side in [-1.0_f32, 1.0] {
            let y = side * (MINE_HALF_W - 0.30);
            prim::boxed(
                &mut m,
                v3(x - 0.15, y - 0.15, 0.0),
                v3(x + 0.15, y + 0.15, MINE_CAP),
                mat::WOOD,
                F_SIDES,
            );
        }
        prim::boxed(
            &mut m,
            v3(x - 0.17, -MINE_HALF_W - 0.05, MINE_CAP),
            v3(x + 0.17, MINE_HALF_W + 0.05, MINE_CAP + 0.34),
            mat::WOOD,
            F_SIDES | F_NZ,
        );
    }

    for side in [-0.55_f32, 0.55] {
        prim::boxed(
            &mut m,
            v3(-half, side - 0.06, 0.06),
            v3(half, side + 0.06, 0.20),
            mat::STONE,
            F_SIDES_TOP,
        );
    }
    let mut x = -half + 0.8;
    while x < half {
        prim::quad(
            &mut m,
            v3(x - 0.16, -0.95, 0.03),
            v3(x + 0.16, -0.95, 0.03),
            v3(x + 0.16, 0.95, 0.03),
            v3(x - 0.16, 0.95, 0.03),
            mat::WOOD,
        );
        x += 1.5;
    }
    let cart_x = -half + 12.5;
    prim::boxed(
        &mut m,
        v3(cart_x - 0.85, -0.72, 0.36),
        v3(cart_x + 0.85, 0.72, 1.18),
        mat::WOOD,
        F_SIDES_TOP,
    );
    for &(dx, dy) in &[
        (-0.6_f32, -0.55_f32),
        (0.6, -0.55),
        (-0.6, 0.55),
        (0.6, 0.55),
    ] {
        prim::prism(
            &mut m,
            cart_x + dx,
            dy,
            0.14,
            0.36,
            0.20,
            0.20,
            5,
            FRAC_PI_2,
            mat::ROCK,
        );
    }

    for i in 0..18 {
        let x = -half + 1.5 + (MINE_LEN - 3.0) * (i as f32 + 0.5) / 18.0;
        let side = if i % 2 == 0 { -1.0_f32 } else { 1.0 };
        let y = side * (MINE_HALF_W - 0.06);
        let z = r.range(0.7, MINE_SPRING - 0.25);
        let s = r.range(0.20, 0.42);
        prim::rect_panel(
            &mut m,
            v3(x - s * 0.5, y, z),
            v3(1.0, 0.0, 0.0),
            v3(0.0, 0.0, 1.0),
            s,
            s * 0.55,
            mat::MOONLIGHT,
        );
    }

    let lit = (8usize).saturating_sub(danger as usize * 2).max(2);
    for i in 0..lit {
        let x = -half + 2.4 + (MINE_LEN - 6.0) * (i as f32) / lit.max(1) as f32;
        let side = if i % 2 == 0 { -1.0_f32 } else { 1.0 };
        let y = side * (MINE_HALF_W - 0.34);
        place(
            &mut m,
            stage,
            seed,
            torch(true).translated(v3(0.0, 0.0, 1.52)),
            x,
            y,
            if side < 0.0 { 0.0 } else { PI },
        );
        prim::disc(&mut m, x, y * 0.70, 0.022, 1.65, 8, 0.0, mat::TORCH_POOL);
    }

    for i in 0..14 {
        let x = -half + 1.0 + (MINE_LEN - 2.0) * (i as f32 + 0.3) / 14.0;
        let side = if i % 3 == 0 { -1.0_f32 } else { 1.0 };
        let y = side * (MINE_HALF_W - r.range(0.2, 0.7));
        let s = r.range(0.22, 0.52);
        prim::boxed(
            &mut m,
            v3(x - s, y - s * 0.7, 0.0),
            v3(x + s, y + s * 0.7, s * r.range(0.5, 1.0)),
            mat::ROCK,
            F_SIDES_TOP,
        );
    }

    mine_working(&mut m, -12.0, -8.0, false);
    mine_working(&mut m, 2.0, 8.0, true);
    stand_chests(&mut m, stage, seed, chests);
    m
}

fn mine_working(m: &mut Mesh, x0: f32, x1: f32, descending: bool) {
    let mouth = -MINE_HALF_W;
    let back = if descending { -10.0 } else { -7.8 };
    let bottom = if descending { -2.6 } else { 0.0 };
    for x in [x0, x1] {
        prim::quad(
            m,
            v3(x, mouth, 0.0),
            v3(x, back, bottom),
            v3(x, back, 2.45),
            v3(x, mouth, MINE_SPRING),
            mat::ROCK,
        );
    }
    prim::quad(
        m,
        v3(x0, back, bottom),
        v3(x1, back, bottom),
        v3(x1, back, 2.45),
        v3(x0, back, 2.45),
        mat::ROCK,
    );
    prim::quad(
        m,
        v3(x0, mouth, MINE_SPRING),
        v3(x1, mouth, MINE_SPRING),
        v3(x1, back, 2.45),
        v3(x0, back, 2.45),
        mat::ROCK,
    );
    if descending {
        for step in 0..8 {
            let front = mouth - step as f32 * 0.64;
            let rear = front - 0.64;
            let top = -(step as f32) * 0.325;
            prim::boxed(
                m,
                v3(x0 + 0.24, rear, top - 0.325),
                v3(x1 - 0.24, front, top),
                mat::STONE,
                prim::F_PY | prim::F_PZ,
            );
        }
        prim::quad(
            m,
            v3(x0, back, bottom),
            v3(x1, back, bottom),
            v3(x1, mouth - 5.12, bottom),
            v3(x0, mouth - 5.12, bottom),
            mat::FLOOR,
        );
        prim::boxed(
            m,
            v3(x0 + 0.1, mouth - 0.25, 1.84),
            v3(x1 - 0.1, mouth + 0.12, 2.20),
            mat::WOOD,
            F_SIDES_TOP,
        );
        prim::boxed(
            m,
            v3(x1 - 0.85, mouth - 0.30, -1.25),
            v3(x1 - 0.80, mouth - 0.25, 1.9),
            mat::BRASS,
            F_SIDES,
        );
    } else {
        prim::quad(
            m,
            v3(x0, back, 0.0),
            v3(x1, back, 0.0),
            v3(x1, mouth, 0.0),
            v3(x0, mouth, 0.0),
            mat::FLOOR,
        );
    }
    for i in 0..5 {
        let x = x0 + 0.65 + (x1 - x0 - 1.3) * i as f32 / 4.0;
        let y = back + 0.65 + (i % 2) as f32 * 0.5;
        let height = 0.65 + (i % 3) as f32 * 0.43;
        prim::prism(
            m,
            x,
            y,
            bottom,
            bottom + height * 0.64,
            0.26,
            0.33,
            5,
            i as f32,
            mat::MOONLIGHT,
        );
        prim::cone(
            m,
            x,
            y,
            bottom + height * 0.64,
            bottom + height,
            0.33,
            5,
            i as f32,
            mat::MOONLIGHT,
        );
    }
    for x in [x0 + 0.14, x1 - 0.14] {
        prim::boxed(
            m,
            v3(x - 0.14, mouth - 0.22, 0.0),
            v3(x + 0.14, mouth + 0.15, MINE_CAP),
            mat::WOOD,
            F_SIDES,
        );
    }
    prim::boxed(
        m,
        v3(x0, mouth - 0.22, MINE_CAP),
        v3(x1, mouth + 0.15, MINE_CAP + 0.30),
        mat::WOOD,
        F_SIDES | F_NZ,
    );
}

fn dark_forest(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::DarkForest;
    let mut r = Rng::new(sub_seed(seed, 0x46_4F_52_53));
    let mut m = Mesh::new();
    let extent = stage_extent(stage);
    m.merge(terrain_patch_with(
        extent * 2.0,
        extent * 2.0,
        seed,
        &stage_terrain(stage),
    ));

    let track = |t: f32| -> (f32, f32) { (7.5 * (t * 0.145).sin(), t) };
    let mut t = -extent + 1.0;
    while t < extent - 1.0 {
        let next = (t + 3.0).min(extent - 1.0);
        path_span(&mut m, stage, seed, track(t), track(next), 1.25);
        t = next;
    }

    let mut planted = 0usize;
    let mut attempts = 0usize;
    while planted < 64 && attempts < 1400 {
        attempts += 1;
        let x = r.range(-extent + 1.5, extent - 1.5);
        let y = r.range(-extent + 1.5, extent - 1.5);
        if (x * x + y * y).sqrt() < CLEARING_R {
            continue;
        }
        let (tx, _) = track(y);
        if (x - tx).abs() < 2.6 {
            continue;
        }
        place(
            &mut m,
            stage,
            seed,
            tree(sub_seed(seed, 0x300 + planted as u64)),
            x,
            y,
            r.range(0.0, TAU),
        );
        planted += 1;
    }

    for i in 0..9 {
        let a = TAU * i as f32 / 9.0 + 0.24;
        let d = FOREST_FRAME_R + r.jitter(0.5);
        let giant = tree(sub_seed(seed, 0x340 + i as u64)).scaled(r.range(1.9, 2.4));
        place(
            &mut m,
            stage,
            seed,
            giant,
            d * a.cos(),
            d * a.sin(),
            r.range(0.0, TAU),
        );
    }

    for i in 0..14 {
        let t = if i % 2 == 0 {
            CLEARING_R + 3.0 + 3.0 * (i / 2) as f32
        } else {
            -(CLEARING_R + 3.0 + 3.0 * (i / 2) as f32)
        };
        if t.abs() > extent - 3.0 {
            continue;
        }
        let (tx, ty) = track(t);
        let giant = tree(sub_seed(seed, 0x360 + i as u64)).scaled(r.range(1.7, 2.1));
        place(
            &mut m,
            stage,
            seed,
            giant,
            tx + r.jitter(2.4),
            ty + r.jitter(1.6),
            r.range(0.0, TAU),
        );
    }

    for i in 0..4 {
        let a = TAU * i as f32 / 4.0 + 0.9;
        let d = r.range(4.5, 9.0);
        let (x, y) = (d * a.cos(), d * a.sin());
        let mut stump = Mesh::new();
        prim::prism(
            &mut stump,
            0.0,
            0.0,
            0.0,
            0.62,
            0.52,
            0.44,
            6,
            0.0,
            mat::TRUNK,
        );
        prim::disc(&mut stump, 0.0, 0.0, 0.62, 0.44, 6, 0.0, mat::TRUNK);
        place(&mut m, stage, seed, stump, x, y, 0.0);
    }

    let flies = (9usize).saturating_sub(danger as usize * 2).max(2);
    for i in 0..flies {
        let a = TAU * i as f32 / flies.max(1) as f32 + 0.3;
        let d = r.range(5.0, 9.5);
        let (x, y) = (d * a.cos(), d * a.sin());
        let z = stage_ground_z(stage, seed, x, y) + r.range(1.0, 2.6);
        m.merge(wisp(0.13).translated(v3(x, y, z)));
    }

    stand_chests(&mut m, stage, seed, chests);
    m
}

fn swamp(seed: u64, danger: u8, chests: usize, phase: u8) -> Mesh {
    let stage = Stage::Swamp;
    let mut r = Rng::new(sub_seed(seed, 0x53_57_4D_50));
    let mut m = Mesh::new();
    let extent = stage_extent(stage);
    m.merge(terrain_patch_with(
        extent * 2.0,
        extent * 2.0,
        seed,
        &stage_terrain(stage),
    ));
    let water = |x: f32, y: f32| stage_ground_z(stage, seed, x, y);

    let mut planted = 0usize;
    while planted < 52 {
        let a = r.range(0.0, TAU);
        let t = r.unit() * r.unit();
        let d = SWAMP_CLEAR_R + 0.6 + t * (extent - 3.0 - SWAMP_CLEAR_R);
        place(
            &mut m,
            stage,
            seed,
            dead_tree(sub_seed(seed, 0x400 + planted as u64)),
            d * a.cos(),
            d * a.sin(),
            r.range(0.0, TAU),
        );
        planted += 1;
    }

    for i in 0..22 {
        let a = TAU * i as f32 / 22.0 + 0.41;
        let d = r.range(2.6, 11.0);
        let (x, y) = (d * a.cos(), d * a.sin());
        prim::disc(
            &mut m,
            x,
            y,
            water(x, y) + 0.035,
            r.range(0.35, 0.72),
            6,
            r.range(0.0, TAU),
            mat::GRASS,
        );
    }

    for i in 0..16 {
        let a = TAU * i as f32 / 16.0 + 0.2;
        let d = r.range(SWAMP_CLEAR_R + 0.4, SWAMP_CLEAR_R + 11.0);
        let (x, y) = (d * a.cos(), d * a.sin());
        let mut clump = Mesh::new();
        for k in 0..4 {
            let ka = a + k as f32 * 1.7;
            prim::prism(
                &mut clump,
                0.34 * ka.cos(),
                0.34 * ka.sin(),
                0.0,
                r.range(1.1, 2.0),
                0.10,
                0.02,
                4,
                ka,
                mat::FOLIAGE,
            );
        }
        place(&mut m, stage, seed, clump, x, y, 0.0);
    }

    for i in 0..12 {
        let a = TAU * i as f32 / 12.0 + 0.13;
        let d = 19.0 + r.jitter(1.4);
        let (x, y) = (d * a.cos(), d * a.sin());
        let (sin, cos) = a.sin_cos();
        let along = v3(-sin, cos, 0.0);
        prim::rect_panel(
            &mut m,
            v3(x, y, water(x, y) + 0.30) - along * 3.2,
            along,
            V3::UP,
            6.4,
            r.range(0.42, 0.66),
            mat::MOONLIGHT,
        );
    }

    let lights = (10usize).saturating_sub(danger as usize).max(5);
    let step = TAU * phase as f32 / WISP_PHASES as f32;
    for i in 0..lights {
        let a = TAU * i as f32 / lights.max(1) as f32 + 0.55;
        let d = r.range(SWAMP_CLEAR_R + 2.0, 19.0);
        let orbit = r.range(0.6, 1.5);
        let drift = a + step + r.jitter(0.2);
        let (x, y) = (
            d * a.cos() + orbit * drift.cos(),
            d * a.sin() + orbit * drift.sin(),
        );
        let size = r.range(0.18, 0.30);
        let lift = r.range(0.8, 2.2) + 0.24 * (step + i as f32).sin();
        let surface = water(x, y);
        m.merge(wisp(size).translated(v3(x, y, surface + lift)));
        prim::disc(
            &mut m,
            x,
            y,
            surface + 0.05,
            size * 1.9,
            6,
            drift,
            mat::MOONLIGHT,
        );
    }

    stand_chests(&mut m, stage, seed, chests);
    m
}

fn dragon_keep(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::DragonKeep;
    let mut r = Rng::new(sub_seed(seed, 0x4B_45_45_50));
    let mut m = Mesh::new();
    let (hx, hy) = (KEEP_HX, KEEP_HY);

    prim::quad(
        &mut m,
        v3(-hx, -hy, 0.0),
        v3(hx, -hy, 0.0),
        v3(hx, hy, 0.0),
        v3(-hx, hy, 0.0),
        mat::FLOOR,
    );
    prim::quad(
        &mut m,
        v3(-hx + 0.5, -1.25, 0.02),
        v3(hx - 4.2, -1.25, 0.02),
        v3(hx - 4.2, 1.25, 0.02),
        v3(-hx + 0.5, 1.25, 0.02),
        mat::BANNER,
    );
    for &sy in &[-1.0_f32, 1.0] {
        prim::quad(
            &mut m,
            v3(-hx, sy * hy, 0.0),
            v3(hx, sy * hy, 0.0),
            v3(hx, sy * hy, KEEP_WALL),
            v3(-hx, sy * hy, KEEP_WALL),
            mat::STONE,
        );
        prim::quad(
            &mut m,
            v3(-hx, sy * hy, KEEP_WALL),
            v3(hx, sy * hy, KEEP_WALL),
            v3(hx, 0.0, KEEP_RIDGE),
            v3(-hx, 0.0, KEEP_RIDGE),
            mat::WOOD,
        );
    }
    for &x in &[-hx, hx] {
        prim::quad(
            &mut m,
            v3(x, -hy, 0.0),
            v3(x, hy, 0.0),
            v3(x, hy, KEEP_WALL),
            v3(x, -hy, KEEP_WALL),
            mat::STONE,
        );
        prim::gable_end(&mut m, x, 0.0, hy, KEEP_WALL, KEEP_RIDGE, mat::STONE);
    }
    for i in 0..5 {
        let x = -hx + 2.6 + (2.0 * hx - 5.2) * i as f32 / 4.0;
        prim::boxed(
            &mut m,
            v3(x - 0.16, -hy, KEEP_WALL - 0.22),
            v3(x + 0.16, hy, KEEP_WALL + 0.14),
            mat::WOOD,
            F_NZ | F_NX | F_PX,
        );
    }

    for i in 0..5 {
        let x = -hx + 3.0 + (2.0 * hx - 7.0) * i as f32 / 4.0;
        for side in [-1.0_f32, 1.0] {
            let y = side * (hy - 1.9);
            prim::prism(&mut m, x, y, 0.0, 0.34, 0.78, 0.62, 8, 0.0, mat::STONE_DARK);
            prim::prism(
                &mut m,
                x,
                y,
                0.34,
                KEEP_WALL - 0.55,
                0.56,
                0.48,
                8,
                0.0,
                mat::STONE,
            );
            prim::prism(
                &mut m,
                x,
                y,
                KEEP_WALL - 0.55,
                KEEP_WALL - 0.10,
                0.62,
                0.74,
                8,
                0.0,
                mat::STONE_DARK,
            );
            prim::rect_panel(
                &mut m,
                v3(x - 0.75, side * (hy - 0.04), 1.9),
                v3(1.0, 0.0, 0.0),
                V3::UP,
                1.5,
                2.6,
                mat::BANNER,
            );
        }
    }

    for (step, inset) in [(0usize, 0.0_f32), (1, 1.1)] {
        let z0 = step as f32 * 0.38;
        prim::boxed(
            &mut m,
            v3(hx - 5.6 + inset, -4.2 + inset * 0.5, z0),
            v3(hx - 0.05, 4.2 - inset * 0.5, z0 + 0.38),
            mat::STONE,
            F_SIDES_TOP,
        );
    }
    prim::boxed(
        &mut m,
        v3(hx - 1.9, -0.95, 0.76),
        v3(hx - 0.9, 0.95, 1.34),
        mat::STONE_DARK,
        F_SIDES_TOP,
    );
    prim::rect_panel(
        &mut m,
        v3(hx - 0.92, -0.95, 1.34),
        v3(0.0, 1.0, 0.0),
        V3::UP,
        1.9,
        2.2,
        mat::STONE_DARK,
    );

    let fires = (6usize).saturating_sub(danger as usize).max(3);
    for i in 0..fires {
        let x = -hx + 4.5 + (2.0 * hx - 8.0) * i as f32 / fires.max(1) as f32;
        let side = if i % 2 == 0 { -1.0_f32 } else { 1.0 };
        let mut brazier = Mesh::new();
        prim::prism(
            &mut brazier,
            0.0,
            0.0,
            0.0,
            0.66,
            0.16,
            0.44,
            6,
            0.0,
            mat::STONE_DARK,
        );
        prim::disc(&mut brazier, 0.0, 0.0, 0.70, 0.42, 6, 0.0, mat::EMBER);
        prim::cone(&mut brazier, 0.0, 0.0, 0.70, 1.36, 0.36, 5, 0.0, mat::FLAME);
        place(&mut m, stage, seed, brazier, x, side * 3.5, r.jitter(0.5));
    }
    for i in 0..4 {
        let x = -hx + 3.6 + (2.0 * hx - 8.0) * i as f32 / 3.0;
        let side = if i % 2 == 0 { -1.0_f32 } else { 1.0 };
        place(
            &mut m,
            stage,
            seed,
            torch(false).translated(v3(0.0, 0.0, 2.5)),
            x,
            side * (hy - 0.22),
            if side < 0.0 { 0.0 } else { PI },
        );
    }

    m.merge(
        dragon(sub_seed(seed, 0x44_52_47_4E))
            .rotated_z(PI - 0.20)
            .translated(v3(hx - 3.1, 0.5, 0.76)),
    );

    stand_chests(&mut m, stage, seed, chests);
    m
}

fn dragon(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x57_59_52_4D));
    let mut m = Mesh::new();
    let hide = mat::STONE_DARK;

    prim::prism(&mut m, 1.7, 0.0, 0.55, 1.95, 1.15, 1.05, 6, 0.3, hide);
    prim::prism(&mut m, 0.2, 0.0, 0.45, 2.15, 1.05, 0.92, 6, 0.0, hide);
    prim::prism(&mut m, -1.3, 0.0, 0.50, 1.95, 0.92, 0.72, 6, 0.2, hide);

    prim::prism(&mut m, -2.3, 0.0, 1.70, 3.20, 0.62, 0.44, 5, 0.0, hide);
    prim::prism(&mut m, -2.9, 0.0, 3.10, 4.30, 0.44, 0.33, 5, 0.0, hide);
    prim::cone(&mut m, -3.5, 0.0, 4.10, 4.90, 0.42, 5, 0.0, hide);
    prim::boxed(
        &mut m,
        v3(-4.6, -0.34, 3.70),
        v3(-2.9, 0.34, 4.25),
        hide,
        F_ALL,
    );
    for side in [-1.0_f32, 1.0] {
        prim::rect_panel(
            &mut m,
            v3(-4.30, side * 0.35, 4.02),
            v3(1.0, 0.0, 0.0),
            v3(0.0, 0.0, 1.0),
            0.30,
            0.17,
            mat::EMBER,
        );
        prim::cone(&mut m, -3.1, side * 0.26, 4.24, 5.10, 0.16, 4, 0.0, hide);
    }

    let mut x = 2.7;
    let mut y = 0.0;
    let mut a: f32 = 0.35;
    for i in 0..3 {
        let len = 2.0 - 0.35 * i as f32;
        let (nx, ny) = (x + len * a.cos(), y + len * a.sin());
        prim::prism(
            &mut m,
            (x + nx) * 0.5,
            (y + ny) * 0.5,
            0.35 - 0.06 * i as f32,
            1.10 - 0.28 * i as f32,
            0.55 - 0.15 * i as f32,
            0.40 - 0.12 * i as f32,
            5,
            a,
            hide,
        );
        x = nx;
        y = ny;
        a += r.range(0.45, 0.75);
    }

    for side in [-1.0_f32, 1.0] {
        let root = v3(0.4, side * 0.85, 1.95);
        let tip = v3(-1.2, side * 3.9, 4.65);
        let elbow = v3(0.1, side * 2.2, 4.05);
        prim::prism(
            &mut m,
            (root.x + elbow.x) * 0.5,
            (root.y + elbow.y) * 0.5,
            root.z,
            elbow.z,
            0.20,
            0.13,
            4,
            0.0,
            hide,
        );
        prim::tri_planar(
            &mut m,
            root,
            v3(0.0, side, 0.0),
            V3::UP,
            [root, elbow, tip],
            hide,
        );
        prim::tri_planar(
            &mut m,
            root,
            v3(0.0, side, 0.0),
            V3::UP,
            [root, tip, v3(2.0, side * 1.5, 1.60)],
            hide,
        );
    }

    for &(lx, lz) in &[(1.9_f32, 1.45_f32), (-0.9, 1.35)] {
        for side in [-1.0_f32, 1.0] {
            prim::prism(&mut m, lx, side * 1.0, 0.0, lz, 0.34, 0.26, 4, 0.4, hide);
        }
    }
    m
}

fn homecoming(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::Homecoming;
    let mut m = court_scene(seed);
    let lit = (5 - danger as usize).max(2);
    for i in 0..lit {
        let y = -11.5 - 3.2 * i as f32;
        for side in [-1.0_f32, 1.0] {
            place(&mut m, stage, seed, lamp_post(), side * 3.2, y, 0.0);
        }
    }
    stand_chests(&mut m, stage, seed, chests);
    m
}
