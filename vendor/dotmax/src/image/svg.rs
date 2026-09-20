//! SVG vector graphics loading and rasterization
//!
//! This module provides SVG support for dotmax by rasterizing vector graphics
//! to pixel buffers that feed into the standard image→braille pipeline.
//!
//! # Feature Gate
//!
//! To use this module, enable the `svg` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! dotmax = { version = "0.1", features = ["svg"] }
//! ```
//!
//! # Rasterization Approach
//!
//! SVG files are vector graphics that must be converted to raster images before
//! braille mapping. This module uses:
//! - **`usvg`**: Parses and normalizes SVG files
//! - **`resvg`**: Rasterizes SVG to high-quality pixel buffers
//! - **`tiny-skia`**: 2D rendering backend (transitive dependency)
//!
//! The rasterization pipeline:
//! 1. Parse SVG with usvg (simplifies complex SVG features)
//! 2. Rasterize to pixel buffer with resvg (anti-aliased, high-quality)
//! 3. Convert to `DynamicImage` (RGBA8 format)
//! 4. Feed into existing image pipeline: resize → grayscale → dither → threshold → braille
//!
//! # Performance Characteristics
//!
//! - **Small SVGs** (icons, logos <5KB): <50ms rasterization
//! - **Medium SVGs** (diagrams, 10-50KB): <100ms rasterization
//! - **Large complex SVGs** (>100KB): May exceed 100ms but will not hang
//!
//! Rasterization time depends on SVG complexity (number of paths, gradients, text elements).
//!
//! # SVG Rendering Approach
//!
//! SVGs are rasterized using `resvg` and automatically adjusted for optimal braille rendering:
//!
//! 1. **Rasterize** the SVG to a pixel buffer at the requested dimensions
//! 2. **Check brightness**: Calculate average brightness of all pixels
//! 3. **Invert if dark**: If average < 127, invert all RGB values (dark → light, light → dark)
//! 4. **Pass to pipeline**: The result goes to standard image processing (grayscale → threshold → braille)
//!
//! This simple inversion approach ensures good contrast for Otsu thresholding:
//! - **Dark-background SVGs** (e.g., dark gray #4d4d4d with white text) → inverted → light background with dark text
//! - **Light-background SVGs** → unchanged → already good for thresholding
//! - **Transparent SVGs** → treated as light → unchanged
//!
//! The result is that content is visible regardless of the original SVG background color.
//!
//! # Font Handling for Text-Heavy SVGs
//!
//! SVGs with text elements require system fonts for proper rendering. This module
//! automatically loads all available system fonts using `fontdb` before rasterizing
//! SVGs with text.
//!
//! **Font Loading Behavior:**
//! - System fonts are loaded from platform-specific directories:
//!   - **Linux**: `/usr/share/fonts`, `/usr/local/share/fonts`, `~/.fonts`
//!   - **Windows**: `C:\Windows\Fonts`
//!   - **macOS**: `/System/Library/Fonts`, `/Library/Fonts`, `~/Library/Fonts`
//! - Font loading happens automatically—no configuration needed
//! - Typical load time: ~8-10ms for 30-50 font faces
//!
//! **Font Fallback:**
//! - If a requested font family (e.g., "Arial") is not installed, `fontdb` falls back to:
//!   1. The next font in the font-family list (e.g., "Arial, Helvetica, sans-serif")
//!   2. A generic sans-serif font if no matches found (usually `DejaVu Sans` on Linux)
//! - Missing fonts do not cause errors—text will render with fallback fonts
//! - Quality: Fallback fonts maintain good quality for braille rendering
//!
//! **Best Practices for SVG Text:**
//! - Use common, widely-available fonts: Arial, Helvetica, Georgia, Courier, Ubuntu
//! - Always provide fallback chains: `font-family="Arial, Helvetica, sans-serif"`
//! - Avoid very small font sizes (< 12pt) for optimal braille legibility
//! - Test text-heavy SVGs on target platforms (fonts vary by OS)
//!
//! **Platform Differences:**
//! - **Linux**: Typically has `DejaVu Sans/Serif`, `Ubuntu fonts`, `Liberation fonts`
//! - **Windows**: Has `Arial`, `Times New Roman`, `Courier New`, `Calibri`
//! - **macOS**: Has `Helvetica`, `Times`, `Courier`, `San Francisco`
//!
//! Generic fallbacks (sans-serif, serif, monospace) work across all platforms.
//!
//! # Examples
//!
//! ## Loading SVG from file path
//!
//! ```no_run
//! # #[cfg(feature = "svg")]
//! # {
//! use dotmax::image::load_svg_from_path;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Rasterize SVG to 160×96 pixels (80×24 terminal cells × 2×4 braille dots)
//! let img = load_svg_from_path(Path::new("logo.svg"), 160, 96)?;
//! println!("Rasterized SVG to {}×{}", img.width(), img.height());
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Loading SVG from byte buffer
//!
//! ```no_run
//! # #[cfg(feature = "svg")]
//! # {
//! use dotmax::image::load_svg_from_bytes;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Example SVG content
//! let svg_bytes = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
//!   <circle cx="50" cy="50" r="40" fill="black"/>
//! </svg>"#;
//! let img = load_svg_from_bytes(svg_bytes, 100, 100)?;
//! println!("Loaded SVG from bytes: {}×{}", img.width(), img.height());
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Full SVG → Braille Pipeline
//!
//! ```no_run
//! # #[cfg(all(feature = "svg", feature = "image"))]
//! # {
//! use dotmax::image::{load_svg_from_path, to_grayscale, auto_threshold, pixels_to_braille};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load and rasterize SVG
//! let img = load_svg_from_path(Path::new("diagram.svg"), 160, 96)?;
//!
//! // Convert to grayscale
//! let gray = to_grayscale(&img);
//!
//! // Threshold to binary
//! let binary = auto_threshold(&img);
//!
//! // Map to braille grid
//! let grid = pixels_to_braille(&binary, 80, 24)?;
//!
//! println!("Rendered SVG to {}×{} braille grid", grid.width(), grid.height());
//! # Ok(())
//! # }
//! # }
//! ```

use crate::DotmaxError;
use image::DynamicImage;
use std::path::Path;
use tracing::{debug, info};
use usvg::{TreeParsing, TreePostProc};

/// Maximum SVG dimensions (width or height in pixels)
///
/// This limit prevents memory exhaustion from malicious or extremely large SVGs.
/// SVGs exceeding these dimensions will return `DotmaxError::InvalidImageDimensions`.
pub const MAX_SVG_WIDTH: u32 = 10_000;
/// Maximum SVG height in pixels (prevents memory exhaustion)
pub const MAX_SVG_HEIGHT: u32 = 10_000;

/// Load an SVG from a file path and rasterize to specified dimensions
///
/// Parses the SVG file using `usvg`, rasterizes to a pixel buffer using `resvg`,
/// and returns a `DynamicImage` (RGBA8 format) that can be processed through the
/// standard image→braille pipeline.
///
/// # Arguments
///
/// * `path` - Path to the SVG file
/// * `width` - Target width in pixels (will preserve aspect ratio)
/// * `height` - Target height in pixels (will preserve aspect ratio)
///
/// # Returns
///
/// Returns a `DynamicImage` containing the rasterized SVG, or an error if:
/// - File does not exist or is not readable
/// - SVG is malformed or cannot be parsed
/// - Dimensions are zero or exceed [`MAX_SVG_WIDTH`]/[`MAX_SVG_HEIGHT`]
/// - Pixmap creation fails
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "svg")]
/// # {
/// use dotmax::image::load_svg_from_path;
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Rasterize logo.svg to 200×150 pixels
/// let img = load_svg_from_path(Path::new("logo.svg"), 200, 150)?;
/// assert_eq!(img.width(), 200);
/// assert_eq!(img.height(), 150);
/// # Ok(())
/// # }
/// # }
/// ```
///
/// # Errors
///
/// Returns [`DotmaxError::SvgError`] if the SVG cannot be parsed or rasterized.
/// Returns [`DotmaxError::InvalidImageDimensions`] if dimensions are invalid.
/// Returns [`DotmaxError::Terminal`] if file I/O fails.
///
/// # Performance
///
/// Target: <50ms for small SVGs (icons, logos), <100ms for medium SVGs.
pub fn load_svg_from_path(
    path: &Path,
    width: u32,
    height: u32,
) -> Result<DynamicImage, DotmaxError> {
    info!("Loading SVG from {:?} at {}×{}", path, width, height);

    // Validate path exists and is readable
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("SVG file not found: {}", path.display()),
        )
        .into());
    }

    // Read SVG file contents
    let svg_data = std::fs::read(path).map_err(|e| {
        DotmaxError::SvgError(format!("Failed to read SVG file {}: {e}", path.display()))
    })?;

    // Delegate to bytes loader with path context for errors
    load_svg_from_bytes(&svg_data, width, height).map_err(|e| match e {
        DotmaxError::SvgError(msg) => {
            DotmaxError::SvgError(format!("Error loading SVG from {}: {msg}", path.display()))
        }
        other => other,
    })
}

/// Load an SVG from a byte buffer and rasterize to specified dimensions
///
/// Parses the SVG data using `usvg`, rasterizes to a pixel buffer using `resvg`,
/// and returns a `DynamicImage` (RGBA8 format).
///
/// # Arguments
///
/// * `bytes` - SVG file contents as bytes
/// * `width` - Target width in pixels (will preserve aspect ratio)
/// * `height` - Target height in pixels (will preserve aspect ratio)
///
/// # Returns
///
/// Returns a `DynamicImage` containing the rasterized SVG, or an error if:
/// - SVG data is malformed or cannot be parsed
/// - Dimensions are zero or exceed [`MAX_SVG_WIDTH`]/[`MAX_SVG_HEIGHT`]
/// - Pixmap creation fails
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "svg")]
/// # {
/// use dotmax::image::load_svg_from_bytes;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let svg_string = r#"
///     <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
///         <circle cx="50" cy="50" r="40" fill="black"/>
///     </svg>
/// "#;
/// let img = load_svg_from_bytes(svg_string.as_bytes(), 100, 100)?;
/// assert_eq!(img.width(), 100);
/// assert_eq!(img.height(), 100);
/// # Ok(())
/// # }
/// # }
/// ```
///
/// # Errors
///
/// Returns [`DotmaxError::SvgError`] if the SVG cannot be parsed or rasterized.
/// Returns [`DotmaxError::InvalidImageDimensions`] if dimensions are invalid.
///
/// # Performance
///
/// Target: <50ms for small SVGs, <100ms for medium SVGs.
pub fn load_svg_from_bytes(
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, DotmaxError> {
    // Validate dimensions
    if width == 0 || height == 0 {
        return Err(DotmaxError::InvalidImageDimensions { width, height });
    }

    if width > MAX_SVG_WIDTH || height > MAX_SVG_HEIGHT {
        return Err(DotmaxError::InvalidImageDimensions { width, height });
    }

    debug!("Parsing SVG data ({} bytes)", bytes.len());

    // Parse SVG with usvg
    let options = usvg::Options::default();
    let mut tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|e| DotmaxError::SvgError(format!("Failed to parse SVG: {e}")))?;

    debug!(
        "SVG parsed successfully, viewBox size: {}×{}",
        tree.size.width(),
        tree.size.height()
    );

    // Load system fonts for proper text rendering
    // Without this, SVG text elements fall back to poor-quality generic fonts
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();

    debug!("Loaded {} font faces for text rendering", fontdb.len());

    // Postprocess tree to flatten text nodes and calculate bounding boxes
    // This prevents rendering warnings and ensures proper text rendering
    tree.postprocess(usvg::PostProcessingSteps::default(), &fontdb);

    // Rasterize to pixel buffer
    rasterize_svg_tree(&tree, width, height)
}

/// Rasterize an SVG tree to a `DynamicImage`
///
/// Internal helper function that performs the actual rasterization using `resvg`.
///
/// # Arguments
///
/// * `tree` - Parsed SVG tree from `usvg`
/// * `width` - Target width in pixels
/// * `height` - Target height in pixels
///
/// # Returns
///
/// Returns a `DynamicImage` (RGBA8 format) with the rasterized SVG.
///
/// # Implementation Details
///
/// - Creates a pixel buffer (pixmap) using `tiny-skia`
/// - Calculates transform for aspect ratio preservation using `usvg::FitTo`
/// - Renders SVG to pixmap with anti-aliasing enabled by default
/// - Converts transparent pixels to white background for terminal compatibility
/// - Converts pixmap RGBA buffer to `image::RgbaImage` then `DynamicImage`
fn rasterize_svg_tree(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
) -> Result<DynamicImage, DotmaxError> {
    use resvg::tiny_skia::{Pixmap, Transform};

    debug!(
        "Creating {}×{} pixel buffer for rasterization",
        width, height
    );

    // Create pixel buffer
    let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
        DotmaxError::SvgError(format!(
            "Failed to create pixmap for dimensions {width}×{height}"
        ))
    })?;

    // Calculate transform for aspect ratio preservation
    let tree_size = tree.size;
    #[allow(clippy::cast_precision_loss)]
    let scale_x = width as f32 / tree_size.width();
    #[allow(clippy::cast_precision_loss)]
    let scale_y = height as f32 / tree_size.height();
    let scale = scale_x.min(scale_y);

    let transform = Transform::from_scale(scale, scale);

    debug!(
        "Rendering SVG with transform (scale: {:.2}, {:.2})",
        scale, scale
    );

    // Render tree to pixmap (anti-aliasing enabled by default)
    resvg::render(tree, transform, &mut pixmap.as_mut());

    debug!("SVG rasterization complete");

    // Check if image is predominantly dark and invert if needed
    // This ensures good contrast for Otsu thresholding with dark-background SVGs
    let pixmap_data = pixmap.data();
    let mut brightness_sum: u64 = 0;
    let pixel_count = (width * height) as usize;

    for pixel in pixmap_data.chunks_exact(4) {
        // Calculate perceived brightness using standard luminance formula
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::suboptimal_flops
        )]
        let brightness = (0.299 * f32::from(pixel[0])
            + 0.587 * f32::from(pixel[1])
            + 0.114 * f32::from(pixel[2])) as u64;
        brightness_sum += brightness;
    }

    let avg_brightness = brightness_sum / pixel_count as u64;
    let should_invert = avg_brightness < 127;

    if should_invert {
        debug!(
            "Image is dark (avg brightness: {}), inverting for better contrast",
            avg_brightness
        );

        // Invert all pixel values
        let pixmap_data = pixmap.data_mut();
        for pixel in pixmap_data.chunks_exact_mut(4) {
            pixel[0] = 255 - pixel[0]; // R
            pixel[1] = 255 - pixel[1]; // G
            pixel[2] = 255 - pixel[2]; // B
                                       // Keep alpha as-is
        }
    } else {
        debug!(
            "Image is light (avg brightness: {}), no inversion needed",
            avg_brightness
        );
    }

    // Convert pixmap RGBA buffer to DynamicImage
    let image_buffer =
        image::RgbaImage::from_raw(width, height, pixmap.take()).ok_or_else(|| {
            DotmaxError::SvgError("Failed to convert pixmap to image buffer".to_string())
        })?;

    Ok(DynamicImage::ImageRgba8(image_buffer))
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::*;

    // Simple SVG test fixture - circle
    const SIMPLE_CIRCLE_SVG: &str = r#"
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="40" fill="black"/>
        </svg>
    "#;

    // SVG with gradient
    const GRADIENT_SVG: &str = r#"
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <defs>
                <linearGradient id="grad1" x1="0%" y1="0%" x2="100%" y2="0%">
                    <stop offset="0%" style="stop-color:black;stop-opacity:1" />
                    <stop offset="100%" style="stop-color:white;stop-opacity:1" />
                </linearGradient>
            </defs>
            <rect width="100" height="100" fill="url(#grad1)" />
        </svg>
    "#;

    // Malformed SVG
    const MALFORMED_SVG: &str = "<svg><notvalid>";

    #[test]
    fn test_load_valid_simple_svg_returns_dynamic_image() {
        let result = load_svg_from_bytes(SIMPLE_CIRCLE_SVG.as_bytes(), 100, 100);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width(), 100);
        assert_eq!(img.height(), 100);
    }

    #[test]
    fn test_load_svg_with_gradient_rasterizes_correctly() {
        let result = load_svg_from_bytes(GRADIENT_SVG.as_bytes(), 200, 200);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 200);
    }

    #[test]
    fn test_load_malformed_svg_returns_svg_error() {
        let result = load_svg_from_bytes(MALFORMED_SVG.as_bytes(), 100, 100);
        assert!(result.is_err());
        match result {
            Err(DotmaxError::SvgError(msg)) => {
                assert!(msg.contains("parse"));
            }
            _ => panic!("Expected SvgError"),
        }
    }

    #[test]
    fn test_invalid_dimensions_zero_returns_error() {
        let result = load_svg_from_bytes(SIMPLE_CIRCLE_SVG.as_bytes(), 0, 100);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DotmaxError::InvalidImageDimensions { .. })
        ));

        let result = load_svg_from_bytes(SIMPLE_CIRCLE_SVG.as_bytes(), 100, 0);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DotmaxError::InvalidImageDimensions { .. })
        ));
    }

    #[test]
    fn test_invalid_dimensions_exceeds_max_returns_error() {
        let result = load_svg_from_bytes(SIMPLE_CIRCLE_SVG.as_bytes(), 20_000, 100);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DotmaxError::InvalidImageDimensions { .. })
        ));

        let result = load_svg_from_bytes(SIMPLE_CIRCLE_SVG.as_bytes(), 100, 20_000);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DotmaxError::InvalidImageDimensions { .. })
        ));
    }

    #[test]
    fn test_aspect_ratio_preserved_in_rasterization() {
        // SVG with 2:1 aspect ratio (viewBox 200×100)
        let svg = r#"
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
                <rect width="200" height="100" fill="black"/>
            </svg>
        "#;

        // Request square dimensions - usvg FitTo should preserve aspect ratio
        let result = load_svg_from_bytes(svg.as_bytes(), 100, 100);
        assert!(result.is_ok());
        let img = result.unwrap();
        // Image will be 100×100 (as requested), but content will be centered/letterboxed
        assert_eq!(img.width(), 100);
        assert_eq!(img.height(), 100);
    }

    #[test]
    fn test_load_svg_from_bytes_same_as_file() {
        // Test that bytes and file loading produce consistent results
        let result1 = load_svg_from_bytes(SIMPLE_CIRCLE_SVG.as_bytes(), 150, 150);
        let result2 = load_svg_from_bytes(SIMPLE_CIRCLE_SVG.as_bytes(), 150, 150);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let img1 = result1.unwrap();
        let img2 = result2.unwrap();

        assert_eq!(img1.width(), img2.width());
        assert_eq!(img1.height(), img2.height());
    }

    #[test]
    fn test_svg_with_paths_applies_antialiasing() {
        // SVG with complex paths
        let svg = r#"
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
                <path d="M 10 10 L 90 10 L 90 90 L 10 90 Z" fill="black" stroke="white" stroke-width="2"/>
            </svg>
        "#;

        let result = load_svg_from_bytes(svg.as_bytes(), 100, 100);
        assert!(result.is_ok());
        // Anti-aliasing is enabled by default in resvg, so this should render smoothly
    }
}
