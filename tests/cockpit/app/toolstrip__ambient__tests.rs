use super::*;
use std::collections::HashSet;

#[test]
fn catalog_loops_keep_moving_through_idle_and_transitions() {
    for width in [8, 30, 80, 160, 400] {
        let mut ambient = Ambient::default();
        let mut window = HashSet::new();
        // Two complete rotations, including every crossfade and wrap.
        for step in 0..(PRESETS.len() as u64 * 600) {
            let row = ambient.frame(width, Duration::from_millis(step * 200), MotionMode::Full);
            assert_eq!(row.chars().count(), width);
            assert!(row.chars().all(|c| ('\u{2800}'..='\u{28ff}').contains(&c)));
            assert!(row.chars().any(|c| c != '\u{2800}'));
            window.insert(row);
            if step % 10 == 9 {
                assert!(
                    window.len() > 1,
                    "static 2s window at {width} cells, step {step}"
                );
                window.clear();
            }
        }
    }
}

#[test]
fn modes_cache_bounds_and_reading_pause_are_respected() {
    let mut ambient = Ambient::default();
    let first = ambient.frame(80, Duration::ZERO, MotionMode::Full);
    assert!(Arc::ptr_eq(
        &first,
        &ambient.frame(80, Duration::from_millis(199), MotionMode::Full)
    ));
    assert!(Arc::ptr_eq(
        &first,
        &ambient.frame(80, Duration::from_millis(999), MotionMode::Reduced)
    ));
    assert_ne!(
        first,
        ambient.frame(80, Duration::from_secs(1), MotionMode::Reduced)
    );
    assert_eq!(first, ambient.frame(80, Duration::MAX, MotionMode::Off));
    assert_eq!(ambient.frame(0, Duration::MAX, MotionMode::Full).len(), 0);
    assert_eq!(
        ambient
            .frame(usize::MAX, Duration::MAX, MotionMode::Full)
            .chars()
            .count(),
        4_096
    );
    for width in [1, 2, 3] {
        assert_eq!(
            ambient
                .frame(width, Duration::MAX, MotionMode::Full)
                .chars()
                .count(),
            width
        );
    }
    let now = Instant::now();
    let held = ambient.row(80, now, MotionMode::Full, false);
    assert_eq!(
        held,
        ambient.row(80, now + Duration::from_secs(1), MotionMode::Full, true)
    );
    assert_ne!(
        held,
        ambient.row(80, now + Duration::from_secs(2), MotionMode::Full, false)
    );
}

#[test]
#[ignore = "explicit native bar generation/cache latency measurement"]
fn measured_catalog_bar_cost() {
    for width in [80, 160, 400] {
        let mut ambient = Ambient::default();
        let mut fresh = Vec::new();
        let mut cached = Vec::new();
        for step in 0..2_000 {
            let elapsed = Duration::from_millis(step * 200);
            let start = Instant::now();
            std::hint::black_box(ambient.frame(width, elapsed, MotionMode::Full));
            fresh.push(start.elapsed().as_nanos());
            let start = Instant::now();
            std::hint::black_box(ambient.frame(width, elapsed, MotionMode::Full));
            cached.push(start.elapsed().as_nanos());
        }
        for (kind, mut samples) in [("new-frame", fresh), ("cached", cached)] {
            samples.sort_unstable();
            eprintln!(
                "bar width={width} {kind} n={} p50_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3}",
                samples.len(),
                samples[1_000] as f64 / 1_000.0,
                samples[1_900] as f64 / 1_000.0,
                samples[1_980] as f64 / 1_000.0,
                samples[1_999] as f64 / 1_000.0
            );
        }
    }
}
