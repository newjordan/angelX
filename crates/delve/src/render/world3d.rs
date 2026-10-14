//! The 3D races' rasterizer: vectors, meshes, and the dot-canvas raster
//! the cockpit's ride and rooms share.

#![allow(dead_code)]

pub mod math;
pub mod mesh;
pub mod raster;

/// Shared by Dotmax's outdoor and interior lighting. Retained at the same
/// bearing as before the obsolete raycast art implementation was removed.
pub const MOON_BEARING: f32 = 0.62;

pub const NOIR_NIGHT_HAZE: [u8; 3] = [9, 13, 24];
pub const NOIR_MOON_SILVER: [u8; 3] = [172, 192, 220];
pub const NOIR_SKY_ZENITH: [u8; 3] = [3, 5, 14];
pub const NOIR_SKY_HORIZON: [u8; 3] = [22, 34, 60];
