//! Display-only ambience from the bundled Dotmax catalog. No work progress
//! or result is inferred from this clock; status receipts own those facts.

use crate::lifecycle_viz::MotionMode;
use dotmax::progress::{BarContext, ProgressStyle};
use std::sync::Arc;
use std::time::{Duration, Instant};

const PRESETS: [(&str, &str); 6] = [
    ("sinewave", "sw-traveling"),
    ("ocean", "bubbles-rising"),
    ("aurora", "polar-arc"),
    ("nature", "falling-leaves"),
    ("sinewave", "sw-sine-scroll"),
    ("space", "satellite-dish"),
];
const SLOT_MS: u128 = 60_000;
const BLEND_MS: u128 = 2_000;

thread_local! {
    // Catalog objects are stateless but its trait is not Send. Resolve just
    // these six styles once on the drawing thread, never all 644 per frame.
    static STYLES: Vec<Box<dyn ProgressStyle>> = PRESETS.iter().map(|(theme, name)| {
        dotmax::progress::styles_for_theme(theme).into_iter()
            .find(|style| style.name() == *name)
            .expect("bundled ambient Dotmax preset")
    }).collect();
}

#[derive(Default)]
pub(super) struct Ambient {
    last: Option<Instant>,
    elapsed: Duration,
    key: Option<(usize, u128)>,
    row: Arc<str>,
}

impl Ambient {
    pub(super) fn row(
        &mut self,
        width: usize,
        now: Instant,
        motion: MotionMode,
        paused: bool,
    ) -> Arc<str> {
        if let Some(last) = self.last
            && !paused
            && motion.animates()
        {
            // Returning from a hidden pane never jumps across missed loops.
            self.elapsed = self.elapsed.saturating_add(
                now.saturating_duration_since(last)
                    .min(Duration::from_secs(1)),
            );
        }
        self.last = Some(now);
        self.frame(width, self.elapsed, motion)
    }

    fn frame(&mut self, width: usize, elapsed: Duration, motion: MotionMode) -> Arc<str> {
        let width = width.min(4_096);
        let ms = match motion {
            MotionMode::Full => elapsed.as_millis() / 200 * 200,
            MotionMode::Reduced => elapsed.as_millis() / 1_000 * 1_000,
            MotionMode::Off => 0,
        } % (SLOT_MS * PRESETS.len() as u128);
        if self.key == Some((width, ms)) {
            return Arc::clone(&self.row);
        }
        let row = if width == 0 {
            String::new()
        } else {
            let slot = (ms / SLOT_MS) as usize;
            let phase = ms % SLOT_MS;
            // All presets receive a fixed extent. Only display time advances;
            // there is no percentage, synthetic progress, or completion event.
            STYLES.with(|styles| {
                let render = |index: usize, time_ms: u128| {
                    let ctx = BarContext::new(1.0, time_ms as f32 / 1_000.0 * 0.35, width, 1);
                    dotmax::progress::render_lines(styles[index].as_ref(), &ctx)
                        .expect("bounded bundled Dotmax preset")
                        .into_iter()
                        .next()
                        .unwrap_or_default()
                };
                let current = render(slot, phase + BLEND_MS);
                if phase < SLOT_MS - BLEND_MS {
                    return current;
                }
                // Slowly replace dots with the next actual preset. Never
                // blank the bar or add a bright transition plate.
                let blend = phase - (SLOT_MS - BLEND_MS);
                let next = render((slot + 1) % styles.len(), blend);
                current
                    .chars()
                    .zip(next.chars())
                    .enumerate()
                    .map(|(cell, (a, b))| {
                        let a = u32::from(a).saturating_sub(0x2800) as u8;
                        let b = u32::from(b).saturating_sub(0x2800) as u8;
                        let mut bits = 0_u8;
                        for bit in 0..8 {
                            let threshold = ((cell * 37 + bit * 97) % BLEND_MS as usize) as u128;
                            bits |= (if threshold < blend { b } else { a }) & (1 << bit);
                        }
                        char::from_u32(0x2800 + u32::from(bits)).unwrap()
                    })
                    .collect()
            })
        };
        self.key = Some((width, ms));
        self.row = Arc::from(row);
        Arc::clone(&self.row)
    }
}

#[cfg(test)]
mod tests {
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
}
