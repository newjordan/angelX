//! The Wax Maze as a seat sees it (`rules::folly`): the wax on the lanes,
//! holy water in the corners and the wraiths' crypt on the floor; the
//! wraiths, the art team's little green kings (`wax_art`), over the
//! knights; the candle and each knight's wax in a strip
//! along the top of the view, over the north wall, so no lane is ever
//! under a board; the title card; and arrows at the view's edge to the
//! wraiths out of sight in a maze bigger than a screen. While the maze is
//! played the skills bar is tucked away (`tucks_tray`): its lanes run along
//! the room's foot.

use super::*;
use super::super::ink::text_width;
use super::wax_art::{self, KingMood};
use crate::rules::folly::{Wraith, WaxMaze};

/// The maze in the room the party stands in.
fn here(run: &Run) -> Option<&WaxMaze> {
    run.folly.as_ref().filter(|m| m.room == run.at)
}

/// Whether the skills bar keeps out of the view: the maze is being
/// played in this room (its title card included).
pub(super) fn tucks_tray(run: &Run) -> bool {
    here(run).is_some_and(|m| !m.done)
}

/// A tile's middle, in the room's pixels.
fn middle((x, y): (i32, i32)) -> (i32, i32) {
    (x * TILE + TILE / 2, y * TILE + TILE / 2)
}

/// The maze's floor, in the room's pixels: the crypt's lid, the wax and
/// the holy water (the art team's drops and vials, `wax_art`). Once the
/// podium is called it is just a crypt again.
pub(super) fn floor(cv: &mut Img, run: &Run) {
    let Some(maze) = here(run).filter(|m| !m.done) else {
        return;
    };
    let cols = run.room().cols as i32;
    let tile = |i: u16| (i as i32 % cols, i as i32 / cols);
    let tick = run.tick as u32;
    // The crypt: a dark slab with a worn rim, where the wraiths wait.
    let (cx, cy, cw, ch) = maze.crypt;
    if cw > 0 && ch > 0 {
        let (x0, y0, w, h) = (cx * TILE + 1, cy * TILE + 1, cw * TILE - 2, ch * TILE - 2);
        cv.rect(x0, y0, w, h, 'x');
        cv.frame(x0, y0, w, h, 'S');
        cv.frame(x0 + 2, y0 + 2, w - 4, h - 4, 's');
    }
    let drop = wax_art::wax(tick);
    for i in maze.wax.iter() {
        let (x, y) = middle(tile(i));
        cv.stamp(&drop, x - drop.w / 2, y - drop.h / 2);
    }
    let vial = wax_art::holy(tick);
    for &i in &maze.holy {
        let (x, y) = middle(tile(i));
        cv.stamp(&vial, x - vial.w / 2, y - vial.h + 3);
    }
}

/// How a wraith looks now, as the art team drew its moods: hunting, pale
/// and fleeing, flashing as the holy water wears off, and at rest in the
/// crypt (or a caught one's crown and eyes racing home), its robe gone.
fn mood(maze: &WaxMaze, w: &Wraith) -> KingMood {
    if w.eyes || w.rest > 0 {
        KingMood::Rest
    } else if maze.pale_now(w) && maze.warning() {
        KingMood::FleeEnding
    } else if maze.pale_now(w) {
        KingMood::Flee
    } else {
        KingMood::Hunt
    }
}

/// A wraith's ink for an arrow at the view's edge: its robe's green when
/// hunting, grey when pale, white as the pale wears off.
fn ink(maze: &WaxMaze, w: &Wraith, tick: u64) -> char {
    match mood(maze, w) {
        KingMood::FleeEnding if tick / 4 % 2 == 0 => 'W',
        KingMood::Flee | KingMood::FleeEnding => 'i',
        _ => 'C',
    }
}

/// The wraiths, in the room's pixels, over the knights: the art team's
/// little green kings (`wax_art::king`), their robes a veil the room shows
/// through and their crowns and eyes solid. The veil is laid as a
/// half-tone, every other pixel on the room's own grid, so it holds still
/// on the floor as the king moves and every pixel stays on the palette.
pub(super) fn wraiths(cv: &mut Img, run: &Run) {
    let Some(maze) = here(run).filter(|m| !m.done) else {
        return;
    };
    let tick = run.tick as u32;
    for w in &maze.wraiths {
        let (veil, solid) = wax_art::king(w.temper, mood(maze, w), tick);
        let (tx, ty) = w.pos();
        let (x, y) = at(tx * TILE_UNITS, ty * TILE_UNITS);
        let (x0, y0) = (x - veil.w / 2, y - veil.h / 2 - 1);
        for vy in 0..veil.h {
            for vx in 0..veil.w {
                let (px, py) = (x0 + vx, y0 + vy);
                if (px + py) % 2 == 0
                    && let Some(c) = veil.get(vx, vy)
                {
                    cv.set(px, py, c);
                }
            }
        }
        cv.stamp(&solid, x0, y0);
    }
}

/// The view's marks: the candle and each knight's wax in a strip along the
/// top, and the title card.
pub(super) fn screen(over: &mut Img, run: &Run) {
    let Some(maze) = here(run) else {
        return;
    };
    let candle = if maze.done { "DONE".to_string() } else { format!("{}S", maze.left()) };
    let best = maze.eaten.values().max().copied().unwrap_or(0);
    let rivals = maze.eaten.len() > 1;
    let names: Vec<(char, String, String, bool)> = maze
        .eaten
        .iter()
        .map(|(&id, &n)| {
            let name: String = run
                .players
                .get(&id)
                .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
                .chars()
                .take(6)
                .collect();
            (super::party::seat_ink(id), name, n.to_string(), rivals && n == best && n > 0)
        })
        .collect();
    let gap = 9;
    // As `Img::text` advances: a glyph and its space.
    let adv = |s: &str| text_width(s) + 1;
    let entry = |(_, name, count, _): &(char, String, String, bool)| 7 + adv(name) + 4 + adv(count);
    let w = 6 + adv("WAX") + 4 + adv(&candle) + names.iter().map(|e| gap + entry(e)).sum::<i32>() + 5;
    let x0 = (NATIVE_W - w) / 2;
    chrome::panel(over, (x0, 2, w, 12), chrome::Tone::SLATE);
    let mut x = x0 + 6;
    x = over.text(x, 5, "WAX", 'h') + 4;
    x = over.text(x, 5, &candle, if maze.left() <= 10 && !maze.done { '7' } else { 'O' });
    for (ink, name, count, leads) in &names {
        x += gap;
        over.rect(x, 6, 5, 5, *ink);
        x = over.text(x + 7, 5, name, 'O') + 4;
        x = over.text(x, 5, count, if *leads { 'O' } else { 'i' });
    }
    if maze.titled() {
        folly_title(over, "THE WAX MAZE", "EAT WAX. FLEE WRAITHS.");
    }
}

/// How far down the view the strip reaches, for boards that settle under
/// it.
pub(super) fn strip_bottom(run: &Run) -> Option<i32> {
    here(run).map(|_| 14)
}

/// The wraiths for `beyond`'s arrows: where each one abroad is, in the
/// room's pixels, and its ink.
pub(super) fn beyond(run: &Run) -> Vec<((i32, i32), char)> {
    let Some(maze) = here(run).filter(|m| !m.done) else {
        return Vec::new();
    };
    maze.wraiths
        .iter()
        .filter(|w| w.roaming())
        .map(|w| {
            let (tx, ty) = w.pos();
            (at(tx * TILE_UNITS, ty * TILE_UNITS), ink(maze, w, run.tick))
        })
        .collect()
}
