//! Media content routing and playback infrastructure.
//!
//! This module provides the [`MediaContent`] enum for polymorphic handling
//! of different media types, and the [`MediaPlayer`] trait for animated
//! content playback.

use std::time::Duration;

use crate::{BrailleGrid, Result};

// ============================================================================
// MediaContent Enum (AC: #8)
// ============================================================================

/// Content loaded from a media file.
///
/// This enum provides a unified type for handling different media categories:
/// - Static images render to a single [`BrailleGrid`]
/// - Animated content implements [`MediaPlayer`] for frame-by-frame playback
///
/// # Examples
///
/// ```
/// use dotmax::media::MediaContent;
/// use dotmax::BrailleGrid;
///
/// // Handle media polymorphically
/// fn display_media(content: MediaContent) {
///     match content {
///         MediaContent::Static(grid) => {
///             println!("Static image: {}x{}", grid.width(), grid.height());
///         }
///         MediaContent::Animated(mut player) => {
///             while let Some(Ok((frame, duration))) = player.next_frame() {
///                 println!("Frame: {}x{}, duration: {:?}",
///                     frame.width(), frame.height(), duration);
///             }
///         }
///     }
/// }
/// ```
///
/// # Future Extensions
///
/// The `Animated` variant will be populated by:
/// - Story 9.2: Animated GIF playback (`GifPlayer`)
/// - Story 9.3: Animated PNG playback (`ApngPlayer`)
/// - Story 9.4: Video playback (`VideoPlayer`)
#[derive(Debug)]
pub enum MediaContent {
    /// Static image rendered to a braille grid.
    ///
    /// This variant is returned for:
    /// - PNG, JPEG, BMP, WebP, TIFF images
    /// - Static (single-frame) GIF files
    /// - SVG files (rasterized to pixels, then to braille)
    Static(BrailleGrid),

    /// Animated content with frame-by-frame playback.
    ///
    /// The boxed [`MediaPlayer`] trait object allows polymorphic handling
    /// of different animation sources (GIF, APNG, video).
    ///
    /// # Note
    ///
    /// This variant is a placeholder for Stories 9.2-9.4. Currently,
    /// all detected animated formats will return an error until those
    /// stories are implemented.
    Animated(Box<dyn MediaPlayer>),
}

// ============================================================================
// MediaPlayer Trait (AC: #8)
// ============================================================================

/// Trait for animated media playback.
///
/// Implementors provide frame-by-frame access to animated content,
/// enabling integration with dotmax's animation infrastructure
/// ([`AnimationLoop`](crate::AnimationLoop), [`FrameTimer`](crate::FrameTimer)).
///
/// # Frame Iteration
///
/// Use `next_frame()` to iterate through frames:
///
/// ```ignore
/// while let Some(Ok((frame, duration))) = player.next_frame() {
///     // Render frame
///     renderer.render(&frame)?;
///     // Wait for frame duration
///     std::thread::sleep(duration);
/// }
/// ```
///
/// # Looping
///
/// Call `reset()` to restart from the beginning. Check `loop_count()`
/// to determine if the animation should loop (0 = infinite).
///
/// # Thread Safety
///
/// `MediaPlayer` requires `Send` to allow moving players across threads.
/// Implementors should ensure thread-safe access to underlying resources.
///
/// # Future Implementation
///
/// This trait will be implemented by:
/// - `GifPlayer` (Story 9.2)
/// - `ApngPlayer` (Story 9.3)
/// - `VideoPlayer` (Story 9.4)
pub trait MediaPlayer: Send + std::fmt::Debug {
    /// Returns the next frame and its display duration.
    ///
    /// # Returns
    ///
    /// - `Some(Ok((grid, duration)))` - Next frame with recommended display time
    /// - `Some(Err(error))` - Frame decode error
    /// - `None` - No more frames (animation complete)
    ///
    /// # Frame Duration
    ///
    /// The returned `Duration` is the recommended display time for the frame.
    /// For GIFs, this comes from the frame's delay time. For videos, this is
    /// derived from the frame rate.
    fn next_frame(&mut self) -> Option<Result<(BrailleGrid, Duration)>>;

    /// Resets playback to the first frame.
    ///
    /// After calling `reset()`, `next_frame()` will return the first frame.
    fn reset(&mut self);

    /// Returns the total number of frames, if known.
    ///
    /// # Returns
    ///
    /// - `Some(count)` - Known frame count (GIF, APNG)
    /// - `None` - Unknown frame count (streaming video)
    fn frame_count(&self) -> Option<usize>;

    /// Returns the animation's loop count.
    ///
    /// # Returns
    ///
    /// - `Some(0)` - Infinite looping
    /// - `Some(n)` - Loop `n` times then stop
    /// - `None` - Loop behavior not specified (default to once)
    fn loop_count(&self) -> Option<u16>;

    /// Handles terminal resize events.
    ///
    /// Call this method when the terminal size changes to update the
    /// internal rendering dimensions. Subsequent frames will be rendered
    /// at the new size.
    ///
    /// # Arguments
    ///
    /// * `width` - New terminal width in cells
    /// * `height` - New terminal height in cells
    ///
    /// # Default Implementation
    ///
    /// The default implementation does nothing. Players that cache terminal
    /// dimensions should override this method.
    fn handle_resize(&mut self, _width: usize, _height: usize) {
        // Default: do nothing. Players can override to update their dimensions.
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test that MediaContent::Static works correctly
    #[test]
    fn test_media_content_static() {
        let grid = BrailleGrid::new(10, 10).unwrap();
        let content = MediaContent::Static(grid);

        match content {
            MediaContent::Static(g) => {
                assert_eq!(g.width(), 10);
                assert_eq!(g.height(), 10);
            }
            MediaContent::Animated(_) => panic!("Expected Static variant"),
        }
    }

    // Test Debug implementation for MediaContent
    #[test]
    fn test_media_content_debug() {
        let grid = BrailleGrid::new(5, 5).unwrap();
        let content = MediaContent::Static(grid);
        let debug_str = format!("{:?}", content);
        assert!(debug_str.contains("Static"));
    }
}
