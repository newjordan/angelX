//! The maze's moments over a seat's view, each a pure function of the
//! maze's own ticks: the title slam and the 3-2-1-GO, the callout banners,
//! scores flying to the strip, the danger rim when a wraith closes in, the
//! cold rim of the holy water, and the candle's last seconds.

use super::super::super::ink::{Img, bayer};
use super::super::juice::{self, Tone};
use super::super::{NATIVE_H, NATIVE_W, TILE};
use super::{bits, hud, wax_art};
use crate::motion::ease;
use crate::rules::folly::{MAZE_TIME, TITLE, Temper, WaxMaze};
use crate::rules::wax_show::{Cry, SHOUT_LIFE, Shout, TOUCH_LIFE};
use crate::rules::{HZ, Run, TILE_UNITS};

/// Ticks a flying number takes to reach the strip, from the catch.
pub(super) const FLY: u32 = 27;
/// Ticks each of 3, 2 and 1 shows, and the first one's start.
const BEAT: u32 = 12;
const FIRST: u32 = TITLE - 3 * BEAT + 1;
/// Ticks GO shows.
pub(super) const GO: u32 = 28;

/// The title, its two plain words and the countdown, over the view.
pub(super) fn opening(over: &mut Img, maze: &WaxMaze, tick: u32) {
    let t = maze.t;
    if t >= TITLE + GO {
        return;
    }
    if t < TITLE {
        // The view dims behind the title, and clears as the numbers come.
        let k = if t < FIRST { 0.55 } else { 0.55 * (1.0 - (t - FIRST) as f32 / 12.0).max(0.0) };
        for y in 0..NATIVE_H {
            for x in 0..NATIVE_W {
                if bayer(x, y) < k {
                    over.put(x, y, 'k');
                }
            }
        }
        if t < FIRST + 6 {
            juice::banner(over, 36, "THE WAX MAZE", "", t, FIRST + 6, Tone::Warm, 3);
        }
        // Two plain words, each with its picture.
        if (6..FIRST).contains(&t) {
            let fade = 1.0 - ((t as f32 - 6.0) / 6.0).clamp(0.0, 1.0);
            let mut eat = juice::big_text_img("EAT", 2, 'T', 'B');
            juice::dissolve(&mut eat, fade);
            let mut flee = juice::big_text_img("FLEE", 2, 'T', 'B');
            juice::dissolve(&mut flee, fade);
            let drop = super::super::enlarge(bits::drop(tick / 6 % 2 == 0), 3.0);
            over.stamp(&drop, 112 - drop.w / 2, 82);
            over.stamp(&eat, 112 - eat.w / 2, 104);
            let (veil, solid) = wax_art::king(Temper::Hound, wax_art::KingMood::Hunt, tick);
            let mut king = veil;
            king.stamp(&solid, 0, 0);
            let king = super::super::enlarge(&king, 2.0);
            over.stamp(&king, 272 - king.w / 2, 82);
            over.stamp(&flee, 272 - flee.w / 2, 104);
        }
        if t >= FIRST {
            let n = ((t - FIRST) / BEAT).min(2);
            let label = ["3", "2", "1"][n as usize];
            juice::countdown(over, (NATIVE_W / 2, 118), label, (t - FIRST) % BEAT, BEAT, false);
        }
    } else {
        juice::countdown(over, (NATIVE_W / 2, 118), "GO", t - TITLE, GO, true);
    }
}

fn words(cry: Cry) -> &'static str {
    match cry {
        Cry::Feast(0) => "FEAST",
        Cry::Feast(1) => "BANQUET",
        Cry::Feast(_) => "GLUTTON",
        Cry::Holy => "HOLY WATER",
        Cry::Catch(2) => "DOUBLE",
        Cry::Catch(3) => "TRIPLE",
        Cry::Catch(_) => "ALL FOUR",
        Cry::Lead => "TAKES THE LEAD",
        Cry::Half => "HALF GONE",
        Cry::Last => "LAST BITES",
        Cry::Quicken => "THEY QUICKEN",
    }
}

/// The banner: the loudest call of the newest moment, big, under the strip;
/// under it a line of small words: whose it is (not yours) and the other
/// calls the same moment made.
pub(super) fn callouts(over: &mut Img, run: &Run, maze: &WaxMaze, focus: Option<u32>, now: u32) {
    let mut calls: Vec<Shout> = maze.show.shouts.iter().copied().filter(|c| now.saturating_sub(c.born) < SHOUT_LIFE).collect();
    calls.sort_by_key(|c| (std::cmp::Reverse(c.born), std::cmp::Reverse(c.cry.rank())));
    let Some(top) = calls.first().copied() else {
        return;
    };
    let same: Vec<Shout> = calls.iter().copied().filter(|c| c.born == top.born && c.who == top.who).collect();
    let mut sub = Vec::new();
    let who = u32::from(top.who);
    if who != 0 && focus != Some(who) {
        sub.push(hud::called(run, who, focus, 9));
    }
    sub.extend(same.iter().skip(1).map(|c| words(c.cry).to_string()));
    let mine = who != 0 && focus == Some(who);
    let tone = match top.cry {
        Cry::Quicken | Cry::Last => Tone::Hot,
        Cry::Half => Tone::Warm,
        _ if mine => Tone::Warm,
        _ => Tone::Steel,
    };
    let title = words(top.cry);
    let cap = if crate::render::overworld::ink::text_width(title) * 3 <= 164 { 3 } else { 2 };
    juice::banner(over, 46, title, &sub.join("  "), now.saturating_sub(top.born), SHOUT_LIFE, tone, cap);
}

/// What flies to the strip: a wraith caught's worth, and, for the seat
/// looking, a crumb off each drop they eat.
pub(super) fn fly_ups(over: &mut Img, maze: &WaxMaze, focus: Option<u32>, (ox, oy): (i32, i32), cols: i32, now: u32) {
    for c in &maze.show.catches {
        let age = now.saturating_sub(c.born);
        if age >= FLY {
            continue;
        }
        let from = (
            (c.at.0 * TILE / 16 - ox).clamp(8, NATIVE_W - 8),
            (c.at.1 * TILE / 16 - oy).clamp(30, NATIVE_H - 8),
        );
        let ink = if focus == Some(u32::from(c.who)) { '@' } else { 'T' };
        juice::fly_up(over, &format!("+{}", c.worth), from, hud::anchor(maze, u32::from(c.who)), age.saturating_sub(5), FLY - 5, ink);
    }
    let Some(me) = focus else { return };
    let to = hud::anchor(maze, me);
    for p in maze.show.pops.iter().filter(|p| u32::from(p.who) == me) {
        let age = now.saturating_sub(p.born);
        if age >= 9 {
            continue;
        }
        let (tx, ty) = (i32::from(p.tile) % cols, i32::from(p.tile) / cols);
        let from = ((tx * TILE + TILE / 2 - ox) as f32, (ty * TILE + TILE / 2 - oy) as f32);
        let (x, y) = crate::motion::arc(from, (to.0 as f32, to.1 as f32), 8.0, ease::both(age as f32 / 9.0));
        over.rect(x as i32, y as i32, 2, 2, 'T');
        over.put(x as i32, y as i32 + 1, 'o');
    }
}

/// The danger of a wraith closing on the seat looking, as a dithered red
/// rim that pulses faster the closer it comes; and the sting of a touch.
pub(super) fn danger(over: &mut Img, run: &Run, maze: &WaxMaze, focus: Option<u32>, now: u32) {
    let Some(me) = focus else { return };
    for t in maze.show.touches.iter().filter(|t| u32::from(t.who) == me) {
        let age = now.saturating_sub(t.born);
        if age < TOUCH_LIFE.min(26) {
            let k = 1.0 - age as f32 / 26.0;
            juice::vignette(over, (0, 0, NATIVE_W, NATIVE_H), k * 0.9, if age < 3 { 20 } else { 16 }, if age < 3 { 'R' } else { '7' });
        }
    }
    let Some(hero) = run.players.get(&me).filter(|h| h.hp > 0) else { return };
    if maze.spooked.get(&me).is_some_and(|&until| maze.t < until) || maze.done || maze.titled() {
        return;
    }
    let (hx, hy) = (hero.x / TILE_UNITS, hero.y / TILE_UNITS);
    let near = maze
        .wraiths
        .iter()
        .filter(|w| w.roaming() && !w.eyes && !maze.pale_now(w))
        .map(|w| {
            let (x, y) = w.pos();
            (x - hx).hypot(y - hy)
        })
        .fold(f32::MAX, f32::min);
    if near < 4.5 {
        let k = 1.0 - near / 4.5;
        let pulse = 0.55 + 0.45 * crate::motion::math::sin(now as f32 * (0.25 + k * 0.5));
        juice::vignette(over, (0, 0, NATIVE_W, NATIVE_H), k * pulse * 0.85, 10 + (k * 12.0) as i32, '7');
    }
}

/// The holy water's cold rim while the wraiths are pale, flashing as it
/// wears off; and the flash of the flask being drunk.
pub(super) fn pale_rim(over: &mut Img, maze: &WaxMaze, now: u32) {
    if maze.pale > 0 && !maze.done {
        let flash = maze.warning() && now / 4 % 2 == 0;
        juice::vignette(over, (0, 0, NATIVE_W, NATIVE_H), if flash { 0.55 } else { 0.32 }, 12, if flash { 'V' } else { 'Q' });
    }
    for s in &maze.show.splashes {
        let age = now.saturating_sub(s.born);
        if age < 5 {
            let k = (1.0 - age as f32 / 5.0) * 0.09;
            for y in 0..NATIVE_H {
                for x in 0..NATIVE_W {
                    if bayer(x, y) < k {
                        over.put(x, y, 'V');
                    }
                }
            }
        }
    }
}

/// The candle's last five seconds, one big number a second, slammed in.
pub(super) fn last_seconds(over: &mut Img, maze: &WaxMaze) {
    if maze.done || maze.titled() || maze.left() > 5 {
        return;
    }
    let rem = TITLE + MAZE_TIME - maze.t;
    let (sec, age) = (rem.div_ceil(HZ), HZ - 1 - (rem - 1) % HZ);
    let scale = 4 + (3 - age as i32).max(0);
    let mut digit = juice::big_text_img(&sec.to_string(), scale, if age < 3 { 'c' } else { 'R' }, 'B');
    juice::dissolve(&mut digit, ((age as f32 - 20.0) / 10.0).clamp(0.0, 1.0));
    over.stamp(&digit, NATIVE_W / 2 - digit.w / 2, 64 - (scale - 4) * 3);
}
