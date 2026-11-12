//! Lanternward's stage frame time on the recorded moments: the quickest and
//! the middle of sixty draws each (the quickest is the one a busy machine
//! does not move). `cargo test -p angel-delve --lib lanternward_frame_time --
//! --ignored --nocapture --test-threads=1`.
use super::layers;
use crate::rules::Run;
use std::time::Instant;

#[test]
#[ignore = "prints how long the stage takes to draw"]
fn lanternward_frame_time() {
    let moments = [
        ("stair", 1), ("lock", 1), ("slash3", 1), ("parry", 1), ("lamp-lit", 1), ("lamp-flare", 1), ("gallery-tell", 1),
        ("vault-flare", 1), ("warden", 2), ("warden-beam", 1), ("warden-slam", 2), ("warden-open", 2), ("warden-dark", 4),
        ("warden-dead", 1), ("finale", 2),
    ];
    let (mut quick, mut middle) = (0.0, 0.0);
    for (m, seats) in moments {
        let r = Run::lanternward_at_for_test(m, seats);
        for _ in 0..6 {
            let _ = layers(&r, Some(1));
        }
        let mut ms: Vec<f64> = (0..80)
            .map(|_| {
                let t = Instant::now();
                std::hint::black_box(layers(&r, Some(1)));
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        ms.sort_by(f64::total_cmp);
        quick += ms[0];
        middle += ms[ms.len() / 2];
        println!("{m:14} {seats}  quickest {:7.3} ms  middle {:7.3} ms", ms[0], ms[ms.len() / 2]);
    }
    println!("mean quickest {:.3} ms, mean middle {:.3} ms over {} moments", quick / moments.len() as f64, middle / moments.len() as f64, moments.len());
}
