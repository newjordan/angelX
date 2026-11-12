//! The waystations: a keeper's greeting and where-to, the coach round the
//! ring carrying the party together, the rally and the arrow it sets.
use super::*;
use crate::world::waystations as halts;

/// A halt's place in the ring, by its name.
fn stop(name: &str) -> usize {
    halts::STOPS.iter().position(|s| s.name == name).unwrap_or_else(|| panic!("no halt called {name}"))
}

fn press(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

fn answer(id: u32, key: u8) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { cast: key, ..Default::default() })])
}

/// A fresh press of E: let go, then down.
fn tap(run: &mut Run, id: u32) {
    run.step(&BTreeMap::new());
    run.step(&press(id));
}

/// Where a knight stands to speak with halt `i`'s keeper (units).
fn beside(i: usize) -> (f32, f32) {
    let p = halts::placed().iter().find(|p| p.stop == i).unwrap();
    ((p.keeper.0 as f32 + 0.5) * TILE_UNITS, (p.keeper.1 as f32 + 1.4) * TILE_UNITS)
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (x, y);
    hero.stone = false;
}

fn party(seats: u32) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.walk_kingdom(None);
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    run
}

/// Knight `id` beside halt `i`'s keeper with the where-to open.
fn asking(run: &mut Run, id: u32, i: usize) {
    put(run, id, beside(i));
    tap(run, id);
    assert_eq!(run.kingdom.stops.talks[&id].page, 0, "the greeting first");
    tap(run, id);
    assert_eq!(run.kingdom.stops.talks[&id].page, 1, "then where to");
}

#[test]
fn e_beside_a_keeper_greets_then_asks_where_to_then_ends() {
    let mut run = party(1);
    let gatehill = stop("Gatehill Halt");
    put(&mut run, 1, beside(gatehill));
    tap(&mut run, 1);
    let view = run.stop_talk_view(1).expect("a greeting");
    assert_eq!(view.halt, "Gatehill Halt");
    assert_eq!(view.words, halts::STOPS[gatehill].hello);
    tap(&mut run, 1);
    let view = run.stop_talk_view(1).unwrap();
    assert_eq!(view.answers[0], "1: COACH TO MINESIDE HALT");
    assert_eq!(view.answers[1], "2: COACH TO HARBOUR HALT");
    assert_eq!(view.answers[2], "3: NO ONE ELSE TO CALL", "alone on the road, no one to call");
    tap(&mut run, 1);
    assert!(run.stop_talk_view(1).is_none(), "E on the question is a farewell");
    assert!(run.valid_snapshot());
}

#[test]
fn holding_e_is_one_press_and_walking_off_ends_the_talk() {
    let mut run = party(1);
    put(&mut run, 1, beside(0));
    run.step(&BTreeMap::new());
    for _ in 0..10 {
        run.step(&press(1));
    }
    assert_eq!(run.kingdom.stops.talks[&1].page, 0, "a held E opens once and goes no further");
    put(&mut run, 1, (beside(0).0 + 30.0, beside(0).1));
    run.step(&BTreeMap::new());
    assert!(run.stop_talk_view(1).is_none(), "a knight who walks off has ended it");
}

#[test]
fn the_coach_takes_every_knight_by_the_post_to_the_next_halt_and_sets_them_down_together() {
    let mut run = party(3);
    let from = stop("Mineside Halt");
    put(&mut run, 2, (beside(from).0 + 4.0, beside(from).1));
    put(&mut run, 3, (beside(from).0 - 4.0, beside(from).1));
    asking(&mut run, 1, from);
    run.step(&answer(1, 1));
    let next = halts::placed().iter().find(|p| p.stop == halts::onward(from)).unwrap();
    let (ax, ay) = next.apron();
    for id in 1..=3 {
        let h = &run.players[&id];
        assert!((h.x / TILE_UNITS - ax).abs() < 3.0 && (h.y / TILE_UNITS - ay).abs() < 0.5, "knight {id} stands at ({}, {})", h.x, h.y);
        let tile = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
        assert_eq!(run.room().tile(tile.0, tile.1), Tile::Floor);
    }
    assert!(run.kingdom.stops.rally.is_none(), "no one was left behind");
    assert_eq!(run.found.as_ref().map(|f| f.2.as_str()), Some("COACH TO ASHROAD HALT"));
    assert!(run.kingdom.stops.burning(from, run.tick) && run.kingdom.stops.burning(halts::onward(from), run.tick));
    assert!(run.stop_talk_view(1).is_none());
    assert!(run.valid_snapshot());
}

#[test]
fn the_coach_back_runs_the_ring_the_other_way_and_wraps() {
    let mut run = party(1);
    asking(&mut run, 1, 0);
    run.step(&answer(1, 2));
    let last = halts::placed().iter().find(|p| p.stop == halts::STOPS.len() - 1).unwrap();
    assert!((run.players[&1].x / TILE_UNITS - last.apron().0).abs() < 1.0);
}

#[test]
fn a_knight_too_far_from_the_post_is_left_behind_and_a_rally_is_called_for_them() {
    let mut run = party(2);
    let from = stop("Listsend Halt");
    let far = (beside(from).0 + 60.0 * TILE_UNITS, beside(from).1);
    put(&mut run, 2, far);
    asking(&mut run, 1, from);
    run.step(&answer(1, 1));
    let to = halts::onward(from);
    assert_eq!(run.kingdom.stops.rally.map(|r| usize::from(r.stop)), Some(to));
    assert!((run.players[&2].x - far.0).abs() < 1.0, "the far knight did not ride");
    assert!(run.found.as_ref().unwrap().2.contains("1 LEFT BEHIND"));
    // Their arrow points to the halt the coach went to.
    let goal = run.kingdom_goal(2).unwrap();
    assert_eq!(goal.name, format!("RALLY: {}", halts::STOPS[to].name.to_uppercase()));
    assert_eq!(goal.stand, halts::placed().iter().find(|p| p.stop == to).unwrap().keeper);
    // The ones who rode are there already: their arrow is the usual one.
    assert!(!run.kingdom_goal(1).unwrap().name.starts_with("RALLY"));
    // They come near it, and the rally stands down.
    put(&mut run, 2, beside(to));
    run.step(&BTreeMap::new());
    assert!(run.kingdom.stops.rally.is_none());
    assert!(run.found.as_ref().unwrap().2.starts_with("THE PARTY IS GATHERED AT"));
}

#[test]
fn a_rally_calls_the_others_and_lapses_after_three_minutes() {
    let mut run = party(2);
    let at = stop("Coastgate Halt");
    put(&mut run, 2, (beside(at).0 + 40.0 * TILE_UNITS, beside(at).1));
    asking(&mut run, 1, at);
    run.step(&answer(1, 3));
    assert_eq!(run.kingdom.stops.rally.map(|r| usize::from(r.stop)), Some(at));
    assert_eq!(run.found.as_ref().map(|f| f.2.as_str()), Some("RALLY AT COASTGATE HALT"));
    assert!(run.kingdom_goal(2).unwrap().name.starts_with("RALLY: COASTGATE"));
    assert!(run.kingdom.stops.burning(at, run.tick) && !run.kingdom.stops.burning(halts::back(at), run.tick));
    run.tick += RALLY_TICKS;
    run.step(&BTreeMap::new());
    assert!(run.kingdom.stops.rally.is_none(), "a rally that nobody answers lapses");
    assert!(!run.kingdom_goal(2).unwrap().name.starts_with("RALLY"));
}

#[test]
fn only_a_talk_on_the_where_to_page_answers_and_the_keys_cast_nothing() {
    let mut run = party(1);
    put(&mut run, 1, beside(stop("Gatehill Halt")));
    tap(&mut run, 1);
    let before = run.players[&1].x;
    run.step(&answer(1, 1));
    assert_eq!(run.players[&1].x, before, "the greeting takes no answer");
    assert!(run.kingdom.stops.rode.is_none());
}

#[test]
fn a_keeper_keeps_the_press_from_a_banner_or_a_door() {
    let mut run = party(1);
    let geo = crate::render::overworld::kingdom::geography(&[]);
    for p in halts::placed() {
        let (kx, ky) = ((p.keeper.0 as f32 + 0.5) * TILE_UNITS, (p.keeper.1 as f32 + 1.0) * TILE_UNITS);
        put(&mut run, 1, (kx, ky));
        assert!(crate::rules::kingdom::door_at(&geo, kx, ky).is_none(), "{} stands at a door", halts::STOPS[p.stop].name);
    }
}

#[test]
fn the_halts_state_saves_loads_and_an_old_save_without_it_loads() {
    let mut run = party(2);
    asking(&mut run, 1, stop("Ashroad Halt"));
    run.step(&answer(1, 3));
    let json = serde_json::to_value(&run.kingdom).unwrap();
    assert!(json.get("stops").is_some());
    let back: crate::rules::kingdom::State = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back.stops, run.kingdom.stops);
    // A kingdom from before the halts: no `stops` at all.
    let mut old = json;
    old.as_object_mut().unwrap().remove("stops");
    let back: crate::rules::kingdom::State = serde_json::from_value(old).unwrap();
    assert!(back.stops.is_idle());
    // An idle kingdom writes nothing of it.
    let idle = serde_json::to_value(crate::rules::kingdom::State::default()).unwrap();
    assert!(idle.get("stops").is_none());
}

#[test]
fn a_damaged_halts_state_is_mended_away() {
    let mut run = party(1);
    run.kingdom.stops.rally = Some(Rally { stop: 200, since: 0 });
    assert!(!run.kingdom.valid());
    run.mend();
    assert!(run.kingdom.stops.is_idle());
}
