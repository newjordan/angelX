//! The shoot's moments over a seat's view, each a pure function of the
//! shoot's own ticks: the title slam and the 3-2-1-GO, the callout banners,
//! scores flying to the strip, the sting of a bite, the bell's flash, and
//! the minute's last seconds.

use super::super::super::ink::{Img, bayer};
use super::super::juice::{self, Tone};
use super::super::{NATIVE_H, NATIVE_W, demonic};
use super::{bits, hud};
use crate::motion::ease;
use crate::rules::belfry_show::{Cry, SHOUT_LIFE, Shout};
use crate::rules::folly::{BAT_TIME, BatShoot, TITLE};
use crate::rules::{EnemyKind, HZ, Run};

/// Ticks a flying number takes to reach the strip, from the kill.
pub(super) const FLY: u32 = 27;
/// Ticks each of 3, 2 and 1 shows, and the first one's start.
const BEAT: u32 = 12;
const FIRST: u32 = TITLE - 3 * BEAT + 1;
/// Ticks GO shows.
pub(super) const GO: u32 = 28;

/// The title, its two plain words and the countdown, over the view.
pub(super) fn opening(over: &mut Img, run: &Run, shoot: &BatShoot) {
    let t = shoot.t;
    let tick = run.tick as u32;
    if t >= TITLE + GO {
        return;
    }
    if t < TITLE {
        // The view dims behind the title, and clears as the numbers come.
        let k = if t < FIRST { 0.8 } else { 0.8 * (1.0 - (t - FIRST) as f32 / 12.0).max(0.0) };
        for y in 0..NATIVE_H {
            for x in 0..NATIVE_W {
                if bayer(x, y) < k {
                    over.put(x, y, 'k');
                }
            }
        }
        if t < FIRST + 6 {
            juice::banner(over, 36, "THE BELFRY SHOOT", "", t, FIRST + 6, Tone::Warm, 3);
        }
        // Two plain words, each with its picture: the bat, the bell.
        if (6..FIRST).contains(&t) {
            let fade = 1.0 - ((t as f32 - 6.0) / 6.0).clamp(0.0, 1.0);
            let mut shoot_w = juice::big_text_img("SHOOT", 2, 'T', 'B');
            juice::dissolve(&mut shoot_w, fade);
            let mut ring_w = juice::big_text_img("RING", 2, 'T', 'B');
            juice::dissolve(&mut ring_w, fade);
            let bat = super::super::enlarge(&demonic::enemy(run, EnemyKind::Bat, tick / 2), 3.0);
            let bob = (crate::motion::math::sin(tick as f32 * 0.25) * 2.0) as i32;
            over.stamp(&bat, 112 - bat.w / 2, 80 + bob);
            over.stamp(&shoot_w, 112 - shoot_w.w / 2, 112);
            let bell = super::super::enlarge(bits::strip(tick / 5 % 2 == 0), 3.0);
            over.stamp(&bell, 272 - bell.w / 2, 80);
            over.stamp(&ring_w, 272 - ring_w.w / 2, 112);
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

fn words(cry: Cry) -> String {
    match cry {
        Cry::First => "FIRST BAT".into(),
        Cry::Chain(2) => "DOUBLE".into(),
        Cry::Chain(3) => "TRIPLE".into(),
        Cry::Chain(4) => "QUAD".into(),
        Cry::Chain(n) if n < 8 => "RAMPAGE".into(),
        Cry::Chain(_) => "BAT STORM".into(),
        Cry::Flock => "FLOCK DOWN".into(),
        Cry::Lead => "TAKES THE LEAD".into(),
        Cry::Toll(_) => "THE BELL TOLLS".into(),
        Cry::Half => "HALF TIME".into(),
        Cry::Last => "LAST TEN".into(),
    }
}

/// The banner: the loudest call of the newest moment, big, under the strip;
/// under it a line of small words: whose it is (not yours) and the other
/// calls the same moment made.
pub(super) fn callouts(over: &mut Img, run: &Run, shoot: &BatShoot, focus: Option<u32>, now: u32) {
    let mut calls: Vec<Shout> = shoot.show.shouts.iter().copied().filter(|c| now.saturating_sub(c.born) < SHOUT_LIFE).collect();
    calls.sort_by_key(|c| (std::cmp::Reverse(c.born), std::cmp::Reverse(c.cry.rank())));
    let Some(top) = calls.first().copied() else {
        return;
    };
    let same: Vec<Shout> = calls.iter().copied().filter(|c| c.born == top.born).collect();
    let mut sub = Vec::new();
    let who = u32::from(top.who);
    if let Cry::Toll(n) = top.cry {
        sub.push(format!("{} BATS", u32::from(n) * u32::from(crate::rules::belfry_show::TOLL_EVERY)));
    } else if who != 0 && focus != Some(who) {
        sub.push(hud::called(run, who, focus, 9));
    }
    sub.extend(same.iter().filter(|c| c.cry != top.cry).map(|c| words(c.cry)));
    let mine = who != 0 && focus == Some(who);
    let tone = match top.cry {
        Cry::Last => Tone::Hot,
        Cry::Toll(_) | Cry::Half | Cry::Flock | Cry::First => Tone::Warm,
        _ if mine => Tone::Warm,
        _ => Tone::Steel,
    };
    let title = words(top.cry);
    let cap = if crate::render::overworld::ink::text_width(&title) * 3 <= 164 { 3 } else { 2 };
    juice::banner(over, hud::BOTTOM + 28, &title, &sub.join("  "), now.saturating_sub(top.born), SHOUT_LIFE, tone, cap);
}

/// What flies to the strip: each bat felled, its harm as a number for the
/// seat looking (a crumb of the knight's ink for the others).
pub(super) fn fly_ups(over: &mut Img, shoot: &BatShoot, focus: Option<u32>, (ox, oy): (i32, i32), now: u32) {
    for h in shoot.show.hits.iter().filter(|h| h.kill && h.who != 0) {
        let age = h.age(now);
        if age >= FLY {
            continue;
        }
        let from = ((h.at.0 - ox).clamp(8, NATIVE_W - 8), (h.at.1 - 5 - oy).clamp(hud::BOTTOM + 8, NATIVE_H - 8));
        let who = u32::from(h.who);
        let to = hud::anchor(shoot, who);
        if focus == Some(who) {
            juice::fly_up(over, &format!("+{}", h.lost), from, to, age.saturating_sub(5), FLY - 5, if h.chain >= 3 { '@' } else { 'T' });
        } else if age >= 3 {
            let t = ease::both((age - 3) as f32 / (FLY - 3) as f32);
            let (x, y) = crate::motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 14.0, t);
            let ink = super::super::knights::seat_mark(who);
            over.rect(x as i32, y as i32, 2, 2, ink);
        }
    }
}

/// A dithered rim of `fill` along the view's edges, `depth` pixels deep, `k`
/// (0 to 1) how strong: `juice::vignette` over the same box, but touching only
/// the rim's own pixels, since it is drawn most frames.
fn rim(over: &mut Img, k: f32, depth: i32, fill: char) {
    if k <= 0.0 {
        return;
    }
    let (w, h) = (NATIVE_W, NATIVE_H);
    for y in 0..h {
        let across = y < depth || y >= h - depth;
        let (mut x, end) = (0, w);
        while x < end {
            if !across && x == depth {
                x = w - depth;
                continue;
            }
            let edge = x.min(w - 1 - x).min(y).min(h - 1 - y);
            if edge < depth {
                let near = 1.0 - edge as f32 / depth as f32;
                if bayer(x, y) < near * near * k {
                    over.put(x, y, fill);
                }
            }
            x += 1;
        }
    }
}

/// The sting of a bite on the seat looking: a red rim that fades, and the
/// bell's warm flash round the edge when it tolls.
pub(super) fn danger(over: &mut Img, run: &Run, shoot: &BatShoot, focus: Option<u32>, now: u32) {
    // A crowd of bats closing on the seat looking: a rim that pulses.
    if let Some(hero) = focus.and_then(|me| run.players.get(&me)).filter(|h| h.hp > 0) {
        let near = run
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Bat && (e.x - hero.x).hypot(e.y - hero.y) < 8.0)
            .count();
        if near >= 3 && !shoot.done && !shoot.titled() {
            let k = ((near - 2) as f32 / 4.0).min(1.0);
            let pulse = 0.5 + 0.5 * crate::motion::math::sin(now as f32 * 0.3);
            rim(over, k * (0.25 + 0.2 * pulse), 9, '7');
        }
    }
    if let Some(me) = focus {
        for b in shoot.show.bites.iter().filter(|b| u32::from(b.who) == me) {
            let age = now.saturating_sub(b.born);
            if age < 22 {
                let k = 1.0 - age as f32 / 22.0;
                rim(over, k * 0.9, if age < 3 { 20 } else { 16 }, if age < 3 { 'R' } else { '7' });
            }
        }
    }
    if let Some(age) = hud::tolled(shoot, now).filter(|&a| a < 14) {
        rim(over, (1.0 - age as f32 / 14.0) * 0.6, 12, 'o');
    }
}

/// The last five seconds, one big number a second, slammed in.
pub(super) fn last_seconds(over: &mut Img, shoot: &BatShoot) {
    if shoot.done || shoot.titled() || shoot.left() > 5 {
        return;
    }
    let rem = (TITLE + BAT_TIME).saturating_sub(shoot.t);
    if rem == 0 {
        return;
    }
    let (sec, age) = (rem.div_ceil(HZ), HZ - 1 - (rem - 1) % HZ);
    let scale = 4 + (3 - age as i32).max(0);
    let mut digit = juice::big_text_img(&sec.to_string(), scale, if age < 3 { 'c' } else { 'R' }, 'B');
    juice::dissolve(&mut digit, ((age as f32 - 20.0) / 10.0).clamp(0.0, 1.0));
    over.stamp(&digit, NATIVE_W / 2 - digit.w / 2, 72 - (scale - 4) * 3);
}
