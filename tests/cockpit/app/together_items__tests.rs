use super::*;

/// An open first-floor room of the Crypt, its fight kept on by a bat frozen
/// in the far corner, knight one standing in the middle.
fn quiet_room(pair: bool) -> Run {
    let mut run = Run::new(4, 1, pair.then_some("Friend"));
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
    run.spawn_at_for_test(EnemyKind::Bat, 44.0, 3.0);
    if let Some(bat) = run.enemies.last_mut() {
        bat.frozen = u32::MAX / 2;
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (10.0, 14.0, 0);
    run
}

/// Knight one plays the card `id` from its hand.
fn play(run: &mut Run, id: &str) {
    let hero = run.players.get_mut(&1).unwrap();
    hero.hand = vec![id.to_string()];
    hero.play_cooldown = 0;
    run.step(&BTreeMap::from([(
        1,
        Input {
            play: 1,
            ..Default::default()
        },
    )]));
}

#[test]
fn the_items_are_in_the_delves_book() {
    let book = cards::Book::builtin();
    for (id, effect) in [
        ("fae-dagger", "blink"),
        ("pendragon-sceptre", "immune"),
        ("censer", "censer"),
    ] {
        let card = book.get(id).unwrap_or_else(|| panic!("{id} in the book"));
        assert_eq!(card.kind, cards::Kind::Play);
        assert!(
            card.effects.iter().any(|e| e.label().starts_with(effect)),
            "{id}: {:?}",
            card.effects
        );
    }
}

#[test]
fn a_fae_dagger_puts_the_knight_where_it_aimed() {
    let mut run = quiet_room(false);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    play(&mut run, "fae-dagger");
    run.step(&BTreeMap::new());
    let x = run.players[&1].x;
    assert!((x - 22.0).abs() < 1.0, "twelve steps east: {x}");
    assert!(run.cues.iter().any(|c| c == "blink"));
    // Aimed at the wall: it stops short of it, inside the room.
    let mut run = quiet_room(false);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.aim_x, hero.aim_y) = (6.0, -1.0, 0.0);
    play(&mut run, "fae-dagger");
    run.step(&BTreeMap::new());
    let x = run.players[&1].x;
    assert!(x > 1.0 && x < 6.0, "against the west wall: {x}");
}

#[test]
fn a_pendragon_sceptre_turns_hex_chill_and_web_aside() {
    let mut run = quiet_room(false);
    play(&mut run, "pendragon-sceptre");
    assert!(run.players[&1].immune > 0);
    assert!(run.cues.iter().any(|c| c == "sceptre"));
    run.hex(1);
    assert!(!run.players[&1].frog(), "no hex takes");
    // A web underfoot doesn't slow it.
    let walk = |immune: bool| {
        let mut run = quiet_room(false);
        if immune {
            play(&mut run, "pendragon-sceptre");
        }
        let (x, y) = (run.players[&1].x, run.players[&1].y);
        run.webs.push(hunters::Web {
            x: x + 2.0,
            y,
            left: 10 * HZ,
        });
        for _ in 0..HZ / 3 {
            run.step(&BTreeMap::from([(
                1,
                Input {
                    move_x: 1,
                    ..Default::default()
                },
            )]));
        }
        run.players[&1].x - x
    };
    assert!(
        walk(true) > walk(false) * 1.3,
        "the web holds the others only"
    );
}

#[test]
fn a_censer_mends_every_knight_near_and_not_those_far() {
    let mut run = quiet_room(true);
    for (id, (x, y)) in [(1, (10.0, 14.0)), (2, (14.0, 14.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.hp) = (x, y, 30);
    }
    play(&mut run, "censer");
    assert_eq!(run.players[&1].hp, 70);
    assert_eq!(run.players[&2].hp, 70);
    assert!(run.cues.iter().any(|c| c == "censer"));
    let mut run = quiet_room(true);
    for (id, (x, y)) in [(1, (10.0, 14.0)), (2, (40.0, 14.0))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.hp) = (x, y, 30);
    }
    play(&mut run, "censer");
    assert_eq!(run.players[&2].hp, 30, "too far for the pulse");
}
