//! Griffinwing's pictures: the 3D scene on the native route and the cells,
//! each beat, the Wyrm, the tally. `ANGEL_ARENA_SHOTS=<dir> cargo test -p
//! angel-delve write_griffinwing_review -- --ignored` writes them to open.
use super::*;
use std::collections::BTreeMap;

#[test]
fn a_run_draws_the_flight_in_three_d_for_each_seat() {
    let mut r = Run::griffinwing_for_test();
    r.join_seat(2, "Knight 2");
    r.step(&BTreeMap::new());
    let (a, fx) = layers(&r, Some(1));
    assert_eq!((a.w, a.h, fx.w, fx.h), (384, 224, 384, 224));
    let b = layers(&r, Some(2)).0;
    assert_ne!(
        a.content_hash(),
        b.content_hash(),
        "each seat has its own camera"
    );
}

use crate::rules::Input;
use crate::rules::griffinwing::{Arm, Kind, St, Thing, Wyrm};

// ─── the layout: no words on words ──────────────────────────────────────────

/// A bots' run at `seats` seats; every `every` ticks of its first `ticks`,
/// each seat's HUD context (its camera, the sim, no scene behind it: the
/// layout does not look at the scene) is handed to `look`.
fn watch(seats: u32, ticks: u32, every: u32, mut look: impl FnMut(u32, &hud::Ctx)) {
    let mut r = Run::griffinwing_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    for tick in 0..ticks {
        let inputs = r.roundtable_inputs();
        r.step(&inputs);
        if tick % every != 0 {
            continue;
        }
        let Some(s) = r.griffinwing.as_ref() else {
            break;
        };
        for (&id, seat) in &s.seats {
            let cam = camera(s, &seat.craft);
            let ctx = hud::Ctx {
                s,
                run: &r,
                seat: id,
                cam: &cam,
                depth: &[],
            };
            look(id, &ctx);
        }
    }
}

#[test]
fn the_names_keep_off_the_hud_the_reticle_and_one_another_at_every_seat_count() {
    use layout::FRAME;
    let (mut samples, mut wanted, mut placed) = (0u32, 0usize, 0usize);
    for seats in 1..=6u32 {
        watch(seats, 9_000, 7, |seat, c| {
            samples += 1;
            let at = format!("{seats} seats, seat {seat}, beat {} t {}", c.s.stage, c.s.t);
            // The floating labels: in the frame, none on another, none on the
            // HUD or the reticle.
            let hud = hud::boxes(c);
            let reticle = hud::reticle_boxes(c);
            let plan = layout::plan(c);
            wanted += hud::wants(c).len();
            placed += plan.placed.len();
            for (i, a) in plan.placed.iter().enumerate() {
                let words = (&a.text, a.plate);
                assert!(a.plate.within(&FRAME), "{at}: {words:?} leaves the frame");
                for (name, b) in &hud {
                    assert!(!a.plate.hits(b), "{at}: {words:?} lies on the HUD's {name}");
                }
                for b in &reticle {
                    assert!(
                        !a.plate.hits(b),
                        "{at}: {words:?} lies on the reticle {b:?}"
                    );
                }
                for b in &plan.placed[i + 1..] {
                    assert!(
                        !a.plate.hits(&b.plate),
                        "{at}: {words:?} lies on {:?}",
                        (&b.text, b.plate)
                    );
                }
            }
        });
    }
    assert!(samples > 500, "{samples} pictures looked at");
    // Keeping off is not hiding: nearly every label the stage wanted is up.
    assert!(
        placed * 10 >= wanted * 8,
        "{placed} of {wanted} labels found a place"
    );
}

#[test]
fn the_hud_keeps_off_itself_at_every_seat_count() {
    use layout::FRAME;
    for seats in 1..=6u32 {
        watch(seats, 9_000, 7, |seat, c| {
            let at = format!("{seats} seats, seat {seat}, beat {} t {}", c.s.stage, c.s.t);
            let hud = hud::boxes(c);
            for (i, (a, ba)) in hud.iter().enumerate() {
                assert!(ba.within(&FRAME), "{at}: {a} {ba:?} leaves the frame");
                for (b, bb) in &hud[i + 1..] {
                    assert!(!ba.hits(bb), "{at}: {a} {ba:?} lies on {b} {bb:?}");
                }
            }
        });
    }
}

#[test]
fn a_name_that_cannot_be_laid_clear_is_left_out_not_laid_on_another() {
    use layout::{Placer, Rank, Want};
    let want = |text: &str, x: i32, y: i32| Want {
        rank: Rank::Craft,
        text: text.into(),
        ink: 'w',
        x,
        y,
        near: 0,
        reach: 40,
    };
    // Two names wanted at the one point: the second is moved off the first.
    let mut p = Placer::new(Vec::new());
    assert!(p.place(&want("KNIGHT 3", 192, 100)));
    assert!(p.place(&want("KNIGHT 2", 192, 100)));
    assert!(!p.placed[0].plate.hits(&p.placed[1].plate));
    // A point whose every place is taken: the name is dropped.
    let mut p = Placer::new(vec![layout::FRAME]);
    assert!(!p.place(&want("KNIGHT 2", 192, 100)));
    assert_eq!((p.placed.len(), p.dropped), (0, 1));
    // A name at the frame's edge is slid in, not cut off.
    let mut p = Placer::new(Vec::new());
    assert!(p.place(&want("KNIGHT 6", 4, 100)));
    assert!(p.placed[0].plate.within(&layout::FRAME));
}

#[test]
fn the_opening_calls_stand_clear_of_the_key_strip_and_the_card() {
    let mut seen = 0;
    for seats in [1u32, 2, 4, 6] {
        watch(seats, 120, 1, |seat, c| {
            if seat != 1 || c.s.t != 40 || c.s.stage != 0 {
                return;
            }
            let hud = hud::boxes(c);
            let get = |n: &str| -> Vec<layout::Box2> {
                hud.iter()
                    .filter(|(k, _)| *k == n)
                    .map(|(_, b)| *b)
                    .collect()
            };
            let (calls, keys, card) = (get("call"), get("keys"), get("card"));
            assert_eq!((keys.len(), card.len()), (1, 1), "{seats} seats");
            assert!(!calls.is_empty(), "{seats} seats: the opening is called");
            seen += 1;
            for call in &calls {
                assert!(
                    call.y + call.h < keys[0].y,
                    "{seats} seats: the call {call:?} runs onto the keys {:?}",
                    keys[0]
                );
                assert!(!call.hits(&card[0]), "{seats} seats: a call on the card");
            }
            // The card's lines sit inside its plate, one under the other.
            let card = hud::card(c).expect("the card is up at t 40");
            let lines = card.lines();
            assert_eq!(lines.len(), 3, "the first beat's card has three lines");
            for (i, a) in lines.iter().enumerate() {
                assert!(
                    a.within(&card.plate),
                    "{seats} seats: {a:?} out of the card"
                );
                for b in &lines[i + 1..] {
                    assert!(!a.hits(b), "{seats} seats: card lines {a:?} and {b:?}");
                }
            }
        });
    }
    assert_eq!(seen, 4);
}

#[test]
fn the_tally_has_the_picture_to_itself() {
    let mut r = Run::griffinwing_for_test();
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    {
        let s = r.griffinwing.as_mut().unwrap();
        s.mode = Mode::Tally;
        s.mode_t = 100;
        s.home = s.anchor();
    }
    r.step(&BTreeMap::new());
    let s = r.griffinwing.as_ref().unwrap();
    let c = s.seats[&1].craft.clone();
    let cam = camera(s, &c);
    let ctx = hud::Ctx {
        s,
        run: &r,
        seat: 1,
        cam: &cam,
        depth: &[],
    };
    let boxes = hud::boxes(&ctx);
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    assert_eq!(boxes[0].0, "tally");
    assert!(boxes[0].1.within(&layout::FRAME), "six rows fit the frame");
}

// ─── the canyon is read ─────────────────────────────────────────────────────

/// How much of the canyon's rock and floor (the dots the scene covers, not
/// the sky) is black, and the mean value of it, through seat 1's camera.
fn canyon_look(stage: u8, t: u32) -> (f32, f32) {
    let mut r = Run::griffinwing_for_test();
    r.step(&BTreeMap::new());
    let s = r.griffinwing.as_mut().unwrap();
    s.stage = stage;
    s.t = t;
    s.next = u16::MAX;
    let (im, depth, _) = dots(s, 1);
    let (mut black, mut covered, mut sum) = (0u32, 0u32, 0u64);
    for (i, d) in depth.iter().enumerate() {
        if !d.is_finite() {
            continue;
        }
        let (x, y) = ((i as i32) % im.w, (i as i32) / im.w);
        let [r, g, b] = im.get(x, y).unwrap_or([0, 0, 0]);
        let peak = r.max(g).max(b);
        covered += 1;
        black += u32::from(peak < 30);
        sum += u64::from(peak);
    }
    (
        black as f32 / covered.max(1) as f32,
        sum as f32 / covered.max(1) as f32,
    )
}

#[test]
fn the_canyon_reads_at_every_beat_and_is_not_a_black_wall() {
    // A dusk canyon, not a lit one: some of it may sink into shadow, never
    // most of it, and what is lit must have a value to be seen by. (Before
    // the canyon was lifted, the gorge was 40 to 50 percent black, the
    // approach 62 and the roost 83, with a mean value of 16 to 38.)
    for (name, stage, t, black, mean) in [
        ("gorge, early", 0u8, 60u32, 0.25, 50.0),
        ("gorge, middle", 0, 700, 0.25, 50.0),
        ("gorge, late", 0, 1300, 0.25, 50.0),
        ("causeway", 1, 900, 0.25, 60.0),
        ("causeway, late", 1, 2000, 0.25, 60.0),
        ("approach", 2, 200, 0.30, 42.0),
        ("roost", 2, 900, 0.30, 42.0),
    ] {
        let (dark, bright) = canyon_look(stage, t);
        assert!(
            dark <= black,
            "{name}: {:.0}% of the canyon is black",
            dark * 100.0
        );
        assert!(
            bright >= mean,
            "{name}: the canyon's mean value is {bright:.0}, under {mean}"
        );
    }
}

/// Where a picture is wanted: a name, and what must be true of the run.
type Want = (&'static str, fn(&Session) -> bool);

/// The moments to look at, each the first time it is so.
fn wants() -> Vec<Want> {
    vec![
        ("01-title", |s| s.stage == 0 && s.t == 40),
        ("02-wisps", |s| s.stage == 0 && s.t == 250),
        ("03-locks", |s| {
            s.seats.values().any(|k| k.craft.locks.len() >= 3)
        }),
        ("04-charged", |s| {
            s.seats.values().any(|k| k.craft.charge >= 36)
        }),
        ("05-latched", |s| s.foes.iter().any(|f| f.st == St::Latched)),
        ("06-gates", |s| s.stage == 0 && s.t == 1480),
        ("07-turret-tell", |s| {
            s.foes
                .iter()
                .any(|f| f.kind == Kind::Turret && f.st == St::Tell && f.ticks < 30)
        }),
        ("08-reflect", |s| {
            s.bolts.iter().any(|b| b.shot == Arm::Turned)
        }),
        ("09-convoy", |s| {
            s.foes.iter().filter(|f| f.kind == Kind::Kite).count() >= 4
        }),
        ("10-bomber-tell", |s| {
            s.foes
                .iter()
                .any(|f| f.kind == Kind::Bomber && f.st == St::Tell && f.ticks < 25)
        }),
        ("11-shutter", |s| {
            s.props
                .iter()
                .any(|p| p.thing == Thing::Shutter && p.s - s.anchor() < 3000 && p.s > s.anchor())
        }),
        ("12-arch", |s| {
            s.props
                .iter()
                .any(|p| p.thing == Thing::Arch && p.s - s.anchor() < 2000 && p.s > s.anchor())
        }),
        ("13-wyrm-rise", |s| {
            s.boss
                .as_ref()
                .is_some_and(|b| b.st == Wyrm::Rise && b.ticks < 40)
        }),
        ("14-wyrm-spit-tell", |s| {
            s.boss
                .as_ref()
                .is_some_and(|b| b.st == Wyrm::Tell && b.atk == 0 && b.ticks < 25)
        }),
        ("15-lamp-open", |s| {
            s.foes
                .iter()
                .any(|f| f.kind == Kind::Lamp && f.st == St::Open)
        }),
        ("16-coil-tell", |s| {
            s.boss
                .as_ref()
                .is_some_and(|b| b.st == Wyrm::Tell && b.atk == 1 && b.ticks < 25)
        }),
        ("17-roar-tell", |s| {
            s.boss
                .as_ref()
                .is_some_and(|b| b.st == Wyrm::Tell && b.atk == 2 && b.ticks < 25)
        }),
        ("18-victory", |s| s.mode == Mode::Victory && s.mode_t == 80),
        ("19-tally", |s| s.mode == Mode::Tally && s.mode_t == 40),
        ("20-hurt", |s| s.seats.values().any(|k| k.craft.hurt == 35)),
    ]
}

/// The pictures of the bots' run at `seats` seats, written to `dir`.
fn shoot(dir: &std::path::Path, seats: u32, tag: &str) {
    use super::super::frame_for;
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let mut r = Run::griffinwing_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    let mut todo = wants();
    let (mut wiped, mut shot_retry) = (false, false);
    for _ in 0..14_000 {
        let inputs = r.roundtable_inputs();
        r.step(&inputs);
        let Some(s) = r.griffinwing.as_ref() else {
            break;
        };
        let mut taken = Vec::new();
        for (k, (name, hit)) in todo.iter().enumerate() {
            if hit(s) {
                let img = frame_for(&r, 384, 224, Some(1));
                save(&format!("{tag}-{name}"), &img);
                if ["01", "03", "07", "14", "19"]
                    .iter()
                    .any(|n| name.starts_with(n))
                {
                    let big = frame_for(&r, 1152, 672, Some(1));
                    save(&format!("{tag}-{name}-3x"), &big);
                    let cells = frame_for(&r, 192, 112, Some(1));
                    let mut up = Img::black(768, 448);
                    for y in 0..448 {
                        for x in 0..768 {
                            if let Some(c) = cells.get(x / 4, y / 4) {
                                up.set(x, y, c);
                            }
                        }
                    }
                    save(&format!("{tag}-{name}-cells"), &up);
                }
                taken.push(k);
            }
        }
        for k in taken.into_iter().rev() {
            todo.remove(k);
        }
        // Force a wipe once, mid second beat, to look at the retry.
        if !wiped && !shot_retry && s.stage == 1 && s.t == 400 {
            wiped = true;
            let g = r.griffinwing.as_mut().unwrap();
            for seat in g.seats.values_mut() {
                seat.craft.shield = 0;
            }
        }
        if wiped
            && r.griffinwing
                .as_ref()
                .is_some_and(|g| g.mode == Mode::Retry)
        {
            save(
                &format!("{tag}-21-retry"),
                &frame_for(&r, 384, 224, Some(1)),
            );
            wiped = false;
            shot_retry = true;
        }
        if r.griffinwing.as_ref().is_some_and(Session::finished) && todo.is_empty() {
            break;
        }
    }
    let g = r.griffinwing.as_ref();
    eprintln!(
        "{tag}: ended at stage {:?} t {:?} mode {:?} tick {} not reached {:?}",
        g.map(|g| g.stage),
        g.map(|g| g.t),
        g.map(|g| g.mode),
        r.tick,
        todo.iter().map(|w| w.0).collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "writes Griffinwing's review pictures"]
fn write_griffinwing_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for seats in [1, 2, 4, 6] {
        shoot(&dir, seats, &format!("seats{seats}"));
    }
}

#[test]
#[ignore = "writes the canyon alone, for looking at"]
fn write_griffinwing_canyon() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, stage, t) in [
        ("gorge-a", 0u8, 60u32),
        ("gorge-b", 0, 700),
        ("gorge-c", 0, 1300),
        ("causeway-a", 1, 100),
        ("causeway-b", 1, 900),
        ("causeway-c", 1, 2000),
        ("approach", 2, 200),
        ("roost", 2, 900),
        ("exit", 2, 2200),
    ] {
        let mut r = Run::griffinwing_for_test();
        r.step(&BTreeMap::new());
        let s = r.griffinwing.as_mut().unwrap();
        s.stage = stage;
        s.t = t;
        s.next = u16::MAX;
        let (im, _, _) = dots(s, 1);
        image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("canyon-{name}.png")))
            .unwrap();
    }
}

/// Moments the bots do not make: a kite on a rider's tail, a craft down, the
/// spur, Starfall, a roll, a thin shield, a full table's tally.
#[test]
#[ignore = "writes Griffinwing's scripted review pictures"]
fn write_griffinwing_scenes() {
    use super::super::frame_for;

    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("scene-{name}.png")))
            .unwrap();
    };
    let fresh = |seats: u32| {
        let mut r = Run::griffinwing_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&BTreeMap::new());
        r.griffinwing.as_mut().unwrap().next = u16::MAX;
        r
    };
    let tick = |r: &mut Run, n: u32, i: Input| {
        for _ in 0..n {
            r.step(&BTreeMap::from([(1, i)]));
        }
    };
    // A kite latched onto a rider, shown the moment after.
    let mut r = fresh(1);
    r.griffinwing.as_mut().unwrap().hunter_for_test(-1);
    for _ in 0..300 {
        r.step(&BTreeMap::new());
        if r.griffinwing
            .as_ref()
            .unwrap()
            .foes
            .iter()
            .any(|f| f.st == St::Latched)
        {
            break;
        }
    }
    tick(&mut r, 20, Input::default());
    save("latched", &frame_for(&r, 384, 224, Some(1)));
    // The spur: speed streaks, the meter running down.
    let mut r = fresh(1);
    tick(&mut r, 90, Input::default());
    tick(
        &mut r,
        30,
        Input {
            interact: true,
            ..Default::default()
        },
    );
    save("boost", &frame_for(&r, 384, 224, Some(1)));
    // Starfall.
    let mut r = fresh(1);
    {
        let s = r.griffinwing.as_mut().unwrap();
        for k in 0..5 {
            let at = s.anchor() + 600 * 10 + k * 300;
            s.spawn_for_test(Kind::Wisp, (at, (k - 2) * 400, 100), (-15, 0, 0), St::Fly);
        }
    }
    tick(&mut r, 100, Input::default());
    tick(
        &mut r,
        1,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    tick(&mut r, 5, Input::default());
    save("starfall", &frame_for(&r, 384, 224, Some(1)));
    // A roll, and a craft that is down.
    let mut r = fresh(2);
    tick(&mut r, 100, Input::default());
    tick(
        &mut r,
        8,
        Input {
            dash: true,
            move_x: 1,
            ..Default::default()
        },
    );
    save("roll", &frame_for(&r, 384, 224, Some(1)));
    let s = r.griffinwing.as_mut().unwrap();
    s.seats.get_mut(&2).unwrap().craft.shield = 0;
    tick(&mut r, 12, Input::default());
    save("down-seat2", &frame_for(&r, 384, 224, Some(2)));
    save("down-seen-by-1", &frame_for(&r, 384, 224, Some(1)));
    // A thin shield.
    let mut r = fresh(1);
    r.griffinwing
        .as_mut()
        .unwrap()
        .seats
        .get_mut(&1)
        .unwrap()
        .craft
        .shield = 22;
    r.griffinwing
        .as_mut()
        .unwrap()
        .seats
        .get_mut(&1)
        .unwrap()
        .craft
        .hurt = 0;
    tick(&mut r, 100, Input::default());
    save("low-shield", &frame_for(&r, 384, 224, Some(1)));
    // The second and third beats' cards, at a full table.
    for (name, stage) in [("card-causeway", 1u8), ("card-roost", 2)] {
        let mut r = fresh(6);
        let s = r.griffinwing.as_mut().unwrap();
        s.stage = stage;
        s.t = 30;
        save(name, &frame_for(&r, 384, 224, Some(1)));
    }
    // A full table at the tally.
    let mut r = fresh(6);
    {
        let s = r.griffinwing.as_mut().unwrap();
        s.mode = Mode::Tally;
        s.mode_t = 100;
        s.done = Some(6300);
        s.home = s.anchor();
        s.bank = crate::rules::griffinwing::Tally {
            pts: 14_800,
            base: 14_000,
            offered: 15_000,
            kills: 120,
            saved: 3,
            lost: 1,
            gates: 90,
        };
        s.medal = Some(crate::rules::griffinwing::Medal::Silver);
        for (k, seat) in s.seats.values_mut().enumerate() {
            seat.craft.pts = 2000 + k as u32 * 130;
            seat.craft.hits = 20 + k as u32;
            seat.craft.saved = (k % 3) as u16;
        }
    }
    r.step(&BTreeMap::new());
    save("tally-six", &frame_for(&r, 384, 224, Some(1)));
}

/// Consecutive frames, for looking at motion: the camera's lag, the flicker,
/// the sky. `ANGEL_ARENA_SHOTS=<dir> cargo test write_griffinwing_strip -- --ignored`.
#[test]
#[ignore = "writes a strip of consecutive frames"]
fn write_griffinwing_strip() {
    use super::super::frame_for;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = Run::griffinwing_for_test();
    r.join_seat(2, "Knight 2");
    r.step(&BTreeMap::new());
    let mut k = 0;
    for tick in 0..2_000u32 {
        let inputs = r.roundtable_inputs();
        r.step(&inputs);
        let at = r.griffinwing.as_ref().map_or(0, |s| s.t);
        if tick > 330 && tick % 3 == 0 && k < 12 {
            let img = frame_for(&r, 384, 224, Some(1));
            image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
                .unwrap()
                .save(dir.join(format!("strip-{k:02}.png")))
                .unwrap();
            k += 1;
        }
        let _ = at;
    }
}

// ─── the painted sky ────────────────────────────────────────────────────────

fn sky_view(heading: f32, pitch: f32) -> crate::render::world3d::raster::View3 {
    crate::render::world3d::raster::View3 {
        pos: crate::render::world3d::math::v3(0.0, 0.0, 0.0),
        heading_rad: heading,
        pitch,
        fov_rad: cam::FOV,
    }
}

/// A depth buffer with a wall of near scene over the lower left.
fn wall_depth() -> Vec<f32> {
    let mut depth = vec![f32::INFINITY; (W * H) as usize];
    for y in 80..H {
        for x in 0..130 {
            depth[(y * W + x) as usize] = 6.0;
        }
    }
    depth
}

#[test]
fn the_painted_sky_is_laid_only_where_the_scene_left_the_sky() {
    let depth = wall_depth();
    for sky_of in [sky::Sky::Dusk, sky::Sky::Smoke, sky::Sky::Dawn] {
        let mut im = Img::new(W, H);
        sky::paint(&mut im, &depth, &sky_view(0.2, -0.05), sky_of, 10);
        for y in 0..H {
            for x in 0..W {
                let covered = depth[(y * W + x) as usize].is_finite();
                assert_eq!(
                    im.get(x, y).is_some(),
                    !covered,
                    "dot ({x}, {y}): the sky is the dots the scene left"
                );
            }
        }
    }
}

#[test]
fn the_painted_sky_is_the_bands_own_inks_and_turns_with_the_camera() {
    let depth = vec![f32::INFINITY; (W * H) as usize];
    // The band's own inks, and the same under the stage's dark layer (a fifth,
    // snapped to the palette's plain inks) when there is one.
    let ink_set = |names: &[&str]| -> std::collections::BTreeSet<[u8; 3]> {
        let mut set = std::collections::BTreeSet::new();
        for n in names {
            let band = &bank::pic("sky", n, 0).expect("the sky is painted whole").img;
            for c in band.pixels() {
                set.insert(c);
                set.insert(crate::render::overworld::ink::nearest_plain(c.map(|v| f32::from(v) * 0.8)));
            }
        }
        set
    };
    for (sky_of, names) in [
        (sky::Sky::Dusk, vec!["dusk", "clouds"]),
        (sky::Sky::Smoke, vec!["smoke"]),
        (sky::Sky::Dawn, vec!["dawn"]),
    ] {
        let inks = ink_set(&names);
        let paint = |heading: f32| {
            let mut im = Img::new(W, H);
            sky::paint(&mut im, &depth, &sky_view(heading, 0.0), sky_of, 0);
            im
        };
        let (a, b) = (paint(0.0), paint(0.35));
        for im in [&a, &b] {
            for c in im.pixels() {
                assert!(inks.contains(&c), "a dot of the sky that is no band's ink: {c:?}");
            }
        }
        assert_ne!(
            a.content_hash(),
            b.content_hash(),
            "a bend in the canyon turns the sky"
        );
        assert_eq!(a.content_hash(), paint(0.0).content_hash(), "the same camera paints the same sky");
    }
}

// ─── the painted canyon is this scene's alone ───────────────────────────────

/// Rock, stone, dark stone, bone and road side by side, drawn by the shared
/// table (`render_meshes`) and not by Griffinwing's.
fn shared_wall() -> (crate::render::world3d::mesh::Mesh, crate::render::world3d::raster::View3) {
    use crate::render::world3d::{math::v3, mesh::{Mesh, mat}};
    let mut mesh = Mesh::new();
    for (k, material) in [mat::ROCK, mat::STONE, mat::STONE_DARK, mat::BONE, mat::PATH]
        .into_iter()
        .enumerate()
    {
        let y = -5.0 + k as f32 * 2.0;
        mesh.push_quad(
            v3(9.0, y, 0.0),
            v3(9.0, y + 2.0, 0.0),
            v3(9.0, y + 2.0, 4.0),
            v3(9.0, y, 4.0),
            material,
        );
    }
    (mesh, sky_view(0.0, 0.0))
}

#[test]
fn painting_the_canyon_leaves_the_shared_material_table_alone() {
    use crate::render::world3d::raster::{Backdrop, Firelight, render_meshes, render_meshes_painted};
    let (mesh, view) = shared_wall();
    let shared = |m: &crate::render::world3d::mesh::Mesh| {
        render_meshes(&[m], &view, 96, 72, Firelight::STEADY, 52.0, Backdrop::Night).0
    };
    let before = shared(&mesh);
    // Griffinwing's own frames, at every beat, through its painted surfaces.
    for stage in 0..3u8 {
        let painted = paint::painted(stage).expect("the canyon textures are painted whole");
        let (mine, _) = render_meshes_painted(
            &[&mesh],
            &view,
            96,
            72,
            Firelight::STEADY,
            52.0,
            Backdrop::Night,
            Some(painted),
        );
        assert_ne!(
            before.as_raw(),
            mine.as_raw(),
            "beat {stage}: the painted surfaces are used"
        );
        let mut r = Run::griffinwing_for_test();
        r.step(&BTreeMap::new());
        let s = r.griffinwing.as_mut().unwrap();
        s.stage = stage;
        let _ = dots(s, 1);
    }
    // Afterwards the shared table still draws what it drew.
    assert_eq!(before.as_raw(), shared(&mesh).as_raw());
}

#[test]
fn only_griffinwing_hands_the_rasteriser_painted_surfaces() {
    // Every other 3D drawer calls `render_meshes` or `render_scene*`, which
    // run the shared table. A scene that wants its own must say so by name.
    fn walk(dir: &std::path::Path, hits: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, hits);
            } else if path.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("render_meshes_painted(") {
                    hits.push(path.to_string_lossy().into_owned());
                }
            }
        }
    }
    let mut hits = Vec::new();
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut hits);
    hits.sort();
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert!(hits[0].ends_with("arena/griffinwing.rs"), "{hits:?}");
    assert!(hits[1].ends_with("world3d/raster.rs"), "{hits:?}");
}

// ─── the set pieces, near and far ───────────────────────────────────────────

/// A run at `stage` with one prop stood `dist` units ahead of seat 1's craft
/// (and `x`, `z` units across and up), nothing else in the air.
fn with_piece(
    stage: u8,
    thing: Thing,
    dist: f32,
    at: (f32, f32),
    size: (f32, f32),
    phase: u16,
) -> Run {
    let mut r = Run::griffinwing_for_test();
    r.step(&BTreeMap::new());
    let g = r.griffinwing.as_mut().unwrap();
    g.stage = stage;
    g.t = 300;
    g.next = u16::MAX;
    g.props.clear();
    g.foes.clear();
    let q = |u: f32| (u * 256.0) as i32;
    let here = g.craft_s(&g.seats[&1].craft);
    g.props.push(crate::rules::griffinwing::Prop {
        thing,
        s: here + q(dist),
        x: q(at.0),
        z: q(at.1),
        w: q(size.0),
        h: q(size.1),
        len: if thing == Thing::Gate || thing == Thing::Ring { 0 } else { q(3.0) },
        taken: 0,
        phase,
    });
    r
}

#[test]
#[ignore = "writes the set pieces at several distances"]
fn write_griffinwing_pieces() {
    use super::super::frame_for;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("piece-{name}.png")))
            .unwrap();
    };
    for (name, thing, w) in [("gate", Thing::Gate, 2.0f32), ("ring", Thing::Ring, 2.2)] {
        for dist in [100.0f32, 70.0, 45.0, 28.0, 18.0, 12.0] {
            let r = with_piece(0, thing, dist, (0.0, 2.0), (w, 0.0), 0);
            save(&format!("{name}-{:03}", dist as i32), &frame_for(&r, 384, 224, Some(1)));
        }
        // The terminal's cells (192 by 112 dots, four to a dot here).
        for dist in [45.0f32, 28.0, 18.0] {
            let r = with_piece(0, thing, dist, (0.0, 2.0), (w, 0.0), 0);
            let cells = frame_for(&r, 192, 112, Some(1));
            let mut up = Img::black(768, 448);
            for y in 0..448 {
                for x in 0..768 {
                    if let Some(c) = cells.get(x / 4, y / 4) {
                        up.set(x, y, c);
                    }
                }
            }
            save(&format!("{name}-cells-{:03}", dist as i32), &up);
        }
        // Off to one side: the ring turned from the eye.
        for (tag, x) in [("left", -8.0f32), ("right", 8.0)] {
            let r = with_piece(0, thing, 22.0, (x, 2.0), (w, 0.0), 0);
            save(&format!("{name}-{tag}"), &frame_for(&r, 384, 224, Some(1)));
        }
    }
}

#[test]
#[ignore = "writes the last beacon at several distances"]
fn write_griffinwing_beacon() {
    use super::super::frame_for;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("piece-{name}.png")))
            .unwrap();
    };
    for dist in [150.0f32, 100.0, 60.0, 35.0] {
        // Dark, before the Wyrm falls: the roost's own beacon 560 units on.
        let mut r = Run::griffinwing_for_test();
        r.step(&BTreeMap::new());
        let g = r.griffinwing.as_mut().unwrap();
        g.stage = 2;
        g.next = u16::MAX;
        g.boss = None;
        g.props.clear();
        g.foes.clear();
        let back = g.seats[&1].craft.back as f32 / 256.0;
        g.t = ((560.0 - dist - back) * 256.0 / 85.0) as u32;
        save(&format!("beacon-unlit-{:03}", dist as i32), &frame_for(&r, 384, 224, Some(1)));
        // Lit, for the way home: 100 units past `home`.
        let mut r = Run::griffinwing_for_test();
        r.step(&BTreeMap::new());
        let g = r.griffinwing.as_mut().unwrap();
        g.stage = 2;
        g.next = u16::MAX;
        g.boss = None;
        g.props.clear();
        g.foes.clear();
        g.mode = Mode::Victory;
        g.mode_t = 300;
        let here = g.craft_s(&g.seats[&1].craft);
        g.home = here + ((dist - 100.0) * 256.0) as i32;
        save(&format!("beacon-lit-{:03}", dist as i32), &frame_for(&r, 384, 224, Some(1)));
    }
}

/// How far from the ring's middle, in world units, the first dot the piece
/// painted lies along the ray at `angle` (0 to the right, a quarter turn up),
/// or `None` if it painted nothing along it.
fn first_painted(
    (with, without): (&Img, &Img),
    (px, py): (f32, f32),
    dots_per_unit: f32,
    angle: f32,
) -> Option<f32> {
    use crate::rules::griffinwing::trig::Det;
    let (sin, cos) = angle.dsin_cos();
    let mut r = 0.0f32;
    while r < 5.0 {
        let (x, y) = ((px + cos * r * dots_per_unit) as i32, (py - sin * r * dots_per_unit) as i32);
        if with.get(x, y) != without.get(x, y) {
            return Some(r);
        }
        r += 0.02;
    }
    None
}

fn median(mut v: Vec<f32>) -> f32 {
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

#[test]
fn a_painted_gate_and_ring_stand_at_the_sims_radius_and_leave_the_hole_clear() {
    use crate::rules::griffinwing::bit;
    for (thing, w, between, expect) in [(Thing::Gate, 2.0f32, false, 1.9f32), (Thing::Ring, 2.2, true, 2.1)] {
        let drawn = with_piece(0, thing, 14.0, (0.0, 2.0), (w, 0.0), 0);
        let mut flown = with_piece(0, thing, 14.0, (0.0, 2.0), (w, 0.0), 0);
        flown.griffinwing.as_mut().unwrap().props[0].taken |= bit(1);
        let s = drawn.griffinwing.as_ref().unwrap();
        let (with, depth, cam) = dots(s, 1);
        let (without, _, _) = dots(flown.griffinwing.as_ref().unwrap(), 1);
        let ctx = hud::Ctx {
            s,
            run: &drawn,
            seat: 1,
            cam: &cam,
            depth: &depth,
        };
        let p = &s.props[0];
        let (px, py, d) = ctx.at(p.s, p.x, p.z).expect("the piece is in view");
        let k = ctx.tall(1.0, d);
        // The hole: along rays that miss a lantern, the first dot the piece
        // paints is at the sim's radius less the tenth of a unit a painted
        // piece may look smaller by, to within a quarter unit.
        let offset = if between { 0.3927 } else { 0.0 };
        let radii: Vec<f32> = (0..16)
            .filter_map(|i| {
                let angle = offset + i as f32 * std::f32::consts::TAU / 16.0;
                first_painted((&with, &without), (px, py), k, angle)
            })
            .collect();
        assert!(radii.len() >= 12, "{thing:?}: the ring is painted all round: {radii:?}");
        let hole = median(radii.clone());
        assert!(
            (hole - expect).abs() < 0.25,
            "{thing:?}: the hole is {hole:.2} units, the sim's radius less a tenth is {expect}: {radii:?}"
        );
        // Nothing is drawn in the middle of it, and the band stops about where
        // the rules stop counting a pass (the radius and a craft's own).
        assert!(radii.iter().all(|&r| r > 0.5 * w), "{thing:?}: the middle is clear: {radii:?}");
    }
}

#[test]
fn a_ring_a_seat_has_flown_is_not_drawn_for_that_seat_and_is_for_the_others() {
    use crate::rules::griffinwing::bit;
    let picture = |mark: &dyn Fn(&mut Session)| {
        let mut r = with_piece(0, Thing::Ring, 20.0, (0.0, 2.0), (2.2, 0.0), 0);
        mark(r.griffinwing.as_mut().unwrap());
        dots(r.griffinwing.as_ref().unwrap(), 1).0.content_hash()
    };
    let untouched = picture(&|_| {});
    let flown_by_me = picture(&|s| s.props[0].taken |= bit(1));
    let flown_by_another = picture(&|s| s.props[0].taken |= bit(2));
    let gone = picture(&|s| s.props.clear());
    assert_ne!(untouched, gone, "the ring is drawn");
    assert_eq!(flown_by_me, gone, "a ring I have flown is not drawn for me");
    assert_eq!(flown_by_another, untouched, "a ring another seat flew still is");
}

#[test]
fn a_painted_piece_is_hidden_by_whatever_stands_nearer_in_the_scene() {
    let pic = bank::pic("gate", "idle", 0).expect("the gate is painted");
    let piece = set::Piece {
        pic,
        at: (192.0, 112.0),
        scale: (2.0, 2.0),
        depth: 10.0,
    };
    let (mut open, mut behind) = (Img::new(W, H), Img::new(W, H));
    set::blit(&mut open, &vec![f32::INFINITY; (W * H) as usize], &piece);
    set::blit(&mut behind, &vec![5.0; (W * H) as usize], &piece);
    assert!(open.pixels().any(|c| c != [0, 0, 0]), "painted where nothing stands in front");
    assert!((0..H).all(|y| (0..W).all(|x| behind.get(x, y).is_none())), "nothing is painted over a nearer scene");
    // And the clear middle of a ring shows what is beyond it.
    assert!(open.get(192, 112).is_none(), "the hole is clear");
}

#[test]
fn the_beacon_flame_follows_the_sheets_own_ticks_and_the_set_pieces_leave_no_blocks() {
    let frame = |t: u32| bank::at("beacon", "lit", t).expect("the beacon is painted") as *const _;
    // lit is four frames of eight ticks.
    assert_eq!(frame(0), frame(7));
    assert_ne!(frame(7), frame(8));
    assert_ne!(frame(8), frame(16));
    assert_ne!(frame(16), frame(24));
    assert_eq!(frame(0), frame(32));
    // A painted gate, ring or beacon is not also built in the 3D scene.
    let mut r = with_piece(0, Thing::Gate, 20.0, (0.0, 2.0), (2.0, 0.0), 0);
    let s = r.griffinwing.as_ref().unwrap();
    let mut with = crate::render::world3d::mesh::Mesh::new();
    actors::movers(&mut with, s, 1, crate::render::world3d::math::v3(0.0, 0.0, 0.0));
    r.griffinwing.as_mut().unwrap().props.clear();
    let s = r.griffinwing.as_ref().unwrap();
    let mut without = crate::render::world3d::mesh::Mesh::new();
    actors::movers(&mut without, s, 1, crate::render::world3d::math::v3(0.0, 0.0, 0.0));
    assert_eq!(with.tris.len(), without.tris.len());
}
