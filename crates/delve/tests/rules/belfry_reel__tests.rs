//! The Belfry Shoot's proof reels and frame-time bench: the real `Run`
//! played through by gunner bots (each shoots the nearest bat and weaves
//! about a post of their own), recorded on seat one's own screen (and seat
//! two's) at every tick (30 fps) in windows: the opening, a flock, a streak,
//! a bite, a toll, and the finish with its podium. It changes no game code
//! and writes only under the directory it is given.
//!
//!   ANGEL_BELFRY_REEL=<dir> [REEL_ONLY=start,flock] cargo test -p angel-delve --lib write_belfry_reel -- --ignored --nocapture
//!   cargo test -p angel-delve --lib belfry_frame_time -- --ignored --nocapture
//!
//! `/work/artifacts/minigames-night-20261107/belfry_shoot/make_gifs.py` makes the GIFs from the frames.
use super::*;
use crate::render::overworld::{Img, arena::frame_for};
use crate::rules::belfry_show::Cry;
use crate::rules::folly::{BAT_TIME, TITLE};
use std::path::PathBuf;

pub(crate) type Keys = BTreeMap<u32, Input>;

/// The shoot with `seats` knights at the table, on a floor laid for them.
pub(crate) fn fresh_for(seats: u32) -> Run {
    let mut r = Run::new(41, 1, None);
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.descend_for_test();
    r.enemies.clear();
    for (id, name) in [(1, "Aldric"), (2, "Brienne"), (3, "Cedric"), (4, "Dara"), (5, "Edda"), (6, "Fenn")] {
        if let Some(h) = r.players.get_mut(&id) {
            h.name = name.into();
        }
    }
    r.pick_folly();
    for h in r.players.values_mut() {
        h.carried.add(Spoil::Gold, 50);
    }
    // Fortune's dare card is shown as a party arrives on a floor and lifts
    // long before they find a folly: the reel starts after it.
    r.dare = None;
    r.enter_for_test(r.folly_room.expect("a hall on floor two"));
    r
}

/// Knights who drink their potions: any under two fifths of their health is
/// mended, so a gunner bot lives out the minute (a bite still shows).
fn mend(r: &mut Run) {
    for h in r.players.values_mut() {
        if h.hp > 0 && h.hp * 5 < h.max_hp * 2 {
            h.hp = h.max_hp;
        }
    }
}

pub(crate) fn fresh() -> Run {
    fresh_for(4)
}

/// What a gunner presses: aim at the nearest bat and fire, drifting about a
/// post along the room's middle (each seat its own place along it).
pub(crate) fn keys(run: &Run) -> Keys {
    let Some(shoot) = run.bats.as_ref() else { return Keys::new() };
    if shoot.titled() || shoot.done {
        return Keys::new();
    }
    let n = run.players.len().max(1) as f32;
    let t = run.tick as f32;
    run.players
        .iter()
        .enumerate()
        .map(|(k, (&id, me))| {
            let post = (run.room().width() * (k as f32 + 1.0) / (n + 1.0), run.room().height() * 0.62);
            let drift = ((t / 23.0 + id as f32 * 1.7).sin() * 5.0, (t / 31.0 + id as f32).cos() * 2.5);
            let (gx, gy) = (post.0 + drift.0 - me.x, post.1 + drift.1 - me.y);
            let step = |v: f32| if v.abs() < 1.2 { 0 } else { v.signum() as i8 };
            let near = run.enemies.iter().min_by(|a, b| {
                (a.x - me.x).hypot(a.y - me.y).total_cmp(&(b.x - me.x).hypot(b.y - me.y))
            });
            let (aim_x, aim_y, fire) = match near {
                Some(e) => {
                    let (dx, dy) = (e.x - me.x, e.y - me.y);
                    let big = dx.abs().max(dy.abs()).max(0.01);
                    let s = |v: f32| if v.abs() < big * 0.4 { 0 } else { v.signum() as i8 };
                    (s(dx), s(dy), true)
                }
                None => (0, -1, false),
            };
            (id, Input { move_x: step(gx), move_y: step(gy), aim_x, aim_y, fire, ..Default::default() })
        })
        .collect()
}

/// The ticks that matter, found by playing the shoot through once unseen.
#[derive(Debug, Default)]
struct Marks {
    first_flock: Option<u32>,
    /// A run of three or more, the seat and its size.
    streaks: Vec<(u32, u32, u8)>,
    flocks: Vec<(u32, u32)>,
    bites: Vec<(u32, u32)>,
    tolls: Vec<u32>,
    lead: Option<(u32, u32)>,
    end: Option<u32>,
}

fn survey(seats: u32) -> Marks {
    let mut r = fresh_for(seats);
    let mut marks = Marks::default();
    let mut seen = std::collections::BTreeSet::new();
    for t in 0..4_000u32 {
        let k = keys(&r);
        mend(&mut r);
        r.step(&k);
        let Some(s) = r.bats.as_ref() else { break };
        if marks.first_flock.is_none() && !s.show.swoops.is_empty() {
            marks.first_flock = Some(t);
        }
        for h in s.show.shouts.iter() {
            if !seen.insert((h.born, u32::from(h.who), u32::from(h.cry.rank()))) {
                continue;
            }
            match h.cry {
                Cry::Chain(n) if n >= 3 => marks.streaks.push((t, u32::from(h.who), n)),
                Cry::Flock => marks.flocks.push((t, u32::from(h.who))),
                Cry::Lead if marks.lead.is_none() => marks.lead = Some((t, u32::from(h.who))),
                _ => {}
            }
        }
        for b in &s.show.bites {
            if seen.insert((b.born, u32::from(b.who), 99)) {
                marks.bites.push((t, u32::from(b.who)));
            }
        }
        for &tl in &s.show.tolls {
            if seen.insert((tl, 0, 98)) {
                marks.tolls.push(t);
            }
        }
        if s.done && marks.end.is_none() {
            marks.end = Some(t);
        }
        if marks.end.is_some_and(|e| t > e + 330) {
            break;
        }
    }
    marks
}

fn save(dir: &PathBuf, n: usize, sheet: &Img) {
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Play again, writing a frame of `seat`'s screen every tick in `[from, to)`.
fn record_for(root: &PathBuf, name: &str, seats: u32, from: u32, to: u32, seat: u32, also: Option<u32>) {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = fresh_for(seats);
    let mut n = 0;
    for t in 0..to {
        let k = keys(&r);
        mend(&mut r);
        r.step(&k);
        if t >= from {
            let mut sheet = Img::black(384 * if also.is_some() { 2 } else { 1 }, 224);
            sheet.stamp(&frame_for(&r, 384, 224, Some(seat)), 0, 0);
            if let Some(other) = also {
                sheet.stamp(&frame_for(&r, 384, 224, Some(other)), 384, 0);
            }
            save(&dir, n, &sheet);
            n += 1;
        }
    }
    println!("{name}: {n} frames from tick {from}, seat {seat}");
}

fn record(root: &PathBuf, name: &str, from: u32, to: u32, seat: u32, also: Option<u32>) {
    record_for(root, name, 4, from, to, seat, also);
}

#[test]
#[ignore = "writes the Belfry Shoot's proof reels"]
fn write_belfry_reel() {
    let Some(root) = std::env::var_os("ANGEL_BELFRY_REEL") else { return };
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let marks = survey(4);
    println!("{marks:?}");
    let only = std::env::var("REEL_ONLY").unwrap_or_default();
    let want = |n: &str| only.is_empty() || only.split(',').any(|x| x == n);
    if want("start") {
        record(&root, "start", 0, TITLE + 150, 1, Some(2));
    }
    if want("flock") && let Some(&(t, who)) = marks.flocks.get(2).or(marks.flocks.first()) {
        record(&root, "flock", t.saturating_sub(70), t + 60, if who == 0 { 1 } else { who }, None);
    }
    if want("streak") && let Some(&(t, who, _)) = marks.streaks.iter().max_by_key(|s| s.2) {
        record(&root, "streak", t.saturating_sub(70), t + 80, who, None);
    }
    if want("bite") && let Some(&(t, who)) = marks.bites.first() {
        record(&root, "bite", t.saturating_sub(40), t + 60, who, None);
    }
    if want("toll") && let Some(&t) = marks.tolls.get(2).or(marks.tolls.last()) {
        record(&root, "toll", t.saturating_sub(40), t + 80, 1, Some(2));
    }
    if want("solo") {
        record_for(&root, "solo", 1, 0, TITLE + 150, 1, None);
    }
    if want("six") {
        record_for(&root, "six", 6, 0, TITLE + 150, 6, None);
    }
    if want("last") {
        let t = TITLE + BAT_TIME - 6 * 30;
        record(&root, "last", t, t + 6 * 30, 1, Some(2));
    }
    if want("finish") && let Some(end) = marks.end {
        record(&root, "finish", end.saturating_sub(40), end + 330, 1, Some(2));
    }
}

/// The Belfry Shoot's frame time: four gunners play to the middle of the
/// minute, then seat one's screen is drawn twice a tick, as the belfry draws
/// it and as the room was drawn before it (`belfry_shoot::OFF`), so both
/// see the same game; and each part of the belfry is timed alone.
#[test]
#[ignore = "times the Belfry Shoot's frames"]
fn belfry_frame_time() {
    use crate::render::overworld::arena::{belfry_profile, belfry_set_off};
    use std::time::Instant;
    let mut r = fresh();
    for _ in 0..TITLE + 600 {
        let k = keys(&r);
        mend(&mut r);
        r.step(&k);
    }
    // A warm-up, so the scenery's bake is not counted.
    let _ = frame_for(&r, 384, 224, Some(1));
    belfry_set_off(true);
    let _ = frame_for(&r, 384, 224, Some(1));
    belfry_set_off(false);
    let (mut new, mut old) = (Vec::new(), Vec::new());
    let mut parts = [f64::MAX; 3];
    let mut sums = [0.0f64; 3];
    // Each frame is drawn several times and its quickest taken: the draw is
    // the same each time, so what is slower is the machine's other work.
    let best = |r: &Run, n: usize| {
        (0..n)
            .map(|_| {
                let t = Instant::now();
                let img = frame_for(r, 384, 224, Some(1));
                std::hint::black_box(&img);
                t.elapsed().as_secs_f64() * 1000.0
            })
            .fold(f64::MAX, f64::min)
    };
    for _ in 0..240 {
        let k = keys(&r);
        mend(&mut r);
        r.step(&k);
        new.push(best(&r, 5));
        let mut tick_parts = [f64::MAX; 3];
        for _ in 0..5 {
            for (i, (_, ms)) in belfry_profile(&r, Some(1)).iter().enumerate() {
                tick_parts[i] = tick_parts[i].min(*ms);
            }
        }
        for i in 0..3 {
            sums[i] += tick_parts[i] / 240.0;
            parts[i] = sums[i];
        }
        belfry_set_off(true);
        old.push(best(&r, 5));
        belfry_set_off(false);
    }
    let stats = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let slow = v[v.len() * 9 / 10..].iter().sum::<f64>() / (v.len() - v.len() * 9 / 10) as f64;
        (mean, v[v.len() / 2], slow)
    };
    let (n, o) = (stats(&mut new), stats(&mut old));
    println!("belfry frame (new): mean {:.2} ms, median {:.2} ms, slowest tenth {:.2} ms, tick {}", n.0, n.1, n.2, r.tick);
    println!("belfry frame (old): mean {:.2} ms, median {:.2} ms, slowest tenth {:.2} ms", o.0, o.1, o.2);
    println!("belfry frame: new/old mean {:.2}x, slowest tenth {:.2}x", n.0 / o.0, n.2 / o.2);
    println!("belfry parts (ms): floor {:.2}, over {:.2}, screen {:.2}", parts[0], parts[1], parts[2]);
}

/// The belfry draws through the whole minute and its finish, for a lone
/// knight, a full table and six seats, and the shoot it draws is the same
/// twice (the host's and a mirror's are one). Its pixels are the same twice
/// too (`the_belfry_pixels_are_the_same_twice`, which wants the process to
/// itself: another test's global switches, the calm screen among them,
/// change a frame).
#[test]
fn the_belfry_draws_through_the_whole_minute_the_same_twice() {
    belfry_digests(false);
}

#[test]
#[ignore = "pixel determinism: run alone"]
fn the_belfry_pixels_are_the_same_twice() {
    belfry_digests(true);
}

fn belfry_digests(pixels: bool) {
    use std::hash::{Hash, Hasher};
    let digest = |seats: u32| {
        let mut r = fresh_for(seats);
        let mut frames = Vec::new();
        let mut states = Vec::new();
        for t in 0..TITLE + BAT_TIME + 420 {
            let k = keys(&r);
            mend(&mut r);
            r.step(&k);
            if t % 29 == 0 || t > TITLE + BAT_TIME && t % 7 == 0 {
                let img = frame_for(&r, 384, 224, Some(1));
                assert_eq!((img.w, img.h), (384, 224));
                let mut h = std::collections::hash_map::DefaultHasher::new();
                img.rgb_bytes().hash(&mut h);
                frames.push((t, h.finish()));
                let mut h = std::collections::hash_map::DefaultHasher::new();
                serde_json::to_string(&r.bats).unwrap().hash(&mut h);
                states.push((t, h.finish()));
            }
            if let Some(shoot) = r.bats.as_ref() {
                assert!(shoot.valid(), "the shoot stays valid at tick {t}");
            }
        }
        let shoot = r.bats.as_ref().expect("the shoot is still on the books");
        assert!(shoot.done && shoot.show.ended.is_some(), "the minute ends with its finish noted");
        assert!(shoot.show.team > 0, "bats were felled");
        (frames, states)
    };
    for seats in [1, 4, 6] {
        let (a, b) = (digest(seats), digest(seats));
        assert_eq!(a.1, b.1, "{seats} seats: the same shoot twice");
        if pixels {
            assert_eq!(a.0, b.0, "{seats} seats: the same picture twice");
        }
    }
}
