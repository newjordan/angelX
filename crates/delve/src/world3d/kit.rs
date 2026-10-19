//! The building kit: parametric buildings made of the primitive shapes, each
//! a few set lines for a footprint (docs/WORLD-3D.md). The kingdom's chunks
//! (`chunks`) stand one on every place, castle and structure of the map,
//! chosen by what it is and sized to its tiles.
//!
//! A kit piece is built in its own frame, `w` across its front and `d` deep,
//! `x` from its left to its right as one faces it, `z` from its back (0) to
//! its front (`d`), `y` up; [`build`] turns the frame so the front faces the
//! door's side ([`Face`]) and moves it onto the footprint. A piece is one or
//! more parts (a building, a tower, a wagon): each part is sized for the
//! camera on its own (`blockout::trim`), so what stands on something is in
//! its part. Everything keeps within the footprint, so open ground round a
//! building stays open.
//!
//! Wall kinds and roof kinds (roof, thatch, slate) are seeded per building.
//! Light comes only from windows and the lighthouse's lantern; no bright
//! kind is decoration.

use super::blockout::{Kind, Shape, Solid};
use crate::render::overworld::ink::hash;

/// The side of its footprint a building's front (its door) is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    South,
    East,
    North,
    West,
}

impl Face {
    /// The turn that takes a piece's own frame (front toward `+z`) to it.
    fn turn(self) -> f32 {
        match self {
            Face::South => 0.0,
            Face::East => 90.0,
            Face::North => 180.0,
            Face::West => 270.0,
        }
    }
}

/// What tops a castle's towers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Top {
    /// Battlements: a parapet and merlons.
    Crenel,
    /// A dome of this kind.
    Dome(Kind),
    /// A cone roof of this kind.
    Cone(Kind),
}

/// A castle of the March, as its house builds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Castle {
    pub walls: Kind,
    pub top: Top,
    /// Corner towers: 2 at the front, or 4.
    pub towers: u8,
}

/// What the kit builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kit {
    Cottage,
    Barn,
    Hall,
    /// A cottage with a forge's open bay and a tall stack.
    Forge,
    /// A working shed: a small house and a lean-to on posts.
    Workshop,
    /// A round tower with a cone roof.
    Tower,
    /// A slim square turret against the keep, crenellated.
    Turret,
    /// A tall square tower of roosts.
    Rookery,
    /// Two towers and an arch over the way through.
    Gatehouse,
    Castle(Castle),
    /// The King's keep: a great square tower with corner bartizans.
    Keep,
    Windmill,
    Watermill,
    /// A market stall: a counter under a dark cloth awning.
    Stall,
    Well,
    Lighthouse,
    Chapel,
    /// A chapel and its cloister.
    Priory,
    /// A round hall under a cone roof.
    RoundTable,
    /// A round tower under a dome.
    Observatory,
    /// A timbered adit in a heap of rock.
    MineMouth,
    /// A dark, broken keep among rubble.
    DragonKeep,
    /// A ring of arched walls round a sand floor.
    Colosseum,
    /// A round tent of cloth.
    Pavilion,
    /// A tilting post with its arm.
    Quintain,
    /// The standings board on its posts.
    Board,
    /// Planted beds in a low hedge.
    Garden,
    /// The School of Magic: a hall with a spired tower.
    School,
    /// Labyrinth College: a long hall between towers, a gate tower.
    College,
    /// The Delve's gate: an arch into a heap of rock.
    DelveGate,
    /// Tiered stands under an awning before a track.
    Hippodrome,
    /// A row of arches carrying a water channel.
    Aqueduct,
    /// A bell tower.
    Belfry,
    /// A small stone hut under thatch.
    Bothy,
    /// A broken watchtower and wall stubs.
    Ruins,
    /// A heap of crags.
    Crag,
    /// A stepped rock face.
    Cliffs,
    /// A crag with a great nest on it.
    Eyrie,
    /// A hut and a dovecote.
    Aviary,
    /// A hall under a dome.
    Bathhouse,
    /// A hall with a pillared porch.
    LandOffice,
    /// A barn and a paddock.
    Ranch,
    /// A low stable.
    Stable,
    /// Rows of small fruit trees.
    Orchard,
    /// Hedge rows in a ring.
    HedgeMaze,
    /// Log stacks and a saw shed.
    Timberyard,
    /// Salt pans and a shed.
    Saltworks,
    /// Upturned boats and net posts.
    Strand,
    /// A drowned chapel's broken walls.
    SunkenAbbey,
    /// A stone quay, a crane, crates.
    Quay,
    /// A raised stone way with posts.
    Causeway,
    /// A toll hut and its barrier.
    TollBridge,
    /// Stone piers and a timber gate over a channel.
    Sluice,
    /// Flower beds in rows, a shed.
    Flowerbeds,
    /// A dairy hut and hay bales.
    Downs,
    /// A cairn and flag poles.
    Flags,
    /// Covered wagons.
    Caravan,
    /// A workshop and its wheels.
    Wheelwright,
    /// Archery butts on stands.
    Butts,
    /// A field of beds and a scarecrow.
    Scarecrow,
    /// A workshop and arrow racks.
    Fletchery,
    /// A penned green, a hut, a pond.
    GoosePen,
    /// Fair booths.
    Fair,
    /// A players' stage.
    Stage,
    /// Wash vats, drying lines, a hut.
    Laundry,
    /// A cottage and kites on poles.
    Kites,
    /// A ward's hall and its banner.
    Ward,
    // The run's finds out in the open country (`world::finds`), each a
    // small piece on its own tile, and the wishes nearest them.
    /// A wayside shrine: a little stone house on a step, a niche in its front.
    Shrine,
    /// Standing stones in an arc.
    Stones,
    /// A cave mouth in a heap of rock.
    Cave,
    /// Logs crossed under a low fire.
    Campfire,
    /// A pack and a bedroll against a staff.
    Pack,
    /// A lean-to of turf on poles, its dark door at the front.
    LeanTo,
    /// Broken wall stubs among rubble.
    Stubs,
}

impl Kit {
    /// What it is, for a painter.
    pub fn what(self) -> &'static str {
        match self {
            Kit::Cottage => "a cottage",
            Kit::Barn => "a barn",
            Kit::Hall => "a hall",
            Kit::Forge => "a forge with its open bay and stack",
            Kit::Workshop => "a workshop and lean-to",
            Kit::Tower => "a round tower",
            Kit::Turret => "a keep turret",
            Kit::Rookery => "a tall roost tower",
            Kit::Gatehouse => "a gatehouse arch between towers",
            Kit::Castle(_) => "a castle: curtain walls, towers, a keep and a gate",
            Kit::Keep => "the great keep",
            Kit::Windmill => "a windmill",
            Kit::Watermill => "a watermill and its wheel",
            Kit::Stall => "market stalls under dark awnings",
            Kit::Well => "a well",
            Kit::Lighthouse => "a lighthouse with a lit lantern",
            Kit::Chapel => "a chapel with a spire",
            Kit::Priory => "a priory: chapel and cloister",
            Kit::RoundTable => "the round hall of the Round Table",
            Kit::Observatory => "a domed observatory tower",
            Kit::MineMouth => "a timbered mine mouth in the rock",
            Kit::DragonKeep => "a dark broken keep among rubble",
            Kit::Colosseum => "an arena ring of arched walls",
            Kit::Pavilion => "a tournament tent",
            Kit::Quintain => "a quintain post",
            Kit::Board => "the standings board",
            Kit::Garden => "planted beds in a low hedge",
            Kit::School => "a school hall with a spired tower",
            Kit::College => "a long college hall between towers",
            Kit::DelveGate => "the Delve's arch into the rock",
            Kit::Hippodrome => "tiered stands before a track",
            Kit::Aqueduct => "aqueduct arches",
            Kit::Belfry => "a bell tower",
            Kit::Bothy => "a stone bothy",
            Kit::Ruins => "a ruined watchtower",
            Kit::Crag => "a heap of crags",
            Kit::Cliffs => "a stepped cliff",
            Kit::Eyrie => "a crag with a griffin's nest",
            Kit::Aviary => "a dovecote and hut",
            Kit::Bathhouse => "a domed bathhouse",
            Kit::LandOffice => "a hall with a pillared porch",
            Kit::Ranch => "a barn and paddock",
            Kit::Stable => "a low stable",
            Kit::Orchard => "rows of fruit trees",
            Kit::HedgeMaze => "a hedge maze",
            Kit::Timberyard => "log stacks and a saw shed",
            Kit::Saltworks => "salt pans and a shed",
            Kit::Strand => "upturned boats and net posts",
            Kit::SunkenAbbey => "a drowned abbey's broken walls",
            Kit::Quay => "a stone quay with a crane",
            Kit::Causeway => "a raised stone causeway",
            Kit::TollBridge => "a toll hut and barrier",
            Kit::Sluice => "a sluice gate between stone piers",
            Kit::Flowerbeds => "flower beds and a shed",
            Kit::Downs => "a dairy hut and hay bales",
            Kit::Flags => "a cairn and flag poles",
            Kit::Caravan => "covered wagons",
            Kit::Wheelwright => "a wheelwright's shop",
            Kit::Butts => "archery butts",
            Kit::Scarecrow => "a field and its scarecrow",
            Kit::Fletchery => "a fletcher's shop and racks",
            Kit::GoosePen => "a penned green and pond",
            Kit::Fair => "fair booths",
            Kit::Stage => "a players' stage",
            Kit::Laundry => "wash vats and drying lines",
            Kit::Kites => "a kite maker's cottage",
            Kit::Ward => "a ward hall with its banner",
            Kit::Shrine => "a wayside shrine",
            Kit::Stones => "standing stones",
            Kit::Cave => "a cave mouth in the rock",
            Kit::Campfire => "logs round a campfire",
            Kit::Pack => "a lost pack against a staff",
            Kit::LeanTo => "a hermit's lean-to",
            Kit::Stubs => "broken wall stubs",
        }
    }

    /// Whether it has windows a lamp lights at night.
    pub fn lit(self) -> bool {
        !matches!(
            self,
            Kit::Stall
                | Kit::Well
                | Kit::Quintain
                | Kit::Board
                | Kit::Garden
                | Kit::Crag
                | Kit::Cliffs
                | Kit::Eyrie
                | Kit::Ruins
                | Kit::Orchard
                | Kit::HedgeMaze
                | Kit::Aqueduct
                | Kit::Causeway
                | Kit::Flags
                | Kit::Butts
                | Kit::Scarecrow
                | Kit::Pavilion
                | Kit::Caravan
                | Kit::Colosseum
                | Kit::DragonKeep
                | Kit::SunkenAbbey
                | Kit::MineMouth
                | Kit::DelveGate
                | Kit::Turret
                | Kit::Fair
                | Kit::Stage
                | Kit::Hippodrome
                | Kit::Strand
                | Kit::Sluice
                | Kit::Saltworks
                | Kit::Timberyard
                | Kit::Laundry
                | Kit::GoosePen
                | Kit::Flowerbeds
                | Kit::Shrine
                | Kit::Stones
                | Kit::Cave
                | Kit::Campfire
                | Kit::Pack
                | Kit::LeanTo
                | Kit::Stubs
        )
    }
}

/// Where a piece stands: its footprint `[x0, z0, x1, z1]` in stage pixels,
/// the side its front is on, and its seed.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub foot: [f32; 4],
    pub face: Face,
    pub seed: u32,
}

/// The piece `kit` builds on `spot`: its parts, in the world.
pub fn build(kit: Kit, spot: &Spot) -> Vec<Vec<Solid>> {
    let [x0, z0, x1, z1] = spot.foot;
    let (fw, fd) = (x1 - x0, z1 - z0);
    let (w, d) = match spot.face {
        Face::South | Face::North => (fw, fd),
        Face::East | Face::West => (fd, fw),
    };
    let mut b = B::new(w, d, spot.seed);
    piece(kit, &mut b);
    b.cut();
    let turn = spot.face.turn();
    let (s, c) = turn.to_radians().sin_cos();
    let (s, c) = (s.round(), c.round());
    let (wx, wz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    b.parts
        .into_iter()
        .map(|part| {
            part.into_iter()
                .map(|mut solid| {
                    let (lx, lz) = (solid.at[0] - w / 2.0, solid.at[2] - d / 2.0);
                    solid.at[0] = wx + lx * c + lz * s;
                    solid.at[2] = wz - lx * s + lz * c;
                    orient(&mut solid, turn);
                    solid
                })
                .collect()
        })
        .collect()
}

/// A solid turned by `turn` more degrees (a whole number of quarters): a
/// shape that is the same every quarter turn over its footprint takes it
/// as its sides swapped; the rest keep it, less the half turns a shape the
/// same both ways does not show.
fn orient(solid: &mut Solid, turn: f32) {
    let quarter = ((turn / 90.0).round() as i32).rem_euclid(4);
    let square = matches!(
        solid.shape,
        Shape::Box | Shape::Sphere | Shape::Cone | Shape::Cylinder | Shape::Frustum | Shape::Pyramid | Shape::Gem | Shape::Rock(_) | Shape::Blob(_)
    );
    if square && solid.turn == 0.0 {
        if quarter % 2 == 1 {
            solid.size.swap(0, 2);
        }
        return;
    }
    let half = if matches!(solid.shape, Shape::Gable | Shape::Hip | Shape::Arch | Shape::Fence(_)) { 180.0 } else { 360.0 };
    let total = (solid.turn + quarter as f32 * 90.0).rem_euclid(half);
    solid.turn = if total.abs() < 1e-3 || (half - total).abs() < 1e-3 { 0.0 } else { total };
}

// ---------------------------------------------------------- the builder ----

/// A piece being built in its own frame.
struct B {
    w: f32,
    d: f32,
    seed: u32,
    parts: Vec<Vec<Solid>>,
    cur: Vec<Solid>,
}

impl B {
    fn new(w: f32, d: f32, seed: u32) -> Self {
        Self { w, d, seed, parts: Vec::new(), cur: Vec::new() }
    }

    fn add(&mut self, s: Solid) {
        self.cur.push(s);
    }

    /// End a part: what follows is sized for the camera on its own.
    fn cut(&mut self) {
        if !self.cur.is_empty() {
            self.parts.push(std::mem::take(&mut self.cur));
        }
    }

    /// A number from the seed and `k`.
    fn r(&self, k: u32) -> u32 {
        hash(self.seed as i32, k as i32, 4111)
    }

    /// One of `items`, by the seed and `k`.
    fn pick<T: Copy>(&self, k: u32, items: &[T]) -> T {
        items[self.r(k) as usize % items.len()]
    }

    /// A number in `lo..=hi`, by the seed and `k`.
    fn range(&self, k: u32, lo: f32, hi: f32) -> f32 {
        lo + (self.r(k) % 1000) as f32 / 999.0 * (hi - lo)
    }

    fn walls(&self) -> Kind {
        self.pick(1, &[Kind::Stone, Kind::Plaster, Kind::Wood, Kind::Stone, Kind::Brick, Kind::Plaster])
    }

    fn roof(&self) -> Kind {
        self.pick(2, &[Kind::Roof, Kind::Thatch, Kind::Slate])
    }
}

fn bx(kind: Kind, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, h: f32) -> Solid {
    Solid::block(kind, [x0, z0, x1, z1], y, h)
}

fn sh(shape: Shape, kind: Kind, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) -> Solid {
    Solid::new(shape, kind, [x, y, z], [sx, sy, sz])
}

/// A seed for a lumpy shape, 1 to 64.
fn seed16(v: u32) -> u16 {
    (v % 64) as u16 + 1
}

/// A house: walls, a pitched roof (its ridge along the longer side), a
/// door and windows on the front, maybe a chimney.
struct House {
    walls: Kind,
    wall_h: f32,
    roof: Kind,
    roof_h: f32,
    hip: bool,
    windows: u8,
    door: bool,
    chimney: bool,
}

fn house(b: &mut B, [x0, z0, x1, z1]: [f32; 4], h: &House) {
    let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    let (w, d) = (x1 - x0, z1 - z0);
    b.add(bx(h.walls, x0, z0, x1, z1, 0.0, h.wall_h));
    let shape = if h.hip { Shape::Hip } else { Shape::Gable };
    let over = 1.5;
    if w >= d {
        b.add(sh(shape, h.roof, cx, h.wall_h, cz, w + 2.0 * over, h.roof_h, d + 2.0 * over));
    } else {
        b.add(sh(shape, h.roof, cx, h.wall_h, cz, d + 2.0 * over, h.roof_h, w + 2.0 * over).turned(90.0));
    }
    if h.chimney {
        let (x, z) = (x0 + w * 0.74, z0 + d * 0.4);
        b.add(bx(Kind::Stone, x - 2.0, z - 2.0, x + 2.0, z + 2.0, h.wall_h, h.roof_h * 0.85 + 3.0));
    }
    let door_h = (h.wall_h - 3.0).clamp(4.0, 11.0);
    let door_x = if h.windows % 2 == 1 { x0 + w * 0.3 } else { cx };
    if h.door {
        b.add(sh(Shape::Box, Kind::Door, door_x, 0.0, z1 + 0.25, 7.0_f32.min(w * 0.4), door_h, 1.0));
    }
    let n = h.windows as usize;
    for i in 0..n {
        let x = x0 + w * (i as f32 + 0.5) / n as f32;
        if h.door && (x - door_x).abs() < 7.0 {
            continue;
        }
        let y = (h.wall_h * 0.42).max(3.0);
        b.add(sh(Shape::Box, Kind::Window, x, y, z1 + 0.25, 5.0, (h.wall_h * 0.3).clamp(2.5, 5.0), 1.0));
    }
}

/// A plain house of the seed's walls and roof filling `rect`.
fn cottage_in(b: &mut B, rect: [f32; 4], wall_h: f32, roof_h: f32, windows: u8, chimney: bool) {
    let (walls, roof, hip) = (b.walls(), b.roof(), b.r(3) % 4 == 0);
    house(b, rect, &House { walls, wall_h, roof, roof_h, hip, windows, door: true, chimney });
}

/// A round tower at `(x, z)`, `r` across its middle, `h` high, under `top`.
fn tower(b: &mut B, x: f32, z: f32, r: f32, h: f32, walls: Kind, top: Top) {
    b.add(sh(Shape::Cylinder, walls, x, 0.0, z, 2.0 * r, h, 2.0 * r));
    tower_top(b, x, z, r, h, top);
}

fn tower_top(b: &mut B, x: f32, z: f32, r: f32, h: f32, top: Top) {
    match top {
        Top::Cone(kind) => b.add(sh(Shape::Cone, kind, x, h, z, 2.0 * r + 2.0, (r * 1.6).max(6.0), 2.0 * r + 2.0)),
        Top::Dome(kind) => {
            let s = 2.0 * r * 0.95;
            b.add(sh(Shape::Sphere, kind, x, h - s * 0.45, z, s, s * 0.9, s));
        }
        Top::Crenel => {
            b.add(sh(Shape::Cylinder, Kind::Stone, x, h, z, 2.0 * r + 1.5, 2.0, 2.0 * r + 1.5));
            let n = if r >= 5.0 { 6 } else { 4 };
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                let (s, c) = a.sin_cos();
                let (mx, mz) = (x + c * (r - 0.5), z + s * (r - 0.5));
                b.add(bx(Kind::Stone, mx - 1.25, mz - 1.25, mx + 1.25, mz + 1.25, h + 2.0, 2.5));
            }
        }
    }
}

/// Merlons along a wall top from `(x0, z0)` to `(x1, z1)` (one of them
/// equal), `t` thick, at `y`.
fn crenels(b: &mut B, (x0, z0): (f32, f32), (x1, z1): (f32, f32), t: f32, y: f32) {
    let along_x = (z1 - z0).abs() < 1e-3;
    let len = if along_x { x1 - x0 } else { z1 - z0 };
    let n = ((len / 5.0).floor() as i32).max(1);
    let step = len / n as f32;
    for k in 0..n {
        let a = (k as f32 + 0.5) * step;
        let m = step * 0.55;
        if along_x {
            b.add(bx(Kind::Stone, x0 + a - m / 2.0, z0 - t / 2.0, x0 + a + m / 2.0, z0 + t / 2.0, y, 2.5));
        } else {
            b.add(bx(Kind::Stone, x0 - t / 2.0, z0 + a - m / 2.0, x0 + t / 2.0, z0 + a + m / 2.0, y, 2.5));
        }
    }
}

/// A square tower of `walls`, `s` across, `h` high, with a pyramid roof
/// (`roof`) or crenels (`None`).
fn square_tower(b: &mut B, x: f32, z: f32, s: f32, h: f32, walls: Kind, roof: Option<Kind>) {
    let hs = s / 2.0;
    b.add(bx(walls, x - hs, z - hs, x + hs, z + hs, 0.0, h));
    match roof {
        Some(kind) => b.add(sh(Shape::Pyramid, kind, x, h, z, s + 2.0, s * 1.2, s + 2.0)),
        None => {
            crenels(b, (x - hs, z - hs + 0.75), (x + hs, z - hs + 0.75), 1.5, h);
            crenels(b, (x - hs, z + hs - 0.75), (x + hs, z + hs - 0.75), 1.5, h);
        }
    }
}

/// A slim window slot on a front at `z`.
fn slot(b: &mut B, x: f32, y: f32, z: f32) {
    b.add(sh(Shape::Box, Kind::Window, x, y, z, 2.5, 4.0, 1.0));
}

/// A tree of an orchard or a yard: a trunk and a round crown.
fn small_tree(b: &mut B, x: f32, z: f32, s: f32, k: u32) {
    let trunk = s * 0.35;
    b.add(sh(Shape::Cylinder, Kind::Trunk, x, 0.0, z, 2.5, trunk + 2.0, 2.5));
    b.add(sh(Shape::Blob(seed16(b.r(k))), Kind::Foliage, x, trunk, z, s, s * 0.85, s));
}

/// A wheel standing on edge, facing the front: a disc.
fn wheel(b: &mut B, x: f32, z: f32, r: f32) {
    b.add(sh(Shape::Sphere, Kind::Wood, x, 0.0, z, 2.0 * r, 2.0 * r, 1.5));
}

/// A fence of `posts` along a run from `x0` to `x1` at `z` (`along_x`), or
/// from `z0` to `z1` at `x`.
fn fence_x(b: &mut B, x0: f32, x1: f32, z: f32, h: f32) {
    let posts = (((x1 - x0) / 8.0).round() as u16).clamp(2, 8);
    b.add(sh(Shape::Fence(posts), Kind::Wood, (x0 + x1) / 2.0, 0.0, z, x1 - x0, h, 2.0));
}

fn fence_z(b: &mut B, z0: f32, z1: f32, x: f32, h: f32) {
    let posts = (((z1 - z0) / 8.0).round() as u16).clamp(2, 8);
    b.add(sh(Shape::Fence(posts), Kind::Wood, x, 0.0, (z0 + z1) / 2.0, z1 - z0, h, 2.0).turned(90.0));
}

/// A heap of `n` rocks over `[x0, z0, x1, z1]`, the tallest `h`.
fn rock_heap(b: &mut B, [x0, z0, x1, z1]: [f32; 4], h: f32, n: u32, k: u32) {
    let (w, d) = (x1 - x0, z1 - z0);
    for i in 0..n {
        let v = b.r(k + i);
        // A turned rock's footprint is its corners' reach: keep that inside.
        let s = (0.45 + (v % 7) as f32 * 0.06) * w.min(d) * if i == 0 { 1.0 } else { 0.7 } / 1.35;
        let half = ((s / 2.0).powi(2) + (s * 0.45).powi(2)).sqrt();
        let x = (x0 + half + (v >> 4) as f32 % 997.0 / 997.0 * (w - 2.0 * half).max(0.0)).min(x1 - half);
        let z = (z0 + half + (v >> 14) as f32 % 991.0 / 991.0 * (d - 2.0 * half).max(0.0)).min(z1 - half);
        let rh = if i == 0 { h } else { h * (0.45 + (v >> 20 & 7) as f32 * 0.06) };
        b.add(sh(Shape::Rock(seed16(v >> 3)), Kind::Rock, x, 0.0, z, s, rh, s * 0.9).turned(((v >> 9) % 360) as f32));
    }
}

// ----------------------------------------------------------- the pieces ----

fn piece(kit: Kit, b: &mut B) {
    let (w, d) = (b.w, b.d);
    match kit {
        Kit::Cottage => {
            let dd = d.min(30.0);
            let wall_h = b.range(10, 12.0, 15.0);
            let roof_h = b.range(11, 9.0, 12.0);
            let windows = 1 + (b.r(12) % 2) as u8 + u8::from(w > 40.0);
            let chimney = b.r(13) % 3 != 0;
            cottage_in(b, [2.0, d - dd + 2.0, w - 2.0, d - 2.0], wall_h, roof_h, windows, chimney);
        }
        Kit::Barn | Kit::Stable => {
            let low = kit == Kit::Stable;
            let dd = d.min(if low { 14.0 } else { 30.0 });
            let roof = b.pick(4, &[Kind::Thatch, Kind::Thatch, Kind::Roof, Kind::Slate]);
            let (x0, z0, x1, z1) = (2.0, d - dd + 1.5, w - 2.0, d - 1.5);
            let wall_h = if low { 9.0 } else { 14.0 };
            house(b, [x0, z0, x1, z1], &House { walls: Kind::Wood, wall_h, roof, roof_h: if low { 7.0 } else { 13.0 }, hip: false, windows: 0, door: false, chimney: false });
            let doors = if low { ((x1 - x0) / 9.0).floor().max(1.0) as usize } else { 1 };
            for i in 0..doors {
                let x = x0 + (x1 - x0) * (i as f32 + 0.5) / doors as f32;
                b.add(sh(Shape::Box, Kind::Door, x, 0.0, z1 + 0.25, if low { 6.0 } else { 11.0 }, wall_h - 2.0, 1.0));
            }
        }
        Kit::Hall | Kit::Ward => {
            let dd = d.min(30.0);
            let walls = b.pick(5, &[Kind::Stone, Kind::Stone, Kind::Plaster, Kind::Brick]);
            let roof = b.pick(6, &[Kind::Slate, Kind::Roof, Kind::Slate]);
            let windows = ((w - 4.0) / 9.0).floor().clamp(2.0, 6.0) as u8;
            let hip = b.r(7) % 2 == 0;
            house(b, [2.0, d - dd + 2.0, w - 2.0, d - 2.0], &House { walls, wall_h: 17.0, roof, roof_h: 12.0, hip, windows, door: true, chimney: true });
            if kit == Kit::Ward {
                b.cut();
                let (x, z) = (w - 4.0, d - dd + 4.0);
                b.add(sh(Shape::Cylinder, Kind::Wood, x, 0.0, z, 1.5, 36.0, 1.5));
                b.add(bx(Kind::Cloth, x - 7.5, z - 0.5, x - 0.75, z + 0.5, 25.0, 10.0));
            }
        }
        Kit::Forge | Kit::Workshop => {
            let dd = d.min(28.0);
            let split = if w >= 40.0 { w * 0.58 } else { w * 0.62 };
            let wall_h = 13.0;
            cottage_in(b, [2.0, d - dd + 2.0, split, d - 2.0], wall_h, 10.0, 1, false);
            // The open bay: a lean-to on posts, its back on the house's side.
            let (x0, x1, z0, z1) = (split, w - 2.0, d - dd + 3.0, d - 2.0);
            for (x, z) in [(x1 - 1.0, z1 - 1.0), (x1 - 1.0, z0 + 1.0)] {
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, z, 2.0, wall_h - 2.0, 2.0));
            }
            b.add(sh(Shape::Wedge, b.roof(), (x0 + x1) / 2.0, wall_h - 2.0, (z0 + z1) / 2.0, z1 - z0 + 2.0, 4.0, x1 - x0 + 1.0).turned(90.0));
            if kit == Kit::Forge {
                // The stack and a hearth block under the bay.
                b.add(bx(Kind::Brick, split - 6.0, d - dd + 4.0, split - 1.0, d - dd + 9.0, 0.0, wall_h + 13.0));
                b.add(bx(Kind::Stone, x0 + 2.0, z0 + 4.0, x0 + 9.0, z0 + 10.0, 0.0, 5.0));
            } else {
                b.add(bx(Kind::Wood, x0 + 2.0, z1 - 7.0, x1 - 3.0, z1 - 3.0, 0.0, 4.0));
            }
        }
        Kit::Tower => {
            let r = (w.min(d) / 2.0 - 2.0).min(10.0);
            let (walls, roof) = (b.walls(), b.roof());
            tower(b, w / 2.0, d - r - 2.0, r, 34.0, walls, Top::Cone(roof));
            b.add(sh(Shape::Box, Kind::Door, w / 2.0, 0.0, d - 2.0 + 0.25, 6.0, 9.0, 1.0));
        }
        Kit::Turret => {
            let (x0, x1) = (w / 2.0 - 6.0, w / 2.0 + 6.0);
            b.add(bx(Kind::Stone, x0, 2.0, x1, d - 2.0, 0.0, 30.0));
            crenels(b, (x0 + 0.75, 2.0), (x0 + 0.75, d - 2.0), 1.5, 30.0);
            crenels(b, (x1 - 0.75, 2.0), (x1 - 0.75, d - 2.0), 1.5, 30.0);
            crenels(b, (x0, d - 2.75), (x1, d - 2.75), 1.5, 30.0);
            slot(b, w / 2.0, 18.0, d - 2.0 + 0.25);
        }
        Kit::Rookery => {
            let s = (w - 4.0).min(12.0);
            let z = d - s / 2.0 - 2.0;
            let walls = b.pick(8, &[Kind::Stone, Kind::Wood]);
            square_tower(b, w / 2.0, z, s, 40.0, walls, Some(Kind::Slate));
            for y in [24.0, 32.0] {
                b.add(bx(Kind::Wood, w / 2.0 - s / 2.0 - 1.5, z + s / 2.0 - 1.0, w / 2.0 + s / 2.0 + 1.5, z + s / 2.0 + 0.5, y, 1.5));
            }
            b.add(sh(Shape::Box, Kind::Door, w / 2.0, 0.0, z + s / 2.0 + 0.25, 5.0, 9.0, 1.0));
            slot(b, w / 2.0, 28.0, z + s / 2.0 + 0.25);
        }
        Kit::Gatehouse => {
            // Two slender round towers at the front, the gate's arch in the
            // wall between them; each its own part, so the camera keeps the
            // towers tall where the wall must stay low.
            let r = (w / 6.0).clamp(3.5, 6.0);
            let z = d - r - 1.0;
            for x in [r + 1.0, w - r - 1.0] {
                tower(b, x, z, r, 30.0, Kind::Stone, Top::Cone(Kind::Slate));
                slot(b, x, 20.0, z + r - 0.25);
                b.cut();
            }
            let span = w - 4.0 * r - 2.0;
            b.add(sh(Shape::Arch, Kind::Stone, w / 2.0, 0.0, z, span + 2.0, 18.0, (2.0 * r - 2.0).min(d - 2.0)));
            b.add(sh(Shape::Box, Kind::Door, w / 2.0, 0.0, z, (span + 2.0) * 0.6 - 0.5, 18.0 * 0.8 - 0.5, 1.0));
            crenels(b, (w / 2.0 - span / 2.0 - 1.0, z + r - 1.75), (w / 2.0 + span / 2.0 + 1.0, z + r - 1.75), 1.5, 18.0);
        }
        Kit::Castle(c) => castle(b, c),
        Kit::Keep => {
            // The great block with its windows and merlons; its corner
            // towers and its door each their own part (they stand beside
            // it, not on it), so the camera sizes each alone.
            let (x0, z0, x1, z1) = (4.0, 4.0_f32.min(d - 12.0), w - 4.0, d - 3.0);
            b.add(bx(Kind::Stone, x0, z0, x1, z1, 0.0, 40.0));
            crenels(b, (x0, z1 - 0.75), (x1, z1 - 0.75), 1.5, 40.0);
            crenels(b, (x0, z0 + 0.75), (x1, z0 + 0.75), 1.5, 40.0);
            for (x, y) in [(x0 + 7.0, 22.0), (x1 - 7.0, 22.0), (w / 2.0, 30.0), (x0 + 7.0, 32.0), (x1 - 7.0, 32.0)] {
                slot(b, x, y, z1 + 0.25);
            }
            b.cut();
            for (x, z) in [(x0, z1), (x1, z1), (x0, z0), (x1, z0)] {
                tower(b, x.clamp(4.0, w - 4.0), z.clamp(4.0, d - 4.0), 3.5, 46.0, Kind::Stone, Top::Cone(Kind::Slate));
                b.cut();
            }
            b.add(sh(Shape::Arch, Kind::Stone, w / 2.0, 0.0, z1 + 0.5, 14.0, 16.0, 3.0));
            b.add(sh(Shape::Box, Kind::Door, w / 2.0, 0.0, z1 + 0.25, 7.5, 12.0, 1.0));
        }
        Kit::Windmill => {
            let r = (w.min(d) / 2.0 - 3.0).min(11.0);
            let (x, z) = (w / 2.0, d / 2.0 + 1.0);
            let walls = b.pick(9, &[Kind::Stone, Kind::Plaster, Kind::Wood]);
            b.add(sh(Shape::Frustum, walls, x, 0.0, z, 2.0 * r, 30.0, 2.0 * r));
            b.add(sh(Shape::Cone, b.pick(10, &[Kind::Thatch, Kind::Slate, Kind::Roof]), x, 30.0, z, 1.35 * r, 8.0, 1.35 * r));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, z + r - 0.25, 5.0, 9.0, 1.0));
            slot(b, x, 18.0, z + r * 0.8);
            // The sails: a cross on its hub at the front, cloth on the frames.
            let (hub, zs) = (27.0, z + r * 0.62 + 1.0);
            let arm = 13.0;
            b.add(sh(Shape::Box, Kind::Wood, x, hub - 1.0, zs, 2.0 * arm, 2.0, 1.0));
            b.add(sh(Shape::Box, Kind::Wood, x, hub - arm, zs, 2.0, 2.0 * arm, 1.0));
            b.add(sh(Shape::Box, Kind::Cloth, x + arm / 2.0 + 1.0, hub + 1.0, zs + 0.25, arm - 2.0, 4.0, 0.75));
            b.add(sh(Shape::Box, Kind::Cloth, x - arm / 2.0 - 1.0, hub - 5.0, zs + 0.25, arm - 2.0, 4.0, 0.75));
            b.add(sh(Shape::Box, Kind::Cloth, x + 2.0, hub - arm, zs + 0.25, 4.0, arm - 2.0, 0.75));
            b.add(sh(Shape::Box, Kind::Cloth, x - 2.0, hub + 2.0, zs + 0.25, 4.0, arm - 2.0, 0.75));
            b.add(sh(Shape::Sphere, Kind::Wood, x, hub - 2.0, zs + 0.5, 4.0, 4.0, 2.0));
        }
        Kit::Watermill => {
            let dd = d.min(28.0);
            let x1 = w * 0.62;
            cottage_in(b, [2.0, d - dd + 2.0, x1, d - 2.0], 14.0, 11.0, 2, true);
            // The race beside it and its wheel on edge.
            b.add(bx(Kind::Water, x1 + 1.0, d - dd + 2.0, w - 2.0, d - 2.0, -2.0, 2.5));
            b.add(bx(Kind::Stone, x1 + 1.0, d - dd + 2.0, w - 2.0, d - dd + 4.0, 0.0, 4.0));
            b.cut();
            let r = ((w - x1) / 2.0 - 1.5).clamp(5.0, 11.0);
            b.add(sh(Shape::Sphere, Kind::Wood, (x1 + w) / 2.0, -2.0, d - 3.5, 2.0 * r, 2.0 * r, 2.5));
        }
        Kit::Stall => {
            // One stall to a tile along the longer side.
            let n = ((w.max(d)) / 16.0).round().max(1.0) as usize;
            for i in 0..n {
                let x0 = 1.5 + i as f32 * (w - 3.0) / n as f32;
                let x1 = x0 + (w - 3.0) / n as f32 - 1.0;
                let (z0, z1) = (d - 13.0_f32.min(d - 2.0), d - 1.5);
                stall(b, [x0, z0, x1, z1], i as u32);
                b.cut();
            }
        }
        Kit::Well => {
            let (x, z) = (w / 2.0, d / 2.0);
            let r = (w.min(d) / 2.0 - 2.0).min(6.0);
            b.add(sh(Shape::Cylinder, Kind::Stone, x, 0.0, z, 2.0 * r, 5.0, 2.0 * r));
            b.add(sh(Shape::Cylinder, Kind::Water, x, 0.0, z, 2.0 * r - 3.0, 4.5, 2.0 * r - 3.0));
            for px in [x - r + 0.5, x + r - 0.5] {
                b.add(sh(Shape::Box, Kind::Wood, px, 0.0, z, 1.5, 13.0, 1.5));
            }
            b.add(sh(Shape::Gable, b.pick(11, &[Kind::Thatch, Kind::Roof, Kind::Wood]), x, 13.0, z, 2.0 * r + 3.0, 5.0, 2.0 * r));
        }
        Kit::Lighthouse => {
            let r = (d / 2.0 - 4.0).min(10.0);
            let (x, z) = (r + 4.0, d - r - 3.0);
            b.add(sh(Shape::Frustum, Kind::Plaster, x, 0.0, z, 2.0 * r, 50.0, 2.0 * r));
            b.add(sh(Shape::Cylinder, Kind::Stone, x, 50.0, z, 1.4 * r, 2.0, 1.4 * r));
            b.add(sh(Shape::Cylinder, Kind::Window, x, 52.0, z, 0.9 * r, 6.0, 0.9 * r));
            b.add(sh(Shape::Cone, Kind::Metal, x, 58.0, z, 1.2 * r, 6.0, 1.2 * r));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, z + r - 0.25, 5.0, 9.0, 1.0));
            slot(b, x, 26.0, z + r * 0.8);
            b.cut();
            cottage_in(b, [2.0 * r + 8.0, d - 22.0, w - 2.0, d - 2.0], 11.0, 8.0, 1, true);
        }
        Kit::Chapel => chapel(b, [2.0, (d - 30.0).max(2.0), w - 2.0, d - 2.0]),
        Kit::Priory => {
            let x1 = (w * 0.55).max(26.0);
            chapel(b, [2.0, (d - 32.0).max(2.0), x1, d - 2.0]);
            b.cut();
            // The cloister: a low arcade round a garth.
            let (cx0, cz0, cx1, cz1) = (x1 + 2.0, (d - 26.0).max(2.0), w - 2.0, d - 2.0);
            b.add(bx(Kind::Stone, cx0, cz0, cx1, cz0 + 3.0, 0.0, 8.0));
            b.add(bx(Kind::Stone, cx1 - 3.0, cz0, cx1, cz1, 0.0, 8.0));
            b.add(sh(Shape::Gable, Kind::Slate, (cx0 + cx1) / 2.0, 8.0, cz0 + 1.5, cx1 - cx0 + 1.0, 3.0, 5.0));
            for k in 0..3 {
                let x = cx0 + (cx1 - cx0) * (k as f32 + 0.5) / 3.0;
                b.add(sh(Shape::Arch, Kind::Stone, x, 0.0, cz1 - 1.5, ((cx1 - cx0) / 3.0 - 1.0).min(9.0), 7.0, 2.5));
            }
            b.add(bx(Kind::Grass, cx0, cz0 + 3.0, cx1 - 3.0, cz1 - 3.0, 0.0, 0.5));
        }
        Kit::RoundTable => {
            let r = w.min(d) / 2.0 - 2.0;
            let (x, z) = (w / 2.0, d / 2.0);
            b.add(sh(Shape::Cylinder, Kind::Plaster, x, 0.0, z, 2.0 * r, 14.0, 2.0 * r));
            b.add(sh(Shape::Cone, b.pick(12, &[Kind::Roof, Kind::Slate]), x, 14.0, z, 2.0 * r + 3.0, 13.0, 2.0 * r + 3.0));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, z + r - 0.25, 6.0, 10.0, 1.0));
            for dx in [-0.6, 0.6] {
                b.add(sh(Shape::Box, Kind::Window, x + dx * r, 6.0, z + r * 0.8 - 0.25, 3.5, 4.0, 1.0));
            }
        }
        Kit::Observatory => {
            let r = (w.min(d) / 2.0 - 2.5).min(11.0);
            let (x, z) = (w / 2.0, d / 2.0);
            tower(b, x, z, r, 28.0, Kind::Stone, Top::Dome(Kind::Metal));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, z + r - 0.25, 6.0, 9.0, 1.0));
            slot(b, x, 18.0, z + r - 0.25);
            b.add(bx(Kind::Wood, x - 1.5, z + r * 0.3, x + 1.5, z + r * 0.9, 30.0, 3.0));
        }
        Kit::MineMouth => {
            rock_heap(b, [1.0, 1.0, w - 1.0, d - 6.0], 22.0, 4, 20);
            b.cut();
            let x = w / 2.0;
            b.add(bx(Kind::Rock, 2.0, d - 10.0, w - 2.0, d - 2.0, 0.0, 14.0));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, d - 2.0 + 0.25, 10.0, 10.0, 1.0));
            for px in [x - 6.0, x + 6.0] {
                b.add(sh(Shape::Box, Kind::Wood, px, 0.0, d - 2.0, 2.0, 12.0, 2.0));
            }
            b.add(sh(Shape::Box, Kind::Wood, x, 11.0, d - 2.0, 15.0, 2.0, 2.0));
        }
        Kit::DragonKeep => {
            let (x0, z0, x1, z1) = (5.0, 5.0, w - 5.0, d - 4.0);
            b.add(bx(Kind::Stone, x0 + 4.0, z0, x1 - 6.0, z1 - 10.0, 0.0, 32.0));
            for (i, (x, z, h)) in [(x0, z0, 40.0), (x1, z0, 26.0), (x0, z1 - 4.0, 20.0), (x1, z1 - 4.0, 30.0)].into_iter().enumerate() {
                b.add(sh(Shape::Cylinder, Kind::Stone, x, 0.0, z, 8.0, h, 8.0));
                if i % 2 == 0 {
                    b.add(bx(Kind::Stone, x - 3.5, z - 1.0, x - 0.5, z + 2.0, h, 4.0));
                }
            }
            b.add(bx(Kind::Stone, x0, z1 - 4.0, x0 + 14.0, z1 - 2.0, 0.0, 12.0));
            b.add(bx(Kind::Stone, x1 - 10.0, z1 - 4.0, x1, z1 - 2.0, 0.0, 7.0));
            b.add(sh(Shape::Arch, Kind::Stone, (x0 + x1) / 2.0 + 2.0, 0.0, z1 - 3.0, 14.0, 18.0, 3.0));
            b.add(sh(Shape::Box, Kind::Door, (x0 + x1) / 2.0 + 2.0, 0.0, z1 - 3.0, 7.5, 13.0, 1.0));
            b.cut();
            rock_heap(b, [x1 - 14.0, z1 - 4.0, w - 1.0, d - 1.0], 6.0, 3, 30);
        }
        Kit::Colosseum => colosseum(b),
        Kit::Pavilion => {
            let r = w.min(d) / 2.0 - 2.0;
            let (x, z) = (w / 2.0, d / 2.0);
            b.add(sh(Shape::Cylinder, Kind::Cloth, x, 0.0, z, 2.0 * r, 10.0, 2.0 * r));
            b.add(sh(Shape::Cone, Kind::Cloth, x, 10.0, z, 2.0 * r + 2.0, 12.0, 2.0 * r + 2.0));
            b.add(sh(Shape::Cylinder, Kind::Wood, x, 21.0, z, 1.0, 6.0, 1.0));
            b.add(bx(Kind::Cloth, x + 0.5, z - 0.4, x + 5.0, z + 0.4, 24.0, 3.0));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, z + r - 0.25, 6.0, 8.0, 1.0));
        }
        Kit::Quintain => {
            let (x, z) = (w / 2.0, d / 2.0);
            b.add(sh(Shape::Cylinder, Kind::Wood, x, 0.0, z, 2.5, 22.0, 2.5));
            b.add(bx(Kind::Wood, x - 6.0, z - 0.75, x + 6.0, z + 0.75, 19.0, 1.5));
            b.add(bx(Kind::Metal, x - 7.0, z - 1.0, x - 3.0, z + 1.0, 14.0, 5.0));
            b.add(sh(Shape::Sphere, Kind::Cloth, x + 5.0, 13.0, z, 3.5, 5.0, 3.5));
        }
        Kit::Board => {
            for x in [3.0, w - 3.0] {
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, d / 2.0, 2.0, 22.0, 2.0));
            }
            b.add(bx(Kind::Wood, 2.0, d / 2.0 - 1.0, w - 2.0, d / 2.0 + 1.0, 8.0, 13.0));
            b.add(sh(Shape::Gable, Kind::Slate, w / 2.0, 21.0, d / 2.0, w - 2.0, 3.0, 4.0));
        }
        Kit::Garden => {
            let (x0, z0, x1, z1) = (1.5, 1.5, w - 1.5, d - 1.5);
            b.add(bx(Kind::Hedge, x0, z0, x1, z0 + 2.5, 0.0, 5.0));
            b.add(bx(Kind::Hedge, x0, z1 - 2.5, x1, z1, 0.0, 4.0));
            b.add(bx(Kind::Hedge, x0, z0 + 2.5, x0 + 2.5, z1 - 2.5, 0.0, 4.5));
            b.add(bx(Kind::Hedge, x1 - 2.5, z0 + 2.5, x1, z1 - 2.5, 0.0, 4.5));
            let n = ((x1 - x0 - 5.0) / 7.0).floor().max(1.0) as usize;
            for i in 0..n {
                let bx0 = x0 + 3.5 + i as f32 * (x1 - x0 - 5.0) / n as f32;
                b.add(bx(Kind::Crop, bx0, z0 + 4.0, bx0 + (x1 - x0 - 5.0) / n as f32 - 2.0, z1 - 4.0, 0.0, 2.5));
            }
        }
        Kit::School => {
            let dd = d.min(36.0);
            let walls = Kind::Stone;
            house(b, [2.0, d - dd + 2.0, w * 0.68, d - 2.0], &House { walls, wall_h: 18.0, roof: Kind::Slate, roof_h: 14.0, hip: false, windows: 4, door: true, chimney: true });
            b.cut();
            let s = (w * 0.26).min(16.0);
            let (x, z) = (w - s / 2.0 - 3.0, d - s / 2.0 - 3.0);
            square_tower(b, x, z, s, 32.0, walls, None);
            b.add(sh(Shape::Cone, Kind::Slate, x, 32.0, z, s - 2.0, 18.0, s - 2.0));
            slot(b, x, 22.0, z + s / 2.0 + 0.25);
            slot(b, x, 12.0, z + s / 2.0 + 0.25);
        }
        Kit::College => {
            let dd = d.min(40.0);
            let (z0, z1) = (d - dd + 2.0, d - 2.0);
            let t = 14.0_f32.min(w / 6.0);
            house(b, [t + 1.0, z0 + 4.0, w - t - 1.0, z1], &House { walls: Kind::Stone, wall_h: 18.0, roof: Kind::Slate, roof_h: 13.0, hip: false, windows: 8, door: false, chimney: false });
            b.cut();
            for x in [t / 2.0 + 1.0, w - t / 2.0 - 1.0] {
                square_tower(b, x, z1 - t / 2.0, t, 30.0, Kind::Stone, Some(Kind::Slate));
                slot(b, x, 20.0, z1 + 0.25);
                b.cut();
            }
            let g = 16.0;
            square_tower(b, w / 2.0, z1 - g / 2.0 + 1.0, g, 34.0, Kind::Stone, None);
            b.add(sh(Shape::Arch, Kind::Stone, w / 2.0, 0.0, z1, g - 2.0, 16.0, 2.0));
            b.add(sh(Shape::Box, Kind::Door, w / 2.0, 0.0, z1, 7.5, 12.0, 1.0));
            slot(b, w / 2.0, 24.0, z1 + 1.25);
        }
        Kit::DelveGate => {
            // A rock face with the gate's arch in it, the door dark inside,
            // a pillar either side; a heap of rock behind where there is room.
            let x = w / 2.0;
            let face = (d - 1.0).min(14.0);
            if d >= 30.0 {
                rock_heap(b, [1.0, 1.0, w - 1.0, d - face], 26.0, 4, 40);
                b.cut();
            }
            let arch = (w * 0.62).min(22.0);
            b.add(bx(Kind::Rock, 1.0, d - face, w - 1.0, d - 3.0, 0.0, arch + 2.0));
            b.add(sh(Shape::Arch, Kind::Stone, x, 0.0, d - 3.0, arch, arch, 3.0));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, d - 3.0, arch * 0.6 - 1.0, arch * 0.8 - 1.0, 1.0));
            for px in [x - arch / 2.0 - 2.0, x + arch / 2.0 + 2.0] {
                b.add(sh(Shape::Box, Kind::Stone, px, 0.0, d - 3.0, 3.5, arch + 2.0, 3.5));
                b.add(sh(Shape::Pyramid, Kind::Stone, px, arch + 2.0, d - 3.0, 4.0, 4.0, 4.0));
            }
        }
        Kit::Hippodrome => {
            // Tiers at the back, facing the front, under an awning; the
            // track's rail before them.
            let (z0, z1) = (3.0, (d * 0.55).max(16.0));
            b.add(sh(Shape::Stairs, Kind::Wood, w / 2.0, 0.0, (z0 + z1) / 2.0, w - 4.0, 12.0, z1 - z0).turned(180.0));
            b.add(bx(Kind::Wood, 2.0, z0 - 1.0, w - 2.0, z0 + 1.0, 0.0, 22.0));
            for x in [3.0, w / 2.0, w - 3.0] {
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, z1 - 1.0, 1.5, 20.0, 1.5));
            }
            b.add(sh(Shape::Wedge, Kind::Cloth, w / 2.0, 20.0, (z0 + z1) / 2.0, w - 2.0, 4.0, z1 - z0 + 2.0));
            b.cut();
            fence_x(b, 2.0, w - 2.0, d - 3.0, 5.0);
            b.add(bx(Kind::Path, 2.0, z1 + 1.0, w - 2.0, d - 4.0, 0.0, 0.5));
        }
        Kit::Aqueduct => {
            let n = ((w - 4.0) / 14.0).round().max(2.0) as usize;
            let z = d - 8.0;
            let span = (w - 4.0) / n as f32;
            for i in 0..n {
                b.add(sh(Shape::Arch, Kind::Stone, 2.0 + span * (i as f32 + 0.5), 0.0, z, span, 22.0, 6.0));
            }
            b.add(bx(Kind::Stone, 2.0, z - 3.5, w - 2.0, z + 3.5, 22.0, 4.0));
            b.add(bx(Kind::Water, 3.0, z - 2.0, w - 3.0, z + 2.0, 24.0, 2.5));
        }
        Kit::Belfry => {
            let s = (w.min(d) - 6.0).min(16.0);
            let (x, z) = (w / 2.0, d - s / 2.0 - 3.0);
            let walls = b.pick(13, &[Kind::Stone, Kind::Stone, Kind::Brick]);
            b.add(bx(walls, x - s / 2.0, z - s / 2.0, x + s / 2.0, z + s / 2.0, 0.0, 30.0));
            for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let (px, pz) = (x + dx * (s / 2.0 - 1.0), z + dz * (s / 2.0 - 1.0));
                b.add(sh(Shape::Box, walls, px, 30.0, pz, 2.0, 9.0, 2.0));
            }
            b.add(sh(Shape::Sphere, Kind::Metal, x, 31.0, z, 6.0, 7.0, 6.0));
            b.add(sh(Shape::Pyramid, Kind::Slate, x, 39.0, z, s + 2.0, 12.0, s + 2.0));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, z + s / 2.0 + 0.25, 6.0, 10.0, 1.0));
            b.cut();
            cottage_in(b, [2.0, d - 18.0, (x - s / 2.0 - 3.0).max(12.0), d - 2.0], 10.0, 7.0, 1, false);
        }
        Kit::Bothy => {
            let x1 = (w * 0.6).max(24.0);
            house(b, [3.0, (d - 22.0).max(3.0), x1, d - 3.0], &House { walls: Kind::Stone, wall_h: 10.0, roof: Kind::Thatch, roof_h: 9.0, hip: false, windows: 1, door: true, chimney: true });
            b.cut();
            b.add(bx(Kind::Trunk, x1 + 4.0, d - 10.0, w - 4.0, d - 4.0, 0.0, 5.0));
        }
        Kit::Ruins | Kit::SunkenAbbey => {
            let abbey = kit == Kit::SunkenAbbey;
            if abbey {
                b.add(bx(Kind::Water, 1.0, 1.0, w - 1.0, d - 1.0, -2.0, 2.0));
                b.cut();
            }
            let r = 7.0;
            let (tx, tz) = if abbey { (w - 10.0, d - 12.0) } else { (10.0, d - 12.0) };
            b.add(sh(Shape::Cylinder, Kind::Stone, tx, 0.0, tz, 2.0 * r, 22.0, 2.0 * r));
            b.add(bx(Kind::Stone, tx - r + 1.0, tz - 2.0, tx - 1.0, tz + 3.0, 22.0, 6.0));
            slot(b, tx, 12.0, tz + r - 0.25);
            b.cut();
            let (x0, x1) = if abbey { (3.0, w - 20.0) } else { (20.0, w - 3.0) };
            b.add(bx(Kind::Stone, x0, d - 26.0, x0 + 3.0, d - 4.0, 0.0, 16.0));
            b.add(bx(Kind::Stone, x0, d - 26.0, x1, d - 23.0, 0.0, 12.0));
            b.add(bx(Kind::Stone, x1 - 3.0, d - 26.0, x1, d - 12.0, 0.0, 8.0));
            if abbey {
                b.add(sh(Shape::Arch, Kind::Stone, (x0 + x1) / 2.0, 0.0, d - 5.0, (x1 - x0).min(18.0), 18.0, 3.0));
            }
            b.cut();
            rock_heap(b, [x0 + 4.0, d - 12.0, x1 - 2.0, d - 2.0], 4.0, 3, 50);
        }
        Kit::Crag | Kit::Cliffs | Kit::Eyrie => {
            if kit == Kit::Cliffs {
                // A face in two steps, lumped along its tops, scree before it.
                b.add(bx(Kind::Rock, 2.0, 2.0, w - 2.0, d * 0.5, 0.0, 18.0));
                b.add(bx(Kind::Rock, 2.0, d * 0.5, w - 2.0, d - 8.0, 0.0, 9.0));
                let n = ((w - 4.0) / 12.0).floor().max(2.0) as usize;
                for i in 0..n {
                    let x = 2.0 + (w - 4.0) * (i as f32 + 0.5) / n as f32;
                    let v = b.r(80 + i as u32);
                    let s = (w - 4.0) / n as f32;
                    b.add(sh(Shape::Rock(seed16(v)), Kind::Rock, x, 16.0, d * 0.27, s, 7.0 + (v % 5) as f32, d * 0.4));
                    b.add(sh(Shape::Rock(seed16(v >> 7)), Kind::Rock, x, 7.5, d * 0.5 + (d * 0.5 - 8.0) / 2.0, s * 0.9, 5.0 + (v >> 3 & 3) as f32, d * 0.5 - 10.0));
                }
                b.cut();
                rock_heap(b, [2.0, d - 12.0, w - 2.0, d - 2.0], 6.0, 4, 60);
            } else {
                rock_heap(b, [2.0, 2.0, w - 2.0, d - 2.0], if kit == Kit::Eyrie { 26.0 } else { 22.0 }, 5, 60);
                if kit == Kit::Eyrie {
                    b.add(sh(Shape::Cylinder, Kind::Wood, w * 0.4, 24.0, d * 0.45, 14.0, 4.0, 12.0));
                }
            }
        }
        Kit::Aviary => {
            let x1 = w * 0.55;
            cottage_in(b, [2.0, d - 22.0, x1, d - 2.0], 11.0, 8.0, 1, false);
            b.cut();
            let s = ((w - x1) - 6.0).min(12.0);
            let (x, z) = (x1 + 3.0 + s / 2.0, d - s / 2.0 - 3.0);
            square_tower(b, x, z, s, 26.0, Kind::Wood, Some(Kind::Roof));
            for y in [12.0, 18.0] {
                b.add(bx(Kind::Door, x - 3.0, z + s / 2.0, x + 3.0, z + s / 2.0 + 0.75, y, 2.5));
            }
        }
        Kit::Bathhouse => {
            let dd = d.min(30.0);
            let (x0, z0, x1, z1) = (2.0, d - dd + 2.0, w - 2.0, d - 2.0);
            b.add(bx(Kind::Stone, x0, z0, x1, z1, 0.0, 14.0));
            b.add(bx(Kind::Slate, x0 - 1.0, z0 - 1.0, x1 + 1.0, z1 + 1.0, 14.0, 2.0));
            let s = (z1 - z0) * 0.8;
            b.add(sh(Shape::Sphere, Kind::Slate, (x0 + x1) / 2.0, 16.0 - s * 0.4, (z0 + z1) / 2.0, s, s * 0.8, s));
            b.add(bx(Kind::Brick, x1 - 8.0, z0 + 2.0, x1 - 4.0, z0 + 6.0, 16.0, 10.0));
            b.add(sh(Shape::Box, Kind::Door, (x0 + x1) / 2.0, 0.0, z1 + 0.25, 7.0, 10.0, 1.0));
            for x in [x0 + 8.0, x1 - 8.0] {
                b.add(sh(Shape::Arch, Kind::Window, x, 4.0, z1 + 0.25, 6.0, 7.0, 1.0));
            }
        }
        Kit::LandOffice => {
            let dd = d.min(32.0);
            let (x0, z0, x1, z1) = (2.0, d - dd + 2.0, w - 2.0, d - 8.0);
            house(b, [x0, z0, x1, z1], &House { walls: Kind::Stone, wall_h: 16.0, roof: Kind::Slate, roof_h: 11.0, hip: true, windows: 4, door: true, chimney: true });
            // The porch: a pediment on columns.
            let (px0, px1) = (w / 2.0 - 10.0, w / 2.0 + 10.0);
            for x in [px0 + 1.5, px0 + 7.5, px1 - 7.5, px1 - 1.5] {
                b.add(sh(Shape::Cylinder, Kind::Stone, x, 0.0, d - 3.5, 2.5, 14.0, 2.5));
            }
            b.add(bx(Kind::Stone, px0, z1, px1, d - 2.0, 14.0, 2.0));
            b.add(sh(Shape::Gable, Kind::Slate, w / 2.0, 16.0, (z1 + d - 2.0) / 2.0, d - 2.0 - z1 + 1.0, 5.0, px1 - px0 + 2.0).turned(90.0));
        }
        Kit::Ranch => {
            let x1 = w * 0.55;
            house(b, [2.0, d - 28.0, x1, d - 2.0], &House { walls: Kind::Wood, wall_h: 14.0, roof: Kind::Thatch, roof_h: 12.0, hip: false, windows: 0, door: true, chimney: false });
            b.cut();
            fence_x(b, x1 + 2.0, w - 2.0, d - 2.5, 6.0);
            fence_x(b, x1 + 2.0, w - 2.0, d - 22.0, 6.0);
            fence_z(b, d - 22.0, d - 2.5, w - 2.5, 6.0);
            b.add(bx(Kind::Wood, x1 + 4.0, d - 12.0, x1 + 12.0, d - 9.0, 0.0, 3.0));
        }
        Kit::Orchard => {
            let (cols, rows) = (((w - 4.0) / 14.0).floor().max(2.0) as usize, ((d - 4.0) / 14.0).floor().max(1.0) as usize);
            for j in 0..rows {
                for i in 0..cols {
                    let x = 2.0 + (w - 4.0) * (i as f32 + 0.5) / cols as f32;
                    let z = 2.0 + (d - 4.0) * (j as f32 + 0.5) / rows as f32;
                    small_tree(b, x, z, 11.0, (j * cols + i) as u32);
                    b.cut();
                }
            }
        }
        Kit::HedgeMaze => {
            let (x0, z0, x1, z1) = (2.0, 2.0, w - 2.0, d - 2.0);
            let h = 9.0;
            let t = 3.0;
            b.add(bx(Kind::Hedge, x0, z0, x1, z0 + t, 0.0, h));
            b.add(bx(Kind::Hedge, x0, z0 + t, x0 + t, z1, 0.0, h));
            b.add(bx(Kind::Hedge, x1 - t, z0 + t, x1, z1, 0.0, h));
            b.add(bx(Kind::Hedge, x0 + t, z1 - t, (x0 + x1) / 2.0 - 4.0, z1, 0.0, h));
            b.add(bx(Kind::Hedge, (x0 + x1) / 2.0 + 4.0, z1 - t, x1 - t, z1, 0.0, h));
            b.add(bx(Kind::Hedge, x0 + 8.0, z0 + 9.0, x1 - 14.0, z0 + 9.0 + t, 0.0, h));
            b.add(bx(Kind::Hedge, x0 + 14.0, z1 - 12.0, x1 - 8.0, z1 - 12.0 + t, 0.0, h));
            b.add(bx(Kind::Hedge, (x0 + x1) / 2.0 - 1.5, z0 + 9.0 + t, (x0 + x1) / 2.0 + 1.5, z1 - 12.0, 0.0, h));
        }
        Kit::Timberyard => {
            let x1 = w * 0.45;
            b.add(sh(Shape::Box, Kind::Wood, 3.0, 0.0, d - 4.0, 2.0, 13.0, 2.0));
            b.add(sh(Shape::Box, Kind::Wood, x1 - 2.0, 0.0, d - 4.0, 2.0, 13.0, 2.0));
            b.add(sh(Shape::Box, Kind::Wood, 3.0, 0.0, d - 20.0, 2.0, 15.0, 2.0));
            b.add(sh(Shape::Box, Kind::Wood, x1 - 2.0, 0.0, d - 20.0, 2.0, 15.0, 2.0));
            b.add(sh(Shape::Wedge, Kind::Roof, (x1 + 1.0) / 2.0, 13.0, d - 12.0, x1, 3.0, 20.0));
            b.add(bx(Kind::Wood, 6.0, d - 14.0, x1 - 5.0, d - 9.0, 0.0, 5.0));
            b.cut();
            for z in [d - 20.0, d - 10.0] {
                let (lx0, lx1) = (x1 + 3.0, w - 3.0);
                for layer in 0..3 {
                    let y = layer as f32 * 3.0;
                    let inset = layer as f32 * 1.5;
                    b.add(bx(Kind::Trunk, lx0 + inset, z - 3.5 + inset * 0.5, lx1 - inset, z + 3.5 - inset * 0.5, y, 3.0));
                }
                b.cut();
            }
        }
        Kit::Saltworks => {
            let x1 = w * 0.42;
            cottage_in(b, [2.0, d - 20.0, x1, d - 2.0], 10.0, 7.0, 1, false);
            b.cut();
            for (i, z) in [d - 22.0, d - 12.0].into_iter().enumerate() {
                b.add(bx(Kind::Stone, x1 + 2.0, z, w - 2.0, z + 9.0, 0.0, 2.0));
                b.add(bx(Kind::Water, x1 + 3.0, z + 1.0, w - 3.0, z + 8.0, 0.0, 2.25));
                b.add(sh(Shape::Cone, Kind::Sand, x1 + 8.0 + i as f32 * 10.0, 2.0, z + 4.5, 7.0, 4.0, 6.0));
            }
        }
        Kit::Strand => {
            b.add(sh(Shape::Gable, Kind::Wood, w * 0.28, 0.0, d - 9.0, 22.0, 6.0, 8.0).turned(12.0));
            b.cut();
            b.add(sh(Shape::Gable, Kind::Wood, w * 0.62, 0.0, d - 18.0, 18.0, 5.0, 7.0).turned(-20.0));
            b.cut();
            for x in [w - 14.0, w - 4.0] {
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, d - 6.0, 1.5, 12.0, 1.5));
            }
            b.add(bx(Kind::Cloth, w - 13.25, d - 6.4, w - 4.75, d - 5.6, 4.0, 7.0));
            b.cut();
            house(b, [3.0, 3.0, 22.0, 18.0], &House { walls: Kind::Wood, wall_h: 9.0, roof: Kind::Thatch, roof_h: 7.0, hip: false, windows: 1, door: true, chimney: false });
        }
        Kit::Quay => {
            b.add(bx(Kind::Stone, 2.0, d - 14.0, w - 2.0, d - 2.0, 0.0, 3.0));
            for x in [8.0, w / 2.0, w - 8.0] {
                b.add(sh(Shape::Cylinder, Kind::Wood, x, 3.0, d - 3.5, 2.5, 3.0, 2.5));
            }
            b.add(bx(Kind::Wood, w - 22.0, d - 12.0, w - 14.0, d - 6.0, 3.0, 5.0));
            b.add(bx(Kind::Wood, w - 13.0, d - 12.0, w - 7.0, d - 7.0, 3.0, 4.0));
            b.cut();
            b.add(sh(Shape::Box, Kind::Wood, 12.0, 3.0, d - 9.0, 2.5, 22.0, 2.5));
            b.add(bx(Kind::Wood, 10.0, d - 10.0, 26.0, d - 8.0, 23.0, 2.0));
            b.cut();
            cottage_in(b, [w * 0.45, 2.0, w - 3.0, d - 16.0], 11.0, 8.0, 2, true);
        }
        Kit::Causeway => {
            b.add(bx(Kind::Stone, 2.0, d / 2.0 - 6.0, w - 2.0, d / 2.0 + 6.0, -2.0, 5.0));
            b.add(bx(Kind::Water, 1.0, 1.0, w - 1.0, d - 1.0, -2.0, 1.5));
            b.cut();
            for x in [5.0, w / 2.0, w - 5.0] {
                for z in [d / 2.0 - 5.0, d / 2.0 + 5.0] {
                    b.add(sh(Shape::Box, Kind::Stone, x, 3.0, z, 2.0, 7.0, 2.0));
                }
            }
        }
        Kit::TollBridge => {
            cottage_in(b, [2.0, d - 20.0, 20.0, d - 2.0], 11.0, 8.0, 1, true);
            b.cut();
            b.add(bx(Kind::Water, 22.0, 2.0, w - 2.0, d - 2.0, -2.0, 1.5));
            b.add(bx(Kind::Wood, 22.0, d / 2.0 - 7.0, w - 2.0, d / 2.0 + 7.0, 0.0, 3.0));
            b.add(sh(Shape::Box, Kind::Wood, 24.0, 0.0, d / 2.0 + 6.0, 2.5, 12.0, 2.5));
            b.add(bx(Kind::Wood, 24.0, d / 2.0 + 5.5, w - 4.0, d / 2.0 + 6.5, 9.0, 1.5));
        }
        Kit::Sluice => {
            b.add(bx(Kind::Water, 2.0, 2.0, w - 2.0, d - 2.0, -3.0, 2.0));
            b.cut();
            let (x0, x1) = (w / 2.0 - 14.0, w / 2.0 + 14.0);
            for x in [x0, x1] {
                b.add(bx(Kind::Stone, x - 4.0, d - 24.0, x + 4.0, d - 6.0, -3.0, 19.0));
            }
            b.add(bx(Kind::Wood, x0 + 4.0, d - 16.0, x1 - 4.0, d - 14.0, -3.0, 13.0));
            b.add(bx(Kind::Wood, x0 - 4.0, d - 17.0, x1 + 4.0, d - 13.0, 16.0, 2.0));
            b.add(sh(Shape::Sphere, Kind::Wood, w / 2.0, 18.0, d - 15.0, 6.0, 6.0, 1.5));
        }
        Kit::Flowerbeds | Kit::Scarecrow => {
            let rows = ((d - 6.0) / 7.0).floor().max(2.0) as usize;
            let x1 = if kit == Kit::Flowerbeds { w * 0.7 } else { w - 3.0 };
            for j in 0..rows {
                let z = 3.0 + j as f32 * (d - 6.0) / rows as f32;
                b.add(bx(Kind::Crop, 3.0, z + 1.0, x1, z + (d - 6.0) / rows as f32 - 1.0, 0.0, 2.5));
            }
            b.cut();
            if kit == Kit::Flowerbeds {
                let roof = b.roof();
                house(b, [x1 + 2.0, d - 18.0, w - 2.0, d - 2.0], &House { walls: Kind::Wood, wall_h: 9.0, roof, roof_h: 6.0, hip: false, windows: 0, door: true, chimney: false });
            } else {
                let (x, z) = (w / 2.0, d / 2.0);
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, z, 1.5, 18.0, 1.5));
                b.add(bx(Kind::Wood, x - 7.0, z - 0.6, x + 7.0, z + 0.6, 13.0, 1.5));
                b.add(sh(Shape::Box, Kind::Cloth, x, 6.0, z, 6.0, 8.0, 3.0));
                b.add(sh(Shape::Sphere, Kind::Cloth, x, 14.0, z, 4.0, 4.0, 4.0));
                b.add(sh(Shape::Cone, Kind::Thatch, x, 17.5, z, 7.0, 4.0, 7.0));
            }
        }
        Kit::Downs => {
            cottage_in(b, [2.0, d - 22.0, w * 0.5, d - 2.0], 11.0, 8.0, 1, true);
            b.cut();
            for k in 0..4 {
                let x = w * 0.58 + (k % 2) as f32 * 10.0;
                let z = d - 18.0 + (k / 2) as f32 * 9.0;
                b.add(sh(Shape::Cylinder, Kind::Thatch, x, 0.0, z, 7.0, 5.0, 7.0));
                b.cut();
            }
            fence_x(b, w * 0.53, w - 2.0, d - 2.5, 5.0);
        }
        Kit::Flags => {
            rock_heap(b, [w / 2.0 - 10.0, d / 2.0 - 8.0, w / 2.0 + 10.0, d / 2.0 + 8.0], 9.0, 3, 70);
            b.cut();
            for (i, x) in [w * 0.2, w * 0.5, w * 0.8].into_iter().enumerate() {
                let z = d - 6.0 - (i % 2) as f32 * 10.0;
                let h = 26.0 + (i % 2) as f32 * 6.0;
                b.add(sh(Shape::Cylinder, Kind::Wood, x, 0.0, z, 1.5, h, 1.5));
                b.add(bx(Kind::Cloth, x + 0.75, z - 0.4, x + 9.0, z + 0.4, h - 7.0, 6.0));
                b.cut();
            }
        }
        Kit::Caravan => {
            for (x, z) in [(w * 0.27, d - 9.0), (w * 0.7, d - 20.0)] {
                let (l, t) = (22.0, 10.0);
                b.add(bx(Kind::Wood, x - l / 2.0, z - t / 2.0, x + l / 2.0, z + t / 2.0, 3.0, 5.0));
                b.add(sh(Shape::Gable, Kind::Cloth, x, 8.0, z, l - 2.0, 7.0, t + 1.0));
                for wx in [x - l / 2.0 + 4.0, x + l / 2.0 - 4.0] {
                    b.add(sh(Shape::Sphere, Kind::Wood, wx, 0.0, z + t / 2.0 + 0.5, 6.0, 6.0, 1.2));
                }
                b.cut();
            }
        }
        Kit::Wheelwright => {
            cottage_in(b, [2.0, d - 26.0, w * 0.62, d - 2.0], 13.0, 10.0, 1, true);
            b.cut();
            wheel(b, w * 0.72, d - 4.0, 5.5);
            wheel(b, w * 0.86, d - 5.0, 4.5);
            b.add(bx(Kind::Wood, w * 0.66, d - 20.0, w - 3.0, d - 12.0, 0.0, 4.0));
        }
        Kit::Butts => {
            for x in [w * 0.2, w * 0.5, w * 0.8] {
                let z = 8.0;
                b.add(sh(Shape::Box, Kind::Wood, x - 3.0, 0.0, z, 1.5, 9.0, 1.5));
                b.add(sh(Shape::Box, Kind::Wood, x + 3.0, 0.0, z, 1.5, 9.0, 1.5));
                b.add(sh(Shape::Sphere, Kind::Thatch, x, 3.0, z + 1.0, 12.0, 12.0, 3.0));
                b.add(sh(Shape::Sphere, Kind::Cloth, x, 7.0, z + 2.5, 4.0, 4.0, 0.8));
                b.cut();
            }
            house(b, [w - 24.0, d - 16.0, w - 3.0, d - 2.0], &House { walls: Kind::Wood, wall_h: 9.0, roof: Kind::Thatch, roof_h: 6.0, hip: false, windows: 0, door: true, chimney: false });
        }
        Kit::Fletchery => {
            cottage_in(b, [2.0, d - 26.0, w * 0.6, d - 2.0], 13.0, 10.0, 2, true);
            b.cut();
            for z in [d - 18.0, d - 8.0] {
                b.add(sh(Shape::Box, Kind::Wood, w * 0.68, 0.0, z, 1.5, 10.0, 1.5));
                b.add(sh(Shape::Box, Kind::Wood, w - 4.0, 0.0, z, 1.5, 10.0, 1.5));
                b.add(bx(Kind::Wood, w * 0.68, z - 0.5, w - 4.0, z + 0.5, 7.0, 1.0));
                b.add(bx(Kind::Wood, w * 0.7, z - 0.3, w - 6.0, z + 0.3, 2.0, 5.0));
                b.cut();
            }
        }
        Kit::GoosePen => {
            b.add(bx(Kind::Water, w * 0.45, 4.0, w - 4.0, d * 0.5, -1.0, 1.5));
            b.cut();
            fence_x(b, 2.0, w - 2.0, d - 2.5, 5.0);
            fence_x(b, 2.0, w - 2.0, 2.5, 5.0);
            fence_z(b, 2.5, d - 2.5, 2.5, 5.0);
            fence_z(b, 2.5, d - 2.5, w - 2.5, 5.0);
            b.cut();
            house(b, [5.0, d - 18.0, 20.0, d - 5.0], &House { walls: Kind::Wood, wall_h: 7.0, roof: Kind::Thatch, roof_h: 5.0, hip: false, windows: 0, door: true, chimney: false });
            b.cut();
            b.add(bx(Kind::Wood, w * 0.5, d - 14.0, w * 0.5 + 14.0, d - 8.0, 3.0, 1.5));
            for x in [w * 0.5 + 1.5, w * 0.5 + 12.5] {
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, d - 11.0, 1.5, 3.0, 1.5));
            }
        }
        Kit::Fair => {
            let n = 3;
            for i in 0..n {
                let x0 = 2.0 + i as f32 * (w - 4.0) / n as f32;
                let x1 = x0 + (w - 4.0) / n as f32 - 2.0;
                let z0 = if i == 1 { d - 30.0 } else { d - 16.0 };
                stall(b, [x0, z0.max(2.0), x1, z0.max(2.0) + 14.0], i as u32 + 7);
                b.cut();
            }
        }
        Kit::Stage => {
            let (x0, z0, x1, z1) = (4.0, d - 26.0, w - 4.0, d - 3.0);
            b.add(bx(Kind::Wood, x0, z0, x1, z1, 0.0, 6.0));
            b.add(bx(Kind::Cloth, x0, z0, x1, z0 + 2.0, 6.0, 16.0));
            for x in [x0 + 1.0, x1 - 1.0] {
                b.add(sh(Shape::Box, Kind::Wood, x, 6.0, z1 - 1.0, 2.0, 16.0, 2.0));
                b.add(bx(Kind::Cloth, x - 1.0, z0 + 2.0, x + 1.0, z1 - 2.0, 6.0, 14.0));
            }
            b.add(bx(Kind::Cloth, x0, z1 - 2.0, x1, z1, 20.0, 3.0));
        }
        Kit::Laundry => {
            cottage_in(b, [2.0, d - 22.0, w * 0.45, d - 2.0], 11.0, 8.0, 1, true);
            b.cut();
            for (i, x) in [w * 0.55, w * 0.7].into_iter().enumerate() {
                b.add(sh(Shape::Cylinder, Kind::Wood, x, 0.0, d - 7.0 - i as f32 * 3.0, 8.0, 5.0, 8.0));
                b.add(sh(Shape::Cylinder, Kind::Water, x, 0.0, d - 7.0 - i as f32 * 3.0, 6.5, 5.25, 6.5));
                b.cut();
            }
            for x in [w * 0.52, w - 3.0] {
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, d - 22.0, 1.5, 14.0, 1.5));
            }
            b.add(bx(Kind::Cloth, w * 0.55, d - 22.4, w * 0.68, d - 21.6, 7.0, 6.0));
            b.add(bx(Kind::Plaster, w * 0.72, d - 22.4, w - 5.0, d - 21.6, 8.0, 5.0));
        }
        Kit::Shrine => {
            // A find's is 14 across; a wish's wider lot builds it bigger.
            let k = ((w - 2.0) / 14.0).clamp(0.6, 1.6);
            let (cx, z1) = (w / 2.0, d - 1.0);
            let (hw, deep) = (7.0 * k, (8.0 * k).min(d - 2.0));
            b.add(bx(Kind::Stone, cx - hw, z1 - deep, cx + hw, z1, 0.0, 1.5));
            let (x0, x1, z0, zf) = (cx - hw + 2.0 * k, cx + hw - 2.0 * k, z1 - deep + 1.0, z1 - 1.0);
            let wall_h = 7.0 * k;
            b.add(bx(Kind::Stone, x0, z0, x1, zf, 1.5, wall_h));
            b.add(sh(Shape::Gable, Kind::Slate, cx, 1.5 + wall_h, (z0 + zf) / 2.0, zf - z0 + 1.5, 4.5 * k, x1 - x0 + 2.0).turned(90.0));
            b.add(sh(Shape::Box, Kind::Door, cx, 1.5, zf + 0.25, 3.5 * k, 5.0 * k, 1.0));
        }
        Kit::Stones => {
            // Five stones in an arc opening to the front, the tallest at its back.
            let k = ((w - 6.0) / 34.0).clamp(0.0, 1.0);
            for (dx, dz, h) in [(-17.0, 0.0, 11.0), (-9.0, -4.0, 14.0), (0.0, -6.0, 15.0), (9.0, -4.0, 13.0), (17.0, 0.0, 10.0)] {
                let (x, z) = (w / 2.0 + dx * k, (d - 2.5 + dz * k).max(2.0));
                b.add(bx(Kind::Rock, x - 2.0, z - 1.5, x + 2.0, z + 1.5, 0.0, h));
                b.cut();
            }
        }
        Kit::Cave => {
            // A mound of rock, its dark mouth in its front.
            b.add(sh(Shape::Rock(seed16(b.r(80))), Kind::Rock, w / 2.0, 0.0, d / 2.0, w - 2.0, 11.0, d - 2.0));
            b.add(sh(Shape::Box, Kind::Door, w / 2.0, 0.0, d - 2.0, (w * 0.45).min(11.0), 6.5, 2.0));
        }
        Kit::Campfire => {
            let (x, z) = (w / 2.0, d / 2.0);
            let log = (w - 2.0).min(10.0);
            for turn in [25.0, -25.0] {
                b.add(sh(Shape::Box, Kind::Trunk, x, 0.0, z, log, 2.5, 2.5).turned(turn));
            }
            b.add(sh(Shape::Cone, Kind::Fire, x, 1.0, z, 4.5, 6.5, 4.5));
        }
        Kit::Pack => {
            let k = (w / 16.0).min(1.0);
            let z1 = d - 1.0;
            b.add(bx(Kind::Wood, 1.0, z1 - 5.0, 1.0 + 8.0 * k, z1, 0.0, 7.0));
            b.add(bx(Kind::Cloth, 1.5, z1 - 4.5, 0.5 + 8.0 * k, z1 - 0.5, 7.0, 1.5));
            b.add(sh(Shape::Cylinder, Kind::Wood, (1.0 + 10.5 * k).min(w - 3.5), 0.0, z1 - 3.0, 1.2, 13.0, 1.2));
            b.add(sh(Shape::Cylinder, Kind::Cloth, (1.0 + 13.0 * k).min(w - 2.0), 0.0, z1 - 2.5, 3.5, 6.0, 3.5));
        }
        Kit::LeanTo => {
            let (x0, x1, z0, z1) = (1.0, w - 1.0, (d - 11.0).max(1.0), d - 1.0);
            // High at the front, sloping down to the ground behind.
            b.add(sh(Shape::Wedge, Kind::Thatch, (x0 + x1) / 2.0, 0.0, (z0 + z1) / 2.0, x1 - x0, 10.0, z1 - z0).turned(180.0));
            b.add(sh(Shape::Box, Kind::Door, (x0 + x1) / 2.0, 0.0, z1 + 0.25, (x1 - x0) * 0.45, 6.5, 1.0));
            for x in [x0 + 0.75, x1 - 0.75] {
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, z1 - 0.75, 1.5, 11.0, 1.5));
            }
        }
        Kit::Stubs => {
            let z = (d - 5.0).max(3.0);
            let n = ((w - 2.0) / 7.0).floor().max(2.0) as usize;
            let step = (w - 2.0) / n as f32;
            for i in 0..n {
                let x0 = 1.0 + i as f32 * step;
                let h = b.range(30 + i as u32, 4.0, 10.0);
                b.add(bx(Kind::Stone, x0, z - 2.0, x0 + step - 2.0, z + 2.0, 0.0, h));
            }
            b.cut();
            rock_heap(b, [2.0, (d - 4.0).max(1.0), w - 2.0, d - 0.5], 3.0, 3, 90);
        }
        Kit::Kites => {
            cottage_in(b, [2.0, d - 26.0, w * 0.6, d - 2.0], 13.0, 10.0, 2, true);
            b.cut();
            for (i, x) in [w * 0.72, w * 0.88].into_iter().enumerate() {
                let z = d - 8.0 - i as f32 * 10.0;
                let h = 20.0 + i as f32 * 6.0;
                b.add(sh(Shape::Box, Kind::Wood, x, 0.0, z, 1.2, h, 1.2));
                b.add(sh(Shape::Gem, Kind::Cloth, x, h - 2.0, z, 8.0, 10.0, 1.0));
                b.cut();
            }
        }
    }
}

/// A market stall over `[x0, z0, x1, z1]`, its counter at the front.
fn stall(b: &mut B, [x0, z0, x1, z1]: [f32; 4], k: u32) {
    b.add(bx(Kind::Wood, x0 + 0.5, z1 - 4.0, x1 - 0.5, z1 - 0.5, 0.0, 5.0));
    b.add(bx(b.pick(20 + k, &[Kind::Crop, Kind::Cloth, Kind::Wood, Kind::Metal]), x0 + 1.5, z1 - 3.5, x1 - 1.5, z1 - 1.0, 5.0, 1.5));
    for x in [x0 + 0.75, x1 - 0.75] {
        for z in [z0 + 0.75, z1 - 0.75] {
            b.add(sh(Shape::Box, Kind::Wood, x, 0.0, z, 1.5, 11.0, 1.5));
        }
    }
    b.add(sh(Shape::Wedge, Kind::Cloth, (x0 + x1) / 2.0, 11.0, (z0 + z1) / 2.0, x1 - x0 + 1.0, 3.5, z1 - z0 + 1.0));
}

/// A chapel over `rect`: a nave with its gable to the front, a tower at the
/// front with a spire, lancets down its side.
fn chapel(b: &mut B, [x0, z0, x1, z1]: [f32; 4]) {
    let walls = b.pick(14, &[Kind::Stone, Kind::Stone, Kind::Plaster]);
    let roof = b.pick(15, &[Kind::Slate, Kind::Slate, Kind::Roof]);
    let nave_w = ((x1 - x0) * 0.62).min(20.0);
    let (nx0, nx1) = (x0, x0 + nave_w);
    b.add(bx(walls, nx0, z0, nx1, z1 - 4.0, 0.0, 16.0));
    b.add(sh(Shape::Gable, roof, (nx0 + nx1) / 2.0, 16.0, (z0 + z1 - 4.0) / 2.0, z1 - 4.0 - z0 + 2.0, 11.0, nave_w + 3.0).turned(90.0));
    for k in 0..2 {
        let z = z0 + (z1 - 4.0 - z0) * (k as f32 + 1.0) / 3.0;
        b.add(sh(Shape::Box, Kind::Window, nx1 + 0.25, 6.0, z, 1.0, 6.0, 2.5));
    }
    b.add(sh(Shape::Arch, Kind::Window, (nx0 + nx1) / 2.0, 7.0, z1 - 4.0 + 0.25, 5.0, 7.0, 1.0));
    let t = ((x1 - x0) - nave_w - 1.0).clamp(8.0, 12.0);
    let (tx, tz) = ((nx1 + t / 2.0 + 0.5).min(x1 - t / 2.0), z1 - t / 2.0);
    b.add(bx(walls, tx - t / 2.0, tz - t / 2.0, tx + t / 2.0, tz + t / 2.0, 0.0, 26.0));
    b.add(sh(Shape::Pyramid, roof, tx, 26.0, tz, t + 1.5, 20.0, t + 1.5));
    b.add(sh(Shape::Box, Kind::Door, tx, 0.0, tz + t / 2.0 + 0.25, 5.0, 10.0, 1.0));
    slot(b, tx, 18.0, tz + t / 2.0 + 0.25);
}

/// A castle of the March over the piece: a curtain wall, towers at its
/// corners, the keep at its back, the gate in its front.
fn castle(b: &mut B, c: Castle) {
    let (w, d) = (b.w, b.d);
    let (x0, z0, x1, z1) = (4.0, 4.0, w - 4.0, d - 3.5);
    let t = 3.0;
    let wall_h = 14.0;
    // The keep first: it stands at the back, over the walls.
    let kw = ((x1 - x0) * 0.45).min(20.0);
    let (kx, kz) = ((x0 + x1) / 2.0, z0 + 7.0);
    b.add(bx(c.walls, kx - kw / 2.0, z0 + 1.0, kx + kw / 2.0, z0 + 13.0, 0.0, 28.0));
    match c.top {
        Top::Crenel => crenels(b, (kx - kw / 2.0, z0 + 12.25), (kx + kw / 2.0, z0 + 12.25), 1.5, 28.0),
        Top::Dome(kind) => b.add(sh(Shape::Sphere, kind, kx, 24.0, kz, kw * 0.7, kw * 0.6, 10.0)),
        Top::Cone(kind) => b.add(sh(Shape::Hip, kind, kx, 28.0, kz, kw + 2.0, 10.0, 14.0)),
    }
    slot(b, kx, 20.0, z0 + 13.25);
    b.cut();
    // The curtain wall.
    b.add(bx(c.walls, x0, z0, x1, z0 + t, 0.0, wall_h));
    b.add(bx(c.walls, x0, z0 + t, x0 + t, z1, 0.0, wall_h));
    b.add(bx(c.walls, x1 - t, z0 + t, x1, z1, 0.0, wall_h));
    let gate = 12.0;
    let (gx0, gx1) = ((x0 + x1) / 2.0 - gate / 2.0, (x0 + x1) / 2.0 + gate / 2.0);
    b.add(bx(c.walls, x0 + t, z1 - t, gx0, z1, 0.0, wall_h));
    b.add(bx(c.walls, gx1, z1 - t, x1 - t, z1, 0.0, wall_h));
    crenels(b, (x0 + t, z1 - 0.75), (gx0, z1 - 0.75), 1.5, wall_h);
    crenels(b, (gx1, z1 - 0.75), (x1 - t, z1 - 0.75), 1.5, wall_h);
    // The gate: an arch in a block a little taller than the wall.
    b.add(sh(Shape::Arch, c.walls, (gx0 + gx1) / 2.0, 0.0, z1 - t / 2.0, gate, wall_h + 4.0, t + 1.0));
    b.add(sh(Shape::Box, Kind::Door, (gx0 + gx1) / 2.0, 0.0, z1 - t / 2.0, gate * 0.6 - 0.5, (wall_h + 4.0) * 0.8 - 0.5, t * 0.5));
    b.cut();
    // The towers, the front pair always, the back pair for four.
    let r = 4.5;
    let mut corners = vec![(x0 + r - 1.0, z1 - r + 1.0), (x1 - r + 1.0, z1 - r + 1.0)];
    if c.towers >= 4 {
        corners.extend([(x0 + r - 1.0, z0 + r - 1.0), (x1 - r + 1.0, z0 + r - 1.0)]);
    }
    for (x, z) in corners {
        tower(b, x, z, r, 22.0, c.walls, c.top);
        slot(b, x, 14.0, z + r - 0.25);
        b.cut();
    }
}

/// The arena: a ring of arched wall round a sand floor, tiers inside it,
/// its gate at the front.
fn colosseum(b: &mut B) {
    let (w, d) = (b.w, b.d);
    let (cx, cz) = (w / 2.0, d / 2.0);
    let (rx, rz) = (w / 2.0 - 4.0, d / 2.0 - 4.0);
    b.add(sh(Shape::Cylinder, Kind::Sand, cx, 0.0, cz, 2.0 * rx - 16.0, 0.5, 2.0 * rz - 16.0));
    b.add(sh(Shape::Cylinder, Kind::Stone, cx, 0.0, cz, 2.0 * rx - 8.0, 6.0, 2.0 * rz - 8.0));
    b.add(sh(Shape::Cylinder, Kind::Sand, cx, 0.0, cz, 2.0 * rx - 18.0, 6.25, 2.0 * rz - 18.0));
    b.cut();
    let n = 28;
    for k in 0..n {
        let a = (k as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        let (x, z) = (cx + rx * c, cz + rz * s);
        // The segment's length: the ring's step there.
        let a2 = (k as f32 + 1.5) / n as f32 * std::f32::consts::TAU;
        let (s2, c2) = a2.sin_cos();
        let len = ((rx * (c2 - c)).powi(2) + (rz * (s2 - s)).powi(2)).sqrt() + 0.8;
        // Tangent's direction, as a turn: the segment's x along it.
        let (tx, tz) = (-rx * s, rz * c);
        let turn = (-tz).atan2(tx).to_degrees();
        let front = s > 0.95;
        if front {
            // The gate.
            b.add(sh(Shape::Arch, Kind::Stone, x, 0.0, z - 1.0, len + 4.0, 30.0, 6.0));
            b.add(sh(Shape::Box, Kind::Door, x, 0.0, z - 1.0, (len + 4.0) * 0.6 - 0.5, 23.5, 2.0));
        } else {
            b.add(sh(Shape::Arch, Kind::Stone, x, 0.0, z, len, 24.0, 5.0).turned(turn));
        }
        b.cut();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn every_kit_piece_keeps_within_its_footprint_whichever_way_it_faces() {
        for kit in all() {
            let sizes: &[(f32, f32)] = match kit {
                Kit::Well | Kit::Quintain => &[(16.0, 16.0)],
                Kit::Stall | Kit::Turret | Kit::Rookery => &[(16.0, 32.0), (16.0, 48.0), (32.0, 16.0)],
                Kit::Colosseum => &[(160.0, 96.0)],
                Kit::Board | Kit::Garden | Kit::Stable | Kit::Gatehouse | Kit::DelveGate => &[(48.0, 16.0), (32.0, 16.0), (32.0, 32.0), (48.0, 48.0)],
                // A find on its tile, as `world3d::lots` stands it; a wish's lot.
                Kit::Shrine => &[(16.0, 12.0), (32.0, 16.0), (32.0, 32.0)],
                Kit::Stones => &[(40.0, 12.0)],
                Kit::Cave => &[(24.0, 14.0)],
                Kit::Campfire => &[(12.0, 10.0)],
                Kit::Pack => &[(16.0, 8.0)],
                Kit::LeanTo => &[(20.0, 12.0)],
                Kit::Stubs => &[(28.0, 10.0)],
                Kit::School | Kit::College => &[(80.0, 64.0), (128.0, 64.0)],
                Kit::Cottage | Kit::Barn | Kit::Hall | Kit::Forge | Kit::Workshop | Kit::Ward | Kit::Chapel | Kit::RoundTable
                | Kit::Observatory | Kit::MineMouth | Kit::Pavilion | Kit::Windmill | Kit::Tower | Kit::Castle(_) => {
                    &[(64.0, 48.0), (32.0, 32.0), (48.0, 32.0), (48.0, 48.0)]
                }
                _ => &[(64.0, 48.0), (48.0, 48.0)],
            };
            for face in [Face::South, Face::East, Face::North, Face::West] {
                for &(fw, fd) in sizes {
                    let spot = Spot { foot: [100.0, 200.0, 100.0 + fw, 200.0 + fd], face, seed: 7 };
                    for s in build(kit, &spot).into_iter().flatten() {
                        let [a, b, c, d] = s.footprint();
                        assert!(
                            a >= 100.0 - 1.0 && b >= 200.0 - 1.0 && c <= 100.0 + fw + 1.0 && d <= 200.0 + fd + 1.0,
                            "{kit:?} {face:?} {fw}x{fd}: {:?} is outside its footprint",
                            super::super::blockout::line(&s)
                        );
                        assert!(s.size.iter().all(|v| *v >= 0.0), "{kit:?} {face:?} {fw}x{fd}: {:?} has a negative size", super::super::blockout::line(&s));
                    }
                }
            }
        }
    }

    /// Every kit, a castle of each top.
    pub fn all() -> Vec<Kit> {
        let mut kits = vec![
            Kit::Cottage, Kit::Barn, Kit::Hall, Kit::Forge, Kit::Workshop, Kit::Tower, Kit::Turret, Kit::Rookery,
            Kit::Gatehouse, Kit::Keep, Kit::Windmill, Kit::Watermill, Kit::Stall, Kit::Well, Kit::Lighthouse,
            Kit::Chapel, Kit::Priory, Kit::RoundTable, Kit::Observatory, Kit::MineMouth, Kit::DragonKeep,
            Kit::Colosseum, Kit::Pavilion, Kit::Quintain, Kit::Board, Kit::Garden, Kit::School, Kit::College,
            Kit::DelveGate, Kit::Hippodrome, Kit::Aqueduct, Kit::Belfry, Kit::Bothy, Kit::Ruins, Kit::Crag,
            Kit::Cliffs, Kit::Eyrie, Kit::Aviary, Kit::Bathhouse, Kit::LandOffice, Kit::Ranch, Kit::Stable,
            Kit::Orchard, Kit::HedgeMaze, Kit::Timberyard, Kit::Saltworks, Kit::Strand, Kit::SunkenAbbey,
            Kit::Quay, Kit::Causeway, Kit::TollBridge, Kit::Sluice, Kit::Flowerbeds, Kit::Downs, Kit::Flags,
            Kit::Caravan, Kit::Wheelwright, Kit::Butts, Kit::Scarecrow, Kit::Fletchery, Kit::GoosePen, Kit::Fair,
            Kit::Stage, Kit::Laundry, Kit::Kites, Kit::Ward, Kit::Shrine, Kit::Stones, Kit::Cave, Kit::Campfire,
            Kit::Pack, Kit::LeanTo, Kit::Stubs,
        ];
        for top in [Top::Crenel, Top::Dome(Kind::Slate), Top::Cone(Kind::Roof)] {
            for towers in [2, 4] {
                kits.push(Kit::Castle(Castle { walls: Kind::Stone, top, towers }));
            }
        }
        kits
    }
}
