//! Knight-journey ceremony art: cropped JPEG sheets → subdued colored Braille.
//!
//! Manifest playback is the source of truth for selected rectangles and frame
//! order. Source pixels never appear in the world pane; this module only emits
//! colored Braille cells. Original jousting remains the preview/fallback when
//! this path declines (jousting `/tourney calibrate` names, missing assets).

use crate::viz::lifecycle_viz::{CeremonyKind, FPS, FRAME_COUNT, MotionMode};
use crate::term::art::{self, ColoredBrailleCell, DMD_PALETTE, ImageRegion};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const SHEET_CACHE_LIMIT: usize = 6;
const MARGIN_CELLS: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Scene {
    Service,
    Study,
    Craft,
    Perseverance,
    Dragon,
    Guardian,
}

impl Scene {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "service" => Some(Self::Service),
            "study" => Some(Self::Study),
            "craft" => Some(Self::Craft),
            "perseverance" => Some(Self::Perseverance),
            "dragon" => Some(Self::Dragon),
            "guardian" => Some(Self::Guardian),
            _ => None,
        }
    }

    #[allow(dead_code)]
    const fn name(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::Study => "study",
            Self::Craft => "craft",
            Self::Perseverance => "perseverance",
            Self::Dragon => "dragon",
            Self::Guardian => "guardian",
        }
    }

    const fn is_success(self) -> bool {
        matches!(self, Self::Service | Self::Guardian)
    }
}

/// Slash-command names that preview a knight-journey sheet (not jousting).
pub(crate) fn is_journey_calibration_name(raw: &str) -> bool {
    Scene::parse(raw).is_some()
}

/// Ceremony kinds that may show service/guardian. Stopped/failed/cleared never
/// qualify; only independently verified goal/loop completion does.
const fn is_verified_success(kind: CeremonyKind) -> bool {
    matches!(kind, CeremonyKind::GoalDone | CeremonyKind::LoopDone)
}

pub(crate) fn scene_for(kind: CeremonyKind, label: &str) -> Option<Scene> {
    if is_jousting_preview(label) {
        return None;
    }
    if is_verified_success(kind) {
        return Some(scene_for_kind(kind));
    }
    // Task labels are arbitrary operator text. A non-success kind may still
    // preview a named activity sheet (study/craft/…); it must never unlock
    // service/guardian from the words in the label.
    if let Some(scene) = named_scene(label)
        && !scene.is_success()
    {
        return Some(scene);
    }
    Some(scene_for_kind(kind))
}

fn scene_for_kind(kind: CeremonyKind) -> Scene {
    match kind {
        CeremonyKind::GoalDone => Scene::Service,
        CeremonyKind::LoopDone => Scene::Guardian,
        CeremonyKind::GoalSet | CeremonyKind::LoopStart => Scene::Study,
        CeremonyKind::LoopEscalate => Scene::Craft,
        CeremonyKind::LoopPaused | CeremonyKind::LoopStopped | CeremonyKind::GoalCleared => {
            Scene::Perseverance
        }
        CeremonyKind::LoopFailed => Scene::Dragon,
    }
}

fn is_jousting_preview(label: &str) -> bool {
    if named_scene(label).is_some() {
        return false;
    }
    let lower = label.trim().to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "start" | "joust" | "win" | "fail" | "retreat"
    ) {
        return true;
    }
    lower
        .split('·')
        .map(str::trim)
        .any(|part| matches!(part, "start" | "joust" | "win" | "fail" | "retreat"))
}

fn named_scene(label: &str) -> Option<Scene> {
    let lower = label.trim().to_ascii_lowercase();
    if let Some(scene) = Scene::parse(&lower) {
        return Some(scene);
    }
    lower.split('·').map(str::trim).find_map(Scene::parse)
}

/// Art rows only. Status rows stay in `lifecycle_viz` so motion chrome matches.
pub(crate) fn try_art(
    kind: CeremonyKind,
    label: &str,
    elapsed_secs: f32,
    width: usize,
    art_height: usize,
    motion: MotionMode,
) -> Option<Vec<Line<'static>>> {
    let (image, origin_x, origin_y) =
        art_frame(kind, label, elapsed_secs, width, art_height, motion)?;
    Some(pane_from_image(
        &image, width, art_height, origin_x, origin_y,
    ))
}

/// The same consumed art as `try_art`, before terminal text serialization.
/// Fine-dot transport consumes this grid and leaves status text at normal size.
pub(crate) fn try_image(
    kind: CeremonyKind,
    label: &str,
    elapsed_secs: f32,
    width: usize,
    height: usize,
    motion: MotionMode,
) -> Option<Arc<crate::term::art::ColoredBrailleImage>> {
    let len = width.checked_mul(height).filter(|len| *len <= 64_000)?;
    let (art, origin_x, origin_y) = art_frame(kind, label, elapsed_secs, width, height, motion)?;
    let mut cells = vec![ColoredBrailleCell::default(); len];
    for y in 0..art.height.min(height.saturating_sub(origin_y)) {
        for x in 0..art.width.min(width.saturating_sub(origin_x)) {
            cells[(origin_y + y) * width + origin_x + x] = art.cell(x, y).unwrap_or_default();
        }
    }
    Some(Arc::new(crate::term::art::ColoredBrailleImage {
        width,
        height,
        cells,
    }))
}

fn art_frame(
    kind: CeremonyKind,
    label: &str,
    elapsed_secs: f32,
    width: usize,
    art_height: usize,
    motion: MotionMode,
) -> Option<(Arc<crate::term::art::ColoredBrailleImage>, usize, usize)> {
    if width == 0 || art_height == 0 {
        return None;
    }
    let scene = scene_for(kind, label)?;
    let spec = manifest()?.scene(scene)?;
    let sheet = load_sheet(&spec.path)?;
    let frame_index = playback_frame(elapsed_secs, motion, spec.frame_ms, spec.sequence.len());
    let rect_index = spec.sequence.get(frame_index).copied()?;
    let rect = spec.frames.get(rect_index).copied()?;
    let (cell_w, cell_h, mut origin_x, mut origin_y) =
        fit_cell_box(rect.width, rect.height, width, art_height);
    let cache_stem = crate::term::art::image_path_key(&spec.path);
    let image =
        crate::term::art::colored_rgba_region_braille(&sheet, cache_stem, rect, cell_w, cell_h, true)?;
    origin_x += cell_w.saturating_sub(image.width) / 2;
    origin_y += cell_h.saturating_sub(image.height) / 2;
    Some((image, origin_x, origin_y))
}

pub(crate) fn warm() {
    let Some(manifest) = manifest() else {
        return;
    };
    for spec in &manifest.scenes {
        let _ = load_sheet(&spec.path);
    }
}

fn playback_frame(elapsed_secs: f32, motion: MotionMode, frame_ms: u64, seq_len: usize) -> usize {
    let seq_len = seq_len.max(1);
    let last = seq_len - 1;
    match motion {
        MotionMode::Off => 0,
        MotionMode::Reduced => match reduced_phase(elapsed_secs) {
            ReducedPhase::Entry => 0,
            ReducedPhase::Hold => (seq_len / 2).min(last),
            ReducedPhase::Exit => last,
        },
        MotionMode::Full => {
            let elapsed = if elapsed_secs.is_finite() {
                elapsed_secs.max(0.0)
            } else {
                0.0
            };
            let step = (elapsed * 1000.0 / frame_ms.max(1) as f32).floor() as usize;
            step % seq_len
        }
    }
}

#[derive(Clone, Copy)]
enum ReducedPhase {
    Entry,
    Hold,
    Exit,
}

fn reduced_phase(elapsed_secs: f32) -> ReducedPhase {
    let elapsed = if elapsed_secs.is_finite() {
        elapsed_secs.max(0.0)
    } else {
        0.0
    };
    let full_frame = ((elapsed * FPS).floor() as usize).min(FRAME_COUNT - 1);
    let frame = if elapsed < 0.45 {
        full_frame.min(5)
    } else if elapsed < 3.15 {
        32
    } else {
        full_frame.max(38)
    };
    match frame {
        0..=5 => ReducedPhase::Entry,
        6..=37 => ReducedPhase::Hold,
        _ => ReducedPhase::Exit,
    }
}

fn fit_cell_box(
    src_w: u32,
    src_h: u32,
    pane_w: usize,
    pane_h: usize,
) -> (usize, usize, usize, usize) {
    let margin = if pane_w > 4 && pane_h > 2 {
        MARGIN_CELLS
    } else {
        0
    };
    let avail_w = pane_w.saturating_sub(margin.saturating_mul(2)).max(1);
    let avail_h = pane_h.saturating_sub(margin.saturating_mul(2)).max(1);
    let src_w = src_w.max(1) as f32;
    let src_h = src_h.max(1) as f32;
    // Braille cells are 2×4 dots. Equal pitch ⇒ cell_w / cell_h = 2 * src_w / src_h.
    let aspect = 2.0 * src_w / src_h;
    let mut cell_h = avail_h;
    let mut cell_w = ((cell_h as f32 * aspect).round() as usize).max(1);
    if cell_w > avail_w {
        cell_w = avail_w;
        cell_h = ((cell_w as f32 / aspect).round() as usize)
            .max(1)
            .min(avail_h);
    }
    // Refit width after row rounding; keeping the old width stretches narrow panes.
    cell_w = ((cell_h as f32 * aspect).round() as usize)
        .min(avail_w)
        .max(1);
    cell_h = cell_h.min(avail_h).max(1);
    let x = margin + avail_w.saturating_sub(cell_w) / 2;
    let y = margin + avail_h.saturating_sub(cell_h) / 2;
    (cell_w, cell_h, x, y)
}

fn pane_from_image(
    image: &crate::term::art::ColoredBrailleImage,
    pane_w: usize,
    pane_h: usize,
    origin_x: usize,
    origin_y: usize,
) -> Vec<Line<'static>> {
    let empty = ColoredBrailleCell::default();
    let mut lines = Vec::with_capacity(pane_h);
    for row in 0..pane_h {
        let mut spans = Vec::with_capacity(pane_w);
        for col in 0..pane_w {
            let cell = if row >= origin_y && col >= origin_x {
                image.cell(col - origin_x, row - origin_y).unwrap_or(empty)
            } else {
                empty
            };
            spans.push(Span::styled(
                cell.glyph.to_string(),
                Style::new()
                    .fg(Color::Rgb(cell.fg[0], cell.fg[1], cell.fg[2]))
                    .bg(Color::Rgb(
                        DMD_PALETTE[0][0],
                        DMD_PALETTE[0][1],
                        DMD_PALETTE[0][2],
                    )),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines
}

#[derive(Debug)]
struct Manifest {
    scenes: Vec<SceneSpec>,
}

impl Manifest {
    fn scene(&self, scene: Scene) -> Option<&SceneSpec> {
        self.scenes.iter().find(|spec| spec.scene == scene)
    }
}

#[derive(Debug)]
struct SceneSpec {
    scene: Scene,
    path: PathBuf,
    frames: Vec<ImageRegion>,
    sequence: Vec<usize>,
    frame_ms: u64,
}

#[derive(Deserialize)]
struct ManifestFile {
    scenes: Vec<ManifestScene>,
}

#[derive(Deserialize)]
struct ManifestScene {
    name: String,
    file: String,
    frames: Vec<Vec<u32>>,
    sequence: Vec<usize>,
    frame_ms: u64,
}

fn default_root() -> PathBuf {
    crate::runtime_paths::cockpit_dir().join("assets/knight-journey")
}

fn manifest() -> Option<&'static Manifest> {
    static MANIFEST: OnceLock<Option<Manifest>> = OnceLock::new();
    MANIFEST
        .get_or_init(|| load_manifest(&default_root()))
        .as_ref()
}

fn load_manifest(root: &Path) -> Option<Manifest> {
    let raw = std::fs::read_to_string(root.join("manifest.json")).ok()?;
    let parsed: ManifestFile = serde_json::from_str(&raw).ok()?;
    let mut scenes = Vec::with_capacity(parsed.scenes.len());
    for entry in parsed.scenes {
        let scene = Scene::parse(&entry.name)?;
        if entry.frames.is_empty() || entry.sequence.is_empty() || entry.frame_ms == 0 {
            return None;
        }
        let mut frames = Vec::with_capacity(entry.frames.len());
        for rect in entry.frames {
            if rect.len() != 4 || rect[2] == 0 || rect[3] == 0 {
                return None;
            }
            frames.push(ImageRegion {
                x: rect[0],
                y: rect[1],
                width: rect[2],
                height: rect[3],
            });
        }
        if entry.sequence.iter().any(|index| *index >= frames.len()) {
            return None;
        }
        scenes.push(SceneSpec {
            scene,
            path: root.join(entry.file),
            frames,
            sequence: entry.sequence,
            frame_ms: entry.frame_ms,
        });
    }
    if scenes.is_empty() {
        None
    } else {
        Some(Manifest { scenes })
    }
}

fn sheet_cache() -> &'static Mutex<HashMap<PathBuf, Arc<image::RgbaImage>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<image::RgbaImage>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_sheet(path: &Path) -> Option<Arc<image::RgbaImage>> {
    if let Ok(cache) = sheet_cache().lock()
        && let Some(image) = cache.get(path).cloned()
    {
        return Some(image);
    }
    let image = Arc::new(image::open(path).ok()?.to_rgba8());
    if let Ok(mut cache) = sheet_cache().lock() {
        if !cache.contains_key(path)
            && cache.len() >= SHEET_CACHE_LIMIT
            && let Some(oldest) = cache.keys().next().cloned()
        {
            cache.remove(&oldest);
        }
        cache.insert(path.to_path_buf(), Arc::clone(&image));
    }
    Some(image)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/knight_journey__tests.rs"]
mod tests;
