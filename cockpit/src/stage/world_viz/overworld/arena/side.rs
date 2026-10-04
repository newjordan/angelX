//! A side-on hall in the same kit: the pack's rock for its stone, a dim
//! coursed back wall behind the open air, timber planks on brackets, iron
//! spikes at the bottom of its pits, torches on the wall, and the same dusk.

use super::*;

/// Paint a side-on hall's scenery and light it for dusk.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let pack = run.dungeon.pack;
    let st = style(pack);
    let tick = run.tick as u32;
    let barred = run.barred();
    let tiles = Tiles::get();
    let (pw, ph) = room_px(room);
    let mut cv = Img::black(pw, ph);
    let mut lights = Vec::new();
    let open = |col: i32, row: i32| {
        matches!(
            room.tile(col, row),
            Tile::Floor | Tile::Ledge | Tile::Door | Tile::Hazard
        )
    };

    // The back wall: coursed stone, its joints only half drawn.
    let mortar = st.pool.0;
    for y in 0..ph {
        for x in 0..pw {
            if !open(x / TILE, y / TILE) || room.tile(x / TILE, y / TILE) == Tile::Hazard {
                continue;
            }
            let course = y.div_euclid(8);
            let lx = (x + if course % 2 == 0 { 0 } else { 8 }).rem_euclid(16);
            if (y.rem_euclid(8) == 0 || lx == 0) && !hash(x / 2, y, 77).is_multiple_of(3) {
                cv.put(x, y, mortar);
            }
        }
    }
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let (x, y) = (col * TILE, row * TILE);
            let v = hash(col, row, 211);
            match room.tile(col, row) {
                Tile::Wall | Tile::Block => cv.stamp(tiles.rock(st.rock, v), x, y),
                Tile::Ledge => {
                    let left = room.tile(col - 1, row) != Tile::Ledge;
                    let right = room.tile(col + 1, row) != Tile::Ledge;
                    plank(&mut cv, x, y, left, right, v);
                }
                // Iron spikes in the bottom of a pit.
                Tile::Hazard if !open(col, row + 1) => {
                    for k in 0..4 {
                        let sx = x + 2 + k * 4;
                        for h in 0..6 {
                            let ink = if h >= 4 { 'h' } else { 'J' };
                            cv.put(sx, y + TILE - 1 - h, ink);
                            if h < 3 {
                                cv.put(sx - 1, y + TILE - 1 - h, 'G');
                                cv.put(sx + 1, y + TILE - 1 - h, 'j');
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // Torches along the back wall, high up where the air is open.
    let mut col = 6;
    while col < room.cols as i32 - 2 {
        let row = 2;
        if open(col, row) && open(col, row + 1) {
            let (x, y) = (col * TILE + 6, row * TILE + 4);
            cv.stamp(&torch(tick / 4, col as u32), x, y);
            lights.push(fire(x + 2, y + 1, 56.0, 0.32));
        }
        col += 10;
    }
    // The doorway: a dark arch in the west wall, torches above, bars down
    // while the hall is fought.
    if let Some(row) = (0..room.rows as i32).find(|&r| room.tile(0, r) == Tile::Door) {
        let y = row * TILE;
        cv.stamp(&torch(tick / 4, 7), 4, y - 12);
        lights.push(fire(6, y - 11, 48.0, 0.3));
        if barred {
            cv.stamp(&sprites::portcullis(false), 0, y);
        }
    }
    dusk(&mut cv, &lights, st.pool);
    cv
}

/// A timber plank on the tile at `(x, y)`: a lit top edge, grain, a dark
/// underside, and a bracket under each end.
fn plank(cv: &mut Img, x: i32, y: i32, left: bool, right: bool, v: u32) {
    for dx in 0..TILE {
        cv.put(x + dx, y, 'o');
        cv.put(x + dx, y + 1, 'R');
        let grain = (dx + v as i32).rem_euclid(7) == 0;
        cv.put(x + dx, y + 2, if grain { 'I' } else { 'B' });
        cv.put(x + dx, y + 3, 'p');
        cv.put(x + dx, y + 4, 'b');
    }
    for (end, toward) in [(left, 1), (right, -1)] {
        if !end {
            continue;
        }
        let bx = if toward == 1 { x + 2 } else { x + TILE - 3 };
        for k in 0..6 {
            cv.put(bx + toward * k / 2, y + 5 + k, 'I');
        }
    }
}

/// A banner over a doorway that leads side-on: crimson, with a gold
/// chevron, hung from an iron rod.
pub(super) fn banner() -> Img {
    Img::from_rows(&[
        "jJJJJJJJj",
        ".7777777.",
        ".7775777.",
        ".7757577.",
        ".7577757.",
        ".7777777.",
        ".7757577.",
        ".7577757.",
        ".7777777.",
        ".777.777.",
        ".77...77.",
        ".7.....7.",
    ])
}
