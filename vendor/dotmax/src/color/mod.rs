//! Color conversion and escape code generation for terminal output.
//!
//! This module provides RGB to ANSI color conversion functions that enable
//! colored output across terminals with varying capability levels (16-color,
//! 256-color, or 24-bit true color).
//!
//! # Overview
//!
//! Terminal color support varies widely:
//! - **ANSI 16**: Basic 16-color palette (legacy terminals)
//! - **ANSI 256**: Extended 256-color palette (most modern terminals)
//! - **True Color**: Full 24-bit RGB (modern terminals like iTerm2, Windows Terminal)
//!
//! This module converts any RGB color to the appropriate format based on
//! detected terminal capabilities.
//!
//! # Examples
//!
//! ## Basic RGB to ANSI Conversion
//!
//! ```
//! use dotmax::color::convert::{rgb_to_ansi256, rgb_to_ansi16};
//!
//! // Convert bright red to ANSI 256 palette index
//! let ansi256 = rgb_to_ansi256(255, 0, 0);
//! assert_eq!(ansi256, 196);  // Bright red in color cube
//!
//! // Convert to basic 16-color palette
//! let ansi16 = rgb_to_ansi16(255, 0, 0);
//! assert_eq!(ansi16, 9);  // Bright red
//! ```
//!
//! ## Generating Escape Codes
//!
//! ```
//! use dotmax::color::convert::{rgb_to_truecolor_escape, ansi256_fg_escape, color_reset};
//!
//! // True color escape code
//! let escape = rgb_to_truecolor_escape(255, 128, 0);
//! assert_eq!(escape, "\x1b[38;2;255;128;0m");
//!
//! // ANSI 256 escape code
//! let escape = ansi256_fg_escape(196);
//! assert_eq!(escape, "\x1b[38;5;196m");
//!
//! // Reset colors
//! assert_eq!(color_reset(), "\x1b[0m");
//! ```
//!
//! ## Smart Conversion Based on Terminal Capability
//!
//! ```
//! use dotmax::color::convert::rgb_to_terminal_color;
//! use dotmax::ColorCapability;
//!
//! let r = 255;
//! let g = 128;
//! let b = 0;
//!
//! // True color terminal - uses full RGB
//! let escape = rgb_to_terminal_color(r, g, b, ColorCapability::TrueColor);
//! assert_eq!(escape, "\x1b[38;2;255;128;0m");
//!
//! // 256-color terminal - converts to nearest palette color
//! let escape = rgb_to_terminal_color(r, g, b, ColorCapability::Ansi256);
//! assert!(escape.starts_with("\x1b[38;5;"));
//!
//! // Monochrome terminal - returns empty string
//! let escape = rgb_to_terminal_color(r, g, b, ColorCapability::Monochrome);
//! assert_eq!(escape, "");
//! ```
//!
//! # Performance
//!
//! All conversion functions are optimized for performance:
//! - `rgb_to_ansi256`: <100ns per conversion
//! - `rgb_to_ansi16`: <50ns per conversion
//! - Escape code generation: <50ns per call
//!
//! # ANSI 256 Palette Structure
//!
//! The ANSI 256 palette is organized as follows:
//! - **Indices 0-15**: Standard 16 ANSI colors (system colors)
//! - **Indices 16-231**: 6×6×6 color cube (216 colors)
//! - **Indices 232-255**: 24-step grayscale ramp
//!
//! See [`convert`] module for detailed documentation.

pub mod apply;
pub mod convert;
pub mod scheme_builder;
pub mod schemes;

// Re-export commonly used functions
pub use convert::{
    ansi16_bg_escape, ansi16_fg_escape, ansi256_bg_escape, ansi256_fg_escape, color_reset,
    rgb_to_ansi16, rgb_to_ansi256, rgb_to_terminal_color, rgb_to_truecolor_bg_escape,
    rgb_to_truecolor_escape,
};

// Re-export color scheme types and functions
pub use schemes::{
    blue_purple, cyan_magenta, get_scheme, grayscale, green_yellow, heat_map, list_schemes,
    monochrome, rainbow, ColorScheme,
};

// Re-export scheme builder (Story 5.4)
pub use scheme_builder::ColorSchemeBuilder;

// Re-export apply functions (Story 5.5)
pub use apply::{apply_color_scheme, apply_colors_to_grid};
