use super::*;
#[allow(dead_code)]
#[path = "together_hireling__fixtures.rs"]
mod fixtures;
fn fight(kind: Kind, level: u8) -> Run {
    let mut r = fixtures::fight_with(false, &[]);
    let key = home::Home::knight_key(&r.players[&1]);
    let s = Squire {
        id: 1,
        name: "Pip".into(),
        kind,
        level,
        talent: 1,
    };
    r.home.squire_hall.rosters.insert(
        key,
        crate::realm::squires::Roster {
            active: Some(1),
            squires: vec![s],
        },
    );
    r.hall_squires_follow();
    r
}
#[test]
fn one_per_owner_survives_rooms_and_recovers_without_taking_loot() {
    let mut r = fight(Kind::Ninja, 3);
    r.hall_squires_follow();
    r.hall_squires_follow();
    assert_eq!(r.squire_hall.companions.len(), 1);
    let before = r.bank.clone();
    r.squire_hall.companions[0].body.hp = 0;
    r.tick_hall_squires();
    assert!(r.squire_hall.companions[0].body.hp > 0);
    assert_eq!(r.bank, before);
    r.squire_hall.companions[0].body.hp = 0;
    fixtures::foe_at(&mut r, EnemyKind::Slime, (20.0, 20.0), 100);
    r.tick_hall_squires();
    assert_eq!(r.squire_hall.companions[0].body.hp, 0);
    r.hall_squires_follow();
    assert!(r.squire_hall.companions[0].body.hp > 0);
    let decoded: Run = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(decoded.squire_hall, r.squire_hall);
}
#[test]
fn ninja_flanks_backstabs_vanishes_and_throws_stars() {
    let mut r = fight(Kind::Ninja, 3);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (16.0, 12.0), 100);
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 18.0;
    c.body.y = 12.0;
    c.body.invulnerable = 0;
    let hp = r.enemies[0].hp;
    r.tick_hall_squires();
    assert!(r.enemies[0].hp < hp);
    assert!(r.squire_hall.companions[0].smoke > 0);
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 10.0;
    c.body.cooldown = 0;
    r.tick_hall_squires();
    assert!(r
        .projectiles
        .iter()
        .any(|p| p.kind == Shot::Blade && !p.hostile));
    assert!(r.squire_hall.companions[0].body.x > 10.0);
}
#[test]
fn healer_picks_health_fraction_cleanses_and_assists_a_skull() {
    let mut r = fight(Kind::Healer, 3);
    r.players.insert(2, r.players[&1].clone());
    {
        let h = r.players.get_mut(&1).unwrap();
        h.hp = 40;
        h.max_hp = 100;
        h.hexed = HZ;
    }
    {
        let h = r.players.get_mut(&2).unwrap();
        h.x = 12.0;
        h.y = 12.0;
        h.hp = 50;
        h.max_hp = 200;
        h.chilled = HZ;
    }
    r.tick_hall_squires();
    assert_eq!(r.players[&1].hp, 40);
    assert!(r.players[&2].hp > 50);
    assert_eq!(r.players[&2].chilled, 0);
    {
        let h = r.players.get_mut(&2).unwrap();
        h.hp = 0;
        h.skull_at = Some((12.0, 12.0));
    }
    let c = &mut r.squire_hall.companions[0];
    c.aid = 0;
    c.body.x = 12.0;
    c.body.y = 12.0;
    r.tick_hall_squires();
    assert!(r.players[&2].hp > 0);
    assert!(r.players[&2].skull_at.is_none());
}
#[test]
fn ranger_keeps_range_fires_arrows_and_marks() {
    let mut r = fight(Kind::Ranger, 2);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (15.0, 12.0), 100);
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 12.0;
    c.body.y = 12.0;
    r.tick_hall_squires();
    assert!(r.squire_hall.companions[0].body.x < 12.0);
    assert!(r.squire_hall.companions[0].mark.is_some());
    assert!(r
        .projectiles
        .iter()
        .any(|p| p.kind == Shot::Arrow && p.damage > r.squire_hall.companions[0].squire.power()));
}
#[test]
fn shield_catches_and_alchemist_splashes_and_mends() {
    let mut r = fight(Kind::Shield, 2);
    let c = &mut r.squire_hall.companions[0];
    c.body.invulnerable = 0;
    r.projectiles.push(Projectile {
        x: c.body.x,
        y: c.body.y,
        vx: 1.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Arrow,
        damage: 20,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    });
    let hp = c.body.hp;
    fixtures::foe_at(&mut r, EnemyKind::Slime, (25.0, 20.0), 100);
    r.step(&BTreeMap::new());
    assert!(r.projectiles.is_empty());
    assert_eq!(r.squire_hall.companions[0].body.hp, hp - 10);
    let mut r = fight(Kind::Alchemist, 2);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (16.0, 12.0), 100);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (17.0, 12.0), 100);
    let hp: Vec<_> = r.enemies.iter().map(|e| e.hp).collect();
    r.players.get_mut(&1).unwrap().hp = 1;
    r.tick_hall_squires();
    assert!(r.enemies.iter().zip(hp).all(|(e, hp)| e.hp < hp));
    assert!(r.players[&1].hp > 1);
}
#[test]
fn host_settles_treasury_requests_once_and_name_punctuation_is_safe() {
    let mut r = Run::at_home(3, 1, None, home::Home::default(), Spoils::default());
    r.home.squire_hall.refresh(0);
    let id = r.home.squire_hall.offers[0].id;
    r.squire_order(1, Command::Hire("Kay: the \"kind\"".into(), id));
    let mut home = r.home.clone();
    let mut gold = Spoils::default();
    gold.add(crate::realm::Spoil::Gold, 1000);
    let kept = BTreeMap::new();
    let mut realm = activity::Realm {
        home: &mut home,
        treasury: &mut gold,
        reclaimed: &kept,
        raids_won: 0,
    };
    let out = settle(&mut realm, &r.marks);
    assert!(out.changed);
    let left = realm.treasury.clone();
    assert!(!settle(&mut realm, &r.marks).changed);
    assert_eq!(*realm.treasury, left);
    assert_eq!(
        realm.home.squire_hall.rosters["Kay: the \"kind\""].active,
        Some(id)
    );
}

#[test]
fn hold_e_hire_select_train_build_then_descend_and_old_wire_defaults() {
    let mut gold = Spoils::default();
    gold.add(crate::realm::Spoil::Gold, 10000);
    let mut r = Run::at_home(7, 1, None, home::Home::default(), gold);
    let yard = r
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::SquireHall)
        .unwrap();
    r.enter_for_test(yard);
    fn use_plate(r: &mut Run, p: usize) {
        let (x, y, _, _) = PLATES[p];
        let h = r.players.get_mut(&1).unwrap();
        h.x = (x as f32 + 0.5) * TILE_UNITS;
        h.y = (y as f32 + 0.5) * TILE_UNITS;
        h.buy_spent = false;
        h.buying = 0;
        let inputs = BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]);
        for _ in 0..home::BUY_HOLD {
            r.tick_squire_hall_room(&inputs);
            r.tick += 1;
        }
        let mut home = r.home.clone();
        let mut gold = r.treasury.clone();
        let kept = BTreeMap::new();
        let marks = std::mem::take(&mut r.marks);
        settle(
            &mut activity::Realm {
                home: &mut home,
                treasury: &mut gold,
                reclaimed: &kept,
                raids_won: 0,
            },
            &marks,
        );
        r.rebuild_home(home, gold);
    }
    use_plate(&mut r, 1);
    let who = home::Home::knight_key(&r.players[&1]);
    assert_eq!(r.home.squire_hall.rosters[&who].squires.len(), 1);
    use_plate(&mut r, 6);
    assert_eq!(r.home.squire_hall.rosters[&who].squires[0].level, 2);
    for p in [2, 5, 8] {
        use_plate(&mut r, p);
    }
    assert_eq!(
        (
            r.home.squire_hall.recruit,
            r.home.squire_hall.hire,
            r.home.squire_hall.train
        ),
        (1, 1, 1)
    );
    use_plate(&mut r, 4);
    assert_eq!(r.home.squire_hall.rosters[&who].active, None);
    use_plate(&mut r, 3);
    assert!(r.home.squire_hall.rosters[&who].active.is_some());
    let before = r.home.squire_hall.cadence;
    r.descend_for_test();
    assert_eq!(r.squire_hall.companions.len(), 1);
    assert_eq!(r.home.squire_hall.cadence, before + 1);
    let mut tree = serde_json::to_value(&r).unwrap();
    tree.as_object_mut().unwrap().remove("squire_hall");
    tree["home"].as_object_mut().unwrap().remove("squire_hall");
    let old: Run = serde_json::from_value(tree).unwrap();
    assert_eq!(old.squire_hall, Party::default());
    assert_eq!(old.home.squire_hall, Default::default());
    let tree = crate::wire::publish(&r).unwrap();
    let guest = crate::wire::read(&tree).unwrap();
    assert_eq!(guest.squire_hall, r.squire_hall);
}
#[test]
fn six_owners_have_one_each_and_downed_or_vanished_are_not_decoys() {
    let mut r = fight(Kind::Shield, 3);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
        let who = home::Home::knight_key(&r.players[&id]);
        r.home.squire_hall.rosters.insert(
            who,
            crate::realm::squires::Roster {
                active: Some(id),
                squires: vec![Squire {
                    id,
                    name: format!("Pip {id}"),
                    kind: Kind::ALL[(id as usize - 1) % 5],
                    level: 3,
                    talent: 0,
                }],
            },
        );
    }
    r.hall_squires_follow();
    r.hall_squires_follow();
    assert_eq!(r.squire_hall.companions.len(), 6);
    assert!(r.squire_hall.valid());
    assert_eq!(r.squire_hall.decoys(&r).count(), 6);
    r.squire_hall.companions[0].body.hp = 0;
    r.squire_hall.companions[1].smoke = HZ;
    assert_eq!(r.squire_hall.decoys(&r).count(), 4);
}
#[test]
fn untrained_types_do_not_get_later_abilities() {
    let mut r = fight(Kind::Ninja, 1);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (16.0, 12.0), 100);
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 18.0;
    c.body.y = 12.0;
    let hp = r.enemies[0].hp;
    r.tick_hall_squires();
    assert_eq!(r.enemies[0].hp, hp);
    assert_eq!(r.squire_hall.companions[0].smoke, 0);
    let mut r = fight(Kind::Healer, 1);
    r.players.get_mut(&1).unwrap().hexed = HZ;
    r.players.get_mut(&1).unwrap().hp = 1;
    r.tick_hall_squires();
    assert!(r.players[&1].hp > 1);
    assert_eq!(r.players[&1].hexed, HZ);
    let mut r = fight(Kind::Ranger, 1);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (16.0, 12.0), 100);
    r.tick_hall_squires();
    assert_eq!(r.squire_hall.companions[0].mark, None);
}
#[test]
fn shield_guard_is_a_brief_cadenced_aid_not_permanent_invulnerability() {
    let mut r = fight(Kind::Shield, 2);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (18.0, 12.0), 100);
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 13.0;
    c.body.y = 12.0;
    r.players.get_mut(&1).unwrap().invulnerable = 0;
    r.tick_hall_squires();
    assert!(r.players[&1].invulnerable > 0);
    assert!(r.squire_hall.companions[0].aid > HZ);
    r.players.get_mut(&1).unwrap().invulnerable = 0;
    r.tick_hall_squires();
    assert_eq!(r.players[&1].invulnerable, 0);
}
#[test]
fn healer_and_bombs_do_not_reach_through_solid_walls() {
    let mut r = fight(Kind::Healer, 3);
    r.squire_hall.companions[0].body.x = 8.0;
    r.squire_hall.companions[0].body.y = 12.0;
    r.players.get_mut(&1).unwrap().hp = 1;
    for row in 1..r.room().rows - 1 {
        r.dungeon.rooms[r.at].set(5, row, Tile::Wall);
    }
    r.tick_hall_squires();
    assert_eq!(r.players[&1].hp, 1);
    let mut r = fight(Kind::Alchemist, 2);
    r.squire_hall.companions[0].body.x = 12.0;
    r.squire_hall.companions[0].body.y = 12.0;
    fixtures::foe_at(&mut r, EnemyKind::Slime, (16.0, 12.0), 100);
    for row in 1..r.room().rows - 1 {
        r.dungeon.rooms[r.at].set(7, row, Tile::Wall);
    }
    let hp = r.enemies[0].hp;
    r.tick_hall_squires();
    assert_eq!(r.enemies[0].hp, hp);
}

#[test]
fn browser_incremental_copy_receives_squire_changes_without_unknown_fallback() {
    use std::borrow::Cow;
    let mut r = fight(Kind::Ranger, 3);
    let mut guest = crate::wire::Guest::default();
    let first = crate::wire::publish(&r).unwrap();
    guest.read(&first, None, &crate::wire::Shots::Same).unwrap();
    r.squire_hall.companions[0].body.hp = 17;
    let next = crate::wire::publish(&r).unwrap();
    let seen = guest
        .read(
            &next,
            Some(&[Cow::Borrowed("squire_hall")]),
            &crate::wire::Shots::Same,
        )
        .unwrap();
    assert_eq!(seen.squire_hall, r.squire_hall);
    assert_eq!(guest.stats().2, "");
}

// Review regressions deliberately exercise Run::step: helper-only ticks miss
// flight/aftermath ordering, cooldowns, and the yard's shared hold counter.
fn review_combat(kind: Kind, level: u8) -> Run {
    let mut r = fight(kind, level);
    fixtures::foe_at(&mut r, EnemyKind::Slime, (28.0, 20.0), 10_000);
    let c = &mut r.squire_hall.companions[0];
    c.body.invulnerable = 0;
    c.body.cooldown = 10_000;
    c.aid = 10_000;
    r
}
fn review_shot(at: (f32, f32), vx: f32) -> Projectile {
    Projectile {
        x: at.0,
        y: at.1,
        vx,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Arrow,
        damage: 20,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 5 * HZ,
    }
}
#[test]
fn review_departure_removes_companion_in_multiplayer_full_step() {
    let mut r = review_combat(Kind::Shield, 1);
    r.join_seat(2, "Still here");
    r.leave(1);
    r.step(&BTreeMap::new());
    assert!(r.players.contains_key(&2));
    assert!(
        r.squire_hall.companions.is_empty(),
        "departed owner's decoy survived"
    );
}
#[test]
fn review_swept_shot_hits_first_body_not_final_point() {
    for (kind, companion_x, knight_hit) in [
        (Kind::Shield, 12.0, false),
        (Kind::Shield, 20.0, true),
        (Kind::Ranger, 12.0, true),
    ] {
        let mut r = review_combat(kind, 1);
        let h = r.players.get_mut(&1).unwrap();
        h.x = 16.0;
        h.y = 12.0;
        h.invulnerable = 0;
        let hp = h.hp;
        let c = &mut r.squire_hall.companions[0];
        c.body.x = companion_x;
        c.body.y = 12.0;
        let chp = c.body.hp;
        r.projectiles
            .push(review_shot((8.0, 12.0), 16.0 * HZ as f32));
        r.step(&BTreeMap::new());
        assert_eq!(
            r.players[&1].hp < hp,
            knight_hit,
            "{kind:?} at {companion_x}"
        );
        assert_eq!(r.squire_hall.companions[0].body.hp < chp, companion_x < 16.0);
    }
}
#[test]
fn review_beam_warns_then_strikes_each_companion_once() {
    for kind in Kind::ALL {
        let mut r = review_combat(kind, 1);
        r.players.get_mut(&1).unwrap().y = 20.0;
        let c = &mut r.squire_hall.companions[0];
        c.body.x = 12.0;
        c.body.y = 12.0;
        let hp = c.body.hp;
        let mut p = review_shot((4.0, 12.0), 0.0);
        p.traits.path.beam = Some(patterns::Beam {
            angle: 0.0,
            reach: 30.0,
            warn: 3,
            burn: 120,
            ..Default::default()
        });
        r.projectiles.push(p);
        for _ in 0..3 {
            r.step(&BTreeMap::new());
        }
        assert_eq!(r.squire_hall.companions[0].body.hp, hp, "warning hurt {kind:?}");
        // Hold the companion on the line, including beyond its hit ward.
        for _ in 0..55 {
            let c = &mut r.squire_hall.companions[0];
            c.body.x = 12.0;
            c.body.y = 12.0;
            r.step(&BTreeMap::new());
        }
        let damage = if kind == Kind::Shield { 10 } else { 20 };
        assert_eq!(r.squire_hall.companions[0].body.hp, hp - damage, "{kind:?}");
    }
}
fn review_fallen(r: &mut Run, id: u32, at: (f32, f32)) {
    r.join_seat(id, &format!("Fallen {id}"));
    let h = r.players.get_mut(&id).unwrap();
    h.hp = 0;
    h.x = at.0;
    h.y = at.1;
    h.skull_at = Some(at);
}
#[test]
fn review_healer_pursues_skull_through_ordinary_healing_cooldowns() {
    let mut r = review_combat(Kind::Healer, 3);
    let h = r.players.get_mut(&1).unwrap();
    h.x = 8.0;
    h.hp = 1;
    r.enemies[0].x = 16.0;
    r.enemies[0].y = 12.0;
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 10.0;
    c.body.y = 12.0;
    c.aid = 0;
    review_fallen(&mut r, 2, (20.0, 12.0));
    for _ in 0..600 {
        r.players.get_mut(&1).unwrap().hp = 1;
        r.step(&BTreeMap::new());
        if r.players[&2].hp > 0 {
            break;
        }
    }
    assert!(r.players[&2].hp > 0, "healer never reached the skull");
}
#[test]
fn review_healer_skips_blocked_lower_id_skull() {
    let mut r = review_combat(Kind::Healer, 3);
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 10.0;
    c.body.y = 12.0;
    c.aid = 0;
    review_fallen(&mut r, 2, (20.0, 12.0));
    review_fallen(&mut r, 3, (10.0, 14.0));
    for row in 1..r.room().rows - 1 {
        r.dungeon.rooms[r.at].set(8, row, Tile::Wall);
    }
    for _ in 0..90 {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.players[&2].hp, 0);
    assert!(
        r.players[&3].hp > 0,
        "blocked skull starved reachable knight"
    );
}
#[test]
fn review_buried_foes_neither_touch_nor_take_squire_attacks() {
    for kind in [Kind::Ninja, Kind::Shield, Kind::Alchemist] {
        let mut r = review_combat(kind, 3);
        r.enemies.clear();
        fixtures::foe_at(&mut r, EnemyKind::Mole, (14.0, 12.0), 10_000);
        r.enemies[0].stage = 1;
        let ehp = r.enemies[0].hp;
        let c = &mut r.squire_hall.companions[0];
        c.body.x = 14.0;
        c.body.y = 12.0;
        c.body.cooldown = 0;
        let hp = c.body.hp;
        r.step(&BTreeMap::new());
        assert_eq!(
            r.squire_hall.companions[0].body.hp, hp,
            "buried touch: {kind:?}"
        );
        assert_eq!(r.enemies[0].hp, ehp, "buried target: {kind:?}");
    }
}
#[test]
fn review_squire_bomb_shares_crowd_damage() {
    let mut r = review_combat(Kind::Alchemist, 1);
    r.enemies.clear();
    for i in 0..7 {
        fixtures::foe_at(
            &mut r,
            EnemyKind::Slime,
            (16.0 + i as f32 * 0.1, 12.0),
            10_000,
        );
    }
    for e in &mut r.enemies {
        e.hp = 1000;
    }
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 10.0;
    c.body.y = 12.0;
    c.body.cooldown = 0;
    let damage = c.squire.power();
    r.step(&BTreeMap::new());
    assert_eq!(1000 - r.enemies[0].hp, damage);
    assert_eq!(1000 - r.enemies[6].hp, aoe::part(damage, 6));
}
#[test]
fn review_squire_arrows_do_not_consume_knight_shot_allowance() {
    let mut r = review_combat(Kind::Ranger, 1);
    r.squire_hall.companions[0].body.cooldown = 0;
    r.enemies[0].x = 18.0;
    r.enemies[0].y = 12.0;
    r.step(&BTreeMap::new());
    let p = r
        .projectiles
        .iter()
        .find(|p| !p.hostile && p.kind == Shot::Arrow)
        .unwrap()
        .clone();
    assert_eq!(p.traits.owner, 1, "keep damage attribution");
    assert!(
        !p.loosed_by(1),
        "companion arrows must not enter knight prediction"
    );
    r.projectiles = vec![p; KNIGHT_SHOTS];
    r.step(&BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            ..Default::default()
        },
    )]));
    assert!(
        r.projectiles.iter().any(|p| p.loosed_by(1)),
        "squire arrows starved owner's arm"
    );
}
fn review_yard() -> Run {
    let mut gold = Spoils::default();
    gold.add(crate::realm::Spoil::Gold, 10000);
    let mut r = Run::at_home(7, 1, None, home::Home::default(), gold);
    let yard = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::SquireHall)
        .unwrap();
    r.enter_for_test(yard);
    r.home.squire_hall.refresh(0);
    r
}
fn review_plate(r: &mut Run, p: usize) {
    let (x, y, _, _) = PLATES[p];
    let h = r.players.get_mut(&1).unwrap();
    h.x = (x as f32 + 0.5) * 2.0;
    h.y = (y as f32 + 0.5) * 2.0;
}
fn review_hold(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
}
#[test]
fn yard_hall_and_countryside_plate_holds_complete_independently() {
    let inputs = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    let mut hall = review_yard();
    review_plate(&mut hall, 1);
    for tick in 1..=home::BUY_HOLD {
        hall.tick_squire_hall_room(&inputs);
        hall.tick += 1;
        assert_eq!(
            hall.marks.keys().filter(|k| k.starts_with("squire:")).count(),
            usize::from(tick == home::BUY_HOLD)
        );
        assert!(!hall.marks.keys().any(|k| k.starts_with("country:squire:")));
        if tick < home::BUY_HOLD {
            assert_eq!(hall.players[&1].buying, tick);
        }
    }
    assert!(hall.players[&1].buy_spent);
    for _ in 0..home::BUY_HOLD {
        hall.tick_squire_hall_room(&inputs);
    }
    assert_eq!(hall.marks.keys().filter(|k| k.starts_with("squire:")).count(), 1);

    // The countryside squires keep the Yard itself (`squire_rooms`).
    let mut country = review_yard();
    let yard = country.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Yard).unwrap();
    country.enter_for_test(yard);
    let (x, y) = super::super::countryside_squires::PLATES[0];
    let hero = country.players.get_mut(&1).unwrap();
    hero.x = x * TILE_UNITS;
    hero.y = y * TILE_UNITS;
    for tick in 1..=home::BUY_HOLD {
        country.tick_yard(&inputs);
        country.tick += 1;
        assert_eq!(
            country.marks.keys().filter(|k| k.starts_with("country:squire:")).count(),
            usize::from(tick == home::BUY_HOLD)
        );
        assert!(!country.marks.keys().any(|k| k.starts_with("squire:")));
    }
    for _ in 0..home::BUY_HOLD {
        country.tick_yard(&inputs);
    }
    assert_eq!(country.marks.keys().filter(|k| k.starts_with("country:squire:")).count(), 1);
}
#[test]
fn review_yard_partial_hold_cancels_on_release_exit_or_action_change() {
    for cancellation in 0..4 {
        let mut r = review_yard();
        review_plate(&mut r, 0);
        review_hold(&mut r, home::BUY_HOLD - 1);
        assert_eq!(r.players[&1].buying, home::BUY_HOLD - 1);
        assert!(r.marks.keys().all(|k| !k.starts_with("squire:")));
        match cancellation {
            0 => {
                r.step(&BTreeMap::new());
            } // release on the plate
            1 | 2 => {
                let h = r.players.get_mut(&1).unwrap();
                h.x = 20.0;
                h.y = 18.0; // no plate, lectern or stall
                if cancellation == 1 {
                    r.step(&BTreeMap::new());
                } else {
                    review_hold(&mut r, 1);
                }
            }
            _ => {} // move directly to another action without releasing
        }
        review_plate(&mut r, 1);
        review_hold(&mut r, 1);
        assert_eq!(r.players[&1].buying, 1, "cancellation {cancellation}");
        assert!(r.marks.keys().all(|k| !k.starts_with("squire:")));
        review_hold(&mut r, home::BUY_HOLD - 1);
        assert_eq!(r.players[&1].buying, 0);
        assert!(r.players[&1].buy_spent);
        let mut home = r.home.clone();
        let mut gold = r.treasury.clone();
        let kept = BTreeMap::new();
        assert!(
            settle(
                &mut activity::Realm {
                    home: &mut home,
                    treasury: &mut gold,
                    reclaimed: &kept,
                    raids_won: 0,
                },
                &r.marks
            )
            .changed
        );
        let key = home::Home::knight_key(&r.players[&1]);
        assert_eq!(home.squire_hall.rosters[&key].squires.len(), 1);
        let marks = r.marks.clone();
        review_hold(&mut r, home::BUY_HOLD * 2);
        assert_eq!(r.marks, marks, "one action per press");
    }
}

#[test]
fn review_inactive_owners_are_not_enemy_sights_or_projectile_guards() {
    for state in 0..3 {
        let mut r = review_combat(Kind::Shield, 1);
        r.join_seat(2, "Still here");
        let h = r.players.get_mut(&2).unwrap();
        h.x = 24.0;
        h.y = 12.0;
        h.invulnerable = 0;
        let hp = h.hp;
        r.players.get_mut(&1).unwrap().y = 20.0;
        let c = &mut r.squire_hall.companions[0];
        c.body.x = 16.0;
        c.body.y = 12.0;
        r.enemies[0].x = 8.0;
        r.enemies[0].y = 12.0;
        match state {
            0 => r.set_absent(1, true),
            1 => {
                r.exits.outside.insert(
                    1,
                    exits::Entry {
                        at: r.at,
                        party: BTreeMap::new(),
                    },
                );
            }
            _ => {
                r.players.remove(&1);
            } // stale/older checkpoint
        }
        assert_eq!(r.squire_hall.decoys(&r).count(), 0);
        let without_companions = {
            let mut plain: Run = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
            plain.squire_hall.companions.clear();
            press::warning_target(&plain, &plain.enemies[0])
        };
        assert_eq!(press::warning_target(&r, &r.enemies[0]), without_companions);
        r.projectiles
            .push(review_shot((10.0, 12.0), 18.0 * HZ as f32));
        r.step(&BTreeMap::new());
        assert!(
            r.players[&2].hp < hp,
            "inactive companion blocked for state {state}"
        );
        assert_eq!(r.squire_hall.decoys(&r).count(), 0);
    }
}
#[test]
fn review_submerged_contact_and_bomb_splash_use_shared_eligibility() {
    for buried_kind in [EnemyKind::Mole, EnemyKind::BogHand] {
        for kind in [Kind::Ninja, Kind::Shield, Kind::Alchemist] {
            let mut r = review_combat(kind, 3);
            r.enemies.clear();
            fixtures::foe_at(&mut r, buried_kind, (14.0, 12.0), 0);
            let e = &mut r.enemies[0];
            e.stage = 1;
            e.age = TELEGRAPH + 1;
            e.timer = 0;
            let ehp = e.hp;
            let c = &mut r.squire_hall.companions[0];
            c.body.x = 14.0;
            c.body.y = 12.0;
            c.body.cooldown = 0;
            let hp = c.body.hp;
            r.step(&BTreeMap::new());
            assert!(r.enemies[0].buried());
            assert_eq!(
                r.squire_hall.companions[0].body.hp, hp,
                "touch {buried_kind:?}/{kind:?}"
            );
            assert_eq!(r.enemies[0].hp, ehp, "attack {buried_kind:?}/{kind:?}");
        }
        let mut r = review_combat(Kind::Alchemist, 3);
        r.enemies.clear();
        fixtures::foe_at(&mut r, EnemyKind::Slime, (16.0, 12.0), 10_000);
        fixtures::foe_at(&mut r, buried_kind, (17.0, 12.0), 10_000);
        r.enemies[1].stage = 1;
        let hp: Vec<_> = r.enemies.iter().map(|e| e.hp).collect();
        r.squire_hall.companions[0].body.cooldown = 0;
        r.step(&BTreeMap::new());
        assert!(r.enemies[0].hp < hp[0]);
        assert_eq!(r.enemies[1].hp, hp[1], "splash {buried_kind:?}");
    }
}
#[test]
fn review_shield_sweeps_intercept_without_a_knight_at_the_end() {
    let mut r = review_combat(Kind::Shield, 1);
    r.players.get_mut(&1).unwrap().y = 20.0;
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 12.0;
    c.body.y = 12.0;
    let hp = c.body.hp;
    r.projectiles
        .push(review_shot((8.0, 12.0), 16.0 * HZ as f32));
    r.step(&BTreeMap::new());
    assert_eq!(r.squire_hall.companions[0].body.hp, hp - 10);
    assert!(r.projectiles.is_empty());
}
#[test]
fn review_healer_chooses_nearest_eligible_skull_then_id_on_tie() {
    for (a, b, expected) in [
        ((12.5, 12.0), (10.5, 12.0), 3),
        ((10.0, 13.0), (10.0, 11.0), 2),
    ] {
        let mut r = review_combat(Kind::Healer, 3);
        let c = &mut r.squire_hall.companions[0];
        c.body.x = 10.0;
        c.body.y = 12.0;
        c.aid = 0;
        review_fallen(&mut r, 2, a);
        review_fallen(&mut r, 3, b);
        r.step(&BTreeMap::new());
        assert!(r.players[&expected].hp > 0);
        assert_eq!(r.players[&(5 - expected)].hp, 0);
    }
}

#[test]
fn review_shield_and_skeleton_wall_share_earliest_hit_order() {
    for (shield_x, skeleton_x) in [(12.0, 16.0), (16.0, 12.0)] {
        let mut r = review_combat(Kind::Shield, 1);
        assert!(r.take_up(1, minions::GRAVECALLER));
        r.minions.graves.entry(1).or_default().wall = 1000;
        r.players.get_mut(&1).unwrap().x = 24.0;
        let c = &mut r.squire_hall.companions[0];
        c.body.x = shield_x;
        c.body.y = 12.0;
        let hp = c.body.hp;
        r.minions.raised.push(hireling::Hireling::minion(
            hireling::AllyKind::Skeleton,
            0,
            r.dungeon.depth,
            (skeleton_x, 12.0),
            1,
        ));
        let skeleton_hp = r.minions.raised[0].hp;
        r.projectiles
            .push(review_shot((8.0, 12.0), 20.0 * HZ as f32));
        r.step(&BTreeMap::new());
        assert_eq!(r.squire_hall.companions[0].body.hp < hp, shield_x < skeleton_x);
        assert_eq!(r.minions.raised[0].hp < skeleton_hp, skeleton_x < shield_x);
        assert!(r.projectiles.is_empty());
    }
}

#[test]
fn review_beam_strikes_track_companions_separately_and_survive_checkpoints() {
    let mut r = review_combat(Kind::Ranger, 1);
    r.join_seat(2, "Second owner");
    for h in r.players.values_mut() {
        h.y = 20.0;
    }
    let c = &mut r.squire_hall.companions[0];
    c.body.x = 12.0;
    c.body.y = 12.0;
    let mut other = c.clone();
    other.owner = 2;
    other.body.x = 16.0;
    r.squire_hall.companions.push(other);
    let hp = r.squire_hall.companions[0].body.hp;
    let mut beam = review_shot((4.0, 12.0), 0.0);
    beam.traits.path.beam = Some(patterns::Beam {
        angle: 0.0,
        reach: 30.0,
        warn: 0,
        burn: 120,
        ..Default::default()
    });
    r.projectiles.push(beam);
    r.step(&BTreeMap::new());
    for c in &r.squire_hall.companions {
        assert_eq!(c.body.hp, hp - 20);
    }
    assert_eq!(r.projectiles[0].traits.squire_struck, vec![1, 2]);
    r = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    let wire = crate::wire::read(&crate::wire::publish(&r).unwrap()).unwrap();
    assert_eq!(wire.projectiles[0].traits.squire_struck, vec![1, 2]);
    for c in &mut r.squire_hall.companions {
        c.body.invulnerable = 0;
        c.body.x = 12.0;
        c.body.y = 12.0;
    }
    r.step(&BTreeMap::new());
    for c in &r.squire_hall.companions {
        assert_eq!(c.body.hp, hp - 20);
    }
}
