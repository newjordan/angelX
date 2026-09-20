//! Reviewed helm poses driven by live cockpit state, with restrained idle glances.

use crate::agent_profile::AgentKey;
use crate::agent_view::PortraitState;
use crate::lifecycle_viz::MotionMode;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Pose {
    Idle,
    Left,
    Right,
    Thinking,
    Acting,
    LongContext,
    Approval,
    Settled,
}

pub(crate) fn sheet(key: AgentKey) -> &'static str {
    match key {
        AgentKey::Turbo | AgentKey::GpuComp => "assets/agents/helms/turbo.png",
        AgentKey::Atlas => "assets/agents/helms/atlas.png",
        AgentKey::Sparky | AgentKey::Unknown => "assets/agents/helms/sparky.png",
        AgentKey::Apollo => "assets/agents/helms/apollo.png",
        AgentKey::Codex | AgentKey::MathGod => "assets/agents/helms/codex.png",
    }
}

pub(crate) fn pose(
    state: PortraitState,
    long_context: bool,
    motion: MotionMode,
    elapsed: Duration,
) -> Pose {
    match state {
        PortraitState::Blocked => Pose::Approval,
        PortraitState::Tool => Pose::Acting,
        PortraitState::Thinking if long_context => Pose::LongContext,
        PortraitState::Thinking => Pose::Thinking,
        PortraitState::Victory | PortraitState::Recovery => Pose::Settled,
        PortraitState::Idle if motion == MotionMode::Full => {
            // Brief, infrequent glances; never invent work or success in idle.
            match elapsed.as_millis() % 24_000 {
                7_000..9_000 => Pose::Left,
                17_000..19_000 => Pose::Right,
                _ => Pose::Idle,
            }
        }
        PortraitState::Idle => Pose::Idle,
    }
}

type Frames = Arc<Vec<image::RgbaImage>>;

/// Decode each selected sheet off the draw thread, then retain bounded 192px
/// frames. No original-sized sheet remains in the cache (five sheets < 6 MiB).
pub(crate) fn frame_image(path: &Path, pose: u8) -> Result<image::DynamicImage, String> {
    if pose >= 8 {
        return Err("unknown helm pose".into());
    }
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Frames>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cached = cache.lock().ok().and_then(|cache| cache.get(path).cloned());
    let frames = if let Some(frames) = cached {
        frames
    } else {
        let source = image::open(path)
            .map_err(|e| format!("decode helm: {e}"))?
            .to_rgba8();
        let (width, height) = source.dimensions();
        // The reviewed originals have odd dimensions. Integer grid boundaries
        // preserve every cell without assuming a divisible sheet or stretching.
        if width < 4 || height < 2 || u64::from(width) * u64::from(height) > 2_000_000 {
            return Err("helm sheet dimensions outside reviewed bounds".into());
        }
        let mut frames = Vec::with_capacity(8);
        for index in 0..8 {
            let col = index % 4;
            let row = index / 4;
            let x = col * width / 4;
            let y = row * height / 2;
            let w = (col + 1) * width / 4 - x;
            let h = (row + 1) * height / 2 - y;
            let cell = image::imageops::crop_imm(&source, x, y, w, h).to_image();
            frames.push(cell);
        }
        let frames = Arc::new(anchor_character_frames(&frames));
        if let Ok(mut cache) = cache.lock() {
            if cache.len() >= 5 && !cache.contains_key(path) {
                cache.clear();
            }
            cache.insert(path.to_path_buf(), Arc::clone(&frames));
        }
        frames
    };
    Ok(image::DynamicImage::ImageRgba8(
        frames[usize::from(pose)].clone(),
    ))
}

/// Character geometry comes exclusively from the reviewed alpha mask: dark
/// armor and face shadows are foreground, never mistaken for black background.
fn character_bounds(frame: &image::RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (mut left, mut top) = frame.dimensions();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in frame.enumerate_pixels() {
        if pixel[3] >= 32 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    (right > left && bottom > top).then_some((left, top, right, bottom))
}

/// Fit the character rather than the sheet cell. One scale across all poses
/// prevents breathing/zoom on state changes; each bust's right and bottom mask
/// edges remain anchored. A 3px transparent margin protects antialiased edges.
fn anchor_character_frames(frames: &[image::RgbaImage]) -> Vec<image::RgbaImage> {
    const SIZE: u32 = 192;
    const PAD: u32 = 3;
    let bounds: Vec<_> = frames.iter().map(character_bounds).collect();
    let extent = bounds
        .iter()
        .flatten()
        .map(|&(l, t, r, b)| (r - l).max(b - t))
        .max()
        .unwrap_or(1);
    let scale = (SIZE - PAD * 2) as f32 / extent as f32;
    frames
        .iter()
        .zip(bounds)
        .map(|(frame, bounds)| {
            let mut canvas = image::RgbaImage::new(SIZE, SIZE);
            if let Some((l, t, r, b)) = bounds {
                let crop = image::imageops::crop_imm(frame, l, t, r - l, b - t).to_image();
                let width = (((r - l) as f32 * scale).round() as u32).clamp(1, SIZE - PAD * 2);
                let height = (((b - t) as f32 * scale).round() as u32).clamp(1, SIZE - PAD * 2);
                let character = image::imageops::resize(
                    &crop,
                    width,
                    height,
                    image::imageops::FilterType::Lanczos3,
                );
                image::imageops::overlay(
                    &mut canvas,
                    &character,
                    i64::from(SIZE - PAD - width),
                    i64::from(SIZE - PAD - height),
                );
            }
            canvas
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_anchor_ignores_empty_canvas_and_preserves_dark_armor_and_scale() {
        let mut a = image::RgbaImage::new(80, 100);
        let mut b = image::RgbaImage::new(120, 120);
        for (frame, left, top) in [(&mut a, 10, 20), (&mut b, 50, 40)] {
            for y in top..top + 50 {
                for x in left..left + 30 {
                    frame.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
                }
            }
            frame.put_pixel(0, 0, image::Rgba([255, 255, 255, 1]));
        }
        let frames = anchor_character_frames(&[a, b]);
        assert_eq!(
            frames[0], frames[1],
            "canvas whitespace cannot move the character"
        );
        assert_eq!(character_bounds(&frames[0]), Some((77, 3, 189, 189)));
        assert_eq!(
            frames[0].get_pixel(100, 100)[3],
            255,
            "black armor stays opaque"
        );
    }

    #[test]
    fn live_state_owns_pose_and_motion_never_fabricates_work() {
        for motion in [MotionMode::Full, MotionMode::Reduced, MotionMode::Off] {
            for ms in [0, 8_000, 18_000, 100_000] {
                let at = Duration::from_millis(ms);
                assert_eq!(
                    pose(PortraitState::Blocked, true, motion, at),
                    Pose::Approval
                );
                assert_eq!(pose(PortraitState::Tool, true, motion, at), Pose::Acting);
                assert_eq!(
                    pose(PortraitState::Thinking, false, motion, at),
                    Pose::Thinking
                );
                assert_eq!(
                    pose(PortraitState::Thinking, true, motion, at),
                    Pose::LongContext
                );
                assert_eq!(
                    pose(PortraitState::Victory, true, motion, at),
                    Pose::Settled
                );
                if motion != MotionMode::Full {
                    assert_eq!(pose(PortraitState::Idle, true, motion, at), Pose::Idle);
                }
            }
        }
        assert_eq!(
            pose(
                PortraitState::Idle,
                false,
                MotionMode::Full,
                Duration::from_secs(8)
            ),
            Pose::Left
        );
        assert_eq!(
            pose(
                PortraitState::Idle,
                false,
                MotionMode::Full,
                Duration::from_secs(18)
            ),
            Pose::Right
        );
    }

    #[test]
    fn every_consumed_sheet_has_distinct_framed_poses() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for key in [
            AgentKey::Turbo,
            AgentKey::Atlas,
            AgentKey::Sparky,
            AgentKey::Apollo,
            AgentKey::Codex,
        ] {
            let path = root.join(sheet(key));
            let mut signatures = std::collections::HashSet::new();
            for pose in 0..8 {
                let frame = frame_image(&path, pose).unwrap().to_rgba8();
                assert_eq!(frame.dimensions(), (192, 192));
                // Optional review export uses the exact consumed runtime crop.
                // Ordinary execution never reads this test-only destination.
                if let Some(dir) = std::env::var_os("ANGEL_HELM_REVIEW_DIR") {
                    let dir = PathBuf::from(dir);
                    std::fs::create_dir_all(&dir).unwrap();
                    frame.save(dir.join(format!("{key:?}-{pose}.png"))).unwrap();
                }
                let (_, _, right, bottom) = character_bounds(&frame).expect("character alpha mask");
                assert!(
                    (188..=189).contains(&right),
                    "right anchor {key:?}/{pose}: {right}"
                );
                assert!(
                    (188..=189).contains(&bottom),
                    "bottom anchor {key:?}/{pose}: {bottom}"
                );
                // The complete bust stays off each vertical edge; this catches
                // a wrong sheet grid that borrows a neighboring character.
                let bright = |p: &image::Rgba<u8>| p[3] > 20 && p[0].max(p[1]).max(p[2]) > 45;
                assert!(
                    frame.pixels().filter(|p| bright(p)).count() > 1_000,
                    "{key:?}/{pose}"
                );
                assert!(
                    (0..192)
                        .filter(
                            |&y| bright(frame.get_pixel(0, y)) || bright(frame.get_pixel(191, y))
                        )
                        .count()
                        < 8,
                    "clipped {key:?}/{pose}"
                );
                use std::hash::{Hash, Hasher};
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                frame.as_raw().hash(&mut hash);
                signatures.insert(hash.finish());
            }
            assert_eq!(
                signatures.len(),
                8,
                "poses must be real distinct frames: {key:?}"
            );
            assert!(frame_image(&path, 8).is_err());
        }
    }
}
