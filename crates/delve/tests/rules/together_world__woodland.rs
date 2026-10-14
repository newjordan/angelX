// Extends the existing host/mirror regression; all gameplay uses Run::step.
fn woodland_adventure_regression(mut run: Run) {
    use super::super::{
        countryside as c,
        pastimes::{self, Game},
        woodland as wood,
    };
    use crate::rules::{Grid, HERO_RADIUS, Mover};
    // Every authored woodland road works in both directions for four knights.
    for id in 2..=MAX_PLAYERS {
        run.join_seat(id, "Woodlander");
    }
    for &(a, b) in TRAILS
        .iter()
        .filter(|(a, b)| wood::PLACES.contains(a) || wood::PLACES.contains(b))
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
            assert_eq!(run.at, bi, "woodland road {a:?}->{b:?}");
            let grid = Grid {
                room: run.room(),
                barred: false,
            };
            let mut positions = std::collections::BTreeSet::new();
            for h in run.players.values() {
                assert!(
                    grid.clear(h.x, h.y, HERO_RADIUS, Mover::Hero),
                    "arrival {b:?}"
                );
                assert!(positions.insert((h.x.to_bits(), h.y.to_bits())));
            }
            let friend = run.wire_copy();
            assert_eq!(friend.at, run.at);
        }
    }
    // In-order spatial F actions. A guest records unit intents, not actor IDs.
    run = Run::at_home(31, 1, None, Home::default(), Default::default());
    run.come_to_the_gate();
    run.join("Badger's guest");
    run.players.get_mut(&1).unwrap().stone = true;
    run.arrive(
        room_of(&run.dungeon, RoomKind::HedgeMaze).unwrap(),
        wood::ERRAND[3].1,
    );
    let f = BTreeMap::from([(
        2,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    run.step(&f);
    assert_eq!(wood::stage(&run.home.countryside, &run.marks), 0);
    let press = |run: &mut Run, i: usize| {
        let (kind, at) = wood::ERRAND[i];
        run.arrive(room_of(&run.dungeon, kind).unwrap(), at);
        let h = run.players.get_mut(&2).unwrap();
        (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
        run.step(&f);
        assert!(run.valid_snapshot());
    };
    // The job; then the forage, any order, all three in the maze at once.
    press(&mut run, 0);
    assert_eq!(wood::stage(&run.home.countryside, &run.marks), 1);
    press(&mut run, 3);
    press(&mut run, 1);
    assert_eq!(run.forage.as_ref().unwrap().carried, [0, 0, 2], "the Boot kicked");
    assert!(run.forage.as_ref().unwrap().kicked);
    press(&mut run, 1);
    press(&mut run, 2);
    assert_eq!(run.forage.as_ref().unwrap().carried, [2, 2, 2]);
    assert_eq!(wood::stage(&run.home.countryside, &run.marks), 1, "not yet in the pot");
    press(&mut run, 4);
    assert_eq!(wood::stage(&run.home.countryside, &run.marks), 5);
    assert!(wood::QUEST_MARKS.iter().all(|m| run.marks.get(*m) == Some(&1)));
    assert!(run.forage.is_none(), "the pot has them");
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 200);
    treasury.add(Spoil::Bone, 3);
    treasury.add(Spoil::Wax, 2);
    let mut country = run.home.countryside.clone();
    let p = &c::PLOTS[8];
    assert!(p.blocked(&country).is_some());
    run.marks.insert("country:build:bothy".into(), 2);
    let before = treasury.clone();
    c::settle(&mut country, &mut treasury, &run.marks);
    assert!(country.valid() && country.has(8) && country.soup_paid);
    assert_eq!(treasury.get(Spoil::Gold), before.get(Spoil::Gold) + 70 - 95);
    assert_eq!(treasury.get(Spoil::Bone), 0);
    assert_eq!(treasury.get(Spoil::Wax), 2);
    let paid = treasury.clone();
    c::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury, paid);
    run.home.countryside = country;
    for (id, kind) in [
        ("bothy-wagon", RoomKind::Orchard),
        ("orchard-wagon", RoomKind::Bothy),
    ] {
        let e = entrance(id);
        run.arrive(room_of(&run.dungeon, e.room).unwrap(), (12.0, 7.0));
        for h in run.players.values_mut() {
            h.stone = false;
        }
        assert!(open(e, &run.home));
        take(&mut run, e);
        assert_eq!(run.room().kind, kind);
    }
    // Old numeric masks and the fixed first four records migrate losslessly.
    let old = serde_json::json!({"built":255,"harbour":3,"forecast":6,"picnic":3,"lamps":7,"relics":7,"survey":7,
        "visited":63,"ramble_paid":true,"games":{"best":[321,0,0,0],"plays":[2,0,0,0],"wins":[1,0,0,0]}});
    let migrated: c::Country = serde_json::from_value(old).unwrap();
    assert!(migrated.valid());
    assert_eq!(migrated.built, 255);
    assert_eq!(migrated.soup, 0);
    assert_eq!(migrated.core_visits(), 6);
    assert_eq!(c::PLOTS[2].blocked(&migrated), None);
    assert_eq!(migrated.games.cart, Default::default());
    assert_eq!(migrated.games.best[0], 321);
    // A missing old woodland topology is restored once, never below the surface.
    run.arrive(room_of(&run.dungeon, RoomKind::Gate).unwrap(), (12.0, 9.5));
    run.dungeon
        .rooms
        .retain(|r| !wood::PLACES.contains(&r.kind));
    assert!(raise_world(&mut run.dungeon));
    assert!(!raise_world(&mut run.dungeon));
    assert!(run.wire_copy().dungeon.rooms.iter().any(|r| wood::PLACES.contains(&r.kind)));
    // Real-input cart win, then held-F failure and departure cleanup.
    let mut game = Run::at_home(31, 1, None, Home::default(), Default::default());
    game.come_to_the_gate();
    game.arrive(
        room_of(&game.dungeon, RoomKind::Timberyard).unwrap(),
        Game::Cart.start(),
    );
    game.step(&BTreeMap::new());
    assert_eq!(game.home.countryside.games.plays_for(Game::Cart), 1);
    let mut held = false;
    let gold = game.treasury.clone();
    for _ in 0..40 * HZ {
        let Some(r) = game.pastimes.round.as_ref() else {
            break;
        };
        let c = r.cart.as_ref().unwrap();
        let fire = pastimes::CART_LOGS[usize::from(r.progress)] * 256 - c.x <= 5 * 36
            && c.rest_left == 0
            && c.air_left == 0
            && !held;
        game.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: 1,
                fire,
                ..Default::default()
            },
        )]));
        held = fire;
        assert!(game.valid_snapshot());
        let friend = game.wire_copy();
        assert_eq!(friend.pastimes, game.pastimes);
    }
    assert!(game.pastimes.round.is_none());
    assert_eq!(game.home.countryside.games.wins_for(Game::Cart), 1);
    assert!(game.home.countryside.games.best_for(Game::Cart) > 0);
    assert_eq!(game.treasury, gold);
    assert_eq!(game.home.countryside.games.best, [0; 4]);
    game.step(&BTreeMap::new());
    assert!(game.pastimes.round.is_none(), "must leave X to retry");
    game.players.get_mut(&1).unwrap().x += 4.0;
    game.step(&BTreeMap::new());
    game.players.get_mut(&1).unwrap().x -= 4.0;
    game.step(&BTreeMap::new());
    assert_eq!(game.home.countryside.games.cart.plays, 2);
    for _ in 0..8 * HZ {
        game.step(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]));
        if game.pastimes.round.is_none() {
            break;
        }
    }
    assert!(game.pastimes.round.is_none());
    assert_eq!(game.home.countryside.games.cart.wins, 1);
    assert!(game.found.as_ref().unwrap().2.contains("Grounded"));
    // Friends cannot drive the owner's cart; owner leaving cancels it safely.
    game.players.get_mut(&1).unwrap().x += 4.0;
    game.step(&BTreeMap::new());
    game.players.get_mut(&1).unwrap().x -= 4.0;
    game.step(&BTreeMap::new());
    let r = game.pastimes.round.as_ref().unwrap().clone();
    let mut invalid = game.pastimes.clone();
    invalid.round.as_mut().unwrap().cart.as_mut().unwrap().x = i32::MAX;
    assert!(!invalid.valid());
    for air in 0..=pastimes::CART_AIR {
        for dx in [8, 16, 36] {
            let mut copy = game.clone();
            let rr = copy.pastimes.round.as_mut().unwrap();
            let c = rr.cart.as_mut().unwrap();
            c.air_left = air;
            c.dx = dx;
            c.y = 5 * 256;
            c.dy = -28;
            assert!(copy.valid_snapshot());
            copy.step(&BTreeMap::new());
            assert!(copy.valid_snapshot(), "cart closure");
        }
    }
    game.join("Friend");
    let mut neutral = game.clone();
    game.step(&BTreeMap::from([(
        2,
        Input {
            aim_x: 1,
            aim_y: -1,
            fire: true,
            ..Default::default()
        },
    )]));
    neutral.step(&BTreeMap::new());
    assert_eq!(game.pastimes.round, neutral.pastimes.round);
    game.players.get_mut(&1).unwrap().stone = true;
    game.step(&BTreeMap::new());
    assert!(game.pastimes.round.is_none());
    assert!(r.cart.as_ref().unwrap().valid(&r));
}
