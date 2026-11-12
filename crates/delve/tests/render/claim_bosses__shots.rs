use super::*;

#[test]
fn claim_boss_art_is_stateful_and_has_visible_transparent_silhouettes() {
    for kind in Guardian::ALL {
        let (run, _) = crate::rules::claim_bosses::tests::start(kind);
        let boss = kind.definition();
        let mut enemy = run.enemies[0].clone();
        let rest = figure(&run, &enemy, &boss, 10).unwrap();
        enemy.stage = fight::WIND;
        enemy.timer = boss.windups[0].wind - 1;
        let wind = figure(&run, &enemy, &boss, 10).unwrap();
        assert_ne!(rest, wind);
        let count = (0..rest.h)
            .flat_map(|y| (0..rest.w).map(move |x| (x, y)))
            .filter(|&(x, y)| rest.get(x, y).is_some())
            .count();
        assert!(count > 500 && count < (rest.w * rest.h) as usize);
    }
}

#[test]
#[ignore = "writes real resource guardian fights using the shared arena renderer"]
fn write_claim_boss_review() {
    let dest =
        std::env::var("ANGEL_CLAIM_SHOTS").unwrap_or_else(|_| "/tmp/claim-boss-review".into());
    std::fs::create_dir_all(&dest).unwrap();
    for kind in Guardian::ALL {
        let (mut run, _) = crate::rules::claim_bosses::tests::start(kind);
        let mut saved = 0;
        for tick in 0..600 {
            let hero = run.players.get_mut(&1).unwrap();
            hero.hp = hero.max_hp;
            hero.invulnerable = 0;
            run.step(&std::collections::BTreeMap::new());
            let enemy = &run.enemies[0];
            let take = match saved {
                0 => tick == 0,
                1 => enemy.stage == fight::WIND && enemy.timer > 10,
                2 => enemy.stage == fight::DAZED,
                _ => false,
            };
            if take {
                let im = super::super::frame_for(&run, 768, 448, Some(1));
                image::save_buffer(
                    format!("{dest}/{}-{saved}.png", kind.id()),
                    &im.rgba_bytes(),
                    im.w as u32,
                    im.h as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                saved += 1;
                if saved == 3 {
                    break;
                }
            }
        }
        assert_eq!(saved, 3, "{kind:?}: arrival, warning and recovery rendered");
        // The death frame comes from real attacks with an unmodified starter
        // knight, without health resets or assigning a boss corpse.
        let (mut won, deposit, _, _) = crate::rules::claim_bosses::tests::play_claim(kind);
        assert!(won.claim_guardian_defeated(&deposit.id));
        for _ in 0..12 {
            won.step(&std::collections::BTreeMap::new());
        }
        let im = super::super::frame_for(&won, 768, 448, Some(1));
        image::save_buffer(
            format!("{dest}/{}-3-victory.png", kind.id()),
            &im.rgba_bytes(),
            im.w as u32,
            im.h as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
