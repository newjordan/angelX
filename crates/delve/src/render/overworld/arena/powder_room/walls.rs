//! The powder store's still stone, laid once with the room's scenery
//! (`scenery` asks for each tile): flagstones in a faint check, soot-dark
//! and cold, so the fire is the only warm thing on them; brick walls with
//! an iron lantern every few paces and kegs stacked against the north wall;
//! stone piers capped in brass for the pillars; iron-bound crates for what
//! is there to be broken. Nothing here moves (`set` has what does), and
//! every tile is judged by itself and its neighbours, so a crate that goes
//! simply leaves a flagstone behind.

use super::super::super::ink::{Img, bayer, hash};
use super::super::TILE;
use crate::rules::powder::pillar;
use crate::rules::{Room, Tile};
use std::sync::OnceLock;

/// Whether a tile is stone to the eye (walls, piers and crates); beyond the
/// room counts as stone.
fn solid(room: &Room, c: i32, r: i32) -> bool {
    c < 0 || r < 0 || c >= room.cols as i32 || r >= room.rows as i32 || matches!(room.tile(c, r), Tile::Block | Tile::Wall)
}

fn is_open(room: &Room, c: i32, r: i32) -> bool {
    matches!(room.tile(c, r), Tile::Floor | Tile::Door | Tile::Stairs)
}

/// A wall tile whose south side is open store floor and whose north is stone.
pub(in super::super) fn north_edge(room: &Room, c: i32, r: i32) -> bool {
    room.tile(c, r) == Tile::Wall && is_open(room, c, r + 1) && solid(room, c, r - 1)
}

/// Whether an iron lantern hangs on this wall tile (and where its flame
/// sits, the middle of the tile's lower half).
pub(in super::super) fn lantern(room: &Room, c: i32, r: i32) -> bool {
    north_edge(room, c, r) && c.rem_euclid(5) == 2
}

/// What the store lays on `room`'s tile `(col, row)`.
pub(in super::super) fn lay(cv: &mut Img, room: &Room, col: i32, row: i32) {
    match room.tile(col, row) {
        Tile::Wall => wall(cv, room, col, row),
        Tile::Block if pillar(col, row) => {
            slab(cv, room, col, row);
            cv.stamp(pier(), col * TILE, row * TILE);
        }
        Tile::Block => {
            slab(cv, room, col, row);
            cv.stamp(crate_img(crate_kind(col, row)), col * TILE, row * TILE);
        }
        Tile::Floor | Tile::Door | Tile::Stairs => slab(cv, room, col, row),
        _ => {}
    }
}

/// Which of the crate pictures stands on a tile.
pub(in super::super) fn crate_kind(col: i32, row: i32) -> usize {
    match hash(col, row, 411) % 9 {
        0..=3 => 0,
        4 | 5 => 1,
        6 | 7 => 2,
        _ => 3,
    }
}

/// A flagstone: kept by what decides it (the check, what stands north of
/// it, a crack) and one of four hands of flecks, so a room's floor is
/// stamped, not worked out pixel by pixel.
fn slab(cv: &mut Img, room: &Room, col: i32, row: i32) {
    let north = match room.tile(col, row - 1) {
        Tile::Block => 1,
        Tile::Wall => 2,
        _ => 0,
    };
    let key = (((col + row) % 2) as usize, north, usize::from(hash(col, row, 5) % 9 == 0), (hash(col, row, 6) % 4) as usize);
    static SLABS: OnceLock<Vec<Img>> = OnceLock::new();
    let slabs = SLABS.get_or_init(|| {
        let mut v = Vec::new();
        for check in 0..2 {
            for north in 0..3 {
                for cracked in 0..2 {
                    for hand in 0..4 {
                        v.push(make_slab(check == 0, north, cracked == 1, hand));
                    }
                }
            }
        }
        v
    });
    cv.stamp(&slabs[((key.0 * 3 + key.1) * 2 + key.2) * 4 + key.3], col * TILE, row * TILE);
}

/// A flagstone: a faint check of two night-blues, a lit lip along its top
/// and left, a dark joint along its foot and right, flecks and the odd
/// crack; and a shadow thrown south by whatever stands north of it.
fn make_slab(check: bool, north: usize, cracked: bool, hand: i32) -> Img {
    let mut im = Img::new(TILE, TILE);
    for py in 0..TILE {
        for px in 0..TILE {
            let (x, y) = (px + hand * 16, py + 7 * hand);
            let mut ink = if check { 'x' } else { 's' };
            let g = hash(x, y, 17);
            if px == TILE - 1 || py == TILE - 1 {
                ink = 'k';
            } else if (px == 0 || py == 0) && g % 3 != 0 {
                ink = if check { 'S' } else { 'x' };
            } else if g % 29 == 0 {
                ink = 'S';
            } else if g % 37 == 0 || bayer(x / 3, y / 3) < 0.05 {
                ink = 'k';
            }
            // A crack across the stone.
            if cracked && ((px + py == 15 && (4..=11).contains(&px) && px % 3 != 0) || (px, py) == (7, 7) || (px, py) == (8, 6)) {
                ink = 'k';
            }
            // What stands north throws a shadow down the stone.
            if north > 0 && py < 4 && bayer(x, y) < (4 - py) as f32 * 0.24 {
                ink = 'k';
            }
            im.put(px, py, ink);
        }
    }
    im
}

/// A brick wall tile: courses of warm soot-grey brick lit along the edge
/// that meets the floor; the north wall's lantern and stacked kegs; stone
/// deep in the rock fades to black paper.
fn wall(cv: &mut Img, room: &Room, col: i32, row: i32) {
    let (x0, y0) = (col * TILE, row * TILE);
    let open = |dc: i32, dr: i32| !solid(room, col + dc, row + dr);
    let (n, e, s, w) = (open(0, -1), open(1, 0), open(0, 1), open(-1, 0));
    let reach = (1..=3)
        .find(|&d| (-d..=d).any(|dy| (-d..=d).any(|dx| (dx != 0 || dy != 0) && is_open(room, col + dx, row + dy))))
        .unwrap_or(4);
    for py in 0..TILE {
        for px in 0..TILE {
            let (x, y) = (x0 + px, y0 + py);
            // Brick courses, eight high, staggered; each brick its own grey.
            let course = (y / 8) % 2 * 8;
            let (bx, by) = ((x + course) / 16, y / 8);
            let (lx, ly) = ((x + course) % 16, y % 8);
            let tone = match hash(bx, by, 57) % 4 {
                0 => 'j',
                1 | 2 => 'g',
                _ => 'X',
            };
            let mut ink = if ly == 7 || lx == 15 {
                'k'
            } else if ly == 0 && hash(x, y, 4) % 3 != 0 {
                'G'
            } else if hash(x, y, 91) % 17 == 0 {
                'k'
            } else {
                tone
            };
            let near = |open: bool, d: i32| open && d < 2;
            if near(s, TILE - 1 - py) {
                ink = if TILE - 1 - py == 0 { 'h' } else { 'G' };
            } else if near(w, px) || near(n, py) {
                ink = if bayer(x, y) < 0.5 { 'j' } else { 'g' };
            } else if near(e, TILE - 1 - px) {
                ink = 'X';
            }
            if reach >= 3 {
                ink = if hash(x, y, 13) % 19 == 0 { 'X' } else { 'k' };
            } else if reach == 2 && bayer(x, y) < 0.55 {
                ink = if hash(x, y, 13) % 9 == 0 { 'X' } else { 'k' };
            }
            cv.put(x, y, ink);
        }
    }
    if north_edge(room, col, row) {
        // A heavy iron rail along the wall's head, rivets every tile.
        cv.rect(x0, y0 + TILE - 5, TILE, 1, 'k');
        cv.rect(x0, y0 + TILE - 4, TILE, 2, 'g');
        cv.rect(x0, y0 + TILE - 4, TILE, 1, 'J');
        cv.put(x0 + 3, y0 + TILE - 3, 'h');
        cv.put(x0 + 12, y0 + TILE - 3, 'h');
        if lantern(room, col, row) {
            // The lantern's iron cage hangs from a bracket; its flame and
            // light are `set`'s (it flickers).
            cv.rect(x0 + 6, y0 + 2, 4, 1, 'G');
            cv.rect(x0 + 7, y0 + 3, 2, 3, 'g');
            cv.rect(x0 + 5, y0 + 6, 6, 1, 'J');
            cv.rect(x0 + 5, y0 + 7, 1, 5, 'G');
            cv.rect(x0 + 10, y0 + 7, 1, 5, 'g');
            cv.rect(x0 + 6, y0 + 7, 4, 5, 'Z');
            cv.rect(x0 + 5, y0 + 12, 6, 1, 'J');
            cv.rect(x0 + 6, y0 + 13, 4, 1, 'k');
        } else if hash(col, row, 71) % 4 == 0 {
            // Two kegs stood against the wall, one on the other's shoulder.
            cv.stamp(small_keg(), x0 + 1, y0 + 2);
            cv.stamp(small_keg(), x0 + 8, y0 + 3);
        }
    }
}

/// A little keg, seen from the front: a stave barrel with two hoops.
fn small_keg() -> &'static Img {
    static KEG: OnceLock<Img> = OnceLock::new();
    KEG.get_or_init(|| {
        Img::from_rows(&[
            "..kkkkkk..",
            ".koOOOOok.",
            "kORRRRRRpk",
            "kjhhhhhhgk",
            "kRRRRrrrpk",
            "kRRRrrrppk",
            "kjhhhhhhgk",
            "kRRRrrrppk",
            ".kpppppbk.",
            "..kkkkkk..",
        ])
    })
}

/// A stone pier for the pillars: a lit cap, a brass stud at each corner of
/// it, and a shadowed shaft in courses; wider than it looks so it reads as
/// the immovable thing.
pub(in super::super) fn pier() -> &'static Img {
    static PIER: OnceLock<Img> = OnceLock::new();
    PIER.get_or_init(|| {
        let mut im = Img::new(TILE, TILE);
        im.rect(0, 0, 16, 16, 'k');
        // The cap: a slab of cold stone, lit from the upper left.
        im.rect(1, 1, 14, 8, 'U');
        im.rect(1, 1, 14, 1, 'V');
        im.rect(1, 1, 1, 8, 'V');
        im.rect(2, 2, 12, 1, 'v');
        im.rect(14, 2, 1, 7, 'u');
        im.rect(2, 8, 13, 1, 'u');
        // Brass studs.
        for (x, y) in [(3, 3), (12, 3), (3, 6), (12, 6)] {
            im.put(x, y, 't');
            im.put(x + 1, y, 'o');
            im.put(x, y + 1, 'o');
            im.put(x + 1, y + 1, 'R');
        }
        // The sigil in the cap's middle: a bomb in a ring of rivets.
        im.rect(6, 4, 4, 3, 'u');
        im.put(7, 5, 'k');
        im.put(8, 5, 'k');
        im.put(8, 4, '6');
        // The shaft: a shadowed face in two courses.
        im.rect(1, 9, 14, 6, 'S');
        im.rect(1, 9, 14, 1, 'x');
        im.rect(1, 12, 14, 1, 's');
        im.rect(1, 14, 14, 1, 's');
        im.rect(7, 10, 1, 2, 's');
        im.rect(4, 13, 1, 1, 's');
        im.rect(11, 13, 1, 1, 's');
        im.rect(1, 9, 1, 6, 'u');
        // An iron band round the foot.
        im.rect(1, 13, 14, 1, 'g');
        im.rect(1, 13, 14, 1, 'j');
        im
    })
}

/// A crate of the picture `kind`: iron-bound, the top face lit, a dark
/// outline; the third wears the red band of powder, the second a brace.
pub(in super::super) fn crate_img(kind: usize) -> &'static Img {
    static CRATES: OnceLock<[Img; 4]> = OnceLock::new();
    &CRATES.get_or_init(|| std::array::from_fn(make_crate))[kind % 4]
}

fn make_crate(kind: usize) -> Img {
    let mut im = Img::new(TILE, TILE);
    im.rect(0, 0, 16, 16, 'n');
    // The lid, seen from above, and the front below it.
    im.rect(1, 1, 14, 5, 'O');
    im.rect(1, 1, 14, 1, 'T');
    im.rect(1, 1, 1, 5, 't');
    im.rect(1, 5, 14, 1, 'o');
    im.rect(2, 3, 12, 1, 'o');
    im.rect(1, 6, 14, 9, 'r');
    im.rect(1, 6, 14, 1, 'p');
    im.rect(1, 7, 1, 7, 'R');
    im.rect(14, 7, 1, 8, 'B');
    im.rect(1, 14, 14, 1, 'p');
    // Plank seams and grain.
    im.rect(2, 10, 12, 1, 'p');
    for (x, y) in [(4, 8), (9, 8), (6, 12), (11, 12), (3, 12), (12, 8)] {
        im.put(x, y, 'R');
    }
    match kind % 4 {
        1 => {
            // A brace: two planks crossed on the front.
            for i in 0..9 {
                im.put(3 + i, 7 + i * 7 / 8, 'p');
                im.put(3 + i, 8 + i * 7 / 8, 'R');
                im.put(12 - i, 7 + i * 7 / 8, 'p');
                im.put(12 - i, 8 + i * 7 / 8, 'R');
            }
        }
        2 => {
            // The red band and its bomb.
            im.rect(2, 8, 12, 5, '7');
            im.rect(2, 8, 12, 1, 'R');
            im.rect(2, 12, 12, 1, 'p');
            im.rect(6, 9, 4, 3, 'k');
            im.put(7, 9, 'h');
            im.put(9, 8, '6');
            im.put(10, 8, '9');
        }
        3 => {
            // A rope round it, and a plank sprung from the lid.
            for i in 0..14 {
                im.put(1 + i, 7 + (i % 2), '$');
                im.put(1 + i, 11 + (i % 2), 'P');
            }
            im.rect(9, 2, 5, 1, 'T');
            im.rect(10, 1, 4, 1, 'T');
        }
        _ => {}
    }
    // Iron straps down the lid and front, nailed.
    if kind % 4 != 1 {
        for x in [3, 11] {
            im.rect(x, 1, 2, 14, 'g');
            im.rect(x, 1, 1, 14, 'G');
            im.put(x, 2, 'h');
            im.put(x + 1, 12, 'h');
        }
    } else {
        for x in [2, 13] {
            im.rect(x, 1, 1, 14, 'G');
        }
    }
    // The outline gives way a little at the corners.
    for (x, y) in [(0, 0), (15, 0), (0, 15), (15, 15)] {
        im.clear(x, y);
    }
    im
}
