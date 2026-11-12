//! The match's moments, each a pure function of the round's own ticks:
//! the title slam and the 3-2-1-FIGHT, the callout banners, scores that fly
//! to the board, the kill cam's words and the winning blow's finish.
use super::super::juice::{self, Tone};
use super::{WARM, called, word};
use crate::rules::deathmatch::{self as dm, Session};
use crate::rules::juice::{CALL_LIFE, Call, Cry, Weight};
use crate::render::overworld::ink::{Img, bayer, text_width};

/// The first tick of the countdown's "3": the title has had its moment.
const TITLE_BEAT: u32 = 18;
/// Ticks each of 3, 2 and 1 shows.
const BEAT: u32 = (dm::TITLE - TITLE_BEAT) / 3;
/// Ticks the word FIGHT shows after the title ends.
const FIGHT: u32 = 28;
/// Ticks the winning blow plays before the scene cuts to the podium.
pub(super) const FINALE: u32 = 54;
/// Ticks the podium's iris takes to open.
const OPEN: u32 = 14;

/// The screen shake the countdown asks for at `ticks`: the title lands, and
/// FIGHT kicks.
pub(super) fn opening_shake(ticks: u32) -> (u32, f32) {
    match ticks {
        0..=5 => (ticks, 3.0),
        t if (dm::TITLE..dm::TITLE + 8).contains(&t) => (t - dm::TITLE, 5.0),
        _ => (99, 0.0),
    }
}

/// How the world is dimmed behind the title: a dither of black over the
/// view, deep at first, gone as the countdown starts.
pub(super) fn curtain(im: &mut Img, ticks: u32) {
    if ticks > TITLE_BEAT + 12 {
        return;
    }
    let k = if ticks < TITLE_BEAT { 0.62 } else { 0.62 * (1.0 - (ticks - TITLE_BEAT) as f32 / 12.0) };
    for y in 0..180 {
        for x in 0..im.w {
            if bayer(x, y) < k {
                im.put(x, y, 'k');
            }
        }
    }
}

/// The title card and the countdown over the view: the realm's name slams
/// down, three plain words under it, then 3, 2, 1 and FIGHT with a ring
/// off each.
pub(super) fn opening(im: &mut Img, s: &Session) {
    let t = s.ticks;
    let centre = (im.w / 2, 82);
    if t <= dm::TITLE {
        if t < TITLE_BEAT + 6 {
            let y = 34 - (6u32.saturating_sub(t) as i32 * 8);
            juice::banner(im, y, "KNIGHTS WISH", "", t, TITLE_BEAT + 6, Tone::Warm, 3);
            if t >= 6 {
                let line = if s.fighters.len() < 2 { "WAITING FOR FRIENDS" } else { "MOVE  AIM  FIRE" };
                let mut small = juice::big_text_img(line, 1, 'h', 'k');
                juice::dissolve(&mut small, ((t as f32 - 14.0) / 10.0).clamp(0.0, 1.0));
                im.stamp(&small, im.w / 2 - small.w / 2, 70);
            }
        }
        if t >= TITLE_BEAT {
            let n = (t - TITLE_BEAT) / BEAT;
            let age = (t - TITLE_BEAT) % BEAT;
            let label = ["3", "2", "1"][n.min(2) as usize];
            juice::countdown(im, centre, label, age, BEAT, false);
        }
    } else if t < dm::TITLE + FIGHT {
        juice::countdown(im, centre, "FIGHT", t - dm::TITLE, FIGHT, true);
    }
}

/// The calls to show: the newest kill's loudest, and the next loudest of
/// the same or an earlier kill, each with its age.
fn active(s: &Session) -> Vec<Call> {
    let mut calls: Vec<Call> = s.show.calls.iter().copied().filter(|c| c.age(s.ticks) < CALL_LIFE).collect();
    calls.sort_by_key(|c| (std::cmp::Reverse(c.born), std::cmp::Reverse(c.cry.rank())));
    calls
}

fn words(cry: Cry) -> &'static str {
    match cry {
        Cry::FirstBlood => "FIRST BLOOD",
        Cry::Chain(2) => "DOUBLE KILL",
        Cry::Chain(3) => "TRIPLE KILL",
        Cry::Chain(_) => "RAMPAGE",
        Cry::Spree(3) => "KILLING SPREE",
        Cry::Spree(5) => "UNSTOPPABLE",
        Cry::Spree(_) => "LEGENDARY",
        Cry::Shutdown(_) => "SHUTDOWN",
        Cry::Revenge => "REVENGE",
        Cry::Lead => "TAKES THE LEAD",
        Cry::Point => "MATCH POINT",
    }
}

/// The banner: the loudest call of the newest kill, big, in the upper third
/// between the board and the feed; under it a line of small words: whose it
/// is (not yours) and the other calls the same kill made.
pub(super) fn callouts(im: &mut Img, s: &Session, id: u32) {
    let calls = active(s);
    let Some(top) = calls.first().copied() else { return };
    let same: Vec<Call> = calls.iter().copied().filter(|c| c.born == top.born && c.who == top.who).collect();
    let mut sub = Vec::new();
    if top.who != id {
        sub.push(called(s, top.who, id, 10));
    }
    sub.extend(same.iter().skip(1).map(|c| words(c.cry).to_string()));
    let tone = match (top.cry, top.who == id) {
        (Cry::Point, false) | (Cry::Shutdown(_), false) => Tone::Hot,
        (_, true) => Tone::Warm,
        _ => Tone::Steel,
    };
    let title = words(top.cry);
    let cap = if text_width(title) * 3 <= 164 { 3 } else { 2 };
    juice::banner(im, 46, title, &sub.join("  "), top.age(s.ticks), CALL_LIFE, tone, cap);
}

/// Kills as scores that fly: a `+1` off the fallen knight (`from`, in
/// screen pixels) to the board row of whoever took it, or to your own
/// counter when it was you.
pub(super) fn fly_ups(im: &mut Img, s: &Session, id: u32, from: impl Fn(u32) -> (i32, i32)) {
    let ranks = s.order();
    for b in s.show.blows.iter().filter(|b| matches!(b.weight, Weight::Kill | Weight::Finale)) {
        let age = b.age(s.ticks);
        let to = if b.owner == id {
            (352, 207)
        } else {
            let row = ranks.iter().position(|&r| r == b.owner).unwrap_or(0) as i32;
            (92, 27 + row * 10)
        };
        juice::fly_up(im, "+1", from(b.target), to, age.saturating_sub(5), 22, if b.owner == id { '@' } else { 'T' });
    }
}

/// The kill cam's words while a knight waits to rise: who slew them, the
/// count, and a bar that fills as they come back.
pub(super) fn slain(im: &mut Img, s: &Session, id: u32) {
    let f = &s.fighters[&id];
    let killer = s.feed.iter().rev().find(|k| k.victim == id).map(|k| k.killer);
    let age = dm::RESPAWN.saturating_sub(f.respawn);
    let rise = ((age as f32 - 4.0) / 8.0).clamp(0.0, 1.0);
    let y = 96 - ((1.0 - rise) * 14.0) as i32;
    if rise > 0.0 {
        let mut a = juice::big_text_img("SLAIN BY", 2, 'T', 'B');
        juice::dissolve(&mut a, 1.0 - rise);
        im.stamp(&a, im.w / 2 - a.w / 2, y);
        if let Some(k) = killer {
            let mut b = juice::big_text_img(&called(s, k, id, 12), 3, 'c', 'p');
            juice::dissolve(&mut b, 1.0 - rise);
            im.stamp(&b, im.w / 2 - b.w / 2, y + 21);
        }
    }
    // The way back: a dithered bar filling toward the rise.
    let (x, w) = (im.w / 2 - 50, 100);
    let filled = (w as u32 * age / dm::RESPAWN.max(1)) as i32;
    for dx in 0..w {
        for dy in 0..4 {
            let ink = if dx < filled { if dy == 0 { 'T' } else { 'o' } } else if bayer(dx, dy) < 0.4 { 'g' } else { 'K' };
            im.put(x + dx, 151 + dy, ink);
        }
    }
    word(im, im.w / 2 - text_width("RISING") / 2, 159, "RISING", false);
}

/// The winning blow's finish, `ended` ticks after it: a flash, the black
/// bars of a film, the words FINAL BLOW; at the end the iris shuts for the
/// podium. The world behind keeps its own life.
pub(super) fn finale(im: &mut Img, s: &Session, ended: u32, beat: u32, id: u32) {
    let winner = s.winner.unwrap_or(0);
    // The flash of the blow: a thin dither of light that burns off over the freeze and a few ticks.
    if beat < 8 {
        let k = 1.0 - beat as f32 / 8.0;
        for y in 0..180 {
            for x in 0..im.w {
                if bayer(x, y) < k * 0.13 {
                    im.put(x, y, 'i');
                }
            }
        }
    }
    // The film bars slide in from the top and the bottom of the view.
    let bars = (juice_ease(((ended as f32 - 2.0) / 10.0).clamp(0.0, 1.0)) * 16.0) as i32;
    let out = ((ended as f32 - (FINALE - 14) as f32) / 14.0).clamp(0.0, 1.0);
    for y in 0..bars {
        for x in 0..im.w {
            im.put(x, y, 'k');
            im.put(x, 179 - y, 'k');
        }
    }
    if (4..FINALE - 10).contains(&ended) {
        let who = if winner == id { String::new() } else { called(s, winner, id, 10) };
        juice::banner(im, 30, "FINAL BLOW", &who, ended - 4, FINALE - 14, Tone::Warm, 3);
    }
    juice::iris(im, (im.w / 2, 90), out, 'k');
    let _ = WARM;
}

fn juice_ease(t: f32) -> f32 {
    crate::motion::ease::out(t)
}

/// How far the podium's iris has opened, `ended` ticks into the match's end.
pub(super) fn podium_open(ended: u32) -> f32 {
    ((ended as f32 - FINALE as f32) / OPEN as f32).clamp(0.0, 1.0)
}
