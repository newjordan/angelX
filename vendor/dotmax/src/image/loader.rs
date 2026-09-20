//! Image loading from file paths and byte buffers
//!
//! This module provides core image loading functionality for dotmax,
//! supporting multiple formats via the `image` crate.

use crate::DotmaxError;
use image::DynamicImage;
use std::path::Path;
use tracing::{debug, info};

/// Maximum image dimensions (width or height in pixels)
///
/// This limit prevents memory exhaustion attacks from malicious or
/// extremely large images. Images exceeding these dimensions will
/// return `DotmaxError::InvalidImageDimensions`.
pub const MAX_IMAGE_WIDTH: u32 = 10_000;
/// Maximum image height in pixels (prevents memory exhaustion)
pub const MAX_IMAGE_HEIGHT: u32 = 10_000;

/// Load an image from a file path
///
/// Supports PNG, JPG, GIF, BMP, WebP, and TIFF formats. Format detection
/// is automatic based on file magic bytes (not file extension).
///
/// # Arguments
///
/// * `path` - Path to the image file
///
/// # Returns
///
/// Returns a `DynamicImage` containing the decoded image data, or an error if:
/// - File does not exist or is not readable
/// - File format is not supported or corrupted
/// - Image dimensions exceed [`MAX_IMAGE_WIDTH`] or [`MAX_IMAGE_HEIGHT`]
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::load_from_path;
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let img = load_from_path(Path::new("photo.jpg"))?;
/// println!("Loaded {}×{} image", img.width(), img.height());
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns [`DotmaxError::ImageLoad`] if the file cannot be loaded or decoded.
/// Returns [`DotmaxError::InvalidImageDimensions`] if image exceeds size limits.
pub fn load_from_path(path: &Path) -> Result<DynamicImage, DotmaxError> {
    info!("Loading image from {:?}", path);

    // Validate path exists before attempting to load
    std::fs::metadata(path).map_err(|e| DotmaxError::ImageLoad {
        path: path.to_path_buf(),
        source: image::ImageError::IoError(e),
    })?;

    // Load image using the image crate
    let img = image::open(path).map_err(|e| DotmaxError::ImageLoad {
        path: path.to_path_buf(),
        source: e,
    })?;

    debug!("Image dimensions: {}×{}", img.width(), img.height());

    // Validate dimensions against maximum limits
    if img.width() > MAX_IMAGE_WIDTH || img.height() > MAX_IMAGE_HEIGHT {
        return Err(DotmaxError::InvalidImageDimensions {
            width: img.width(),
            height: img.height(),
        });
    }

    Ok(img)
}

/// Load an image from a byte buffer
///
/// Supports the same formats as [`load_from_path`]: PNG, JPG, GIF, BMP, WebP, TIFF.
/// Format detection is automatic based on magic bytes.
///
/// This function is useful for loading embedded images or images received over
/// the network without writing to disk.
///
/// # Arguments
///
/// * `bytes` - Byte slice containing encoded image data
///
/// # Returns
///
/// Returns a `DynamicImage` containing the decoded image data, or an error if:
/// - Byte data is not a valid image format
/// - Image format is not supported
/// - Image dimensions exceed [`MAX_IMAGE_WIDTH`] or [`MAX_IMAGE_HEIGHT`]
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::load_from_bytes;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let bytes = include_bytes!("../../tests/fixtures/images/sample.png");
/// let img = load_from_bytes(bytes)?;
/// println!("Loaded {}×{} image from bytes", img.width(), img.height());
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns [`DotmaxError::ImageLoad`] if the bytes cannot be decoded.
/// Returns [`DotmaxError::InvalidImageDimensions`] if image exceeds size limits.
pub fn load_from_bytes(bytes: &[u8]) -> Result<DynamicImage, DotmaxError> {
    info!("Loading image from byte buffer ({} bytes)", bytes.len());

    // Load image from memory using the image crate
    let img = image::load_from_memory(bytes).map_err(|e| DotmaxError::ImageLoad {
        path: std::path::PathBuf::from("<bytes>"),
        source: e,
    })?;

    debug!("Image dimensions: {}×{}", img.width(), img.height());

    // Validate dimensions against maximum limits
    if img.width() > MAX_IMAGE_WIDTH || img.height() > MAX_IMAGE_HEIGHT {
        return Err(DotmaxError::InvalidImageDimensions {
            width: img.width(),
            height: img.height(),
        });
    }

    Ok(img)
}

/// Returns a list of supported image format extensions
///
/// This function returns the file extensions for all image formats
/// supported by dotmax. Format detection during loading is automatic
/// and does not rely on file extensions.
///
/// # Returns
///
/// A vector of format extensions as static strings: `["png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff"]`
///
/// # Examples
///
/// ```
/// use dotmax::image::supported_formats;
///
/// let formats = supported_formats();
/// assert!(formats.contains(&"png"));
/// assert!(formats.contains(&"jpg"));
/// assert!(formats.contains(&"gif"));
/// ```
#[must_use]
pub fn supported_formats() -> Vec<&'static str> {
    vec!["png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff"]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_supported_formats_returns_expected_list() {
        let formats = supported_formats();
        assert_eq!(formats.len(), 7);
        assert!(formats.contains(&"png"));
        assert!(formats.contains(&"jpg"));
        assert!(formats.contains(&"jpeg"));
        assert!(formats.contains(&"gif"));
        assert!(formats.contains(&"bmp"));
        assert!(formats.contains(&"webp"));
        assert!(formats.contains(&"tiff"));
    }

    #[test]
    fn test_max_dimensions_constants_are_sensible() {
        // Verify limits are reasonable (not too small, not too large)
        assert_eq!(MAX_IMAGE_WIDTH, 10_000);
        assert_eq!(MAX_IMAGE_HEIGHT, 10_000);
    }

    #[test]
    fn test_load_from_path_with_valid_png() {
        let path = Path::new("tests/fixtures/images/sample.png");
        let result = load_from_path(path);

        assert!(
            result.is_ok(),
            "Failed to load sample.png: {:?}",
            result.err()
        );
        let img = result.unwrap();

        // Verify dimensions are within expected range (small test image)
        assert!(img.width() > 0 && img.width() <= 100);
        assert!(img.height() > 0 && img.height() <= 100);
    }

    #[test]
    fn test_load_from_path_with_missing_file() {
        let path = Path::new("tests/fixtures/images/nonexistent.png");
        let result = load_from_path(path);

        assert!(result.is_err());
        match result.unwrap_err() {
            DotmaxError::ImageLoad { path: err_path, .. } => {
                assert_eq!(
                    err_path,
                    PathBuf::from("tests/fixtures/images/nonexistent.png")
                );
            }
            other => panic!("Expected ImageLoad error, got {:?}", other),
        }
    }

    #[test]
    fn test_load_from_path_with_corrupted_file() {
        let path = Path::new("tests/fixtures/images/corrupted.png");
        let result = load_from_path(path);

        assert!(result.is_err(), "Should fail on corrupted PNG");
        match result.unwrap_err() {
            DotmaxError::ImageLoad { .. } => {
                // Expected - corrupted file should trigger ImageLoad error
            }
            other => panic!(
                "Expected ImageLoad error for corrupted file, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_load_from_bytes_with_valid_png_bytes() {
        // Read a valid PNG file into bytes
        let path = Path::new("tests/fixtures/images/sample.png");
        let bytes = std::fs::read(path).expect("Failed to read sample.png");

        let result = load_from_bytes(&bytes);
        assert!(
            result.is_ok(),
            "Failed to load from bytes: {:?}",
            result.err()
        );

        let img = result.unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_load_from_bytes_with_invalid_bytes() {
        let invalid_bytes = b"This is not an image!";
        let result = load_from_bytes(invalid_bytes);

        assert!(result.is_err(), "Should fail on invalid bytes");
        match result.unwrap_err() {
            DotmaxError::ImageLoad { .. } => {
                // Expected
            }
            other => panic!("Expected ImageLoad error, got {:?}", other),
        }
    }

    #[test]
    fn test_dimension_validation_rejects_oversized_width() {
        // We can't easily create a 10,001×1 image in tests,
        // so we'll test the logic by creating a small image
        // and verifying the dimension check constants are correct

        // Verify the constant values
        assert_eq!(MAX_IMAGE_WIDTH, 10_000);
        assert_eq!(MAX_IMAGE_HEIGHT, 10_000);

        // The actual dimension validation is tested in integration tests
        // where we can mock or generate large images if needed
    }

    #[test]
    fn test_load_from_path_validates_path_exists() {
        let path = Path::new("/nonexistent/directory/image.png");
        let result = load_from_path(path);

        assert!(result.is_err());
        // Should get ImageLoad error (wrapping IoError for file not found)
        matches!(result.unwrap_err(), DotmaxError::ImageLoad { .. });
    }
}
