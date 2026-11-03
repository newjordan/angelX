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
    // The first floor's opening room is bare of foes now (the wishing room,
    // 2026-11-02): bring one to the knight's side for the punch.
    if run.enemies.is_empty() {
        run.spawn_at_for_test(EnemyKind::Slime, 0.0, 0.0);
    }
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
fn an_empty_seats_knight_is_owed_no_early_gear() {
    // The host who plays no knight keeps an empty seat, bare: a friend
    // who has it all is dropped nothing for it.
    let mut run = naked(3, 1, Home::default());
    run.join_seat(2, "Harry");
    run.descend_for_test();
    run.players.get_mut(&1).unwrap().absent = true;
    run.players.get_mut(&2).unwrap().owned = gear::Owned::equipped();
    for room in &mut run.dungeon.rooms {
        room.items.retain(|i| !gear::piece(&i.card));
    }
    for _ in 0..12 { run.early_gear_drop(20.0, 20.0); }
    let lying = run.dungeon.rooms.iter().flat_map(|r| &r.items).filter(|i| gear::piece(&i.card)).count();
    assert_eq!(lying, 0, "nothing for the empty seat");
    // Seated again, bare: the next kill drops them their first piece.
    run.players.get_mut(&1).unwrap().absent = false;
    run.early_gear_drop(20.0, 20.0);
    assert_eq!(run.room().items.iter().filter(|i| gear::piece(&i.card)).count(), 1);
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
        // Shared loot: one of each piece lies for the four of them.
        for _ in 0..12 { run.early_gear_drop(20.0, 20.0); }
        for piece in ["iron-helm", "body-armour", "kite-shield"] {
            assert_eq!(run.room().items.iter().filter(|i| i.card == piece).count(), 1);
        }
        let before: Vec<_> = run.enemies.iter().map(|e| e.hp).collect();
        take(&mut run, "bow");
        assert_eq!(before, run.enemies.iter().map(|e| e.hp).collect::<Vec<_>>());
        run.dungeon = layout::floor(2, Pack::Cavern, &mut run.rng);
        run.stock_gear_cache();
        for piece in ["iron-helm", "body-armour", "kite-shield"] {
            assert_eq!(run.dungeon.rooms[0].items.iter().filter(|i| i.card == piece).count(), 1);
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

#[test]
fn together_a_fresh_ranger_rolls_even_with_a_shield_kept_at_home() {
    // A ranger who went home with a kite shield in hand.
    let mut home = Home::default();
    let kept = Kept { guard: Some("kite-shield".into()), ..Kept::equipped() };
    home.wardrobe.knights.insert("Ranger".into(), kept.clone());
    home.wardrobe.knights.insert("Maiden".into(), kept);
    let mut run = naked(11, 1, home);
    run.join_seat(2, "Ranger");
    run.outfit(2, knights::knight("percival").unwrap());
    let h = &run.players[&2];
    assert_eq!(classes::of(h).map(|c| c.name), Some("Archer"));
    assert_eq!(h.guard, cards::Guard::Roll);
    assert_eq!(h.guard_card.as_deref(), Some("dodge-roll"));
    // And a new one, with nothing kept.
    run.join_seat(3, "Greenhorn");
    run.outfit(3, knights::knight("percival").unwrap());
    assert_eq!(run.players[&3].guard, cards::Guard::Roll);
    // Another class keeps the shield it brought home.
    run.join_seat(4, "Maiden");
    run.outfit(4, knights::knight("lynette").unwrap());
    assert_eq!(run.players[&4].guard, cards::Guard::Shield);
}

#[test]
fn together_a_friend_takes_knight_one_and_the_host_keeps_its_gear() {
    let mut run = naked(17, 1, Home::default());
    let host = run.players[&1].name.clone();
    run.players.get_mut(&1).unwrap().owned.helmet = true;
    run.seat_first("Ywain");
    assert_eq!(run.players[&1].name, "Ywain");
    assert!(!run.players[&1].owned.helmet, "the friend comes in their own gear");
    assert!(run.home.wardrobe.knights[&host].owned.helmet, "the host's gear is kept");
    // The same name again changes nothing.
    run.players.get_mut(&1).unwrap().owned.body = true;
    run.seat_first("Ywain");
    assert!(run.players[&1].owned.body);
}

#[test]
fn together_a_shield_taken_leaves_the_old_guard_for_its_knight_alone_and_walking_back_never_swaps() {
    let mut run = naked(4, 1, Home::default());
    run.join_seat(2, "Ann");
    run.calm_for_test();
    run.dungeon.rooms[run.at].items.clear();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    run.drop_item("kite-shield".into(), x, y, None);
    // A knight on the roll keeps it, standing on a shield: a guard of
    // another kind is taken with the key (2026-10-22).
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].guard_card.as_deref(), Some("dodge-roll"), "a roll knight never takes a shield by walking over it");
    press(&mut run);
    assert_eq!(run.players[&1].guard_card.as_deref(), Some("kite-shield"));
    // The shield lies on for Ann; the old roll is knight 1's alone.
    let items = &run.room().items;
    assert_eq!(items.len(), 2, "{items:?}");
    let shield = items.iter().find(|i| i.card == "kite-shield").unwrap();
    assert!(shield.took.contains(&1) && !shield.seen_by(Some(1)) && shield.seen_by(Some(2)));
    let roll = items.iter().find(|i| i.card == "dodge-roll").unwrap();
    assert_eq!(roll.owner, Some(1));
    assert!(roll.seen_by(Some(1)) && !roll.seen_by(Some(2)));
    // Walking off and back over it never swaps the roll back in.
    for dx in [4.0, 0.0, 4.0, 0.0] {
        run.players.get_mut(&1).unwrap().x = x + dx;
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].guard_card.as_deref(), Some("kite-shield"));
    assert_eq!(run.room().items.len(), 2, "no card made by walking about");
    // A second shield for a knight who holds one stays where it lies.
    run.drop_item("kite-shield".into(), x, y, None);
    run.step(&BTreeMap::new());
    assert_eq!(run.room().items.iter().filter(|i| i.card == "kite-shield").count(), 2);
    // The take key takes the roll back, and the shield goes down as theirs.
    press(&mut run);
    assert_eq!(run.players[&1].guard_card.as_deref(), Some("dodge-roll"));
    assert!(run.room().items.iter().any(|i| i.card == "kite-shield" && i.owner == Some(1)));
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].guard_card.as_deref(), Some("dodge-roll"), "standing there never undoes the swap");
}

#[test]
fn together_a_bottle_says_how_it_is_drunk_and_a_full_hand_says_so() {
    let mut run = naked(6, 1, Home::default());
    run.calm_for_test();
    run.dungeon.rooms[run.at].items.clear();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    run.drop_item("potion".into(), x, y, None);
    run.step(&BTreeMap::new());
    assert_eq!(run.found.as_ref().unwrap().2, "Potion: Z to drink");
    assert!(crate::rules::take_line(&run.found.as_ref().unwrap().2));
    run.players.get_mut(&1).unwrap().hand = vec!["potion".into(); cards::HAND];
    run.drop_item("holy-water".into(), x, y, None);
    run.step(&BTreeMap::new());
    assert_eq!(run.found.as_ref().unwrap().2, crate::rules::HAND_FULL);
    let said = run.found.clone();
    run.step(&BTreeMap::new());
    assert_eq!(run.found, said, "said once, not every tick");
}

#[test]
fn together_a_guard_of_another_kind_underfoot_waits_for_the_take_key() {
    let mut run = naked(4, 1, Home::default());
    run.calm_for_test();
    run.dungeon.rooms[run.at].items.clear();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    // A knight on the roll walks over a shield: it stays, and the HUD offers it on the key.
    run.drop_item("kite-shield".into(), x, y, None);
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].guard, cards::Guard::Roll);
    assert_eq!(run.weapon_beside(1).map(|c| c.id.as_str()), Some("kite-shield"));
    press(&mut run);
    assert_eq!(run.players[&1].guard, cards::Guard::Shield);
    // A knight with a shield walks over a roll that is no one's: the shield stays.
    run.dungeon.rooms[run.at].items.clear();
    run.drop_item("dodge-roll".into(), x, y, None);
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].guard, cards::Guard::Shield, "a shield is never lost to a roll card underfoot");
}
