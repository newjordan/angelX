fn eastern_world_shots(run: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{
        Input, RoomKind, countryside,
        pastimes::{self, Game},
        world,
    };
    use std::collections::BTreeMap;
    for (stage, name) in [
        (0, "world-picnic-ticket.png"),
        (1, "world-picnic-hamper.png"),
        (2, "world-picnic-return.png"),
        (3, "world-picnic-delivered.png"),
    ] {
        let mut r = run.clone();
        r.home.countryside.picnic = stage;
        r.home.countryside.picnic_paid = stage == 3;
        let (room, at) = countryside::PICNIC[usize::from(stage.min(2))];
        r.arrive(world::room_of(&r.dungeon, room).unwrap(), at);
        assert!(super::eastern::board(&r).is_some());
        let mut layer = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        super::countryside::boards(&mut layer, &r);
        assert!(
            (0..layer.h).any(|y| (0..layer.w).any(|x| layer.get(x, y).is_some())),
            "picnic board must actually render"
        );
        assert!(r.valid_snapshot());
        save(name, &native(&r));
    }
    for ready in [false, true] {
        let mut r = run.clone();
        let plot = &countryside::PLOTS[5];
        r.home.countryside.picnic = if ready { 3 } else { 0 };
        r.home.countryside.picnic_paid = ready;
        r.arrive(world::room_of(&r.dungeon, plot.room).unwrap(), plot.at);
        let mut instructions = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        super::countryside::boards(&mut instructions, &r);
        let before = instructions.clone();
        assert!(
            (0..before.h).any(|y| (0..before.w).any(|x| before.get(x, y).is_some())),
            "post board must actually render"
        );
        super::pastimes::boards(&mut instructions, &r);
        assert!(
            before.pixels().eq(instructions.pixels()),
            "post board suppresses game instructions"
        );
        tray::corner(&mut instructions, &r, None);
        for y in 0..instructions.h {
            for x in 0..instructions.w {
                if before.get(x, y).is_some() {
                    assert_eq!(
                        before.get(x, y),
                        instructions.get(x, y),
                        "post plot text overlaps tray"
                    );
                }
            }
        }
        save(
            if ready {
                "world-post-ready.png"
            } else {
                "world-post-locked.png"
            },
            &native(&r),
        );
    }
    let mut r = run.clone();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Causeway).unwrap(),
        Game::Hobbyhorse.start(),
    );
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (
        Game::Hobbyhorse.start().0 * 2.0,
        Game::Hobbyhorse.start().1 * 2.0,
    );
    r.step(&BTreeMap::new());
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((pastimes::HURDLES[0] - 0.5) * 2.0, 7.0 * 2.0);
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            move_x: 1,
            ..Input::default()
        },
    )]));
    assert!(r.pastimes.round.as_ref().unwrap().riders[&1].air_left > 0);
    assert!(r.valid_snapshot());
    save("world-hobbyhorse-jump.png", &native(&r));
}
