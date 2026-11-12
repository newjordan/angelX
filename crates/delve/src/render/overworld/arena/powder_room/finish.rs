//! The end of the minute: the store goes up. From the last tick every crate
//! and pier still standing in a seat's view catches in a ripple that runs
//! out from the knight looking, each one a small blast (the same pictures
//! as a keg's, `boom`), while the film's bars close and the call is made;
//! then the iris shuts, and the yard: the ruined store burning against the
//! night, fireworks over it, three steps of stone rising, the winner
//! laureled, ash and paper falling, the pot counting up. All of it from the
//! show's `ended` tick, so every seat sees it alike.

use super::super::super::ink::{Img, bayer, hash};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{NATIVE_H, NATIVE_W, enlarge, knights};
use super::words::Ledger;
use super::{boom, hud::called, walls};
use crate::motion::ease;
use super::super::super::ink::text_width;
use crate::rules::powder::{CRATES_PER_GOLD, Powder, pillar};
use crate::rules::powder_show::Blast;
use crate::rules::juice::Weight;
use crate::rules::Run;

/// Ticks the live finish plays before the scene cuts to the yard.
pub(super) const FINALE: u32 = 66;
/// Ticks the yard stands, and the ticks its iris takes to open.
pub(super) const HALL: u32 = 270;
const OPEN: u32 = 14;
/// Ticks the yard dissolves back into the room at the end.
const LEAVE: u32 = 12;
/// How much of the store the table must break for each title.
const TIERS: [(u32, &str); 3] = [(50, "FUSE CREW"), (80, "BLAST CREW"), (100, "STORE BREAKERS")];
/// Pixels a tick the ripple runs.
const SPEED: f32 = 12.0;

/// Ticks since the minute ended, while the finish is still the show.
pub(super) fn age(p: &Powder, now: u32) -> Option<u32> {
    p.show.since_end(now).filter(|&e| e < FINALE + HALL)
}

/// The knights by crates, most first, ties to the lower seat.
fn order(p: &Powder) -> Vec<u32> {
    let mut ids: Vec<u32> = p.broken.keys().copied().collect();
    ids.sort_by_key(|id| (std::cmp::Reverse(p.broken[id]), *id));
    ids
}

/// The share of the store the table broke, 0 to 100.
fn share(p: &Powder) -> u32 {
    let total = u32::from(p.show.team) + p.crates.len() as u32;
    if total == 0 { 0 } else { u32::from(p.show.team) * 100 / total }
}

/// What still stands when the minute ends in a view, and when each piece
/// catches: `(tile, is a crate, tick it goes)`.
fn standing(run: &Run, p: &Powder, view: (i32, i32), from: (i32, i32)) -> Vec<((i32, i32), bool, u32)> {
    let room = run.room();
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let (c0, c1) = ((view.0 / 16 - 2).max(1), ((view.0 + 384) / 16 + 2).min(cols - 1));
    let (r0, r1) = ((view.1 / 16 - 2).max(1), ((view.1 + 224) / 16 + 2).min(rows - 1));
    let mut out = Vec::new();
    for r in r0..r1 {
        for c in c0..c1 {
            let is_crate = p.crates.contains(&((r * cols + c) as u16));
            let is_pier = pillar(c, r) && !is_crate;
            if !is_crate && !is_pier {
                continue;
            }
            let (x, y) = boom::centre((c, r));
            let d = (((x - from.0) * (x - from.0) + (y - from.1) * (y - from.1)) as f32).sqrt();
            let jitter = hash(c, r, 21) % 4;
            let go = 4 + (d / SPEED) as u32 + jitter + if is_crate { 0 } else { 6 };
            out.push(((c, r), is_crate, go));
        }
    }
    out
}

/// Where the ripple starts: under the knight looking.
fn source(run: &Run, focus: Option<u32>, view: (i32, i32)) -> (i32, i32) {
    focus
        .and_then(|id| run.players.get(&id))
        .map_or((view.0 + 192, view.1 + 112), |h| super::super::at(h.x, h.y))
}

/// Under the knights, for the live finish: what has not yet caught.
pub(super) fn ground(cv: &mut Img, run: &Run, p: &Powder, focus: Option<u32>, view: (i32, i32)) {
    let now = run.tick as u32;
    let Some(e) = age(p, now).filter(|&e| e < FINALE) else { return };
    for ((c, r), is_crate, go) in standing(run, p, view, source(run, focus, view)) {
        if e >= go {
            continue;
        }
        // A crate or pier about to go shakes.
        let shake = i32::from(go - e < 5 && e % 2 == 0);
        let sprite = if is_crate { walls::crate_img(walls::crate_kind(c, r)) } else { walls::pier() };
        cv.stamp(sprite, c * 16 + shake, r * 16);
    }
}

/// Over the knights, for the live finish: each piece that has caught is a
/// small blast of its own.
pub(super) fn over(cv: &mut Img, run: &Run, p: &Powder, focus: Option<u32>, view: (i32, i32)) {
    let now = run.tick as u32;
    let Some(e) = age(p, now).filter(|&e| e < FINALE) else { return };
    let blasts: Vec<Blast> = standing(run, p, view, source(run, focus, view))
        .into_iter()
        .filter(|&(_, _, go)| e >= go)
        .map(|((c, r), is_crate, go)| Blast {
            at: (c, r),
            who: 0,
            arms: [0; 4],
            crates: if is_crate { vec![(c, r)] } else { Vec::new() },
            depth: 0,
            born: now.saturating_sub(e - go),
            weight: if is_crate { Weight::Crushing } else { Weight::Solid },
            sooted: 0,
            last: false,
        })
        .collect();
    boom::over(cv, &blasts, now, view, run.tick);
}

/// The live finish, `e` ticks after the last: a flash, the film's bars, the
/// call, and the iris shutting for the yard.
pub(super) fn finale(led: &mut Ledger, over: &mut Img, run: &Run, p: &Powder, e: u32, focus: Option<u32>) {
    let Some(end) = p.show.ended else { return };
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
    if (4..FINALE - 12).contains(&e) {
        let winner = u32::from(end.winner);
        let sub = if p.broken.len() < 2 || winner == 0 {
            String::new()
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 10))
        };
        let title = if end.cleared { "STORE CLEARED" } else { "TIME" };
        led.banner(over, "finale", 62, title, &sub, e - 4, FINALE - 16, Tone::Warm, 3);
    }
    let out = ((e as f32 - (FINALE - 14) as f32) / 14.0).clamp(0.0, 1.0);
    juice::iris(over, (NATIVE_W / 2, NATIVE_H / 2 - 10), out, 'k');
}

/// The night over the ruin: a sky going from deep to the fire's red at the
/// ground, stars, and the glow of the burning store.
fn sky(im: &mut Img, wall: i32, tick: u32) {
    for y in 0..wall {
        let k = y as f32 / wall as f32;
        for x in 0..NATIVE_W {
            let ink = if bayer(x, y) < k * k * 0.85 { 'p' } else if bayer(x, y) < k * 0.9 + 0.3 { 's' } else { 'k' };
            let ink = if k > 0.78 && bayer(x + 1, y) < (k - 0.78) * 3.2 { '7' } else { ink };
            im.put(x, y, ink);
        }
    }
    for k in 0..46 {
        let (x, y) = ((hash(k, 1, 9) % NATIVE_W as u32) as i32, (hash(k, 2, 9) % 60) as i32);
        let twinkle = (tick / 9 + hash(k, 3, 9) % 7) % 7 == 0;
        im.put(x, y, if twinkle { 'W' } else { 'U' });
    }
}

/// The ruined store along the horizon: broken brick walls with the sky in
/// their windows, fire in the gaps, smoke standing up from it.
fn ruin(im: &mut Img, wall: i32, age: u32, tick: u32) {
    let mut x = -6;
    while x < NATIVE_W {
        let w = 26 + (hash(x, 5, 3) % 30) as i32;
        let h = 22 + (hash(x, 6, 3) % 34) as i32;
        // A wall with a jagged top.
        for dx in 0..w {
            let top = wall - h + ((hash(x + dx / 5, 8, 3) % 9) as i32);
            for y in top..wall {
                let brick = (dx + (y / 6 % 2) * 6) % 12 == 0 || y % 6 == 0;
                im.put(x + dx, y, if brick { 'k' } else if bayer(x + dx, y) < 0.3 { 'K' } else { 'X' });
            }
            im.put(x + dx, top, 'g');
        }
        // A window: sky behind it, lit red from the fire below.
        if hash(x, 9, 3) % 3 != 0 {
            im.rect(x + w / 2 - 3, wall - h + 14, 6, 9, 'k');
            im.rect(x + w / 2 - 2, wall - h + 15, 4, 7, if tick / 6 % 2 == 0 { '7' } else { 'p' });
        }
        x += w + 4 + (hash(x, 7, 3) % 12) as i32;
    }
    // Fire in the gaps and smoke standing off it.
    for (k, fx) in [34, 120, 205, 290, 352].into_iter().enumerate() {
        let t = (age as f32 * 0.4 + k as f32 * 3.0) % 12.0;
        let a = 3.0 + (t - 6.0).abs();
        boom::flame(im, (fx, wall - 6), a, 1.3, 41 + k as u32);
    }
    for (k, sx) in [26, 88, 150, 214, 270, 330].into_iter().enumerate() {
        for j in 0..4u32 {
            let t = ((age + k as u32 * 9 + j * 14) % 72) as f32;
            let p = t / 72.0;
            boom::puff(im, (sx as f32 + (p * 18.0), wall as f32 - 20.0 - t * 0.9), 4.0 + 7.0 * p, 0.85 * (1.0 - p), 0.5 + p * 0.4, 17 + k as u32 * 5 + j, false);
        }
    }
}

/// The yard under it: a cobbled dark with a rim of firelight along its top.
fn yard(im: &mut Img, wall: i32) {
    for y in wall..NATIVE_H {
        for x in 0..NATIVE_W {
            let row = (y - wall) / 7;
            let seam = (y - wall) % 7 == 6;
            let joint = (x + (row % 2) * 9) % 18 == 0;
            let fire = if (y - wall) < 6 { 0.5 - (y - wall) as f32 * 0.08 } else { 0.0 };
            let ink = if seam || joint {
                'k'
            } else if bayer(x, y) < fire {
                '7'
            } else if bayer(x, y) < 0.2 {
                'n'
            } else {
                'K'
            };
            im.put(x, y, ink);
        }
    }
    im.rect(0, wall, NATIVE_W, 1, 'R');
}

/// Light from above onto the winner, a dither behind the steps.
fn spot(im: &mut Img, cx: i32, wall: i32, floor: i32, k: f32) {
    for y in wall..floor {
        let half = 22.0 + (y - wall) as f32 * 0.6;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if bayer(x, y) < edge * edge * 0.3 * k {
                im.put(x, y, 'J');
            }
        }
    }
}

/// The yard for a seat, `e` ticks after the minute ended.
pub(super) fn scene(led: &mut Ledger, run: &Run, p: &Powder, e: u32, focus: Option<u32>) -> Img {
    let Some(end) = p.show.ended else { return Img::black(NATIVE_W, NATIVE_H) };
    let age = e.saturating_sub(FINALE);
    let now = run.tick as u32;
    const WALL: i32 = 112;
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    sky(&mut im, WALL, now);
    // Fireworks over the ruin, three launchers in turn.
    if age >= 6 {
        for (j, (x, top, ramp)) in [(66.0, 26.0, ramps::GOLD), (192.0, 18.0, ramps::EMBER), (318.0, 30.0, ramps::HOLY)].into_iter().enumerate() {
            let t = age as i32 - 6 - j as i32 * 15;
            if t >= 0 {
                let round = (t / 50) as u32;
                juice::firework(&mut im, x + (round as f32 * 17.0) % 30.0 - 15.0, 100.0, top, (t % 50) as u32, 29 * j as u32 + round * 5, ramp, age as u64);
            }
        }
    }
    ruin(&mut im, WALL, age, now);
    yard(&mut im, WALL);
    let ranks = order(p);
    let winner = ranks.first().copied().unwrap_or(0);
    let rivals = ranks.len() > 1;
    let (cx, floor) = (NATIVE_W / 2, 182);
    spot(&mut im, cx, WALL, floor, ((age as f32 - 6.0) / 12.0).clamp(0.0, 1.0));
    let places: Vec<Place> = ranks
        .iter()
        .take(3)
        .filter_map(|&id| {
            let hero = run.players.get(&id)?;
            Some(Place { sprite: knights::figure(hero, id), name: called(run, id, focus, 9), line: format!("{} CRATES", p.broken[&id]) })
        })
        .collect();
    // The kit draws each name and score under its step: claim their words.
    for (i, place) in places.iter().enumerate() {
        let x = [cx, cx - 88, cx + 88][i];
        if age >= [16, 8, 0][i] as u32 + 18 {
            let (nw, lw) = (text_width(&place.name), text_width(&place.line));
            led.claim("podium name", (x - nw / 2, floor + 4, nw, 7));
            led.claim("podium line", (x - lw / 2, floor + 13, lw, 7));
        }
    }
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
    // Rivals below the podium, small and plain: seat colour and count.
    for (k, &id) in ranks.iter().enumerate().skip(3) {
        if age < 24 + k as u32 * 3 {
            continue;
        }
        let y = 44 + (k as i32 - 3) * 11;
        im.rect(8, y + 1, 5, 5, knights::seat_mark(id));
        led.small(&mut im, "rival", 16, y, &format!("{} {} {}", k + 1, called(run, id, focus, 8), p.broken[&id]), if focus == Some(id) { 'T' } else { 'h' });
    }
    if age >= 14 {
        juice::confetti(&mut im, (0, 0, NATIVE_W, NATIVE_H), age, 9, 110);
        // Ash and embers drifting up off the ruin.
        let mut pen = Pen::new(&mut im, 260);
        let embers = Spray { gravity: -0.012, drag: 0.02, stagger: 3.0, ..Spray::round(77, 30, (192.0, 118.0), (0.6, 2.4), (50.0, 110.0)) };
        embers.each(age as f32 - 14.0, |m| pfx::mote(&mut pen, &m, Shape::Ember, ramps::EMBER, 1, now as u64));
    }
    // The winner, big, off the top.
    if age >= 6 {
        let title = if !rivals {
            if end.cleared { "STORE CLEARED".to_string() } else { "TIME UP".to_string() }
        } else if focus == Some(winner) {
            "YOU WIN".to_string()
        } else {
            format!("{} WINS", called(run, winner, focus, 12))
        };
        led.banner(&mut im, "winner", 12, &title, "", age - 6, 64, Tone::Warm, 3);
    }
    // What the seat looking takes home, counting up; and the table's tally.
    if age >= 24 {
        let mine = focus.and_then(|id| p.broken.get(&id).map(|&n| (id, n)));
        let gold = mine.map_or(0, |(id, n)| u32::from(n) / CRATES_PER_GOLD as u32 + if id == winner && rivals { end.pot } else { 0 });
        let shown = (gold as f32 * ease::out(((age - 24) as f32 / 40.0).min(1.0))) as u32;
        led.big(&mut im, "gold", NATIVE_W / 2, 205, &format!("+{shown} GOLD"), 2, 'T', 'B');
    }
    if age >= 30 {
        let (best, who) = p.show.best.iter().map(|(&id, &n)| (n, id)).max().unwrap_or((0, 0));
        led.small(&mut im, "table", 6, 207, &format!("TABLE {} CRATES", p.show.team), 'o');
        // A reason to go again: the next rung of the table's ladder.
        let tier = TIERS.iter().rev().find(|&&(need, _)| share(p) >= need);
        let hook = match TIERS.iter().find(|&&(need, _)| share(p) < need) {
            Some(&(_, name)) => format!("NEXT {name}"),
            None => tier.map_or_else(String::new, |&(_, name)| name.to_string()),
        };
        led.small(&mut im, "next", 6, 215, &hook, 'h');
        if best >= 3 {
            let line = format!("BEST RUN {} {}", best, called(run, who, focus, 8));
            led.small(&mut im, "best", NATIVE_W - 6 - text_width(&line), 207, &line, 'o');
        }
        if rivals {
            let line = if focus == Some(winner) { "YOU TAKE THE POT" } else { "THE POT IS THEIRS" };
            led.small(&mut im, "pot line", NATIVE_W - 6 - text_width(line), 215, line, 'h');
        }
    }
    // The way home: a thin bar filling along the bottom edge.
    let filled = (NATIVE_W as u32 * age.min(HALL) / HALL) as i32;
    im.rect(0, NATIVE_H - 1, filled, 1, 'o');
    // The iris opening from the finale's black, and the yard dissolving at the end.
    juice::iris(&mut im, (NATIVE_W / 2, 100), 1.0 - ((e as f32 - FINALE as f32) / OPEN as f32).clamp(0.0, 1.0), 'k');
    let left = FINALE + HALL - e;
    if left < LEAVE {
        juice::dissolve(&mut im, 1.0 - left as f32 / LEAVE as f32);
    }
    let _ = enlarge;
    im
}
