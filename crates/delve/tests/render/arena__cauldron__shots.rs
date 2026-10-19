//! The cauldron on screen: risen by the stairs, waiting, a knight picking,
//! a brew thrown in, boiling over, its card risen, and a misfire.

use super::super::*;
use crate::rules::cauldron::{Misfire, Outcome};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H)
}

/// Steps until the run's clock reads `tick` (an impact's hold stops it).
fn tick_to(run: &mut Run, tick: u64, keys: &BTreeMap<u32, Input>) {
    while run.tick < tick {
        run.step(keys);
    }
}

fn press(run: &mut Run, input: Input) {
    let t = run.tick;
    tick_to(run, t + 1, &BTreeMap::from([(1, input)]));
    tick_to(run, t + 2, &BTreeMap::new());
}

/// A party of three on an open floor, its guardian just fallen, the pot
/// risen; Galahad a step south of it with three spells, the others by.
fn risen(seed: u64) -> Run {
    let mut run = Run::new(seed, 7, None);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    run.open_floor_for_test(2);
    let open = run.dungeon.open.clone().unwrap();
    let g = usize::from(open.guardian);
    let (cx, cy) = open.chambers[g].centre();
    let inside = run.open_spots(open.map, (cx, cy + 4.0), 3);
    for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = inside[k];
        hero.invulnerable = 100_000;
    }
    run.step(&BTreeMap::new());
    for e in run.enemies.iter_mut().filter(|e| e.home == Some(g as u8)) {
        e.hp = 0;
    }
    for _ in 0..400 {
        run.step(&BTreeMap::new());
        if run.cauldron_here().is_some() {
            break;
        }
    }
    let pot = run.cauldron_here().expect("the pot rises").clone();
    for (id, (dx, dy)) in [(1u32, (0.4, 2.2)), (2, (-3.6, 1.2)), (3, (3.4, 2.6))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (pot.x + dx, pot.y + dy);
        (hero.aim_x, hero.aim_y) = (0.0, -1.0);
    }
    let hero = run.players.get_mut(&1).unwrap();
    hero.spells = [Some("cabbage-storm".into()), Some("hedge-maze".into()), Some("the-imposture".into()), None];
    run
}

/// The first seed from `from` whose brew of slots 1 and 2 comes to what
/// `want` asks, thrown in: the run and the tick it was thrown.
fn thrown(from: u64, want: impl Fn(&Outcome) -> bool) -> (Run, u64) {
    for seed in from..from + 400 {
        let mut run = risen(seed);
        let rose = run.cauldron_here().unwrap().rose;
        tick_to(&mut run, rose + u64::from(crate::rules::cauldron::RISE_TICKS) + 10, &BTreeMap::new());
        press(&mut run, Input { interact: true, take: true, ..Input::default() });
        press(&mut run, Input { cast: 1, ..Input::default() });
        press(&mut run, Input { cast: 2, ..Input::default() });
        let t = run.tick;
        tick_to(&mut run, t + 1, &BTreeMap::from([(1, Input { interact: true, ..Input::default() })]));
        let Some(brew) = run.cauldron_here().and_then(|p| p.brew(1)).cloned() else {
            continue;
        };
        if want(&brew.outcome) {
            return (run, brew.at);
        }
    }
    panic!("no seed brewed it");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_cauldron_shots -- --ignored`.
#[test]
#[ignore]
fn write_cauldron_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (w, h) = seat();
    let save = |name: &str, run: &Run| {
        let img = seat_scene(run, w, h, Some(1)).compose();
        let rgba = img.rgba_scaled(3);
        image::RgbaImage::from_raw(img.w as u32 * 3, img.h as u32 * 3, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // Rising, then waiting with its prompt.
    let mut run = risen(17);
    let rose = run.cauldron_here().unwrap().rose;
    tick_to(&mut run, rose + 12, &BTreeMap::new());
    save("cauldron-01-rising", &run);
    tick_to(&mut run, rose + 50, &BTreeMap::new());
    save("cauldron-02-waiting", &run);
    tick_to(&mut run, rose + 57, &BTreeMap::new());
    save("cauldron-03-waiting-later", &run);
    press(&mut run, Input { interact: true, take: true, ..Input::default() });
    press(&mut run, Input { cast: 1, ..Input::default() });
    save("cauldron-04-picking-one", &run);
    press(&mut run, Input { cast: 2, ..Input::default() });
    save("cauldron-05-picking-two", &run);
    // A spell brewed, through its whole show.
    let (mut run, at) = thrown(17, |o| matches!(o, Outcome::Spell { .. }));
    for (name, age) in [
        ("cauldron-06-thrown", 5u64),
        ("cauldron-07-splash", 11),
        ("cauldron-08-boiling", 40),
        ("cauldron-09-boiling-over", 56),
        ("cauldron-10-puff", 64),
        ("cauldron-11-card-rising", 76),
        ("cauldron-12-card-risen", 100),
        ("cauldron-13-card-named", 140),
    ] {
        tick_to(&mut run, at + age, &BTreeMap::new());
        save(name, &run);
    }
    // A potent or perfect brew's rim.
    let (mut run, at) = thrown(400, |o| matches!(o, Outcome::Spell { tier, .. } if *tier != crate::rules::cauldron::Tier::Common));
    tick_to(&mut run, at + 110, &BTreeMap::new());
    save("cauldron-14-potent", &run);
    // The room-by-room delve: the guardian's stairs room, and the first
    // floor's, which becomes a Sanctuary round Tobbin's anvil.
    for (name, depth) in [("cauldron-15-stairs-room", 2u32), ("cauldron-16-sanctuary", 1)] {
        let mut run = Run::new(23, 7, None);
        run.join_seat(2, "Wren");
        while run.dungeon.depth < depth {
            run.descend_for_test();
        }
        run.boss_gates = None;
        let room = run.dungeon.rooms.iter().position(|r| r.kind == crate::rules::RoomKind::Stairs).unwrap();
        run.enter_for_test(room);
        for hero in run.players.values_mut() {
            hero.invulnerable = 100_000;
        }
        run.clear_for_test();
        let pot = run.cauldron_here().expect("the pot rises").clone();
        for (id, (dx, dy)) in [(1u32, (1.0, 2.6)), (2, (-3.4, 1.6))] {
            let hero = run.players.get_mut(&id).unwrap();
            (hero.x, hero.y) = (pot.x + dx, pot.y + dy);
        }
        tick_to(&mut run, pot.rose + 60, &BTreeMap::new());
        save(name, &run);
    }
    // Misfires.
    for (name, kind) in [("confetti", Misfire::Confetti), ("hiccups", Misfire::Hiccups), ("frog", Misfire::Frog)] {
        let (mut run, at) = thrown(1, |o| *o == Outcome::Misfire(kind));
        for age in [66u64, 100] {
            tick_to(&mut run, at + age, &BTreeMap::new());
            save(&format!("cauldron-misfire-{name}-{age}"), &run);
        }
    }
}
