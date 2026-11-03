use super::*;

#[test]
fn experience_brings_levels_and_each_level_a_lesson() {
    let mut prowess = Prowess::default();
    assert_eq!((prowess.level(), prowess.waiting()), (1, 0));
    prowess.xp = LEVELS[0];
    assert_eq!((prowess.level(), prowess.waiting()), (2, 1));
    prowess.xp = LEVELS[3] + 1;
    assert_eq!((prowess.level(), prowess.waiting()), (5, 4));
    prowess.learned = vec![0, 1];
    assert_eq!(prowess.waiting(), 2);
    let names: Vec<&str> = prowess.talents().map(|(_, t)| t.name).collect();
    assert_eq!(names, [LESSONS[0][0].name, LESSONS[1][1].name]);
}

#[test]
fn sir_ector_teaches_only_what_has_been_earned() {
    let mut home = Home::default();
    assert!(home.learn("percival", 0).is_err(), "nothing earned yet");
    home.knights.entry("percival".into()).or_default().xp = LEVELS[1];
    assert_eq!(home.learn("percival", 1).unwrap().name, LESSONS[0][1].name);
    assert_eq!(home.learn("percival", 0).unwrap().name, LESSONS[1][0].name);
    assert!(
        home.learn("percival", 0).is_err(),
        "the third waits for level 4"
    );
    home.knights.get_mut("percival").unwrap().xp = LEVELS[3];
    assert!(home.learn("percival", 0).is_ok());
    assert!(home.learn("percival", 1).is_ok());
    assert!(home.learn("percival", 1).is_err(), "the fifth waits for level 6");
    assert_eq!(home.prowess("percival").learned, [1, 0, 0, 1]);
    assert_eq!(
        home.prowess("gareth"),
        Prowess::default(),
        "each knight his own"
    );
}

#[test]
fn the_road_runs_past_the_first_four_lessons_to_a_lesson_at_every_level() {
    assert_eq!(&LEVELS[..4], [600, 1800, 3600, 6000], "an old record keeps its level");
    assert!(LESSONS.len() > 4);
    assert!(LEVELS.windows(2).all(|w| w[0] < w[1]));
    let mut home = Home::default();
    home.knights.entry("frank".into()).or_default().xp = *LEVELS.last().unwrap();
    assert_eq!(home.prowess("frank").level(), LEVELS.len() as u32 + 1);
    for lesson in &LESSONS {
        assert_eq!(home.learn("frank", 1).unwrap().name, lesson[1].name);
    }
    assert!(home.learn("frank", 0).is_err(), "all he knows");
    assert_eq!(home.prowess("frank").waiting(), 0);
}

#[test]
fn every_talent_is_a_real_held_card_and_no_name_comes_twice() {
    let mut names = std::collections::BTreeSet::new();
    for talent in LESSONS.iter().flatten() {
        assert!(names.insert(talent.name), "{} twice", talent.name);
        let word = talent.effect.split(' ').next().unwrap();
        assert!(cards::cap(word) > 0, "{word} is not a held word");
        let value: u32 = talent.effect.split(' ').nth(1).unwrap().parse().unwrap();
        assert!(value <= cards::cap(word), "{}: over the deck's cap", talent.name);
    }
}

#[test]
fn twelve_lessons_on_one_knight_stay_under_the_decks_caps() {
    let mut home = Home::default();
    home.knights.insert(
        "you".into(),
        Prowess { xp: *LEVELS.last().unwrap(), learned: vec![1; LESSONS.len()] },
    );
    let run = Run::at_home(5, 1, None, home, Default::default());
    let hero = &run.players[&1];
    let held = hero.deck.iter().filter(|c| c.starts_with("home-talent-")).count();
    assert_eq!(held, LESSONS.len(), "{:?}", hero.deck);
    assert_eq!(hero.bonus.damage, 22, "Keen Edge and Finishing Stroke");
    assert_eq!(hero.bonus.boomerang, 1, "Homeward, the last lesson");
}

#[test]
fn a_player_on_a_kit_takes_their_own_lessons_down_the_stair() {
    let mut home = Home::default();
    home.knights.insert("frank".into(), Prowess { xp: LEVELS[0], learned: vec![1] });
    let mut run = Run::at_home(5, 1, None, home, Default::default());
    let hero = run.players.get_mut(&1).unwrap();
    hero.name = "Frank".into();
    run.outfit(1, knights::knight("percival").unwrap());
    run.rebuild_home(run.home.clone(), Default::default());
    let hero = &run.players[&1];
    assert_ne!(knight_key(hero), profile_key(hero));
    assert!(hero.deck.iter().any(|c| c == "home-talent-1"), "{:?}", hero.deck);
    assert_eq!(hero.bonus.damage, 12, "Keen Edge");
}

#[test]
fn what_falls_is_experience() {
    let mut run = Run::new(5, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 10.0);
    run.spawn_at_for_test(EnemyKind::Bat, 34.0, 10.0);
    run.phase = Phase::Fighting;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    let xp = (EnemyKind::Skeleton.bounty() / 25).max(1) + (EnemyKind::Bat.bounty() / 25).max(1);
    assert_eq!(run.marks.get("xp"), Some(&xp));
}

#[test]
fn holding_e_at_a_lectern_asks_for_its_lesson() {
    let mut run = Run::at_home(5, 1, None, Default::default(), Default::default());
    let yard = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Yard)
        .unwrap();
    run.enter_for_test(yard);
    let (c, r, w, _) = LECTERNS[1];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (
        (c as f32 + w as f32 / 2.0) * TILE_UNITS,
        (r as f32 + 0.5) * TILE_UNITS,
    );
    let fire = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    for _ in 0..home::BUY_HOLD + 2 {
        run.step(&fire);
    }
    let asked: Vec<home::Station> = run.orders.iter().map(|o| o.station).collect();
    assert_eq!(asked, [home::Station::LessonB], "once per hold");
}

#[test]
fn a_lesson_learned_goes_down_the_stair_as_a_held_card() {
    let mut home = Home::default();
    home.knights.insert(
        "you".into(),
        Prowess {
            xp: LEVELS[0],
            learned: vec![1],
        },
    );
    let run = Run::at_home(5, 1, None, home, Default::default());
    let hero = &run.players[&1];
    assert_eq!(knight_key(hero), "you");
    assert!(
        hero.deck.iter().any(|c| c == "home-talent-1"),
        "{:?}",
        hero.deck
    );
    assert_eq!(hero.bonus.damage, 12, "Keen Edge");
}
