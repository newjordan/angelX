//! Pennon Rush's film and its clock.
//!
//!   ANGEL_PENNON_REEL=<dir> cargo test -p angel-delve --lib write_pennon_reel -- --ignored --nocapture
//!
//! plays whole rounds with bots through the real `Run` and writes a frame a
//! tick (30 fps, 384 by 224 a seat, two seats side by side) of each moment
//! worth seeing into `<dir>/<name>/f0000.png ...`; `scripts` of the night
//! turn those into GIFs. `pennon_frame_time` times a mid-round screen.

use super::{Beat, FLOOR, LENGTH, Lane, Q, show};
use crate::render::overworld::{Img, arena::frame_for};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const NAMES: [&str; 6] = ["Gawain", "Lyra", "Bran", "Elin", "Mordred", "Tamsin"];

fn table(seats: u32) -> Run {
    let mut r = Run::pennon_for_test();
    for id in 2..=seats {
        r.join_seat(id, NAMES[(id - 1) as usize]);
    }
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert!(r.pennon.is_some());
    r
}

/// Each seat's hands: knight one plays it straight, knight two blunders into
/// the first breach, the rest start a little later.
fn hands(run: &Run, clumsy: bool) -> BTreeMap<u32, Input> {
    let Some(s) = run.pennon.as_ref() else { return BTreeMap::new() };
    if s.hold > 0 || s.left == 0 {
        return BTreeMap::new();
    }
    let elapsed = LENGTH - s.left;
    s.lanes
        .iter()
        .map(|(&id, l)| {
            let mut bot = l.bot_input(elapsed);
            if clumsy && id == 2 && (170..230).contains(&(l.x / Q)) && l.falls == 0 {
                bot.interact = false;
            }
            let late = (id.saturating_sub(2)) * 18;
            if id > 2 && elapsed < late {
                bot = Input::default();
            }
            (id, bot)
        })
        .collect()
}

#[derive(Default, Debug)]
struct Marks {
    go: u32,
    first_hit: u32,
    kill: u32,
    captain: u32,
    torn: u32,
    fall: u32,
    ward: u32,
    rally: u32,
    finish: u32,
    end: u32,
}

/// Play a table through and note when the moments came.
fn mark(seats: u32) -> Marks {
    let mut r = table(seats);
    let mut m = Marks::default();
    for t in 0..(LENGTH + 66 + 400) {
        let k = hands(&r, true);
        r.step(&k);
        let Some(s) = r.pennon.as_ref() else { break };
        if m.go == 0 && s.hold == 0 {
            m.go = t;
        }
        let one = &s.lanes[&1];
        if m.first_hit == 0 && one.foes.iter().any(|f| f.hp < if f.captain { 18 } else { 3 }) {
            m.first_hit = t;
        }
        if m.kill == 0 && one.foes[0].hp == 0 {
            m.kill = t;
        }
        if m.captain == 0 && one.foes[3].attack > 0 {
            m.captain = t;
        }
        if m.torn == 0 && one.foes[3].stage() > 0 {
            m.torn = t;
        }
        if m.fall == 0 && s.lanes.get(&2).is_some_and(|l| l.falls > 0) {
            m.fall = t;
        }
        if m.ward == 0 && one.pickups[0] {
            m.ward = t;
        }
        if m.rally == 0 && s.show.rallies > 0 {
            m.rally = t;
        }
        if m.finish == 0 && one.finish.is_some() {
            m.finish = t;
        }
        if m.end == 0 && s.left == 0 {
            m.end = t;
        }
        if m.end > 0 && t > m.end + 330 {
            break;
        }
    }
    m
}

struct Reel {
    root: PathBuf,
}

impl Reel {
    fn dir(&self, name: &str) -> PathBuf {
        let d = self.root.join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

fn shot(run: &Run, seats: &[u32], dir: &Path, n: u32) {
    let mut sheet = Img::black(384 * seats.len() as i32, 224);
    for (i, &seat) in seats.iter().enumerate() {
        if run.players.contains_key(&seat) {
            sheet.stamp(&frame_for(run, 384, 224, Some(seat)), 384 * i as i32, 0);
        }
    }
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Replay the same table and keep the frames of the windows `(name, from, to, seats)`.
fn film(reel: &Reel, seats: u32, windows: &[(&str, u32, u32, Vec<u32>)]) {
    let mut r = table(seats);
    let dirs: Vec<PathBuf> = windows.iter().map(|w| reel.dir(w.0)).collect();
    let mut counts = vec![0u32; windows.len()];
    let last = windows.iter().map(|w| w.2).max().unwrap_or(0);
    for t in 0..=last {
        for (i, (_, a, b, who)) in windows.iter().enumerate() {
            if t >= *a && t < *b {
                shot(&r, who, &dirs[i], counts[i]);
                counts[i] += 1;
            }
        }
        let k = hands(&r, true);
        r.step(&k);
    }
    for (i, w) in windows.iter().enumerate() {
        println!("{}: {} frames", w.0, counts[i]);
    }
}

#[test]
#[ignore = "writes the film of Pennon Rush"]
fn write_pennon_reel() {
    let root = PathBuf::from(std::env::var("ANGEL_PENNON_REEL").expect("ANGEL_PENNON_REEL"));
    let reel = Reel { root };
    let m = mark(4);
    println!("{m:?}");
    let back = |t: u32, n: u32| t.saturating_sub(n);
    film(
        &reel,
        4,
        &[
            ("start", 0, m.go + 70, vec![1, 2]),
            ("hit", back(m.first_hit, 22), m.first_hit + 54, vec![1, 2]),
            ("kill", back(m.kill, 14), m.kill + 80, vec![1, 2]),
            ("captain", back(m.captain, 16), m.captain + 96, vec![1, 2]),
            ("torn", back(m.torn, 18), m.torn + 70, vec![1, 2]),
            ("fall", back(m.fall, 28), m.fall + 70, vec![2, 1]),
            ("ward", back(m.ward, 14), m.ward + 48, vec![1, 2]),
            ("rally", back(m.rally, 10), m.rally + 70, vec![1, 2]),
            ("finish", back(m.finish, 20), m.end + 54 + 210, vec![1, 2]),
        ],
    );
    // The clock running out, five seconds from the horn.
    let mut r = table(4);
    {
        let s = r.pennon.as_mut().unwrap();
        s.hold = 0;
        s.show.t = 66;
    }
    for _ in 0..300 {
        r.step(&BTreeMap::new());
    }
    r.pennon.as_mut().unwrap().left = 5 * 30 + 6;
    let dir = reel.dir("timeup");
    let mut n = 0;
    for t in 0..(5 * 30 + 6 + 54 + 150) {
        if t >= 20 {
            shot(&r, &[1, 2], &dir, n);
            n += 1;
        }
        let k = hands(&r, false);
        r.step(&k);
    }
    // One knight alone, and a table of six.
    for (name, seats) in [("solo", 1u32), ("six", 6)] {
        let m = mark(seats);
        film(&reel, seats, &[(name, 0, m.go + 160, if seats == 1 { vec![1] } else { vec![1, 6] })]);
        let m2 = mark(seats);
        film(&reel, seats, &[(&format!("{name}-end"), back(m2.end, 5), m2.end + 54 + 150, if seats == 1 { vec![1] } else { vec![1, 6] })]);
    }
}

/// The clock: a mid-round screen drawn at four seats, the quickest of five
/// draws of each of sixty ticks, the mean of those.
#[test]
#[ignore = "times the screen"]
fn pennon_frame_time() {
    let mut r = table(4);
    for _ in 0..(66 + 420) {
        let k = hands(&r, false);
        r.step(&k);
    }
    // Warm the caches (the art, the scenery) before the clock starts.
    let _ = frame_for(&r, 384, 224, Some(1));
    let mut best = Vec::new();
    for _ in 0..60 {
        let k = hands(&r, false);
        r.step(&k);
        let mut quickest = f64::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            let im = frame_for(&r, 384, 224, Some(1));
            quickest = quickest.min(t.elapsed().as_secs_f64() * 1000.0);
            assert_eq!((im.w, im.h), (384, 224));
        }
        best.push(quickest);
    }
    best.sort_by(f64::total_cmp);
    let mean = best.iter().sum::<f64>() / best.len() as f64;
    println!("pennon frame: mean {mean:.2} ms, cheapest {:.2}, dearest {:.2}", best[0], best[best.len() - 1]);
    let _ = (FLOOR, Beat::Walk, show::HOLD, std::mem::size_of::<Lane>());
}
