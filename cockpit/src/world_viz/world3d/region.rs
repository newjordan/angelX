//! Adventure-region staging (Zelda overworld Z3): the camera marks, the fog
//! and the ride seam for the five places the quest goes when it leaves Castle
//! Town.
//!
//! `arch::regions` builds the *stages*; this file decides where the camera
//! stands in them. It is the exact counterpart of the vista staging in
//! `world3d.rs` (which owns the castle court) and of `interior.rs` (which owns
//! the eight rooms) — same screen-fraction framing, same solve, same rails.
//!
//! # Where the camera stands
//! Each region carries a table of marks, one per **waypoint** of the hero's
//! walk through it. The waypoint is the same number the authored navigation walks the
//! hero along (Z2): the loop's iteration count, modulo the region's waypoint
//! count. So `v` toggles between the map and the mesh world without the scene
//! moving — both views are looking at the same place in the same region.
//!
//! [`waypoint`] is the camera index — the region's own waypoint derived from
//! `Quest.iteration`, which is the same number the authored walk indexes its
//! legs by. [`hero_station`] is the other half of the handshake: it asks the
//! shared navigation module where the hero actually stands, and for a room-locked
//! region the leg the map is walking always ends in the chamber [`waypoint`]
//! stages. That is what makes `v` a camera change and not a teleport.
//!
//! # What danger does
//! Stall depth is "how far can you see". It pulls the night haze in
//! ([`fog_distance`]) and it guts the torches (`arch::regions`), which is the
//! same thing Z2's tileset does to its bank-4 pixels when a loop stalls.
//!
//! # Determinism
//! Every mesh is built once per `(region, danger, chest count)` behind
//! [`super::scene::region_scene`]'s fixed `OnceLock` array; the camera is a
//! pure function of that table plus the `RayView` the ride already built. All
//! of it is state `cinematic_key` hashes (`cinematics::hash_quest_state`), so
//! a region frame can never be served stale and can never be served fresh
//! twice with different bytes.
#![allow(dead_code)]

use super::super::Quest;
use super::super::adventure::Region;
use super::arch::{self, Stage};
use super::math::v3;
use super::raster::{self, Firelight, View3};
use super::scene::SCENE_SEED;
#[cfg(test)]
use super::wrap_pi;
use super::{EYE_RELIEF, PARALLAX, VERT_FOV_MAX};

/// How far the operator's pan may swing a staged region camera off its mark,
/// in radians. Comfortably past the cockpit's own yaw clamp (±π/6) so ordinary
/// looking-around is linear, and only a wild input gets bent.
const SWING_LIMIT: f32 = 1.20;

/// How a mark's vertical framing is solved — and, with it, which set of floors
/// the frame is judged by.
///
/// This is the Z3b split. Outdoors the composition is about the **horizon**:
/// where the sky meets the ground decides the picture, and the world vista law
/// (sky ≥ 20 %, mass ≤ 50 %, ground ≥ 8 %) is the judge. Indoors there is no
/// horizon at all — a room is judged the way `interior.rs`'s eight staged rooms
/// are, on ink bands, a lit floor band and a light in frame — so the mark aims
/// its identity piece at a point *down* the frame instead.
///
/// The first Z3 pass forced the Mines and the Dragon Keep to satisfy the
/// outdoor floor by taking their roofs off, and the dumps came back reading as
/// a brick gate under stars and as the castle court with fires in it. The
/// outdoor floor was right; applying it to an interior was not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Framing {
    /// Outdoors: the horizon lands `horizon` down the frame (0 = top).
    Vista { horizon: f32 },
    /// Indoors: the identity piece, at world height `anchor_z`, lands
    /// `frame_y` down the frame. Same solve `interior::staged_view` uses.
    Room { anchor_z: f32, frame_y: f32 },
}

impl Framing {
    pub(crate) fn is_room(self) -> bool {
        matches!(self, Framing::Room { .. })
    }
}

/// A staged camera mark inside a region.
///
/// `eye` and `anchor` are stage-tile positions: where the camera stands and
/// what it looks at. `frame_x` puts the anchor on a third across the frame;
/// `framing` solves the vertical (horizon outdoors, identity piece indoors).
/// `eye_h` is the eye above the stage ground, `lens` narrows the field from
/// the ride's own, and `hero` names the shot for the dump sheets and the
/// failure messages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RegionVantage {
    pub(crate) eye: (f32, f32),
    pub(crate) anchor: (f32, f32),
    pub(crate) frame_x: f32,
    pub(crate) framing: Framing,
    pub(crate) eye_h: f32,
    pub(crate) lens: f32,
    pub(crate) hero: &'static str,
}

/// True when a stage is an interior — roofed, torch-lit, judged by the
/// interior floors rather than the outdoor vista floors.
pub(crate) fn is_interior(stage: Stage) -> bool {
    matches!(stage, Stage::Mines | Stage::DragonKeep)
}

/// Which `Region` has a 3D stage of its own. `CastleTown` is `None`: it keeps
/// the shipped court path, untouched.
pub(crate) fn stage_for(region: Region) -> Option<Stage> {
    match region {
        Region::CastleTown => None,
        Region::TheMines => Some(Stage::Mines),
        Region::DarkForest => Some(Stage::DarkForest),
        Region::Swamp => Some(Stage::Swamp),
        Region::DragonKeep => Some(Stage::DragonKeep),
        Region::Homecoming => Some(Stage::Homecoming),
    }
}

// ── the marks ─────────────────────────────────────────────────────────────

/// Nine stations down the gallery, walked from the near end to the far one.
///
/// Every one looks **along** the tunnel, because that is the whole shot: the
/// timber frames stepping away, the rail converging, and a black hole at the
/// end that the torches never reach. Depth is the composition; there is
/// nothing else in a mine.
const MINE_MARKS: [RegionVantage; 9] = [
    mine_mark(-17.0, 0.42, "the gallery mouth, timbering stepping away"),
    mine_mark(-13.5, 0.58, "the first frames and the rail running out"),
    mine_mark(-10.0, 0.42, "ore in the west face, torch on the prop"),
    mine_mark(-6.5, 0.58, "the cart standing on the rails"),
    mine_mark(-3.0, 0.42, "past the cart into the deep gallery"),
    mine_mark(0.5, 0.58, "the middle working, rubble at the feet"),
    mine_mark(4.0, 0.42, "the east frames under the vault"),
    mine_mark(7.5, 0.58, "the last lit frame before the dark"),
    mine_mark(11.0, 0.42, "the far face, where the torches end"),
];

/// One station: back a little off the rail, aimed at a point 13 tiles down the
/// gallery at chest height, so the vanishing point of the timbering sits on a
/// third and the vault crosses the top of the frame.
const fn mine_mark(x: f32, frame_x: f32, hero: &'static str) -> RegionVantage {
    RegionVantage {
        eye: (x, 1.05),
        anchor: (x + 13.0, -0.35),
        frame_x,
        framing: Framing::Room {
            anchor_z: 1.45,
            frame_y: 0.52,
        },
        eye_h: 1.55,
        lens: 0.95,
        hero,
    }
}

/// Four marks around the clearing, all standing near its centre and looking
/// outward past the framing giants at the treeline.
///
/// The eyes sit within three tiles of the origin on purpose. The giants ring
/// the clearing at `FOREST_FRAME_R` (7.6 tiles), so from the middle two or
/// three of them always land in the left and right thirds with their crowns
/// across the top — which is the difference between "in the woods" and "a
/// meadow with a hedge at the far end". It also keeps every trunk 4.6 tiles
/// off the eye, so the standoff rail holds untouched.
const FOREST_MARKS: [RegionVantage; 4] = [
    forest_mark(
        -2.4,
        -1.8,
        8.0,
        11.0,
        0.40,
        "the north treeline through the giants",
    ),
    forest_mark(-2.6, 1.4, 11.0, 3.0, 0.60, "the east wall of trunks"),
    forest_mark(
        1.9,
        -2.0,
        -9.0,
        8.0,
        0.38,
        "the north-west stumps under the moon",
    ),
    forest_mark(0.4, -2.8, 1.5, 12.0, 0.58, "the track leaving the clearing"),
];

const fn forest_mark(
    ex: f32,
    ey: f32,
    ax: f32,
    ay: f32,
    frame_x: f32,
    hero: &'static str,
) -> RegionVantage {
    RegionVantage {
        eye: (ex, ey),
        anchor: (ax, ay),
        frame_x,
        framing: Framing::Vista { horizon: 0.64 },
        eye_h: 1.75,
        lens: 0.95,
        hero,
    }
}

/// Four marks on the open water, all standing near the middle of the pan.
///
/// Pulled in from the old ring for the same reason the forest's were: the
/// drowned wood now starts at `SWAMP_CLEAR_R` (6.6 tiles), so a mark near the
/// origin has trunks and reeds at 5–9 tiles — near field — while still holding
/// its four tiles of standoff.
const SWAMP_MARKS: [RegionVantage; 4] = [
    swamp_mark(
        -1.6,
        -1.6,
        7.0,
        8.0,
        0.40,
        "drowned trunks over the mist band",
    ),
    swamp_mark(
        0.0,
        -2.0,
        -5.0,
        9.0,
        0.60,
        "wisps and their reflections north",
    ),
    swamp_mark(
        1.8,
        -1.2,
        -7.5,
        6.5,
        0.38,
        "the north-west reeds and dead wood",
    ),
    swamp_mark(
        -1.9,
        0.8,
        8.0,
        4.0,
        0.60,
        "the east margin, mist to the horizon",
    ),
];

const fn swamp_mark(
    ex: f32,
    ey: f32,
    ax: f32,
    ay: f32,
    frame_x: f32,
    hero: &'static str,
) -> RegionVantage {
    RegionVantage {
        eye: (ex, ey),
        anchor: (ax, ay),
        frame_x,
        framing: Framing::Vista { horizon: 0.62 },
        eye_h: 1.62,
        lens: 0.95,
        hero,
    }
}

/// Three marks up the great hall, all from the door end, all about the dragon.
///
/// The dragon sits on the dais at x ≈ 11.9 and spans about seven tiles across
/// the wings; from nine to thirteen tiles back that is a third of the frame
/// width, which is what "there is a dragon in this hall" costs. The colonnade
/// and the runner do the leading-in.
const KEEP_MARKS: [RegionVantage; 3] = [
    RegionVantage {
        eye: (-2.6, 1.5),
        anchor: (11.9, 0.2),
        frame_x: 0.57,
        framing: Framing::Room {
            anchor_z: 3.30,
            frame_y: 0.44,
        },
        eye_h: 1.72,
        lens: 0.92,
        hero: "the dragon on the dais at the head of the hall",
    },
    RegionVantage {
        eye: (-6.4, -2.4),
        anchor: (11.9, 0.6),
        frame_x: 0.40,
        framing: Framing::Room {
            anchor_z: 2.90,
            frame_y: 0.48,
        },
        eye_h: 1.66,
        lens: 0.90,
        hero: "the colonnade and the braziers down to the dragon",
    },
    RegionVantage {
        eye: (-10.2, 0.6),
        anchor: (11.6, 0.0),
        frame_x: 0.56,
        framing: Framing::Room {
            anchor_z: 3.10,
            frame_y: 0.42,
        },
        eye_h: 1.80,
        lens: 0.86,
        hero: "the whole hall from the door, the wyrm at the end of it",
    },
];

/// Three marks on the walk home. These are the castle court's own
/// compositions, re-solved for the wide ride pane: Homecoming *is* the court,
/// and the one thing the stage adds is the lit road, which all three look
/// along.
const HOME_MARKS: [RegionVantage; 3] = [
    RegionVantage {
        eye: (8.0, -25.0),
        anchor: (0.0, -11.0),
        frame_x: 0.40,
        framing: Framing::Vista { horizon: 0.66 },
        eye_h: 1.90,
        lens: 1.00,
        hero: "the lit gate road from the river bank",
    },
    RegionVantage {
        eye: (-15.0, -24.0),
        anchor: (0.0, -11.0),
        frame_x: 0.64,
        framing: Framing::Vista { horizon: 0.67 },
        eye_h: 1.90,
        lens: 1.00,
        hero: "the lamp-lit road up to the gate, from the west bank",
    },
    RegionVantage {
        eye: (-22.0, -16.0),
        anchor: (0.0, 1.5),
        frame_x: 0.60,
        framing: Framing::Vista { horizon: 0.62 },
        eye_h: 2.40,
        lens: 1.00,
        hero: "home: the west curtain and the keep behind its drum",
    },
];

/// The marks for a stage, in waypoint order.
pub(crate) fn marks(stage: Stage) -> &'static [RegionVantage] {
    match stage {
        Stage::Mines => &MINE_MARKS,
        Stage::DarkForest => &FOREST_MARKS,
        Stage::Swamp => &SWAMP_MARKS,
        Stage::DragonKeep => &KEEP_MARKS,
        Stage::Homecoming => &HOME_MARKS,
    }
}

/// How many waypoints a region's walk has.
pub(crate) fn waypoint_count(region: Region) -> usize {
    stage_for(region).map_or(1, |stage| marks(stage).len())
}

/// Which waypoint of its region the hero is walking toward — the camera index.
///
/// The marks are authored one per chamber, so this stays the loop's own
/// iteration count modulo the region's waypoint count: exactly the number the
/// navigation module indexes its legs by. [`hero_station`] is the check that the
/// two agree; `region_walk::hero_station` is where the map answers from.
pub(crate) fn waypoint(quest: &Quest) -> usize {
    let count = waypoint_count(quest.region());
    quest.iteration() % count.max(1)
}

/// The authored name of the shot the hero stands on — the debugging window
/// `/world quest` prints. `CastleTown` has no staged marks, so it has no
/// waypoint name either.
pub(crate) fn waypoint_label(region: Region, waypoint: usize) -> Option<&'static str> {
    let table = marks(stage_for(region)?);
    table
        .get(waypoint.min(table.len().saturating_sub(1)))
        .map(|mark| mark.hero)
}

/// Where the hero stands in stage tiles — the eye of the waypoint's mark.
pub(crate) fn hero_pos(stage: Stage, waypoint: usize) -> (f32, f32) {
    let table = marks(stage);
    table[waypoint.min(table.len() - 1)].eye
}

/// The station the authored walk has the hero standing in, asked of the
/// authored region map rather than of the loop clock.
///
/// This is the Z2/Z3 handshake: the marks stay authored per chamber and
/// [`waypoint`] stays the camera index, but a room-locked region can now be
/// asked where its hero actually is, and the answer has to agree with the
/// camera index or the view toggle would move the hero. `None` for a region
/// that scrolls or has no authored map.
pub(crate) fn hero_station(world: &crate::world_viz::World) -> Option<usize> {
    crate::world_viz::region_walk::hero_station(world)
}

// ── the haze ──────────────────────────────────────────────────────────────

/// How far you can see in a region at a given stall depth, in tiles.
///
/// The court's own [`raster::FOG_DISTANCE`] (52) is a clear night over open
/// ground. None of these places is that. The falls are per-region because the
/// air is: a mine gallery is dusty, a forest is closed, a swamp is standing
/// mist, and the walk home is the clear night again.
pub(crate) fn fog_distance(stage: Stage, danger: u8) -> f32 {
    let danger = danger.min(3) as f32;
    match stage {
        // A gallery you can see the end of is not a gallery. The near end is
        // torch-lit and the far end is black, and danger walks the black end
        // closer until the stall is a wall of dark two frames away.
        Stage::Mines => 32.0 - 6.0 * danger,
        Stage::DarkForest => 30.0 - 4.5 * danger,
        Stage::Swamp => 28.0 - 4.0 * danger,
        Stage::DragonKeep => 34.0 - 5.0 * danger,
        Stage::Homecoming => raster::FOG_DISTANCE - 4.0 * danger,
    }
}

// ── the ride seam ─────────────────────────────────────────────────────────

/// The 3D frame for a region, staged on the waypoint the hero stands on.
pub(crate) fn render_stage_frame(
    stage: Stage,
    quest: &Quest,
    map: &super::super::raycast::RayMap,
    view: &super::super::raycast::RayView,
    yaw: f32,
    bucket: u64,
    dimensions: (u32, u32),
) -> image::RgbaImage {
    let (dot_w, dot_h) = dimensions;
    let danger = quest.danger().level();
    let chests = quest.treasures().min(arch::MAX_CHESTS as u32) as u8;
    raster::render_scene_fogged(
        &super::scene::region_scene(stage, danger, chests, wisp_phase(stage, bucket)),
        &staged_view(
            stage,
            waypoint(quest),
            map,
            view,
            yaw,
            dot_w as usize,
            dot_h as usize,
        ),
        dot_w as usize,
        dot_h as usize,
        Firelight::STEADY,
        fog_distance(stage, danger),
    )
}

/// How many world ticks one step of the Swamp's wisp drift lasts.
///
/// Sixteen puts a step at 0.4 s (the world ticks every 25 ms) and the whole
/// four-step loop at 1.6 s — the rate a marsh light actually wanders, and slow
/// enough that the ride's memo still holds for most frames.
pub(crate) const WISP_TICKS: u64 = 16;

/// Which step of the drift a tick bucket is on. Zero for every stage that does
/// not move, so a static stage can never be re-keyed by the clock.
/// The wisp phase the live world is on: the region's stage at `tick`, or 0
/// wherever nothing drifts. `cinematic_key` hashes this so a Swamp frame is
/// re-keyed every `WISP_TICKS` and a static stage never is.
pub(crate) fn wisp_phase_at(quest: &Quest, tick: u64) -> u8 {
    match stage_for(quest.region()) {
        Some(stage) => wisp_phase(stage, tick / WISP_TICKS),
        None => 0,
    }
}

pub(crate) fn wisp_phase(stage: Stage, bucket: u64) -> u8 {
    if arch::stage_animates(stage) {
        (bucket % arch::WISP_PHASES as u64) as u8
    } else {
        0
    }
}

/// The staged camera in a region: the authored mark, panned and pitched by the
/// operator.
///
/// The arithmetic is [`super::view3_from_ray`]'s settled branch with the road
/// sweep taken out — there is no approach to a region, the hero is simply
/// there. Framing is expressed in screen fractions and solved per aspect, so
/// one table holds up from a 96 × 72 ride pane to a 200 × 304 vista plate.
fn staged_view(
    stage: Stage,
    waypoint: usize,
    _map: &super::super::raycast::RayMap,
    view: &super::super::raycast::RayView,
    yaw: f32,
    dot_w: usize,
    dot_h: usize,
) -> View3 {
    let table = marks(stage);
    let vantage = &table[waypoint.min(table.len() - 1)];

    let aspect = dot_h.max(1) as f32 / dot_w.max(1) as f32;
    let mut tan_h = (view.fov_rad.clamp(0.70, 1.40) * 0.5).tan();
    let tan_v_max = (VERT_FOV_MAX * 0.5).tan();
    if tan_h * aspect > tan_v_max {
        tan_h = tan_v_max / aspect.max(1e-3);
    }
    tan_h *= vantage.lens.clamp(0.35, 1.0);
    let tan_v = tan_h * aspect;

    // The operator's own pan — and *only* that.
    //
    // The court measures its swing against the ride's bearing to a lit
    // landmark billboard (`world3d::operator_swing`), which works because the
    // court is what the billboards are in. A region is somewhere else: the
    // town bearing is unrelated to where the region camera points, and feeding
    // it in swung every staged mark up to `SWING_LIMIT` off its authored
    // heading — a mark can be tuned to the vista floors and still render a
    // wall, which is exactly the failure the law exists to stop. So a region
    // takes the operator's yaw offset straight from the ride seam.
    //
    // Soft limit, not a clamp: a hard clamp makes two different operator
    // headings render the identical frame once both saturate, which is a
    // camera that has stopped answering the arrow keys. `tanh` bends without
    // ever flattening.
    let swing = SWING_LIMIT * (yaw / SWING_LIMIT).tanh();

    let to_anchor = (vantage.anchor.1 - vantage.eye.1).atan2(vantage.anchor.0 - vantage.eye.0);
    let heading = to_anchor - (tan_h * (2.0 * vantage.frame_x - 1.0)).atan() + swing;

    // Sub-tile parallax, so the stage breathes with the operator instead of
    // standing dead still — and it rides the **pan**, not the ride's own
    // sub-tile drift around the town avatar. Same reason the swing does: that
    // drift is a fact about where the knight stands in Castle Town, and
    // feeding it in walks a staged mark up to `PARALLAX` tiles off the mark it
    // was measured on. At yaw zero the camera is exactly where the table put
    // it, which is what makes the authoring rail and the live seam the same
    // measurement.
    let (sin_h, cos_h) = heading.sin_cos();
    let dolly = (yaw / SWING_LIMIT).clamp(-1.0, 1.0) * PARALLAX;
    let eye = (vantage.eye.0 - sin_h * dolly, vantage.eye.1 + cos_h * dolly);

    // Vertical framing. Outdoors it is the horizon that has to land where the
    // table says; indoors there is no horizon, so it is the identity piece —
    // the same solve `interior::staged_view` runs, and the reason the two
    // families of region can share one table and one camera.
    let level = match vantage.framing {
        Framing::Vista { horizon } => (tan_v * (2.0 * horizon - 1.0)).atan(),
        Framing::Room { anchor_z, frame_y } => {
            let reach = ((vantage.anchor.0 - eye.0).powi(2) + (vantage.anchor.1 - eye.1).powi(2))
                .sqrt()
                .max(1e-3);
            let elevation = ((anchor_z - vantage.eye_h) / reach).atan();
            elevation - (tan_v * (1.0 - 2.0 * frame_y)).atan()
        }
    };
    let pitch = (level + view.bob * 0.03).clamp(-0.90, 0.90);

    let extent = arch::stage_extent(stage);
    // Interiors have a flat cut floor at z = 0 (`stage_terrain` gives them no
    // relief), so this is zero for them by construction rather than by a
    // special case.
    let ground = arch::stage_ground_z(stage, SCENE_SEED, eye.0, eye.1);
    View3 {
        pos: v3(
            eye.0.clamp(-extent, extent),
            eye.1.clamp(-extent, extent),
            ground + vantage.eye_h + view.eye_h.clamp(0.0, 1.0) * EYE_RELIEF,
        ),
        heading_rad: heading,
        pitch,
        fov_rad: 2.0 * tan_h.atan(),
    }
}

// ── test windows ──────────────────────────────────────────────────────────

/// The settled camera a mark produces — [`staged_view`] with the operator's
/// hands off. Same arithmetic, not a copy of the intent:
/// `a_probe_camera_matches_the_staged_ride` fails loudly if the two drift.
#[cfg(test)]
pub(crate) fn settled_camera(stage: Stage, waypoint: usize, dot_w: usize, dot_h: usize) -> View3 {
    let table = marks(stage);
    let vantage = &table[waypoint.min(table.len() - 1)];
    let aspect = dot_h.max(1) as f32 / dot_w.max(1) as f32;
    let mut tan_h = (1.05_f32 * 0.5).tan();
    let tan_v_max = (VERT_FOV_MAX * 0.5).tan();
    if tan_h * aspect > tan_v_max {
        tan_h = tan_v_max / aspect.max(1e-3);
    }
    tan_h *= vantage.lens.clamp(0.35, 1.0);
    let tan_v = tan_h * aspect;

    let to_anchor = (vantage.anchor.1 - vantage.eye.1).atan2(vantage.anchor.0 - vantage.eye.0);
    let heading = to_anchor - (tan_h * (2.0 * vantage.frame_x - 1.0)).atan();
    let pitch = match vantage.framing {
        Framing::Vista { horizon } => (tan_v * (2.0 * horizon - 1.0)).atan(),
        Framing::Room { anchor_z, frame_y } => {
            let reach = ((vantage.anchor.0 - vantage.eye.0).powi(2)
                + (vantage.anchor.1 - vantage.eye.1).powi(2))
            .sqrt()
            .max(1e-3);
            let elevation = ((anchor_z - vantage.eye_h) / reach).atan();
            elevation - (tan_v * (1.0 - 2.0 * frame_y)).atan()
        }
    }
    .clamp(-0.90, 0.90);
    let ground = arch::stage_ground_z(stage, SCENE_SEED, vantage.eye.0, vantage.eye.1);
    View3 {
        pos: v3(vantage.eye.0, vantage.eye.1, ground + vantage.eye_h),
        heading_rad: heading,
        pitch,
        fov_rad: 2.0 * tan_h.atan(),
    }
}

/// `(sky, built mass, ground)` for a staged region frame — the vista law's own
/// vocabulary, measured on the geometry the ride would paint.
#[cfg(test)]
pub(crate) fn composition_mix(
    stage: Stage,
    waypoint: usize,
    danger: u8,
    chests: u8,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    raster::composition_mix(
        &super::scene::region_scene(stage, danger, chests, 0),
        &settled_camera(stage, waypoint, dot_w, dot_h),
        dot_w,
        dot_h,
    )
}

/// How far away the nearest thing the picture is *about* is — the
/// anti-close-up rail, measured the way each family of region needs.
///
/// **Outdoors** it is a bearing sweep for the nearest built surface inside the
/// field, exactly as `world3d::standoff` does it for the court. Ground planes
/// are skipped for the court's own stated reason — standing on the road is not
/// standing in a doorway — and so is anything whose whole triangle sits below
/// [`SILL`] above the eye's ground, because a rail sleeper or a lily pad you
/// could step over is not a wall in your face either.
///
/// **Indoors** a bearing sweep is the wrong instrument, and `interior.rs` says
/// why: a room is a box you stand inside, so there is always a rafter within a
/// tile of the eye at the edge of the field, and a nearest-vertex rail either
/// fails every honest interior or gets loosened until it proves nothing.
/// Instead the nine rays of the central 24 % of the frame are shot into the
/// scene and the nearest surface any of them lands on is reported — how far
/// away the thing in the middle of the picture is.
#[cfg(test)]
const SILL: f32 = 0.6;

#[cfg(test)]
pub(crate) fn standoff(
    stage: Stage,
    waypoint: usize,
    danger: u8,
    chests: u8,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    use super::mesh::mat;
    let table = marks(stage);
    let vantage = &table[waypoint.min(table.len() - 1)];
    let camera = settled_camera(stage, waypoint, dot_w, dot_h);
    let scene = super::scene::region_scene(stage, danger, chests, 0);

    if vantage.framing.is_room() {
        let centre = super::interior::centre_reach(&scene, &camera, dot_w, dot_h);
        return (centre, camera.pos.x, camera.pos.y);
    }

    let half_fov = camera.fov_rad * 0.5 * 1.1;
    let sill = camera.pos.z - vantage.eye_h + SILL;
    let mut nearest = (f32::INFINITY, 0.0, 0.0);
    for tri in &scene.tris {
        if matches!(tri.mat, mat::GRASS | mat::PATH | mat::WATER | mat::FLOOR) {
            continue;
        }
        if tri.v.iter().all(|p| p.z < sill) {
            continue;
        }
        for point in tri.v {
            let (dx, dy) = (point.x - camera.pos.x, point.y - camera.pos.y);
            if wrap_pi(dy.atan2(dx) - camera.heading_rad).abs() > half_fov {
                continue;
            }
            let d = (dx * dx + dy * dy).sqrt();
            if d < nearest.0 {
                nearest = (d, point.x, point.y);
            }
        }
    }
    nearest
}

/// `(sky, mass, ground)` for the frame the **live seam** would paint — the
/// authored mark plus whatever drift and pan the ride hands in. The authoring
/// rail is [`composition_mix`]; this is the same measurement taken on the real
/// camera, so a mark cannot pass the table and fail the cockpit.
#[cfg(test)]
pub(crate) fn seam_composition(
    quest: &Quest,
    map: &super::super::raycast::RayMap,
    view: &super::super::raycast::RayView,
    yaw: f32,
    dot_w: usize,
    dot_h: usize,
) -> Option<(f32, f32, f32)> {
    let stage = stage_for(quest.region())?;
    let danger = quest.danger().level();
    let chests = quest.treasures().min(arch::MAX_CHESTS as u32) as u8;
    Some(raster::composition_mix(
        &super::scene::region_scene(stage, danger, chests, 0),
        &staged_view(stage, waypoint(quest), map, view, yaw, dot_w, dot_h),
        dot_w,
        dot_h,
    ))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/world3d__region__tests.rs"]
mod tests;
