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
//! # Why every stage is open to the sky
//! The world vista law's floors (`docs/plans/world-vista-law.md`, measured by
//! `raster::composition_mix`) want ≥ 20 % sky in every frame, and "sky" is
//! literally "the ray hit nothing". A sealed cave and a roofed great hall
//! score zero sky and can never pass. So the Mines are an **open-cast working**
//! — rock galleries cut down from the surface, roofless — and the Dragon Keep
//! is a **ruined** hall whose roof is long gone. Both read better at braille
//! scale anyway: a night sky behind a rock wall is what makes the rock wall a
//! silhouette instead of a grey field.
//!
//! # Triangle budgets (measured; `region::tests` asserts the 8k ceiling)
//! | stage | tris | footprint (tiles) |
//! |---|---|---|
//! | [`mines`] | ~1.1k | 52 × 52, 3 × 3 chambers on a 15-tile pitch |
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

/// Which region stage to build. The renderer's `Region` maps onto this;
/// `CastleTown` has no entry because it keeps the shipped court path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Stage {
    Mines,
    DarkForest,
    Swamp,
    DragonKeep,
    Homecoming,
}

impl Stage {
    /// Dense index, for fixed-size caches and test tables.
    pub(crate) const ALL: [Stage; 5] = [
        Stage::Mines,
        Stage::DarkForest,
        Stage::Swamp,
        Stage::DragonKeep,
        Stage::Homecoming,
    ];

    pub(crate) fn index(self) -> usize {
        match self {
            Stage::Mines => 0,
            Stage::DarkForest => 1,
            Stage::Swamp => 2,
            Stage::DragonKeep => 3,
            Stage::Homecoming => 4,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Stage::Mines => "the Mines",
            Stage::DarkForest => "the Dark Forest",
            Stage::Swamp => "the Swamp",
            Stage::DragonKeep => "the Dragon Keep",
            Stage::Homecoming => "the Homecoming road",
        }
    }
}

// ── shared shape of a region ──────────────────────────────────────────────

/// Stations of the Mines walk — the authored room grid flattened onto one
/// gallery.
///
/// Rooms become **stations** down the gallery because a mine is a tunnel and a
/// tunnel is walked along, not wandered around; but there is exactly one
/// station per chamber, so the two views always agree about where the hero
/// is. The count is *derived* from the shared navigation grid rather than
/// re-authored here — one loop, as the Z3 report asked for.
pub(crate) const MINE_ROOMS: (usize, usize) =
    match crate::world_viz::region_walk::room_grid(crate::world_viz::adventure::Region::TheMines) {
        Some(grid) => grid,
        None => panic!("the Mines are the room-locked region; they must have an authored grid"),
    };

/// How many chambers — and therefore how many camera stations — the Mines have.
pub(crate) const MINE_STATIONS: usize = MINE_ROOMS.0 * MINE_ROOMS.1;

/// The gallery, in tiles: length along ±x, half-width, the height the walls
/// start canting in at, the vault crown, and the height of the timber caps.
const MINE_LEN: f32 = 40.0;
const MINE_HALF_W: f32 = 3.4;
const MINE_SPRING: f32 = 2.05;
const MINE_ROOF: f32 = 3.25;
const MINE_CAP: f32 = 2.45;
/// Centre-to-centre spacing of the timber support frames.
const MINE_FRAME_PITCH: f32 = 3.2;

/// The great hall, in tiles.
const KEEP_HX: f32 = 15.0;
const KEEP_HY: f32 = 6.5;
const KEEP_WALL: f32 = 5.4;
const KEEP_RIDGE: f32 = 8.2;

/// Radius of the Dark Forest's clearing — the ring the sampled treeline starts
/// at. The framing giants stand *inside* it, at [`FOREST_FRAME_R`].
const CLEARING_R: f32 = 10.5;
/// Where the giants that frame every forest shot stand. Far enough out that no
/// mark is inside the four-tile standoff, near enough that their crowns fill
/// the top corners of the frame instead of sitting on the horizon.
const FOREST_FRAME_R: f32 = 7.6;
/// Radius of the Swamp's open water pan, for the same reason.
const SWAMP_CLEAR_R: f32 = 6.6;

/// How many phases the Swamp's wisps drift through before coming round again.
/// A loop, not noise: the same phase renders the same bytes, which is what
/// lets the ride memoize a region frame at all.
pub(crate) const WISP_PHASES: u8 = 4;

/// Half-extent of the ground patch each stage sits on. A camera outside this
/// stands off the edge of the world and sees haze where the ground should be.
pub(crate) fn stage_extent(stage: Stage) -> f32 {
    match stage {
        Stage::DarkForest | Stage::Swamp => 26.0,
        Stage::Mines => MINE_LEN * 0.5 + 1.0,
        Stage::DragonKeep => KEEP_HX + 1.0,
        Stage::Homecoming => COURT_EXTENT,
    }
}

/// The terrain options a stage displaces its ground with — exposed so a camera
/// can stand *on* that surface instead of floating at z = 0.
pub(crate) fn stage_terrain(stage: Stage) -> TerrainOpts {
    match stage {
        // Interiors: a cut floor is flat, and `relief: 0` makes
        // `stage_ground_z` return exactly 0 for every point in them.
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
        // One wide, shallow, standing sheet of water with a grass margin.
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

/// Ground height under a point of `stage` built with `seed`.
pub(crate) fn stage_ground_z(stage: Stage, seed: u64, x: f32, y: f32) -> f32 {
    ground_z(seed, x, y, &stage_terrain(stage))
}

/// The staged mesh for one region.
///
/// `danger` (0..=3) is the stall depth: it thins the torches out, the way Z2's
/// tileset guts its bank-4 pixels at danger 3. `treasures` puts that many
/// (capped at [`MAX_CHESTS`]) lit chests on the stage's authored plinths.
/// `phase` (0..[`WISP_PHASES`]) steps the one thing in any region that moves:
/// the Swamp's will-o'-the-wisps.
pub(crate) fn region_stage(stage: Stage, seed: u64, danger: u8, treasures: u8, phase: u8) -> Mesh {
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

/// Whether a stage's geometry moves with the world clock. Only the Swamp does
/// — its wisps drift — and only that stage pays a cache slot per phase or a
/// re-key per bucket.
pub(crate) fn stage_animates(stage: Stage) -> bool {
    stage == Stage::Swamp
}

/// How many chests a stage can ever show. Three is the point at which a row of
/// lit boxes stops reading as "loot" and starts reading as a warehouse.
pub(crate) const MAX_CHESTS: usize = 3;

/// Where each stage stands its chests, in stage tiles. Authored, not sampled:
/// a chest is a lantern in a dark frame, so it has to land where a vantage can
/// see it and nowhere near enough to a vantage to break the 4-tile standoff.
fn chest_spots(stage: Stage) -> [(f32, f32); MAX_CHESTS] {
    match stage {
        Stage::Mines => [(-9.5, 2.1), (2.0, -2.2), (13.0, 2.0)],
        Stage::DarkForest => [(10.0, 5.0), (-9.0, 8.0), (4.0, 11.0)],
        Stage::Swamp => [(9.0, 6.0), (-8.0, 9.0), (6.0, -9.5)],
        Stage::DragonKeep => [(-2.5, -3.4), (3.5, 3.3), (8.0, -3.2)],
        Stage::Homecoming => [(2.4, -13.2), (-2.4, -16.6), (2.4, -19.8)],
    }
}

/// Sit a part on the stage's own ground, sunk slightly so no seam shows.
fn place(out: &mut Mesh, stage: Stage, seed: u64, part: Mesh, x: f32, y: f32, yaw: f32) {
    let z = stage_ground_z(stage, seed, x, y) - 0.12;
    out.merge(part.rotated_z(yaw).translated(v3(x, y, z)));
}

// ── props ─────────────────────────────────────────────────────────────────

/// A banded strongbox with its lid ajar and the hoard glowing out of it.
/// **Footprint 1.1 × 0.8, 0.78 tall**, 16 triangles.
fn chest() -> Mesh {
    let mut m = Mesh::new();
    prim::boxed(
        &mut m,
        v3(-0.55, -0.40, 0.0),
        v3(0.55, 0.40, 0.46),
        mat::WOOD,
        F_SIDES,
    );
    // The lid, tipped back off the box, and the light coming out under it.
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

/// A torch guttering on a wall face: a stub bracket and a flame, facing `-y`.
/// 8 triangles. `hot` picks live coals over candle-warm glass.
fn torch(hot: bool) -> Mesh {
    let mut m = Mesh::new();
    prim::boxed(
        &mut m,
        v3(-0.07, -0.26, -0.34),
        v3(0.07, 0.0, 0.06),
        mat::WOOD,
        F_SIDES,
    );
    let flame = if hot { mat::EMBER } else { mat::WINDOW };
    prim::cone(&mut m, 0.0, -0.20, 0.02, 0.52, 0.19, 5, 0.0, flame);
    m
}

/// A pole-mounted lamp for the gate road. **0.7 × 0.7, 3.0 tall**, 26 tris.
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

/// A drowned tree: a bare leaning trunk with three stubs of branch.
/// **≤ 2.2 × 2.2, 2.6–4.0 tall**, 44 triangles.
fn dead_tree(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x44_45_41_44)); // "DEAD"
    let mut m = Mesh::new();
    let h = r.range(3.0, 5.0);
    let lean = r.range(0.0, TAU);
    let tilt = r.range(0.10, 0.34);
    let (tx, ty) = (tilt * lean.cos(), tilt * lean.sin());
    // Trunk as two tapering prisms, the upper one leaning off the lower.
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

/// A will-o'-the-wisp: a cold little lamp floating over the water. 10 tris.
fn wisp(r: f32) -> Mesh {
    let mut m = Mesh::new();
    prim::cone(&mut m, 0.0, 0.0, 0.0, r * 1.5, r, 5, 0.0, mat::MOONLIGHT);
    prim::cone(&mut m, 0.0, 0.0, 0.0, -r * 1.5, r, 5, 0.0, mat::MOONLIGHT);
    m
}

/// A quad of PATH laid on the ground between two points — the winding forest
/// track and the mine's cart road. 2 triangles per call.
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

/// Stand `n` chests on the stage's authored plinths.
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

// ── The Mines ─────────────────────────────────────────────────────────────

/// A worked gallery: rock walls under a low rock vault, timber support frames
/// marching away into the dark, a rail line down the middle with a cart on it,
/// ore glinting in the faces, wall torches, and rubble at the feet.
///
/// **An interior.** The first pass built the Mines as an open-cast working so
/// they could clear the outdoor vista floor's 20 % sky — and the dump came
/// back reading as *a brick gate under a starry sky*. A mine is a hole in the
/// ground; roofing it is the identity. The stage is therefore routed through
/// the interior floors (ink bands, a lit floor band, a light in frame), which
/// is what the eight staged rooms are already judged by, and the outdoor floor
/// is left exactly where it is for the stages that are actually outdoors.
///
/// **Footprint `MINE_LEN` × 2·`MINE_HALF_W` tiles**, running along ±x, floor
/// at z = 0, vault at `MINE_ROOF`. The far end is unlit on purpose: fog takes
/// it, and that black hole at the end of the timbering is the depth cue the
/// whole frame is built on.
fn mines(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::Mines;
    let mut r = Rng::new(sub_seed(seed, 0x4D_49_4E_45)); // "MINE"
    let mut m = Mesh::new();
    let half = MINE_LEN * 0.5;

    // ---- floor -------------------------------------------------------------
    // One lantern-lit slab. `FLOOR` carries the near-bright / far-black depth
    // ramp on its own (raster.rs `LANTERN_REACH`), which indoors is doing the
    // work the sky does outdoors.
    prim::quad(
        &mut m,
        v3(-half, -MINE_HALF_W - 0.4, 0.0),
        v3(half, -MINE_HALF_W - 0.4, 0.0),
        v3(half, MINE_HALF_W + 0.4, 0.0),
        v3(-half, MINE_HALF_W + 0.4, 0.0),
        mat::FLOOR,
    );

    // ---- rock shell --------------------------------------------------------
    // Cut in 2-tile segments whose width and vault height wander a little, so
    // the walls read as hewn rock and not as a corridor in a hotel. The wander
    // is seeded, so it is the same rock every time.
    let segs = (MINE_LEN / 2.0) as usize;
    fn wall(m: &mut Mesh, x0: f32, x1: f32, y0: f32, y1: f32, r: &mut Rng) -> (f32, f32, f32) {
        let z0 = MINE_SPRING + r.jitter(0.12);
        let z1 = MINE_SPRING + r.jitter(0.12);
        let roof = MINE_ROOF + r.jitter(0.10);
        // Wall foot to springing.
        prim::quad(
            m,
            v3(x0, y0, 0.0),
            v3(x1, y1, 0.0),
            v3(x1, y1, z1),
            v3(x0, y0, z0),
            mat::STONE_DARK,
        );
        // Haunch canting in to the crown.
        prim::quad(
            m,
            v3(x0, y0, z0),
            v3(x1, y1, z1),
            v3(x1, y1 * 0.42, roof),
            v3(x0, y0 * 0.42, roof),
            mat::STONE_DARK,
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
        // The crown between the two haunches.
        prim::quad(
            &mut m,
            v3(x0, cl0, roof_l),
            v3(x1, cl1, roof_l),
            v3(x1, cr1, roof_r),
            v3(x0, cr0, roof_r),
            mat::STONE_DARK,
        );
    }
    // Caps, so neither end of the gallery is a hole into the void.
    for &x in &[-half, half] {
        prim::quad(
            &mut m,
            v3(x, -MINE_HALF_W - 0.4, 0.0),
            v3(x, MINE_HALF_W + 0.4, 0.0),
            v3(x, MINE_HALF_W + 0.4, MINE_ROOF),
            v3(x, -MINE_HALF_W - 0.4, MINE_ROOF),
            mat::STONE_DARK,
        );
    }

    // ---- timber support frames --------------------------------------------
    // The one thing that makes a tunnel read as a *mine* at braille scale: a
    // receding row of verticals with a beam across the top, each pair smaller
    // than the last, marching into the dark.
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

    // ---- the rail line -----------------------------------------------------
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
    // A cart, stopped on the rails in the near-middle distance. It is the one
    // piece of silhouette between the eye and the black end of the gallery.
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
            mat::STONE_DARK,
        );
    }

    // ---- ore in the faces --------------------------------------------------
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

    // ---- torches on the timbering -----------------------------------------
    // Danger guts the lights, the way Z2's tileset halves its bank-4 pixels
    // when a loop stalls — and the fog comes in to meet them
    // (`region::fog_distance`), so a deep stall is a gallery you cannot see
    // the end of.
    let lit = (8usize).saturating_sub(danger as usize * 2).max(2);
    for i in 0..lit {
        let x = -half + 2.4 + (MINE_LEN - 6.0) * (i as f32) / lit.max(1) as f32;
        let side = if i % 2 == 0 { -1.0_f32 } else { 1.0 };
        let y = side * (MINE_HALF_W - 0.34);
        place(
            &mut m,
            stage,
            seed,
            torch(true),
            x,
            y,
            if side < 0.0 { 0.0 } else { PI },
        );
    }

    // ---- rubble at the feet ------------------------------------------------
    for i in 0..14 {
        let x = -half + 1.0 + (MINE_LEN - 2.0) * (i as f32 + 0.3) / 14.0;
        let side = if i % 3 == 0 { -1.0_f32 } else { 1.0 };
        let y = side * (MINE_HALF_W - r.range(0.2, 0.7));
        let s = r.range(0.22, 0.52);
        prim::boxed(
            &mut m,
            v3(x - s, y - s * 0.7, 0.0),
            v3(x + s, y + s * 0.7, s * r.range(0.5, 1.0)),
            mat::STONE_DARK,
            F_SIDES_TOP,
        );
    }

    stand_chests(&mut m, stage, seed, chests);
    m
}

// ── The Dark Forest ───────────────────────────────────────────────────────

/// A clearing in deep woods: a winding track in from the south, a wall of
/// trunks all round, a canopy closing over the track beyond the clearing, and
/// stumps in the open ground.
///
/// **Footprint 52 × 52 tiles**, clearing radius 13.
fn dark_forest(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::DarkForest;
    let mut r = Rng::new(sub_seed(seed, 0x46_4F_52_53)); // "FORS"
    let mut m = Mesh::new();
    let extent = stage_extent(stage);
    m.merge(terrain_patch_with(
        extent * 2.0,
        extent * 2.0,
        seed,
        &stage_terrain(stage),
    ));

    // ---- the winding track ------------------------------------------------
    // A sine, not a straight strip: `TerrainOpts::path` only draws bands, and
    // a road you can see the far end of is not a forest road.
    let track = |t: f32| -> (f32, f32) { (7.5 * (t * 0.145).sin(), t) };
    let mut t = -extent + 1.0;
    while t < extent - 1.0 {
        let next = (t + 3.0).min(extent - 1.0);
        path_span(&mut m, stage, seed, track(t), track(next), 1.25);
        t = next;
    }

    // ---- the treeline -----------------------------------------------------
    // Rejection-sampled with a hard attempt cap: bounded, and deterministic
    // because the only entropy is the seeded stream. Nothing inside the
    // clearing, and nothing standing in the track.
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

    // ---- the giants that frame every shot ---------------------------------
    // A ring of grown trees standing *inside* the clearing ring, at
    // `FOREST_FRAME_R`. Every mark stands within 3 tiles of the origin and
    // looks outward, so two or three of these always land in the left and
    // right thirds with their crowns across the top of the frame — which is
    // what makes a shot read as *in the woods* rather than as a meadow with a
    // hedge at the far end. Far enough out that the nearest is 4.6 tiles from
    // any eye, so the standoff rail holds without being touched.
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

    // ---- the canopy closing over the track --------------------------------
    // Beyond the clearing only. A lid over the clearing itself would take the
    // sky the outdoor vista floor wants, and the forest is judged by that
    // floor — it is outdoors, whatever the canopy is doing.
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

    // ---- stumps in the open ----------------------------------------------
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

    // ---- fireflies over the clearing --------------------------------------
    // Danger takes them: a stalled loop's woods go dark.
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

// ── The Swamp ─────────────────────────────────────────────────────────────

/// A standing sheet of black water with a grass margin, drowned trees out of
/// it, and will-o'-the-wisps hanging over the shallows.
///
/// **Footprint 52 × 52 tiles**, open water out to radius 16.
fn swamp(seed: u64, danger: u8, chests: usize, phase: u8) -> Mesh {
    let stage = Stage::Swamp;
    let mut r = Rng::new(sub_seed(seed, 0x53_57_4D_50)); // "SWMP"
    let mut m = Mesh::new();
    let extent = stage_extent(stage);
    m.merge(terrain_patch_with(
        extent * 2.0,
        extent * 2.0,
        seed,
        &stage_terrain(stage),
    ));
    let water = |x: f32, y: f32| stage_ground_z(stage, seed, x, y);

    // ---- the drowned wood --------------------------------------------------
    // Sampled in polar coordinates rather than rejection-sampled in the
    // square: a flat black pan with two sticks on it is not a swamp, and
    // uniform-in-the-square puts most of its draws in the far corners where
    // the haze eats them. Polar keeps the wood in the 7–23 tile band, where a
    // trunk still reads as a trunk, and packs the near end of that band.
    let mut planted = 0usize;
    while planted < 52 {
        let a = r.range(0.0, TAU);
        // Square-rooting a uniform draw would spread the wood evenly over the
        // *area*; squaring it does the opposite and crowds the near ring,
        // which is where a trunk is worth its triangles.
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

    // ---- lily pads on the near water --------------------------------------
    // `GRASS` on purpose, not `FOLIAGE`: the ground materials are the ones the
    // rider's lantern pools on (raster.rs `lantern_lit`), so a raft of pads a
    // few tiles off the eye lights up and gives the flat pan a near field. It
    // also keeps them out of the standoff scan, which is right — a lily pad is
    // not a wall.
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

    // ---- reeds -------------------------------------------------------------
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

    // ---- the mist band -----------------------------------------------------
    // A pale bar of `MOONLIGHT` lying along the far water. There is no
    // volumetric fog in the rasterizer, so mist has to be *geometry*: a ring
    // of low tangential slabs out at the fog's own distance, which reads as a
    // lit band under the horizon exactly the way a real mist bank does. Cheap
    // — 12 quads — and it is the single thing that turns a black pan into a
    // swamp at 48 × 18 cells.
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

    // ---- will-o'-the-wisps and what the water does with them --------------
    // The wisps drift: `phase` walks each one a fifth of a turn around its own
    // little orbit, so a settled swamp is never a still photograph. It is a
    // *loop* of `WISP_PHASES` steps, not noise, which is what keeps the frame
    // memoizable — the same phase renders the same bytes, forever.
    //
    // Each one also drops a reflection. There is no mirror in the rasterizer
    // either, so it is a flat emissive disc lying on the water directly under
    // the light, slightly wider and dimmer-looking for being seen at a grazing
    // angle. At dot scale that is exactly what a reflected lamp looks like.
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

// ── The Dragon Keep ───────────────────────────────────────────────────────

/// The great hall the judge holds court in: a colonnade down both sides,
/// banners on the walls, a red runner leading to a throne dais at the far end,
/// braziers either side of it, and the dragon on the dais.
///
/// **An interior**, for the same reason the Mines are. The first pass built a
/// roofless ruin so it could clear the outdoor sky floor, and the dump came
/// back reading as *the castle court with some fires in it* — no hall, no
/// dragon. A great hall is a roof and two rows of columns; without them there
/// is nothing to be inside of.
///
/// **Footprint 2·`KEEP_HX` × 2·`KEEP_HY` tiles**, floor at z = 0, wall head at
/// `KEEP_WALL`, ridge at `KEEP_RIDGE`.
fn dragon_keep(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::DragonKeep;
    let mut r = Rng::new(sub_seed(seed, 0x4B_45_45_50)); // "KEEP"
    let mut m = Mesh::new();
    let (hx, hy) = (KEEP_HX, KEEP_HY);

    // ---- floor, runner, shell ---------------------------------------------
    prim::quad(
        &mut m,
        v3(-hx, -hy, 0.0),
        v3(hx, -hy, 0.0),
        v3(hx, hy, 0.0),
        v3(-hx, hy, 0.0),
        mat::FLOOR,
    );
    // The runner: a red road straight to the dais. Narrow on purpose — the
    // lantern-lit flags either side of it are what carry the room's depth
    // ramp, and a carpet wall to wall would take that away.
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
        // Open timber ceiling: two slopes to a ridge. The shape that says
        // "hall" rather than "box".
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

    // ---- the colonnade -----------------------------------------------------
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
            // A banner hung on the wall behind each column.
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

    // ---- the dais and the throne ------------------------------------------
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

    // ---- fire --------------------------------------------------------------
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
        prim::cone(&mut brazier, 0.0, 0.0, 0.70, 1.36, 0.36, 5, 0.0, mat::EMBER);
        place(&mut m, stage, seed, brazier, x, side * 3.5, r.jitter(0.5));
    }
    // Wall torches on the colonnade, so the walls are not black between fires.
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

    // ---- the dragon --------------------------------------------------------
    m.merge(
        dragon(sub_seed(seed, 0x44_52_47_4E))
            .rotated_z(PI - 0.20)
            .translated(v3(hx - 3.1, 0.5, 0.76)),
    );

    stand_chests(&mut m, stage, seed, chests);
    m
}

fn dragon(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x57_59_52_4D)); // "WYRM"
    let mut m = Mesh::new();
    let hide = mat::STONE_DARK;

    // Body: three barrels along -x, haunch to shoulder.
    prim::prism(&mut m, 1.7, 0.0, 0.55, 1.95, 1.15, 1.05, 6, 0.3, hide);
    prim::prism(&mut m, 0.2, 0.0, 0.45, 2.15, 1.05, 0.92, 6, 0.0, hide);
    prim::prism(&mut m, -1.3, 0.0, 0.50, 1.95, 0.92, 0.72, 6, 0.2, hide);

    // Neck: two lengths rising forward, then the wedge of a head.
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
        // Horn.
        prim::cone(&mut m, -3.1, side * 0.26, 4.24, 5.10, 0.16, 4, 0.0, hide);
    }

    // Tail: three tapering lengths sweeping back and round.
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

    // Wings: a spar out and up, and one membrane triangle hung off it. Two
    // triangles per membrane is all the dither can resolve at vista range.
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

    // Legs.
    for &(lx, lz) in &[(1.9_f32, 1.45_f32), (-0.9, 1.35)] {
        for side in [-1.0_f32, 1.0] {
            prim::prism(&mut m, lx, side * 1.0, 0.0, lz, 0.34, 0.26, 4, 0.4, hide);
        }
    }
    m
}

// ── Homecoming ────────────────────────────────────────────────────────────

/// The castle court exactly as the realm always was, with the gate road lit
/// for the party walking home: lamp posts down both verges and the loot on the
/// cobbles inside the gate.
///
/// **Footprint 56 × 56 tiles** — the court's own patch, unchanged.
fn homecoming(seed: u64, danger: u8, chests: usize) -> Mesh {
    let stage = Stage::Homecoming;
    let mut m = court_scene(seed);
    // Lamps stand 3.2 tiles off the road centre: outside the ride sweep's
    // corridor (`world3d::ROAD_*` runs x ∈ 0.6..4.6) and well inside the
    // hamlet, so nothing new comes within a vantage's standoff.
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
