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

const SWING_LIMIT: f32 = 1.20;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Framing {
    Vista { horizon: f32 },
    Room { anchor_z: f32, frame_y: f32 },
}

impl Framing {
    pub fn is_room(self) -> bool {
        matches!(self, Framing::Room { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionVantage {
    pub eye: (f32, f32),
    pub anchor: (f32, f32),
    pub frame_x: f32,
    pub framing: Framing,
    pub eye_h: f32,
    pub lens: f32,
    pub hero: &'static str,
}

pub fn is_interior(stage: Stage) -> bool {
    matches!(stage, Stage::Mines | Stage::DragonKeep)
}

pub fn stage_for(region: Region) -> Option<Stage> {
    match region {
        Region::CastleTown => None,
        Region::TheMines => Some(Stage::Mines),
        Region::DarkForest => Some(Stage::DarkForest),
        Region::Swamp => Some(Stage::Swamp),
        Region::DragonKeep => Some(Stage::DragonKeep),
        Region::Homecoming => Some(Stage::Homecoming),
    }
}

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

pub fn marks(stage: Stage) -> &'static [RegionVantage] {
    match stage {
        Stage::Mines => &MINE_MARKS,
        Stage::DarkForest => &FOREST_MARKS,
        Stage::Swamp => &SWAMP_MARKS,
        Stage::DragonKeep => &KEEP_MARKS,
        Stage::Homecoming => &HOME_MARKS,
    }
}

pub fn waypoint_count(region: Region) -> usize {
    stage_for(region).map_or(1, |stage| marks(stage).len())
}

pub fn waypoint(quest: &Quest) -> usize {
    let count = waypoint_count(quest.region());
    quest.iteration() % count.max(1)
}

pub fn waypoint_label(region: Region, waypoint: usize) -> Option<&'static str> {
    let table = marks(stage_for(region)?);
    table
        .get(waypoint.min(table.len().saturating_sub(1)))
        .map(|mark| mark.hero)
}

pub fn hero_pos(stage: Stage, waypoint: usize) -> (f32, f32) {
    let table = marks(stage);
    table[waypoint.min(table.len() - 1)].eye
}

pub fn hero_station(world: &crate::world_viz::World) -> Option<usize> {
    crate::world_viz::region_walk::hero_station(world)
}

pub fn fog_distance(stage: Stage, danger: u8) -> f32 {
    let danger = danger.min(3) as f32;
    match stage {
        Stage::Mines => 32.0 - 6.0 * danger,
        Stage::DarkForest => 30.0 - 4.5 * danger,
        Stage::Swamp => 28.0 - 4.0 * danger,
        Stage::DragonKeep => 34.0 - 5.0 * danger,
        Stage::Homecoming => raster::FOG_DISTANCE - 4.0 * danger,
    }
}

pub fn render_stage_frame(
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

pub const WISP_TICKS: u64 = 16;

pub fn wisp_phase_at(quest: &Quest, tick: u64) -> u8 {
    match stage_for(quest.region()) {
        Some(stage) => wisp_phase(stage, tick / WISP_TICKS),
        None => 0,
    }
}

pub fn wisp_phase(stage: Stage, bucket: u64) -> u8 {
    if arch::stage_animates(stage) {
        (bucket % arch::WISP_PHASES as u64) as u8
    } else {
        0
    }
}

pub(super) fn staged_view(
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

    let swing = SWING_LIMIT * (yaw / SWING_LIMIT).tanh();

    let to_anchor = (vantage.anchor.1 - vantage.eye.1).atan2(vantage.anchor.0 - vantage.eye.0);
    let heading = to_anchor - (tan_h * (2.0 * vantage.frame_x - 1.0)).atan() + swing;

    let (sin_h, cos_h) = heading.sin_cos();
    let dolly = (yaw / SWING_LIMIT).clamp(-1.0, 1.0) * PARALLAX;
    let eye = (vantage.eye.0 - sin_h * dolly, vantage.eye.1 + cos_h * dolly);

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

#[cfg(test)]
pub fn settled_camera(stage: Stage, waypoint: usize, dot_w: usize, dot_h: usize) -> View3 {
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

#[cfg(test)]
pub fn composition_mix(
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

#[cfg(test)]
const SILL: f32 = 0.6;

#[cfg(test)]
pub fn standoff(
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

#[cfg(test)]
pub fn seam_composition(
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
#[path = "../../../tests/world_viz/world3d__region__tests.rs"]
mod tests;
