//! Triangle mesh types shared by the architecture generators and the raster
//! core. This file is the interface contract between the two — keep changes
//! additive (docs/plans/world3d-spec.md).

use super::math::V3;

pub mod mat {
    pub const STONE: u8 = 1;
    pub const STONE_DARK: u8 = 2;
    pub const ROOF: u8 = 3;
    pub const WOOD: u8 = 4;
    pub const WINDOW: u8 = 5;
    pub const DOOR: u8 = 6;
    pub const GRASS: u8 = 7;
    pub const PATH: u8 = 8;
    pub const WATER: u8 = 9;
    pub const FOLIAGE: u8 = 10;
    pub const TRUNK: u8 = 11;
    pub const BANNER: u8 = 12;
    pub const BOOKS: u8 = 13;
    pub const FLOOR: u8 = 14;
    pub const EMBER: u8 = 15;
    pub const MOONLIGHT: u8 = 16;
    pub const FIRE: u8 = 17;
    pub const BRASS: u8 = 18;
    pub const PARCHMENT: u8 = 19;
    pub const ROCK: u8 = 20;
    pub const ARMOR: u8 = 21;
    pub const TORCH_POOL: u8 = 22;
    pub const CRYSTAL: u8 = 23;
    pub const HORSE_BAY: u8 = 24;
    pub const HORSE_CHESTNUT: u8 = 25;
    pub const HORSE_GREY: u8 = 26;
    pub const HAY: u8 = 27;
    pub const CLOTH_GOLD: u8 = 28;
    pub const CLOTH_BLUE: u8 = 29;
    pub const STABLE_TIMBER: u8 = 30;
    pub const STABLE_ROOF: u8 = 31;
    pub const STABLE_WALL: u8 = 32;
    pub const STABLE_FLOOR: u8 = 33;
    pub const TACK_LEATHER: u8 = 34;
    pub const SCULPTED_STONE: u8 = 35;
    pub const BONE: u8 = 36;
    pub const FLAME: u8 = 37;
}

#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub v: [V3; 3],
    pub uv: [[f32; 2]; 3],
    pub mat: u8,
    pub normal: V3,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub tris: Vec<Tri>,
}

impl Mesh {
    pub fn new() -> Mesh {
        Mesh::default()
    }

    pub fn push_tri(&mut self, v: [V3; 3], uv: [[f32; 2]; 3], mat: u8) {
        let normal = (v[1] - v[0]).cross(v[2] - v[0]).normalize();
        self.tris.push(Tri { v, uv, mat, normal });
    }

    pub fn push_quad(&mut self, a: V3, b: V3, c: V3, d: V3, mat: u8) {
        let u = (b - a).length();
        let v = (d - a).length();
        self.push_tri([a, b, c], [[0.0, 0.0], [u, 0.0], [u, v]], mat);
        self.push_tri([a, c, d], [[0.0, 0.0], [u, v], [0.0, v]], mat);
    }

    pub fn merge(&mut self, other: Mesh) {
        self.tris.extend(other.tris);
    }

    pub fn translated(mut self, d: V3) -> Mesh {
        for t in &mut self.tris {
            for p in &mut t.v {
                *p += d;
            }
        }
        self
    }

    pub fn rotated_z(mut self, rad: f32) -> Mesh {
        let (s, c) = rad.sin_cos();
        for t in &mut self.tris {
            for p in &mut t.v {
                let (x, y) = (p.x, p.y);
                p.x = x * c - y * s;
                p.y = x * s + y * c;
            }
            let (nx, ny) = (t.normal.x, t.normal.y);
            t.normal.x = nx * c - ny * s;
            t.normal.y = nx * s + ny * c;
        }
        self
    }

    pub fn scaled(mut self, s: f32) -> Mesh {
        for t in &mut self.tris {
            for p in &mut t.v {
                *p = *p * s;
            }
            for uv in &mut t.uv {
                uv[0] *= s;
                uv[1] *= s;
            }
        }
        self
    }
}
