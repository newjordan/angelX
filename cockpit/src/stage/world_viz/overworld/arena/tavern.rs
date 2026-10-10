//! The Siege Perilous, Maud's tavern, drawn: a plank floor, the bar along
//! the north wall with its taps and its shelves of bottles, two tables and
//! their stools, barrels in the corner, lanterns, the rumour board, and the
//! Siege Perilous itself in the south-west corner on its step, gilt and
//! high-backed, a faint light about it. Maud keeps the bar once she's home;
//! until then the taps are dusty. In the Trophy Hall, until the west wing is
//! dug, its doorway is rubble and planks, the plate before it Tobbin's.

use super::super::ink::{Img, hash};
use super::super::kit::{self, RockKind, Tiles};
use super::{TILE, at, dusk, fire, home, stand};
use crate::drive::together_shooter::home::Station;
use crate::drive::together_shooter::tavern::{
    BAR, BOARD, DINADAN_AT, DRINKS, MAUD_AT, SIEGE, SIEGE_WORTHY, SONG_PLATES, SONGS, STAGE,
    TABLES, TAPS, WING_PLATE, drink, song, song_at, tap_at,
};
use crate::drive::together_shooter::{RoomKind, Run, Tile};
use crate::stage::world_viz::overworld::light::Light;

/// The tavern at rest.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let tick = run.tick as u32;
    let tiles = Tiles::get();
    let (pw, ph) = (room.cols as i32 * TILE, room.rows as i32 * TILE);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    // Planks, east to west, their ends staggered.
    for y in 0..ph {
        for x in 0..pw {
            if room.tile(x / TILE, y / TILE) == Tile::Wall {
                continue;
            }
            let row = y / 5;
            let end = (x + (hash(row, 3, 71) % 23) as i32) % 24 == 0;
            let ink = if y % 5 == 0 || end {
                'n'
            } else if hash(x / 6, row, 13).is_multiple_of(5) {
                'B'
            } else {
                'I'
            };
            cv.put(x, y, ink);
        }
    }
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            if room.tile(col, row) == Tile::Wall {
                cv.stamp(
                    tiles.rock(RockKind::Slate, hash(col, row, 23)),
                    col * TILE,
                    row * TILE,
                );
            }
        }
    }
    shelves(&mut cv);
    bar(&mut cv, run.home.residents.contains("maud"));
    if run.home.residents.contains("maud") {
        for (tap, plate) in TAPS.iter().enumerate() {
            home::plate(&mut cv, *plate, false, TAP_STATIONS[tap]);
        }
    }
    for table in TABLES {
        self::table(&mut cv, table);
    }
    board(&mut cv);
    stage(&mut cv, &mut lights);
    for (plate, at) in SONG_PLATES.iter().enumerate() {
        home::plate(&mut cv, *at, false, SONG_STATIONS[plate]);
    }
    super::hireling::plate(&mut cv);
    siege(&mut cv, tick);
    let barrels = kit::barrels();
    cv.stamp(&barrels, pw - 3 * TILE, ph - 2 * TILE - 2);
    cv.stamp(&barrels, pw - 4 * TILE - 6, ph - 2 * TILE + 3);
    // Lanterns on the walls, and the light they throw.
    for (col, row) in [(1, 4), (1, 9), (22, 4), (22, 10), (11, 12)] {
        let lamp = kit::lantern(true);
        let (x, y) = (col * TILE + (TILE - lamp.w) / 2, row * TILE - 6);
        cv.stamp(&lamp, x, y);
        lights.push(fire(x + 3, y + 3, 64.0, 0.42));
    }
    let (sc, sr, _, _) = SIEGE;
    lights.push(fire(sc * TILE + 8, sr * TILE + 4, 28.0, 0.2));
    dusk(&mut cv, &lights, ('j', 'o'));
    cv
}

/// The shelves behind the bar: two boards of bottles in every colour.
fn shelves(cv: &mut Img) {
    let (c, _, w, _) = BAR;
    let (x0, x1) = (c * TILE + 4, (c + w) * TILE - 4);
    for (k, y) in [17, 29].into_iter().enumerate() {
        cv.line(x0, y, x1, y, 'P');
        cv.line(x0, y + 1, x1, y + 1, 'b');
        let mut x = x0 + 2;
        while x < x1 - 3 {
            let pick = hash(x, k as i32, 57) % 6;
            let glass = ['C', 'z', 'R', 'Y', 'c', 'q'][pick as usize];
            let tall = 4 + (hash(x, k as i32, 9) % 3) as i32;
            cv.rect(x, y - tall, 2, tall, glass);
            cv.put(x, y - tall - 1, 'g');
            x += 4 + (hash(x, k as i32, 3) % 3) as i32;
        }
    }
}

/// The bar: a long counter, its front panelled, three brass taps, mugs;
/// dusty taps and a cobweb while the tap is dry.
fn bar(cv: &mut Img, kept: bool) {
    let (c, r, w, h) = BAR;
    let (x, y, bw, bh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    cv.rect(x, y + 3, bw, bh - 3, 'P');
    cv.line(x, y + 2, x + bw - 1, y + 2, 'o');
    cv.line(x, y + 3, x + bw - 1, y + 3, 'O');
    for px in (x + 6..x + bw - 4).step_by(16) {
        cv.line(px, y + 5, px, y + bh - 1, 'r');
    }
    cv.line(x, y + bh - 1, x + bw - 1, y + bh - 1, 'n');
    for k in 0..3 {
        let tx = x + bw / 2 - 20 + k * 20;
        let (tap, head) = if kept { ('5', '6') } else { ('J', 'G') };
        cv.rect(tx, y - 3, 2, 5, tap);
        cv.put(tx - 1, y - 3, head);
        cv.put(tx + 2, y - 3, head);
    }
    if kept {
        for k in 0..4 {
            let mx = x + 14 + k * 46;
            cv.rect(mx, y - 1, 4, 4, 'c');
            cv.line(mx, y - 2, mx + 3, y - 2, '9');
            cv.put(mx + 4, y, 'c');
        }
    } else {
        // A cobweb strung between the middle taps.
        let tx = x + bw / 2 - 20;
        for i in 0..20 {
            cv.put(
                tx + 1 + i,
                y - 4 + (i as f32 * 0.31).sin().abs() as i32,
                'V',
            );
        }
    }
}

/// A table: a slab of oak, a mug or two, a stool at each end.
fn table(cv: &mut Img, (c, r, w, h): (i32, i32, i32, i32)) {
    let (x, y, tw, th) = (c * TILE + 2, r * TILE + 4, w * TILE - 4, h * TILE - 8);
    cv.rect(x, y, tw, th, 'P');
    cv.frame(x, y, tw, th, 'r');
    cv.line(x + 1, y + 1, x + tw - 2, y + 1, 'o');
    for (sx, sy) in [(x - 8, y + th / 2 - 3), (x + tw + 2, y + th / 2 - 3)] {
        cv.rect(sx, sy, 6, 6, 'r');
        cv.frame(sx, sy, 6, 6, 'b');
    }
    for k in 0..2 {
        let mx = x + 8 + k * (tw - 20);
        cv.rect(mx, y + 6, 4, 4, 'c');
        cv.line(mx, y + 5, mx + 3, y + 5, '9');
    }
}

/// The rumour board on its posts: three notes pinned to it.
fn board(cv: &mut Img) {
    let (c, r, w, _) = BOARD;
    let (x, y, bw) = (c * TILE, r * TILE - 4, w * TILE);
    cv.rect(x + 3, y + 12, 2, 10, 'b');
    cv.rect(x + bw - 5, y + 12, 2, 10, 'b');
    cv.rect(x, y, bw, 14, 'P');
    cv.frame(x, y, bw, 14, 'B');
    for k in 0..3 {
        let nx = x + 4 + k * 14;
        cv.rect(nx, y + 2, 10, 10, '9');
        cv.put(nx + 4, y + 2, '7');
        for line in 0..3 {
            cv.line(nx + 2, y + 5 + line * 2, nx + 7, y + 5 + line * 2, 'g');
        }
    }
}

/// The Siege Perilous: a gilt, high-backed chair on a stone step, a red
/// seat, and a faint light about it that never quite goes out.
fn siege(cv: &mut Img, tick: u32) {
    let (c, r, _, _) = SIEGE;
    let (x, y) = (c * TILE, r * TILE);
    cv.rect(x - 3, y + 12, TILE + 6, 4, 'J');
    cv.line(x - 3, y + 12, x + TILE + 2, y + 12, 'h');
    let chair = Img::from_rows(&CHAIR);
    cv.stamp(&chair, x + (TILE - chair.w) / 2, y + 13 - chair.h);
    for k in 0..6 {
        if (tick / 7 + k).is_multiple_of(3) {
            continue;
        }
        let a = k as f32 / 6.0 * std::f32::consts::TAU + tick as f32 * 0.01;
        cv.put(
            x + 8 + (a.cos() * 12.0) as i32,
            y - 4 + (a.sin() * 9.0) as i32,
            '5',
        );
    }
}

/// The chair itself.
const CHAIR: [&str; 20] = [
    "...5..5..5...",
    "..55555555...",
    "..5HiiiiH5...",
    "..5iHHHHi5...",
    "..5iH55Hi5...",
    "..5iHHHHi5...",
    "..5iiiiii5...",
    "..5iHHHHi5...",
    "..5iiiiii5...",
    "..5iiiiii5...",
    ".55555555555.",
    ".5RRRRRRRRR5.",
    ".5pRRRRRRRp5.",
    ".55555555555.",
    ".5.5.....5.5.",
    ".5.5.....5.5.",
    ".5.5.....5.5.",
    ".5.5.....5.5.",
    ".5.5.....5.5.",
    ".a.a.....a.a.",
];

/// The taps' plates, as stations to the plate drawer; and the songs'.
const TAP_STATIONS: [Station; 3] = [Station::TapA, Station::TapB, Station::TapC];
const SONG_STATIONS: [Station; 3] = [Station::SongA, Station::SongB, Station::SongC];

/// Sir Dinadan's stage: a low dais of boards, footlights along its edge.
fn stage(cv: &mut Img, lights: &mut Vec<Light>) {
    let (c, r, w, h) = STAGE;
    let (x, y, sw, sh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    cv.rect(x, y + 2, sw, sh - 2, 'O');
    for px in (x..x + sw).step_by(6) {
        cv.line(px, y + 2, px, y + sh - 4, 'o');
    }
    cv.rect(x, y + sh - 4, sw, 4, 'r');
    cv.line(x, y + 2, x + sw - 1, y + 2, 't');
    for k in 0..5 {
        let fx = x + 4 + k * (sw - 8) / 4;
        cv.put(fx, y + sh - 5, '6');
        cv.put(fx, y + sh - 6, '5');
        lights.push(fire(fx, y + sh - 6, 18.0, 0.18));
    }
}

/// Sir Dinadan: a jester-knight in motley, his hat belled, his lute.
pub(super) const DINADAN: [&str; 18] = [
    ".5.......5.....",
    "..7.....q......",
    "..77...qq......",
    "...77.qq.......",
    "....7qq........",
    "....OOOO.......",
    "...OKOOKO......",
    "....OOOO.......",
    "....OrrO....b..",
    "...7777qqqq.b..",
    "..77777qqqqob..",
    ".O77777qqqoOo..",
    ".O7777qqqqoOoO.",
    "...7777qqqqoo..",
    "...7777qqqq....",
    "....77..qq.....",
    "....77..qq.....",
    "...nnn..nnn....",
];

/// A note, rising.
const NOTE: [&str; 4] = ["..h", "..h", "hhh", "hh."];

/// Sir Dinadan on his stage, notes rising once a song is asked for; a
/// song's plate lit under a knight.
fn dinadan(cv: &mut Img, run: &Run) {
    let (x, y) = at(DINADAN_AT.0 * 2.0, DINADAN_AT.1 * 2.0);
    stand(cv, &Img::from_rows(&DINADAN), x, y);
    if run.home.song.is_some() {
        let note = Img::from_rows(&NOTE);
        for k in 0..2u64 {
            let age = ((run.tick + k * 37) % 70) as i32;
            cv.stamp(&note, x + 6 + (k as i32) * 5, y - 22 - age / 3);
        }
    }
    for hero in run.players.values().filter(|h| h.hp > 0) {
        if let Some(plate) = song_at(hero.x, hero.y) {
            home::plate(cv, SONG_PLATES[plate], true, SONG_STATIONS[plate]);
        }
    }
}

/// Maud behind her bar, once she's home, and a tap's plate lit under a
/// knight; Sir Dinadan on his stage.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    super::hireling::tavern(cv, run);
    if !run.home.residents.contains("maud") {
        dinadan(cv, run);
        return;
    }
    for hero in run.players.values().filter(|h| h.hp > 0) {
        if let Some(tap) = tap_at(hero.x, hero.y) {
            home::plate(cv, TAPS[tap], true, TAP_STATIONS[tap]);
        }
    }
    dinadan(cv, run);
    let (x, y) = at(MAUD_AT.0 * 2.0, MAUD_AT.1 * 2.0);
    stand(cv, &super::rescues::sprite_of("maud"), x, y);
}

/// To a knight below the rumour board, the rumours; to one at the Siege
/// Perilous, its plaque.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let near = |(c, r, w, h): (i32, i32, i32, i32), reach: f32| {
        let (cx, cy) = (
            (c as f32 + w as f32 / 2.0) * 2.0,
            (r as f32 + h as f32 / 2.0) * 2.0,
        );
        run.players
            .values()
            .any(|p| p.hp > 0 && (p.x - cx).hypot(p.y - cy) < reach)
    };
    if near(BOARD, 4.5) {
        let mut lines = vec![("RUMOURS".to_string(), '9')];
        lines.extend(run.rumours().into_iter().map(|line| (line, 'h')));
        let board = home::board(&lines);
        let (c, r, w, _) = BOARD;
        let bx = ((c * TILE + w * TILE / 2) - board.w / 2).clamp(2, cv.w - board.w - 2);
        cv.stamp(&board, bx, r * TILE + 26);
    }
    // A knight at a tap: what it pours, what it costs, and the hold.
    if run.home.residents.contains("maud")
        && let Some((hero, tap)) = run
            .players
            .values()
            .filter(|h| h.hp > 0)
            .find_map(|h| Some((h, tap_at(h.x, h.y)?)))
    {
        let pour = &DRINKS[tap];
        let lines = match run.home.round.as_deref().and_then(drink) {
            Some(waiting) => vec![
                (pour.name.to_uppercase(), '9'),
                ("A ROUND WAITS ALREADY".to_string(), 'h'),
                (waiting.name.to_uppercase(), '5'),
            ],
            None => {
                let filled = hero.buying * 10 / crate::drive::together_shooter::home::BUY_HOLD;
                vec![
                    (pour.name.to_uppercase(), '9'),
                    (pour.does.to_uppercase(), 'h'),
                    (format!("{} GOLD", pour.price), '5'),
                    (
                        format!("HOLD F {}", "#".repeat(filled.min(10) as usize)),
                        'O',
                    ),
                ]
            }
        };
        let board = home::board(&lines);
        let (c, r, w, _) = TAPS[tap];
        let bx = ((c * TILE + w * TILE / 2) - board.w / 2).clamp(2, cv.w - board.w - 2);
        cv.stamp(&board, bx, (r + 1) * TILE + 4);
    }
    // A knight at a song's plate: what it does, what it costs, the hold.
    if let Some((hero, plate)) = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .find_map(|h| Some((h, song_at(h.x, h.y)?)))
    {
        let ask = &SONGS[plate];
        let lines = match run.home.song.as_deref().and_then(song) {
            Some(waiting) => vec![
                (ask.name.to_uppercase(), '9'),
                ("A SONG WAITS ALREADY".to_string(), 'h'),
                (waiting.name.to_uppercase(), '5'),
            ],
            None => {
                let filled = hero.buying * 10 / crate::drive::together_shooter::home::BUY_HOLD;
                vec![
                    (ask.name.to_uppercase(), '9'),
                    (ask.does.to_uppercase(), 'h'),
                    (format!("{} GOLD", ask.price), '5'),
                    (
                        format!("HOLD F {}", "#".repeat(filled.min(10) as usize)),
                        'O',
                    ),
                ]
            }
        };
        let board = home::board(&lines);
        // Below the plates, clear of Dinadan and his notes.
        let (c, r, w, h) = SONG_PLATES[plate];
        let bx = ((c * TILE + w * TILE / 2) - board.w / 2).clamp(2, cv.w - board.w - 2);
        cv.stamp(&board, bx, ((r + h) * TILE + 4).min(cv.h - board.h - 2));
    }
    super::hireling::board(cv, run);
    if near(SIEGE, 3.5) {
        let deeds = run.home.feats.len();
        let lines = [
            ("THE SIEGE PERILOUS".to_string(), '9'),
            ("ONLY THE WORTHIEST MAY SIT".to_string(), 'h'),
            (
                format!("THE REALM'S DEEDS: {deeds} OF {SIEGE_WORTHY}"),
                if deeds >= SIEGE_WORTHY { '5' } else { 'h' },
            ),
        ];
        // Over the chair, clear of the taps' boards and of whoever stands
        // beside it.
        let board = home::board(&lines);
        let (c, r, _, _) = SIEGE;
        cv.stamp(&board, (c * TILE - 8).max(2), r * TILE - board.h - 16);
    }
}

/// The Trophy Hall's west wall until the wing is dug: rubble heaped in the
/// doorway, planks nailed across it; and Tobbin's plate before it.
pub(super) fn rubble(cv: &mut Img, run: &Run) {
    let dug = run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Tavern);
    if !dug {
        let (x, y) = (0, 6 * TILE);
        for k in 0..14 {
            let (rx, ry) = (
                2 + (hash(k, 1, 33) % 12) as i32,
                y + 2 + (hash(k, 2, 33) % 28) as i32,
            );
            let lit = hash(k, 3, 33).is_multiple_of(2);
            cv.rect(x + rx, ry, 4, 3, if lit { 'J' } else { 'G' });
            cv.put(x + rx, ry + 2, 'g');
        }
        for (y0, y1) in [(y + 3, y + 27), (y + 27, y + 5)] {
            cv.line(x + 1, y0, x + 14, y1, 'o');
            cv.line(x + 1, y0 + 1, x + 14, y1 + 1, 'r');
        }
    }
    if run.home.next(Station::Wing).is_some() {
        home::plate(cv, WING_PLATE, false, Station::Wing);
    }
}

/// Tobbin's plate lit, and the West Wing's ledger, to a knight on it.
pub(super) fn wing_ledger(cv: &mut Img, run: &Run) {
    if run.home.next(Station::Wing).is_none() {
        return;
    }
    let (c, r, w, h) = WING_PLATE;
    let on = |x: f32, y: f32| {
        let (cc, rr) = (x / 2.0, y / 2.0);
        cc >= c as f32 && cc < (c + w) as f32 && rr >= r as f32 && rr < (r + h) as f32
    };
    let Some(hero) = run.players.values().find(|h| h.hp > 0 && on(h.x, h.y)) else {
        return;
    };
    home::plate(cv, WING_PLATE, true, Station::Wing);
    let board = home::ledger_board(run, Station::Wing, hero.buying);
    cv.stamp(&board, 4, (r + h) * TILE + 6);
}
