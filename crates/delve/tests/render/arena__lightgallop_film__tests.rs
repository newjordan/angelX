//! Lightgallop's film strips: the real run, bots at the table, a frame every
//! second tick (15 fps) of one or two seats' own screens, written as
//! `<dir>/<strip>/f0000.png` for `scripts/audit_gifs.py`.
//!
//!   ANGEL_LG_FILM=<dir> [LG_ONLY=start,loop] \
//!     cargo test -p angel-delve --lib write_lightgallop_film -- --ignored --nocapture
//!
//! A strip is found, then filmed: the first pass rides until the moment it
//! wants (a hit, a loop, the warden), the second rides the same ticks again
//! (the sim is deterministic) and records around it.
use super::*;
use crate::render::overworld::arena::frame_for;
use crate::rules::lightgallop::legacy::SessionOracle;
use crate::rules::lightgallop::{level::level, oracle::Route, rider::Numbers};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn table(seats: u32) -> Run {
    let mut r = Run::lightgallop_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

/// Every seat's bot, the later seats held at the gate a little so the pack spreads.
fn bots(r: &Run, stagger: u32) -> BTreeMap<u32, Input> {
    let s = r.lightgallop.as_ref().expect("a ride");
    s.seats
        .keys()
        .map(|&id| {
            let route = if id % 2 == 0 { Route::Lower } else { Route::Upper };
            let held = s.elapsed < (id - 1) * stagger;
            (id, if held { Input::default() } else { s.drive(id, route, &Numbers::default()) })
        })
        .collect()
}

fn step(r: &mut Run, stagger: u32) {
    let k = bots(r, stagger);
    r.step(&k);
}

struct Strip {
    dir: PathBuf,
    n: u32,
    views: Vec<u32>,
}

impl Strip {
    fn new(root: &Path, name: &str, views: &[u32]) -> Strip {
        let dir = root.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Strip { dir, n: 0, views: views.to_vec() }
    }
    fn shot(&mut self, r: &Run) {
        let mut sheet = Img::black(384 * self.views.len() as i32, 224);
        for (i, &v) in self.views.iter().enumerate() {
            if r.players.contains_key(&v) {
                sheet.stamp(&frame_for(r, 384, 224, Some(v)), 384 * i as i32, 0);
            }
        }
        image::save_buffer(
            self.dir.join(format!("f{:04}.png", self.n)),
            &sheet.rgb_bytes(),
            sheet.w as u32,
            sheet.h as u32,
            image::ColorType::Rgb8,
        )
        .unwrap();
        self.n += 1;
    }
}

fn want(name: &str) -> bool {
    std::env::var("LG_ONLY").map_or(true, |o| o.split(',').any(|x| x == name))
}

/// Ride from the gate until `found` says so (at most `limit` ticks); the
/// tick it did.
fn find(seats: u32, stagger: u32, limit: u32, found: impl Fn(&Run) -> bool) -> u32 {
    let mut r = table(seats);
    for _ in 0..limit {
        if found(&r) {
            return r.lightgallop.as_ref().unwrap().elapsed;
        }
        step(&mut r, stagger);
    }
    panic!("the moment never came in {limit} ticks");
}

/// Ride to `from`, then film `ticks` ticks, a frame every second tick.
fn film(root: &Path, name: &str, views: &[u32], seats: u32, stagger: u32, from: u32, ticks: u32) -> Run {
    let mut r = table(seats);
    while r.lightgallop.as_ref().unwrap().elapsed < from {
        step(&mut r, stagger);
    }
    let mut strip = Strip::new(root, name, views);
    for t in 0..ticks {
        if t % 2 == 0 {
            strip.shot(&r);
        }
        step(&mut r, stagger);
    }
    println!("{name}: {} frames from tick {from}", strip.n);
    r
}

#[test]
#[ignore = "writes Lightgallop's film strips"]
fn write_lightgallop_film() {
    let Some(root) = std::env::var_os("ANGEL_LG_FILM") else {
        return;
    };
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let lvl = level();

    // The gate: the title, the pack leaving, the first slope.
    if want("start") {
        film(&root, "start", &[1, 2], 4, 24, 0, 200);
    }
    // The downhill run: speed lines, dust, the pace building.
    if want("speed") {
        let t = find(4, 24, 3000, |r| r.lightgallop.as_ref().unwrap().seats[&1].rider.x / 256 > 560);
        film(&root, "speed", &[1], 4, 24, t, 120);
    }
    // The first loop.
    if want("loop") {
        let t = find(4, 24, 4000, |r| r.lightgallop.as_ref().unwrap().seats[&1].rider.looping);
        film(&root, "loop", &[1], 4, 24, t.saturating_sub(40), 110);
    }
    // A hit.
    if want("hit") {
        let t = find(4, 24, 6000, |r| r.lightgallop.as_ref().unwrap().seats[&1].rider.hurt > 0);
        film(&root, "hit", &[1], 4, 24, t.saturating_sub(40), 100);
    }
    // The warden and the bell.
    if want("warden") {
        let t = find(4, 24, 8000, |r| {
            let s = r.lightgallop.as_ref().unwrap();
            s.foes.iter().zip(&lvl.foes).any(|(f, spot)| spot.kind == crate::rules::lightgallop::world::FoeKind::Warden && f.hp == 0)
        });
        film(&root, "warden", &[1], 4, 24, t.saturating_sub(50), 130);
    }
    // The finish: the bell and the podium.
    if want("finish") {
        let t = find(4, 24, 9000, |r| r.lightgallop.as_ref().unwrap().bell_t.is_some());
        film(&root, "finish", &[1], 4, 24, t.saturating_sub(30), 420);
    }
}

#[test]
#[ignore = "times Lightgallop's frames"]
fn time_lightgallop_frames() {
    use std::time::Instant;
    // Four seats, the same ride, a window of frames at four places on the road.
    for (name, from) in [("gate", 10u32), ("downhill", 330), ("midway", 1400), ("late", 2600)] {
        let mut r = table(4);
        while r.lightgallop.as_ref().unwrap().elapsed < from {
            step(&mut r, 24);
        }
        let mut ms = Vec::new();
        for _ in 0..90 {
            step(&mut r, 24);
            let t0 = Instant::now();
            let img = frame_for(&r, 384, 224, Some(1));
            ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(&img);
        }
        ms.sort_by(|a, b| a.total_cmp(b));
        println!("frame-time {name}: median {:.2} ms, p90 {:.2} ms, worst {:.2} ms", ms[ms.len() / 2], ms[ms.len() * 9 / 10], ms[ms.len() - 1]);
    }
}
