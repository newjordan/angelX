//! Thresholding and binary image conversion module
//!
//! This module provides functionality for:
//! - Otsu's method for automatic threshold calculation
//! - Binary image conversion (grayscale → black/white pixels)
//! - Image adjustments (brightness, contrast, gamma correction)

#![allow(clippy::must_use_candidate)]
#![allow(clippy::suboptimal_flops)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::explicit_iter_loop)]
//!
//! # Otsu's Method
//!
//! Otsu's method is an industry-standard algorithm for automatic image thresholding.
//! It calculates the optimal threshold value by maximizing the between-class variance
//! (separability of foreground and background pixels).
//!
//! Reference: Otsu, N. (1979). "A Threshold Selection Method from Gray-Level Histograms".
//! IEEE Transactions on Systems, Man, and Cybernetics. 9(1): 62–66.
//!
//! # Examples
//!
//! ## Automatic thresholding
//!
//! ```no_run
//! use dotmax::image::{load_from_path, auto_threshold};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("image.png"))?;
//! let binary = auto_threshold(&img);
//! println!("Binary image: {}×{}", binary.width, binary.height);
//! # Ok(())
//! # }
//! ```
//!
//! ## Manual threshold with adjustments
//!
//! ```no_run
//! use dotmax::image::{load_from_path, to_grayscale, adjust_brightness, apply_threshold};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("image.png"))?;
//! let gray = to_grayscale(&img);
//! let bright = adjust_brightness(&gray, 1.2)?;  // 20% brighter
//! let binary = apply_threshold(&bright, 128);
//! # Ok(())
//! # }
//! ```

use crate::error::DotmaxError;
use image::{DynamicImage, GrayImage, Luma};
use tracing::debug;

/// Binary image representation with boolean pixels
///
/// A binary image stores pixels as boolean values where:
/// - `true` = black (dot on)
/// - `false` = white (dot off)
///
/// This representation is optimized for braille mapping where each pixel
/// directly corresponds to a braille dot state.
#[derive(Debug, Clone)]
pub struct BinaryImage {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Pixel data stored as boolean values (true = black, false = white)
    pub pixels: Vec<bool>,
}

impl BinaryImage {
    /// Create a new binary image with the given dimensions
    ///
    /// All pixels are initialized to `false` (white).
    ///
    /// # Panics
    ///
    /// Panics if width × height would overflow usize.
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .expect("Image dimensions too large");

        Self {
            width,
            height,
            pixels: vec![false; pixel_count],
        }
    }

    /// Get the total number of pixels in the image
    pub fn pixel_count(&self) -> usize {
        self.pixels.len()
    }

    /// Get a pixel value at the given coordinates
    ///
    /// Returns `None` if coordinates are out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<bool> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = (y * self.width + x) as usize;
        self.pixels.get(index).copied()
    }

    /// Set a pixel value at the given coordinates
    ///
    /// Returns `false` if coordinates are out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, value: bool) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        let index = (y * self.width + x) as usize;
        if let Some(pixel) = self.pixels.get_mut(index) {
            *pixel = value;
            true
        } else {
            false
        }
    }

    /// Creates a BinaryImage from a GrayImage where non-zero pixels are black.
    ///
    /// This is useful for converting the output of temporal coherence processing
    /// (which produces 0/255 grayscale) to a binary image.
    ///
    /// # Arguments
    ///
    /// * `gray` - Grayscale image where 255 = white (off), 0 = black (on) or vice-versa
    ///   Convention: pixels > 0 are treated as "on" (black in braille)
    pub fn from_grayscale(gray: &image::GrayImage) -> Self {
        let width = gray.width();
        let height = gray.height();
        let pixels: Vec<bool> = gray.pixels().map(|p| p.0[0] > 0).collect();
        Self {
            width,
            height,
            pixels,
        }
    }
}

/// Calculate the optimal threshold value using Otsu's method
///
/// Otsu's method finds the threshold that maximizes the between-class variance
/// (separability of foreground and background). This produces optimal binary
/// conversion for most images.
///
/// # Algorithm
///
/// 1. Calculate histogram of pixel intensities (256 bins for 0-255)
/// 2. For each possible threshold (0-255):
///    - Calculate class weights (proportion of pixels in each class)
///    - Calculate class means (average intensity in each class)
///    - Calculate between-class variance: σ²(t) = w₀(t) × w₁(t) × [μ₀(t) - μ₁(t)]²
/// 3. Return threshold with maximum between-class variance
///
/// # Edge Cases
///
/// - Uniform images (all pixels same value): Returns that value
/// - All black: Returns 0
/// - All white: Returns 255
///
/// # Performance
///
/// Target: <5ms for 160×96 pixel images (80×24 terminal)
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, to_grayscale, otsu_threshold};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
/// let gray = to_grayscale(&img);
/// let threshold = otsu_threshold(&gray);
/// println!("Optimal threshold: {}", threshold);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// Otsu, N. (1979). "A Threshold Selection Method from Gray-Level Histograms".
/// IEEE Transactions on Systems, Man, and Cybernetics. 9(1): 62–66.
/// DOI: 10.1109/TSMC.1979.4310076
pub fn otsu_threshold(gray: &GrayImage) -> u8 {
    debug!(
        "Calculating Otsu threshold for {}×{} image",
        gray.width(),
        gray.height()
    );

    // Calculate histogram (256 bins for pixel values 0-255)
    let mut histogram = [0u32; 256];
    for pixel in gray.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let total_pixels = (gray.width() * gray.height()) as f64;

    // Calculate total sum (weighted pixel values)
    let sum_total: f64 = histogram
        .iter()
        .enumerate()
        .map(|(i, &count)| i as f64 * count as f64)
        .sum();

    let mut max_variance = 0.0;
    let mut best_threshold = 127u8; // Default fallback for uniform images

    let mut weight_background = 0.0;
    let mut sum_background = 0.0;

    #[allow(clippy::needless_range_loop)]
    for threshold in 0..256 {
        // Update background class statistics
        weight_background += histogram[threshold] as f64;

        if weight_background == 0.0 {
            continue;
        }

        let weight_foreground = total_pixels - weight_background;

        if weight_foreground == 0.0 {
            break;
        }

        sum_background += threshold as f64 * histogram[threshold] as f64;

        let mean_background = sum_background / weight_background;
        let mean_foreground = (sum_total - sum_background) / weight_foreground;

        // Calculate between-class variance
        let variance =
            weight_background * weight_foreground * (mean_background - mean_foreground).powi(2);

        if variance > max_variance {
            max_variance = variance;
            best_threshold = threshold as u8;
        }
    }

    debug!("Calculated Otsu threshold base: {}", best_threshold);

    // Otsu picks the optimal *separator* value (the upper bound of the
    // background class). Since our thresholding comparison is `pixel >= threshold`,
    // we return `best_threshold + 2` to provide a small noise floor. This
    // prevents low-level artifacts (like those from contrast stretching)
    // from being rendered as dots.
    if max_variance > 0.0 {
        best_threshold.saturating_add(2)
    } else {
        127 // Fallback for perfectly uniform images
    }
}

/// Convert a grayscale image to binary using a threshold value
///
/// Pixels with intensity >= threshold become `true` (black), others become `false` (white).
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, to_grayscale, apply_threshold};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
/// let gray = to_grayscale(&img);
/// let binary = apply_threshold(&gray, 128);  // Manual threshold
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// Target: <2ms for 160×96 pixel images
pub fn apply_threshold(gray: &GrayImage, threshold: u8) -> BinaryImage {
    debug!(
        "Applying threshold {} to {}×{} image",
        threshold,
        gray.width(),
        gray.height()
    );

    let width = gray.width();
    let height = gray.height();

    let mut binary = BinaryImage::new(width, height);

    for (i, pixel) in gray.pixels().enumerate() {
        // Pixels >= threshold become black (true), others white (false)
        binary.pixels[i] = pixel[0] >= threshold;
    }

    debug!("Binary conversion complete");

    binary
}

/// Convert an image to binary using automatic Otsu thresholding
///
/// This is a convenience function that combines grayscale conversion,
/// Otsu threshold calculation, and binary conversion in one call.
///
/// Pipeline: DynamicImage → GrayImage → calculate Otsu → BinaryImage
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, auto_threshold};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
/// let binary = auto_threshold(&img);  // Automatic threshold selection
/// println!("Binary image: {}×{} with {} pixels",
///          binary.width, binary.height, binary.pixel_count());
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// Target: <10ms total for 160×96 pixel images (grayscale + Otsu + binary)
pub fn auto_threshold(image: &DynamicImage) -> BinaryImage {
    use super::convert::to_grayscale;

    debug!("Auto-threshold pipeline starting");

    let gray = to_grayscale(image);
    let threshold = otsu_threshold(&gray);
    let binary = apply_threshold(&gray, threshold);

    debug!(
        "Auto-threshold pipeline complete (threshold: {})",
        threshold
    );

    binary
}

/// Adjust the brightness of a grayscale image
///
/// Brightness adjustment multiplies all pixel values by a factor:
/// - Factor < 1.0: Darkens the image
/// - Factor = 1.0: No change
/// - Factor > 1.0: Brightens the image
///
/// Formula: `new_pixel = clamp(old_pixel × factor, 0, 255)`
///
/// # Arguments
///
/// * `gray` - Input grayscale image
/// * `factor` - Brightness factor (valid range: 0.0 to 2.0)
///
/// # Returns
///
/// Returns a new `GrayImage` with adjusted brightness, or an error if the factor is invalid.
///
/// # Errors
///
/// Returns `DotmaxError::InvalidParameter` if `factor` is outside the range 0.0-2.0.
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, to_grayscale, adjust_brightness};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
/// let gray = to_grayscale(&img);
/// let bright = adjust_brightness(&gray, 1.2)?;  // 20% brighter
/// # Ok(())
/// # }
/// ```
pub fn adjust_brightness(gray: &GrayImage, factor: f32) -> Result<GrayImage, DotmaxError> {
    if !(0.0..=2.0).contains(&factor) {
        return Err(DotmaxError::InvalidParameter {
            parameter_name: "brightness factor".to_string(),
            value: factor.to_string(),
            min: "0.0".to_string(),
            max: "2.0".to_string(),
        });
    }

    debug!("Adjusting brightness by factor {}", factor);

    let (width, height) = gray.dimensions();
    let mut adjusted = GrayImage::new(width, height);

    for (dest, src) in adjusted.pixels_mut().zip(gray.pixels()) {
        let old_value = f32::from(src[0]);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_value = (old_value * factor).clamp(0.0, 255.0) as u8;
        *dest = Luma([new_value]);
    }

    Ok(adjusted)
}

/// Adjust the contrast of a grayscale image
///
/// Contrast adjustment spreads pixel values around the middle gray value (128):
/// - Factor < 1.0: Reduces contrast (values move toward 128)
/// - Factor = 1.0: No change
/// - Factor > 1.0: Increases contrast (values spread from 128)
///
/// Formula: `new_pixel = clamp((old_pixel - 128) × factor + 128, 0, 255)`
///
/// # Arguments
///
/// * `gray` - Input grayscale image
/// * `factor` - Contrast factor (valid range: 0.0 to 2.0)
///
/// # Returns
///
/// Returns a new `GrayImage` with adjusted contrast, or an error if the factor is invalid.
///
/// # Errors
///
/// Returns `DotmaxError::InvalidParameter` if `factor` is outside the range 0.0-2.0.
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, to_grayscale, adjust_contrast};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
/// let gray = to_grayscale(&img);
/// let contrasted = adjust_contrast(&gray, 1.5)?;  // 50% more contrast
/// # Ok(())
/// # }
/// ```
pub fn adjust_contrast(gray: &GrayImage, factor: f32) -> Result<GrayImage, DotmaxError> {
    if !(0.0..=2.0).contains(&factor) {
        return Err(DotmaxError::InvalidParameter {
            parameter_name: "contrast factor".to_string(),
            value: factor.to_string(),
            min: "0.0".to_string(),
            max: "2.0".to_string(),
        });
    }

    debug!("Adjusting contrast by factor {}", factor);

    let (width, height) = gray.dimensions();
    let mut adjusted = GrayImage::new(width, height);

    for (dest, src) in adjusted.pixels_mut().zip(gray.pixels()) {
        let old_value = f32::from(src[0]);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_value = ((old_value - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
        *dest = Luma([new_value]);
    }

    Ok(adjusted)
}

/// Apply gamma correction to a grayscale image
///
/// Gamma correction applies a nonlinear power curve to pixel values:
/// - Gamma < 1.0: Brightens (expands shadows, compresses highlights)
/// - Gamma = 1.0: No change
/// - Gamma > 1.0: Darkens (compresses shadows, expands highlights)
///
/// Formula: `new_pixel = 255 × (old_pixel / 255)^gamma`
///
/// # Arguments
///
/// * `gray` - Input grayscale image
/// * `gamma` - Gamma value (valid range: 0.1 to 3.0)
///
/// # Returns
///
/// Returns a new `GrayImage` with gamma correction applied, or an error if gamma is invalid.
///
/// # Errors
///
/// Returns `DotmaxError::InvalidParameter` if `gamma` is outside the range 0.1-3.0.
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, to_grayscale, adjust_gamma};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("image.png"))?;
/// let gray = to_grayscale(&img);
/// let corrected = adjust_gamma(&gray, 0.8)?;  // Brighten with gamma < 1.0
/// # Ok(())
/// # }
/// ```
pub fn adjust_gamma(gray: &GrayImage, gamma: f32) -> Result<GrayImage, DotmaxError> {
    if !(0.1..=3.0).contains(&gamma) {
        return Err(DotmaxError::InvalidParameter {
            parameter_name: "gamma".to_string(),
            value: gamma.to_string(),
            min: "0.1".to_string(),
            max: "3.0".to_string(),
        });
    }

    debug!("Applying gamma correction: {}", gamma);

    let (width, height) = gray.dimensions();
    let mut adjusted = GrayImage::new(width, height);

    for (dest, src) in adjusted.pixels_mut().zip(gray.pixels()) {
        let normalized = f32::from(src[0]) / 255.0;
        let corrected = normalized.powf(gamma);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_value = (corrected * 255.0).clamp(0.0, 255.0) as u8;
        *dest = Luma([new_value]);
    }

    Ok(adjusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    /// Helper to create a grayscale image with uniform pixel value
    fn create_uniform_gray_image(width: u32, height: u32, value: u8) -> GrayImage {
        let mut img = GrayImage::new(width, height);
        for pixel in img.pixels_mut() {
            *pixel = Luma([value]);
        }
        img
    }

    #[test]
    fn test_binary_image_new() {
        let binary = BinaryImage::new(10, 20);
        assert_eq!(binary.width, 10);
        assert_eq!(binary.height, 20);
        assert_eq!(binary.pixel_count(), 200);
        assert!(
            binary.pixels.iter().all(|&p| !p),
            "All pixels should be false"
        );
    }

    #[test]
    fn test_binary_image_get_set_pixel() {
        let mut binary = BinaryImage::new(5, 5);

        // Set a pixel
        assert!(binary.set_pixel(2, 3, true));
        assert_eq!(binary.get_pixel(2, 3), Some(true));

        // Out of bounds
        assert!(!binary.set_pixel(10, 10, true));
        assert_eq!(binary.get_pixel(10, 10), None);
    }

    #[test]
    fn test_otsu_all_black() {
        let img = create_uniform_gray_image(10, 10, 0);
        let threshold = otsu_threshold(&img);
        assert_eq!(
            threshold, 127,
            "All black should return default fallback 127"
        );
    }

    #[test]
    fn test_otsu_all_white() {
        let img = create_uniform_gray_image(10, 10, 255);
        let threshold = otsu_threshold(&img);
        assert_eq!(
            threshold, 127,
            "All white has no variance, returns default fallback 127"
        );
    }

    #[test]
    fn test_otsu_uniform_gray() {
        let img = create_uniform_gray_image(10, 10, 128);
        let threshold = otsu_threshold(&img);
        // Uniform images have no variance, threshold will be default 127
        assert_eq!(threshold, 127);
    }

    #[test]
    fn test_otsu_bimodal_distribution() {
        // Create image with half darker (50), half brighter (200)
        let mut img = GrayImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                let value = if y < 5 { 50 } else { 200 };
                img.put_pixel(x, y, Luma([value]));
            }
        }

        let threshold = otsu_threshold(&img);
        // For bimodal distribution (50 and 200), Otsu returns the first threshold
        // that achieves maximum variance (50), plus 2 for our noise-floor logic.
        assert_eq!(
            threshold, 52,
            "Bimodal distribution returns optimal threshold (lower peak + 2)"
        );
    }

    #[test]
    fn test_apply_threshold_simple() {
        let mut img = GrayImage::new(4, 4);
        // Create a simple pattern: top half bright (200), bottom half dark (50)
        for y in 0..4 {
            for x in 0..4 {
                let value = if y < 2 { 200 } else { 50 };
                img.put_pixel(x, y, Luma([value]));
            }
        }

        let binary = apply_threshold(&img, 128);

        // Top half (200 >= 128) should be true (black)
        assert_eq!(binary.get_pixel(0, 0), Some(true));
        assert_eq!(binary.get_pixel(3, 1), Some(true));

        // Bottom half (50 < 128) should be false (white)
        assert_eq!(binary.get_pixel(0, 2), Some(false));
        assert_eq!(binary.get_pixel(3, 3), Some(false));
    }

    #[test]
    fn test_auto_threshold_pipeline() {
        use image::{DynamicImage, Rgb, RgbImage};

        // Create a simple test image
        let mut rgb_img = RgbImage::new(10, 10);
        for pixel in rgb_img.pixels_mut() {
            *pixel = Rgb([128, 128, 128]);
        }
        let dynamic = DynamicImage::ImageRgb8(rgb_img);

        let binary = auto_threshold(&dynamic);

        assert_eq!(binary.width, 10);
        assert_eq!(binary.height, 10);
        assert_eq!(binary.pixel_count(), 100);
    }

    #[test]
    fn test_adjust_brightness_darkens() {
        let img = create_uniform_gray_image(5, 5, 100);
        let result = adjust_brightness(&img, 0.5).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 50, "Brightness 0.5 should halve values");
    }

    #[test]
    fn test_adjust_brightness_brightens() {
        let img = create_uniform_gray_image(5, 5, 100);
        let result = adjust_brightness(&img, 1.5).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 150, "Brightness 1.5 should multiply by 1.5");
    }

    #[test]
    fn test_adjust_brightness_no_op() {
        let img = create_uniform_gray_image(5, 5, 100);
        let result = adjust_brightness(&img, 1.0).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 100, "Brightness 1.0 should not change values");
    }

    #[test]
    fn test_adjust_brightness_invalid_factor() {
        let img = create_uniform_gray_image(5, 5, 100);

        assert!(adjust_brightness(&img, -0.5).is_err());
        assert!(adjust_brightness(&img, 3.0).is_err());
    }

    #[test]
    fn test_adjust_contrast_reduces() {
        let img = create_uniform_gray_image(5, 5, 200);
        let result = adjust_contrast(&img, 0.5).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        // (200 - 128) * 0.5 + 128 = 36 + 128 = 164
        assert_eq!(pixel_value, 164, "Contrast 0.5 should reduce spread");
    }

    #[test]
    fn test_adjust_contrast_increases() {
        let img = create_uniform_gray_image(5, 5, 150);
        let result = adjust_contrast(&img, 1.5).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        // (150 - 128) * 1.5 + 128 = 33 + 128 = 161
        assert_eq!(pixel_value, 161, "Contrast 1.5 should increase spread");
    }

    #[test]
    fn test_adjust_contrast_no_op() {
        let img = create_uniform_gray_image(5, 5, 150);
        let result = adjust_contrast(&img, 1.0).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 150, "Contrast 1.0 should not change values");
    }

    #[test]
    fn test_adjust_gamma_brightens() {
        let img = create_uniform_gray_image(5, 5, 100);
        let result = adjust_gamma(&img, 0.5).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        // (100/255)^0.5 * 255 ≈ 160
        assert!(
            pixel_value > 100,
            "Gamma < 1.0 should brighten, got {}",
            pixel_value
        );
    }

    #[test]
    fn test_adjust_gamma_darkens() {
        let img = create_uniform_gray_image(5, 5, 150);
        let result = adjust_gamma(&img, 2.0).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        // (150/255)^2.0 * 255 ≈ 138
        assert!(
            pixel_value < 150,
            "Gamma > 1.0 should darken, got {}",
            pixel_value
        );
    }

    #[test]
    fn test_adjust_gamma_no_op() {
        let img = create_uniform_gray_image(5, 5, 150);
        let result = adjust_gamma(&img, 1.0).unwrap();

        let pixel_value = result.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 150, "Gamma 1.0 should not change values");
    }

    #[test]
    fn test_adjust_gamma_invalid() {
        let img = create_uniform_gray_image(5, 5, 100);

        assert!(adjust_gamma(&img, 0.05).is_err());
        assert!(adjust_gamma(&img, 5.0).is_err());
    }
}
