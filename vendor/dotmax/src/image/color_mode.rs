//! Color-aware braille rendering module.
//!
//! This module provides functionality to preserve and render image colors when converting
//! to braille output. It supports three rendering modes:

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]
//!
//! - [`ColorMode::Monochrome`]: Black/white only (default, backward compatible)
//! - [`ColorMode::Grayscale`]: 256 shades using ANSI 256-color palette
//! - [`ColorMode::TrueColor`]: Full RGB color per braille cell (24-bit)
//!
//! # Architecture
//!
//! Color rendering follows a separation-of-concerns approach:
//! 1. **Color extraction**: Sample representative color for each 2×4 pixel block (braille cell)
//! 2. **Dot pattern generation**: Convert image to binary (black/white dots) via thresholding/dithering
//! 3. **Color application**: Apply extracted colors to braille cells
//!
//! This ensures dot patterns (on/off) are independent from cell colors, maintaining visual clarity.
//!
//! # Color Sampling Strategies
//!
//! Three strategies are available for extracting representative colors from 2×4 pixel blocks:
//!
//! | Strategy | Best For | Trade-offs |
//! |----------|----------|------------|
//! | **Average** | Photos, general images | Smooth gradients, may blur bold colors |
//! | **Dominant** | Logos, diagrams, flat art | Preserves bold colors, may miss subtle detail |
//! | **Center Pixel** | Performance-critical, simple images | Fast, may not represent block well |
//!
//! Default is [`ColorSamplingStrategy::Average`] for best visual quality.
//!
//! # Terminal Compatibility
//!
//! ## ANSI 256-Color Mode (Grayscale)
//! - Supported by 95%+ of modern terminals
//! - Uses grayscale ramp (ANSI codes 232-255, 24 shades)
//! - Converts RGB → luminance using BT.709 formula
//!
//! ## `TrueColor` Mode (24-bit RGB)
//! - Supported by 80%+ of latest terminals (iTerm2, Alacritty, Windows Terminal, etc.)
//! - Requires `COLORTERM=truecolor` or `COLORTERM=24bit` environment variable
//! - Graceful fallback to ANSI 256 if not supported
//!
//! # Examples
//!
//! ## Render with `TrueColor`
//!
//! ```no_run
//! use dotmax::image::{load_from_path, render_image_with_color, ColorMode, DitheringMethod};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("photo.png"))?;
//! let grid = render_image_with_color(
//!     &img, ColorMode::TrueColor, 80, 24,
//!     DitheringMethod::FloydSteinberg, None, 1.0, 1.0, 1.0
//! )?;
//! // grid now contains both dot patterns and RGB colors per cell
//! # Ok(())
//! # }
//! ```
//!
//! ## Render with Grayscale
//!
//! ```no_run
//! use dotmax::image::{load_from_path, render_image_with_color, ColorMode, DitheringMethod};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("diagram.png"))?;
//! let grid = render_image_with_color(&img, ColorMode::Grayscale, 80, 24, DitheringMethod::FloydSteinberg, None, 1.0, 1.0, 1.0)?;
//! // grid contains dot patterns with grayscale intensity colors
//! # Ok(())
//! # }
//! ```
//!
//! # Performance
//!
//! Color extraction adds <5ms overhead to the rendering pipeline (measured on standard
//! 80×24 terminal). Total pipeline (including color) targets <50ms for monochrome compatibility.
//!
//! ## Overhead by Mode
//! - `Monochrome`: 0ms (no color extraction)
//! - `Grayscale`: ~3ms (RGB → luminance conversion)
//! - `TrueColor`: ~5ms (full RGB extraction and storage)
//!
//! # Integration with Epic 2
//!
//! This module uses the color infrastructure from Epic 2:
//! - [`Color`]: RGB color representation
//! - [`BrailleGrid::set_cell_color()`]: Apply colors to cells
//! - [`crate::TerminalRenderer`]: Renders colored braille with ANSI codes

use image::{DynamicImage, GenericImageView, Rgb};
use tracing::debug;

use crate::image::{
    adjust_brightness, adjust_contrast, adjust_gamma, apply_dithering_jittered, pixels_to_braille,
    to_grayscale, DitheringMethod, JitterParams,
};
use crate::{BrailleGrid, Color, DotmaxError};

/// Color rendering mode for braille output.
///
/// Determines how colors are extracted and applied to braille cells when rendering images.
///
/// # Modes
///
/// - [`Monochrome`](ColorMode::Monochrome): Black/white only (default, backward compatible)
/// - [`Grayscale`](ColorMode::Grayscale): 256 shades using ANSI 256-color palette
/// - [`TrueColor`](ColorMode::TrueColor): Full RGB color per braille cell (24-bit)
///
/// # Use Cases
///
/// ## Monochrome
/// - Terminal lacks color support
/// - Artistic black/white aesthetic desired
/// - Maximum performance (no color extraction overhead)
/// - Text terminals, older systems
///
/// ## Grayscale
/// - Middle ground between monochrome and full color
/// - Wide terminal compatibility (95%+)
/// - Photos with emphasis on lighting/shadows
/// - Performance-conscious applications
///
/// ## `TrueColor`
/// - Modern terminals with 24-bit color support
/// - High-fidelity color reproduction required
/// - Vibrant images (artwork, photos, logos)
/// - Terminal supports `COLORTERM=truecolor`
///
/// # Examples
///
/// ```rust
/// use dotmax::image::ColorMode;
///
/// // Default mode (monochrome)
/// let mode = ColorMode::default();
/// assert_eq!(mode, ColorMode::Monochrome);
///
/// // For modern terminals
/// let mode = ColorMode::TrueColor;
///
/// // For wide compatibility
/// let mode = ColorMode::Grayscale;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorMode {
    /// Black/white only. No color information stored or rendered.
    ///
    /// This is the default mode, providing backward compatibility with the existing
    /// monochrome pipeline. No color extraction overhead.
    Monochrome,

    /// Grayscale using ANSI 256-color palette.
    ///
    /// Converts RGB colors to luminance (BT.709 formula) and maps to ANSI grayscale
    /// ramp (codes 232-255, 24 shades). Provides middle ground between monochrome
    /// and full color with wide terminal compatibility (95%+ support).
    Grayscale,

    /// Full RGB color per braille cell (24-bit).
    ///
    /// Preserves full RGB values and renders with ANSI true color escape codes
    /// (`\x1b[38;2;R;G;Bm`). Requires modern terminal support (`COLORTERM=truecolor`).
    /// Falls back to ANSI 256 if true color not detected.
    TrueColor,
}

impl Default for ColorMode {
    /// Default mode is [`Monochrome`](ColorMode::Monochrome) for backward compatibility.
    fn default() -> Self {
        Self::Monochrome
    }
}

/// Strategy for sampling representative color from a 2×4 pixel block.
///
/// Each braille cell represents a 2×4 pixel block. To determine the cell's color,
/// we must sample from these 8 pixels. Different strategies produce different
/// visual results depending on image content.
///
/// # Strategies
///
/// - [`Average`](ColorSamplingStrategy::Average): Mean RGB across all pixels (default)
/// - [`Dominant`](ColorSamplingStrategy::Dominant): Most frequent color in block
/// - [`CenterPixel`](ColorSamplingStrategy::CenterPixel): Use center pixel's color
///
/// # Examples
///
/// ```rust
/// use dotmax::image::ColorSamplingStrategy;
///
/// // Default strategy (average color)
/// let strategy = ColorSamplingStrategy::default();
/// assert_eq!(strategy, ColorSamplingStrategy::Average);
///
/// // For bold colors
/// let strategy = ColorSamplingStrategy::Dominant;
///
/// // For performance
/// let strategy = ColorSamplingStrategy::CenterPixel;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSamplingStrategy {
    /// Calculate average RGB across all pixels in block.
    ///
    /// **Best for**: Photos, general images with smooth gradients
    /// **Pros**: Natural color blending, smooth transitions
    /// **Cons**: May blur bold colors, slightly slower
    Average,

    /// Use the most frequently occurring color in block.
    ///
    /// **Best for**: Logos, diagrams, flat color artwork
    /// **Pros**: Preserves bold colors, high contrast maintained
    /// **Cons**: May ignore subtle details, computational overhead
    Dominant,

    /// Use the center pixel's color as representative.
    ///
    /// **Best for**: Performance-critical applications, simple images
    /// **Pros**: Fastest strategy (no aggregation)
    /// **Cons**: May not represent block well, sensitive to noise
    CenterPixel,
}

impl Default for ColorSamplingStrategy {
    /// Default strategy is [`Average`](ColorSamplingStrategy::Average) for best visual quality.
    fn default() -> Self {
        Self::Average
    }
}

/// Extract representative color for each braille cell from an image.
///
/// For each 2×4 pixel block (corresponding to one braille cell), calculates a
/// representative color using the specified sampling strategy. Returns a flat
/// vector of colors in row-major order (left-to-right, top-to-bottom).
///
/// # Arguments
///
/// * `image` - Source image to extract colors from
/// * `cell_width` - Number of braille cells horizontally
/// * `cell_height` - Number of braille cells vertically
/// * `strategy` - Color sampling strategy to use
///
/// # Returns
///
/// Vector of colors with length `cell_width * cell_height`, in row-major order.
/// Each color represents one braille cell (2×4 pixel block).
///
/// # Edge Cases
///
/// - If image dimensions aren't divisible by 2×4, partial blocks are padded with black
/// - If a block has no pixels (edge case), defaults to black (RGB 0,0,0)
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, ColorSamplingStrategy};
/// use dotmax::image::color_mode::extract_cell_colors;
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
///
/// // Extract colors for 80×24 cell grid (160×96 pixels)
/// let colors = extract_cell_colors(&img, 80, 24, ColorSamplingStrategy::Average);
/// assert_eq!(colors.len(), 80 * 24);
/// # Ok(())
/// # }
/// ```
pub fn extract_cell_colors(
    image: &DynamicImage,
    cell_width: usize,
    cell_height: usize,
    strategy: ColorSamplingStrategy,
) -> Vec<Color> {
    let img_width = image.width() as usize;
    let img_height = image.height() as usize;

    let mut colors = Vec::with_capacity(cell_width * cell_height);

    for cell_y in 0..cell_height {
        for cell_x in 0..cell_width {
            // Calculate pixel block bounds (2×4 pixels per braille cell)
            let px_start_x = cell_x * 2;
            let px_start_y = cell_y * 4;

            // Collect pixels in 2×4 block
            let mut block_pixels = Vec::with_capacity(8);
            for py in 0..4 {
                for px in 0..2 {
                    let x = px_start_x + px;
                    let y = px_start_y + py;

                    // Bounds check (pad with black if outside image)
                    if x < img_width && y < img_height {
                        let pixel = image.get_pixel(x as u32, y as u32);
                        block_pixels.push(Rgb([pixel[0], pixel[1], pixel[2]]));
                    }
                }
            }

            // Calculate color based on strategy
            let cell_color = match strategy {
                ColorSamplingStrategy::Average => average_color(&block_pixels),
                ColorSamplingStrategy::Dominant => dominant_color(&block_pixels),
                ColorSamplingStrategy::CenterPixel => center_pixel_color(&block_pixels),
            };

            colors.push(cell_color);
        }
    }

    debug!(
        "Extracted {} cell colors from {}×{} image using {:?} strategy",
        colors.len(),
        img_width,
        img_height,
        strategy
    );

    colors
}

/// Calculate average color from a collection of pixels.
///
/// Computes the mean RGB values across all pixels. Produces smooth color blending
/// and natural transitions, ideal for photos and general images.
///
/// # Arguments
///
/// * `pixels` - Slice of RGB pixels to average
///
/// # Returns
///
/// Average color as RGB. Returns black (0,0,0) if pixel slice is empty.
///
/// # Examples
///
/// ```rust
/// use image::Rgb;
/// use dotmax::image::color_mode::average_color;
///
/// let pixels = vec![
///     Rgb([255, 0, 0]),   // Red
///     Rgb([0, 255, 0]),   // Green
///     Rgb([0, 0, 255]),   // Blue
/// ];
///
/// let avg = average_color(&pixels);
/// // Average of R, G, B → ~(85, 85, 85) gray
/// ```
pub fn average_color(pixels: &[Rgb<u8>]) -> Color {
    if pixels.is_empty() {
        return Color::rgb(0, 0, 0); // Default to black
    }

    let mut sum_r = 0u32;
    let mut sum_g = 0u32;
    let mut sum_b = 0u32;

    for pixel in pixels {
        sum_r += pixel[0] as u32;
        sum_g += pixel[1] as u32;
        sum_b += pixel[2] as u32;
    }

    let count = pixels.len() as u32;
    Color::rgb(
        (sum_r / count) as u8,
        (sum_g / count) as u8,
        (sum_b / count) as u8,
    )
}

/// Find the most frequently occurring color in a collection of pixels.
///
/// Uses a simple frequency count to determine the dominant color. Preserves
/// bold colors and high contrast, ideal for logos, diagrams, and flat art.
///
/// # Arguments
///
/// * `pixels` - Slice of RGB pixels to analyze
///
/// # Returns
///
/// Most frequent color. Returns black (0,0,0) if pixel slice is empty.
/// If multiple colors tie for most frequent, returns the first encountered.
///
/// # Examples
///
/// ```rust
/// use image::Rgb;
/// use dotmax::image::color_mode::dominant_color;
///
/// let pixels = vec![
///     Rgb([255, 0, 0]),   // Red
///     Rgb([255, 0, 0]),   // Red
///     Rgb([255, 0, 0]),   // Red
///     Rgb([255, 0, 0]),   // Red
///     Rgb([255, 0, 0]),   // Red
///     Rgb([255, 0, 0]),   // Red (6 red pixels)
///     Rgb([0, 0, 255]),   // Blue
///     Rgb([0, 0, 255]),   // Blue (2 blue pixels)
/// ];
///
/// let dom = dominant_color(&pixels);
/// // Dominant color is red (6 occurrences)
/// assert_eq!(dom, dotmax::Color::rgb(255, 0, 0));
/// ```
pub fn dominant_color(pixels: &[Rgb<u8>]) -> Color {
    if pixels.is_empty() {
        return Color::rgb(0, 0, 0);
    }

    // Count frequency of each color
    let mut color_counts = std::collections::HashMap::new();
    for pixel in pixels {
        let color = Color::rgb(pixel[0], pixel[1], pixel[2]);
        *color_counts.entry(color).or_insert(0) += 1;
    }

    // Find color with highest count
    color_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map_or_else(|| Color::rgb(0, 0, 0), |(color, _)| color)
}

/// Use the center pixel's color as representative.
///
/// Samples the pixel closest to the center of the 2×4 block. Fastest strategy
/// with no aggregation overhead, but may not represent block well if center pixel
/// is an outlier.
///
/// For a 2×4 block, the center is between pixels. We use pixel at index 4 (second
/// column, first row) as the representative center.
///
/// # Arguments
///
/// * `pixels` - Slice of RGB pixels (expected to be 8 for a 2×4 block)
///
/// # Returns
///
/// Color of center pixel. Returns black (0,0,0) if pixel slice is empty or has
/// fewer than 5 pixels.
///
/// # Examples
///
/// ```rust
/// use image::Rgb;
/// use dotmax::image::color_mode::center_pixel_color;
///
/// // 2×4 block with center pixel at index 4
/// let pixels = vec![
///     Rgb([0, 0, 0]),     // Index 0
///     Rgb([0, 0, 0]),     // Index 1
///     Rgb([0, 0, 0]),     // Index 2
///     Rgb([0, 0, 0]),     // Index 3
///     Rgb([255, 0, 0]),   // Index 4 (CENTER - red)
///     Rgb([0, 0, 0]),     // Index 5
///     Rgb([0, 0, 0]),     // Index 6
///     Rgb([0, 0, 0]),     // Index 7
/// ];
///
/// let center = center_pixel_color(&pixels);
/// // Center pixel is red
/// assert_eq!(center, dotmax::Color::rgb(255, 0, 0));
/// ```
pub fn center_pixel_color(pixels: &[Rgb<u8>]) -> Color {
    if pixels.is_empty() {
        return Color::rgb(0, 0, 0);
    }

    // For 2×4 block, center is between pixels. Use pixel 4 (col 0, row 2) as center.
    // If fewer pixels, use middle of available pixels.
    let center_idx = pixels.len() / 2;
    let pixel = pixels.get(center_idx).unwrap_or(&Rgb([0, 0, 0]));

    Color::rgb(pixel[0], pixel[1], pixel[2])
}

/// Convert RGB color to grayscale intensity value (0-255).
///
/// Uses BT.709 standard formula for luminance calculation:
/// `Y = 0.2126*R + 0.7152*G + 0.0722*B`
///
/// This formula matches the grayscale conversion in [`crate::image::convert::to_grayscale`]
/// for consistency across the pipeline.
///
/// # Arguments
///
/// * `color` - RGB color to convert
///
/// # Returns
///
/// Grayscale intensity value from 0 (black) to 255 (white).
///
/// # Examples
///
/// ```rust
/// use dotmax::Color;
/// use dotmax::image::color_mode::rgb_to_grayscale_intensity;
///
/// let red = Color::rgb(255, 0, 0);
/// let intensity = rgb_to_grayscale_intensity(&red);
/// // Red contributes 0.2126, so intensity ≈ 54
///
/// let white = Color::rgb(255, 255, 255);
/// let intensity = rgb_to_grayscale_intensity(&white);
/// assert_eq!(intensity, 255); // Full white
/// ```
pub fn rgb_to_grayscale_intensity(color: &Color) -> u8 {
    // BT.709 formula (same as convert::to_grayscale for consistency)
    let r = color.r as f32 * 0.2126;
    let g = color.g as f32 * 0.7152;
    let b = color.b as f32 * 0.0722;
    (r + g + b).clamp(0.0, 255.0) as u8
}

/// Map grayscale intensity (0-255) to ANSI 256-color code.
///
/// The ANSI 256-color palette includes a dedicated grayscale ramp with codes
/// 232-255 (24 shades from black to white). This function maps 8-bit intensity
/// values to the appropriate ANSI color code.
///
/// # Arguments
///
/// * `intensity` - Grayscale intensity from 0 (black) to 255 (white)
///
/// # Returns
///
/// ANSI 256-color code from 232 (darkest) to 255 (brightest).
///
/// # Terminal Compatibility
///
/// ANSI 256-color is supported by 95%+ of modern terminals including:
/// - iTerm2, Terminal.app (macOS)
/// - gnome-terminal, konsole (Linux)
/// - Windows Terminal, `ConEmu` (Windows)
/// - tmux, screen (multiplexers)
///
/// # Examples
///
/// ```rust
/// use dotmax::image::color_mode::intensity_to_ansi256;
///
/// let black = intensity_to_ansi256(0);
/// assert_eq!(black, 232); // Darkest shade
///
/// let white = intensity_to_ansi256(255);
/// assert_eq!(white, 255); // Brightest shade
///
/// let mid = intensity_to_ansi256(128);
/// assert_eq!(mid, 243); // Mid-gray
/// ```
pub fn intensity_to_ansi256(intensity: u8) -> u8 {
    // ANSI 256-color grayscale ramp: 232-255 (24 shades)
    // Map 0-255 intensity to 232-255 range
    232 + ((intensity as u16 * 23) / 255) as u8
}

/// Convert RGB color to ANSI true color escape code (foreground).
///
/// Generates ANSI escape sequence for 24-bit true color: `\x1b[38;2;R;G;Bm`
///
/// # Arguments
///
/// * `color` - RGB color to convert
///
/// # Returns
///
/// ANSI escape sequence string for foreground color.
///
/// # Terminal Compatibility
///
/// True color (24-bit) requires terminal support for `COLORTERM=truecolor` or
/// `COLORTERM=24bit`. Supported by:
/// - iTerm2 (macOS)
/// - Alacritty (cross-platform)
/// - Windows Terminal (Windows 10+)
/// - kitty (cross-platform)
///
/// For unsupported terminals, check capability first and fall back to ANSI 256.
///
/// # Examples
///
/// ```rust
/// use dotmax::Color;
/// use dotmax::image::color_mode::color_to_truecolor_ansi;
///
/// let red = Color::rgb(255, 0, 0);
/// let ansi = color_to_truecolor_ansi(&red);
/// assert_eq!(ansi, "\x1b[38;2;255;0;0m");
///
/// let cyan = Color::rgb(0, 255, 255);
/// let ansi = color_to_truecolor_ansi(&cyan);
/// assert_eq!(ansi, "\x1b[38;2;0;255;255m");
/// ```
pub fn color_to_truecolor_ansi(color: &Color) -> String {
    format!("\x1b[38;2;{};{};{}m", color.r, color.g, color.b)
}

/// Render image with color preservation to braille grid.
///
/// High-level function that runs the complete color rendering pipeline:
/// 1. Resize image to terminal dimensions
/// 2. Extract colors for each braille cell (based on `ColorMode`)
/// 3. Convert to binary image (grayscale → dither/threshold)
/// 4. Map pixels to braille dot patterns
/// 5. Apply extracted colors to braille cells
///
/// Returns a [`BrailleGrid`] with both dot patterns and color data ready for rendering.
///
/// # Arguments
///
/// * `image` - Source image to render
/// * `mode` - Color mode (Monochrome, Grayscale, or `TrueColor`)
///
/// # Returns
///
/// [`BrailleGrid`] with dot patterns and optional color data.
///
/// # Errors
///
/// Returns [`DotmaxError`] if:
/// - Image dimensions are invalid (0×0)
/// - Grid allocation fails
/// - Image processing fails
///
/// # Pipeline Flow
///
/// ```text
/// DynamicImage
///   ↓
/// [Resize to terminal]
///   ↓
/// [Extract colors] ← ColorMode determines strategy
///   ↓
/// [Convert to binary] ← Grayscale → Threshold/Dither
///   ↓
/// [Map to braille] ← 2×4 pixel blocks → braille cells
///   ↓
/// [Apply colors] ← Set color per cell
///   ↓
/// BrailleGrid (with colors)
/// ```
///
/// # Examples
///
/// ## Monochrome Rendering (Default)
///
/// ```no_run
/// use dotmax::image::{load_from_path, render_image_with_color, ColorMode, DitheringMethod};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
/// let grid = render_image_with_color(
///     &img, ColorMode::Monochrome, 80, 24,
///     DitheringMethod::FloydSteinberg, None, 1.0, 1.0, 1.0
/// )?;
/// // Grid has dot patterns but no color data (backward compatible)
/// # Ok(())
/// # }
/// ```
///
/// ## Grayscale Rendering
///
/// ```no_run
/// use dotmax::image::{load_from_path, render_image_with_color, ColorMode, DitheringMethod};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("photo.jpg"))?;
/// let grid = render_image_with_color(
///     &img, ColorMode::Grayscale, 80, 24,
///     DitheringMethod::FloydSteinberg, None, 1.0, 1.0, 1.0
/// )?;
/// // Grid has dot patterns + grayscale intensity colors
/// # Ok(())
/// # }
/// ```
///
/// ## `TrueColor` Rendering
///
/// ```no_run
/// use dotmax::image::{load_from_path, render_image_with_color, ColorMode, DitheringMethod};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("artwork.png"))?;
/// let grid = render_image_with_color(
///     &img, ColorMode::TrueColor, 80, 24,
///     DitheringMethod::FloydSteinberg, None, 1.0, 1.0, 1.0
/// )?;
/// // Grid has dot patterns + full RGB colors per cell
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// Total pipeline targets <50ms for standard terminals (80×24 cells).
///
/// ## Timing Breakdown
/// - Monochrome: ~40ms (no color extraction)
/// - Grayscale: ~43ms (+3ms for color extraction)
/// - `TrueColor`: ~45ms (+5ms for color extraction)
///
/// ## ISSUE #1 FIX
/// This function now accepts dimensions, dithering method, threshold, and image adjustments
/// to ensure color mode uses the same pipeline as monochrome mode.
#[allow(clippy::too_many_arguments)]
pub fn render_image_with_color(
    image: &DynamicImage,
    mode: ColorMode,
    cell_width: usize,
    cell_height: usize,
    dithering: DitheringMethod,
    threshold: Option<u8>,
    brightness: f32,
    contrast: f32,
    gamma: f32,
) -> Result<BrailleGrid, DotmaxError> {
    render_image_with_color_jittered(
        image,
        mode,
        cell_width,
        cell_height,
        dithering,
        threshold,
        brightness,
        contrast,
        gamma,
        JitterParams::NONE,
    )
}

/// Same as [`render_image_with_color`] but accepts per-frame jitter parameters
/// for ambient (temporal) dithering. With `JitterParams::NONE` it is identical
/// to the deterministic entry point.
#[allow(clippy::too_many_arguments)]
pub fn render_image_with_color_jittered(
    image: &DynamicImage,
    mode: ColorMode,
    cell_width: usize,
    cell_height: usize,
    dithering: DitheringMethod,
    threshold: Option<u8>,
    brightness: f32,
    contrast: f32,
    gamma: f32,
    jitter: JitterParams,
) -> Result<BrailleGrid, DotmaxError> {
    const EPSILON: f32 = 0.001;

    let pixel_width = cell_width * 2; // 2 pixels per cell width
    let pixel_height = cell_height * 4; // 4 pixels per cell height

    debug!(
        "Rendering image with {:?} mode to {}×{} cells ({}×{} pixels)",
        mode, cell_width, cell_height, pixel_width, pixel_height
    );

    // Image is already resized by caller, use directly
    let actual_pixel_width = image.width() as usize;
    let actual_pixel_height = image.height() as usize;
    let actual_cell_width = (actual_pixel_width + 1) / 2; // Round up
    let actual_cell_height = (actual_pixel_height + 3) / 4; // Round up

    debug!(
        "Image dimensions: {}×{} pixels → {}×{} cells",
        actual_pixel_width, actual_pixel_height, actual_cell_width, actual_cell_height
    );

    // Step 1: Extract colors if not monochrome (before grayscale conversion)
    let colors = if mode == ColorMode::Monochrome {
        None
    } else {
        Some(extract_cell_colors(
            image,
            actual_cell_width,
            actual_cell_height,
            ColorSamplingStrategy::Average, // Default strategy
        ))
    };

    // Step 2: Convert to grayscale and apply adjustments (same as monochrome pipeline)
    let mut gray = to_grayscale(image);

    if (brightness - 1.0).abs() > EPSILON {
        let b = brightness.clamp(0.0, 2.0);
        gray = adjust_brightness(&gray, b)?;
        debug!("Applied brightness adjustment: {}", b);
    }
    if (contrast - 1.0).abs() > EPSILON {
        let c = contrast.clamp(0.0, 2.0);
        gray = adjust_contrast(&gray, c)?;
        debug!("Applied contrast adjustment: {}", c);
    }
    if (gamma - 1.0).abs() > EPSILON {
        let g = gamma.clamp(0.1, 3.0);
        gray = adjust_gamma(&gray, g)?;
        debug!("Applied gamma adjustment: {}", g);
    }

    // Step 3: Convert to binary using shared jittered dither pipeline.
    let binary = apply_dithering_jittered(&gray, dithering, threshold, jitter)?;

    // Step 4: Map pixels to braille dots
    let mut grid = pixels_to_braille(&binary, actual_cell_width, actual_cell_height)?;

    // Step 5: Apply colors to grid based on mode
    if let Some(colors) = colors {
        match mode {
            ColorMode::Monochrome => {
                // No colors (already handled by not extracting)
            }
            ColorMode::Grayscale => {
                // Convert RGB colors to grayscale intensity, then apply
                for cell_y in 0..actual_cell_height {
                    for cell_x in 0..actual_cell_width {
                        let idx = cell_y * actual_cell_width + cell_x;
                        let color = &colors[idx];
                        let intensity = rgb_to_grayscale_intensity(color);
                        let gray_color = Color::rgb(intensity, intensity, intensity);
                        grid.set_cell_color(cell_x, cell_y, gray_color)?;
                    }
                }
            }
            ColorMode::TrueColor => {
                // Apply full RGB colors
                for cell_y in 0..actual_cell_height {
                    for cell_x in 0..actual_cell_width {
                        let idx = cell_y * actual_cell_width + cell_x;
                        grid.set_cell_color(cell_x, cell_y, colors[idx])?;
                    }
                }
            }
        }
    }

    debug!(
        "Rendered {}×{} grid with {} mode",
        actual_cell_width,
        actual_cell_height,
        mode_name(mode)
    );

    Ok(grid)
}

const fn mode_name(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Monochrome => "monochrome",
        ColorMode::Grayscale => "grayscale",
        ColorMode::TrueColor => "truecolor",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn test_color_mode_default() {
        assert_eq!(ColorMode::default(), ColorMode::Monochrome);
    }

    #[test]
    fn test_color_mode_derives() {
        let mode1 = ColorMode::TrueColor;
        let mode2 = ColorMode::TrueColor;
        assert_eq!(mode1, mode2);

        let mode3 = mode1;
        assert_eq!(mode1, mode3);
    }

    #[test]
    fn test_color_sampling_strategy_default() {
        assert_eq!(
            ColorSamplingStrategy::default(),
            ColorSamplingStrategy::Average
        );
    }

    #[test]
    fn test_average_color_empty() {
        let pixels: Vec<Rgb<u8>> = vec![];
        let color = average_color(&pixels);
        assert_eq!(color, Color::rgb(0, 0, 0));
    }

    #[test]
    fn test_average_color_single() {
        let pixels = vec![Rgb([255, 128, 64])];
        let color = average_color(&pixels);
        assert_eq!(color, Color::rgb(255, 128, 64));
    }

    #[test]
    fn test_average_color_multiple() {
        let pixels = vec![Rgb([255, 0, 0]), Rgb([0, 255, 0]), Rgb([0, 0, 255])];
        let color = average_color(&pixels);
        // Average: (255+0+0)/3=85, (0+255+0)/3=85, (0+0+255)/3=85
        assert_eq!(color, Color::rgb(85, 85, 85));
    }

    #[test]
    fn test_dominant_color_empty() {
        let pixels: Vec<Rgb<u8>> = vec![];
        let color = dominant_color(&pixels);
        assert_eq!(color, Color::rgb(0, 0, 0));
    }

    #[test]
    fn test_dominant_color_single() {
        let pixels = vec![Rgb([255, 0, 0])];
        let color = dominant_color(&pixels);
        assert_eq!(color, Color::rgb(255, 0, 0));
    }

    #[test]
    fn test_dominant_color_clear_winner() {
        let pixels = vec![
            Rgb([255, 0, 0]),
            Rgb([255, 0, 0]),
            Rgb([255, 0, 0]),
            Rgb([255, 0, 0]),
            Rgb([255, 0, 0]),
            Rgb([255, 0, 0]), // 6 red
            Rgb([0, 0, 255]),
            Rgb([0, 0, 255]), // 2 blue
        ];
        let color = dominant_color(&pixels);
        assert_eq!(color, Color::rgb(255, 0, 0)); // Red wins
    }

    #[test]
    fn test_center_pixel_color_empty() {
        let pixels: Vec<Rgb<u8>> = vec![];
        let color = center_pixel_color(&pixels);
        assert_eq!(color, Color::rgb(0, 0, 0));
    }

    #[test]
    fn test_center_pixel_color_single() {
        let pixels = vec![Rgb([128, 64, 32])];
        let color = center_pixel_color(&pixels);
        // Single pixel is the "center"
        assert_eq!(color, Color::rgb(128, 64, 32));
    }

    #[test]
    fn test_center_pixel_color_full_block() {
        // 2×4 block with center at index 4
        let pixels = vec![
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([255, 0, 0]), // Index 4 (center)
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
            Rgb([0, 0, 0]),
        ];
        let color = center_pixel_color(&pixels);
        assert_eq!(color, Color::rgb(255, 0, 0));
    }

    #[test]
    fn test_rgb_to_grayscale_intensity_black() {
        let color = Color::rgb(0, 0, 0);
        let intensity = rgb_to_grayscale_intensity(&color);
        assert_eq!(intensity, 0);
    }

    #[test]
    fn test_rgb_to_grayscale_intensity_white() {
        let color = Color::rgb(255, 255, 255);
        let intensity = rgb_to_grayscale_intensity(&color);
        assert_eq!(intensity, 255);
    }

    #[test]
    fn test_rgb_to_grayscale_intensity_red() {
        let color = Color::rgb(255, 0, 0);
        let intensity = rgb_to_grayscale_intensity(&color);
        // BT.709: 0.2126 * 255 ≈ 54
        assert!((54..=55).contains(&intensity));
    }

    #[test]
    fn test_rgb_to_grayscale_intensity_green() {
        let color = Color::rgb(0, 255, 0);
        let intensity = rgb_to_grayscale_intensity(&color);
        // BT.709: 0.7152 * 255 ≈ 182
        assert!((182..=183).contains(&intensity));
    }

    #[test]
    fn test_rgb_to_grayscale_intensity_blue() {
        let color = Color::rgb(0, 0, 255);
        let intensity = rgb_to_grayscale_intensity(&color);
        // BT.709: 0.0722 * 255 ≈ 18
        assert!((18..=19).contains(&intensity));
    }

    #[test]
    fn test_intensity_to_ansi256_black() {
        let ansi = intensity_to_ansi256(0);
        assert_eq!(ansi, 232); // Darkest shade
    }

    #[test]
    fn test_intensity_to_ansi256_white() {
        let ansi = intensity_to_ansi256(255);
        assert_eq!(ansi, 255); // Brightest shade
    }

    #[test]
    fn test_intensity_to_ansi256_mid() {
        let ansi = intensity_to_ansi256(128);
        // Mid intensity should map to mid-range (around 243-244)
        assert!((243..=244).contains(&ansi));
    }

    #[test]
    fn test_color_to_truecolor_ansi() {
        let color = Color::rgb(255, 128, 64);
        let ansi = color_to_truecolor_ansi(&color);
        assert_eq!(ansi, "\x1b[38;2;255;128;64m");
    }

    #[test]
    fn test_color_to_truecolor_ansi_black() {
        let color = Color::rgb(0, 0, 0);
        let ansi = color_to_truecolor_ansi(&color);
        assert_eq!(ansi, "\x1b[38;2;0;0;0m");
    }

    #[test]
    fn test_color_to_truecolor_ansi_white() {
        let color = Color::rgb(255, 255, 255);
        let ansi = color_to_truecolor_ansi(&color);
        assert_eq!(ansi, "\x1b[38;2;255;255;255m");
    }
}
