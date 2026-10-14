// Extend the existing write_world_shots exporter: captures are real host ticks.
fn coastal_world_shots(base: &Run, save: &impl Fn(&str, &Img)) {
    use crate::realm::Spoils;
    use crate::rules::home::Home;
    use crate::rules::{
        HZ, Input, RoomKind, coastal as coast, countryside as c, pastimes::Game, world,
    };
    use crate::render::overworld::arena::coastal as paint;
    use std::collections::BTreeMap;
    for (kind, name) in [
        (RoomKind::Strand, "world-strand.png"),
        (RoomKind::Saltworks, "world-saltworks.png"),
        (RoomKind::Lighthouse, "world-beacon.png"),
    ] {
        let mut r = base.clone();
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 9.5));
        let scenery = crate::render::overworld::arena::scenery(&r);
        assert!(
            scenery
                .rgba_scaled(1)
                .chunks_exact(4)
                .filter(|px| px[3] > 0)
                .count()
                > 10_000
        );
        save(name, &native(&r));
    }
    let mut quest = base.clone();
    for (i, &(kind, at)) in coast::ERRAND.iter().enumerate() {
        if i == 1 {
            // The chase: in from the west, the crab dug in at high tide,
            // out at low tide, then grabbed.
            quest.arrive(world::room_of(&quest.dungeon, kind).unwrap(), (6.0, 7.0));
            quest.tick = u64::from(9 * HZ);
            quest.step(&BTreeMap::new());
            assert!(quest.crab.as_ref().is_some_and(|c| c.dug), "{:?} stage {} at {:?} round {:?}", quest.crab, coast::stage(&quest.home.countryside, &quest.marks), quest.room().kind, quest.pastimes.round.as_ref().map(|r| r.game));
            assert!(paint::board_visible(&quest));
            save("world-tide-high.png", &native(&quest));
            while !coast::low_tide(quest.tick + 1) {
                quest.step(&BTreeMap::new());
            }
            quest.step(&BTreeMap::new());
            save("world-tide-low.png", &native(&quest));
            let crab = quest.crab.clone().unwrap();
            let h = quest.players.get_mut(&1).unwrap();
            (h.x, h.y) = (crab.x, crab.y);
            quest.step(&BTreeMap::new());
            assert_eq!(coast::stage(&quest.home.countryside, &quest.marks), 2);
            continue;
        }
        quest.arrive(world::room_of(&quest.dungeon, kind).unwrap(), at);
        assert!(paint::board_visible(&quest));
        let mut plain = native(&quest);
        let before = plain.clone();
        paint::boards(&mut plain, &quest);
        let (x, y) = crate::render::overworld::arena::at(at.0 * 2.0, at.1 * 2.0);
        for dy in -26..=26 {
            for dx in -26..=26 {
                assert_eq!(
                    before.get(x + dx, y + dy),
                    plain.get(x + dx, y + dy),
                    "quest HUD covers ring at {kind:?}"
                );
            }
        }
        save(
            if i == 0 {
                "world-beacon-quest.png"
            } else {
                "world-beacon-return.png"
            },
            &native(&quest),
        );
        quest.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    let mut home = Home::default();
    let mut purse = Spoils::default();
    c::settle(&mut home.countryside, &mut purse, &quest.marks);
    assert_eq!(home.countryside.harbour, 3);
    let mut built = quest.clone();
    home.countryside.built |= 128;
    built.rebuild_home(home, purse);
    save("world-beacon-lit.png", &native(&built));
    // Painting cached state both before and after construction must change.
    let salt = world::room_of(&built.dungeon, RoomKind::Saltworks).unwrap();
    quest.arrive(salt, c::PLOTS[7].at);
    built.arrive(salt, c::PLOTS[7].at);
    assert_ne!(
        crate::render::overworld::arena::scenery(&quest).rgba_scaled(1),
        crate::render::overworld::arena::scenery(&built).rgba_scaled(1)
    );
    save("world-harbour-built.png", &native(&built));
    for id in ["saltworks-barge", "quay-barge"] {
        let e = world::ENTRANCES.iter().find(|e| e.id == id).unwrap();
        let at = world::entrance_centre(e);
        built.arrive(
            world::room_of(&built.dungeon, e.room).unwrap(),
            (at.0 / 2.0, at.1 / 2.0),
        );
        save(
            if e.room == RoomKind::Saltworks {
                "world-saltworks-barge.png"
            } else {
                "world-quay-barge.png"
            },
            &native(&built),
        );
    }
    let mut r = base.clone();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Strand).unwrap(),
        Game::Pot.start(),
    );
    r.step(&BTreeMap::new());
    assert!(r.pastimes.round.is_some());
    save("world-pot-start.png", &native(&r));
    // The compact 3-line board stops above the highest possible trajectory.
    let mut hud = Img::new(r.room().cols as i32 * 14, r.room().rows as i32 * 14);
    crate::render::overworld::arena::pastimes::boards(&mut hud, &r);
    for y in 46..190 {
        for x in 40..325 {
            assert_eq!(hud.get(x, y), None, "pot HUD covers launch arc/target");
        }
    }
    // Independently exercise the tallest supported angle/charge through real
    // keys; sprite top including its rim must stay below the HUD bottom y45.
    let mut high = r.clone();
    for _ in 0..25 {
        high.step(&BTreeMap::from([(
            1,
            Input {
                aim_y: -1,
                ..Default::default()
            },
        )]));
    }
    for _ in 0..HZ {
        high.step(&BTreeMap::from([(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]));
    }
    high.step(&BTreeMap::new());
    let mut peak = 0;
    for _ in 0..HZ * 3 {
        if let Some(pot) = high
            .pastimes
            .round
            .as_ref()
            .and_then(|r| r.put.as_ref())
            .and_then(|p| p.flight.as_ref())
        {
            assert!(
                98 - pot.z.min(2053) * 14 / 640 - 8 >= 46,
                "maximum put arc under HUD"
            );
            if pot.z > peak {
                peak = pot.z;
                save("world-pot-highest-arc.png", &native(&high));
            }
        } else {
            break;
        }
        high.step(&BTreeMap::new());
    }
    assert_eq!(peak, 2053, "all supported launch angles fit below HUD");
    for (n, charge) in [16, 20, 30].into_iter().enumerate() {
        for _ in 0..charge {
            r.step(&BTreeMap::from([(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]));
        }
        save(
            if n == 0 {
                "world-pot-charge.png"
            } else if n == 1 {
                "world-pot-second-charge.png"
            } else {
                "world-pot-third-charge.png"
            },
            &native(&r),
        );
        r.step(&BTreeMap::new());
        for _ in 0..8 {
            r.step(&BTreeMap::new());
        }
        assert!(
            r.pastimes
                .round
                .as_ref()
                .unwrap()
                .put
                .as_ref()
                .unwrap()
                .flight
                .is_some()
        );
        save(
            if n == 0 {
                "world-pot-flight.png"
            } else if n == 1 {
                "world-pot-second-flight.png"
            } else {
                "world-pot-third-flight.png"
            },
            &native(&r),
        );
        for _ in 0..3 * HZ {
            r.step(&BTreeMap::new());
        }
    }
    assert_eq!(r.home.countryside.games.wins_for(Game::Pot), 1);
    save("world-pot-finish.png", &native(&r));
}
