//! The Keep's wall on the production sim: the seams `works_seam` makes between
//! the Works' hands and the hillside kingdom, each tested on a run whose
//! industry is the hillside, the Keep staked in it from the game's wall.
use super::*;
use crate::kingdom::{Command, Stock, catalog, site::stage_needs};
use std::collections::BTreeMap;

fn idle() -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

/// A rich hillside, as the sim's own tests keep one: stock in the camp, every
/// charter researched, fodder for the oxen.
fn rich(run: &mut Run) {
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 40);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
}

/// A run at home with the hillside founded, rich, and the Keep staked from the game's wall.
fn live() -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    rich(&mut run);
    for _ in 0..u64::from(HZ) * 12 {
        run.step(&idle());
    }
    assert!(run.keep_live().is_some(), "the Keep is staked: {:?}", run.hands.seen.note);
    // The stockyard stands (the sim's tests finish what they build, too).
    for b in run.home.industry.buildings.values_mut() {
        b.construction = 0;
    }
    run
}

fn yard_of(run: &Run) -> u32 {
    let id = run.keep_live().expect("live");
    run.home.industry.sites[&id].yard
}

/// Lay `goods` in the Keep's yard, as the carts would.
fn stock_yard(run: &mut Run, goods: &[(&str, u32)]) {
    let y = yard_of(run);
    for (r, n) in goods {
        *run.home.industry.buildings.get_mut(&y).expect("the yard").input.entry((*r).into()).or_default() += n;
    }
}

/// The goods for the stage in hand, all of it, and the scaffold's.
fn stock_stage(run: &mut Run) {
    let id = run.keep_live().expect("live");
    let c = catalog();
    let site = &run.home.industry.sites[&id];
    let needs: Stock = stage_needs(c, &c.castles[&site.plan].stages[usize::from(site.stage)]);
    let goods: Vec<(String, u32)> = needs.into_iter().collect();
    for (r, n) in goods {
        stock_yard(run, &[(&r, n)]);
    }
    stock_yard(run, &[("pole", 30), ("centring", 8)]);
}

fn laid(run: &Run) -> u32 {
    run.sites.keep().map_or(0, |s| s.laid)
}

#[test]
fn the_keep_is_staked_in_the_sim_from_the_games_wall() {
    let run = live();
    let id = run.keep_live().expect("live");
    let site = &run.home.industry.sites[&id];
    assert_eq!(site.plan, "great-tower");
    assert_eq!(site.laid, laid(&run));
    assert_eq!(site.laid, super::site::START);
    // The stage in hand is the one the game's wall is in: the second lift.
    assert_eq!(catalog().castles["great-tower"].stages[usize::from(site.stage)].id, "03-keep-lift-2");
    assert!(run.home.industry.valid(catalog()));
    // The wall the game shows moves the sim's: a developer's hook, a chronicle, a lift.
    let mut run = run;
    run.lay_to("great-tower", 3_000);
    for _ in 0..u64::from(HZ) * 2 {
        run.step(&idle());
    }
    assert_eq!(run.home.industry.sites[&id].laid, 3_000);
    assert!(run.home.industry.valid(catalog()));
}

/// A lodge beside the Keep's yard, finished and joined to the roads.
fn lodge(run: &mut Run) -> u32 {
    let k = &mut run.home.industry;
    for g in ["timber", "plank", "stone", "block"] {
        k.stock.insert(g.into(), 100);
    }
    k.apply(catalog(), Command::Build { plan: "lodge".into(), tile: (150, 90) }).expect("a lodge");
    let id = k.at((150, 90)).expect("built");
    k.buildings.get_mut(&id).expect("built").construction = 0;
    k.apply(catalog(), Command::Connect(id)).expect("a road to the lodge");
    id
}

/// The stage in hand's goods in the yard, as many as a yard holds.
fn stock_stage_capped(run: &mut Run) {
    let id = run.keep_live().expect("live");
    let c = catalog();
    let site = &run.home.industry.sites[&id];
    let needs = stage_needs(c, &c.castles[&site.plan].stages[usize::from(site.stage)]);
    for (r, n) in needs {
        stock_yard(run, &[(&r, n.min(480))]);
    }
    stock_yard(run, &[("pole", 30), ("centring", 8)]);
}

#[test]
fn the_hired_masons_set_what_the_yard_pays_for_and_the_stones_stand_in_the_games_wall_as_the_realms() {
    let mut run = live();
    lodge(&mut run);
    stock_stage_capped(&mut run);
    run.hands.yard.timber = 4_000;
    let id = run.keep_live().expect("live");
    let (before, yard) = (laid(&run), run.home.industry.buildings[&yard_of(&run)].input.clone());
    for _ in 0..u64::from(HZ) * 90 {
        run.step(&idle());
    }
    let set = laid(&run) - before;
    assert!(set >= 10, "the masons set stones in a minute and a half: {set}");
    // The sim's wall and the game's are the same wall, and the stones are the realm's own.
    assert_eq!(run.home.industry.sites[&id].laid, laid(&run));
    let site = run.sites.keep().expect("the Keep");
    assert!(site.tally().iter().any(|&(seat, n)| seat == 0 && n >= before + set), "{:?}", site.tally());
    // They were paid for out of the yard: stone, mortar, the rest, in the bill's proportion.
    let after = &run.home.industry.buildings[&yard_of(&run)].input;
    assert!(after["ashlar"] < yard["ashlar"] && after["mortar"] < yard["mortar"], "{after:?} against {yard:?}");
    assert!(run.home.industry.valid(catalog()));
    // The Clerk of Works speaks from the sim.
    assert!(run.keep_clerk().is_some());
}

#[test]
fn the_masons_wait_when_the_yard_is_dry_and_the_clerk_says_what_for() {
    let mut run = live();
    lodge(&mut run);
    stock_stage_capped(&mut run);
    run.hands.yard.timber = 4_000;
    let y = yard_of(&run);
    run.home.industry.buildings.get_mut(&y).expect("yard").input.insert("ashlar".into(), 0);
    let before = laid(&run);
    for _ in 0..u64::from(HZ) * 40 {
        run.step(&idle());
    }
    assert_eq!(laid(&run), before, "no ashlar, no stones");
    assert!(run.yard_holds(1).is_some_and(|w| w.contains("ASHLAR")), "{:?}", run.yard_holds(1));
    assert!(run.keep_clerk().is_some_and(|s| s.contains("ASHLAR")), "{:?}", run.keep_clerk());
}

#[test]
fn hired_hands_are_the_sims_people_at_their_posts_by_trade() {
    use super::trades::Trade;
    let mut run = live();
    let lodge = lodge(&mut run);
    stock_stage_capped(&mut run);
    for _ in 0..u64::from(HZ) * 30 {
        run.step(&idle());
    }
    let crew = u32::from(run.home.industry.report(catalog()).population.assigned.get(&lodge).copied().unwrap_or(0));
    assert!(crew > 0, "the lodge is staffed");
    assert_eq!(run.hired_hands(Trade::Setter), crew);
    // Not a stub's number: a trade nobody is staffed in has none.
    assert_eq!(run.hired_hands(Trade::Bargeman), 0);
    // Without the sim live the crew is what it was.
    let old = Run::at_home(41, 1, None, Default::default(), Default::default());
    assert_eq!(old.hired_hands(Trade::Setter), super::hands::hired(Trade::Setter));
}

#[test]
fn the_stockyards_tier_is_the_great_shafts_real_limit_on_what_a_claim_sends_up() {
    use super::stockyard::Tier;
    let mut a = live();
    let c = catalog();
    a.home.industry.stock.insert("limestone".into(), 0);
    for g in ["timber", "plank", "stone"] {
        a.home.industry.stock.insert(g.into(), 100);
    }
    let claim = crate::kingdom::Claim {
        resource: "limestone".into(),
        name: "Sound limestone face".into(),
        source: "The Mines / floor 3".into(),
        secured: false,
        rate: 4,
        yields: [("limestone".to_string(), 4), ("iron-ore".to_string(), 1)].into(), surface: false };
    assert!(a.home.industry.discover(c, "mines-3".into(), claim));
    assert!(a.home.industry.secure("mines-3"));
    a.home.industry.apply(c, Command::Build { plan: "claim-quarry".into(), tile: (150, 97) }).expect("a quarry");
    let q = a.home.industry.at((150, 97)).expect("built");
    a.home.industry.buildings.get_mut(&q).expect("built").construction = 0;
    a.home.industry.apply(c, Command::Link { building: q, claim: "mines-3".into() }).expect("linked");
    a.home.industry.apply(c, Command::Connect(q)).expect("a road");
    let mut b = a.clone();
    let start = a.home.industry.produced.get("limestone").copied().unwrap_or(0);
    assert_eq!(a.home.stockyard.tier, Tier::Hand);
    for _ in 0..u64::from(HZ) * 300 {
        a.step(&idle());
    }
    let hand = a.home.industry.produced.get("limestone").copied().unwrap_or(0) - start;
    // The same quarry under a ley lift: the lift in the sim is the tier, one scale.
    let lift = b.shaft_lift().expect("the lift").0;
    b.home.industry.buildings.get_mut(&lift).expect("the lift").plan = "shaft-ley".into();
    for _ in 0..u64::from(HZ) * 300 {
        b.step(&idle());
    }
    let ley = b.home.industry.produced.get("limestone").copied().unwrap_or(0) - start;
    assert_eq!(a.home.stockyard.tier, Tier::Hand);
    assert_eq!(b.home.stockyard.tier, Tier::LeyLift, "the tier follows the lift");
    assert_eq!(a.home.industry.shaft_limit, None);
    assert_eq!(a.shaft_throughput(), Tier::Hand.throughput());
    assert!(hand > 0, "a batch gets up even by hand carry: {hand}");
    assert!(hand * 3 < ley, "the tier is what limits it: {hand} by hand, {ley} by ley lift");
}

#[test]
fn mortar_at_the_tub_is_the_yards_own_and_goes_off() {
    let mut run = live();
    stock_yard(&mut run, &[("mortar", 300)]);
    assert_eq!(run.tub_stock(), 300 + run.home.industry.yard_stock(run.keep_live().expect("live"), "mortar") - 300);
    let before = run.tub_stock();
    for _ in 0..u64::from(HZ) * 600 {
        run.step(&idle());
    }
    assert!(run.tub_stock() < before, "mortar goes off: {} of {before} left after ten minutes", run.tub_stock());
    assert!(run.home.industry.spoiled.get("mortar").copied().unwrap_or(0) > 0);
    // Without the sim the tub is the stub's.
    let old = Run::at_home(41, 1, None, Default::default(), Default::default());
    assert_eq!(old.tub_stock(), old.hands.yard.tub);
}

#[test]
fn a_block_set_down_at_the_yard_is_the_camps_rock_and_a_bench_may_cut_it() {
    let mut run = live();
    for r in ["limestone", "freestone", "flint", "basalt", "tufa", "voidstone", "stone"] {
        run.home.industry.stock.insert(r.into(), 0);
    }
    assert!(!run.rough_ready());
    let hauled = run.home.stockyard.hauled;
    assert_eq!(run.stock_rough("limestone", 2), 2);
    assert_eq!(run.home.industry.amount("limestone"), 2);
    assert!(run.rough_ready());
    assert_eq!(run.yard_view().count("limestone"), 2, "the picture of the yard is the camp's rock");
    // The hillside's field stone is the sim's `stone`.
    assert_eq!(run.stock_rough(super::geology::RUBBLE, 3), 3);
    assert_eq!(run.home.industry.amount("stone"), 3);
    assert_eq!(run.home.stockyard.hauled, hauled + 5);
    assert!(run.home.stockyard.valid());
}

/// What the first chain makes: the goods the wall's bill is paid in, and the lime and timber work behind them.
const MADE: [&str; 9] = ["ashlar", "quoin", "voussoir", "rubble", "mortar", "joist", "pole", "centring", "template"];

/// The scripted run, from a fresh save: a face is claimed, the first chain is
/// raised by commands, and the hired masons set stones from goods the sim made.
/// Prints the run's story. Returns the run and the second the first stone stood.
fn first_stone_scripted(print: bool) -> (Run, u64) {
    use super::{geology, industry};
    let c = catalog();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(c, 7);
    // The founding train, topped up with materials to build with and every charter
    // researched; nothing the chain makes is in the camp.
    let k = &mut run.home.industry;
    for (g, n) in [("timber", 600), ("plank", 150), ("stone", 200), ("block", 20), ("coal", 80), ("fodder", 100_000)] {
        k.stock.insert(g.into(), n);
    }
    k.researched = c.all_techs().map(|(k, _)| k.clone()).collect();
    for g in MADE.iter().chain(["limestone", "quicklime", "lime-putty", "sand", "beam"].iter()) {
        assert_eq!(k.amount(g), 0, "{g} is made, not given");
    }
    // The Delve's gold, as the realm banks it: coin for the teachers.
    assert!(k.receive(c, "coin", 300) > 0);
    // 1. A face is won. The seam is the Mines' third floor, as the game's geology has it.
    let seam = geology::seam_at(Pack::Cavern, 3);
    let face = industry::Deposit { id: "1ff-3-1-limestone".into(), x: 0.0, y: 0.0, clear: true, resource: seam.stone(), name: seam.name(), seam };
    assert!(run.discover_frontier_claim(&face));
    assert!(run.home.industry.secure(&face.id), "the guards have fallen and the charter is signed");
    // 2. The Keep is staked in the sim from the game's wall.
    for _ in 0..u64::from(HZ) * 12 {
        run.step(&idle());
    }
    assert!(run.keep_live().is_some(), "{:?}", run.hands.seen.note);
    // The falsework's bays stand ahead of the masons: the carpenters' job, still the game's.
    run.hands.yard.timber = 4_000;
    // 3. The first chain, by commands alone.
    let mut chain = crate::kingdom::first_chain::raise(c, &mut run.home.industry, &face.id).expect("the chain is raised");
    let start = laid(&run);
    let mut first = None;
    // 4. Time passes: the sim runs a second at a time, the foreman between jobs.
    for second in 0..(5 * 3600u64) {
        for _ in 0..HZ {
            run.step(&idle());
        }
        chain.tend(c, &mut run.home.industry);
        if print && second % 120 == 0 {
            let k = &run.home.industry;
            let made = |g: &str| k.produced.get(g).copied().unwrap_or(0);
            println!(
                "t{second:>5}: {} | tier {} lifts {} a minute | up the shaft {} limestone | made ashlar {} mortar {} pole {} centring {} | carts {} made {} trips, {} kg | laid {}",
                run.keep_clerk().unwrap_or_default().to_lowercase(),
                run.home.stockyard.tier.name(),
                run.home.stockyard.tier.throughput(),
                made("limestone"),
                made("ashlar"),
                made("mortar"),
                made("pole"),
                made("centring"),
                k.road_report(c).carts,
                k.roadwork.trips,
                k.roadwork.kilos,
                laid(&run)
            );
        }
        if laid(&run) > start && first.is_none() {
            first = Some(second);
            if print {
                println!(
                    "first stone, second {second}: the sim's people at their posts {:?}; the game reads {:?}",
                    run.home.industry.staffed_by_trade(c),
                    run.hands.seen.staffed
                );
            }
        }
        if first.is_some_and(|f| second >= f + 120) {
            break;
        }
    }
    let first = first.expect("the first stone stands within five hours");
    if print {
        println!("the first stone stood at {first} s; laid {} (from {start})", laid(&run));
        let k = &run.home.industry;
        println!(
            "two minutes on, at their posts: {:?}; the sim's wall stands at {} (from {start})",
            run.home.industry.staffed_by_trade(c),
            k.sites.values().map(|s| s.laid).max().unwrap_or(0)
        );
    }
    (run, first)
}

#[test]
fn a_fresh_run_wins_a_face_raises_the_first_chain_by_commands_and_the_hired_masons_set_stones_the_sim_made() {
    let (run, first) = first_stone_scripted(true);
    let c = catalog();
    let id = run.keep_live().expect("live");
    let k = &run.home.industry;
    // The stones are the realm's masons': the realm's mark, in the game's wall, the sim's count the same.
    let site = run.sites.keep().expect("the Keep");
    assert!(laid(&run) > super::site::START);
    assert_eq!(k.sites[&id].laid, laid(&run));
    assert!(site.tally().iter().any(|&(seat, n)| seat == 0 && n > super::site::START), "{:?}", site.tally());
    // What the stones were paid in was made by buildings of the chain: stone, mortar, the scaffold.
    // (The bill's earlier share of a stage the Keep is partway through is paid already.)
    for g in ["ashlar", "mortar", "pole", "centring"] {
        assert!(k.produced.get(g).copied().unwrap_or(0) > 0, "{g} was made (first stone at {first} s)");
    }
    // The rock came up the Great Shaft at the lift's one scale, the hand lift's three a minute.
    assert_eq!(k.shaft_limit, None);
    assert_eq!(k.report(c).shaft, run.home.stockyard.tier.throughput());
    let up = k.produced.get("limestone").copied().unwrap_or(0);
    let minutes = (first / 60).max(1) as u32;
    assert!(up <= run.home.stockyard.tier.throughput() * (minutes + 3), "{up} blocks of limestone in {minutes} minutes");
    assert!(first > 600, "the lime must be burned and slaked first: {first}");
    // Two minutes on the setters and the lime-burner are still at their posts, idle where goods are
    // waited for, not at home.
    let at = run.home.industry.staffed_by_trade(c);
    assert!(at.get("setter").copied().unwrap_or(0) > 0 && at.get("lime-burner").copied().unwrap_or(0) > 0, "{at:?}");
    assert!(k.valid(c));
}

const NEEDS_TABULA: &str = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib works_seam -- --include-ignored";

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib works_seam -- --include-ignored"]
fn a_knights_set_stone_is_paid_from_the_same_yard_as_the_masons_and_laid_in_order() {
    let _ = NEEDS_TABULA;
    let mut run = live();
    stock_stage_capped(&mut run);
    run.hands.yard.timber = 4_000;
    let y = yard_of(&run);
    let (l, before) = (laid(&run), run.home.industry.buildings[&y].input.clone());
    // A knight (seat 1) has set the next stone with a true set: it is bedded, and the wall lays it.
    run.hands.yard.done.insert(l, (1, true));
    run.step(&idle());
    assert_eq!(laid(&run), l + 1);
    assert_eq!(run.home.industry.sites[&run.keep_live().expect("live")].laid, l + 1, "the sim's wall is the game's");
    let site = run.sites.keep().expect("the Keep");
    assert_eq!(site.setter(l), 1, "the knight's mark");
    assert!(site.is_true(l));
    let after = &run.home.industry.buildings[&y].input;
    let total = |g: &BTreeMap<String, u32>| g.values().sum::<u32>();
    assert!(total(after) < total(&before), "the bill was paid from the yard: {after:?} against {before:?}");
    // With the yard dry the stone cannot be laid: it is lifted again, and says why.
    let l = laid(&run);
    run.home.industry.buildings.get_mut(&y).expect("yard").input.insert("ashlar".into(), 0);
    run.hands.yard.course = 0;
    run.hands.yard.mortar = 0;
    run.hands.yard.done.insert(l, (1, true));
    run.step(&idle());
    assert_eq!(laid(&run), l, "an unpaid stone is not laid");
    assert!(run.hands.yard.done.is_empty() && run.hands.yard.course == 1 && run.hands.yard.mortar == 1);
    assert!(run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.contains("COULD NOT PAY")), "{:?}", run.hands.seats.get(&1).map(|h| h.said.clone()));
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib works_seam -- --include-ignored"]
fn a_knight_at_the_course_is_told_what_the_yard_lacks_before_the_set_begins() {
    use super::hands::{self, Work};
    use super::trades::Trade;
    let mut run = live();
    run.hands.yard.timber = 4_000;
    run.hands.yard.course = 3;
    run.hands.yard.mortar = 3;
    // No stage's goods in the yard: nothing to set a stone from.
    let castle = run.sites.keep().and_then(|s| s.castle()).expect("the great tower");
    let at = hands::stone_at(castle, laid(&run));
    run.teach_trade(1, Trade::Setter, 34);
    let (x, y) = hands::units([at[0], at[1] - 2.6]);
    let h = run.players.get_mut(&1).expect("a knight");
    (h.x, h.y) = (x, y);
    h.stone = false;
    h.idle = false;
    for tick in 0..8 {
        let swing = super::Input { swing: tick % 2 == 0, ..Default::default() };
        run.step(&BTreeMap::from([(1, swing)]));
    }
    assert!(run.hands.seats[&1].work.is_none(), "no set begins on an empty yard");
    assert!(run.hands.seats[&1].said.as_ref().is_some_and(|(_, t)| t.starts_with("WAITING ON")), "{:?}", run.hands.seats[&1].said);
    // With the yard stocked the same swings begin a set.
    stock_stage_capped(&mut run);
    for tick in 0..8 {
        let swing = super::Input { swing: tick % 2 == 0, ..Default::default() };
        run.step(&BTreeMap::from([(1, swing)]));
    }
    assert!(matches!(run.hands.seats[&1].work, Some(Work::Set { .. })), "{:?}", run.hands.seats[&1].work);
}


#[test]
fn the_falsework_is_one_system_the_sims_poles_in_the_yard() {
    let mut run = live();
    let id = run.keep_live().expect("live");
    let c = catalog();
    let laid = run.sites.keep().expect("the Keep").laid;
    let castle = run.sites.keep().and_then(|s| s.castle()).expect("the tower");
    let passed = super::hands::bays(castle).iter().filter(|b| b.appears < laid).map(|b| b.appears).max().unwrap_or(0);
    // No poles in the yard: the scaffold is raised only as far as the wall has gone.
    run.home.industry.buildings.get_mut(&run.home.industry.sites[&id].yard).expect("yard").input.remove("pole");
    for _ in 0..u64::from(HZ) * 2 {
        run.step(&idle());
    }
    assert_eq!(run.hands.yard.timber, passed, "no poles, no bay ahead of the masons");
    // A knight carpenter stands what the camp has: the yard's poles, and then the bay stands ahead.
    run.home.industry.stock.insert("pole".into(), 40);
    let stood = run.home.industry.stand_poles(c, id).expect("poles stood");
    assert!(stood >= c.works.poles_standing, "{stood}");
    assert!(run.home.industry.scaffold_stands(c, id));
    for _ in 0..u64::from(HZ) * 2 {
        run.step(&idle());
    }
    assert!(run.hands.yard.timber > passed, "the next bay is raised ahead of the wall: {} from {passed}", run.hands.yard.timber);
    // With none in the camp the knight cannot.
    run.home.industry.buildings.get_mut(&run.home.industry.sites[&id].yard).expect("yard").input.remove("pole");
    run.home.industry.stock.remove("pole");
    assert!(run.home.industry.stand_poles(c, id).is_err());
}

#[test]
fn the_winch_raises_the_sims_lift_and_the_tier_is_the_lift() {
    use super::stockyard::Tier;
    let mut run = live();
    let c = catalog();
    for _ in 0..u64::from(HZ) * 2 {
        run.step(&idle());
    }
    assert_eq!(run.shaft_lift().map(|(_, t)| t), Some(Tier::Hand));
    assert_eq!(run.home.stockyard.tier, Tier::Hand);
    // The windlass is timber, rope and a bag of nails: the founding kit's, which this camp has not.
    run.home.industry.stock.insert("nails".into(), 100);
    assert_eq!(c.plans["shaft-hand"].shaft, Tier::Hand.throughput(), "the one scale: blocks a minute");
    let (next, cost, ready) = run.shaft_next().expect("a next lift");
    assert_eq!(next, Tier::Windlass);
    assert_eq!(cost, c.plans["shaft-windlass"].cost, "the sim's own price");
    ready.expect("the camp can pay for it");
    run.upgrade_shaft().expect("raised");
    let (id, _) = run.shaft_lift().expect("the lift");
    assert_eq!(run.home.industry.buildings[&id].plan, "shaft-windlass");
    for b in run.home.industry.buildings.values_mut() {
        b.construction = 0;
    }
    for _ in 0..u64::from(HZ) * 12 {
        run.step(&idle());
    }
    assert_eq!(run.home.stockyard.tier, Tier::Windlass);
    assert_eq!(c.plans["shaft-windlass"].shaft, Tier::Windlass.throughput());
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib works_seam -- --include-ignored"]
fn a_valley_village_comes_up_onto_the_hillside_when_the_blank_slate_stands() {
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let c = catalog();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    // A save from before the hillside: a village on the valley meadow.
    run.home.industry.found(c, 7);
    run.home.industry.stock.insert("timber".into(), 200);
    run.home.industry.apply(c, Command::Build { plan: "forester".into(), tile: (8, 8) }).expect("a forester");
    run.home.industry.apply(c, Command::Build { plan: "farm".into(), tile: (10, 8) }).expect("a farm");
    assert!(run.home.industry.is_valley_village());
    let people = run.home.industry.population.citizens.len();
    for _ in 0..u64::from(HZ) * 2 {
        run.step(&idle());
    }
    let k = &run.home.industry;
    assert!(k.world.is_hillside(), "the village is on the hillside");
    assert!(k.valid(c));
    assert!(k.population.citizens.len() >= people);
    assert!(k.buildings.values().any(|b| b.plan == "farm" || b.plan == "forester") || run.notices.iter().any(|n| n.contains("hillside")), "its works came with it");
    assert!(run.notices.iter().any(|n| n.contains("come up onto the hillside")), "{:?}", run.notices);
}
