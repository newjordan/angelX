use super::*;
use crate::drive::together_realm::Spoils;
use crate::drive::together_shooter::home::{BUY_HOLD, Home, Station};

/// A realm whose west wing is dug: the tavern, and Beaumains in it.
fn tavern_home() -> Home {
    let mut home = Home {
        deepest: 1,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    home
}

fn centre((c, r, w, h): (i32, i32, i32, i32)) -> (f32, f32) {
    (
        (c as f32 + w as f32 / 2.0) * TILE_UNITS,
        (r as f32 + h as f32 / 2.0) * TILE_UNITS,
    )
}

#[test]
fn beaumains_greets_a_knight_and_takes_a_wage_at_his_plate() {
    let mut run = Run::at_home(3, 1, None, tavern_home(), Spoils::default());
    let tavern = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Tavern)
        .expect("the tavern is dug");
    run.enter_for_test(tavern);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (
        BEAUMAINS_AT.0 * TILE_UNITS + 2.0,
        BEAUMAINS_AT.1 * TILE_UNITS,
    );
    run.step(&BTreeMap::new());
    assert!(run.cues.iter().any(|c| c == "npc:beaumains"));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = centre(HIRE_PLATE);
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&fire);
    }
    assert_eq!(
        run.orders.iter().map(|o| o.station).collect::<Vec<_>>(),
        [Station::Hire]
    );
}

/// One knight down the stair, with Beaumains if `hired`.
fn down(hired: bool) -> Run {
    let mut home = tavern_home();
    home.hire = hired.then(|| "beaumains".into());
    let mut run = Run::at_home(3, 1, None, home, Spoils::default());
    run.descend_for_test();
    run
}

#[test]
fn hired_he_goes_down_the_stair_and_the_tab_clears() {
    let run = down(true);
    let hire = run.hireling.as_ref().expect("he came down");
    assert_eq!(hire.hp, hire.max_hp);
    assert_eq!(run.home.hire, None);
    assert_eq!(run.marks.get("hired"), Some(&1), "the cockpit clears it");
    assert!(run.cues.iter().any(|c| c == "hired:beaumains"));
    let run = down(false);
    assert!(run.hireling.is_none());
    assert!(!run.marks.contains_key("hired"));
}

/// A first-floor fight room, emptied and calmed, knight one and Beaumains
/// side by side; a bat frozen in the far corner keeps the fight on.
fn with_beaumains() -> Run {
    let mut run = down(true);
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
    run.spawn_at_for_test(EnemyKind::Bat, 44.0, 3.0);
    if let Some(bat) = run.enemies.last_mut() {
        bat.frozen = u32::MAX / 2;
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (10.0, 12.0, 0);
    let hire = run.hireling.as_mut().unwrap();
    (hire.x, hire.y, hire.invulnerable) = (12.0, 14.0, 0);
    run
}

#[test]
fn he_throws_knives_at_the_nearest_monster_from_a_few_steps_off() {
    let mut run = with_beaumains();
    run.spawn_at_for_test(EnemyKind::Bat, 15.0, 14.0);
    let mark = run.enemies.last().unwrap().id;
    if let Some(bat) = run.enemies.last_mut() {
        (bat.frozen, bat.hp, bat.max_hp) = (u32::MAX / 2, 10_000, 10_000);
    }
    for _ in 0..3 * HZ {
        run.players.get_mut(&1).unwrap().hp = 100;
        run.step(&BTreeMap::new());
    }
    let bat = run.enemies.iter().find(|e| e.id == mark).unwrap();
    assert!(bat.hp < 10_000, "his knives found it");
    let hire = run.hireling.as_ref().unwrap();
    let d = (hire.x - bat.x).hypot(hire.y - bat.y);
    assert!(d > 4.0, "he keeps a few steps off: {d}");
}

#[test]
fn felled_he_sits_the_fight_out_and_gets_up_after_it() {
    let mut run = with_beaumains();
    let hire = run.hireling.as_mut().unwrap();
    hire.hp = 5;
    let (x, y) = (hire.x, hire.y);
    run.projectiles.push(Projectile {
        x,
        y,
        vx: 8.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Orb,
        damage: 12,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    });
    run.step(&BTreeMap::new());
    assert!(run.hireling.as_ref().unwrap().down());
    assert!(run.cues.iter().any(|c| c == "beaumains_down"));
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert!(
        run.hireling.as_ref().unwrap().down(),
        "while the fight lasts"
    );
    run.enemies.clear();
    run.step(&BTreeMap::new());
    let hire = run.hireling.as_ref().unwrap();
    assert_eq!(hire.hp, hire.max_hp / 2, "up again, sore");
    assert!(run.cues.iter().any(|c| c == "beaumains_up"));
}

#[test]
fn he_follows_the_party_through_the_doors() {
    let mut run = with_beaumains();
    run.enemies.clear();
    run.step(&BTreeMap::new());
    let next = (0..4)
        .find_map(|side| run.dungeon.neighbour(run.at, side))
        .expect("a way on");
    run.enter_for_test(next);
    let hero = &run.players[&1];
    let hire = run.hireling.as_ref().unwrap();
    assert!((hire.x - hero.x).hypot(hire.y - hero.y) < 2.5);
}

#[test]
fn he_is_no_knight_of_the_party() {
    // A guardian doesn't grow for him.
    let mut run = down(true);
    let stairs = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(stairs);
    let boss = run.enemies.iter().find(|e| e.boss.is_some()).unwrap();
    let guardian = &run.bosses[usize::from(boss.boss.unwrap())];
    assert_eq!(boss.max_hp, guardian.hp, "one knight's worth");
    // And the party falls when its knights do, whether he stands or not.
    let mut run = with_beaumains();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.hp, hero.winds, hero.talisman) = (0, 0, false);
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
}

#[test]
fn a_ravage_throws_him_too() {
    let mut run = with_beaumains();
    let hire = run.hireling.as_ref().unwrap();
    let (x, y, hp) = (hire.x, hire.y, hire.hp);
    run.ravages.push(tide::Ravage {
        x: x + tide::RING_GAP,
        y,
        rings: 1,
        lane: 0.0,
        width: 0.5,
        step: 6,
        damage: 16,
        age: 0,
    });
    for _ in 0..tide::WARN + 1 {
        run.step(&BTreeMap::new());
    }
    assert!(run.hireling.as_ref().unwrap().hp < hp);
}
