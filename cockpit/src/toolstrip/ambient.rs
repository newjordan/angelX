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
#[path = "../../../tests/cockpit/app/toolstrip__ambient__tests.rs"]
mod tests;
