//! The tourney's end: the lists hold a moment on the fallen knight and the
//! crowd on its feet, then the iris closes and the stands show again at
//! night, the crowd on its feet in the torchlight, fireworks over the
//! awning, paper falling, the winner crowned on the tallest step and the
//! other on his own, the purse counting up, each wager paid, and the
//! realm's record of champions beaten.
//!
//! All of it from the verdict's own ticks (`Joust::t` in `Stage::Done`), so
//! every seat sees it alike.

use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Place, Tone as Mood};
use super::super::knights;
use super::super::pfx::ramps;
use super::frame::Cx;
use super::hud::Ledger;
use super::{crowd, stage};
use crate::motion::ease;
use crate::rules::joust::{self, RIVALS, Stage};

/// Ticks the lists hold on the aftermath before the iris, and the iris's own.
pub(super) const HOLD: u32 = 26;
pub(super) const FADE: u32 = 14;
/// Where the podium stands on the canvas.
const BOX: (i32, i32, i32, i32) = (0, 80, 384, 114);

/// Ticks into the podium, once the iris has closed.
pub(super) fn age(cx: &Cx) -> Option<u32> {
    (cx.j.stage == Stage::Done && cx.j.t >= HOLD + FADE).then(|| cx.j.t - HOLD - FADE)
}

fn standing(cx: &Cx, side: u8) -> Img {
    let j = cx.j;
    match j.rider(side).and_then(|id| cx.run.players.get(&id).map(|h| (id, h))) {
        Some((id, hero)) => {
            let mut hero = hero.clone();
            (hero.aim_x, hero.aim_y) = (0.0, 1.0);
            knights::figure(&hero, id)
        }
        None => {
            let (tab, field) = RIVALS[usize::from(j.rival).min(RIVALS.len() - 1)].colours;
            knights::of_kit(None, &[('7', tab), ('5', field)])
        }
    }
}

fn cxm_of() -> i32 {
    BOX.0 + BOX.2 / 2
}

/// A beam of torchlight down onto the winner: a dither, behind the steps.
fn spot(im: &mut Img, cx: i32, floor: i32, k: f32) {
    for y in 36..floor {
        let half = 20.0 + y as f32 * 0.42;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if super::super::super::ink::bayer(x, y) < edge * edge * 0.26 * k {
                im.put(x, y, 'J');
            }
        }
    }
}

/// The podium scene for a seat, `e` ticks in.
pub(super) fn scene(im: &mut Img, cx: &Cx, led: &mut Ledger, e: u32) {
    let j = cx.j;
    let tick = cx.tick;
    *im = stage::backdrop().clone();
    stage::live(im, tick);
    // The sand goes dark under the steps: night fallen on the lists.
    for y in 100..224 {
        for x in 0..384 {
            if super::super::super::ink::bayer(x, y) < 0.42 {
                im.put(x, y, 'k');
            }
        }
    }
    crowd::draw(im, tick, 1.0, 192);
    // Fireworks over the stands, three launchers in turn.
    if e >= 6 {
        for (n, (x, top, ramp)) in [(70.0, 14.0, ramps::GOLD), (192.0, 8.0, ramps::EMBER), (314.0, 16.0, ramps::BONE)].into_iter().enumerate() {
            let t = e as i32 - 6 - n as i32 * 15;
            if t >= 0 {
                let round = (t / 56) as u32;
                juice::firework(im, x + (round as f32 * 23.0) % 36.0 - 18.0, 60.0, top + (round % 2) as f32 * 8.0, (t % 56) as u32, 29 * n as u32 + round * 7, ramp, tick as u64);
            }
        }
    }
    let winner = j.winner();
    let (first, second) = match winner {
        Some(w) => (w, 1 - w),
        None => (0, 1),
    };
    let ends = |s: u8| if s == 0 { j.fallen.0 } else { j.fallen.1 };
    let passes = |s: u8| if s == 0 { j.passes.0 } else { j.passes.1 };
    let line_of = |s: u8, won: bool| -> String {
        if winner.is_none() {
            "A DRAW".to_string()
        } else if ends(s) {
            "ON THE SAND".to_string()
        } else if won && j.fallen.0 != j.fallen.1 && (j.fallen.0 || j.fallen.1) {
            "UNHORSED".to_string()
        } else {
            format!("{} PASSES", passes(s))
        }
    };
    let places = vec![
        Place { sprite: standing(cx, first), name: cx.name(first, 10), line: line_of(first, true) },
        Place { sprite: standing(cx, second), name: cx.name(second, 10), line: line_of(second, false) },
    ];
    spot(im, cxm_of(), BOX.1 + BOX.3 - 6, ((e as f32 - 6.0) / 12.0).clamp(0.0, 1.0));
    let head = juice::podium(im, BOX, &places, e);
    // The names and lines under the steps are the kit's: claim their boxes.
    let floor = BOX.1 + BOX.3 - 6;
    let cxm = BOX.0 + BOX.2 / 2;
    for (i, p) in places.iter().enumerate() {
        let x = [cxm, cxm - 88][i];
        let start = [16, 8][i];
        if e >= start + 18 {
            let (nw, lw) = (text_width(&p.name), text_width(&p.line));
            led.claim("podium-name", (x - nw / 2, floor + 4, nw, 7), &p.name);
            led.claim("podium-line", (x - lw / 2, floor + 13, lw, 7), &p.line);
        }
    }
    // The winner's laurel, once they have stood a moment.
    if winner.is_some() && e >= 34 {
        let k = (((e - 34) as f32) / 10.0).min(1.0);
        let bob = ((e as f32 * 0.2).sin() * 1.5) as i32;
        let (x, y) = (head.0, head.1 - 6 + ((1.0 - k) * -20.0) as i32 + bob);
        for a in 0..11 {
            let t = a as f32 / 10.0;
            let (dx, dy) = ((t * 2.0 - 1.0) * 18.0, -(1.0 - (t * 2.0 - 1.0).powi(2)) * 8.0 + 5.0);
            im.rect(x + dx as i32 - 2, y + dy as i32, 5, 3, if a % 2 == 0 { 't' } else { 'O' });
            im.rect(x + dx as i32 - 1, y + dy as i32, 2, 1, 'T');
        }
        im.rect(x - 2, y - 9, 5, 5, '@');
    }
    super::moments::herald(im, e < 70, tick);
    if e >= 14 {
        juice::confetti(im, (0, 0, 384, 224), e, 9, 120);
    }
    // The title.
    let me = cx.side();
    let (a, b) = (j.passes.0, j.passes.1);
    let how = |w: u8| -> String {
        if ends(1 - w) {
            "UNHORSED".to_string()
        } else if passes(w) > passes(1 - w) {
            format!("{} PASSES TO {}", passes(w), passes(1 - w))
        } else {
            "ON POINTS".to_string()
        }
    };
    let _ = (a, b);
    let (title, sub) = match winner {
        Some(w) if me == Some(w) => ("YOU WIN".to_string(), how(w)),
        Some(w) => (format!("{} WINS", cx.name(w, 10)), how(w)),
        None => ("A DRAW".to_string(), "STAKES RETURNED".to_string()),
    };
    if e >= 4 {
        led.banner(im, "verdict", 14, &title, &sub, e - 4, 400, Mood::Warm, 3);
    }
    // The purse, counting up, and the wagers paid.
    if e >= 22 {
        let (x, y) = (312, 104);
        if winner.is_some() && j.purse > 0 {
            let n = ((joust::PURSE - joust::ENTRY) as f32 * ease::out(((e - 22) as f32 / 40.0).min(1.0))) as u32;
            led.big(im, "purse", x, y, &format!("+{n}G"), 3, 'T', 'B');
            led.text(im, "purse-line", (x - text_width("THE PURSE") / 2, y + 30), "THE PURSE", 'h');
        } else if winner.is_some() {
            led.big(im, "purse", x, y, "LAUREL", 2, 'T', 'B');
            led.text(im, "purse-line", (x - text_width("FOR HONOUR") / 2, y + 22), "FOR HONOUR", 'h');
        }
        let mut row = y + 42;
        for (&id, w) in &j.gallery {
            let Some(won) = w.won else { continue };
            let name = super::hud::called(cx.run, id, cx.me, 9);
            let line = if won { format!("{name} +{}G", w.stake) } else { format!("{name} -{}G", w.stake) };
            let x0 = 312 - text_width(&line) / 2;
            led.text(im, "wager", (x0, row), &line, if won { '5' } else { 'h' });
            row += 10;
        }
    }
    // The realm's record of champions.
    if e >= 30 && j.foe.is_none() {
        let beaten = RIVALS
            .iter()
            .filter(|r| cx.run.home.stable.wins.get(r.id).copied().unwrap_or(0) > 0 || (j.winner() == Some(0) && r.id == j.rival().id))
            .count();
        let line = format!("CHAMPIONS BEATEN {beaten} OF {}", RIVALS.len());
        led.text(im, "record", (cxm - text_width(&line) / 2, 214), &line, 'o');
    }
    // The way home: a thin bar along the foot.
    let total = joust::VERDICT_TICKS - HOLD - FADE;
    let filled = (384 * e.min(total) / total.max(1)) as i32;
    for x in 0..filled {
        im.put(x, 223, 'o');
    }
    // The iris opening from the aftermath's black.
    juice::iris(im, (cxm, 120), 1.0 - ease::out((e as f32 / 14.0).min(1.0)), 'k');
}
