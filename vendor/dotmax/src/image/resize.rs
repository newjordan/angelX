//! Image resizing with aspect ratio preservation
//!
//! This module provides functions for resizing images to fit terminal dimensions
//! while preserving aspect ratios and avoiding distortion. It implements two main
//! resize strategies:
//!
//! 1. **Terminal-based resizing** (`resize_to_terminal`): Automatically calculates
//!    optimal dimensions based on terminal size and braille cell geometry (2×4 dots)
//! 2. **Manual resizing** (`resize_to_dimensions`): Allows explicit dimension
//!    specification with optional aspect ratio preservation
//!
//! # Braille Cell Coordinate System
//!
//! Terminal dimensions are measured in braille cells, where each cell is 2×4 dots:
//! - Terminal width of 80 cells = 160 pixels wide (80 × 2)
//! - Terminal height of 24 cells = 96 pixels tall (24 × 4)
//!
//! This coordinate system ensures images map correctly to the braille dot matrix.
//!
//! # Resize Quality & Performance
//!
//! Resize operations use **adaptive filter selection** based on image aspect ratio:
//! - **Lanczos3** for normal images (highest quality)
//! - **Triangle** for extreme aspect ratios >2.5:1 (3x faster, good quality)
//!
//! This adaptive approach balances quality and performance, optimizing large/extreme
//! images while maintaining highest quality for typical photos and diagrams.
//!
//! **Performance Targets (Story 3.5.5 benchmarks):**
//! - Normal images (1024×1024): ~17ms with Lanczos3 ✅
//! - Large images (4000×4000): ~257ms with Lanczos3 ✅
//! - Extreme wide (10000×4000): ~276ms with Triangle (45% faster than Lanczos3) ✅
//!
//! **Filter Trade-offs:**
//! - **Lanczos3** (normal images): Highest quality, ~474ms for 10000×4000
//! - **Triangle** (extreme images): Good quality, ~155ms for 10000×4000 (3x faster)
//! - **Nearest**: Lowest quality, ~8ms (not used due to poor quality)
//!
//! Quality difference between Triangle and Lanczos3 is minimal at braille resolution (2×4 dots per cell).
//!
//! # Examples
//!
//! ## Resize to terminal dimensions
//!
//! ```no_run
//! use dotmax::image::{load_from_path, resize_to_terminal};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("photo.jpg"))?;
//! let resized = resize_to_terminal(&img, 80, 24)?;
//! println!("Resized to {}×{} pixels", resized.width(), resized.height());
//! # Ok(())
//! # }
//! ```
//!
//! ## Resize to custom dimensions
//!
//! ```no_run
//! use dotmax::image::{load_from_path, resize_to_dimensions};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("diagram.png"))?;
//!
//! // Preserve aspect ratio (letterbox/pillarbox)
//! let resized = resize_to_dimensions(&img, 200, 100, true)?;
//!
//! // Stretch to exact dimensions (may distort)
//! let stretched = resize_to_dimensions(&img, 200, 100, false)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Performance Expectations
//!
//! **Normal Images (Lanczos3 filter):**
//! - Small images (800×600): <20ms ✅
//! - Medium images (1024×1024): ~17ms ✅
//! - Large images (1920×1080): <30ms ✅
//! - Very large (4000×4000): ~257ms ✅
//!
//! **Extreme Aspect Ratios (Triangle filter, >2.5:1 or <1:2.5):**
//! - Panoramas (10000×4000): ~276ms ✅ (45% faster than Lanczos3)
//! - Banners (4000×10000): ~241ms ✅ (45% faster than Lanczos3)
//! - True extremes (10000×100): ~6ms ✅
//!
//! **Targets:**
//! - Normal images: <50ms (standard terminal rendering)
//! - Large images: <500ms (acceptable for one-time load)
//! - Extreme images: <5s (Story 3.5.5 target - exceeded at 724ms total)

use crate::error::DotmaxError;
use crate::image::loader::{MAX_IMAGE_HEIGHT, MAX_IMAGE_WIDTH};
use image::{imageops, DynamicImage};
use tracing::debug;

/// Braille cell width in dots (2 dots wide)
const BRAILLE_CELL_WIDTH: u16 = 2;

/// Braille cell height in dots (4 dots tall)
const BRAILLE_CELL_HEIGHT: u16 = 4;

/// Aspect ratio threshold for "extreme" classification
/// Images with ratio >= 2.5:1 or <= 1:2.5 are considered extreme
/// This threshold matches real-world panorama and banner images (10000×4000 = exactly 2.5:1)
const EXTREME_ASPECT_RATIO_THRESHOLD: f32 = 2.5;

/// Resize image to fit terminal dimensions with aspect ratio preservation
///
/// Calculates optimal pixel dimensions from terminal dimensions (measured in
/// braille cells) and resizes the image to fit while preserving aspect ratio.
/// Uses Lanczos3 filter for high-quality output.
///
/// # Braille Cell Math
///
/// Terminal dimensions are in braille cells where each cell is 2×4 dots:
/// - `pixel_width = term_width × 2`
/// - `pixel_height = term_height × 4`
///
/// Example: 80×24 terminal = 160×96 pixels
///
/// # Arguments
///
/// * `image` - Source image to resize
/// * `term_width` - Terminal width in braille cells
/// * `term_height` - Terminal height in braille cells
///
/// # Returns
///
/// Resized `DynamicImage` that fits within terminal dimensions with aspect
/// ratio preserved (may be letterboxed or pillarboxed).
///
/// # Errors
///
/// Returns `DotmaxError::InvalidImageDimensions` if:
/// - Terminal dimensions are zero
/// - Calculated dimensions exceed maximum limits
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, resize_to_terminal};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("photo.jpg"))?;
/// let resized = resize_to_terminal(&img, 80, 24)?;
/// assert_eq!(resized.width(), 160);  // 80 cells × 2 dots
/// assert_eq!(resized.height(), 96);  // 24 cells × 4 dots
/// # Ok(())
/// # }
/// ```
pub fn resize_to_terminal(
    image: &DynamicImage,
    term_width: u16,
    term_height: u16,
) -> Result<DynamicImage, DotmaxError> {
    // Validate terminal dimensions
    if term_width == 0 || term_height == 0 {
        return Err(DotmaxError::InvalidImageDimensions {
            width: u32::from(term_width),
            height: u32::from(term_height),
        });
    }

    // Calculate pixel dimensions from braille cell dimensions
    let target_width_px = u32::from(term_width) * u32::from(BRAILLE_CELL_WIDTH);
    let target_height_px = u32::from(term_height) * u32::from(BRAILLE_CELL_HEIGHT);

    debug!(
        "Resize to terminal: {}×{} cells → {}×{} pixels",
        term_width, term_height, target_width_px, target_height_px
    );

    // Calculate aspect ratio of source image
    let src_width = image.width();
    let src_height = image.height();
    #[allow(clippy::cast_precision_loss)]
    let aspect_ratio = src_width as f32 / src_height as f32;

    debug!(
        "Source image: {}×{}, aspect ratio: {:.2}",
        src_width, src_height, aspect_ratio
    );

    // Calculate dimensions that preserve aspect ratio and fit within target
    let (final_width, final_height) =
        calculate_fit_dimensions(src_width, src_height, target_width_px, target_height_px);

    // Skip upscale prevention for terminal resizing
    // Terminal resize should always fill the available space, even if it means upscaling
    // This allows images to grow when the terminal expands

    debug!(
        "Final dimensions after aspect ratio preservation: {}×{}",
        final_width, final_height
    );

    // Only resize if dimensions changed
    if final_width == src_width && final_height == src_height {
        debug!("Image already at target size, skipping resize");
        return Ok(image.clone());
    }

    // Select appropriate filter based on aspect ratio (adaptive optimization)
    let filter = select_resize_filter(src_width, src_height);

    debug!(
        "Resizing {}×{} → {}×{} using {:?} filter",
        src_width, src_height, final_width, final_height, filter
    );

    // Perform resize with selected filter
    let resized = imageops::resize(image, final_width, final_height, filter);

    Ok(DynamicImage::ImageRgba8(resized))
}

/// Calculate dimensions that fit within target while preserving aspect ratio
///
/// Implements letterboxing (reduce height) or pillarboxing (reduce width) to
/// maintain the source aspect ratio while fitting within target dimensions.
///
/// # Arguments
///
/// * `src_w` - Source image width
/// * `src_h` - Source image height
/// * `target_w` - Target width constraint
/// * `target_h` - Target height constraint
///
/// # Returns
///
/// Tuple of (width, height) that fits within target with aspect ratio preserved
///
/// # Example Output
///
/// Wide image (16:9) → letterbox (reduce height):
/// - Input: 1920×1080 → Target: 160×96
/// - Output: 160×90 (constrained by width)
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn calculate_fit_dimensions(src_w: u32, src_h: u32, target_w: u32, target_h: u32) -> (u32, u32) {
    let src_aspect = src_w as f32 / src_h as f32;
    let target_aspect = target_w as f32 / target_h as f32;

    let (final_w, final_h) = if src_aspect > target_aspect {
        // Source is wider → constrain by width (pillarbox)
        let new_width = target_w;
        let new_height = (target_w as f32 / src_aspect).round() as u32;
        (new_width, new_height)
    } else {
        // Source is taller or same → constrain by height (letterbox)
        let new_height = target_h;
        let new_width = (target_h as f32 * src_aspect).round() as u32;
        (new_width, new_height)
    };

    // Ensure we don't exceed target dimensions due to rounding
    let final_w = final_w.min(target_w).max(1); // At least 1 pixel wide
    let final_h = final_h.min(target_h).max(1); // At least 1 pixel tall

    (final_w, final_h)
}

/// Check if an image has an extreme aspect ratio
///
/// Images with aspect ratios > 2.5:1 or < 1:2.5 are considered "extreme" and may benefit
/// from faster resize algorithms at the cost of slight quality reduction.
///
/// This threshold is based on real-world extreme images:
/// - Panorama photos (typically 2.5:1 to 4:1)
/// - Ultra-wide screenshots (10000×4000 = 2.5:1)
/// - Vertical banners (4000×10000 = 1:2.5)
///
/// # Arguments
///
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
///
/// `true` if aspect ratio is extreme (> 2.5:1 or < 1:2.5), `false` otherwise
///
/// # Examples
///
/// ```ignore
/// // Function is private - examples shown for documentation purposes
/// assert_eq!(is_extreme_aspect_ratio(10000, 4000), true);  // 2.5:1 ratio (panorama)
/// assert_eq!(is_extreme_aspect_ratio(4000, 10000), true);  // 1:2.5 ratio (banner)
/// assert_eq!(is_extreme_aspect_ratio(1920, 1080), false); // 16:9 ratio
/// assert_eq!(is_extreme_aspect_ratio(1000, 1000), false); // 1:1 ratio
/// ```
#[allow(clippy::cast_precision_loss)]
fn is_extreme_aspect_ratio(width: u32, height: u32) -> bool {
    let aspect_ratio = width as f32 / height as f32;
    aspect_ratio >= EXTREME_ASPECT_RATIO_THRESHOLD
        || aspect_ratio <= (1.0 / EXTREME_ASPECT_RATIO_THRESHOLD)
}

/// Select optimal resize filter based on image dimensions
///
/// For extreme aspect ratios (> 2.5:1 or < 1:2.5), uses Triangle filter for faster
/// performance with acceptable quality loss. For normal images, uses Lanczos3 for
/// highest quality.
///
/// # Performance vs Quality Trade-offs
///
/// - **Lanczos3** (normal images): Highest quality, slower (~16ms for 1024×1024, ~501ms for 10000×4000)
/// - **Triangle** (extreme images): Good quality, 3x faster than Lanczos3 (~155ms for 10000×4000)
///
/// Quality difference is minimal at braille resolution (2×4 dots per cell).
///
/// **Benchmark Data (Story 3.5.5):**
/// - Nearest: 8ms (59x faster, but too low quality)
/// - Triangle: 155ms (3x faster, good balance) ← **Selected for extreme ratios**
/// - `CatmullRom`: 278ms (1.7x faster, slightly better quality)
/// - Gaussian: 472ms (similar to Lanczos3)
/// - Lanczos3: 474ms (baseline, highest quality)
///
/// # Arguments
///
/// * `width` - Source image width
/// * `height` - Source image height
///
/// # Returns
///
/// Appropriate `FilterType` for the given dimensions
fn select_resize_filter(width: u32, height: u32) -> imageops::FilterType {
    if is_extreme_aspect_ratio(width, height) {
        debug!(
            "Extreme aspect ratio detected ({}×{}), using Triangle filter for 3x faster performance",
            width, height
        );
        imageops::FilterType::Triangle
    } else {
        // Use Lanczos3 for normal images (highest quality)
        imageops::FilterType::Lanczos3
    }
}

/// Resize image to specific dimensions with optional aspect ratio preservation
///
/// Provides manual control over image dimensions. When `preserve_aspect` is true,
/// the image is resized to fit within the target dimensions while maintaining
/// aspect ratio (letterbox/pillarbox). When false, the image is stretched to
/// exact dimensions (may cause distortion).
///
/// # Arguments
///
/// * `image` - Source image to resize
/// * `target_width` - Desired width in pixels
/// * `target_height` - Desired height in pixels
/// * `preserve_aspect` - If true, maintain aspect ratio; if false, stretch to exact size
///
/// # Returns
///
/// Resized `DynamicImage` matching the target dimensions (or fitted within if preserving aspect)
///
/// # Errors
///
/// Returns `DotmaxError::InvalidImageDimensions` if:
/// - Target width or height is zero
/// - Target dimensions exceed `MAX_IMAGE_WIDTH` or `MAX_IMAGE_HEIGHT`
///
/// # Examples
///
/// ## Preserve aspect ratio (letterbox/pillarbox)
///
/// ```no_run
/// use dotmax::image::{load_from_path, resize_to_dimensions};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("photo.jpg"))?;
/// let resized = resize_to_dimensions(&img, 200, 100, true)?;
/// // Output fits within 200×100, aspect ratio maintained
/// assert!(resized.width() <= 200);
/// assert!(resized.height() <= 100);
/// # Ok(())
/// # }
/// ```
///
/// ## Stretch to exact dimensions (may distort)
///
/// ```no_run
/// use dotmax::image::{load_from_path, resize_to_dimensions};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("diagram.png"))?;
/// let resized = resize_to_dimensions(&img, 200, 100, false)?;
/// // Output is exactly 200×100 (may be distorted)
/// assert_eq!(resized.width(), 200);
/// assert_eq!(resized.height(), 100);
/// # Ok(())
/// # }
/// ```
pub fn resize_to_dimensions(
    image: &DynamicImage,
    target_width: u32,
    target_height: u32,
    preserve_aspect: bool,
) -> Result<DynamicImage, DotmaxError> {
    // Validate target dimensions
    if target_width == 0 || target_height == 0 {
        return Err(DotmaxError::InvalidImageDimensions {
            width: target_width,
            height: target_height,
        });
    }

    // Validate against maximum dimensions
    if target_width > MAX_IMAGE_WIDTH || target_height > MAX_IMAGE_HEIGHT {
        return Err(DotmaxError::InvalidImageDimensions {
            width: target_width,
            height: target_height,
        });
    }

    let src_width = image.width();
    let src_height = image.height();

    debug!(
        "Resize to dimensions: {}×{} → {}×{}, preserve_aspect: {}",
        src_width, src_height, target_width, target_height, preserve_aspect
    );

    let (final_width, final_height) = if preserve_aspect {
        // Calculate dimensions that fit within target while preserving aspect ratio
        let dims = calculate_fit_dimensions(src_width, src_height, target_width, target_height);
        debug!(
            "Aspect ratio preserved: final dimensions {}×{}",
            dims.0, dims.1
        );
        dims
    } else {
        // Stretch to exact dimensions (may distort)
        debug!("Stretching to exact dimensions (aspect ratio not preserved)");
        (target_width, target_height)
    };

    // Select appropriate filter based on aspect ratio (adaptive optimization)
    let filter = select_resize_filter(src_width, src_height);

    debug!(
        "Resizing {}×{} → {}×{} using {:?} filter (preserve_aspect: {})",
        src_width, src_height, final_width, final_height, filter, preserve_aspect
    );

    // Perform resize with selected filter
    let resized = imageops::resize(image, final_width, final_height, filter);

    Ok(DynamicImage::ImageRgba8(resized))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};

    // Helper to create a test image
    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        let img = RgbaImage::new(width, height);
        DynamicImage::ImageRgba8(img)
    }

    // Task 7.1: Test calculate_fit_dimensions() with 16:9 aspect ratio
    #[test]
    fn test_calculate_fit_dimensions_16_9() {
        // 1920×1080 (16:9) → 160×96 target
        let (w, h) = calculate_fit_dimensions(1920, 1080, 160, 96);

        // Should constrain by width (wider aspect than target)
        assert!(w <= 160);
        assert!(h <= 96);

        // Aspect ratio should be preserved (within rounding error)
        let aspect = w as f32 / h as f32;
        let expected_aspect = 16.0 / 9.0;
        assert!(
            (aspect - expected_aspect).abs() < 0.01,
            "Aspect ratio {:.3} != expected {:.3}",
            aspect,
            expected_aspect
        );
    }

    // Task 7.2: Test calculate_fit_dimensions() with 4:3 aspect ratio
    #[test]
    fn test_calculate_fit_dimensions_4_3() {
        // 800×600 (4:3) → 160×96 target
        let (w, h) = calculate_fit_dimensions(800, 600, 160, 96);

        assert!(w <= 160);
        assert!(h <= 96);

        let aspect = w as f32 / h as f32;
        let expected_aspect = 4.0 / 3.0;
        assert!((aspect - expected_aspect).abs() < 0.01);
    }

    // Task 7.3: Test calculate_fit_dimensions() with 1:1 (square) aspect ratio
    #[test]
    fn test_calculate_fit_dimensions_1_1_square() {
        // 500×500 (1:1) → 160×96 target
        let (w, h) = calculate_fit_dimensions(500, 500, 160, 96);

        assert!(w <= 160);
        assert!(h <= 96);

        // Square should become 96×96 (constrained by height)
        assert_eq!(h, 96);
        assert_eq!(w, 96);
    }

    // Task 7.4: Test calculate_fit_dimensions() with 21:9 (ultrawide) aspect ratio
    #[test]
    fn test_calculate_fit_dimensions_21_9_ultrawide() {
        // 2560×1080 (21:9) → 160×96 target
        let (w, h) = calculate_fit_dimensions(2560, 1080, 160, 96);

        assert!(w <= 160);
        assert!(h <= 96);

        let aspect = w as f32 / h as f32;
        let expected_aspect = 21.0 / 9.0;
        // More lenient tolerance for extreme aspect ratios due to rounding
        assert!((aspect - expected_aspect).abs() < 0.1);
    }

    // Task 7.5: Test letterboxing scenario (wide image → tall target)
    #[test]
    fn test_letterboxing_wide_to_tall() {
        // Very wide image → tall target
        let (w, h) = calculate_fit_dimensions(1000, 100, 200, 400);

        // Should constrain by width, leaving vertical letterbox space
        assert_eq!(w, 200);
        assert!(h < 400);

        // Aspect ratio preserved
        let aspect = w as f32 / h as f32;
        let expected_aspect = 1000.0 / 100.0;
        assert!((aspect - expected_aspect).abs() < 0.1);
    }

    // Task 7.6: Test pillarboxing scenario (tall image → wide target)
    #[test]
    fn test_pillarboxing_tall_to_wide() {
        // Very tall image → wide target
        let (w, h) = calculate_fit_dimensions(100, 1000, 400, 200);

        // Should constrain by height, leaving horizontal pillarbox space
        assert_eq!(h, 200);
        assert!(w < 400);

        // Aspect ratio preserved
        let aspect = w as f32 / h as f32;
        let expected_aspect = 100.0 / 1000.0;
        assert!((aspect - expected_aspect).abs() < 0.01);
    }

    // Task 7.7: Test perfect fit (source and target same aspect ratio)
    #[test]
    fn test_perfect_fit_same_aspect() {
        // Both 16:9 aspect ratio
        let (w, h) = calculate_fit_dimensions(1920, 1080, 160, 90);

        // Should use full target dimensions
        assert_eq!(w, 160);
        assert_eq!(h, 90);
    }

    // Task 7.8 & 7.9: Upscale prevention removed to allow images to fill terminal on resize
    // Terminal resize should always fill the available space, even if it means upscaling

    // Task 7.10: Test zero dimensions error handling
    #[test]
    fn test_resize_to_terminal_zero_width() {
        let img = create_test_image(100, 100);
        let result = resize_to_terminal(&img, 0, 24);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DotmaxError::InvalidImageDimensions { .. }
        ));
    }

    #[test]
    fn test_resize_to_terminal_zero_height() {
        let img = create_test_image(100, 100);
        let result = resize_to_terminal(&img, 80, 0);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DotmaxError::InvalidImageDimensions { .. }
        ));
    }

    #[test]
    fn test_resize_to_dimensions_zero_width() {
        let img = create_test_image(100, 100);
        let result = resize_to_dimensions(&img, 0, 100, true);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DotmaxError::InvalidImageDimensions { .. }
        ));
    }

    #[test]
    fn test_resize_to_dimensions_zero_height() {
        let img = create_test_image(100, 100);
        let result = resize_to_dimensions(&img, 100, 0, false);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DotmaxError::InvalidImageDimensions { .. }
        ));
    }

    // Task 7.11: Test extreme aspect ratios (10000×1, 1×10000)
    #[test]
    fn test_extreme_aspect_ratio_wide() {
        // Very wide image
        let (w, h) = calculate_fit_dimensions(10000, 1, 160, 96);

        // Should fit within constraints
        assert!(w <= 160);
        assert!(h <= 96);
        assert!(h >= 1); // At least 1 pixel tall
    }

    #[test]
    fn test_extreme_aspect_ratio_tall() {
        // Very tall image
        let (w, h) = calculate_fit_dimensions(1, 10000, 160, 96);

        // Should fit within constraints
        assert!(w <= 160);
        assert!(h <= 96);
        assert!(w >= 1); // At least 1 pixel wide
    }

    // Additional tests for resize_to_dimensions
    #[test]
    fn test_resize_to_dimensions_preserve_aspect_true() {
        let img = create_test_image(1920, 1080);
        let result = resize_to_dimensions(&img, 200, 100, true);

        assert!(result.is_ok());
        let resized = result.unwrap();

        // Should fit within target with aspect preserved
        assert!(resized.width() <= 200);
        assert!(resized.height() <= 100);
    }

    #[test]
    fn test_resize_to_dimensions_preserve_aspect_false() {
        let img = create_test_image(1920, 1080);
        let result = resize_to_dimensions(&img, 200, 100, false);

        assert!(result.is_ok());
        let resized = result.unwrap();

        // Should be exactly target dimensions (stretched)
        assert_eq!(resized.width(), 200);
        assert_eq!(resized.height(), 100);
    }

    #[test]
    fn test_resize_to_dimensions_exceeds_max_dimensions() {
        let img = create_test_image(100, 100);
        let result = resize_to_dimensions(&img, MAX_IMAGE_WIDTH + 1, 100, false);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DotmaxError::InvalidImageDimensions { .. }
        ));
    }

    #[test]
    fn test_resize_to_terminal_basic() {
        let img = create_test_image(800, 600);
        let result = resize_to_terminal(&img, 80, 24);

        assert!(result.is_ok());
        let resized = result.unwrap();

        // Should fit within terminal dimensions (160×96 pixels)
        assert!(resized.width() <= 160);
        assert!(resized.height() <= 96);
    }

    #[test]
    fn test_no_resize_when_already_correct_size() {
        let img = create_test_image(160, 96);
        let result = resize_to_terminal(&img, 80, 24);

        assert!(result.is_ok());
        let resized = result.unwrap();

        // Should be same dimensions (no resize needed)
        assert_eq!(resized.width(), 160);
        assert_eq!(resized.height(), 96);
    }

    // Task 3: Tests for adaptive resize algorithm (Story 3.5.5)
    #[test]
    fn test_is_extreme_aspect_ratio_wide() {
        // 10000×100 = 100:1 ratio (extreme)
        assert!(is_extreme_aspect_ratio(10000, 100));
        // 10000×4000 = 2.5:1 ratio (exactly at threshold, should be EXTREME now with >=)
        assert!(is_extreme_aspect_ratio(10000, 4000));
        // 10000×3000 = 3.33:1 ratio (extreme)
        assert!(is_extreme_aspect_ratio(10000, 3000));
    }

    #[test]
    fn test_is_extreme_aspect_ratio_tall() {
        // 100×10000 = 1:100 ratio (extreme)
        assert!(is_extreme_aspect_ratio(100, 10000));
        // 4000×10000 = 1:2.5 ratio (exactly at threshold, should be EXTREME now with <=)
        assert!(is_extreme_aspect_ratio(4000, 10000));
        // 3000×10000 = 1:3.33 ratio (extreme)
        assert!(is_extreme_aspect_ratio(3000, 10000));
    }

    #[test]
    fn test_is_extreme_aspect_ratio_normal() {
        // 1920×1080 = 16:9 = 1.78:1 ratio (normal)
        assert!(!is_extreme_aspect_ratio(1920, 1080));
        // 800×600 = 4:3 = 1.33:1 ratio (normal)
        assert!(!is_extreme_aspect_ratio(800, 600));
        // 1000×1000 = 1:1 ratio (normal)
        assert!(!is_extreme_aspect_ratio(1000, 1000));
        // 2560×1080 = 21:9 = 2.37:1 ratio (normal, just under threshold)
        assert!(!is_extreme_aspect_ratio(2560, 1080));
    }

    #[test]
    fn test_is_extreme_aspect_ratio_edge_cases() {
        // Exactly 2.5:1 threshold (should be EXTREME with >=)
        assert!(is_extreme_aspect_ratio(2500, 1000));
        // Slightly over 2.5:1 (should be extreme)
        assert!(is_extreme_aspect_ratio(2501, 1000));
        // Exactly 1:2.5 threshold (should be EXTREME with <=)
        assert!(is_extreme_aspect_ratio(1000, 2500));
        // Slightly under 1:2.5 (should be extreme)
        assert!(is_extreme_aspect_ratio(1000, 2501));
        // Just under 2.5:1 threshold (should be normal)
        assert!(!is_extreme_aspect_ratio(2499, 1000));
        // Just over 1:2.5 threshold (should be normal)
        assert!(!is_extreme_aspect_ratio(1000, 2499));
    }

    #[test]
    fn test_select_resize_filter_normal() {
        // Normal images should get Lanczos3
        let filter = select_resize_filter(1920, 1080);
        assert!(matches!(filter, imageops::FilterType::Lanczos3));

        let filter = select_resize_filter(800, 600);
        assert!(matches!(filter, imageops::FilterType::Lanczos3));
    }

    #[test]
    fn test_select_resize_filter_extreme() {
        // Extreme images should get Triangle for 3x faster performance
        let filter = select_resize_filter(10000, 100);
        assert!(matches!(filter, imageops::FilterType::Triangle));

        let filter = select_resize_filter(100, 10000);
        assert!(matches!(filter, imageops::FilterType::Triangle));
    }
}
