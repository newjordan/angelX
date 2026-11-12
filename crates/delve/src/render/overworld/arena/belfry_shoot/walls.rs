//! The belfry's stone and boards, laid once with the room's scenery
//! (`scenery` asks for each tile): night-grey tower stone in courses with a
//! lit rim and a louvred slit every few tiles; a heavy rafter along the top
//! of the north wall; oak posts for the pillars; and a plank floor, nearly
//! black, with the moon falling through the louvres in dithered shafts.
//! Nothing here moves (`set` has what does).

use super::super::super::ink::{bayer, hash};
use super::super::{Img, TILE};
use crate::rules::{Room, Tile};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Whether a tile is stone (the room's own walls and the posts); beyond the
/// room counts as stone.
fn stone(room: &Room, c: i32, r: i32) -> bool {
    c < 0 || r < 0 || c >= room.cols as i32 || r >= room.rows as i32 || matches!(room.tile(c, r), Tile::Block | Tile::Wall)
}

/// Where the room's furniture goes, found once from its shape: the rooms are
/// caves with ragged stone, so every wall that looks south onto the boards
/// is a north wall for the belfry, wherever it is.
pub(in super::super) struct Plan {
    /// Louvred slits: the middle of the slit and the row of pixels under
    /// its wall, where its shaft of moon begins.
    pub slits: Vec<(i32, i32)>,
    /// Lanterns: the middle and the wall's foot.
    pub lanterns: Vec<(i32, i32)>,
    /// Roosting bats: where their rafter hangs them.
    pub roosts: Vec<(i32, i32)>,
    /// The great bell's pivot, on the straightest stretch of north wall
    /// near the middle of the boards.
    pub bell: (i32, i32),
}

fn is_open(room: &Room, c: i32, r: i32) -> bool {
    matches!(room.tile(c, r), Tile::Floor | Tile::Door | Tile::Stairs)
}

/// A wall tile whose south side is open boards and whose north is stone.
fn north_edge(room: &Room, c: i32, r: i32) -> bool {
    room.tile(c, r) == Tile::Wall && is_open(room, c, r + 1) && stone(room, c, r - 1)
}

fn plan_of(room: &Room) -> Plan {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let (mut slits, mut lanterns, mut roosts) = (Vec::new(), Vec::new(), Vec::new());
    // The top boards of each column: the first row of open floor under stone.
    let mut top: Vec<Option<i32>> = vec![None; cols as usize];
    for c in 0..cols {
        for r in 1..rows - 1 {
            if north_edge(room, c, r - 1) && is_open(room, c, r) {
                top[c as usize] = Some(r);
                break;
            }
        }
        for r in 0..rows {
            if north_edge(room, c, r) {
                let (x, base) = (c * TILE + TILE / 2, (r + 1) * TILE);
                match c.rem_euclid(5) {
                    3 => slits.push((x, base)),
                    0 => lanterns.push((x, base)),
                    _ if hash(c, r, 71) % 4 == 0 => roosts.push((c * TILE + 5, r * TILE + 6)),
                    _ => {}
                }
            }
        }
    }
    // The bell: the middle of the boards' widest stretch of one top row.
    let floor_cols: Vec<i32> = (0..cols).filter(|&c| top[c as usize].is_some()).collect();
    let mid = if floor_cols.is_empty() { cols / 2 } else { (floor_cols[0] + floor_cols[floor_cols.len() - 1]) / 2 };
    let flat = |c: i32| -> i32 {
        let Some(t) = top.get(c as usize).copied().flatten() else { return 0 };
        (-4..=4).filter(|d| top.get((c + d) as usize).copied().flatten() == Some(t)).count() as i32
    };
    let best = floor_cols
        .iter()
        .copied()
        .max_by_key(|&c| (flat(c), -(c - mid).abs()))
        .unwrap_or(mid);
    let row = top.get(best as usize).copied().flatten().unwrap_or(1);
    Plan { slits, lanterns, roosts, bell: (best * TILE + TILE / 2, row * TILE + 2) }
}

/// The room's plan, kept by its shape. A frame asks (`plan_here`) with a
/// cheap key; the bake, once a room, with the room's whole fingerprint.
pub(in super::super) fn plan_here(run: &crate::rules::Run) -> Arc<Plan> {
    thread_local! {
        static LAST: std::cell::RefCell<Option<((u64, u32, usize, usize, usize), Arc<Plan>)>> = const { std::cell::RefCell::new(None) };
    }
    let room = run.room();
    let key = (run.raid_id, run.dungeon.depth, run.at, room.cols, room.rows);
    LAST.with(|last| {
        if let Some((k, p)) = last.borrow().as_ref()
            && *k == key
        {
            return Arc::clone(p);
        }
        let p = plan(room);
        *last.borrow_mut() = Some((key, Arc::clone(&p)));
        p
    })
}

/// The room's plan, kept by its shape.
pub(in super::super) fn plan(room: &Room) -> Arc<Plan> {
    static PLANS: OnceLock<Mutex<HashMap<u64, Arc<Plan>>>> = OnceLock::new();
    let key = room.fingerprint();
    let plans = PLANS.get_or_init(Default::default);
    if let Ok(mut m) = plans.lock() {
        if let Some(p) = m.get(&key) {
            return Arc::clone(p);
        }
        if m.len() > 8 {
            m.clear();
        }
        let p = Arc::new(plan_of(room));
        m.insert(key, Arc::clone(&p));
        return p;
    }
    Arc::new(plan_of(room))
}

/// What the belfry lays on `room`'s tile `(col, row)`.
pub(in super::super) fn lay(cv: &mut Img, room: &Room, col: i32, row: i32) {
    let plan = plan(room);
    match room.tile(col, row) {
        Tile::Wall => wall(cv, room, &plan, col, row),
        Tile::Block => post(cv, room, &plan, col, row),
        Tile::Floor | Tile::Door | Tile::Stairs => planks(cv, &plan, col, row),
        _ => {}
    }
}

/// The moon's light on the floor at pixel `(x, y)`, 0 to 1: a slanting
/// shaft from each slit, broken by the louvre's slats, fading with the fall.
fn moon(plan: &Plan, x: i32, y: i32) -> f32 {
    let mut k = 0.0f32;
    for &(sx, base) in &plan.slits {
        let fall = (y - base) as f32;
        if !(0.0..170.0).contains(&fall) {
            continue;
        }
        // Slants down and to the right, widening as it falls.
        let centre = sx as f32 + fall * 0.62;
        let half = 6.5 + fall * 0.12;
        let d = (x as f32 - centre).abs();
        if d < half {
            let edge = 1.0 - d / half;
            let reach = (1.0 - fall / 150.0).max(0.0);
            // The slats break it into bands.
            let slat = if (y / 3) % 3 == 0 { 0.45 } else { 1.0 };
            k = k.max(edge.sqrt() * reach * slat);
        }
    }
    k
}

/// A plank floor: three boards a tile, butt joints staggered, each board
/// its own brown with a grain and a lit lip; the moon's shafts laid over in
/// cold dither.
fn planks(cv: &mut Img, plan: &Plan, col: i32, row: i32) {
    let (x0, y0) = (col * TILE, row * TILE);
    for py in 0..TILE {
        for px in 0..TILE {
            let (x, y) = (x0 + px, y0 + py);
            // Boards 5, 5 and 6 high; a seam under each.
            let board = if py < 5 { 0 } else if py < 10 { 1 } else { 2 };
            let seam = py == 4 || py == 9 || py == 15;
            let top = py == 0 || py == 5 || py == 10;
            // The butt joints, a different one for each board and row.
            let slide = hash(row, board, 31) % 72;
            let joint = (x + slide as i32) % 72 == 0;
            // Each board has a brown of its own, by where it lies.
            let seg = (x + slide as i32) / 72;
            let tone = match hash(seg, row * 3 + board, 9) % 5 {
                0 => 'Z',
                1 | 2 => 'b',
                3 => 'b',
                _ => 'I',
            };
            let mut ink = if seam || joint {
                'n'
            } else {
                let g = hash(x / 5, y, 17 + board as u32);
                if top && g % 3 != 0 {
                    'I'
                } else if g % 9 == 0 {
                    'Z'
                } else if g % 13 == 0 {
                    'I'
                } else {
                    tone
                }
            };
            let m = moon(plan, x, y);
            if !seam && m > 0.0 {
                if m > 0.7 && bayer(x, y) < m {
                    ink = 'u';
                } else if m > 0.35 && bayer(x, y) < m * 1.1 {
                    ink = 'S';
                } else if bayer(x, y) < m * 1.2 {
                    ink = 'x';
                }
            }
            cv.put(x, y, ink);
        }
    }
}

/// A wall tile: courses of night-grey stone with joints, flecks and a lit
/// rim toward the floor; the north wall's slit and rafter.
fn wall(cv: &mut Img, room: &Room, plan: &Plan, col: i32, row: i32) {
    let (x0, y0) = (col * TILE, row * TILE);
    let open = |dc: i32, dr: i32| !stone(room, col + dc, row + dr);
    let (n, e, s, w) = (open(0, -1), open(1, 0), open(0, 1), open(-1, 0));
    let north_wall = north_edge(room, col, row);
    let slit = north_wall && plan.slits.contains(&(col * TILE + TILE / 2, (row + 1) * TILE));
    // How deep in the rock it lies: the far stone fades to black paper.
    let reach = (1..=3)
        .find(|&d| (-d..=d).any(|dy| (-d..=d).any(|dx| (dx != 0 || dy != 0) && is_open(room, col + dx, row + dy))))
        .unwrap_or(4);
    for py in 0..TILE {
        for px in 0..TILE {
            let (x, y) = (x0 + px, y0 + py);
            // Courses of stone, eight high, staggered; each block its own
            // night-grey, lit along its top and shaded down its right edge.
            let course = (y / 8) % 2 * 8;
            let (bx, by) = ((x + course) / 16, y / 8);
            let (lx, ly) = ((x + course) % 16, y % 8);
            let tone = if hash(bx, by, 57) % 3 == 0 { 'S' } else { 'x' };
            let mut ink = if ly == 7 || lx == 15 {
                's'
            } else if ly == 0 && hash(x, y, 4) % 3 != 0 {
                'u'
            } else if hash(x, y, 91) % 17 == 0 {
                's'
            } else if bayer(x, y) < 0.12 {
                's'
            } else {
                tone
            };
            // A lit rim along every edge that meets the floor: cold on the
            // north and west, deeper on the south and east.
            let near = |open: bool, d: i32| open && d < 2;
            if near(s, TILE - 1 - py) {
                ink = if TILE - 1 - py == 0 { 'u' } else { 'S' };
            } else if near(w, px) || near(n, py) {
                ink = if bayer(x, y) < 0.5 { 'S' } else { 'x' };
            } else if near(e, TILE - 1 - px) {
                ink = 's';
            }
            if reach >= 3 {
                ink = if hash(x, y, 13) % 19 == 0 { 's' } else { 'k' };
            } else if reach == 2 && bayer(x, y) < 0.55 {
                ink = if hash(x, y, 13) % 9 == 0 { 's' } else { 'k' };
            }
            cv.put(x, y, ink);
        }
    }
    if north_wall {
        // The rafter: a heavy oak beam along the wall's head, lit on top,
        // with an iron strap at each tile joint.
        for px in 0..TILE {
            let x = x0 + px;
            cv.put(x, y0, 'k');
            cv.put(x, y0 + 1, 'P');
            cv.put(x, y0 + 2, if hash(x, 3, 7) % 4 == 0 { 'r' } else { 'p' });
            cv.put(x, y0 + 3, 'B');
            cv.put(x, y0 + 4, if hash(x, 5, 9) % 5 == 0 { 'p' } else { 'b' });
            cv.put(x, y0 + 5, 'n');
        }
        if col % 4 == 0 {
            cv.rect(x0 + 6, y0, 3, 6, 'g');
            cv.put(x0 + 7, y0 + 1, 'J');
        }
    }
    if slit {
        // A louvred slit: moon-sky behind slats, a stone sill, a lit frame.
        let (sx, sy) = (x0 + 4, y0 + 6);
        cv.rect(sx - 1, sy - 1, 10, 10, 's');
        for yy in 0..8 {
            for xx in 0..8 {
                let sky = if yy % 3 == 2 { 's' } else if bayer(sx + xx, sy + yy) < 0.5 { 'Q' } else { 'z' };
                cv.put(sx + xx, sy + yy, sky);
            }
        }
        // A star or two in the gaps, and the moon's edge in the middle one.
        cv.put(sx + 2, sy + 1, 'W');
        cv.put(sx + 6, sy + 4, 'V');
        cv.rect(sx - 2, sy + 8, 12, 2, 'u');
        cv.rect(sx - 2, sy - 2, 1, 11, 'S');
        cv.rect(sx + 9, sy - 2, 1, 11, 'g');
    }
}

/// An oak post: a squared timber on a stone footing, lit on its west face,
/// banded in iron.
fn post(cv: &mut Img, room: &Room, plan: &Plan, col: i32, row: i32) {
    let _ = room;
    // The floor under it, so the post stands on boards.
    planks(cv, plan, col, row);
    let (x0, y0) = (col * TILE, row * TILE);
    // Its footing and shadow.
    cv.rect(x0 + 1, y0 + 12, 14, 3, 'g');
    cv.rect(x0 + 1, y0 + 12, 14, 1, 'j');
    cv.rect(x0 + 2, y0 + 15, 13, 1, 'k');
    // The timber.
    cv.rect(x0 + 3, y0 + 1, 10, 11, 'B');
    cv.rect(x0 + 3, y0 + 1, 3, 11, 'r');
    cv.rect(x0 + 4, y0 + 1, 1, 11, 'o');
    cv.rect(x0 + 10, y0 + 1, 3, 11, 'p');
    for y in 1..12 {
        if hash(col, y, 3) % 4 == 0 {
            cv.put(x0 + 7 + (hash(col, y, 4) % 3) as i32, y0 + y, 'I');
        }
    }
    // Iron bands.
    for by in [3, 9] {
        cv.rect(x0 + 2, y0 + by, 12, 2, 'g');
        cv.rect(x0 + 2, y0 + by, 12, 1, 'J');
        cv.put(x0 + 4, y0 + by + 1, 'h');
        cv.put(x0 + 11, y0 + by + 1, 'h');
    }
    cv.rect(x0 + 3, y0, 10, 1, 'k');
}
