//! The Colosseum: its door opens on an entrance hall with two arched gates,
//! each gate on its own arena game, and back out again; the Old Watch and
//! the Marsh are places again; a heat in an arena still crowns a winner; an
//! old save still loads.
use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::pastimes::{FIRE, FUSE, Game};

/// A party of `seats` in the kingdom, by the Keep.
fn kingdom(seats: u32) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    for id in 3..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.players.retain(|&id, _| id <= seats.max(1));
    r.walk_kingdom(Some(geography::keep_stand()));
    r.floor_plates.clear();
    r
}

/// The Colosseum's door on the kingdom's map.
fn colosseum_site(r: &Run) -> usize {
    let geo = geography::geography(&r.kingdom.wishes);
    geo.sites
        .iter()
        .position(|s| s.room == RoomKind::ColosseumHall)
        .expect("the Colosseum has a door on the map")
}

/// In through the Colosseum's door, as E at its stand does.
fn in_the_hall(seats: u32) -> Run {
    let mut r = kingdom(seats);
    let site = colosseum_site(&r);
    let stand = geography::geography(&r.kingdom.wishes).sites[site].stand;
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((stand.0 as f32 + 0.5) * TILE_UNITS, (stand.1 as f32 + 0.7) * TILE_UNITS);
    let e = Input { interact: true, ..Default::default() };
    // A fresh press: the E that walked into the kingdom is let go first.
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(1, e)]));
    r.step(&BTreeMap::new());
    r
}

fn place(r: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = r.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

/// The party stands on plate `at` until it has gone through it: a gate
/// takes the whole fellowship (`fellowship`).
fn take(r: &mut Run, at: (i32, i32, i32, i32)) {
    let from = r.room().kind;
    // Off any plate first, so the next one is a fresh step.
    r.step(&BTreeMap::new());
    let ids: Vec<u32> = r.players.keys().copied().collect();
    for id in ids {
        place(r, id, (at.0 as f32 + at.2 as f32 / 2.0, at.1 as f32 + 0.5));
    }
    for _ in 0..4 * HZ {
        r.step(&BTreeMap::new());
        if r.room().kind != from {
            break;
        }
    }
    for _ in 0..3 {
        r.step(&BTreeMap::new());
    }
}

fn key(id: u32, (mx, my): (i8, i8), fire: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { move_x: mx, move_y: my, fire, ..Default::default() })])
}

fn tiles(r: &Run, id: u32) -> (f32, f32) {
    let h = &r.players[&id];
    (h.x / TILE_UNITS, h.y / TILE_UNITS)
}

#[test]
fn the_door_leads_to_the_hall_its_gates_to_the_arenas_and_back_out() {
    let mut r = in_the_hall(2);
    assert_eq!(r.room().kind, RoomKind::ColosseumHall, "the door opens on the entrance hall");
    assert!(r.kingdom.return_to.is_some(), "a visit from the kingdom");
    let stand = r.kingdom.return_to.as_ref().map(|b| (b.x, b.y)).unwrap();

    // The west gate: Powder Keg, and back.
    take(&mut r, colosseum::GATES[0]);
    assert_eq!(r.room().kind, RoomKind::KegYard);
    assert_eq!(colosseum::game_in(r.room().kind), Some(Game::Powder));
    take(&mut r, colosseum::YARD_GATE);
    assert_eq!(r.room().kind, RoomKind::ColosseumHall, "the yard's gate leads back to the hall");
    let (x, y) = tiles(&r, 1);
    let (gx, gy) = colosseum::GATE_FRONT[0];
    assert!((x - gx).abs() < 2.0 && (y - gy).abs() < 1.0, "before the west gate, at {x},{y}");

    // The east gate: Gander Joust, and back.
    take(&mut r, colosseum::GATES[1]);
    assert_eq!(r.room().kind, RoomKind::GanderHall);
    assert_eq!(colosseum::game_in(r.room().kind), Some(Game::Gander));
    take(&mut r, colosseum::HALL_GATE);
    assert_eq!(r.room().kind, RoomKind::ColosseumHall);
    let (x, _) = tiles(&r, 1);
    assert!((x - colosseum::GATE_FRONT[1].0).abs() < 2.0, "before the east gate, at {x}");

    // The leave key in an arena also comes back to the hall, and the visit
    // still returns to the kingdom from there.
    take(&mut r, colosseum::GATES[0]);
    assert_eq!(r.room().kind, RoomKind::KegYard);
    for id in [1, 2] {
        r.step(&BTreeMap::from([(id, Input { leave_activity: true, ..Default::default() })]));
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.room().kind, RoomKind::ColosseumHall, "out of the arena is the hall");
    assert!(r.kingdom.return_to.is_some(), "the hall still knows the way out");

    // The great door: back to the kingdom, outside the Colosseum. Each
    // knight goes out on their own feet, as at the College's gate.
    place(&mut r, 2, (12.0, 12.5));
    take(&mut r, colosseum::GREAT_DOOR);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    let h = &r.players[&1];
    assert!((h.x - stand.0).hypot(h.y - stand.1) < 4.0, "outside the Colosseum's door");
    assert!(r.valid_snapshot());
}

#[test]
fn an_arena_gate_stays_shut_while_its_heat_runs() {
    let mut r = in_the_hall(2);
    take(&mut r, colosseum::GATES[1]);
    assert_eq!(r.room().kind, RoomKind::GanderHall);
    place(&mut r, 1, Game::Gander.start());
    r.step(&BTreeMap::new());
    assert!(r.pastimes.heat.is_some(), "the heat begins on the mark");
    // A rider flung onto the gate does not carry the party out.
    for _ in 0..2 * HZ {
        place(&mut r, 2, (2.0, 10.0));
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.room().kind, RoomKind::GanderHall);
}

#[test]
fn the_old_watch_and_the_marsh_are_places_again() {
    for (kind, old_pad) in [(RoomKind::Ruins, (3.0, 5.0)), (RoomKind::Marsh, (9.0, 8.0))] {
        assert!(
            Game::ALL.iter().all(|g| g.room() != kind),
            "no game is played in {kind:?} now"
        );
        let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 11.0));
        place(&mut r, 1, old_pad);
        for _ in 0..10 {
            r.step(&BTreeMap::new());
        }
        assert!(r.pastimes.round.is_none() && r.pastimes.heat.is_none(), "the old pad in {kind:?} is gone");
    }
    // The two arena games are each played in their own Colosseum room.
    assert_eq!(Game::Powder.room(), RoomKind::KegYard);
    assert_eq!(Game::Gander.room(), RoomKind::GanderHall);
}

#[test]
fn two_knights_in_the_keg_yard_crown_a_winner() {
    let mut r = in_the_hall(2);
    take(&mut r, colosseum::GATES[0]);
    assert_eq!(r.room().kind, RoomKind::KegYard);
    place(&mut r, 1, Game::Powder.start());
    place(&mut r, 2, (Game::Powder.start().0 + 1.0, Game::Powder.start().1 + 1.0));
    r.tick_pastimes(&BTreeMap::new());
    let heat = r.pastimes.heat.as_ref().expect("a heat for both knights");
    assert_eq!(heat.seats.len(), 2, "every knight in the arena plays");
    // Knight 2 drops a keg where they stand and waits in its cross.
    r.tick_pastimes(&key(2, (0, 0), true));
    for _ in 0..u32::from(FUSE) + u32::from(FIRE) + 4 {
        r.tick_pastimes(&key(1, (0, 0), false));
        if r.pastimes.heat.is_none() {
            break;
        }
    }
    assert!(r.pastimes.heat.is_none(), "the last knight standing ends the heat");
    assert!(r.players[&1].laurel, "the knight left standing wears the laurel");
    assert!(!r.players[&2].laurel);
    assert_eq!(r.home.countryside.games.powder.plays, 1);
    assert!(r.home.countryside.games.powder.best > 0, "the winner's purse is the best to beat");
}

#[test]
fn an_old_save_loads() {
    // A checkpoint from before the Colosseum: no hall and no arenas on the
    // home floor, the party in the Old Watch with a Powder Keg heat on (as
    // it was played there), and the records the games kept then.
    let mut old = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    old.home.countryside.games.powder.best = 420;
    old.home.countryside.games.powder.plays = 3;
    old.home.countryside.games.gander.wins = 1;
    old.home.countryside.games.gander.plays = 2;
    let yard = world::room_of(&old.dungeon, RoomKind::KegYard).unwrap();
    old.arrive(yard, colosseum::YARD_ARRIVAL);
    place(&mut old, 1, Game::Powder.start());
    old.tick_pastimes(&BTreeMap::new());
    let heat = serde_json::to_value(&old.pastimes).unwrap();
    old.arrive(world::room_of(&old.dungeon, RoomKind::Ruins).unwrap(), (12.0, 11.0));
    old.exits = Default::default();
    old.dungeon.rooms.retain(|room| !colosseum::is_room(room.kind));
    let mut json = serde_json::to_value(&old).unwrap();
    json["pastimes"] = heat;
    let mut r: Run = serde_json::from_value(json).unwrap();
    assert!(r.valid_snapshot());
    assert!(r.pastimes.round.as_ref().is_some_and(|r| r.game == Game::Powder));
    assert_eq!(world::room_of(&r.dungeon, RoomKind::ColosseumHall), None);
    // The realm rises around it: the Colosseum's rooms are added.
    let (home, treasury) = (r.home.clone(), r.treasury.clone());
    r.rebuild_home(home, treasury);
    for kind in colosseum::ROOMS {
        assert!(world::room_of(&r.dungeon, kind).is_some(), "{kind:?} added");
    }
    // The old heat in the Old Watch is let go; the records are kept.
    r.step(&BTreeMap::new());
    assert!(r.pastimes.round.is_none() && r.pastimes.heat.is_none());
    assert_eq!(r.room().kind, RoomKind::Ruins);
    assert_eq!(r.home.countryside.games.best_for(Game::Powder), 420);
    assert_eq!(r.home.countryside.games.plays_for(Game::Gander), 2);
    assert!(r.valid_snapshot());
    // Every game keeps its place in the list (saved masks index by it).
    assert_eq!(Game::Powder.index(), 16);
    assert_eq!(Game::Gander.index(), 17);
    assert_eq!(Game::ALL.len(), 19);
}

/// The frames, to look at:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_colosseum_shots -- --ignored`
#[test]
#[ignore]
fn write_colosseum_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("colosseum");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |r: &Run, seat: u32, name: &str| {
        let view = r.view_for_seat(seat);
        let img = crate::render::overworld::arena::frame_for(view.as_ref().unwrap_or(r), 768, 448, Some(seat));
        image::RgbaImage::from_raw(768, 448, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    // Outside, at the Colosseum's door.
    let mut r = kingdom(2);
    let site = colosseum_site(&r);
    let stand = geography::geography(&r.kingdom.wishes).sites[site].stand;
    r.walk_kingdom(Some(stand));
    for _ in 0..20 {
        r.step(&BTreeMap::new());
    }
    save(&r, 1, "1-outside.png");
    // In the entrance hall.
    let mut r = in_the_hall(2);
    for _ in 0..20 {
        r.step(&BTreeMap::new());
    }
    save(&r, 1, "2-hall.png");
    // The Powder Keg yard, before the heat and mid-heat with two knights.
    take(&mut r, colosseum::GATES[0]);
    settle(&mut r);
    save(&r, 1, "3-yard-arrive.png");
    place(&mut r, 1, Game::Powder.start());
    place(&mut r, 2, (Game::Powder.start().0 + 1.0, Game::Powder.start().1 + 1.0));
    r.step(&BTreeMap::new());
    for _ in 0..12 {
        r.step(&key(1, (0, 1), false));
    }
    r.step(&key(1, (0, 0), true));
    for _ in 0..14 {
        r.step(&key(1, (-1, 0), false));
    }
    for _ in 0..10 {
        r.step(&key(2, (1, 0), false));
    }
    save(&r, 1, "4-yard-heat.png");
    // Out through the yard's gate once the heat is done: back in the hall.
    r.pastimes.leave();
    take(&mut r, colosseum::YARD_GATE);
    settle(&mut r);
    save(&r, 1, "5-back-in-hall.png");
    // The Gander Joust hall mid-heat: both knights aloft, leaning apart.
    take(&mut r, colosseum::GATES[1]);
    settle(&mut r);
    save(&r, 1, "6-gander-arrive.png");
    place(&mut r, 1, Game::Gander.start());
    r.step(&BTreeMap::new());
    let ride = |id: u32, lean: i8, flap: bool| {
        BTreeMap::from([(id, Input { aim_x: lean, fire: flap, ..Default::default() })])
    };
    for i in 0..36 {
        r.step(&ride(1, 1, i % 3 == 0));
        r.step(&ride(2, -1, i % 4 == 0));
    }
    // Seat one dives on seat two over the middle ledge: the clack to come.
    {
        let g = r.pastimes.round.as_mut().and_then(|r| r.gander.as_mut()).unwrap();
        for (k, (x, y, face)) in [(9 * 256, 4 * 256 + 40, 1), (11 * 256, 6 * 256, -1)].into_iter().enumerate() {
            let rider = &mut g.riders[k];
            (rider.x, rider.y, rider.dx, rider.dy, rider.face, rider.stun, rider.clash) = (x, y, 0, 40, face, 0, 0);
        }
    }
    r.step(&BTreeMap::new());
    save(&r, 1, "7-gander-heat.png");
    // The Old Watch and the Marsh, places again: no pads on their floors.
    for (kind, name) in [(RoomKind::Ruins, "8-old-watch.png"), (RoomKind::Marsh, "9-marsh.png")] {
        let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
        r.floor_plates.clear();
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 11.0));
        settle(&mut r);
        save(&r, 1, name);
    }
}

/// A few quiet ticks: the arrival's blink is over.
fn settle(r: &mut Run) {
    for _ in 0..40 {
        r.step(&BTreeMap::new());
    }
}

