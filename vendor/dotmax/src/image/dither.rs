//! Dithering algorithms for binary image conversion.
//!
//! This module provides three industry-standard dithering algorithms to convert
//! grayscale images to binary (black/white) images while preserving visual quality
//! through optimal error diffusion or ordered dithering patterns.
//!
//! # Algorithms
//!
//! ## Floyd-Steinberg (1976) - Error Diffusion
//!
//! The Floyd-Steinberg algorithm diffuses quantization error to neighboring pixels
//! using a carefully designed coefficient pattern. This produces the highest quality
//! results for photographs and complex images.
//!
//! **Reference:** Floyd, R. W.; Steinberg, L. (1976). "An Adaptive Algorithm for
//! Spatial Grey Scale". Proceedings of the Society of Information Display. 17: 75–77.
//!
//! **Characteristics:**
//! - Best quality (minimal visual artifacts)
//! - Slowest (error diffusion to 4 neighbors per pixel)
//! - Target performance: <15ms for 160×96 images
//!
//! ## Bayer Ordered Dithering - Threshold Matrix
//!
//! Bayer dithering uses an 8×8 threshold matrix to make binary decisions without
//! error propagation. This makes it stateless, parallelizable, and fast.
//!
//! **Reference:** Bayer, B. E. (1973). "An optimum method for two-level rendition
//! of continuous-tone pictures". IEEE International Conference on Communications.
//!
//! **Characteristics:**
//! - Good quality (visible pattern on uniform areas)
//! - Fastest (no error propagation, stateless)
//! - Target performance: <10ms for 160×96 images
//!
//! ## Atkinson (1984) - Partial Error Diffusion
//!
//! The Atkinson algorithm, developed by Bill Atkinson for Apple MacPaint, diffuses
//! only 75% of the quantization error (6/8), discarding 25%. This produces softer,
//! more artistic results compared to Floyd-Steinberg.
//!
//! **Reference:** Bill Atkinson, Apple Computer (1984). Algorithm used in MacPaint
//! and HyperCard.
//!
//! **Characteristics:**
//! - Artistic quality (softer than Floyd-Steinberg)
//! - Moderate speed (error diffusion to 6 neighbors)
//! - Target performance: <12ms for 160×96 images
//!
//! # Examples
//!
//! ## Using the unified API
//!
//! ```no_run
//! use dotmax::image::{to_grayscale, load_from_path};
//! use dotmax::image::dither::{apply_dithering, DitheringMethod};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("photo.jpg"))?;
//! let gray = to_grayscale(&img);
//! let binary = apply_dithering(&gray, DitheringMethod::FloydSteinberg)?;
//! println!("Dithered to {}×{} binary image", binary.width, binary.height);
//! # Ok(())
//! # }
//! ```
//!
//! ## Comparing algorithms
//!
//! ```no_run
//! # use dotmax::image::{to_grayscale, load_from_path};
//! # use dotmax::image::dither::{apply_dithering, DitheringMethod};
//! # use std::path::Path;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let img = load_from_path(Path::new("photo.jpg"))?;
//! # let gray = to_grayscale(&img);
//! // Try all three methods
//! let floyd = apply_dithering(&gray, DitheringMethod::FloydSteinberg)?;
//! let bayer = apply_dithering(&gray, DitheringMethod::Bayer)?;
//! let atkinson = apply_dithering(&gray, DitheringMethod::Atkinson)?;
//!
//! // Or skip dithering and use direct threshold
//! let direct = apply_dithering(&gray, DitheringMethod::None)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Performance Trade-offs
//!
//! | Algorithm | Speed | Quality | Best For |
//! |-----------|-------|---------|----------|
//! | Floyd-Steinberg | Slower | Highest | Photos, complex images |
//! | Bayer | Fastest | Good | Gradients, simple images |
//! | Atkinson | Moderate | Artistic | Line art, artistic renders |
//! | None (direct threshold) | Fast | Basic | When dithering not needed |

#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::float_cmp)]
#![allow(clippy::uninlined_format_args)]

use image::GrayImage;
use tracing::debug;

use crate::error::DotmaxError;
use crate::image::threshold::{apply_threshold, auto_threshold, otsu_threshold, BinaryImage};

/// Per-frame jitter parameters for ambient/temporal dithering.
///
/// Passing a non-zero `intensity` makes any dithering algorithm produce a
/// slightly different binary pattern for the same input image — the variation
/// is keyed off `seed`, so animating `seed` (e.g., from a frame counter or the
/// system clock) makes a still image visibly evolve frame-to-frame.
///
/// # Fields
///
/// - `seed`: 64-bit seed. Different seeds → different patterns. Same seed →
///   reproducible output (still useful for testing).
/// - `intensity`: 0.0 disables jitter (algorithms behave identically to the
///   non-jittered variants). 1.0 is full ambient noise. Sensible defaults are
///   in the 0.25–0.75 range.
#[derive(Debug, Clone, Copy)]
pub struct JitterParams {
    /// 64-bit seed driving the noise pattern. Different seeds → different
    /// patterns; advancing it per frame produces the ambient/temporal effect.
    pub seed: u64,
    /// Noise intensity in 0.0..=1.0. 0.0 disables jitter entirely.
    pub intensity: f32,
}

impl JitterParams {
    /// Disabled jitter — equivalent to deterministic dithering.
    pub const NONE: Self = Self {
        seed: 0,
        intensity: 0.0,
    };

    /// New jitter with a seed and intensity in 0.0..=1.0.
    #[must_use]
    pub fn new(seed: u64, intensity: f32) -> Self {
        Self {
            seed,
            intensity: intensity.clamp(0.0, 1.0),
        }
    }

    /// True iff jitter is active (intensity > 0).
    #[inline]
    #[must_use]
    pub fn enabled(self) -> bool {
        self.intensity > 0.0
    }
}

impl Default for JitterParams {
    fn default() -> Self {
        Self::NONE
    }
}

/// Inline 64-bit xorshift hash mixing (x, y, seed) into a u32.
///
/// Cheap, stateless, and good enough for noise injection. Used by the
/// jittered dither paths to drive blue-noise-like per-pixel perturbation
/// without pulling in a `rand` dependency.
#[inline]
fn hash_xy_seed(x: u32, y: u32, seed: u64) -> u32 {
    let mut h = seed
        .wrapping_add((x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    h ^= h >> 30;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 27;
    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 31;
    (h ^ (h >> 32)) as u32
}

/// Symmetric jitter in (-1.0, 1.0) for a given (x, y, seed) triple.
#[inline]
fn jitter_signed(x: u32, y: u32, seed: u64) -> f32 {
    // Map u32 to (-1.0, 1.0). The +1 avoids ever returning exactly -1.
    let v = hash_xy_seed(x, y, seed) as f64;
    ((v / (u32::MAX as f64)) * 2.0 - 1.0) as f32
}

/// Dithering algorithm selection.
///
/// This enum allows users to choose between different dithering algorithms,
/// each with different performance and quality characteristics.
///
/// # Examples
///
/// ```
/// use dotmax::image::dither::DitheringMethod;
///
/// // For best quality (photographs)
/// let method = DitheringMethod::FloydSteinberg;
///
/// // For fastest rendering (real-time)
/// let method = DitheringMethod::Bayer;
///
/// // For artistic output
/// let method = DitheringMethod::Atkinson;
///
/// // For no dithering (direct threshold)
/// let method = DitheringMethod::None;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DitheringMethod {
    /// Skip dithering and use direct Otsu threshold.
    ///
    /// This is the fastest option but produces the lowest quality for images
    /// with gradients or complex tonal ranges.
    None,

    /// Floyd-Steinberg error diffusion (1976).
    ///
    /// Diffuses quantization error to 4 neighbors with coefficients:
    /// - Right pixel (x+1, y): 7/16
    /// - Bottom-left (x-1, y+1): 3/16
    /// - Bottom (x, y+1): 5/16
    /// - Bottom-right (x+1, y+1): 1/16
    ///
    /// **Best for:** Photographs, complex images with many tonal variations.
    ///
    /// **Performance:** ~15ms for 160×96 images (slowest, highest quality).
    FloydSteinberg,

    /// Bayer ordered dithering with 8×8 threshold matrix.
    ///
    /// Uses a stateless ordered dithering pattern. No error propagation,
    /// making it parallelizable and fast.
    ///
    /// **Best for:** Gradients, simple images, real-time rendering.
    ///
    /// **Performance:** ~10ms for 160×96 images (fastest).
    Bayer,

    /// Atkinson error diffusion (1984, Apple MacPaint).
    ///
    /// Diffuses only 6/8 of quantization error to 6 neighbors (1/8 each),
    /// discarding 2/8. This produces softer, more artistic output compared
    /// to Floyd-Steinberg.
    ///
    /// **Best for:** Line art, diagrams, artistic rendering.
    ///
    /// **Performance:** ~12ms for 160×96 images (moderate).
    Atkinson,
}

/// Standard 8×8 Bayer threshold matrix.
///
/// Values range from 0-63 and are used to determine threshold points
/// in the ordered dithering pattern. The matrix is normalized to 0.0-1.0
/// range during threshold comparison.
///
/// Reference: Bayer, B. E. (1973). "An optimum method for two-level rendition
/// of continuous-tone pictures".
const BAYER_MATRIX_8X8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Ordered RGB quantization for native colored rasters. Unlike the binary
/// braille mapper, this retains chroma. Uses the maintained Bayer matrix;
/// no temporal seed, diffusion buffer, or per-frame pattern changes.
#[must_use]
pub fn bayer_rgb(rgb: [u8; 3], x: u32, y: u32) -> [u8; 3] {
    let threshold = u32::from(BAYER_MATRIX_8X8[(y & 7) as usize][(x & 7) as usize]);
    rgb.map(|channel| {
        // 32 levels per channel: quiet steps, not a monochrome dot mask.
        let value = u32::from(channel) * 31;
        let level = value / 255 + u32::from((value % 255) * 64 > threshold * 255);
        (level.min(31) * 255 / 31) as u8
    })
}

/// Threshold value for binary decision (middle gray).
///
/// Pixels with value >= THRESHOLD become white (true), pixels < THRESHOLD become black (false).
const THRESHOLD: u8 = 127;

/// Apply dithering to a grayscale image using the specified method.
///
/// This is the primary entry point for dithering. It dispatches to the appropriate
/// algorithm based on the `method` parameter.
///
/// # Arguments
///
/// * `gray` - Input grayscale image (8-bit per pixel)
/// * `method` - Dithering algorithm to use
///
/// # Returns
///
/// A binary image (boolean pixels) where `true` = black dot, `false` = white/empty.
///
/// # Errors
///
/// Returns [`DotmaxError::InvalidParameter`] if:
/// - Image width or height is 0
/// - Image dimensions would cause overflow
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{to_grayscale, load_from_path};
/// use dotmax::image::dither::{apply_dithering, DitheringMethod};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("photo.jpg"))?;
/// let gray = to_grayscale(&img);
///
/// // High quality dithering
/// let binary = apply_dithering(&gray, DitheringMethod::FloydSteinberg)?;
/// # Ok(())
/// # }
/// ```
pub fn apply_dithering(
    gray: &GrayImage,
    method: DitheringMethod,
) -> Result<BinaryImage, DotmaxError> {
    apply_dithering_with_custom_threshold(gray, method, None)
}

/// Apply dithering with a custom threshold value.
///
/// This allows combining dithering algorithms with manual threshold control.
/// If `threshold` is None, uses the default value of 127.
///
/// # Arguments
///
/// * `gray` - Grayscale image to dither
/// * `method` - Which dithering algorithm to use
/// * `threshold` - Optional custom threshold (0-255). None uses default 127.
///
/// # Returns
///
/// Returns a binary image (black/white pixels stored as booleans).
///
/// # Errors
///
/// Returns an error if:
/// - Image dimensions are zero
/// - Invalid parameters detected
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{to_grayscale, load_from_path};
/// use dotmax::image::dither::{apply_dithering_with_custom_threshold, DitheringMethod};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("photo.jpg"))?;
/// let gray = to_grayscale(&img);
/// // Use Floyd-Steinberg with darker threshold (100 instead of 127)
/// let binary = apply_dithering_with_custom_threshold(&gray, DitheringMethod::FloydSteinberg, Some(100))?;
/// # Ok(())
/// # }
/// ```
pub fn apply_dithering_with_custom_threshold(
    gray: &GrayImage,
    method: DitheringMethod,
    threshold: Option<u8>,
) -> Result<BinaryImage, DotmaxError> {
    apply_dithering_jittered(gray, method, threshold, JitterParams::NONE)
}

/// Apply dithering with both a custom threshold and per-frame jitter.
///
/// When `jitter.intensity > 0.0`, each algorithm injects bounded noise keyed
/// off `jitter.seed` so the same input produces visibly different output for
/// different seeds — that's the knob for ambient / temporal dithering. With
/// `JitterParams::NONE` this function behaves identically to
/// [`apply_dithering_with_custom_threshold`].
///
/// If `threshold` is `None`, the threshold is derived from Otsu's method on
/// the input image. (Previously Floyd-Steinberg / Atkinson silently used a
/// hardcoded 127 in that case, which produced fully-black or fully-white
/// output for any image whose tonal range didn't straddle 127 — the most
/// common cause of "the image isn't loading.")
///
/// # Errors
///
/// Returns [`DotmaxError::InvalidParameter`] if image dimensions are 0.
pub fn apply_dithering_jittered(
    gray: &GrayImage,
    method: DitheringMethod,
    threshold: Option<u8>,
    jitter: JitterParams,
) -> Result<BinaryImage, DotmaxError> {
    // Bug fix: when no manual threshold is set, prefer Otsu over the fixed
    // midpoint so FS / Atkinson don't collapse high-contrast images into a
    // solid block.
    let threshold_value = threshold.unwrap_or_else(|| {
        if matches!(
            method,
            DitheringMethod::FloydSteinberg | DitheringMethod::Atkinson
        ) {
            otsu_threshold(gray)
        } else {
            THRESHOLD
        }
    });

    debug!(
        "Applying {:?} dithering to {}×{} image (threshold {}, jitter intensity={:.2}, seed={})",
        method,
        gray.width(),
        gray.height(),
        threshold_value,
        jitter.intensity,
        jitter.seed,
    );

    match method {
        DitheringMethod::None => {
            if jitter.enabled() {
                // Random-noise threshold: produces a stippled blue-noise-like
                // result that re-stipples per seed.
                Ok(noise_threshold_jittered(gray, threshold_value, jitter))
            } else if let Some(t) = threshold {
                Ok(apply_threshold(gray, t))
            } else {
                Ok(auto_threshold(&image::DynamicImage::ImageLuma8(
                    gray.clone(),
                )))
            }
        }
        DitheringMethod::FloydSteinberg => floyd_steinberg_jittered(gray, threshold_value, jitter),
        DitheringMethod::Bayer => bayer_jittered(gray, threshold_value, jitter),
        DitheringMethod::Atkinson => atkinson_jittered(gray, threshold_value, jitter),
    }
}

/// Floyd-Steinberg error diffusion dithering.
///
/// Implements the classic Floyd-Steinberg algorithm (1976) which diffuses
/// quantization error to 4 neighboring pixels. This produces high-quality
/// results but is the slowest of the three methods.
///
/// # Algorithm
///
/// For each pixel (left to right, top to bottom):
/// 1. Calculate `new_value = old_value + accumulated_error`
/// 2. Apply threshold: `output = if new_value >= 127 { 255 } else { 0 }`
/// 3. Calculate error: `error = new_value - output`
/// 4. Diffuse error to neighbors:
///    - Right (x+1, y): `error * 7/16`
///    - Bottom-left (x-1, y+1): `error * 3/16`
///    - Bottom (x, y+1): `error * 5/16`
///    - Bottom-right (x+1, y+1): `error * 1/16`
///
/// # Performance
///
/// Target: <15ms for 160×96 images
///
/// # Errors
///
/// Returns [`DotmaxError::InvalidParameter`] if image dimensions are 0.
///
/// # Examples
///
/// ```no_run
/// # use dotmax::image::{to_grayscale, load_from_path};
/// # use dotmax::image::dither::floyd_steinberg;
/// # use std::path::Path;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let img = load_from_path(Path::new("photo.jpg"))?;
/// # let gray = to_grayscale(&img);
/// let binary = floyd_steinberg(&gray)?;
/// # Ok(())
/// # }
/// ```
pub fn floyd_steinberg(gray: &GrayImage) -> Result<BinaryImage, DotmaxError> {
    floyd_steinberg_jittered(gray, THRESHOLD, JitterParams::NONE)
}

#[allow(dead_code)] // kept for crate-internal callers / tests
fn floyd_steinberg_with_threshold(
    gray: &GrayImage,
    threshold: u8,
) -> Result<BinaryImage, DotmaxError> {
    floyd_steinberg_jittered(gray, threshold, JitterParams::NONE)
}

fn floyd_steinberg_jittered(
    gray: &GrayImage,
    threshold: u8,
    jitter: JitterParams,
) -> Result<BinaryImage, DotmaxError> {
    let width = gray.width() as usize;
    let height = gray.height() as usize;

    if width == 0 || height == 0 {
        return Err(DotmaxError::InvalidParameter {
            parameter_name: "image dimensions".to_string(),
            value: format!("{}×{}", width, height),
            min: "1×1".to_string(),
            max: "unlimited".to_string(),
        });
    }

    debug!(
        "Floyd-Steinberg dithering {}×{} image (jitter intensity {:.2})",
        width, height, jitter.intensity
    );

    let mut errors = vec![0.0f32; width * height];
    let mut binary = BinaryImage::new(width as u32, height as u32);

    // Per-pixel input perturbation magnitude. ±32 at intensity=1.0 is enough
    // to dissolve uniform regions into stipple without losing edges.
    let noise_amp = 32.0 * jitter.intensity;

    // Toggle serpentine direction across alternating frames so the diffusion
    // grain itself shifts. Even rows always go left-to-right; odd rows flip
    // when the seed's low bit is set.
    let serpentine_flip = (jitter.seed & 1) != 0;

    for y in 0..height {
        let row_reversed = serpentine_flip && (y & 1 == 1);
        let xs: Box<dyn Iterator<Item = usize>> = if row_reversed {
            Box::new((0..width).rev())
        } else {
            Box::new(0..width)
        };

        for x in xs {
            let pixel_idx = y * width + x;
            let old_pixel = gray.get_pixel(x as u32, y as u32)[0] as f32;
            let mut new_pixel = old_pixel + errors[pixel_idx];

            if jitter.enabled() {
                new_pixel += jitter_signed(x as u32, y as u32, jitter.seed) * noise_amp;
            }

            let output_value = if new_pixel >= threshold as f32 {
                255.0
            } else {
                0.0
            };
            binary.set_pixel(x as u32, y as u32, output_value == 255.0);

            let quant_error = new_pixel - output_value;

            // Mirror the neighbour offsets when traversing right-to-left so the
            // serpentine sweep diffuses error in the direction of travel.
            let dx_forward: isize = if row_reversed { -1 } else { 1 };

            // "Right" neighbour in the direction of travel
            let nx = x as isize + dx_forward;
            if nx >= 0 && (nx as usize) < width {
                errors[pixel_idx.wrapping_add_signed(dx_forward)] += quant_error * 7.0 / 16.0;
            }

            if y + 1 < height {
                let next_row_idx = (y + 1) * width;

                // Diagonal "behind"
                let nx_back = x as isize - dx_forward;
                if nx_back >= 0 && (nx_back as usize) < width {
                    errors[next_row_idx + nx_back as usize] += quant_error * 3.0 / 16.0;
                }

                errors[next_row_idx + x] += quant_error * 5.0 / 16.0;

                // Diagonal "ahead"
                if nx >= 0 && (nx as usize) < width {
                    errors[next_row_idx + nx as usize] += quant_error * 1.0 / 16.0;
                }
            }
        }
    }

    Ok(binary)
}

/// Bayer ordered dithering with 8×8 threshold matrix.
///
/// Implements stateless ordered dithering using a standard 8×8 Bayer matrix.
/// Each pixel's binary value is determined independently based on its position
/// in the matrix pattern, making this algorithm fast and parallelizable.
///
/// # Algorithm
///
/// For each pixel at (x, y):
/// 1. Get Bayer threshold: `bayer_matrix[y % 8][x % 8] / 64.0`
/// 2. Compare: `if (pixel_value / 255.0) > bayer_threshold { white } else { black }`
///
/// # Performance
///
/// Target: <10ms for 160×96 images (fastest algorithm)
///
/// # Errors
///
/// Returns [`DotmaxError::InvalidParameter`] if image dimensions are 0.
///
/// # Examples
///
/// ```no_run
/// # use dotmax::image::{to_grayscale, load_from_path};
/// # use dotmax::image::dither::bayer;
/// # use std::path::Path;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let img = load_from_path(Path::new("gradient.png"))?;
/// # let gray = to_grayscale(&img);
/// let binary = bayer(&gray)?;
/// # Ok(())
/// # }
/// ```
pub fn bayer(gray: &GrayImage) -> Result<BinaryImage, DotmaxError> {
    bayer_jittered(gray, THRESHOLD, JitterParams::NONE)
}

fn bayer_jittered(
    gray: &GrayImage,
    threshold: u8,
    jitter: JitterParams,
) -> Result<BinaryImage, DotmaxError> {
    let width = gray.width() as usize;
    let height = gray.height() as usize;

    if width == 0 || height == 0 {
        return Err(DotmaxError::InvalidParameter {
            parameter_name: "image dimensions".to_string(),
            value: format!("{}×{}", width, height),
            min: "1×1".to_string(),
            max: "unlimited".to_string(),
        });
    }

    debug!(
        "Bayer dithering {}×{} image (jitter intensity {:.2})",
        width, height, jitter.intensity
    );

    // Offset the matrix lookup per frame so the same image visibly shifts.
    let (off_x, off_y) = if jitter.enabled() {
        (
            (jitter.seed & 0x7) as usize,
            ((jitter.seed >> 3) & 0x7) as usize,
        )
    } else {
        (0, 0)
    };

    // Normalize the manual threshold around the matrix midpoint (32/64) so a
    // user-supplied threshold biases the whole pattern up or down.
    let threshold_bias = (threshold as f32 / 255.0) - 0.5;

    // Per-pixel symmetric noise on the comparison value.
    let noise_amp = 0.18 * jitter.intensity;

    let mut binary = BinaryImage::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let pixel_value = gray.get_pixel(x as u32, y as u32)[0];
            let bayer_threshold = BAYER_MATRIX_8X8[(y + off_y) % 8][(x + off_x) % 8] as f32 / 64.0;

            let mut comparison = pixel_value as f32 / 255.0 + threshold_bias;
            if jitter.enabled() {
                comparison += jitter_signed(x as u32, y as u32, jitter.seed) * noise_amp;
            }

            binary.set_pixel(x as u32, y as u32, comparison > bayer_threshold);
        }
    }

    Ok(binary)
}

/// Atkinson error diffusion dithering.
///
/// Implements the Atkinson algorithm developed by Bill Atkinson for Apple MacPaint (1984).
/// This algorithm diffuses only 6/8 of the quantization error to 6 neighbors (1/8 each),
/// discarding 2/8. This produces softer, more artistic results than Floyd-Steinberg.
///
/// # Algorithm
///
/// For each pixel (left to right, top to bottom):
/// 1. Calculate `new_value = old_value + accumulated_error`
/// 2. Apply threshold: `output = if new_value >= 127 { 255 } else { 0 }`
/// 3. Calculate error: `error = new_value - output`
/// 4. Diffuse 1/8 of error to each of 6 neighbors:
///    - Right (x+1, y)
///    - Two-right (x+2, y)
///    - Bottom-left (x-1, y+1)
///    - Bottom (x, y+1)
///    - Bottom-right (x+1, y+1)
///    - Two-down (x, y+2)
/// 5. Discard 2/8 of error (Atkinson's signature)
///
/// # Performance
///
/// Target: <12ms for 160×96 images (moderate speed)
///
/// # Errors
///
/// Returns [`DotmaxError::InvalidParameter`] if image dimensions are 0.
///
/// # Examples
///
/// ```no_run
/// # use dotmax::image::{to_grayscale, load_from_path};
/// # use dotmax::image::dither::atkinson;
/// # use std::path::Path;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let img = load_from_path(Path::new("lineart.png"))?;
/// # let gray = to_grayscale(&img);
/// let binary = atkinson(&gray)?;
/// # Ok(())
/// # }
/// ```
pub fn atkinson(gray: &GrayImage) -> Result<BinaryImage, DotmaxError> {
    atkinson_jittered(gray, THRESHOLD, JitterParams::NONE)
}

#[allow(dead_code)]
fn atkinson_with_threshold(gray: &GrayImage, threshold: u8) -> Result<BinaryImage, DotmaxError> {
    atkinson_jittered(gray, threshold, JitterParams::NONE)
}

fn atkinson_jittered(
    gray: &GrayImage,
    threshold: u8,
    jitter: JitterParams,
) -> Result<BinaryImage, DotmaxError> {
    let width = gray.width() as usize;
    let height = gray.height() as usize;

    if width == 0 || height == 0 {
        return Err(DotmaxError::InvalidParameter {
            parameter_name: "image dimensions".to_string(),
            value: format!("{}×{}", width, height),
            min: "1×1".to_string(),
            max: "unlimited".to_string(),
        });
    }

    debug!(
        "Atkinson dithering {}×{} image (jitter intensity {:.2})",
        width, height, jitter.intensity
    );

    let mut errors = vec![0.0f32; width * height];
    let mut binary = BinaryImage::new(width as u32, height as u32);

    let noise_amp = 28.0 * jitter.intensity;
    let serpentine_flip = (jitter.seed & 1) != 0;

    for y in 0..height {
        let row_reversed = serpentine_flip && (y & 1 == 1);
        let xs: Box<dyn Iterator<Item = usize>> = if row_reversed {
            Box::new((0..width).rev())
        } else {
            Box::new(0..width)
        };

        for x in xs {
            let pixel_idx = y * width + x;
            let old_pixel = gray.get_pixel(x as u32, y as u32)[0] as f32;
            let mut new_pixel = old_pixel + errors[pixel_idx];

            if jitter.enabled() {
                new_pixel += jitter_signed(x as u32, y as u32, jitter.seed) * noise_amp;
            }

            let output_value = if new_pixel >= threshold as f32 {
                255.0
            } else {
                0.0
            };
            binary.set_pixel(x as u32, y as u32, output_value == 255.0);

            let quant_error = new_pixel - output_value;
            let dx_forward: isize = if row_reversed { -1 } else { 1 };

            // +1 in direction of travel
            let nx1 = x as isize + dx_forward;
            if nx1 >= 0 && (nx1 as usize) < width {
                errors[(y * width).wrapping_add_signed(nx1)] += quant_error / 8.0;
            }
            // +2 in direction of travel
            let nx2 = x as isize + 2 * dx_forward;
            if nx2 >= 0 && (nx2 as usize) < width {
                errors[(y * width).wrapping_add_signed(nx2)] += quant_error / 8.0;
            }

            if y + 1 < height {
                let next_row_idx = (y + 1) * width;
                let nx_back = x as isize - dx_forward;
                if nx_back >= 0 && (nx_back as usize) < width {
                    errors[next_row_idx + nx_back as usize] += quant_error / 8.0;
                }
                errors[next_row_idx + x] += quant_error / 8.0;
                if nx1 >= 0 && (nx1 as usize) < width {
                    errors[next_row_idx + nx1 as usize] += quant_error / 8.0;
                }
            }

            if y + 2 < height {
                errors[(y + 2) * width + x] += quant_error / 8.0;
            }
        }
    }

    Ok(binary)
}

/// Pure noise-thresholding: per-pixel random comparison around `threshold`.
///
/// Used by `DitheringMethod::None` whenever jitter is enabled — produces a
/// stipple pattern that fully reshuffles per seed instead of a hard binary
/// step.
fn noise_threshold_jittered(gray: &GrayImage, threshold: u8, jitter: JitterParams) -> BinaryImage {
    let width = gray.width();
    let height = gray.height();
    let mut binary = BinaryImage::new(width, height);

    // Map threshold to a comparison level in 0..1, then bias each pixel by
    // signed noise scaled by intensity.
    let level = threshold as f32 / 255.0;
    let amp = 0.5 * jitter.intensity; // up to ±0.5 — enough to fully randomize

    for (i, p) in gray.pixels().enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        let v = p[0] as f32 / 255.0;
        let cmp = v + jitter_signed(x, y, jitter.seed) * amp;
        binary.pixels[i] = cmp >= level;
    }

    binary
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    /// Helper: Create a uniform gray image (all pixels same value)
    fn create_uniform_image(width: u32, height: u32, value: u8) -> GrayImage {
        GrayImage::from_fn(width, height, |_, _| Luma([value]))
    }

    /// Helper: Create a gradient image (0 to 255 smooth gradient left to right)
    fn create_gradient_image(width: u32, height: u32) -> GrayImage {
        GrayImage::from_fn(width, height, |x, _| {
            let value = (x as f32 / width as f32 * 255.0) as u8;
            Luma([value])
        })
    }

    // ===== DitheringMethod Enum Tests =====

    #[test]
    fn test_dithering_method_enum() {
        // Verify all variants exist
        let _ = DitheringMethod::None;
        let _ = DitheringMethod::FloydSteinberg;
        let _ = DitheringMethod::Bayer;
        let _ = DitheringMethod::Atkinson;
    }

    #[test]
    fn test_dithering_method_derives() {
        let method1 = DitheringMethod::FloydSteinberg;
        let method2 = DitheringMethod::FloydSteinberg;
        let method3 = DitheringMethod::Bayer;

        // Test Debug
        assert_eq!(format!("{:?}", method1), "FloydSteinberg");

        // Test Clone (Copy types can also clone)
        #[allow(clippy::clone_on_copy)]
        let method1_clone = method1.clone();
        assert_eq!(method1, method1_clone);

        // Test Copy
        let method1_copy = method1;
        assert_eq!(method1, method1_copy);

        // Test PartialEq
        assert_eq!(method1, method2);
        assert_ne!(method1, method3);

        // Test Eq (no separate test needed, derives from PartialEq)
    }

    // ===== Floyd-Steinberg Tests =====

    #[test]
    fn test_floyd_steinberg_uniform_gray() {
        let gray = create_uniform_image(10, 10, 128);
        let binary = floyd_steinberg(&gray).unwrap();

        // For uniform gray (128), expect roughly 50% black/white distribution
        let black_count = binary.pixels.iter().filter(|&&p| p).count();
        let total = binary.pixels.len();

        // Should be roughly balanced (within 30% tolerance for small image)
        assert!(
            (black_count as f32 / total as f32) > 0.3 && (black_count as f32 / total as f32) < 0.7,
            "Expected balanced black/white for uniform gray, got {} black out of {}",
            black_count,
            total
        );
    }

    #[test]
    fn test_floyd_steinberg_all_black() {
        let gray = create_uniform_image(10, 10, 0);
        let binary = floyd_steinberg(&gray).unwrap();

        // All pixels should be black (false)
        assert!(
            binary.pixels.iter().all(|&p| !p),
            "Expected all black for input value 0"
        );
    }

    #[test]
    fn test_floyd_steinberg_all_white() {
        let gray = create_uniform_image(10, 10, 255);
        let binary = floyd_steinberg(&gray).unwrap();

        // All pixels should be white (true)
        assert!(
            binary.pixels.iter().all(|&p| p),
            "Expected all white for input value 255"
        );
    }

    #[test]
    fn test_floyd_steinberg_gradient() {
        let gray = create_gradient_image(100, 10);
        let binary = floyd_steinberg(&gray).unwrap();

        // Left side (dark) should have more black, right side (bright) more white
        let left_quarter = &binary.pixels[0..250]; // First 25% of pixels
        let right_quarter = &binary.pixels[750..1000]; // Last 25% of pixels

        let left_black = left_quarter.iter().filter(|&&p| !p).count();
        let right_black = right_quarter.iter().filter(|&&p| !p).count();

        assert!(
            left_black > right_black,
            "Expected more black on left (dark) side, got left={} right={}",
            left_black,
            right_black
        );
    }

    #[test]
    fn test_floyd_steinberg_small_image() {
        // Test 1×1 image (edge case)
        let gray = create_uniform_image(1, 1, 128);
        let binary = floyd_steinberg(&gray).unwrap();
        assert_eq!(binary.width, 1);
        assert_eq!(binary.height, 1);

        // Test 2×2 image
        let gray = create_uniform_image(2, 2, 128);
        let binary = floyd_steinberg(&gray).unwrap();
        assert_eq!(binary.width, 2);
        assert_eq!(binary.height, 2);

        // Test 3×3 image
        let gray = create_uniform_image(3, 3, 128);
        let binary = floyd_steinberg(&gray).unwrap();
        assert_eq!(binary.width, 3);
        assert_eq!(binary.height, 3);
    }

    #[test]
    fn test_floyd_steinberg_zero_dimensions() {
        // Zero width
        let gray = GrayImage::new(0, 10);
        assert!(floyd_steinberg(&gray).is_err());

        // Zero height
        let gray = GrayImage::new(10, 0);
        assert!(floyd_steinberg(&gray).is_err());

        // Both zero
        let gray = GrayImage::new(0, 0);
        assert!(floyd_steinberg(&gray).is_err());
    }

    // ===== Bayer Tests =====

    #[test]
    fn test_bayer_uniform_gray() {
        let gray = create_uniform_image(16, 16, 128);
        let binary = bayer(&gray).unwrap();

        // For uniform gray (128 = 50%), expect Bayer pattern visible
        let black_count = binary.pixels.iter().filter(|&&p| p).count();
        let total = binary.pixels.len();

        // Should be roughly balanced (within 30% tolerance)
        assert!(
            (black_count as f32 / total as f32) > 0.3 && (black_count as f32 / total as f32) < 0.7,
            "Expected balanced black/white for uniform gray, got {} black out of {}",
            black_count,
            total
        );
    }

    #[test]
    fn test_bayer_all_black() {
        let gray = create_uniform_image(16, 16, 0);
        let binary = bayer(&gray).unwrap();

        // All pixels should be black (false)
        assert!(
            binary.pixels.iter().all(|&p| !p),
            "Expected all black for input value 0"
        );
    }

    #[test]
    fn test_bayer_all_white() {
        let gray = create_uniform_image(16, 16, 255);
        let binary = bayer(&gray).unwrap();

        // All pixels should be white (true)
        assert!(
            binary.pixels.iter().all(|&p| p),
            "Expected all white for input value 255"
        );
    }

    #[test]
    fn test_bayer_deterministic() {
        // Same input should always produce same output
        let gray = create_uniform_image(10, 10, 128);
        let binary1 = bayer(&gray).unwrap();
        let binary2 = bayer(&gray).unwrap();

        assert_eq!(
            binary1.pixels, binary2.pixels,
            "Bayer should be deterministic"
        );
    }

    #[test]
    fn test_bayer_pattern_applied() {
        // Verify that Bayer matrix is being applied (pattern should be visible)
        let gray = create_uniform_image(8, 8, 128);
        let binary = bayer(&gray).unwrap();

        // For an 8×8 image with uniform gray, the Bayer pattern should create
        // a specific pattern. We can't predict exact output, but we can verify
        // it's not all black or all white.
        let black_count = binary.pixels.iter().filter(|&&p| p).count();
        assert!(
            black_count > 0 && black_count < binary.pixels.len(),
            "Bayer pattern should produce mixed output for uniform gray"
        );
    }

    #[test]
    fn test_bayer_small_image() {
        // Test 1×1 image
        let gray = create_uniform_image(1, 1, 128);
        let binary = bayer(&gray).unwrap();
        assert_eq!(binary.width, 1);
        assert_eq!(binary.height, 1);

        // Test 2×2 image
        let gray = create_uniform_image(2, 2, 128);
        let binary = bayer(&gray).unwrap();
        assert_eq!(binary.width, 2);
        assert_eq!(binary.height, 2);
    }

    #[test]
    fn test_bayer_zero_dimensions() {
        // Zero width
        let gray = GrayImage::new(0, 10);
        assert!(bayer(&gray).is_err());

        // Zero height
        let gray = GrayImage::new(10, 0);
        assert!(bayer(&gray).is_err());
    }

    // ===== Atkinson Tests =====

    #[test]
    fn test_atkinson_uniform_gray() {
        let gray = create_uniform_image(10, 10, 128);
        let binary = atkinson(&gray).unwrap();

        // For uniform gray (128), expect mixed output
        let black_count = binary.pixels.iter().filter(|&&p| p).count();
        let total = binary.pixels.len();

        // Should have some variation (within 30% tolerance)
        assert!(
            (black_count as f32 / total as f32) > 0.3 && (black_count as f32 / total as f32) < 0.7,
            "Expected balanced black/white for uniform gray, got {} black out of {}",
            black_count,
            total
        );
    }

    #[test]
    fn test_atkinson_all_black() {
        let gray = create_uniform_image(10, 10, 0);
        let binary = atkinson(&gray).unwrap();

        // All pixels should be black (false)
        assert!(
            binary.pixels.iter().all(|&p| !p),
            "Expected all black for input value 0"
        );
    }

    #[test]
    fn test_atkinson_all_white() {
        let gray = create_uniform_image(10, 10, 255);
        let binary = atkinson(&gray).unwrap();

        // All pixels should be white (true)
        assert!(
            binary.pixels.iter().all(|&p| p),
            "Expected all white for input value 255"
        );
    }

    #[test]
    fn test_atkinson_gradient() {
        let gray = create_gradient_image(100, 10);
        let binary = atkinson(&gray).unwrap();

        // Left side (dark) should have more black, right side (bright) more white
        let left_quarter = &binary.pixels[0..250]; // First 25% of pixels
        let right_quarter = &binary.pixels[750..1000]; // Last 25% of pixels

        let left_black = left_quarter.iter().filter(|&&p| !p).count();
        let right_black = right_quarter.iter().filter(|&&p| !p).count();

        assert!(
            left_black > right_black,
            "Expected more black on left (dark) side, got left={} right={}",
            left_black,
            right_black
        );
    }

    #[test]
    fn test_atkinson_small_image() {
        // Test 1×1 image
        let gray = create_uniform_image(1, 1, 128);
        let binary = atkinson(&gray).unwrap();
        assert_eq!(binary.width, 1);
        assert_eq!(binary.height, 1);

        // Test 2×2 image
        let gray = create_uniform_image(2, 2, 128);
        let binary = atkinson(&gray).unwrap();
        assert_eq!(binary.width, 2);
        assert_eq!(binary.height, 2);
    }

    #[test]
    fn test_atkinson_zero_dimensions() {
        // Zero width
        let gray = GrayImage::new(0, 10);
        assert!(atkinson(&gray).is_err());

        // Zero height
        let gray = GrayImage::new(10, 0);
        assert!(atkinson(&gray).is_err());
    }

    // ===== apply_dithering() Tests =====

    #[test]
    fn test_apply_dithering_none() {
        let gray = create_uniform_image(10, 10, 128);
        let binary = apply_dithering(&gray, DitheringMethod::None).unwrap();

        // Should use auto_threshold fallback
        assert_eq!(binary.width, 10);
        assert_eq!(binary.height, 10);
    }

    #[test]
    fn test_apply_dithering_floyd_steinberg() {
        let gray = create_uniform_image(10, 10, 128);
        let binary = apply_dithering(&gray, DitheringMethod::FloydSteinberg).unwrap();

        assert_eq!(binary.width, 10);
        assert_eq!(binary.height, 10);
    }

    #[test]
    fn test_apply_dithering_bayer() {
        let gray = create_uniform_image(10, 10, 128);
        let binary = apply_dithering(&gray, DitheringMethod::Bayer).unwrap();

        assert_eq!(binary.width, 10);
        assert_eq!(binary.height, 10);
    }

    #[test]
    fn test_apply_dithering_atkinson() {
        let gray = create_uniform_image(10, 10, 128);
        let binary = apply_dithering(&gray, DitheringMethod::Atkinson).unwrap();

        assert_eq!(binary.width, 10);
        assert_eq!(binary.height, 10);
    }

    #[test]
    fn test_all_algorithms_same_dimensions() {
        let gray = create_uniform_image(20, 15, 128);

        let none = apply_dithering(&gray, DitheringMethod::None).unwrap();
        let floyd = apply_dithering(&gray, DitheringMethod::FloydSteinberg).unwrap();
        let bayer = apply_dithering(&gray, DitheringMethod::Bayer).unwrap();
        let atkinson = apply_dithering(&gray, DitheringMethod::Atkinson).unwrap();

        // All should preserve dimensions
        assert_eq!(none.width, 20);
        assert_eq!(none.height, 15);
        assert_eq!(floyd.width, 20);
        assert_eq!(floyd.height, 15);
        assert_eq!(bayer.width, 20);
        assert_eq!(bayer.height, 15);
        assert_eq!(atkinson.width, 20);
        assert_eq!(atkinson.height, 15);
    }

    #[test]
    fn test_algorithms_produce_different_output() {
        // All three algorithms should produce different patterns for same input
        let gray = create_uniform_image(20, 20, 128);

        let floyd = apply_dithering(&gray, DitheringMethod::FloydSteinberg).unwrap();
        let bayer = apply_dithering(&gray, DitheringMethod::Bayer).unwrap();
        let atkinson = apply_dithering(&gray, DitheringMethod::Atkinson).unwrap();

        // They should not all be identical
        let all_same = floyd.pixels == bayer.pixels && floyd.pixels == atkinson.pixels;
        assert!(
            !all_same,
            "Expected different patterns from different algorithms"
        );
    }

    // ===== Integration Tests =====

    #[test]
    fn test_large_image_performance_check() {
        // Test with 160×96 image (standard terminal size in pixels)
        let gray = create_uniform_image(160, 96, 128);

        // All algorithms should complete without panic
        let _ = floyd_steinberg(&gray).unwrap();
        let _ = bayer(&gray).unwrap();
        let _ = atkinson(&gray).unwrap();
    }

    #[test]
    fn test_extreme_dimensions() {
        // Very wide image
        let gray = create_uniform_image(1000, 1, 128);
        assert!(floyd_steinberg(&gray).is_ok());
        assert!(bayer(&gray).is_ok());
        assert!(atkinson(&gray).is_ok());

        // Very tall image
        let gray = create_uniform_image(1, 1000, 128);
        assert!(floyd_steinberg(&gray).is_ok());
        assert!(bayer(&gray).is_ok());
        assert!(atkinson(&gray).is_ok());
    }
}
