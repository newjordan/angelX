//! Book Hop's moments over a seat's view, each a pure function of the hop's
//! own ticks and each word a claim in the screen's ledger (`words`): the
//! title slam and the 3-2-1-GO, the callout banners, a crossing flying to
//! the strip, a drowned knight's gloop, the minute's last seconds, and the
//! rims of ink and gold round the view.

use super::super::super::ink::{Img, bayer};
use super::super::juice::{self, Tone};
use super::super::{NATIVE_H, NATIVE_W, enlarge};
use super::words::{Ledger, banner_rect};
use super::{bits, hud};
use crate::motion::ease;
use crate::rules::folly::TITLE;
use crate::rules::hop::{HOP_TIME, Hop};
use crate::rules::hop_show::{Cry, SHOUT_LIFE, SINK_LIFE, Shout};
use crate::rules::{HZ, Run};

/// Ticks each of 3, 2 and 1 shows, and the first one's start.
const BEAT: u32 = 12;
const FIRST: u32 = TITLE - 3 * BEAT + 1;
/// Ticks GO shows.
pub(super) const GO: u32 = 28;
/// Where the countdown's middle is.
const COUNT_AT: (i32, i32) = (NATIVE_W / 2, 100);

/// Whether a banner would sit in the clear; if so, draw it (and claim it).
#[allow(clippy::too_many_arguments)]
fn banner(led: &mut Ledger, over: &mut Img, what: &'static str, y: i32, title: &str, sub: &str, age: u32, life: u32, tone: Tone, cap: i32) -> bool {
    if age >= life || !led.free(banner_rect(y, title, sub, age, life, cap)) {
        return false;
    }
    led.banner(over, what, y, title, sub, age, life, tone, cap);
    true
}

/// The title, its two plain words with their pictures, and the countdown.
pub(super) fn opening(led: &mut Ledger, over: &mut Img, run: &Run, h: &Hop) {
    let t = h.t;
    let _ = run;
    if t >= TITLE + GO {
        return;
    }
    if t >= TITLE {
        let a = t - TITLE;
        if led.free(count_box("GO")) {
            led.count(over, "go", COUNT_AT, "GO", a, GO, true);
        }
        return;
    }
    // The view dims behind the title, and clears as the numbers come.
    let k = if t < FIRST { 0.8 } else { 0.8 * (1.0 - (t - FIRST) as f32 / 12.0).max(0.0) };
    for y in 0..NATIVE_H {
        for x in 0..NATIVE_W {
            if bayer(x, y) < k {
                over.put(x, y, 'k');
            }
        }
    }
    if t < FIRST {
        banner(led, over, "title", 18, "BOOK HOP", "", t, FIRST, Tone::Warm, 3);
    }
    // Two plain words, each with its picture: a book to ride, a lantern to reach.
    if (6..FIRST).contains(&t) {
        let tick = run.tick as u32;
        let bob = (crate::motion::math::sin(tick as f32 * 0.25) * 2.0) as i32;
        let book = enlarge(&bits::tome(32, 0, false), 2.0);
        over.stamp(&book, 112 - book.w / 2, 76 + bob);
        led.big(over, "ride", 112, 112, "RIDE", 2, 'T', 'B');
        let lamp = enlarge(&bits::lantern(Some('6'), tick), 3.0);
        over.stamp(&lamp, 272 - lamp.w / 2, 66 - bob);
        led.big(over, "across", 272, 112, "ACROSS", 2, 'T', 'B');
    }
    if t >= FIRST {
        let n = ((t - FIRST) / BEAT).min(2);
        let label = ["3", "2", "1"][n as usize];
        if led.free(count_box(label)) {
            led.count(over, "count", COUNT_AT, label, (t - FIRST) % BEAT, BEAT, false);
        }
    }
}

/// The biggest box a countdown beat can take.
fn count_box(label: &str) -> (i32, i32, i32, i32) {
    let tw = crate::render::overworld::ink::text_width(label).max(5);
    let rest = (150 / tw).clamp(4, 9);
    let scale = ((rest as f32 * 1.7).round() as i32).max(1);
    let (w, h) = (tw * scale + 4, 7 * scale + 4);
    (COUNT_AT.0 - w / 2, COUNT_AT.1 - h / 2, w, h)
}

fn words(cry: Cry) -> String {
    match cry {
        Cry::First => "FIRST ACROSS".into(),
        Cry::Clean(n) if n >= 5 => format!("ON FIRE {n}"),
        Cry::Clean(n) => format!("CLEAN {n}"),
        Cry::Lead => "TAKES THE LEAD".into(),
        Cry::Lit(_) => "ARCHIVE LIT".into(),
        Cry::Half => "HALF TIME".into(),
        Cry::Last => "LAST TEN".into(),
    }
}

/// The banner: the loudest call of the newest moment, under the far bank;
/// under it a line of small words: whose it is (not yours) and the other
/// calls the same moment made.
pub(super) fn callouts(led: &mut Ledger, over: &mut Img, run: &Run, h: &Hop, focus: Option<u32>, now: u32) {
    let mut calls: Vec<Shout> = h.show.shouts.iter().copied().filter(|c| now.saturating_sub(c.born) < SHOUT_LIFE).collect();
    calls.sort_by_key(|c| (std::cmp::Reverse(c.born), std::cmp::Reverse(c.cry.rank())));
    let Some(top) = calls.first().copied() else { return };
    let same: Vec<Shout> = calls.iter().copied().filter(|c| c.born == top.born).collect();
    let mut sub = Vec::new();
    let who = u32::from(top.who);
    if let Cry::Lit(_) = top.cry {
        sub.push(format!("ALL ACROSS +{}G", crate::rules::hop_show::ARCHIVE_GOLD));
    } else if who != 0 && focus != Some(who) {
        sub.push(hud::called(run, who, focus, 9));
    }
    sub.extend(same.iter().filter(|c| c.cry != top.cry).map(|c| words(c.cry)));
    let mine = who != 0 && focus == Some(who);
    let tone = match top.cry {
        Cry::Last => Tone::Hot,
        Cry::Lit(_) | Cry::Half | Cry::First => Tone::Warm,
        _ if mine => Tone::Warm,
        _ => Tone::Steel,
    };
    let title = words(top.cry);
    let cap = if crate::render::overworld::ink::text_width(&title) * 3 <= 164 { 3 } else { 2 };
    banner(led, over, "call", 36, &title, &sub.join("  "), now.saturating_sub(top.born), SHOUT_LIFE, tone, cap);
}

/// What flies to the strip: each crossing's count, as a number for the seat
/// looking (a crumb of the knight's ink for the others).
pub(super) fn fly_ups(led: &mut Ledger, over: &mut Img, run: &Run, h: &Hop, focus: Option<u32>, now: u32) {
    for c in &h.show.crosses {
        let age = now.saturating_sub(c.born);
        if age >= hud::FLY {
            continue;
        }
        let from = (c.at.0.clamp(10, NATIVE_W - 10), 24);
        let who = u32::from(c.who);
        let to = hud::anchor(h, run, who, focus);
        if focus == Some(who) {
            led.fly(over, "flying", "+1", from, to, age.saturating_sub(4), hud::FLY - 4, if c.clean >= 3 { '@' } else { 'T' });
        } else if age >= 3 {
            let t = ease::both((age - 3) as f32 / (hud::FLY - 3) as f32);
            let (x, y) = crate::motion::arc((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32), 14.0, t);
            over.rect(x as i32, y as i32, 2, 2, super::super::knights::seat_mark(who));
        }
    }
}

/// The seat looking sinks: a drowned word over the splash.
pub(super) fn gloops(led: &mut Ledger, over: &mut Img, h: &Hop, focus: Option<u32>, now: u32) {
    let Some(me) = focus else { return };
    for s in h.show.sinks.iter().filter(|s| u32::from(s.who) == me) {
        let age = now.saturating_sub(s.born);
        if age < 3 || age >= SINK_LIFE.min(28) {
            continue;
        }
        let word = if s.wall { "BONK" } else { "GLOOP" };
        led.pop(over, "gloop", word, (s.at.0.clamp(24, NATIVE_W - 24), s.at.1 - 14), age - 3, 24, 2, if s.wall { 'R' } else { 'z' });
    }
}

/// The last five seconds, one big number a second, slammed in.
pub(super) fn last_seconds(led: &mut Ledger, over: &mut Img, h: &Hop) {
    if h.done || h.titled() || h.left() > 5 {
        return;
    }
    let rem = (TITLE + HOP_TIME).saturating_sub(h.t);
    if rem == 0 {
        return;
    }
    let (sec, age) = (rem.div_ceil(HZ), HZ - 1 - (rem - 1) % HZ);
    let scale = 4 + (3 - age as i32).max(0);
    let mut digit = juice::big_text_img(&sec.to_string(), scale, if age < 3 { 'c' } else { 'R' }, 'B');
    juice::dissolve(&mut digit, ((age as f32 - 20.0) / 10.0).clamp(0.0, 1.0));
    let (x, y) = (NATIVE_W / 2 - digit.w / 2, 88 - (scale - 4) * 3);
    led.claim("last", (x, y, digit.w, digit.h));
    over.stamp(&digit, x, y);
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

/// The ink closing in on the seat that sank, gold round the one that
/// crossed, and gold again as the archive is lit.
pub(super) fn danger(over: &mut Img, h: &Hop, focus: Option<u32>, now: u32) {
    if let Some(me) = focus {
        for s in h.show.sinks.iter().filter(|s| u32::from(s.who) == me) {
            let age = now.saturating_sub(s.born);
            if age < 18 {
                rim(over, (1.0 - age as f32 / 18.0) * 0.95, if s.wall { 18 } else { 15 }, if s.wall { '7' } else { 'k' });
            }
        }
        for c in h.show.crosses.iter().filter(|c| u32::from(c.who) == me) {
            let age = now.saturating_sub(c.born);
            if age < 12 {
                rim(over, (1.0 - age as f32 / 12.0) * 0.6, 10, 'o');
            }
        }
    }
    if let Some(a) = h.show.flared.map(|f| now.saturating_sub(f)).filter(|&a| a < 24) {
        rim(over, (1.0 - a as f32 / 24.0) * 0.7, 14, '@');
    }
    // The last seconds, a red rim beating once a second.
    if !h.done && !h.titled() && h.left() <= 5 {
        let beat = (h.t % HZ) as f32 / HZ as f32;
        rim(over, (1.0 - beat) * 0.45, 10, 'R');
    }
}
