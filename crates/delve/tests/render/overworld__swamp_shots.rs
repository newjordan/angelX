use super::super::{BAND_H, HUD_H, NATIVE_H, NATIVE_W, frame, seat_scene};
use super::*;
use crate::rules::home::Home;
use crate::rules::{Input, Pack, swamp, world};
use std::collections::BTreeMap;

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + BAND_H + HUD_H)
}

/// Two knights in the swamp: floor `depth`, the plate's way down.
fn swamp_run(seed: u64, depth: u32) -> Run {
    let mut run = Run::at_home(seed, 1, Some("Squire"), Home::default(), Default::default());
    run.begin_in(Pack::Swamp);
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    run
}

/// A foggy fight of the swamp's `depth`, a bog hand rippling, a drowned
/// knight with its blade up and a wisp luring.
fn foggy_fight(depth: u32) -> Run {
    let mut run = swamp_run(4, depth);
    let room = (0..run.dungeon.rooms.len())
        .filter(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight)
        .find(|&i| !swamp::fog(Pack::Swamp, &run.dungeon.rooms[i], depth).is_empty())
        .expect("a foggy fight");
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    let (w, h) = (run.room().width(), run.room().height());
    for (i, hero) in run.players.values_mut().enumerate() {
        (hero.x, hero.y) = (w / 2.0 - 3.0 + 6.0 * i as f32, h / 2.0);
        hero.hp = hero.max_hp;
    }
    for (kind, x, y, stage, timer) in [
        (EnemyKind::BogHand, w / 2.0 - 5.0, h / 2.0 + 3.0, 1, 18),
        (EnemyKind::Drowned, w / 2.0 + 7.0, h / 2.0 - 2.0, 1, 20),
        (EnemyKind::Wisp, w / 2.0 - 12.0, h / 2.0 - 7.0, 1, 30),
        (EnemyKind::Leech, w / 2.0 + 12.0, h / 2.0 + 6.0, 0, 0),
    ] {
        run.spawn_at_for_test(kind, x, y);
        run.dare = None;
        let e = run.enemies.last_mut().unwrap();
        (e.stage, e.timer) = (stage, timer);
        if kind == EnemyKind::Drowned {
            e.dir = (-1.0, 0.3);
        }
    }
    run.tick = 90;
    run
}

#[test]
fn swamp_frames_draw_in_the_palette_and_the_fog_darkens_only_its_patches() {
    let run = foggy_fight(2);
    let a = frame(&run, 384, 224);
    assert!(a == frame(&run, 384, 224), "deterministic");
    assert!(a.pixels().all(|v| v == super::super::super::ink::BLACK
        || super::super::super::ink::palette_index(v).is_some()));
    let mut clear = run.clone();
    clear.dungeon.pack = Pack::Crypt;
    let lit = frame(&clear, 384, 224);
    assert!(a != lit, "the swamp's fog and mire are drawn");
    // The pen with the white horse in it, and the Marsh's plate, draw too.
    let mut hall = swamp_run(5, swamp::FLOORS);
    let i = hall.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    hall.enter(i, Some(2));
    let pen = hall.captive.clone().unwrap();
    for hero in hall.players.values_mut() {
        (hero.x, hero.y) = (pen.x, pen.y + 4.0);
    }
    let with = frame(&hall, 384, 224);
    hall.captive = None;
    assert!(with != frame(&hall, 384, 224), "the pen is drawn");
}

#[test]
#[ignore = "writes the swamp's rooms for visual review"]
fn write_swamp_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &Img| {
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_scaled(3),
            im.w as u32 * 3,
            im.h as u32 * 3,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let (w, h) = seat();
    // 1. The plate in Sir Damp's Marsh, a knight reading its notice.
    let mut run = Run::at_home(23, 1, Some("Squire"), Home::default(), Default::default());
    run.come_to_the_gate();
    let marsh = world::room_of(&run.dungeon, RoomKind::Marsh).unwrap();
    run.arrive(marsh, (12.0, 7.0));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (swamp::NOTICE.0 * 2.0, (swamp::NOTICE.1 + 0.6) * 2.0);
    let squire = run.players.get_mut(&2).unwrap();
    (squire.x, squire.y) = (30.0, 16.0);
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    run.tick = 60;
    save("01-marsh-plate", &seat_scene(&run, w, h, Some(1)).compose());
    // 2. A swamp fight: bog, fog, and the horrors' tells.
    for depth in [1, 2] {
        let run = foggy_fight(depth);
        save(&format!("02-swamp-floor{depth}-bog-fog"), &seat_scene(&run, w, h, Some(1)).compose());
    }
    // 3. The Mire King's hall: the King, the pen with the white horse.
    let mut hall = swamp_run(5, swamp::FLOORS);
    let i = hall.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
    hall.enter(i, Some(2));
    hall.dare = None;
    let pen = hall.captive.clone().unwrap();
    for _ in 0..50 {
        for hero in hall.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        hall.step(&BTreeMap::new());
    }
    let hero = hall.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (pen.x - 2.0, pen.y + 3.0);
    save("03-mire-king-pen", &seat_scene(&hall, w, h, Some(1)).compose());
    // 4. He falls; the pen opens and she walks out; the light home.
    hall.clear_for_test();
    let hero = hall.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (pen.x, pen.y + 2.0);
    for _ in 0..crate::rules::home::BUY_HOLD + 20 {
        hall.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    }
    save("04-pen-open-light-home", &seat_scene(&hall, w, h, Some(1)).compose());
    // 5. In the herd, at grass in the Flower Fields.
    let mut home = Home::default();
    home.stable.herd.push(swamp::white_horse());
    let mut fields = Run::at_home(23, 1, Some("Squire"), home, Default::default());
    fields.come_to_the_gate();
    fields.arrive(world::room_of(&fields.dungeon, RoomKind::Pasture).unwrap(), (12.0, 10.5));
    for t in [40u64, 300] {
        fields.tick = t;
        save(&format!("05-pasture-white-horse-{t:03}"), &seat_scene(&fields, w, h, Some(1)).compose());
    }
}
