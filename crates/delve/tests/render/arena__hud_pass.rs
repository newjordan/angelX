//! The play screen's HUD, as a seat sees it at native size: the party's
//! band with six seats, the skills bar, and the prompts in a fight, the
//! kingdom and the taverns (`ANGEL_ARENA_SHOTS=<dir> cargo test -p
//! angel-delve write_hud_pass_shots -- --ignored`).
use super::*;

/// `run` with every seat filled, as the dev host's bots fill them.
fn six(mut run: Run) -> Run {
    for id in 2..=6 {
        run.join_seat(id, &format!("Bot {id}"));
    }
    if let Some(h) = run.players.get_mut(&1) {
        h.name = "Bot 1".into();
    }
    run
}

#[test]
#[ignore = "writes the HUD review PNGs"]
fn write_hud_pass_shots() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("shot directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let h = NATIVE_H + party::BAND_H + party::HUD_H;
    let save = |name: &str, run: &Run| {
        let img = seat_frame(run, NATIVE_W, h, Some(1));
        for k in [1u32, 3] {
            image::save_buffer(dir.join(format!("{name}-{k}x.png")), &img.rgba_scaled(k),
                img.w as u32 * k, img.h as u32 * k, image::ColorType::Rgba8).unwrap();
        }
    };
    save("fight-1", &testing::fight());
    save("fight-6", &six(testing::fight()));
    save("boss-6", &six(testing::boss("cinderjaw")));
    save("kingdom", &testing::kingdom());
    save("kingdom-6", &six(testing::kingdom()));
    // The taverns, the knight at each place that asks for a key (tiles).
    let spots = |mut run: Run, at: &[(f32, f32)], tag: &str| {
        run.floor_plates.clear();
        for _ in 0..100 {
            run.step(&std::collections::BTreeMap::new());
        }
        for (i, &(x, y)) in at.iter().enumerate() {
            let mut run = run.clone();
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (x * 2.0, y * 2.0);
            run.step(&std::collections::BTreeMap::new());
            save(&format!("{tag}-{i}"), &run);
        }
    };
    let mut home = crate::rules::home::Home { deepest: 2, ..Default::default() };
    home.levels.insert(crate::rules::home::Station::Wing, 1);
    home.residents.insert("maud".into());
    let mut tavern = Run::at_home(5, 1, Some("Bot 1"), home, Default::default());
    let at = tavern.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Tavern).unwrap();
    tavern.enter_for_test(at);
    spots(tavern, &[(9.0, 8.0), (5.0, 5.4), (9.0, 5.4), (18.5, 3.6), (3.0, 12.6)], "tavern");
    let mut ale = Run::at_home(41, 1, Some("Bot 1"), Default::default(), Default::default());
    ale.walk_alehouse();
    spots(ale, &[(20.5, 6.0), (7.0, 4.6), (31.5, 10.4), (31.5, 15.6), (18.0, 17.0)], "alehouse");
}
