//! Per-frame render time of `layers`, at points around the lap. Prints when
//! `ANGEL_PODRACER_TIMES` is set, so a plain `cargo test` stays quiet.
use super::super::*;
use crate::rules::Input;
use std::time::Instant;

/// Median, min and max of `n` renders of `layers(&run, Some(1))`, in ms.
fn time_frames(run: &Run, n: usize) -> (f64, f64, f64) {
    let _ = layers(run, Some(1));
    let mut ms: Vec<f64> = (0..n).map(|_| {
        let t = Instant::now();
        let _ = layers(run, Some(1));
        t.elapsed().as_secs_f64() * 1000.
    }).collect();
    ms.sort_by(|a, b| a.total_cmp(b));
    (ms[ms.len() / 2], ms[0], ms[ms.len() - 1])
}

#[test]
fn frame_time_around_the_lap() {
    if std::env::var_os("ANGEL_PODRACER_TIMES").is_none() { return; }
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let id = match std::env::var("ANGEL_PODRACER_COURSE").ok().as_deref() { Some("1") => rules::CourseId::Emberfall, Some("2") => rules::CourseId::Glassway, Some("3") => rules::CourseId::Moatwalk, _ => rules::CourseId::Proving };
    let mut race = rules::Session::blank_on(id);
    race.join_ids(1..=6);
    let _ = Input::default();
    super::route::select(id, 0);
    let lap = rules::course(id).lap();
    let cold = Instant::now();
    std::hint::black_box(world(30.));
    eprintln!("first frame on a cold cache: {:.1} ms", cold.elapsed().as_secs_f64() * 1000.);
    let mut medians = Vec::new();
    for k in 0..8 {
        let d = lap * k as f32 / 8. + 30.;
        for (slot, car) in race.cars.values_mut().enumerate() {
            car.d = d - slot as f32 * 7.;
            car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
            car.v = 40.;
        }
        run.knight_racers = Some(race.clone());
        let (med, lo, hi) = time_frames(&run, 20);
        let tris = world(d).tris.len();
        let w0 = Instant::now();
        for _ in 0..20 { std::hint::black_box(world(d)); }
        let wms = w0.elapsed().as_secs_f64() * 1000. / 20.;
        eprintln!("frame d={d:7.1}  median {med:6.2} ms  min {lo:6.2}  max {hi:6.2}  world tris {tris}  world() {wms:5.2} ms");
        medians.push(med);
    }
    let mean = medians.iter().sum::<f64>() / medians.len() as f64;
    eprintln!("MEAN of medians {mean:.2} ms");
}

fn course_by_digit(c: &str) -> rules::CourseId {
    match c { "1" => rules::CourseId::Emberfall, "2" => rules::CourseId::Glassway, "3" => rules::CourseId::Moatwalk, _ => rules::CourseId::Proving }
}

/// Courses timed against each other: `ANGEL_PODRACER_COMPARE=0,2` runs every point of both
/// laps in turn, round after round, so a busy machine slows them alike. Prints each course's
/// mean of per-point medians and its ratio to the first.
#[test]
fn frame_time_compared() {
    let Some(list) = std::env::var("ANGEL_PODRACER_COMPARE").ok() else { return };
    let ids: Vec<rules::CourseId> = list.split(',').map(course_by_digit).collect();
    let rounds: usize = std::env::var("ANGEL_PODRACER_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let mut run = Run::roundtable_for_test("knight_racers").unwrap();
    let mut races: Vec<rules::Session> = ids.iter().map(|&id| { let mut r = rules::Session::blank_on(id); r.join_ids(1..=6); r }).collect();
    let points = 8;
    // times[course][point] = every sample, ms
    let mut times: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); points]; ids.len()];
    for k in 0..points {
        for round in 0..rounds {
            for (c, &id) in ids.iter().enumerate() {
                let lap = rules::course(id).lap();
                let d = lap * k as f32 / points as f32 + 30.;
                for (slot, car) in races[c].cars.values_mut().enumerate() {
                    car.d = d - slot as f32 * 7.;
                    car.y = if slot % 2 == 0 { -3.2 } else { 3.2 };
                    car.v = 40.;
                }
                run.knight_racers = Some(races[c].clone());
                let _ = layers(&run, Some(1));
                if round == 0 { let _ = layers(&run, Some(1)); }
                let mut ms: Vec<f64> = (0..6).map(|_| { let t = Instant::now(); let _ = layers(&run, Some(1)); t.elapsed().as_secs_f64() * 1000. }).collect();
                ms.sort_by(|a, b| a.total_cmp(b));
                times[c][k].push(ms[ms.len() / 2]);
            }
        }
    }
    let mut means = Vec::new();
    for (c, &id) in ids.iter().enumerate() {
        let per_point: Vec<f64> = times[c].iter().map(|v| { let mut v = v.clone(); v.sort_by(|a, b| a.total_cmp(b)); v[v.len() / 2] }).collect();
        let mean = per_point.iter().sum::<f64>() / points as f64;
        means.push(mean);
        eprintln!("COMPARE {id:?}: mean {mean:.2} ms  points {:?}", per_point.iter().map(|v| (v * 10.).round() / 10.).collect::<Vec<_>>());
    }
    for (c, &id) in ids.iter().enumerate() { eprintln!("COMPARE ratio {id:?} = {:.3}x of {:?}", means[c] / means[0], ids[0]); }
}
