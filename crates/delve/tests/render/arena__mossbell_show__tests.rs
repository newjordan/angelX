//! Mossbell's show: the words on every seat's screen over whole rounds, the
//! pictures of its moments, and the reels the proof is cut from.
//!
//!   ANGEL_MOSSBELL_REEL=<dir> [REEL_ONLY=start,ride] \
//!     cargo test -p angel-delve --lib write_mossbell_reel -- --ignored --nocapture
use super::*;
use super::claims::{self, Claim, inside, meets};
use crate::rules::mossbell::{self as rules, INTRO, LENGTH};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;
use std::path::PathBuf;

type Keys = BTreeMap<u32, Input>;

/// The names a table of up to six carries: a clean name, the default `You`, an empty name,
/// a long name and two more.
const NAMES: [&str; 6] = ["Gawain", "You", "Dara", "", "Alexandrina", "Brienne"];

/// A table of `seats` knights, the round open and waiting out its title.
fn table(seats: u32) -> Run {
    let mut r = Run::mossbell_for_test();
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
    assert!(r.mossbell.is_some(), "the table did not open");
    r
}

/// A knight who plays: seat one plays true, the others miss a flip now and then and nudge when
/// they should not, each in their own way, so no two tables are the same.
fn hands(r: &Run) -> Keys {
    let Some(s) = r.mossbell.as_ref() else { return Keys::new() };
    let luck = |id: u32, bucket: u64| {
        (u64::from(id).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ bucket.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)).rotate_left(23) % 1000
    };
    s.tables
        .iter()
        .map(|(&id, t)| {
            let mut input = t.bot_input();
            if id > 1 {
                if luck(id, r.tick / 4) < 40 + u64::from(id) * 12 {
                    input = Input::default();
                } else if luck(id + 9, r.tick / 3) < 35 + u64::from(id) * 8 {
                    input.move_y = -1;
                }
            }
            (id, input)
        })
        .collect()
}

fn frame(r: &Run, seat: u32) -> Img {
    super::super::frame_for(r, NATIVE_W, NATIVE_H, Some(seat))
}

/// The round played through and a while of the podium: calls `each` before every tick.
fn through(r: &mut Run, mut each: impl FnMut(&Run)) {
    for _ in 0..LENGTH + INTRO + 700 {
        each(r);
        let k = hands(r);
        r.step(&k);
    }
}

/// How the test spells a knight's name from the viewer's seat.
fn spelled(r: &Run, id: u32, me: u32) -> String {
    if id == me {
        return "YOU".into();
    }
    let n = r.players[&id].name.trim().to_uppercase();
    if n.is_empty() || n == "YOU" { format!("KNIGHT {id}") } else { n }
}

/// The three rules of the words, for every claim on `me`'s screen: nothing meets, everything is
/// in frame, and `YOU` is only the viewer.
fn check(r: &Run, me: u32, what: &str) {
    let s = r.mossbell.as_ref().unwrap();
    let now = r.tick as u32;
    let plan = if s.left == 0 && s.show.since_end(now).is_some_and(|e| e >= claims::LIVE) {
        claims::finish(r, me, s.show.since_end(now).unwrap() - claims::LIVE)
    } else {
        claims::plan(r, me)
    };
    let claims = plan.claims();
    for (i, a) in claims.iter().enumerate() {
        assert!(inside(a.rect), "{what}: seat {me}: {:?} leaves the frame: {:?}", a.text, a.rect);
        for b in &claims[i + 1..] {
            assert!(!meets(a.rect, b.rect), "{what}: seat {me}: {:?} {:?} meets {:?} {:?}", a.text, a.rect, b.text, b.rect);
        }
        match a.who {
            Some(w) if w == me => assert!(a.text.contains("YOU"), "{what}: seat {me}: their own seat is YOU, not {:?}", a.text),
            Some(w) if s.tables.contains_key(&w) => {
                let want = spelled(r, w, me);
                let head: String = want.chars().take(6).collect();
                assert!(a.text.contains(&head), "{what}: seat {me}: knight {w} is {want:?}, not {:?}", a.text);
            }
            _ => {}
        }
        // The word YOU stands for the viewer only: on another knight's claim it never shows.
        if a.who.is_some_and(|w| w != me) {
            assert!(!a.text.split_whitespace().any(|t| t == "YOU"), "{what}: seat {me}: YOU on another knight: {:?}", a.text);
        }
    }
}

#[test]
fn the_words_never_meet_stay_in_frame_and_you_is_only_you() {
    for seats in 1..=6u32 {
        let mut r = table(seats);
        let mut seen_kinds = std::collections::BTreeSet::new();
        through(&mut r, |r| {
            for me in 1..=seats {
                check(r, me, &format!("{seats} seats, tick {}", r.tick));
            }
            if let Some(s) = r.mossbell.as_ref() {
                if let Some(c) = claims::plan(r, 1).bigs.first() {
                    seen_kinds.insert(c.0.what);
                }
                let _ = s;
            }
        });
        assert!(seen_kinds.len() >= 3, "{seats} seats: the run showed too few moments: {seen_kinds:?}");
    }
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
                let boxes: Vec<_> = super::labels(r, Some(me))
                    .into_iter()
                    .map(|(x, y, t)| ((x / 4 * 4, y / 8 * 8, t.chars().count() as i32 * 4, 8), t))
                    .collect();
                for (i, (a, ta)) in boxes.iter().enumerate() {
                    assert!(inside(*a), "{seats} seats, seat {me}: {ta:?} leaves the frame");
                    for (b, tb) in &boxes[i + 1..] {
                        assert!(!meets(*a, *b), "{seats} seats, seat {me}, tick {}: {ta:?} meets {tb:?}", r.tick);
                    }
                }
            }
        });
    }
}

#[test]
fn what_is_drawn_is_what_the_plan_claims() {
    use crate::render::overworld::ink::{said, text_width};
    for seats in [1u32, 3, 6] {
        let mut r = table(seats);
        let mut at = 0u32;
        through(&mut r, |r| {
            at += 1;
            if at % 37 != 5 {
                return;
            }
            for me in 1..=seats {
                let (_, said) = said::read(|| frame(r, me));
                let s = r.mossbell.as_ref().unwrap();
                let now = r.tick as u32;
                let plan = if s.left == 0 && s.show.since_end(now).is_some_and(|e| e >= claims::LIVE) {
                    claims::finish(r, me, s.show.since_end(now).unwrap() - claims::LIVE)
                } else {
                    claims::plan(r, me)
                };
                let claimed: Vec<Claim> = plan.claims();
                for line in said.lines.iter().filter(|l| l.on == (NATIVE_W, NATIVE_H)) {
                    let rect = (line.at.0, line.at.1, text_width(&line.text), 7);
                    assert!(inside(rect), "seat {me}: {:?} is drawn off the frame at {:?}", line.text, line.at);
                    assert!(
                        claimed.iter().any(|c| c.text == line.text && c.rect.0 == rect.0 && c.rect.1 == rect.1),
                        "seat {me}, tick {}: {:?} at {:?} is drawn but nobody claimed it",
                        r.tick,
                        line.text,
                        line.at
                    );
                }
            }
        });
    }
}

#[test]
fn the_default_name_is_never_you_on_a_rivals_screen() {
    let mut r = table(3);
    // Seat two carries the default `You`; seat one's screen shows KNIGHT 2.
    for _ in 0..INTRO + 5 * 30 {
        let k = hands(&r);
        r.step(&k);
    }
    let plan = claims::plan(&r, 1);
    let names: Vec<_> = plan.words.iter().filter(|w| w.what == "card name").map(|w| (w.who.unwrap(), w.text.clone())).collect();
    assert!(names.contains(&(1, "YOU".into())));
    assert!(names.contains(&(2, "KNIGHT 2".into())), "{names:?}");
    assert!(names.contains(&(3, "DARA".into())), "{names:?}");
    let plan = claims::plan(&r, 2);
    let names: Vec<_> = plan.words.iter().filter(|w| w.what == "card name").map(|w| (w.who.unwrap(), w.text.clone())).collect();
    assert!(names.contains(&(2, "YOU".into())));
    assert!(names.contains(&(1, "GAWAIN".into())), "{names:?}");
}

#[test]
fn the_moments_each_look_their_own_way() {
    let mut r = table(3);
    let mut seen = std::collections::BTreeSet::new();
    // The title, then each beat of the countdown, then GO and the play.
    for t in 0..INTRO + 40 {
        if matches!(t, 3 | 12 | 30 | 44 | 56 | 68) || t == INTRO + 6 || t == INTRO + 34 {
            assert!(seen.insert(frame(&r, 1).content_hash()), "tick {t} looks like another");
        }
        let k = hands(&r);
        r.step(&k);
    }
}

#[test]
fn a_round_has_a_bell_a_fall_a_jackpot_and_a_podium() {
    let mut r = table(2);
    let (mut bell, mut fall, mut jackpot, mut podium, mut last) = (false, false, false, false, false);
    through(&mut r, |r| {
        let Some(s) = r.mossbell.as_ref() else { return };
        let now = r.tick as u32;
        use crate::rules::mossbell_show::Cry;
        bell |= s.show.shouts.iter().any(|c| c.cry == Cry::Bell);
        fall |= s.show.shouts.iter().any(|c| c.cry == Cry::KeeperDown);
        jackpot |= s.show.shouts.iter().any(|c| c.cry == Cry::Jackpot);
        last |= s.show.shouts.iter().any(|c| c.cry == Cry::Last);
        podium |= s.left == 0 && s.show.since_end(now).is_some_and(|e| e > claims::LIVE + 60);
    });
    assert!(bell && fall && jackpot && last && podium, "bell {bell} fall {fall} jackpot {jackpot} last {last} podium {podium}");
}

#[test]
fn the_frame_is_as_cheap_as_it_was() {
    // A guard, not a bench: a busy frame of six seats stays well inside a frame's budget.
    let mut r = table(6);
    for _ in 0..INTRO + 200 {
        let k = hands(&r);
        r.step(&k);
    }
    let t = std::time::Instant::now();
    for _ in 0..5 {
        for me in 1..=6 {
            frame(&r, me);
        }
    }
    let each = t.elapsed().as_secs_f64() * 1000.0 / 30.0;
    assert!(each < 400.0, "a frame took {each:.1} ms even in a debug build");
}

// ─── the bench and the reels ────────────────────────────────────────────────

/// The cheapest of sixty frames of a busy table, in milliseconds, for the frame-time ledger.
///
///   cargo test -p angel-delve --lib bench_mossbell_frame --release -- --ignored --nocapture
#[test]
#[ignore = "a bench"]
fn bench_mossbell_frame() {
    let mut r = table(4);
    let mut best = f64::MAX;
    let mut sum = 0.0;
    let mut n = 0;
    for tick in 0..INTRO + 1500 {
        let k = hands(&r);
        r.step(&k);
        if tick > INTRO && tick % 25 == 0 {
            let t = std::time::Instant::now();
            let _ = frame(&r, 1);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            best = best.min(ms);
            sum += ms;
            n += 1;
        }
    }
    println!("mossbell frame: cheapest {best:.3} ms, mean {:.3} ms over {n}", sum / f64::from(n));
}

fn start_of(seats: u32) -> u64 {
    table(seats).tick
}

fn save(dir: &std::path::Path, n: u32, seats: &[u32], r: &Run) {
    let mut sheet = Img::black(NATIVE_W * seats.len() as i32, NATIVE_H);
    for (i, &seat) in seats.iter().enumerate() {
        sheet.stamp(&frame(r, seat), NATIVE_W * i as i32, 0);
    }
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Reels of whole rounds: each window of ticks is a folder of frames (30 per second), the seats
/// named side by side. `scripts/` turns them into GIFs.
#[test]
#[ignore = "writes the proof reels"]
fn write_mossbell_reel() {
    let Some(root) = std::env::var_os("ANGEL_MOSSBELL_REEL") else { return };
    let root = PathBuf::from(root);
    let only = std::env::var("REEL_ONLY").ok();
    let want = |name: &str| only.as_deref().is_none_or(|o| o.split(',').any(|x| x == name));
    // One pass to find where the moments fall; the second records them.
    let probe = |seats: u32| {
        let mut r = table(seats);
        let (mut fall, mut jackpot, mut end) = (None, None, None);
        for _ in 0..LENGTH + INTRO + 900 {
            let k = hands(&r);
            r.step(&k);
            let s = r.mossbell.as_ref().unwrap();
            use crate::rules::mossbell_show::Cry;
            if fall.is_none() {
                fall = s.show.shouts.iter().find(|c| c.cry == Cry::KeeperDown).map(|c| (r.tick, u32::from(c.who)));
            }
            if jackpot.is_none() {
                jackpot = s.show.shouts.iter().find(|c| c.cry == Cry::Jackpot).map(|c| (r.tick, u32::from(c.who)));
            }
            if end.is_none() && s.left == 0 {
                end = Some(r.tick);
            }
        }
        (fall, jackpot, end, r.tick)
    };
    let (fall, jackpot, end, _) = probe(4);
    println!("four seats: fall {fall:?} jackpot {jackpot:?} end {end:?}");
    // The seat that felled the keeper, and the one who took the jackpot, are the ones to watch.
    let (fall_at, fall_who) = fall.unwrap_or((start_of(4) + 600, 1));
    let (jack_at, jack_who) = jackpot.unwrap_or((start_of(4) + 900, 1));
    let pair = |who: u32| if who == 1 { vec![1, 2] } else { vec![who, 1] };
    let start = start_of(4);
    let end = end.expect("a round ends");
    let windows: Vec<(&str, u32, u64, u64, Vec<u32>)> = vec![
        ("start", 4, start, start + u64::from(INTRO) + 100, vec![1, 2]),
        ("ride", 4, start + 330, start + 330 + 330, vec![1, 2]),
        ("bell", 4, start + u64::from(INTRO) + 14 * 30, start + u64::from(INTRO) + 22 * 30, vec![1, 2]),
        ("keeper", 4, fall_at.saturating_sub(40), fall_at + 90, pair(fall_who)),
        ("jackpot", 4, jack_at.saturating_sub(20), jack_at + 110, pair(jack_who)),
        ("last", 4, end.saturating_sub(7 * 30), end + 40, vec![1, 2]),
        ("finish", 4, end, end + 400, vec![1, 2]),
        ("six", 6, start, start + u64::from(INTRO) + 240, vec![1, 6]),
        ("six-finish", 6, end, end + 330, vec![1, 6]),
        ("solo", 1, start, start + u64::from(INTRO) + 200, vec![1]),
    ];
    for (name, seats, from, to, show) in windows {
        if !want(name) {
            continue;
        }
        let dir = root.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut r = table(seats);
        let mut n = 0;
        let mut guard = 0;
        while r.tick < to && guard < 6000 {
            guard += 1;
            if r.tick >= from && r.mossbell.is_some() {
                save(&dir, n, &show, &r);
                n += 1;
            }
            let k = hands(&r);
            r.step(&k);
        }
        println!("{name}: {n} frames");
    }
    let _ = rules::LENGTH;
}

/// Where a frame's time goes, piece by piece, in microseconds.
///
///   cargo test -p angel-delve --lib profile_mossbell_frame -- --ignored --nocapture
#[test]
#[ignore = "a profile"]
fn profile_mossbell_frame() {
    let mut r = table(4);
    for _ in 0..INTRO + 700 {
        let k = hands(&r);
        r.step(&k);
    }
    let s = r.mossbell.as_ref().unwrap();
    let me = 1;
    let now = r.tick as u32;
    let time = |name: &str, f: &mut dyn FnMut()| {
        let mut best = f64::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            for _ in 0..60 {
                f();
            }
            best = best.min(t.elapsed().as_secs_f64() * 1e6 / 60.0);
        }
        println!("{name:>16}: {best:8.1} us");
    };
    time("frame_for", &mut || {
        let _ = frame(&r, me);
    });
    time("room clone", &mut || {
        let _ = super::room().clone();
    });
    let plan = claims::plan(&r, me);
    time("plan", &mut || {
        let _ = claims::plan(&r, me);
    });
    let mut im = super::room().clone();
    time("cards", &mut || super::hud::cards(&mut im, &r, s, me, now));
    time("panel", &mut || super::hud::panel(&mut im, s, me, &plan, now));
    time("words", &mut || {
        for w in &plan.words {
            im.text(w.x, w.y, &w.text, w.ink);
        }
    });
    time("window new", &mut || {
        let _ = Img::window(NATIVE_W, NATIVE_H, super::TABLE_BOX);
    });
    let mut world = Img::window(NATIVE_W, NATIVE_H, super::TABLE_BOX);
    time("world stamp", &mut || im.stamp(&world, 0, 0));
    time("shafts", &mut || super::fx::shafts(&mut world, r.tick));
    time("ambient", &mut || super::fx::ambient(&mut world, r.tick));
    let b = &s.tables[&me].ball;
    time("lamps", &mut || super::fx::lamps(&mut world, b, r.tick));
    time("comet", &mut || super::fx::comet(&mut world, b, r.tick));
    time("moments", &mut || super::moments::draw(&mut im, &r, s, me, &plan, now));
    time("knight", &mut || super::knight(&mut im, me, false, false, false));
}
