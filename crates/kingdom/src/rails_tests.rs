//! Rails as a network: bays, routes, blocks, switches, jams (`rails`).
use super::*;
use crate::rails::{Act, Stop, WagonState};
use crate::works_tests::build;

fn rich() -> Kingdom {
    let mut k = crate::works_tests::rich();
    for r in catalog().resources.keys() {
        if r != "fodder" {
            k.stock.insert(r.clone(), 600);
        }
    }
    k
}
fn rail(k: &mut Kingdom, t: Tile) {
    k.apply(catalog(), Command::Rail(t)).unwrap_or_else(|e| panic!("rail at {t:?}: {e}"));
}
fn track(k: &mut Kingdom, from: Tile, to: Tile) {
    let (dx, dy) = ((to.0 - from.0).signum(), (to.1 - from.1).signum());
    let mut t = from;
    loop {
        rail(k, t);
        if t == to {
            break;
        }
        t = (t.0 + dx, t.1 + dy);
    }
}
fn depot(k: &mut Kingdom, tile: Tile) -> u32 {
    let id = build(k, "depot", tile);
    k.buildings.get_mut(&id).unwrap().construction = 0;
    id
}
fn bay(k: &mut Kingdom, tile: Tile, name: &str) {
    k.apply(catalog(), Command::Bay { tile, name: Some(name.into()) }).unwrap_or_else(|e| panic!("bay {name}: {e}"));
}
fn route(k: &mut Kingdom, name: &str, from: &str, to: &str, wagons: u16) -> u32 {
    let stops = vec![Stop { bay: from.into(), act: Act::Load, good: None }, Stop { bay: to.into(), act: Act::Unload, good: None }];
    k.apply(catalog(), Command::Route { name: Some(name.into()), stops, wagons }).unwrap_or_else(|e| panic!("route {name}: {e}"));
    *k.rails.routes.iter().find(|(_, r)| r.name == name).map(|(i, _)| i).unwrap()
}
fn plank(k: &Kingdom, id: u32) -> u32 {
    k.buildings[&id].output.get("plank").copied().unwrap_or(0)
}
/// A second at a time, with the blocks checked every second: a tile holds one wagon, or two on plain track
/// going opposite ways.
fn run_checked(k: &mut Kingdom, seconds: usize) {
    for _ in 0..seconds {
        k.advance_second(catalog());
        let mut held: std::collections::BTreeMap<Tile, Vec<Dir>> = Default::default();
        for w in k.rails.wagons.values() {
            held.entry(w.tile).or_default().push(w.face);
        }
        for (t, faces) in held {
            assert!(faces.len() == 1 || (faces.len() == 2 && faces[0] == faces[1].back()), "two wagons on {t:?}: {faces:?}");
        }
    }
    assert!(k.valid(catalog()));
}

/// Two stores a straight track apart, a bay at each end, one wagon between.
fn straight(len: i16) -> (Kingdom, u32, u32, u32) {
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    track(&mut k, (163, 96), (163 + len - 1, 96));
    let b = depot(&mut k, (163 + len, 96));
    bay(&mut k, (163, 96), "Store A");
    bay(&mut k, (163 + len - 1, 96), "Store B");
    let r = route(&mut k, "A to B", "Store A", "Store B", 1);
    (k, a, b, r)
}

#[test]
fn a_wagon_runs_a_route_between_two_bays_and_none_of_the_goods_are_lost() {
    let c = catalog();
    let (mut k, a, b, r) = straight(10);
    assert_eq!(k.rails.bays["Store A"].side, Dir::W, "the bay faces the building it serves");
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 60);
    let total = k.total("plank");
    run_checked(&mut k, 150);
    let moved = plank(&k, b);
    assert!(moved >= 16, "a wagon's round is about half a minute: {moved} planks in 150 seconds");
    assert_eq!(k.total("plank"), total, "the wagon's load is counted");
    assert_eq!(plank(&k, a) + plank(&k, b) + k.railed("plank"), 60);
    let row = k.route_report(c).into_iter().find(|x| x.id == r).unwrap();
    assert_eq!((row.stops, row.wagons), (2, 1));
    assert!(row.per_min > 0 && row.capacity_per_min >= row.per_min, "{row:?}");
    // A second wagon lifts the flow.
    let before = plank(&k, b);
    k.apply(c, Command::Wagons { route: r, wagons: 2 }).unwrap();
    run_checked(&mut k, 100);
    let with_two = plank(&k, b) - before;
    assert!(with_two >= 16, "{with_two}");
}

#[test]
fn bays_take_the_names_of_what_they_serve_and_a_route_needs_bays_that_exist() {
    let c = catalog();
    let mut k = rich();
    let _a = depot(&mut k, (160, 96));
    let _b = depot(&mut k, (160, 100));
    track(&mut k, (163, 96), (165, 96));
    track(&mut k, (163, 100), (165, 100));
    k.apply(c, Command::Bay { tile: (163, 96), name: None }).unwrap();
    k.apply(c, Command::Bay { tile: (163, 100), name: None }).unwrap();
    assert_eq!(k.rails.bays.keys().collect::<Vec<_>>(), ["Warehouse", "Warehouse 2"]);
    assert!(k.apply(c, Command::Bay { tile: (164, 96), name: None }).is_err(), "no building beside it");
    assert!(k.apply(c, Command::Bay { tile: (163, 96), name: Some("Again".into()) }).is_err(), "a bay already");
    let stops = vec![Stop { bay: "Warehouse".into(), act: Act::Load, good: None }, Stop { bay: "Nowhere".into(), act: Act::Unload, good: None }];
    assert!(k.apply(c, Command::Route { name: None, stops, wagons: 1 }).is_err());
    // A refused route costs nothing: the rail between the bays is not joined, so there is no room.
    let horses = k.amount("horse");
    let stops = vec![Stop { bay: "Warehouse".into(), act: Act::Load, good: None }, Stop { bay: "Warehouse 2".into(), act: Act::Unload, good: None }];
    k.apply(c, Command::Route { name: None, stops, wagons: 99 }).unwrap_err();
    assert_eq!(k.amount("horse"), horses);
}

#[test]
fn rail_is_laid_tile_by_tile_and_a_wagon_blocks_it_being_taken_up() {
    let c = catalog();
    let (mut k, _, _, _) = straight(8);
    let wagon_at = k.rails.wagons.values().next().unwrap().tile;
    assert!(k.apply(c, Command::Demolish(wagon_at)).is_err(), "a wagon stands on it");
    // Taking the bay's tile up takes the bay and its route.
    k.rails.wagons.clear();
    k.apply(c, Command::Demolish((163, 96))).unwrap();
    assert!(k.rails.bays.get("Store A").is_none() && k.rails.routes.is_empty(), "the route has one stop left, so it goes");
    assert!(k.valid(c));
    // The old tramway command is gone.
    let err = k.apply(c, Command::Line { kind: LineKind::Tram, from: 1, to: 2, path: Vec::new(), vehicles: 1 }).unwrap_err();
    assert!(err.contains("rails are laid tile by tile"), "{err}");
}

#[test]
fn two_wagons_meeting_at_a_switch_take_turns_and_never_share_a_tile() {
    // Two stores feed one: wagons from the north and from the south come to a switch and go east together.
    let mut k = rich();
    let a = depot(&mut k, (160, 90));
    let b = depot(&mut k, (160, 100));
    track(&mut k, (163, 91), (167, 91));
    track(&mut k, (168, 91), (168, 95));
    track(&mut k, (163, 101), (167, 101));
    track(&mut k, (168, 101), (168, 97));
    track(&mut k, (168, 96), (172, 96));
    let out = depot(&mut k, (173, 95));
    bay(&mut k, (163, 91), "North");
    bay(&mut k, (163, 101), "South");
    bay(&mut k, (172, 96), "Out");
    assert!(k.rails.is_switch((168, 96)), "three ways meet at the switch");
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 60);
    k.buildings.get_mut(&b).unwrap().output.insert("beam".into(), 60);
    route(&mut k, "N", "North", "Out", 2);
    route(&mut k, "S", "South", "Out", 2);
    run_checked(&mut k, 300);
    let o = &k.buildings[&out].output;
    assert!(o.get("plank").copied().unwrap_or(0) >= 16 && o.get("beam").copied().unwrap_or(0) >= 16, "both routes get through: {o:?}");
    assert!(k.rail_jams(catalog()).is_empty(), "taking turns is not a jam: {:?}", k.rail_jams(catalog()));
}

#[test]
fn two_wagons_pass_on_plain_track_and_meet_head_on_only_where_the_track_is_shut() {
    let c = catalog();
    // One track between two stores, a route each way and two wagons to a route: they pass one another on
    // the way, a lane each, and the goods go both ways.
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    track(&mut k, (163, 96), (176, 96));
    let b = depot(&mut k, (177, 96));
    bay(&mut k, (163, 96), "West");
    bay(&mut k, (176, 96), "East");
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 80);
    k.buildings.get_mut(&b).unwrap().output.insert("beam".into(), 80);
    route(&mut k, "East-bound", "West", "East", 2);
    route(&mut k, "West-bound", "East", "West", 2);
    run_checked(&mut k, 300);
    assert!(k.rail_jams(c).is_empty(), "{:?}", k.rail_jams(c));
    assert!(plank(&k, b) >= 24 && k.buildings[&a].output.get("beam").copied().unwrap_or(0) >= 24, "goods go both ways");
}

#[test]
fn a_bay_on_the_main_line_holds_up_what_passes_and_a_bypass_lets_it_round() {
    let c = catalog();
    // A main line from West to East with a bay in its middle serving a store whose wagon waits there for
    // goods that come slowly: the wagons of the long route queue behind it. A bypass round the bay's tile
    // lets them pass.
    let mut k = rich();
    let west = depot(&mut k, (160, 96));
    track(&mut k, (163, 96), (180, 96));
    let east = depot(&mut k, (181, 96));
    let mid = depot(&mut k, (170, 97));
    bay(&mut k, (163, 96), "West");
    bay(&mut k, (180, 96), "East");
    bay(&mut k, (171, 96), "Middle");
    k.buildings.get_mut(&west).unwrap().output.insert("plank".into(), 200);
    let _ = mid;
    route(&mut k, "Long", "West", "East", 3);
    // The middle store's own wagons: they load from a store with nothing to give and stand out their patience.
    route(&mut k, "Short", "Middle", "East", 3);
    // Wagon-seconds spent waiting on the track, and what reaches the east, over 300 seconds.
    let mut trial = |k: &mut Kingdom| {
        let before = plank(k, east);
        let mut queued = 0u32;
        for _ in 0..300 {
            run_checked(k, 1);
            queued += k.rails.wagons.values().filter(|w| w.waiting > 0).count() as u32;
        }
        (plank(k, east) - before, queued)
    };
    let (slow, jammed) = trial(&mut k);
    assert!(jammed > 300, "the long route's wagons wait behind the middle bay: {jammed} wagon-seconds");
    // A bypass round the bay's tile: the wagons find it by themselves.
    for t in [(169, 95), (170, 95), (171, 95), (172, 95), (173, 95)] {
        rail(&mut k, t);
    }
    let (fast, waiting) = trial(&mut k);
    assert!(fast > slow, "the bypass lets the long route run: {slow} before, {fast} after");
    assert!(waiting < jammed, "and the queue is shorter: {jammed} wagon-seconds before, {waiting} after");
    let _ = c;
}

#[test]
fn a_wagon_at_a_bay_holds_it_and_the_ones_behind_queue_on_the_rail() {
    let c = catalog();
    let (mut k, a, b, r) = straight(6);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 80);
    k.apply(c, Command::Wagons { route: r, wagons: 4 }).unwrap();
    run_checked(&mut k, 40);
    let waiting = k.rails.wagons.values().filter(|w| w.waiting > 0).count();
    assert!(waiting >= 1, "wagons wait for the bay: {waiting}");
    run_checked(&mut k, 200);
    assert!(k.buildings[&b].output.get("plank").copied().unwrap_or(0) >= 24);
}

#[test]
fn horses_need_fodder_and_a_route_without_it_stands() {
    let c = catalog();
    let (mut k, a, b, r) = straight(8);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 40);
    k.stock.insert("fodder".into(), 0);
    k.rails.routes.get_mut(&r).unwrap().fed = 0;
    run_checked(&mut k, 60);
    assert_eq!(plank(&k, b), 0, "no hay, no wagon");
    assert_eq!(k.wagon_state(c, *k.rails.wagons.keys().next().unwrap()), WagonState::Unfed);
    k.stock.insert("fodder".into(), 100);
    run_checked(&mut k, 120);
    assert!(plank(&k, b) >= 8);
}

#[test]
fn a_loaded_wagon_is_saved_and_runs_on_as_it_did() {
    let c = catalog();
    let (mut k, a, b, _) = straight(9);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 60);
    run_checked(&mut k, 70);
    let json = serde_json::to_string(&k).unwrap();
    let mut loaded: Kingdom = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded, k);
    assert!(loaded.valid(c));
    run_checked(&mut k, 90);
    run_checked(&mut loaded, 90);
    assert_eq!(plank(&loaded, b), plank(&k, b));
    assert_eq!(loaded.rails, k.rails);
}

#[test]
fn an_old_tramway_comes_onto_the_network_as_rail_two_bays_and_a_route() {
    let c = catalog();
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let b = depot(&mut k, (173, 96));
    // What a save from before the rails held: a tramway line along twelve tiles of track, two wagons on it.
    let path: Vec<Tile> = (162..=173).map(|x| (x, 96)).collect();
    k.lines.insert(
        1,
        Line {
            kind: LineKind::Tram,
            from: a,
            to: b,
            path,
            forward: 120,
            back: 120,
            fleet: vec![Vehicle::default(), Vehicle { phase: Phase::Out, resource: "plank".into(), amount: 3, clock: 40 }],
            stats: LineStats::default(),
            fed: 100,
        },
    );
    k.next_line = 2;
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 40);
    let total = k.total("plank");
    run_checked(&mut k, 1);
    assert!(k.lines.is_empty(), "the tramway line is gone");
    assert_eq!(k.rails.cells.len(), 10, "its track is rail now");
    assert_eq!(k.rails.bays.len(), 2);
    assert_eq!(k.rails.routes.len(), 1);
    assert_eq!(k.rails.wagons.len(), 2);
    assert_eq!(k.total("plank"), total, "what the wagons carried is not lost");
    run_checked(&mut k, 150);
    assert!(plank(&k, b) >= 8, "the wagons run as they did: {}", plank(&k, b));
    assert!(k.valid(c));
}

// ---- the one-gesture railway ---------------------------------------------------------

#[test]
fn a_railway_is_one_gesture_from_one_work_to_another_and_the_goods_ride_it() {
    let c = catalog();
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let b = depot(&mut k, (174, 100));
    let said = k.apply(c, Command::Railway { from: a, to: b, path: Vec::new(), wagons: 2, good: None }).unwrap();
    assert!(said.contains("railway runs") && said.contains("2 wagons"), "{said}");
    // The rail, a bay at each work named for it, the route and the wagons: all from the one command.
    assert!(k.rails.cells.len() >= 14, "the rail between them: {}", k.rails.cells.len());
    assert_eq!(k.rails.bays.keys().cloned().collect::<Vec<_>>(), ["Warehouse", "Warehouse 2"]);
    assert_eq!(k.bay_building("Warehouse"), Some(a));
    assert_eq!(k.bay_building("Warehouse 2"), Some(b));
    let route = k.rails.routes.values().next().unwrap();
    assert_eq!((route.stops.len(), route.stops[0].act, route.stops[1].act), (2, Act::Load, Act::Unload));
    assert_eq!(k.rails.wagons.len(), 2);
    assert!(k.valid(c));
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 60);
    run_checked(&mut k, 200);
    assert!(plank(&k, b) >= 24, "the railway carries: {} planks over", plank(&k, b));
    assert!(k.rail_jams(c).is_empty());
}

#[test]
fn a_railway_that_cannot_be_paid_for_or_would_not_run_changes_nothing() {
    let c = catalog();
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let b = depot(&mut k, (174, 100));
    let before = (k.stock.clone(), k.rails.clone());
    // No horses for the wagons: the rail is not laid either.
    k.stock.insert("horse".into(), 0);
    let before = (k.stock.clone(), before.1);
    let err = k.apply(c, Command::Railway { from: a, to: b, path: Vec::new(), wagons: 2, good: None }).unwrap_err();
    assert!(err.contains("wagons need"), "{err}");
    assert_eq!((k.stock.clone(), k.rails.clone()), before, "nothing was laid or paid");
    // Too many wagons, a swape, the same work twice, a path that does not run beside the works.
    k.stock.insert("horse".into(), 600);
    for (cmd, why) in [
        (Command::Railway { from: a, to: b, path: Vec::new(), wagons: 99, good: None }, "wagons"),
        (Command::Railway { from: a, to: a, path: Vec::new(), wagons: 1, good: None }, "different works"),
        (Command::Railway { from: a, to: b, path: vec![(163, 96), (165, 96)], wagons: 1, good: None }, "one beside the next"),
        (Command::Railway { from: a, to: b, path: vec![(163, 96), (164, 96)], wagons: 1, good: None }, "beside the two works"),
        (Command::Railway { from: a, to: b, path: Vec::new(), wagons: 1, good: Some("nothing".into()) }, "no such good"),
    ] {
        let err = k.apply(c, cmd).unwrap_err();
        assert!(err.contains(why), "{why}: {err}");
        assert!(k.rails.cells.is_empty() && k.rails.routes.is_empty(), "a refused railway lays nothing");
    }
    // The charter first.
    let mut young = rich();
    let (x, y) = (depot(&mut young, (160, 96)), depot(&mut young, (174, 100)));
    young.researched.remove("rails");
    let err = young.apply(c, Command::Railway { from: x, to: y, path: Vec::new(), wagons: 1, good: None }).unwrap_err();
    assert!(err.contains("research"), "{err}");
}

#[test]
fn a_second_railway_joins_the_first_and_shares_its_bay() {
    let c = catalog();
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let b = depot(&mut k, (176, 96));
    let d = depot(&mut k, (176, 104));
    k.apply(c, Command::Railway { from: a, to: b, path: Vec::new(), wagons: 1, good: Some("plank".into()) }).unwrap();
    let laid = k.rails.cells.len();
    k.apply(c, Command::Railway { from: a, to: d, path: Vec::new(), wagons: 1, good: Some("beam".into()) }).unwrap();
    assert_eq!(k.rails.routes.len(), 2);
    assert_eq!(k.rails.bays.len(), 3, "the first work's bay serves both railways: {:?}", k.rails.bays.keys().collect::<Vec<_>>());
    assert!(k.rails.cells.len() - laid < laid, "the second railway used the first's rail where it could");
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 40);
    k.buildings.get_mut(&a).unwrap().output.insert("beam".into(), 40);
    run_checked(&mut k, 300);
    assert!(plank(&k, b) >= 8, "{}", plank(&k, b));
    assert!(k.buildings[&d].output.get("beam").copied().unwrap_or(0) >= 8, "{:?}", k.buildings[&d].output);
}

#[test]
fn the_planner_goes_round_what_stands_between_two_works() {
    let c = catalog();
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let b = depot(&mut k, (176, 96));
    // A wall of stores across the straight way.
    for y in (90..=102).step_by(2) {
        depot(&mut k, (168, y));
    }
    let way = k.plan_rail_path(c, a, b).unwrap();
    assert!(way.len() > 16, "the way bends round the wall: {}", way.len());
    assert!(way.iter().all(|t| k.check_rail_site(c, *t).is_ok()));
    k.apply(c, Command::Railway { from: a, to: b, path: way, wagons: 1, good: None }).unwrap();
    assert!(k.valid(c));
}
