//! The roads: a shared pool of carts, hubs, loads by weight, jams.
use super::*;
use crate::works_tests::{build, finish, rich, run};

/// A carter's yard beside the camp, joined to its roads, with `carts` carts.
fn yard(k: &mut Kingdom, tile: Tile, carts: u16) -> u32 {
    let c = catalog();
    k.stock.insert("timber".into(), 400);
    let y = build(k, "carters-yard", tile);
    finish(k);
    k.apply(c, Command::Connect(y)).unwrap();
    k.apply(c, Command::Carts { yard: y, carts }).unwrap();
    y
}
/// A woodsman's camp, joined to the camp's roads.
fn woodsman(k: &mut Kingdom, tile: Tile) -> u32 {
    k.stock.insert("timber".into(), 400);
    let f = build(k, "woodsman-camp", tile);
    finish(k);
    k.apply(catalog(), Command::Connect(f)).unwrap();
    f
}

#[test]
fn a_cart_takes_a_tonne_and_the_shared_pool_keeps_the_camp_stocked() {
    let c = catalog();
    // Weights make loads: ten blocks, twenty sacks, hundreds of nails.
    assert_eq!(Kingdom::cart_lot(c, "ashlar"), 10);
    assert_eq!(Kingdom::cart_lot(c, "grain"), 20);
    assert_eq!(Kingdom::cart_lot(c, "flour"), 20);
    assert_eq!(Kingdom::cart_lot(c, "plank"), 40);
    assert_eq!(Kingdom::cart_lot(c, "nails"), 500);
    let mut k = rich();
    let f = woodsman(&mut k, (139, 97));
    let y = yard(&mut k, (143, 99), 2);
    k.stock.insert("timber".into(), 0);
    let before = k.total("timber");
    let mut peak = 0;
    for _ in 0..400 {
        k.advance_second(c);
        for cart in &k.buildings[&y].carts {
            peak = peak.max(Kingdom::cart_load_kilos(c, cart));
        }
    }
    assert!(k.valid(c));
    assert!(k.amount("timber") > 40, "carts brought the camp its timber: {}", k.amount("timber"));
    assert!(peak > 0 && peak <= 1000, "never more than a tonne aboard: {peak}");
    // Nothing is made or lost on the road.
    assert_eq!(k.total("timber"), before + k.produced["timber"]);
    assert!(k.roadwork.moved > 0 && k.roadwork.trips > 0);
    assert!(k.buildings[&f].stats.worked > 100);
}

#[test]
fn one_cart_serves_two_makers_and_more_carts_clear_a_backed_up_bench() {
    let c = catalog();
    let mut k = rich();
    // Two woodsmen at once make more timber than one cart can take away.
    let f1 = woodsman(&mut k, (118, 97));
    let f2 = woodsman(&mut k, (120, 99));
    let y = yard(&mut k, (143, 99), 1);
    // The camp is emptied each minute so it can always take timber: the cart is the limit.
    let measure = |k: &mut Kingdom| {
        let start = k.roadwork.moved;
        let (w1, w2) = (k.buildings[&f1].stats.worked, k.buildings[&f2].stats.worked);
        for s in 0..300 {
            if s % 30 == 0 {
                k.stock.insert("timber".into(), 0);
            }
            k.advance_second(c);
        }
        (k.roadwork.moved - start, k.buildings[&f1].stats.worked - w1 + k.buildings[&f2].stats.worked - w2)
    };
    run(&mut k, 60);
    let (slow, slow_worked) = measure(&mut k);
    assert!(slow > 0, "one cart serves both makers");
    k.stock.insert("fodder".into(), 100_000);
    k.apply(c, Command::Carts { yard: y, carts: 8 }).unwrap();
    run(&mut k, 60);
    let (fast, fast_worked) = measure(&mut k);
    assert!(fast > slow * 2, "eight carts move far more than one: {fast} vs {slow}");
    assert!(fast_worked > slow_worked, "and the makers are held up less: {slow_worked} -> {fast_worked}");
    assert!(k.valid(c));
}

#[test]
fn a_cart_runs_loaded_both_ways_and_carries_mixed_goods() {
    let c = catalog();
    let mut k = rich();
    // A bakery at one end wants flour, water and charcoal; the camp at the other holds
    // them all and wants the bread back. Every trip should carry something.
    let b = build(&mut k, "bakery", (146, 97));
    finish(&mut k);
    k.apply(c, Command::Connect(b)).unwrap();
    let y = yard(&mut k, (141, 98), 1);
    for g in ["flour", "water", "charcoal"] {
        k.stock.insert(g.into(), 200);
    }
    k.stock.insert("bread".into(), 0);
    let mut mixed = false;
    let mut loaded_back = false;
    let mut last_to = 0;
    for _ in 0..600 {
        k.advance_second(c);
        for cart in &k.buildings[&y].carts {
            if cart.load.len() >= 2 {
                mixed = true;
            }
            // Carrying bread to the camp straight after leaving the bakery's inputs.
            if cart.phase == CartPhase::Haul && cart.to == 1 && last_to == b {
                loaded_back = true;
            }
            if cart.phase == CartPhase::Haul {
                last_to = cart.to;
            }
        }
    }
    assert!(mixed, "one cart carried several goods to the same place");
    assert!(loaded_back, "and came back with a load");
    assert!(k.buildings[&b].stats.worked > 0, "the bakery baked");
    assert!(k.valid(c));
}

#[test]
fn a_filling_bench_pushes_to_the_nearest_warehouse_and_works_pull_from_the_nearest_store() {
    let c = catalog();
    let mut k = rich();
    let f = woodsman(&mut k, (139, 97));
    // A warehouse beside the woodsman; the camp is already at its level for timber.
    let w = build(&mut k, "depot", (136, 99));
    finish(&mut k);
    k.apply(c, Command::Connect(w)).unwrap();
    yard(&mut k, (143, 99), 2);
    k.stock.insert("timber".into(), 500);
    run(&mut k, 300);
    let stored = k.buildings[&w].output.get("timber").copied().unwrap_or(0);
    assert!(stored > 10, "the woodsman pushed to the warehouse beside him: {stored}");
    // A sawpit by the warehouse takes its timber from there, not from the far camp.
    let s = build(&mut k, "sawpit", (131, 99));
    finish(&mut k);
    k.apply(c, Command::Connect(s)).unwrap();
    let camp_before = k.amount("timber");
    k.buildings.get_mut(&f).unwrap().paused = true;
    run(&mut k, 200);
    assert!(k.buildings[&s].stats.worked > 0, "the sawpit worked");
    assert!(k.amount("timber") >= camp_before - 20, "it did not drain the camp: {} -> {}", camp_before, k.amount("timber"));
    assert!(k.total("timber") > 0 && k.valid(c));
}

#[test]
fn oxen_without_fodder_stand_still_and_the_yard_says_so() {
    let c = catalog();
    let mut k = rich();
    woodsman(&mut k, (139, 97));
    let y = yard(&mut k, (143, 99), 2);
    k.stock.insert("fodder".into(), 0);
    k.stock.insert("timber".into(), 0);
    run(&mut k, 100);
    assert_eq!(k.report(c).status[&y], Status::Fodder);
    assert_eq!(k.roadwork.moved, 0, "the oxen stand in their stalls");
    k.stock.insert("fodder".into(), 50);
    run(&mut k, 200);
    assert_ne!(k.report(c).status[&y], Status::Fodder);
    assert!(k.roadwork.moved > 0);
}

#[test]
fn a_cut_road_holds_the_carts_and_mending_it_sends_them_on() {
    let c = catalog();
    let mut k = rich();
    let f = woodsman(&mut k, (139, 97));
    yard(&mut k, (143, 99), 2);
    k.stock.insert("timber".into(), 0);
    run(&mut k, 120);
    let moved = k.roadwork.moved;
    assert!(moved > 0);
    // Cut every road tile beside the woodsman.
    let cut: Vec<Tile> = k.roads.iter().copied().filter(|t| crate::economy::neighbours(k.buildings[&f].tile).contains(t)).collect();
    k.stock.insert("timber".into(), 400);
    assert!(!cut.is_empty());
    for t in &cut {
        k.apply(c, Command::Demolish(*t)).unwrap();
    }
    k.stock.insert("timber".into(), 0);
    run(&mut k, 200);
    let during = k.roadwork.moved - moved;
    k.stock.insert("timber".into(), 400);
    for t in &cut {
        k.apply(c, Command::Road(*t)).unwrap();
    }
    k.stock.insert("timber".into(), 0);
    run(&mut k, 200);
    assert!(k.roadwork.moved - moved - during > 10, "carts went on once the road was mended");
    assert!(k.valid(c));
}

#[test]
fn demolishing_a_yard_or_a_maker_keeps_every_unit() {
    let c = catalog();
    let mut k = rich();
    let f = woodsman(&mut k, (139, 97));
    let y = yard(&mut k, (143, 99), 3);
    k.stock.insert("timber".into(), 0);
    run(&mut k, 120);
    let total = k.total("timber");
    let carts = k.stock.get("cart").copied().unwrap_or(0);
    k.apply(c, Command::Demolish(k.buildings[&f].tile)).unwrap();
    run(&mut k, 120);
    assert!(k.total("timber") >= total - 10, "cargo in flight was not lost");
    k.apply(c, Command::Demolish(k.buildings[&y].tile)).unwrap();
    assert!(k.stock.get("cart").copied().unwrap_or(0) >= carts + 3 - 1, "the yard's carts are sold back");
    assert!(k.valid(c));
}

#[test]
fn identical_runs_and_reloaded_saves_agree_with_carts_in_flight() {
    let c = catalog();
    let mut a = rich();
    woodsman(&mut a, (139, 97));
    yard(&mut a, (143, 99), 3);
    let mut mid_road = false;
    for _ in 0..200 {
        run(&mut a, 1);
        mid_road |= a.buildings.values().flat_map(|b| &b.carts).any(|x| !x.route.is_empty());
        if mid_road {
            break;
        }
    }
    assert!(mid_road, "a cart is mid-road");
    // Jams, flows and waits are keyed by tiles and pairs: they must survive a JSON save.
    a.roadwork.crowd.insert((141, 96), 5);
    a.roadwork.flow.insert((3, 1), 100);
    a.roadwork.waiting.insert((1, "timber".into()), 7);
    let saved = serde_json::to_string(&a).unwrap();
    let mut b: Kingdom = serde_json::from_str(&saved).unwrap();
    assert_eq!(a, b);
    run(&mut a, 200);
    run(&mut b, 200);
    assert_eq!(a, b, "a reloaded kingdom replays identically");
    assert!(a.valid(c));
}

#[test]
fn old_saves_with_cart_lines_load_and_the_roads_take_over() {
    let c = catalog();
    let mut k = rich();
    let f = woodsman(&mut k, (139, 97));
    // The kind of line an earlier version laid between a pair of works.
    let before = k.stock.get("cart").copied().unwrap_or(0);
    k.lines.insert(
        99,
        Line {
            kind: LineKind::Cart,
            from: f,
            to: 1,
            path: vec![(139, 97), (140, 96), (140, 95)],
            forward: 50,
            back: 50,
            fleet: vec![Vehicle { phase: Phase::Out, resource: "timber".into(), amount: 4, clock: 10 }; 3],
            stats: LineStats::default(),
            fed: 100,
        },
    );
    k.next_line = 100;
    let timber = k.total("timber");
    k.apply(c, Command::Pause(f)).unwrap();
    assert!(k.lines.is_empty(), "the cart line was retired");
    assert_eq!(k.total("timber"), timber, "its cargo went to the camp");
    assert_eq!(k.stock.get("cart").copied().unwrap_or(0), before + 3, "its carts went back in the stock");
    assert!(k.valid(c));
    // And no new one can be laid.
    let err = k.apply(c, Command::Line { kind: LineKind::Cart, from: f, to: 1, path: vec![], vehicles: 1 }).unwrap_err();
    assert!(err.contains("carter"), "{err}");
}

// ---- the Clerk and the solver read the roads -----------------------------

#[test]
fn the_clerk_says_the_carter_has_no_free_cart_or_that_a_road_is_jammed() {
    let c = catalog();
    let mut k = rich();
    k.stock.insert("timber".into(), 800);
    let (site, yard_id) = {
        k.apply(c, Command::FoundSite { plan: "motte-and-bailey".into(), centre: hillside::KEEP_CENTRE, yard: (150, 84) }).unwrap();
        let s = *k.sites.keys().next().unwrap();
        (s, k.sites[&s].yard)
    };
    finish(&mut k);
    k.apply(c, Command::Connect(yard_id)).unwrap();
    // The ditch is dug: poles are the next thing the wall asks for, and a woodsman has them.
    k.sites.get_mut(&site).unwrap().stage = 3;
    let w = woodsman(&mut k, (139, 97));
    k.apply(c, Command::Recipe { building: w, recipe: "coppice-poles".into() }).unwrap();
    k.buildings.get_mut(&w).unwrap().output.insert("pole".into(), 24);
    let y = yard(&mut k, (143, 99), 1);
    // One cart, and the camp asks for plenty besides: the cart is out, the rest wait.
    for _ in 0..6 {
        k.advance_second(c);
    }
    assert_eq!(k.road_report(c).free, 0, "the one cart is out");
    let said = k.clerk(c, site).sentence;
    assert!(said.contains("no free carts") || said.contains("for a cart"), "{said}");
    // Add carts and the sentence changes; then a jam is named by the way it runs from the camp.
    k.apply(c, Command::Carts { yard: y, carts: 6 }).unwrap();
    k.roadwork.crowd.insert((150, 95), 40);
    k.roadwork.free = 3;
    k.buildings.get_mut(&w).unwrap().output.insert("pole".into(), 24);
    let said = k.clerk(c, site).sentence;
    assert!(said.contains("east road is jammed") || said.contains("scaffold") || said.contains("for a cart"), "{said}");
    assert_eq!(k.road_name((150, 95)), "the east road");
    assert_eq!(k.road_name((140, 70)), "the north road");
}

#[test]
fn carts_crowding_a_tile_slow_each_other_and_the_jam_is_remembered() {
    let run_with = |jam: u16| {
        let mut c = catalog().clone();
        c.works.jam_percent = jam;
        let mut k = rich();
        k.stock.insert("timber".into(), 2000);
        let w = woodsman(&mut k, (139, 97));
        let y = yard(&mut k, (143, 99), 8);
        let _ = w;
        // Eight loaded carts set off together along one street towards the camp.
        let start = *k.roads.iter().min().unwrap();
        let route: Vec<Tile> = (0..12).map(|i| (start.0 + 12 - i, start.1)).collect();
        for t in &route {
            k.roads.insert(*t);
        }
        let b = k.buildings.get_mut(&y).unwrap();
        for cart in &mut b.carts {
            cart.tile = route[route.len() - 1];
            cart.phase = CartPhase::Haul;
            cart.to = 1;
            cart.from = 1;
            cart.load = Stock::from([("timber".into(), 2)]);
            cart.route = route[..route.len() - 1].to_vec();
        }
        for _ in 0..30 {
            k.advance_second(&c);
        }
        let left: usize = k.buildings[&y].carts.iter().map(|x| x.route.len()).sum();
        (left, k.roadwork.crowd.values().map(|v| u32::from(*v)).sum::<u32>())
    };
    let (left_free, crowd_free) = run_with(0);
    let (left_jam, crowd_jam) = run_with(60);
    assert!(left_jam > left_free, "crowded carts get on slower: {left_jam} tiles left vs {left_free}");
    assert_eq!(crowd_free, crowd_jam.min(crowd_free), "an empty-lane road holds nobody up");
    assert!(crowd_jam > 0, "and the jam is remembered where carts were held up");
}

#[test]
fn a_cart_plans_its_way_round_a_remembered_jam_when_there_is_a_second_road() {
    let mut k = rich();
    // Two streets from (100, 100) to (120, 100): the straight one and one two rows south.
    for x in 100..=120 {
        k.roads.insert((x, 100));
        k.roads.insert((x, 102));
    }
    for y in 100..=102 {
        k.roads.insert((100, y));
        k.roads.insert((120, y));
    }
    let net = k.road_net();
    let straight = k.route_net(&net, (100, 100), &[(120, 100)]).unwrap();
    assert!(straight.iter().all(|t| t.1 == 100), "with no jam it goes straight: {straight:?}");
    for x in 101..120 {
        k.roadwork.crowd.insert((x, 100), 90);
    }
    let round = k.route_net(&net, (100, 100), &[(120, 100)]).unwrap();
    assert!(round.iter().any(|t| t.1 == 102), "a jammed street is planned round: {round:?}");
    assert!(round.len() > straight.len(), "the long way costs tiles");
}

#[test]
fn the_solver_counts_pooled_carts_and_yards_from_loads_by_weight() {
    use crate::design::{solve, SolveOptions};
    let c = catalog();
    let req = solve(c, &SolveOptions::new("great-tower", 30.0)).unwrap();
    // A pool, not a vehicle for every pair: tens of carts, not hundreds, and a few yards.
    assert!(req.vehicles >= 10 && req.vehicles < 120, "carts {}", req.vehicles);
    assert!(req.yards >= 1 && req.yards <= req.vehicles.div_ceil(12), "yards {}", req.yards);
    // Heavy goods fill a cart at ten units, so the stone takes more carts than the nails do.
    let carts_of = |g: &str| req.lines.iter().filter(|l| l.good == g).map(|l| l.loads_per_min).sum::<f64>();
    assert!(carts_of("ashlar") > 0.0 && carts_of("nails") < carts_of("ashlar"));
    let json = req.json(c);
    assert_eq!(json["transport"]["carts"].as_u64().unwrap(), u64::from(req.vehicles));
    assert_eq!(json["transport"]["yards"].as_u64().unwrap(), u64::from(req.yards));
    assert!(json["transport"]["flows"].as_array().unwrap().len() > 10);
    // Twice the pace needs more carts.
    let double = solve(c, &SolveOptions::new("great-tower", 60.0)).unwrap();
    assert!(double.vehicles > req.vehicles);
}
