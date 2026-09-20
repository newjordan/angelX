//! Temporal coherence algorithms for video/animation playback.
//!
//! This module provides algorithms to reduce visual flicker when rendering
//! sequential frames (video, webcam, animations) to braille. The core problem
//! is that small per-frame variations in intensity cause dots to rapidly
//! toggle on/off, creating distracting flicker.
//!
//! # Algorithms
//!
//! Three complementary approaches are provided:
//!
//! 1. **Hysteresis Thresholding** ([`HysteresisFilter`]): Uses two thresholds
//!    with memory - pixels only switch state when crossing the opposite threshold.
//!    Very effective for reducing edge noise.
//!
//! 2. **Frame Blending** ([`FrameBlender`]): Blends current frame with previous
//!    frames using exponential moving average. Smooths transitions but adds latency.
//!
//! 3. **Dot-Level Temporal Filtering** ([`DotTemporalFilter`]): Applies IIR filter
//!    to individual braille dot decisions. Fine-grained control over dot stability.
//!
//! # Usage
//!
//! For most use cases, the [`TemporalCoherence`] struct provides a unified API
//! that combines all algorithms with sensible defaults:
//!
//! ```
//! use dotmax::image::temporal::{TemporalCoherence, TemporalConfig};
//!
//! // Create with default settings (hysteresis only)
//! let mut coherence = TemporalCoherence::new(TemporalConfig::default());
//!
//! // Or use preset for video playback
//! let mut coherence = TemporalCoherence::new(TemporalConfig::video());
//!
//! // Process each frame
//! // let stabilized = coherence.process_grayscale(&grayscale_frame);
//! ```
//!
//! # Performance
//!
//! All algorithms are designed for real-time video processing:
//! - Memory: O(width × height) for state buffers
//! - Time: O(width × height) per frame
//! - Target: <1ms overhead for 1080p frames

use image::GrayImage;

// ============================================================================
// Configuration Types
// ============================================================================

/// Configuration for temporal coherence processing.
///
/// Combines settings for all temporal stabilization algorithms. Use the preset
/// methods for common use cases, or customize individual parameters.
///
/// # Presets
///
/// - [`TemporalConfig::default()`]: Hysteresis only (minimal latency)
/// - [`TemporalConfig::video()`]: Balanced for video playback
/// - [`TemporalConfig::webcam()`]: Aggressive smoothing for noisy webcams
/// - [`TemporalConfig::animation()`]: Gentle smoothing for GIF/APNG
///
/// # Examples
///
/// ```
/// use dotmax::image::temporal::TemporalConfig;
///
/// // Default: hysteresis only
/// let config = TemporalConfig::default();
///
/// // Custom configuration
/// let config = TemporalConfig {
///     hysteresis_enabled: true,
///     hysteresis_margin: 15,
///     frame_blend_enabled: true,
///     frame_blend_alpha: 0.7,
///     dot_filter_enabled: false,
///     dot_filter_alpha: 0.5,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporalConfig {
    /// Enable hysteresis thresholding (dual-threshold with memory).
    pub hysteresis_enabled: bool,

    /// Margin for hysteresis thresholding (0-127).
    ///
    /// Creates a "dead zone" around the threshold where pixels don't switch:
    /// - To turn ON: pixel must exceed `threshold + margin`
    /// - To turn OFF: pixel must drop below `threshold - margin`
    ///
    /// Higher values = more stability but less responsiveness.
    /// Recommended range: 5-20.
    pub hysteresis_margin: u8,

    /// Enable frame blending (exponential moving average).
    pub frame_blend_enabled: bool,

    /// Alpha value for frame blending (0.0-1.0).
    ///
    /// Controls how much of the current frame vs. history is used:
    /// - 1.0 = current frame only (no blending)
    /// - 0.5 = equal mix of current and history
    /// - 0.0 = history only (frozen)
    ///
    /// Recommended range: 0.6-0.9.
    pub frame_blend_alpha: f32,

    /// Enable dot-level temporal filtering.
    pub dot_filter_enabled: bool,

    /// Alpha value for dot-level filtering (0.0-1.0).
    ///
    /// Controls how quickly dots respond to changes:
    /// - 1.0 = immediate response (no filtering)
    /// - 0.5 = smooth transitions over several frames
    /// - 0.0 = dots never change (frozen)
    ///
    /// Recommended range: 0.3-0.7.
    pub dot_filter_alpha: f32,
}

impl Default for TemporalConfig {
    /// Default configuration: hysteresis only.
    ///
    /// Provides flicker reduction with minimal latency, suitable for
    /// most use cases.
    fn default() -> Self {
        Self {
            hysteresis_enabled: true,
            hysteresis_margin: 10,
            frame_blend_enabled: false,
            frame_blend_alpha: 0.8,
            dot_filter_enabled: false,
            dot_filter_alpha: 0.5,
        }
    }
}

impl TemporalConfig {
    /// Preset for video file playback.
    ///
    /// Balanced settings with hysteresis and light frame blending.
    /// Good for most video content.
    #[must_use]
    pub const fn video() -> Self {
        Self {
            hysteresis_enabled: true,
            hysteresis_margin: 12,
            frame_blend_enabled: true,
            frame_blend_alpha: 0.85,
            dot_filter_enabled: false,
            dot_filter_alpha: 0.5,
        }
    }

    /// Preset for webcam capture.
    ///
    /// More aggressive smoothing to handle sensor noise and
    /// lighting variations. Higher latency but much smoother output.
    #[must_use]
    pub const fn webcam() -> Self {
        Self {
            hysteresis_enabled: true,
            hysteresis_margin: 15,
            frame_blend_enabled: true,
            frame_blend_alpha: 0.7,
            dot_filter_enabled: true,
            dot_filter_alpha: 0.6,
        }
    }

    /// Preset for animations (GIF, APNG).
    ///
    /// Light smoothing that preserves animation crispness while
    /// reducing noise in compressed frames.
    #[must_use]
    pub const fn animation() -> Self {
        Self {
            hysteresis_enabled: true,
            hysteresis_margin: 8,
            frame_blend_enabled: false,
            frame_blend_alpha: 0.9,
            dot_filter_enabled: false,
            dot_filter_alpha: 0.7,
        }
    }

    /// Disables all temporal processing.
    ///
    /// Use this to compare with/without temporal coherence, or when
    /// processing static images where temporal filtering isn't needed.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            hysteresis_enabled: false,
            hysteresis_margin: 0,
            frame_blend_enabled: false,
            frame_blend_alpha: 1.0,
            dot_filter_enabled: false,
            dot_filter_alpha: 1.0,
        }
    }
}

// ============================================================================
// Hysteresis Thresholding
// ============================================================================

/// Hysteresis thresholding filter for temporal stability.
///
/// Uses two thresholds (high/low) with memory: pixels only switch state when
/// crossing the opposite threshold. This creates a "dead zone" that prevents
/// rapid toggling from minor intensity fluctuations.
///
/// # Algorithm
///
/// For a given threshold T and margin M:
/// - High threshold: T + M (pixel must exceed this to turn ON)
/// - Low threshold: T - M (pixel must drop below this to turn OFF)
///
/// A pixel that is currently:
/// - ON stays ON until it drops below (T - M)
/// - OFF stays OFF until it exceeds (T + M)
///
/// # Examples
///
/// ```
/// use dotmax::image::temporal::HysteresisFilter;
/// use image::GrayImage;
///
/// let mut filter = HysteresisFilter::new(10);
///
/// // First frame - no history, uses standard threshold
/// let frame1 = GrayImage::new(100, 100);
/// let result1 = filter.apply(&frame1, 128);
///
/// // Subsequent frames use hysteresis
/// let frame2 = GrayImage::new(100, 100);
/// let result2 = filter.apply(&frame2, 128);
/// ```
#[derive(Debug, Clone)]
pub struct HysteresisFilter {
    /// Previous frame's binary state (Some = has history, None = first frame).
    previous_state: Option<Vec<bool>>,
    /// Margin for hysteresis (creates dead zone).
    margin: u8,
    /// Dimensions of previous frame (for resize detection).
    dimensions: Option<(u32, u32)>,
}

impl HysteresisFilter {
    /// Creates a new hysteresis filter with the specified margin.
    ///
    /// # Arguments
    ///
    /// * `margin` - Dead zone size around threshold (0-127)
    #[must_use]
    pub const fn new(margin: u8) -> Self {
        Self {
            previous_state: None,
            margin,
            dimensions: None,
        }
    }

    /// Applies hysteresis thresholding to a grayscale frame.
    ///
    /// # Arguments
    ///
    /// * `frame` - Input grayscale image
    /// * `threshold` - Base threshold value (0-255)
    ///
    /// # Returns
    ///
    /// Binary image where white (255) = on, black (0) = off.
    pub fn apply(&mut self, frame: &GrayImage, threshold: u8) -> GrayImage {
        let width = frame.width();
        let height = frame.height();
        let pixel_count = (width * height) as usize;

        // Check for dimension change (invalidates history)
        if self.dimensions != Some((width, height)) {
            self.previous_state = None;
            self.dimensions = Some((width, height));
        }

        // Calculate thresholds with saturation
        let high_thresh = threshold.saturating_add(self.margin);
        let low_thresh = threshold.saturating_sub(self.margin);

        // Create output buffer
        let mut new_state = Vec::with_capacity(pixel_count);
        let mut output = GrayImage::new(width, height);

        match &self.previous_state {
            None => {
                // First frame: use standard threshold, initialize state
                for (i, pixel) in frame.pixels().enumerate() {
                    let is_on = pixel.0[0] >= threshold;
                    new_state.push(is_on);
                    let x = (i % width as usize) as u32;
                    let y = (i / width as usize) as u32;
                    output.put_pixel(x, y, image::Luma([if is_on { 255 } else { 0 }]));
                }
            }
            Some(prev) => {
                // Subsequent frames: apply hysteresis
                for (i, pixel) in frame.pixels().enumerate() {
                    let value = pixel.0[0];
                    let was_on = prev.get(i).copied().unwrap_or(false);

                    // Hysteresis logic:
                    // - If was ON, stay ON unless drops below low_thresh
                    // - If was OFF, stay OFF unless exceeds high_thresh
                    let is_on = if was_on {
                        value >= low_thresh
                    } else {
                        value > high_thresh
                    };

                    new_state.push(is_on);
                    let x = (i % width as usize) as u32;
                    let y = (i / width as usize) as u32;
                    output.put_pixel(x, y, image::Luma([if is_on { 255 } else { 0 }]));
                }
            }
        }

        self.previous_state = Some(new_state);
        output
    }

    /// Resets the filter state (clears history).
    pub fn reset(&mut self) {
        self.previous_state = None;
        self.dimensions = None;
    }

    /// Sets the hysteresis margin.
    pub fn set_margin(&mut self, margin: u8) {
        self.margin = margin;
    }

    /// Returns the current margin.
    #[must_use]
    pub const fn margin(&self) -> u8 {
        self.margin
    }
}

// ============================================================================
// Frame Blending
// ============================================================================

/// Frame blending filter using exponential moving average.
///
/// Smooths frame transitions by blending the current frame with accumulated
/// history. Reduces flicker at the cost of some motion blur/latency.
///
/// # Algorithm
///
/// For each pixel:
/// ```text
/// output = alpha * current + (1 - alpha) * history
/// ```
///
/// Where alpha controls the blend ratio (1.0 = current only, 0.0 = history only).
///
/// # Examples
///
/// ```
/// use dotmax::image::temporal::FrameBlender;
/// use image::GrayImage;
///
/// let mut blender = FrameBlender::new(0.8);
///
/// // Process frames
/// let frame1 = GrayImage::new(100, 100);
/// let blended1 = blender.blend(&frame1);
///
/// let frame2 = GrayImage::new(100, 100);
/// let blended2 = blender.blend(&frame2);
/// ```
#[derive(Debug, Clone)]
pub struct FrameBlender {
    /// Accumulated frame history (floating point for precision).
    history: Option<Vec<f32>>,
    /// Blend alpha (0.0-1.0, higher = more current frame).
    alpha: f32,
    /// Dimensions of history buffer.
    dimensions: Option<(u32, u32)>,
}

impl FrameBlender {
    /// Creates a new frame blender with the specified alpha.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Blend ratio (0.0-1.0). Higher values = more responsive,
    ///   lower values = smoother but more latency.
    #[must_use]
    pub fn new(alpha: f32) -> Self {
        Self {
            history: None,
            alpha: alpha.clamp(0.0, 1.0),
            dimensions: None,
        }
    }

    /// Blends a frame with the accumulated history.
    ///
    /// # Arguments
    ///
    /// * `frame` - Input grayscale image
    ///
    /// # Returns
    ///
    /// Blended grayscale image.
    pub fn blend(&mut self, frame: &GrayImage) -> GrayImage {
        let width = frame.width();
        let height = frame.height();
        let pixel_count = (width * height) as usize;

        // Check for dimension change (invalidates history)
        if self.dimensions != Some((width, height)) {
            self.history = None;
            self.dimensions = Some((width, height));
        }

        let mut output = GrayImage::new(width, height);

        match &mut self.history {
            None => {
                // First frame: initialize history
                let mut hist = Vec::with_capacity(pixel_count);
                for (i, pixel) in frame.pixels().enumerate() {
                    let value = f32::from(pixel.0[0]);
                    hist.push(value);
                    let x = (i % width as usize) as u32;
                    let y = (i / width as usize) as u32;
                    output.put_pixel(x, y, image::Luma([pixel.0[0]]));
                }
                self.history = Some(hist);
            }
            Some(hist) => {
                // Subsequent frames: blend with history
                let inv_alpha = 1.0 - self.alpha;
                for (i, pixel) in frame.pixels().enumerate() {
                    let current = f32::from(pixel.0[0]);
                    let prev = hist.get(i).copied().unwrap_or(current);

                    // Exponential moving average
                    let blended = self.alpha * current + inv_alpha * prev;
                    hist[i] = blended;

                    let x = (i % width as usize) as u32;
                    let y = (i / width as usize) as u32;
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    output.put_pixel(x, y, image::Luma([blended.round() as u8]));
                }
            }
        }

        output
    }

    /// Resets the blender state (clears history).
    pub fn reset(&mut self) {
        self.history = None;
        self.dimensions = None;
    }

    /// Sets the blend alpha.
    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// Returns the current alpha.
    #[must_use]
    pub const fn alpha(&self) -> f32 {
        self.alpha
    }
}

// ============================================================================
// Dot-Level Temporal Filtering
// ============================================================================

/// Dot-level temporal filter for braille output.
///
/// Applies IIR (infinite impulse response) filtering to individual braille
/// dot decisions. Unlike frame blending which operates on grayscale pixels,
/// this operates on the final binary dot states for finer control.
///
/// # Algorithm
///
/// For each dot, maintains a "confidence" value (0.0-1.0):
/// - When dot should be ON: confidence increases toward 1.0
/// - When dot should be OFF: confidence decreases toward 0.0
/// - Dot is displayed as ON when confidence >= 0.5
///
/// The alpha parameter controls how quickly confidence changes.
///
/// # Examples
///
/// ```
/// use dotmax::image::temporal::DotTemporalFilter;
///
/// let mut filter = DotTemporalFilter::new(0.5, 100, 50);
///
/// // Process each frame's binary data
/// let binary_dots: Vec<bool> = vec![false; 100 * 50];
/// let filtered = filter.filter(&binary_dots);
/// ```
#[derive(Debug, Clone)]
pub struct DotTemporalFilter {
    /// Confidence values for each dot (0.0-1.0).
    confidence: Option<Vec<f32>>,
    /// Filter alpha (0.0-1.0, higher = faster response).
    alpha: f32,
    /// Grid width in dots.
    width: usize,
    /// Grid height in dots.
    height: usize,
}

impl DotTemporalFilter {
    /// Creates a new dot temporal filter.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Filter responsiveness (0.0-1.0)
    /// * `width` - Grid width in dots
    /// * `height` - Grid height in dots
    #[must_use]
    pub fn new(alpha: f32, width: usize, height: usize) -> Self {
        Self {
            confidence: None,
            alpha: alpha.clamp(0.0, 1.0),
            width,
            height,
        }
    }

    /// Filters a frame of binary dot values.
    ///
    /// # Arguments
    ///
    /// * `dots` - Current frame's dot states (true = on, false = off)
    ///
    /// # Returns
    ///
    /// Filtered dot states.
    pub fn filter(&mut self, dots: &[bool]) -> Vec<bool> {
        let expected_len = self.width * self.height;

        // Initialize confidence if needed
        if self.confidence.is_none()
            || self
                .confidence
                .as_ref()
                .is_some_and(|c| c.len() != expected_len)
        {
            // Initialize all dots to 0.5 (neutral)
            self.confidence = Some(vec![0.5; expected_len]);
        }

        let conf = self.confidence.as_mut().unwrap();
        let inv_alpha = 1.0 - self.alpha;

        dots.iter()
            .enumerate()
            .map(|(i, &is_on)| {
                if i < conf.len() {
                    // Update confidence with IIR filter
                    let target = if is_on { 1.0 } else { 0.0 };
                    conf[i] = self.alpha * target + inv_alpha * conf[i];
                    // Threshold at 0.5
                    conf[i] >= 0.5
                } else {
                    is_on
                }
            })
            .collect()
    }

    /// Resets the filter state.
    pub fn reset(&mut self) {
        self.confidence = None;
    }

    /// Updates the grid dimensions.
    pub fn resize(&mut self, width: usize, height: usize) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.confidence = None;
        }
    }

    /// Sets the filter alpha.
    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// Returns the current alpha.
    #[must_use]
    pub const fn alpha(&self) -> f32 {
        self.alpha
    }
}

// ============================================================================
// Unified Temporal Coherence
// ============================================================================

/// Unified temporal coherence processor.
///
/// Combines hysteresis thresholding, frame blending, and dot-level filtering
/// into a single convenient API. Use this for most temporal stabilization needs.
///
/// # Examples
///
/// ```
/// use dotmax::image::temporal::{TemporalCoherence, TemporalConfig};
/// use image::GrayImage;
///
/// // Create with video preset
/// let mut coherence = TemporalCoherence::new(TemporalConfig::video());
///
/// // Process each frame
/// let frame = GrayImage::new(100, 100);
/// let stabilized = coherence.process_grayscale(&frame, 128);
/// ```
#[derive(Debug, Clone)]
pub struct TemporalCoherence {
    /// Configuration.
    config: TemporalConfig,
    /// Hysteresis filter.
    hysteresis: HysteresisFilter,
    /// Frame blender.
    blender: FrameBlender,
    /// Dot-level filter (created lazily when needed).
    dot_filter: Option<DotTemporalFilter>,
}

impl TemporalCoherence {
    /// Creates a new temporal coherence processor.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for temporal processing
    #[must_use]
    pub fn new(config: TemporalConfig) -> Self {
        Self {
            hysteresis: HysteresisFilter::new(config.hysteresis_margin),
            blender: FrameBlender::new(config.frame_blend_alpha),
            dot_filter: None,
            config,
        }
    }

    /// Processes a grayscale frame through the temporal pipeline.
    ///
    /// Applies enabled algorithms in order:
    /// 1. Frame blending (if enabled)
    /// 2. Hysteresis thresholding (if enabled)
    ///
    /// Note: Dot-level filtering is applied separately via [`process_dots`].
    ///
    /// # Arguments
    ///
    /// * `frame` - Input grayscale image
    /// * `threshold` - Base threshold for binary conversion
    ///
    /// # Returns
    ///
    /// Binary image (white = on, black = off).
    pub fn process_grayscale(&mut self, frame: &GrayImage, threshold: u8) -> GrayImage {
        // Step 1: Frame blending (if enabled)
        let blended = if self.config.frame_blend_enabled {
            self.blender.blend(frame)
        } else {
            frame.clone()
        };

        // Step 2: Hysteresis thresholding (if enabled)
        if self.config.hysteresis_enabled {
            self.hysteresis.apply(&blended, threshold)
        } else {
            // Standard threshold
            let mut output = GrayImage::new(blended.width(), blended.height());
            for (i, pixel) in blended.pixels().enumerate() {
                let is_on = pixel.0[0] >= threshold;
                let x = (i % blended.width() as usize) as u32;
                let y = (i / blended.width() as usize) as u32;
                output.put_pixel(x, y, image::Luma([if is_on { 255 } else { 0 }]));
            }
            output
        }
    }

    /// Applies dot-level temporal filtering to binary dot states.
    ///
    /// Call this after converting to braille dots but before final rendering.
    ///
    /// # Arguments
    ///
    /// * `dots` - Binary dot states (true = on)
    /// * `width` - Grid width in dots
    /// * `height` - Grid height in dots
    ///
    /// # Returns
    ///
    /// Filtered dot states.
    pub fn process_dots(&mut self, dots: &[bool], width: usize, height: usize) -> Vec<bool> {
        if !self.config.dot_filter_enabled {
            return dots.to_vec();
        }

        // Initialize or resize dot filter
        match &mut self.dot_filter {
            Some(filter) => {
                filter.resize(width, height);
            }
            None => {
                self.dot_filter = Some(DotTemporalFilter::new(
                    self.config.dot_filter_alpha,
                    width,
                    height,
                ));
            }
        }

        self.dot_filter.as_mut().unwrap().filter(dots)
    }

    /// Updates the configuration.
    ///
    /// Can be called at any time to adjust parameters.
    pub fn set_config(&mut self, config: TemporalConfig) {
        self.hysteresis.set_margin(config.hysteresis_margin);
        self.blender.set_alpha(config.frame_blend_alpha);
        if let Some(ref mut filter) = self.dot_filter {
            filter.set_alpha(config.dot_filter_alpha);
        }
        self.config = config;
    }

    /// Returns the current configuration.
    #[must_use]
    pub const fn config(&self) -> &TemporalConfig {
        &self.config
    }

    /// Resets all temporal state (clears history).
    ///
    /// Call this when seeking in a video or switching content.
    pub fn reset(&mut self) {
        self.hysteresis.reset();
        self.blender.reset();
        if let Some(ref mut filter) = self.dot_filter {
            filter.reset();
        }
    }
}

// ============================================================================
// Flicker Measurement
// ============================================================================

/// Measures temporal flicker between frames.
///
/// Flicker is quantified as the percentage of dots that changed state
/// between consecutive frames. Lower values indicate more temporal stability.
///
/// # Examples
///
/// ```
/// use dotmax::image::temporal::measure_flicker;
///
/// let frame1: Vec<bool> = vec![true, false, true, false];
/// let frame2: Vec<bool> = vec![true, true, true, false];
///
/// let flicker = measure_flicker(&frame1, &frame2);
/// assert!((flicker - 0.25).abs() < 0.01); // 1 out of 4 dots changed
/// ```
#[must_use]
pub fn measure_flicker(frame1: &[bool], frame2: &[bool]) -> f64 {
    if frame1.is_empty() || frame2.is_empty() {
        return 0.0;
    }

    let len = frame1.len().min(frame2.len());
    let changes = frame1
        .iter()
        .zip(frame2.iter())
        .take(len)
        .filter(|(a, b)| a != b)
        .count();

    changes as f64 / len as f64
}

/// Calculates average flicker across a sequence of frames.
///
/// # Arguments
///
/// * `frames` - Sequence of binary dot states
///
/// # Returns
///
/// Average flicker percentage (0.0-1.0).
#[must_use]
pub fn average_flicker(frames: &[Vec<bool>]) -> f64 {
    if frames.len() < 2 {
        return 0.0;
    }

    let total_flicker: f64 = frames
        .windows(2)
        .map(|pair| measure_flicker(&pair[0], &pair[1]))
        .sum();

    total_flicker / (frames.len() - 1) as f64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_config_presets() {
        let default = TemporalConfig::default();
        assert!(default.hysteresis_enabled);
        assert!(!default.frame_blend_enabled);

        let video = TemporalConfig::video();
        assert!(video.hysteresis_enabled);
        assert!(video.frame_blend_enabled);

        let webcam = TemporalConfig::webcam();
        assert!(webcam.dot_filter_enabled);

        let disabled = TemporalConfig::disabled();
        assert!(!disabled.hysteresis_enabled);
        assert!(!disabled.frame_blend_enabled);
        assert!(!disabled.dot_filter_enabled);
    }

    #[test]
    fn test_hysteresis_filter_first_frame() {
        let mut filter = HysteresisFilter::new(10);
        let frame = GrayImage::from_fn(4, 4, |x, _| image::Luma([if x < 2 { 100 } else { 150 }]));

        let result = filter.apply(&frame, 128);

        // First frame uses standard threshold (128)
        // Pixels with value 100 -> OFF (< 128)
        // Pixels with value 150 -> ON (>= 128)
        assert_eq!(result.get_pixel(0, 0).0[0], 0); // 100 < 128 -> OFF
        assert_eq!(result.get_pixel(2, 0).0[0], 255); // 150 >= 128 -> ON
    }

    #[test]
    fn test_hysteresis_filter_stability() {
        let mut filter = HysteresisFilter::new(20);

        // First frame: pixel at 140 (above 128) -> ON
        let frame1 = GrayImage::from_fn(1, 1, |_, _| image::Luma([140]));
        let result1 = filter.apply(&frame1, 128);
        assert_eq!(result1.get_pixel(0, 0).0[0], 255); // ON

        // Second frame: pixel drops to 115 (below 128 but above low_thresh 108)
        // Should stay ON due to hysteresis
        let frame2 = GrayImage::from_fn(1, 1, |_, _| image::Luma([115]));
        let result2 = filter.apply(&frame2, 128);
        assert_eq!(result2.get_pixel(0, 0).0[0], 255); // Still ON (hysteresis)

        // Third frame: pixel drops to 105 (below low_thresh 108)
        // Should turn OFF
        let frame3 = GrayImage::from_fn(1, 1, |_, _| image::Luma([105]));
        let result3 = filter.apply(&frame3, 128);
        assert_eq!(result3.get_pixel(0, 0).0[0], 0); // Now OFF
    }

    #[test]
    fn test_frame_blender() {
        let mut blender = FrameBlender::new(0.5);

        // First frame: all 100
        let frame1 = GrayImage::from_fn(2, 2, |_, _| image::Luma([100]));
        let result1 = blender.blend(&frame1);
        assert_eq!(result1.get_pixel(0, 0).0[0], 100);

        // Second frame: all 200
        // With alpha=0.5: output = 0.5*200 + 0.5*100 = 150
        let frame2 = GrayImage::from_fn(2, 2, |_, _| image::Luma([200]));
        let result2 = blender.blend(&frame2);
        assert_eq!(result2.get_pixel(0, 0).0[0], 150);
    }

    #[test]
    fn test_dot_temporal_filter() {
        let mut filter = DotTemporalFilter::new(0.5, 4, 1);

        // First frame: [OFF, OFF, OFF, OFF]
        let frame1 = vec![false, false, false, false];
        let result1 = filter.filter(&frame1);
        // Initial confidence is 0.5, target 0.0 -> new confidence = 0.5*0 + 0.5*0.5 = 0.25
        // 0.25 < 0.5 -> all OFF
        assert!(result1.iter().all(|&d| !d));

        // Second frame: [ON, ON, ON, ON]
        let frame2 = vec![true, true, true, true];
        let result2 = filter.filter(&frame2);
        // confidence was 0.25, target 1.0 -> new = 0.5*1.0 + 0.5*0.25 = 0.625
        // 0.625 >= 0.5 -> all ON
        assert!(result2.iter().all(|&d| d));
    }

    #[test]
    fn test_measure_flicker() {
        let frame1 = vec![true, false, true, false];
        let frame2 = vec![true, true, true, false];

        // 1 out of 4 changed
        let flicker = measure_flicker(&frame1, &frame2);
        assert!((flicker - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_average_flicker() {
        let frames = vec![
            vec![true, false, true, false],
            vec![true, true, true, false], // 1 change from frame 0 (25%)
            vec![false, true, false, false], // 2 changes from frame 1 (50%)
        ];

        // Average: (0.25 + 0.50) / 2 = 0.375
        let avg = average_flicker(&frames);
        assert!((avg - 0.375).abs() < 0.001);
    }

    #[test]
    fn test_temporal_coherence_full_pipeline() {
        let config = TemporalConfig {
            hysteresis_enabled: true,
            hysteresis_margin: 10,
            frame_blend_enabled: true,
            frame_blend_alpha: 0.8,
            dot_filter_enabled: false,
            dot_filter_alpha: 0.5,
        };

        let mut coherence = TemporalCoherence::new(config);

        let frame = GrayImage::from_fn(10, 10, |x, _| image::Luma([if x < 5 { 100 } else { 150 }]));

        let result = coherence.process_grayscale(&frame, 128);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 10);
    }
}
