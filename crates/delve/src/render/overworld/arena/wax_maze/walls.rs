//! The maze's stone and floor, laid once with the room's scenery (`scenery`
//! asks for a pillar's tile or a lane's): old violet stone with a lit rim
//! along every lane, so the walls read as walls; and dark lanes with a warm
//! path worn down their middles, where the wax lies. Nothing here moves.

use super::super::super::ink::{bayer, hash};
use super::super::{Img, TILE};
use crate::rules::{Room, Tile};

/// Whether a tile is stone (the room's own walls and the maze's pillars);
/// beyond the room counts as stone.
fn stone(room: &Room, c: i32, r: i32) -> bool {
    c < 0 || r < 0 || c >= room.cols as i32 || r >= room.rows as i32 || matches!(room.tile(c, r), Tile::Block | Tile::Wall)
}

/// What the maze lays on `room`'s tile `(col, row)`: a pillar's stone, a
/// lane's floor, or nothing (the room's walls and doors keep the pack's own).
pub(in super::super) fn lay(cv: &mut Img, room: &Room, col: i32, row: i32) {
    match room.tile(col, row) {
        Tile::Block | Tile::Wall => pillar(cv, room, col, row),
        Tile::Floor => lane(cv, room, col, row),
        _ => {}
    }
}

/// The distance from pixel `(px, py)` of a tile to the nearest lane edge
/// of it, and whether that edge faces the light (its north or west side).
fn rim(room: &Room, col: i32, row: i32, px: i32, py: i32, solid: bool) -> (f32, bool) {
    // Which sides of this tile meet the other kind (a pillar's lane sides;
    // a lane's pillar sides).
    let edge = |dc: i32, dr: i32| stone(room, col + dc, row + dr) != solid;
    let (n, e, s, w) = (edge(0, -1), edge(1, 0), edge(0, 1), edge(-1, 0));
    let (ne, se, sw, nw) = (edge(1, -1), edge(1, 1), edge(-1, 1), edge(-1, -1));
    let (fx, fy) = (px as f32, py as f32);
    let (rx, ry) = ((TILE - 1 - px) as f32, (TILE - 1 - py) as f32);
    let mut best = (99.0f32, false);
    let mut take = |d: f32, lit: bool| {
        if d < best.0 {
            best = (d, lit);
        }
    };
    if n {
        take(fy, true);
    }
    if w {
        take(fx, true);
    }
    if s {
        take(ry, false);
    }
    if e {
        take(rx, false);
    }
    // A corner where two lanes meet round a pillar: the diagonal's own.
    if nw && !n && !w {
        take(fx.hypot(fy), true);
    }
    if ne && !n && !e {
        take(rx.hypot(fy), true);
    }
    if sw && !s && !w {
        take(fx.hypot(ry), false);
    }
    if se && !s && !e {
        take(rx.hypot(ry), false);
    }
    best
}

/// A pillar's tile: a lit rim along each lane side (bright to the north and
/// west, shaded to the south and east), the corners rounded, and old stone
/// inside with its courses and flecks.
fn pillar(cv: &mut Img, room: &Room, col: i32, row: i32) {
    let (x0, y0) = (col * TILE, row * TILE);
    let open = |dc: i32, dr: i32| !stone(room, col + dc, row + dr);
    let (n, w, s, e) = (open(0, -1), open(-1, 0), open(0, 1), open(1, 0));
    for py in 0..TILE {
        for px in 0..TILE {
            let (x, y) = (x0 + px, y0 + py);
            // A convex corner is cut away, so a pillar is not a plain box.
            let cut = (n && w && px + py < 2) || (n && e && (TILE - 1 - px) + py < 2) || (s && w && px + (TILE - 1 - py) < 2) || (s && e && (TILE - 1 - px) + (TILE - 1 - py) < 2);
            if cut {
                cv.put(x, y, 'k');
                continue;
            }
            let (d, lit) = rim(room, col, row, px, py, true);
            // Inks one step above what the room's dusk leaves: the dusk
            // steps each down its bank.
            let ink = if d < 1.0 {
                // The lip catches the light now and then.
                if lit && bayer(x, y) < 0.2 { '^' } else if lit { ']' } else { '[' }
            } else if d < 2.0 {
                if lit { '[' } else { ')' }
            } else if d < 4.0 {
                if bayer(x, y) < (4.0 - d) * 0.3 { '[' } else { ')' }
            } else {
                // Courses of stone: a joint every eight rows, staggered
                // between them, and the odd fleck.
                let course = (y / 8) % 2 * 8;
                if y % 8 == 7 || (x + course) % 16 == 15 {
                    '('
                } else if hash(x, y, 91) % 29 == 0 {
                    '['
                } else {
                    ')'
                }
            };
            cv.put(x, y, ink);
        }
    }
}

/// A lane's tile: black paper with faint flags, a warm worn path down its
/// middle, and wax that has run down off the pillar to the north.
fn lane(cv: &mut Img, room: &Room, col: i32, row: i32) {
    let (x0, y0) = (col * TILE, row * TILE);
    for py in 0..TILE {
        for px in 0..TILE {
            let (x, y) = (x0 + px, y0 + py);
            let (d, _) = rim(room, col, row, px, py, false);
            // How far from the stone: the path's warmth is its middle's.
            let mid = ((d - 1.0) / 6.0).clamp(0.0, 1.0);
            let ink = if d < 1.0 {
                'k'
            } else if (x % 8 == 0 || y % 8 == 0) && d >= 2.0 {
                'K'
            } else if bayer(x, y) < 0.26 * mid {
                'Z'
            } else if hash(x, y, 17) % 37 == 0 {
                'K'
            } else {
                'k'
            };
            cv.put(x, y, ink);
        }
    }
    // Wax run down the pillar's south face onto the lane.
    if stone(room, col, row - 1) && matches!(room.tile(col, row - 1), Tile::Block) {
        for k in 0..2u32 {
            let h = hash(col, row, 71 + k);
            if h % 5 > 1 {
                continue;
            }
            let dx = 3 + (h / 5 % 10) as i32;
            let len = 2 + (h / 50 % 5) as i32;
            for dy in 0..len {
                cv.put(x0 + dx, y0 + dy, if dy + 1 == len { 'o' } else { 'r' });
                if dy < len - 1 {
                    cv.put(x0 + dx + 1, y0 + dy, 'P');
                }
            }
        }
    }
}
