//! Reviewed helm poses driven by live cockpit state, with restrained idle glances.

use crate::ui::agent_panel::profile::AgentKey;
use crate::ui::views::agent_view::PortraitState;
use crate::ui::viz::lifecycle_viz::MotionMode;
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
#[path = "../../../tests/cockpit/app/helm__tests.rs"]
mod tests;
