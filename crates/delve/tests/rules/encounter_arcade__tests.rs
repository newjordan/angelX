//! The arcade games dealt to a hall: each plays on the stage of its own
//! kingdom room, raised in the hall's place and put back after.
use super::*;
use crate::render::overworld::arena::frame_for;

fn open(seats: u32, kind: Kind) -> (Run, usize) {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.enemies.clear();
    run.pick_folly();
    let index = run.folly_room.expect("a hall");
    run.encounter_rooms = vec![(index, kind)];
    for h in run.players.values_mut() {
        h.carried.add(Spoil::Gold, 100);
    }
    run.enter_for_test(index);
    (run, index)
}

fn idle() -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

/// How many pixels of the frame are lit.
fn lit(run: &Run, seat: u32) -> usize {
    frame_for(run, 384, 224, Some(seat)).rgb_bytes().chunks(3).filter(|p| p.iter().any(|&c| c > 24)).count()
}

#[test]
fn every_arcade_game_raises_its_stage_draws_it_and_gives_the_hall_back() {
    for game in ARCADE {
        let kind = Kind::Arcade(game);
        let (mut run, index) = open(2, kind);
        let mut hall = run.dungeon.rooms[index].clone();
        run.step(&idle());
        let live = run.encounter.as_ref().unwrap_or_else(|| panic!("{game:?}: opens"));
        assert!(live.hall.is_some(), "{game:?}: its stage stands in the hall's place");
        assert_eq!(run.room().kind, game.room(), "{game:?}");
        assert!(run.pastimes.heat.is_some() && run.barred(), "{game:?}");
        assert!(run.enemies.is_empty(), "{game:?}: no company on a stage");
        assert!((0..run.room().rows as i32).all(|y| (0..run.room().cols as i32).all(|x| !matches!(run.room().tile(x, y), Tile::Stairs))),
            "{game:?}: a stage has no stair to take the party down");
        assert!(lit(&run, 1) > 3000, "{game:?}: the stage is drawn ({} pixels)", lit(&run, 1));
        assert!(lit(&run, 2) > 3000, "{game:?}: and on the second seat");
        assert!(run.valid_snapshot(), "{game:?}");
        // Played to its end with no one winning: the stake is lost, the hall comes back.
        let mut n = 0;
        while run.encounter.is_some() && n < Kind::Arcade(game).cap() + encounter::PAYOUT_TICKS + 600 {
            run.step(&idle());
            n += 1;
            if n % 300 == 0 {
                assert!(run.valid_snapshot(), "{game:?} at {n}");
            }
        }
        assert!(run.encounter.is_none(), "{game:?}: over after {n} ticks");
        // How long an idle table holds the room: the round, the payout card and no more.
        println!("{game:?}: an idle table's round and card last {n} ticks ({}s)", n / HZ);
        assert!(n >= encounter::PAYOUT_TICKS + 30, "{game:?}: the round ended before it began ({n} ticks)");
        assert_eq!(run.room().kind, RoomKind::Hall, "{game:?}: the hall is back");
        assert!(run.room().cleared && !run.barred(), "{game:?}");
        hall.cleared = true;
        hall.visited = true;
        assert_eq!(serde_json::to_string(run.room()).unwrap(), serde_json::to_string(&hall).unwrap(), "{game:?}: the hall as it was");
        assert!(run.pastimes.heat.is_none() && run.pastimes.round.is_none());
        assert!(run.valid_snapshot(), "{game:?}");
    }
}

#[test]
fn a_won_round_pays_in_order_and_a_lost_one_costs_the_ante() {
    let (mut run, _) = open(2, Kind::Arcade(Game::Cheese));
    run.step(&idle());
    let banked = run.bank.len();
    // Knight two wins the round, knight one's runs out.
    let heat = run.pastimes.heat.take().unwrap();
    run.pastimes.round = None;
    run.pastimes.heats.clear();
    let mut done = heat.done.clone();
    done.insert(1, pastimes::Finish { progress: 2, points: None });
    done.insert(2, pastimes::Finish { progress: Game::Cheese.need(), points: Some(700) });
    run.heat_to_encounter(&pastimes::Heat { done, ..heat });
    run.step(&idle());
    let end = run.encounter.as_ref().and_then(|e| e.end.clone()).expect("settled");
    assert!(end.won && end.order == [2]);
    assert_eq!(run.bank.len(), banked + 1);
    assert_eq!(end.paid[&2], crossroads::gold(0) + 20, "the purse and both antes");
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    // Nobody wins: the stake is lost.
    let (mut run, _) = open(1, Kind::Arcade(Game::Cheese));
    run.step(&idle());
    let gold = run.players[&1].carried.get(Spoil::Gold);
    run.pastimes.round = None;
    run.pastimes.heats.clear();
    let heat = run.pastimes.heat.take().unwrap();
    run.heat_to_encounter(&heat);
    run.step(&idle());
    let end = run.encounter.as_ref().and_then(|e| e.end.clone()).expect("settled");
    assert!(!end.won && end.paid.is_empty());
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), gold, "nothing more is taken");
}

#[test]
fn leaving_an_arcade_stage_by_the_door_puts_the_hall_back() {
    let (mut run, index) = open(1, Kind::Arcade(Game::Barrel));
    run.step(&idle());
    let at = run.at;
    assert_eq!(at, index);
    assert!(run.can_hold_leave(1) && run.leave_activity(1));
    assert_ne!(run.at, index, "back through the door");
    assert!(run.encounter.is_none() && run.pastimes.heat.is_none());
    assert_eq!(run.dungeon.rooms[index].kind, RoomKind::Hall, "the hall is as it was");
    assert!(!run.dungeon.rooms[index].cleared);
    assert!(run.valid_snapshot());
}

#[test]
fn a_save_made_on_an_arcade_stage_loads_and_plays_on() {
    let (mut run, _) = open(2, Kind::Arcade(Game::Surf));
    for _ in 0..40 {
        run.step(&idle());
    }
    let saved = serde_json::to_string(&run).unwrap();
    let mut back: Run = serde_json::from_str(&saved).expect("loads");
    assert!(back.valid_snapshot());
    assert!(back.encounter.as_ref().is_some_and(|e| e.hall.is_some()));
    for _ in 0..40 {
        back.step(&idle());
    }
    assert!(back.encounter.is_some() && back.valid_snapshot());
}
