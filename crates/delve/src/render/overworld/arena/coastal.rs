//! Cached seaside silhouettes; the tide, wind sock and thrown pot are live ink.
use super::super::ink::Img;
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{
    HZ, RoomKind, Run, coastal as coast, countryside as c,
    pastimes::{Game, Put, TARGETS},
};

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    // The sea and the pans are the room's water tiles, drawn by the world.
    match run.room().kind {
        RoomKind::Strand => {
            // A funny sign and shells on the beach, never hidden collision.
            m::sign(cv, 3 * TILE, 4 * TILE + 4, "CLEAN POTS ONLY", 'T');
            for (x, y) in [(6, 5), (8, 9), (15, 9), (20, 5)] {
                let (x, y) = (x * TILE, y * TILE);
                cv.ellipse(x as f32, y as f32, 3.5, 2.0, 'J');
                cv.line(x - 2, y - 1, x + 1, y - 1, 'i');
                cv.put(x + 3, y + 1, 'g');
            }
            // Driftwood and a beached rowing boat by the north water.
            m::boards(cv, (10 * TILE, 4 * TILE + 8, 18, 3), false, 130);
            let keep = |_: i32, _: i32| true;
            m::lobe(cv, (19.0 * TILE as f32, 5.0 * TILE as f32 + 2.0), (14.0, 5.0), m::ramp::TIMBER, 131, 8.0, 0.0, &keep);
            cv.ellipse(19.0 * TILE as f32, 5.0 * TILE as f32 + 1.0, 10.0, 2.5, 'n');
            cv.line(19 * TILE - 2, 5 * TILE - 2, 19 * TILE - 2, 5 * TILE + 4, 'P');
        }
        RoomKind::Saltworks => {
            for x in [3, 16] {
                // Stone kerbs round the pans, salt heaped on the dry side.
                m::block(cv, (x * TILE - 2, 2 * TILE - 4, 5 * TILE + 4, 4), m::ramp::STONE, false, 132);
                for dx in [1, 3] {
                    let keep = |_: i32, _: i32| true;
                    m::lobe(cv, (((x + dx) * TILE) as f32, (4 * TILE + 6) as f32), (8.0, 4.5), m::ramp::SLATE, 133 + dx as u32, 2.0, 0.1, &keep);
                }
            }
            let (c, r, w, _) = c::BARGE;
            m::block(cv, (c * TILE, r * TILE - 2, w * TILE, 11), m::ramp::TIMBER, true, 134);
            m::sign(cv, 3 * TILE, 5 * TILE - 4, "CRABS UNION LOCAL I", 'T');
            for (k, x) in [9 * TILE, 13 * TILE].into_iter().enumerate() {
                m::barrel(cv, x, 3 * TILE + 2 + k as i32 * 3);
            }
        }
        RoomKind::Lighthouse => {
            let (x, y) = (4 * TILE, 2 * TILE);
            // The keeper's cottage, and the beacon tower beside it, its
            // lamp lit once the harbour is done.
            m::house(cv, (x + 2, y - 4, 3 * TILE - 2, 3 * TILE + 2), &m::House { roof: m::ramp::SLATE, stone: true, windows: 1 }, 135);
            let (tx, foot) = (x + 4 * TILE - 2, y + 3 * TILE - 1);
            m::tower(cv, tx, foot, 9, 52, 136);
            let lit = run.home.countryside.harbour == 3;
            cv.ellipse(tx as f32, (foot - 58) as f32, 4.0, 2.5, if lit { 'O' } else { 'r' });
            cv.rect(tx - 2, foot - 11, 5, 11, 'n');
        }
        RoomKind::Riverbank => {
            let (x, y, w, _) = c::QUAY_BARGE;
            m::block(cv, (x * TILE, y * TILE - 2, w * TILE, 11), m::ramp::TIMBER, true, 137);
        }
        _ => {}
    }
}
fn ring(cv: &mut Img, x: i32, y: i32, ink: char) {
    for n in 0..40 {
        let a = n as f32 * std::f32::consts::TAU / 40.0;
        cv.put(
            x + (a.cos() * 10.0) as i32,
            y + (a.sin() * 10.0) as i32,
            ink,
        );
    }
}
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let s = coast::stage(&run.home.countryside, &run.marks);
    if run.room().kind == RoomKind::Lighthouse {
        let (x, y) = at(18.5 * 2.0, 8.5 * 2.0);
        ring(cv, x, y, if s == 0 || s == 2 { 'O' } else { 'h' });
        cv.rect(x - 5, y - 11, 10, 11, 'B');
        cv.ellipse(x as f32, (y - 16) as f32, 4.0, 4.0, 'h');
        cv.rect(x - 6, y - 22, 12, 3, 'G');
        cv.put(x + 2, y - 16, 'k');
        cv.line(x + 8, y - 20, x + 8, y + 2, 'o');
        cv.text(x - 14, y + 14, "KEEPER", 'h');
    }
    if run.room().kind == RoomKind::Strand {
        match &run.crab {
            Some(crab) => thief(cv, run, crab),
            None => {
                // Off duty: the crab idles on its rock, oil long gone.
                let (x, y) = at(19.0 * 2.0, 9.0 * 2.0);
                let dx = ((run.tick / 8) % 5) as i32 - 2;
                legs_and_shell(cv, x + dx, y + 4, run.tick, false);
            }
        }
    }
}

/// A crab: red shell, two claws up, legs going.
fn legs_and_shell(cv: &mut Img, x: i32, y: i32, tick: u64, running: bool) {
    let kick = if running { (tick / 2 % 2) as i32 } else { 0 };
    for (k, sign) in [(0, -1), (1, -1), (0, 1), (1, 1)] {
        let lx = x + sign * (4 + k * 2);
        cv.line(lx, y + 1, lx + sign * 2, y + 3 + (kick + k) % 2, '7');
    }
    cv.ellipse(x as f32, y as f32, 6.0, 3.5, 'R');
    cv.rect(x - 4, y - 2, 8, 1, 'T');
    for sign in [-1, 1] {
        cv.line(x + sign * 4, y - 2, x + sign * 7, y - 6, 'R');
        cv.rect(x + sign * 7 - 1, y - 9, 3, 3, 'R');
        cv.put(x + sign * 2, y - 5, '9');
        cv.put(x + sign * 2, y - 4, 'R');
    }
}

/// The oil thief: running with the jar, or dug in with only its eyes up.
fn thief(cv: &mut Img, run: &Run, crab: &coast::Crab) {
    let (x, y) = at(crab.x, crab.y);
    if crab.dug {
        cv.ellipse(x as f32, (y + 2) as f32, 7.0, 2.5, 'h');
        for sign in [-1, 1] {
            cv.put(x + sign * 2, y - 2, '9');
            cv.put(x + sign * 2, y - 1, 'R');
        }
        let left = coast::tide_left(run.tick).div_ceil(HZ);
        cv.text(x - 14, y - 12, &format!("DUG {left}S"), 'U');
        return;
    }
    cv.ellipse(x as f32, (y + 5) as f32, 7.0, 2.0, 'k');
    legs_and_shell(cv, x, y, run.tick, true);
    // The stolen jar, held aloft.
    cv.rect(x - 3, y - 14, 6, 6, 'B');
    cv.rect(x - 2, y - 15, 4, 1, 'O');
    cv.rect(x - 2, y - 12, 2, 2, 'O');
}
pub(super) fn put_figures(cv: &mut Img, run: &Run) {
    if run.room().kind != RoomKind::Strand {
        return;
    }
    let active = run.pastimes.round.as_ref().filter(|r| r.game == Game::Pot);
    for (i, tx) in TARGETS.into_iter().enumerate() {
        let (x, y) = at(tx as f32 * 2.0, 7.0 * 2.0);
        let ink = if active.is_some_and(|r| usize::from(r.progress) > i) {
            '9'
        } else if active.is_some_and(|r| usize::from(r.progress) == i) {
            'O'
        } else {
            '9'
        };
        // Sand/speckles must not compete with the target's landing bracket.
        cv.rect(x - TILE - 2, y + 1, 2 * TILE + 5, 11, 'k');
        cv.line(x - TILE, y + 5, x + TILE, y + 5, ink);
        cv.line(x - TILE, y + 5, x - TILE, y + 1, ink);
        cv.line(x + TILE, y + 5, x + TILE, y + 1, ink);
        cv.line(x - TILE, y + 6, x + TILE, y + 6, ink);
        cv.rect(x - 5, y + 13, 10, 10, 'k');
        cv.text(x - 2, y + 15, &format!("{}", i + 1), ink);
    }
    if let Some(r) = active {
        let p = r.put.as_ref().unwrap();
        let (x, y) = at(4.0 * 2.0, 7.0 * 2.0);
        let a = f32::from(p.angle).to_radians();
        cv.line(
            x,
            y - 4,
            x + (a.cos() * 23.0) as i32,
            y - 4 - (a.sin() * 23.0) as i32,
            'O',
        );
        cv.rect(x - 11, y + 19, 22, 3, 'r');
        cv.rect(x - 11, y + 19, (p.charge * 22 / HZ) as i32, 3, '9');
        let (wx, wy) = at(9.0 * 2.0, 5.0 * 2.0);
        let wind = Put::wind(r.progress);
        cv.line(wx, wy - 12, wx, wy + 3, 'o');
        cv.line(wx, wy - 12, wx + wind * 9, wy - 8, 'R');
        cv.line(wx, wy - 9, wx + wind * 9, wy - 8, 'R');
        if let Some(pot) = &p.flight {
            let (px, py) = at(pot.x as f32 / 128.0, 14.0);
            cv.ellipse(px as f32, (py + 4) as f32, 4.0, 1.5, 'k');
            // At full charge/70 degrees, a physical peak is 2053 fixed units.
            // Fit even that arc below the top board (bottom at y45). Compress
            // only screen height; physics, landing range and shadows unchanged.
            let y = py - pot.z.min(2053) * TILE / 640 - 7;
            cv.rect(px - 4, y, 8, 6, 'h');
            cv.line(px - 5, y, px + 5, y, 'O');
            cv.line(px + 4, y + 2, px + 6, y + 2, 'h');
        }
    }
}
pub(super) fn board_visible(run: &Run) -> bool {
    if run.pastimes.round.is_some() {
        return false;
    }
    if !matches!(run.room().kind, RoomKind::Strand | RoomKind::Lighthouse) {
        return false;
    }
    let s = coast::stage(&run.home.countryside, &run.marks);
    if s == 3 {
        return false;
    }
    if s == 1 && run.room().kind == RoomKind::Strand {
        // The chase: a board while the crab is about.
        return run.crab.is_some();
    }
    let (kind, (x, y)) = coast::ERRAND[usize::from(s)];
    kind == run.room().kind
        && run.players.values().any(|h| {
            h.hp > 0 && !h.stone && h.privy == 0 && (h.x - x * 2.0).hypot(h.y - y * 2.0) < 5.0
        })
}
pub(super) fn boards(cv: &mut Img, run: &Run) {
    if !board_visible(run) {
        return;
    }
    let s = coast::stage(&run.home.countryside, &run.marks);
    let lines = if s == 0 {
        vec![("THE KEEPER".into(), '9'), ("E: LISTEN".into(), 'O')]
    } else if s == 1 {
        vec![("CRAB HAS OIL".into(), '9'), ("CATCH IT".into(), 'O')]
    } else {
        vec![("OIL BACK".into(), '9'), ("E: GIVE OIL".into(), 'O')]
    };
    let board = home::board(&lines);
    cv.stamp(&board, 4, 13);
}
