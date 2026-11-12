//! Building where you stand: the builder in the walking world, on a run whose
//! industry is the hillside kingdom (docs/KINGDOM-WORKS.md).
use super::*;
use crate::kingdom::{Command, LineKind, catalog};
use std::collections::BTreeMap;

fn keys(seat: u32, input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(seat, input)])
}

fn rich(run: &mut Run) {
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 400);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
}

/// `seats` knights at home on the surface, the hillside founded and rich, every knight out
/// on open ground east of the camp, the builder not yet out.
fn run(seats: u32) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    for n in 1..seats {
        run.join(&format!("K{}", n + 1));
    }
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    rich(&mut run);
    // Open ground east of the camp, on whichever plan the tests run (the old
    // realm has trees there; the blank slate has none).
    let room = world::room_of(&run.dungeon, RoomKind::Kingdom).expect("the kingdom's room");
    for y in 95..130 {
        for x in 148..200 {
            run.dungeon.rooms[room].set(x, y, layout::Tile::Floor);
        }
    }
    for (n, h) in run.players.values_mut().enumerate() {
        (h.x, h.y) = ((158.5 + n as f32 * 4.0) * TILE_UNITS, 104.5 * TILE_UNITS);
        h.invulnerable = 100_000;
        h.stone = false;
    }
    still(&mut run, 3);
    run
}

/// `n` ticks of every knight standing still (the client sends its keys every tick).
fn still(run: &mut Run, n: usize) {
    let all: BTreeMap<u32, Input> = run.players.keys().map(|&id| (id, Input::default())).collect();
    for _ in 0..n {
        run.step(&all);
    }
}

fn tool_index(tool: Tool) -> usize {
    tools().iter().position(|t| *t == tool).expect("a tool of the builder")
}

fn press(run: &mut Run, seat: u32, input: Input) {
    run.step(&keys(seat, input));
    still(run, 1);
}

fn builder(run: &mut Run, seat: u32) {
    press(run, seat, Input { cast: 4, ..Default::default() });
    assert!(run.kingdom.industry[&seat].open && building(run, seat), "the builder is out");
}

fn tile_of(run: &Run, seat: u32) -> (i16, i16) {
    let h = &run.players[&seat];
    ((h.x / TILE_UNITS) as i16, (h.y / TILE_UNITS) as i16)
}

#[test]
fn key_four_puts_the_builder_in_hand_and_the_knight_keeps_walking() {
    let mut run = run(1);
    let before = run.players[&1].x;
    builder(&mut run, 1);
    // Not the old desk: no frontier, no separate map, and the knight walks on.
    assert!(!run.kingdom.frontier);
    for _ in 0..30 {
        run.step(&keys(1, Input { move_x: 1, ..Default::default() }));
    }
    assert!(run.players[&1].x > before + 1.0, "the knight walked: {} from {before}", run.players[&1].x);
    assert_eq!(run.kingdom.industry[&1].face, 1, "and faces the way they walk");
    // Key four again puts it away.
    press(&mut run, 1, Input { cast: 4, ..Default::default() });
    assert!(!run.kingdom.industry[&1].open);
}

#[test]
fn the_ghost_stands_on_the_ground_ahead_with_its_price_and_whether_the_land_suits_it() {
    let mut run = run(1);
    builder(&mut run, 1);
    let well = tool_index(Tool::Plan("well"));
    let kiln = tool_index(Tool::Plan("lime-kiln"));
    run.kingdom.industry.get_mut(&1).unwrap().plan = kiln;
    // Face east, then stand still.
    for _ in 0..3 {
        run.step(&keys(1, Input { move_x: 1, ..Default::default() }));
    }
    still(&mut run, 1);
    let feet = tile_of(&run, 1);
    let g = run.hill_ghost(1).expect("a ghost");
    assert_eq!(g.tool, Tool::Plan("lime-kiln"));
    // A 2 by 2 kiln, its near edge on the tile before the knight's feet.
    let (w, d) = (2, 2);
    assert_eq!((g.rect.2 - g.rect.0 + 1, g.rect.3 - g.rect.1 + 1), (w, d));
    assert_eq!(g.rect.0, feet.0 + 1, "the kiln starts the tile ahead: {:?} from {feet:?}", g.rect);
    assert!(g.rect.1 <= feet.1 && feet.1 <= g.rect.3, "centred on the knight's row");
    assert_eq!(g.verdict, Verdict::Fits, "{}", g.note);
    assert!(g.note.contains("THE LAND SUITS IT") && g.note.contains("TIMBER") || g.note.contains("STONE") || !g.cost.is_empty(), "{}", g.note);
    assert!(!g.cost.is_empty());
    // A well changes the footprint, not the cursor.
    run.kingdom.industry.get_mut(&1).unwrap().plan = well;
    let w = run.hill_ghost(1).unwrap();
    assert_eq!(w.cursor, g.cursor);
    // Poor: the land suits it and the camp lacks the price.
    run.home.industry.stock.clear();
    run.kingdom.industry.get_mut(&1).unwrap().plan = kiln;
    assert_eq!(run.hill_ghost(1).unwrap().verdict, Verdict::Short);
    // The Keep's ground refuses everything.
    let keep = run.players.get_mut(&1).unwrap();
    (keep.x, keep.y) = (125.5 * TILE_UNITS, 75.5 * TILE_UNITS);
    still(&mut run, 1);
    let g = run.hill_ghost(1).unwrap();
    assert_eq!(g.verdict, Verdict::Refused);
    assert!(g.note.contains("KEEP"), "{}", g.note);
}

#[test]
fn e_lays_what_the_ghost_shows_and_the_camp_pays_for_it() {
    let mut run = run(1);
    builder(&mut run, 1);
    let kiln = tool_index(Tool::Plan("lime-kiln"));
    run.kingdom.industry.get_mut(&1).unwrap().plan = kiln;
    for _ in 0..3 {
        run.step(&keys(1, Input { move_x: 1, ..Default::default() }));
    }
    still(&mut run, 1);
    let g = run.hill_ghost(1).unwrap();
    let timber = run.home.industry.amount("timber");
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    let id = run.home.industry.at((g.anchor.0, g.anchor.1)).expect("the kiln stands where the ghost was");
    assert_eq!(run.home.industry.buildings[&id].plan, "lime-kiln");
    assert!(run.home.industry.amount("timber") < timber || run.home.industry.amount("stone") < 400, "it was paid for");
    // The whole footprint is the kiln's.
    for (x, y) in [(g.rect.0, g.rect.1), (g.rect.2, g.rect.3)] {
        assert_eq!(run.home.industry.at((x, y)), Some(id));
    }
    // And a second one on the same ground is refused, with the words.
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert!(run.kingdom.industry[&1].message.to_lowercase().contains("empty") || run.kingdom.industry[&1].message.to_lowercase().contains("clear"), "{}", run.kingdom.industry[&1].message);
    assert_eq!(run.home.industry.buildings.values().filter(|b| b.plan == "lime-kiln").count(), 1);
}

#[test]
fn holding_e_while_walking_paints_a_road() {
    let mut run = run(1);
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Road);
    let roads = run.home.industry.roads.len();
    for _ in 0..80 {
        run.step(&keys(1, Input { move_x: 1, interact: true, ..Default::default() }));
    }
    let laid = run.home.industry.roads.len() - roads;
    assert!(laid >= 5, "a road behind the knight: {laid} tiles");
    // In a row, along the way they walked.
    let ys: std::collections::BTreeSet<i16> = run.home.industry.roads.iter().filter(|t| t.1 >= 100).map(|t| t.1).collect();
    assert!(ys.contains(&104) || ys.contains(&105), "{ys:?}");
}

#[test]
/// (The tramway this was written for is rails now, laid by the rail tool; the ropeway is the line that is still walked.)
fn a_line_is_drawn_by_walking_it_from_one_building_to_another() {
    let mut run = run(1);
    let c = catalog();
    // Two depots, finished, on open ground east of the camp.
    let k = &mut run.home.industry;
    k.apply(c, Command::Build { plan: "depot".into(), tile: (150, 100) }).expect("a depot");
    k.apply(c, Command::Build { plan: "depot".into(), tile: (162, 100) }).expect("another");
    for b in k.buildings.values_mut() {
        b.construction = 0;
    }
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Line(LineKind::Rope));
    // Stand by the first depot, press E, walk east along its row holding E, let go by the second.
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (152.5 * TILE_UNITS, 99.5 * TILE_UNITS);
    still(&mut run, 1);
    run.step(&keys(1, Input { move_x: 1, ..Default::default() }));
    // (Facing east, the cursor is the tile ahead.)
    // Begin: the cursor must be at or near the depot.
    run.players.get_mut(&1).unwrap().x = 153.5 * TILE_UNITS;
    run.step(&keys(1, Input { interact: true, ..Default::default() }));
    assert!(run.kingdom.industry[&1].draft.is_some(), "the line is begun: {}", run.kingdom.industry[&1].message);
    for _ in 0..140 {
        run.step(&keys(1, Input { move_x: 1, interact: true, ..Default::default() }));
        if tile_of(&run, 1).0 >= 160 {
            break;
        }
    }
    assert!(run.kingdom.industry[&1].draft.as_ref().is_some_and(|d| d.path.len() >= 4), "the ground walked is the line's way");
    run.step(&keys(1, Input::default()));
    assert!(run.kingdom.industry[&1].draft.is_none(), "let go ends it");
    assert_eq!(run.home.industry.lines.len(), 1, "{}", run.kingdom.industry[&1].message);
    let line = run.home.industry.lines.values().next().unwrap();
    assert_eq!(line.kind, LineKind::Rope);
    assert!(line.length() >= 4);
}

#[test]
fn every_seat_has_its_own_cursor_and_tool_and_everyone_shares_the_economy() {
    let mut run = run(2);
    builder(&mut run, 1);
    builder(&mut run, 2);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Plan("well"));
    run.kingdom.industry.get_mut(&2).unwrap().plan = tool_index(Tool::Plan("sawpit"));
    for _ in 0..3 {
        run.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() }), (2, Input { move_y: -1, ..Default::default() })]));
    }
    still(&mut run, 1);
    let (a, b) = (run.hill_ghost(1).unwrap(), run.hill_ghost(2).unwrap());
    assert_ne!(a.cursor, b.cursor);
    assert_eq!((a.tool.clone(), b.tool.clone()), (Tool::Plan("well"), Tool::Plan("sawpit")));
    assert_eq!(run.kingdom.industry[&1].face, 1);
    assert_eq!(run.kingdom.industry[&2].face, 0);
    let before = run.home.industry.buildings.len();
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() }), (2, Input { interact: true, ..Default::default() })]));
    assert_eq!(run.home.industry.buildings.len() - before, 2, "both laid theirs: {} / {}", run.kingdom.industry[&1].message, run.kingdom.industry[&2].message);
}

#[test]
fn the_mouse_sets_the_cursor_and_lays_with_the_left_button() {
    let mut run = run(1);
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Plan("well"));
    let at = BuilderAction::Plot { tile: (170, 110), build: false };
    assert!(at.valid());
    press(&mut run, 1, Input { builder: Some(at), ..Default::default() });
    let g = run.hill_ghost(1).unwrap();
    assert_eq!(g.cursor, (170, 110));
    assert!(run.kingdom.industry[&1].pointed);
    assert!(run.home.industry.at((170, 110)).is_none(), "a right click only points");
    press(&mut run, 1, Input { builder: Some(BuilderAction::Plot { tile: (170, 110), build: true }), ..Default::default() });
    assert!(run.home.industry.at((170, 110)).is_some(), "the left button lays it: {}", run.kingdom.industry[&1].message);
    // The valley's desk bounds no longer cap the pointer, and the realm's still do.
    assert!(!BuilderAction::Plot { tile: (250, 10), build: true }.valid());
    // Walking takes the cursor back.
    press(&mut run, 1, Input { move_x: 1, ..Default::default() });
    assert!(!run.kingdom.industry[&1].pointed);
}

#[test]
fn the_builders_state_goes_to_a_guest_and_stays_valid() {
    let mut run = run(1);
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Line(LineKind::Rope));
    run.kingdom.industry.get_mut(&1).unwrap().draft = Some(Draft { kind: LineKind::Rope, from: 2, path: vec![(150, 100), (151, 100)] });
    run.kingdom.industry.get_mut(&1).unwrap().overlay = true;
    let json = serde_json::to_string(&run.kingdom).expect("saved");
    let back: kingdom::State = serde_json::from_str(&json).expect("read");
    let b = &back.industry[&1];
    assert!(b.valid());
    assert_eq!(b.draft, run.kingdom.industry[&1].draft);
    assert!(b.overlay);
    // A hostile board is refused.
    let mut bad = b.clone();
    bad.draft = Some(Draft { kind: LineKind::Rope, from: 1, path: vec![(1, 1); MAX_PATH + 1] });
    assert!(!bad.valid());
    bad = b.clone();
    bad.tile = (300, 5);
    assert!(!bad.valid());
}

#[test]
fn the_ledger_opens_over_the_world_and_c_puts_it_back() {
    let mut run = run(1);
    builder(&mut run, 1);
    press(&mut run, 1, Input { ride: true, ..Default::default() });
    assert_eq!(run.kingdom.industry[&1].page, 1, "the ledger's first page");
    assert!(!run.kingdom.frontier, "over the world, not on the old map");
    // P turns the pages, C closes the ledger back to the builder.
    press(&mut run, 1, Input { cycle: true, ..Default::default() });
    assert_eq!(run.kingdom.industry[&1].page, 3, "Charters, then Claims, People, Works");
    press(&mut run, 1, Input { ride: true, ..Default::default() });
    assert_eq!(run.kingdom.industry[&1].page, 0);
    assert!(building(&run, 1));
}

// ---- the works as the renderers read them -------------------------------

use crate::world::hillside as view;

fn stepped(run: &mut Run, seconds: usize) {
    still(run, seconds * HZ as usize);
}

#[test]
fn the_view_reads_the_works_and_what_moves_and_a_copy_reads_the_same() {
    let mut run = super::shots::small_works();
    stepped(&mut run, 90);
    let hv = view::of(&run).expect("the hillside's works");
    let k = &run.home.industry;
    assert_eq!(hv.buildings.len(), k.buildings.len());
    assert_eq!(hv.roads.len(), k.roads.len());
    // Each building is read at the size the sim built it.
    for b in &hv.buildings {
        let p = &catalog().plans[&b.plan];
        let (span, depth) = k.dims(p);
        assert_eq!((b.rect[2] - b.rect[0], b.rect[3] - b.rect[1]), (i16::from(span), i16::from(depth)), "{}", b.plan);
    }
    // What moves: the carts of the yard, the people of the camp.
    assert_eq!(hv.carts.len(), k.buildings.values().map(|b| b.carts.len()).sum::<usize>());
    assert!(!hv.folk.is_empty(), "the camp's people are seen");
    assert!(hv.folk.iter().any(|f| f.trade.is_some()), "some stand at a post of a trade");
    for r in hv.carts.iter().chain(hv.folk.iter().map(|f| &f.rider)) {
        for frac in [0.0, 0.5, 1.0] {
            let p = r.place(frac);
            let (lo, hi) = ((r.from.0.min(r.to.0) - 0.01, r.from.1.min(r.to.1) - 0.01), (r.from.0.max(r.to.0) + 0.01, r.from.1.max(r.to.1) + 0.01));
            assert!(p.0 >= lo.0 && p.0 <= hi.0 && p.1 >= lo.1 && p.1 <= hi.1, "a rider stays between its two tiles: {p:?} of {:?} to {:?}", r.from, r.to);
        }
    }
    // The states are the sim's: a working kiln is a working picture, a building under construction is going up.
    assert!(hv.buildings.iter().any(|b| matches!(b.state, crate::render::overworld::works_kit::State::Working | crate::render::overworld::works_kit::State::Starved(_) | crate::render::overworld::works_kit::State::BackedUp(_))));
    // A guest's copy of the kingdom (the run is sent whole and read again) reads the same works.
    let json = serde_json::to_string(&run.home.industry).expect("saved");
    let copy: crate::kingdom::Kingdom = serde_json::from_str(&json).expect("read");
    assert_eq!(copy, run.home.industry);
    let mut guest = Run::at_home(41, 1, None, Default::default(), Default::default());
    guest.walk_kingdom(None);
    guest.home.industry = copy;
    guest.tick = run.tick;
    let gv = view::of(&guest).expect("the guest's works");
    assert_eq!((gv.buildings.len(), gv.roads.len(), gv.carts.len(), gv.folk.len()), (hv.buildings.len(), hv.roads.len(), hv.carts.len(), hv.folk.len()));
}

#[test]
fn a_run_off_the_hillside_has_no_works_to_read() {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    assert!(view::of(&run).is_none());
}

#[test]
#[ignore = "prints the cost of a large works; run with --ignored --nocapture"]
fn perf_a_large_works() {
    use std::time::Instant;
    let mut run = super::shots::run_at(160.5, 108.5);
    let c = catalog();
    let yard = super::shots::raise(&mut run, "carters-yard", (143, 98));
    // Every kind of work the hillside builds that its ground suits, round and round.
    let menu: Vec<&String> = c.hillside_menu.iter().filter(|p| !c.plans[*p].yard && c.plans[*p].connector.is_none() && c.plans[*p].stalls == 0 && c.plans[*p].replaces.is_none()).collect();
    let (mut n, mut ids) = (0usize, vec![yard]);
    run.home.industry.population = crate::kingdom::population::Population::founders_of(100);
    'fill: for row in 0..10i16 {
        for col in 0..12i16 {
            let tile = (148 + col * 7, 98 + row * 6);
            let k = &mut run.home.industry;
            for try_ in 0..menu.len() {
                let plan = menu[(n + try_) % menu.len()];
                if k.apply(c, Command::Build { plan: plan.clone(), tile }).is_ok() {
                    let id = k.at(tile).unwrap();
                    k.buildings.get_mut(&id).unwrap().construction = 0;
                    ids.push(id);
                    n += 1;
                    break;
                }
            }
            if n >= 70 {
                break 'fill;
            }
        }
    }
    let k = &mut run.home.industry;
    for id in &ids {
        let _ = k.apply(c, Command::Connect(*id));
    }
    let _ = k.apply(c, Command::Carts { yard, carts: 12 });
    stepped(&mut run, 120);
    let k = &run.home.industry;
    println!("{} buildings, {} roads, {} carts, {} people", k.buildings.len(), k.roads.len(), k.buildings.values().map(|b| b.carts.len()).sum::<usize>(), k.population.citizens.len());
    let t = Instant::now();
    for _ in 0..30 {
        run.home.industry.advance_second(c);
    }
    println!("sim second: {:?} each", t.elapsed() / 30);
    let t = Instant::now();
    let cold = {
        run.home.industry.rev += 1;
        view::of(&run).unwrap()
    };
    println!("the view, read once a second: {:?} ({} buildings, {} folk, {} carts, {} arrows)", t.elapsed(), cold.buildings.len(), cold.folk.len(), cold.carts.len(), cold.arrows.len());
    let t = Instant::now();
    for _ in 0..1000 {
        let _ = view::of(&run).unwrap();
    }
    println!("the view, kept: {:?} each", t.elapsed() / 1000);
    let json = serde_json::to_string(&run.home.industry).unwrap();
    println!("the kingdom as a guest is sent it: {} KB", json.len() / 1024);
}

#[test]
fn a_click_in_the_flat_view_lands_on_the_tile_under_it() {
    let mut run = run(1);
    still(&mut run, 40);
    let at = |x: i32, y: i32| crate::render::overworld::arena::hill_tile_at(&run, 1, x, y).expect("on the map");
    let mid = at(384, 224);
    // The view's middle is where the knight's camera is (it trails the knight a little: a dead zone).
    let cam = run.players[&1].camera.expect("a camera");
    assert_eq!((i32::from(mid.0), i32::from(mid.1)), ((cam.0 / TILE_UNITS).floor() as i32, (cam.1 / TILE_UNITS).floor() as i32));
    // A tile is sixteen pixels of the native view, thirty-two of the picture.
    assert_eq!(at(384 + 32, 224).0, mid.0 + 1);
    assert_eq!(at(384, 224 + 32).1, mid.1 + 1);
    assert_eq!(at(384 - 32, 224).0, mid.0 - 1);
}

#[test]
fn a_pattern_is_stamped_down_where_the_knight_stands() {
    let mut run = run(1);
    builder(&mut run, 1);
    let name = catalog().patterns.keys().next().expect("a pattern").as_str();
    let tool = tools().iter().find(|t| matches!(t, Tool::Pattern(n) if *n == name)).expect("its tool").clone();
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(tool);
    still(&mut run, 1);
    let g = run.hill_ghost(1).expect("a ghost");
    assert!(matches!(g.verdict, Verdict::Fits | Verdict::Short), "{}", g.note);
    assert!(g.rect.2 > g.rect.0 || g.rect.3 > g.rect.1, "a pattern covers ground: {:?}", g.rect);
    let before = run.home.industry.buildings.len();
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert!(run.home.industry.buildings.len() > before, "{}", run.kingdom.industry[&1].message);
    // P turns it a quarter, and the ghost's ground turns with it.
    let first = run.hill_ghost(1).unwrap().rect;
    press(&mut run, 1, Input { cycle: true, ..Default::default() });
    assert_eq!(run.kingdom.industry[&1].turn, 1);
    let turned = run.hill_ghost(1).unwrap().rect;
    assert_ne!(first, turned);
}

#[test]
fn a_building_shuts_its_ground_to_the_knights_and_opens_it_when_it_comes_down() {
    let mut run = run(1);
    let room = world::room_of(&run.dungeon, RoomKind::Kingdom).expect("the kingdom's room");
    let id = super::shots::raise(&mut run, "lime-kiln", (160, 104));
    run.follow_works_tiles(room);
    for t in [(160, 104), (161, 105)] {
        assert_eq!(run.dungeon.rooms[room].tile(t.0, t.1), layout::Tile::Wall, "{t:?} is the kiln's");
    }
    assert_eq!(run.dungeon.rooms[room].tile(162, 104), layout::Tile::Floor, "the ground beside it stays open");
    // A knight a building went up on is moved to open ground.
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (161.5 * TILE_UNITS, 105.5 * TILE_UNITS);
    run.home.industry.rev += 1;
    run.hands.seen.walled.clear();
    run.follow_works_tiles(room);
    let (tx, ty) = ((run.players[&1].x / TILE_UNITS) as i32, (run.players[&1].y / TILE_UNITS) as i32);
    assert_eq!(run.dungeon.rooms[room].tile(tx, ty), layout::Tile::Floor, "out from under it: {tx},{ty}");
    // Down it comes: the ground is open again.
    run.home.industry.apply(catalog(), Command::Demolish((160, 104))).expect("down");
    assert!(run.home.industry.buildings.get(&id).is_none());
    run.follow_works_tiles(room);
    assert_eq!(run.dungeon.rooms[room].tile(160, 104), layout::Tile::Floor);
}

#[test]
fn a_guest_across_the_wire_sees_the_same_works_and_the_same_ghost_and_each_second_is_a_small_patch() {
    use crate::wire::{Publisher, read};
    use angel_net::doc::{delta::diff, wire::Writer};
    let mut run = super::shots::small_works();
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Plan("sawpit"));
    stepped(&mut run, 40);
    let mut publisher = Publisher::default();
    let first = publisher.publish(&run).expect("the tick's tree");
    let guest = read(&first).expect("a guest reads it");
    assert_eq!(guest.home.industry, run.home.industry, "the kingdom crosses whole");
    assert_eq!(serde_json::to_value(&guest.kingdom.industry).unwrap(), serde_json::to_value(&run.kingdom.industry).unwrap(), "and every seat's builder");
    let (a, b) = (run.hill_ghost(1).expect("the host's ghost"), guest.hill_ghost(1).expect("the guest's"));
    assert_eq!((a.rect, a.verdict, a.note.clone()), (b.rect, b.verdict, b.note));
    let (hv, gv) = (view::of(&run).unwrap(), view::of(&guest).unwrap());
    assert_eq!((hv.buildings.len(), hv.roads.len(), hv.carts.len(), hv.folk.len()), (gv.buildings.len(), gv.roads.len(), gv.carts.len(), gv.folk.len()));
    // A second on, what the guest is sent is a patch: the cart, the people, the clock.
    stepped(&mut run, 1);
    let next = publisher.publish(&run).expect("the next tick's tree");
    let patch = diff(&first, &next).expect("a second changes something");
    let mut bytes = Vec::new();
    Writer::default().patch(&mut bytes, &patch);
    println!("a second of the small works to a guest: {} bytes", bytes.len());
    assert!(bytes.len() < 12_000, "{} bytes", bytes.len());
}

// ---- rollerways, swapes and rails by the builder's keys -------------------------------------

fn at_tile(run: &mut Run, seat: u32, tile: (i16, i16)) {
    let h = run.players.get_mut(&seat).unwrap();
    (h.x, h.y) = ((f32::from(tile.0) + 0.5) * TILE_UNITS, (f32::from(tile.1) + 0.5) * TILE_UNITS);
    still(run, 1);
}

/// Face a way without leaving the tile.
fn face(run: &mut Run, seat: u32, dir: crate::kingdom::Dir) {
    let (mx, my) = dir.delta();
    run.step(&keys(seat, Input { move_x: mx as i8, move_y: my as i8, ..Default::default() }));
    still(run, 1);
}

#[test]
fn walking_with_e_held_paints_a_run_of_rollers_each_facing_the_way_walked_and_x_takes_one_up() {
    use crate::kingdom::{Dir, Piece};
    let mut run = run(1);
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Roller(Piece::Roller));
    at_tile(&mut run, 1, (156, 104));
    face(&mut run, 1, Dir::E);
    // The ghost says what it is and which way it faces.
    let g = run.hill_ghost(1).expect("a ghost");
    assert_eq!(g.tool, Tool::Roller(Piece::Roller));
    assert_eq!(g.dir, Dir::E.index());
    assert_eq!(g.verdict, Verdict::Fits, "{}", g.note);
    assert!(g.note.contains("FACING EAST"), "{}", g.note);
    for _ in 0..200 {
        run.step(&keys(1, Input { move_x: 1, interact: true, ..Default::default() }));
        if tile_of(&run, 1).0 >= 164 {
            break;
        }
    }
    still(&mut run, 1);
    let cells: Vec<_> = run.home.industry.rollers.cells.iter().filter(|(t, _)| t.1 == 104).collect();
    assert!(cells.len() >= 6, "a run of it was laid: {}", cells.len());
    assert!(cells.iter().all(|(_, c)| c.dir == Dir::E && c.piece == Piece::Roller), "each faces the way the knight walked");
    // The tiles are the ground walked, in a row.
    let xs: Vec<i16> = cells.iter().map(|(t, _)| t.0).collect();
    assert!(xs.windows(2).all(|w| w[1] == w[0] + 1), "{xs:?}");
    // X takes the tile ahead up, not the building near it.
    let before = run.home.industry.rollers.cells.len();
    let ahead = (tile_of(&run, 1).0 + 1, 104);
    run.home.industry.rollers.cells.insert(ahead, crate::kingdom::RollerCell::new(Piece::Roller, Dir::E));
    let _ = ahead;
    press(&mut run, 1, Input { bomb: true, ..Default::default() });
    assert!(run.home.industry.rollers.cells.len() <= before, "a tile was taken up: {} from {before}", run.home.industry.rollers.cells.len());
    // Turning a corner: walking south turns the tile at the corner to face south.
    at_tile(&mut run, 1, (160, 104));
    for _ in 0..60 {
        run.step(&keys(1, Input { move_y: 1, interact: true, ..Default::default() }));
        if tile_of(&run, 1).1 >= 108 {
            break;
        }
    }
    still(&mut run, 1);
    let col: Vec<_> = run.home.industry.rollers.cells.iter().filter(|(t, _)| t.0 == 160 && t.1 > 104).collect();
    assert!(!col.is_empty() && col.iter().all(|(_, c)| c.dir == Dir::S), "the run turns south: {col:?}");
    assert!(run.home.industry.valid(catalog()));
}

#[test]
fn a_swape_is_raised_facing_the_way_the_builder_faces_and_p_turns_it() {
    use crate::kingdom::Dir;
    let mut run = run(1);
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Plan("swape"));
    at_tile(&mut run, 1, (156, 110));
    face(&mut run, 1, Dir::E);
    let g = run.hill_ghost(1).expect("a ghost");
    assert_eq!(g.dir, Dir::E.index());
    assert!(g.note.contains("FACING EAST"), "{}", g.note);
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    let id = run.home.industry.at((157, 110)).expect("a swape stands ahead");
    assert!(run.home.industry.buildings[&id].swape);
    assert_eq!(run.home.industry.buildings[&id].facing, Dir::E.index());
    // P turns the one under the cursor a quarter, clockwise: east to south.
    press(&mut run, 1, Input { cycle: true, ..Default::default() });
    assert_eq!(run.home.industry.buildings[&id].facing, Dir::S.index(), "{}", run.kingdom.industry[&1].message);
    // And with nothing under the cursor, P turns what is about to be laid.
    at_tile(&mut run, 1, (156, 114));
    face(&mut run, 1, Dir::E);
    let turn = run.kingdom.industry[&1].turn;
    press(&mut run, 1, Input { cycle: true, ..Default::default() });
    assert_eq!(run.kingdom.industry[&1].turn, (turn + 1) % 4);
    // Key 1 sets a swape to one good of those it stands between.
    assert!(run.home.industry.buildings[&id].filter.is_none());
}

#[test]
fn rail_bays_and_a_route_are_laid_by_the_builders_keys_and_a_wagon_runs_it() {
    use crate::kingdom::Dir;
    let mut run = run(1);
    let c = catalog();
    let k = &mut run.home.industry;
    k.apply(c, Command::Build { plan: "depot".into(), tile: (150, 108) }).expect("a depot");
    k.apply(c, Command::Build { plan: "depot".into(), tile: (166, 108) }).expect("another");
    for b in k.buildings.values_mut() {
        b.construction = 0;
    }
    let from = k.at((150, 108)).unwrap();
    k.buildings.get_mut(&from).unwrap().output.insert("plank".into(), 40);
    builder(&mut run, 1);
    // Rail: E on the tile north-east of the first depot, then walking east with E held.
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Rail);
    at_tile(&mut run, 1, (153, 107));
    face(&mut run, 1, Dir::S);
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert!(run.home.industry.rails.cells.contains_key(&(153, 108)), "{}", run.kingdom.industry[&1].message);
    at_tile(&mut run, 1, (153, 108));
    face(&mut run, 1, Dir::E);
    for _ in 0..300 {
        run.step(&keys(1, Input { move_x: 1, interact: true, ..Default::default() }));
        if tile_of(&run, 1).0 >= 165 {
            break;
        }
    }
    still(&mut run, 1);
    assert!(run.home.industry.rails.cells.len() >= 10, "{}", run.home.industry.rails.cells.len());
    assert!(run.home.industry.rails.cells.contains_key(&(165, 108)) || run.home.industry.rails.cells.contains_key(&(164, 108)));
    // Bays: the first beside the first depot, the second beside the other.
    let (a, b) = ((153, 108), (165, 108));
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Bay);
    for (stand, face_to, tile) in [((153, 107), Dir::S, a), ((165, 107), Dir::S, b)] {
        at_tile(&mut run, 1, stand);
        face(&mut run, 1, face_to);
        let g = run.hill_ghost(1).unwrap();
        assert_eq!(g.cursor, tile);
        press(&mut run, 1, Input { interact: true, ..Default::default() });
    }
    assert_eq!(run.home.industry.rails.bays.len(), 2, "{}", run.kingdom.industry[&1].message);
    // A route: E at the bay it loads at, E at the bay it unloads at.
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Route);
    at_tile(&mut run, 1, (153, 107));
    face(&mut run, 1, Dir::S);
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert!(run.kingdom.industry[&1].stop.is_some(), "{}", run.kingdom.industry[&1].message);
    at_tile(&mut run, 1, (165, 107));
    face(&mut run, 1, Dir::S);
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert_eq!(run.home.industry.rails.routes.len(), 1, "{}", run.kingdom.industry[&1].message);
    assert!(run.kingdom.industry[&1].stop.is_none());
    assert_eq!(run.home.industry.rails.wagons.len(), 1);
    // `.` sends another wagon on the route (the keys 1 to 3 are the hotbar's).
    at_tile(&mut run, 1, (153, 107));
    face(&mut run, 1, Dir::S);
    press(&mut run, 1, Input { skill: 3, ..Default::default() });
    assert_eq!(run.home.industry.rails.wagons.len(), 2, "{}", run.kingdom.industry[&1].message);
    press(&mut run, 1, Input { cast: 4, ..Default::default() });
    for _ in 0..30 * 90 {
        run.step(&keys(1, Input::default()));
    }
    let moved = run.home.industry.buildings[&run.home.industry.at((166, 108)).unwrap()].output.get("plank").copied().unwrap_or(0);
    assert!(moved >= 8, "the wagons carried planks: {moved}");
    assert!(run.home.industry.valid(c));
    // The view the renderers read has the rails, the bays and the wagons.
    let v = crate::world::hillside::of(&run).expect("a view");
    assert!(v.rails.len() >= 10 && v.rails.iter().filter(|r| r.bay.is_some()).count() == 2);
    assert_eq!(v.wagons.len(), 2);
    // `;` sells one again (the builder was put away: out again, at the bay, with the route tool).
    builder(&mut run, 1);
    run.kingdom.industry.get_mut(&1).unwrap().plan = tool_index(Tool::Route);
    at_tile(&mut run, 1, (153, 107));
    face(&mut run, 1, Dir::S);
    press(&mut run, 1, Input { skill: 4, ..Default::default() });
    assert_eq!(run.home.industry.rails.wagons.len(), 1, "{}", run.kingdom.industry[&1].message);
}
