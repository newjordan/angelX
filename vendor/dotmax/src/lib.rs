//! dotmax - High-performance terminal braille rendering
//!
#![warn(missing_docs)]
//!
//! This library provides braille-based rendering capabilities for terminal applications,
//! enabling images, animations, and graphics in any terminal environment.
//!
//! # Getting Started
//!
//! Create a braille grid and manipulate individual dots:
//!
//! ```
//! use dotmax::BrailleGrid;
//!
//! // Create an 80×24 braille grid (typical terminal size)
//! let mut grid = BrailleGrid::new(80, 24).unwrap();
//!
//! // Set individual dots using pixel coordinates
//! // Grid is 80×24 cells = 160×96 dots (2×4 dots per cell)
//! grid.set_dot(0, 0).unwrap(); // Top-left dot of first cell
//! grid.set_dot(1, 0).unwrap(); // Top-right dot of first cell
//!
//! // Query dimensions
//! let (width, height) = grid.dimensions();
//! ```
//!
//! # Logging
//!
//! Dotmax uses the [`tracing`](https://docs.rs/tracing) crate for structured logging.
//! The library does **not** initialize a tracing subscriber - your application must
//! do this if you want to see log output.
//!
//! To enable logging in your application:
//!
//! ```no_run
//! use tracing_subscriber;
//!
//! // Initialize the tracing subscriber (do this once at startup)
//! tracing_subscriber::fmt()
//!     .with_max_level(tracing::Level::DEBUG)
//!     .init();
//!
//! // Now dotmax operations will emit trace events
//! use dotmax::BrailleGrid;
//! let grid = BrailleGrid::new(80, 24).unwrap();  // Logs: "Creating BrailleGrid: 80×24"
//! ```
//!
//! **Log Levels:**
//! - `ERROR`: Operation failures (e.g., out-of-bounds errors)
//! - `WARN`: Degraded operation (e.g., terminal lacks Unicode support)
//! - `INFO`: Major operations (grid creation, rendering)
//! - `DEBUG`: Detailed flow (resize, color changes)
//! - `TRACE`: Hot path internals (not used by default for performance)
//!
//! For more information, see the [tracing documentation](https://docs.rs/tracing).
//!
//! # Thread Safety
//!
//! All types in dotmax are `Send` and `Sync` where possible:
//!
//! - [`BrailleGrid`]: `Send + Sync` (can be shared across threads)
//! - [`Color`]: `Send + Sync + Copy` (trivially thread-safe)
//! - [`ColorScheme`]: `Send + Sync` (immutable after creation)
//! - [`DotmaxError`]: `Send + Sync` (can be propagated across threads)
//! - [`TerminalRenderer`]: `Send` but **not** `Sync` (owns terminal handle)
//!
//! For concurrent rendering, create one [`TerminalRenderer`] per thread or use
//! a mutex to serialize access. [`BrailleGrid`] buffers can be prepared in
//! parallel and then rendered sequentially.
//!
//! # License
//!
//! Licensed under either of:
//! - MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
//! - Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
//!
//! at your option.

// Core modules (Epic 2)
pub mod error;
pub mod frame_export;
pub mod grid;
pub mod prelude;
pub mod progress;
pub mod quick;
pub mod render;

// Utility modules (Epic 5)
pub mod utils;

// Re-export public types for convenience
pub use error::DotmaxError;
pub use frame_export::{capture_frame, write_frame_pack, DotmaxFrame, DotmaxFramePack};
pub use grid::{BrailleGrid, Color};
pub use render::{TerminalBackend, TerminalCapabilities, TerminalRenderer, TerminalType};

// Re-export color capability detection (Epic 5)
pub use utils::terminal_caps::{detect_color_capability, ColorCapability};

// Re-export color scheme types (Epic 5, Story 5.3)
pub use color::schemes::{
    blue_purple, cyan_magenta, get_scheme, grayscale, green_yellow, heat_map, list_schemes,
    monochrome, rainbow, ColorScheme,
};

// Re-export color scheme builder (Epic 5, Story 5.4)
pub use color::scheme_builder::ColorSchemeBuilder;

// Re-export color application functions (Epic 5, Story 5.5)
pub use color::apply::{apply_color_scheme, apply_colors_to_grid};

// Re-export animation types (Epic 6, Stories 6.1, 6.2, 6.3, 6.4, 6.5)
pub use animation::{
    AnimationLoop, AnimationLoopBuilder, DifferentialRenderer, FrameBuffer, FrameTimer,
    PrerenderedAnimation,
};

// Re-export progress-bar types (modular loading bars)
pub use progress::{all_styles, styles_for_theme, BarContext, Easing, Palette, ProgressStyle};

/// Convenience type alias for Results using `DotmaxError`
///
/// This allows writing `dotmax::Result<T>` instead of `Result<T, DotmaxError>`
/// in applications using this library.
pub type Result<T> = std::result::Result<T, DotmaxError>;

// Feature modules (Epic 3+): image, primitives, density, color, animation
#[cfg(feature = "image")]
pub mod image;

// Media format detection and routing (Epic 9)
#[cfg(feature = "image")]
pub mod media;

// Drawing primitives (Epic 4)
pub mod primitives;

// Character density rendering (Epic 4)
pub mod density;

// Color conversion (Epic 5)
pub mod color;

// Animation & frame management (Epic 6)
pub mod animation;

// CPU raytracer with braille output (ported from crabmusic)
#[cfg(feature = "raytracer")]
pub mod raytracer;

// Chess board rendering (ported from dotchess)
#[cfg(feature = "chess")]
pub mod chess;

#[cfg(test)]
mod tests {
    #[test]
    fn test_ci_basic() {
        assert_eq!(1 + 1, 2); // Intentional formatting issue
    }
}
