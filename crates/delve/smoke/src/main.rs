//! The same frames natively: `cargo run --release -p angel-delve-smoke`.

use angel_delve_smoke::{SCENES, force_rage, force_ravage, frame, scene, step, wire_client, wire_host, wire_scene};
use std::time::Instant;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--entry") {
        let kind: u32 = args[2].parse().unwrap();
        let kit: u32 = args[3].parse().unwrap();
        let (w, h) = if kit == 0 { (384, 224) } else { (768, 448) };
        scene(kind, kit);
        if args.iter().any(|s| s == "--party") { angel_delve_smoke::party(); }
        let state_before = angel_delve_smoke::state_hash(0);
        let prep = Instant::now();
        if args.iter().any(|s| s == "--warm") {
            angel_delve_smoke::warm_entry(kind);
        }
        let preparation_ms = prep.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(state_before, angel_delve_smoke::state_hash(0), "entry preparation changed the wire state");
        let live = Instant::now();
        angel_delve_smoke::frame_live(w, h);
        let live_first_ms = live.elapsed().as_secs_f64() * 1000.0;
        let mut times = Vec::new();
        let mut sums = Vec::new();
        for n in 0..240 {
            if n > 0 && n % 2 == 0 {
                step(1);
            }
            let t = Instant::now();
            sums.push(frame(w, h));
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        println!(
            "{}",
            serde_json::json!({"game": SCENES[kind as usize], "kit": kit + 1, "state_before":state_before, "state_after":angel_delve_smoke::state_hash(0), "preparation_ms": preparation_ms, "live_first_ms": live_first_ms, "ms": times, "sums": sums,
            "runtime_banks": angel_delve::render::overworld::arena::slot_art::runtime_banks()})
        );
        return;
    }
    let party = args.iter().any(|arg| arg == "--party");
    let rage = std::env::args().any(|arg| arg == "--rage");
    let ravage = std::env::args().any(|arg| arg == "--ravage");
    for (art_v2, (w, h)) in [(0, (384, 224)), (1, (768, 448))] {
        for (kind, name) in SCENES.iter().enumerate() {
            if rage && !(9..=20).contains(&kind) { continue; }
            if ravage && *name != "late-fee-leviathan" { continue; }
            scene(kind as u32, art_v2);
            if party { angel_delve_smoke::party(); }
            if rage { force_rage(); }
            if ravage { force_ravage(); }
            let prep = Instant::now();
            if !args.iter().any(|arg| arg == "--unprepared") {
                angel_delve_smoke::warm_entry(kind as u32);
            }
            let preparation_ms = prep.elapsed().as_secs_f64() * 1000.0;
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
                "native {w}x{h} v{} {name}: first {cold:.1} ms, then {warm:.2} ms a frame, preparation {preparation_ms:.1} ms, sums {sum:08x} {last:08x}",
                art_v2 + 1
            );
        }
    }
    if rage || ravage { return; }
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
