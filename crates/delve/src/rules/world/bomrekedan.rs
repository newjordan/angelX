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

pub const TOP: RoomKind = RoomKind::BomGate;

pub const CELLS: [(RoomKind, (i32, i32)); LEVELS.len()] = {
    let mut out = [(TOP, (0, 0)); LEVELS.len()];
    let mut i = 0;
    while i < LEVELS.len() {
        out[i] = (LEVELS[i].kind, LEVELS[i].cell);
        i += 1;
    }
    out
};

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

pub const fn is_level(kind: RoomKind) -> bool {
    index(kind).is_some()
}

pub const GATE_GOAL: (i32, i32) = (66, 20);
pub const GATE_NAME: &str = "Gate of Bomrekedan";

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

pub const DOORS: [Entrance; 19] = [
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

pub struct Level {
    pub kind: RoomKind,
    pub slug: &'static str,
    pub name: &'static str,
    pub title: String,
    pub z: i32,
    pub size: (usize, usize),
    pub origin: (i32, i32),
    pub tiles: Vec<Tile>,
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

    pub fn dress_at(&self, x: i32, y: i32) -> u8 {
        self.at(&self.dress, x, y)
    }

    pub fn rock_at(&self, x: i32, y: i32) -> u8 {
        self.at(&self.rock, x, y)
    }

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

pub fn levels() -> &'static [Level] {
    static LEVELS_READ: OnceLock<Vec<Level>> = OnceLock::new();
    LEVELS_READ.get_or_init(|| {
        LEVELS
            .iter()
            .map(|def| parse(def).unwrap_or_else(|e| panic!("bomrekedan level data: {e}")))
            .collect()
    })
}

pub fn level(kind: RoomKind) -> Option<&'static Level> {
    index(kind).map(|i| &levels()[i])
}

pub fn size(kind: RoomKind) -> Option<(usize, usize)> {
    level(kind).map(|l| l.size)
}

pub fn title(kind: RoomKind) -> Option<&'static str> {
    level(kind).map(|l| l.title.as_str())
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

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

pub fn kingdom() -> &'static serde_json::Value {
    static KINGDOM: OnceLock<serde_json::Value> = OnceLock::new();
    KINGDOM.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/bomrekedan/kingdom.json"))
            .unwrap_or_else(|e| panic!("bomrekedan kingdom.json: {e}"))
    })
}

pub fn places() -> &'static [Place] {
    static PLACES: OnceLock<Vec<Place>> = OnceLock::new();
    PLACES.get_or_init(|| {
        serde_json::from_value(kingdom()["places"].clone()).unwrap_or_else(|e| panic!("bomrekedan places: {e}"))
    })
}

pub fn places_on(kind: RoomKind) -> impl Iterator<Item = &'static Place> {
    let slug = index(kind).map(|i| LEVELS[i].slug);
    places().iter().filter(move |p| slug.is_some() && p.level.as_deref() == slug)
}

#[cfg(test)]
#[path = "../../../tests/rules/bomrekedan__tests.rs"]
mod tests;
