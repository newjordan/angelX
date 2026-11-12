//! The hall's talk, the stoked hearth, a place's name on arrival and the
//! courtyard's watching dragon, on screen. `ANGEL_ARENA_SHOTS=<dir> cargo
//! test write_hall_voices_shots -- --ignored` writes the pictures; the
//! plain tests hold that each is drawn and goes when its moment is up.

use super::*;
use crate::rules::hall_voices::{CALL_TICKS, TITLE_TICKS, Voice};
use crate::rules::{Input, Run, alehouse, bazaar, world};
use std::collections::BTreeMap;

fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    super::alehouse::boards(&mut cv, run, None);
    cv
}

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::party::BAND_H + super::party::HUD_H)
}

fn start(seats: u32) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
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

fn run_for(r: &mut Run, ticks: u64) {
    for _ in 0..ticks {
        r.step(&BTreeMap::new());
    }
}

fn press(r: &mut Run, id: u32) {
    r.step(&BTreeMap::new());
    r.step(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
    r.step(&BTreeMap::new());
}

fn hold(r: &mut Run, id: u32) {
    r.step(&BTreeMap::new());
    for _ in 0..crate::rules::home::BUY_HOLD + 3 {
        r.step(&BTreeMap::from([(id, Input { interact: true, ..Default::default() })]));
    }
}

fn seat_img(run: &Run, focus: u32) -> Img {
    let (w, h) = seat();
    seat_scene(run, w, h, Some(focus)).compose()
}

fn courtyard(seats: u32) -> Run {
    let mut run = start(seats);
    let gate = world::room_of(&run.dungeon, crate::rules::RoomKind::Gate).unwrap();
    run.arrive(gate, world::COURTYARD_ARRIVAL);
    run.floor_plates.clear();
    run.step(&BTreeMap::new());
    run
}

/// The inn's front in the courtyard's picture (top-left, pixels).
fn inn_at() -> (i32, i32) {
    let (dc, dr, dw, dh) = alehouse::COURTYARD_DOOR;
    (dc * TILE + dw * TILE / 2 - 48, (dr + dh) * TILE - 80)
}

fn crop(img: &Img, x: i32, y: i32, w: i32, h: i32) -> Img {
    Img::from_view(img, x, y, w, h)
}

fn same(a: &Img, b: &Img) -> bool {
    a.w == b.w && a.h == b.h && (0..a.h).all(|y| (0..a.w).all(|x| a.get(x, y) == b.get(x, y)))
}

#[test]
fn a_keepers_line_is_drawn_and_goes_when_its_moment_is_up() {
    let mut run = start(1);
    run.alehouse.greeted = u32::MAX;
    run_for(&mut run, 30);
    let bare = whole(&run);
    run.call(Voice::Keeper, "EVENING, FRIEND. WHAT'LL IT BE?");
    // Ten ticks in: up on its plate.
    run.alehouse.calls[0].at -= 10;
    let spoken = whole(&run);
    assert!(!same(&bare, &spoken), "a plate by the keeper");
    // On a seat's screen as well.
    let mut quiet = run.clone();
    quiet.alehouse.calls.clear();
    place(&mut run, 1, alehouse::TAPS[1]);
    place(&mut quiet, 1, alehouse::TAPS[1]);
    assert!(!same(&seat_img(&run, 1), &seat_img(&quiet, 1)));
    // Past its moment the hall is the hall as it was.
    run.tick += CALL_TICKS;
    quiet.tick += CALL_TICKS;
    assert_eq!(run.calls_up().count(), 0);
    assert!(same(&whole(&run), &whole(&quiet)));
}

#[test]
fn the_stoked_hearth_is_drawn_higher_than_the_banked_one() {
    let mut run = start(1);
    run_for(&mut run, 30);
    let (c, r, w, h) = alehouse::HEARTH;
    let area = |img: &Img| crop(img, c * 16, (r - 1) * 16, w * 16, h * 16 + 16);
    let banked = area(&whole(&run));
    run.alehouse.stoked = run.tick;
    let leaping = area(&whole(&run));
    assert!(!same(&banked, &leaping));
}

#[test]
fn the_place_is_named_on_screen_for_its_moment_only() {
    for mut run in [start(1), courtyard(1)] {
        run_for(&mut run, TITLE_TICKS + 10);
        let mut bare = run.clone();
        bare.alehouse.came = 1;
        run.alehouse.came = run.tick - 12;
        assert!(run.title_age().is_some() && bare.title_age().is_none());
        assert!(!same(&seat_img(&run, 1), &seat_img(&bare, 1)), "the name is up");
        run.tick += TITLE_TICKS;
        bare.tick += TITLE_TICKS;
        assert!(same(&seat_img(&run, 1), &seat_img(&bare, 1)), "and gone");
    }
}

#[test]
fn the_dragons_eye_turns_to_the_nearest_knight() {
    let mut run = courtyard(1);
    run_for(&mut run, TITLE_TICKS + 10);
    let (x0, y0) = inn_at();
    let eye = |run: &Run| crop(&whole(run), x0 + 38, y0 + 26, 14, 10);
    // A knight to the west of the eye, then one to the east of it.
    place(&mut run, 1, (alehouse::COURTYARD_FRONT.0 - 4.0, alehouse::COURTYARD_FRONT.1));
    let west = eye(&run);
    place(&mut run, 1, (alehouse::COURTYARD_FRONT.0 + 4.0, alehouse::COURTYARD_FRONT.1));
    let east = eye(&run);
    assert!(!same(&west, &east), "the slit turns after them");
    // No one near: a dimmer eye than a watching one.
    place(&mut run, 1, (1.5, 1.5));
    let idle = eye(&run);
    assert!(!same(&idle, &west));
}

#[test]
#[ignore]
fn write_hall_voices_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // Arrival: the tavern's name in the corner, the party at the door.
    let mut run = start(4);
    run_for(&mut run, 25);
    save("voices-01-arrival-tavern", &seat_img(&run, 1), 3);
    // The keeper greets the first knight at his bar by name.
    let mut run = start(2);
    run_for(&mut run, 100);
    place(&mut run, 2, alehouse::TAPS[1]);
    run_for(&mut run, 18);
    save("voices-02-keeper-greets", &seat_img(&run, 2), 3);
    // A calling taken up: the next bench toasts its knight.
    let mut run = start(2);
    run_for(&mut run, 100);
    let mage = alehouse::PATRONS.iter().find(|p| p.kit == "scryglass").unwrap().at;
    place(&mut run, 2, (mage.0, mage.1 - 1.0));
    press(&mut run, 2);
    run_for(&mut run, 12);
    save("voices-03-toast", &seat_img(&run, 2), 3);
    // A pour paid for: the purse on the board, then the keeper's call.
    let mut run = start(2);
    run_for(&mut run, 100);
    run.alehouse.greeted = u32::MAX;
    place(&mut run, 2, alehouse::TAPS[0]);
    run.step(&BTreeMap::new());
    save("voices-04-pour-prompt", &seat_img(&run, 2), 3);
    hold(&mut run, 2);
    run_for(&mut run, 8);
    save("voices-05-pour-called", &seat_img(&run, 2), 3);
    // The hearth, banked, then stoked.
    let mut run = start(1);
    run_for(&mut run, 100);
    let (c, r, w, h) = alehouse::HEARTH;
    let hearth = |run: &Run| crop(&whole(run), (c - 1) * 16, (r - 1) * 16 - 8, (w + 2) * 16, (h + 2) * 16 + 16);
    save("voices-06-hearth-banked", &hearth(&run), 5);
    place(&mut run, 1, alehouse::WARM_AT);
    press(&mut run, 1);
    save("voices-07-hearth-stoked", &hearth(&run), 5);
    run_for(&mut run, 14);
    save("voices-08-hearth-stoked-later", &hearth(&run), 5);
    save("voices-09-hearth-seat", &seat_img(&run, 1), 3);
    // The bazaar: its name on coming in, and a sale called by the vendor.
    let mut run = start(2);
    let to = world::room_of(&run.dungeon, crate::rules::RoomKind::Bazaar).unwrap();
    run.arrive(to, bazaar::ARRIVAL);
    run_for(&mut run, 25);
    save("voices-10-arrival-bazaar", &seat_img(&run, 2), 3);
    run.players.get_mut(&2).unwrap().carried.add(crate::realm::Spoil::Gold, 500);
    let slot = 4u8;
    place(&mut run, 2, bazaar::stand_of(slot).unwrap());
    run.step(&BTreeMap::new());
    save("voices-11-ware-prompt", &seat_img(&run, 2), 3);
    hold(&mut run, 2);
    run_for(&mut run, 8);
    save("voices-12-sale-called", &seat_img(&run, 2), 3);
    // The courtyard: its name on arrival and the dragon snorting at the
    // party; the head close, watching, idle and blinking.
    let mut run = courtyard(2);
    run_for(&mut run, 22);
    save("voices-13-arrival-courtyard", &seat_img(&run, 1), 3);
    let (x0, y0) = inn_at();
    save("voices-14-dragon-snort", &crop(&whole(&run), x0 - 10, y0 - 6, 116, 96), 6);
    run_for(&mut run, TITLE_TICKS + 20);
    place(&mut run, 1, (alehouse::COURTYARD_FRONT.0 - 3.0, alehouse::COURTYARD_FRONT.1));
    save("voices-15-dragon-watching", &crop(&whole(&run), x0 - 10, y0 - 6, 116, 96), 6);
    place(&mut run, 1, (1.5, 1.5));
    place(&mut run, 2, (1.5, 1.5));
    save("voices-16-dragon-idle", &crop(&whole(&run), x0 - 10, y0 - 6, 116, 96), 6);
}
