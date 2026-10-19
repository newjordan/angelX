//! Isolated arcade stage cost; no game scene or physics is changed.
use angel_delve::render::arcade;
use std::{hint::black_box, time::Instant};
fn main() {
    let cold = Instant::now();
    black_box(arcade::frame("knight", "idle", 0, 0));
    let cold = cold.elapsed().as_secs_f64() * 1000.0;
    println!("arcade 384x224 decode + first stage: {cold:.3} ms");
    for kit in ["knight", "fx"] {
        let states = arcade::states(kit);
        let mut samples = Vec::new();
        for tick in 0..240 {
            let state = &states[tick % states.len()];
            let t = Instant::now();
            black_box(arcade::frame(kit, state, (tick * 30) as u64, tick % 4));
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "arcade {kit} 384x224 warm stage + native braille: median {:.3} ms, p95 {:.3} ms",
            samples[120], samples[228]
        );
    }
}
