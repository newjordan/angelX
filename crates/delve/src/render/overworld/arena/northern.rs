//! Snowy ridge, postal birds and a weather machine. Stonework is cached;
//! quest markers and sled state are dynamic so progress never goes stale.
use super::super::ink::{Img, hash};
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{RoomKind, Run, countryside};

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    if kind == RoomKind::Crag {
        // Snow-capped outcrops; a rope course between them.
        for (k, (c, r, w, h)) in [(3, 1, 5, 2), (16, 1, 5, 2), (3, 11, 6, 2), (16, 11, 5, 2)].into_iter().enumerate() {
            let mut stones = Vec::new();
            for n in 0..w * 2 {
                let v = hash(k as i32, n, 150);
                stones.push((c * TILE + 4 + n * 8 + (v % 4) as i32 - 2, r * TILE + h * TILE - 2 - (v >> 4) as i32 % (h * 6), v));
            }
            stones.sort_by_key(|s| s.1);
            for (x, y, v) in stones {
                m::boulder(cv, x, y, 6.0 + (v >> 9) as f32 % 3.0, m::ramp::SLATE, v);
                for d in -3..=3 {
                    cv.put(x + d, y - 10 + (d * d) / 4, if d % 2 == 0 { 'V' } else { 'v' });
                }
            }
        }
        for y in [4 * TILE, 10 * TILE] {
            for x in (3 * TILE..22 * TILE).step_by(2) {
                cv.put(x, y + ((x / 6) % 2), 'J');
            }
            for x in (3 * TILE..=22 * TILE).step_by(48) {
                cv.line(x, y - 6, x, y + 2, 'P');
            }
        }
        m::sign(cv, 9 * TILE, 3 * TILE - 2, "SQUIRES THIS WAY", 'T');
    } else if kind == RoomKind::Rookery {
        // The Royal Bird Office and its loft, pigeon-holed.
        for (k, (c, r, w, h)) in [(3, 2, 6, 3), (16, 2, 4, 3)].into_iter().enumerate() {
            let (x, y, pw, ph) = (c * TILE + 2, r * TILE - 8, w * TILE - 4, h * TILE + 8);
            m::house(cv, (x, y, pw, ph), &m::House { roof: m::ramp::SLATE, stone: true, windows: 0 }, 151 + k as u32);
            for hx in (x + 6..x + pw - 6).step_by(9) {
                for hy in [y + ph * 11 / 20 + 4] {
                    cv.ellipse(hx as f32, hy as f32, 2.5, 2.0, 'n');
                    cv.line(hx - 3, hy + 3, hx + 3, hy + 3, 'J');
                }
            }
        }
        m::sign(cv, 3 * TILE + 4, 5 * TILE + 5, "ROYAL BIRD OFFICE", 'T');
        m::sign(cv, 15 * TILE, 6 * TILE, "NO AIR MAIL", 'T');
        cv.text(15 * TILE, 6 * TILE + 12, "AFTER TEA", 'h');
    } else if kind == RoomKind::Observatory {
        // A stone tower under a slate dome, its telescope out.
        let keep = |_: i32, _: i32| true;
        m::block(cv, (6 * TILE + 4, TILE + 6, 4 * TILE - 8, 3 * TILE - 6), m::ramp::STONE, false, 152);
        m::lobe(cv, (8.0 * TILE as f32, 2.0 * TILE as f32), (28.0, 14.0), m::ramp::SLATE, 153, 5.0, 0.0, &keep);
        cv.line(8 * TILE - 2, TILE - 3, 8 * TILE + 2, 3 * TILE + 3, 'n');
        cv.line(7 * TILE, 3 * TILE, 10 * TILE, TILE, 'P');
        cv.line(7 * TILE, 3 * TILE + 1, 10 * TILE, TILE + 1, 'b');
        cv.line(10 * TILE - 2, TILE - 1, 10 * TILE + 3, TILE - 1, 'o');
        m::block(cv, (3 * TILE + 2, 11 * TILE - 4, 5 * TILE - 4, 2 * TILE + 2), m::ramp::TIMBER, true, 154);
        m::sign(cv, 8 * TILE, 5 * TILE, "CLOUDS BY APPOINTMENT", 'T');
        for &(room, pos) in countryside::FORECAST.iter().skip(2).take(3) {
            debug_assert_eq!(room, kind);
            let (x, y) = at(pos.0 * 2.0, pos.1 * 2.0);
            cv.rect(x - 7, y - 20, 14, 12, 'g');
            cv.frame(x - 7, y - 20, 14, 12, 'J');
            cv.ellipse(x as f32, (y - 14) as f32, 4.0, 4.0, 'u');
            cv.put(x - 1, y - 16, 'v');
            cv.line(x, y - 8, x - 7, y + 1, 'P');
            cv.line(x, y - 8, x + 7, y + 1, 'P');
        }
    }
    if matches!(kind, RoomKind::Observatory | RoomKind::MineHead) {
        let (c, r, w, _) = countryside::ROPEWAY;
        m::boards(cv, (c * TILE, r * TILE, w * TILE, TILE), false, 155);
        for x in [c * TILE, (c + w) * TILE - 2] {
            cv.line(x, r * TILE - 10, x, (r + 1) * TILE - 1, 'P');
            cv.line(x + 1, r * TILE - 10, x + 1, (r + 1) * TILE - 1, 'b');
        }
        if run.home.countryside.has(6) {
            super::open_arrow(cv, c * TILE + w * TILE / 2, r * TILE + 3);
        } else {
            super::shut_planks(cv, c * TILE, r * TILE, w * TILE);
        }
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let stage = countryside::forecast_stage(&run.home.countryside, &run.marks);
    for (i, &(room, pos)) in countryside::FORECAST.iter().enumerate().take(5) {
        if run.room().kind != room {
            continue;
        }
        let (x, y) = at(pos.0 * 2.0, pos.1 * 2.0);
        let current = stage == i as u8 || (i == 0 && stage == 5);
        let ink = if current { 'O' } else if stage > i as u8 { 'g' } else { 'u' };
        if i == 0 {
            cv.rect(x - 5, y - 14, 10, 14, 'u');
            cv.rect(x - 4, y - 21, 8, 7, 'h');
            cv.line(x - 6, y - 23, x + 6, y - 23, 'O');
            cv.rect(x + 5, y - 10, 9, 6, '9');
        } else if i == 1 {
            cv.ellipse(x as f32, (y - 7) as f32, 7.0, 5.0, 'O');
            cv.line(x + 3, y - 9, x + 3, y - 18, 'o');
            cv.rect(x + 1, y - 20, 6, 5, 'O');
            cv.rect(x + 7, y - 18, 4, 2, 'h');
            cv.put(x + 3, y - 22, 'R');
            cv.line(x - 2, y - 2, x - 2, y + 2, 'o');
        } else {
            cv.ellipse(x as f32, (y - 14) as f32, 4.0, 4.0, ink);
        }
        if current {
            for d in -3i32..=3 {
                let a = 9 - d.abs();
                cv.put(x + d * 2, y + a, ink);
                cv.put(x + d * 2, y - a, ink);
            }
        }
        if i >= 2 {
            // These names are puzzle clues, not dusk-lit ground decoration.
            // Stamp them after lighting on dark paper for readable contrast.
            let label = ["BOOT", "GOOSE", "CROWN"][i - 2];
            let width = super::super::ink::text_width(label) + 6;
            cv.rect(x - width / 2, y + 10, width, 9, 'k');
            cv.text(x - width / 2 + 3, y + 11, label, if current { 'O' } else { '9' });
        }
    }
    if run.room().kind == RoomKind::Rookery {
        for (i, x) in [11 * TILE, 14 * TILE, 17 * TILE].into_iter().enumerate() {
            let y = 8 * TILE - ((run.tick / 8 + i as u64) % 3) as i32;
            cv.ellipse(x as f32, y as f32, 5.0, 3.0, 'G');
            cv.line(x - 6, y - 2, x + 5, y - 2, 'h');
            cv.put(x + 6, y, 'O');
        }
    }
}

pub(super) fn board_visible(run: &Run) -> bool {
    if run.pastimes.round.is_some() {
        return false;
    }
    matches!(run.room().kind, RoomKind::Rookery | RoomKind::Observatory)
        || (run.room().kind == RoomKind::Crag && run.players.values().any(|h| {
            h.hp > 0 && !h.stone && h.privy == 0 && (h.x - 38.0).hypot(h.y - 9.0) < 3.0
        }))
}

pub(super) fn board(run: &Run) -> Option<Img> {
    if !board_visible(run) {
        return None;
    }
    let stage = countryside::forecast_stage(&run.home.countryside, &run.marks);
    let clue = [
        "E BY THE ROOKERY CLERK: GET THE JOB",
        "E BY THE BRASS ROOSTER AT CRAG",
        "OBSERVATORY: E AT THE BOOT LENS",
        "OBSERVATORY: E AT THE GOOSE LENS",
        "OBSERVATORY: E AT THE CROWN LENS",
        "E BY ROOKERY CLERK: RETURN FORECAST",
        "ROPEWAY READY",
    ][usize::from(stage)];
    Some(home::board(&[
        ("THE KING'S MISSING FORECAST".into(), '9'),
        (clue.into(), 'O'),
        (format!("ROAD JOB {stage}/6"), 'h'),
        ("SCATTERED EMBARRASSMENT".into(), 'h'),
    ]))
}
