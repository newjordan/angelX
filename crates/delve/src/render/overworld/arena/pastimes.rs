//! The surface arcade uses cached scenery. Only pads, six course markers and
//! a tiny nuisance move per frame. Browser friends see the host's same picture.

use super::super::ink::Img;
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{
    HZ, RoomKind, Run,
    pastimes::{self, Game},
};

fn ring(cv: &mut Img, x: i32, y: i32, radius: f32, ink: char) {
    for n in 0..40 {
        let a = n as f32 * std::f32::consts::TAU / 40.0;
        cv.put(
            x + (a.cos() * radius) as i32,
            y + (a.sin() * radius) as i32,
            ink,
        );
    }
}

fn cottage(cv: &mut Img, c: i32, r: i32, w: i32, h: i32, k: usize) {
    let (x, y, w, h) = (c * TILE, r * TILE, w * TILE, h * TILE);
    let roof = [m::ramp::THATCH, m::ramp::TILE, m::ramp::SLATE][k % 3];
    m::house(cv, (x + 3, y - 6, w - 6, h + 4), &m::House { roof, stone: k % 2 == 0, windows: (w / 24).max(1) }, 100 + k as u32);
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
                m::block(cv, (x + 3, y + 12, w * TILE - 6, 18), m::ramp::TIMBER, true, 110 + k as u32);
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
                    cv.line(x + 7 + d / 2, 3 * TILE + 3 + d, x + 11 - d / 2, 3 * TILE + 3 + d, ink);
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
    for game in Game::ALL.into_iter().filter(|g| g.room() == kind) {
        let (tx, ty) = game.start();
        let (x, y) = at(tx * 2.0, ty * 2.0);
        let ink = if active.is_some_and(|r| r.game == game) {
            '9'
        } else {
            'O'
        };
        ring(cv, x, y, 11.0, ink);
        let symbol = pad_letter(game).to_string();
        cv.text(x - 2, y - 3, &symbol, ink);
    }
    if kind == RoomKind::Village {
        for (i, (tx, ty)) in pastimes::MAILBOXES.into_iter().enumerate() {
            let (x, y) = at(tx * 2.0, ty * 2.0);
            let marked =
                active.is_some_and(|r| r.game == Game::Broadsheet && r.recipients & (1 << i) != 0);
            let done =
                active.is_some_and(|r| r.game == Game::Broadsheet && r.tried & (1 << i) != 0);
            cv.rect(x - 2, y - 5, 4, 11, 'r');
            cv.rect(x - 6, y - 11, 12, 8, 'B');
            cv.line(x - 6, y - 11, x + 5, y - 11, if marked { 'O' } else { 'h' });
            cv.rect(x - 3, y - 8, 6, 2, if done { 'g' } else { 'k' });
            if marked && !done {
                cv.line(x + 6, y - 13, x + 6, y - 3, 'O');
                cv.rect(x + 7, y - 13, 5, 4, 'O');
            }
        }
    }
    for game in [Game::Barrel, Game::Washtub]
        .into_iter()
        .filter(|g| g.room() == kind)
    {
        for i in 0..6 {
            let (tx, ty) = if game == Game::Barrel {
                pastimes::FLAGS[i]
            } else {
                pastimes::wake(i, run.tick)
            };
            let (x, y) = at(tx * 2.0, ty * 2.0);
            let next = active.is_some_and(|r| r.game == game && usize::from(r.progress) == i);
            let done = active.is_some_and(|r| r.game == game && usize::from(r.progress) > i);
            let ink = if next {
                'O'
            } else if done {
                'g'
            } else {
                'u'
            };
            ring(cv, x, y, if next { 12.0 } else { 8.0 }, ink);
            if game == Game::Barrel {
                cv.line(x, y - 14, x, y - 3, ink);
                cv.rect(x + 1, y - 14, 7, 5, ink);
            }
            cv.text(x - 2, y - 2, &(i + 1).to_string(), ink);
        }
    }
    if kind == RoomKind::Fairground {
        let (tx, ty) = Game::Turnip.start();
        let (x, y) = at(tx * 2.0, ty * 2.0);
        let elapsed = active
            .filter(|r| r.game == Game::Turnip)
            .map_or(0, |r| Game::Turnip.time() - r.left);
        let phase = elapsed % HZ;
        let height = ((phase as f32 / HZ as f32 * std::f32::consts::PI).sin() * 24.0) as i32;
        cv.rect(
            x - 4,
            y - height - 6,
            8,
            5,
            if phase <= HZ / 5 { 'O' } else { 'h' },
        );
        cv.line(x, y - height - 8, x + 2, y - height - 11, 'g');
        ring(cv, x, y, 14.0, if phase <= HZ / 5 { '9' } else { 'r' });
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
            cv.rect(x - 3, y - 3, 7, 7, 'O');
            cv.rect(x - 2, y - 2, 5, 5, 'Y');
            cv.rect(x - 1, y, 2, 1, 'o');
            cv.rect(x + 1, y - 1, 1, 1, 'o');
        }
    }
    if kind == RoomKind::Causeway {
        let (lx, ly) = at(2.0 * 2.0, (pastimes::RIDING_LANE.0 - 0.25) * 2.0);
        let (rx, by) = at(22.0 * 2.0, (pastimes::RIDING_LANE.1 + 0.25) * 2.0);
        for x in (lx..rx).step_by(12) {
            cv.line(x, ly, x + 6, ly, 'g');
            cv.line(x, by, x + 6, by, 'g');
        }
        let current = active.filter(|r| r.game == Game::Hobbyhorse);
        for (i, &fence) in pastimes::HURDLES.iter().enumerate() {
            let (x, top) = at(fence * 2.0, pastimes::RIDING_LANE.0 * 2.0);
            let (_, bottom) = at(fence * 2.0, pastimes::RIDING_LANE.1 * 2.0);
            let ink = current.map_or('h', |r| {
                if i < usize::from(r.progress) {
                    'g'
                } else if i == usize::from(r.progress) {
                    'O'
                } else {
                    'h'
                }
            });
            cv.line(x, top, x, bottom, ink);
            for y in [top, bottom] {
                cv.rect(x - 2, y - 4, 5, 8, 'o');
                cv.put(x, y - 4, ink);
            }
            cv.text(x - 2, top - 10, &(i + 1).to_string(), ink);
        }
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
                // A very wooden hobbyhorse. The arc doesn't move collision.
                cv.line(x + 5, y + 4, x + 11, y - 6 - lift, 'o');
                cv.rect(x + 9, y - 11 - lift, 9, 6, 'O');
                cv.rect(x + 10, y - 14 - lift, 3, 4, 'o');
                cv.put(x + 16, y - 9 - lift, 'k');
                if lift > 0 {
                    ring(cv, x, y - lift, 12.0, 'O');
                }
            }
        }
    }
    if kind == RoomKind::Crag {
        let current = active.filter(|r| r.game == Game::Sled);
        for (i, (gx, gy)) in pastimes::SLED_GATES.into_iter().enumerate() {
            let (x, y) = at((gx * 2) as f32, (gy * 2) as f32);
            let ink = if current.is_some_and(|r| usize::from(r.progress) == i) {
                '5'
            } else if current.is_some_and(|r| usize::from(r.progress) > i) {
                '2'
            } else {
                'z'
            };
            for side in [-1, 1] {
                for dx in -1..=1 {
                    cv.line(x + dx, y + side * TILE, x + dx, y + side * (TILE + 12), ink);
                }
                cv.rect(x + 2, y + side * (TILE + 10), 8, 4, ink);
            }
            cv.rect(x - 6, y - 4, 13, 10, 'k');
            cv.text(x - 2, y - 3, &(i + 1).to_string(), ink);
        }
        if let Some(sled) = current.and_then(|r| r.sled.as_ref()) {
            let (x, y) = at(sled.x as f32 / 128.0, sled.y as f32 / 128.0);
            cv.ellipse((x - 1) as f32, (y + 3) as f32, 10.0, 4.0, 's');
            cv.line(x - 10, y + 3, x + 9, y + 3, '2');
            cv.rect(x - 4, y - 7, 7, 9, 'u');
            cv.rect(x - 3, y - 13, 6, 6, 'h');
            cv.line(x - 2, y - 15, x + 7, y - 15, '5');
            cv.line(x + 3, y - 7, x + 9, y - 1, 'h');
            cv.put(x + 2, y - 11, 'k');
            cv.line(x - 13, y + 6, x - 7, y + 6, 'h');
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

pub(super) fn boards(cv: &mut Img, run: &Run) {
    if run.room().kind == RoomKind::GooseGreen {return;}
    if super::mummers::travel_near(run) { return; }
    // The race circuit has its own controls, spectator card and quest board.
    if kind_is_hippodrome(run) { return; }
    if run.room().kind==RoomKind::GooseGreen {return;}
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
        let up_top = matches!(game, Game::Sled | Game::Pot | Game::Cart | Game::Surf | Game::Halfpipe);
        let board = match active {
            None => home::board(&[
                (game.name().into(), '9'),
                (format!("STEP ON {}", pad_letter(game)), 'O'),
                (format!("BEST {}", record.best_for(game)), 'h'),
            ]),
            Some(r) => {
                let score = format!("{} / {}", r.progress, game.need());
                let mut lines = vec![
                    (format!("{}  {}S", game.name(), r.left.div_ceil(HZ)), '9'),
                    (how(game).into(), 'O'),
                ];
                match live_cue(game, r) {
                    Some(cue) if up_top => lines.push((format!("{score}  {cue}"), '9')),
                    Some(cue) => lines.extend([(score, '9'), (cue, '6')]),
                    None => lines.push((score, '9')),
                }
                home::board(&lines)
            }
        };
        let x = if n == 0 { 4 } else { cv.w - board.w - 4 };
        let y = if up_top {
            13
        } else {
            cv.h - board.h - super::tray::max_height(run, cv.w - 8) - 16
        };
        cv.stamp(&board, x, y);
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
        Game::Volley => "BAT AT BELLS",
        Game::Halfpipe => "PUMP, THEN TRICK",
        Game::Chariot => "RACE THREE LAPS",
        Game::Trencher => "TOSS TO GOOSE",
        Game::DuckShoot => "AIM AND SHOOT",
        Game::Longbow => "WALK, DRAW, LOOSE",
    }
}

/// The one thing to do right now, where a game has a moment.
fn live_cue(game: Game, r: &crate::rules::pastimes::Round) -> Option<String> {
    match game {
        Game::Surf => {
            let s = r.surf.as_ref()?;
            (s.phase >= pastimes::WAVE_TICKS - pastimes::CREST_WINDOW).then(|| "F NOW!".to_string())
        }
        Game::Pot => {
            let p = r.put.as_ref()?;
            Some(format!("POWER {}%", p.charge * 100 / HZ))
        }
        Game::Volley => Some(format!("TURNIPS {}", r.volley.as_ref()?.lives)),
        Game::Halfpipe => Some(format!("HELMETS {}", 3u8.saturating_sub(r.mistakes))),
        Game::DuckShoot => Some("FAR DUCK: TWO".into()),
        Game::Longbow => {
            let b = r.longbow.as_ref()?;
            Some(if b.draw >= pastimes::LONGBOW_FULL_DRAW {
                "FULL DRAW!".into()
            } else {
                format!("ARROWS {}", pastimes::LONGBOW_ARROWS.saturating_sub(b.arrows))
            })
        }
        _ => None,
    }
}

fn kind_is_hippodrome(run: &Run) -> bool {run.room().kind == RoomKind::Hippodrome}
