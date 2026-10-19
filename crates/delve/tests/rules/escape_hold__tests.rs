use super::*;

fn fixtures() -> Vec<(String, Run)> {
    fixtures_of(4)
}

/// Every game, started with `seats` knights at it.
fn fixtures_of(seats: u32) -> Vec<(String, Run)> {
    let mut out = Vec::new();
    for (name, make) in [
        ("Brasswake", Run::brasswake_for_test as fn() -> Run),
        ("Rafterwake", Run::rafterwake_for_test),
        ("Pennon", Run::pennon_for_test),
        ("Mossbell", Run::mossbell_for_test),
        ("Cinder", Run::cinder_for_test),
        ("Latch", Run::latch_for_test),
        ("Forest Road", Run::forest_road_for_test),
        // The Round Table adventures: each its own fixture, its door's way in.
        ("Lanternward", Run::lanternward_for_test),

        ("Moonfall", Run::moonfall_for_test),

        ("Bannerpeak", Run::bannerpeak_for_test),

        ("Griffinwing", Run::griffinwing_for_test),

        ("Lightgallop", Run::lightgallop_for_test),

    ] {
        let mut r = make();
        for id in 2..=seats { r.join_seat(id, &format!("Knight {id}")); }
        if r.forest_road_showing() { r.start_forest_road(); }
        r.floor_plates.clear();
        r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
        assert!(r.minigame_showing(), "{name} started");
        out.push((name.into(), r));
    }
    for game in pastimes::Game::ALL {
        let mut r = surface(seats);
        visit(&mut r, game.room());
        for h in r.players.values_mut() { (h.x, h.y) = (game.start().0 * TILE_UNITS, game.start().1 * TILE_UNITS); }
        r.step(&BTreeMap::new());
        assert_eq!(r.pastimes.round.as_ref().map(|s| s.game), Some(game));
        out.push((format!("{game:?}"), r));
    }
    let mut r = surface(seats);
    visit(&mut r, RoomKind::Hippodrome);
    r.board_chariot(1);
    r.chariot.as_mut().unwrap().phase = chariot::Phase::Racing;
    out.push(("Hippodrome".into(), r));
    let mut r = surface(seats);
    visit(&mut r, RoomKind::Lists);
    r.mount_up(1);
    r.joust.as_mut().unwrap().stage = joust::Stage::Charge;
    out.push(("Lists".into(), r));
    for kind in nooks::KINDS {
        let mut r = Run::new(41, 1, None);
        for id in 2..=seats { r.join_seat(id, &format!("Knight {id}")); }
        let at = r.dungeon.rooms.len();
        r.dungeon.rooms.push(nooks::room(kind, (99, 99), 2));
        r.enter(at, None);
        r.nook = Some(nooks::Nook { room: at, ..Default::default() });
        out.push((format!("{kind:?}"), r));
    }
    for name in ["Wax Maze", "Belfry Shoot", "Powder Room", "Book Hop", "Keg Gantry", "Shaft", "Ore Cart", "Griffin", "Touge"] {
        // The first seed (from 41 for a hall) whose floor has the room it needs.
        let mut r = if name == "Keg Gantry" {
            (0..400).map(|seed| Run::new(seed, 1, None)).find(|r| r.dungeon.rooms.iter().any(|s| s.kind == RoomKind::Ledge && s.cols == 48)).unwrap()
        } else {
            (41..141).map(|seed| {
                let mut r = Run::new(seed, 1, None);
                for id in 2..=seats { r.join_seat(id, &format!("Knight {id}")); }
                r.descend_for_test();
                r
            }).find(|r| r.dungeon.rooms.iter().any(|s| s.kind == RoomKind::Hall && s.cols >= 20)).unwrap()
        };
        if name == "Keg Gantry" { for id in 2..=seats { r.join_seat(id, &format!("Knight {id}")); } }
        r.clear_prerequisites_for_test();
        let at = r.dungeon.rooms.iter().position(|s| if name == "Keg Gantry" { s.kind == RoomKind::Ledge && s.cols == 48 } else { s.kind == RoomKind::Hall && s.cols >= 20 }).unwrap();
        r.folly_room = Some(at);
        r.race3d_room = None;
        r.gantry_room = None;
        r.enter(at, None);
        r.folly = None; r.bats = None; r.powder = None; r.hop = None;
        match name {
            "Wax Maze" => r.start_wax_maze(at, None),
            "Belfry Shoot" => r.start_bat_shoot(at),
            "Powder Room" => r.start_powder(at),
            "Book Hop" => r.start_book_hop(at),
            "Keg Gantry" => r.start_gantry(at),
            _ => {
                r.start_race3d(at);
                r.race3d.as_mut().unwrap().course = match name {
                    "Shaft" => race3d::Course::Shaft,
                    "Ore Cart" => race3d::Course::Cart,
                    "Touge" => race3d::Course::Touge,
                    _ => race3d::Course::Griffin,
                };
                r.race3d.as_mut().unwrap().t = race3d::TITLE;
            }
        }
        if let Some(s) = &mut r.folly { s.t = folly::TITLE; }
        if let Some(s) = &mut r.bats { s.t = folly::TITLE; }
        if let Some(s) = &mut r.gantry { s.t = folly::TITLE; }
        if let Some(s) = &mut r.powder { s.t = folly::TITLE; }
        if let Some(s) = &mut r.hop { s.t = folly::TITLE; }
        r.floor_plates.clear();
        out.push((name.into(), r));
    }
    out
}

fn bots(r: &Run) -> BTreeMap<u32, Input> {
    r.players.keys().map(|&id| {
        let i = if let Some(s) = &r.brasswake { s.lanes.get(&id).map(|d| d.bot_input(brasswake::LENGTH - s.left)) }
        else if let Some(s) = &r.rafterwake { s.lanes.get(&id).map(|d| d.bot_input(rafterwake::LENGTH - s.left)) }
        else if let Some(s) = &r.pennon { s.lanes.get(&id).map(|d| d.bot_input(pennon::LENGTH - s.left)) }
        else if let Some(s) = &r.mossbell { s.tables.get(&id).map(|d| d.bot_input()) }
        else if let Some(s) = &r.cinder { s.boards.get(&id).map(|d| d.bot_input()) }
        else if let Some(s) = &r.latch { s.duels.get(&id).map(|d| d.bot_input(1)) }
        else if let Some(a) = r.roundtable_under_way() { Some((a.bot)(r, id)) }
        else { Some(Input { move_x: if r.tick % 90 < 45 { 1 } else { -1 }, fire: r.tick % 2 == 0, dash: r.tick % 20 == 0, ..Default::default() }) };
        (id, i.unwrap_or_default())
    }).collect()
}

fn hold(r: &mut Run, seat: u32) {
    let mut keys = bots(r);
    keys.insert(seat, Input { leave_held: true, ..Default::default() });
    r.step(&keys);
}

#[test]
fn escape_hold_every_game_four_seats_tap_reset_return_and_last_cleanup() {
    every_game_tap_reset_return_and_last_cleanup(4);
}

#[test]
fn escape_hold_every_game_all_six_seats_tap_reset_return_and_last_cleanup() {
    every_game_tap_reset_return_and_last_cleanup(MAX_PLAYERS);
}

fn every_game_tap_reset_return_and_last_cleanup(seats: u32) {
    for (name, fixture) in fixtures_of(seats) {
        for seat in 1..=seats {
            let mut r = fixture.clone();
            let inside = r.at;
            assert!(r.can_hold_leave(seat), "{name}");
            let back = r.activity_return().unwrap();
            hold(&mut r, seat);
            assert_eq!(r.leave_progress(seat), 1, "{name}");
            r.step(&bots(&r));
            assert_eq!(r.leave_progress(seat), 0, "release resets {name}");
            assert!(r.exits.outside.is_empty(), "tap does nothing {name}");
            for _ in 1..LEAVE_TICKS { hold(&mut r, seat); }
            assert_eq!(r.leave_progress(seat), LEAVE_TICKS - 1, "{name}");
            assert!(r.exits.outside.is_empty());
            let saved: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
            assert_eq!(saved.leave_progress(seat), r.leave_progress(seat));
            hold(&mut r, seat);
            assert!(r.exits.outside.contains_key(&seat), "{name} seat {seat}");
            assert_eq!(r.at, inside, "friends keep their room {name}");
            let view = r.view_for_seat(seat).unwrap();
            assert_eq!(view.at, back.at, "return room {name}");
            assert!(!view.minigame_showing(), "leaver has no stage {name}");
            if let Some(s) = &r.forest_road { assert!(!s.knights.contains_key(&seat), "Forest rider departed"); }
            if let Some(seated) = r.roundtable_seated() { assert!(!seated.contains(&seat), "Round Table seat departed {name}"); }
            if let Some(s) = &r.race3d { assert!(!s.riders.contains_key(&seat), "Race rider departed"); }
            let gold = r.players[&seat].carried.get(Spoil::Gold);
            let tick = r.tick;
            for _ in 0..30 { r.step(&bots(&r)); }
            assert!(r.tick > tick, "three bots continue {name}");
            assert_eq!(r.players[&seat].carried.get(Spoil::Gold), gold, "no payout outside {name}");
            for other in 1..=seats {
                if other != seat { assert!(r.leave_activity(other), "last exits {name}"); }
            }
            assert_eq!(r.at, back.at, "last return {name}");
            assert!(r.exits.outside.is_empty());
            assert!(!r.minigame_showing(), "last cleanup {name}");
        }
    }
}

fn trace(r: &Run) -> Vec<u8> {
    // Immutable floor tiles dominate a whole Run. Record every changing game
    // and seat field instead, including rewards and the host hold counters.
    serde_json::to_vec(&(
        (&r.players, r.tick, r.score, &r.marks, &r.exits, &r.home.countryside.games),
        (&r.brasswake, &r.rafterwake, &r.pennon, &r.mossbell, &r.cinder, &r.latch, &r.forest_road),
        (&r.pastimes, &r.joust, &r.chariot, &r.race3d, &r.folly, &r.bats, &r.gantry, &r.powder, &r.hop, &r.nook),
        roundtable::sessions(r),
    )).unwrap()
}

#[test]
fn escape_hold_four_bot_no_leave_traces_are_byte_identical() {
    no_leave_traces_are_byte_identical(4);
}

#[test]
fn escape_hold_six_bot_no_leave_traces_are_byte_identical() {
    no_leave_traces_are_byte_identical(MAX_PLAYERS);
}

fn no_leave_traces_are_byte_identical(seats: u32) {
    for (name, fixture) in fixtures_of(seats) {
        let (mut a, mut b) = (fixture.clone(), fixture);
        for tick in 0..180 {
            a.step(&bots(&a));
            b.step(&bots(&b));
            assert_eq!(trace(&a), trace(&b), "{name} tick {tick}");
            assert!(a.exits.holding.is_empty());
        }
    }
}

#[test]
fn escape_hold_invalid_commands_and_ordinary_rooms_do_not_count() {
    let mut r = fixtures().remove(0).1;
    for _ in 0..LEAVE_TICKS {
        r.step(&BTreeMap::from([(1, Input { leave_held: true, move_x: 2, ..Default::default() })]));
    }
    assert_eq!(r.leave_progress(1), 0);
    let mut ordinary = Run::new(1, 1, None);
    for _ in 0..LEAVE_TICKS { hold(&mut ordinary, 1); }
    assert_eq!(ordinary.leave_progress(1), 0);
}

#[test]
fn escape_hold_discards_other_controls_on_the_departure_step() {
    let mut r = fixtures().remove(0).1;
    for _ in 1..LEAVE_TICKS { hold(&mut r, 1); }
    r.step(&BTreeMap::from([(1, Input {
        leave_held: true, interact: true, fire: true, dash: true,
        ..Default::default()
    })]));
    assert!(r.exits.outside.contains_key(&1), "held interact cannot bounce back through the entrance");
}

#[test]
fn escape_hold_forfeits_antes_and_cannot_win_from_outside() {
    for (name, mut r) in fixtures() {
        let pot = r.folly.as_ref().map(|s| s.ante.values().sum::<u32>())
            .or_else(|| r.bats.as_ref().map(|s| s.ante.values().sum()))
            .or_else(|| r.powder.as_ref().map(|s| s.ante.values().sum()))
            .or_else(|| r.hop.as_ref().map(|s| s.ante.values().sum()))
            .or_else(|| r.gantry.as_ref().map(|s| s.ante.values().sum()))
            .or_else(|| r.race3d.as_ref().map(|s| s.ante.values().sum()));
        if pot.is_none() { continue; }
        for h in r.players.values_mut() { h.carried.add(Spoil::Gold, 100); }
        let (ante, _) = folly::take_antes(&mut r.players);
        assert!(ante[&1] > 0, "a real stake {name}");
        if let Some(s) = &mut r.folly { s.ante = ante.clone(); }
        if let Some(s) = &mut r.bats { s.ante = ante.clone(); }
        if let Some(s) = &mut r.gantry { s.ante = ante.clone(); }
        if let Some(s) = &mut r.powder { s.ante = ante.clone(); }
        if let Some(s) = &mut r.hop { s.ante = ante.clone(); }
        if let Some(s) = &mut r.race3d { s.ante = ante.clone(); }
        let pot = Some(ante.values().sum::<u32>());
        for _ in 0..LEAVE_TICKS { hold(&mut r, 1); }
        let gold = r.players[&1].carried.get(Spoil::Gold);
        if let Some(s) = &mut r.folly { assert!(!s.eaten.contains_key(&1)); assert_eq!(Some(s.ante.values().sum()), pot); s.t = folly::TITLE + folly::MAZE_TIME - 1; }
        if let Some(s) = &mut r.bats { assert!(!s.scores.contains_key(&1)); assert_eq!(Some(s.ante.values().sum()), pot); s.t = folly::TITLE + folly::BAT_TIME - 1; }
        if let Some(s) = &mut r.gantry { assert!(!s.jumped.contains_key(&1)); assert_eq!(Some(s.ante.values().sum()), pot); s.t = folly::TITLE + gantry::CLIMB_TIME - 1; }
        if let Some(s) = &mut r.powder { assert!(!s.broken.contains_key(&1)); assert_eq!(Some(s.ante.values().sum()), pot); s.t = folly::TITLE + powder::POWDER_TIME - 1; }
        if let Some(s) = &mut r.hop { assert!(!s.crossings.contains_key(&1)); assert_eq!(Some(s.ante.values().sum()), pot); s.t = folly::TITLE + hop::HOP_TIME - 1; }
        if let Some(s) = &mut r.race3d { assert!(!s.riders.contains_key(&1)); assert_eq!(Some(s.ante.values().sum()), pot); s.t = race3d::TITLE + race3d::RACE_TIME - 1; }
        r.step(&bots(&r));
        assert_eq!(r.players[&1].carried.get(Spoil::Gold), gold, "no payout {name}");
    }
}

#[test]
fn escape_hold_uses_transport_ticks_even_during_an_impact_hold() {
    let mut r = fixtures().remove(0).1;
    r.hitstop.left = 12;
    for _ in 0..LEAVE_TICKS { hold(&mut r, 1); }
    assert!(r.exits.outside.contains_key(&1));
    assert!(!r.hitstop.buffered.contains_key(&1));
}

#[test]
fn escape_hold_pastime_leader_forfeits_even_if_every_friend_times_out() {
    for (name, mut r) in fixtures() {
        let Some(round) = &mut r.pastimes.round else { continue; };
        round.progress = round.game.need() - 1;
        r.exits.holding.insert(1, LEAVE_TICKS - 1);
        hold(&mut r, 1);
        let heat = r.pastimes.heat.as_ref().unwrap_or_else(|| panic!("heat {name}"));
        assert!(!heat.done.contains_key(&1), "forfeited leader {name}");
        for round in r.pastimes.heats.values_mut() { round.left = 1; }
        r.step(&BTreeMap::new());
        assert!(!r.players[&1].laurel, "cannot win outside {name}");
    }
}

#[test]
fn escape_hold_last_departure_preserves_puzzles_in_other_rooms() {
    let mut r = fixtures().into_iter().find(|(name, _)| name == "Wax Maze").unwrap().1;
    let nook = r.nook.clone().expect("a different floor room's puzzle");
    assert_ne!(nook.room, r.at);
    for seat in 1..=4 { for _ in 0..LEAVE_TICKS { hold(&mut r, seat); } }
    assert_eq!(r.nook, Some(nook));
    assert!(!r.minigame_showing());
}
