//! One stable, original sprite cast for the world and tournament.
//! Authored figures are composited before Dotmax screening, never shown as PNG plates.

use image::RgbaImage;
use std::sync::OnceLock;

pub(crate) const STEP_TICKS: u64 = 9;
const FRAME_W: u32 = 320;
const FRAME_H: u32 = 350;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Side {
    RedLeft,
    BlueRight,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Activity {
    Rest,
    Travel,
    Study,
    Craft,
    Council,
    Guard,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Pose {
    Mounted,
    StepA,
    StepB,
    Study,
    WorkA,
    WorkB,
    Rest,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FrameKey {
    activity: Activity,
    step: bool,
}

impl FrameKey {
    pub(crate) fn at(activity: Activity, tick: u64) -> Self {
        let period = if activity == Activity::Travel {
            STEP_TICKS
        } else {
            STEP_TICKS * 2
        };
        let animated = matches!(
            activity,
            Activity::Travel | Activity::Craft | Activity::Guard
        );
        Self {
            activity,
            step: animated && (tick / period) % 2 == 1,
        }
    }

    pub(crate) fn pose(self, side: Side) -> Pose {
        let work = if self.step { Pose::WorkB } else { Pose::WorkA };
        match (self.activity, side) {
            (Activity::Travel, _) => {
                if self.step {
                    Pose::StepB
                } else {
                    Pose::StepA
                }
            }
            (Activity::Study, Side::BlueRight) | (Activity::Council, _) => Pose::Study,
            (Activity::Craft, _) => work,
            (Activity::Guard, Side::BlueRight) => work,
            (Activity::Guard, Side::RedLeft) => Pose::Rest,
            _ => Pose::Mounted,
        }
    }
}

static FRAMES: OnceLock<Option<[RgbaImage; 14]>> = OnceLock::new();

pub(crate) fn warm() {
    let _ = frames();
}
pub(crate) fn available() -> bool {
    frames().is_some()
}

fn frames() -> Option<&'static [RgbaImage; 14]> {
    FRAMES
        .get_or_init(|| {
            let path = crate::runtime_paths::cockpit_dir().join("assets/knights/cast-v1.png");
            let source = image::ImageReader::open(path)
                .ok()?
                .decode()
                .ok()?
                .to_rgba8();
            if source.dimensions() != (1278, 1230) {
                return None;
            }
            Some(std::array::from_fn(|index| {
                let blue = index >= 7;
                let pose = index % 7;
                // Hand-reviewed crop bounds, not an assumed uniform atlas grid.
                // Mounted rows share the exact hoof baseline and pixel scale.
                let (col, y, height, dest_y) = if pose < 3 {
                    (pose, if blue { 630 } else { 20 }, 320, 25)
                } else {
                    (
                        pose - 3,
                        if blue { 950 } else { 345 },
                        if blue { 275 } else { 270 },
                        if blue { 92 } else { 73 },
                    )
                };
                let x = col as u32 * 320;
                let width = FRAME_W.min(source.width() - x);
                let mut frame = RgbaImage::new(FRAME_W, FRAME_H);
                for sy in 0..height {
                    for sx in 0..width {
                        let pixel = source.get_pixel(x + sx, y + sy);
                        // Generated antialias fringe is sub-visible dust at this scale.
                        // Preserve opaque black ink inside armor and horse silhouettes.
                        if pixel[3] >= 96 && dest_y + sy < FRAME_H {
                            let mut ink = *pixel;
                            // Preserve black ink while letting the two heraldic colors
                            // survive the world tone curve at a 25-dot sprite height.
                            if blue
                                && u16::from(ink[2]) * 10 > u16::from(ink[0]) * 13
                                && ink[2] > ink[1]
                            {
                                ink[2] = (f32::from(ink[2]) * 1.35).min(185.0) as u8;
                            } else if !blue
                                && u16::from(ink[0]) * 10 > u16::from(ink[1]) * 14
                                && ink[0] > ink[2]
                            {
                                ink[0] = (f32::from(ink[0]) * 1.25).min(185.0) as u8;
                            }
                            frame.put_pixel(sx, dest_y + sy, ink);
                        }
                    }
                }
                frame
            }))
        })
        .as_ref()
}

pub(crate) fn sprite(side: Side, pose: Pose) -> Option<&'static RgbaImage> {
    let index = match pose {
        Pose::Mounted => 0,
        Pose::StepA => 1,
        Pose::StepB => 2,
        Pose::Study => 3,
        Pose::WorkA => 4,
        Pose::WorkB => 5,
        Pose::Rest => 6,
    };
    frames().map(|frames| &frames[index + if side == Side::BlueRight { 7 } else { 0 }])
}

/// Fit both axes by the same scale. Side identity is fixed in screen space;
/// the center of the road remains clear even in narrow panes.
pub(crate) fn composite(frame: &mut RgbaImage, key: FrameKey) {
    let (w, h) = frame.dimensions();
    if w < 24 || h < 16 {
        return;
    }
    let scale = ((w as f32 * 0.29) / FRAME_W as f32).min((h as f32 * 0.44) / FRAME_H as f32);
    let sw = (FRAME_W as f32 * scale).round().max(1.0) as u32;
    let sh = (FRAME_H as f32 * scale).round().max(1.0) as u32;
    for side in [Side::RedLeft, Side::BlueRight] {
        let Some(sprite) = sprite(side, key.pose(side)) else {
            continue;
        };
        let resized = image::imageops::resize(sprite, sw, sh, image::imageops::FilterType::Nearest);
        let x = if side == Side::RedLeft {
            2
        } else {
            w.saturating_sub(sw + 2)
        };
        image::imageops::overlay(
            frame,
            &resized,
            i64::from(x),
            i64::from(h.saturating_sub(sh + 1)),
        );
    }
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/knight_cast__tests.rs"]
mod tests;
