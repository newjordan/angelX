use super::*;
use crate::realm::Spoil;

fn run() -> Run {
    Run::at_home(
        3,
        9,
        Some("Guest"),
        home::Home::default(),
        Spoils::default(),
    )
}
fn prepare(transfer: &HomeTransfer, gold: u32) -> HomeResult {
    let mut home = home::Home::default();
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, gold);
    settle(
        &mut game::activity::Realm {
            home: &mut home,
            treasury: &mut treasury,
            reclaimed: &BTreeMap::new(),
            raids_won: 0,
        },
        transfer,
        "Host",
        false,
    )
    .unwrap()
}
fn sale(run: &mut Run, buyer: u32) {
    run.stall = vec!["potion".into()];
    run.orders.push(home::Order {
        knight: buyer,
        station: home::Station::StallA,
    });
}

#[test]
fn versioned_policy_uses_only_explicit_inputs_and_fences_bank_owned_facts() {
    let mut run = run();
    run.marks.insert("xp".into(), 5);
    let request = HomePolicyRequest {
        context: HomeContext {
            home: run.home.clone(),
            treasury: run.treasury.clone(),
            reclaimed: BTreeMap::from([("Hill".into(), 2)]),
            raids_won: 3,
        },
        transfer: HomeTransfer::capture_and_reserve(&mut run).unwrap(),
        host: "Host".into(),
        unsaved: false,
    };
    let before = serde_json::to_value(&request).unwrap();
    let result = request.evaluate().unwrap();
    assert_eq!(serde_json::to_value(&request).unwrap(), before);
    request.validate_result(&result).unwrap();
    let mut bad = result.clone();
    bad.raids_won += 1;
    assert!(request.validate_result(&bad).is_err());
    bad = result.clone();
    bad.reclaimed.clear();
    assert!(request.validate_result(&bad).is_err());
    bad = result;
    bad.purchases.push(999);
    assert!(request.validate_result(&bad).is_err());
    let mut bad = request.clone();
    bad.host = "x".repeat(1025);
    assert!(bad.evaluate().is_err());
    let mut value = before;
    value["unknown"] = true.into();
    assert!(serde_json::from_value::<HomePolicyRequest>(value).is_err());
}

#[test]
fn a_stall_reservation_survives_serialization_and_simultaneous_orders_pay_only_once() {
    let mut run = run();
    sale(&mut run, 1);
    run.orders.push(home::Order {
        knight: 2,
        station: home::Station::StallA,
    });
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    assert_eq!(transfer.reserved, [0]);
    assert_eq!(run.stall, [""]);
    let transfer: HomeTransfer =
        serde_json::from_value(serde_json::to_value(transfer).unwrap()).unwrap();
    let result = prepare(&transfer, 100);
    assert_eq!(result.sales, [(0, 1)]);
    assert_eq!(
        result.treasury.get(Spoil::Gold),
        100 - transfer.stall_prices[0]
    );
    let before = run.room().items.len();
    let delivery = result.deliver(&mut run, &transfer, "session-1").unwrap();
    assert!(delivery.refund.is_empty());
    assert_eq!(run.room().items.len(), before + 1);
    assert_eq!(run.stall, [""]);
}

#[test]
fn leaving_buyers_receive_an_identified_refund_and_reserved_stock_is_restored() {
    let mut run = run();
    sale(&mut run, 2);
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let result = prepare(&transfer, 100);
    run.leave(2);
    let before = run.room().items.len();
    let delivery = result.deliver(&mut run, &transfer, "session-2").unwrap();
    assert_eq!(delivery.refunded_sales, [0]);
    assert_eq!(delivery.refund.get(Spoil::Gold), transfer.stall_prices[0]);
    assert_eq!(run.room().items.len(), before);
    assert_eq!(run.stall, ["potion"]);
}

#[test]
fn a_paid_sale_keeps_its_captured_card_when_the_live_book_changes() {
    let mut run = run();
    sale(&mut run, 1);
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let original = transfer.stall_cards[0].clone().unwrap();
    let mut edited = original.clone();
    edited.name = "Changed after purchase".into();
    assert!(run.book.insert(edited));
    let result = prepare(&transfer, 100);
    let delivery = result.deliver(&mut run, &transfer, "session-3").unwrap();
    assert!(delivery.refund.is_empty());
    let held = run.room().items.last().unwrap();
    assert_eq!(held.card, "paid-session-3-stock-0");
    assert_eq!(run.book.get(&held.card).unwrap().name, original.name);
    assert_eq!(
        run.book.get("potion").unwrap().name,
        "Changed after purchase"
    );
}

#[test]
fn a_full_room_refunds_the_sale_without_discarding_reserved_stock() {
    let mut run = run();
    sale(&mut run, 1);
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let result = prepare(&transfer, 100);
    let most = if run.open_map() {
        game::depths::OPEN_ITEMS
    } else {
        game::MAX_ITEMS
    };
    run.dungeon.rooms[run.at].items = (0..most)
        .map(|_| game::loot::Item::lying("potion".into(), 4.0, 4.0, 0))
        .collect();
    let delivery = result.deliver(&mut run, &transfer, "session-4").unwrap();
    assert_eq!(delivery.refund.get(Spoil::Gold), transfer.stall_prices[0]);
    assert_eq!(run.room().items.len(), most);
    assert_eq!(run.stall, ["potion"]);
}

#[test]
fn cart_deliveries_obey_current_capacity_and_refund_the_undeliverable_purchase() {
    let mut run = run();
    let camp = run.merchant_camp().unwrap();
    let offer = camp
        .offers
        .iter()
        .position(|offer| offer.goods != merchant::Goods::Bomb)
        .unwrap();
    run.enter_for_test(camp.room);
    run.players.get_mut(&1).unwrap().hand = vec!["potion".into(); cards::HAND - 1];
    let purchase = merchant::Purchase {
        knight: 1,
        offer,
        shop: merchant::Shop::Peddler,
    };
    run.peddled.extend([purchase, purchase]);
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    assert_eq!(transfer.purchases.len(), 2);
    let result = prepare(&transfer, 1000);
    assert_eq!(result.purchases, [0, 1]);
    let delivery = result.deliver(&mut run, &transfer, "session-5").unwrap();
    assert_eq!(run.players[&1].hand.len(), cards::HAND);
    assert_eq!(delivery.refunded_purchases, [1]);
    assert_eq!(delivery.refund.get(Spoil::Gold), transfer.purchases[1].gold);
}

#[test]
fn combat_progress_is_marked_for_durability_before_its_source_facts_are_acknowledged() {
    let mut run = Run::new(3, 9, None);
    run.marks.insert("xp".into(), 2);
    run.marks.insert("slay:skeleton".into(), 1);
    // Re-pinned 2026-11-02: a fresh delve opens on the wishing room, calm
    // and cleared — mark the transfer from a fight, as combat does.
    run.phase = game::Phase::Fighting;
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    assert!(transfer.fighting);
    let result = prepare(&transfer, 0);
    assert!(result.changed);
    assert!(result.home.knights.values().any(|prowess| prowess.xp == 2));
    assert_eq!(result.home.bestiary.get("skeleton"), Some(&1));
    assert_eq!(
        run.marks, transfer.marks,
        "pending progress stays visible to gameplay"
    );
}

#[test]
fn private_outbox_rejects_unknown_actions_without_mutating_the_existing_world() {
    let mut run = run();
    run.marks.insert("xp".into(), 3);
    let good = HomeOutbox::capture(&run).unwrap();
    for key in [
        "country:unbounded",
        "college:accept:made-up",
        "country:sell:ore:999",
    ] {
        let mut bad = good.clone();
        bad.marks.insert(key.into(), 1);
        assert!(bad.install(&mut run).is_err());
        assert_eq!(HomeOutbox::capture(&run).unwrap(), good);
    }
    let mut bad = good.clone();
    bad.noticed.push("a-new-unknown-feat".into());
    assert!(bad.install(&mut run).is_err());
    assert_eq!(HomeOutbox::capture(&run).unwrap(), good);
}

#[test]
fn repeated_goblin_escapes_remain_distinct_while_an_older_claim_is_pending() {
    use game::host_commands::Journal;
    let mut run = run();
    run.notice("sticky_fingers");
    let mut journal = Journal::new("escape-session".into(), 9);
    let first = journal.claim_home(&mut run).unwrap().unwrap();
    run.notice("sticky_fingers");
    run.notice("sticky_fingers");
    assert_eq!(run.feats.len(), 3);
    let result = prepare(&first.transfer, 0);
    assert_eq!(result.home.goblins, 1);
    journal.complete_home(&mut run, &first, result).unwrap();
    assert_eq!(run.feats, ["sticky_fingers", "sticky_fingers"]);
    let done = journal.home_completion(&first).unwrap().unwrap();
    journal.acknowledge_home(&first, &done).unwrap();
    let second = journal.claim_home(&mut run).unwrap().unwrap();
    let mut home = run.home.clone();
    let mut treasury = run.treasury.clone();
    let result = settle(
        &mut game::activity::Realm {
            home: &mut home,
            treasury: &mut treasury,
            reclaimed: &Default::default(),
            raids_won: 0,
        },
        &second.transfer,
        "Host",
        false,
    )
    .unwrap();
    assert_eq!(result.home.goblins, 3);
    assert!(!result.announced.contains(&"sticky_fingers".to_owned()));
    journal.complete_home(&mut run, &second, result).unwrap();
    assert!(run.feats.is_empty());
}

#[test]
fn squires_home_claim_charges_once_and_preserves_newer_edits() {
    let mut run = run();
    run.home.squire_academy.paid = 100;
    let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let result = prepare(&transfer, 500);
    assert_eq!(result.home.squire_academy, transfer.view.squire_academy);
    assert_eq!(result.treasury.get(Spoil::Gold), 400);
    assert!(transfer.observed(&result).matches(&run));
    run.home.squire_academy.departures += 1;
    assert!(!transfer.observed(&result).matches(&run));
    result.deliver(&mut run, &transfer, "squires-test").unwrap();
    assert_eq!(run.home.squire_academy.departures, 1);
    let next = HomeTransfer::capture_and_reserve(&mut run).unwrap();
    let mut home = result.home.clone();
    let mut treasury = result.treasury.clone();
    let again = settle(&mut game::activity::Realm {
        home: &mut home, treasury: &mut treasury,
        reclaimed: &BTreeMap::new(), raids_won: 0,
    }, &next, "Host", false).unwrap();
    assert_eq!(again.treasury.get(Spoil::Gold), 400);
    assert_eq!(again.home.squire_academy.departures, 1);
}
