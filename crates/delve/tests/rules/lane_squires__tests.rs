use super::*;

#[test]
fn old_homes_restore_a_squire_company() {
    let home: home::Home = serde_json::from_str("{}").unwrap();
    assert!(
        home.lane_squires.is_default(),
        "old saves need a default company"
    );
    let saved = serde_json::to_value(&home).unwrap();
    assert!(
        saved.get("lane_squires").is_none(),
        "idle companies must preserve legacy bytes"
    );
    let restored: home::Home = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.lane_squires, home.lane_squires);
}

#[test]
fn each_knight_hires_trains_and_selects_only_one() {
    use lane_squires::*;
    let mut company = Company::default();
    company.refresh(41);
    let mut gold = 10_000;
    let first = company.offers[0].id;
    assert!(company.hire("Ada", 0, &mut gold));
    assert!(company.select("Ada", 0));
    assert_eq!(company.active("Ada").unwrap().id, first);
    assert!(company.hire("Ada", 0, &mut gold));
    assert!(company.select("Ada", 1));
    assert_ne!(company.active("Ada").unwrap().id, first);
    assert!(company.active("Bea").is_none());
    assert!(company.train("Ada", &mut gold));
    assert_eq!(company.active("Ada").unwrap().level, 2);
    let before = gold;
    assert!(!company.train("Ada", &mut gold));
    assert_eq!(gold, before);
    assert!(company.upgrade(Building::Yard, &mut gold));
    assert_eq!(company.max_level(), 4);
}

#[test]
fn board_refresh_is_deterministic_and_upgrades_improve_each_step() {
    use lane_squires::*;
    let mut a = Company::default();
    let mut gold = 10_000;
    a.refresh(12);
    let before = a.clone();
    a.refresh(12);
    assert_eq!(a, before);
    let wage = a.wage(&a.offers[0]);
    assert!(a.upgrade(Building::Hall, &mut gold));
    assert!(a.wage(&a.offers[0]) < wage);
    assert_eq!(a.cap(), 4);
    assert!(a.upgrade(Building::Board, &mut gold));
    assert!(a.upgrade(Building::Board, &mut gold));
    a.refresh(13);
    assert_eq!(a.offers.len(), 5);
    assert!(a.offers.iter().all(|r| r.level == 2));
    let saved = serde_json::to_string(&a).unwrap();
    assert_eq!(serde_json::from_str::<Company>(&saved).unwrap(), a);
}

#[test]
fn selected_squire_joins_and_recovers_without_replacing_minions() {
    let mut run = Run::new(51, 1, None);
    let who = home::Home::knight_key(&run.players[&1]);
    run.home.lane_squires.refresh(51);
    let mut gold = 1000;
    assert!(run.home.lane_squires.hire(&who, 0, &mut gold));
    assert!(run.home.lane_squires.select(&who, 0));
    run.tick_lane_squires();
    assert_eq!(run.lane_squires.len(), 1);
    run.lane_squires[0].hp = 0;
    run.enemies.clear();
    run.tick_lane_squires();
    assert!(run.lane_squires[0].hp > 0);
    assert!(run.hireling.is_none());
}

fn companion(kind: lane_squires::Kind, level: u8) -> Run {
    let mut run = Run::new(81, 1, None);
    run.enemies.clear();
    let who = home::Home::knight_key(&run.players[&1]);
    let r = lane_squires::Recruit {
        id: 1,
        name: "Pip".into(),
        kind,
        level,
    };
    run.home.lane_squires.knights.insert(
        who,
        lane_squires::Roster {
            members: vec![r],
            active: Some(1),
            ..Default::default()
        },
    );
    run.tick_lane_squires();
    run.lane_squires[0].grace = 0;
    run.lane_squires[0].cooldown = 0;
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (30.0, 24.0);
    (run.lane_squires[0].x, run.lane_squires[0].y) = (32.0, 24.0);
    run
}

#[test]
fn healer_mends_lowest_fraction_and_cleanses_without_resurrecting_early() {
    let mut run = companion(lane_squires::Kind::Healer, 2);
    run.join("Bea");
    let a = run.players.get_mut(&1).unwrap();
    a.hp = 80;
    a.max_hp = 100;
    let b = run.players.get_mut(&2).unwrap();
    b.hp = 20;
    b.max_hp = 100;
    b.chilled = HZ;
    b.hexed = HZ;
    (b.x, b.y) = (31.0, 24.0);
    run.tick_lane_squires();
    assert_eq!(run.players[&1].hp, 80);
    assert!(run.players[&2].hp > 20);
    assert_eq!(run.players[&2].hexed, 0);
    assert_eq!(run.players[&2].chilled, 0);
}

#[test]
fn ranger_marks_a_target_for_the_owner_and_ninja_throws_stars() {
    let mut run = companion(lane_squires::Kind::Ranger, 2);
    run.spawn_at(EnemyKind::Slime, 38.0, 24.0);
    run.enemies[0].age = TELEGRAPH;
    run.tick_lane_squires();
    assert_eq!(run.minions.quarry.get(&1), Some(&run.enemies[0].id));
    assert!(
        run.projectiles
            .iter()
            .any(|p| !p.hostile && p.kind == Shot::Arrow)
    );
    let mut ninja = companion(lane_squires::Kind::Ninja, 1);
    ninja.spawn_at(EnemyKind::Slime, 38.0, 24.0);
    ninja.enemies[0].age = TELEGRAPH;
    ninja.tick_lane_squires();
    assert!(
        ninja
            .projectiles
            .iter()
            .any(|p| !p.hostile && p.kind == Shot::Blade)
    );
}

#[test]
fn alchemist_cannot_bomb_through_walls() {
    let mut run = companion(lane_squires::Kind::Alchemist, 2);
    let c = (35.0 / TILE_UNITS) as usize;
    for row in 1..run.room().rows - 1 {
        run.dungeon.rooms[run.at].set(c, row, Tile::Wall);
    }
    run.spawn_at(EnemyKind::Slime, 43.0, 24.0);
    run.enemies[0].age = TELEGRAPH;
    let hp = run.enemies[0].hp;
    run.tick_lane_squires();
    assert_eq!(run.enemies[0].hp, hp);
}

#[test]
fn six_knights_get_one_each_and_legacy_runs_restore_without_companions() {
    let mut run = companion(lane_squires::Kind::Ninja, 4);
    for id in 2..=6 {
        run.join_seat(id, &format!("Knight {id}"));
        let who = home::Home::knight_key(&run.players[&id]);
        let r = lane_squires::Recruit {
            id,
            name: "Kit".into(),
            kind: lane_squires::Kind::ALL[(id as usize - 1) % 5],
            level: 2,
        };
        run.home.lane_squires.knights.insert(
            who,
            lane_squires::Roster {
                members: vec![r],
                active: Some(id),
                ..Default::default()
            },
        );
    }
    run.tick_lane_squires();
    run.tick_lane_squires();
    assert_eq!(run.lane_squires.len(), 6);
    let mut json = serde_json::to_value(&run).unwrap();
    let restored: Run = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(restored.lane_squires, run.lane_squires);
    json.as_object_mut().unwrap().remove("lane_squires");
    json.as_object_mut().unwrap().remove("lane_squire_holds");
    json["home"].as_object_mut().unwrap().remove("lane_squires");
    let legacy: Run = serde_json::from_value(json).unwrap();
    assert!(legacy.lane_squires.is_empty());
    assert!(legacy.home.lane_squires.knights.is_empty());
}

#[test]
fn yard_hold_hires_once_and_replays_exact_treasury_charge() {
    use lane_squires::*;
    let mut run = Run::at_home(93, 7, None, home::Home::default(), Default::default());
    let yard = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::SquireLanes)
        .unwrap();
    run.enter_for_test(yard);
    run.treasury.add(crate::realm::Spoil::Gold, 1000);
    run.tick_lane_squire_yard(&BTreeMap::new());
    let mut durable = Company::default();
    let mut bank = run.treasury.clone();
    durable.settle(&mut bank, &run.lane_squire_orders);
    run.lane_squire_orders.clear();
    let h = run.players.get_mut(&1).unwrap();
    h.x = 3.0 * TILE_UNITS;
    h.y = 3.5 * TILE_UNITS;
    let inputs = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    let wage = run.home.lane_squires.wage(&run.home.lane_squires.offers[0]);
    for _ in 0..home::BUY_HOLD * 3 {
        run.tick_lane_squire_yard(&inputs);
    }
    let who = home::Home::knight_key(&run.players[&1]);
    assert_eq!(run.home.lane_squires.knights[&who].members.len(), 1);
    assert_eq!(run.treasury.get(crate::realm::Spoil::Gold), 1000 - wage);
    assert_eq!(run.lane_squire_orders.len(), 1);
    assert!(durable.settle(&mut bank, &run.lane_squire_orders));
    assert_eq!(durable, run.home.lane_squires);
    assert_eq!(bank, run.treasury);
    // Holding on the same plate does not buy again; release rearms it.
    run.tick_lane_squire_yard(&BTreeMap::new());
    for _ in 0..home::BUY_HOLD {
        run.tick_lane_squire_yard(&inputs);
    }
    assert_eq!(run.home.lane_squires.knights[&who].members.len(), 2);
}

#[test]
fn insufficient_funds_full_roster_and_capped_buildings_leave_gold_alone() {
    use lane_squires::*;
    let mut c = Company::default();
    c.refresh(300);
    let mut gold = 0;
    assert!(!c.hire("Ada", 0, &mut gold));
    assert!(!c.upgrade(Building::Hall, &mut gold));
    gold = 100_000;
    for _ in 0..3 {
        assert!(c.upgrade(Building::Hall, &mut gold));
    }
    let before = gold;
    assert!(!c.upgrade(Building::Hall, &mut gold));
    assert_eq!(gold, before);
    for day in 1..=3 {
        c.refresh(day);
        while !c.offers.is_empty() && c.knights.get("Ada").map_or(0, |r| r.members.len()) < c.cap()
        {
            assert!(c.hire("Ada", 0, &mut gold));
        }
    }
    c.refresh(44);
    let before = gold;
    assert!(!c.hire("Ada", 0, &mut gold));
    assert_eq!(gold, before);
    assert!(!c.select("Ada", 99));
}

#[test]
fn healer_assists_a_skull_only_at_trained_level_and_recovers_at_its_location() {
    let mut run = companion(lane_squires::Kind::Healer, 4);
    run.join("Bea");
    let fallen = run.players.get_mut(&2).unwrap();
    fallen.hp = 0;
    fallen.skull_at = Some((32.0, 24.0));
    (fallen.x, fallen.y) = (50.0, 40.0);
    run.tick_lane_squires();
    assert!(run.players[&2].hp > 0);
    assert_eq!((run.players[&2].x, run.players[&2].y), (32.0, 24.0));
    assert_eq!(run.players[&2].skull_at, None);
    assert!(run.players[&2].invulnerable > 0);
}

#[test]
fn shield_intercepts_a_fast_swept_arrow_before_its_knight() {
    let mut run = hireling::testing::fight_with(false, &[]);
    let who = home::Home::knight_key(&run.players[&1]);
    let r = lane_squires::Recruit {
        id: 1,
        name: "Bram".into(),
        kind: lane_squires::Kind::Shield,
        level: 2,
    };
    run.home.lane_squires.knights.insert(
        who,
        lane_squires::Roster {
            members: vec![r],
            active: Some(1),
            ..Default::default()
        },
    );
    run.tick_lane_squires();
    (run.lane_squires[0].x, run.lane_squires[0].y, run.lane_squires[0].grace) = (15.0, 12.0, 0);
    let hp = run.players[&1].hp;
    let shield_hp = run.lane_squires[0].hp;
    run.projectiles.push(Projectile {
        x: 20.0,
        y: 12.0,
        vx: -300.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Orb,
        look: None,
        damage: 20,
        pierce: 0,
        last_hit: None,
        ttl: HZ,
        empowered: false,
        traits: Default::default(),
    });
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].hp, hp);
    assert!(run.lane_squires[0].hp <= shield_hp - 10);
    assert!(!run.projectiles.iter().any(|p| p.hostile && p.ttl > 0));
}

#[test]
fn ninja_backstab_and_smoke_unlock_with_training() {
    let mut run = companion(lane_squires::Kind::Ninja, 4);
    (
        run.players.get_mut(&1).unwrap().x,
        run.players.get_mut(&1).unwrap().y,
    ) = (30.0, 24.0);
    (run.lane_squires[0].x, run.lane_squires[0].y) = (34.0, 24.0);
    run.spawn_at(EnemyKind::Slime, 33.0, 24.0);
    let enemy = run.enemies.last_mut().unwrap();
    enemy.age = TELEGRAPH;
    enemy.hp = 1000;
    enemy.max_hp = 1000;
    run.tick_lane_squires();
    assert_eq!(run.enemies[0].hp, 1000 - lane_squires::Kind::Ninja.power(4) * 2);
    run.projectiles.push(Projectile {
        x: run.lane_squires[0].x,
        y: run.lane_squires[0].y,
        vx: 0.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Orb,
        look: None,
        damage: 5,
        pierce: 0,
        last_hit: None,
        ttl: HZ,
        empowered: false,
        traits: Default::default(),
    });
    run.lane_squires[0].grace = 0;
    run.tick_lane_squires();
    assert!(run.lane_squires[0].smoke > 0);
}

#[test]
fn wire_and_drawing_preserve_company_and_blend_only_positions() {
    let mut run = companion(lane_squires::Kind::Ranger, 2);
    let copy = run.wire_copy();
    assert_eq!(copy.lane_squires, run.lane_squires);
    assert_eq!(copy.home.lane_squires, run.home.lane_squires);
    let before = run.pose();
    let x = run.lane_squires[0].x;
    run.tick += 1;
    run.lane_squires[0].x += 0.5;
    let now = run.lane_squires[0].x;
    let half = run.drawn_with(&before, 0.5, None, |r| r.lane_squires[0].x);
    assert!((half - (x + now) * 0.5).abs() < 0.001);
    assert_eq!(run.lane_squires[0].x, now);
}

#[test]
fn squire_services_do_not_overlap_ector_or_grubbins_plates() {
    let company = lane_squires::Company {
        board: 3,
        hall: 3,
        ..Default::default()
    };
    for (_, a) in lane_squires::plates(&company) {
        for b in talents::LECTERNS.into_iter().chain(yard::STALL_PLATES) {
            let overlap = a.0 < b.0 + b.2 && a.0 + a.2 > b.0 && a.1 < b.1 + b.3 && a.1 + a.3 > b.1;
            assert!(!overlap, "squire plate {a:?} overlaps existing plate {b:?}");
        }
    }
}
