//! A floor's results on screen: the board, its motion, and where a seat
//! sees it.

use super::*;
use crate::rules::floor_stats::{End, Line, Results, Tally};
use crate::rules::{HZ, Run};

/// The pixels lit above the board's darkest greys.
fn lit(img: &Img) -> usize {
    img.pixels().filter(|c| c.iter().any(|&v| v > 40)).count()
}

fn on_palette(img: &Img) -> bool {
    img.pixels().all(|c| c == BLACK || super::super::super::ink::palette_index(c).is_some())
}

/// A party of `n` (Matt, Wren, Bors and friends) and a floor they played:
/// each knight their own story, a wish that doubled Wren's pace.
fn party(n: u32) -> Run {
    let mut run = Run::new(17, 7, None);
    for (id, name) in [(2, "Wren"), (3, "Bors"), (4, "Ysolde"), (5, "Gawain"), (6, "Lynette")].into_iter().take(n as usize - 1) {
        run.join_seat(id, name);
    }
    run.players.get_mut(&1).unwrap().name = "Matt".into();
    run
}

fn played(run: &Run, depth: u32, end: End) -> Results {
    let mut tally = Tally::default();
    let lines = [
        Line { kills: 14, damage: 1260, taken: 42, shots: 180, hits: 117, gold: 23, fought: 2400, ..Line::default() },
        Line { kills: 22, damage: 2315, taken: 0, shots: 260, hits: 203, gold: 9, fought: 2400, ..Line::default() },
        Line { kills: 6, damage: 540, taken: 118, falls: 1, raised: 1, gold: 31, fought: 2100, ..Line::default() },
        Line { kills: 9, damage: 880, taken: 60, shots: 90, hits: 41, gold: 4, fought: 2400, ..Line::default() },
        Line { kills: 3, damage: 310, taken: 75, shots: 40, hits: 22, fought: 1800, ..Line::default() },
        Line { kills: 11, damage: 990, taken: 20, shots: 120, hits: 80, gold: 12, fought: 2400, ..Line::default() },
    ];
    let hz = u64::from(HZ);
    for t in 0..20 * hz {
        tally.count(2, if t % hz == 0 { 40 } else { 0 }, true);
    }
    tally.wished(2, "Twin Bolts");
    for t in 0..20 * hz {
        tally.count(2, if t % hz == 0 { 76 } else { 0 }, true);
    }
    for (&id, line) in run.players.keys().zip(lines) {
        *tally.line(id) = line;
    }
    Results::of(&tally, &run.players, depth, end, 4 * 60 * hz + 12 * hz)
}

/// The steps the board is drawn at in its shots and tests.
const AGES: [u32; 10] = [6, 16, 26, 40, 56, 72, 86, 100, 120, 190];

#[test]
fn the_board_draws_on_black_within_the_stage_on_the_palette() {
    for n in [1, 3, 6] {
        let run = party(n);
        let results = played(&run, 3, End::Cleared);
        for age in AGES {
            let img = frame(&run, &results, age, NATIVE_W, NATIVE_H);
            assert_eq!((img.w, img.h), (NATIVE_W, NATIVE_H));
            assert!(on_palette(&img), "{n} knights, step {age}: off the palette");
            for (x, y) in [(0, 0), (NATIVE_W - 1, 0), (0, NATIVE_H - 1), (NATIVE_W - 1, NATIVE_H - 1)] {
                assert_eq!(img.get(x, y), Some(BLACK), "{n} knights, step {age}: the board lies on black");
            }
            assert!(lit(&img) * 4 < img.pixels().count(), "{n} knights, step {age}: mostly black and dark greys");
        }
        for (w, h) in [(NATIVE_W * 2, NATIVE_H * 2), (NATIVE_W * 3 / 2, NATIVE_H), (120, 70)] {
            let img = frame(&run, &results, 120, w, h);
            assert_eq!((img.w, img.h), (w, h));
            assert!(on_palette(&img));
            assert!(img.pixels().any(|c| c != BLACK), "{w}x{h}: something to read");
        }
    }
}

#[test]
fn its_motion_advances_with_the_step_and_every_seat_draws_the_same_step_alike() {
    let run = party(3);
    let results = played(&run, 3, End::Cleared);
    let frames: Vec<Img> = (0..=crate::rules::floor_stats::SHOWN as u32).map(|age| frame(&run, &results, age, NATIVE_W, NATIVE_H)).collect();
    // It builds up: more of the board is drawn as it plays.
    assert!(lit(&frames[10]) < lit(&frames[40]));
    assert!(lit(&frames[40]) < lit(&frames[150]));
    // And moves the whole while: no long stretch stands still before the
    // wish's counts have landed.
    let still = frames.windows(6).take(130).filter(|w| w.iter().all(|f| *f == w[0])).count();
    assert_eq!(still, 0, "a stretch of six steps without a change");
    assert_eq!(frames[77], frame(&run, &results, 77, NATIVE_W, NATIVE_H), "a step is drawn alike every time");
    // It closes onto the floor's plate.
    assert!(lit(&frames[crate::rules::floor_stats::SHOWN as usize - 1]) < lit(&frames[150]));
}

#[test]
fn a_seat_sees_the_board_first_in_the_next_floors_plate_and_then_its_title() {
    let mut run = party(3);
    let results = played(&run, 1, End::Cleared);
    run.floor_results = Some(results.clone());
    run.begin_floor_plate(crate::rules::floor_plates::Kind::Crypt);
    for plate in run.floor_plates.values_mut() {
        plate.results = crate::rules::floor_stats::SHOWN;
        plate.age = 50;
    }
    assert_eq!(frame_for(&run, NATIVE_W, NATIVE_H, Some(2)), frame(&run, &results, 50, NATIVE_W, NATIVE_H));
    let seat = seat_scene(&run, NATIVE_W, NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H, Some(2)).compose();
    assert!(on_palette(&seat));
    let plate = run.floor_plates.get_mut(&2).unwrap();
    plate.age = crate::rules::floor_stats::SHOWN + 10;
    let title = run.floor_plate(Some(2)).unwrap().title().unwrap();
    assert_eq!(title.age, 10);
    assert_eq!(frame_for(&run, NATIVE_W, NATIVE_H, Some(2)), super::super::plates::frame(title, NATIVE_W, NATIVE_H));
}

/// A finished run's deeds: the game's achievements by their ids.
const DEEDS: [&str; 4] = ["first_delve", "party_of_four", "below_the_bottom", "out_of_time"];

fn finished(run: &Run, end: End) -> Results {
    let mut results = played(run, 5, end);
    results.deeds = DEEDS.iter().map(|d| d.to_string()).collect();
    results
}

#[test]
fn a_finished_runs_board_turns_to_its_deeds_once_its_floor_is_read() {
    let run = party(3);
    let results = finished(&run, End::Fallen);
    for id in DEEDS {
        assert!(crate::rules::feats::feat(id).is_some(), "{id} is an achievement");
    }
    let at = Beats::of(&results).deeds;
    let floor = frame(&run, &results, at - 1, NATIVE_W, NATIVE_H);
    let deeds: Vec<Img> = (0..80).map(|d| frame(&run, &results, at + d, NATIVE_W, NATIVE_H)).collect();
    assert_ne!(floor, deeds[0], "the floor gives way to the deeds");
    for img in &deeds {
        assert!(on_palette(img));
    }
    assert!(lit(&deeds[4]) < lit(&deeds[60]), "the deeds come up one by one");
    // A floor cleared, or a run with no deeds, keeps its one board.
    let cleared = Results { end: End::Cleared, ..results.clone() };
    assert_eq!(frame(&run, &cleared, at + 40, NATIVE_W, NATIVE_H), frame(&run, &Results { deeds: Vec::new(), ..cleared.clone() }, at + 40, NATIVE_W, NATIVE_H));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_floor_stats_shots -- --ignored`:
/// the board for a party of three through its motion, and a party of six,
/// a knight alone and a fallen party's last floor at their ends.
#[test]
#[ignore]
fn write_floor_stats_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let run = party(3);
    let results = played(&run, 3, End::Cleared);
    for age in AGES {
        save(&format!("floor-stats-3-{age:03}"), &frame(&run, &results, age, NATIVE_W, NATIVE_H), 3);
    }
    let six = party(6);
    save("floor-stats-6-090", &frame(&six, &played(&six, 4, End::Cleared), 90, NATIVE_W, NATIVE_H), 3);
    save("floor-stats-6-end", &frame(&six, &played(&six, 4, End::Cleared), 170, NATIVE_W, NATIVE_H), 3);
    let five = party(5);
    save("floor-stats-5-end", &frame(&five, &played(&five, 4, End::Cleared), 170, NATIVE_W, NATIVE_H), 3);
    let one = party(1);
    save("floor-stats-1-end", &frame(&one, &played(&one, 2, End::Cleared), 170, NATIVE_W, NATIVE_H), 3);
    let mut fallen = party(3);
    fallen.again = Some(crate::rules::again::Again { asked: Default::default(), left: 5 * HZ });
    save("floor-stats-fallen", &frame(&fallen, &played(&fallen, 5, End::Fallen), 170, NATIVE_W, NATIVE_H), 3);
    save("floor-stats-small", &frame(&run, &results, 170, 160, 93), 4);
    let won = finished(&run, End::Won);
    let at = Beats::of(&won).deeds;
    for d in [0, 12, 30, 90] {
        save(&format!("floor-stats-deeds-{d:03}"), &frame(&run, &won, at + d, NATIVE_W, NATIVE_H), 3);
    }
}
