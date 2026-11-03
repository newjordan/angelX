// Extend the existing offline write_world_shots gate, never open a host.
fn wetland_world_shots(base: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{
        HZ, Input, RoomKind,
        pastimes::{self, Game},
        wetland as wet, world,
    };
    use crate::render::overworld::arena::{self, pastimes as arcade, wetland as paint};
    use std::collections::BTreeMap;
    for (kind, name) in [
        (RoomKind::Millpond, "world-millpond.png"),
        (RoomKind::Sluice, "world-sluice.png"),
        (RoomKind::SunkenAbbey, "world-abbey.png"),
    ] {
        let mut r = base.clone();
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 9.5));
        let scenery = arena::scenery(&r);
        assert!(
            scenery
                .rgba_scaled(1)
                .chunks_exact(4)
                .filter(|p| p[3] > 0)
                .count()
                > 10000
        );
        save(name, &native(&r));
    }
    let mut quest = base.clone();
    for (s, &(kind, at)) in wet::ERRAND.iter().enumerate() {
        quest.arrive(world::room_of(&quest.dungeon, kind).unwrap(), at);
        assert!(paint::board_visible(&quest));
        let mut plain = Img::new(
            quest.room().cols as i32 * arena::TILE,
            quest.room().rows as i32 * arena::TILE,
        );
        paint::boards(&mut plain, &quest);
        let (x, y) = arena::at(at.0 * 2.0, at.1 * 2.0);
        for dy in -26..=26 {
            for dx in -26..=26 {
                assert!(
                    plain.get(x + dx, y + dy).is_none(),
                    "sluice ledger covers ring {s}"
                );
            }
        }
        save(&format!("world-sluice-quest-{s}.png"), &native(&quest));
        quest.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    assert_eq!(wet::stage(&quest.home.countryside, &quest.marks), 6);
    quest.home.countryside.sluice = 6;
    quest.home.countryside.sluice_paid = true;
    quest.home.countryside.built |= 1 << 9;
    let p = &crate::rules::countryside::PLOTS[9];
    quest.arrive(world::room_of(&quest.dungeon, p.room).unwrap(), p.at);
    let mut instructions = Img::new(
        quest.room().cols as i32 * arena::TILE,
        quest.room().rows as i32 * arena::TILE,
    );
    arena::countryside::boards(&mut instructions, &quest);
    let before_tray = instructions.clone();
    arena::tray::corner(&mut instructions, &quest, None);
    for y in 0..instructions.h {
        for x in 0..instructions.w {
            if before_tray.get(x, y).is_some() {
                assert_eq!(
                    instructions.get(x, y),
                    before_tray.get(x, y),
                    "tray covers punt instructions"
                );
            }
        }
    }
    save("world-abbey-built.png", &native(&quest));
    quest.arrive(
        world::room_of(&quest.dungeon, RoomKind::Orchard).unwrap(),
        (19.5, 6.5),
    );
    save("world-orchard-punt.png", &native(&quest));
    let mut r = base.clone();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Millpond).unwrap(),
        Game::Surf.start(),
    );
    r.step(&BTreeMap::new());
    save("world-surf-start.png", &native(&r));
    let mut held = false;
    let mut wave = 0;
    for _ in 0..20 * HZ {
        let Some(round) = r.pastimes.round.as_ref() else {
            break;
        };
        let surf = round.surf.as_ref().unwrap();
        let mut plain = Img::new(
            r.room().cols as i32 * arena::TILE,
            r.room().rows as i32 * arena::TILE,
        );
        arcade::boards(&mut plain, &r);
        for (x, y) in [
            (8 * arena::TILE, 6 * arena::TILE),
            (12 * arena::TILE, 10 * arena::TILE),
        ] {
            for dy in -25..=18 {
                for dx in -25..=25 {
                    assert!(
                        plain.get(x + dx, y + dy).is_none(),
                        "surf ledger hides piece/gauge"
                    );
                }
            }
        }
        if surf.phase == pastimes::WAVE_TICKS - pastimes::CREST_WINDOW {
            save(
                &format!("world-surf-crest-{}.png", round.progress),
                &native(&r),
            );
        }
        let target = pastimes::WAVE_LEANS[usize::from(round.progress)];
        let fire = surf.phase + 1 >= pastimes::WAVE_TICKS - pastimes::CREST_WINDOW && !held;
        r.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: (target - surf.lean).signum() as i8,
                fire,
                ..Default::default()
            },
        )]));
        held = fire;
        assert!(r.valid_snapshot());
        if let Some(next) = r.pastimes.round.as_ref() {
            if next.progress != wave {
                wave = next.progress;
                save(&format!("world-surf-wave-{wave}.png"), &native(&r));
            }
        }
    }
    assert_eq!(r.home.countryside.games.surf.wins, 1);
    save("world-surf-win.png", &native(&r));
}
