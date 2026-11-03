//! The surface arcade uses cached scenery. Only pads, six course markers and
//! a tiny nuisance move per frame. Browser friends see the host's same picture.

use super::super::ink::{Img, text_width};
use super::super::material as m;
use super::{TILE, at};
use crate::rules::{
    HZ, RoomKind, Run,
    pastimes::{self, Game},
};

/// One glyph onto the canvas without a heap `String`, for the per-frame
/// pad/flag/gate labels the arcade draws every tick.
fn glyph1(cv: &mut Img, x: i32, y: i32, c: char, ink: char) {
    let mut buf = [0u8; 4];
    cv.text(x, y, c.encode_utf8(&mut buf), ink);
}

fn ring(cv: &mut Img, x: i32, y: i32, radius: f32, ink: char) {
    // The 40 unit vectors are fixed; a ring only scales them by `radius`, so
    // compute the trig once rather than 40 sin+cos per ring per frame.
    static UNIT: std::sync::OnceLock<[(f32, f32); 40]> = std::sync::OnceLock::new();
    let unit = UNIT.get_or_init(|| {
        std::array::from_fn(|n| {
            let a = n as f32 * std::f32::consts::TAU / 40.0;
            (a.cos(), a.sin())
        })
    });
    for (c, s) in unit {
        cv.put(x + (*c * radius) as i32, y + (*s * radius) as i32, ink);
    }
}

fn cottage(cv: &mut Img, c: i32, r: i32, w: i32, h: i32, k: usize) {
    let (x, y, w, h) = (c * TILE, r * TILE, w * TILE, h * TILE);
    let roof = [m::ramp::THATCH, m::ramp::TILE, m::ramp::SLATE][k % 3];
    m::house(
        cv,
        (x + 3, y - 6, w - 6, h + 4),
        &m::House {
            roof,
            stone: k % 2 == 0,
            windows: (w / 24).max(1),
        },
        100 + k as u32,
    );
}

pub(super) fn scenery(cv: &mut Img, kind: RoomKind) {
    match kind {
        RoomKind::Village => {
            for (i, (c, r, w, h)) in [
                (3, 1, 4, 2),
                (7, 1, 2, 2),
                (16, 1, 5, 2),
                (3, 10, 4, 3),
                (15, 10, 4, 3),
            ]
            .into_iter()
            .enumerate()
            {
                cottage(cv, c, r, w, h, i);
            }
            m::barrel(cv, 10 * TILE - 4, 3 * TILE - 2);
            m::crate_box(cv, 21 * TILE + 8, 3 * TILE - 1, 101);
            m::fence(cv, 7 * TILE + 4, 9 * TILE + 8, 12 * TILE + 14);
        }
        RoomKind::Fairground => {
            for (k, (c, r, w)) in [(3, 1, 5), (17, 1, 4)].into_iter().enumerate() {
                let (x, y) = (c * TILE, r * TILE);
                // A stall: striped awning over a boarded counter.
                m::block(
                    cv,
                    (x + 3, y + 12, w * TILE - 6, 18),
                    m::ramp::TIMBER,
                    true,
                    110 + k as u32,
                );
                for n in 0..w * TILE {
                    let stripe = (n / 8) % 2 == 0;
                    for d in 0..11 {
                        let ch = match (stripe, d) {
                            (_, 0) => 'b',
                            (true, d) if d < 4 => 'R',
                            (true, _) => 'p',
                            (false, d) if d < 4 => 'o',
                            (false, _) => 'r',
                        };
                        cv.put(x + n, y + 3 + d, ch);
                    }
                    if n % 8 == 4 {
                        cv.put(x + n, y + 14, if stripe { 'p' } else { 'r' });
                    }
                }
                cv.line(x, y + 15, x + w * TILE - 1, y + 15, 'n');
            }
            // Bunting well above the playable slalom.
            for x in (2 * TILE..22 * TILE).step_by(20) {
                cv.line(x, 3 * TILE, x + 10, 3 * TILE + 5, 'P');
                cv.line(x + 10, 3 * TILE + 5, x + 20, 3 * TILE, 'P');
                let ink = if x % 40 == 0 { 'R' } else { 'u' };
                for d in 0..5 {
                    cv.line(
                        x + 7 + d / 2,
                        3 * TILE + 3 + d,
                        x + 11 - d / 2,
                        3 * TILE + 3 + d,
                        ink,
                    );
                }
            }
        }
        RoomKind::Riverbank => {
            // The quay's landing stage over the river, on piles.
            m::boards(cv, (9 * TILE + 2, TILE, 6 * TILE - 4, 3 * TILE), true, 120);
            for x in [9 * TILE + 2, 15 * TILE - 4] {
                for y in (TILE + 4..4 * TILE).step_by(12) {
                    cv.rect(x, y, 2, 3, 'n');
                }
            }
            for x in (9 * TILE + 2..15 * TILE - 2).step_by(2) {
                cv.put(x, 4 * TILE, 'f');
            }
            // Wash lines and drying socks along the quay, outside the course.
            for x in [3 * TILE, 9 * TILE] {
                cv.line(x, 4 * TILE - 6, x, 4 * TILE + 8, 'P');
            }
            cv.line(3 * TILE, 4 * TILE - 5, 9 * TILE, 4 * TILE - 4, 'J');
            for (n, x) in (3 * TILE + 6..9 * TILE - 4).step_by(14).enumerate() {
                let ink = ['h', 'u', 'J', 'o'][n % 4];
                cv.rect(x, 4 * TILE - 4, 4, 7, ink);
                cv.rect(x + 2, 4 * TILE + 1, 5, 3, ink);
                cv.put(x, 4 * TILE - 4, 'i');
            }
        }
        _ => {}
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let active = run.pastimes.round.as_ref();
    let kind = run.room().kind;
    if kind == RoomKind::GanderHall {
        gander_hall(cv, run);
    }
    for game in Game::ALL.into_iter().filter(|g| g.room() == kind) {
        let (tx, ty) = game.start();
        let (x, y) = at(tx * 2.0, ty * 2.0);
        // At rest a pad is a quiet dark ring, readable, never lit; the
        // amber lamp only while its round is on.
        let (ring_ink, letter_ink) = if active.is_some_and(|r| r.game == game) {
            ('4', '4')
        } else {
            ('g', 'J')
        };
        ring(cv, x, y, 11.0, ring_ink);
        glyph1(cv, x - 2, y - 3, pad_letter(game), letter_ink);
    }
    if let Some(peal) = active
        .filter(|r| r.game == Game::Peal)
        .and_then(|r| r.peal.as_ref())
    {
        let (tx, ty) = Game::Peal.start();
        let (x, y) = at(tx * 2.0, ty * 2.0);
        // The rope up to the ringer's bell (drawn the tax bells' way), and
        // the ring that closes on the bell as the beat comes: gold while a
        // pull rings true, green once rung, red on a clanger. The ring's
        // size is the time left in the swing.
        let by = y - 3 * TILE;
        let sway = (((peal.phase as f32 / peal.span() as f32) * std::f32::consts::TAU).sin() * 3.0) as i32;
        let bell_ink = if peal.clang_flash > 0 { 'R' } else { 'O' };
        // A timber A-frame carries the practice bell; the rope runs from
        // the bell's mouth down to the ringer's hands at the pad.
        cv.line(x - 8, by - 8, x - 15, y + 4, 'P');
        cv.line(x + 8, by - 8, x + 15, y + 4, 'P');
        cv.line(x - 8, by - 8, x + 8, by - 8, 'P');
        cv.line(x + sway, by + 2, x, y - 2, 'P');
        cv.ellipse((x + sway) as f32, by as f32, 7.0, 5.0, bell_ink);
        cv.ellipse((x + sway) as f32, (by - 5) as f32, 4.0, 3.0, bell_ink);
        cv.line(x + sway - 7, by + 5, x + sway + 7, by + 5, 'J');
        cv.rect(x + sway - 1, by - 8, 2, 3, 'k');
        cv.ellipse((x + sway) as f32, (by + 2) as f32, 2.0, 2.0, 'n');
        cv.put(x + sway - 3, by - 2, 'H');
        let left = peal.span().saturating_sub(peal.phase) as f32;
        let radius = 8.0 + left * 12.0 / peal.span() as f32;
        let ink = if peal.in_window() {
            '4'
        } else if peal.rung {
            '2'
        } else if peal.spoiled {
            'R'
        } else {
            'g'
        };
        ring(cv, x + sway, by, radius, ink);
    }
    if kind == RoomKind::Village {
        for (i, (tx, ty)) in pastimes::MAILBOXES.into_iter().enumerate() {
            let (x, y) = at(tx * 2.0, ty * 2.0);
            let marked =
                active.is_some_and(|r| r.game == Game::Broadsheet && r.recipients & (1 << i) != 0);
            let done =
                active.is_some_and(|r| r.game == Game::Broadsheet && r.tried & (1 << i) != 0);
            cv.rect(x - 1, y + 6, 2, 8, 'n');
            cv.ellipse(x as f32, (y - 11) as f32, 6.0, 3.0, 'B');
            cv.rect(x - 6, y - 11, 12, 10, 'B');
            cv.rect(x - 3, y - 8, 6, 1, 'k');
            cv.rect(x - 7, y - 1, 14, 2, 'n');
            if marked && !done {
                cv.line(x + 6, y - 13, x + 11, y - 11, 'O');
                cv.line(x + 11, y - 11, x + 6, y - 9, 'O');
                cv.line(x + 6, y - 9, x + 6, y - 13, 'O');
            }
        }
    }
    super::coastal::put_figures(cv, run);
    super::woodland::cart_figures(cv, run);
    if let Some(round) = active.filter(|r| r.game == Game::Cheese) {
        let (tx, ty) = pastimes::cheese_target(round);
        let (x, y) = at(tx * 2.0, ty * 2.0);
        ring(
            cv,
            x,
            y,
            14.0,
            if round.hazard_left > 0 { 'g' } else { 'O' },
        );
        // Basket, goat body, cloven feet and officious horns.
        cv.rect(x - 5, y - 3, 10, 5, 'o');
        for c in [-3, 0, 3] {
            cv.line(x + c, y - 3, x + c, y + 1, 'O');
        }
        cv.rect(x + 5, y - 5, 8, 5, 'h');
        cv.rect(x + 11, y - 8, 4, 6, 'h');
        cv.line(x + 11, y - 8, x + 10, y - 11, 'O');
        cv.line(x + 14, y - 8, x + 16, y - 10, 'O');
        cv.line(x + 7, y, x + 7, y + 4, 'o');
        cv.line(x + 11, y, x + 11, y + 4, 'o');
        if let Some(cheese) = &round.cheese {
            let (x, y) = at(cheese.x as f32 / 256.0 * 2.0, cheese.y as f32 / 256.0 * 2.0);
            cv.ellipse(x as f32, y as f32, 5.0, 4.0, 'O');
            cv.ellipse(x as f32, y as f32, 4.0, 3.0, 'Y');
            cv.put(x - 1, y - 1, 'o');
            cv.put(x + 1, y, 'o');
            cv.put(x, y + 1, 'o');
            cv.line(x - 2, y + 2, x + 2, y - 2, 'o');
        }
    }
    if let Some(round) = active.filter(|r| r.game == Game::Powder) {
        if let Some(p) = &round.powder {
            // ONE shared courtyard for every knight in the heat: flagstone
            // floor, stone posts on every other tile the way the trope has
            // them, bales on the rest, lanes open at each corner. Each
            // painter cell is half a tile.
            let at_tile = |c: u8, r: u8| {
                at(
                    f32::from(pastimes::POWDER_X0 + c) * 2.0,
                    f32::from(pastimes::POWDER_Y0 + r) * 2.0,
                )
            };
            // The flagstone floor of its own: big pale slabs, cracks dark.
            for r in 0..pastimes::POWDER_H {
                for c in 0..pastimes::POWDER_W {
                    let (x, y) = at_tile(c, r);
                    let slab = ((u32::from(c) * 7 + u32::from(r) * 13) % 3) as i32;
                    // Tonal, so the bales, kegs and fire read first.
                    cv.rect(x, y, 16, 16, if slab == 0 { 'G' } else { 'j' });
                    if (u32::from(c) + u32::from(r)) % 4 == 0 {
                        cv.put(x + 8, y + 8, 'g');
                    }
                    cv.line(x, y + 15, x + 15, y + 15, 'g');
                    cv.line(x + 15, y, x + 15, y + 15, 'g');
                }
            }
            // Scorch marks where fire has been, fading to grey.
            for s in &p.scorch {
                let (x, y) = at_tile(s.col, s.row);
                let pale = s.left < pastimes::FIRE;
                let ink = if pale { 'X' } else { 'K' };
                cv.rect(x + 3, y + 3, 10, 10, ink);
                cv.put(x + 8, y + 8, ink);
            }
            // The stone posts: on every other tile, mossy stubs.
            for &(c, r) in pastimes::POWDER_POSTS.iter() {
                let (x, y) = at_tile(c, r);
                cv.rect(x + 4, y + 2, 8, 12, 'S');
                cv.rect(x + 3, y, 10, 4, 'S');
                cv.put(x + 5, y + 3, 'l');
                cv.put(x + 9, y + 2, 'l');
                cv.line(x + 3, y + 13, x + 12, y + 13, '5');
            }
            // The hay bales.
            for (i, &t) in p.bales.iter().enumerate() {
                let (c, r) = (t % pastimes::POWDER_W, t / pastimes::POWDER_W);
                let (x, y) = at_tile(c, r);
                m::hay(cv, x + 8, y + 12, 9.5, 300 + i as u32);
            }
            // The charms, bobbing on their pins.
            for c in &p.charms {
                let (x, y) = at_tile(c.col, c.row);
                let bob = [0, 1, 2, 1]
                    .into_iter()
                    .cycle()
                    .nth(run.tick as usize / 8 + usize::from(c.col))
                    .unwrap_or(0);
                let (ink, pin) = match c.kind {
                    0 => ('R', 'r'),
                    1 => ('Y', 'y'),
                    _ => ('G', 'g'),
                };
                cv.rect(x + 6, y + 5 - bob, 5, 5, ink);
                cv.put(x + 8, y + 4 - bob, 'W');
                cv.line(x + 8, y + 10, x + 8, y + 14, pin);
            }
            // The knights of the heat, each a small figure with their soot
            // flash when a blast catches them.
            for k in &p.knights {
                let (x, y) = at_tile(k.tile().0, k.tile().1);
                let sub = (k.x % 256, k.y % 256);
                let (x, y) = (x + sub.0 as i32 / 32, y + sub.1 as i32 / 32);
                // A standing knight is drawn as themself (the rules walk
                // them with their yard knight); only the soot flash here.
                if k.soot == 0 {
                    continue;
                }
                let body = if k.soot > 0 { '5' } else { 'O' };
                cv.rect(x + 5, y + 4, 6, 9, body);
                cv.rect(x + 6, y, 4, 4, if k.soot > 0 { '5' } else { 'W' });
                cv.put(x + 7, y + 12, '3');
                if k.soot > 0 {
                    cv.put(x + 4, y - 2, 'k');
                    cv.put(x + 11, y - 2, 'k');
                }
            }
            // The kegs: twice the size, a fuse spark that shortens as the
            // fuse burns.
            for k in &p.kegs {
                let (x, y) = at_tile(k.col, k.row);
                // A black powder keg with iron hoops.
                cv.ellipse(x as f32 + 8.0, (y + 12) as f32, 14.0, 6.0, '5');
                cv.rect(x - 4, y + 4, 24, 16, '5');
                cv.line(x - 4, y + 9, x + 19, y + 9, '3');
                cv.line(x - 4, y + 14, x + 19, y + 14, '3');
                cv.rect(x - 5, y + 2, 26, 3, '5');
                cv.rect(x - 2, y - 4, 6, 6, 'R');
                // The fuse: long at the drop, a stub at the boom. The spark
                // rides its tip.
                let burn = 1.0 - f32::from(k.left) / f32::from(pastimes::FUSE);
                let fuse_len = (12.0 * (1.0 - burn)) as i32;
                cv.line(x + 1, y - 4, x + 1 + fuse_len, y - 12, '3');
                let (sx, sy) = (x + 1 + fuse_len, y - 12);
                cv.put(sx, sy, 'Y');
                cv.put(sx + 1, sy - 1, 'O');
                if k.left % 10 < 5 {
                    cv.put(sx - 1, sy - 1, 'O');
                }
            }
            // The cross of fire: a burst frame, flame tongues along the
            // arms, white-hot early, ember red late, on screen a full
            // second and a half (FIRE = 45).
            for f in &p.flames {
                let (x, y) = at_tile(f.col, f.row);
                let hot = f.left > pastimes::FIRE - 6;
                let warm = f.left > pastimes::FIRE / 2;
                // Signal inks: orange round a yellow heart while hot, red
                // round orange as it burns, ember round red at the last.
                let ink = if hot {
                    '@'
                } else if warm {
                    '7'
                } else {
                    '8'
                };
                // The burst frame: a starburst where the keg stood.
                if f.arm == 4 && f.left > pastimes::FIRE - 4 {
                    for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                        cv.put(x + 8 + dx * 9, y + 8 + dy * 9, '6');
                        cv.put(x + 8 + dx * 13, y + 8 + dy * 13, '@');
                    }
                    cv.rect(x + 1, y + 1, 14, 14, '@');
                    cv.rect(x + 4, y + 4, 8, 8, '6');
                }
                // The arm's own tile of flame, with tongues flickering.
                cv.rect(x + 2, y + 2, 12, 12, ink);
                let heart = if hot {
                    '6'
                } else if warm {
                    '@'
                } else {
                    '7'
                };
                cv.rect(x + 5, y + 4, 6, 7, heart);
                // Tongues licking up off every tile, flickering.
                let lick = (run.tick + u64::from(f.col) * 5 + u64::from(f.row) * 7) % 3;
                cv.rect(x + 4 + lick as i32 * 2, y - 2, 2, 4, ink);
                cv.rect(x + 9 - lick as i32, y - 1, 2, 3, ink);
                cv.put(x + 7, y + 1 - lick as i32, heart);
                let flick = (run.tick + u64::from(f.col) * 3 + u64::from(f.row)) % 4;
                let mut tongue = |tx: i32, ty: i32, ink: char| cv.put(tx, ty, ink);
                match f.arm {
                    0 => {
                        tongue(x + 16 + flick as i32, y + 8, ink);
                        tongue(x + 18, y + 6, ink);
                    }
                    1 => {
                        tongue(x - 1 - flick as i32, y + 8, ink);
                        tongue(x - 3, y + 10, ink);
                    }
                    2 => {
                        tongue(x + 8, y + 16 + flick as i32, ink);
                        tongue(x + 6, y + 18, ink);
                    }
                    3 => {
                        tongue(x + 8, y - 1 - flick as i32, ink);
                        tongue(x + 10, y - 3, ink);
                    }
                    _ => {}
                }
            }
            // Chain explosion visual: bright flash at both keg positions + connecting line.
            if let Some((ticks, x1, y1, x2, y2)) = p.chain_flash {
                if ticks > 0 {
                    let (px1, py1) = at_tile(x1, y1);
                    let (px2, py2) = at_tile(x2, y2);
                    // Bright flash at both keg positions.
                    let flash_ink = if ticks > 4 {
                        'W'
                    } else if ticks > 2 {
                        '6'
                    } else {
                        '@'
                    };
                    cv.rect(px1 + 2, py1 + 2, 12, 12, flash_ink);
                    cv.rect(px2 + 2, py2 + 2, 12, 12, flash_ink);
                    cv.put(px1 + 8, py1 + 8, '9');
                    cv.put(px2 + 8, py2 + 8, '9');
                    // Connecting line between the two positions.
                    let line_ink = if ticks > 3 { 'Y' } else { 'O' };
                    let dx = px2 - px1;
                    let dy = py2 - py1;
                    let steps = dx.abs().max(dy.abs());
                    if steps > 0 {
                        for i in 0..=steps {
                            let lx = px1 + dx * i / steps;
                            let ly = py1 + dy * i / steps;
                            cv.put(lx + 8, ly + 8, line_ink);
                        }
                    }
                }
            }
            // Chain shockwave: expanding ring between the two keg positions.
            if p.chain_shockwave > 0 {
                if let Some((_, x1, y1, x2, y2)) = p.chain_flash {
                    let (px1, py1) = at_tile(x1, y1);
                    let (px2, py2) = at_tile(x2, y2);
                    // Center point between the two kegs.
                    let cx = (px1 + px2) / 2 + 8;
                    let cy = (py1 + py2) / 2 + 8;
                    // Expanding ring, fading out.
                    let alpha = p.chain_shockwave as f32 / 6.0;
                    let radius = 8.0 + (6.0 - p.chain_shockwave as f32) * 3.0;
                    let ring_ink = if alpha > 0.6 {
                        'O'
                    } else if alpha > 0.3 {
                        '6'
                    } else {
                        '@'
                    };
                    ring(cv, cx, cy, radius, ring_ink);
                    // Inner bright ring on first few ticks.
                    if p.chain_shockwave > 4 {
                        ring(cv, cx, cy, radius * 0.6, 'W');
                    }
                }
            }
        }
    }

    if kind == RoomKind::Causeway {
        let (lx, ly) = at(2.0 * 2.0, (pastimes::RIDING_LANE.0 - 0.25) * 2.0);
        let (rx, by) = at(22.0 * 2.0, (pastimes::RIDING_LANE.1 + 0.25) * 2.0);
        for x in (lx..rx).step_by(12) {
            cv.line(x, ly, x + 6, ly, 'g');
            cv.line(x, by, x + 6, by, 'g');
        }
        // The hurdles themselves stand in the crowd (`pastimes::crowd`) so
        // a window tilts them with the lane and shrinks them with distance.
        let current = active.filter(|r| r.game == Game::Hobbyhorse);
        if let Some(round) = current {
            for (&id, rider) in &round.riders {
                let Some(hero) = run
                    .players
                    .get(&id)
                    .filter(|h| h.hp > 0 && !h.stone && h.privy == 0)
                else {
                    continue;
                };
                let (x, y) = at(hero.x, hero.y);
                let lift = if rider.air_left > 0 {
                    let age = pastimes::JUMP_TICKS - rider.air_left;
                    2 + (age.min(pastimes::JUMP_TICKS - age) * 2) as i32
                } else {
                    0
                };
                // A hobbyhorse: stick, crossbar, horse-head silhouette.
                let hx = x + 8;
                let hy = y - 2 - lift;
                cv.line(hx, hy + 2, hx, hy + 14, 'n'); // brown stick
                cv.line(hx - 3, hy + 8, hx + 3, hy + 8, 'n'); // crossbar
                cv.ellipse(hx as f32, (hy - 4) as f32, 3.0, 2.0, 'h'); // tan muzzle
                cv.line(hx - 2, hy - 6, hx - 2, hy - 2, 'R'); // red mane
                cv.put(hx + 1, hy - 3, 'k'); // nostril
                cv.line(hx - 1, hy - 6, hx - 2, hy - 8, 'h'); // left ear
                cv.line(hx + 1, hy - 6, hx + 2, hy - 8, 'h'); // right ear
                cv.put(hx, hy - 5, 'k'); // eye
                cv.line(hx + 2, hy - 4, hx + 3, hy - 4, 'o'); // mouth
                if lift > 0 {
                    ring(cv, x, y - lift, 12.0, 'o');
                }
            }
        }
    }
    if kind == RoomKind::Crag {
        // The six gates stand in the crowd (`pastimes::crowd`), each at
        // its own row of the hill; the sled itself stays with the rider.
        let current = active.filter(|r| r.game == Game::Sled);
        if let Some(sled) = current.and_then(|r| r.sled.as_ref()) {
            let (x, y) = at(sled.x as f32 / 128.0, sled.y as f32 / 128.0);
            // Sled runners with curved fronts.
            cv.line(x - 5, y, x - 3, y + 3, 's');
            cv.line(x - 3, y + 3, x + 3, y + 3, 's');
            cv.line(x + 5, y, x + 3, y + 3, 's');
            cv.line(x + 3, y + 3, x - 3, y + 3, 's');
            // Sled body as parallelogram (leaning posture).
            cv.line(x - 3, y - 8, x + 3, y - 8, 'u');
            cv.line(x - 4, y - 2, x + 2, y - 2, 'u');
            cv.line(x - 3, y - 8, x - 4, y - 2, 'u');
            cv.line(x + 3, y - 8, x + 2, y - 2, 'u');
            // Round head.
            cv.ellipse(x as f32, (y - 11) as f32, 3.0, 3.0, 'h');
            // Bright visor.
            cv.line(x - 2, y - 11, x + 2, y - 11, '2');
            // Arms gripping reins.
            cv.line(x - 4, y - 6, x - 7, y - 3, 'h');
            cv.line(x + 2, y - 6, x + 5, y - 3, 'h');
            // Orange scarf trailing behind.
            cv.line(x + 3, y - 10, x + 7, y - 9, 'O');
            cv.line(x + 7, y - 9, x + 10, y - 10, 'O');
            // Motion blur line behind sled.
            cv.line(x + 8, y - 5, x + 12, y - 5, 'j');
        }
    }
    if let Some(round) = active.filter(|r| {
        !matches!(
            r.game,
            Game::Turnip
                | Game::Cheese
                | Game::Hobbyhorse
                | Game::Sled
                | Game::Pot
                | Game::Cart
                | Game::Surf
                | Game::Longbow
        )
    }) {
        let (tx, ty) = pastimes::nuisance(round.game, run.tick);
        let (x, y) = at(tx * 2.0, ty * 2.0);
        if round.game == Game::Broadsheet || round.game == Game::Washtub {
            // Goose/royal duck: long neck, white belly, aggressively orange bill.
            cv.rect(x - 5, y - 5, 10, 6, 'h');
            cv.line(x + 3, y - 5, x + 3, y - 12, '9');
            cv.rect(x + 2, y - 13, 5, 4, '9');
            cv.rect(x + 7, y - 11, 4, 2, 'O');
            cv.put(x + 5, y - 12, 'k');
            cv.line(x - 3, y + 1, x - 3, y + 3, 'O');
            cv.line(x + 2, y + 1, x + 2, y + 3, 'O');
        } else {
            cv.rect(x - 4, y - 9, 8, 9, 'R');
            cv.rect(x - 3, y - 14, 6, 5, 'h');
            cv.line(x - 6, y - 15, x, y - 20, 'u');
            cv.line(x, y - 20, x + 6, y - 15, 'O');
            cv.put(x - 6, y - 15, '9');
            cv.put(x + 6, y - 15, '9');
        }
    }
}

/// The arcade's standing course markers join the room's crowd so a window
/// tilts them with the floor and shrinks them with distance: Barrel flags,
/// Washtub rings (their tile recomputes each tick) and the Turnip ball
/// (feet on the ring, the bounce baked into its sprite). More games follow
/// the same shape as they are pulled through.
pub(super) fn crowd<'a>(
    crowd: &mut Vec<(f32, u8, (f32, f32), Box<dyn FnOnce(&mut Img) + 'a>)>,
    run: &'a Run,
) {
    let kind = run.room().kind;
    let active = run.pastimes.round.as_ref();
    if kind == Game::Barrel.room() {
        for i in 0..6 {
            let (tx, ty) = pastimes::FLAGS[i];
            let next =
                active.is_some_and(|r| r.game == Game::Barrel && usize::from(r.progress) == i);
            let done =
                active.is_some_and(|r| r.game == Game::Barrel && usize::from(r.progress) > i);
            let ink = if next {
                'o'
            } else if done {
                'g'
            } else {
                'u'
            };
            let radius = if next { 12.0 } else { 8.0 };
            let frac = super::pixel_fraction(tx * 2.0, ty * 2.0);
            let lift_px = if next { 3.0 } else { 0.0 };
            crowd.push((
                ty * 2.0,
                2,
                frac,
                Box::new(move |cv| {
                    super::super::arena::lift(lift_px);
                    if lift_px > 0.0 {
                        super::super::arena::glow();
                    }
                    let (x, y) = at(tx * 2.0, ty * 2.0);
                    ring(cv, x, y, radius, ink);
                    cv.line(x, y - 14, x, y - 3, ink);
                    cv.rect(x + 1, y - 14, 7, 5, ink);
                    glyph1(cv, x - 2, y - 2, char::from(b'1' + i as u8), ink);
                }),
            ));
        }
    }
    if kind == Game::Washtub.room() {
        for i in 0..6 {
            let (tx, ty) = pastimes::wake(i, run.tick);
            let next =
                active.is_some_and(|r| r.game == Game::Washtub && usize::from(r.progress) == i);
            let done =
                active.is_some_and(|r| r.game == Game::Washtub && usize::from(r.progress) > i);
            let ink = if next {
                'o'
            } else if done {
                'g'
            } else {
                'u'
            };
            let radius = if next { 12.0 } else { 8.0 };
            let frac = super::pixel_fraction(tx * 2.0, ty * 2.0);
            let lift_px = if next { 3.0 } else { 0.0 };
            crowd.push((
                ty * 2.0,
                2,
                frac,
                Box::new(move |cv| {
                    super::super::arena::lift(lift_px);
                    if lift_px > 0.0 {
                        super::super::arena::glow();
                    }
                    let (x, y) = at(tx * 2.0, ty * 2.0);
                    ring(cv, x, y, radius, ink);
                    glyph1(cv, x - 2, y - 2, char::from(b'1' + i as u8), ink);
                }),
            ));
        }
    }
    if kind == Game::Turnip.room() {
        let (tx, ty) = Game::Turnip.start();
        let round = active.filter(|r| r.game == Game::Turnip);
        let elapsed = round.map_or(0, |r| Game::Turnip.time() - r.left);
        let phase = elapsed % HZ;
        let height = ((phase as f32 / HZ as f32 * std::f32::consts::PI).sin() * 24.0) as i32;
        let cue = round.is_some() && phase <= HZ / 5;
        let ring_ink = if cue { '4' } else { 'r' };
        let ball_ink = if cue { 'O' } else { 'h' };
        let frac = super::pixel_fraction(tx * 2.0, ty * 2.0);
        let lift_px = if cue { 2.0 } else { 0.0 };
        crowd.push((
            ty * 2.0,
            2,
            frac,
            Box::new(move |cv| {
                super::super::arena::lift(lift_px);
                if lift_px > 0.0 {
                    super::super::arena::glow();
                }
                let (x, y) = at(tx * 2.0, ty * 2.0);
                cv.ellipse(x as f32, (y - height) as f32, 5.0, 4.0, ball_ink);
                cv.line(x - 2, y - height + 4, x - 3, y - height + 6, 'o');
                cv.line(x, y - height + 4, x, y - height + 7, 'o');
                cv.line(x + 2, y - height + 4, x + 3, y - height + 6, 'o');
                cv.line(x, y - height - 4, x, y - height - 2, 'n');
                cv.line(x - 1, y - height - 4, x - 2, y - height - 7, 'A');
                cv.line(x, y - height - 4, x, y - height - 8, 'M');
                cv.line(x + 1, y - height - 4, x + 2, y - height - 7, 'A');
                cv.put(x - 2, y - height - 1, 'H');
                ring(cv, x, y, 14.0, ring_ink);
            }),
        ));
    }
    if kind == RoomKind::Causeway {
        // Six hurdles stand along the riding lane at the lane's foot, so a
        // window leans them with the ground and a knight riding past reads
        // as passing in front. The lane itself stays in scenery.
        let current = active.filter(|r| r.game == Game::Hobbyhorse);
        let foot = pastimes::RIDING_LANE.1 * 2.0;
        for (i, &fence) in pastimes::HURDLES.iter().enumerate() {
            let is_next = current.is_some_and(|r| i == usize::from(r.progress));
            let is_cleared = current.is_some_and(|r| i < usize::from(r.progress));
            let ink = current.map_or('h', |r| {
                if i < usize::from(r.progress) {
                    'g'
                } else if i == usize::from(r.progress) {
                    'O'
                } else {
                    'h'
                }
            });
            let crossbar_ink = if is_next { 'Y' } else { ink };
            let frac = super::pixel_fraction(fence * 2.0, foot);
            let lift_px = if is_next { 2.0 } else { 0.0 };
            crowd.push((
                foot,
                2,
                frac,
                Box::new(move |cv| {
                    super::super::arena::lift(lift_px);
                    if lift_px > 0.0 {
                        super::super::arena::glow();
                    }
                    let (x, top) = at(fence * 2.0, pastimes::RIDING_LANE.0 * 2.0);
                    let (_, bottom) = at(fence * 2.0, pastimes::RIDING_LANE.1 * 2.0);
                    cv.rect(x - 8, top, 2, bottom - top, 'n');
                    cv.rect(x + 6, top, 2, bottom - top, 'n');
                    cv.rect(x - 8, (top + bottom) / 2 - 1, 16, 2, crossbar_ink);
                    cv.rect(x - 9, bottom, 4, 2, 'n');
                    cv.rect(x + 5, bottom, 4, 2, 'n');
                    cv.line(x - 6, bottom - 2, x - 2, (top + bottom) / 2, 'n');
                    cv.line(x + 6, bottom - 2, x + 2, (top + bottom) / 2, 'n');
                    let glyph_ink = if is_cleared { 'g' } else { crossbar_ink };
                    glyph1(cv, x - 2, top - 10, char::from(b'1' + i as u8), glyph_ink);
                }),
            ));
        }
    }
    if kind == RoomKind::Crag {
        // A Sled gate stands in its own row of the hill, alternating near
        // and far, so each is tilted and scaled by its own depth. The
        // slope and the sled itself stay flat (slope = floor, sled = rider).
        let current = active.filter(|r| r.game == Game::Sled);
        for (i, (gx, gy)) in pastimes::SLED_GATES.into_iter().enumerate() {
            let ink = if current.is_some_and(|r| usize::from(r.progress) == i) {
                '5'
            } else if current.is_some_and(|r| usize::from(r.progress) > i) {
                '2'
            } else {
                'z'
            };
            let depth = (gy * 2) as f32;
            let frac = super::pixel_fraction((gx * 2) as f32, depth);
            let is_next = current.is_some_and(|r| usize::from(r.progress) == i);
            let lift_px = if is_next { 3.0 } else { 0.0 };
            crowd.push((
                depth,
                2,
                frac,
                Box::new(move |cv| {
                    super::super::arena::lift(lift_px);
                    if lift_px > 0.0 {
                        super::super::arena::glow();
                    }
                    let (x, y) = at((gx * 2) as f32, (gy * 2) as f32);
                    for side in [-1, 1] {
                        for dx in -1..=1 {
                            cv.line(x + dx, y + side * TILE, x + dx, y + side * (TILE + 12), ink);
                        }
                        cv.rect(x + 2, y + side * (TILE + 10), 8, 4, ink);
                    }
                    super::super::chrome::inset_panel(
                        cv,
                        (x - 6, y - 5, 13, 11),
                        super::super::chrome::Tone::SLATE.rimmed(super::super::chrome::rim_of(ink)),
                    );
                    glyph1(cv, x - 2, y - 3, char::from(b'1' + i as u8), ink);
                }),
            ));
        }
    }
    if kind == RoomKind::Timberyard {
        // The four jump-logs stand at the lane's foot so a window leans
        // them and shrinks them with distance; the cart itself moves with
        // the rider and stays in `woodland::cart_figures`.
        let foot = 10.0 * 2.0;
        for (i, &log) in pastimes::CART_LOGS.iter().enumerate() {
            let x_px = log * TILE;
            let frac = super::pixel_fraction((log * 2) as f32, foot);
            crowd.push((
                foot,
                2,
                frac,
                Box::new(move |cv| {
                    for y in 4 * TILE..10 * TILE {
                        for (dx, ch) in [
                            (-3, 'P'),
                            (-2, 'r'),
                            (-1, 'o'),
                            (0, 'r'),
                            (1, 'B'),
                            (2, 'b'),
                            (3, 'n'),
                        ] {
                            let ch = if super::super::ink::hash(x_px + dx, y / 3, 160) % 7 == 0 {
                                'b'
                            } else {
                                ch
                            };
                            cv.put(x_px + dx, y, ch);
                        }
                        cv.put(x_px + 4, y + 1, 'K');
                    }
                    cv.ellipse(x_px as f32, (4 * TILE) as f32, 3.5, 2.0, 'o');
                    cv.put(x_px, 4 * TILE, 'B');
                    cv.text(x_px - 2, 10 * TILE + 5, &(i + 1).to_string(), 'O');
                }),
            ));
        }
    }
}

/// The arcade plaques, in a scrying glass rather than a sign: ink black
/// under a steel rim (`chrome::Tone::GLASS`), lit along the top by a single
/// diagonal specular streak so they read as frosted, not flat. Same 5x7 hand
/// as `home::board`, so text metrics stay put.
fn glass_board(lines: &[(String, char)]) -> Img {
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 10;
    let h = lines.len() as i32 * 9 + 5;
    let mut im = Img::new(w + 1, h + 1);
    super::super::chrome::panel(&mut im, (0, 0, w, h), super::super::chrome::Tone::GLASS);
    // The streak: a shallow diagonal across the upper-left of the panel,
    // one pixel thick, drawn in the tone's own steel so it never overlights
    // the letters that come after it.
    for x in 2..(w - 2).max(2) {
        let y = 1 + (x / 3).min(2);
        if y < h - 2 {
            im.put(x, y, 'u');
        }
    }
    for (i, (text, ink)) in lines.iter().enumerate() {
        let tw = text_width(text);
        im.text(
            (w - tw) / 2,
            3 + i as i32 * 9,
            text,
            super::super::chrome::dusk_mark(*ink),
        );
    }
    im
}

pub(super) fn boards(cv: &mut Img, run: &Run) {
    if run.room().kind == RoomKind::GooseGreen {
        return;
    }
    if super::mummers::travel_near(run) {
        return;
    }
    // The race circuit has its own controls, spectator card and quest board.
    if kind_is_hippodrome(run) {
        return;
    }
    if run.room().kind == RoomKind::GooseGreen {
        return;
    }
    let kind = run.room().kind;
    // The ledgers share a corner: survey while idle, game while playing.
    if run.pastimes.round.is_none() && super::countryside::survey_near(run).is_some() {
        return;
    }
    if super::eastern::board_visible(run) {
        return;
    }
    if super::aerie::board_visible(run) {
        return;
    }
    if super::wetland::board_visible(run) {
        return;
    }
    if super::coastal::board_visible(run) {
        return;
    }
    if super::northern::board_visible(run) {
        return;
    }
    if !Game::ALL.iter().any(|g| g.room() == kind) {
        return;
    }
    let playing = run.pastimes.round.as_ref().map(|r| r.game);
    if let Some(r) = run
        .pastimes
        .round
        .as_ref()
        .filter(|r| r.game == Game::Volley)
    {
        if let Some(v) = &r.volley {
            // One top strip, never a four-line board across the serve lane.
            // The bell order lives on the court; the HUD only keeps score.
            let x = 4;
            let y = 13;
            let w = 244;
            super::super::chrome::panel(cv, (x, y, w, 15), super::super::chrome::Tone::SLATE);
            cv.text(
                x + 5,
                y + 4,
                &format!("BELL {}/4  {}S", r.progress, r.left.div_ceil(HZ)),
                '9',
            );
            cv.text(x + 99, y + 4, &format!("RALLY {}", v.best_combo), '6');
            cv.text(x + 174, y + 4, &format!("LIVES {}", v.lives), 'O');
            return;
        }
    }
    // The intro screen: controls and objective, shown while players are invulnerable.
    if let Some(round) = &run.pastimes.round {
        if round.intro_ticks > 0 {
            let board = intro_board(round.game);
            let x = (cv.w - board.w) / 2;
            let y = (cv.h - board.h) / 2;
            cv.stamp(&board, x, y);
            return;
        }
    }
    for (n, game) in Game::ALL
        .into_iter()
        .filter(|g| g.room() == kind)
        .enumerate()
    {
        // While a game is on, only its own board shows.
        if playing.is_some_and(|g| g != game) {
            continue;
        }
        let record = &run.home.countryside.games;
        let active = run.pastimes.round.as_ref().filter(|r| r.game == game);
        // Games played under the top board keep it to three lines.
        let up_top = matches!(
            game,
            Game::Sled
                | Game::Pot
                | Game::Cart
                | Game::Surf
                | Game::Halfpipe
                | Game::Powder
                | Game::Gander
                | Game::Peal
        );
        let board = match active {
            None => glass_board(&[
                (game.name().into(), '9'),
                (format!("STEP ON {}", pad_letter(game)), 'O'),
                (format!("BEST {}", record.best_for(game)), 'h'),
            ]),
            Some(r) => {
                let score = format!("{} / {}", r.progress, game.need());
                let mut lines = vec![
                    (format!("{}  {}S", game.name(), r.left.div_ceil(HZ)), '9'),
                    (
                        if run.crossroads.is_some() {
                            if game == Game::Powder {
                                "MOVE   F DROP"
                            } else {
                                "MOVE   F FLAP"
                            }
                        } else {
                            how(game)
                        }
                        .into(),
                        'O',
                    ),
                ];
                match live_cue(game, r) {
                    Some(cue) if up_top => lines.push((format!("{score}  {cue}"), '9')),
                    Some(cue) => lines.extend([(score, '9'), (cue, '6')]),
                    None => lines.push((score, '9')),
                }
                glass_board(&lines)
            }
        };
        let x = if n == 0 { 4 } else { cv.w - board.w - 4 };
        let y = if up_top {
            13
        } else {
            cv.h - board.h - super::tray::max_height(run, cv.w - 8) - 16
        };
        cv.stamp(&board, x, y);
        // Floating score popup: rises and fades over 8 ticks.
        if let Some(r) = active.filter(|r| {
            r.score_popup > 0 && !r.score_popup_text.is_empty() && r.game != Game::Volley
        }) {
            let popup = r.score_popup;
            let rise = (8 - popup) as i32 * 2;
            let px = x + board.w / 2 - (r.score_popup_text.len() as i32 * 3);
            let py = y - 8 - rise;
            // Bright ink for first 4 ticks, then fading to grey.
            let ink = if popup > 4 {
                'O'
            } else if popup > 2 {
                'Y'
            } else {
                'g'
            };
            cv.text(px, py, &r.score_popup_text, ink);
        }
    }
}

/// The letter on a game's start ring.
fn pad_letter(game: Game) -> char {
    match game {
        Game::Broadsheet => 'P',
        Game::Barrel => 'B',
        Game::Turnip => 'T',
        Game::Washtub => 'W',
        Game::Cheese => 'C',
        Game::Hobbyhorse => 'H',
        Game::Sled => 'S',
        Game::Pot => 'P',
        Game::Cart => 'X',
        Game::Surf => 'U',
        Game::Volley => 'V',
        Game::Halfpipe => 'J',
        Game::Chariot => 'C',
        Game::Trencher => 'D',
        Game::DuckShoot => 'K',
        Game::Longbow => 'A',
        Game::Powder => 'K',
        Game::Gander => 'G',
        // S for the Siege Trail; G stays Gander Joust's.
        Game::Siege => 'S',
        Game::Peal => 'R',
    }
}

/// How to play, in two or three words.
fn how(game: Game) -> &'static str {
    match game {
        Game::Broadsheet => "POST GOLD BOXES",
        Game::Barrel => "RIDE THE FLAGS",
        Game::Turnip => "KICK ON LANDING",
        Game::Washtub => "PADDLE THE RINGS",
        Game::Cheese => "LOB AT GOATS",
        Game::Hobbyhorse => "JUMP THE HURDLES",
        Game::Sled => "STEER THE GATES",
        Game::Pot => "HOLD, THEN THROW",
        Game::Cart => "STEER, HOP LOGS",
        Game::Surf => "LEAN, CATCH GOLD",
        Game::Volley => "F SWING · ARROWS AIM",
        Game::Halfpipe => "PUMP, THEN TRICK",
        Game::Chariot => "RACE THREE LAPS",
        Game::Trencher => "TOSS TO GOOSE",
        Game::DuckShoot => "AIM AND SHOOT",
        Game::Longbow => "WALK, DRAW, LOOSE",
        Game::Powder => "DROP KEGS · LAST STANDS",
        Game::Gander => "FLAP, LAND ON TOP",
        Game::Siege => "AIM, LOB, SURVIVE",
        Game::Peal => "RING ON GOLD",
    }
}

/// The intro screen: a title, the goal, the controls, and a "press F" hint.
fn intro_board(game: Game) -> Img {
    let (goal, keys): (&str, [&str; 3]) = match game {
        Game::Broadsheet => (
            "Deliver papers to the gold boxes",
            ["Move to a box", "F: drop the paper", "Dodge the goose"],
        ),
        Game::Barrel => (
            "Ride the flags in order",
            ["Move to steer", "F: jump obstacles", "Flags 1 to 6"],
        ),
        Game::Turnip => (
            "Keep the turnip aloft",
            ["Stay under it", "F: kick on landing", "Time the kick"],
        ),
        Game::Washtub => (
            "Paddle through the rings",
            ["Move to steer", "F: paddle stroke", "Rings 1 to 6"],
        ),
        Game::Cheese => (
            "Lob cheese at the goats",
            ["Move to aim", "F: throw (hold=power)", "Hit the goat"],
        ),
        Game::Hobbyhorse => (
            "Jump every hurdle",
            ["Move to ride", "F: jump", "Three misses ends it"],
        ),
        Game::Sled => (
            "Steer through the gates",
            ["Move left/right", "Speed builds alone", "All six gates"],
        ),
        Game::Pot => (
            "Put the pot on the mark",
            [
                "Move: set the angle",
                "Hold F, then loose",
                "Land on target",
            ],
        ),
        Game::Cart => (
            "Wheel the cart across",
            ["Move to steer", "F: hop the logs", "Stay upright"],
        ),
        Game::Surf => (
            "Ride the gold crest",
            ["Move to lean", "F at the crest", "Mind the balance"],
        ),
        Game::Volley => (
            "Ring the four bells in order",
            [
                "Move, then tap F to swing",
                "Perfect timing: more speed",
                "Arrows aim; a miss keeps bells",
            ],
        ),
        Game::Halfpipe => (
            "Land a trick on the lip",
            ["Move to pump", "F at the lip", "Land three tricks"],
        ),
        Game::Chariot => (
            "Race the laps home",
            ["Move to steer", "F: throw cabbage", "First over wins"],
        ),
        Game::Trencher => (
            "Toss plates to the goose",
            ["Move to aim", "F: throw", "The goose must catch"],
        ),
        Game::DuckShoot => (
            "Shoot the flying ducks",
            ["Move to aim", "F: fire", "Far ducks count double"],
        ),
        Game::Longbow => (
            "Shoot the popinjay",
            [
                "Move to aim",
                "Hold F to draw, loose",
                "Full draw flies true",
            ],
        ),
        Game::Powder => (
            "Drop kegs, last one stands",
            ["Move to position", "F: drop a keg", "Outlast the yard"],
        ),
        Game::Gander => (
            "Joust — stay on top",
            ["Move to position", "F: flap up", "Unhorse your rivals"],
        ),
        Game::Siege => (
            "Breach the far castle",
            ["Move to aim", "F: loose a shot", "Five legs to win"],
        ),
        Game::Peal => ("Ring ten strikes true", ["Stand at the rope", "F: pull on gold", "Early is a clanger"]),
    };
    let mut lines = vec![
        (game.name().to_string(), '9'),
        (goal.to_string(), 'O'),
        ("F: to play".to_string(), '6'),
    ];
    for (k, line) in keys.iter().enumerate() {
        lines.push((format!("  {}. {}", k + 1, line), 'h'));
    }
    lines.push(("Press F to start".to_string(), 'O'));
    glass_board(&lines)
}

/// The one thing to do right now, where a game has a moment.
fn live_cue(game: Game, r: &crate::rules::pastimes::Round) -> Option<String> {
    match game {
        Game::Surf => {
            let s = r.surf.as_ref()?;
            (s.phase >= pastimes::WAVE_TICKS - pastimes::CREST_WINDOW).then(|| "F NOW!".to_string())
        }
        Game::Peal => {
            let p = r.peal.as_ref()?;
            if p.rung {
                Some("RUNG TRUE".into())
            } else if p.spoiled {
                Some("CLANGER".into())
            } else {
                p.in_window().then(|| "F NOW!".to_string())
            }
        }
        Game::Pot => {
            let p = r.put.as_ref()?;
            Some(format!("POWER {}%", p.charge * 100 / HZ))
        }
        Game::Volley => {
            let v = r.volley.as_ref()?;
            let bell = v.next_bell().map_or(4, |i| i + 1);
            Some(format!(
                "NEXT BELL {}  TURNIPS {}  BEST RALLY {}",
                bell, v.lives, v.best_combo
            ))
        }
        Game::Halfpipe => Some(format!("HELMETS {}", 3u8.saturating_sub(r.mistakes))),
        Game::DuckShoot => Some("FAR DUCK: TWO".into()),
        Game::Gander => Some(format!("TAR SPLASHES {}", r.mistakes)),
        Game::Longbow => {
            let b = r.longbow.as_ref()?;
            Some(if b.draw >= pastimes::LONGBOW_FULL_DRAW {
                "FULL DRAW!".into()
            } else {
                format!(
                    "ARROWS {}",
                    pastimes::LONGBOW_ARROWS.saturating_sub(b.arrows)
                )
            })
        }
        Game::Powder => {
            let p = r.powder.as_ref()?;
            Some(format!("KEGS LEFT {}", p.bales.len()))
        }
        Game::Siege => {
            let s = r.siege.as_ref()?;
            if !s.fighting {
                Some(format!("MEN {}", s.soldiers))
            } else if s.reload > 0 {
                Some("RELOADING...".into())
            } else {
                Some(format!("WIND {}", s.wind))
            }
        }
        _ => None,
    }
}

fn kind_is_hippodrome(run: &Run) -> bool {
    run.room().kind == RoomKind::Hippodrome
}

/// The Gander Joust hall, side-on: stone ledges with moss and torches over
/// a tar pit, drawn in the realm inks the way the other side-on halls are
/// (rock for stone, coursed faces, torchlight). The riders are twice their
/// old size so a player reads them at a glance; each knight in their own
/// colour, a wing frame on every flap, the lance tipping down when diving.
/// Unhorsed riders drop an egg that rolls on the ledge; the tar's hand
/// rises for a low rider, in four frames.
fn gander_hall(cv: &mut Img, run: &Run) {
    use crate::rules::pastimes::{GANDER_LEDGES as LEDGES, GANDER_TAR_ROW as TAR_ROW};
    // The ledges: stone in the side-on kit — a lit top, a coursed face
    // below with joints half drawn, moss along the lip (foliage bank).
    for &(x0, x1, row) in LEDGES.iter() {
        let (px, w, py) = (x0 * TILE, (x1 - x0 + 1) * TILE, row * TILE);
        cv.rect(px, py, w, TILE, 'j');
        for k in 0..TILE {
            for x in px..px + w {
                let course = k / 4;
                let lx = (x + if course % 2 == 0 { 0 } else { 4 }).rem_euclid(8);
                if (k % 4 == 0 || lx == 0) && (x * 7 + k) % 3 != 0 {
                    cv.put(x, py + TILE + k, 'g');
                } else {
                    cv.put(x, py + TILE + k, 'X');
                }
            }
        }
        for x in (px + 2..px + w - 2).step_by(3) {
            if (x + row) % 4 != 0 {
                cv.put(x, py - 1, 'A');
                cv.put(x + 1, py - 1, 'y');
                if (x + row) % 8 < 4 {
                    cv.put(x, py, 'E');
                }
            }
        }
    }
    // The tar pit: black glass with a slow skin, wall to wall below the
    // ledges.
    for row in TAR_ROW..(run.room().rows as i32 - 1) {
        cv.rect(
            TILE,
            row * TILE,
            22 * TILE,
            TILE,
            if row == TAR_ROW { 'K' } else { 'k' },
        );
    }
    for n in 0..7i32 {
        let x = (2 + n * 3) * TILE + 4;
        if ((run.tick / 12) as i32 + n) % 4 == 0 {
            cv.put(x, TAR_ROW * TILE + 4, 'K');
            cv.put(x + 1, TAR_ROW * TILE + 4, 'g');
        }
    }
    // Torches on the back wall, above the ledges — the side-on kit's own.
    let tick = run.tick as u32;
    for x in [4, 12, 19] {
        let (tx, ty) = (x * TILE + 6, 3 * TILE);
        cv.stamp(
            &crate::render::overworld::kit::torch(tick / 4, x as u32),
            tx,
            ty,
        );
    }
    let Some(round) = run
        .pastimes
        .round
        .as_ref()
        .filter(|r| r.game == Game::Gander)
    else {
        return;
    };
    let Some(g) = round.gander.as_ref() else {
        return;
    };
    let count = g.tally.len();
    // The eggs: white, rolling on their ledge; a crack as hatching nears.
    for e in &g.eggs {
        let (x, y) = (e.x * TILE / 256, e.row * TILE);
        cv.ellipse(x as f32, y as f32 - 2.0, 3.0, 3.0, 'i');
        cv.put(x, y - 4, 'H');
        if e.left < 60 {
            cv.put(x - 1, y - 2, 'K');
        }
    }
    for (i, rider) in g.riders.iter().enumerate() {
        if rider.down > 0 && i >= count {
            // A felled rival: rings on the tar where it went in.
            let (x, y) = (rider.x * TILE / 256, TAR_ROW * TILE + 4);
            for r in [3, 6, 9] {
                ring(cv, x, y, r as f32, 'K');
            }
            continue;
        }
        // Twice the old size: the goose reads at a glance.
        let (x, y) = (rider.x * TILE / 256, rider.y * TILE / 256);
        let face = rider.face as i32;
        let wing = (run.tick as i32 / 4 + i as i32 * 3) % 6 < 3;
        let tumbling = rider.stun > 0;
        let knight = i < count;
        // Each knight in their own colour — the same ink the standings
        // board uses for their seat. Rivals stay crow-dark.
        let trim = if knight {
            super::knights::seat_mark(i as u32 + 1)
        } else {
            'u'
        };
        let (body, bill) = if knight { ('h', 'O') } else { ('v', 'p') };
        if !tumbling {
            cv.ellipse(x as f32, y as f32 - 8.0, 13.0, 8.0, body);
            cv.ellipse(
                x as f32,
                y as f32 - 10.0,
                9.0,
                4.0,
                if knight { '9' } else { 'u' },
            );
        } else {
            cv.ellipse(x as f32, y as f32 - 8.0, 13.0, 8.0, body);
        }
        let (nx, ny) = (x + face * 10, y - 20);
        cv.line(x + face * 8, y - 12, nx, ny, trim);
        cv.rect(nx - 2, ny - 4, 5, 6, trim);
        cv.line(nx + face * 3, ny, nx + face * 10, ny + 2, bill);
        cv.put(nx + face * 3, ny - 2, 'k');
        // The wings: a frame on every flap, up then down.
        let spread = if wing && !tumbling { -8 } else { 6 };
        cv.line(x - face * 4, y - 12, x - face * 14, y - 12 + spread, trim);
        cv.line(x + face * 2, y - 12, x + face * 12, y - 12 + spread, trim);
        let helm = if knight { trim } else { 'X' };
        if !tumbling {
            cv.rect(x - 6, y - 28, 11, 10, helm);
            cv.put(x + face * 4, y - 24, 'k');
            // The lance: level on the cruise, tipping down on the dive.
            let tip = if rider.dy > 32 { 8 } else { 2 };
            cv.line(x + face * 10, y - 24, x + face * 24, y - 24 + tip, 'o');
            cv.put(x + face * 24, y - 24 + tip, 'u');
        } else {
            cv.rect(x - 6, y - 36, 11, 10, helm);
        }
    }
    // The tar's hand: rising for a low rider, four frames — knuckles,
    // wrist, spread fingers, then closed on the rider and sinking.
    if let Some(hand) = &g.hand {
        let (x, base) = (hand.x * TILE / 256, TAR_ROW * TILE);
        match hand.frame {
            1 => {
                // Knuckles just breaking the skin.
                cv.ellipse(x as f32, base as f32 + 1.0, 7.0, 3.0, 'K');
                for k in -3..=3 {
                    cv.put(x + k * 2, base - k.abs() / 2, 'g');
                }
            }
            2 => {
                // The wrist: a broad dark mass with knuckle ridges.
                cv.ellipse(x as f32, base as f32 - 3.0, 8.0, 6.0, 'K');
                for k in -2i32..=2 {
                    cv.put(x + k * 3, base - 8, 'X');
                    cv.put(x + k * 3, base - 7, 'g');
                }
            }
            3 | 4 => {
                // Fingers spread wide (then curling), the forearm behind.
                for h in 0..16 {
                    cv.rect(x - 3, base - h, 6, 1, 'k');
                }
                let finger = if hand.frame == 4 { 8 } else { 14 };
                for k in -2i32..=2 {
                    cv.line(x + k * 4, base - 4, x + k * 3, base - finger, 'K');
                    cv.put(x + k * 3, base - finger, 'g');
                }
                cv.ellipse(x as f32, base as f32 - 8.0, 10.0, 5.0, 'K');
            }
            _ => {}
        }
    }
}
