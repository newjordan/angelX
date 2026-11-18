use super::*;
use super::super::{NATIVE_H, NATIVE_W};
use crate::rules::{Input, RoomKind, TILE_UNITS, knights};
use std::collections::BTreeMap;

fn inks(im: &Img, ch: char) -> usize {
    let want = crate::render::overworld::ink::ink(ch).unwrap();
    (0..im.h).flat_map(|y| (0..im.w).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y) == Some(want)).count()
}

#[test]
fn the_lamp_smokes_while_wishes_are_written_and_flares_when_one_lands() {
    let rest = sprite(0, 0, None);
    let smoking = sprite(0, 3, None);
    let flaring = sprite(0, 0, Some(3));
    assert!(inks(&smoking, '2') > inks(&rest, '2'), "teal smoke rises off the lamp");
    assert!(inks(&flaring, 'w') > inks(&rest, 'w'), "the lamp burns white at a grant");
    assert!(inks(&rest, '7') > 0 && inks(&rest, 'v') > 0, "a red runner on a stone slab");
}

#[test]
fn the_start_room_draws_its_altar() {
    let mut run = Run::new(7, 1, None);
    let with = super::super::frame_for(&run, 2 * NATIVE_W, 2 * NATIVE_H, Some(1));
    run.identity = true;
    let without = super::super::frame_for(&run, 2 * NATIVE_W, 2 * NATIVE_H, Some(1));
    assert_ne!(with, without, "the altar is drawn where it stands");
}

fn party(n: u32) -> Run {
    let mut run = Run::new(11, 3, Some("Ada"));
    for k in 3..=n {
        run.join_seat(k, ["", "", "", "Bors", "Cai", "Dag", "Enid"][k as usize]);
    }
    for k in 1..=n {
        run.outfit(k, &knights::COMPANY[k as usize % knights::COMPANY.len()]);
    }
    for hero in run.players.values_mut() {
        hero.wishes = 3;
    }
    run.floor_plates.clear();
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    run
}

#[test]
#[ignore]
fn write_wish_altar_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2;
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let step = |run: &mut Run, n: u32| {
        for _ in 0..n {
            run.step(&BTreeMap::new());
        }
    };
    let mut run = party(4);
    assert_eq!(run.room().kind, RoomKind::Start);
    let (ax, ay) = run.wish_altar().unwrap();
    for (i, hero) in run.players.values_mut().enumerate() {
        hero.x = ax + (i as f32 - 1.5) * 3.0 * TILE_UNITS;
        hero.y = ay + 4.0 * TILE_UNITS;
    }
    let ada = run.players.get_mut(&1).unwrap();
    ada.x = ax + 0.5;
    ada.y = ay + 3.0;
    step(&mut run, 20);
    save("1-altar-and-lamp", &super::super::frame_for(&run, 2 * NATIVE_W, 2 * NATIVE_H, Some(1)));
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert!(run.lamp.rubbed.contains_key(&1));
    run.lamp_sent(1, "my arrows turn into fireballs");
    run.lamp_sent(2, "a frog that fights beside me");
    run.lamp_sent(3, "a shield made of ice");
    step(&mut run, 9);
    save("2-lamp-smoking-wishes-in-flight", &super::super::frame_for(&run, 2 * NATIVE_W, 2 * NATIVE_H, Some(1)));
    let fight = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.enter_for_test(fight);
    let fire = BTreeMap::from([
        (1, Input { aim_x: 1, fire: true, ..Default::default() }),
        (2, Input { aim_x: -1, fire: true, ..Default::default() }),
        (3, Input { aim_y: -1, fire: true, ..Default::default() }),
        (4, Input { aim_x: 1, aim_y: 1, fire: true, ..Default::default() }),
    ]);
    let (mx, my) = (run.room().width() / 2.0, run.room().height() * 0.62);
    for (i, hero) in run.players.values_mut().enumerate() {
        hero.x = mx + (i as f32 - 1.5) * 2.5 * TILE_UNITS;
        hero.y = my + (i % 2) as f32 * TILE_UNITS;
        hero.invulnerable = 0;
    }
    for _ in 0..41 {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&fire);
    }
    save("3-fighting-with-wishes-in-flight", &super::super::frame_for(&run, 2 * NATIVE_W, 2 * NATIVE_H, Some(1)));
    let wish = crate::rules::phrasebook::get("ricochet").unwrap();
    let name = run.grant(1, &wish).unwrap();
    run.lamp_landed(1, "my arrows turn into fireballs", &name);
    for _ in 0..9 {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
            hero.invulnerable = 0;
        }
        run.step(&fire);
    }
    save("4-grant-lands-mid-fight", &super::super::frame_for(&run, 2 * NATIVE_W, 2 * NATIVE_H, Some(1)));
    let mut run = party(2);
    let (ax, ay) = run.wish_altar().unwrap();
    let ada = run.players.get_mut(&1).unwrap();
    ada.x = ax - 3.0 * TILE_UNITS;
    ada.y = ay + 3.0 * TILE_UNITS;
    run.lamp_sent(2, "a frog that fights beside me");
    step(&mut run, 12);
    run.lamp_landed(2, "a frog that fights beside me", "Frog Friend");
    step(&mut run, 4);
    save("5-lamp-flare-at-a-grant", &super::super::frame_for(&run, 2 * NATIVE_W, 2 * NATIVE_H, Some(1)));
}

#[test]
#[ignore]
fn write_wish_altar_snapshot() {
    let Some(path) = std::env::var_os("ANGEL_WISH_SNAPSHOT") else {
        return;
    };
    let mut run = Run::new(11, 3, None);
    run.outfit(1, &knights::COMPANY[1]);
    run.floor_plates.clear();
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    let (ax, ay) = run.wish_altar().unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = ax + 0.5;
    hero.y = ay + 3.0;
    hero.wishes = 3;
    hero.invulnerable = 0;
    hero.max_hp = 900;
    hero.hp = 900;
    std::fs::write(path, serde_json::to_vec(&run).unwrap()).unwrap();
}
