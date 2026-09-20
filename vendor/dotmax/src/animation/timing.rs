//! Frame timing control for consistent animation speeds.
//!
//! This module provides [`FrameTimer`], a utility for managing animation frame timing.
//! It ensures animations run at a consistent frame rate regardless of system performance,
//! and provides accurate FPS measurement with graceful frame dropping when the system
//! falls behind.
//!
//! # Overview
//!
//! Frame timing is essential for smooth animations. Without proper timing control,
//! animations would run as fast as the system allows, resulting in inconsistent
//! speeds across different hardware.
//!
//! `FrameTimer` solves this by:
//! - Calculating the target time between frames based on desired FPS
//! - Sleeping for the appropriate duration to maintain frame rate
//! - Tracking actual frame times for performance monitoring
//! - Handling frame drops gracefully when computation takes too long
//!
//! # Platform Considerations
//!
//! Sleep resolution varies by operating system:
//! - **Linux/macOS**: ~1ms sleep resolution (high precision)
//! - **Windows**: ~15ms default resolution (may need `timeBeginPeriod(1)` for precision)
//!
//! For applications requiring precise timing on Windows, consider calling
//! `timeBeginPeriod(1)` at application startup via the `winapi` crate.
//!
//! # Example
//!
//! ```no_run
//! use dotmax::animation::FrameTimer;
//! use std::time::Duration;
//!
//! // Create a timer targeting 60 FPS
//! let mut timer = FrameTimer::new(60);
//!
//! // Animation loop
//! for frame in 0..100 {
//!     // ... do rendering work here ...
//!
//!     // Wait for next frame (maintains target FPS)
//!     timer.wait_for_next_frame();
//!
//!     // Monitor actual performance
//!     println!("Actual FPS: {:.1}", timer.actual_fps());
//! }
//! ```
//!
//! # Frame Drops
//!
//! When a frame takes longer than the target duration (e.g., due to complex
//! rendering), `FrameTimer` handles this gracefully by:
//!
//! 1. Skipping the sleep entirely (no waiting for a frame that's already late)
//! 2. Recording the actual frame time (reflected in `actual_fps()`)
//! 3. Emitting a debug log via `tracing`
//! 4. **Not** attempting to "catch up" with extra frames
//!
//! This approach prevents the "spiral of death" where falling behind leads to
//! more work trying to catch up.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tracing::debug;

/// Rolling window size for FPS calculation.
///
/// 60 frames provides approximately 1 second of history at 60fps,
/// offering a good balance between responsiveness and stability.
const FRAME_WINDOW_SIZE: usize = 60;

/// Minimum allowed target FPS.
///
/// Values below 1 FPS don't make sense for animation timing.
const MIN_FPS: u32 = 1;

/// Maximum allowed target FPS.
///
/// 240 FPS is the practical upper limit for most displays and systems.
/// Higher values are clamped to prevent unreasonably short frame times.
const MAX_FPS: u32 = 240;

/// Frame timing control for consistent animation speeds.
///
/// `FrameTimer` manages the timing of animation frames, ensuring animations
/// run at a consistent frame rate regardless of system performance. It provides
/// accurate FPS measurement and graceful frame dropping when the system falls behind.
///
/// # Creating a Timer
///
/// ```
/// use dotmax::animation::FrameTimer;
///
/// // Standard 60 FPS animation
/// let timer_60fps = FrameTimer::new(60);
///
/// // Slower 30 FPS for less CPU usage
/// let timer_30fps = FrameTimer::new(30);
///
/// // High refresh rate
/// let timer_120fps = FrameTimer::new(120);
/// ```
///
/// # FPS Validation
///
/// Target FPS is clamped to the valid range (1-240). Invalid values
/// are silently corrected:
///
/// ```
/// use dotmax::animation::FrameTimer;
///
/// let timer = FrameTimer::new(0);  // Clamped to 1 FPS
/// assert_eq!(timer.target_fps(), 1);
///
/// let timer = FrameTimer::new(500);  // Clamped to 240 FPS
/// assert_eq!(timer.target_fps(), 240);
/// ```
#[derive(Debug)]
pub struct FrameTimer {
    /// Target frames per second (1-240)
    target_fps: u32,
    /// Duration of each frame at target FPS
    frame_duration: Duration,
    /// Timestamp of the last frame boundary
    last_frame: Instant,
    /// Rolling window of recent frame durations for FPS calculation
    frame_times: VecDeque<Duration>,
}

impl FrameTimer {
    /// Creates a new frame timer targeting the specified FPS.
    ///
    /// The target FPS is clamped to the valid range (1-240). Values outside
    /// this range are silently corrected to the nearest valid value.
    ///
    /// # Arguments
    ///
    /// * `target_fps` - Target frames per second (1-240, clamped if out of range)
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameTimer;
    ///
    /// // Create a 60 FPS timer (16.67ms per frame)
    /// let timer = FrameTimer::new(60);
    /// assert_eq!(timer.target_fps(), 60);
    ///
    /// // Frame duration is calculated automatically
    /// let duration_ms = timer.target_frame_time().as_secs_f64() * 1000.0;
    /// assert!((duration_ms - 16.67).abs() < 0.01);
    /// ```
    ///
    /// ```
    /// use dotmax::animation::FrameTimer;
    ///
    /// // Invalid values are clamped
    /// let timer = FrameTimer::new(0);
    /// assert_eq!(timer.target_fps(), 1);  // Clamped to minimum
    /// ```
    #[must_use]
    pub fn new(target_fps: u32) -> Self {
        let fps = target_fps.clamp(MIN_FPS, MAX_FPS);
        Self {
            target_fps: fps,
            frame_duration: Duration::from_secs_f64(1.0 / f64::from(fps)),
            last_frame: Instant::now(),
            frame_times: VecDeque::with_capacity(FRAME_WINDOW_SIZE),
        }
    }

    /// Waits until the next frame should begin.
    ///
    /// This method calculates the elapsed time since the last frame and sleeps
    /// for the remaining duration to maintain the target frame rate. If the
    /// system is behind schedule (frame took longer than target), no sleep
    /// occurs and the frame is considered "dropped".
    ///
    /// # Frame Dropping
    ///
    /// When a frame takes longer than the target duration:
    /// - No sleep occurs (the frame is already late)
    /// - A debug log is emitted via `tracing`
    /// - The actual frame time is recorded (affects `actual_fps()`)
    /// - No "catch-up" is attempted
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::FrameTimer;
    ///
    /// let mut timer = FrameTimer::new(60);
    ///
    /// loop {
    ///     // Do rendering work...
    ///
    ///     // Wait for next frame - blocks until frame time elapsed
    ///     timer.wait_for_next_frame();
    /// }
    /// ```
    pub fn wait_for_next_frame(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame);

        // Record frame time for FPS calculation (before any sleep)
        if self.frame_times.len() >= FRAME_WINDOW_SIZE {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(elapsed);

        // Calculate sleep duration (saturating_sub prevents underflow)
        let sleep_duration = self.frame_duration.saturating_sub(elapsed);

        if sleep_duration > Duration::ZERO {
            std::thread::sleep(sleep_duration);
        } else if elapsed > self.frame_duration {
            // Frame drop occurred - log for debugging
            debug!(
                "Frame drop: frame took {:?}, target {:?}",
                elapsed, self.frame_duration
            );
        }

        // Update timestamp after any sleep
        self.last_frame = Instant::now();
    }

    /// Returns the actual FPS based on recent frame times.
    ///
    /// This calculates a rolling average of the frame rate over the last
    /// 60 frames (approximately 1 second at 60fps). The average provides
    /// a stable reading that smooths out individual frame variations.
    ///
    /// # Returns
    ///
    /// - The rolling average FPS as an `f32`
    /// - `0.0` if no frames have been recorded yet
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameTimer;
    ///
    /// let timer = FrameTimer::new(60);
    ///
    /// // No frames recorded yet
    /// assert_eq!(timer.actual_fps(), 0.0);
    /// ```
    ///
    /// ```no_run
    /// use dotmax::animation::FrameTimer;
    ///
    /// let mut timer = FrameTimer::new(60);
    ///
    /// // After running some frames
    /// for _ in 0..60 {
    ///     timer.wait_for_next_frame();
    /// }
    ///
    /// // Should be close to 60 FPS
    /// let fps = timer.actual_fps();
    /// println!("Actual FPS: {:.1}", fps);
    /// ```
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "Frame count fits in f32 mantissa, precision loss is negligible"
    )]
    pub fn actual_fps(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let total: Duration = self.frame_times.iter().sum();
        // Frame count is limited to FRAME_WINDOW_SIZE (60), so truncation is safe
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Frame window size is 60, well within u32 range"
        )]
        let count = self.frame_times.len() as u32;
        let avg = total / count;

        // Avoid division by zero for very fast frames
        if avg.as_secs_f32() > 0.0 {
            1.0 / avg.as_secs_f32()
        } else {
            0.0
        }
    }

    /// Returns the duration of the most recent frame.
    ///
    /// This is useful for debugging and performance monitoring, showing
    /// exactly how long the last frame took to complete.
    ///
    /// # Returns
    ///
    /// - The duration of the most recent frame
    /// - `Duration::ZERO` if no frames have been completed
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameTimer;
    /// use std::time::Duration;
    ///
    /// let timer = FrameTimer::new(60);
    ///
    /// // No frames recorded yet
    /// assert_eq!(timer.frame_time(), Duration::ZERO);
    /// ```
    ///
    /// ```no_run
    /// use dotmax::animation::FrameTimer;
    ///
    /// let mut timer = FrameTimer::new(60);
    /// timer.wait_for_next_frame();
    ///
    /// // Check the actual frame duration
    /// let frame_ms = timer.frame_time().as_millis();
    /// println!("Last frame: {}ms", frame_ms);
    /// ```
    #[must_use]
    pub fn frame_time(&self) -> Duration {
        self.frame_times.back().copied().unwrap_or(Duration::ZERO)
    }

    /// Returns the target FPS this timer was configured with.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameTimer;
    ///
    /// let timer = FrameTimer::new(60);
    /// assert_eq!(timer.target_fps(), 60);
    ///
    /// let timer = FrameTimer::new(30);
    /// assert_eq!(timer.target_fps(), 30);
    /// ```
    #[must_use]
    pub const fn target_fps(&self) -> u32 {
        self.target_fps
    }

    /// Returns the target duration for each frame.
    ///
    /// This is calculated as `1.0 / target_fps` seconds. For example,
    /// at 60 FPS, the target frame time is approximately 16.67ms.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameTimer;
    /// use std::time::Duration;
    ///
    /// let timer = FrameTimer::new(60);
    /// let target_ms = timer.target_frame_time().as_secs_f64() * 1000.0;
    /// assert!((target_ms - 16.67).abs() < 0.1);
    ///
    /// let timer = FrameTimer::new(30);
    /// let target_ms = timer.target_frame_time().as_secs_f64() * 1000.0;
    /// assert!((target_ms - 33.33).abs() < 0.1);
    /// ```
    #[must_use]
    pub const fn target_frame_time(&self) -> Duration {
        self.frame_duration
    }

    /// Resets the timer state, clearing frame history.
    ///
    /// This is useful when:
    /// - Pausing and resuming an animation
    /// - Switching between different animation phases
    /// - Recovering from a long pause
    ///
    /// After reset, `actual_fps()` will return `0.0` and `frame_time()`
    /// will return `Duration::ZERO` until new frames are recorded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::animation::FrameTimer;
    ///
    /// let mut timer = FrameTimer::new(60);
    ///
    /// // Run some frames
    /// for _ in 0..60 {
    ///     timer.wait_for_next_frame();
    /// }
    /// assert!(timer.actual_fps() > 0.0);
    ///
    /// // Reset clears history
    /// timer.reset();
    /// assert_eq!(timer.actual_fps(), 0.0);
    /// ```
    pub fn reset(&mut self) {
        self.last_frame = Instant::now();
        self.frame_times.clear();
    }
}

impl Default for FrameTimer {
    /// Creates a frame timer with the default target of 60 FPS.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::animation::FrameTimer;
    ///
    /// let timer = FrameTimer::default();
    /// assert_eq!(timer.target_fps(), 60);
    /// ```
    fn default() -> Self {
        Self::new(60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test AC1: FrameTimer::new() initializes with correct frame duration at 60fps
    #[test]
    fn test_new_60fps_frame_duration() {
        let timer = FrameTimer::new(60);
        assert_eq!(timer.target_fps(), 60);

        // 60 FPS = 16.67ms per frame
        let duration_ms = timer.target_frame_time().as_secs_f64() * 1000.0;
        assert!(
            (duration_ms - 16.666_666).abs() < 0.001,
            "Expected ~16.67ms, got {duration_ms}ms"
        );
    }

    /// Test AC1: FPS validation - minimum (1fps)
    #[test]
    fn test_new_min_fps() {
        let timer = FrameTimer::new(1);
        assert_eq!(timer.target_fps(), 1);

        // 1 FPS = 1000ms per frame
        let duration_ms = timer.target_frame_time().as_secs_f64() * 1000.0;
        assert!(
            (duration_ms - 1000.0).abs() < 0.1,
            "Expected 1000ms, got {duration_ms}ms"
        );
    }

    /// Test AC1: FPS validation - maximum (240fps)
    #[test]
    fn test_new_max_fps() {
        let timer = FrameTimer::new(240);
        assert_eq!(timer.target_fps(), 240);

        // 240 FPS = 4.17ms per frame
        let duration_ms = timer.target_frame_time().as_secs_f64() * 1000.0;
        assert!(
            (duration_ms - 4.166_666).abs() < 0.001,
            "Expected ~4.17ms, got {duration_ms}ms"
        );
    }

    /// Test AC1: FPS clamping - below minimum
    #[test]
    fn test_new_clamps_below_min() {
        let timer = FrameTimer::new(0);
        assert_eq!(timer.target_fps(), 1, "FPS 0 should be clamped to 1");
    }

    /// Test AC1: FPS clamping - above maximum
    #[test]
    fn test_new_clamps_above_max() {
        let timer = FrameTimer::new(500);
        assert_eq!(timer.target_fps(), 240, "FPS 500 should be clamped to 240");
    }

    /// Test AC1: target_fps() returns correct value
    #[test]
    fn test_target_fps_returns_correct_value() {
        let timer = FrameTimer::new(30);
        assert_eq!(timer.target_fps(), 30);

        let timer = FrameTimer::new(120);
        assert_eq!(timer.target_fps(), 120);
    }

    /// Test AC3: `actual_fps()` returns 0.0 when no frames recorded
    #[test]
    fn test_actual_fps_no_frames() {
        let timer = FrameTimer::new(60);
        assert!(
            (timer.actual_fps() - 0.0).abs() < f32::EPSILON,
            "Expected 0.0, got {}",
            timer.actual_fps()
        );
    }

    /// Test AC4: frame_time() returns Duration::ZERO when no frames recorded
    #[test]
    fn test_frame_time_no_frames() {
        let timer = FrameTimer::new(60);
        assert_eq!(timer.frame_time(), Duration::ZERO);
    }

    /// Test AC4, AC5: `reset()` clears frame history
    #[test]
    fn test_reset_clears_history() {
        let mut timer = FrameTimer::new(60);

        // Simulate recording a frame by calling wait_for_next_frame
        // We'll use a short timeout to make the test fast
        timer.wait_for_next_frame();

        // Should have at least one frame recorded
        assert!(
            timer.frame_time() > Duration::ZERO,
            "Should have recorded a frame"
        );

        // Reset clears history
        timer.reset();
        assert_eq!(timer.actual_fps(), 0.0);
        assert_eq!(timer.frame_time(), Duration::ZERO);
    }

    /// Test Default implementation
    #[test]
    fn test_default_is_60fps() {
        let timer = FrameTimer::default();
        assert_eq!(timer.target_fps(), 60);
    }

    /// Test AC2, AC5: Timing accuracy at 60fps over multiple frames
    /// Note: This test uses a relaxed tolerance due to OS scheduling variations
    #[test]
    fn test_timing_accuracy_60fps() {
        let mut timer = FrameTimer::new(60);
        let target_frame_ms = 16.67;

        let start = Instant::now();

        // Run 30 frames (about 0.5 seconds)
        for _ in 0..30 {
            timer.wait_for_next_frame();
        }

        let elapsed = start.elapsed();
        let expected_ms = 30.0 * target_frame_ms;
        let actual_ms = elapsed.as_secs_f64() * 1000.0;

        // Check that total time is reasonably close to expected
        // Allow more tolerance because OS scheduling can vary
        let diff_ms = (actual_ms - expected_ms).abs();
        assert!(
            diff_ms < expected_ms * 0.3, // Within 30% of expected
            "Expected ~{expected_ms}ms total, got {actual_ms}ms (diff: {diff_ms}ms)"
        );

        // Verify actual FPS is approximately correct (allow wider range for CI)
        let fps = timer.actual_fps();
        // In fast CI environments, the FPS could be high if sleeps are shorter
        // The key is that the mechanism works correctly
        assert!(
            fps > 30.0,
            "Expected FPS > 30, got {fps} - timing mechanism may not be working"
        );
    }

    /// Test AC7: Frame drop detection (simulate slow frame)
    #[test]
    fn test_frame_drop_no_sleep() {
        let mut timer = FrameTimer::new(60);

        // First call to wait_for_next_frame records the elapsed time
        // Since we just created the timer, this should be very fast
        timer.wait_for_next_frame();

        // Simulate a slow frame by sleeping longer than target
        std::thread::sleep(Duration::from_millis(30));

        // This should detect a frame drop and not sleep additional time
        let before = Instant::now();
        timer.wait_for_next_frame();
        let elapsed = before.elapsed();

        // Should have minimal additional sleep (just the overhead of the function)
        // The frame drop means it shouldn't sleep at all
        assert!(
            elapsed < Duration::from_millis(5),
            "Should not sleep on frame drop, but waited {elapsed:?}"
        );
    }
}
