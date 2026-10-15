//! The Wanted hall, drawn: a saloon street of timber balconies and striped
//! awnings, crates and barrels, lanterns on chains; the WANTED poster that
//! goes up as the party walks in (parchment, the outlaw's likeness, its
//! name, the reward in gold); the tell before a volley and the taunt after
//! one; ropes that lower minions; and the outlaw's fall, the coins that fly
//! out of it and the bounty counted at the foot of the screen.
//!
//! Medieval, as the realm is: a notice nailed up by the Crown, not a poster
//! of the Old West. Tonal inks; gold only where the reward is.

use super::super::ink::{Img, bayer, hash, ink, text_width};
use super::super::kit::{Tiles, torch};
use super::{
    NATIVE_H, NATIVE_W, TILE, UNIT, at, dusk, enlarge, fire, room_px, sprites, stature, style,
};
use crate::rules::outlaw::{self, FALL, POSTER, ROPE};
use crate::rules::{Enemy, Run, Tile, ledge::FOOT};
use std::f32::consts::PI;

/// The hall at rest: its back wall of boards and beams, shuttered windows,
/// notices nailed up, the street's stage, balconies with rails, awnings,
/// crates, barrels and lanterns on chains.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let st = style(run.dungeon.pack);
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
    // The back wall: boards, a gap between each, a beam every three tiles.
    for y in 0..ph {
        for x in 0..pw {
            if !open(x / TILE, y / TILE) {
                continue;
            }
            let board = x / 7;
            let ink = if x % 7 == 0 {
                'n'
            } else if y % 48 < 3 {
                if y % 48 == 0 { 'P' } else { 'p' }
            } else {
                // Dark, even boards: the figures and the planks stand out.
                if hash(board, 1, 5) % 5 == 0 {
                    'I'
                } else if hash(x, y, 3).is_multiple_of(17) {
                    'n'
                } else {
                    'b'
                }
            };
            cv.put(x, y, ink);
        }
    }
    // Shuttered windows and nailed notices along the wall.
    for col in [10, 14, 19, 28, 33, 37] {
        let (x, y) = (col * TILE, 3 * TILE);
        if open(col, 3) {
            window(&mut cv, x, y, (col % 2) as u32);
        }
    }
    for (i, col) in [17, 22, 25, 30].iter().enumerate() {
        notice(&mut cv, col * TILE + 3, 3 * TILE + 4, i as u32);
    }
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let (x, y) = (col * TILE, row * TILE);
            let v = hash(col, row, 311);
            match room.tile(col, row) {
                Tile::Wall => cv.stamp(tiles.rock(st.rock, v), x, y),
                Tile::Block if row == 11 && (1..room.cols as i32 - 1).contains(&col) => {
                    stage_board(&mut cv, x, y, col);
                }
                Tile::Block if row > 11 || row == 0 || col == 0 || col == room.cols as i32 - 1 => {
                    cv.stamp(tiles.rock(st.rock, v), x, y)
                }
                Tile::Block if row == 10 && matches!(col, 23 | 24) => {
                    cv.stamp(&barrel(col as u32), x, y)
                }
                Tile::Block => cv.stamp(&crate_tile(v), x, y),
                Tile::Ledge => {
                    let left = room.tile(col - 1, row) != Tile::Ledge;
                    let right = room.tile(col + 1, row) != Tile::Ledge;
                    plank(&mut cv, x, y, left, right, v);
                }
                _ => {}
            }
        }
    }
    // Balcony rails and awnings over the planks.
    for row in [6, 9] {
        let mut col = 0;
        while col < room.cols as i32 {
            if room.tile(col, row) != Tile::Ledge {
                col += 1;
                continue;
            }
            let start = col;
            while room.tile(col, row) == Tile::Ledge {
                col += 1;
            }
            let (x0, x1) = (start * TILE, col * TILE);
            if row == 6 {
                rail(&mut cv, x0, x1, row * TILE);
            } else {
                awning(&mut cv, x0, x1, row * TILE, start == 2);
            }
        }
    }
    // Lanterns on chains from the beams.
    for col in [12, 24, 35] {
        let (x, y) = (col * TILE + 8, 4 * TILE);
        for dy in 0..y - 6 {
            if dy % 2 == 0 {
                cv.put(x, dy, 'J');
            }
        }
        cv.stamp(&lantern(tick / 5 + col as u32), x - 4, y - 8);
        lights.push(fire(x, y - 3, 96.0, 0.5));
    }
    // The doorway: a torch above, bars down while the hall is fought.
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

fn window(cv: &mut Img, x: i32, y: i32, v: u32) {
    // Frame, dark glass with a gleam, a shutter folded back each side.
    cv.rect(x + 2, y + 1, 12, 20, 'p');
    cv.rect(x + 3, y + 2, 10, 18, 'k');
    cv.rect(x + 4, y + 3, 8, 8, 's');
    cv.rect(x + 4, y + 12, 8, 7, 'x');
    cv.line(x + 8, y + 2, x + 8, y + 19, 'p');
    cv.line(x + 3, y + 11, x + 12, y + 11, 'p');
    cv.put(x + 5, y + 4, 'u');
    cv.put(x + 6, y + 5, 'u');
    for k in 0..16 {
        cv.put(x, y + 1 + k, if v == 0 { 'R' } else { 'r' });
        cv.put(x + 1, y + 1 + k, 'B');
        cv.put(x + 15, y + 1 + k, if v == 0 { 'r' } else { 'R' });
        cv.put(x + 14, y + 1 + k, 'B');
    }
}

/// A small notice nailed to the wall: parchment, a dark face, two nails.
fn notice(cv: &mut Img, x: i32, y: i32, v: u32) {
    for dy in 0..15 {
        for dx in 0..11 {
            let torn = (dx == 0 || dx == 10 || dy == 0 || dy == 14) && hash(dx, dy, v) % 3 == 0;
            if !torn {
                cv.put(
                    x + dx,
                    y + dy,
                    if hash(dx, dy, 77) % 7 == 0 { 't' } else { 'T' },
                );
            }
        }
    }
    cv.rect(x + 2, y + 1, 7, 2, 'b');
    cv.rect(x + 3, y + 4, 5, 5, 'p');
    cv.put(x + 4, y + 6, 'k');
    cv.put(x + 6, y + 6, 'k');
    cv.rect(x + 2, y + 11, 7, 1, 'p');
    cv.put(x + 1, y, 'J');
    cv.put(x + 9, y, 'J');
}

/// The stage's floor: boards laid along, nails at their ends.
fn stage_board(cv: &mut Img, x: i32, y: i32, col: i32) {
    for dx in 0..TILE {
        cv.put(x + dx, y, 'O');
        cv.put(x + dx, y + 1, 'o');
        for dy in 2..TILE {
            let seam = dy == 8;
            let ink = if seam {
                'b'
            } else if hash(x + dx, y + dy, 17).is_multiple_of(19) {
                'p'
            } else if dy < 8 {
                'R'
            } else {
                'B'
            };
            cv.put(x + dx, y + dy, ink);
        }
    }
    let _ = col;
    for (nx, ny) in [(2, 4), (13, 4), (6, 12), (10, 12)] {
        cv.put(x + nx, y + ny, 'n');
    }
}

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

/// A balcony's rail: posts every few pixels and a top rail.
fn rail(cv: &mut Img, x0: i32, x1: i32, y: i32) {
    let mut x = x0 + 1;
    while x < x1 {
        for dy in 1..9 {
            cv.put(x, y - dy, if dy == 8 { 'T' } else { 'o' });
        }
        x += 6;
    }
    for x in x0..x1 {
        cv.put(x, y - 8, 'O');
        cv.put(x, y - 7, 'R');
        cv.put(x, y - 3, 'p');
    }
}

/// A striped cloth awning over a plank, sloping out and up.
fn awning(cv: &mut Img, x0: i32, x1: i32, y: i32, west: bool) {
    let w = x1 - x0;
    for dx in 0..w {
        let rise = (dx * 7 / w.max(1)) as i32;
        let top = y - 14 + if west { rise } else { 7 - rise };
        for yy in top..y - 6 {
            let stripe = (dx / 5) % 2 == 0;
            let ink = if yy == top {
                'p'
            } else if stripe {
                'R'
            } else {
                'T'
            };
            cv.put(x0 + dx, yy, ink);
        }
        // The scalloped hem.
        if (dx / 5) % 2 == 0 {
            cv.put(x0 + dx, y - 6, 'R');
            cv.put(x0 + dx, y - 5, 'r');
        } else {
            cv.put(x0 + dx, y - 6, 't');
        }
    }
}

fn crate_tile(v: u32) -> Img {
    let mut im = Img::new(16, 16);
    im.rect(0, 0, 16, 16, 'p');
    im.rect(1, 1, 14, 14, if v.is_multiple_of(2) { 'o' } else { 'r' });
    im.rect(2, 2, 12, 12, 'R');
    im.line(2, 2, 13, 13, 'p');
    im.line(13, 2, 2, 13, 'p');
    im.line(2, 3, 12, 13, 'B');
    for (x, y) in [(1, 1), (14, 1), (1, 14), (14, 14)] {
        im.put(x, y, 'J');
    }
    im.line(0, 0, 15, 0, 'O');
    im
}

fn barrel(v: u32) -> Img {
    let mut im = Img::new(16, 16);
    for y in 0..16i32 {
        let bulge = 1 + ((y - 8).abs() as f32 / 8.0 * 2.0) as i32;
        for x in bulge..16 - bulge {
            let stave = (x + v as i32).rem_euclid(4) == 0;
            im.put(
                x,
                y,
                if stave {
                    'p'
                } else if x < 7 {
                    'o'
                } else {
                    'R'
                },
            );
        }
    }
    for y in [3, 7, 12] {
        for x in 1..15 {
            im.put(x, y, 'J');
        }
    }
    im
}

fn lantern(step: u32) -> Img {
    let flick = step.is_multiple_of(2);
    Img::from_rows(&[
        "..JJJJ..",
        ".J4@@4J.",
        "J4@66@4J",
        "J@6665@J",
        "J4@66@4J",
        if flick { ".J4@@4J." } else { ".J4444J." },
        "..JJJJ..",
    ])
}

// ─── in the hall ─────────────────────────────────────────────────────────────

/// A taunt, in the outlaw's mouth.
const TAUNTS: [&str; 5] = [
    "HANG ME IF YOU CAN!",
    "THE REWARD IS MINE!",
    "STAND AND DRAW!",
    "NOT ONE STEP MORE!",
    "COME AND TAKE IT!",
];

/// The outlaw as it stands in the hall.
fn art_of(run: &Run, index: u8) -> Img {
    stature::boss_figure(run, index).unwrap_or_else(|| Img::new(1, 1))
}

fn bubble(cv: &mut Img, x: i32, y: i32, text: &str, view: (i32, i32)) {
    let w = text_width(text) + 8;
    // Whole on this seat's screen while the speaker is in its view; the tail
    // stays at the speaker, inside the bubble's span.
    let (x0, y0) = (super::in_view_x(x, w, view, cv.w), y - 14);
    let x = x.clamp(x0 + 2, x0 + w - 4);
    super::super::chrome::inset_panel(cv, (x0, y0, w, 12), super::super::chrome::Tone::PAINTED.rimmed([ink('b').unwrap_or_default(); 3]));
    cv.text(x0 + 4, y0 + 2, text, 'b');
    for k in 0..4 {
        cv.put(x - 1 + k / 2, y0 + 12 + k, 'b');
        cv.put(x + k / 2, y0 + 12 + k, 'T');
    }
}

/// The hall's live marks, in the world: the tell before a volley, a taunt,
/// ropes, the outlaw's fall and its coins.
pub(super) fn over(cv: &mut Img, run: &Run, view: (i32, i32)) {
    if run.room().kind != crate::rules::RoomKind::Wanted {
        return;
    }
    let tick = run.tick;
    for e in run.enemies.iter().filter(|e| e.boss.is_some() && e.hp > 0) {
        let Some((index, boss)) = e
            .boss
            .and_then(|i| run.bosses.get(usize::from(i)).map(|b| (i, b)))
        else {
            continue;
        };
        let (x, feet) = at(e.x, e.y + e.radius());
        let h = art_of(run, index).h;
        let tell = outlaw::tell(e, boss);
        if tell > 0.0 {
            // The gun comes up: a gold flash at the muzzle, a mark over the head.
            let side = nearest_side(run, e);
            let (mx, my) = (x + side * (h / 3 + 3), feet - h * 2 / 3);
            let r = (1.0 + tell * 3.0) as i32;
            for s in 0..=r {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    cv.put(mx + dx * s, my + dy * s, if s < 2 { '6' } else { '4' });
                }
            }
            if tell > 0.4 {
                cv.rect(x - 1, feet - h - 12, 2, 6, '7');
                cv.rect(x - 1, feet - h - 4, 2, 2, '7');
            }
            if tell > 0.6
                && let Some(target) = nearest_knight(run, e)
            {
                // A dotted sight to the knight it has marked.
                let (tx, ty) = at(target.0, target.1);
                let n = ((tx - mx).abs().max((ty - my).abs()) / 3).max(1);
                for k in 0..n {
                    if k % 2 == 0 {
                        cv.put(mx + (tx - mx) * k / n, my + (ty - my) * k / n, '7');
                    }
                }
            }
        }
        if e.stage == outlaw::TAUNT {
            let line =
                TAUNTS[(e.id as usize + usize::try_from(e.age / 400).unwrap_or(0)) % TAUNTS.len()];
            // Over its name plate (`nameplates`, 22 pixels over its head),
            // the tail pointing down at it, so the words never cover the name.
            bubble(cv, x, super::in_view_y(feet - h - 39, feet, view) + 14, line, view);
        }
        if e.stage == outlaw::CALL {
            // A whistle: three notes rising.
            for k in 0..3 {
                let up = ((tick as i32 + k * 5) % 16) as i32;
                cv.put(x + 6 + k * 3, feet - h - 4 - up, '6');
                cv.put(x + 6 + k * 3, feet - h - 3 - up, '4');
            }
        }
    }
    // Ropes, lowering their company.
    for rope in &run.works.outlaw.ropes {
        let (rx, _) = at(rope.x, 0.0);
        let ground = 11 * TILE;
        let prog = 1.0 - rope.left as f32 / ROPE as f32;
        let drop = (6.0 + prog * (ground as f32 - 36.0)) as i32;
        for y in 0..drop {
            cv.put(rx, y, if y % 3 == 0 { 'P' } else { 'o' });
        }
        let im = sprites::enemy(rope.kind, tick as u32);
        cv.stamp(&im, rx - im.w / 2, drop);
        // Where it will land: a shadow that tightens.
        let r = (8.0 - prog * 4.0) as i32;
        for k in 0..16 {
            let a = k as f32 / 16.0 * 2.0 * PI;
            cv.put(
                rx + (a.cos() * r as f32) as i32,
                ground - 1 + (a.sin() * 2.0) as i32,
                '7',
            );
        }
    }
    if let Some(fall) = run.works.outlaw.fall {
        fall_world(cv, run, &fall);
    }
}

fn nearest_knight(run: &Run, e: &Enemy) -> Option<(f32, f32)> {
    run.players
        .values()
        .filter(|h| h.hp > 0 && !h.stone)
        .map(|h| (h.x, h.y))
        .min_by(|a, b| {
            (a.0 - e.x)
                .hypot(a.1 - e.y)
                .total_cmp(&(b.0 - e.x).hypot(b.1 - e.y))
        })
}

fn nearest_side(run: &Run, e: &Enemy) -> i32 {
    match nearest_knight(run, e) {
        Some((x, _)) if x < e.x => -1,
        _ => 1,
    }
}

/// Rotate a sprite about its foot (bottom middle) by `angle`, nearest.
fn rotated(im: &Img, angle: f32) -> Img {
    let size = im.w.max(im.h) * 2;
    let mut out = Img::new(size, size);
    let (cx, cy) = (size as f32 / 2.0, size as f32 / 2.0);
    let (sx, sy) = (im.w as f32 / 2.0, im.h as f32);
    let (sin, cos) = angle.sin_cos();
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            let (ux, uy) = (dx * cos + dy * sin + sx, -dx * sin + dy * cos + sy);
            if let Some(c) = im.get(ux.floor() as i32, uy.floor() as i32) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// A body flashing: every lit pixel to one ink, the dark ones to another.
fn flash(im: &Img, light: char, dark: char) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(x, y, if l > 70 { light } else { dark });
            }
        }
    }
    out
}

/// The fall: the outlaw flashes and shakes, tips back over its heels and
/// comes apart, and the coins it carried fly out and bounce.
fn fall_world(cv: &mut Img, run: &Run, fall: &outlaw::Fall) {
    let age = run.tick.saturating_sub(fall.at) as f32;
    if age >= FALL as f32 {
        return;
    }
    let p = age / FALL as f32;
    let (x, feet) = at(fall.x, fall.y);
    let art = art_of(run, fall.boss);
    // The body: flash, then fall.
    let body = if p < 0.3 {
        let bright = (age as i32 / 3) % 2 == 0;
        flash(
            &art,
            if bright { 'c' } else { '7' },
            if bright { '$' } else { '8' },
        )
    } else {
        art.clone()
    };
    let shake = if p < 0.3 {
        ((age as i32 % 4) - 2).signum() * 2
    } else {
        0
    };
    if p < 0.3 {
        cv.stamp(&body, x - body.w / 2 + shake, feet - body.h + 1);
    } else {
        let t = ((p - 0.3) / 0.45).min(1.0);
        let tipped = rotated(&body, -t * t * 1.5);
        let dissolve = ((p - 0.55) / 0.45).max(0.0);
        let mut shown = Img::new(tipped.w, tipped.h);
        for yy in 0..tipped.h {
            for xx in 0..tipped.w {
                if let Some(c) = tipped.get(xx, yy)
                    && bayer(xx + x, yy + feet) >= dissolve
                {
                    shown.set(xx, yy, c);
                }
            }
        }
        cv.stamp(
            &shown,
            x - shown.w / 2,
            feet - shown.h + 1 + (t * 2.0) as i32,
        );
    }
    // The coins: thrown up from the middle at the first flash, falling and
    // bouncing on the stage.
    let ground = 11 * TILE - 2;
    for k in 0..16 {
        let start = 6.0 + (k % 4) as f32;
        if age < start {
            continue;
        }
        let t = age - start;
        let vx = ((hash(k, 3, 21) % 100) as f32 / 100.0 - 0.5) * 2.6;
        let vy = -2.6 - (hash(k, 4, 21) % 100) as f32 / 40.0;
        let mut cy = feet as f32 - 16.0 + vy * t + 0.18 * t * t;
        let cx = x as f32 + vx * t;
        if cy > ground as f32 {
            // A bounce, and then it lies.
            let over = cy - ground as f32;
            cy = ground as f32 - (over * 0.5).min(5.0) * ((t * 0.3).sin().abs());
            cy = cy.min(ground as f32);
        }
        let (px, py) = (cx.round() as i32, cy.round() as i32);
        let glint = (age as i32 + k) % 6 < 3;
        cv.put(px, py, if glint { '6' } else { '4' });
        cv.put(px + 1, py, '4');
        cv.put(px, py + 1, 'a');
    }
}

// ─── the poster and the bounty, in the screen ────────────────────────────────

/// A small text, enlarged `k` times, in `ink`.
fn big_text(s: &str, k: i32, ink: char) -> Img {
    let mut small = Img::new(text_width(s) + 1, 8);
    small.text(0, 0, s, ink);
    enlarge(&small, k as f32)
}

fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Lettering with a dark edge, so gold reads on parchment.
fn gilded(cv: &mut Img, x: i32, y: i32, s: &str, k: i32) {
    let edge = big_text(s, k, 'b');
    let face = big_text(s, k, '@');
    let hi = big_text(s, k, '5');
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (1, 1), (-1, -1)] {
        cv.stamp(&edge, x + dx, y + dy);
    }
    cv.stamp(&face, x, y);
    cv.stamp(&hi, x, y - 1);
}

/// The poster, when it is up, and the bounty banner after a fall.
pub(super) fn poster(cv: &mut Img, run: &Run, legible: bool) {
    if !legible || run.room().kind != crate::rules::RoomKind::Wanted {
        return;
    }
    if run.poster_up() {
        let t = run.tick.saturating_sub(run.works.outlaw.began.unwrap_or(0)) as i32;
        notice_board(cv, run, t);
    }
    if let Some(fall) = run.works.outlaw.fall {
        banner(cv, run, &fall);
    }
}

fn notice_board(cv: &mut Img, run: &Run, t: i32) {
    let Some(index) = run.works.outlaw.boss else {
        return;
    };
    let Some(boss) = run.bosses.get(usize::from(index)) else {
        return;
    };
    // The room dims; the poster drops from above and lifts away at the end.
    let total = POSTER as i32;
    let fade = if t > total - 14 {
        (total - t) as f32 / 14.0
    } else {
        1.0
    };
    for y in 0..cv.h {
        for x in 0..cv.w {
            if bayer(x, y) < 0.5 * fade {
                cv.put(x, y, 'k');
            }
        }
    }
    let (w, h) = (150, 208);
    let rise = if t < 10 {
        ((10 - t) as f32 / 10.0).powi(2)
    } else if t > total - 14 {
        ((t - (total - 14)) as f32 / 14.0).powi(2)
    } else {
        0.0
    };
    let sway = if t < 22 {
        ((t as f32 * 0.5).sin() * (22 - t) as f32 * 0.15).round() as i32
    } else {
        0
    };
    let (x0, y0) = (
        (NATIVE_W - w) / 2 + sway,
        5 - (rise * (h + 20) as f32) as i32,
    );
    // Parchment, ragged at its edge, stained.
    for y in 0..h {
        for x in 0..w {
            let edge = x.min(w - 1 - x).min(y).min(h - 1 - y);
            let rag = (hash(x, y, 91) % 4) as i32;
            if edge < rag {
                continue;
            }
            let ink = if edge < 2 {
                'o'
            } else if edge < 4 {
                'O'
            } else if hash(x / 3, y / 3, 12) % 11 == 0 {
                't'
            } else {
                'T'
            };
            cv.put(x0 + x, y0 + y, ink);
        }
    }
    for (nx, ny) in [(8, 7), (w - 9, 7)] {
        cv.rect(x0 + nx - 1, y0 + ny - 1, 3, 3, 'J');
        cv.put(x0 + nx, y0 + ny, 'i');
    }
    // WANTED, large.
    let title = big_text("WANTED", 3, 'p');
    cv.stamp(
        &big_text("WANTED", 3, 'b'),
        x0 + (w - title.w) / 2 + 1,
        y0 + 16,
    );
    cv.stamp(&title, x0 + (w - title.w) / 2, y0 + 15);
    let sub = "DEAD OR ALIVE";
    cv.text(x0 + (w - text_width(sub)) / 2, y0 + 40, sub, 'p');
    cv.line(x0 + 14, y0 + 51, x0 + w - 15, y0 + 51, 'p');
    cv.line(x0 + 14, y0 + 53, x0 + w - 15, y0 + 53, 'o');
    // The likeness, in a frame.
    let art = Img::from_rows(&boss.art.iter().map(String::as_str).collect::<Vec<_>>());
    let pic = enlarge(&art, 2.0);
    let (fx, fy) = (x0 + (w - 72) / 2, y0 + 60);
    cv.rect(fx, fy, 72, 72, 'p');
    cv.rect(fx + 2, fy + 2, 68, 68, 'n');
    for y in 0..68 {
        for x in 0..68 {
            if bayer(x, y) < 0.18 + y as f32 / 68.0 * 0.2 {
                cv.put(fx + 2 + x, fy + 2 + y, 'b');
            }
        }
    }
    cv.stamp(&pic, fx + 4 + (64 - pic.w) / 2, fy + 4 + (64 - pic.h));
    for (cx, cy) in [(fx, fy), (fx + 70, fy), (fx, fy + 70), (fx + 70, fy + 70)] {
        cv.rect(cx, cy, 2, 2, 'O');
    }
    // The name, one line or two.
    let name = boss.name.to_uppercase();
    let mut lines: Vec<String> = Vec::new();
    if name.chars().count() > 22 {
        let mid = name.len() / 2;
        let cut = name
            .char_indices()
            .filter(|(_, c)| *c == ' ')
            .min_by_key(|(i, _)| i.abs_diff(mid))
            .map_or(mid, |(i, _)| i);
        lines.push(name[..cut].to_string());
        lines.push(name[cut..].trim().to_string());
    } else {
        lines.push(name);
    }
    let mut ny = y0 + 138;
    for line in &lines {
        let k = if text_width(line) * 2 <= w - 14 { 2 } else { 1 };
        let im = big_text(line, k, 'b');
        cv.stamp(&im, x0 + (w - im.w) / 2, ny);
        ny += 8 * k + 2;
    }
    let crown = "BY ORDER OF THE CROWN";
    cv.text(x0 + (w - text_width(crown)) / 2, ny + 2, crown, 'p');
    cv.line(x0 + 30, ny + 13, x0 + w - 31, ny + 13, 'o');
    // The reward, in gold.
    let word = "REWARD";
    cv.text(x0 + (w - text_width(word)) / 2, ny + 18, word, 'p');
    let sum = thousands(run.works.outlaw.bounty);
    let k = if text_width(&sum) * 2 <= w - 70 { 2 } else { 1 };
    let sum_w = text_width(&sum) * k;
    let gold = "GOLD";
    // The coin, the sum and the word, in one line.
    let whole = 9 + sum_w + 5 + text_width(gold);
    let left = x0 + (w - whole) / 2;
    let sy = ny + 28;
    coin(cv, left + 3, sy + 4 * k);
    gilded(cv, left + 9, sy, &sum, k);
    cv.text(left + 9 + sum_w + 5, sy + 4 * k - 3, gold, 'p');
    // A wax seal, the realm's sun-star in it.
    let (sx, sy) = (x0 + w - 13, y0 + h - 12);
    for y in -7..=7i32 {
        for x in -7..=7i32 {
            let d = x * x + y * y;
            if d <= 49 && hash(x, y, 4) % 9 != 0 {
                cv.put(sx + x, sy + y, if d > 36 { '8' } else { '7' });
            }
        }
    }
    for (dx, dy) in [(0, -3), (0, 3), (-3, 0), (3, 0), (0, 0)] {
        cv.put(sx + dx, sy + dy, '4');
    }
}

fn coin(cv: &mut Img, x: i32, y: i32) {
    let im = Img::from_rows(&[".aaaa.", "a4@@5a", "a@55@a", "a@55@a", "a4@@5a", ".aaaa."]);
    cv.stamp(&im, x - im.w / 2, y - im.h / 2);
}

/// After the fall: a scroll across the foot of the screen counts the
/// bounty out.
fn banner(cv: &mut Img, run: &Run, fall: &outlaw::Fall) {
    let age = run.tick.saturating_sub(fall.at) as i32;
    if age < 24 || age > FALL as i32 + 60 {
        return;
    }
    let rise = ((age - 24).min(14) as f32 / 14.0).min(1.0);
    let w = 190;
    let (x0, y0) = ((NATIVE_W - w) / 2, NATIVE_H - 6 - (rise * 42.0) as i32);
    super::super::chrome::panel(cv, (x0, y0, w, 40), super::super::chrome::Tone::PAINTED.rimmed(super::super::chrome::rim_of('p')));
    cv.rect(x0 + 3, y0 + 2, w - 6, 2, 'O');
    cv.rect(x0 + 3, y0 + 36, w - 6, 2, 'O');
    let head = "BOUNTY PAID";
    cv.text(x0 + (w - text_width(head)) / 2, y0 + 5, head, 'p');
    // The sum counts up.
    let shown = (fall.bounty as f32 * ((age - 24) as f32 / 40.0).min(1.0)) as u32 / 10 * 10;
    let sum = thousands(shown.min(fall.bounty));
    let sum_w = text_width(&sum) * 2;
    gilded(cv, x0 + (w - sum_w) / 2 + 6, y0 + 17, &sum, 2);
    coin(cv, x0 + (w - sum_w) / 2 - 4, y0 + 25);
}

/// A boss or a route chief's name over its health bar, and the key it
/// holds.
pub(super) fn nameplates(cv: &mut Img, run: &Run, view: (i32, i32)) {
    use crate::rules::keyring::Hold;
    let chiefs: Vec<u8> = run.route_chiefs();
    let keyless = run.works.key.hold == Hold::Kept;
    for e in run.enemies.iter().filter(|e| e.hp > 0 && e.boss.is_some()) {
        let Some(index) = e.boss else { continue };
        let Some(boss) = run.bosses.get(usize::from(index)) else {
            continue;
        };
        if run.poster_up() {
            continue;
        }
        let (x, feet) = if run.side_on() {
            at(e.x, e.y + e.radius())
        } else {
            at(e.x, e.y + super::signatures::foot_lift(run, e))
        };
        let top = feet - super::foe_figure(run, e, run.tick as u32).h;
        let name = boss.name.to_uppercase();
        let holds_key = keyless && chiefs.contains(&index);
        let w = text_width(&name) + if holds_key { 12 } else { 0 };
        // Whole on this seat's screen while the boss is in its view.
        let x0 = super::in_view_x(x, w + 6, view, cv.w) + 3;
        let y0 = super::in_view_y(top - 20, feet, view);
        // The name on a slate plate, in the steel rim of the realm's names.
        super::super::chrome::panel(cv, (x0 - 3, y0 - 2, w + 6, 11), super::super::chrome::Tone::SLATE);
        let mut tx = x0;
        if holds_key {
            let key = Img::from_rows(&[".44....", "4..4444", ".44.4.4"]);
            cv.stamp(&key, tx, y0 + 1);
            tx += 12;
        }
        cv.text(tx, y0, &name, '$');
    }
    let _ = (UNIT, FOOT);
}
