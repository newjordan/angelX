//! Bomrekedan, the Kingdom of Meat Gods: the operator's Dwarf Fortress
//! fortress, read tile by tile from the live save and walked as rooms of the
//! world (`docs/BOMREKEDAN.md`).
//!
//! Each kept level of the fort is one room of the home floor, at its full
//! imported size, in a corner of the floor's grid of its own. No doorway
//! joins them on the grid: the fort's stairs are plates (`DOORS`), each pair
//! of neighbouring levels joined by one rect that is the same on both, and
//! the top level's plate by the trade depot is the way back out to the
//! kingdom, whose map holds the fort's one door (`GATE_GOAL`).
//!
//! The data is generated (`crates/df-import`, the format in
//! `expansions/bomrekedan/kingdom/README.md`): `assets/bomrekedan/levels/
//! <slug>.txt` for each level's ground and what stands on it, and
//! `kingdom.json` for its links, places and seams. Never edit it by hand. The
//! plates are consts, as every entrance of the world is; the tests hold them
//! to `kingdom.json` (and print them afresh from it, `print_doors`).
//!
//! Later phases build on: `levels()` (each level's TILES, DRESS and ROCK
//! grids, its name and depth), `places()` (the fort's rooms, workshops and
//! zones by level) and `kingdom()` (the whole of `kingdom.json`). The fort's
//! people stand on their levels and talk (`people`).

use super::{Dest, Entrance};
use crate::rules::{Room, RoomKind, Tile};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub mod people;
pub mod works;

/// A kept level: its room, its slug in the data, its cell on the home
/// floor's grid, and its map.
pub struct Def {
    pub kind: RoomKind,
    pub slug: &'static str,
    pub cell: (i32, i32),
    data: &'static str,
}

macro_rules! level {
    ($kind:ident, $slug:literal, $col:expr) => {
        Def {
            kind: RoomKind::$kind,
            slug: $slug,
            cell: ($col, -24),
            data: include_str!(concat!("../../../assets/bomrekedan/levels/", $slug, ".txt")),
        }
    };
}

/// Every kept level, top down. The cells are a row of their own far north
/// east of everything else on the floor (the kingdom's map is at (20, -20)),
/// well beyond any loop settlement's dig.
pub const LEVELS: [Def; 10] = [
    level!(BomGate, "the-gate", 24),
    level!(BomGreatHall, "the-great-hall", 25),
    level!(BomFarms, "the-farms", 26),
    level!(BomWorkshops, "the-workshops", 27),
    level!(BomNobleHalls, "the-noble-halls", 28),
    level!(BomTemples, "the-temples", 29),
    level!(BomGuildhalls, "the-guildhalls", 30),
    level!(BomDormitories, "the-dormitories", 31),
    level!(BomBarracks, "the-barracks", 32),
    level!(BomForges, "the-forges", 33),
];

/// The surface level, with the trade depot: the fort's door from the
/// kingdom opens here.
pub const TOP: RoomKind = RoomKind::BomGate;

/// Each level's room and cell, for `world::ROOMS`.
pub const CELLS: [(RoomKind, (i32, i32)); LEVELS.len()] = {
    let mut out = [(TOP, (0, 0)); LEVELS.len()];
    let mut i = 0;
    while i < LEVELS.len() {
        out[i] = (LEVELS[i].kind, LEVELS[i].cell);
        i += 1;
    }
    out
};

/// Where level `kind` stands in `LEVELS`, if it is one.
pub const fn index(kind: RoomKind) -> Option<usize> {
    Some(match kind {
        RoomKind::BomGate => 0,
        RoomKind::BomGreatHall => 1,
        RoomKind::BomFarms => 2,
        RoomKind::BomWorkshops => 3,
        RoomKind::BomNobleHalls => 4,
        RoomKind::BomTemples => 5,
        RoomKind::BomGuildhalls => 6,
        RoomKind::BomDormitories => 7,
        RoomKind::BomBarracks => 8,
        RoomKind::BomForges => 9,
        _ => return None,
    })
}

/// A room that is a level of Bomrekedan.
pub const fn is_level(kind: RoomKind) -> bool {
    index(kind).is_some()
}

/// The kingdom's door to the fort is built on the free lot nearest this
/// tile: on the Greenwood's eastern edge, in the forest a few tiles short of
/// the Highlands' first hills (`world::kingdom::build`; the lot is (66, 18),
/// its door at (67, 21)).
pub const GATE_GOAL: (i32, i32) = (66, 20);
/// The door's name on the kingdom's map.
pub const GATE_NAME: &str = "Gate of Bomrekedan";

/// The clear 3x3 by the trade depot on the surface level, and the floor
/// tile below it a party comes in on from the kingdom (`kingdom.json`
/// `levels[0].entrance`).
pub const ENTRANCE: (i32, i32, i32, i32) = (36, 52, 3, 3);
pub const ARRIVAL: (f32, f32) = (38.5, 55.5);

const fn door(
    id: &'static str,
    room: RoomKind,
    at: (i32, i32, i32, i32),
    to: RoomKind,
    arrive: (f32, f32),
    label: &'static str,
) -> Entrance {
    Entrance { id, room, at, to: Dest::Room(to, arrive), label, needs: None }
}

/// The fort's stairs, each a plate on both of its levels, and the depot's
/// plate back out. A link's `arrive` is a tile; a party stands on its
/// middle. Printed from `kingdom.json` by the tests' `print_doors`.
pub const DOORS: [Entrance; 19] = [
    // Out to the kingdom: back to the door on its map (`rules::kingdom`),
    // or for a party that never came that way, the Delve's gate.
    door("bomrekedan-out", TOP, ENTRANCE, RoomKind::Gate, super::GATE_FRONT, "OUT TO THE KINGDOM"),
    door("bom-gate-down", RoomKind::BomGate, (61, 63, 2, 2), RoomKind::BomGreatHall, (61.5, 62.5), "DOWN TO THE GREAT HALL"),
    door("bom-great-hall-up", RoomKind::BomGreatHall, (61, 63, 2, 2), RoomKind::BomGate, (61.5, 61.5), "UP TO THE GATE"),
    door("bom-great-hall-down", RoomKind::BomGreatHall, (64, 61, 2, 5), RoomKind::BomFarms, (64.5, 60.5), "DOWN TO THE FARMS AND STILLS"),
    door("bom-farms-up", RoomKind::BomFarms, (64, 61, 2, 5), RoomKind::BomGreatHall, (61.5, 61.5), "UP TO THE GREAT HALL"),
    door("bom-farms-down", RoomKind::BomFarms, (61, 61, 2, 5), RoomKind::BomWorkshops, (62.5, 60.5), "DOWN TO THE WORKSHOPS"),
    door("bom-workshops-up", RoomKind::BomWorkshops, (61, 61, 2, 5), RoomKind::BomFarms, (63.5, 60.5), "UP TO THE FARMS AND STILLS"),
    door("bom-workshops-down", RoomKind::BomWorkshops, (64, 61, 2, 5), RoomKind::BomNobleHalls, (64.5, 60.5), "DOWN TO THE NOBLE HALLS"),
    door("bom-noble-halls-up", RoomKind::BomNobleHalls, (64, 61, 2, 5), RoomKind::BomWorkshops, (64.5, 60.5), "UP TO THE WORKSHOPS"),
    door("bom-noble-halls-down", RoomKind::BomNobleHalls, (61, 61, 2, 5), RoomKind::BomTemples, (61.5, 60.5), "DOWN TO THE TEMPLES"),
    door("bom-temples-up", RoomKind::BomTemples, (61, 61, 2, 5), RoomKind::BomNobleHalls, (62.5, 60.5), "UP TO THE NOBLE HALLS"),
    door("bom-temples-down", RoomKind::BomTemples, (64, 61, 2, 5), RoomKind::BomGuildhalls, (64.5, 60.5), "DOWN TO THE GUILDHALLS"),
    door("bom-guildhalls-up", RoomKind::BomGuildhalls, (64, 61, 2, 5), RoomKind::BomTemples, (64.5, 60.5), "UP TO THE TEMPLES"),
    door("bom-guildhalls-down", RoomKind::BomGuildhalls, (61, 62, 2, 2), RoomKind::BomDormitories, (62.5, 64.5), "DOWN TO THE DORMITORIES"),
    door("bom-dormitories-up", RoomKind::BomDormitories, (61, 62, 2, 2), RoomKind::BomGuildhalls, (61.5, 61.5), "UP TO THE GUILDHALLS"),
    door("bom-dormitories-down", RoomKind::BomDormitories, (62, 61, 1, 1), RoomKind::BomBarracks, (66.5, 48.5), "DOWN TO THE BARRACKS"),
    door("bom-barracks-up", RoomKind::BomBarracks, (62, 61, 1, 1), RoomKind::BomDormitories, (62.5, 60.5), "UP TO THE DORMITORIES"),
    door("bom-barracks-down", RoomKind::BomBarracks, (38, 23, 1, 3), RoomKind::BomForges, (38.5, 22.5), "DOWN TO THE FORGES"),
    door("bom-forges-up", RoomKind::BomForges, (38, 23, 1, 3), RoomKind::BomBarracks, (41.5, 23.5), "UP TO THE BARRACKS"),
];

/// A level as the import laid it.
pub struct Level {
    pub kind: RoomKind,
    pub slug: &'static str,
    /// As the fort names it ("The Farms and Stills"), and in capitals for
    /// a board.
    pub name: &'static str,
    pub title: String,
    /// Its depth in the fort (DF's z): the surface is the highest.
    pub z: i32,
    /// Tiles across and down, and where its top left is in the fort.
    pub size: (usize, usize),
    pub origin: (i32, i32),
    /// The ground, row by row, as the game's tiles: a `>` that is no plate
    /// of `DOORS` is plain floor.
    pub tiles: Vec<Tile>,
    /// What stands on each tile (`b` bed, `W` workshop, ...) and what each
    /// natural wall is made of (`s` soil, `i` iron ore, ...), a byte a
    /// tile, `b' '` for nothing: the README's keys.
    pub dress: Vec<u8>,
    pub rock: Vec<u8>,
}

impl Level {
    fn at(&self, grid: &[u8], x: i32, y: i32) -> u8 {
        let (w, h) = self.size;
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            return b' ';
        }
        grid[y as usize * w + x as usize]
    }

    /// What stands on tile `(x, y)`, `b' '` for nothing.
    pub fn dress_at(&self, x: i32, y: i32) -> u8 {
        self.at(&self.dress, x, y)
    }

    /// What the natural wall at `(x, y)` is made of, `b' '` for none.
    pub fn rock_at(&self, x: i32, y: i32) -> u8 {
        self.at(&self.rock, x, y)
    }

    /// Whether this is the surface, open to the sky.
    pub fn surface(&self) -> bool {
        self.kind == TOP
    }
}

fn glyph(ch: u8) -> Result<Tile, String> {
    Ok(match ch {
        b'.' => Tile::Floor,
        b'#' => Tile::Wall,
        b'B' => Tile::Block,
        b'~' => Tile::Hazard,
        b'D' => Tile::Door,
        b'>' => Tile::Stairs,
        _ => return Err(format!("tile {:?}", ch as char)),
    })
}

fn inside((c, r, w, h): (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= c && x < c + w && y >= r && y < r + h
}

type Lines = dyn Iterator<Item = &'static str>;

/// The next line, a header `key` and its value.
fn field(lines: &mut Lines, key: &str) -> Result<&'static str, String> {
    let line = lines.next().ok_or(format!("no {key}"))?;
    line.strip_prefix(key)
        .and_then(|rest| if rest.is_empty() { Some(rest) } else { rest.strip_prefix(' ') })
        .ok_or(format!("{key} expected, read {line:?}"))
}

fn pair(text: &str) -> Result<(i64, i64), String> {
    let mut it = text.split_whitespace().map(str::parse::<i64>);
    match (it.next(), it.next(), it.next()) {
        (Some(Ok(a)), Some(Ok(b)), None) => Ok((a, b)),
        _ => Err(format!("{text:?} is no pair")),
    }
}

/// A grid under its header `key`, `h` rows of `w` bytes (a short row is
/// padded with nothing).
fn grid(lines: &mut Lines, key: &str, (w, h): (usize, usize)) -> Result<Vec<u8>, String> {
    field(lines, key)?;
    let mut out = Vec::with_capacity(w * h);
    for row in 0..h {
        let line = lines.next().ok_or(format!("{key} row {row} missing"))?.as_bytes();
        if line.len() > w {
            return Err(format!("{key} row {row} is {} wide", line.len()));
        }
        out.extend_from_slice(line);
        out.resize((row + 1) * w, b' ');
    }
    Ok(out)
}

/// One level's file read (the README's format).
pub fn parse(def: &Def) -> Result<Level, String> {
    read(def).map_err(|e| format!("{}: {e}", def.slug))
}

fn read(def: &Def) -> Result<Level, String> {
    let mut iter = def.data.lines().map(|l| l.strip_suffix('\r').unwrap_or(l));
    let lines: &mut Lines = &mut iter;
    let slug = field(lines, "LEVEL")?;
    if slug != def.slug {
        return Err(format!("the file is {slug}"));
    }
    let name = field(lines, "NAME")?;
    let z = field(lines, "Z")?.trim().parse::<i32>().map_err(|e| format!("Z {e}"))?;
    let (w, h) = pair(field(lines, "SIZE")?)?;
    let (ox, oy) = pair(field(lines, "ORIGIN")?)?;
    if !(1..=256).contains(&w) || !(1..=256).contains(&h) {
        return Err(format!("SIZE {w}x{h}"));
    }
    let (w, h) = (w as usize, h as usize);
    let raw = grid(lines, "TILES", (w, h))?;
    let dress = grid(lines, "DRESS", (w, h))?;
    let rock = grid(lines, "ROCK", (w, h))?;
    let plates: Vec<_> = DOORS.iter().filter(|e| e.room == def.kind).map(|e| e.at).collect();
    let tiles = raw
        .iter()
        .enumerate()
        .map(|(i, &ch)| {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            match glyph(ch)? {
                Tile::Stairs if !plates.iter().any(|&p| inside(p, x, y)) => Ok(Tile::Floor),
                tile => Ok(tile),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Level {
        kind: def.kind,
        slug: def.slug,
        name,
        title: name.to_uppercase(),
        z,
        size: (w, h),
        origin: (ox as i32, oy as i32),
        tiles,
        dress,
        rock,
    })
}

/// Every level, read once.
pub fn levels() -> &'static [Level] {
    static LEVELS_READ: OnceLock<Vec<Level>> = OnceLock::new();
    LEVELS_READ.get_or_init(|| {
        LEVELS
            .iter()
            .map(|def| parse(def).unwrap_or_else(|e| panic!("bomrekedan level data: {e}")))
            .collect()
    })
}

/// Level `kind`, if it is one.
pub fn level(kind: RoomKind) -> Option<&'static Level> {
    index(kind).map(|i| &levels()[i])
}

/// The size level `kind` is laid at, if it is one.
pub fn size(kind: RoomKind) -> Option<(usize, usize)> {
    level(kind).map(|l| l.size)
}

/// Level `kind`'s name for a board ("THE GREAT HALL").
pub fn title(kind: RoomKind) -> Option<&'static str> {
    level(kind).map(|l| l.title.as_str())
}

/// Level `kind`'s room at `cell`: its ground, every plate of `DOORS` in it
/// stairs, nothing else in it.
pub fn room(kind: RoomKind, cell: (i32, i32)) -> Room {
    static ROOMS: OnceLock<Vec<Room>> = OnceLock::new();
    let rooms = ROOMS.get_or_init(|| {
        levels()
            .iter()
            .map(|l| {
                let mut room = Room::imported(LEVELS[index(l.kind).unwrap()].cell, l.kind, l.size.0, l.size.1, l.tiles.clone());
                for e in DOORS.iter().filter(|e| e.room == l.kind) {
                    let (c, r, w, h) = e.at;
                    for y in r..r + h {
                        for x in c..c + w {
                            room.set(x as usize, y as usize, Tile::Stairs);
                        }
                    }
                }
                room
            })
            .collect()
    });
    let mut room = rooms[index(kind).expect("a level of Bomrekedan")].clone();
    room.cell = cell;
    room
}

/// A rect in level coordinates, as `kingdom.json` writes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A place of the fort (`kingdom.json` `places`): a room, a workshop, a
/// zone, on its level (`None`: a level not kept), with all else it carries.
#[derive(Clone, Debug, Deserialize)]
pub struct Place {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub name: String,
    pub rect: Rect,
    #[serde(flatten)]
    pub more: BTreeMap<String, serde_json::Value>,
}

/// The whole of `kingdom.json`, read once.
pub fn kingdom() -> &'static serde_json::Value {
    static KINGDOM: OnceLock<serde_json::Value> = OnceLock::new();
    KINGDOM.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/bomrekedan/kingdom.json"))
            .unwrap_or_else(|e| panic!("bomrekedan kingdom.json: {e}"))
    })
}

/// Every place of the fort, read once.
pub fn places() -> &'static [Place] {
    static PLACES: OnceLock<Vec<Place>> = OnceLock::new();
    PLACES.get_or_init(|| {
        serde_json::from_value(kingdom()["places"].clone()).unwrap_or_else(|e| panic!("bomrekedan places: {e}"))
    })
}

/// The places on level `kind`.
pub fn places_on(kind: RoomKind) -> impl Iterator<Item = &'static Place> {
    let slug = index(kind).map(|i| LEVELS[i].slug);
    places().iter().filter(move |p| slug.is_some() && p.level.as_deref() == slug)
}

#[cfg(test)]
#[path = "../../../tests/rules/bomrekedan__tests.rs"]
mod tests;
