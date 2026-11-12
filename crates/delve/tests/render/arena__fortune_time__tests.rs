use super::*;
use crate::realm::Spoils;
use crate::render::overworld::arena::frame_for;
use crate::rules::fortune;
use crate::rules::home::{BUY_HOLD, Home, Station};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;

/// The wheel's frame cost: the hall rendered through a whole spin at four seats.
#[test]
#[ignore = "measures the picture's cost"]
fn fortune_frame_times() {
    let mut home = Home { deepest: 3, ..Home::default() };
    home.levels.insert(Station::Wheel, 4);
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    for id in 2..=4 {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.enter_for_test(1);
    let (c, r, w, h) = fortune::LEVER;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * 2.0;
    hero.y = (r as f32 + h as f32 / 2.0) * 2.0;
    run.step(&BTreeMap::new());
    let hold = |on| BTreeMap::from([(1, Input { interact: on, ..Default::default() })]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&hold(true));
    }
    run.step(&hold(false));
    let mut idle = std::time::Duration::ZERO;
    let mut spin = std::time::Duration::ZERO;
    let (mut ni, mut ns) = (0u32, 0u32);
    for t in 0..400u32 {
        run.step(&BTreeMap::new());
        let at = std::time::Instant::now();
        let img = frame_for(&run, 384, 224, Some(1));
        let took = at.elapsed();
        std::hint::black_box(&img);
        if t < 330 {
            spin += took;
            ns += 1;
        } else {
            idle += took;
            ni += 1;
        }
    }
    println!("fortune frame: spin and landing {:.2} ms, then idle {:.2} ms", spin.as_secs_f64() * 1e3 / ns as f64, idle.as_secs_f64() * 1e3 / ni as f64);
}
