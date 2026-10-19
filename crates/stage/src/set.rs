//! A set (docs/WORLD-3D.md, "Set files"): the plain blocks one item of the
//! world is built of (a room, a stretch of the kingdom), read from text, and
//! the scenes it makes. A capture the art lane paints over, flat colours by
//! surface kind under the scene's light; the kinds' exact id colours,
//! unlit; and the painted plate cast back onto the blocks from the camera it
//! was captured with.
//!
//! The world is the stage's own (see [`Tilt::camera`](crate::Tilt::camera)):
//! units are stage pixels, `x` right, `y` up, `z` down the floor toward the
//! viewer.
//!
//! A set is text, one statement a line, `#` to the end of a line a comment:
//!
//! ```text
//! set yard-sample
//! camera room 768 448
//! plate 1536 1024
//! box  stone   at 200 0 180 size 96 64 48
//! cone foliage at 420 0 300 size 40 70 40 turn 15 id tree-3
//! ```
//!
//! Any line that cannot be read is refused with its line number, and a set
//! with a refused line is not a set: [`Set::parse`] answers every refusal.

use std::fmt;

use crate::scene::{
    Camera, Material, Mat4, MeshData, MeshRef, Node, Patch, SetRef, Projection, Projector, Scene, Transform, World,
    mat_mul, quat_axis_angle, quat_rotate, stage_camera,
};
use crate::brand;
use crate::shapes;

/// How far the Delve's floor tips (`stage_frame::TILT` in crates/delve): a
/// room camera is the stage camera at this tilt.
pub const GAME_TILT: f32 = 0.3;

/// The capture's size when a set names none: the art lane's native size.
pub const DEFAULT_PLATE: (u32, u32) = (1536, 1024);

/// What the plate leaves around the stage on each side, a part of its width
/// and of its height: the stage is scaled to fit inside the rest, centred.
pub const PLATE_MARGIN: f32 = 0.04;

/// The largest stage or plate side, in pixels (the GPU's largest target).
pub const MAX_SIDE: u32 = 8192;

/// The most shapes a set may hold.
pub const MAX_SHAPES: usize = 4096;

/// The longest set or shape id.
pub const MAX_ID: usize = 64;

/// The colour a capture and an id map are cleared to: black, which no kind
/// is in either.
pub const BACKGROUND: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// A side of a cell of an item's swatches (`swatches.png`, docs/WORLD-3D.md
/// "Swatches"): each kind's patch of the plate lies at the top left of its
/// cell ([`swatch_cell`]), opaque, the rest of the cell clear.
pub const SWATCH_CELL: u32 = 64;
/// The swatches' cells a row: kind `i` (in the table's order) is cell
/// `(i % 9, i / 9)`, so the 27 kinds are three rows.
pub const SWATCH_COLS: u32 = 9;

/// The swatches picture's size: every kind's cell.
pub const SWATCH_ATLAS: (u32, u32) = (SWATCH_CELL * SWATCH_COLS, SWATCH_CELL * 27u32.div_ceil(SWATCH_COLS));

/// Where kind `kind`'s cell lies in the swatches picture: its top left, in pixels.
pub fn swatch_cell(kind: Kind) -> (u32, u32) {
    let i = kind.index() as u32;
    ((i % SWATCH_COLS) * SWATCH_CELL, (i / SWATCH_COLS) * SWATCH_CELL)
}

/// An item's patches of its plate (`swatches.png`) as one picture, for its
/// blocks' surfaces the plate's projector does not see (`scene::Patch`):
/// picture `atlas` is the swatches picture, kind `k`'s patch the rectangle
/// `sizes[k]` (width, height, at most [`SWATCH_CELL`]) at the top left of
/// its cell ([`swatch_cell`]), none where it is 0 across. A kind with no
/// patch of its own takes the patch of the kind nearest it in capture
/// colour, as an item's extra blocks do; with none at all such a surface is
/// its kind's colour two shades down ([`Kind::dark`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patches {
    pub atlas: u32,
    pub sizes: [[u8; 2]; 27],
}

impl Default for Patches {
    fn default() -> Self {
        Patches::NONE
    }
}

impl Patches {
    /// No patches.
    pub const NONE: Patches = Patches { atlas: 0, sizes: [[0; 2]; 27] };

    /// The patches of picture `atlas`: each kind's width and height.
    pub fn of(atlas: u32, patches: impl IntoIterator<Item = (Kind, u32, u32)>) -> Patches {
        let mut sizes = [[0; 2]; 27];
        for (kind, w, h) in patches {
            if w > 0 && h > 0 {
                sizes[kind.index()] = [w.min(SWATCH_CELL) as u8, h.min(SWATCH_CELL) as u8];
            }
        }
        Patches { atlas, sizes }
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.sizes.iter().all(|s| s[0] == 0 || s[1] == 0)
    }

    /// The kind whose patch kind `kind` is drawn with: its own, else the one
    /// nearest it in capture colour (the first such in the table); `None`
    /// when there are none.
    pub fn drawn_as(&self, kind: Kind) -> Option<Kind> {
        let has = |k: &Kind| self.sizes[k.index()].iter().all(|&v| v > 0);
        if has(&kind) {
            return Some(kind);
        }
        let want = kind.capture();
        let far = |k: &Kind| {
            let have = k.capture();
            (0..3).map(|c| (i32::from(have[c]) - i32::from(want[c])).pow(2)).sum::<i32>()
        };
        Kind::ALL.into_iter().filter(has).min_by_key(far)
    }

    /// Where in the atlas kind `kind` is drawn from where its projector does
    /// not see it, as `[x, y, width, height]` in pixels, if anywhere.
    pub fn rect(&self, kind: Kind) -> Option<[u32; 4]> {
        let k = self.drawn_as(kind)?;
        let (x, y) = swatch_cell(k);
        let [w, h] = self.sizes[k.index()];
        Some([x, y, u32::from(w), u32::from(h)])
    }
}

// ---------------------------------------------------------------- kinds ----

/// What a surface is: it tells the painter, and gives the surface a flat
/// colour in the capture and an exact colour in the id map. The list is the
/// table in docs/WORLD-3D.md; a new kind is a change to both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Ground,
    Grass,
    Path,
    Road,
    Sand,
    Water,
    Mud,
    Floor,
    Rug,
    Stone,
    Brick,
    Plaster,
    Wood,
    Roof,
    Thatch,
    Slate,
    Metal,
    Cloth,
    Foliage,
    Trunk,
    Rock,
    Crop,
    Hedge,
    Bone,
    Door,
    Window,
    Fire,
}

/// Each kind: its name, and its capture colour (sRGB, before the light).
/// Muted, told apart from one another, none near black or white.
const KINDS: [(Kind, &str, [u8; 3]); 27] = [
    (Kind::Ground, "ground", [0x85, 0x76, 0x59]),
    (Kind::Grass, "grass", [0x77, 0xa0, 0x52]),
    (Kind::Path, "path", [0xad, 0x94, 0x6a]),
    (Kind::Road, "road", [0x8a, 0x8c, 0x90]),
    (Kind::Sand, "sand", [0xdc, 0xb8, 0x78]),
    (Kind::Water, "water", [0x4a, 0x80, 0xb0]),
    (Kind::Mud, "mud", [0x4f, 0x4a, 0x36]),
    (Kind::Floor, "floor", [0xb0, 0x80, 0x50]),
    (Kind::Rug, "rug", [0x9c, 0x44, 0x48]),
    (Kind::Stone, "stone", [0xb4, 0xb2, 0xa6]),
    (Kind::Brick, "brick", [0xa3, 0x55, 0x3d]),
    (Kind::Plaster, "plaster", [0xd4, 0xd4, 0xcc]),
    (Kind::Wood, "wood", [0x7f, 0x55, 0x2e]),
    (Kind::Roof, "roof", [0xc0, 0x68, 0x3e]),
    (Kind::Thatch, "thatch", [0xb5, 0xa2, 0x58]),
    (Kind::Slate, "slate", [0x5a, 0x64, 0x75]),
    (Kind::Metal, "metal", [0x7a, 0x90, 0xa4]),
    (Kind::Cloth, "cloth", [0x86, 0x59, 0x9e]),
    (Kind::Foliage, "foliage", [0x48, 0x75, 0x36]),
    (Kind::Trunk, "trunk", [0x6e, 0x44, 0x30]),
    (Kind::Rock, "rock", [0x6b, 0x6a, 0x64]),
    (Kind::Crop, "crop", [0xa9, 0xb5, 0x52]),
    (Kind::Hedge, "hedge", [0x3b, 0x6b, 0x5a]),
    (Kind::Bone, "bone", [0xe6, 0xd6, 0xa6]),
    (Kind::Door, "door", [0x4b, 0x44, 0x86]),
    (Kind::Window, "window", [0xee, 0xc6, 0x6a]),
    (Kind::Fire, "fire", [0xe8, 0x74, 0x2c]),
];

/// The three levels an id colour's channels take: kind `i` (in table order)
/// is `[L[i / 9], L[i / 3 % 3], L[i % 3]]`, so the 27 kinds are the 27
/// colours of the levels, none black (the id map's nothing).
const ID_LEVELS: [u8; 3] = [0x30, 0x90, 0xf0];

impl Kind {
    /// Every kind, in the table's order.
    pub const ALL: [Kind; 27] = {
        let mut all = [Kind::Ground; 27];
        let mut i = 0;
        while i < 27 {
            all[i] = KINDS[i].0;
            i += 1;
        }
        all
    };

    fn index(self) -> usize {
        self as usize
    }

    /// The name a set file writes it by.
    pub fn name(self) -> &'static str {
        KINDS[self.index()].1
    }

    /// The kind a name stands for.
    pub fn from_name(name: &str) -> Option<Kind> {
        KINDS.iter().find(|k| k.1 == name).map(|k| k.0)
    }

    /// Its flat colour in the capture (sRGB, before the light shades it).
    pub fn capture(self) -> [u8; 3] {
        KINDS[self.index()].2
    }

    /// Its exact colour in the id map, unique among the kinds.
    pub fn id(self) -> [u8; 3] {
        let i = self.index();
        [ID_LEVELS[i / 9], ID_LEVELS[i / 3 % 3], ID_LEVELS[i % 3]]
    }

    /// The kind an id map's colour is, exactly; `None` for any other colour.
    pub fn from_id(rgb: [u8; 3]) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == rgb)
    }

    /// Whether it shines by itself (a lit window, a fire): the capture does
    /// not shade it.
    pub fn shines(self) -> bool {
        matches!(self, Kind::Window | Kind::Fire)
    }

    /// Its capture colour two shades down its bank: the realm's nearest
    /// plain ink taken two steps down (`light::step_down` in the Delve),
    /// what a surface of it is drawn with when there is nothing painted of
    /// it to draw (a chunk with no plate and no patches; a projected surface
    /// its projector does not see, with no patch). The Delve checks the
    /// table against its own inks (`world3d::kingdom::darker`).
    pub fn dark(self) -> [u8; 3] {
        DARK[self.index()]
    }

    /// Its brightness in a look that lights its scene (docs/REALM-MONOLITH.md):
    /// a flat grey before the light, which the place's ramp then colours.
    /// Each sits in its brand class's band (docs/BRAND.md, `brand::class_of`).
    /// Roads and paths are the brightest ground (a ribbon to follow), woods
    /// and doors the darkest masses.
    pub fn tone(self) -> f32 {
        TONES[self.index()]
    }

    /// What it is to a look that lights its scene beyond its tone: water
    /// runs and glints, everything else is plain.
    pub fn role(self) -> crate::scene::Role {
        match self {
            Kind::Water => crate::scene::Role::Water,
            _ => crate::scene::Role::Plain,
        }
    }
}

/// [`Kind::tone`], in the table's order.
const TONES: [f32; 27] = [
    0.24, // ground
    0.22, // grass
    0.42, // path
    0.48, // road
    0.40, // sand
    0.50, // water
    0.16, // mud
    0.36, // floor
    0.26, // rug
    0.32, // stone
    0.28, // brick
    0.34, // plaster: a mass, never brighter than the ground (docs/BRAND.md)
    0.22, // wood
    0.26, // roof
    0.32, // thatch
    0.20, // slate
    0.30, // metal
    0.30, // cloth
    0.13, // foliage
    0.12, // trunk
    0.26, // rock
    0.30, // crop
    0.14, // hedge
    0.34, // bone: dressing, never brighter than a mass (docs/BRAND.md)
    0.12, // door
    0.90, // window
    0.90, // fire
];

/// [`Kind::dark`], in the table's order.
const DARK: [[u8; 3]; 27] = [
    [0x3d, 0x3a, 0x36], // ground
    [0x33, 0x50, 0x22], // grass
    [0x51, 0x2d, 0x18], // path
    [0x3d, 0x3a, 0x36], // road
    [0x68, 0x46, 0x29], // sand
    [0x22, 0x30, 0x3a], // water
    [0x32, 0x2b, 0x2b], // mud
    [0x51, 0x2d, 0x18], // floor
    [0x45, 0x27, 0x12], // rug
    [0x51, 0x53, 0x49], // stone
    [0x45, 0x29, 0x1c], // brick
    [0x63, 0x65, 0x5c], // plaster
    [0x33, 0x1a, 0x0c], // wood
    [0x45, 0x29, 0x1c], // roof
    [0x51, 0x2d, 0x18], // thatch
    [0x13, 0x27, 0x24], // slate
    [0x26, 0x3b, 0x40], // metal
    [0x26, 0x3b, 0x40], // cloth
    [0x26, 0x3b, 0x1c], // foliage
    [0x33, 0x1a, 0x0c], // trunk
    [0x32, 0x2b, 0x2b], // rock
    [0x4b, 0x59, 0x1f], // crop
    [0x13, 0x27, 0x24], // hedge
    [0x68, 0x46, 0x29], // bone
    [0x13, 0x27, 0x24], // door
    [0x68, 0x46, 0x29], // window
    [0x51, 0x2d, 0x18], // fire
];

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

fn unit_rgba(rgb: [u8; 3]) -> [f32; 4] {
    [f32::from(rgb[0]) / 255.0, f32::from(rgb[1]) / 255.0, f32::from(rgb[2]) / 255.0, 1.0]
}

// --------------------------------------------------------------- camera ----

/// The game camera an item is seen through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SetCamera {
    /// `camera room <w> <h>`: the Delve's camera for a room drawn on a stage
    /// of `size` pixels, exactly ([`stage_camera`] at [`GAME_TILT`]). The
    /// world is the room's stage: its pixel `(x, y)` is the floor point
    /// `(x, 0, y)`.
    Room { size: (u32, u32) },
    /// `camera chunk <w> <h> centre <cx> <cz>`: a stretch of the kingdom on
    /// a stage of `size` pixels, the floor point `(cx, 0, cz)` (kingdom
    /// pixels) where the room camera has the stage's middle `(w / 2, 0,
    /// h / 2)`: the room camera, the world moved by `(w / 2 - cx, 0,
    /// h / 2 - cz)` under it.
    Chunk { size: (u32, u32), centre: [f32; 2] },
}

impl SetCamera {
    /// The stage's size in its pixels.
    pub fn size(&self) -> (u32, u32) {
        match *self {
            SetCamera::Room { size } | SetCamera::Chunk { size, .. } => size,
        }
    }

    /// The camera the game draws the item with, onto its stage (any target
    /// of the stage's shape).
    pub fn game(&self) -> Camera {
        let size = self.size();
        let mut camera = stage_camera(GAME_TILT, size);
        if let SetCamera::Chunk { centre, .. } = *self {
            // A camera's place is undone by its view: placed at the centre's
            // offset from the stage's middle, it moves the world back by it.
            camera.transform =
                Transform::from_translation([centre[0] - size.0 as f32 / 2.0, 0.0, centre[1] - size.1 as f32 / 2.0]);
        }
        camera
    }

    /// The floor the stage covers in the set's world, `[x0, z0, x1, z1]`: a
    /// room's `(0, 0)` to its size; a chunk's size about its centre, in
    /// kingdom pixels.
    pub fn stage(&self) -> [f32; 4] {
        match *self {
            SetCamera::Room { size } => [0.0, 0.0, size.0 as f32, size.1 as f32],
            SetCamera::Chunk { size, centre } => {
                let (hw, hh) = (size.0 as f32 / 2.0, size.1 as f32 / 2.0);
                [centre[0] - hw, centre[1] - hh, centre[0] + hw, centre[1] + hh]
            }
        }
    }
}

/// How far apart two stages' edges may be (kingdom pixels) and still meet.
pub const BORDER_TOLERANCE: f32 = 0.5;

/// The edge two sets' stages share (docs/WORLD-3D.md, "Seams").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    /// The shared edge's ends as floor points `(x, z)`, the lower coordinate
    /// along it first.
    pub from: [f32; 2],
    pub to: [f32; 2],
    /// A unit step across the edge, from the set to its neighbour: `(1, 0)`
    /// east, `(-1, 0)` west, `(0, 1)` down the floor toward the viewer,
    /// `(0, -1)` up it.
    pub across: [f32; 2],
}

impl Border {
    /// The floor point a fraction `t` (0 to 1) along the edge.
    pub fn at(&self, t: f32) -> [f32; 2] {
        [self.from[0] + (self.to[0] - self.from[0]) * t, self.from[1] + (self.to[1] - self.from[1]) * t]
    }

    /// The edge's length in stage pixels.
    pub fn length(&self) -> f32 {
        (self.to[0] - self.from[0]).hypot(self.to[1] - self.from[1])
    }

    /// Which way the neighbour lies: `east`, `west`, `south` (down the floor
    /// toward the viewer) or `north`.
    pub fn side(&self) -> &'static str {
        match (self.across[0] > 0.5, self.across[0] < -0.5, self.across[1] > 0.5) {
            (true, _, _) => "east",
            (_, true, _) => "west",
            (_, _, true) => "south",
            _ => "north",
        }
    }
}

// ---------------------------------------------------------------- shape ----

/// One block of a set: a built-in mesh by name, sized and placed by its foot.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The line of the set file it was read from.
    pub line: usize,
    /// The shape's name as written (`shapes::by_name`).
    pub shape: String,
    pub mesh: MeshRef,
    pub kind: Kind,
    /// The middle of its footprint, at the height its bottom sits.
    pub at: [f32; 3],
    /// Its extent along x, y and z before it is turned.
    pub size: [f32; 3],
    /// Degrees about the vertical, counter-clockwise seen from above.
    pub turn: f32,
    /// A name for tools to find it by, unique in its set.
    pub id: Option<String>,
    /// A box's top its own tone, 0 to 1, in a look that lights its scene
    /// (`top <0..100>` on its line): a cap read as dark mass, not floor.
    pub top: Option<f32>,
}

impl Shape {
    /// Where the mesh goes: scaled so its box is `size` (an axis it is flat
    /// along keeps its own), turned about the vertical through its foot,
    /// and moved so its foot (the middle of its box's bottom) is `at`.
    pub fn transform(&self) -> Transform {
        let (lo, hi) = bounds(self.mesh);
        let scale: [f32; 3] = std::array::from_fn(|k| {
            let extent = hi[k] - lo[k];
            if extent > 1e-6 { self.size[k] / extent } else { 1.0 }
        });
        let rotation = quat_axis_angle([0.0, 1.0, 0.0], self.turn.to_radians());
        let foot = [(lo[0] + hi[0]) * 0.5 * scale[0], lo[1] * scale[1], (lo[2] + hi[2]) * 0.5 * scale[2]];
        let turned = quat_rotate(rotation, foot);
        Transform {
            translation: [self.at[0] - turned[0], self.at[1] - turned[1], self.at[2] - turned[2]],
            rotation,
            scale,
        }
    }
}

impl Shape {
    /// Whether it lies on the floor rather than standing on it: its top is
    /// at most [`FLOOR_TOP`] over the floor (the ground, a path, a pond).
    pub fn on_floor(&self) -> bool {
        self.at[1] + self.size[1] <= FLOOR_TOP
    }
}

/// How high (stage pixels) the top of a shape that lies on the floor may be.
pub const FLOOR_TOP: f32 = 1.0;

/// The box a built-in mesh fills, low and high corners (a unit cube's for
/// one that is not built in).
fn bounds(mesh: MeshRef) -> ([f32; 3], [f32; 3]) {
    let Some(data) = mesh.generate().filter(|d| !d.positions.is_empty()) else {
        return ([-0.5; 3], [0.5; 3]);
    };
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in &data.positions {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

// --------------------------------------------------------------- ground ----

/// The most corners a ground's height field has on a side.
pub const MAX_GROUND_SIDE: usize = 512;

/// A set's ground as one height field (docs/REALM-MONOLITH.md): `cols` by
/// `rows` cells, `step` stage pixels apart, its first corner at floor point
/// `at` (x, z), and each of its `(cols + 1) * (rows + 1)` corners' height
/// over the floor and tone (0 to 100), row by row from the corner, x first.
/// A set whose neighbour gives their shared edge the same heights and tones
/// meets it with no seam.
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cols: usize,
    pub rows: usize,
    pub step: f32,
    pub at: [f32; 2],
    pub heights: Vec<f32>,
    pub tones: Vec<u8>,
}

impl Ground {
    /// Corner `(i, j)`'s height (clamped into the field).
    pub fn height(&self, i: usize, j: usize) -> f32 {
        self.heights[j.min(self.rows) * (self.cols + 1) + i.min(self.cols)]
    }

    /// The height at floor point `(x, z)`, between the corners round it
    /// (the field's edge past its ends).
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.at[0]) / self.step).clamp(0.0, self.cols as f32);
        let fz = ((z - self.at[1]) / self.step).clamp(0.0, self.rows as f32);
        let (i, j) = ((fx.floor() as usize).min(self.cols.saturating_sub(1)), (fz.floor() as usize).min(self.rows.saturating_sub(1)));
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let a = self.height(i, j) + (self.height(i + 1, j) - self.height(i, j)) * tx;
        let b = self.height(i, j + 1) + (self.height(i + 1, j + 1) - self.height(i, j + 1)) * tx;
        a + (b - a) * tz
    }

    /// The field as a mesh in the set's world: each corner its height and
    /// its tone as a grey, normals smooth across the corners.
    pub fn mesh(&self) -> MeshData {
        let (w, h) = (self.cols + 1, self.rows + 1);
        let mut mesh = MeshData::default();
        for j in 0..h {
            for i in 0..w {
                let x = self.at[0] + i as f32 * self.step;
                let z = self.at[1] + j as f32 * self.step;
                let dx = self.height(i + 1, j) - self.height(i.saturating_sub(1), j);
                let dz = self.height(i, j + 1) - self.height(i, j.saturating_sub(1));
                let span = |k: usize, n: usize| if k == 0 || k + 1 == n { 1.0 } else { 2.0 };
                let n = [-dx / (span(i, w) * self.step), 1.0, -dz / (span(j, h) * self.step)];
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                mesh.positions.push([x, self.height(i, j), z]);
                mesh.normals.push(n.map(|c| c / len));
                mesh.uvs.push([0.0, 0.0]);
                // Ground is ground or a route: its tones are held in their
                // bands (docs/BRAND.md), whatever a set writes.
                let t = f32::from(self.tones[j * w + i].min(100)) / 100.0;
                let t = t.clamp(brand::Class::Ground.band().tone.0, brand::Class::Route.band().tone.1);
                mesh.colors.push([t, t, t, 1.0]);
            }
        }
        for j in 0..self.rows {
            for i in 0..self.cols {
                let a = (j * w + i) as u32;
                let (b, c) = (a + 1, a + w as u32);
                let d = c + 1;
                // Counter-clockwise from above (+Y), z toward the viewer.
                mesh.indices.extend([a, c, d, a, d, b]);
            }
        }
        mesh
    }
}

/// A lamp a set stands (docs/REALM-MONOLITH.md): light = activity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetLamp {
    pub at: [f32; 3],
    pub reach: f32,
    pub strength: f32,
}

/// The mesh id a set's ground is uploaded as: its own for every set key,
/// in a range of its own.
pub fn ground_mesh_id(key: u64) -> u32 {
    0x5000_0000 | ((key ^ (key >> 29)) as u32 & 0x0FFF_FFFF)
}

// ------------------------------------------------------------------ set ----

/// One fixed view of the world in blocks (docs/WORLD-3D.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Set {
    pub id: String,
    pub camera: SetCamera,
    /// The capture's (and the plate's) size in pixels.
    pub plate: (u32, u32),
    pub shapes: Vec<Shape>,
    /// Its ground as one height field, when it says one.
    pub ground: Option<Ground>,
    /// The lamps it stands.
    pub lamps: Vec<SetLamp>,
    /// Its own tones for kinds (`tone <kind> <0..100>`), over [`Kind::tone`].
    pub tones: Vec<(Kind, f32)>,
    /// Its own roles for kinds (`role <kind> …`), over [`Kind::role`].
    pub roles: Vec<(Kind, crate::scene::Role)>,
}

/// Which scene a set makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Each kind's flat colour, lit by the scene's one light (a lit window
    /// and a fire shine by themselves): what the painter paints over.
    Capture,
    /// Each kind's exact id colour, unlit, so a pixel says what it is.
    Ids,
    /// Picture `picture` (the painted plate) cast from the capture's camera
    /// onto every block, unlit; a surface the plate does not see is its
    /// kind's colour two shades down ([`Kind::dark`]; its patch of the plate
    /// in a world, [`Set::world_nodes`]), shaded as a plate lights a block.
    Projected { picture: u32 },
}

/// Why a line of a set file was refused. Line 0 is the file as a whole (a
/// statement it lacks).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub line: usize,
    pub why: String,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            0 => write!(f, "the set: {}", self.why),
            line => write!(f, "line {line}: {}", self.why),
        }
    }
}

impl std::error::Error for Refusal {}

impl Set {
    /// Read a set file. Every line that cannot be read is refused, with its
    /// line number and why; any refusal and there is no set.
    pub fn parse(text: &str) -> Result<Set, Vec<Refusal>> {
        let mut refusals = Vec::new();
        let mut id: Option<String> = None;
        let mut camera: Option<(usize, SetCamera)> = None;
        let mut plate: Option<(usize, (u32, u32))> = None;
        let mut shapes: Vec<Shape> = Vec::new();
        let mut ground: Option<(usize, Ground)> = None;
        let mut lamps: Vec<SetLamp> = Vec::new();
        let mut tones: Vec<(Kind, f32)> = Vec::new();
        let mut roles: Vec<(Kind, crate::scene::Role)> = Vec::new();
        let mut first = true;
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let words: Vec<&str> = raw.split('#').next().unwrap_or("").split_whitespace().collect();
            let Some(&head) = words.first() else {
                continue;
            };
            let mut refuse = |why: String| refusals.push(Refusal { line, why });
            if first && head != "set" {
                refuse("a set file starts with `set <id>`".into());
            }
            first = false;
            match head {
                "set" => match (&id, words.as_slice()) {
                    (Some(_), _) => refuse("a second `set` line".into()),
                    (None, [_, name]) => match check_id(name) {
                        Ok(()) => id = Some((*name).to_string()),
                        Err(why) => refuse(why),
                    },
                    (None, _) => refuse("`set` takes one id: `set <id>`".into()),
                },
                "camera" => match (camera, read_camera(&words[1..])) {
                    (Some((at, _)), _) => refuse(format!("a second `camera` line (the first is line {at})")),
                    (None, Ok(c)) => camera = Some((line, c)),
                    (None, Err(why)) => refuse(why),
                },
                "plate" => match (plate, words.as_slice()) {
                    (Some((at, _)), _) => refuse(format!("a second `plate` line (the first is line {at})")),
                    (None, [_, w, h]) => match (side(w, "plate width"), side(h, "plate height")) {
                        (Ok(w), Ok(h)) => plate = Some((line, (w, h))),
                        (Err(why), _) | (_, Err(why)) => refuse(why),
                    },
                    (None, _) => refuse("`plate` takes a width and a height: `plate <w> <h>`".into()),
                },
                "ground" => match (&ground, read_ground(&words)) {
                    (Some((at, _)), _) => refuse(format!("a second `ground` line (the first is line {at})")),
                    (None, Ok(g)) => ground = Some((line, g)),
                    (None, Err(why)) => refuse(why),
                },
                "lamp" => match read_lamp(&words) {
                    Ok(lamp) => lamps.push(lamp),
                    Err(why) => refuse(why),
                },
                "tone" => match read_tone(&words) {
                    Ok(t) => tones.push(t),
                    Err(why) => refuse(why),
                },
                "role" => match read_role(&words) {
                    Ok(r) => roles.push(r),
                    Err(why) => refuse(why),
                },
                _ => match read_shape(line, &words) {
                    Ok(shape) => {
                        let taken = shape.id.as_ref().and_then(|name| {
                            shapes.iter().find(|s| s.id.as_ref() == Some(name)).map(|s| s.line)
                        });
                        if let Some(at) = taken {
                            refuse(format!("the id `{}` is line {at}'s already", shape.id.as_deref().unwrap_or("")));
                        } else if shapes.len() == MAX_SHAPES {
                            refuse(format!("more than {MAX_SHAPES} shapes"));
                        } else {
                            shapes.push(shape);
                        }
                    }
                    Err(why) => refuse(why),
                },
            }
        }
        if first {
            refusals.push(Refusal { line: 0, why: "no `set <id>` line: the file is empty".into() });
        } else if id.is_none() && refusals.is_empty() {
            refusals.push(Refusal { line: 0, why: "no `set <id>` line".into() });
        }
        if camera.is_none() {
            refusals.push(Refusal { line: 0, why: "no `camera` line".into() });
        }
        match (refusals.is_empty(), id, camera) {
            (true, Some(id), Some((_, camera))) => {
                Ok(Set { id, camera, plate: plate.map_or(DEFAULT_PLATE, |p| p.1), shapes, ground: ground.map(|g| g.1), lamps, tones, roles })
            }
            _ => Err(refusals),
        }
    }

    /// The clip-space scale that frames the stage in the plate: the stage
    /// fills the plate less [`PLATE_MARGIN`] a side, its pixels square,
    /// centred. `[sx, sy]`: the stage's clip x and y are multiplied by them.
    pub fn framing(&self) -> [f32; 2] {
        let (w, h) = self.camera.size();
        let (pw, ph) = (self.plate.0.max(1) as f32, self.plate.1.max(1) as f32);
        let room = 1.0 - 2.0 * PLATE_MARGIN;
        let scale = (pw * room / w.max(1) as f32).min(ph * room / h.max(1) as f32);
        [scale * w.max(1) as f32 / pw, scale * h.max(1) as f32 / ph]
    }

    /// The capture's camera, which is also the plate's projector: the game
    /// camera framed in the plate. Its matrix is
    /// `diag(sx, sy, 1, 1) * game.view_projection` ([`Set::framing`]).
    pub fn projector(&self) -> Projector {
        let [sx, sy] = self.framing();
        let frame: Mat4 = [[sx, 0.0, 0.0, 0.0], [0.0, sy, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        let game = self.camera.game();
        let aspect = self.camera.size().0.max(1) as f32 / self.camera.size().1.max(1) as f32;
        // The game camera always sees (a stage camera's matrix is finite).
        let view_projection = game.view_projection(aspect).unwrap_or(crate::scene::IDENTITY_MATRIX);
        Projector { view_projection: mat_mul(&frame, &view_projection) }
    }

    /// The camera the capture is drawn with, at the plate's size, cleared to
    /// [`BACKGROUND`].
    pub fn capture_camera(&self) -> Camera {
        Camera {
            transform: Transform::IDENTITY,
            projection: Projection::Custom(self.projector().view_projection),
            clear: Some(BACKGROUND),
        }
    }

    /// The edge this set's stage shares with `other`'s, when the two stages
    /// touch along one (within [`BORDER_TOLERANCE`], their other sides
    /// overlapping by more than that): chunks side by side on the kingdom's
    /// floor. Stages that overlap, only meet at a corner or lie apart share
    /// none.
    pub fn border_with(&self, other: &Set) -> Option<Border> {
        let (a, b) = (self.camera.stage(), other.camera.stage());
        let meets = |p: f32, q: f32| (p - q).abs() <= BORDER_TOLERANCE;
        // The stretch of the other axis both stages cover.
        let along = |lo: usize| {
            let (from, to) = (a[lo].max(b[lo]), a[lo + 2].min(b[lo + 2]));
            (to - from > BORDER_TOLERANCE).then_some((from, to))
        };
        if meets(a[2], b[0]) || meets(a[0], b[2]) {
            let (from, to) = along(1)?;
            let (x, across) = if meets(a[2], b[0]) { (a[2], 1.0) } else { (a[0], -1.0) };
            return Some(Border { from: [x, from], to: [x, to], across: [across, 0.0] });
        }
        if meets(a[3], b[1]) || meets(a[1], b[3]) {
            let (from, to) = along(0)?;
            let (z, across) = if meets(a[3], b[1]) { (a[3], 1.0) } else { (a[1], -1.0) };
            return Some(Border { from: [from, z], to: [to, z], across: [0.0, across] });
        }
        None
    }

    /// Where the world point `p` lands in the capture, in plate pixels from
    /// the top left (`None` when the camera cannot see it).
    pub fn plate_pixel(&self, p: [f32; 3]) -> Option<[f32; 2]> {
        let m = self.projector().view_projection;
        let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
        let w = row(3);
        if !(w.is_finite() && w > 0.0) {
            return None;
        }
        let (x, y) = (row(0) / w, row(1) / w);
        Some([(x + 1.0) * 0.5 * self.plate.0 as f32, (1.0 - y) * 0.5 * self.plate.1 as f32])
    }

    /// Plate pixels to a pixel of the stage: how densely the plate paints
    /// the set's world ([`Set::framing`]'s scale).
    pub fn density(&self) -> f32 {
        let (w, h) = self.camera.size();
        let (pw, ph) = (self.plate.0.max(1) as f32, self.plate.1.max(1) as f32);
        let room = 1.0 - 2.0 * PLATE_MARGIN;
        (pw * room / w.max(1) as f32).min(ph * room / h.max(1) as f32)
    }

    /// The set's blocks as nodes (ids 1 up, in the file's order), drawn as
    /// `mode` says, for any camera: the capture's, or the game's.
    pub fn nodes(&self, mode: Mode) -> Vec<Node> {
        self.nodes_with(mode, Patches::NONE)
    }

    /// [`Set::nodes`], a projected surface the plate does not see drawn with
    /// its kind's patch of `patches` at the plate's own density.
    fn nodes_with(&self, mode: Mode, patches: Patches) -> Vec<Node> {
        let projector = self.projector();
        let texel = 1.0 / self.density();
        self.shapes
            .iter()
            .enumerate()
            .map(|(i, shape)| {
                let kind = shape.kind;
                let material = match mode {
                    Mode::Capture => Material::lit(unit_rgba(kind.capture()), if kind.shines() { 1.0 } else { 0.0 }),
                    Mode::Ids => Material::color(unit_rgba(kind.id())),
                    Mode::Projected { picture } => Material {
                        // A surface that shines is not shaded where it is not seen.
                        emissive: if kind.shines() { 1.0 } else { 0.0 },
                        patch: patches.rect(kind).map(|rect| Patch { picture: patches.atlas, rect, origin: [0.0; 3], texel }),
                        ..Material::projected(picture, projector, unit_rgba(kind.dark()))
                    },
                };
                let mut node = Node::new(i as u64 + 1, shape.mesh, material);
                node.transform = shape.transform();
                node
            })
            .collect()
    }

    /// [`Set::nodes`] put on a game's stage: the set's world scaled by
    /// `scale` about its origin, then moved by `offset`, as a room's set is
    /// when the room's picture is another size than the set's stage, or is
    /// scrolled by the game's camera. A projected block's plate stays where
    /// it was painted: its projector is placed with it (the projector of a
    /// placed point is the set's projector of where the point was).
    pub fn placed_nodes(&self, mode: Mode, scale: f32, offset: [f32; 3]) -> Vec<Node> {
        place(&self.nodes(mode), scale, offset)
    }

    /// The set's blocks with picture `plate` cast on them, parted as a
    /// [`World`] draws them: what lies on the floor ([`Shape::on_floor`]),
    /// and what stands on it, a surface the plate does not see drawn with
    /// its kind's patch of `patches`. Unplaced: [`place`] puts them on a
    /// stage, every frame, for much less than making them again.
    pub fn world_nodes(&self, plate: u32, patches: Patches) -> (Vec<Node>, Vec<Node>) {
        let nodes = self.nodes_with(Mode::Projected { picture: plate }, patches);
        let (mut floor, mut blocks) = (Vec::new(), Vec::new());
        for (node, shape) in nodes.into_iter().zip(&self.shapes) {
            if shape.on_floor() { floor.push(node) } else { blocks.push(node) }
        }
        (floor, blocks)
    }

    /// [`Set::world_nodes`] placed as [`Set::placed_nodes`] places them.
    pub fn placed_world(&self, plate: u32, patches: Patches, scale: f32, offset: [f32; 3]) -> (Vec<Node>, Vec<Node>) {
        let (floor, blocks) = self.world_nodes(plate, patches);
        (place(&floor, scale, offset), place(&blocks, scale, offset))
    }

    /// The set's world in a look that lights its scene
    /// (docs/REALM-MONOLITH.md), parted as [`Set::world_nodes`] parts it:
    /// what lies on the floor (its ground's height field first, as uploaded
    /// mesh `ground_mesh`, then the shapes that lie on the floor) and what
    /// stands on it. Each shape is its kind's tone, lit; water runs; a lit
    /// window and a fire are accents in their own colours. With them, the
    /// ground's mesh, when the set has one, to upload as `ground_mesh`.
    pub fn lit_world_nodes(&self, ground_mesh: u32) -> (Vec<Node>, Vec<Node>, Option<MeshData>) {
        let (mut floor, mut blocks) = (Vec::new(), Vec::new());
        let mesh = self.ground.as_ref().map(Ground::mesh);
        if mesh.is_some() {
            let ground = Material { ceiling: brand::Class::Ground.material_ceiling(), ..Material::lit([1.0; 4], 0.0) };
            let mut node = Node::new(1 << 32, MeshRef::Uploaded(ground_mesh), ground);
            node.transform = Transform::IDENTITY;
            floor.push(node);
        }
        for (i, shape) in self.shapes.iter().enumerate() {
            let kind = shape.kind;
            // Its class's tone and ceiling (docs/BRAND.md): light never
            // lifts it past its class's step. A tone the set gives its kind
            // is the set's own.
            let class = brand::class_of(kind);
            let tone = self.tones.iter().rev().find(|t| t.0 == kind).map_or(class.clamp_tone(kind.tone()), |t| t.1);
            let role = self.roles.iter().rev().find(|r| r.0 == kind).map_or(kind.role(), |r| r.1);
            let toned = |t: f32| Material { role, ceiling: class.material_ceiling(), ..Material::lit([t, t, t, 1.0], 0.0) };
            // A kind that shines is an accent in its own colour, unless the
            // set makes it glow on the warm ramp (an ember) or plain.
            let shines = kind.shines() && !self.roles.iter().any(|r| r.0 == kind);
            let material = if shines { Material { accent: true, ..Material::color(unit_rgba(kind.capture())) } } else { toned(tone) };
            let mut node = Node::new(i as u64 + 1, shape.mesh, material);
            if let (Some(top), MeshRef::Cube, false) = (shape.top, shape.mesh, shines) {
                // The cube's faces in order +X, -X, +Y, -Y, +Z, -Z: the top its own.
                node.face_materials = (0..6).map(|f| if f == 2 { toned(top) } else { toned(tone) }).collect();
            }
            node.transform = shape.transform();
            if shape.on_floor() { floor.push(node) } else { blocks.push(node) }
        }
        (floor, blocks, mesh)
    }

    /// The set's lamps as a look lights with them.
    pub fn look_lamps(&self) -> Vec<crate::look::Lamp> {
        // No lamp brighter than the brand guide's (docs/BRAND.md).
        self.lamps
            .iter()
            .map(|l| crate::look::Lamp { at: l.at, reach: l.reach, strength: l.strength.min(brand::LAMP_STRENGTH) })
            .collect()
    }

    /// The scene `mode` makes, seen by the capture's camera: drawn at the
    /// plate's size it is the capture, the id map, or the plate cast back.
    pub fn scene(&self, mode: Mode) -> Scene {
        Scene { camera: self.capture_camera(), nodes: self.nodes(mode), look: None }
    }
}

/// A set's nodes (as [`Set::nodes`] or [`Set::world_nodes`] made them) put
/// on a game's stage as [`Set::placed_nodes`] says.
pub fn place(nodes: &[Node], scale: f32, offset: [f32; 3]) -> Vec<Node> {
    let placing = Transform { translation: offset, scale: [scale; 3], ..Transform::IDENTITY };
    // The way back from the stage to the set's own world.
    let back: Mat4 = [
        [1.0 / scale, 0.0, 0.0, 0.0],
        [0.0, 1.0 / scale, 0.0, 0.0],
        [0.0, 0.0, 1.0 / scale, 0.0],
        [-offset[0] / scale, -offset[1] / scale, -offset[2] / scale, 1.0],
    ];
    nodes
        .iter()
        .map(|node| {
            let mut node = node.clone();
            node.transform = placing.then(&node.transform);
            if let Some(projector) = &mut node.material.projector {
                projector.view_projection = mat_mul(&projector.view_projection, &back);
            }
            if let Some(patch) = &mut node.material.patch {
                patch.origin = [0, 1, 2].map(|k| patch.origin[k] * scale + offset[k]);
                patch.texel *= scale;
            }
            node
        })
        .collect()
}

// ----------------------------------------------------------------- sets ----

/// The most sets [`Sets`] keeps: a room's and the rooms either side of it,
/// or the kingdom's chunks near a view (at most 16) and the room left.
pub const KEPT_SETS: usize = 32;

/// The sets the far side of a wire has been sent, by key, to make a world's
/// floor and blocks again from its reference ([`crate::scene::SetRef`]).
#[derive(Debug, Default)]
pub struct Sets {
    /// Newest last: each set, and its world's nodes as last made (for the
    /// plate they were made with), which are placed every frame.
    kept: Vec<Kept>,
}

#[derive(Debug)]
struct Kept {
    key: u64,
    set: Set,
    made: Option<((u32, Patches), Vec<Node>, Vec<Node>)>,
    /// Its world made lit (a world with a look), and its ground's mesh.
    lit: Option<(Vec<Node>, Vec<Node>, Option<std::sync::Arc<MeshData>>)>,
}

impl Sets {
    /// `world`'s floor and blocks made from its set reference, the set read
    /// from the reference's text when it comes with it (and kept). False when
    /// it names a set that is not kept, or whose text is refused: the world
    /// then has no floor or blocks, and the far side should be sent
    /// everything again. A world with no reference is left as it is.
    pub fn fill(&mut self, world: &mut World) -> bool {
        let Some(set) = &world.set else {
            return true;
        };
        let lit = world.look.is_some();
        world.meshes.clear();
        if world.more.is_empty() {
            return self.fill_one(set, &mut world.floor, &mut world.blocks, lit, &mut world.meshes);
        }
        // Several sets (the kingdom's chunks): each one's floor and blocks
        // after the last's.
        let (mut floor, mut blocks) = (Vec::new(), Vec::new());
        let mut whole = true;
        for (k, set) in std::iter::once(set).chain(&world.more).enumerate() {
            let (mut f, mut b) = (Vec::new(), Vec::new());
            whole &= self.fill_one(set, &mut f, &mut b, lit, &mut world.meshes);
            // A scene draws one node an id: each set's its own.
            for node in f.iter_mut().chain(&mut b) {
                node.id |= (k as u64 + 1) << 40;
            }
            floor.append(&mut f);
            blocks.append(&mut b);
        }
        if !whole {
            (floor, blocks) = (Vec::new(), Vec::new());
        }
        (world.floor, world.blocks) = (floor, blocks);
        whole
    }

    /// Set reference `set`'s floor and blocks into `floor` and `blocks`
    /// (`fill` of a world of one set).
    fn fill_one(
        &mut self,
        set: &SetRef,
        out_floor: &mut Vec<Node>,
        out_blocks: &mut Vec<Node>,
        lit: bool,
        meshes: &mut Vec<(u32, std::sync::Arc<MeshData>)>,
    ) -> bool {
        if let Some(text) = &set.text
            && !self.kept.iter().any(|k| k.key == set.key)
        {
            match Set::parse(text) {
                Ok(parsed) => {
                    if self.kept.len() >= KEPT_SETS {
                        self.kept.remove(0);
                    }
                    self.kept.push(Kept { key: set.key, set: parsed, made: None, lit: None });
                }
                Err(_) => {
                    (*out_floor, *out_blocks) = (Vec::new(), Vec::new());
                    return false;
                }
            }
        }
        let Some(at) = self.kept.iter().position(|k| k.key == set.key) else {
            (*out_floor, *out_blocks) = (Vec::new(), Vec::new());
            return false;
        };
        // The one used is the newest.
        let mut kept = self.kept.remove(at);
        if lit {
            // Lit (docs/REALM-MONOLITH.md): toned and lit, no plate, the
            // ground's mesh uploaded under its set's own id.
            let mesh_id = ground_mesh_id(set.key);
            if kept.lit.is_none() {
                let (floor, blocks, mesh) = kept.set.lit_world_nodes(mesh_id);
                kept.lit = Some((floor, blocks, mesh.map(std::sync::Arc::new)));
            }
            if let Some((floor, blocks, mesh)) = &kept.lit {
                (*out_floor, *out_blocks) = (place(floor, set.scale, set.offset), place(blocks, set.scale, set.offset));
                if let Some(mesh) = mesh {
                    meshes.push((mesh_id, mesh.clone()));
                }
            }
            self.kept.push(kept);
            return true;
        }
        if kept.made.as_ref().is_none_or(|m| m.0 != (set.plate, set.patches)) {
            let (floor, blocks) = kept.set.world_nodes(set.plate, set.patches);
            kept.made = Some(((set.plate, set.patches), floor, blocks));
        }
        if let Some((_, floor, blocks)) = &kept.made {
            (*out_floor, *out_blocks) = (place(floor, set.scale, set.offset), place(blocks, set.scale, set.offset));
        }
        self.kept.push(kept);
        true
    }
}

// --------------------------------------------------------------- reading ----

/// Lower case letters, digits and `-`, at most [`MAX_ID`] of them.
fn check_id(name: &str) -> Result<(), String> {
    let fine = !name.is_empty()
        && name.len() <= MAX_ID
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if fine {
        Ok(())
    } else {
        Err(format!("`{name}` is not an id (lower case letters, digits and `-`, at most {MAX_ID})"))
    }
}

/// A stage or plate side: a whole number of pixels, 1 to [`MAX_SIDE`].
fn side(word: &str, what: &str) -> Result<u32, String> {
    match word.parse::<u32>() {
        Ok(n) if (1..=MAX_SIDE).contains(&n) => Ok(n),
        _ => Err(format!("the {what} `{word}` is not a whole number of pixels from 1 to {MAX_SIDE}")),
    }
}

/// A finite number.
fn number(word: &str, what: &str) -> Result<f32, String> {
    match word.parse::<f32>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(format!("the {what} `{word}` is not a number")),
    }
}

fn read_camera(words: &[&str]) -> Result<SetCamera, String> {
    match words {
        ["room", w, h] => Ok(SetCamera::Room { size: (side(w, "stage width")?, side(h, "stage height")?) }),
        ["chunk", w, h, "centre", cx, cz] => Ok(SetCamera::Chunk {
            size: (side(w, "stage width")?, side(h, "stage height")?),
            centre: [number(cx, "centre x")?, number(cz, "centre z")?],
        }),
        ["room", ..] => Err("`camera room` takes the stage's size: `camera room <w> <h>`".into()),
        ["chunk", ..] => {
            Err("`camera chunk` takes the stage's size and a centre: `camera chunk <w> <h> centre <cx> <cz>`".into())
        }
        [other, ..] => Err(format!("`{other}` is not a camera (`room` or `chunk`)")),
        [] => Err("`camera` names one: `camera room <w> <h>` or `camera chunk <w> <h> centre <cx> <cz>`".into()),
    }
}

/// `<shape> <kind> at <x> <y> <z> size <sx> <sy> <sz> [turn <deg>] [id <name>]`.
/// A tone word, 0 to 100, as 0 to 1.
fn tone_word(w: &str) -> Result<f32, String> {
    w.parse::<u8>().ok().filter(|t| *t <= 100).map(|t| f32::from(t) / 100.0).ok_or_else(|| format!("`{w}` is not a tone 0 to 100"))
}

/// `tone <kind> <0..100>`: the set's own tone for a kind.
fn read_tone(words: &[&str]) -> Result<(Kind, f32), String> {
    let [_, kind, tone] = words else {
        return Err("`tone <kind> <0..100>` is wanted".into());
    };
    let kind = Kind::from_name(kind).ok_or_else(|| format!("`{kind}` is not a kind"))?;
    Ok((kind, tone_word(tone)?))
}

/// `role <kind> plain|water|ember`: the set's own role for a kind.
fn read_role(words: &[&str]) -> Result<(Kind, crate::scene::Role), String> {
    use crate::scene::Role;
    let [_, kind, role] = words else {
        return Err("`role <kind> plain|water|ember` is wanted".into());
    };
    let kind = Kind::from_name(kind).ok_or_else(|| format!("`{kind}` is not a kind"))?;
    let role = match *role {
        "plain" => Role::Plain,
        "water" => Role::Water,
        "ember" => Role::Ember,
        other => return Err(format!("`{other}` is not plain, water or ember")),
    };
    Ok((kind, role))
}

/// `ground <cols> <rows> step <s> at <x> <z> heights <h...> tones <t...>`.
fn read_ground(words: &[&str]) -> Result<Ground, String> {
    let usage = "`ground <cols> <rows> step <s> at <x> <z> heights <h...> tones <t...>`";
    let [_, cols, rows, "step", step, "at", x, z, "heights", rest @ ..] = words else {
        return Err(format!("{usage} is wanted"));
    };
    let count = |w: &str, what: &str| -> Result<usize, String> {
        match w.parse::<usize>() {
            Ok(n) if (1..MAX_GROUND_SIDE).contains(&n) => Ok(n),
            _ => Err(format!("the {what} is a whole number 1 to {}", MAX_GROUND_SIDE - 1)),
        }
    };
    let (cols, rows) = (count(cols, "ground's columns")?, count(rows, "ground's rows")?);
    let step = number(step, "step")?;
    if step <= 0.0 {
        return Err("a ground's step is more than 0".into());
    }
    let at = [number(x, "ground x")?, number(z, "ground z")?];
    let corners = (cols + 1) * (rows + 1);
    if rest.len() != 2 * corners + 1 || rest[corners] != "tones" {
        return Err(format!("{usage}: {corners} heights, then `tones` and {corners} tones"));
    }
    let heights = rest[..corners].iter().map(|w| number(w, "a height")).collect::<Result<Vec<_>, _>>()?;
    let tones = rest[corners + 1..]
        .iter()
        .map(|w| w.parse::<u8>().ok().filter(|t| *t <= 100).ok_or_else(|| format!("`{w}` is not a tone 0 to 100")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Ground { cols, rows, step, at, heights, tones })
}

/// `lamp at <x> <y> <z> reach <r> strength <s>`.
fn read_lamp(words: &[&str]) -> Result<SetLamp, String> {
    let [_, "at", x, y, z, "reach", reach, "strength", strength] = words else {
        return Err("`lamp at <x> <y> <z> reach <r> strength <s>` is wanted".into());
    };
    let lamp = SetLamp {
        at: [number(x, "lamp x")?, number(y, "lamp y")?, number(z, "lamp z")?],
        reach: number(reach, "reach")?,
        strength: number(strength, "strength")?,
    };
    if lamp.reach <= 0.0 || lamp.strength < 0.0 {
        return Err("a lamp has a reach and no less than no strength".into());
    }
    Ok(lamp)
}

fn read_shape(line: usize, words: &[&str]) -> Result<Shape, String> {
    let name = words[0];
    let Some(mesh) = shapes::by_name(name) else {
        return Err(format!("`{name}` is not a statement or a shape (shapes: {})", shapes::NAMES.join(", ")));
    };
    let Some(&kind_name) = words.get(1) else {
        return Err(format!("`{name}` takes a kind next"));
    };
    let Some(kind) = Kind::from_name(kind_name) else {
        return Err(format!("`{kind_name}` is not a kind (docs/WORLD-3D.md, \"Surface kinds\")"));
    };
    let triple = |at: usize, word: &str, what: &str| -> Result<[f32; 3], String> {
        if words.get(at) != Some(&word) {
            return Err(format!("`{word} <x> <y> <z>` is wanted after the {what}"));
        }
        let mut out = [0.0; 3];
        for (k, axis) in ["x", "y", "z"].iter().enumerate() {
            let Some(w) = words.get(at + 1 + k) else {
                return Err(format!("`{word}` takes three numbers"));
            };
            out[k] = number(w, &format!("{word} {axis}"))?;
        }
        Ok(out)
    };
    let at = triple(2, "at", "kind")?;
    let size = triple(6, "size", "place")?;
    if size.iter().any(|&s| s < 0.0) {
        return Err("a size cannot be less than 0".into());
    }
    if size.iter().all(|&s| s == 0.0) {
        return Err("a shape of no size".into());
    }
    let (mut turn, mut id, mut top) = (None, None, None);
    let mut rest = &words[10..];
    while let [word, value, tail @ ..] = rest {
        match *word {
            "turn" if turn.is_some() => return Err("a second `turn`".into()),
            "turn" => turn = Some(number(value, "turn")?),
            "top" if top.is_some() => return Err("a second `top`".into()),
            "top" => top = Some(tone_word(value)?),
            "id" if id.is_some() => return Err("a second `id`".into()),
            "id" => {
                check_id(value)?;
                id = Some((*value).to_string());
            }
            other => return Err(format!("`{other}` is not `turn`, `id` or `top`")),
        }
        rest = tail;
    }
    if let [word] = rest {
        return Err(match *word {
            "turn" | "id" | "top" => format!("`{word}` takes a value"),
            other => format!("`{other}` is not `turn`, `id` or `top`"),
        });
    }
    Ok(Shape { line, shape: name.to_string(), mesh, kind, at, size, turn: turn.unwrap_or(0.0), id, top })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{AlphaMode, TextureSource, mat_point};

    const SAMPLE: &str = "\
# a corner of a yard
set yard-sample
camera room 768 448            # the Delve's room camera
plate 1536 1024

plane grass   at 384 0 224 size 768 0 448
box   stone   at 200 0 180  size 96 64 48            # a wall block
box   wood    at 200 64 180 size 104 8 56 turn 0
cone  foliage at 420 0 300  size 40 70 40 id tree-3
sphere rock   at 600 0 120 size 30 20 30 turn -30.5 id boulder
";

    fn refusals(text: &str) -> Vec<Refusal> {
        Set::parse(text).expect_err("refused")
    }

    /// The single refusal of a file of `head` then `line` (on line 3).
    fn refused(line: &str) -> Refusal {
        let text = format!("set s\ncamera room 384 224\n{line}\n");
        let all = refusals(&text);
        assert_eq!(all.len(), 1, "{line}: {all:?}");
        assert_eq!(all[0].line, 3, "{line}: {all:?}");
        all.into_iter().next().unwrap()
    }

    #[test]
    fn a_sample_parses() {
        let set = Set::parse(SAMPLE).unwrap();
        assert_eq!(set.id, "yard-sample");
        assert_eq!(set.camera, SetCamera::Room { size: (768, 448) });
        assert_eq!(set.plate, (1536, 1024));
        assert_eq!(set.shapes.len(), 5);
        let wall = &set.shapes[1];
        assert_eq!((wall.line, wall.shape.as_str(), wall.mesh, wall.kind), (7, "box", MeshRef::Cube, Kind::Stone));
        assert_eq!((wall.at, wall.size, wall.turn, wall.id.as_deref()), ([200.0, 0.0, 180.0], [96.0, 64.0, 48.0], 0.0, None));
        let tree = &set.shapes[3];
        assert_eq!((tree.mesh, tree.id.as_deref()), (MeshRef::Cone { segments: 12 }, Some("tree-3")));
        assert_eq!((set.shapes[4].turn, set.shapes[4].id.as_deref()), (-30.5, Some("boulder")));
        // A plate is the art lane's size unless a set says otherwise.
        let bare = Set::parse("set bare\ncamera chunk 768 448 centre 1000 2000.5\n").unwrap();
        assert_eq!(bare.plate, DEFAULT_PLATE);
        assert_eq!(bare.camera, SetCamera::Chunk { size: (768, 448), centre: [1000.0, 2000.5] });
        assert!(bare.shapes.is_empty());
    }

    #[test]
    fn the_docs_sample_parses_but_for_shapes_yet_to_land() {
        let doc = include_str!("../../../docs/WORLD-3D.md");
        let sample = doc.split("```\n").nth(1).expect("the doc's set sample");
        assert!(sample.starts_with("set "), "{sample}");
        match Set::parse(sample) {
            Ok(set) => assert!(!set.shapes.is_empty()),
            Err(all) => {
                for r in all {
                    let name = sample.lines().nth(r.line - 1).unwrap().split_whitespace().next().unwrap();
                    assert!(
                        doc.contains(&format!("`{name}`")) && shapes::by_name(name).is_none(),
                        "{r}: only a shape the doc names but shapes::by_name does not know yet may be refused"
                    );
                }
            }
        }
    }

    #[test]
    fn every_bad_line_is_refused_with_its_number() {
        let cases = [
            ("set again", "second `set`"),
            ("camera room 384 224", "second `camera` line (the first is line 2)"),
            ("plate 1536", "`plate` takes"),
            ("plate 0 1024", "plate width `0`"),
            ("plate 1536 9000", "plate height `9000`"),
            ("plate 15.5 10", "plate width"),
            ("castle stone at 0 0 0 size 1 1 1", "`castle` is not a statement or a shape"),
            ("Box stone at 0 0 0 size 1 1 1", "`Box` is not"),
            ("box", "takes a kind"),
            ("box marble at 0 0 0 size 1 1 1", "`marble` is not a kind"),
            ("box stone 0 0 0 size 1 1 1", "`at <x> <y> <z>`"),
            ("box stone at 0 0 size 1 1 1", "the at z `size` is not a number"),
            ("box stone at 0 0 0", "`size <x> <y> <z>`"),
            ("box stone at 0 0 0 size 1 1", "`size` takes three numbers"),
            ("box stone at 0 0 nan size 1 1 1", "at z `nan`"),
            ("box stone at 0 0 0 size 1 inf 1", "size y `inf`"),
            ("box stone at 0 0 0 size 1 -1 1", "less than 0"),
            ("box stone at 0 0 0 size 0 0 0", "no size"),
            ("box stone at 0 0 0 size 1 1 1 turn", "`turn` takes a value"),
            ("box stone at 0 0 0 size 1 1 1 turn left", "turn `left`"),
            ("box stone at 0 0 0 size 1 1 1 turn 5 turn 6", "second `turn`"),
            ("box stone at 0 0 0 size 1 1 1 id Tree", "`Tree` is not an id"),
            ("box stone at 0 0 0 size 1 1 1 id a id b", "second `id`"),
            ("box stone at 0 0 0 size 1 1 1 colour red", "`colour` is not `turn`, `id` or `top`"),
            ("box stone at 0 0 0 size 1 1 1 tall", "`tall` is not `turn`, `id` or `top`"),
        ];
        for (line, why) in cases {
            let r = refused(line);
            assert!(r.why.contains(why), "{line}: {r}");
            assert!(r.to_string().starts_with("line 3: "), "{r}");
        }
        // A camera line that is wrong, as the set's only camera.
        for (camera, why) in [
            ("camera", "`camera` names one"),
            ("camera eye 1 2", "`eye` is not a camera"),
            ("camera room 768", "`camera room` takes"),
            ("camera room 0 448", "stage width `0`"),
            ("camera room 768 448.5", "stage height"),
            ("camera chunk 768 448", "`camera chunk` takes"),
            ("camera chunk 768 448 middle 1 2", "`camera chunk` takes"),
            ("camera chunk 768 448 centre 1 east", "centre z `east`"),
        ] {
            let all = refusals(&format!("set s\n{camera}\n"));
            assert_eq!(all.len(), 2, "{camera}: {all:?}");
            assert!(all[0].line == 2 && all[0].why.contains(why), "{camera}: {}", all[0]);
            assert_eq!(all[1], Refusal { line: 0, why: "no `camera` line".into() });
        }
    }

    #[test]
    fn a_set_must_start_with_set_and_have_a_camera_and_unique_ids() {
        assert_eq!(refusals(""), [Refusal { line: 0, why: "no `set <id>` line: the file is empty".into() }, Refusal {
            line: 0,
            why: "no `camera` line".into()
        }]);
        assert_eq!(refusals("# nothing\n\n").len(), 2);
        let all = refusals("\ncamera room 10 10\nset late\n");
        assert_eq!((all.len(), all[0].line), (1, 2), "{all:?}");
        assert!(all[0].why.contains("starts with `set <id>`"));
        for bad in ["set", "set Yard", "set yard sample", "set yard_sample", &format!("set {}", "a".repeat(65))] {
            let all = refusals(&format!("{bad}\ncamera room 10 10\n"));
            assert_eq!(all.len(), 1, "{bad}: {all:?}");
            assert_eq!(all[0].line, 1);
        }
        let all = refusals("set s\nplate 10 10\n");
        assert_eq!(all, [Refusal { line: 0, why: "no `camera` line".into() }]);
        assert_eq!(all[0].to_string(), "the set: no `camera` line");
        let all = refusals("set s\ncamera room 10 10\nbox stone at 0 0 0 size 1 1 1 id a\nbox stone at 0 0 0 size 1 1 1 id a\n");
        assert_eq!((all.len(), all[0].line), (1, 4));
        assert!(all[0].why.contains("line 3's already"), "{}", all[0]);
        // Every bad line is told, not just the first.
        let all = refusals("set s\ncamera room 10 10\nbox x at 0 0 0 size 1 1 1\nplate 1 1 1\ncone y at 0 0 0 size 1 1 1\n");
        assert_eq!(all.iter().map(|r| r.line).collect::<Vec<_>>(), [3, 4, 5]);
    }

    #[test]
    fn kinds_are_the_docs_table_with_unique_colours() {
        let doc = include_str!("../../../docs/WORLD-3D.md");
        let table = doc.split("## Surface kinds").nth(1).unwrap().split("\n## ").next().unwrap();
        let names: Vec<&str> = table
            .lines()
            .filter_map(|l| l.strip_prefix("| "))
            .filter_map(|l| l.split(" |").next())
            .filter(|n| *n != "kind" && !n.starts_with('-'))
            .collect();
        assert_eq!(names, Kind::ALL.map(Kind::name), "the doc's table, in its order");
        // And the colours it gives the painter are these.
        let hex = |rgb: [u8; 3]| format!("`#{:02x}{:02x}{:02x}`", rgb[0], rgb[1], rgb[2]);
        for kind in Kind::ALL {
            let row = format!("| {} |", kind.name());
            let line = table.lines().find(|l| l.starts_with(&row)).unwrap();
            assert!(line.ends_with(&format!("| {} | {} |", hex(kind.capture()), hex(kind.id()))), "{line}");
        }
        for (i, kind) in Kind::ALL.into_iter().enumerate() {
            assert_eq!(kind as usize, i);
            assert_eq!(Kind::from_name(kind.name()), Some(kind));
            assert_eq!(Kind::from_id(kind.id()), Some(kind));
            let c = kind.capture();
            // Not black, not white, not glaring: a muted colour that reads.
            let (lo, hi) = (*c.iter().min().unwrap(), *c.iter().max().unwrap());
            assert!(lo > 0x20 && hi < 0xf8 && hi > 0x48, "{kind}: {c:?}");
            assert!(f32::from(hi - lo) / f32::from(hi) < 0.85, "{kind} is not muted: {c:?}");
            assert_ne!(kind.id(), [0, 0, 0], "black is the id map's nothing");
            for other in &Kind::ALL[i + 1..] {
                assert_ne!(kind.id(), other.id(), "{kind} and {other}");
                let o = other.capture();
                let d = (0..3).map(|k| (f32::from(c[k]) - f32::from(o[k])).powi(2)).sum::<f32>().sqrt();
                assert!(d > 20.0, "{kind} and {other} are too alike: {d}");
            }
        }
        assert_eq!(Kind::from_name("Stone"), None);
        assert_eq!(Kind::from_id([1, 2, 3]), None);
        assert!(Kind::Fire.shines() && Kind::Window.shines() && !Kind::Stone.shines());
    }

    fn shape(mesh: &str, at: [f32; 3], size: [f32; 3], turn: f32) -> Shape {
        let text = format!(
            "{mesh} stone at {} {} {} size {} {} {} turn {turn}",
            at[0], at[1], at[2], size[0], size[1], size[2]
        );
        let set = Set::parse(&format!("set s\ncamera room 10 10\n{text}\n")).unwrap();
        set.shapes[0].clone()
    }

    fn near3(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3)
    }

    /// The box the shape's mesh fills once placed.
    fn placed_box(s: &Shape) -> ([f32; 3], [f32; 3]) {
        let m = s.transform().matrix();
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in s.mesh.generate().unwrap().positions {
            let q = mat_point(&m, p);
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn a_shape_stands_on_its_foot_at_its_size() {
        // A wall block: its footprint centred on `at`, its bottom at `at`'s height.
        let wall = shape("box", [200.0, 0.0, 180.0], [96.0, 64.0, 48.0], 0.0);
        assert_eq!(placed_box(&wall), ([152.0, 0.0, 156.0], [248.0, 64.0, 204.0]));
        // On a wall: the roof's bottom is the wall's top.
        let roof = shape("box", [200.0, 64.0, 180.0], [104.0, 40.0, 56.0], 0.0);
        let (lo, hi) = placed_box(&roof);
        assert!(near3(lo, [148.0, 64.0, 152.0]) && near3(hi, [252.0, 104.0, 208.0]), "{lo:?} {hi:?}");
        // A cone's base on the ground and its point `size` up; a plane flat at its height.
        let tree = shape("cone", [420.0, 10.0, 300.0], [40.0, 70.0, 40.0], 0.0);
        let (lo, hi) = placed_box(&tree);
        assert!((lo[1] - 10.0).abs() < 1e-3 && (hi[1] - 80.0).abs() < 1e-3, "{lo:?} {hi:?}");
        let tip = mat_point(&tree.transform().matrix(), [0.0, 0.5, 0.0]);
        assert!(near3(tip, [420.0, 80.0, 300.0]), "{tip:?}");
        let ground = shape("plane", [384.0, 0.0, 224.0], [768.0, 0.0, 448.0], 0.0);
        assert_eq!(placed_box(&ground), ([0.0, 0.0, 0.0], [768.0, 0.0, 448.0]));
        // An upright card: its bottom edge on the floor, facing down the floor.
        let card = shape("quad", [50.0, 0.0, 60.0], [20.0, 30.0, 0.0], 0.0);
        assert_eq!(placed_box(&card), ([40.0, 0.0, 60.0], [60.0, 30.0, 60.0]));
    }

    #[test]
    fn a_turn_is_counter_clockwise_seen_from_above_about_the_foot() {
        // A quarter turn: the footprint's x and z extents swap about the foot.
        let wall = shape("box", [200.0, 0.0, 180.0], [96.0, 64.0, 48.0], 90.0);
        let (lo, hi) = placed_box(&wall);
        assert!(near3(lo, [176.0, 0.0, 132.0]) && near3(hi, [224.0, 64.0, 228.0]), "{lo:?} {hi:?}");
        // Seen from above (x right, z down the page), counter-clockwise takes
        // the shape's right (+x) to the top of the page (-z, up the floor).
        let m = wall.transform().matrix();
        let right_end = mat_point(&m, [0.5, -0.5, 0.0]);
        assert!(near3(right_end, [200.0, 0.0, 132.0]), "{right_end:?}");
        // Off the vertical, a turn moves nothing: the foot stays put whatever the angle.
        for turn in [0.0, 33.0, -120.0, 270.0] {
            let s = shape("box", [10.0, 5.0, -7.0], [4.0, 6.0, 2.0], turn);
            let foot = mat_point(&s.transform().matrix(), [0.0, -0.5, 0.0]);
            assert!(near3(foot, [10.0, 5.0, -7.0]), "{turn}: {foot:?}");
            assert!((placed_box(&s).1[1] - 11.0).abs() < 1e-3);
        }
    }

    /// Where the capture's camera puts world point `p` in the plate, pixels.
    fn in_plate(set: &Set, p: [f32; 3]) -> [f32; 2] {
        let m = set.capture_camera().view_projection(1.0).unwrap();
        let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
        let (x, y, w) = (row(0) / row(3), row(1) / row(3), set.plate);
        [(x + 1.0) * 0.5 * w.0 as f32, (1.0 - y) * 0.5 * w.1 as f32]
    }

    #[test]
    fn the_room_camera_is_the_delves_framed_in_the_plate() {
        let set = Set::parse("set r\ncamera room 768 448\nplate 1536 1024\n").unwrap();
        assert_eq!(set.camera.game(), stage_camera(0.3, (768, 448)));
        // The stage is width-bound in a 3:2 plate: 92% of 1536 across, square pixels.
        let [sx, sy] = set.framing();
        let scale = 1536.0 * 0.92 / 768.0;
        assert!((sx - 0.92).abs() < 1e-6 && (sy - scale * 448.0 / 1024.0).abs() < 1e-6, "{sx} {sy}");
        // Each point lands where the stage's own floor maths put it on the
        // stage, scaled about the middles: the stage's pixel `q` is the
        // plate's `plate / 2 + scale * (q - stage / 2)`.
        let tilt = crate::Tilt::for_stage(0.3, (768, 448)).unwrap();
        for (x, h, z) in [(0.0, 0.0, 448.0), (768.0, 0.0, 448.0), (384.0, 0.0, 0.0), (100.0, 40.0, 200.0), (700.0, 90.0, 30.0)] {
            let [fx, fy] = tilt.floor([x, z]);
            let q = [fx, fy - h * tilt.scale_at([x, z])];
            let want = [768.0 + scale * (q[0] - 384.0), 512.0 + scale * (q[1] - 224.0)];
            let got = in_plate(&set, [x, h, z]);
            assert!((got[0] - want[0]).abs() < 0.05 && (got[1] - want[1]).abs() < 0.05, "{x} {h} {z}: {got:?} for {want:?}");
            // The floor's near corners spill past the stage's sides, but not past the plate's.
            assert!(got[0] >= 0.0 && got[0] <= 1536.0 && got[1] >= 0.0 && got[1] <= 1024.0, "{got:?}");
        }
        // The projector is the capture's camera.
        assert_eq!(Projection::Custom(set.projector().view_projection), set.capture_camera().projection);
        // A tall stage in a wide plate is height-bound.
        let tall = Set::parse("set t\ncamera room 200 400\n").unwrap();
        let [sx, sy] = tall.framing();
        assert!((sy - 0.92).abs() < 1e-6 && (sx - 1024.0 * 0.92 / 400.0 * 200.0 / 1536.0).abs() < 1e-6);
    }

    #[test]
    fn a_chunk_camera_is_the_room_camera_over_its_centre() {
        let room = Set::parse("set r\ncamera room 768 448\n").unwrap();
        let chunk = Set::parse("set c\ncamera chunk 768 448 centre 5000 -1200\n").unwrap();
        // What the room camera has at the stage's middle, the chunk has at its centre.
        for (dx, h, dz) in [(0.0, 0.0, 0.0), (37.0, 12.0, -50.0), (-300.0, 0.0, 180.0)] {
            let r = in_plate(&room, [384.0 + dx, h, 224.0 + dz]);
            let c = in_plate(&chunk, [5000.0 + dx, h, -1200.0 + dz]);
            assert!((r[0] - c[0]).abs() < 0.05 && (r[1] - c[1]).abs() < 0.05, "{r:?} {c:?}");
        }
    }

    fn chunk(id: &str, cx: f32, cz: f32) -> Set {
        Set::parse(&format!("set {id}\ncamera chunk 768 512 centre {cx} {cz}\n")).unwrap()
    }

    #[test]
    fn chunks_side_by_side_share_a_border() {
        let me = chunk("me", 1000.0, 2000.0);
        assert_eq!(me.camera.stage(), [616.0, 1744.0, 1384.0, 2256.0]);
        assert_eq!(Set::parse("set r\ncamera room 768 448\n").unwrap().camera.stage(), [0.0, 0.0, 768.0, 448.0]);
        let east = me.border_with(&chunk("e", 1768.0, 2000.0)).unwrap();
        assert_eq!(east, Border { from: [1384.0, 1744.0], to: [1384.0, 2256.0], across: [1.0, 0.0] });
        assert_eq!((east.side(), east.length(), east.at(0.25)), ("east", 512.0, [1384.0, 1872.0]));
        let west = me.border_with(&chunk("w", 232.0, 2000.0)).unwrap();
        assert_eq!((west.side(), west.from, west.to), ("west", [616.0, 1744.0], [616.0, 2256.0]));
        let south = me.border_with(&chunk("s", 1000.0, 2512.0)).unwrap();
        assert_eq!((south.side(), south.from, south.to), ("south", [616.0, 2256.0], [1384.0, 2256.0]));
        let north = me.border_with(&chunk("n", 1000.0, 1488.0)).unwrap();
        assert_eq!((north.side(), north.across), ("north", [0.0, -1.0]));
        // A neighbour offset along the edge shares the stretch both cover.
        let slid = me.border_with(&chunk("e", 1768.0, 2200.0)).unwrap();
        assert_eq!((slid.from, slid.to), ([1384.0, 1944.0], [1384.0, 2256.0]));
        // The border is the same edge seen from the other side.
        let back = chunk("e", 1768.0, 2000.0).border_with(&me).unwrap();
        assert_eq!((back.side(), back.from, back.to), ("west", [1384.0, 1744.0], [1384.0, 2256.0]));
        // A corner, a gap, an overlap and itself share no edge.
        assert_eq!(me.border_with(&chunk("c", 1768.0, 2512.0)), None);
        assert_eq!(me.border_with(&chunk("g", 1900.0, 2000.0)), None);
        assert_eq!(me.border_with(&chunk("o", 1500.0, 2000.0)), None);
        assert_eq!(me.border_with(&me), None);
    }

    #[test]
    fn plate_pixels_are_where_the_capture_puts_a_point() {
        let set = Set::parse("set r\ncamera room 768 448\nplate 1536 1024\n").unwrap();
        for p in [[0.0, 0.0, 448.0], [384.0, 0.0, 0.0], [100.0, 40.0, 200.0], [700.0, 90.0, 30.0]] {
            let (want, got) = (in_plate(&set, p), set.plate_pixel(p).unwrap());
            assert!((want[0] - got[0]).abs() < 1e-3 && (want[1] - got[1]).abs() < 1e-3, "{p:?}: {got:?} for {want:?}");
        }
        // A chunk's centre is the plate's middle, wherever the chunk lies.
        let far = chunk("far", 5000.0, -1200.0);
        let middle = far.plate_pixel([5000.0, 0.0, -1200.0]).unwrap();
        let room = Set::parse("set r\ncamera room 768 512\n").unwrap().plate_pixel([384.0, 0.0, 256.0]).unwrap();
        assert!((middle[0] - room[0]).abs() < 0.05 && (middle[1] - room[1]).abs() < 0.05, "{middle:?} {room:?}");
    }

    #[test]
    fn what_lies_on_the_floor_is_told_from_what_stands() {
        let set = Set::parse(SAMPLE).unwrap();
        let lying: Vec<bool> = set.shapes.iter().map(Shape::on_floor).collect();
        assert_eq!(lying, [true, false, false, false, false]);
        let path = Set::parse("set p\ncamera room 10 10\nplane path at 5 0.5 5 size 4 0 2\nbox stone at 5 0.5 5 size 4 1 2\n").unwrap();
        assert!(path.shapes[0].on_floor(), "a path a hair over the ground");
        assert!(!path.shapes[1].on_floor(), "a step a pixel and a half high stands");
    }

    #[test]
    fn placed_nodes_move_the_blocks_and_their_plate_with_them() {
        let set = Set::parse(SAMPLE).unwrap();
        let (scale, offset) = (0.5, [-3.25, 0.0, 7.5]);
        let mode = Mode::Projected { picture: 9 };
        let plain = set.nodes(mode);
        let placed = set.placed_nodes(mode, scale, offset);
        assert_eq!(placed.len(), plain.len());
        for (a, b) in plain.iter().zip(&placed) {
            assert_eq!((a.id, a.mesh, a.material.source), (b.id, b.mesh, b.material.source));
            // Every corner of the mesh lands scaled and moved...
            let (ma, mb) = (a.transform.matrix(), b.transform.matrix());
            for p in a.mesh.unwrap().generate().unwrap().positions {
                let (pa, pb) = (mat_point(&ma, p), mat_point(&mb, p));
                let want = [pa[0] * scale + offset[0], pa[1] * scale + offset[1], pa[2] * scale + offset[2]];
                assert!(near3(pb, want), "{pb:?} for {want:?}");
                // ...and is cast the texel it was before it moved.
                let (ua, ub) = (a.material.projector.unwrap().uv(pa), b.material.projector.unwrap().uv(pb));
                match (ua, ub) {
                    (Some(ua), Some(ub)) => assert!((ua[0] - ub[0]).abs() < 1e-5 && (ua[1] - ub[1]).abs() < 1e-5),
                    (ua, ub) => assert_eq!(ua.is_some(), ub.is_some()),
                }
            }
        }
        // Unmoved, it is the nodes as they are.
        assert_eq!(set.placed_nodes(mode, 1.0, [0.0; 3]), plain);
        // A patch moves with its block: a point placed is the patch pixel it was.
        let patches = Patches::of(400, [(Kind::Stone, 40, 40), (Kind::Grass, 64, 64)]);
        let (floor, blocks) = set.world_nodes(9, patches);
        let (placed_floor, placed_blocks) = set.placed_world(9, patches, scale, offset);
        for (a, b) in floor.iter().chain(&blocks).zip(placed_floor.iter().chain(&placed_blocks)) {
            let (pa, pb) = (a.material.patch.unwrap(), b.material.patch.unwrap());
            assert_eq!(pa.picture, pb.picture);
            let p = [12.0, 3.0, -40.0];
            let q = [0, 1, 2].map(|k| p[k] * scale + offset[k]);
            for k in 0..3 {
                let (before, after) = ((p[k] - pa.origin[k]) / pa.texel, (q[k] - pb.origin[k]) / pb.texel);
                assert!((before - after).abs() < 1e-3, "{before} {after}");
            }
        }
        // A capture has no projector to move.
        assert!(set.placed_nodes(Mode::Capture, 2.0, [1.0, 0.0, 1.0]).iter().all(|n| n.material.projector.is_none()));
    }

    #[test]
    fn a_far_side_makes_the_world_again_from_its_set_once_sent() {
        use crate::scene::SetRef;
        let set = Set::parse(SAMPLE).unwrap();
        let (scale, offset) = (0.5, [-2.0, 0.0, 3.5]);
        let (floor, blocks) = set.placed_world(5, Patches::NONE, scale, offset);
        assert_eq!((floor.len(), blocks.len()), (1, 4));
        let reference = |text: Option<&str>| SetRef { key: 77, text: text.map(Into::into), plate: 5, png: None, scale, offset, patches: Patches::NONE };
        let world = |set| World { set: Some(set), ..World::default() };
        let mut sets = Sets::default();
        assert_eq!(set.world_nodes(5, Patches::NONE).0.len(), 1);
        // Named before it was sent: nothing to make it from.
        let mut w = world(reference(None));
        assert!(!sets.fill(&mut w) && w.floor.is_empty() && w.blocks.is_empty());
        // Sent once, then by its key alone.
        let mut w = world(reference(Some(SAMPLE)));
        assert!(sets.fill(&mut w));
        assert_eq!((&w.floor, &w.blocks), (&floor, &blocks));
        let mut w = world(reference(None));
        assert!(sets.fill(&mut w));
        assert_eq!((&w.floor, &w.blocks), (&floor, &blocks));
        // Its patches come with the reference: the blocks are made again with them.
        let patches = Patches::of(900, [(Kind::Stone, 30, 20)]);
        let mut w = world(SetRef { patches, ..reference(None) });
        assert!(sets.fill(&mut w));
        assert_eq!((&w.floor, &w.blocks), (&set.placed_world(5, patches, scale, offset).0, &set.placed_world(5, patches, scale, offset).1));
        let stone = [swatch_cell(Kind::Stone).0, swatch_cell(Kind::Stone).1, 30, 20];
        assert!(w.blocks.iter().all(|n| n.material.patch.is_some_and(|p| p.picture == 900 && p.rect == stone)), "every block's patch the stone's");
        // A refused text makes nothing.
        let mut w = world(SetRef { key: 78, text: Some("set broken\n".into()), ..reference(None) });
        assert!(!sets.fill(&mut w) && w.blocks.is_empty());
        // Only the newest few are kept: the oldest goes first.
        for key in 100..100 + KEPT_SETS as u64 {
            assert!(sets.fill(&mut world(SetRef { key, text: Some(SAMPLE.into()), ..reference(None) })));
        }
        assert!(!sets.fill(&mut world(reference(None))), "77 has gone");
        // A world with no set is left alone.
        let mut bare = World { figures_at: 3, ..World::default() };
        assert!(sets.fill(&mut bare));
        assert_eq!(bare, World { figures_at: 3, ..World::default() });
        // Several sets: each one's floor and blocks, every node its own id.
        let mut sets = Sets::default();
        let text = "set two\ncamera room 64 64\nbox stone at 8 0 8 size 8 8 8\nplane grass at 32 0 32 size 64 0 64\n";
        let one = SetRef { key: 21, text: Some(text.into()), plate: 5, png: None, scale: 1.0, offset: [0.0; 3], patches: Patches::NONE };
        let two = SetRef { key: 22, text: Some(text.replace("two", "three").into()), plate: 6, png: None, scale: 1.0, offset: [64.0, 0.0, 0.0], patches: Patches::NONE };
        let mut both = World { set: Some(one), more: vec![two], ..World::default() };
        assert!(sets.fill(&mut both));
        assert_eq!((both.floor.len(), both.blocks.len()), (2, 2));
        let mut ids: Vec<u64> = both.floor.iter().chain(&both.blocks).map(|n| n.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4, "no two nodes share an id");
        let mut missing = World { set: Some(SetRef { key: 99, text: None, plate: 1, png: None, scale: 1.0, offset: [0.0; 3], patches: Patches::NONE }), more: both.more.clone(), ..World::default() };
        assert!(!sets.fill(&mut missing), "a set not kept: all again");
        assert!(missing.floor.is_empty() && missing.blocks.is_empty());
    }

    #[test]
    fn a_set_makes_its_three_scenes() {
        let set = Set::parse(SAMPLE).unwrap();
        let capture = set.scene(Mode::Capture);
        assert_eq!(capture.nodes.len(), 5);
        assert_eq!(capture.camera, set.capture_camera());
        assert_eq!(capture.camera.clear, Some(BACKGROUND));
        let wall = &capture.nodes[1];
        assert_eq!((wall.id, wall.mesh, wall.transform), (2, Some(MeshRef::Cube), set.shapes[1].transform()));
        assert!(wall.material.lit && wall.material.emissive == 0.0 && wall.material.projector.is_none());
        let c = Kind::Stone.capture();
        assert_eq!(wall.material.tint, [f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0, 1.0]);
        let ids = set.scene(Mode::Ids);
        let i = Kind::Stone.id();
        assert!(!ids.nodes[1].material.lit);
        assert_eq!(ids.nodes[1].material.tint, [f32::from(i[0]) / 255.0, f32::from(i[1]) / 255.0, f32::from(i[2]) / 255.0, 1.0]);
        let cast = set.scene(Mode::Projected { picture: 42 });
        let m = cast.nodes[1].material;
        assert_eq!((m.source, m.projector, m.lit, m.alpha), (TextureSource::Picture(42), Some(set.projector()), false, AlphaMode::Opaque));
        assert_eq!(m.tint, unit_rgba(Kind::Stone.dark()), "a surface the plate misses is its kind's colour two shades down");
        assert_eq!(m.patch, None, "with no patches");
        // The ids run from 1 in the file's order; the game draws the same nodes with its own camera.
        assert_eq!(set.nodes(Mode::Ids).iter().map(|n| n.id).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        let fire = Set::parse("set f\ncamera room 10 10\nsphere fire at 0 0 0 size 1 1 1\n").unwrap();
        assert_eq!(fire.nodes(Mode::Capture)[0].material.emissive, 1.0);
    }

    #[test]
    fn a_surface_the_plate_misses_takes_its_kinds_patch_or_the_nearest() {
        // Kind k's patch is its size at its cell's corner of the atlas; a kind
        // with none takes the nearest kind's by capture colour, as an item's
        // extra blocks do.
        let patches = Patches::of(1000, [(Kind::Grass, 64, 64), (Kind::Stone, 32, 48), (Kind::Foliage, 12, 12), (Kind::Rug, 0, 9)]);
        assert_eq!(patches.rect(Kind::Grass), Some([64, 0, 64, 64]));
        assert_eq!(patches.rect(Kind::Stone), Some([0, 64, 32, 48]));
        assert_eq!(patches.drawn_as(Kind::Hedge), Some(Kind::Foliage));
        assert_eq!(patches.drawn_as(Kind::Plaster), Some(Kind::Stone));
        assert_ne!(patches.drawn_as(Kind::Rug), Some(Kind::Rug), "an empty patch is none");
        assert_eq!(Patches::NONE.rect(Kind::Grass), None);
        assert!(Patches::NONE.is_empty() && !patches.is_empty());
        assert_eq!(Patches::of(7, [(Kind::Fire, 200, 3)]).sizes[Kind::Fire.index()], [64, 3], "at most a cell");
        // The world's blocks carry them, at the plate's own density.
        let set = Set::parse(SAMPLE).unwrap();
        let (floor, blocks) = set.world_nodes(42, patches);
        for (node, shape) in floor.iter().chain(&blocks).zip(&set.shapes) {
            let patch = node.material.patch.expect("every kind has one");
            assert_eq!((patch.picture, Some(patch.rect)), (1000, patches.rect(shape.kind)));
            assert_eq!((patch.origin, patch.texel), ([0.0; 3], 1.0 / set.density()));
            assert_eq!(node.material.tint, unit_rgba(shape.kind.dark()));
        }
        // A room of 768 x 448 in a 1536 x 1024 plate: 1.84 plate pixels a stage pixel.
        assert!((set.density() - 1.84).abs() < 1e-5, "{}", set.density());
        // Every kind's dark colour is darker than its capture colour.
        for kind in Kind::ALL {
            let (dark, capture) = (kind.dark(), kind.capture());
            let sum = |c: [u8; 3]| c.iter().map(|&v| u32::from(v)).sum::<u32>();
            assert!(sum(dark) < sum(capture) && sum(dark) > 0, "{kind}: {dark:?} of {capture:?}");
        }
    }

    #[test]
    fn a_ground_and_lamps_are_read_and_drawn_lit_with_their_mesh() {
        let text = "set g\ncamera room 32 32\nground 2 1 step 16 at 0 0 heights 0 4 8 2 6 10 tones 20 40 60 30 50 100\n\
                    lamp at 8 6 8 reach 20 strength 0.8\nbox stone at 16 0 8 size 8 20 8\nplane water at 16 0 24 size 32 0 16\nbox window at 4 10 4 size 1 2 1\n";
        let set = Set::parse(text).expect("reads");
        let ground = set.ground.as_ref().expect("a ground");
        assert_eq!((ground.cols, ground.rows, ground.step), (2, 1, 16.0));
        assert_eq!(ground.height(2, 1), 10.0);
        assert_eq!(ground.height_at(8.0, 0.0), 2.0, "halfway along the first edge");
        assert_eq!(set.lamps, vec![SetLamp { at: [8.0, 6.0, 8.0], reach: 20.0, strength: 0.8 }]);
        let (floor, blocks, mesh) = set.lit_world_nodes(77);
        let mesh = mesh.expect("the ground's mesh");
        assert_eq!(mesh.validate(), Ok(()));
        assert_eq!(mesh.positions.len(), 6);
        // The brand guide holds a ground's tones in its bands: 100 is the
        // brightest a route may be, never white (docs/BRAND.md).
        let top = brand::Class::Route.band().tone.1;
        assert_eq!(mesh.colors[5], [top, top, top, 1.0], "tone 100 is held at a route's top");
        assert_eq!(mesh.colors[0], [0.2, 0.2, 0.2, 1.0], "tone 20 is as written");
        assert_eq!(floor[0].mesh, Some(MeshRef::Uploaded(77)), "the ground first");
        assert_eq!(floor[0].material.ceiling, brand::Class::Ground.material_ceiling(), "the ground never reaches the top step");
        assert_eq!(floor[1].material.role, crate::scene::Role::Water, "water runs");
        let stone = &blocks[0].material;
        assert!(stone.lit && (stone.tint[0] - Kind::Stone.tone()).abs() < 1e-6);
        assert_eq!(stone.ceiling, brand::Class::Mass.material_ceiling(), "stone is a mass");
        assert!(blocks[1].material.accent, "a lit window is an accent");
        // Refusals: the wrong count of heights, a tone past 100, a bad lamp.
        for bad in [
            "set g\ncamera room 8 8\nground 1 1 step 4 at 0 0 heights 0 0 0 tones 0 0 0 0\n",
            "set g\ncamera room 8 8\nground 1 1 step 4 at 0 0 heights 0 0 0 0 tones 0 0 0 101\n",
            "set g\ncamera room 8 8\nlamp at 0 0 0 reach 0 strength 1\n",
        ] {
            assert!(Set::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_world_with_a_look_is_filled_lit_with_its_grounds_mesh() {
        let text: std::sync::Arc<str> = "set g\ncamera room 32 32\nground 1 1 step 32 at 0 0 heights 0 0 0 0 tones 50 50 50 50\nbox rock at 16 0 16 size 8 8 8\n".into();
        let mut sets = Sets::default();
        let mut world = World {
            set: Some(SetRef { key: 42, text: Some(text), plate: 0, png: None, scale: 1.0, offset: [0.0; 3], patches: Patches::NONE }),
            look: Some(crate::look::Look::new(vec![[0, 0, 0], [255, 255, 255]])),
            ..World::default()
        };
        assert!(sets.fill(&mut world));
        assert_eq!(world.meshes.len(), 1);
        assert_eq!(world.meshes[0].0, ground_mesh_id(42));
        assert_eq!(world.floor[0].mesh, Some(MeshRef::Uploaded(ground_mesh_id(42))));
        assert!(world.blocks[0].material.lit && world.blocks[0].material.projector.is_none(), "toned, not cast");
    }

    #[test]
    fn a_set_tones_and_roles_its_own_kinds_and_a_box_its_top() {
        use crate::scene::Role;
        let text = "set t\ncamera room 32 32\ntone water 30\nrole fire ember\ntone fire 60\n\
                    plane water at 16 0 16 size 32 0 32\nbox fire at 8 0 8 size 4 2 4\nbox rock at 24 0 24 size 6 6 6 top 8\n";
        let set = Set::parse(text).expect("reads");
        let (floor, blocks, _) = set.lit_world_nodes(1);
        let water = &floor[0].material;
        assert_eq!(water.role, Role::Water);
        assert!((water.tint[0] - 0.30).abs() < 1e-6, "its own tone");
        let fire = &blocks[0].material;
        assert_eq!(fire.role, Role::Ember);
        assert!(!fire.accent, "an ember glows on the warm ramp, not as an accent");
        assert!((fire.tint[0] - 0.60).abs() < 1e-6);
        let rock = &blocks[1];
        assert_eq!(rock.face_materials.len(), 6);
        assert!((rock.face_materials[2].tint[0] - 0.08).abs() < 1e-6, "the top its own");
        assert!((rock.face_materials[0].tint[0] - Kind::Rock.tone()).abs() < 1e-6);
        for bad in ["tone stone 120", "role water lava", "tone nothing 10", "box rock at 0 0 0 size 1 1 1 top"] {
            assert!(Set::parse(&format!("set t\ncamera room 8 8\n{bad}\n")).is_err(), "{bad}");
        }
    }
}
