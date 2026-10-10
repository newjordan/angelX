use super::*;
use crate::drive::together_realm::Spoil;

/// Kill everything in the room and every wave still to come.
fn clear_all(run: &mut Run) {
    run.calm_for_test();
    for _ in 0..12 {
        for enemy in &mut run.enemies {
            enemy.hp = 0;
        }
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
        if run.enemies.is_empty() && !run.waves.pending() {
            break;
        }
    }
}

fn slay_the_dragon() -> Run {
    let mut run = Run::new(9, 1, Some("Friend"));
    run.lair_for_test();
    assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::Dragon));
    clear_all(&mut run);
    run
}

#[test]
fn the_dragon_opens_the_deep_and_a_light_home() {
    let run = slay_the_dragon();
    assert_eq!(run.phase, Phase::Exploring, "the delve goes on");
    assert!(run.light.is_some(), "a light home");
    assert_eq!(run.triumph, Some(Triumph::Dragon));
    let stairs = (0..run.room().rows as i32)
        .flat_map(|r| (0..run.room().cols as i32).map(move |c| (c, r)))
        .filter(|&(c, r)| run.room().tile(c, r) == Tile::Stairs)
        .count();
    assert_eq!(stairs, 4, "and stairs down");
    assert!(run.bank.iter().any(|h| h.why == "the dragon is slain"));
    assert!(run.cues.iter().any(|c| c == "the_deep"));
}

#[test]
fn the_light_takes_the_party_home_with_everything() {
    let mut run = slay_the_dragon();
    let (lx, ly) = run.light.unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    hero.carried.add(Spoil::Gold, 40);
    (hero.x, hero.y) = (lx, ly);
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Won);
    assert!(run.cues.iter().any(|c| c == "homeward"));
    assert!(
        run.bank.iter().any(|h| h.spoils.get(Spoil::Gold) >= 40),
        "everything carried comes home whole"
    );
}

#[test]
fn the_stairs_go_down_into_the_archive_the_fungal_deep_and_the_unknown() {
    let mut run = slay_the_dragon();
    // Off the stairs first (a dragon falling at your feet drops no one).
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (6.0, 6.0);
    run.step(&BTreeMap::new());
    let (w, h) = (run.room().width(), run.room().height());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (w / 2.0, h / 2.0 + 3.0);
    run.step(&BTreeMap::new());
    assert_eq!(run.floor(), 4);
    assert_eq!(run.dungeon.pack, Pack::Archive);
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    run.descend_for_test();
    assert_eq!((run.floor(), run.dungeon.pack), (5, Pack::Fungal));
    run.descend_for_test();
    assert_eq!((run.floor(), run.dungeon.pack), (6, Pack::Unknown));
    assert!(run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Threshold));
    assert!(run.valid_snapshot());
    // There is nothing below the Unknown.
    run.descend_for_test();
    assert_eq!(run.floor(), 6);
}

#[test]
fn the_deep_keeps_its_own_guardians() {
    let mut run = Run::new(9, 1, None);
    run.lair_for_test();
    for pack in [Pack::Archive, Pack::Fungal] {
        run.descend_for_test();
        assert_eq!(run.dungeon.pack, pack);
        let stairs = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(stairs);
        let boss = run
            .enemies
            .iter()
            .find_map(|e| e.boss)
            .map(|i| run.bosses[usize::from(i)].clone())
            .expect("a guardian on the stairs");
        assert_eq!(boss.only_in, pack);
        assert!(
            run.cues
                .iter()
                .any(|c| c == &format!("boss_rise:{}", boss.id))
        );
    }
}

#[test]
fn the_unknown_is_a_trial_then_the_grail() {
    let mut run = Run::new(9, 1, Some("Friend"));
    run.lair_for_test();
    for _ in 0..3 {
        run.descend_for_test();
    }
    let threshold = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Threshold)
        .unwrap();
    run.enter_for_test(threshold);
    assert_eq!(run.phase, Phase::Fighting);
    assert_eq!(run.waves.total, 4, "everything it has met, sent up to meet you");
    // Fight through the waves (they keep coming until the last is down).
    for _ in 0..30 {
        if run.phase != Phase::Fighting {
            break;
        }
        for enemy in &mut run.enemies {
            enemy.hp = 0;
        }
        for _ in 0..(4 * HZ) {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
            if !run.enemies.is_empty() || run.phase != Phase::Fighting {
                break;
            }
        }
    }
    assert_eq!(run.phase, Phase::Won);
    assert_eq!(run.triumph, Some(Triumph::Grail));
    assert!(run.cues.iter().any(|c| c == "grail"));
    for id in [1, 2] {
        let banked: u32 = run
            .bank
            .iter()
            .filter(|h| h.hero == id)
            .map(|h| h.spoils.get(Spoil::Scale))
            .sum();
        assert!(banked >= 1, "knight {id} carries a scale of it home");
    }
}

#[test]
fn deep_floors_shed_their_kins_materials() {
    let book = Book::builtin();
    let mut rng = Rng::new(3);
    for (pack, kin) in [
        (Pack::Archive, [Spoil::Bone, Spoil::Wax]),
        (Pack::Fungal, [Spoil::Ore, Spoil::Gem]),
    ] {
        let rolled: Vec<String> = (0..200)
            .filter_map(|_| book.roll_spoil(&mut rng, pack))
            .collect();
        assert!(
            kin.iter().any(|s| rolled.iter().any(|r| r == s.word())),
            "{pack:?} drops its kin's materials: {rolled:?}"
        );
    }
}

#[test]
fn wrens_landings_reach_the_deep() {
    let mut home = home::Home {
        deepest: 5,
        landing: 5,
        ..home::Home::default()
    };
    home.levels.insert(home::Station::Map, 4);
    assert_eq!(home.landings(), vec![1, 2, 3, 4, 5]);
    let mut run = Run::at_home(4, 1, None, home, Default::default());
    run.descend_for_test();
    assert_eq!((run.floor(), run.dungeon.pack), (5, Pack::Fungal));
}
