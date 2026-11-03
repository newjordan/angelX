use super::*;

fn e(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

/// Knight `id` stands just south of the altar's lamp, holding wishes.
fn at_the_lamp(run: &mut Run, id: u32, wishes: u8) {
    let (x, y) = run.wish_altar().expect("an altar in this room");
    let hero = run.players.get_mut(&id).unwrap();
    hero.x = x;
    hero.y = y + 1.0;
    hero.wishes = wishes;
    hero.great_wishes = 0;
}

#[test]
fn the_first_room_keeps_a_wish_altar_on_clear_floor() {
    let run = Run::new(7, 1, None);
    assert_eq!(run.room().kind, RoomKind::Start);
    let (x, y) = run.wish_altar().expect("the start room has an altar");
    let room = run.room();
    let (c, r) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
    for (dc, dr) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0)] {
        assert_eq!(room.tile(c + dc, r + dr), Tile::Floor, "the altar stands on clear floor");
    }
}

#[test]
fn e_at_the_lamp_opens_a_knights_wish_line_once_per_press() {
    let mut run = Run::new(7, 1, None);
    at_the_lamp(&mut run, 1, 2);
    assert!(run.near_altar(1));
    run.step(&e(1));
    let first = *run.lamp.rubbed.get(&1).expect("the lamp was rubbed");
    // Held down, it rubs once.
    run.step(&e(1));
    assert_eq!(run.lamp.rubbed[&1], first);
    // Let go and pressed again: a newer rub, for the seat to open again.
    run.step(&BTreeMap::new());
    run.step(&e(1));
    assert!(run.lamp.rubbed[&1] > first);
    assert_eq!(run.players[&1].wishes, 2, "rubbing spends nothing: the wish is spent when it is sealed");
}

#[test]
fn a_knight_with_no_wish_is_told_so_at_the_lamp() {
    let mut run = Run::new(7, 1, None);
    at_the_lamp(&mut run, 1, 0);
    run.step(&e(1));
    assert!(run.lamp.rubbed.is_empty());
    assert!(run.found.as_ref().is_some_and(|(_, id, line)| *id == 1 && line.contains("NO WISH")), "{:?}", run.found);
}

#[test]
fn e_away_from_the_lamp_does_not_rub_it() {
    let mut run = Run::new(7, 1, None);
    at_the_lamp(&mut run, 1, 2);
    run.players.get_mut(&1).unwrap().x += 12.0 * TILE_UNITS;
    assert!(!run.near_altar(1));
    run.step(&e(1));
    assert!(run.lamp.rubbed.is_empty());
}

#[test]
fn a_deeper_floor_begins_at_an_altar_too() {
    let mut run = Run::new(7, 1, None);
    run.descend_for_test();
    assert!(run.dungeon.depth >= 2);
    assert!(run.wish_altar().is_some(), "the floor's first room ({:?}) keeps an altar", run.room().kind);
}

#[test]
fn an_open_floor_keeps_its_altar_beside_the_arrival() {
    for seed in 1..6u64 {
        let mut rng = Rng::new(seed);
        let floor = layout::open_floor(1, Pack::Crypt, 2, &mut rng);
        let open = floor.open.as_ref().expect("an open floor");
        let map = &floor.rooms[open.map];
        let (x, y) = altar::spot(map, Some(open.arrival)).expect("an altar beside the arrival");
        let (ax, ay) = open.arrival;
        assert!((x - ax).hypot(y - ay) < 20.0 * TILE_UNITS, "seed {seed}: near the arrival");
        let (c, r) = ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
        assert_eq!(map.tile(c, r), Tile::Floor);
        assert_eq!(map.tile(c, r - 1), Tile::Floor);
    }
}

#[test]
fn a_fight_room_has_no_altar() {
    let mut run = Run::new(7, 1, None);
    let fight = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap();
    run.enter_for_test(fight);
    assert!(run.wish_altar().is_none());
}

#[test]
fn a_grant_lands_on_its_knight_in_the_middle_of_a_fight() {
    let mut run = Run::new(7, 1, None);
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(fight);
    // The fight is on: monsters stand and the knight is shooting.
    for _ in 0..30 {
        run.step(&BTreeMap::from([(1, Input { aim_x: 1, fire: true, ..Default::default() })]));
    }
    assert!(!run.enemies.is_empty(), "the fight goes on");
    run.lamp_sent(1, "my arrows bounce off walls");
    assert_eq!(run.lamp.pending(1).count(), 1);
    let wish = phrasebook::get("ricochet").unwrap();
    let name = run.grant(1, &wish).expect("granted mid-fight");
    run.lamp_landed(1, "my arrows bounce off walls", &name);
    assert!(run.players[&1].deck.iter().any(|c| c == "wish-ricochet-1"));
    assert!(run.lamp.wishing.is_empty(), "no longer in flight");
    assert!(run.lamp.flaring(run.tick).is_some_and(|l| l.knight == 1), "the lamp flares");
    // The knight fights on with it, the room still being fought.
    for _ in 0..10 {
        run.step(&BTreeMap::from([(1, Input { aim_x: 1, fire: true, ..Default::default() })]));
    }
    assert!(run.players[&1].hp > 0);
    assert!(run.lamp.flaring(run.tick + altar::FLARE_TICKS).is_none(), "the flare fades");
}

#[test]
fn the_lamp_rides_the_wire_only_once_something_happened_at_it() {
    let mut run = Run::new(7, 1, None);
    let quiet = crate::wire::publish(&run).unwrap();
    assert!(quiet.field("lamp").is_none());
    run.lamp_sent(2, "a frog body");
    let tree = crate::wire::publish(&run).unwrap();
    let back = crate::wire::read(&tree).unwrap();
    assert_eq!(back.lamp.wishing, run.lamp.wishing);
}

#[test]
fn the_altar_is_stone_a_knight_does_not_walk_into_it() {
    let mut run = Run::new(7, 1, None);
    let (x, y) = run.wish_altar().unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = x;
    hero.y = y - 0.2 * TILE_UNITS;
    run.step(&BTreeMap::new());
    let hero = &run.players[&1];
    assert!(hero.y >= y + 0.5 || hero.y <= y - 1.4 * TILE_UNITS, "pushed out: {:.1} vs altar at {y:.1}", hero.y);
    assert!(run.near_altar(1), "and still at the lamp");
}

#[test]
fn a_knight_walking_into_the_altar_goes_round_it() {
    // A corner of the stone, not its middle: walking into that end slides
    // along the edge and comes out beside the altar, never through it.
    let mut run = Run::new(7, 1, None);
    run.open_room_for_test(0);
    run.enemies.clear();
    let (ax, ay) = run.wish_altar().unwrap();
    let (left, right, top, foot) = (ax - 1.5 * TILE_UNITS, ax + 1.5 * TILE_UNITS, ay - 1.4 * TILE_UNITS, ay + 0.6);
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = right + 0.2;
    hero.y = top + 0.4;
    hero.invulnerable = 100_000;
    let west = BTreeMap::from([(1, Input { move_x: -1, ..Input::default() })]);
    for _ in 0..HZ * 4 {
        run.step(&west);
        let h = &run.players[&1];
        let through = h.x > left + 0.15 && h.x < right - 0.15 && h.y > top + 0.15 && h.y < foot - 0.15;
        assert!(!through, "went through the stone at ({:.1}, {:.1})", h.x, h.y);
    }
    let hero = &run.players[&1];
    assert!(
        hero.x < left,
        "walking into the corner should carry round it, at {:.1} (stone {:.1}..{:.1})",
        hero.x,
        left,
        right
    );
    assert!(
        hero.y <= top + 0.15,
        "went round the top edge, not through, y {:.1} top {:.1}",
        hero.y,
        top
    );
}

#[test]
fn a_knight_walking_into_the_middle_of_the_altar_stays_on_the_near_side() {
    // An open room, straight into the middle of a face: the stone holds.
    // They stay outside it, on the side they walked in from.
    let mut run = Run::new(7, 1, None);
    run.open_room_for_test(0);
    run.enemies.clear();
    let (ax, ay) = run.wish_altar().unwrap();
    let (left, right, top, foot) = (ax - 1.5 * TILE_UNITS, ax + 1.5 * TILE_UNITS, ay - 1.4 * TILE_UNITS, ay + 0.6);
    let mid = (top + foot) * 0.5;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = right + 0.3;
    hero.y = mid;
    hero.invulnerable = 100_000;
    let west = BTreeMap::from([(1, Input { move_x: -1, ..Input::default() })]);
    for _ in 0..HZ * 2 {
        run.step(&west);
        let h = &run.players[&1];
        assert!(
            h.x >= right - 0.05,
            "stayed on the near side, at {:.1} (stone {:.1}..{:.1})",
            h.x,
            left,
            right
        );
        assert!(
            !(h.x > left && h.x < right && h.y > top && h.y < foot),
            "outside the stone at ({:.1}, {:.1})",
            h.x,
            h.y
        );
    }
}

#[test]
fn the_altar_is_not_stood_where_its_stone_fills_a_passage() {
    // A three-tile passage between two chambers. The search begins in the
    // passage; standing there would seal the far chamber.
    let cols = 30;
    let rows = 20;
    let tiles = vec![Tile::Wall; cols * rows];
    let mut room = Room::kingdom(tiles, cols, rows);
    let carve = |room: &mut Room, c0: usize, c1: usize, r0: usize, r1: usize| {
        for r in r0..r1 {
            for c in c0..c1 {
                room.set(c, r, Tile::Floor);
            }
        }
    };
    carve(&mut room, 8, 22, 2, 7);
    carve(&mut room, 13, 16, 7, 14);
    carve(&mut room, 8, 22, 14, 18);
    // Anchor (tile(x) - 6, tile(y) - 2) = (14, 10), in the passage.
    let found = spot(&room, Some((41.0, 25.0))).expect("a chamber still has room for the altar");
    let (c, r) = ((found.0 / TILE_UNITS) as i32, (found.1 / TILE_UNITS) as i32 - 1);
    assert!(
        !stone_cuts_a_passage(&room, c, r),
        "altar at ({c}, {r}) seals a passage"
    );
}

#[test]
fn the_altar_spot_is_chosen_once_and_kept() {
    let mut run = Run::new(7, 1, None);
    let spot = run.wish_altar().unwrap();
    // The stone stands on this tile. Wall its footprint so a fresh search
    // would stand the altar somewhere else. The kept spot does not move.
    let (c, r) = ((spot.0 / TILE_UNITS) as i32, (spot.1 / TILE_UNITS) as i32 - 1);
    {
        let room = &mut run.dungeon.rooms[run.at];
        for dc in -1..=1 {
            for dr in 0..=1 {
                room.set((c + dc) as usize, (r + dr) as usize, Tile::Wall);
            }
        }
    }
    assert_eq!(run.wish_altar(), Some(spot), "the altar stays where it was stood");
    assert!(
        super::place(run.room(), None, &[]).is_none_or(|found| found != spot),
        "the search would have moved; the test must see that"
    );
}

#[test]
fn an_old_save_picks_the_altar_when_it_loads() {
    let run = Run::new(5, 1, None);
    let spot = run.wish_altar().unwrap();
    use angel_net::doc::value::Value;
    let mut tree = crate::wire::publish(&run).expect("a run makes a tree");
    let dungeon = tree.field_mut("dungeon").expect("dungeon");
    let Value::Struct(fields) = dungeon else { panic!("dungeon") };
    assert!(fields.iter().any(|(name, _)| name == "altar_at"), "the kept spot is on the wire");
    fields.retain(|(name, _)| name != "altar_at");
    let loaded = crate::wire::read(&tree).expect("an old save still loads");
    assert_eq!(loaded.wish_altar(), Some(spot));
}

#[test]
fn a_rub_breathes_and_the_genie_shimmers_up_a_breath_after() {
    let mut run = Run::new(7, 1, None);
    at_the_lamp(&mut run, 1, 2);
    run.sounds.clear();
    run.step(&e(1));
    let rubbed = run.tick;
    assert!(run.sounds.contains(&altar::SOUND_RUB) && !run.sounds.contains(&"spell_cast"), "{:?}", run.sounds);
    let mut genie = Vec::new();
    for _ in 0..40 {
        run.sounds.clear();
        run.step(&BTreeMap::new());
        if run.sounds.contains(&altar::SOUND_GENIE) {
            genie.push(run.tick);
        }
    }
    // Once, as the picture has him rise out of the smoke.
    assert_eq!(genie.len(), 1, "{genie:?}");
    assert_eq!(genie[0] - rubbed, altar::GENIE_AT);
}

#[test]
fn a_grant_lands_grander_at_each_tier() {
    use crate::rules::overclass::{LAND_SOUNDS, land_sound};
    assert_eq!([0, 1, 2, 3, 4, 9].map(land_sound), [LAND_SOUNDS[0], LAND_SOUNDS[0], LAND_SOUNDS[1], LAND_SOUNDS[1], LAND_SOUNDS[2], LAND_SOUNDS[2]]);
    let mut run = Run::new(7, 1, None);
    let wish = phrasebook::get("ricochet").unwrap();
    run.sounds.clear();
    run.grant(1, &wish).expect("granted");
    assert!(run.sounds.contains(&"wish_land") && !run.sounds.contains(&"wish_grant"), "{:?}", run.sounds);
    // Climbing on to III in the same breath lands once, at III.
    run.grant(1, &wish).expect("II");
    run.grant(1, &wish).expect("III");
    let landed: Vec<&str> = run.sounds.iter().copied().filter(|s| LAND_SOUNDS.contains(s)).collect();
    assert_eq!(landed, ["wish_land_grand"]);
    // A later wish lands on its own, even with the last still unheard.
    run.step(&BTreeMap::new());
    run.grant(1, &phrasebook::get("seeker").unwrap()).expect("granted");
    let landed: Vec<&str> = run.sounds.iter().copied().filter(|s| LAND_SOUNDS.contains(s)).collect();
    assert_eq!(landed, ["wish_land_grand", "wish_land"]);
}
