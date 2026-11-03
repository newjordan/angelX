//! The Gravecaller and his dead on screen, at the seat's own scale: at his
//! bench in the tavern, beside a knight of the company, raising a ghoul,
//! his whole company at work with the mark on a foe and the warden
//! roaring, the Tombstone and its zombies, a soul rip, his dead at heel,
//! and a close sheet of every drawing. `ANGEL_ARENA_SHOTS=<dir> cargo test
//! write_minion_shots -- --ignored`.

use super::super::class_looks::{Pose, gravecaller};
use super::super::knights::Facing;
use super::super::*;
use crate::rules::hireling::testing::{fight_with, foe_at};
use crate::rules::minions::GRAVECALLER;
use crate::rules::{EnemyKind, HZ, Input, Phase, Run, alehouse};
use std::collections::BTreeMap;

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + party::BAND_H + party::HUD_H)
}

/// The Gravecaller (knight 1) and the Archer (knight 2) in an open fight
/// room, awake, side by side.
fn field() -> Run {
    let mut run = fight_with(false, &[]);
    run.join_seat(2, "Wren");
    assert!(run.take_up(1, GRAVECALLER));
    assert!(run.take_up(2, "percival"));
    for (id, at) in [(1, (14.0, 15.0)), (2, (13.0, 22.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.idle, hero.stone, hero.invulnerable) = (at.0, at.1, false, false, 0);
        (hero.aim_x, hero.aim_y) = (0.0, 1.0);
        hero.spell_cooldowns = [0; 4];
    }
    run
}

/// A foe standing still where it is put, awake.
fn foe(run: &mut Run, kind: EnemyKind, at: (f32, f32)) -> u32 {
    let id = foe_at(run, kind, at, u32::MAX / 2);
    let foe = run.enemies.last_mut().unwrap();
    (foe.hp, foe.max_hp) = (900, 900);
    run.phase = Phase::Fighting;
    id
}

fn cast(run: &mut Run, slot: u8, aim: (i8, i8)) {
    run.players.get_mut(&1).unwrap().spell_cooldowns = [0; 4];
    run.step(&BTreeMap::from([(1, Input { cast: slot, aim_x: aim.0, aim_y: aim.1, ..Default::default() })]));
}

fn wait(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
    }
}

#[test]
#[ignore]
fn write_minion_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save_img = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let shot = |name: &str, run: &Run| {
        let img = seat_scene(run, w, h, Some(1)).compose();
        save_img(name, &img, 3);
    };

    // At his bench in the tavern, a knight walking up to him.
    let mut run = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    run.walk_alehouse();
    run.floor_plates.clear();
    run.alehouse.company_page = 3;
    let seat_at = alehouse::PATRONS.iter().find(|p| p.kit == GRAVECALLER).unwrap().at;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (seat_at.0 * 2.0, (seat_at.1 - 1.2) * 2.0);
    for _ in 0..100 {
        run.step(&BTreeMap::new());
    }
    shot("minions_01_tavern", &run);

    // Beside the Archer: a hero's size, toward the viewer.
    let mut run = field();
    let archer = run.players.get_mut(&2).unwrap();
    (archer.x, archer.y) = (18.0, 15.0);
    foe(&mut run, EnemyKind::Bat, (40.0, 4.0));
    wait(&mut run, 2);
    shot("minions_02_beside_a_knight", &run);

    // Raising a ghoul: staff up, the flare, the dead climbing out.
    let mut run = field();
    foe(&mut run, EnemyKind::Skeleton, (30.0, 8.0));
    cast(&mut run, 1, (1, 0));
    wait(&mut run, 6);
    shot("minions_03_raise", &run);

    // His whole company at work: a mark on the skeleton he fires at, the
    // ghoul on it, the warden roaring before him, the totem spitting fire.
    let mut run = field();
    let marked = foe(&mut run, EnemyKind::Skeleton, (24.0, 10.0));
    foe(&mut run, EnemyKind::Skeleton, (8.0, 22.0));
    foe(&mut run, EnemyKind::Goblin, (26.0, 18.0));
    cast(&mut run, 1, (1, 0));
    cast(&mut run, 2, (1, 0));
    cast(&mut run, 3, (1, 0));
    wait(&mut run, RISE_WAIT);
    for _ in 0..20 {
        run.step(&BTreeMap::from([(1, Input { fire: true, aim_x: 1, aim_y: -1, ..Default::default() })]));
    }
    assert_eq!(run.minions.quarry.get(&1), Some(&marked));
    shot("minions_04_company", &run);

    // The Tombstone, its zombies up and taking hold.
    let mut run = field();
    foe(&mut run, EnemyKind::Skeleton, (22.0, 9.0));
    foe(&mut run, EnemyKind::Skeleton, (25.0, 13.0));
    foe(&mut run, EnemyKind::Bat, (12.0, 6.0));
    run.players.get_mut(&1).unwrap().ult_charge = crate::rules::ults::ULT_FULL;
    run.step(&BTreeMap::from([(1, Input { ult: true, aim_x: 1, ..Default::default() })]));
    shot("minions_05_tombstone_slam", &run);
    wait(&mut run, 4 * HZ);
    shot("minions_06_tombstone_zombies", &run);

    // A soul rip, the souls streaming home.
    let mut run = field();
    cast(&mut run, 1, (1, 0));
    wait(&mut run, RISE_WAIT);
    for at in [(22.0, 15.0), (23.0, 13.5), (21.5, 17.0)] {
        foe(&mut run, EnemyKind::Skeleton, at);
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    cast(&mut run, 4, (1, 0));
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::new());
    shot("minions_07_soul_rip", &run);

    // At heel: the rings at their feet.
    let mut run = field();
    foe(&mut run, EnemyKind::Skeleton, (36.0, 6.0));
    cast(&mut run, 1, (1, 0));
    cast(&mut run, 2, (1, 0));
    wait(&mut run, RISE_WAIT);
    run.step(&BTreeMap::from([(1, Input { vigil: true, ..Default::default() })]));
    wait(&mut run, HZ);
    shot("minions_08_heel", &run);

    // Close: every way the Gravecaller is drawn (stand, a step, the
    // strike) beside the realm's knight and the Archer, then his dead and
    // the stone.
    let run = field();
    let mut figures: Vec<Img> = vec![super::super::knights::figure(&run.players[&2], 2)];
    let mut knight = run.players[&2].clone();
    (knight.knight, knight.class) = (Some("composer".into()), None);
    figures.push(super::super::knights::figure(&knight, 2));
    for facing in [Facing::Down, Facing::Side, Facing::Up] {
        for pose in [Pose::Stand, Pose::Step(0), Pose::Strike] {
            figures.push(gravecaller::figure(facing, pose, true).recolor(super::super::knights::seat_colours(1)));
        }
    }
    for rows in [&super::GHOUL[..], &super::WARDEN[..], &super::TOTEM[..], &super::ZOMBIE[..]] {
        figures.push(Img::from_rows(rows));
    }
    let gap = 4;
    let wide: i32 = figures.iter().map(|f| f.w + gap).sum::<i32>() + gap;
    let tall = figures.iter().map(|f| f.h).max().unwrap() + 2 * gap;
    let mut sheet = Img::new(wide, tall);
    sheet.rect(0, 0, wide, tall, 'k');
    let mut x = gap;
    for f in &figures {
        sheet.stamp(f, x, tall - gap - f.h);
        x += f.w + gap;
    }
    save_img("minions_09_sheet", &sheet, 6);
}

/// Long enough for the raised to be out of the ground.
const RISE_WAIT: u32 = crate::rules::minions::RISE + 2;
