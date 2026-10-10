//! What the crawl's walls, floors and vaults are faced with, in realm inks.
//! A painted texture in `assets/crawl/<name>.ink` wins; until one is
//! there, each is drawn here from a simple pattern, so the crawl always has
//! a face to show.

use super::render::Tex;
use std::sync::OnceLock;

/// The wall faces, in the order the crawl's grid names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wall {
    Stone,
    Crypt,
    Mine,
    Keep,
    Archive,
    Fungal,
    Unknown,
    Pillar,
    Shelf,
    Door,
}

pub(crate) const WALLS: [Wall; 10] = [
    Wall::Stone,
    Wall::Crypt,
    Wall::Mine,
    Wall::Keep,
    Wall::Archive,
    Wall::Fungal,
    Wall::Unknown,
    Wall::Pillar,
    Wall::Shelf,
    Wall::Door,
];

/// Every texture the crawl uses.
pub(crate) struct Faces {
    pub(crate) walls: Vec<Tex>,
    pub(crate) floor: Tex,
    pub(crate) water: Tex,
    pub(crate) lava: Tex,
    pub(crate) stairs: Tex,
    pub(crate) ceiling: Tex,
}

/// The painted textures, by name (painted from a brief: see
/// `assets/crawl/NOTES.md`; `assets/crawl/tools` rebuilds them).
fn painted(name: &str) -> Option<&'static str> {
    Some(match name {
        "wall_stone" => include_str!("../../../../assets/crawl/wall_stone.ink"),
        "wall_crypt" => include_str!("../../../../assets/crawl/wall_crypt.ink"),
        "wall_mine" => include_str!("../../../../assets/crawl/wall_mine.ink"),
        "wall_keep" => include_str!("../../../../assets/crawl/wall_keep.ink"),
        "wall_archive" => include_str!("../../../../assets/crawl/wall_archive.ink"),
        "wall_fungal" => include_str!("../../../../assets/crawl/wall_fungal.ink"),
        "wall_unknown" => include_str!("../../../../assets/crawl/wall_unknown.ink"),
        "door" => include_str!("../../../../assets/crawl/door.ink"),
        "stairs" => include_str!("../../../../assets/crawl/stairs.ink"),
        "floor_flags" => include_str!("../../../../assets/crawl/floor_flags.ink"),
        "ceiling" => include_str!("../../../../assets/crawl/ceiling.ink"),
        _ => return None,
    })
}

fn tex(name: &str, drawn: fn() -> Vec<String>) -> Tex {
    match painted(name) {
        Some(raw) => {
            let rows: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
            Tex::from_rows(&rows)
        }
        None => {
            let rows = drawn();
            Tex::from_rows(&rows.iter().map(String::as_str).collect::<Vec<_>>())
        }
    }
}

pub(crate) fn faces() -> &'static Faces {
    static FACES: OnceLock<Faces> = OnceLock::new();
    FACES.get_or_init(|| Faces {
        walls: WALLS
            .iter()
            .map(|wall| match wall {
                Wall::Stone => tex("wall_stone", || bricks(['K', 'S', 'u', 'x'], None)),
                Wall::Crypt => tex("wall_crypt", crypt),
                Wall::Mine => tex("wall_mine", mine),
                Wall::Keep => tex("wall_keep", || bricks(['Z', 'X', 'g', 'K'], Some('7'))),
                Wall::Archive => tex("wall_archive", shelf),
                Wall::Fungal => tex("wall_fungal", fungal),
                Wall::Unknown => tex("wall_unknown", unknown),
                Wall::Pillar => tex("pillar", || bricks(['K', 'j', 'G', 'g'], None)),
                Wall::Shelf => tex("shelf", shelf),
                Wall::Door => tex("door", door),
            })
            .collect(),
        floor: tex("floor_flags", flags),
        water: tex("water", water),
        lava: tex("lava", lava),
        stairs: tex("stairs", stairs),
        ceiling: tex("ceiling", ceiling),
    })
}

/// A 16x16 grid filled by `ink(x, y)`.
fn draw(ink: impl Fn(usize, usize) -> char) -> Vec<String> {
    (0..16)
        .map(|y| (0..16).map(|x| ink(x, y)).collect())
        .collect()
}

/// Courses of bricks: mortar, face, lit top edge, shadowed bottom edge; a
/// crack of `ember` through some of them.
fn bricks([mortar, face, lit, dark]: [char; 4], ember: Option<char>) -> Vec<String> {
    draw(|x, y| {
        let course = y / 4;
        let shift = if course % 2 == 0 { 0 } else { 4 };
        let (bx, by) = ((x + shift) % 8, y % 4);
        if by == 3 || bx == 7 {
            return mortar;
        }
        if let Some(ember) = ember
            && (x * 7 + y * 3) % 23 == 0
        {
            return ember;
        }
        if by == 0 {
            lit
        } else if by == 2 && bx > 3 {
            dark
        } else {
            face
        }
    })
}

/// A crypt wall: bricks with a burial niche and a skull in it.
fn crypt() -> Vec<String> {
    let mut rows = bricks(['K', 'S', 'u', 'x'], None);
    let skull = ["..hih...", ".iHHHi..", ".HkHkH..", ".iHHHi..", "..hih..."];
    for (y, row) in rows.iter_mut().enumerate().take(13).skip(4) {
        let mut chars: Vec<char> = row.chars().collect();
        for (x, c) in chars.iter_mut().enumerate().take(12).skip(4) {
            *c = if y == 4 || y == 12 || x == 4 || x == 11 {
                'g'
            } else {
                'k'
            };
        }
        if (6..11).contains(&y) {
            for (k, s) in skull[y - 6].chars().enumerate() {
                if s != '.' {
                    chars[4 + k] = s;
                }
            }
        }
        *row = chars.into_iter().collect();
    }
    rows
}

/// A mine face: broken rock between two timber props and a lintel.
fn mine() -> Vec<String> {
    draw(|x, y| {
        if y < 2 {
            return if y == 0 { 'r' } else { 'B' };
        }
        if !(2..=13).contains(&x) {
            return if x == 0 || x == 15 { 'b' } else { 'P' };
        }
        match (x * 5 + y * 11 + (x * y) % 7) % 9 {
            0 => 'K',
            1 | 2 => 'g',
            3 => 'j',
            _ => 'X',
        }
    })
}

/// Shelves of books, water-stained low down.
fn shelf() -> Vec<String> {
    let spines = ['p', 'B', 'q', 'l', 'R', 'S', 'b', 'o'];
    draw(|x, y| {
        if y % 5 == 4 {
            return 'I';
        }
        if x == 0 || x == 15 {
            return 'b';
        }
        if y >= 12 && (x + y) % 3 == 0 {
            return 'e';
        }
        spines[(x / 2 + y / 5 * 3) % spines.len()]
    })
}

/// Mossy stone, a mushroom glowing at its foot.
fn fungal() -> Vec<String> {
    let mut rows = bricks(['D', 'e', 'm', 'F'], None);
    let cap = [".2222.", "233332", "..yy..", "..yy.."];
    for (k, line) in cap.iter().enumerate() {
        let mut chars: Vec<char> = rows[11 + k].chars().collect();
        for (i, c) in line.chars().enumerate() {
            if c != '.' {
                chars[5 + i] = c;
            }
        }
        rows[11 + k] = chars.into_iter().collect();
    }
    rows
}

/// Black, crossed by a pale lattice, one eye.
fn unknown() -> Vec<String> {
    draw(|x, y| {
        if (6..=9).contains(&x) && (6..=9).contains(&y) {
            return match (x, y) {
                (7 | 8, 7 | 8) => '1',
                _ => 'u',
            };
        }
        if (x + y) % 8 == 0 || (x + 16 - y) % 8 == 0 {
            'S'
        } else {
            'k'
        }
    })
}

/// An iron-banded door in a stone frame.
fn door() -> Vec<String> {
    draw(|x, y| {
        if !(2..=13).contains(&x) || y < 1 {
            return 'g';
        }
        if y == 4 || y == 11 {
            return 'J';
        }
        if x == 11 && (7..=8).contains(&y) {
            return 'h';
        }
        if x % 3 == 2 { 'b' } else { 'B' }
    })
}

/// Flagstones.
fn flags() -> Vec<String> {
    draw(|x, y| {
        let (bx, by) = (x % 8, y % 8);
        let shift = if (y / 8) % 2 == 0 { 0 } else { 4 };
        if by == 7 || (bx + shift) % 8 == 7 {
            'K'
        } else if (x * 3 + y * 5) % 11 == 0 {
            'x'
        } else {
            'X'
        }
    })
}

fn water() -> Vec<String> {
    draw(|x, y| {
        if (x + 2 * y) % 7 == 0 {
            'Q'
        } else if y % 3 == 0 {
            'q'
        } else {
            's'
        }
    })
}

fn lava() -> Vec<String> {
    draw(|x, y| match (x * 3 + y * 5 + x * y) % 9 {
        0 => '5',
        1 | 2 => '@',
        3..=5 => '7',
        _ => '8',
    })
}

/// Steps going down into the dark, seen from above.
fn stairs() -> Vec<String> {
    draw(|_, y| match y % 4 {
        0 => 'J',
        1 => 'g',
        2 => 'X',
        _ => 'k',
    })
}

fn ceiling() -> Vec<String> {
    draw(|x, y| if x % 8 == 0 || y % 8 == 0 { 'X' } else { 'K' })
}

/// The crawl's hands and standing things, painted from a brief
/// (see `assets/crawl/sprites/NOTES.md`).
pub(crate) struct Sprites {
    /// The leading knight's sword hand: at rest, drawn back, mid-swing.
    pub(crate) sword: [Tex; 3],
    /// The other hand's torch, two flickers.
    pub(crate) torch: [Tex; 2],
    pub(crate) campfire: [Tex; 2],
    pub(crate) boulder: Tex,
    pub(crate) lectern: Tex,
    pub(crate) chest: Tex,
    pub(crate) chest_open: Tex,
    /// A wall sconce's two flickers, and a hanging banner.
    pub(crate) sconce: [Tex; 2],
    pub(crate) banner: Tex,
}

fn sprite(raw: &str) -> Tex {
    let rows: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    Tex::from_rows(&rows)
}

pub(crate) fn sprites() -> &'static Sprites {
    static SPRITES: OnceLock<Sprites> = OnceLock::new();
    SPRITES.get_or_init(|| Sprites {
        sword: [
            sprite(include_str!(
                "../../../../assets/crawl/sprites/hand_sword_idle.ink"
            )),
            sprite(include_str!(
                "../../../../assets/crawl/sprites/hand_sword_raise.ink"
            )),
            sprite(include_str!(
                "../../../../assets/crawl/sprites/hand_sword_swing.ink"
            )),
        ],
        torch: [
            sprite(include_str!(
                "../../../../assets/crawl/sprites/hand_torch_a.ink"
            )),
            sprite(include_str!(
                "../../../../assets/crawl/sprites/hand_torch_b.ink"
            )),
        ],
        campfire: [
            sprite(include_str!(
                "../../../../assets/crawl/sprites/campfire_a.ink"
            )),
            sprite(include_str!(
                "../../../../assets/crawl/sprites/campfire_b.ink"
            )),
        ],
        boulder: sprite(include_str!("../../../../assets/crawl/sprites/boulder.ink")),
        lectern: sprite(include_str!("../../../../assets/crawl/sprites/lectern.ink")),
        chest: sprite(include_str!("../../../../assets/crawl/sprites/chest.ink")),
        chest_open: sprite(include_str!(
            "../../../../assets/crawl/sprites/chest_open.ink"
        )),
        sconce: [
            sprite(include_str!(
                "../../../../assets/crawl/sprites/sconce_a.ink"
            )),
            sprite(include_str!(
                "../../../../assets/crawl/sprites/sconce_b.ink"
            )),
        ],
        banner: sprite(include_str!("../../../../assets/crawl/sprites/banner.ink")),
    })
}
