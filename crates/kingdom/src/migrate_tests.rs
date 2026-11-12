//! A valley village is brought onto the hillside: every building stands again on ground the
//! hillside suits it to, near where it stood to the camp, joined to the camp's road, with
//! its stock, its charters, its claims and its people.
use super::*;

fn build(k: &mut Kingdom, plan: &str, tile: Tile) -> u32 {
    k.apply(catalog(), Command::Build { plan: plan.into(), tile }).unwrap_or_else(|e| panic!("{plan} at {tile:?}: {e}"));
    k.at(tile).unwrap()
}

/// The village of the valley, the way a save from before the hillside has it.
fn village() -> Kingdom {
    let c = catalog();
    let mut k = Kingdom::default();
    k.start(c);
    k.stock.insert("timber".into(), 200);
    k.stock.insert("plank".into(), 40);
    k.stock.insert("stone".into(), 60);
    for (plan, tile) in [("forester", (8, 8)), ("quarry", (9, 8)), ("farm", (10, 8)), ("sawmill", (12, 8)), ("clay-pit", (13, 8)), ("cottage", (10, 10))] {
        build(&mut k, plan, tile);
    }
    for _ in 0..200 {
        k.advance_second(c);
    }
    k
}

#[test]
fn a_valley_village_is_brought_onto_the_hillside_with_what_it_owns() {
    let c = catalog();
    let old = village();
    assert!(old.is_valley_village() && !old.world.is_hillside());
    let (stock, people, buildings) = (old.stock.clone(), old.population.citizens.len(), old.buildings.len());
    let mut k = old.clone();
    let moved = k.migrate_to_hillside(c, 7).expect("it moves");
    assert!(k.world.is_hillside() && !k.is_valley_village());
    assert!(k.valid(c), "{:?}", moved.left);
    // Everything it held is still held (more besides: the hillside's founding stock).
    for (r, n) in &stock {
        assert!(k.amount(r) >= *n, "{r}: {} of {n}", k.amount(r));
    }
    assert!(k.population.citizens.len() >= people, "the people came");
    // Each work that has a place on the hillside stands on it, near where it stood to the camp.
    assert!(!moved.placed.is_empty());
    assert!(moved.placed.len() + moved.left.len() == buildings - 1, "{} placed, {:?} left, {buildings} had", moved.placed.len(), moved.left);
    for (from, to) in &moved.placed {
        let want = (from.0 + (k.camp().0 - 11), from.1 + (k.camp().1 - 8));
        assert!((to.0 - want.0).abs() + (to.1 - want.1).abs() <= 45, "{from:?} went to {to:?}, far from {want:?}");
        assert!(k.at(*to).is_some());
        assert!(!crate::hillside::in_rect(*to, crate::hillside::keep_rect()), "not on the Keep's ground: {to:?}");
    }
    // And joined to the camp's roads.
    let joined = k.connected();
    for (id, b) in &k.buildings {
        assert!(joined.contains(id), "{} at {:?} is joined to the camp", b.plan, b.tile);
    }
    // It runs: a minute on, nothing breaks and the village still counts its people.
    for _ in 0..60 {
        k.advance_second(c);
    }
    assert!(k.valid(c));
    // A second migration is no migration.
    assert!(k.migrate_to_hillside(c, 7).is_err());
    // It saves and reads back whole.
    let again: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert!(again.valid(c));
}

#[test]
fn a_mine_keeps_its_claim_across_the_move_and_a_work_the_hillside_has_no_place_for_is_said() {
    let c = catalog();
    let mut old = village();
    old.discover(
        c,
        "seam".into(),
        Claim { resource: "iron-ore".into(), name: "Iron seam".into(), source: "Mines / floor 1".into(), secured: false, rate: 2, yields: Default::default(), surface: false },
    );
    old.secure("seam");
    old.stock.insert("plank".into(), 100);
    old.stock.insert("block".into(), 100);
    old.researched = c.all_techs().map(|(k, _)| k.clone()).collect();
    let mine = build(&mut old, "mine", (14, 10));
    old.buildings.get_mut(&mine).unwrap().construction = 0;
    old.apply(c, Command::Link { building: mine, claim: "seam".into() }).unwrap();
    let mut k = old.clone();
    let moved = k.migrate_to_hillside(c, 7).expect("it moves");
    let linked = k.buildings.values().filter(|b| b.claim.as_deref() == Some("seam")).count();
    // The valley's mine is a work of the valley's; if the hillside builds it, it keeps its seam.
    let mine_placed = moved.placed.iter().any(|(from, _)| *from == (14, 10));
    assert_eq!(linked, usize::from(mine_placed), "{moved:?}");
    assert!(k.claims.contains_key("seam"), "the claim stays the village's");
    assert!(k.valid(c));
}

#[test]
fn an_empty_kingdom_and_the_hillside_are_left_alone() {
    let c = catalog();
    let mut empty = Kingdom::default();
    assert!(empty.migrate_to_hillside(c, 7).is_err() && empty.is_empty());
    let mut hill = Kingdom::default();
    hill.found_hillside(c, 7);
    let before = hill.clone();
    assert!(hill.migrate_to_hillside(c, 7).is_err());
    assert_eq!(hill, before);
}
