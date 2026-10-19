//! The gauntlets on screen: the axe bridge both ways, the hammer row, the
//! boulder run and the saw rail, each lit by its own warm light on the
//! black floor.

use super::*;
use super::super::{NATIVE_H, NATIVE_W, seat_scene};
use crate::rules::{Input, Pack, RoomKind, Run, fortune, layout};
use std::collections::BTreeMap;

/// A run standing in a straight passage of floor `depth` (north to south
/// when `upright`) of `pack`, its gauntlet `kind` laid; three knights.
fn passage(upright: bool, depth: u32, pack: Pack, kind: &str) -> Run {
    let want = if upright { [true, false, true, false] } else { [false, true, false, true] };
    for seed in 0..400u64 {
        let mut run = Run::new(seed, 3, None);
        run.join_seat(2, "Wren");
        run.join_seat(3, "Bors");
        run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, depth, pack, &mut crate::rules::Rng::new(seed ^ 0x51));
        let Some(i) = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Hall && r.doors == want) else {
            continue;
        };
        run.enter_for_test(i);
        run.calm_for_test();
        run.enemies.clear();
        run.works.gauntlets.clear();
        run.works.perils.clear();
        run.window = None;
        assert!(run.lay_gauntlet_for_test(kind, seed + 5));
        for hero in run.players.values_mut() {
            hero.invulnerable = 100_000;
        }
        return run;
    }
    panic!("no straight passage");
}

/// A bent passage (its doorways on two sides at a corner) with its
/// longer leg bridged.
fn bent(depth: u32, pack: Pack) -> Run {
    for seed in 0..400u64 {
        let mut run = Run::new(seed, 3, None);
        run.join_seat(2, "Wren");
        run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, depth, pack, &mut crate::rules::Rng::new(seed ^ 0x51));
        let Some(i) = run.dungeon.rooms.iter().position(|r| {
            r.kind == RoomKind::Hall && r.doors.iter().filter(|&&d| d).count() == 2 && r.doors[0] != r.doors[2]
        }) else {
            continue;
        };
        run.enter_for_test(i);
        run.calm_for_test();
        run.enemies.clear();
        run.works.gauntlets.clear();
        run.works.perils.clear();
        run.window = None;
        if !run.lay_gauntlet_for_test("bridge", seed + 5) {
            continue;
        }
        for hero in run.players.values_mut() {
            hero.invulnerable = 100_000;
        }
        return run;
    }
    panic!("no bent passage with a leg to bridge");
}

/// A fighting room of `pack` with its gauntlet `kind` laid.
fn fight_room(pack: Pack, depth: u32, kind: &str) -> Run {
    for seed in 0..200u64 {
        let mut run = Run::new(seed, 4, None);
        run.join_seat(2, "Wren");
        run.dungeon = layout::floor_for(fortune::Mode::LongWayDown, depth, pack, &mut crate::rules::Rng::new(seed ^ 0x51));
        let Some(i) = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()) else {
            continue;
        };
        run.enter_for_test(i);
        run.calm_for_test();
        run.enemies.clear();
        run.works.gauntlets.clear();
        run.works.perils.clear();
        run.window = None;
        if !run.lay_gauntlet_for_test(kind, seed + 3) {
            continue;
        }
        for hero in run.players.values_mut() {
            hero.invulnerable = 100_000;
        }
        return run;
    }
    panic!("no room for a {kind}");
}

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H)
}

fn shot(run: &Run) -> Img {
    let (w, h) = seat();
    seat_scene(run, w, h, Some(1)).compose()
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (x, y);
}

/// Step until `pred` holds (at most a few seconds).
fn until(run: &mut Run, pred: impl Fn(&Run) -> bool) {
    for _ in 0..600 {
        if pred(run) {
            return;
        }
        run.step(&BTreeMap::new());
    }
}

fn bridge(run: &Run) -> (f32, f32, f32) {
    run.works
        .gauntlets
        .iter()
        .find_map(|g| match *g {
            Gauntlet::Bridge { from, to, line, .. } => Some((from, to, line)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn every_gauntlet_draws_its_danger_warm_and_lit_on_the_black_floor() {
    let mut run = passage(false, 1, Pack::Crypt, "bridge");
    let (from, _, line) = bridge(&run);
    put(&mut run, 1, (from - 2.0, line));
    let still = shot(&run);
    // Some warm light falls on the floor: a lantern's pool, an ember.
    let warm = (0..still.h)
        .flat_map(|y| (0..still.w).map(move |x| (x, y)))
        .filter(|&(x, y)| still.get(x, y).is_some_and(|c| c[0] as i32 > c[2] as i32 + 40 && c[0] > 70))
        .count();
    assert!(warm > 200, "warm light: {warm} pixels");
    // The frame is pure: the same run paints the same.
    assert_eq!(shot(&run).rgb_bytes(), still.rgb_bytes());
    // An axe's blade moves between frames.
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::new());
    assert_ne!(shot(&run).rgb_bytes(), still.rgb_bytes());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_gauntlet_shots -- --ignored`:
/// each gauntlet at its telling moments, three times native size.
#[test]
#[ignore]
fn write_gauntlet_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 3;
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // The axe bridge along a passage, a knight setting out, one on it.
    let mut run = passage(false, 1, Pack::Crypt, "bridge");
    let (from, to, line) = bridge(&run);
    put(&mut run, 1, (from + 5.0, line));
    put(&mut run, 2, (from - 2.5, line - 1.0));
    put(&mut run, 3, (to + 2.0, line + 0.8));
    save("bridge-crypt-a", &shot(&run));
    for k in 0..3 {
        for _ in 0..9 {
            run.step(&BTreeMap::new());
        }
        put(&mut run, 1, (from + 5.0 + k as f32 * 3.0, line));
        save(&format!("bridge-crypt-{}", ["b", "c", "d"][k]), &shot(&run));
    }
    // Upright: the axes swing across the screen.
    let mut run = passage(true, 3, Pack::Hellforge, "bridge");
    let (from, to, line) = bridge(&run);
    put(&mut run, 1, (line, from - 2.0));
    put(&mut run, 2, (line, (from + to) / 2.0));
    put(&mut run, 3, (line + 1.0, to + 2.0));
    save("bridge-keep-a", &shot(&run));
    for name in ["bridge-keep-b", "bridge-keep-c"] {
        for _ in 0..10 {
            run.step(&BTreeMap::new());
        }
        save(name, &shot(&run));
    }
    let mut run = passage(false, 2, Pack::Cavern, "bridge");
    let (from, _, line) = bridge(&run);
    put(&mut run, 1, (from + 9.0, line + 0.5));
    save("bridge-mines", &shot(&run));
    // A bent passage, its longer leg a gorge.
    let mut run = bent(2, Pack::Crypt);
    let (from, _, line) = bridge(&run);
    let upright = matches!(run.works.gauntlets[0], Gauntlet::Bridge { upright: true, .. });
    put(&mut run, 1, if upright { (line, from + 3.0) } else { (from + 3.0, line) });
    put(&mut run, 2, if upright { (line + 0.8, from - 2.0) } else { (from - 2.0, line + 0.8) });
    for _ in 0..7 {
        run.step(&BTreeMap::new());
    }
    save("bridge-bend", &shot(&run));
    // The hammer row: at rest, one gate shuddering, one down.
    let mut run = passage(false, 3, Pack::Hellforge, "hammers");
    put(&mut run, 1, (3.0, 14.0));
    put(&mut run, 2, (12.5, 15.0));
    put(&mut run, 3, (44.0, 13.0));
    until(&mut run, |r| r.works.gauntlets.iter().all(|g| g.drop(r.tick) == 0.0 && g.warning(r.tick).is_none()));
    save("hammers-rest", &shot(&run));
    until(&mut run, |r| r.works.gauntlets.iter().any(|g| g.warning(r.tick).is_some_and(|w| w > 0.6)));
    save("hammers-warn", &shot(&run));
    until(&mut run, |r| r.works.gauntlets.iter().any(|g| g.down(r.tick)));
    run.step(&BTreeMap::new());
    save("hammers-down", &shot(&run));
    let mut run = passage(true, 2, Pack::Cavern, "hammers");
    put(&mut run, 1, (24.0, 3.0));
    put(&mut run, 2, (23.0, 14.0));
    put(&mut run, 3, (25.0, 25.0));
    until(&mut run, |r| r.works.gauntlets.iter().any(|g| g.down(r.tick)));
    save("hammers-upright", &shot(&run));
    // The boulder: rumbling, then rolling at a knight.
    let mut run = fight_room(Pack::Cavern, 2, "boulder");
    let Gauntlet::Boulder { from, to, y, .. } = run.works.gauntlets[0] else { unreachable!() };
    put(&mut run, 1, ((from + to) / 2.0 + 4.0, y + 4.0));
    put(&mut run, 2, ((from + to) / 2.0, y - 4.5));
    until(&mut run, |r| r.works.gauntlets[0].warning(r.tick).is_some_and(|w| w > 0.6));
    save("boulder-rumble", &shot(&run));
    until(&mut run, |r| r.works.gauntlets[0].boulder(r.tick).is_some_and(|(x, _)| (x - (from + to) / 2.0).abs() < 4.0));
    save("boulder-roll", &shot(&run));
    let mut run = fight_room(Pack::Fungal, 5, "boulder");
    until(&mut run, |r| r.works.gauntlets[0].boulder(r.tick).is_some());
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    save("boulder-fungal", &shot(&run));
    // The saw, running and revving.
    let mut run = fight_room(Pack::Crypt, 1, "saw");
    let Gauntlet::Saw { a, b, .. } = run.works.gauntlets[0] else { unreachable!() };
    put(&mut run, 1, ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0 + 3.0));
    until(&mut run, |r| r.works.gauntlets[0].saw_at(r.tick).1.is_none());
    for _ in 0..8 {
        run.step(&BTreeMap::new());
    }
    save("saw-run", &shot(&run));
    until(&mut run, |r| r.works.gauntlets[0].saw_at(r.tick).1.is_some_and(|w| w > 0.5));
    save("saw-rev", &shot(&run));
    let mut run = fight_room(Pack::Archive, 4, "saw");
    for _ in 0..12 {
        run.step(&BTreeMap::new());
    }
    save("saw-archive", &shot(&run));
    // A knight swept by an axe, mid-air over the gorge... and back on the
    // landing.
    let mut run = passage(false, 1, Pack::Crypt, "bridge");
    let (from, _, line) = bridge(&run);
    let walk = BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]);
    put(&mut run, 1, (from - 2.0, line + 1.3));
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    for _ in 0..90 {
        run.step(&walk);
        if run.cues.iter().any(|c| c == "gorge_fall") {
            break;
        }
    }
    save("bridge-fall", &shot(&run));
}
