//! The 3D scene (docs/GLASS_CORE.md, decisions 3 and 4): nodes in a tree, each
//! with a transform and, perhaps, a mesh and a material, seen by a camera.
//! Plain data and the small maths it needs, no GPU types, so the window and
//! the browser describe a scene the same way and `angel-gpu` draws it.
//!
//! Conventions, which every host shares:
//! - Right-handed, +Y up. A camera looks down its own -Z, so a node at
//!   `z = 0` and a camera at `z = 3` face one another when the node's front
//!   is +Z.
//! - Matrices are column-major, `m[column][row]`, as WGSL's `mat4x4` is built
//!   from them. A point is a column: `world = parent * local`.
//! - Clip space is WebGPU's: x and y in -1..1, z in 0..1 (near to far).
//! - A triangle's front is its counter-clockwise side.
//! - A texture's `v` runs down: `(0, 0)` is the picture's top left.
//!
//! Nothing here panics on bad input. A node whose parent is missing or whose
//! chain loops is not drawn; a camera that cannot project draws nothing; a
//! mesh that fails [`MeshData::validate`] is refused.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::Tilt;
pub use crate::look::{Dither, Fog, Lamp, Look, LookError, Reveal, Shadow, Sun, Zone};

pub type Mat4 = [[f32; 4]; 4];

pub const IDENTITY_MATRIX: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub const MAX_NODES: usize = 65_536;

pub const MAX_MESH_VERTICES: usize = 1 << 18;
pub const MAX_MESH_INDICES: usize = 3 << 19;

pub const SPHERE_SEGMENTS: std::ops::RangeInclusive<u16> = 3..=128;

pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (c, column) in out.iter_mut().enumerate() {
        for (r, value) in column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

pub fn mat_point(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    let at = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
    [at(0), at(1), at(2)]
}

pub fn mat_finite(m: &Mat4) -> bool {
    m.iter().flatten().all(|v| v.is_finite())
}

pub const QUAT_IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len.is_finite() && len > 1e-12 {
        [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
    } else {
        QUAT_IDENTITY
    }
}

pub fn quat_axis_angle(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if !(len.is_finite() && len > 1e-12 && angle.is_finite()) {
        return QUAT_IDENTITY;
    }
    let (s, c) = (angle * 0.5).sin_cos();
    let k = s / len;
    [axis[0] * k, axis[1] * k, axis[2] * k, c]
}

pub fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let w = q[3];
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross(u, v);
    let t = [2.0 * t[0], 2.0 * t[1], 2.0 * t[2]];
    let ut = cross(u, t);
    [
        v[0] + w * t[0] + ut[0],
        v[1] + w * t[1] + ut[1],
        v[2] + w * t[2] + ut[2],
    ]
}

fn quat_columns(q: [f32; 4]) -> [[f32; 3]; 3] {
    let [x, y, z, w] = quat_normalize(q);
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);
    [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy + wz), 2.0 * (xz - wy)],
        [2.0 * (xy - wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + wx)],
        [2.0 * (xz + wy), 2.0 * (yz - wx), 1.0 - 2.0 * (xx + yy)],
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: [0.0; 3],
        rotation: QUAT_IDENTITY,
        scale: [1.0; 3],
    };

    pub fn from_translation(translation: [f32; 3]) -> Self {
        Self { translation, ..Self::IDENTITY }
    }

    pub fn from_rotation_axis_angle(axis: [f32; 3], angle: f32) -> Self {
        Self { rotation: quat_axis_angle(axis, angle), ..Self::IDENTITY }
    }

    pub fn from_scale(scale: [f32; 3]) -> Self {
        Self { scale, ..Self::IDENTITY }
    }

    pub fn then(&self, child: &Transform) -> Transform {
        let scaled = [
            child.translation[0] * self.scale[0],
            child.translation[1] * self.scale[1],
            child.translation[2] * self.scale[2],
        ];
        let turned = quat_rotate(quat_normalize(self.rotation), scaled);
        Transform {
            translation: [
                self.translation[0] + turned[0],
                self.translation[1] + turned[1],
                self.translation[2] + turned[2],
            ],
            rotation: quat_normalize(quat_mul(
                quat_normalize(self.rotation),
                quat_normalize(child.rotation),
            )),
            scale: [
                self.scale[0] * child.scale[0],
                self.scale[1] * child.scale[1],
                self.scale[2] * child.scale[2],
            ],
        }
    }

    pub fn matrix(&self) -> Mat4 {
        let [c0, c1, c2] = quat_columns(self.rotation);
        let s = self.scale;
        let t = self.translation;
        [
            [c0[0] * s[0], c0[1] * s[0], c0[2] * s[0], 0.0],
            [c1[0] * s[1], c1[1] * s[1], c1[2] * s[1], 0.0],
            [c2[0] * s[2], c2[1] * s[2], c2[2] * s[2], 0.0],
            [t[0], t[1], t[2], 1.0],
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextureSource {
    #[default]
    None,
    Picture(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Sampling {
    #[default]
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AlphaMode {
    #[default]
    Opaque,
    Blend,
    Premultiplied,
}

impl AlphaMode {
    pub fn blended(self) -> bool {
        !matches!(self, AlphaMode::Opaque)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Material {
    pub source: TextureSource,
    pub tint: [f32; 4],
    pub sampling: Sampling,
    pub alpha: AlphaMode,
    pub double_sided: bool,
    pub lit: bool,
    pub emissive: f32,
    pub accent: bool,
    pub projector: Option<Projector>,
    pub patch: Option<Patch>,
    pub role: Role,
    pub ceiling: u8,
    pub normals: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub chrome: f32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub liquid: f32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub glow: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Role {
    #[default]
    Plain,
    Figure,
    Water,
    Sky,
    Ember,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Patch {
    pub picture: u32,
    pub rect: [u32; 4],
    pub origin: [f32; 3],
    pub texel: f32,
}

pub const FACE_SHADES: [f32; 3] = [1.0, 0.86, 0.74];

pub const GRAZING: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Projector {
    pub view_projection: Mat4,
}

impl Projector {
    pub fn of(camera: &Camera, aspect: f32) -> Option<Self> {
        camera.view_projection(aspect).map(|view_projection| Self { view_projection })
    }

    pub fn uv(&self, p: [f32; 3]) -> Option<[f32; 2]> {
        let m = &self.view_projection;
        let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
        let (x, y, w) = (row(0), row(1), row(3));
        if !(w.is_finite() && w > 0.0) {
            return None;
        }
        let uv = [(x / w + 1.0) * 0.5, (1.0 - y / w) * 0.5];
        uv.iter().all(|v| (0.0..=1.0).contains(v)).then_some(uv)
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            source: TextureSource::None,
            tint: [1.0; 4],
            sampling: Sampling::Nearest,
            alpha: AlphaMode::Opaque,
            double_sided: false,
            lit: false,
            emissive: 0.0,
            accent: false,
            projector: None,
            patch: None,
            role: Role::Plain,
            ceiling: 0,
            normals: None,
            chrome: 0.0,
            liquid: 0.0,
            glow: 0.0,
        }
    }
}

pub const KEY_LIGHT: [f32; 3] = [-0.47, 0.70, 0.54];

pub const AMBIENT_LIGHT: f32 = 0.42;

impl Material {
    pub fn color(rgba: [f32; 4]) -> Self {
        Self {
            tint: rgba,
            alpha: if rgba[3] < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
            ..Self::default()
        }
    }

    pub fn picture(id: u32) -> Self {
        Self { source: TextureSource::Picture(id), ..Self::default() }
    }

    pub fn projected(id: u32, projector: Projector, fallback: [f32; 4]) -> Self {
        Self { source: TextureSource::Picture(id), projector: Some(projector), ..Self::color(fallback) }
    }

    pub fn lit(rgba: [f32; 4], emissive: f32) -> Self {
        Self { lit: true, emissive, ..Self::color(rgba) }
    }

    pub fn tint_clamped(&self) -> [f32; 4] {
        self.tint.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 })
    }

    pub fn emissive_clamped(&self) -> f32 {
        unit(self.emissive)
    }

    pub fn finish_clamped(&self) -> [f32; 3] {
        [unit(self.chrome), unit(self.liquid), unit(self.glow)]
    }
}

fn unit(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MeshRef {
    Quad,
    Cube,
    Plane,
    Sphere { segments: u16 },
    Gem,
    Cone { segments: u16 },
    Cylinder { segments: u16 },
    Frustum { segments: u16 },
    Wedge,
    Gable,
    Hip,
    Pyramid,
    Arch { segments: u16 },
    Stairs { steps: u16 },
    Rock { seed: u16 },
    Blob { seed: u16 },
    Fence { posts: u16 },
    RoundCube { radius: u16, segments: u16 },
    Uploaded(u32),
}

impl MeshRef {
    pub fn generate(&self) -> Option<MeshData> {
        match *self {
            MeshRef::Quad => Some(MeshData::quad()),
            MeshRef::Cube => Some(MeshData::cube()),
            MeshRef::Plane => Some(MeshData::plane()),
            MeshRef::Sphere { segments } => Some(MeshData::sphere(segments)),
            MeshRef::Gem => Some(MeshData::gem()),
            MeshRef::Cone { segments } => Some(MeshData::cone(segments)),
            MeshRef::Cylinder { segments } => Some(MeshData::cylinder(segments)),
            MeshRef::Frustum { segments } => Some(MeshData::frustum(segments)),
            MeshRef::Wedge => Some(MeshData::wedge()),
            MeshRef::Gable => Some(MeshData::gable()),
            MeshRef::Hip => Some(MeshData::hip()),
            MeshRef::Pyramid => Some(MeshData::pyramid()),
            MeshRef::Arch { segments } => Some(MeshData::arch(segments)),
            MeshRef::Stairs { steps } => Some(MeshData::stairs(steps)),
            MeshRef::Rock { seed } => Some(MeshData::rock(seed)),
            MeshRef::Blob { seed } => Some(MeshData::blob(seed)),
            MeshRef::Fence { posts } => Some(MeshData::fence(posts)),
            MeshRef::RoundCube { radius, segments } => Some(MeshData::round_cube(radius, segments)),
            MeshRef::Uploaded(_) => None,
        }
    }
}

type GridVertex = ([f32; 3], [f32; 3], [f32; 2]);

pub const ROUND_RADIUS: u16 = 500;

pub const ROUND_SEGMENTS: std::ops::RangeInclusive<u16> = 1..=16;

pub const MAX_JOINTS: usize = 256;

pub const CUBE_FACES: [CubeFace; 6] = [
    CubeFace { name: "+X", normal: [1.0, 0.0, 0.0], right: [0.0, 0.0, -1.0], up: [0.0, 1.0, 0.0] },
    CubeFace { name: "-X", normal: [-1.0, 0.0, 0.0], right: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0] },
    CubeFace { name: "+Y", normal: [0.0, 1.0, 0.0], right: [1.0, 0.0, 0.0], up: [0.0, 0.0, -1.0] },
    CubeFace { name: "-Y", normal: [0.0, -1.0, 0.0], right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0] },
    CubeFace { name: "+Z", normal: [0.0, 0.0, 1.0], right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0] },
    CubeFace { name: "-Z", normal: [0.0, 0.0, -1.0], right: [-1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0] },
];

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CubeFace {
    pub name: &'static str,
    pub normal: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshError {
    Empty,
    TooManyVertices,
    TooManyIndices,
    AttributeLength(&'static str),
    NotTriangles,
    IndexOutOfRange,
    NotFinite,
    BadGroup,
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            MeshError::Empty => "the mesh has no triangles",
            MeshError::TooManyVertices => "the mesh has too many vertices",
            MeshError::TooManyIndices => "the mesh has too many indices",
            MeshError::AttributeLength(name) => {
                return write!(f, "the mesh's {name} are not one per position");
            }
            MeshError::NotTriangles => "the mesh's indices are not whole triangles",
            MeshError::IndexOutOfRange => "an index names a vertex that is not there",
            MeshError::NotFinite => "the mesh has a number that is not finite",
            MeshError::BadGroup => "a group of the mesh runs outside its indices",
        };
        f.write_str(text)
    }
}

impl std::error::Error for MeshError {}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub groups: Vec<(u32, u32)>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub joints: Vec<[u16; 4]>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub weights: Vec<[f32; 4]>,
}

impl MeshData {
    pub fn validate(&self) -> Result<(), MeshError> {
        if self.positions.is_empty() || self.indices.is_empty() {
            return Err(MeshError::Empty);
        }
        if self.positions.len() > MAX_MESH_VERTICES {
            return Err(MeshError::TooManyVertices);
        }
        if self.indices.len() > MAX_MESH_INDICES {
            return Err(MeshError::TooManyIndices);
        }
        let n = self.positions.len();
        for (name, len) in [
            ("normals", self.normals.len()),
            ("uvs", self.uvs.len()),
            ("colors", self.colors.len()),
            ("joints", self.joints.len()),
            ("weights", self.weights.len()),
        ] {
            if len != 0 && len != n {
                return Err(MeshError::AttributeLength(name));
            }
        }
        if self.joints.len() != self.weights.len() {
            return Err(MeshError::AttributeLength(if self.joints.is_empty() { "joints" } else { "weights" }));
        }
        if !self.indices.len().is_multiple_of(3) {
            return Err(MeshError::NotTriangles);
        }
        if self.indices.iter().any(|&i| i as usize >= n) {
            return Err(MeshError::IndexOutOfRange);
        }
        let finite = self.positions.iter().flatten().all(|v| v.is_finite())
            && self.normals.iter().flatten().all(|v| v.is_finite())
            && self.uvs.iter().flatten().all(|v| v.is_finite())
            && self.colors.iter().flatten().all(|v| v.is_finite())
            && self.weights.iter().flatten().all(|v| v.is_finite());
        if !finite {
            return Err(MeshError::NotFinite);
        }
        let total = self.indices.len() as u64;
        for &(first, count) in &self.groups {
            let end = u64::from(first) + u64::from(count);
            if count == 0 || !first.is_multiple_of(3) || !count.is_multiple_of(3) || end > total {
                return Err(MeshError::BadGroup);
            }
        }
        Ok(())
    }

    pub fn centre(&self) -> [f32; 3] {
        if self.positions.is_empty() {
            return [0.0; 3];
        }
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in &self.positions {
            for ((lo, hi), v) in lo.iter_mut().zip(hi.iter_mut()).zip(p) {
                *lo = lo.min(*v);
                *hi = hi.max(*v);
            }
        }
        [
            (lo[0] + hi[0]) * 0.5,
            (lo[1] + hi[1]) * 0.5,
            (lo[2] + hi[2]) * 0.5,
        ]
    }

    fn push_face(
        &mut self,
        centre: [f32; 3],
        normal: [f32; 3],
        half_right: [f32; 3],
        half_up: [f32; 3],
    ) {
        let base = self.positions.len() as u32;
        let corner = |sr: f32, su: f32| {
            [
                centre[0] + half_right[0] * sr + half_up[0] * su,
                centre[1] + half_right[1] * sr + half_up[1] * su,
                centre[2] + half_right[2] * sr + half_up[2] * su,
            ]
        };
        self.positions.extend([corner(-1.0, 1.0), corner(1.0, 1.0), corner(1.0, -1.0), corner(-1.0, -1.0)]);
        self.normals.extend([normal; 4]);
        self.uvs.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        self.colors.extend([[1.0; 4]; 4]);
        self.indices.extend([base, base + 3, base + 2, base, base + 2, base + 1]);
    }

    pub fn quad() -> Self {
        let mut mesh = Self::default();
        mesh.push_face([0.0; 3], [0.0, 0.0, 1.0], [0.5, 0.0, 0.0], [0.0, 0.5, 0.0]);
        mesh
    }

    pub fn plane() -> Self {
        let mut mesh = Self::default();
        mesh.push_face([0.0; 3], [0.0, 1.0, 0.0], [0.5, 0.0, 0.0], [0.0, 0.0, -0.5]);
        mesh
    }

    pub fn cube() -> Self {
        let mut mesh = Self::default();
        for face in &CUBE_FACES {
            let scaled = |v: [f32; 3]| [v[0] * 0.5, v[1] * 0.5, v[2] * 0.5];
            mesh.push_face(scaled(face.normal), face.normal, scaled(face.right), scaled(face.up));
        }
        mesh.groups = (0..6).map(|i| (i * 6, 6)).collect();
        mesh
    }

    pub fn sphere(segments: u16) -> Self {
        let segments = segments.clamp(*SPHERE_SEGMENTS.start(), *SPHERE_SEGMENTS.end());
        let (cols, rows) = (u32::from(segments), u32::from(segments / 2).max(2));
        let mut mesh = Self::default();
        for j in 0..=rows {
            let v = j as f32 / rows as f32;
            let (sin_t, cos_t) = (v * std::f32::consts::PI).sin_cos();
            for i in 0..=cols {
                let u = i as f32 / cols as f32;
                let (sin_p, cos_p) = (u * std::f32::consts::TAU).sin_cos();
                let normal = [sin_t * sin_p, cos_t, sin_t * cos_p];
                mesh.positions.push(normal.map(|n| n * 0.5));
                mesh.normals.push(normal);
                mesh.uvs.push([u, v]);
                mesh.colors.push([1.0; 4]);
            }
        }
        let at = |i: u32, j: u32| j * (cols + 1) + i;
        for j in 0..rows {
            for i in 0..cols {
                let (tl, tr, bl, br) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
                if j + 1 < rows {
                    mesh.indices.extend([tl, bl, br]);
                }
                if j > 0 {
                    mesh.indices.extend([tl, br, tr]);
                }
            }
        }
        mesh
    }

    pub fn round_cube(radius: u16, segments: u16) -> Self {
        let r = f32::from(radius.min(ROUND_RADIUS)) / 1000.0;
        let n = usize::from(segments.clamp(*ROUND_SEGMENTS.start(), *ROUND_SEGMENTS.end()));
        let h = 0.5 - r;
        let mut mesh = Self::default();
        for face in &CUBE_FACES {
            let scaled = |v: [f32; 3], k: f32| [v[0] * k, v[1] * k, v[2] * k];
            mesh.push_face(scaled(face.normal, 0.5), face.normal, scaled(face.right, h), scaled(face.up, h));
        }
        mesh.groups = (0..6).map(|i| (i * 6, 6)).collect();
        let round = mesh.indices.len() as u32;
        let arc: Vec<(f32, f32)> = (0..=n)
            .map(|k| {
                let (c, s) = if 2 * k <= n {
                    let a = k as f32 / n as f32 * std::f32::consts::FRAC_PI_2;
                    (a.cos(), a.sin())
                } else {
                    let a = (n - k) as f32 / n as f32 * std::f32::consts::FRAC_PI_2;
                    (a.sin(), a.cos())
                };
                if k == 0 { (1.0, 0.0) } else if k == n { (0.0, 1.0) } else { (c, s) }
            })
            .collect();
        let axis = |a: usize, sign: f32| {
            let mut v = [0.0f32; 3];
            v[a] = sign;
            v
        };
        let at = |centre: [f32; 3], normal: [f32; 3]| {
            [centre[0] + r * normal[0], centre[1] + r * normal[1], centre[2] + r * normal[2]]
        };
        for a in 0..3 {
            let (b, c) = ((a + 1) % 3, (a + 2) % 3);
            for (s1, s2) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                let (eb, ec) = (axis(b, s1), axis(c, s2));
                let rows: Vec<Vec<GridVertex>> = (0..=n)
                    .map(|k| {
                        let (cos, sin) = arc[k];
                        let normal = [cos * eb[0] + sin * ec[0], cos * eb[1] + sin * ec[1], cos * eb[2] + sin * ec[2]];
                        [-h, h]
                            .iter()
                            .enumerate()
                            .map(|(end, &along)| {
                                let mut centre = [0.0f32; 3];
                                centre[a] = along;
                                centre[b] = s1 * h;
                                centre[c] = s2 * h;
                                (at(centre, normal), normal, [k as f32 / n as f32, end as f32])
                            })
                            .collect()
                    })
                    .collect();
                mesh.push_grid(&rows, false);
            }
        }
        for sx in [1.0f32, -1.0] {
            for sy in [1.0f32, -1.0] {
                for sz in [1.0f32, -1.0] {
                    let centre = [sx * h, sy * h, sz * h];
                    let rows: Vec<Vec<GridVertex>> = (0..=n)
                        .map(|j| {
                            let (ct, st) = arc[j];
                            (0..=n)
                                .map(|i| {
                                    let (cp, sp) = arc[i];
                                    let normal = [ct * (cp * sx), ct * (sp * sy), st * sz];
                                    (at(centre, normal), normal, [i as f32 / n as f32, j as f32 / n as f32])
                                })
                                .collect()
                        })
                        .collect();
                    mesh.push_grid(&rows, true);
                }
            }
        }
        mesh.groups.push((round, mesh.indices.len() as u32 - round));
        mesh
    }

    fn push_grid(&mut self, rows: &[Vec<GridVertex>], pole: bool) {
        let base = self.positions.len() as u32;
        let width = rows.first().map_or(0, Vec::len) as u32;
        for row in rows {
            for &(p, normal, uv) in row {
                self.positions.push(p);
                self.normals.push(normal);
                self.uvs.push(uv);
                self.colors.push([1.0; 4]);
            }
        }
        let rows_n = rows.len() as u32;
        let tri = |mesh: &mut Self, a: u32, b: u32, c: u32| {
            let p = |i: u32| mesh.positions[(base + i) as usize];
            let (pa, pb, pc) = (p(a), p(b), p(c));
            let u = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
            let v = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
            let cross = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let n = |i: u32| mesh.normals[(base + i) as usize];
            let (na, nb, nc) = (n(a), n(b), n(c));
            let out = [na[0] + nb[0] + nc[0], na[1] + nb[1] + nc[1], na[2] + nb[2] + nc[2]];
            let facing = cross[0] * out[0] + cross[1] * out[1] + cross[2] * out[2];
            if facing < 0.0 {
                mesh.indices.extend([base + a, base + c, base + b]);
            } else {
                mesh.indices.extend([base + a, base + b, base + c]);
            }
        };
        for j in 0..rows_n.saturating_sub(1) {
            for i in 0..width.saturating_sub(1) {
                let (tl, tr) = (j * width + i, j * width + i + 1);
                let (bl, br) = ((j + 1) * width + i, (j + 1) * width + i + 1);
                tri(self, tl, tr, br);
                if !(pole && j + 2 == rows_n) {
                    tri(self, tl, br, bl);
                }
            }
        }
    }
}

impl MeshData {
    pub fn gem() -> Self {
        let mut mesh = Self::default();
        for face in 0..8u32 {
            let sign = |bit: u32| if face >> bit & 1 == 0 { 1.0f32 } else { -1.0 };
            let (sx, sy, sz) = (sign(0), sign(1), sign(2));
            let px = [0.5 * sx, 0.0, 0.0];
            let py = [0.0, 0.5 * sy, 0.0];
            let pz = [0.0, 0.0, 0.5 * sz];
            let k = 1.0 / 3.0f32.sqrt();
            let normal = [sx * k, sy * k, sz * k];
            let corners = if sx * sy * sz > 0.0 { [px, py, pz] } else { [px, pz, py] };
            let base = mesh.positions.len() as u32;
            mesh.positions.extend(corners);
            mesh.normals.extend([normal; 3]);
            mesh.uvs.extend([[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]]);
            mesh.colors.extend([[1.0; 4]; 3]);
            mesh.indices.extend([base, base + 1, base + 2]);
        }
        mesh
    }

    pub fn cone(segments: u16) -> Self {
        let n = u32::from(segments.clamp(*SPHERE_SEGMENTS.start(), *SPHERE_SEGMENTS.end()));
        let mut mesh = Self::default();
        let ring = |i: u32| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            a.sin_cos()
        };
        let slant = 1.0 / 1.25f32.sqrt();
        for i in 0..n {
            let (s0, c0) = ring(i);
            let (s1, c1) = ring(i + 1);
            let (sm, cm) = ((s0 + s1) * 0.5, (c0 + c1) * 0.5);
            let len = (sm * sm + cm * cm).sqrt().max(1e-6);
            let normal = [sm / len * slant, 0.5 * slant, cm / len * slant];
            let base = mesh.positions.len() as u32;
            mesh.positions.extend([[0.0, 0.5, 0.0], [0.5 * s0, -0.5, 0.5 * c0], [0.5 * s1, -0.5, 0.5 * c1]]);
            mesh.normals.extend([normal; 3]);
            let (u0, u1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            mesh.uvs.extend([[(u0 + u1) * 0.5, 0.0], [u0, 1.0], [u1, 1.0]]);
            mesh.colors.extend([[1.0; 4]; 3]);
            mesh.indices.extend([base, base + 1, base + 2]);
        }
        let middle = mesh.positions.len() as u32;
        mesh.positions.push([0.0, -0.5, 0.0]);
        mesh.normals.push([0.0, -1.0, 0.0]);
        mesh.uvs.push([0.5, 0.5]);
        mesh.colors.push([1.0; 4]);
        for i in 0..=n {
            let (s, c) = ring(i);
            mesh.positions.push([0.5 * s, -0.5, 0.5 * c]);
            mesh.normals.push([0.0, -1.0, 0.0]);
            mesh.uvs.push([0.5 + 0.5 * s, 0.5 + 0.5 * c]);
            mesh.colors.push([1.0; 4]);
        }
        for i in 0..n {
            mesh.indices.extend([middle, middle + 2 + i, middle + 1 + i]);
        }
        mesh
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Node {
    pub id: u64,
    pub parent: Option<u64>,
    pub transform: Transform,
    pub mesh: Option<MeshRef>,
    pub material: Material,
    pub face_materials: Vec<Material>,
    pub visible: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub skin: Option<Skin>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Skin {
    pub joints: Arc<[u64]>,
    pub inverse_bind: Arc<[Mat4]>,
}

pub fn skin_matrices(skin: &Skin, mut world_of: impl FnMut(u64) -> Option<Mat4>) -> Option<Vec<Mat4>> {
    if skin.joints.is_empty() || skin.joints.len() > MAX_JOINTS || skin.joints.len() != skin.inverse_bind.len() {
        return None;
    }
    let mut out = Vec::with_capacity(skin.joints.len());
    for (&joint, bind) in skin.joints.iter().zip(skin.inverse_bind.iter()) {
        let m = mat_mul(&world_of(joint)?, bind);
        if !mat_finite(&m) {
            return None;
        }
        out.push(m);
    }
    Some(out)
}

pub fn skin_matrix(matrices: &[Mat4], joints: [u16; 4], weights: [f32; 4], fallback: &Mat4) -> Mat4 {
    let mut out = [[0.0f32; 4]; 4];
    let mut total = 0.0f32;
    for (&j, &w) in joints.iter().zip(&weights) {
        let Some(m) = matrices.get(usize::from(j)) else { continue };
        if !(w.is_finite() && w > 0.0) {
            continue;
        }
        total += w;
        for (column, from) in out.iter_mut().zip(m) {
            for (value, v) in column.iter_mut().zip(from) {
                *value += w * v;
            }
        }
    }
    if total <= 1e-6 {
        return *fallback;
    }
    out.map(|column| column.map(|v| v / total))
}

impl Node {
    pub fn new(id: u64, mesh: MeshRef, material: Material) -> Self {
        Self {
            id,
            parent: None,
            transform: Transform::IDENTITY,
            mesh: Some(mesh),
            material,
            face_materials: Vec::new(),
            visible: true,
            skin: None,
        }
    }

    pub fn empty(id: u64) -> Self {
        Self {
            id,
            parent: None,
            transform: Transform::IDENTITY,
            mesh: None,
            material: Material::default(),
            face_materials: Vec::new(),
            visible: true,
            skin: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Projection {
    Perspective { fov_y: f32, near: f32, far: f32 },
    Orthographic { height: f32, near: f32, far: f32 },
    Custom(Mat4),
}

impl Projection {
    pub fn matrix(&self, aspect: f32) -> Option<Mat4> {
        if !(aspect.is_finite() && aspect > 0.0) {
            return None;
        }
        match *self {
            Projection::Perspective { fov_y, near, far } => {
                let sane = fov_y.is_finite()
                    && near.is_finite()
                    && far.is_finite()
                    && fov_y > 0.0
                    && fov_y < std::f32::consts::PI
                    && near > 0.0
                    && far > near;
                if !sane {
                    return None;
                }
                let f = 1.0 / (fov_y * 0.5).tan();
                let range = near - far;
                Some([
                    [f / aspect, 0.0, 0.0, 0.0],
                    [0.0, f, 0.0, 0.0],
                    [0.0, 0.0, far / range, -1.0],
                    [0.0, 0.0, near * far / range, 0.0],
                ])
            }
            Projection::Orthographic { height, near, far } => {
                let sane = height.is_finite()
                    && near.is_finite()
                    && far.is_finite()
                    && height > 0.0
                    && far > near;
                if !sane {
                    return None;
                }
                let range = near - far;
                Some([
                    [2.0 / (height * aspect), 0.0, 0.0, 0.0],
                    [0.0, 2.0 / height, 0.0, 0.0],
                    [0.0, 0.0, 1.0 / range, 0.0],
                    [0.0, 0.0, near / range, 1.0],
                ])
            }
            Projection::Custom(m) => mat_finite(&m).then_some(m),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Camera {
    pub transform: Transform,
    pub projection: Projection,
    pub clear: Option<[f32; 4]>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            transform: Transform::from_translation([0.0, 0.0, 3.0]),
            projection: Projection::Perspective {
                fov_y: std::f32::consts::FRAC_PI_4,
                near: 0.1,
                far: 100.0,
            },
            clear: Some([0.0; 4]),
        }
    }
}

impl Camera {
    pub fn view_matrix(&self) -> Mat4 {
        let [c0, c1, c2] = quat_columns(self.transform.rotation);
        let t = self.transform.translation;
        let dot = |c: [f32; 3]| -(c[0] * t[0] + c[1] * t[1] + c[2] * t[2]);
        [
            [c0[0], c1[0], c2[0], 0.0],
            [c0[1], c1[1], c2[1], 0.0],
            [c0[2], c1[2], c2[2], 0.0],
            [dot(c0), dot(c1), dot(c2), 1.0],
        ]
    }

    pub fn view_projection(&self, aspect: f32) -> Option<Mat4> {
        let view = self.view_matrix();
        if !mat_finite(&view) {
            return None;
        }
        let m = mat_mul(&self.projection.matrix(aspect)?, &view);
        mat_finite(&m).then_some(m)
    }

    pub fn clear_premultiplied(&self) -> Option<[f32; 4]> {
        self.clear.map(|c| {
            let c = c.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 });
            [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
        })
    }
}

const STAGE_DEPTH_PER_PX: f32 = 1.0 / 8192.0;

impl Tilt {
    pub fn flat(size: (u32, u32)) -> Self {
        Self { centre: [size.0 as f32 / 2.0, size.1 as f32 / 2.0], near: 0.0, zoom: 1.0, squash: 1.0 }
    }

    pub fn camera(&self, size: (u32, u32)) -> Camera {
        let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let [cx, cy] = self.centre;
        let (zoom, squash, near) = (self.zoom, self.squash, self.near);
        type Row = [f32; 4];
        let add = |a: Row, k: f32, b: Row| [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2], a[3] + k * b[3]];
        let scale = |a: Row, k: f32| a.map(|v| v * k);
        let depth: Row = [0.0, 0.0, -near, 1.0 + near * cy];
        let screen_x = add([zoom, 0.0, 0.0, -zoom * cx], cx, depth);
        let screen_y = add([0.0, -zoom, zoom * squash, -zoom * squash * cy], cy, depth);
        let clip_x = add(scale(screen_x, 2.0 / w), -1.0, depth);
        let clip_y = add(depth, -2.0 / h, screen_y);
        let clip_z = add([0.0, 0.0, -STAGE_DEPTH_PER_PX, STAGE_DEPTH_PER_PX * cy], 0.5, depth);
        let rows = [clip_x, clip_y, clip_z, depth];
        let matrix: Mat4 = std::array::from_fn(|column| std::array::from_fn(|row| rows[row][column]));
        Camera { transform: Transform::IDENTITY, projection: Projection::Custom(matrix), clear: Some([0.0; 4]) }
    }
}

pub fn stage_camera(tilt: f32, size: (u32, u32)) -> Camera {
    Tilt::for_stage(tilt, size).unwrap_or_else(|| Tilt::flat(size)).camera(size)
}

pub const MAX_GRAIN: u8 = 8;

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Props {
    pub at: usize,
    pub grain: u8,
    pub nodes: Vec<Node>,
    pub world: Option<World>,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct World {
    pub floor: Vec<Node>,
    pub blocks: Vec<Node>,
    pub extra: Vec<Node>,
    pub at: usize,
    pub figures: Vec<Node>,
    pub figures_at: usize,
    pub set: Option<SetRef>,
    pub more: Vec<SetRef>,
    pub look: Option<Look>,
    pub meshes: Vec<(u32, std::sync::Arc<MeshData>)>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetRef {
    pub key: u64,
    pub text: Option<std::sync::Arc<str>>,
    pub plate: u32,
    pub png: Option<std::sync::Arc<[u8]>>,
    pub scale: f32,
    pub offset: [f32; 3],
    pub patches: crate::set::Patches,
}

impl Props {
    pub fn grain_clamped(&self) -> u32 {
        u32::from(self.grain.clamp(1, MAX_GRAIN))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawable {
    pub node: usize,
    pub world: Mat4,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scene {
    pub camera: Camera,
    pub nodes: Vec<Node>,
    pub look: Option<Look>,
}

impl Scene {
    pub fn new(camera: Camera) -> Self {
        Self { camera, nodes: Vec::new(), look: None }
    }

    pub fn world_matrices(&self) -> Vec<Option<Mat4>> {
        self.placed().into_iter().map(|p| p.map(|(m, _)| m)).collect()
    }

    fn placed(&self) -> Vec<Option<(Mat4, bool)>> {
        let n = self.nodes.len().min(MAX_NODES);
        let mut out: Vec<Option<(Mat4, bool)>> = vec![None; self.nodes.len()];
        let mut by_id: HashMap<u64, usize> = HashMap::with_capacity(n);
        let mut taken = vec![false; n];
        for (i, node) in self.nodes[..n].iter().enumerate() {
            if let Entry::Vacant(slot) = by_id.entry(node.id) {
                slot.insert(i);
            } else {
                taken[i] = true;
            }
        }
        let mut state = vec![0u8; n];
        let mut chain: Vec<usize> = Vec::new();
        for start in 0..n {
            if state[start] == 2 {
                continue;
            }
            chain.clear();
            let mut cur = start;
            let base: Option<(Mat4, bool)> = loop {
                if state[cur] == 2 {
                    break out[cur];
                }
                if state[cur] == 1 {
                    break None;
                }
                state[cur] = 1;
                chain.push(cur);
                if taken[cur] {
                    break None;
                }
                match self.nodes[cur].parent {
                    None => break Some((IDENTITY_MATRIX, true)),
                    Some(parent) => match by_id.get(&parent) {
                        Some(&p) => cur = p,
                        None => break None,
                    },
                }
            };
            let mut above = base;
            for &i in chain.iter().rev() {
                above = match above {
                    Some((parent, shown)) if !taken[i] => {
                        let node = &self.nodes[i];
                        let world = mat_mul(&parent, &node.transform.matrix());
                        mat_finite(&world).then_some((world, shown && node.visible))
                    }
                    _ => None,
                };
                out[i] = above;
                state[i] = 2;
            }
        }
        out
    }

    pub fn skin_matrices(&self, index: usize) -> Option<Vec<Mat4>> {
        let skin = self.nodes.get(index)?.skin.as_ref()?;
        let worlds = self.world_matrices();
        let mut by_id: HashMap<u64, usize> = HashMap::with_capacity(worlds.len());
        for (i, node) in self.nodes.iter().enumerate().take(MAX_NODES) {
            by_id.entry(node.id).or_insert(i);
        }
        skin_matrices(skin, |id| by_id.get(&id).and_then(|&i| worlds.get(i).copied().flatten()))
    }

    pub fn draw_list(&self) -> Vec<Drawable> {
        self.placed()
            .into_iter()
            .enumerate()
            .filter_map(|(node, placed)| {
                let (world, shown) = placed?;
                (shown && self.nodes[node].mesh.is_some()).then_some(Drawable { node, world })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn near3(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|k| near(a[k], b[k]))
    }

    #[test]
    fn a_quarter_turn_about_z_takes_x_to_y() {
        let q = quat_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(near(q[2], h) && near(q[3], h) && near(q[0], 0.0) && near(q[1], 0.0));
        assert!(near3(quat_rotate(q, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        assert!(near3(quat_rotate(q, [0.0, 1.0, 0.0]), [-1.0, 0.0, 0.0]));
        assert!(near3(quat_rotate(q, [0.0, 0.0, 1.0]), [0.0, 0.0, 1.0]));
        let long = quat_axis_angle([0.0, 0.0, 5.0], FRAC_PI_2);
        assert!((0..4).all(|k| near(long[k], q[k])));
    }

    #[test]
    fn a_quarter_turn_about_y_takes_z_to_x_and_x_to_minus_z() {
        let q = quat_axis_angle([0.0, 1.0, 0.0], FRAC_PI_2);
        assert!(near3(quat_rotate(q, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]));
        assert!(near3(quat_rotate(q, [1.0, 0.0, 0.0]), [0.0, 0.0, -1.0]));
    }

    #[test]
    fn quaternions_compose_right_to_left() {
        let about_x = quat_axis_angle([1.0, 0.0, 0.0], FRAC_PI_2);
        let about_y = quat_axis_angle([0.0, 1.0, 0.0], FRAC_PI_2);
        let both = quat_mul(about_y, about_x);
        assert!(near3(quat_rotate(both, [0.0, 0.0, 1.0]), [0.0, -1.0, 0.0]));
        let other = quat_mul(about_x, about_y);
        assert!(near3(quat_rotate(other, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]));
        assert_eq!(quat_mul(QUAT_IDENTITY, about_x), about_x);
        let half = quat_axis_angle([0.0, 0.0, 1.0], PI);
        let twice = quat_mul(half, half);
        assert!(near3(quat_rotate(twice, [1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]));
    }

    #[test]
    fn a_useless_turn_is_no_turn() {
        assert_eq!(quat_axis_angle([0.0; 3], 1.0), QUAT_IDENTITY);
        assert_eq!(quat_axis_angle([0.0, 1.0, 0.0], f32::NAN), QUAT_IDENTITY);
        assert_eq!(quat_normalize([0.0; 4]), QUAT_IDENTITY);
        assert_eq!(quat_normalize([f32::NAN, 0.0, 0.0, 1.0]), QUAT_IDENTITY);
        let q = quat_normalize([0.0, 0.0, 3.0, 4.0]);
        assert!(near(q[2], 0.6) && near(q[3], 0.8));
    }

    #[test]
    fn the_identity_matrix_is_the_identity_transform() {
        assert_eq!(Transform::IDENTITY.matrix(), IDENTITY_MATRIX);
        assert_eq!(Transform::default(), Transform::IDENTITY);
    }

    #[test]
    fn translation_sits_in_the_last_column() {
        let m = Transform::from_translation([1.0, 2.0, 3.0]).matrix();
        assert_eq!(m[3], [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(mat_point(&m, [1.0, 1.0, 1.0]), [2.0, 3.0, 4.0]);
    }

    #[test]
    fn a_quarter_turn_matrix_has_the_hand_computed_columns() {
        let m = Transform::from_rotation_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2).matrix();
        let expect = [
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        for c in 0..4 {
            for r in 0..4 {
                assert!(near(m[c][r], expect[c][r]), "[{c}][{r}] = {}", m[c][r]);
            }
        }
    }

    #[test]
    fn the_matrix_scales_then_turns_then_moves() {
        let t = Transform {
            translation: [10.0, 0.0, 0.0],
            rotation: quat_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2),
            scale: [2.0, 3.0, 4.0],
        };
        assert!(near3(mat_point(&t.matrix(), [1.0, 0.0, 0.0]), [10.0, 2.0, 0.0]));
        assert!(near3(mat_point(&t.matrix(), [0.0, 1.0, 0.0]), [7.0, 0.0, 0.0]));
        assert!(near3(mat_point(&t.matrix(), [0.0, 0.0, 1.0]), [10.0, 0.0, 4.0]));
    }

    #[test]
    fn a_matrix_product_applies_the_right_one_first() {
        let move_x = Transform::from_translation([5.0, 0.0, 0.0]).matrix();
        let turn = Transform::from_rotation_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2).matrix();
        assert!(near3(mat_point(&mat_mul(&move_x, &turn), [1.0, 0.0, 0.0]), [5.0, 1.0, 0.0]));
        assert!(near3(mat_point(&mat_mul(&turn, &move_x), [1.0, 0.0, 0.0]), [0.0, 6.0, 0.0]));
        assert_eq!(mat_mul(&IDENTITY_MATRIX, &turn), turn);
    }

    #[test]
    fn then_puts_the_child_in_the_parents_frame() {
        let parent = Transform {
            translation: [10.0, 0.0, 0.0],
            rotation: quat_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2),
            scale: [2.0, 2.0, 2.0],
        };
        let child = Transform {
            translation: [1.0, 0.0, 0.0],
            rotation: quat_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2),
            scale: [0.5, 0.5, 0.5],
        };
        let both = parent.then(&child);
        assert!(near3(both.translation, [10.0, 2.0, 0.0]));
        assert!(near3(both.scale, [1.0, 1.0, 1.0]));
        let product = mat_mul(&parent.matrix(), &child.matrix());
        for p in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.3, -2.0, 5.0]] {
            assert!(near3(mat_point(&both.matrix(), p), mat_point(&product, p)));
        }
        assert_eq!(Transform::IDENTITY.then(&child).matrix(), child.matrix());
    }

    #[test]
    fn the_cameras_view_undoes_its_place() {
        let cam = Camera {
            transform: Transform {
                translation: [1.0, 2.0, 3.0],
                rotation: quat_axis_angle([0.0, 1.0, 0.0], FRAC_PI_2),
                scale: [1.0; 3],
            },
            ..Camera::default()
        };
        assert!(near3(mat_point(&cam.view_matrix(), [1.0, 2.0, 3.0]), [0.0; 3]));
        let both = mat_mul(&cam.view_matrix(), &cam.transform.matrix());
        for c in 0..4 {
            for r in 0..4 {
                assert!(near(both[c][r], IDENTITY_MATRIX[c][r]), "[{c}][{r}]");
            }
        }
        assert!(near3(mat_point(&cam.view_matrix(), [0.0, 2.0, 3.0]), [0.0, 0.0, -1.0]));
    }

    #[test]
    fn a_perspective_view_maps_near_to_zero_and_far_to_one() {
        let p = Projection::Perspective { fov_y: FRAC_PI_2, near: 1.0, far: 5.0 }
            .matrix(2.0)
            .unwrap();
        let clip = |z: f32, x: f32, y: f32| {
            let v = [x, y, z, 1.0];
            let at = |r: usize| (0..4).map(|k| p[k][r] * v[k]).sum::<f32>();
            [at(0) / at(3), at(1) / at(3), at(2) / at(3)]
        };
        assert!(near3(clip(-1.0, 0.0, 0.0), [0.0, 0.0, 0.0]));
        assert!(near3(clip(-5.0, 0.0, 0.0), [0.0, 0.0, 1.0]));
        assert!(near3(clip(-2.0, 4.0, 2.0), [1.0, 1.0, 0.625]));
        assert!(near3(clip(-3.0, -3.0, -6.0), [-0.5, -2.0, 5.0 / 6.0]));
    }

    #[test]
    fn an_orthographic_view_maps_its_box_to_clip_space() {
        let p = Projection::Orthographic { height: 4.0, near: 1.0, far: 3.0 }
            .matrix(2.0)
            .unwrap();
        let clip = |x: f32, y: f32, z: f32| {
            let v = [x, y, z, 1.0];
            [0, 1, 2].map(|r| (0..4).map(|k| p[k][r] * v[k]).sum::<f32>())
        };
        assert!(near3(clip(4.0, 2.0, -1.0), [1.0, 1.0, 0.0]));
        assert!(near3(clip(-4.0, -2.0, -3.0), [-1.0, -1.0, 1.0]));
        assert!(near3(clip(0.0, 0.0, -2.0), [0.0, 0.0, 0.5]));
    }

    #[test]
    fn a_camera_that_cannot_project_sees_nothing() {
        let bad = |p: Projection| Camera { projection: p, ..Camera::default() }.view_projection(1.0);
        assert!(bad(Projection::Perspective { fov_y: 0.0, near: 0.1, far: 10.0 }).is_none());
        assert!(bad(Projection::Perspective { fov_y: PI, near: 0.1, far: 10.0 }).is_none());
        assert!(bad(Projection::Perspective { fov_y: 1.0, near: 0.0, far: 10.0 }).is_none());
        assert!(bad(Projection::Perspective { fov_y: 1.0, near: 5.0, far: 5.0 }).is_none());
        assert!(bad(Projection::Perspective { fov_y: f32::NAN, near: 0.1, far: 10.0 }).is_none());
        assert!(bad(Projection::Orthographic { height: -1.0, near: 0.0, far: 1.0 }).is_none());
        assert!(Camera::default().view_projection(0.0).is_none());
        assert!(Camera::default().view_projection(f32::NAN).is_none());
        assert!(Camera::default().view_projection(1.5).is_some());
        let mut away = Camera::default();
        away.transform.translation = [f32::INFINITY, 0.0, 0.0];
        assert!(away.view_projection(1.0).is_none());
    }

    #[test]
    fn the_clear_colour_is_premultiplied_for_the_target() {
        let mut cam = Camera::default();
        assert_eq!(cam.clear_premultiplied(), Some([0.0; 4]));
        cam.clear = Some([1.0, 0.5, 0.0, 0.5]);
        assert_eq!(cam.clear_premultiplied(), Some([0.5, 0.25, 0.0, 0.5]));
        cam.clear = None;
        assert_eq!(cam.clear_premultiplied(), None);
    }

    fn node(id: u64, parent: Option<u64>, at: [f32; 3]) -> Node {
        let mut n = Node::new(id, MeshRef::Cube, Material::default());
        n.parent = parent;
        n.transform = Transform::from_translation(at);
        n
    }

    fn scene(nodes: Vec<Node>) -> Scene {
        Scene { camera: Camera::default(), nodes, look: None }
    }

    #[test]
    fn a_parent_chain_composes_parent_first() {
        let mut turned = node(1, None, [10.0, 0.0, 0.0]);
        turned.transform.rotation = quat_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2);
        let s = scene(vec![node(3, Some(2), [1.0, 0.0, 0.0]), node(2, Some(1), [0.0, 1.0, 0.0]), turned]);
        let world = s.world_matrices();
        assert!(near3(mat_point(&world[1].unwrap(), [0.0; 3]), [9.0, 0.0, 0.0]));
        assert!(near3(mat_point(&world[0].unwrap(), [0.0; 3]), [9.0, 1.0, 0.0]));
        assert!(near3(mat_point(&world[2].unwrap(), [0.0; 3]), [10.0, 0.0, 0.0]));
    }

    #[test]
    fn a_missing_parent_is_not_drawn_and_does_not_panic() {
        let s = scene(vec![node(1, None, [0.0; 3]), node(2, Some(99), [0.0; 3]), node(3, Some(2), [0.0; 3])]);
        let world = s.world_matrices();
        assert!(world[0].is_some() && world[1].is_none() && world[2].is_none());
        assert_eq!(s.draw_list().iter().map(|d| d.node).collect::<Vec<_>>(), [0]);
    }

    #[test]
    fn a_loop_is_not_drawn_and_does_not_hang() {
        let s = scene(vec![
            node(1, Some(3), [0.0; 3]),
            node(2, Some(1), [0.0; 3]),
            node(3, Some(2), [0.0; 3]),
            node(4, Some(4), [0.0; 3]),
            node(5, Some(2), [0.0; 3]),
            node(6, None, [0.0; 3]),
        ]);
        let world = s.world_matrices();
        assert_eq!(
            world.iter().map(Option::is_some).collect::<Vec<_>>(),
            [false, false, false, false, false, true]
        );
        assert_eq!(s.draw_list().len(), 1);
    }

    #[test]
    fn a_taken_id_is_not_drawn_and_the_first_keeps_it() {
        let s = scene(vec![node(1, None, [0.0; 3]), node(1, None, [5.0, 0.0, 0.0]), node(2, Some(1), [0.0; 3])]);
        let world = s.world_matrices();
        assert!(world[0].is_some() && world[1].is_none() && world[2].is_some());
    }

    #[test]
    fn hiding_a_node_hides_what_hangs_from_it() {
        let mut hidden = node(1, None, [0.0; 3]);
        hidden.visible = false;
        let group = {
            let mut g = Node::empty(7);
            g.visible = false;
            g
        };
        let s = scene(vec![hidden, node(2, Some(1), [0.0; 3]), group, node(3, Some(7), [0.0; 3]), node(4, None, [0.0; 3])]);
        assert_eq!(s.draw_list().iter().map(|d| d.node).collect::<Vec<_>>(), [4]);
        let s = scene(vec![Node::empty(1), node(2, Some(1), [0.0; 3])]);
        assert_eq!(s.draw_list().iter().map(|d| d.node).collect::<Vec<_>>(), [1]);
    }

    #[test]
    fn a_number_that_is_not_finite_is_not_drawn() {
        let s = scene(vec![node(1, None, [f32::NAN, 0.0, 0.0]), node(2, Some(1), [0.0; 3]), node(3, None, [0.0; 3])]);
        assert_eq!(s.draw_list().iter().map(|d| d.node).collect::<Vec<_>>(), [2]);
    }

    #[test]
    fn a_long_chain_costs_a_walk_not_a_recursion() {
        let mut nodes = vec![node(0, None, [1.0, 0.0, 0.0])];
        for i in 1..20_000u64 {
            nodes.push(node(i, Some(i - 1), [1.0, 0.0, 0.0]));
        }
        nodes.reverse();
        let world = scene(nodes).world_matrices();
        assert!(near(world[0].unwrap()[3][0], 20_000.0));
    }

    #[test]
    fn nodes_past_the_limit_are_ignored() {
        let nodes: Vec<Node> = (0..MAX_NODES as u64 + 5).map(|i| node(i, None, [0.0; 3])).collect();
        let s = scene(nodes);
        assert_eq!(s.draw_list().len(), MAX_NODES);
        assert_eq!(s.world_matrices().len(), MAX_NODES + 5);
    }

    fn length(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    fn faces_outward(mesh: &MeshData) -> bool {
        mesh.indices.chunks(3).all(|t| {
            let p = |k: usize| mesh.positions[t[k] as usize];
            let (a, b, c) = (p(0), p(1), p(2));
            let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let vn = mesh.normals[t[0] as usize];
            length(n) > 1e-9 && n[0] * vn[0] + n[1] * vn[1] + n[2] * vn[2] > 0.0
        })
    }

    fn well_made(mesh: &MeshData) {
        assert_eq!(mesh.validate(), Ok(()));
        assert!(mesh.normals.iter().all(|n| (length(*n) - 1.0).abs() < 1e-5), "unit normals");
        assert!(mesh.uvs.iter().all(|uv| uv.iter().all(|v| (0.0..=1.0).contains(v))), "uvs in 0..1");
        assert!(faces_outward(mesh), "counter-clockwise from outside");
    }

    #[test]
    fn the_quad_is_a_unit_square_facing_plus_z() {
        let m = MeshData::quad();
        well_made(&m);
        assert_eq!((m.positions.len(), m.indices.len()), (4, 6));
        assert!(m.positions.iter().all(|p| p[2] == 0.0 && p[0].abs() == 0.5 && p[1].abs() == 0.5));
        assert!(m.normals.iter().all(|n| *n == [0.0, 0.0, 1.0]));
        assert_eq!((m.positions[0], m.uvs[0]), ([-0.5, 0.5, 0.0], [0.0, 0.0]));
        assert_eq!((m.positions[2], m.uvs[2]), ([0.5, -0.5, 0.0], [1.0, 1.0]));
    }

    #[test]
    fn the_plane_is_a_unit_square_facing_plus_y() {
        let m = MeshData::plane();
        well_made(&m);
        assert_eq!((m.positions.len(), m.indices.len()), (4, 6));
        assert!(m.positions.iter().all(|p| p[1] == 0.0 && p[0].abs() == 0.5 && p[2].abs() == 0.5));
        assert!(m.normals.iter().all(|n| *n == [0.0, 1.0, 0.0]));
        assert_eq!(m.positions[0], [-0.5, 0.0, -0.5]);
    }

    #[test]
    fn the_cube_has_four_vertices_to_a_face_in_a_fixed_order() {
        let m = MeshData::cube();
        well_made(&m);
        assert_eq!((m.positions.len(), m.indices.len()), (24, 36));
        assert_eq!(m.groups, [(0, 6), (6, 6), (12, 6), (18, 6), (24, 6), (30, 6)]);
        for (i, face) in CUBE_FACES.iter().enumerate() {
            for v in 4 * i..4 * i + 4 {
                assert_eq!(m.normals[v], face.normal, "{} normal", face.name);
                let p = m.positions[v];
                let along = p[0] * face.normal[0] + p[1] * face.normal[1] + p[2] * face.normal[2];
                assert!((along - 0.5).abs() < 1e-6, "{} lies on its side", face.name);
            }
            let (first, count) = m.groups[i];
            for &index in &m.indices[first as usize..(first + count) as usize] {
                assert!((4 * i as u32..4 * i as u32 + 4).contains(&index), "{} group", face.name);
            }
            let uvs = &m.uvs[4 * i..4 * i + 4];
            assert_eq!(uvs, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            let (tl, tr, bl) = (m.positions[4 * i], m.positions[4 * i + 1], m.positions[4 * i + 3]);
            let dir = |a: [f32; 3], b: [f32; 3]| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            assert_eq!(dir(tl, tr), face.right, "{} right", face.name);
            assert_eq!(dir(bl, tl), face.up, "{} up", face.name);
        }
        let names: Vec<&str> = CUBE_FACES.iter().map(|f| f.name).collect();
        assert_eq!(names, ["+X", "-X", "+Y", "-Y", "+Z", "-Z"]);
        for face in &CUBE_FACES {
            let (r, u) = (face.right, face.up);
            let cross = [r[1] * u[2] - r[2] * u[1], r[2] * u[0] - r[0] * u[2], r[0] * u[1] - r[1] * u[0]];
            assert_eq!(cross, face.normal, "{}", face.name);
        }
    }

    #[test]
    fn a_sphere_is_round_and_clamped() {
        let m = MeshData::sphere(16);
        well_made(&m);
        assert_eq!(m.positions.len(), (16 + 1) * (8 + 1));
        assert!(m.positions.iter().all(|p| (length(*p) - 0.5).abs() < 1e-5), "radius a half");
        assert_eq!(m.indices.len(), 3 * (16 * 8 * 2 - 2 * 16));
        let front = m
            .positions
            .iter()
            .zip(&m.uvs)
            .find(|(_, uv)| (uv[0] - 0.0).abs() < 1e-6 && (uv[1] - 0.5).abs() < 1e-6)
            .unwrap();
        assert!(near3(*front.0, [0.0, 0.0, 0.5]));
        let low = MeshData::sphere(0);
        well_made(&low);
        assert_eq!(low.positions.len(), (3 + 1) * (2 + 1));
        let high = MeshData::sphere(u16::MAX);
        well_made(&high);
        assert_eq!(high.positions.len(), (128 + 1) * (64 + 1));
    }

    #[test]
    fn built_ins_come_from_their_ref_and_uploaded_ones_do_not() {
        assert_eq!(MeshRef::Quad.generate(), Some(MeshData::quad()));
        assert_eq!(MeshRef::Cube.generate(), Some(MeshData::cube()));
        assert_eq!(MeshRef::Plane.generate(), Some(MeshData::plane()));
        assert_eq!(MeshRef::Sphere { segments: 8 }.generate(), Some(MeshData::sphere(8)));
        assert_eq!(MeshRef::Uploaded(3).generate(), None);
    }

    fn triangle() -> MeshData {
        MeshData {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..MeshData::default()
        }
    }

    #[test]
    fn a_mesh_with_only_positions_and_indices_is_valid() {
        assert_eq!(triangle().validate(), Ok(()));
    }

    #[test]
    fn a_bad_mesh_is_refused_with_why() {
        let with = |f: &dyn Fn(&mut MeshData)| {
            let mut m = triangle();
            f(&mut m);
            m.validate()
        };
        assert_eq!(MeshData::default().validate(), Err(MeshError::Empty));
        assert_eq!(with(&|m| m.indices.clear()), Err(MeshError::Empty));
        assert_eq!(with(&|m| m.normals = vec![[0.0, 0.0, 1.0]; 2]), Err(MeshError::AttributeLength("normals")));
        assert_eq!(with(&|m| m.uvs = vec![[0.0; 2]; 4]), Err(MeshError::AttributeLength("uvs")));
        assert_eq!(with(&|m| m.colors = vec![[1.0; 4]; 1]), Err(MeshError::AttributeLength("colors")));
        assert_eq!(with(&|m| m.indices.push(0)), Err(MeshError::NotTriangles));
        assert_eq!(with(&|m| m.indices[2] = 3), Err(MeshError::IndexOutOfRange));
        assert_eq!(with(&|m| m.positions[1][0] = f32::NAN), Err(MeshError::NotFinite));
        assert_eq!(with(&|m| m.positions[1][0] = f32::INFINITY), Err(MeshError::NotFinite));
        assert_eq!(with(&|m| m.uvs = vec![[f32::NAN; 2]; 3]), Err(MeshError::NotFinite));
        assert_eq!(with(&|m| m.groups = vec![(0, 6)]), Err(MeshError::BadGroup));
        assert_eq!(with(&|m| m.groups = vec![(3, 3)]), Err(MeshError::BadGroup));
        assert_eq!(with(&|m| m.groups = vec![(0, 0)]), Err(MeshError::BadGroup));
        assert_eq!(with(&|m| m.groups = vec![(1, 2)]), Err(MeshError::BadGroup));
        assert_eq!(with(&|m| m.groups = vec![(u32::MAX, 3)]), Err(MeshError::BadGroup));
        assert_eq!(with(&|m| m.groups = vec![(0, 3)]), Ok(()));
    }

    #[test]
    fn an_oversize_mesh_is_refused() {
        let mut m = triangle();
        m.positions = vec![[0.0; 3]; MAX_MESH_VERTICES + 1];
        assert_eq!(m.validate(), Err(MeshError::TooManyVertices));
        let mut m = triangle();
        m.indices = vec![0; MAX_MESH_INDICES + 3];
        assert_eq!(m.validate(), Err(MeshError::TooManyIndices));
        let mut m = triangle();
        m.positions = vec![[0.0; 3]; MAX_MESH_VERTICES];
        m.indices = vec![0; MAX_MESH_INDICES];
        assert_eq!(m.validate(), Ok(()), "exactly the limit is fine");
    }

    #[test]
    fn the_centre_is_the_middle_of_the_box() {
        assert_eq!(MeshData::cube().centre(), [0.0; 3]);
        assert_eq!(triangle().centre(), [0.5, 0.5, 0.0]);
        assert_eq!(MeshData::default().centre(), [0.0; 3]);
    }

    #[test]
    fn a_colour_with_alpha_blends() {
        assert_eq!(Material::color([1.0, 0.0, 0.0, 1.0]).alpha, AlphaMode::Opaque);
        assert_eq!(Material::color([1.0, 0.0, 0.0, 0.5]).alpha, AlphaMode::Blend);
        assert_eq!(Material::picture(7).source, TextureSource::Picture(7));
        assert!(AlphaMode::Blend.blended() && AlphaMode::Premultiplied.blended() && !AlphaMode::Opaque.blended());
    }

    #[test]
    fn a_bad_tint_is_made_usable() {
        let m = Material { tint: [2.0, -1.0, f32::NAN, 0.5], ..Material::default() };
        assert_eq!(m.tint_clamped(), [1.0, 0.0, 1.0, 0.5]);
    }

    #[test]
    fn a_round_cube_is_six_flat_faces_then_its_rounded_edges_and_corners() {
        for (radius, segments) in [(120u16, 4u16), (250, 1), (40, 16), (499, 7), (500, 3)] {
            let m = MeshData::round_cube(radius, segments);
            if radius < ROUND_RADIUS {
                well_made(&m);
            } else {
                assert_eq!(m.validate(), Ok(()));
            }
            let n = usize::from(segments);
            assert_eq!(m.positions.len(), 6 * 4 + 12 * 2 * (n + 1) + 8 * (n + 1) * (n + 1), "{radius} {segments}");
            let round = 12 * 6 * n + 8 * 3 * (2 * n * n - n);
            assert_eq!(m.indices.len(), 36 + round, "{radius} {segments}");
            assert_eq!(m.groups.len(), 7);
            assert_eq!(&m.groups[..6], &[(0, 6), (6, 6), (12, 6), (18, 6), (24, 6), (30, 6)]);
            assert_eq!(m.groups[6], (36, round as u32), "group 6 is every rounded triangle");
            let r = f32::from(radius) / 1000.0;
            let h = 0.5 - r;
            for (i, face) in CUBE_FACES.iter().enumerate() {
                for v in 4 * i..4 * i + 4 {
                    assert_eq!(m.normals[v], face.normal);
                    let p = m.positions[v];
                    assert!((p[0] * face.normal[0] + p[1] * face.normal[1] + p[2] * face.normal[2] - 0.5).abs() < 1e-6);
                }
                assert_eq!(&m.uvs[4 * i..4 * i + 4], &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
                let (tl, tr, bl) = (m.positions[4 * i], m.positions[4 * i + 1], m.positions[4 * i + 3]);
                for k in 0..3 {
                    assert!((tr[k] - tl[k] - 2.0 * h * face.right[k]).abs() < 1e-6, "{} right", face.name);
                    assert!((tl[k] - bl[k] - 2.0 * h * face.up[k]).abs() < 1e-6, "{} up", face.name);
                }
            }
            for (p, nrm) in m.positions.iter().zip(&m.normals).skip(24) {
                let inner = p.map(|v| v.clamp(-h, h));
                for k in 0..3 {
                    assert!((p[k] - inner[k] - r * nrm[k]).abs() < 1e-5, "{p:?} {nrm:?}");
                    assert!(p[k].abs() <= 0.5 + 1e-6);
                }
            }
            assert!(m.uvs.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
            assert!(m.uvs[24..].iter().any(|uv| uv[0] == 1.0) && m.uvs[24..].iter().any(|uv| uv[1] == 1.0));
            let reach = m.positions.iter().flatten().fold(0.0f32, |a, v| a.max(v.abs()));
            assert!((reach - 0.5).abs() < 1e-6);
        }
        assert_eq!(MeshData::round_cube(9999, 99), MeshData::round_cube(500, 16));
        assert_eq!(MeshData::round_cube(120, 0), MeshData::round_cube(120, 1));
        let sharp = MeshData::round_cube(0, 4);
        assert_eq!(sharp.validate(), Ok(()));
        assert_eq!(sharp.groups.len(), 7);
        assert_eq!(MeshRef::RoundCube { radius: 120, segments: 4 }.generate(), Some(MeshData::round_cube(120, 4)));
    }

    fn two_joints() -> Scene {
        let a = Node::empty(10);
        let mut b = Node::empty(11);
        b.parent = Some(10);
        b.transform = Transform::from_translation([0.0, 1.0, 0.0]);
        let mut mesh = Node::new(12, MeshRef::Uploaded(1), Material::default());
        mesh.skin = Some(Skin {
            joints: Arc::from([10u64, 11]),
            inverse_bind: Arc::from([IDENTITY_MATRIX, Transform::from_translation([0.0, -1.0, 0.0]).matrix()]),
        });
        scene(vec![a, b, mesh])
    }

    #[test]
    fn a_two_joint_skin_bends_as_worked_by_hand() {
        let mut s = two_joints();
        let rest = s.skin_matrices(2).expect("a skin");
        for m in &rest {
            assert!((0..4).all(|c| (0..4).all(|r| near(m[c][r], IDENTITY_MATRIX[c][r]))));
        }
        s.nodes[1].transform.rotation = quat_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2);
        let m = s.skin_matrices(2).expect("a skin");
        let fallback = IDENTITY_MATRIX;
        let v = [1.0, 1.0, 0.0];
        let at = |joints: [u16; 4], weights: [f32; 4]| mat_point(&skin_matrix(&m, joints, weights, &fallback), v);
        assert!(near3(at([1, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]), [0.0, 2.0, 0.0]));
        assert!(near3(at([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]), [1.0, 1.0, 0.0]));
        assert!(near3(at([0, 1, 0, 0], [0.5, 0.5, 0.0, 0.0]), [0.5, 1.5, 0.0]));
        assert!(near3(at([0, 1, 0, 0], [0.25, 0.75, 0.0, 0.0]), [0.25, 1.75, 0.0]));
        assert!(near3(at([0, 1, 0, 0], [2.0, 2.0, 0.0, 0.0]), [0.5, 1.5, 0.0]));
        assert!(near3(at([1, 9, 0, 0], [0.5, 0.5, 0.0, 0.0]), [0.0, 2.0, 0.0]));
        assert!(near3(at([0, 1, 0, 0], [f32::NAN, 1.0, 0.0, 0.0]), [0.0, 2.0, 0.0]));
        let node = Transform::from_translation([5.0, 0.0, 0.0]).matrix();
        assert!(near3(mat_point(&skin_matrix(&m, [0; 4], [0.0; 4], &node), v), [6.0, 1.0, 0.0]));
        s.nodes[0].transform.translation = [2.0, 0.0, 0.0];
        let m = s.skin_matrices(2).expect("a skin");
        assert!(near3(mat_point(&skin_matrix(&m, [0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0], &fallback), v), [3.0, 1.0, 0.0]));
        assert!(near3(mat_point(&skin_matrix(&m, [1, 0, 0, 0], [1.0, 0.0, 0.0, 0.0], &fallback), v), [2.0, 2.0, 0.0]));
    }

    #[test]
    fn a_skin_at_rest_draws_the_mesh_where_its_node_would() {
        let invert = |t: &Transform| {
            let inv_q = [-t.rotation[0], -t.rotation[1], -t.rotation[2], t.rotation[3]];
            let back = Transform { rotation: inv_q, ..Transform::IDENTITY }.matrix();
            mat_mul(&back, &Transform::from_translation(t.translation.map(|v| -v)).matrix())
        };
        let mut root = Node::empty(1);
        root.transform = Transform { translation: [3.0, 0.0, -2.0], rotation: quat_axis_angle([1.0, 0.0, 0.0], FRAC_PI_2), scale: [0.01; 3] };
        let mut hip = Node::empty(2);
        hip.parent = Some(1);
        hip.transform = Transform { translation: [0.0, 0.0, 95.0], rotation: quat_axis_angle([0.3, 1.0, 0.2], 0.7), scale: [1.0; 3] };
        let mut arm = Node::empty(3);
        arm.parent = Some(2);
        arm.transform = Transform { translation: [20.0, 5.0, 40.0], rotation: quat_axis_angle([0.0, 0.0, 1.0], -1.1), scale: [1.0; 3] };
        let hip_ibm = invert(&hip.transform);
        let arm_ibm = mat_mul(&invert(&arm.transform), &invert(&hip.transform));
        let mut mesh = Node::new(4, MeshRef::Uploaded(1), Material::default());
        mesh.parent = Some(1);
        mesh.skin = Some(Skin { joints: Arc::from([2u64, 3]), inverse_bind: Arc::from([hip_ibm, arm_ibm]) });
        let s = scene(vec![root, hip, arm, mesh]);
        let m = s.skin_matrices(3).expect("a skin");
        let mesh_world = s.world_matrices()[3].expect("placed");
        for (p, joints, weights) in [
            ([10.0, -4.0, 120.0], [0, 1, 0, 0], [0.3, 0.7, 0.0, 0.0]),
            ([-25.0, 2.0, 60.0], [1, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
            ([0.0, 0.0, 0.0], [0, 1, 1, 0], [0.2, 0.4, 0.4, 0.0]),
        ] {
            let bent = mat_point(&skin_matrix(&m, joints, weights, &mesh_world), p);
            let still = mat_point(&mesh_world, p);
            assert!((0..3).all(|k| (bent[k] - still[k]).abs() < 1e-4), "{bent:?} != {still:?}");
        }
    }

    #[test]
    fn a_skin_that_cannot_bend_says_so() {
        let s = two_joints();
        let worlds = s.world_matrices();
        let world_of = |id: u64| worlds.get(id.checked_sub(10)? as usize).copied().flatten();
        let skin = s.nodes[2].skin.clone().expect("a skin");
        assert!(skin_matrices(&skin, world_of).is_some());
        let missing = Skin { joints: Arc::from([10u64, 99]), ..skin.clone() };
        assert_eq!(skin_matrices(&missing, world_of), None);
        let short = Skin { inverse_bind: Arc::from([IDENTITY_MATRIX]), ..skin.clone() };
        assert_eq!(skin_matrices(&short, world_of), None);
        let none = Skin { joints: Arc::from([]), inverse_bind: Arc::from([]) };
        assert_eq!(skin_matrices(&none, world_of), None);
        let many = Skin { joints: vec![10u64; MAX_JOINTS + 1].into(), inverse_bind: vec![IDENTITY_MATRIX; MAX_JOINTS + 1].into() };
        assert_eq!(skin_matrices(&many, world_of), None);
        let most = Skin { joints: vec![10u64; MAX_JOINTS].into(), inverse_bind: vec![IDENTITY_MATRIX; MAX_JOINTS].into() };
        assert_eq!(skin_matrices(&most, world_of).map(|m| m.len()), Some(MAX_JOINTS));
        assert_eq!(s.skin_matrices(0), None);
        assert_eq!(s.skin_matrices(99), None);
        let mut m = triangle();
        m.joints = vec![[0; 4]; 3];
        assert_eq!(m.validate(), Err(MeshError::AttributeLength("weights")));
        m.weights = vec![[1.0, 0.0, 0.0, 0.0]; 2];
        assert_eq!(m.validate(), Err(MeshError::AttributeLength("weights")));
        m.weights = vec![[1.0, 0.0, 0.0, 0.0]; 3];
        assert_eq!(m.validate(), Ok(()));
        m.weights[1][2] = f32::INFINITY;
        assert_eq!(m.validate(), Err(MeshError::NotFinite));
    }

    #[test]
    fn a_finish_is_made_usable() {
        let m = Material { chrome: 2.0, liquid: f32::NAN, glow: 0.25, ..Material::default() };
        assert_eq!(m.finish_clamped(), [1.0, 0.0, 0.25]);
        assert_eq!(Material::default().finish_clamped(), [0.0; 3]);
    }

    #[test]
    fn a_cube_with_a_picture_on_each_face_can_be_said() {
        let mut cube = Node::new(1, MeshRef::Cube, Material::default());
        cube.face_materials = (0..6).map(|i| Material::picture(100 + i)).collect();
        let s = scene(vec![cube]);
        assert_eq!(s.draw_list().len(), 1);
        let ids: Vec<u32> = s.nodes[0]
            .face_materials
            .iter()
            .map(|m| match m.source {
                TextureSource::Picture(id) => id,
                TextureSource::None => 0,
            })
            .collect();
        assert_eq!(ids, [100, 101, 102, 103, 104, 105]);
    }

    #[test]
    fn a_material_is_unlit_unless_it_says_so() {
        let m = Material::default();
        assert!(!m.lit && m.emissive == 0.0);
        let lit = Material::lit([1.0, 0.5, 0.0, 1.0], 0.25);
        assert!(lit.lit && lit.emissive == 0.25 && lit.alpha == AlphaMode::Opaque);
        assert_eq!(lit.tint, [1.0, 0.5, 0.0, 1.0]);
        let wild = |emissive| Material { emissive, ..Material::default() }.emissive_clamped();
        assert_eq!((wild(2.0), wild(-1.0), wild(f32::NAN), wild(0.5)), (1.0, 0.0, 0.0, 0.5));
    }

    #[test]
    fn the_key_light_is_a_unit_vector_from_above_left_and_in_front() {
        assert!((length(KEY_LIGHT) - 1.0).abs() < 0.01);
        const { assert!(KEY_LIGHT[0] < 0.0 && KEY_LIGHT[1] > 0.0 && KEY_LIGHT[2] > 0.0) };
        const { assert!(AMBIENT_LIGHT > 0.0 && AMBIENT_LIGHT < 1.0) };
    }

    #[test]
    fn a_gem_is_an_octahedron_of_flat_faces() {
        let m = MeshData::gem();
        assert_eq!(m.validate(), Ok(()));
        assert!(m.normals.iter().all(|n| (length(*n) - 1.0).abs() < 1e-5), "unit normals");
        assert!(faces_outward(&m), "counter-clockwise from outside");
        assert_eq!((m.positions.len(), m.indices.len()), (24, 24));
        let mut points: Vec<[i32; 3]> = m
            .positions
            .iter()
            .map(|p| p.map(|v| (v * 2.0).round() as i32))
            .collect();
        points.sort_unstable();
        points.dedup();
        assert_eq!(points.len(), 6);
        assert!(points.iter().all(|p| p.iter().map(|v| v.abs()).sum::<i32>() == 1));
        for face in m.normals.chunks(3) {
            assert!(face.iter().all(|n| *n == face[0]));
        }
    }

    #[test]
    fn a_cone_points_up_and_stands_on_a_fan() {
        let m = MeshData::cone(6);
        assert_eq!(m.validate(), Ok(()));
        assert!(m.normals.iter().all(|n| (length(*n) - 1.0).abs() < 1e-5), "unit normals");
        assert!(faces_outward(&m), "counter-clockwise from outside");
        assert_eq!(m.positions.len(), 6 * 3 + 1 + 7);
        assert_eq!(m.indices.len(), 3 * (6 + 6));
        let top = m.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        let bottom = m.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert_eq!((top, bottom), (0.5, -0.5));
        assert!(m.positions.iter().all(|p| p[0].hypot(p[2]) <= 0.5 + 1e-6));
        for n in [0, u16::MAX] {
            let c = MeshData::cone(n);
            assert_eq!(c.validate(), Ok(()));
            assert!(faces_outward(&c));
        }
        assert_eq!(MeshData::cone(0).positions.len(), 3 * 3 + 1 + 4);
        assert_eq!(MeshRef::Gem.generate(), Some(MeshData::gem()));
        assert_eq!(MeshRef::Cone { segments: 6 }.generate(), Some(MeshData::cone(6)));
    }

    fn seen(camera: &Camera, size: (u32, u32), p: [f32; 3]) -> ([f32; 2], f32) {
        let m = camera.view_projection(size.0 as f32 / size.1 as f32).expect("a camera that sees");
        let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
        let (x, y, z, w) = (row(0), row(1), row(2), row(3));
        ([(x / w + 1.0) * 0.5 * size.0 as f32, (1.0 - y / w) * 0.5 * size.1 as f32], z / w)
    }

    fn layer_maths(tilt: Option<Tilt>, foot: [f32; 2], height: f32) -> [f32; 2] {
        match tilt {
            Some(t) => {
                let [x, y] = t.floor(foot);
                [x, y - height * t.scale_at(foot)]
            }
            None => [foot[0], foot[1] - height],
        }
    }

    #[test]
    fn a_prop_lands_where_an_upright_layer_lifted_to_it_does() {
        for size in [(384u32, 224u32), (768, 448), (800, 600)] {
            for tilt in [0.0f32, 0.05, 0.3, 0.45, crate::MAX_TILT] {
                let floor = Tilt::for_stage(tilt, size);
                assert_eq!(floor.is_some(), tilt > 0.0);
                let camera = stage_camera(tilt, size);
                let mut worst = 0.0f32;
                let mut checked = 0;
                for gx in 0..=12 {
                    for gy in 0..=8 {
                        let foot = [size.0 as f32 * gx as f32 / 12.0, size.1 as f32 * gy as f32 / 8.0];
                        for height in [0.0, 1.0, 4.0, 11.5, 30.0, 90.0] {
                            let want = layer_maths(floor, foot, height);
                            let (got, _) = seen(&camera, size, [foot[0], height, foot[1]]);
                            let off = (got[0] - want[0]).hypot(got[1] - want[1]);
                            assert!(off < 0.01, "tilt {tilt} {size:?} at {foot:?} lifted {height}: {got:?} for {want:?}");
                            worst = worst.max(off);
                            checked += 1;
                        }
                    }
                }
                assert_eq!(checked, 13 * 9 * 6);
                println!("tilt {tilt} over {size:?}: {checked} points, worst {worst:.5} px");
            }
        }
    }

    #[test]
    fn a_flat_stage_camera_is_the_pixels_themselves() {
        let size = (384, 224);
        let camera = stage_camera(0.0, size);
        let (floor, _) = seen(&camera, size, [100.0, 0.0, 50.0]);
        assert!((floor[0] - 100.0).abs() < 1e-3 && (floor[1] - 50.0).abs() < 1e-3);
        let (lifted, _) = seen(&camera, size, [100.0, 12.0, 50.0]);
        assert!((lifted[0] - 100.0).abs() < 1e-4 && (lifted[1] - 38.0).abs() < 1e-4);
        for tilt in [f32::NAN, -1.0] {
            assert_eq!(stage_camera(tilt, size).projection, camera.projection);
        }
        assert!(stage_camera(0.3, (0, 0)).view_projection(1.0).is_some());
    }

    #[test]
    fn the_stage_camera_is_the_same_whatever_the_target_is_shaped_like() {
        let camera = stage_camera(0.3, (384, 224));
        assert_eq!(camera.view_projection(1.0), camera.view_projection(3.0));
        assert_eq!(camera.view_projection(0.5), camera.view_projection(384.0 / 224.0));
        let bad = Camera { projection: Projection::Custom([[f32::NAN; 4]; 4]), ..Camera::default() };
        assert_eq!(bad.view_projection(1.0), None);
    }

    #[test]
    fn nearer_props_are_nearer_in_depth_and_level_ones_are_level() {
        let size = (384, 224);
        for tilt in [0.0, 0.3, crate::MAX_TILT] {
            let camera = stage_camera(tilt, size);
            let depth = |x: f32, h: f32, y: f32| seen(&camera, size, [x, h, y]).1;
            let mut last = f32::MAX;
            for y in (-40..=260).step_by(1) {
                let z = depth(192.0, 4.0, y as f32);
                assert!((0.0..1.0).contains(&z), "tilt {tilt} y {y}: {z}");
                assert!(z < last, "tilt {tilt} y {y}: {z} not nearer than {last}");
                last = z;
            }
            let z = depth(10.0, 0.0, 100.0);
            for (x, h) in [(300.0, 0.0), (10.0, 40.0), (200.0, 12.0)] {
                assert!((depth(x, h, 100.0) - z).abs() < 1e-6, "tilt {tilt}");
            }
            assert!(depth(0.0, 0.0, 100.0) - depth(0.0, 0.0, 101.0) > 1e-5);
        }
    }

    #[test]
    fn a_projector_casts_its_picture_by_its_clip_space() {
        let size = (384, 224);
        let camera = stage_camera(0.3, size);
        let projector = Projector::of(&camera, 384.0 / 224.0).unwrap();
        for p in [[100.0, 0.0, 50.0], [300.0, 40.0, 200.0], [192.0, 12.0, 112.0]] {
            let (pixel, _) = seen(&camera, size, p);
            let uv = projector.uv(p).unwrap();
            assert!((uv[0] * 384.0 - pixel[0]).abs() < 1e-2 && (uv[1] * 224.0 - pixel[1]).abs() < 1e-2, "{p:?}");
        }
        assert_eq!(projector.uv([-50.0, 0.0, 100.0]), None);
        let pinhole = Projector::of(&Camera::default(), 1.0).unwrap();
        assert!(pinhole.uv([0.0, 0.0, 0.0]).is_some());
        assert_eq!(pinhole.uv([0.0, 0.0, 5.0]), None);
        assert_eq!(Projector { view_projection: [[f32::NAN; 4]; 4] }.uv([0.0; 3]), None);
        let m = Material::projected(7, projector, [0.2, 0.4, 0.6, 1.0]);
        assert_eq!((m.source, m.projector, m.tint, m.alpha, m.lit), (TextureSource::Picture(7), Some(projector), [0.2, 0.4, 0.6, 1.0], AlphaMode::Opaque, false));
        assert_eq!(Material::default().projector, None);
    }

    #[test]
    fn props_are_nodes_between_layers() {
        let props = Props { at: 3, grain: 0, nodes: vec![Node::new(1, MeshRef::Gem, Material::default())], world: None };
        assert_eq!(props.grain_clamped(), 1);
        assert_eq!(Props { grain: 200, ..props.clone() }.grain_clamped(), u32::from(MAX_GRAIN));
        assert_eq!(Props { grain: 3, ..props }.grain_clamped(), 3);
        let scene = Scene {
            camera: stage_camera(0.3, (384, 224)),
            nodes: vec![Node::new(1, MeshRef::Gem, Material::default())],
            look: None,
        };
        assert_eq!(scene.draw_list().len(), 1);
        assert!(scene.camera.view_projection(384.0 / 224.0).is_some());
    }
}
