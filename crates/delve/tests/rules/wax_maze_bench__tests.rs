//! The Wax Maze's frame time: four bots play the maze to its middle, then
//! seat one's screen is drawn again and again as the game goes on. Prints the
//! mean and the slowest tenth in milliseconds.
//!
//!   cargo test -p angel-delve --lib wax_maze_frame_time -- --ignored --nocapture
use super::wax_maze_bot::{fresh, keys};
use crate::render::overworld::arena::frame_for;
use std::time::Instant;

#[test]
#[ignore = "times the Wax Maze's frames"]
fn wax_maze_frame_time() {
    let mut r = fresh();
    for _ in 0..900 {
        let k = keys(&r);
        r.step(&k);
    }
    // A warm-up, so the scenery's bake is not counted.
    let _ = frame_for(&r, 384, 224, Some(1));
    let mut times: Vec<f64> = Vec::new();
    for _ in 0..240 {
        let k = keys(&r);
        r.step(&k);
        let t = Instant::now();
        let img = frame_for(&r, 384, 224, Some(1));
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&img);
    }
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let slow = times[times.len() * 9 / 10..].iter().sum::<f64>() / (times.len() - times.len() * 9 / 10) as f64;
    println!("wax maze frame: mean {mean:.2} ms, median {:.2} ms, slowest tenth {slow:.2} ms, tick {}", times[times.len() / 2], r.tick);
}
