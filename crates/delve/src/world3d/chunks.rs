//! The kingdom as blockout sets, one to a chunk of [`CHUNK`] tiles
//! (docs/WORLD-3D.md, "Set files"; docs/KINGDOM-OPEN.md, "Size and fixed
//! numbers"): the geometry half of projection painting for the Delve's
//! kingdom map. The map is the truth, read through its data model (the
//! terrain bytes, the geography's walking tiles and roads, the sites, the
//! structures and castles with their footprints), never through where
//! things stand today, so a new layout regenerates the same way:
//!
//! - the ground is merged slabs by terrain: grass, road, path, sand, mud,
//!   crop fields (furrowed), ash, and water and lava lowered under the
//!   floor line, their banks the land's edge;
//! - woods are groves: trunks under round or pointed crowns, seeded in
//!   size, kind and place, a big tree where four wood tiles meet, a bush
//!   in the thick of it, a hedge where the trees run one deep;
//! - rock is raised in steps, higher the further from its edge, with
//!   boulders on it; dead trees stand on the ash;
//! - a road over water is a bridge (a deck, its arches, rails on the water
//!   side);
//! - every place, castle and structure gets a kit building (`kit`) sized
//!   to its footprint and facing its door;
//! - the map's rim and any other blocked ground nothing stands on is hedge.
//!
//! The game's rules hold as they do for a room (`blockout`): no walkable
//! tile is under anything taller than a boot, save what spans high over
//! it, and none is more than half hidden from the chunk's camera, so what
//! stands before open ground is sized for the camera (`blockout::trim`).
//!
//! The camera is the Delve's stage camera at its tilt, on a stage the
//! chunk's size ([`STAGE`], 24 x 16 tiles of 16 of the kingdom's pixels;
//! a knight's view of the kingdom is 24 x 14 of them), centred on the
//! chunk's middle (`camera chunk 384 256 centre <x> <z>`): the chunks'
//! stages tile the kingdom, so neighbours' captures and plates meet at
//! their shared edges (docs/WORLD-3D.md, "Seams"). Units are the
//! kingdom's pixels, 16 to a tile, as the room sets' are.
//!
//! Fingerposts (`world::signs`) stand in their chunks, a slender post and
//! its arms a knight passes. What a run places or moves (the finds from
//! the run's seed, the party's wishes, figures, pads, animals) is never in
//! a set, and its tiles are plain ground here: the party's builds and the
//! finds stand up as extra blocks of the run's (`world3d::lots`), the
//! figures as cards.

use std::fmt::Write as _;

use super::blockout::{
    self, Group, HEADROOM, LOW, PLATE, Rect, Role, SLAB, Shape, Solid, TILE, View, merge, num, occluders,
};
use super::kit::{self, Castle, Face, Kit, Spot, Top};
use crate::render::overworld::ink::{hash, vnoise};
use crate::rules::{RoomKind, Tile};
use crate::world::houses::{self, RoofKind, Tops, Walls};
use crate::world::kingdom::Geography;
use crate::world::map::{self, Place, Realm, STRUCTURES, place_tile};
use crate::world::dressing::{self, Item as Dress, Piece};
use crate::world::signs::{self, Way};
use crate::world::{college, wishes};

pub use super::blockout::Kind;

/// A chunk's tiles: across, down (the map's own cut, `map::CHUNK_W` x
/// `map::CHUNK_H`).
pub const CHUNK: (i32, i32) = (map::CHUNK_W, map::CHUNK_H);
/// A chunk's stage: the chunk itself, in the kingdom's pixels, so the
/// chunks' stages tile the kingdom's floor edge to edge and two
/// neighbours' cameras are one stage apart (docs/WORLD-3D.md, "Seams").
pub const STAGE: (u32, u32) = ((CHUNK.0 * 16) as u32, (CHUNK.1 * 16) as u32);
/// How far water and lava lie under the floor line.
const WATER_DROP: f32 = 3.0;
const LAVA_DROP: f32 = 2.0;
/// A bridge's deck: its top, a boot high at most.
const DECK: f32 = 2.5;
/// How far a crown or a hedge may reach over open ground at its side, and
/// over blocked ground.
const SPILL_OPEN: f32 = 1.5;
const SPILL_BLOCKED: f32 = 5.0;
/// The height of a step of rock.
const ROCK_STEP: f32 = 6.0;

// ------------------------------------------------------------- the land ----

/// One thing the kit stands on the map: a place's building, a castle, a
/// structure.
#[derive(Clone, Debug)]
pub struct Build {
    /// What it is called on the map, or what it is.
    pub name: String,
    /// The place it belongs to, when it is one of a place's buildings.
    pub place: Option<String>,
    pub kit: Kit,
    /// Its tiles: column, row, width, height.
    pub foot: Rect,
    pub face: Face,
    pub seed: u32,
}

/// The kingdom as the generator reads it: the geography's tiles and roads,
/// the terrain under them, and what stands where.
pub struct Land<'a> {
    pub geo: &'a Geography,
    pub cols: i32,
    pub rows: i32,
    pub builds: Vec<Build>,
    /// The fingerposts: the tile each stands on, the road tile it marks,
    /// the ways its arms point.
    pub posts: Vec<((i32, i32), (i32, i32), Vec<Way>)>,
    /// The realm's set dressing (`world::dressing`): fixed for the realm,
    /// on open ground a knight walks through.
    pub pieces: Vec<Piece>,
    /// Each tile's build, by its index in `builds`.
    built: Vec<Option<usize>>,
}

/// What a tile is covered with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    /// Ground a knight walks, or ground a building stands on.
    Open,
    Woods,
    DeadTree,
    Rock,
    Water,
    Lava,
    /// A road over water.
    Bridge,
    /// Blocked ground nothing else stands on: the map's rim.
    Hedge,
}

impl<'a> Land<'a> {
    pub fn of(geo: &'a Geography) -> Self {
        let (cols, rows) = (geo.room.cols as i32, geo.room.rows as i32);
        let builds = builds(geo);
        let mut built = vec![None; (cols * rows) as usize];
        for (i, b) in builds.iter().enumerate() {
            let (c, r, w, h) = b.foot;
            for y in r.max(0)..(r + h).min(rows) {
                for x in c.max(0)..(c + w).min(cols) {
                    built[(y * cols + x) as usize].get_or_insert(i);
                }
            }
        }
        let posts = signs::posts().iter().map(|p| (p.at, p.road, p.arms.iter().map(|a| a.0).collect())).collect();
        let pieces = dressing::get().pieces.clone();
        Self { geo, cols, rows, builds, posts, pieces, built }
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.cols && y < self.rows
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.cols + x) as usize
    }

    /// The terrain byte at a tile; off the map, nothing.
    pub fn terrain(&self, x: i32, y: i32) -> u8 {
        if self.inside(x, y) { Realm::get().at(x, y) } else { 0 }
    }

    pub fn tile(&self, x: i32, y: i32) -> Tile {
        self.geo.room.tile(x, y)
    }

    /// A tile a knight walks on.
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.tile(x, y) == Tile::Floor
    }

    fn road(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.geo.roads[self.index(x, y)]
    }

    fn cleared(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.geo.cleared_mask[self.index(x, y)]
    }

    /// The build standing on a tile.
    pub fn built(&self, x: i32, y: i32) -> Option<&Build> {
        if !self.inside(x, y) {
            return None;
        }
        self.built[self.index(x, y)].map(|i| &self.builds[i])
    }

    /// The chunks across and down.
    pub fn grid(&self) -> (i32, i32) {
        ((self.cols + CHUNK.0 - 1) / CHUNK.0, (self.rows + CHUNK.1 - 1) / CHUNK.1)
    }

    /// A chunk's tiles, clipped to the map.
    pub fn chunk_rect(&self, cx: i32, cz: i32) -> Rect {
        let (c, r) = (cx * CHUNK.0, cz * CHUNK.1);
        (c, r, CHUNK.0.min(self.cols - c), CHUNK.1.min(self.rows - r))
    }

    /// The chunk a build belongs to: the one under its footprint's middle.
    pub fn owner(&self, b: &Build) -> (i32, i32) {
        let (c, r, w, h) = b.foot;
        ((c + w / 2).div_euclid(CHUNK.0), (r + h / 2).div_euclid(CHUNK.1))
    }

    fn water(&self, x: i32, y: i32) -> bool {
        matches!(self.terrain(x, y), b'~' | b'w' | b'f')
    }

    /// What covers a tile.
    pub fn cover(&self, x: i32, y: i32) -> Cover {
        if !self.inside(x, y) {
            return Cover::Hedge;
        }
        let walk = self.walkable(x, y);
        let t = self.terrain(x, y);
        if self.built(x, y).is_some() {
            return match t {
                b'~' | b'w' | b'f' if !walk => Cover::Water,
                _ => Cover::Open,
            };
        }
        let near_water = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| self.water(x + dx, y + dy));
        match t {
            b'~' | b'w' | b'f' => {
                if walk {
                    Cover::Bridge
                } else {
                    Cover::Water
                }
            }
            b'L' | b'F' => Cover::Lava,
            b'H' if near_water => Cover::Bridge,
            b'T' if !walk && !self.cleared(x, y) => Cover::Woods,
            b'd' if !walk && !self.cleared(x, y) => Cover::DeadTree,
            b'^' if !walk && !self.cleared(x, y) => Cover::Rock,
            _ if walk => Cover::Open,
            b'T' => Cover::Woods,
            b'd' => Cover::DeadTree,
            b'^' => Cover::Rock,
            _ => Cover::Hedge,
        }
    }

    /// The ground's kind at a tile, and its top.
    pub fn ground(&self, x: i32, y: i32) -> (Kind, f32) {
        let t = self.terrain(x, y);
        let lane = self.walkable(x, y) && self.road(x, y);
        match self.cover(x, y) {
            Cover::Water | Cover::Bridge => return (Kind::Water, -WATER_DROP),
            Cover::Lava => return (Kind::Fire, -LAVA_DROP),
            Cover::Rock => return (Kind::Rock, 0.0),
            _ => {}
        }
        let trail = self.walkable(x, y) && dressing::get().trail(x, y);
        let kind = match t {
            b':' => Kind::Road,
            b'=' => Kind::Path,
            b'H' => Kind::Wood,
            _ if lane || trail => Kind::Path,
            b's' => Kind::Sand,
            b'%' => Kind::Mud,
            b'c' => Kind::Crop,
            b'a' => Kind::Ground,
            b'd' => {
                if [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| self.terrain(x + dx, y + dy) == b'%') {
                    Kind::Mud
                } else {
                    Kind::Ground
                }
            }
            b'^' => Kind::Ground,
            _ => Kind::Grass,
        };
        (kind, 0.0)
    }
}

/// The face of `foot` that `stand` is before.
pub(crate) fn face_to((c, r, w, h): Rect, (sx, sy): (i32, i32)) -> Face {
    if sy >= r + h {
        Face::South
    } else if sy < r {
        Face::North
    } else if sx >= c + w {
        Face::East
    } else {
        Face::West
    }
}

pub(crate) fn seed_of((c, r, w, h): Rect) -> u32 {
    hash(c * 7 + w, r * 5 + h, 977)
}

/// The kit for a structure of the map, by its footprint in the authored
/// screens' own tiles (`map::STRUCTURES`, as `render::overworld::scene`
/// draws each), with what it is.
fn authored(foot: Rect) -> Option<(Kit, &'static str)> {
    Some(match foot {
        (22, 12, 3, 3) => (Kit::Keep, "the Keep"),
        (21, 12, 1, 3) | (25, 12, 1, 3) => (Kit::Turret, "a keep turret"),
        (26, 12, 1, 2) => (Kit::Rookery, "the Rookery"),
        (28, 12, 2, 2) => (Kit::Chapel, "the Chapel"),
        (17, 12, 2, 2) => (Kit::RoundTable, "the Round Table"),
        (17, 17, 2, 2) => (Kit::Forge, "the Smithy"),
        (28, 17, 2, 2) => (Kit::Hall, "the Scriptorium"),
        (25, 17, 1, 1) => (Kit::Well, "the well"),
        (20, 17, 1, 2) => (Kit::Stall, "the market stalls"),
        (26, 20, 2, 1) => (Kit::Garden, "the garden"),
        (5, 14, 2, 2) => (Kit::Gatehouse, "the Gatehouse"),
        (9, 2, 2, 2) => (Kit::Observatory, "the Observatory"),
        (23, 1, 2, 2) => (Kit::MineMouth, "the mine mouth"),
        (39, 2, 3, 3) => (Kit::DragonKeep, "Dragon Keep"),
        (36, 19, 1, 1) | (58, 40, 1, 1) => (Kit::Quintain, "the quintain"),
        (41, 19, 3, 1) => (Kit::Board, "the standings board"),
        (42, 26, 2, 2) => (Kit::Windmill, "the windmill"),
        (18, 24, 2, 2) | (21, 24, 2, 2) | (25, 24, 2, 2) | (28, 24, 2, 2) => (Kit::Cottage, "a cottage"),
        (18, 29, 2, 2) => (Kit::Forge, "the forge"),
        (21, 29, 2, 2) => (Kit::Workshop, "the weaver's"),
        (24, 29, 2, 2) => (Kit::Workshop, "the cooper's"),
        (27, 29, 2, 2) => (Kit::Barn, "the granary"),
        (30, 29, 1, 1) | (30, 36, 1, 1) => (Kit::Well, "the well"),
        (51, 13, 2, 2) | (56, 13, 2, 2) | (58, 19, 2, 2) => (Kit::Ward, "a ward"),
        (17, 34, 2, 2) | (20, 34, 2, 2) | (25, 34, 2, 2) | (28, 34, 2, 2) => (Kit::Cottage, "an artisan's cottage"),
        (17, 40, 3, 2) => (Kit::Forge, "the smith's working bay"),
        (21, 40, 2, 2) => (Kit::Workshop, "the loom"),
        (24, 40, 2, 2) => (Kit::Workshop, "the cooper's"),
        (27, 40, 3, 2) => (Kit::Barn, "the granary"),
        (35, 34, 10, 6) => (Kit::Colosseum, "the Colosseum"),
        (50, 34, 2, 2) | (60, 34, 2, 2) => (Kit::Pavilion, "a pavilion"),
        (60, 40, 2, 1) => (Kit::Stable, "the tournament stable"),
        (9, 34, 5, 4) => (Kit::School, "the Sloptomizer School"),
        _ => return None,
    })
}

/// A structure no table names: what its size suggests.
fn by_size((_, _, w, h): Rect) -> (Kit, &'static str) {
    match (w, h) {
        (1, 1) => (Kit::Well, "a well"),
        (1, _) | (_, 1) => (Kit::Stall, "stalls"),
        (w, h) if w >= 6 && h >= 5 => (Kit::Hall, "a hall"),
        (w, h) if w * h >= 9 => (Kit::Hall, "a hall"),
        _ => (Kit::Cottage, "a cottage"),
    }
}

/// The kit for a countryside place (a site with its own lot), by what it is.
pub fn kit_of(room: RoomKind) -> Kit {
    match room {
        RoomKind::Lighthouse => Kit::Lighthouse,
        RoomKind::Sluice => Kit::Sluice,
        RoomKind::Aviary => Kit::Aviary,
        RoomKind::Kitewright => Kit::Kites,
        RoomKind::LongbowGreen => Kit::Butts,
        RoomKind::Scarecrow => Kit::Scarecrow,
        RoomKind::Bathhouse => Kit::Bathhouse,
        RoomKind::Fletchery => Kit::Fletchery,
        RoomKind::GooseGreen => Kit::GoosePen,
        RoomKind::Fairground => Kit::Fair,
        RoomKind::Mummers => Kit::Stage,
        RoomKind::Laundry => Kit::Laundry,
        RoomKind::HedgeMaze => Kit::HedgeMaze,
        RoomKind::Bothy => Kit::Bothy,
        RoomKind::Timberyard => Kit::Timberyard,
        RoomKind::Orchard => Kit::Orchard,
        RoomKind::Ruins => Kit::Ruins,
        RoomKind::Crag => Kit::Crag,
        RoomKind::Belfry => Kit::Belfry,
        RoomKind::Aqueduct => Kit::Aqueduct,
        RoomKind::Priory => Kit::Priory,
        RoomKind::Cliffs => Kit::Cliffs,
        RoomKind::Saltworks => Kit::Saltworks,
        RoomKind::Strand => Kit::Strand,
        RoomKind::Millpond => Kit::Watermill,
        RoomKind::SunkenAbbey => Kit::SunkenAbbey,
        RoomKind::Riverbank => Kit::Quay,
        RoomKind::Causeway => Kit::Causeway,
        RoomKind::TollBridge => Kit::TollBridge,
        RoomKind::Pasture => Kit::Flowerbeds,
        RoomKind::Downs => Kit::Downs,
        RoomKind::SignalHill => Kit::Flags,
        RoomKind::Caravan => Kit::Caravan,
        RoomKind::Wheelwright => Kit::Wheelwright,
        RoomKind::Windmill => Kit::Windmill,
        RoomKind::Hippodrome => Kit::Hippodrome,
        RoomKind::Eyrie => Kit::Eyrie,
        RoomKind::Ranch => Kit::Ranch,
        RoomKind::LandOffice => Kit::LandOffice,
        _ => Kit::Cottage,
    }
}

/// A castle as its house builds it.
fn castle_of(house: &houses::House) -> Castle {
    let roof = match house.roof {
        RoofKind::Slate => Kind::Slate,
        RoofKind::Tile => Kind::Roof,
        RoofKind::Thatch => Kind::Thatch,
    };
    Castle {
        walls: match house.walls {
            Walls::PaleStone | Walls::DarkStone => Kind::Stone,
            Walls::Plaster => Kind::Plaster,
            Walls::Timber => Kind::Wood,
        },
        top: match house.tops {
            Tops::Battlements => Top::Crenel,
            Tops::Domes => Top::Dome(if roof == Kind::Thatch { Kind::Slate } else { roof }),
            Tops::Pointed => Top::Cone(roof),
        },
        towers: if house.towers >= 3 { 4 } else { 2 },
    }
}

/// Everything the kit stands on the map: the structures, the castles, the
/// College, the Delve's gate and every site's lot.
fn builds(geo: &Geography) -> Vec<Build> {
    let mut out = Vec::new();
    let place_of = |(x, y): (i32, i32)| {
        Place::ALL.into_iter().find(|p| {
            let (c, r, w, h) = p.footprint_world();
            x >= c && x < c + w && y >= r && y < r + h
        })
    };
    for &(ax, ay, w, h) in STRUCTURES.iter() {
        let (c, r) = place_tile(ax, ay);
        let foot = (c, r, w, h);
        let (kit, what) = authored((ax, ay, w, h)).unwrap_or_else(|| by_size(foot));
        // In a place's footprint, or against it (the keep's turrets).
        let place = place_of((c + w / 2, r + h / 2)).or_else(|| {
            Place::ALL.into_iter().find(|p| {
                let (pc, pr, pw, ph) = p.footprint_world();
                pw * ph > 1 && c <= pc + pw && c + w >= pc && r <= pr + ph && r + h >= pr
            })
        });
        // A place's own building faces its door; one of a place's many, or
        // a structure on its own, faces the front.
        let face = match place {
            Some(p) if p.footprint_world() == foot => face_to(foot, p.stand_world()),
            _ => Face::South,
        };
        out.push(Build {
            name: what.to_string(),
            place: place.map(|p| title(p.label())),
            kit,
            foot,
            face,
            seed: seed_of(foot),
        });
    }
    for id in houses::ids() {
        let house = houses::get(id);
        let foot = houses::footprint(id);
        out.push(Build {
            name: house.castle.clone(),
            place: Some(format!("the house of {}", house.family)),
            kit: Kit::Castle(castle_of(house)),
            foot,
            face: face_to(foot, houses::stand(id)),
            seed: seed_of(foot),
        });
    }
    let lot = college::lot();
    let foot = (lot.tx, lot.ty, lot.tw, lot.th);
    out.push(Build {
        name: "Labyrinth College".into(),
        place: None,
        kit: Kit::College,
        foot,
        face: face_to(foot, college::stand()),
        seed: seed_of(foot),
    });
    let gate = wishes::delve_gate();
    let foot = (gate.tx, gate.ty, gate.tw, gate.th);
    out.push(Build {
        name: "the Delve's gate".into(),
        place: None,
        kit: Kit::DelveGate,
        foot,
        face: face_to(foot, wishes::delve_stand()),
        seed: seed_of(foot),
    });
    for site in &geo.sites {
        let Some(lot) = site.lot else {
            continue;
        };
        let foot = (lot.tx, lot.ty, lot.tw, lot.th);
        out.push(Build {
            name: site.name.clone(),
            place: None,
            kit: kit_of(site.room),
            foot,
            face: face_to(foot, site.stand),
            seed: seed_of(foot),
        });
    }
    for b in &mut out {
        keep_off_open(geo, b);
    }
    out
}

/// A footprint that holds ground a knight walks (a road through a gate's
/// tiles) gives it up: the build stands on the rest, its front to the
/// open ground.
fn keep_off_open(geo: &Geography, b: &mut Build) {
    let (c, r, w, h) = b.foot;
    let walk = |x: i32, y: i32| geo.room.tile(x, y) == Tile::Floor;
    let cells: Vec<(i32, i32)> = (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).collect();
    if !cells.iter().any(|&(x, y)| walk(x, y)) {
        return;
    }
    let blocked: Vec<(i32, i32)> = cells.iter().copied().filter(|&(x, y)| !walk(x, y)).collect();
    let Some(&(x0, _)) = blocked.iter().min_by_key(|p| p.0) else {
        // All of it is walked on (the Delve's yard before its gate): the
        // build stands in the blocked ground behind it, facing the yard.
        let behind: Vec<i32> = (c..c + w).filter(|&x| !walk(x, r - 1) && r > 1).collect();
        if let (Some(&a), Some(&z)) = (behind.first(), behind.last())
            && (a..=z).all(|x| !walk(x, r - 1))
        {
            b.foot = (a, r - 1, z - a + 1, 1);
            b.face = Face::South;
        }
        return;
    };
    let x1 = blocked.iter().map(|p| p.0).max().unwrap_or(x0);
    let y0 = blocked.iter().map(|p| p.1).min().unwrap_or(r);
    let y1 = blocked.iter().map(|p| p.1).max().unwrap_or(r);
    let inner = (x0, y0, x1 - x0 + 1, y1 - y0 + 1);
    if (y0..=y1).any(|y| (x0..=x1).any(|x| walk(x, y))) {
        return;
    }
    // The open ground's middle, for the face.
    let open: Vec<(i32, i32)> = cells.iter().copied().filter(|&(x, y)| walk(x, y)).collect();
    let mid = (open.iter().map(|p| p.0).sum::<i32>() / open.len() as i32, open.iter().map(|p| p.1).sum::<i32>() / open.len() as i32);
    b.face = face_to(inner, mid);
    b.foot = inner;
}

/// A label in capitals as a name: "THE LISTS" as "The Lists".
fn title(label: &str) -> String {
    label
        .split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_string() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ----------------------------------------------------------- the chunk ----

/// The chunk's camera: the kingdom's stage, centred on the chunk's middle.
pub fn centre(cx: i32, cz: i32) -> [f32; 2] {
    [(cx * CHUNK.0) as f32 * TILE + CHUNK.0 as f32 * TILE / 2.0, (cz * CHUNK.1) as f32 * TILE + CHUNK.1 as f32 * TILE / 2.0]
}

/// The set's id.
pub fn chunk_id(cx: i32, cz: i32) -> String {
    format!("chunk-{cx}-{cz}")
}

/// The chunk `(cx, cz)` of the kingdom as a set file.
pub fn chunk_set(geo: &Geography, cx: i32, cz: i32) -> String {
    write_set(&Land::of(geo), cx, cz)
}

/// The chunk `(cx, cz)` of `land` as a set file ([`chunk_set`] of a land
/// made once).
pub fn write_set(land: &Land, cx: i32, cz: i32) -> String {
    write(land, cx, cz, &chunk(land, cx, cz))
}

/// The walkable tiles the chunk's camera must keep in view: its own and a
/// margin round it (what stands at its edge hides its neighbour's ground).
pub fn open_tiles(land: &Land, cx: i32, cz: i32) -> Vec<(i32, i32)> {
    let (c, r, w, h) = land.chunk_rect(cx, cz);
    (r - 4..r + h + 1)
        .flat_map(|y| (c - 2..c + w + 2).map(move |x| (x, y)))
        .filter(|&(x, y)| land.inside(x, y) && land.walkable(x, y))
        .collect()
}

/// The chunk's blockout: its ground, its builds, its woods and rock, its
/// bridges and hedges, each sized for the camera.
pub fn chunk(land: &Land, cx: i32, cz: i32) -> Vec<Group> {
    let view = View::chunk(STAGE, centre(cx, cz));
    let open = open_tiles(land, cx, cz);
    let rect = land.chunk_rect(cx, cz);
    let mut placed = Vec::new();
    let mut groups = vec![ground(land, rect)];
    groups.extend(furrows(land, rect));
    for (i, b) in land.builds.iter().enumerate() {
        if land.owner(b) != (cx, cz) {
            continue;
        }
        let group = building(land, i, b, &view, &open, &mut placed);
        groups.push(group);
    }
    let mut rest = vec![
        fingerposts(land, rect),
        dressed(land, rect),
        hedges(land, rect),
        rock(land, rect),
        woods(land, rect),
        dead_trees(land, rect),
        bridges(land, rect),
    ];
    for group in &mut rest {
        if group.role == Role::Ground {
            continue;
        }
        blockout::trim(group, &view, &open, &mut placed);
    }
    for mut group in rest {
        if group.name == "rock" {
            join_steps(&mut group);
        }
        if group.name == "hedges" || group.name == "bridges" {
            group.solids = blockout::join_runs(std::mem::take(&mut group.solids));
            group.parts = vec![0];
        }
        if !group.solids.is_empty() {
            groups.push(group);
        }
    }
    blockout::name_ids(&mut groups);
    groups
}

/// The ground under the chunk: one slab to a run of equal tiles, water and
/// lava lowered.
fn ground(land: &Land, (c, r, w, h): Rect) -> Group {
    let cells: Vec<Option<(Kind, i32)>> = (r..r + h)
        .flat_map(|y| (c..c + w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (kind, top) = land.ground(x, y);
            Some((kind, (top * 2.0) as i32))
        })
        .collect();
    let solids = merge(w as usize, h as usize, &cells)
        .into_iter()
        .map(|((mc, mr, mw, mh), (kind, top))| {
            let top = top as f32 / 2.0;
            Solid::block(kind, blockout::px((c + mc, r + mr, mw, mh)), -SLAB, SLAB + top)
        })
        .collect();
    Group::new(Role::Ground, "ground", solids)
}

/// Furrows on the crop fields: low ridges across each field, a boot high.
fn furrows(land: &Land, (c, r, w, h): Rect) -> Option<Group> {
    let cells: Vec<Option<bool>> = (r..r + h)
        .flat_map(|y| (c..c + w).map(move |x| (x, y)))
        .map(|(x, y)| (land.ground(x, y).0 == Kind::Crop).then_some(true))
        .collect();
    let mut solids = Vec::new();
    for ((mc, mr, mw, mh), _) in merge(w as usize, h as usize, &cells) {
        let [x0, z0, x1, z1] = blockout::px((c + mc, r + mr, mw, mh));
        let mut z = z0 + 2.5;
        while z + 2.0 <= z1 - 1.0 {
            solids.push(Solid::block(Kind::Crop, [x0 + 1.0, z, x1 - 1.0, z + 2.0], 0.0, 2.5));
            z += 5.0;
        }
    }
    (!solids.is_empty()).then(|| Group::new(Role::Prop, "crop furrows", solids))
}

/// A build's kit piece, its parts each sized for the camera on their own;
/// nothing it stands of is left over open ground its footprint holds (a
/// gate's way through).
fn building(land: &Land, i: usize, b: &Build, view: &View, open_tiles: &[(i32, i32)], placed: &mut Vec<blockout::Hull>) -> Group {
    let (c, r, w, h) = b.foot;
    let open: Vec<[f32; 4]> = (r..r + h)
        .flat_map(|y| (c..c + w).map(move |x| (x, y)))
        .filter(|&(x, y)| land.walkable(x, y))
        .map(|(x, y)| blockout::px((x, y, 1, 1)))
        .collect();
    let over_open = |s: &Solid| {
        let [a, bb, cc, d] = s.footprint();
        s.top() > LOW
            && s.bottom() < HEADROOM
            && open.iter().any(|t| a < t[2] - 2.0 && cc > t[0] + 2.0 && bb < t[3] - 2.0 && d > t[1] + 2.0)
    };
    let name = match &b.place {
        Some(place) => format!("{} of {}: {} at {c},{r} {w}x{h}", b.name, place, b.kit.what()),
        None => format!("{}: {} at {c},{r} {w}x{h}", b.name, b.kit.what()),
    };
    // The piece on its whole footprint, and drawn forward onto its front
    // tiles (the back left to the yard): a building with open ground
    // behind it keeps more of its height the less deep it stands. The one
    // that keeps the most, for what it gives up in depth, stands.
    let [x0, z0, x1, z1] = blockout::px(b.foot);
    let deep = match b.face {
        Face::South | Face::North => h,
        Face::East | Face::West => w,
    };
    let mut best: Option<(f32, Group, Vec<blockout::Hull>)> = None;
    for back in 0..deep.min(3) {
        let k = back as f32 * TILE;
        let foot = match b.face {
            Face::South => [x0, z0 + k, x1, z1],
            Face::North => [x0, z0, x1, z1 - k],
            Face::East => [x0 + k, z0, x1, z1],
            Face::West => [x0, z0, x1 - k, z1],
        };
        let mut parts = kit::build(b.kit, &Spot { foot, face: b.face, seed: b.seed });
        // A piece too big for the shallower ground is no variant.
        let fits = parts.iter().flatten().all(|s| {
            let [a, bb, cc, d] = s.footprint();
            s.size.iter().all(|v| *v >= 0.0) && a >= foot[0] - 1.0 && bb >= foot[1] - 1.0 && cc <= foot[2] + 1.0 && d <= foot[3] + 1.0
        });
        if back > 0 && !fits {
            continue;
        }
        for part in &mut parts {
            part.retain(|s| !over_open(s));
        }
        if let Some(first) = parts.iter_mut().flat_map(|p| p.iter_mut()).next() {
            first.id = Some(slug(&b.name, i));
        }
        let mut group = Group::of_parts(Role::Prop, name.clone(), parts);
        group.rect = Some(b.foot);
        let mut trial = placed.clone();
        blockout::trim(&mut group, view, open_tiles, &mut trial);
        let score = group.kept * ((deep - back) as f32 / deep as f32).sqrt();
        if best.as_ref().is_none_or(|(s, _, _)| score > *s + 0.02) {
            best = Some((score, group, trial));
        }
        if group_whole(&best) {
            break;
        }
    }
    let (_, mut group, trial) = best.expect("a piece on its footprint");
    *placed = trial;
    // What the piece leaves of its footprint bare is its yard: bushes, a
    // small tree, a stack, so blocked ground reads as blocked.
    let hulls = occluders(view, &group.solids);
    let bare: Vec<(i32, i32)> = (r..r + h)
        .flat_map(|y| (c..c + w).map(move |x| (x, y)))
        .filter(|&(x, y)| !land.walkable(x, y) && !covers(view, &group.solids, &hulls, (x, y)))
        .collect();
    if !bare.is_empty() {
        // Before the front, only what stays low; trees go behind and beside.
        let front = |(x, y): (i32, i32)| match b.face {
            Face::South => y == r + h - 1,
            Face::North => y == r,
            Face::East => x == c + w - 1,
            Face::West => x == c,
        };
        let mut yard = Group::of_parts(Role::Prop, "yard", bare.iter().map(|&t| yard_at(land, t, front(t))).collect());
        blockout::trim(&mut yard, view, open_tiles, placed);
        let at = group.solids.len();
        group.parts.extend(yard.parts.iter().map(|p| p + at));
        group.solids.extend(yard.solids);
        group.kept = group.kept.min(yard.kept);
    }
    group
}

/// Build `b` (what a run adds to the kingdom, `world3d::lots`) as chunk
/// `(cx, cz)`'s generator stands a build there: its kit piece on its
/// footprint, facing its door, sized for the chunk's camera, and what it
/// leaves of its footprint bare dressed as a yard.
pub fn stand(land: &Land, b: &Build, (cx, cz): (i32, i32)) -> Group {
    let view = View::chunk(STAGE, centre(cx, cz));
    let open = open_tiles(land, cx, cz);
    building(land, 0, b, &view, &open, &mut Vec::new())
}

/// Whether the best piece so far kept all its height: no need to draw it
/// forward.
fn group_whole(best: &Option<(f32, Group, Vec<blockout::Hull>)>) -> bool {
    best.as_ref().is_some_and(|(_, g, _)| g.kept >= 0.999)
}

/// Whether `solids` cover a tile: something standing over its floor, or
/// hiding a good part of it from the camera.
pub fn covers(view: &View, solids: &[Solid], hulls: &[blockout::Hull], (x, y): (i32, i32)) -> bool {
    let [x0, z0, _, _] = blockout::px((x, y, 1, 1));
    let under = (0..4).any(|j| {
        (0..4).any(|i| {
            let (px, pz) = (x0 + 3.0 + 2.5 * (i as f32 + 0.5), z0 + 3.0 + 2.5 * (j as f32 + 0.5));
            solids.iter().any(|s| {
                let [a, b, c, d] = s.footprint();
                s.top() > 1.0 && px > a && px < c && pz > b && pz < d
            })
        })
    });
    under || view.hidden(hulls, (x, y)) > 0.3
}

/// A yard's dressing on a bare tile of a footprint: a bush, a small tree
/// or a stack of timber, seeded.
fn yard_at(land: &Land, (x, y): (i32, i32), low: bool) -> Vec<Solid> {
    let v = hash(x, y, 91);
    let v = if low && v % 5 >= 2 && v % 5 <= 3 { v - 2 } else { v };
    let [rw, rn, re, rs] = reach(land, x, y);
    let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
    let (lo_x, hi_x, lo_z, hi_z) = (x0 - rw.min(1.0), x1 + re.min(1.0), z0 - rn.min(1.0), z1 + rs.min(1.0));
    let (cx, cz) = ((lo_x + hi_x) / 2.0, (lo_z + hi_z) / 2.0);
    match v % 5 {
        0 | 1 => vec![Solid::new(Shape::Blob(((v >> 4) % 64) as u16 + 1), Kind::Foliage, [cx, 0.0, cz], [13.0, if low { 6.0 } else { 8.0 + (v >> 3 & 3) as f32 }, 12.0])],
        2 | 3 => tree(cx, cz, 14.0, v, false, false),
        _ => vec![
            Solid::block(Kind::Wood, [lo_x + 2.0, cz - 4.0, hi_x - 2.0, cz + 4.0], 0.0, 4.0),
            Solid::block(Kind::Wood, [lo_x + 4.0, cz - 3.0, hi_x - 4.0, cz + 3.0], 4.0, 3.0),
        ],
    }
}

/// A set id from a name: its words, lower case, joined by `-`.
fn slug(name: &str, i: usize) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty() && !matches!(w.to_ascii_lowercase().as_str(), "the" | "a" | "an" | "of" | "s"))
        .map(|w| w.to_ascii_lowercase())
        .collect();
    let id = words.join("-");
    let id: String = id.chars().take(40).collect();
    if id.is_empty() { format!("build-{i}") } else { id.trim_end_matches('-').to_string() }
}

/// How far a thing on tile `(x, y)` may reach past each side: west, north,
/// east, south.
pub(crate) fn reach(land: &Land, x: i32, y: i32) -> [f32; 4] {
    let open = |nx: i32, ny: i32| !land.inside(nx, ny) || land.walkable(nx, ny) || land.built(nx, ny).is_some();
    // A side is open when the tile across it, or either tile at its ends,
    // is: what reaches over a side reaches over its corners too.
    let side = |dx: i32, dy: i32| {
        let ends = if dx != 0 { [(dx, -1), (dx, 1)] } else { [(-1, dy), (1, dy)] };
        if open(x + dx, y + dy) || ends.iter().any(|&(ex, ey)| open(x + ex, y + ey)) { SPILL_OPEN } else { SPILL_BLOCKED }
    };
    [side(-1, 0), side(0, -1), side(1, 0), side(0, 1)]
}

/// Each tile of `rect` that `want` holds.
fn tiles_of(land: &Land, (c, r, w, h): Rect, want: Cover) -> Vec<(i32, i32)> {
    (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).filter(|&(x, y)| land.cover(x, y) == want).collect()
}

/// The fingerposts on the verges: a slender post a knight passes, its arms
/// pointing down the roads, a cap on top.
pub(crate) fn fingerposts(land: &Land, (c, r, w, h): Rect) -> Group {
    let mut parts = Vec::new();
    for (at, road, ways) in &land.posts {
        if !(at.0 >= c && at.0 < c + w && at.1 >= r && at.1 < r + h) {
            continue;
        }
        let [x0, z0, x1, z1] = blockout::px((at.0, at.1, 1, 1));
        // On the tile's side toward the road it marks.
        let (dx, dz) = ((road.0 - at.0).signum() as f32, (road.1 - at.1).signum() as f32);
        let (x, z) = ((x0 + x1) / 2.0 + dx * 4.0, (z0 + z1) / 2.0 + dz * 4.0);
        let top = 19.0;
        let mut part = vec![
            Solid::new(Shape::Box, Kind::Wood, [x, 0.0, z], [1.5, top, 1.5]),
            Solid::new(Shape::Pyramid, Kind::Wood, [x, top, z], [2.2, 1.5, 2.2]),
        ];
        for (k, way) in ways.iter().enumerate() {
            let y = top - 3.0 - k as f32 * 2.5;
            let (ax, az, sx, sz) = match way {
                Way::East => (x + 4.0, z, 6.5, 0.6),
                Way::West => (x - 4.0, z, 6.5, 0.6),
                Way::North => (x, z - 4.0, 0.6, 6.5),
                Way::South => (x, z + 4.0, 0.6, 6.5),
            };
            part.push(Solid::new(Shape::Box, Kind::Wood, [ax, y, az], [sx, 1.8, sz]));
        }
        parts.push(part);
    }
    let mut g = Group::of_parts(Role::Prop, "fingerposts", parts);
    g.rect = Some((c, r, w, h));
    g
}

/// Whether a solid is slender: a post or a pole a knight passes, which
/// may stand on open ground (a fingerpost, a cross, a scarecrow's pole).
pub fn slender(s: &Solid) -> bool {
    let [a, b, c, d] = s.footprint();
    (c - a).min(d - b) <= 2.5 && (c - a).max(d - b) <= 7.0
}

/// The realm's set dressing in the chunk (`world::dressing`): the small
/// things between the places, each on open ground a knight walks
/// through, so each is boot-high, or a slender post.
pub(crate) fn dressed(land: &Land, (c, r, w, h): Rect) -> Group {
    let mut parts = Vec::new();
    for p in &land.pieces {
        if p.at.0 >= c && p.at.0 < c + w && p.at.1 >= r && p.at.1 < r + h {
            parts.push(dress(p));
        }
    }
    let mut g = Group::of_parts(Role::Prop, "set dressing", parts);
    g.rect = Some((c, r, w, h));
    g
}

/// One piece of dressing as blocks: boot-high, or slender.
fn dress(p: &Piece) -> Vec<Solid> {
    let (tx, ty) = p.at;
    let reach = p.item.reach();
    let [x0, z0, x1, _] = blockout::px((tx - reach, ty, 1 + 2 * reach, 1));
    let (cx, cz) = ((x0 + x1) / 2.0, z0 + TILE / 2.0);
    let f = if p.flip { -1.0 } else { 1.0 };
    let v = hash(tx, ty, 101);
    let bx = |kind: Kind, x: f32, z: f32, sx: f32, sy: f32, sz: f32| Solid::new(Shape::Box, kind, [x, 0.0, z], [sx, sy, sz]);
    let at = |shape: Shape, kind: Kind, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32| Solid::new(shape, kind, [x, y, z], [sx, sy, sz]);
    let post = |kind: Kind, x: f32, z: f32, hgt: f32| Solid::new(Shape::Box, kind, [x, 0.0, z], [1.5, hgt, 1.5]);
    let seed = |k: u32| ((v >> k) % 64) as u16 + 1;
    match p.item {
        Dress::FallenLog => vec![bx(Kind::Trunk, cx, cz, (x1 - x0) - 8.0, 3.5, 4.0).turned(f * 6.0)],
        Dress::StumpAxe => vec![
            at(Shape::Cylinder, Kind::Trunk, cx, 0.0, cz, 7.0, 3.0, 7.0),
            bx(Kind::Wood, cx + f * 1.5, cz, 6.0, 0.5, 1.0).turned(30.0),
            at(Shape::Box, Kind::Metal, cx + f * 4.0, 3.0, cz, 2.0, 1.0, 2.0),
        ],
        Dress::MushroomRing => (0..7)
            .map(|k| {
                let (sn, cs) = (k as f32 / 7.0 * std::f32::consts::TAU).sin_cos();
                at(Shape::Sphere, Kind::Plaster, cx + cs * 5.0, 0.0, cz + sn * 4.0, 2.5, 2.0, 2.5)
            })
            .collect(),
        Dress::Hide => vec![at(Shape::Wedge, Kind::Foliage, cx, 0.0, cz, 12.0, 4.0, 9.0), post(Kind::Wood, cx - 5.0, cz - 3.5, 9.0)],
        Dress::Woodpile => vec![bx(Kind::Wood, cx, cz, 13.0, 2.0, 7.0), at(Shape::Box, Kind::Trunk, cx, 2.0, cz, 11.0, 2.0, 6.0)],
        Dress::Cairn => vec![
            at(Shape::Rock(seed(1)), Kind::Rock, cx, 0.0, cz, 8.0, 2.5, 7.0),
            at(Shape::Rock(seed(7)), Kind::Rock, cx, 2.0, cz, 5.0, 2.0, 4.5),
        ],
        Dress::Fold => {
            let (a, b) = (x0 + 2.0, x1 - 2.0);
            vec![
                Solid::block(Kind::Stone, [a, z0 + 2.0, b, z0 + 4.0], 0.0, 4.0),
                Solid::block(Kind::Stone, [a, z0 + 4.0, a + 2.0, z0 + 14.0], 0.0, 4.0),
                Solid::block(Kind::Stone, [b - 2.0, z0 + 4.0, b, z0 + 14.0], 0.0, 4.0),
                Solid::block(Kind::Stone, [a + 2.0, z0 + 12.0, cx - 4.0, z0 + 14.0], 0.0, 3.5),
            ]
        }
        Dress::Scree => (0..4)
            .map(|k| {
                let u = hash(tx + k, ty, 103);
                at(Shape::Rock(seed(k as u32 * 5)), Kind::Rock, cx - 5.0 + (u % 11) as f32, 0.0, cz - 4.0 + (u >> 4) as f32 % 9.0, 4.0, 2.0, 3.5)
            })
            .collect(),
        Dress::WatchStub => vec![
            at(Shape::Cylinder, Kind::Stone, cx, 0.0, cz, 12.0, 4.0, 12.0),
            at(Shape::Rock(seed(3)), Kind::Stone, cx + f * 7.0, 0.0, cz + 4.0, 4.0, 2.0, 3.0),
        ],
        Dress::Bones => vec![
            bx(Kind::Bone, cx - 2.0, cz, 7.0, 1.0, 1.5).turned(f * 25.0),
            bx(Kind::Bone, cx + 2.0, cz + 2.0, 5.0, 1.0, 1.2).turned(f * -40.0),
            at(Shape::Sphere, Kind::Bone, cx + f * 4.0, 0.0, cz - 2.0, 3.0, 2.5, 3.0),
        ],
        Dress::BurntCart => vec![
            bx(Kind::Trunk, cx, cz, 13.0, 3.5, 7.0).turned(f * 10.0),
            at(Shape::Sphere, Kind::Trunk, cx + f * 5.0, 0.0, cz + 4.0, 6.0, 1.5, 6.0),
        ],
        Dress::AshPit => vec![
            at(Shape::Cylinder, Kind::Rock, cx, 0.0, cz, 10.0, 1.5, 9.0),
            at(Shape::Cylinder, Kind::Ground, cx, 0.0, cz, 7.0, 1.75, 6.0),
        ],
        Dress::Boat => vec![at(Shape::Gable, Kind::Wood, cx, 0.0, cz, (x1 - x0) - 14.0, 4.0, 9.0).turned(f * 8.0)],
        Dress::Nets => vec![
            post(Kind::Wood, cx - 12.0, cz, 12.0),
            post(Kind::Wood, cx, cz, 12.0),
            post(Kind::Wood, cx + 12.0, cz, 12.0),
            bx(Kind::Cloth, cx, cz + 2.5, 26.0, 1.0, 4.0),
        ],
        Dress::Driftwood => vec![
            bx(Kind::Trunk, cx, cz, 10.0, 1.5, 2.0).turned(f * 20.0),
            bx(Kind::Trunk, cx + f * 3.0, cz + 3.0, 7.0, 1.5, 1.5).turned(f * -35.0),
        ],
        Dress::TidePool => vec![
            at(Shape::Rock(seed(2)), Kind::Rock, cx - 6.0, 0.0, cz, 5.0, 2.5, 5.0),
            at(Shape::Cylinder, Kind::Water, cx, 0.0, cz, 9.0, 0.5, 7.0),
            at(Shape::Rock(seed(9)), Kind::Rock, cx + 6.0, 0.0, cz + 1.0, 4.0, 2.0, 4.0),
        ],
        Dress::Punt => vec![bx(Kind::Wood, cx, cz, (x1 - x0) - 12.0, 1.5, 7.0).turned(f * -7.0)],
        Dress::EelTraps => (0..3).map(|k| at(Shape::Cylinder, Kind::Wood, cx - 5.0 + k as f32 * 5.0, 0.0, cz + (k % 2) as f32 * 2.0, 3.5, 3.0, 3.5)).collect(),
        Dress::Reeds => vec![at(Shape::Cone, Kind::Thatch, cx, 0.0, cz, 9.0, 4.0, 8.0)],
        Dress::StiltHut => vec![
            post(Kind::Wood, cx - 5.0, cz - 4.0, 10.0),
            post(Kind::Wood, cx + 5.0, cz - 4.0, 10.0),
            bx(Kind::Wood, cx, cz + 1.0, 14.0, 2.5, 10.0),
            at(Shape::Gable, Kind::Thatch, cx, 2.5, cz + 1.0, 15.0, 1.5, 11.0),
        ],
        Dress::Stooks => (0..3).map(|k| at(Shape::Cone, Kind::Thatch, cx - 5.0 + k as f32 * 5.0, 0.0, cz + (k % 2) as f32 * 3.0 - 1.5, 4.5, 4.0, 4.5)).collect(),
        Dress::Scarecrow => vec![
            post(Kind::Wood, cx, cz, 14.0),
            at(Shape::Box, Kind::Wood, cx, 10.0, cz, 7.0, 1.0, 1.0),
            at(Shape::Box, Kind::Cloth, cx, 6.0, cz, 2.5, 4.0, 1.5),
        ],
        Dress::Stile => vec![bx(Kind::Stone, cx, cz, 9.0, 4.0, 3.0), bx(Kind::Stone, cx, cz + 2.5, 6.0, 2.0, 2.0)],
        Dress::Hives => (0..3).map(|k| at(Shape::Cone, Kind::Thatch, cx - 5.0 + k as f32 * 5.0, 0.0, cz, 4.0, 4.0, 4.0)).collect(),
        Dress::Rail => vec![
            post(Kind::Wood, x0 + 4.0, cz, 4.0),
            post(Kind::Wood, x1 - 4.0, cz, 4.0),
            Solid::block(Kind::Wood, [x0 + 3.0, cz - 0.6, x1 - 3.0, cz + 0.6], 2.75, 1.25),
        ],
        Dress::LanceRack => vec![
            bx(Kind::Wood, cx, cz, 10.0, 3.0, 3.0),
            post(Kind::Wood, cx - 3.0, cz, 13.0),
            post(Kind::Wood, cx + 2.0, cz, 9.0),
        ],
        Dress::Wagon => vec![
            Solid::block(Kind::Wood, [x0 + 6.0, cz - 5.0, x1 - 6.0, cz + 5.0], 0.0, 2.5),
            at(Shape::Gable, Kind::Cloth, cx, 2.5, cz, (x1 - x0) - 14.0, 1.5, 9.0),
        ],
        Dress::Bunting => vec![post(Kind::Wood, cx - 4.0, cz, 16.0), post(Kind::Wood, cx + 4.0, cz, 16.0)],
        Dress::Barrels => (0..3).map(|k| at(Shape::Cylinder, Kind::Wood, cx - 4.0 + (k % 2) as f32 * 7.0, 0.0, cz - 2.0 + (k / 2) as f32 * 5.0, 5.0, 4.0, 5.0)).collect(),
        Dress::BurntPost => vec![post(Kind::Trunk, cx, cz, 13.0), at(Shape::Box, Kind::Trunk, cx + f * 2.5, 9.0, cz, 4.0, 1.0, 1.0)],
        Dress::Milestone => vec![at(Shape::Box, Kind::Stone, cx, 0.0, cz, 2.5, 6.0, 2.5)],
        Dress::Cross => vec![post(Kind::Stone, cx, cz, 13.0), at(Shape::Box, Kind::Stone, cx, 9.0, cz, 6.0, 1.5, 1.2)],
        Dress::Barrow => vec![bx(Kind::Wood, cx, cz, 10.0, 3.5, 6.0), at(Shape::Sphere, Kind::Wood, cx + f * 6.0, 0.0, cz, 1.2, 4.0, 4.0)],
        Dress::Trough => vec![bx(Kind::Wood, cx, cz, 12.0, 3.5, 5.0), at(Shape::Box, Kind::Water, cx, 0.0, cz, 10.0, 3.75, 3.0)],
        Dress::Stocks => vec![post(Kind::Wood, cx - 4.0, cz, 9.0), post(Kind::Wood, cx + 4.0, cz, 9.0), bx(Kind::Wood, cx, cz, 9.0, 3.0, 2.0)],
        Dress::Stall => vec![
            bx(Kind::Wood, cx, cz + 3.0, 12.0, 4.0, 4.0),
            post(Kind::Wood, cx - 6.0, cz - 4.0, 12.0),
            post(Kind::Wood, cx + 6.0, cz - 4.0, 12.0),
        ],
        Dress::Sacks => (0..3).map(|k| at(Shape::Sphere, Kind::Cloth, cx - 4.0 + k as f32 * 4.0, 0.0, cz + (k % 2) as f32 * 2.0, 4.0, 4.0, 3.5)).collect(),
        Dress::Garden => {
            let (a, b) = (x0 + 3.0, x1 - 3.0);
            vec![
                Solid::block(Kind::Hedge, [a, z0 + 2.0, b, z0 + 4.0], 0.0, 3.5),
                Solid::block(Kind::Hedge, [a, z0 + 12.0, b, z0 + 14.0], 0.0, 3.0),
                Solid::block(Kind::Crop, [a + 1.0, z0 + 5.0, b - 1.0, z0 + 11.0], 0.0, 2.0),
            ]
        }
    }
}

/// The map's rim and blocked ground nothing stands on: hedge.
fn hedges(land: &Land, rect: Rect) -> Group {
    let mut parts = Vec::new();
    for (x, y) in tiles_of(land, rect, Cover::Hedge) {
        let [w, n, e, s] = reach(land, x, y);
        let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
        let v = hash(x, y, 31);
        parts.push(hedge_on([x0 - w.min(1.0), z0 - n.min(1.0), x1 + e.min(1.0), z1 + s.min(1.0)], v));
    }
    let mut g = Group::of_parts(Role::Wall, "hedges", parts);
    g.rect = Some(rect);
    g
}

/// A tile of hedge over `[x0, z0, x1, z1]`: a clipped body, a lumpy top.
fn hedge_on([x0, z0, x1, z1]: [f32; 4], v: u32) -> Vec<Solid> {
    let body = 6.0 + (v % 2) as f32;
    vec![
        Solid::block(Kind::Hedge, [x0, z0, x1, z1], 0.0, body),
        Solid::new(Shape::Blob(((v >> 3) % 64) as u16 + 1), Kind::Hedge, [(x0 + x1) / 2.0, body - 2.5, (z0 + z1) / 2.0], [x1 - x0, 5.0 + (v >> 6 & 1) as f32, z1 - z0]),
    ]
}

/// How deep in its rock a tile is: 1 at the edge, up to 4.
fn rock_depth(land: &Land, x: i32, y: i32) -> i32 {
    for d in 1..4i32 {
        for dy in -d..=d {
            for dx in -d..=d {
                if (dx.abs() == d || dy.abs() == d) && land.inside(x + dx, y + dy) && land.cover(x + dx, y + dy) != Cover::Rock {
                    return d;
                }
            }
        }
    }
    4
}

/// Rock in steps, each tile its own part (its step and the boulder on it,
/// so they are sized together); the steps joined after.
fn rock(land: &Land, rect: Rect) -> Group {
    let mut parts = Vec::new();
    for (x, y) in tiles_of(land, rect, Cover::Rock) {
        let depth = rock_depth(land, x, y);
        let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
        let v = hash(x, y, 53);
        // A low base at the edge, steps further in; a boulder on every
        // tile, so the rock reads as lumps, not boxes.
        let step = if depth == 1 { 3.0 } else { (depth - 1) as f32 * ROCK_STEP + 3.0 };
        let mut part = vec![Solid::block(Kind::Rock, [x0, z0, x1, z1], 0.0, step)];
        let [rw, rn, re, rs] = reach(land, x, y);
        let want = if depth == 1 { 17.0 } else { 15.0 } - (v >> 3 & 3) as f32;
        let s = want.min(TILE + rw.min(rn).min(re).min(rs) * 2.0);
        let (jx, jz) = (((v >> 6) % 3) as f32 - 1.0, ((v >> 9) % 3) as f32 - 1.0);
        let bx = ((x0 + x1) / 2.0 + jx).clamp(x0 - rw + s / 2.0, (x1 + re - s / 2.0).max(x0 - rw + s / 2.0));
        let bz = ((z0 + z1) / 2.0 + jz).clamp(z0 - rn + s * 0.45, (z1 + rs - s * 0.45).max(z0 - rn + s * 0.45));
        let rh = if depth == 1 { 7.0 } else { 6.0 } + (v >> 12 & 3) as f32;
        part.push(
            Solid::new(Shape::Rock(((v >> 15) % 64) as u16 + 1), Kind::Rock, [bx, step - 1.5, bz], [s, rh, s * 0.9])
                .turned(((v >> 18) % 4) as f32 * 90.0),
        );
        parts.push(part);
    }
    let mut g = Group::of_parts(Role::Wall, "rock", parts);
    g.rect = Some(rect);
    g
}

/// Rock's steps joined where they stand level side by side.
fn join_steps(group: &mut Group) {
    let (steps, rest): (Vec<Solid>, Vec<Solid>) = std::mem::take(&mut group.solids)
        .into_iter()
        .partition(|s| s.shape == Shape::Box && s.bottom() == 0.0 && (s.size[0] - TILE).abs() < 1e-3 && (s.size[2] - TILE).abs() < 1e-3);
    if steps.is_empty() {
        group.solids = rest;
        return;
    }
    let col = |s: &Solid| ((s.at[0] - TILE / 2.0) / TILE).round() as i32;
    let row = |s: &Solid| ((s.at[2] - TILE / 2.0) / TILE).round() as i32;
    let (c0, r0) = (steps.iter().map(col).min().unwrap(), steps.iter().map(row).min().unwrap());
    let (c1, r1) = (steps.iter().map(col).max().unwrap(), steps.iter().map(row).max().unwrap());
    let (w, h) = ((c1 - c0 + 1) as usize, (r1 - r0 + 1) as usize);
    let mut cells = vec![None; w * h];
    for s in &steps {
        cells[(row(s) - r0) as usize * w + (col(s) - c0) as usize] = Some((s.top() * 2.0).round() as i32);
    }
    let mut solids: Vec<Solid> = merge(w, h, &cells)
        .into_iter()
        .map(|((c, r, mw, mh), top)| Solid::block(Kind::Rock, blockout::px((c0 + c, r0 + r, mw, mh)), 0.0, top as f32 / 2.0))
        .collect();
    solids.extend(rest);
    group.solids = solids;
    group.parts = vec![0];
}

/// The leaf of a grove at a tile: pines in their stands, broadleaf about.
fn pine_at(x: i32, y: i32) -> bool {
    vnoise(x as f32 / 7.0, y as f32 / 7.0, 61) < 0.36
}

/// A tree whose foot is at `(x, z)`, its crown `s` across at most.
fn tree(x: f32, z: f32, s: f32, v: u32, pine: bool, big: bool) -> Vec<Solid> {
    let trunk_kind = if v % 2 == 0 { Shape::Cylinder } else { Shape::Frustum };
    if pine {
        let trunk_h = 4.0 + (v >> 3 & 3) as f32;
        let h = if big { 40.0 + (v >> 6 & 7) as f32 } else { 26.0 + (v >> 6 & 7) as f32 };
        let mut out = vec![Solid::new(trunk_kind, Kind::Trunk, [x, 0.0, z], [if big { 4.5 } else { 3.0 }, trunk_h + 3.0, if big { 4.5 } else { 3.0 }])];
        out.push(Solid::new(Shape::Cone, Kind::Foliage, [x, trunk_h, z], [s, h * 0.62, s]));
        out.push(Solid::new(Shape::Cone, Kind::Foliage, [x, trunk_h + h * 0.38, z], [s * 0.72, h * 0.62, s * 0.72]));
        out
    } else {
        let trunk_h = if big { 10.0 + (v >> 3 & 3) as f32 } else { 6.0 + (v >> 3 & 3) as f32 };
        let crown = if big { 22.0 + (v >> 6 & 7) as f32 } else { 14.0 + (v >> 6 & 7) as f32 };
        vec![
            Solid::new(trunk_kind, Kind::Trunk, [x, 0.0, z], [if big { 5.0 } else { 3.0 }, trunk_h + 3.0, if big { 5.0 } else { 3.0 }]),
            Solid::new(Shape::Blob(((v >> 9) % 64) as u16 + 1), Kind::Foliage, [x, trunk_h - 2.0, z], [s, crown, s * 0.95])
                .turned(((v >> 15) % 4) as f32 * 90.0),
        ]
    }
}

/// A crown's breadth and middle on tile `(x, y)` (or the 2x2 from it),
/// kept within what the tile may reach over.
fn crown_on(land: &Land, (x, y): (i32, i32), n: i32, want: f32, v: u32) -> (f32, f32, f32) {
    let edge = |k: usize, tiles: &mut dyn Iterator<Item = (i32, i32)>| tiles.map(|(a, b)| reach(land, a, b)[k]).fold(f32::MAX, f32::min);
    let rw = edge(0, &mut (y..y + n).map(|b| (x, b)));
    let rn = edge(1, &mut (x..x + n).map(|a| (a, y)));
    let re = edge(2, &mut (y..y + n).map(|b| (x + n - 1, b)));
    let rs = edge(3, &mut (x..x + n).map(|a| (a, y + n - 1)));
    let [x0, z0, x1, z1] = blockout::px((x, y, n, n));
    let (lo_x, hi_x, lo_z, hi_z) = (x0 - rw, x1 + re, z0 - rn, z1 + rs);
    let s = want.min(hi_x - lo_x).min(hi_z - lo_z);
    let (jx, jz) = (((v >> 20) % 5) as f32 - 2.0, ((v >> 23) % 5) as f32 - 2.0);
    let cx = ((x0 + x1) / 2.0 + jx).clamp(lo_x + s / 2.0, hi_x - s / 2.0);
    let cz = ((z0 + z1) / 2.0 + jz).clamp(lo_z + s / 2.0, hi_z - s / 2.0);
    (cx, cz, s)
}

/// The woods: groves of trees, a big tree where four wood tiles meet (now
/// and then), bushes in the thick of it, hedges where trees run one deep.
fn woods(land: &Land, rect: Rect) -> Group {
    let (c, r, w, h) = rect;
    let wood = |x: i32, y: i32| land.cover(x, y) == Cover::Woods;
    let in_rect = |x: i32, y: i32| x >= c && y >= r && x < c + w && y < r + h;
    let mut done = std::collections::HashSet::new();
    let mut parts = Vec::new();
    let mut hedge = Vec::new();
    // Hedgerows: a line of wood one tile deep.
    for (x, y) in tiles_of(land, rect, Cover::Woods) {
        let (n, s, wst, e) = (wood(x, y - 1), wood(x, y + 1), wood(x - 1, y), wood(x + 1, y));
        let line = (!n && !s && (wst || e)) || (!wst && !e && (n || s));
        if line && hash(x, y, 71) % 6 != 0 {
            let [rw, rn, re, rs] = reach(land, x, y);
            let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
            let (a, b) = if !n && !s { (x0, x1) } else { (x0 + 2.0 - rw.min(2.0), x1 - 2.0 + re.min(2.0)) };
            let (cz0, cz1) = if !n && !s { (z0 + 2.0 - rn.min(2.0), z1 - 2.0 + rs.min(2.0)) } else { (z0, z1) };
            hedge.push(hedge_on([a, cz0, b, cz1], hash(x, y, 72)));
            done.insert((x, y));
        }
    }
    // Big trees on 2x2 of wood.
    for (x, y) in tiles_of(land, rect, Cover::Woods) {
        let four = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
        if four.iter().all(|&(a, b)| in_rect(a, b) && wood(a, b) && !done.contains(&(a, b))) && hash(x, y, 73) % 100 < 30 {
            let v = hash(x, y, 74);
            let pine = pine_at(x, y);
            let (cx, cz, s) = crown_on(land, (x, y), 2, if pine { 28.0 } else { 33.0 }, v);
            parts.push(tree(cx, cz, s, v, pine, true));
            done.extend(four);
        }
    }
    for (x, y) in tiles_of(land, rect, Cover::Woods) {
        if done.contains(&(x, y)) {
            continue;
        }
        let v = hash(x, y, 75);
        let thick = (-1..=1).all(|dy| (-1..=1).all(|dx| wood(x + dx, y + dy)));
        if thick && v % 100 < 14 {
            let (cx, cz, s) = crown_on(land, (x, y), 1, 15.0, v);
            parts.push(vec![Solid::new(Shape::Blob(((v >> 9) % 64) as u16 + 1), Kind::Foliage, [cx, 0.0, cz], [s, 9.0 + (v >> 4 & 3) as f32, s])]);
        } else {
            let pine = pine_at(x, y);
            let (cx, cz, s) = crown_on(land, (x, y), 1, 17.0 + (v >> 2 & 3) as f32, v);
            parts.push(tree(cx, cz, s, v, pine, false));
        }
    }
    parts.extend(hedge);
    let mut g = Group::of_parts(Role::Wall, "woods", parts);
    g.rect = Some(rect);
    g
}

/// Dead trees on the ash: a bare trunk and two crooked limbs.
fn dead_trees(land: &Land, rect: Rect) -> Group {
    let mut parts = Vec::new();
    for (x, y) in tiles_of(land, rect, Cover::DeadTree) {
        let v = hash(x, y, 81);
        let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
        let (cx, cz) = ((x0 + x1) / 2.0 + ((v % 5) as f32 - 2.0), (z0 + z1) / 2.0 + ((v >> 3) % 5) as f32 - 2.0);
        let h = 15.0 + (v >> 6 & 7) as f32;
        let turn = ((v >> 9) % 180) as f32;
        // Limbs reach one way each from the trunk, at three heights.
        let limb = |len: f32, y: f32, a: f32| {
            let (s, c) = a.to_radians().sin_cos();
            Solid::new(Shape::Box, Kind::Trunk, [cx + c * len / 2.0, y, cz - s * len / 2.0], [len, 1.2, 1.2]).turned(a)
        };
        parts.push(vec![
            Solid::new(Shape::Frustum, Kind::Trunk, [cx, 0.0, cz], [4.0, h, 4.0]),
            limb(5.5, h * 0.55, turn),
            limb(4.5, h * 0.72, turn + 150.0),
            limb(3.5, h * 0.88, turn + 250.0),
        ]);
    }
    let mut g = Group::of_parts(Role::Wall, "dead trees", parts);
    g.rect = Some(rect);
    g
}

/// Roads over water: a deck a boot high on the water, arches along its
/// front, rails on the water's side.
pub(crate) fn bridges(land: &Land, rect: Rect) -> Group {
    let (c, r, w, h) = rect;
    let tiles = tiles_of(land, rect, Cover::Bridge);
    let mut parts = Vec::new();
    let cells: Vec<Option<bool>> = (r..r + h)
        .flat_map(|y| (c..c + w).map(move |x| (x, y)))
        .map(|(x, y)| (land.cover(x, y) == Cover::Bridge).then_some(true))
        .collect();
    for ((mc, mr, mw, mh), _) in merge(w as usize, h as usize, &cells) {
        let [x0, z0, x1, z1] = blockout::px((c + mc, r + mr, mw, mh));
        parts.push(vec![Solid::block(Kind::Wood, [x0, z0, x1, z1], -WATER_DROP, WATER_DROP + DECK)]);
    }
    for &(x, y) in &tiles {
        let [x0, z0, x1, z1] = blockout::px((x, y, 1, 1));
        let water = |dx: i32, dy: i32| land.cover(x + dx, y + dy) == Cover::Water;
        // Arches under the deck's front, where the water runs under it.
        if water(0, 1) {
            parts.push(vec![Solid::new(Shape::Arch, Kind::Stone, [(x0 + x1) / 2.0, -WATER_DROP - 1.0, z1 - 1.0], [TILE, WATER_DROP + DECK + 1.0, 2.0])]);
        }
        // Rails on the water's side.
        if water(0, -1) {
            parts.push(vec![Solid::block(Kind::Wood, [x0, z0 - 1.5, x1, z0], 0.0, 6.0)]);
        }
        if water(0, 1) {
            parts.push(vec![Solid::block(Kind::Wood, [x0, z1, x1, z1 + 1.5], 0.0, 6.0)]);
        }
        if water(-1, 0) {
            parts.push(vec![Solid::block(Kind::Wood, [x0 - 1.5, z0, x0, z1], 0.0, 6.0)]);
        }
        if water(1, 0) {
            parts.push(vec![Solid::block(Kind::Wood, [x1, z0, x1 + 1.5, z1], 0.0, 6.0)]);
        }
    }
    let mut g = Group::of_parts(Role::Prop, "bridges", parts);
    g.rect = Some(rect);
    g
}

// ------------------------------------------------------------ the words ----

/// The biome a chunk lies in (`world::biome`).
pub fn biome(_land: &Land, cx: i32, cz: i32) -> Option<&'static str> {
    Some(crate::world::biome::chunk(cx, cz).name())
}

/// A biome's mood for the painters.
fn mood(biome: Option<&str>) -> &'static str {
    let b = biome.unwrap_or("").to_ascii_lowercase();
    if b.contains("wood") || b.contains("green") {
        "old woodland, deep and cool, moss and leaf litter under the crowns"
    } else if b.contains("mountain") || b.contains("high") {
        "high, wind-scoured ground, grey rock and dark pines"
    } else if b.contains("waste") {
        "ash heath under a low sky, dead trees, scree and embers"
    } else if b.contains("coast") {
        "salt wind off the sea, pale dunes and cold water"
    } else if b.contains("town") {
        "the busy heart of the kingdom, cobbles and greens, lamplight in windows"
    } else if b.contains("wet") || b.contains("fen") {
        "low marsh and still meres, reeds and mud, mist on the water"
    } else if b.contains("fair") {
        "a trodden fair field, booths and bunting gone dusky"
    } else if b.contains("wind") || b.contains("downs") {
        "rolling downs, crop strips and open sky"
    } else if b.contains("longbow") {
        "hedged fields and archery greens"
    } else if b.contains("tilt") {
        "open lists and paddocks, the tourney ground"
    } else if b.contains("march") {
        "the frontier, groves and fields between the great houses' castles"
    } else {
        "open country at dusk"
    }
}

/// What a chunk's set is, for the painters: a title and a note or two.
pub struct Item {
    pub id: String,
    pub title: String,
    pub notes: String,
}

/// The chunk's title and notes, from what it holds.
pub fn item(land: &Land, cx: i32, cz: i32) -> Item {
    let (c, r, w, h) = land.chunk_rect(cx, cz);
    let biome = biome(land, cx, cz);
    let mut kinds: Vec<(Kind, usize)> = Vec::new();
    let mut covers: Vec<(Cover, usize)> = Vec::new();
    for y in r..r + h {
        for x in c..c + w {
            let k = land.ground(x, y).0;
            match kinds.iter_mut().find(|(kk, _)| *kk == k) {
                Some((_, n)) => *n += 1,
                None => kinds.push((k, 1)),
            }
            let cv = land.cover(x, y);
            match covers.iter_mut().find(|(cc, _)| *cc == cv) {
                Some((_, n)) => *n += 1,
                None => covers.push((cv, 1)),
            }
        }
    }
    kinds.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    let total = (w * h) as usize;
    let share = |cv: Cover| covers.iter().find(|(c, _)| *c == cv).map_or(0, |&(_, n)| n) * 100 / total.max(1);
    // What stands here, a place's buildings together.
    let builds: Vec<&Build> = land.builds.iter().filter(|b| land.owner(b) == (cx, cz)).collect();
    let mut groups: Vec<(Option<&str>, Vec<&Build>)> = Vec::new();
    for b in &builds {
        let key = b.place.as_deref().filter(|p| !p.starts_with("the house of"));
        match groups.iter_mut().find(|(k, _)| key.is_some() && *k == key) {
            Some((_, list)) => list.push(b),
            None => groups.push((key, vec![b])),
        }
    }
    let mut here = Vec::new();
    let mut names = Vec::new();
    let mut lit = Vec::new();
    for (place, list) in &groups {
        let lights = list.iter().any(|b| b.kit.lit());
        match (place, list.as_slice()) {
            (Some(place), [_, _, ..]) => {
                let mut counted: Vec<(&str, usize)> = Vec::new();
                for b in list {
                    match counted.iter_mut().find(|(n, _)| *n == b.name) {
                        Some((_, k)) => *k += 1,
                        None => counted.push((&b.name, 1)),
                    }
                }
                let parts: Vec<String> = counted.iter().map(|&(n, k)| plural(n, k)).collect();
                here.push(format!("{place} ({})", parts.join(", ")));
                names.push(place.to_string());
                if lights {
                    lit.push(place.to_string());
                }
            }
            (_, [b, ..]) => {
                let name = display(&b.name);
                let what = match &b.place {
                    Some(house) if house.starts_with("the house of") => format!("the castle of {house}"),
                    _ => b.kit.what().to_string(),
                };
                here.push(format!("{name} ({what})"));
                names.push(name.clone());
                if lights {
                    lit.push(name);
                }
            }
            _ => {}
        }
    }
    let mut land_words = Vec::new();
    if share(Cover::Woods) >= 25 {
        land_words.push("woods");
    } else if share(Cover::Woods) >= 6 {
        land_words.push("groves");
    }
    if share(Cover::Rock) >= 6 {
        land_words.push("rock outcrops");
    }
    if share(Cover::DeadTree) >= 3 {
        land_words.push("dead trees");
    }
    if share(Cover::Bridge) > 0 {
        land_words.push("a bridge");
    }
    // The set dressing's stories, each with its things.
    let inside = |(x, y): (i32, i32)| x >= c && x < c + w && y >= r && y < r + h;
    for camp in dressing::get().camps.iter().filter(|camp| inside(camp.at)) {
        let mut things: Vec<&str> = land
            .pieces
            .iter()
            .filter(|p| inside(p.at) && (p.at.0 - camp.at.0).abs() <= 5 && (p.at.1 - camp.at.1).abs() <= 3)
            .map(|p| p.item.name())
            .collect();
        things.dedup();
        if !things.is_empty() {
            here.push(format!("{} ({}: low things a knight walks through)", camp.story, things.join(", ")));
        }
    }
    let posts = land.posts.iter().filter(|(at, _, _)| at.0 >= c && at.0 < c + w && at.1 >= r && at.1 < r + h).count();
    if posts > 0 {
        here.push(if posts == 1 { "a fingerpost on the verge (a post, its arms down the roads)".to_string() } else { format!("{} fingerposts on the verges", plural("a fingerpost", posts).split(' ').next().unwrap_or("two")) });
    }
    let what = if names.is_empty() {
        if land_words.is_empty() { "open ground".to_string() } else { land_words.join(", ") }
    } else {
        names.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
    };
    let title = format!("{} — {}", biome.unwrap_or("The kingdom"), what);
    let ground: Vec<&str> = kinds.iter().take(3).map(|&(k, _)| ground_word(k)).collect();
    let mut notes = format!("{}; the ground is {}", capital(mood(biome)), and_list(&ground));
    if !land_words.is_empty() {
        let _ = write!(notes, ", with {}", and_list(&land_words));
    }
    notes.push('.');
    let lamps: Vec<String> = builds.iter().filter(|b| b.kit == Kit::Lighthouse).map(|b| display(&b.name)).collect();
    if !here.is_empty() {
        let _ = write!(notes, " Here: {}", here.join("; "));
        if lit.is_empty() {
            notes.push_str("; no lights");
        } else {
            let _ = write!(notes, "; light only from the lit windows of {}", and_list(&lit));
            if !lamps.is_empty() {
                let _ = write!(notes, " and the lantern of {}", and_list(&lamps));
            }
        }
        notes.push('.');
    } else {
        notes.push_str(" No buildings, no lights.");
    }
    Item { id: chunk_id(cx, cz), title, notes }
}

/// A name as the notes write it: a sign's capitals as a name.
fn display(name: &str) -> String {
    if name.chars().any(|c| c.is_ascii_lowercase()) { name.to_string() } else { title(name) }
}

/// `n` of a thing named `a cottage`: "four cottages".
fn plural(name: &str, n: usize) -> String {
    if n == 1 {
        return name.to_string();
    }
    let bare = name.strip_prefix("a ").or_else(|| name.strip_prefix("an ")).or_else(|| name.strip_prefix("the ")).unwrap_or(name);
    let count = ["", "one", "two", "three", "four", "five", "six", "seven", "eight"].get(n).copied().unwrap_or("many");
    format!("{count} {bare}s")
}

/// A ground kind in a painter's words.
fn ground_word(k: Kind) -> &'static str {
    match k {
        Kind::Grass => "grass",
        Kind::Path => "dirt roads",
        Kind::Road => "cobbles",
        Kind::Water => "water",
        Kind::Mud => "mud and marsh",
        Kind::Sand => "sand",
        Kind::Crop => "crop fields",
        Kind::Ground => "bare earth and ash",
        Kind::Rock => "rock",
        Kind::Fire => "lava",
        Kind::Wood => "boards",
        other => other.name(),
    }
}

/// "a, b and c".
fn and_list<S: AsRef<str>>(items: &[S]) -> String {
    match items {
        [] => String::new(),
        [one] => one.as_ref().to_string(),
        [rest @ .., last] => format!("{} and {}", rest.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(", "), last.as_ref()),
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ------------------------------------------------------------- the text ----

fn write(land: &Land, cx: i32, cz: i32, groups: &[Group]) -> String {
    let (c, r, w, h) = land.chunk_rect(cx, cz);
    let [x, z] = centre(cx, cz);
    let mut out = String::new();
    let _ = writeln!(out, "set {}", chunk_id(cx, cz));
    let _ = writeln!(
        out,
        "# The kingdom, chunk {cx},{cz}{}: tiles {c}..{} x {r}..{} ({w} x {h} of {} stage pixels).",
        biome(land, cx, cz).map(|b| format!(" in {b}")).unwrap_or_default(),
        c + w,
        r + h,
        TILE
    );
    let _ = writeln!(out, "# Made from the game's own map by angel-delve world3d::chunks; written by the");
    let _ = writeln!(out, "# write_world3d_chunks test (ANGEL_WORLD3D_CHUNKS=1). Change the generator, not this file.");
    let _ = writeln!(out, "camera chunk {} {} centre {} {}", STAGE.0, STAGE.1, num(x), num(z));
    let _ = writeln!(out, "plate {} {}", PLATE.0, PLATE.1);
    for g in groups {
        if g.solids.is_empty() {
            continue;
        }
        let _ = writeln!(out);
        if g.kept < 1.0 {
            let _ = writeln!(out, "# {} (at {}% of its height, for the camera)", g.name, (g.kept * 100.0).floor());
        } else {
            let _ = writeln!(out, "# {}", g.name);
        }
        for s in &g.solids {
            let _ = writeln!(out, "{}", blockout::line(s));
        }
    }
    out
}

/// The hulls of what in `groups` stands up, for the tests.
pub fn standing_hulls(view: &View, groups: &[Group]) -> Vec<blockout::Hull> {
    occluders(view, groups.iter().filter(|g| g.role != Role::Ground).flat_map(|g| &g.solids))
}

#[cfg(test)]
#[path = "../../tests/world3d/chunks__tests.rs"]
mod tests;
