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

use crate::Tilt;
pub use crate::look::{Dither, Fog, Lamp, Look, LookError, Reveal, Shadow, Sun, Zone};

/// A 4x4 matrix, column-major: `m[column][row]`.
pub type Mat4 = [[f32; 4]; 4];

/// The matrix that changes nothing.
pub const IDENTITY_MATRIX: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// The most nodes a scene draws; later ones are ignored.
pub const MAX_NODES: usize = 65_536;

/// The most vertices and indices one mesh may carry (two triangles to a
/// vertex). A vertex is 48 bytes on the GPU, so a mesh at the limit is about
/// 19 MB.
pub const MAX_MESH_VERTICES: usize = 1 << 18;
pub const MAX_MESH_INDICES: usize = 3 << 19;

/// The fewest and most longitudes a sphere is made with.
pub const SPHERE_SEGMENTS: std::ops::RangeInclusive<u16> = 3..=128;

// ---------------------------------------------------------------- maths ----

/// `a * b`: the transform that applies `b` first, then `a`.
pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (c, column) in out.iter_mut().enumerate() {
        for (r, value) in column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

/// `m` applied to a point (w = 1), with the perspective divide left to the
/// caller: for an affine matrix the fourth row is already (0, 0, 0, 1).
pub fn mat_point(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    let at = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
    [at(0), at(1), at(2)]
}

/// Whether every entry is a finite number.
pub fn mat_finite(m: &Mat4) -> bool {
    m.iter().flatten().all(|v| v.is_finite())
}

/// The identity rotation, as a quaternion `[x, y, z, w]`.
pub const QUAT_IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// `a * b` for quaternions `[x, y, z, w]`: the rotation that applies `b`
/// first, then `a`.
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

/// `q` scaled to length 1; the identity when it has no usable length.
pub fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len.is_finite() && len > 1e-12 {
        [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
    } else {
        QUAT_IDENTITY
    }
}

/// The turn of `angle` radians about `axis` (counter-clockwise looking down
/// the axis toward the origin, the right-hand rule). An axis with no length
/// is no turn.
pub fn quat_axis_angle(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if !(len.is_finite() && len > 1e-12 && angle.is_finite()) {
        return QUAT_IDENTITY;
    }
    let (s, c) = (angle * 0.5).sin_cos();
    let k = s / len;
    [axis[0] * k, axis[1] * k, axis[2] * k, c]
}

/// `v` turned by the unit quaternion `q`.
pub fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // v' = v + 2w (u x v) + 2 u x (u x v), u the vector part.
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

/// The 3x3 turn of a unit quaternion as three columns.
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

// ------------------------------------------------------------ transform ----

/// Where a node is, relative to its parent: scaled, then turned, then moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: [f32; 3],
    /// A unit quaternion `[x, y, z, w]`; one that is not unit is normalized
    /// when used.
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

    /// Moved by `translation`, not turned or scaled.
    pub fn from_translation(translation: [f32; 3]) -> Self {
        Self { translation, ..Self::IDENTITY }
    }

    /// Turned by `angle` radians about `axis` (the right-hand rule).
    pub fn from_rotation_axis_angle(axis: [f32; 3], angle: f32) -> Self {
        Self { rotation: quat_axis_angle(axis, angle), ..Self::IDENTITY }
    }

    /// Scaled by `scale` on each axis.
    pub fn from_scale(scale: [f32; 3]) -> Self {
        Self { scale, ..Self::IDENTITY }
    }

    /// `child` placed in this frame: a point in `child` goes through `child`
    /// first, then through this. `parent.then(child)` is the matrix
    /// `parent.matrix() * child.matrix()`, so `from_translation(a)` then
    /// `from_rotation_axis_angle(..)` turns about the moved origin.
    ///
    /// A transform holds no shear. The result is exact when this one's scale
    /// is uniform (or `child` is not turned against it); otherwise it is the
    /// usual nearest transform, and a scene's world matrices are made with
    /// the exact [`mat_mul`] of the two instead.
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

    /// The matrix, column-major: translation * rotation * scale.
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

// ------------------------------------------------------------- material ----

/// What a material takes its picture from. Today a picture the host
/// uploaded, or none (white, so the tint and vertex colours show).
///
/// Planned, not built (docs/GLASS_CORE.md decision 4): `Video(stream)`, the
/// decoded frames of a clip; `Grid`, the cockpit's text drawn into a target;
/// `Target(scene, camera)`, another scene's render; `Pixels`, a buffer a
/// producer writes. Each is one more way to name a texture; nothing else in a
/// material changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureSource {
    /// No picture: white.
    #[default]
    None,
    /// A picture by id, as `Core::upload` keeps them (a layer's picture, a
    /// stage, a scene target).
    Picture(u32),
}

/// How a texture is read between its texels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sampling {
    /// Crisp texels: pixel art.
    #[default]
    Nearest,
    Linear,
}

/// How a surface's alpha is used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlphaMode {
    /// Fully covers what is behind it (the texture's alpha is ignored); drawn
    /// first, with depth.
    #[default]
    Opaque,
    /// The texture's colour was handed over straight (not premultiplied; the
    /// renderer premultiplies it when it takes the picture): drawn after the
    /// opaque ones, back to front, tested against depth but not writing it.
    Blend,
    /// The texture's colour is already multiplied by its alpha (a scene
    /// target, a premultiplied video, taken in with `upload_premultiplied`):
    /// drawn as `Blend` is.
    Premultiplied,
}

impl AlphaMode {
    /// Whether it draws in the blended pass.
    pub fn blended(self) -> bool {
        !matches!(self, AlphaMode::Opaque)
    }
}

/// What a surface looks like: `texture x vertex colour x tint`, unlit unless
/// it says [`lit`](Material::lit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub source: TextureSource,
    /// Straight RGBA, 0..1, multiplied into the texture and the vertex
    /// colours (for a [`projector`](Material::projector)'s surface, its
    /// colour where the projector does not reach).
    pub tint: [f32; 4],
    pub sampling: Sampling,
    pub alpha: AlphaMode,
    /// Draw the back of a triangle too (a flat card seen from behind).
    pub double_sided: bool,
    /// Shaded by the one fixed light every scene has ([`KEY_LIGHT`] plus
    /// [`AMBIENT_LIGHT`], a plain Lambert), so a shape reads as solid. Unlit
    /// (the default) is the colour as it is.
    pub lit: bool,
    /// For a lit surface, how much of it shines by itself, 0 to 1: at 0 the
    /// light shades it, at 1 it is its own colour whatever the light does
    /// (magic, fire). The tint is clamped to 0..1, so a surface cannot be
    /// brighter than its colour; this is how it stops being darker than it.
    pub emissive: f32,
    /// In a scene with a [`Look`]: keep this surface's colour as the nearest
    /// of the look's accents, undithered (the moon on the water, a lit
    /// window), instead of putting it on the ramp. Ignored without a look.
    pub accent: bool,
    /// Cast the picture onto the surface from a [`Projector`] instead of
    /// laying it on by the mesh's uvs (docs/WORLD-3D.md, "project back").
    /// Where the projector sees the surface, the surface is the picture
    /// there, untinted. Where it does not (behind it, outside its picture, a
    /// face turned away from it, or one it sees so much more edge-on than
    /// the camera does that its texels would smear, [`GRAZING`]), the
    /// surface is its [`patch`](Material::patch), else its tint, shaded by
    /// the way it faces as a plate lights a block ([`FACE_SHADES`]; not a
    /// surface that shines, `emissive` 1). The vertex colours and the alpha
    /// apply to both as they do to any surface. `None` (the default) is the
    /// mesh's uvs.
    pub projector: Option<Projector>,
    /// For a surface cast from a projector: what it is where the projector
    /// does not see it, a picture tiled over it. `None`: its tint.
    pub patch: Option<Patch>,
    /// What the surface is to a look that lights its scene itself
    /// ([`Look::lights`]), past its colour. Ignored otherwise.
    pub role: Role,
    /// The brand guide's ceiling (docs/BRAND.md, `brand::Class::
    /// material_ceiling`): in a look that lights its scene, light never
    /// lifts the surface past step `ceiling - 1` of an eight-step ramp
    /// (scaled to the look's). 0: no ceiling but the ramp's top.
    pub ceiling: u8,
}

/// What a surface is to a look that lights its scene ([`Look::lights`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    /// Lit, then put on its place's ramp by its brightness.
    #[default]
    Plain,
    /// A figure (a knight's sprite): keeps its own colours, darkened by the
    /// light where it stands but never put on a ramp, so it reads against a
    /// world of one hue. It casts no shadow (its card would cast a sliver).
    Figure,
    /// Water: its brightness runs in slow bands along the mesh's `u` (the
    /// way it flows, in world units) with the look's clock, and where the
    /// light glints off it, it is the look's first accent. It casts no
    /// shadow.
    Water,
    /// The sky: unlit and unfogged, seen through the ramp of the place the
    /// eye stands in (not of the far point it is drawn at), so the sky
    /// changes with the land. It casts no shadow.
    Sky,
    /// What glows by itself (lava, a bed of coals): no light needed, its
    /// tone put on the look's warm ramp, slowly breathing with the clock.
    /// It casts no shadow.
    Ember,
}

/// A patch of a picture tiled over a projected surface where its projector
/// does not see it (an item's patch of its plate for the surface's kind, in
/// its swatches picture, docs/WORLD-3D.md "Swatches"): the rectangle `rect`
/// (`[x, y, width, height]` in the picture's pixels) laid on each face from
/// the side it faces most (a top from above, a front from the front, a side
/// from the side), mirrored at each repeat (every copy flipped against its
/// neighbours), a pixel of it `texel` world units a side, its corner at
/// `origin`. It moves with the blocks it is on (`set::place`). A picture
/// that is not there yet is the tint meanwhile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
    pub picture: u32,
    pub rect: [u32; 4],
    pub origin: [f32; 3],
    pub texel: f32,
}

/// How a projected surface the projector does not see is shaded by the way
/// it faces, as a plate paints a block lit from above: a top as it is, a
/// front and a side (or a bottom) in shade. A room's extra blocks are
/// shaded the same (`angel-delve`'s `world3d::fit`).
pub const FACE_SHADES: [f32; 3] = [1.0, 0.86, 0.74];

/// How much more edge-on than the camera a projector may see a surface and
/// still cast its picture on it: where `cos(projector) < GRAZING *
/// cos(camera)`, each the cosine between the surface's normal and the way
/// to that eye, the plate's texels would be stretched over more than
/// `1 / GRAZING` times the surface they were painted for, and it is drawn
/// as one the projector does not see. A camera that is its projector never
/// stretches them so.
pub const GRAZING: f32 = 0.5;

/// A camera a picture is cast from: a painted plate thrown back onto the
/// blocks it was painted over, from the camera it was captured with.
///
/// `view_projection` is that camera's whole matrix, world to clip space,
/// column-major like every [`Mat4`] here (`Camera::view_projection`, with
/// the plate's aspect). A world point `p` is cast the texel at
/// `uv = ((x / w + 1) / 2, (1 - y / w) / 2)` of `(x, y, _, w) = M (p, 1)`:
/// clip `x` -1..1 left to right across the picture, clip `y` +1..-1 down it
/// (`v` runs down, as every texture's does). A point with `w <= 0` is behind
/// the projector, and one whose `uv` is outside 0..1 is outside its picture;
/// neither is cast anything. Nor is a face turned away from it (the GPU
/// tells by whether the projector sees the surface wound the way the camera
/// does), nor one it sees much more edge-on than the camera does
/// ([`GRAZING`]). What stands between the projector and a surface is not
/// looked at (a plate is cast from the camera it is drawn for, which sees
/// no further than the projector does). A matrix that is not finite casts
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projector {
    pub view_projection: Mat4,
}

impl Projector {
    /// The projector of `camera` for a picture `aspect` (width / height)
    /// wide, or `None` when the camera cannot see ([`Camera::view_projection`]).
    pub fn of(camera: &Camera, aspect: f32) -> Option<Self> {
        camera.view_projection(aspect).map(|view_projection| Self { view_projection })
    }

    /// Where world point `p` is cast from, as texel coordinates (0..1, `v`
    /// down); `None` when it is not in the picture (behind the projector, or
    /// outside it). Whether a face is turned away is the GPU's to tell: it
    /// takes the surface around the point.
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
        }
    }
}

/// Where a lit surface's light comes from, as the direction toward it (unit
/// length): over the left shoulder of a viewer, from above and in front. The
/// world is the scene's own (+Y up, +Z toward the viewer). Fixed: one light,
/// the same for every scene.
pub const KEY_LIGHT: [f32; 3] = [-0.47, 0.70, 0.54];

/// The least a lit surface is lit (a face turned away from the key light).
pub const AMBIENT_LIGHT: f32 = 0.42;

impl Material {
    /// A flat colour (straight RGBA), blended when it is not opaque.
    pub fn color(rgba: [f32; 4]) -> Self {
        Self {
            tint: rgba,
            alpha: if rgba[3] < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
            ..Self::default()
        }
    }

    /// A picture by id, opaque.
    pub fn picture(id: u32) -> Self {
        Self { source: TextureSource::Picture(id), ..Self::default() }
    }

    /// Picture `id` cast from `projector`, the surface `fallback` (straight
    /// RGBA, blended when it is not opaque) wherever the projector does not
    /// see it, shaded as a plate lights a block. Unlit: a painted plate
    /// carries its own light.
    pub fn projected(id: u32, projector: Projector, fallback: [f32; 4]) -> Self {
        Self { source: TextureSource::Picture(id), projector: Some(projector), ..Self::color(fallback) }
    }

    /// A flat colour (straight RGBA) shaded by the light, shining by itself
    /// as far as `emissive` (0..1) says.
    pub fn lit(rgba: [f32; 4], emissive: f32) -> Self {
        Self { lit: true, emissive, ..Self::color(rgba) }
    }

    /// The tint with each channel made a number in 0..1 (a bad one is 1).
    pub fn tint_clamped(&self) -> [f32; 4] {
        self.tint.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 })
    }

    /// The emissive part as a number in 0..1 (a bad one is 0).
    pub fn emissive_clamped(&self) -> f32 {
        if self.emissive.is_finite() { self.emissive.clamp(0.0, 1.0) } else { 0.0 }
    }
}

// ----------------------------------------------------------------- mesh ----

/// A mesh a node draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshRef {
    /// A 1 x 1 square in XY, centred, facing +Z.
    Quad,
    /// A 1 x 1 x 1 cube, centred; faces in the order of [`CUBE_FACES`], so
    /// `face_materials[i]` is face `i`.
    Cube,
    /// A 1 x 1 square in XZ, centred, facing +Y.
    Plane,
    /// A sphere of diameter 1, centred, with this many longitudes (clamped to
    /// [`SPHERE_SEGMENTS`]) and half as many rings.
    Sphere { segments: u16 },
    /// An octahedron, its six points a half unit from the middle along the
    /// axes, flat faces: a crystal, a shard, an arrowhead when stretched.
    Gem,
    /// A cone of base diameter 1 and height 1, centred, its point at +Y and
    /// its base facing -Y, with this many sides (clamped to
    /// [`SPHERE_SEGMENTS`]), flat faces.
    Cone { segments: u16 },
    /// A cylinder of diameter 1 and height 1, centred: flat caps, smooth
    /// sides, this many of them (clamped to `primitives::RING_SEGMENTS`).
    Cylinder { segments: u16 },
    /// A cylinder whose top diameter is 0.6 (a tower's top, a tree trunk, a
    /// chimney pot), this many sides.
    Frustum { segments: u16 },
    /// A ramp: the bottom is the unit square, the back (-Z) full height,
    /// sloping down to nothing at +Z (`primitives`).
    Wedge,
    /// A pitched roof prism: the ridge along X at the top centre, slopes
    /// down to -Z and +Z, gable ends at -X and +X.
    Gable,
    /// A hipped roof: a ridge along X from -0.25 to +0.25 at the top, four
    /// sloped faces down to the unit square.
    Hip,
    /// A square-based spire to a point at the top centre.
    Pyramid,
    /// A wall block with a round-topped opening through it along Z (0.6
    /// wide, straight sides to y = 0 and a half circle to y = +0.3), the
    /// opening's inner faces part of the mesh.
    Arch { segments: u16 },
    /// A staircase of this many steps (clamped to `STAIR_STEPS`), rising
    /// toward -Z.
    Stairs { steps: u16 },
    /// A lumpy boulder: a subdivided icosphere, each point pushed by a hash
    /// of this seed, flat shaded, sitting on y = -0.5.
    Rock { seed: u16 },
    /// A soft foliage clump: a few overlapping squashed spheres placed by
    /// this seed, merged, inside the unit cube.
    Blob { seed: u16 },
    /// A fence of this many posts (clamped to `FENCE_POSTS`) evenly along X
    /// with two rails, thin in Z.
    Fence { posts: u16 },
    /// A mesh the host uploaded (`Core::upload_mesh`) under this id.
    Uploaded(u32),
}

impl MeshRef {
    /// The mesh as data, for a built-in; `None` for an uploaded one.
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
            MeshRef::Uploaded(_) => None,
        }
    }
}

/// The faces of [`MeshData::cube`], in order: the outward normal, and the
/// directions a viewer outside the face sees as right and as up when the
/// picture on it is upright.
pub const CUBE_FACES: [CubeFace; 6] = [
    CubeFace { name: "+X", normal: [1.0, 0.0, 0.0], right: [0.0, 0.0, -1.0], up: [0.0, 1.0, 0.0] },
    CubeFace { name: "-X", normal: [-1.0, 0.0, 0.0], right: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0] },
    CubeFace { name: "+Y", normal: [0.0, 1.0, 0.0], right: [1.0, 0.0, 0.0], up: [0.0, 0.0, -1.0] },
    CubeFace { name: "-Y", normal: [0.0, -1.0, 0.0], right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0] },
    CubeFace { name: "+Z", normal: [0.0, 0.0, 1.0], right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0] },
    CubeFace { name: "-Z", normal: [0.0, 0.0, -1.0], right: [-1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0] },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeFace {
    pub name: &'static str,
    pub normal: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
}

/// Why a mesh was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshError {
    /// No vertices or no triangles.
    Empty,
    /// More than [`MAX_MESH_VERTICES`].
    TooManyVertices,
    /// More than [`MAX_MESH_INDICES`].
    TooManyIndices,
    /// An attribute list neither empty nor one entry per position (its name).
    AttributeLength(&'static str),
    /// The indices are not whole triangles.
    NotTriangles,
    /// An index names a vertex that is not there.
    IndexOutOfRange,
    /// A number is NaN or infinite.
    NotFinite,
    /// A group runs past the indices, or is not whole triangles.
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

/// Triangles with attributes. `normals`, `uvs` and `colors` are each either
/// empty (a default: +Z, (0, 0), white) or one entry per position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// `(0, 0)` is the picture's top left; `v` runs down.
    pub uvs: Vec<[f32; 2]>,
    /// Straight RGBA, multiplied into the texture.
    pub colors: Vec<[f32; 4]>,
    /// Triangles, counter-clockwise from the front.
    pub indices: Vec<u32>,
    /// Ranges of `indices`, `(first, count)`, each whole triangles. With
    /// none, the mesh is one piece drawn with the node's `material`. With
    /// some, only they are drawn, group `i` with the node's `face_materials[i]`
    /// (the node's `material` where it has none): the six faces of a cube,
    /// or the parts of a model.
    pub groups: Vec<(u32, u32)>,
}

impl MeshData {
    /// Whether the GPU can take this mesh as it is.
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
        ] {
            if len != 0 && len != n {
                return Err(MeshError::AttributeLength(name));
            }
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
            && self.colors.iter().flatten().all(|v| v.is_finite());
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

    /// The middle of the box the positions fill (zero for none).
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

    /// One flat face: the four corners around `centre`, `half_right` and
    /// `half_up` the vectors from the middle to the right and top edges, the
    /// picture upright for a viewer on the `normal` side.
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
        // Top left, top right, bottom right, bottom left.
        self.positions.extend([corner(-1.0, 1.0), corner(1.0, 1.0), corner(1.0, -1.0), corner(-1.0, -1.0)]);
        self.normals.extend([normal; 4]);
        self.uvs.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        self.colors.extend([[1.0; 4]; 4]);
        // Counter-clockwise from the front: top left, bottom left, bottom right; then top left, bottom right, top right.
        self.indices.extend([base, base + 3, base + 2, base, base + 2, base + 1]);
    }

    /// A 1 x 1 square in XY facing +Z.
    pub fn quad() -> Self {
        let mut mesh = Self::default();
        mesh.push_face([0.0; 3], [0.0, 0.0, 1.0], [0.5, 0.0, 0.0], [0.0, 0.5, 0.0]);
        mesh
    }

    /// A 1 x 1 square in XZ facing +Y, its picture upright for a viewer above
    /// with -Z away from them.
    pub fn plane() -> Self {
        let mut mesh = Self::default();
        mesh.push_face([0.0; 3], [0.0, 1.0, 0.0], [0.5, 0.0, 0.0], [0.0, 0.0, -0.5]);
        mesh
    }

    /// A 1 x 1 x 1 cube centred on the origin: 24 vertices, four to a face so
    /// each face has the whole picture, in the order of [`CUBE_FACES`] (+X,
    /// -X, +Y, -Y, +Z, -Z). Six groups, face `i` being indices `6 * i`, 6.
    pub fn cube() -> Self {
        let mut mesh = Self::default();
        for face in &CUBE_FACES {
            let scaled = |v: [f32; 3]| [v[0] * 0.5, v[1] * 0.5, v[2] * 0.5];
            mesh.push_face(scaled(face.normal), face.normal, scaled(face.right), scaled(face.up));
        }
        mesh.groups = (0..6).map(|i| (i * 6, 6)).collect();
        mesh
    }

    /// A sphere of diameter 1 centred on the origin. `segments` longitudes
    /// (clamped to [`SPHERE_SEGMENTS`]) and half as many rings; the picture
    /// wraps once around it, upright from the front (+Z), `u` growing toward +X.
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
                // Longitude 0 is the front (+Z); it grows toward +X.
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
                // The first ring and the last meet in a point: no sliver there.
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
}

impl MeshData {
    /// An octahedron: six points a half unit out along the axes, eight flat
    /// triangular faces of three vertices each (24 vertices), every face's
    /// normal pointing out. The picture on a face is its corners.
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
            // x, y, z is counter-clockwise from outside when the signs agree
            // in parity, and clockwise when they do not.
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

    /// A cone: its point at +Y (height 0.5), its base a ring of radius 0.5 at
    /// -Y (a fan facing down), `segments` flat sides (clamped to
    /// [`SPHERE_SEGMENTS`]).
    pub fn cone(segments: u16) -> Self {
        let n = u32::from(segments.clamp(*SPHERE_SEGMENTS.start(), *SPHERE_SEGMENTS.end()));
        let mut mesh = Self::default();
        let ring = |i: u32| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            a.sin_cos()
        };
        // The sides: the point, and two neighbours on the ring, the normal
        // that of the slant (height 1 over radius 0.5).
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
            // Counter-clockwise from outside: the ring runs from +Z toward +X.
            mesh.indices.extend([base, base + 1, base + 2]);
        }
        // The base: a fan about its middle, facing down.
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

// ----------------------------------------------------------------- node ----

/// One thing in a scene: a place, and perhaps a mesh with a look.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Unique in its scene (a later node with an id already taken is not
    /// drawn). Parents are named by it.
    pub id: u64,
    /// The node this one is placed in; `None` places it in the scene.
    pub parent: Option<u64>,
    pub transform: Transform,
    /// What to draw; a node with none is a place for others to hang from.
    pub mesh: Option<MeshRef>,
    pub material: Material,
    /// Per group of the mesh (per face of a cube): group `i` is drawn with
    /// `face_materials[i]`, or with `material` past the end of this list.
    /// A mesh with no groups ignores it.
    pub face_materials: Vec<Material>,
    /// Hides the node and everything placed in it.
    pub visible: bool,
}

impl Node {
    /// A node at the scene's origin drawing `mesh` with `material`.
    pub fn new(id: u64, mesh: MeshRef, material: Material) -> Self {
        Self {
            id,
            parent: None,
            transform: Transform::IDENTITY,
            mesh: Some(mesh),
            material,
            face_materials: Vec::new(),
            visible: true,
        }
    }

    /// A node that draws nothing: a place for others to hang from.
    pub fn empty(id: u64) -> Self {
        Self {
            id,
            parent: None,
            transform: Transform::IDENTITY,
            mesh: None,
            material: Material::default(),
            face_materials: Vec::new(),
            visible: true,
        }
    }
}

/// How a camera sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// A pyramid of sight: `fov_y` is the whole vertical angle, in radians.
    Perspective { fov_y: f32, near: f32, far: f32 },
    /// A box of sight, `height` units tall; the width follows the target's
    /// aspect.
    Orthographic { height: f32, near: f32, far: f32 },
    /// A matrix as it is, view space to clip space (WebGPU's: z in 0..1),
    /// whatever the target's aspect: a camera that is neither of the above
    /// (the stage's, [`Tilt::camera`]).
    Custom(Mat4),
}

impl Projection {
    /// The projection matrix for a target `aspect` (width / height) wide, or
    /// `None` when these numbers cannot make one (not finite, a non-positive
    /// size or `near`, `far` not past `near`, a field of view outside 0..pi).
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

/// The eye. Its `transform` places it in the scene (its scale is ignored).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub transform: Transform,
    pub projection: Projection,
    /// The colour the target is cleared to first, straight RGBA; `None`
    /// draws over what the target holds.
    pub clear: Option<[f32; 4]>,
}

impl Default for Camera {
    /// At `z = 3` looking down -Z, a 45 degree view, clearing to nothing.
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
    /// The view matrix: the inverse of the camera's turn and move.
    pub fn view_matrix(&self) -> Mat4 {
        let [c0, c1, c2] = quat_columns(self.transform.rotation);
        let t = self.transform.translation;
        // The transpose of the turn, then the move undone through it.
        let dot = |c: [f32; 3]| -(c[0] * t[0] + c[1] * t[1] + c[2] * t[2]);
        [
            [c0[0], c1[0], c2[0], 0.0],
            [c0[1], c1[1], c2[1], 0.0],
            [c0[2], c1[2], c2[2], 0.0],
            [dot(c0), dot(c1), dot(c2), 1.0],
        ]
    }

    /// Projection * view for a target `aspect` wide, or `None` when the
    /// camera cannot see (see [`Projection::matrix`]) or sits at a number
    /// that is not finite.
    pub fn view_projection(&self, aspect: f32) -> Option<Mat4> {
        let view = self.view_matrix();
        if !mat_finite(&view) {
            return None;
        }
        let m = mat_mul(&self.projection.matrix(aspect)?, &view);
        mat_finite(&m).then_some(m)
    }

    /// The clear colour as the premultiplied colour a target is cleared to.
    pub fn clear_premultiplied(&self) -> Option<[f32; 4]> {
        self.clear.map(|c| {
            let c = c.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 });
            [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
        })
    }
}

// ------------------------------------------------------------ the stage ----

/// How far apart in depth the stage camera tells two floor points that are a
/// pixel apart: the depth buffer of a stage's props is spread over the whole
/// floor at this rate (a stage is never anywhere near 8,192 pixels deep).
const STAGE_DEPTH_PER_PX: f32 = 1.0 / 8192.0;

impl Tilt {
    /// The floor of a stage that is not tilted: the identity, each floor
    /// point where it is, what is lifted drawn straight up the screen.
    pub fn flat(size: (u32, u32)) -> Self {
        Self { centre: [size.0 as f32 / 2.0, size.1 as f32 / 2.0], near: 0.0, zoom: 1.0, squash: 1.0 }
    }

    /// The camera that sees a stage's props the way its tilted floor shows
    /// its layers (docs/GLASS_CORE.md, "3D shots"). The world is the
    /// stage's own: units are its pixels, `x` runs right across it, `z` runs
    /// down the floor toward the viewer (the floor point `(x, y)` is the
    /// world's `(x, 0, y)`), and `y` is the height up off the floor. A prop
    /// at floor point `p` and height `h` then lands exactly where an upright
    /// layer standing at `p` and lifted `h` pixels does: at `floor(p)` plus
    /// `h * scale_at(p)` straight up. (The camera looks along -`z` with the
    /// floor's vanishing point above the stage; a pinhole level with the
    /// horizon, its picture shifted, so that what stands up stays upright.)
    ///
    /// `size` is the stage's size in its own pixels, which the clip space is
    /// measured in; the camera draws onto a target of any size of the same
    /// shape. The depth is the floor's: a nearer point is nearer, everywhere,
    /// and what is level with another is level.
    pub fn camera(&self, size: (u32, u32)) -> Camera {
        let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let [cx, cy] = self.centre;
        let (zoom, squash, near) = (self.zoom, self.squash, self.near);
        type Row = [f32; 4];
        let add = |a: Row, k: f32, b: Row| [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2], a[3] + k * b[3]];
        let scale = |a: Row, k: f32| a.map(|v| v * k);
        // Rows over (x, y, z, 1). Depth: w = 1 - near (z - cy).
        let depth: Row = [0.0, 0.0, -near, 1.0 + near * cy];
        // The screen point (x', y') times w: the floor's map, and the lift.
        let screen_x = add([zoom, 0.0, 0.0, -zoom * cx], cx, depth);
        let screen_y = add([0.0, -zoom, zoom * squash, -zoom * squash * cy], cy, depth);
        // Clip space: x from -1 at the left, y from +1 at the top.
        let clip_x = add(scale(screen_x, 2.0 / w), -1.0, depth);
        let clip_y = add(depth, -2.0 / h, screen_y);
        // z grows with distance: 0.5 at the pivot row, a hair more for each
        // pixel further up the floor (divided by w, as clip space does).
        let clip_z = add([0.0, 0.0, -STAGE_DEPTH_PER_PX, STAGE_DEPTH_PER_PX * cy], 0.5, depth);
        let rows = [clip_x, clip_y, clip_z, depth];
        let matrix: Mat4 = std::array::from_fn(|column| std::array::from_fn(|row| rows[row][column]));
        Camera { transform: Transform::IDENTITY, projection: Projection::Custom(matrix), clear: Some([0.0; 4]) }
    }
}

/// The camera for a stage of `size` pixels whose floor tips by `tilt`
/// (`Frame::tilt`; none when flat): [`Tilt::camera`] over the floor
/// [`Tilt::for_stage`] makes, or the flat one.
pub fn stage_camera(tilt: f32, size: (u32, u32)) -> Camera {
    Tilt::for_stage(tilt, size).unwrap_or_else(|| Tilt::flat(size)).camera(size)
}

/// The most a stage's props may be drawn finer than the stage's own pixels.
pub const MAX_GRAIN: u8 = 8;

/// The 3D things a stage holds (docs/GLASS_CORE.md, "3D shots"): nodes in the
/// stage's own world (see [`Tilt::camera`]), seen by the camera its tilt
/// gives, drawn with depth among themselves.
///
/// They are not drawn into the stage's pixels but into a picture of their
/// own, `grain` times as fine as the stage's, which is then laid over the
/// stage's layers like any picture is, nearest: so the props have the stage's
/// pixel grain, a shot's edge a pixel of the art or a fraction of one, and
/// not the window's. (A stage is itself drawn at the density of the window,
/// so drawing into it directly would give them none.)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props {
    /// How many of the stage's layers lie under the props: they are laid
    /// over layers `0..at` and under the rest (a shot's shadow and glow under
    /// it, the screen's own marks over it).
    pub at: usize,
    /// The picture the props are drawn into is this many times the stage's
    /// size on a side (1 to [`MAX_GRAIN`]).
    pub grain: u8,
    pub nodes: Vec<Node>,
    /// The room itself in 3D, with its figures standing in it; none: the
    /// stage's layers draw the room, and the props are drawn on their own.
    pub world: Option<World>,
}

/// A room drawn in 3D among a stage's layers (docs/WORLD-3D.md, "Project
/// back"): its blocks, each with its painted plate cast on it, and the
/// figures standing among them as upright cards, seen by the camera the
/// stage's tilt gives (the props' camera) and drawn with depth, so a wall
/// hides the knight behind it and not the one in front.
///
/// It is drawn into pictures of the stage's own texture size (the window's
/// density, so a figure's pixels land one to one, nearest, as an upright
/// layer's do), each laid over the stage's layers as the props' picture is:
///
/// - the floor, then the blocks over it, over layers `0..at`;
/// - the figures, tested against the blocks' depth, over layers
///   `0..figures_at`;
/// - and the props' own nodes, tested against the blocks too (at the
///   texture's size, not at their grain), where the props lie.
///
/// The floor is drawn first and keeps no depth: nothing that stands on it
/// sinks into it (the part of a figure's picture below its foot, its
/// shadow, lies on the floor as it is painted). What stands is drawn with
/// depth among itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct World {
    /// What lies on the floor: the ground, paths, water level with it.
    pub floor: Vec<Node>,
    /// What stands on it: walls, roofs, trees, fences.
    pub blocks: Vec<Node>,
    /// Blocks its set does not have, drawn with the blocks: where a room
    /// differs from the item its set was made for (a doorway walled up, a
    /// boulder rolled in), each textured as it is. They cross a wire as
    /// nodes, the set's blocks by its reference.
    pub extra: Vec<Node>,
    /// How many of the stage's layers lie under the floor and the blocks.
    pub at: usize,
    /// The figures, each an upright card (or its own shapes).
    pub figures: Vec<Node>,
    /// How many of the stage's layers lie under the figures.
    pub figures_at: usize,
    /// The set the floor and the blocks were made from, when they were: what
    /// a wire carries in their place (the set's text once, then its key),
    /// for the far side to make them again (`set::Sets::fill`). A world
    /// with none goes over a wire without its floor and blocks.
    pub set: Option<SetRef>,
    /// The further sets its floor and blocks were made from, after `set`'s
    /// (the kingdom's chunks: each its own set, plate and projector), each
    /// crossing a wire as `set` does.
    pub more: Vec<SetRef>,
    /// The look it is drawn through (docs/REALM-MONOLITH.md): one that
    /// lights its scene draws the floor and blocks lit, toned and on the
    /// place's ramps, at the stage's own pixel grain, instead of cast with
    /// plates. Its sets are then made lit ([`crate::set::Set::lit_world_nodes`]).
    pub look: Option<Look>,
    /// Meshes the floor and blocks name (`MeshRef::Uploaded`): its sets'
    /// grounds, uploaded by whoever draws it before it is drawn. Made again
    /// from the sets on the far side of a wire, never sent.
    pub meshes: Vec<(u32, std::sync::Arc<MeshData>)>,
}

/// A set's blocks by reference (docs/WORLD-3D.md): which set, the picture
/// its plate is, and how it is placed on the stage (`Set::placed_nodes`).
#[derive(Clone, Debug, PartialEq)]
pub struct SetRef {
    /// The set's own number: the same text and plate, the same key.
    pub key: u64,
    /// The set file's text, when the far side may not have it yet (the
    /// first frame of a room, or when everything is sent again).
    pub text: Option<std::sync::Arc<str>>,
    /// The picture its plate is.
    pub plate: u32,
    /// The plate as a PNG, when the far side has not been sent it (the first
    /// room of an item, or when everything is sent again): it decodes it and
    /// keeps it as picture `plate`. A plate crosses a wire so, never as
    /// pixels; one drawn where it is made goes as a picture, as ever.
    pub png: Option<std::sync::Arc<[u8]>>,
    /// The set's world scaled by this about its origin, then moved by
    /// `offset`, onto the stage.
    pub scale: f32,
    pub offset: [f32; 3],
    /// The picture its kinds' patches of the plate are in, and where, for
    /// the surfaces the plate's projector does not see (`set::Patches`;
    /// none: their kinds' colours).
    pub patches: crate::set::Patches,
}

impl Props {
    /// The grain as it is drawn: 1 to [`MAX_GRAIN`].
    pub fn grain_clamped(&self) -> u32 {
        u32::from(self.grain.clamp(1, MAX_GRAIN))
    }
}

/// A node that is to be drawn: where it ended up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawable {
    /// Its index in [`Scene::nodes`].
    pub node: usize,
    pub world: Mat4,
}

/// A camera and the nodes it sees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub camera: Camera,
    pub nodes: Vec<Node>,
    /// The palette and pixel size the scene is seen through (`look.rs`), or
    /// none: drawn as it is, at the size of its target.
    pub look: Option<Look>,
}

impl Scene {
    pub fn new(camera: Camera) -> Self {
        Self { camera, nodes: Vec::new(), look: None }
    }

    /// Each node's world matrix (its parent chain composed, parent first), or
    /// `None` for a node that cannot be placed: a missing parent, a chain that
    /// loops, an id already taken by an earlier node, a number that is not
    /// finite, or a place past [`MAX_NODES`]. Its length is the node count.
    pub fn world_matrices(&self) -> Vec<Option<Mat4>> {
        self.placed().into_iter().map(|p| p.map(|(m, _)| m)).collect()
    }

    /// Every node's world matrix and whether it and all its parents are shown.
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
        // 0: not looked at, 1: on the chain being walked, 2: settled.
        let mut state = vec![0u8; n];
        let mut chain: Vec<usize> = Vec::new();
        for start in 0..n {
            if state[start] == 2 {
                continue;
            }
            chain.clear();
            let mut cur = start;
            // What the top of the chain hangs from: the scene, or a settled
            // node; `None` when the chain cannot be placed at all.
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
            // Settle the chain from the top down.
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

    /// The nodes to draw, in node order: those with a mesh that are shown
    /// (themselves and every parent) and can be placed.
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
        // Hand computed: (0, 0, sin 45, cos 45).
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(near(q[2], h) && near(q[3], h) && near(q[0], 0.0) && near(q[1], 0.0));
        assert!(near3(quat_rotate(q, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        assert!(near3(quat_rotate(q, [0.0, 1.0, 0.0]), [-1.0, 0.0, 0.0]));
        assert!(near3(quat_rotate(q, [0.0, 0.0, 1.0]), [0.0, 0.0, 1.0]));
        // The axis need not be unit.
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
        // Turn about x first, then about y: z -> -y (x) -> -y (y).
        let both = quat_mul(about_y, about_x);
        assert!(near3(quat_rotate(both, [0.0, 0.0, 1.0]), [0.0, -1.0, 0.0]));
        // The other way: y first (z -> x), then x (x stays).
        let other = quat_mul(about_x, about_y);
        assert!(near3(quat_rotate(other, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]));
        // The identity changes nothing, and a half turn twice is nothing.
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
        // Rotating by 90 degrees about z: x -> y, y -> -x.
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
        // (1, 0, 0) -> scaled (2, 0, 0) -> turned (0, 2, 0) -> moved (10, 2, 0).
        assert!(near3(mat_point(&t.matrix(), [1.0, 0.0, 0.0]), [10.0, 2.0, 0.0]));
        // (0, 1, 0) -> (0, 3, 0) -> (-3, 0, 0) -> (7, 0, 0).
        assert!(near3(mat_point(&t.matrix(), [0.0, 1.0, 0.0]), [7.0, 0.0, 0.0]));
        assert!(near3(mat_point(&t.matrix(), [0.0, 0.0, 1.0]), [10.0, 0.0, 4.0]));
    }

    #[test]
    fn a_matrix_product_applies_the_right_one_first() {
        let move_x = Transform::from_translation([5.0, 0.0, 0.0]).matrix();
        let turn = Transform::from_rotation_axis_angle([0.0, 0.0, 1.0], FRAC_PI_2).matrix();
        // Turn first, then move: (1, 0, 0) -> (0, 1, 0) -> (5, 1, 0).
        assert!(near3(mat_point(&mat_mul(&move_x, &turn), [1.0, 0.0, 0.0]), [5.0, 1.0, 0.0]));
        // Move first, then turn: (1, 0, 0) -> (6, 0, 0) -> (0, 6, 0).
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
        // The child's origin: (1, 0, 0) scaled 2, turned to (0, 2, 0), moved to (10, 2, 0).
        assert!(near3(both.translation, [10.0, 2.0, 0.0]));
        assert!(near3(both.scale, [1.0, 1.0, 1.0]));
        // It agrees with the matrix product, a point at a time.
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
        // The camera's own place maps to the view's origin.
        assert!(near3(mat_point(&cam.view_matrix(), [1.0, 2.0, 3.0]), [0.0; 3]));
        // View and world cancel.
        let both = mat_mul(&cam.view_matrix(), &cam.transform.matrix());
        for c in 0..4 {
            for r in 0..4 {
                assert!(near(both[c][r], IDENTITY_MATRIX[c][r]), "[{c}][{r}]");
            }
        }
        // Turned a quarter about y, the camera looks down world -X (its -Z
        // turned): a point one unit that way is one unit in front.
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
        // A right angle view: at distance d the screen's top is y = d, and the right is 2d (aspect 2).
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
        // 4 high and 8 wide: the right edge is x = 4, the top y = 2.
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
        // Listed child first: order in the list does not matter.
        let s = scene(vec![node(3, Some(2), [1.0, 0.0, 0.0]), node(2, Some(1), [0.0, 1.0, 0.0]), turned]);
        let world = s.world_matrices();
        // Node 2 sits at (0, 1, 0) in node 1, whose turn takes it to (-1, 0, 0), then (10, 0, 0): (9, 0, 0).
        assert!(near3(mat_point(&world[1].unwrap(), [0.0; 3]), [9.0, 0.0, 0.0]));
        // Node 3 is (1, 0, 0) further in node 2, which is turned like node 1's frame:
        // (1, 0, 0) -> (0, 1, 0) in the scene's axes -> (9, 1, 0).
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
        // 1 -> 2 -> 3 -> 1, a node its own parent, and a child of the loop.
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
        // A node with no mesh is a place, not a drawing.
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
        // The deepest first, the worst order.
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

    // ---- meshes ----

    fn length(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    /// Every triangle's counter-clockwise normal points the way its vertices'
    /// normals do (so the front is the outside).
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
        // Top left of the picture is top left of the quad.
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
        // The picture's top is away (-Z).
        assert_eq!(m.positions[0], [-0.5, 0.0, -0.5]);
    }

    #[test]
    fn the_cube_has_four_vertices_to_a_face_in_a_fixed_order() {
        let m = MeshData::cube();
        well_made(&m);
        assert_eq!((m.positions.len(), m.indices.len()), (24, 36));
        assert_eq!(m.groups, [(0, 6), (6, 6), (12, 6), (18, 6), (24, 6), (30, 6)]);
        for (i, face) in CUBE_FACES.iter().enumerate() {
            // Face i's vertices are 4i..4i+4, all on its side, with its normal.
            for v in 4 * i..4 * i + 4 {
                assert_eq!(m.normals[v], face.normal, "{} normal", face.name);
                let p = m.positions[v];
                let along = p[0] * face.normal[0] + p[1] * face.normal[1] + p[2] * face.normal[2];
                assert!((along - 0.5).abs() < 1e-6, "{} lies on its side", face.name);
            }
            // Its group draws its own vertices and no others.
            let (first, count) = m.groups[i];
            for &index in &m.indices[first as usize..(first + count) as usize] {
                assert!((4 * i as u32..4 * i as u32 + 4).contains(&index), "{} group", face.name);
            }
            // Each face carries the whole picture.
            let uvs = &m.uvs[4 * i..4 * i + 4];
            assert_eq!(uvs, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            // And the picture is upright: its top left is up-left from outside.
            let (tl, tr, bl) = (m.positions[4 * i], m.positions[4 * i + 1], m.positions[4 * i + 3]);
            let dir = |a: [f32; 3], b: [f32; 3]| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            assert_eq!(dir(tl, tr), face.right, "{} right", face.name);
            assert_eq!(dir(bl, tl), face.up, "{} up", face.name);
        }
        // The order is +X, -X, +Y, -Y, +Z, -Z.
        let names: Vec<&str> = CUBE_FACES.iter().map(|f| f.name).collect();
        assert_eq!(names, ["+X", "-X", "+Y", "-Y", "+Z", "-Z"]);
        // Right, up and the outward normal are a right-handed set on every face.
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
        // No sliver triangles at the poles: 16 * 8 quads, two triangles each, less one per pole ring cell.
        assert_eq!(m.indices.len(), 3 * (16 * 8 * 2 - 2 * 16));
        // The front is +Z, and the picture's middle is there.
        let front = m
            .positions
            .iter()
            .zip(&m.uvs)
            .find(|(_, uv)| (uv[0] - 0.0).abs() < 1e-6 && (uv[1] - 0.5).abs() < 1e-6)
            .unwrap();
        assert!(near3(*front.0, [0.0, 0.0, 0.5]));
        // Too few and too many are clamped, and still valid.
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

    // ---- materials ----

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

    // ---- lit materials, gems and cones ----

    #[test]
    fn a_material_is_unlit_unless_it_says_so() {
        let m = Material::default();
        assert!(!m.lit && m.emissive == 0.0);
        let lit = Material::lit([1.0, 0.5, 0.0, 1.0], 0.25);
        assert!(lit.lit && lit.emissive == 0.25 && lit.alpha == AlphaMode::Opaque);
        assert_eq!(lit.tint, [1.0, 0.5, 0.0, 1.0]);
        // A bad or wild emissive is made usable.
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
        // Six points, a half unit out along the axes.
        let mut points: Vec<[i32; 3]> = m
            .positions
            .iter()
            .map(|p| p.map(|v| (v * 2.0).round() as i32))
            .collect();
        points.sort_unstable();
        points.dedup();
        assert_eq!(points.len(), 6);
        assert!(points.iter().all(|p| p.iter().map(|v| v.abs()).sum::<i32>() == 1));
        // Flat: all three corners of a face share its normal.
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
        // Six sides of three vertices, a middle and a ring (closed) for the base.
        assert_eq!(m.positions.len(), 6 * 3 + 1 + 7);
        assert_eq!(m.indices.len(), 3 * (6 + 6));
        let top = m.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        let bottom = m.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert_eq!((top, bottom), (0.5, -0.5));
        assert!(m.positions.iter().all(|p| p[0].hypot(p[2]) <= 0.5 + 1e-6));
        // Too few and too many sides are clamped, and still valid.
        for n in [0, u16::MAX] {
            let c = MeshData::cone(n);
            assert_eq!(c.validate(), Ok(()));
            assert!(faces_outward(&c));
        }
        assert_eq!(MeshData::cone(0).positions.len(), 3 * 3 + 1 + 4);
        assert_eq!(MeshRef::Gem.generate(), Some(MeshData::gem()));
        assert_eq!(MeshRef::Cone { segments: 6 }.generate(), Some(MeshData::cone(6)));
    }

    // ---- the stage's camera ----

    /// Where the camera puts world point `p` of a stage of `size`: its pixel
    /// in the stage, and its depth in 0..1.
    fn seen(camera: &Camera, size: (u32, u32), p: [f32; 3]) -> ([f32; 2], f32) {
        let m = camera.view_projection(size.0 as f32 / size.1 as f32).expect("a camera that sees");
        let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
        let (x, y, z, w) = (row(0), row(1), row(2), row(3));
        ([(x / w + 1.0) * 0.5 * size.0 as f32, (1.0 - y / w) * 0.5 * size.1 as f32], z / w)
    }

    /// Where the layers' maths put an upright thing standing at floor point
    /// `foot` and lifted `height` pixels: the floor's map, and the lift
    /// scaled by how big what stands there is drawn (glass.wgsl, `stand` 2).
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
        // The floors a game sends, over the sizes it draws at.
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
        // Not tilted is also what a NaN, a negative and a zero-sized stage give: still a camera.
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
        // And a camera that cannot project still cannot: the matrix must be finite.
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
            // Down the floor, toward the viewer: strictly nearer, a pixel at a time.
            for y in (-40..=260).step_by(1) {
                let z = depth(192.0, 4.0, y as f32);
                assert!((0.0..1.0).contains(&z), "tilt {tilt} y {y}: {z}");
                assert!(z < last, "tilt {tilt} y {y}: {z} not nearer than {last}");
                last = z;
            }
            // Level with each other across and up: the same depth.
            let z = depth(10.0, 0.0, 100.0);
            for (x, h) in [(300.0, 0.0), (10.0, 40.0), (200.0, 12.0)] {
                assert!((depth(x, h, 100.0) - z).abs() < 1e-6, "tilt {tilt}");
            }
            // A pixel apart is far more than the depth buffer can tell apart.
            assert!(depth(0.0, 0.0, 100.0) - depth(0.0, 0.0, 101.0) > 1e-5);
        }
    }

    #[test]
    fn a_projector_casts_its_picture_by_its_clip_space() {
        // The stage's camera as a projector: a floor point lands on the texel
        // its pixel is, the picture the stage's size.
        let size = (384, 224);
        let camera = stage_camera(0.3, size);
        let projector = Projector::of(&camera, 384.0 / 224.0).unwrap();
        for p in [[100.0, 0.0, 50.0], [300.0, 40.0, 200.0], [192.0, 12.0, 112.0]] {
            let (pixel, _) = seen(&camera, size, p);
            let uv = projector.uv(p).unwrap();
            assert!((uv[0] * 384.0 - pixel[0]).abs() < 1e-2 && (uv[1] * 224.0 - pixel[1]).abs() < 1e-2, "{p:?}");
        }
        // Outside the picture, and behind a pinhole: nothing.
        assert_eq!(projector.uv([-50.0, 0.0, 100.0]), None);
        let pinhole = Projector::of(&Camera::default(), 1.0).unwrap();
        assert!(pinhole.uv([0.0, 0.0, 0.0]).is_some());
        assert_eq!(pinhole.uv([0.0, 0.0, 5.0]), None);
        assert_eq!(Projector { view_projection: [[f32::NAN; 4]; 4] }.uv([0.0; 3]), None);
        // A projected material is its picture's, its fallback the tint.
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
        // A scene of them under the stage's camera draws the way any does.
        let scene = Scene {
            camera: stage_camera(0.3, (384, 224)),
            nodes: vec![Node::new(1, MeshRef::Gem, Material::default())],
            look: None,
        };
        assert_eq!(scene.draw_list().len(), 1);
        assert!(scene.camera.view_projection(384.0 / 224.0).is_some());
    }
}
