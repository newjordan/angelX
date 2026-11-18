//! Shared map geometry and camera state consumed by Dotmax and room scenes.
//! The historical grid renderer is retired; these scene contracts remain.

#![cfg_attr(not(test), allow(dead_code))]

pub const MATERIAL_ROCK: u8 = 2;
pub const MATERIAL_TIMBER: u8 = 3;
pub const MATERIAL_STONE: u8 = 4;
pub const MATERIAL_DOOR: u8 = 5;
pub const MATERIAL_WATER: u8 = 6;

pub const TERRAIN_MEADOW: u8 = 0;
pub const TERRAIN_SAND: u8 = 1;
pub const TERRAIN_ROAD: u8 = 2;
pub const TERRAIN_HILL: u8 = 3;

pub struct RaySprite {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub id: u8,
    pub lit: bool,
}

pub struct RayMap {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<u8>,
    pub terrain: Vec<f32>,
    pub terrain_kind: Vec<u8>,
    pub sprites: Vec<RaySprite>,
}

impl RayMap {
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return MATERIAL_ROCK;
        }
        self.cells[y as usize * self.width as usize + x as usize]
    }

    pub fn terrain_kind_at(&self, x: i32, y: i32) -> u8 {
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

pub struct RayView {
    pub x: f32,
    pub y: f32,
    pub heading_rad: f32,
    pub look_yaw: f32,
    pub fov_rad: f32,
    pub bob: f32,
    pub eye_h: f32,
}
