use super::*;

#[test]
fn squires_hire_train_select_and_spend_atomically() {
    let mut yard = Yard::default();
    let mut purse = Spoils::default();
    assert!(yard.apply(&mut purse, "ash", Action::Hire(0)).is_err());
    assert!(yard.companies.is_empty());
    purse.add(Spoil::Gold, 1000);
    yard.apply(&mut purse, "ash", Action::Hire(0)).unwrap();
    assert_eq!(purse.get(Spoil::Gold), 900);
    yard.apply(&mut purse, "ash", Action::Hire(1)).unwrap();
    assert_eq!(yard.companies["ash"].active, Some(1));
    yard.apply(&mut purse, "ash", Action::Select(1)).unwrap();
    assert_eq!(yard.companies["ash"].active, Some(2));
    yard.apply(&mut purse, "ash", Action::Train(1)).unwrap();
    assert_eq!(yard.companies["ash"].roster[1].level, 2);
    let before = purse.get(Spoil::Gold);
    assert!(yard.apply(&mut purse, "ash", Action::Train(1)).is_err());
    assert_eq!(purse.get(Spoil::Gold), before);
    assert!(yard.apply(&mut purse, "ash", Action::Hire(2)).is_err());
    assert!(!yard.companies.contains_key("pip"));
    yard.apply(&mut purse, "ash", Action::Rest).unwrap();
    assert_eq!(yard.companies["ash"].active, None);
}

#[test]
fn squires_town_upgrades_improve_every_step_and_persist() {
    let mut yard = Yard::default();
    let mut purse = Spoils(BTreeMap::from([(Spoil::Gold, 10000)]));
    let wage = yard.wage(&yard.offers()[0]);
    for step in 0..3 { yard.apply(&mut purse, "pip", Action::Upgrade(step)).unwrap(); }
    assert_eq!(yard.offers().len(), 4);
    assert_eq!(yard.capacity(), 4);
    assert_eq!(yard.ceiling(), 3);
    assert!(yard.wage(&yard.offers()[0]) < wage);
    let old = yard.offers();
    yard.season += 1;
    assert_ne!(yard.offers(), old);
    let restored: Yard = serde_json::from_str(&serde_json::to_string(&yard).unwrap()).unwrap();
    assert_eq!(restored, yard);
}

#[test]
fn squires_selected_company_deploys_at_the_stair_without_replacing_a_hire() {
    let mut home = home::Home::default();
    let mut purse = Spoils(BTreeMap::from([(Spoil::Gold, 1000)]));
    home.squires.apply(&mut purse, "you", Action::Hire(0)).unwrap();
    let mut run = Run::at_home(8, 1, None, home, purse);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (24.0, 14.0);
    for _ in 0..=home::DESCEND_HOLD { run.step(&BTreeMap::new()); }
    let saved = serde_json::to_value(&run).unwrap();
    assert_eq!(saved.get("squires").and_then(|s| s.as_array()).map(Vec::len), Some(1));
}

fn in_dungeon(kind: Kind, level: u8) -> Run {
    let mut run = Run::new(37, 1, Some("Friend"));
    run.enemies.clear(); run.projectiles.clear();
    for (&id, h) in &mut run.players {
        (h.x, h.y) = (12.0 + id as f32 * 3.0, 14.0);
        h.hp = h.max_hp; h.idle = false; h.stone = false;
    }
    let squire = Squire { id: 1, kind, name: "Pip".into(), level };
    let mut body = Hireling::minion(kind.ally(), 0, 1, (15.0, 14.0), 1);
    body.hp = squire.health(); body.max_hp = body.hp; body.cooldown = 0;
    run.squires.push(Companion { owner: 1, squire, body, smoke: 0, vigil: 0, rescued: false });
    run
}

#[test]
fn squires_healer_mends_lowest_health_and_cleanses_not_just_nearest() {
    let mut run = in_dungeon(Kind::Healer, 1);
    run.players.get_mut(&1).unwrap().hp = 80;
    let friend = run.players.get_mut(&2).unwrap();
    friend.hp = 10; friend.hexed = HZ; friend.chilled = HZ;
    run.tick_squires();
    assert!(run.players[&2].hp > 10, "healer picks the most hurt knight");
    assert_eq!((run.players[&2].hexed, run.players[&2].chilled), (0, 0));
    assert_eq!(run.players[&1].hp, 80);
}

#[test]
fn squires_yard_hold_requests_settle_once_from_the_real_treasury() {
    let mut treasury = Spoils(BTreeMap::from([(Spoil::Gold, 1000)]));
    let home = home::Home::default();
    let mut run = Run::at_home(17, 1, None, home.clone(), treasury.clone());
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Yard).unwrap();
    run.enter_for_test(room);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (5.0, 7.0);
    let key = talents::knight_key(h);
    let pressing = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    for _ in 0..home::BUY_HOLD - 1 { run.step(&pressing); }
    assert!(run.home.squires.companies.is_empty(), "passing taps do not hire");
    run.step(&pressing);
    assert_eq!(run.home.squires.companies[&key].roster.len(), 1);
    for _ in 0..home::BUY_HOLD * 2 { run.step(&pressing); }
    assert_eq!(run.home.squires.companies[&key].roster.len(), 1, "release to rearm");
    let mut yard = home.squires;
    let paid = settle(&mut yard, &mut treasury, &run.marks);
    assert!(paid.changed);
    assert_eq!(treasury.get(Spoil::Gold), 900);
    assert_eq!(yard.companies, run.home.squires.companies);
    assert!(!settle(&mut yard, &mut treasury, &run.marks).changed);
    assert_eq!(treasury.get(Spoil::Gold), 900);
}

#[test]
fn squires_six_owners_have_one_active_each_and_revive_after_room_not_loot() {
    let mut run = in_dungeon(Kind::Ranger, 3);
    run.squires.clear();
    let mut purse = Spoils(BTreeMap::from([(Spoil::Gold, 10000)]));
    for id in 1..=6 {
        if !run.players.contains_key(&id) { run.join_seat(id, &format!("Knight {id}")); }
        let key = talents::knight_key(&run.players[&id]);
        run.home.squires.apply(&mut purse, &key, Action::Hire(0)).unwrap();
        run.home.squires.apply(&mut purse, &key, Action::Hire(1)).unwrap();
        run.home.squires.apply(&mut purse, &key, Action::Select(1)).unwrap();
    }
    run.deploy_squires();
    assert_eq!(run.squires.len(), 6);
    assert!(run.squires.iter().all(|s| s.squire.kind == Kind::Healer));
    run.squires[0].body.hp = 0;
    let loot = run.bank.clone();
    run.squires_follow();
    assert!(run.squires[0].body.hp > 0);
    assert_eq!(run.bank, loot);
    let encoded = serde_json::to_string(&run).unwrap();
    let decoded: Run = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.squires, run.squires);
    let mut legacy = serde_json::to_value(&run).unwrap();
    legacy.as_object_mut().unwrap().remove("squires");
    legacy.as_object_mut().unwrap().remove("squire_mark");
    let restored: Run = serde_json::from_value(legacy).unwrap();
    assert!(restored.squires.is_empty());
}

fn target(run: &mut Run, at: (f32, f32)) -> usize {
    run.spawn(EnemyKind::Bat);
    let i = run.enemies.len() - 1;
    let e = &mut run.enemies[i];
    (e.x,e.y) = at;
    e.hp = 500; e.max_hp = 500; e.age = TELEGRAPH; e.frozen = HZ;
    run.phase = Phase::Fighting;
    i
}

#[test]
fn squires_ninja_throws_stars_then_backstabs_and_vanishes() {
    let mut run = in_dungeon(Kind::Ninja, 3);
    target(&mut run, (22.0, 14.0));
    run.tick_squires();
    assert!(run.projectiles.iter().any(|p| !p.hostile && p.kind == Shot::Blade && p.traits.owner == 1));
    run.enemies.clear(); run.projectiles.clear();
    let i = target(&mut run, (15.0, 14.0));
    let s = &mut run.squires[0];
    s.body.x = 16.0; s.body.cooldown = 0;
    run.players.get_mut(&1).unwrap().x = 12.0;
    run.tick_squires();
    assert_eq!(run.enemies[i].hp, 500 - 3 * run.squires[0].squire.power());
    assert!(run.squires[0].smoke > 0 && run.squires[0].body.invulnerable > 0);
}

#[test]
fn squires_ranger_arrows_mark_for_the_party_and_alchemist_blasts_a_cluster() {
    let mut run = in_dungeon(Kind::Ranger, 3);
    let i = target(&mut run, (22.0, 14.0));
    run.tick_squires();
    assert!(run.projectiles.iter().any(|p| p.kind == Shot::Arrow && p.traits.owner == 1));
    assert_eq!(run.squire_mark.as_ref().unwrap().foe, run.enemies[i].id);
    let mut run = in_dungeon(Kind::Alchemist, 3);
    run.players.get_mut(&1).unwrap().hp = 20;
    let a = target(&mut run, (22.0, 14.0));
    let b = target(&mut run, (24.5, 14.0));
    run.tick_squires();
    assert!(run.enemies[a].hp < 500 && run.enemies[b].hp < 500);
    assert!(run.players[&1].hp > 20);
    assert!(!run.blasts.is_empty());
}

#[test]
fn squires_shield_catches_shots_and_healer_vigil_restores_a_skull_once() {
    let mut run = in_dungeon(Kind::Shield, 3);
    run.projectiles.push(Projectile { x: 15.0, y: 14.0, vx: 0.0, vy: 0.0, hostile: true, kind: Shot::Orb, look: None, damage: 20, pierce: 0, last_hit: None, empowered: false, ttl: HZ, traits: Default::default() });
    let before = run.squires[0].body.hp;
    run.tick_squires();
    assert_eq!(run.projectiles[0].ttl, 0);
    assert!(run.squires[0].body.hp > before - 20);
    let mut run = in_dungeon(Kind::Healer, 3);
    let friend = run.players.get_mut(&2).unwrap();
    friend.hp = 0; friend.x = 15.0; friend.y = 14.0;
    for _ in 0..5 * HZ { run.tick_squires(); }
    assert!(run.players[&2].hp > 0);
    assert!(run.squires[0].rescued);
    run.players.get_mut(&2).unwrap().hp = 0;
    for _ in 0..6 * HZ { run.tick_squires(); }
    assert_eq!(run.players[&2].hp, 0, "one vigil rescue per room");
}

#[test]
fn squires_stale_offers_and_duplicate_requests_never_spend_twice() {
    let mut yard = Yard::default();
    let mut purse = Spoils(BTreeMap::from([(Spoil::Gold, 1000)]));
    let request = Request { owner: "pip".into(), action: Action::Hire(0), season: 0, game: "a".into(), serial: 1 };
    let marks = BTreeMap::from([(format!("squire:{}", serde_json::to_string(&request).unwrap()), 1)]);
    yard.season = 1;
    settle(&mut yard, &mut purse, &marks);
    assert_eq!(purse.get(Spoil::Gold), 1000);
    assert!(yard.companies.is_empty());
    assert!(!settle(&mut yard, &mut purse, &marks).changed);
}

#[test]
fn squires_leaving_a_plate_cancels_progress_and_release_rearms_it() {
    let mut run = Run::at_home(17, 1, None, home::Home::default(), Default::default());
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Yard).unwrap();
    run.enter_for_test(room);
    let h = run.players.get_mut(&1).unwrap();
    (h.x,h.y,h.buying,h.buy_spent) = (5.0,7.0,5,true);
    (h.x,h.y) = (10.0,16.0);
    run.tick_squire_yard(&BTreeMap::new());
    assert!(!run.players[&1].buy_spent);
    assert_eq!(run.players[&1].buying, 0);
}

#[test]
fn squire_yard_holds_do_not_steal_or_transfer_other_plates() {
    let mut run = Run::at_home(17, 1, None, home::Home::default(), Default::default());
    let room = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Yard).unwrap();
    run.enter_for_test(room);
    let pressing = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    { let h = run.players.get_mut(&1).unwrap(); (h.x, h.y) = (5.0, 7.0); }
    for _ in 0..home::BUY_HOLD - 1 { run.tick_squire_yard(&pressing); }
    assert_eq!(run.players[&1].buying, home::BUY_HOLD - 1);
    { let h = run.players.get_mut(&1).unwrap(); (h.x, h.y) = (13.0, 7.0); }
    run.tick_squire_yard(&pressing);
    assert_eq!(run.players[&1].buying, 1, "a different squire plate starts a fresh hold");
    { let h = run.players.get_mut(&1).unwrap(); (h.x, h.y) = (5.0, 23.0); }
    run.tick_squire_yard(&pressing);
    assert_eq!(run.players[&1].buying, 0, "leaving cancels the squire hold");
    run.players.get_mut(&1).unwrap().buying = 7;
    run.tick_squire_yard(&pressing);
    assert_eq!(run.players[&1].buying, 7, "another plate owns its own progress");
}

#[test]
fn squires_shield_iron_guard_and_entry_ward_still_catch_without_damage() {
    let mut run = in_dungeon(Kind::Shield, 3);
    run.squires[0].body.invulnerable = HZ;
    let hp = run.squires[0].body.hp;
    run.projectiles.push(Projectile { x:15.0,y:14.0,vx:0.0,vy:0.0,hostile:true,kind:Shot::Orb,look:None,damage:50,pierce:0,last_hit:None,empowered:false,ttl:HZ,traits:Default::default() });
    run.tick_squires();
    assert_eq!(run.projectiles[0].ttl,0);
    assert_eq!(run.squires[0].body.hp,hp);
}

#[test]
fn squires_board_turnover_replays_without_changing_offers() {
    let mut yard = Yard::default();
    let mut treasury = Spoils::default();
    let marks = BTreeMap::from([("squire_turn:0:1".into(),1)]);
    assert!(settle(&mut yard, &mut treasury, &marks).changed);
    let offers = yard.offers();
    assert!(!settle(&mut yard, &mut treasury, &marks).changed);
    assert_eq!(yard.offers(),offers);
}
