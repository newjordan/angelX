//! The end of the round: the table stands as it was while TIME is called, the
//! iris shuts, and the conservatory comes up with the podium: three steps
//! rising, the winner laureled, fireworks over the glass, paper falling, the
//! purse counting up. All of it from the session's `ended` tick, so every
//! seat sees it alike, and it plays on until a knight asks to go again.

use super::super::super::ink::{Img, bayer};
use super::super::juice::{self, Place};
use super::super::pfx::ramps;
use super::super::{NATIVE_H, NATIVE_W, knights};
use super::claims::{self, Big, LIVE, Plan};
use super::fx;
use crate::motion::ease;
use crate::rules::Run;
use crate::rules::mossbell::Session;
use std::sync::OnceLock;

/// Ticks the iris takes to open on the podium.
const OPEN: u32 = 14;

/// The conservatory with its panels taken away: dim glass, vines and lamps.
fn backdrop() -> &'static Img {
    static ORIGINAL: OnceLock<Img> = OnceLock::new();
    static ART: OnceLock<Img> = OnceLock::new();
    let cache = if super::super::slot_art::enabled() { &ART } else { &ORIGINAL };
    cache.get_or_init(|| {
        if let Some(art) = super::slot("mossbell.room", "room") {
            return super::dim(art);
        }
        let mut im = Img::black(NATIVE_W, NATIVE_H);
        for row in 0..5 {
            for col in 0..10 {
                let x = 8 + col * 38 + (row % 2) * 8;
                let y = 31 + row * 27;
                im.rect(x, y, 34, 24, 'k');
                im.line(x + 2, y + 23, x + 33, y + 23, 'x');
            }
        }
        im
    })
}

/// A spot of light from above onto the winner's step, a dither.
fn spot(im: &mut Img, cx: i32, floor: i32, k: f32) {
    for y in 40..floor {
        let half = 26.0 + (y - 40) as f32 * 0.5;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if bayer(x, y) < edge * edge * 0.3 * k {
                im.put(x, y, 'J');
            }
        }
    }
}

/// The podium, `a` ticks after the iris began to open, for the seat `me`.
pub(super) fn scene(run: &Run, s: &Session, me: u32, a: u32, plan: &Plan) -> Img {
    let tick = run.tick;
    let mut im = backdrop().clone();
    let (x0, y0, w, h) = claims::PODIUM;
    let ranks = claims::places(s);
    let n = ranks.len();
    // The glass at the back, lit by fireworks.
    if a >= 6 {
        for (j, (fx_x, top, ramp)) in [(70.0, 36.0, ramps::GOLD), (192.0, 26.0, ramps::EMBER), (314.0, 40.0, ramps::HOLY)].into_iter().enumerate() {
            let t = a as i32 - 6 - j as i32 * 15;
            if t >= 0 {
                let round = (t / 50) as u32;
                juice::firework(&mut im, fx_x + (round as f32 * 17.0) % 30.0 - 15.0, 112.0, top, (t % 50) as u32, 29 * j as u32 + round * 5, ramp, tick);
            }
        }
    }
    spot(&mut im, x0 + w / 2, claims::FLOOR, ((a as f32 - 6.0) / 12.0).clamp(0.0, 1.0));
    let places: Vec<Place> = ranks
        .iter()
        .take(3)
        .filter_map(|&id| {
            let hero = run.players.get(&id)?;
            let c = plan.extra.iter().find(|c| c.what == "podium name" && c.who == Some(id));
            let line = plan.extra.iter().find(|c| c.what == "podium line" && c.rect.0 + c.rect.2 / 2 == c.rect.0 + c.rect.2 / 2);
            let _ = line;
            Some(Place {
                sprite: knights::figure(hero, id),
                name: c.map_or_else(|| claims::called(run, id, me, 9), |c| c.text.clone()),
                line: format!("{} PTS", s.points(id).min(99999)),
            })
        })
        .collect();
    // The kit sets names and lines when a step has risen; the plan holds the same words.
    let head = juice::podium(&mut im, (x0, y0, w, h), &places, a);
    // The laurel over the winner once they have stood a moment.
    if a >= 44 && n > 1 {
        let k = (((a - 44) as f32) / 10.0).min(1.0);
        let bob = (crate::motion::math::sin(a as f32 * 0.2) * 1.5) as i32;
        let (x, y) = (head.0, head.1 - 8 + ((1.0 - k) * -20.0) as i32 + bob);
        for i in 0..11 {
            let t = i as f32 / 10.0;
            let (dx, dy) = ((t * 2.0 - 1.0) * 22.0, -(1.0 - (t * 2.0 - 1.0).powi(2)) * 9.0 + 6.0);
            im.rect(x + dx as i32 - 2, y + dy as i32, 5, 3, if i % 2 == 0 { 't' } else { 'O' });
            im.rect(x + dx as i32 - 1, y + dy as i32, 2, 1, 'T');
        }
    }
    if a >= 14 {
        juice::confetti(&mut im, (0, 0, NATIVE_W, NATIVE_H), a, 9, 120);
    }
    // The words the plan holds.
    for w in &plan.words {
        im.text(w.x, w.y, &w.text, w.ink);
    }
    for (_, big) in &plan.bigs {
        match big {
            Big::Banner { y, title, sub, age, life, tone, cap } => juice::banner(&mut im, *y, title, sub, *age, *life, *tone, *cap),
            Big::Text { text, x, y, scale, ink } => im.stamp(&juice::big_text_img(text, *scale, *ink, 'k'), *x, *y),
            _ => {}
        }
    }
    // The iris opening from the live finish's black.
    juice::iris(&mut im, (NATIVE_W / 2, 110), 1.0 - (a as f32 / OPEN as f32).clamp(0.0, 1.0), 'k');
    im
}

/// The live finish: the table as it stood, TIME called, the iris closing on it.
pub(super) fn close(over: &mut Img, e: u32) {
    let k = ((e as f32 - (LIVE - 14) as f32) / 14.0).clamp(0.0, 1.0);
    if e < 6 {
        let k = (1.0 - e as f32 / 6.0) * 0.1;
        for y in 0..NATIVE_H {
            for x in 0..NATIVE_W {
                if bayer(x, y) < k {
                    over.put(x, y, 'i');
                }
            }
        }
    }
    juice::iris(over, (NATIVE_W / 2, 110), ease::in_quad(k), 'k');
    let _ = fx::WINDOW;
}
