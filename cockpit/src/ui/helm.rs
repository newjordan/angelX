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

pub(crate) fn sheet(key: AgentKey) -> Option<&'static str> {
    Some(match key {
        AgentKey::Codex => "assets/realm/avatars/round-table/sheet/sol.png",
        AgentKey::Luna => "assets/realm/avatars/round-table/sheet/luna.png",
        AgentKey::Astra => "assets/realm/avatars/round-table/sheet/astra.png",
        AgentKey::Daybreak => return None,
        AgentKey::Grok => "assets/realm/avatars/round-table/sheet/grok.png",
        AgentKey::DeepSeek => "assets/realm/avatars/round-table/sheet/deepseek.png",
        AgentKey::Glm => "assets/realm/avatars/round-table/sheet/glm.png",
        AgentKey::Kimi => "assets/realm/avatars/round-table/sheet/kimi.png",
        AgentKey::Qwen => "assets/realm/avatars/round-table/sheet/qwen.png",
        AgentKey::Muse => "assets/realm/avatars/round-table/sheet/muse.png",
        AgentKey::LongCat => "assets/realm/avatars/round-table/sheet/longcat.png",
        AgentKey::Hy => "assets/realm/avatars/round-table/sheet/hy.png",
        AgentKey::Nemotron => "assets/realm/avatars/round-table/sheet/nemotron.png",
        AgentKey::Gemma => "assets/realm/avatars/round-table/sheet/gemma.png",
        AgentKey::Inkling => "assets/realm/avatars/round-table/sheet/inkling.png",
        AgentKey::Laguna => "assets/realm/avatars/round-table/sheet/laguna.png",
        AgentKey::North => "assets/realm/avatars/round-table/sheet/north.png",
        AgentKey::Unknown => "assets/agents/helms/sparky.png",
    })
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

fn frame_cache() -> &'static Mutex<HashMap<PathBuf, Frames>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Frames>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn frame_size(path: &Path, decode: bool) -> Option<(u32, u32)> {
    let cached = frame_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(path).map(|frames| frames[0].dimensions()));
    match cached {
        Some(size) => Some(size),
        None if decode => frame_image(path, 0)
            .ok()
            .map(|frame| (frame.width(), frame.height())),
        None => None,
    }
}

pub(crate) fn frame_image(path: &Path, pose: u8) -> Result<image::DynamicImage, String> {
    if pose >= 8 {
        return Err("unknown helm pose".into());
    }
    let cache = frame_cache();
    let cached = cache.lock().ok().and_then(|cache| cache.get(path).cloned());
    let frames = if let Some(frames) = cached {
        frames
    } else {
        let source = image::open(path)
            .map_err(|e| format!("decode helm: {e}"))?
            .to_rgba8();
        let (width, height) = source.dimensions();
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
        let frames = Arc::new(if is_pixel_sheet(path) {
            anchor_pixel_frames(&frames)
        } else if is_registered_sheet(path) {
            anchor_registered_frames(&frames)
        } else {
            anchor_character_frames(&frames)
        });
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

pub(crate) fn is_pixel_sheet(path: &Path) -> bool {
    path.to_string_lossy().contains("round-table/sheet/")
}

pub(crate) fn is_registered_sheet(path: &Path) -> bool {
    path.to_string_lossy().contains("onboarding/setup/")
}

pub(crate) fn anchor_registered_frames(frames: &[image::RgbaImage]) -> Vec<image::RgbaImage> {
    const SIZE: u32 = 512;
    let Some((l, t, r, b)) = frames
        .iter()
        .filter_map(character_bounds)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
    else {
        return frames.to_vec();
    };
    let (w, h) = (r - l, b - t);
    let scale = SIZE as f32 / w.max(h) as f32;
    let (tw, th) = (
        ((w as f32 * scale).round() as u32).clamp(1, SIZE),
        ((h as f32 * scale).round() as u32).clamp(1, SIZE),
    );
    frames
        .iter()
        .map(|frame| {
            let crop = image::imageops::crop_imm(frame, l, t, w, h).to_image();
            image::imageops::resize(&crop, tw, th, image::imageops::FilterType::Lanczos3)
        })
        .collect()
}

fn anchor_pixel_frames(frames: &[image::RgbaImage]) -> Vec<image::RgbaImage> {
    let bounds: Vec<_> = frames.iter().map(character_bounds).collect();
    let (width, height) = bounds
        .iter()
        .flatten()
        .fold((0, 0), |(w, h), &(l, t, r, b)| (w.max(r - l), h.max(b - t)));
    if width == 0 || height == 0 {
        return frames.to_vec();
    }
    frames
        .iter()
        .zip(bounds)
        .map(|(frame, bounds)| {
            let mut canvas = image::RgbaImage::new(width, height);
            if let Some((l, t, r, b)) = bounds {
                let crop = image::imageops::crop_imm(frame, l, t, r - l, b - t).to_image();
                image::imageops::replace(
                    &mut canvas,
                    &crop,
                    i64::from(width - (r - l)),
                    i64::from(height - (b - t)),
                );
            }
            canvas
        })
        .collect()
}

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
