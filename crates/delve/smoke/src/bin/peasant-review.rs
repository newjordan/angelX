//! Frame cost of saved four-seat Peasant stones/desertion states, and their quiet room.
use angel_delve::{render::overworld::arena, rules::Run, settings::{self, Settings}};
use std::{hint::black_box, path::PathBuf, time::Instant};

fn main() {
    let dir = PathBuf::from(std::env::args_os().nth(1).expect("Peasant review directory"));
    settings::set(Settings { art_v2: true, feel_ledger: false, open_floors: false });
    for name in ["stones", "desertion"] {
        let run: Run = serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        assert_eq!(run.players.len(), 4);
        let mut quiet = run.clone();
        quiet.projectiles.clear();
        quiet.scarecrows.clear();
        for h in quiet.players.values_mut() { h.flee_ticks = 0; }
        for e in &mut quiet.enemies { e.sleep_until = 0; }
        for (label, r) in [("quiet", &quiet), (name, &run)] {
            arena::prepare_entry(r, Some(1));
            for (w, h) in [(384,224),(768,448)] {
                black_box(arena::frame_for(r,w,h,Some(1)));
                let mut times = Vec::new();
                for _ in 0..120 {
                    let start = Instant::now();
                    black_box(arena::frame_for(r,w,h,Some(1)));
                    times.push(start.elapsed().as_secs_f64()*1000.0);
                }
                times.sort_by(f64::total_cmp);
                println!("{name}/{label} {w}x{h}: p50 {:.3} p95 {:.3} max {:.3} ms",times[60],times[114],times[119]);
            }
        }
    }
}
