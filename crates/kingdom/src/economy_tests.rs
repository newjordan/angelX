//! The kingdom economy's mechanisms, one at a time: the checker and solver,
//! trades and skill, the tool loop, mortar going off, woods running out,
//! claims through the Great Shaft, oxen and fodder, meals and wages, the
//! Deep's stir and long buildings. Each test uses `rich()` to set a stage and
//! then runs real seconds; the scenarios (works_scenario) play from founding.
use super::*;
use crate::design::{check, reach, solve, Severity, SolveOptions};
use crate::population::{trade_index, Population, Role, Rules, Site, TRADES};
use crate::works_tests::{build, cart, finish, rich, run};

fn errors(c: &Catalog) -> Vec<String> {
    check(c)
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{} {}", f.code, f.subject))
        .collect()
}

#[test]
fn the_shipped_catalogue_passes_its_own_checks() {
    let c = catalog();
    assert_eq!(errors(c), Vec::<String>::new(), "every good has a maker and a use, every building is reachable");
    // The camp age needs no Delve: the motte and bailey is raised from the ground.
    let camp = reach(c, false);
    for st in &c.castles["motte-and-bailey"].stages {
        for (g, n) in site::stage_needs(c, st) {
            assert!(n == 0 || camp.goods.contains(&g), "{g} for {}", st.id);
        }
    }
    // The works age wants the Delve: limestone is not in the camp's reach.
    assert!(!camp.goods.contains("limestone") && reach(c, true).goods.contains("limestone"));
}

fn rechecked(change: impl FnOnce(&mut Catalog)) -> Vec<String> {
    let mut c = catalog().clone();
    change(&mut c);
    let c = Catalog::parse(&serde_json::to_string(&c).unwrap()).expect("the broken catalogue is still well formed");
    errors(&c)
}

#[test]
fn the_checker_names_every_kind_of_hole() {
    // A good nobody makes.
    let e = rechecked(|c| {
        c.plans.get_mut("ropewalk").unwrap().recipes.clear();
        c.recipes.remove("spin-rope");
    });
    assert!(e.contains(&"no-maker rope".to_string()), "{e:?}");
    // A good nobody uses.
    let e = rechecked(|c| {
        c.resources.insert("trinket".into(), Resource { name: "Trinkets".into(), colour: 1, shelf: 0, waste: false, weight: 0 });
        c.recipes.insert(
            "make-trinkets".into(),
            Recipe { name: "Make trinkets".into(), inputs: Stock::new(), outputs: Stock::from([("trinket".into(), 1)]), shifts: 1, patch: None, level: 0 },
        );
        c.plans.get_mut("depot").unwrap().recipes.clear();
        c.plans.get_mut("well").unwrap().recipes.push("make-trinkets".into());
    });
    assert!(e.contains(&"no-use trinket".to_string()), "{e:?}");
    // A building that needs something never made.
    let e = rechecked(|c| {
        c.resources.insert("unobtainium".into(), Resource { name: "Unobtainium".into(), colour: 1, shelf: 0, waste: false, weight: 0 });
        c.plans.get_mut("bloomery").unwrap().cost.insert("unobtainium".into(), 1);
    });
    assert!(e.contains(&"unbuildable bloomery".to_string()) && e.contains(&"no-maker unobtainium".to_string()), "{e:?}");
    // A loop that needs its own product to start.
    let e = rechecked(|c| {
        for (g, other) in [("chicken", "egg"), ("egg", "chicken")] {
            c.resources.insert(g.into(), Resource { name: g.into(), colour: 1, shelf: 0, waste: false, weight: 0 });
            c.recipes.insert(
                format!("lay-{g}"),
                Recipe { name: g.into(), inputs: Stock::from([(other.into(), 1)]), outputs: Stock::from([(g.into(), 1)]), shifts: 1, patch: None, level: 0 },
            );
        }
        c.plans.get_mut("well").unwrap().recipes.extend(["lay-chicken".to_string(), "lay-egg".to_string()]);
    });
    assert!(e.contains(&"unreachable chicken".to_string()) && e.contains(&"unreachable egg".to_string()), "{e:?}");
    // Nobody re-steels the picks.
    let e = rechecked(|c| {
        c.plans.get_mut("smithy").unwrap().recipes.retain(|r| r != "resteel-pick");
        c.recipes.remove("resteel-pick");
    });
    assert!(e.contains(&"tool-loop pick".to_string()), "{e:?}");
    // A castle stage that wants something the works cannot make.
    let e = rechecked(|c| {
        c.resources.insert("unobtainium".into(), Resource { name: "Unobtainium".into(), colour: 1, shelf: 0, waste: false, weight: 0 });
        c.fixtures.push(Fixture { stage: "fittings".into(), needs: Stock::from([("unobtainium".into(), 1)]) });
    });
    assert!(e.iter().any(|x| x.starts_with("bill-chain great-tower")), "{e:?}");
    // A trade nobody can ever hold (the wagon train brings a wheelwright now, so the
    // example is the brewer, who has no founder and no kin).
    let e = rechecked(|c| {
        c.trades.get_mut("brewer").unwrap().kin.clear();
        for t in c.trades.values_mut() {
            t.kin.retain(|k| k != "brewer");
        }
    });
    assert!(e.contains(&"trade brewery".to_string()), "{e:?}");
}

#[test]
fn the_solver_counts_everything_down_to_the_ground() {
    let c = catalog();
    let req = solve(c, &SolveOptions::new("great-tower", 30.0)).unwrap();
    let stones = f64::from(c.castles["great-tower"].stones());
    let need: f64 = c.castles["great-tower"].stages.iter().map(|s| site::stage_needs(c, s).get("ashlar").copied().unwrap_or(0)).sum::<u32>().into();
    let ashlar = req.goods["ashlar"];
    assert!((ashlar - need / stones * 30.0).abs() < 1e-6, "{ashlar}");
    // Every plant runs at least as fast as its good is wanted.
    for p in &req.plants {
        let rec = &c.recipes[&p.recipe];
        assert!(f64::from(p.count) >= p.machines - 1e-9, "{} {}", p.plan, p.recipe);
        let per_min = 60.0 / (f64::from(rec.shifts) * 10.0);
        assert!(f64::from(p.count) * per_min + 1e-9 >= p.batches_per_min, "{} cannot keep up", p.recipe);
    }
    // The bankers' rubble covers the wall's core: no rubble pit is needed.
    assert!(!req.buildings.contains_key("rubble-pit"), "{:?}", req.buildings.keys());
    assert!(req.buildings["banker"] >= 5 && req.buildings["lime-kiln"] >= 1 && req.buildings["smithy"] >= 1);
    // Rock, ore and gold come from the Delve through the shaft; wood and bread do not.
    for g in ["limestone", "iron-ore", "galena"] {
        assert!(req.claims.contains_key(g), "{g} comes up the shaft");
    }
    assert!(!req.claims.contains_key("timber") && !req.claims.contains_key("grain"));
    // The people: bread for all, ale for the skilled; supervisors one per four posts.
    assert!(req.goods["bread"] > 0.0 && req.goods["ale"] > 0.0 && req.coin_per_min > 0.0);
    for (t, posts) in &req.posts_by_trade {
        assert_eq!(req.supervisors_by_trade[t], posts.div_ceil(4));
    }
    // Twice the pace is about twice the works.
    let double = solve(c, &SolveOptions::new("great-tower", 60.0)).unwrap();
    assert!(double.buildings["banker"] >= req.buildings["banker"] * 2 - 1, "{} vs {}", double.buildings["banker"], req.buildings["banker"]);
    assert!(double.workers > req.workers && double.vehicles > req.vehicles && double.yards >= req.yards && double.shaft_per_shift > req.shaft_per_shift);
    // Power halves the bankers.
    let mut o = SolveOptions::new("great-tower", 60.0);
    o.powered = true;
    let powered = solve(c, &o).unwrap();
    assert!(powered.buildings["banker"] < double.buildings["banker"] && powered.power_units > 0);
    // The JSON is the tree from the wall down, then the counts.
    let json = req.json(c);
    assert!(json["tree"]["set_in_the_wall"]["needs"].as_array().unwrap().len() >= 9);
    assert_eq!(json["findings"].as_array().unwrap().len(), 0);
    assert!(json["buildings"]["banker"].as_u64().unwrap() >= 5);
    // The motte and bailey needs no stone chain at all.
    let motte = solve(c, &SolveOptions::new("motte-and-bailey", 20.0)).unwrap();
    assert!(!motte.buildings.contains_key("banker") && motte.buildings.contains_key("carpentry"));
}

// ---- trades -----------------------------------------------------------

fn post(id: u32, trade: &str, level: u8) -> Site {
    Site { id, tile: (id as i16, 1), role: Role::Builder, jobs: 1, beds: 0, online: true, priority: 1, held: 0, trade: trade_index(trade).map(|t| (t, level)) }
}

#[test]
fn one_qualified_person_supervises_four_posts_and_the_rest_learn_beside_them() {
    let c = catalog();
    let rules = Rules::of(c, true);
    let mut pop = Population::founders();
    for _ in 0..12 {
        let _ = pop.advance_with(&[], &mut Stock::from([("bread".into(), 99)]), &rules);
    }
    let ids: Vec<u32> = pop.citizens.keys().copied().collect();
    // Nobody has the trade yet: a banker's post cannot run.
    let banker: Vec<Site> = (1..=6).map(|i| post(i, "banker-mason", 2)).collect();
    pop.reconcile_with(&banker, &rules).unwrap();
    assert!(pop.citizens.values().all(|p| p.job.is_none()), "no one to supervise the work");
    // One journeyman mason: four posts run, the fifth waits for another.
    assert!(pop.grant(ids[0], "banker-mason", 2, &rules));
    pop.reconcile_with(&banker, &rules).unwrap();
    let running = pop.citizens.values().filter(|p| p.job.is_some()).count();
    assert_eq!(running, 4, "one supervisor, four posts");
    // Learners work slower than the journeyman.
    let report = pop.report_with(&banker, &rules);
    assert!(report.efficiency.values().any(|e| *e == 60) && report.efficiency.values().any(|e| *e == 100), "{:?}", report.efficiency);
    // Time at the post is how it is learned: after enough seconds a learner is a journeyman.
    let mut stock = Stock::from([("bread".into(), 5000)]);
    for _ in 0..c.works.journeyman_xp + 10 {
        pop.advance_with(&banker[..4], &mut stock, &rules).unwrap();
    }
    let journeymen = pop.citizens.values().filter(|p| p.skill("banker-mason", &rules) >= 2).count();
    assert!(journeymen >= 2, "learners became journeymen: {journeymen}");
    // And now the work of the fifth and sixth posts can be supervised too.
    pop.reconcile_with(&banker, &rules).unwrap();
    assert!(pop.citizens.values().filter(|p| p.job.is_some()).count() >= 5);
    // A kindred journeyman supervises at one level lower: a mason can oversee lime-burners at level 1.
    let mut kin = Population::founders();
    kin.grant(1, "banker-mason", 2, &rules);
    kin.reconcile_with(&[post(1, "lime-burner", 1)], &rules).unwrap();
    assert!(kin.citizens.values().any(|p| p.job.is_some()));
    kin.reconcile_with(&[post(1, "lime-burner", 2)], &rules).unwrap();
    assert!(kin.citizens.values().all(|p| p.job.is_none()), "not at journeyman level");
    // The valley holds no trades at all.
    let mut old = Population::founders();
    old.reconcile(&[post(1, "banker-mason", 2)]).unwrap();
    assert!(old.citizens.values().any(|p| p.job.is_some()), "legacy sites ignore trades");
    assert_eq!(TRADES.len(), 23);
}

#[test]
fn a_teacher_takes_coin_and_brings_a_person_one_level_on() {
    let c = catalog();
    let mut k = rich();
    k.stock.insert("coin".into(), 100);
    let rules = Rules::of(c, true);
    let who = k.population.citizens.keys().copied().nth(3).unwrap();
    let before = k.amount("coin");
    k.apply(c, Command::Teach { citizen: who, trade: "miller".into() }).unwrap();
    assert_eq!(k.amount("coin"), before - c.works.tuition);
    assert_eq!(k.population.citizens[&who].skill("miller", &rules), 1);
    k.apply(c, Command::Teach { citizen: who, trade: "miller".into() }).unwrap();
    assert_eq!(k.population.citizens[&who].skill("miller", &rules), 2);
    assert_eq!(k.amount("coin"), before - 3 * c.works.tuition, "the second lesson costs twice");
    k.apply(c, Command::Teach { citizen: who, trade: "miller".into() }).unwrap();
    assert!(k.apply(c, Command::Teach { citizen: who, trade: "miller".into() }).is_err(), "a master has nothing to learn here");
    k.stock.insert("coin".into(), 0);
    assert!(k.apply(c, Command::Teach { citizen: 1, trade: "baker".into() }).unwrap_err().contains("coin"));
    // Everyone's trade and level can be read off the report.
    let seen = k.report(c).population.citizens;
    assert!(seen.iter().any(|v| v.trades.iter().any(|(t, l)| t == "miller" && *l == 3)));
    assert!(seen.iter().any(|v| v.trades.first().is_some_and(|(t, l)| t == "woodsman" && *l == 2)), "the founders' trades");
}

// ---- tools, patches, perishables ---------------------------------------

#[test]
fn a_worn_tool_goes_back_to_the_smith_and_a_post_without_one_stops() {
    let c = catalog();
    let mut k = rich();
    for r in ["axe", "worn-axe"] {
        k.stock.insert(r.into(), 0);
    }
    k.stock.insert("axe".into(), 1);
    let id = build(&mut k, "woodsman-camp", (139, 97));
    finish(&mut k);
    cart(&mut k, id, 2);
    k.stock.insert("axe".into(), 1);
    // One axe: it is taken at the first batch and worn out after its life.
    // (The camp's timber is spent as it comes, so the woodsman never stands idle.)
    let spend = |k: &mut Kingdom, seconds: usize| {
        for _ in 0..seconds / 10 {
            k.stock.insert("timber".into(), 0);
            run(k, 10);
        }
    };
    spend(&mut k, 60);
    assert_eq!(k.amount("axe"), 0, "the axe is in the woodsman's hand");
    assert!(k.buildings[&id].tool_left > 0);
    let life = u32::from(c.plans["woodsman-camp"].tool_life);
    spend(&mut k, (life * 10) as usize * 2);
    assert_eq!(k.amount("worn-axe"), 1, "worn out, it goes back to the camp for a smith");
    assert_eq!(k.report(c).status[&id], Status::Tools, "and there is no other to give");
    let before = k.produced.get("timber").copied().unwrap_or(0);
    run(&mut k, 120);
    assert_eq!(k.produced.get("timber").copied().unwrap_or(0), before, "a site without a tool grinds to a halt");
    // The Clerk says why.
    let said = k.clerk_why_idle(c, id);
    assert!(said.contains("axe"), "{said}");
    // A smith re-steels four worn axes with a little steel and fuel.
    k.stock.insert("timber".into(), 100);
    let smithy = build(&mut k, "smithy", (136, 98));
    finish(&mut k);
    k.apply(c, Command::Recipe { building: smithy, recipe: "resteel-axe".into() }).unwrap();
    let b = k.buildings.get_mut(&smithy).unwrap();
    b.input = Stock::from([("worn-axe".into(), 4), ("charcoal".into(), 2), ("steel".into(), 2)]);
    k.apply(c, Command::Connect(smithy)).unwrap();
    run(&mut k, 60);
    assert_eq!(k.buildings[&smithy].output.get("axe").copied().unwrap_or(0), 4, "four re-steeled axes");
    assert!(!k.buildings[&smithy].input.contains_key("worn-axe"), "paid for in worn tools");
}

#[test]
fn woods_run_out_under_plain_felling_and_coppicing_keeps_them() {
    let c = catalog();
    let mut k = rich();
    let id = build(&mut k, "woodsman-camp", (139, 97));
    finish(&mut k);
    let wood = k.buildings[&id].patch.clone().expect("it works a wood");
    assert!(wood.starts_with("wood-"));
    let cap = hillside::patch_capacity(&wood);
    assert!(cap > 1000);
    cart(&mut k, id, 3);
    // Felling takes timber from the wood, 4 a batch.
    k.stock.insert("timber".into(), 0);
    run(&mut k, 200);
    assert!(k.nature[&wood] < cap, "the wood is shorter");
    // Run it nearly dry: it stops, and says so.
    *k.nature.get_mut(&wood).unwrap() = 6;
    k.stock.insert("timber".into(), 0);
    run(&mut k, 100);
    assert!(k.nature[&wood] < 4, "{}", k.nature[&wood]);
    assert_eq!(k.report(c).status[&id], Status::Depleted);
    // Under a quarter it does not regrow; the wood is gone.
    let left = k.nature[&wood];
    run(&mut k, 200);
    assert_eq!(k.nature[&wood], left);
    // Coppicing leaves it: set to poles, a healthy wood gives poles and stays full.
    *k.nature.get_mut(&wood).unwrap() = cap / 2;
    k.apply(c, Command::Recipe { building: id, recipe: "coppice-poles".into() }).unwrap();
    k.stock.insert("pole".into(), 0);
    let before = k.nature[&wood];
    run(&mut k, 200);
    assert!(k.produced.get("pole").copied().unwrap_or(0) > 0, "poles");
    assert!(k.nature[&wood] >= before, "a coppiced wood never shrinks, it regrows");
    // The outcrop of field stone is finite as well, and the valley's forester is not affected.
    assert!(k.nature.contains_key("field"));
    let mut valley = Kingdom::default();
    valley.start(c);
    assert!(valley.nature.is_empty());
}

#[test]
fn mortar_goes_off_so_the_mill_must_stand_near_the_wall() {
    let c = catalog();
    let mut k = rich();
    assert_eq!(c.resources["mortar"].shelf, 900);
    // Mortar sitting in a store loses a share of itself every shift.
    k.stock.insert("mortar".into(), 90);
    let mill = build(&mut k, "mortar-mill", (141, 99));
    finish(&mut k);
    k.buildings.get_mut(&mill).unwrap().output.insert("mortar".into(), 24);
    let total = k.total("mortar");
    run(&mut k, 300);
    let spoiled = k.spoiled.get("mortar").copied().unwrap_or(0);
    assert!(spoiled > 20, "{spoiled} went off in five minutes");
    assert_eq!(k.total("mortar") + spoiled, total, "nothing vanishes unrecorded");
    // A good that keeps does not.
    k.stock.insert("rope".into(), 10);
    run(&mut k, 100);
    assert_eq!(k.amount("rope"), 10);
    // The yard asks for mortar a little at a time, not a stage's worth.
    let (mut k, s) = {
        let mut k = rich();
        k.apply(c, Command::FoundSite { plan: "great-tower".into(), centre: hillside::KEEP_CENTRE, yard: (150, 84) }).unwrap();
        finish(&mut k);
        (k, 1)
    };
    k.sites.get_mut(&s).unwrap().stage = 1;
    let wants = k.site_wants(c, &k.sites[&s]);
    assert_eq!(wants["mortar"], c.works.perishable_stock);
    assert!(wants["ashlar"] > 400, "what keeps is stocked ahead");
}

// ---- the Delve: claims, the Great Shaft -------------------------------

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

#[test]
fn a_charter_brings_up_the_rock_and_what_lies_with_it_through_the_great_shaft() {
    let c = catalog();
    let mut k = rich();
    for r in ["limestone", "iron-ore", "galena"] {
        k.stock.insert(r.into(), 0);
    }
    claim(&mut k, "mines-2", &[("limestone", 4), ("iron-ore", 1)]);
    claim(&mut k, "mines-3", &[("limestone", 4), ("galena", 1)]);
    let a = build(&mut k, "claim-quarry", (150, 97));
    let b = build(&mut k, "claim-quarry", (156, 97));
    finish(&mut k);
    k.apply(c, Command::Link { building: a, claim: "mines-2".into() }).unwrap();
    k.apply(c, Command::Link { building: b, claim: "mines-3".into() }).unwrap();
    k.apply(c, Command::Connect(a)).unwrap();
    k.apply(c, Command::Connect(b)).unwrap();
    // The hand lift raises three blocks a minute; one seam is five, two are ten.
    let shaft = k.buildings.values().find(|x| x.plan == "shaft-hand").unwrap().tile;
    assert_eq!(shaft, hillside::HILL_SHAFT);
    run(&mut k, 10);
    let r = k.report(c);
    assert_eq!(r.shaft, 3, "hand lift");
    run(&mut k, 300);
    // Each shift one quarry's batch fits and the other waits: the lift is the limit.
    let lifted = k.shaft_moved;
    assert!(lifted <= 6, "{lifted}");
    let made = k.produced.get("limestone").copied().unwrap_or(0);
    assert!(made > 0 && made <= 3 * 5 + 5, "{made} units of limestone in five minutes");
    assert!(k.buildings[&a].stats.worked + k.buildings[&b].stats.worked > 0);
    // Raise the lift: a windlass makes it ten, in place, for beams, rope and a bag of nails (the
    // founding kit has them; the iron cramps wait for the gin).
    k.researched.insert("ropemaking".into());
    k.stock.insert("nails".into(), 100);
    let hoist = k.buildings.iter().find(|(_, x)| x.plan == "shaft-hand").map(|(&i, _)| i).unwrap();
    k.apply(c, Command::Upgrade(hoist)).unwrap();
    assert_eq!(k.buildings[&hoist].plan, "shaft-windlass");
    run(&mut k, 60);
    assert_eq!(k.report(c).shaft, 10, "windlass");
    // And so up: gin, water hoist, ley lift, each replacing the last.
    for (want, tier) in [("shaft-gin", 24), ("shaft-water", 20), ("shaft-ley", 70)] {
        for (t, _) in c.all_techs() {
            k.researched.insert(t.clone());
        }
        k.stock.insert("horse".into(), 10);
        for g in ["beam", "rope", "cramp", "block", "plank", "steel", "crystal", "mana"] {
            k.stock.insert(g.into(), 500);
        }
        k.buildings.get_mut(&hoist).unwrap().construction = 0;
        k.apply(c, Command::Upgrade(hoist)).unwrap();
        assert_eq!(k.buildings[&hoist].plan, want);
        assert_eq!(c.plans[want].shaft, tier);
    }
    assert!(k.apply(c, Command::Upgrade(hoist)).is_err(), "nothing above the ley lift");
}

#[test]
fn old_claims_without_yields_still_yield_what_they_always_did() {
    let old = r#"{"resource":"iron-ore","name":"Vein","source":"Mines / floor 1","secured":true,"rate":2}"#;
    let claim: Claim = serde_json::from_str(old).unwrap();
    assert!(claim.yields.is_empty());
    assert_eq!(claim.output(), Stock::from([("iron-ore".to_string(), 2)]));
    let with = Claim { yields: [("limestone".into(), 3), ("crystal".into(), 1)].into(), ..claim.clone() };
    assert_eq!(with.output().len(), 2);
    let again: Claim = serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
    assert_eq!(again, with);
}

// ---- oxen, fodder, meals, wages ----------------------------------------

#[test]
fn oxen_eat_fodder_and_a_yard_that_cannot_feed_them_stands_still() {
    let c = catalog();
    let mut k = rich();
    let f = build(&mut k, "woodsman-camp", (139, 97));
    finish(&mut k);
    cart(&mut k, f, 4);
    let y = *k.buildings.iter().find(|(_, b)| b.plan == "carters-yard").unwrap().0;
    k.stock.insert("timber".into(), 0);
    let fodder = k.total("fodder");
    run(&mut k, 180);
    assert!(k.total("fodder") < fodder, "four oxen eat");
    let eaten = fodder - k.total("fodder");
    assert!((4..=8).contains(&eaten), "a unit every ten minutes each, the first at once: {eaten}");
    assert!(k.roadwork.moved > 0);
    // No fodder, no pulling.
    k.stock.insert("fodder".into(), 0);
    k.buildings.get_mut(&y).unwrap().input.clear();
    run(&mut k, 700);
    let moved = k.roadwork.moved;
    k.stock.insert("timber".into(), 0);
    run(&mut k, 200);
    assert_eq!(k.report(c).status[&y], Status::Fodder);
    assert!(k.roadwork.moved <= moved + 40, "the oxen stand in their stalls");
    // Fed again, they work.
    k.stock.insert("fodder".into(), 100);
    k.stock.insert("timber".into(), 0);
    run(&mut k, 200);
    assert!(k.roadwork.moved > moved + 40);
}

#[test]
fn skilled_people_want_ale_and_the_delve_pays_the_wages() {
    let c = catalog();
    let mut k = rich();
    let rules = Rules::of(c, true);
    for g in ["bread", "ale", "meat", "cheese", "coin", "livery"] {
        k.stock.insert(g.into(), 0);
    }
    // Founders are journeymen (skilled): bread alone leaves them uneasy.
    k.stock.insert("bread".into(), 500);
    let skilled: Vec<u32> = k.population.citizens.iter().filter(|(_, p)| p.best(&rules).1 >= 2).map(|(&i, _)| i).collect();
    assert!(!skilled.is_empty());
    run(&mut k, 1500);
    let morale = |k: &Kingdom, id: u32| k.population.citizens[&id].morale;
    let no_ale: u32 = skilled.iter().map(|&i| u32::from(morale(&k, i))).sum();
    k.stock.insert("ale".into(), 500);
    k.stock.insert("coin".into(), 500);
    run(&mut k, 1500);
    let with_ale: u32 = skilled.iter().map(|&i| u32::from(morale(&k, i))).sum();
    assert!(with_ale > no_ale, "ale and wages lift the skilled: {no_ale} -> {with_ale}");
    assert!(k.amount("ale") < 500, "ale was drunk");
    assert!(k.amount("coin") < 500, "wages were paid in the Delve's coin");
    let paid = 500 - k.amount("coin");
    // Wages run: journeymen at one coin a pay day (600 s).
    assert!(paid >= skilled.len() as u32, "{paid} coin to {} skilled", skilled.len());
    // A coat of livery lifts morale for its season.
    k.stock.insert("livery".into(), 20);
    run(&mut k, 400);
    assert!(k.amount("livery") < 20, "livery issued");
    assert!(k.population.citizens.values().any(|p| p.livery > 0));
}

// ---- pressure, long buildings -------------------------------------------

#[test]
fn smoke_noise_and_digging_stir_the_deep_and_a_standing_castle_calms_it() {
    let c = catalog();
    let mut k = rich();
    // The works stand east of the Keep's wall, a longer road from the camp.
    k.stock.insert("timber".into(), 200);
    let quiet = k.pressure(c);
    assert_eq!(quiet.level, Stir::Calm);
    assert_eq!(quiet.stir, 0);
    // Kilns smoke, quarries ring and the shaft digs.
    k.population.add_people(20);
    claim(&mut k, "seam", &[("limestone", 6)]);
    let q = build(&mut k, "claim-quarry", (150, 97));
    let kiln = build(&mut k, "lime-kiln", (156, 97));
    let bank = build(&mut k, "banker", (162, 97));
    finish(&mut k);
    k.apply(c, Command::Link { building: q, claim: "seam".into() }).unwrap();
    for id in [q, kiln, bank] {
        k.apply(c, Command::Connect(id)).unwrap();
    }
    k.apply(c, Command::Recipe { building: bank, recipe: "dress-limestone".into() }).unwrap();
    k.apply(c, Command::Recipe { building: kiln, recipe: "burn-lime-coal".into() }).unwrap();
    k.buildings.get_mut(&kiln).unwrap().input = Stock::from([("limestone".into(), 32), ("coal".into(), 8)]);
    k.buildings.get_mut(&bank).unwrap().input = Stock::from([("limestone".into(), 30)]);
    cart(&mut k, q, 4);
    k.stock.insert("limestone".into(), 0);
    run(&mut k, 60);
    // Machines come and go between seconds; take the most the works did in a minute.
    let mut busy = k.pressure(c);
    for _ in 0..60 {
        run(&mut k, 1);
        let p = k.pressure(c);
        busy = Pressure { smoke: busy.smoke.max(p.smoke), noise: busy.noise.max(p.noise), digging: busy.digging.max(p.digging), stir: busy.stir.max(p.stir), ..busy };
    }
    assert!(busy.smoke >= 4 && busy.noise >= 3 && busy.digging >= 3, "{busy:?}");
    assert!(busy.stir > quiet.stir);
    // A castle going up calms the land by the stones it has laid.
    for g in ["timber", "plank", "stone"] {
        k.stock.insert(g.into(), 200);
    }
    k.apply(c, Command::FoundSite { plan: "great-tower".into(), centre: hillside::KEEP_CENTRE, yard: (150, 84) }).unwrap();
    k.sites.get_mut(&1).unwrap().laid = 0;
    let before = k.pressure(c).calm;
    k.sites.get_mut(&1).unwrap().stage = 6;
    k.sites.get_mut(&1).unwrap().laid = c.castles["great-tower"].stages[6].first;
    let after = k.pressure(c);
    assert!(after.calm > before && after.calm <= pressure::CALM_MAX, "{after:?}");
    assert!(Stir::Raiding > Stir::Stirring && Stir::Stirring > Stir::Uneasy && Stir::Uneasy > Stir::Calm);
}

#[test]
fn a_ropewalk_is_a_long_straight_shed_you_must_find_room_for() {
    let c = catalog();
    let mut k = rich();
    assert_eq!(c.plans["ropewalk"].span, 5);
    let walk = build(&mut k, "ropewalk", (126, 100));
    finish(&mut k);
    // All six tiles are the ropewalk's.
    for dx in 0..=5 {
        assert_eq!(k.at((126 + dx, 100)), Some(walk), "tile {dx}");
    }
    assert_eq!(k.at((132, 100)), None);
    assert!(k.apply(c, Command::Road((129, 100))).is_err(), "no road through a shed");
    assert!(k.apply(c, Command::Build { plan: "cottage".into(), tile: (131, 100) }).is_err());
    // It needs the room: a cottage in the way, or the river's edge, stops it.
    assert!(k.apply(c, Command::Build { plan: "ropewalk".into(), tile: (129, 100) }).is_err());
    let mut tight = rich();
    build(&mut tight, "cottage", (130, 100));
    let e = tight.apply(c, Command::Build { plan: "ropewalk".into(), tile: (126, 100) }).unwrap_err();
    assert!(e.contains("clear plots"), "{e}");
    // It saves and reloads whole, and the walkers go round it.
    let again: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert!(again.valid(c) && again == k);
    run(&mut k, 30);
}

#[test]
fn patterns_stamp_whole_groups_atomically_and_price_themselves() {
    let c = catalog();
    let mut k = rich();
    for g in ["timber", "plank", "stone", "block", "nails", "beam"] {
        k.stock.insert(g.into(), 400);
    }
    let cost = k.stamp_cost(c, "lime-works", (150, 60), 0).unwrap();
    assert!(cost["stone"] > 40 && cost["timber"] > 20 && !cost.contains_key("cart"), "{cost:?}");
    let (b0, l0) = (k.buildings.len(), k.lines.len());
    let msg = k.apply(c, Command::Stamp { pattern: "lime-works".into(), origin: (150, 60), turns: 0 }).unwrap();
    assert!(msg.contains("lime works") || msg.contains("Lime works"), "{msg}");
    assert_eq!(k.buildings.len(), b0 + 5);
    assert_eq!(k.lines.len(), l0, "the roads carry what a pattern's buildings trade");
    assert!(k.valid(c));
    // Turned a quarter, it stands the other way, and still joins up.
    let mut turned = rich();
    for g in ["timber", "plank", "stone", "block", "nails", "beam"] {
        turned.stock.insert(g.into(), 400);
    }
    turned.apply(c, Command::Stamp { pattern: "lime-works".into(), origin: (160, 70), turns: 1 }).unwrap();
    let ys: Vec<i16> = turned.buildings.values().filter(|b| b.plan == "lime-kiln" || b.plan == "mortar-mill").map(|b| b.tile.1).collect();
    assert!(ys.iter().any(|y| *y > 70), "{ys:?}");
    // One that cannot be finished anywhere changes nothing at all.
    k.apply(c, Command::FoundSite { plan: "great-tower".into(), centre: hillside::KEEP_CENTRE, yard: (150, 84) }).unwrap();
    let before = k.clone();
    assert!(k.apply(c, Command::Stamp { pattern: "lime-works".into(), origin: (150, 60), turns: 0 }).is_err(), "the same ground twice");
    assert!(k.apply(c, Command::Stamp { pattern: "banker-shed".into(), origin: (125, 75), turns: 0 }).is_err(), "inside the castle's ground");
    assert_eq!(k, before);
    assert!(k.apply(c, Command::Stamp { pattern: "no-such".into(), origin: (1, 1), turns: 0 }).is_err());
}

#[test]
fn the_oxen_that_carry_a_farms_hay_eat_it_so_the_fodder_road_cannot_starve_itself() {
    let c = catalog();
    let mut k = rich();
    let farm = build(&mut k, "farmstead", (139, 97));
    finish(&mut k);
    k.apply(c, Command::Recipe { building: farm, recipe: "grow-fodder".into() }).unwrap();
    cart(&mut k, farm, 2);
    let y = *k.buildings.iter().find(|(_, b)| b.plan == "carters-yard").unwrap().0;
    // The camp has no fodder at all, and nothing else to feed the oxen with.
    k.stock.insert("fodder".into(), 0);
    k.stock.insert("timber".into(), 0);
    run(&mut k, 900);
    assert!(k.roadwork.moved > 0, "the hay got through");
    assert!(k.amount("fodder") > 0, "and it reached the camp");
    assert_ne!(k.report(c).status[&y], Status::Fodder);
}

/// A by-product nobody hauls away is tipped on the midden: the clamp never jams on its ash.
#[test]
fn a_waste_good_never_blocks_its_maker() {
    let c = catalog();
    let mut k = crate::works_tests::hill();
    k.stock.insert("timber".into(), 400);
    let id = build(&mut k, "clamp", (140, 99));
    finish(&mut k);
    k.apply(c, Command::Connect(id)).unwrap();
    // Give it timber by hand (a line would do this on the hillside) and let it burn for ten minutes.
    for _ in 0..600 {
        let b = k.buildings.get_mut(&id).unwrap();
        b.input.insert("timber".into(), 32);
        b.output.remove("charcoal");
        k.advance_second(c);
    }
    let b = &k.buildings[&id];
    assert!(b.output.get("ash").copied().unwrap_or(0) <= 24, "the heap is capped: {:?}", b.output);
    assert!(k.produced.get("charcoal").copied().unwrap_or(0) >= 20, "the clamp kept burning: {:?}", k.produced);
    assert!(!matches!(k.report(c).status[&id], Status::Storage), "ash does not back the clamp up");
}
