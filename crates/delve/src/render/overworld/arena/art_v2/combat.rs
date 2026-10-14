//! Upgrade creatures and emission. Every tell reads the simulation's anchors.
use super::{super::*, figures, kit, paint::*};
use crate::rules::{Enemy, signatures as fight};

pub(super) fn supported(run: &Run, e: &Enemy) -> bool {
    e.frozen == 0
        && (matches!(
            e.kind,
            EnemyKind::Skeleton | EnemyKind::Wraith | EnemyKind::Slime | EnemyKind::Imp
        ) || e
            .boss
            .and_then(|i| run.bosses.get(i as usize))
            .is_some_and(|b| matches!(b.id.as_str(), fight::RIPPER | fight::ENGINE)))
}
pub(super) fn foe(cv: &mut Canvas, run: &Run, e: &Enemy) {
    let (x, y) = (e.x * UNIT, e.y * UNIT);
    if let Some(b) = e.boss.and_then(|i| run.bosses.get(i as usize)) {
        if b.id == fight::RIPPER {
            ripper(cv, x, y, e, run.tick);
            return;
        }
        if b.id == fight::ENGINE {
            engine(cv, x, y, e, run.tick);
            return;
        }
    }
    if e.kind == EnemyKind::Skeleton {
        figures::skeleton(cv, x, y, e, run.tick);
    } else {
        let name = match e.kind {
            EnemyKind::Wraith => "ghost",
            EnemyKind::Slime => "slime",
            EnemyKind::Imp => "imp",
            _ => return,
        };
        let im = kit::get(name);
        let p = (run.tick as f32 * 0.17 + e.id as f32).sin();
        let float = if e.kind == EnemyKind::Wraith {
            3.0 + p
        } else {
            0.0
        };
        cv.shadow(x, y + 1.0, im.w as f32 * 0.24, 2.5, 0.67);
        if e.kind == EnemyKind::Wraith {
            cv.glow(Point::new(x, y - 10.0, 18.0, '3', 0.65));
        }
        kit::sprite(
            cv,
            im,
            x - im.w as f32 * 0.25,
            y - im.h as f32 * 0.5 - float,
            e.dir.0 >= 0.0,
            1.0,
            None,
            p,
        );
    }
    if e.hurt > 0 {
        cv.flat_ellipse(x, y - 8.0, 6.0, 8.0, material('w'), 0.45);
    }
}
fn ripper(cv: &mut Canvas, x: f32, y: f32, e: &Enemy, tick: u64) {
    let size = fight::swell(e);
    let im = kit::get("ripper");
    let bob = (tick as f32 * 0.15).sin() * 0.4;
    cv.shadow(x, y + 2.0, 22.0 * size, 5.0, 0.8);
    kit::sprite(
        cv,
        im,
        x - im.w as f32 * size * 0.25,
        y - im.h as f32 * size * 0.5 - bob,
        true,
        size,
        None,
        0.0,
    );
}
fn engine(cv: &mut Canvas, x: f32, y: f32, e: &Enemy, tick: u64) {
    let steel = Ramp::inks('s', 'S', 'u');
    let brass = Ramp::inks('b', 'o', 'T');
    let phase = tick as f32 * 0.22;
    cv.shadow(x, y + 2.0, 38.0, 8.0, 0.8);
    for side in [-1.0, 1.0] {
        for i in 0..4 {
            let p = (phase + i as f32 * 1.7 + side).sin();
            let root = (x + side * 12.0, y - 17.0 + i as f32 * 2.0);
            let knee = (
                x + side * (25.0 + i as f32 * 2.0),
                y - 25.0 + i as f32 * 6.0 + p * 2.0,
            );
            let foot = (
                x + side * (29.0 - i as f32 * 2.0),
                y - 1.0 - p.max(0.0) * 4.0,
            );
            for (a, b) in [(root, knee), (knee, foot)] {
                cv.line(a, b, 3.5, steel.at(0.18));
                cv.line((a.0 - 0.5, a.1), (b.0 - 0.5, b.1), 1.8, steel.at(0.75));
            }
            cv.ellipse(knee.0, knee.1, 1.3, 1.3, brass, i);
            cv.line(
                (foot.0 - 2.0, foot.1),
                (foot.0 + 2.0, foot.1),
                1.2,
                brass.at(0.7),
            );
        }
    }
    let im = kit::get("engine-boiler");
    kit::sprite(
        cv,
        im,
        x - im.w as f32 * 0.25,
        y - im.h as f32 * 0.5,
        true,
        1.0,
        None,
        0.0,
    );
    for i in 0..7 {
        let age = ((tick + i * 7) % 36) as f32 / 36.0;
        cv.mist(
            x - 7.0 + age * 12.0 + (i as f32).sin() * 3.0,
            y - 41.0 - age * 17.0,
            2.0 + age * 4.0,
            2.0 + age * 3.0,
            material('i'),
            (1.0 - age) * 0.25,
        );
    }
    cv.glow(Point::new(x + 15.0, y - 27.0, 22.0, '@', 0.45));
    let _ = e;
}
pub(super) fn under(cv: &mut Canvas, run: &Run) {
    for cloud in &run.works.clouds {
        let (x, y, r) = (cloud.x * UNIT, cloud.y * UNIT, cloud.radius() * UNIT);
        cv.glow(Point::new(x, y, r * 2.0, 'A', 0.7));
        // The round core remains at the collision anchor. Wisps are translucent.
        let im = kit::get("gas-core");
        let scale = r * 3.6 / im.w as f32;
        kit::sprite(cv, im, x - r * 0.9, y - r * 0.9, true, scale, None, 0.0);
        for i in 0..5 {
            let a = i as f32 * 1.25 + run.tick as f32 * 0.05;
            cv.mist(
                x + a.cos() * r,
                y + a.sin() * r * 0.6,
                r * 0.6,
                r * 0.4,
                material('A'),
                0.10,
            );
        }
    }
    for e in &run.enemies {
        if !e
            .boss
            .and_then(|i| run.bosses.get(i as usize))
            .is_some_and(|b| b.id == fight::ENGINE)
        {
            continue;
        }
        for (i, r) in e.signature.rails.iter().enumerate() {
            let a = (r.a.0 * UNIT, r.a.1 * UNIT);
            let b = (r.b.0 * UNIT, r.b.1 * UNIT);
            let d = (b.0 - a.0, b.1 - a.1);
            let len = d.0.hypot(d.1).max(1.0);
            let n = (-d.1 / len, d.0 / len);
            let width = (e.radius() + crate::rules::HERO_RADIUS) * UNIT;
            let lit =
                i == e.signature.rail as usize && matches!(e.stage, fight::TELL | fight::DASH);
            for k in (0..len as i32).step_by(10) {
                let t = k as f32 / len;
                let p = (a.0 + d.0 * t, a.1 + d.1 * t);
                cv.line(
                    (p.0 - n.0 * width, p.1 - n.1 * width),
                    (p.0 + n.0 * width, p.1 + n.1 * width),
                    0.7,
                    material('o'),
                );
            }
            for side in [-1.0, 1.0] {
                let a = (a.0 + n.0 * width * side, a.1 + n.1 * width * side);
                let b = (b.0 + n.0 * width * side, b.1 + n.1 * width * side);
                cv.line(
                    a,
                    b,
                    if lit { 4.5 } else { 2.0 },
                    material(if lit { '7' } else { 'S' }),
                );
                cv.line(
                    a,
                    b,
                    if lit { 1.7 } else { 0.7 },
                    material(if lit { '9' } else { 'i' }),
                );
                if lit {
                    for k in (0..len as i32).step_by(8) {
                        let t = k as f32 / len;
                        cv.glow(Point::new(
                            a.0 + (b.0 - a.0) * t,
                            a.1 + (b.1 - a.1) * t,
                            8.0,
                            '@',
                            0.5,
                        ));
                    }
                }
            }
        }
    }
}

pub(super) fn shot_supported(p: &crate::rules::Projectile) -> bool {
    use crate::rules::Shot;
    matches!(p.kind, Shot::Wisp | Shot::Ember | Shot::Page | Shot::Ball)
        && p.look.is_none()
        && p.sprite_card().is_none()
        && !p.empowered
        && p.traits.path.beam.is_none()
        && !p.traits.path.waiting()
}
pub(super) fn shots(cv: &mut Canvas, run: &Run) {
    use crate::rules::Shot;
    for p in run.projectiles.iter().filter(|p| shot_supported(p)) {
        let (x, y) = (p.x * UNIT, p.y * UNIT);
        let speed = p.vx.hypot(p.vy).max(0.01);
        let (dx, dy) = (p.vx / speed, p.vy / speed);
        let cold = p.kind == Shot::Wisp;
        let rim = if cold { '3' } else { '@' };
        cv.glow(Point::new(x, y, if cold { 6.0 } else { 8.0 }, rim, 0.65));
        cv.line((x - dx * 2.7, y - dy * 2.7), (x, y), 1.0, material(rim));
        if p.kind == Shot::Page {
            // A small irregular ivory shard/burst replaces the cream square.
            for i in 0..5 {
                let a = i as f32 * 1.256 + run.tick as f32 * 0.05;
                cv.line(
                    (x + a.cos() * 1.0, y + a.sin() * 1.0),
                    (x + a.cos() * 2.6, y + a.sin() * 2.6),
                    0.5,
                    material('T'),
                );
            }
        }
        cv.line((x - 2.2, y), (x + 2.2, y), 0.5, material(rim));
        cv.line((x, y - 2.2), (x, y + 2.2), 0.5, material(rim));
        cv.ellipse(
            x,
            y,
            1.3,
            1.3,
            Ramp::inks(if cold { 'q' } else { '7' }, rim, '9'),
            17,
        );
        cv.flat_ellipse(
            x - 0.2,
            y - 0.2,
            0.55,
            0.55,
            crate::render::art::IVORY,
            1.0,
        );
    }
}
