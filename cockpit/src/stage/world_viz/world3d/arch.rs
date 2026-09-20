//! Procedural medieval architecture generators — OWNED BY THE ARCHITECTURE
//! AGENT (docs/plans/world3d-spec.md). Keeps, towers, curtain walls, the
//! library, cottages, bridges, terrain, trees, and a composed sample vista.
//!
//! # Contract
//! Every builder here is a **pure function of its arguments**: no wall clock,
//! no ambient randomness, no `HashMap` iteration, no interior mutability.
//! Variation comes only from an explicit `seed: u64` pushed through the
//! splitmix-style hash in [`rng`], so the same seed always emits a
//! byte-identical triangle stream (world3d determinism law).
//!
//! # Conventions
//! - Units are RayMap tiles: `1.0` = one map tile. `+z` is up, ground at `z = 0`.
//! - Each building is **centred on the origin** with its footprint stated in
//!   its doc comment. Place it with `Mesh::rotated_z(yaw)` then
//!   `Mesh::translated(v3(x, y, z))`.
//! - Nothing sits below `z = 0` except WATER (river beds, the well).
//! - Triangles are **double-sided** — the raster shader flips the flat normal
//!   toward the camera — so no builder emits back faces or worries about
//!   winding. A wall is one quad; an open-backed merlon silhouettes exactly
//!   like a solid one, for half the triangles.
//! - Surfaces are tagged with `mesh::mat::*`; the shader owns the palette.
//!   WINDOW is emissive, BOOKS is shelf fill, PATH/WATER/GRASS are ground.
//!   MOONLIGHT is the cold emissive — interior shafts and the pools they land
//!   in; nothing outdoors uses it, because out there the moon is the key light.
//!
//! # Dot-scale art direction
//! Output lands on a braille canvas ~100–160 dots wide under Bayer dithering,
//! so this library is deliberately coarse:
//! - **Bold masses, few of them.** Battered bases and corbelled string courses
//!   give each building one strong horizontal; the rest of the wall stays flat.
//! - **Teeth you can count.** Crenellations are pitched ~1.3 tiles with a 58%
//!   duty cycle, so each merlon spans 2+ dots at vista range and the parapet
//!   reads as a comb rather than a fuzzy line.
//! - **One spike per skyline.** Cone-capped turrets, the chapel spire and the
//!   library's crossing lantern break the roofline so the silhouette has an
//!   accent to catch.
//! - **Punched openings.** Windows are clean rectangles or 5-facet arches
//!   proud of the wall by 0.02 tiles; 5 facets is the floor at which an arch
//!   still reads as an arch instead of a triangle.
//! - **Nothing below ~0.3 tiles** on exteriors. Finer ornament dissolves into
//!   the dither and only costs triangles. (Interiors run finer — they are only
//!   ever seen from inside the room.)
//!
//! # Triangle budgets (measured; the test floor asserts the cap)
//! | builder | tris | cap | footprint (tiles) |
//! |---|---|---|---|
//! | [`keep`] | 386 | 600 | 7.5 × 7.5, ~5.9 tall |
//! | [`round_tower`] | 100–128 | 250 | 2.7·r across, `h + 1.5r + 0.56` tall |
//! | [`rookery_dressing`] | 144 | 160 | 2.7·r across, tower top + 0.36 |
//! | [`curtain_wall`] | 70–134 | 150 | `len` × 1.36, ~3.75 tall |
//! | [`gatehouse`] | 308–344 | 450 | 8.1 × 3.5, ~7.4 tall |
//! | [`forge`] | 135 | 260 | 5.6 × 4.2, ~4.4 tall |
//! | [`observatory`] | 145 | 260 | 5.2 × 5.2, ~7.6 tall |
//! | [`library`] | 404 | 800 | 13.0 × 8.7, ~9.8 tall |
//! | [`library_interior`] | 320 | 600 | 11.4 × 5.4, 6.8 tall |
//! | [`cottage`] | 68 | 120 | 4.2 × 2.8, ~3.6 tall |
//! | [`chapel`] | 130 | 220 | 7.4 × 3.6, ~6.9 tall |
//! | [`market_stall`] | 80 | 140 | 2.8 × 2.0, ~2.3 tall |
//! | [`well`] | 64 | 120 | 1.9 × 1.5, ~2.5 tall |
//! | [`bridge`] | 148 | 200 | `len + 1.4` × 2.6, ~2.4 tall |
//! | [`terrain_patch`] | ≤ 800 | 800 | `w` × `d` (grid capped at 20 × 20) |
//! | [`tree`] | 36 / 52 | 60 | ≤ 2.6 × 2.6, 2.9–4.3 tall |
//! | [`court_scene`] | 4787 | 5000 | 56 × 56 |
//! | [`hamlet_scene`] | ~620 | 1600 | 26 × 18 |
//!
//! The adventure regions ([`regions`]) carry their own table; their ceiling is
//! 8k triangles per stage (Z3 spec §3).

pub(crate) mod castle;
pub(crate) mod compose;
pub(crate) mod land;
pub(crate) mod library;
pub(crate) mod prim;
pub(crate) mod realm_dressing;
pub(crate) mod realm_land;
pub(crate) mod regions;
pub(crate) mod rng;
pub(crate) mod village;
pub(crate) mod works;

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/world3d__arch__tests.rs"]
mod tests;

// Flat re-export surface: the renderer side calls `arch::keep(seed)`, not
// `arch::castle::keep(seed)`. `allow(unused_imports)` because the renderer has
// not wired these up yet — without it every generator reads as dead on arrival.
#[allow(unused_imports)]
pub(crate) use castle::{curtain_wall, gatehouse, keep, rookery_dressing, round_tower};
#[allow(unused_imports)]
pub(crate) use compose::{COURT_EXTENT, court_ground_z, court_scene, hamlet_scene, scene_bounds};
#[allow(unused_imports)]
pub(crate) use land::{Axis, Strip, TerrainOpts, terrain_patch, terrain_patch_with, tree};
#[allow(unused_imports)]
pub(crate) use library::{library, library_interior};
#[allow(unused_imports)]
pub(crate) use regions::{
    MAX_CHESTS, Stage, WISP_PHASES, region_stage, stage_animates, stage_extent, stage_ground_z,
};
#[allow(unused_imports)]
pub(crate) use village::{bridge, chapel, cottage, market_stall, well};
#[allow(unused_imports)]
pub(crate) use works::{forge, observatory};
