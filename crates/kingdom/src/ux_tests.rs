//! The feel of building: ghost plans and the hands that raise them, taking a
//! placement back, copies kept with the realm, the books and the numbers read
//! from them, the milestones, and what an old village's lost works are paid back.
use super::*;
use crate::ghosts::{self, Laid, Leg};
use crate::works_tests::{build, cart, finish, hill, rich, run};

/// A free tile of ground for a plan near `near`.
fn ground(k: &Kingdom, plan: &str, near: Tile) -> Tile {
    first_chain::ground_for(catalog(), k, plan, near).unwrap_or_else(|| panic!("no ground for {plan}"))
}

fn plan(k: &mut Kingdom, plan: &str, near: Tile, priority: u8) -> u32 {
    let t = ground(k, plan, near);
    k.apply(catalog(), Command::Plan { plan: plan.into(), tile: t, priority, by: 1 }).unwrap_or_else(|e| panic!("{plan} at {t:?}: {e}"));
    k.plans.next
}

#[test]
fn a_plan_can_be_laid_that_the_camp_cannot_pay_for_and_says_what_it_waits_on() {
    let c = catalog();
    let mut k = hill();
    k.researched = c.all_techs().map(|(k, _)| k.clone()).collect();
    k.stock.clear();
    k.stock.insert("fodder".into(), 100);
    let id = plan(&mut k, "sawpit", (150, 100), 1);
    assert_eq!(k.plans.ghosts.len(), 1);
    let why = k.ghost_wait(c, id).expect("it waits");
    assert!(why.starts_with("needs ") && why.contains("timber"), "{why}");
    run(&mut k, 60);
    assert!(k.at(k.plans.ghosts[&id].tile).is_none(), "nothing is raised from nothing");
    assert!(k.plans.crews.iter().all(|w| w.leg == Leg::Idle), "the hands wait");
    // The camp gets the goods: the hands walk out with them and raise it.
    k.stock.insert("timber".into(), 100);
    k.stock.insert("plank".into(), 100);
    k.stock.insert("stone".into(), 100);
    let tile = k.plans.ghosts[&id].tile;
    let before = k.amount("timber");
    run(&mut k, 1);
    assert_eq!(k.plans.crews[0].leg, Leg::Out, "walking out");
    assert!(!k.plans.crews[0].carrying.is_empty(), "with the materials");
    assert!(k.amount("timber") < before, "taken from the camp as they set out");
    run(&mut k, 80);
    let b = k.at(tile).expect("the sawpit stands");
    assert_eq!(k.buildings[&b].plan, "sawpit");
    assert_eq!(k.buildings[&b].construction, 0, "raised by the hands that carried it");
    assert!(k.plans.ghosts.is_empty() && k.plans.raised == 1);
    assert!(k.amount("timber") < before, "and the camp paid for it");
    assert!(k.valid(c));
}

#[test]
fn the_hands_take_the_most_urgent_first_and_the_nearest_of_those() {
    let c = catalog();
    let mut k = rich();
    k.stock.insert("timber".into(), 400);
    k.stock.insert("plank".into(), 400);
    k.stock.insert("stone".into(), 400);
    let far = plan(&mut k, "well", (190, 118), 1);
    let near = plan(&mut k, "well", (150, 100), 1);
    let urgent = plan(&mut k, "well", (200, 126), 2);
    let low = plan(&mut k, "well", (146, 99), 0);
    run(&mut k, 1);
    assert_eq!(k.plans.crews[0].ghost, urgent, "urgent beats near");
    // One crew, so the rest wait their turn: after the urgent, the near usual one, the far one, the low one.
    let mut order = Vec::new();
    for _ in 0..600 {
        run(&mut k, 1);
        let at = k.plans.crews.first().map_or(0, |w| w.ghost);
        if at != 0 && order.last() != Some(&at) {
            order.push(at);
        }
    }
    assert_eq!(order, vec![urgent, near, far, low], "{order:?}");
    assert!(k.plans.ghosts.is_empty(), "all raised");
    assert_eq!(k.plans.raised, 4);
    let _ = c;
}

#[test]
fn a_carpenters_shop_fields_another_crew_and_the_work_goes_faster() {
    let c = catalog();
    let mut one = rich();
    for k in [&mut one] {
        k.stock.insert("timber".into(), 800);
        k.stock.insert("plank".into(), 800);
        k.stock.insert("stone".into(), 800);
    }
    let mut two = one.clone();
    let spot = ground(&two, "carpentry", (146, 99));
    let shop = build(&mut two, "carpentry", spot);
    finish(&mut two);
    assert_eq!(two.crews_wanted(c), 2);
    assert_eq!(one.crews_wanted(c), 1);
    for k in [&mut one, &mut two] {
        for i in 0..6 {
            plan(k, "well", (150 + i * 3, 104), 1);
        }
    }
    let time = |k: &mut Kingdom| {
        let mut s = 0;
        while !k.plans.ghosts.is_empty() && s < 2000 {
            run(k, 1);
            s += 1;
        }
        s
    };
    let (a, b) = (time(&mut one), time(&mut two));
    assert!(b < a, "two crews ({b} s) beat one ({a} s)");
    assert!(two.buildings.contains_key(&shop));
}

#[test]
fn a_plan_is_cancelled_and_the_goods_a_crew_carried_come_back() {
    let c = catalog();
    let mut k = rich();
    let id = plan(&mut k, "sawpit", (160, 104), 1);
    let stock = k.stock.clone();
    run(&mut k, 2);
    assert_eq!(k.plans.crews[0].leg, Leg::Out);
    assert!(k.amount("timber") < stock["timber"], "the crew took its goods");
    k.apply(c, Command::Unplan(id)).expect("cancelled");
    assert!(k.plans.ghosts.is_empty());
    assert_eq!(k.stock, stock, "everything is back in the camp");
    run(&mut k, 60);
    assert!(k.plans.crews.is_empty() || k.plans.crews.iter().all(|w| w.leg == Leg::Idle));
    assert!(k.buildings.values().all(|b| b.plan != "sawpit"));
}

#[test]
fn the_ground_is_the_plans_to_keep_and_what_a_plan_may_not_overlap_is_said() {
    let c = catalog();
    let mut k = rich();
    let t = ground(&k, "sawpit", (160, 104));
    k.apply(c, Command::Plan { plan: "sawpit".into(), tile: t, priority: 1, by: 1 }).unwrap();
    let e = k.apply(c, Command::Plan { plan: "well".into(), tile: (t.0 + 1, t.1), priority: 1, by: 2 }).unwrap_err();
    assert!(e.contains("a plan is laid there"), "{e}");
    let e = k.apply(c, Command::Plan { plan: "well".into(), tile: k.camp(), priority: 1, by: 2 }).unwrap_err();
    assert!(!e.is_empty());
    // A plan is the party's: every seat sees it, and anyone's can be cancelled or reranked.
    let id = k.plans.next;
    k.apply(c, Command::Rank { ghost: id, priority: 2 }).unwrap();
    assert_eq!(k.plans.ghosts[&id].priority, 2);
    assert_eq!(k.plans.ghosts[&id].by, 1);
    // It saves and reads back whole.
    let back: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert_eq!(back.plans, k.plans);
    assert!(back.valid(c));
}

#[test]
fn a_road_and_a_stamped_pattern_can_be_planned_and_are_raised_whole() {
    let c = catalog();
    let mut k = rich();
    for r in ["timber", "plank", "stone", "block", "nails"] {
        k.stock.insert(r.into(), 600);
    }
    let t = ground(&k, "well", (170, 108));
    k.apply(c, Command::Plan { plan: ghosts::ROAD.into(), tile: (t.0, t.1 + 3), priority: 1, by: 1 }).unwrap();
    let origin = ground(&k, "cottage", (176, 112));
    let (words, ids) = k.plan_stamp(c, "hamlet", origin, 0, 1, 1).expect("the hamlet is planned");
    assert!(words.contains("Hamlet") && ids.len() >= 12, "{words} {}", ids.len());
    let want = k.plans.ghosts.len();
    run(&mut k, 900);
    assert!(k.plans.ghosts.len() < want, "the hands have been at it");
    run(&mut k, 3000);
    assert!(k.plans.ghosts.is_empty(), "{:?}", k.plans.ghosts.values().map(|g| (&g.plan, g.tile)).collect::<Vec<_>>());
    assert_eq!(k.buildings.values().filter(|b| b.plan == "cottage").count(), 4);
    // And all or nothing: a stamp that cannot be fully planned plans none of it.
    let n = k.plans.ghosts.len();
    assert!(k.plan_stamp(c, "hamlet", k.camp(), 0, 1, 1).is_err());
    assert_eq!(k.plans.ghosts.len(), n);
}

#[test]
fn taking_back_what_was_just_laid_gives_the_whole_price_and_a_worked_thing_half() {
    let c = catalog();
    let mut k = rich();
    let stock = k.stock.clone();
    let t = ground(&k, "sawpit", (160, 104));
    let id = build(&mut k, "sawpit", t);
    assert!(k.amount("timber") < stock["timber"]);
    k.apply(c, Command::Retract(vec![Laid::Building(id)])).expect("taken back");
    assert!(k.buildings.get(&id).is_none());
    for (r, n) in &c.plans["sawpit"].cost {
        assert_eq!(k.amount(r), stock[r], "{r}: all {n} back");
    }
    // A road, too.
    k.apply(c, Command::Road((t.0, t.1 + 5))).unwrap();
    assert_eq!(k.amount("timber"), stock["timber"] - 1);
    k.apply(c, Command::Retract(vec![Laid::Road((t.0, t.1 + 5))])).unwrap();
    assert_eq!(k.amount("timber"), stock["timber"]);
    // A works that has done its work comes down as a demolition does, for half.
    let t2 = ground(&k, "sawpit", (160, 112));
    let id2 = build(&mut k, "sawpit", t2);
    k.buildings.get_mut(&id2).unwrap().stats.worked = 30;
    let before = k.amount("timber");
    k.apply(c, Command::Retract(vec![Laid::Building(id2)])).unwrap();
    let cost = c.plans["sawpit"].cost["timber"];
    assert_eq!(k.amount("timber"), before + cost / 2);
    // Nothing left to take back is said.
    assert!(k.apply(c, Command::Retract(vec![Laid::Building(id2)])).is_err());
}

#[test]
fn a_group_of_works_is_copied_into_a_pattern_kept_with_the_realm_and_stamped_anew() {
    let c = catalog();
    let mut k = rich();
    for r in c.resources.keys() {
        k.stock.insert(r.clone(), 400);
    }
    k.stock.insert("fodder".into(), 100_000);
    let a = ground(&k, "sawpit", (160, 104));
    let saw = build(&mut k, "sawpit", a);
    let b = ground(&k, "well", (a.0 + 6, a.1));
    let well = build(&mut k, "well", b);
    k.apply(c, Command::Road((a.0, a.1 + 3))).unwrap();
    let (x0, y0, x1, y1) = (a.0 - 1, a.1 - 1, b.0 + 1, a.1 + 4);
    let words = k.clip(c, "copy-a", (x1, y1), (x0, y0)).expect("copied, either corner first");
    assert!(words.contains("2 works") && words.contains("1 roads"), "{words}");
    let p = &k.copies["copy-a"];
    assert_eq!(p.builds.len(), 2);
    assert!(p.builds.iter().all(|b| b.at.0 >= 0 && b.at.1 >= 0), "offsets from the patch's top left");
    // Kept with the realm: it saves, reads back, and is valid.
    let back: Kingdom = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert_eq!(back.copies, k.copies);
    assert!(back.valid(c));
    // Stamped elsewhere, turned.
    let before = k.buildings.len();
    let at = ground(&k, "sawpit", (190, 118));
    k.apply(c, Command::Stamp { pattern: "copy-a".into(), origin: at, turns: 0 }).expect("stamped");
    assert_eq!(k.buildings.len(), before + 2);
    assert!(k.stamp_cost(c, "copy-a", ground(&k, "sawpit", (200, 130)), 1).is_ok());
    // Nothing standing there: said.
    assert!(k.clip(c, "copy-b", (230, 160), (235, 165)).is_err());
    assert!(k.clip(c, "no-such-name", (a.0, a.1), (a.0 + 3, a.1 + 3)).is_err());
    let _ = (saw, well);
    k.apply(c, Command::Forget("copy-a".into())).unwrap();
    assert!(k.copies.is_empty());
}

#[test]
fn the_books_count_what_was_made_used_and_held_a_minute_at_a_time() {
    let c = catalog();
    let mut k = rich();
    for r in c.resources.keys() {
        k.stock.insert(r.clone(), 200);
    }
    k.stock.insert("fodder".into(), 100_000);
    let ty = ground(&k, "carters-yard", (143, 99));
    build(&mut k, "carters-yard", ty);
    let t = ground(&k, "well", (150, 100));
    let saw = build(&mut k, "well", t);
    finish(&mut k);
    cart(&mut k, saw, 2);
    run(&mut k, 60 * 6);
    let w = k.books.window(saw, k.second);
    assert!(w.seconds >= 240, "{w:?}");
    let prod = k.production(c, 8);
    assert!(prod.minutes >= 4, "{}", prod.minutes);
    assert!(!prod.goods.is_empty());
    for g in &prod.goods {
        assert_eq!(g.made.len(), g.used.len());
        assert_eq!(g.made.len(), usize::from(prod.minutes));
        assert!(g.made_hour + g.used_hour > 0);
    }
    // The well makes water; it is in the books, and so is what the camp ate.
    assert!(prod.goods.iter().any(|g| g.made_hour > 0), "{prod:?}");
    assert!(prod.goods.iter().any(|g| g.used_hour > 0), "{prod:?}");
    // Never saved nor sent with the kingdom.
    let s = serde_json::to_string(&k).unwrap();
    assert!(!s.contains("\"books\""));
    let back: Kingdom = serde_json::from_str(&s).unwrap();
    assert!(back.books.goods.is_empty());
}

#[test]
fn a_hover_card_says_what_a_works_makes_in_and_out_a_minute_stock_crew_and_what_it_waits_on() {
    let c = catalog();
    let mut k = rich();
    for r in c.resources.keys() {
        k.stock.insert(r.clone(), 200);
    }
    k.stock.insert("fodder".into(), 100_000);
    // A banker with no rock to dress: starved, and says so.
    let t = ground(&k, "banker", (150, 100));
    let banker = build(&mut k, "banker", t);
    finish(&mut k);
    k.apply(c, Command::Recipe { building: banker, recipe: "dress-limestone".into() }).unwrap();
    k.apply(c, Command::Connect(banker)).unwrap();
    run(&mut k, 150);
    let card = k.card(c, t).expect("a card for the banker");
    assert_eq!(card.kind, "WORKS");
    let labels: Vec<&str> = card.rows.iter().map(|r| r.label.as_str()).collect();
    for want in ["STATE", "MAKES", "IN /MIN", "OUT /MIN", "STOCK", "CREW", "WAITING"] {
        assert!(labels.contains(&want), "{labels:?}");
    }
    let row = |l: &str| card.rows.iter().find(|r| r.label == l).unwrap();
    assert_eq!(row("STATE").tone, Tone::Warn);
    assert!(row("WAITING").text.to_lowercase().contains("limestone") || row("WAITING").text.to_lowercase().contains("nothing"), "{}", row("WAITING").text);
    // A line and a store get cards too, a plan its own.
    let camp = k.camp();
    assert_eq!(k.card(c, camp).unwrap().kind, "STORE");
    let g = plan(&mut k, "well", (170, 108), 2);
    let gt = k.plans.ghosts[&g].tile;
    let gc = k.card(c, gt).unwrap();
    assert_eq!(gc.kind, "PLAN");
    assert!(gc.rows.iter().any(|r| r.label == "URGENCY" && r.text == "URGENT"));
    assert!(k.card(c, (3, 3)).is_none(), "empty ground has no card");
}

#[test]
fn the_bottleneck_list_ranks_what_starves_the_works_and_each_has_a_place() {
    let c = catalog();
    let mut k = rich();
    for r in c.resources.keys() {
        k.stock.insert(r.clone(), 100);
    }
    k.stock.insert("fodder".into(), 100_000);
    // A mortar mill with no lime anywhere upstream.
    let t = ground(&k, "mortar-mill", (150, 100));
    let mill = build(&mut k, "mortar-mill", t);
    let t2 = ground(&k, "banker", (154, 106));
    let banker = build(&mut k, "banker", t2);
    finish(&mut k);
    k.apply(c, Command::Recipe { building: banker, recipe: "dress-limestone".into() }).unwrap();
    for id in [mill, banker] {
        k.apply(c, Command::Connect(id)).unwrap();
    }
    k.stock.remove("limestone");
    k.stock.remove("quicklime");
    k.stock.remove("slaked-lime");
    run(&mut k, 240);
    let list = k.bottlenecks(c, 8);
    assert!(!list.is_empty(), "something starves");
    for w in list.windows(2) {
        assert!(w[0].lost >= w[1].lost, "worst first: {:?}", list.iter().map(|l| l.lost).collect::<Vec<_>>());
    }
    for choke in &list {
        assert!(!choke.text.is_empty());
        assert!(k.world.contains(choke.at), "a place on the map: {choke:?}");
    }
    assert!(list.iter().any(|l| l.plan == "banker" || l.plan == "mortar-mill" || l.text.contains("NOTHING MAKES") || l.text.contains("LIME") || l.text.contains("LIMESTONE")), "{list:?}");
}

#[test]
fn there_is_always_one_next_goal_and_the_tree_says_what_each_charter_and_stage_opens() {
    let c = catalog();
    let mut k = hill();
    let g = k.goals(c);
    let next = g.next().expect("a first goal");
    assert_eq!(next.id, "seam", "the first minute: win a seam");
    assert!(!next.how.is_empty() && !next.why.is_empty() && !next.unlocks.is_empty());
    // The charters are in it with what they unlock.
    let masonry = g.nodes.iter().find(|n| n.id == "charter:masonry").expect("masonry");
    assert!(masonry.unlocks.iter().any(|u| u.contains("BANKER")), "{:?}", masonry.unlocks);
    assert!(g.nodes.iter().any(|n| n.kind == goals::Kind::Stage), "the Keep's stages are in the tree before the Keep is staked");
    // Parents come before children and depth grows along the tree.
    let at = |id: &str| g.nodes.iter().position(|n| n.id == id).unwrap();
    for n in &g.nodes {
        for a in &n.after {
            assert!(at(a) < at(&n.id), "{a} before {}", n.id);
        }
    }
    // Win the seam; the goal moves to the charter that opens the quarry.
    k.discover(c, "mines-3".into(), Claim { resource: "limestone".into(), name: "Face".into(), source: "Mines".into(), secured: false, rate: 4, yields: Default::default(), surface: false });
    k.secure("mines-3");
    assert_eq!(k.goals(c).next().unwrap().id, "charter:masonry");
    k.stock.insert("plank".into(), 40);
    k.stock.insert("stone".into(), 60);
    k.apply(c, Command::Research("masonry".into())).unwrap();
    assert_eq!(k.goals(c).next().unwrap().id, "charter:charter", "the charter is on the way to the quarry");
    // A charter nobody waits on is a branch, not a goal.
    let g = k.goals(c);
    assert!(g.nodes.iter().find(|n| n.id == "charter:husbandry").is_some_and(|n| !n.done));
    assert_ne!(g.next().unwrap().id, "charter:husbandry");
    assert!(g.done_count() >= 2);
}

#[test]
fn the_hillside_charters_page_can_fund_every_charter_the_works_stand_on() {
    let c = catalog();
    let list = c.hill_charters();
    let ids: Vec<&str> = list.iter().map(|(id, _)| id.as_str()).collect();
    for want in ["masonry", "charter", "limeburning", "joinery"] {
        assert!(ids.contains(&want), "{want} in {ids:?}");
    }
    // Every charter's own requirements come before it.
    for (i, (_, t)) in list.iter().enumerate() {
        for r in &t.requires {
            assert!(ids[..i].contains(&r.as_str()), "{r} before {:?}", ids[i]);
        }
    }
    // Every hillside work's charter can be funded from it.
    for p in c.plans.values().filter(|p| p.hillside && !p.requires.is_empty()) {
        for r in &p.requires {
            assert!(ids.contains(&r.as_str()) || c.tech(r).is_none(), "{} needs {r}", p.name);
        }
    }
}

#[test]
fn the_works_an_old_village_leaves_behind_are_paid_back() {
    let c = catalog();
    let mut old = Kingdom::default();
    old.start(c);
    old.stock.insert("timber".into(), 300);
    old.stock.insert("plank".into(), 60);
    old.stock.insert("stone".into(), 120);
    old.apply(c, Command::Build { plan: "forester".into(), tile: (8, 8) }).unwrap();
    old.apply(c, Command::Build { plan: "farm".into(), tile: (10, 8) }).unwrap();
    // A work of the valley the hillside builds no such thing as.
    let valley_only = c.plans.iter().find(|(_, p)| p.valley && !p.hillside && p.connector.is_none() && p.cost.values().sum::<u32>() > 0 && p.requires.is_empty()).map(|(k, _)| k.clone());
    let Some(plan) = valley_only else { return };
    let tile = (12, 8);
    old.stock.insert("timber".into(), 800);
    old.stock.insert("plank".into(), 200);
    old.stock.insert("stone".into(), 400);
    old.stock.insert("brick".into(), 200);
    old.stock.insert("block".into(), 200);
    let Ok(_) = old.apply(c, Command::Build { plan: plan.clone(), tile }) else { return };
    let cost = c.plans[&plan].cost.clone();
    let stock_before: u32 = old.stock.values().sum();
    let mut k = old.clone();
    let moved = k.migrate_to_hillside(c, 7).expect("it moves");
    assert!(moved.left.iter().any(|(p, _)| *p == plan), "{moved:?}");
    for (r, n) in &cost {
        assert!(moved.refund.get(r).copied().unwrap_or(0) >= *n, "{r}: its {n} given back");
        assert!(k.amount(r) >= *n);
    }
    assert!(k.valid(c));
    let _ = stock_before;
}

#[test]
fn the_numbers_are_cheap_enough_to_read_every_frame_or_every_second() {
    let c = catalog();
    let mut k = rich();
    for r in c.resources.keys() {
        k.stock.insert(r.clone(), 100);
    }
    k.stock.insert("fodder".into(), 100_000);
    let mut ids = Vec::new();
    for plan in ["well", "sawpit", "banker", "cottage", "mortar-mill", "lime-kiln", "carpentry", "woodsman-camp"] {
        let t = ground(&k, plan, (150, 100));
        ids.push(build(&mut k, plan, t));
    }
    finish(&mut k);
    for id in &ids {
        let _ = k.apply(c, Command::Connect(*id));
    }
    run(&mut k, 200);
    let t = std::time::Instant::now();
    for _ in 0..200 {
        std::hint::black_box(k.goals(c).next);
    }
    let goals = t.elapsed() / 200;
    let t = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(k.bottlenecks(c, 12));
        std::hint::black_box(k.production(c, 12));
        std::hint::black_box(k.card(c, k.buildings[&ids[2]].tile));
    }
    let numbers = t.elapsed() / 20;
    println!("goals {goals:?} a call (every frame); card, output and chokes {numbers:?} together (once a second, for the seat that asked)");
    assert!(goals.as_micros() < 1500, "{goals:?}");
    assert!(numbers.as_millis() < 100, "{numbers:?}");
}
