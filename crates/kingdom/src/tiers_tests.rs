//! The tiers pass 3 added to the economy: lifting at the wall (hods and ladders, the treadwheel
//! crane, the great crane), pack mules, aqueducts, wine, the near sand, coppice, the Great Shaft
//! raised on what the quarries ask, a second castle site on the same stock.
use super::*;
use crate::works_tests::{build, cart, finish, hill, lay, rich, run};

/// A camp stocked for the purchase in hand, as a player's is when he has saved for it.
fn stocked(k: &mut Kingdom) {
    for r in catalog().resources.keys() {
        if r != "fodder" {
            k.stock.insert(r.clone(), 400);
        }
    }
}

#[test]
fn lifting_climbs_in_place_from_hods_to_the_great_crane() {
    let c = catalog();
    let (hod, wheel, great) = (&c.plans["hod-gang"], &c.plans["treadwheel-crane"], &c.plans["great-crane"]);
    assert!(hod.crane < wheel.crane && wheel.crane < great.crane, "each tier lifts more than the last");
    assert!(hod.requires.is_empty(), "hods and ladders need no charter: they are the first thing a site has");
    let mut k = rich();
    let id = build(&mut k, "hod-gang", (156, 86));
    finish(&mut k);
    k.apply(c, Command::Upgrade(id)).expect("the hods give way to the treadwheel");
    assert_eq!(k.buildings[&id].plan, "treadwheel-crane");
    finish(&mut k);
    stocked(&mut k);
    k.apply(c, Command::Upgrade(id)).expect("the treadwheel gives way to the great crane");
    assert_eq!(k.buildings[&id].plan, "great-crane");
    assert!(k.apply(c, Command::Upgrade(id)).is_err(), "there is nothing above the great crane");
    // The cost of each tier is paid out of the camp's stock as it is raised.
    assert!(k.valid(c));
}

#[test]
fn a_crane_in_range_quickens_the_masons_by_its_percent() {
    let c = catalog();
    let mut k = rich();
    k.apply(c, Command::FoundSite { plan: "motte-and-bailey".into(), centre: hillside::KEEP_CENTRE, yard: (150, 84) }).unwrap();
    let site = *k.sites.keys().next().unwrap();
    finish(&mut k);
    stocked(&mut k);
    for at in [(149, 87), (149, 89)] {
        let lodge = build(&mut k, "lodge", at);
        k.apply(c, Command::Connect(lodge)).expect("a road to the lodge");
    }
    finish(&mut k);
    // The yard has the stones to pay for, so the lodges keep their crews at the wall.
    let yard = k.sites[&site].yard;
    let wants = k.site_wants(c, &k.sites[&site].clone());
    let y = k.buildings.get_mut(&yard).unwrap();
    for (r, n) in wants {
        y.input.insert(r, n);
    }
    run(&mut k, 60);
    let plain = k.masons(c, site);
    assert!(plain >= 4, "two lodges are crewed: {plain}");
    stocked(&mut k);
    let hod = build(&mut k, "hod-gang", (155, 86));
    k.apply(c, Command::Connect(hod)).expect("a road to the hods");
    finish(&mut k);
    run(&mut k, 60);
    let with_hods = k.masons(c, site);
    let bonus = u32::from(c.plans["hod-gang"].crane);
    assert!(with_hods >= plain * (100 + bonus) / 100, "hods add their {bonus} percent: {plain} -> {with_hods}");
}

#[test]
fn a_mule_train_carries_goods_with_no_road_and_no_track() {
    let c = catalog();
    let mut k = rich();
    let f = build(&mut k, "woodsman-camp", (139, 97));
    let store = build(&mut k, "depot", (127, 98));
    finish(&mut k);
    let spec = &c.lines["mule"];
    assert!(spec.tile_cost.is_empty(), "a trodden way costs nothing to lay");
    let l = lay(&mut k, LineKind::Mule, f, store, 3);
    assert!(k.track_tiles().is_empty(), "a mule trail leaves no track to build round");
    run(&mut k, 300);
    let line = &k.lines[&l];
    assert!(line.stats.moved > 0, "timber came by mule");
    let per_min = line.capacity_per_min(spec);
    assert!(per_min > 0 && per_min < 40, "mules are the small-load tier: {per_min} a minute");
    assert!(c.plans["pasture"].recipes.iter().any(|r| c.recipes[r].outputs.contains_key("mule")), "the pasture rears them");
    assert!(k.valid(c));
}

#[test]
fn an_aqueduct_turns_a_wheel_wherever_it_stands() {
    let c = catalog();
    let mut k = rich();
    // Far from the river: the waterwheel could not stand here, the aqueduct can.
    let id = build(&mut k, "aqueduct", (150, 106));
    finish(&mut k);
    k.apply(c, Command::Connect(id)).ok();
    run(&mut k, 120);
    assert_eq!(k.report(c).status[&id], Status::Working, "{:?}", k.report(c).status[&id]);
    let totals = k.energy_report(c).totals(PowerKind::Mechanical);
    assert_eq!(totals.rated_capacity, 8, "an aqueduct is rated like a waterwheel");
    assert!(c.plans["aqueduct"].siting == Siting::Any, "no riverbank needed");
    assert!(k.valid(c));
}

#[test]
fn the_vineyard_and_the_press_make_wine_for_the_masters() {
    let c = catalog();
    assert!(c.diet.master.iter().any(|d| d == "wine"), "masters drink wine");
    assert!(!c.diet.skilled.iter().any(|d| d == "wine"), "only masters do");
    let mut k = rich();
    stocked(&mut k);
    let v = build(&mut k, "vineyard", (151, 104));
    let p = build(&mut k, "wine-press", (153, 104));
    finish(&mut k);
    for id in [v, p] {
        k.apply(c, Command::Connect(id)).expect("a road to the works");
    }
    // The carts would bring the grapes and the water; here the press is simply stocked.
    k.buildings.get_mut(&p).unwrap().input.extend([("grapes".to_string(), 16), ("water".to_string(), 8)]);
    run(&mut k, 400);
    assert!(k.produced.get("grapes").copied().unwrap_or(0) > 0, "the vines bear");
    assert!(k.produced.get("wine").copied().unwrap_or(0) > 0, "the press runs");
    assert!(k.valid(c));
}

#[test]
fn the_near_sand_lies_a_dozen_tiles_from_the_stockyard_and_the_river_banks_still_serve() {
    let w = World::hillside(7);
    let hollow: Vec<Tile> = (0..176).flat_map(|y| (0..240).map(move |x| (x, y))).filter(|&t| hillside::in_rect(t, hillside::SAND_HOLLOW)).collect();
    assert!(hollow.iter().all(|&t| w.deposit(t) == Some(Deposit::Sand)), "the hollow is all sand");
    assert!(hollow.iter().all(|&t| w.terrain(t).walkable()), "and dry ground");
    let yard = (150, 84);
    let nearest = hollow.iter().map(|t| (t.0 - yard.0).abs() + (t.1 - yard.1).abs()).min().unwrap();
    assert!(nearest <= 14, "sand a dozen tiles from the stockyard, not forty: {nearest}");
    let mut k = hill();
    k.stock.insert("timber".into(), 100);
    k.apply(catalog(), Command::Build { plan: "sand-pit".into(), tile: (155, 80) }).expect("a pit beside the hollow");
    assert!(k.apply(catalog(), Command::Build { plan: "sand-pit".into(), tile: (170, 130) }).is_err(), "a pit needs sand within two tiles");
}

#[test]
fn coppice_cuts_timber_for_ever_where_felling_strips_the_wood() {
    let c = catalog();
    let fell = &c.recipes["fell-wood"];
    let cut = &c.recipes["coppice-timber"];
    let (fp, cp) = (fell.patch.as_ref().unwrap(), cut.patch.as_ref().unwrap());
    assert!(fp.takes > 0 && cp.takes == 0, "felling takes timber from the wood; coppice takes none");
    assert!(cut.outputs["timber"] * 4 >= fell.outputs["timber"] * 3, "a coppice camp cuts at least three quarters of what a felling camp does");
    let mut k = rich();
    let camp = build(&mut k, "woodsman-camp", (139, 97));
    finish(&mut k);
    k.apply(c, Command::Recipe { building: camp, recipe: "coppice-timber".into() }).unwrap();
    let wood = k.buildings[&camp].patch.clone().expect("a camp works a wood");
    let before = k.nature[&wood];
    cart(&mut k, camp, 3);
    k.stock.insert("timber".into(), 0);
    run(&mut k, 600);
    assert!(k.produced.get("timber").copied().unwrap_or(0) > 100, "the camp cut {:?}", k.produced.get("timber"));
    assert!(k.nature[&wood] >= before, "the wood is no smaller: coppice grows back");
}

#[test]
fn mortar_going_off_takes_a_share_of_what_a_store_holds_not_a_unit_from_every_store() {
    let c = catalog();
    let mut k = rich();
    // One sack in each of two stores: it used to cost both of them a sack every shift.
    k.stock.insert("mortar".into(), 7);
    let mill = build(&mut k, "mortar-mill", (141, 99));
    finish(&mut k);
    k.buildings.get_mut(&mill).unwrap().output.insert("mortar".into(), 7);
    let total = k.total("mortar");
    run(&mut k, 600);
    let lost = k.spoiled.get("mortar").copied().unwrap_or(0);
    assert!(lost >= 3, "mortar does go off: {lost}");
    assert!(lost <= 11, "but 14 sacks do not vanish in a minute: {lost} of {total} gone in 600 s");
    assert_eq!(k.total("mortar") + lost, total, "nothing vanishes unrecorded");
    // A full stock goes off in proportion: ninety sacks lose about a sack a shift.
    let mut k = rich();
    k.stock.insert("mortar".into(), 90);
    run(&mut k, 100);
    let lost = k.spoiled.get("mortar").copied().unwrap_or(0);
    assert!((8..=11).contains(&lost), "ninety sacks lose about a sack a shift: {lost} in ten shifts");
    let _ = c;
}

#[test]
fn the_great_shaft_is_raised_on_what_the_quarries_ask() {
    let c = catalog();
    let windlass = &c.plans["shaft-windlass"];
    assert!(windlass.cost.get("cramp").is_none(), "the windlass needs no iron: the founding kit raises it");
    assert!(windlass.cost.keys().all(|g| c.hillside_starter.contains_key(g) || ["beam"].contains(&g.as_str())), "timber, rope, nails and planks: {:?}", windlass.cost);
    assert!(c.plans["shaft-gin"].cost.contains_key("cramp"), "iron is for the horse gin");
    assert!(c.plans["shaft-hand"].shaft < windlass.shaft && windlass.shaft < c.plans["shaft-gin"].shaft);
}

#[test]
fn a_second_castle_site_draws_on_the_same_stock_and_the_same_carts() {
    let c = catalog();
    let mut k = rich();
    stocked(&mut k);
    // The Keep's ground and a second castle on the empty slope east of the model houses.
    k.apply(c, Command::FoundSite { plan: "motte-and-bailey".into(), centre: hillside::KEEP_CENTRE, yard: (150, 84) }).unwrap();
    k.apply(c, Command::FoundSite { plan: "motte-and-bailey".into(), centre: (205, 95), yard: (190, 94) }).expect("a second site on free ground");
    assert_eq!(k.sites.len(), 2, "two castles stand staked at once");
    let ids: Vec<u32> = k.sites.keys().copied().collect();
    assert_ne!(k.sites[&ids[0]].yard, k.sites[&ids[1]].yard, "each has its own stockyard");
    finish(&mut k);
    // Both ditches are dug from the one stock of earth the ditch gangs make: each yard is paid its own.
    for &s in &ids {
        let yard = k.sites[&s].yard;
        k.buildings.get_mut(&yard).unwrap().input.insert("earth".into(), 40);
    }
    for &s in &ids {
        let dug = k.site_dig(c, s, 40).expect("the ditch is dug");
        assert_eq!(dug, 40);
    }
    assert!(ids.iter().all(|s| k.sites[s].dug > 0 || k.sites[s].stage > 0), "both sites moved");
    // The Clerk speaks for each site in turn, and the kingdom stays valid.
    assert!(!k.clerk(c, ids[0]).sentence.is_empty() && !k.clerk(c, ids[1]).sentence.is_empty());
    assert!(k.valid(c));
    // And a ground that is already taken is refused, as it was.
    assert!(k.apply(c, Command::FoundSite { plan: "motte-and-bailey".into(), centre: (205, 95), yard: (191, 80) }).is_err());
}


#[test]
fn an_aqueduct_drives_the_water_hoist_and_the_shaft_lifts_sixty_a_minute() {
    let c = catalog();
    let mut k = rich();
    for (t, _) in c.all_techs() {
        k.researched.insert(t.clone());
    }
    let hoist = *k.buildings.iter().find(|(_, b)| b.plan == "shaft-hand").unwrap().0;
    for want in ["shaft-windlass", "shaft-gin", "shaft-water"] {
        stocked(&mut k);
        k.stock.insert("horse".into(), 10);
        k.apply(c, Command::Upgrade(hoist)).unwrap();
        assert_eq!(k.buildings[&hoist].plan, want);
        finish(&mut k);
    }
    stocked(&mut k);
    // The lift is crewed only while a seam is worked below it.
    k.discover(
        c,
        "mines-1".into(),
        Claim { resource: "limestone".into(), name: "Seam".into(), source: "Mines / floor 1".into(), secured: false, rate: 4, yields: [("limestone".to_string(), 4)].into(), surface: false },
    );
    assert!(k.secure("mines-1"));
    let quarry = build(&mut k, "claim-quarry", (150, 97));
    finish(&mut k);
    k.apply(c, Command::Link { building: quarry, claim: "mines-1".into() }).unwrap();
    k.apply(c, Command::Connect(quarry)).unwrap();
    // The aqueduct stands south of the Keep; its drive runs west along the foot of the wall and
    // up the shaft road through the gate to the hoist (the one way across the Keep's ground).
    let aqueduct = build(&mut k, "aqueduct", (141, 98));
    finish(&mut k);
    k.apply(c, Command::Connect(aqueduct)).expect("a road to the aqueduct");
    // Shafts from the aqueduct's foot to the hoist, a plank a tile.
    let mut drive: Vec<Tile> = (133..141).rev().map(|x| (x, 98)).collect();
    drive.extend((88..98).rev().map(|y| (133, y)));
    drive.extend((134..hillside::HILL_SHAFT.0).map(|x| (x, 88)));
    for tile in drive {
        k.apply(c, Command::PowerLine { kind: PowerKind::Mechanical, tile }).unwrap_or_else(|e| panic!("shaft at {tile:?}: {e}"));
    }
    k.apply(c, Command::EnablePower(hoist)).expect("the hoist is fitted to the drive");
    run(&mut k, 120);
    assert_eq!(k.report(c).status[&aqueduct], Status::Working);
    let rep = k.report(c);
    assert!(k.energy_report(c).powered(PowerKind::Mechanical, hoist), "the hoist is on the drive: {:?} status {:?} crew {:?} enabled {} upgrade {} connected {}", k.energy_report(c).consumer(PowerKind::Mechanical, hoist), rep.status[&hoist], rep.population.assigned.get(&hoist), k.buildings[&hoist].power_enabled, k.buildings[&hoist].power_upgrade, k.connected().contains(&hoist));
    assert_eq!(k.report(c).shaft, 60, "a water hoist on the drive lifts three times its twenty");
    // Cut the drive and it lifts what it can by hand: nothing the aqueduct made.
    k.apply(c, Command::RemovePower { kind: PowerKind::Mechanical, tile: (133, 92) }).unwrap();
    run(&mut k, 10);
    assert!(!k.energy_report(c).powered(PowerKind::Mechanical, hoist));
    assert!(k.valid(c));
}
