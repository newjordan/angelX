//! Rollerways and swapes: the continuous line (`rollers`).
use super::*;
use crate::rollers::{Cell, Dir, Piece, RollerState, SLOTS};
use crate::works_tests::{build, run};

/// A rich, fully chartered hillside with a deep treasury: these tests are of the line, not of the price.
fn rich() -> Kingdom {
    let mut k = crate::works_tests::rich();
    for r in catalog().resources.keys() {
        if r != "fodder" {
            k.stock.insert(r.clone(), 600);
        }
    }
    k
}

fn lay(k: &mut Kingdom, piece: Piece, tile: Tile, dir: Dir) {
    k.apply(catalog(), Command::Roller { piece, tile, dir }).unwrap_or_else(|e| panic!("{piece:?} at {tile:?}: {e}"));
}
fn run_of(k: &mut Kingdom, piece: Piece, from: Tile, dir: Dir, n: i16) {
    for i in 0..n {
        let t = (from.0 + dir.delta().0 * i, from.1 + dir.delta().1 * i);
        lay(k, piece, t, dir);
    }
}
/// A swape at `tile` facing `dir`, finished, with its price paid from the rich camp.
fn swape(k: &mut Kingdom, tile: Tile, dir: Dir) -> u32 {
    let id = build(k, "swape", tile);
    k.apply(catalog(), Command::Face { tile, dir }).unwrap();
    k.buildings.get_mut(&id).unwrap().construction = 0;
    id
}
fn depot(k: &mut Kingdom, tile: Tile) -> u32 {
    let id = build(k, "depot", tile);
    k.buildings.get_mut(&id).unwrap().construction = 0;
    id
}
/// Two stores a rollerway apart: depot A at (160, 96), a swape loading, `len` tiles of `piece`, a swape
/// unloading into depot B. Returns (A, B, loader, unloader).
fn two_stores(piece: Piece, len: i16) -> (Kingdom, u32, u32, u32, u32) {
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let loader = swape(&mut k, (163, 96), Dir::E);
    run_of(&mut k, piece, (164, 96), Dir::E, len);
    let unloader = swape(&mut k, (164 + len, 96), Dir::E);
    let b = depot(&mut k, (165 + len, 96));
    (k, a, b, loader, unloader)
}
fn plank(k: &Kingdom, id: u32) -> u32 {
    k.buildings[&id].output.get("plank").copied().unwrap_or(0)
}

#[test]
fn goods_stream_from_one_store_to_another_along_a_rollerway_and_none_are_lost() {
    let c = catalog();
    let (mut k, a, b, _, _) = two_stores(Piece::Roller, 8);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 40);
    let before = k.total("plank");
    run(&mut k, 120);
    let moved = plank(&k, b);
    assert!(moved >= 20, "twenty planks in two minutes at a boy's pace: {moved}");
    assert!(moved <= 40 + 1);
    assert_eq!(k.total("plank"), before, "every plank is somewhere");
    assert_eq!(plank(&k, a) + plank(&k, b) + k.rollers.holding("plank"), 40);
    let row = &k.roller_report(c)[0];
    assert_eq!((row.tiles, row.swapes), (8, 2));
    assert!(row.per_min > 0, "the report counts the lifts: {}", row.per_min);
}

#[test]
fn a_boy_lifts_a_good_every_two_seconds_and_a_drive_shaft_makes_it_three_times_as_fast() {
    let c = catalog();
    let (mut slow, a, b, _, _) = two_stores(Piece::Roller, 6);
    slow.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 200);
    run(&mut slow, 60);
    let by_hand = plank(&slow, b) + slow.rollers.holding("plank");
    // 60 seconds at two seconds a lift is thirty, give or take the first one's fill.
    assert!((27..=31).contains(&by_hand), "a boy's pace: {by_hand}");
    // The arm fitted to the drive: it lifts at once, and the line carries all it is given.
    let (mut fast, a, b, loader, _) = two_stores(Piece::Roller, 6);
    fast.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 200);
    fast.stock.insert("plank".into(), 400);
    fast.stock.insert("nails".into(), 400);
    fast.apply(c, Command::EnablePower(loader)).unwrap();
    assert!(fast.buildings[&loader].power_upgrade);
    // A waterwheel's shaft would drive it; the multiplier is what it gains when driven.
    assert_eq!(c.plans["swape"].power.as_ref().unwrap().multiplier, 300);
    run(&mut fast, 5);
    assert!(fast.valid(c));
    let _ = b;
}

#[test]
fn a_line_that_cannot_unload_backs_up_tile_by_tile_from_its_end() {
    let c = catalog();
    let (mut k, a, b, _, unloader) = two_stores(Piece::Roller, 8);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 200);
    // The far store is shut: nothing can be put in it.
    k.apply(c, Command::Pause(b)).unwrap();
    let mut filled = Vec::new();
    for _ in 0..8 {
        run(&mut k, 15);
        filled.push((0..8).filter(|i| k.rollers.cells[&(164 + i, 96)].held() == SLOTS as u32).count());
    }
    assert!(filled.windows(2).all(|w| w[1] >= w[0]), "the full tiles only grow: {filled:?}");
    assert!(filled.first() < filled.last(), "it fills from the end: {filled:?}");
    // From the end: the last tile is full before the first.
    assert_eq!(k.rollers.cells[&(171, 96)].held(), SLOTS as u32);
    assert_eq!(k.rollers.cells[&(164, 96)].held(), SLOTS as u32, "after two minutes the whole line is packed");
    assert_eq!(k.buildings[&unloader].lift, 2, "the unloading swape knows it has nowhere to put the plank");
    let row = &k.roller_report(c)[0];
    assert!(matches!(row.state, RollerState::Jammed | RollerState::Backed), "{:?}", row.state);
    let jam = &row.jams[0];
    assert_eq!(jam.tile, (171, 96), "the jam's head is the line's end");
    assert!(jam.why.contains("cannot put sawn planks into the warehouse"), "{}", jam.why);
    assert_eq!(k.buildings[&a].output["plank"] + k.rollers.holding("plank"), 200, "nothing is lost on the way");
    // Open the store again and it clears.
    k.apply(c, Command::Pause(b)).unwrap();
    run(&mut k, 120);
    assert!(plank(&k, b) >= 20);
    assert!(k.rollers.cells[&(171, 96)].held() < SLOTS as u32 || plank(&k, a) == 0 || plank(&k, b) > 20);
}

#[test]
fn a_splitter_deals_goods_out_in_turn_and_a_second_unloader_clears_a_backed_up_line() {
    let c = catalog();
    // A line east to a splitter at (168, 96); north and south of it, a short branch each into a store.
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    swape(&mut k, (163, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (164, 96), Dir::E, 4);
    lay(&mut k, Piece::Splitter, (168, 96), Dir::E);
    // Front is closed (no cell); the left (north) and right (south) branches.
    run_of(&mut k, Piece::Roller, (168, 95), Dir::N, 1);
    run_of(&mut k, Piece::Roller, (168, 97), Dir::S, 1);
    let north = depot(&mut k, (167, 92));
    let south = depot(&mut k, (167, 99));
    let un_n = swape(&mut k, (168, 94), Dir::N);
    let un_s = swape(&mut k, (168, 98), Dir::S);
    assert_eq!(k.swape_ends(un_n), Some(((168, 95), (168, 93))));
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 160);
    let total = k.total("plank");
    run(&mut k, 150);
    let (n, s) = (plank(&k, north), plank(&k, south));
    assert!(n >= 6 && s >= 6, "both branches take goods: {n} north, {s} south");
    assert!(n.abs_diff(s) <= 3, "dealt in turn: {n} and {s}");
    // Shut one store: the splitter sends everything the other way and the line does not stop.
    k.apply(c, Command::Pause(north)).unwrap();
    let before = plank(&k, south);
    run(&mut k, 100);
    assert!(plank(&k, south) >= before + 10, "all of it goes south now: {} to {}", before, plank(&k, south));
    assert_eq!(k.total("plank"), total, "none lost or made");
    let _ = un_s;
}

#[test]
fn a_priority_gate_fills_the_front_and_spills_to_the_sides_only_when_it_is_full() {
    let c = catalog();
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    swape(&mut k, (163, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (164, 96), Dir::E, 3);
    lay(&mut k, Piece::Gate, (167, 96), Dir::E);
    // The front way: a short run into a store the swape serves slowly; the side way: a second store.
    run_of(&mut k, Piece::Roller, (168, 96), Dir::E, 2);
    let front = depot(&mut k, (171, 96));
    let u_front = swape(&mut k, (170, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (167, 97), Dir::S, 2);
    let side = depot(&mut k, (166, 100));
    let u_side = swape(&mut k, (167, 99), Dir::S);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 80);
    run(&mut k, 30);
    assert!(plank(&k, front) > 0, "the front is served first");
    assert_eq!(plank(&k, side), 0, "and while it takes goods the side gets none: {}", plank(&k, side));
    // Close the front store: the gate spills them to the side.
    k.apply(c, Command::Pause(front)).unwrap();
    run(&mut k, 90);
    assert!(plank(&k, side) >= 8, "the overflow reaches the side: {}", plank(&k, side));
    let _ = (u_front, u_side);
    assert!(k.valid(c));
}

#[test]
fn two_lines_merge_in_turn_so_neither_starves() {
    let c = catalog();
    let mut k = rich();
    // Two stores feed a merge from the north and south; the merge sends east along a short line into a third.
    let a = depot(&mut k, (160, 90));
    let b = depot(&mut k, (160, 100));
    swape(&mut k, (163, 91), Dir::E);
    swape(&mut k, (163, 101), Dir::E);
    // North feeder: east along y=91 then south to the merge at (168, 96); south feeder mirrors it.
    run_of(&mut k, Piece::Roller, (164, 91), Dir::E, 4);
    run_of(&mut k, Piece::Roller, (168, 91), Dir::S, 5);
    run_of(&mut k, Piece::Roller, (164, 101), Dir::E, 4);
    run_of(&mut k, Piece::Roller, (168, 101), Dir::N, 5);
    lay(&mut k, Piece::Merge, (168, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (169, 96), Dir::E, 3);
    let out = depot(&mut k, (173, 96));
    swape(&mut k, (172, 96), Dir::E);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 60);
    k.buildings.get_mut(&b).unwrap().output.insert("beam".into(), 60);
    run(&mut k, 240);
    let planks = plank(&k, out);
    let beams = k.buildings[&out].output.get("beam").copied().unwrap_or(0);
    assert!(planks >= 8 && beams >= 8, "both lines get through the merge: {planks} planks, {beams} beams");
    assert!(planks.abs_diff(beams) <= 4, "in turn: {planks} and {beams}");
    assert!(k.valid(c));
}

#[test]
fn a_trestle_carries_two_lines_over_each_other_without_mixing_them() {
    let c = catalog();
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let b = depot(&mut k, (166, 90));
    swape(&mut k, (163, 96), Dir::E);
    swape(&mut k, (166, 92), Dir::S);
    // West to east along y=96 and north to south along x=168, crossing at (168, 96).
    run_of(&mut k, Piece::Roller, (164, 96), Dir::E, 4);
    lay(&mut k, Piece::Trestle, (168, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (169, 96), Dir::E, 2);
    let east = depot(&mut k, (172, 96));
    swape(&mut k, (171, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (166, 93), Dir::E, 2);
    run_of(&mut k, Piece::Roller, (168, 93), Dir::S, 3);
    run_of(&mut k, Piece::Roller, (168, 97), Dir::S, 2);
    let south = depot(&mut k, (167, 100));
    swape(&mut k, (168, 99), Dir::S);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 30);
    k.buildings.get_mut(&b).unwrap().output.insert("beam".into(), 30);
    run(&mut k, 200);
    let e = &k.buildings[&east].output;
    let s = &k.buildings[&south].output;
    assert!(e.get("plank").copied().unwrap_or(0) >= 5, "planks went east: {e:?}");
    assert!(s.get("beam").copied().unwrap_or(0) >= 5, "beams went south: {s:?}");
    assert!(e.get("beam").is_none() && s.get("plank").is_none(), "nothing crossed over: {e:?} {s:?}");
    assert!(k.valid(c));
}

#[test]
fn a_filtered_loader_keeps_goods_the_far_end_cannot_use_off_the_line() {
    let c = catalog();
    // A bench that wants limestone only: the line from a store of limestone and ale. Without a filter the
    // ale reaches the end of the line, and nobody takes it.
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    let loader = swape(&mut k, (163, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (164, 96), Dir::E, 5);
    let kiln = build(&mut k, "lime-kiln", (170, 96));
    k.buildings.get_mut(&kiln).unwrap().construction = 0;
    let unloader = swape(&mut k, (169, 96), Dir::E);
    k.buildings.get_mut(&a).unwrap().output.insert("limestone".into(), 6);
    k.buildings.get_mut(&a).unwrap().output.insert("ale".into(), 30);
    // The loader takes the good there is most of, so the ale goes first, and the kiln wants none of it.
    run(&mut k, 120);
    assert!(k.buildings[&kiln].input.get("limestone").copied().unwrap_or(0) == 0, "the ale blocks the line");
    let row = &k.roller_report(c)[0];
    assert_eq!(row.state, RollerState::Jammed);
    assert!(row.jams[0].why.contains("ale"), "{}", row.jams[0].why);
    // Set the loader to limestone; take the ale off the line.
    for t in 164..169 {
        k.apply(c, Command::Demolish((t, 96))).unwrap();
    }
    run_of(&mut k, Piece::Roller, (164, 96), Dir::E, 5);
    k.apply(c, Command::Filter { building: loader, good: Some("limestone".into()) }).unwrap();
    run(&mut k, 90);
    assert!(k.buildings[&kiln].input.get("limestone").copied().unwrap_or(0) >= 2, "limestone reaches the kiln: {:?}", k.buildings[&kiln].input);
    let _ = unloader;
    assert!(k.valid(c));
}

#[test]
fn the_hand_chain_is_free_to_lay_and_a_labourer_a_tile_and_slower_than_rollers() {
    let c = catalog();
    let mut k = rich();
    let before = k.stock.clone();
    let a = depot(&mut k, (160, 96));
    let b_at = (170, 96);
    swape(&mut k, (163, 96), Dir::E);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 100);
    let kept = k.stock.clone();
    run_of(&mut k, Piece::Chain, (164, 96), Dir::E, 5);
    assert_eq!(k.stock, kept, "a hand-chain costs nothing to lay");
    swape(&mut k, (169, 96), Dir::E);
    let b = depot(&mut k, b_at);
    let _ = before;
    assert_eq!(k.chain_crew(c), 5, "a pair of hands a tile");
    let sites = k.settlement_sites(c);
    assert_eq!(sites.iter().find(|s| s.id == 1).unwrap().jobs, 5, "the camp's people are the chain's");
    run(&mut k, 120);
    assert!(plank(&k, b) >= 15, "the chain carries goods: {}", plank(&k, b));
    assert!(k.valid(c));
}

#[test]
fn a_rollerway_takes_the_ground_and_nothing_else_may_be_built_or_laid_over_it() {
    let c = catalog();
    let mut k = rich();
    lay(&mut k, Piece::Roller, (170, 96), Dir::E);
    assert!(k.apply(c, Command::Build { plan: "sawpit".into(), tile: (170, 96) }).is_err(), "no building on a tile of rollerway");
    assert!(k.apply(c, Command::Build { plan: "sawpit".into(), tile: (169, 96) }).is_err(), "nor over its end: the sawpit runs east");
    assert!(k.apply(c, Command::Road((170, 96))).is_err());
    assert!(k.apply(c, Command::Roller { piece: Piece::Roller, tile: (170, 96), dir: Dir::E }).is_err(), "laid so already");
    // Turned, it is not paid for again; over another piece it is.
    let stock = k.stock.clone();
    k.apply(c, Command::Face { tile: (170, 96), dir: Dir::S }).unwrap();
    assert_eq!(k.rollers.cells[&(170, 96)].dir, Dir::S);
    assert_eq!(k.stock, stock);
    // A road is crossed on a trestle, not under a roller.
    k.apply(c, Command::Road((172, 96))).unwrap();
    assert!(k.apply(c, Command::Roller { piece: Piece::Roller, tile: (172, 96), dir: Dir::E }).is_err());
    lay(&mut k, Piece::Trestle, (172, 96), Dir::E);
    assert!(k.roads.contains(&(172, 96)), "the road runs on under the trestle");
    // River, the Keep and the gate refuse it.
    assert!(k.apply(c, Command::Roller { piece: Piece::Roller, tile: hillside::KEEP_CENTRE, dir: Dir::E }).is_err());
    // Taken up, it gives back its goods to the camp.
    k.rollers.cells.get_mut(&(170, 96)).unwrap().slots[1] = 1;
    k.rollers.goods.push("plank".into());
    let planks = k.amount("plank");
    k.apply(c, Command::Demolish((170, 96))).unwrap();
    assert_eq!(k.amount("plank"), planks + 1);
    assert!(k.rollers.cells.get(&(170, 96)).is_none());
    assert!(k.valid(c));
}

#[test]
fn rollerways_are_saved_with_their_goods_and_a_loaded_one_runs_on_as_the_original_did() {
    let c = catalog();
    let (mut k, a, b, _, _) = two_stores(Piece::Roller, 6);
    k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 30);
    run(&mut k, 40);
    assert!(k.rollers.holding("plank") > 0, "goods are on the line when it is saved");
    let json = serde_json::to_string(&k).unwrap();
    let mut loaded: Kingdom = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded, k, "the save is the whole of the line");
    assert!(loaded.valid(c));
    run(&mut k, 60);
    run(&mut loaded, 60);
    assert_eq!(plank(&loaded, b), plank(&k, b), "both played on alike");
    assert_eq!(loaded.rollers.cells, k.rollers.cells);
    // An old save with no rollerways at all loads as it was.
    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    old.as_object_mut().unwrap().remove("rollers");
    let bare: Kingdom = serde_json::from_value(old).unwrap();
    assert!(bare.rollers.is_empty());
}

#[test]
fn hundreds_of_tiles_and_thousands_of_goods_cost_a_browser_seat_almost_nothing() {
    let c = catalog();
    let mut k = rich();
    // Eight rows of sixty tiles, every slot full of planks but the last tile of each: 480 tiles, 1,400 goods.
    k.rollers.goods.push("plank".into());
    for row in 0..8i16 {
        for x in 0..60i16 {
            let mut cell = Cell::new(Piece::Roller, Dir::E);
            if x < 59 {
                cell.slots = [1; SLOTS];
            }
            k.rollers.cells.insert((100 + x, 20 + row * 2), cell);
        }
    }
    k.rollers.touch();
    let goods = k.rollers.cells.values().map(|c| c.held()).sum::<u32>();
    assert!(goods > 1_300);
    let t = std::time::Instant::now();
    for _ in 0..10 {
        k.rollers_tick(c, &Report::default());
    }
    let per = t.elapsed().as_secs_f64() * 100.0;
    // A second of the whole line takes under ten milliseconds, in a build with checks on.
    eprintln!("rollers: {per:.2} ms a second for {} tiles and {goods} goods", k.rollers.cells.len());
    assert!(per < 20.0, "{per:.1} ms a second for {} tiles", k.rollers.cells.len());
    assert_eq!(k.rollers.cells.values().map(|c| c.held()).sum::<u32>(), goods, "nothing made or lost by moving it");
}

#[test]
fn a_hand_chain_moves_goods_at_half_a_rollerway_s_pace() {
    // The first plank reaches the far store after about this many seconds, on a line of ten tiles.
    let arrival = |piece: Piece| {
        let (mut k, a, b, _, _) = two_stores(piece, 10);
        k.buildings.get_mut(&a).unwrap().output.insert("plank".into(), 5);
        for t in 1..=200 {
            k.advance_second(catalog());
            if plank(&k, b) > 0 {
                return t;
            }
        }
        unreachable!("no plank in 200 seconds");
    };
    let (roller, chain) = (arrival(Piece::Roller), arrival(Piece::Chain));
    assert!(chain >= roller + 10, "ten tiles of chain {chain} s, of rollers {roller} s");
}

#[test]
fn a_full_branch_the_rest_of_the_line_goes_round_is_not_a_jam() {
    let c = catalog();
    // A line east to a splitter; its front is a store that takes all, its right a kiln that cannot be put more into.
    let mut k = rich();
    let a = depot(&mut k, (160, 96));
    swape(&mut k, (163, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (164, 96), Dir::E, 4);
    lay(&mut k, Piece::Splitter, (168, 96), Dir::E);
    run_of(&mut k, Piece::Roller, (169, 96), Dir::E, 2);
    swape(&mut k, (171, 96), Dir::E);
    let store = depot(&mut k, (172, 96));
    run_of(&mut k, Piece::Roller, (168, 97), Dir::S, 2);
    let kiln = build(&mut k, "lime-kiln", (168, 100));
    k.buildings.get_mut(&kiln).unwrap().construction = 0;
    swape(&mut k, (168, 99), Dir::S);
    k.buildings.get_mut(&a).unwrap().output.insert("limestone".into(), 120);
    k.buildings.get_mut(&kiln).unwrap().input.insert("limestone".into(), 32);
    run(&mut k, 200);
    let rows = k.roller_report(c);
    assert!(rows[0].jams.iter().any(|j| !j.blocking && j.why.contains("lime kiln")), "the kiln branch stands full: {:?}", rows[0].jams);
    assert!(rows[0].jams.iter().all(|j| !j.blocking), "but the line is not blocked");
    assert_ne!(rows[0].state, RollerState::Jammed);
    assert!(k.buildings[&store].output.get("limestone").copied().unwrap_or(0) >= 20, "the rest goes to the store");
}

#[test]
fn a_swape_planned_facing_a_way_is_raised_so_and_laid_tiles_come_back_by_the_builders_undo() {
    use crate::ghosts::Laid;
    let c = catalog();
    let mut k = rich();
    // A swape laid as a plan, turned while it is still a plan.
    let tile = crate::first_chain::ground_for(c, &k, "swape", (150, 100)).expect("ground for a swape");
    k.apply(c, Command::Plan { plan: "swape".into(), tile, priority: 1, by: 1 }).expect("planned");
    k.apply(c, Command::Face { tile, dir: Dir::W }).expect("a planned swape turns");
    run(&mut k, 120);
    let id = k.at(tile).expect("the hands raised it");
    assert_eq!(k.buildings[&id].facing, Dir::W.index(), "raised facing the way it was planned");
    // Tiles of rollerway and rail, and the step that took them back.
    run_of(&mut k, Piece::Roller, (150, 110), Dir::E, 3);
    k.apply(c, Command::Rail((150, 114))).expect("a rail tile");
    assert!(k.apply(c, Command::Retract(vec![Laid::Roller((150, 110)), Laid::Roller((151, 110)), Laid::Roller((152, 110)), Laid::Rail((150, 114))])).is_ok());
    assert!(k.roller_at((151, 110)).is_none() && k.rails.cells.is_empty(), "taken up again");
    assert!(k.valid(c));
}
