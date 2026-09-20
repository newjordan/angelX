//! Shared map geometry and camera state consumed by Dotmax and room scenes.
//! The historical grid renderer is retired; these scene contracts remain.

#![cfg_attr(not(test), allow(dead_code))]

/// Wall material ids used by `RayMap` cells.
pub(crate) const MATERIAL_ROCK: u8 = 2;
pub(crate) const MATERIAL_TIMBER: u8 = 3;
pub(crate) const MATERIAL_STONE: u8 = 4;
pub(crate) const MATERIAL_DOOR: u8 = 5;
pub(crate) const MATERIAL_WATER: u8 = 6;

pub(crate) const TERRAIN_MEADOW: u8 = 0;
pub(crate) const TERRAIN_SAND: u8 = 1;
pub(crate) const TERRAIN_ROAD: u8 = 2;
pub(crate) const TERRAIN_HILL: u8 = 3;

/// A camera-facing world-space billboard, planted on the floor at `(x, y)`.
pub(crate) struct RaySprite {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) id: u8,
    pub(crate) lit: bool,
}

/// A small grid map. `cells[y * width + x]`: `0` = open floor, anything else
/// is a wall drawn with that material id.
pub(crate) struct RayMap {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) cells: Vec<u8>,
    /// Per-cell terrain height ∈ [0,1] for open ground. Walls, water and
    /// out-of-bounds stay flat (0.0); only today's open-floor cells lift off
    /// the floor, forming the heightfield the first-person view shades.
    pub(crate) terrain: Vec<f32>,
    /// Per-cell open-ground identity, parallel to `terrain`.
    pub(crate) terrain_kind: Vec<u8>,
    /// Camera-facing landmark billboards.
    pub(crate) sprites: Vec<RaySprite>,
}

impl RayMap {
    /// Material at `(x, y)`. Out-of-bounds reads as rock so every ray
    /// terminates without a range check in the march loop.
    pub(crate) fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return MATERIAL_ROCK;
        }
        self.cells[y as usize * self.width as usize + x as usize]
    }

    pub(crate) fn terrain_kind_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return TERRAIN_MEADOW;
        }
        let idx = y as usize * self.width as usize + x as usize;
        if self.cells[idx] != 0 {
            return TERRAIN_MEADOW;
        }
        self.terrain_kind[idx]
    }
}

/// Camera state, in map-cell units.
pub(crate) struct RayView {
    pub(crate) x: f32,
    pub(crate) y: f32,
    /// Radians; 0 looks along +x, π/2 along +y.
    pub(crate) heading_rad: f32,
    /// Explicit operator yaw, separate from the automatic travel heading.
    pub(crate) look_yaw: f32,
    /// Horizontal field of view in radians (~1.05 reads well here).
    pub(crate) fov_rad: f32,
    /// Canter bob in −1.0..=1.0; offsets the horizon a few pixels.
    pub(crate) bob: f32,
    /// Normalized camera-ground height: 0 is the original flat baseline.
    pub(crate) eye_h: f32,
}
