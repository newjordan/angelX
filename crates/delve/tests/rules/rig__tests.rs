//! The feel of building in the walking world: a hotbar, plans the camp's hands raise, rows
//! laid by holding E (or a mouse button) and walking (or dragging), taking back, grabbing
//! what is under the cursor, copying a group into a pattern, and the numbers on demand.
use super::*;
use crate::kingdom::{Command, Laid, catalog};
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

fn run(seats: u32) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    for n in 1..seats {
        run.join(&format!("K{}", n + 1));
    }
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    rich(&mut run);
    let room = world::room_of(&run.dungeon, RoomKind::Kingdom).expect("the kingdom's room");
    for y in 95..140 {
        for x in 148..220 {
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

fn still(run: &mut Run, n: usize) {
    let all: BTreeMap<u32, Input> = run.players.keys().map(|&id| (id, Input::default())).collect();
    for _ in 0..n {
        run.step(&all);
    }
}

fn press(run: &mut Run, seat: u32, input: Input) {
    run.step(&keys(seat, input));
    still(run, 1);
}

fn out(run: &mut Run, seat: u32) {
    press(run, seat, Input { cast: 4, ..Default::default() });
    assert!(building(run, seat), "the builder is out");
}

fn tool_index(tool: Tool) -> usize {
    tools().iter().position(|t| *t == tool).expect("a tool of the builder")
}

fn board(run: &Run, seat: u32) -> &Board {
    &run.kingdom.industry[&seat]
}

fn set_tool(run: &mut Run, seat: u32, tool: Tool) {
    let b = run.kingdom.industry.get_mut(&seat).unwrap();
    b.plan = tool_index(tool);
    b.rig.keep(b.plan);
}

fn seconds(run: &mut Run, n: u64) {
    for _ in 0..n * u64::from(HZ) {
        still(run, 1);
    }
}

#[test]
fn the_number_keys_take_the_hotbars_slots_and_the_arrows_change_its_bank() {
    let mut run = run(1);
    out(&mut run, 1);
    let bar = board(&run, 1).rig.bar;
    assert_eq!(tools()[usize::from(bar[0])], Tool::Road, "slot one is the road");
    press(&mut run, 1, Input { cast: 2, ..Default::default() });
    assert_eq!(board(&run, 1).plan, usize::from(bar[1]));
    assert_eq!(board(&run, 1).rig.slot, 1);
    // Down an arrow, the next bank: the same place in it.
    press(&mut run, 1, Input { aim_y: 1, ..Default::default() });
    assert_eq!(board(&run, 1).rig.slot, 4);
    assert_eq!(board(&run, 1).plan, usize::from(bar[4]));
    press(&mut run, 1, Input { cast: 3, ..Default::default() });
    assert_eq!(board(&run, 1).rig.slot, 5, "key 3 of the second bank");
    // Q takes another tool, and the slot in hand holds it.
    let before = board(&run, 1).plan;
    press(&mut run, 1, Input { swing: true, ..Default::default() });
    let now = board(&run, 1).plan;
    assert_ne!(before, now);
    assert_eq!(usize::from(board(&run, 1).rig.bar[5]), now);
    // The first bank still holds what it did.
    press(&mut run, 1, Input { aim_y: -1, ..Default::default() });
    press(&mut run, 1, Input { cast: 1, ..Default::default() });
    assert_eq!(board(&run, 1).plan, usize::from(bar[0]));
    // A hotbar is the seat's own.
    assert!(board(&run, 1).valid());
}

#[test]
fn what_the_camp_cannot_pay_for_is_laid_as_a_plan_and_the_hands_raise_it_when_it_can() {
    let mut run = run(1);
    out(&mut run, 1);
    set_tool(&mut run, 1, Tool::Plan("sawpit"));
    run.home.industry.stock.clear();
    run.home.industry.stock.insert("fodder".into(), 1000);
    for _ in 0..3 {
        run.step(&keys(1, Input { move_x: 1, ..Default::default() }));
    }
    still(&mut run, 1);
    let g = run.hill_ghost(1).unwrap();
    assert_eq!(g.verdict, Verdict::Short, "{}", g.note);
    assert!(g.note.contains("A PLAN"), "{}", g.note);
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert_eq!(run.home.industry.plans.ghosts.len(), 1, "laid as a plan: {}", board(&run, 1).message);
    assert!(run.home.industry.buildings.values().all(|b| b.plan != "sawpit"));
    // Everyone sees it; the camp's goods come; the hands raise it.
    rich(&mut run);
    seconds(&mut run, 40);
    assert!(run.home.industry.plans.ghosts.is_empty(), "raised: {:?}", run.home.industry.plans);
    assert!(run.home.industry.buildings.values().any(|b| b.plan == "sawpit" && b.construction == 0));
    assert!(run.home.industry.valid(catalog()));
}

#[test]
fn the_comma_key_lays_plans_even_when_the_camp_is_rich_and_f_ranks_one_and_x_cancels_it() {
    let mut run = run(1);
    out(&mut run, 1);
    set_tool(&mut run, 1, Tool::Plan("well"));
    press(&mut run, 1, Input { skill: 2, ..Default::default() });
    assert!(board(&run, 1).rig.planning);
    for _ in 0..3 {
        run.step(&keys(1, Input { move_x: 1, ..Default::default() }));
    }
    still(&mut run, 1);
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert_eq!(run.home.industry.plans.ghosts.len(), 1, "a plan, though the camp is rich");
    let id = *run.home.industry.plans.ghosts.keys().next().unwrap();
    assert_eq!(run.home.industry.plans.ghosts[&id].priority, 1);
    // F on the plan: more urgent. X on it: cancelled.
    let at = run.home.industry.plans.ghosts[&id].tile;
    run.kingdom.industry.get_mut(&1).unwrap().tile = at;
    run.kingdom.industry.get_mut(&1).unwrap().pointed = true;
    press(&mut run, 1, Input { fire: true, ..Default::default() });
    assert_eq!(run.home.industry.plans.ghosts[&id].priority, 2, "{}", board(&run, 1).message);
    press(&mut run, 1, Input { bomb: true, ..Default::default() });
    assert!(run.home.industry.plans.ghosts.is_empty(), "{}", board(&run, 1).message);
    // The comma again: building at once.
    press(&mut run, 1, Input { skill: 2, ..Default::default() });
    assert!(!board(&run, 1).rig.planning);
}

#[test]
fn holding_e_while_walking_lays_a_row_of_plans_that_the_hands_raise_one_after_another() {
    let mut run = run(1);
    out(&mut run, 1);
    set_tool(&mut run, 1, Tool::Plan("well"));
    press(&mut run, 1, Input { skill: 2, ..Default::default() });
    for _ in 0..140 {
        run.step(&keys(1, Input { move_x: 1, interact: true, ..Default::default() }));
    }
    still(&mut run, 2);
    let n = run.home.industry.plans.ghosts.len();
    assert!(n >= 5, "a row of plans behind the knight: {n}");
    let ys: std::collections::BTreeSet<i16> = run.home.industry.plans.ghosts.values().map(|g| g.tile.1).collect();
    assert!(ys.len() <= 2, "in a line: {ys:?}");
    // It is one step to take back.
    assert_eq!(board(&run, 1).rig.undo.len(), 1);
    seconds(&mut run, 120);
    let wells = run.home.industry.buildings.values().filter(|b| b.plan == "well").count();
    assert!(wells >= 3, "{wells} raised, {} left", run.home.industry.plans.ghosts.len());
    assert!(run.home.industry.valid(catalog()));
}

#[test]
fn z_takes_back_a_whole_row_with_its_price() {
    let mut run = run(1);
    out(&mut run, 1);
    set_tool(&mut run, 1, Tool::Road);
    let (roads, timber) = (run.home.industry.roads.len(), run.home.industry.amount("timber"));
    for _ in 0..80 {
        run.step(&keys(1, Input { move_x: 1, interact: true, ..Default::default() }));
    }
    still(&mut run, 2);
    let laid = run.home.industry.roads.len() - roads;
    assert!(laid >= 5, "{laid}");
    assert!(run.home.industry.amount("timber") < timber);
    press(&mut run, 1, Input { play: 1, ..Default::default() });
    assert_eq!(run.home.industry.roads.len(), roads, "the whole row is gone: {}", board(&run, 1).message);
    assert_eq!(run.home.industry.amount("timber"), timber, "and every board is back");
    press(&mut run, 1, Input { play: 1, ..Default::default() });
    assert!(board(&run, 1).message.contains("NOTHING"), "{}", board(&run, 1).message);
}

#[test]
fn g_takes_in_hand_what_the_cursor_is_on() {
    let mut run = run(1);
    out(&mut run, 1);
    let c = catalog();
    run.home.industry.apply(c, Command::Build { plan: "well".into(), tile: (160, 106) }).unwrap();
    run.home.industry.apply(c, Command::Road((162, 106))).unwrap();
    let b = run.kingdom.industry.get_mut(&1).unwrap();
    b.tile = (160, 106);
    b.pointed = true;
    press(&mut run, 1, Input { vigil: true, ..Default::default() });
    assert_eq!(tools()[board(&run, 1).plan], Tool::Plan("well"), "{}", board(&run, 1).message);
    let b = run.kingdom.industry.get_mut(&1).unwrap();
    b.tile = (162, 106);
    press(&mut run, 1, Input { vigil: true, ..Default::default() });
    assert_eq!(tools()[board(&run, 1).plan], Tool::Road);
    let b = run.kingdom.industry.get_mut(&1).unwrap();
    b.tile = (170, 120);
    press(&mut run, 1, Input { vigil: true, ..Default::default() });
    assert!(board(&run, 1).message.contains("NOTHING"));
}

#[test]
fn a_group_is_copied_with_m_and_stamped_with_e_and_kept_with_the_realm() {
    let mut run = run(1);
    out(&mut run, 1);
    let c = catalog();
    run.home.industry.apply(c, Command::Build { plan: "well".into(), tile: (160, 106) }).unwrap();
    run.home.industry.apply(c, Command::Build { plan: "well".into(), tile: (164, 106) }).unwrap();
    for x in 160..=165 {
        let _ = run.home.industry.apply(c, Command::Road((x, 108)));
    }
    let b = run.kingdom.industry.get_mut(&1).unwrap();
    b.tile = (159, 105);
    b.pointed = true;
    press(&mut run, 1, Input { skill: 1, ..Default::default() });
    assert!(board(&run, 1).rig.mark.is_some());
    assert!(run.hill_ghost(1).unwrap().select.is_some(), "the patch shows");
    run.kingdom.industry.get_mut(&1).unwrap().tile = (166, 109);
    press(&mut run, 1, Input { skill: 1, ..Default::default() });
    assert!(board(&run, 1).rig.mark.is_none());
    assert_eq!(run.home.industry.copies.len(), 1, "{}", board(&run, 1).message);
    let in_hand = tools()[board(&run, 1).plan].clone();
    assert_eq!(in_hand, Tool::Pattern("copy-a"), "the copy is in hand");
    let before = run.home.industry.buildings.len();
    // Stamp it elsewhere.
    let b = run.kingdom.industry.get_mut(&1).unwrap();
    b.tile = (180, 120);
    b.pointed = true;
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    assert_eq!(run.home.industry.buildings.len(), before + 2, "{}", board(&run, 1).message);
    // Taking it back is one press.
    press(&mut run, 1, Input { play: 1, ..Default::default() });
    assert_eq!(run.home.industry.buildings.len(), before);
    // It is kept: the realm saves it, and so does a seat read back.
    let saved = serde_json::to_string(&run.home.industry).unwrap();
    let back: crate::kingdom::Kingdom = serde_json::from_str(&saved).unwrap();
    assert!(back.copies.contains_key("copy-a"));
    // The tool is in the list; an empty place's is skipped by Q.
    let k = &run.home.industry;
    assert!(usable(k, &Tool::Pattern("copy-a")) && !usable(k, &Tool::Pattern("copy-b")));
}

#[test]
fn the_card_is_made_for_the_seat_that_asked_and_sent_to_that_seat_only() {
    let mut run = run(2);
    out(&mut run, 1);
    let c = catalog();
    run.home.industry.apply(c, Command::Build { plan: "banker".into(), tile: (160, 106) }).unwrap();
    for b in run.home.industry.buildings.values_mut() {
        b.construction = 0;
    }
    run.kingdom.industry.get_mut(&1).unwrap().tile = (160, 106);
    run.kingdom.industry.get_mut(&1).unwrap().pointed = true;
    press(&mut run, 1, Input { team: true, ..Default::default() });
    assert!(board(&run, 1).rig.card);
    still(&mut run, 2);
    let n = board(&run, 1).rig.numbers.as_ref().expect("numbers for seat one");
    let card = n.card.as_ref().expect("a card");
    assert_eq!(card.kind, "WORKS");
    assert!(card.rows.iter().any(|r| r.label == "STATE"));
    assert!(run.kingdom.industry.get(&2).is_none_or(|b| b.rig.numbers.is_none()), "the other seat asked for nothing");
    // Across the wire the other seat's view of the tree has none of seat one's numbers.
    let tree = crate::wire::publish(&run).expect("a tree");
    let has = |tree: &angel_net::doc::value::Value| {
        tree.field("kingdom")
            .and_then(|k| k.field("industry"))
            .and_then(|i| match i {
                angel_net::doc::value::Value::Map(m) => m.iter().find(|(k, _)| k.as_id() == Some(1)).map(|(_, b)| b.field("rig").is_some_and(|r| r.field("numbers").is_some())),
                _ => None,
            })
            .unwrap_or(false)
    };
    let mine = crate::wire::seat_tree(&tree, Some(0));
    let theirs = crate::wire::seat_tree(&tree, Some(1));
    assert!(has(&mine), "seat one is sent its own numbers");
    assert!(!has(&theirs), "seat two is not");
    // The card goes when asked to.
    press(&mut run, 1, Input { team: true, ..Default::default() });
    still(&mut run, 2);
    assert!(board(&run, 1).rig.numbers.is_none());
}

#[test]
fn the_ledger_has_goals_output_and_chokes_and_a_choke_sends_the_view_there() {
    let mut run = run(1);
    out(&mut run, 1);
    let c = catalog();
    // A starving bench to find.
    let k = &mut run.home.industry;
    k.apply(c, Command::Build { plan: "banker".into(), tile: (160, 106) }).unwrap();
    for b in k.buildings.values_mut() {
        b.construction = 0;
    }
    let id = k.at((160, 106)).unwrap();
    k.apply(c, Command::Recipe { building: id, recipe: "dress-limestone".into() }).unwrap();
    k.apply(c, Command::Connect(id)).unwrap();
    for r in ["limestone", "block"] {
        k.stock.remove(r);
    }
    seconds(&mut run, 90);
    // C opens the ledger; P turns it: charters, claims, people, works, goals, output, chokes.
    press(&mut run, 1, Input { ride: true, ..Default::default() });
    let mut seen = vec![board(&run, 1).page];
    for _ in 0..6 {
        press(&mut run, 1, Input { cycle: true, ..Default::default() });
        seen.push(board(&run, 1).page);
    }
    assert_eq!(seen, vec![1, 3, 6, 8, PAGE_GOALS, PAGE_PRODUCTION, PAGE_CHOKES], "{seen:?}");
    still(&mut run, 2);
    let chokes = board(&run, 1).rig.numbers.as_ref().expect("numbers").chokes.clone();
    assert!(!chokes.is_empty(), "something chokes the works");
    // E goes there: the view looks at the place, the ledger is away, walking brings it back.
    press(&mut run, 1, Input { interact: true, ..Default::default() });
    let b = board(&run, 1);
    assert_eq!(b.page, 0);
    assert_eq!(b.tile, chokes[0].at);
    assert!(b.rig.look.is_some());
    let cam = crate::render::overworld::arena::camera(&run, Some(1));
    let want = (chokes[0].at.0 as i32 * 16 + 8) - 192;
    assert!((cam.0 - want.clamp(0, i32::MAX)).abs() <= 16 || cam.0 == 0, "{cam:?} for {want}");
    run.step(&keys(1, Input { move_x: 1, ..Default::default() }));
    assert!(board(&run, 1).rig.look.is_none(), "walking brings the view back");
    assert!(run.kingdom.industry[&1].valid());
}

#[test]
fn a_mouse_button_held_and_dragged_lays_a_row_as_holding_e_does() {
    let mut run = run(1);
    out(&mut run, 1);
    set_tool(&mut run, 1, Tool::Road);
    let roads = run.home.industry.roads.len();
    let act = |run: &mut Run, a: crate::rules::builder_input::BuilderAction| {
        run.step(&keys(1, Input { builder: Some(a), ..Default::default() }));
    };
    use crate::rules::builder_input::BuilderAction as A;
    act(&mut run, A::Plot { tile: (162, 112), build: true });
    for x in 163..=168 {
        act(&mut run, A::Hold((x, 112)));
    }
    act(&mut run, A::Let((168, 112)));
    still(&mut run, 1);
    let laid = run.home.industry.roads.len() - roads;
    assert_eq!(laid, 7, "a road from (162, 112) to (168, 112): {laid}");
    assert!((162..=168).all(|x| run.home.industry.roads.contains(&(x, 112))));
    assert!(board(&run, 1).rig.drag.is_none(), "the button is up");
    // The hotbar clicked.
    act(&mut run, A::Slot(2));
    assert_eq!(board(&run, 1).rig.slot, 2);
    // A right click points: the card, and nothing is laid.
    let before = run.home.industry.roads.len();
    act(&mut run, A::Plot { tile: (165, 112), build: false });
    assert!(board(&run, 1).rig.card);
    assert_eq!(run.home.industry.roads.len(), before);
    // One undo takes the dragged row.
    press(&mut run, 1, Input { play: 1, ..Default::default() });
    assert_eq!(run.home.industry.roads.len(), roads);
}

#[test]
fn the_recipe_the_claim_and_the_carts_have_keys_of_their_own() {
    let mut run = run(1);
    out(&mut run, 1);
    let c = catalog();
    let k = &mut run.home.industry;
    k.apply(c, Command::Build { plan: "carters-yard".into(), tile: (160, 106) }).unwrap();
    for b in k.buildings.values_mut() {
        b.construction = 0;
    }
    let id = k.at((160, 106)).unwrap();
    let b = run.kingdom.industry.get_mut(&1).unwrap();
    b.tile = (160, 106);
    b.pointed = true;
    press(&mut run, 1, Input { skill: 3, ..Default::default() });
    assert_eq!(run.home.industry.buildings[&id].carts.len(), 2, "{}", board(&run, 1).message);
    press(&mut run, 1, Input { skill: 3, ..Default::default() });
    assert_eq!(run.home.industry.buildings[&id].carts.len(), 4);
    // And keys 1, 2 and 3 are the hotbar's now.
    assert!(board(&run, 1).message.contains("CARTS"));
    let _ = Laid::Road((0, 0));
}

#[test]
fn the_party_sees_the_same_plans_and_the_hands_at_them() {
    let mut run = run(2);
    out(&mut run, 1);
    run.home.industry.apply(catalog(), Command::Plan { plan: "well".into(), tile: (170, 112), priority: 2, by: 1 }).unwrap();
    seconds(&mut run, 3);
    let v = crate::world::hillside::of(&run).expect("the works");
    assert_eq!(v.plans.len(), 1);
    assert_eq!(v.plans[0].priority, 2);
    assert!(!v.crews.is_empty(), "the hands are out: {:?}", run.home.industry.plans.crews);
    // A second seat's own board has not been touched by the first's keys.
    assert!(run.kingdom.industry.get(&2).is_none_or(|b| b.rig.is_idle()));
    assert!(board(&run, 1).valid());
}
