use super::*;

/// Floor one of the Crypt, its first fight room emptied and calmed, knight
/// one standing on the left.
fn quiet_room() -> Run {
    let mut run = Run::new(4, 1, None);
    run.begin_in(Pack::Crypt);
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
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (10.0, 14.0, 0);
    run
}

fn keys(input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, input)])
}

#[test]
fn a_hexed_knight_is_a_frog_that_can_only_hop() {
    let mut run = quiet_room();
    run.spawn_at_for_test(EnemyKind::Bat, 40.0, 4.0);
    run.hex(1);
    assert!(run.players[&1].frog());
    assert!(run.cues.iter().any(|c| c == "hexed"));
    assert_eq!(run.found.as_ref().map(|f| f.2.as_str()), Some("Ribbit."));
    // It neither shoots, nor rolls.
    let shoot = keys(Input {
        fire: true,
        aim_x: 1,
        dash: true,
        ..Default::default()
    });
    for _ in 0..10 {
        run.step(&shoot);
    }
    assert!(
        run.projectiles.iter().all(|p| p.hostile),
        "a frog looses nothing"
    );
    assert_eq!(run.players[&1].rolling(), None, "nor rolls");
    // It hops, slowly.
    let walk = |frog: bool| {
        let mut run = quiet_room();
        if frog {
            run.hex(1);
        }
        let x = run.players[&1].x;
        for _ in 0..HZ / 2 {
            run.step(&keys(Input {
                move_x: 1,
                ..Default::default()
            }));
        }
        run.players[&1].x - x
    };
    let (knight, frog) = (walk(false), walk(true));
    assert!(frog > 0.0 && frog < knight * 0.8, "frog {frog} vs {knight}");
}

#[test]
fn the_hex_wears_off_and_wards_the_knight_for_a_while() {
    let mut run = quiet_room();
    run.hex(1);
    for _ in 0..HEX_TICKS {
        run.step(&BTreeMap::new());
    }
    assert!(!run.players[&1].frog(), "a knight again");
    run.hex(1);
    assert!(!run.players[&1].frog(), "warded: no second hex yet");
    for _ in 0..HEX_IMMUNE {
        run.step(&BTreeMap::new());
    }
    run.hex(1);
    assert!(run.players[&1].frog(), "and then it takes again");
}

#[test]
fn the_hexer_looses_a_bolt_at_the_nearest_knight_and_it_makes_a_frog() {
    let mut run = quiet_room();
    run.spawn_at_for_test(EnemyKind::Hexer, 22.0, 14.0);
    let mut bolt = false;
    for _ in 0..8 * HZ {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (10.0, 14.0);
        hero.hp = hero.max_hp;
        run.step(&BTreeMap::new());
        bolt |= run
            .projectiles
            .iter()
            .any(|p| p.hostile && p.kind == Shot::Hex && p.vx < 0.0);
        if run.players[&1].frog() {
            break;
        }
    }
    assert!(bolt, "a green bolt, toward the knight");
    assert!(run.players[&1].frog(), "and the knight is a frog");
}

#[test]
fn three_hexes_in_a_delve_is_kiss_me_im_a_knight() {
    let mut run = quiet_room();
    for _ in 0..3 {
        run.hex(1);
        run.players.get_mut(&1).unwrap().hexed = 0;
    }
    assert_eq!(run.hexes, 3);
    assert!(run.feats.contains(&"ribbit"));
}

#[test]
fn hexers_haunt_the_crypt_below_the_first_floor_the_fungal_deep_and_the_unknown() {
    let has = |pack: Pack, depth: u32| {
        pack.roster_at(depth)
            .iter()
            .any(|&(kind, _)| kind == EnemyKind::Hexer)
    };
    assert!(!has(Pack::Crypt, 1), "the first floor teaches");
    assert!(has(Pack::Crypt, 2));
    assert!(has(Pack::Fungal, 7));
    assert!(has(Pack::Unknown, 8));
    assert!(EnemyKind::Hexer.elite(), "one to a room");
}
