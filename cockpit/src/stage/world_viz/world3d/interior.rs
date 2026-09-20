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
use super::instruments::{self, tube};
use super::math::{V3, v3};
use super::mesh::{Mesh, mat};
use super::raster::{self, View3};
use super::scene::SCENE_SEED;

/// The moon's absolute bearing in radians — the same disc the sky shader hangs
/// (raster.rs:438). Rooms that open on the night orient by it.
const MOON_AZIMUTH: f32 = super::MOON_BEARING * TAU;

/// The heading `interiors::compose` authors for every room (`interiors.rs:407`)
/// before the ride adds operator yaw. Everything past it is the operator and
/// the settled sway, which is exactly what should pan the staged camera.
const BASE_HEADING: f32 = -FRAC_PI_2;

/// Soft limit on the operator's pan, in radians. `tanh` bends rather than
/// clamps, so two different operator headings never render the same frame.
const SWING_LIMIT: f32 = 1.60;

/// How much of that pan the staged interior camera actually takes.
///
/// Measured in the live cockpit, not guessed: the settled orbit sway runs to
/// ±0.30 rad (`life.rs:757`) and an interior lens has `tan(fov/2) ≈ 0.5`, so an
/// un-damped sway walks the room's hero piece ±0.31 of the frame width — a
/// 160 × 48 capture put the Keep's hearth on its third and a 240 × 64 one, two
/// sway phases later, had it hanging off the right edge. Outdoors the same sway
/// is a breath across a 56-tile court; indoors it is the whole composition.
/// Damping keeps the arrow keys answering (and answering monotonically) while
/// holding the staged frame together.
const SWING_GAIN: f32 = 0.55;

/// How far the pan dollies the eye sideways, in tiles. The room breathes with
/// the operator instead of standing dead still; far too small to walk the
/// camera into a wall.
const PARALLAX: f32 = 0.55;

/// Vertical field ceiling, shared with the exterior staging (world3d.rs:430):
/// braille dots are square, so a tall pane would otherwise stretch the vertical
/// field past 80° and shrink the whole room to a smudge.
const VERT_FOV_MAX: f32 = 1.12;

// ── the eight rooms ───────────────────────────────────────────────────────

/// One staged interior: the room's proportions, its openings, and the camera
/// mark inside it.
///
/// Room-local convention: the room is centred on the origin with the floor at
/// `z = 0`, the **identity wall at +x**, the door behind the camera at −x, and
/// the ceiling ridge running along `y = 0`. The camera stands near −x and looks
/// toward +x. `yaw` then turns the whole room (mesh *and* camera) into world
/// space, which is how a room aims its opening at the moon without any of the
/// authored numbers below changing.
struct Chamber {
    /// Half-extents of the inside face of the shell, in tiles.
    hx: f32,
    hy: f32,
    /// Wall head and ceiling ridge heights.
    z_wall: f32,
    z_ridge: f32,
    /// Wall material — STONE for the civic halls, WOOD for the timber shells,
    /// matching the raycast rooms' own `shell` field (interiors.rs:306).
    shell: u8,
    /// x positions of the arched WINDOW bays, mirrored on both flank walls.
    bays: &'static [f32],
    /// Sill height of those bays.
    bay_sill: f32,
    /// x positions on the **+y** flank where a light shaft drops from the bay
    /// to a pool on the floor. One or two per room; more reads as bunting.
    /// Every entry must also appear in `bays` — a shaft is the light *from* a
    /// window, and one falling out of blank masonry reads as a rendering bug
    /// (`every_shaft_falls_from_a_real_bay` holds the line).
    shafts: &'static [f32],
    /// Room orientation in world space.
    yaw: f32,
    /// Eye mark, room-local, and its height above the floor.
    eye: (f32, f32),
    eye_h: f32,
    /// The identity piece the shot is about, room-local.
    anchor: (f32, f32, f32),
    /// Where the anchor sits across and down the frame (0 = left/top).
    frame_x: f32,
    frame_y: f32,
    /// Focal length as a multiplier on the ride's lens; below 1.0 is longer.
    lens: f32,
    /// What the frame is about — for the dump sheet and the test table.
    hero: &'static str,
}

/// The eight rooms in `building_index` order: Keep, Gatehouse, Rookery,
/// Scriptorium, Smithy, Chapel, Round Table, Observatory.
const CHAMBERS: [Chamber; 8] = [
    // Keep — the hearth hall. The fire mouth burns on the end wall behind the
    // long table; banners hang either side of it.
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
    // Gatehouse — the passage, open to the night at the far end with the
    // portcullis teeth down across it and the moon beyond. Low and long.
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
    // Rookery — the timber loft: three perch beams under heavy rafters, birds
    // roosting on them, and the loft hatch open on the moon to the left.
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
    // Scriptorium — the hero interior, `arch::library_interior`: shelf rows,
    // reading tables under candle light, the hearth at the far end and window
    // bays running both flanks.
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
        // Off the centre line on purpose. Standing in the middle of the aisle
        // put the two shelf rows exactly edge-on and the shot became a
        // corridor: two brown walls, a bright hole at the end, and not one
        // book anywhere in it. From the −y quarter the stacks that now project
        // off that wall (`arch::library_interior`) present their faces
        // *square* to the lens and telescope into the left third, and the
        // hearth still burns on its own third to the right of them.
        eye: (-3.95, -0.30),
        eye_h: 1.82,
        anchor: (4.85, 0.0, 1.00),
        frame_x: 0.62,
        frame_y: 0.55,
        lens: 0.86,
        hero: "the reading hall past the shelf stacks to the hearth",
    },
    // Smithy — the forge: the fire in its hood on the end wall, the anvil on
    // its stump between the eye and the light.
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
    // Chapel — the nave: a rose window high on the east wall, the altar and
    // its tapers under it, pews leading in.
    Chamber {
        hx: 6.0,
        hy: 3.2,
        z_wall: 4.6,
        z_ridge: 7.0,
        shell: mat::STONE,
        // The third bay is the shaft's source. Without it the nave's pool of
        // moonlight fell out of a blank wall, which is a light with no window
        // — the one thing an audience notices instantly and cannot name.
        bays: &[-3.6, -1.2, 2.9],
        bay_sill: 2.30,
        // Far enough up the nave to be inside a 42° field: the eye stands four
        // tiles off the west wall, so a bay any nearer than about seven tiles
        // sits behind the frame edge and drops its shaft off the picture.
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
    // Round Table — the council chamber: the ring of light on the table with
    // the empty seats around it, seen from the one seat left open.
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
    // Observatory — the star slit: the wall opens on the night from sill to
    // ridge and the telescope leans up into it.
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
        anchor: (2.30, -0.36, 2.33),
        frame_x: 0.41,
        frame_y: 0.53,
        lens: 0.94,
        hero: "the telescope leaning into the star slit",
    },
];

/// What the frame is about, for the dump sheet.
pub(crate) fn hero(index: u8) -> &'static str {
    CHAMBERS[(index as usize).min(7)].hero
}

// ── the ride seam ─────────────────────────────────────────────────────────

/// Retained interior geometry renderer for scene fixtures and review dumps.
/// Ordinary room entry displays the approved room/location plate instead.
///
/// Reads only the building, the
/// `RayView` the ride already built — whose heading carries the settled sway
/// plus operator yaw, and whose `bob` carries operator pitch — and `bucket`,
/// which is the same held tick bucket the cinematic key hashes. No
/// clock, no world state the key does not cover: hand it a bucket the key did
/// not see and the memo would serve a stale fire.
pub(crate) fn render_interior_frame(
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

// ── the fire breathes ─────────────────────────────────────────────────────

/// Steps in one breath of the firelight before it comes round again.
///
/// Four, because three reads as a stutter and eight is a light show. The cycle
/// is a *loop*, not noise: the same room at the same phase renders the same
/// bytes, which is what lets the ride memoize interiors at all.
pub(crate) const BREATH_STEPS: u64 = 4;

/// How many `cinematic_key` buckets one step of that breath lasts.
///
/// Measured, not guessed. The key's interior branch steps on `tick / 2`
/// (cinematics.rs:215) and the world ticks every 25 ms by wall clock
/// (app_control/turn_io.rs:319), so one bucket is 50 ms. At a step per bucket
/// the hearth would gutter at 20 Hz — a fault light on a dashboard, not a fire.
/// Eight buckets puts a step at 0.4 s and the whole four-step cycle at 1.6 s,
/// which is the rate a real fire draws breath at.
pub(crate) const BUCKETS_PER_STEP: u64 = 8;

/// One step of the breath as `(emissive gain, warm swing)`.
///
/// SUBTLE IS THE LAW. The whole excursion is ±3.5% of the flame's authored
/// value with a couple of code values of hue on it — at braille dot scale that
/// moves a handful of dots at the edge of the fire's dither and nothing else,
/// which is exactly what firelight does to a stone wall. Anything an eye can
/// *catch* here reads as a rendering fault, because the room is otherwise
/// dead still.
///
/// Shaped like a fire and not like a sine, and biased **downward** on purpose.
/// Two reasons, one artistic and one mechanical: a fire at rest is already at
/// its brightest and spends its time dipping between draws of air; and the lit
/// pane of `window_tint` is [255, 198, 116], whose red is already at the top of
/// the scale, so gain above 1.0 clips and desaturates instead of brightening —
/// the visible half of the swing is the half that dims. Gain and warm pull
/// against each other throughout: a flame is palest when it burns hardest.
const BREATH: [(f32, f32); BREATH_STEPS as usize] = [
    (1.000, 0.00), // full burn, the authored fire
    (0.972, 0.05), // settling back, redder
    (0.988, 0.02), // catching again
    (0.958, 0.08), // the dip, and its deepest red
];

/// Where each room stands in that cycle, so eight hearths do not breathe in
/// lockstep — they are eight separate fires in eight separate buildings and
/// only the operator's own frame rate connects them.
const BREATH_PHASE: [u64; 8] = [0, 2, 3, 1, 0, 3, 1, 2];

/// The live flame in room `index` at tick-bucket `bucket` — a pure function of
/// the two, and of nothing else.
pub(crate) fn firelight(index: u8, bucket: u64) -> raster::Firelight {
    let room = (index as usize).min(BREATH_PHASE.len() - 1);
    let step = (bucket / BUCKETS_PER_STEP).wrapping_add(BREATH_PHASE[room]) % BREATH_STEPS;
    let (gain, warm) = BREATH[step as usize];
    raster::Firelight { gain, warm }
}

/// The staged camera inside a room: the authored mark, panned by the operator.
///
/// Heading is solved so the room's identity piece lands on its authored third,
/// and pitch so it lands at its authored height down the frame — the same
/// screen-fraction framing the exterior vantages use, which is what makes one
/// table hold up from a 96 × 72 ride pane to a 200 × 304 plate.
fn staged_view(
    index: u8,
    view: &super::super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> View3 {
    let chamber = &CHAMBERS[(index as usize).min(CHAMBERS.len() - 1)];

    // Field of view: the operator's zoom, capped so a portrait pane cannot blow
    // the vertical field out, then lensed.
    let aspect = dot_h.max(1) as f32 / dot_w.max(1) as f32;
    let mut tan_h = (view.fov_rad.clamp(0.70, 1.40) * 0.5).tan();
    let tan_v_max = (VERT_FOV_MAX * 0.5).tan();
    if tan_h * aspect > tan_v_max {
        tan_h = tan_v_max / aspect.max(1e-3);
    }
    tan_h *= chamber.lens.clamp(0.35, 1.0);
    let tan_v = tan_h * aspect;

    // Everything past the authored interior heading is the operator's pan plus
    // the settled orbit sway. Soft-limited rather than clamped: a hard clamp
    // makes two different headings render one frame once they saturate, which
    // is a camera that has stopped answering the arrow keys.
    let raw = wrap_pi(view.heading_rad - BASE_HEADING);
    let swing = SWING_LIMIT * (raw / SWING_LIMIT).tanh() * SWING_GAIN;

    // Sub-tile parallax: the eye slides along the room's own cross axis as the
    // operator pans, so the near furniture moves against the far wall. Applied
    // before the heading solve, so the anchor stays exactly on its third while
    // the eye breathes.
    let local = (
        chamber.eye.0,
        chamber.eye.1 + (swing * 0.5).clamp(-1.0, 1.0) * PARALLAX,
    );
    let eye = rotate(local, chamber.yaw);
    let anchor = rotate((chamber.anchor.0, chamber.anchor.1), chamber.yaw);

    // Solve the heading that puts the anchor at `frame_x`: a point at angular
    // offset θ lands at 0.5·(1 + tanθ/tan_h) across the frame.
    let bearing = (anchor.1 - eye.1).atan2(anchor.0 - eye.0);
    let heading = bearing - (tan_h * (2.0 * chamber.frame_x - 1.0)).atan() + swing;

    // ...and the pitch that puts it at `frame_y` down the frame: a ray at world
    // elevation `elevation` lands at 0.5·(1 − tan(elevation − pitch)/tan_v).
    let reach = ((anchor.0 - eye.0).powi(2) + (anchor.1 - eye.1).powi(2))
        .sqrt()
        .max(1e-3);
    let elevation = ((chamber.anchor.2 - chamber.eye_h) / reach).atan();
    let pitch = elevation - (tan_v * (1.0 - 2.0 * chamber.frame_y)).atan();
    // The ride's `bob` is the operator's pitch in units of 3% of frame height
    // (ride.rs:142), exactly as the exterior staging reads it.
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

// ── the rooms themselves ──────────────────────────────────────────────────

/// Build one interior. The Scriptorium is the hand-built hero hall; everybody
/// else gets the parameterized chamber plus their identity dressing.
///
/// Pure function of `index` — the determinism law. `arch::library_interior`
/// and every helper below take an explicit seed and read no ambient state.
pub(crate) fn interior_mesh(index: u8) -> Mesh {
    let index = index.min(7);
    let chamber = &CHAMBERS[index as usize];
    let seed = sub_seed(SCENE_SEED, 0x494E_5445_0000_0000 | index as u64); // "INTE"
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

/// The shared great hall: floor, shell, open timber ceiling, window bays and
/// their light, then the room's own identity piece.
fn chamber_mesh(chamber: &Chamber, seed: u64, index: u8) -> Mesh {
    let mut rng = Rng::new(sub_seed(seed, 0x484F_4C4C)); // "HOLL"
    let mut mesh = Mesh::new();
    let (hx, hy) = (chamber.hx, chamber.hy);

    // Floor. Lantern-lit, so it carries the room's depth ramp on its own.
    prim::quad(
        &mut mesh,
        v3(-hx, -hy, 0.0),
        v3(hx, -hy, 0.0),
        v3(hx, hy, 0.0),
        v3(-hx, hy, 0.0),
        mat::FLOOR,
    );

    // Shell. The two flanks and the door wall are plain; the +x wall belongs to
    // the room's identity and is built by the dressing below.
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
    // Door wall behind the eye, with the way out punched through it.
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

    // A plinth course around the room: one strong horizontal that stops the
    // walls reading as a folded sheet at dot scale. A *ring*, not a slab — a
    // full box here would roof the whole floor over at 0.42 tiles and take the
    // lantern-lit flags (and the room's entire depth ramp) with it.
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

    // Open timber ceiling with tie beams — the shape that says "hall".
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

    // Window bays on both flanks, and the light they throw on the floor.
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

/// The plain +x end wall, for rooms whose identity does not rebuild it.
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

// ── dressing ──────────────────────────────────────────────────────────────

/// Keep — the long hearth burning under its hood, banners either side of it,
/// and the trestle table leading the eye in.
fn dress_keep(mesh: &mut Mesh, chamber: &Chamber, rng: &mut Rng) {
    end_wall(mesh, chamber);
    let x = chamber.hx;

    // Chimney breast, with a battered hood taking it up into the gable.
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

    // The fire itself: an arched mouth of light, with the ember bed showing
    // through it and a pool of firelight on the flags in front.
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

    // Banners flanking the fire.
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

    // Trestle table down the hall with benches and three candles on it.
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

/// Gatehouse — the passage opens on the night; the portcullis is down across
/// it and two braziers burn at the jambs.
fn dress_gatehouse(mesh: &mut Mesh, chamber: &Chamber) {
    let x = chamber.hx;
    let (oy, oz) = (1.95_f32, 3.20_f32);

    // The gate mouth: the end wall with a big opening punched through it, the
    // top corners chamfered so it reads as an arch and not a letterbox.
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
    // Ground outside, so the mouth frames a moonlit road rather than a void.
    prim::quad(
        mesh,
        v3(x, -14.0, 0.0),
        v3(x + 22.0, -14.0, 0.0),
        v3(x + 22.0, 14.0, 0.0),
        v3(x, 14.0, 0.0),
        mat::PATH,
    );

    // Portcullis: iron teeth down across the opening.
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

    // Braziers at the jambs — the only warm light in a cold passage.
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
        prim::disc(mesh, bx, by, 1.25, 0.40, 8, 0.0, mat::FIRE);
        prim::cone(mesh, bx, by, 1.25, 1.86, 0.34, 6, 0.0, mat::FIRE);
    }
}

/// Rookery — the loft door stands open on the night and the perch rows cross
/// in front of it, so every bird in the room is a black shape against the moon.
fn dress_rookery(mesh: &mut Mesh, chamber: &Chamber, rng: &mut Rng) {
    let (hx, hy) = (chamber.hx, chamber.hy);
    let half = 2.05_f32;

    // The loft door: the whole middle of the end wall is missing, from waist
    // height on up through the gable to the ridge, because the moon rides at
    // ~0.29 rad and an opening that stops at the wall head would sit under it.
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
    // The door leaf, swung inboard off the near jamb.
    prim::quad(
        mesh,
        v3(hx - 0.02, -half, 1.22),
        v3(hx - 1.15, -half - 0.46, 1.22),
        v3(hx - 1.15, -half - 0.46, 3.20),
        v3(hx - 0.02, -half, 3.20),
        mat::WOOD,
    );
    // Sill boards, and the light landing on them.
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

    // Perch rows crossing the doorway's light, ravens roosting along them.
    //
    // The heights are not arbitrary and the rows are not level. The sky's
    // luminous band sits within ~0.17 rad of the horizon (raster.rs:81) and
    // everything below the horizon is flat night haze, so a bird only reads as
    // a silhouette if it stands **just above the eye line and no more than
    // ~0.17 × its distance above it**. A level row puts the near birds against
    // black wall and only the far ones against light. The rows therefore rise
    // with depth — which is also how a real loft is built, the near roosts
    // under the far ones — and the camera sits low (`eye_h` 1.15) so the
    // horizon runs under every roost in the room.
    // (x, perch height, birds). The heights clear the eye line by a little and
    // stay under the 0.17-rad glow band; the depths keep every bird clear of
    // the tie beams, which cross the doorway at the wall head.
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
            // Spread along the whole beam, not huddled on its middle. The old
            // pitch bunched every bird inside ±0.9 of the room's centre line,
            // so four rows of roosts arrived as one merged dark ridge; a raven
            // is only a raven if there is night either side of it.
            let span = 3.70_f32;
            let by = if birds <= 1 {
                rng.range(-1.35, 1.35)
            } else {
                -span * 0.5 + slot as f32 * span / (birds as f32 - 1.0) + rng.jitter(0.30)
            };
            // A real profile, not three boxes. The camera looks down the room
            // at +x, so the birds are drawn broadside across y: beak, head-step
            // and tail-notch all land square on the moonlit doorway behind
            // them, which is the only surface in the room bright enough to
            // silhouette anything against.
            prim::bird_profile(
                mesh,
                v3(px, by, z + 0.16),
                v3(0.0, 1.0, 0.0),
                0.78,
                mat::STONE_DARK,
            );
        }
    }

    // Rafters over the perches, a hanging lantern for the near warmth, and the
    // ladder to the loft.
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
    // A brazier by the ladder: at 48 × 18 cells the loft door's cool band is
    // half a dozen dots, so the room needs one warm mass that survives the
    // tone curve or it reads as an empty black pane.
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
    prim::disc(mesh, 2.40, -1.35, 1.07, 0.38, 8, 0.0, mat::FIRE);
    prim::cone(mesh, 2.40, -1.35, 1.07, 1.58, 0.32, 6, 0.0, mat::FIRE);

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

/// Smithy — the forge fire in its hood, the anvil on its stump in front of it,
/// the quench barrel and the tool rack.
fn dress_smithy(mesh: &mut Mesh, chamber: &Chamber) {
    end_wall(mesh, chamber);
    let x = chamber.hx;

    // Forge: a stone bed under a battered hood, the fire burning in its mouth.
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
    // Firelight on the floor in front of the bed.
    prim::quad(
        mesh,
        v3(x - 3.10, -2.10, 0.03),
        v3(x - 1.20, -2.10, 0.03),
        v3(x - 1.20, 0.95, 0.03),
        v3(x - 3.10, 0.95, 0.03),
        mat::PATH,
    );

    // Anvil on its stump, between the eye and the fire.
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

    // Quench barrel and the tool rack on the far flank.
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

/// Chapel — the rose window over the altar, tapers burning on it, an arcade
/// and pews leading the eye up the nave.
fn dress_chapel(mesh: &mut Mesh, chamber: &Chamber) {
    end_wall(mesh, chamber);
    let x = chamber.hx;

    // Rose window: a lit wheel inside a dark stone ring.
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
    // Lancets under it.
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

    // Altar on its step, two tapers burning at the corners.
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

    // Arcade columns and the pews.
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

/// Round Table — the ring of light on the board with the empty seats around it.
fn dress_round_table(mesh: &mut Mesh, chamber: &Chamber) {
    end_wall(mesh, chamber);

    // The table: a pedestal, a rim, a board, and the lit ring inlaid in it.
    prim::prism(mesh, 0.0, 0.0, 0.0, 0.72, 1.00, 0.80, 6, 0.0, mat::WOOD);
    prim::prism(mesh, 0.0, 0.0, 0.72, 0.90, 2.55, 2.55, 12, 0.0, mat::WOOD);
    prim::disc(mesh, 0.0, 0.0, 0.90, 2.55, 12, 0.0, mat::WOOD);
    instruments::council_map(mesh);

    // Seats around it — all but the one the camera is standing at.
    for i in 0..8 {
        let angle = TAU * i as f32 / 8.0 + 0.39;
        // The seat the camera is standing at stays empty — filling it would
        // mass a chair back across the middle of the board and close off the
        // whole shot. Index 4 is the one on the staged eye's own bearing.
        if i == 4 {
            continue;
        }
        let (sin, cos) = angle.sin_cos();
        let (sx, sy) = (cos * 3.35, sin * 3.35);
        let seat = {
            let mut chair = Mesh::new();
            // A chair is a *stepped* silhouette or it is a fence post. The
            // previous seat was one 0.68-wide slab running floor to 2.14 with
            // a rail barely wider than itself — at the twelve dots of height
            // the ring gets, that is a picket, and eight of them read as a
            // fence around the board. Three steps fix it, and all three have
            // to be at least a quarter-tile so the Bayer screen keeps them:
            //   1. a seat wider than the back it carries,
            //   2. two uprights with real daylight between them,
            //   3. a top rail that oversails the uprights on both sides.
            // The eye finds a chair from the daylight, not from the timber.
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
            // The splat between them — low and broad, so the gap the eye reads
            // is the one *above* it, right under the rail.
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

    // Candles standing on the board, so the near half of the table is not one
    // flat plank between the eye and the ring.
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

    // Banners on the walls behind them.
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

/// Observatory — the star slit open from sill to ridge with the telescope
/// leaning up into it.
fn dress_observatory(mesh: &mut Mesh, chamber: &Chamber) {
    let (x, hy) = (chamber.hx, chamber.hy);
    let slit = 1.40_f32;

    // The slit: the end wall opens from the sill up, and the gable above it
    // splits either side so the night runs all the way to the ridge.
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
    // What comes *through* the slit. The opening itself is real night sky, so
    // the room already has a hole in it; what it lacked was any evidence the
    // hole lets light in. A cold wash on the flags in front of it supplies
    // that, and the tripod legs standing black in the middle of it are what
    // turn a bright patch into a shaft.
    prim::quad(
        mesh,
        v3(x - 3.40, -2.05, 0.03),
        v3(x - 0.06, -2.05, 0.03),
        v3(x - 0.06, 2.05, 0.03),
        v3(x - 3.40, 2.05, 0.03),
        mat::PATH,
    );
    // Wound from the slit inward, so the moonlight's own fall-off runs the
    // right way down the floor.
    prim::quad(
        mesh,
        v3(x - 0.10, -slit - 0.10, 0.05),
        v3(x - 0.10, slit + 0.10, 0.05),
        v3(x - 2.70, slit + 0.10, 0.05),
        v3(x - 2.70, -slit - 0.10, 0.05),
        mat::MOONLIGHT,
    );

    // Reveals down the slit's cheeks, so the wall reads as a thickness the
    // night is cut through rather than as a painted rectangle.
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

    // The telescope: a tapered tube on a yoke, leaning up the slit, with the
    // eyepiece drawn back over the observer's stool.
    let base = v3(0.55, -0.60, 1.00);
    let muzzle = v3(4.05, -0.12, 3.65);
    instruments::telescope(mesh, base, muzzle);
    // Yoke and tripod.
    for &sy in &[-1.0_f32, 1.0] {
        prim::boxed(
            mesh,
            v3(1.62, sy * 0.52 - 0.08, 0.0),
            v3(1.78, sy * 0.52 + 0.08, 1.72),
            mat::WOOD,
            F_SIDES,
        );
        tube(
            mesh,
            v3(1.70, sy * 0.52, 1.30),
            v3(0.62, sy * 1.02, 0.0),
            0.08,
            0.06,
            5,
            mat::WOOD,
        );
    }
    prim::boxed(
        mesh,
        v3(1.42, -0.78, 0.0),
        v3(1.98, 0.78, 0.20),
        mat::WOOD,
        F_SIDES_TOP,
    );

    // Chart stand with its lamp, and the star globe on its post.
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
    // A lamp hung over the instrument. Same reason as the rookery's brazier:
    // the star slit alone is cool and thin, and the ride pane crushes it.
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

// ── local primitives ──────────────────────────────────────────────────────

/// A wall panel on the plane spanned by unit `right`/`up` from `origin`, with a
/// rectangular hole punched through it. Emits the four surviving strips (any
/// that collapse are dropped by `prim::quad`'s degenerate guard), so a doorway
/// or a gate mouth costs eight triangles and nothing has to be modelled twice.
///
/// `hole` is `(u0, u1, v0, v1)` in the panel's own coordinates.
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
    // Below and above the hole, full width.
    prim::quad(
        mesh,
        at(0.0, 0.0),
        at(w, 0.0),
        at(w, v0),
        at(0.0, v0),
        material,
    );
    prim::quad(mesh, at(0.0, v1), at(w, v1), at(w, h), at(0.0, h), material);
    // The jambs either side of it.
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

/// The light a bay on the +y flank throws into the room: a wedge of moonlight
/// from the sill down to a pool on the flags, with a lit halo of stone round it.
///
/// This started life as a WINDOW slab and had to be deleted: `window_tint`'s
/// leaded grid (raster.rs) is sized for a pane, so stretched over two and a
/// half tiles it read as a glowing lattice ramp propped against the wall, and
/// it was the first thing the eye found in three of the eight rooms. The idea
/// was never wrong — light arrives from off-frame and lands somewhere — only
/// the material was. `mat::MOONLIGHT` is cold, unpatterned, weaker than a
/// flame and less fog-proof, and [`prim::light_shaft`] gives it a body that an
/// axial camera can actually see, so the shaft is back and this time it reads
/// as air.
///
/// `sill` is the bay's own sill height; the wedge starts a little above it, at
/// the middle of the opening, so the beam looks like it came *through* glass
/// rather than out of a slot in the masonry.
fn light_pool(mesh: &mut Mesh, x: f32, hy: f32, sill: f32) {
    let cy = hy - 1.75;
    // The halo first, so the bright patch wins the depth test on top of it.
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

/// The +x gable with a slit cut out of its middle, so an opening in the wall
/// below runs on up to the ridge. The apex above the slit is left open: past
/// the end wall there is no roof, so the night runs straight in.
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

// ── test windows ──────────────────────────────────────────────────────────

/// The staged interior camera as `(x, y, z, heading, pitch, fov)` — the same
/// review window `world3d::staged_eye` opens on the exterior vantages.
#[cfg(test)]
pub(crate) fn staged_eye(
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

/// `(sky, built mass, ground)` for a staged interior — a room is not held to
/// the vista law, but the mix is how a dump sheet explains a frame.
#[cfg(test)]
pub(crate) fn composition_mix(
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

/// How far away the thing **in the middle of the picture** is — the
/// anti-wall-close-up rail, measured the only way that means anything indoors.
///
/// A bearing sweep over the mesh is the wrong instrument here: a room is a box
/// the camera stands *inside*, so there is always a bookcase or a rafter within
/// a tile of the eye at the edge of the field, and a nearest-vertex rail either
/// fails every honest interior or has to be loosened until it proves nothing.
/// Instead this shoots the nine rays of the central 24% of the frame into the
/// scene and reports the nearest surface any of them lands on. Ground planes
/// are skipped — looking down at the flags you stand on is not a close-up.
#[cfg(test)]
pub(crate) fn standoff(
    index: u8,
    view: &super::super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> f32 {
    let camera = staged_view(index, view, dot_w, dot_h);
    let scene = super::scene::interior_scene(index);
    centre_reach(&scene, &camera, dot_w, dot_h)
}

/// The nearest non-ground surface under the central 24% of the frame — the
/// instrument behind [`standoff`], on any mesh and camera. The adventure
/// regions' roomed stages (`region::standoff`) measure themselves with it.
#[cfg(test)]
pub(crate) fn centre_reach(scene: &Mesh, camera: &View3, dot_w: usize, dot_h: usize) -> f32 {
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

/// Möller–Trumbore, double-sided (the whole renderer is).
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
#[path = "../../../../../tests/cockpit/world_viz/world3d__interior__tests.rs"]
mod tests;
