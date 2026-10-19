use super::*;
use crate::realm::chivalry::HERD_CAP;
use crate::rules::home::{BUY_HOLD, DESCEND_HOLD, Home};
use crate::rules::world::{self, Dest, ENTRANCES};
use std::collections::VecDeque;

fn press_e() -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )])
}

/// At home, the world raised, knight one standing in Sir Damp's Marsh.
fn in_the_marsh() -> Run {
    let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
    run.come_to_the_gate();
    let marsh = world::room_of(&run.dungeon, RoomKind::Marsh).expect("the Marsh is a room of the world");
    run.arrive(marsh, (12.0, 7.0));
    run.step(&BTreeMap::new());
    run
}

/// Floor `depth` of the swamp, as the plate takes a party down.
fn swamp_floor(seed: u64, depth: u32) -> Run {
    let mut run = Run::at_home(seed, 1, None, Home::default(), Default::default());
    run.begin_in(Pack::Swamp);
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    run
}

/// The swamp's last floor, the party walked into the Mire King's hall.
fn mire_kings_hall(seed: u64) -> Run {
    let mut run = swamp_floor(seed, FLOORS);
    let hall = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .expect("the last floor has the Mire King's hall");
    run.enter(hall, Some(2));
    run
}

fn reachable(floor: &Floor) -> Vec<bool> {
    let mut seen = vec![false; floor.rooms.len()];
    seen[0] = true;
    let mut todo = VecDeque::from([0]);
    while let Some(at) = todo.pop_front() {
        for dir in 0..4 {
            if let Some(next) = floor.neighbour(at, dir)
                && !seen[next]
            {
                seen[next] = true;
                todo.push_back(next);
            }
        }
    }
    seen
}

#[test]
fn the_wetland_plate_leads_into_the_swamp_delve() {
    let plate = ENTRANCES.iter().find(|e| e.id == "swamp-plate").unwrap();
    assert_eq!(plate.room, RoomKind::Marsh, "the plate is in the realm's Swamp, Sir Damp's Marsh");
    assert_eq!(plate.to, Dest::Delve(Pack::Swamp));
    assert!(plate.needs.is_none(), "it stands open from the first");
    // The Marsh is walked to from the gate, room by room.
    let mut run = in_the_marsh();
    let gate = world::room_of(&run.dungeon, RoomKind::Gate).unwrap();
    let marsh = world::room_of(&run.dungeon, RoomKind::Marsh).unwrap();
    let mut seen = vec![false; run.dungeon.rooms.len()];
    seen[gate] = true;
    let mut todo = VecDeque::from([gate]);
    while let Some(at) = todo.pop_front() {
        for dir in 0..4 {
            if let Some(next) = run.dungeon.neighbour(at, dir)
                && !seen[next]
            {
                seen[next] = true;
                todo.push_back(next);
            }
        }
    }
    assert!(seen[marsh], "the gate's roads reach the Marsh");
    let (c, r, w, h) = PLATE;
    for row in r..r + h {
        for col in c..c + w {
            assert_eq!(run.room().tile(col, row), Tile::Stairs);
            assert_eq!(world::entrance_at(RoomKind::Marsh, col, row).map(|e| e.id), Some("swamp-plate"));
        }
    }
    // Stood on a moment, it takes the party down to floor one of the swamp.
    let (x, y) = world::entrance_centre(plate);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x, y);
    for _ in 0..DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.dungeon.pack, Pack::Swamp);
    assert_eq!(run.dungeon.depth, 1);
    assert!(run.cues.iter().any(|c| c == "entrance:swamp-plate"));
    assert_eq!(run.floor_plate(None).map(|p| p.kind), Some(floor_plates::Kind::Swamp));
}

#[test]
fn the_notice_by_the_plate_says_where_the_horse_went() {
    let mut run = in_the_marsh();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (NOTICE.0 * TILE_UNITS, (NOTICE.1 + 1.0) * TILE_UNITS);
    run.step(&press_e());
    assert_eq!(run.marks.get(NOTICE_MARK), Some(&1));
    assert_eq!(run.found.as_ref().map(|f| f.2.as_str()), Some(NOTICE_TEXT));
    let mut country = Country::default();
    let mut stable = Stable::default();
    let (changed, _) = settle(&mut country, &mut stable, &run.marks);
    assert!(changed && country.swamp_heard);
    // And Wren pins a bounty for her, counted by the moment she is freed.
    let bounty = bounties::bounty("white_horse").unwrap();
    assert_eq!(bounty.goal, bounties::Goal::Moment(MOMENT));
    assert_eq!(bounty.from, 0, "on the board from the first");
}

#[test]
fn the_swamp_is_three_floors_of_mire_that_lay_out_and_are_reachable() {
    for d in 1..=DEEPEST {
        assert_eq!(Pack::at(d, Pack::Swamp), Pack::Swamp, "mire all the way down");
    }
    for seed in 0..10u64 {
        for depth in 1..=FLOORS {
            for party in 1..=4usize {
                let floor = layout::floor_for_party(
                    fortune::Mode::LongWayDown,
                    depth,
                    Pack::Swamp,
                    party,
                    &mut Rng::new(seed * 31 + u64::from(depth)),
                );
                let seen = reachable(&floor);
                for (i, room) in floor.rooms.iter().enumerate() {
                    // A vault behind a cracked wall waits for a bomb.
                    assert!(seen[i] || room.kind == RoomKind::Secret, "seed {seed} floor {depth}: {:?} reached", room.kind);
                }
                for room in &floor.rooms {
                    assert!(layout::connected(room), "seed {seed} floor {depth}: {:?} connected", room.kind);
                }
                assert!(!floor.rooms.iter().any(|r| r.kind == RoomKind::Lair), "no dragon in the swamp");
                let halls: Vec<&Room> = floor.rooms.iter().filter(|r| r.kind == RoomKind::Stairs).collect();
                assert_eq!(halls.len(), 1, "one way on, or the King's hall");
                let stairs = (0..halls[0].rows as i32)
                    .flat_map(|r| (0..halls[0].cols as i32).map(move |c| (c, r)))
                    .any(|(c, r)| halls[0].tile(c, r) == Tile::Stairs);
                assert_eq!(stairs, depth < FLOORS, "floor {depth}: the last hall has no way down");
                let bog: usize = floor
                    .rooms
                    .iter()
                    .filter(|r| r.kind == RoomKind::Fight)
                    .map(|r| {
                        (0..r.rows as i32)
                            .flat_map(|y| (0..r.cols as i32).map(move |x| (x, y)))
                            .filter(|&(x, y)| r.tile(x, y) == Tile::Bog)
                            .count()
                    })
                    .sum();
                assert!(bog > 0, "seed {seed} floor {depth}: the fights stand in mire");
            }
        }
    }
    // The wheel's horde and gauntlet keep away: the swamp keeps its shape.
    for mode in [fortune::Mode::HoldTheStair, fortune::Mode::Gauntlet] {
        let floor = layout::floor_for_party(mode, FLOORS, Pack::Swamp, 1, &mut Rng::new(5));
        assert!(floor.rooms.iter().any(|r| r.kind == RoomKind::Stairs));
        assert!(!floor.rooms.iter().any(|r| r.kind == RoomKind::Lair));
    }
}

#[test]
fn bog_takes_its_share_and_keeps_the_lanes_and_the_reed_islands_dry() {
    for depth in 1..=FLOORS {
        let mut q: Vec<Vec<u8>> = vec![b"...........".to_vec(); 6];
        q[2][4] = b'#';
        mire(&mut q, depth, salt((1, 2), depth));
        // Of the open ground, the share asked; no more than lies off the
        // lanes and the island (the 9 by 4 corner, less the island's 3 by 3).
        let (open, off_the_lanes) = (66 - 1, 9 * 4 - 9);
        let bog = q.iter().flatten().filter(|&&c| c == b',').count();
        assert_eq!(bog, (open * BOG_SHARE[(depth - 1) as usize] / 100).min(off_the_lanes), "floor {depth}");
        for row in &q {
            assert!(row[9..].iter().all(|&c| c != b','), "the lanes to the doors stay dry");
        }
        assert!(q[4..].iter().flatten().all(|&c| c != b','), "the crossing stays dry");
        for row in &q[1..=3] {
            assert!(row[3..=5].iter().all(|&c| c != b','), "the reed island stays dry");
        }
    }
}

#[test]
fn fog_lies_in_patches_on_the_bog_clear_of_the_doors() {
    let mut patched = 0;
    for seed in 0..8u64 {
        for depth in 1..=FLOORS {
            let floor = layout::floor(depth, Pack::Swamp, &mut Rng::new(seed));
            for room in &floor.rooms {
                let patches = fog(Pack::Swamp, room, depth);
                if !matches!(room.kind, RoomKind::Fight | RoomKind::Stairs) {
                    assert!(patches.is_empty(), "{:?} keeps clear", room.kind);
                    continue;
                }
                assert!(patches.len() <= FOG_PATCHES[(depth - 1) as usize]);
                patched += usize::from(!patches.is_empty());
                let (w, h) = (room.width(), room.height());
                let doors = [(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)];
                for &(x, y) in &patches {
                    assert_eq!(room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32), Tile::Bog);
                    for (side, door) in doors.into_iter().enumerate() {
                        if room.doors[side] {
                            assert!((door.0 - x).hypot(door.1 - y) >= 8.0, "fog keeps off the doorway");
                        }
                    }
                }
                assert!(fog(Pack::Crypt, room, depth).is_empty(), "no fog outside the swamp");
            }
        }
    }
    assert!(patched > 20, "most of the swamp's fights are foggy: {patched}");
}

#[test]
fn the_swamp_rosters_come_from_the_mire_and_grow_worse_with_depth() {
    use EnemyKind::*;
    let allowed = [Leech, Slime, Spitter, Wraith, Hexer, Whirlgrave, BogHand, Drowned, Wisp];
    let kinds = |depth: u32| -> Vec<EnemyKind> { Pack::Swamp.roster_at(depth).iter().map(|&(k, _)| k).collect() };
    for depth in 1..=FLOORS {
        assert!(kinds(depth).iter().all(|k| allowed.contains(k)), "floor {depth}");
    }
    let first = kinds(1);
    for later in [Drowned, Hexer, Whirlgrave, Spitter] {
        assert!(!first.contains(&later), "floor one meets no {later:?}");
    }
    assert!(kinds(2).contains(&Drowned) && kinds(2).contains(&Hexer));
    assert!(kinds(3).contains(&Whirlgrave));
    for kind in [BogHand, Drowned, Wisp] {
        assert!((1..=FLOORS).any(|d| kinds(d).contains(&kind)), "{kind:?} lives in the swamp");
    }
    // The rooms laid out hold the swamp's own.
    for seed in 0..12u64 {
        for depth in 1..=FLOORS {
            let floor = layout::floor(depth, Pack::Swamp, &mut Rng::new(seed));
            for room in floor
                .rooms
                .iter()
                .filter(|r| matches!(r.kind, RoomKind::Fight | RoomKind::Stairs | RoomKind::Start))
            {
                assert!(room.roster.iter().all(|k| allowed.contains(k)), "seed {seed} floor {depth}: {:?}", room.roster);
            }
        }
    }
}

#[test]
fn bog_slows_walkers_not_shots_wings_or_the_swamps_own() {
    let mut room = Room::solid_rock((0, 0));
    for row in 1..ROWS - 1 {
        for col in 1..COLS - 1 {
            room.set_for_test(col, row, if col < 12 { Tile::Floor } else { Tile::Bog });
        }
    }
    let grid = Grid { room: &room, barred: false };
    let dry = (6.0 * TILE_UNITS, 6.0 * TILE_UNITS);
    let wet = (16.0 * TILE_UNITS, 6.0 * TILE_UNITS);
    let walked = |from: (f32, f32), mover: Mover| grid.slide(from, (0.5, 0.0), 0.4, mover).0 - from.0;
    assert!((walked(dry, Mover::Hero) - 0.5).abs() < 1e-5);
    assert!((walked(wet, Mover::Hero) - 0.5 * BOG_PACE).abs() < 1e-5, "a knight wades");
    assert!((walked(wet, Mover::Walker) - 0.5 * BOG_PACE).abs() < 1e-5, "a walking monster wades");
    assert!((walked(wet, Mover::Flier) - 0.5).abs() < 1e-5, "wings pass over");
    assert!((walked(wet, Mover::Wader) - 0.5).abs() < 1e-5, "the swamp's own wade it as dry ground");
    assert!(!grid.stops_shot(wet.0, wet.1), "shots fly over the bog");
    assert!(grid.clear(wet.0, wet.1, HERO_RADIUS, Mover::Hero), "it is walkable");
    // In play: a knight crossing bog covers BOG_PACE of the ground, and a
    // shot over it flies at full speed.
    let mut run = swamp_floor(3, 1);
    let fight = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap();
    run.enter_for_test(fight);
    run.enemies.clear();
    run.calm_for_test();
    let (w, h) = (run.room().width(), run.room().height());
    let (cols, rows) = (run.room().cols, run.room().rows);
    for row in 1..rows - 1 {
        for col in 1..cols - 1 {
            run.dungeon.rooms[fight].set_for_test(col, row, if row < rows / 2 { Tile::Floor } else { Tile::Bog });
        }
    }
    let walk = BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]);
    let stride = |run: &mut Run, y: f32| {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (6.0, y);
        for _ in 0..10 {
            run.step(&walk);
        }
        run.players[&1].x - 6.0
    };
    let on_dry = stride(&mut run, h / 2.0 - 4.0);
    let on_bog = stride(&mut run, h / 2.0 + 4.0);
    assert!((on_bog / on_dry - BOG_PACE).abs() < 0.05, "dry {on_dry}, bog {on_bog}");
    run.projectiles.push(Projectile {
        x: 6.0,
        y: h / 2.0 + 6.0,
        vx: 9.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Orb,
        damage: 1,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 60,
    });
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (w - 6.0, 6.0);
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    let shot = run.projectiles.iter().find(|p| p.kind == Shot::Orb).expect("the shot flies on");
    assert!((shot.x - (6.0 + 9.0 * 10.0 * DT)).abs() < 0.05, "a shot over the bog is not slowed: {}", shot.x);
}

#[test]
fn the_mire_king_holds_the_pen_until_he_falls_then_she_walks_free_once() {
    let mut run = mire_kings_hall(5);
    let captive = run.captive.clone().expect("the white horse is penned in the last hall");
    assert_eq!(captive.who, HORSE);
    assert_eq!(captive.room, run.at);
    let king: Vec<&Enemy> = run.enemies.iter().filter(|e| e.is_boss()).collect();
    assert_eq!(king.len(), 1);
    assert_eq!(run.bosses[usize::from(king[0].boss.unwrap())].id, KING);
    assert_eq!(run.enemies.len(), 1, "on the third floor he stands alone (BOSS_ALONE_TO)");
    // While he stands, the pen holds.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (captive.x, captive.y + 2.0);
    hero.invulnerable = 999;
    for _ in 0..BUY_HOLD + 4 {
        run.step(&press_e());
    }
    assert!(run.captive.as_ref().unwrap().freed.is_none());
    assert!(!run.marks.contains_key(RESCUE_MARK));
    assert_eq!(run.found.as_ref().map(|f| f.2.as_str()), Some(PEN_HELD));
    assert!(run.light.is_none());
    // He falls: the hall is his floor's, banked; the pen opens.
    run.clear_for_test();
    assert!(run.room().cleared);
    assert!(run.cues.iter().any(|c| c == "mire_king_falls"));
    assert!(run.reclaimed.contains(&Pack::Swamp));
    assert!(run.light.is_none(), "the light waits for the horse");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (captive.x, captive.y + 2.0);
    for _ in 0..BUY_HOLD + 2 {
        run.step(&press_e());
    }
    assert!(run.captive.as_ref().unwrap().freed.is_some());
    assert_eq!(run.marks.get(RESCUE_MARK), Some(&1));
    assert!(run.cues.iter().any(|c| c == "rescued:white-horse"));
    assert!(run.cues.iter().any(|c| c == MOMENT));
    assert_eq!(run.marks.get(&bounties::moment_mark(MOMENT)), Some(&1));
    assert!(run.feats.contains(&"white_horse"));
    assert!(feats::feat("white_horse").is_some());
    // Once: holding on does nothing more.
    for _ in 0..BUY_HOLD * 3 {
        run.step(&press_e());
    }
    assert_eq!(run.marks.get(RESCUE_MARK), Some(&1));
    // The light leads home.
    let (lx, ly) = run.light.expect("the light home");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (lx, ly);
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Won);
    // There is no floor below the King's.
    let mut below = mire_kings_hall(5);
    below.descend_for_test();
    assert_eq!(below.dungeon.depth, FLOORS);
}

#[test]
fn the_upper_floors_end_in_their_hardest_fight_not_the_king() {
    for depth in 1..FLOORS {
        let mut run = swamp_floor(9, depth);
        assert!(run.captive.is_none(), "she is on the last floor only");
        let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
        run.enter(stairs, Some(2));
        assert!(!run.enemies.iter().any(|e| e.is_boss()), "floor {depth}: no guardian above the King's hall");
        assert!(!run.enemies.is_empty(), "floor {depth}: the stair is fought for");
    }
}

#[test]
fn the_swamps_floors_count_for_none_of_the_realms_depth_records() {
    for depth in 1..=FLOORS {
        let run = swamp_floor(9, depth);
        assert_eq!(run.dungeon.depth, depth);
        assert_eq!(run.record_floor(), 0, "swamp floor {depth}");
    }
    let mut run = Run::at_home(9, 1, None, Home::default(), Default::default());
    run.begin_in(Pack::Crypt);
    while run.dungeon.depth < 3 {
        run.descend_for_test();
    }
    assert_eq!(run.record_floor(), 3, "a keep floor counts as itself");
    // Wren's "Down to the Keep" is not done from the swamp's third floor,
    // and is from the keep's.
    let mut home = Home::default();
    home.deepest = 1;
    home.bounties = vec![crate::rules::bounties::Pinned { id: "keep".into(), have: 0 }];
    let swamp = swamp_floor(9, FLOORS).record_floor();
    assert!(home.work_bounties(&BTreeMap::new(), swamp).is_empty());
    assert_eq!(home.bounties[0].have, 0);
    let done = home.work_bounties(&BTreeMap::new(), run.record_floor());
    assert_eq!(done.iter().map(|b| b.id).collect::<Vec<_>>(), ["keep"]);
}

#[test]
fn a_realm_that_has_her_pens_nobody() {
    let mut home = Home::default();
    home.countryside.white_horse = 2;
    let mut run = Run::at_home(5, 1, None, home, Default::default());
    run.begin_in(Pack::Swamp);
    while run.dungeon.depth < FLOORS {
        run.descend_for_test();
    }
    assert!(run.captive.is_none());
    let mut home = Home::default();
    home.stable.herd.push(white_horse());
    let mut run = Run::at_home(5, 1, None, home, Default::default());
    run.begin_in(Pack::Swamp);
    while run.dungeon.depth < FLOORS {
        run.descend_for_test();
    }
    assert!(run.captive.is_none());
}

#[test]
fn freed_she_joins_the_herd_white_and_named_once_and_waits_when_the_yard_is_full() {
    let marks = BTreeMap::from([(RESCUE_MARK.to_string(), 1)]);
    let mut country = Country::default();
    let mut stable = Stable::default();
    let (changed, said) = settle(&mut country, &mut stable, &marks);
    assert!(changed);
    assert_eq!(country.white_horse, 2);
    assert_eq!(stable.herd.len(), 1);
    let her = &stable.herd[0];
    assert_eq!(her.name, HORSE_NAME);
    assert_eq!(her.coat, Some(HorseCoat::White));
    assert!(her.mare && her.raceable(0), "a grown mare: she races");
    assert!(said.iter().any(|s| s.contains(HORSE_NAME)));
    // Settled again, nothing doubles.
    let (changed, _) = settle(&mut country, &mut stable, &marks);
    assert!(!changed);
    assert_eq!(stable.herd.len(), 1);
    // A full yard: she waits at the ranch until there is room.
    let mut country = Country::default();
    let mut stable = Stable::default();
    for i in 0..HERD_CAP {
        stable.herd.push(Horse {
            name: format!("Nag {i}"),
            ..Horse::default()
        });
    }
    let (changed, said) = settle(&mut country, &mut stable, &marks);
    assert!(changed);
    assert_eq!(country.white_horse, 1);
    assert_eq!(stable.herd.len(), HERD_CAP);
    assert!(said.iter().any(|s| s.contains("full")));
    stable.sell(0);
    let (changed, _) = settle(&mut country, &mut stable, &BTreeMap::new());
    assert!(changed);
    assert_eq!(country.white_horse, 2);
    assert!(in_herd(&stable));
    // The cockpit settles her through the registry of places.
    let mut home = Home::default();
    let mut treasury = crate::realm::Spoils::default();
    let reclaimed = BTreeMap::new();
    let settled = activity::settle_all(
        &mut activity::Realm {
            home: &mut home,
            treasury: &mut treasury,
            reclaimed: &reclaimed,
            raids_won: 0,
        },
        &marks,
    );
    assert!(settled.changed);
    assert!(in_herd(&home.stable));
    assert!(settled.said.iter().any(|(cue, _)| cue == "swamp"));
}

#[test]
fn the_swamps_marks_are_kept_by_the_checkpoint_journal() {
    for mark in INTENT_MARKS {
        assert!(countryside::intent_key(mark), "{mark}");
        assert!(countryside::intent_valid(mark, 1));
        assert!(!countryside::intent_valid(mark, 2));
    }
}

#[test]
fn old_saves_load_without_the_swamp() {
    // A countryside saved before the swamp: no notice, no horse.
    let old = serde_json::json!({"visited": 3, "sluice": 6, "sluice_paid": true, "built": 512});
    let country = countryside::load(old);
    assert!(country.valid());
    assert!(!country.swamp_heard);
    assert_eq!(country.white_horse, 0);
    assert_eq!(country.sluice, 6);
    // A default country writes no swamp fields at all.
    let written = serde_json::to_value(Country::default()).unwrap();
    assert!(written.get("swamp_heard").is_none() && written.get("white_horse").is_none());
    // A stage past its end is mended.
    let mended = countryside::load(serde_json::json!({"white_horse": 9}));
    assert!(mended.valid() && mended.white_horse == 0);
    // A herd horse saved before coats.
    let horse: Horse = serde_json::from_value(serde_json::json!({
        "name": "Ashfall", "house": "qwen",
        "blood": {"pace": 5, "stamina": 5, "corner": 5, "nerve": 5}
    }))
    .unwrap();
    assert_eq!(horse.coat, None);
    assert!(serde_json::to_value(&horse).unwrap().get("coat").is_none());
    let stable: Stable =
        serde_json::from_value(serde_json::json!({"herd": [serde_json::to_value(&horse).unwrap()]})).unwrap();
    assert_eq!(stable.herd[0].coat, None);
    // Knights saved before the bog hands' grip.
    let raw = include_str!("fixtures/balance_party.json");
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let heroes: Vec<Hero> = serde_json::from_value(value["heroes"].clone()).unwrap();
    assert!(!heroes.is_empty() && heroes.iter().all(|h| h.mired == 0));
    // A home floor saved before the plate gets it as the world is raised.
    let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
    let marsh = world::room_of(&run.dungeon, RoomKind::Marsh).unwrap();
    let (c, r, w, h) = PLATE;
    for row in r..r + h {
        for col in c..c + w {
            run.dungeon.rooms[marsh].set_for_test(col as usize, row as usize, Tile::Floor);
        }
    }
    assert!(world::raise_world(&mut run.dungeon));
    assert_eq!(run.dungeon.rooms[marsh].tile(c, r), Tile::Stairs);
    // A whole run in the swamp, written and read back, is still a valid
    // snapshot.
    let mut run = mire_kings_hall(5);
    run.step(&BTreeMap::new());
    let back: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(back.valid_snapshot());
    assert_eq!(back.dungeon.pack, Pack::Swamp);
}

#[test]
fn the_mire_king_is_a_checked_boss_file_with_a_tell_before_each_attack() {
    let raw = include_str!("../../../../cockpit/assets/dungeon/bosses/the-mire-king.boss");
    let (king, notes) = bosses::check(KING, raw).expect("the Mire King passes the checker");
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(king.only_in, Pack::Swamp);
    assert!(bosses::builtin().iter().any(|b| b.id == KING));
    // Every attack he has shows itself first: the Ravage's ripples, the
    // rain's marks, the laser's line, the seekers' slow turn.
    for attack in &king.attacks {
        assert!(matches!(attack.pattern, bosses::Pattern::Ravage), "{:?}", attack.pattern);
    }
    let phases = king.phases.as_ref().expect("he walks a chain");
    for emitter in &phases.chain {
        assert!(
            matches!(
                emitter,
                patterns::Emitter::Rain | patterns::Emitter::Laser | patterns::Emitter::Seeker
            ),
            "{emitter:?}"
        );
    }
}

#[test]
fn the_pen_stands_on_dry_ground_a_knight_can_reach() {
    for seed in 0..12u64 {
        let run = swamp_floor(seed, FLOORS);
        let pen = run.captive.clone().expect("penned on the last floor");
        let room = &run.dungeon.rooms[pen.room];
        let (col, row) = ((pen.x / TILE_UNITS) as i32, (pen.y / TILE_UNITS) as i32);
        for r in row - 1..=row + 1 {
            for c in col - 1..=col + 1 {
                assert_eq!(room.tile(c, r), Tile::Floor, "seed {seed}: dry round the pen");
            }
        }
        assert!(layout::connected(room), "seed {seed}: the hall stays whole");
        let grid = Grid { room, barred: false };
        assert!(grid.clear(pen.x, pen.y + 2.0, HERO_RADIUS, Mover::Hero), "seed {seed}: a knight stands at the pen");
    }
}
