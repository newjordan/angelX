//! Terminal-native braille image conversion for resident cockpit art.
//!
//! Two looks: `colored_image_braille` (palette-quantized, Bayer ordered dither
//! — the MoA card look) and `mono_image_braille` (one-bit Atkinson error
//! diffusion — the classic-Macintosh plate look for the noir miniviz).
//!
//! This module has no external renderer, manifest, background-layer, or terminal
//! configuration integration. It only converts bundled images into ratatui
//! cells used by the ordinary terminal UI.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

// Sized for the largest per-surface working set — a grand-joust ceremony
// touches 27 images (arena + poses + two sequence loops) — rendered on two
// concurrent surfaces at different canvas sizes, with headroom for portraits.
// The FIFO never refreshes on hit, so a working set over capacity evicts hot
// entries continuously; entries are small (quantized cell grids), so oversize.
const COLORED_BRAILLE_CACHE_LIMIT: usize = 96;
const DECODED_IMAGE_CACHE_LIMIT: usize = 24;
const REGION_BRAILLE_MAX_WIDTH: usize = 180;
const REGION_BRAILLE_MAX_HEIGHT: usize = 72;

/// Deliberately small palette shared by terminal-native visualizations.
pub const DMD_PALETTE: [[u8; 3]; 8] = [
    [0, 0, 0],
    [86, 232, 255],
    [91, 190, 255],
    [58, 111, 151],
    [184, 228, 255],
    [255, 207, 92],
    [255, 103, 126],
    [99, 241, 169],
];
const DMD_DITHER_LEVEL: [u16; 8] = [0, 233, 219, 185, 239, 235, 201, 232];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColoredBrailleCell {
    pub glyph: char,
    pub fg: [u8; 3],
}

impl Default for ColoredBrailleCell {
    fn default() -> Self {
        Self {
            glyph: '\u{2800}',
            fg: DMD_PALETTE[0],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColoredBrailleImage {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<ColoredBrailleCell>,
}

impl ColoredBrailleImage {
    pub fn cell(&self, x: usize, y: usize) -> Option<ColoredBrailleCell> {
        (x < self.width && y < self.height).then(|| self.cells[y * self.width + x])
    }
}

/// Ink color for one-bit plates: a warm paper-white, restylable at draw time.
// TODO(noir-wave): drop the allows once the plate pipeline consumes the mono
// converter from the miniviz draw path.
#[allow(dead_code)]
pub const MONO_INK: [u8; 3] = [236, 236, 228];

/// Decode, palette-quantize, and ordered-dither an image into colored braille.
/// Cache dimensions are bucketed so resize drags stay bounded.
pub fn colored_image_braille(
    image_path: &Path,
    width: usize,
    height: usize,
) -> Option<Arc<ColoredBrailleImage>> {
    if width == 0 || height == 0 {
        return None;
    }
    let (width, height) = quantize_braille_image_size(width, height);
    let key = braille_text_key(image_path, width, height, b'c');
    let cache = colored_braille_cache();
    if let Some(image) = cache
        .lock()
        .ok()
        .and_then(|cache| cache.map.get(&key).cloned())
    {
        return Some(image);
    }

    let image = Arc::new(colored_image_braille_uncached(image_path, width, height)?);
    if let Ok(mut cache) = cache.lock() {
        if !cache.map.contains_key(&key) {
            if cache.order.len() >= COLORED_BRAILLE_CACHE_LIMIT
                && let Some(oldest) = cache.order.pop_front()
            {
                cache.map.remove(&oldest);
            }
            cache.order.push_back(key);
        }
        cache.map.insert(key, Arc::clone(&image));
    }
    Some(image)
}

/// Source-pixel rectangle inside a decoded sheet. Runtime crop/fit uses this;
/// the original file is never shown as a world plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Crop a decoded image, optionally drop dark-neutral panel fill, and convert
/// the region to colored Braille. Requested cell size is fit proportionally
/// inside the conversion cap so cache limits never stretch or crop the subject.
/// Cache key includes path stem + region + fitted size.
pub fn colored_rgba_region_braille(
    source: &image::RgbaImage,
    cache_stem: u64,
    region: ImageRegion,
    width: usize,
    height: usize,
    suppress_dark_neutral: bool,
) -> Option<Arc<ColoredBrailleImage>> {
    if width == 0 || height == 0 {
        return None;
    }
    let (width, height) = fit_region_braille_size(width, height);
    let key = region_braille_key(cache_stem, region, width, height, suppress_dark_neutral);
    let cache = colored_braille_cache();
    if let Some(image) = cache
        .lock()
        .ok()
        .and_then(|cache| cache.map.get(&key).cloned())
    {
        return Some(image);
    }

    let image = Arc::new(colored_rgba_region_braille_uncached(
        source,
        region,
        width,
        height,
        suppress_dark_neutral,
    )?);
    if let Ok(mut cache) = cache.lock() {
        if !cache.map.contains_key(&key) {
            if cache.order.len() >= COLORED_BRAILLE_CACHE_LIMIT
                && let Some(oldest) = cache.order.pop_front()
            {
                cache.map.remove(&oldest);
            }
            cache.order.push_back(key);
        }
        cache.map.insert(key, Arc::clone(&image));
    }
    Some(image)
}

fn colored_image_braille_uncached(
    image_path: &Path,
    width: usize,
    height: usize,
) -> Option<ColoredBrailleImage> {
    let source = decoded_image(image_path)?;
    let fitted = fit_image_to_canvas(&source, (width * 2) as u32, (height * 4) as u32);
    Some(rgba_canvas_to_colored_braille(&fitted, width, height))
}

fn colored_rgba_region_braille_uncached(
    source: &image::RgbaImage,
    region: ImageRegion,
    width: usize,
    height: usize,
    suppress_dark_neutral: bool,
) -> Option<ColoredBrailleImage> {
    let cropped = crop_region(source, region)?;
    let prepared = if suppress_dark_neutral {
        let mut image = cropped;
        clear_dark_neutral(&mut image);
        image
    } else {
        cropped
    };
    let fitted = fit_image_to_canvas(&prepared, (width * 2) as u32, (height * 4) as u32);
    Some(rgba_canvas_to_colored_braille(&fitted, width, height))
}

fn fit_region_braille_size(width: usize, height: usize) -> (usize, usize) {
    let width = width.max(1);
    let height = height.max(1);
    if width <= REGION_BRAILLE_MAX_WIDTH && height <= REGION_BRAILLE_MAX_HEIGHT {
        return (width, height);
    }
    let scale = (REGION_BRAILLE_MAX_WIDTH as f32 / width as f32)
        .min(REGION_BRAILLE_MAX_HEIGHT as f32 / height as f32);
    let fitted_w = ((width as f32 * scale).round() as usize).clamp(1, REGION_BRAILLE_MAX_WIDTH);
    let fitted_h = ((height as f32 * scale).round() as usize).clamp(1, REGION_BRAILLE_MAX_HEIGHT);
    (fitted_w, fitted_h)
}

fn crop_region(source: &image::RgbaImage, region: ImageRegion) -> Option<image::RgbaImage> {
    let (img_w, img_h) = source.dimensions();
    if region.width == 0 || region.height == 0 || region.x >= img_w || region.y >= img_h {
        return None;
    }
    let width = region.width.min(img_w.saturating_sub(region.x));
    let height = region.height.min(img_h.saturating_sub(region.y));
    if width == 0 || height == 0 {
        return None;
    }
    Some(image::imageops::crop_imm(source, region.x, region.y, width, height).to_image())
}

fn clear_dark_neutral(image: &mut image::RgbaImage) {
    for pixel in image.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        if a < 12 {
            continue;
        }
        if is_dark_neutral([r, g, b]) {
            pixel.0[3] = 0;
        }
    }
}

fn is_dark_neutral([r, g, b]: [u8; 3]) -> bool {
    let luma = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let chroma = r.abs_diff(g).max(g.abs_diff(b)).max(r.abs_diff(b));
    luma <= 36 && chroma <= 16
}

fn rgba_canvas_to_colored_braille(
    fitted: &image::RgbaImage,
    width: usize,
    height: usize,
) -> ColoredBrailleImage {
    let mut cells = vec![ColoredBrailleCell::default(); width * height];
    for cell_y in 0..height {
        for cell_x in 0..width {
            let mut bits = 0u8;
            let mut palette_votes = [0u8; DMD_PALETTE.len()];
            for local_y in 0..4 {
                for local_x in 0..2 {
                    let x = cell_x * 2 + local_x;
                    let y = cell_y * 4 + local_y;
                    let rgba = fitted.get_pixel(x as u32, y as u32).0;
                    if rgba[3] < 12 {
                        continue;
                    }
                    let raw_color = [rgba[0], rgba[1], rgba[2]];
                    let palette_index = DMD_PALETTE
                        .iter()
                        .position(|color| *color == raw_color)
                        .unwrap_or_else(|| nearest_palette(raw_color));
                    if palette_index == 0 {
                        continue;
                    }
                    let coverage = u16::from(rgba[3]) * DMD_DITHER_LEVEL[palette_index] / 255;
                    let threshold = u16::from(bayer4(x, y)) * 16 + 8;
                    if coverage < threshold {
                        continue;
                    }
                    bits |= braille_dot_bit(local_x, local_y);
                    palette_votes[palette_index] = palette_votes[palette_index].saturating_add(1);
                }
            }
            if bits == 0 {
                continue;
            }
            let palette_index = palette_votes
                .iter()
                .enumerate()
                .skip(1)
                .max_by_key(|(index, votes)| (**votes, *index))
                .map(|(index, _)| index)
                .unwrap_or(1);
            cells[cell_y * width + cell_x] = ColoredBrailleCell {
                glyph: braille_char(bits),
                fg: DMD_PALETTE[palette_index],
            };
        }
    }
    ColoredBrailleImage {
        width,
        height,
        cells,
    }
}

/// Decode and Atkinson-dither an image into one-bit braille — the classic
/// Macintosh look. Grayscale with a percentile contrast stretch, then Bill
/// Atkinson's error diffusion (3/4 of the error propagates, so midtones lift
/// toward paper-white and shadows pool into solid ink). Every lit cell carries
/// [`MONO_INK`]; restyle at draw time for scene accents.
#[allow(dead_code)]
pub fn mono_image_braille(
    image_path: &Path,
    width: usize,
    height: usize,
) -> Option<Arc<ColoredBrailleImage>> {
    if width == 0 || height == 0 {
        return None;
    }
    let (width, height) = quantize_braille_image_size(width, height);
    let key = braille_text_key(image_path, width, height, b'm');
    let cache = colored_braille_cache();
    if let Some(image) = cache
        .lock()
        .ok()
        .and_then(|cache| cache.map.get(&key).cloned())
    {
        return Some(image);
    }

    let image = Arc::new(mono_image_braille_uncached(image_path, width, height)?);
    if let Ok(mut cache) = cache.lock() {
        if !cache.map.contains_key(&key) {
            if cache.order.len() >= COLORED_BRAILLE_CACHE_LIMIT
                && let Some(oldest) = cache.order.pop_front()
            {
                cache.map.remove(&oldest);
            }
            cache.order.push_back(key);
        }
        cache.map.insert(key, Arc::clone(&image));
    }
    Some(image)
}

fn mono_image_braille_uncached(
    image_path: &Path,
    width: usize,
    height: usize,
) -> Option<ColoredBrailleImage> {
    let source = decoded_image(image_path)?;
    let (dots_w, dots_h) = ((width * 2) as u32, (height * 4) as u32);
    // Smooth resample (unlike the colored path's pixel-art Nearest): the
    // dither needs real midtones to diffuse, not posterized blocks.
    let (source_width, source_height) = source.dimensions();
    if source_width == 0 || source_height == 0 {
        return None;
    }
    let scale = (dots_w as f32 / source_width as f32).min(dots_h as f32 / source_height as f32);
    let resized_width = ((source_width as f32 * scale).round() as u32).clamp(1, dots_w);
    let resized_height = ((source_height as f32 * scale).round() as u32).clamp(1, dots_h);
    let resized = image::imageops::resize(
        source.as_ref(),
        resized_width,
        resized_height,
        image::imageops::FilterType::CatmullRom,
    );
    let mut canvas = image::RgbaImage::new(dots_w, dots_h);
    image::imageops::overlay(
        &mut canvas,
        &resized,
        i64::from((dots_w - resized_width) / 2),
        i64::from((dots_h - resized_height) / 2),
    );

    // Grayscale (alpha-weighted) + histogram for the 2% contrast stretch.
    let mut luma = vec![0i32; (dots_w * dots_h) as usize];
    let mut histogram = [0u32; 256];
    for (index, pixel) in canvas.pixels().enumerate() {
        let [r, g, b, a] = pixel.0;
        let gray = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
        let value = (gray * u32::from(a) / 255) as i32;
        luma[index] = value;
        histogram[value as usize] += 1;
    }
    let total = dots_w * dots_h;
    let cutoff = total / 50;
    let percentile = |from_low: bool| -> i32 {
        let mut seen = 0u32;
        let indices: Box<dyn Iterator<Item = usize>> = if from_low {
            Box::new(0..256)
        } else {
            Box::new((0..256).rev())
        };
        for value in indices {
            seen += histogram[value];
            if seen > cutoff {
                return value as i32;
            }
        }
        if from_low { 0 } else { 255 }
    };
    let (low, high) = (percentile(true), percentile(false));
    let range = (high - low).max(1);

    // Atkinson error diffusion over the stretched luma.
    for value in &mut luma {
        *value = ((*value - low) * 255 / range).clamp(0, 255);
    }
    let (dots_w, dots_h) = (dots_w as usize, dots_h as usize);
    let mut bitmap = vec![false; luma.len()];
    for y in 0..dots_h {
        for x in 0..dots_w {
            let index = y * dots_w + x;
            let old = luma[index];
            let lit = old > 127;
            bitmap[index] = lit;
            let error = (old - if lit { 255 } else { 0 }) / 8;
            for (dx, dy) in [(1i64, 0i64), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx >= 0 && (nx as usize) < dots_w && (ny as usize) < dots_h {
                    luma[ny as usize * dots_w + nx as usize] += error;
                }
            }
        }
    }

    let mut cells = vec![ColoredBrailleCell::default(); width * height];
    for cell_y in 0..height {
        for cell_x in 0..width {
            let mut bits = 0u8;
            for local_y in 0..4 {
                for local_x in 0..2 {
                    if bitmap[(cell_y * 4 + local_y) * dots_w + cell_x * 2 + local_x] {
                        bits |= braille_dot_bit(local_x, local_y);
                    }
                }
            }
            if bits != 0 {
                cells[cell_y * width + cell_x] = ColoredBrailleCell {
                    glyph: braille_char(bits),
                    fg: MONO_INK,
                };
            }
        }
    }
    Some(ColoredBrailleImage {
        width,
        height,
        cells,
    })
}

fn quantize_braille_image_size(width: usize, height: usize) -> (usize, usize) {
    let width = width.clamp(1, 180);
    let height = height.clamp(1, 72);
    let bucket = |value: usize, step: usize| {
        if value <= step {
            value
        } else {
            value.div_ceil(step) * step
        }
    };
    (bucket(width, 4).min(180), bucket(height, 2).min(72))
}

fn nearest_palette(rgb: [u8; 3]) -> usize {
    DMD_PALETTE
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| {
            let dr = i32::from(rgb[0]) - i32::from(candidate[0]);
            let dg = i32::from(rgb[1]) - i32::from(candidate[1]);
            let db = i32::from(rgb[2]) - i32::from(candidate[2]);
            2 * dr * dr + 4 * dg * dg + db * db
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}

pub const fn braille_dot_bit(local_x: usize, local_y: usize) -> u8 {
    match (local_x, local_y) {
        (0, 0) => 1 << 0,
        (0, 1) => 1 << 1,
        (0, 2) => 1 << 2,
        (0, 3) => 1 << 6,
        (1, 0) => 1 << 3,
        (1, 1) => 1 << 4,
        (1, 2) => 1 << 5,
        (1, 3) => 1 << 7,
        _ => 0,
    }
}

pub const fn braille_bits(glyph: char) -> u8 {
    let code = glyph as u32;
    if code >= 0x2800 && code <= 0x28ff {
        (code - 0x2800) as u8
    } else {
        0
    }
}

pub fn braille_char(bits: u8) -> char {
    char::from_u32(0x2800 + u32::from(bits)).unwrap_or('\u{2800}')
}

#[derive(Default)]
struct ColoredBrailleCache {
    map: HashMap<u64, Arc<ColoredBrailleImage>>,
    order: VecDeque<u64>,
}

fn colored_braille_cache() -> &'static Mutex<ColoredBrailleCache> {
    static CACHE: OnceLock<Mutex<ColoredBrailleCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ColoredBrailleCache::default()))
}

/// Warm the bounded image cache before an animation's first terminal frame.
pub fn preload_colored_image(image_path: &Path) -> bool {
    decoded_image(image_path).is_some()
}

#[derive(Default)]
struct DecodedImageCache {
    map: HashMap<u64, Arc<image::RgbaImage>>,
    order: VecDeque<u64>,
}

fn decoded_image(image_path: &Path) -> Option<Arc<image::RgbaImage>> {
    let key = image_path_key(image_path);
    let cache = decoded_image_cache();
    if let Some(image) = cache
        .lock()
        .ok()
        .and_then(|cache| cache.map.get(&key).cloned())
    {
        return Some(image);
    }
    let image = Arc::new(image::open(image_path).ok()?.to_rgba8());
    if let Ok(mut cache) = cache.lock() {
        if !cache.map.contains_key(&key) {
            if cache.order.len() >= DECODED_IMAGE_CACHE_LIMIT
                && let Some(oldest) = cache.order.pop_front()
            {
                cache.map.remove(&oldest);
            }
            cache.order.push_back(key);
        }
        cache.map.insert(key, Arc::clone(&image));
    }
    Some(image)
}

fn decoded_image_cache() -> &'static Mutex<DecodedImageCache> {
    static CACHE: OnceLock<Mutex<DecodedImageCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(DecodedImageCache::default()))
}

pub(crate) fn image_path_key(image_path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    image_path.hash(&mut hasher);
    hasher.finish()
}

fn region_braille_key(
    stem: u64,
    region: ImageRegion,
    width: usize,
    height: usize,
    suppress_dark_neutral: bool,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    stem.hash(&mut hasher);
    region.x.hash(&mut hasher);
    region.y.hash(&mut hasher);
    region.width.hash(&mut hasher);
    region.height.hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    suppress_dark_neutral.hash(&mut hasher);
    b'k'.hash(&mut hasher);
    hasher.finish()
}

fn braille_text_key(image_path: &Path, width: usize, height: usize, mode: u8) -> u64 {
    let mut hasher = DefaultHasher::new();
    image_path.hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    mode.hash(&mut hasher);
    hasher.finish()
}

fn fit_image_to_canvas(source: &image::RgbaImage, width: u32, height: u32) -> image::RgbaImage {
    let (source_width, source_height) = source.dimensions();
    if source_width == 0 || source_height == 0 || width == 0 || height == 0 {
        return image::RgbaImage::new(width.max(1), height.max(1));
    }
    let scale = (width as f32 / source_width as f32).min(height as f32 / source_height as f32);
    let resized_width = ((source_width as f32 * scale).round() as u32).clamp(1, width);
    let resized_height = ((source_height as f32 * scale).round() as u32).clamp(1, height);
    let resized = image::imageops::resize(
        source,
        resized_width,
        resized_height,
        image::imageops::FilterType::Nearest,
    );
    let mut canvas = image::RgbaImage::new(width, height);
    image::imageops::overlay(
        &mut canvas,
        &resized,
        i64::from((width - resized_width) / 2),
        i64::from((height - resized_height) / 2),
    );
    canvas
}

fn bayer4(x: usize, y: usize) -> u8 {
    const MATRIX: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    MATRIX[y % 4][x % 4]
}

/// Canonical braille-grid → styled lines conversion: consecutive cells with the
/// same color (and braille-ness, rendered bold for non-braille glyph overlays)
/// merge into one span — a String+Span per cell per frame is thousands of
/// allocations per second at animation rate. Shared by the Realm world and the
/// loop stage; radial/uniform-styled charts keep their own specialized paths.
pub(crate) fn grid_lines(grid: &dotmax::BrailleGrid) -> Vec<ratatui::text::Line<'static>> {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    fn styled_run(text: String, (color, bold): (Option<(u8, u8, u8)>, bool)) -> Span<'static> {
        let mut style =
            color.map_or_else(Style::new, |(r, g, b)| Style::new().fg(Color::Rgb(r, g, b)));
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        Span::styled(text, style)
    }
    let mut lines = Vec::with_capacity(grid.height());
    for y in 0..grid.height() {
        let mut spans = Vec::new();
        let mut run = String::new();
        let mut run_key: (Option<(u8, u8, u8)>, bool) = (None, false);
        for x in 0..grid.width() {
            let ch = grid.get_char(x, y);
            let key = (
                grid.get_color(x, y).map(|c| (c.r, c.g, c.b)),
                !('\u{2800}'..='\u{28FF}').contains(&ch),
            );
            if x > 0 && key != run_key {
                spans.push(styled_run(std::mem::take(&mut run), run_key));
            }
            run_key = key;
            run.push(ch);
        }
        if !run.is_empty() {
            spans.push(styled_run(run, run_key));
        }
        lines.push(Line::from(spans));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn colored_braille_quantizes_to_one_palette_color_per_cell() {
        let asset =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tourney/arena-plate.png");
        let image = colored_image_braille(&asset, 73, 17).expect("colored arena");
        assert_eq!((image.width, image.height), (76, 18));
        assert_eq!(image.cells.len(), image.width * image.height);
        assert!(
            image
                .cells
                .iter()
                .all(|cell| DMD_PALETTE.contains(&cell.fg))
        );
        assert!(
            image
                .cells
                .iter()
                .any(|cell| braille_bits(cell.glyph) != 0 && cell.fg != DMD_PALETTE[0])
        );
    }

    #[test]
    fn mono_braille_dithers_a_gradient_with_uniform_ink() {
        let path =
            std::env::temp_dir().join(format!("angel-mono-gradient-{}.png", std::process::id()));
        let gradient = image::RgbaImage::from_fn(160, 80, |x, _| {
            let value = (x * 255 / 159) as u8;
            image::Rgba([value, value, value, 255])
        });
        gradient.save(&path).unwrap();

        let plate = mono_image_braille(&path, 40, 10).expect("mono gradient");
        let _ = std::fs::remove_file(&path);

        assert!(
            plate
                .cells
                .iter()
                .all(|cell| braille_bits(cell.glyph) == 0 || cell.fg == MONO_INK)
        );
        let dots = |cells: &[ColoredBrailleCell]| -> u32 {
            cells
                .iter()
                .map(|cell| u32::from(braille_bits(cell.glyph)).count_ones())
                .sum()
        };
        let third = plate.width / 3;
        let mut dark = 0;
        let mut bright = 0;
        for y in 0..plate.height {
            let row = &plate.cells[y * plate.width..(y + 1) * plate.width];
            dark += dots(&row[..third]);
            bright += dots(&row[plate.width - third..]);
        }
        assert!(
            dark * 2 < bright,
            "dither must lay far more ink on the bright side (dark={dark}, bright={bright})"
        );
    }

    #[test]
    fn mono_and_colored_conversions_do_not_share_cache_entries() {
        let asset =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tourney/arena-plate.png");
        let colored = colored_image_braille(&asset, 73, 17).expect("colored arena");
        let mono = mono_image_braille(&asset, 73, 17).expect("mono arena");
        assert_ne!(
            colored.cells, mono.cells,
            "mono result must not be served from the colored cache entry"
        );
        assert!(
            mono.cells
                .iter()
                .all(|cell| braille_bits(cell.glyph) == 0 || cell.fg == MONO_INK)
        );
    }

    #[test]
    fn corrupt_image_is_a_clean_cache_miss() {
        let path = std::env::temp_dir().join(format!(
            "angel-corrupt-terminal-art-{}.png",
            std::process::id()
        ));
        std::fs::write(&path, b"not a png").unwrap();
        assert!(colored_image_braille(&path, 20, 10).is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn region_crop_suppresses_panel_and_keeps_subject_ink() {
        let mut sheet = image::RgbaImage::from_pixel(40, 20, image::Rgba([18, 18, 20, 255]));
        for y in 4..16 {
            for x in 6..18 {
                sheet.put_pixel(x, y, image::Rgba([255, 207, 92, 255]));
            }
        }
        let region = ImageRegion {
            x: 4,
            y: 2,
            width: 20,
            height: 16,
        };
        let image = colored_rgba_region_braille(&sheet, 0x6b6e69, region, 10, 4, true)
            .expect("cropped gold");
        assert_eq!((image.width, image.height), (10, 4));
        assert!(
            image
                .cells
                .iter()
                .any(|cell| braille_bits(cell.glyph) != 0 && cell.fg == DMD_PALETTE[5]),
            "gold subject must survive"
        );
        let empty = image
            .cells
            .iter()
            .filter(|cell| braille_bits(cell.glyph) == 0)
            .count();
        assert!(empty > 0, "dark-neutral panel must not fill every cell");
    }

    #[test]
    fn region_conversion_fits_large_and_narrow_panes_without_stretching() {
        let mut sheet = image::RgbaImage::from_pixel(80, 40, image::Rgba([0, 0, 0, 255]));
        for y in 0..40 {
            for x in 0..80 {
                sheet.put_pixel(x, y, image::Rgba([255, 207, 92, 255]));
            }
        }
        let region = ImageRegion {
            x: 0,
            y: 0,
            width: 80,
            height: 40,
        };

        let large = colored_rgba_region_braille(&sheet, 0x6c617267, region, 220, 160, false)
            .expect("220x160");
        assert!(
            large.width <= REGION_BRAILLE_MAX_WIDTH && large.height <= REGION_BRAILLE_MAX_HEIGHT,
            "conversion cap {0}x{1}",
            large.width,
            large.height
        );
        let requested_aspect = 220.0 / 160.0;
        let fitted_aspect = large.width as f32 / large.height as f32;
        assert!(
            (fitted_aspect - requested_aspect).abs() / requested_aspect < 0.08,
            "capped size {}x{} stretched (aspect {fitted_aspect} vs {requested_aspect})",
            large.width,
            large.height
        );
        assert!(
            large
                .cells
                .iter()
                .any(|cell| braille_bits(cell.glyph) != 0 && cell.fg == DMD_PALETTE[5])
        );

        let narrow =
            colored_rgba_region_braille(&sheet, 0x6e617272, region, 8, 40, false).expect("narrow");
        assert_eq!((narrow.width, narrow.height), (8, 40));

        let modest =
            colored_rgba_region_braille(&sheet, 0x6d6f64, region, 48, 18, false).expect("48x18");
        assert_eq!((modest.width, modest.height), (48, 18));
    }

    #[test]
    fn decoded_source_cache_is_bounded_by_asset() {
        let root =
            std::env::temp_dir().join(format!("angel-terminal-art-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for index in 0..(DECODED_IMAGE_CACHE_LIMIT + 7) {
            let path = root.join(format!("{index}.png"));
            image::RgbaImage::from_pixel(1, 1, image::Rgba([86, 232, 255, 255]))
                .save(&path)
                .unwrap();
            assert!(preload_colored_image(&path));
        }
        let cache = decoded_image_cache().lock().unwrap();
        assert!(cache.map.len() <= DECODED_IMAGE_CACHE_LIMIT);
        assert!(cache.order.len() <= DECODED_IMAGE_CACHE_LIMIT);
        drop(cache);
        let _ = std::fs::remove_dir_all(root);
    }
}
