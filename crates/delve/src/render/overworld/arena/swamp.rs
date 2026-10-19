//! The Swamp of Horrors in the arena: the horrors' tells on the floor, the
//! fog's local dark, the Mire King's pen with the white horse in it, the
//! plate and the notice post in Sir Damp's Marsh, and the white horse at
//! grass in the Flower Fields.
//!
//! Every picture comes from `swamp_art` (the art team's module); this one
//! only places them, and draws the tells from the monsters' own state
//! (`Enemy::stage`, `timer`, `dir`), so every seat shows the same warning
//! for exactly the ticks the monster winds up.

use super::super::ink::{BLACK, Img, bayer, is_signal};
use super::super::light::step_down;
use super::{TILE, UNIT, at, home, sprites, stand, swamp_art};
use crate::rules::rescues::Captive;
use crate::rules::{Enemy, EnemyKind, RoomKind, Run, TILE_UNITS, horrors, swamp};

/// A horror's drawing in its state, undressed (`swamp_art`).
pub(super) fn raw(kind: EnemyKind, stage: u8, t: u32, tick: u32) -> Img {
    match kind {
        EnemyKind::BogHand => swamp_art::bog_hand(stage, t, tick),
        EnemyKind::Drowned => swamp_art::drowned(stage, t, tick),
        _ => swamp_art::wisp(stage, t, tick),
    }
}

/// A horror as it stands now, dressed and kept as every foe's picture is,
/// facing the way it means to go.
pub(super) fn figure(run: &Run, enemy: &Enemy, tick: u32) -> Img {
    let (kind, stage, t) = (enemy.kind, enemy.stage, enemy.timer);
    let im = super::foe_material::pose(
        192 + kind as u8,
        [tick % 48, u32::from(stage), (t / 8).min(15), 0],
        || super::foe_material::dress(&raw(kind, stage, t, tick), kind),
    );
    let way = if enemy.kind == EnemyKind::Drowned && enemy.dir.0 != 0.0 {
        enemy.dir.0
    } else {
        run.players
            .values()
            .filter(|h| h.hp > 0 && !h.stone)
            .map(|h| h.x - enemy.x)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(1.0)
    };
    if way < 0.0 { im.flip_h() } else { im }
}

fn ring(cv: &mut Img, (x, y): (i32, i32), r: f32, every: usize, ink: char) {
    let n = ((r * 0.9) as usize).clamp(24, 96);
    for i in (0..n).filter(|i| i % every == 0) {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        cv.put(x + (a.cos() * r) as i32, y + (a.sin() * r * 0.7) as i32, ink);
    }
}

/// What a horror is about to do, drawn on the floor round it.
pub(super) fn marks(cv: &mut Img, enemy: &Enemy, tick: u32) {
    let (x, y) = at(enemy.x, enemy.y);
    let t = enemy.timer;
    match (enemy.kind, enemy.stage) {
        // The ripples where the hand will rise: they close in, and burn
        // hard for the last of the tell.
        (EnemyKind::BogHand, 1) => {
            let k = t as f32 / horrors::HAND_TELL as f32;
            let reach = horrors::HAND_REACH * UNIT;
            let hot = k > 0.7;
            ring(cv, (x, y), reach * (1.3 - 0.3 * k), if hot { 1 } else { 3 }, if hot { '@' } else { '7' });
            let inner = reach * (0.35 + 0.4 * ((tick / 3 % 4) as f32 / 4.0));
            ring(cv, (x, y), inner, 4, 'u');
        }
        // The cleave's arc in front of the lifted blade, filling.
        (EnemyKind::Drowned, 1) => {
            let k = t as f32 / horrors::DROWNED_WINDUP as f32;
            let locked = t >= horrors::DROWNED_LOCK;
            let (dx, dy) = enemy.dir;
            let (cx, cy) = (enemy.x + dx * 1.4, enemy.y + dy * 1.4);
            let reach = horrors::DROWNED_REACH * UNIT * (0.55 + 0.45 * k);
            let (px, py) = at(cx, cy);
            for i in -10..=10 {
                let a = i as f32 * 0.12;
                let (c, s) = (a.cos(), a.sin());
                let (ux, uy) = (dx * c - dy * s, dx * s + dy * c);
                cv.put(
                    px + (ux * reach) as i32,
                    py + (uy * reach * 0.8) as i32,
                    if locked { '@' } else { '7' },
                );
            }
        }
        // The lure: rings drawn in toward the wisp.
        (EnemyKind::Wisp, 1) => {
            let k = t as f32 / horrors::WISP_LURE as f32;
            let reach = horrors::LURE_REACH * UNIT;
            for step in 0..3 {
                let phase = ((tick / 2 + step * 7) % 21) as f32 / 21.0;
                let r = reach * (1.0 - phase) * (1.0 - 0.3 * k);
                if r > 6.0 {
                    ring(cv, (x, y), r, 3, if k > 0.75 { '@' } else { '6' });
                }
            }
        }
        _ => {}
    }
}

/// Whether the room the party stands in has fog in it.
pub(super) fn fogged(run: &Run) -> bool {
    !run.at_home_now() && !patches(run).is_empty()
}

fn patches(run: &Run) -> Vec<(f32, f32)> {
    swamp::fog(run.dungeon.pack, run.room(), run.dungeon.depth)
}

/// The fog: within each patch the room sinks toward black paper, as Lights
/// Out does everywhere, but here only in the patch, and a knight's lantern
/// still lights round them. Signal inks (eyes, fire, shots, the tells)
/// glow through it.
pub(super) fn fog(scenery: &mut Img, over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    const LANTERN: f32 = 40.0;
    let patches = patches(run);
    if patches.is_empty() || run.at_home_now() {
        return;
    }
    let lamps: Vec<(f32, f32)> = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .map(|h| {
            let (x, y) = at(h.x, h.y);
            ((x - ox) as f32, (y - oy - 6) as f32)
        })
        .collect();
    let reach = swamp::FOG_REACH * UNIT;
    let tick = run.tick as u32;
    for &(fx, fy) in &patches {
        let (cx, cy) = at(fx, fy);
        let (cx, cy) = ((cx - ox) as f32, (cy - oy) as f32);
        let (x0, x1) = ((cx - reach) as i32, (cx + reach) as i32);
        let (y0, y1) = ((cy - reach) as i32, (cy + reach) as i32);
        for y in y0.max(0)..y1.min(scenery.h) {
            for x in x0.max(0)..x1.min(scenery.w) {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let thick = 1.0 - (px - cx).hypot((py - cy) / 0.8) / reach;
                if thick <= 0.0 {
                    continue;
                }
                let lit = lamps
                    .iter()
                    .map(|&(lx, ly)| 1.0 - (px - lx).hypot(py - ly) / LANTERN)
                    .fold(0.0f32, f32::max);
                let dark = ((thick.min(0.6) - lit.max(0.0)) * 12.0 + bayer(x, y) * 2.0).max(0.0) as usize;
                if dark == 0 {
                    continue;
                }
                if let Some(c) = scenery.get(x, y)
                    && c != BLACK
                    && !is_signal(c)
                {
                    scenery.set(x, y, step_down(c, dark.min(6)));
                }
                if let Some(c) = over.get(x, y)
                    && c != BLACK
                    && !is_signal(c)
                {
                    over.set(x, y, step_down(c, dark.min(3)));
                }
                if let Some(ink) = swamp_art::mist(x + ox, y + oy, tick) {
                    scenery.put(x, y, ink);
                }
            }
        }
    }
}

/// The Mire King's pen, the white horse in it: frightened while he stands,
/// and once out, walking off east before she goes home.
pub(super) fn pen(cv: &mut Img, run: &Run, captive: &Captive) {
    let (x, y) = at(captive.x, captive.y);
    let tick = run.tick as u32;
    let held = !run.dungeon.rooms[captive.room].cleared;
    let gone = captive
        .freed
        .is_some_and(|at| run.tick.saturating_sub(at) > 75);
    if !gone {
        let walked = captive
            .freed
            .map_or(0, |at| (run.tick.saturating_sub(at) as i32 / 2).min(40));
        let pose = if captive.freed.is_some() {
            swamp_art::HorsePose::Gallop(tick / 4)
        } else if held {
            swamp_art::HorsePose::Frightened
        } else {
            swamp_art::HorsePose::Stand
        };
        stand(cv, &swamp_art::white_horse(pose), x + walked, y);
    }
    let im = swamp_art::pen(captive.freed.is_some(), tick);
    cv.stamp(&im, x - im.w / 2, y - im.h + 6);
    let near = run.players.values().any(|h| {
        h.hp > 0 && (h.x - captive.x).hypot(h.y - captive.y) < crate::rules::rescues::CAGE_REACH + 2.0
    });
    if near && captive.freed.is_none() {
        let last = if held {
            (swamp::PEN_HELD.to_string(), 'h')
        } else {
            ("HOLD E: OPEN THE PEN".to_string(), '5')
        };
        let board = home::board(&[(swamp::HORSE_NAME.to_uppercase(), '9'), last]);
        // Beside the pen, never over the knights at its gate below it.
        let right = x + im.w / 2 + 4;
        let bx = if right + board.w <= cv.w - 2 { right } else { x - im.w / 2 - 4 - board.w };
        cv.stamp(&board, bx.max(2), (y - im.h + 6).clamp(2, cv.h - board.h - 2));
    }
}

/// The plate down into the swamp, and its notice post, in Sir Damp's Marsh.
pub(super) fn marsh(cv: &mut Img) {
    let (c, r, _, _) = swamp::PLATE;
    cv.stamp(&sprites::stairs(true), c * TILE, r * TILE);
    let icon = swamp_art::plaque_icon();
    let (px, py) = at(swamp::NOTICE.0 * TILE_UNITS, swamp::NOTICE.1 * TILE_UNITS);
    cv.line(px, py - 2, px, py + 10, 'n');
    cv.line(px + 1, py - 2, px + 1, py + 10, 'b');
    cv.stamp(&icon, px - icon.w / 2, py - icon.h - 2);
}

/// The notice, to a knight at its post.
pub(super) fn marsh_board(cv: &mut Img, run: &Run) {
    if run.room().kind != RoomKind::Marsh {
        return;
    }
    let (nx, ny) = (swamp::NOTICE.0 * TILE_UNITS, swamp::NOTICE.1 * TILE_UNITS);
    let near = run.players.values().any(|h| {
        h.hp > 0 && (h.x - nx).hypot(h.y - ny) < swamp::NOTICE_REACH * TILE_UNITS + 1.0
    });
    if !near {
        return;
    }
    let board = home::board(&[
        ("THE SWAMP OF HORRORS".into(), '9'),
        ("A WHITE HORSE WAS TAKEN INTO THE SWAMP".into(), 'O'),
        ("THE PLATE GOES DOWN. BRING HER HOME.".into(), 'h'),
        ("E: READ THE NOTICE".into(), 'h'),
    ]);
    cv.stamp(&board, cv.w - board.w - 4, 13);
}

/// The white horse at grass in the Flower Fields, once she is in the herd:
/// where she stands (tiles) and her picture.
pub(super) fn grazing(run: &Run) -> Option<((f32, f32), Img)> {
    if !swamp::in_herd(&run.home.stable) {
        return None;
    }
    let tick = run.tick as u32;
    let t = run.tick as f32 / (crate::rules::HZ as f32 * 11.0) + 1.3;
    let (x, y) = (9.0 + 1.4 * t.sin(), 11.4 + 0.3 * (t * 0.6).cos());
    let pose = if (tick / 150) % 3 == 0 {
        swamp_art::HorsePose::Stand
    } else {
        swamp_art::HorsePose::Graze
    };
    let im = swamp_art::white_horse(pose);
    Some(((x, y), if t.cos() > 0.0 { im } else { im.flip_h() }))
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__swamp_shots.rs"]
mod tests;
