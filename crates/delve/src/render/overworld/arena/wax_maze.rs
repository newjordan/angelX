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
use super::pfx;
use super::juice;
use super::wax_art::{self, KingMood};
use crate::rules::folly::{Wraith, WaxMaze};

mod bits;
mod finish;
mod fx;
mod hud;
mod moments;
pub(super) mod walls;

pub(super) fn walled(run: &Run) -> bool {
    here(run).is_some_and(|m| m.crypt.2 > 0)
}

fn here(run: &Run) -> Option<&WaxMaze> {
    run.folly.as_ref().filter(|m| m.room == run.at)
}

pub(super) fn tucks_tray(run: &Run) -> bool {
    here(run).is_some_and(|m| !m.done)
}

fn middle((x, y): (i32, i32)) -> (i32, i32) {
    (x * TILE + TILE / 2, y * TILE + TILE / 2)
}

pub(super) fn floor(cv: &mut Img, run: &Run) {
    let Some(maze) = here(run) else {
        return;
    };
    let cols = run.room().cols as i32;
    let tile = |i: u16| (i as i32 % cols, i as i32 / cols);
    let tick = run.tick as u32;
    if !maze.done {
        crypt(cv, maze);
        for i in maze.wax.iter() {
            let (tx, ty) = tile(i);
            let (x, y) = middle((tx, ty));
            let glint = (tx + ty + (tick / 5) as i32) % 19 == 0;
            let drop = bits::drop(glint);
            cv.stamp(drop, x - drop.w / 2, y - drop.h / 2);
        }
        let vial = wax_art::holy(tick);
        for &i in &maze.holy {
            let (x, y) = middle(tile(i));
            let mut pen = pfx::Pen::new(cv, 400);
            let r = 9.0 + 1.5 * crate::motion::math::sin(tick as f32 * 0.14 + i as f32);
            pen.ring((x as f32, y as f32 - 1.0), r, 1.1, 1.0, pfx::ramps::FROST, 0.6);
            let bubbles = pfx::Spray { gravity: -0.012, ..pfx::Spray::round(i as u32, 3, (x as f32, y as f32 + 2.0), (0.1, 0.3), (18.0, 28.0)) };
            bubbles.each((tick % 28) as f32, |m| pfx::mote(&mut pen, &m, pfx::Shape::Bubble, pfx::ramps::WATER, 1, run.tick));
            cv.stamp(&vial, x - vial.w / 2, y - vial.h + 3);
        }
    }
    fx::under(cv, run, maze);
}

fn crypt(cv: &mut Img, maze: &WaxMaze) {
    for w in &maze.wraiths {
        let (x, y) = middle(w.home);
        let (x0, y0) = (x - 6, y - 4);
        cv.rect(x0, y0, 13, 9, 'k');
        cv.frame(x0 - 1, y0 - 1, 15, 11, 's');
        for bar in [x0 + 3, x0 + 6, x0 + 9] {
            cv.rect(bar, y0, 1, 9, 'S');
        }
        if w.rest > 0 || (w.at == w.home && !w.roaming()) {
            cv.put(x0 + 4, y0 + 4, 'C');
            cv.put(x0 + 8, y0 + 4, 'C');
        }
    }
}

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

fn ink(maze: &WaxMaze, w: &Wraith, tick: u64) -> char {
    match mood(maze, w) {
        KingMood::FleeEnding if tick / 4 % 2 == 0 => 'W',
        KingMood::Flee | KingMood::FleeEnding => 'i',
        _ => 'C',
    }
}

pub(super) fn wraiths(cv: &mut Img, run: &Run) {
    let Some(maze) = here(run) else {
        return;
    };
    let tick = run.tick as u32;
    if !maze.done {
        for (k, w) in maze.wraiths.iter().enumerate() {
            let (veil, solid) = wax_art::king(w.temper, mood(maze, w), tick);
            let (tx, ty) = w.pos();
            let (x, y) = at(tx * TILE_UNITS, ty * TILE_UNITS);
            let (x0, y0) = (x - veil.w / 2, y - veil.h / 2 - 1);
            if !w.eyes {
                let (aura, reach) = if maze.pale_now(w) { ('Q', 9) } else if w.rest > 0 { ('F', 7) } else { ('E', 9) };
                for dy in -reach..=reach {
                    for dx in -reach..=reach {
                        let d = (dx * dx + dy * dy) as f32 / (reach * reach) as f32;
                        if d < 1.0 && super::super::ink::bayer(x + dx, y + dy) < (1.0 - d) * 0.5 && cv.get(x + dx, y + dy).is_none() {
                            cv.put(x + dx, y + dy, aura);
                        }
                    }
                }
            }
            if !w.eyes {
                for dy in 0..3 {
                    for dx in -5..=5 {
                        if super::super::ink::bayer(x + dx, y + 5 + dy) < 0.55 && dx * dx + dy * dy * 9 <= 28 {
                            cv.put(x + dx, y + 5 + dy, 'k');
                        }
                    }
                }
            }
            let (dx, dy) = ((w.to.0 - w.at.0).signum(), (w.to.1 - w.at.1).signum());
            if (dx, dy) != (0, 0) && w.roaming() {
                for (j, back) in [7, 11, 15].into_iter().enumerate() {
                    let (px, py) = (x - dx * back, y - dy * back);
                    for (ox, oy) in [(0, 0), (1, 0), (0, 1)] {
                        if super::super::ink::bayer(px + ox, py + oy) < 0.7 - j as f32 * 0.22 {
                            cv.put(px + ox, py + oy, if maze.pale_now(w) { 'U' } else { 'e' });
                        }
                    }
                }
            }
            let caught = maze
                .show
                .catches
                .iter()
                .any(|c| usize::from(c.king) == k && tick.saturating_sub(c.born) < 3);
            let mut body = Img::new(veil.w, veil.h);
            for vy in 0..veil.h {
                for vx in 0..veil.w {
                    if (x0 + vx + y0 + vy) % 2 == 0
                        && let Some(c) = veil.get(vx, vy)
                    {
                        body.set(vx, vy, c);
                    }
                }
            }
            body.stamp(&solid, 0, 0);
            if caught {
                body = juice::impact_frame(&body, 'W');
            }
            cv.stamp(&body, x0, y0);
        }
    }
    fx::over(cv, run, maze, !maze.done);
}

pub(super) fn screen(over: &mut Img, run: &Run, focus: Option<u32>, origin: (i32, i32)) {
    let Some(maze) = here(run) else {
        return;
    };
    let now = run.tick as u32;
    let finish = finish::age(maze, now);
    if let Some(e) = finish.filter(|&e| e >= finish::FINALE) {
        over.stamp(&finish::scene(run, maze, e, focus), 0, 0);
        return;
    }
    let cols = run.room().cols as i32;
    moments::pale_rim(over, maze, now);
    moments::danger(over, run, maze, focus, now);
    hud::strip(over, run, maze, focus);
    hud::minimap(over, run, maze, focus, origin);
    moments::fly_ups(over, maze, focus, origin, cols, now);
    moments::last_seconds(over, maze);
    moments::callouts(over, run, maze, focus, now);
    if !maze.done {
        moments::opening(over, maze, now);
    }
    if let Some(e) = finish {
        finish::finale(over, run, maze, e, focus);
    }
}

pub(super) fn strip_bottom(run: &Run) -> Option<i32> {
    here(run).map(|_| hud::BOTTOM)
}

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
