//! The wheel itself, drawn as a thing in motion: it winds back and swells,
//! blurs as it turns (each pixel takes the wedge a little behind it by the
//! ordered dither, so the streak is made of the wedges' own inks), its lamps
//! chase round the rim, the pointer is flicked by every peg and slows with
//! them, it hangs on the wedge beside its own, and then it lands: the
//! wedge flares, the rest dim.

use std::f32::consts::{FRAC_PI_2, TAU};

use super::super::super::ink::{Img, bayer};
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::show::{self, Show};
use super::{hub, icon, opened, wedge_ink};
use crate::motion::{self, Spring, ease};
use crate::rules::Run;
use crate::rules::fortune::{Beat, Luck, Mode};

/// The wheel's own radius at rest.
pub(super) const R0: f32 = 40.0;
const LAMPS: i32 = 28;

/// How far the wheel has swelled (as a share of its radius): it crouches as
/// it winds back, looms while it runs, and slams back on its landing.
pub(super) fn swell(s: Option<&Show>) -> f32 {
    let Some(s) = s else { return 0.0 };
    if let Some(since) = s.since {
        return Spring::WOBBLY.at(since as f32, 0.16, 0.0, 0.0).clamp(-0.06, 0.16);
    }
    match s.beat {
        Beat::Wind => -0.05 * ease::both(s.n as f32 / 8.0),
        Beat::Run => {
            let up = ease::out((s.n.saturating_sub(crate::rules::fortune::WIND)) as f32 / 14.0);
            -0.05 + 0.15 * up
        }
        Beat::Hang => 0.10,
        _ => 0.10,
    }
}

/// How much of the wheel outside the winning wedge is dimmed behind the
/// reveal (0 to 0.55), for the dither.
pub(super) fn veil(s: &Show) -> f32 {
    let Some(l) = s.since else { return 0.0 };
    let up = ease::out((l as f32 - 6.0) / 10.0);
    let down = 1.0 - ease::both((l as f32 - super::moments::CALL_LIFE as f32) / 24.0);
    0.55 * up * down
}

/// The wheel and its pointer, hub at `hub()`.
pub(super) fn draw(cv: &mut Img, run: &Run, s: Option<&Show>) {
    let (hx, hy) = hub();
    let w = TAU / Mode::WHEEL.len() as f32;
    let open = opened(run);
    let tick = run.tick as u32;
    let rot = s.map_or(0.0, |s| s.rot);
    let speed = s.map_or(0.0, |s| s.speed.abs());
    let r_out = (R0 * (1.0 + swell(s))).max(30.0);
    let landed = s.and_then(|s| s.since.map(|_| s.spin.wedge));
    let since = s.and_then(|s| s.since);
    let lit_neighbour = s.filter(|s| matches!(s.beat, Beat::Hang)).map(|s| s.spin.neighbour());
    let dim = s.map_or(0.0, veil);
    let clock = s.map_or(tick * 4, |s| s.clock(run));
    // At speed the wedges smear along their path; the dividers go.
    let smear = (speed.min(2.0)) * w;
    let blink = (tick / 6).is_multiple_of(2);
    let reach = r_out.ceil() as i32 + 2;
    for y in hy - reach..=hy + reach {
        for x in hx - reach..=hx + reach {
            let (dx, dy) = (x as f32 + 0.5 - hx as f32, y as f32 + 0.5 - hy as f32);
            let r = dx.hypot(dy);
            if r > r_out {
                continue;
            }
            if r > r_out - 3.5 {
                // The frame: gold, a light edge and a dark underside.
                let ink = if r > r_out - 1.0 { if dy < 0.0 { 'T' } else { 'o' } } else if r > r_out - 2.4 { 'O' } else { 'r' };
                cv.put(x, y, ink);
                continue;
            }
            if r < 5.0 {
                cv.put(x, y, if r < 2.5 { '6' } else { '4' });
                continue;
            }
            // The wedge under this pixel, smeared back along the turn.
            let base = (dy.atan2(dx) + FRAC_PI_2 - rot).rem_euclid(TAU);
            let a = if smear > 0.02 { (base + smear * (bayer(x, y) - 0.5)).rem_euclid(TAU) } else { base };
            let index = ((a / w) as usize).min(Mode::WHEEL.len() - 1);
            let fract = (a / w).fract();
            let edge = smear < 0.12 && (fract < 0.04 || fract > 0.96);
            let is_win = landed == Some(index as u8);
            let lit = is_win && since.is_some_and(|l| l < 150);
            let ink = if lit && (edge || fract < 0.07 || fract > 0.93) && r > 7.0 {
                if blink { 'T' } else { '6' }
            } else if lit && r > 7.0 && open.contains(&(index as u8)) {
                // The wedge in the light: its bright ink, a band of gold chasing out along it.
                let (dark, light) = wedge_ink(Mode::WHEEL[index]);
                let band = ((r - 7.0) * 0.35 - (clock / 3) as f32).rem_euclid(4.0) < 1.0;
                if band { '6' } else if bayer(x, y) < 0.3 { 'T' } else if r < r_out * 0.55 { light } else { dark }
            } else if r < 7.0 || edge {
                'b'
            } else if !open.contains(&(index as u8)) {
                if (x + y) % 2 == 0 { 'K' } else { 'k' }
            } else if is_win && since.is_some_and(|l| l < 3) {
                'H'
            } else if is_win && r > r_out - 8.0 && blink {
                '6'
            } else if lit_neighbour == Some(index as u8) && ((clock / 3) % 2 == 0) && r > r_out - 9.0 {
                'T'
            } else {
                let (dark, light) = wedge_ink(Mode::WHEEL[index]);
                let mut ink = if r < r_out * 0.55 { dark } else { light };
                // Ordered-dither bands on a fast wheel: it reads as turning.
                if smear > 0.3 && bayer(x + (clock as i32) / 2, y) < 0.18 {
                    ink = if r < r_out * 0.55 { light } else { dark };
                }
                ink
            };
            // The dimmed wedges behind the reveal: black paper by the dither.
            let ink = if dim > 0.0 && !is_win && bayer(x, y) < dim { 'k' } else { ink };
            cv.put(x, y, ink);
        }
    }
    // Pegs, one on every wedge's edge, just inside the frame.
    if smear < 0.5 {
        for i in 0..Mode::WHEEL.len() {
            let ang = i as f32 * w + rot - FRAC_PI_2;
            let (px, py) = (hx as f32 + ang.cos() * (r_out - 6.0), hy as f32 + ang.sin() * (r_out - 6.0));
            cv.rect(px.round() as i32 - 1, py.round() as i32 - 1, 2, 2, 'T');
        }
    }
    // Each wedge's mark, thinning with speed.
    let keep = (1.0 - (speed - 0.25) / 0.6).clamp(0.0, 1.0);
    if keep > 0.0 {
        for (i, mode) in Mode::WHEEL.iter().enumerate() {
            let mid = (i as f32 + 0.5) * w + rot - FRAC_PI_2;
            let (mx, my) = (
                hx + (mid.cos() * r_out * 0.62).round() as i32,
                hy + (mid.sin() * r_out * 0.62).round() as i32,
            );
            let dimmed = dim > 0.0 && landed != Some(i as u8);
            if open.contains(&(i as u8)) {
                let im = icon(*mode);
                stamp_thin(cv, &im, mx - im.w / 2, my - im.h / 2, keep, if dimmed { dim } else { 0.0 });
            } else {
                cv.text(mx - 2, my - 3, "?", 'G');
            }
        }
    }
    lamps(cv, run, s, (hx, hy), r_out, clock);
    pointer(cv, s, (hx, hy), r_out, clock);
    if let (Some(l), Some(s)) = (since, s) {
        landing(cv, s, (hx, hy), r_out, l, clock);
    }
}

/// A sprite stamped through the ordered dither: `keep` of its pixels, and `dim` of those
/// taken away again.
fn stamp_thin(cv: &mut Img, im: &Img, x: i32, y: i32, keep: f32, dim: f32) {
    for yy in 0..im.h {
        for xx in 0..im.w {
            let Some(c) = im.get(xx, yy) else { continue };
            let b = bayer(x + xx, y + yy);
            if b < keep && b >= dim {
                cv.set(x + xx, y + yy, c);
            }
        }
    }
}

/// The lamps on the frame: still bulbs twinkling, a chase while it turns,
/// the ones at the pointer fluttering as it hangs, all of them flashing
/// when it lands.
fn lamps(cv: &mut Img, run: &Run, s: Option<&Show>, (hx, hy): (i32, i32), r_out: f32, clock: u32) {
    let tick = run.tick as u32;
    for k in 0..LAMPS {
        let ang = k as f32 / LAMPS as f32 * TAU - FRAC_PI_2;
        let (px, py) = (hx + (ang.cos() * (r_out - 1.8)).round() as i32, hy + (ang.sin() * (r_out - 1.8)).round() as i32);
        let near_top = {
            let d = (k as f32 / LAMPS as f32).min(1.0 - k as f32 / LAMPS as f32);
            d < 0.1
        };
        let lit = match s {
            None => (k as u32 * 5 + tick / 14) % 7 == 0,
            Some(s) => match (s.since, s.beat) {
                (Some(l), _) if l < 30 => (clock / 3 + u32::from(k % 2 == 0)) % 2 == 0,
                (Some(l), _) if matches!(s.luck, Luck::Grand) && l < 110 => (k as u32 + clock / 2) % 2 == 0,
                (Some(_), _) => (k as u32 * 5 + tick / 14) % 5 != 0,
                (None, Beat::Wind) => true,
                (None, Beat::Run) => (k as u32 + clock / 2 % 3) % 3 == 0,
                (None, Beat::Hang) => near_top && (clock / 2) % 2 == 0,
                (None, _) => (clock / 2) % 2 == 0,
            },
        };
        let ink = if lit {
            match s.map(|s| s.luck) {
                Some(Luck::Grand) if s.is_some_and(|s| s.since.is_some()) && k % 2 == 0 => '@',
                _ => '6',
            }
        } else {
            'B'
        };
        cv.rect(px - 1, py - 1, 2, 2, ink);
        if lit {
            cv.put(px - 1, py - 1, 'w');
        }
    }
}

/// The pointer: a gold flapper on a pivot over the rim, pushed aside by each
/// peg and snapping back (ringing, while the wheel is slow), hung hard against
/// the peg while the wheel hangs.
fn pointer(cv: &mut Img, s: Option<&Show>, (hx, hy): (i32, i32), r_out: f32, clock: u32) {
    let pivot_y = hy - r_out.round() as i32 - 4;
    let (rot, speed) = s.map_or((0.0, 0.0), |s| (s.rot, s.speed.abs()));
    let mut push = if speed > 1.0 {
        // Too fast to read each peg: it buzzes.
        0.25 + 0.2 * motion::math::sin(clock as f32 * 2.3)
    } else {
        show::push(rot)
    };
    if let Some(s) = s {
        if let Some(age) = s.since_click(10) {
            // The snap back overshoots the other way, then rings down.
            let a = age as f32;
            push = push.min(0.0) - 0.55 * motion::math::exp(-0.45 * a) * motion::math::cos(a * 1.5);
        }
        if s.beat == Beat::Hang {
            push = push.max(0.0) + 0.05 * motion::math::sin(clock as f32 * 1.9);
        }
        if let Some(l) = s.since {
            let a = l as f32;
            push = -0.3 * motion::math::exp(-0.3 * a) * motion::math::cos(a * 1.3);
        }
    }
    let swing = push * 7.0;
    for k in 0..10 {
        let t = k as f32 / 9.0;
        let cx = hx as f32 + swing * t * t;
        let half = ((1.0 - t) * 4.5).round() as i32;
        let y = pivot_y + k;
        for dx in -half..=half {
            let ink = if k < 2 { '6' } else if dx < 0 { '6' } else if dx > half / 2 { 'o' } else { '5' };
            cv.put(cx.round() as i32 + dx, y, ink);
        }
    }
    cv.put((hx as f32 + swing).round() as i32, pivot_y + 10, '4');
    cv.rect(hx - 1, pivot_y - 1, 3, 2, 'k');
}

/// The landing: the shock off the pointer, sparks, the wedge's flare and a
/// ring of light round the whole wheel, each as the wedge's luck asks.
fn landing(cv: &mut Img, s: &Show, (hx, hy): (i32, i32), r_out: f32, l: u32, clock: u32) {
    let tip = (hx as f32, hy as f32 - r_out + 6.0);
    let (flare, ring) = match s.luck {
        Luck::Grand => (ramps::EMBER, ramps::FIRE),
        Luck::Fair => (ramps::GOLD, ramps::GOLD),
        Luck::Plain => (ramps::BONE, ramps::SMOKE),
    };
    let age = l as f32;
    let mut pen = Pen::new(cv, 900);
    if l < 24 {
        pen.shock(tip, 6.0 + age * 5.0, 2.0, 0.9, ring, (age / 24.0).min(1.0));
    }
    if l < 36 && !matches!(s.luck, Luck::Plain) {
        pen.shock((hx as f32, hy as f32), r_out + 2.0 + age * 3.2, 2.4, 1.0, flare, (age / 36.0).min(1.0));
    }
    let n = match s.luck { Luck::Grand => 40, Luck::Fair => 26, Luck::Plain => 10 };
    let burst = Spray { gravity: 0.07, drag: 0.08, angle: -FRAC_PI_2, spread: 2.6, ..Spray::round(0x4f01, n, tip, (1.6, 5.2), (14.0, 30.0)) };
    burst.each(age, |m| pfx::mote(&mut pen, &m, Shape::Spark, flare, 2, clock as u64));
    if !matches!(s.luck, Luck::Plain) {
        let glitter = Spray { gravity: 0.03, drag: 0.1, angle: -FRAC_PI_2, spread: 3.4, ..Spray::round(0x4f02, n / 2, tip, (0.8, 2.6), (24.0, 44.0)) };
        glitter.each(age, |m| pfx::mote(&mut pen, &m, Shape::Star, ramps::GOLD, 1, clock as u64));
    }
}

/// Sparks thrown off the pointer by a peg as the wheel slows: a few, off
/// to the side the peg was going.
pub(super) fn clicks(cv: &mut Img, run: &Run, s: &Show) {
    let Some(age) = s.since_click(7) else { return };
    if s.speed.abs() > 0.6 {
        return;
    }
    let (hx, hy) = hub();
    let r_out = R0 * (1.0 + swell(Some(s)));
    let tip = (hx as f32 + 3.0, hy as f32 - r_out + 6.0);
    let seed = s.spin.pegs(s.n - age) as u32;
    let mut pen = Pen::new(cv, 120);
    let spray = Spray { gravity: 0.05, drag: 0.1, angle: -0.9, spread: 1.4, ..Spray::round(seed ^ 0x77, 5, tip, (1.2, 2.8), (6.0, 11.0)) };
    spray.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 1, run.tick));
}
