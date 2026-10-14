// Extended by the existing ignored write_world_shots export entry point.
fn aerie_world_shots(run: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{aerie as a, countryside as c, pastimes::Game, world};
    let empty = BTreeMap::new();
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for (kind, name) in [
        (RoomKind::Belfry, "belfry"),
        (RoomKind::Aqueduct, "aqueduct"),
        (RoomKind::Aviary, "parliament"),
    ] {
        let mut r = run.clone();
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 9.5));
        assert!(r.valid_snapshot());
        save(&format!("world-{name}.png"), &native(&r));
    }
    let take = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    for i in 0..3 {
        let mut r = run.clone();
        r.home.countryside.post = 1;
        let (kind, at, _) = a::DOCUMENTS[i];
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
        save(&format!("world-minutes-{}-pigeon.png", i + 1), &native(&r));
        // Bonked: the paper falls, and must lie clear of the HUD.
        let (px, py) = r.bonk_pigeon_for_test(1).expect("a pigeon up");
        let mut paper = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        super::aerie::figures(&mut paper, &r);
        let before = paper.clone();
        tray::corner(&mut paper, &r, None);
        let (cx, cy) = super::at(px, py);
        for y in cy - 8..cy + 4 {
            for x in cx - 6..cx + 6 {
                assert_eq!(paper.get(x, y), before.get(x, y), "dropped paper clear of HUD");
            }
        }
        save(&format!("world-minutes-{}-found.png", i + 1), &native(&r));
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (px, py);
        r.step(&take);
        assert_eq!(r.marks.get(a::QUEST_MARKS[i + 1]), Some(&1));
        save(&format!("world-minutes-{}-taken.png", i + 1), &native(&r));
    }
    for stage in [0u8, 1, 2] {
        let mut r = run.clone();
        r.home.countryside.post = stage;
        r.home.countryside.minutes = if stage == 2 { 7 } else { 0 };
        r.home.countryside.post_paid = stage == 2;
        r.arrive(
            world::room_of(&r.dungeon, RoomKind::Aviary).unwrap(),
            a::CLERK,
        );
        save(
            &format!("world-parliament-speaker-{stage}.png"),
            &native(&r),
        );
    }
    let plot = &c::PLOTS[10];
    for built in [false, true] {
        let mut r = run.clone();
        r.home.countryside.post = 2;
        r.home.countryside.minutes = 7;
        r.home.countryside.post_paid = true;
        if built {
            r.home.countryside.built |= 1 << 10;
        }
        r.arrive(world::room_of(&r.dungeon, plot.room).unwrap(), plot.at);
        assert!(r.valid_snapshot());
        let mut board = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        super::countryside::boards(&mut board, &r);
        let before = board.clone();
        tray::corner(&mut board, &r, None);
        for y in 0..board.h {
            for x in 0..board.w {
                if before.get(x, y).is_some() {
                    assert_eq!(
                        board.get(x, y),
                        before.get(x, y),
                        "build board clear of HUD"
                    );
                }
            }
        }
        save(
            if built {
                "world-airmail-built.png"
            } else {
                "world-airmail-ready.png"
            },
            &native(&r),
        );
        for id in ["aviary-mail", "rookery-mail"] {
            let e = world::ENTRANCES.iter().find(|e| e.id == id).unwrap();
            r.arrive(
                world::room_of(&r.dungeon, e.room).unwrap(),
                world::entrance_centre(e),
            );
            r.descending = if built { 15 } else { 0 };
            assert!(r.valid_snapshot());
            save(
                &format!("world-{id}-{}.png", if built { "open" } else { "closed" }),
                &native(&r),
            );
        }
    }
    let mut r = run.clone();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Belfry).unwrap(),
        Game::Volley.start(),
    );
    if let Some(h) = r.players.get_mut(&2) {
        (h.x, h.y) = (42.0, 22.0);
    }
    r.step(&empty);
    assert!(r.valid_snapshot());
    save("world-belfry-ready.png", &native(&r));
    r.step(&fire);
    for frame in 0..130 {
        r.step(&empty);
        assert!(r.valid_snapshot());
        if [8, 30, 63, 98, 128].contains(&frame) {
            save(&format!("world-belfry-flight-{frame}.png"), &native(&r));
        }
    }
    r.step(&BTreeMap::from([(
        1,
        Input {
            aim_x: -1,
            ..Default::default()
        },
    )]));
    save("world-belfry-paddle.png", &native(&r));
}
