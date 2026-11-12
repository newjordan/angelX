//! The end of a round: Clasp's last breath (the play screen, held and shaking, with Clasp coming
//! apart), then a hall of little Clasps cheering, three steps rising, the winner crowned, fireworks
//! and paper, the pot counting up and a time to beat. Everything is a function of the ticks since
//! the round ended, so every seat sees it alike.
use super::super::super::ink::{Img, bayer, text_width};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::ramps;
use super::hud::{self, Rect, Word, centre};
use super::{art, knight_img};
use crate::motion::ease;
use crate::rules::Run;
use crate::rules::latch::Session;

/// Ticks the play screen is held, shaking, after Clasp yields and after the bell rings.
pub(super) const HOLD_WON: u32 = 26;
pub(super) const HOLD_BELL: u32 = 22;
/// The longest of them, for banners that must last the whole hold and the wipe.
pub(super) const HOLD: u32 = HOLD_WON;
/// Ticks the iris takes to close on the play screen and to open on the podium.
pub(super) const WIPE: u32 = 10;
/// The podium's box and its floor.
const STAGE: Rect = (0, 0, 384, 176);
const FLOOR: i32 = 170;

pub(super) fn held(s: &Session) -> u32 {
    if s.won { HOLD_WON } else { HOLD_BELL }
}

/// The table in the order it finished.
fn places(s: &Session) -> Vec<u32> {
    s.order.iter().copied().filter(|id| s.duels.contains_key(id)).collect()
}

/// A knight as the podium stands them: their guard, trimmed to the figure.
fn figure(run: &Run, id: u32) -> Img {
    let Some(src) = knight_img(run, id, 0, false) else { return Img::new(8, 8) };
    let (mut x0, mut y0, mut x1, mut y1) = (src.w, src.h, 0, 0);
    for y in 0..src.h {
        for x in 0..src.w {
            if src.get(x, y).is_some() {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if x1 <= x0 {
        return src;
    }
    let mut out = Img::new(x1 - x0, y1 - y0);
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(c) = src.get(x, y) {
                out.set(x - x0, y - y0, c);
            }
        }
    }
    out
}

/// What stands under each step, the purse and the way home, as the scene shows them `age` ticks
/// after the podium opened.
fn places_of(run: &Run, s: &Session, me: Option<u32>) -> Vec<Place> {
    places(s)
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, &who)| {
            let d = &s.duels[&who];
            let line = match (i, s.won, s.win_at) {
                (0, true, Some(t)) => hud::clock_text(t),
                _ => format!("{}/3 COUNTERS", d.counters),
            };
            Place { sprite: figure(run, who), name: hud::called(run, who, me, 13), line }
        })
        .collect()
}

/// The lines of 5x7 words that are not under a step.
pub(super) fn words(run: &Run, s: &Session, me: Option<u32>, age: u32) -> Vec<Word> {
    let order = places(s);
    let mut out = Vec::new();
    // Fourth place on, small, down the left.
    if age >= 24 {
        for (k, &who) in order.iter().enumerate().skip(3) {
            let line = format!("{} {} {}/3", k + 1, hud::called(run, who, me, 6), s.duels[&who].counters);
            out.push(Word { x: 4, y: 50 + (k as i32 - 3) * 10, text: line, ink: if Some(who) == me { 'T' } else { 'h' } });
        }
    }
    if age >= 30 {
        let winner = order.first().copied().unwrap_or(1);
        let mine = me.map_or(0, |m| s.payout(m));
        let line = if mine > 0 {
            format!("+{}G", (mine as f32 * ease::out(((age - 30) as f32 / 30.0).min(1.0))).round() as u32)
        } else {
            format!("{} TAKES {}G", hud::called(run, winner, me, 10), s.payout(winner))
        };
        out.push(centre(192, 195, line, 'T'));
        let quicker = s.won && s.to_beat > 0 && s.win_at.is_some_and(|t| t < s.to_beat);
        let record = s.quickest();
        if quicker {
            out.push(centre(192, 204, "NEW BEST", 'O'));
        } else if record > 0 {
            out.push(centre(192, 204, format!("BEST {}", hud::clock_text(record)), 'G'));
        }
    }
    if age >= 40 {
        out.push(centre(192, 214, "E AGAIN", 'u'));
    }
    let _ = run;
    out
}

/// One cheering Clasp, a real sprite, its hook arm up and down, hopping on the spot.
fn cheer(im: &mut Img, x: i32, age: u32, phase: u32) {
    let beat = (age + phase * 5) / 6 % 2;
    let src = &art().states[0][beat as usize];
    let hop = if (age + phase * 5) / 6 % 2 == 0 { 0 } else { 3 };
    let mut small = Img::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            if let Some(c) = src.get(x * 100 / 32 + 1, y * 100 / 32 + 1) {
                small.set(x, y, c);
            }
        }
    }
    im.stamp(&small, x, 134 - hop);
}

/// A warm wreath over the winner's head.
fn laurel(im: &mut Img, at: (i32, i32), age: u32) {
    let k = (age as f32 / 10.0).min(1.0);
    let bob = ((age as f32 * 0.2).sin() * 1.5) as i32;
    let (x, y) = (at.0, at.1 - 8 + ((1.0 - k) * -20.0) as i32 + bob);
    for a in 0..11 {
        let t = a as f32 / 10.0;
        let (dx, dy) = ((t * 2.0 - 1.0) * 22.0, -(1.0 - (t * 2.0 - 1.0).powi(2)) * 9.0 + 6.0);
        im.rect(x + dx as i32 - 2, y + dy as i32, 5, 3, if a % 2 == 0 { 't' } else { 'O' });
        im.rect(x + dx as i32 - 1, y + dy as i32, 2, 1, 'T');
    }
}

/// The podium scene for seat `me` (or the first), `age` ticks after it opened.
pub(super) fn scene(run: &Run, s: &Session, id: u32, me: Option<u32>, age: u32) -> Img {
    let mut im = Img::black(384, 224);
    // The floor of the hall: dark flags with a warm line at the steps' feet.
    for y in 112..224 {
        for x in 0..384 {
            let flag = ((x / 24) + (y - 112) / 12) % 2 == 0;
            if bayer(x, y) < 0.14 + if flag { 0.12 } else { 0.0 } {
                im.put(x, y, 'g');
            }
        }
    }
    let order = places(s);
    // Fireworks over the hall, three launchers in turn, when Clasp is broken.
    if s.won && age >= 6 {
        for (j, (x, top, ramp)) in [(70.0, 56.0, ramps::GOLD), (192.0, 34.0, ramps::EMBER), (314.0, 60.0, ramps::BONE)].into_iter().enumerate() {
            let t = age as i32 - 6 - j as i32 * 17;
            if t >= 0 {
                let round = (t / 52) as u32;
                juice::firework(&mut im, x + (round as f32 * 23.0) % 40.0 - 20.0, 112.0, top, (t % 52) as u32, 31 * j as u32 + round * 7, ramp, age as u64);
            }
        }
    }
    // The little Clasps, real sprites, cheering in the corners of the hall.
    if age >= 10 {
        for (n, x) in [6, 40, 312, 346].into_iter().enumerate() {
            cheer(&mut im, x, age, n as u32);
        }
    }
    let places = places_of(run, s, me);
    let head = juice::podium(&mut im, STAGE, &places, age);
    for stand in juice::podium_stands((384, 224), STAGE, &places) {
        if age >= stand.words_at {
            hud::claim_box("name", stand.name);
            hud::claim_box("line", stand.line);
        }
    }
    if s.won && age >= 34 {
        laurel(&mut im, head, age - 34);
    }
    if age >= 14 && s.won {
        juice::confetti(&mut im, (0, 0, 384, 224), age, 9, 150);
    }
    // The call over it all.
    if age >= 4 {
        let winner = order.first().copied().unwrap_or(id);
        let (title, sub, tone) = if !s.won {
            ("THE BELL CLOSES".to_string(), "MOST COUNTERS WIN".to_string(), Tone::Steel)
        } else if Some(winner) == me {
            ("CLASP YIELDS".to_string(), "YOU WIN".to_string(), Tone::Warm)
        } else {
            ("CLASP YIELDS".to_string(), format!("{} WINS", hud::called(run, winner, me, 14)), Tone::Warm)
        };
        let mut layer = Img::new(384, 46);
        juice::banner(&mut layer, 8, &title, &sub, age - 4, 4000, tone, 2);
        hud::claim("title", &layer, (0, 0));
        im.stamp(&layer, 0, 0);
    }
    for w in words(run, s, me, age) {
        im.text(w.x, w.y, &w.text, w.ink);
    }
    // The iris opening from the finale's black.
    juice::iris(&mut im, (192, 100), 1.0 - (age as f32 / WIPE as f32).min(1.0), 'k');
    let _ = (text_width("A"), FLOOR);
    im
}
