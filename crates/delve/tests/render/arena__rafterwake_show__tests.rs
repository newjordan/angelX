//! Rafterwake's show: the words on every seat's screen over whole rounds, the pictures of its
//! moments, and the reels the proof is cut from.
//!
//!   ANGEL_RAFTERWAKE_REEL=<dir> [REEL_ONLY=start,climb] \
//!     cargo test -p angel-delve --lib write_rafterwake_reel -- --ignored --nocapture
use super::*;
use crate::render::overworld::ink::said;
use crate::rules::rafterwake::{self as rules, INTRO, LENGTH};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;
use std::path::PathBuf;

type Keys = BTreeMap<u32, Input>;

/// The names a table of up to six carries: a clean name, the default `You`, a short one, an empty
/// one, a long one and one with a number in it.
const NAMES: [&str; 6] = ["Gawain", "You", "Dara", "", "Alexandrina", "Bran 2"];

/// A table of `seats` knights, the round open and waiting out its title.
pub(super) fn table(seats: u32) -> Run {
    let mut r = Run::rafterwake_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    if seats == 1 {
        r.leave(2);
    }
    for id in 1..=seats {
        r.players.get_mut(&id).unwrap().name = NAMES[id as usize - 1].to_string();
    }
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert!(r.rafterwake.is_some(), "the round did not open");
    r
}

/// A knight who plays: seat one plays true, the others hesitate now and then, each in their own
/// way, so no two towers are the same.
pub(super) fn hands(r: &Run) -> Keys {
    let Some(s) = r.rafterwake.as_ref() else { return Keys::new() };
    let luck = |id: u32, bucket: u64| {
        (u64::from(id).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ bucket.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)).rotate_left(23) % 1000
    };
    s.lanes
        .iter()
        .map(|(&id, d)| {
            let mut input = d.bot_input(LENGTH - s.left);
            if id > 1 && luck(id, r.tick / 5) < 70 + u64::from(id) * 25 {
                input = Input::default();
            }
            (id, input)
        })
        .collect()
}

fn frame(r: &Run, seat: u32) -> Img {
    super::super::frame_for(r, NATIVE_W, NATIVE_H, Some(seat))
}

/// The round played through and a while of the finish: calls `each` before every tick.
fn through(r: &mut Run, mut each: impl FnMut(&Run)) {
    for _ in 0..LENGTH + INTRO + 500 {
        each(r);
        let k = hands(r);
        r.step(&k);
    }
}

fn reel_dir() -> Option<PathBuf> {
    std::env::var_os("ANGEL_RAFTERWAKE_REEL").map(PathBuf::from)
}

fn want(name: &str) -> bool {
    std::env::var("REEL_ONLY").map_or(true, |o| o.split(',').any(|x| x == name))
}

fn save(dir: &std::path::Path, n: u32, seats: &[u32], r: &Run) {
    let mut sheet = Img::black(384 * seats.len() as i32, 224);
    for (i, &seat) in seats.iter().enumerate() {
        if r.players.contains_key(&seat) {
            sheet.stamp(&frame(r, seat), 384 * i as i32, 0);
        }
    }
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Play a table of `seats` and write the frames of the tick window `[from, to)` (30 fps).
fn reel(root: &std::path::Path, name: &str, seats: u32, show: &[u32], from: u32, to: u32) {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = table(seats);
    let mut n = 0;
    for t in 0..to {
        if t >= from {
            save(&dir, n, show, &r);
            n += 1;
        }
        let k = hands(&r);
        r.step(&k);
    }
    println!("{name}: {n} frames");
}

#[test]
#[ignore = "writes the reels the proof is cut from"]
fn write_rafterwake_reel() {
    let Some(root) = reel_dir() else { return };
    let i = INTRO;
    for (name, seats, show, from, to) in [
        ("start", 4, vec![1, 2], 0, i + 70),
        ("climb", 4, vec![1, 2], i + 100, i + 260),
        ("bell", 4, vec![1, 3], i + 170, i + 330),
        ("keeper", 4, vec![1, 2], i + 560, i + 760),
        ("finish", 4, vec![1, 2], i + 1100, i + 1500),
        ("six", 6, vec![1, 6], i + 150, i + 330),
        ("solo", 1, vec![1], i + 100, i + 280),
    ] {
        if want(name) {
            reel(&root, name, seats, &show, from, to);
        }
    }
}

#[test]
#[ignore = "prints what the table does with its time"]
fn rafterwake_timeline() {
    let mut r = table(4);
    let mut last = String::new();
    through(&mut r, |r| {
        let s = r.rafterwake.as_ref().unwrap();
        let line = format!(
            "go {} left {} bells {} lead {:?} keeper {:?} slain {:?} heights {:?} fin {:?}",
            s.go,
            s.left,
            s.bells,
            s.show.leader,
            s.show.first_keeper,
            s.show.first_slain,
            s.lanes.values().map(|d| d.checkpoint).collect::<Vec<_>>(),
            s.lanes.values().map(|d| d.finish).collect::<Vec<_>>()
        );
        if r.tick % 60 == 0 && line != last {
            println!("t{} {line}", r.tick);
            last = line;
        }
    });
    let _ = said::read(|| ());
    let _ = rules::INTRO;
}

// ─── the words ───────────────────────────────────────────────────────────────

use super::hud::{self, View, inside, meets};
use super::moments::Plan;
use super::stage;

/// How the test spells a knight's name from the viewer's seat.
fn spelled(r: &Run, id: u32, me: u32) -> String {
    if id == me {
        return "YOU".into();
    }
    let n = r.players[&id].name.trim().to_uppercase();
    if n.is_empty() || n == "YOU" { format!("KNIGHT {id}") } else { n }
}

/// Kinds of word that name a knight.
const NAMES_A_KNIGHT: [&str; 5] = ["card name", "tag", "you", "podium name", "ledger name"];

/// The plan of `me`'s screen now.
fn plan(r: &Run, me: u32) -> Plan {
    stage::plan_of(&View::new(r, Some(me)))
}

/// The rules of the words, for every claim on `me`'s screen: nothing meets, everything is in
/// frame, and `YOU` is only the viewer.
fn check(r: &Run, me: u32, what: &str) -> Plan {
    let plan = plan(r, me);
    let claims = plan.claims();
    for (i, (an, a, _)) in claims.iter().enumerate() {
        assert!(inside(*a), "{what}: seat {me}: {an} leaves the frame: {a:?}");
        for (bn, b, _) in &claims[i + 1..] {
            assert!(!meets(*a, *b), "{what}: seat {me}: {an} {a:?} meets {bn} {b:?}");
        }
    }
    for w in &plan.words {
        if let Some(id) = w.who {
            if NAMES_A_KNIGHT.contains(&w.kind) {
                let want = spelled(r, id, me);
                if id == me {
                    assert_eq!(w.text, "YOU", "{what}: seat {me}: their own seat is YOU, not {:?}", w.text);
                } else {
                    let head: String = want.chars().take(w.text.chars().count()).collect();
                    assert_eq!(w.text, head, "{what}: seat {me}: knight {id} is {want:?}, not {:?}", w.text);
                }
            }
        }
        // The word YOU stands for the viewer only.
        if w.who.is_some_and(|id| id != me) {
            assert!(!w.text.split_whitespace().any(|t| t == "YOU"), "{what}: seat {me}: YOU on another knight: {:?}", w.text);
        }
    }
    if let Some(b) = &plan.banner {
        let says_you = b.title.split_whitespace().any(|t| t == "YOU") || b.sub.split_whitespace().any(|t| t == "YOU");
        if says_you {
            assert_eq!(b.who, Some(me), "{what}: seat {me}: a banner says YOU for another knight: {:?}", b.title);
        }
        if let Some(w) = b.who.filter(|&w| w != me) {
            let name = spelled(r, w, me);
            let head: String = name.chars().take(8).collect();
            assert!(b.title.contains(&head) || !NAMES_BANNERS.iter().any(|k| b.title.ends_with(k)), "{what}: seat {me}: {:?} is not knight {w} ({name})", b.title);
        }
    }
    plan
}

/// The endings of banners that name a knight.
const NAMES_BANNERS: [&str; 5] = ["CLEARED", "AT KEEPER", "LEADS", "FELL", "WINS"];

/// Every line the pen wrote on the whole 384 by 224 screen is one of the claimed boxes.
fn check_pen(r: &Run, me: u32, what: &str) {
    let (_, said) = said::read(|| stage::frame(r, Some(me)));
    let plan = plan(r, me);
    let claimed: Vec<_> = plan
        .words
        .iter()
        .filter(|w| matches!(w.face, hud::Face::Pen(_)))
        .map(|w| w.rect())
        .collect();
    for line in said.lines.iter().filter(|l| l.on == (384, 224)) {
        let rect = (line.at.0, line.at.1, crate::render::overworld::ink::text_width(&line.text), 7);
        assert!(inside(rect), "{what}: seat {me}: {:?} leaves the screen", line.text);
        assert!(claimed.contains(&rect), "{what}: seat {me}: {:?} at {rect:?} is not a claimed box", line.text);
    }
}

#[test]
fn the_words_never_meet_stay_in_frame_and_you_is_only_you() {
    for seats in 1..=6u32 {
        let mut r = table(seats);
        let mut kinds = std::collections::BTreeSet::new();
        let mut at = 0u32;
        through(&mut r, |r| {
            at += 1;
            for me in 1..=seats {
                let p = check(r, me, &format!("{seats} seats, tick {}", r.tick));
                if let Some(b) = &p.banner {
                    kinds.insert(b.title.split(' ').next().unwrap_or("").to_string());
                }
                if !p.pops.is_empty() {
                    kinds.insert("pop".into());
                }
                if p.count.is_some() {
                    kinds.insert("count".into());
                }
                if p.words.iter().any(|w| w.kind == "tag") {
                    kinds.insert("tag".into());
                }
            }
            if at % 13 == 0 {
                check_pen(r, 1 + at / 13 % seats, &format!("{seats} seats, tick {}", r.tick));
            }
        });
        assert!(kinds.len() >= 6, "{seats} seats: the run showed too few moments: {kinds:?}");
    }
}

#[test]
fn the_title_the_count_and_the_go_show_on_every_seat() {
    let mut r = table(4);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..INTRO + 40 {
        for me in 1..=4 {
            let p = check(&r, me, "the count");
            if let Some(c) = &p.count {
                seen.insert(c.label);
            }
            if p.banner.as_ref().is_some_and(|b| b.title == "RAFTERWAKE") {
                seen.insert("title");
            }
        }
        r.step(&Keys::new());
    }
    assert_eq!(seen, ["1", "2", "3", "GO", "title"].into_iter().collect());
}

/// The cost of a frame, the old picture against the new, over a four-seat round.
#[test]
#[ignore = "measures the picture's cost"]
fn bench_rafterwake_frame() {
    use std::time::Instant;
    let mut r = table(4);
    let (mut old, mut new) = (Vec::new(), Vec::new());
    for t in 0..INTRO + 900 {
        let k = hands(&r);
        r.step(&k);
        if t > INTRO + 40 && t % 6 == 0 {
            let at = Instant::now();
            let _ = super::draw(&r, Some(1), true);
            old.push(at.elapsed().as_micros());
            let at = Instant::now();
            let _ = stage::frame(&r, Some(1));
            new.push(at.elapsed().as_micros());
        }
    }
    old.sort();
    new.sort();
    let mean = |v: &[u128]| v.iter().sum::<u128>() / v.len() as u128;
    println!("old: min {} us, median {} us, mean {} us", old[0], old[old.len() / 2], mean(&old));
    println!("new: min {} us, median {} us, mean {} us", new[0], new[new.len() / 2], mean(&new));
}

#[test]
fn the_terminal_words_stay_clear_too() {
    // The cell painter sets each label in four-pixel cells, eight high.
    for seats in 1..=6u32 {
        let mut r = table(seats);
        let mut at = 0;
        through(&mut r, |r| {
            at += 1;
            if at % 29 != 3 {
                return;
            }
            for me in 1..=seats {
                let words = super::labels(r, Some(me));
                let boxes: Vec<_> = words
                    .iter()
                    .map(|(x, y, t)| ((x / 4 * 4, y / 8 * 8, t.chars().count() as i32 * 4, 8), t.clone()))
                    .collect();
                for (i, (a, ta)) in boxes.iter().enumerate() {
                    assert!(inside(*a), "{seats} seats, seat {me}, tick {}: {ta:?} leaves the frame", r.tick);
                    for (b, tb) in &boxes[i + 1..] {
                        assert!(!meets(*a, *b), "{seats} seats, seat {me}, tick {}: {ta:?} meets {tb:?}", r.tick);
                    }
                }
                let yous = words.iter().filter(|(_, _, t)| t.split_whitespace().any(|w| w == "YOU")).count();
                assert!(yous <= 1, "{seats} seats, seat {me}: YOU more than once: {words:?}");
                // A rival that carries the default name is never called YOU.
                for (_, _, t) in &words {
                    assert!(t.split_whitespace().filter(|w| *w == "YOU").count() <= 1);
                }
            }
        });
    }
}

#[test]
fn the_default_name_is_never_you_on_a_rivals_screen() {
    let mut r = table(3);
    // Seat two carries the default `You`: seat one's screen calls it KNIGHT 2, its own screen YOU.
    for _ in 0..INTRO + 40 {
        let k = hands(&r);
        r.step(&k);
    }
    let one = plan(&r, 1);
    assert!(one.words.iter().any(|w| w.kind == "card name" && w.text == "KNIGHT 2"), "{:?}", one.words.iter().map(|w| &w.text).collect::<Vec<_>>());
    assert_eq!(one.words.iter().filter(|w| w.kind == "card name" && w.text == "YOU").count(), 1);
    let two = plan(&r, 2);
    assert!(two.words.iter().any(|w| w.kind == "card name" && w.text == "YOU" && w.who == Some(2)));
    assert!(two.words.iter().all(|w| w.kind != "card name" || w.who == Some(2) || w.text != "YOU"));
}
