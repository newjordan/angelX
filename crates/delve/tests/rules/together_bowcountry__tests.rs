//! Longbow country: its rooms stand on the home floor and in the kingdom,
//! the scarecrow's wardrobe pays once and opens the Quiver Coach, and the
//! Popinjay Shoot is played by every knight at once for the crown.
use super::*;
use crate::rules::{
    home::Home,
    pastimes::{Game, LONGBOW_ARROWS, LONGBOW_FLIGHT, LONGBOW_FULL_DRAW, Longbow},
    world,
};
use crate::render::overworld::kingdom as geography;

fn in_room(kind: RoomKind, seats: u32) -> Run {
    let mut run = Run::at_home(53, 1, None, Home::default(), Spoils::default());
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.arrive(world::room_of(&run.dungeon, kind).unwrap(), (12.0, 11.0));
    run
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn press(run: &mut Run, id: u32) {
    run.step(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
}

fn go(run: &mut Run, kind: RoomKind) {
    let at = world::room_of(&run.dungeon, kind).unwrap();
    run.arrive(at, (12.0, 11.0));
    run.step(&BTreeMap::new());
}

#[test]
fn longbow_country_stands_on_the_home_floor_with_doors_and_a_kingdom_door_each() {
    let run = in_room(RoomKind::LongbowGreen, 1);
    for (kind, cell) in [
        (RoomKind::LongbowGreen, (-2, -9)),
        (RoomKind::Fletchery, (-1, -8)),
        (RoomKind::Scarecrow, (-3, -9)),
    ] {
        let i = world::room_of(&run.dungeon, kind).expect("raised with the world");
        let room = &run.dungeon.rooms[i];
        assert_eq!(room.cell, cell);
        assert!(room.doors.iter().any(|&d| d), "{kind:?} has a door out");
        assert!(kind.in_world());
    }
    // The green opens south onto the flags and west onto the field.
    let green = &run.dungeon.rooms[world::room_of(&run.dungeon, RoomKind::LongbowGreen).unwrap()];
    assert!(green.doors[2] && green.doors[3]);
    // Each has its own door in the kingdom, reached by road from the Keep.
    let geo = geography::geography(&[]);
    for kind in PLACES {
        let site = geo.sites.iter().find(|s| s.room == kind).unwrap_or_else(|| panic!("{kind:?} has no door"));
        assert!(geography::is_road(&geo, site.stand), "{kind:?} stands on a road");
    }
}

#[test]
fn every_knight_walks_in_from_the_kingdom_and_back_out() {
    let mut run = Run::at_home(53, 1, None, Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    run.walk_kingdom(Some(geography::keep_stand()));
    run.floor_plates.clear();
    let geo = geography::geography(&run.kingdom.wishes);
    let site = geo.sites.iter().position(|s| s.room == RoomKind::LongbowGreen).unwrap();
    run.enter_kingdom_place(site);
    assert_eq!(run.room().kind, RoomKind::LongbowGreen);
    assert!(run.valid_snapshot());
    // Down the south road and out of the door: back in the kingdom.
    for _ in 0..HZ * 3 {
        run.step(&BTreeMap::from([(1, Input { move_y: 1, ..Default::default() }), (2, Input { move_y: 1, ..Default::default() })]));
        if run.room().kind == RoomKind::Kingdom {
            break;
        }
    }
    assert_eq!(run.room().kind, RoomKind::Kingdom);
}

#[test]
fn the_scarecrows_wardrobe_pays_once_and_opens_the_quiver_coach() {
    let mut run = in_room(RoomKind::Scarecrow, 2);
    // Before he is asked, the clothes cannot be taken.
    put(&mut run, 1, CLOTHES[2].1);
    press(&mut run, 1);
    assert_eq!(progress(&run.home.countryside, &run.marks), (0, 0));
    put(&mut run, 1, CLIENT);
    press(&mut run, 1);
    assert_eq!(progress(&run.home.countryside, &run.marks).0, 1);
    assert!(run.found.as_ref().unwrap().2.contains("FIND HIS CLOTHES"));
    // He will not dress half-clothed.
    put(&mut run, 2, CLIENT);
    press(&mut run, 2);
    assert!(run.found.as_ref().unwrap().2.contains("STILL UNDRESSED"));
    for (i, &(room, at, name)) in CLOTHES.iter().enumerate() {
        go(&mut run, room);
        put(&mut run, 2, at);
        press(&mut run, 2);
        assert_eq!(progress(&run.home.countryside, &run.marks).1 & (1 << i), 1 << i, "{name}");
        assert!(run.found.as_ref().unwrap().2.contains(name));
    }
    go(&mut run, RoomKind::Scarecrow);
    put(&mut run, 1, CLIENT);
    press(&mut run, 1);
    assert_eq!(progress(&run.home.countryside, &run.marks), (2, 7));
    assert!(run.valid_snapshot());
    let mut country = run.home.countryside.clone();
    let mut treasury = Spoils::default();
    let (_, news) = countryside::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), PURSE.0);
    assert_eq!(treasury.get(Spoil::Bone), PURSE.1);
    assert!(news.iter().any(|n| n.contains("scarecrow")));
    assert!(country.valid() && country.wardrobe_paid);
    // Paid once: settling again pays nothing.
    let before = treasury.clone();
    countryside::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury, before);
    // The Quiver Coach is now buildable.
    let plot = &countryside::PLOTS[PLOT];
    assert_eq!(plot.id, "quivercoach");
    assert!(plot.blocked(&country).is_none());
    assert!(plot.blocked(&countryside::Country::default()).is_some());
}

#[test]
fn the_quiver_coach_rides_both_ways_once_built() {
    let mut run = in_room(RoomKind::Fletchery, 1);
    let out = world::ENTRANCES.iter().find(|e| e.id == "quiver-out").unwrap();
    assert!(!world::open(out, &run.home));
    run.home.countryside.wardrobe = 2;
    run.home.countryside.clothes = 7;
    run.home.countryside.built |= 1 << PLOT;
    assert!(run.home.countryside.valid());
    assert!(world::open(out, &run.home));
    run.take_entrance(out);
    assert_eq!(run.room().kind, RoomKind::Village);
    let back = world::ENTRANCES.iter().find(|e| e.id == "quiver-back").unwrap();
    run.take_entrance(back);
    assert_eq!(run.room().kind, RoomKind::Fletchery);
    assert!(run.valid_snapshot());
}

#[test]
fn the_crier_sends_the_party_to_the_scarecrow_and_then_his_clothes() {
    let mut run = in_room(RoomKind::Scarecrow, 1);
    assert!(run.open_errands().contains(&(RoomKind::Scarecrow, "THE SCARECROW HAS QUIT")));
    run.marks.insert(QUEST_MARKS[0].into(), 1);
    run.marks.insert(QUEST_MARKS[1].into(), 1);
    let errands = run.open_errands();
    assert!(!errands.iter().any(|e| e.1 == WANTED[0]), "the hat is found");
    assert!(errands.contains(&(RoomKind::Fletchery, WANTED[1])));
    assert!(errands.contains(&(RoomKind::Scarecrow, WANTED[2])));
}

/// The popinjay's round for seat `id`, wherever it is kept.
fn round_of(run: &Run, id: u32) -> Option<&crate::rules::pastimes::Round> {
    let lead = run.pastimes.heat.as_ref().and_then(|h| h.seats.first().copied());
    if lead == Some(id) { run.pastimes.round.as_ref() } else { run.pastimes.heats.get(&id) }
}

fn fire(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { fire: true, ..Default::default() })])
}

/// Seat `id` draws in full, then waits for the moment a loosed arrow would
/// meet the popinjay, and looses: a hit, found by trying each moment on a
/// copy of the run.
fn hit(run: &mut Run, id: u32) {
    // The last arrow lands and the archer nocks the next.
    while round_of(run, id).and_then(|r| r.longbow.as_ref()).is_some_and(|b| b.flight.is_some() || b.wait > 0) {
        run.step(&BTreeMap::new());
    }
    let before = round_of(run, id).unwrap().progress;
    for _ in 0..LONGBOW_FULL_DRAW + 1 {
        run.step(&fire(id));
    }
    for _ in 0..HZ * 5 {
        let mut trial = run.clone();
        for _ in 0..LONGBOW_FLIGHT + 2 {
            trial.step(&BTreeMap::new());
        }
        if round_of(&trial, id).is_none_or(|r| r.progress > before) {
            run.step(&BTreeMap::new());
            return;
        }
        run.step(&fire(id));
    }
    panic!("no moment to hit the popinjay: {:?} {:?}", round_of(run, id).map(|r| (r.progress, r.mistakes, r.left)), round_of(run, id).and_then(|r| r.longbow.clone()));
}

#[test]
fn everyone_shoots_at_once_and_the_best_archer_takes_the_crown() {
    let mut run = in_room(RoomKind::LongbowGreen, 2);
    put(&mut run, 2, (14.0, 8.0));
    put(&mut run, 1, Game::Longbow.start());
    run.step(&BTreeMap::new());
    let heat = run.pastimes.heat.clone().expect("a heat begins");
    assert_eq!(heat.seats, vec![1, 2]);
    // Both archers toe the line.
    run.step(&BTreeMap::new());
    for id in [1, 2] {
        assert_eq!(run.players[&id].y, LINE * TILE_UNITS);
    }
    assert!(run.valid_snapshot());
    // The friend sees their own round.
    let view = run.view_for_seat(2).unwrap();
    assert_eq!(view.pastimes.round.as_ref(), run.pastimes.heats.get(&2));
    // A friend's angelX mirrors the round.
    let friend = run.wire_copy();
    assert_eq!(friend.pastimes, run.pastimes);
    for _ in 0..Game::Longbow.need() {
        hit(&mut run, 2);
    }
    for _ in 0..LONGBOW_FLIGHT + 1 {
        run.step(&BTreeMap::new());
    }
    assert!(run.pastimes.heat.is_none(), "first to three ends the heat");
    assert!(run.players[&2].laurel, "the winner is crowned");
    assert_eq!(run.players[&1].booby, Some(Game::Longbow), "an apple on the loser's helm");
    assert!(run.marks.contains_key(&countryside::game_mark(Game::Longbow.index())));
    assert!(run.feats.contains(&"popinjay"), "three for three is a feat");
    assert!(run.valid_snapshot());
}

#[test]
fn a_short_draw_falls_short_and_an_empty_quiver_loses() {
    let mut run = in_room(RoomKind::LongbowGreen, 1);
    put(&mut run, 1, Game::Longbow.start());
    run.step(&BTreeMap::new());
    assert!(run.pastimes.round.is_some());
    for n in 0..LONGBOW_ARROWS {
        // A tap: barely drawn.
        run.step(&fire(1));
        run.step(&BTreeMap::new());
        assert!(run.pastimes.round.as_ref().unwrap().longbow.as_ref().unwrap().flight.is_some());
        for _ in 0..LONGBOW_FLIGHT + HZ / 2 {
            run.step(&BTreeMap::new());
        }
        if n + 1 < LONGBOW_ARROWS {
            let r = run.pastimes.round.as_ref().unwrap();
            assert_eq!((r.progress, r.mistakes), (0, n + 1));
            assert!(run.valid_snapshot());
        }
    }
    assert!(run.pastimes.round.is_none(), "out of arrows");
    assert!(!run.marks.contains_key(&countryside::game_mark(Game::Longbow.index())));
}

#[test]
fn the_popinjay_runs_its_rope_and_the_wind_turns() {
    let ends: Vec<i32> = (0..PERIOD_TEST).map(|t| Longbow::target_at(t, 0)).collect();
    assert_eq!(*ends.iter().min().unwrap(), 7 * 256);
    assert_eq!(*ends.iter().max().unwrap(), 17 * 256);
    assert_ne!(Longbow::wind(0), Longbow::wind(1));
}
const PERIOD_TEST: u32 = 4 * HZ;

/// Longbow country in play: `ANGEL_ARENA_SHOTS=<dir> cargo test
/// write_bowcountry_shots -- --ignored`.
#[test]
#[ignore]
fn write_bowcountry_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("bowcountry");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str, focus: u32| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(focus));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    // The green at rest, two knights by the line.
    let mut run = in_room(RoomKind::LongbowGreen, 2);
    put(&mut run, 1, (5.0, 9.0));
    put(&mut run, 2, (14.0, 8.0));
    run.step(&BTreeMap::new());
    save(&run, "a-green", 1);
    // In play: seat one drawing, seat two's arrow in the air.
    put(&mut run, 1, Game::Longbow.start());
    run.step(&BTreeMap::new());
    put(&mut run, 1, (9.0, 10.0));
    put(&mut run, 2, (15.0, 10.0));
    for _ in 0..12 {
        run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() }), (2, Input { fire: true, ..Default::default() })]));
    }
    run.step(&fire(1));
    save(&run, "b-drawing", 1);
    for _ in 0..8 {
        run.step(&fire(1));
    }
    save(&run.view_for_seat(2).unwrap(), "c-arrow-flying", 2);
    hit(&mut run, 1);
    save(&run, "d-hit", 1);
    // The crowned and the apple-helmed.
    let mut won = in_room(RoomKind::LongbowGreen, 2);
    put(&mut won, 2, (14.0, 8.0));
    put(&mut won, 1, Game::Longbow.start());
    won.step(&BTreeMap::new());
    put(&mut won, 1, (10.0, 10.0));
    for _ in 0..3 {
        hit(&mut won, 1);
    }
    for _ in 0..HZ * 2 {
        won.step(&BTreeMap::new());
    }
    save(&won, "e-crowned", 1);
    // The fletchery and the field, with the errand under way.
    let mut fl = in_room(RoomKind::Fletchery, 2);
    put(&mut fl, 1, (6.0, 6.0));
    put(&mut fl, 2, (16.0, 6.0));
    fl.marks.insert(QUEST_MARKS[0].into(), 1);
    fl.step(&BTreeMap::new());
    save(&fl, "f-fletchery", 1);
    let mut field = in_room(RoomKind::Scarecrow, 2);
    put(&mut field, 1, (12.0, 5.2));
    put(&mut field, 2, (18.0, 8.0));
    field.step(&BTreeMap::new());
    save(&field, "g-field-on-strike", 1);
    for i in 0..5 {
        field.marks.insert(QUEST_MARKS[i].into(), 1);
    }
    field.step(&BTreeMap::new());
    save(&field, "h-field-dressed", 1);
    // The Quiver Coach built, from the fletchery.
    fl.home.countryside.wardrobe = 2;
    fl.home.countryside.clothes = 7;
    fl.home.countryside.built |= 1 << PLOT;
    put(&mut fl, 1, (19.0, 8.6));
    fl.step(&BTreeMap::new());
    save(&fl, "i-quiver-coach", 1);
    // Its other end on Broadsheet Row.
    let mut row = in_room(RoomKind::Village, 1);
    row.home = fl.home.clone();
    put(&mut row, 1, (19.0, 6.5));
    row.step(&BTreeMap::new());
    save(&row, "j-village-coach", 1);
    // In the kingdom: longbow country's doors and their plaques.
    let mut k = Run::at_home(53, 1, None, Home::default(), Spoils::default());
    k.walk_kingdom(Some(geography::keep_stand()));
    k.floor_plates.clear();
    let geo = geography::geography(&k.kingdom.wishes);
    for (n, kind) in PLACES.into_iter().enumerate() {
        let site = geo.sites.iter().find(|s| s.room == kind).unwrap();
        let h = k.players.get_mut(&1).unwrap();
        (h.x, h.y) = ((site.stand.0 as f32 + 0.5) * TILE_UNITS, (site.stand.1 as f32 + 1.5) * TILE_UNITS);
        h.camera = Some((h.x, h.y - 1.0));
        k.step(&BTreeMap::new());
        save(&k, &format!("k-kingdom-{n}"), 1);
    }
}
