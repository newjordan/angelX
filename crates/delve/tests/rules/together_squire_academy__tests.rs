use super::*;
fn yard() -> Run {
    let mut treasury = Spoils::default(); treasury.add(Spoil::Gold, 20_000);
    let mut run = Run::at_home(740, 1, Some("Bess"), home::Home::default(), treasury);
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::SquireAcademy).unwrap();
    run.enter_for_test(room); run.calm_for_test(); run
}
#[test]
fn squires_rosters_caps_and_independent_buildings() {
    let mut r = yard(); let before = r.treasury.get(Spoil::Gold);
    for i in 0..3 { r.academy_squire_action(1, Action::Hire(i)).unwrap(); }
    assert!(r.academy_squire_action(1, Action::Hire(3)).is_err());
    r.academy_squire_action(2, Action::Hire(3)).unwrap();
    r.academy_squire_action(1, Action::Upgrade(0)).unwrap();
    assert_eq!((r.home.squire_academy.board, r.home.squire_academy.hall, r.home.squire_academy.yard), (1,0,0));
    r.academy_squire_action(1, Action::Hire(4)).unwrap();
    assert!(r.treasury.get(Spoil::Gold) < before);
    r.embark_squires();
    assert_eq!(r.squire_academy.active.len(), 2);
    assert_eq!(r.squire_academy.active[&1].body.kind, AllyKind::SquireAlchemist);
    assert_eq!(r.squire_academy.active[&2].body.kind, AllyKind::SquireShieldBearer);
}
#[test]
fn squires_training_cost_time_and_delve_gate() {
    let mut r = yard(); r.academy_squire_action(1, Action::Hire(0)).unwrap();
    assert!(r.academy_squire_action(1, Action::Train).is_err());
    r.embark_squires();
    let before = r.treasury.get(Spoil::Gold);
    r.academy_squire_action(1, Action::Train).unwrap();
    assert_eq!(r.treasury.get(Spoil::Gold), before - 80);
    assert!(r.academy_squire_action(1, Action::Train).is_err());
    r.embark_squires(); assert!(r.squire_academy.active.is_empty());
    for _ in 0..TRAIN_SECONDS * HZ { r.tick_academy_squires(); }
    let key = r.academy_squire_key(1).unwrap();
    assert_eq!(r.home.squire_academy.rosters[&key].members[0].level, 2);
    assert!(r.academy_squire_action(1, Action::Train).is_err()); // hall level zero caps at two
}
#[test]
fn squires_old_saves_and_receipts_are_idempotent() {
    let old: home::Home = serde_json::from_str("{}").unwrap();
    assert_eq!(old.squire_academy, Academy::default());
    let mut r = yard(); r.academy_squire_action(1, Action::Hire(0)).unwrap();
    let mut saved = Academy::default(); let mut money = gold(20_000);
    assert!(settle(&mut saved, &mut money, &r.home.squire_academy));
    assert_eq!(money.get(Spoil::Gold), r.treasury.get(Spoil::Gold));
    assert!(!settle(&mut saved, &mut money, &r.home.squire_academy));
    assert_eq!(serde_json::from_str::<Academy>(&serde_json::to_string(&saved).unwrap()).unwrap(), saved);
}
#[test]
fn squires_recruit_board_is_deterministic_and_refreshes_every_three_delves() {
    let mut a = Academy::default(); let first = a.recruits();
    a.departures = 2; assert_eq!(first, a.recruits());
    a.departures = 3; assert_ne!(first, a.recruits());
    assert_eq!(a.recruits().len(), 5);
    for (i, r) in a.recruits().iter().enumerate() { assert_eq!(r.kind, Kind::ALL[i]); }
}

fn fight(kind: Kind, level: u8) -> (Run, Companion) {
    let mut r = Run::new(72, 1, Some("Bess"));
    let at = r.dungeon.rooms.iter().position(|room| room.kind == RoomKind::Fight).unwrap();
    r.enter_for_test(at); r.calm_for_test(); r.open_room_for_test(at);
    r.enemies.clear(); r.projectiles.clear();
    for h in r.players.values_mut() { h.x = 8.; h.y = 10.; h.invulnerable = 0; }
    let stats = kind.stats(level, 0);
    let mut body = Hireling::new(1, (10., 10.));
    body.kind = kind.ally(); body.power = u32::from(level); body.summoner = Some(1);
    body.hp = stats.hp; body.max_hp = stats.hp; body.cooldown = 0;
    (r, Companion { body, name: "Test".into(), level, special: 0, reviving: None })
}
fn foe(r: &mut Run, x: f32, y: f32) -> usize {
    r.spawn_at_for_test(EnemyKind::Bat, x, y);
    let i = r.enemies.len() - 1; let e = &mut r.enemies[i]; e.age = TELEGRAPH; e.hp = 500; e.max_hp = 500; i
}
#[test]
fn squires_heal_lowest_fraction_at_range_and_cleanse() {
    let (mut r, mut c) = fight(Kind::Healer, 2);
    let a = r.players.get_mut(&1).unwrap(); a.hp = 40; a.max_hp = 50;
    let b = r.players.get_mut(&2).unwrap(); b.hp = 60; b.max_hp = 200; b.hexed = HZ; b.chilled = HZ; b.mired = HZ;
    r.squire_attack(&mut c.body);
    assert_eq!(r.players[&1].hp, 40);
    assert!(r.players[&2].hp > 60);
    assert_eq!((r.players[&2].hexed, r.players[&2].chilled, r.players[&2].mired), (0,0,0));
    c.body.cooldown = 0;
    r.players.get_mut(&2).unwrap().x = 40.;
    r.squire_attack(&mut c.body); assert!(r.players[&1].hp > 40);
}
#[test]
fn squires_ninja_flanks_backstabs_throws_stars_and_smokes() {
    let (mut r, mut c) = fight(Kind::Ninja, 3);
    let i = foe(&mut r, 9., 10.);
    let damage = Kind::Ninja.stats(3, 0).damage;
    r.squire_attack(&mut c.body); assert_eq!(r.enemies[i].hp, 500 - 2 * damage);
    r.enemies[i].x = 18.; c.body.cooldown = 0;
    r.squire_attack(&mut c.body); assert_eq!(r.projectiles.len(), 1); assert_eq!(r.projectiles[0].traits.owner, 1);
    assert!(r.squire_way(&c.body, (0., 0.)).0 > 0.);
    c.body.hp = 10; r.squire_special(&mut c); assert_eq!(c.special, 8 * HZ); assert!(c.body.invulnerable >= HZ);
}
#[test]
fn squires_ranger_range_mark_and_alchemist_area_potion() {
    let (mut r, mut c) = fight(Kind::Ranger, 2);
    let i = foe(&mut r, 29., 10.); // outside kitchen-knight sight, inside arrow range
    r.squire_attack(&mut c.body);
    assert_eq!(r.projectiles.len(), 1); assert!(r.squire_academy.marks.contains_key(&r.enemies[i].id));
    let base = r.projectiles[0].damage; r.squire_attack(&mut c.body);
    assert!(r.projectiles[1].damage > base);
    let (mut r, mut c) = fight(Kind::Alchemist, 2);
    let i = foe(&mut r, 15., 10.); let j = foe(&mut r, 16., 10.);
    r.squire_attack(&mut c.body); assert!(r.enemies[i].hp < 500 && r.enemies[j].hp < 500);
    r.players.get_mut(&1).unwrap().hp = 10;
    r.squire_special(&mut c); assert!(r.players[&1].hp > 10); assert_eq!(c.special, 6 * HZ);
}
#[test]
fn squires_shield_intercepts_and_taunts_but_not_great_foes() {
    let (mut r, mut c) = fight(Kind::ShieldBearer, 2);
    let i = foe(&mut r, 12., 10.);
    r.squire_attack(&mut c.body);
    assert!(c.body.taunt > 0);
    r.squire_academy.active.insert(1, c.clone());
    let mut seen = vec![(1.,1.)]; r.squire_academy.roared(&r.enemies[i], &mut seen);
    assert_eq!(seen, [(10.,10.)]);
    r.enemies.clear();
    r.projectiles.push(Projectile { x: 10., y: 10., vx: -10., vy: 0., hostile: true, kind: Shot::Blade,
        look: None, damage: 20, pierce: 0, last_hit: None, empowered: false, traits: Default::default(), ttl: HZ });
    let hp = c.body.hp; r.drive_hireling(&mut c.body);
    assert_eq!(c.body.hp, hp - 10); assert_eq!(r.projectiles[0].ttl, 0);
}
#[test]
fn squires_revive_carried_skull_after_channel_and_recover_without_loot() {
    let (mut r, mut c) = fight(Kind::Healer, 3);
    r.tick = 100;
    let h = r.players.get_mut(&2).unwrap(); h.hp = 0; h.fell = 1; h.skull_at = None;
    r.players.get_mut(&1).unwrap().carrying_skull = Some(2);
    for _ in 0..3 * HZ - 1 { r.squire_special(&mut c); }
    assert_eq!(r.players[&2].hp, 0);
    r.squire_special(&mut c); assert!(r.players[&2].hp > 0); assert_eq!(r.players[&1].carrying_skull, None);
    assert_eq!((r.players[&2].x, r.players[&2].y), (r.players[&1].x, r.players[&1].y));
    c.body.hp = 0; r.squire_academy.active.insert(1, c);
    let before = r.score; r.academy_squires_follow();
    assert!(r.squire_academy.active[&1].body.hp > 0); assert_eq!(r.score, before);
}
#[test]
fn squires_yard_press_is_one_action_and_avoids_old_station_plates() {
    let mut r = yard();
    let h = r.players.get_mut(&1).unwrap(); (h.x,h.y) = (RECRUIT_AT[0].0 * TILE_UNITS, RECRUIT_AT[0].1 * TILE_UNITS);
    assert!(super::super::talents::lectern_at(h.x, h.y).is_none());
    let inputs = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    let before = r.treasury.get(Spoil::Gold);
    for _ in 0..10 { r.step(&inputs); }
    assert_eq!(r.treasury.get(Spoil::Gold), before - 100);
    assert!(r.orders.is_empty());
    let key = r.academy_squire_key(1).unwrap(); assert_eq!(r.home.squire_academy.rosters[&key].members.len(), 1);
}
#[test]
fn squires_wire_full_and_patch_and_legacy_run_defaults() {
    let (mut r, c) = fight(Kind::Ranger, 3); r.squire_academy.active.insert(1, c);
    let mut pub_ = crate::wire::Publisher::default();
    let tree = pub_.publish(&r).unwrap();
    let full = crate::wire::read(&tree).unwrap(); assert_eq!(full.squire_academy, r.squire_academy);
    let mut guest = crate::wire::Guest::default();
    guest.read(&tree, None, &crate::wire::Shots::Rebuild).unwrap();
    r.squire_academy.active.get_mut(&1).unwrap().body.hp -= 1;
    let next = pub_.publish(&r).unwrap();
    // The incremental guest must recognize the additive top-level field.
    let fields = vec![std::borrow::Cow::Borrowed("squire_academy")];
    let patched = guest.read(&next, Some(&fields), &crate::wire::Shots::Same).unwrap();
    assert_eq!(patched.squire_academy, r.squire_academy);
    let mut old = serde_json::to_value(&r).unwrap(); old.as_object_mut().unwrap().remove("squire_academy");
    let legacy: Run = serde_json::from_value(old).unwrap(); assert!(legacy.squire_academy.active.is_empty());
}

#[test]
fn squires_academy_legacy_saved_fields_migrate_without_losing_progress() {
    let mut r = yard();
    r.academy_squire_action(1, Action::Hire(0)).unwrap();
    r.embark_squires();
    let mut saved = serde_json::to_value(&r).unwrap();
    let root = saved.as_object_mut().unwrap();
    let field = root.remove("squire_academy").unwrap();
    root.insert("squires".into(), field);
    let home = root.get_mut("home").unwrap().as_object_mut().unwrap();
    let academy = home.remove("squire_academy").unwrap();
    home.insert("squires".into(), academy);
    let restored: Run = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.squire_academy, r.squire_academy);
    assert_eq!(restored.home.squire_academy, r.home.squire_academy);
    assert_eq!(restored.home.squires, Default::default());
}

#[test]
fn squires_academy_plates_do_not_trigger_existing_yard_plates() {
    for (x, y) in RECRUIT_AT.into_iter().chain(MANAGE_AT).chain(BUILD_AT) {
        let (x, y) = (x * TILE_UNITS, y * TILE_UNITS);
        assert!(super::super::squires::action_at(x, y).is_none());
        assert!(super::super::squire_hall::plate_at(x, y).is_none());
        assert!(super::super::countryside_squires::plate(x, y).is_none());
        assert!(super::super::yard::stall_plate(x, y).is_none());
        assert!(super::super::talents::lectern_at(x, y).is_none());
    }
}
