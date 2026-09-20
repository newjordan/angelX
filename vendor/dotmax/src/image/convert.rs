//! Grayscale conversion module
//!
//! This module provides functionality to convert color images to grayscale using
//! the standard ITU-R BT.709 luminance formula: Y = 0.2126*R + 0.7152*G + 0.0722*B.
//!
//! # Examples
//!
//! ```no_run
//! use dotmax::image::{load_from_path, to_grayscale};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("color_image.png"))?;
//! let gray = to_grayscale(&img);
//! println!("Converted to grayscale: {}×{}", gray.width(), gray.height());
//! # Ok(())
//! # }
//! ```
//!
//! # Performance
//!
//! Grayscale conversion targets <2ms for terminal-sized images (160×96 pixels).
//! The operation is a simple pixel-wise luminance calculation.

use image::{DynamicImage, GrayImage};
use tracing::debug;

/// Convert a color or grayscale image to 8-bit grayscale
///
/// This function converts any `DynamicImage` to a `GrayImage` (8-bit grayscale)
/// using the standard ITU-R BT.709 luminance conversion formula:
///
/// Y = 0.2126*R + 0.7152*G + 0.0722*B
///
/// If the input image is already grayscale, it is converted to the standard
/// `GrayImage` format for consistency.
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::{load_from_path, to_grayscale};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Load a color image
/// let color_img = load_from_path(Path::new("photo.jpg"))?;
///
/// // Convert to grayscale
/// let gray_img = to_grayscale(&color_img);
///
/// println!("Grayscale dimensions: {}×{}", gray_img.width(), gray_img.height());
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// - Target: <2ms for 160×96 pixel images (80×24 terminal)
/// - Operation: O(width × height) pixel-wise transformation
/// - Memory: Allocates new buffer of size width × height × 1 byte
///
/// # Technical Details
///
/// The `to_luma8()` method from the `image` crate implements the ITU-R BT.709
/// luminance conversion formula. This method is optimized and widely tested.
pub fn to_grayscale(image: &DynamicImage) -> GrayImage {
    debug!(
        "Converting {}×{} image to grayscale",
        image.width(),
        image.height()
    );

    // Use the standard luminance conversion from the image crate
    // This implements: Y = 0.299*R + 0.587*G + 0.114*B
    let gray = image.to_luma8();

    debug!("Grayscale conversion complete");

    gray
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage};

    /// Helper function to create a test RGB image with known pixel values
    fn create_test_rgb_image(width: u32, height: u32, color: Rgb<u8>) -> DynamicImage {
        let mut img = RgbImage::new(width, height);
        for pixel in img.pixels_mut() {
            *pixel = color;
        }
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_to_grayscale_pure_red() {
        // Pure red (255, 0, 0) - actual luminance value from image crate
        let img = create_test_rgb_image(10, 10, Rgb([255, 0, 0]));
        let gray = to_grayscale(&img);

        assert_eq!(gray.width(), 10);
        assert_eq!(gray.height(), 10);

        // Check first pixel value
        let pixel_value = gray.get_pixel(0, 0)[0];
        // Actual value from image crate (uses BT.709: 0.2126*R + 0.7152*G + 0.0722*B)
        // For red: 0.2126 * 255 ≈ 54.2
        assert!(
            (54..=55).contains(&pixel_value),
            "Expected ~54, got {}",
            pixel_value
        );
    }

    #[test]
    fn test_to_grayscale_pure_green() {
        // Pure green (0, 255, 0) - actual luminance value from image crate
        let img = create_test_rgb_image(10, 10, Rgb([0, 255, 0]));
        let gray = to_grayscale(&img);

        let pixel_value = gray.get_pixel(0, 0)[0];
        // Actual value from image crate (BT.709: 0.7152 * 255 ≈ 182.4)
        assert!(
            (182..=183).contains(&pixel_value),
            "Expected ~182, got {}",
            pixel_value
        );
    }

    #[test]
    fn test_to_grayscale_pure_blue() {
        // Pure blue (0, 0, 255) - actual luminance value from image crate
        let img = create_test_rgb_image(10, 10, Rgb([0, 0, 255]));
        let gray = to_grayscale(&img);

        let pixel_value = gray.get_pixel(0, 0)[0];
        // Actual value from image crate (BT.709: 0.0722 * 255 ≈ 18.4)
        assert!(
            (18..=19).contains(&pixel_value),
            "Expected ~18, got {}",
            pixel_value
        );
    }

    #[test]
    fn test_to_grayscale_white() {
        // White (255, 255, 255) should convert to 255
        let img = create_test_rgb_image(10, 10, Rgb([255, 255, 255]));
        let gray = to_grayscale(&img);

        let pixel_value = gray.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 255, "White should convert to 255");
    }

    #[test]
    fn test_to_grayscale_black() {
        // Black (0, 0, 0) should convert to 0
        let img = create_test_rgb_image(10, 10, Rgb([0, 0, 0]));
        let gray = to_grayscale(&img);

        let pixel_value = gray.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 0, "Black should convert to 0");
    }

    #[test]
    fn test_to_grayscale_already_grayscale() {
        // Create a grayscale image and ensure it passes through correctly
        use image::{GrayImage, Luma};

        let mut gray_input = GrayImage::new(10, 10);
        for pixel in gray_input.pixels_mut() {
            *pixel = Luma([128]);
        }

        let img = DynamicImage::ImageLuma8(gray_input);
        let gray_output = to_grayscale(&img);

        let pixel_value = gray_output.get_pixel(0, 0)[0];
        assert_eq!(pixel_value, 128, "Grayscale image should pass through");
    }

    #[test]
    fn test_to_grayscale_dimensions_preserved() {
        let img = create_test_rgb_image(100, 50, Rgb([128, 128, 128]));
        let gray = to_grayscale(&img);

        assert_eq!(gray.width(), 100, "Width should be preserved");
        assert_eq!(gray.height(), 50, "Height should be preserved");
    }

    #[test]
    fn test_to_grayscale_mixed_colors() {
        // Test with a mixed color to verify formula correctness
        // RGB(100, 150, 200) -> Y = 0.2126*100 + 0.7152*150 + 0.0722*200
        //                      -> Y = 21.26 + 107.28 + 14.44 = 142.98 ≈ 143
        let img = create_test_rgb_image(5, 5, Rgb([100, 150, 200]));
        let gray = to_grayscale(&img);

        let pixel_value = gray.get_pixel(0, 0)[0];
        assert!(
            (142..=144).contains(&pixel_value),
            "Expected ~143, got {}",
            pixel_value
        );
    }
}
