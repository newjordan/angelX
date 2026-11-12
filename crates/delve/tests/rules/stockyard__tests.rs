use super::*;
use crate::rules::stockyard::{Stockyard, Tier};

fn founded() -> Run {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    r.home.industry.start(catalog());
    r
}
fn stock(r: &mut Run, goods: &[(&str, u32)]) {
    for (g, n) in goods {
        *r.home.industry.stock.entry((*g).into()).or_default() += n;
    }
}

#[test]
fn the_shaft_lifts_more_at_every_tier_and_the_yard_holds_more() {
    let mut last = (0, 0);
    for tier in Tier::ALL {
        assert!(tier.throughput() > last.0 && tier.capacity() > last.1, "{tier:?}");
        last = (tier.throughput(), tier.capacity());
        assert!(!tier.name().is_empty() && !tier.says().is_empty());
        for r in tier.cost().keys() {
            assert!(catalog().resources.contains_key(r), "{r}");
        }
        if let Some(p) = tier.needs() {
            assert!(catalog().plans.contains_key(p), "{p}");
        }
    }
    assert!(Tier::Hand.cost().is_empty());
    assert_eq!(Tier::LeyLift.next(), None);
    // The great tower is eleven thousand stones: the top tier lifts what it
    // takes to raise it in a few hours.
    assert!(Tier::LeyLift.throughput() * 180 > 11_266);
}

#[test]
fn the_yard_is_a_real_store_with_a_ceiling() {
    let mut y = Stockyard::default();
    assert!(y.is_empty() && y.valid());
    assert_eq!(y.stack("limestone", 10), 10);
    assert_eq!(y.stack("flint", 10), 6, "sixteen blocks, then it is full");
    assert_eq!((y.held(), y.room()), (16, 0));
    assert_eq!(y.stack("flint", 1), 0);
    assert_eq!(y.stack("moon-cheese", 1), 0, "only stone goes in the yard");
    assert!(y.valid() && !y.is_empty());
    assert_eq!(y.draw("limestone", 4), 4);
    assert_eq!(y.draw("limestone", 40), 6, "no more than is there");
    assert_eq!(y.count("limestone"), 0);
    assert!(!y.stone.contains_key("limestone"), "an empty stack is gone");
    // Past its tier's room it is not valid.
    y.stone.insert("basalt".into(), 99);
    assert!(!y.valid());
}

#[test]
fn raising_the_shaft_costs_what_it_says_and_nothing_less() {
    let mut r = founded();
    r.home.industry.stock.clear();
    let (next, cost, ready) = r.shaft_next().unwrap();
    assert_eq!(next, Tier::Windlass);
    assert_eq!(cost, Tier::Windlass.cost());
    assert!(ready.is_err());
    let err = r.upgrade_shaft().unwrap_err();
    assert!(err.starts_with("need "), "{err}");
    assert_eq!(r.home.stockyard.tier, Tier::Hand, "nothing changed");
    stock(&mut r, &[("plank", 12), ("timber", 8), ("stone", 9)]);
    assert!(r.upgrade_shaft().is_err(), "one stone short");
    assert_eq!(r.home.industry.amount("plank"), 12, "a failed raising takes nothing");
    stock(&mut r, &[("stone", 1), ("plank", 3)]);
    let line = r.upgrade_shaft().unwrap();
    assert!(line.contains("WINDLASS") && line.contains("10 blocks"));
    assert_eq!(r.home.stockyard.tier, Tier::Windlass);
    assert_eq!(r.shaft_throughput(), 10);
    assert_eq!(r.home.industry.amount("plank"), 3);
    assert_eq!(r.home.industry.amount("stone"), 0);
    assert!(r.home.stockyard.rev > 0 && r.home.stockyard.valid() && r.home.industry.valid(catalog()));
    assert!(r.kingdom.stir.level > 0, "raising a shaft is noisy");
}

#[test]
fn the_water_hoist_and_the_ley_lift_stand_on_what_drives_them() {
    let mut r = founded();
    r.home.stockyard.tier = Tier::HorseGin;
    stock(&mut r, &[("plank", 100), ("ingot", 100), ("block", 100), ("mana", 100)]);
    let (next, _, ready) = r.shaft_next().unwrap();
    assert_eq!(next, Tier::WaterHoist);
    let why = ready.unwrap_err();
    assert!(why.contains("waterwheel") || why.contains("wheel"), "{why}");
    assert!(r.upgrade_shaft().is_err());
}

#[test]
fn an_unfounded_kingdom_cannot_pay() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    assert!(r.home.industry.is_empty());
    assert!(r.upgrade_shaft().unwrap_err().contains("not founded"));
}

#[test]
fn the_yard_is_saved_and_an_old_save_without_one_still_loads() {
    let mut r = founded();
    r.home.stockyard.tier = Tier::Windlass;
    r.home.stockyard.stack("basalt", 3);
    r.home.stockyard.bank_gold(250);
    let json = serde_json::to_string(&r.home).unwrap();
    let back: crate::rules::home::Home = serde_json::from_str(&json).unwrap();
    assert_eq!(back.stockyard, r.home.stockyard);
    assert_eq!(back.stockyard.count("basalt"), 3);
    // A realm that never hauled a stone writes no yard at all.
    let bare = serde_json::to_string(&crate::rules::home::Home::default()).unwrap();
    assert!(!bare.contains("stockyard"), "{bare}");
    let old: crate::rules::home::Home = serde_json::from_str(&bare).unwrap();
    assert!(old.stockyard.is_empty());
    // And the whole run, as a guest is sent it.
    let run: Run = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(run.home.stockyard, r.home.stockyard);
    assert!(run.valid_snapshot());
}

#[test]
fn a_newer_yard_wins_a_settlement_and_an_older_one_does_not() {
    let mut r = founded();
    r.home.stockyard.stack("flint", 2);
    let mut realm = r.home.clone();
    realm.stockyard = Stockyard::default();
    let mut fresh = Run::at_home(41, 1, None, Default::default(), Default::default());
    fresh.home = realm.clone();
    fresh.rebuild_home(r.home.clone(), Default::default());
    assert_eq!(fresh.home.stockyard.count("flint"), 2, "the realm's newer yard stands");
    // The run's own is kept when it is the newer one.
    let mut mine = founded();
    mine.home.stockyard.stack("freestone", 2);
    mine.home.stockyard.stack("freestone", 2);
    let older = r.home.clone();
    mine.rebuild_home(older, Default::default());
    assert_eq!(mine.home.stockyard.count("freestone"), 4, "the run's newer yard survives a stale sync");
}

#[test]
fn the_delves_gold_is_the_kingdoms_coin() {
    let mut r = founded();
    assert_eq!(r.home.industry.amount("coin"), 0);
    // The camp stores each good up to a cap; the rest waits in the yard.
    let cap = r.home.industry.report(catalog()).capacity;
    assert!(cap >= 50, "the camp keeps some coin: {cap}");
    // A bank by hand: the same path every delve's gold takes.
    let mut spoils = Spoils::default();
    spoils.add(Spoil::Gold, 120);
    spoils.add(Spoil::Bone, 5);
    r.tally_coin(&spoils);
    assert_eq!(r.home.stockyard.coin, 120, "only gold counts");
    r.deliver_coin();
    let took = 120.min(cap);
    assert_eq!(r.home.industry.amount("coin"), took);
    assert_eq!(r.home.stockyard.coin, 120 - took, "what did not fit waits");
    assert!(r.home.industry.valid(catalog()) && r.home.stockyard.valid());
    // An unfounded kingdom is not started by a gift: the gold waits in the yard.
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.tally_coin(&spoils);
    r.deliver_coin();
    assert!(r.home.industry.is_empty());
    assert_eq!(r.home.stockyard.coin, 120);
    r.home.industry.start(catalog());
    r.deliver_coin();
    assert_eq!(r.home.industry.amount("coin"), took);
    // It never exceeds what the camp can store, however much is banked.
    let mut big = Spoils::default();
    big.add(Spoil::Gold, cap + 500);
    r.tally_coin(&big);
    r.deliver_coin();
    assert_eq!(r.home.industry.amount("coin"), cap);
    assert_eq!(r.home.stockyard.coin, 620, "the rest keeps waiting");
    // Spent coin makes room, and the waiting gold follows it in.
    r.home.industry.spend(catalog(), &crate::kingdom::Stock::from([("coin".to_string(), 30)])).unwrap();
    r.deliver_coin();
    assert_eq!(r.home.industry.amount("coin"), cap);
    assert_eq!(r.home.stockyard.coin, 590);
}

#[test]
fn banking_a_delve_banks_its_gold_as_coin_once() {
    let mut r = founded();
    let id = *r.players.keys().next().unwrap();
    r.players.get_mut(&id).unwrap().carried.add(Spoil::Gold, 75);
    r.bank_all(true, "test");
    assert_eq!(r.home.stockyard.coin, 75);
    assert_eq!(r.bank.iter().map(|h| h.spoils.get(Spoil::Gold)).sum::<u32>(), 75, "the treasury still gets it");
    r.bank_all(true, "again");
    assert_eq!(r.home.stockyard.coin, 75, "nothing carried, nothing banked twice");
}

#[test]
fn the_winch_reads_the_ledger_and_a_second_press_pays() {
    let mut r = founded();
    r.home.industry.stock.clear();
    stock(&mut r, &[("plank", 12), ("timber", 8), ("stone", 10)]);
    let geo = map::geography(&r.kingdom.wishes);
    let (tx, ty) = trinkets::things(&geo)
        .into_iter()
        .find(|(t, _)| *t == trinkets::Thing::Winch)
        .map(|(_, at)| at)
        .expect("a winch stands by the Delve's gate");
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((tx as f32 + 0.5) * TILE_UNITS, (ty as f32 + 0.5) * TILE_UNITS);
    // The first press only reads: the price, and that the camp can pay it.
    assert!(r.use_trinket(1, &geo));
    assert_eq!(r.home.stockyard.tier, Tier::Hand);
    let said = r.found_line().unwrap();
    assert!(said.contains("WINDLASS") && said.contains("PRESS E AGAIN"), "{said}");
    assert_eq!(r.home.industry.amount("plank"), 12, "reading takes nothing");
    // A second press soon after pays.
    r.tick += 20;
    assert!(r.use_trinket(1, &geo));
    assert_eq!(r.home.stockyard.tier, Tier::Windlass);
    assert_eq!(r.home.industry.amount("plank"), 0);
    // Late, or short of goods, it only reads again, and says what is missing.
    r.tick += trinkets::WINCH_ASK + 40;
    assert!(r.use_trinket(1, &geo));
    assert_eq!(r.home.stockyard.tier, Tier::Windlass);
    let said = r.found_line().unwrap();
    assert!(said.contains("HORSE GIN") && said.contains("NEED"), "{said}");
    r.tick += 10;
    assert!(r.use_trinket(1, &geo));
    assert_eq!(r.home.stockyard.tier, Tier::Windlass, "a second press without the goods pays nothing");
}

#[test]
fn the_hillsides_rubble_is_a_poor_share_of_the_yard_and_a_quarry_block_goes_first() {
    let mut y = Stockyard::default();
    // Rubble cuts to a stone like any rough stone, and there is only so much of it.
    assert!(!y.can_cut() && y.take_for_cut().is_none());
    assert_eq!(y.stack("rubble", 20), Stockyard::RUBBLE_MOST, "the hillside fills no more of the yard than its share");
    assert!(!y.wants_rubble() && y.can_cut());
    assert_eq!(y.take_for_cut().as_deref(), Some("rubble"));
    assert!(y.wants_rubble());
    // The Delve's blocks still find room beside a full share of rubble.
    assert_eq!(y.stack("limestone", 3), 3);
    y.draw("limestone", 3);
    y.draw("rubble", 20);
    assert_eq!(y.held(), 0);
    // A block from below goes first, and costs the one block.
    y.stack("rubble", 3);
    y.stack("limestone", 1);
    y.stack("flint", 2);
    assert_eq!(y.take_for_cut().as_deref(), Some("flint"), "the plentiful stone first, so no seam is starved");
    assert_eq!((y.count("flint"), y.count("limestone"), y.count("rubble")), (1, 1, 3));
    assert!(y.valid());
    // Only rough stone stands in the yard.
    assert_eq!(y.stack("iron-ore", 1), 0);
}

#[test]
fn the_outcrops_rubble_and_the_delves_blocks_are_one_store_the_bench_cuts_from() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    // Rubble from the hillside and a block carried up from below stand together.
    assert_eq!(r.stock_rough("rubble", 2), 2);
    r.players.get_mut(&1).expect("a knight").block = Some("limestone".into());
    let (yx, yy) = crate::rules::quarrying::yard_spot();
    r.walk_kingdom(Some(crate::world::wishes::delve_stand()));
    let h = r.players.get_mut(&1).expect("a knight");
    (h.x, h.y) = (yx, yy);
    r.step(&BTreeMap::new());
    assert_eq!(r.home.stockyard.count("limestone"), 1, "the carry set its block down");
    assert_eq!(r.home.stockyard.held(), 3);
    assert!(r.home.stockyard.valid());
    // A block carried up on a knight's back comes through the host's command.
    let mut next = Run::at_home(41, 2, None, r.home.clone(), Default::default());
    next.carry_block(1, "flint").expect("a knight takes it up");
    assert_eq!(next.block_of(1), Some("flint"));
    assert!(next.carry_block(1, "iron-ore").is_err(), "only rough stone is carried");
    // The knight who already has one does not get a second.
    next.carry_block(1, "basalt").expect("a no-op");
    assert_eq!(next.block_of(1), Some("flint"));
    // The stockyard is the same store after the new run begins.
    assert_eq!(next.home.stockyard.held(), 3);
}
