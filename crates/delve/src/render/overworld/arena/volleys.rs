//! The monsters' pattern shots where a plain sprite will not do: a beam's
//! warning line and its burn, the marks where rain and crossfire will come
//! from, and the blink of a mine about to pop (`patterns`).

use super::*;

#[path = "ranged_events.rs"]
mod events;

#[path = "ranged_looks.rs"]
pub(super) mod ranged;

pub(super) fn muzzle(cv: &mut Img, run: &Run, p: &crate::rules::feel::Puff) -> bool { ranged::muzzle(cv, run, p) }
pub(super) fn accents(cv: &mut Img, run: &Run) { ranged::accents(cv, run); ranged::events(cv, run); }
pub(super) fn recoil(run: &Run, id: u32, hero: &crate::rules::Hero) -> f32 { ranged::recoil(run, id, hero) }
use crate::rules::patterns::{BEAM_WARN, SWEEP_WARN, warning_reach};
use crate::rules::{Projectile, Shot};

fn beam_inks(kind: Shot) -> (char, char, char) {
    match kind {
        Shot::Eye => ('1', '2', 'c'),
        Shot::Wisp => ('2', '2', 'c'),
        Shot::Spore => ('C', 'A', 'c'),
        Shot::Page => ('$', 'U', 'c'),
        _ => ('7', 'o', 'c'),
    }
}

pub(super) fn draw(cv: &mut Img, run: &Run, shot: &Projectile) -> bool {
    let path = &shot.traits.path;
    if let Some(beam) = path.beam {
        if crate::rules::bosses::Art::of_shot(shot) == Some(crate::rules::bosses::Art::Leviathan) {
            super::boss_art::leviathan_laser(cv, run, shot, beam);
            return true;
        }
        if super::deep_village::dread_lantern(cv, run, shot, beam) {
            return true;
        }
        let (warn, rim, core) = crate::rules::bosses::Art::of_shot(shot)
            .map(super::boss_art::inks)
            .unwrap_or_else(|| beam_inks(shot.kind));
        let (x, y) = at(shot.x, shot.y);
        let length = (beam.reach * UNIT) as i32;
        if beam.burning(path.age) {
            line(cv, (x, y), beam.angle, length, |cv, px, py, (nx, ny)| {
                for k in [-2, -1, 1, 2] {
                    cv.put(px + nx * k, py + ny * k, rim);
                }
                cv.put(px, py, core);
            });
        } else {
            let close = path.age + 7 > u16::from(beam.warn);
            let march = (run.tick / 2) as i32;
            let art = crate::rules::bosses::Art::of_shot(shot);
            let half = (crate::rules::patterns::BEAM_HALF * UNIT).ceil() as i32;
            let fill =
                i32::from(path.age.min(u16::from(beam.warn))) * half / i32::from(beam.warn.max(1));
            let charged = beam.warn > BEAM_WARN.max(SWEEP_WARN);
            let charge = f32::from(path.age) / f32::from(beam.warn.max(1));
            let width = if !charged {
                0
            } else if charge > 0.75 {
                2
            } else if charge > 0.4 {
                1
            } else {
                0
            };
            line(cv, (x, y), beam.angle, length, |cv, px, py, (nx, ny)| {
                let i = (px - x).abs().max((py - y).abs());
                if let Some(art) = art {
                    if art == crate::rules::bosses::Art::Index {
                        let stage =
                            (usize::from(path.age) * 4 / usize::from(beam.warn.max(1))).min(3);
                        for side in -half..=half {
                            if let Some(c) =
                                super::boss_art::index_lane_pixel(i, side, half, length, stage)
                            {
                                cv.set(px + nx * side, py + ny * side, c);
                            }
                        }
                        return;
                    }
                    for side in -half..=half {
                        let motif = match art {
                            crate::rules::bosses::Art::Cinderjaw => {
                                (i + side.abs() * 2).rem_euclid(12) < 2
                            }
                            crate::rules::bosses::Art::Index => {
                                i.rem_euclid(12) < 2 || (side == 0 && i.rem_euclid(12) < 6)
                            }
                            crate::rules::bosses::Art::Mother => (i + side * 3).rem_euclid(10) < 2,
                            crate::rules::bosses::Art::Waxen => (i + side.abs()).rem_euclid(11) < 2,
                            crate::rules::bosses::Art::Choir => (i - side * 2).rem_euclid(13) < 2,
                            crate::rules::bosses::Art::Marshal => (i + side * 2).rem_euclid(9) < 2,
                            crate::rules::bosses::Art::TaxKeeper
                            | crate::rules::bosses::Art::RelicLiege
                            | crate::rules::bosses::Art::IronTusk
                            | crate::rules::bosses::Art::RedPick
                            | crate::rules::bosses::Art::Foreman
                            | crate::rules::bosses::Art::Leviathan => i.rem_euclid(12) < 2,
                            _ => i.rem_euclid(12) < 2,
                        };
                        if side.abs() >= half - fill || motif {
                            cv.put(
                                px + nx * side,
                                py + ny * side,
                                if motif { core } else { rim },
                            );
                        }
                    }
                } else if close || (i + march) % 4 < 2 {
                    cv.put(px, py, warn);
                    for k in 1..=width {
                        cv.put(px + nx * k, py + ny * k, warn);
                        cv.put(px - nx * k, py - ny * k, warn);
                    }
                }
            });
            if beam.sweep != 0.0 {
                for i in 1..=32 {
                    let a = beam.angle + (beam.end_angle() - beam.angle) * i as f32 / 32.0;
                    let length = (warning_reach(run, (shot.x, shot.y), a) * UNIT) as i32;
                    line(cv, (x, y), a, length, |cv, px, py, _| {
                        let d = ((px - x) as f32).hypot((py - y) as f32) as i32;
                        if (i == 32 && (d + march) % 4 < 2) || (d + march) % 24 == 0 {
                            cv.put(px, py, warn);
                        }
                    });
                }
            }
        }
        return true;
    }
    if path.waiting() {
        if path.mark {
            let (x, y) = at(shot.x, shot.y);
            let lit = path.wait < 8 || (run.tick / 3).is_multiple_of(2);
            let art = crate::rules::bosses::Art::of_shot(shot);
            if art == Some(crate::rules::bosses::Art::Index) {
                index_crossfire_lane(cv, run, shot);
                return true;
            }
            let (rim, core) = art
                .map(|art| {
                    let (warn, rim, core) = super::boss_art::inks(art);
                    (if lit { rim } else { warn }, core)
                })
                .unwrap_or(if lit { ('@', '6') } else { ('7', '8') });
            if art.is_none() {
                for dy in -5i32..=5 {
                    for dx in -5i32..=5 {
                        let d = dx.abs() + dy.abs();
                        if d == 5 || d == 4 {
                            cv.put(x + dx, y + dy, rim);
                        } else if d <= 1 {
                            cv.put(x + dx, y + dy, core);
                        }
                    }
                }
            }
            let speed = shot.vx.hypot(shot.vy).max(0.001);
            let (ux, uy) = (shot.vx / speed, shot.vy / speed);
            for k in 7..13 {
                for side in [-1.0f32, 0.0, 1.0] {
                    if side != 0.0 && k > 8 {
                        continue;
                    }
                    cv.put(
                        x + (ux * k as f32 - uy * side * (13 - k) as f32 * 0.4).round() as i32,
                        y + (uy * k as f32 + ux * side * (13 - k) as f32 * 0.4).round() as i32,
                        rim,
                    );
                }
            }
        }
        return true;
    }
    if path.bursts() {
        if crate::rules::bosses::Art::of_shot(shot).is_some() && path.accel < 0.0 && path.top == 0.0
        {
            return shot.vx.hypot(shot.vy) < 0.05;
        }
        let (x, y) = at(shot.x, shot.y);
        let still = shot.vx.hypot(shot.vy) < 0.05;
        let lit = !still || (run.tick / 2).is_multiple_of(2);
        if lit {
            let r = if still { 7.0 } else { 6.0 };
            for i in 0..16 {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                cv.put(
                    x + (a.cos() * r).round() as i32,
                    y + (a.sin() * r).round() as i32,
                    crate::rules::bosses::Art::of_shot(shot)
                        .map(|art| super::boss_art::inks(art).0)
                        .unwrap_or(if still { '7' } else { '8' }),
                );
            }
        }
    }
    false
}

fn index_crossfire_lane(cv: &mut Img, run: &Run, shot: &Projectile) {
    let (x, y) = at(shot.x, shot.y);
    let angle = shot.vy.atan2(shot.vx);
    let length = (warning_reach(run, (shot.x, shot.y), angle) * UNIT) as i32;
    let speed = shot.vx.hypot(shot.vy);
    if speed == 0.0 {
        return;
    }
    let end = (
        x + (shot.vx / speed * length as f32).round() as i32,
        y + (shot.vy / speed * length as f32).round() as i32,
    );
    let half = ((crate::rules::HERO_RADIUS + 0.16) * UNIT).ceil() as i32;
    let side = if shot.vx.abs() > shot.vy.abs() {
        (0, 1)
    } else {
        (1, 0)
    };
    let warning = crate::rules::patterns::CROSS_MARK;
    let stage = (usize::from(warning.saturating_sub(shot.traits.path.wait)) * 4
        / usize::from(warning))
    .min(3);
    let (mut px, mut py) = (x, y);
    let (dx, dy) = ((end.0 - x).abs(), -(end.1 - y).abs());
    let (sx, sy) = ((end.0 - x).signum(), (end.1 - y).signum());
    let mut error = dx + dy;
    loop {
        let i = (px - x).abs().max((py - y).abs());
        for k in -half..=half {
            if let Some(c) = super::boss_art::index_lane_pixel(i, k, half, length, stage) {
                cv.set(px + side.0 * k, py + side.1 * k, c);
            }
        }
        if (px, py) == end {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            px += sx;
        }
        if twice <= dx {
            error += dx;
            py += sy;
        }
    }
}

fn line(
    cv: &mut Img,
    from: (i32, i32),
    angle: f32,
    length: i32,
    mut put: impl FnMut(&mut Img, i32, i32, (i32, i32)),
) {
    let (ux, uy) = (angle.cos(), angle.sin());
    let side = if ux.abs() > uy.abs() { (0, 1) } else { (1, 0) };
    for i in 0..length.max(0) {
        let px = from.0 + (ux * i as f32).round() as i32;
        let py = from.1 + (uy * i as f32).round() as i32;
        put(cv, px, py, side);
    }
}

fn tones(kind: Shot) -> (char, char, char) {
    match kind {
        Shot::Wisp => ('3', '2', 'q'),
        Shot::Bone => ('H', 'h', 'G'),
        Shot::Ore => ('T', 'O', 'r'),
        Shot::Flame | Shot::Ember => ('6', '@', 'o'),
        Shot::Page => ('$', 'U', 'u'),
        Shot::Spore => ('Y', 'C', 'm'),
        Shot::Eye => ('3', '2', '1'),
        Shot::Ball => ('t', 'O', 'r'),
        _ => ('V', 'v', 'u'),
    }
}

fn hue(kind: Shot) -> fx::Hue {
    match kind {
        Shot::Flame | Shot::Ember | Shot::Ore | Shot::Ball => fx::Hue::Warm,
        Shot::Spore => fx::Hue::Gas,
        _ => fx::Hue::Cold,
    }
}

fn streak(speed: f32) -> i32 {
    (speed * 1.1).round() as i32
}

pub(super) fn family(cv: &mut Img, run: &Run, shot: &Projectile) -> bool {
    use crate::rules::HERO_RADIUS;
    use crate::rules::patterns::{Family, LOB_SPLASH, family as of, lob_flight};
    if !shot.hostile || shot.look.is_some() || !shot.in_flight() {
        return false;
    }
    let (x, y) = at(shot.x, shot.y);
    let speed = shot.vx.hypot(shot.vy);
    let (ux, uy) = if speed > 1e-3 { (shot.vx / speed, shot.vy / speed) } else { (0.0, 1.0) };
    let (head, mid, tail) = tones(shot.kind);
    let back = |k: f32| (x - (ux * k).round() as i32, y - (uy * k).round() as i32);
    match of(shot) {
        Family::Dart => {
            let n = streak(speed).max(6);
            for k in (0..n).rev() {
                let ink = match k {
                    0 | 1 => head,
                    _ if k < n / 2 => mid,
                    _ => tail,
                };
                let (px, py) = back(k as f32);
                cv.put(px, py, ink);
            }
            true
        }
        Family::Orb => {
            crate::render::overworld::material::shadow(cv, (x as f32, y as f32 + 5.0), (6.0, 2.0), &|_, _| true);
            fx::glow(cv, x, y, hue(shot.kind));
            let r = 5;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > r as f32 + 0.3 {
                        continue;
                    }
                    let ink = if d > r as f32 - 0.8 { mid } else if (dx + dy) < -3 { mid } else { tail };
                    cv.put(x + dx, y + dy, ink);
                }
            }
            cv.put(x - 2, y - 2, head);
            cv.put(x - 1, y - 2, head);
            cv.put(x - 2, y - 1, head);
            true
        }
        Family::Lob => {
            let Some(((lx, ly), rise)) = lob_flight(shot) else {
                return false;
            };
            let path = &shot.traits.path;
            let k = f32::from(path.age) / f32::from(path.fuse.max(1));
            let (tx, ty) = at(lx, ly);
            let r = (HERO_RADIUS + LOB_SPLASH) * UNIT;
            let ring = if k > 0.7 { '7' } else { '8' };
            let n = 40;
            let step = if k > 0.7 { 1 } else { 2 };
            for i in (0..n).step_by(step) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU + run.tick as f32 * 0.05;
                cv.put(tx + (a.cos() * r).round() as i32, ty + (a.sin() * r * 0.6).round() as i32, ring);
            }
            let inner = r * (1.0 - k).max(0.15);
            for i in (0..n).step_by(4) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                cv.put(tx + (a.cos() * inner).round() as i32, ty + (a.sin() * inner * 0.6).round() as i32, '8');
            }
            let lift = (rise * UNIT).round() as i32;
            let size = (4.5 - rise * 0.5).max(2.0);
            let (sw, sh) = (size.round() as i32, (size * 0.5).round().max(1.0) as i32);
            for dy in -sh..=sh {
                for dx in -sw..=sw {
                    let (fx_, fy_) = (dx as f32 / size, dy as f32 / (size * 0.5).max(1.0));
                    let d = fx_ * fx_ + fy_ * fy_;
                    if d <= 1.0 {
                        cv.put(x + dx, y + dy, if d > 0.55 { 'j' } else { 'g' });
                    }
                }
            }
            let (bx, by) = (x, y - lift);
            fx::glow(cv, bx, by, hue(shot.kind));
            for (dx, dy) in [(0, -2), (-1, -1), (0, -1), (1, -1), (-2, 0), (-1, 0), (0, 0), (1, 0), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)] {
                cv.put(bx + dx, by + dy, if dy < 0 && dx <= 0 { head } else { mid });
            }
            cv.put(bx + 1, by + 1, tail);
            true
        }
        Family::Wave => {
            let n = streak(speed);
            let phase = f32::from(shot.traits.path.age) * shot.traits.path.wave_rate;
            for k in (2..n + 2).rev() {
                let side = (phase - k as f32 * 0.5).sin() * 2.0 * shot.traits.path.wave.signum();
                let (px, py) = back(k as f32);
                cv.put(px + (-uy * side).round() as i32, py + (ux * side).round() as i32, if k < n / 2 + 2 { mid } else { tail });
            }
            false
        }
        _ => {
            let n = streak(speed);
            for k in (2..n + 2).rev() {
                let (px, py) = back(k as f32);
                cv.put(px, py, if k < n / 2 + 2 { mid } else { tail });
            }
            false
        }
    }
}

pub(super) fn shots(cv: &mut Img, run: &Run) {
    let apart = capturing() && parts_outside() && !atari::here(run);
    let mut mark = Img::new(25, 25);
    let dimensions = apart.then(|| dimensions::Dimensions::of(run));
    for shot in &run.projectiles {
        if volleys::draw(cv, run, shot) {
            continue;
        }
        if apart && let Some(form) = ranged::form(shot) {
            let (x,y)=at(shot.x,shot.y);
            let (fx,fy)=pixel_fraction(shot.x,shot.y);
            let ink = ranged::projectile_look(shot);
            let bright = ink.body == ranged::Body::Magic;
            let hue = match ink.core { '3' | 'V' => room_scene::Hue::Cold, 'Y' => room_scene::Hue::Gas, _ => room_scene::Hue::Fire };
            with_capture(|capture|capture.shots.push(room_scene::SceneShot {
                x:x as f32+fx,y:y as f32+fy,vx:shot.vx*UNIT,vy:shot.vy*UNIT,
                hue,bright,hostile:shot.hostile,empowered:shot.empowered,form,
                dimension:dimensions.as_ref().map_or_else(Default::default,|d|d.shot(shot)),
                mark:Img::new(0,0),paint:None,scale:shot.scale() * if shot.empowered {1.25} else {1.0},
            }));
            continue;
        }
        if (!apart || shot.hostile) && ranged::flight(cv, run, shot) { continue; }
        if volleys::family(cv, run, shot) {
            continue;
        }
        let (x, y) = at(shot.x, shot.y);
        if let Some(art) = crate::rules::bosses::Art::of_shot(shot) {
            boss_art::projectile(cv, art, x, y, shot.vx, shot.vy, run.tick);
            continue;
        }
        if class_looks::peasant::stone_shot(cv, shot, x, y) { continue; }
        if dungeon_art::supported(run) && dungeon_art::knight_shot(cv, shot, x, y) {continue;}
        let size = shot.scale();
        let song = shot.look.as_ref().is_some_and(|(colour, _)| colour == crate::rules::classes::SONG);
        let hue = match (shot.tint(), shot.kind) {
            (Some(Tint::Burn), _) => fx::Hue::Warm,
            (Some(Tint::Chill), _) => fx::Hue::Cold,
            (Some(Tint::Poison), _) => fx::Hue::Gas,
            (None, _) if song => fx::Hue::Warm,
            (None, Shot::Ember | Shot::Flame | Shot::Sacred | Shot::Ball) => fx::Hue::Warm,
            (None, Shot::Hex | Shot::Spore) => fx::Hue::Gas,
            _ => fx::Hue::Cold,
        };
        if !apart {
            fx::glow_within(cv, x, y, hue, (25.0 * size.min(1.0) * size.min(1.0)).ceil() as i32);
        }
        mark.clear_all();
        if shot.empowered {
            for (dx, dy) in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
            ] {
                mark.put(12 + dx, 12 + dy, '6');
            }
        }
        let form;
        let mut fine: Option<(Img, (i32, i32))> = None;
        let mut painted: Option<paintings::Painting> = None;
        if let Some(sprite) = shot
            .sprite_card()
            .and_then(|id| run.book.get(id))
            .and_then(|card| card.looks.shot.as_ref())
        {
            let mut im = knights::sprite_img(sprite, run.tick);
            let flip = shot.vx < 0.0;
            if flip {
                im = im.flip_h();
            }
            let width = im.w.max(1);
            let side = im.w.max(im.h);
            if side > CARD_FINE_SIDE {
                im = enlarge(&im, CARD_FINE_SIDE as f32 / side as f32);
            }
            let heading = (shot.vx, shot.vy);
            let course = ((shot.vy.atan2(shot.vx) + std::f32::consts::PI) * 8.0 / std::f32::consts::PI) as u64;
            let phase = paintings::phase_of(&[run.tick + u64::from(shot.ttl), course]);
            let pose = paintings::Pose::of(&sprite.moves, run.tick, phase, heading);
            if apart {
                let mut small = im.clone();
                if side > CARD_SHOT_SIDE {
                    small = enlarge(&small, CARD_SHOT_SIDE as f32 / small.w.max(small.h) as f32);
                }
                let scale = small.w as f32 / width as f32;
                let (moved, (ox, oy)) = pose.apply(&small, paintings::pivot(sprite, flip, scale, (small.w, small.h)));
                let to = (12 - small.w / 2 + ox, 12 - small.h / 2 + oy);
                mark.stamp(&moved, to.0, to.1);
                painted = paintings::Painting::of(
                    sprite,
                    run.tick,
                    flip,
                    (-2.0 * (small.w / 2) as f32, -2.0 * (small.h / 2) as f32),
                    2.0 * scale,
                    pose,
                )
                .map(|mut p| {
                    p.inks = moved;
                    p.at = to;
                    p
                });
            } else {
                let im = if size == 1.0 { im } else { resized(&im, size) };
                let scale = im.w as f32 / width as f32;
                let (moved, (ox, oy)) = pose.apply(&im, paintings::pivot(sprite, flip, scale, (im.w, im.h)));
                fine = Some((moved, (ox - im.w / 2, oy - im.h / 2)));
            }
            form = ShotForm::Card;
        } else if shot.sprite_card().is_some() {
            sprites::shot(&mut mark, (12, 12), (shot.vx, shot.vy), shot.kind);
            form = ShotForm::Kind(shot.kind);
        } else if let Some((colour, _)) = &shot.look
            && class_looks::shot(&mut mark, (12, 12), (shot.vx, shot.vy), colour, run.tick)
        {
            if song
                && run
                    .players
                    .get(&u32::from(shot.traits.owner))
                    .is_some_and(|h| crate::rules::bard::stage(h.groove) >= 2)
            {
                class_looks::bard::glitch_mark(&mut mark, run.tick);
            }
            form = class_looks::class_of(colour).map_or(ShotForm::Kind(shot.kind), ShotForm::Class);
        } else if let Some((colour, shape)) = &shot.look {
            sprites::forged_shot(&mut mark, 12, 12, colour, shape);
            form = ShotForm::Forged(ForgedColour::of(colour), ForgedShape::of(shape));
        } else {
            sprites::shot(&mut mark, (12, 12), (shot.vx, shot.vy), shot.kind);
            form = ShotForm::Kind(shot.kind);
        }
        if shot.traits.boomerang {
            wake(&mut mark, (shot.vx, shot.vy), shot.traits.back == 2);
        }
        let lent = (shot.traits.borrowed && !shot.hostile).then(|| owned(&mark, u32::from(shot.traits.owner)));
        let base = lent.as_ref().unwrap_or(&mark);
        let mark = match shot.tint() {
            Some(tint) => {
                let mut im = tinted(base, tint);
                brand_trail(&mut im, (shot.vx, shot.vy), tint, run.tick);
                im
            }
            None => base.clone(),
        };
        if apart {
            let (fx, fy) = pixel_fraction(shot.x, shot.y);
            let per_second = UNIT;
            let light = match (shot.kind, hue) {
                (Shot::Sacred, _) => room_scene::Hue::Holy,
                (_, fx::Hue::Warm) => room_scene::Hue::Fire,
                (_, fx::Hue::Gas) => room_scene::Hue::Gas,
                (_, fx::Hue::Cold) => room_scene::Hue::Cold,
            };
            let bright = song
                || !matches!(
                    shot.kind,
                    Shot::Blade | Shot::Arrow | Shot::Bolt | Shot::Bone | Shot::Ore | Shot::Page
                );
            let scene_shot = room_scene::SceneShot {
                x: x as f32 + fx,
                y: y as f32 + fy,
                vx: shot.vx * per_second,
                vy: shot.vy * per_second,
                hue: light,
                bright,
                hostile: shot.hostile,
                empowered: shot.empowered,
                form,
                dimension: dimensions.as_ref().map_or_else(Default::default, |d| d.shot(shot)),
                mark: mark.clone(),
                paint: painted,
                scale: size,
            };
            with_capture(|capture| capture.shots.push(scene_shot));
            continue;
        }
        if size == 1.0 {
            cv.stamp_twice(&mark, x - 24, y - 24);
        } else {
            let k = 2.0 * size;
            let drawn = resized(&mark, k);
            let anchor = (12.5 * k).round() as i32;
            cv.stamp(&drawn, x + 1 - anchor, y + 1 - anchor);
        }
        if let Some((im, (dx, dy))) = fine {
            let im = match shot.tint() {
                Some(tint) => tinted(&im, tint),
                None => im,
            };
            cv.stamp(&im, x + dx, y + dy);
        }
    }
    for shot in run.projectiles.iter().filter(|p| p.hostile && !p.traits.turned && p.in_flight()) {
        let (x, y) = at(shot.x, shot.y);
        danger(cv, x, y, run.tick);
    }
}

pub(super) fn wake(mark: &mut Img, (vx, vy): (f32, f32), home: bool) {
    let length = vx.hypot(vy).max(1e-3);
    let (ux, uy) = (vx / length, vy / length);
    let side = if home { -1.0 } else { 1.0 };
    let (px, py) = (-uy * side, ux * side);
    for k in 3..=9 {
        let k = k as f32;
        let bend = k * k / 14.0;
        let ink = if k < 6.0 { 'h' } else { 'G' };
        mark.put(
            12 - (ux * k).round() as i32 + (px * bend).round() as i32,
            12 - (uy * k).round() as i32 + (py * bend).round() as i32,
            ink,
        );
    }
}

fn brand_trail(mark: &mut Img, (vx, vy): (f32, f32), tint: Tint, tick: u64) {
    let length = vx.hypot(vy).max(1e-3);
    let (ux, uy) = (vx / length, vy / length);
    let (px, py) = (-uy, ux);
    let inks: [char; 3] = match tint {
        Tint::Burn => ['c', '@', '7'],
        Tint::Chill => ['w', '3', '1'],
        Tint::Poison => ['Y', 'C', 'y'],
    };
    for k in 0..5 {
        let back = 4.0 + k as f32 * 1.6;
        let sway = match (tick as i32 / 2 + k) % 3 {
            0 => -1.0,
            1 => 0.0,
            _ => 1.0,
        };
        let drop = if tint == Tint::Poison { k as f32 * 0.6 } else { 0.0 };
        mark.put(
            12 - (ux * back).round() as i32 + (px * sway).round() as i32,
            12 - (uy * back).round() as i32 + (py * sway + drop).round() as i32,
            inks[(k as usize).min(2)],
        );
    }
}

pub(super) fn tinted(im: &Img, tint: Tint) -> Img {
    let ramp: &[char] = match tint {
        Tint::Burn => &['8', '7', 'o', '@', '6', 'c'],
        Tint::Chill => &['0', '1', '2', '3', 'w', '9'],
        Tint::Poison => &['D', 'E', 'y', 'C', 'Y', 'A'],
    };
    ramped(im, ramp)
}

pub(super) fn owned(im: &Img, owner: u32) -> Img {
    let cloth = |from: char| {
        super::knights::seat_colours(owner).iter().find(|(f, _)| *f == from).map_or(from, |&(_, to)| to)
    };
    let (dark, main, light) = (cloth('8'), cloth('7'), cloth('5'));
    ramped(im, &[dark, dark, main, main, light, light])
}

fn ramped(im: &Img, ramp: &[char]) -> Img {
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                let i = (l as usize * ramp.len() / 256).min(ramp.len() - 1);
                out.put(x, y, ramp[i]);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__bullets__tests.rs"]
mod size_tests;

#[cfg(test)]
#[path = "../../../../tests/render/arena__ranged_shots.rs"]
mod ranged_shots;

pub(super) fn danger(cv: &mut Img, x: i32, y: i32, tick: u64) {
    let ring = if (tick / 4) % 2 == 0 { '%' } else { ';' };
    for dy in -4i32..=4 {
        for dx in -4i32..=4 {
            let d2 = dx * dx + dy * dy;
            let ink = match d2 {
                0..=1 => 'w',
                2..=4 => '+',
                5..=10 => ring,
                11..=18 => 'k',
                _ => continue,
            };
            cv.put(x + dx, y + dy, ink);
        }
    }
}
