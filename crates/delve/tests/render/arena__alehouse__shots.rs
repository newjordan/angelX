//! The Sozzled Griffin and its bazaar on screen: empty, with the party, a
//! pick, a purchase. `ANGEL_ARENA_SHOTS=<dir> cargo test write_alehouse_shots
//! -- --ignored`.

use super::*;
use crate::rules::{Input, Run, alehouse, bazaar};
use std::collections::BTreeMap;

/// The whole hall at once, the camera's crop taken off: the scenery with
/// every layer over it.
fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    super::alehouse::boards(&mut cv, run, None);
    cv
}

fn seat() -> (i32, i32) {
    (
        NATIVE_W,
        NATIVE_H + super::party::BAND_H + super::party::HUD_H,
    )
}

fn start(seats: u32) -> Run {
    let mut r = Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    for id in 3..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.players.retain(|&id, _| id <= seats.max(1));
    r.walk_alehouse();
    r.floor_plates.clear();
    r.step(&BTreeMap::new());
    r
}

fn place(r: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = r.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * 2.0, y * 2.0);
}

/// Past the blink of arriving, so every knight is drawn.
fn settle(r: &mut Run) {
    for _ in 0..100 {
        r.step(&BTreeMap::new());
    }
}

fn press(r: &mut Run, id: u32) {
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(
        id,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    r.step(&BTreeMap::new());
}

fn hold(r: &mut Run, id: u32) {
    r.step(&BTreeMap::new());
    for _ in 0..crate::rules::home::BUY_HOLD + 3 {
        r.step(&BTreeMap::from([(
            id,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
}

#[test]
#[ignore]
fn write_alehouse_shots() {
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
    let seat_shot = |name: &str, run: &Run, focus: u32| {
        let img = seat_scene(run, w, h, Some(focus)).compose();
        save_img(name, &img, 3);
    };
    // The hall with nobody in it: the one knight waits out at the door.
    let mut run = start(1);
    run.players.clear();
    save_img("tavern-01-empty", &whole(&run), 2);
    // The party, together, as the delve begins.
    let mut run = start(4);
    settle(&mut run);
    save_img("tavern-02-party-hall", &whole(&run), 2);
    seat_shot("tavern-03-party-seat", &run, 1);
    // A knight before a patron: the pitch; then picked.
    let mut run = start(2);
    settle(&mut run);
    run.alehouse.company_page = 1;
    let mage = alehouse::PATRONS
        .iter()
        .find(|p| p.kit == "scryglass")
        .unwrap()
        .at;
    place(&mut run, 1, (mage.0, mage.1 - 1.0));
    run.step(&BTreeMap::new());
    seat_shot("tavern-04-pitch", &run, 1);
    press(&mut run, 1);
    seat_shot("tavern-05-picked", &run, 1);
    save_img("tavern-06-picked-hall", &whole(&run), 2);
    // A second knight is turned away from the same table.
    place(&mut run, 2, (mage.0 + 0.4, mage.1 - 1.0));
    press(&mut run, 2);
    seat_shot("tavern-07-taken", &run, 2);
    // The keeper's taps, the stew, the cat, the fence.
    let mut run = start(1);
    settle(&mut run);
    place(&mut run, 1, alehouse::TAPS[1]);
    run.step(&BTreeMap::new());
    seat_shot("tavern-08-bar", &run, 1);
    place(&mut run, 1, alehouse::TOYS[0].1);
    press(&mut run, 1);
    seat_shot("tavern-09-stew", &run, 1);
    place(
        &mut run,
        1,
        (alehouse::FENCE_AT.0 + 1.4, alehouse::FENCE_AT.1),
    );
    run.step(&BTreeMap::new());
    seat_shot("tavern-10-fence", &run, 1);
    // The bazaar: empty, with the party, a ware, a purchase.
    let mut run = start(1);
    run.players.clear();
    let to = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == crate::rules::RoomKind::Bazaar)
        .unwrap();
    run.arrive(to, (16.0, 7.0));
    run.players.clear();
    save_img("bazaar-01-empty", &whole(&run), 2);
    let mut run = start(4);
    run.arrive(to, bazaar::ARRIVAL);
    settle(&mut run);
    seat_shot("bazaar-02-party", &run, 1);
    let at = bazaar::stand_of(1).unwrap();
    place(&mut run, 1, at);
    run.step(&BTreeMap::new());
    seat_shot("bazaar-03-ware", &run, 1);
    hold(&mut run, 1);
    seat_shot("bazaar-04-bought", &run, 1);
    save_img("bazaar-05-bought-hall", &whole(&run), 2);
    // Out in the Delve's courtyard: the green house is the tavern, and the
    // peddler's plates (on a run whose cart stands there) the bazaar.
    let gate_cart = (41..80)
        .map(|seed| {
            let mut r = Run::at_home(
                seed,
                1,
                Some("Friend"),
                Default::default(),
                Default::default(),
            );
            r.walk_alehouse();
            r
        })
        .find(|r| {
            r.merchant_camp()
                .is_some_and(|c| r.dungeon.rooms[c.room].kind == crate::rules::RoomKind::Gate)
        })
        .unwrap();
    let mut run = gate_cart;
    let gate = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == crate::rules::RoomKind::Gate)
        .unwrap();
    run.arrive(gate, crate::rules::world::COURTYARD_ARRIVAL);
    run.floor_plates.clear();
    settle(&mut run);
    save_img("courtyard-01-hall", &whole(&run), 2);
    seat_shot("courtyard-02-seat", &run, 1);
    // Come out of the tavern's front: before its door, its name on approach.
    place(
        &mut run,
        1,
        (
            alehouse::COURTYARD_FRONT.0,
            alehouse::COURTYARD_FRONT.1 + 0.4,
        ),
    );
    run.step(&BTreeMap::new());
    seat_shot("courtyard-03-door", &run, 1);
}

/// Explicit visual review only; no PNGs are written by ordinary test runs.
#[test]
#[ignore]
fn write_alehouse_home_lamp_shots() {
    let dir = std::env::var_os("ANGEL_ARENA_SHOTS")
        .map(std::path::PathBuf::from)
        .expect("set ANGEL_ARENA_SHOTS to the output directory");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: Img| {
        image::RgbaImage::from_raw(img.w as u32 * 3, img.h as u32 * 3, img.rgba_scaled(3))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let mut r = start(1);
    settle(&mut r);
    place(&mut r, 1, alehouse::LAMP_STAND);
    let (w, h) = seat();
    save(
        "home-lamp-unclaimed",
        seat_scene(&r, w, h, Some(1)).compose(),
    );
    for _ in 0..crate::rules::home::BUY_HOLD {
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    save("home-lamp-claimed", seat_scene(&r, w, h, Some(1)).compose());
    save("home-lamp-hall", whole(&r));
}

/// The complete rack and its per-arm boards, plus equipped and mid-hold states.
#[test]
#[ignore]
fn write_alehouse_home_arms_shots() {
    let dir = std::env::var_os("ANGEL_ARENA_SHOTS")
        .map(std::path::PathBuf::from)
        .expect("set ANGEL_ARENA_SHOTS to the output directory");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: Img| {
        image::RgbaImage::from_raw(img.w as u32 * 3, img.h as u32 * 3, img.rgba_scaled(3))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let mut r = start(1);
    settle(&mut r);
    let (w, h) = seat();
    for (slot, card) in alehouse::home_arms().iter().enumerate() {
        place(&mut r, 1, alehouse::arm_stand(slot));
        save(
            &format!("home-arms-{}", card.id),
            seat_scene(&r, w, h, Some(1)).compose(),
        );
    }
    let slot = alehouse::home_arms().iter().position(|c| c.id == "stock-maul").unwrap();
    place(&mut r, 1, alehouse::arm_stand(slot));
    for _ in 0..crate::rules::home::BUY_HOLD / 2 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    save("home-arms-hold", seat_scene(&r, w, h, Some(1)).compose());
    hold(&mut r, 1);
    save(
        "home-arms-equipped",
        seat_scene(&r, w, h, Some(1)).compose(),
    );
    save("home-arms-hall", whole(&r));
}

#[test]
#[ignore]
fn write_bathroom_mirror_shots() {
    let dir = std::path::PathBuf::from(std::env::var("ANGEL_ARENA_SHOTS").expect("shot directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        for k in [1, 3] {
            image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
                .unwrap()
                .save(dir.join(format!("{name}-{k}x.png")))
                .unwrap();
        }
    };
    let mut run = start(3);
    settle(&mut run);
    place(&mut run, 1, alehouse::MIRROR_STAND);
    let (w, h) = seat();
    save("01-bathroom", &seat_scene(&run, w, h, Some(1)).compose());
    run.players.get_mut(&1).unwrap().mirror.open = true;
    {
        let h = run.players.get_mut(&1).unwrap();
        h.look.custom = true;
        h.look.hair = 2;
        h.look.primary = 4;
        h.look.name = "Rowan".into();
    }
    save("02-creator", &seat_scene(&run, w, h, Some(1)).compose());
    let path = dir.parent().unwrap().join("browser-snapshot.json");
    run.players.get_mut(&1).unwrap().mirror.open = false;
    place(&mut run, 2, alehouse::MIRROR_STAND);
    {
        let h = run.players.get_mut(&2).unwrap();
        h.mirror.open = true;
        h.mirror.row = 4;
        h.look.custom = true;
        h.look.skin = 3;
        h.look.hair = 4;
        h.look.beard = 2;
        h.look.primary = 1;
    }
    std::fs::write(path, serde_json::to_vec(&run).unwrap()).unwrap();
    save("04-guest-view", &seat_scene(&run, w, h, Some(2)).compose());
    for id in 1..=3 {
        place(
            &mut run,
            id,
            (
                9.5 + id as f32 * 2.5,
                12.0 + if id == 2 { 0.7 } else { 0.0 },
            ),
        );
        let h = run.players.get_mut(&id).unwrap();
        h.mirror.open = false;
        h.look.custom = true;
        h.look.name = ["", "Rowan", "Bram", "Nell"][id as usize].into();
        h.look.style = 0;
        h.look.hair = [0, 2, 4, 3][id as usize];
        h.look.skin = (id - 1) as u8 * 2;
        h.look.beard = (id - 1) as u8;
        h.look.primary = [0, 0, 1, 2][id as usize];
        h.look.trim = [0, 3, 5, 7][id as usize];
        h.look.stature = [0, 2, 1, 0][id as usize];
        h.look.body = (id - 1) as u8;
    }
    save(
        "03-three-knights",
        &seat_scene(&run, w, h, Some(2)).compose(),
    );
    save("05-whole-tavern", &whole(&run));
}
