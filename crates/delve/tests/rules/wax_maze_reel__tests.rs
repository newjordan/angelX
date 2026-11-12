//! The Wax Maze's proof reels: the real `Run` played through by four bots
//! that eat the nearest wax, drink the holy water when a wraith closes and
//! chase the pale ones, recorded on seat one's own screen (and seat two's)
//! at every tick (30 fps) in four windows: the opening, the holy water, a
//! touch, and the finish with its podium. It changes no game code and writes
//! only under the directory it is given.
//!
//!   ANGEL_WAX_REEL=<dir> cargo test -p angel-delve --lib write_wax_maze_reel -- --ignored --nocapture
//!
//! `scripts/wax_gifs.py` makes the GIFs from the frames.
use super::*;
use crate::render::overworld::{Img, arena::frame_for};
use super::wax_maze_bot::{fresh, fresh_for, keys};
use crate::rules::folly::TITLE;
use std::path::PathBuf;

/// The ticks that matter, found by playing the maze through once unseen: a
/// step, the seat, where (tile sixteenths), and for a catch its chain.
#[derive(Debug, Default)]
struct Marks {
    first_splash: Option<(u32, u32)>,
    catches: Vec<(u32, u32, (i32, i32), u8)>,
    /// Each touch, with the seat nearest to it that was not touched.
    touches: Vec<(u32, u32, (i32, i32), Option<u32>)>,
    end: Option<u32>,
}

fn survey() -> Marks {
    let mut r = fresh();
    let mut marks = Marks::default();
    let (mut caught, mut touched) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for t in 0..4_000u32 {
        let k = keys(&r);
        r.step(&k);
        let Some(m) = r.folly.as_ref() else { break };
        if marks.first_splash.is_none() && let Some(s) = m.show.splashes.first() {
            marks.first_splash = Some((t, u32::from(s.who)));
        }
        for c in &m.show.catches {
            if caught.insert((c.born, c.king)) {
                marks.catches.push((t, u32::from(c.who), c.at, c.chain));
            }
        }
        for h in &m.show.touches {
            if touched.insert((h.born, h.who)) {
                let near = r
                    .players
                    .iter()
                    .filter(|(id, _)| **id != u32::from(h.who))
                    .min_by_key(|(_, p)| {
                        let (x, y) = ((p.x / TILE_UNITS * 16.0) as i32, (p.y / TILE_UNITS * 16.0) as i32);
                        (x - h.at.0).abs() + (y - h.at.1).abs()
                    })
                    .map(|(&id, _)| id);
                marks.touches.push((t, u32::from(h.who), h.at, near));
            }
        }
        if m.done && marks.end.is_none() {
            marks.end = Some(t);
        }
        if marks.end.is_some_and(|e| t > e + 330) {
            break;
        }
    }
    marks
}

/// How far from the maze's top and bottom edges a spot (tile sixteenths)
/// lies, in tiles: the middle of the maze shows a moment best.
fn middle(at: (i32, i32), rows: i32) -> i32 {
    (at.1 / 16).min(rows - at.1 / 16)
}

fn save(dir: &PathBuf, n: usize, sheet: &Img) {
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Play again, writing a frame of `seat`'s screen every tick in `[from, to)`.
fn record(root: &PathBuf, name: &str, from: u32, to: u32, seat: u32, also: Option<u32>) {
    record_for(root, name, 4, from, to, seat, also);
}

/// `record` for a table of `seats` knights.
fn record_for(root: &PathBuf, name: &str, seats: u32, from: u32, to: u32, seat: u32, also: Option<u32>) {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = fresh_for(seats);
    let mut n = 0;
    for t in 0..to {
        let k = keys(&r);
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

#[test]
#[ignore = "writes the Wax Maze's proof reels"]
fn write_wax_maze_reel() {
    let Some(root) = std::env::var_os("ANGEL_WAX_REEL") else { return };
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let marks = survey();
    println!("{marks:?}");
    let only = std::env::var("REEL_ONLY").unwrap_or_default();
    let want = |n: &str| only.is_empty() || only.split(',').any(|x| x == n);
    if want("start") {
        record(&root, "start", 0, TITLE + 140, 1, Some(2));
    }
    if want("holy") && let Some((t, who)) = marks.first_splash {
        record(&root, "holy", t.saturating_sub(20), t + 150, who, None);
    }
    // The biggest catch, the most in the maze's middle.
    if want("catch") && let Some(&(t, who, _, _)) = marks.catches.iter().max_by_key(|&&(_, _, at, chain)| (chain, middle(at, 28))) {
        record(&root, "catch", t.saturating_sub(60), t + 70, who, None);
    }
    // A touch, from the touched seat's screen and from the nearest friend's.
    if want("touch") && let Some(&(t, who, _, near)) = marks.touches.iter().max_by_key(|&&(_, _, at, _)| middle(at, 28)) {
        record(&root, "touch", t.saturating_sub(50), t + 70, who, None);
        if let Some(near) = near {
            record(&root, "touch-near", t.saturating_sub(50), t + 70, near, None);
        }
    }
    // One knight alone, and a full table of six.
    if want("solo") {
        record_for(&root, "solo", 1, 0, TITLE + 150, 1, None);
    }
    if want("six") {
        record_for(&root, "six", 6, 0, TITLE + 150, 6, None);
    }
    if want("finish") && let Some(end) = marks.end {
        record(&root, "finish", end.saturating_sub(40), end + 330, 1, Some(2));
    }
}
