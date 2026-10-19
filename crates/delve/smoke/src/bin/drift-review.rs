//! Reproducible four-seat Touge inputs, saved frames and per-seat frame cost.
use angel_delve::{
    render::overworld::arena,
    rules::{
        Run,
        race3d::{self as r3, Course},
    },
    settings::{self, Settings},
};
use std::{collections::BTreeMap, hint::black_box, path::Path, time::Instant};
fn save(dir: &Path, name: &str, run: &Run) {
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec(run).unwrap(),
    )
    .unwrap();
    for (label, w, h) in [("native", 384, 224), ("halfblock", 96, 56)] {
        let img = arena::frame_for(run, w, h, Some(1));
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}-{label}.ppm")), out).unwrap();
    }
    for seat in 1..=4 {
        black_box(arena::frame_for(run, 384, 224, Some(seat)));
        let mut times = Vec::new();
        for _ in 0..120 {
            let start = Instant::now();
            black_box(arena::frame_for(run, 384, 224, Some(seat)));
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{name} seat {seat} 384x224 p50 {:.3} p95 {:.3} max {:.3} ms",
            times[60], times[114], times[119]
        );
    }
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args_os().nth(1).expect("drift-review OUT"));
    std::fs::create_dir_all(&dir).unwrap();
    settings::set(Settings {
        art_v2: true,
        feel_ledger: false,
        open_floors: false,
    });
    let mut run = arena::race3d::testing::racing(Course::Touge, 4);
    run.floor_plates.clear();
    let mut seen = [false; 3];
    // Seat one supplies recorded driver inputs; the other seats use the same
    // public bot controller as the development host's three native clients.
    let mut inputs_file = Vec::new();
    loop {
        let race = run.race3d.as_ref().unwrap();
        let input: BTreeMap<_, _> = race
            .riders
            .keys()
            .map(|&id| (id, r3::drift_bot(race, id)))
            .collect();
        inputs_file.push(input.clone());
        run.step(&input);
        let race = run.race3d.as_ref().unwrap();
        let me = &race.riders[&1];
        let stage = me.drift.stage();
        if stage > 0 && !seen[stage - 1] && me.drift.sliding {
            seen[stage - 1] = true;
            save(&dir, ["blue", "orange", "purple"][stage - 1], &run);
        }
        if race.done {
            break;
        }
    }
    assert!(seen.iter().all(|s| *s));
    save(&dir, "podium", &run);
    std::fs::write(
        dir.join("inputs.json"),
        serde_json::to_vec(&inputs_file).unwrap(),
    )
    .unwrap();
    let race = run.race3d.as_ref().unwrap();
    println!(
        "finish {:?}",
        race.riders
            .iter()
            .map(|(id, r)| (id, r.finish, r.crashes, r.drift.score))
            .collect::<Vec<_>>()
    );
}
