//! The moments over a seat's table, each a pure function of the session's
//! own ticks: the title slam and the 3-2-1-GO, the calls on the marquee, the
//! big moments on the field (the keeper's fall, the jackpot, the bell), the
//! scores that pop, the last five seconds, and the rims that warn and cheer.

use super::super::super::ink::{Img, bayer};
use super::super::juice;
use super::claims::{self, Big, Plan, TITLE};
use super::fx;
use crate::rules::mossbell::{self as rules, Beat, INTRO, LENGTH, Session};
use crate::rules::mossbell_show::Cry;

/// A dither of `ink` over the field's window, `k` (0 to 1) of its pixels.
fn dim(over: &mut Img, k: f32, ink: char) {
    let (x0, y0, w, h) = fx::WINDOW;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if bayer(x, y) < k {
                over.put(x, y, ink);
            }
        }
    }
}

/// The two plain words of the title, each with its picture: the paddle, the
/// nudge.
fn hints(over: &mut Img, t: u32) {
    if t < 8 {
        return;
    }
    let swing = (t / 5) % 2 == 0;
    // A paddle that flips: the rest and the raised, with the marble it throws.
    let (x, y) = (150, 138);
    let tip = if swing { y - 5 } else { y + 4 };
    for dy in -1..=1 {
        over.line(x - 14, y + 2 + dy, x + 12, tip + dy, 'o');
    }
    over.line(x - 14, y + 1, x + 12, tip - 1, 't');
    over.rect(x - 16, y, 4, 5, 'I');
    if swing {
        over.rect(x + 12, tip - 9, 4, 4, 'T');
        over.rect(x + 13, tip - 8, 2, 2, 'H');
    }
    // A nudge: the table's arrow kicks up.
    let (nx, ny) = (234, 138 - i32::from(!swing) * 2);
    over.rect(nx - 1, ny - 2, 3, 10, 't');
    for i in 0..4 {
        over.rect(nx - i, ny - 6 + i, 1 + 2 * i + 1, 1, 'T');
    }
}

/// Everything over the table for one seat.
pub(super) fn draw(over: &mut Img, run: &crate::rules::Run, s: &Session, me: u32, plan: &Plan, now: u32) {
    let tick = run.tick;
    let b = &s.tables[&me].ball;
    let age = LENGTH - s.left;
    // The title dims the table and clears for the countdown.
    if s.intro > 0 {
        let t = INTRO - s.intro;
        let k = if t < TITLE { 0.78 } else { 0.78 * (1.0 - (t - TITLE) as f32 / 10.0).max(0.0) };
        dim(over, k, 'k');
        if t < TITLE {
            hints(over, t);
            // The words under the pictures, set above the dim.
            for w in plan.words.iter().filter(|w| w.what == "hint") {
                over.text(w.x, w.y, &w.text, w.ink);
            }
        }
    }
    // A kill or the jackpot flashes the table's whole window for a tick or two.
    for bl in s.show.juice.blows.iter().filter(|bl| bl.owner == me && bl.weight >= crate::rules::juice::Weight::Kill) {
        let a = bl.age(now);
        if a < 3 {
            dim(over, 0.3 - a as f32 * 0.1, 'T');
        }
    }
    // The bell's rim, its toll and the last seconds' red.
    if s.intro == 0 {
        let ringing = rules::bell(age);
        if ringing {
            let pulse = 0.5 + 0.5 * crate::motion::math::sin(now as f32 * 0.25);
            fx::rim(over, 0.35 + 0.2 * pulse, 9, 'o');
        }
        for sh in s.show.shouts.iter().filter(|sh| sh.cry == Cry::Bell) {
            let a = sh.age(now);
            if a < 14 {
                fx::rim(over, 0.9 * (1.0 - a as f32 / 14.0), 16, 'T');
            }
        }
        if s.left <= 10 * crate::rules::HZ && s.left > 0 {
            let pulse = 0.5 + 0.5 * crate::motion::math::sin(now as f32 * 0.4);
            fx::rim(over, 0.1 + 0.15 * pulse, 7, 'R');
        }
        // A tell the keeper winds up: the rim warms toward the strike.
        if matches!(b.beat, Beat::RootTell | Beat::CrownTell) && b.hp > 0 {
            let total = if b.hp > 2 { 36.0 } else { 30.0 };
            let k = 1.0 - b.beat_left as f32 / total;
            if k > 0.5 {
                let pulse = 0.5 + 0.5 * crate::motion::math::sin(now as f32 * 0.7);
                fx::rim(over, (k - 0.5) * 0.5 * (0.6 + 0.4 * pulse), 8, 'p');
            }
        }
        // The drain's sting.
        if b.save >= 2 && b.drains > 0 {
            fx::rim(over, 0.85 * f32::from(b.save) / 12.0, 14, 'R');
        }
        // The jackpot's own coins, fireworks and gold rim, for the one who took it.
        for sh in s.show.shouts.iter().filter(|sh| sh.cry == Cry::Jackpot && u32::from(sh.who) == me) {
            let a = sh.age(now);
            fx::jackpot(over, a, tick);
            if a < 30 {
                fx::rim(over, 0.7 * (1.0 - a as f32 / 30.0), 18, 'T');
            }
        }
    }
    // The words the plan holds, as the kit draws them.
    for (_, big) in &plan.bigs {
        match big {
            Big::Banner { y, title, sub, age, life, tone, cap } => juice::banner(over, *y, title, sub, *age, *life, *tone, *cap),
            Big::Count { label, age, life, go } => juice::countdown(over, claims::MIDDLE, label, *age, *life, *go),
            Big::Digit { n, age } => {
                let scale = 4 + (3 - *age as i32).max(0);
                let mut digit = juice::big_text_img(&n.to_string(), scale, if *age < 3 { 'c' } else { '@' }, 'p');
                juice::dissolve(&mut digit, ((*age as f32 - 20.0) / 10.0).clamp(0.0, 1.0));
                over.stamp(&digit, claims::MIDDLE.0 - digit.w / 2, 72 - (scale - 4) * 3);
            }
            Big::Text { text, x, y, scale, ink } => over.stamp(&juice::big_text_img(text, *scale, *ink, 'k'), *x, *y),
            Big::Pop { text, at, age, ink } => juice::pop_number(over, text, *at, *age, crate::rules::mossbell_show::POP_LIFE, 1, *ink),
        }
    }
}
