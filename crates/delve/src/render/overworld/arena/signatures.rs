//! Material-ink guardian poses and the floor marks that make their attacks fair.
use super::super::ink::{Img, text_width};
use super::{UNIT, at, enlarge, stature};
use crate::rules::bosses::Windup;
use crate::rules::{Boss, Enemy, Run, signatures as fight};
use std::f32::consts::TAU;

pub(super) fn figure(run: &Run, enemy: &Enemy, boss: &Boss, tick: u32) -> Option<Img> {
    if boss.id == fight::RIPPER {
        let mut im = super::sprites::ripper();
        if enemy.stage == fight::INHALE {
            im.ellipse(15.5, 16.0, 3.0, 2.5, 'D');
            im.ellipse(15.5, 16.0, 1.5, 1.5, 'k');
            im.ellipse(5.0, 13.0, 3.5, 3.0, 'L');
            im.ellipse(26.0, 13.0, 3.5, 3.0, 'N');
        } else if enemy.stage == fight::TELL {
            im.rect(9, 10, 5, 2, 'N');
            im.rect(18, 10, 5, 2, 'N');
            im.line(9, 11, 13, 10, 'D');
            im.line(18, 10, 22, 11, 'D');
            im.rect(13, 16, 6, 2, 'D');
            im.rect(14, 16, 4, 1, 'G');
            im.line(9, 19, 11, 22, 'N');
            im.line(21, 19, 20, 23, 'N');
        } else if enemy.stage == fight::DAZED {
            im.line(10, 10, 13, 12, 'D');
            im.line(13, 10, 10, 12, 'D');
            im.line(18, 10, 21, 12, 'D');
            im.line(21, 10, 18, 12, 'D');
            im.rect(14, 16, 5, 3, 'D');
            im.rect(15, 16, 3, 1, 'G');
        }
        im.put(11, 16, 'T');
        im.put(21, 16, 'O');
        let im = super::foe_material::ripper(&im);
        let base = stature::boss(&im, boss.radius, stature::floor(run, enemy));
        let im = enlarge(&base, fight::swell(enemy));
        if enemy.stage == fight::TELL {
            let mut shaking = Img::new(im.w + 2, im.h);
            shaking.stamp(&im, (tick % 2) as i32, 0);
            return Some(shaking);
        }
        return Some(im);
    }
    if boss.id != fight::ENGINE {
        return winding_figure(run, enemy, boss, tick);
    }
    let mut im = Img::new(80, 64);
    let step = (tick / 4) % 4;
    let bob = if enemy.stage == 0 {
        (step % 2) as i32
    } else {
        0
    };
    // Alternating diagonal pairs plant, swing forward, lift, then reach back.
    // The wide knees and splayed toes stay outside the boiler in every pose.
    for side in [-1, 1] {
        for leg in 0..4 {
            let phase = (step + leg as u32 + u32::from(side > 0) * 2) % 4;
            let swing = [-5, 0, 5, 0][phase as usize];
            let lift = if phase == 2 { 7 } else { 0 };
            let root = (40 + side * 10, 27 + leg * 3 + bob);
            let knee = (
                40 + side * [37, 34, 28, 22][leg as usize],
                19 + leg * 8 + swing,
            );
            let foot = (
                40 + side * [26, 36, 18, 30][leg as usize],
                49 + leg * 4 - lift,
            );
            limb(&mut im, root, knee, side);
            limb(&mut im, knee, foot, side);
            im.rect(knee.0 - 2, knee.1 - 2, 5, 5, 'o');
            im.rect(knee.0 - 1, knee.1 - 1, 3, 3, 't');
            im.put(knee.0 - 1, knee.1 - 1, 'T');
            im.line(foot.0 - 2, foot.1, foot.0 + 2, foot.1, 't');
        }
    }
    // Cool iron and lit brass separate the boiler from the warm cave floor.
    im.ellipse(48.0, 31.0 + bob as f32, 16.0, 12.0, 'J');
    im.ellipse(49.0, 32.0 + bob as f32, 15.0, 10.0, 'g');
    im.ellipse(49.0, 29.0 + bob as f32, 14.0, 7.0, 'G');
    im.line(36, 22 + bob, 59, 22 + bob, 'h');
    im.rect(54, 13 + bob, 12, 15, 'J');
    im.rect(55, 14 + bob, 10, 13, 'g');
    im.rect(52, 11 + bob, 16, 3, 't');
    im.line(53, 11 + bob, 67, 11 + bob, 'T');
    im.rect(57, 16 + bob, 6, 7, 'o');
    im.rect(58, 17 + bob, 4, 5, '4');
    im.put(59, 18 + bob, '6');
    for x in [43, 52, 61] {
        im.rect(x, 23 + bob, 3, 17, 'o');
        im.line(x, 23 + bob, x, 39 + bob, 't');
        im.put(x, 24 + bob, 'T');
        im.put(x, 38 + bob, 'T');
    }
    im.rect(22, 44 + bob, 44, 4, 'o');
    im.line(22, 44 + bob, 65, 44 + bob, 't');
    for x in (24..65).step_by(7) {
        im.put(x, 45 + bob, 'T');
    }
    // Flared funnel, with a permanent moving soot plume within the silhouette.
    im.rect(33, 10 + bob, 9, 15, 'G');
    im.rect(34, 11 + bob, 6, 13, 'g');
    im.line(34, 11 + bob, 34, 24 + bob, 'h');
    im.rect(30, 8 + bob, 15, 4, 'o');
    im.line(30, 8 + bob, 44, 8 + bob, 't');
    for k in 0..3 {
        let drift = ((tick + k * 4) % 12) as i32;
        let cx = 37 + drift / 3 + k as i32 * 3;
        let cy = 8 - drift / 2;
        im.ellipse(cx as f32, cy as f32, (4 + k) as f32, 3.0, 'G');
        im.ellipse(cx as f32 + 1.0, cy as f32 + 1.0, (3 + k) as f32, 2.0, 'g');
        im.line(cx - 2, cy - 2, cx + 2, cy - 2, 'J');
    }
    // The porcelain mask is the focal point: shaded cream, cracks, red lamps,
    // and a broad black crescent full of irregular teeth.
    let fy = 32 + bob;
    im.ellipse(29.0, fy as f32, 13.0, 15.0, 'o');
    im.ellipse(29.0, fy as f32 - 1.0, 12.0, 14.0, 'h');
    im.ellipse(28.0, fy as f32 - 2.0, 10.5, 12.5, 'H');
    im.ellipse(36.0, fy as f32 + 1.0, 3.0, 10.0, 'i');
    for (a, b) in [
        ((28, fy - 14), (26, fy - 10)),
        ((26, fy - 10), (29, fy - 7)),
        ((19, fy + 1), (23, fy + 4)),
        ((23, fy + 4), (21, fy + 7)),
        ((38, fy - 9), (35, fy - 6)),
    ] {
        im.line(a.0, a.1, b.0, b.1, 'J');
    }
    for x in [23, 35] {
        im.ellipse(x as f32, (fy - 4) as f32, 4.5, 5.0, 'O');
        im.ellipse(x as f32, (fy - 4) as f32, 3.5, 4.0, '8');
        im.ellipse(x as f32, (fy - 4) as f32, 2.5, 3.0, '7');
        im.rect(x - 1, fy - 6, 2, 2, '@');
        im.put(x - 2, fy - 8, 'T');
    }
    im.ellipse(29.0, (fy + 6) as f32, 9.5, 6.0, 'K');
    im.ellipse(29.0, (fy + 1) as f32, 8.0, 3.5, 'H');
    for x in [22, 25, 28, 31, 34, 37] {
        im.rect(x, fy + 5, 2, if x == 22 || x == 37 { 2 } else { 4 }, 'H');
    }
    im.line(25, fy + 10, 32, fy + 11, 'i');
    im.put(27, fy, 'J');
    let im = super::foe_material::dress(&im, crate::rules::EnemyKind::Boss);
    if enemy.stage == fight::JUMP {
        let rise = (enemy.timer as f32 / 24.0 * std::f32::consts::PI).sin() * 14.0;
        let mut airborne = Img::new(im.w, im.h + rise.max(0.0) as i32);
        airborne.stamp(&im, 0, 0);
        return Some(airborne);
    }
    Some(im)
}

/// A guardian at one of its file's charge-ups: crouched lower and wider as
/// it winds up, shaking at the last; up in the air as it leaps.
fn winding_figure(run: &Run, enemy: &Enemy, boss: &Boss, tick: u32) -> Option<Img> {
    let up = fight::winding(enemy, boss)?;
    let index = enemy.boss?;
    let im = super::stature::boss_figure(run, index)?;
    match enemy.stage {
        fight::WIND => {
            let t = (enemy.timer as f32 / up.wind.max(1) as f32).min(1.0);
            let im = squash(&im, 1.0 + 0.14 * t, 1.0 - 0.18 * t);
            if t > 0.6 {
                let mut shaking = Img::new(im.w + 2, im.h);
                shaking.stamp(&im, (tick % 2) as i32 * 2, 0);
                return Some(shaking);
            }
            Some(im)
        }
        fight::JUMP => {
            let t = enemy.timer as f32 / fight::FLIGHT as f32;
            let rise = ((t * std::f32::consts::PI).sin() * 26.0).max(0.0) as i32;
            let im = squash(&im, 0.92, 1.1);
            let mut airborne = Img::new(im.w, im.h + rise * 2);
            airborne.stamp(&im, 0, 0);
            Some(airborne)
        }
        fight::DASH => Some(squash(&im, 1.12, 0.9)),
        _ => None,
    }
}

/// `im` drawn `kx` as wide and `ky` as tall, its feet where they were.
fn squash(im: &Img, kx: f32, ky: f32) -> Img {
    let (w, h) = (
        (im.w as f32 * kx).round().max(1.0) as i32,
        (im.h as f32 * ky).round().max(1.0) as i32,
    );
    let mut out = Img::new(w, im.h.max(h));
    let top = im.h.max(h) - h;
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get((x as f32 / kx) as i32, (y as f32 / ky) as i32) {
                out.set(x, top + y, c);
            }
        }
    }
    out
}

fn limb(im: &mut Img, a: (i32, i32), b: (i32, i32), side: i32) {
    for dx in -1..=1 {
        im.line(a.0 + dx, a.1, b.0 + dx, b.1, 'G');
    }
    im.line(a.0 - side, a.1 - 1, b.0 - side, b.1 - 1, 'h');
    im.line(a.0, a.1, b.0, b.1, 'j');
}

pub(super) fn tender(tick: u32) -> Img {
    let mut im = Img::new(30, 26);
    for side in [-1, 1] {
        for leg in 0..3 {
            let swing = if (tick / 4 + leg as u32 + u32::from(side > 0)) % 2 == 0 {
                3
            } else {
                -3
            };
            let root = (15 + side * 6, 13 + leg * 2);
            let knee = (15 + side * 12, 12 + leg * 3 + swing);
            let foot = (15 + side * (8 + leg * 2), 23 - leg);
            limb(&mut im, root, knee, side);
            limb(&mut im, knee, foot, side);
            im.put(knee.0, knee.1, 't');
        }
    }
    im.rect(7, 9, 17, 10, 'O');
    im.rect(8, 10, 15, 7, 'g');
    im.line(8, 10, 22, 10, 't');
    im.rect(8, 17, 15, 2, 'o');
    for (x, y) in [(8, 7), (11, 5), (15, 6), (19, 5), (12, 8), (18, 8)] {
        im.rect(x, y, 4, 4, 'K');
        im.line(x, y, x + 2, y, 'G');
    }
    // One hanging lantern, large enough to survive terminal sampling.
    im.rect(12, 12, 7, 7, 't');
    im.rect(13, 13, 5, 5, '7');
    im.rect(14, 14, 3, 3, '@');
    im.put(15, 14, '6');
    let im = super::foe_material::dress(&im, crate::rules::EnemyKind::Boss);
    im
}

/// Wide spiders stand with their boiler/coal bed over the collision centre.
/// Their feet reach one hit radius below it in the top-down view as well.
pub(super) fn foot_lift(run: &Run, enemy: &Enemy) -> f32 {
    if enemy.kind == crate::rules::EnemyKind::Spiderling && enemy.stage == fight::TENDER
        || enemy
            .boss
            .and_then(|i| run.bosses.get(i as usize))
            .is_some_and(|b| b.id == fight::ENGINE)
    {
        enemy.radius()
    } else {
        0.0
    }
}

fn rail(cv: &mut Img, r: &fight::Rail, width: f32, lit: bool, stage: u8, tick: u32) {
    let (a, b) = (at(r.a.0, r.a.1), at(r.b.0, r.b.1));
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = (dx as f32).hypot(dy as f32).max(1.0);
    let (nx, ny) = (-dy as f32 / d, dx as f32 / d);
    let pulse = tick / 4 % 2 == 0;
    let core = if lit {
        if stage == fight::DASH {
            'c'
        } else if pulse {
            '@'
        } else {
            'O'
        }
    } else {
        'U'
    };
    // Sleepers and rails bound the actual swept collision corridor.
    for n in (0..d as i32).step_by(10) {
        let (x, y) = (
            a.0 + (dx as f32 * n as f32 / d) as i32,
            a.1 + (dy as f32 * n as f32 / d) as i32,
        );
        cv.line(
            x - (nx * (width + 3.0)) as i32,
            y - (ny * (width + 3.0)) as i32,
            x + (nx * (width + 3.0)) as i32,
            y + (ny * (width + 3.0)) as i32,
            if lit && pulse { 'O' } else { 'P' },
        );
    }
    for side in [-1.0, 1.0] {
        let offset = width * side;
        let edge = if lit { 'o' } else { 'P' };
        let thickness = if lit && pulse { 3 } else { 2 };
        for n in -thickness..=thickness {
            let (ox, oy) = (
                (nx * (offset + n as f32)) as i32,
                (ny * (offset + n as f32)) as i32,
            );
            cv.line(a.0 + ox, a.1 + oy, b.0 + ox, b.1 + oy, edge);
        }
        for n in -1..=1 {
            let (ox, oy) = (
                (nx * (offset + n as f32)) as i32,
                (ny * (offset + n as f32)) as i32,
            );
            cv.line(a.0 + ox, a.1 + oy, b.0 + ox, b.1 + oy, core);
        }
    }
}

fn glow(cv: &mut Img, x: i32, y: i32, reach: f32) {
    for dy in -(reach as i32 / 2)..=reach as i32 / 2 {
        for dx in -(reach as i32)..=reach as i32 {
            let d = (dx as f32 / reach).hypot(dy as f32 / (reach * 0.5));
            if d < 1.0 && super::super::ink::bayer(x + dx, y + dy) < (1.0 - d) * 0.8 {
                cv.put(
                    x + dx,
                    y + dy,
                    if d < 0.3 {
                        'r'
                    } else if d < 0.6 {
                        'P'
                    } else {
                        'I'
                    },
                );
            }
        }
    }
}

/// Three arms of dashes spiralling in on `(x, y)`: something drawing the
/// room toward it.
fn pull_arms(cv: &mut Img, x: i32, y: i32, tick: u64, (bright, dim): (char, char)) {
    let t = tick as f32 * 0.12;
    for arm in 0..3 {
        for n in 0..38 {
            let r = (fight::PULL_REACH * UNIT) * (1.0 - n as f32 / 40.0);
            let a = t + arm as f32 * TAU / 3.0 + n as f32 * 0.13;
            let px = x + (a.cos() * r) as i32;
            let py = y + (a.sin() * r * 0.65) as i32;
            cv.line(px, py, px + 2, py - 1, if n % 4 == 0 { bright } else { dim });
        }
    }
}

/// A dotted ring on the floor, `rx` across, flattened to the floor's slant.
fn floor_ring(cv: &mut Img, x: i32, y: i32, rx: f32, ink: char, dots: u32) {
    for k in 0..dots {
        let a = k as f32 / dots as f32 * TAU;
        cv.put(x + (a.cos() * rx) as i32, y + (a.sin() * rx * 0.55) as i32, ink);
        cv.put(x + (a.cos() * rx) as i32 + 1, y + (a.sin() * rx * 0.55) as i32, ink);
    }
}

/// A target closing in on `(x, y)`: a bold ring `rx` across and four
/// brackets pointing in.
fn closing(cv: &mut Img, x: i32, y: i32, rx: f32, ink: char) {
    for r in [rx, rx + 1.0] {
        floor_ring(cv, x, y, r, ink, 96);
    }
    for q in 0..4 {
        let a = TAU / 8.0 + q as f32 * TAU / 4.0;
        let (cx, cy) = (a.cos(), a.sin() * 0.55);
        let out = (x + (cx * (rx + 9.0)) as i32, y + (cy * (rx + 9.0)) as i32);
        let tip = (x + (cx * (rx + 3.0)) as i32, y + (cy * (rx + 3.0)) as i32);
        for d in -1..=1 {
            cv.line(out.0 + d, out.1, tip.0 + d, tip.1, ink);
        }
    }
}

/// A charge's lane from `a` to `b`, `width` either side: a warm dithered
/// floor that fills in as the wind-up goes on (`t`), bright marching edges
/// and chevrons the way it will come.
fn lane(cv: &mut Img, a: (i32, i32), b: (i32, i32), width: f32, t: f32, tick: u64) {
    let d = ((b.0 - a.0) as f32).hypot((b.1 - a.1) as f32).max(1.0);
    let (ux, uy) = ((b.0 - a.0) as f32 / d, (b.1 - a.1) as f32 / d);
    let (nx, ny) = (-uy, ux);
    let hot = if tick / 3 % 2 == 0 { '@' } else { '7' };
    let fill = 0.15 + 0.4 * t;
    for k in 0..d as i32 {
        let k = k as f32;
        for w in -(width as i32)..=width as i32 {
            let (px, py) = (
                a.0 + (ux * k + nx * w as f32) as i32,
                a.1 + (uy * k + ny * w as f32) as i32,
            );
            if super::super::ink::bayer(px, py) < fill {
                cv.put(px, py, if t > 0.66 { 'r' } else { 'P' });
            }
        }
        let march = (tick % 8) as f32;
        if (k + march) % 8.0 < 5.0 {
            for side in [-1.0, 1.0] {
                cv.put(
                    a.0 + (ux * k + nx * width * side) as i32,
                    a.1 + (uy * k + ny * width * side) as i32,
                    hot,
                );
            }
        }
    }
    let mut k = 12.0 + (tick % 12) as f32;
    while k < d - 4.0 {
        let (cx, cy) = (a.0 + (ux * k) as i32, a.1 + (uy * k) as i32);
        for s in [-1.0, 1.0] {
            for o in 0..2 {
                cv.line(
                    cx - (ux * o as f32) as i32,
                    cy - (uy * o as f32) as i32,
                    cx + ((-ux + nx * s) * (width * 0.7)) as i32,
                    cy + ((-uy + ny * s) * (width * 0.7)) as i32,
                    hot,
                );
            }
        }
        k += 12.0;
    }
}

/// The floor under a guardian winding up one of its file's charge-ups:
/// a ring closing in on it as the blow nears, and where the blow will go —
/// a charge's lane, a leap's growing shadow, a breath's pull. A slam's
/// ripples and a beam's line draw themselves (`tide`, `volleys`).
fn charge_marks(cv: &mut Img, run: &Run, e: &Enemy, b: &Boss) {
    let Some(up) = fight::winding(e, b) else {
        return;
    };
    let (x, y) = at(e.x, e.y);
    let pulse = run.tick / 3 % 2 == 0;
    let hot = if pulse { '@' } else { '7' };
    let t = (e.timer as f32 / up.wind.max(1) as f32).min(1.0);
    if e.stage == fight::WIND {
        // Closing in: wide and dim at first, tight and bright at the blow.
        let rx = e.radius() * UNIT * (2.4 - 1.2 * t) + 6.0;
        closing(cv, x, y, rx, if t > 0.66 { hot } else if t > 0.33 { '7' } else { 'o' });
    }
    match up.kind {
        Windup::Charge if matches!(e.stage, fight::WIND | fight::DASH) => {
            if let Some(r) = e.signature.rails.first() {
                let width = (e.radius() + crate::rules::HERO_RADIUS) * UNIT;
                let t = if e.stage == fight::DASH { 1.0 } else { t };
                lane(cv, at(r.a.0, r.a.1), at(r.b.0, r.b.1), width, t, run.tick);
            }
        }
        Windup::Leap if matches!(e.stage, fight::WIND | fight::JUMP) => {
            // Where it will land: a warm shadow growing to the blow's full
            // reach, ringed and crossed.
            let t = if e.stage == fight::WIND {
                0.3 + 0.6 * t
            } else {
                0.9 + 0.1 * (e.timer as f32 / fight::FLIGHT as f32).min(1.0)
            };
            let (mx, my) = at(e.signature.mark.0, e.signature.mark.1);
            let reach = (fight::LANDING + crate::rules::HERO_RADIUS) * UNIT;
            glow(cv, mx, my, reach * t * 1.2);
            closing(cv, mx, my, reach, hot);
            for d in -1..=1 {
                cv.line(mx - 6, my + d, mx + 6, my + d, hot);
                cv.line(mx + d, my - 4, mx + d, my + 4, hot);
            }
        }
        Windup::Inhale if e.stage == fight::WIND => {
            pull_arms(cv, x, y, run.tick, ('7', 'o'));
        }
        Windup::Slam if e.stage == fight::WIND => {
            // Each shockwave ring where it will burst, warm and brightening,
            // with its lane left dark.
            let lane = run
                .ravages
                .iter()
                .find(|r| (r.x - e.x).hypot(r.y - e.y) < 0.5);
            for k in 0..up.shots.min(8) as u8 {
                let radius = crate::rules::tide::RING_GAP * f32::from(k + 1);
                let n = (TAU * radius * UNIT / 2.0) as u32;
                for i in 0..n {
                    let a = i as f32 / n as f32 * TAU;
                    let (ux, uy) = (e.x + a.cos() * radius, e.y + a.sin() * radius);
                    if lane.is_some_and(|r| r.in_lane(ux, uy)) {
                        continue;
                    }
                    if (i + (run.tick / 2) as u32).is_multiple_of(3) && t < 0.66 {
                        continue;
                    }
                    let (px, py) = at(ux, uy);
                    cv.put(px, py, if t > 0.66 { hot } else { 'o' });
                }
            }
        }
        _ => {}
    }
}

pub(super) fn under_with(cv: &mut Img, run: &Run, modern: bool) {
    for e in run.enemies.iter().filter(|e| e.hp > 0) {
        if let Some(b) = e.boss.and_then(|i| run.bosses.get(i as usize))
            && !fight::special(b)
        {
            charge_marks(cv, run, e, b);
        }
        if e.kind == crate::rules::EnemyKind::Spiderling && e.stage == fight::TENDER {
            let (x, y) = at(e.x, e.y);
            glow(cv, x, y + 5, 18.0);
        }
        let Some(b) = e.boss.and_then(|i| run.bosses.get(i as usize)) else {
            continue;
        };
        let (x, y) = at(e.x, e.y);
        if b.id == fight::ENGINE {
            if modern {
                continue;
            }
            glow(cv, x, y + 16, e.radius() * UNIT + 10.0);
            // Drifting steam curls catch lantern light while leaving floor gaps.
            if (e.stage == 0 && e.timer > 70) || (e.stage == fight::TELL && e.timer < 10) {
                for k in 0..9 {
                    let drift = ((run.tick as i32 + k * 7) % 18) - 9;
                    let cx = x + (k % 3 - 1) * 23 + drift;
                    let cy = y + 5 + (k / 3) * 9 - drift / 2;
                    for n in 0..24 {
                        let a = n as f32 * TAU / 24.0;
                        let px = cx + (a.cos() * 13.0) as i32;
                        let py = cy + (a.sin() * 5.0) as i32;
                        cv.line(px, py, px + 2, py, if n < 10 { 'h' } else { 'G' });
                    }
                }
            }
            let selected = e.signature.rail as usize;
            for (_, r) in e
                .signature
                .rails
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != selected)
            {
                rail(
                    cv,
                    r,
                    (e.radius() + crate::rules::HERO_RADIUS) * UNIT,
                    false,
                    e.stage,
                    e.timer,
                );
            }
            if let Some(r) = e.signature.rails.get(selected) {
                rail(
                    cv,
                    r,
                    (e.radius() + crate::rules::HERO_RADIUS) * UNIT,
                    matches!(e.stage, fight::TELL | fight::DASH),
                    e.stage,
                    e.timer,
                );
                if e.stage == fight::DASH {
                    let a = at(r.a.0, r.a.1);
                    let tail = at(e.x - e.dir.0 * 1.5, e.y - e.dir.1 * 1.5);
                    for dy in -3..=3 {
                        cv.line(
                            a.0,
                            a.1 + dy,
                            tail.0,
                            tail.1 + dy,
                            if dy.abs() < 2 { '6' } else { '7' },
                        );
                    }
                    for k in 0..6 {
                        let p = at(
                            e.x - e.dir.0 * (k as f32 + 2.0),
                            e.y - e.dir.1 * (k as f32 + 2.0),
                        );
                        cv.put(p.0 + k % 3 - 1, p.1 - 5, '@');
                    }
                }
            }
        } else if b.id == fight::RIPPER {
            if e.stage == fight::INHALE {
                pull_arms(cv, x, y, run.tick, ('Y', 'y'));
            }
            if e.stage == fight::TELL {
                let end = at(e.x + e.dir.0 * 9.0, e.y + e.dir.1 * 9.0);
                let ink = if run.tick / 3 % 2 == 0 { '4' } else { '7' };
                cv.line(x, y, end.0, end.1, ink);
                let (nx, ny) = (-e.dir.1, e.dir.0);
                for s in [-1.0, 1.0] {
                    cv.line(
                        end.0,
                        end.1,
                        end.0 + ((-e.dir.0 + nx * s) * 9.0) as i32,
                        end.1 + ((-e.dir.1 + ny * s) * 9.0) as i32,
                        ink,
                    );
                }
            }
        }
    }
}
pub(super) fn over(cv: &mut Img, run: &Run) {
    for e in run.enemies.iter().filter(|e| e.hp > 0) {
        let Some(b) = e.boss.and_then(|i| run.bosses.get(i as usize)) else {
            continue;
        };
        let (x, y) = at(e.x, e.y);
        if b.id == fight::RIPPER {
            let word = match e.stage {
                fight::INHALE => "HUUUP",
                fight::TELL => "CLENCH",
                fight::DASH => {
                    if e.signature.cycle % 2 == 0 {
                        "BRAAAP"
                    } else {
                        "PFRRRT"
                    }
                }
                fight::DAZED => "...pardon",
                _ => continue,
            };
            let h = (32.0 * fight::swell(e)) as i32;
            let right = super::camera(run, None).0 + super::NATIVE_W;
            let beside = x + h / 2 + 5;
            let px = if beside + text_width(word) < right - 4 {
                beside
            } else {
                x - h / 2 - text_width(word) - 5
            };
            cv.text(
                px,
                y - h / 2 - 5,
                word,
                if e.stage == fight::DASH { 'Y' } else { 'T' },
            );
            if e.stage == fight::DAZED {
                for k in 0..3 {
                    let a = run.tick as f32 * 0.15 + k as f32 * TAU / 3.0;
                    cv.put(
                        x + (a.cos() * 18.0) as i32,
                        y - 32 + (a.sin() * 5.0) as i32,
                        '4',
                    );
                }
            }
        } else if b.id == fight::ENGINE && e.stage == fight::TELL {
            cv.text(x + 45, y - 18, "SKREEEE", 'T');
        } else if let Some(up) = fight::winding(e, b).filter(|_| e.stage == fight::WIND) {
            // Its cry beside it, in a box, lit at the last.
            let word = up.cry.as_str();
            let half = (e.radius() * UNIT * 1.6) as i32 + 6;
            let right = super::camera(run, None).0 + super::NATIVE_W;
            let w = text_width(word);
            let px = if x + half + w + 4 < right {
                x + half
            } else {
                x - half - w
            };
            let late = e.timer * 3 > up.wind * 2;
            cv.rect(px - 2, y - 6, w + 4, 10, 'k');
            cv.frame(px - 2, y - 6, w + 4, 10, if late { '@' } else { '7' });
            cv.text(px, y - 4, word, if late { 'Y' } else { 'T' });
        }
    }
}
