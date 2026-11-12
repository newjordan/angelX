//! Rafterwake's end of the round: the tower irises shut, the bell loft opens behind a podium of
//! timber steps, the three best knights drop onto them, fireworks go up over the belfry and paper
//! falls, the purse counts up, the hall's best is told and E asks for another climb. A pure
//! function of the run's tick and the tick the round ended on.
use crate::render::overworld::ink::{Img, bayer, text_width};
use super::super::juice::{self, Tone};
use super::super::pfx::ramps;
use super::hud::{self, H, View, W, Word};
use super::moments::{self, Banner, Plan};
use crate::motion::ease;
use crate::rules::HZ;
use std::sync::OnceLock;

/// Ticks the tower irises shut before the scene opens, and how long the scene's own iris opens.
pub(super) const SHUT: u32 = 22;
const OPEN: u32 = 18;
/// Where the steps stand: the floor row and the centres of first, second and third.
const FLOOR: i32 = 174;
const STEP_W: i32 = 66;
const XS: [i32; 3] = [192, 120, 264];
const HEIGHTS: [i32; 3] = [58, 40, 28];
const START: [u32; 3] = [14, 6, 0];

/// Ticks since the round ended, if it has.
pub(super) fn since(v: &View) -> Option<u32> {
    (v.s.left == 0).then(|| if v.s.ended == 0 { SHUT + 1000 } else { v.now.saturating_sub(v.s.ended) })
}

/// The loft's top, with the bell, across the whole screen and dimmed.
fn loft() -> &'static Img {
    static BG: [OnceLock<Img>; 2] = [const { OnceLock::new() }, const { OnceLock::new() }];
    BG[usize::from(super::super::slot_art::enabled())].get_or_init(|| {
        let mut im = Img::black(W, H);
        let source = super::slot_image("rafterwake.room", "backdrop", &Img::new(1, 1));
        if source.w > 1 {
            let shift = (source.w - W) / 2;
            for y in 0..H {
                for x in 0..W {
                    if let Some(c) = source.get(x + shift, y) {
                        // Dim toward the floor so the steps stand clear.
                        let k = if y < 120 { 2.0 / 5.0 } else { 2.0 / 5.0 * (1.0 - (y - 120) as f32 / 140.0) };
                        im.set(x, y, c.map(|v| (f32::from(v) * k) as u8));
                    }
                }
            }
        }
        let plank = &super::art().all["plank"];
        for x in (0..W).step_by(24) {
            im.stamp(plank, x, FLOOR + 1);
        }
        im.rect(0, FLOOR + 11, W, H - FLOOR - 11, 'k');
        im
    })
}

/// A round time, tenths of a second: `24.3 S`.
fn clock(ticks: u32) -> String {
    format!("{}.{} S", ticks / HZ, ticks % HZ * 10 / HZ)
}

fn ordinal(n: usize) -> String {
    let suffix = match n {
        1 => "ST",
        2 => "ND",
        3 => "RD",
        _ => "TH",
    };
    format!("{n}{suffix}")
}

/// The crown over the winner's head, 11 by 6.
fn crown(im: &mut Img, cx: i32, y: i32) {
    const ROWS: [&str; 6] = ["#...#...#..", "##.###.##..", "###########", "#####5#####", "###########", ".#########."];
    for (r, row) in ROWS.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let ink = match ch {
                '#' => '6',
                '5' => '7',
                _ => continue,
            };
            im.put(cx - 5 + c as i32, y + r as i32, ink);
        }
    }
}

/// The scene's words and banner, `t` ticks into it.
pub(super) fn plan(v: &View, t: u32) -> Plan {
    let s = v.s;
    let run = v.run;
    let order = &s.order;
    let mut words: Vec<Word> = Vec::new();
    let leader = order.first().copied().unwrap_or(v.me);
    let tied = order.iter().filter(|&&r| s.score(r) == s.score(leader)).count() > 1;
    let title = if tied {
        "TIE".to_string()
    } else if leader == v.me {
        "YOU WIN".to_string()
    } else {
        format!("{} WINS", v.name(leader, 8))
    };
    let sub = if tied {
        "POT SPLIT"
    } else if s.won {
        "LOFT CLEARED"
    } else {
        "TIME UP"
    };
    let r = moments::banner_rect(W, 12, &title, sub, t, 3);
    let banner = Banner { title, sub: sub.into(), tone: Tone::Warm, age: t, life: u32::MAX / 2, cap: 3, rank: 9, who: if tied { None } else { Some(leader) }, rect: r };
    for i in 0..3 {
        let Some(&id) = order.get(i) else { continue };
        let name = v.name(id, 8);
        let lane = &s.lanes[&id];
        let x = XS[i];
        if t >= START[i] + 18 {
            words.push(Word::pen(x - text_width(&name) / 2, FLOOR + 14, name, if id == v.me { 'T' } else { 'i' }, "podium name").of(id));
            let score = lane.finish.map_or_else(|| lane.score.to_string(), clock);
            words.push(Word::pen(x - text_width(&score) / 2, FLOOR + 23, score, 'h', "podium score").of(id));
        }
    }
    // The rest of the table in a short ledger down the right.
    for (n, &id) in order.iter().enumerate().skip(3) {
        let y = 70 + (n as i32 - 3) * 11;
        words.push(Word::pen(298, y, ordinal(n + 1)[..1].to_string(), 'h', "ledger rank").of(id));
        words.push(Word::pen(306, y, v.name(id, 8), if id == v.me { 'T' } else { 'i' }, "ledger name").of(id));
        let score = s.lanes[&id].score.to_string();
        words.push(hud::right(W - 5, y, &score, 'h', "ledger score").of(id));
    }
    // The viewer's own place and falls, down the left.
    let place = s.place(v.me);
    words.push(Word::big(14, 74, ordinal(place), 2, if place == 1 { '6' } else { 'T' }, "your place").of(v.me));
    words.push(Word::pen(14, 98, format!("FALLS {}", v.d.falls), 'h', "your falls").of(v.me));
    match v.d.finish {
        Some(t) => {
            words.push(Word::pen(14, 108, clock(t), 'h', "your time").of(v.me));
            if v.d.falls == 0 {
                words.push(Word::pen(14, 118, "CLEAN", '6', "your clean").of(v.me));
            }
        }
        None => words.push(Word::pen(14, 108, format!("DECK {}", v.d.checkpoint), 'h', "your deck").of(v.me)),
    }
    // The purse, the again and the hall's best along the foot.
    let pay = s.payout(v.me);
    let counted = (pay as f32 * ease::out(((t as f32 - 40.0) / 36.0).clamp(0.0, 1.0))) as u32;
    if t >= 40 {
        if pay > 0 {
            words.push(Word::big(10, 204, format!("+{counted}G"), 2, '6', "purse").of(v.me));
        } else {
            words.push(Word::pen(10, 209, "NO PURSE", 'J', "purse").of(v.me));
        }
    }
    if t >= 52 {
        words.push(hud::centred(W / 2, 209, "E AGAIN", if t / 8 % 2 == 0 { 'T' } else { 'o' }, "again"));
    }
    let best = run.home.countryside.games.rafterwake.best;
    let fresh = best > s.was && s.lanes.values().any(|d| d.score == u32::from(best));
    let line = if fresh { format!("NEW BEST {best}") } else { format!("BEST {best}") };
    if t >= 30 {
        words.push(hud::right(W - 8, 209, &line, if fresh { '6' } else { 'h' }, "best"));
    }
    Plan { words, banner: Some(banner), pops: Vec::new(), count: None }
}

/// The scene `t` ticks after the tower has shut.
pub(super) fn frame(v: &View, t: u32) -> (Img, Plan) {
    let s = v.s;
    let tick = u64::from(v.now);
    let plan = plan(v, t);
    let mut im = loft().clone();
    let steps = [(0usize, "first"), (1, "second"), (2, "third")];
    let (mut crown_at, mut top_of_first) = ((XS[0], FLOOR - HEIGHTS[0]), false);
    // A column of light behind the winner, dither only.
    if t >= START[0] + 14 {
        let k = ease::out(((t - START[0] - 14) as f32 / 20.0).min(1.0));
        for y in 40..FLOOR - HEIGHTS[0] + 4 {
            let half = ((y - 40) as f32 * 0.34 * k) as i32 + 6;
            for x in XS[0] - half..=XS[0] + half {
                let edge = (x - XS[0]).abs() as f32 / half.max(1) as f32;
                if bayer(x, y) < 0.1 * (1.0 - edge * 0.7) * k {
                    im.put(x, y, 'T');
                }
            }
        }
    }
    for &(i, _) in steps.iter().rev() {
        let Some(&id) = s.order.get(i) else { continue };
        let rise = ease::out(((t as f32 - START[i] as f32) / 18.0).clamp(0.0, 1.0));
        let h = (HEIGHTS[i] as f32 * rise) as i32;
        if h <= 0 {
            continue;
        }
        let top = FLOOR - h;
        let left = XS[i] - STEP_W / 2;
        for yy in top..FLOOR {
            for xx in left..left + STEP_W {
                let ink = if yy == top {
                    if i == 0 { 'T' } else { 'O' }
                } else if xx == left || xx == left + STEP_W - 1 {
                    'I'
                } else if yy - top < 3 {
                    if i == 0 { 'o' } else { 'r' }
                } else if (yy - top) % 12 == 0 {
                    'b'
                } else if bayer(xx, yy) < 0.2 {
                    'I'
                } else {
                    'B'
                };
                im.put(xx, yy, ink);
            }
        }
        if h + 2 >= HEIGHTS[i] {
            let rank = juice::big_text_img(&(i + 1).to_string(), if i == 0 { 5 } else { 4 }, if i == 0 { 'O' } else { 'o' }, 'b');
            im.stamp(&rank, XS[i] - rank.w / 2, top + 6);
        }
        let drop = ((1.0 - ease::out(((t as f32 - START[i] as f32 - 10.0) / 12.0).clamp(0.0, 1.0))) * 80.0) as i32;
        if t as f32 >= START[i] as f32 + 6.0 {
            let pose = if i == 0 { 9 } else { 0 };
            let sprite = super::knight(v.run, id, pose, 1, false);
            let (fx_, fy) = (XS[i] - sprite.w / 2, top - sprite.h - drop);
            // The landing's squash, a few ticks after it drops.
            let land = (t as f32 - START[i] as f32 - 22.0).max(0.0) as u32;
            if drop == 0 && land < 12 {
                let (body, grow) = juice::squash_stretch(&sprite, land, 0.3);
                im.stamp(&body, XS[i] - body.w / 2, top - (sprite.h as f32 * grow).round() as i32);
            } else {
                im.stamp(&sprite, fx_, fy);
            }
            if i == 0 {
                crown_at = (XS[i], fy);
                top_of_first = drop == 0;
            }
        }
    }
    if top_of_first && s.won || top_of_first && !s.order.is_empty() {
        crown(&mut im, crown_at.0, crown_at.1 - 6);
    }
    // The belfry's bell tolls for the winner: rings of gold roll out of its mouth.
    if t >= 30 {
        let mut pen = super::super::pfx::Pen::new(&mut im, 900);
        for n in 0..4u32 {
            let at = 30 + n * 17;
            if t >= at && t - at < 26 {
                let a = (t - at) as f32;
                pen.shock((195.0, 114.0), 8.0 + a * 4.4, 2.0, 0.3, ramps::GOLD, a / 26.0);
            }
        }
    }
    // Paper and fireworks over the belfry.
    if t >= 24 {
        let big = s.won || s.order.len() > 1;
        for n in 0..3u32 {
            let at = 24 + n * 16;
            if t >= at && (big || n == 0) {
                let x = [96.0, 192.0, 288.0][n as usize];
                juice::firework(&mut im, x, 120.0, 40.0 + n as f32 * 8.0, (t - at) % 70, 0x31 + n * 17, ramps::GOLD, tick);
            }
        }
        juice::confetti(&mut im, (0, 0, W, FLOOR), v.now, 0x77, if s.won { 26 } else { 12 });
    }
    // Words last: the pen's, then the banner's own rails and slam.
    for word in &plan.words {
        word.paint(&mut im);
    }
    if let Some(b) = &plan.banner {
        juice::banner(&mut im, 12, &b.title, &b.sub, b.age, b.life, b.tone, b.cap);
    }
    // The scene opens out of the iris that shut the tower.
    if t < OPEN {
        juice::iris(&mut im, (W / 2, 96), 1.0 - t as f32 / OPEN as f32, 'k');
    }
    (im, plan)
}

