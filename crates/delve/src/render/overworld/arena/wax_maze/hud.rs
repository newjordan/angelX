//! The maze's marks along the top and corner of a seat's view: the candle
//! and its seconds, each knight's wax with the leader crowned, the pot, one
//! bar for the wax the whole table has eaten, the holy water's bar, and a
//! small map of the maze (knights, wraiths, wax, flasks) for a room bigger
//! than the screen. The view's own HUD never shakes.

use super::super::super::chrome;
use super::super::super::ink::{Img, text_width};
use super::super::{NATIVE_H, NATIVE_W, TILE, juice, knights};
use super::bits;
use crate::rules::Run;
use crate::rules::folly::{MAZE_TIME, PALE, WaxMaze};

/// How far down the view the top marks reach.
pub(super) const BOTTOM: i32 = 16;

/// One knight's place in the strip.
pub(super) struct Chip {
    pub id: u32,
    pub x: i32,
    /// Letters of the name that fit.
    pub name: usize,
}

const LEFT: i32 = 40;
const RIGHT: i32 = 44;

/// The knights' chips, left to right in seat order, fitted to the strip.
pub(super) fn chips(maze: &WaxMaze) -> Vec<Chip> {
    let n = maze.eaten.len().max(1) as i32;
    let room = NATIVE_W - 8 - LEFT - RIGHT;
    let each = room / n;
    let name = ((each - 7 - 4 - 16 - 8) / 5).clamp(1, 6) as usize;
    maze.eaten
        .keys()
        .enumerate()
        .map(|(i, &id)| Chip { id, x: 4 + LEFT + each * i as i32, name })
        .collect()
}

/// The name a seat goes by here: `YOU` for the seat looking.
pub(super) fn called(run: &Run, id: u32, focus: Option<u32>, most: usize) -> String {
    if focus == Some(id) {
        return "YOU".to_string();
    }
    run.players
        .get(&id)
        .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
        .chars()
        .take(most)
        .collect()
}

/// Where a knight's number sits in the strip, for what flies to it.
pub(super) fn anchor(maze: &WaxMaze, id: u32) -> (i32, i32) {
    chips(maze)
        .iter()
        .find(|c| c.id == id)
        .map_or((NATIVE_W / 2, 8), |c| (c.x + 7 + c.name as i32 * 5 + 4 + 8, 8))
}

/// The strip, the team's bar and the holy water's bar.
pub(super) fn strip(over: &mut Img, run: &Run, maze: &WaxMaze, focus: Option<u32>) {
    let now = run.tick as u32;
    chrome::panel(over, (4, 1, NATIVE_W - 8, 12), chrome::Tone::SLATE);
    // The candle: burning down as the time goes.
    let left = if maze.done { 0 } else { maze.left() };
    let frac = if maze.done { 0.0 } else { (left as f32 / (MAZE_TIME / crate::rules::HZ) as f32).clamp(0.0, 1.0) };
    let body = 1 + (frac * 7.0).round() as i32;
    over.rect(9, 11 - body, 4, body, 'T');
    over.rect(12, 11 - body, 1, body, 't');
    over.put(9, 11 - body, 'H');
    if !maze.done {
        over.stamp(bits::flame(now / 5), 8, 11 - body - 5);
    }
    let low = left <= 10 && !maze.done;
    let secs = if maze.done { "DONE".to_string() } else { format!("{left}S") };
    let blink = low && now / 6 % 2 == 0;
    over.text(17, 4, &secs, if blink { '7' } else if low { 'R' } else { 'O' });
    let best = maze.eaten.values().max().copied().unwrap_or(0);
    let rivals = maze.eaten.len() > 1;
    for chip in chips(maze) {
        let wax = maze.eaten.get(&chip.id).copied().unwrap_or(0);
        let ink = knights::seat_mark(chip.id);
        let you = focus == Some(chip.id);
        over.rect(chip.x, 5, 5, 5, 'k');
        over.rect(chip.x + 1, 6, 3, 3, ink);
        let name = called(run, chip.id, focus, chip.name);
        let nx = over.text(chip.x + 7, 4, &name, if you { 'T' } else { 'i' });
        let lead = rivals && wax == best && wax > 0;
        let bumped = maze.show.pops.iter().any(|p| u32::from(p.who) == chip.id && now.saturating_sub(p.born) < 3);
        // A number flying in lands, and the knight's wax swells.
        let landed = maze
            .show
            .catches
            .iter()
            .filter(|c| u32::from(c.who) == chip.id)
            .filter_map(|c| now.saturating_sub(c.born).checked_sub(super::moments::FLY))
            .min();
        let pulse = landed.map_or(0.0, |age| juice::bump(age, 8));
        if lead {
            over.stamp(bits::crown(), nx + 2, 2);
        }
        let num = wax.to_string();
        let ink = if pulse > 0.0 || bumped { '9' } else if lead { 'T' } else { 'h' };
        over.text(nx + 5 + (3 - num.len() as i32).max(0) * 5, 4 - i32::from(pulse > 0.5), &num, ink);
    }
    // The pot, with a coin.
    let pot: u32 = maze.ante.values().sum();
    let tag = format!("{pot}G");
    let px = NATIVE_W - 9 - text_width(&tag) - 8;
    over.stamp(bits::coin(), px, 4);
    over.text(px + 7, 4, &tag, 'O');
    // The table's bar: every knight's share of the wax laid, in their ink.
    let (bx, by, bw) = (8, 13, NATIVE_W - 16);
    over.rect(bx - 1, by - 1, bw + 2, 4, 'k');
    over.rect(bx, by, bw, 2, 's');
    let total = i32::from(maze.show.total.max(1));
    let mut x = bx;
    for chip in chips(maze) {
        let w = i32::from(maze.eaten.get(&chip.id).copied().unwrap_or(0)) * bw / total;
        let w = w.min(bx + bw - x);
        over.rect(x, by, w, 2, knights::seat_mark(chip.id));
        x += w;
    }
    for q in 1..4 {
        over.put(bx + bw * q / 4, by, 'k');
    }
    // The holy water, running out: a bright line along the strip's foot.
    if maze.pale > 0 {
        let w = (bw as u32 * maze.pale / PALE) as i32;
        let flash = maze.warning() && now / 4 % 2 == 0;
        over.rect(bx, by - 2, bw, 1, 'k');
        over.rect(bx, by - 2, w, 1, if flash { 'W' } else { 'z' });
    }
}

/// The maze in small, bottom right: two pixels a tile. Only for a room
/// bigger than a screen.
pub(super) fn minimap(over: &mut Img, run: &Run, maze: &WaxMaze, focus: Option<u32>, (ox, oy): (i32, i32)) {
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    if cols * TILE <= NATIVE_W && rows * TILE <= NATIVE_H {
        return;
    }
    let (w, h) = (cols * 2, rows * 2);
    let (x0, y0) = (NATIVE_W - w - 7, NATIVE_H - h - 7);
    over.rect(x0 - 3, y0 - 3, w + 6, h + 6, 'k');
    over.frame(x0 - 3, y0 - 3, w + 6, h + 6, 's');
    let now = run.tick as u32;
    for r in 0..rows {
        for c in 0..cols {
            if room.tile(c, r) != crate::rules::Tile::Floor {
                continue;
            }
            over.rect(x0 + c * 2, y0 + r * 2, 2, 2, 'x');
            let i = (r * cols + c) as u16;
            if maze.wax.contains(&i) {
                over.put(x0 + c * 2, y0 + r * 2, 'O');
            }
        }
    }
    for &i in &maze.holy {
        let (c, r) = (i32::from(i) % cols, i32::from(i) / cols);
        over.rect(x0 + c * 2 - 1, y0 + r * 2 - 1, 4, 4, if now / 6 % 2 == 0 { 'z' } else { 'W' });
    }
    // The view's own frame, so a knight sees where they are.
    let (vx, vy) = (x0 + ox.clamp(0, cols * TILE) / 8, y0 + oy.clamp(0, rows * TILE) / 8);
    let (vw, vh) = ((NATIVE_W / 8).min(x0 + w - vx), (NATIVE_H / 8).min(y0 + h - vy));
    for dx in 0..vw {
        if dx % 2 == 0 {
            over.put(vx + dx, vy, 'h');
            over.put(vx + dx, vy + vh - 1, 'h');
        }
    }
    for dy in 0..vh {
        if dy % 2 == 0 {
            over.put(vx, vy + dy, 'h');
            over.put(vx + vw - 1, vy + dy, 'h');
        }
    }
    for wr in maze.wraiths.iter().filter(|w| w.roaming() || w.eyes) {
        let (tx, ty) = wr.pos();
        let (px, py) = (x0 + (tx * 2.0) as i32 - 1, y0 + (ty * 2.0) as i32 - 1);
        let ink = if wr.eyes { 'W' } else if maze.pale_now(wr) { 'V' } else { 'C' };
        over.rect(px - 1, py - 1, 4, 4, 'k');
        over.rect(px, py, 2, 2, ink);
    }
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && !h.stone && !h.absent) {
        let px = x0 + (hero.x / crate::rules::TILE_UNITS * 2.0) as i32 - 1;
        let py = y0 + (hero.y / crate::rules::TILE_UNITS * 2.0) as i32 - 1;
        let me = focus == Some(id);
        if me && now / 5 % 2 == 0 {
            over.rect(px - 2, py - 2, 7, 7, 'k');
        }
        over.rect(px - 1, py - 1, 4, 4, 'k');
        over.rect(px, py, 3, 3, knights::seat_mark(id));
    }
}
