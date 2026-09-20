//! Video playback support using FFmpeg.
//!
//! This module provides [`VideoPlayer`] for video file playback, implementing
//! the [`MediaPlayer`] trait for integration with the universal media system.
//!
//! # Requirements
//!
//! This module requires:
//! 1. The `video` feature flag enabled in Cargo.toml
//! 2. FFmpeg libraries installed on the system
//!
//! # Supported Formats
//!
//! - **Containers**: MP4, MKV, AVI, WebM, MOV
//! - **Codecs**: H.264, H.265/HEVC, VP9, AV1, and others supported by FFmpeg
//!
//! # Examples
//!
//! ## Basic Playback
//!
//! ```no_run
//! use dotmax::media::{VideoPlayer, MediaPlayer};
//! use std::time::Duration;
//!
//! let mut player = VideoPlayer::new("video.mp4")?;
//! println!("Video: {}x{} @ {:.2} fps", player.width(), player.height(), player.fps());
//!
//! while let Some(result) = player.next_frame() {
//!     let (grid, delay) = result?;
//!     // Render grid and wait for delay
//! }
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! ## Using as MediaPlayer Trait Object
//!
//! ```no_run
//! use dotmax::media::{VideoPlayer, MediaPlayer};
//!
//! let player: Box<dyn MediaPlayer> = Box::new(VideoPlayer::new("video.mp4")?);
//! println!("Frame count: {:?}", player.frame_count());
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! # Architecture
//!
//! `VideoPlayer` uses FFmpeg (via `ffmpeg-next` crate) for video decoding:
//!
//! 1. **Demuxing**: `format::input()` opens the container and extracts video stream
//! 2. **Decoding**: `codec::decoder::Video` decodes frames to raw pixel data
//! 3. **Scaling**: `software::scaling::Context` converts to RGB24 format
//! 4. **Rendering**: `ImageRenderer` converts RGB data to `BrailleGrid`
//!
//! # Thread Safety
//!
//! `VideoPlayer` is `Send` but not `Sync`. It can be moved between threads
//! but should not be accessed from multiple threads simultaneously.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::image::temporal::{TemporalCoherence, TemporalConfig};
use crate::image::{ColorMode, DitheringMethod, ImageRenderer};
use crate::{BrailleGrid, DotmaxError, Result};

use super::MediaPlayer;

extern crate ffmpeg_next as ffmpeg;

use ffmpeg::format::{input, input_with_dictionary, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};
use ffmpeg::util::frame::video::Video as VideoFrame;

// Wrapper to make ScalingContext Send-safe
// Safety: The ScalingContext is only accessed from the VideoPlayer's methods.
// VideoPlayer is Send but not Sync, meaning it can be moved between threads
// but cannot be shared. This ensures the scaler is only ever accessed from
// one thread at a time.
struct SendableScaler(ScalingContext);

// SAFETY: ScalingContext contains raw pointers to FFmpeg structures.
// We ensure thread safety because:
// 1. VideoPlayer owns SendableScaler exclusively (no sharing)
// 2. VideoPlayer is Send but not Sync (can move, can't share)
// 3. The scaler is only used within VideoPlayer's methods
// 4. Only one thread can access the scaler at any time
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for SendableScaler {}

/// A decoded braille frame with source-media timing.
#[derive(Debug)]
pub struct TimedFrame {
    /// Rendered terminal-cell grid.
    pub grid: BrailleGrid,
    /// Presentation timestamp in the media timeline.
    pub pts: Duration,
    /// Recommended display duration.
    pub duration: Duration,
}

/// Decoded pixels from the same scaler and clock as the braille player.
/// A terminal-native consumer can skip the lossy grid conversion entirely.
#[derive(Debug)]
pub struct TimedRgbaFrame {
    /// Scaled, interleaved RGBA pixels; no terminal grid conversion has run.
    pub rgba: image::RgbaImage,
    /// Presentation timestamp on the source-media clock.
    pub pts: Duration,
    /// Recommended source-frame display duration.
    pub duration: Duration,
}

// ============================================================================
// VideoPlayer (AC: #1, #2, #3, #4, #5, #6, #7)
// ============================================================================

/// Video file player implementing the [`MediaPlayer`] trait.
///
/// `VideoPlayer` provides frame-by-frame access to video files using FFmpeg
/// for decoding. It supports all major video formats and codecs.
///
/// # Frame Iteration
///
/// Use [`next_frame()`](Self::next_frame) to get frames one at a time:
///
/// ```no_run
/// use dotmax::media::{VideoPlayer, MediaPlayer};
///
/// let mut player = VideoPlayer::new("video.mp4")?;
/// while let Some(result) = player.next_frame() {
///     let (grid, delay) = result?;
///     // Display grid, then sleep for delay
///     std::thread::sleep(delay);
/// }
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Video Properties
///
/// Access video metadata via dedicated methods:
/// - [`width()`](Self::width) / [`height()`](Self::height) - Video dimensions
/// - [`fps()`](Self::fps) - Frame rate
/// - [`duration()`](Self::duration) - Total duration
///
/// # Memory Efficiency
///
/// Frames are decoded on-demand, not all at once. The player maintains
/// a single frame buffer that is reused for each frame, keeping memory
/// usage constant regardless of video length.
pub struct VideoPlayer {
    /// Path to the video file (for reset support and error messages).
    path: PathBuf,

    /// FFmpeg input context (demuxer).
    input_context: ffmpeg::format::context::Input,

    /// Video stream index within the container.
    video_stream_index: usize,

    /// Video decoder.
    decoder: ffmpeg::decoder::Video,

    /// Scaler for converting to RGB24 (wrapped for Send safety).
    scaler: SendableScaler,

    /// Video width in pixels.
    width: u32,

    /// Video height in pixels.
    height: u32,

    /// Frame rate (frames per second).
    fps: f64,

    /// Stream time base used to turn decoded PTS values into wall time.
    time_base: ffmpeg::Rational,

    /// Duration of the video.
    video_duration: Option<Duration>,

    /// Total frame count (estimated from duration and fps).
    estimated_frame_count: Option<usize>,

    /// Current frame index (0-based).
    current_frame: usize,

    /// Whether playback has ended.
    playback_ended: bool,

    /// Terminal dimensions for rendering.
    terminal_width: usize,
    terminal_height: usize,

    /// Reusable frame buffers.
    decoded_frame: VideoFrame,
    rgb_frame: VideoFrame,

    /// Whether we've sent EOF to decoder.
    eof_sent: bool,

    /// Reusable RGB data buffer to avoid per-frame allocations.
    rgb_buffer: Vec<u8>,

    // ========== Render Settings ==========
    /// Dithering algorithm for binary conversion.
    dithering: DitheringMethod,

    /// Manual threshold (0-255) or None for automatic Otsu thresholding.
    threshold: Option<u8>,

    /// Brightness adjustment (1.0 = neutral, >1 = brighter, <1 = darker).
    brightness: f32,

    /// Contrast adjustment (1.0 = neutral, >1 = more contrast).
    contrast: f32,

    /// Gamma correction (1.0 = neutral, <1 = brighter midtones, >1 = darker).
    gamma: f32,

    /// Color mode for rendering (Monochrome, Grayscale, or TrueColor).
    color_mode: ColorMode,

    /// Temporal coherence processor for reducing flicker.
    temporal_coherence: TemporalCoherence,
}

impl std::fmt::Debug for VideoPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoPlayer")
            .field("path", &self.path)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("fps", &self.fps)
            .field("duration", &self.video_duration)
            .field("frame_count", &self.estimated_frame_count)
            .field("current_frame", &self.current_frame)
            .field("dithering", &self.dithering)
            .field("threshold", &self.threshold)
            .field("brightness", &self.brightness)
            .field("contrast", &self.contrast)
            .field("gamma", &self.gamma)
            .field("color_mode", &self.color_mode)
            .finish_non_exhaustive()
    }
}

impl VideoPlayer {
    /// Creates a new `VideoPlayer` from a video file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the video file (MP4, MKV, AVI, WebM, etc.)
    ///
    /// # Errors
    ///
    /// Returns `DotmaxError::VideoError` if:
    /// - The file cannot be opened
    /// - No video stream is found in the container
    /// - The video codec is not supported
    /// - FFmpeg initialization fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    ///
    /// let player = VideoPlayer::new("video.mp4")?;
    /// println!("Video: {}x{} @ {:.2} fps", player.width(), player.height(), player.fps());
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        Self::new_with_policy(path, false)
    }

    /// Open a local, self-contained video. Reject playlists and network
    /// protocols before probing; MOV external track references stay disabled.
    pub fn new_local(path: impl AsRef<Path>) -> Result<Self> {
        Self::new_with_policy(path, true)
    }

    fn new_with_policy(path: impl AsRef<Path>, local_only: bool) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Initialize FFmpeg (safe to call multiple times)
        ffmpeg::init().map_err(|e| DotmaxError::VideoError {
            path: path.clone(),
            message: format!("FFmpeg initialization failed: {e}"),
        })?;

        // Open input file
        let opened = if local_only {
            let mut options = ffmpeg::Dictionary::new();
            options.set("protocol_whitelist", "file");
            options.set("format_whitelist", "mov,matroska,avi");
            options.set("enable_drefs", "0");
            options.set("use_absolute_path", "0");
            input_with_dictionary(&path, options)
        } else {
            input(&path)
        };
        let input_context = opened.map_err(|e| DotmaxError::VideoError {
            path: path.clone(),
            message: format!("Failed to open video file: {e}"),
        })?;

        // Find video stream
        let video_stream =
            input_context
                .streams()
                .best(Type::Video)
                .ok_or_else(|| DotmaxError::VideoError {
                    path: path.clone(),
                    message: "No video stream found in file".to_string(),
                })?;

        let video_stream_index = video_stream.index();
        let time_base = video_stream.time_base();

        // Get stream parameters
        let codec_params = video_stream.parameters();

        // Create decoder
        let context =
            ffmpeg::codec::context::Context::from_parameters(codec_params).map_err(|e| {
                DotmaxError::VideoError {
                    path: path.clone(),
                    message: format!("Failed to create codec context: {e}"),
                }
            })?;

        let decoder = context
            .decoder()
            .video()
            .map_err(|e| DotmaxError::VideoError {
                path: path.clone(),
                message: format!("Failed to create video decoder: {e}"),
            })?;

        let width = decoder.width();
        let height = decoder.height();
        if local_only && u64::from(width) * u64::from(height) > 16 * 1024 * 1024 {
            return Err(DotmaxError::VideoError {
                path: path.clone(),
                message: "Local video exceeds the 16-megapixel frame limit".into(),
            });
        }

        // Get frame rate
        let fps = video_stream.avg_frame_rate();
        let fps = if fps.denominator() != 0 {
            f64::from(fps.numerator()) / f64::from(fps.denominator())
        } else {
            30.0 // Default to 30 fps if unknown
        };

        // Get duration (fix: multiply before divide to preserve precision)
        let video_duration = if input_context.duration() > 0 {
            // AV_TIME_BASE is typically 1_000_000, so duration is already in microseconds
            // but we need to convert from AV_TIME_BASE units to microseconds
            let duration_us =
                (input_context.duration() as u64 * 1_000_000) / ffmpeg::ffi::AV_TIME_BASE as u64;
            Some(Duration::from_micros(duration_us))
        } else {
            None
        };

        // Estimate frame count
        let estimated_frame_count = video_duration.map(|d| (d.as_secs_f64() * fps) as usize);

        // Get terminal size for rendering
        let (terminal_width, terminal_height) = crossterm::terminal::size()
            .map(|(w, h)| (w as usize, h as usize))
            .unwrap_or((80, 24));
        let (terminal_width, terminal_height) = if local_only {
            (terminal_width.clamp(1, 192), terminal_height.clamp(1, 64))
        } else {
            (terminal_width, terminal_height)
        };

        // Calculate target pixel dimensions for braille grid
        // Each braille cell is 2 pixels wide and 4 pixels tall
        let target_pixel_width = (terminal_width * 2) as u32;
        let target_pixel_height = (terminal_height * 4) as u32;

        // Create scaler that ALSO resizes to terminal dimensions
        // This is much faster than resizing in Rust later
        let scaler = SendableScaler(
            ScalingContext::get(
                decoder.format(),
                width,
                height,
                Pixel::RGB24,
                target_pixel_width,
                target_pixel_height,
                Flags::BILINEAR,
            )
            .map_err(|e| DotmaxError::VideoError {
                path: path.clone(),
                message: format!("Failed to create scaler: {e}"),
            })?,
        );

        tracing::info!(
            "Opened video: {:?}, {}x{} @ {:.2} fps, time_base: {:?}, duration: {:?}",
            path,
            width,
            height,
            fps,
            time_base,
            video_duration
        );

        // Pre-allocate RGB buffer for frame data (avoids per-frame allocation)
        let rgb_buffer_size = (target_pixel_width * target_pixel_height * 3) as usize;

        Ok(Self {
            path,
            input_context,
            video_stream_index,
            decoder,
            scaler,
            width,
            height,
            fps,
            time_base,
            video_duration,
            estimated_frame_count,
            current_frame: 0,
            playback_ended: false,
            terminal_width,
            terminal_height,
            decoded_frame: VideoFrame::empty(),
            rgb_frame: VideoFrame::empty(),
            eof_sent: false,
            rgb_buffer: vec![0u8; rgb_buffer_size],
            // Render settings - sensible defaults
            // Use Bayer dithering for video - it's deterministic (same input = same output)
            // which reduces temporal flicker compared to error-diffusion methods like Floyd-Steinberg
            dithering: DitheringMethod::Bayer,
            threshold: None, // Auto (Otsu)
            brightness: 1.0,
            contrast: 1.0,
            gamma: 1.0,
            color_mode: ColorMode::Monochrome,
            // Temporal coherence with video preset
            temporal_coherence: TemporalCoherence::new(TemporalConfig::video()),
        })
    }

    /// Open a player already scaled for a terminal-cell viewport.
    pub fn new_for_viewport(path: impl AsRef<Path>, width: usize, height: usize) -> Result<Self> {
        let mut player = Self::new(path)?;
        MediaPlayer::handle_resize(&mut player, width.max(1), height.max(1));
        Ok(player)
    }

    /// Seek to an absolute media position and reset decoder/temporal state.
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        let micros = position.as_micros().min(i64::MAX as u128) as i64;
        self.input_context
            .seek(micros, std::ops::RangeFull)
            .map_err(|error| DotmaxError::VideoError {
                path: self.path.clone(),
                message: format!("seek failed: {error}"),
            })?;
        self.decoder.flush();
        self.current_frame = (position.as_secs_f64() * self.fps.max(0.0)) as usize;
        self.playback_ended = false;
        self.eof_sent = false;
        self.temporal_coherence.reset();
        Ok(())
    }

    /// Decode the next frame while preserving its source presentation time.
    pub fn next_timed_frame(&mut self) -> Result<Option<TimedFrame>> {
        let Some(pts) = self.decode_next_pts()? else {
            return Ok(None);
        };
        Ok(Some(TimedFrame {
            grid: self.frame_to_grid()?,
            pts,
            duration: self.frame_delay(),
        }))
    }

    /// Decode once and expose scaled RGBA without rendering braille.
    pub fn next_timed_rgba_frame(&mut self) -> Result<Option<TimedRgbaFrame>> {
        let Some(pts) = self.decode_next_pts()? else {
            return Ok(None);
        };
        Ok(Some(TimedRgbaFrame {
            rgba: self.frame_to_rgba()?,
            pts,
            duration: self.frame_delay(),
        }))
    }

    fn decode_next_pts(&mut self) -> Result<Option<Duration>> {
        match self.decode_next_frame() {
            Some(Ok(())) => {}
            Some(Err(error)) => return Err(error),
            None => return Ok(None),
        }
        let pts = self
            .decoded_frame
            .pts()
            .filter(|pts| *pts >= 0)
            .map(|pts| {
                let seconds = pts as f64 * f64::from(self.time_base.numerator())
                    / f64::from(self.time_base.denominator().max(1));
                Duration::from_secs_f64(seconds.max(0.0))
            })
            .unwrap_or_else(|| {
                Duration::from_secs_f64(self.current_frame as f64 / self.fps.max(1.0))
            });
        self.current_frame += 1;
        Ok(Some(pts))
    }

    /// Decode and discard one source frame without paying the braille conversion
    /// cost. Playback workers use this to sample high-frame-rate media without
    /// stretching its wall-clock duration.
    pub fn skip_next_frame(&mut self) -> Result<Option<Duration>> {
        self.decode_next_pts()
    }

    /// Returns the video width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the video height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the frame rate (frames per second).
    #[must_use]
    pub const fn fps(&self) -> f64 {
        self.fps
    }

    /// Returns the video duration, if known.
    #[must_use]
    pub const fn duration(&self) -> Option<Duration> {
        self.video_duration
    }

    /// Returns the current frame index (0-based).
    #[must_use]
    pub const fn current_frame_index(&self) -> usize {
        self.current_frame
    }

    // ========== Render Settings Builder Methods ==========

    /// Sets the dithering algorithm for binary image conversion.
    ///
    /// Dithering affects how grayscale images are converted to the binary
    /// (on/off) representation needed for braille characters.
    ///
    /// # Arguments
    ///
    /// * `method` - The dithering algorithm to use
    ///
    /// # Dithering Methods
    ///
    /// - [`DitheringMethod::None`]: Simple thresholding, fastest but lowest quality
    /// - [`DitheringMethod::FloydSteinberg`]: Error diffusion, best quality (default)
    /// - [`DitheringMethod::Bayer`]: Ordered dithering, good for animations
    /// - [`DitheringMethod::Atkinson`]: Classic Mac-style, preserves highlights
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    /// use dotmax::image::DitheringMethod;
    ///
    /// let player = VideoPlayer::new("video.mp4")?
    ///     .dithering(DitheringMethod::Bayer);
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[must_use]
    pub fn dithering(mut self, method: DitheringMethod) -> Self {
        self.dithering = method;
        self
    }

    /// Sets a manual threshold for binary conversion.
    ///
    /// When set to `Some(value)`, pixels brighter than the threshold become
    /// white (on), and darker pixels become black (off). When `None`, automatic
    /// Otsu thresholding is used (default).
    ///
    /// # Arguments
    ///
    /// * `threshold` - Manual threshold (0-255), or None for automatic
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    ///
    /// // Use manual threshold of 128 (mid-gray)
    /// let player = VideoPlayer::new("video.mp4")?
    ///     .threshold(Some(128));
    ///
    /// // Use automatic Otsu thresholding
    /// let player = VideoPlayer::new("video.mp4")?
    ///     .threshold(None);
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[must_use]
    pub fn threshold(mut self, threshold: Option<u8>) -> Self {
        self.threshold = threshold;
        self
    }

    /// Sets the brightness adjustment factor.
    ///
    /// Values greater than 1.0 increase brightness, less than 1.0 decrease it.
    /// The default is 1.0 (no adjustment).
    ///
    /// # Arguments
    ///
    /// * `brightness` - Brightness multiplier (0.1 to 3.0 recommended)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    ///
    /// let player = VideoPlayer::new("dark_video.mp4")?
    ///     .brightness(1.3); // 30% brighter
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[must_use]
    pub fn brightness(mut self, brightness: f32) -> Self {
        self.brightness = brightness;
        self
    }

    /// Sets the contrast adjustment factor.
    ///
    /// Values greater than 1.0 increase contrast, less than 1.0 decrease it.
    /// The default is 1.0 (no adjustment).
    ///
    /// # Arguments
    ///
    /// * `contrast` - Contrast multiplier (0.1 to 3.0 recommended)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    ///
    /// let player = VideoPlayer::new("flat_video.mp4")?
    ///     .contrast(1.2); // 20% more contrast
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[must_use]
    pub fn contrast(mut self, contrast: f32) -> Self {
        self.contrast = contrast;
        self
    }

    /// Sets the gamma correction factor.
    ///
    /// Values less than 1.0 brighten midtones, greater than 1.0 darken them.
    /// The default is 1.0 (no correction).
    ///
    /// # Arguments
    ///
    /// * `gamma` - Gamma value (0.1 to 3.0 recommended)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    ///
    /// let player = VideoPlayer::new("video.mp4")?
    ///     .gamma(0.8); // Brighten midtones
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[must_use]
    pub fn gamma(mut self, gamma: f32) -> Self {
        self.gamma = gamma;
        self
    }

    /// Sets the color mode for rendering.
    ///
    /// Color mode determines how color information is displayed:
    ///
    /// - [`ColorMode::Monochrome`]: Black/white only (default, fastest)
    /// - [`ColorMode::Grayscale`]: 256 shades using ANSI 256-color palette
    /// - [`ColorMode::TrueColor`]: Full RGB color per braille cell (24-bit)
    ///
    /// # Arguments
    ///
    /// * `mode` - The color mode to use
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    /// use dotmax::image::ColorMode;
    ///
    /// let player = VideoPlayer::new("video.mp4")?
    ///     .color_mode(ColorMode::TrueColor);
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[must_use]
    pub fn color_mode(mut self, mode: ColorMode) -> Self {
        self.color_mode = mode;
        self
    }

    // ========== Getters for current render settings ==========

    /// Returns the current dithering method.
    #[must_use]
    pub const fn get_dithering(&self) -> DitheringMethod {
        self.dithering
    }

    /// Returns the current threshold setting.
    #[must_use]
    pub const fn get_threshold(&self) -> Option<u8> {
        self.threshold
    }

    /// Returns the current brightness setting.
    #[must_use]
    pub const fn get_brightness(&self) -> f32 {
        self.brightness
    }

    /// Returns the current contrast setting.
    #[must_use]
    pub const fn get_contrast(&self) -> f32 {
        self.contrast
    }

    /// Returns the current gamma setting.
    #[must_use]
    pub const fn get_gamma(&self) -> f32 {
        self.gamma
    }

    /// Returns the current color mode.
    #[must_use]
    pub const fn get_color_mode(&self) -> ColorMode {
        self.color_mode
    }

    // ========== Mutable setters for runtime adjustment ==========

    /// Updates the dithering method at runtime.
    pub fn set_dithering(&mut self, method: DitheringMethod) {
        self.dithering = method;
    }

    /// Updates the threshold at runtime.
    pub fn set_threshold(&mut self, threshold: Option<u8>) {
        self.threshold = threshold;
    }

    /// Updates the brightness at runtime.
    pub fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness;
    }

    /// Updates the contrast at runtime.
    pub fn set_contrast(&mut self, contrast: f32) {
        self.contrast = contrast;
    }

    /// Updates the gamma at runtime.
    pub fn set_gamma(&mut self, gamma: f32) {
        self.gamma = gamma;
    }

    /// Updates the color mode at runtime.
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
    }

    // ========== Temporal Coherence Settings ==========

    /// Returns a reference to the temporal coherence configuration.
    #[must_use]
    pub fn temporal_config(&self) -> &TemporalConfig {
        self.temporal_coherence.config()
    }

    /// Updates the temporal coherence configuration at runtime.
    ///
    /// # Arguments
    ///
    /// * `config` - New temporal coherence settings
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::VideoPlayer;
    /// use dotmax::image::temporal::TemporalConfig;
    ///
    /// let mut player = VideoPlayer::new("video.mp4")?;
    ///
    /// // Use more aggressive smoothing
    /// player.set_temporal_config(TemporalConfig::webcam());
    ///
    /// // Or disable temporal processing entirely
    /// player.set_temporal_config(TemporalConfig::disabled());
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn set_temporal_config(&mut self, config: TemporalConfig) {
        self.temporal_coherence.set_config(config);
    }

    /// Sets the temporal coherence configuration using builder pattern.
    ///
    /// # Arguments
    ///
    /// * `config` - Temporal coherence configuration
    #[must_use]
    pub fn temporal_coherence(mut self, config: TemporalConfig) -> Self {
        self.temporal_coherence.set_config(config);
        self
    }

    /// Resets temporal coherence state (clears history).
    ///
    /// Call this when seeking in the video to prevent artifacts from
    /// blending unrelated frames.
    pub fn reset_temporal_state(&mut self) {
        self.temporal_coherence.reset();
    }

    /// Decodes the next frame from the video.
    fn decode_next_frame(&mut self) -> Option<Result<()>> {
        if self.playback_ended {
            return None;
        }

        // Try to receive a decoded frame
        loop {
            // First, try to receive from decoder
            match self.decoder.receive_frame(&mut self.decoded_frame) {
                Ok(()) => {
                    // Successfully decoded a frame
                    return Some(Ok(()));
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                    // Decoder needs more data - send another packet
                }
                Err(ffmpeg::Error::Eof) => {
                    // Decoder has no more frames
                    self.playback_ended = true;
                    return None;
                }
                Err(e) => {
                    return Some(Err(DotmaxError::VideoError {
                        path: self.path.clone(),
                        message: format!("Frame decode error: {e}"),
                    }));
                }
            }

            // Read next packet from input and send to decoder
            let mut found_video_packet = false;
            for (stream, packet) in self.input_context.packets() {
                if stream.index() == self.video_stream_index {
                    if let Err(e) = self.decoder.send_packet(&packet) {
                        tracing::warn!("Error sending packet to decoder: {}", e);
                    }
                    found_video_packet = true;
                    break;
                }
            }

            if !found_video_packet {
                // End of file - send EOF to decoder
                if self.eof_sent {
                    // Already sent EOF and no more frames
                    self.playback_ended = true;
                    return None;
                }
                self.eof_sent = true;
                if let Err(e) = self.decoder.send_eof() {
                    tracing::warn!("Error sending EOF to decoder: {}", e);
                }
            }
        }
    }

    /// Converts the decoded frame to a BrailleGrid.
    fn frame_to_rgba(&mut self) -> Result<image::RgbaImage> {
        // Scale to RGB24 at terminal dimensions (done by FFmpeg, very fast)
        self.scaler
            .0
            .run(&self.decoded_frame, &mut self.rgb_frame)
            .map_err(|e| DotmaxError::VideoError {
                path: self.path.clone(),
                message: format!("Frame scaling error: {e}"),
            })?;

        // Get RGB data - frame is already at target size
        let data = self.rgb_frame.data(0);
        let stride = self.rgb_frame.stride(0);
        let target_width = (self.terminal_width * 2) as u32;
        let target_height = (self.terminal_height * 4) as u32;

        // Copy RGB data into reusable buffer (avoids per-frame allocation)
        let expected_size = (target_width * target_height * 3) as usize;
        if self.rgb_buffer.len() != expected_size {
            self.rgb_buffer.resize(expected_size, 0);
        }

        let mut offset = 0;
        for y in 0..target_height {
            let row_start = (y as usize) * stride;
            let row_len = (target_width as usize) * 3;
            self.rgb_buffer[offset..offset + row_len]
                .copy_from_slice(&data[row_start..row_start + row_len]);
            offset += row_len;
        }

        // Create RGB image from pre-scaled frame data (uses buffer, no allocation)
        let img = image::RgbImage::from_raw(target_width, target_height, self.rgb_buffer.clone())
            .ok_or_else(|| DotmaxError::VideoError {
            path: self.path.clone(),
            message: "Failed to create image from frame data".to_string(),
        })?;

        // Convert to RGBA for ImageRenderer
        Ok(image::DynamicImage::ImageRgb8(img).into_rgba8())
    }

    fn frame_to_grid(&mut self) -> Result<BrailleGrid> {
        let rgba_img = self.frame_to_rgba()?;

        // Use ImageRenderer - NO RESIZE needed, frame is already at correct size
        let mut renderer = ImageRenderer::new()
            .load_from_rgba(rgba_img)
            .resize(self.terminal_width, self.terminal_height, false)? // false = don't preserve aspect, already correct
            .dithering(self.dithering)
            .color_mode(self.color_mode);

        // Apply manual threshold if set
        if let Some(t) = self.threshold {
            renderer = renderer.threshold(t);
        }

        // Apply adjustments (these return Result, so we chain with ?)
        if (self.brightness - 1.0).abs() > f32::EPSILON {
            renderer = renderer.brightness(self.brightness)?;
        }
        if (self.contrast - 1.0).abs() > f32::EPSILON {
            renderer = renderer.contrast(self.contrast)?;
        }
        if (self.gamma - 1.0).abs() > f32::EPSILON {
            renderer = renderer.gamma(self.gamma)?;
        }

        let grid = renderer.render()?;

        Ok(grid)
    }

    /// Calculates the delay for the current frame.
    fn frame_delay(&self) -> Duration {
        if self.fps > 0.0 {
            Duration::from_secs_f64(1.0 / self.fps)
        } else {
            Duration::from_millis(33) // ~30 fps default
        }
    }
}

impl MediaPlayer for VideoPlayer {
    /// Returns the next frame and its display duration.
    ///
    /// Decodes video frames on-demand using FFmpeg. Returns `None` when
    /// the video ends.
    fn next_frame(&mut self) -> Option<Result<(BrailleGrid, Duration)>> {
        match self.next_timed_frame() {
            Ok(Some(frame)) => Some(Ok((frame.grid, frame.duration))),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }

    /// Resets playback to the beginning.
    ///
    /// Note: This requires reopening the file as FFmpeg doesn't support
    /// efficient seeking in all formats.
    fn reset(&mut self) {
        // Seek to beginning
        if let Err(e) = self.input_context.seek(0, std::ops::RangeFull) {
            tracing::warn!("Failed to seek to beginning: {}", e);
            // Try reopening the file
            if let Ok(new_player) = Self::new(&self.path) {
                *self = new_player;
                return;
            }
        }

        // Flush decoder
        self.decoder.flush();
        self.current_frame = 0;
        self.playback_ended = false;
        self.eof_sent = false;

        // Reset temporal coherence state (important when seeking/looping)
        self.temporal_coherence.reset();
    }

    /// Returns the estimated total number of frames.
    ///
    /// This is calculated from duration and fps, so may not be exact
    /// for variable frame rate videos.
    fn frame_count(&self) -> Option<usize> {
        self.estimated_frame_count
    }

    /// Returns `Some(1)` as videos don't loop by default.
    fn loop_count(&self) -> Option<u16> {
        Some(1)
    }

    /// Updates terminal dimensions for subsequent frame rendering.
    ///
    /// Call this when the terminal is resized to ensure frames are
    /// rendered at the correct size. This recreates the FFmpeg scaler
    /// with the new target dimensions.
    fn handle_resize(&mut self, width: usize, height: usize) {
        if self.terminal_width == width && self.terminal_height == height {
            return; // No change
        }

        // Calculate new target pixel dimensions
        let target_pixel_width = (width * 2) as u32;
        let target_pixel_height = (height * 4) as u32;

        // Recreate scaler with new dimensions
        match ScalingContext::get(
            self.decoder.format(),
            self.width,
            self.height,
            Pixel::RGB24,
            target_pixel_width,
            target_pixel_height,
            Flags::BILINEAR,
        ) {
            Ok(new_scaler) => {
                self.terminal_width = width;
                self.terminal_height = height;
                self.scaler = SendableScaler(new_scaler);
                // swscale rejects an already-allocated output frame with the
                // old dimensions. Resize the scaler and its output together.
                self.rgb_frame = VideoFrame::empty();
                // Resize the RGB buffer to match
                let rgb_buffer_size = (target_pixel_width * target_pixel_height * 3) as usize;
                self.rgb_buffer.resize(rgb_buffer_size, 0);
                tracing::debug!("VideoPlayer resized to {}x{}", width, height);
            }
            Err(e) => {
                tracing::warn!("Failed to resize video scaler: {}", e);
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Ensure VideoPlayer is Send (required by MediaPlayer trait)
    fn _assert_video_player_send() {
        fn assert_send<T: Send>() {}
        assert_send::<VideoPlayer>();
    }

    #[test]
    fn test_video_player_new_nonexistent() {
        let player = VideoPlayer::new("nonexistent_video.mp4");
        assert!(player.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_video_player_new_invalid_file() {
        // Create a temp file with invalid content
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("invalid_video_test.mp4");

        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(&[0x00, 0x00, 0x00, 0x00]).unwrap();
        drop(file);

        let player = VideoPlayer::new(&temp_file);
        assert!(player.is_err(), "Should fail for invalid video file");

        // Cleanup
        let _ = std::fs::remove_file(&temp_file);
    }

    #[test]
    fn test_video_player_debug() {
        // Just test that Debug impl compiles and doesn't panic
        // We can't easily test with a real video in unit tests
        let debug_output = format!(
            "VideoPlayer path={:?} width={} height={}",
            PathBuf::from("test.mp4"),
            1920,
            1080
        );
        assert!(debug_output.contains("VideoPlayer"));
    }

    // Integration tests with real video files would go in tests/video_tests.rs
    // and require test fixture videos
}
