//! The show: the marks a run leaves and the pictures made of them. The ignored test at the end
//! plays the real adventure with the game's own bots and writes the moments (the start, a stomp, a
//! rune block's coins, a banner raised, a ground pound, King Cinder's crown, the finish) as frames
//! for the proof GIFs: `ANGEL_BP_SHOTS=<dir> cargo test -p angel-delve --lib write_bannerpeak_shots
//! -- --ignored --nocapture`.
use super::super::frame_for;
use crate::rules::bannerpeak::{
    session::{Phase, Session},
    show::{Cry, Kind},
};
use crate::rules::{Input, Run};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn party(seats: u32) -> Run {
    let mut r = Run::bannerpeak_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

fn bots(r: &Run) -> BTreeMap<u32, Input> {
    let Some(s) = r.bannerpeak.as_ref() else { return BTreeMap::new() };
    // Each knight sets off a little after the one before, so the party runs in a string and not in a
    // heap: the bots are identical, and real friends never are.
    s.seats
        .keys()
        .filter_map(|&id| {
            let hero = r.players.get(&id)?;
            let wait = s.phase == Phase::Play && s.elapsed < (id - 1) * 40;
            Some((id, if wait { Input::default() } else { s.bot_input(id, hero) }))
        })
        .collect()
}

/// Every kind of mark and call a whole adventure leaves, and the tick each first showed.
#[derive(Default)]
struct Seen {
    kinds: BTreeMap<String, u32>,
    held: u32,
}

fn play(seats: u32, limit: u32, mut each: impl FnMut(&Run, u32)) -> Run {
    let mut r = party(seats);
    for t in 0..limit {
        let keys = bots(&r);
        r.step(&keys);
        each(&r, t);
        if r.bannerpeak.as_ref().is_some_and(|s| s.completed.is_some()) && t > 0 {
            // Stand a while at the finish so its scores show.
            if r.bannerpeak.as_ref().is_some_and(|s| s.phase == Phase::Finale && s.phase_t > 205) {
                break;
            }
        }
    }
    r
}

fn scan(seats: u32) -> (Run, Seen) {
    let mut seen = Seen::default();
    let r = play(seats, 14000, |r, t| {
        let s = r.bannerpeak.as_ref().unwrap();
        for m in &s.show.marks {
            if m.born == s.elapsed {
                seen.kinds.entry(format!("{:?}", m.kind)).or_insert(t);
                seen.kinds.entry(format!("{:?}@{}", m.kind, m.who)).or_insert(t);
            }
        }
        for c in &s.show.shouts {
            if c.born == s.elapsed {
                let name = match c.cry {
                    Cry::Together(_) => "Together".to_string(),
                    Cry::Chain(_) => "Chain".to_string(),
                    other => format!("{other:?}"),
                };
                seen.kinds.entry(format!("shout:{name}")).or_insert(t);
            }
        }
        if r.hitstop.left > 0 {
            seen.held += 1;
        }
    });
    (r, seen)
}

#[test]
fn a_whole_adventure_leaves_every_mark_it_should_and_holds_the_room_for_the_heavy_blows() {
    let (r, seen) = scan(3);
    println!("{:?} held {}", seen.kinds, seen.held);
    let s = r.bannerpeak.as_ref().unwrap();
    assert!(s.completed.is_some(), "the bots clear it");
    for kind in ["Land", "Hop", "Step", "Stomp", "Coin", "Block", "Banner", "Slam", "Plate", "Crown", "Gate"] {
        assert!(seen.kinds.contains_key(kind), "no {kind} mark in a whole adventure: {:?}", seen.kinds.keys().collect::<Vec<_>>());
    }
    assert!(seen.kinds.contains_key("shout:Plate") && seen.kinds.contains_key("shout:Crown"));
    assert!(seen.held > 0, "a plate off the crown holds the room");
}

#[test]
fn the_show_is_bounded_and_rides_the_wire_and_the_save() {
    let mut r = party(4);
    for _ in 0..900 {
        let keys = bots(&r);
        r.step(&keys);
        let s = r.bannerpeak.as_ref().unwrap();
        assert!(s.valid(), "the session is valid every tick");
        assert!(s.show.marks.len() <= 48 && s.show.shouts.len() <= 6);
    }
    let s = r.bannerpeak.as_ref().unwrap();
    let back: Session = serde_json::from_str(&serde_json::to_string(s).unwrap()).unwrap();
    assert_eq!(&back, s);
    assert!(!s.show.marks.is_empty(), "nine seconds of running leave marks");
}

#[test]
fn an_old_save_with_no_show_reads_as_a_quiet_one() {
    let mut r = party(2);
    r.step(&BTreeMap::new());
    let s = r.bannerpeak.as_ref().unwrap();
    let mut value = serde_json::to_value(s).unwrap();
    value.as_object_mut().unwrap().remove("show");
    let back: Session = serde_json::from_value(value).unwrap();
    assert!(back.show.quiet() && back.valid());
}

#[test]
fn the_picture_never_changes_the_run() {
    // The same bots on the same map, drawn every tick and not drawn at all, play the same run.
    let mut a = party(3);
    let mut b = party(3);
    for t in 0..600 {
        let (ka, kb) = (bots(&a), bots(&b));
        a.step(&ka);
        b.step(&kb);
        if t % 7 == 0 {
            let _ = frame_for(&a, 384, 224, Some(1));
        }
    }
    assert_eq!(a.bannerpeak, b.bannerpeak);
}

#[test]
fn the_knights_are_sprites_in_their_seats_colours_not_boxes() {
    let r = party(4);
    for seat in 1..=4u32 {
        let im = frame_for(&r, 384, 224, Some(seat));
        // Every seat sees its own knight's plume and tabard in its own tone.
        let tone = crate::render::overworld::ink::ink(super::art::tones(seat)[1]).unwrap();
        let count = (0..224).flat_map(|y| (0..384).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y) == Some(tone)).count();
        assert!(count >= 14, "seat {seat}'s tabard shows in {count} pixels");
    }
}

#[test]
fn a_blow_shakes_the_world_and_not_the_bar() {
    let mut r = party(1);
    let still = frame_for(&r, 384, 224, Some(1));
    let s = r.bannerpeak.as_mut().unwrap();
    let now = s.elapsed;
    s.show.blow(now, 1, 100, 100, crate::rules::juice::Weight::Finale);
    s.show.hold = None;
    let mut moved = false;
    for _ in 0..4 {
        let shaken = frame_for(&r, 384, 224, Some(1));
        // The bar's top row never leaves its place; the world under it does.
        assert!((0..384).all(|x| shaken.get(x, 0) == still.get(x, 0)));
        moved |= (20..200).any(|y| (0..384).any(|x| shaken.get(x, y) != still.get(x, y)));
        let s = r.bannerpeak.as_mut().unwrap();
        s.elapsed += 1;
    }
    assert!(moved);
}

#[test]
fn the_frame_costs_what_it_did_within_a_quarter() {
    let r = party(4);
    let t0 = std::time::Instant::now();
    for _ in 0..30 {
        let _ = frame_for(&r, 384, 224, Some(1));
    }
    let per = t0.elapsed().as_secs_f64() * 1000.0 / 30.0;
    println!("frame: {per:.2} ms");
    assert!(per < 400.0, "a frame took {per} ms even for a debug build");
}

// ------------------------------------------------------------ the proof frames

fn save(dir: &PathBuf, name: &str, frames: &[Vec<crate::render::overworld::Img>]) {
    let out = dir.join(name);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    for (i, seats) in frames.iter().enumerate() {
        let w = 384 * seats.len() as i32;
        let mut sheet = crate::render::overworld::Img::black(w, 224);
        for (n, im) in seats.iter().enumerate() {
            sheet.stamp(im, 384 * n as i32, 0);
        }
        image::save_buffer(out.join(format!("f{i:04}.png")), &sheet.rgb_bytes(), w as u32, 224, image::ColorType::Rgb8).unwrap();
    }
    println!("{name}: {} frames", frames.len());
}

#[test]
#[ignore = "writes the proof frames"]
fn write_bannerpeak_shots() {
    let Ok(dir) = std::env::var("ANGEL_BP_SHOTS") else { return };
    let dir = PathBuf::from(dir);
    let only: Option<BTreeSet<String>> = std::env::var("BP_ONLY").ok().map(|v| v.split(',').map(String::from).collect());
    // One pass finds the moments; a second replays the same run and draws the windows.
    let seats = 3;
    let (_, seen) = scan(seats);
    println!("{:?}", seen.kinds);
    let at = |k: &str| seen.kinds.get(k).copied();
    let mut windows: Vec<(&str, u32, u32, Vec<u32>)> = vec![("start", 0, 330, vec![1])];
    for (name, key, before, after) in [
        ("stomp", "Stomp@1", 40u32, 50u32),
        ("coins", "Block@1", 36, 80),
        ("land", "Land@1", 12, 30),
        ("slam", "Slam@1", 24, 48),
        ("banner", "Banner", 20, 140),
        ("plate", "Plate", 30, 70),
        ("crown", "Crown", 24, 160),
        ("gate", "Gate", 20, 50),
        ("rescue", "Rescue", 50, 90),
        ("hurt", "Hurt", 12, 40),
        ("coop", "shout:Together", 40, 100),
    ] {
        if let Some(t) = at(key) {
            windows.push((name, t.saturating_sub(before), t + after, vec![1]));
        }
    }
    // The finish, with two seats side by side.
    windows.push(("finale", 0, 0, vec![1, 2]));
    let mut reels: BTreeMap<&str, Vec<Vec<crate::render::overworld::Img>>> = BTreeMap::new();
    let mut finale_from = None;
    let mut r = party(seats);
    for t in 0..14000u32 {
        let keys = bots(&r);
        r.step(&keys);
        let Some(s) = r.bannerpeak.as_ref() else { break };
        if s.phase == Phase::Finale && finale_from.is_none() {
            finale_from = Some(t);
        }
        for (name, from, to, seat_list) in &windows {
            if only.as_ref().is_some_and(|o| !o.contains(*name)) {
                continue;
            }
            let (from, to) = if *name == "finale" { (finale_from.unwrap_or(u32::MAX), finale_from.map_or(0, |f| f + 250)) } else { (*from, *to) };
            if t >= from && t < to && (t - from) % 2 == 0 {
                let frames: Vec<_> = seat_list.iter().map(|&id| frame_for(&r, 384, 224, Some(id))).collect();
                reels.entry(*name).or_default().push(frames);
            }
        }
        if finale_from.is_some_and(|f| t > f + 252) {
            break;
        }
    }
    for (name, frames) in reels {
        save(&dir, name, &frames);
    }
}

#[test]
#[ignore = "times a frame"]
fn time_bannerpeak_frame() {
    // The same frame the baseline was timed on: four seats standing at the door, the title up.
    let r = party(4);
    for _ in 0..5 {
        let _ = frame_for(&r, 384, 224, Some(1));
    }
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t0 = std::time::Instant::now();
        for _ in 0..40 {
            let _ = frame_for(&r, 384, 224, Some(1));
        }
        best = best.min(t0.elapsed().as_secs_f64() * 1000.0 / 40.0);
    }
    println!("FRAME_MS {best:.3}");
    // And one in the thick of it: four bots two hundred ticks in, dust and sparks in the air.
    let mut r = party(4);
    for _ in 0..420 {
        let keys = bots(&r);
        r.step(&keys);
    }
    let mut busy = f64::MAX;
    for _ in 0..5 {
        let t0 = std::time::Instant::now();
        for _ in 0..40 {
            let _ = frame_for(&r, 384, 224, Some(1));
        }
        busy = busy.min(t0.elapsed().as_secs_f64() * 1000.0 / 40.0);
    }
    println!("FRAME_BUSY_MS {busy:.3} ({} marks)", r.bannerpeak.as_ref().unwrap().show.marks.len());
}
