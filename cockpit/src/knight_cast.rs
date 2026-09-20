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
            let path =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/knights/cast-v1.png");
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
mod tests {
    use super::*;

    #[test]
    #[ignore = "manual art review: fourteen normalized cast poses"]
    fn dump_knight_cast_for_review() {
        let out = std::env::var("CAST_DUMP_DIR").expect("CAST_DUMP_DIR");
        std::fs::create_dir_all(&out).unwrap();
        for side in [Side::RedLeft, Side::BlueRight] {
            for pose in [
                Pose::Mounted,
                Pose::StepA,
                Pose::StepB,
                Pose::Study,
                Pose::WorkA,
                Pose::WorkB,
                Pose::Rest,
            ] {
                let source = sprite(side, pose).unwrap();
                source.save(format!("{out}/{side:?}-{pose:?}.png")).unwrap();
            }
        }
    }

    #[test]
    fn cast_identity_and_activity_do_not_rotate_with_elapsed_idle_time() {
        for tick in [0, 9, 360, 720, 36000, u64::MAX] {
            assert_eq!(
                FrameKey::at(Activity::Rest, tick),
                FrameKey::at(Activity::Rest, 0)
            );
            assert_eq!(
                FrameKey::at(Activity::Study, tick).pose(Side::BlueRight),
                Pose::Study
            );
            assert_eq!(
                FrameKey::at(Activity::Study, tick).pose(Side::RedLeft),
                Pose::Mounted
            );
        }
        for tick in 0..STEP_TICKS {
            assert_eq!(
                FrameKey::at(Activity::Travel, 0),
                FrameKey::at(Activity::Travel, tick)
            );
        }
        assert_ne!(
            FrameKey::at(Activity::Travel, 0),
            FrameKey::at(Activity::Travel, STEP_TICKS)
        );
    }

    #[test]
    fn all_fourteen_frames_have_ink_padding_and_one_hoof_baseline() {
        let frames = frames().expect("bundled original cast must decode");
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(frame.dimensions(), (FRAME_W, FRAME_H));
            let ink: Vec<_> = frame
                .enumerate_pixels()
                .filter(|(_, _, p)| p[3] >= 128)
                .collect();
            assert!(ink.len() > 1000, "empty cast frame {i}");
            assert!(
                ink.iter()
                    .all(|(x, y, _)| *x > 4 && *x < FRAME_W - 4 && *y > 4 && *y < FRAME_H - 4),
                "clipped frame {i}"
            );
            let foot = ink.iter().map(|(_, y, _)| *y).max().unwrap();
            assert!((334..=341).contains(&foot), "frame {i} ground at {foot}");
        }
    }

    #[test]
    fn paired_cast_leaves_the_road_clear_and_never_stretches_the_horse() {
        for (w, h) in [(24, 16), (96, 72), (200, 304), (400, 72)] {
            let mut frame = RgbaImage::new(w, h);
            composite(&mut frame, FrameKey::at(Activity::Travel, 0));
            for x in w * 2 / 5..w * 3 / 5 {
                for y in 0..h {
                    assert_eq!(frame.get_pixel(x, y)[3], 0);
                }
            }
            let red: u64 = frame
                .enumerate_pixels()
                .filter(|(x, _, p)| {
                    *x < w / 2 && u16::from(p[0]) > u16::from(p[2]) * 2 && p[3] > 128
                })
                .count() as u64;
            let blue: u64 = frame
                .enumerate_pixels()
                .filter(|(x, _, p)| {
                    *x >= w / 2 && u16::from(p[2]) > u16::from(p[0]) * 2 && p[3] > 128
                })
                .count() as u64;
            if w >= 96 {
                assert!(red > 0 && blue > 0, "heraldry lost at {w}x{h}");
            }
        }
    }
}
