use super::*;

/// Floor one's entrance as a fight, knight one at its middle, the Pit Tyrant
/// at `(x, y)`.
fn pit_with(x: f32, y: f32) -> Run {
    let mut run = Run::new(21, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(EnemyKind::PitTyrant, x, y);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.invulnerable = 0;
    run.phase = Phase::Fighting;
    run
}

#[test]
fn about_half_the_floors_from_two_to_five_hang_a_pit_off_a_fight_room() {
    let mut pits = 0;
    let mut floors = 0;
    for seed in 0..40 {
        for depth in 1..=DEEPEST {
            let floor = layout::floor(depth, Pack::at(depth, Pack::Crypt), &mut Rng::new(seed));
            let found: Vec<&Room> = floor
                .rooms
                .iter()
                .filter(|r| r.kind == RoomKind::Pit)
                .collect();
            if !(2..DEEPEST).contains(&depth) {
                assert!(found.is_empty(), "floor {depth}");
                continue;
            }
            floors += 1;
            assert!(found.len() <= 1);
            if let Some(pit) = found.first() {
                pits += 1;
                assert_eq!(pit.doors.iter().filter(|&&d| d).count(), 1, "a dead end");
                assert_eq!(pit.roster, [EnemyKind::PitTyrant]);
            }
        }
    }
    assert!(
        pits * 4 > floors && pits * 4 < floors * 3,
        "{pits} pits on {floors} floors"
    );
}

#[test]
fn the_slam_gathers_in_a_ring_then_strikes_and_throws_whoever_is_in_it() {
    let mut run = pit_with(27.0, 14.0);
    let mut ring = None;
    let mut hp = 0;
    for _ in 0..8 * HZ {
        // Held still, and unhurt by anything but the slam.
        run.projectiles.clear();
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (24.0, 14.0);
        hero.hp = hero.max_hp;
        hero.invulnerable = 0;
        hp = hero.hp;
        run.step(&BTreeMap::new());
        if ring.is_none() {
            ring = run.slams.first().map(|s| (s.x, s.y));
        } else if run.slams.is_empty() {
            break;
        }
    }
    let (x, y) = ring.expect("a ring gathers first");
    let hero = &run.players[&1];
    assert!(hero.hp < hp, "the slam lands");
    assert!(
        (hero.x - x).hypot(hero.y - y) > (24.0 - x).hypot(14.0 - y) + 1.0,
        "and throws the knight clear of it"
    );
}

#[test]
fn the_pit_tyrant_leaves_the_talisman_and_a_knight_takes_it() {
    let mut run = pit_with(30.0, 14.0);
    run.enemies[0].hp = 0;
    run.step(&BTreeMap::new());
    assert!(run.cues.iter().any(|c| c == "pit_fall"));
    assert!(run.feats.contains(&"pit_tyrant"));
    let talisman = run
        .room()
        .items
        .iter()
        .find(|i| i.card == "talisman")
        .map(|i| (i.x, i.y))
        .expect("the Talisman where it fell");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = talisman;
    hero.hp = 10;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert!(hero.talisman, "carried");
    assert_eq!(hero.hp, hero.max_hp, "and mends");
    assert!(run.cues.iter().any(|c| c == "talisman"));
}

#[test]
fn the_talisman_raises_a_fallen_knight_whole_before_any_second_wind() {
    let mut run = pit_with(46.0, 26.0);
    run.enemies.clear();
    let hero = run.players.get_mut(&1).unwrap();
    hero.talisman = true;
    hero.winds = 1;
    hero.hp = 0;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert_eq!(hero.hp, hero.max_hp);
    assert!(!hero.talisman, "once");
    assert_eq!(hero.winds, 1, "the chapel's wind is kept");
    assert!(run.cues.iter().any(|c| c == "talisman_used"));
}

#[test]
fn friends_see_the_slam_gather() {
    let mut run = pit_with(27.0, 14.0);
    run.slams.push(pit::Slam {
        x: 27.0,
        y: 14.0,
        left: 20,
    });
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.slams.clear();
    assert!(friend.apply_live(live));
    assert_eq!(friend.slams.len(), 1);
}
