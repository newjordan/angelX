use super::*;

fn home() -> Run {
    let mut run = Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    run.home.industry.start(crate::kingdom::catalog());
    run
}

fn issue(run: &mut Run, player: u32, id: &str) {
    run.home.industry.stock.insert(id.into(), 1);
    run.kingdom_gear_action(player, id).unwrap();
}

fn fight(id: &str) -> Run {
    let mut source = home();
    issue(&mut source, 1, id);
    let mut run = Run::new(21, 1, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.open_room_for_test(room);
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.home = source.home;
    let hero = run.players.get_mut(&1).unwrap();
    hero.name = source.players[&1].name.clone();
    hero.deck.clear();
    hero.rebonus(&run.book);
    hero.spells = Default::default();
    hero.module_spells = Default::default();
    hero.spell_cooldowns = Default::default();
    hero.knight = None;
    hero.stone = false;
    hero.idle = false;
    hero.absent = false;
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (12.0, 14.0, 1.0, 0.0);
    hero.invulnerable = 0;
    run.outfit_kingdom_gear();
    run.phase = Phase::Fighting;
    run
}

#[test]
fn six_checked_definitions_have_distinct_roles_and_cannot_drop_for_free() {
    let mut ids = BTreeSet::new();
    for gear in ITEMS {
        assert!(ids.insert(gear.id));
        let definition = cards::check(gear.id, gear.raw).unwrap().card;
        assert_eq!((definition.drop, definition.chest), (0, 0));
        assert!(!definition.art.is_empty());
        assert!(!definition.text.is_empty());
        assert_eq!(card(gear.id), Some(&definition));
        if let Some(cards::Arm::Forged(weapon)) = &definition.arm {
            assert!(weapon.dps() <= 70.0, "{}", gear.id);
        }
        assert!(
            crate::kingdom::catalog().resources.contains_key(gear.id),
            "{} needs a production output",
            gear.id
        );
        assert!(
            crate::kingdom::catalog().plans[gear.station]
                .recipes
                .iter()
                .any(|id| {
                    crate::kingdom::catalog().recipes[id]
                        .outputs
                        .contains_key(gear.id)
                }),
            "{} needs a workshop recipe",
            gear.id
        );
    }
    assert_eq!(ids.len(), 6);
}

#[test]
fn issuing_consumes_one_finished_item_and_re_equipping_never_mints_or_stacks() {
    let mut run = home();
    run.home.industry.stock.insert("iron-oathblade".into(), 2);
    let revision = run.home.industry.rev;
    run.kingdom_gear_action(1, "iron-oathblade").unwrap();
    assert_eq!(run.home.industry.amount("iron-oathblade"), 1);
    assert_eq!(run.home.industry.rev, revision + 1);
    let ledger = run.home.kingdom_gear.clone();
    let stock = run.home.industry.clone();
    let items = run
        .dungeon
        .rooms
        .iter()
        .map(|r| r.items.len())
        .sum::<usize>();
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 30;
    hero.mana = 10.0;
    hero.fire_cooldown = 25;
    hero.temper = 2;
    for _ in 0..5 {
        run.kingdom_gear_action(1, "iron-oathblade").unwrap();
        run.outfit_kingdom_gear();
    }
    assert_eq!(run.home.kingdom_gear, ledger);
    assert_eq!(run.home.industry, stock);
    assert_eq!(run.players[&1].hp, 30);
    assert_eq!(run.players[&1].mana, 10.0);
    assert_eq!(run.players[&1].fire_cooldown, 25);
    assert_eq!(run.players[&1].temper, 2);
    assert_eq!(
        run.dungeon
            .rooms
            .iter()
            .map(|r| r.items.len())
            .sum::<usize>(),
        items
    );
}

#[test]
fn missing_stock_dead_seats_and_full_spell_slots_fail_before_spending() {
    let mut run = home();
    let before = serde_json::to_value(&run).unwrap();
    assert!(run.kingdom_gear_action(1, "ember-edge").is_err());
    assert_eq!(serde_json::to_value(&run).unwrap(), before);
    run.home.industry.stock.insert("hearth-wardstone".into(), 1);
    run.players.get_mut(&1).unwrap().spells = std::array::from_fn(|_| Some("ember-ring".into()));
    let before = serde_json::to_value(&run).unwrap();
    assert!(
        run.kingdom_gear_action(1, "hearth-wardstone")
            .unwrap_err()
            .contains("slot")
    );
    assert_eq!(serde_json::to_value(&run).unwrap(), before);
    run.players.get_mut(&1).unwrap().hp = 0;
    run.home.industry.stock.insert("iron-oathblade".into(), 1);
    let before = serde_json::to_value(&run).unwrap();
    assert!(run.kingdom_gear_action(1, "iron-oathblade").is_err());
    assert_eq!(serde_json::to_value(&run).unwrap(), before);
}

#[test]
fn ownership_is_per_profile_and_each_friend_needs_their_own_produced_item() {
    let mut run = home();
    issue(&mut run, 1, "iron-oathblade");
    assert!(!run.kingdom_gear_status(2, "iron-oathblade").unwrap().owned);
    assert!(run.kingdom_gear_action(2, "iron-oathblade").is_err());
    issue(&mut run, 2, "iron-oathblade");
    assert_eq!(run.home.kingdom_gear.profiles.len(), 2);
    let name = run.players[&1].name.to_uppercase();
    run.players.get_mut(&1).unwrap().name = name;
    assert!(run.kingdom_gear_status(1, "iron-oathblade").unwrap().owned);
}

#[test]
fn a_new_player_in_the_same_seat_cannot_inherit_bound_gear() {
    let mut run = home();
    for id in ["iron-oathblade", "runebound-aegis", "hearth-wardstone"] {
        issue(&mut run, 1, id);
    }
    let owner = run.players[&1].name.clone();
    let ledger = run.home.kingdom_gear.clone();
    let stock = run.home.industry.clone();
    run.seat_first("New friend");
    let hero = &run.players[&1];
    assert!(!hero.arm.as_deref().is_some_and(|id| item(id).is_some()));
    assert!(hero.forged.is_none());
    assert_eq!(hero.guard, cards::Guard::Roll);
    assert!(!hero.spells.iter().flatten().any(|id| item(id).is_some()));
    assert!(
        !hero
            .bar
            .iter()
            .flatten()
            .any(|slot| { matches!(slot, cards::BarSlot::Spell(id) if item(id).is_some()) })
    );
    assert_eq!(run.home.kingdom_gear, ledger);
    assert_eq!(run.home.industry, stock);
    run.seat_first(&owner);
    for id in ["iron-oathblade", "runebound-aegis", "hearth-wardstone"] {
        assert!(run.kingdom_gear_status(1, id).unwrap().equipped);
    }
    assert_eq!(run.home.industry, stock);
}

#[test]
fn invalid_player_names_do_not_spend_or_poison_the_persistent_ledger() {
    for name in [String::new(), "n".repeat(257)] {
        let mut run = home();
        run.home.industry.stock.insert("iron-oathblade".into(), 1);
        run.players.get_mut(&1).unwrap().name = name;
        let before = serde_json::to_value(&run).unwrap();
        assert!(run.kingdom_gear_action(1, "iron-oathblade").is_err());
        assert_eq!(serde_json::to_value(&run).unwrap(), before);
    }
}

#[test]
fn lost_relic_waits_for_a_free_slot_then_restores_without_spending_or_refreshing() {
    let mut run = home();
    issue(&mut run, 1, "hearth-wardstone");
    let ledger = run.home.kingdom_gear.clone();
    let stock = run.home.industry.clone();
    let hero = run.players.get_mut(&1).unwrap();
    hero.spells = std::array::from_fn(|_| Some("ember-ring".into()));
    hero.bar = Default::default();
    hero.spell_cooldowns = [90; cards::SPELL_SLOTS];
    run.restore_kingdom_gear(1);
    assert!(
        !run.kingdom_gear_status(1, "hearth-wardstone")
            .unwrap()
            .equipped
    );
    run.players.get_mut(&1).unwrap().spells[2] = None;
    run.restore_kingdom_gear(1);
    assert_eq!(
        run.players[&1].spells[2].as_deref(),
        Some("hearth-wardstone")
    );
    assert_eq!(run.players[&1].spell_cooldowns[2], 90);
    assert_eq!(run.home.kingdom_gear, ledger);
    assert_eq!(run.home.industry, stock);
}

#[test]
fn cauldron_never_consumes_a_bound_relic_as_a_renewable_ingredient() {
    let mut run = fight("hearth-wardstone");
    assert!(run.book.get("cabbage-storm").is_some());
    assert!(run.book.get("hedge-maze").is_some());
    run.tick = 120;
    let hero = run.players.get_mut(&1).unwrap();
    hero.spells[1] = Some("cabbage-storm".into());
    run.cauldron = Some(cauldron::Cauldron {
        raid: run.raid_id,
        depth: run.dungeon.depth,
        room: run.at,
        x: hero.x,
        y: hero.y,
        rose: 0,
        ..Default::default()
    });
    assert_eq!(run.cauldron_ask(1), Some(cauldron::Ask::Short));
    run.players.get_mut(&1).unwrap().spells[2] = Some("hedge-maze".into());
    assert_eq!(run.cauldron_ask(1), Some(cauldron::Ask::Brew));
    let press = |run: &mut Run, input| {
        run.tend_cauldron(&BTreeMap::from([(1, input)]));
        run.tend_cauldron(&BTreeMap::new());
    };
    press(
        &mut run,
        Input {
            interact: true,
            ..Default::default()
        },
    );
    press(
        &mut run,
        Input {
            cast: 1,
            ..Default::default()
        },
    );
    assert!(
        run.cauldron
            .as_ref()
            .unwrap()
            .stir(1)
            .unwrap()
            .picks
            .is_empty()
    );
    // A saved selection made before this restriction is also harmless.
    run.cauldron.as_mut().unwrap().stirs[0].picks = vec![0, 1];
    press(
        &mut run,
        Input {
            interact: true,
            ..Default::default()
        },
    );
    assert_eq!(
        run.players[&1].spells[0].as_deref(),
        Some("hearth-wardstone")
    );
    assert!(run.cauldron.as_ref().unwrap().brews.is_empty());
    assert!(run.cauldron.as_ref().unwrap().brewed.is_empty());
}

#[test]
fn loadout_persists_roundtrip_and_newer_equipment_survives_stale_home_sync() {
    let mut run = home();
    let stale = run.home.clone();
    issue(&mut run, 1, "mooncrystal-staff");
    issue(&mut run, 1, "runebound-aegis");
    issue(&mut run, 1, "hearth-wardstone");
    let expected = run.home.kingdom_gear.clone();
    run.rebuild_home(stale, Default::default());
    assert_eq!(run.home.kingdom_gear, expected);
    let saved = serde_json::to_string(&run).unwrap();
    let mut restored: Run = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored.home.kingdom_gear, expected);
    restored.players.get_mut(&1).unwrap().arm = None;
    restored.players.get_mut(&1).unwrap().forged = None;
    restored.outfit_kingdom_gear();
    assert_eq!(
        restored.players[&1].arm.as_deref(),
        Some("mooncrystal-staff")
    );
    assert!(restored.valid_snapshot());
    let old: home::Home = serde_json::from_str("{}").unwrap();
    assert!(old.kingdom_gear.is_empty());
}

#[test]
fn settlement_keeps_a_second_issue_made_while_the_first_is_being_saved() {
    use crate::rules::home_settlement::{HomeContext, HomePolicyRequest, HomeTransfer};
    let mut run = home();
    issue(&mut run, 1, "iron-oathblade");
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let req = HomePolicyRequest {
        context: HomeContext {
            home: Default::default(),
            treasury: Default::default(),
            reclaimed: Default::default(),
            raids_won: 0,
        },
        transfer: transfer.clone(),
        host: "Host".into(),
        unsaved: false,
    };
    let result = req.evaluate().unwrap();
    issue(&mut run, 1, "prospectors-pick");
    let expected = run.home.kingdom_gear.clone();
    let stock = run.home.industry.clone();
    result.deliver(&mut run, &transfer, "gear-test").unwrap();
    assert_eq!(run.home.kingdom_gear, expected);
    assert_eq!(run.home.industry, stock);
    assert!(!transfer.observed(&result).matches(&run));
}

#[test]
fn oathblade_and_pick_deal_their_distinct_damage_in_the_existing_combat_turn() {
    for (id, damage) in [
        ("iron-oathblade", 28),
        ("prospectors-pick", 44),
        ("ember-edge", 28),
    ] {
        let mut run = fight(id);
        run.spawn_at_for_test(EnemyKind::Dummy, 14.0, 14.0);
        let enemy = run.enemies.last_mut().unwrap();
        (enemy.hp, enemy.max_hp, enemy.seen_hp) = (1000, 1000, 1000);
        enemy.age = TELEGRAPH;
        run.knights_turn(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                aim_x: 1,
                ..Default::default()
            },
        )]));
        assert_eq!(1000 - run.enemies.last().unwrap().hp, damage, "{id}");
        assert!(run.players[&1].fire_cooldown > 0);
        assert!(!run.feel.melee.swings.is_empty());
        if id == "prospectors-pick" {
            assert_eq!(run.feel.melee.swings[0].stroke, melee::Stroke::Overhead);
            assert_eq!(melee::Blade::of(&run.players[&1]), melee::Blade::Maul);
        }
    }
}

#[test]
fn moon_staff_fires_three_owned_shards_and_ember_edge_throws_a_returning_blade() {
    let input = BTreeMap::from([(
        1,
        Input {
            fire: true,
            aim_x: 1,
            ..Default::default()
        },
    )]);
    let mut staff = fight("mooncrystal-staff");
    let sortie = staff.knights_turn(&input);
    assert_eq!(sortie.shots.len(), 3);
    assert!(
        sortie
            .shots
            .iter()
            .all(|p| !p.hostile && p.damage == 12 && p.traits.owner == 1)
    );
    assert!(sortie.shots[0].vy != sortie.shots[2].vy);
    let mut blade = fight("ember-edge");
    let sortie = blade.knights_turn(&input);
    assert_eq!(sortie.shots.len(), 1);
    assert_eq!(sortie.shots[0].kind, Shot::Blade);
    assert_eq!(sortie.shots[0].damage, 18);
    assert_eq!(sortie.shots[0].traits.back, 1);
}

#[test]
fn wardstone_heals_and_wards_on_cast_without_refreshing_on_repeat_equip() {
    let mut run = fight("hearth-wardstone");
    run.players.get_mut(&1).unwrap().hp = 30;
    run.knights_turn(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.players[&1].hp, 48);
    assert!(run.players[&1].invulnerable >= HZ * 2 - 1);
    let timer = run.players[&1].spell_cooldowns[0];
    assert_eq!(timer, HZ * 18);
    for _ in 0..4 {
        run.outfit_kingdom_gear();
    }
    assert_eq!(run.players[&1].spell_cooldowns[0], timer);
    assert_eq!(
        run.players[&1]
            .spells
            .iter()
            .flatten()
            .filter(|id| id.as_str() == "hearth-wardstone")
            .count(),
        1
    );
    run.knights_turn(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.players[&1].hp, 48);
}

#[test]
fn aegis_raises_real_guard_wall_and_spends_mana() {
    let mut run = fight("runebound-aegis");
    let mana = run.players[&1].mana;
    run.knights_turn(&BTreeMap::from([(
        1,
        Input {
            dash: true,
            aim_x: 1,
            ..Default::default()
        },
    )]));
    let hero = &run.players[&1];
    assert!(hero.walling);
    assert!(hero.mana < mana);
    assert!(hero.wall_span().is_some());
    assert_eq!(
        hero.guard,
        cards::Guard::Wall(cards::Wall {
            width: 4,
            drain: 25,
            empower: 15
        })
    );
}

#[test]
fn replacing_bound_equipment_cannot_drop_copies_for_another_knight() {
    for (bound, replacement) in [
        ("iron-oathblade", "crossbow"),
        ("runebound-aegis", "kite-shield"),
    ] {
        let mut run = fight(bound);
        let hero = &run.players[&1];
        let (x, y) = (hero.x, hero.y);
        run.dungeon.rooms[run.at].items.clear();
        run.drop_item(replacement.into(), x, y, None);
        run.step_sim_for_test(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                take: true,
                ..Default::default()
            },
        )]));
        assert!(
            !run.dungeon
                .rooms
                .iter()
                .flat_map(|r| &r.items)
                .any(|i| i.card == bound)
        );
        if bound == "iron-oathblade" {
            assert_eq!(run.players[&1].arm.as_deref(), Some(replacement));
        } else {
            assert_eq!(run.players[&1].guard_card.as_deref(), Some(replacement));
        }
        run.outfit_kingdom_gear();
        assert!(equipped(&run.players[&1], item(bound).unwrap()));
        assert!(
            !run.dungeon
                .rooms
                .iter()
                .flat_map(|r| &r.items)
                .any(|i| i.card == bound)
        );
    }
}

#[test]
fn ledger_rejects_unowned_selection_wrong_slot_unknown_items_and_unbounded_revision() {
    let mut ledger = Ledger::default();
    let profile = ledger.profiles.entry("you".into()).or_default();
    profile.arm = Some("iron-oathblade".into());
    assert!(!ledger.valid());
    ledger
        .profiles
        .get_mut("you")
        .unwrap()
        .owned
        .insert("iron-oathblade".into());
    assert!(ledger.valid());
    ledger.profiles.get_mut("you").unwrap().guard = Some("iron-oathblade".into());
    assert!(!ledger.valid());
    ledger.profiles.get_mut("you").unwrap().guard = None;
    ledger.rev = u64::MAX;
    assert!(!ledger.valid());
}
