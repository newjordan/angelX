//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::rules::{Input, home::Home, world};
use std::collections::BTreeMap;

pub fn fight() -> Run {
    let mut r = Run::new(11, 1, Some("Knight"));
    let i = r
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight && !room.great())
        .unwrap();
    // A wraiths' room, whichever company the floor gave it: the tableau
    // this was drawn for (a skeleton's bone shots are not yet kept whole
    // by the v2 art; the overlay test fails on them at the base too).
    r.dungeon.rooms[i].roster = vec![EnemyKind::Wraith; 4];
    r.enter_for_test(i);
    r.dungeon.pack = Pack::Crypt;
    for _ in 0..95 {
        for h in r.players.values_mut() {
            h.hp = h.max_hp;
        }
        r.step(&BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                ..Default::default()
            },
        )]));
    }
    // Repeatable tableau; projectiles come from the actual simulation.
    if let Some(e) = r.enemies.first().cloned() {
        r.enemies.truncate(2);
        for (i, (x, y, kind)) in [
            (14.0, 9.0, EnemyKind::Skeleton),
            (33.0, 12.0, EnemyKind::Skeleton),
            (25.0, 6.0, EnemyKind::Wraith),
        ]
        .into_iter()
        .enumerate()
        {
            let mut e = e.clone();
            e.id = 100 + i as u32;
            e.kind = kind;
            e.x = x;
            e.y = y;
            e.hp = e.max_hp;
            e.age = 100;
            e.hurt = 0;
            r.enemies.push(e);
        }
    }
    if let Some(shot) = r.projectiles.iter().find(|p| p.hostile).cloned() {
        for i in 0..24 {
            let angle = i as f32 * std::f32::consts::TAU / 24.0;
            let mut p = shot.clone();
            p.x = 25.0 + angle.cos() * 7.0;
            p.y = 12.0 + angle.sin() * 5.0;
            p.vx = angle.cos() * 8.0;
            p.vy = angle.sin() * 8.0;
            r.projectiles.push(p);
        }
    }
    for (id, x, y) in [(1, 21.0, 19.0), (2, 29.0, 21.0)] {
        if let Some(h) = r.players.get_mut(&id) {
            h.x = x;
            h.y = y;
            h.invulnerable = 0;
            h.struck = 0;
        }
    }
    r
}

/// One live guardian, using its own pack and unchanged attacks.
pub fn boss(id: &str) -> Run {
    let mut run = fight();
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.feel = Default::default();
    if !run.bosses.iter().any(|b| b.id == id) {
        let boss = crate::rules::bosses::checked(id).expect("review boss definition");
        // Retain the existing single-pack replacement in the original fixtures.
        if matches!(id, "crypt-tithe-marshal" | "crypt-tax-keeper" | "crypt-relic-liege" | "red-pick-chief" | "iron-tusk-chief") {
            run.bosses.retain(|b| b.only_in != boss.only_in);
        }
        run.bosses.push(boss);
    }
    let index = run.bosses.iter().position(|b| b.id == id).expect("review boss exists");
    run.dungeon.pack = run.bosses[index].only_in;
    run.spawn_boss_at_for_test(index, 24.0, 12.0);
    if matches!(id, crate::rules::signatures::ENGINE | crate::rules::signatures::RIPPER) {
        run.enemies[0].signature.active = true;
    }
    run
}

pub fn home(kind: RoomKind, tick: u64) -> Run {
    let mut r = Run::at_home(23, 1, Some("Knight"), Home::default(), Default::default());
    r.come_to_the_gate();
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 10.5));
    r.tick = tick;
    r
}

pub fn kingdom() -> Run {
    let mut r = Run::at_home(41, 1, Some("Knight"), Home::default(), Default::default());
    let geo = super::super::super::kingdom::geography(&r.kingdom.wishes);
    let site = geo
        .sites
        .iter()
        .find(|s| s.room == RoomKind::Ranch)
        .unwrap();
    r.walk_kingdom(Some(site.stand));
    r.tick = 80;
    r
}

/// Existing side-on rules and the Book Hop folly, for kit coverage measurements.
pub fn hall() -> Run {
    let mut r = Run::new(0, 1, None);
    let at = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Ledge).unwrap();
    r.enter(at, Some(3));
    r
}

pub fn book_hop() -> Run {
    let mut r = Run::new(0, 1, None);
    for _ in 0..5 { r.descend_for_test(); }
    for seat in 2..=4 { r.join_seat(seat, &format!("Knight {seat}")); }
    r.enter_for_test(r.folly_room.expect("archive folly"));
    r.hop.as_mut().expect("sixth floor Book Hop").t = crate::rules::folly::TITLE + 95;
    r
}

#[path = "overworld__art_v2_match.rs"]
pub mod matching;

pub fn fixtures() -> Vec<(&'static str, Run)> {
    vec![
        ("crypt", fight()),
        ("fields", home(RoomKind::Pasture, 20)),
        ("college", home(RoomKind::CollegeHall, 83)),
        ("kingdom", kingdom()),
    ]
}
