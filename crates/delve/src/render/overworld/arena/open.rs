//! An open floor's map drawn (`rules::depths`): the rock between its
//! chambers left as the dark the old rooms stood in, so only the walls that
//! face a floor are drawn and every chamber and passage reads as walled;
//! braziers in its chambers' corners, torches beside their doors and the
//! guardian's gates (barred while it seals), sconces along the passages.

use super::*;
use crate::rules::layout::Open;

/// A wall tile of an open map with no floor round it (diagonals too):
/// rock, left dark.
pub(super) fn deep_rock(room: &Room, col: i32, row: i32) -> bool {
    room.kind == RoomKind::Depths
        && (-1..=1).all(|dr| (-1..=1).all(|dc| matches!(room.tile(col + dc, row + dr), Tile::Wall)))
}

/// The open map's fixtures and their lights, drawn once into its scenery.
pub(super) fn fixtures(run: &Run, cv: &mut Img, lights: &mut Vec<Light>) {
    let Some(open) = run.open_here() else {
        return;
    };
    let room = run.room();
    let tick = run.tick as u32;
    let sealed = run.barred();
    corner_braziers(open, cv, lights, tick);
    sconces(room, open, cv, lights, tick);
    // Torches beside each door off the map, a banner over one that leads
    // side-on.
    for (k, door) in open.doors.iter().enumerate() {
        let [(c0, r0), _] = door.tiles();
        if room.tile(c0, r0) != Tile::Door {
            continue;
        }
        flank(cv, lights, (c0, r0), usize::from(door.dir), 2, tick, 40 + k as u32);
        if matches!(run.dungeon.rooms[door.room].kind, RoomKind::Ledge | RoomKind::Wanted) {
            let banner = side::banner();
            let (x, y) = (c0 * TILE, r0 * TILE);
            let (bx, by) = match door.dir {
                0 => (x + TILE - banner.w / 2, y + 1),
                2 => (x + TILE - banner.w / 2, y + TILE - banner.h - 1),
                1 => (x + TILE - banner.w - 1, y + TILE - banner.h / 2),
                _ => (x + 1, y + TILE - banner.h / 2),
            };
            cv.stamp(&banner, bx, by);
        }
        if sealed {
            for (c, r) in door.tiles() {
                bar(cv, c, r, door.dir.is_multiple_of(2));
            }
        }
    }
    // The guardian's mouths: torches either side, bars while it seals.
    for ch in &open.chambers {
        let (x0, y0) = (i32::from(ch.rect.0), i32::from(ch.rect.1));
        for &(c, r) in &ch.gates {
            let (c, r) = (i32::from(c), i32::from(r));
            let across = r == y0 || r == y0 + i32::from(ch.rect.3) - 1;
            if sealed {
                bar(cv, c, r, across);
            }
            let first = if across { !ch.gates.contains(&((c - 1) as u16, r as u16)) } else { !ch.gates.contains(&(c as u16, (r - 1) as u16)) };
            if first {
                let wide = (0..6)
                    .take_while(|&k| if across { ch.gates.contains(&((c + k) as u16, r as u16)) } else { ch.gates.contains(&(c as u16, (r + k) as u16)) })
                    .count() as i32;
                let side = if across { if r == y0 { 0 } else { 2 } } else if c == x0 { 3 } else { 1 };
                flank(cv, lights, (c, r), side, wide, tick, 90 + (c * 7 + r) as u32);
            }
        }
    }
}

/// A brazier in each inner corner of every chamber, as each old room had
/// in its corners.
fn corner_braziers(open: &Open, cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    for (i, ch) in open.chambers.iter().enumerate() {
        let (x, y, w, h) = (i32::from(ch.rect.0), i32::from(ch.rect.1), i32::from(ch.rect.2), i32::from(ch.rect.3));
        let lit = if ch.guarded() { 0.5 } else { 0.32 };
        for (k, (c, r)) in [(x, y), (x + w - 1, y), (x, y + h - 1), (x + w - 1, y + h - 1)].into_iter().enumerate() {
            let v = hash(c, r, 101 + i as u32);
            let im = sprites::brazier(false, tick / 4 + k as u32, v);
            let (px, py) = (c * TILE, r * TILE);
            cv.stamp(&im, px + (TILE - im.w) / 2, py + TILE - im.h);
            lights.push(fire(px + TILE / 2, py + 4, 64.0, lit));
        }
        // A great chamber's long walls keep a torch along them too.
        if w > COLS as i32 + 4 {
            for (c, r) in [(x + w / 4, y), (x + 3 * w / 4, y)] {
                let im = torch(tick / 4, (c * 3 + r) as u32);
                let (px, py) = (c * TILE + TILE / 2 - 2, r * TILE + 6);
                cv.stamp(&im, px, py);
                lights.push(fire(px + 2, py + 1, 52.0, 0.3));
            }
        }
    }
}

/// Torches on the walls either side of a doorway of `wide` tiles whose
/// first tile is `(c, r)`, in a wall on `side` of its chamber.
fn flank(cv: &mut Img, lights: &mut Vec<Light>, (c, r): (i32, i32), side: usize, wide: i32, tick: u32, seed: u32) {
    let across = side.is_multiple_of(2);
    let (x, y) = (c * TILE, r * TILE);
    let spots = if across {
        [(x - TILE / 2, y), (x + wide * TILE + TILE / 2, y)]
    } else {
        [(x, y - TILE / 2), (x, y + wide * TILE + TILE / 2)]
    };
    for (k, (fx, fy)) in spots.into_iter().enumerate() {
        let im = torch(tick / 4, seed + k as u32);
        let (tx, ty) = if across { (fx - 2, fy + 3) } else { (fx + TILE / 2 - 2, fy - 4) };
        cv.stamp(&im, tx, ty);
        lights.push(fire(tx + 2, ty + 1, 40.0, 0.3));
    }
}

/// A doorway tile barred: half a two-tile portcullis.
fn bar(cv: &mut Img, c: i32, r: i32, across: bool) {
    let im = sprites::portcullis(across);
    let half = if across { Img::from_view(&im, 0, 0, TILE, im.h) } else { Img::from_view(&im, 0, 0, im.w, TILE) };
    cv.stamp(&half, c * TILE, r * TILE);
}

/// Sconces along the passages: on the north wall of a passage running
/// east-west, the west wall of one running north-south, a few tiles apart.
fn sconces(room: &Room, open: &Open, cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let near_chamber = |c: i32, r: i32| {
        open.chambers.iter().any(|ch| {
            let (x, y, w, h) = (i32::from(ch.rect.0), i32::from(ch.rect.1), i32::from(ch.rect.2), i32::from(ch.rect.3));
            c >= x - 2 && c <= x + w + 1 && r >= y - 2 && r <= y + h + 1
        })
    };
    for r in 1..room.rows as i32 - 1 {
        for c in 1..room.cols as i32 - 1 {
            if room.tile(c, r) != Tile::Floor || near_chamber(c, r) {
                continue;
            }
            let wall = |c: i32, r: i32| room.tile(c, r) == Tile::Wall;
            let north = wall(c, r - 1) && wall(c - 1, r - 1) && wall(c + 1, r - 1);
            let west = wall(c - 1, r) && wall(c - 1, r - 1) && wall(c - 1, r + 1);
            if north && c.rem_euclid(9) == 4 {
                let im = torch(tick / 4, (c * 5 + r) as u32);
                let (px, py) = (c * TILE + TILE / 2 - 2, (r - 1) * TILE + 6);
                cv.stamp(&im, px, py);
                lights.push(fire(px + 2, py + 1, 44.0, 0.26));
            } else if west && !wall(c, r - 1) && r.rem_euclid(7) == 3 {
                let im = torch(tick / 4, (c * 3 + r * 5) as u32);
                let (px, py) = ((c - 1) * TILE + TILE - 5, r * TILE + 2);
                cv.stamp(&im, px, py);
                lights.push(fire(px + 2, py + 1, 44.0, 0.26));
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__open__tests.rs"]
mod tests;
