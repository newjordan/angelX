// Bounded offline extension of the existing write_world_shots exporter.
fn woodland_world_shots(base: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{
        HZ, Input, RoomKind,
        pastimes::{self, Game},
        woodland as wood, world,
    };
    use crate::render::overworld::arena::{self, pastimes as arcade, woodland as paint};
    use std::collections::BTreeMap;
    for (kind, name) in [
        (RoomKind::Timberyard, "world-timberyard.png"),
        (RoomKind::HedgeMaze, "world-hedge.png"),
        (RoomKind::Bothy, "world-bothy.png"),
    ] {
        let mut r = base.clone();
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), (12.0, 9.5));
        let scenery = crate::render::overworld::arena::scenery(&r);
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
    // The Boot kicks the first try, so it is pressed twice.
    for (n, s) in [0, 1, 1, 2, 3, 4].into_iter().enumerate() {
        let (kind, at) = wood::ERRAND[s];
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
                    "soup HUD covers ring at {s}"
                );
            }
        }
        save(&format!("world-soup-{n}.png"), &native(&quest));
        quest.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    assert_eq!(wood::stage(&quest.home.countryside, &quest.marks), 5);
    quest.home.countryside.soup = 5;
    quest.home.countryside.soup_paid = true;
    quest.home.countryside.built |= 1 << 8;
    save("world-bothy-built.png", &native(&quest));
    for kind in [RoomKind::Bothy, RoomKind::Orchard] {
        quest.arrive(world::room_of(&quest.dungeon, kind).unwrap(), (12.0, 9.5));
        save(
            if kind == RoomKind::Bothy {
                "world-bothy-wagon.png"
            } else {
                "world-orchard-wagon.png"
            },
            &native(&quest),
        );
    }
    let mut r = base.clone();
    r.arrive(
        world::room_of(&r.dungeon, RoomKind::Timberyard).unwrap(),
        Game::Cart.start(),
    );
    r.step(&BTreeMap::new());
    save("world-cart-start.png", &native(&r));
    let mut held = false;
    let mut crossed = 0;
    for _ in 0..40 * HZ {
        let Some(round) = r.pastimes.round.as_ref() else {
            break;
        };
        let c = round.cart.as_ref().unwrap();
        let y_px = c.y * arena::TILE / 256;
        let lift = (c.air_left.min(pastimes::CART_AIR - c.air_left) * 3) as i32;
        assert!(y_px - lift - 15 > 45, "airborne cart clears top ledger");
        let mut plain = native(&r);
        let before = plain.clone();
        arcade::boards(&mut plain, &r);
        for &log in &pastimes::CART_LOGS {
            for dy in -2..=2 {
                for dx in -2..=2 {
                    assert_eq!(
                        before.get(log * arena::TILE + dx, 7 * arena::TILE + dy),
                        plain.get(log * arena::TILE + dx, 7 * arena::TILE + dy),
                        "cart HUD covers log"
                    );
                }
            }
        }
        if c.air_left == pastimes::CART_AIR / 2 {
            save(&format!("world-cart-air-{}.png", round.progress), &plain);
        }
        let fire = pastimes::CART_LOGS[usize::from(round.progress)] * 256 - c.x <= 5 * 36
            && c.air_left == 0
            && c.rest_left == 0
            && !held;
        r.step(&BTreeMap::from([(
            1,
            Input {
                aim_x: 1,
                fire,
                ..Default::default()
            },
        )]));
        held = fire;
        assert!(r.valid_snapshot());
        let n = r.pastimes.round.as_ref().map_or(4, |round| round.progress);
        if n > crossed {
            save(&format!("world-cart-log-{n}.png"), &native(&r));
            crossed = n;
        }
    }
    assert_eq!(r.home.countryside.games.cart.wins, 1);
    assert_eq!(crossed, 4);
    save("world-cart-win.png", &native(&r));
    // Highest allowed jump from the lane's northern edge still clears the HUD.
    for h in r.players.values_mut() {
        h.x += 4.0;
    }
    r.step(&BTreeMap::new());
    for h in r.players.values_mut() {
        h.x -= 4.0;
    }
    r.step(&BTreeMap::new());
    {
        let c = r.pastimes.round.as_mut().unwrap().cart.as_mut().unwrap();
        c.y = 5 * 256;
        c.air_left = pastimes::CART_AIR / 2;
    }
    assert!(r.valid_snapshot());
    save("world-cart-highest-jump.png", &native(&r));
}
