//! The Atari board: the first floor drawn as the rung below the flat pixel
//! Delve (docs/WORLD-TIERS.md, "A0 the Atari board"). Experimental, off
//! unless the host sets `ANGEL_DELVE_ATARI=1`, or `ANGEL_DELVE_IDENTITY=1`,
//! whose three-room first floor it draws (`rules::identity`).
//!
//! A knight who is still only a shape begins on a black field with a few
//! flat inks: walls are plain grey bars, doorways plain gaps, the stairs a
//! marker, a dashed line down the middle of each room as on a Pong court.
//! What stands and lies on it is blocky: two room pixels to a block, one
//! flat hue a sprite, its bright details pale grey and its dark ones holes.
//! Nothing is lit, nothing dithered, and a window does not tip the floor
//! (`RoomScene::board`). The rules are not touched: walls, doors, stairs,
//! foes and their timings are where and what they were.

use super::*;

/// The board's inks, every one from the realm's palette (`ink::PALETTE`),
/// so every view that keeps to the palette keeps them.
/// The field.
pub const FIELD: Rgb = BLACK;
/// Walls: a mid grey, never a glare.
pub const BAR: Rgb = rgb(0x63655c);
/// Pillars and blocks, the centre line, what stands about the room.
pub const GREY: Rgb = rgb(0x7a7c72);
/// Pong's white, toned: a sprite's bright details, the markers.
pub const PONG: Rgb = rgb(0xb0b2a8);
/// The first hue: what fights the knights.
pub const FOE: Rgb = rgb(0x924f1d);
/// The first hue dimmed: what hurts underfoot (pits, water, traps).
pub const HURT: Rgb = rgb(0x512d18);
/// The second hue: what a knight can take (loot, the chest).
pub const GOLD: Rgb = rgb(0xd8a95e);

/// The inks a board layer is held to (`board`), black first.
const INKS: [Rgb; 7] = [FIELD, BAR, GREY, PONG, FOE, HURT, GOLD];

// ------------------------------------------------------------ the switch --

#[cfg(not(test))]
static SET: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

// Tests keep the floor's look unless a test turns the board on, on its own
// thread.
#[cfg(test)]
std::thread_local! {
    static SET: std::cell::Cell<u8> = const { std::cell::Cell::new(2) };
}

/// Turn the board on or off for this process (a page, from its host's
/// welcome: `wire::Params::atari`); in a test, for its thread.
pub fn set(on: bool) {
    let code = if on { 1 } else { 2 };
    #[cfg(not(test))]
    SET.store(code, std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    SET.with(|c| c.set(code));
}

/// Whether the first floor is the Atari board: what a page's host said,
/// else `ANGEL_DELVE_ATARI=1` (experimental, off by default). Tests: off
/// unless `set`.
pub fn on() -> bool {
    #[cfg(not(test))]
    let code = SET.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    let code = SET.with(|c| c.get());
    match code {
        1 => true,
        2 => false,
        _ => {
            // The identity start's first floor is the board: one switch
            // turns on the experiment (`rules::identity::on`).
            let on = std::env::var_os("ANGEL_DELVE_ATARI").is_some_and(|v| v == "1") || crate::rules::identity::on();
            set(on);
            on
        }
    }
}

/// The floor that is the board.
pub const FLOOR: u32 = 1;

/// Whether `run`'s room is drawn as the board: the switch on, the first
/// floor, a room of the dungeon's own (not a side-on hall, the Wanted
/// hall, a ride, or a floor lit from below).
pub fn here(run: &Run) -> bool {
    run.dungeon.depth == FLOOR
        && on()
        && !run.side_on()
        && !run.race3d_showing()
        && run.circle() == 0
}

// ------------------------------------------------------------ the field --

/// The room as the board: the field, its walls as bars, its doors as gaps,
/// its stairs a marker, the centre line, its fixtures as blocks. No light.
pub(super) fn playfield(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = room_px(room);
    let mut cv = Img::black(pw, ph);
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let open = |c: i32, r: i32| room.tile(c, r) != Tile::Wall;
    for row in 0..rows {
        for col in 0..cols {
            let (x, y) = (col * TILE, row * TILE);
            match room.tile(col, row) {
                // A wall shows as a bar where it meets the room; rock deeper
                // in is the field.
                Tile::Wall => {
                    let edge = (-1..=1).any(|dr| (-1..=1).any(|dc| open(col + dc, row + dr)));
                    // The head wall over stairs set against it is the board's
                    // gap, the way on (the streamlined first floor's).
                    let gap = row == 0 && room.tile(col, 1) == Tile::Stairs;
                    if edge && !gap {
                        fill(&mut cv, x, y, TILE, TILE, BAR);
                    }
                }
                Tile::Block => fill(&mut cv, x + 2, y + 2, TILE - 4, TILE - 4, GREY),
                Tile::Hazard => fill(&mut cv, x, y, TILE, TILE, HURT),
                Tile::Bog => fill(&mut cv, x + 2, y + 2, TILE - 4, TILE - 4, HURT),
                Tile::Ledge => fill(&mut cv, x, y, TILE, 4, GREY),
                Tile::Floor | Tile::Door | Tile::Stairs => {}
            }
        }
    }
    centre_line(room, &mut cv);
    // The fixtures the room's own drawers make (the stairs, a shut door's
    // bars, an altar, plates and grilles, a cracked wall), as grey blocks.
    let barred = run.barred() || run.open_guarded();
    let mut fixtures = Img::new(pw, ph);
    for (side, &(col, row)) in doors(room).iter().enumerate() {
        if room.doors[side] && barred {
            // Shut while the fight lasts: a bar across the gap.
            let (x, y) = (col * TILE, row * TILE);
            if side % 2 == 0 {
                fill(&mut fixtures, x, y + TILE / 2 - 2, 2 * TILE, 4, FOE);
            } else {
                fill(&mut fixtures, x + TILE / 2 - 2, y, 4, 2 * TILE, FOE);
            }
        }
    }
    for row in 0..rows {
        for col in 0..cols {
            if room.tile(col, row) == Tile::Stairs {
                stairs(&mut fixtures, col * TILE, row * TILE, barred);
            }
        }
    }
    let mut details = Img::new(pw, ph);
    // The streamlined first floor's room is the bare board (`identity::simple`).
    if !run.simple_creation() {
        room_details(run, &mut details, &mut Vec::new());
    }
    let details = sprite_in(&details, GREY, (0, 0));
    cv.stamp(&details, 0, 0);
    // A spike plate is floor that hurts, not a block: its tile outlined in
    // the dim hue.
    for trap in &run.traps {
        if let Trap::Spikes { col, row, .. } = *trap {
            // Its plate is two tiles a side (`sprites::spike_plate`).
            let (x, y, n) = (col * TILE, row * TILE, 2 * TILE);
            fill(&mut cv, x, y, n, n, FIELD);
            for (fx, fy, fw, fh) in [(x, y, n, 2), (x, y + n - 2, n, 2), (x, y, 2, n), (x + n - 2, y, 2, n)] {
                fill(&mut cv, fx, fy, fw, fh, HURT);
            }
        }
    }
    cv.stamp(&fixtures, 0, 0);
    // The chest, in the hue of what can be taken: here on every page (no
    // page's shapes are asked for on the board).
    if let Some(c) = room.chest {
        let im = sprite(&chest(!c.open), GOLD);
        let (x, y) = at(c.x, c.y);
        cv.stamp(&im, x - im.w / 2, y - im.h / 2);
    }
    cv
}

/// The stairs down: treads across the tile in pale grey, dim while the
/// floor's guardian still holds them.
fn stairs(cv: &mut Img, x: i32, y: i32, shut: bool) {
    let ink = if shut { GREY } else { PONG };
    for k in 0..3 {
        fill(cv, x, y + 2 + 5 * k, TILE, 2, ink);
    }
}

/// Pong's net: a dashed line down the room's middle, on its floor only.
/// An open floor's map (bigger than any hall) has none.
fn centre_line(room: &Room, cv: &mut Img) {
    if room.cols > crate::rules::layout::MAX_COLS || room.rows > crate::rules::layout::MAX_ROWS {
        return;
    }
    let x = (room.cols as i32 * TILE / 2 - 1) & !1;
    for y in (0..room.rows as i32 * TILE).step_by(2) {
        let floor = |x: i32| room.tile(x / TILE, y / TILE) == Tile::Floor;
        if (y / 6) % 2 == 0 && floor(x) && floor(x + 1) {
            fill(cv, x, y, 2, 2, GREY);
        }
    }
}

/// Paint a `w` x `h` square of `c` at `(x, y)`.
fn fill(cv: &mut Img, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
    for yy in y..y + h {
        for xx in x..x + w {
            cv.set(xx, yy, c);
        }
    }
}

// ------------------------------------------------------------ the sprites --

fn lum(c: Rgb) -> u32 {
    (299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2])) / 1000
}

/// A sprite as an Atari one, the same size: two pixels to a block, each
/// block taken where half of it is drawn; one flat `body` ink, its signal
/// inks (eyes, a mark) pale grey, its darkest inner blocks holes.
pub fn sprite(im: &Img, body: Rgb) -> Img {
    sprite_in(im, body, (0, 0))
}

/// `sprite` with its blocks on the grid of `origin` (a layer of the room's
/// pixels keeps the room's blocks).
pub(super) fn sprite_in(im: &Img, body: Rgb, origin: (i32, i32)) -> Img {
    let (sx, sy, sw, sh) = im.span();
    let mut out = Img::window(im.w, im.h, (sx, sy, sw, sh));
    let (x0, y0) = (sx - (sx - origin.0).rem_euclid(2), sy - (sy - origin.1).rem_euclid(2));
    // Each block: drawn, its brightness, signal.
    let bw = (sx + sw - x0 + 1) / 2;
    let bh = (sy + sh - y0 + 1) / 2;
    let mut blocks: Vec<Option<(u32, bool)>> = vec![None; (bw * bh).max(0) as usize];
    let mut lums = Vec::new();
    for by in 0..bh {
        for bx in 0..bw {
            let (mut n, mut l, mut signal) = (0, 0, false);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if let Some(c) = im.get(x0 + 2 * bx + dx, y0 + 2 * by + dy) {
                    n += 1;
                    l += lum(c);
                    signal |= is_signal(c);
                }
            }
            if n >= 2 || (n == 1 && signal) {
                let l = l / n;
                blocks[(by * bw + bx) as usize] = Some((l, signal));
                lums.push(l);
            }
        }
    }
    if lums.is_empty() {
        return out;
    }
    lums.sort_unstable();
    let middle = lums[lums.len() / 2];
    let drawn = |bx: i32, by: i32| bx >= 0 && by >= 0 && bx < bw && by < bh && blocks[(by * bw + bx) as usize].is_some();
    for by in 0..bh {
        for bx in 0..bw {
            let Some((l, signal)) = blocks[(by * bw + bx) as usize] else { continue };
            let inner = drawn(bx - 1, by) && drawn(bx + 1, by) && drawn(bx, by - 1) && drawn(bx, by + 1);
            let ink = if signal {
                PONG
            } else if inner && l < 48 && l * 2 < middle {
                FIELD
            } else {
                body
            };
            fill(&mut out, x0 + 2 * bx, y0 + 2 * by, 2, 2, ink);
        }
    }
    out
}

/// A room's prop or creature drawn by `draw` (a frog, a hireling, the hand
/// from the floor): on the board one blocky sprite in `body`, off it as
/// itself.
pub(super) fn held_to(cv: &mut Img, run: &Run, body: Rgb, draw: impl FnOnce(&mut Img)) {
    if !here(run) {
        return draw(cv);
    }
    let mut layer = Img::window(cv.w, cv.h, cv.span());
    draw(&mut layer);
    cv.stamp(&sprite_in(&layer, body, (0, 0)), 0, 0);
}

/// What lies on the room's floor (its folk, pots, hazards' marks, rings),
/// held to the board in place: two room pixels to a block, each block the
/// board's ink nearest its commonest colour.
pub(super) fn board(cv: &mut Img) {
    let (sx, sy, sw, sh) = cv.span();
    let (x0, y0) = (sx & !1, sy & !1);
    let near = |c: Rgb| {
        let d = |a: Rgb| (0..3).map(|i| (i32::from(c[i]) - i32::from(a[i])).pow(2)).sum::<i32>();
        *INKS.iter().min_by_key(|&&a| d(a)).unwrap_or(&FIELD)
    };
    let mut y = y0;
    while y < sy + sh {
        let mut x = x0;
        while x < sx + sw {
            let cells = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
            let mut seen: [(Rgb, u8); 4] = [(BLACK, 0); 4];
            let mut any = false;
            for &(px, py) in &cells {
                if let Some(c) = cv.get(px, py) {
                    any = true;
                    if let Some(s) = seen.iter_mut().find(|s| s.1 > 0 && s.0 == c) {
                        s.1 += 1;
                    } else if let Some(s) = seen.iter_mut().find(|s| s.1 == 0) {
                        *s = (c, 1);
                    }
                }
            }
            if any {
                let most = seen.iter().filter(|s| s.1 > 0).max_by_key(|s| (s.1, lum(s.0))).map_or(BLACK, |s| s.0);
                let ink = if is_signal(most) && lum(most) > 120 { PONG } else { near(most) };
                for &(px, py) in &cells {
                    cv.set(px, py, ink);
                }
            }
            x += 2;
        }
        y += 2;
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__atari_tests.rs"]
mod tests;
