//! The Colosseum inside: its entrance hall (an arcade of slate stone, two
//! open gates with their banners, a board before each), the Powder Keg yard
//! and the Gander Joust hall, each framed by stands of the realm's folk.
//! The same slate and sand as the Colosseum on the kingdom's map; dark,
//! tonal, the gates and the games the brightest things in view.

use super::super::ink::{Img, hash};
use super::super::material as m;
use super::{TILE, home, joust};
use crate::rules::{
    RoomKind, Run, colosseum as c,
    pastimes::{self, Game},
};

/// The east stand's steps in the Powder Keg yard: how many, and how wide.
const TIERS: i32 = 3;
const TIER: i32 = 25;

/// The Gander Joust hall's gallery rail, its pixel row.
const RAIL: i32 = 21;

/// Slate, darkest to lightest: the Colosseum's stone.
const STONE: &[char] = &['k', 's', 'x', 'S', 'u', 'U'];

/// Each gate's banner: its cloth, its lit edge, and its emblem's ink.
fn banner_inks(game: Game) -> (char, char, char) {
    match game {
        Game::Gander => ('E', 'l', 'h'),
        _ => ('p', 'R', 'K'),
    }
}

pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let mut cv = Img::black(room.cols as i32 * TILE, room.rows as i32 * TILE);
    let t = run.tick as u32;
    match room.kind {
        RoomKind::KegYard => yard(&mut cv),
        RoomKind::GanderHall => gander_hall(&mut cv),
        _ => hall(&mut cv, t),
    }
    cv
}

/// Big worn flagstones, warm and dark under the slate walls: rows of
/// stones of uneven length, each its own shade, joints darker still.
fn flags(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), seed: u32) {
    const ROW: i32 = 14;
    for y in y0..y0 + h {
        let row = (y - y0) / ROW;
        let ly = (y - y0) % ROW;
        // Each row's joints, from its own lengths.
        let mut x = x0 - (hash(row, 0, seed) % 20) as i32;
        let mut k = 0;
        while x < x0 + w {
            let len = 18 + (hash(row, k, seed) % 14) as i32;
            let tone = hash(row, k, seed + 3) % 6;
            let face = if tone == 0 { 'g' } else { 'X' };
            for xx in x.max(x0)..(x + len).min(x0 + w) {
                let lx = xx - x;
                let ink = if ly == 0 || lx == 0 {
                    'K'
                } else if hash(xx, y, seed + 9) % 31 == 0 {
                    'K'
                } else {
                    face
                };
                cv.put(xx, y, ink);
            }
            x += len;
            k += 1;
        }
    }
}

/// Raked sand, the arena floor: dark timber tones, never bright.
fn sand(cv: &mut Img, (x0, y0, w, h): (i32, i32, i32, i32), seed: u32) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let v = hash(x, y, seed) % 23;
            let ink = match v {
                0 => 'P',
                1..=3 => 'b',
                4 => 'n',
                _ => 'I',
            };
            cv.put(x, y, ink);
        }
    }
}

/// A run of coursed slate, its top edge lit.
fn wall(cv: &mut Img, rect: (i32, i32, i32, i32), seed: u32) {
    m::ashlar(cv, rect, STONE, seed);
}

/// An open gate: a round-headed mouth in the wall, its voussoirs lit, the
/// portcullis drawn up into the arch, and the light of the arena beyond.
fn gate(cv: &mut Img, x: i32, top: i32, w: i32, foot: i32, beyond: char) {
    let r = w / 2;
    let cx = x + r;
    let spring = top + r;
    // Voussoirs: a ring a little wider than the mouth.
    for y in top - 4..foot {
        for xx in x - 4..x + w + 4 {
            let dy = (y - spring).min(0) as f32;
            let d = ((xx - cx) as f32 + 0.5).hypot(dy);
            if d <= r as f32 + 4.0 && d > r as f32 {
                let lit = xx < cx;
                let joint = ((d as i32) + y) % 5 == 0;
                cv.put(xx, y, if joint { 'x' } else if lit { 'U' } else { 'u' });
            }
        }
    }
    // The mouth: dark, the far end lit by the arena, a strip of its
    // floor and the stands' foot seen through it.
    for y in top..foot {
        for xx in x..x + w {
            let dy = (y - spring).min(0) as f32;
            if ((xx - cx) as f32 + 0.5).hypot(dy) <= r as f32 {
                let depth = foot - y;
                let span = foot - top;
                let ink = if depth < span / 3 {
                    if (xx + y) % 3 == 0 { 'k' } else { beyond }
                } else if depth < span / 2 && (xx + y) % 2 == 0 {
                    beyond
                } else if depth < span * 2 / 3 && (xx * 3 + y) % 5 == 0 {
                    'X'
                } else {
                    'k'
                };
                cv.put(xx, y, ink);
            }
        }
    }
    // The portcullis, raised: its teeth just below the crown.
    for xx in (x + 2..x + w - 1).step_by(4) {
        cv.line(xx, top + 2, xx, top + 8, 'g');
        cv.put(xx, top + 9, 'j');
    }
    cv.line(x + 3, top + 4, x + w - 4, top + 4, 'g');
    // A keystone.
    cv.rect(cx - 2, top - 5, 5, 5, 'U');
    cv.line(cx - 2, top - 5, cx + 2, top - 5, 'v');
}

/// A blind arch: a recess in the arcade.
fn recess(cv: &mut Img, cx: i32, top: i32, w: i32, foot: i32) {
    let r = w / 2;
    let spring = top + r;
    for y in top - 2..foot {
        for x in cx - r - 2..cx + r + 2 {
            let dy = (y - spring).min(0) as f32;
            let d = ((x - cx) as f32 + 0.5).hypot(dy);
            if d <= r as f32 {
                cv.put(x, y, if y > foot - 3 { 's' } else { 'K' });
            } else if d <= r as f32 + 2.0 {
                cv.put(x, y, if x < cx { 'u' } else { 'S' });
            }
        }
    }
}

/// A hanging banner on its pole, swallow-tailed, its game's emblem on it.
fn banner(cv: &mut Img, x: i32, y: i32, game: Game, t: u32) {
    let (cloth, lit, mark) = banner_inks(game);
    let (w, h) = (14, 26);
    cv.line(x - 2, y, x + w + 1, y, 'B');
    cv.put(x - 3, y, 'o');
    cv.put(x + w + 2, y, 'o');
    let sway = i32::from(t / 20 % 2 == 0);
    for yy in 1..h {
        // The swallow-tail: a notch cut up into its foot.
        let notch = yy - (h - 5);
        for xx in 0..w {
            if notch > 0 && (xx - w / 2).abs() < notch {
                continue;
            }
            let edge = xx == 0;
            let ink = if edge { lit } else if xx == w - 1 { 'n' } else { cloth };
            cv.put(x + xx + if yy > h / 2 { sway } else { 0 }, y + yy, ink);
        }
    }
    // Gold trim along its head.
    cv.line(x, y + 1, x + w - 1, y + 1, 'o');
    let (ex, ey) = (x + 3, y + 6);
    match game {
        Game::Gander => {
            // A goose's head and neck, bill out.
            cv.rect(ex + 2, ey + 6, 6, 4, mark);
            cv.line(ex + 6, ey + 5, ex + 6, ey + 1, mark);
            cv.rect(ex + 5, ey, 3, 2, mark);
            cv.line(ex + 8, ey + 1, ex + 9, ey + 1, 'o');
            cv.put(ex + 6, ey, 'k');
        }
        _ => {
            // A keg on its side, a fuse and its spark.
            cv.rect(ex + 1, ey + 4, 7, 6, mark);
            cv.line(ex + 1, ey + 6, ex + 7, ey + 6, 'g');
            cv.line(ex + 1, ey + 8, ex + 7, ey + 8, 'g');
            cv.line(ex + 5, ey + 3, ex + 7, ey + 1, 'g');
            cv.put(ex + 8, ey, 'T');
        }
    }
}

/// The upper tiers seen over a wall: rows of the realm's folk, dim.
fn tiers(cv: &mut Img, (x0, y0, w): (i32, i32, i32), rows: i32, seed: u32) {
    for row in 0..rows {
        let y = y0 + row * 5;
        cv.rect(x0, y + 4, w, 1, 'x');
        let mut x = x0 + 2 + row * 3;
        let mut k = 0;
        while x < x0 + w - 3 {
            let h = hash(k, row, seed);
            if h % 7 != 0 {
                let head = ['g', 'X', 'Z', 'b'][(h % 4) as usize];
                let body = ['s', 'S', 'X', 'I', 'b'][((h >> 3) % 5) as usize];
                cv.rect(x, y + 1, 2, 2, head);
                cv.rect(x - 1, y + 3, 4, 1, body);
            }
            x += 5 + (h % 2) as i32;
            k += 1;
        }
    }
}

/// The entrance hall: the arcade along the north wall with the two gates,
/// the great door to the south, a board on posts before each gate.
fn hall(cv: &mut Img, t: u32) {
    let (w, h) = (cv.w, cv.h);
    flags(cv, (0, 0, w, h), 61);
    // A worn runner from the great door up the middle of the hall.
    let (rx, rw) = (11 * TILE + 3, 2 * TILE - 6);
    cv.rect(rx, 4 * TILE, rw, 8 * TILE, 'b');
    cv.line(rx, 4 * TILE, rx, 12 * TILE - 1, 'I');
    cv.line(rx + rw - 1, 4 * TILE, rx + rw - 1, 12 * TILE - 1, 'n');
    for y in (4 * TILE + 8..12 * TILE).step_by(16) {
        let cx = rx + rw / 2;
        for d in 0..4 {
            cv.put(cx - d, y + d, 'B');
            cv.put(cx + d, y + d, 'B');
            cv.put(cx - d, y + 7 - d, 'B');
            cv.put(cx + d, y + 7 - d, 'B');
        }
    }
    // The stands beyond the north wall, then the arcade's wall.
    cv.rect(0, 0, w, 3 * TILE, 'k');
    tiers(cv, (TILE, 0, w - 2 * TILE), 2, 7);
    cv.rect(TILE, 10, w - 2 * TILE, 2, 'u');
    wall(cv, (TILE, 12, w - 2 * TILE, 3 * TILE - 12), 31);
    // Niches between the gates, each with a dim statue of a champion.
    for cx in [9 * TILE, 12 * TILE, 15 * TILE] {
        recess(cv, cx, 22, 14, 3 * TILE - 2);
        cv.rect(cx - 2, 26, 4, 4, 'g');
        cv.rect(cx - 3, 30, 6, 10, 'X');
        cv.line(cx - 3, 30, cx - 3, 39, 'g');
        cv.rect(cx - 4, 40, 8, 4, 'g');
    }
    for (i, game) in c::GAMES.into_iter().enumerate() {
        let (gx, gy, gw, _) = c::GATES[i];
        let beyond = if game == Game::Gander { 'X' } else { 'P' };
        let (mx, mw) = (gx * TILE - 8, gw * TILE + 16);
        gate(cv, mx, 10, mw, (gy + 1) * TILE, beyond);
        banner(cv, mx - 20, 14, game, t);
        banner(cv, mx + mw + 6, 14, game, t + 7);
    }
    // Side walls and the south wall, with the great door.
    for x in [0, w - TILE] {
        wall(cv, (x, 0, TILE, h), 33);
        let edge = if x == 0 { TILE - 1 } else { x };
        cv.line(edge, 3 * TILE, edge, h - 1, 'k');
    }
    wall(cv, (0, h - TILE, w, TILE), 35);
    let (dx, dy, dw, _) = c::GREAT_DOOR;
    let (px, py) = (dx * TILE, dy * TILE);
    gate(cv, px - 4, py + 2, dw * TILE + 8, h, 's');
    // The boards' posts and their shadows (the boards are lettered over).
    for &(bx, by, bw, _) in &c::BOARDS {
        for x in [bx * TILE + 10, (bx + bw) * TILE - 12] {
            cv.ellipse(x as f32 + 1.0, (by * TILE + 15) as f32, 5.0, 2.0, 'K');
            cv.rect(x - 1, by * TILE - 6, 3, 21, 'I');
            cv.line(x - 1, by * TILE - 6, x - 1, by * TILE + 14, 'P');
        }
    }
    // Torches on the arcade beside the niches.
    for x in [10 * TILE + 6, 13 * TILE + 6] {
        cv.stamp(&super::super::kit::torch(t / 4, x as u32), x, 26);
    }
}

/// The Powder Keg yard: the courtyard (its flags, posts and bales) in the
/// middle, a low parapet round it, the stands north, east and south, the
/// knights' bench west with the start mark, and the gate back to the hall.
fn yard(cv: &mut Img) {
    let (w, h) = (cv.w, cv.h);
    sand(cv, (0, 0, w, h), 71);
    let x0 = i32::from(pastimes::POWDER_X0) * TILE;
    let y0 = i32::from(pastimes::POWDER_Y0) * TILE;
    let (yw, yh) = (i32::from(pastimes::POWDER_W) * TILE, i32::from(pastimes::POWDER_H) * TILE);
    // The courtyard at rest: flagstones, posts and every bale standing.
    for r in 0..pastimes::POWDER_H {
        for c in 0..pastimes::POWDER_W {
            let (x, y) = (x0 + i32::from(c) * TILE, y0 + i32::from(r) * TILE);
            let slab = ((u32::from(c) * 7 + u32::from(r) * 13) % 3) as i32;
            cv.rect(x, y, 16, 16, if slab == 0 { 'G' } else { 'j' });
            cv.line(x, y + 15, x + 15, y + 15, 'g');
            cv.line(x + 15, y, x + 15, y + 15, 'g');
        }
    }
    for &(c, r) in pastimes::POWDER_POSTS.iter() {
        let (x, y) = (x0 + i32::from(c) * TILE, y0 + i32::from(r) * TILE);
        cv.rect(x + 4, y + 2, 8, 12, 'S');
        cv.rect(x + 3, y, 10, 4, 'u');
        cv.line(x + 3, y + 13, x + 12, y + 13, 'k');
    }
    for (i, &(c, r)) in pastimes::POWDER_BALES.iter().enumerate() {
        let (x, y) = (x0 + i32::from(c) * TILE, y0 + i32::from(r) * TILE);
        m::hay(cv, x + 8, y + 12, 9.5, 300 + i as u32);
    }
    // The parapet: a low slate curb round the courtyard.
    cv.frame(x0 - 3, y0 - 3, yw + 6, yh + 6, 'u');
    cv.frame(x0 - 2, y0 - 2, yw + 4, yh + 4, 'S');
    cv.frame(x0 - 1, y0 - 1, yw + 2, yh + 2, 'k');
    // The stands: north over the yard, east in tiers, south a low wall.
    cv.rect(0, 0, w, 2 * TILE, 'k');
    tiers(cv, (TILE, 1, w - 2 * TILE), 2, 17);
    wall(cv, (TILE, 11, w - 2 * TILE, 2 * TILE - 11), 73);
    let (sx, sy, sw, sh) = c::YARD_STANDS[1];
    let (x0s, y0s, ws, hs) = (sx * TILE, sy * TILE, sw * TILE, sh * TILE);
    cv.rect(x0s, y0s, ws, hs, 'K');
    for k in 0..TIERS {
        let x = x0s + 4 + k * TIER;
        // Each step a slab of slate, its lit lip to the west.
        cv.rect(x, y0s, TIER - 4, hs, 'x');
        cv.line(x, y0s, x, y0s + hs - 1, 'u');
        cv.line(x + 1, y0s, x + 1, y0s + hs - 1, 'S');
        cv.line(x + TIER - 5, y0s, x + TIER - 5, y0s + hs - 1, 's');
    }
    let (sx, sy, sw, _) = c::YARD_STANDS[2];
    wall(cv, (sx * TILE, sy * TILE, sw * TILE, TILE), 77);
    // Outer walls.
    wall(cv, (0, 0, TILE, h), 79);
    wall(cv, (w - TILE, 0, TILE, h), 81);
    wall(cv, (0, h - TILE, w, TILE), 83);
    cv.line(TILE - 1, 2 * TILE, TILE - 1, h - TILE, 'k');
    // The knights' bench along the west wall, and a rack of spare kegs.
    m::boards(cv, (TILE + 2, 7 * TILE, 10, 3 * TILE), true, 85);
    cv.line(TILE + 2, 7 * TILE, TILE + 2, 10 * TILE - 1, 'o');
    for k in 0..3 {
        let (x, y) = (3 * TILE + 2 + k * 9, 2 * TILE + 4);
        cv.rect(x, y, 8, 9, 'K');
        cv.line(x, y + 3, x + 7, y + 3, 'g');
        cv.line(x, y + 6, x + 7, y + 6, 'g');
        cv.put(x + 4, y - 1, 'g');
    }
    // A stone dais under the start mark.
    let (mx, my) = Game::Powder.start();
    let (mx, my) = ((mx * TILE as f32) as i32, (my * TILE as f32) as i32);
    cv.ellipse(mx as f32, my as f32 + 2.0, 15.0, 9.0, 'k');
    cv.ellipse(mx as f32, my as f32, 14.0, 8.0, 'S');
    cv.ellipse(mx as f32, my as f32, 12.0, 6.5, 'x');
    // The gate back to the hall, in the south wall below the bench.
    let (gx, gy, gw, _) = c::YARD_GATE;
    gate(cv, gx * TILE - 2, gy * TILE + 2, gw * TILE + 4, h, 's');
}

/// The Gander Joust hall, side-on: a back wall of slate arches with the
/// folk in the gallery above, pillars at either end, and the gate low in
/// the west wall. The ledges, the tar and the riders are the game's own
/// (`pastimes::gander_hall`).
fn gander_hall(cv: &mut Img) {
    let (w, h) = (cv.w, cv.h);
    // The back wall: coursed slate, dim, its joints half drawn.
    for y in 0..h {
        for x in 0..w {
            let course = y / 8;
            let lx = (x + if course % 2 == 0 { 0 } else { 8 }) % 16;
            let ink = if (y % 8 == 0 || lx == 0) && hash(x / 2, y, 77) % 3 != 0 {
                's'
            } else {
                'K'
            };
            cv.put(x, y, ink);
        }
    }
    // Great arches along the back wall, dark between the riders' ledges.
    for cx in [5 * TILE, 12 * TILE, 19 * TILE] {
        recess(cv, cx, 3 * TILE + 8, 4 * TILE, 10 * TILE + 8);
    }
    // The gallery: the realm's folk at the rail above the hall, kept high
    // so a rider on the top ledge flies clear below it.
    cv.rect(0, 0, w, RAIL + 7, 'k');
    tiers(cv, (TILE, 0, w - 2 * TILE), 2, 23);
    cv.rect(TILE, RAIL, w - 2 * TILE, 3, 'u');
    cv.line(TILE, RAIL, w - TILE, RAIL, 'U');
    for x in (TILE + 6..w - TILE).step_by(12) {
        cv.line(x, RAIL + 3, x, RAIL + 6, 'S');
    }
    // The end pillars, full height.
    for x in [0, w - TILE] {
        wall(cv, (x, 0, TILE, h), 91);
        cv.line(if x == 0 { TILE - 1 } else { x }, 0, if x == 0 { TILE - 1 } else { x }, h - 1, 'k');
    }
    // The gate back to the hall, low in the west wall, and its sill.
    let (gx, gy, gw, gh) = c::HALL_GATE;
    cv.rect(0, gy * TILE - 4, (gx + gw) * TILE, gh * TILE + 4, 'k');
    gate(cv, 2, gy * TILE - 6, gw * TILE + 6, (gy + gh) * TILE, 's');
    cv.rect(0, (gy + gh) * TILE - 3, (gx + gw) * TILE + 4, 3, 'u');
    cv.line(0, (gy + gh) * TILE - 3, (gx + gw) * TILE + 3, (gy + gh) * TILE - 3, 'U');
    // The floor below the tar.
    wall(cv, (0, h - TILE, w, TILE), 93);
}

/// The folk in the stands, cheering while a heat is on: drawn per frame
/// over the scenery's tiers, in the arena rooms only.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    if !c::is_arena(kind) {
        return;
    }
    let tick = run.tick as u32;
    let cheer = run.pastimes.heat.is_some() || run.pastimes.round.is_some();
    let skins = ['O', 'o', 't', 'P'];
    let cloth = ['B', 'u', 'p', 'E', 'X', 'r', 'S'];
    let row = |cv: &mut Img, x0: i32, x1: i32, y: i32, seed: i32| {
        let mut x = x0;
        let mut k = 0;
        while x < x1 {
            let h = hash(k, seed, 211);
            if h % 5 != 0 {
                let jump = if cheer && (tick / 4 + h).is_multiple_of(2) {
                    2
                } else if (tick / 30 + h).is_multiple_of(11) {
                    1
                } else {
                    0
                };
                joust::spectator(
                    cv,
                    x,
                    y - jump,
                    h,
                    skins[(h % 4) as usize],
                    cloth[((h >> 3) % 7) as usize],
                    cheer && jump == 2,
                );
            }
            x += 6 + (h % 3) as i32;
            k += 1;
        }
    };
    if kind == RoomKind::KegYard {
        // North stand, over the yard.
        row(cv, 5 * TILE + 4, cv.w - TILE - 6, 2 * TILE - 13, 1);
        // East stand, in tiers stepping back.
        let (sx, sy, _, sh) = c::YARD_STANDS[1];
        for k in 0..TIERS {
            let x = sx * TILE + 10 + k * TIER;
            let mut y = sy * TILE + 4;
            let mut n = 0;
            while y < (sy + sh) * TILE - 8 {
                let hh = hash(k, n, 233);
                if hh % 4 != 0 {
                    joust::spectator(
                        cv,
                        x + (hh % 7) as i32,
                        y - i32::from(cheer && (tick / 4 + hh).is_multiple_of(2)) * 2,
                        hh,
                        skins[(hh % 4) as usize],
                        cloth[((hh >> 3) % 7) as usize],
                        cheer && (tick / 4 + hh).is_multiple_of(2),
                    );
                }
                y += 9;
                n += 1;
            }
        }
    } else {
        // The gallery rail.
        row(cv, TILE + 4, cv.w - TILE - 6, RAIL - 7, 3);
    }
}

/// The Colosseum's lettering: each room's name, the entrance hall's
/// boards (one before each gate: the game's name, three words of how to
/// play, and its best; always up, they are the hall's signs), and the way
/// out when a knight comes near it.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    if !c::is_room(kind) {
        return;
    }
    // The hall's name, top left; in an arena the game's own board, always
    // up, names it.
    if kind == RoomKind::ColosseumHall {
        let title = home::board(&[(c::label(kind).into(), 'i')]);
        cv.stamp(&title, 3, 3);
    }
    if kind == RoomKind::ColosseumHall {
        let record = &run.home.countryside.games;
        for (i, game) in c::GAMES.into_iter().enumerate() {
            let score = match game {
                Game::Gander => &record.gander,
                _ => &record.powder,
            };
            let board = home::board(&[
                (game.name().into(), '9'),
                (c::words(game).into(), 'O'),
                (format!("BEST {}   WINS {}", score.best, score.wins), 'h'),
            ]);
            let (bx, by, bw, _) = c::BOARDS[i];
            let cx = (bx * TILE * 2 + bw * TILE) / 2;
            cv.stamp(&board, cx - board.w / 2, by * TILE - board.h + 6);
        }
    }
    // The way out, once a knight is near it and no heat holds the gate.
    if run.arena_heat_on() {
        return;
    }
    for e in crate::rules::world::ENTRANCES.iter().filter(|e| e.room == kind) {
        let label = match e.id {
            "colosseum-out" => "OUT TO THE KINGDOM",
            "kegs-out" | "gander-out" => "BACK TO THE HALL",
            _ => continue,
        };
        let (ex, ey) = crate::rules::world::entrance_centre(e);
        let near = run.players.values().any(|h| h.hp > 0 && (h.x - ex).hypot(h.y - ey) < 5.0);
        if !near {
            continue;
        }
        let board = home::board(&[(label.into(), '9')]);
        let (x, y) = super::at(ex, ey);
        // Beside the gate, on the side away from the tray.
        let bx = if x - 24 - board.w >= 2 { x - 24 - board.w } else { x + 24 };
        cv.stamp(&board, bx.clamp(2, cv.w - board.w - 2), (y - board.h - 4).clamp(2, cv.h - board.h - 2));
    }
}
