use super::*;
use crate::rules::{
    home::Home,
    merchant::{Purchase, Shop},
    world,
};

fn yard() -> Run {
    let mut run = Run::at_home(61, 1, None, Home::default(), Spoils::default());
    run.players.get_mut(&1).unwrap().name = "Ada".into();
    run.join_seat(2, "Bryn");
    run.treasury.add(Spoil::Gold, 100_000);
    run.arrive(
        world::room_of(&run.dungeon, RoomKind::SquireBarracks).unwrap(),
        (12.0, 10.0),
    );
    run
}

// The field/driver seat owns the Yard call site. Drive its public hook
// alongside the normal step until that integration is merged. The hook is
// press-latched, so this remains valid when the driver calls it itself.
fn step(run: &mut Run, inputs: &BTreeMap<u32, Input>) {
    run.step(inputs);
    run.tick_squire_company(inputs);
}

fn stand(run: &mut Run, id: u32, (x, y, w, h): (i32, i32, i32, i32)) {
    let hero = run.players.get_mut(&id).unwrap();
    hero.x = (x as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (y as f32 + h as f32 / 2.0) * TILE_UNITS;
}

fn ask(run: &mut Run, id: u32, plate: (i32, i32, i32, i32)) -> Purchase {
    stand(run, id, plate);
    step(run, &BTreeMap::new());
    step(
        run,
        &BTreeMap::from([(
            id,
            Input {
                interact: true,
                ..Input::default()
            },
        )]),
    );
    let p = run.peddled.pop().expect("a payable company order");
    assert_eq!(p.shop, Shop::Squires);
    p
}

fn pay(run: &mut Run, p: Purchase) -> u32 {
    let gold = run.peddle_price(p).expect("still valid");
    let mut cost = Spoils::default();
    cost.add(Spoil::Gold, gold);
    assert!(run.treasury.covers(&cost));
    run.treasury.take(&cost);
    run.peddle(p);
    gold
}

fn hire(run: &mut Run, id: u32, slot: usize) {
    let p = ask(run, id, RECRUIT_PLATES[slot]);
    pay(run, p);
}

#[test]
fn old_save_and_company_round_trip() {
    let old: Home = serde_json::from_str("{}").unwrap();
    assert_eq!(old.squire_company, Company::default());
    let mut run = yard();
    hire(&mut run, 1, 0);
    let saved = serde_json::to_string(&run.home).unwrap();
    let loaded: Home = serde_json::from_str(&saved).unwrap();
    assert_eq!(loaded.squire_company, run.home.squire_company);
    assert!(run.valid_snapshot());
}

#[test]
fn offers_are_deterministic_refresh_only_on_descent_and_cover_five_kinds() {
    let mut company = Company::default();
    let first = company.offers();
    assert_eq!(first, company.offers());
    assert_eq!(first.len(), 2);
    company.delves += 1;
    assert_ne!(first, company.offers());
    company.board = 3;
    assert_eq!(company.offers().len(), 5);
    let mut kinds = Vec::new();
    for delve in 0..100 {
        company.delves = delve;
        for offer in company.offers() {
            assert!(offer.level > 0 && offer.level <= company.level_cap());
            if !kinds.contains(&offer.kind) {
                kinds.push(offer.kind);
            }
        }
    }
    assert_eq!(kinds.len(), 5);
}

#[test]
fn hiring_is_paid_once_and_rosters_belong_to_names_not_seats() {
    let mut run = yard();
    let p = ask(&mut run, 1, RECRUIT_PLATES[0]);
    assert!(
        run.home.squire_company.rosters.is_empty(),
        "asking never grants goods"
    );
    let before = run.treasury.get(Spoil::Gold);
    let gold = pay(&mut run, p);
    assert_eq!(before - run.treasury.get(Spoil::Gold), gold);
    let company = run.home.squire_company.clone();
    assert_eq!(run.peddle_price(p), None);
    run.peddle(p);
    assert_eq!(
        run.home.squire_company, company,
        "replaying payment cannot hire twice"
    );
    hire(&mut run, 2, 0);
    assert_eq!(run.home.squire_company.rosters["Ada"].squires.len(), 1);
    assert_eq!(run.home.squire_company.rosters["Bryn"].squires.len(), 1);
    assert_ne!(
        run.home.squire_company.rosters["Ada"].squires[0].id,
        run.home.squire_company.rosters["Bryn"].squires[0].id
    );
    run.players.get_mut(&2).unwrap().name = "Ada".into();
    stand(&mut run, 2, RECRUIT_PLATES[0]);
    assert_eq!(run.squire_price(Purchase { knight: 2, ..p }), None);
}

#[test]
fn stale_orders_revalidate_location_health_identity_and_company_state() {
    let mut run = yard();
    let p = ask(&mut run, 1, RECRUIT_PLATES[0]);
    let old = run.home.squire_company.clone();
    run.home.squire_company.delves += 1;
    assert_eq!(run.peddle_price(p), None);
    run.peddle(p);
    assert!(run.home.squire_company.rosters.is_empty());
    run.home.squire_company = old;
    run.players.get_mut(&1).unwrap().hp = 0;
    assert_eq!(run.peddle_price(p), None);
    run.players.get_mut(&1).unwrap().hp = 10;
    run.players.get_mut(&1).unwrap().name = "New knight".into();
    assert_eq!(run.peddle_price(p), None);
    run.players.get_mut(&1).unwrap().name = "Ada".into();
    stand(&mut run, 1, SELECT_PLATE);
    assert_eq!(run.peddle_price(p), None);
    stand(&mut run, 1, RECRUIT_PLATES[0]);
    run.dungeon.depth = 1;
    assert_eq!(run.peddle_price(p), None);
    run.dungeon.depth = 0;
    run.arrive(
        world::room_of(&run.dungeon, RoomKind::Market).unwrap(),
        (12.0, 10.0),
    );
    assert_eq!(run.peddle_price(p), None);
}

#[test]
fn cap_claims_refresh_and_hall_discount_are_enforced() {
    let mut run = yard();
    assert_eq!(run.home.squire_company.roster_cap(), 2);
    hire(&mut run, 1, 0);
    hire(&mut run, 1, 1);
    run.home.squire_company.delves += 1;
    stand(&mut run, 1, RECRUIT_PLATES[0]);
    assert!(
        run.company_order(1, 0).is_none(),
        "new stock cannot bypass roster cap"
    );
    let before = run.home.squire_company.hire_price(0).unwrap();
    let p = ask(&mut run, 1, UPGRADE_PLATES[1]);
    pay(&mut run, p);
    assert_eq!(run.home.squire_company.roster_cap(), 3);
    assert!(run.home.squire_company.hire_price(0).unwrap() < before);
    hire(&mut run, 1, 0);
    assert_eq!(run.home.squire_company.rosters["Ada"].squires.len(), 3);
    assert_eq!(
        run.home.squire_company.rosters["Ada"].claimed.len(),
        1,
        "claims stay bounded to this delve"
    );
}

#[test]
fn select_cycles_each_owned_squire_and_none_without_charging_or_repeating_held_e() {
    let mut run = yard();
    hire(&mut run, 1, 0);
    hire(&mut run, 1, 1);
    let ids: Vec<_> = run.home.squire_company.rosters["Ada"]
        .squires
        .iter()
        .map(|s| s.id)
        .collect();
    let gold = run.treasury.clone();
    for expected in [Some(ids[1]), None, Some(ids[0])] {
        stand(&mut run, 1, SELECT_PLATE);
        step(&mut run, &BTreeMap::new());
        let e = BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Input::default()
            },
        )]);
        step(&mut run, &e);
        assert_eq!(run.home.squire_company.rosters["Ada"].active, expected);
        step(&mut run, &e);
        assert_eq!(run.home.squire_company.rosters["Ada"].active, expected);
    }
    assert_eq!(run.treasury, gold);
    assert!(run.peddled.is_empty());
}

#[test]
fn training_cost_cadence_and_yard_level_cap() {
    let mut run = yard();
    hire(&mut run, 1, 0);
    let before = run.home.squire_company.rosters["Ada"].squires[0].level;
    let p = ask(&mut run, 1, TRAIN_PLATE);
    let gold = pay(&mut run, p);
    assert!(gold > 0);
    assert_eq!(run.home.squire_company.rosters["Ada"].squires[0].level, before + 1);
    assert_eq!(run.home.squire_company.train_price("Ada"), None);
    run.peddle(p);
    assert_eq!(run.home.squire_company.rosters["Ada"].squires[0].level, before + 1);
    run.home.squire_company.delves += 1;
    assert_eq!(run.home.squire_company.train_price("Ada"), None);
    let cap = run.home.squire_company.level_cap();
    let p = ask(&mut run, 1, UPGRADE_PLATES[2]);
    pay(&mut run, p);
    assert!(run.home.squire_company.level_cap() > cap);
    run.home.squire_company.delves += 1;
    let p = ask(&mut run, 1, TRAIN_PLATE);
    assert!(pay(&mut run, p) > gold);
    run.home.squire_company.yard = 3;
    run.home.squire_company.delves += 1;
    assert!(run.home.squire_company.train_price("Ada").is_some());
    run.home.squire_company.rosters.get_mut("Ada").unwrap().squires[0].level =
        run.home.squire_company.level_cap();
    assert_eq!(run.home.squire_company.train_price("Ada"), None);
}

#[test]
fn upgrades_are_bounded_and_orders_cannot_be_replayed() {
    let mut run = yard();
    for plate in UPGRADE_PLATES {
        let mut last = 0;
        for _ in 0..3 {
            let p = ask(&mut run, 1, plate);
            let price = pay(&mut run, p);
            assert!(price > last);
            last = price;
            assert_eq!(run.peddle_price(p), None);
        }
        stand(&mut run, 1, plate);
        step(&mut run, &BTreeMap::new());
        step(
            &mut run,
            &BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    ..Input::default()
                },
            )]),
        );
        assert!(run.peddled.is_empty());
    }
    assert_eq!(run.home.squire_company.offers().len(), 5);
    assert_eq!(run.home.squire_company.roster_cap(), 5);
    assert_eq!(run.home.squire_company.level_cap(), 10);
}

#[test]
fn company_plates_are_floor_and_do_not_overlap_existing_yard_activities() {
    let run = yard();
    let plates: Vec<_> = RECRUIT_PLATES
        .into_iter()
        .chain([SELECT_PLATE, TRAIN_PLATE])
        .chain(UPGRADE_PLATES)
        .collect();
    let overlaps = |(x, y, w, h): (i32, i32, i32, i32), (a, b, c, d): (i32, i32, i32, i32)| {
        x < a + c && a < x + w && y < b + d && b < y + h
    };
    for (i, &plate @ (x, y, w, h)) in plates.iter().enumerate() {
        for r in y..y + h {
            for c in x..x + w {
                assert_eq!(run.room().tile(c, r), Tile::Floor);
            }
        }
        for &other in plates
            .iter()
            .skip(i + 1)
            .chain(crate::rules::talents::LECTERNS.iter())
            .chain(crate::rules::yard::STALL_PLATES.iter())
            .chain(std::iter::once(&crate::rules::yard::STALL))
        {
            assert!(!overlaps(plate, other), "{plate:?} overlaps {other:?}");
        }
        for &(c, r) in &crate::rules::yard::QUINTAINS {
            assert!(!(c >= x as f32 && c < (x + w) as f32 && r >= y as f32 && r < (y + h) as f32));
        }
    }
}

#[test]
fn unpaid_and_incapacitated_requests_never_grant_a_squire() {
    let mut run = yard();
    let p = ask(&mut run, 1, RECRUIT_PLATES[0]);
    let company = run.home.squire_company.clone();
    let price = run.peddle_price(p).unwrap();
    run.treasury = Spoils::default();
    let mut cost = Spoils::default();
    cost.add(Spoil::Gold, price);
    assert!(!run.treasury.covers(&cost));
    // The driver refuses payment and does not call peddle.
    assert_eq!(run.home.squire_company, company);
    for condition in 0..3 {
        let hero = run.players.get_mut(&1).unwrap();
        hero.stone = condition == 0;
        hero.privy = u32::from(condition == 1);
        hero.soak = u32::from(condition == 2);
        assert_eq!(run.peddle_price(p), None);
        run.peddle(p);
        assert_eq!(run.home.squire_company, company);
    }
    run.players.remove(&1);
    assert_eq!(run.peddle_price(p), None);
}

#[test]
fn board_upgrades_improve_levels_and_the_frequency_of_rare_roles() {
    let mut company = Company::default();
    let mut rare = [0; 2];
    for delve in 0..200 {
        company.delves = delve;
        company.board = 0;
        let basic = company.offers();
        company.board = 3;
        let better = company.offers();
        for (before, after) in basic.iter().zip(&better) {
            assert!(after.level >= before.level);
            assert_eq!(before.id, after.id, "upgrades do not refresh claimed stock");
            rare[0] += usize::from(matches!(before.kind, Kind::Healer | Kind::Alchemist));
            rare[1] += usize::from(matches!(after.kind, Kind::Healer | Kind::Alchemist));
        }
    }
    assert!(
        rare[1] > rare[0],
        "the board should attract rare roles: {rare:?}"
    );
}

fn press_both(run: &mut Run) {
    step(run, &BTreeMap::new());
    step(
        run,
        &BTreeMap::from([
            (
                1,
                Input {
                    interact: true,
                    ..Input::default()
                },
            ),
            (
                2,
                Input {
                    interact: true,
                    ..Input::default()
                },
            ),
        ]),
    );
}

#[test]
fn independent_knights_can_buy_together_but_conflicting_batches_are_serialized() {
    let mut run = yard();
    stand(&mut run, 1, RECRUIT_PLATES[0]);
    stand(&mut run, 2, RECRUIT_PLATES[0]);
    press_both(&mut run);
    let orders = std::mem::take(&mut run.peddled);
    assert_eq!(orders.len(), 2);
    for p in orders {
        pay(&mut run, p);
    }
    assert_eq!(run.home.squire_company.rosters.len(), 2);

    run.home.squire_company.delves += 1;
    // A board upgrade changes a hire quote. Do not charge both from the
    // driver's pre-priced batch and silently discard the later delivery.
    stand(&mut run, 1, UPGRADE_PLATES[0]);
    stand(&mut run, 2, RECRUIT_PLATES[1]);
    press_both(&mut run);
    assert_eq!(
        run.peddled.len(),
        1,
        "conflicting orders must wait for another press"
    );
    let p = run.peddled.pop().unwrap();
    pay(&mut run, p);

    // An order for a name shared by two seats must not be invalidated by
    // the second seat cycling its active companion in the same batch.
    run.players.get_mut(&2).unwrap().name = "Ada".into();
    stand(&mut run, 1, RECRUIT_PLATES[1]);
    stand(&mut run, 2, SELECT_PLATE);
    let active = run.home.squire_company.rosters["Ada"].active;
    press_both(&mut run);
    assert_eq!(run.home.squire_company.rosters["Ada"].active, active);
    let p = run.peddled.pop().unwrap();
    pay(&mut run, p);
}

#[test]
fn claims_survive_reload_board_upgrade_and_seat_changes() {
    let mut run = yard();
    hire(&mut run, 1, 0);
    let saved = serde_json::to_string(&run.home).unwrap();
    run.home = serde_json::from_str(&saved).unwrap();
    assert_eq!(run.home.squire_company.hire_price(5), None);
    stand(&mut run, 1, RECRUIT_PLATES[0]);
    assert!(run.company_order(1, 0).is_none());
    let p = ask(&mut run, 1, UPGRADE_PLATES[0]);
    pay(&mut run, p);
    stand(&mut run, 1, RECRUIT_PLATES[0]);
    assert!(run.company_order(1, 0).is_none());
    hire(&mut run, 2, 0);
    run.players.get_mut(&1).unwrap().name = "Bryn".into();
    assert_eq!(run.home.squire_company.rosters["Bryn"].squires.len(), 1);
    stand(&mut run, 1, RECRUIT_PLATES[0]);
    assert!(run.company_order(1, 0).is_none());
}

#[test]
fn training_targets_the_selected_id_and_revalidates_selection_and_cadence() {
    let mut run = yard();
    hire(&mut run, 1, 0);
    hire(&mut run, 1, 1);
    let p = ask(&mut run, 1, TRAIN_PLATE);
    run.home.squire_company.select("Ada");
    assert_eq!(run.peddle_price(p), None);
    run.peddle(p);
    assert!(
        run.home.squire_company.rosters["Ada"]
            .squires
            .iter()
            .all(|s| s.level == 1)
    );
    let p = ask(&mut run, 1, TRAIN_PLATE);
    pay(&mut run, p);
    let roster = &run.home.squire_company.rosters["Ada"];
    assert_eq!(roster.squires[0].level, 1);
    assert_eq!(roster.squires[1].level, 2);
    assert_eq!(
        roster.squires[1].trained_on,
        Some(0),
        "delve zero is not untrained"
    );
    run.home.squire_company.select("Ada");
    assert_eq!(
        run.home.squire_company.train_price("Ada"),
        None,
        "no selected squire"
    );
    run.home.squire_company.select("Ada");
    assert!(
        run.home.squire_company.train_price("Ada").is_some(),
        "each companion has its own cadence"
    );
    let roster = run.home.squire_company.rosters.get_mut("Ada").unwrap();
    roster.squires[0].trained_on = Some(10);
    assert_eq!(
        run.home.squire_company.train_price("Ada"),
        None,
        "a future timestamp cannot underflow"
    );
}

#[test]
fn old_partial_squire_save_gets_defaults_and_owned_ids_never_collide() {
    // The company is saved as `squire_company`, beside the yard's `squires`.
    let home: Home = serde_json::from_str(r#"{"squire_company":{"rosters":{"Ada":{"squires":[{"id":40,"name":"Old Pip","kind":"Ninja","level":2}],"active":40}}}}"#).unwrap();
    assert_eq!(home.squire_company.rosters["Ada"].squires[0].trained_on, None);
    assert!(home.squire_company.rosters["Ada"].claimed.is_empty());
    assert_eq!(home.squire_company.next_id, 0);
    let mut run = yard();
    run.home = home;
    hire(&mut run, 1, 0);
    assert_eq!(run.home.squire_company.rosters["Ada"].squires[1].id, 41);
    run.home.squire_company.next_id = u32::MAX;
    stand(&mut run, 2, RECRUIT_PLATES[0]);
    assert!(
        run.company_order(2, 0).is_none(),
        "exhausted IDs must not wrap"
    );
}

#[test]
fn upgrade_quotes_expire_on_descent_and_hire_quotes_on_a_hall_upgrade() {
    let mut run = yard();
    let p = ask(&mut run, 1, UPGRADE_PLATES[0]);
    run.home.squire_company.delves += 1;
    assert_eq!(run.peddle_price(p), None);
    run.peddle(p);
    assert_eq!(run.home.squire_company.board, 0);
    let p = ask(&mut run, 1, RECRUIT_PLATES[0]);
    run.home.squire_company.hall += 1;
    assert_eq!(run.peddle_price(p), None);
    run.peddle(p);
    assert!(run.home.squire_company.rosters.is_empty());
}

#[test]
fn company_actions_do_not_consume_run_rng_or_carried_gold_and_invalid_inputs_are_ignored() {
    let mut run = yard();
    run.players
        .get_mut(&1)
        .unwrap()
        .carried
        .add(Spoil::Gold, 777);
    let carried = run.players[&1].carried.clone();
    let rng = serde_json::to_value(&run.rng).unwrap();
    hire(&mut run, 1, 0);
    assert_eq!(serde_json::to_value(&run.rng).unwrap(), rng);
    assert_eq!(run.players[&1].carried, carried);
    stand(&mut run, 1, RECRUIT_PLATES[1]);
    run.tick_squire_company(&BTreeMap::new());
    run.tick_squire_company(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            move_x: 127,
            ..Input::default()
        },
    )]));
    assert!(run.peddled.is_empty());
    let p = Purchase {
        knight: 1,
        offer: usize::MAX,
        shop: Shop::Squires,
    };
    assert_eq!(run.peddle_price(p), None);
    let p = Purchase {
        knight: 1,
        offer: 15,
        shop: Shop::Squires,
    };
    assert_eq!(run.peddle_price(p), None);
}

#[test]
fn reserved_purchase_delivers_by_captured_name_after_movement_and_seat_reuse() {
    let mut run = yard();
    let p = ask(&mut run, 1, RECRUIT_PLATES[0]);
    let prepared = run.prepare_squire_purchase(p).unwrap();
    let mut company = run.reserve_squire_company().unwrap();
    assert!(run.reserve_squire_company().is_none());
    assert!(run.prepare_squire_purchase(p).is_none());
    let tick = run.tick;
    run.step(&BTreeMap::new());
    assert!(run.tick > tick, "company reservation must not pause the sim");
    // The simulation is free to move, die, rename or replace the seat while
    // the host stages and writes the realm. None changes who is owed goods.
    run.players.get_mut(&1).unwrap().name = "Cy".into();
    run.players.get_mut(&1).unwrap().hp = 0;
    stand(&mut run, 1, SELECT_PLATE);
    assert!(run.peddle_price(p).is_none());
    let cost = prepared.apply(&mut company).unwrap();
    assert!(cost > 0);
    assert_eq!(
        prepared.apply(&mut company),
        None,
        "duplicate is not charged"
    );
    run.complete_squire_company(Some(company));
    assert_eq!(run.home.squire_company.rosters["Ada"].squires.len(), 1);
    assert!(!run.home.squire_company.rosters.contains_key("Cy"));
    assert!(!run.squire_company_reserved);
}

#[test]
fn reservation_defers_selection_new_orders_and_delve_cadence_on_both_outcomes() {
    for success in [false, true] {
        let mut run = yard();
        let p = ask(&mut run, 1, RECRUIT_PLATES[0]);
        pay(&mut run, p);
        // A free selection made before reservation must survive failure.
        stand(&mut run, 1, SELECT_PLATE);
        run.tick_squire_company(&BTreeMap::new());
        let interact = BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]);
        run.tick_squire_company(&interact);
        assert_eq!(run.home.squire_company.rosters["Ada"].active, None);
        let before = run.home.squire_company.clone();
        let mut staged = run.reserve_squire_company().unwrap();
        run.tick_squire_company(&BTreeMap::new());
        run.tick_squire_company(&interact);
        assert_eq!(run.home.squire_company, before, "selection cannot race a save");
        stand(&mut run, 2, RECRUIT_PLATES[0]);
        run.tick_squire_company(&BTreeMap::new());
        run.tick_squire_company(&BTreeMap::from([(
            2,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
        assert!(run.peddled.is_empty(), "new orders cannot race a save");
        // Actual floor transition, not just a hand-edited counter.
        run.descend_for_test();
        assert_eq!(run.home.squire_company, before);
        assert_eq!(run.squire_pending_delves, 1);
        staged.board = 1;
        run.complete_squire_company(success.then_some(staged));
        assert_eq!(run.home.squire_company.board, u8::from(success));
        assert_eq!(run.home.squire_company.delves, before.delves + 1);
        assert!(run.home.squire_company.rosters["Ada"].claimed.is_empty());
        assert_eq!(run.home.squire_company.rosters["Ada"].active, None);
        assert_eq!(run.squire_pending_delves, 0);
        assert!(!run.squire_company_reserved);
        // Completion's deferred cadence must not be lost to the next rebuild.
        let company = run.reserve_squire_company().unwrap();
        assert_eq!(company.delves, before.delves + 1);
        run.complete_squire_company(Some(company));
    }
}

#[test]
fn failed_save_can_retry_the_same_reserved_company_order_without_charging() {
    let mut run = yard();
    let p = ask(&mut run, 1, RECRUIT_PLATES[0]);
    run.peddled.push(p);
    let gold = run.treasury.get(Spoil::Gold);
    let orders = std::mem::take(&mut run.peddled);
    let prepared = run.prepare_squire_purchase(orders[0]).unwrap();
    let mut candidate = run.reserve_squire_company().unwrap();
    let cost = prepared.apply(&mut candidate).unwrap();
    // Failed realm save: candidate/charge are discarded and requests restored.
    run.complete_squire_company(None);
    run.peddled.splice(0..0, orders);
    assert_eq!(run.treasury.get(Spoil::Gold), gold);
    assert!(run.home.squire_company.rosters.is_empty());
    let retry = run.peddled.pop().unwrap();
    let prepared = run.prepare_squire_purchase(retry).unwrap();
    let mut candidate = run.reserve_squire_company().unwrap();
    assert_eq!(prepared.apply(&mut candidate), Some(cost));
    run.complete_squire_company(Some(candidate));
    assert_eq!(run.home.squire_company.rosters["Ada"].squires.len(), 1);
}

#[test]
fn reservation_and_deferred_delves_are_never_checkpoint_state() {
    let mut run = yard();
    run.reserve_squire_company().unwrap();
    run.squire_pending_delves = 2;
    let value = serde_json::to_value(&run).unwrap();
    assert!(value.get("squire_company_reserved").is_none());
    assert!(value.get("squire_pending_delves").is_none());
    let restored: Run = serde_json::from_value(value).unwrap();
    assert!(!restored.squire_company_reserved);
    assert_eq!(restored.squire_pending_delves, 0);
}
