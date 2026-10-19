//! The eastern picnic road: cached stonework, small dynamic props, local boards.
use super::super::ink::Img;
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{RoomKind, Run, countryside};

fn ring(cv: &mut Img, x: i32, y: i32, ink: char) {
    for i in 0..40 {
        let a = i as f32 * std::f32::consts::TAU / 40.0;
        cv.put(
            x + (a.cos() * 11.0) as i32,
            y + (a.sin() * 11.0) as i32,
            ink,
        );
    }
}

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    let kind = run.room().kind;
    if kind == RoomKind::Causeway {
        // The causeway's dressed kerbs along the water, setts between.
        for y in [4 * TILE - 10, 10 * TILE - 1] {
            m::block(cv, (TILE, y, 22 * TILE, 11), m::ramp::STONE, false, 140);
        }
        for y in (5 * TILE..9 * TILE).step_by(8) {
            for x in (TILE + (y / 8 % 2) * 9..23 * TILE).step_by(18) {
                cv.line(x, y, x + 12, y, 'g');
                cv.line(x, y + 1, x + 12, y + 1, 'K');
            }
        }
        m::sign(cv, 9 * TILE + 2, 4 * TILE + 4, "NO BRIDGE TAX", 'T');
        cv.text(10 * TILE, 5 * TILE + 6, "ONLY SNACK TAX", 'O');
    } else if kind == RoomKind::Priory {
        // The priory: a stone range under slate, its windows glowing.
        for (k, (c, r, w, h)) in [(3, 2, 6, 3), (4, 11, 5, 2)].into_iter().enumerate() {
            let (x, y, pw, ph) = (c * TILE + 2, r * TILE - 8, w * TILE - 4, h * TILE + 8);
            m::block(cv, (x, y + 10, pw, ph - 10), m::ramp::STONE, false, 141 + k as u32);
            m::roof(cv, (x - 2, y, pw + 4, 12), m::ramp::SLATE, 143 + k as u32);
            for wx in (x + 5..x + pw - 8).step_by(15) {
                cv.rect(wx, y + 16, 7, 14, 'n');
                cv.rect(wx + 1, y + 17, 5, 12, 'b');
                cv.rect(wx + 2, y + 18, 3, 9, 'O');
                cv.put(wx + 3, y + 16, 'J');
                cv.line(wx, y + 30, wx + 6, y + 30, 'J');
            }
        }
        m::sign(cv, 3 * TILE + 3, 5 * TILE + 6, "PUDDINGSTONE", 'T');
        for x in [12 * TILE, 14 * TILE] {
            m::bush(cv, x, 4 * TILE, 5.5, m::Leaf::Hedge, x as u32);
        }
    }
    if matches!(kind, RoomKind::Priory | RoomKind::Lists) {
        let (c, r, w, _) = countryside::POST_FLIGHT;
        m::boards(cv, (c * TILE, r * TILE, w * TILE, TILE), false, 145);
        for x in [c * TILE, (c + w) * TILE - 2] {
            cv.line(x, r * TILE - 13, x, (r + 1) * TILE - 1, 'P');
            cv.line(x + 1, r * TILE - 13, x + 1, (r + 1) * TILE - 1, 'b');
        }
        if run.home.countryside.has(5) {
            super::open_arrow(cv, c * TILE + w * TILE / 2, r * TILE + 3);
        } else {
            super::shut_planks(cv, c * TILE, r * TILE, w * TILE);
        }
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    let stage = countryside::picnic_stage(&run.home.countryside, &run.marks);
    for (i, &(room, pos)) in countryside::PICNIC.iter().enumerate() {
        if run.room().kind != room {
            continue;
        }
        let (x, y) = at(pos.0 * 2.0, pos.1 * 2.0);
        let ink = if stage > i as u8 {
            'g'
        } else if stage == i as u8 {
            'o'
        } else {
            'h'
        };
        ring(cv, x, y, ink);
        if i == 0 {
            // A cheerful troll whose entire toll booth is a sandwich invoice.
            cv.rect(x + 13, y - 15, 12, 14, 'g');
            cv.rect(x + 11, y - 22, 16, 9, 'g');
            cv.put(x + 15, y - 18, 'k');
            cv.put(x + 23, y - 18, 'k');
            cv.rect(x - 5, y - 8, 10, 7, 'h');
            cv.line(x - 3, y - 6, x + 2, y - 6, 'o');
            super::near_prompt(cv, run, (x, y), "TALK");
        } else if i == 1 {
            cv.rect(x - 7, y - 9, 14, 9, 'o');
            cv.rect(x - 6, y - 12, 12, 5, 'h');
            for offset in [-4, 0, 4] {
                cv.line(x + offset, y - 7, x + offset, y - 1, 'O');
            }
            super::near_prompt(cv, run, (x, y), "TALK");
        } else {
            cv.rect(x + 12, y - 15, 10, 15, 'p');
            cv.rect(x + 13, y - 22, 8, 8, 'h');
            cv.line(x + 9, y - 11, x + 3, y - 6, 'h');
            cv.rect(x - 6, y - 3, 12, 5, 'R');
            if stage == 3 {
                cv.text(x - 18, y - 25, "THANKS", ink);
            } else {
                super::near_prompt(cv, run, (x, y), "TALK");
            }
        }
    }
    if run.room().kind == RoomKind::Priory && run.home.countryside.has(5) {
        // Idle wings are dynamic, not part of the cached stonework.
        let x = 18 * TILE + (run.tick / 8 % 4) as i32;
        let y = 2 * TILE - 4;
        cv.ellipse(x as f32, y as f32, 5.0, 3.0, 'h');
        let wing = if run.tick / 4 % 2 == 0 { -5 } else { 4 };
        cv.line(x - 1, y, x - 9, y + wing, '9');
        cv.line(x + 1, y, x + 9, y + wing, '9');
        cv.put(x + 6, y - 1, 'O');
    }
}

pub(super) fn board_visible(run: &Run) -> bool {
    let kind = run.room().kind;
    let near = countryside::PICNIC.iter().any(|&(room, at)| {
        room == kind
            && run.players.values().any(|h| {
                h.hp > 0
                    && !h.stone
                    && h.privy == 0
                    && (h.x - at.0 * 2.0).hypot(h.y - at.1 * 2.0) < 4.0
            })
    });
    // Leave the hurdle scoreboard alone at its start/in its lane.
    run.pastimes.round.is_none() && (near || kind == RoomKind::Priory)
}

pub(super) fn board(run: &Run) -> Option<Img> {
    if !board_visible(run) {
        return None;
    }
    let stage = countryside::picnic_stage(&run.home.countryside, &run.marks);
    Some(home::quest_board(&[
        ("THE ABBOT'S LOST PICNIC".into(), '9'),
        (
            match stage {
                0 => "E AT CAUSEWAY TROLL: CLAIM TICKET",
                1 => "E AT PRIORY HAMPER: RECOVER LUNCH",
                2 => "E AT QUAY ABBOT: RETURN THE HAMPER",
                _ => "MUSTARD FORGIVEN",
            }
            .into(),
            'O',
        ),
        ("CAUSEWAY > PRIORY > WASHBUCKET QUAY".into(), 'h'),
        ("60 GOLD, 3 WAX".into(), 'h'),
        ("PIGEON POST: LISTS".into(), 'h'),
    ]))
}
