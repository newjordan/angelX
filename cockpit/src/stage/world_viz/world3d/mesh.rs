//! Triangle mesh types shared by the architecture generators and the raster
//! core. This file is the interface contract between the two — keep changes
//! additive (docs/plans/world3d-spec.md).

use super::math::V3;

/// Semantic material ids for 3D geometry. Generators only ever tag triangles
/// with these; the raster shader owns the mapping onto the shared night palette.
pub(crate) mod mat {
    pub(crate) const STONE: u8 = 1;
    pub(crate) const STONE_DARK: u8 = 2;
    pub(crate) const ROOF: u8 = 3;
    pub(crate) const WOOD: u8 = 4;
    pub(crate) const WINDOW: u8 = 5; // emissive warm glow
    pub(crate) const DOOR: u8 = 6;
    pub(crate) const GRASS: u8 = 7;
    pub(crate) const PATH: u8 = 8;
    pub(crate) const WATER: u8 = 9;
    pub(crate) const FOLIAGE: u8 = 10;
    pub(crate) const TRUNK: u8 = 11;
    pub(crate) const BANNER: u8 = 12;
    pub(crate) const BOOKS: u8 = 13; // library shelf fill
    pub(crate) const FLOOR: u8 = 14; // interior flagstone
    /// Live coals: emissive like WINDOW but hotter and oranger, and it holds
    /// its colour through the night haze. The forge throat, the spark-lit
    /// anvil, the pool of firelight on a smithy floor.
    pub(crate) const EMBER: u8 = 15;
    /// Cold light: the moon coming in through an opening, and the pool it
    /// lands in. Emissive like `WINDOW` but silver-blue, unpatterned and only
    /// modestly fog-proof — a shaft is *air*, so it must never carry the
    /// leaded grid a pane does, and it must sink into the haze faster than a
    /// candle does. Interiors only; outside, the moon is a light, not a mass.
    pub(crate) const MOONLIGHT: u8 = 16;
    /// Live flame: the warm dressing a *room* is lit by — a hearth mouth, a
    /// brazier's coals, a candle, a hung lamp. Shades exactly like `WINDOW`
    /// at rest, because it is the same fire behind the same warm palette; the
    /// split exists so the interior seam can make the firelight *breathe*
    /// (`interior::firelight`) without a window pane or a shaft of moon
    /// breathing with it. A window is glazing and the moon is a clock —
    /// neither one flickers.
    pub(crate) const FIRE: u8 = 17;
    /// Aged instrument brass and parchment: reflected light, never status glow.
    pub(crate) const BRASS: u8 = 18;
    pub(crate) const PARCHMENT: u8 = 19;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tri {
    pub v: [V3; 3],
    /// World-scaled surface coords (~1.0 per map tile) for surface patterning.
    pub uv: [[f32; 2]; 3],
    pub mat: u8,
    /// Flat face normal (unit). Double-sided: the shader flips it toward the
    /// camera, so winding is not load-bearing.
    pub normal: V3,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Mesh {
    pub tris: Vec<Tri>,
}

impl Mesh {
    pub(crate) fn new() -> Mesh {
        Mesh::default()
    }

    pub(crate) fn push_tri(&mut self, v: [V3; 3], uv: [[f32; 2]; 3], mat: u8) {
        let normal = (v[1] - v[0]).cross(v[2] - v[0]).normalize();
        self.tris.push(Tri { v, uv, mat, normal });
    }

    /// Quad a→b→c→d around the face; splits into (a,b,c) + (a,c,d). UVs are
    /// auto-derived from edge lengths: u runs a→b, v runs a→d.
    pub(crate) fn push_quad(&mut self, a: V3, b: V3, c: V3, d: V3, mat: u8) {
        let u = (b - a).length();
        let v = (d - a).length();
        self.push_tri([a, b, c], [[0.0, 0.0], [u, 0.0], [u, v]], mat);
        self.push_tri([a, c, d], [[0.0, 0.0], [u, v], [0.0, v]], mat);
    }

    pub(crate) fn merge(&mut self, other: Mesh) {
        self.tris.extend(other.tris);
    }

    pub(crate) fn translated(mut self, d: V3) -> Mesh {
        for t in &mut self.tris {
            for p in &mut t.v {
                *p += d;
            }
        }
        self
    }

    /// Rotate around the +z (up) axis about the origin.
    pub(crate) fn rotated_z(mut self, rad: f32) -> Mesh {
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

    pub(crate) fn scaled(mut self, s: f32) -> Mesh {
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
