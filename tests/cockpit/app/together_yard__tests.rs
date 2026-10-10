use super::*;
use crate::drive::together_shooter::home::{BUY_HOLD, Home};

/// A delve at home, walked north into the Training Yard.
fn in_the_yard(goblins: u32) -> Run {
    let home = Home {
        goblins,
        ..Home::default()
    };
    let mut run = Run::at_home(8, 1, None, home, Default::default());
    let yard = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Yard)
        .expect("the yard is north of the Undercroft");
    run.enter_for_test(yard);
    run
}

#[test]
fn the_yard_is_through_the_undercrofts_north_door() {
    let mut run = Run::at_home(8, 1, None, Home::default(), Default::default());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 0.3);
    run.step(&BTreeMap::new());
    assert_eq!(run.room().kind, RoomKind::Yard);
    assert_eq!(run.phase, Phase::Exploring, "quintains are no fight");
    assert!(!run.barred(), "and bar no doors");
    assert_eq!(
        run.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Dummy)
            .count(),
        3
    );
}

#[test]
fn quintains_count_the_damage_never_fall_and_charge_an_ultimate() {
    let mut run = in_the_yard(0);
    let (qx, qy) = (
        yard::QUINTAINS[1].0 * TILE_UNITS,
        yard::QUINTAINS[1].1 * TILE_UNITS,
    );
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (qx, qy + 6.0);
    let fire = BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            fire: true,
            ..Input::default()
        },
    )]);
    for _ in 0..(3 * HZ) {
        run.step(&fire);
    }
    let dummy = run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Dummy)
        .min_by(|a, b| (a.x - qx).abs().total_cmp(&(b.x - qx).abs()))
        .unwrap();
    assert!(dummy.dir.1 > 0.0, "it shows what it took");
    assert_eq!(dummy.hp, dummy.max_hp, "and stands");
    assert!(
        run.players[&1].ult_charge > 0,
        "the blows charge the ultimate"
    );
    assert_eq!(run.enemies.len(), 3);
}

#[test]
fn grubbins_keeps_a_stall_only_once_a_goblin_got_away() {
    let run = in_the_yard(0);
    assert!(run.stall.is_empty());
    let run = in_the_yard(1);
    assert_eq!(run.stall.len(), 3);
    assert!(run.stall.iter().all(|id| run.book.get(id).is_some()));
    // The same three for the whole delve.
    assert_eq!(in_the_yard(1).stall, run.stall);
}

#[test]
fn a_stall_plate_orders_its_ware_and_a_sale_lands_at_the_knights_feet() {
    let mut run = in_the_yard(1);
    let (c, r, w, h) = yard::STALL_PLATES[1];
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Input::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&fire);
    }
    assert_eq!(run.orders.len(), 1);
    assert_eq!(run.orders[0].station, home::Station::StallB);
    let ware = run.stall[1].clone();
    assert_eq!(run.sell(1, 1).as_deref(), Some(ware.as_str()));
    assert!(run.stall[1].is_empty(), "sold");
    assert!(run.room().items.iter().any(|i| i.card == ware));
    assert_eq!(run.sell(1, 1), None, "only once");
}
