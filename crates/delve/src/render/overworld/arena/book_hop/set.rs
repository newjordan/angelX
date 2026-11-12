//! The drowned archive, in the room's pixels, under the knights: bookcases
//! round the hall with owls perched along the top shelf, the dry banks and
//! the landing between the lanes (flagstone laid once and kept), the black
//! river with its current and the pages lost on it, the drifting tomes
//! (their wake and their dip when a knight lands), a lantern hung over the
//! far bank for each seat that must cross, and the pools of light they
//! throw. The still stone is cached; what moves is drawn each frame.

use super::super::super::ink::{Img, bayer, hash};
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{TILE, enlarge, knights};
use super::bits::{self, COVERS, TOME_H};
use crate::motion;
use crate::rules::hop::{self, Hop};
use crate::rules::{HZ, Run, Tile, TILE_UNITS};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Pixels a lane's current carries a thing a tick: the tomes ride at their
/// own pace; the water and its pages drift at a little over half of it.
fn lane_speed(l: usize) -> f32 {
    hop::PACES[l % hop::PACES.len()] / TILE_UNITS * TILE as f32 / HZ as f32
}

/// The book spine inks a shelf is filled with.
const SPINES: [char; 8] = ['7', 'p', 'l', 'e', 'u', ')', 'o', 'r'];

/// One shelf of books laid along `x0..x1` with their feet on `foot`: spines
/// of three to five pixels, five to eight tall, a gilt dash on the tall ones.
fn shelf(im: &mut Img, x0: i32, x1: i32, foot: i32, top: i32, seed: u32, dark: bool) {
    im.rect(x0, top, x1 - x0, foot - top, 'k');
    let mut x = x0;
    let mut k = 0;
    while x < x1 {
        let h = hash(k, seed as i32, 7);
        let w = 2 + (h % 3) as i32;
        let tall = ((foot - top) - 2).min(4 + ((h >> 3) % 4) as i32);
        // A gap now and then, where a book has gone to the river.
        if (h >> 8) % 11 == 0 {
            x += w;
            k += 1;
            continue;
        }
        let ink = SPINES[((h >> 5) % SPINES.len() as u32) as usize];
        let cover = COVERS.iter().find(|c| c.base == ink || c.lit == ink);
        let (lit, base, deep) = cover.map_or((ink, ink, ink), |c| (c.lit, c.base, c.deep));
        let (l, b, d) = if dark { (base, deep, 'k') } else { (lit, base, deep) };
        for xx in x..(x + w).min(x1) {
            im.rect(xx, foot - tall, 1, tall, if xx == x { l } else { b });
        }
        im.rect(x + w - 1, foot - tall, 1, tall, d);
        if tall >= 6 && (h >> 12) % 3 == 0 {
            im.rect(x, foot - tall + 2, w, 1, if dark { 'a' } else { '4' });
        }
        x += w;
        k += 1;
    }
    // The shelf board: lit along its edge, dark beneath.
    im.rect(x0, foot, x1 - x0, 1, if dark { 'p' } else { 'o' });
    im.rect(x0, foot + 1, x1 - x0, 1, 'b');
}

/// A bookcase along the top or foot of the hall: two shelves, a cornice.
pub(super) fn bookcase_row(im: &mut Img, y0: i32, w: i32, seed: u32, dark: bool) {
    shelf(im, 0, w, y0 + 7, y0 + 1, seed, dark);
    shelf(im, 0, w, y0 + 14, y0 + 8, seed ^ 0x55, dark);
    // Pilasters every six tiles, a cornice along the top.
    for px in (0..w).step_by(TILE as usize * 6) {
        im.rect(px, y0, 3, 16, 'B');
        im.rect(px, y0, 1, 16, 'o');
        im.rect(px + 2, y0, 1, 16, 'b');
    }
    im.rect(0, y0, w, 1, 'p');
}

/// A side wall, a tile wide, from `y0` for `rows` tiles: stone courses with
/// a niche at the middle of each tile and a stack of books in it.
fn side_wall(im: &mut Img, x0: i32, y0: i32, rows: i32, seed: u32) {
    for r in 0..rows {
        let y = y0 + r * TILE;
        for yy in y..y + TILE {
            let course = (yy - y) / 8;
            for x in x0..x0 + TILE {
                let joint = (yy - y) % 8 == 7 || (x - x0 + course * 7) % 16 == 15;
                let ink = if joint {
                    'k'
                } else if (yy - y) % 8 == 0 {
                    'S'
                } else if bayer(x, yy) < 0.14 {
                    'S'
                } else {
                    'x'
                };
                im.put(x, yy, ink);
            }
        }
        // The niche: a dark recess with a stack of laid books and a stub of
        // candle (its flame is lit live).
        im.rect(x0 + 3, y + 3, 10, 11, 'k');
        im.rect(x0 + 3, y + 3, 10, 1, 's');
        im.rect(x0 + 3, y + 13, 10, 1, 'S');
        for (j, bw) in [8, 7, 8].into_iter().enumerate() {
            let h = hash(r, j as i32, seed);
            let c = COVERS[(h % COVERS.len() as u32) as usize];
            let by = y + 12 - j as i32 * 3;
            im.rect(x0 + 4, by - 2, bw, 3, c.base);
            im.rect(x0 + 4, by - 2, bw, 1, c.lit);
            im.rect(x0 + 4, by, bw, 1, c.deep);
            im.rect(x0 + 4 + bw - 1, by - 2, 1, 3, 'c');
        }
    }
}

/// The flagstones of a dry row `r`: each lit along its top and shaded at its
/// foot, the joints staggered row to row; `warm` greys them up for the
/// landing.
fn flagstones(im: &mut Img, r: i32, x0: i32, x1: i32, warm: bool) {
    let y = r * TILE;
    let shift = if r % 2 == 0 { 0 } else { TILE / 2 };
    for yy in y..y + TILE {
        for x in x0..x1 {
            let (dx, dy) = ((x - x0 + shift).rem_euclid(TILE), yy - y);
            let stone = hash((x - x0 + shift) / TILE, r, 61);
            let ink = if dx == 0 || dy == TILE - 1 {
                if warm { 'g' } else { 's' }
            } else if dy == 0 {
                if warm { 'J' } else { 'S' }
            } else if dy >= TILE - 3 {
                if warm { 'j' } else { 'x' }
            } else if stone % 3 == 0 {
                if warm { 'G' } else { 'S' }
            } else if stone % 7 == 0 && bayer(x, yy) < 0.2 {
                // Moss in a crack.
                'e'
            } else if warm {
                'j'
            } else {
                'x'
            };
            im.put(x, yy, ink);
        }
    }
}

/// The still hall for a room `cols` by `rows` tiles with doors at `doors`.
fn hall(cols: i32, rows: i32, doors: &[(i32, i32)]) -> Img {
    let (w, h) = (cols * TILE, rows * TILE);
    let mut im = Img::black(w, h);
    bookcase_row(&mut im, 0, w, 3, false);
    bookcase_row(&mut im, (rows - 1) * TILE, w, 9, true);
    side_wall(&mut im, 0, TILE, rows - 2, 21);
    side_wall(&mut im, (cols - 1) * TILE, TILE, rows - 2, 34);
    for r in 1..rows - 1 {
        if hop::lane(rows, r).is_none() {
            flagstones(&mut im, r, TILE, (cols - 1) * TILE, r == hop::landing(rows));
        } else {
            // The river's bed, so the first frame is never bare paper.
            im.rect(TILE, r * TILE, (cols - 2) * TILE, TILE, 's');
        }
    }
    // The landing's gilt rim and its studs: the place to stand safe.
    let l = hop::landing(rows);
    if rows > 6 {
        let (y0, y1) = (l * TILE, l * TILE + TILE - 1);
        for x in TILE..(cols - 1) * TILE {
            if x % 4 < 3 {
                im.put(x, y0 + 1, 'a');
                im.put(x, y1 - 1, 'a');
            }
        }
        for tx in (2..cols - 2).step_by(3) {
            let cx = tx * TILE + TILE / 2;
            im.put(cx, y0 + 8, '4');
            im.put(cx - 1, y0 + 8, 'a');
            im.put(cx + 1, y0 + 8, 'a');
            im.put(cx, y0 + 7, 'a');
            im.put(cx, y0 + 9, 'a');
        }
    }
    // The far bank's gilt lip along the water, and the near bank's.
    let far = hop::far_bank(rows);
    let near = hop::near_bank(rows);
    for x in TILE..(cols - 1) * TILE {
        if x % 6 < 4 {
            im.put(x, far * TILE + TILE - 2, '4');
        }
        im.put(x, far * TILE + TILE - 1, 'a');
        im.put(x, near * TILE, 'S');
    }
    // The shadow of the top case on the far bank, dithered.
    for y in TILE..TILE + 6 {
        for x in TILE..(cols - 1) * TILE {
            if bayer(x, y) < (1.0 - (y - TILE) as f32 / 6.0) * 0.55 {
                im.put(x, y, 'k');
            }
        }
    }
    // The doors (if the walls have them): an arch of dark stone with a gilt
    // keystone, laid over the cases.
    for &(c, r) in doors {
        let (x, y) = (c * TILE, r * TILE);
        im.rect(x, y, TILE, TILE, 'k');
        im.rect(x, y, TILE, 2, 'S');
        im.rect(x, y + TILE - 2, TILE, 2, 'S');
        im.rect(x + 2, y + 2, TILE - 4, TILE - 4, 'K');
        im.put(x + TILE / 2, y + 2, '4');
    }
    im
}

/// The hall for `run`'s room, drawn once and kept.
fn still(run: &Run) -> Arc<Img> {
    type Key = (i32, i32, u64);
    static CACHE: OnceLock<Mutex<HashMap<Key, Arc<Img>>>> = OnceLock::new();
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let doors: Vec<(i32, i32)> = (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (c, r)))
        .filter(|&(c, r)| room.tile(c, r) == Tile::Door)
        .collect();
    let key = (cols, rows, doors.iter().fold(0u64, |a, &(c, r)| a.wrapping_mul(131).wrapping_add((c * 64 + r) as u64 + 1)));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(im) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return im;
    }
    let im = Arc::new(hall(cols, rows, &doors));
    if let Ok(mut c) = cache.lock() {
        if c.len() > 8 {
            c.clear();
        }
        c.insert(key, im.clone());
    }
    im
}

/// The seats at the table, in seat order.
pub(super) fn seats(h: &Hop) -> Vec<u32> {
    h.crossings.keys().copied().collect()
}

/// How many lanterns hang over the far bank: one for each seat, three for a
/// lone knight.
pub(super) fn lantern_count(h: &Hop) -> usize {
    if h.crossings.len() > 1 { h.crossings.len() } else { crate::rules::hop_show::SOLO_LANTERNS as usize }
}

/// Where lantern `k` of `n` hangs (room pixels: the middle of its chain's
/// foot), spread along the far bank between the walls.
pub(super) fn lantern_at(cols: i32, k: usize, n: usize) -> (i32, i32) {
    let span = (cols - 6) * TILE;
    let x = 3 * TILE + span * (2 * k as i32 + 1) / (2 * n.max(1) as i32);
    (x, 24)
}

/// Whether lantern `k` is lit now, and in whose ink: a seat's lantern is
/// lit while that seat has crossed since the archive was last lit; a lone
/// knight's, one for each crossing toward three.
fn lantern_lit(h: &Hop, k: usize) -> Option<char> {
    let ids = seats(h);
    if ids.len() > 1 {
        let id = *ids.get(k)?;
        (h.show.lit.get(&id).copied().unwrap_or(0) > 0).then(|| knights::seat_mark(id))
    } else {
        let id = *ids.first()?;
        (usize::from(h.show.lit.get(&id).copied().unwrap_or(0)) > k).then(|| knights::seat_mark(id))
    }
}

/// The flowing water of lane `l` in row `r`: a current of dashes and the
/// pages lost on it, at `t` ticks into the minute.
fn current(cv: &mut Img, r: i32, l: usize, cols: i32, t: u32) {
    let y = r * TILE;
    let (x0, x1) = (TILE, (cols - 1) * TILE);
    let w = x1 - x0;
    let dir = if l % 2 == 0 { 1.0 } else { -1.0 };
    cv.rect(x0, y, w, TILE, 's');
    // A lighter band along the lane's top edge, where the bank above shows
    // in the water.
    cv.rect(x0, y, w, 2, 'x');
    let drift = lane_speed(l) * 0.55 * t as f32 * dir;
    for k in 0..9 {
        let h = hash(k, l as i32, 311);
        let ly = y + 3 + (h % 10) as i32;
        let len = 4 + ((h >> 4) % 8) as i32;
        let sx = ((h >> 8) % w as u32) as f32 + drift;
        let sx = x0 + (sx.rem_euclid(w as f32)) as i32;
        for i in 0..len {
            let px = x0 + (sx - x0 + i).rem_euclid(w);
            cv.put(px, ly, if i == len - 1 && dir > 0.0 || i == 0 && dir < 0.0 { 'u' } else { 'S' });
        }
    }
    // Sparkles that come and go.
    for k in 0..6 {
        let h = hash(k, l as i32 + 40, 313);
        if (t / 7 + h) % 6 < 2 {
            let px = x0 + ((h >> 3) % w as u32) as i32;
            cv.put(px, y + 3 + ((h >> 14) % 10) as i32, 'U');
        }
    }
    // Pages lost on the current: little cream leaves that bob and turn.
    for k in 0..3 {
        let h = hash(k, l as i32 + 77, 317);
        let sx = ((h % w as u32) as f32 + lane_speed(l) * 0.75 * t as f32 * dir).rem_euclid(w as f32) as i32;
        let bob = ((t / 8 + h) % 4 == 0) as i32;
        let (px, py) = (x0 + sx, y + 4 + ((h >> 6) % 7) as i32 + bob);
        let tilt = (t / 12 + h) % 2 == 0;
        cv.rect(px, py, 4, 2, '$');
        cv.put(px + i32::from(tilt), py - 1, 'c');
        cv.put(px + 3 - i32::from(tilt), py + 2, 's');
    }
}

/// The light a lit lantern throws on the far bank and down the river, as a
/// warm dither with a shimmer.
fn pools(cv: &mut Img, run: &Run, h: &Hop, cols: i32, tick: u32, flare: f32) {
    let n = lantern_count(h);
    for k in 0..n {
        let (lx, _) = lantern_at(cols, k, n);
        let lit = lantern_lit(h, k).is_some();
        let power = if flare > 0.0 { 0.5 + 0.5 * flare } else if lit { 0.4 } else { 0.0 };
        if power <= 0.0 {
            continue;
        }
        // A pool on the bank.
        for dy in 0..14i32 {
            for dx in -22..=22i32 {
                let d = (dx * dx) as f32 / 484.0 + (dy * dy) as f32 / 196.0;
                if d < 1.0 && bayer(lx + dx, 16 + dy) < (1.0 - d) * 0.34 * power {
                    cv.put(lx + dx, 16 + dy, 'Z');
                }
            }
        }
        // Its reflection, a dither column that wavers down the first lanes.
        let rows = (run.room().rows as i32).min(14);
        for y in 32..(rows / 2) * TILE {
            let fall = 1.0 - (y - 32) as f32 / ((rows / 2) * TILE - 32) as f32;
            let sway = (motion::math::sin(y as f32 * 0.35 + tick as f32 * 0.12 + k as f32) * 1.6) as i32;
            for dx in -3..=3i32 {
                let b = bayer(lx + dx + sway, y);
                let across = 1.0 - dx.abs() as f32 / 4.0;
                if b < fall * fall * fall * 0.07 * power * across {
                    cv.put(lx + dx + sway, y, 'a');
                } else if b < fall * fall * 0.2 * power * across {
                    cv.put(lx + dx + sway, y, 'Z');
                }
            }
        }
    }
}

/// Which seat stands where on the near bank before the start: their pad.
fn pads(cv: &mut Img, h: &Hop, cols: i32, rows: i32) {
    let ids = seats(h);
    let seated = ids.len().max(1) as f32;
    let y = hop::near_bank(rows) * TILE + 12;
    for (k, &id) in ids.iter().enumerate() {
        let x = ((cols as f32 / 2.0 + (k as f32 - (seated - 1.0) / 2.0) * 2.0) * TILE as f32) as i32;
        let ink = knights::seat_mark(id);
        for dy in -3..=3i32 {
            for dx in -9..=9i32 {
                let d = (dx * dx) as f32 / 81.0 + (dy * dy) as f32 / 9.0;
                if (0.62..1.0).contains(&d) && bayer(x + dx, y + dy) < 0.8 {
                    cv.put(x + dx, y + dy, ink);
                }
            }
        }
    }
}

/// The owls on the top shelf: their frame at `tick`, given what the table is
/// cheering.
fn owls(cv: &mut Img, run: &Run, h: &Hop, cols: i32, tick: u32) {
    let n = 6;
    let w = cols * TILE;
    let cheer = h.show.crosses.iter().map(|c| tick.saturating_sub(c.born)).min().filter(|&a| a < 30);
    let flare = h.show.flared.map(|f| tick.saturating_sub(f)).filter(|&a| a < crate::rules::hop_show::LIT_LIFE);
    let sunk = h.show.sinks.iter().map(|s| tick.saturating_sub(s.born)).min().filter(|&a| a < 12);
    let _ = run;
    for k in 0..n {
        let x = 22 + (w - 56) * k / (n - 1) + (hash(k, 4, 5) % 8) as i32;
        let phase = hash(k, 1, 0x0f) % 40;
        // The cheer rolls along the row of owls, one a tick behind the next.
        let wave = cheer.map(|a| a >= k as u32 * 2).unwrap_or(false);
        let frame = if flare.is_some() || wave {
            2
        } else if (tick / 3 + phase) % 33 == 0 {
            1
        } else {
            0
        };
        let hop_up = i32::from(flare.is_some() && (tick / 4 + k as u32) % 2 == 0) * 2 + i32::from(sunk.is_some() && frame == 0);
        let owl = bits::owl(frame);
        cv.stamp(owl, x, 2 - hop_up);
    }
}

/// The lanterns over the far bank, lit in the seats' own inks, twice the
/// size of the pixel art so the table's goal reads across the hall.
fn lanterns(cv: &mut Img, h: &Hop, cols: i32, tick: u32) {
    let n = lantern_count(h);
    let flare = h.show.flared.map(|f| tick.saturating_sub(f)).filter(|&a| a < crate::rules::hop_show::LIT_LIFE);
    for k in 0..n {
        let (x, _) = lantern_at(cols, k, n);
        let lit = if flare.is_some() { Some('6') } else { lantern_lit(h, k) };
        let swing = if lit.is_some() { [0, 0, 1, 1][((tick / 9 + k as u32) % 4) as usize] } else { 0 };
        // The chain from the rafter.
        for yy in 0..4 {
            cv.put(x + swing, yy, 'j');
            cv.put(x + 1 + swing, yy, 'G');
        }
        let lamp = enlarge(&bits::lantern(lit, tick + k as u32 * 2), 2.0);
        if lit.is_some() {
            let mut pen = Pen::new(cv, 160);
            let power = if flare.is_some() { 0.75 } else { 0.5 };
            pen.glow(((x + swing) as f32, 13.0), 17.0, 1.0, ramps::GOLD, power);
        }
        cv.stamp(&lamp, x - 7 + swing, 4);
        // The seat's plate under it, so every lantern is somebody's, lit or cold.
        let ids = seats(h);
        let owner = if ids.len() > 1 { ids.get(k) } else { ids.first() };
        if let Some(&id) = owner {
            cv.rect(x - 5 + swing, 23, 11, 4, 'k');
            cv.rect(x - 4 + swing, 24, 9, 2, knights::seat_mark(id));
        }
    }
}

/// A tome of lane `l` at left edge `x0`: the book with its bob, wake and
/// shadow.
#[allow(clippy::too_many_arguments)]
fn tome_at(cv: &mut Img, y: i32, x0: i32, len: i32, cover: usize, dir: f32, l: usize, bob: i32, dip: bool, lo: i32, hi: i32) {
    let book = bits::tome(len, cover, dip);
    let by = y + 1 + bob + i32::from(dip);
    // Its shadow in the ink under the front edge.
    for x in (x0 + 2)..(x0 + len - 2) {
        if x >= lo && x < hi && bayer(x, by + TOME_H) < 0.7 {
            cv.put(x, by + TOME_H, 'k');
        }
    }
    // The wake: a bow wave on the leading end, short dashes behind.
    let (lead, tail) = if dir > 0.0 { (x0 + len, x0 - 1) } else { (x0 - 1, x0 + len) };
    let step = if dir > 0.0 { 1 } else { -1 };
    for (i, (dx, dy)) in [(1, 4), (2, 5), (3, 7), (2, 9), (1, 10)].into_iter().enumerate() {
        let px = lead + step * dx;
        if px >= lo && px < hi {
            cv.put(px, by + dy, if i % 2 == 0 { 'u' } else { 'U' });
        }
    }
    for j in 0..3 {
        for i in 0..(4 - j) {
            let px = tail - step * (2 + j * 5 + i);
            if px >= lo && px < hi {
                cv.put(px, by + 6 + (j % 2) * 4, if j == 0 { 'u' } else { 'S' });
            }
        }
    }
    let _ = l;
    // The book itself, clipped to the river's banks.
    for yy in 0..book.h {
        for xx in 0..book.w {
            let px = x0 + xx;
            if px < lo || px >= hi {
                continue;
            }
            if let Some(c) = book.get(xx, yy) {
                cv.set(px, by + yy, c);
            }
        }
    }
}

/// The hall under the knights for the seat's view of the room.
pub(super) fn under(cv: &mut Img, run: &Run, h: &Hop) {
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let width = cols as f32 * TILE_UNITS;
    let tick = run.tick as u32;
    let t = h.t.saturating_sub(crate::rules::folly::TITLE);
    cv.stamp(&still(run), 0, 0);
    let flare = h
        .show
        .flared
        .map(|f| tick.saturating_sub(f))
        .filter(|&a| a < crate::rules::hop_show::LIT_LIFE)
        .map_or(0.0, |a| 1.0 - a as f32 / crate::rules::hop_show::LIT_LIFE as f32);
    let (lo, hi) = (TILE, (cols - 1) * TILE);
    for r in 1..rows - 1 {
        let Some(l) = hop::lane(rows, r) else { continue };
        current(cv, r, l, cols, t);
    }
    pools(cv, run, h, cols, tick, flare);
    // The books, over the water's own lights.
    for r in 1..rows - 1 {
        let Some(l) = hop::lane(rows, r) else { continue };
        let y = r * TILE;
        let dir = if l % 2 == 0 { 1.0 } else { -1.0 };
        let (lefts, len) = hop::books(l, t, width);
        let w = (len / TILE_UNITS * TILE as f32) as i32;
        let span = lefts.len() as i32 * hop::SPACING as i32 * TILE;
        for (k, &bx) in lefts.iter().enumerate() {
            let x0 = (bx / TILE_UNITS * TILE as f32) as i32;
            // A book running off one end comes back in at the other.
            for piece in [x0, x0 - span] {
                if piece + w + 5 < lo || piece - 5 >= hi {
                    continue;
                }
                let bob = [0, 0, 1, 1][((tick / 7 + k as u32 * 3 + l as u32 * 2) % 4) as usize];
                // A landing dips the book it was on.
                let dip = h.show.lands.iter().any(|d| {
                    usize::from(d.lane) == l && tick.saturating_sub(d.born) < 9 && d.at.0 >= piece && d.at.0 < piece + w
                });
                tome_at(cv, y, piece, w, (k + l) % COVERS.len(), dir, l, bob, dip, lo, hi);
            }
        }
    }
    pads(cv, h, cols, rows);
    lanterns(cv, h, cols, tick);
    owls(cv, run, h, cols, tick);
    // The candles in the side niches, and dust turning in the lantern light.
    for side in [0, cols - 1] {
        for r in 1..rows - 1 {
            let (cx, cy) = (side * TILE + 8, r * TILE + 6);
            let f = bits_flame(tick + (r * 3 + side) as u32);
            cv.stamp(f, cx - 1, cy - 5);
        }
    }
    let mut pen = Pen::new(cv, 90);
    let n = lantern_count(h);
    for k in 0..n {
        let (lx, _) = lantern_at(cols, k, n);
        let dust = Spray { gravity: -0.004, drag: 0.02, stagger: 5.0, ..Spray::round(9 + k as u32, 5, (lx as f32, 40.0), (0.1, 0.35), (60.0, 110.0)) };
        dust.each((tick % 110) as f32, |m| pfx::mote(&mut pen, &m, Shape::Dot, ramps::HOLY, 1, tick as u64));
    }
}

fn bits_flame(tick: u32) -> &'static Img {
    bits::flame(tick / 4)
}
