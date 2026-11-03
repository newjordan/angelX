//! The room's hazards: pit, pool and lava marked on the floor, the traps
//! and vents that strike live, and the kegs and bombs with the ring where
//! each will land.

use super::*;

/// Whether native pixel `(x, y)` lies over a hazard tile.
fn hazard_at(room: &Room, x: i32, y: i32) -> bool {
    room.tile(x.div_euclid(TILE), y.div_euclid(TILE)) == Tile::Hazard
}

/// Pit, pool or lava at a native pixel, with the overworld's marks.
pub(super) fn hazard_mark(pack: Pack, room: &Room, x: i32, y: i32, t: i32) -> Option<char> {
    let mut shore = i32::MAX;
    for k in 1..=3 {
        if !hazard_at(room, x - k, y)
            || !hazard_at(room, x + k, y)
            || !hazard_at(room, x, y - k)
            || !hazard_at(room, x, y + k)
        {
            shore = k;
            break;
        }
    }
    match pack {
        // A pit: a broken stone lip, and under the near lip its far wall
        // going down out of the light into nothing.
        Pack::Crypt => {
            // The corners are broken off the lip.
            let side = !hazard_at(room, x - 1, y) || !hazard_at(room, x + 1, y);
            if let Some(k) = (1..=6).find(|&k| !hazard_at(room, x, y - k)) {
                if k == 1 && side {
                    return None;
                }
                return match k {
                    1 => (!hash(x, y, 91).is_multiple_of(5)).then_some('u'),
                    2 => Some('S'),
                    3 => Some('S'),
                    4 => Some('x'),
                    5 => (hash(x / 2, y, 93) % 3 != 0).then_some('x'),
                    _ => (hash(x / 2, y, 94) % 3 == 0).then_some('s'),
                };
            }
            match shore {
                1 => (!hash(x, y, 91).is_multiple_of(4)).then_some('S'),
                2 => (hash(x / 2, y / 2, 92).is_multiple_of(3)).then_some('x'),
                _ => None,
            }
        }
        // Standing water is open water, drawn whole by `floor_marks`.
        Pack::Cavern | Pack::Archive | Pack::Swamp => None,
        // Spore pools: a glowing green shimmer that drifts.
        Pack::Fungal => {
            if shore == 1 {
                return Some('E');
            }
            let glow = vnoise((x + t / 3) as f32 / 5.0, y as f32 / 4.0, 452);
            (glow > 0.45).then_some(if glow > 0.7 {
                'Y'
            } else if glow > 0.58 {
                'y'
            } else {
                'l'
            })
        }
        // The void: nothing below, now and then a far light.
        Pack::Unknown => {
            if shore == 1 {
                return (!hash(x, y, 98).is_multiple_of(3)).then_some('0');
            }
            (hash(x, y + t / 8, 99).is_multiple_of(300)).then_some('3')
        }
        // Lava: dark crust creeping east, the melt glowing through.
        Pack::Hellforge => {
            if shore == 1 {
                return Some('n');
            }
            let crust = vnoise((x - t / 2) as f32 / 6.0, y as f32 / 3.0, 451);
            Some(if crust > 0.62 {
                'n'
            } else if crust > 0.5 {
                'b'
            } else if crust > 0.38 {
                'B'
            } else if crust > 0.24 {
                'p'
            } else if crust > 0.14 {
                'R'
            } else {
                'o'
            })
        }
    }
}

/// A vent grille's top-left on its wall.
pub(super) fn vent_at(room: &Room, side: usize, lane: f32) -> (i32, i32) {
    let l = (lane * UNIT) as i32;
    let (pw, ph) = room_px(room);
    match side {
        1 => (pw - 8, l - 5),
        3 => (2, l - 5),
        0 => (l - 3, 2),
        _ => (l - 3, ph - 12),
    }
}

/// Live hazards over the room: rattling and striking spikes, glowing vents,
/// a doorway about to pour in a wave, rocks coming down.
pub(super) fn hazards(cv: &mut Img, run: &Run) {
    if atari::here(run) {
        // On the Atari board what springs from the floor is blocky, in the
        // first hue.
        let (x, y, w, h) = cv.span();
        let mut layer = Img::window(cv.w, cv.h, (x, y, w, h));
        hazards_drawn(&mut layer, run);
        cv.stamp(&atari::sprite_in(&layer, atari::FOE, (0, 0)), 0, 0);
        return;
    }
    hazards_drawn(cv, run);
}

fn hazards_drawn(cv: &mut Img, run: &Run) {
    use crate::rules::hazards::{self as hz, Spikes};
    let tick = run.tick as u32;
    for trap in &run.traps {
        match *trap {
            Trap::Spikes { col, row, phase } => match hz::spikes(run.tick, phase) {
                Spikes::Down => {}
                state => cv.stamp(
                    &match super::demonic::style(run.dungeon.pack) {
                        Some(style) => super::demonic::spikes_out(style, state == Spikes::Up, tick),
                        None => sprites::spikes_out(state == Spikes::Up, tick),
                    },
                    col * TILE,
                    row * TILE,
                ),
            },
            Trap::Vent { side, lane, phase } => {
                let beat = hz::vent_beat(run.tick, phase);
                let glow = if beat >= hz::VENT_PERIOD - hz::VENT_GLOW {
                    beat + hz::VENT_GLOW + 1 - hz::VENT_PERIOD
                } else if beat < 18 {
                    hz::VENT_GLOW
                } else {
                    0
                };
                if glow > 0 {
                    let (x, y) = vent_at(run.room(), side, lane);
                    cv.stamp(&sprites::vent(glow), x, y);
                }
            }
            Trap::Rockfall => {}
        }
    }
    if let Some((side, _)) = run.waves.gate {
        // The doorway burns: something is coming through.
        let (col, row) = doors(run.room())[side];
        let (w, h) = if side % 2 == 0 {
            (2 * TILE, TILE)
        } else {
            (TILE, 2 * TILE)
        };
        for y in 0..h {
            for x in 0..w {
                let n = super::super::ink::hash(x + col * TILE, y + row * TILE, tick / 3);
                if n.is_multiple_of(3) {
                    cv.put(
                        col * TILE + x,
                        row * TILE + y,
                        if n.is_multiple_of(7) {
                            '6'
                        } else if n.is_multiple_of(2) {
                            '@'
                        } else {
                            '7'
                        },
                    );
                }
            }
        }
    }
    for rock in &run.rocks {
        let (x, y) = at(rock.x, rock.y);
        // Hell's embers: a coal for the stone, and cinders for its dust.
        let stone = if rock.ember { sprites::ember(tick) } else { sprites::rock() };
        if rock.fall > 0 {
            let (ring, height) = sprites::falling_rock(rock.fall, hz::ROCK_FALL);
            cv.stamp(&ring, x - ring.w / 2, y - ring.h / 2);
            cv.stamp(&stone, x - stone.w / 2, y - stone.h - height);
        } else if rock.ember {
            let puff = sprites::cinders(rock.dust);
            cv.stamp(&puff, x - puff.w / 2, y - puff.h / 2);
        } else {
            cv.stamp(&stone, x - stone.w / 2, y - stone.h / 2);
            let puff = sprites::dust(rock.dust);
            cv.stamp(&puff, x - puff.w / 2, y - puff.h / 2);
        }
    }
}

/// Kegs on the floor and bombs in the air, with the ring where each will go.
pub(super) fn foes_hazards(cv: &mut Img, run: &Run) {
    use crate::rules::foes::{FLING_FALL, KEG_ARMED, LOB_FALL, WELL_REACH};
    use crate::rules::hunters::{HOOK_RANGE, WEB_REACH};
    let tick = run.tick as u32;
    // Webs: spokes and rings of silk, thinning as they fade.
    for web in &run.webs {
        let (x, y) = at(web.x, web.y);
        let r = WEB_REACH * 8.0;
        let thin = if web.left < 30 { 2 } else { 1 };
        for spoke in 0..8 {
            let a = spoke as f32 * std::f32::consts::FRAC_PI_4 + 0.2;
            for k in (2..r as i32).step_by(2 * thin) {
                let k = k as f32;
                cv.put(
                    x + (a.cos() * k) as i32,
                    y + (a.sin() * k * 0.6) as i32,
                    'J',
                );
            }
        }
        for ring in [0.35f32, 0.65, 0.95] {
            let rr = r * ring;
            let n = (rr * 2.0) as i32;
            for i in (0..n).step_by(2 * thin) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                cv.put(
                    x + (a.cos() * rr) as i32,
                    y + (a.sin() * rr * 0.6) as i32,
                    'h',
                );
            }
        }
    }
    // The Pit Tyrant's slam gathering: a red ring, filling in as it comes.
    for slam in &run.slams {
        use crate::rules::pit::{SLAM_REACH, SLAM_WINDUP};
        let (x, y) = at(slam.x, slam.y);
        let k = 1.0 - slam.left as f32 / SLAM_WINDUP as f32;
        let r = SLAM_REACH * 8.0;
        let n = 72;
        let step = if k > 0.66 {
            1
        } else if k > 0.33 {
            2
        } else {
            3
        };
        for i in (0..n).step_by(step) {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            for rr in [r, r - 1.0] {
                cv.put(
                    x + (a.cos() * rr) as i32,
                    y + (a.sin() * rr * 0.6) as i32,
                    '7',
                );
            }
        }
    }
    // The Flesher's stare: his hook's line, red on the floor.
    for flesher in run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Flesher && e.stage == 1 && e.hp > 0)
    {
        let (x, y) = at(flesher.x, flesher.y);
        let reach = HOOK_RANGE * 8.0;
        // Two rows of marching dashes, either side of the hook's path.
        let (nx, ny) = (-flesher.dir.1, flesher.dir.0);
        for k in 10..reach as i32 {
            if (k as u32 + tick).rem_euclid(5) > 2 {
                continue;
            }
            let k = k as f32;
            for side in [-1.5f32, 1.5] {
                cv.put(
                    x + (flesher.dir.0 * k + nx * side) as i32,
                    y + (flesher.dir.1 * k + ny * side) as i32,
                    '7',
                );
            }
        }
    }
    for keg in &run.kegs {
        let (x, y) = at(keg.x, keg.y);
        let im = sprites::keg(keg.fuse, keg.fuse < KEG_ARMED, tick);
        cv.stamp(&im, x - im.w / 2, y - im.h / 2);
        if keg.fuse < KEG_ARMED {
            let r = 22.0;
            let n = 60;
            for i in (0..n).step_by(3) {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                cv.put(
                    x + (a.cos() * r) as i32,
                    y + (a.sin() * r * 0.6) as i32,
                    '8',
                );
            }
        }
    }
    for lob in &run.lobs {
        let (tx, ty) = at(lob.x, lob.y);
        let k = 1.0 - lob.fall as f32 / LOB_FALL as f32;
        let r = 19.0;
        let n = 56;
        for i in (0..n).step_by(2) {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            cv.put(
                tx + (a.cos() * r) as i32,
                ty + (a.sin() * r * 0.6) as i32,
                '7',
            );
        }
        let (fx, fy) = at(lob.from.0, lob.from.1);
        let arc = (k * std::f32::consts::PI).sin() * 26.0;
        let (bx, by) = (
            fx + ((tx - fx) as f32 * k) as i32,
            fy + ((ty - fy) as f32 * k) as i32 - arc as i32,
        );
        if !lifted_outside() {
            let im = sprites::bomb(tick);
            cv.stamp(&im, bx - im.w / 2, by - im.h / 2);
        }
    }
    for well in &run.wells {
        let (wx, wy) = at(well.x, well.y);
        // The lean's reach: a fading ring, and grit turning in the bend.
        let reach = (WELL_REACH * UNIT * 0.9) as i32;
        for i in 0..64 {
            let a = i as f32 / 64.0 * std::f32::consts::TAU;
            let r = reach as f32 * (0.75 + 0.25 * (a * 3.0 + run.tick as f32 * 0.05).sin());
            cv.put(
                wx + (a.cos() * r) as i32,
                wy + (a.sin() * r * 0.7) as i32,
                if i % 8 == 0 { 'u' } else { 'x' },
            );
        }
        for arm in 0..3 {
            let a = run.tick as f32 * 0.11 + arm as f32 * (std::f32::consts::TAU / 3.0);
            for k in 3..8 {
                let r = reach as f32 * k as f32 / 10.0;
                cv.put(
                    wx + (a.cos() * r) as i32,
                    wy + (a.sin() * r * 0.7) as i32,
                    'U',
                );
            }
        }
    }
    for fling in &run.flings {
        let (tx, ty) = at(fling.x, fling.y);
        let r = 17.0;
        let n = 48;
        for i in (0..n).step_by(3) {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            cv.put(tx + (a.cos() * r) as i32, ty + (a.sin() * r * 0.6) as i32, '8');
        }
        let (fx, fy) = at(fling.from.0, fling.from.1);
        let k = 1.0 - fling.fall as f32 / FLING_FALL as f32;
        let arc = (k * std::f32::consts::PI).sin() * 20.0;
        let (bx, by) = (
            fx + ((tx - fx) as f32 * k) as i32,
            fy + ((ty - fy) as f32 * k) as i32 - arc as i32,
        );
        if !lifted_outside() {
            let im = sprites::flung_impling(tick);
            cv.stamp(&im, bx - im.w / 2, by - im.h / 2);
        }
    }
}
