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

/// The court's seed, fixed for the life of the process.
///
/// The `World`'s own `self.seed` is deliberately **not** used: the ride seam
/// (`ride.rs::scryglass_frame_paced`) hands the renderer a `RayMap`/`RayView`,
/// not the world, and a per-world seed would mean either a per-seed mesh cache
/// or a rebuild whenever the cache key moves. A fixed seed keeps the stage a
/// constant — the realm is one place, and the camera is what travels — and
/// keeps the determinism law trivially true.
pub(crate) const SCENE_SEED: u64 = 0x4341_5354_4C45_3344; // "CASTLE3D"

/// What a scene is built for. Today the court is the whole world, so the key
/// is a formality — it is the seam a second stage (an interior, a different
/// biome) grows into without changing `scene_for`'s shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SceneKey {
    pub building: u8,
}

impl SceneKey {
    /// The castle court: the one staged world every vantage looks at.
    pub(crate) const COURT: SceneKey = SceneKey { building: 0 };
}

/// The scene for a key, built once and shared. `Arc` rather than `&'static` so
/// a per-key cache can replace the single `OnceLock` later without rippling
/// through the raster core or the ride seam.
pub(crate) fn scene_for(_key: SceneKey) -> Arc<Mesh> {
    static COURT: OnceLock<Arc<Mesh>> = OnceLock::new();
    Arc::clone(COURT.get_or_init(|| Arc::new(arch::court_scene(SCENE_SEED))))
}

/// The staged room behind `/world enter`, one per civic landmark, built on
/// first sight and shared thereafter.
///
/// A fixed array of `OnceLock`s rather than a map: the realm has exactly eight
/// landmarks (`identity::landmark`), the index *is* the slot, and a `HashMap`
/// would put an unordered container in the render path, which the determinism
/// law (docs/plans/world3d-spec.md) forbids. Out-of-range indices fold onto the
/// last room rather than panicking — the ride must always have a frame.
pub(crate) fn interior_scene(building: u8) -> Arc<Mesh> {
    const ROOMS: usize = 8;
    #[allow(clippy::declare_interior_mutable_const)]
    const EMPTY: OnceLock<Arc<Mesh>> = OnceLock::new();
    static INTERIORS: [OnceLock<Arc<Mesh>>; ROOMS] = [EMPTY; ROOMS];
    let slot = (building as usize).min(ROOMS - 1);
    Arc::clone(INTERIORS[slot].get_or_init(|| Arc::new(super::interior::interior_mesh(slot as u8))))
}

/// The staged mesh for one adventure region, built on first sight and shared
/// thereafter.
///
/// Keyed on the four facts the stage's geometry actually depends on — which
/// region, how deep the stall is (torch count), how much loot is on the floor,
/// and which step of the Swamp's wisp drift it is — every one of which
/// `cinematic_key` already hashes. A fixed array of `OnceLock`s rather than a
/// map, for the same reason [`interior_scene`] uses one: the render path must
/// not walk an unordered container (determinism law), and the key here is
/// dense and small (5 × 4 × 4 × 4 = 320 slots, of which only the Swamp's 16
/// phase variants are ever distinct).
pub(crate) fn region_scene(stage: arch::Stage, danger: u8, chests: u8, phase: u8) -> Arc<Mesh> {
    const DANGERS: usize = 4;
    const CHESTS: usize = arch::MAX_CHESTS + 1;
    const PHASES: usize = arch::WISP_PHASES as usize;
    const SLOTS: usize = 5 * DANGERS * CHESTS * PHASES;
    #[allow(clippy::declare_interior_mutable_const)]
    const EMPTY: OnceLock<Arc<Mesh>> = OnceLock::new();
    static STAGES: [OnceLock<Arc<Mesh>>; SLOTS] = [EMPTY; SLOTS];
    let danger = (danger as usize).min(DANGERS - 1);
    let chests = (chests as usize).min(CHESTS - 1);
    // Only the stage that actually moves pays for a slot per phase; every
    // other stage folds onto phase 0, so the cache never holds four identical
    // copies of a static mine.
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
#[path = "../../../../../tests/cockpit/world_viz/world3d__scene__tests.rs"]
mod tests;
