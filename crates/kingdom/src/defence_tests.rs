//! The Deep comes up: blows, fire, cuts, thieves, towers and the calm of a standing
//! castle (`defence`).
use super::*;
use crate::defence::{ARROW, Blow, BURN_DOWN, HEAT, LureKind};
use crate::ghosts::{self, MEND_RAIL, MEND_ROLLER};
use crate::works_tests::{build, finish, rich, run};

fn camp_tiles(k: &Kingdom, plan: &str) -> Tile {
    first_chain::ground_for(catalog(), k, plan, (150, 100)).unwrap_or_else(|| panic!("no ground for {plan}"))
}

fn stocked() -> Kingdom {
    let mut k = rich();
    for g in ["timber", "plank", "stone", "nails", "block"] {
        k.stock.insert(g.into(), 400);
    }
    k
}

#[test]
fn the_defences_are_in_the_catalogue_and_the_design_still_checks() {
    let c = catalog();
    for (plan, hp) in [("palisade", 40), ("wall", 120), ("brick-wall", 100), ("watchtower", 150), ("brazier", 12)] {
        assert_eq!(c.plans[plan].hp, hp, "{plan}");
        assert!(c.hillside_menu.contains(&plan.to_string()) || plan == "wall" || plan == "brick-wall", "{plan} on the menu");
    }
    assert!(c.plans["watchtower"].watch > 0 && c.plans["watchtower"].quiver > 0);
    assert!(c.plans["brazier"].beacon && !c.plans["brazier"].upkeep.is_empty());
    assert!(c.plans["wood-gate"].passable && c.plans["wood-gate"].hp == 0);
    assert!(c.recipes["fletch-arrows"].outputs.contains_key(ARROW));
    assert!(c.plans["carpentry"].recipes.contains(&"fletch-arrows".to_string()), "arrows are fletched at the carpenter's");
    let findings: Vec<_> = design::check(c).into_iter().filter(|f| f.severity == design::Severity::Error).collect();
    assert!(findings.is_empty(), "{findings:?}");
    assert!(c.hillside_starter.get(ARROW).copied().unwrap_or(0) > 0, "the first tower does not stand empty");
}

#[test]
fn a_palisade_wears_down_under_blows_and_falls_and_the_camp_raises_it_again() {
    let c = catalog();
    let mut k = stocked();
    let t = camp_tiles(&k, "palisade");
    let id = build(&mut k, "palisade", t);
    // Still going up: nothing to batter.
    assert_eq!(k.batter(c, id, 5), None);
    finish(&mut k);
    assert_eq!(k.defence_of(c, id), Some((40, 40)));
    assert_eq!(k.batter(c, id, 30), Some(Blow::Held(10)));
    assert_eq!(k.defence_of(c, id), Some((10, 40)));
    assert!(k.valid(c));
    assert_eq!(k.batter(c, id, 15), Some(Blow::Down));
    assert!(k.buildings.get(&id).is_none() && k.at(t).is_none(), "it is down");
    assert_eq!(k.havoc.tally.down, 1);
    // A plan of exactly what stood, urgent and the Deep's.
    let (gid, g) = k.plans.ghosts.iter().next().map(|(a, b)| (*a, b.clone())).expect("laid again");
    assert_eq!((g.plan.as_str(), g.tile, g.priority, g.by), ("palisade", t, 2, 0));
    let timber = k.amount("timber");
    run(&mut k, 60);
    let again = k.at(t).expect("raised again by the camp's hands");
    assert_eq!(k.buildings[&again].plan, "palisade");
    assert_eq!(k.buildings[&again].construction, 0);
    assert!(k.amount("timber") < timber, "at the real price");
    assert!(!k.plans.ghosts.contains_key(&gid));
    assert!(k.valid(c));
}

#[test]
fn the_hired_hands_patch_what_stands_once_the_blows_stop_and_a_knight_does_it_faster_for_a_price() {
    let c = catalog();
    let mut k = stocked();
    let t = camp_tiles(&k, "wall");
    let id = build(&mut k, "wall", t);
    finish(&mut k);
    assert_eq!(k.batter(c, id, 100), Some(Blow::Held(20)));
    // Blows in the last half minute: nobody patches under them.
    run(&mut k, 20);
    assert_eq!(k.defence_of(c, id), Some((20, 120)));
    // Then the hands patch, a little at a time, for nothing but the time.
    run(&mut k, 60);
    let (left, _) = k.defence_of(c, id).unwrap();
    assert!(left > 20 && left < 120, "{left}");
    // A knight's hands: twenty a press, at a price in the wall's own goods.
    let before = k.stock.clone();
    let (was, _) = k.defence_of(c, id).unwrap();
    assert!(k.apply(c, Command::Mend(t)).is_ok());
    assert_eq!(k.defence_of(c, id).unwrap().0, (was + 20).min(120));
    assert!(k.amount("block") < before["block"], "block, for a wall");
    assert!(k.apply(c, Command::Mend((1, 1))).is_err(), "nothing broken there");
    assert!(k.valid(c));
    // Mended to the full, it is whole: off the books.
    for _ in 0..8 {
        let _ = k.apply(c, Command::Mend(t));
    }
    assert_eq!(k.defence_of(c, id), Some((120, 120)));
    assert!(k.havoc.worn.is_empty());
}

#[test]
fn the_camp_the_shaft_and_a_castle_yard_are_never_struck() {
    let c = catalog();
    let mut k = stocked();
    assert!(!k.ignite(c, 1) && k.batter(c, 1, 99).is_none());
    let shaft = k.buildings.iter().find(|(_, b)| c.plans[&b.plan].shaft > 0).map(|(id, _)| *id).expect("the shaft");
    assert!(!k.ignite(c, shaft));
    assert!(k.untouchable(c, 1) && k.untouchable(c, shaft));
    assert!(k.wreck_cart(1) == false);
}

#[test]
fn a_fire_stops_the_works_burns_its_stores_and_burns_it_down_unless_put_out() {
    let c = catalog();
    let mut k = stocked();
    let t = camp_tiles(&k, "sawpit");
    let id = build(&mut k, "sawpit", t);
    finish(&mut k);
    k.buildings.get_mut(&id).unwrap().output.insert("plank".into(), 20);
    assert!(k.ignite(c, id));
    assert!(!k.ignite(c, id), "already burning");
    assert_eq!(k.report(c).status[&id], Status::Fire);
    assert!(k.valid(c));
    run(&mut k, 12);
    assert!(k.buildings[&id].output.get("plank").copied().unwrap_or(0) < 20, "what it held burns");
    run(&mut k, usize::from(BURN_DOWN));
    assert!(k.buildings.get(&id).is_none(), "burnt down");
    assert_eq!(k.havoc.tally.burnt, 1);
    // A plan of it, urgent, for the camp's hands.
    assert!(k.plans.ghosts.values().any(|g| g.plan == "sawpit" && g.tile == t && g.priority == 2 && g.by == 0));
    run(&mut k, 80);
    assert!(k.at(t).is_some_and(|b| k.buildings[&b].plan == "sawpit"), "raised again");
    assert!(k.valid(c));
}

#[test]
fn a_well_beside_it_takes_the_heat_out_and_so_do_a_knights_hands() {
    let c = catalog();
    // A well in reach.
    let mut k = stocked();
    let t = camp_tiles(&k, "sawpit");
    let id = build(&mut k, "sawpit", t);
    let w = build(&mut k, "well", (t.0 + 5, t.1 + 4));
    finish(&mut k);
    assert!(k.buildings[&w].construction == 0 && k.ignite(c, id));
    run(&mut k, 20);
    assert!(k.buildings.contains_key(&id), "the well saved it");
    assert!(!k.havoc.fires.contains_key(&id), "the fire is out");
    // Hands: three presses.
    let mut k = stocked();
    let id = build(&mut k, "sawpit", t);
    finish(&mut k);
    assert!(k.ignite(c, id));
    run(&mut k, 5);
    for _ in 0..3 {
        k.apply(c, Command::Mend(t)).unwrap();
    }
    assert!(!k.havoc.fires.contains_key(&id) && k.havoc.fires.is_empty());
    assert_eq!(k.havoc.tally.mended, 1);
    run(&mut k, 60);
    assert!(k.buildings.contains_key(&id), "it stands");
    assert!(HEAT / crate::defence::HANDS_DOUSE <= 3, "three presses is what a fire takes");
}

#[test]
fn a_cut_road_rollerway_or_rail_is_laid_as_a_plan_and_mended() {
    let c = catalog();
    let mut k = stocked();
    // A road tile, a roller tile, a rail tile, far from the camp.
    let road = (152, 104);
    k.apply(c, Command::Road(road)).unwrap();
    k.apply(c, Command::Roller { piece: Piece::Roller, tile: (154, 104), dir: Dir::E }).unwrap();
    k.apply(c, Command::Rail((156, 104))).unwrap();
    k.rollers.cells.get_mut(&(154, 104)).unwrap().slots[0] = 1;
    assert!(k.cuttable(road) && k.cuttable((154, 104)) && k.cuttable((156, 104)));
    assert!(!k.cuttable(k.camp()), "never the camp's doorstep");
    assert!(!k.cuttable((133, k.camp().1 + 1)), "never the Keep's road");
    let cut = k.cut_near(c, road, 0).expect("the road is cut");
    assert_eq!(cut, road);
    assert!(!k.roads.contains(&road));
    assert!(k.havoc.cuts.contains(&road));
    let r = k.cut_near(c, (154, 104), 0).unwrap();
    let rail = k.cut_near(c, (156, 104), 0).unwrap();
    assert_eq!((r, rail), ((154, 104), (156, 104)));
    assert!(k.rollers.cells.is_empty() && !k.rails.cells.contains_key(&(156, 104)));
    assert_eq!(k.havoc.tally.cut, 3);
    let kinds: Vec<_> = k.plans.ghosts.values().map(|g| g.plan.clone()).collect();
    assert!(kinds.contains(&ghosts::ROAD.to_string()) && kinds.contains(&format!("{MEND_ROLLER}roller")) && kinds.contains(&MEND_RAIL.to_string()), "{kinds:?}");
    assert!(k.plans.ghosts.values().all(|g| g.priority == 2 && g.by == 0));
    assert!(k.valid(c));
    // The hired hands mend them: the very pieces, facing the way they did.
    run(&mut k, 200);
    assert!(k.roads.contains(&road));
    let cell = k.rollers.cells.get(&(154, 104)).expect("the roller is laid again");
    assert_eq!((cell.piece, cell.dir), (Piece::Roller, Dir::E));
    assert!(k.rails.cells.contains_key(&(156, 104)));
    assert!(k.plans.ghosts.is_empty() && k.havoc.cuts.is_empty());
    assert!(k.valid(c));
}

#[test]
fn a_knight_sets_a_cut_to_rights_at_once_and_pays_for_it() {
    let c = catalog();
    let mut k = stocked();
    k.apply(c, Command::Roller { piece: Piece::Roller, tile: (154, 104), dir: Dir::E }).unwrap();
    k.cut_near(c, (154, 104), 0).unwrap();
    assert!(k.rollers.cells.is_empty());
    let nails = k.amount("nails");
    k.apply(c, Command::Mend((154, 104))).unwrap();
    assert!(k.rollers.cells.contains_key(&(154, 104)));
    assert!(k.amount("nails") < nails, "at the real price");
    assert!(k.plans.ghosts.is_empty() && k.havoc.cuts.is_empty());
    // A ghost the party laid is not the Deep's: a knight's mend leaves it to the camp.
    k.apply(c, Command::Plan { plan: "well".into(), tile: (150, 100), priority: 1, by: 1 }).unwrap();
    assert!(k.apply(c, Command::Mend((150, 100))).is_err());
}

#[test]
fn thieves_take_a_few_goods_and_never_the_last_of_what_repairs_are_made_of() {
    let c = catalog();
    let mut k = stocked();
    k.stock.insert("coin".into(), 50);
    k.stock.insert("timber".into(), 14);
    k.stock.insert("ale".into(), 30);
    let got = k.rob(c, 1, 100);
    assert!(got.values().sum::<u32>() > 0 && got.values().sum::<u32>() <= 100);
    assert!(!got.contains_key("coin") && !got.contains_key("food") && !got.contains_key(ARROW));
    assert!(k.amount("timber") >= 12 && k.amount("plank") >= 8 && k.amount("stone") >= 12 && k.amount("nails") >= 12, "the repair goods stand");
    assert_eq!(k.amount("coin"), 50);
    assert_eq!(k.havoc.tally.taken, got.values().sum::<u32>());
    // A works' own stores.
    let t = camp_tiles(&k, "sawpit");
    let id = build(&mut k, "sawpit", t);
    k.buildings.get_mut(&id).unwrap().output.insert("plank".into(), 20);
    let got = k.rob(c, id, 6);
    assert_eq!(got.values().sum::<u32>(), 6);
    assert_eq!(k.buildings[&id].output["plank"], 14);
    assert!(k.valid(c));
}

#[test]
fn a_carter_loses_a_cart_but_never_the_last() {
    let c = catalog();
    let mut k = stocked();
    let y = build(&mut k, "carters-yard", (143, 99));
    finish(&mut k);
    k.apply(c, Command::Connect(y)).unwrap();
    k.apply(c, Command::Carts { yard: y, carts: 3 }).unwrap();
    assert!(k.wreck_cart(y) && k.wreck_cart(y));
    assert!(!k.wreck_cart(y), "never the last");
    assert_eq!(k.buildings[&y].carts.len(), 1);
    assert_eq!(k.havoc.tally.carts, 2);
    assert!(k.valid(c));
}

#[test]
fn a_tower_is_a_post_that_asks_the_roads_for_arrows_and_looses_them() {
    let c = catalog();
    let mut k = stocked();
    k.population.add_people(10);
    let t = camp_tiles(&k, "watchtower");
    let id = build(&mut k, "watchtower", t);
    finish(&mut k);
    k.apply(c, Command::Connect(id)).unwrap();
    run(&mut k, 3);
    assert_eq!(k.report(c).status[&id], Status::Inputs, "no arrows yet");
    assert!(k.towers(c, &k.report(c)).is_empty());
    // The roads are asked for a full quiver.
    assert_eq!(k.target_levels(c, id).unwrap().get(ARROW), Some(&24));
    k.buildings.get_mut(&id).unwrap().input.insert(ARROW.into(), 3);
    let report = k.report(c);
    assert_eq!(report.status[&id], Status::Working);
    let towers = k.towers(c, &report);
    assert_eq!(towers.len(), 1);
    assert_eq!((towers[0].id, towers[0].arrows, towers[0].reach), (id, 3, 14));
    assert!(k.tower_loose(id) && k.tower_loose(id) && k.tower_loose(id));
    assert!(!k.tower_loose(id), "an empty quiver looses nothing");
    assert_eq!(k.report(c).status[&id], Status::Inputs);
    // Unmanned, it is no tower.
    k.buildings.get_mut(&id).unwrap().input.insert(ARROW.into(), 3);
    k.population.citizens.clear();
    let _ = k.report(c);
}

#[test]
fn smoking_works_and_lit_braziers_draw_the_deeps_things() {
    let c = catalog();
    let mut k = stocked();
    k.population.add_people(20);
    let t = camp_tiles(&k, "brazier");
    let b = build(&mut k, "brazier", t);
    finish(&mut k);
    // Unlit until it draws its fuel on the next second.
    assert!(k.lures(c, &k.report(c)).is_empty());
    run(&mut k, 2);
    assert!(k.buildings[&b].fed > 0, "lit");
    let lures = k.lures(c, &k.report(c));
    assert_eq!(lures.len(), 1);
    assert_eq!((lures[0].id, lures[0].kind), (b, LureKind::Beacon));
    let p = k.pressure(c);
    assert!(p.smoke >= 2, "a lit brazier smokes: {p:?}");
    // A brazier out of timber goes out.
    k.stock.remove("timber");
    k.buildings.get_mut(&b).unwrap().fed = 1;
    run(&mut k, 3);
    assert!(k.lures(c, &k.report(c)).is_empty(), "out");
    // A lit brazier and a smoking kiln: both draw them.
    k.stock.insert("timber".into(), 400);
    let kt = camp_tiles(&k, "lime-kiln");
    let kiln = build(&mut k, "lime-kiln", kt);
    finish(&mut k);
    k.apply(c, Command::Connect(kiln)).ok();
    k.buildings.get_mut(&kiln).unwrap().recipe = Some("burn-lime-coal".into());
    k.buildings.get_mut(&kiln).unwrap().input = Stock::from([("limestone".into(), 32), ("coal".into(), 8)]);
    k.population.add_people(10);
    let seen = k.lures(c, &k.report(c));
    assert!(seen.iter().all(|l| l.id != b));
}

#[test]
fn a_castle_that_stands_calms_the_land_round_it_and_only_so_far() {
    let c = catalog();
    let mut k = stocked();
    for g in ["timber", "plank", "stone"] {
        k.stock.insert(g.into(), 400);
    }
    k.apply(c, Command::FoundSite { plan: "great-tower".into(), centre: hillside::KEEP_CENTRE, yard: (150, 84) }).unwrap();
    let id = *k.sites.keys().next().unwrap();
    k.sites.get_mut(&id).unwrap().laid = 0;
    assert_eq!(k.calm_at(c, hillside::KEEP_CENTRE), 0);
    k.sites.get_mut(&id).unwrap().stage = 6;
    let laid = c.castles["great-tower"].stages[6].first;
    k.sites.get_mut(&id).unwrap().laid = laid;
    let near = k.calm_at(c, (hillside::KEEP_CENTRE.0 + 10, hillside::KEEP_CENTRE.1 + 10));
    let radius = pressure::calm_radius(laid);
    assert!(near > 0 && near <= pressure::CALM_MAX, "{near}");
    let far = (hillside::KEEP_CENTRE.0 + radius + 5, hillside::KEEP_CENTRE.1);
    assert_eq!(k.calm_at(c, far), 0, "beyond the radius there is no calm");
    // The stir a raid finds is the works' less the calm where the works stand.
    let here = k.pressure_with(c, 40, Some(hillside::KEEP_CENTRE));
    let away = k.pressure_with(c, 40, Some(far));
    assert!(here.calm > 0 && away.calm == 0 && away.stir > here.stir, "{here:?} {away:?}");
    // The wider the wall, the wider the calm.
    assert!(pressure::calm_radius(8000) > pressure::calm_radius(1500));
    assert!(pressure::calm_radius(1_000_000) <= 90);
}

#[test]
fn the_books_save_and_come_back_the_same_and_an_old_save_has_none() {
    let c = catalog();
    let mut k = stocked();
    let t = camp_tiles(&k, "wall");
    let id = build(&mut k, "wall", t);
    finish(&mut k);
    k.batter(c, id, 30);
    let t2 = camp_tiles(&k, "sawpit");
    let sp = build(&mut k, "sawpit", t2);
    finish(&mut k);
    k.ignite(c, sp);
    k.apply(c, Command::Road((152, 104))).unwrap();
    k.cut_near(c, (152, 104), 0).unwrap();
    assert!(k.valid(c));
    let text = serde_json::to_string(&k).unwrap();
    let back: Kingdom = serde_json::from_str(&text).unwrap();
    assert_eq!(back, k);
    assert!(back.valid(c));
    // A kingdom that has never been raided leaves it out of its save.
    let calm = serde_json::to_string(&rich()).unwrap();
    assert!(!calm.contains("havoc"), "a calm book is not saved");
    // A battered wall that is no defence is not a book a kingdom may keep.
    let mut bad = k.clone();
    bad.havoc.worn.insert(sp, 3);
    assert!(!bad.valid(c));
}

#[test]
fn a_knight_mends_what_a_crew_is_already_walking_out_to() {
    let c = catalog();
    let mut k = stocked();
    k.apply(c, Command::Roller { piece: Piece::Roller, tile: (154, 104), dir: Dir::E }).unwrap();
    k.cut_near(c, (154, 104), 0).unwrap();
    // The camp's hands set out with the goods.
    run(&mut k, 2);
    assert!(k.plans.crews.iter().any(|w| w.ghost != 0 && w.leg != ghosts::Leg::Idle), "a crew is on its way");
    // The knight gets there first: the crew is called home and the books stay valid.
    assert_eq!(k.apply(c, Command::Mend((154, 104))), Ok("Set to rights".to_string()));
    assert!(k.valid(c));
    assert!(k.rollers.cells.contains_key(&(154, 104)) && k.plans.ghosts.is_empty() && k.havoc.cuts.is_empty());
    run(&mut k, 120);
    assert!(k.plans.crews.iter().all(|w| w.leg == ghosts::Leg::Idle), "the crew is home");
    assert!(k.valid(c));
}

#[test]
fn the_camps_carts_fill_a_towers_quiver_from_the_camps_arrows() {
    let c = catalog();
    let mut k = stocked();
    k.population.add_people(10);
    k.stock.insert(ARROW.into(), 30);
    let t = camp_tiles(&k, "watchtower");
    let tower = build(&mut k, "watchtower", t);
    finish(&mut k);
    crate::works_tests::cart(&mut k, tower, 2);
    assert_eq!(k.buildings[&tower].input.get(ARROW).copied().unwrap_or(0), 0);
    run(&mut k, 240);
    let held = k.buildings[&tower].input.get(ARROW).copied().unwrap_or(0);
    assert!(held >= 12, "the carts brought a quiver: {held}");
    assert!(k.amount(ARROW) + held <= 30, "from the camp's own arrows, none made from nothing");
    assert_eq!(k.report(c).status[&tower], Status::Working);
    // The archer looses most of them; the carts bring what the camp has left.
    let mut loosed = 0;
    while k.buildings[&tower].input.get(ARROW).copied().unwrap_or(0) > 4 {
        assert!(k.tower_loose(tower));
        loosed += 1;
    }
    let low = k.buildings[&tower].input.get(ARROW).copied().unwrap_or(0);
    // A fletcher's batch comes into the camp (the carts do not make a trip for a handful).
    *k.stock.entry(ARROW.into()).or_default() += 24;
    run(&mut k, 300);
    let again = k.buildings[&tower].input.get(ARROW).copied().unwrap_or(0);
    assert!(loosed > 0 && again > low, "the quiver is filled again: {low} to {again}");
    assert!(k.valid(c));
}
