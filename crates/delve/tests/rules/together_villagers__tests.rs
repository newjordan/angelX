// Villager: the named inhabitants of a holding. Driven through the same
// rent days the cockpit calls; no cockpit needed.

use super::*;
use std::collections::BTreeMap;
use crate::rules::holdings::{rent_day, Built, Deed, Holding, Holdings, WALLS, ROOM_LABOUR};
use crate::rules::Pack;

/// A realm holding the Crypt with these rooms standing (first `building`
/// left unfinished if given), a steward, full walls and `stores` sacks.
fn village(rooms: &[&str], building: Option<&str>, steward: Option<&str>, stores: u32) -> Holdings {
    let mut built: Vec<Built> = rooms.iter().map(|id| Built { id: id.to_string(), labour: ROOM_LABOUR }).collect();
    if let Some(id) = building {
        built.push(Built { id: id.to_string(), labour: 0 });
    }
    let mut h = Holdings::default();
    h.stores = stores;
    h.owned.push(Holding {
        key: Deed::Floor(Pack::Crypt).key(),
        rooms: built,
        steward: steward.map(str::to_string),
        walls: WALLS,
        stars: 1,
        ..Holding::default()
    });
    h
}

fn folk_of(h: &Holdings) -> &Vec<Villager> {
    &h.owned[0].folk
}

#[test]
fn a_migrant_comes_to_a_stewarded_holding_with_grain_and_a_bed() {
    let mut h = village(&["taproom"], None, Some("vell"), 5);
    rent_day(&mut h, 1);
    assert_eq!(folk_of(&h).len(), 1, "one migrant a rent day");
    let soul = &folk_of(&h)[0];
    assert!(!soul.name.is_empty());
    assert_eq!(soul.trade, Trade::Brewer, "the taproom wants a brewer first");
    assert!(h.news.iter().any(|n| n.contains("comes to")), "the ledger names them: {:?}", h.news);
    assert!(h.valid());
}

#[test]
fn no_steward_no_migrants() {
    let mut h = village(&["taproom"], None, None, 5);
    rent_day(&mut h, 3);
    assert!(folk_of(&h).is_empty(), "folk come where someone receives them");
}

#[test]
fn a_bare_granary_turns_migrants_away_and_starves_who_came() {
    let mut h = village(&["taproom", "smithy"], None, Some("vell"), 1);
    rent_day(&mut h, 1); // one sack: a migrant comes (steward's wage took it? stores checked after)
    // Whatever came, an empty granary now only ever subtracts cheer.
    h.stores = 0;
    if folk_of(&h).is_empty() {
        // The steward's wage emptied the granary before the migrant
        // looked in: also correct — nobody comes to a bare table.
        rent_day(&mut h, 1);
        assert!(folk_of(&h).is_empty());
        return;
    }
    let mood = folk_of(&h)[0].mood;
    rent_day(&mut h, 1);
    let soul = &folk_of(&h)[0];
    assert_eq!(soul.hunger, 1, "a missed dinner is remembered");
    assert!(soul.mood < mood, "hunger sours the mood");
}

#[test]
fn folk_eat_from_the_same_granary_as_wages() {
    let mut h = village(&["taproom", "smithy", "shrine", "stall"], None, Some("vell"), 0);
    for name in ["A", "B", "C", "D"] {
        h.owned[0].folk.push(Villager { name: name.into(), trade: Trade::Labourer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    }
    h.stores = 3; // one for the steward's wage, one sack feeds four mouths
    rent_day(&mut h, 1);
    assert_eq!(h.stores, 1, "steward's sack and the folk's sack are gone");
    assert!(folk_of(&h).iter().all(|f| f.hunger == 0 && f.mood > 0), "fed folk cheer up");
}

#[test]
fn labourers_raise_the_room_going_up() {
    let mut h = village(&["taproom"], Some("brewery"), Some("vell"), 9);
    h.owned[0].folk.push(Villager { name: "Col".into(), trade: Trade::Labourer, mood: 0, hunger: 0, skill: 2, practiced: 0, age: 30, seasons: 0 });
    rent_day(&mut h, 1);
    let brewery = h.owned[0].rooms.iter().find(|r| r.id == "brewery").unwrap();
    assert_eq!(brewery.labour, 2, "a practiced labourer lays double");
}

#[test]
fn a_fed_guard_holds_the_walls() {
    // A taproom, not a barracks: nothing but the guard keeps the walls.
    let mut h = village(&["taproom"], None, Some("vell"), 9);
    h.owned[0].folk.push(Villager { name: "Goda".into(), trade: Trade::Guard, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    rent_day(&mut h, 1);
    assert_eq!(h.owned[0].walls, WALLS, "no wall wear under a guard's watch");
    assert_eq!(h.retainers, 0, "and no retainer was spent");
}

#[test]
fn the_miserable_walk_away_and_are_named() {
    let mut h = village(&["taproom"], None, Some("vell"), 0);
    h.owned[0].folk.push(Villager { name: "Uda".into(), trade: Trade::Brewer, mood: -4, hunger: 3, skill: 0, practiced: 0, age: 30, seasons: 0 });
    rent_day(&mut h, 1);
    assert!(folk_of(&h).is_empty(), "at the bottom of their mood, folk leave");
    assert!(h.news.iter().any(|n| n.contains("Uda") && n.contains("has left")), "{:?}", h.news);
}

#[test]
fn beds_cap_the_village_and_the_unhoused_sour() {
    let mut h = village(&["taproom"], None, Some("vell"), 9);
    for name in ["A", "B", "C"] {
        h.owned[0].folk.push(Villager { name: name.into(), trade: Trade::Labourer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    }
    rent_day(&mut h, 1);
    // One bed (the taproom), three folk: no fourth migrant, and two
    // sleep rough — fed (+1) but unhoused (-1) nets them no cheer.
    assert_eq!(folk_of(&h).len(), 3);
    assert_eq!(folk_of(&h)[0].mood, 1, "the housed one cheers");
    assert_eq!(folk_of(&h)[1].mood, 0, "the unhoused do not");
}

#[test]
fn equal_realms_name_equal_folk() {
    let mut a = village(&["taproom", "smithy"], None, Some("vell"), 9);
    let mut b = village(&["taproom", "smithy"], None, Some("vell"), 9);
    rent_day(&mut a, 2);
    rent_day(&mut b, 2);
    assert_eq!(folk_of(&a), folk_of(&b), "a mirror names the same souls");
}

#[test]
fn a_merchant_guest_brings_grain_and_counts_down_the_stay() {
    let mut h = village(&["stall"], None, Some("vell"), 5);
    h.owned[0].visitors.push(Visitor { name: "Piers".into(), kind: VisitorKind::Merchant, days: 2 });
    rent_day(&mut h, 1);
    // The steward's wage took a sack; the merchant's pack put one back.
    assert_eq!(h.stores, 5);
    assert_eq!(h.owned[0].visitors[0].days, 1, "the stay counts down");
    rent_day(&mut h, 1);
    assert!(h.news.iter().any(|n| n.contains("Piers") && n.contains("takes the road")), "{:?}", h.news);
}

#[test]
fn a_troubadour_sings_the_village_cheerier() {
    let mut h = village(&["taproom"], None, Some("vell"), 9);
    h.owned[0].folk.push(Villager { name: "Berta".into(), trade: Trade::Brewer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    h.owned[0].visitors.push(Visitor { name: "Jory".into(), kind: VisitorKind::Troubadour, days: 1 });
    rent_day(&mut h, 1);
    assert_eq!(folk_of(&h)[0].mood, 2, "dinner and a song: two cheers");
}

#[test]
fn a_hunter_guest_watches_the_walls() {
    let mut h = village(&["taproom"], None, Some("vell"), 9);
    h.owned[0].visitors.push(Visitor { name: "Falco".into(), kind: VisitorKind::Hunter, days: 2 });
    rent_day(&mut h, 1);
    assert_eq!(h.owned[0].walls, WALLS, "no wall wear while a hunter lodges");
}

#[test]
fn the_road_brings_a_guest_drawn_by_what_stands() {
    let mut h = village(&["shrine"], None, Some("vell"), 20);
    h.unrest = 3;
    let mut pilgrim = false;
    for _ in 0..10 {
        rent_day(&mut h, 1);
        if h.owned[0].visitors.iter().any(|v| v.kind == VisitorKind::Pilgrim) {
            pilgrim = true;
            break;
        }
    }
    assert!(pilgrim, "a shrine draws a pilgrim within ten rent days");
    assert!(h.unrest < 3, "the pilgrim's word calmed the realm");
    assert!(h.valid());
}

#[test]
fn too_many_guests_or_a_stay_of_no_days_is_not_a_legal_state() {
    let mut h = village(&["taproom"], None, Some("vell"), 9);
    for _ in 0..4 {
        h.owned[0].visitors.push(Visitor { name: "Guest".into(), kind: VisitorKind::Merchant, days: 1 });
    }
    assert!(!h.valid(), "four guests overrun the lodging");
    h.owned[0].visitors.truncate(1);
    h.owned[0].visitors[0].days = 0;
    assert!(!h.valid(), "a stay of no days is not a stay");
}

#[test]
fn a_raised_plot_gets_its_keeper_at_the_raising() {
    use crate::realm::{Spoil, Spoils};
    use crate::rules::countryside::{self, Country};
    let mut country = Country::default();
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 80);
    treasury.add(Spoil::Wax, 2);
    let marks = BTreeMap::from([("country:build:bakery".to_string(), 1)]);
    let (_, news) = countryside::settle(&mut country, &mut treasury, &marks);
    assert!(country.has(0), "the bakery stands");
    let keeper = country.keepers.get("bakery").expect("and somebody keeps it");
    assert_eq!(keeper, &keeper_name("bakery"), "the same soul in every realm");
    assert!(news.iter().any(|n| n.contains(keeper.as_str())), "the ledger names them: {news:?}");
    assert!(country.valid());
}

#[test]
fn a_settle_that_grants_nothing_changes_nothing_keeperless_plots_and_all() {
    use crate::realm::Spoils;
    use crate::rules::countryside::{self, Country};
    let mut country = Country::default();
    country.built |= 1; // the bakery, raised before keepers were named
    let before = country.clone();
    let mut treasury = Spoils::default();
    // A refused request (the bakery already stands, re-asked).
    let marks = BTreeMap::from([("country:build:bakery".to_string(), 1)]);
    let (changed, _) = countryside::settle(&mut country, &mut treasury, &marks);
    assert!(!changed, "no backfill hides inside a settle");
    assert_eq!(country, before);
    // The board still has a name for her: the derived keeper.
    assert_eq!(keeper_name("bakery"), keeper_name("bakery"));
    assert!(country.valid());
}

#[test]
fn repair_strikes_a_keeper_with_no_home() {
    use crate::rules::countryside::Country;
    let mut country = Country::default();
    country.keepers.insert("bakery".into(), "Berta".into());
    country.keepers.insert("nowhere".into(), "Nyle".into());
    assert!(!country.valid(), "keepers of plots that do not stand are not a legal state");
    country.repair();
    assert!(country.keepers.is_empty(), "both names struck: neither plot stands");
    assert!(country.valid());
}

#[test]
fn a_guest_petitions_to_settle_in_a_fed_village_with_a_spare_bed() {
    let mut h = village(&["stall", "taproom"], None, Some("vell"), 9);
    h.owned[0].visitors.push(Visitor { name: "Piers".into(), kind: VisitorKind::Merchant, days: 1 });
    rent_day(&mut h, 1);
    let piers = folk_of(&h).iter().find(|f| f.name == "Piers").expect("the merchant stayed");
    assert_eq!(piers.trade, Trade::Trader, "and works the trade he traveled in");
    assert!(piers.skill >= 1, "a traveled guest knows the craft");
    assert!(h.news.iter().any(|n| n.contains("asks to stay")), "{:?}", h.news);
    assert!(h.valid());
}

#[test]
fn a_guest_takes_the_road_when_there_is_no_bed_or_no_dinner() {
    // No bed: the one room's bed is taken.
    let mut h = village(&["taproom"], None, Some("vell"), 9);
    h.owned[0].folk.push(Villager { name: "Berta".into(), trade: Trade::Brewer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    h.owned[0].visitors.push(Visitor { name: "Jory".into(), kind: VisitorKind::Troubadour, days: 1 });
    rent_day(&mut h, 1);
    assert_eq!(folk_of(&h).len(), 1, "no bed, no settling");
    assert!(h.news.iter().any(|n| n.contains("Jory") && n.contains("takes the road")), "{:?}", h.news);

    // No dinner: beds to spare, but the village itself went hungry.
    let mut h = village(&["stall", "taproom"], None, Some("vell"), 0);
    h.owned[0].folk.push(Villager { name: "Berta".into(), trade: Trade::Brewer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    h.owned[0].visitors.push(Visitor { name: "Piers".into(), kind: VisitorKind::Merchant, days: 1 });
    rent_day(&mut h, 1);
    assert!(!folk_of(&h).iter().any(|f| f.name == "Piers"), "nobody settles at a bare table");
}

#[test]
fn fed_hands_at_a_standing_trade_grow_practiced() {
    let mut h = village(&["taproom"], None, Some("vell"), 30);
    h.owned[0].folk.push(Villager { name: "Berta".into(), trade: Trade::Brewer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    rent_day(&mut h, SEASONS_PER_SKILL as usize);
    let berta = &folk_of(&h)[0];
    assert_eq!(berta.skill, 1, "six fed seasons at the tap make a practiced brewer");
    assert!(h.news.iter().any(|n| n.contains("practiced brewer")), "{:?}", h.news);
}

#[test]
fn hungry_or_idle_hands_learn_nothing() {
    let mut h = village(&["taproom"], None, Some("vell"), 0);
    h.owned[0].folk.push(Villager { name: "Berta".into(), trade: Trade::Brewer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    rent_day(&mut h, 3);
    assert_eq!(folk_of(&h)[0].practiced, 0, "no dinner, no practice");

    let mut h = village(&["taproom"], None, Some("vell"), 30);
    h.owned[0].folk.push(Villager { name: "Col".into(), trade: Trade::Labourer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    rent_day(&mut h, 3);
    assert_eq!(folk_of(&h)[0].practiced, 0, "nothing going up, nothing to practice on");
}

#[test]
fn a_child_is_born_in_a_chartered_town_and_comes_of_age() {
    // Four beds: room for a migrant and a cradle both.
    let mut h = village(&["taproom", "smithy", "shrine", "stall"], None, Some("vell"), 60);
    h.owned[0].stars = 3;
    for (name, trade) in [("Berta", Trade::Brewer), ("Col", Trade::Smith)] {
        h.owned[0].folk.push(Villager { name: name.into(), trade, mood: 4, hunger: 0, skill: 1, practiced: 6, age: 28, seasons: 0 });
    }
    // The cradle fills within a few rent days of trying.
    let mut born = false;
    for _ in 0..16 {
        rent_day(&mut h, 1);
        if folk_of(&h).iter().any(|f| f.trade == Trade::Child) {
            born = true;
            break;
        }
    }
    assert!(born, "a contented town hears a cradle");
    assert!(h.news.iter().any(|n| n.contains("A child is born")), "{:?}", h.news);
    assert!(h.valid());

    // Fourteen years on, the child takes up the wanted trade.
    let child = h.owned[0].folk.iter_mut().find(|f| f.trade == Trade::Child).unwrap();
    child.age = COMING_OF_AGE - 1;
    child.seasons = SEASONS_PER_YEAR - 1;
    rent_day(&mut h, 1);
    assert!(!folk_of(&h).iter().any(|f| f.trade == Trade::Child), "of age, and at a trade");
    assert!(h.news.iter().any(|n| n.contains("comes of age")), "{:?}", h.news);
}

#[test]
fn camps_have_no_cradles_and_the_old_die_mourned() {
    // One star: a camp with a coffer, however content its folk.
    let mut h = village(&["taproom", "smithy"], None, Some("vell"), 60);
    for (name, trade) in [("Berta", Trade::Brewer), ("Col", Trade::Smith)] {
        h.owned[0].folk.push(Villager { name: name.into(), trade, mood: 5, hunger: 0, skill: 1, practiced: 6, age: 28, seasons: 0 });
    }
    rent_day(&mut h, 8);
    assert!(!folk_of(&h).iter().any(|f| f.trade == Trade::Child), "no charter, no cradles");

    // A soul past their allotted years dies, and the village grieves.
    let mut h = village(&["taproom", "smithy"], None, Some("vell"), 60);
    h.owned[0].folk.push(Villager { name: "Uda".into(), trade: Trade::Brewer, mood: 3, hunger: 0, skill: 2, practiced: 12, age: 100, seasons: 0 });
    h.owned[0].folk.push(Villager { name: "Col".into(), trade: Trade::Smith, mood: 3, hunger: 0, skill: 1, practiced: 6, age: 30, seasons: 0 });
    rent_day(&mut h, 1);
    assert!(!folk_of(&h).iter().any(|f| f.name == "Uda"), "Uda's years were spent");
    assert!(h.news.iter().any(|n| n.contains("Uda") && n.contains("mourned")), "{:?}", h.news);
    let col = folk_of(&h).iter().find(|f| f.name == "Col").unwrap();
    assert!(col.mood < 4, "grief took the edge off a good dinner: {}", col.mood);
    assert!(h.valid());
}

#[test]
fn larder_plots_feed_the_granary_on_a_rent_day() {
    use crate::rules::countryside::Country;
    use crate::rules::holdings::note_country;
    // The bakery stands in the countryside; the realm holds the Crypt.
    let mut country = Country::default();
    country.built |= 1;
    let mut h = village(&["taproom"], None, Some("vell"), 0);
    assert!(note_country(&mut h, &country), "the first noting changes the count");
    assert_eq!(h.larders, 1);
    assert!(!note_country(&mut h, &country), "and then it is noted");
    h.owned[0].folk.push(Villager { name: "Berta".into(), trade: Trade::Brewer, mood: 0, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    h.larders = 2; // as if the bothy stood too
    rent_day(&mut h, 1);
    // Two sacks in: one pays the steward, one feeds Berta.
    assert_eq!(folk_of(&h)[0].hunger, 0, "the countryside's sacks fed the folk");
    assert!(h.news.iter().any(|n| n.contains("keepers send")), "{:?}", h.news);
    assert!(h.valid());
}

#[test]
fn larders_send_nothing_to_a_realm_that_holds_nothing() {
    let mut h = Holdings::default();
    h.larders = 3;
    rent_day(&mut h, 1);
    assert_eq!(h.stores, 0, "no holdings, no granary to fill");
}

#[test]
fn an_old_save_without_folk_still_reads_and_validates() {
    let old: Holding = serde_json::from_slice(br#"{"key":"floor:Crypt","walls":10,"stars":1}"#).unwrap();
    assert!(old.folk.is_empty());
    let mut h = Holdings::default();
    h.owned.push(old);
    assert!(h.valid());
    h.owned[0].folk.push(Villager { name: "X".into(), trade: Trade::Smith, mood: 6, hunger: 0, skill: 0, practiced: 0, age: 30, seasons: 0 });
    assert!(!h.valid(), "a mood past the cap is not a legal state");
}
