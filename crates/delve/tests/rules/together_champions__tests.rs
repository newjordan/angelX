use super::*;

/// A champion of `kit` at (12, 14) aiming east, a tough foe at `(x, y)`.
fn arena(kit: &str, x: f32, y: f32) -> Run {
    let mut r = Run::new(21, 1, None);
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    assert!(r.take_up(1, kit));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.invulnerable = 1000;
    r.spawn_at_for_test(EnemyKind::Demon, x, y);
    tough(&mut r, 0);
    r.phase = Phase::Fighting;
    r
}

fn tough(r: &mut Run, i: usize) {
    let e = &mut r.enemies[i];
    e.hp = 10000;
    e.max_hp = 10000;
    e.seen_hp = 10000;
    e.age = TELEGRAPH;
}

fn step(r: &mut Run, i: Input) {
    r.step(&BTreeMap::from([(1, i)]));
}

fn idle(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        step(r, Input::default());
    }
}

fn cast(r: &mut Run, key: u8) {
    step(r, Input { cast: key, ..Default::default() });
}

fn lost(r: &Run, i: usize) -> u32 {
    r.enemies[i].max_hp - r.enemies[i].hp
}

#[test]
fn a_champion_takes_up_three_spells_on_keys_one_to_three_and_gives_them_back() {
    let mut r = Run::new(21, 1, None);
    for c in champions::Champion::ALL {
        assert!(r.take_up(1, c.kit()));
        let h = &r.players[&1];
        let want: Vec<Option<String>> = c.spells().iter().map(|s| Some(s.to_string())).collect();
        assert_eq!(&h.spells[..3], &want[..], "{}", c.kit());
        assert_eq!(champions::of(h), Some(c));
        assert_eq!(knights::knight(c.kit()).unwrap().max_hp + h.home_hp, h.max_hp);
        for spell in c.spells() {
            let card = r.book.get(spell).unwrap_or_else(|| panic!("{spell} is in the book"));
            assert_eq!(card.kind, cards::Kind::Spell);
            assert_eq!((card.drop, card.chest), (0, 0), "{spell} is the champion's alone");
        }
    }
    assert_eq!(ults::Ult::of(Some("hammerer")), ults::Ult::Earthsplitter);
    assert_eq!(ults::Ult::of(Some("butcher")), ults::Ult::Dismember);
    assert_eq!(ults::Ult::of(Some("alchemist")), ults::Ult::PhilosophersRain);
    // Back to a plain knight: the champion's spells go with the kit.
    assert!(r.take_up(1, "percival"));
    assert!(r.players[&1].spells.iter().flatten().all(|s| !champions::is_spell(s)));
}

#[test]
fn the_storm_hammer_flies_stuns_the_first_foe_and_bursts_round_it() {
    let mut r = arena("hammerer", 20.0, 14.0);
    r.spawn_at_for_test(EnemyKind::Demon, 21.5, 15.0);
    tough(&mut r, 1);
    cast(&mut r, 1);
    assert_eq!(r.champions.throws.len(), 1);
    idle(&mut r, 12);
    assert!(r.champions.throws.is_empty(), "it struck and fell");
    assert!(lost(&r, 0) >= 50, "the first takes it whole: {}", lost(&r, 0));
    assert!(lost(&r, 1) >= 25 && lost(&r, 1) < lost(&r, 0), "the one beside takes the burst");
    assert!(r.enemies[0].frozen > HZ, "stunned");
    // Its wait is the card's: eight seconds.
    assert_eq!(r.players[&1].spell_cooldowns[0], 8 * HZ - 12);
}

#[test]
fn groundslam_roots_through_its_wind_up_then_throws_back_stuns_and_pops_shots() {
    let mut r = arena("hammerer", 15.0, 14.0);
    cast(&mut r, 2);
    let at = r.players[&1].x;
    let before = r.enemies[0].x;
    for _ in 0..champions::SLAM_WINDUP - 2 {
        step(&mut r, Input { move_x: 1, ..Default::default() });
    }
    assert_eq!(r.players[&1].x, at, "rooted under the maul");
    assert_eq!(lost(&r, 0), 0, "nothing yet");
    idle(&mut r, 3);
    assert!(lost(&r, 0) >= 45, "the slam lands: {}", lost(&r, 0));
    assert!(r.enemies[0].frozen > 0);
    assert!(r.enemies[0].x > before + 1.0, "thrown back");
    assert!(r.champions.rings.iter().any(|g| g.kind == champions::RingKind::Quake));
}

#[test]
fn brute_force_swings_the_maul_twice_as_often_and_harder() {
    let mut r = arena("hammerer", 14.5, 14.0);
    step(&mut r, Input { fire: true, ..Default::default() });
    let plain = r.players[&1].fire_cooldown;
    let one = lost(&r, 0);
    assert!(one > 0, "the maul reaches");
    let mut r = arena("hammerer", 14.5, 14.0);
    cast(&mut r, 3);
    assert!(r.players[&1].champ.fury > 0);
    step(&mut r, Input { fire: true, ..Default::default() });
    assert_eq!(r.players[&1].fire_cooldown, plain / 2);
    assert!(lost(&r, 0) > one, "half again as hard");
}

#[test]
fn earthsplitter_runs_out_then_bursts_along_the_line_only() {
    let mut r = arena("hammerer", 22.0, 14.0);
    r.spawn_at_for_test(EnemyKind::Demon, 22.0, 22.0);
    tough(&mut r, 1);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(&mut r, Input { ult: true, ..Default::default() });
    assert_eq!(r.champions.cracks.len(), 1);
    idle(&mut r, champions::CRACK_ERUPT - 2);
    assert_eq!(lost(&r, 0), 0, "the crack is still running");
    idle(&mut r, 3);
    assert!(lost(&r, 0) >= 130, "the line bursts: {}", lost(&r, 0));
    assert_eq!(lost(&r, 1), 0, "off the line, untouched");
    assert!(r.enemies[0].frozen > HZ);
}

#[test]
fn the_meat_hook_drags_a_foe_to_the_butchers_feet() {
    let mut r = arena("butcher", 22.0, 14.0);
    cast(&mut r, 1);
    idle(&mut r, 40);
    let (hx, e) = (r.players[&1].x, &r.enemies[0]);
    assert!((e.x - hx).abs() < 3.5, "hauled in: {} from {}", e.x, hx);
    assert!(lost(&r, 0) >= 55);
    assert!(r.champions.throws.is_empty());
}

#[test]
fn chain_haul_hauls_the_butcher_toward_the_foe_and_bowls_it_over() {
    let mut r = arena("butcher", 20.0, 14.0);
    let from = r.players[&1].x;
    cast(&mut r, 3);
    idle(&mut r, 10);
    assert!(r.players[&1].x > from + 4.0, "hauled along: {}", r.players[&1].x);
    assert!(r.players[&1].x < 20.0, "short of the foe");
}

#[test]
fn rot_eats_the_near_and_spares_the_far() {
    let mut r = arena("butcher", 14.0, 14.0);
    r.spawn_at_for_test(EnemyKind::Demon, 30.0, 14.0);
    tough(&mut r, 1);
    cast(&mut r, 2);
    idle(&mut r, 2 * HZ);
    assert!(lost(&r, 0) >= 45, "two seconds of rot: {}", lost(&r, 0));
    assert_eq!(lost(&r, 1), 0);
}

#[test]
fn dismember_holds_a_foe_carves_it_and_feeds_the_butcher() {
    let mut r = arena("butcher", 15.0, 14.0);
    let h = r.players.get_mut(&1).unwrap();
    h.hp = 60;
    h.ult_charge = ults::ULT_FULL;
    step(&mut r, Input { ult: true, ..Default::default() });
    assert_eq!(r.champions.grips.len(), 1);
    idle(&mut r, champions::GRIP_TICKS);
    assert!(r.champions.grips.is_empty());
    assert!(lost(&r, 0) >= 130, "carved: {}", lost(&r, 0));
    assert!(r.players[&1].hp > 60, "fed");
    // Nobody near: the charge keeps.
    let mut r = arena("butcher", 40.0, 14.0);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(&mut r, Input { ult: true, ..Default::default() });
    assert!(r.champions.grips.is_empty());
    assert_eq!(r.players[&1].ult_charge, ults::ULT_FULL);
}

#[test]
fn the_alchemists_flask_is_lobbed_lands_bursts_and_leaves_fire() {
    let mut r = arena("alchemist", 19.0, 14.0);
    step(&mut r, Input { fire: true, ..Default::default() });
    assert_eq!(r.champions.throws.len(), 1);
    assert!(r.projectiles.iter().all(|p| p.hostile), "a flask, not a stone");
    idle(&mut r, champions::FLASK_FLIGHT + 1);
    assert!(r.champions.throws.is_empty());
    assert!(r.works.blazes.iter().any(|b| b.owner == 1), "the ground burns");
    assert!(lost(&r, 0) > 0);
}

#[test]
fn fire_thrown_into_the_alchemists_foul_air_flashes_it_off() {
    let mut r = arena("alchemist", 19.0, 14.0);
    cast(&mut r, 1);
    idle(&mut r, champions::FLASK_FLIGHT + 1);
    assert_eq!(r.works.clouds.iter().filter(|c| c.owner == 1).count(), 1);
    cast(&mut r, 2);
    idle(&mut r, champions::FLASK_FLIGHT + 2);
    assert_eq!(r.works.clouds.iter().filter(|c| c.owner == 1).count(), 0, "it went up");
}

#[test]
fn the_elixir_mends_friends_near_and_not_far() {
    let mut r = Run::new(21, 1, Some("Friend"));
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    r.calm_for_test();
    r.enemies.clear();
    assert!(r.take_up(1, "alchemist"));
    for (id, x) in [(1, 12.0), (2, 12.0 + champions::ELIXIR_REACH + 4.0)] {
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y, h.hp) = (x, 14.0, 40);
    }
    r.step(&BTreeMap::from([(1, Input { cast: 3, ..Default::default() })]));
    assert!(r.players[&1].hp > 40);
    assert_eq!(r.players[&2].hp, 40);
}

#[test]
fn philosophers_rain_throws_foul_air_then_fire_on_each_foe() {
    let mut r = arena("alchemist", 20.0, 14.0);
    r.spawn_at_for_test(EnemyKind::Demon, 20.0, 20.0);
    tough(&mut r, 1);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(&mut r, Input { ult: true, ..Default::default() });
    assert_eq!(r.champions.throws.len(), 4, "two flasks to each of the two");
    idle(&mut r, 2 * HZ);
    assert!(lost(&r, 0) > 34 && lost(&r, 1) > 34, "{} {}", lost(&r, 0), lost(&r, 1));
}

#[test]
fn a_champions_run_round_trips_with_its_things_in_the_air() {
    let mut r = arena("butcher", 22.0, 14.0);
    cast(&mut r, 1);
    cast(&mut r, 2);
    let text = serde_json::to_string(&r).unwrap();
    let back: Run = serde_json::from_str(&text).unwrap();
    assert_eq!(back.champions, r.champions);
    assert_eq!(back.players[&1].champ, r.players[&1].champ);
    // A plain knight's run writes nothing of them.
    let plain = Run::new(21, 1, None);
    let text = serde_json::to_string(&plain).unwrap();
    assert!(!text.contains("\"champ") && !text.contains("\"champions\""));
}

#[test]
fn three_champions_and_a_knight_mash_every_key_through_a_fight_and_a_floor_change() {
    let mut r = Run::new(21, 1, Some("Friend"));
    for id in 3..=4 {
        r.join_seat(id, &format!("Seat {id}"));
    }
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    for (id, kit) in [(1, "hammerer"), (2, "butcher"), (3, "alchemist"), (4, "percival")] {
        assert!(r.take_up(id, kit));
    }
    let mut salt = 7u64;
    let (mut thrown, mut cracked, mut cast) = (0, 0, 0);
    for t in 0..900u64 {
        let mut inputs = BTreeMap::new();
        for id in 1..=4u32 {
            salt = salt.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let roll = (salt >> 33) as u32;
            let axis = |k: u32| ((roll >> k) % 3) as i8 - 1;
            inputs.insert(id, Input {
                move_x: axis(0),
                move_y: axis(2),
                aim_x: axis(4),
                aim_y: axis(6),
                fire: roll % 3 != 0,
                dash: roll % 41 == 0,
                cast: ((roll >> 8) % 7) as u8 % 4,
                ult: roll % 53 == 0,
                ..Default::default()
            });
            if t % 120 == 0 {
                r.players.get_mut(&id).unwrap().ult_charge = ults::ULT_FULL;
            }
        }
        r.step(&inputs);
        for e in &r.enemies {
            assert!(e.x.is_finite() && e.y.is_finite());
        }
        for h in r.players.values() {
            assert!(h.x.is_finite() && h.y.is_finite());
        }
        assert!(r.champions.throws.len() < 64 && r.champions.rings.len() <= 24);
        thrown += r.champions.throws.len();
        cracked += r.champions.cracks.len();
        cast += r.players.values().filter(|h| h.champ.cast > 0).count();
    }
    assert!(thrown > 0 && cracked > 0 && cast > 0, "{thrown} {cracked} {cast}");
    // Leaving the room leaves its hooks and cracks behind.
    let other = r.dungeon.rooms.iter().enumerate().position(|(i, room)| i != r.at && room.kind == RoomKind::Fight).unwrap();
    r.enter_for_test(other);
    assert!(r.champions.is_empty());
}

#[test]
fn every_champion_can_swap_a_found_arm_without_losing_spells() {
    for champion in champions::Champion::ALL {
        let mut r = arena(champion.kit(), 40.0, 14.0);
        let original = r.players[&1].arm.clone().unwrap();
        let spells = r.players[&1].spells.clone();
        let at = r.at;
        let (x, y) = (r.players[&1].x, r.players[&1].y);
        r.dungeon.rooms[at].items.push(Item {
            card: "handgonne".into(), x, y, held_off: None,
            took: Vec::new(), dropped: 0, owner: None,
        });
        step(&mut r, Input { take: true, ..Input::default() });
        idle(&mut r, 2);
        let h = &r.players[&1];
        assert_eq!(h.arm.as_deref(), Some("handgonne"), "{} may choose a gun", champion.kit());
        assert_eq!(h.spells, spells, "swapping an arm does not change a calling");
        assert_eq!(champions::of(h), Some(champion));
        assert!(r.room().items.iter().any(|i| i.card == original), "the old arm returns to the floor");
        assert!(!r.room().items.iter().any(|i| i.card == "handgonne"));
    }
}

#[test]
fn the_table_arms_a_champion_again_who_lost_his_arm() {
    // An older save: the maul already swapped away for a found gun.
    let mut r = arena("hammerer", 40.0, 14.0);
    let gun = r.book.get("handgonne").unwrap().clone();
    r.players.get_mut(&1).unwrap().equip(&gun);
    assert_eq!(r.players[&1].arm.as_deref(), Some("handgonne"));
    let i = alehouse::PATRONS.iter().position(|p| p.kit == "hammerer").unwrap();
    r.alehouse.company_page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, i)).unwrap();
    r.pick_patron(1, i);
    let h = &r.players[&1];
    assert_eq!(h.arm.as_deref(), Some("great-maul"), "the Hammerer at the bar hands the maul back");
    assert!(h.forged.as_ref().is_some_and(|w| w.melee.is_some()));
    // Armed, a second word at the table is the old one.
    r.pick_patron(1, i);
    assert_eq!(r.players[&1].arm.as_deref(), Some("great-maul"));
}

#[test]
fn alchemist_fires_his_equipped_gun_in_host_and_prediction() {
    let mut r = arena("alchemist", 40.0, 14.0);
    let gun = r.book.get("handgonne").unwrap().clone();
    r.players.get_mut(&1).unwrap().equip(&gun);
    let mut predicted = r.clone();
    let mut shots = Vec::new();
    let input = Input { fire: true, aim_x: 1, ..Default::default() };
    assert!(predicted.predict_fire(1, input, r.tick + 1, &mut shots));
    assert!(!shots.is_empty(), "the equipped gun fires in prediction");
    step(&mut r, input);
    assert!(r.projectiles.iter().any(|p| !p.hostile), "the equipped gun fires on the server");
    assert!(r.champions.throws.is_empty(), "an Alchemist need not throw flasks");
    assert_eq!(r.players[&1].fire_cooldown, predicted.players[&1].fire_cooldown);
}

#[test]
fn any_knight_can_use_the_flask_bandolier_without_becoming_alchemist() {
    let mut r = arena("percival", 40.0, 14.0);
    let arm = r.book.get("flask-bandolier").unwrap().clone();
    r.players.get_mut(&1).unwrap().equip(&arm);
    let mut predicted = r.clone();
    let mut shots = Vec::new();
    let input = Input { fire: true, aim_x: 1, ..Default::default() };
    assert!(predicted.predict_fire(1, input, r.tick + 1, &mut shots));
    step(&mut r, input);
    assert!(shots.is_empty(), "flasks use the shared thrown rendering, not stone shots");
    assert!(r.projectiles.iter().all(|p| p.hostile));
    assert!(r.champions.throws.iter().any(|t| matches!(t.kind, champions::Thrown::Flask(champions::Brew::Fire))));
    assert_eq!(r.players[&1].fire_cooldown, predicted.players[&1].fire_cooldown);
    assert_eq!(r.players[&1].knight.as_deref(), Some("percival"));
}

#[test]
fn the_reaver_and_the_lancer_carry_their_own_ultimates() {
    assert_eq!(ults::Ult::of(Some("reaver")), ults::Ult::RedHarvest);
    assert_eq!(ults::Ult::of(Some("lancer")), ults::Ult::SkewerDrive);
    for u in [ults::Ult::RedHarvest, ults::Ult::SkewerDrive] {
        assert_eq!(ults::Ult::parse(u.word()), Some(u), "{u:?} reads back from its word");
        assert_eq!(ults::Ult::parse(u.name()), Some(u), "{u:?} reads back from its name");
    }
}

#[test]
fn the_hurled_axe_cuts_on_its_way_out_and_home_and_comes_back() {
    let mut r = arena("reaver", 17.0, 14.0);
    cast(&mut r, 1);
    assert_eq!(r.champions.throws.len(), 1);
    idle(&mut r, 60);
    assert!(r.champions.throws.is_empty(), "the axe came home");
    assert!(lost(&r, 0) >= champions::AXE_DAMAGE, "cut: {}", lost(&r, 0));
}

#[test]
fn the_leap_bounds_along_the_aim_and_lands_in_a_quake_that_stuns() {
    let mut r = arena("reaver", 21.0, 14.0);
    let from = r.players[&1].x;
    cast(&mut r, 2);
    assert!(r.players[&1].champ.dash > 0, "in the air");
    idle(&mut r, 20);
    let h = &r.players[&1];
    assert_eq!(h.champ.dash, 0);
    assert!(h.x > from + 4.0, "bounded: {} from {from}", h.x);
    assert!(h.x < 21.0, "short of the foe");
    assert!(lost(&r, 0) >= champions::LEAP_DAMAGE, "the quake: {}", lost(&r, 0));
    assert!(h.champ.idle() || h.champ.dash_kind == champions::Dash::default(), "a bound done leaves no trace");
}

#[test]
fn bloodlust_feeds_the_reaver_on_his_cuts_and_quickens_him() {
    let mut r = arena("reaver", 13.6, 14.0);
    r.players.get_mut(&1).unwrap().hp = 60;
    cast(&mut r, 3);
    assert!(r.players[&1].champ.lust > 0);
    for _ in 0..40 {
        step(&mut r, Input { fire: true, ..Default::default() });
    }
    assert!(lost(&r, 0) > 0);
    assert!(r.players[&1].hp > 60, "fed: {}", r.players[&1].hp);
    // Quicker on his feet while it lasts.
    let mut slow = arena("reaver", 40.0, 14.0);
    let mut fast = arena("reaver", 40.0, 14.0);
    cast(&mut fast, 3);
    idle(&mut slow, 1);
    let (a, b) = (slow.players[&1].x, fast.players[&1].x);
    for _ in 0..15 {
        step(&mut slow, Input { move_x: -1, ..Default::default() });
        step(&mut fast, Input { move_x: -1, ..Default::default() });
    }
    assert!(b - fast.players[&1].x > (a - slow.players[&1].x) * 1.15, "faster in the bloodlust");
}

#[test]
fn red_harvest_reaps_the_wounded_bleeds_the_rest_and_gives_back_charge() {
    let mut r = arena("reaver", 14.5, 14.0);
    r.spawn_at_for_test(EnemyKind::Demon, 12.0, 15.5);
    let wounded = &mut r.enemies[1];
    (wounded.max_hp, wounded.hp, wounded.age) = (200, 40, TELEGRAPH);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(&mut r, Input { ult: true, ..Default::default() });
    assert_eq!(r.enemies.iter().find(|e| e.max_hp == 200).map_or(0, |e| e.hp), 0, "reaped outright");
    assert!(lost(&r, 0) >= champions::HARVEST_DAMAGE, "the hale one bleeds: {}", lost(&r, 0));
    assert!(lost(&r, 0) < 10000, "but is not reaped");
    assert!(r.players[&1].ult_charge >= ults::ULT_FULL / 5, "a reaped foe gives charge back");
    assert!(r.champions.rings.iter().any(|g| g.kind == champions::RingKind::Reap));
}

#[test]
fn the_javelin_flies_through_every_foe_on_its_line() {
    let mut r = arena("lancer", 16.0, 14.0);
    for (i, x) in [(1, 19.5), (2, 23.0)] {
        r.spawn_at_for_test(EnemyKind::Demon, x, 14.0);
        tough(&mut r, i);
    }
    cast(&mut r, 1);
    idle(&mut r, champions::JAVELIN_LIFE + 2);
    for i in 0..3 {
        assert!(lost(&r, i) >= champions::JAVELIN_DAMAGE, "foe {i} on the line: {}", lost(&r, i));
    }
    assert!(r.champions.throws.is_empty());
}

#[test]
fn the_vault_carries_her_over_the_front_rank_cutting_as_she_goes() {
    let mut r = arena("lancer", 14.0, 14.0);
    let from = r.players[&1].x;
    cast(&mut r, 2);
    idle(&mut r, 12);
    assert!(r.players[&1].x > 14.5, "over the foe: {} from {from}", r.players[&1].x);
    assert!(lost(&r, 0) > 0, "cut on the way over");
}

#[test]
fn the_braced_lance_roots_her_halves_blows_and_throws_back_a_rusher() {
    let mut r = arena("lancer", 13.8, 14.0);
    let (x0, d0) = (r.players[&1].x, r.enemies[0].x - r.players[&1].x);
    cast(&mut r, 3);
    assert!(r.players[&1].champ.brace > 0);
    for _ in 0..10 {
        step(&mut r, Input { move_x: -1, ..Default::default() });
    }
    assert_eq!(r.players[&1].x, x0, "rooted behind the lance");
    assert!(lost(&r, 0) > 0, "the rusher is struck");
    assert!(r.enemies[0].x - r.players[&1].x > d0, "and thrown back");
    // The same blow, braced and not.
    let h = r.players.get_mut(&1).unwrap();
    (h.invulnerable, h.hp) = (0, 200);
    h.attack_hurt(40);
    let braced = 200 - h.hp;
    (h.champ.brace, h.hp, h.invulnerable) = (0, 200, 0);
    h.attack_hurt(40);
    let open = 200 - h.hp;
    assert!(braced * 2 <= open + 1 && braced > 0, "a blow lands at half: {braced} braced, {open} open");
}

#[test]
fn the_skewer_drive_carries_foes_before_the_lance_and_pins_them() {
    let mut r = arena("lancer", 15.0, 14.0);
    let from = r.players[&1].x;
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(&mut r, Input { ult: true, ..Default::default() });
    assert!(r.players[&1].champ.dash > 0, "the charge runs");
    idle(&mut r, 6);
    assert_eq!(r.champions.skewered.len(), 1, "the foe is on the lance");
    idle(&mut r, 30);
    assert!(r.champions.skewered.is_empty(), "pinned and let go");
    assert!(r.players[&1].x > from + 5.0, "she drove on: {}", r.players[&1].x);
    assert!(r.enemies[0].x > 17.0, "carried: {}", r.enemies[0].x);
    assert!(lost(&r, 0) >= champions::DRIVE_DAMAGE, "pinned: {}", lost(&r, 0));
}

#[test]
fn the_reaver_and_the_lancer_mash_every_key_through_a_fight_and_a_floor_change() {
    let mut r = Run::new(23, 1, Some("Friend"));
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    for (id, kit) in [(1, "reaver"), (2, "lancer")] {
        assert!(r.take_up(id, kit));
    }
    let mut salt = 11u64;
    let (mut thrown, mut bounded) = (0, 0);
    for t in 0..900u64 {
        let mut inputs = BTreeMap::new();
        for id in 1..=2u32 {
            salt = salt.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let roll = (salt >> 33) as u32;
            let axis = |k: u32| ((roll >> k) % 3) as i8 - 1;
            inputs.insert(id, Input {
                move_x: axis(0),
                move_y: axis(2),
                aim_x: axis(4),
                aim_y: axis(6),
                fire: roll % 3 != 0,
                dash: roll % 41 == 0,
                cast: ((roll >> 8) % 7) as u8 % 4,
                ult: roll % 53 == 0,
                ..Default::default()
            });
            if t % 120 == 0 {
                r.players.get_mut(&id).unwrap().ult_charge = ults::ULT_FULL;
            }
        }
        r.step(&inputs);
        for e in &r.enemies {
            assert!(e.x.is_finite() && e.y.is_finite());
        }
        for h in r.players.values() {
            assert!(h.x.is_finite() && h.y.is_finite());
        }
        assert!(r.champions.throws.len() < 64 && r.champions.skewered.len() < 64);
        thrown += r.champions.throws.len();
        bounded += r.players.values().filter(|h| h.champ.dash > 0).count();
    }
    assert!(thrown > 0 && bounded > 0, "{thrown} {bounded}");
    let other = r.dungeon.rooms.iter().enumerate().position(|(i, room)| i != r.at && room.kind == RoomKind::Fight).unwrap();
    r.enter_for_test(other);
    assert!(r.champions.is_empty(), "nothing of theirs follows them out");
}
