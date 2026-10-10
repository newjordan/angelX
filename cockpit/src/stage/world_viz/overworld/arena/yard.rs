//! The Training Yard, drawn: a straw-strewn practice hall north of the
//! Undercroft, quintains to strike (each shows the damage it took in the
//! last second), archery butts and blade racks on the wall, and Grubbins'
//! stall — shut, with a sign, until a goblin has got away and come back.

use super::super::ink::{Img, hash, text_width};
use super::super::kit::{self, RockKind, Tiles};
use super::{TILE, at, cards, dusk, fire, flags, home, stand};
use crate::drive::together_shooter::yard::{
    GRUBBINS_AT, QUINTAINS, STALL, STALL_PLATES, stall_plate, stall_price,
};
use crate::drive::together_shooter::{EnemyKind, Run, Tile};
use crate::stage::world_viz::overworld::light::Light;

/// The yard at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let tick = run.tick as u32;
    let tiles = Tiles::get();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    for y in 0..ph {
        for x in 0..pw {
            if !matches!(room.tile(x / TILE, y / TILE), Tile::Floor | Tile::Door) {
                continue;
            }
            // Straw on the flags.
            let ink = if hash(x / 2, y, 131).is_multiple_of(90) {
                Some(if (x + y) % 3 == 0 { 'O' } else { 'o' })
            } else {
                flags(x, y)
            };
            if let Some(ch) = ink {
                cv.put(x, y, ch);
            }
        }
    }
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) != Tile::Wall {
                continue;
            }
            let (x, y) = (col * TILE, row * TILE);
            let corner = (col == 0 || col == room.cols as i32 - 1)
                && (row == 0 || row == room.rows as i32 - 1);
            if corner {
                let im = kit::brazier(tick / 4, (col * 5 + row) as u32);
                cv.stamp(&im, x + (TILE - im.w) / 2, y + TILE - im.h);
                lights.push(fire(x + TILE / 2, y + 4, 56.0, 0.32));
            } else {
                cv.stamp(tiles.rock(RockKind::Slate, hash(col, row, 13)), x, y);
            }
        }
    }
    // Archery butts and blade racks on the north wall.
    for bx in [56, 152, 232, 328] {
        butt(&mut cv, bx, 2);
    }
    for rx in [96, 280] {
        let rack = kit::blade_rack(3);
        cv.stamp(&rack, rx, 4);
    }
    // The way back down, with torches.
    for (k, tx) in [(0, 166), (1, 214)] {
        let im = kit::torch(tick / 4, k);
        cv.stamp(&im, tx, ph - 13);
        lights.push(fire(tx + 2, ph - 12, 36.0, 0.3));
    }
    stall(&mut cv, run.home.goblins > 0);
    if run.home.goblins > 0 {
        for plate in STALL_PLATES {
            slab(&mut cv, plate, false);
        }
    }
    let (sx, sy) = stall_middle();
    lights.push(fire(sx, sy, 44.0, 0.3));
    super::lessons::furniture(&mut cv);
    dusk(&mut cv, &lights, ('x', 'I'));
    cv
}

/// An archery butt: straw rings on a stand.
fn butt(cv: &mut Img, x: i32, y: i32) {
    for (r, ink) in [(6.0, 'o'), (4.5, 'B'), (3.0, 'O'), (1.5, 'r')] {
        cv.ellipse(x as f32 + 6.0, y as f32 + 6.0, r, r, ink);
    }
    cv.line(x + 2, y + 12, x + 10, y + 12, 'b');
}

fn stall_middle() -> (i32, i32) {
    let (c, r, w, h) = STALL;
    ((c * 2 + w) * TILE / 2, (r * 2 + h) * TILE / 2)
}

/// Grubbins' stall: a timber counter under a striped awning. Shut with a
/// sign until a goblin has come back to keep it.
fn stall(cv: &mut Img, open: bool) {
    let (c, r, w, h) = STALL;
    let (x, y, pw, ph) = (c * TILE, r * TILE, w * TILE, h * TILE);
    // The awning, striped.
    for yy in y - 10..y + 2 {
        for xx in x - 2..x + pw + 2 {
            let stripe = ((xx - x).div_euclid(6)) % 2 == 0;
            cv.put(xx, yy, if stripe { 'p' } else { 'T' });
        }
    }
    for xx in (x - 2..x + pw + 2).step_by(6) {
        cv.put(xx + 2, y + 2, 'p');
        cv.put(xx + 3, y + 3, 'p');
    }
    cv.rect(x, y + 4, pw, ph - 4, 'P');
    cv.frame(x, y + 4, pw, ph - 4, 'b');
    cv.line(x + 1, y + 4, x + pw - 2, y + 4, 'O');
    for lx in [x + 1, x + pw - 2] {
        cv.line(lx, y - 10, lx, y + ph - 1, 'b');
    }
    if !open {
        let sign = kit::district_sign("CLOSED");
        cv.stamp(&sign, x + (pw - sign.w) / 2, y + 10);
    }
}

/// A plate before the stall, bright while a knight stands on it.
fn slab(cv: &mut Img, (c, r, w, h): (i32, i32, i32, i32), lit: bool) {
    let (x, y, pw, ph) = (c * TILE + 1, r * TILE + 1, w * TILE - 2, h * TILE - 2);
    cv.rect(x, y, pw, ph, 'X');
    cv.frame(x, y, pw, ph, if lit { '5' } else { 'o' });
    let coin = if lit { '6' } else { 'O' };
    cv.ellipse((x + pw / 2) as f32, (y + ph / 2) as f32, 2.5, 2.5, coin);
}

/// The live yard: Grubbins and his wares, each quintain's count, and the
/// plates knights stand on.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    if run.home.goblins > 0 {
        let (gx, gy) = at(GRUBBINS_AT.0 * 2.0, GRUBBINS_AT.1 * 2.0);
        let near = run.players.values().any(|h| {
            h.hp > 0 && (h.x - GRUBBINS_AT.0 * 2.0).hypot(h.y - GRUBBINS_AT.1 * 2.0) < 5.0
        });
        let im = Img::from_rows(if near && (tick / 8).is_multiple_of(2) {
            &GRUBBINS_WAVE
        } else {
            &GRUBBINS
        });
        stand(cv, &im, gx, gy);
        // His wares on the counter, one above each plate.
        for (i, plate) in STALL_PLATES.iter().enumerate() {
            let Some(id) = run.stall.get(i).filter(|c| !c.is_empty()) else {
                continue;
            };
            let card = run.book.get(id);
            let im = cards::floor_card(card, tick);
            let (c, r, w, _) = *plate;
            cv.stamp(&im, c * TILE + w * TILE / 2 - im.w / 2, r * TILE - im.h + 2);
        }
        for hero in run.players.values().filter(|h| h.hp > 0) {
            if let Some(item) = stall_plate(hero.x, hero.y) {
                slab(cv, STALL_PLATES[item], true);
            }
        }
    }
    // Each quintain's count: the damage it took in the last second.
    for dummy in run.enemies.iter().filter(|e| e.kind == EnemyKind::Dummy) {
        let dps = dummy.dir.1.round() as u32;
        if dps == 0 {
            continue;
        }
        let (x, y) = at(dummy.x, dummy.y);
        let text = dps.to_string();
        let w = text_width(&text);
        cv.text(x - w / 2, y - 36, &text, '6');
    }
    let _ = QUINTAINS;
}

/// Over everything: the ware a knight is reading.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    if run.home.goblins == 0 {
        return;
    }
    let reading = run
        .players
        .iter()
        .filter(|(_, h)| h.hp > 0)
        .find_map(|(&id, h)| stall_plate(h.x, h.y).map(|i| (id, i)));
    let Some((id, item)) = reading else {
        return;
    };
    let Some(card) = run
        .stall
        .get(item)
        .filter(|c| !c.is_empty())
        .and_then(|c| run.book.get(c))
    else {
        let board = home::board(&[("SOLD".to_string(), '9'), ("NO REFUNDS".to_string(), 'h')]);
        let (c, r, _, _) = STALL_PLATES[item];
        cv.stamp(
            &board,
            (c * TILE - board.w / 2).max(2),
            r * TILE - board.h - 22,
        );
        return;
    };
    let price = stall_price(card);
    let have = run.treasury.get(crate::drive::together_realm::Spoil::Gold);
    let buying = run.players.get(&id).map_or(0, |h| h.buying);
    let rules = card.rules().to_uppercase();
    let mut lines = vec![(card.name.to_uppercase(), '9')];
    for chunk in rules.split(" · ").take(2) {
        lines.push((chunk.chars().take(34).collect(), 'h'));
    }
    lines.push((
        format!("{price} GOLD"),
        if have >= price { 'H' } else { '7' },
    ));
    // Room for HOLD F and its bar.
    lines.push(("                ".to_string(), 'h'));
    let mut board = home::board(&lines);
    let label = "HOLD F";
    let bar_w = 40;
    let row = 3 + (lines.len() as i32 - 1) * 9;
    let start = (board.w - (text_width(label) + 4 + bar_w)) / 2;
    let after = board.text(start, row, label, 'O');
    board.frame(after + 4, row + 1, bar_w, 5, 'b');
    let bh = crate::drive::together_shooter::home::BUY_HOLD;
    let fill = (buying.min(bh) * (bar_w as u32 - 2) / bh) as i32;
    if fill > 0 {
        board.rect(after + 5, row + 2, fill, 3, '5');
    }
    // Beside the stall, so Grubbins stays in sight.
    let (_, r, _, _) = STALL_PLATES[item];
    let x = (STALL.0 * TILE - board.w - 8).max(2);
    cv.stamp(&board, x, (r * TILE - board.h).max(2));
}

/// Grubbins: a goblin in a feathered hat and a waistcoat he insists he
/// bought, a purse at his belt.
const GRUBBINS: [&str; 15] = [
    ".....T........",
    "....pT........",
    "...pppp.......",
    "..pppppp......",
    ".MMMMMMMm.....",
    "MmK7MK7Mm.....",
    ".MMMMMMM......",
    "..MyyyM.......",
    ".qqMMMqq......",
    "mqQqqqQqm.....",
    "m.qqQqq.m5....",
    "..qqqqq.a5....",
    "..MM.MM.......",
    ".mM...Mm......",
    ".bb...bb......",
];

const GRUBBINS_WAVE: [&str; 15] = [
    ".....T........",
    "....pT....m...",
    "...pppp...m...",
    "..pppppp..m...",
    ".MMMMMMMm.m...",
    "MmK7MK7Mmm....",
    ".MMMMMMM......",
    "..MyyyM.......",
    ".qqMMMqq......",
    "mqQqqqQq......",
    "m.qqQqq..5....",
    "..qqqqq.a5....",
    "..MM.MM.......",
    ".mM...Mm......",
    ".bb...bb......",
];
