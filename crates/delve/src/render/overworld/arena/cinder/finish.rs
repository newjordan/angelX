//! The end of a round: the kiln's last breath (the play screen, held and shaking, with the kiln
//! coming apart), then a hall of little Crocks cheering, three steps rising, the winner crowned,
//! fireworks and paper, the pot counting up and a time to beat. Everything is a function of the
//! ticks since the round ended, so every seat sees it alike.
use super::super::super::ink::{Img, bayer, text_width};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::ramps;
use super::hud::{self, Rect, Word};
use super::{art, knight};
use crate::motion::ease;
use crate::rules::Run;
use crate::rules::cinder::Session;

/// Ticks the play screen is held, shaking, after a cracked kiln and after the clock runs out.
pub(super) const FINALE: u32 = 46;
pub(super) const TIME_UP: u32 = 26;
/// Ticks the iris takes to close on the play screen and to open on the podium.
pub(super) const WIPE: u32 = 10;
/// The podium's box and its floor.
const STAGE: Rect = (0, 0, 384, 176);
const FLOOR: i32 = 170;
/// Ticks each step's knight takes to settle before the name and line are shown.
const SHOW: [u32; 3] = [34, 26, 18];

pub(super) fn held(s: &Session) -> u32 {
    if s.won { FINALE } else { TIME_UP }
}

/// The first three of the table, in order, and the rest.
fn places(s: &Session) -> Vec<u32> {
    s.order.iter().copied().filter(|id| s.boards.contains_key(id)).collect()
}

/// The words that go under the steps and the rest of the screen's lines, as the scene shows them
/// `age` ticks after the podium opened.
pub(super) fn words(run: &Run, s: &Session, id: u32, age: u32) -> Vec<Word> {
    let order = places(s);
    let mut out = Vec::new();
    let cx = [192, 104, 280];
    for (i, &who) in order.iter().take(3).enumerate() {
        if age < SHOW[i] {
            continue;
        }
        let b = &s.boards[&who];
        let name = hud::called(run, who, id, 10);
        let line = match (i, b.finish_tick()) {
            (0, Some(t)) => hud::clock_text(t),
            _ => format!("{} CRACKS", b.cracks),
        };
        out.push(Word { x: cx[i] - text_width(&name) / 2, y: FLOOR + 2, text: name, ink: 'i' });
        out.push(Word { x: cx[i] - text_width(&line) / 2, y: FLOOR + 11, text: line, ink: if i == 0 { 't' } else { 'h' } });
    }
    // Fourth place on, small, down the left.
    if age >= 24 {
        for (k, &who) in order.iter().enumerate().skip(3) {
            let line = format!("{} {} {}", k + 1, hud::called(run, who, id, 6), s.boards[&who].cracks);
            out.push(Word { x: 4, y: 50 + (k as i32 - 3) * 10, text: line, ink: if who == id { 'T' } else { 'h' } });
        }
    }
    if age >= 30 {
        let mine = s.payout(id);
        let winner = order.first().copied().unwrap_or(id);
        let line = if !s.won {
            "NO PURSE".to_string()
        } else if mine > 0 {
            format!("+{}G", (mine as f32 * ease::out(((age - 30) as f32 / 30.0).min(1.0))).round() as u32)
        } else {
            format!("{} TAKES {}G", hud::called(run, winner, id, 10), s.payout(winner))
        };
        out.push(Word { x: 192 - text_width(&line) / 2, y: 195, text: line, ink: 'T' });
        let record = s.quickest();
        let quicker = s.won && s.to_beat > 0 && s.boards[&winner].finish_tick().is_some_and(|t| t < s.to_beat);
        if quicker {
            out.push(Word { x: 192 - text_width("NEW BEST") / 2, y: 204, text: "NEW BEST".into(), ink: 'O' });
        } else if record > 0 {
            let line = format!("BEST {}", hud::clock_text(record));
            out.push(Word { x: 192 - text_width(&line) / 2, y: 204, text: line, ink: 'G' });
        }
    }
    if age >= 40 {
        out.push(Word { x: 192 - text_width("E AGAIN") / 2, y: 214, text: "E AGAIN".into(), ink: 'u' });
    }
    out
}

/// One cheering crock, hopping on the spot.
fn cheer(im: &mut Img, x: i32, age: u32, phase: u32) {
    let small = &art().small;
    let hop = if (age + phase * 5) / 5 % 2 == 0 { 0 } else { 3 };
    im.stamp(small, x, 112 - hop);
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

/// The podium scene for seat `id`, `age` ticks after it opened.
pub(super) fn scene(run: &Run, s: &Session, id: u32, age: u32) -> Img {
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
    let winner = order.first().copied().unwrap_or(id);
    // Fireworks over the hall, three launchers in turn, when the kiln is broken.
    if s.won && age >= 6 {
        for (j, (x, top, ramp)) in [(70.0, 56.0, ramps::GOLD), (192.0, 34.0, ramps::EMBER), (314.0, 60.0, ramps::BONE)].into_iter().enumerate() {
            let t = age as i32 - 6 - j as i32 * 17;
            if t >= 0 {
                let round = (t / 52) as u32;
                juice::firework(&mut im, x + (round as f32 * 23.0) % 40.0 - 20.0, 112.0, top, (t % 52) as u32, 31 * j as u32 + round * 7, ramp, age as u64);
            }
        }
    }
    // The little crocks, real sprites, in the corners of the hall.
    if age >= 10 {
        for (n, x) in [4, 32, 322, 350].into_iter().enumerate() {
            cheer(&mut im, x, age, n as u32);
        }
    }
    let place: Vec<Place> = order
        .iter()
        .take(3)
        .map(|&who| Place { sprite: knight(run, who, false).unwrap_or_else(|| Img::new(8, 8)), name: String::new(), line: String::new() })
        .collect();
    let head = juice::podium(&mut im, STAGE, &place, age);
    if s.won && age >= 34 {
        laurel(&mut im, head, age - 34);
    }
    if age >= 14 && s.won {
        juice::confetti(&mut im, (0, 0, 384, 224), age, 9, 150);
    }
    // The call over it all.
    if age >= 4 {
        let (title, sub, tone) = if !s.won {
            ("THE KILN HOLDS".to_string(), "MOST CRACKS WIN".to_string(), Tone::Steel)
        } else if winner == id {
            ("YOU CRACKED IT".to_string(), String::new(), Tone::Warm)
        } else {
            ("KILN CRACKED".to_string(), format!("{} WINS", hud::called(run, winner, id, 14)), Tone::Warm)
        };
        let mut layer = Img::new(384, 46);
        juice::banner(&mut layer, 8, &title, &sub, age - 4, 4000, tone, 2);
        hud::claim("title", &layer, (0, 0));
        im.stamp(&layer, 0, 0);
    }
    for w in words(run, s, id, age) {
        im.text(w.x, w.y, &w.text, w.ink);
    }
    // The iris opening from the finale's black.
    juice::iris(&mut im, (192, 100), 1.0 - (age as f32 / WIPE as f32).min(1.0), 'k');
    im
}
