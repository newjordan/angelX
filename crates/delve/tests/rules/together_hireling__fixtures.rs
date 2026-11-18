//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::realm::Spoils;
use crate::rules::home::{Home, Station};

pub fn tavern_home() -> Home {
    let mut home = Home {
        deepest: 1,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    home
}

pub fn down(hired: bool) -> Run {
    let mut home = tavern_home();
    home.hire = hired.then(|| "beaumains".into());
    let mut run = Run::at_home(3, 1, None, home, Spoils::default());
    run.descend_for_test();
    run
}

pub fn fight_with(hired: bool, hand: &[&str]) -> Run {
    let mut run = down(hired);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    let open = &mut run.dungeon.rooms[room];
    for row in 1..open.rows - 1 {
        for col in 1..open.cols - 1 {
            open.set(col, row, Tile::Floor);
        }
    }
    run.enter_for_test(room);
    run.calm_for_test();
    run.settle_seams_for_test();
    run.enemies.clear();
    run.rune = None;
    run.dungeon.rooms[room].items.clear();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (12.0, 12.0, 0);
    hero.hand = hand.iter().map(|id| id.to_string()).collect();
    hero.play_cooldown = 0;
    run
}

pub fn play(run: &mut Run, slot: u8) {
    run.step(&BTreeMap::from([(
        1,
        Input {
            play: slot,
            ..Default::default()
        },
    )]));
}

pub fn foe_at(run: &mut Run, kind: EnemyKind, (x, y): (f32, f32), frozen: u32) -> u32 {
    run.spawn_at_for_test(kind, x, y);
    let foe = run.enemies.last_mut().unwrap();
    foe.frozen = frozen;
    foe.id
}
