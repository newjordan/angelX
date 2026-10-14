//! High-road landmarks are cached; only papers, bells and the turnip animate.
use super::super::ink::Img;
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{
    RoomKind, Run, aerie,
    pastimes::{self, Game},
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
fn pigeon(cv: &mut Img, x: i32, y: i32, ink: char) {
    cv.rect(x - 6, y - 8, 11, 6, ink);
    cv.rect(x + 3, y - 13, 5, 8, ink);
    cv.put(x + 6, y - 11, 'k');
    cv.line(x + 8, y - 9, x + 11, y - 7, 'O');
    cv.line(x - 3, y - 2, x - 3, y + 2, 'o');
    cv.line(x + 2, y - 2, x + 2, y + 2, 'o');
}

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::Belfry => {
            // Two stone bell towers, bells hung in their open lanterns.
            for c in [3, 19] {
                let (x, y) = (c * TILE + 1, 2 * TILE - 14);
                m::block(cv, (x, y + 8, 2 * TILE - 2, 3 * TILE + 6), m::ramp::STONE, false, 170 + c as u32);
                m::roof(cv, (x - 3, y, 2 * TILE + 4, 9), m::ramp::SLATE, 172);
                cv.rect(x + 8, y + 18, TILE - 2, 18, 'n');
                cv.line(x + 8, y + 18, x + TILE + 5, y + 18, 'J');
                cv.ellipse((x + 15) as f32, (y + 29) as f32, 5.0, 5.5, 'R');
                cv.ellipse((x + 14) as f32, (y + 28) as f32, 3.0, 3.5, 'o');
                cv.line(x + 10, y + 34, x + 20, y + 34, 'B');
                cv.put(x + 15, y + 35, 'O');
            }
            // The rope course between them, on timber posts.
            for x in [6, 18] {
                cv.line(x * TILE, 4 * TILE, x * TILE, 10 * TILE, 'P');
                cv.line(x * TILE + 1, 4 * TILE, x * TILE + 1, 10 * TILE, 'b');
            }
            for y in [4 * TILE, 10 * TILE] {
                for x in 6 * TILE..18 * TILE {
                    cv.put(x, y + ((x / 5) % 2), if x % 3 == 0 { 'o' } else { 'r' });
                }
            }
            m::sign(cv, 7 * TILE, 11 * TILE + 2, "NO RINGING AFTER VESPERS", 'T');
        }
        RoomKind::Aqueduct => {
            // Stone piers; the high channel carried on arches between them.
            for c in [7, 15] {
                for (r, h) in [(2, 3), (10, 2)] {
                    m::block(cv, (c * TILE + 2, r * TILE - 6, 2 * TILE - 4, h * TILE + 6), m::ramp::STONE, false, 173 + (c + r) as u32);
                }
            }
            let (x0, x1, y) = (9 * TILE - 2, 15 * TILE + 2, 2 * TILE - 4);
            m::block(cv, (x0, y, x1 - x0, 14), m::ramp::STONE, false, 175);
            m::water(cv, (x0 + 2, y + 1, x1 - x0 - 4, 4), 176, (run.tick / 6) as u32);
            for ax in (x0 + 4..x1 - 12).step_by(16) {
                cv.ellipse((ax + 6) as f32, (y + 14) as f32, 6.0, 5.0, 'k');
            }
            m::sign(cv, 8 * TILE, 12 * TILE, "WATER SOLD SEPARATELY", 'T');
        }
        RoomKind::Aviary => {
            // The pigeon parliament's dovecote: boarded, pigeon-holed, roofed.
            m::block(cv, (3 * TILE, 2 * TILE - 4, 5 * TILE, 2 * TILE + 4), m::ramp::TIMBER, true, 177);
            m::roof(cv, (3 * TILE - 3, 2 * TILE - 13, 5 * TILE + 6, 10), m::ramp::TILE, 178);
            for c in [4, 5, 6, 7] {
                for row in [0, 12] {
                    cv.ellipse((c * TILE + 4) as f32, (2 * TILE + 8 + row) as f32, 3.0, 3.0, 'n');
                    cv.line(c * TILE, 2 * TILE + 12 + row, c * TILE + 8, 2 * TILE + 12 + row, 'o');
                }
            }
            m::sign(cv, 3 * TILE + 3, 4 * TILE + 6, "COO-RUM REQUIRED", 'T');
            for (x, y) in [(9.0, 4.0), (13.0, 9.5), (8.0, 11.5)] {
                let (x, y) = at(x * 2.0, y * 2.0);
                pigeon(cv, x, y, 'h');
            }
        }
        _ => {}
    }
}
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let (stage, papers) = aerie::progress(&run.home.countryside, &run.marks);
    if run.room().kind == RoomKind::Aviary {
        let (x, y) = at(aerie::CLERK.0 * 2.0, aerie::CLERK.1 * 2.0);
        ring(
            cv,
            x,
            y,
            if stage == 0 || (stage == 1 && papers == 7) {
                'O'
            } else {
                '9'
            },
        );
        cv.rect(x - 7, y - 19, 14, 14, 'b');
        pigeon(cv, x, y - 19, 'h');
        cv.rect(x - 5, y - 35, 10, 4, 'O');
        cv.text(x - 17, y - 45, "SPEAKER", 'O');
    }
    if let Some(p) = &run.pigeons {
        // A paper on the ground, waiting for E.
        if let Some((i, px, py)) = p.dropped {
            if aerie::DOCUMENTS[usize::from(i)].0 == run.room().kind {
                let (x, y) = at(px, py);
                paper(cv, x, y);
                super::near_prompt(cv, run, (x, y), "TAKE");
            }
        }
    }
    let kind = run.room().kind;
    if let Some(i) = aerie::flying(kind, stage, papers, run.pigeons.as_ref()) {
        let (fx, fy) = aerie::pigeon_at(run.room(), i, run.tick);
        let (x, y) = at(fx, fy);
        // Its shadow on the ground, the bird above with the paper.
        cv.ellipse(x as f32, (y + 3) as f32, 6.0, 2.0, 'k');
        let flap = run.tick / 4 % 2 == 0;
        let y = y - 14;
        cv.rect(x - 5, y - 3, 10, 5, 'u');
        cv.rect(x + 3, y - 6, 4, 4, 'u');
        cv.put(x + 5, y - 5, 'k');
        cv.line(x + 7, y - 4, x + 9, y - 3, 'O');
        let wing = if flap { -6 } else { 2 };
        cv.line(x - 3, y - 2, x - 7, y + wing, 'U');
        cv.line(x + 1, y - 2, x - 1, y + wing - 1, 'U');
        cv.rect(x + 8, y - 3, 5, 6, 'h');
        cv.line(x + 9, y - 1, x + 11, y - 1, 'o');
    }
    if run.room().kind != RoomKind::Belfry {
        return;
    }
    let volley = run
        .pastimes
        .round
        .as_ref()
        .filter(|r| r.game == Game::Volley)
        .and_then(|r| r.volley.as_ref());
    for (i, x) in pastimes::BELL_X.into_iter().enumerate() {
        let (x, y) = at(x as f32 / 256.0 * 2.0, 8.0);
        let ink = if volley.is_some_and(|v| v.bells & (1 << i) != 0) {
            'g'
        } else {
            'O'
        };
        cv.rect(x - 7, y - 16, 14, 10, ink);
        cv.line(x - 10, y - 5, x + 10, y - 5, ink);
        cv.rect(x - 2, y - 4, 4, 3, ink);
        cv.text(x - 2, y - 27, &(i + 1).to_string(), ink);
    }
    if let Some(v) = volley {
        let (x, y) = at(v.paddle as f32 / 256.0 * 2.0, 20.0);
        let reach = pastimes::PADDLE_REACH * TILE / 256;
        cv.rect(x - reach, y - 3, reach * 2 + 1, 6, 'o');
        cv.rect(x - 6, y - 6, 12, 10, 'O');
        cv.line(x, y - 5, x, y + 3, 'h');
        if let Some(b) = &v.ball {
            let (x, y) = at(b.x as f32 / 256.0 * 2.0, b.y as f32 / 256.0 * 2.0);
            cv.rect(x - 3, y - 3, 7, 6, 'O');
            cv.rect(x - 2, y - 2, 5, 4, 'h');
            cv.line(x, y - 3, x + 3, y - 6, 'g');
        } else {
            cv.text(9 * TILE, 8 * TILE, "TAP F TO SERVE", 'O');
        }
    }
}
/// Over the knights: poop on those the pigeons voted on.
pub(super) fn poop(cv: &mut Img, run: &Run) {
    let Some(p) = &run.pigeons else {
        return;
    };
    if !aerie::PLACES.contains(&run.room().kind) {
        return;
    }
    for (id, &n) in &p.poop {
        if let Some(h) = run.players.get(id) {
            let (x, y) = at(h.x, h.y);
            for k in 0..n.min(6) as usize {
                let (dx, dy) = [(-3, -15), (2, -13), (-1, -10), (3, -17), (-4, -8), (1, -6)][k];
                cv.rect(x + dx, y + dy, 2, 2, '9');
                cv.put(x + dx + 1, y + dy + 2, 'u');
            }
        }
    }
}

/// A tax paper, lying where it fell.
fn paper(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x - 5, y - 6, 10, 8, 'h');
    cv.line(x - 3, y - 4, x + 3, y - 4, 'o');
    cv.line(x - 3, y - 2, x + 2, y - 2, 'o');
    cv.rect(x + 2, y, 2, 2, '7');
}

pub(super) fn board_visible(run: &Run) -> bool {
    if !aerie::PLACES.contains(&run.room().kind) || run.pastimes.round.is_some() {
        return false;
    }
    let near = |at: (f32, f32)| {
        run.players.values().any(|h| {
            h.hp > 0 && !h.stone && h.privy == 0 && (h.x - at.0 * 2.0).hypot(h.y - at.1 * 2.0) < 5.0
        })
    };
    if crate::rules::countryside::PLOTS
        .iter()
        .any(|p| p.room == run.room().kind && near(p.at))
    {
        return false;
    }
    let (stage, papers) = aerie::progress(&run.home.countryside, &run.marks);
    (run.room().kind == RoomKind::Aviary && near(aerie::CLERK))
        || aerie::flying(run.room().kind, stage, papers, run.pigeons.as_ref()).is_some()
        || run.pigeons.as_ref().and_then(|p| p.dropped).is_some_and(|d| aerie::DOCUMENTS[usize::from(d.0)].0 == run.room().kind)
}
pub(super) fn boards(cv: &mut Img, run: &Run) {
    if !board_visible(run) {
        return;
    }
    let (stage, papers) = aerie::progress(&run.home.countryside, &run.marks);
    let lines = match stage {
        0 => [("THE SPEAKER".to_string(), '9'), ("E: LISTEN".to_string(), 'O')],
        1 if papers == 7 => [("MINUTES 3/3".to_string(), '9'), ("E: SPEAKER".to_string(), 'O')],
        1 if run.pigeons.as_ref().and_then(|p| p.dropped).is_some() => {
            [(format!("MINUTES {}/3", papers.count_ones()), '9'), ("E: TAKE PAPER".to_string(), 'O')]
        }
        _ => [(format!("MINUTES {}/3", papers.count_ones()), '9'), ("F: BONK PIGEON".to_string(), 'O')],
    };
    let board = home::board(&lines);
    cv.stamp(&board, 4, 13);
}
