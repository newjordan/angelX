//! The finish: when the bell rings the road goes to the Bell Gate's night
//! and the pack is on a podium, the best rider crowned, fireworks and paper
//! over the Moonwell, the rank stamped big, and what the ride was worth in a
//! row of plain numbers. All of it from the tally's tick count, so every seat
//! sees it alike.
use super::super::juice::{self, Place, Tone};
use super::super::pfx::ramps;
use super::{Img, View, Word, anim_at, backdrop, dimmed, dressed, frame_of, say, stamp_scaled};
use crate::rules::lightgallop::{
    self as rules, Session,
    level::level,
};
use crate::rules::{HZ, Run};

/// The chips' row, on its own line under the podium's names.
const CHIPS_Y: i32 = 205;
const PROMPT_Y: i32 = 215;

fn clock(ticks: u32) -> String {
    let s = ticks / HZ;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The riders best first.
pub(super) fn order(y: &rules::Tally) -> Vec<(u32, u32)> {
    let mut all: Vec<(u32, u32)> = y.scores.iter().map(|(&id, &v)| (id, v)).collect();
    // The knight who rode best (the laurel) takes the top step; the rest by points, then seat.
    all.sort_by(|a, b| (a.0 != y.best).cmp(&(b.0 != y.best)).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
    all
}

/// A rider in victory, in `seat`'s colours, `t` ticks into the scene.
fn cheering(seat: u32, t: u32) -> Img {
    let (anim, i) = anim_at("lightgallop.rider", "victory", t / 2);
    dressed(&anim.frames[i], &anim.cloth[i], seat)
}

/// The words on the finish at this moment, the chips and the way on.
pub(super) fn words(s: &Session, y: &rules::Tally, best: u32) -> Vec<Word> {
    let lvl = level();
    let t = s.elapsed.saturating_sub(y.at);
    let mut out = Vec::new();
    if t >= 40 {
        // Each one ticks up as it comes in, the way a score does.
        let up = |from: u32, to: u32, len: u32| -> u32 { to * t.saturating_sub(from).min(len) / len };
        let secs = y.time / HZ;
        let chips: [(String, bool); 4] = [
            (format!("TIME {}", clock(up(44, y.time, 24))), secs <= lvl.par / HZ),
            (format!("SPARKS {}", up(50, y.sparks, 24)), true),
            (format!("CHAIN {}", up(56, y.chain, 16)), y.chain >= 4),
            (format!("LANTERNS {} OF {}", up(62, y.posts, 10), lvl.posts.len()), y.posts as usize == lvl.posts.len()),
        ];
        let widths: Vec<i32> = chips.iter().map(|(c, _)| crate::render::overworld::ink::text_width(c)).collect();
        let gap = 14;
        let total: i32 = widths.iter().sum::<i32>() + gap * (chips.len() as i32 - 1);
        let mut x = 192 - total / 2;
        for ((text, good), w) in chips.into_iter().zip(widths) {
            out.push(Word::new(x, CHIPS_Y, &text, if good { 'T' } else { 'H' }, true));
            x += w + gap;
        }
    }
    if t >= 70 {
        let tag = if y.points >= best { format!("{} POINTS  NEW BEST", y.points) } else { format!("{} POINTS  BEST {}", y.points, best) };
        out.push(Word::centred(192, 196, &tag, if y.points >= best { '6' } else { 'v' }));
    }
    if t >= 45 {
        out.push(Word::centred(192, PROMPT_Y, "E RIDE AGAIN   HOLD ESC LEAVE", if (s.elapsed / 15) % 2 == 0 { 'T' } else { 'H' }));
    }
    out
}

/// The finish scene over the whole picture below the heads-up display's bar.
pub(super) fn scene(im: &mut Img, run: &Run, s: &Session, y: &rules::Tally) {
    let lvl = level();
    let t = s.elapsed.saturating_sub(y.at);
    let tick = u64::from(s.elapsed);
    // The stage: the Bell Gate's own night, a little darker.
    let mut stage = Img::black(384, 224);
    let v = View { cx: lvl.bell.0 - 192, cy: lvl.bell.1 - 140, zoom: 256, w: 384, h: 224 };
    backdrop(&mut stage, &v);
    let mut stage = dimmed(&stage, 0.62);
    // The warm road under their feet, and a lit lantern post at each side.
    for yy in 170..224 {
        for xx in 0..384 {
            let c = if yy < 173 { 'g' } else if crate::render::overworld::ink::bayer(xx, yy) < 0.22 { 'X' } else { 'K' };
            stage.put(xx, yy, c);
        }
    }
    for x in [22, 362] {
        stamp_scaled(&mut stage, frame_of("lightgallop.post", "lit", s.elapsed), x, 174, 256);
    }
    // Fireworks over the Moonwell, four launchers in turn, and paper.
    let tops = [(76.0, 52.0, ramps::GOLD), (306.0, 60.0, ramps::EMBER), (190.0, 40.0, ramps::BONE), (130.0, 72.0, ramps::GOLD)];
    for (j, (x, top, ramp)) in tops.into_iter().enumerate() {
        let t0 = 14 + j as u32 * 17;
        if t >= t0 {
            let round = (t - t0) / 76;
            let jitter = crate::render::overworld::ink::hash(j as i32, round as i32, 3) as f32 % 24.0 - 12.0;
            juice::firework(&mut stage, x + jitter, 150.0, top, (t - t0) % 76, 0x51 + j as u32 * 7 + round * 31, ramp, tick);
        }
    }
    im.stamp(&Img::from_view(&stage, 0, 30, 384, 194), 0, 30);
    // The podium: best first, up to three, and the rest standing by.
    let ranks = order(y);
    let places: Vec<Place> = ranks
        .iter()
        .take(3)
        .map(|&(id, score)| Place {
            sprite: cheering(id, t),
            name: run.players.get(&id).map_or_else(|| format!("KNIGHT {id}"), |h| h.name.to_uppercase().chars().take(10).collect()),
            line: format!("{score}"),
        })
        .collect();
    let crown = juice::podium(im, (0, 56, 384, 124), &places, t);
    for (n, &(id, _)) in ranks.iter().enumerate().skip(3) {
        let side = if n % 2 == 1 { 1 } else { -1 };
        let slot = ((n - 3) / 2) as i32;
        let x = 192 + side * (150 + slot * 30);
        let (anim, i) = anim_at("lightgallop.rider", "victory", t / 2 + n as u32);
        let sprite = dressed(&anim.frames[i], &anim.cloth[i], id);
        stamp_scaled(im, &sprite, x, 178, 256);
    }
    // The best rider's crown: a laurel of light.
    if t >= 30 {
        let (cx, cy) = (crown.0, crown.1 - 4);
        let k = ((t - 30) as f32 / 8.0).min(1.0);
        for dx in -6..=6i32 {
            let up = (dx.abs() < 3) as i32 * 2 + (dx % 3 == 0) as i32;
            if (dx.abs() as f32) < 7.0 * k {
                im.put(cx + dx, cy - up, if dx % 2 == 0 { '6' } else { '5' });
            }
        }
    }
    // The title slams in, then the rank.
    if t < 44 {
        juice::banner(im, 38, "ROAD CLEAR", "", t, 44, Tone::Warm, 3);
    }
    if t >= 52 {
        let letter = ["C", "B", "A", "S"][usize::from(y.rank.min(3))];
        let age = t - 52;
        let scale = if age < 3 { 10 } else if age < 6 { 8 } else { 7 };
        juice::big_text(im, 336, 58, letter, scale, if y.rank == 3 { '6' } else { 'T' }, 'B');
    }
    juice::confetti(im, (0, 31, 384, 193), s.elapsed, 0xCA7, if t >= 12 { 26 } else { 0 });
    let best = u32::from(run.home.countryside.games.roundtable[rules::SLOT].best);
    for w in words(s, y, best) {
        say(im, &w);
    }
}
