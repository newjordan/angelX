//! The moments of the siege: the title and the 3-2-1, GO, the banners for a
//! pennon down, a run of hits, the captain waking, his banner torn, the
//! siege broken, the table's rally and a lead taken, the last five seconds,
//! and the world's shake. All read off the show's records by their age, so
//! every seat sees the same moment from its own lane.

use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone};
use super::super::pfx::{self, Pen, ramps};
use super::actors::Cx;
use super::hud::{Words, called, claim_big};
use crate::motion::{Spring, ease};
use crate::rules::juice::Weight;
use crate::rules::pennon::show::{CALL_LIFE, Call, Cry, GO_LIFE, HOLD, Puff};
use crate::rules::{HZ, Run};

const TITLE_Y: i32 = 20;
/// Where the count and the GO land: high, so the strides under them stay in view.
const SLAM_Y: i32 = 104;

// ─── the shake ───────────────────────────────────────────────────────────────

/// A blow a notch lighter: what another knight's finale does to your screen.
fn lighter(w: Weight) -> Weight {
    match w {
        Weight::Finale => Weight::Crushing,
        Weight::Kill | Weight::Crushing => Weight::Solid,
        _ => Weight::Graze,
    }
}

/// The world's shake for the knight looked through: its own blows and falls
/// in full, the captain's blow coming down, and a rival breaking the siege a
/// notch lighter. The picture keeps its phase while the room is held, so a
/// freeze still trembles.
pub(super) fn shake(run: &Run, cx: &Cx) -> (i32, i32) {
    let show = &cx.s.show;
    let now = show.t;
    let phase = u64::from(now) * 7 + u64::from(run.hitstop.left) * 3;
    let mut blows: Vec<(Weight, u32)> = Vec::new();
    for h in &show.hits {
        if h.seat == cx.id {
            blows.push((h.weight, h.age(now)));
        } else if h.kill && h.foe == 3 {
            blows.push((lighter(h.weight), h.age(now)));
        }
    }
    for k in show.downs.iter().filter(|k| k.seat == cx.id) {
        blows.push((if k.ward { Weight::Solid } else { Weight::Crushing }, now.saturating_sub(k.born)));
    }
    for m in show.moments.iter().filter(|m| m.seat == cx.id && m.kind == Puff::Strike && m.dx == 3) {
        blows.push((Weight::Crushing, m.age(now)));
    }
    juice::shake(blows, phase)
}

// ─── the opening ─────────────────────────────────────────────────────────────

/// A figure slamming in at `at`, springing to size, ringing, and going: the
/// kit's countdown at the size of this screen.
fn slam(im: &mut Img, at: (i32, i32), label: &str, age: u32, life: u32, go: bool) {
    if age >= life {
        return;
    }
    let width = text_width(label).max(5);
    let rest = (88 / width).clamp(4, 5);
    let pop = Spring::SNAPPY.at(age as f32, 1.6, 1.0, 0.0).clamp(0.9, 1.6);
    let out = ((age as f32 - life as f32 * 0.7) / (life as f32 * 0.3)).max(0.0);
    let scale = (((rest as f32 * pop) * (1.0 - out * 0.4)).round() as i32).max(1);
    let (fill, edge) = if go { ('5', 'p') } else { ('T', 'B') };
    let mut t = juice::big_text_img(label, scale, if age < 2 { 'H' } else { fill }, edge);
    juice::dissolve(&mut t, out);
    {
        let mut pen = Pen::new(im, 700);
        let a = age as f32;
        let c = (at.0 as f32, at.1 as f32 + 4.0);
        pen.shock(c, (6.0 + a * 10.0).min(84.0), 2.2, 0.5, if go { ramps::GOLD } else { ramps::BONE }, (a / 14.0).min(1.0));
        if go {
            pen.shock(c, (2.0 + a * 16.0).min(110.0), 1.6, 0.5, ramps::EMBER, (a / 12.0).min(1.0));
            pfx::rays(&mut pen, (at.0 as f32, at.1 as f32), 14, 64.0, 0.7, 0.2, ramps::GOLD, (a / 14.0).min(1.0));
        }
    }
    let (x, y) = (at.0 - t.w / 2, at.1 - t.h / 2);
    im.stamp(&t, x, y);
    claim_big("slam", (x, y, t.w, t.h), label);
}

/// The kit's banner, with the boxes it draws noted for the tests.
fn banner(im: &mut Img, y: i32, title: &str, sub: &str, age: u32, life: u32, tone: Tone, cap: i32) {
    if age >= life {
        return;
    }
    juice::banner(im, y, title, sub, age, life, tone, cap);
    let w = im.w;
    let tw = text_width(title).max(1);
    let rest = ((w - 24) / tw).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let tail = life.saturating_sub(14);
    let out = if age > tail { (age - tail) as f32 / 14.0 } else { 0.0 };
    let lift = (ease::in_quad(out) * 10.0) as i32;
    let (bw, bh) = (tw * scale + 4, 7 * scale + 4);
    claim_big("banner", (w / 2 - bw / 2, y - lift - (scale - rest) * 3, bw, bh), title);
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub).max(1) + 4;
        claim_big("banner-sub", (w / 2 - sw / 2, y + 7 * rest + 12 - lift, sw, 11), sub);
    }
}

/// The title card, then the 3-2-1, then GO over the first strides.
pub(super) fn opening(im: &mut Img, cx: &Cx) {
    let t = cx.s.show.t;
    if cx.s.hold > 0 {
        banner(im, TITLE_Y, "PENNON RUSH", "BREAK THE SIEGE", t, HOLD, Tone::Warm, 3);
        let beat = (t * 3 / HOLD).min(2);
        let age = t - beat * HOLD / 3;
        slam(im, (192, SLAM_Y), ["3", "2", "1"][beat as usize], age, HOLD / 3, false);
    } else {
        let age = t.saturating_sub(HOLD);
        if age < GO_LIFE {
            slam(im, (192, SLAM_Y), "GO", age, GO_LIFE, true);
        }
    }
}

// ─── the calls ───────────────────────────────────────────────────────────────

/// What a call says, how loud, how inked.
struct Words2 {
    title: String,
    sub: String,
    tone: Tone,
}

fn words(run: &Run, cx: &Cx, c: &Call) -> Option<Words2> {
    let me = cx.id;
    let mine = c.who == me;
    let table = cx.s.lanes.len() > 1;
    let (title, sub, tone) = match c.cry {
        Cry::Wave(n) if mine => (
            "SENTRY DOWN".to_string(),
            if c.first && table { "FIRST AT TABLE".to_string() } else { format!("{n} OF 3") },
            Tone::Warm,
        ),
        Cry::Combo(n) if mine => (format!("X{n} COMBO"), "UNBROKEN".to_string(), Tone::Warm),
        Cry::Captain if mine => ("THE CAPTAIN".to_string(), "READ HIS TELL".to_string(), Tone::Hot),
        Cry::Torn(1) if mine => ("BANNER TORN".to_string(), "HE QUICKENS".to_string(), Tone::Hot),
        Cry::Torn(_) if mine => ("LAST STAND".to_string(), "BREAK HIM".to_string(), Tone::Hot),
        Cry::Broken if mine => {
            let falls = cx.d.falls;
            let sub = if c.first && table {
                "FIRST HOME".to_string()
            } else if falls == 0 {
                "NO FALLS".to_string()
            } else {
                format!("{falls} FALLS")
            };
            ("SIEGE BROKEN".to_string(), sub, Tone::Warm)
        }
        Cry::Broken => (
            "SIEGE BROKEN".to_string(),
            format!("{} {}", called(run, c.who, Some(me), 9), if c.first { "FIRST" } else { "TOO" }),
            Tone::Steel,
        ),
        Cry::Rally => ("WARD FOR ALL".to_string(), "TABLE RALLY".to_string(), Tone::Warm),
        Cry::Lead if mine => ("TAKES THE LEAD".to_string(), "YOU".to_string(), Tone::Warm),
        Cry::Lead => ("TAKES THE LEAD".to_string(), called(run, c.who, Some(me), 9), Tone::Steel),
        _ => return None,
    };
    Some(Words2 { title, sub, tone })
}

/// The loudest call standing for the seat, if any.
pub(super) fn call<'a>(run: &Run, cx: &'a Cx) -> Option<(&'a Call, u32)> {
    let now = cx.s.show.t;
    cx.s
        .show
        .calls
        .iter()
        .filter(|c| words(run, cx, c).is_some())
        .max_by_key(|c| (c.cry.rank(), c.born))
        .map(|c| (c, c.age(now)))
}

/// The call of the moment, high on the screen where nothing else is.
pub(super) fn callout(im: &mut Img, run: &Run, cx: &Cx) {
    let Some((c, age)) = call(run, cx) else { return };
    if cx.s.hold > 0 || age >= CALL_LIFE {
        return;
    }
    let Some(w) = words(run, cx, c) else { return };
    let cap = if matches!(c.cry, Cry::Broken | Cry::Captain) && (c.who == cx.id) { 3 } else { 2 };
    banner(im, 32, &w.title, &w.sub, age, CALL_LIFE, w.tone, cap);
}

// ─── the clock ───────────────────────────────────────────────────────────────

/// The last five seconds as numbers, big, at the right: the strip's clock
/// only keeps time, these are the call.
pub(super) fn last_seconds(im: &mut Img, cx: &Cx) {
    let s = cx.s;
    if s.hold > 0 || s.left == 0 || cx.d.finish.is_some() {
        return;
    }
    let secs = s.left.div_ceil(HZ);
    if secs > crate::rules::pennon::show::LAST_SECONDS {
        return;
    }
    let phase = (s.left + HZ - 1) % HZ;
    let beat = 1 + i32::from(phase >= HZ - 6) + i32::from(phase >= HZ - 3);
    let t = juice::big_text_img(&secs.to_string(), 2 + beat, if phase >= HZ - 6 { 'H' } else { 'R' }, 'n');
    let (x, y) = (384 - 10 - t.w, 30);
    im.stamp(&t, x, y);
    claim_big("last", (x, y, t.w, t.h), &secs.to_string());
    if phase >= HZ - 4 {
        juice::vignette(im, (0, 14, 384, 188), 0.35 * (phase - (HZ - 4)) as f32 / 4.0 + 0.1, 16, 'R');
    }
}

/// The hint keys fade when the knight has begun.
pub(super) fn hints(im: &mut Img, words: &mut Words, cx: &Cx) {
    let early = cx.s.hold > 0 || cx.s.show.t < HOLD + 3 * HZ;
    if early && super::hud::green(cx.d) && cx.d.finish.is_none() {
        super::hud::keys(im, words, 186);
    }
}

/// After the world was thrown aside, the strips it uncovered at its edges
/// are filled from the nearest column or row, so a shake never shows the
/// canvas's black.
pub(super) fn mend_edges(im: &mut Img, (dx, dy): (i32, i32)) {
    let (top, bottom) = (super::hud::VIEW_TOP, super::hud::VIEW_BOTTOM);
    if dx > 0 {
        for y in top..bottom {
            if let Some(c) = im.get(dx, y) {
                for x in 0..dx {
                    im.set(x, y, c);
                }
            }
        }
    } else if dx < 0 {
        let edge = im.w + dx - 1;
        for y in top..bottom {
            if let Some(c) = im.get(edge, y) {
                for x in edge + 1..im.w {
                    im.set(x, y, c);
                }
            }
        }
    }
    if dy > 0 {
        for x in 0..im.w {
            if let Some(c) = im.get(x, top + dy) {
                for y in top..top + dy {
                    im.set(x, y, c);
                }
            }
        }
    } else if dy < 0 {
        for x in 0..im.w {
            if let Some(c) = im.get(x, bottom + dy - 1) {
                for y in bottom + dy..bottom {
                    im.set(x, y, c);
                }
            }
        }
    }
}

/// A knight who has broken the siege watches the others run it: paper falls
/// and the camp's sky is lit with fireworks for a few seconds.
pub(super) fn celebrate(im: &mut Img, cx: &Cx) {
    let Some(fin) = cx.d.finish else { return };
    if cx.s.left == 0 {
        return;
    }
    let since = cx.s.show.t.saturating_sub(HOLD).saturating_sub(fin);
    if since > 6 * HZ {
        return;
    }
    let n = if since > 4 * HZ { 90 * (6 * HZ - since) / (2 * HZ) } else { 90 };
    juice::confetti(im, (0, 14, 384, 188), since, 7, n);
    for (j, (x, top, ramp)) in [(48.0f32, 84.0f32, ramps::GOLD), (336.0, 70.0, ramps::EMBER), (30.0 + 300.0 * 0.5, 96.0, ramps::HOLY)].into_iter().enumerate() {
        let tt = since as i32 - 3 - j as i32 * 17;
        if tt >= 0 && tt / 52 < 3 {
            let round = (tt / 52) as u32;
            juice::firework(im, x + (round as f32 * 13.0) % 20.0 - 10.0, 150.0, top, (tt % 52) as u32, 31 * j as u32 + round * 7, ramp, cx.s.show.t as u64);
        }
    }
}
