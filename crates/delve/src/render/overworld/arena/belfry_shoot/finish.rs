//! The end of the shoot: the clock's last tick holds the room, film bars
//! close in, the bell is rung and the winner called, the iris shuts; then
//! the loft over the belfry: three louvred windows on the night with
//! fireworks over the town, the great bell hanging in the middle one,
//! three steps rising, the winner laureled, paper and feathers falling, the
//! pot counting up. All of it from the shoot's `ended` tick, so every seat
//! sees it alike.

use super::super::super::ink::{Img, bayer, hash};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{NATIVE_H, NATIVE_W, enlarge, knights};
use super::{bits, hud::called, set};
use crate::motion::ease;
use crate::rules::folly::{BatShoot, HARM_PER_GOLD};
use crate::rules::Run;

/// Ticks the live finish plays before the scene cuts to the loft.
pub(super) const FINALE: u32 = 48;
/// Ticks the loft stands, and the ticks its iris takes to open.
pub(super) const HALL: u32 = 270;
const OPEN: u32 = 14;
/// Ticks the loft dissolves back into the room at the end.
const LEAVE: u32 = 12;
/// The ladder of the table's bats: how many each rung asks.
const TIERS: [(u16, &str); 3] = [(20, "BAT HUNTERS"), (40, "BELLRINGERS"), (60, "BELFRY LEGENDS")];

/// Ticks since the shoot ended, while the finish is still the show.
pub(super) fn age(shoot: &BatShoot, now: u32) -> Option<u32> {
    shoot.show.since_end(now).filter(|&e| e < FINALE + HALL)
}

/// The knights by harm, most first, ties to the lower seat.
fn order(shoot: &BatShoot) -> Vec<u32> {
    let mut ids: Vec<u32> = shoot.scores.keys().copied().collect();
    ids.sort_by_key(|id| (std::cmp::Reverse(shoot.scores[id]), *id));
    ids
}

/// The live finish, `e` ticks after the last: a flash, the film's bars, the
/// call, and the iris shutting for the loft.
pub(super) fn finale(over: &mut Img, run: &Run, shoot: &BatShoot, e: u32, focus: Option<u32>) {
    let Some(end) = shoot.show.ended else { return };
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
        let sub = if shoot.scores.len() < 2 || winner == 0 {
            String::new()
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 10))
        };
        juice::banner(over, 46, "TIME", &sub, e - 4, FINALE - 14, Tone::Warm, 3);
    }
    let out = ((e as f32 - (FINALE - 14) as f32) / 14.0).clamp(0.0, 1.0);
    juice::iris(over, (NATIVE_W / 2, NATIVE_H / 2 - 10), out, 'k');
}

/// Three arched windows in the back wall: `(left, right)` of each.
const WINDOWS: [(i32, i32); 3] = [(26, 106), (152, 232), (278, 358)];
/// The back wall's foot (the loft's floor begins under it).
const WALL: i32 = 112;

/// Whether a pixel of the back wall lies in a window's arch.
fn in_window(x: i32, y: i32) -> bool {
    WINDOWS.iter().any(|&(l, r)| {
        if x < l || x >= r || y < 10 || y >= WALL - 8 {
            return false;
        }
        // A round head: the top 40 rows are a half-ellipse.
        let (cx, half) = ((l + r) as f32 / 2.0, (r - l) as f32 / 2.0);
        if y < 50 {
            let dy = (50 - y) as f32 / 40.0;
            let w = half * (1.0 - dy * dy).max(0.0).sqrt();
            ((x as f32 + 0.5) - cx).abs() <= w
        } else {
            true
        }
    })
}

/// The night: a dithered sky going from deep to a pale rim at the town, a
/// moon over the middle window, stars, and the town's roofs along the foot.
fn sky(tick: u32) -> Img {
    let mut im = Img::black(NATIVE_W, WALL);
    for y in 0..WALL {
        let k = y as f32 / WALL as f32;
        for x in 0..NATIVE_W {
            let ink = if bayer(x, y) < k * 0.9 { 'q' } else if bayer(x, y) < k * 0.9 + 0.35 { 's' } else { 'k' };
            im.put(x, y, ink);
        }
    }
    for k in 0..46 {
        let (x, y) = ((hash(k, 1, 9) % NATIVE_W as u32) as i32, (hash(k, 2, 9) % 70) as i32);
        let twinkle = (tick / 9 + hash(k, 3, 9) % 7) % 7 == 0;
        im.put(x, y, if twinkle { 'W' } else { 'U' });
    }
    // The moon, in the right window's upper left.
    for dy in -9..=9i32 {
        for dx in -9..=9i32 {
            let d = dx * dx + dy * dy;
            if d <= 81 {
                let shade = if (dx + 3) * (dx + 3) + (dy - 1) * (dy - 1) < 70 { 'V' } else { 'W' };
                im.put(322 + dx, 36 + dy, if d > 64 { 'v' } else { shade });
            }
        }
    }
    // Roofs of the town, black on the sky, a lit window or two.
    let mut x = 0;
    while x < NATIVE_W {
        let w = 10 + (hash(x, 5, 3) % 14) as i32;
        let h = 8 + (hash(x, 6, 3) % 16) as i32;
        im.rect(x, WALL - h, w, h, 'k');
        for roof in 0..(w / 2) {
            im.rect(x + roof, WALL - h - roof / 2 - 1, w - roof * 2, 1, 'k');
        }
        if hash(x, 7, 3) % 3 == 0 {
            im.put(x + 3, WALL - h + 3, '6');
        }
        x += w;
    }
    im
}

/// The loft's wall and floor around the windows.
fn loft(im: &mut Img, age: u32, sky: &Img) {
    for y in 0..WALL {
        for x in 0..NATIVE_W {
            if in_window(x, y) {
                if let Some(c) = sky.get(x, y) {
                    im.set(x, y, c);
                }
                continue;
            }
            let course = (y / 14) % 2 * 12;
            let joint = y % 14 == 13 || (x + course) % 24 == 23;
            let ink = if joint {
                's'
            } else if bayer(x, y) < 0.16 + (y as f32 / WALL as f32) * 0.1 {
                's'
            } else {
                'x'
            };
            im.put(x, y, ink);
        }
    }
    // Each window's frame: a lit sill and a dark reveal, and louvre slats
    // standing in the lower part of it.
    for &(l, r) in &WINDOWS {
        im.rect(l - 3, WALL - 8, r - l + 6, 8, 'u');
        im.rect(l - 3, WALL - 8, r - l + 6, 1, 'v');
        for y in 10..WALL - 8 {
            for x in [l - 1, l, r - 1, r] {
                if in_window(x.clamp(l, r - 1), y) {
                    im.put(x, y, if x <= l { 'S' } else { 'g' });
                }
            }
        }
    }
    // A rafter across the head of the wall, and the floor boards.
    im.rect(0, 0, NATIVE_W, 9, 'B');
    im.rect(0, 0, NATIVE_W, 1, 'o');
    im.rect(0, 8, NATIVE_W, 1, 'p');
    for x in (0..NATIVE_W).step_by(48) {
        im.rect(x + 20, 0, 4, 9, 'g');
    }
    // The crowd: bats hanging along the rafter against the night, restless
    // at first and wild once the winner is called.
    let fever = ((age as f32 - 10.0) / 20.0).clamp(0.0, 1.0);
    for k in 0..17 {
        let h = hash(k, 3, 0x51);
        let x = 6 + k * 22 + (h % 9) as i32;
        let wild = fever > 0.0 && (age / 4 + h % 3) % 2 == 0;
        let hop = i32::from(fever > 0.0 && (age / 6 + h % 4) % 2 == 0);
        let sprite = enlarge(bits::crowd(wild), 2.0);
        im.stamp(&sprite, x, 9 - hop);
    }
    for y in WALL..NATIVE_H {
        for x in 0..NATIVE_W {
            let board = ((y - WALL) / 6) % 2;
            let seam = (y - WALL) % 6 == 5;
            let joint = (x + board * 20 + ((y - WALL) / 6) as i32 * 7) % 48 == 0;
            let ink = if seam || joint { 'k' } else if bayer(x, y) < 0.2 { 'n' } else { 'K' };
            im.put(x, y, ink);
        }
    }
    im.rect(0, WALL, NATIVE_W, 1, 'P');
    // Lanterns on the piers, flickering, a warm pool under each.
    for x in [10, 132, 255, 374] {
        im.rect(x - 3, 44, 7, 7, 'g');
        im.rect(x - 2, 45, 5, 5, 'Z');
        im.stamp(bits::flame(age / 4 + x as u32), x - 1, 41);
        for dy in -10..=10i32 {
            for dx in -16..=16i32 {
                let d = (dx * dx) as f32 / 256.0 + (dy * dy) as f32 / 100.0;
                if d < 1.0 && bayer(x + dx, 48 + dy) < (1.0 - d) * 0.22 && im.get(x + dx, 48 + dy).is_some_and(|p| p != crate::render::overworld::ink::BLACK) {
                    im.put(x + dx, 48 + dy, 'Z');
                }
            }
        }
    }
}

/// Light from above onto the winner, a dither behind the steps.
fn spot(im: &mut Img, cx: i32, floor: i32, k: f32) {
    for y in WALL..floor {
        let half = 22.0 + (y - WALL) as f32 * 0.6;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if bayer(x, y) < edge * edge * 0.3 * k {
                im.put(x, y, 'J');
            }
        }
    }
}

/// The loft for a seat, `e` ticks after the shoot ended.
pub(super) fn scene(run: &Run, shoot: &BatShoot, e: u32, focus: Option<u32>) -> Img {
    let Some(end) = shoot.show.ended else { return Img::black(NATIVE_W, NATIVE_H) };
    let age = e.saturating_sub(FINALE);
    let now = run.tick as u32;
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    let mut night = sky(now);
    // Fireworks over the town, three launchers in turn, each clipped to the
    // windows by laying them on the sky.
    if age >= 6 {
        for (j, (x, top, ramp)) in [(66.0, 30.0, ramps::GOLD), (192.0, 20.0, ramps::EMBER), (318.0, 34.0, ramps::HOLY)].into_iter().enumerate() {
            let t = age as i32 - 6 - j as i32 * 15;
            if t >= 0 {
                let round = (t / 50) as u32;
                juice::firework(&mut night, x + (round as f32 * 17.0) % 30.0 - 15.0, 100.0, top, (t % 50) as u32, 29 * j as u32 + round * 5, ramp, age as u64);
            }
        }
    }
    loft(&mut im, age, &night);
    // The great bell hangs in the left window, rung as the shoot ended and
    // again as the winner is crowned.
    let bell = enlarge(bits::great(), 1.6);
    let turn = set::swing(shoot, now) * 1.1;
    let swung = set::turned(&bell, (bell.w as f32 / 2.0, 2.0), turn);
    let ox = if turn.abs() < 0.012 { 0 } else { 12 };
    im.stamp(&swung, 66 - bell.w / 2 - ox, 9);
    let ranks = order(shoot);
    let winner = ranks.first().copied().unwrap_or(0);
    let rivals = ranks.len() > 1;
    let (cx, floor) = (NATIVE_W / 2, 182);
    spot(&mut im, cx, floor, ((age as f32 - 6.0) / 12.0).clamp(0.0, 1.0));
    let places: Vec<Place> = ranks
        .iter()
        .take(3)
        .filter_map(|&id| {
            let hero = run.players.get(&id)?;
            Some(Place { sprite: knights::figure(hero, id), name: called(run, id, focus, 9), line: format!("{} PTS", shoot.scores[&id]) })
        })
        .collect();
    let head = juice::podium(&mut im, (0, 0, NATIVE_W, floor + 6), &places, age);
    // The laurel over the winner once they have stood a moment.
    if age >= 44 && rivals {
        let k = (((age - 44) as f32) / 10.0).min(1.0);
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
        let line = format!("{} {} {}", k + 1, called(run, id, focus, 8), shoot.scores[&id]);
        let t = juice::big_text_img(&line, 1, if focus == Some(id) { 'T' } else { 'h' }, 'k');
        im.stamp(&t, 8, 120 + (k as i32 - 3) * 12);
    }
    if age >= 14 {
        juice::confetti(&mut im, (0, 0, NATIVE_W, NATIVE_H), age, 9, 140);
        // Feathers from the rafters drifting down among the paper.
        let mut pen = Pen::new(&mut im, 200);
        let feathers = Spray { gravity: 0.01, drag: 0.02, stagger: 4.0, ..Spray::round(77, 24, (192.0, 10.0), (0.5, 2.0), (60.0, 110.0)) };
        feathers.each(age as f32 - 14.0, |m| pfx::mote(&mut pen, &m, Shape::Petal, ramps::EARTH, 1, now as u64));
    }
    // The winner, big, off the top.
    if age >= 6 {
        let title = if !rivals {
            "BELFRY CLEARED".to_string()
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 12))
        };
        let tier = TIERS.iter().rev().find(|&&(need, _)| shoot.show.team >= need).map_or("A QUIET NIGHT", |&(_, name)| name);
        let table = format!("{} BATS  {tier}", shoot.show.team);
        juice::banner(&mut im, 18, &title, &table, age - 6, 64, Tone::Warm, 3);
    }
    // What the seat looking takes home, counting up; and the table's tally.
    if age >= 24 {
        let mine = focus.and_then(|id| shoot.scores.get(&id).map(|&n| (id, n)));
        let gold = mine.map_or(0, |(id, n)| n / HARM_PER_GOLD + if id == winner && rivals { end.pot } else { 0 });
        let shown = (gold as f32 * ease::out(((age - 24) as f32 / 40.0).min(1.0))) as u32;
        let tag = juice::big_text_img(&format!("+{shown} GOLD"), 2, 'T', 'B');
        im.stamp(&tag, NATIVE_W / 2 - tag.w / 2, 205);
        if rivals && age >= 30 {
            let line = if focus == Some(winner) { "YOU TAKE THE POT" } else { "THE POT IS THEIRS" };
            let t = juice::big_text_img(line, 1, 'h', 'k');
            im.stamp(&t, NATIVE_W - 6 - t.w, 212);
        }
    }
    if age >= 30 {
        let (best, who) = shoot.show.best.iter().map(|(&id, &n)| (n, id)).max().unwrap_or((0, 0));
        let table = juice::big_text_img(&format!("TABLE {} BATS", shoot.show.team), 1, 'o', 'k');
        im.stamp(&table, 6, 203);
        // A reason to go again: the next rung of the table's ladder.
        if let Some(&(need, name)) = TIERS.iter().find(|&&(need, _)| shoot.show.team < need) {
            let hook = juice::big_text_img(&format!("NEXT {name} {need}"), 1, 'h', 'k');
            im.stamp(&hook, 6, 212);
        }
        if best >= 3 {
            let line = format!("BEST RUN {} {}", best, called(run, who, focus, 8));
            let t = juice::big_text_img(&line, 1, 'o', 'k');
            im.stamp(&t, NATIVE_W - 6 - t.w, 203);
        }
    }
    // The way home: a thin bar filling along the bottom edge.
    let filled = (NATIVE_W as u32 * age.min(HALL) / HALL) as i32;
    im.rect(0, NATIVE_H - 1, filled, 1, 'o');
    // The iris opening from the finale's black, and the loft dissolving at the end.
    juice::iris(&mut im, (NATIVE_W / 2, 100), 1.0 - ((e as f32 - FINALE as f32) / OPEN as f32).clamp(0.0, 1.0), 'k');
    let left = FINALE + HALL - e;
    if left < LEAVE {
        juice::dissolve(&mut im, 1.0 - left as f32 / LEAVE as f32);
    }
    im
}
