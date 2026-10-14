// Extra cases in the established host/mirror regression, not a separate harness.
fn wetland_adventure_regression(mut run: Run) {
    use super::super::{
        countryside as c,
        pastimes::{self, Game},
        wetland as wet,
    };
    use crate::rules::{Grid, HERO_RADIUS, Mover};
    for id in 2..=MAX_PLAYERS {
        run.join_seat(id, "Paddler");
    }
    for &(a, b) in TRAILS
        .iter()
        .filter(|(a, b)| wet::PLACES.contains(a) || wet::PLACES.contains(b))
    {
        for (a, b) in [(a, b), (b, a)] {
            let ai = room_of(&run.dungeon, a).unwrap();
            let bi = room_of(&run.dungeon, b).unwrap();
            let ac = run.dungeon.rooms[ai].cell;
            let bc = run.dungeon.rooms[bi].cell;
            let d = layout::DIRS
                .iter()
                .position(|&(x, y)| (ac.0 + x, ac.1 + y) == bc)
                .unwrap();
            run.arrive(ai, (12.0, 7.0));
            let (x, y, mx, my) = match d {
                0 => (24.0, 1.2, 0, -1),
                1 => (46.8, 14.0, 1, 0),
                2 => (24.0, 26.8, 0, 1),
                _ => (1.2, 14.0, -1, 0),
            };
            for (n, h) in run.players.values_mut().enumerate() {
                let offset = (n as f32 - 1.5) * 1.8;
                (h.x, h.y) = if d % 2 == 0 {
                    (x + offset, y)
                } else {
                    (x, y + offset)
                };
            }
            let inputs = run
                .players
                .keys()
                .map(|&id| {
                    (
                        id,
                        Input {
                            move_x: mx,
                            move_y: my,
                            ..Default::default()
                        },
                    )
                })
                .collect();
            for _ in 0..20 {
                run.step(&inputs);
                if run.at == bi {
                    break;
                }
            }
            assert_eq!(run.at, bi, "wetland road {a:?}->{b:?}");
            let grid = Grid {
                room: run.room(),
                barred: false,
            };
            let mut positions = std::collections::BTreeSet::new();
            for h in run.players.values() {
                assert!(
                    grid.clear(h.x, h.y, HERO_RADIUS, Mover::Hero),
                    "wetland arrival {b:?}"
                );
                assert!(positions.insert((h.x.to_bits(), h.y.to_bits())));
            }
        }
    }
    run = Run::at_home(41, 1, None, Home::default(), Default::default());
    run.join("Bucket's guest");
    run.players.get_mut(&1).unwrap().stone = true;
    let f = BTreeMap::from([(
        2,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    run.arrive(
        room_of(&run.dungeon, RoomKind::SunkenAbbey).unwrap(),
        wet::ERRAND[4].1,
    );
    run.step(&f);
    assert_eq!(wet::stage(&run.home.countryside, &run.marks), 0);
    // The valves drain the sluice only all open at once: the stage holds at
    // one until the third is turned, then the bell and the brother.
    for (s, &(kind, at)) in wet::ERRAND.iter().enumerate() {
        run.arrive(room_of(&run.dungeon, kind).unwrap(), at);
        let h = run.players.get_mut(&2).unwrap();
        (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
        run.step(&f);
        let want = if (1..3).contains(&s) { 1 } else { s as u8 + 1 };
        assert_eq!(
            wet::stage(&run.home.countryside, &run.marks),
            want,
            "stage {s} in {kind:?} / actual {:?} / h {:?}",
            run.room().kind,
            run.players.get(&2)
        );
        assert!(run.valid_snapshot());
    }
    assert!(wet::QUEST_MARKS.iter().all(|m| run.marks.get(*m) == Some(&1)));
    let mut country = c::Country::default();
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 200);
    treasury.add(Spoil::Bone, 4);
    treasury.add(Spoil::Ore, 2);
    let e = entrance("abbey-boat");
    assert!(!open(e, &run.home));
    assert!(c::PLOTS[9].blocked(&country).is_some());
    c::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury.get(Spoil::Gold), 265);
    assert_eq!(treasury.get(Spoil::Ore), 4);
    assert!(country.sluice_paid && !country.has(9));
    run.marks.insert("country:build:punthouse".into(), 1);
    c::settle(&mut country, &mut treasury, &run.marks);
    assert!(country.valid() && country.has(9));
    assert_eq!(treasury.get(Spoil::Gold), 160);
    assert_eq!(treasury.get(Spoil::Bone), 0);
    assert_eq!(treasury.get(Spoil::Ore), 2);
    let paid = treasury.clone();
    c::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury, paid);
    run.home.countryside = country;
    for (id, dest) in [
        ("abbey-boat", RoomKind::Orchard),
        ("orchard-boat", RoomKind::SunkenAbbey),
    ] {
        let e = entrance(id);
        run.arrive(room_of(&run.dungeon, e.room).unwrap(), (12.0, 7.0));
        for h in run.players.values_mut() {
            h.stone = false;
        }
        assert!(open(e, &run.home));
        take(&mut run, e);
        assert_eq!(run.room().kind, dest);
        let grid = Grid {
            room: run.room(),
            barred: false,
        };
        for h in run.players.values() {
            assert!(grid.clear(h.x, h.y, HERO_RADIUS, Mover::Hero));
        }
    }
    let old: c::Country=serde_json::from_value(serde_json::json!({"built":511,"visited":63,"ramble_paid":true,"relics":7,"lamps":7,"survey":7,"picnic":3,"forecast":6,"harbour":3,"soup":5,"games":{"best":[321,0,0,0],"plays":[2,0,0,0],"wins":[1,0,0,0]}})).unwrap();
    assert!(old.valid());
    assert_eq!(old.built, 511);
    assert_eq!(old.sluice, 0);
    assert_eq!(old.games.surf, Default::default());
    assert_eq!(old.core_visits(), 6);
    assert_eq!(c::PLOTS[2].blocked(&old), None);
    // Checkpoint migration and live geometry key refresh.
    run.arrive(room_of(&run.dungeon, RoomKind::Gate).unwrap(), (12.0, 9.5));
    run.dungeon.rooms.retain(|r| !wet::PLACES.contains(&r.kind));
    assert!(raise_world(&mut run.dungeon));
    assert!(!raise_world(&mut run.dungeon));
    // Full surf course through real Input/step: no round state injection.
    let mut game = Run::at_home(41, 1, None, Home::default(), Default::default());
    game.arrive(
        room_of(&game.dungeon, RoomKind::Millpond).unwrap(),
        Game::Surf.start(),
    );
    game.step(&BTreeMap::new());
    for _ in 0..Game::Surf.time() {
        let Some(r) = game.pastimes.round.as_ref() else {
            break;
        };
        let s = r.surf.as_ref().unwrap();
        let target = pastimes::WAVE_LEANS[usize::from(r.progress)];
        let fire = s.phase == pastimes::WAVE_TICKS - pastimes::CREST_WINDOW;
        let input = Input {
            aim_x: (target - s.lean).signum() as i8,
            fire,
            ..Default::default()
        };
        game.step(&BTreeMap::from([(1, input)]));
        assert!(game.valid_snapshot());
        let friend = game.wire_copy();
        assert_eq!(friend.pastimes, game.pastimes);
    }
    assert!(game.pastimes.round.is_none());
    assert_eq!(game.home.countryside.games.surf.wins, 1);
    assert!(game.score > 0);
    let records = game.home.countryside.games.clone();
    game.step(&BTreeMap::new());
    assert!(game.pastimes.round.is_none(), "plate cannot repeat itself");
    for h in game.players.values_mut() {
        (h.x, h.y) = (6.0, 23.0);
    }
    game.step(&BTreeMap::new());
    for h in game.players.values_mut() {
        (h.x, h.y) = (6.0, 18.0);
    }
    game.step(&BTreeMap::new());
    for _ in 0..pastimes::WAVE_TICKS + 1 {
        game.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: -1,
                fire: true,
                ..Default::default()
            },
        )]));
    }
    assert!(game.pastimes.round.is_none(), "held F is not a crest tap");
    assert_eq!(game.home.countryside.games.surf.wins, 1);
    assert_eq!(game.home.countryside.games.surf.plays, 2);
    assert_eq!(game.home.countryside.games.surf.best, records.surf.best);
    // Friend's fire cannot rescue owner's untouched board; departure stops it.
    game.join("Unhelpful surfer");
    game.arrive(
        room_of(&game.dungeon, RoomKind::Millpond).unwrap(),
        Game::Surf.start(),
    );
    game.step(&BTreeMap::new());
    let untouched = game
        .pastimes
        .round
        .as_ref()
        .unwrap()
        .surf
        .as_ref()
        .unwrap()
        .lean;
    game.step(&BTreeMap::from([(
        2,
        Input {
            aim_x: 1,
            fire: true,
            ..Default::default()
        },
    )]));
    assert_eq!(
        game.pastimes
            .round
            .as_ref()
            .unwrap()
            .surf
            .as_ref()
            .unwrap()
            .lean,
        untouched
    );
    let mut bad = game.clone();
    bad.pastimes
        .round
        .as_mut()
        .unwrap()
        .surf
        .as_mut()
        .unwrap()
        .phase = pastimes::WAVE_TICKS;
    assert!(!bad.valid_snapshot());
    game.players.get_mut(&1).unwrap().stone = true;
    game.step(&BTreeMap::new());
    assert!(game.pastimes.round.is_none());
}
