//! The live-patch probe: does a rules edit in `angel-delve` reach a running
//! program through ra-jit, without a restart?

use std::time::{Duration, Instant};

fn main() {
    let start = Instant::now();
    let mut live = ra_jit_live::Live::start().ok();
    println!("live: {}", if live.is_some() { "started" } else { "unavailable" });

    let mut tick: u64 = 0;
    loop {
        if let Some(outcome) = live.as_mut().and_then(|it| it.poll()) {
            println!("ra-jit: {outcome}");
        }
        tick += 1;
        println!("tick={tick} up={:.1}s probe_value={}", start.elapsed().as_secs_f64(), angel_delve::probe_value());
        std::thread::sleep(Duration::from_millis(500));
    }
}
