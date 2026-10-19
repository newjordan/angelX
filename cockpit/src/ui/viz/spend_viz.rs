//! Display-only gold-coin acknowledgements for metered input milestones.
//!
//! A burst is sampled lazily: even a large milestone needs at most eight
//! live poses, not one allocation or image upload per million tokens.

use crate::ui::viz::lifecycle_viz::MotionMode;
use ratatui::{Frame, layout::Rect, style::Color};

const FULL_DURATION_SECS: f32 = 1.35;
const REDUCED_DURATION_SECS: f32 = 0.90;
const OFF_DURATION_SECS: f32 = 1.20;
const COIN_WIDTH: u16 = 6;
const COIN_HEIGHT: u16 = 3;
const STAGGER_SECS: f64 = 0.18;
const MAX_LIVE_COINS: u64 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CoinPose {
    pub(crate) area: Rect,
    pub(crate) face_visible: bool,
    /// Continuous foreshortening; the milled edge remains visible edge-on.
    pub(crate) face_scale: f32,
}

pub(crate) const fn duration_secs(motion: MotionMode) -> f32 {
    match motion {
        MotionMode::Full => FULL_DURATION_SECS,
        MotionMode::Reduced => REDUCED_DURATION_SECS,
        MotionMode::Off => OFF_DURATION_SECS,
    }
}

pub(crate) fn milestone_coins(input_tokens: u64) -> u64 {
    input_tokens / 1_000_000
}

pub(crate) fn burst_duration_secs(coins: u64, motion: MotionMode) -> f64 {
    if coins == 0 {
        0.0
    } else if motion == MotionMode::Off {
        f64::from(OFF_DURATION_SECS)
    } else {
        f64::from(duration_secs(motion)) + coins.saturating_sub(1) as f64 * STAGGER_SECS
    }
}

pub(crate) fn burst_active(elapsed_secs: f64, coins: u64, motion: MotionMode) -> bool {
    elapsed_secs.is_finite() && elapsed_secs >= 0.0
        && elapsed_secs < burst_duration_secs(coins, motion)
}

pub(crate) fn is_active(elapsed_secs: f32, motion: MotionMode) -> bool {
    elapsed_secs.is_finite() && elapsed_secs >= 0.0 && elapsed_secs < duration_secs(motion)
}

pub(crate) fn pose(stage: Rect, elapsed_secs: f32, motion: MotionMode) -> Option<CoinPose> {
    if !is_active(elapsed_secs, motion) || stage.width < 3 || stage.height == 0 {
        return None;
    }
    let width = COIN_WIDTH.min(stage.width);
    let height = COIN_HEIGHT.min(stage.height);
    let progress = (elapsed_secs / duration_secs(motion)).clamp(0.0, 1.0);
    let travel = stage.height.saturating_sub(height.saturating_add(1)).min(6);
    let lift = match motion {
        MotionMode::Full => (4.0 * progress * (1.0 - progress) * f32::from(travel)).round() as u16,
        MotionMode::Reduced => u16::from(travel > 0 && (0.20..0.75).contains(&progress)),
        MotionMode::Off => 0,
    };
    let x = stage.right().saturating_sub(width.saturating_add(1)).max(stage.x);
    let y = stage.bottom().saturating_sub(height.saturating_add(1))
        .max(stage.y).saturating_sub(lift).max(stage.y);
    let face_scale = match motion {
        MotionMode::Full => (progress * std::f32::consts::TAU * 2.0).cos().abs().max(0.08),
        MotionMode::Reduced | MotionMode::Off => 1.0,
    };
    Some(CoinPose {
        area: Rect::new(x, y, width, height),
        face_visible: face_scale > 0.22,
        face_scale,
    })
}

/// Return just the coins currently in flight, with stable lanes and no history
/// traversal. Motion-off uses a static fan (and a count badge for large totals).
pub(crate) fn poses(stage: Rect, elapsed_secs: f64, coins: u64, motion: MotionMode) -> Vec<CoinPose> {
    if !burst_active(elapsed_secs, coins, motion) {
        return Vec::new();
    }
    let lanes = (stage.width / (COIN_WIDTH + 1)).clamp(1, 3);
    let (first, last) = if motion == MotionMode::Off {
        (0, coins.min(u64::from(lanes)) - 1)
    } else {
        let last = ((elapsed_secs / STAGGER_SECS).floor() as u64).min(coins - 1);
        (last.saturating_sub(MAX_LIVE_COINS - 1), last)
    };
    let mut result = Vec::with_capacity(MAX_LIVE_COINS as usize);
    for index in first..=last {
        let age = if motion == MotionMode::Off { 0.0 } else { elapsed_secs - index as f64 * STAGGER_SECS };
        if let Some(mut coin) = pose(stage, age as f32, motion) {
            let offset = (index % u64::from(lanes)) as u16 * (COIN_WIDTH + 1);
            coin.area.x = coin.area.x.saturating_sub(offset).max(stage.x);
            result.push(coin);
        }
    }
    result
}

/// Procedural minted gold: rounded face, raised double rim, dark inset,
/// embossed currency stamp, and reeded thickness visible throughout a flip.
/// Coordinates are -1..1. No external asset or image worker is involved.
pub(crate) fn gold_pixel(x: f32, y: f32, face_scale: f32) -> Option<[u8; 3]> {
    let scale = face_scale.clamp(0.08, 1.0);
    let thickness = 0.13 * (1.0 - scale * 0.45);
    let fx = (x + thickness * 0.5) / (0.82 * scale);
    let fy = y / 0.88;
    let radius = fx.hypot(fy);
    if radius > 1.0 {
        let edge_x = ((x + thickness * 0.5).abs() - thickness).max(0.0) / (0.82 * scale);
        if edge_x.hypot(fy) > 1.0 { return None; }
        let reed = (y * 56.0).sin() * 0.12;
        let light = (0.72 - y * 0.18 + reed).clamp(0.4, 1.0);
        return Some([235.0, 159.0, 34.0].map(|c| (c * light) as u8));
    }
    let light = (0.88 - fx * 0.14 - fy * 0.18).clamp(0.50, 1.12);
    let mut rgb = if radius > 0.84 {
        [255.0, 223.0, 112.0] // raised polished rim
    } else if radius > 0.72 {
        [183.0, 111.0, 19.0] // engraved ring, not a flat yellow blob
    } else {
        [249.0, 183.0, 42.0]
    };
    const STAMP: [u8; 7] = [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100];
    if fx.abs() < 0.42 && fy.abs() < 0.57 {
        let col = ((fx + 0.42) / 0.84 * 5.0) as usize;
        let row = ((fy + 0.57) / 1.14 * 7.0) as usize;
        if col < 5 && row < 7 && STAMP[row] & (1 << (4 - col)) != 0 {
            rgb = [153.0, 86.0, 12.0];
        }
    }
    Some(rgb.map(|c| (c * light).clamp(0.0, 255.0) as u8))
}

pub(crate) fn render_coin(frame: &mut Frame<'_>, coin: CoinPose) {
    const BITS: [(u16, u16, u8); 8] = [
        (0, 0, 0), (0, 1, 1), (0, 2, 2), (1, 0, 3),
        (1, 1, 4), (1, 2, 5), (0, 3, 6), (1, 3, 7),
    ];
    let area = coin.area.intersection(frame.area());
    for row in area.y..area.bottom() {
        for col in area.x..area.right() {
            let mut bits = 0u32;
            let mut rgb = [0u32; 3];
            let mut count = 0;
            for (dx, dy, bit) in BITS {
                let x = (f32::from((col - coin.area.x) * 2 + dx) + 0.5)
                    / f32::from(coin.area.width * 2) * 2.0 - 1.0;
                let y = (f32::from((row - coin.area.y) * 4 + dy) + 0.5)
                    / f32::from(coin.area.height * 4) * 2.0 - 1.0;
                if let Some(color) = gold_pixel(x, y, coin.face_scale) {
                    bits |= 1 << bit;
                    for (sum, c) in rgb.iter_mut().zip(color) { *sum += u32::from(c); }
                    count += 1;
                }
            }
            if count > 0 {
                let [r, g, b] = rgb.map(|c| (c / count) as u8);
                frame.buffer_mut()[(col, row)]
                    .set_char(char::from_u32(0x2800 + bits).unwrap())
                    .set_fg(Color::Rgb(r, g, b));
            }
        }
    }
}

pub(crate) fn format_input_tokens(input_tokens: u64) -> String {
    let millions = input_tokens / 1_000_000;
    let tenths = (input_tokens % 1_000_000) / 100_000;
    format!("{millions}.{tenths}M")
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/spend_viz__tests.rs"]
mod tests;
