//! Bounded Stage acknowledgement for metered input milestones.
//!
//! The pose is pure and fixed-geometry so Kitty reuses one decoded protocol
//! while the terminal position moves. Operational turn state is never read or
//! changed here.

use crate::lifecycle_viz::MotionMode;
use ratatui::layout::Rect;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const FULL_DURATION_SECS: f32 = 1.35;
const REDUCED_DURATION_SECS: f32 = 0.90;
const OFF_DURATION_SECS: f32 = 1.20;
const COIN_WIDTH: u16 = 10;
const COIN_HEIGHT: u16 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CoinPose {
    pub(crate) area: Rect,
    pub(crate) face_visible: bool,
}

pub(crate) fn asset_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("hud")
            .join("spend-coin.png")
    })
    .as_path()
}

pub(crate) const fn duration_secs(motion: MotionMode) -> f32 {
    match motion {
        MotionMode::Full => FULL_DURATION_SECS,
        MotionMode::Reduced => REDUCED_DURATION_SECS,
        MotionMode::Off => OFF_DURATION_SECS,
    }
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
    let right = stage.x.saturating_add(stage.width);
    let bottom = stage.y.saturating_add(stage.height);
    let x = right.saturating_sub(width.saturating_add(1)).max(stage.x);
    let y = bottom
        .saturating_sub(height.saturating_add(1))
        .max(stage.y)
        .saturating_sub(lift)
        .max(stage.y);
    let face_visible = match motion {
        MotionMode::Full => (progress * std::f32::consts::TAU * 2.0).cos().abs() > 0.22,
        MotionMode::Reduced | MotionMode::Off => true,
    };

    Some(CoinPose {
        area: Rect::new(x, y, width, height),
        face_visible,
    })
}

pub(crate) fn format_input_tokens(input_tokens: u64) -> String {
    let millions = input_tokens / 1_000_000;
    let tenths = (input_tokens % 1_000_000) / 100_000;
    format!("{millions}.{tenths}M")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_motion_tosses_with_fixed_geometry_and_a_flip() {
        let stage = Rect::new(4, 3, 30, 14);
        let start = pose(stage, 0.0, MotionMode::Full).unwrap();
        let apex = pose(stage, FULL_DURATION_SECS / 2.0, MotionMode::Full).unwrap();
        assert_eq!((start.area.width, start.area.height), (10, 5));
        assert_eq!((apex.area.width, apex.area.height), (10, 5));
        assert!(apex.area.y < start.area.y);
        assert!((0..40).any(|step| {
            !pose(
                stage,
                FULL_DURATION_SECS * step as f32 / 40.0,
                MotionMode::Full,
            )
            .unwrap()
            .face_visible
        }));
    }

    #[test]
    fn reduced_and_off_never_rotate_the_coin() {
        let stage = Rect::new(0, 0, 20, 8);
        assert!(pose(stage, 0.4, MotionMode::Reduced).unwrap().face_visible);
        assert!(pose(stage, 0.4, MotionMode::Off).unwrap().face_visible);
    }

    #[test]
    fn pose_expires_and_formats_the_milestone() {
        assert!(pose(Rect::new(0, 0, 20, 8), FULL_DURATION_SECS, MotionMode::Full).is_none());
        assert_eq!(format_input_tokens(1_000_000), "1.0M");
        assert_eq!(format_input_tokens(2_500_000), "2.5M");
    }
}
