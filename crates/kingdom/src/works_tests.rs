//! The hillside, its lines, castle sites, the Clerk, patterns and machines.
use super::*;
use std::collections::{BTreeSet, VecDeque};

pub(crate) fn hill() -> Kingdom {
    let mut k = Kingdom::default();
    k.found_hillside(catalog(), 7);
    k
}
/// A rich, fully chartered hillside for tests of one mechanism at a time.
/// The scenario test never uses this: it plays from founding stock.
pub(crate) fn rich() -> Kingdom {
    let mut k = hill();
    // Stocked, but with room in the camp for deliveries, and fodder for every
    // ox the test will hire.
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 40);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    k
}
pub(crate) fn build(k: &mut Kingdom, plan: &str, tile: Tile) -> u32 {
    k.apply(catalog(), Command::Build { plan: plan.into(), tile })
        .unwrap_or_else(|e| panic!("{plan} at {tile:?}: {e}"));
    k.at(tile).unwrap()
}
pub(crate) fn finish(k: &mut Kingdom) {
    for b in k.buildings.values_mut() {
        b.construction = 0;
    }
}
pub(crate) fn lay(k: &mut Kingdom, kind: LineKind, from: u32, to: u32, vehicles: u16) -> u32 {
    // Lines cost timber and planks; top the camp up for the purchase.
    let kept = k.stock.clone();
    for r in catalog().resources.keys() {
        if r != "fodder" {
            k.stock.insert(r.into(), 400);
        }
    }
    let paid = k.stock.clone();
    k.apply(
        catalog(),
        Command::Line { kind, from, to, path: Vec::new(), vehicles },
    )
    .unwrap_or_else(|e| panic!("{kind:?} line {from}->{to}: {e}"));
    // Give back the topping-up, keeping what the line cost.
    for (r, topped) in paid {
        let kept_n = kept.get(&r).copied().unwrap_or(0);
        if let Some(left) = k.stock.get_mut(&r) {
            *left = (*left).saturating_sub(topped.saturating_sub(kept_n));
        }
    }
    k.stock.retain(|_, n| *n > 0);
    *k.lines.keys().last().unwrap()
}
/// Joins `id` to the roads and makes sure a carter's yard with `carts` carts serves
/// them, as a player does before anything is carried. The camp is topped up for the
/// purchase and the topping-up given back.
pub(crate) fn cart(k: &mut Kingdom, id: u32, carts: u16) {
    let c = catalog();
    let kept = k.stock.clone();
    for r in c.resources.keys() {
        if r != "fodder" {
            k.stock.insert(r.into(), 400);
        }
    }
    let paid = k.stock.clone();
    let yard = match k.buildings.iter().find(|(_, b)| b.plan == "carters-yard") {
        Some((&y, _)) => y,
        None => {
            let y = build(k, "carters-yard", (143, 99));
            k.buildings.get_mut(&y).unwrap().construction = 0;
            k.apply(c, Command::Connect(y)).unwrap_or_else(|e| panic!("yard road: {e}"));
            y
        }
    };
    k.apply(c, Command::Connect(id)).unwrap_or_else(|e| panic!("road to {id}: {e}"));
    k.apply(c, Command::Carts { yard, carts }).unwrap_or_else(|e| panic!("carts: {e}"));
    for (r, topped) in paid {
        let kept_n = kept.get(&r).copied().unwrap_or(0);
        if let Some(left) = k.stock.get_mut(&r) {
            *left = (*left).saturating_sub(topped.saturating_sub(kept_n));
        }
    }
    k.stock.retain(|_, n| *n > 0);
}
pub(crate) fn run(k: &mut Kingdom, seconds: usize) {
    for _ in 0..seconds {
        k.advance_second(catalog());
    }
    assert!(k.valid(catalog()), "kingdom stayed valid");
}

fn reach(k: &Kingdom, from: Tile) -> BTreeSet<Tile> {
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some((x, y)) = queue.pop_front() {
        for n in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if k.world.contains(n) && k.world.terrain(n).walkable() && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

#[test]
fn the_hillside_is_the_realm_with_every_site_a_castle_needs() {
    let k = hill();
    assert!(k.valid(catalog()));
    assert_eq!(k.world.bounds(), (240, 176));
    assert_eq!(k.camp(), hillside::HILL_CAMP);
    for seed in [0, 1, 7, 99, u64::MAX] {
        let w = World::hillside(seed);
        assert!(w.valid());
        let sand = (0..176).flat_map(|y| (0..240).map(move |x| (x, y))).filter(|&p| w.deposit(p) == Some(Deposit::Sand)).count();
        assert!(sand >= 100, "seed {seed}: sand banks along the river ({sand})");
        // The hillside holds field stone only: the good rock is won below.
        let stone = (0..176).flat_map(|y| (0..240).map(move |x| (x, y))).filter(|&p| w.terrain(p) == Terrain::Stone).count();
        assert_eq!(stone, 24, "one small outcrop of field stone");
        let count = |t: Terrain| (0..176).flat_map(|y| (0..240).map(move |x| (x, y))).filter(|&p| w.terrain(p) == t).count();
        assert!(count(Terrain::Forest) > 600, "woodland");
        assert!(count(Terrain::ClayBank) > 30, "a clay bank");
        assert!(count(Terrain::River) > 400, "a river");
        // The river is one connected course from the top of the map to the foot.
        let river: Vec<Tile> = (0..176).map(|y| (hillside::river_column(seed, y), y)).collect();
        assert!(river.iter().all(|&t| w.terrain(t) == Terrain::River));
        assert!(river.windows(2).all(|p| (p[0].0 - p[1].0).abs() <= 1), "no jumps in the course");
        let tops = (0..176).flat_map(|y| (0..240).map(move |x| (x, y))).filter(|&p| w.hilltop(p)).count();
        assert!(tops > 60, "seed {seed}: a windy ridge to build on ({tops})");
    }
    // Every site is reachable on foot from the camp, and the Keep, its spawn
    // and the Delve's stair stand where the Tabula plan puts them.
    let reached = reach(&k, k.camp());
    for rect in [hillside::FIELDSTONE, hillside::CLAY] {
        assert!(reached.contains(&(rect.0, rect.1)) && reached.contains(&(rect.2, rect.3)));
    }
    for t in [(128, 88), hillside::HILL_GATE, hillside::KEEP_CENTRE, k.camp()] {
        assert!(reached.contains(&t), "{t:?} reachable");
    }
    // Nothing the works builds can sit on the camps or on the Delve's stair.
    let mut rich = rich();
    assert!(rich.apply(catalog(), Command::Build { plan: "cottage".into(), tile: hillside::HILL_GATE }).is_err());
    assert!(k.check_site(catalog(), "cottage", (130, 55)).is_err(), "a model house's camp");
    assert!(k.check_site(catalog(), "cottage", (137, 97)).is_ok());
}

#[test]
fn heights_slope_up_from_the_river_and_the_keep_stands_on_level_ground() {
    let k = hill();
    let w = &k.world;
    let plateau = hillside::KEEP_PLATEAU;
    for dy in -10..=10 {
        for dx in -10..=10 {
            assert_eq!(w.height((128 + dx, 78 + dy)), plateau, "the keep's plateau is level");
        }
    }
    // East of the river it climbs, the ridge above the castle.
    assert!(w.height((165, 66)) > plateau + 5, "the east is high ground");
    assert!(w.height((101, 100)) < plateau, "the river valley is the low line");
}

#[test]
fn old_saves_load_unchanged_and_keep_their_valley() {
    // Written by the code before the hillside existed: a 72 x 54 valley with a
    // forester, quarry, farm and cottage working.
    let saved = include_str!("../fixtures/old-save-valley-72x54.json");
    let k: Kingdom = serde_json::from_str(saved).expect("old save parses");
    let c = catalog();
    assert!(k.valid(c), "old save is valid under the new rules");
    assert_eq!(k.world.version, 1);
    assert_eq!(k.world.bounds(), (72, 54));
    assert_eq!(k.camp(), CAMP);
    assert!(k.lines.is_empty() && k.sites.is_empty());
    let mut a = k.clone();
    let mut b = k;
    for _ in 0..30 {
        a.work(c);
        b.work(c);
    }
    assert_eq!(a, b, "deterministic");
    assert!(a.valid(c));
    assert!(a.total("timber") > 0 && a.produced.get("timber").copied().unwrap_or(0) > 0);
    // And a new save round-trips with every new field present.
    let again: Kingdom = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert_eq!(again, a);
}

// ---- hauling on infrastructure lines -------------------------------------

/// A woodsman in the southern wood and a ropeway carrying its timber to a warehouse a dozen tiles off. (The
/// tramway this was written for is rails now; the line is the same machine and a ropeway is its long-haul
/// kind that is left.)
fn forester_line(k: &mut Kingdom, wagons: u16) -> (u32, u32) {
    let f = build(k, "woodsman-camp", (139, 97));
    let store = build(k, "depot", (124, 98));
    finish(k);
    let l = lay(k, LineKind::Rope, f, store, wagons);
    (f, l)
}

#[test]
fn a_line_has_capacity_and_time_you_can_read_and_goods_are_conserved() {
    let c = catalog();
    let mut k = rich();
    let (_f, l) = forester_line(&mut k, 1);
    let spec = &c.lines["rope"];
    let line = &k.lines[&l];
    assert!(line.length() >= 8 && line.length() <= 20, "length {}", line.length());
    let one_way = line.forward;
    assert_eq!(one_way, line.length() as u32 * u32::from(spec.speed), "a tile takes the wagon's speed");
    let cycle = line.cycle(spec);
    let per_min = line.capacity_per_min(spec);
    assert_eq!(per_min, u32::from(spec.capacity) * 60 / cycle, "capacity is wagons x capacity / cycle");
    let before_line = k.total("timber");
    run(&mut k, 300);
    let line = &k.lines[&l];
    assert!(line.stats.moved > 0, "timber arrived by rope");
    // One wagon can never beat its own capacity: a trip takes a cycle.
    let trips_possible = 300 / u64::from(cycle) + 1;
    assert!(line.stats.moved <= u64::from(spec.capacity) * trips_possible, "{} moved vs {} possible", line.stats.moved, u64::from(spec.capacity) * trips_possible);
    // Conservation: everything the forester made is in the camp, its buffer or a wagon.
    let made = k.produced["timber"];
    assert_eq!(k.total("timber"), before_line + made);
    assert!(k.lines[&l].stats.moved <= u64::from(made));
}

#[test]
fn a_producer_outrunning_its_line_backs_up_and_more_vehicles_clear_it() {
    let c = catalog();
    let mut k = rich();
    let (f, l) = forester_line(&mut k, 1);
    run(&mut k, 100);
    // Equal windows, the camp emptied before each so it can always take timber.
    k.stock.insert("timber".into(), 0);
    let start = k.lines[&l].stats.moved;
    run(&mut k, 200);
    let slow = k.lines[&l].stats.moved - start;
    let rows = k.line_report(c);
    assert_eq!(rows[0].state, LineState::Saturated, "one wagon is the limit: {rows:?}");
    assert!(rows[0].observed_per_min <= rows[0].capacity_per_min + 8);
    // Four timber every ten seconds is 24 a minute; one wagon cannot take it.
    assert!(k.buildings[&f].stats.backed_up > 100, "the forester is backed up: {:?}", k.buildings[&f].stats);
    // Fix it: build more wagons. The same line now clears the forester.
    k.stock.insert("timber".into(), 100);
    k.apply(c, Command::Fleet { line: l, vehicles: 24 }).unwrap();
    k.stock.insert("timber".into(), 0);
    let (start, was) = (k.lines[&l].stats.moved, k.buildings[&f].stats);
    run(&mut k, 200);
    let fast = k.lines[&l].stats.moved - start;
    let now = k.buildings[&f].stats;
    assert!(fast > slow * 2 / 1, "twenty-four buckets move far more than one: {fast} vs {slow}");
    assert!(now.worked - was.worked > 150, "the forester works almost all its time now: {was:?} -> {now:?}");
    assert_eq!(k.line_report(c)[0].state, LineState::Starved, "now the cause is upstream: the forester");
    // Selling vehicles is possible only for those waiting at the dock.
    k.apply(c, Command::Fleet { line: l, vehicles: 2 }).unwrap();
    assert!((2..24).contains(&k.lines[&l].fleet.len()), "only those waiting at the dock are sold: {}", k.lines[&l].fleet.len());
    assert!(k.valid(c));
}

#[test]
fn a_depot_is_a_junction_with_berths_and_a_buffer_that_can_choke_it() {
    let c = catalog();
    let mut k = rich();
    // Two producers into one warehouse, the warehouse out to the camp by one line.
    let f1 = build(&mut k, "woodsman-camp", (139, 97));
    let f2 = build(&mut k, "woodsman-camp", (143, 97));
    let depot = build(&mut k, "depot", (122, 99));
    finish(&mut k);
    let a = lay(&mut k, LineKind::Rope, f1, depot, 4);
    let b = lay(&mut k, LineKind::Rope, f2, depot, 4);
    // Out of the warehouse a single wagon is the bottleneck.
    let out = lay(&mut k, LineKind::Rope, depot, 1, 1);
    // The camp is emptied as it fills so it can always take timber: the wagon is the limit.
    for _ in 0..9 {
        k.stock.insert("timber".into(), 0);
        run(&mut k, 100);
    }
    k.stock.insert("timber".into(), 0);
    let store = k.buildings[&depot].output.get("timber").copied().unwrap_or(0);
    // Full, less what its inbound wagons are holding in the road.
    assert!(store >= 150, "the warehouse filled to its buffer: {store}");
    let rows = k.line_report(c);
    let into: Vec<_> = rows.iter().filter(|r| r.id == a || r.id == b).collect();
    assert!(into.iter().any(|r| r.state == LineState::Backed), "inflow backs up behind the warehouse: {rows:?}");
    assert_eq!(rows.iter().find(|r| r.id == out).unwrap().state, LineState::Saturated);
    let dock = &k.lines[&a].stats;
    assert!(dock.queued > 0, "wagons queued at the full warehouse");
    // The cure is on the outbound side: more wagons out, and the warehouse drains.
    k.stock.insert("timber".into(), 200);
    k.apply(c, Command::Fleet { line: out, vehicles: 8 }).unwrap();
    let before = k.buildings[&depot].output["timber"];
    // The camp has been filling too: spend its timber so there is room.
    k.stock.insert("timber".into(), 10);
    run(&mut k, 60);
    assert!(k.buildings[&depot].output.get("timber").copied().unwrap_or(0) < before, "the warehouse drains");
}

#[test]
fn berths_limit_a_dock_however_many_wagons_wait() {
    let c = catalog();
    // One producer, one berth: its wagons take turns however many there are.
    let mut k = rich();
    let f = build(&mut k, "woodsman-camp", (139, 97));
    finish(&mut k);
    let l = lay(&mut k, LineKind::Rope, f, 1, 8);
    run(&mut k, 600);
    let line = &k.lines[&l];
    let spec = &c.lines["rope"];
    // A berth holds a wagon for `dwell` seconds, so a single-berth dock serves
    // at most capacity / dwell units a second, whatever the fleet.
    let ceiling = 600 * u64::from(spec.capacity) / u64::from(spec.dwell) + 8;
    assert!(line.stats.moved <= ceiling, "{} vs dock ceiling {ceiling}", line.stats.moved);
    assert!(line.stats.moved >= 50, "but it does move: {}", line.stats.moved);
}

#[test]
fn demolishing_a_building_takes_its_lines_down_and_keeps_every_unit() {
    let c = catalog();
    let mut k = rich();
    let (f, l) = forester_line(&mut k, 3);
    run(&mut k, 120);
    let total = k.total("timber");
    k.apply(c, Command::Demolish(k.buildings[&f].tile)).unwrap();
    assert!(!k.lines.contains_key(&l), "its line went with it");
    // Half the forester's cost is salvaged; the rest of timber is only what it was.
    assert!(k.total("timber") >= total, "cargo was returned to the camp");
    assert!(k.valid(c));
}

#[test]
fn identical_runs_and_reloaded_saves_agree_with_lines_in_flight() {
    let c = catalog();
    let mut a = rich();
    forester_line(&mut a, 3);
    run(&mut a, 70);
    let saved = serde_json::to_string(&a).unwrap();
    let mut b: Kingdom = serde_json::from_str(&saved).unwrap();
    assert_eq!(a, b);
    run(&mut a, 200);
    run(&mut b, 200);
    assert_eq!(a, b, "a reloaded kingdom replays identically");
    assert!(a.lines.values().any(|l| l.fleet.iter().any(|v| v.amount > 0)) || a.valid(c));
}

// ---- castle sites ------------------------------------------------------

const YARD: Tile = (150, 84);

fn founded_site() -> (Kingdom, u32) {
    let c = catalog();
    let mut k = rich();
    k.apply(c, Command::FoundSite { plan: "great-tower".into(), centre: hillside::KEEP_CENTRE, yard: YARD })
        .unwrap();
    finish(&mut k);
    (k, 1)
}
fn yard_of(k: &Kingdom, site: u32) -> u32 {
    k.sites[&site].yard
}
fn stock_yard(k: &mut Kingdom, site: u32, goods: &[(&str, u32)]) {
    let y = yard_of(k, site);
    for (r, n) in goods {
        *k.buildings.get_mut(&y).unwrap().input.entry((*r).into()).or_default() += n;
    }
}

#[test]
fn a_site_covers_the_keeps_ground_and_keeps_the_way_down_inside_it() {
    let (k, s) = founded_site();
    let site = &k.sites[&s];
    assert_eq!(site.rect(), (119, 67, 146, 94));
    assert!(site.covers((128, 88)) && site.covers(hillside::HILL_GATE), "spawn and Delve stair are inside the bailey");
    assert!(!site.covers(k.camp()));
    assert!(k.valid(catalog()));
    let mut k = k;
    // The footprint is the castle's: nothing is built or laid in it.
    assert!(k.apply(catalog(), Command::Build { plan: "cottage".into(), tile: (120, 70) }).is_err());
    assert!(k.apply(catalog(), Command::Road((120, 70))).is_err());
    // Nor a second castle on top, nor the yard inside the walls.
    assert!(k.apply(catalog(), Command::FoundSite { plan: "shell-keep".into(), centre: (130, 80), yard: (60, 60) }).is_err());
    // The stockyard stays as long as the castle does.
    assert!(k.apply(catalog(), Command::Demolish(YARD)).is_err());
}

#[test]
fn earth_is_dug_first_then_poles_gate_the_masons_and_the_bill_is_paid_stone_by_stone() {
    let c = catalog();
    let (mut k, s) = founded_site();
    // Stage 0 is the ditch: stone cannot be set, only earth moved.
    assert_eq!(k.site_allowance(c, s, 10).hold, Hold::Earthwork);
    let need = site::stage_needs(c, &c.castles["great-tower"].stages[0]);
    assert_eq!(need["earth"], 135, "671.1 m3 at five to a load");
    assert_eq!(k.earth_may_dig(c, s), 0, "no spoil in the yard");
    stock_yard(&mut k, s, &[("earth", 135)]);
    assert_eq!(k.site_dig(c, s, 140).unwrap_err(), "there is earth for 135, not 140");
    k.site_dig(c, s, 100).unwrap();
    assert_eq!(k.sites[&s].stage, 0, "not yet");
    k.site_dig(c, s, 35).unwrap();
    assert_eq!(k.sites[&s].stage, 1);
    assert!(k.buildings[&yard_of(&k, s)].input.get("earth").is_none(), "the spoil was used");
    // Lift 1 needs scaffold, and wants the goods of its bill.
    assert_eq!(k.site_allowance(c, s, 10).hold, Hold::Scaffold { have: 0, need: 24 });
    stock_yard(&mut k, s, &[("pole", 24), ("centring", 6)]);
    assert_eq!(k.site_allowance(c, s, 10).hold, Hold::Short { resource: "ashlar".into() }.clone());
    let lift = &c.castles["great-tower"].stages[1];
    let needs = site::stage_needs(c, lift);
    assert_eq!(lift.count, 975);
    assert_eq!(needs["voussoir"], 29, "one in thirty-two of 949 cut stone");
    stock_yard(&mut k, s, &[("ashlar", 100), ("voussoir", 29), ("quoin", 14), ("rubble", 134), ("mortar", 480), ("joist", 5)]);
    // 100 ashlar pays for 105 stones of a stage that wants 920 ashlar in 975.
    let a = k.site_allowance(c, s, 1000);
    assert_eq!(a, Allowance { stones: 105, hold: Hold::Short { resource: "ashlar".into() } });
    // Ten masons are slower than that: 12 stones with the half carried over.
    assert_eq!(k.site_allowance(c, s, 10), Allowance { stones: 12, hold: Hold::Pace });
    // Masons set the pace: 4 masons set 5 stones a shift, with the carry.
    stock_yard(&mut k, s, &[("ashlar", 1000)]);
    let a = k.site_allowance(c, s, 4);
    assert_eq!(a, Allowance { stones: 5, hold: Hold::Pace });
    let before = k.buildings[&yard_of(&k, s)].input.clone();
    k.site_set(c, s, 5, 4).unwrap();
    assert_eq!(k.sites[&s].laid, 5);
    let after = &k.buildings[&yard_of(&k, s)].input;
    for (r, n) in &needs {
        let used = before.get(r).copied().unwrap_or(0) - after.get(r).copied().unwrap_or(0);
        assert_eq!(used as u64, (u64::from(*n) * 5).div_ceil(975), "{r} paid proportionally");
    }
    // Cannot set more than allowed.
    assert!(k.site_set(c, s, 50, 4).is_err());
    assert_eq!(k.sites[&s].laid, 5);
}

#[test]
fn every_stage_costs_exactly_its_bill_whatever_the_shifts_do() {
    let c = catalog();
    for plan in ["motte-and-bailey", "shell-keep", "great-tower"] {
        let castle = &c.castles[plan];
        let mut k = rich();
        k.apply(c, Command::FoundSite { plan: plan.into(), centre: hillside::KEEP_CENTRE, yard: YARD }).unwrap();
        finish(&mut k);
        let y = yard_of(&k, 1);
        let mut need = Stock::new();
        for st in &castle.stages {
            for (r, n) in site::stage_needs(c, st) {
                *need.entry(r).or_default() += n;
            }
        }
        let mut paid = Stock::new();
        let mut guard = 0;
        while !k.site_finished(c, 1) {
            guard += 1;
            assert!(guard < 200_000, "{plan} stuck at {:?}", k.sites[&1]);
            // Top every good up to a full yard, as a depot with endless carts would.
            for r in need.keys() {
                let have = k.buildings[&y].input.get(r).copied().unwrap_or(0);
                *k.buildings.get_mut(&y).unwrap().input.entry(r.clone()).or_default() += 480 - have;
            }
            k.buildings.get_mut(&y).unwrap().input.insert("pole".into(), 30);
            k.buildings.get_mut(&y).unwrap().input.insert("centring".into(), 8);
            let before = k.buildings[&y].input.clone();
            if k.earth_may_dig(c, 1) > 0 {
                k.site_dig(c, 1, k.earth_may_dig(c, 1)).unwrap();
            } else {
                // A different crew size every shift, down to a single mason.
                let masons = 1 + (guard % 9) * 17;
                let a = k.site_allowance(c, 1, masons);
                k.site_set(c, 1, a.stones, masons).unwrap();
            }
            let after = &k.buildings[&y].input;
            for (r, n) in &before {
                if !matches!(r.as_str(), "pole" | "centring") {
                    *paid.entry(r.clone()).or_default() += n - after.get(r).copied().unwrap_or(0);
                }
            }
        }
        paid.retain(|_, n| *n > 0);
        // Earth is dug from the yard too; the totals are exactly the bill.
        assert_eq!(paid, need, "{plan}: paid exactly what the stages needed");
        assert_eq!(k.sites[&1].laid, castle.stones());
    }
}

#[test]
fn the_great_towers_bill_as_chain_goods() {
    let c = catalog();
    let castle = &c.castles["great-tower"];
    let mut need = Stock::new();
    for st in &castle.stages {
        for (r, n) in site::stage_needs(c, st) {
            *need.entry(r).or_default() += n;
        }
    }
    // Every line of the bill has a chain: each of these is made by a building.
    for (r, lo, hi) in [
        ("ashlar", 10_000, 10_200), ("voussoir", 300, 340), ("quoin", 553, 553), ("rubble", 1620, 1640),
        ("mortar", 5280, 5310), ("joist", 200, 240), ("shingle", 30, 60), ("cramp", 72, 80),
        ("lead-sheet", 366, 375), ("slate", 94, 100), ("earth", 135, 135), ("portcullis", 1, 1),
        ("hinge", 6, 6), ("glass", 12, 12), ("nails", 48, 48),
    ] {
        let n = need.get(r).copied().unwrap_or(0);
        assert!((lo..=hi).contains(&n), "{r}: {n} not in {lo}..={hi}");
        assert!(
            c.plans.values().any(|p| p.recipes.iter().any(|x| c.recipes[x].outputs.contains_key(r))),
            "{r} has a building that makes it"
        );
    }
    assert_eq!(castle.stones(), 11_266);
    // Stages run in lay order and tile the castle exactly.
    assert!(castle.validate().is_ok());
}
