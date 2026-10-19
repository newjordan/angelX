//! Cached timber, living hedge walls, and a badger cook. Only the cart moves.
use super::super::ink::{Img, hash};
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{
    RoomKind, Run,
    pastimes::{self, Game},
    woodland as wood,
};

fn ring(cv: &mut Img, x: i32, y: i32, ink: char) {
    for n in 0..40 {
        let a = n as f32 * std::f32::consts::TAU / 40.0;
        cv.put(
            x + (a.cos() * 11.0) as i32,
            y + (a.sin() * 11.0) as i32,
            ink,
        );
    }
}
pub(super) fn scenery(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::Timberyard => {
            // Log stacks: ringed ends toward the viewer, bark on top.
            for (k, (c, r)) in [(3, 1), (16, 1), (3, 11), (16, 11)].into_iter().enumerate() {
                let (x, y) = (c * TILE + 2, r * TILE + 2);
                let keep = |_: i32, _: i32| true;
                m::shadow(cv, ((x + 5 * TILE - 8) as f32, (y + 2 * TILE - 2) as f32), (30.0, 4.0), &keep);
                for row in 0..3 {
                    let ly = y + 2 * TILE - 8 - row * 8;
                    for n in 0..(8 - row) {
                        let lx = x + 4 + row * 4 + n * 9;
                        if lx + 4 > x + 5 * TILE - 4 {
                            continue;
                        }
                        cv.ellipse(lx as f32, ly as f32, 4.5, 4.0, 'b');
                        cv.ellipse(lx as f32, ly as f32, 3.5, 3.0, 'o');
                        cv.ellipse(lx as f32, ly as f32, 2.0, 1.5, 'r');
                        cv.put(lx, ly, 'B');
                        cv.put(lx - 2, ly - 2, 'O');
                    }
                }
                let _ = k;
                m::sign(cv, x + 22, y - 12, "LOGS", 'T');
            }
            // The grass lane has one full-width log at each numbered jump.
            for (i, log) in pastimes::CART_LOGS.iter().enumerate() {
                let x = log * TILE;
                for y in 4 * TILE..10 * TILE {
                    for (dx, ch) in [(-3, 'P'), (-2, 'r'), (-1, 'o'), (0, 'r'), (1, 'B'), (2, 'b'), (3, 'n')] {
                        let ch = if hash(x + dx, y / 3, 160) % 7 == 0 { 'b' } else { ch };
                        cv.put(x + dx, y, ch);
                    }
                    cv.put(x + 4, y + 1, 'K');
                }
                cv.ellipse(x as f32, (4 * TILE) as f32, 3.5, 2.0, 'o');
                cv.put(x, 4 * TILE, 'B');
                cv.text(x - 2, 10 * TILE + 5, &(i + 1).to_string(), 'O');
            }
            for (x, y) in pastimes::CART_RUTS {
                let (x, y) = (x * TILE, y * TILE);
                cv.ellipse(x as f32, y as f32, 6.5, 3.0, 'n');
                cv.ellipse(x as f32, (y + 1) as f32, 5.0, 2.0, 'q');
                cv.line(x - 4, y - 2, x + 3, y - 2, 'B');
                cv.put(x - 2, y + 1, 'Q');
            }
        }
        RoomKind::HedgeMaze => {
            for (k, (c, r, w, h)) in wood::HEDGES.into_iter().enumerate() {
                m::hedge(cv, (c as i32 * TILE, r as i32 * TILE - 3, w as i32 * TILE, h as i32 * TILE + 2), k as u32);
                for n in 0..(w * h) as i32 {
                    let v = hash(k as i32, n, 161);
                    if v % 3 == 0 {
                        cv.put(c as i32 * TILE + (v >> 4) as i32 % (w as i32 * TILE), r as i32 * TILE - 2 + (v >> 12) as i32 % (h as i32 * TILE - 6), 'T');
                    }
                }
            }
        }
        RoomKind::Bothy => {
            let (x, y) = (3 * TILE, 2 * TILE);
            m::house(cv, (x + 1, y - 12, 5 * TILE - 2, 3 * TILE + 10), &m::House { roof: m::ramp::THATCH, stone: true, windows: 1 }, 162);
            // A chimney with a thread of smoke, and the stew pot outside.
            m::sign(cv, x + 6, y + 3 * TILE + 4, "BADGER", 'T');
            m::barrel(cv, x + 5 * TILE + 8, y + 3 * TILE - 2);
        }
        _ => {}
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    for e in crate::rules::world::ENTRANCES
        .iter()
        .filter(|e| e.room == run.room().kind && e.id.ends_with("wagon"))
    {
        let (c, r, w, h) = e.at;
        let ink = if run.home.countryside.has(8) {
            'o'
        } else {
            'B'
        };
        let (x, y) = (c * TILE + w * TILE / 2, r * TILE + h * TILE / 2);
        ring(cv, x, y, ink);
        cv.rect(x - 9, y - 5, 18, 8, 'o');
        cv.line(x - 9, y - 8, x + 9, y - 8, ink);
        for dx in [-6, 6] {
            cv.rect(x + dx - 2, y + 3, 5, 5, 'b');
            cv.put(x + dx, y + 5, 'h');
        }
        cv.text(x - 12, y + 15, "WAGON", ink);
    }
    let stage = wood::stage(&run.home.countryside, &run.marks);
    let kind = run.room().kind;
    if kind == RoomKind::HedgeMaze && !run.marks.contains_key(wood::ACORN_MARK) {
        // The golden acorn, glinting, behind the long hedge.
        let (x, y) = at(wood::ACORN.0 * 2.0, wood::ACORN.1 * 2.0);
        let glint = run.tick / 10 % 3 == 0;
        for (dy, row) in ["..bb..", ".bbbb.", "666666", "655556", "655556", ".6556.", "..66.."].iter().enumerate() {
            for (dx, c) in row.chars().enumerate() {
                if c != '.' {
                    cv.put(x - 3 + dx as i32, y - 9 + dy as i32, c);
                }
            }
        }
        if glint {
            cv.put(x + 3, y - 10, 'w');
            cv.put(x + 4, y - 11, 'w');
        }
        super::near_prompt(cv, run, (x, y), "TAKE");
    }
    let foraging = (1..5).contains(&stage);
    let already = usize::from(stage.saturating_sub(1)).min(3);
    if kind == RoomKind::HedgeMaze && foraging {
        let carried = run.forage.as_ref().map_or([0; 3], |f| f.carried);
        for i in already..3 {
            if carried[i] != 0 {
                continue;
            }
            let (_, (x, y)) = wood::ERRAND[1 + i];
            let (x, y) = at(x * 2.0, y * 2.0);
            // Growing: a little sway, so they read as alive.
            let sway = [0, 1, 0, -1][(run.tick / 9 % 4) as usize];
            mushroom(cv, x + sway, y, i);
            cv.text(x - super::super::ink::text_width(wood::MUSHROOMS[i]) / 2, y + 6, wood::MUSHROOMS[i], 'h');
        }
    }
    if foraging {
        // Carried over the picker's head, all the way home.
        if let Some(f) = &run.forage {
            for (i, &id) in f.carried.iter().enumerate() {
                if let Some(h) = run.players.get(&id).filter(|_| id != 0) {
                    let (x, y) = at(h.x, h.y);
                    let stack = f.carried[..i].iter().filter(|&&c| c == id).count() as i32;
                    mushroom(cv, x + stack * 9, y - 24, i);
                }
            }
        }
    }
    if kind == RoomKind::Bothy {
        let (x, y) = at(10.0, 17.0);
        ring(cv, x, y, if stage == 0 || stage == 4 { 'o' } else { 'g' });
        // Striped badger, hat and soup kettle beside the actual F ring.
        cv.rect(x - 6, y - 9, 12, 6, 'h');
        cv.rect(x - 3, y - 9, 3, 5, 'b');
        cv.rect(x + 2, y - 9, 3, 5, 'b');
        cv.rect(x - 5, y - 16, 10, 5, 'h');
        cv.rect(x + 10, y - 7, 10, 8, 'b');
        cv.line(x + 9, y - 8, x + 20, y - 8, 'o');
    }
}
/// One of the cook's mushrooms, standing at `(x, y)` (its foot): a red
/// cap on a pale stem, and its own silly feature.
fn mushroom(cv: &mut Img, x: i32, y: i32, which: usize) {
    cv.rect(x - 2, y - 6, 4, 6, 'h');
    match which {
        // In a boot.
        0 => {
            cv.rect(x - 3, y - 4, 6, 4, 'B');
            cv.rect(x - 3, y - 1, 9, 2, 'B');
            cv.line(x - 3, y - 4, x + 2, y - 4, 'o');
        }
        // A beard.
        2 => {
            cv.rect(x - 3, y - 5, 6, 4, '9');
            cv.put(x - 1, y - 1, '9');
            cv.put(x, y, '9');
        }
        _ => {}
    }
    cv.rect(x - 6, y - 10, 12, 4, 'R');
    cv.rect(x - 4, y - 12, 8, 2, 'R');
    if which == 1 {
        // A hat on its hat.
        cv.rect(x - 3, y - 16, 6, 4, 'b');
        cv.rect(x - 5, y - 13, 10, 1, 'b');
    }
    for dx in [-4, 0, 3] {
        cv.put(x + dx, y - 9, '9');
    }
}

pub(super) fn cart_figures(cv: &mut Img, run: &Run) {
    if run.room().kind != RoomKind::Timberyard {
        return;
    }
    let Some(round) = run.pastimes.round.as_ref().filter(|r| r.game == Game::Cart) else {
        return;
    };
    let c = round.cart.as_ref().unwrap();
    let (x, y) = at(c.x as f32 / 256.0 * 2.0, c.y as f32 / 256.0 * 2.0);
    let lift = (c.air_left.min(pastimes::CART_AIR - c.air_left) * 3) as i32;
    cv.line(x - 9, y + 3, x + 8, y + 3, 'b');
    let y = y - lift;
    cv.rect(x - 7, y - 7, 14, 7, 'o');
    cv.line(x - 9, y - 9, x + 9, y - 9, 'O');
    cv.rect(x + 5, y + 1, 5, 5, 'b');
    cv.put(x + 7, y + 3, 'h');
    cv.rect(x - 10, y - 12, 6, 7, 'h');
    cv.rect(x - 11, y - 15, 8, 4, 'u');
    cv.line(x - 10, y - 5, x - 14, y + 1, 'h');
    ring(
        cv,
        pastimes::CART_LOGS[usize::from(round.progress)] * TILE,
        7 * TILE,
        'O',
    );
}
pub(super) fn board_visible(run: &Run) -> bool {
    let s = wood::stage(&run.home.countryside, &run.marks);
    if s == 5 {
        return false;
    }
    if (1..5).contains(&s) && run.room().kind == RoomKind::HedgeMaze {
        let already = usize::from(s - 1).min(3);
        let carried = run.forage.as_ref().map_or([0; 3], |f| f.carried);
        return carried[already..].iter().any(|&c| c == 0);
    }
    let (kind, (x, y)) = if s == 0 { wood::ERRAND[0] } else { wood::ERRAND[4] };
    kind == run.room().kind
        && run.players.values().any(|h| {
            h.hp > 0 && !h.stone && h.privy == 0 && (h.x - x * 2.0).hypot(h.y - y * 2.0) < 5.0
        })
}
pub(super) fn boards(cv: &mut Img, run: &Run) {
    if !board_visible(run) {
        return;
    }
    let s = wood::stage(&run.home.countryside, &run.marks);
    let lines = match (s, run.room().kind) {
        (0, _) => [("BADGER'S SOUP", '9'), ("E: TAKE JOB", 'O')],
        (_, RoomKind::HedgeMaze) => [("BOOT HAT BEARD", '9'), ("E: PICK", 'O')],
        _ => [("SOUP POT", '9'), ("E: PUT IN", 'O')],
    };
    let board = home::board(&lines.map(|(t, ink)| (t.to_string(), ink)));
    cv.stamp(&board, 4, 13);
}
