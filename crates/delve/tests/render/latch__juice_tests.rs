//! Latch Duel's picture: the words walked over whole runs at one to six seats, the proof reels and
//! the frame-time bench. The real `Run` is played through by the game's own bots; nothing here changes
//! game code and the reels write only under the directory they are given.
//!
//!   ANGEL_LATCH_REEL=<dir> [REEL_ONLY=counter,finish] cargo test -p angel-delve --lib write_latch_reel -- --ignored --nocapture
//!   cargo test -p angel-delve --lib latch_frame_time -- --ignored --nocapture
//!
//! `/work/artifacts/minigames-night-20261107/latch/make_gifs.py` makes the GIFs from the frames.
use super::*;
use crate::render::overworld::arena::frame_for;
use crate::render::overworld::ink::{Img, said, text_width};
use crate::rules::latch::{Beat, COUNT, LENGTH};
use crate::rules::latch_show::{Cry, Kind};
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

/// A round with `seats` knights, opened by knight one pressing E at the bay. Some knights still carry
/// the default `You`, and one has a name too long for any box.
fn table(seats: u32) -> Run {
    let mut r = Run::latch_for_test();
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
    assert!(r.latch.is_some(), "the bay opened");
    r
}

/// How a seat plays: 0 on the beat, 1 sits still, 2 a beat late, 3 too soon, 4 the wrong guard.
fn bots(r: &Run, modes: &[u32], t: u32) -> Keys {
    let Some(s) = r.latch.as_ref().filter(|s| s.left > 0 && s.count == 0) else { return Keys::new() };
    s.duels
        .iter()
        .filter_map(|(&id, d)| {
            let mode = modes[(id as usize - 1).min(modes.len() - 1)];
            let early = 1 + id % 3;
            Some((
                id,
                match mode {
                    0 => d.bot_input(early),
                    1 => return None,
                    2 => d.late_bot_input(r.tune.latch_recover),
                    3 => d.bot_input(r.tune.latch_window + 6 + id % 4),
                    5 => d.bot_input(1),
                    _ => Input { move_y: -d.height, ..d.bot_input(early) },
                },
            ))
            .filter(|_| mode != 1 || t % 7 == 0)
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
    let s = r.latch.as_ref().unwrap();
    let clock = s.elapsed(r.tick);
    let ended = if s.left == 0 { (r.tick as u32).saturating_sub(s.done) } else { 0 };
    let mut yous = 0;
    let leads = s.show.calls.iter().any(|c| c.who == id && c.cry == Cry::Lead && c.age(clock) < 80);
    let wins = s.left == 0 && s.won && s.order.first() == Some(&id);
    for line in &said.lines {
        let t = line.text.as_str();
        if !(t == "YOU" || t.starts_with("YOU ") || t.ends_with(" YOU") || t.contains(" YOU ")) {
            continue;
        }
        match t {
            "YOU" if line.on == (384, 224) => yous += 1,
            // The podium's own name for a winner or a runner-up, on a picture of its own.
            "YOU" => assert!(s.left == 0, "{what} seat {id}: a spare YOU in play"),
            "YOU LEAD" => assert!(leads, "{what} seat {id}: YOU LEAD for a knight who is not leading"),
            "YOU WIN" => assert!(wins, "{what} seat {id}: YOU WIN for another's win"),
            // A line of the table under the podium: its place, who, and their counters.
            other if other.contains(" YOU ") => {
                let place: usize = other.split(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0);
                assert_eq!(s.order.iter().position(|&w| w == id).map(|p| p + 1), Some(place), "{what} seat {id}: {other:?} is not this seat's line");
            }
            other => panic!("{what} seat {id}: {other:?} puts YOU somewhere it should not be"),
        }
    }
    assert!(yous <= 1, "{what} seat {id}: YOU drawn {yous} times");
    // The words the kit draws are pictures; the plan of them is a pure function, read here.
    if let Some(b) = moments::banner(r, s, id, Some(id), clock, ended) {
        for text in [&b.title, &b.sub] {
            if text.split(' ').any(|w| w == "YOU") {
                assert!((text == "YOU LEAD" && leads) || (text == "YOU WIN" && wins), "{what} seat {id}: banner {text:?} puts YOU where it should not be");
            }
        }
    }
    // And for any other seat looking, a banner about this seat never says YOU.
    for other in s.duels.keys().copied().filter(|&o| o != id) {
        if let Some(b) = moments::banner(r, s, other, Some(other), clock, ended) {
            assert!(
                !(b.title == "YOU LEAD" && !s.show.calls.iter().any(|c| c.who == other && c.cry == Cry::Lead)),
                "{what}: seat {other} told YOU LEAD without leading"
            );
        }
    }
}

/// The words of a seat are never `YOU` for another seat, in a card or a toast or on the podium.
#[test]
fn you_names_only_the_seat_looking() {
    let r = table(6);
    for me in 1..=6 {
        for who in 1..=6 {
            let tag = hud::tag(&r, who, 8);
            assert!(tag != "YOU", "card tag of seat {who} is YOU");
            let said = hud::called(&r, who, Some(me), 18);
            assert_eq!(said == "YOU", who == me, "seat {me} calls seat {who} {said}");
        }
        assert_ne!(hud::called(&r, me, None, 18), "YOU", "a watcher is nobody");
    }
}

fn walk(seats: u32, modes: &[u32], what: &str, extra: usize) {
    let mut r = table(seats);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut t = 0u32;
    let mut after = 0;
    loop {
        r.step(&bots(&r, modes, t));
        t += 1;
        let s = r.latch.as_ref().unwrap();
        let sampled = if t % 3 == 0 { vec![1 + t / 3 % seats] } else { vec![] };
        let all = t % 45 == 0 || s.left == 0 && t % 4 == 0;
        for id in if all { (1..=seats).collect() } else { sampled } {
            assert_clear(&r, id, &format!("{what}, {seats} seats, tick {t}"));
            for (w, _) in boxes(&r, id).0 {
                seen.insert(w);
            }
        }
        assert!(r.latch.as_ref().unwrap().valid(), "{what}, {seats} seats, tick {t}: the session is valid");
        if r.latch.as_ref().unwrap().left == 0 {
            after += 1;
            if after > 140 + extra {
                break;
            }
        }
        assert!(t < COUNT + LENGTH + 400, "{what}, {seats} seats: the round ended");
    }
    for want in ["<digit>", "<go>", "<banner>", "CLASP", "E AGAIN"] {
        assert!(seen.contains(want), "{what}, {seats} seats: never saw {want}: {seen:?}");
    }
}

#[test]
fn the_words_never_meet_and_every_prompt_sits_inside_the_screen_at_one_to_six_seats() {
    for seats in 1..=6u32 {
        // Quick hands: somebody breaks Clasp. Mixed hands: late, early, wrong and idle knights fold their
        // guards and the clock runs down.
        walk(seats, &[0, 0, 0, 0, 0, 0], "quick", 0);
        walk(seats, &[0, 1, 2, 3, 4, 0], "mixed", 0);
        walk(seats, &[1, 4, 3, 1, 2, 1], "dismal", 0);
    }
}

#[test]
fn a_storm_of_marks_calls_and_long_names_still_leaves_every_word_clear() {
    let mut r = table(6);
    {
        let s = r.latch.as_mut().unwrap();
        s.count = 0;
        s.left -= 600;
        let now = s.elapsed(r.tick);
        for (&id, d) in s.duels.iter_mut() {
            d.counters = (id % 3) as u8 + 1;
            d.chain = d.counters;
            d.beat = Beat::Tell;
            d.ticks = 4;
        }
        for who in 1..=6u32 {
            s.show.counter(who, 2, 2, 1, who, now.saturating_sub(who), &[1, 2, 3, 4, 5, 6]);
            s.show.miss(who, crate::rules::latch_show::Why::Wrong, 2, now);
        }
        s.show.standings(now, &[(1, 1), (2, 2), (3, 0), (4, 0), (5, 0), (6, 0)]);
        s.show.standings(now, &[(1, 1), (2, 2), (3, 2), (4, 0), (5, 3), (6, 0)]);
        assert!(s.valid());
    }
    for id in 1..=6 {
        assert_clear(&r, id, "storm");
    }
    // The end screen with six seats and the longest names, at every beat of it.
    let s = r.latch.as_mut().unwrap();
    s.left = 0;
    s.won = true;
    s.done = r.tick as u32;
    s.ended = 900;
    s.win_at = Some(880);
    s.to_beat = 1000;
    s.order = (1..=6).collect();
    for d in s.duels.values_mut() {
        d.counters = 1;
    }
    s.duels.get_mut(&1).unwrap().counters = 3;
    let done = r.tick;
    for tick in (0..260u64).step_by(4) {
        r.tick = done + tick;
        for id in 1..=6 {
            assert_clear(&r, id, &format!("end screen {tick}"));
        }
    }
}

#[test]
fn every_screen_of_a_round_draws_the_same_on_the_far_side_of_the_wire() {
    let mut r = table(4);
    for t in 0..260u32 {
        r.step(&bots(&r, &[0, 2, 0, 4], t));
        if t % 13 == 0 {
            let copy = r.wire_copy();
            assert_eq!(copy.latch, r.latch, "the show survives the wire at tick {t}");
            for id in 1..=4 {
                assert!(scene::layers(&copy, Some(id)).0.content_hash() == scene::layers(&r, Some(id)).0.content_hash(), "seat {id} tick {t}");
            }
        }
    }
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

/// Skip `n` steps unseen with `modes`.
fn skip(r: &mut Run, modes: &[u32], n: u32) {
    for t in 0..n {
        r.step(&bots(r, modes, t));
    }
}

#[test]
#[ignore = "writes the proof reels"]
fn write_latch_reel() {
    let Some(root) = std::env::var_os("ANGEL_LATCH_REEL") else { return };
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let quiet = |_: &Run, _: u32| Keys::new();

    if reel_only("before") {
        // The screen as it was, on the same table and the same bots, for the comparison.
        BEFORE.with(|b| b.set(true));
        let mut r = table(4);
        window(&root, "before", &mut r, COUNT, 300, |r, t| bots(r, &[0, 0, 0, 0], t));
        BEFORE.with(|b| b.set(false));
    }
    if reel_only("start") {
        let mut r = table(4);
        window(&root, "start", &mut r, 0, 150, |r, t| bots(r, &[0, 0, 0, 0], t));
    }
    if reel_only("counter") {
        let mut r = table(4);
        skip(&mut r, &[0, 0, 0, 0], COUNT + 40);
        window(&root, "counter", &mut r, 0, 120, |r, t| bots(r, &[0, 2, 0, 0], t + 100));
    }
    if reel_only("chain") {
        let mut r = table(4);
        skip(&mut r, &[0, 0, 0, 0], COUNT + 40);
        window(&root, "chain", &mut r, 0, 180, |r, t| bots(r, &[0, 0, 4, 0], t + 100));
    }
    if reel_only("miss") {
        let mut r = table(4);
        skip(&mut r, &[0, 0, 0, 0], COUNT + 20);
        window(&root, "miss", &mut r, 0, 150, |r, t| bots(r, &[4, 0, 0, 0], t + 100));
    }
    if reel_only("finish") {
        let mut r = table(4);
        {
            let s = r.latch.as_mut().unwrap();
            s.to_beat = 8 * 30;
        }
        window(&root, "finish", &mut r, 0, 560, |r, t| bots(r, &[0, 0, 0, 0], t));
    }
    if reel_only("win") {
        // Seat one wins: perfect hands against late, wrong-guard and idle ones.
        let mut r = table(4);
        window(&root, "win", &mut r, 0, 520, |r, t| bots(r, &[5, 2, 4, 1], t));
    }
    if reel_only("bell") {
        let mut r = table(4);
        {
            let s = r.latch.as_mut().unwrap();
            s.left = 5 * 30 + 20;
            s.count = 0;
        }
        window(&root, "bell", &mut r, 0, 360, |r, t| bots(r, &[1, 2, 4, 1], t));
    }
    if reel_only("solo") {
        let mut r = table(1);
        skip(&mut r, &[0], COUNT + 60);
        window(&root, "solo", &mut r, 0, 120, |r, t| bots(r, &[0], t));
    }
    if reel_only("six") {
        let mut r = table(6);
        skip(&mut r, &[0, 0, 0, 0, 0, 0], COUNT + 80);
        window(&root, "six", &mut r, 0, 150, |r, t| bots(r, &[0, 2, 0, 4, 0, 3], t));
    }
    let _ = quiet;
}

/// Where a frame's time goes, over a bot's round: the picture as it is, five draws of each frame and
/// the quickest taken, the box being what it is.
#[test]
#[ignore = "measures the picture's cost"]
fn latch_frame_time() {
    use std::time::Instant;
    let mut r = table(4);
    let mut total = [0u128; 2];
    let mut worst = [0u128; 2];
    let mut n = 0u128;
    for t in 0..900u32 {
        r.step(&bots(&r, &[0, 2, 0, 4], t));
        if t < 20 || t % 3 != 0 {
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
    let mut six = table(6);
    for t in 0..400u32 {
        six.step(&bots(&six, &[0, 2, 0, 4, 0, 3], t));
    }
    let mut end = table(4);
    for t in 0..120u32 {
        end.step(&bots(&end, &[0], t));
    }
    {
        let s = end.latch.as_mut().unwrap();
        s.left = 0;
        s.won = true;
        s.order = s.duels.keys().copied().collect();
        s.duels.get_mut(&1).unwrap().counters = 3;
        s.win_at = Some(900);
        s.ended = 900;
        s.done = end.tick as u32;
    }
    let done = end.tick;
    for (what, at) in [("six seats", 0u64), ("clasp coming apart", 12), ("podium", 140)] {
        let run = if what == "six seats" { &mut six } else { &mut end };
        if what != "six seats" {
            run.tick = done + at;
        }
        let mut cost = [0u128; 2];
        for (k, old) in [true, false].into_iter().enumerate() {
            BEFORE.with(|b| b.set(old));
            cost[k] = (0..5)
                .map(|_| {
                    let at = Instant::now();
                    let _ = frame_for(run, 384, 224, Some(1));
                    at.elapsed().as_micros()
                })
                .min()
                .unwrap();
        }
        BEFORE.with(|b| b.set(false));
        println!("latch {what}: before {} us, after {} us", cost[0], cost[1]);
    }
    let mean = |k: usize| total[k] / n.max(1);
    println!("latch frame over {n} frames: before mean {} us worst {} us; after mean {} us worst {} us; ratio {:.2}", mean(0), worst[0], mean(1), worst[1], mean(1) as f64 / mean(0).max(1) as f64);
}

#[allow(dead_code)]
fn unused(_: Kind) {}
