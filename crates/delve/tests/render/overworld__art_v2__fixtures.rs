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
