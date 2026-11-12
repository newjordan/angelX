//! The Keep's working yard in 3D: the crane stood up tall (not laid flat), its
//! yard of stone and carts on the side away from the keep, the ladder and the
//! hodmen on it, and what it costs.

use super::*;
use crate::world::works;

fn tower() -> &'static Castle {
    works::castle(works::KEEP).expect("the great tower")
}

fn run() -> Run {
    crate::world3d::kingdom::proof_run(&[[133.0 * 16.0, 87.0 * 16.0]])
}

/// How far a node reaches up (kingdom pixels): half its extent along each of its axes, as turned.
fn up(n: &Node) -> f32 {
    let t = &n.transform;
    [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        .iter()
        .zip(t.scale)
        .map(|(axis, size)| angel_stage::scene::quat_rotate(t.rotation, *axis)[1].abs() * size / 2.0)
        .sum()
}

/// How high above the ground (kingdom pixels) the highest node reaches.
fn height(nodes: &[Node]) -> f32 {
    nodes.iter().map(|n| n.transform.translation[1] + up(n)).fold(0.0, f32::max)
}

/// Whether a node is a long piece standing upright: a post or a rung's rail.
fn upright(n: &Node) -> bool {
    angel_stage::scene::quat_rotate(n.transform.rotation, [1.0, 0.0, 0.0])[1].abs() > 0.8 && n.transform.scale[0] > 2.5 * K
}

#[test]
fn the_crane_stands_the_height_of_its_lift_with_a_pennon_on_top() {
    let c = tower();
    let r = run();
    for laid in [1500u32, 5000, 7000, 9000] {
        let base = wl::crane(c, laid, 0.0).expect("a crane");
        let y = of(c, &r, laid, laid + 320, 0.0);
        assert!(!y.nodes.is_empty());
        // The mast reaches its top, and the pennon over it.
        let tall = height(&y.nodes);
        assert!(tall >= base.top * K, "laid {laid}: the highest node is {tall}, the mast {}", base.top * K);
        // The mast is a tower, not a stick laid on the ground: a good part of the nodes stand up.
        let posts = y.nodes.iter().filter(|n| upright(n)).count();
        assert!(posts >= 8, "laid {laid}: {posts} upright pieces");
        assert!(y.zone.is_some() && y.lane.is_some());
        assert!(y.lamps.len() >= 3, "the head, the yard and the deck");
    }
}

#[test]
fn the_crane_turns_lifts_and_sets_a_stone_where_the_flat_view_does() {
    let c = tower();
    let r = run();
    // Wherever the cycle is, the stone hangs from the jib's tip once it is off the ground.
    let (mut hung, mut grounded) = (false, false);
    for k in 0..90 {
        let t = k as f32 * 0.1;
        let pose = pose(c, 5000, t, Plan::of(wl::crane(c, 5000, t).unwrap().at).pick).unwrap();
        if pose.stone[2] > 1.0 {
            hung = true;
            let tip = [pose.at[0] + pose.reach * pose.yaw.cos(), pose.at[1] + pose.reach * pose.yaw.sin()];
            assert!((pose.stone[0] - tip[0]).hypot(pose.stone[1] - tip[1]) < 0.01, "the stone hangs under the jib's tip at {t}");
        } else {
            grounded = true;
        }
        let _ = of(c, &r, 5000, 5320, t);
    }
    assert!(hung && grounded);
}

#[test]
fn the_yard_lies_on_the_side_of_the_crane_away_from_the_keep_and_off_the_keeps_face() {
    let c = tower();
    let r = run();
    for (laid, west) in [(5000u32, true), (9000, false)] {
        let y = of(c, &r, laid, laid, 0.0);
        let [x0, _, x1, _] = y.zone.expect("a yard");
        let tower_face = if west { c.world([-9.6, 0.0, 0.0])[0] } else { c.world([10.8, 0.0, 0.0])[0] };
        if west {
            assert!(x1 <= tower_face + 1.0 && x0 < tower_face - 30.0, "{x0} {x1} {tower_face}");
        } else {
            assert!(x0 >= tower_face - 1.0 && x1 > tower_face + 30.0, "{x0} {x1} {tower_face}");
        }
    }
}

#[test]
fn the_stack_is_the_yards_own_stock_and_carts_come_while_stone_is_wanted() {
    let c = tower();
    let mut r = run();
    r.hands.yard.cut = 0;
    let bare = of(c, &r, 5000, 5000, 0.0).nodes.len();
    r.hands.yard.cut = 24;
    let full = of(c, &r, 5000, 5000, 0.0).nodes.len();
    assert!(full > bare + 5, "{bare} -> {full}");
    // A finished castle has no carts, no stack and no stone on the hook.
    let done = of(c, &r, c.len(), c.len(), 0.0);
    assert!(done.nodes.len() < bare / 2, "{} nodes still", done.nodes.len());
    // Through a turn of the carts' cycle the yard changes: a cart on the lane, standing at the bay with
    // hodmen carrying, going out empty.
    let mut counts = std::collections::BTreeSet::new();
    for k in 0..600 {
        r.tick = k * 15;
        counts.insert(of(c, &r, 5000, 5000, 0.0).nodes.len());
    }
    assert!(counts.len() >= 3, "{counts:?}");
}

#[test]
fn a_whole_site_is_a_few_hundred_nodes_and_cheap_to_make() {
    let c = tower();
    let r = run();
    let started = std::time::Instant::now();
    let mut most = 0;
    for k in 0..200 {
        let y = of(c, &r, 3000 + k * 30, 3000 + k * 30 + 320, k as f32 * 0.37);
        most = most.max(y.nodes.len());
    }
    let each = started.elapsed() / 200;
    println!("works_yard: at most {most} nodes, {each:?} a frame");
    assert!(most < 700, "{most}");
    assert!(each.as_millis() < 5, "{each:?}");
}

#[test]
fn a_path_is_walked_by_its_length_and_heads_the_way_it_goes() {
    let p = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]];
    assert_eq!(path_len(&p), 7.0);
    assert_eq!(along(&p, 2.0), ([2.0, 0.0], Dir::East));
    assert_eq!(along(&p, 5.0), ([4.0, 1.0], Dir::North));
    assert_eq!(along(&p, 99.0).0, [4.0, 3.0]);
}

// ------------------------------------------------------ the sim's yard ----

fn yard_of(cut: u32, carts: Vec<hill::YardCart>) -> KeepYard {
    KeepYard { rect: [147, 96, 149, 96], cut, rough: 0, mortar: 0, timber: 0, can_pay: cut > 0, carts }
}

fn cart(index: usize, dist: f32, phase: Delivery, left: u32) -> hill::YardCart {
    hill::YardCart { index, dist, phase, left, dwell: 5, load: Some(Load::Ashlar) }
}

#[test]
fn what_the_yard_holds_is_what_is_drawn_and_an_empty_yard_is_bare() {
    let r = run();
    let slots = 9;
    let none = held(&r, Some(&yard_of(0, Vec::new())), slots);
    assert_eq!((none.blocks, none.lumps, none.logs), (0, 0, 0), "nothing in the yard, nothing on the ground");
    let few = held(&r, Some(&yard_of(5, Vec::new())), slots);
    let many = held(&r, Some(&yard_of(300, Vec::new())), slots);
    let most = held(&r, Some(&yard_of(9_000, Vec::new())), slots);
    assert_eq!(few.blocks, 1);
    assert!(few.blocks < many.blocks && many.blocks < most.blocks.max(many.blocks + 1));
    assert_eq!(most.blocks, slots as u32 * 2, "the stack is as tall as its place");
    // Without the sim it is the Works' own stock, and the old yard's heap and pile stand.
    let stub = held(&r, None, slots);
    assert_eq!((stub.lumps, stub.logs), (3, 3));
}

#[test]
fn the_sims_carts_stand_in_the_bailey_where_the_sim_has_them() {
    let c = tower();
    let base = wl::crane(c, 1500, 0.0).expect("a crane");
    let plan = Plan::of(base.at);
    let count = |y: &KeepYard| {
        let mut pen = Pen::new(c);
        real_carts(&mut pen, &plan, y, 0.0, 0);
        pen.out
    };
    assert!(count(&yard_of(40, Vec::new())).is_empty(), "no cart on the way, none in the bailey");
    // A cart a dozen tiles out is on the lane, one far down the road is not (it is drawn there).
    let on_lane = count(&yard_of(40, vec![cart(0, 6.0, Delivery::Hauling, 0)]));
    assert!(!on_lane.is_empty());
    assert!(count(&yard_of(40, vec![cart(0, 40.0, Delivery::Hauling, 0)])).is_empty());
    // At its berth the unloading cart has two hodmen to carry its load, while the sim has it unloading.
    let berth = count(&yard_of(40, vec![cart(0, 0.0, Delivery::Unloading, 3)]));
    let waiting = count(&yard_of(40, vec![cart(0, 0.0, Delivery::Waiting, 0)]));
    assert!(berth.len() > waiting.len(), "hodmen at the unloading cart: {} against {}", berth.len(), waiting.len());
    // Two carts stand one behind the other, not on one spot.
    let spot = |nodes: &[Node]| nodes.iter().map(|n| n.transform.translation[0].round() as i64).max();
    let two = count(&yard_of(40, vec![cart(0, 0.0, Delivery::Unloading, 3), cart(1, 0.0, Delivery::Waiting, 0)]));
    assert!(two.len() > berth.len());
    assert_ne!(spot(&two), spot(&berth), "the second cart stands behind the first");
}

/// A run with the hillside kingdom founded, rich, and the Keep staked in the sim from the game's wall.
fn live() -> Run {
    use crate::kingdom::catalog;
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    for r in catalog().resources.keys() {
        run.home.industry.stock.insert(r.clone(), 40);
    }
    run.home.industry.stock.insert("fodder".into(), 100_000);
    run.home.industry.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    for _ in 0..u64::from(HZ) * 12 {
        run.step(&std::collections::BTreeMap::new());
    }
    for b in run.home.industry.buildings.values_mut() {
        b.construction = 0;
    }
    assert!(run.keep_live().is_some(), "the Keep is staked: {:?}", run.hands.seen.note);
    run
}

/// What the yard stands as, as numbers a comparison can use.
fn shape(y: &Yard) -> Vec<([i64; 3], [i64; 3])> {
    let q = |v: [f32; 3]| v.map(|x| (x * 100.0).round() as i64);
    y.nodes.iter().map(|n| (q(n.transform.translation), q(n.transform.scale))).collect()
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib works_yard -- --include-ignored"]
fn a_stocked_yard_works_the_crane_and_a_dry_one_stands_it_idle() {
    let c = tower();
    let mut r = live();
    let site = r.keep_live().expect("live");
    let laid = r.sites.keep().expect("a site").laid;
    let yard_id = r.home.industry.sites[&site].yard;
    // Stocked: the stage in hand's whole bill, the scaffold's poles and an arch's centring, and plenty of dressed stone.
    {
        use crate::kingdom::{catalog, site::stage_needs};
        let c = catalog();
        let sim = &r.home.industry.sites[&site];
        let mut goods: Vec<(String, u32)> = stage_needs(c, &c.castles[&sim.plan].stages[usize::from(sim.stage)]).into_iter().collect();
        goods.extend([("pole".to_string(), 30), ("centring".to_string(), 8), ("ashlar".to_string(), 400), ("rubble".to_string(), 200)]);
        let y = r.home.industry.buildings.get_mut(&yard_id).expect("the yard");
        for (g, n) in goods {
            *y.input.entry(g).or_default() += n;
        }
    }
    let hv = crate::world::hillside::of(&r).expect("the hillside");
    let y = hv.keep_yard.as_ref().expect("the sim's yard is read");
    assert!(y.cut >= 400 && y.can_pay, "a stocked yard can pay: {y:?}");
    let working = of(c, &r, laid, laid + 320, 0.0);
    let working_later = of(c, &r, laid, laid + 320, 3.7);
    assert_ne!(shape(&working), shape(&working_later), "the crane turns while the yard can pay");
    assert!(working.lamps.len() >= 3, "the lantern is lit");
    // The supply stops: the yard is emptied and the masons cannot be paid.
    r.home.industry.buildings.get_mut(&yard_id).expect("the yard").input.clear();
    // (The view is kept for the sim's revision and second it was read at; a sim that changed would have moved them.)
    r.home.industry.rev += 1;
    let r2 = r.clone();
    let hv = crate::world::hillside::of(&r2).expect("the hillside");
    let y = hv.keep_yard.as_ref().expect("the sim's yard is read");
    assert!(y.cut == 0 && !y.can_pay, "a dry yard cannot pay: {y:?}");
    let idle = of(c, &r2, laid, laid + 320, 0.0);
    let idle_later = of(c, &r2, laid, laid + 320, 3.7);
    assert_eq!(shape(&idle), shape(&idle_later), "the crane stands idle: nothing moves in a dry yard");
    assert!(idle.lamps.len() < working.lamps.len(), "the lantern is out");
    assert!(idle.nodes.len() < working.nodes.len(), "no stack, no heap, no cart, no stone in the cradle");
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib works_yard -- --include-ignored"]
fn the_carts_the_sim_sends_to_the_keeps_yard_are_read_and_drawn_in_the_bailey_not_on_the_road() {
    use crate::kingdom::{CartPhase, Command, catalog};
    let c = catalog();
    let mut r = live();
    let site = r.keep_live().expect("live");
    let yard_id = r.home.industry.sites[&site].yard;
    // A carter's yard with a cart, the cart at the stockyard's door bound for it and unloading.
    let carter = {
        let k = &mut r.home.industry;
        k.stock.insert("timber".into(), 600);
        k.stock.insert("plank".into(), 600);
        k.stock.insert("stone".into(), 600);
        k.apply(c, Command::Build { plan: "carters-yard".into(), tile: (143, 98) }).expect("a carter's yard");
        let id = k.at((143, 98)).expect("it stands");
        k.buildings.get_mut(&id).expect("it").construction = 0;
        k.apply(c, Command::Connect(id)).ok();
        k.apply(c, Command::Carts { yard: id, carts: 1 }).expect("a cart");
        id
    };
    let door = {
        let k = &r.home.industry;
        k.covered(&k.buildings[&yard_id]).next().expect("a tile")
    };
    let cart = &mut r.home.industry.buildings.get_mut(&carter).expect("the carter's yard").carts[0];
    cart.tile = (door.0 - 1, door.1);
    cart.phase = CartPhase::Unloading;
    cart.to = yard_id;
    cart.clock = 3;
    cart.load.insert("ashlar".into(), 10);
    let hv = crate::world::hillside::of(&r).expect("the hillside");
    let y = hv.keep_yard.as_ref().expect("the yard");
    assert_eq!(y.carts.len(), 1, "the sim's cart for the yard is read");
    assert_eq!(y.carts[0].phase, Delivery::Unloading);
    assert!(y.carts[0].dist <= 1.0, "it stands at the yard's door: {}", y.carts[0].dist);
    assert!(shows_real_carts(&r), "the bailey draws the sim's carts");
}
