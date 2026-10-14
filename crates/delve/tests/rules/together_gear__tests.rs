use super::*;
use crate::rules::home::Home;

fn press(run: &mut Run) {
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
}
/// A run as the game starts one: knights with nothing kept are naked.
fn naked(seed: u64, raid: u64, home: Home) -> Run {
    let mut run = Run::at_home(seed, raid, None, home, Spoils::default());
    run.naked_start = true;
    for id in run.players.keys().copied().collect::<Vec<_>>() {
        run.restore_gear(id);
    }
    run
}
fn take(run: &mut Run, card: &str) {
    let h = &run.players[&1];
    let (x, y) = (h.x, h.y);
    run.dungeon.rooms[run.at].items.clear();
    run.drop_item(card.into(), x, y, None);
    press(run);
}

#[test]
fn together_fresh_knight_is_naked_and_f_punches_quickly() {
    let mut run = naked(17, 1, Home::default());
    run.outfit(1, knights::knight("lynette").unwrap());
    let h = &run.players[&1];
    assert!(h.unarmed());
    assert_eq!(h.owned, Owned::default());
    assert_eq!((h.gear, h.bombs), (2, 0));
    assert!(h.hand.is_empty() && h.arm.is_none() && h.forged.is_none());
    run.descend_for_test();
    run.enemies.truncate(1);
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    let e = &mut run.enemies[0];
    (e.x, e.y, e.hp, e.age) = (hx + 1.1, hy, 100, 0);
    run.step(&BTreeMap::from([(1, Input { fire: true, aim_x: 1, ..Default::default() })]));
    assert_eq!(run.enemies[0].hp, 94);
    assert!(run.enemies[0].x > hx + 1.1);
    assert!(run.enemies[0].frozen > 0);
    assert!(!run.feel.impacts.is_empty());
    assert!(run.projectiles.iter().all(|p| p.hostile));
    assert!(run.players[&1].swing > 0);
    assert_eq!(run.tray(1, &hud::Keys::FIRST)[0].name, "Fists");
}

#[test]
fn together_first_two_floors_supply_every_naked_knight_without_weakening_foes() {
    for seed in 0..32 {
        let mut run = naked(seed, 1, Home::default());
        for id in 2..=4 { run.join_seat(id, &format!("Friend {id}")); }
        run.descend_for_test();
        let weapons = run.dungeon.rooms[0].items.iter().filter(|i| run.book.get(&i.card).unwrap().kind == cards::Kind::Arm).count();
        assert_eq!(weapons, 4, "seed {seed}");
        assert!(run.dungeon.rooms[0].chest.is_some());
        for _ in 0..12 { run.early_gear_drop(20.0, 20.0); }
        for piece in ["iron-helm", "body-armour", "kite-shield"] {
            assert_eq!(run.room().items.iter().filter(|i| i.card == piece).count(), 4);
        }
        let before: Vec<_> = run.enemies.iter().map(|e| e.hp).collect();
        take(&mut run, "bow");
        assert_eq!(before, run.enemies.iter().map(|e| e.hp).collect::<Vec<_>>());
        run.dungeon = layout::floor(2, Pack::Cavern, &mut run.rng);
        run.stock_gear_cache();
        for piece in ["iron-helm", "body-armour", "kite-shield"] {
            assert_eq!(run.dungeon.rooms[0].items.iter().filter(|i| i.card == piece).count(), 4);
        }
    }
}

#[test]
fn together_e_equips_each_piece_and_dress_tracks_ownership() {
    let mut run = naked(3, 1, Home::default());
    run.dungeon.rooms[run.at].items.clear();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    run.drop_item("bow".into(), x, y, None);
    run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
    assert!(run.players[&1].unarmed(), "F never takes gear");
    press(&mut run);
    assert_eq!(run.players[&1].weapon, Weapon::Bow);
    take(&mut run, "body-armour");
    assert_eq!(run.players[&1].gear, 1);
    take(&mut run, "iron-helm");
    assert_eq!(run.players[&1].gear, 0);
    assert!(run.found.as_ref().unwrap().2.starts_with("Iron helm: worn"));
    take(&mut run, "kite-shield");
    assert_eq!(run.players[&1].owned, Owned::equipped());
    assert_eq!(run.players[&1].guard, cards::Guard::Shield);
    let h = run.players.get_mut(&1).unwrap();
    h.hp = 40; h.dress(); assert_eq!(h.gear, 1);
    h.hp = 15; h.dress(); assert_eq!(h.gear, 2);
    h.hp = 100; h.dress(); assert_eq!(h.gear, 0);
}

#[test]
fn together_found_gear_is_kept_on_return_and_only_unbanked_gear_is_lost_on_wipe() {
    let mut run = Run::new(5, 1, None);
    run.naked_start = true;
    run.restore_gear(1);
    run.calm_for_test();
    take(&mut run, "crossbow");
    take(&mut run, "body-armour");
    run.retreat();
    let saved: Home = serde_json::from_value(serde_json::to_value(&run.home).unwrap()).unwrap();
    let mut next = naked(5, 2, saved);
    assert_eq!(next.players[&1].weapon, Weapon::Crossbow);
    assert!(next.players[&1].owned.body);
    take(&mut next, "iron-helm");
    next.descend_for_test();
    next.players.get_mut(&1).unwrap().hp = 0;
    next.step(&BTreeMap::new());
    assert_eq!(next.phase, Phase::Wiped);
    assert!(!next.players[&1].owned.helmet);
    assert!(next.players[&1].owned.body);
    let home = next.home.clone();
    let mut again = naked(5, 3, home);
    assert_eq!(again.players[&1].weapon, Weapon::Crossbow);
    again.join_seat(2, "Browser friend");
    again.players.get_mut(&2).unwrap().wear_piece("iron-helm");
    again.leave(2);
    again.join_seat(3, "Browser friend");
    assert!(again.players[&3].owned.helmet, "gear follows names across seats");
    again.join_seat(4, "New friend");
    assert!(again.players[&4].unarmed());
}

#[test]
fn together_missing_old_save_gear_means_equipped_but_new_knights_are_naked() {
    let mut old = serde_json::to_value(Home::default()).unwrap();
    old.as_object_mut().unwrap().remove("wardrobe");
    let home: Home = serde_json::from_value(old).unwrap();
    let mut run = naked(5, 1, home);
    assert_eq!(run.players[&1].owned, Owned::equipped());
    assert_eq!(run.players[&1].weapon, Weapon::Bow);
    run.join_seat(2, "New knight");
    assert!(run.players[&2].unarmed());
    assert_eq!(run.players[&2].owned, Owned::default());
    let mut hero = serde_json::to_value(&run.players[&1]).unwrap();
    hero.as_object_mut().unwrap().remove("owned");
    let h: Hero = serde_json::from_value(hero).unwrap();
    assert_eq!(h.owned, Owned::equipped());
    let mut malformed = serde_json::to_value(&run.home).unwrap();
    malformed["wardrobe"] = serde_json::json!("broken");
    let recovered: Home = serde_json::from_value(malformed).unwrap();
    assert!(!recovered.wardrobe.legacy);
}

#[test]
fn together_each_owned_piece_gives_real_protection() {
    let mut bare = Hero::new("Fresh", 0.0, 0.0);
    bare.owned = Owned::default();
    bare.dress();
    bare.invulnerable = 0;
    bare.hurt(20);
    for piece in ["iron-helm", "body-armour"] {
        let mut dressed = Hero::new("Found", 0.0, 0.0);
        dressed.owned = Owned::default();
        dressed.wear_piece(piece);
        dressed.invulnerable = 0;
        dressed.hurt(20);
        assert!(dressed.hp > bare.hp, "{piece}");
    }
}
