//! A room of the Delve as a blockout set (docs/WORLD-3D.md, "Set files"):
//! the geometry half of projection painting. The room's tiles are the
//! truth, so gameplay and picture agree:
//!
//! - every wall tile is under a wall;
//! - no tile a knight walks on is under a block taller than a boot
//!   ([`LOW`]), save a lintel or an arch high over it ([`HEADROOM`]);
//! - no tile a knight walks on is more than half hidden from the game's
//!   camera ([`HIDE_ALL`]);
//! - every blocked tile is under, or hidden by, the thing that blocks it
//!   (a round crown leaves its corners to the grass, as the painter's tree
//!   does; a staked site is roped-off ground);
//! - every hazard lies below the floor line, and every door is a gap in its
//!   wall where the game's door tiles are.
//!
//! What a blocked tile *is* (a stall's counter, an apple tree) comes from
//! the room's [`Dressing`], read off the painters that draw the room today;
//! a blocked tile no dressing names gets its setting's own block (a tomb in
//! the Crypt, a boulder in the Mines).
//!
//! Units are stage pixels, as the Delve draws a room, 16 to a tile: `x`
//! across to the right, `z` down the floor toward the viewer, `y` up. The
//! floor line is `y` 0; the ground is a slab [`SLAB`] thick under it.
//!
//! The camera is the room's own ([`View`]): the Delve's stage camera at its
//! tilt (`stage_frame::TILT`) for a stage the room's size, the camera a set
//! names with `camera room <w> <h>`. Whatever stands up hides the floor
//! behind it, a little more than a pixel of floor for each pixel of height.
//! A wall or a thing in front of open floor is therefore sized for the
//! camera, a part at a time (a tile of wall, a tree, a prop): when it would
//! hide more than [`HIDE_ONE`] of a walkable tile, or with what stands
//! already more than [`HIDE_ALL`], its heights are scaled down together
//! until it does not ([`trim`]). The far wall hides nothing a knight walks
//! on and stands at full height, as do the side walls but where they stand
//! before a side door: there they are one low cutaway, then a step up
//! (`cutaway`). The near wall is a low parapet. Walls stand from the
//! slab's bottom, so no edge of the ground shows under them.
//!
//! Only built-in shapes (`angel_stage::shapes`: `box`, `sphere`, `gem`,
//! `cone`) are generated. Where a primitive of the coming library would read
//! better, the line says so with a `# TODO prims: <name>` comment.
//!
//! What the generator cannot know, a place's signature structure (a
//! windmill's tower and sails, an aqueduct's arches), is placed by hand in
//! `assets/world3d/<item>/landmark.set` ([`landmark`]): shape lines of the
//! set format, any shape of the library, which [`room_set_with`] appends to
//! the generated set. It is held to the same agreement with the game
//! ([`landmark_faults`]): it stands on tiles a knight cannot walk, and it
//! keeps the floor in view.

use std::collections::HashMap;
use std::fmt::Write as _;

use angel_stage::Tilt;

use crate::render::overworld::arena::stage_frame::TILT;
use crate::render::overworld::ink::{hash, vnoise};
use crate::rules::{Pack, Room, Tile};

/// Stage pixels to a tile.
pub const TILE: f32 = 16.0;
/// The plate the art lane paints, and the capture it paints over.
pub const PLATE: (u32, u32) = (1536, 1024);
/// The ground slab's thickness under the floor line.
pub const SLAB: f32 = 4.0;
/// The tallest thing that may lie on ground a knight walks: a boot.
pub const LOW: f32 = 4.0;
/// A lintel or an arch may span ground a knight walks above this: clear of
/// a knight's head and plume.
pub const HEADROOM: f32 = 30.0;
/// The most of one walkable tile one wall or prop may hide from the camera.
pub const HIDE_ONE: f32 = 0.45;
/// The most of one walkable tile everything together may hide.
pub const HIDE_ALL: f32 = 0.5;

/// The far wall's and the side walls' height: well over a knight, and
/// never out of the plate.
const WALL: f32 = 48.0;
/// A doorway's height, under its lintel.
const DOOR: f32 = 32.0;
/// The near wall: a parapet on the room's outer edge and a step inside it,
/// both trimmed for the camera.
const PARAPET: f32 = 16.0;
const STEP: f32 = 8.0;
/// How far water, lava and spores lie under the floor line.
const WATER_DROP: f32 = 3.0;
const LAVA_DROP: f32 = 2.0;

/// A rectangle of tiles: column, row, width, height.
pub type Rect = (i32, i32, i32, i32);

/// A rectangle of tiles in stage pixels: x0, z0, x1, z1.
pub fn px((c, r, w, h): Rect) -> [f32; 4] {
    [c as f32 * TILE, r as f32 * TILE, (c + w) as f32 * TILE, (r + h) as f32 * TILE]
}

/// A tile a knight can stand on.
pub fn walkable(tile: Tile) -> bool {
    matches!(tile, Tile::Floor | Tile::Door | Tile::Stairs | Tile::Bog | Tile::Ledge)
}

// ------------------------------------------------------------ the words ----

/// The surface kinds of docs/WORLD-3D.md, "Surface kinds": the set
/// format's own.
pub use angel_stage::set::Kind;

/// The built-in shapes a set may name (`angel_stage::shapes::NAMES`): the
/// first four, and the primitive library (`angel_stage::primitives`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Box,
    Sphere,
    Gem,
    Cone,
    /// A round column, a tree's trunk, a tower.
    Cylinder,
    /// A cylinder narrowing to 0.6 across at the top.
    Frustum,
    /// A ramp, full height at the back (-z), nothing at the front.
    Wedge,
    /// A pitched roof, its ridge along x.
    Gable,
    /// A hipped roof, its ridge along x.
    Hip,
    /// A square spire.
    Pyramid,
    /// A wall block with a round-topped opening through it along z.
    Arch,
    /// Steps rising toward the back (-z).
    Stairs,
    /// A boulder, lumped by its seed.
    Rock(u16),
    /// A clump of foliage, puffed by its seed.
    Blob(u16),
    /// A fence of this many posts along x, two rails between them.
    Fence(u16),
}

impl Shape {
    pub const ALL: [Shape; 15] = [
        Shape::Box,
        Shape::Sphere,
        Shape::Gem,
        Shape::Cone,
        Shape::Cylinder,
        Shape::Frustum,
        Shape::Wedge,
        Shape::Gable,
        Shape::Hip,
        Shape::Pyramid,
        Shape::Arch,
        Shape::Stairs,
        Shape::Rock(1),
        Shape::Blob(1),
        Shape::Fence(4),
    ];

    /// Its name in `angel_stage::shapes`.
    pub fn name(self) -> &'static str {
        match self {
            Shape::Box => "box",
            Shape::Sphere => "sphere",
            Shape::Gem => "gem",
            Shape::Cone => "cone",
            Shape::Cylinder => "cylinder",
            Shape::Frustum => "frustum",
            Shape::Wedge => "wedge",
            Shape::Gable => "gable",
            Shape::Hip => "hip",
            Shape::Pyramid => "pyramid",
            Shape::Arch => "arch",
            Shape::Stairs => "stairs",
            Shape::Rock(_) => "rock",
            Shape::Blob(_) => "blob",
            Shape::Fence(_) => "fence",
        }
    }

    /// Its name as a set writes it: a seeded or counted shape with its
    /// number (`rock-7`, `fence-5`) once the shapes know the suffix, the
    /// plain name until then.
    pub fn written(self) -> String {
        let n = match self {
            Shape::Rock(n) | Shape::Blob(n) | Shape::Fence(n) => n,
            _ => return self.name().to_string(),
        };
        let named = format!("{}-{n}", self.name());
        if angel_stage::shapes::by_name(&named).is_some() { named } else { self.name().to_string() }
    }
}

// ----------------------------------------------------------- the blocks ----

/// One line of a set: a shape of a kind, the middle of its foot, its size.
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    pub shape: Shape,
    pub kind: Kind,
    /// The middle of its foot: x, y (its bottom), z.
    pub at: [f32; 3],
    /// Its extent along x, y, z before the turn.
    pub size: [f32; 3],
    /// Degrees about the vertical, counter-clockwise seen from above.
    pub turn: f32,
    pub id: Option<String>,
    /// The primitive of the coming library that would read better.
    pub todo: Option<&'static str>,
}

impl Solid {
    pub fn new(shape: Shape, kind: Kind, at: [f32; 3], size: [f32; 3]) -> Self {
        Self { shape, kind, at, size, turn: 0.0, id: None, todo: None }
    }

    /// A box over the footprint `[x0, z0, x1, z1]`, its bottom at `y`, `h` high.
    pub fn block(kind: Kind, [x0, z0, x1, z1]: [f32; 4], y: f32, h: f32) -> Self {
        Self::new(Shape::Box, kind, [(x0 + x1) / 2.0, y, (z0 + z1) / 2.0], [x1 - x0, h, z1 - z0])
    }

    pub(crate) fn todo(mut self, prim: &'static str) -> Self {
        self.todo = Some(prim);
        self
    }

    pub(crate) fn with_todo(mut self, prim: Option<&'static str>) -> Self {
        self.todo = prim;
        self
    }

    pub(crate) fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub(crate) fn turned(mut self, degrees: f32) -> Self {
        self.turn = degrees;
        self
    }

    pub fn bottom(&self) -> f32 {
        self.at[1]
    }

    pub fn top(&self) -> f32 {
        self.at[1] + self.size[1]
    }

    /// Its footprint `[x0, z0, x1, z1]` (a turned one's bounding square).
    pub fn footprint(&self) -> [f32; 4] {
        let (hx, hz) = (self.size[0] / 2.0, self.size[2] / 2.0);
        let (hx, hz) = if self.turn.rem_euclid(180.0) == 0.0 {
            (hx, hz)
        } else {
            let (s, c) = self.turn.to_radians().sin_cos();
            (c.abs() * hx + s.abs() * hz, s.abs() * hx + c.abs() * hz)
        };
        [self.at[0] - hx, self.at[2] - hz, self.at[0] + hx, self.at[2] + hz]
    }

    /// Points on it whose hull, seen by any camera, is its outline.
    pub fn outline(&self) -> Vec<[f32; 3]> {
        let [cx, y0, cz] = self.at;
        let [sx, sy, sz] = self.size;
        let (s, c) = self.turn.to_radians().sin_cos();
        // Counter-clockwise seen from above: +x turns toward -z.
        let place = |lx: f32, y: f32, lz: f32| [cx + lx * c + lz * s, y, cz - lx * s + lz * c];
        let (hx, hy, hz) = (sx / 2.0, sy / 2.0, sz / 2.0);
        match self.shape {
            Shape::Box => {
                let mut points = Vec::with_capacity(8);
                for y in [y0, y0 + sy] {
                    for (lx, lz) in [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)] {
                        points.push(place(lx, y, lz));
                    }
                }
                points
            }
            Shape::Sphere => {
                let mut points = vec![place(0.0, y0, 0.0), place(0.0, y0 + sy, 0.0)];
                for ring in 1..8 {
                    let (sin_t, cos_t) = (ring as f32 / 8.0 * std::f32::consts::PI).sin_cos();
                    for k in 0..16 {
                        let (sp, cp) = (k as f32 / 16.0 * std::f32::consts::TAU).sin_cos();
                        points.push(place(hx * sin_t * cp, y0 + hy + hy * cos_t, hz * sin_t * sp));
                    }
                }
                points
            }
            Shape::Cone => {
                let mut points = vec![place(0.0, y0 + sy, 0.0)];
                for k in 0..16 {
                    let (sp, cp) = (k as f32 / 16.0 * std::f32::consts::TAU).sin_cos();
                    points.push(place(hx * cp, y0, hz * sp));
                }
                points
            }
            Shape::Gem => vec![
                place(-hx, y0 + hy, 0.0),
                place(hx, y0 + hy, 0.0),
                place(0.0, y0, 0.0),
                place(0.0, y0 + sy, 0.0),
                place(0.0, y0 + hy, -hz),
                place(0.0, y0 + hy, hz),
            ],
            Shape::Cylinder | Shape::Frustum => {
                let top = if self.shape == Shape::Frustum { 0.6 } else { 1.0 };
                let mut points = Vec::with_capacity(32);
                for k in 0..16 {
                    let (sp, cp) = (k as f32 / 16.0 * std::f32::consts::TAU).sin_cos();
                    points.push(place(hx * cp, y0, hz * sp));
                    points.push(place(hx * top * cp, y0 + sy, hz * top * sp));
                }
                points
            }
            Shape::Rock(_) | Shape::Blob(_) => {
                // Inside the unit cube and round: the sphere's outline holds it.
                let mut points = vec![place(0.0, y0, 0.0), place(0.0, y0 + sy, 0.0)];
                for ring in 1..8 {
                    let (sin_t, cos_t) = (ring as f32 / 8.0 * std::f32::consts::PI).sin_cos();
                    for k in 0..16 {
                        let (sp, cp) = (k as f32 / 16.0 * std::f32::consts::TAU).sin_cos();
                        points.push(place(hx * sin_t * cp, y0 + hy + hy * cos_t, hz * sin_t * sp));
                    }
                }
                points
            }
            Shape::Wedge | Shape::Stairs => {
                let mut points = Vec::with_capacity(6);
                for (lx, lz) in [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)] {
                    points.push(place(lx, y0, lz));
                }
                points.extend([place(-hx, y0 + sy, -hz), place(hx, y0 + sy, -hz)]);
                points
            }
            Shape::Gable | Shape::Hip | Shape::Pyramid => {
                let ridge = match self.shape {
                    Shape::Gable => hx,
                    Shape::Hip => hx * 0.5,
                    _ => 0.0,
                };
                let mut points = Vec::with_capacity(6);
                for (lx, lz) in [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)] {
                    points.push(place(lx, y0, lz));
                }
                points.extend([place(-ridge, y0 + sy, 0.0), place(ridge, y0 + sy, 0.0)]);
                points
            }
            Shape::Arch | Shape::Fence(_) => {
                let mut points = Vec::with_capacity(8);
                for y in [y0, y0 + sy] {
                    for (lx, lz) in [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)] {
                        points.push(place(lx, y, lz));
                    }
                }
                points
            }
        }
    }
}

/// What a run of a set's lines is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The ground slab under the floor line.
    Ground,
    /// Water, lava or spores, below the floor line.
    Hazard,
    /// The room's walls, or its edge of woods, hedge or rock.
    Wall,
    /// A doorway's leaf, lintel, posts and torches.
    Door,
    /// What stands on the room's blocked tiles, and the boot-high things
    /// on its open ground.
    Prop,
}

/// A run of a set's lines under one comment: a wall, a prop.
#[derive(Clone, Debug)]
pub struct Group {
    pub name: String,
    pub role: Role,
    /// The tiles it stands for, when it is a prop.
    pub rect: Option<Rect>,
    pub solids: Vec<Solid>,
    /// Where each of its parts begins in `solids`: a part (a tile of wall,
    /// one tree) is sized for the camera on its own.
    pub parts: Vec<usize>,
    /// The least share of its heights a part kept for the camera (1: all).
    pub kept: f32,
    /// Whether a part that would sink to a boot's height is squashed to a
    /// ceiling instead (`fit`): the second wave's props, not the pilot's.
    pub squash: bool,
}

impl Group {
    pub(crate) fn new(role: Role, name: impl Into<String>, solids: Vec<Solid>) -> Self {
        Self::of_parts(role, name, vec![solids])
    }

    pub(crate) fn of_parts(role: Role, name: impl Into<String>, parts: Vec<Vec<Solid>>) -> Self {
        let mut starts = Vec::with_capacity(parts.len());
        let mut solids = Vec::new();
        for part in parts.into_iter().filter(|p| !p.is_empty()) {
            starts.push(solids.len());
            solids.extend(part);
        }
        Self { name: name.into(), role, rect: None, solids, parts: starts, kept: 1.0, squash: false }
    }
}

// ---------------------------------------------------------- the dressing ----

/// The trees of a wood.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    Oak,
    Copper,
    Pine,
}

/// How a room of the world is closed, as `world::edge_of` draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Wood(Leaf),
    Hedge,
    Rock,
}

/// What an outdoor room's open ground is, away from its roads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Grass,
    /// Grass and mud in patches (`world::field`'s noise for the marsh).
    Marsh,
    Sand,
    /// Bare trodden earth: a yard.
    Earth,
    /// Snow on the high ground (plaster-coloured in the capture).
    Snow,
    /// Grass with patches of grey scree (`world::field`'s noise).
    Scree,
}

/// The kind of place a room is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// A hall of the world: dressed stone walls, flagstones, braziers in
    /// the corners (`world::indoor`).
    Indoor,
    /// A room of the open world, closed by its edge.
    Outdoor { edge: Edge, field: Field },
    /// A hall of dressed stone with no braziers: the College's rooms.
    Hall,
    /// An open court inside stone walls: its field, its paving where the
    /// dressing says, no braziers.
    Courtyard { field: Field },
    /// A chamber of a delve, in its pack's stone.
    Dungeon(Pack),
}

/// What stands on a rectangle of a room's tiles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prop {
    /// The smith's hearth: a stone bed, its coals, the hood over them.
    Hearth,
    /// An anvil on its stump.
    Anvil,
    /// An armour rack against the wall.
    Rack,
    /// A water trough `depth` deep at the south of its tiles, `height` high.
    Trough { depth: f32, height: f32 },
    /// A market stall: a counter, its goods, an awning on posts. A stall
    /// that faces north has its counter at the north of its tiles.
    Stall { north: bool, goods: Kind },
    /// A stone market cross on its steps.
    Cross,
    /// A long timber building under thatch, with stall doors (x, width in
    /// stage pixels) on its front.
    Barn { doors: &'static [(f32, f32)] },
    /// A beam across a way through, over head height, from one building to
    /// the next.
    Arch,
    /// A fence of rails on posts along its tiles.
    Fence,
    /// A cottage: walls, a roof, a door and windows on its front.
    Cottage { walls: Kind, roof: Kind, windows: u8 },
    /// A broad tree: a trunk, a canopy.
    Tree,
    /// A staked-out building site: corner stakes, a cord round them,
    /// boards and stones waiting.
    Site,
    /// A boarded jetty on posts at the water's edge.
    Jetty,
    /// A travel plate or stair a knight stands on: a low platform.
    Plate(Kind),
    /// Two posts and a bar over head height across a way in.
    Gate,
    /// Rushes standing in water.
    Reeds,
    /// Something boot-high on open ground (bar iron, a board), by name.
    Low(Kind, &'static str),
    /// A low heap on open ground (coal, hay), by name.
    Heap(Kind, &'static str),
    /// The Crypt: a tomb or, where the block is two tiles both ways, a pillar.
    Tomb,
    /// The Mines: boulders, one to a tile.
    Boulder,
    /// Dragon Keep: a cooling slag heap.
    Slag,
    /// The Fungal Deep: a mushroom to a tile.
    Mushroom,
    /// The Swamp: a tussock of rushes on dry ground, one to a tile (its
    /// blocked tiles, as its painter draws reeds there).
    Tussock,
    /// The Archive: a bookcase.
    Shelf,
    /// The Unknown: a standing shard.
    Shard,
    /// Indoors: a crate.
    Crate,
    /// A tower: a drum of stone under a cap, its window lit; with `sails`
    /// a windmill's, with `lamp` a beacon's.
    Tower { walls: Kind, cap: Kind, height: f32, sails: bool, lamp: bool },
    /// A fenced yard, a step up from the ground, its floor of `floor`; with
    /// `drying`, cloth hung out on a cord between two poles.
    Pen { floor: Kind, drying: bool },
    /// Tiered benches or steps of `Kind`, rising toward the side named.
    Stand(Kind, Side),
    /// A tent or marquee of cloth.
    Tent(Kind),
    /// Natural rock standing up: a mass with peaks, snow-capped when `cap`.
    Outcrop { cap: Option<Kind> },
    /// A stack of cut logs.
    Logs,
    /// A clipped hedge.
    Hedge,
    /// A column, a pier or a post of `kind`, `height` high; with `lamp` a
    /// fire on its top. It stands from the bed under the floor line.
    Column { kind: Kind, height: f32, lamp: bool },
    /// A long hay rick.
    Rick,
    /// A broken wall with a doorway into the dark.
    Ruin,
    /// A boarded van on wheels under a canvas tilt.
    Van,
    /// A wash vat: a coopered tub of water.
    Vat,
    /// The players' stage: boards, a curtained backdrop, footlights.
    Stage,
    /// The archery butts: a turf bank with straw butts on it.
    Butts,
    /// An open shed: a roof on posts over a bench.
    Shed,
    /// Wheat stooks in rows.
    Stooks,
    /// A table, desk, counter or bench `height` high, its top of `top`.
    Table { top: Kind, height: f32 },
    /// A tall case against the wall: a bookcase or a board, its face of
    /// `face`.
    Case { body: Kind, face: Kind, height: f32 },
    /// An iron pot on its fire in a stone ring.
    Cauldron,
    /// A basin of water round a spout.
    Fountain,
    /// A furnace: a stone body, a hood, its mouth lit or cold.
    Furnace { lit: bool },
    /// A dais: two steps and a seat on them, a banner behind.
    Dais,
    /// A well with its little roof on two posts.
    Well,
    /// A flat deck or rug on open ground, a step above it (a wooden one on
    /// piles).
    Deck(Kind),
    /// A flag store: a timber hut with flagpoles on its roof.
    Flags,
    /// A rail on posts a knight cannot pass, its lower rail thick enough to
    /// show the tile it closes (a barrier, a hitching rail, a cliff rail).
    Rail,
    /// A stairwell's stone ring round dark water.
    Shaft,
    /// A telescope on its stand.
    Telescope,
    /// A block of dressed `kind`, `height` high: a plinth, a pedestal, a
    /// parapet.
    Plinth { kind: Kind, height: f32 },
    /// Dame Fortune's wheel on its timber stand, red drapes either side.
    Wheel,
}

impl Prop {
    /// Its name in the set's comments and ids.
    pub fn word(self) -> &'static str {
        match self {
            Prop::Hearth => "hearth",
            Prop::Anvil => "anvil",
            Prop::Rack => "rack",
            Prop::Trough { .. } => "trough",
            Prop::Stall { .. } => "stall",
            Prop::Cross => "cross",
            Prop::Barn { .. } => "stable",
            Prop::Arch => "arch",
            Prop::Fence => "fence",
            Prop::Cottage { .. } => "cottage",
            Prop::Tree => "tree",
            Prop::Site => "site",
            Prop::Jetty => "jetty",
            Prop::Plate(_) => "plate",
            Prop::Gate => "gate",
            Prop::Reeds => "reeds",
            Prop::Low(_, what) | Prop::Heap(_, what) => what,
            Prop::Tomb => "tomb",
            Prop::Boulder => "boulder",
            Prop::Slag => "slag",
            Prop::Mushroom => "mushroom",
            Prop::Tussock => "tussock",
            Prop::Shelf => "shelf",
            Prop::Shard => "shard",
            Prop::Crate => "crate",
            Prop::Tower { sails: true, .. } => "mill",
            Prop::Tower { lamp: true, .. } => "beacon",
            Prop::Tower { .. } => "tower",
            Prop::Pen { drying: true, .. } => "drying-yard",
            Prop::Pen { .. } => "pen",
            Prop::Stand(..) => "stand",
            Prop::Tent(_) => "tent",
            Prop::Outcrop { .. } => "outcrop",
            Prop::Logs => "logs",
            Prop::Hedge => "hedge",
            Prop::Column { kind: Kind::Wood, .. } => "post",
            Prop::Column { kind: Kind::Bone, .. } => "skull-stack",
            Prop::Column { kind: Kind::Metal, .. } => "bell",
            Prop::Column { height, .. } if height < 30.0 => "pier",
            Prop::Column { .. } => "column",
            Prop::Rick => "rick",
            Prop::Ruin => "ruin",
            Prop::Van => "van",
            Prop::Vat => "vat",
            Prop::Stage => "stage",
            Prop::Butts => "butts",
            Prop::Shed => "shed",
            Prop::Stooks => "stooks",
            Prop::Table { top, .. } => match top {
                Kind::Plaster => "desk",
                Kind::Wood => "bench",
                Kind::Metal => "counter",
                _ => "table",
            },
            Prop::Case { body, face, .. } => match (body, face) {
                (Kind::Wood, Kind::Cloth) => "bookcase",
                (Kind::Wood, Kind::Plaster) => "board",
                (Kind::Wood, Kind::Slate) => "slate-board",
                (Kind::Wood, Kind::Water) => "jar-shelf",
                (Kind::Wood, Kind::Trunk) => "wheel-rack",
                (Kind::Wood, Kind::Crop) => "drying-rack",
                (Kind::Wood, Kind::Stone) => "flat",
                (Kind::Wood, Kind::Metal) => "armour-rack",
                (Kind::Slate, _) => "arcade",
                (Kind::Bone, _) => "coffer",
                _ => "case",
            },
            Prop::Cauldron => "cauldron",
            Prop::Fountain => "fountain",
            Prop::Furnace { .. } => "furnace",
            Prop::Dais => "dais",
            Prop::Well => "well",
            Prop::Deck(Kind::Rug) => "rug",
            Prop::Deck(_) => "deck",
            Prop::Flags => "flags",
            Prop::Rail => "rail",
            Prop::Shaft => "shaft",
            Prop::Telescope => "telescope",
            Prop::Plinth { .. } => "plinth",
            Prop::Wheel => "wheel",
        }
    }

    /// Whether it is one of the second wave's props (`Group::squash`).
    pub fn squashes(self) -> bool {
        !matches!(
            self,
            Prop::Hearth
                | Prop::Anvil
                | Prop::Rack
                | Prop::Trough { .. }
                | Prop::Stall { .. }
                | Prop::Cross
                | Prop::Barn { .. }
                | Prop::Arch
                | Prop::Fence
                | Prop::Cottage { .. }
                | Prop::Tree
                | Prop::Site
                | Prop::Jetty
                | Prop::Plate(_)
                | Prop::Gate
                | Prop::Reeds
                | Prop::Low(..)
                | Prop::Heap(..)
                | Prop::Tomb
                | Prop::Boulder
                | Prop::Slag
                | Prop::Mushroom
                | Prop::Tussock
                | Prop::Shelf
                | Prop::Shard
                | Prop::Crate
        )
    }

    /// Whether it stands on ground a knight walks (and is built to be
    /// boot-high there, or high over it), not on blocked tiles.
    pub fn on_open_ground(self) -> bool {
        matches!(self, Prop::Low(..) | Prop::Heap(..) | Prop::Arch | Prop::Gate | Prop::Jetty | Prop::Plate(_) | Prop::Deck(_))
    }
}

/// What is fixed to the face of a room's far (north) wall, over the floor
/// and within the wall's own thickness: it never stands on a tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fix {
    /// A window, lit.
    Window,
    /// A banner of cloth.
    Banner,
    /// A hearth in the wall: its surround and its fire.
    Hearth,
    /// A lamp or brazier on a bracket.
    Lamp,
}

/// A fixture on the far wall: what it is, and its middle along the wall in
/// stage pixels.
pub type Fixture = (Fix, f32);

/// What a room's tiles are, read off the painters that draw it today.
#[derive(Clone, Copy, Debug)]
pub struct Dressing {
    /// The set's id: lower case letters, digits and `-`.
    pub id: &'static str,
    pub title: &'static str,
    pub setting: Setting,
    /// Tiles of road or path through an outdoor room.
    pub roads: &'static [Rect],
    pub props: &'static [(Rect, Prop)],
    /// What the game draws that the blocks leave to the paint (things on
    /// open ground a knight walks through), for the art lane.
    pub notes: &'static [&'static str],
    /// Tiles of paving (cobbles, flags) through an open room: a road kind.
    pub paved: &'static [Rect],
    /// What is fixed to the far wall.
    pub fixtures: &'static [Fixture],
    /// What the place is, in a sentence, for the painter.
    pub what: &'static str,
}

impl Dressing {
    /// The empty dressing: an item names its own fields over it.
    pub const BASE: Dressing = Dressing {
        id: "",
        title: "",
        setting: Setting::Indoor,
        roads: &[],
        props: &[],
        notes: &[],
        paved: &[],
        fixtures: &[],
        what: "",
    };
}

// ------------------------------------------------------------ the camera ----

/// The game's camera on a room: the Delve's stage camera at its tilt, for
/// a stage the room's size.
pub struct View {
    tilt: Tilt,
    pub size: (u32, u32),
    /// What the camera moves the world by: nothing for a room, the stage's
    /// middle less the centre for a stretch of the kingdom (`camera chunk`).
    offset: [f32; 2],
}

/// A solid's outline on the screen: a convex polygon, counter-clockwise.
#[derive(Clone)]
pub struct Hull {
    points: Vec<[f32; 2]>,
    bounds: [f32; 4],
}

impl View {
    pub fn of(room: &Room) -> Self {
        let size = ((room.cols as f32 * TILE) as u32, (room.rows as f32 * TILE) as u32);
        let tilt = Tilt::for_stage(TILT, size).expect("the Delve's rooms are seen at a tilt");
        Self { tilt, size, offset: [0.0, 0.0] }
    }

    /// The camera a set names with `camera chunk <w> <h> centre <cx> <cz>`:
    /// the room camera for a stage of `size`, the world moved so the floor
    /// point `centre` is where the stage's middle is.
    pub fn chunk(size: (u32, u32), centre: [f32; 2]) -> Self {
        let tilt = Tilt::for_stage(TILT, size).expect("the kingdom is seen at a tilt");
        let offset = [size.0 as f32 / 2.0 - centre[0], size.1 as f32 / 2.0 - centre[1]];
        Self { tilt, size, offset }
    }

    /// Where a point of the room's world lands on the stage.
    pub fn screen(&self, [x, y, z]: [f32; 3]) -> [f32; 2] {
        let p = [x + self.offset[0], z + self.offset[1]];
        let floor = self.tilt.floor(p);
        [floor[0], floor[1] - y * self.tilt.scale_at(p)]
    }

    /// A solid's outline on the screen, above the floor line: what lies
    /// below it is under the ground and hides nothing.
    pub fn hull(&self, solid: &Solid) -> Hull {
        Hull::of(solid.outline().into_iter().map(|[x, y, z]| self.screen([x, y.max(0.0), z])).collect())
    }

    /// The share of tile `(col, row)`'s floor that `hulls` hide.
    pub fn hidden(&self, hulls: &[Hull], tile: (i32, i32)) -> f32 {
        self.hidden_by(&hulls.iter().collect::<Vec<_>>(), tile)
    }

    /// The tile's floor on the screen: the bounds of its corners.
    pub(crate) fn tile_bounds(&self, (col, row): (i32, i32)) -> [f32; 4] {
        let (x0, z0, x1, z1) = (col as f32 * TILE, row as f32 * TILE, (col + 1) as f32 * TILE, (row + 1) as f32 * TILE);
        bounds(&[
            self.screen([x0, 0.0, z0]),
            self.screen([x1, 0.0, z1]),
            self.screen([x0, 0.0, z1]),
            self.screen([x1, 0.0, z0]),
        ])
    }

    pub(crate) fn hidden_by(&self, hulls: &[&Hull], (col, row): (i32, i32)) -> f32 {
        const N: usize = 6;
        let b = self.tile_bounds((col, row));
        let near: Vec<&&Hull> = hulls.iter().filter(|h| overlap(h.bounds, b)).collect();
        if near.is_empty() {
            return 0.0;
        }
        let mut hidden = 0;
        for j in 0..N {
            for i in 0..N {
                let x = (col as f32 + (i as f32 + 0.5) / N as f32) * TILE;
                let z = (row as f32 + (j as f32 + 0.5) / N as f32) * TILE;
                let p = self.screen([x, 0.0, z]);
                if near.iter().any(|h| h.contains(p)) {
                    hidden += 1;
                }
            }
        }
        hidden as f32 / (N * N) as f32
    }
}

fn bounds(points: &[[f32; 2]]) -> [f32; 4] {
    points.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    })
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

impl Hull {
    fn of(mut points: Vec<[f32; 2]>) -> Self {
        let bounds = bounds(&points);
        points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
        points.dedup();
        let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
        let mut hull: Vec<[f32; 2]> = Vec::with_capacity(points.len() * 2);
        for pass in 0..2 {
            let start = hull.len();
            let iter: Box<dyn Iterator<Item = &[f32; 2]>> =
                if pass == 0 { Box::new(points.iter()) } else { Box::new(points.iter().rev()) };
            for &p in iter {
                while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                    hull.pop();
                }
                hull.push(p);
            }
            hull.pop();
        }
        Self { points: hull, bounds }
    }

    /// Whether `p` lies strictly inside.
    pub fn contains(&self, p: [f32; 2]) -> bool {
        if self.points.len() < 3 || !(p[0] > self.bounds[0] && p[0] < self.bounds[2] && p[1] > self.bounds[1] && p[1] < self.bounds[3]) {
            return false;
        }
        let n = self.points.len();
        (0..n).all(|i| {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) > 1e-3
        })
    }
}

/// The hulls of what in `solids` stands taller than a boot: what can hide
/// the floor.
pub fn occluders<'a>(view: &View, solids: impl IntoIterator<Item = &'a Solid>) -> Vec<Hull> {
    solids.into_iter().filter(|s| s.top() > LOW).map(|s| view.hull(s)).collect()
}

/// The walkable tiles of a room.
pub fn open_tiles(room: &Room) -> Vec<(i32, i32)> {
    (0..room.rows as i32)
        .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
        .filter(|&(c, r)| walkable(room.tile(c, r)))
        .collect()
}

/// The most of any walkable tile that `solids` hide.
pub fn worst_hidden(view: &View, open: &[(i32, i32)], solids: &[Solid]) -> f32 {
    let hulls = occluders(view, solids);
    if hulls.is_empty() {
        return 0.0;
    }
    open.iter().map(|&t| view.hidden(&hulls, t)).fold(0.0, f32::max)
}

/// Whether `solids`, set among the hulls already `placed`, keep the floor in
/// view: alone they hide no more than [`HIDE_ONE`] of any walkable tile, and
/// with everything placed before them no more than [`HIDE_ALL`].
pub(crate) fn keeps_in_view(view: &View, open: &[(i32, i32)], placed: &[Hull], solids: &[Solid]) -> bool {
    let hulls = occluders(view, solids);
    if hulls.is_empty() {
        return true;
    }
    let reach = bounds(&hulls.iter().flat_map(|h| [[h.bounds[0], h.bounds[1]], [h.bounds[2], h.bounds[3]]]).collect::<Vec<_>>());
    let own: Vec<&Hull> = hulls.iter().collect();
    let mut all: Vec<&Hull> = placed.iter().collect();
    all.extend(own.iter().copied());
    open.iter().filter(|&&t| overlap(view.tile_bounds(t), reach)).all(|&t| {
        view.hidden_by(&own, t) <= HIDE_ONE && view.hidden_by(&all, t) <= HIDE_ALL
    })
}

/// Whether `solids`, made of `before` (a lit room's masses,
/// `rooms_lit_mass`), hide no walkable tile of `open` more than `before`
/// did, a hair over, as the camera `view` sees them: what the generator
/// kept in view stays in view.
pub fn worst_hidden_ok(view: &View, open: &[(i32, i32)], solids: &[Solid], before: &[Solid]) -> bool {
    let (now, then) = (occluders(view, solids), occluders(view, before));
    if now.is_empty() {
        return true;
    }
    let reach = bounds(&now.iter().flat_map(|h| [[h.bounds[0], h.bounds[1]], [h.bounds[2], h.bounds[3]]]).collect::<Vec<_>>());
    let (now, then): (Vec<&Hull>, Vec<&Hull>) = (now.iter().collect(), then.iter().collect());
    open.iter()
        .filter(|&&t| overlap(view.tile_bounds(t), reach))
        .all(|&t| view.hidden_by(&now, t) <= view.hidden_by(&then, t) + 0.02)
}

/// Scale a group's heights by `keep` (its bottoms and tops together, so
/// what stands on what still does), to the half pixel below. What lies
/// below the floor line stays where it is.
fn scaled(solids: &[Solid], keep: f32) -> Vec<Solid> {
    let down = |v: f32| if v <= 0.0 { v } else { (v * keep * 2.0).floor() / 2.0 };
    solids
        .iter()
        .map(|s| {
            let (bottom, top) = (down(s.bottom()), down(s.top()));
            let mut s = s.clone();
            s.at[1] = bottom;
            s.size[1] = (top - bottom).max(0.5);
            s
        })
        .collect()
}

/// Squash solids to a ceiling `cap` pixels high: what stands above it is cut
/// off there, and what lies wholly above it is gone.
fn capped(solids: &[Solid], cap: f32) -> Vec<Solid> {
    solids
        .iter()
        .filter(|s| s.bottom() < cap - 0.5)
        .map(|s| {
            let mut s = s.clone();
            let top = s.top().min(cap);
            s.size[1] = top - s.at[1];
            s
        })
        .collect()
}

/// Size solids for the camera ([`keeps_in_view`]): all their heights when
/// that keeps the floor in view, else the largest share of them that does.
/// A part whose first solid (its body, the one that stands on its blocked
/// tiles) would fall to a boot's height that way is squashed to a ceiling
/// instead, the lowest that still shows it (a furnace's hood and flue come
/// off before its body does).
pub fn fit(solids: &[Solid], view: &View, open: &[(i32, i32)], placed: &[Hull], squash: bool) -> (Vec<Solid>, f32) {
    if keeps_in_view(view, open, placed, solids) {
        return (solids.to_vec(), 1.0);
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        if keeps_in_view(view, open, placed, &scaled(solids, mid)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let trimmed = scaled(solids, lo);
    let body = solids.first().map(Solid::top).unwrap_or(0.0);
    if squash && body > LOW + 1.0 && trimmed.first().is_some_and(|b| b.top() <= LOW + 0.5) {
        let most = solids.iter().map(Solid::top).fold(0.0, f32::max);
        let floor = LOW + 1.5;
        let (mut lo, mut hi) = (floor, most);
        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            if keeps_in_view(view, open, placed, &capped(solids, mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        return (capped(solids, lo), lo / most);
    }
    (trimmed, lo)
}

/// Size each of a group's parts for the camera, in turn ([`fit`]), among
/// what is `placed` already; each part's outline joins it.
pub fn trim(group: &mut Group, view: &View, open: &[(i32, i32)], placed: &mut Vec<Hull>) {
    let mut ends: Vec<usize> = group.parts.iter().skip(1).copied().collect();
    ends.push(group.solids.len());
    let mut solids = Vec::with_capacity(group.solids.len());
    for (&a, &b) in group.parts.iter().zip(&ends) {
        let (part, kept) = fit(&group.solids[a..b], view, open, placed, group.squash);
        placed.extend(occluders(view, &part));
        solids.extend(part);
        group.kept = group.kept.min(kept);
    }
    group.solids = solids;
}

/// Join boxes that lie end to end along a run with the same kind, bottom,
/// height and breadth: a wall's tiles, once each is sized, as few lines as
/// there are heights.
pub(crate) fn join_runs(solids: Vec<Solid>) -> Vec<Solid> {
    let mut out: Vec<Solid> = Vec::with_capacity(solids.len());
    for s in solids {
        if let Some(last) = out.last_mut() {
            let same = last.shape == Shape::Box
                && s.shape == Shape::Box
                && last.kind == s.kind
                && last.turn == 0.0
                && s.turn == 0.0
                && last.id.is_none()
                && s.id.is_none()
                && last.todo == s.todo
                && last.bottom() == s.bottom()
                && last.top() == s.top();
            let [a0, b0, a1, b1] = last.footprint();
            let [c0, d0, c1, d1] = s.footprint();
            if same && b0 == d0 && b1 == d1 && a1 == c0 {
                *last = Solid::block(s.kind, [a0, b0, c1, b1], s.bottom(), s.size[1]).with_todo(s.todo);
                continue;
            }
            if same && a0 == c0 && a1 == c1 && b1 == d0 {
                *last = Solid::block(s.kind, [a0, b0, a1, d1], s.bottom(), s.size[1]).with_todo(s.todo);
                continue;
            }
        }
        out.push(s);
    }
    out
}

// ------------------------------------------------------------- the room ----

/// The room's blockout as a set file.
pub fn room_set(room: &Room, dressing: &Dressing) -> String {
    room_set_with(room, dressing, None)
}

/// The room's blockout as a set file, with the item's hand-placed landmark
/// (the text of its `landmark.set`, see [`landmark`]) appended after the
/// generated groups.
pub fn room_set_with(room: &Room, dressing: &Dressing, landmark: Option<&str>) -> String {
    let mut out = write(room, dressing, &blockout(room, dressing));
    if let Some(text) = landmark {
        let _ = writeln!(out);
        let _ = writeln!(out, "# landmark (assets/world3d/{}/landmark.set, hand-placed)", dressing.id);
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}

/// The room's blockout: its ground, hazards, walls, doors and props, each
/// sized for the camera.
pub fn blockout(room: &Room, dressing: &Dressing) -> Vec<Group> {
    let view = View::of(room);
    let open = open_tiles(room);
    let mut placed = Vec::new();
    let mut groups = vec![ground(room, dressing)];
    groups.extend(hazards(room, dressing));
    groups.extend(walls(room, dressing));
    groups.extend(doors(room, dressing));
    groups.extend(props(room, dressing));
    for group in &mut groups {
        if matches!(group.role, Role::Wall | Role::Door | Role::Prop) {
            trim(group, &view, &open, &mut placed);
        }
        if group.role == Role::Wall {
            // The wall's own blocks first, so they join, then what stands on
            // them.
            let (mut wall, rest): (Vec<Solid>, Vec<Solid>) =
                std::mem::take(&mut group.solids).into_iter().partition(|s| s.shape == Shape::Box && s.bottom() < 0.0);
            cutaway(&mut wall);
            wall.extend(rest);
            group.solids = join_runs(wall);
            group.parts = vec![0];
        }
    }
    if let Some(group) = fixtures(dressing) {
        groups.push(group);
    }
    if let Some(mut torches) = torches(room, dressing, &groups) {
        trim(&mut torches, &view, &open, &mut placed);
        groups.push(torches);
    }
    name_ids(&mut groups);
    groups
}

/// What is fixed to the far wall's inner face: boxes set into the wall's own
/// thickness (never on a tile, the face at z = [`TILE`]), a pixel or two
/// proud of it.
fn fixtures(d: &Dressing) -> Option<Group> {
    if d.fixtures.is_empty() {
        return None;
    }
    let (z0, z1) = (TILE - 1.5, TILE + 1.0);
    let mut parts = Vec::new();
    for &(what, x) in d.fixtures {
        let part = match what {
            Fix::Window => vec![Solid::block(Kind::Window, [x - 4.0, z0, x + 4.0, z1], 22.0, 10.0)],
            Fix::Banner => vec![Solid::block(Kind::Cloth, [x - 5.0, z0, x + 5.0, z1], 14.0, 26.0)],
            Fix::Hearth => vec![
                Solid::block(Kind::Stone, [x - 14.0, z0, x + 14.0, z1], 0.0, 20.0),
                Solid::block(Kind::Fire, [x - 8.0, z0 + 1.0, x + 8.0, z1 + 0.5], 2.0, 9.0),
            ],
            Fix::Lamp => vec![
                Solid::block(Kind::Metal, [x - 1.5, z0, x + 1.5, z1 + 0.5], 18.0, 5.0),
                Solid::new(Shape::Cone, Kind::Fire, [x, 23.0, TILE], [5.0, 7.0, 5.0]),
            ],
        };
        parts.push(part);
    }
    Some(Group::of_parts(Role::Prop, "fixed to the far wall: windows, banners, hearths, lamps", parts))
}

/// A run of wall tiles the camera cut down (a side wall before a side door)
/// as one low stretch at the height of its lowest tile, then the step up:
/// one cutaway, not a stair of teeth. A slope would read better still.
fn cutaway(wall: &mut [Solid]) {
    let full = wall.iter().map(Solid::top).fold(0.0, f32::max);
    let mut k = 0;
    while k < wall.len() {
        if wall[k].top() >= full {
            k += 1;
            continue;
        }
        let start = k;
        while k < wall.len() && wall[k].top() < full {
            k += 1;
        }
        let low = wall[start..k].iter().map(Solid::top).fold(f32::MAX, f32::min);
        for s in &mut wall[start..k] {
            s.size[1] = low - s.bottom();
            s.todo = Some("wedge");
        }
    }
}

/// Torches flank every doorway of a delve but the near wall's: on the
/// wall's face where the wall stands tall, on its top where the camera cut
/// it down.
fn torches(room: &Room, d: &Dressing, groups: &[Group]) -> Option<Group> {
    if !matches!(d.setting, Setting::Dungeon(_)) {
        return None;
    }
    let w = room.cols as f32 * TILE;
    let walls: Vec<&Solid> = groups.iter().filter(|g| g.role == Role::Wall).flat_map(|g| &g.solids).collect();
    let mut parts = Vec::new();
    for (side, from, to) in door_gaps(room) {
        let (a, b) = (from as f32 * TILE, to as f32 * TILE);
        for t in [a - 8.0, b + 8.0] {
            // On the wall's inner face, standing a pixel proud of it.
            let (x, z) = match side {
                Side::North => (t, TILE),
                Side::West => (TILE, t),
                Side::East => (w - TILE, t),
                Side::South => continue,
            };
            let wall_top = walls
                .iter()
                .filter(|s| {
                    let [x0, z0, x1, z1] = s.footprint();
                    let (px_, pz) = match side {
                        Side::North => (x, z - 1.0),
                        Side::West => (x - 1.0, z),
                        _ => (x + 1.0, z),
                    };
                    px_ >= x0 && px_ <= x1 && pz >= z0 && pz <= z1 && s.bottom() < 0.0
                })
                .map(|s| s.top())
                .fold(0.0, f32::max);
            let y = if wall_top >= 26.0 { 18.0 } else { wall_top };
            parts.push(vec![
                Solid::block(Kind::Metal, [x - 1.5, z - 1.5, x + 1.5, z + 1.5], y, 6.0),
                Solid::new(Shape::Cone, Kind::Fire, [x, y + 6.0, z], [4.0, 7.0, 4.0]),
            ]);
        }
    }
    (!parts.is_empty()).then(|| Group::of_parts(Role::Door, "torches by the doors", parts))
}

/// Rectangles of equal cells, row-major, each as wide and then as tall as
/// it goes: a room's runs of equal tiles as few blocks as a greedy pass
/// finds.
pub fn merge<T: Copy + PartialEq>(cols: usize, rows: usize, cells: &[Option<T>]) -> Vec<(Rect, T)> {
    let mut done = vec![false; cols * rows];
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            let Some(k) = cells[i] else {
                continue;
            };
            if done[i] {
                continue;
            }
            let mut w = 1;
            while c + w < cols && !done[i + w] && cells[i + w] == Some(k) {
                w += 1;
            }
            let mut h = 1;
            'grow: while r + h < rows {
                for x in c..c + w {
                    let j = (r + h) * cols + x;
                    if done[j] || cells[j] != Some(k) {
                        break 'grow;
                    }
                }
                h += 1;
            }
            for y in r..r + h {
                for x in c..c + w {
                    done[y * cols + x] = true;
                }
            }
            out.push(((c as i32, r as i32, w as i32, h as i32), k));
        }
    }
    out
}

pub(crate) fn inside((c, r, w, h): Rect, col: i32, row: i32) -> bool {
    col >= c && col < c + w && row >= r && row < r + h
}

/// The floor of a delve's chambers.
fn pack_floor(pack: Pack) -> Kind {
    match pack {
        Pack::Cavern | Pack::Fungal | Pack::Swamp => Kind::Ground,
        Pack::Crypt | Pack::Hellforge | Pack::Archive | Pack::Unknown => Kind::Floor,
    }
}

/// A delve's walls and its blocks' default.
fn pack_wall(pack: Pack) -> Kind {
    match pack {
        Pack::Cavern | Pack::Fungal | Pack::Swamp => Kind::Rock,
        Pack::Crypt | Pack::Hellforge | Pack::Archive | Pack::Unknown => Kind::Stone,
    }
}

/// What a room's hazard tiles are and how far below the floor they lie;
/// none for a pit, a hole in the floor into the dark.
fn hazard_of(setting: Setting) -> Option<(Kind, f32)> {
    match setting {
        Setting::Indoor | Setting::Hall | Setting::Courtyard { .. } | Setting::Outdoor { .. } => Some((Kind::Water, WATER_DROP)),
        Setting::Dungeon(Pack::Cavern | Pack::Archive | Pack::Swamp) => Some((Kind::Water, WATER_DROP)),
        // Lava: the brightest thing in the keep.
        Setting::Dungeon(Pack::Hellforge) => Some((Kind::Fire, LAVA_DROP)),
        // A spore pool: a sunken slick, glowing.
        Setting::Dungeon(Pack::Fungal) => Some((Kind::Mud, LAVA_DROP)),
        Setting::Dungeon(Pack::Crypt | Pack::Unknown) => None,
    }
}

fn ground_kind(room: &Room, d: &Dressing, col: i32, row: i32) -> Option<Kind> {
    let tile = room.tile(col, row);
    if tile == Tile::Hazard {
        return None;
    }
    if tile == Tile::Bog {
        return Some(Kind::Mud);
    }
    Some(match d.setting {
        Setting::Indoor | Setting::Hall => Kind::Floor,
        Setting::Dungeon(pack) => pack_floor(pack),
        Setting::Outdoor { field, .. } | Setting::Courtyard { field } => {
            if tile == Tile::Wall {
                // The earth under the wood, the flags under the stone.
                if matches!(d.setting, Setting::Courtyard { .. }) { Kind::Floor } else { Kind::Ground }
            } else if d.paved.iter().any(|&r| inside(r, col, row)) {
                Kind::Road
            } else if d.roads.iter().any(|&r| inside(r, col, row)) {
                Kind::Path
            } else {
                let (x, y) = (col as f32 * TILE + 8.0, row as f32 * TILE + 8.0);
                match field {
                    Field::Grass => Kind::Grass,
                    Field::Sand => Kind::Sand,
                    Field::Earth => Kind::Ground,
                    Field::Snow => Kind::Plaster,
                    Field::Marsh => {
                        if vnoise(x / 34.0, y / 26.0, 72) > 0.5 { Kind::Mud } else { Kind::Grass }
                    }
                    Field::Scree => {
                        if vnoise(x / 40.0, y / 30.0, 71) > 0.55 { Kind::Rock } else { Kind::Grass }
                    }
                }
            }
        }
    })
}

fn ground(room: &Room, d: &Dressing) -> Group {
    let (cols, rows) = (room.cols, room.rows);
    let cells: Vec<Option<Kind>> = (0..rows as i32)
        .flat_map(|r| (0..cols as i32).map(move |c| (c, r)))
        .map(|(c, r)| ground_kind(room, d, c, r))
        .collect();
    let solids = merge(cols, rows, &cells)
        .into_iter()
        .map(|(rect, kind)| Solid::block(kind, px(rect), -SLAB, SLAB))
        .collect();
    Group::new(Role::Ground, "the ground: its top is the floor line", solids)
}

fn hazards(room: &Room, d: &Dressing) -> Option<Group> {
    let any = (0..room.rows as i32).any(|r| (0..room.cols as i32).any(|c| room.tile(c, r) == Tile::Hazard));
    if !any {
        return None;
    }
    let Some((kind, drop)) = hazard_of(d.setting) else {
        return Some(Group::new(Role::Hazard, "pits: holes in the floor, dark below", Vec::new()));
    };
    let cells: Vec<Option<Kind>> = (0..room.rows as i32)
        .flat_map(|r| (0..room.cols as i32).map(move |c| (c, r)))
        .map(|(c, r)| (room.tile(c, r) == Tile::Hazard).then_some(kind))
        .collect();
    let solids = merge(room.cols, room.rows, &cells)
        .into_iter()
        .map(|(rect, kind)| Solid::block(kind, px(rect), -SLAB, SLAB - drop))
        .collect();
    let what = match kind {
        Kind::Fire => "lava",
        Kind::Mud => "the spore pool",
        _ => "water",
    };
    Some(Group::new(Role::Hazard, format!("{what}, {drop} below the floor line"), solids))
}

/// A side of a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    North,
    East,
    South,
    West,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::North => "north",
            Side::East => "east",
            Side::South => "south",
            Side::West => "west",
        }
    }
}

/// Runs of `want` tiles along each side's border, `[from, to)` along it:
/// the north and south rows whole (their corners with them), the west and
/// east columns between them.
pub fn border_runs(room: &Room, want: Tile) -> Vec<(Side, i32, i32)> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let mut out = Vec::new();
    let mut scan = |side: Side, range: std::ops::Range<i32>, at: &dyn Fn(i32) -> (i32, i32)| {
        let mut start = None;
        for k in range.clone().chain(std::iter::once(range.end)) {
            let here = k < range.end && {
                let (c, r) = at(k);
                room.tile(c, r) == want
            };
            match (here, start) {
                (true, None) => start = Some(k),
                (false, Some(s)) => {
                    out.push((side, s, k));
                    start = None;
                }
                _ => {}
            }
        }
    };
    scan(Side::North, 0..cols, &|k| (k, 0));
    scan(Side::South, 0..cols, &|k| (k, rows - 1));
    scan(Side::West, 1..rows - 1, &|k| (0, k));
    scan(Side::East, 1..rows - 1, &|k| (cols - 1, k));
    out
}

/// The footprint of a run along a side: the whole border tile's depth.
fn run_rect(room: &Room, side: Side, from: i32, to: i32) -> [f32; 4] {
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let (a, b) = (from as f32 * TILE, to as f32 * TILE);
    match side {
        Side::North => [a, 0.0, b, TILE],
        Side::South => [a, h - TILE, b, h],
        Side::West => [0.0, a, TILE, b],
        Side::East => [w - TILE, a, w, b],
    }
}

/// The near wall's two tiers over `[x0, x1)`: a step over the inner half
/// of its tiles, a parapet over the outer.
fn near_wall(kind: Kind, room: &Room, x0: f32, x1: f32, step: f32, parapet: f32) -> [Solid; 2] {
    let h = room.rows as f32 * TILE;
    [
        Solid::block(kind, [x0, h - TILE, x1, h - TILE / 2.0], -SLAB, step + SLAB),
        Solid::block(kind, [x0, h - TILE / 2.0, x1, h], -SLAB, parapet + SLAB),
    ]
}

/// A brazier on a wall's top: a bowl and its flame.
fn brazier(x: f32, y: f32, z: f32, small: bool) -> [Solid; 2] {
    let (bowl, flame) = if small { (8.0, 6.0) } else { (10.0, 8.0) };
    [
        Solid::block(Kind::Metal, [x - bowl / 2.0, z - bowl / 2.0, x + bowl / 2.0, z + bowl / 2.0], y, 3.0).todo("frustum"),
        Solid::new(Shape::Cone, Kind::Fire, [x, y + 3.0, z], [flame, flame * 1.3, flame]),
    ]
}

fn walls(room: &Room, d: &Dressing) -> Vec<Group> {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let mut groups = Vec::new();
    for (side, from, to) in border_runs(room, Tile::Wall) {
        let name = match side {
            Side::North | Side::South => format!("the {} wall, columns {from} to {}", side.word(), to - 1),
            Side::West | Side::East => format!("the {} wall, rows {from} to {}", side.word(), to - 1),
        };
        let parts = match d.setting {
            Setting::Indoor | Setting::Hall | Setting::Courtyard { .. } | Setting::Dungeon(_) => {
                let kind = match d.setting {
                    Setting::Dungeon(pack) => pack_wall(pack),
                    _ => Kind::Stone,
                };
                masonry(room, side, from, to, kind, matches!(d.setting, Setting::Indoor | Setting::Dungeon(_)))
            }
            Setting::Outdoor { edge, .. } => edge_run(room, side, from, to, edge, d),
        };
        groups.push(Group::of_parts(Role::Wall, name, parts));
    }
    // Wall tiles off the border: a wall standing in the room.
    let cells: Vec<Option<()>> = (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (c, r)))
        .map(|(c, r)| {
            let border = c == 0 || r == 0 || c == cols - 1 || r == rows - 1;
            (!border && room.tile(c, r) == Tile::Wall).then_some(())
        })
        .collect();
    for (rect, ()) in merge(room.cols, room.rows, &cells) {
        let kind = match d.setting {
            Setting::Dungeon(pack) => pack_wall(pack),
            Setting::Outdoor { edge: Edge::Hedge, .. } => Kind::Hedge,
            Setting::Outdoor { edge: Edge::Rock, .. } => Kind::Rock,
            Setting::Outdoor { .. } => Kind::Foliage,
            Setting::Indoor | Setting::Hall | Setting::Courtyard { .. } => Kind::Stone,
        };
        let (c, r, w, h) = rect;
        groups.push(Group::new(Role::Wall, format!("a wall in the room, {c},{r} {w}x{h}"), vec![Solid::block(kind, px(rect), 0.0, WALL / 2.0)]));
    }
    groups
}

/// The footprint of tile `k` of a side's run.
fn run_tile(room: &Room, side: Side, k: i32) -> [f32; 4] {
    run_rect(room, side, k, k + 1)
}

/// Dressed stone or a delve's rock, a part to each tile: the far and side
/// walls at full height (a side wall's tiles before a doorway lower, so the
/// way in stays in view), the near wall a parapet; braziers on the corners.
fn masonry(room: &Room, side: Side, from: i32, to: i32, kind: Kind, braziers: bool) -> Vec<Vec<Solid>> {
    let cols = room.cols as i32;
    let corner = |k: i32| braziers && matches!(side, Side::North | Side::South) && (k == 0 || k == cols - 1);
    if side == Side::South {
        let rect = run_rect(room, side, from, to);
        let mut part = near_wall(kind, room, rect[0], rect[2], STEP, PARAPET).to_vec();
        for k in (from..to).filter(|&k| corner(k)) {
            part.extend(brazier(k as f32 * TILE + 8.0, PARAPET, rect[3] - TILE / 4.0, true));
        }
        return vec![part];
    }
    (from..to)
        .map(|k| {
            let mut part = vec![Solid::block(kind, run_tile(room, side, k), -SLAB, WALL + SLAB)];
            if corner(k) {
                part.extend(brazier(k as f32 * TILE + 8.0, WALL, TILE / 2.0, false));
            }
            part
        })
        .collect()
}

/// A run of an outdoor room's edge, a part to each tile or tree: a wood of
/// trees over undergrowth (and a taller row of them beyond the far edge), a
/// clipped hedge, or boulders on the mountain's rock. The near edge is low.
fn edge_run(room: &Room, side: Side, from: i32, to: i32, edge: Edge, d: &Dressing) -> Vec<Vec<Solid>> {
    let rect = run_rect(room, side, from, to);
    let seed = 71 + d.id.bytes().map(u32::from).sum::<u32>();
    let mass = match edge {
        Edge::Wood(_) => Kind::Foliage,
        Edge::Hedge => Kind::Hedge,
        Edge::Rock => Kind::Rock,
    };
    if side == Side::South {
        // A clipped hedge is lower still along the near edge.
        let low = if edge == Edge::Hedge { 0.6 } else { 0.75 };
        let [step, parapet] = near_wall(mass, room, rect[0], rect[2], STEP / 2.0, PARAPET * low);
        let prim = match edge {
            Edge::Wood(_) => "blob",
            Edge::Rock => "rock",
            Edge::Hedge => "box",
        };
        let parapet = if prim == "box" { parapet } else { parapet.todo(prim) };
        return vec![vec![step, parapet]];
    }
    // Along the run: where it starts and ends, in stage pixels.
    let (lo, hi) = match side {
        Side::North | Side::South => (rect[0], rect[2]),
        Side::West | Side::East => (rect[1], rect[3]),
    };
    let mut parts = Vec::new();
    match edge {
        Edge::Hedge => {
            for k in from..to {
                parts.push(vec![Solid::block(Kind::Hedge, run_tile(room, side, k), -SLAB, WALL / 2.0 + SLAB)]);
            }
        }
        Edge::Rock => {
            for k in from..to {
                // The far edge stands high, the sides lower.
                let high = if side == Side::North { 26.0 } else { 16.0 };
                parts.push(vec![Solid::block(Kind::Rock, run_tile(room, side, k), -SLAB, high + SLAB).todo("rock")]);
            }
            for k in from..to {
                let v = hash(k, side as i32, seed);
                let s = 14.0;
                let (x, z) = along(room, side, k, v, s, lo, hi);
                parts.push(vec![
                    Solid::new(Shape::Gem, Kind::Rock, [x, 10.0, z], [s, 12.0 + (v >> 12 & 7) as f32, s]).turned(45.0).todo("rock"),
                ]);
            }
        }
        Edge::Wood(leaf) => {
            for k in from..to {
                parts.push(vec![Solid::block(Kind::Foliage, run_tile(room, side, k), -SLAB, 5.0 + SLAB).todo("blob")]);
            }
            for k in from..to {
                let v = hash(k, side as i32, seed);
                let spread = 18.0 + (v >> 8 & 3) as f32;
                let (x, z) = along(room, side, k, v, spread, lo, hi);
                parts.push(tree_at(leaf, x, z, v, side, spread).to_vec());
            }
            if side == Side::North {
                // The wood goes on beyond the far edge, taller, save where a
                // road leaves through a door.
                for k in from..to {
                    if (k - 1..=k + 1).any(|c| room.tile(c, 0) == Tile::Door) {
                        continue;
                    }
                    let v = hash(k, 9, seed);
                    let x = k as f32 * TILE + 8.0 + (v % 9) as f32 - 4.0;
                    let s = [22.0 + (v >> 4 & 7) as f32, 30.0 + (v >> 8 & 7) as f32, 18.0];
                    let z = -s[2] / 2.0;
                    parts.push(vec![if leaf == Leaf::Pine {
                        Solid::new(Shape::Cone, Kind::Foliage, [x, 0.0, z], [s[0] * 0.8, s[1] + 8.0, s[2]])
                    } else {
                        Solid::new(Shape::Sphere, Kind::Foliage, [x, 0.0, z], s)
                    }]);
                }
            }
        }
    }
    parts
}

/// Where along a side's run thing `k`, `spread` across along the run,
/// stands: about the middle of its tile, jittered, kept within the run.
fn along(room: &Room, side: Side, k: i32, v: u32, spread: f32, lo: f32, hi: f32) -> (f32, f32) {
    let jitter = (v % 7) as f32 - 3.0;
    let at = (k as f32 * TILE + 8.0 + jitter).clamp(lo + spread / 2.0, (hi - spread / 2.0).max(lo + spread / 2.0));
    let w = room.cols as f32 * TILE;
    match side {
        Side::North | Side::South => (at, TILE / 2.0),
        Side::West => (TILE / 2.0, at),
        Side::East => (w - TILE / 2.0, at),
    }
}

/// One tree of a wood's edge: a trunk and a crown kept within its tile
/// across the edge.
fn tree_at(leaf: Leaf, x: f32, z: f32, v: u32, side: Side, spread: f32) -> [Solid; 2] {
    let trunk_h = 9.0 + (v >> 4 & 3) as f32;
    let across = TILE;
    let (sx, sz) = if matches!(side, Side::North | Side::South) { (spread, across) } else { (across, spread) };
    let trunk = Solid::block(Kind::Trunk, [x - 2.0, z - 2.0, x + 2.0, z + 2.0], 0.0, trunk_h).todo("cylinder");
    let crown = match leaf {
        Leaf::Pine => Solid::new(Shape::Cone, Kind::Foliage, [x, trunk_h - 4.0, z], [sx * 0.9, 24.0 + (v >> 12 & 7) as f32, sz * 0.9]),
        Leaf::Oak | Leaf::Copper => {
            Solid::new(Shape::Sphere, Kind::Foliage, [x, trunk_h - 3.0, z], [sx, 16.0 + (v >> 12 & 3) as f32, sz])
        }
    };
    [trunk, crown]
}

/// Contiguous door tiles along each side.
pub fn door_gaps(room: &Room) -> Vec<(Side, i32, i32)> {
    border_runs(room, Tile::Door)
}

fn doors(room: &Room, d: &Dressing) -> Vec<Group> {
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let mut groups = Vec::new();
    let masonry = match d.setting {
        Setting::Dungeon(pack) => Some(pack_wall(pack)),
        Setting::Indoor | Setting::Hall | Setting::Courtyard { .. } => Some(Kind::Stone),
        Setting::Outdoor { .. } => None,
    };
    for (side, from, to) in door_gaps(room) {
        let (a, b) = (from as f32 * TILE, to as f32 * TILE);
        let id = format!("door-{}", side.word());
        let mut parts = Vec::new();
        match masonry {
            Some(stone) => {
                // The leaf just outside the wall's outer face, the lintel
                // over the way.
                parts.push(match side {
                    Side::North => vec![
                        Solid::block(Kind::Door, [a, -2.0, b, 0.0], -SLAB, DOOR + SLAB).id(&id),
                        Solid::block(stone, [a, 0.0, b, TILE], DOOR, WALL - DOOR).todo("arch"),
                    ],
                    Side::West => vec![
                        Solid::block(Kind::Door, [-2.0, a, 0.0, b], -SLAB, DOOR + SLAB).id(&id),
                        Solid::block(stone, [0.0, a, TILE, b], DOOR, WALL - DOOR).todo("arch"),
                    ],
                    Side::East => vec![
                        Solid::block(Kind::Door, [w, a, w + 2.0, b], -SLAB, DOOR + SLAB).id(&id),
                        Solid::block(stone, [w - TILE, a, w, b], DOOR, WALL - DOOR).todo("arch"),
                    ],
                    // The near wall is low: an open way out, its sill.
                    Side::South => vec![Solid::block(Kind::Door, [a, h, b, h + 2.0], -SLAB, SLAB + 1.0).id(&id)],
                });
            }
            None => {
                // A road leaves through a gap in the edge, between two posts
                // at its outer face.
                for p in [a - 3.0, b + 3.0] {
                    let (foot, tall) = match side {
                        Side::North => ([p - 1.5, 1.0, p + 1.5, 4.0], 18.0),
                        Side::South => ([p - 1.5, h - 4.0, p + 1.5, h - 1.0], 12.0),
                        Side::West => ([1.0, p - 1.5, 4.0, p + 1.5], 18.0),
                        Side::East => ([w - 4.0, p - 1.5, w - 1.0, p + 1.5], 18.0),
                    };
                    parts.push(vec![Solid::block(Kind::Wood, foot, 0.0, tall).todo("fence")]);
                }
            }
        }
        let name = match side {
            Side::North | Side::South => format!("the {} door, columns {from} to {}", side.word(), to - 1),
            _ => format!("the {} door, rows {from} to {}", side.word(), to - 1),
        };
        groups.push(Group::of_parts(Role::Door, name, parts));
    }
    groups
}

// ------------------------------------------------------------ the props ----

/// The setting's own prop for blocked tiles no dressing names.
fn default_prop(setting: Setting) -> Prop {
    match setting {
        Setting::Indoor | Setting::Hall => Prop::Crate,
        Setting::Outdoor { .. } | Setting::Courtyard { .. } => Prop::Boulder,
        Setting::Dungeon(pack) => match pack {
            Pack::Crypt => Prop::Tomb,
            Pack::Cavern => Prop::Boulder,
            Pack::Hellforge => Prop::Slag,
            Pack::Archive => Prop::Shelf,
            Pack::Fungal => Prop::Mushroom,
            Pack::Unknown => Prop::Shard,
            Pack::Swamp => Prop::Tussock,
        },
    }
}

fn props(room: &Room, d: &Dressing) -> Vec<Group> {
    let (cols, rows) = (room.cols, room.rows);
    let mut claimed = vec![false; cols * rows];
    let mut groups = Vec::new();
    let mut add = |rect: Rect, prop: Prop, claimed: &mut Vec<bool>| {
        let (c, r, w, h) = rect;
        for y in r..r + h {
            for x in c..c + w {
                if x >= 0 && y >= 0 && (x as usize) < cols && (y as usize) < rows {
                    claimed[y as usize * cols + x as usize] = true;
                }
            }
        }
        let mut group = Group::of_parts(Role::Prop, format!("{} at {c},{r} {w}x{h}", prop.word()), parts(prop, rect, room));
        group.rect = Some(rect);
        group.squash = prop.squashes();
        groups.push(group);
    };
    for &(rect, prop) in d.props {
        for sub in clipped(room, rect, prop, &claimed) {
            add(sub, prop, &mut claimed);
        }
    }
    // What no dressing names: the setting's own blocks, and a plate on
    // every stair.
    let rest: Vec<Option<Prop>> = (0..rows as i32)
        .flat_map(|r| (0..cols as i32).map(move |c| (c, r)))
        .map(|(c, r)| {
            if claimed[r as usize * cols + c as usize] {
                return None;
            }
            match room.tile(c, r) {
                Tile::Block => Some(default_prop(d.setting)),
                Tile::Stairs => Some(Prop::Plate(match d.setting {
                    Setting::Outdoor { .. } => Kind::Wood,
                    _ => Kind::Stone,
                })),
                _ => None,
            }
        })
        .collect();
    for (rect, prop) in merge(cols, rows, &rest) {
        add(rect, prop, &mut claimed);
    }
    groups
}

/// The rectangles a prop named over `rect` stands on: the rectangle itself
/// when it is all blocked tiles (or holds none, as a prop on open ground, in
/// the water or a plot's footprint does), else the blocked tiles it holds
/// that no other prop has taken, in as few rectangles as a greedy pass
/// finds. A dressing may so name a region and let its blocked tiles be what
/// they are, as a game's clearing of a doorway's fan has left them.
fn clipped(room: &Room, rect: Rect, prop: Prop, claimed: &[bool]) -> Vec<Rect> {
    if prop.on_open_ground() || matches!(prop, Prop::Site | Prop::Reeds) {
        return vec![rect];
    }
    let (cols, rows) = (room.cols, room.rows);
    let blocked = |x: i32, y: i32| room.tile(x, y) == Tile::Block && !claimed[y as usize * cols + x as usize];
    let (c, r, w, h) = rect;
    let held = (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).filter(|&(x, y)| blocked(x, y)).count();
    if held == 0 || held == (w * h) as usize {
        return vec![rect];
    }
    let cells: Vec<Option<()>> = (0..rows as i32)
        .flat_map(|y| (0..cols as i32).map(move |x| (x, y)))
        .map(|(x, y)| (inside(rect, x, y) && blocked(x, y)).then_some(()))
        .collect();
    merge(cols, rows, &cells).into_iter().map(|(rect, ())| rect).collect()
}

/// Whether any tile just north of `rect` (across its columns) is ground a
/// knight walks: then a tall thing on it sits back to the south.
fn open_north(room: &Room, (c, r, w, _): Rect) -> bool {
    (c..c + w).any(|x| walkable(room.tile(x, r - 1)))
}

/// A prop's solids in the parts the camera sizes one at a time: a pillar's
/// plinth (which shows its tiles blocked) apart from its column.
fn parts(prop: Prop, rect: Rect, room: &Room) -> Vec<Vec<Solid>> {
    let mut solids = build(prop, rect, room);
    if solids.first().is_some_and(|s| s.id.as_deref() == Some("pillar")) {
        let rest = solids.split_off(1);
        return vec![solids, rest];
    }
    vec![solids]
}

fn build(prop: Prop, rect: Rect, room: &Room) -> Vec<Solid> {
    let [x0, z0, x1, z1] = px(rect);
    let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    let word = prop.word();
    let v = hash(rect.0, rect.1, 97);
    match prop {
        Prop::Hearth => vec![
            Solid::block(Kind::Stone, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 0.0, 14.0).id(word),
            // The coals, always hot.
            Solid::block(Kind::Fire, [x0 + 12.0, z0 + 14.0, x1 - 12.0, z1 - 6.0], 14.0, 2.0),
            // The hood over them, and its chimney.
            Solid::block(Kind::Stone, [cx - 24.0, z0 + 2.0, cx + 24.0, z0 + 14.0], 14.0, 26.0).todo("frustum"),
            Solid::block(Kind::Stone, [cx - 10.0, z0 + 2.0, cx + 10.0, z0 + 10.0], 40.0, 14.0),
        ],
        Prop::Anvil if x1 - x0 < 24.0 => vec![
            Solid::block(Kind::Trunk, [cx - 3.5, z1 - 10.0, cx + 3.5, z1 - 3.0], 0.0, 6.0).todo("cylinder"),
            Solid::block(Kind::Metal, [cx - 6.0, z1 - 10.0, cx + 4.0, z1 - 4.0], 6.0, 4.0).id(word),
            // The horn.
            Solid::new(Shape::Gem, Kind::Metal, [cx + 6.5, 7.0, z1 - 7.0], [5.0, 2.5, 3.5]).todo("cone"),
        ],
        Prop::Anvil => vec![
            Solid::block(Kind::Trunk, [cx - 5.0, z1 - 11.0, cx + 5.0, z1 - 3.0], 0.0, 7.0).todo("cylinder"),
            Solid::block(Kind::Metal, [cx - 10.0, z1 - 11.0, cx + 7.0, z1 - 4.0], 7.0, 5.0).id(word),
            // The horn.
            Solid::new(Shape::Gem, Kind::Metal, [cx + 11.0, 8.0, z1 - 7.5], [8.0, 3.0, 4.0]).todo("cone"),
        ],
        Prop::Rack => {
            let mut s = vec![
                Solid::block(Kind::Wood, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z0 + 5.0], 0.0, 30.0).id(word),
                Solid::block(Kind::Wood, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 4.0], 0.0, 6.0),
            ];
            let n = ((x1 - x0) / 20.0).floor().max(1.0) as i32;
            for k in 0..n {
                let ax = x0 + (x1 - x0) * (k as f32 + 0.5) / n as f32;
                s.push(Solid::block(Kind::Metal, [ax - 6.0, z0 + 6.0, ax + 6.0, z0 + 14.0], 6.0, 13.0));
                s.push(Solid::new(Shape::Sphere, Kind::Metal, [ax, 19.0, z0 + 10.0], [9.0, 8.0, 9.0]));
            }
            s
        }
        Prop::Trough { depth, height } => {
            let (a, b) = (z1 - 2.0 - depth, z1 - 2.0);
            vec![
                Solid::block(Kind::Wood, [x0 + 2.0, a, x1 - 2.0, b], 0.0, height).id(word),
                Solid::block(Kind::Water, [x0 + 4.0, a + 2.0, x1 - 4.0, b - 2.0], height - 1.0, 1.5),
            ]
        }
        Prop::Stall { north, goods } => {
            let mut s = Vec::new();
            // The counter, its goods, the awning on four posts.
            let (counter, awning, awning_y) = if north {
                ([x0 + 2.0, z0 + 6.0, x1 - 2.0, z0 + 14.0], [x0 + 1.0, z0 + 14.0, x1 - 1.0, z1 - 1.0], 22.0)
            } else {
                ([x0 + 2.0, z1 - 12.0, x1 - 2.0, z1 - 2.0], [x0 + 1.0, z1 - 18.0, x1 - 1.0, z1 - 1.0], 22.0)
            };
            s.push(Solid::block(Kind::Wood, counter, 0.0, 12.0).id(word));
            let n = ((counter[2] - counter[0]) / 12.0).floor() as i32;
            for k in 0..n {
                let gx = counter[0] + 6.0 + k as f32 * 12.0;
                s.push(Solid::block(goods, [gx - 3.0, counter[1] + 2.0, gx + 3.0, counter[3] - 2.0], 12.0, 3.0));
            }
            for (px_, pz) in [(awning[0] + 1.0, awning[1] + 1.0), (awning[2] - 1.0, awning[1] + 1.0), (awning[0] + 1.0, awning[3] - 1.0), (awning[2] - 1.0, awning[3] - 1.0)] {
                s.push(Solid::block(Kind::Wood, [px_ - 1.0, pz - 1.0, px_ + 1.0, pz + 1.0], 0.0, awning_y));
            }
            s.push(Solid::block(Kind::Cloth, awning, awning_y, 2.0).todo("wedge"));
            s
        }
        Prop::Cross => {
            // Its steps over all its tiles, the shaft and arms on the upper.
            let z = z1 - 11.0;
            vec![
                Solid::block(Kind::Stone, [x0 + 1.0, z0 + 3.0, x1 - 1.0, z1 - 1.0], 0.0, 5.0).id(word).todo("stairs"),
                Solid::block(Kind::Stone, [x0 + 5.0, z0 + 10.0, x1 - 5.0, z1 - 5.0], 5.0, 3.0),
                Solid::block(Kind::Stone, [cx - 2.0, z - 2.0, cx + 2.0, z + 2.0], 8.0, 24.0),
                Solid::block(Kind::Stone, [cx - 8.0, z - 2.0, cx + 8.0, z + 2.0], 23.0, 4.0),
            ]
        }
        Prop::Barn { doors } => {
            let (f0, f1) = (z0 + 2.0, z1 - 2.0);
            let mut s = vec![
                Solid::block(Kind::Wood, [x0 + 1.0, f0, x1 - 1.0, f1], 0.0, 20.0).id(word),
                Solid::block(Kind::Thatch, [x0, z0 + 1.0, x1, z1 - 1.0], 20.0, 12.0).todo("gable"),
            ];
            for &(dx, dw) in doors {
                if dx >= x0 && dx + dw <= x1 {
                    s.push(Solid::block(Kind::Door, [dx, f1, dx + dw, f1 + 1.0], 0.0, 16.0));
                }
            }
            s
        }
        Prop::Arch => vec![
            Solid::block(Kind::Wood, [x0, z0 + 1.0, x0 + 2.0, z0 + 4.0], 0.0, HEADROOM),
            Solid::block(Kind::Wood, [x1 - 2.0, z0 + 1.0, x1, z0 + 4.0], 0.0, HEADROOM),
            Solid::block(Kind::Wood, [x0, z0, x1, z0 + 6.0], HEADROOM, 6.0).id(word).todo("arch"),
        ],
        Prop::Fence => {
            let mut s = Vec::new();
            let z = z0 + 11.0;
            let mut x = x0;
            while x < x1 - 1.0 {
                s.push(Solid::block(Kind::Wood, [x, z - 1.0, x + 2.0, z + 1.0], 0.0, 10.0));
                x += TILE;
            }
            s.push(Solid::block(Kind::Wood, [x1 - 2.0, z - 1.0, x1, z + 1.0], 0.0, 10.0));
            s.push(Solid::block(Kind::Wood, [x0, z - 0.5, x1, z + 0.5], 3.0, 1.5).id(word).todo("fence"));
            s.push(Solid::block(Kind::Wood, [x0, z - 0.5, x1, z + 0.5], 7.0, 1.5).todo("fence"));
            s
        }
        Prop::Cottage { walls, roof, windows } => {
            let (a, b) = (x0 + 3.0, x1 - 3.0);
            let f1 = z1 - 2.0;
            let f0 = if open_north(room, rect) { f1 - ((z1 - z0) * 0.5).max(18.0) } else { z0 + 2.0 };
            let wall_h = 18.0;
            let mut s = vec![
                Solid::block(walls, [a, f0, b, f1], 0.0, wall_h).id(word),
                Solid::block(roof, [a - 1.0, f0 - 1.0, b + 1.0, f1 + 1.0], wall_h, 12.0).todo("gable"),
                Solid::block(Kind::Door, [cx - 4.0, f1, cx + 4.0, f1 + 1.0], 0.0, 12.0),
            ];
            let n = windows as i32;
            let (left, right) = ((n + 1) / 2, n / 2);
            for (count, lo, hi) in [(left, a + 4.0, cx - 7.0), (right, cx + 7.0, b - 4.0)] {
                for k in 0..count {
                    let wx = lo + (hi - lo) * (k as f32 + 0.5) / count as f32;
                    s.push(Solid::block(Kind::Window, [wx - 3.0, f1, wx + 3.0, f1 + 1.0], 7.0, 5.0));
                }
            }
            s
        }
        Prop::Tree => {
            let (w, depth) = (x1 - x0, z1 - z0);
            // A broad crown over most of its tiles, toward the south, on a
            // short trunk at their south edge.
            let crown_d = depth * 0.6;
            let crown_z = z1 - crown_d / 2.0 - 3.0;
            vec![
                Solid::block(Kind::Trunk, [cx - 3.0, z1 - 7.0, cx + 3.0, z1 - 3.0], 0.0, 13.0).todo("cylinder"),
                Solid::new(Shape::Sphere, Kind::Foliage, [cx, 9.0, crown_z], [w * 0.8, 30.0, crown_d]).id(word),
            ]
        }
        Prop::Site => {
            let (a, b, n, s_) = (x0 + 2.0, x1 - 3.0, z0 + 6.0, z1 - 3.0);
            let mut s = Vec::new();
            for (sx, sz) in [(a, n), (b, n), (a, s_), (b, s_)] {
                s.push(Solid::block(Kind::Wood, [sx - 1.0, sz - 1.0, sx + 1.0, sz + 1.0], 0.0, 7.0));
            }
            // The surveyor's cord round the stakes.
            s.push(Solid::block(Kind::Cloth, [a, n - 0.5, b, n + 0.5], 5.0, 1.0).id(word).todo("fence"));
            s.push(Solid::block(Kind::Cloth, [a, s_ - 0.5, b, s_ + 0.5], 5.0, 1.0).todo("fence"));
            s.push(Solid::block(Kind::Cloth, [a - 0.5, n, a + 0.5, s_], 5.0, 1.0).todo("fence"));
            s.push(Solid::block(Kind::Cloth, [b - 0.5, n, b + 0.5, s_], 5.0, 1.0).todo("fence"));
            // A stack of boards and a heap of stone, waiting.
            s.push(Solid::block(Kind::Wood, [a + 5.0, s_ - 9.0, a + 21.0, s_ - 3.0], 0.0, 3.0));
            for k in 0..3 {
                let (sx, sz) = (b - 8.0 + k as f32 * 3.0, s_ - 2.0 - (k % 2) as f32 * 3.0);
                s.push(Solid::new(Shape::Gem, Kind::Rock, [sx, 0.0, sz], [5.0, 4.0, 5.0]).turned(30.0 * k as f32).todo("rock"));
            }
            // Where the plot's footprint takes in a travel plate or open
            // floor, nothing is staked on it.
            s.retain(|sol| on_blocks(room, sol));
            s
        }
        Prop::Jetty => vec![
            Solid::block(Kind::Wood, [x0, z0 - 6.0, x1, z1 + 2.0], 0.0, 2.0).id(word),
            Solid::block(Kind::Wood, [x0 + 1.0, z1, x0 + 3.0, z1 + 2.0], -SLAB, SLAB),
            Solid::block(Kind::Wood, [x1 - 3.0, z1, x1 - 1.0, z1 + 2.0], -SLAB, SLAB),
        ],
        Prop::Plate(kind) => vec![Solid::block(kind, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 1.5).id(word).todo("stairs")],
        Prop::Gate => vec![
            Solid::block(Kind::Wood, [x0 - 2.0, z0 + 2.0, x0, z0 + 5.0], 0.0, HEADROOM + 3.0),
            Solid::block(Kind::Wood, [x1, z0 + 2.0, x1 + 2.0, z0 + 5.0], 0.0, HEADROOM + 3.0),
            Solid::block(Kind::Wood, [x0 - 2.0, z0 + 2.0, x1 + 2.0, z0 + 5.0], HEADROOM, 3.0).id(word).todo("arch"),
            Solid::block(Kind::Wood, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 1.5).todo("stairs"),
        ],
        Prop::Reeds => (0..3)
            .map(|k| {
                let h = hash(rect.0 * 3 + k, rect.1, 83);
                let (rx, rz) = (x0 + 4.0 + (h % 8) as f32, z0 + 4.0 + (h >> 4 & 7) as f32);
                let s = Solid::new(Shape::Cone, Kind::Foliage, [rx, -WATER_DROP, rz], [3.0, 10.0 + (h >> 8 & 3) as f32, 3.0]);
                if k == 0 { s.id(word) } else { s }
            })
            .collect(),
        Prop::Low(kind, _) => vec![Solid::block(kind, [x0 + 1.0, z0 + 4.0, x1 - 2.0, z1 - 1.0], 0.0, 2.5).id(word)],
        Prop::Heap(kind, _) => vec![Solid::new(Shape::Sphere, kind, [cx, 0.0, cz], [x1 - x0 - 6.0, 3.5, z1 - z0 - 6.0]).id(word).todo("blob")],
        Prop::Tomb => {
            let (w, depth) = (x1 - x0, z1 - z0);
            if w >= 2.0 * TILE && depth >= 2.0 * TILE {
                // A pillar: a square column at the south of its tiles, on a
                // plinth, under a capital.
                let (pz0, pz1) = (z1 - 10.0, z1 - 3.0);
                vec![
                    Solid::block(Kind::Stone, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 5.0).id("pillar"),
                    Solid::block(Kind::Stone, [cx - 6.0, pz0, cx + 6.0, pz1], 5.0, 22.0),
                    Solid::block(Kind::Stone, [cx - 7.0, pz0 - 1.0, cx + 7.0, pz1 + 1.0], 27.0, 3.0),
                ]
            } else {
                // A sarcophagus and its lid.
                vec![
                    Solid::block(Kind::Stone, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 0.0, 9.0).id(word),
                    Solid::block(Kind::Stone, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 9.0, 3.0),
                ]
            }
        }
        Prop::Boulder => per_tile(rect, |tx, tz, h| {
            let s = 12.0 + (h % 3) as f32;
            vec![Solid::new(Shape::Sphere, Kind::Rock, [tx + (h >> 3 & 3) as f32 - 1.5, 0.0, tz], [s + 1.0, 11.0 + (h >> 5 & 3) as f32, s - 1.0]).todo("rock")]
        }, word),
        Prop::Slag => vec![
            Solid::new(Shape::Sphere, Kind::Rock, [cx, 0.0, cz], [x1 - x0 - 3.0, 12.0, z1 - z0 - 3.0]).id(word).todo("blob"),
            // Still glowing in its cracks.
            Solid::new(Shape::Gem, Kind::Fire, [cx + (v % 5) as f32 - 2.0, 8.0, cz], [6.0, 5.0, 6.0]),
        ],
        Prop::Mushroom => per_tile(rect, |tx, tz, h| {
            let stalk = 7.0 + (h % 4) as f32;
            vec![
                Solid::block(Kind::Trunk, [tx - 2.0, tz - 1.0, tx + 2.0, tz + 3.0], 0.0, stalk).todo("cylinder"),
                Solid::new(Shape::Sphere, Kind::Foliage, [tx, stalk - 2.0, tz + 1.0], [14.0, 7.0, 12.0]).todo("blob"),
            ]
        }, word),
        Prop::Tussock => per_tile(rect, |tx, tz, h| {
            let tall = 12.0 + (h % 4) as f32;
            let mut out = vec![Solid::new(Shape::Sphere, Kind::Mud, [tx, 0.0, tz + 1.0], [14.0, 6.0, 12.0]).todo("blob")];
            for k in 0..3 {
                let (dx, dz) = (k as f32 * 4.0 - 4.0 + (h >> (k * 2) & 1) as f32, (h >> (k * 3 + 1) & 3) as f32 - 1.5);
                out.push(Solid::new(Shape::Cone, Kind::Foliage, [tx + dx, 3.0, tz + dz], [3.0, tall - k as f32 * 2.0, 3.0]));
            }
            out
        }, word),
        // The Archive paints a shelf on every blocked tile.
        Prop::Shelf => per_tile(rect, |tx, tz, h| {
            vec![Solid::block(Kind::Wood, [tx - 7.5, tz - 7.5, tx + 7.5, tz + 7.5], 0.0, 24.0 + (h % 3) as f32 * 2.0)]
        }, word),
        Prop::Shard => per_tile(rect, |tx, tz, h| {
            vec![Solid::new(Shape::Gem, Kind::Stone, [tx, 0.0, tz + 2.0], [8.0, 26.0 + (h % 6) as f32, 8.0]).turned((h % 90) as f32)]
        }, word),
        Prop::Crate => vec![Solid::block(Kind::Wood, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 0.0, 12.0).id(word)],
        Prop::Tower { walls, cap, height, sails, lamp } => {
            let (a, b, c, d) = (x0 + 2.0, x1 - 2.0, z0 + 2.0, z1 - 2.0);
            let (w, depth) = (b - a, d - c);
            let mut s = vec![Solid::block(walls, [a, c, b, d], 0.0, height).id(word).todo("cylinder")];
            let mut top = height;
            if lamp {
                // The lamp room: a lit box under the cap.
                s.push(Solid::block(Kind::Fire, [cx - w * 0.3, cz - depth * 0.3, cx + w * 0.3, cz + depth * 0.3], top, 6.0));
                top += 6.0;
            }
            s.push(Solid::new(Shape::Cone, cap, [cx, top, cz], [w + 4.0, w * 0.6 + 4.0, depth + 4.0]).todo("pyramid"));
            // A slit window on the front, lit.
            let wy = (height * 0.55).max(6.0);
            s.push(Solid::block(Kind::Window, [cx - 2.0, d - 0.5, cx + 2.0, d + 1.0], wy, 6.0));
            if sails {
                // Four sails on a hub, on the front face.
                let hub = height * 0.7;
                let reach = ((w.min(depth)) * 0.5 + 6.0).min(hub - 2.0).max(8.0);
                s.push(Solid::block(Kind::Wood, [cx - 1.0, d, cx + 1.0, d + 1.5], hub - reach, reach * 2.0));
                s.push(Solid::block(Kind::Wood, [cx - reach, d, cx + reach, d + 1.5], hub - 1.0, 2.0));
                s.push(Solid::block(Kind::Cloth, [cx - reach, d + 0.5, cx - 2.0, d + 1.5], hub + 1.0, reach * 0.5));
                s.push(Solid::block(Kind::Cloth, [cx + 2.0, d + 0.5, cx + reach, d + 1.5], hub - reach * 0.5 - 1.0, reach * 0.5));
            }
            s
        }
        Prop::Pen { floor, drying } => {
            let (a, b, c, d) = (x0 + 1.0, x1 - 1.0, z0 + 1.0, z1 - 1.0);
            let mut s = vec![Solid::block(floor, [a, c, b, d], 0.0, 6.0).id(word)];
            // Posts at the corners and every tile along the edges.
            let mut xs = vec![a + 1.0];
            let mut x = x0 + TILE;
            while x < x1 - 4.0 {
                xs.push(x);
                x += TILE;
            }
            xs.push(b - 1.0);
            let mut zs = vec![c + 1.0];
            let mut z = z0 + TILE;
            while z < z1 - 4.0 {
                zs.push(z);
                z += TILE;
            }
            zs.push(d - 1.0);
            for &px_ in &xs {
                for &pz in &[c + 1.0, d - 1.0] {
                    s.push(Solid::block(Kind::Wood, [px_ - 1.0, pz - 1.0, px_ + 1.0, pz + 1.0], 0.0, 12.0));
                }
            }
            for &pz in &zs[1..zs.len() - 1] {
                for &px_ in &[a + 1.0, b - 1.0] {
                    s.push(Solid::block(Kind::Wood, [px_ - 1.0, pz - 1.0, px_ + 1.0, pz + 1.0], 0.0, 12.0));
                }
            }
            for &pz in &[c + 1.0, d - 1.0] {
                s.push(Solid::block(Kind::Wood, [a, pz - 0.5, b, pz + 0.5], 9.0, 1.5).todo("fence"));
            }
            for &px_ in &[a + 1.0, b - 1.0] {
                s.push(Solid::block(Kind::Wood, [px_ - 0.5, c, px_ + 0.5, d], 9.0, 1.5).todo("fence"));
            }
            if drying {
                // Two poles and a cord, kite cloth pegged along it.
                let zc = cz;
                for px_ in [a + 3.0, b - 3.0] {
                    s.push(Solid::block(Kind::Wood, [px_ - 1.0, zc - 1.0, px_ + 1.0, zc + 1.0], 0.0, 20.0));
                }
                let n = (((b - a) - 14.0) / 14.0).floor().max(1.0) as i32;
                let span = (b - a) - 14.0;
                for k in 0..n {
                    let fx = a + 7.0 + span * (k as f32 + 0.5) / n as f32;
                    s.push(Solid::block(Kind::Cloth, [fx - 4.0, zc - 0.5, fx + 4.0, zc + 0.5], 9.0, 7.0));
                }
            }
            s
        }
        Prop::Stand(kind, rise) => {
            let along_x = matches!(rise, Side::East | Side::West);
            let (lo, hi) = if along_x { (x0 + 1.0, x1 - 1.0) } else { (z0 + 1.0, z1 - 1.0) };
            let n = (((hi - lo) / 4.5).floor() as i32).clamp(3, 8);
            let step = (hi - lo) / n as f32;
            let mut s = Vec::new();
            for k in 0..n {
                // Tier `k`, the lowest first, at the open side.
                let (a, b) = match rise {
                    Side::North | Side::West => (hi - (k + 1) as f32 * step, hi - k as f32 * step),
                    Side::South | Side::East => (lo + k as f32 * step, lo + (k + 1) as f32 * step),
                };
                let foot = if along_x { [a, z0 + 1.0, b, z1 - 1.0] } else { [x0 + 1.0, a, x1 - 1.0, b] };
                let mut tier = Solid::block(kind, foot, 0.0, 6.0 + 3.0 * k as f32);
                if k == 0 {
                    tier = tier.id(word).todo("stairs");
                }
                s.push(tier);
            }
            s
        }
        Prop::Tent(cloth) => {
            let (w, depth) = (x1 - x0 - 2.0, z1 - z0 - 2.0);
            vec![
                Solid::block(cloth, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 9.0).id(word),
                Solid::new(Shape::Cone, cloth, [cx, 9.0, cz], [w + 2.0, 12.0, depth + 2.0]).todo("pyramid"),
                // The way in, on the front.
                Solid::block(Kind::Door, [cx - 3.0, z1 - 1.5, cx + 3.0, z1 - 0.5], 0.0, 7.0),
            ]
        }
        Prop::Outcrop { cap } => {
            let mut s = vec![Solid::block(Kind::Rock, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 7.0).id(word).todo("rock")];
            let tiles = (((x1 - x0) / TILE) as i32, ((z1 - z0) / TILE) as i32);
            for j in 0..tiles.1 {
                for i in 0..tiles.0 {
                    let h = hash(rect.0 + i, rect.1 + j, 91);
                    let (px_, pz) = (x0 + i as f32 * TILE + 8.0 + (h % 5) as f32 - 2.0, z0 + j as f32 * TILE + 8.0 + (h >> 4 & 3) as f32 - 1.0);
                    let tall = 14.0 + (h >> 8 & 7) as f32;
                    s.push(Solid::new(Shape::Gem, Kind::Rock, [px_, 5.0, pz], [13.0, tall, 12.0]).turned((h % 60) as f32).todo("rock"));
                    if let Some(cap) = cap {
                        s.push(Solid::new(Shape::Cone, cap, [px_, 5.0 + tall * 0.55, pz], [8.0, tall * 0.45, 8.0]));
                    }
                }
            }
            s
        }
        Prop::Logs => {
            let mut s = Vec::new();
            for row in 0..3 {
                let inset = 2.0 + row as f32 * 2.0;
                let mut log = Solid::block(Kind::Trunk, [x0 + inset, z0 + 3.0, x1 - inset, z1 - 3.0], row as f32 * 5.0, 5.0).todo("cylinder");
                if row == 0 {
                    log = log.id(word);
                }
                s.push(log);
            }
            // The cut ends, toward the viewer.
            let n = ((x1 - x0 - 8.0) / 9.0).floor().max(1.0) as i32;
            for k in 0..n {
                let ex = x0 + 4.0 + (x1 - x0 - 8.0) * (k as f32 + 0.5) / n as f32;
                s.push(Solid::block(Kind::Wood, [ex - 3.0, z1 - 3.0, ex + 3.0, z1 - 2.0], 0.5, 4.0));
            }
            s
        }
        Prop::Hedge => vec![Solid::block(Kind::Hedge, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 14.0).id(word)],
        Prop::Column { kind, height, lamp } => {
            let mut s = vec![
                Solid::block(kind, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], -SLAB, height + SLAB).id(word).todo("cylinder"),
                Solid::block(kind, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], height, 2.5),
            ];
            if lamp {
                s.push(Solid::new(Shape::Cone, Kind::Fire, [cx, height + 2.5, cz], [6.0, 8.0, 6.0]));
            }
            s
        }
        Prop::Rick => vec![
            Solid::block(Kind::Crop, [x0 + 1.0, z0 + 3.0, x1 - 1.0, z1 - 1.0], 0.0, 9.0).id(word),
            Solid::new(Shape::Sphere, Kind::Crop, [cx, 9.0, (z0 + z1) / 2.0 + 1.0], [x1 - x0 - 4.0, 8.0, z1 - z0 - 6.0]).todo("gable"),
        ],
        Prop::Ruin => {
            let mut s = vec![Solid::block(Kind::Stone, [x0 + 1.0, z0 + 2.0, x1 - 1.0, z1 - 1.0], 0.0, 15.0).id(word)];
            // The crenels that still stand along the front.
            let mut x = x0 + 2.0;
            while x < x1 - 6.0 {
                if hash((x as i32) / 4, rect.1, 41) % 3 != 0 {
                    s.push(Solid::block(Kind::Stone, [x, z1 - 4.0, x + 4.0, z1 - 1.0], 15.0, 5.0));
                }
                x += 6.0;
            }
            // Moss along the top, a doorway into the dark.
            s.push(Solid::block(Kind::Foliage, [x0 + 3.0, z0 + 3.0, x1 - 3.0, z0 + 6.0], 15.0, 1.0));
            s.push(Solid::block(Kind::Door, [cx - 3.0, z1 - 1.5, cx + 3.0, z1 - 0.5], 0.0, 9.0));
            s
        }
        Prop::Van => {
            let (a, b, c, d) = (x0 + 2.0, x1 - 2.0, z0 + 4.0, z1 - 4.0);
            vec![
                Solid::block(Kind::Wood, [a, c, b, d], 3.0, 9.0).id(word),
                Solid::new(Shape::Sphere, Kind::Cloth, [cx, 12.0, cz], [b - a, 10.0, d - c]).todo("gable"),
                // Under the floor of the van, the axle and its wheels.
                Solid::block(Kind::Trunk, [a + 3.0, c + 2.0, a + 5.0, d - 2.0], 0.0, 3.0),
                Solid::block(Kind::Trunk, [b - 5.0, c + 2.0, b - 3.0, d - 2.0], 0.0, 3.0),
                Solid::new(Shape::Sphere, Kind::Wood, [a + 4.0, 0.0, d + 0.5], [2.0, 7.0, 7.0]),
                Solid::new(Shape::Sphere, Kind::Wood, [b - 4.0, 0.0, d + 0.5], [2.0, 7.0, 7.0]),
            ]
        }
        Prop::Vat => vec![
            Solid::block(Kind::Wood, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 0.0, 10.0).id(word).todo("cylinder"),
            Solid::block(Kind::Water, [x0 + 4.0, z0 + 4.0, x1 - 4.0, z1 - 4.0], 10.0, 1.0),
            Solid::block(Kind::Metal, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 4.0, 1.0),
        ],
        Prop::Stage => {
            let (a, b) = (x0 + 1.0, x1 - 1.0);
            let mut s = vec![
                Solid::block(Kind::Wood, [a, z0 + 3.0, b, z1 - 1.0], 0.0, 6.0).id(word),
                // The curtained backdrop.
                Solid::block(Kind::Cloth, [a, z0 + 1.0, b, z0 + 3.0], 0.0, 22.0),
            ];
            let n = 3;
            for k in 0..n {
                let lx = a + (b - a) * (k as f32 + 0.5) / n as f32;
                s.push(Solid::block(Kind::Fire, [lx - 1.0, z1 - 2.5, lx + 1.0, z1 - 1.5], 6.0, 2.0));
            }
            s
        }
        Prop::Butts => {
            let mut s = vec![Solid::block(Kind::Grass, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 5.0).id(word)];
            let n = 3;
            for k in 0..n {
                let bx = x0 + (x1 - x0) * (k as f32 * 2.0 + 1.0) / (n as f32 * 2.0);
                s.push(Solid::block(Kind::Crop, [bx - 9.0, z1 - 14.0, bx + 9.0, z1 - 5.0], 5.0, 13.0));
            }
            s
        }
        Prop::Shed => {
            let mut s = vec![Solid::block(Kind::Wood, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 18.0, 2.5).id(word).todo("gable")];
            for (px_, pz) in [(x0 + 2.0, z0 + 2.0), (x1 - 2.0, z0 + 2.0), (x0 + 2.0, z1 - 2.0), (x1 - 2.0, z1 - 2.0)] {
                s.push(Solid::block(Kind::Wood, [px_ - 1.0, pz - 1.0, px_ + 1.0, pz + 1.0], 0.0, 18.0));
            }
            // The bench under it, and what is on it.
            s.push(Solid::block(Kind::Wood, [x0 + 4.0, z0 + 3.0, x1 - 4.0, z0 + 10.0], 0.0, 8.0));
            s.push(Solid::block(Kind::Crop, [x0 + 6.0, z0 + 4.0, x1 - 6.0, z0 + 9.0], 8.0, 1.5));
            s
        }
        Prop::Stooks => {
            let mut s = vec![Solid::block(Kind::Crop, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 5.0).id(word)];
            let tiles = (((x1 - x0) / TILE) as i32, ((z1 - z0) / TILE) as i32);
            for j in 0..tiles.1 {
                for i in 0..tiles.0 {
                    let h = hash(rect.0 + i, rect.1 + j, 87);
                    for k in 0..2 {
                        let (px_, pz) = (x0 + i as f32 * TILE + 4.0 + k as f32 * 8.0, z0 + j as f32 * TILE + 5.0 + ((h >> (k * 3)) & 3) as f32 * 2.0);
                        s.push(Solid::new(Shape::Cone, Kind::Crop, [px_, 5.0, pz], [6.0, 8.0, 6.0]).todo("pyramid"));
                    }
                }
            }
            s
        }
        Prop::Table { top, height } => vec![
            Solid::block(Kind::Wood, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, height - 1.2).id(word),
            Solid::block(top, [x0 + 0.5, z0 + 0.5, x1 - 0.5, z1 - 0.5], height - 1.2, 1.2),
        ],
        Prop::Case { body, face, height } => vec![
            Solid::block(body, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, height).id(word),
            Solid::block(face, [x0 + 2.0, z1 - 1.0, x1 - 2.0, z1 - 0.5], 3.0, height - 6.0),
        ],
        Prop::Cauldron => {
            let (w, depth) = (x1 - x0 - 4.0, z1 - z0 - 4.0);
            vec![
                Solid::block(Kind::Stone, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 0.0, 6.0).id(word),
                Solid::new(Shape::Sphere, Kind::Metal, [cx, 6.0, cz], [w - 2.0, 9.0, depth - 2.0]),
                // The fire under the pot, showing at its front.
                Solid::new(Shape::Cone, Kind::Fire, [cx, 0.5, z1 - 3.0], [7.0, 5.0, 4.0]),
            ]
        }
        Prop::Fountain => vec![
            Solid::block(Kind::Stone, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 0.0, 5.0).id(word),
            Solid::block(Kind::Water, [x0 + 4.0, z0 + 4.0, x1 - 4.0, z1 - 4.0], 5.0, 0.8),
            Solid::block(Kind::Stone, [cx - 2.0, cz - 2.0, cx + 2.0, cz + 2.0], 5.0, 13.0).todo("cylinder"),
            Solid::new(Shape::Sphere, Kind::Water, [cx, 18.0, cz], [6.0, 4.0, 6.0]),
        ],
        Prop::Furnace { lit } => {
            let mouth = (x1 - x0 - 8.0).clamp(8.0, 20.0);
            vec![
                Solid::block(Kind::Stone, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, 22.0).id(word),
                // The hood and its flue.
                Solid::block(Kind::Stone, [cx - (x1 - x0) * 0.3, z0 + 1.0, cx + (x1 - x0) * 0.3, z0 + 8.0], 22.0, 10.0).todo("frustum"),
                Solid::block(Kind::Stone, [cx - 3.0, z0 + 1.0, cx + 3.0, z0 + 6.0], 32.0, 12.0),
                // The mouth: black while cold, the brightest thing in the room lit.
                Solid::block(if lit { Kind::Fire } else { Kind::Metal }, [cx - mouth / 2.0, z1 - 1.5, cx + mouth / 2.0, z1 - 0.5], 3.0, 10.0),
            ]
        }
        Prop::Dais => {
            let mut s = vec![
                Solid::block(Kind::Stone, [x0 + 0.5, z0 + 0.5, x1 - 0.5, z1 - 0.5], 0.0, 3.0).id(word).todo("stairs"),
                Solid::block(Kind::Stone, [x0 + 4.0, z0 + 3.0, x1 - 4.0, z1 - 5.0], 3.0, 3.0),
                // The seat of dressed stone, and the banner on the wall behind.
                Solid::block(Kind::Stone, [cx - 6.0, z0 + 4.0, cx + 6.0, z0 + 13.0], 6.0, 13.0),
            ];
            if z0 <= TILE + 0.5 {
                s.push(Solid::block(Kind::Cloth, [cx - 8.0, z0 - 2.0, cx + 8.0, z0 + 1.0], 14.0, 28.0));
            }
            s
        }
        Prop::Well => vec![
            Solid::block(Kind::Stone, [x0 + 2.0, z0 + 2.0, x1 - 2.0, z1 - 2.0], 0.0, 6.0).id(word).todo("cylinder"),
            Solid::block(Kind::Water, [x0 + 6.0, z0 + 6.0, x1 - 6.0, z1 - 6.0], 6.0, 0.5),
            Solid::block(Kind::Wood, [x0 + 3.0, cz - 1.0, x0 + 5.0, cz + 1.0], 6.0, 14.0),
            Solid::block(Kind::Wood, [x1 - 5.0, cz - 1.0, x1 - 3.0, cz + 1.0], 6.0, 14.0),
            Solid::block(Kind::Roof, [x0 + 2.0, z0 + 6.0, x1 - 2.0, z1 - 6.0], 20.0, 4.0).todo("gable"),
        ],
        Prop::Deck(kind) => {
            let mut s = vec![Solid::block(kind, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, if kind == Kind::Rug { 0.6 } else { 1.5 }).id(word)];
            if kind == Kind::Wood {
                // On piles.
                for (px_, pz) in [(x0 + 2.0, z0 + 2.0), (x1 - 2.0, z0 + 2.0), (x0 + 2.0, z1 - 2.0), (x1 - 2.0, z1 - 2.0)] {
                    s.push(Solid::block(Kind::Wood, [px_ - 1.0, pz - 1.0, px_ + 1.0, pz + 1.0], -SLAB, SLAB));
                }
            }
            s
        }
        Prop::Rail => {
            let mut s = Vec::new();
            let z = z0 + 11.0;
            let mut x = x0;
            while x < x1 - 1.0 {
                s.push(Solid::block(Kind::Wood, [x, z - 1.0, x + 2.0, z + 1.0], 0.0, 10.0));
                x += TILE;
            }
            s.push(Solid::block(Kind::Wood, [x1 - 2.0, z - 1.0, x1, z + 1.0], 0.0, 10.0));
            s.push(Solid::block(Kind::Wood, [x0, z - 1.0, x1, z + 1.0], 2.0, 3.2).id(word).todo("fence"));
            s.push(Solid::block(Kind::Wood, [x0, z - 0.5, x1, z + 0.5], 7.0, 1.5).todo("fence"));
            s
        }
        Prop::Shaft => vec![
            Solid::block(Kind::Stone, [x0 + 1.5, z0 + 1.5, x1 - 1.5, z1 - 1.5], 0.0, 7.0).id(word).todo("cylinder"),
            Solid::block(Kind::Water, [x0 + 5.0, z0 + 5.0, x1 - 5.0, z1 - 5.0], 7.0, 0.5),
        ],
        Prop::Plinth { kind, height } => vec![Solid::block(kind, [x0 + 1.0, z0 + 1.0, x1 - 1.0, z1 - 1.0], 0.0, height).id(word)],
        Prop::Wheel => {
            let (a, b) = (x0 + 1.0, x1 - 1.0);
            vec![
                // The stand's podium, its A-frame, the wheel on it facing the
                // room with its pointer, the drapes.
                Solid::block(Kind::Wood, [a, z0 + 2.0, b, z1 - 1.0], 0.0, 8.0).id(word),
                Solid::block(Kind::Wood, [cx - 21.0, z1 - 6.0, cx - 18.0, z1 - 3.0], 8.0, 14.0),
                Solid::block(Kind::Wood, [cx + 18.0, z1 - 6.0, cx + 21.0, z1 - 3.0], 8.0, 14.0),
                Solid::new(Shape::Sphere, Kind::Wood, [cx, 8.0, z1 - 6.0], [40.0, 40.0, 4.0]),
                Solid::block(Kind::Window, [cx - 1.5, z1 - 8.0, cx + 1.5, z1 - 4.0], 46.0, 4.0),
                Solid::block(Kind::Cloth, [a, z0 + 2.0, a + 6.0, z0 + 5.0], 8.0, 36.0),
                Solid::block(Kind::Cloth, [b - 6.0, z0 + 2.0, b, z0 + 5.0], 8.0, 36.0),
            ]
        }
        Prop::Telescope => vec![
            Solid::block(Kind::Metal, [x0 + 3.0, z0 + 3.0, x1 - 3.0, z1 - 3.0], 0.0, 5.0).id(word),
            Solid::block(Kind::Metal, [cx - 2.0, cz - 2.0, cx + 2.0, cz + 2.0], 5.0, 7.0),
            Solid::block(Kind::Metal, [cx - 14.0, cz - 2.5, cx + 14.0, cz + 2.5], 12.0, 5.0).turned(25.0),
            Solid::new(Shape::Sphere, Kind::Metal, [cx + 12.0, 12.0, cz - 5.0], [6.0, 7.0, 6.0]),
        ],
        Prop::Flags => {
            let mut s = vec![
                Solid::block(Kind::Wood, [x0 + 1.0, z0 + 3.0, x1 - 1.0, z1 - 2.0], 0.0, 16.0).id(word),
                Solid::block(Kind::Wood, [x0, z0 + 2.0, x1, z1 - 1.0], 16.0, 2.0).todo("gable"),
            ];
            let n = (((x1 - x0) - 8.0) / 12.0).floor().max(2.0) as i32;
            for k in 0..n {
                let fx = x0 + 6.0 + (x1 - x0 - 12.0) * k as f32 / (n - 1) as f32;
                s.push(Solid::block(Kind::Wood, [fx - 0.75, cz - 0.75, fx + 0.75, cz + 0.75], 18.0, 14.0));
                s.push(Solid::block(Kind::Cloth, [fx + 0.75, cz - 0.5, fx + 6.0, cz + 0.5], 25.0, 6.0));
            }
            s
        }
    }
}

/// Whether every tile a solid's footprint touches is a blocked one.
fn on_blocks(room: &Room, s: &Solid) -> bool {
    let [x0, z0, x1, z1] = s.footprint();
    let (c0, c1) = (((x0 + 0.5) / TILE).floor() as i32, ((x1 - 0.5) / TILE).floor() as i32);
    let (r0, r1) = (((z0 + 0.5) / TILE).floor() as i32, ((z1 - 0.5) / TILE).floor() as i32);
    (r0..=r1).all(|r| (c0..=c1).all(|c| room.tile(c, r) == Tile::Block))
}

/// Something to each tile of `rect`, from its middle and a hash of it; the
/// first solid named `word`.
fn per_tile(rect: Rect, mut each: impl FnMut(f32, f32, u32) -> Vec<Solid>, word: &str) -> Vec<Solid> {
    let (c, r, w, h) = rect;
    let mut out = Vec::new();
    for y in r..r + h {
        for x in c..c + w {
            out.extend(each(x as f32 * TILE + 8.0, y as f32 * TILE + 8.0, hash(x, y, 89)));
        }
    }
    if let Some(first) = out.first_mut() {
        first.id = Some(word.to_string());
    }
    out
}

/// Every id in a set once: a word used by more than one prop is numbered.
pub(crate) fn name_ids(groups: &mut [Group]) {
    let mut count: HashMap<String, usize> = HashMap::new();
    for s in groups.iter().flat_map(|g| &g.solids) {
        if let Some(id) = &s.id {
            *count.entry(id.clone()).or_default() += 1;
        }
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    for s in groups.iter_mut().flat_map(|g| &mut g.solids) {
        if let Some(id) = &mut s.id
            && count[id.as_str()] > 1
        {
            let n = seen.entry(id.clone()).or_default();
            *n += 1;
            *id = format!("{id}-{n}");
        }
    }
}

// ---------------------------------------------------------- the landmark ----

/// The crate's own asset folder, as it was built: where the landmark
/// overlays are written and the generator's tests read them. A portable
/// build (`ANGEL_PORTABLE_BUILD=1`, the release packages) names no build
/// folder, so it reads the package's own: `ANGEL_RESOURCE_DIR`, which the
/// launcher sets, else the first folder above the binary that holds it.
pub fn landmark_folder() -> std::path::PathBuf {
    const OWN: &str = "crates/delve/assets/world3d";
    if let Some(dir) = option_env!("ANGEL_SOURCE_DELVE_DIR") {
        return std::path::Path::new(dir).join("assets/world3d");
    }
    static PACKAGED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PACKAGED
        .get_or_init(|| {
            if let Some(root) = std::env::var_os("ANGEL_RESOURCE_DIR").filter(|v| !v.is_empty()) {
                return std::path::PathBuf::from(root).join(OWN);
            }
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.ancestors().skip(1).take(6).map(|a| a.join(OWN)).find(|f| f.is_dir()))
                .unwrap_or_else(|| std::path::PathBuf::from(OWN))
        })
        .clone()
}

/// Where an item's landmark overlay lives: `assets/world3d/<item>/landmark.set`.
pub fn landmark_path(id: &str) -> std::path::PathBuf {
    landmark_folder().join(id).join("landmark.set")
}

/// The landmark overlay the asset folder `folder` holds for item `id`
/// (`<folder>/<id>/landmark.set`), when it has one.
pub fn landmark_in(folder: &std::path::Path, id: &str) -> Option<String> {
    std::fs::read_to_string(folder.join(id).join("landmark.set")).ok()
}

/// The item's landmark overlay, when it has one: shape lines of the set
/// format (no `set`, `camera` or `plate` line) that the generator appends to
/// the item's generated set. It is where a place's signature structure (a
/// windmill's tower and sails, an aqueduct's arches) is placed by hand on the
/// room's blocked tiles, as the generator cannot know it.
pub fn landmark(id: &str) -> Option<String> {
    std::fs::read_to_string(landmark_path(id)).ok()
}

/// A landmark overlay's blocks, or every line of it that is refused. It is
/// read by the set format's own parser (`angel_stage::set::Set::parse`: the
/// built-in shapes and the primitive library, their `-<n>` suffixes, every
/// kind), held to shape lines only, each with a size on every axis. The
/// hulls know the built-ins the generator uses; a primitive of the library
/// is held to its bounding block, which hides at least as much.
pub fn landmark_solids(text: &str) -> Result<Vec<Solid>, Vec<String>> {
    // The overlay under a set's head, so its lines keep their numbers past it.
    const HEAD: &str = "set landmark\ncamera room 1 1\n";
    const HEAD_LINES: usize = 2;
    let mut refused: Vec<(usize, String)> = Vec::new();
    let mut body = String::from(HEAD);
    for (n, raw) in text.lines().enumerate() {
        match raw.split('#').next().unwrap_or("").split_whitespace().next() {
            Some(word @ ("set" | "camera" | "plate")) => {
                refused.push((n + 1, format!("a landmark carries shape lines only, not `{word}`")));
            }
            _ => body.push_str(raw),
        }
        body.push('\n');
    }
    let shapes = match angel_stage::set::Set::parse(&body) {
        Ok(set) => set.shapes,
        Err(refusals) => {
            refused.extend(refusals.into_iter().filter(|r| r.line > HEAD_LINES).map(|r| (r.line - HEAD_LINES, r.why)));
            Vec::new()
        }
    };
    let mut solids = Vec::with_capacity(shapes.len());
    for s in shapes {
        if s.size.iter().any(|&v| v <= 0.0) {
            refused.push((s.line - HEAD_LINES, "a landmark's block has a size on every axis".into()));
            continue;
        }
        let shape = match s.shape.split('-').next().unwrap_or("") {
            "sphere" => Shape::Sphere,
            "gem" => Shape::Gem,
            "cone" => Shape::Cone,
            _ => Shape::Box,
        };
        let mut solid = Solid::new(shape, s.kind, s.at, s.size);
        solid.turn = s.turn;
        solid.id = s.id;
        solids.push(solid);
    }
    if refused.is_empty() {
        return Ok(solids);
    }
    refused.sort_by_key(|(line, _)| *line);
    Err(refused.into_iter().map(|(line, why)| format!("landmark line {line}: {why}")).collect())
}

/// How the overlay `landmark` breaks the room's agreement with the game, in
/// words; none when it keeps it. It is held to what the generator holds
/// itself to:
///
/// - what stands on the floor (it begins lower than [`HEADROOM`]) stands
///   only on tiles a knight cannot walk: walls, blocks and hazards;
/// - it hides no more than [`HIDE_ONE`] of any walkable tile from the game's
///   camera by itself, nor more than [`HIDE_ALL`] with everything the
///   generator placed ([`blockout`]'s `groups`);
/// - it keeps to the plate, with the margin the capture leaves round the
///   stage, and its ids are its own.
pub fn landmark_faults(room: &Room, groups: &[Group], landmark: &[Solid]) -> Vec<String> {
    let mut faults = Vec::new();
    for s in landmark.iter().filter(|s| s.bottom() < HEADROOM) {
        let [x0, z0, x1, z1] = s.footprint();
        let (c0, c1) = (((x0 + 0.5) / TILE).floor() as i32, ((x1 - 0.5) / TILE).floor() as i32);
        let (r0, r1) = (((z0 + 0.5) / TILE).floor() as i32, ((z1 - 0.5) / TILE).floor() as i32);
        let open: Vec<(i32, i32)> = (r0..=r1)
            .flat_map(|r| (c0..=c1).map(move |c| (c, r)))
            .filter(|&(c, r)| walkable(room.tile(c, r)))
            .collect();
        if !open.is_empty() {
            faults.push(format!("`{}` stands over walkable tiles {open:?}", line(s)));
        }
    }
    let view = View::of(room);
    let open = open_tiles(room);
    let own = occluders(&view, landmark);
    let others = occluders(
        &view,
        groups.iter().filter(|g| g.role != Role::Ground && g.role != Role::Hazard).flat_map(|g| &g.solids),
    );
    for &t in &open {
        let alone = view.hidden(&own, t);
        if alone > HIDE_ONE {
            faults.push(format!("the landmark hides {:.0}% of walkable tile {t:?}", alone * 100.0));
        }
        let all: Vec<&Hull> = others.iter().chain(&own).collect();
        let together = view.hidden_by(&all, t);
        if together > HIDE_ALL {
            faults.push(format!("the landmark and the room hide {:.0}% of walkable tile {t:?}", together * 100.0));
        }
    }
    // The plate holds the stage and its margin.
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let room_for = 1.0 - 2.0 * angel_stage::set::PLATE_MARGIN;
    let scale = (PLATE.0 as f32 * room_for / w).min(PLATE.1 as f32 * room_for / h);
    let (half_w, half_h) = (PLATE.0 as f32 / scale / 2.0, PLATE.1 as f32 / scale / 2.0);
    for s in landmark {
        let out = s.outline().into_iter().any(|[x, y, z]| {
            let q = view.screen([x, y.max(0.0), z]);
            (q[0] - w / 2.0).abs() > half_w || (q[1] - h / 2.0).abs() > half_h
        });
        if out {
            faults.push(format!("`{}` rises out of the plate", line(s)));
        }
    }
    let taken: Vec<&String> = groups.iter().flat_map(|g| &g.solids).filter_map(|s| s.id.as_ref()).collect();
    for (k, s) in landmark.iter().enumerate() {
        if let Some(id) = &s.id
            && (taken.contains(&id) || landmark[..k].iter().any(|o| o.id.as_ref() == Some(id)))
        {
            faults.push(format!("id `{id}` is already the set's"));
        }
    }
    faults
}

// ------------------------------------------------------------- the text ----

/// A number as a set writes it: whole when it is, else to the hundredth.
pub fn num(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        let s = format!("{r:.2}");
        s.trim_end_matches('0').to_string()
    }
}

/// A solid as a line of a set.
pub fn line(s: &Solid) -> String {
    let mut l = format!(
        "{:<6} {:<7} at {} {} {} size {} {} {}",
        s.shape.written(),
        s.kind.name(),
        num(s.at[0]),
        num(s.at[1]),
        num(s.at[2]),
        num(s.size[0]),
        num(s.size[1]),
        num(s.size[2]),
    );
    if s.turn != 0.0 {
        let _ = write!(l, " turn {}", num(s.turn));
    }
    if let Some(id) = &s.id {
        let _ = write!(l, " id {id}");
    }
    if let Some(prim) = s.todo {
        let _ = write!(l, "  # TODO prims: {prim}");
    }
    l
}

fn write(room: &Room, d: &Dressing, groups: &[Group]) -> String {
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let mut out = String::new();
    let _ = writeln!(out, "set {}", d.id);
    let _ = writeln!(out, "# {}: {} x {} tiles of {} stage pixels.", d.title, room.cols, room.rows, TILE);
    let _ = writeln!(out, "# Made from the game's own layout by angel-delve world3d::blockout; written by the");
    let _ = writeln!(out, "# write_world3d_sets test (ANGEL_WORLD3D_SETS=1). Change the generator, not this file.");
    let _ = writeln!(out, "camera room {} {}", num(w), num(h));
    let _ = writeln!(out, "plate {} {}", PLATE.0, PLATE.1);
    for note in d.notes {
        let _ = writeln!(out, "# {note}");
    }
    for g in groups {
        if g.solids.is_empty() && g.role != Role::Hazard {
            continue;
        }
        let _ = writeln!(out);
        if g.kept < 1.0 {
            let _ = writeln!(out, "# {} (at {}% of its height, for the camera)", g.name, (g.kept * 100.0).floor());
        } else {
            let _ = writeln!(out, "# {}", g.name);
        }
        for s in &g.solids {
            let _ = writeln!(out, "{}", line(s));
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/world3d/blockout__tests.rs"]
mod tests;
