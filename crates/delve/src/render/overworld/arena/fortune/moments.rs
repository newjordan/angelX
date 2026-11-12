//! The spin's moments over the hall: the pull's call, the tease ("SO
//! CLOSE"), the landing's slam and reveal, the cheer that fits the prize (a
//! sigh, paper, fireworks), the knights' own faces, and the sign that stays.
//! Every box of words goes through `lay`, which keeps its place for the test
//! that no two meet and none leaves the screen.

use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone};
use super::super::{NATIVE_H, NATIVE_W, at};
use super::show::Show;
use super::{claims, hub};
use crate::motion::ease;
use crate::rules::fortune::{Beat, HANG, Luck, RUN, WIND};
use crate::rules::Run;

/// Ticks the reveal's banner stays up after the wheel lands.
pub(super) const CALL_LIFE: u32 = 112;
/// Ticks the pull's call stays up.
const PULL_LIFE: u32 = 34;
/// Ticks the tease's call stays up.
const TEASE_LIFE: u32 = 26;
/// Where the calls sit: the middle of the wheel for the reveal, the floor
/// before the lever for the others.
const REVEAL_TOP: i32 = 108;
const CALL_TOP: i32 = 104;

/// How the seat `me` calls a knight: `YOU` for themselves and only
/// themselves; any other knight by their own name (`KNIGHT n` for one who
/// still carries the default `You`, or none).
pub(super) fn called(run: &Run, id: u32, me: Option<u32>, take: usize) -> String {
    if Some(id) == me {
        return "YOU".into();
    }
    let full = run.players.get(&id).map_or_else(String::new, |h| h.name.trim().to_uppercase());
    if full.is_empty() || full == "YOU" {
        return format!("KNIGHT {id}");
    }
    full.chars().take(take).collect()
}

fn tone(luck: Luck) -> Tone {
    match luck {
        Luck::Grand => Tone::Hot,
        Luck::Fair => Tone::Warm,
        Luck::Plain => Tone::Steel,
    }
}

/// Stamp `im` at `(x, y)` and keep its box of words.
pub(super) fn lay(cv: &mut Img, what: &str, im: &Img, x: i32, y: i32) {
    cv.stamp(im, x, y);
    claims::note_img(what, im, x, y);
}

/// Whether the spin's calls are up: while it turns and for the banner's life
/// after, the plates' boards keep quiet so no two lines of words meet.
pub(super) fn calling(run: &Run) -> bool {
    run.spin.is_some_and(|s| !s.done(run.tick) || run.tick.saturating_sub(s.started) < u64::from(crate::rules::fortune::SPIN_TICKS + CALL_LIFE + 6) && !s.settled)
}

/// What a banner draws as its own picture, to be laid at `top`.
fn banner(title: &str, sub: &str, age: u32, life: u32, tone: Tone, cap: i32) -> Img {
    let mut tmp = Img::new(NATIVE_W, 60);
    juice::banner(&mut tmp, 14, title, sub, age, life, tone, cap);
    tmp
}

/// The calls and the cheer, over the hall.
pub(super) fn over(cv: &mut Img, run: &Run, s: &Show, focus: Option<u32>) {
    let clock = s.clock(run);
    let rect = (0, 0, NATIVE_W, NATIVE_H - 32);
    // The tease: the room's edges close in on the wheel.
    if s.beat == Beat::Hang || (s.beat == Beat::Drop && s.n < WIND + RUN + HANG + 3) {
        let k = ease::out((s.n.saturating_sub(WIND + RUN)) as f32 / 10.0) * 0.5;
        juice::vignette(cv, rect, k, 34, 'k');
    }
    if let Some(l) = s.since {
        cheer(cv, run, s, l, clock);
    }
    // The pull: SPIN, and who pulled.
    if s.since.is_none() && s.n < PULL_LIFE {
        let who = called(run, s.spin.by, focus, 9);
        let sub = if who == "YOU" { "YOU PULL".to_string() } else { format!("{who} PULLS") };
        let im = banner("SPIN", &sub, s.n, PULL_LIFE, Tone::Warm, 2);
        lay(cv, "pull", &im, 0, CALL_TOP - 14);
    }
    // The tease: so close, the neighbour's wedge under the pointer.
    let hang_at = WIND + RUN;
    if s.since.is_none() && (hang_at + 2..hang_at + 2 + TEASE_LIFE).contains(&s.n) {
        let im = banner("SO CLOSE", "", s.n - hang_at - 2, TEASE_LIFE, Tone::Steel, 2);
        lay(cv, "tease", &im, 0, CALL_TOP - 14);
    }
    // The reveal.
    if let Some(l) = s.since.filter(|&l| l < CALL_LIFE) {
        reveal(cv, s, l);
    } else if s.since.is_some() {
        sign(cv, s);
    }
}

/// The landing's banner over the wheel, the wedge named, what it is in a few
/// plain words, and what it pays.
fn reveal(cv: &mut Img, s: &Show, l: u32) {
    let (hx, _) = hub();
    let _ = hx;
    // A beat after the slam, so the freeze reads before the words do.
    let age = l.saturating_sub(2);
    if l < 2 {
        return;
    }
    let mode = s.mode;
    let im = banner(mode.headline(), mode.gist(), age, CALL_LIFE - 2, tone(s.luck), 3);
    lay(cv, "reveal", &im, 0, REVEAL_TOP - 14);
    if let Some(pay) = mode.pay() {
        let fade = ((l as f32 - (CALL_LIFE - 16) as f32) / 14.0).clamp(0.0, 1.0);
        if l >= 8 && fade < 1.0 {
            let mut line = juice::big_text_img(pay, 1, '6', 'k');
            juice::dissolve(&mut line, fade);
            let pop = ease::back((l as f32 - 8.0) / 8.0);
            let y = REVEAL_TOP + 49 + ((1.0 - pop) * -6.0) as i32;
            lay(cv, "pay", &line, (NATIVE_W - line.w) / 2, y);
        }
    }
}

/// The sign that stays once the banner has gone: the wedge's name in a
/// little plate hung on the valance.
pub(super) fn sign(cv: &mut Img, s: &Show) {
    let text = s.mode.headline();
    let w = text_width(text) + 12;
    let mut im = Img::new(w + 1, 12);
    super::super::super::chrome::panel(&mut im, (0, 0, w, 11), super::super::super::chrome::Tone::SIGN);
    im.text(6, 2, text, '6');
    lay(cv, "sign", &im, (NATIVE_W - w) / 2, 1);
}

/// The room's cheer for what the wheel paid.
fn cheer(cv: &mut Img, run: &Run, s: &Show, l: u32, clock: u32) {
    let sky = (0, 0, NATIVE_W, NATIVE_H - 40);
    let tick = clock as u64;
    match s.luck {
        Luck::Plain => {
            // A soft sigh: a few motes drift up off the wheel.
            if l < 60 {
                let (hx, hy) = hub();
                let mut pen = super::super::pfx::Pen::new(cv, 120);
                let spray = super::super::pfx::Spray { gravity: -0.01, drag: 0.05, angle: -std::f32::consts::FRAC_PI_2, spread: 2.0, ..super::super::pfx::Spray::round(0x9a1, 12, (hx as f32, hy as f32 - 20.0), (0.4, 1.4), (24.0, 50.0)) };
                spray.each(l as f32, |m| super::super::pfx::mote(&mut pen, &m, super::super::pfx::Shape::Dot, super::super::pfx::ramps::SMOKE, 1, tick));
            }
        }
        Luck::Fair => {
            if (4..110).contains(&l) {
                juice::confetti(cv, sky, clock / 4, 0x3f1, 84);
            }
            for (i, (x, top, from)) in [(70, 36, 6u32), (322, 30, 14), (112, 18, 24)].into_iter().enumerate() {
                if l >= from {
                    juice::firework(cv, x as f32, 150.0, top as f32, l - from, 0x51 + i as u32, super::super::pfx::ramps::GOLD, tick);
                }
            }
        }
        Luck::Grand => {
            if (3..150).contains(&l) {
                juice::confetti(cv, sky, clock / 4, 0x7d3, 150);
            }
            let shells = [(64, 34, 4u32), (318, 28, 10), (110, 14, 18), (276, 16, 26), (36, 70, 38), (352, 62, 46)];
            for (i, (x, top, from)) in shells.into_iter().enumerate() {
                if l >= from {
                    let ramp = if i % 2 == 0 { super::super::pfx::ramps::FIRE } else { super::super::pfx::ramps::EMBER };
                    juice::firework(cv, x as f32, 156.0, top as f32, l - from, 0x77 + i as u32, ramp, tick);
                }
            }
            // The drapes' braziers flare: flame jets up either side of the wheel.
            if l < 70 {
                let (hx, hy) = hub();
                let mut pen = super::super::pfx::Pen::new(cv, 300);
                for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                    let base = (hx as f32 + side * 58.0, hy as f32 + 36.0);
                    let jet = super::super::pfx::Spray { gravity: -0.05, drag: 0.04, angle: -std::f32::consts::FRAC_PI_2, spread: 0.7, ..super::super::pfx::Spray::round(0x66 + k as u32, 16, base, (1.4, 3.2), (14.0, 28.0)) };
                    jet.each(l as f32, |m| super::super::pfx::mote(&mut pen, &m, super::super::pfx::Shape::Ember, super::super::pfx::ramps::FIRE, 2, tick));
                }
            }
        }
    }
    let _ = run;
}

const CROWN: [&str; 4] = ["6.6.6.6", "6666666", "5545455", "ooooooo"];
const STAR: [&str; 5] = ["..6..", "..6..", "66666", "..6..", "..6.."];
const DROP: [&str; 4] = ["..V.", ".VV.", ".VVV", "..V."];

/// Each standing knight's face for the prize, over their head: a crown for a
/// grand wedge, a spark for a fair one, a drop of sweat for the plain road.
pub(super) fn faces(cv: &mut Img, run: &Run, s: &Show) {
    let Some(l) = s.since else { return };
    if !(8..110).contains(&l) {
        return;
    }
    let rows: &[&str] = match s.luck {
        Luck::Grand => &CROWN,
        Luck::Fair => &STAR,
        Luck::Plain => &DROP,
    };
    let sprite = Img::from_rows(rows);
    let rise = (ease::out((l as f32 - 8.0) / 10.0) * 6.0) as i32;
    let fade = ((l as f32 - 92.0) / 18.0).clamp(0.0, 1.0);
    for hero in run.players.values().filter(|h| h.hp > 0 && !h.stone) {
        let (x, y) = at(hero.x, hero.y);
        let bob = i32::from((l / 5) % 2 == 0);
        let mut mark = sprite.clone();
        juice::dissolve(&mut mark, fade);
        cv.stamp(&mark, x - mark.w / 2, y - 31 - rise - bob);
    }
}
