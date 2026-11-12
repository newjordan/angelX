//! Cinder Stack's picture: the words walked over whole runs at one to six seats, the proof reels and
//! the frame-time bench. The real `Run` is played through by the game's own bot; nothing here changes
//! game code and the reels write only under the directory they are given.
//!
//!   ANGEL_CINDER_REEL=<dir> [REEL_ONLY=chain,finish] cargo test -p angel-delve --lib write_cinder_reel -- --ignored --nocapture
//!   cargo test -p angel-delve --lib cinder_frame_time -- --ignored --nocapture
//!
//! `/work/artifacts/minigames-night-20261107/cinder/make_gifs.py` makes the GIFs from the frames.
use super::*;
use crate::render::overworld::arena::frame_for;
use crate::render::overworld::ink::{Img, said, text_width};
use crate::rules::cinder::{CRACKS, LENGTH};
use crate::rules::cinder_show::{Cry, Kind};
use crate::rules::{Input, Run};
use std::collections::{BTreeMap, BTreeSet};

type Keys = BTreeMap<u32, Input>;
type Rect = hud::Rect;

fn press(id: u32) -> Keys {
    BTreeMap::from([(id, Input { interact: true, ..Default::default() })])
}

thread_local! {
    static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the screen is drawn as it was before the juice (the bench's baseline).
pub(super) fn before() -> bool {
    BEFORE.with(std::cell::Cell::get)
}

const NAMES: [&str; 6] = ["You", "Knight 2", "Cedric", "Dara", "You", "Wolfgang Amadeus"];

/// A round with `seats` knights, opened by knight one pressing E at the kiln. Some knights still
/// carry the default `You`, and one has a name too long for any box.
fn table(seats: u32) -> Run {
    let mut r = Run::cinder_for_test();
    if seats == 1 {
        r.leave(2);
    }
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for (i, name) in NAMES.iter().enumerate() {
        if let Some(h) = r.players.get_mut(&(i as u32 + 1)) {
            h.name = (*name).into();
        }
    }
    r.step(&press(1));
    assert!(r.cinder.is_some(), "the kiln opened");
    r
}

/// Every seat's bot, each acting once in `every[seat]` ticks (a seat with 0 sits still).
fn bots(r: &Run, t: u32, every: &[u32]) -> Keys {
    let Some(s) = r.cinder.as_ref().filter(|s| s.left > 0) else { return Keys::new() };
    s.boards
        .iter()
        .filter_map(|(&id, b)| {
            let n = every[(id as usize - 1).min(every.len() - 1)];
            (n > 0 && t % n == 0).then(|| (id, b.bot_input()))
        })
        .collect()
}

/// Every box of words on seat `id`'s screen: the lines of the 5x7 font the pen wrote on the screen,
/// and the pixels of every layer the kit drew words on.
fn boxes(r: &Run, id: u32) -> (Vec<(String, Rect)>, said::Said) {
    let ((_, claims), said) = said::read(|| hud::claimed(|| scene::layers(r, Some(id))));
    let mut out: Vec<(String, Rect)> = said
        .lines
        .iter()
        .filter(|l| l.on == (384, 224))
        .map(|l| (l.text.clone(), (l.at.0, l.at.1, text_width(&l.text), 7)))
        .collect();
    out.extend(claims.into_iter().map(|(name, rect)| (format!("<{name}>"), rect)));
    (out, said)
}

/// No two boxes meet, none leaves the 384 by 224 screen, and `YOU` names only the seat looking.
fn assert_clear(r: &Run, id: u32, what: &str) {
    let (boxes, said) = boxes(r, id);
    for (i, (a, ra)) in boxes.iter().enumerate() {
        assert!(hud::inside(*ra), "{what} seat {id}: {a:?} {ra:?} leaves the screen");
        for (b, rb) in &boxes[i + 1..] {
            assert!(!hud::meets(*ra, *rb), "{what} seat {id}: {a:?} {ra:?} meets {b:?} {rb:?}");
        }
    }
    let s = r.cinder.as_ref().unwrap();
    let now = hud::elapsed(s);
    let mut yous = 0;
    for line in &said.lines {
        let t = line.text.as_str();
        if !(t == "YOU" || t.starts_with("YOU ") || t.ends_with(" YOU") || t.contains(" YOU ")) {
            continue;
        }
        match t {
            "YOU" => yous += 1,
            "YOU LEAD" => assert!(
                s.calls.list.iter().any(|c| c.who == id && c.cry == Cry::Lead && c.age(now) < 80),
                "{what} seat {id}: YOU LEAD for a knight who is not leading"
            ),
            "YOU CRACKED IT" => assert!(s.won && s.order.first() == Some(&id), "{what} seat {id}: YOU CRACKED IT for another's win"),
            // A line of the table under the podium: its place, who, and their cracks.
            other if other.contains(" YOU ") => {
                let place: usize = other.split(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0);
                assert_eq!(s.order.iter().position(|&w| w == id).map(|p| p + 1), Some(place), "{what} seat {id}: {other:?} is not this seat's line");
            }
            other => panic!("{what} seat {id}: {other:?} puts YOU somewhere it should not be"),
        }
    }
    assert!(yous <= 1, "{what} seat {id}: YOU drawn {yous} times");
}

/// The words of a seat are never `YOU` for another seat, in a tile or a toast or on the podium.
#[test]
fn you_names_only_the_seat_looking() {
    let r = table(6);
    for me in 1..=6 {
        for who in 1..=6 {
            let tag = hud::tag(&r, who, 8);
            assert!(tag != "YOU", "tile tag of seat {who} is YOU");
            let said = hud::called(&r, who, me, 18);
            assert_eq!(said == "YOU", who == me, "seat {me} calls seat {who} {said}");
        }
    }
}

fn walk(seats: u32, every: &[u32], what: &str, extra: usize) {
    let mut r = table(seats);
    let mut seen_words: BTreeSet<String> = BTreeSet::new();
    let mut t = 0u32;
    let mut after = 0;
    loop {
        r.step(&bots(&r, t, every));
        t += 1;
        let s = r.cinder.as_ref().unwrap();
        let sampled = if t % 3 == 0 { vec![1 + t / 3 % seats] } else { vec![] };
        let all = t % 45 == 0 || s.left == 0 && t % 4 == 0;
        for id in if all { (1..=seats).collect() } else { sampled } {
            assert_clear(&r, id, &format!("{what}, {seats} seats, tick {t}"));
            for (w, _) in boxes(&r, id).0 {
                seen_words.insert(w);
            }
        }
        assert!(r.cinder.as_ref().unwrap().valid(), "{what}, {seats} seats, tick {t}: the session is valid");
        if r.cinder.as_ref().unwrap().left == 0 {
            after += 1;
            if after > 150 + extra {
                break;
            }
        }
        assert!(t < LENGTH + 400, "{what}, {seats} seats: the round ended");
    }
    for want in ["<go>", "<banner>", "CINDER STACK", "CROCK", "E AGAIN"] {
        assert!(seen_words.contains(want), "{what}, {seats} seats: never saw {want}: {seen_words:?}");
    }
}

#[test]
fn the_words_never_meet_and_every_prompt_sits_inside_the_screen_at_one_to_six_seats() {
    for seats in 1..=6u32 {
        // Quick hands: somebody cracks the kiln. Slow and idle hands: the clock runs out, one knight
        // sits still and her stack climbs to the lid.
        walk(seats, &[3, 5, 4, 6, 7, 5], "fast", 0);
        walk(seats, &[36, 0, 24, 40, 30, 28], "slow", 0);
    }
}

#[test]
fn the_last_five_seconds_the_toasts_and_the_podium_are_all_seen_and_clear() {
    let mut r = table(4);
    // Time almost out, one knight a few cracks short, and the others a chain apart.
    let s = r.cinder.as_mut().unwrap();
    s.left = 6 * 30;
    for (n, b) in s.boards.values_mut().enumerate() {
        b.cracks = [4, 9, 11, 7][n];
    }
    let mut count = false;
    for t in 0..260u32 {
        r.step(&bots(&r, t, &[9, 7, 11, 13]));
        for id in 1..=4 {
            assert_clear(&r, id, &format!("last five, tick {t}"));
        }
        let (words, _) = boxes(&r, 1);
        count |= words.iter().any(|(w, _)| w == "<count>");
    }
    assert!(count, "the last five seconds showed their numbers");
    assert!(r.cinder.as_ref().unwrap().left == 0, "time ran out");
}

#[test]
fn a_storm_of_calls_marks_and_long_names_still_leaves_every_word_clear() {
    let mut r = table(6);
    let s = r.cinder.as_mut().unwrap();
    s.left -= 700;
    let now = hud::elapsed(s);
    for (&id, b) in s.boards.iter_mut() {
        b.cracks = 28 + id as u16;
        b.clock = 400;
        for k in 0..8u32 {
            let mut by = [crate::rules::cinder_show::Cells::default(); 4];
            by[(k % 3) as usize].set((k % 6) as usize, 9);
            by[(k % 3) as usize].set(((k + 1) % 6) as usize, 9);
            by[(k % 3) as usize].set(((k + 2) % 6) as usize, 9);
            b.show.mark(Kind::Pop { by, chain: 1 + (k % 5) as u8, gain: 3 }, 400 - k * 2);
        }
        b.show.mark(Kind::Stage { n: 2 }, 400);
        b.show.mark(Kind::Slag { col: 2, row: 5, hit: true, near: true }, 398);
        b.show.mark(Kind::Cough, 399);
        b.show.mark(Kind::Land { a: (1, 8), b: (1, 7), hard: true, from: 2 }, 400);
    }
    for (n, cry) in [Cry::Lead, Cry::Chain(6), Cry::Stage(2), Cry::Point, Cry::Cough, Cry::Cracked].into_iter().enumerate() {
        s.calls.call(cry, 1 + n as u32, now);
    }
    for id in 1..=6 {
        assert_clear(&r, id, "storm");
    }
    // The end screen with six seats and the longest names, at every beat of it.
    let s = r.cinder.as_mut().unwrap();
    s.left = 0;
    s.won = true;
    s.done = r.tick as u32;
    s.order = (1..=6).collect();
    s.to_beat = 900;
    s.boards.get_mut(&1).unwrap().crack_tick = 600;
    s.boards.get_mut(&1).unwrap().cracks = CRACKS;
    let done = r.tick;
    for tick in (0..260u64).step_by(5) {
        r.tick = done + tick;
        for id in 1..=6 {
            assert_clear(&r, id, &format!("end screen {tick}"));
        }
    }
}

#[test]
fn a_chain_is_staged_and_its_marks_ride_the_wire_and_the_picture_draws_them() {
    let mut r = table(4);
    let b = staged_chain(&mut r, 1, 3);
    assert_eq!(b, 3, "a three link chain");
    let mut kinds = std::collections::HashSet::new();
    let mut called = false;
    for t in 0..120u32 {
        r.step(&bots(&r, t + 1000, &[0, 40, 40, 40]));
        let s = r.cinder.as_ref().unwrap();
        for m in &s.boards[&1].show.marks {
            kinds.insert(std::mem::discriminant(&m.kind));
        }
        assert!(s.valid());
        called |= s.calls.list.iter().any(|c| c.who == 1 && c.cry == Cry::Chain(3));
        if t % 9 == 0 {
            assert_eq!(r.wire_copy().cinder, r.cinder, "the show survives the wire at tick {t}");
            for id in 1..=4 {
                assert_clear(&r, id, &format!("staged chain {t}"));
            }
        }
    }
    assert!(kinds.len() >= 3, "pops, stages and a landing were marked: {kinds:?}");
    assert!(called, "the table heard the chain");
}

/// Lay a board that must chain `links` times when the first pair is dropped in its first column, and
/// drop it. Returns the longest chain the board ran.
fn staged_chain(r: &mut Run, id: u32, links: u8) -> u8 {
    let s = r.cinder.as_mut().unwrap();
    let b = s.boards.get_mut(&id).unwrap();
    let cols: [&[u8]; 6] = if links >= 3 {
        [&[1, 1], &[2, 3], &[2, 3], &[3], &[2, 1], &[3, 2]]
    } else {
        [&[1, 1], &[2], &[2], &[3], &[2, 1], &[3, 2]]
    };
    b.cells = [[0; 6]; 10];
    for (x, col) in cols.iter().enumerate() {
        for (i, &c) in col.iter().enumerate() {
            b.cells[9 - i][x] = c;
        }
    }
    b.piece = 0;
    b.drop_key = false;
    b.fire_key = false;
    b.x = 0;
    b.y = 1;
    b.turn = 0;
    b.fall = 0;
    b.lock = 0;
    b.tell = 0;
    b.attack = 0;
    let mut probe = b.clone();
    let tune = crate::rules::tune::Tune::default();
    probe.step(Input { interact: true, ..Default::default() }, &tune);
    let mut most = 0;
    for _ in 0..200 {
        most = most.max(probe.chain);
        if probe.flash == 0 && probe.chain == 0 {
            break;
        }
        probe.step(Input::default(), &tune);
    }
    // The drop itself: E, held for one tick.
    r.step(&Keys::from([(id, Input { interact: true, ..Default::default() })]));
    most
}

/// Where a frame's time goes, over a bot's round: the picture as it is, five draws of each frame and
/// the quickest taken, the box being what it is.
#[test]
#[ignore = "measures the picture's cost"]
fn cinder_frame_time() {
    use std::time::Instant;
    let mut r = table(4);
    let mut total = [0u128; 2];
    let mut worst = [0u128; 2];
    let mut n = 0u128;
    for t in 0..900u32 {
        r.step(&bots(&r, t, &[6, 7, 5, 8]));
        if t < 30 || t % 3 != 0 {
            continue;
        }
        for (k, old) in [true, false].into_iter().enumerate() {
            BEFORE.with(|b| b.set(old));
            let mut best = u128::MAX;
            for _ in 0..5 {
                let at = Instant::now();
                let _ = frame_for(&r, 384, 224, Some(1));
                best = best.min(at.elapsed().as_micros());
            }
            total[k] += best;
            worst[k] = worst[k].max(best);
        }
        n += 1;
    }
    BEFORE.with(|b| b.set(false));
    // The heaviest pictures: six seats mid-round, and the end screen with the kiln cracked.
    let mut six = table(6);
    for t in 0..400u32 {
        six.step(&bots(&six, t, &[6, 7, 5, 8, 9, 6]));
    }
    let mut end = table(4);
    for t in 0..120u32 {
        end.step(&bots(&end, t, &[6, 7, 5, 8]));
    }
    {
        let s = end.cinder.as_mut().unwrap();
        s.left = 0;
        s.won = true;
        s.order = s.boards.keys().copied().collect();
        s.boards.get_mut(&1).unwrap().cracks = CRACKS;
        s.boards.get_mut(&1).unwrap().crack_tick = 900;
        s.done = end.tick as u32;
    }
    let done = end.tick;
    for (what, at) in [("six seats", 0u64), ("kiln coming apart", 12), ("podium", 140)] {
        let run = if what == "six seats" { &mut six } else { &mut end };
        if what != "six seats" {
            run.tick = done + at;
        }
        let mut cost = [0u128; 2];
        for (k, old) in [true, false].into_iter().enumerate() {
            BEFORE.with(|b| b.set(old));
            cost[k] = (0..5).map(|_| {
                let at = Instant::now();
                let _ = frame_for(run, 384, 224, Some(1));
                at.elapsed().as_micros()
            }).min().unwrap();
        }
        BEFORE.with(|b| b.set(false));
        println!("cinder {what}: before {} us, after {} us", cost[0], cost[1]);
    }
    let mean = |k: usize| total[k] / n.max(1);
    println!("cinder frame over {n} frames: before mean {} us worst {} us; after mean {} us worst {} us; ratio {:.2}", mean(0), worst[0], mean(1), worst[1], mean(1) as f64 / mean(0).max(1) as f64);
}

// ─── the proof reels ─────────────────────────────────────────────────────────

fn save(dir: &std::path::Path, n: u32, im: &Img) {
    image::save_buffer(dir.join(format!("f{n:04}.png")), &im.rgb_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgb8).unwrap();
}

fn reel_only(name: &str) -> bool {
    std::env::var("REEL_ONLY").map_or(true, |o| o.split(',').any(|x| x == name))
}

/// A window of `ticks` of seat one's screen (a frame every step, 30 fps), after `lead` unseen steps.
fn window(root: &std::path::Path, name: &str, r: &mut Run, lead: u32, ticks: u32, mut keys: impl FnMut(&Run, u32) -> Keys) {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for t in 0..lead + ticks {
        let k = keys(r, t);
        r.step(&k);
        if t >= lead {
            save(&dir, t - lead, &frame_for(r, 384, 224, Some(1)));
        }
    }
    println!("{name}: {ticks} frames, {} seats", r.players.len());
}

#[test]
#[ignore = "writes the proof reels"]
fn write_cinder_reel() {
    let Some(root) = std::env::var_os("ANGEL_CINDER_REEL") else { return };
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let quiet = |_: &Run, _: u32| Keys::new();

    if reel_only("before") {
        // The screen as it was, on the same table and the same bots, for the comparison.
        BEFORE.with(|b| b.set(true));
        let mut r = table(4);
        window(&root, "before", &mut r, 0, 240, |r, t| bots(r, t, &[24, 20, 26, 22]));
        BEFORE.with(|b| b.set(false));
    }
    if reel_only("start") {
        let mut r = table(4);
        window(&root, "start", &mut r, 0, 150, |r, t| bots(r, t, &[11, 9, 13, 10]));
    }
    if reel_only("chain") {
        let mut r = table(4);
        for t in 0..90u32 {
            r.step(&bots(&r, t, &[0, 14, 10, 12]));
        }
        assert_eq!(staged_chain_probe(&mut r, 1, 2), 2);
        window(&root, "chain", &mut r, 0, 100, |r, t| bots(r, t + 500, &[0, 14, 10, 12]));
    }
    if reel_only("bigchain") {
        let mut r = table(4);
        for t in 0..90u32 {
            r.step(&bots(&r, t, &[0, 14, 10, 12]));
        }
        assert_eq!(staged_chain_probe(&mut r, 1, 3), 3);
        window(&root, "bigchain", &mut r, 0, 150, |r, t| bots(r, t + 500, &[0, 14, 10, 12]));
    }
    if reel_only("slag") {
        let mut r = table(4);
        r.cinder.as_mut().unwrap().boards.get_mut(&1).unwrap().attack = 120;
        window(&root, "slag", &mut r, 0, 110, |r, t| bots(r, t, &[19, 13, 17, 11]));
    }
    if reel_only("stage") {
        let mut r = table(4);
        for t in 0..60u32 {
            r.step(&bots(&r, t, &[0, 14, 10, 12]));
        }
        r.cinder.as_mut().unwrap().boards.get_mut(&1).unwrap().cracks = 10;
        assert_eq!(staged_chain_probe(&mut r, 1, 2), 2);
        window(&root, "stage", &mut r, 0, 120, |r, t| bots(r, t + 500, &[0, 14, 10, 12]));
    }
    if reel_only("rival") {
        let mut r = table(5);
        for t in 0..90u32 {
            r.step(&bots(&r, t, &[9, 0, 10, 12, 11]));
        }
        r.cinder.as_mut().unwrap().boards.get_mut(&2).unwrap().cracks = 8;
        assert_eq!(staged_chain_probe(&mut r, 2, 3), 3);
        window(&root, "rival", &mut r, 0, 150, |r, t| bots(r, t + 500, &[9, 0, 10, 12, 11]));
    }
    if reel_only("last") {
        let mut r = table(4);
        let s = r.cinder.as_mut().unwrap();
        s.left = 6 * 30;
        for (n, b) in s.boards.values_mut().enumerate() {
            b.cracks = [14, 17, 13, 9][n];
        }
        window(&root, "last", &mut r, 0, 400, |r, t| bots(r, t, &[11, 9, 13, 10]));
    }
    if reel_only("finish") {
        let mut r = table(4);
        for t in 0..60u32 {
            r.step(&bots(&r, t, &[0, 14, 10, 12]));
        }
        let s = r.cinder.as_mut().unwrap();
        s.to_beat = 40 * 30;
        for (n, b) in s.boards.values_mut().enumerate() {
            b.cracks = [31, 22, 17, 9][n];
        }
        assert_eq!(staged_chain_probe(&mut r, 1, 2), 2);
        window(&root, "finish", &mut r, 0, 330, quiet);
    }
    if reel_only("solo") {
        let mut r = table(1);
        window(&root, "solo", &mut r, 40, 120, |r, t| bots(r, t, &[6]));
    }
    if reel_only("six") {
        let mut r = table(6);
        window(&root, "six", &mut r, 90, 140, |r, t| bots(r, t, &[10, 8, 12, 9, 11, 7]));
    }
    if reel_only("danger") {
        let mut r = table(3);
        {
            let b = r.cinder.as_mut().unwrap().boards.get_mut(&1).unwrap();
            for y in 2..10 {
                for x in 0..6 {
                    b.cells[y][x] = [1, 2, 3, 2, 1, 3, 3, 1, 2][(x * 3 + y * 5 + y / 2) % 9];
                }
            }
            b.cells[2] = [1, 2, 0, 0, 1, 2];
        }
        window(&root, "danger", &mut r, 0, 100, |r, t| bots(r, t, &[0, 7, 9]));
    }
}

/// `staged_chain`, for the reel: the board that must chain, dropped, and what it ran.
fn staged_chain_probe(r: &mut Run, id: u32, links: u8) -> u8 {
    staged_chain(r, id, links)
}
