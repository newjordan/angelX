//! Shoulder rides on screen: a tower of two and of the whole party walking
//! and shooting through a fight, and a throw off the top down the room.

use super::super::*;
use crate::rules::{HZ, Input, RoomKind, Run};
use std::collections::BTreeMap;

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + party::BAND_H + party::HUD_H)
}

/// `n` knights in the first fight of the floor, knights 2 to `n` up on
/// knight 1's shoulders, the monsters held a moment so the tower is seen.
fn tower(seed: u64, n: u32) -> Run {
    let mut run = Run::new(seed, 1, None);
    for id in 2..=n {
        run.join_seat(id, ["Wren", "Bors", "Ysolde", "Kay", "Tor"][id as usize - 2]);
    }
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .expect("a fight on floor one");
    run.enter_for_test(index);
    for hero in run.players.values_mut() {
        (hero.idle, hero.stone) = (false, false);
    }
    // The rest stand back in a row, and come up to the tower one by one.
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    for id in 2..=n {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x + 4.0 + id as f32 * 2.4, y);
    }
    for id in 2..=n {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x + 2.0, y);
        run.step(&BTreeMap::from([(id, Input { ride: true, ..Input::default() })]));
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.shoulders.riders(1).len(), n as usize - 1);
    run
}

/// The tower's keys on tick `t`: the carrier walks a slow square, the
/// riders aim at the nearest monster and shoot.
fn keys(run: &Run, t: u32) -> BTreeMap<u32, Input> {
    let carrier = &run.players[&1];
    let near = run.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
        (a.x - carrier.x).hypot(a.y - carrier.y).total_cmp(&(b.x - carrier.x).hypot(b.y - carrier.y))
    });
    let (ax, ay) = near.map_or((1, 0), |e| {
        let (dx, dy) = (e.x - carrier.x, e.y - carrier.y);
        let d = dx.hypot(dy).max(1e-3);
        let s = |v: f32| if v > 0.38 { 1 } else if v < -0.38 { -1 } else { 0 };
        (s(dx / d), s(dy / d))
    });
    let side = (t / 20) % 4;
    let (mx, my) = [(1, 0), (0, -1), (-1, 0), (0, 1)][side as usize];
    run.players
        .keys()
        .map(|&id| {
            let mut i = Input { aim_x: ax, aim_y: ay, fire: true, ..Input::default() };
            if id == 1 {
                (i.move_x, i.move_y) = (mx, my);
            }
            (id, i)
        })
        .collect()
}

fn save(dir: &std::path::Path, name: &str, run: &Run) {
    let (w, h) = seat();
    let img = seat_scene(run, w, h, Some(1)).compose();
    let rgba = img.rgba_scaled(3);
    image::RgbaImage::from_raw(img.w as u32 * 3, img.h as u32 * 3, rgba)
        .unwrap()
        .save(dir.join(format!("{name}.png")))
        .unwrap();
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_shoulder_shots -- --ignored`:
/// frames of a two-high and a six-high tower walking and shooting, and a
/// throw off the top, every other tick for a frame sequence.
#[test]
#[ignore]
fn write_shoulder_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (n, name) in [(2u32, "stack2"), (6, "stack6")] {
        let mut run = tower(23, n);
        for t in 0..80 {
            let k = keys(&run, t);
            run.step(&k);
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            if t % 2 == 0 {
                save(&dir, &format!("{name}-{:03}", t / 2), &run);
            }
        }
    }
    // The throw: the whole party up, the carrier turns and throws the top
    // knight across the room into the monsters.
    let mut run = tower(23, 6);
    let carrier = &run.players[&1];
    let near = run.enemies.iter().filter(|e| e.hp > 0).max_by(|a, b| {
        (a.x - carrier.x).hypot(a.y - carrier.y).total_cmp(&(b.x - carrier.x).hypot(b.y - carrier.y))
    });
    let (dx, dy) = near.map_or((1.0, 0.0), |e| (e.x - carrier.x, e.y - carrier.y));
    let s = |v: f32| if v > 0.38 { 1 } else if v < -0.38 { -1 } else { 0 };
    let d = dx.hypot(dy).max(1e-3);
    let aim = Input { aim_x: s(dx / d), aim_y: s(dy / d), ..Input::default() };
    for t in 0..(HZ + HZ / 2) {
        let mut k = BTreeMap::from([(1, aim)]);
        if t == 4 {
            k.get_mut(&1).unwrap().ride = true;
        }
        run.step(&k);
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        save(&dir, &format!("throw-{t:03}"), &run);
    }
}

/// `ANGEL_SHOULDER_SNAP=<file> cargo test write_shoulder_snapshot -- --ignored`:
/// a dev host's starting run (`delve-dev-host --snapshot`) for the browser
/// proof: six knights in the first fight, knights 2 to 6 in a ring close
/// round knight 1, so each seat can climb on with C alone.
#[test]
#[ignore]
fn write_shoulder_snapshot() {
    let Some(path) = std::env::var_os("ANGEL_SHOULDER_SNAP") else {
        return;
    };
    let mut run = Run::new(23, 1, None);
    for id in 2..=6 {
        run.join_seat(id, ["Wren", "Bors", "Ysolde", "Kay", "Tor"][id as usize - 2]);
    }
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .expect("a fight on floor one");
    run.enter_for_test(index);
    let (x, y) = (run.room().width() * 0.3, run.room().height() * 0.62);
    // Clear floor round the party, and a handful of monsters across the
    // room to shoot at.
    let at = run.at;
    let (c0, r0) = ((x / crate::rules::TILE_UNITS) as i32, (y / crate::rules::TILE_UNITS) as i32);
    for r in r0 - 3..=r0 + 3 {
        for c in c0 - 4..=c0 + 4 {
            let room = &mut run.dungeon.rooms[at];
            if r > 0 && c > 0 && r < room.rows as i32 - 1 && c < room.cols as i32 - 1 {
                room.set(c as usize, r as usize, crate::rules::Tile::Floor);
            }
        }
    }
    run.enemies.truncate(6);
    let (w, h) = (run.room().width(), run.room().height());
    for (i, e) in run.enemies.iter_mut().enumerate() {
        (e.x, e.y) = (w * 0.78, h * (0.25 + 0.1 * i as f32));
        // Held still and hardy: targets for the proof, not a threat.
        (e.hp, e.max_hp, e.frozen) = (4000, 4000, 150 * crate::rules::HZ);
    }
    run.waves = Default::default();
    for (&id, hero) in run.players.iter_mut() {
        let a = (id as f32 - 2.0) / 5.0 * std::f32::consts::TAU;
        (hero.x, hero.y) = if id == 1 { (x, y) } else { (x + 1.9 * a.cos(), y + 1.9 * a.sin()) };
        (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    }
    std::fs::write(path, serde_json::to_vec(&run).unwrap()).unwrap();
}
