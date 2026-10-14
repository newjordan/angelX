//! The same frames natively: `cargo run --release -p angel-delve-smoke`.

use angel_delve_smoke::{SCENES, frame, scene, step, wire_client, wire_host, wire_scene};
use std::time::Instant;

fn main() {
    for (art_v2, (w, h)) in [(0, (384, 224)), (1, (768, 448))] {
        for (kind, name) in SCENES.iter().enumerate() {
            scene(kind as u32, art_v2);
            let cold = Instant::now();
            let sum = frame(w, h);
            let cold = cold.elapsed().as_secs_f64() * 1000.0;
            let n = 60;
            let mut warm = 0.0;
            let mut last = sum;
            for _ in 0..n {
                step(1);
                let t = Instant::now();
                last = frame(w, h);
                warm += t.elapsed().as_secs_f64() * 1000.0;
            }
            let warm = warm / f64::from(n);
            println!(
                "native {w}x{h} v{} {name}: first {cold:.1} ms, then {warm:.2} ms a frame, sums {sum:08x} {last:08x}",
                art_v2 + 1
            );
        }
    }
    for (kind, name) in SCENES.iter().enumerate() {
        let whole = wire_scene(kind as u32);
        // Taken in for a while first, as the wasm run is.
        for _ in 0..240 {
            wire_host();
            wire_client();
        }
        let n = 120;
        let (mut took, mut bytes) = (0.0, 0);
        for _ in 0..n {
            bytes += wire_host();
            let t = Instant::now();
            wire_client();
            took += t.elapsed().as_secs_f64() * 1000.0;
        }
        println!(
            "native wire {name}: whole {whole} bytes, then {} bytes and {:.3} ms a tick to take in",
            bytes / n,
            took / f64::from(n)
        );
    }
}
