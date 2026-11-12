//! The store's moments over a seat's view, each a pure function of the
//! show's own ticks: the title slam with its two plain words and the
//! 3-2-1-GO, the callout banners, crates flying to the strip, the sting of
//! a sooting and the red rim of a lane about to burn, the last five
//! seconds. All the words go through the ledger (`words`), so none meets
//! another.

use super::super::super::ink::{Img, bayer};
use super::super::juice::{self, Tone};
use super::super::knights;
use super::super::{NATIVE_H, NATIVE_W};
use super::words::Ledger;
use super::{bits, hud, set, walls};
use crate::rules::folly::TITLE;
use crate::rules::juice::Weight;
use crate::rules::powder::Powder;
use crate::rules::powder_show::{Cry, LINK, SHOUT_LIFE, Shout};
use crate::rules::{HZ, Run};

/// Ticks a flying number takes to reach the strip, from the blast.
pub(super) const FLY: u32 = 27;
/// Ticks each of 3, 2 and 1 shows, and the first one's start.
const BEAT: u32 = 12;
const FIRST: u32 = TITLE - 3 * BEAT + 1;
/// Ticks GO shows.
pub(super) const GO: u32 = 28;

/// The title, its two plain words and the countdown, over the view.
pub(super) fn opening(led: &mut Ledger, over: &mut Img, run: &Run, p: &Powder) {
    let t = p.t;
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
            led.banner(over, "title", 36, "THE POWDER ROOM", "", t, FIRST + 6, Tone::Warm, 3);
        }
        // Two plain words, each with its picture: the keg and the crate.
        if (6..FIRST).contains(&t) {
            let fade = 1.0 - ((t as f32 - 6.0) / 6.0).clamp(0.0, 1.0);
            let bob = (crate::motion::math::sin(tick as f32 * 0.25) * 2.0) as i32;
            let keg = super::super::enlarge(bits::keg(if tick / 4 % 2 == 0 { 0 } else { 1 }), 3.0);
            over.stamp(&keg, 96 - keg.w / 2, 80 + bob);
            let mut drop_w = juice::big_text_img("DROP KEG", 2, 'T', 'B');
            juice::dissolve(&mut drop_w, fade);
            led.claim("drop", (96 - drop_w.w / 2, 112, drop_w.w, drop_w.h));
            over.stamp(&drop_w, 96 - drop_w.w / 2, 112);
            // The key to press, on a cap.
            over.rect(88, 134, 15, 12, 'k');
            over.frame(88, 134, 15, 12, 'h');
            led.small(over, "key", 93, 137, "X", 'T');
            let crate_img = super::super::enlarge(walls::crate_img(2), 3.0);
            over.stamp(&crate_img, 280 - crate_img.w / 2, 78 + bob);
            let mut break_w = juice::big_text_img("BREAK CRATES", 2, 'T', 'B');
            juice::dissolve(&mut break_w, fade);
            led.claim("break", (280 - break_w.w / 2, 112, break_w.w, break_w.h));
            over.stamp(&break_w, 280 - break_w.w / 2, 112);
        }
        if t >= FIRST {
            let n = ((t - FIRST) / BEAT).min(2);
            let label = ["3", "2", "1"][n as usize];
            led.count(over, "beat", (NATIVE_W / 2, 118), label, (t - FIRST) % BEAT, BEAT, false);
        }
    } else {
        led.count(over, "go", (NATIVE_W / 2, 118), "GO", t - TITLE, GO, true);
    }
}

/// The words of a call.
pub(super) fn words(cry: Cry) -> String {
    match cry {
        Cry::First => "FIRST BLAST".into(),
        Cry::Haul(3) => "TRIPLE BREAK".into(),
        Cry::Haul(_) => "QUAD BREAK".into(),
        Cry::Chain(2) => "DOUBLE BLAST".into(),
        Cry::Chain(3) => "TRIPLE BLAST".into(),
        Cry::Chain(4) => "QUAD BLAST".into(),
        Cry::Chain(n) if n < 7 => "BIG BANG".into(),
        Cry::Chain(_) => "STORE QUAKE".into(),
        Cry::Soot => "SOOTED".into(),
        Cry::Payback => "PAYBACK".into(),
        Cry::OwnKeg => "OWN KEG".into(),
        Cry::Lead => "TAKES THE LEAD".into(),
        Cry::Store(25) => "QUARTER GONE".into(),
        Cry::Store(50) => "HALF THE STORE".into(),
        Cry::Store(_) => "NEARLY CLEAR".into(),
        Cry::Roll(5) => "ON A ROLL".into(),
        Cry::Roll(10) => "DEMOLITION".into(),
        Cry::Roll(_) => "STORE BREAKER".into(),
        Cry::Half => "HALF TIME".into(),
        Cry::Last => "LAST TEN".into(),
    }
}

/// The banner: the loudest call of the newest moment, big, under the strip;
/// under it a line of small words: whose it is (not yours) and the other
/// calls the same moment made. Where the last seconds' numbers hold the
/// middle it stands lower; where neither fits, it is not shown.
pub(super) fn callouts(led: &mut Ledger, over: &mut Img, run: &Run, p: &Powder, focus: Option<u32>, now: u32) {
    let mut calls: Vec<Shout> = p.show.shouts.iter().copied().filter(|c| now.saturating_sub(c.born) < SHOUT_LIFE).collect();
    calls.sort_by_key(|c| (std::cmp::Reverse(c.born), std::cmp::Reverse(c.cry.rank())));
    let Some(top) = calls.first().copied() else {
        return;
    };
    let same: Vec<Shout> = calls.iter().copied().filter(|c| c.born == top.born).collect();
    let mut sub = Vec::new();
    let who = u32::from(top.who);
    if who != 0 && focus != Some(who) {
        sub.push(hud::called(run, who, focus, 9));
    }
    let mut rest: Vec<&Shout> = same.iter().filter(|c| c.cry != top.cry).collect();
    rest.sort_by_key(|c| std::cmp::Reverse(c.cry.rank()));
    sub.extend(rest.into_iter().take(1).map(|c| words(c.cry)));
    let mine = who != 0 && focus == Some(who);
    let tone = match top.cry {
        Cry::Last | Cry::OwnKeg => Tone::Hot,
        Cry::Soot | Cry::Payback if mine => Tone::Hot,
        Cry::Chain(_) | Cry::Store(_) | Cry::Half | Cry::First | Cry::Haul(_) | Cry::Payback => Tone::Warm,
        _ if mine => Tone::Warm,
        _ => Tone::Steel,
    };
    let title = words(top.cry);
    let cap = if crate::render::overworld::ink::text_width(&title) * 3 <= 164 { 3 } else { 2 };
    let age = now.saturating_sub(top.born);
    let sub = sub.join("  ");
    for y in [hud::BOTTOM + 24, 142] {
        let r = super::words::banner_rect(y, &title, &sub, age, SHOUT_LIFE, cap);
        if led.free(r) {
            led.banner(over, "call", y, &title, &sub, age, SHOUT_LIFE, tone, cap);
            return;
        }
    }
}

/// What flies to the strip: each tick's crates, summed for the knight whose
/// kegs they were, as a number for the seat looking (a crumb of the
/// knight's ink for the others).
pub(super) fn fly_ups(led: &mut Ledger, over: &mut Img, p: &Powder, focus: Option<u32>, (ox, oy): (i32, i32), now: u32) {
    let mut seen: Vec<(u8, u32)> = Vec::new();
    for b in p.show.blasts.iter().filter(|b| !b.crates.is_empty() && b.who != 0) {
        if seen.contains(&(b.who, b.born)) {
            continue;
        }
        seen.push((b.who, b.born));
        let n: usize = p.show.blasts.iter().filter(|c| c.who == b.who && c.born == b.born).map(|c| c.crates.len()).sum();
        let age = b.age(now);
        if age >= FLY {
            continue;
        }
        let at = super::boom::centre(b.at);
        let from = ((at.0 - ox).clamp(8, NATIVE_W - 8), (at.1 - 6 - oy).clamp(hud::BOTTOM + 10, NATIVE_H - 8));
        let who = u32::from(b.who);
        let to = hud::anchor(p, who);
        let ink = if n >= 3 { '@' } else { 'T' };
        if focus == Some(who) {
            led.fly(over, "flying", &format!("+{n}"), from, to, age.saturating_sub(5), FLY - 5, ink);
        } else if age >= 3 {
            let t = crate::motion::ease::both((age - 3) as f32 / (FLY - 3) as f32);
            let (x, y) = crate::motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 14.0, t);
            over.rect(x as i32, y as i32, 2, 2, knights::seat_mark(who));
        }
    }
}

/// What a sooting says, over the knight: what it cost them, as a number
/// popping off the place they stood.
pub(super) fn sootings(led: &mut Ledger, over: &mut Img, p: &Powder, (ox, oy): (i32, i32), now: u32) {
    for s in &p.show.soots {
        let age = now.saturating_sub(s.born);
        if s.lost == 0 || age >= 34 {
            continue;
        }
        let at = super::boom::centre(s.from);
        let (x, y) = ((at.0 - ox).clamp(14, NATIVE_W - 14), (at.1 - 4 - oy).clamp(hud::BOTTOM + 34, NATIVE_H - 10));
        led.pop(over, "sooted", &format!("-{}", s.lost), (x, y), age, 34, 2, '7');
    }
}

/// A dithered rim of `fill` along the view's edges, `depth` pixels deep, `k`
/// (0 to 1) how strong.
fn rim(over: &mut Img, k: f32, depth: i32, fill: char) {
    if k <= 0.0 {
        return;
    }
    let (w, h) = (NATIVE_W, NATIVE_H);
    for y in 0..h {
        let across = y < depth || y >= h - depth;
        let mut x = 0;
        while x < w {
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

/// The sting on the seat looking: a red rim when a lane they stand in is
/// about to burn, a hard one as they are sooted, and the warm flash round
/// the edge when a chain tears the store.
pub(super) fn danger(over: &mut Img, run: &Run, p: &Powder, focus: Option<u32>, now: u32) {
    let Some(me) = focus else { return };
    if let Some(hero) = run.players.get(&me).filter(|h| h.hp > 0) {
        let tile = ((hero.x / crate::rules::TILE_UNITS) as i32, (hero.y / crate::rules::TILE_UNITS) as i32);
        let soon = p
            .kegs
            .iter()
            .filter(|k| k.fuse < 22 && set::in_lane(run, p, k, tile))
            .map(|k| k.fuse)
            .min();
        if let Some(fuse) = soon.filter(|&f| f < 22 && !p.done && !p.titled()) {
            let pulse = 0.5 + 0.5 * crate::motion::math::sin(now as f32 * 0.5);
            rim(over, (1.0 - fuse as f32 / 22.0) * (0.25 + 0.25 * pulse), 8, '7');
        }
    }
    for s in p.show.soots.iter().filter(|s| u32::from(s.who) == me) {
        let age = now.saturating_sub(s.born);
        if age < 18 {
            let k = 1.0 - age as f32 / 18.0;
            rim(over, k * 0.8, if age < 3 { 20 } else { 12 }, if age < 3 { 'R' } else { '7' });
        }
    }
    for b in p.show.blasts.iter().filter(|b| b.weight >= Weight::Kill && b.depth >= 1) {
        let Some(age) = b.age(now).checked_sub(u32::from(b.depth) * LINK).filter(|&a| a < 10) else { continue };
        rim(over, (1.0 - age as f32 / 10.0) * 0.3, 10, 'o');
    }
}

/// The last five seconds, one big number a second, slammed in. Returns
/// nothing; its box is claimed first so the calls give way to it.
pub(super) fn last_seconds(led: &mut Ledger, over: &mut Img, p: &Powder) {
    if p.done || p.titled() || p.left() > 5 {
        return;
    }
    let rem = (TITLE + crate::rules::powder::POWDER_TIME).saturating_sub(p.t);
    if rem == 0 {
        return;
    }
    let (sec, age) = (rem.div_ceil(HZ), HZ - 1 - (rem - 1) % HZ);
    let scale = 4 + (3 - age as i32).max(0);
    let mut digit = juice::big_text_img(&sec.to_string(), scale, if age < 3 { 'c' } else { 'R' }, 'B');
    juice::dissolve(&mut digit, ((age as f32 - 20.0) / 10.0).clamp(0.0, 1.0));
    let (x, y) = (NATIVE_W / 2 - digit.w / 2, 72 - (scale - 4) * 3);
    led.claim("last", (x, y, digit.w, digit.h));
    over.stamp(&digit, x, y);
}
