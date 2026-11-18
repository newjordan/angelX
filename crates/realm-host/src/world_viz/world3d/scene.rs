//! The staged world — the mesh the ride's camera moves through.
//!
//! Phase C replaced the hand-rolled proving stage with the real procedural
//! architecture: [`arch::court_scene`] — a walled court with a keep,
//! four cone-capped corner towers, a gatehouse, the library outside the west
//! wall, a chapel on the east rise, a hamlet strung along the approach road
//! and a stone bridge over the river, all sitting on one 56 × 56 tile terrain
//! patch with district roads and a stable yard (~4.8k triangles).
//!
//! One scene serves every destination. The *camera* is what changes per
//! target: [`super::vantage`] stages a painting-worthy pose around this court
//! for each of the eight civic landmarks (world vista law). Building the mesh
//! is a once-per-process cost behind a `OnceLock`; every frame just walks the
//! triangle list.
//!
//! Coordinates are tile units with +z up and the court centred on the origin.

use std::sync::{Arc, OnceLock};

use super::arch;
use super::mesh::Mesh;

pub const SCENE_SEED: u64 = 0x4341_5354_4C45_3344;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneKey {
    pub building: u8,
}

impl SceneKey {
    pub const COURT: SceneKey = SceneKey { building: 0 };
}

pub fn scene_for(_key: SceneKey) -> Arc<Mesh> {
    static COURT: OnceLock<Arc<Mesh>> = OnceLock::new();
    Arc::clone(COURT.get_or_init(|| Arc::new(arch::court_scene(SCENE_SEED))))
}

pub fn interior_scene(building: u8) -> Arc<Mesh> {
    const ROOMS: usize = 8;
    #[allow(clippy::declare_interior_mutable_const)]
    const EMPTY: OnceLock<Arc<Mesh>> = OnceLock::new();
    static INTERIORS: [OnceLock<Arc<Mesh>>; ROOMS] = [EMPTY; ROOMS];
    let slot = (building as usize).min(ROOMS - 1);
    Arc::clone(INTERIORS[slot].get_or_init(|| Arc::new(super::interior::interior_mesh(slot as u8))))
}

pub fn region_scene(stage: arch::Stage, danger: u8, chests: u8, phase: u8) -> Arc<Mesh> {
    const DANGERS: usize = 4;
    const CHESTS: usize = arch::MAX_CHESTS + 1;
    const PHASES: usize = arch::WISP_PHASES as usize;
    const SLOTS: usize = 5 * DANGERS * CHESTS * PHASES;
    #[allow(clippy::declare_interior_mutable_const)]
    const EMPTY: OnceLock<Arc<Mesh>> = OnceLock::new();
    static STAGES: [OnceLock<Arc<Mesh>>; SLOTS] = [EMPTY; SLOTS];
    let danger = (danger as usize).min(DANGERS - 1);
    let chests = (chests as usize).min(CHESTS - 1);
    let phase = if arch::stage_animates(stage) {
        (phase as usize) % PHASES
    } else {
        0
    };
    let slot = ((stage.index() * DANGERS + danger) * CHESTS + chests) * PHASES + phase;
    Arc::clone(STAGES[slot].get_or_init(|| {
        Arc::new(arch::region_stage(
            stage,
            SCENE_SEED,
            danger as u8,
            chests as u8,
            phase as u8,
        ))
    }))
}

#[cfg(test)]
#[path = "../../../tests/world_viz/world3d__scene__tests.rs"]
mod tests;
