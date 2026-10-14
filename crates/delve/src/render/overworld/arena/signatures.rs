//! Material-ink guardian poses and the floor marks that make their attacks fair.
use super::super::ink::{Img, text_width};
use super::{UNIT, at, enlarge, stature};
use crate::rules::{Boss, Enemy, Run, signatures as fight};
use std::f32::consts::TAU;

pub(super) fn figure(run: &Run, enemy: &Enemy, boss: &Boss, tick: u32) -> Option<Img> {
    if boss.id == fight::RIPPER {
        let mut im = Img::from_rows(&boss.art.iter().map(String::as_str).collect::<Vec<_>>());
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
        return None;
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
    im.outline_outside('J');
    if enemy.stage == fight::JUMP {
        let rise = (enemy.timer as f32 / 24.0 * std::f32::consts::PI).sin() * 14.0;
        let mut airborne = Img::new(im.w, im.h + rise.max(0.0) as i32);
        airborne.stamp(&im, 0, 0);
        return Some(airborne);
    }
    Some(im)
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
    im.outline_outside('J');
    im
}

/// Wide spiders stand with their boiler/coal bed over the collision centre.
/// Their feet reach one hit radius below it in the top-down view as well.
pub(super) fn foot_lift(run: &Run, enemy: &Enemy) -> f32 {
    if enemy.kind == crate::rules::EnemyKind::Spiderling
        && enemy.stage == fight::TENDER
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
            '6'
        } else if pulse {
            '@'
        } else {
            'O'
        }
    } else {
        'J'
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
        let edge = if lit { '7' } else { 'o' };
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

pub(super) fn under_with(cv: &mut Img, run: &Run, modern: bool) {
    for e in run.enemies.iter().filter(|e| e.hp > 0) {
        if e.kind == crate::rules::EnemyKind::Spiderling
            && e.stage == fight::TENDER
        {
            let (x, y) = at(e.x, e.y);
            glow(cv, x, y + 5, 18.0);
        }
        let Some(b) = e.boss.and_then(|i| run.bosses.get(i as usize)) else {
            continue;
        };
        let (x, y) = at(e.x, e.y);
        if b.id == fight::ENGINE {
            if modern {continue;}
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
                let t = run.tick as f32 * 0.12;
                for arm in 0..3 {
                    for n in 0..38 {
                        let r = (fight::PULL_REACH * UNIT) * (1.0 - n as f32 / 40.0);
                        let a = t + arm as f32 * TAU / 3.0 + n as f32 * 0.13;
                        let px = x + (a.cos() * r) as i32;
                        let py = y + (a.sin() * r * 0.65) as i32;
                        cv.line(px, py, px + 2, py - 1, if n % 4 == 0 { 'Y' } else { 'y' });
                    }
                }
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
        }
    }
}
