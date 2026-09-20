//! Image rendering module (feature-gated)
//!
//! This module provides functionality to load and render images to braille grids.
//! All image loading code is behind the `image` feature flag to keep the core
//! library lightweight.
//!
//! # Feature Gate
//!
//! To use this module, enable the `image` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! dotmax = { version = "0.1", features = ["image"] }
//! ```
//!
//! # Supported Formats
//!
//! The following image formats are supported via the `image` crate:
//! - PNG (Portable Network Graphics)
//! - JPEG/JPG (Joint Photographic Experts Group)
//! - GIF (Graphics Interchange Format)
//! - BMP (Windows Bitmap)
//! - WebP (Google WebP)
//! - TIFF (Tagged Image File Format)
//!
//! Format detection is automatic based on file magic bytes.
//!
//! # SVG Support
//!
//! SVG vector graphics are supported via the `svg` feature flag (separate from raster images).
//! Enable with:
//!
//! ```toml
//! [dependencies]
//! dotmax = { version = "0.1", features = ["svg"] }
//! ```
//!
//! SVG files are rasterized to pixel buffers before braille mapping. See the `svg` module for details.
//!
//! # Examples
//!
//! ## Loading from file path
//!
//! ```no_run
//! use dotmax::image::load_from_path;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let img = load_from_path(Path::new("image.png"))?;
//! println!("Loaded image: {}×{}", img.width(), img.height());
//! # Ok(())
//! # }
//! ```
//!
//! ## Loading from byte buffer
//!
//! ```no_run
//! use dotmax::image::load_from_bytes;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let bytes = include_bytes!("../../tests/fixtures/images/sample.png");
//! let img = load_from_bytes(bytes)?;
//! println!("Loaded image from bytes: {}×{}", img.width(), img.height());
//! # Ok(())
//! # }
//! ```
//!
//! # Performance
//!
//! Image loading targets <5ms for typical images (cached after first load).
//! The primary bottleneck is disk I/O for file-based loading.
//!
//! # Image Processing Pipeline
//!
//! The typical image-to-braille pipeline:
//! 1. Load image with [`load_from_path`] or [`load_from_bytes`]
//! 2. Resize to terminal dimensions with [`resize_to_terminal`]
//! 3. Convert to grayscale with [`convert::to_grayscale`]
//! 4. (Optional) Adjust brightness/contrast/gamma with [`threshold`] module functions
//! 5. (Optional) Apply dithering with [`dither::apply_dithering`] for improved quality
//! 6. Apply thresholding with [`threshold::auto_threshold`] or [`threshold::apply_threshold`]
//! 7. Map to braille grid (Story 3.5)
//!
//! # High-Level API
//!
//! For simple use cases, the [`ImageRenderer`] builder pattern provides a high-level API
//! that handles the full pipeline with sensible defaults:
//!
//! ```no_run
//! use dotmax::image::ImageRenderer;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Simple case: auto-resize to terminal with defaults
//! let grid = ImageRenderer::new()
//!     .load_from_path(Path::new("image.png"))?
//!     .resize_to_terminal()?
//!     .render()?;
//! # Ok(())
//! # }
//! ```
//!
//! For even simpler usage, the [`render_image_simple`] function provides a one-liner:
//!
//! ```no_run
//! use dotmax::image::render_image_simple;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let grid = render_image_simple(Path::new("image.png"))?;
//! # Ok(())
//! # }
//! ```

pub mod color_mode;
pub mod convert;
pub mod dither;
pub mod loader;
pub mod mapper;
pub mod resize;
#[cfg(feature = "svg")]
pub mod svg;
pub mod temporal;
pub mod threshold;

// Re-export public types and functions for convenience
pub use color_mode::{
    render_image_with_color, render_image_with_color_jittered, ColorMode, ColorSamplingStrategy,
};
pub use convert::to_grayscale;
pub use dither::{
    apply_dithering, apply_dithering_jittered, apply_dithering_with_custom_threshold,
    DitheringMethod, JitterParams,
};
pub use loader::{load_from_bytes, load_from_path, supported_formats};
pub use mapper::pixels_to_braille;
pub use resize::{resize_to_dimensions, resize_to_terminal};
#[cfg(feature = "svg")]
pub use svg::{load_svg_from_bytes, load_svg_from_path};
pub use threshold::{
    adjust_brightness, adjust_contrast, adjust_gamma, apply_threshold, auto_threshold,
    otsu_threshold, BinaryImage,
};

// High-level API types and functions are defined below and automatically exported

use crate::{BrailleGrid, DotmaxError};
use image::DynamicImage;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info, instrument};

/// Process-wide fallback seed source for ambient dithering.
///
/// Each call increments a counter; combined with the renderer's own frame
/// counter this guarantees that two renderers created back-to-back still
/// produce different ambient noise patterns. The starting value is mixed with
/// the system clock so different processes don't collide either.
static AMBIENT_GLOBAL_SEED: AtomicU64 = AtomicU64::new(0);

fn next_global_seed_offset() -> u64 {
    AMBIENT_GLOBAL_SEED.fetch_add(1, Ordering::Relaxed)
}

fn clock_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xA5A5_5A5A_5A5A_A5A5)
}

/// Resize mode configuration for [`ImageRenderer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeMode {
    /// Automatically detect terminal dimensions and resize to fit.
    AutoTerminal { preserve_aspect: bool },
    /// Manual dimensions with optional aspect ratio preservation.
    Manual {
        width: usize,
        height: usize,
        preserve_aspect: bool,
    },
}

/// High-level image renderer with fluent builder pattern.
///
/// `ImageRenderer` provides a convenient API for loading, configuring, and rendering
/// images to braille grids with sensible defaults. The builder pattern allows chaining
/// configuration methods for a fluent API.
///
/// # Default Configuration
///
/// - **Dithering**: Floyd-Steinberg (best quality)
/// - **Color mode**: Monochrome (universal compatibility)
/// - **Threshold**: Automatic Otsu threshold (optimal binary conversion)
/// - **Resize**: Automatic terminal dimensions with aspect ratio preservation
/// - **Brightness/Contrast/Gamma**: 1.0 (neutral, no adjustment)
///
/// # Examples
///
/// Basic usage with defaults:
/// ```no_run
/// use dotmax::image::ImageRenderer;
/// use std::path::Path;
///
/// # fn main() -> Result<(), dotmax::DotmaxError> {
/// let grid = ImageRenderer::new()
///     .load_from_path(Path::new("image.png"))?
///     .resize_to_terminal()?
///     .render()?;
/// # Ok(())
/// # }
/// ```
///
/// Full customization:
/// ```no_run
/// use dotmax::image::{ImageRenderer, DitheringMethod, ColorMode};
/// use std::path::Path;
///
/// # fn main() -> Result<(), dotmax::DotmaxError> {
/// let grid = ImageRenderer::new()
///     .load_from_path(Path::new("photo.jpg"))?
///     .resize(100, 50, true)?
///     .brightness(1.2)?
///     .contrast(1.1)?
///     .gamma(0.9)?
///     .dithering(DitheringMethod::Atkinson)
///     .color_mode(ColorMode::TrueColor)
///     .render()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ImageRenderer {
    image: Option<DynamicImage>,
    dithering: DitheringMethod,
    color_mode: ColorMode,
    threshold: Option<u8>,
    resize_mode: ResizeMode,
    brightness: f32,
    contrast: f32,
    gamma: f32,
    /// Ambient dithering intensity in 0.0..=1.0. 0.0 keeps the renderer fully
    /// deterministic; >0.0 injects per-frame noise into the dither and gently
    /// modulates contrast/brightness/gamma/threshold so consecutive `render()`
    /// calls produce visibly different output even for a static input image.
    ambient_intensity: f32,
    /// Optional explicit seed. When `None`, the seed is drawn from the system
    /// clock + a process-global counter on first render and then advances per
    /// render call (so each frame evolves).
    ambient_seed: Option<u64>,
    /// Frame counter advanced by every `render()` call. Combined with
    /// `ambient_seed` to make a unique per-frame jitter seed.
    frame_counter: u64,
    /// ISSUE #3 FIX: Cache for resized image to enable fast re-renders
    /// when only adjustments (brightness/contrast/gamma) change
    cached_resized: Option<DynamicImage>,
    /// Cache for original color image before grayscale conversion
    /// (needed for color mode rendering)
    cached_original_resized: Option<DynamicImage>,
    /// Dimensions used for the cached resized image (to detect terminal resize)
    cached_dimensions: Option<(u32, u32)>,
}

impl ImageRenderer {
    /// Creates a new image renderer with sensible defaults.
    ///
    /// # Default Configuration
    ///
    /// - Dithering: `FloydSteinberg` (best quality)
    /// - Color mode: Monochrome (universal compatibility)
    /// - Threshold: None (automatic Otsu thresholding)
    /// - Resize: `AutoTerminal` with aspect ratio preservation
    /// - Brightness/Contrast/Gamma: 1.0 (neutral)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    ///
    /// let renderer = ImageRenderer::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            image: None,
            dithering: DitheringMethod::FloydSteinberg,
            color_mode: ColorMode::Monochrome,
            threshold: None,
            resize_mode: ResizeMode::AutoTerminal {
                preserve_aspect: true,
            },
            brightness: 1.0,
            contrast: 1.0,
            gamma: 1.0,
            ambient_intensity: 0.0,
            ambient_seed: None,
            frame_counter: 0,
            cached_resized: None,
            cached_original_resized: None,
            cached_dimensions: None,
        }
    }

    /// Enable ambient (temporal) dithering at the given intensity.
    ///
    /// `intensity` is clamped to 0.0..=1.0. With ambient enabled, every call
    /// to [`render`](Self::render) advances an internal frame counter and
    /// injects bounded noise into the dither pipeline plus gentle modulation
    /// of contrast / brightness / gamma / threshold. Same input image →
    /// different braille output per frame, so a still image visibly evolves
    /// instead of locking to a single static pattern.
    ///
    /// 0.0 disables ambient and restores fully deterministic output.
    /// 0.25–0.6 is a calm shimmer; 1.0 is heavy stipple churn.
    #[must_use]
    pub fn ambient(mut self, intensity: f32) -> Self {
        self.ambient_intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Pin the ambient seed so the dither sequence is reproducible.
    ///
    /// Each `render()` call still advances the frame counter — pin the seed
    /// when you want a deterministic animation (e.g., generating a fixed set
    /// of frames for a preview). Pass `None` (or call [`unpin_ambient_seed`]
    /// — see below) to fall back to clock-driven seeding.
    #[must_use]
    pub fn ambient_seed(mut self, seed: u64) -> Self {
        self.ambient_seed = Some(seed);
        self
    }

    /// Set the frame counter used for ambient seeding.
    ///
    /// Useful when driving the renderer from an external animation clock —
    /// pass the frame index and you'll get a deterministic, evolving output
    /// per frame.
    #[must_use]
    pub fn ambient_frame(mut self, frame: u64) -> Self {
        self.frame_counter = frame;
        self
    }

    /// Compute the JitterParams for the current frame and advance the counter.
    fn next_jitter(&mut self) -> JitterParams {
        if self.ambient_intensity <= 0.0 {
            return JitterParams::NONE;
        }
        let base = self.ambient_seed.unwrap_or_else(|| {
            clock_seed() ^ next_global_seed_offset().wrapping_mul(0x9E37_79B9_7F4A_7C15)
        });
        // Mix in the frame counter so each render advances the pattern.
        let mut s = base ^ self.frame_counter.wrapping_mul(0xD1B5_4A32_D192_ED03);
        s ^= s >> 33;
        s = s.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        s ^= s >> 33;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        JitterParams::new(s, self.ambient_intensity)
    }

    /// Apply ambient modulation to a base parameter (e.g. contrast).
    ///
    /// Produces a frame-dependent variation in roughly `base * (1 ± amp)`.
    fn modulated(base: f32, jitter: JitterParams, amp: f32, freq: u64) -> f32 {
        if !jitter.enabled() {
            return base;
        }
        // Cheap deterministic wave: hash the seed to a phase, take cosine.
        let phase = (jitter.seed.wrapping_mul(freq) >> 32) as u32 as f32 / u32::MAX as f32;
        let wave = (phase * std::f32::consts::TAU).cos();
        base * (1.0 + amp * jitter.intensity * wave)
    }

    /// Loads an image from a file path.
    ///
    /// Supports PNG, JPEG, GIF, BMP, WebP, and TIFF formats via automatic format detection.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the image file
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining on success.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::ImageLoad`] if:
    /// - File does not exist
    /// - File format is not supported
    /// - Image data is corrupted
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::image::ImageRenderer;
    /// use std::path::Path;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let renderer = ImageRenderer::new()
    ///     .load_from_path(Path::new("image.png"))?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub fn load_from_path(mut self, path: &Path) -> Result<Self, DotmaxError> {
        let img = load_from_path(path)?;
        info!(
            "Loaded image from {:?}, dimensions: {}x{}",
            path,
            img.width(),
            img.height()
        );
        self.image = Some(img);
        // ISSUE #3: Invalidate cache when new image loaded
        self.cached_resized = None;
        self.cached_original_resized = None;
        self.cached_dimensions = None;
        Ok(self)
    }

    /// Loads an image from a byte buffer.
    ///
    /// Useful for loading images from memory or embedded resources.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw image data
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining on success.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::UnsupportedFormat`] if the image format cannot be determined
    /// or is not supported.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::image::ImageRenderer;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let bytes = include_bytes!("../../tests/fixtures/images/sample.png");
    /// let renderer = ImageRenderer::new()
    ///     .load_from_bytes(bytes)?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, bytes))]
    pub fn load_from_bytes(mut self, bytes: &[u8]) -> Result<Self, DotmaxError> {
        let img = load_from_bytes(bytes)?;
        info!(
            "Loaded image from bytes, dimensions: {}x{}",
            img.width(),
            img.height()
        );
        self.image = Some(img);
        // ISSUE #3: Invalidate cache when new image loaded
        self.cached_resized = None;
        self.cached_original_resized = None;
        self.cached_dimensions = None;
        Ok(self)
    }

    /// Loads an image from an RGBA image buffer.
    ///
    /// This method is useful for loading images from memory when you already
    /// have the decoded RGBA pixel data (e.g., from GIF frame decoding).
    ///
    /// # Arguments
    ///
    /// * `img` - RGBA image buffer
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    /// use image::RgbaImage;
    ///
    /// // Create a 100x100 red image
    /// let img = RgbaImage::from_fn(100, 100, |_, _| {
    ///     image::Rgba([255, 0, 0, 255])
    /// });
    ///
    /// let renderer = ImageRenderer::new().load_from_rgba(img);
    /// ```
    #[must_use]
    pub fn load_from_rgba(mut self, img: image::RgbaImage) -> Self {
        info!(
            "Loaded RGBA image, dimensions: {}x{}",
            img.width(),
            img.height()
        );
        self.image = Some(DynamicImage::ImageRgba8(img));
        // Invalidate cache when new image loaded
        self.cached_resized = None;
        self.cached_original_resized = None;
        self.cached_dimensions = None;
        self
    }

    /// Loads an SVG image from a file path and rasterizes it to the specified dimensions.
    ///
    /// This method is only available when the `svg` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SVG file
    /// * `width` - Rasterization width in pixels
    /// * `height` - Rasterization height in pixels
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining on success.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::SvgError`] if:
    /// - SVG file cannot be parsed
    /// - Rasterization fails
    /// - File does not exist
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "svg")]
    /// # {
    /// use dotmax::image::ImageRenderer;
    /// use std::path::Path;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let renderer = ImageRenderer::new()
    ///     .load_svg_from_path(Path::new("graphic.svg"), 800, 600)?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    #[cfg(feature = "svg")]
    #[instrument(skip(self))]
    pub fn load_svg_from_path(
        mut self,
        path: &Path,
        width: u32,
        height: u32,
    ) -> Result<Self, DotmaxError> {
        let img = svg::load_svg_from_path(path, width, height)?;
        info!(
            "Loaded SVG from {:?}, rasterized to {}x{}",
            path, width, height
        );
        self.image = Some(img);
        // ISSUE #3: Invalidate cache when new image loaded
        self.cached_resized = None;
        self.cached_original_resized = None;
        self.cached_dimensions = None;
        Ok(self)
    }

    /// Configures automatic terminal-sized rendering.
    ///
    /// The image will be automatically resized to fit the current terminal dimensions
    /// while preserving aspect ratio by default. If terminal size detection fails,
    /// falls back to 80×24 cells.
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining.
    ///
    /// # Errors
    ///
    /// This method does not currently error, but returns `Result` for API consistency
    /// with other builder methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let renderer = ImageRenderer::new()
    ///     .resize_to_terminal()?;
    /// # Ok(())
    /// # }
    /// ```
    pub const fn resize_to_terminal(mut self) -> Result<Self, DotmaxError> {
        self.resize_mode = ResizeMode::AutoTerminal {
            preserve_aspect: true,
        };
        Ok(self)
    }

    /// Configures manual image dimensions.
    ///
    /// # Arguments
    ///
    /// * `width` - Target width in braille cells
    /// * `height` - Target height in braille cells
    /// * `preserve_aspect` - If true, letterbox to preserve aspect ratio; if false, stretch to fit
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining on success.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::InvalidDimensions`] if width or height is 0 or exceeds 10,000.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let renderer = ImageRenderer::new()
    ///     .resize(100, 50, true)?;  // 100x50 cells, preserve aspect ratio
    /// # Ok(())
    /// # }
    /// ```
    pub fn resize(
        mut self,
        width: usize,
        height: usize,
        preserve_aspect: bool,
    ) -> Result<Self, DotmaxError> {
        if width == 0 || height == 0 || width > 10_000 || height > 10_000 {
            return Err(DotmaxError::InvalidDimensions { width, height });
        }
        self.resize_mode = ResizeMode::Manual {
            width,
            height,
            preserve_aspect,
        };
        Ok(self)
    }

    /// Adjusts image brightness.
    ///
    /// # Arguments
    ///
    /// * `factor` - Brightness multiplier (0.0-2.0, default 1.0)
    ///   - 0.0: completely black
    ///   - 1.0: original brightness
    ///   - 2.0: twice as bright
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining on success.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::InvalidParameter`] if factor is outside the valid range (0.0-2.0).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let renderer = ImageRenderer::new()
    ///     .brightness(1.2)?;  // 20% brighter
    /// # Ok(())
    /// # }
    /// ```
    pub fn brightness(mut self, factor: f32) -> Result<Self, DotmaxError> {
        if !(0.0..=2.0).contains(&factor) {
            return Err(DotmaxError::InvalidParameter {
                parameter_name: "brightness".to_string(),
                value: factor.to_string(),
                min: "0.0".to_string(),
                max: "2.0".to_string(),
            });
        }
        self.brightness = factor;
        Ok(self)
    }

    /// Adjusts image contrast.
    ///
    /// # Arguments
    ///
    /// * `factor` - Contrast multiplier (0.0-2.0, default 1.0)
    ///   - 0.0: no contrast (uniform gray)
    ///   - 1.0: original contrast
    ///   - 2.0: twice the contrast
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining on success.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::InvalidParameter`] if factor is outside the valid range (0.0-2.0).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let renderer = ImageRenderer::new()
    ///     .contrast(1.3)?;  // 30% more contrast
    /// # Ok(())
    /// # }
    /// ```
    pub fn contrast(mut self, factor: f32) -> Result<Self, DotmaxError> {
        if !(0.0..=2.0).contains(&factor) {
            return Err(DotmaxError::InvalidParameter {
                parameter_name: "contrast".to_string(),
                value: factor.to_string(),
                min: "0.0".to_string(),
                max: "2.0".to_string(),
            });
        }
        self.contrast = factor;
        Ok(self)
    }

    /// Adjusts image gamma.
    ///
    /// # Arguments
    ///
    /// * `value` - Gamma correction value (0.1-3.0, default 1.0)
    ///   - <1.0: darkens the image (gamma correction)
    ///   - 1.0: no gamma adjustment
    ///   - >1.0: lightens the image
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining on success.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::InvalidParameter`] if value is outside the valid range (0.1-3.0).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let renderer = ImageRenderer::new()
    ///     .gamma(0.8)?;  // Darken with gamma correction
    /// # Ok(())
    /// # }
    /// ```
    pub fn gamma(mut self, value: f32) -> Result<Self, DotmaxError> {
        if !(0.1..=3.0).contains(&value) {
            return Err(DotmaxError::InvalidParameter {
                parameter_name: "gamma".to_string(),
                value: value.to_string(),
                min: "0.1".to_string(),
                max: "3.0".to_string(),
            });
        }
        self.gamma = value;
        Ok(self)
    }

    /// Configures the dithering algorithm.
    ///
    /// # Arguments
    ///
    /// * `method` - Dithering algorithm to use
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining.
    ///
    /// # Dithering Methods
    ///
    /// - `FloydSteinberg`: Error diffusion, best quality (default)
    /// - `Bayer`: Ordered dithering, good for gradients, faster
    /// - `Atkinson`: Error diffusion, softer appearance
    /// - `None`: No dithering, direct threshold
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::{ImageRenderer, DitheringMethod};
    ///
    /// let renderer = ImageRenderer::new()
    ///     .dithering(DitheringMethod::Atkinson);
    /// ```
    #[must_use]
    pub const fn dithering(mut self, method: DitheringMethod) -> Self {
        self.dithering = method;
        self
    }

    /// Configures the color rendering mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Color mode to use
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining.
    ///
    /// # Color Modes
    ///
    /// - `Monochrome`: Black/white only (default)
    /// - `Grayscale`: 256 shades using color intensity
    /// - `TrueColor`: Full RGB color per braille cell
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::{ImageRenderer, ColorMode};
    ///
    /// let renderer = ImageRenderer::new()
    ///     .color_mode(ColorMode::TrueColor);
    /// ```
    #[must_use]
    pub const fn color_mode(mut self, mode: ColorMode) -> Self {
        self.color_mode = mode;
        self
    }

    /// Sets a manual threshold value (overrides automatic Otsu thresholding).
    ///
    /// # Arguments
    ///
    /// * `value` - Threshold value (0-255)
    ///   - Pixels below this value become white
    ///   - Pixels at or above this value become black
    ///
    /// # Returns
    ///
    /// Returns `Self` for method chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::image::ImageRenderer;
    ///
    /// let renderer = ImageRenderer::new()
    ///     .threshold(128);  // Mid-point threshold
    /// ```
    #[must_use]
    pub const fn threshold(mut self, value: u8) -> Self {
        self.threshold = Some(value);
        self
    }

    /// Executes the full image rendering pipeline.
    ///
    /// This method performs the following steps:
    /// 1. Validates that an image has been loaded
    /// 2. Resizes the image based on the configured resize mode
    /// 3. Applies brightness/contrast/gamma adjustments if configured
    /// 4. Converts to grayscale (if color mode is Monochrome)
    /// 5. Applies dithering or thresholding
    /// 6. Maps pixels to braille dots
    /// 7. Applies colors if color mode is not Monochrome
    ///
    /// # Returns
    ///
    /// Returns a [`BrailleGrid`] ready for terminal rendering.
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::InvalidParameter`] if no image has been loaded
    /// (call [`load_from_path`](Self::load_from_path) or
    /// [`load_from_bytes`](Self::load_from_bytes) first).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::image::ImageRenderer;
    /// use std::path::Path;
    ///
    /// # fn main() -> Result<(), dotmax::DotmaxError> {
    /// let grid = ImageRenderer::new()
    ///     .load_from_path(Path::new("image.png"))?
    ///     .resize_to_terminal()?
    ///     .render()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## ISSUE #3 FIX: Performance Caching
    ///
    /// This method now caches the resized image internally. When called multiple times
    /// with only brightness/contrast/gamma changes, it reuses the cached resized image
    /// instead of re-loading and re-resizing. This enables responsive parameter adjustments
    /// (<10ms) suitable for interactive image editing.
    ///
    /// Cache is automatically invalidated when:
    /// - A new image is loaded (`load_from_path`, `load_from_bytes`, `load_svg_from_path`)
    /// - Resize mode changes
    ///
    /// Cache is preserved (fast path) when only these change:
    /// - Brightness, contrast, gamma adjustments
    /// - Color mode
    /// - Dithering method
    /// - Threshold value
    #[instrument(skip(self))]
    #[allow(clippy::too_many_lines)]
    pub fn render(&mut self) -> Result<BrailleGrid, DotmaxError> {
        // Validate image is loaded
        let img = self
            .image
            .as_ref()
            .ok_or_else(|| DotmaxError::InvalidParameter {
                parameter_name: "image".to_string(),
                value: "None".to_string(),
                min: "Must load image first".to_string(),
                max: "loaded image".to_string(),
            })?;

        info!("Starting image rendering pipeline");

        // Calculate target dimensions
        let (target_width_pixels, target_height_pixels) = self.calculate_target_dimensions();
        debug!(
            "Target dimensions: {}x{} pixels",
            target_width_pixels, target_height_pixels
        );

        // ISSUE #3 FIX: Check if we can reuse cached resized image
        // Must check both cache existence AND that dimensions haven't changed (e.g., terminal resize)
        let dimensions_match = self
            .cached_dimensions
            .is_some_and(|(w, h)| w == target_width_pixels && h == target_height_pixels);

        let resized = if let Some(cached) = &self.cached_resized {
            if dimensions_match {
                // Fast path: reuse cached resized image (dimensions unchanged)
                debug!("Using cached resized image (fast path for parameter adjustments)");
                cached.clone()
            } else {
                // Dimensions changed (e.g., terminal resize) - must re-resize
                debug!(
                    "Dimensions changed from {:?} to {}x{}, invalidating cache and re-resizing",
                    self.cached_dimensions, target_width_pixels, target_height_pixels
                );
                let resized = match &self.resize_mode {
                    ResizeMode::AutoTerminal { preserve_aspect }
                    | ResizeMode::Manual {
                        preserve_aspect, ..
                    } => resize_to_dimensions(
                        img,
                        target_width_pixels,
                        target_height_pixels,
                        *preserve_aspect,
                    )?,
                };
                debug!(
                    "Image resized to {}x{}, caching for future renders",
                    resized.width(),
                    resized.height()
                );
                self.cached_resized = Some(resized.clone());
                self.cached_original_resized = Some(resized.clone());
                self.cached_dimensions = Some((target_width_pixels, target_height_pixels));
                resized
            }
        } else {
            // Slow path: resize image and cache it (no cache available)
            debug!("Resizing image (no cache available)");
            let resized = match &self.resize_mode {
                ResizeMode::AutoTerminal { preserve_aspect }
                | ResizeMode::Manual {
                    preserve_aspect, ..
                } => resize_to_dimensions(
                    img,
                    target_width_pixels,
                    target_height_pixels,
                    *preserve_aspect,
                )?,
            };
            debug!(
                "Image resized to {}x{}, caching for future renders",
                resized.width(),
                resized.height()
            );
            self.cached_resized = Some(resized.clone());
            self.cached_original_resized = Some(resized.clone());
            self.cached_dimensions = Some((target_width_pixels, target_height_pixels));
            resized
        };

        // Frame-by-frame jitter: advances the seed each render so consecutive
        // calls visibly evolve. JitterParams::NONE when ambient_intensity=0.
        let jitter = self.next_jitter();

        // Ambient modulation of the user-set tone curve. Each parameter gets
        // its own frequency multiplier so they don't all wiggle in lock-step.
        let modulated_brightness = Self::modulated(self.brightness, jitter, 0.10, 0x9E37_79B9);
        let modulated_contrast = Self::modulated(self.contrast, jitter, 0.18, 0x6B5F_7A91);
        let modulated_gamma = Self::modulated(self.gamma, jitter, 0.12, 0xC2B2_AE3D);
        let modulated_threshold = self.threshold.map(|t| {
            let m = Self::modulated(t as f32, jitter, 0.18, 0xA24B_AED4);
            m.clamp(0.0, 255.0) as u8
        });

        // ISSUE #1 FIX: Pass all rendering settings to color pipeline
        // to ensure consistent behavior across color modes
        if self.color_mode != ColorMode::Monochrome {
            info!("Using color rendering pipeline for {:?}", self.color_mode);
            let cell_width = target_width_pixels as usize / 2;
            let cell_height = target_height_pixels as usize / 4;
            return render_image_with_color_jittered(
                &resized,
                self.color_mode,
                cell_width,
                cell_height,
                self.dithering,
                modulated_threshold,
                modulated_brightness,
                modulated_contrast,
                modulated_gamma,
                jitter,
            );
        }

        // Convert to grayscale
        let mut gray = to_grayscale(&resized);
        debug!("Converted to grayscale");

        // Apply adjustments (with epsilon for float comparison)
        const EPSILON: f32 = 0.001;
        if (modulated_brightness - 1.0).abs() > EPSILON {
            // adjust_brightness clamps to 0.0..=2.0 and errors otherwise; modulation
            // can push the value slightly out of range, so clamp before applying.
            let b = modulated_brightness.clamp(0.0, 2.0);
            gray = adjust_brightness(&gray, b)?;
            debug!("Applied brightness adjustment: {}", b);
        }
        if (modulated_contrast - 1.0).abs() > EPSILON {
            let c = modulated_contrast.clamp(0.0, 2.0);
            gray = adjust_contrast(&gray, c)?;
            debug!("Applied contrast adjustment: {}", c);
        }
        if (modulated_gamma - 1.0).abs() > EPSILON {
            let g = modulated_gamma.clamp(0.1, 3.0);
            gray = adjust_gamma(&gray, g)?;
            debug!("Applied gamma adjustment: {}", g);
        }

        // Convert to binary (dithering or threshold) with jitter.
        let binary = apply_dithering_jittered(&gray, self.dithering, modulated_threshold, jitter)?;

        // Map to braille grid
        let cell_width = target_width_pixels as usize / 2;
        let cell_height = target_height_pixels as usize / 4;
        let grid = pixels_to_braille(&binary, cell_width, cell_height)?;
        info!(
            "Rendering complete: {}x{} braille cells",
            cell_width, cell_height
        );

        Ok(grid)
    }

    /// Helper method to calculate target pixel dimensions based on resize mode.
    #[allow(clippy::cast_possible_truncation)] // Terminal dimensions won't exceed u32
    fn calculate_target_dimensions(&self) -> (u32, u32) {
        match &self.resize_mode {
            ResizeMode::AutoTerminal { .. } => {
                let (cols, rows) = detect_terminal_size();
                // Convert cells to pixels: width×2 (2 dots wide), height×4 (4 dots tall)
                (cols as u32 * 2, rows as u32 * 4)
            }
            ResizeMode::Manual { width, height, .. } => {
                // Convert cells to pixels
                (*width as u32 * 2, *height as u32 * 4)
            }
        }
    }
}

impl Default for ImageRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// One-liner convenience function for simple image rendering.
///
/// Loads an image from a file path, automatically resizes it to fit the terminal,
/// and renders with optimal defaults (Floyd-Steinberg dithering, Monochrome mode,
/// automatic Otsu thresholding).
///
/// This is equivalent to:
/// ```no_run
/// # use dotmax::image::ImageRenderer;
/// # use std::path::Path;
/// # fn main() -> Result<(), dotmax::DotmaxError> {
/// # let path = Path::new("image.png");
/// let grid = ImageRenderer::new()
///     .load_from_path(path)?
///     .resize_to_terminal()?
///     .render()?;
/// # Ok(())
/// # }
/// ```
///
/// # Arguments
///
/// * `path` - Path to the image file
///
/// # Returns
///
/// Returns a [`BrailleGrid`] ready for terminal rendering.
///
/// # Errors
///
/// Returns an error if:
/// - Image file cannot be loaded
/// - Image format is not supported
/// - Rendering pipeline fails
///
/// # Examples
///
/// ```no_run
/// use dotmax::image::render_image_simple;
/// use std::path::Path;
///
/// # fn main() -> Result<(), dotmax::DotmaxError> {
/// let grid = render_image_simple(Path::new("logo.png"))?;
/// // Grid is now ready for rendering via TerminalRenderer
/// let unicode = grid.to_unicode_grid();
/// # Ok(())
/// # }
/// ```
#[instrument]
pub fn render_image_simple(path: &Path) -> Result<BrailleGrid, DotmaxError> {
    info!("Simple render from {:?}", path);
    ImageRenderer::new()
        .load_from_path(path)?
        .resize_to_terminal()?
        .render()
}

/// Detects the current terminal size.
///
/// Uses `crossterm::terminal::size()` to get terminal dimensions. If detection fails,
/// returns a default of 80×24 cells (standard VT100 terminal size).
///
/// # Returns
///
/// Returns `(width, height)` in terminal cells.
///
/// # Examples
///
/// ```
/// use dotmax::image::detect_terminal_size;
///
/// let (width, height) = detect_terminal_size();
/// println!("Terminal size: {}x{}", width, height);
/// ```
pub fn detect_terminal_size() -> (usize, usize) {
    match crossterm::terminal::size() {
        Ok((cols, rows)) => {
            debug!("Detected terminal size: {}x{} cells", cols, rows);
            (cols as usize, rows as usize)
        }
        Err(e) => {
            debug!(
                "Terminal size detection failed ({}), using default 80x24",
                e
            );
            (80, 24)
        }
    }
}
