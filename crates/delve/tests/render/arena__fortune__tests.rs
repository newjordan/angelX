use super::*;
use crate::render::overworld::arena::frame_for;
use crate::render::overworld::ink::text_width;
use crate::rules::{Input, Run};
use std::collections::BTreeMap;

type Keys = BTreeMap<u32, Input>;

/// A knight who wanders: a new heading every `hold` ticks.
fn wander(id: u32, t: u32, hold: u32) -> Input {
    let mut h = id.wrapping_mul(0x9E37_79B1) ^ (t / hold).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    let (x, y) = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)][(h % 8) as usize];
    Input { move_x: x, move_y: y, ..Default::default() }
}

fn join(r: &mut Run, seats: u32) {
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
}

/// A delve floor's own mini-game, `depth` floors down: the folly its hall holds.
fn floor_game(depth: u32, seats: u32) -> Run {
    let mut r = Run::new(41, 1, None);
    for _ in 0..depth {
        r.descend_for_test();
    }
    join(&mut r, seats);
    r.enemies.clear();
    r.deal_legacy_for_test();
    r.enter_for_test(r.folly_room.expect("a folly room"));
    r
}

/// Which folly a floor holds.
fn which(r: &Run) -> &'static str {
    if r.folly.is_some() {
        "wax_maze"
    } else if r.bats.is_some() {
        "belfry_shoot"
    } else if r.powder.is_some() {
        "powder"
    } else if r.hop.is_some() {
        "book_hop"
    } else {
        "none"
    }
}

/// Run the game with its knights wandering until the dare's board is up.
fn into_the_window(r: &mut Run, extra: u32) {
    let after = (r.tune.dare_after * crate::rules::HZ as f32) as u64;
    let since = r.dare.as_ref().expect("a dare is called").since;
    let mut t = 0;
    while r.tick < since + after + u64::from(extra) {
        let k: Keys = r.players.keys().map(|&id| (id, wander(id, t, 14))).collect();
        r.step(&k);
        t += 1;
    }
}

#[test]
#[ignore]
fn write_dare_baseline() {
    let Some(dir) = std::env::var_os("ANGEL_FORTUNE_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for depth in 1..=6 {
        let mut r = floor_game(depth, 2);
        let game = which(&r);
        into_the_window(&mut r, 24);
        let img = frame_for(&r, 384, 224, Some(1));
        image::save_buffer(dir.join(format!("{depth}-{game}.png")), &img.rgb_bytes(), img.w as u32, img.h as u32, image::ColorType::Rgb8).unwrap();
    }
}

// ─── the wheel ──────────────────────────────────────────────────────────────

use crate::realm::Spoils;
use crate::rules::fortune::{self, Luck, Mode};
use crate::rules::home::{BUY_HOLD, Home, Station};

/// The hall with the wheel's `level` rung, `seats` knights in it, knight one
/// on the lever, the rest on the floor before the benches.
fn hall(seed: u64, level: u8, seats: u32) -> Run {
    let mut home = Home { deepest: 3, ..Home::default() };
    home.levels.insert(Station::Wheel, level);
    let mut run = Run::at_home(seed, 1, None, home, Spoils::default());
    join(&mut run, seats);
    run.enter_for_test(1);
    let (c, r, w, h) = fortune::LEVER;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * 2.0;
    hero.y = (r as f32 + h as f32 / 2.0) * 2.0;
    for id in 2..=seats {
        let hero = run.players.get_mut(&id).unwrap();
        hero.x = (14.0 + id as f32 * 1.6) * 2.0;
        hero.y = 9.2 * 2.0;
    }
    run.step(&Keys::new());
    run
}

fn hold(on: bool) -> Keys {
    BTreeMap::from([(1, Input { interact: on, ..Default::default() })])
}

/// A seed whose wheel stops on a wedge of `luck`, at the fourth rung.
fn seed_for(luck: Luck, skip: usize) -> u64 {
    (0..400u64)
        .filter(|&seed| {
            let mut r = hall(seed, 4, 1);
            for _ in 0..BUY_HOLD + 1 {
                r.step(&hold(true));
            }
            r.spin.is_some_and(|s| Mode::WHEEL[usize::from(s.wedge)].luck() == luck)
        })
        .nth(skip)
        .expect("a seed")
}

fn save(dir: &std::path::Path, n: u32, img: &crate::render::overworld::Img) {
    image::save_buffer(dir.join(format!("f{n:04}.png")), &img.rgb_bytes(), img.w as u32, img.h as u32, image::ColorType::Rgb8).unwrap();
}

/// The spin on film, a frame every tick, as seat one sees it and (beside it)
/// seat two: `ANGEL_FORTUNE_FILM=<dir>` writes `<dir>/<name>/f0000.png`.
#[test]
#[ignore]
fn write_fortune_film() {
    let Some(root) = std::env::var_os("ANGEL_FORTUNE_FILM") else { return };
    let root = std::path::PathBuf::from(root);
    for (name, luck, skip, seats) in [("grand", Luck::Grand, 0, 4), ("fair", Luck::Fair, 0, 4), ("plain", Luck::Plain, 0, 4), ("grand-two", Luck::Grand, 1, 2)] {
        if let Ok(only) = std::env::var("FILM_ONLY") && !only.split(',').any(|o| o == name) {
            continue;
        }
        let dir = root.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut run = hall(seed_for(luck, skip), 4, seats);
        let mut n = 0;
        let mut frame = |run: &Run| {
            let mut sheet = crate::render::overworld::Img::black(768, 224);
            sheet.stamp(&frame_for(run, 384, 224, Some(1)), 0, 0);
            sheet.stamp(&frame_for(run, 384, 224, Some(2.min(seats))), 384, 0);
            save(&dir, n, &sheet);
            n += 1;
        };
        for _ in 0..20 {
            run.step(&Keys::new());
            frame(&run);
        }
        for _ in 0..BUY_HOLD + 1 {
            run.step(&hold(true));
            frame(&run);
        }
        run.step(&hold(false));
        let after = fortune::SPIN_TICKS + 190;
        for _ in 0..after {
            run.step(&Keys::new());
            frame(&run);
        }
        println!("{name}: {n} frames, landed on {:?}", run.mode);
    }
}

// ─── words ──────────────────────────────────────────────────────────────────

use super::claims::{self, Rect};
use crate::render::overworld::ink::said;

fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

fn inside(r: Rect, w: i32, h: i32) -> bool {
    r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= w && r.1 + r.3 <= h
}

/// Every box of words laid in seat `seat`'s view of the hall, once each.
fn hall_words(run: &Run, seat: u32) -> Vec<(String, Rect)> {
    let (_, mut laid) = claims::read(|| frame_for(run, 384, 224, Some(seat)));
    laid.sort();
    laid.dedup();
    laid
}

/// No two boxes of words meet and none leaves the room above the tray.
fn assert_clear(laid: &[(String, Rect)], what: &str) {
    for (i, (a, ra)) in laid.iter().enumerate() {
        assert!(inside(*ra, 384, 192), "{what}: {a} {ra:?} leaves the view");
        for (b, rb) in &laid[i + 1..] {
            assert!(!meets(*ra, *rb), "{what}: {a} {ra:?} meets {b} {rb:?}");
        }
    }
}

#[test]
fn the_halls_words_never_meet_through_a_whole_spin_at_one_to_four_seats() {
    for luck in [Luck::Grand, Luck::Fair, Luck::Plain] {
        for seats in [1, 2, 4] {
            let mut run = hall(seed_for(luck, 0), 4, seats);
            let mut seen = std::collections::BTreeSet::new();
            for _ in 0..BUY_HOLD + 1 {
                run.step(&hold(true));
            }
            run.step(&hold(false));
            for t in 0..fortune::SPIN_TICKS + 220 {
                run.step(&Keys::new());
                if t % 2 == 0 {
                    for seat in 1..=seats {
                        let laid = hall_words(&run, seat);
                        seen.extend(laid.iter().map(|(w, _)| w.clone()));
                        assert_clear(&laid, &format!("{luck:?} seats {seats} seat {seat} tick {t}"));
                    }
                }
            }
            for want in ["pull", "tease", "reveal", "sign"] {
                assert!(seen.contains(want), "{luck:?}: no {want} was ever laid ({seen:?})");
            }
        }
    }
}

#[test]
fn the_plates_boards_and_the_coffers_never_meet_either() {
    use crate::rules::feats::Tier;
    let mut run = hall(5, 4, 4);
    run.home.boxes = vec![Tier::Gold, Tier::Silver];
    // One knight on the lever, one on Fortune's plate, one on the coffer's.
    let stand = |run: &mut Run, id: u32, plate: (i32, i32, i32, i32)| {
        let hero = run.players.get_mut(&id).unwrap();
        hero.x = (plate.0 as f32 + plate.2 as f32 / 2.0) * 2.0;
        hero.y = (plate.1 as f32 + plate.3 as f32 / 2.0) * 2.0;
    };
    stand(&mut run, 1, fortune::LEVER);
    stand(&mut run, 2, fortune::FORTUNE_PLATE);
    stand(&mut run, 3, fortune::COFFER_PLATE);
    run.unboxed = Some((run.tick, Tier::Legendary, "A legendary find".into()));
    run.step(&Keys::new());
    for seat in 1..=4 {
        let laid = hall_words(&run, seat);
        assert!(laid.iter().any(|(w, _)| w == "lever") || laid.iter().any(|(w, _)| w == "ledger"), "{laid:?}");
        assert_clear(&laid, &format!("plates, seat {seat}"));
    }
}

#[test]
fn you_names_only_the_seat_that_pulled_and_only_for_that_seat() {
    let mut run = hall(seed_for(Luck::Fair, 0), 4, 4);
    // Knight three pulls: they hold the lever.
    let (c, r, w, h) = fortune::LEVER;
    for id in 1..=4 {
        let hero = run.players.get_mut(&id).unwrap();
        if id == 3 {
            hero.x = (c as f32 + w as f32 / 2.0) * 2.0;
            hero.y = (r as f32 + h as f32 / 2.0) * 2.0;
        } else {
            hero.x = (3.0 + id as f32) * 2.0;
            hero.y = 12.0 * 2.0;
        }
    }
    let hold3 = |on| BTreeMap::from([(3, Input { interact: on, ..Default::default() })]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&hold3(true));
    }
    run.step(&hold3(false));
    assert_eq!(run.spin.map(|s| s.by), Some(3));
    for _ in 0..8 {
        run.step(&Keys::new());
    }
    for seat in 1..=4u32 {
        let (_, said) = said::read(|| frame_for(&run, 384, 224, Some(seat)));
        let texts: Vec<&str> = said.lines.iter().map(|l| l.text.as_str()).collect();
        let you = texts.iter().filter(|t| t.split(' ').any(|w| w == "YOU")).count();
        if seat == 3 {
            assert!(texts.contains(&"YOU PULL"), "seat 3 pulled: {texts:?}");
        } else {
            assert_eq!(you, 0, "seat {seat} did not pull and is not told YOU: {texts:?}");
            assert!(texts.iter().any(|t| t.ends_with(" PULLS")), "seat {seat} reads who pulled: {texts:?}");
            assert!(texts.iter().any(|t| *t == "KNIGHT 3 PULLS"), "{texts:?}");
        }
        for line in texts.iter().filter(|t| t.ends_with("PULL") || t.ends_with("PULLS")) {
            assert!(line.split(' ').count() <= 3, "{line}");
        }
    }
}

#[test]
fn every_line_the_wheel_speaks_is_two_or_three_plain_words() {
    let mut run = hall(seed_for(Luck::Grand, 1), 4, 2);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&hold(true));
    }
    run.step(&hold(false));
    for t in 0..fortune::SPIN_TICKS + 200 {
        run.step(&Keys::new());
        if t % 3 == 0 {
            let (_, said) = said::read(|| frame_for(&run, 384, 224, Some(1)));
            for line in said.lines.iter().filter(|l| l.on.0 > 8 && l.text.len() > 3) {
                assert!(line.text.split(' ').count() <= 3, "{:?} is more than three words", line.text);
            }
        }
    }
}

// ─── the dare card ──────────────────────────────────────────────────────────

use crate::rules::dares::{CALLED, DareKind};

#[test]
fn the_dare_card_is_two_short_lines_and_fits_the_view() {
    for kind in [DareKind::Untouched, DareKind::StandFirm, DareKind::AgainstTheClock, DareKind::Pathfinder, DareKind::MakeItQuick, DareKind::Unstoppable] {
        for line in [kind.card_name(), kind.card_rule()] {
            assert!(line.split(' ').count() <= 3, "{kind:?}: {line:?}");
            assert_eq!(line, line.to_uppercase());
        }
        let dare = crate::rules::dares::Dare { kind, since: 0, count: 0, kept: false, broken: false, rose: None };
        let card = super::dare::card(&dare);
        let (x, y) = super::dare::rest(&card, 224);
        assert!(inside((x, y, card.w, card.h), 384, 192), "{kind:?}: {x},{y} {}x{}", card.w, card.h);
        assert!(card.w <= 140 && card.h <= 30, "{kind:?} is a card, not a wall: {}x{}", card.w, card.h);
    }
}

/// Where each game keeps its words while a dare is called (the dare is up
/// from five seconds into a floor), as boxes in the 384 by 224 view.
fn kept_words(game: &str) -> Vec<(&'static str, Rect)> {
    let mut out = vec![
        // The Wax Maze's strip (16) and the Belfry's (21) run along the top.
        ("strip", (0, 0, 384, 22)),
        // The folly boards top right, their titles, and every banner and call the
        // lanes put up, in the middle of the view.
        ("middle", (0, 22, 384, 138)),
    ];
    if game == "wax_maze" {
        // Its map in the bottom right corner.
        out.push(("map", (192, 160, 192, 32)));
    }
    out
}

/// A seat's view with the dare card up and without it, each re-drawn once to be sure of it: the
/// views are only a fair pair when every re-draw of an unchanged state is the same picture. The
/// suite's other tests share process-wide switches and caches, so a draw can now and then differ
/// from the next by a few pixels in the tray whatever this test does; such a pair is tried again.
#[allow(clippy::type_complexity)]
fn fair_pair(
    run: &Run,
    bare: &Run,
    seat: u32,
    game: &str,
) -> Option<(crate::render::overworld::Img, Vec<(String, Rect)>, Vec<Rect>, crate::render::overworld::Img, said::Said)> {
    for _ in 0..8 {
        // Hold the lit-scenery cache for both looks, so another test's room cannot
        // evict this room's drawing between them (`scenery::pin_scene_cache`).
        let _pin = crate::render::overworld::arena::scenery::pin_scene_cache();
        let (with, laid) = claims::read(|| frame_for(run, 384, 224, Some(seat)));
        // What the Powder Room says it keeps on that screen (the only game that lists its boxes).
        let theirs: Vec<Rect> = if game == "powder" {
            crate::render::overworld::arena::powder_words().iter().map(|c| c.rect).collect()
        } else {
            Vec::new()
        };
        let (without, said) = said::read(|| frame_for(bare, 384, 224, Some(seat)));
        if with == frame_for(run, 384, 224, Some(seat)) && without == frame_for(bare, 384, 224, Some(seat)) {
            return Some((with, laid, theirs, without, said));
        }
    }
    None
}

#[test]
fn the_dare_card_never_meets_another_games_words_and_only_its_own_corner_changes() {
    let mut seen = std::collections::BTreeSet::new();
    let (mut pairs, mut unfair) = (0, 0);
    for depth in 1..=6 {
        for seats in [1, 2, 4] {
            let mut r = floor_game(depth, seats);
            let game = which(&r);
            seen.insert(game);
            let after = (r.tune.dare_after * crate::rules::HZ as f32) as u64;
            let from = r.dare.as_ref().expect("a dare is called on every floor").since + after;
            let mut t = 0;
            while r.tick < from {
                let k: Keys = r.players.keys().map(|&id| (id, wander(id, t, 14))).collect();
                r.step(&k);
                t += 1;
            }
            let mut rested = false;
            for i in 0..CALLED as u32 {
                let k: Keys = r.players.keys().map(|&id| (id, wander(id, t + i, 14))).collect();
                r.step(&k);
                if i % 3 != 0 {
                    continue;
                }
                let card = super::dare::card(r.dare.as_ref().unwrap());
                let (rx, ry) = super::dare::rest(&card, 224);
                let rect = (rx, ry, card.w, card.h);
                // Whole in the view, above the tray, and clear of every box the games keep.
                assert!(inside(rect, 384, 192), "{game}: the card {rect:?} leaves the view");
                for (what, zone) in kept_words(game) {
                    assert!(!meets(rect, zone), "{game}: the card {rect:?} meets the game's {what} {zone:?}");
                }
                let mut bare = r.clone();
                bare.tune.dare_after = 10.0;
                for seat in 1..=seats {
                    let Some((with, laid, theirs, without, said)) = fair_pair(&r, &bare, seat, game) else {
                        unfair += 1;
                        continue;
                    };
                    pairs += 1;
                    rested |= laid.iter().any(|(w, _)| w == "dare");
                    for theirs in &theirs {
                        assert!(!meets(rect, *theirs), "{game} seat {seat} tick {i}: the card {rect:?} meets the game's words {theirs:?}");
                    }
                    // Nothing but the card's own corner changes (the card slides in from the edge).
                    let mut changed = 0;
                    for y in 0..224 {
                        for x in 0..384 {
                            if with.get(x, y) != without.get(x, y) {
                                changed += 1;
                                assert!(x < rx + card.w + 2 && y >= ry && y < ry + card.h + 2, "{game} depth {depth} seats {seats} seat {seat} tick {i}: pixel {x},{y} changed outside the card {rect:?}");
                            }
                        }
                    }
                    // And it sits on no line of words the game wrote on the view itself.
                    for line in said.lines.iter().filter(|l| l.on == (384, 224)) {
                        let words = (line.at.0, line.at.1, text_width(&line.text), 7);
                        assert!(!meets(rect, words) || changed == 0, "{game}: the card meets {:?} at {words:?}", line.text);
                    }
                }
            }
            assert!(rested, "{game} depth {depth} seats {seats}: the card was never at rest");
        }
    }
    assert_eq!(seen.len(), 4, "every folly was covered: {seen:?}");
    assert!(pairs > 0 && unfair * 4 <= pairs, "{unfair} of {} pairs would not hold still", pairs + unfair);
}

#[test]
fn the_dare_card_waits_behind_a_finished_games_own_screen() {
    let mut r = floor_game(1, 2);
    r.bats.as_mut().expect("the belfry").done = true;
    let after = (r.tune.dare_after * crate::rules::HZ as f32) as u64;
    let from = r.dare.as_ref().unwrap().since + after;
    while r.tick < from + 30 {
        r.step(&Keys::new());
    }
    assert!(super::dare::age(&r).is_some(), "the dare is called");
    let mut bare = r.clone();
    bare.tune.dare_after = 10.0;
    assert_eq!(frame_for(&r, 384, 224, Some(1)), frame_for(&bare, 384, 224, Some(1)));
}

/// The dare card over each folly on film, a frame every tick of its five seconds and a little either side:
/// `ANGEL_DARE_FILM=<dir>` writes `<dir>/dare-<game>/f0000.png`.
#[test]
#[ignore]
fn write_dare_film() {
    let Some(root) = std::env::var_os("ANGEL_DARE_FILM") else { return };
    let root = std::path::PathBuf::from(root);
    for depth in [1, 2, 3, 6] {
        let mut r = floor_game(depth, 4);
        let game = which(&r);
        let dir = root.join(format!("dare-{game}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let after = (r.tune.dare_after * crate::rules::HZ as f32) as u64;
        let from = r.dare.as_ref().unwrap().since + after;
        let mut t = 0;
        let mut n = 0;
        while r.tick < from + CALLED + 12 {
            let k: Keys = r.players.keys().map(|&id| (id, wander(id, t, 14))).collect();
            r.step(&k);
            t += 1;
            if r.tick + 14 >= from {
                let img = frame_for(&r, 384, 224, Some(1));
                image::save_buffer(dir.join(format!("f{n:04}.png")), &img.rgb_bytes(), img.w as u32, img.h as u32, image::ColorType::Rgb8).unwrap();
                n += 1;
            }
        }
        println!("dare-{game}: {n} frames");
    }
}

