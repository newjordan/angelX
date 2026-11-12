//! The end of the minute: the last tick holds the room, film bars close in,
//! the winner is called, the iris shuts; then the reading room over the
//! drowned hall: a wall of books with a great arched window on the night and
//! fireworks over it, owls cheering along the gallery rail, three steps
//! rising, the winner laureled, paper falling, the pot counting up. All of
//! it from the hop's `ended` tick, so every seat sees it alike; every word
//! is a claim in the screen's ledger.

use super::super::super::ink::{Img, bayer, hash, text_width};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{NATIVE_H, NATIVE_W, enlarge, knights};
use super::words::Ledger;
use super::{bits, hud::called, set};
use crate::motion::ease;
use crate::rules::hop::{CROSSINGS_PER_GOLD, Hop};
use crate::rules::hop_show::ARCHIVE_GOLD;
use crate::rules::Run;

/// Ticks the live finish plays before the scene cuts to the reading room.
pub(super) const FINALE: u32 = 48;
/// Ticks the reading room stands, and the ticks its iris takes to open.
pub(super) const HALL: u32 = 270;
const OPEN: u32 = 14;
/// Ticks the room dissolves back into the hall at the end.
const LEAVE: u32 = 12;

/// Ticks since the minute ended, while the finish is still the show.
pub(super) fn age(h: &Hop, now: u32) -> Option<u32> {
    h.show.since_end(now).filter(|&e| e < FINALE + HALL)
}

/// The knights by crossings, most first, ties to the lower seat.
fn order(h: &Hop) -> Vec<u32> {
    let mut ids: Vec<u32> = h.crossings.keys().copied().collect();
    ids.sort_by_key(|id| (std::cmp::Reverse(h.crossings[id]), *id));
    ids
}

/// The live finish, `e` ticks after the last: a flash, the film's bars, the
/// call, and the iris shutting for the reading room.
pub(super) fn finale(led: &mut Ledger, over: &mut Img, run: &Run, h: &Hop, e: u32, focus: Option<u32>) {
    let Some(end) = h.show.ended else { return };
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
        let sub = if h.crossings.len() < 2 || winner == 0 {
            String::new()
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 10))
        };
        led.banner(over, "finale", 46, "TIME", &sub, e - 4, FINALE - 14, Tone::Warm, 3);
    }
    let out = ((e as f32 - (FINALE - 14) as f32) / 14.0).clamp(0.0, 1.0);
    juice::iris(over, (NATIVE_W / 2, NATIVE_H / 2 - 10), out, 'k');
}

/// The arched window's span: left, right, top, and the foot of the glass.
const GLASS: (i32, i32, i32, i32) = (140, 244, 10, 98);

/// Whether a pixel lies in the window's arch.
fn in_glass(x: i32, y: i32) -> bool {
    let (l, r, top, foot) = GLASS;
    if x < l || x >= r || y < top || y >= foot {
        return false;
    }
    let (cx, half) = ((l + r) as f32 / 2.0, (r - l) as f32 / 2.0);
    let head = top + half as i32;
    if y < head {
        let dy = (head - y) as f32 / half;
        ((x as f32 + 0.5) - cx).abs() <= half * (1.0 - dy * dy).max(0.0).sqrt()
    } else {
        true
    }
}

/// The night in the window: a dithered sky, a moon, stars, the roofs.
fn night(tick: u32) -> Img {
    let (l, r, top, foot) = GLASS;
    let mut im = Img::black(NATIVE_W, foot);
    for y in top..foot {
        let k = (y - top) as f32 / (foot - top) as f32;
        for x in l..r {
            let ink = if bayer(x, y) < k * 0.9 { 'q' } else if bayer(x, y) < k * 0.9 + 0.35 { 's' } else { 'k' };
            im.put(x, y, ink);
        }
    }
    for k in 0..26 {
        let (x, y) = (l + (hash(k, 1, 9) % (r - l) as u32) as i32, top + (hash(k, 2, 9) % 46) as i32);
        let twinkle = (tick / 9 + hash(k, 3, 9) % 7) % 7 == 0;
        im.put(x, y, if twinkle { 'W' } else { 'U' });
    }
    for dy in -8..=8i32 {
        for dx in -8..=8i32 {
            let d = dx * dx + dy * dy;
            if d <= 64 {
                im.put(l + 22 + dx, top + 30 + dy, if d > 49 { 'v' } else if (dx + 3) * (dx + 3) + (dy - 1) * (dy - 1) < 50 { 'V' } else { 'W' });
            }
        }
    }
    let mut x = l;
    while x < r {
        let w = 9 + (hash(x, 5, 3) % 12) as i32;
        let hgt = 6 + (hash(x, 6, 3) % 14) as i32;
        im.rect(x, foot - hgt, w, hgt, 'k');
        if hash(x, 7, 3) % 3 == 0 {
            im.put(x + 3, foot - hgt + 3, '6');
        }
        x += w;
    }
    im
}

/// The reading room's wall of books, its window, and the gallery rail.
fn room(im: &mut Img, age: u32, glass: &Img) {
    for row in 0..7 {
        set::bookcase_row(im, row * 16, NATIVE_W, 40 + row as u32, row >= 4);
    }
    // The window: the night in an arch, gilt round it, a stone sill.
    let (l, r, top, foot) = GLASS;
    for y in top - 3..foot + 3 {
        for x in l - 3..r + 3 {
            if in_glass(x, y) {
                if let Some(c) = glass.get(x, y) {
                    im.set(x, y, c);
                }
            } else if (in_glass(x - 1, y) || in_glass(x + 1, y) || in_glass(x, y - 1) || in_glass(x, y + 1) || in_glass(x - 2, y) || in_glass(x + 2, y) || in_glass(x, y - 2))
                && y < foot + 3
            {
                im.put(x, y, if (x + y) % 5 == 0 { '6' } else { '4' });
            }
        }
    }
    im.rect(l - 6, foot, r - l + 12, 6, 'u');
    im.rect(l - 6, foot, r - l + 12, 1, 'v');
    // The gallery rail the owls stand on.
    im.rect(0, 104, NATIVE_W, 8, 'B');
    im.rect(0, 104, NATIVE_W, 1, 'o');
    im.rect(0, 111, NATIVE_W, 1, 'b');
    for x in (10..NATIVE_W).step_by(46) {
        im.rect(x, 105, 3, 7, 'p');
    }
    // The floor: dark boards, a rug under the steps.
    for y in 112..NATIVE_H {
        for x in 0..NATIVE_W {
            let board = ((y - 112) / 6) % 2;
            let seam = (y - 112) % 6 == 5;
            let joint = (x + board * 20 + ((y - 112) / 6) * 7) % 48 == 0;
            im.put(x, y, if seam || joint { 'k' } else if bayer(x, y) < 0.2 { 'n' } else { 'K' });
        }
    }
    for y in 150..206 {
        for x in 24..NATIVE_W - 24 {
            let edge = x < 28 || x >= NATIVE_W - 28 || y < 154 || y >= 202;
            let ink = if edge { 'a' } else if (x + y) % 9 == 0 { 'P' } else if bayer(x, y) < 0.5 { 'p' } else { 'B' };
            if bayer(x + y, y) < 0.9 || edge {
                im.put(x, y, ink);
            }
        }
    }
    // Lamps on stands at either side, flickering.
    for x in [20, NATIVE_W - 22] {
        im.rect(x, 126, 2, 40, 'g');
        im.rect(x - 4, 120, 10, 7, 'g');
        im.rect(x - 3, 121, 8, 5, 'Z');
        im.stamp(bits::flame(age / 4 + x as u32), x, 122);
        for dy in -12..=12i32 {
            for dx in -18..=18i32 {
                let d = (dx * dx) as f32 / 324.0 + (dy * dy) as f32 / 144.0;
                if d < 1.0 && bayer(x + dx, 124 + dy) < (1.0 - d) * 0.22 && im.get(x + dx, 124 + dy).is_some_and(|p| p != crate::render::overworld::ink::BLACK) {
                    im.put(x + dx, 124 + dy, 'Z');
                }
            }
        }
    }
}

/// Light from above onto the winner, a dither behind the steps.
fn spot(im: &mut Img, cx: i32, floor: i32, k: f32) {
    for y in 112..floor {
        let half = 22.0 + (y - 112) as f32 * 0.6;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if bayer(x, y) < edge * edge * 0.3 * k {
                im.put(x, y, 'J');
            }
        }
    }
}

/// The reading room for a seat, `e` ticks after the minute ended.
pub(super) fn scene(led: &mut Ledger, run: &Run, h: &Hop, e: u32, focus: Option<u32>) -> Img {
    let Some(end) = h.show.ended else { return Img::black(NATIVE_W, NATIVE_H) };
    let age = e.saturating_sub(FINALE);
    let now = run.tick as u32;
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    let mut glass = night(now);
    // Fireworks over the roofs, three launchers in turn, laid on the sky so
    // the window clips them.
    if age >= 6 {
        let (l, r, top, foot) = GLASS;
        for (j, (x, apex, ramp)) in [(l as f32 + 24.0, top as f32 + 30.0, ramps::GOLD), ((l + r) as f32 / 2.0, top as f32 + 18.0, ramps::EMBER), (r as f32 - 24.0, top as f32 + 34.0, ramps::HOLY)].into_iter().enumerate() {
            let t = age as i32 - 6 - j as i32 * 15;
            if t >= 0 {
                let round = (t / 50) as u32;
                juice::firework(&mut glass, x + (round as f32 * 11.0) % 20.0 - 10.0, foot as f32 - 4.0, apex, (t % 50) as u32, 29 * j as u32 + round * 5, ramp, age as u64);
            }
        }
    }
    room(&mut im, age, &glass);
    // The owls along the rail: watching, then wild once the winner is called.
    let fever = age >= 10;
    for k in 0..9 {
        let hsh = hash(k, 3, 0x61);
        let x = 8 + k * 43 + (hsh % 7) as i32;
        let frame = if fever && (age / 5 + hsh % 3) % 2 == 0 { 2 } else if (age / 3 + hsh % 40) % 33 == 0 { 1 } else { 0 };
        let jump = if fever && (age / 6 + hsh % 4) % 2 == 0 { 3 } else { 0 };
        let owl = enlarge(bits::owl(frame), 2.0);
        im.stamp(&owl, x, 104 - owl.h - jump);
    }
    let ranks = order(h);
    let winner = ranks.first().copied().unwrap_or(0);
    let rivals = ranks.len() > 1;
    let (cx, floor) = (NATIVE_W / 2, 180);
    spot(&mut im, cx, floor, ((age as f32 - 6.0) / 12.0).clamp(0.0, 1.0));
    let places: Vec<Place> = ranks
        .iter()
        .take(3)
        .filter_map(|&id| {
            let hero = run.players.get(&id)?;
            Some(Place { sprite: knights::figure(hero, id), name: called(run, id, focus, 9), line: format!("{} ACROSS", h.crossings[&id]) })
        })
        .collect();
    // The kit draws each rank, name and score on its step: claim their boxes.
    let area = (0, 0, NATIVE_W, floor + 6);
    for stand in juice::podium_stands((NATIVE_W, NATIVE_H), area, &places) {
        if age >= stand.words_at {
            led.claim("podium rank", stand.rank);
            led.say("podium name", stand.name, &stand.name_text);
            led.say("podium line", stand.line, &stand.line_text);
        }
    }
    let head = juice::podium(&mut im, area, &places, age);
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
    // Rivals below the podium, small and plain: their seat's ink and count.
    for (k, &id) in ranks.iter().enumerate().skip(3) {
        if age < 24 + k as u32 * 3 {
            continue;
        }
        let y = 120 + (k as i32 - 3) * 11;
        im.rect(8, y + 1, 5, 5, knights::seat_mark(id));
        led.small(&mut im, "rival", 16, y, &format!("{} {} {}", k + 1, called(run, id, focus, 8), h.crossings[&id]), if focus == Some(id) { 'T' } else { 'h' });
    }
    if age >= 14 {
        juice::confetti(&mut im, (0, 0, NATIVE_W, NATIVE_H), age, 9, 140);
        // Loose pages from the shelves, drifting down among the paper.
        let mut pen = Pen::new(&mut im, 220);
        let pages = Spray { gravity: 0.011, drag: 0.025, stagger: 4.0, ..Spray::round(77, 22, (192.0, 4.0), (0.5, 2.4), (60.0, 110.0)) };
        pages.each(age as f32 - 14.0, |m| pfx::mote(&mut pen, &m, Shape::Flake, ramps::BONE, 1, now as u64));
    }
    // The winner, big, off the top.
    if age >= 6 {
        let title = if !rivals {
            if h.crossings.values().all(|&n| n == 0) { "TIME UP".to_string() } else { "WELL HOPPED".to_string() }
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 12))
        };
        let sub = if end.waves > 0 { format!("ARCHIVE LIT {}", end.waves) } else { format!("{} ACROSS", h.show.team) };
        led.banner(&mut im, "winner", 14, &title, &sub, age - 6, 64, Tone::Warm, 3);
    }
    // What the seat looking takes home, counting up; and the table's tally.
    if age >= 24 {
        let mine = focus.and_then(|id| h.crossings.get(&id).map(|&n| (id, n)));
        let gold = mine.map_or(0, |(id, n)| {
            u32::from(n / CROSSINGS_PER_GOLD) + u32::from(end.waves) * ARCHIVE_GOLD + if id == winner && rivals { end.pot } else { 0 }
        });
        let shown = (gold as f32 * ease::out(((age - 24) as f32 / 40.0).min(1.0))) as u32;
        led.big(&mut im, "gold", NATIVE_W / 2, 205, &format!("+{shown} GOLD"), 2, 'T', 'B');
    }
    if age >= 30 {
        led.small(&mut im, "table", 6, 203, &format!("TABLE {} ACROSS", h.show.team), 'o');
        // A reason to go again: light the archive (once more).
        let hook = if end.waves == 0 { "LIGHT THE ARCHIVE".to_string() } else { format!("LIGHT IT {} TIMES", end.waves + 1) };
        led.small(&mut im, "again", 6, 212, &hook, 'h');
        let fresh = end.most > end.was;
        let best = if fresh { format!("NEW BEST {}", end.most) } else { format!("BEST {}", end.was) };
        let flash = fresh && age / 6 % 2 == 0;
        led.small(&mut im, "best", NATIVE_W - 6 - text_width(&best), 203, &best, if fresh { if flash { 'T' } else { '6' } } else { 'o' });
        if rivals {
            let line = if focus == Some(winner) { "YOU TAKE THE POT" } else { "THE POT IS THEIRS" };
            led.small(&mut im, "pot line", NATIVE_W - 6 - text_width(line), 212, line, 'h');
        }
    }
    // The way home: a thin bar filling along the bottom edge.
    let filled = (NATIVE_W as u32 * age.min(HALL) / HALL) as i32;
    im.rect(0, NATIVE_H - 1, filled, 1, 'o');
    // The iris opening from the finale's black, and the room dissolving at the end.
    juice::iris(&mut im, (NATIVE_W / 2, 100), 1.0 - ((e as f32 - FINALE as f32) / OPEN as f32).clamp(0.0, 1.0), 'k');
    let left = FINALE + HALL - e;
    if left < LEAVE {
        juice::dissolve(&mut im, 1.0 - left as f32 / LEAVE as f32);
    }
    im
}
