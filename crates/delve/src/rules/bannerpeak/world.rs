//! Bannerpeak's world: one tile map, 16 pixel tiles, drawn from the sections
//! in `assets/bannerpeak/*.map` (see `layout`). The map never changes after it
//! is read; what a knight does to it (a block struck, a floor broken, a door
//! opened) is kept by the session as a small table of changed tiles.
//!
//! Tile glyphs, one character each:
//!
//! ```text
//! .  air          #  stone         %  snow (slippery)   =  ledge (one way)
//! ?  rune: coin   L  rune: loaf/flame   M  rune: five coins   B  stonework
//! X  cracked floor (a slam breaks it)   ~  water   ^  spikes
//! H  cellar hatch (crouch on it)        d  tapestry door (two banners)
//! D  summit door (seven banners)
//! ```
//!
//! Anything else is a mark: a spawn, a pickup, a gate, a bot hint. The mark
//! is read and its tile is air (`marks`).
use serde::{Deserialize, Serialize};

pub const TILE: i32 = 16;

/// One cell of the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Air,
    Stone,
    Snow,
    Ledge,
    /// A rune block that gives a coin.
    Coin,
    /// A rune block that gives a loaf to a small knight, a flame to a big one.
    Power,
    /// A rune block that gives up to five coins, one a bump.
    Many(u8),
    /// A rune block with nothing left.
    Spent,
    Brick,
    Cracked,
    Water,
    Spikes,
    Hatch,
    /// 0: the courtyard's tapestry door; 1: the summit door.
    Gate(u8),
}

impl Tile {
    pub fn from_glyph(c: char) -> Option<Tile> {
        Some(match c {
            '.' | ' ' => Tile::Air,
            '#' => Tile::Stone,
            '%' => Tile::Snow,
            '=' => Tile::Ledge,
            '?' => Tile::Coin,
            'L' => Tile::Power,
            'M' => Tile::Many(5),
            'B' => Tile::Brick,
            'X' => Tile::Cracked,
            '~' => Tile::Water,
            '^' => Tile::Spikes,
            'H' => Tile::Hatch,
            'd' => Tile::Gate(0),
            'D' => Tile::Gate(1),
            _ => return None,
        })
    }
    /// The byte the session keeps for a changed tile.
    pub fn code(self) -> u8 {
        match self {
            Tile::Air => 0,
            Tile::Stone => 1,
            Tile::Snow => 2,
            Tile::Ledge => 3,
            Tile::Coin => 4,
            Tile::Power => 5,
            Tile::Spent => 6,
            Tile::Brick => 7,
            Tile::Cracked => 8,
            Tile::Water => 9,
            Tile::Spikes => 10,
            Tile::Hatch => 11,
            Tile::Gate(k) => 12 + k.min(1),
            Tile::Many(n) => 20 + n.min(5),
        }
    }
    pub fn from_code(b: u8) -> Option<Tile> {
        Some(match b {
            0 => Tile::Air,
            1 => Tile::Stone,
            2 => Tile::Snow,
            3 => Tile::Ledge,
            4 => Tile::Coin,
            5 => Tile::Power,
            6 => Tile::Spent,
            7 => Tile::Brick,
            8 => Tile::Cracked,
            9 => Tile::Water,
            10 => Tile::Spikes,
            11 => Tile::Hatch,
            12 | 13 => Tile::Gate(b - 12),
            21..=25 => Tile::Many(b - 20),
            _ => return None,
        })
    }
    /// A knight cannot enter it.
    pub fn solid(self) -> bool {
        !matches!(
            self,
            Tile::Air | Tile::Ledge | Tile::Water | Tile::Spikes
        )
    }
    /// A knight can land on it from above and pass up through it.
    pub fn one_way(self) -> bool {
        self == Tile::Ledge
    }
    /// A rune block: struck from below it gives something.
    pub fn rune(self) -> bool {
        matches!(self, Tile::Coin | Tile::Power | Tile::Many(_))
    }
    pub fn slippery(self) -> bool {
        self == Tile::Snow
    }
}

/// What a knight and the foes read of the map: the tile at a cell.
pub trait Grid {
    fn tile(&self, tx: i32, ty: i32) -> Tile;
}

/// A thing placed on the map by a glyph: its character and its cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub k: char,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug)]
pub struct World {
    pub w: i32,
    pub h: i32,
    tiles: Vec<Tile>,
    pub marks: Vec<Mark>,
}

impl World {
    /// A map from rows of glyphs, the shortest row padded with air. Marks
    /// are read in reading order (top row first, left to right).
    pub fn parse(rows: &[&str]) -> World {
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let h = rows.len() as i32;
        let mut world = World {
            w,
            h,
            tiles: vec![Tile::Air; (w * h) as usize],
            marks: Vec::new(),
        };
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                match Tile::from_glyph(c) {
                    Some(t) => world.tiles[y * w as usize + x] = t,
                    None => world.marks.push(Mark {
                        k: c,
                        x: x as i32,
                        y: y as i32,
                    }),
                }
            }
        }
        world
    }
    /// Sections laid on a blank map `w` by `h`: each `(rows, x, y)` puts the
    /// section's top-left cell at `(x, y)`. Later sections write over earlier.
    pub fn compose(w: i32, h: i32, sections: &[(&[&str], i32, i32)]) -> World {
        let mut world = World {
            w,
            h,
            tiles: vec![Tile::Air; (w * h) as usize],
            marks: Vec::new(),
        };
        for &(rows, ox, oy) in sections {
            for (y, row) in rows.iter().enumerate() {
                for (x, c) in row.chars().enumerate() {
                    let (cx, cy) = (ox + x as i32, oy + y as i32);
                    if cx < 0 || cy < 0 || cx >= w || cy >= h {
                        continue;
                    }
                    match Tile::from_glyph(c) {
                        Some(Tile::Air) if c == ' ' => {}
                        Some(t) => world.tiles[(cy * w + cx) as usize] = t,
                        None => world.marks.push(Mark { k: c, x: cx, y: cy }),
                    }
                }
            }
        }
        world.marks.sort_by_key(|m| (m.y, m.x));
        world
    }
    pub fn at(&self, tx: i32, ty: i32) -> Tile {
        if tx < 0 || tx >= self.w {
            Tile::Stone
        } else if ty < 0 || ty >= self.h {
            Tile::Air
        } else {
            self.tiles[(ty * self.w + tx) as usize]
        }
    }
    pub fn marks_of(&self, k: char) -> impl Iterator<Item = &Mark> {
        self.marks.iter().filter(move |m| m.k == k)
    }
}

impl Grid for World {
    fn tile(&self, tx: i32, ty: i32) -> Tile {
        self.at(tx, ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_round_trip_through_their_codes() {
        for c in ".#%=?LMBX~^HdD".chars() {
            let t = Tile::from_glyph(c).unwrap();
            assert_eq!(Tile::from_code(t.code()), Some(t), "{c}");
        }
        for n in 1..=5 {
            assert_eq!(Tile::from_code(Tile::Many(n).code()), Some(Tile::Many(n)));
        }
        assert_eq!(Tile::from_code(99), None);
    }

    #[test]
    fn a_map_reads_tiles_and_marks_and_stays_closed_at_the_sides() {
        let w = World::parse(&["..s.", "####"]);
        assert_eq!((w.w, w.h), (4, 2));
        assert_eq!(w.at(1, 1), Tile::Stone);
        assert_eq!(w.at(2, 0), Tile::Air, "a mark leaves air behind");
        assert_eq!(
            w.marks,
            vec![Mark {
                k: 's',
                x: 2,
                y: 0
            }]
        );
        assert_eq!(w.at(-1, 0), Tile::Stone, "walls at the sides");
        assert_eq!(w.at(0, -3), Tile::Air, "open above");
        assert_eq!(w.at(0, 9), Tile::Air, "open below, a fall");
    }
}
