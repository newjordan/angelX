//! The game's seams into the sim (`seam`): the Great Shaft's limit, the Keep's
//! site staked from the game's wall, a knight's stone paid from the masons' yard,
//! a knight at the bench, the people at their posts by trade.
use super::*;
use crate::site::stage_needs;
use crate::works_tests::{build, finish, rich, run};

fn claim(k: &mut Kingdom, id: &str, yields: &[(&str, u32)]) {
    assert!(k.discover(
        catalog(),
        id.into(),
        Claim {
            resource: yields[0].0.into(),
            name: "Test seam".into(),
            source: "Mines / floor 2".into(),
            secured: false,
            rate: yields[0].1,
            yields: yields.iter().map(|(r, n)| ((*r).into(), *n)).collect(), surface: false }
    ));
    assert!(k.secure(id));
}

/// A rich hillside with one quarry on a limestone seam, joined to the roads.
fn quarry() -> Kingdom {
    let c = catalog();
    let mut k = rich();
    k.stock.insert("limestone".into(), 0);
    claim(&mut k, "mines-1", &[("limestone", 4), ("iron-ore", 1)]);
    let q = build(&mut k, "claim-quarry", (150, 97));
    finish(&mut k);
    k.apply(c, Command::Link { building: q, claim: "mines-1".into() }).unwrap();
    k.apply(c, Command::Connect(q)).unwrap();
    k
}

fn produced(k: &Kingdom) -> u32 {
    k.produced.get("limestone").copied().unwrap_or(0) + k.produced.get("iron-ore").copied().unwrap_or(0)
}

/// The Great Shaft has one scale, blocks a minute, in the sim's lift plans and
/// the game's stockyard tiers alike: 3, 10, 24, 60, 140.
#[test]
fn the_lifts_and_the_stockyard_tiers_are_one_scale_and_a_batch_always_gets_up_in_the_end() {
    let c = catalog();
    let scale = [("shaft-hand", 3), ("shaft-windlass", 10), ("shaft-gin", 24), ("shaft-water", 60), ("shaft-ley", 140)];
    for (plan, per_minute) in scale {
        let p = &c.plans[plan];
        let gain = p.power.as_ref().map_or(100, |q| u32::from(q.multiplier)) * p.shaft / 100;
        assert_eq!(gain, per_minute, "{plan} lifts {per_minute} blocks a minute, powered if it is");
    }
    let (mut hand, mut windlass, mut held) = (quarry(), quarry(), quarry());
    // A windlass in place of the hand lift: ten a minute.
    windlass.researched.insert("ropemaking".into());
    windlass.stock.insert("nails".into(), 100);
    let hoist = windlass.buildings.iter().find(|(_, x)| x.plan == "shaft-hand").map(|(&i, _)| i).unwrap();
    windlass.apply(c, Command::Upgrade(hoist)).unwrap();
    windlass.buildings.get_mut(&hoist).unwrap().construction = 0;
    // The game may hold a lift lower still.
    held.set_shaft_limit(Some(1));
    for k in [&mut hand, &mut windlass, &mut held] {
        run(k, 600);
    }
    let (h, w, l) = (produced(&hand), produced(&windlass), produced(&held));
    // A batch is five units, so one gets up every hundred seconds at three a minute, not none.
    assert!((20..=40).contains(&h), "three blocks a minute is thirty in ten minutes: {h}");
    assert!(w > h * 2, "a windlass lifts more than the hand: {w} against {h}");
    assert!(l * 2 < h, "the limit bites: {l} against {h}");
    assert!(hand.valid(c) && windlass.valid(c) && held.valid(c));
    // Lifted again, the lift rules.
    held.set_shaft_limit(None);
    let before = produced(&held);
    run(&mut held, 600);
    assert!(produced(&held) - before > l * 2, "{} after the limit is lifted, {l} under it", produced(&held) - before);
}

#[test]
fn the_keep_is_staked_at_the_games_wall_and_found_again_is_the_same_site() {
    let c = catalog();
    let mut k = rich();
    let id = k.found_keep(c, 1_500).unwrap();
    let s = &k.sites[&id];
    assert_eq!((s.plan.as_str(), s.laid, s.dug), ("great-tower", 1_500, 0));
    assert_eq!(c.castles["great-tower"].stages[usize::from(s.stage)].id, "03-keep-lift-2");
    assert!(k.valid(c));
    assert_eq!(k.found_keep(c, 7_000).unwrap(), id, "a second asking finds it");
    assert_eq!(k.sites[&id].laid, 1_500);
    // The wall the game shows moves the site's: to a stage's end, to the end of the castle.
    for (laid, stage) in [(1_992, "04-keep-lift-3"), (c.castles["great-tower"].stones(), "")] {
        k.site_progress(c, id, laid).unwrap();
        assert_eq!(k.sites[&id].laid, laid);
        let s = &k.sites[&id];
        assert_eq!(c.castles["great-tower"].stages.get(usize::from(s.stage)).map_or("", |x| x.id.as_str()), stage);
        assert!(k.valid(c), "{laid}");
    }
    assert!(k.site_progress(c, id, 99_999).is_err());
    // On the valley there is no hillside to raise a castle on.
    let mut valley = Kingdom::default();
    valley.found(c, 7);
    assert!(valley.found_keep(c, 1_500).is_err());
}

/// A kingdom with the Keep staked, its yard stocked for the stage in hand.
fn stocked() -> (Kingdom, u32) {
    let c = catalog();
    let mut k = rich();
    let id = k.found_keep(c, 1_500).unwrap();
    finish(&mut k);
    let yard = k.sites[&id].yard;
    let s = &k.sites[&id];
    let needs = stage_needs(c, &c.castles[&s.plan].stages[usize::from(s.stage)]);
    // A yard holds no more than its store (480 of a good).
    for (r, n) in needs {
        *k.buildings.get_mut(&yard).unwrap().input.entry(r).or_default() += n.min(480);
    }
    for (r, n) in [("pole", 30), ("centring", 8)] {
        k.buildings.get_mut(&yard).unwrap().input.insert(r.into(), n);
    }
    (k, id)
}

#[test]
fn a_knights_stone_pays_the_same_bill_from_the_same_yard_as_the_masons() {
    let c = catalog();
    let (mut masons, id) = stocked();
    let mut knight = masons.clone();
    let yard = masons.sites[&id].yard;
    let before = masons.buildings[&yard].input.clone();
    // Ten stones: the masons' pace and the knight's, the same yard.
    for _ in 0..10 {
        let a = masons.site_allowance(c, id, 100);
        assert!(a.stones >= 1);
        masons.site_set(c, id, 1, 100).unwrap();
        knight.site_hand_set(c, id).unwrap();
    }
    assert_eq!(knight.sites[&id].laid, 1_510);
    assert_eq!(masons.sites[&id].laid, 1_510);
    assert_eq!(knight.buildings[&yard].input, masons.buildings[&yard].input, "the same goods were taken");
    assert_ne!(knight.buildings[&yard].input, before, "and some were");
    // The knight does not spend the masons' saved effort, nor add to it.
    assert_eq!(knight.sites[&id].credit, 0);
    assert!(knight.valid(c));
    // With the yard dry the knight cannot set, and says why.
    let y = knight.buildings.get_mut(&yard).unwrap();
    y.input.insert("ashlar".into(), 0);
    assert!(knight.site_hand_set(c, id).is_err());
    assert_eq!(knight.site_ready(c, id), Hold::Short { resource: "ashlar".into() });
}

#[test]
fn a_knight_at_the_bench_cuts_rock_from_the_camp_into_the_yard() {
    let c = catalog();
    let (mut k, id) = stocked();
    let yard = k.sites[&id].yard;
    k.buildings.get_mut(&yard).unwrap().input.insert("ashlar".into(), 10);
    for r in ["limestone", "freestone", "flint", "basalt", "tufa", "voidstone", "stone"] {
        k.stock.insert(r.into(), 0);
    }
    assert!(k.hand_dress(c, id, "ashlar").is_err(), "no rough stone in the camp");
    k.stock.insert("stone".into(), 3);
    k.stock.insert("limestone".into(), 2);
    let ashlar = k.yard_stock(id, "ashlar");
    // Quarry rock goes first, a block to a stone; the hillside's field stone, three to a stone.
    assert_eq!(k.hand_dress(c, id, "ashlar").unwrap(), "limestone");
    assert_eq!((k.amount("limestone"), k.yard_stock(id, "ashlar")), (1, ashlar + 1));
    assert_eq!(k.hand_dress(c, id, "quoin").unwrap(), "limestone");
    assert_eq!(k.hand_dress(c, id, "ashlar").unwrap(), "stone");
    assert_eq!((k.amount("stone"), k.yard_stock(id, "ashlar")), (0, ashlar + 2));
    assert!(k.hand_dress(c, id, "ashlar").is_err());
    assert!(k.valid(c));
    // A stone cut by hand is a stone the masons may set.
    assert!(k.buildings[&yard].input["ashlar"] > 0);
}

#[test]
fn a_lodge_waiting_on_stone_keeps_its_setters_standing_at_it() {
    let c = catalog();
    let (mut k, yard) = stocked();
    for g in ["timber", "plank", "stone", "block"] {
        k.stock.insert(g.into(), 100);
    }
    let lodge = build(&mut k, "lodge", (150, 90));
    finish(&mut k);
    k.apply(c, Command::Connect(lodge)).unwrap();
    run(&mut k, 30);
    let crew = k.report(c).population.assigned.get(&lodge).copied().unwrap_or(0);
    assert!(crew > 0, "the lodge works while the yard can pay");
    // The yard runs dry: the lodge cannot work, and its setters stand at it, idle, not at home.
    k.buildings.get_mut(&yard).unwrap().input.clear();
    k.stock.clear();
    run(&mut k, 12);
    let r = k.report(c);
    assert_eq!(r.population.assigned.get(&lodge).copied().unwrap_or(0), 0, "no one is working it");
    assert_eq!(r.population.waiting.get(&lodge).copied(), Some(crew), "the same people wait at it");
    assert_eq!(k.staffed_by_trade(c).get("setter").copied(), Some(u32::from(crew)), "still at their posts, by trade");
    let there = r.population.citizens.iter().filter(|p| p.waiting == Some(lodge)).count();
    assert_eq!(there, usize::from(crew));
    for p in r.population.citizens.iter().filter(|p| p.waiting == Some(lodge)) {
        assert_eq!(p.target, k.buildings[&lodge].tile, "they walk to the post and wait there");
        assert!(p.job.is_none(), "free for any post that can use them: they count as idle");
    }
    // The economy is as it was: they are free for a post that can work.
    assert!(r.population.idle >= crew);
}

#[test]
fn the_people_at_their_posts_are_counted_by_trade() {
    let c = catalog();
    // A lodge beside a yard that can pay for stones: its crew are the setters.
    let (mut k, _) = stocked();
    for g in ["timber", "plank", "stone", "block"] {
        k.stock.insert(g.into(), 100);
    }
    let lodge = build(&mut k, "lodge", (150, 90));
    finish(&mut k);
    k.apply(c, Command::Connect(lodge)).unwrap();
    run(&mut k, 30);
    let crew = u32::from(k.report(c).population.assigned.get(&lodge).copied().unwrap_or(0));
    assert!(crew > 0);
    assert_eq!(k.staffed_by_trade(c).get("setter").copied(), Some(crew));
    // A quarry on a seam: its crew are quarrymen, and none while it is paused.
    let mut q = quarry();
    run(&mut q, 30);
    let id = q.buildings.iter().find(|(_, b)| b.plan == "claim-quarry").map(|(&i, _)| i).unwrap();
    let crew = u32::from(q.report(c).population.assigned.get(&id).copied().unwrap_or(0));
    assert!(crew > 0);
    assert_eq!(q.staffed_by_trade(c).get("quarryman").copied(), Some(crew));
    q.buildings.get_mut(&id).unwrap().paused = true;
    run(&mut q, 30);
    assert_eq!(q.staffed_by_trade(c).get("quarryman").copied().unwrap_or(0), 0);
}

/// A fresh hillside with the founding train topped up with building materials
/// and every charter researched: nothing the chain makes is in the camp.
fn fresh() -> Kingdom {
    let mut k = crate::works_tests::hill();
    for (g, n) in [("timber", 600), ("plank", 150), ("stone", 200), ("block", 20), ("coal", 80), ("coin", 300), ("fodder", 100_000)] {
        k.stock.insert(g.into(), n);
    }
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    k
}

/// What the first chain makes: the goods the wall's bill is paid in.
const MADE: [&str; 9] = ["ashlar", "quoin", "voussoir", "rubble", "mortar", "joist", "pole", "centring", "template"];

#[test]
fn a_fresh_hillside_raises_the_first_chain_and_the_masons_set_a_stone_from_goods_it_made() {
    let c = catalog();
    let mut k = fresh();
    // Nothing the chain makes is in the camp: the stone, the lime, the timber work, all of it is to be made.
    for g in MADE.iter().chain(["limestone", "quicklime", "lime-putty", "sand", "beam"].iter()) {
        assert_eq!(k.amount(g), 0, "{g} is made, not given");
    }
    let site = k.found_keep(c, 1_500).unwrap();
    // The Mines' third floor: what the game's geology says that seam yields.
    claim(&mut k, "mines-3", &[("limestone", 3), ("iron-ore", 2), ("galena", 1)]);
    let mut chain = crate::first_chain::raise(c, &mut k, "mines-3").unwrap();
    let mut first = None;
    // The stockyard's hand carry: three units a minute up the Great Shaft.
    k.set_shaft_limit(Some(3));
    for _ in 0..(3 * 3600) {
        k.advance_second(c);
        chain.tend(c, &mut k);
        if k.second % 10 == 0 && k.work_sites(c).iter().any(|&(s, n)| s == site && n > 0) {
            first = Some(k.second);
            break;
        }
    }
    let first = first.expect("a stone is set within three hours, at three units a minute");
    assert!(first > 600, "the lime has to be burned and slaked first: {first}");
    // What the stones were paid in was made by buildings of the chain: stone, mortar, and the scaffold.
    // (The bill's earlier share of a stage the Keep is partway through is paid already.)
    for g in ["ashlar", "mortar", "pole", "centring"] {
        assert!(k.produced.get(g).copied().unwrap_or(0) > 0, "{g} was made (first stone at {first} s)");
    }
    // The legs between the works are carts of the carter's yard, on the roads: a tonne a load.
    let roads = k.road_report(c);
    assert_eq!((roads.carts, roads.yards.len()), (usize::from(crate::first_chain::CARTS), 1), "{roads:?}");
    assert!(k.roadwork.trips >= 20 && k.roadwork.kilos >= 5_000, "{} trips, {} kg by cart", k.roadwork.trips, k.roadwork.kilos);
    eprintln!(
        "first stone at {first} s at three units a minute; made {:?}; {} carts made {} trips, {} kg",
        MADE.iter().map(|g| (*g, k.produced.get(*g).copied().unwrap_or(0))).collect::<Vec<_>>(),
        roads.carts,
        k.roadwork.trips,
        k.roadwork.kilos
    );
    // The stones set stand in the site's count.
    assert!(k.sites[&site].laid > 1_500);
    assert!(k.valid(c));
}

#[test]
fn a_chain_with_no_carts_stands_still_for_the_roads_carry_every_leg() {
    let c = catalog();
    let mut k = fresh();
    let site = k.found_keep(c, 1_500).unwrap();
    claim(&mut k, "mines-3", &[("limestone", 3), ("iron-ore", 2), ("galena", 1)]);
    let mut chain = crate::first_chain::raise(c, &mut k, "mines-3").unwrap();
    // The yard stands, but its carts are never bought.
    chain.carted = true;
    k.set_shaft_limit(Some(3));
    for _ in 0..1_500 {
        k.advance_second(c);
        chain.tend(c, &mut k);
    }
    assert_eq!(k.road_report(c).carts, 0);
    assert_eq!(k.roadwork.trips, 0);
    assert!(k.work_sites(c).iter().all(|&(s, n)| s != site || n == 0), "no stone is set without a cart");
    assert_eq!(k.yard_stock(site, "ashlar") + k.yard_stock(site, "mortar"), 0, "nothing reached the yard");
    assert_eq!(k.sites[&site].laid, 1_500);
}

#[test]
fn cave_surface_sites_are_buildable() {
    let c = catalog();
    let sites = [(147i16, 96), (151, 96), (155, 96), (159, 96)];
    let mut k = rich();
    for site in sites {
        k.stock.insert("timber".into(), 80);
        k.stock.insert("stone".into(), 40);
        k.apply(c, Command::Build { plan: "claim-quarry".into(), tile: site })
            .unwrap_or_else(|why| panic!("{site:?}: {why}"));
    }
}

#[test]
fn a_surface_claim_is_not_shaft_limited_and_an_underground_claim_is() {
    let c = catalog();
    let mut under = quarry();
    under.set_shaft_limit(Some(0));
    run(&mut under, 40);
    assert_eq!(produced(&under), 0, "an underground claim waits on the shaft");
    let mut lifted = quarry();
    run(&mut lifted, 200);
    assert!(produced(&lifted) > 0, "the same claim works once the shaft can lift");

    let mut over = rich();
    assert!(over.discover(
        c,
        "cave-surface".into(),
        Claim {
            resource: "limestone".into(),
            name: "Limestone node 1".into(),
            source: "the caves".into(),
            secured: false,
            rate: 4,
            yields: [("limestone".into(), 4), ("iron-ore".into(), 1)].into(),
            surface: true,
        },
    ));
    assert!(over.secure("cave-surface"));
    let q = build(&mut over, "claim-quarry", (156, 97));
    finish(&mut over);
    over.apply(c, Command::Link { building: q, claim: "cave-surface".into() }).unwrap();
    over.apply(c, Command::Connect(q)).unwrap();
    over.set_shaft_limit(Some(0));
    run(&mut over, 40);
    let made = over.produced.get("limestone").copied().unwrap_or(0);
    assert!(made > 0, "a surface claim does not wait on the shaft, made {made}");
}
