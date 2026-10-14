//! The races on screen: every knight's own camera, the frame on black
//! paper, the board, and how long a frame takes.
use super::*;
use super::testing::*;
use crate::rules::race3d::{Ground, ItemKind, RACE_TIME};
use crate::rules::Input;
use std::collections::BTreeMap;

fn race_mut(run: &mut Run) -> &mut Race3d {
    run.race3d.as_mut().unwrap()
}

fn place(run: &mut Run, id: u32, s: f32, x: f32, z: f32, vs: f32) {
    let r = race_mut(run).riders.get_mut(&id).unwrap();
    (r.s, r.x, r.z, r.vs) = (s, x, z, vs);
}

fn lit(img: &Img) -> f32 {
    let n = img.pixels().filter(|&c| c != BLACK).count();
    n as f32 / (img.w * img.h) as f32
}

#[test]
fn each_knight_sees_the_race_from_their_own_ride() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let mut run = racing(course, 2);
        let s = r3::track(course).posts[2] + 3.0;
        place(&mut run, 1, s, 0.0, 0.0, 6.0);
        place(&mut run, 2, s + 9.0, 0.0, 0.0, 6.0);
        let one = super::super::frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
        let two = super::super::frame_for(&run, NATIVE_W, NATIVE_H, Some(2));
        assert_ne!(one, two, "{course:?}: two knights, two cameras");
        let k = lit(&one);
        // Dark art stays dark: mostly black paper, but never empty.
        assert!(k > 0.04 && k < 0.6, "{course:?} lit {k}");
        // A small half-block pane gets the same race, scaled down.
        let small = super::super::frame_for(&run, 160, 90, Some(1));
        assert!(lit(&small) > 0.02, "{course:?} small pane");
    }
}

#[test]
fn a_racer_rides_as_their_class() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let mut run = racing(course, 2);
        let s = r3::track(course).posts[2] + 3.0;
        place(&mut run, 1, s, 0.0, 0.0, 6.0);
        place(&mut run, 2, s + 6.0, 0.0, 0.0, 6.0);
        let plain = layers(&run, Some(1), true).0;
        classed(&mut run);
        let dressed = layers(&run, Some(1), true).0;
        assert_ne!(plain, dressed, "{course:?}: a mage is not the stock knight");
    }
}

#[test]
fn the_frame_is_dots_on_the_realms_inks() {
    let mut run = racing(Course::Shaft, 2);
    place(&mut run, 1, 30.0, 0.0, 0.0, 6.0);
    let (scene, _) = layers(&run, Some(1), true);
    let inks: std::collections::BTreeSet<Rgb> = scene.pixels().collect();
    assert!(inks.contains(&BLACK));
    for c in inks {
        assert!(super::super::super::ink::palette_index(c).is_some(), "{c:?} is off the palette");
    }
}

#[test]
fn the_board_shows_once_the_race_is_run_and_the_hall_comes_back_after() {
    let mut run = racing(Course::Cart, 2);
    let finish = r3::track(Course::Cart).finish;
    place(&mut run, 2, finish - 0.05, 0.0, 0.0, 8.0);
    place(&mut run, 1, finish - 0.05, 0.0, 0.0, 8.0);
    run.step(&BTreeMap::new());
    assert!(run.race3d.as_ref().unwrap().done);
    assert!(takeover(&run), "the board is up");
    for _ in 0..r3::PODIUM {
        run.step(&BTreeMap::new());
    }
    assert!(!takeover(&run), "the hall comes back");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_race3d_shots -- --ignored`:
/// each race at its start, mid-race with the field about, a crash, and the
/// finish board, from more than one seat.
#[test]
#[ignore]
fn write_race3d_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("races3d");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, seat: u32, name: &str| {
        let img = super::super::frame_for(run, 768, 448, Some(seat));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    for (course, tag) in [(Course::Shaft, "shaft"), (Course::Cart, "cart"), (Course::Griffin, "griffin")] {
        let t = r3::track(course);
        // The title card.
        let mut run = racing(course, 4);
        classed(&mut run);
        race_mut(&mut run).t = HZ;
        save(&run, 1, &format!("{tag}-1-title"));
        // The gate drops.
        race_mut(&mut run).t = TITLE + 4;
        save(&run, 2, &format!("{tag}-2-go"));
        // Mid-race: the field strung out down the course.
        let mut run = racing(course, 4);
        classed(&mut run);
        let mid = match course {
            Course::Shaft => t.posts[3] + 2.0,
            Course::Cart => t.posts[1] + 10.0,
            Course::Griffin => t.posts[1] + 10.0,
        };
        let lanes = [0.0f32, -1.7, 1.7, 0.0];
        for (k, id) in [1u32, 2, 3, 4].into_iter().enumerate() {
            let (x, z) = match course {
                Course::Shaft => ([0.0, -0.6, 0.6, 0.2][k], 0.0),
                Course::Cart => (lanes[k], 0.0),
                Course::Griffin => ([0.0, -2.0, 2.2, 0.8][k], [0.0, 0.8, -0.6, 1.6][k]),
            };
            place(&mut run, id, mid + k as f32 * 3.2, x, z, 7.0);
            if course == Course::Cart {
                let r = race_mut(&mut run).riders.get_mut(&id).unwrap();
                r.lane = [1, 0, 2, 1][k];
            }
        }
        for k in 0..6 {
            race_mut(&mut run).t = TITLE + 300 + k;
            if course == Course::Griffin && k == 2 {
                run.step(&BTreeMap::from([(1, Input { fire: true, ..Default::default() })]));
            }
        }
        race_mut(&mut run).riders.get_mut(&1).unwrap().haul = 3;
        save(&run, 1, &format!("{tag}-3-mid-seat1"));
        save(&run, 4, &format!("{tag}-3-mid-seat4"));
        // A crash, with its joke.
        let mut run = racing(course, 3);
        classed(&mut run);
        match course {
            Course::Shaft => {
                let bridge = t.posts[3] + 8.0;
                place(&mut run, 1, bridge, 1.0, 0.0, 6.0);
                place(&mut run, 2, bridge - 4.0, 0.0, 0.0, 6.0);
                let left = BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]);
                for _ in 0..40 {
                    run.step(&left);
                    if race_mut(&mut run).riders[&1].crash > 0 && race_mut(&mut run).riders[&1].crash < r3::CRASH - 6 {
                        break;
                    }
                }
            }
            Course::Cart => {
                let timber = t.items.iter().find(|i| i.kind == ItemKind::Timber).unwrap();
                let lane = r3::LANES.iter().position(|&x| (x - timber.x).abs() < 0.01).unwrap() as u8;
                place(&mut run, 1, timber.s - 2.0, timber.x, 0.0, 8.5);
                race_mut(&mut run).riders.get_mut(&1).unwrap().lane = lane;
                place(&mut run, 2, timber.s - 7.0, 0.0, 0.0, 8.5);
                for _ in 0..30 {
                    run.step(&BTreeMap::new());
                    if race_mut(&mut run).riders[&1].crash > 0 && race_mut(&mut run).riders[&1].crash < r3::CRASH - 10 {
                        break;
                    }
                }
            }
            Course::Griffin => {
                let spire = t.blocks[0];
                place(&mut run, 1, spire.s - 3.0, (spire.x.0 + spire.x.1) / 2.0, 0.0, 10.0);
                place(&mut run, 2, spire.s - 16.0, 2.0, 1.0, 10.0);
                for _ in 0..30 {
                    run.step(&BTreeMap::new());
                    if race_mut(&mut run).riders[&1].crash > 0 && race_mut(&mut run).riders[&1].crash < r3::CRASH - 14 {
                        break;
                    }
                }
            }
        }
        save(&run, 1, &format!("{tag}-4-crash"));
        // The finish board.
        let mut run = racing(course, 4);
        classed(&mut run);
        for (k, id) in [3u32, 1, 4, 2].into_iter().enumerate() {
            // Home a minute or so in, a few seconds apart.
            race_mut(&mut run).t = TITLE + [58, 61, 66, 70][k] * HZ;
            let r = race_mut(&mut run).riders.get_mut(&id).unwrap();
            r.haul = [4, 2, 5, 1][k];
            r.crashes = k as u16;
            if k < 3 {
                r.s = t.finish - 0.01;
                r.vs = 8.0;
            } else {
                r.s = t.finish - 40.0;
            }
            run.step(&BTreeMap::new());
        }
        race_mut(&mut run).t = TITLE + RACE_TIME - 1;
        run.step(&BTreeMap::new());
        run.step(&BTreeMap::new());
        save(&run, 1, &format!("{tag}-5-board"));
        let _ = Ground::Floor;
    }
}

/// `cargo test --release race3d_frame_bench -- --ignored --nocapture`: ms
/// a frame for one seat's race view, four knights on the course, at the
/// Delve's frame sizes.
#[test]
#[ignore]
fn race3d_frame_bench() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let mut run = racing(course, 4);
        let t = r3::track(course);
        for (k, id) in [1u32, 2, 3, 4].into_iter().enumerate() {
            place(&mut run, id, t.posts[2] + k as f32 * 3.0, 0.0, 0.0, 7.0);
        }
        eprintln!("{course:?}: {} course triangles", build::course(course).tris.len());
        for (w, h) in [(NATIVE_W, NATIVE_H), (768, 448)] {
            let _ = super::super::frame_for(&run, w, h, Some(1));
            let n = 60;
            let start = std::time::Instant::now();
            for k in 0..n {
                race_mut(&mut run).t = TITLE + 10 + k;
                let _ = super::super::frame_for(&run, w, h, Some(1 + k % 4));
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
            eprintln!("{course:?} {w}x{h}: {ms:.2} ms a seat's frame ({:.0} fps)", 1000.0 / ms);
        }
    }
}

