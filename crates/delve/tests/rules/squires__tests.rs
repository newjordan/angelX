use super::*;

fn purse() -> Spoils {
    let mut gold = Spoils::default();
    gold.add(Spoil::Gold, 10000);
    gold
}
fn intent(company: &Company, name: &str, action: &str) -> String {
    let key: String = name.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "country:squire:{}:{key}:{action}",
        company.revision.get(&key).copied().unwrap_or(0)
    )
}
#[test]
fn recruits_refresh_and_buildings_change_each_step() {
    let mut c = Company::default();
    let mut gold = purse();
    assert_eq!(c.offers().len(), 3);
    let before = c.offers();
    c.hire(0, &mut gold).unwrap();
    assert!(c.hire(0, &mut gold).is_err());
    assert_eq!(gold.get(Spoil::Gold), 9920);
    for building in 0..3 {
        c.upgrade(building, &mut gold).unwrap();
    }
    assert_eq!(c.offers().len(), 4);
    assert_eq!(c.cap(), 5);
    assert_eq!(c.wage(&before[0]), 70);
    let mut marks = BTreeMap::new();
    marks.insert("country:squire-day:0".into(), 1);
    settle(&mut c, &mut gold, &marks, &mut vec![]);
    assert_ne!(c.offers(), before);
    assert!(c.hired.is_empty());
    let after = c.clone();
    settle(&mut c, &mut gold, &marks, &mut vec![]);
    assert_eq!(c, after);
}
#[test]
fn no_gold_no_purchase_or_upgrade() {
    let mut c = Company::default();
    let mut gold = Spoils::default();
    assert!(c.hire(0, &mut gold).is_err());
    assert!(c.upgrade(0, &mut gold).is_err());
    assert!(c.is_empty());
}
#[test]
fn one_active_each_no_shared_body_and_training_needs_practice() {
    let mut c = Company::default();
    let mut gold = purse();
    c.hire(0, &mut gold).unwrap();
    c.hire(1, &mut gold).unwrap();
    c.cycle("one").unwrap();
    c.cycle("two").unwrap();
    assert_ne!(c.active("one").unwrap().id, c.active("two").unwrap().id);
    assert!(c.train("one", &mut gold).is_err());
    c.roster[0].drills = 1;
    c.train("one", &mut gold).unwrap();
    assert_eq!(c.roster[0].level, 2);
    c.roster[0].drills = 9;
    assert!(c.train("one", &mut gold).is_err());
    c.upgrade(2, &mut gold).unwrap();
    c.train("one", &mut gold).unwrap();
    assert_eq!(c.roster[0].level, 3);
}
#[test]
fn settlement_replays_do_not_charge_twice_and_two_seats_work() {
    let mut c = Company::default();
    let mut gold = purse();
    let mut marks = BTreeMap::new();
    marks.insert(intent(&c, "one", "hire:0"), 1);
    marks.insert(intent(&c, "two", "hire:1"), 2);
    let (_, mut news) = (false, vec![]);
    settle(&mut c, &mut gold, &marks, &mut news);
    assert_eq!(c.roster.len(), 2);
    let before = (c.clone(), gold.clone());
    settle(&mut c, &mut gold, &marks, &mut news);
    assert_eq!((c, gold), before);
}
#[test]
fn legacy_country_and_company_restore_and_intents_are_bounded() {
    let country: countryside::Country = serde_json::from_str("{}").unwrap();
    assert!(country.squires.is_empty());
    assert!(!serde_json::to_string(&country).unwrap().contains("squires"));
    let mut c = Company::default();
    c.hire(0, &mut purse()).unwrap();
    c.cycle("Name: with / punctuation").unwrap();
    assert_eq!(
        serde_json::from_str::<Company>(&serde_json::to_string(&c).unwrap()).unwrap(),
        c
    );
    let mark = intent(&c, "Name: with / punctuation", "train:0");
    assert!(countryside::intent_valid(&mark, 6));
    assert!(!countryside::intent_valid(&mark, 7));
    for bad in [
        "country:squire:0:zz:hire:0",
        "country:squire:0:61:hire:9",
        "country:squire:0:61:upgrade:3",
    ] {
        assert!(!intent_key(bad));
    }
}
#[test]
fn six_knights_each_get_one_and_room_change_recovers_downed_squires() {
    let mut run = Run::new(7, 1, Some("Pip"));
    for id in 3..=6 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.home.countryside.squires.hall = 3;
    for (&id, h) in &run.players {
        let c = &mut run.home.countryside.squires;
        c.next += 1;
        c.roster.push(Squire {
            id: c.next,
            name: format!("Squire {id}"),
            kind: Kind::ALL[id as usize % 5],
            level: 3,
            drills: 0,
        });
        c.selected.insert(home::Home::knight_key(h), c.next);
    }
    run.tick_countryside_squires();
    assert_eq!(run.countryside_squires.len(), 6);
    run.tick_countryside_squires();
    assert_eq!(run.countryside_squires.len(), 6);
    run.countryside_squires[0].body.hp = 0;
    run.countryside_squires[0].room = usize::MAX;
    run.tick_countryside_squires();
    assert!(run.countryside_squires[0].body.hp > 0);
    let json = serde_json::to_string(&run).unwrap();
    let restored: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.countryside_squires.len(), 6);
}

#[test]
fn yard_uses_existing_interact_and_one_order_per_hold() {
    let mut run = Run::at_home(3, 1, None, home::Home::default(), purse());
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Yard)
        .unwrap();
    run.enter_for_test(room);
    let h = run.players.get_mut(&1).unwrap();
    h.x = PLATES[0].0 * TILE_UNITS;
    h.y = PLATES[0].1 * TILE_UNITS;
    let input = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    for _ in 0..home::BUY_HOLD - 1 {
        run.step(&input);
    }
    assert!(!run.marks.keys().any(|s| s.starts_with("country:squire:")));
    run.step(&input);
    assert_eq!(
        run.marks
            .keys()
            .filter(|s| s.starts_with("country:squire:"))
            .count(),
        1
    );
    let before = run.marks.clone();
    for _ in 0..home::BUY_HOLD {
        run.step(&input);
    }
    assert_eq!(run.marks, before);
    let (changed, _) =
        countryside::settle(&mut run.home.countryside, &mut run.treasury, &run.marks);
    assert!(changed);
    assert_eq!(run.home.countryside.squires.roster.len(), 1);
    assert_eq!(run.treasury.get(Spoil::Gold), 9920);
}

fn companion(kind: Kind, level: u8) -> Companion {
    let recruit = Squire {
        id: 1,
        name: "Kit".into(),
        kind,
        level,
        drills: 0,
    };
    let mut body = hireling::Hireling::minion(hireling::AllyKind::Beaumains, 0, 1, (12.0, 12.0), 1);
    body.hp = recruit.health();
    body.max_hp = body.hp;
    Companion {
        owner: 1,
        recruit,
        body,
        room: 0,
        depth: 1,
        ability: 0,
        smoke: 0,
        trick: 0,
        marked: None,
    }
}
fn fight() -> Run {
    hireling::testing::fight_with(false, &[])
}
#[test]
fn ninja_backstabs_vanishes_and_throws_owner_credited_stars() {
    let mut run = fight();
    run.spawn_at_for_test(EnemyKind::Slime, 13.0, 12.0);
    run.enemies[0].hp = 200;
    run.enemies[0].age = TELEGRAPH;
    run.enemies[0].wits.foe = 2;
    let mut c = companion(Kind::Ninja, 3);
    run.countryside_squire_action(&mut c, None);
    assert_eq!(run.enemies[0].hp, 200 - c.recruit.power() * 3);
    assert_eq!(c.smoke, HZ);
    c.body.x = 5.0;
    run.countryside_squire_action(&mut c, None);
    let star = run.projectiles.last().unwrap();
    assert_eq!(star.kind, Shot::Blade);
    assert_eq!(star.traits.owner, 1);
}
#[test]
fn healer_mends_lowest_ratio_cleanses_and_assists_a_skull() {
    let mut run = fight();
    run.join_seat(2, "Other");
    run.players.get_mut(&1).unwrap().hp = 70;
    let h = run.players.get_mut(&2).unwrap();
    h.x = 12.0;
    h.y = 12.0;
    h.hp = 10;
    h.hexed = 20;
    h.chilled = 20;
    let mut c = companion(Kind::Healer, 3);
    run.countryside_squire_action(&mut c, None);
    assert!(run.players[&2].hp > 10);
    assert_eq!(run.players[&2].hexed, 0);
    assert_eq!(run.players[&2].chilled, 0);
    for h in run.players.values_mut() {
        h.hp = h.max_hp;
    }
    run.players.get_mut(&2).unwrap().hp = 0;
    run.countryside_squire_action(&mut c, None);
    assert!(run.players[&2].hp > 0);
    assert!(run.players[&2].invulnerable > 0);
}
#[test]
fn ranger_marks_shield_taunts_and_alchemist_blasts_groups() {
    let mut run = fight();
    run.spawn_at_for_test(EnemyKind::Slime, 13.0, 12.0);
    run.spawn_at_for_test(EnemyKind::Slime, 14.0, 12.0);
    for e in &mut run.enemies {
        e.age = TELEGRAPH;
        e.hp = 200;
    }
    let mut ranger = companion(Kind::Ranger, 3);
    run.countryside_squire_action(&mut ranger, None);
    assert_eq!(run.projectiles.last().unwrap().kind, Shot::Arrow);
    assert_eq!(ranger.marked, Some(run.enemies[0].id));
    let mut tank = companion(Kind::Shieldbearer, 3);
    run.countryside_squire_action(&mut tank, None);
    let mut seen = vec![(0.0, 0.0)];
    roared(&[tank], &run.enemies[0], &mut seen);
    assert_eq!(seen, vec![(12.0, 12.0)]);
    let mut alchemist = companion(Kind::Alchemist, 3);
    let before: Vec<_> = run.enemies.iter().map(|e| e.hp).collect();
    run.countryside_squire_action(&mut alchemist, None);
    assert!(run.enemies.iter().zip(before).all(|(e, hp)| e.hp < hp));
    assert_eq!(run.blasts.len(), 1);
}
