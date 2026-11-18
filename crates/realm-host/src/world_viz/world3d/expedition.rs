//! Live adventure presentation: a camera walking between authored marks and
//! a small party occupying the same 3D space. Tool activity chooses a pose;
//! elapsed visual time never creates an iteration, measurement or reward.

use std::hash::{Hash, Hasher};

use super::arch::{self, Stage, prim};
use super::math::{V3, v3};
use super::mesh::{Mesh, mat};
use super::raster::{self, View3};
use super::{region, scene};
use crate::world_viz::{Quest, RealmActivity, World};
use crate::telemetry::MotionMode;

pub const FRAME_TICKS: u64 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Work {
    #[default]
    Rest,
    Study,
    Forge,
    Council,
    Dispatch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Playback {
    pub travel: u16,
    pub phase: u8,
    pub moving: bool,
    pub work: Work,
}

impl Playback {
    pub fn key(self) -> u64 {
        u64::from(self.travel)
            | u64::from(self.phase) << 16
            | u64::from(self.moving) << 24
            | (self.work as u64) << 25
    }
}

pub fn frame_key(quest: &Quest, playback: Playback) -> u64 {
    let mut key = std::collections::hash_map::DefaultHasher::new();
    quest.region().hash(&mut key);
    quest.iteration().hash(&mut key);
    quest.danger().level().hash(&mut key);
    quest
        .treasures()
        .min(arch::MAX_CHESTS as u32)
        .hash(&mut key);
    quest.party().clamp(1, 3).hash(&mut key);
    playback.key().hash(&mut key);
    key.finish()
}

impl World {
    pub fn live_adventure_view(&self) -> bool {
        (region::stage_for(self.quest().region()).is_some() || self.record_fight_on())
            && self.overworld_view_label().is_none()
            && !self.inside_interior()
            && !self.graph_visiting()
    }

    pub fn expedition_playback(&self, motion: MotionMode) -> Playback {
        let Some(stage) = region::stage_for(self.quest().region()) else {
            return Playback::default();
        };
        let quest = self.quest();
        let marks = region::marks(stage);
        let to = region::waypoint(quest);
        let from = if quest.iteration() == 0 {
            to
        } else {
            (to + marks.len() - 1) % marks.len()
        };
        let distance = if stage == Stage::Mines && to == 0 {
            1.8
        } else {
            ((marks[to].eye.0 - marks[from].eye.0).powi(2)
                + (marks[to].eye.1 - marks[from].eye.1).powi(2))
            .sqrt()
        };
        let duration = (distance * 40.0 / 0.85).clamp(120.0, 400.0);
        let elapsed = (quest.walk_half_ticks(self.tick) / 2).min(u128::from(u32::MAX)) as u32;
        let age = (elapsed / FRAME_TICKS as u32 * FRAME_TICKS as u32) as f32;
        let travel = if motion == MotionMode::Full {
            (age / duration * 1024.0).clamp(0.0, 1024.0) as u16
        } else {
            1024
        };
        let work = self
            .latest_active_work()
            .map_or(Work::Rest, |work| match work.activity {
                RealmActivity::Study | RealmActivity::Memory | RealmActivity::Research => {
                    Work::Study
                }
                RealmActivity::Forge => Work::Forge,
                RealmActivity::Council => Work::Council,
                RealmActivity::Chronicle | RealmActivity::Dispatch => Work::Dispatch,
                RealmActivity::Errand => Work::Rest,
            });
        let phase = match motion {
            MotionMode::Off | MotionMode::Reduced => 0,
            MotionMode::Full => (self.tick / FRAME_TICKS % 240) as u8,
        };
        Playback {
            travel,
            phase,
            moving: travel < 1024,
            work,
        }
    }
}

fn smooth(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

pub fn camera(
    stage: Stage,
    quest: &Quest,
    map: &super::super::raycast::RayMap,
    view: &super::super::raycast::RayView,
    yaw: f32,
    playback: Playback,
    dimensions: (usize, usize),
) -> View3 {
    let to = region::waypoint(quest);
    let count = region::marks(stage).len();
    let from = if quest.iteration() == 0 {
        to
    } else {
        (to + count - 1) % count
    };
    let (w, h) = dimensions;
    let controls = super::super::raycast::RayView {
        x: 0.0,
        y: 0.0,
        heading_rad: 0.0,
        look_yaw: yaw,
        fov_rad: view.fov_rad,
        bob: view.bob,
        eye_h: 0.0,
    };
    let mut a = region::staged_view(stage, from, map, &controls, yaw, w, h);
    let b = region::staged_view(stage, to, map, &controls, yaw, w, h);
    if stage == Stage::Mines && to == 0 {
        a = b;
        a.pos.x -= 1.8;
    }
    let t = smooth(f32::from(playback.travel.min(1024)) / 1024.0);
    let heading = super::wrap_pi(b.heading_rad - a.heading_rad);
    let phase = f32::from(playback.phase) * std::f32::consts::TAU / 240.0;
    let sway = phase.sin() * 0.035;
    let mut pos = a.pos + (b.pos - a.pos) * t + v3(0.0, sway * 0.3, 0.0);
    if stage == Stage::Mines {
        pos.y = pos.y.clamp(0.95, 1.55);
    }
    View3 {
        pos,
        heading_rad: a.heading_rad + heading * t + sway,
        pitch: a.pitch + (b.pitch - a.pitch) * t,
        fov_rad: b.fov_rad,
    }
}

fn beam(mesh: &mut Mesh, from: V3, to: V3, radius: f32, material: u8) {
    let axis = (to - from).normalize();
    let reference = if axis.z.abs() < 0.9 {
        v3(0.0, 0.0, 1.0)
    } else {
        v3(0.0, 1.0, 0.0)
    };
    let across = axis.cross(reference).normalize() * radius;
    let up = axis.cross(across).normalize() * radius;
    let offsets = [across + up, across - up, (across + up) * -1.0, up - across];
    for i in 0..4 {
        let j = (i + 1) % 4;
        prim::quad(
            mesh,
            from + offsets[i],
            to + offsets[i],
            to + offsets[j],
            from + offsets[j],
            material,
        );
    }
    prim::quad(
        mesh,
        to + offsets[0],
        to + offsets[1],
        to + offsets[2],
        to + offsets[3],
        material,
    );
}

fn knight(playback: Playback, companion: bool) -> Mesh {
    let mut mesh = Mesh::new();
    let stride = if playback.moving {
        (f32::from(playback.phase) * std::f32::consts::TAU / 8.0).sin() * 0.19
    } else {
        0.0
    };
    for (side, offset) in [(-1.0, stride), (1.0, -stride)] {
        beam(
            &mut mesh,
            v3(0.0, side * 0.13, 0.68),
            v3(offset, side * 0.13, 0.12),
            0.085,
            mat::STONE_DARK,
        );
        prim::boxed(
            &mut mesh,
            v3(offset - 0.08, side * 0.13 - 0.10, 0.03),
            v3(offset + 0.19, side * 0.13 + 0.10, 0.15),
            mat::STONE,
            prim::F_SIDES_TOP,
        );
    }
    prim::prism(
        &mut mesh,
        0.0,
        0.0,
        0.62,
        1.06,
        0.22,
        0.28,
        6,
        0.0,
        mat::ARMOR,
    );
    prim::disc(&mut mesh, 0.0, 0.0, 1.06, 0.28, 6, 0.0, mat::ARMOR);
    for side in [-1.0, 1.0] {
        prim::prism(
            &mut mesh,
            0.0,
            side * 0.30,
            0.91,
            1.07,
            0.14,
            0.17,
            5,
            0.0,
            mat::ARMOR,
        );
        prim::disc(&mut mesh, 0.0, side * 0.30, 1.07, 0.17, 5, 0.0, mat::ARMOR);
    }
    let cloth = if companion { mat::ROOF } else { mat::BANNER };
    let hem = -0.32 - stride.abs() * 0.7;
    prim::quad(
        &mut mesh,
        v3(-0.24, -0.21, 1.04),
        v3(-0.24, 0.21, 1.04),
        v3(hem, 0.32, 0.45),
        v3(hem, -0.32, 0.37),
        cloth,
    );
    prim::quad(
        &mut mesh,
        v3(-0.24, 0.0, 1.04),
        v3(hem - 0.045, 0.0, 0.38),
        v3(hem, -0.32, 0.37),
        v3(-0.24, -0.21, 1.04),
        cloth,
    );
    prim::prism(
        &mut mesh,
        0.0,
        0.0,
        1.11,
        1.34,
        0.23,
        0.21,
        6,
        std::f32::consts::FRAC_PI_6,
        mat::ARMOR,
    );
    prim::cone(
        &mut mesh,
        0.0,
        0.0,
        1.34,
        1.46,
        0.21,
        6,
        std::f32::consts::FRAC_PI_6,
        mat::ARMOR,
    );
    prim::boxed(
        &mut mesh,
        v3(0.195, -0.16, 1.24),
        v3(0.215, 0.16, 1.29),
        mat::STONE_DARK,
        prim::F_ALL,
    );
    prim::quad(
        &mut mesh,
        v3(-0.20, -0.025, 1.34),
        v3(0.09, -0.025, 1.43),
        v3(-0.04, -0.025, 1.57),
        v3(-0.33, -0.025, 1.43),
        cloth,
    );
    prim::boxed(
        &mut mesh,
        v3(0.205, -0.07, 0.69),
        v3(0.245, 0.07, 0.80),
        mat::BRASS,
        prim::F_SIDES_TOP,
    );
    let lift = if playback.work == Work::Forge && !playback.moving {
        (f32::from(playback.phase) * std::f32::consts::TAU / 16.0).sin() * 0.26
    } else {
        0.0
    };
    for side in [-1.0, 1.0] {
        let hand = v3(
            0.24,
            side * 0.36,
            0.78 + if side < 0.0 { lift } else { 0.0 },
        );
        beam(
            &mut mesh,
            v3(0.0, side * 0.29, 1.03),
            hand,
            0.075,
            mat::ARMOR,
        );
    }
    if playback.work == Work::Study && !playback.moving {
        prim::quad(
            &mut mesh,
            v3(0.23, -0.30, 0.87),
            v3(0.56, -0.30, 0.94),
            v3(0.56, 0.30, 0.94),
            v3(0.23, 0.30, 0.87),
            mat::PARCHMENT,
        );
    } else if playback.work == Work::Forge && !playback.moving {
        let end = v3(0.45, -0.36, 1.20 + lift);
        beam(
            &mut mesh,
            v3(0.22, -0.36, 0.70 + lift),
            end,
            0.035,
            mat::WOOD,
        );
        beam(
            &mut mesh,
            end + v3(-0.18, 0.0, 0.0),
            end + v3(0.24, 0.0, 0.0),
            0.075,
            mat::ARMOR,
        );
        prim::boxed(
            &mut mesh,
            v3(0.52, -0.58, 0.0),
            v3(0.84, -0.20, 0.39),
            mat::WOOD,
            prim::F_SIDES_TOP,
        );
        prim::boxed(
            &mut mesh,
            v3(0.43, -0.62, 0.39),
            v3(0.91, -0.16, 0.57),
            mat::ARMOR,
            prim::F_SIDES_TOP,
        );
    } else {
        beam(
            &mut mesh,
            v3(0.24, -0.36, 0.77),
            v3(0.24, -0.36, 0.57),
            0.025,
            mat::BRASS,
        );
        prim::boxed(
            &mut mesh,
            v3(0.17, -0.43, 0.40),
            v3(0.31, -0.29, 0.59),
            mat::FIRE,
            prim::F_SIDES_TOP,
        );
    }
    mesh
}

fn party_position(stage: Stage, camera: View3, forward: f32, member: u8) -> V3 {
    let (ahead, side) = match member {
        0 => (4.8, -0.45),
        1 => (6.1, 0.62),
        _ => (7.1, -0.55),
    };
    let side = if stage == Stage::Mines {
        [0.85, 1.15, 0.50][usize::from(member.min(2))]
    } else {
        side
    };
    let (sin, cos) = forward.sin_cos();
    let (x, y) = (
        camera.pos.x + cos * ahead - sin * side,
        camera.pos.y + sin * ahead + cos * side,
    );
    v3(x, y, arch::stage_ground_z(stage, scene::SCENE_SEED, x, y))
}

pub fn render_controls(
    stage: Stage,
    quest: &Quest,
    yaw: f32,
    pitch: f32,
    fov: f32,
    playback: Playback,
    dimensions: (u32, u32),
) -> image::RgbaImage {
    let map = super::super::raycast::RayMap {
        width: 0,
        height: 0,
        cells: Vec::new(),
        terrain: Vec::new(),
        terrain_kind: Vec::new(),
        sprites: Vec::new(),
    };
    let view = super::super::raycast::RayView {
        x: 0.0,
        y: 0.0,
        heading_rad: 0.0,
        look_yaw: yaw,
        fov_rad: fov,
        bob: pitch / 0.03,
        eye_h: 0.0,
    };
    render(stage, quest, &map, &view, yaw, playback, dimensions)
}

pub fn render(
    stage: Stage,
    quest: &Quest,
    map: &super::super::raycast::RayMap,
    view: &super::super::raycast::RayView,
    yaw: f32,
    playback: Playback,
    dimensions: (u32, u32),
) -> image::RgbaImage {
    let (w, h) = (dimensions.0 as usize, dimensions.1 as usize);
    let actor_camera = camera(stage, quest, map, view, 0.0, playback, (w, h));
    let camera = camera(stage, quest, map, view, yaw, playback, (w, h));
    let mut mesh = (*scene::region_scene(
        stage,
        quest.danger().level(),
        quest.treasures().min(arch::MAX_CHESTS as u32) as u8,
        region::wisp_phase(stage, u64::from(playback.phase) / 4),
    ))
    .clone();
    let forward = if stage == Stage::Mines {
        0.0
    } else {
        actor_camera.heading_rad
    };
    for member in 0..quest.party().clamp(1, 3) {
        let position = party_position(stage, actor_camera, forward, member);
        let facing = forward + if playback.moving { 0.0 } else { -2.05 };
        mesh.merge(
            knight(playback, member != 0)
                .rotated_z(facing)
                .translated(position),
        );
    }
    raster::render_scene_fogged(
        &mesh,
        &camera,
        w,
        h,
        super::interior::firelight(stage.index() as u8, u64::from(playback.phase)),
        region::fog_distance(stage, quest.danger().level()),
    )
}

#[cfg(test)]
#[path = "../../../tests/world_viz/world3d__expedition_tests.rs"]
mod tests;
