//! Pre-rendered animation storage and playback.
//!
//! This module provides [`PrerenderedAnimation`], a struct for pre-computing animation
//! frames and playing them back with zero computation overhead. This is ideal for:
//!
//! - **Loading spinners**: Known, looping patterns that repeat indefinitely
//! - **Intro animations**: Fixed sequences shown at startup
//! - **Caching complex computations**: Fractal zooms, particle effects, etc.
//!
//! # Overview
//!
//! `PrerenderedAnimation` stores a sequence of [`BrailleGrid`](crate::BrailleGrid) frames
//! that can be played back at a specified frame rate. Unlike [`AnimationLoop`](super::AnimationLoop),
//! which computes frames on-the-fly, pre-rendered animations front-load all computation,
//! enabling buttery-smooth playback even for complex graphics.
//!
//! # Memory Usage
//!
//! Each frame uses approximately `width × height` bytes. For a typical 80×24 terminal,
//! that's about 2KB per frame. A 10-second animation at 30fps (300 frames) uses
//! approximately 600KB.
//!
//! # File Format
//!
//! Animations can be saved to and loaded from disk using a simple binary format:
//!
//! | Offset | Size   | Field       | Description                              |
//! |--------|--------|-------------|------------------------------------------|
//! | 0      | 4      | Magic       | `b"DMAX"` - File type identifier        |
//! | 4      | 1      | Version     | Format version (currently 1)            |
//! | 5      | 4      | Frame Rate  | Target FPS (u32 little-endian)          |
//! | 9      | 4      | Frame Count | Number of frames (u32 little-endian)    |
//! | 13     | 4      | Width       | Grid width in cells (u32 little-endian) |
//! | 17     | 4      | Height      | Grid height in cells (u32 little-endian)|
//! | 21     | N      | Frame Data  | Sequential frame bytes (width*height per frame) |
//!
//! # Example
//!
//! ```no_run
//! use dotmax::animation::PrerenderedAnimation;
//! use dotmax::BrailleGrid;
//! use dotmax::TerminalRenderer;
//!
//! // Pre-render frames (expensive computation done once)
//! let mut animation = PrerenderedAnimation::new(30);
//! for frame_num in 0..60 {
//!     let mut grid = BrailleGrid::new(80, 24).unwrap();
//!     // Draw frame content (e.g., rotating shape)
//!     let angle = (frame_num as f64) * 6.0 * std::f64::consts::PI / 180.0;
//!     let cx = 80;
//!     let cy = 48;
//!     for r in 0..30 {
//!         let x = cx + (angle.cos() * r as f64) as i32;
//!         let y = cy + (angle.sin() * r as f64) as i32;
//!         if x >= 0 && y >= 0 {
//!             let _ = grid.set_dot(x as usize, y as usize);
//!         }
//!     }
//!     animation.add_frame(grid);
//! }
//!
//! // Playback is instant - no computation
//! // let mut renderer = TerminalRenderer::new().unwrap();
//! // animation.play(&mut renderer).unwrap();
//! ```

use crate::animation::FrameTimer;
use crate::error::DotmaxError;
use crate::grid::BrailleGrid;
use crate::render::TerminalRenderer;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Duration;
use tracing::debug;

/// Magic bytes for the DMAX animation file format.
const MAGIC: &[u8; 4] = b"DMAX";

/// Current version of the file format.
const VERSION: u8 = 1;

/// Minimum allowed target FPS.
const MIN_FPS: u32 = 1;

/// Maximum allowed target FPS.
const MAX_FPS: u32 = 240;

/// Pre-rendered animation for optimal playback performance.
///
/// `PrerenderedAnimation` stores a sequence of [`BrailleGrid`] frames that can be
/// played back at a specified frame rate with zero computation during playback.
/// This is ideal for loading spinners, intro animations, and caching expensive
/// computations.
///
/// # Memory Usage
///
/// Each frame uses approximately `width × height` bytes. For a typical 80×24
/// terminal, that's about 2KB per frame. A 10-second animation at 30fps (300
/// frames) uses approximately 600KB.
///
/// # Example
///
/// ```
/// use dotmax::animation::PrerenderedAnimation;
/// use dotmax::BrailleGrid;
///
/// // Create animation with target frame rate
/// let mut animation = PrerenderedAnimation::new(30);
///
/// // Add pre-computed frames
/// let grid = BrailleGrid::new(10, 5).unwrap();
/// animation.add_frame(grid);
///
/// assert_eq!(animation.frame_count(), 1);
/// assert_eq!(animation.frame_rate(), 30);
/// ```
#[derive(Debug)]
pub struct PrerenderedAnimation {
    /// Pre-rendered frames stored in sequence.
    frames: Vec<BrailleGrid>,
    /// Target frames per second (1-240).
    frame_rate: u32,
}

impl PrerenderedAnimation {
    /// Creates a new empty pre-rendered animation with the specified frame rate.
    ///
    /// The frame rate is clamped to the valid range (1-240). Values outside
    /// this range are silently corrected to the nearest valid value.
    ///
    /// # Arguments
    ///
    /// * `frame_rate` - Target frames per second (1-240, clamped if out of range)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::PrerenderedAnimation;
    ///
    /// // Standard 30 FPS animation
    /// let animation = PrerenderedAnimation::new(30);
    /// assert_eq!(animation.frame_rate(), 30);
    /// assert_eq!(animation.frame_count(), 0);
    ///
    /// // Values are clamped to valid range
    /// let animation = PrerenderedAnimation::new(0);
    /// assert_eq!(animation.frame_rate(), 1);  // Clamped to minimum
    ///
    /// let animation = PrerenderedAnimation::new(500);
    /// assert_eq!(animation.frame_rate(), 240);  // Clamped to maximum
    /// ```
    #[must_use]
    pub fn new(frame_rate: u32) -> Self {
        Self {
            frames: Vec::new(),
            frame_rate: frame_rate.clamp(MIN_FPS, MAX_FPS),
        }
    }

    /// Adds a frame to the animation.
    ///
    /// Frames are stored by value (owned `BrailleGrid`). There is no validation
    /// on frame dimensions - mixed sizes are allowed for flexibility.
    ///
    /// # Arguments
    ///
    /// * `frame` - The [`BrailleGrid`] to add to the animation
    ///
    /// # Returns
    ///
    /// `&mut Self` for builder-style method chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::PrerenderedAnimation;
    /// use dotmax::BrailleGrid;
    ///
    /// let mut animation = PrerenderedAnimation::new(30);
    ///
    /// // Add frames with chaining
    /// animation
    ///     .add_frame(BrailleGrid::new(10, 5).unwrap())
    ///     .add_frame(BrailleGrid::new(10, 5).unwrap())
    ///     .add_frame(BrailleGrid::new(10, 5).unwrap());
    ///
    /// assert_eq!(animation.frame_count(), 3);
    /// ```
    pub fn add_frame(&mut self, frame: BrailleGrid) -> &mut Self {
        self.frames.push(frame);
        self
    }

    /// Returns the number of stored frames.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::PrerenderedAnimation;
    /// use dotmax::BrailleGrid;
    ///
    /// let animation = PrerenderedAnimation::new(30);
    /// assert_eq!(animation.frame_count(), 0);
    ///
    /// let mut animation = PrerenderedAnimation::new(30);
    /// animation.add_frame(BrailleGrid::new(10, 5).unwrap());
    /// assert_eq!(animation.frame_count(), 1);
    /// ```
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Returns the target frame rate (FPS).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::PrerenderedAnimation;
    ///
    /// let animation = PrerenderedAnimation::new(60);
    /// assert_eq!(animation.frame_rate(), 60);
    /// ```
    #[must_use]
    pub const fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Plays the animation once from start to finish.
    ///
    /// Renders all frames at the specified frame rate using [`FrameTimer`](super::FrameTimer)
    /// for consistent timing. Returns immediately if no frames are stored.
    ///
    /// # Arguments
    ///
    /// * `renderer` - The [`TerminalRenderer`] to render frames to
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Animation played successfully
    /// * `Err(DotmaxError)` - Rendering failed
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::Terminal`] if rendering to the terminal fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::PrerenderedAnimation;
    /// use dotmax::BrailleGrid;
    /// use dotmax::TerminalRenderer;
    ///
    /// let mut animation = PrerenderedAnimation::new(30);
    /// // ... add frames ...
    ///
    /// let mut renderer = TerminalRenderer::new()?;
    /// animation.play(&mut renderer)?;
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn play(&self, renderer: &mut TerminalRenderer) -> Result<(), DotmaxError> {
        if self.frames.is_empty() {
            debug!("play() called with empty animation, returning immediately");
            return Ok(());
        }

        debug!(
            frame_count = self.frames.len(),
            frame_rate = self.frame_rate,
            "Starting single playback"
        );

        let mut timer = FrameTimer::new(self.frame_rate);

        for (i, frame) in self.frames.iter().enumerate() {
            renderer.render(frame)?;
            debug!(frame = i, "Rendered frame");
            timer.wait_for_next_frame();
        }

        debug!("Single playback complete");
        Ok(())
    }

    /// Plays the animation in a continuous loop until Ctrl+C is pressed.
    ///
    /// Loops seamlessly with no pause between repetitions. Stops gracefully
    /// when Ctrl+C is detected, returning `Ok(())` rather than an error.
    ///
    /// # Arguments
    ///
    /// * `renderer` - The [`TerminalRenderer`] to render frames to
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Animation stopped (either Ctrl+C or empty)
    /// * `Err(DotmaxError)` - Rendering failed
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::Terminal`] if rendering to the terminal fails.
    ///
    /// # Ctrl+C Handling
    ///
    /// The loop checks for Ctrl+C before each frame using non-blocking event polling.
    /// When detected, the function returns `Ok(())` gracefully - not an error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::PrerenderedAnimation;
    /// use dotmax::BrailleGrid;
    /// use dotmax::TerminalRenderer;
    ///
    /// let mut animation = PrerenderedAnimation::new(30);
    /// // ... add frames ...
    ///
    /// let mut renderer = TerminalRenderer::new()?;
    /// println!("Press Ctrl+C to stop");
    /// animation.play_loop(&mut renderer)?;  // Runs until Ctrl+C
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn play_loop(&self, renderer: &mut TerminalRenderer) -> Result<(), DotmaxError> {
        if self.frames.is_empty() {
            debug!("play_loop() called with empty animation, returning immediately");
            return Ok(());
        }

        debug!(
            frame_count = self.frames.len(),
            frame_rate = self.frame_rate,
            "Starting looped playback"
        );

        let mut timer = FrameTimer::new(self.frame_rate);
        let mut loop_count: u64 = 0;

        'outer: loop {
            loop_count += 1;
            debug!(loop_iteration = loop_count, "Starting animation loop");

            for (i, frame) in self.frames.iter().enumerate() {
                // Check for Ctrl+C with non-blocking poll
                if event::poll(Duration::ZERO)? {
                    if let Event::Key(key) = event::read()? {
                        if key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            debug!(
                                loops_completed = loop_count,
                                frame = i,
                                "Ctrl+C detected, stopping playback"
                            );
                            break 'outer;
                        }
                    }
                }

                renderer.render(frame)?;
                timer.wait_for_next_frame();
            }
        }

        debug!(total_loops = loop_count, "Looped playback stopped");
        Ok(())
    }

    /// Saves the animation to a file.
    ///
    /// Uses a simple binary format (see module documentation for details).
    /// Creates parent directories if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path to save to
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Animation saved successfully
    /// * `Err(DotmaxError)` - I/O error occurred
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::Terminal`] (wrapping `io::Error`) if:
    /// - Directory creation fails
    /// - File creation fails
    /// - Write operations fail
    ///
    /// # File Format
    ///
    /// See module-level documentation for the complete file format specification.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::PrerenderedAnimation;
    /// use dotmax::BrailleGrid;
    /// use std::path::Path;
    ///
    /// let mut animation = PrerenderedAnimation::new(30);
    /// animation.add_frame(BrailleGrid::new(80, 24).unwrap());
    ///
    /// animation.save_to_file(Path::new("my_animation.dmax"))?;
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn save_to_file(&self, path: &Path) -> Result<(), DotmaxError> {
        debug!(path = ?path, frames = self.frames.len(), "Saving animation to file");

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Determine dimensions from first frame (or use 0x0 for empty)
        let (width, height) = self.frames.first().map_or((0, 0), BrailleGrid::dimensions);

        // Write header
        writer.write_all(MAGIC)?;
        writer.write_all(&[VERSION])?;
        writer.write_all(&self.frame_rate.to_le_bytes())?;
        #[allow(clippy::cast_possible_truncation)]
        let frame_count = self.frames.len() as u32;
        writer.write_all(&frame_count.to_le_bytes())?;
        #[allow(clippy::cast_possible_truncation)]
        let width_u32 = width as u32;
        #[allow(clippy::cast_possible_truncation)]
        let height_u32 = height as u32;
        writer.write_all(&width_u32.to_le_bytes())?;
        writer.write_all(&height_u32.to_le_bytes())?;

        // Write frame data
        for frame in &self.frames {
            let data = frame.get_raw_patterns();
            writer.write_all(data)?;
        }

        writer.flush()?;
        debug!(path = ?path, "Animation saved successfully");
        Ok(())
    }

    /// Loads an animation from a file.
    ///
    /// Validates the file format and returns appropriate errors for invalid files.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path to load from
    ///
    /// # Returns
    ///
    /// * `Ok(PrerenderedAnimation)` - Animation loaded successfully
    /// * `Err(DotmaxError)` - File not found, invalid format, or I/O error
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::Terminal`] (wrapping `io::Error`) if:
    /// - File not found
    /// - Permission denied
    /// - Invalid magic bytes (not a DMAX file)
    /// - Truncated or corrupted data
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::PrerenderedAnimation;
    /// use std::path::Path;
    ///
    /// let animation = PrerenderedAnimation::load_from_file(Path::new("my_animation.dmax"))?;
    /// println!("Loaded {} frames at {} FPS", animation.frame_count(), animation.frame_rate());
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    pub fn load_from_file(path: &Path) -> Result<Self, DotmaxError> {
        debug!(path = ?path, "Loading animation from file");

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read and validate magic bytes
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(DotmaxError::Terminal(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid magic bytes: expected {MAGIC:?}, got {magic:?}"),
            )));
        }

        // Read version
        let mut version = [0u8; 1];
        reader.read_exact(&mut version)?;
        let file_version = version[0];
        if file_version != VERSION {
            return Err(DotmaxError::Terminal(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unsupported file version: expected {VERSION}, got {file_version}"),
            )));
        }

        // Read header fields
        let mut frame_rate_bytes = [0u8; 4];
        reader.read_exact(&mut frame_rate_bytes)?;
        let frame_rate = u32::from_le_bytes(frame_rate_bytes);

        let mut frame_count_bytes = [0u8; 4];
        reader.read_exact(&mut frame_count_bytes)?;
        let frame_count = u32::from_le_bytes(frame_count_bytes);

        let mut width_bytes = [0u8; 4];
        reader.read_exact(&mut width_bytes)?;
        let width = u32::from_le_bytes(width_bytes) as usize;

        let mut height_bytes = [0u8; 4];
        reader.read_exact(&mut height_bytes)?;
        let height = u32::from_le_bytes(height_bytes) as usize;

        debug!(
            frame_rate = frame_rate,
            frame_count = frame_count,
            width = width,
            height = height,
            "Read animation header"
        );

        // Read frames
        let mut frames = Vec::with_capacity(frame_count as usize);
        let frame_size = width * height;

        for i in 0..frame_count {
            let mut data = vec![0u8; frame_size];
            reader.read_exact(&mut data)?;

            // Create BrailleGrid and populate with data
            let mut grid = BrailleGrid::new(width, height)?;
            grid.set_raw_patterns(&data);
            frames.push(grid);

            debug!(frame = i, "Loaded frame");
        }

        debug!(path = ?path, frames = frames.len(), "Animation loaded successfully");

        Ok(Self {
            frames,
            frame_rate: frame_rate.clamp(MIN_FPS, MAX_FPS),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ========================================================================
    // AC #1: Constructor Tests
    // ========================================================================

    #[test]
    fn test_new_creates_empty_animation() {
        let animation = PrerenderedAnimation::new(30);
        assert_eq!(animation.frame_count(), 0);
        assert_eq!(animation.frame_rate(), 30);
    }

    #[test]
    fn test_new_clamps_fps_below_min() {
        let animation = PrerenderedAnimation::new(0);
        assert_eq!(animation.frame_rate(), 1);
    }

    #[test]
    fn test_new_clamps_fps_above_max() {
        let animation = PrerenderedAnimation::new(1000);
        assert_eq!(animation.frame_rate(), 240);
    }

    #[test]
    fn test_new_at_min_boundary() {
        let animation = PrerenderedAnimation::new(1);
        assert_eq!(animation.frame_rate(), 1);
    }

    #[test]
    fn test_new_at_max_boundary() {
        let animation = PrerenderedAnimation::new(240);
        assert_eq!(animation.frame_rate(), 240);
    }

    // ========================================================================
    // AC #2: add_frame() Tests
    // ========================================================================

    #[test]
    fn test_add_frame_increments_count() {
        let mut animation = PrerenderedAnimation::new(30);
        assert_eq!(animation.frame_count(), 0);

        let grid = BrailleGrid::new(10, 5).unwrap();
        animation.add_frame(grid);
        assert_eq!(animation.frame_count(), 1);
    }

    #[test]
    fn test_add_frame_chaining_works() {
        let mut animation = PrerenderedAnimation::new(30);
        animation
            .add_frame(BrailleGrid::new(10, 5).unwrap())
            .add_frame(BrailleGrid::new(10, 5).unwrap())
            .add_frame(BrailleGrid::new(10, 5).unwrap());

        assert_eq!(animation.frame_count(), 3);
    }

    #[test]
    fn test_add_frame_accepts_different_sizes() {
        let mut animation = PrerenderedAnimation::new(30);
        animation
            .add_frame(BrailleGrid::new(10, 5).unwrap())
            .add_frame(BrailleGrid::new(20, 10).unwrap())
            .add_frame(BrailleGrid::new(5, 3).unwrap());

        assert_eq!(animation.frame_count(), 3);
    }

    // ========================================================================
    // AC #5: frame_count() Tests
    // ========================================================================

    #[test]
    fn test_frame_count_returns_zero_for_empty() {
        let animation = PrerenderedAnimation::new(30);
        assert_eq!(animation.frame_count(), 0);
    }

    #[test]
    fn test_frame_count_returns_correct_value() {
        let mut animation = PrerenderedAnimation::new(30);
        for _ in 0..5 {
            animation.add_frame(BrailleGrid::new(10, 5).unwrap());
        }
        assert_eq!(animation.frame_count(), 5);
    }

    // ========================================================================
    // AC #6, AC #7: File I/O Tests
    // ========================================================================

    #[test]
    fn test_save_load_roundtrip_preserves_data() {
        let mut animation = PrerenderedAnimation::new(30);

        // Add frames with some data
        for i in 0..3 {
            let mut grid = BrailleGrid::new(10, 5).unwrap();
            // Set a dot at position based on frame number
            grid.set_dot(i * 2, 0).unwrap();
            animation.add_frame(grid);
        }

        // Save to temp file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        animation.save_to_file(path).unwrap();

        // Load back
        let loaded = PrerenderedAnimation::load_from_file(path).unwrap();

        // Verify
        assert_eq!(loaded.frame_rate(), 30);
        assert_eq!(loaded.frame_count(), 3);
    }

    #[test]
    fn test_load_with_invalid_magic_returns_error() {
        // Create file with wrong magic bytes
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"BADX").unwrap();
        temp_file.write_all(&[1u8]).unwrap(); // version
        temp_file.write_all(&30u32.to_le_bytes()).unwrap(); // fps
        temp_file.write_all(&0u32.to_le_bytes()).unwrap(); // frame count
        temp_file.write_all(&10u32.to_le_bytes()).unwrap(); // width
        temp_file.write_all(&5u32.to_le_bytes()).unwrap(); // height
        temp_file.flush().unwrap();

        let result = PrerenderedAnimation::load_from_file(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_nonexistent_file_returns_error() {
        let result = PrerenderedAnimation::load_from_file(Path::new("/nonexistent/path/file.dmax"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_truncated_file_returns_error() {
        // Create file with truncated header
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"DMAX").unwrap();
        // Missing version and other header fields
        temp_file.flush().unwrap();

        let result = PrerenderedAnimation::load_from_file(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_save_empty_animation() {
        let animation = PrerenderedAnimation::new(60);

        let temp_file = NamedTempFile::new().unwrap();
        let result = animation.save_to_file(temp_file.path());
        assert!(result.is_ok());

        // Load and verify empty
        let loaded = PrerenderedAnimation::load_from_file(temp_file.path()).unwrap();
        assert_eq!(loaded.frame_count(), 0);
        assert_eq!(loaded.frame_rate(), 60);
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("subdir/nested/animation.dmax");

        let animation = PrerenderedAnimation::new(30);
        let result = animation.save_to_file(&path);
        assert!(result.is_ok());
        assert!(path.exists());
    }
}
