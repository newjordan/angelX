//! Animated PNG (APNG) playback support.
//!
//! This module provides [`ApngPlayer`] for animated PNG playback, implementing
//! the [`MediaPlayer`] trait for integration with the universal media system.
//!
//! # Features
//!
//! - Frame-by-frame APNG decoding with correct timing
//! - Loop count handling (finite and infinite loops)
//! - Blend operation support (Source, Over) for correct alpha compositing
//! - Dispose operation support (None, Background, Previous)
//! - Memory-efficient streaming decode (frames processed one at a time)
//! - Full alpha transparency support (24-bit color + 8-bit alpha)
//!
//! # Examples
//!
//! ## Basic Playback
//!
//! ```no_run
//! use dotmax::media::{ApngPlayer, MediaPlayer};
//! use std::time::Duration;
//!
//! let mut player = ApngPlayer::new("animation.png")?;
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
//! use dotmax::media::{ApngPlayer, MediaPlayer};
//!
//! let player: Box<dyn MediaPlayer> = Box::new(ApngPlayer::new("animation.png")?);
//! println!("Frame count: {:?}", player.frame_count());
//! # Ok::<(), dotmax::DotmaxError>(())
//! ```
//!
//! # APNG vs GIF
//!
//! | Feature | GIF | APNG |
//! |---------|-----|------|
//! | Colors | 256 (palette) | 16M (24-bit + alpha) |
//! | Transparency | 1-bit (on/off) | 8-bit alpha channel |
//! | Compression | LZW | Deflate (PNG) |
//! | Blend modes | Implicit | Explicit (Source/Over) |

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::image::ImageRenderer;
use crate::{BrailleGrid, DotmaxError, Result};

use super::MediaPlayer;

// ============================================================================
// APNG Frame Types (AC: #3, #6)
// ============================================================================

/// Blend operation for APNG frames.
///
/// Specifies how frame pixels are written into the output buffer.
///
/// # APNG Specification Reference
///
/// | Value | Method | Description |
/// |-------|--------|-------------|
/// | 0 | Source | All pixels replace output buffer completely |
/// | 1 | Over | Alpha composite using Porter-Duff "over" |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendOp {
    /// Source blend - frame pixels replace output buffer completely.
    ///
    /// All color channels and alpha are written directly to the canvas,
    /// replacing whatever was there before.
    #[default]
    Source,

    /// Over blend - alpha composite frame onto canvas.
    ///
    /// Uses Porter-Duff "over" compositing: new pixels are blended with
    /// existing canvas pixels based on alpha values.
    Over,
}

impl From<png::BlendOp> for BlendOp {
    fn from(op: png::BlendOp) -> Self {
        match op {
            png::BlendOp::Source => Self::Source,
            png::BlendOp::Over => Self::Over,
        }
    }
}

/// Dispose operation for APNG frames.
///
/// Specifies how to reset the output buffer after displaying a frame
/// and before rendering the next frame.
///
/// # APNG Specification Reference
///
/// | Value | Method | Description |
/// |-------|--------|-------------|
/// | 0 | None | Leave output buffer unchanged |
/// | 1 | Background | Clear frame region to transparent black |
/// | 2 | Previous | Restore canvas to state before frame |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisposeOp {
    /// No disposal - leave frame in place.
    ///
    /// The frame remains visible and subsequent frames are drawn over it.
    #[default]
    None,

    /// Clear frame region to transparent black.
    ///
    /// The frame's rectangular region is cleared to fully transparent
    /// (RGBA: 0, 0, 0, 0) after display.
    Background,

    /// Restore canvas to previous state.
    ///
    /// The canvas is restored to the state it was in before this frame
    /// was rendered.
    Previous,
}

impl From<png::DisposeOp> for DisposeOp {
    fn from(op: png::DisposeOp) -> Self {
        match op {
            png::DisposeOp::None => Self::None,
            png::DisposeOp::Background => Self::Background,
            png::DisposeOp::Previous => Self::Previous,
        }
    }
}

/// A single frame from an animated PNG.
///
/// Contains frame metadata including timing, position, and blend/dispose operations.
/// The pixel data is stored in the canvas buffer during playback.
#[derive(Debug, Clone)]
pub struct ApngFrame {
    /// Frame width in pixels.
    pub width: u32,

    /// Frame height in pixels.
    pub height: u32,

    /// Frame position (left offset from canvas origin).
    pub x_offset: u32,

    /// Frame position (top offset from canvas origin).
    pub y_offset: u32,

    /// Frame delay duration.
    pub delay: Duration,

    /// Frame index (0-based).
    pub index: usize,

    /// Blend operation for this frame.
    pub blend_op: BlendOp,

    /// Dispose operation for this frame.
    pub dispose_op: DisposeOp,
}

// ============================================================================
// ApngPlayer (AC: #4, #5, #7)
// ============================================================================

/// Animated PNG (APNG) player implementing the [`MediaPlayer`] trait.
///
/// `ApngPlayer` provides frame-by-frame access to animated PNGs with correct
/// timing, loop handling, and blend/dispose operation support.
///
/// # Frame Iteration
///
/// Use [`next_frame()`](Self::next_frame) to get frames one at a time:
///
/// ```no_run
/// use dotmax::media::{ApngPlayer, MediaPlayer};
///
/// let mut player = ApngPlayer::new("animation.png")?;
/// while let Some(result) = player.next_frame() {
///     let (grid, delay) = result?;
///     // Display grid, then sleep for delay
///     std::thread::sleep(delay);
/// }
/// # Ok::<(), dotmax::DotmaxError>(())
/// ```
///
/// # Loop Handling
///
/// APNG loop count is respected:
/// - `loop_count() == Some(0)` → infinite loop
/// - `loop_count() == Some(n)` → play n times
/// - After all loops complete, `next_frame()` returns `None`
///
/// Use [`reset()`](Self::reset) to restart from frame 0.
///
/// # Memory Efficiency
///
/// Frames are decoded on-demand, not all at once. The player maintains
/// a canvas buffer for blend/dispose handling, but individual frames
/// are not cached.
///
/// # Thread Safety
///
/// `ApngPlayer` implements `Send` and can be moved across threads.
pub struct ApngPlayer {
    /// Path to the APNG file (for reset support).
    path: PathBuf,

    /// PNG decoder (streaming).
    decoder: png::Reader<BufReader<File>>,

    /// Canvas dimensions (from PNG header).
    canvas_width: u32,
    canvas_height: u32,

    /// Current canvas state (RGBA pixels).
    /// Used for blend/dispose operation handling.
    canvas: Vec<u8>,

    /// Previous canvas state (for DisposeOp::Previous).
    previous_canvas: Vec<u8>,

    /// Total frame count from animation control.
    frame_count: Option<usize>,

    /// Loop count from animation control.
    /// - `Some(0)` → infinite
    /// - `Some(n)` → play n times
    apng_loop_count: Option<u16>,

    /// Current frame index (0-based).
    current_frame: usize,

    /// Current loop iteration (1-based).
    current_loop: u16,

    /// Whether we've completed all loops.
    loops_completed: bool,

    /// Previous frame's dispose operation.
    previous_dispose: DisposeOp,

    /// Previous frame's rectangle (for disposal).
    previous_rect: (u32, u32, u32, u32), // (x, y, width, height)

    /// Terminal dimensions for rendering.
    terminal_width: usize,
    terminal_height: usize,

    /// Buffer for reading frame data.
    frame_buffer: Vec<u8>,

    /// Whether this is the first frame (may be default image).
    is_first_frame: bool,
}

impl std::fmt::Debug for ApngPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApngPlayer")
            .field("path", &self.path)
            .field("canvas_width", &self.canvas_width)
            .field("canvas_height", &self.canvas_height)
            .field("frame_count", &self.frame_count)
            .field("loop_count", &self.apng_loop_count)
            .field("current_frame", &self.current_frame)
            .field("current_loop", &self.current_loop)
            .finish_non_exhaustive()
    }
}

impl ApngPlayer {
    /// Creates a new `ApngPlayer` from an APNG file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the animated PNG file
    ///
    /// # Errors
    ///
    /// Returns `DotmaxError::ImageLoad` if the file cannot be opened or is
    /// not a valid APNG.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::media::ApngPlayer;
    ///
    /// let player = ApngPlayer::new("animation.png")?;
    /// println!("Canvas: {}x{}", player.canvas_width(), player.canvas_height());
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let decoder = png::Decoder::new(reader);
        let png_reader = decoder.read_info().map_err(|e| DotmaxError::ApngError {
            path: path.clone(),
            message: format!("Failed to decode APNG: {e}"),
        })?;

        let info = png_reader.info();
        let canvas_width = info.width;
        let canvas_height = info.height;

        // Get animation control data
        let animation_control = info.animation_control();
        let (frame_count, apng_loop_count) = animation_control.map_or_else(
            || {
                // Not an APNG or no animation control
                tracing::warn!("APNG file {:?} has no animation control chunk", path);
                (Some(1), Some(1))
            },
            |actl| {
                let loops = if actl.num_plays == 0 {
                    Some(0) // Infinite
                } else {
                    Some(actl.num_plays as u16)
                };
                (Some(actl.num_frames as usize), loops)
            },
        );

        tracing::info!(
            "Loaded APNG: {}x{}, {} frames, loop_count={:?}",
            canvas_width,
            canvas_height,
            frame_count.unwrap_or(0),
            apng_loop_count
        );

        // Initialize canvas buffer (RGBA)
        let canvas_size = (canvas_width as usize) * (canvas_height as usize) * 4;
        let canvas = vec![0u8; canvas_size];
        let previous_canvas = vec![0u8; canvas_size];

        // Get terminal size for rendering
        let (terminal_width, terminal_height) = crossterm::terminal::size()
            .map(|(w, h)| (w as usize, h as usize))
            .unwrap_or((80, 24));

        // Allocate frame buffer
        let frame_buffer = vec![0u8; png_reader.output_buffer_size().unwrap_or(canvas_size)];

        Ok(Self {
            path,
            decoder: png_reader,
            canvas_width,
            canvas_height,
            canvas,
            previous_canvas,
            frame_count,
            apng_loop_count,
            current_frame: 0,
            current_loop: 1,
            loops_completed: false,
            previous_dispose: DisposeOp::None,
            previous_rect: (0, 0, 0, 0),
            terminal_width,
            terminal_height,
            frame_buffer,
            is_first_frame: true,
        })
    }

    /// Returns the canvas width in pixels.
    #[must_use]
    pub const fn canvas_width(&self) -> u32 {
        self.canvas_width
    }

    /// Returns the canvas height in pixels.
    #[must_use]
    pub const fn canvas_height(&self) -> u32 {
        self.canvas_height
    }

    /// Decodes the next frame from the APNG.
    ///
    /// Returns `None` if no more frames are available.
    fn decode_next_frame(&mut self) -> Option<Result<ApngFrame>> {
        // Read next frame
        let output_info = match self.decoder.next_frame(&mut self.frame_buffer) {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!(
                    "APNG frame decode error at frame {}: {:?}",
                    self.current_frame,
                    e
                );
                return None; // No more frames
            }
        };

        // Get frame control info
        let frame_control = self.decoder.info().frame_control();

        let (x_offset, y_offset, width, height, delay, blend_op, dispose_op) =
            if let Some(fctl) = frame_control {
                let delay = Self::calculate_delay(fctl.delay_num, fctl.delay_den);
                (
                    fctl.x_offset,
                    fctl.y_offset,
                    fctl.width,
                    fctl.height,
                    delay,
                    BlendOp::from(fctl.blend_op),
                    DisposeOp::from(fctl.dispose_op),
                )
            } else {
                // No frame control - use full canvas with default timing
                (
                    0,
                    0,
                    self.canvas_width,
                    self.canvas_height,
                    Duration::from_millis(100),
                    BlendOp::Source,
                    DisposeOp::None,
                )
            };

        tracing::debug!(
            "Frame {}: {}x{} at ({},{}), delay={:?}, blend={:?}, dispose={:?}",
            self.current_frame,
            width,
            height,
            x_offset,
            y_offset,
            delay,
            blend_op,
            dispose_op
        );

        // Resize frame buffer if needed (output_info tells us actual output size)
        let expected_size = output_info.buffer_size();
        if self.frame_buffer.len() < expected_size {
            self.frame_buffer.resize(expected_size, 0);
        }

        Some(Ok(ApngFrame {
            width,
            height,
            x_offset,
            y_offset,
            delay,
            index: self.current_frame,
            blend_op,
            dispose_op,
        }))
    }

    /// Calculates frame delay from numerator and denominator.
    ///
    /// APNG delay is specified as delay_num/delay_den seconds.
    /// If delay_den is 0, it's treated as 100 (1/100th second units).
    fn calculate_delay(delay_num: u16, delay_den: u16) -> Duration {
        let millis = if delay_den == 0 {
            // Default denominator is 100
            u64::from(delay_num) * 10
        } else {
            (u64::from(delay_num) * 1000) / u64::from(delay_den)
        };

        // Minimum delay of 10ms to prevent CPU spinning
        // Also handle 0 delay as 100ms per spec recommendation
        let millis = if millis == 0 { 100 } else { millis.max(10) };

        Duration::from_millis(millis)
    }

    /// Applies the previous frame's dispose operation before drawing new frame.
    fn apply_previous_disposal(&mut self) {
        let (x, y, width, height) = self.previous_rect;
        if width == 0 || height == 0 {
            return;
        }

        match self.previous_dispose {
            DisposeOp::None => {
                // Leave canvas unchanged - nothing to do
            }
            DisposeOp::Background => {
                // Clear the previous frame area to transparent black
                for row in 0..height {
                    let canvas_y = (y + row) as usize;
                    if canvas_y >= self.canvas_height as usize {
                        continue;
                    }
                    for col in 0..width {
                        let canvas_x = (x + col) as usize;
                        if canvas_x >= self.canvas_width as usize {
                            continue;
                        }
                        let idx = (canvas_y * self.canvas_width as usize + canvas_x) * 4;
                        if idx + 3 < self.canvas.len() {
                            self.canvas[idx] = 0; // R
                            self.canvas[idx + 1] = 0; // G
                            self.canvas[idx + 2] = 0; // B
                            self.canvas[idx + 3] = 0; // A
                        }
                    }
                }
            }
            DisposeOp::Previous => {
                // Restore canvas to previous state
                self.canvas.copy_from_slice(&self.previous_canvas);
            }
        }
    }

    /// Saves current canvas state for DisposeOp::Previous.
    fn save_canvas_state(&mut self) {
        self.previous_canvas.copy_from_slice(&self.canvas);
    }

    /// Composites a frame onto the canvas using the specified blend operation.
    fn composite_frame(&mut self, frame: &ApngFrame) {
        let frame_width = frame.width as usize;
        let frame_height = frame.height as usize;
        let canvas_width = self.canvas_width as usize;

        // Determine bytes per pixel from the output
        // PNG can output RGB, RGBA, Grayscale, etc.
        let output_bytes_per_pixel = self.decoder.info().bytes_per_pixel();

        for row in 0..frame_height {
            let canvas_y = (frame.y_offset as usize) + row;
            if canvas_y >= self.canvas_height as usize {
                continue;
            }

            for col in 0..frame_width {
                let canvas_x = (frame.x_offset as usize) + col;
                if canvas_x >= canvas_width {
                    continue;
                }

                let frame_idx = (row * frame_width + col) * output_bytes_per_pixel;
                let canvas_idx = (canvas_y * canvas_width + canvas_x) * 4;

                if frame_idx + output_bytes_per_pixel > self.frame_buffer.len()
                    || canvas_idx + 4 > self.canvas.len()
                {
                    continue;
                }

                // Extract RGBA from frame buffer (handle different color types)
                let (r, g, b, a) = match self.decoder.info().color_type {
                    png::ColorType::Rgba => (
                        self.frame_buffer[frame_idx],
                        self.frame_buffer[frame_idx + 1],
                        self.frame_buffer[frame_idx + 2],
                        self.frame_buffer[frame_idx + 3],
                    ),
                    png::ColorType::Rgb => (
                        self.frame_buffer[frame_idx],
                        self.frame_buffer[frame_idx + 1],
                        self.frame_buffer[frame_idx + 2],
                        255,
                    ),
                    png::ColorType::GrayscaleAlpha => {
                        let gray = self.frame_buffer[frame_idx];
                        let alpha = self.frame_buffer[frame_idx + 1];
                        (gray, gray, gray, alpha)
                    }
                    png::ColorType::Grayscale => {
                        let gray = self.frame_buffer[frame_idx];
                        (gray, gray, gray, 255)
                    }
                    png::ColorType::Indexed => {
                        // Indexed color - should be expanded by decoder, but fallback
                        let idx = self.frame_buffer[frame_idx] as usize;
                        self.decoder
                            .info()
                            .palette
                            .as_ref()
                            .map_or((0, 0, 0, 255), |palette| {
                                if idx * 3 + 2 < palette.len() {
                                    (
                                        palette[idx * 3],
                                        palette[idx * 3 + 1],
                                        palette[idx * 3 + 2],
                                        255,
                                    )
                                } else {
                                    (0, 0, 0, 255)
                                }
                            })
                    }
                };

                match frame.blend_op {
                    BlendOp::Source => {
                        // Direct replacement
                        self.canvas[canvas_idx] = r;
                        self.canvas[canvas_idx + 1] = g;
                        self.canvas[canvas_idx + 2] = b;
                        self.canvas[canvas_idx + 3] = a;
                    }
                    BlendOp::Over => {
                        // Alpha composite (Porter-Duff "over")
                        if a == 255 {
                            // Fully opaque - direct copy
                            self.canvas[canvas_idx] = r;
                            self.canvas[canvas_idx + 1] = g;
                            self.canvas[canvas_idx + 2] = b;
                            self.canvas[canvas_idx + 3] = 255;
                        } else if a > 0 {
                            // Alpha blend
                            let src_a = f32::from(a) / 255.0;
                            let dst_a = f32::from(self.canvas[canvas_idx + 3]) / 255.0;
                            let out_a = src_a + dst_a * (1.0 - src_a);

                            if out_a > 0.0 {
                                let src_rgb = [f32::from(r), f32::from(g), f32::from(b)];
                                for (i, &src_component) in src_rgb.iter().enumerate() {
                                    let dst = f32::from(self.canvas[canvas_idx + i]);
                                    let blended = src_component
                                        .mul_add(src_a, dst * dst_a * (1.0 - src_a))
                                        / out_a;
                                    self.canvas[canvas_idx + i] = blended as u8;
                                }
                                self.canvas[canvas_idx + 3] = (out_a * 255.0) as u8;
                            }
                        }
                        // a == 0: fully transparent, leave canvas unchanged
                    }
                }
            }
        }
    }

    /// Converts the current canvas to a BrailleGrid.
    fn canvas_to_grid(&self) -> Result<BrailleGrid> {
        // Create RGBA image from canvas
        let img =
            image::RgbaImage::from_raw(self.canvas_width, self.canvas_height, self.canvas.clone())
                .ok_or_else(|| DotmaxError::ApngError {
                    path: self.path.clone(),
                    message: "Failed to create image from canvas".to_string(),
                })?;

        // Use ImageRenderer to convert to BrailleGrid
        let grid = ImageRenderer::new()
            .load_from_rgba(img)
            .resize(self.terminal_width, self.terminal_height, true)?
            .render()?;

        Ok(grid)
    }

    /// Reopens the APNG file and resets decoder state.
    fn reopen_decoder(&mut self) -> Result<()> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);

        let decoder = png::Decoder::new(reader);
        self.decoder = decoder.read_info().map_err(|e| DotmaxError::ApngError {
            path: self.path.clone(),
            message: format!("Failed to reopen APNG: {e}"),
        })?;

        // Reallocate frame buffer
        let canvas_size = (self.canvas_width as usize) * (self.canvas_height as usize) * 4;
        self.frame_buffer = vec![0u8; self.decoder.output_buffer_size().unwrap_or(canvas_size)];
        self.is_first_frame = true;

        Ok(())
    }
}

impl MediaPlayer for ApngPlayer {
    /// Returns the next frame and its display duration.
    ///
    /// Handles loop iteration automatically. Returns `None` when all loops
    /// are complete or no more frames are available.
    fn next_frame(&mut self) -> Option<Result<(BrailleGrid, Duration)>> {
        if self.loops_completed {
            return None;
        }

        // Apply previous frame's disposal method
        self.apply_previous_disposal();

        // Decode next frame
        let frame = match self.decode_next_frame() {
            Some(Ok(f)) => f,
            Some(Err(e)) => {
                // Log error and try to continue
                tracing::warn!("Skipping corrupted frame {}: {:?}", self.current_frame, e);
                self.current_frame += 1;
                // Recursively try next frame
                return self.next_frame();
            }
            None => {
                // No more frames - handle loop
                if let Some(frame_count) = self.frame_count {
                    tracing::debug!(
                        "Loop {} complete ({} frames)",
                        self.current_loop,
                        frame_count
                    );
                } else {
                    self.frame_count = Some(self.current_frame);
                    tracing::debug!("First loop complete, {} frames", self.current_frame);
                }

                // Check if we should loop
                let should_loop = match self.apng_loop_count {
                    Some(0) => true, // Infinite loop
                    Some(n) if self.current_loop < n => true,
                    _ => false,
                };

                if should_loop {
                    // Reset for next loop
                    self.current_loop += 1;
                    self.current_frame = 0;

                    // Clear canvas for fresh start
                    self.canvas.fill(0);
                    self.previous_canvas.fill(0);
                    self.previous_dispose = DisposeOp::None;
                    self.previous_rect = (0, 0, 0, 0);

                    // Reopen decoder to restart from beginning
                    if let Err(e) = self.reopen_decoder() {
                        return Some(Err(e));
                    }

                    // Recursively get first frame of new loop
                    return self.next_frame();
                }

                // All loops complete
                self.loops_completed = true;
                return None;
            }
        };

        // Save canvas state if this frame has DisposeOp::Previous
        if frame.dispose_op == DisposeOp::Previous {
            self.save_canvas_state();
        }

        // Composite frame onto canvas
        self.composite_frame(&frame);

        // Convert canvas to BrailleGrid
        let grid = match self.canvas_to_grid() {
            Ok(g) => g,
            Err(e) => return Some(Err(e)),
        };

        // Store disposal info for next frame
        self.previous_dispose = frame.dispose_op;
        self.previous_rect = (frame.x_offset, frame.y_offset, frame.width, frame.height);
        self.current_frame += 1;
        self.is_first_frame = false;

        Some(Ok((grid, frame.delay)))
    }

    /// Resets playback to the first frame.
    fn reset(&mut self) {
        self.current_frame = 0;
        self.current_loop = 1;
        self.loops_completed = false;
        self.canvas.fill(0);
        self.previous_canvas.fill(0);
        self.previous_dispose = DisposeOp::None;
        self.previous_rect = (0, 0, 0, 0);
        self.is_first_frame = true;

        // Reopen decoder
        if let Err(e) = self.reopen_decoder() {
            tracing::warn!("Failed to reset APNG decoder: {:?}", e);
        }
    }

    /// Returns the total number of frames.
    fn frame_count(&self) -> Option<usize> {
        self.frame_count
    }

    /// Returns the APNG's loop count.
    ///
    /// - `Some(0)` → infinite looping
    /// - `Some(n)` → loop n times
    /// - `None` → play once
    fn loop_count(&self) -> Option<u16> {
        self.apng_loop_count
    }

    /// Updates terminal dimensions for subsequent frame rendering.
    ///
    /// Call this when the terminal is resized to ensure frames are
    /// rendered at the correct size.
    fn handle_resize(&mut self, width: usize, height: usize) {
        self.terminal_width = width;
        self.terminal_height = height;
        tracing::debug!("ApngPlayer resized to {}x{}", width, height);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Ensure ApngPlayer is Send (required by MediaPlayer trait)
    fn _assert_apng_player_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ApngPlayer>();
    }

    #[test]
    fn test_blend_op_from_png() {
        assert_eq!(BlendOp::from(png::BlendOp::Source), BlendOp::Source);
        assert_eq!(BlendOp::from(png::BlendOp::Over), BlendOp::Over);
    }

    #[test]
    fn test_dispose_op_from_png() {
        assert_eq!(DisposeOp::from(png::DisposeOp::None), DisposeOp::None);
        assert_eq!(
            DisposeOp::from(png::DisposeOp::Background),
            DisposeOp::Background
        );
        assert_eq!(
            DisposeOp::from(png::DisposeOp::Previous),
            DisposeOp::Previous
        );
    }

    #[test]
    fn test_blend_op_default() {
        assert_eq!(BlendOp::default(), BlendOp::Source);
    }

    #[test]
    fn test_dispose_op_default() {
        assert_eq!(DisposeOp::default(), DisposeOp::None);
    }

    #[test]
    fn test_calculate_delay_normal() {
        // 1/10 second = 100ms
        assert_eq!(
            ApngPlayer::calculate_delay(1, 10),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn test_calculate_delay_zero_denominator() {
        // delay_den=0 means denominator is 100
        // So delay_num=10, delay_den=0 means 10/100 = 0.1 seconds = 100ms
        assert_eq!(
            ApngPlayer::calculate_delay(10, 0),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn test_calculate_delay_zero_numerator() {
        // delay_num=0 should use default 100ms
        assert_eq!(
            ApngPlayer::calculate_delay(0, 100),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn test_calculate_delay_minimum() {
        // Very small delay should be clamped to 10ms minimum
        assert_eq!(
            ApngPlayer::calculate_delay(1, 1000),
            Duration::from_millis(10)
        );
    }

    #[test]
    fn test_apng_frame_debug() {
        let frame = ApngFrame {
            width: 100,
            height: 100,
            x_offset: 0,
            y_offset: 0,
            delay: Duration::from_millis(100),
            index: 0,
            blend_op: BlendOp::Source,
            dispose_op: DisposeOp::None,
        };
        let debug_str = format!("{:?}", frame);
        assert!(debug_str.contains("ApngFrame"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_apng_player_new_nonexistent() {
        let player = ApngPlayer::new("nonexistent.png");
        assert!(player.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_apng_player_new_animated() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.png");
        if path.exists() {
            let player = ApngPlayer::new(path);
            assert!(
                player.is_ok(),
                "Should load animated PNG: {:?}",
                player.err()
            );
            let player = player.unwrap();
            assert_eq!(player.canvas_width(), 10);
            assert_eq!(player.canvas_height(), 10);
            // 3 frames, infinite loop
            assert_eq!(player.frame_count(), Some(3));
            assert_eq!(player.loop_count(), Some(0));
        }
    }

    #[test]
    fn test_apng_player_new_static_png() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/static_png.png");
        if path.exists() {
            // Static PNG can still be "played" (just 1 frame)
            let player = ApngPlayer::new(path);
            assert!(player.is_ok(), "Should load static PNG: {:?}", player.err());
            let player = player.unwrap();
            // Static PNG has no animation control, defaults to 1 frame
            assert_eq!(player.frame_count(), Some(1));
        }
    }

    #[test]
    fn test_apng_player_next_frame() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.png");
        if path.exists() {
            let mut player = ApngPlayer::new(path).unwrap();

            // Get first frame
            let frame1 = player.next_frame();
            assert!(frame1.is_some(), "Should have first frame");
            let (grid, delay) = frame1.unwrap().unwrap();
            assert!(grid.width() > 0);
            assert!(grid.height() > 0);
            assert_eq!(delay.as_millis(), 100); // 10/100 = 0.1s = 100ms
        }
    }

    #[test]
    fn test_apng_player_loop_twice() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/loop_twice.png");
        if path.exists() {
            let player = ApngPlayer::new(path).unwrap();
            assert_eq!(player.loop_count(), Some(2), "Should loop twice");
        }
    }

    #[test]
    fn test_apng_player_reset() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.png");
        if path.exists() {
            let mut player = ApngPlayer::new(path).unwrap();

            // Consume some frames
            let _ = player.next_frame();
            let _ = player.next_frame();

            // Reset
            player.reset();

            // Should get first frame again
            let frame = player.next_frame();
            assert!(frame.is_some(), "Should have frame after reset");
        }
    }

    #[test]
    fn test_apng_player_debug() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.png");
        if path.exists() {
            let player = ApngPlayer::new(path).unwrap();
            let debug_str = format!("{:?}", player);
            assert!(debug_str.contains("ApngPlayer"));
            assert!(debug_str.contains("canvas_width"));
        }
    }

    #[test]
    fn test_apng_player_as_media_player_trait_object() {
        use std::path::Path;
        let path = Path::new("tests/fixtures/media/animated.png");
        if path.exists() {
            let player = ApngPlayer::new(path).unwrap();
            // This compiles = ApngPlayer implements MediaPlayer
            let _trait_obj: Box<dyn MediaPlayer> = Box::new(player);
        }
    }
}
