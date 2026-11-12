//! The hillside: layout 2 of the world, the ground the knights walk (the
//! realm is 240 x 176 tiles). The valley (layout 1) is a 72 x 54 meadow of
//! nine earned parcels; the hillside is one open slope that is all yours,
//! with the sites a castle needs set on it by hand, the way the Tabula plan
//! leaves the realm: field stone, woodland, a clay bank, sand along a river
//! and a windy ridge. Good stone is not here: it is won below, by the knights. Seeds only vary the river's meander and the
//! ragged edges of woods; the places themselves never move, so a plan, a
//! walkthrough and the saved sites always agree.
//!
//! Terrain is a pure function of the seed. The grid is built once per world
//! and shared by clones, never saved: a save keeps the seed and cleared trees.
use crate::world::Terrain;
use crate::Tile;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

pub const REALM_WIDTH: i16 = 240;
pub const REALM_HEIGHT: i16 = 176;
const CELLS: usize = REALM_WIDTH as usize * REALM_HEIGHT as usize;

/// Where the founding camp stands on the hillside: just south of the Keep's
/// gate, on the way down to the Delve and beside the southern woods. The Keep's footprint is centred on `KEEP_CENTRE`.
pub const HILL_CAMP: Tile = (140, 95);
/// The Delve's stair mouth, beside the Keep: nothing may be built on it.
pub const HILL_GATE: Tile = (140, 87);
/// Where the Great Shaft's lift stands: the Delve's stair comes out in the
/// bailey, and its hoist stands beside the stair's stand, inside the wall, on
/// the bailey's open ground (the Works' baked castle never closes it).
pub const HILL_SHAFT: Tile = (141, 88);
/// The Keep's middle: the game's site (`world::works::ORIGIN`), so the sim's
/// Keep and the one the knights walk are one place.
pub const KEEP_CENTRE: Tile = (133, 81);
/// A great tower with its curtain: 28 x 28 tiles, gate facing south, the
/// game's site rect `(119, 67, 28, 28)`.
pub const KEEP_FOOTPRINT: (i16, i16) = (28, 28);

/// The Keep's ground, inclusive: x 119 to 146, y 67 to 94.
pub fn keep_rect() -> Rect {
    let x0 = KEEP_CENTRE.0 - KEEP_FOOTPRINT.0 / 2;
    let y0 = KEEP_CENTRE.1 - KEEP_FOOTPRINT.1 / 2;
    (x0, y0, x0 + KEEP_FOOTPRINT.0 - 1, y0 + KEEP_FOOTPRINT.1 - 1)
}

/// The road from the camp's road, up the causeway and through the Keep's
/// gate (x 132 and 133 stay open in the baked castle), across the bailey to
/// the Great Shaft's hoist. These tiles are the Keep's fixtures: the one
/// road and building the castle's ground may hold.
pub fn shaft_road() -> Vec<Tile> {
    let camp = HILL_CAMP;
    let mut road: Vec<Tile> = (133..camp.0 - 4).rev().map(|x| (x, camp.1 + 1)).collect();
    road.extend((HILL_SHAFT.1..=camp.1).rev().map(|y| (133, y)));
    road.extend((134..HILL_SHAFT.0).map(|x| (x, HILL_SHAFT.1)));
    road
}
/// The tiles at or above this height catch the wind.
pub const HILLTOP: u8 = 26;

/// A rectangle in tiles, inclusive at both ends.
pub type Rect = (i16, i16, i16, i16);
pub fn in_rect((x, y): Tile, (x0, y0, x1, y1): Rect) -> bool {
    x >= x0 && x <= x1 && y >= y0 && y <= y1
}

/// The sixteen model houses' camps (copied from the realm's Tabula plan: 7 x 4
/// tiles each). The works never build on them.
pub const CAMP_GROUNDS: [Rect; 16] = [
    (128, 54, 134, 57),
    (149, 52, 155, 55),
    (172, 56, 178, 59),
    (181, 70, 187, 73),
    (174, 86, 180, 89),
    (181, 100, 187, 103),
    (173, 117, 179, 120),
    (155, 119, 161, 122),
    (127, 115, 133, 118),
    (109, 118, 115, 121),
    (78, 116, 84, 119),
    (76, 101, 82, 104),
    (84, 87, 90, 90),
    (82, 70, 88, 73),
    (77, 55, 83, 58),
    (108, 52, 114, 55),
];

pub const CLAY: Rect = (104, 84, 108, 94);
/// A hollow of sharp sand an old stream left beside the Keep's east wall, a dozen tiles from the
/// stockyard: the first sand pits dig here, and the river banks forty tiles west are for when it
/// is not enough.
pub const SAND_HOLLOW: Rect = (153, 72, 158, 78);
/// A small outcrop of field stone beside the camp, for the first quarry.
pub const FIELDSTONE: Rect = (147, 91, 152, 94);
pub const WOODS: [Rect; 4] = [
    (104, 99, 124, 112),
    (132, 99, 156, 112),
    (104, 62, 112, 78),
    (136, 36, 150, 48),
];
/// Hill crowns that lift the windy ridge above the slope.
const CROWNS: [(i16, i16, i16, i16); 2] = [(146, 94, 14, 9), (200, 60, 18, 8)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deposit {
    /// Gravel and sand banks beside the river.
    Sand,
}
impl Deposit {
    pub fn name(self) -> &'static str {
        match self {
            Self::Sand => "Sand bank",
        }
    }
}

pub(crate) struct Grid {
    terrain: Vec<Terrain>,
    deposit: Vec<Option<Deposit>>,
    height: Vec<u8>,
    /// Land a person may walk on, which is every cell but deep river.
    pub walkable: Vec<bool>,
}
impl Grid {
    fn at(&self, (x, y): Tile) -> Option<usize> {
        ((0..REALM_WIDTH).contains(&x) && (0..REALM_HEIGHT).contains(&y))
            .then(|| y as usize * REALM_WIDTH as usize + x as usize)
    }
    pub fn terrain(&self, tile: Tile) -> Terrain {
        self.at(tile).map_or(Terrain::Meadow, |i| self.terrain[i])
    }
    pub fn deposit(&self, tile: Tile) -> Option<Deposit> {
        self.at(tile).and_then(|i| self.deposit[i])
    }
    pub fn height(&self, tile: Tile) -> u8 {
        self.at(tile).map_or(0, |i| self.height[i])
    }
}

fn mix(mut n: u64) -> u64 {
    n = n.wrapping_add(0x9e3779b97f4a7c15);
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}

/// The river's centre column at row `y`. It runs north to south (downstream is
/// +y), bending a few tiles either way between hand-set nodes so barges have a
/// real course and nothing about it is a straight ruler line.
pub fn river_column(seed: u64, y: i16) -> i16 {
    let node = |k: i16| (mix(seed ^ 0x51ed ^ (k as u64)) % 7) as i16 - 3;
    let (cell, along) = (y.div_euclid(16), y.rem_euclid(16));
    // Linear between nodes, in integers, so every platform agrees.
    let (a, b) = (node(cell), node(cell + 1));
    100 + a + (b - a) * along / 16
}

/// A smooth bump: `peak` at the centre falling to zero at `rx`, `ry`.
fn crown((cx, cy, rx, ry): (i16, i16, i16, i16), peak: i32, (x, y): Tile) -> i32 {
    let dx = i32::from((x - cx).abs());
    let dy = i32::from((y - cy).abs());
    // Diamond falloff in integer steps.
    let d = dx * 100 / i32::from(rx) + dy * 100 / i32::from(ry);
    (peak * (100 - d.min(100)) / 100).max(0)
}

fn height_at(seed: u64, (x, y): Tile) -> u8 {
    // The slope rises east of the river and a little to the far west; the river
    // valley is the low line. The Keep's plateau is level so a castle can stand.
    let river = river_column(seed, y);
    let base = i32::from(x - river).max(0) / 3 + i32::from(river - x).max(0) / 5;
    let mut h = base;
    for (i, c) in CROWNS.iter().enumerate() {
        h += crown(*c, if i == 0 { 18 } else { 10 }, (x, y));
    }
    // A one-step ripple, so no hill is a plane.
    h += (mix(seed ^ ((x as u64 / 3) << 16) ^ (y as u64 / 3)) % 2) as i32;
    let (kx0, ky0, kx1, ky1) = keep_rect();
    let keep = (kx0 - 4, ky0 - 4, kx1 + 4, ky1 + 4);
    if in_rect((x, y), keep) {
        h = i32::from(KEEP_PLATEAU);
    }
    h.clamp(0, 60) as u8
}
/// The height of the castle's level ground.
pub const KEEP_PLATEAU: u8 = 11;

pub(crate) fn build(seed: u64) -> Grid {
    let mut terrain = vec![Terrain::Meadow; CELLS];
    let mut deposit = vec![None; CELLS];
    let mut height = vec![0u8; CELLS];
    for y in 0..REALM_HEIGHT {
        let column = river_column(seed, y);
        for x in 0..REALM_WIDTH {
            let i = y as usize * REALM_WIDTH as usize + x as usize;
            let tile = (x, y);
            height[i] = height_at(seed, tile);
            let t = if (x - column).abs() <= 1 {
                Terrain::River
            } else if in_rect(tile, CLAY) {
                Terrain::ClayBank
            } else if in_rect(tile, FIELDSTONE) {
                Terrain::Stone
            } else if WOODS.iter().any(|&r| in_rect(tile, r)) {
                // Ragged edges: the rim of a wood thins out by seed.
                let rim = WOODS
                    .iter()
                    .find(|&&r| in_rect(tile, r))
                    .is_some_and(|&(x0, y0, x1, y1)| x == x0 || x == x1 || y == y0 || y == y1);
                if rim && mix(seed ^ ((x as u64) << 20) ^ y as u64) % 3 == 0 {
                    Terrain::Meadow
                } else {
                    Terrain::Forest
                }
            } else {
                Terrain::Meadow
            };
            terrain[i] = t;
            deposit[i] = if t == Terrain::Meadow
                && (((2..=3).contains(&(x - column).abs()) && (50..=140).contains(&y)) || in_rect(tile, SAND_HOLLOW))
            {
                Some(Deposit::Sand)
            } else {
                None
            };
        }
    }
    let walkable = terrain.iter().map(|t| t.walkable()).collect();
    Grid {
        terrain,
        deposit,
        height,
        walkable,
    }
}

/// The shared grid. Clones point at one allocation; two worlds are equal by
/// their saved fields alone, never by this cache.
#[derive(Clone, Default)]
pub struct GridCache(OnceLock<Arc<Grid>>);
impl GridCache {
    pub(crate) fn get(&self, seed: u64) -> &Grid {
        self.0.get_or_init(|| Arc::new(build(seed)))
    }
}
impl std::fmt::Debug for GridCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GridCache")
    }
}
impl PartialEq for GridCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for GridCache {}

/// Timber a tile of wood holds, and field stone a tile of outcrop. The woods are
/// not endless: plain felling takes them to nothing, coppicing keeps them.
pub const WOOD_DENSITY: u32 = 10;
pub const FIELD_DENSITY: u32 = 60;

fn area((x0, y0, x1, y1): Rect) -> u32 {
    ((x1 - x0 + 1) * (y1 - y0 + 1)) as u32
}
/// Every patch of the hillside that can run out, with its full size.
pub fn patches() -> Vec<(String, u32)> {
    let mut v: Vec<(String, u32)> =
        WOODS.iter().enumerate().map(|(i, &r)| (format!("wood-{i}"), area(r) * WOOD_DENSITY)).collect();
    v.push(("field".into(), area(FIELDSTONE) * FIELD_DENSITY));
    v
}
pub fn patch_capacity(id: &str) -> u32 {
    patches().into_iter().find(|(k, _)| k == id).map_or(0, |(_, n)| n)
}
/// The nearest patch of `kind` (`wood` or `field`) within `radius` tiles.
pub fn nearest_patch(kind: &str, (x, y): Tile, radius: i16) -> Option<String> {
    let dist = |(x0, y0, x1, y1): Rect| {
        let dx = (x0 - x).max(0).max(x - x1);
        let dy = (y0 - y).max(0).max(y - y1);
        dx + dy
    };
    match kind {
        "wood" => WOODS
            .iter()
            .enumerate()
            .map(|(i, &r)| (dist(r), i))
            .filter(|(d, _)| *d <= radius)
            .min()
            .map(|(_, i)| format!("wood-{i}")),
        "field" => (dist(FIELDSTONE) <= radius).then(|| "field".to_string()),
        _ => None,
    }
}
