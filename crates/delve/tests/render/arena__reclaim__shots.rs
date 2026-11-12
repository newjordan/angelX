//! King Brannoc's lost halls on screen: each hall cold and played out, the
//! talk with the King and with a keeper, the King's Hall lamps.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_reclaim_shots -- --ignored`.

use super::*;
use crate::rules::barony::{self, Court};
use crate::rules::reclaim::{self, Won, rooms};
use crate::rules::{Input, Pack, Run, RoomKind, home::Home, world};
use std::collections::BTreeMap;

fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    cv
}

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::party::BAND_H + super::party::HUD_H)
}

fn met() -> Home {
    let mut home = Home::default();
    home.barony.court = Court::Met;
    home
}

fn won(home: &mut Home, ids: &[(&str, &str)]) {
    for &(id, how) in ids {
        home.barony.halls.won.push(Won { id: id.into(), how: how.into() });
    }
}

fn press() -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { interact: true, ..Input::default() })])
}

fn answer(pick: u8) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { cast: pick, ..Input::default() })])
}

fn tap(run: &mut Run) {
    run.step(&press());
    run.step(&BTreeMap::new());
}

fn stand_at(run: &mut Run, at: (f32, f32)) {
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (at.0 * 2.0, at.1 * 2.0);
}

/// The party in a delve's hall of `depth` floors down from the Mines.
fn in_hall(home: Home, floors: usize, treasury: crate::realm::Spoils) -> Run {
    let mut run = Run::at_home(7, 1, None, home, treasury);
    run.dungeon.pack = Pack::Cavern;
    for _ in 0..floors {
        run.descend_for_test();
    }
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::DwarfHall).expect("a hall");
    run.enter_for_test(i);
    run.floor_plates.clear();
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    run
}

#[test]
#[ignore]
fn write_reclaim_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save_img = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let seat_shot = |name: &str, run: &Run| {
        let img = seat_scene(run, w, h, Some(1)).compose();
        save_img(name, &img, 2);
    };
    let toll = {
        let mut s = crate::realm::Spoils::default();
        s.add(crate::realm::Spoil::Gold, 900);
        s
    };

    // The King at his camp: his talk, and the ask.
    let mut run = Run::at_home(7, 1, None, met(), Default::default());
    let i = world::room_of(&run.dungeon, RoomKind::MineHead).unwrap();
    run.arrive(i, (12.0, 11.0));
    run.floor_plates.clear();
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    let (kx, ky) = barony::CAMP_KING;
    stand_at(&mut run, (kx + 1.2, ky + 0.8));
    tap(&mut run);
    seat_shot("king-1-greeting", &run);
    tap(&mut run);
    seat_shot("king-2-ask", &run);
    run.step(&answer(2));
    seat_shot("king-3-lore", &run);

    // The King's Hall: the lamps, none lit, then two, then all three.
    for (name, held) in [("kings-hall-0", 0usize), ("kings-hall-2", 2), ("kings-hall-3", 3)] {
        let mut home = met();
        home.barony.court = Court::Sworn;
        let ids = [("gallery", "fight"), ("lamps", "rite"), ("anvils", "freed")];
        won(&mut home, &ids[..held]);
        let mut run = Run::at_home(7, 1, None, home, Default::default());
        let i = world::room_of(&run.dungeon, RoomKind::KingsHall).unwrap();
        run.arrive(i, (12.0, 8.0));
        run.floor_plates.clear();
        for _ in 0..60 {
            run.step(&BTreeMap::new());
        }
        let (c, r, pw, _) = super::reclaim::LAMP_PLATE;
        stand_at(&mut run, (c as f32 + pw as f32 / 2.0, r as f32 + 0.5));
        run.step(&BTreeMap::new());
        let mut cv = whole(&run);
        super::barony::boards(&mut cv, &run);
        super::reclaim::hall_boards(&mut cv, &run);
        save_img(name, &cv, 2);
        if held == 3 {
            // The King on his seat, in his talk, with all three halls his.
            let (sx, sy) = barony::SEAT_AT;
            stand_at(&mut run, (sx + 1.0, sy + 1.2));
            tap(&mut run);
            tap(&mut run);
            seat_shot("kings-hall-3-talk", &run);
        }
    }

    // The Delvers' Gallery: cold with Nubbin, his talk, the fight, won each way.
    let mut home = met();
    home.barony.halls.asked = Some(0);
    let mut run = in_hall(home.clone(), 1, toll.clone());
    save_img("gallery-1-cold", &whole(&run), 2);
    let at = rooms::KEEPER_AT[0];
    stand_at(&mut run, (at.0, at.1 + 1.6));
    tap(&mut run);
    tap(&mut run);
    seat_shot("gallery-2-nubbin-asks", &run);
    run.step(&answer(2));
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    save_img("gallery-3-the-fight", &whole(&run), 2);
    let mut fought = home.clone();
    won(&mut fought, &[("gallery", "fight")]);
    let run = in_hall(fought, 1, Default::default());
    save_img("gallery-4-won-by-fight", &whole(&run), 2);
    let mut bargained = home.clone();
    won(&mut bargained, &[("gallery", "bargain")]);
    let run = in_hall(bargained, 1, Default::default());
    save_img("gallery-5-won-by-toll", &whole(&run), 2);

    // The Hall of Lamps: cold, the rite half done, at rest.
    let mut home = met();
    won(&mut home, &[("gallery", "fight")]);
    home.barony.halls.asked = Some(1);
    let mut run = in_hall(home.clone(), 2, Default::default());
    let at = rooms::KEEPER_AT[1];
    stand_at(&mut run, (at.0, at.1 + 1.8));
    run.step(&BTreeMap::new());
    save_img("lamps-1-cold", &whole(&run), 2);
    tap(&mut run);
    tap(&mut run);
    seat_shot("lamps-2-madoc-asks", &run);
    tap(&mut run);
    let order = rooms::rite_order(run.seed);
    for &lamp in order.iter().take(2) {
        stand_at(&mut run, rooms::LAMPS[lamp]);
        tap(&mut run);
    }
    let (px_, py_) = rooms::LAMPS[order[1]];
    stand_at(&mut run, (px_, py_ + 2.2));
    run.step(&BTreeMap::new());
    let mut cv = whole(&run);
    super::reclaim::boards(&mut cv, &run);
    save_img("lamps-3-two-burn", &cv, 2);
    let mut rested = home.clone();
    won(&mut rested, &[("lamps", "rite")]);
    rested.barony.halls.asked = None;
    let run = in_hall(rested, 2, Default::default());
    save_img("lamps-4-at-rest", &whole(&run), 2);

    // The Anvil Gallery: buried, one heap down, freed.
    let mut home = met();
    won(&mut home, &[("gallery", "fight"), ("lamps", "rite")]);
    home.barony.halls.asked = Some(2);
    let mut run = in_hall(home.clone(), 3, Default::default());
    let at = rooms::KEEPER_AT[2];
    stand_at(&mut run, (at.0, at.1 + 1.8));
    run.step(&BTreeMap::new());
    save_img("anvils-1-buried", &whole(&run), 2);
    tap(&mut run);
    tap(&mut run);
    seat_shot("anvils-2-olwen-asks", &run);
    tap(&mut run);
    let (c, r, dw, dh) = rooms::DIGS[0];
    stand_at(&mut run, (c as f32 + dw as f32 / 2.0, r as f32 + dh as f32 / 2.0));
    for _ in 0..reclaim_dig() + 2 {
        run.step(&press());
    }
    run.step(&BTreeMap::new());
    let (c, r, dw, dh) = rooms::DIGS[1];
    stand_at(&mut run, (c as f32 + dw as f32 / 2.0, r as f32 + dh as f32 / 2.0));
    for _ in 0..reclaim_dig() / 2 {
        run.step(&press());
    }
    let mut cv = whole(&run);
    super::reclaim::boards(&mut cv, &run);
    save_img("anvils-3-digging", &cv, 2);
    let mut freed = home.clone();
    won(&mut freed, &[("anvils", "freed")]);
    freed.barony.halls.asked = None;
    let run = in_hall(freed, 3, Default::default());
    save_img("anvils-4-freed", &whole(&run), 2);
    let _ = reclaim::HALLS.len();
}

fn reclaim_dig() -> u32 {
    crate::rules::reclaim::play::DIG_HOLD
}

#[test]
fn every_hall_draws_in_every_state_and_every_talk_shows() {
    let _pin = super::scenery::pin_scene_cache();
    let toll = {
        let mut s = crate::realm::Spoils::default();
        s.add(crate::realm::Spoil::Gold, 900);
        s
    };
    let ids = [("gallery", "fight"), ("lamps", "rite"), ("anvils", "freed")];
    for floors in 1..=3usize {
        // Asked and cold, played out and held: each has its look.
        for held in [false, true] {
            let mut home = met();
            won(&mut home, &ids[..floors - 1]);
            if held {
                won(&mut home, &ids[floors - 1..floors]);
            } else {
                home.barony.halls.asked = Some(floors as u8 - 1);
            }
            let mut run = in_hall(home, floors, toll.clone());
            let at = rooms::KEEPER_AT[floors - 1];
            stand_at(&mut run, (at.0, at.1 + 1.6));
            run.step(&BTreeMap::new());
            let cold = whole(&run);
            assert!(cold.w > 0, "hall {floors} held {held}");
            tap(&mut run);
            let (w, h) = seat();
            let shot = seat_scene(&run, w, h, Some(1)).compose();
            assert!(run.reclaim_view(1).is_some(), "hall {floors} held {held}: a keeper to talk to");
            // The talk is on the seat's screen: the picture differs with it open.
            let mut away = run.clone();
            away.reclaim.talks.clear();
            let plain = seat_scene(&away, w, h, Some(1)).compose();
            assert_ne!(shot.rgba_scaled(1), plain.rgba_scaled(1), "hall {floors} held {held}");
        }
    }
}

#[test]
fn the_kings_talk_shows_on_the_seat_and_the_lamps_burn_for_each_hall_held() {
    let _pin = super::scenery::pin_scene_cache();
    let lit = |held: usize| {
        let mut home = met();
        home.barony.court = Court::Sworn;
        won(&mut home, &[("gallery", "fight"), ("lamps", "rite"), ("anvils", "freed")][..held]);
        let mut run = Run::at_home(7, 1, None, home, Default::default());
        let i = world::room_of(&run.dungeon, RoomKind::KingsHall).unwrap();
        run.arrive(i, (12.0, 8.0));
        run.floor_plates.clear();
        for _ in 0..30 {
            run.step(&BTreeMap::new());
        }
        let cv = whole(&run);
        // Count the flames: warm fire inks along the hall's north wall.
        let mut flames = 0;
        for x in 0..cv.w {
            for y in 0..14 {
                if cv.get(x, y).is_some_and(|c| c[0] > 200 && c[1] > 150 && c[2] < 120) {
                    flames += 1;
                }
            }
        }
        flames
    };
    let (none, two, three) = (lit(0), lit(2), lit(3));
    assert!(two > none && three > two, "{none} {two} {three}");
}
