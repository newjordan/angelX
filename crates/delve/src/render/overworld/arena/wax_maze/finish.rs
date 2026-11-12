//! The end of the maze: the last drop (or the candle) holds the room, film
//! bars close in, the winner is called, the iris shuts; then a crypt hall
//! with the little green kings on their feet, three steps rising, the winner
//! laureled, fireworks and paper, the pot counting up. All of it from the
//! maze's `ended` tick, so every seat sees it alike.

use super::super::super::ink::{Img, bayer, hash};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::ramps;
use super::super::{NATIVE_H, NATIVE_W, enlarge, knights};
use super::hud::called;
use super::wax_art::{self, KingMood};
use crate::motion::ease;
use crate::rules::folly::{Temper, WAX_PER_GOLD, WaxMaze};
use crate::rules::Run;

/// Ticks the live finish plays before the scene cuts to the hall.
pub(super) const FINALE: u32 = 48;
/// Ticks the hall stands, and the ticks its iris takes to open.
pub(super) const HALL: u32 = 270;
const OPEN: u32 = 14;
/// Ticks the hall dissolves back into the maze at the end.
const LEAVE: u32 = 12;

/// Ticks since the maze ended, while the finish is still the show.
pub(super) fn age(maze: &WaxMaze, now: u32) -> Option<u32> {
    maze.show.since_end(now).filter(|&e| e < FINALE + HALL)
}

/// The knights by wax eaten, most first, ties to the lower seat.
fn order(maze: &WaxMaze) -> Vec<u32> {
    let mut ids: Vec<u32> = maze.eaten.keys().copied().collect();
    ids.sort_by_key(|id| (std::cmp::Reverse(maze.eaten[id]), *id));
    ids
}

/// The live finish, `e` ticks after the last drop: a flash, the film's bars,
/// the call, and the iris shutting for the hall.
pub(super) fn finale(over: &mut Img, run: &Run, maze: &WaxMaze, e: u32, focus: Option<u32>) {
    let Some(end) = maze.show.ended else { return };
    if e < 8 {
        let k = (1.0 - e as f32 / 8.0) * 0.08;
        for y in 0..NATIVE_H {
            for x in 0..NATIVE_W {
                if bayer(x, y) < k {
                    over.put(x, y, 'i');
                }
            }
        }
    }
    let bars = (ease::out(((e as f32 - 2.0) / 10.0).clamp(0.0, 1.0)) * 18.0) as i32;
    for y in 0..bars {
        over.rect(0, y, NATIVE_W, 1, 'k');
        over.rect(0, NATIVE_H - 1 - y, NATIVE_W, 1, 'k');
    }
    if (4..FINALE - 10).contains(&e) {
        let winner = u32::from(end.winner);
        let sub = if maze.eaten.len() < 2 || winner == 0 {
            String::new()
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 10))
        };
        let title = if end.burnt { "CANDLE OUT" } else { "LAST WAX" };
        juice::banner(over, 46, title, &sub, e - 4, FINALE - 14, Tone::Warm, 3);
    }
    let out = ((e as f32 - (FINALE - 14) as f32) / 14.0).clamp(0.0, 1.0);
    juice::iris(over, (NATIVE_W / 2, NATIVE_H / 2 - 10), out, 'k');
}

/// One little green king as a crowd, every other pixel of its robe a veil.
fn king(temper: Temper, tick: u32) -> Img {
    let (veil, solid) = wax_art::king(temper, KingMood::Hunt, tick);
    let mut out = Img::new(veil.w, veil.h);
    for y in 0..veil.h {
        for x in 0..veil.w {
            if (x + y) % 2 == 0
                && let Some(c) = veil.get(x, y)
            {
                out.set(x, y, c);
            }
        }
    }
    out.stamp(&solid, 0, 0);
    out
}

/// The hall: back wall of old stone with candles, a flagged floor.
fn hall(im: &mut Img, age: u32) {
    for y in 0..112 {
        for x in 0..NATIVE_W {
            // Courses of stone, dithered dark.
            let course = (y / 14) % 2 * 12;
            let joint = y % 14 == 13 || (x + course) % 24 == 23;
            let ink = if joint {
                '('
            } else if bayer(x, y) < 0.16 + (y as f32 / 112.0) * 0.1 {
                '('
            } else {
                'd'
            };
            im.put(x, y, ink);
        }
    }
    for y in 112..NATIVE_H {
        for x in 0..NATIVE_W {
            let flag = ((x / 24) + (y - 112) / 12) % 2 == 0;
            if bayer(x, y) < 0.14 + if flag { 0.12 } else { 0.0 } {
                im.put(x, y, 'g');
            }
        }
    }
    // The back wall's foot, a lit line.
    im.rect(0, 111, NATIVE_W, 1, ')');
    // Candles along the wall, their flames flickering, a warm pool under each.
    for k in 0..5 {
        let x = 38 + k * 77;
        im.rect(x - 2, 56, 5, 18, 'T');
        im.rect(x + 1, 56, 2, 18, 't');
        im.put(x - 2, 56, 'H');
        im.stamp(super::bits::flame(age / 4 + k as u32), x - 1, 50);
        for dy in -10..=10i32 {
            for dx in -16..=16i32 {
                let d = (dx * dx) as f32 / 256.0 + (dy * dy) as f32 / 100.0;
                if d < 1.0 && bayer(x + dx, 52 + dy) < (1.0 - d) * 0.22 && im.get(x + dx, 52 + dy).is_some_and(|p| p != crate::render::overworld::ink::BLACK) {
                    im.put(x + dx, 52 + dy, 'Z');
                }
            }
        }
    }
}

/// Light from above onto the winner, a dither behind the steps.
fn spot(im: &mut Img, cx: i32, floor: i32, k: f32) {
    for y in 0..floor {
        let half = 22.0 + y as f32 * 0.5;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if bayer(x, y) < edge * edge * 0.3 * k {
                im.put(x, y, 'J');
            }
        }
    }
}

/// The hall for a seat, `e` ticks after the maze ended.
pub(super) fn scene(run: &Run, maze: &WaxMaze, e: u32, focus: Option<u32>) -> Img {
    let Some(end) = maze.show.ended else { return Img::black(NATIVE_W, NATIVE_H) };
    let age = e.saturating_sub(FINALE);
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    hall(&mut im, age);
    let ranks = order(maze);
    let winner = ranks.first().copied().unwrap_or(0);
    let rivals = ranks.len() > 1;
    let (cx, floor) = (NATIVE_W / 2, 182);
    // Fireworks up past the wall, three launchers in turn.
    if age >= 8 {
        for (j, (x, top, ramp)) in [(72.0, 40.0, ramps::GOLD), (192.0, 26.0, ramps::EMBER), (312.0, 44.0, ramps::HOLY)].into_iter().enumerate() {
            let t = age as i32 - 8 - j as i32 * 17;
            if t >= 0 {
                let round = (t / 52) as u32;
                juice::firework(&mut im, x + (round as f32 * 23.0) % 40.0 - 20.0, 100.0, top, (t % 52) as u32, 31 * j as u32 + round * 7, ramp, age as u64);
            }
        }
    }
    // The little green kings on their feet along the back, hopping.
    let fever = ((age as f32 - 14.0) / 20.0).clamp(0.0, 1.0);
    for (k, &temper) in Temper::ALL.iter().cycle().take(10).enumerate() {
        let h = hash(k as i32, 7, 0xc7);
        let side = if k % 2 == 0 { 1 } else { -1 };
        let x = cx + side * (62 + (k as i32 / 2) * 28 + (h % 5) as i32) - 14;
        let hop = if fever > 0.0 && (age / 5 + h % 4) % 2 == 0 { 3 } else { 0 };
        let tall = enlarge(&king(temper, age / 6 + k as u32), 2.0);
        im.stamp(&tall, x, 118 - tall.h + 6 - hop - (k as i32 / 4) * 6);
    }
    spot(&mut im, cx, floor, ((age as f32 - 6.0) / 12.0).clamp(0.0, 1.0));
    let places: Vec<Place> = ranks
        .iter()
        .take(3)
        .filter_map(|&id| {
            let hero = run.players.get(&id)?;
            Some(Place { sprite: knights::figure(hero, id), name: called(run, id, focus, 9), line: format!("{} WAX", maze.eaten[&id]) })
        })
        .collect();
    let head = juice::podium(&mut im, (0, 0, NATIVE_W, floor + 6), &places, age);
    // The laurel over the winner once they have stood a moment.
    if age >= 34 && rivals {
        let k = (((age - 34) as f32) / 10.0).min(1.0);
        let bob = ((age as f32 * 0.2).sin() * 1.5) as i32;
        let (x, y) = (head.0, head.1 - 8 + ((1.0 - k) * -20.0) as i32 + bob);
        for a in 0..11 {
            let t = a as f32 / 10.0;
            let (dx, dy) = ((t * 2.0 - 1.0) * 22.0, -(1.0 - (t * 2.0 - 1.0).powi(2)) * 9.0 + 6.0);
            im.rect(x + dx as i32 - 2, y + dy as i32, 5, 3, if a % 2 == 0 { 't' } else { 'O' });
            im.rect(x + dx as i32 - 1, y + dy as i32, 2, 1, 'T');
        }
        im.rect(x - 2, y - 10, 5, 6, '@');
        im.rect(x - 4, y - 7, 9, 3, '5');
    }
    // Rivals below the podium, small and plain.
    for (k, &id) in ranks.iter().enumerate().skip(3) {
        if age < 24 + k as u32 * 3 {
            continue;
        }
        let line = format!("{} {} {}", k + 1, called(run, id, focus, 8), maze.eaten[&id]);
        let t = juice::big_text_img(&line, 1, if focus == Some(id) { 'T' } else { 'h' }, 'k');
        im.stamp(&t, 8, 49 + (k as i32 - 3) * 12);
    }
    if age >= 14 {
        juice::confetti(&mut im, (0, 0, NATIVE_W, NATIVE_H), age, 9, 150);
    }
    // The winner, big, off the top.
    if age >= 6 {
        let title = if !rivals {
            if end.burnt { "CANDLE OUT".to_string() } else { "MAZE CLEARED".to_string() }
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 12))
        };
        juice::banner(&mut im, 8, &title, "", age - 6, 400, Tone::Warm, 3);
    }
    // What the seat looking takes home, counting up; and the table's best run.
    if age >= 24 {
        let mine = focus.and_then(|id| maze.eaten.get(&id).map(|&n| (id, n)));
        let gold = mine.map_or(0, |(id, n)| u32::from(n / WAX_PER_GOLD) + if id == winner && rivals { end.pot } else { 0 });
        let shown = (gold as f32 * ease::out(((age - 24) as f32 / 40.0).min(1.0))) as u32;
        let tag = juice::big_text_img(&format!("+{shown} GOLD"), 2, 'T', 'B');
        im.stamp(&tag, NATIVE_W / 2 - tag.w / 2, 205);
        if rivals && age >= 30 {
            let line = if focus == Some(winner) { "YOU TAKE THE POT" } else { "THE POT IS THEIRS" };
            let t = juice::big_text_img(line, 1, 'h', 'k');
            im.stamp(&t, NATIVE_W - 6 - t.w, 209);
        }
    }
    if age >= 30 {
        let (best, who) = maze.show.best.iter().map(|(&id, &n)| (n, id)).max().unwrap_or((0, 0));
        if best >= 10 {
            let line = format!("BEST RUN {} {}", best, called(run, who, focus, 8));
            let t = juice::big_text_img(&line, 1, 'o', 'k');
            im.stamp(&t, 6, 209);
        }
    }
    // The way home: a thin bar filling along the bottom edge.
    let filled = (NATIVE_W as u32 * age.min(HALL) / HALL) as i32;
    im.rect(0, NATIVE_H - 1, filled, 1, 'o');
    // The iris opening from the finale's black, and the hall dissolving at the end.
    juice::iris(&mut im, (NATIVE_W / 2, 100), 1.0 - ((e as f32 - FINALE as f32) / OPEN as f32).clamp(0.0, 1.0), 'k');
    let left = FINALE + HALL - e;
    if left < LEAVE {
        juice::dissolve(&mut im, 1.0 - left as f32 / LEAVE as f32);
    }
    im
}
