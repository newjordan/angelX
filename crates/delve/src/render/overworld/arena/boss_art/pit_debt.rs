//! The overseer's outgoing lashes and the collector's committed floor attack.
use super::*;
use crate::rules::patterns::Beam;
use crate::rules::tide::{BAND, UP, WARN};
use crate::rules::{Projectile, Tile, tide::Ravage};
use std::f32::consts::TAU;

pub(super) fn foreman_warning(cv: &mut Img, run: &Run, enemy: &Enemy) -> bool {
    let Some(t) = enemy.ring_warning() else {
        return false;
    };
    let Some((tx, ty)) = crate::rules::press::warning_target(run, enemy) else {
        return true;
    };
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let stage = ((t * 4.0) as usize).min(3);
    let (x, y) = at(enemy.x, enemy.y);
    let kit = effects(Art::Foreman);
    let ink = kit.ring_segments[3][0].get(7, 2).unwrap_or([131, 166, 180]);
    let boss = &run.bosses[enemy.boss.unwrap() as usize];
    let preview = crate::rules::desperate::ring_preview(enemy, boss, aim);
    if stage == 3 {
        let rays: Vec<_> = preview.iter().filter(|(_, primary)| *primary).map(|(a, _)| *a).collect();
        let step = rays.windows(2).map(|p| p[1] - p[0]).min_by(f32::total_cmp).unwrap_or(0.0);
        let [core, edge] = [[87, 124, 144], [131, 166, 180]]
            .map(|ink| if crate::settings::get().art_v2 { effect_exposure(ink) } else { ink });
        for i in 0..rays.len() {
            let (a, b) = (rays[i], rays[(i + 1) % rays.len()]);
            if (b - a).rem_euclid(TAU) > step * 1.01 { continue; }
            if [a, b].iter().any(|angle| crate::rules::patterns::warning_reach(run, (enemy.x, enemy.y), *angle) * super::super::UNIT < 45.0) { continue; }
            let point = |angle: f32, radius: f32| {
                (x + (angle.cos() * radius).round() as i32,
                 y + (angle.sin() * radius).round() as i32)
            };
            // Join the same 38px rage directions, leaving their real safe gap.
            ring_band(cv, run, [point(a, 34.0), point(a, 42.0), point(b, 42.0), point(b, 34.0)], core);
            ring_band(cv, run, [point(a, 40.0), point(a, 42.0), point(b, 42.0), point(b, 40.0)], edge);
        }
    }
    for (angle, primary) in preview {
        let reach = crate::rules::patterns::warning_reach(run, (enemy.x, enemy.y), angle)
            * super::super::UNIT;
        let radius = 38.0f32.min(reach - 6.0);
        if radius <= 12.0 {
            continue;
        }
        let (ux, uy) = (angle.cos(), angle.sin());
        let point = |r: f32| (x + (ux * r).round() as i32, y + (uy * r).round() as i32);
        let length = [1.0, 3.0, 6.0, 10.0][stage];
        ink_line(cv, point(radius - 4.0 - length), point(radius - 4.0), ink);
        if primary && stage < 3 {
            let direction = (angle.rem_euclid(TAU) * 72.0 / TAU).round() as usize % 72;
            let (px, py) = point(radius);
            centered(cv, &kit.ring_segments[stage][direction], px, py);
        }
    }
    true
}

struct Pixel {
    x: i32,
    y: i32,
    foot: (i32, i32),
    colour: [u8; 3],
}

struct Annulus {
    warning: [Vec<Pixel>; 4],
    burst: [Vec<Pixel>; 5],
}

/// Geometry is baked once for each of Ravage's existing eight radii. A new
/// cast only supplies its anchor and two integer lane edges; rotated casts
/// do not allocate another set of full-room canvases.
fn annuli() -> &'static [Annulus; 8] {
    static RINGS: [OnceLock<[Annulus; 8]>; 2] = [OnceLock::new(), OnceLock::new()];
    let modern = crate::settings::get().art_v2;
    RINGS[usize::from(modern)].get_or_init(|| {
        let kit = effects(Art::Leviathan);
        std::array::from_fn(|k| {
            let radius = crate::rules::tide::RING_GAP * (k + 1) as f32 * super::super::UNIT;
            let band = BAND * super::super::UNIT;
            let extent = (radius + band).ceil() as i32;
            let mut warning: [Vec<Pixel>; 4] = std::array::from_fn(|_| Vec::new());
            for dy in -extent..=extent {
                for dx in -extent..=extent {
                    let d = (dx as f32).hypot(dy as f32);
                    if (d - radius).abs() > band {
                        continue;
                    }
                    let arc =
                        ((dy as f32).atan2(dx as f32).rem_euclid(TAU) * radius).floor() as i32;
                    let along = arc.rem_euclid(40);
                    let across = ((d - radius + band) * 10.0 / (2.0 * band)).round() as i32;
                    let boundary = (d - radius).abs() > band - 1.2;
                    for stage in 0..4 {
                        let window = [4, 10, 20, 40][stage];
                        let motif = if along < window {
                            kit.ring_glyph[stage].get(along * 9 / window, across * 8 / 10)
                        } else {
                            None
                        };
                        let colour = if stage == 3 {
                            // Two clean radial bands; the final annulus has no tiled flecks.
                            Some({
                                let ink = if d >= radius { [63, 156, 140] } else { [43, 111, 102] };
                                if modern { effect_exposure(ink) } else { ink }
                            })
                        } else if boundary && along < [2, 4, 8][stage] {
                            Some(if modern { effect_exposure([80, 163, 144]) } else { [80, 163, 144] })
                        } else {
                            motif
                        };
                        if let Some(colour) = colour {
                            warning[stage].push(Pixel {
                                x: dx,
                                y: dy,
                                foot: (dx, dy),
                                colour,
                            });
                        }
                    }
                }
            }
            let burst = std::array::from_fn(|stage| {
                let mut points = Vec::new();
                let im = &kit.impact[stage];
                let n = ((TAU * radius / super::super::UNIT / 1.5).ceil() as usize).max(6);
                for i in 0..n {
                    let a = (i as f32 + 0.5 * (k % 2) as f32) / n as f32 * TAU;
                    let (dx, dy) = (
                        (a.cos() * radius).round() as i32,
                        (a.sin() * radius).round() as i32,
                    );
                    for y in 0..im.h {
                        for x in 0..im.w {
                            if let Some(colour) = im.get(x, y) {
                                points.push(Pixel {
                                    x: dx + x - 14,
                                    y: dy + y - 26,
                                    foot: (dx, dy),
                                    colour,
                                });
                            }
                        }
                    }
                }
                points
            });
            Annulus { warning, burst }
        })
    })
}

struct Lane {
    lower: (i64, i64),
    upper: (i64, i64),
}

impl Lane {
    fn of(ravage: &Ravage) -> Self {
        let edge = |a: f32| {
            (
                (a.cos() * 1_048_576.0).round() as i64,
                (a.sin() * 1_048_576.0).round() as i64,
            )
        };
        Self {
            lower: edge(ravage.lane - ravage.width / 2.0),
            upper: edge(ravage.lane + ravage.width / 2.0),
        }
    }

    fn contains(&self, (x, y): (i32, i32)) -> bool {
        let (x, y) = (i64::from(x), i64::from(y));
        self.lower.0 * y - self.lower.1 * x >= 0 && x * self.upper.1 - y * self.upper.0 >= 0
    }
}

fn stamp(cv: &mut Img, run: &Run, points: &[Pixel], anchor: (i32, i32), lane: &Lane) {
    for p in points {
        if lane.contains(p.foot) {
            continue;
        }
        let (x, y) = (anchor.0 + p.x, anchor.1 + p.y);
        if !matches!(
            run.room().tile(
                x.div_euclid(super::super::TILE),
                y.div_euclid(super::super::TILE)
            ),
            Tile::Wall | Tile::Block
        ) {
            cv.set(x, y, p.colour);
        }
    }
}

pub(in super::super) fn ravage(cv: &mut Img, run: &Run, ravage: &Ravage) -> bool {
    if ravage.boss_art != Some(Art::Leviathan) {
        return false;
    }
    let anchor = at(ravage.x, ravage.y);
    let lane = Lane::of(ravage);
    for (k, ring) in annuli().iter().enumerate().take(usize::from(ravage.rings)) {
        let bursts = ravage.ring(k as u8).1;
        if ravage.age < bursts {
            // All annuli are committed at cast and charge together; each
            // still bursts on its own original beat.
            let stage = (ravage.age * 4 / WARN).min(3) as usize;
            stamp(cv, run, &ring.warning[stage], anchor, &lane);
        } else if ravage.age < bursts + UP {
            let stage = ((ravage.age - bursts) * 5 / UP).min(4) as usize;
            stamp(cv, run, &ring.burst[stage], anchor, &lane);
        }
    }
    true
}

/// The Laser retains its own 21-tick warning and exact committed reach.
/// Its danger corridor uses BEAM_HALF, with no invented destination circle.
pub(in super::super) fn leviathan_laser(cv: &mut Img, _run: &Run, shot: &Projectile, beam: Beam) {
    let kit = effects(Art::Leviathan);
    let from = at(shot.x, shot.y);
    let length = (beam.reach * super::super::UNIT) as i32;
    let (ux, uy) = (beam.angle.cos(), beam.angle.sin());
    let to = (
        from.0 + (ux * length as f32).round() as i32,
        from.1 + (uy * length as f32).round() as i32,
    );
    let sideways = if ux.abs() > uy.abs() { (0, 1) } else { (1, 0) };
    let half = (crate::rules::patterns::BEAM_HALF * super::super::UNIT).ceil() as i32;
    let stage = (usize::from(shot.traits.path.age) * 4 / usize::from(beam.warn.max(1))).min(3);
    let burning = beam.burning(shot.traits.path.age);
    let inks = [[131, 186, 161], [56, 126, 118], [38, 88, 89], [80, 163, 144]]
        .map(|ink| if crate::settings::get().art_v2 { effect_exposure(ink) } else { ink });
    let (mut x, mut y) = from;
    let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
    let (sx, sy) = ((to.0 - x).signum(), (to.1 - y).signum());
    let mut error = dx + dy;
    while (x, y) != to {
        let along = (x - from.0).abs().max((y - from.1).abs());
        for side in -half..=half {
            let window = [4, 10, 20, 40][stage];
            let phase = along.rem_euclid(40);
            let motif = if phase < window {
                kit.ring_glyph[stage].get(phase * 8 / window, (side + half) * 8 / (2 * half))
            } else {
                None
            };
            let colour = if burning {
                Some(if side.abs() < 2 {
                    inks[0]
                } else {
                    inks[1]
                })
            } else if stage == 3 {
                motif.or(Some(inks[2]))
            } else if side.abs() == half && phase < [2, 4, 8][stage] {
                Some(inks[3])
            } else {
                motif
            };
            if let Some(c) = colour {
                cv.set(x + sideways.0 * side, y + sideways.1 * side, c);
            }
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hostile(art: Art) -> Projectile {
        let mut shot: Projectile = serde_json::from_value(serde_json::json!({
            "x":24.0,"y":8.0,"vx":0.0,"vy":8.0,"hostile":true,
            "kind":"ore","damage":11,"pierce":0,"last_hit":null,"ttl":200
        }))
        .unwrap();
        shot.look = art.look();
        shot
    }

    #[test]
    fn red_pick_warns_its_braking_point_and_rejects_a_stone_blocked_landing() {
        let mut run = super::super::super::art_v2::testing::boss("red-pick-chief");
        let mut mine = hostile(Art::RedPick);
        mine.traits.path.accel = -8.0;
        mine.traits.path.fuse = 51;
        mine.traits.path.then = crate::rules::patterns::Then::Burst(6, 4.0);
        let landing = mine_landing(&mine).unwrap();
        assert!(landing.1 > 11.0 && landing.1 < 12.0);
        run.projectiles = vec![mine];
        for y in 4..=6 {
            run.dungeon.rooms[run.at].set(12, y, Tile::Floor);
        }
        let blank = Img::black(384, 224);
        for age in [0, 13, 26, 39] {
            run.projectiles[0].traits.path.age = age;
            let predicted = mine_landing(&run.projectiles[0]).unwrap();
            let (x, y) = at(predicted.0, predicted.1);
            let mut warning = blank.clone();
            landings(&mut warning, &run);
            assert_ne!(warning.content_hash(), blank.content_hash());
            for py in 0..warning.h {
                for px in 0..warning.w {
                    if warning.get(px, py) != Some([0, 0, 0]) {
                        assert!((px - x).pow(2) + (py - y).pow(2) <= 25);
                    }
                }
            }
        }
        run.dungeon.rooms[run.at].set(12, 5, Tile::Block);
        let mut blocked = blank.clone();
        landings(&mut blocked, &run);
        assert_eq!(blocked.content_hash(), blank.content_hash());
    }

    #[test]
    fn foremans_unmarked_gap_wall_follower_has_no_landing_tell() {
        let mut run = super::super::super::art_v2::testing::boss("the-foreman");
        let mut follower = hostile(Art::Foreman);
        follower.traits.path.wait = 24;
        follower.traits.path.mark = false;
        run.projectiles = vec![follower];
        let blank = Img::black(384, 224);
        let mut warning = blank.clone();
        landings(&mut warning, &run);
        assert_eq!(warning.content_hash(), blank.content_hash());
    }

    #[test]
    fn leviathan_laser_keeps_its_committed_width_reach_and_warning_ticks() {
        let run = super::super::super::art_v2::testing::boss("late-fee-leviathan");
        let mut shot = hostile(Art::Leviathan);
        let beam = Beam {
            angle: std::f32::consts::FRAC_PI_2,
            reach: 10.0,
            warn: 21,
            burn: 18,
            ..Default::default()
        };
        assert_eq!(beam.warn, crate::rules::patterns::BEAM_WARN);
        for age in [0, 6, 11, 16, 20, 21] {
            shot.traits.path.age = age;
            let mut cv = Img::black(384, 224);
            leviathan_laser(&mut cv, &run, &shot, beam);
            for y in 0..cv.h {
                for x in 0..cv.w {
                    if cv.get(x, y) != Some([0, 0, 0]) {
                        assert!((189..=195).contains(&x));
                        assert!((64..144).contains(&y));
                    }
                }
            }
        }
    }

    #[test]
    fn ravage_art_covers_only_its_committed_annuli_and_clear_lane() {
        let mut cast = Ravage::cast((24.0, 14.0), 5, 50.0, 8.0, 16, 17);
        cast.boss_art = Some(Art::Leviathan);
        let lane = Lane::of(&cast);
        for (k, ring) in annuli().iter().enumerate().take(usize::from(cast.rings)) {
            let radius = cast.ring(k as u8).0 * super::super::super::UNIT;
            for stage in &ring.warning {
                for p in stage {
                    if lane.contains(p.foot) {
                        continue;
                    }
                    let (dx, dy) = (p.x as f32, p.y as f32);
                    assert!((dx.hypot(dy) - radius).abs() <= BAND * super::super::super::UNIT);
                    assert!(!cast.in_lane(
                        cast.x + dx / super::super::super::UNIT,
                        cast.y + dy / super::super::super::UNIT
                    ));
                }
            }
        }
        assert_eq!(cast.ring(0).1, 36);
        assert_eq!(cast.ring(4).0, 11.0);
    }

    #[test]
    fn old_ravage_wire_defaults_to_plain_and_new_appearance_is_independent() {
        let old = Ravage::cast((10.0, 12.0), 5, 50.0, 8.0, 16, 17);
        let json = serde_json::to_value(old).unwrap();
        assert!(json.get("boss_art").is_none());
        let mut painted: Ravage = serde_json::from_value(json).unwrap();
        painted.boss_art = Some(Art::Leviathan);
        for k in 0..5 {
            assert_eq!(old.ring(k), painted.ring(k));
        }
        assert_eq!(
            painted,
            serde_json::from_slice(&serde_json::to_vec(&painted).unwrap()).unwrap()
        );
    }
}
