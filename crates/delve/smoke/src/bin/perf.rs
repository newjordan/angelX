//! Where a floor's time goes: the load, the tick, and the picture, apart.
//! `cargo run --release -p angel-delve-smoke --bin perf -- <scene>`

use angel_delve_smoke::{SCENES, frame, scene, state_json, step};
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let kind: u32 = std::env::args().nth(1).and_then(|it| it.parse().ok()).unwrap_or(0);
    let (w, h) = (384u32, 224u32);

    let at = Instant::now();
    scene(kind);
    angel_delve_smoke::party();
    let load = at.elapsed();

    let at = Instant::now();
    for _ in 0..900 {
        step(1);
    }
    let ticks = at.elapsed();

    let mut frames = Vec::with_capacity(240);
    for _ in 0..240 {
        let at = Instant::now();
        black_box(frame(w, h));
        frames.push(at.elapsed().as_secs_f64() * 1e3);
    }
    frames.sort_by(f64::total_cmp);

    let at = Instant::now();
    let saved = state_json().len();
    let save = at.elapsed();

    let at = Instant::now();
    black_box(state_json());
    let save_again = at.elapsed();

    println!(
        "{}",
        serde_json::json!({
            "scene": SCENES[kind as usize],
            "load_ms": load.as_secs_f64() * 1e3,
            "tick_ms_900": ticks.as_secs_f64() * 1e3,
            "tick_ms_each": ticks.as_secs_f64() * 1e3 / 900.0,
            "frame_p50_ms": frames[frames.len() / 2],
            "frame_p99_ms": frames[frames.len() * 99 / 100],
            "frame_max_ms": frames[frames.len() - 1],
            "state_bytes": saved,
            "state_write_ms": save.as_secs_f64() * 1e3,
            "state_write_again_ms": save_again.as_secs_f64() * 1e3,
        })
    );
}
