//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::realm::Spoils;
use crate::rules::home::{Home, Station};

/// A realm whose west wing is dug: the tavern, and Beaumains in it.
pub fn tavern_home() -> Home {
    let mut home = Home {
        deepest: 1,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    home
}

/// One knight down the stair, with Beaumains if `hired`.
pub fn down(hired: bool) -> Run {
    let mut home = tavern_home();
    home.hire = hired.then(|| "beaumains".into());
    let mut run = Run::at_home(3, 1, None, home, Spoils::default());
    run.descend_for_test();
    run
}

/// A first-floor fight room, emptied and calmed, with Beaumains if
/// `hired`; knight one at (12, 12), unhurt, holding `hand`.
pub fn fight_with(hired: bool, hand: &[&str]) -> Run {
    let mut run = down(hired);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
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

/// A foe at `(x, y)`, awake, that holds still by itself (`frozen`).
pub fn foe_at(run: &mut Run, kind: EnemyKind, (x, y): (f32, f32), frozen: u32) -> u32 {
    run.spawn_at_for_test(kind, x, y);
    let foe = run.enemies.last_mut().unwrap();
    foe.frozen = frozen;
    foe.id
}
