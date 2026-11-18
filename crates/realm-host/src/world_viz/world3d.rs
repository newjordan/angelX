//! world3d — true-3D mesh renderer for the scryglass world pane.
//!
//! Replaces the Wolfenstein-style grid raycaster with real geometry: castles,
//! towers, libraries, courtyards, staged as painting-worthy vistas (world vista
//! law). Contract + ownership map: docs/plans/world3d-spec.md.
//!
//! Coordinates: x/y = ground plane in RayMap tile units, +z = up.
//! Triangles are double-sided; the shader flips the flat normal toward the
//! camera. Everything here must be deterministic — byte-identical frames for
//! identical inputs.
//!
//! The seam is `ride.rs::scryglass_frame_paced`: Dotmax paints outdoors and
//! explicit entry paints the retained room plate. The paced cache, rider overlay,
//! Bayer screen and braille bridge retain their established contracts.
#![allow(dead_code)]

pub mod arch;
pub mod chivalry;
pub mod expedition;
pub mod instruments;
pub mod interior;
pub use angel_delve::render::world3d::{math, mesh, raster};
pub mod props;
pub mod region;
pub mod scene;

use math::v3;
use raster::View3;

const EYE_RELIEF: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorldView {
    Mesh3d,
}

impl WorldView {
    pub fn label(self) -> &'static str {
        "Dotmax 3D"
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "3d" | "mesh" | "mesh3d" | "dotmax" | "journey" | "living" | "top" | "topdown"
            | "top-down" | "2d" | "raycast" | "ray" | "ambient" | "art" => Some(Self::Mesh3d),
            _ => None,
        }
    }
}

const DEFAULT_VIEW: WorldView = WorldView::Mesh3d;

pub fn current() -> WorldView {
    DEFAULT_VIEW
}

pub fn set(_view: WorldView) {}

pub fn toggle() -> bool {
    true
}

pub fn cycle() -> WorldView {
    current()
}

pub fn status_line() -> String {
    if super::overworld::map_enabled() {
        format!(
            "world renderer: overworld map on the Realm, {} on Explore; Enter opens room plates, Leave returns to the world",
            current().label()
        )
    } else {
        format!(
            "world renderer: {} outdoors; Enter opens room plates, Leave returns to the world",
            current().label()
        )
    }
}

#[cfg(any(test, feature = "testing"))]
pub struct RendererPin;

#[cfg(any(test, feature = "testing"))]
pub fn pin(_view: WorldView) -> RendererPin {
    RendererPin
}

#[cfg(any(test, feature = "testing"))]
pub fn pin_world3d() -> RendererPin {
    pin(WorldView::Mesh3d)
}

pub(super) use angel_delve::render::world3d::MOON_BEARING;

pub fn render_ride_frame(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: u32,
    dot_h: u32,
) -> image::RgbaImage {
    raster::render_scene(
        &scene::scene_for(scene::SceneKey::COURT),
        &view3_from_ray(map, view, dot_w as usize, dot_h as usize),
        dot_w as usize,
        dot_h as usize,
    )
}

pub fn render_region_frame(
    quest: &super::Quest,
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    yaw: f32,
    bucket: u64,
    dot_w: u32,
    dot_h: u32,
) -> image::RgbaImage {
    match region::stage_for(quest.region()) {
        None => render_ride_frame(map, view, dot_w, dot_h),
        Some(stage) => {
            region::render_stage_frame(stage, quest, map, view, yaw, bucket, (dot_w, dot_h))
        }
    }
}

#[cfg(test)]
pub fn composition_mix(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    raster::composition_mix(
        &scene::scene_for(scene::SceneKey::COURT),
        &view3_from_ray(map, view, dot_w, dot_h),
        dot_w,
        dot_h,
    )
}

#[cfg(test)]
pub fn staged_eye(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32, f32, f32, f32) {
    let camera = view3_from_ray(map, view, dot_w, dot_h);
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
pub fn staged_progress(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
) -> (i32, f32, f32) {
    let mark = target_mark(map);
    let distance = mark.map_or(f32::INFINITY, |(_, x, y)| {
        ((x - view.x).powi(2) + (y - view.y).powi(2)).sqrt()
    });
    let travel = ((distance - ARRIVE_DIST) / (SWEEP_DIST - ARRIVE_DIST)).clamp(0.0, 1.0);
    (
        mark.map_or(-1, |(index, _, _)| index as i32),
        distance,
        smoothstep(1.0 - travel),
    )
}

#[cfg(test)]
pub fn standoff(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> (f32, f32, f32) {
    let camera = view3_from_ray(map, view, dot_w, dot_h);
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let half_fov = camera.fov_rad * 0.5 * 1.1;
    let mut nearest = (f32::INFINITY, 0.0, 0.0);
    for tri in &scene.tris {
        if matches!(
            tri.mat,
            mesh::mat::GRASS | mesh::mat::PATH | mesh::mat::WATER | mesh::mat::FLOOR
        ) {
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

struct Vantage {
    eye: (f32, f32),
    anchor: (f32, f32),
    frame_x: f32,
    horizon: f32,
    eye_h: f32,
    lens: f32,
    hero: &'static str,
}

const VANTAGES: [Vantage; 8] = [
    Vantage {
        eye: (-22.0, -16.0),
        anchor: (0.0, 1.5),
        frame_x: 0.60,
        horizon: 0.66,
        eye_h: 2.4,
        lens: 1.00,
        hero: "the west curtain and the keep behind its drum tower",
    },
    Vantage {
        eye: (8.0, -25.0),
        anchor: (0.0, -11.0),
        frame_x: 0.40,
        horizon: 0.63,
        eye_h: 1.9,
        lens: 0.80,
        hero: "the gatehouse at the head of the road, over the bridge",
    },
    Vantage {
        eye: (-22.5, 16.5),
        anchor: (-11.0, 11.0),
        frame_x: 0.36,
        horizon: 0.68,
        eye_h: 1.8,
        lens: 0.85,
        hero: "the rookery drum's moonlit band, birds on the rim",
    },
    Vantage {
        eye: (-14.0, -17.0),
        anchor: (-20.0, 6.0),
        frame_x: 0.42,
        horizon: 0.66,
        eye_h: 1.9,
        lens: 1.00,
        hero: "the library's lit flank and crossing lantern across the west meadow",
    },
    Vantage {
        eye: (16.4, -24.0),
        anchor: (10.8, -16.4),
        frame_x: 0.58,
        horizon: 0.64,
        eye_h: 1.7,
        lens: 1.00,
        hero: "the forge mouth burning under the castle's south-east drum",
    },
    Vantage {
        eye: (26.0, -1.0),
        anchor: (17.0, 7.0),
        frame_x: 0.56,
        horizon: 0.66,
        eye_h: 1.7,
        lens: 1.00,
        hero: "the chapel rose window on the east rise",
    },
    Vantage {
        eye: (20.0, -22.0),
        anchor: (0.0, -3.0),
        frame_x: 0.60,
        horizon: 0.66,
        eye_h: 2.8,
        lens: 0.88,
        hero: "the south-east drum tower over the court wall",
    },
    Vantage {
        eye: (25.5, -19.5),
        anchor: (18.5, -11.5),
        frame_x: 0.40,
        horizon: 0.64,
        eye_h: 2.4,
        lens: 0.96,
        hero: "the star-tower's faceted dome against the castle's south-east face",
    },
];

fn road_pose(travel: f32) -> (f32, f32) {
    let travel = travel.clamp(0.0, 1.0);
    let eye = VANTAGES[1].eye;
    (eye.0 + 0.3 * travel, eye.1 - 0.6 * travel)
}
const ROAD_ANCHOR: (f32, f32) = (0.0, -11.0);
const ROAD_FRAME_X: f32 = 0.42;
const ROAD_HORIZON: f32 = 0.62;
const ROAD_EYE_H: f32 = 1.9;

const SWEEP_DIST: f32 = 20.0;
const ARRIVE_DIST: f32 = 7.0;

const SWING_LIMIT: f32 = 1.80;

const PARALLAX: f32 = 0.8;

const VERT_FOV_MAX: f32 = 1.12;

fn view3_from_ray(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    dot_w: usize,
    dot_h: usize,
) -> View3 {
    let mark = target_mark(map);
    let building = mark.map_or(0, |(index, _, _)| index as usize);
    let vantage = &VANTAGES[building.min(VANTAGES.len() - 1)];

    let distance = mark.map_or(f32::INFINITY, |(_, x, y)| {
        ((x - view.x).powi(2) + (y - view.y).powi(2)).sqrt()
    });
    let travel = ((distance - ARRIVE_DIST) / (SWEEP_DIST - ARRIVE_DIST)).clamp(0.0, 1.0);
    let arrival = smoothstep(1.0 - travel);

    let road_eye = road_pose(travel);
    let stage = if arrival >= 0.85 { 1.0 } else { 0.0 };
    let eye = (
        lerp(road_eye.0, vantage.eye.0, stage),
        lerp(road_eye.1, vantage.eye.1, stage),
    );
    let anchor = (
        lerp(ROAD_ANCHOR.0, vantage.anchor.0, stage),
        lerp(ROAD_ANCHOR.1, vantage.anchor.1, stage),
    );
    let frame_x = lerp(ROAD_FRAME_X, vantage.frame_x, stage);
    let horizon = lerp(ROAD_HORIZON, vantage.horizon, stage);
    let eye_h = lerp(ROAD_EYE_H, vantage.eye_h, stage);
    let lens = lerp(VANTAGES[1].lens, vantage.lens, stage);

    let aspect = dot_h.max(1) as f32 / dot_w.max(1) as f32;
    let mut tan_h = (view.fov_rad.clamp(0.70, 1.40) * 0.5).tan();
    let tan_v_max = (VERT_FOV_MAX * 0.5).tan();
    if tan_h * aspect > tan_v_max {
        tan_h = tan_v_max / aspect.max(1e-3);
    }
    tan_h *= lens.clamp(0.35, 1.0);
    let tan_v = tan_h * aspect;

    let swing = if arrival < 1.0 {
        view.look_yaw
    } else {
        operator_swing(map, view, arrival)
    };

    let to_anchor = (anchor.1 - eye.1).atan2(anchor.0 - eye.0);
    let heading = to_anchor - (tan_h * (2.0 * frame_x - 1.0)).atan() + swing;

    let drift_x = (view.x - (map.width as f32 - 1.0) * 0.5).clamp(-0.5, 0.5);
    let drift_y = (view.y - (map.height as f32 - 1.0) * 0.5).clamp(-0.5, 0.5);
    let (sin_h, cos_h) = heading.sin_cos();
    let eye = (
        eye.0 + (-sin_h * drift_x + cos_h * drift_y) * PARALLAX,
        eye.1 + (cos_h * drift_x + sin_h * drift_y) * PARALLAX,
    );

    let level = (tan_v * (2.0 * horizon - 1.0)).atan();
    let pitch = (level + view.bob * 0.03).clamp(-0.90, 0.90);

    let ground = arch::court_ground_z(scene::SCENE_SEED, eye.0, eye.1);
    View3 {
        pos: v3(
            eye.0.clamp(-arch::COURT_EXTENT, arch::COURT_EXTENT),
            eye.1.clamp(-arch::COURT_EXTENT, arch::COURT_EXTENT),
            ground + eye_h + view.eye_h.clamp(0.0, 1.0) * EYE_RELIEF,
        ),
        heading_rad: heading,
        pitch,
        fov_rad: 2.0 * tan_h.atan(),
    }
}

pub(super) fn operator_swing(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
    arrival: f32,
) -> f32 {
    match target_mark(map).or_else(|| any_landmark(map, view)) {
        Some((index, x, y)) => {
            let bearing = (y - view.y).atan2(x - view.x);
            let authored = super::cinematics::VISTA_MARKS[index as usize % 8].thirds_offset_rad;
            let gain = 0.35 + 0.65 * arrival;
            let raw = wrap_pi(view.heading_rad - bearing) - authored * arrival;
            SWING_LIMIT * (raw / SWING_LIMIT).tanh() * gain
        }
        None => 0.0,
    }
}

fn target_mark(map: &super::raycast::RayMap) -> Option<(u8, f32, f32)> {
    map.sprites
        .iter()
        .find(|sprite| sprite.lit && sprite.id < 8)
        .map(|sprite| (sprite.id, sprite.x, sprite.y))
}

const REFERENCE_MIN_DIST: f32 = 3.0;

fn any_landmark(
    map: &super::raycast::RayMap,
    view: &super::raycast::RayView,
) -> Option<(u8, f32, f32)> {
    map.sprites
        .iter()
        .find(|sprite| {
            sprite.id < 8
                && (sprite.x - view.x).powi(2) + (sprite.y - view.y).powi(2)
                    > REFERENCE_MIN_DIST * REFERENCE_MIN_DIST
        })
        .map(|sprite| (sprite.id, sprite.x, sprite.y))
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn wrap_pi(angle: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    let wrapped = (angle + std::f32::consts::PI).rem_euclid(turn);
    wrapped - std::f32::consts::PI
}

#[cfg(test)]
fn settled_camera(vantage: &Vantage, dot_w: usize, dot_h: usize) -> View3 {
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
    let pitch = (tan_v * (2.0 * vantage.horizon - 1.0))
        .atan()
        .clamp(-0.90, 0.90);
    let ground = arch::court_ground_z(scene::SCENE_SEED, vantage.eye.0, vantage.eye.1);
    View3 {
        pos: v3(vantage.eye.0, vantage.eye.1, ground + vantage.eye_h),
        heading_rad: heading,
        pitch,
        fov_rad: 2.0 * tan_h.atan(),
    }
}

#[cfg(test)]
fn camera_standoff(camera: &View3) -> (f32, f32, f32) {
    let scene = scene::scene_for(scene::SceneKey::COURT);
    let half_fov = camera.fov_rad * 0.5 * 1.1;
    let mut nearest = (f32::INFINITY, 0.0, 0.0);
    for tri in &scene.tris {
        if matches!(
            tri.mat,
            mesh::mat::GRASS | mesh::mat::PATH | mesh::mat::WATER | mesh::mat::FLOOR
        ) {
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
#[path = "../../tests/world_viz/world3d__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/world_viz/world3d__raster__tests.rs"]
mod raster_tests;

#[cfg(test)]
#[path = "../../tests/world_viz/frames__world_tests.rs"]
mod frames_world_tests;
