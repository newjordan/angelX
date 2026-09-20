//! Terminal color capability detection.
//!
//! This module provides automatic detection of terminal color capabilities,
//! enabling color output to adapt to each terminal's support level without
//! manual configuration.
//!
//! # Overview
//!
//! Terminal color support varies widely across environments:
//! - **Monochrome**: No color support (rare in modern terminals)
//! - **ANSI 16**: Basic 16-color palette (legacy terminals)
//! - **ANSI 256**: Extended 256-color palette (most terminals)
//! - **True Color**: Full 24-bit RGB (modern terminals like iTerm2, Windows Terminal)
//!
//! This module detects the appropriate level by examining environment variables
//! and caches the result for the entire process lifetime.
//!
//! # Detection Algorithm
//!
//! 1. Check `$COLORTERM` for "truecolor" or "24bit" → [`TrueColor`](ColorCapability::TrueColor)
//! 2. Check `$TERM` for "256color" → [`Ansi256`](ColorCapability::Ansi256)
//! 3. Check `$TERM` for "color" → [`Ansi16`](ColorCapability::Ansi16)
//! 4. Default fallback → [`Ansi256`](ColorCapability::Ansi256) (widely supported)
//!
//! # Performance
//!
//! Detection is cached using [`OnceLock`], so:
//! - **First call**: <1ms (environment variable reads)
//! - **Subsequent calls**: <1ns (cached result returned instantly)
//!
//! # Examples
//!
//! ```
//! use dotmax::utils::terminal_caps::{ColorCapability, detect_color_capability};
//!
//! // Detect terminal capability (result is cached)
//! let capability = detect_color_capability();
//!
//! // Check support levels
//! if capability.supports_color() {
//!     println!("Terminal supports color output");
//! }
//!
//! if capability.supports_truecolor() {
//!     println!("Terminal supports 24-bit true color!");
//! }
//!
//! // Pattern match for specific handling
//! match capability {
//!     ColorCapability::TrueColor => println!("Using full RGB colors"),
//!     ColorCapability::Ansi256 => println!("Using 256-color palette"),
//!     ColorCapability::Ansi16 => println!("Using basic 16 colors"),
//!     ColorCapability::Monochrome => println!("No color support"),
//! }
//! ```
//!
//! # Cross-Platform Support
//!
//! This module works on all major platforms:
//! - **Windows**: `PowerShell`, CMD, Windows Terminal
//! - **Linux**: bash, zsh, various terminal emulators
//! - **macOS**: iTerm2, Terminal.app, Alacritty
//!
//! Detection uses pure environment variable reading with no platform-specific code.

use std::sync::OnceLock;
use tracing::{debug, info, instrument};

/// Terminal color capability levels.
///
/// Represents the color support level of the current terminal, from no color
/// support (monochrome) to full 24-bit true color (16 million colors).
///
/// # Ordering
///
/// Variants are ordered from lowest to highest capability:
/// 1. [`Monochrome`](Self::Monochrome) - No color
/// 2. [`Ansi16`](Self::Ansi16) - 16 colors
/// 3. [`Ansi256`](Self::Ansi256) - 256 colors
/// 4. [`TrueColor`](Self::TrueColor) - 16 million colors
///
/// # Examples
///
/// ```
/// use dotmax::utils::terminal_caps::ColorCapability;
///
/// let cap = ColorCapability::TrueColor;
/// assert!(cap.supports_color());
/// assert!(cap.supports_truecolor());
///
/// let cap = ColorCapability::Monochrome;
/// assert!(!cap.supports_color());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorCapability {
    /// No color support (monochrome output only).
    ///
    /// This variant is rarely used in modern terminals but is provided
    /// for completeness. In practice, most terminals support at least
    /// 16 colors.
    Monochrome,

    /// 16 colors (basic ANSI palette).
    ///
    /// The classic ANSI color set: 8 colors (black, red, green, yellow,
    /// blue, magenta, cyan, white) plus 8 bright variants. Supported by
    /// virtually all color terminals.
    Ansi16,

    /// 256 colors (extended ANSI palette).
    ///
    /// Includes the 16 ANSI colors plus a 6×6×6 color cube (216 colors)
    /// and a 24-step grayscale ramp. This is the most common capability
    /// level in modern terminals.
    Ansi256,

    /// 24-bit true color (16 million colors).
    ///
    /// Full RGB color support with 8 bits per channel, allowing any of
    /// 16,777,216 colors. Supported by modern terminals like iTerm2,
    /// Windows Terminal, Alacritty, and others.
    TrueColor,
}

impl ColorCapability {
    /// Check if the terminal supports any color output.
    ///
    /// Returns `true` for all variants except [`Monochrome`](Self::Monochrome).
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::utils::terminal_caps::ColorCapability;
    ///
    /// assert!(!ColorCapability::Monochrome.supports_color());
    /// assert!(ColorCapability::Ansi16.supports_color());
    /// assert!(ColorCapability::Ansi256.supports_color());
    /// assert!(ColorCapability::TrueColor.supports_color());
    /// ```
    #[inline]
    #[must_use]
    pub const fn supports_color(&self) -> bool {
        !matches!(self, Self::Monochrome)
    }

    /// Check if the terminal supports 24-bit true color (RGB).
    ///
    /// Returns `true` only for [`TrueColor`](Self::TrueColor) variant.
    /// Use this to decide whether to output full RGB escape codes or
    /// fall back to palette-based colors.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::utils::terminal_caps::ColorCapability;
    ///
    /// assert!(!ColorCapability::Monochrome.supports_truecolor());
    /// assert!(!ColorCapability::Ansi16.supports_truecolor());
    /// assert!(!ColorCapability::Ansi256.supports_truecolor());
    /// assert!(ColorCapability::TrueColor.supports_truecolor());
    /// ```
    #[inline]
    #[must_use]
    pub const fn supports_truecolor(&self) -> bool {
        matches!(self, Self::TrueColor)
    }

    /// Detect current terminal capability.
    ///
    /// This is a convenience alias for [`detect_color_capability()`].
    /// The result is cached globally, so detection only happens once.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::utils::terminal_caps::ColorCapability;
    ///
    /// let capability = ColorCapability::detect();
    /// println!("Terminal supports: {:?}", capability);
    /// ```
    #[inline]
    #[must_use]
    pub fn detect() -> Self {
        detect_color_capability()
    }
}

impl Default for ColorCapability {
    /// Returns the default color capability ([`Ansi256`](Self::Ansi256)).
    ///
    /// This default is chosen because 256-color support is nearly universal
    /// in modern terminals, making it a safe choice when detection is uncertain.
    fn default() -> Self {
        Self::Ansi256
    }
}

impl std::fmt::Display for ColorCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Monochrome => write!(f, "Monochrome (no color)"),
            Self::Ansi16 => write!(f, "ANSI 16 colors"),
            Self::Ansi256 => write!(f, "ANSI 256 colors"),
            Self::TrueColor => write!(f, "True Color (24-bit RGB)"),
        }
    }
}

/// Global cache for detected capability.
///
/// Uses [`OnceLock`] for thread-safe, lazy initialization.
/// The value is computed once on first access and then returned
/// instantly on subsequent calls.
static DETECTED_CAPABILITY: OnceLock<ColorCapability> = OnceLock::new();

/// Detect terminal color capability from environment variables.
///
/// This function examines `$COLORTERM` and `$TERM` environment variables
/// to determine the terminal's color support level. The result is cached
/// globally, so detection only happens once per process.
///
/// # Detection Algorithm
///
/// 1. Check `$COLORTERM` for "truecolor" or "24bit" → [`TrueColor`](ColorCapability::TrueColor)
/// 2. Check `$TERM` for "256color" → [`Ansi256`](ColorCapability::Ansi256)
/// 3. Check `$TERM` for "color" → [`Ansi16`](ColorCapability::Ansi16)
/// 4. Default fallback → [`Ansi256`](ColorCapability::Ansi256) (widely supported)
///
/// # Performance
///
/// - **First call**: <1ms (environment variable reads)
/// - **Subsequent calls**: <1ns (cached result)
///
/// # Cross-Platform Support
///
/// Works on Windows, Linux, and macOS. Uses standard environment variable
/// reading with no platform-specific code. Missing environment variables
/// are handled gracefully.
///
/// # Examples
///
/// ```
/// use dotmax::detect_color_capability;
///
/// let capability = detect_color_capability();
/// println!("Terminal supports: {:?}", capability);
///
/// if capability.supports_truecolor() {
///     println!("Using 24-bit RGB colors");
/// }
/// ```
#[instrument(level = "debug")]
pub fn detect_color_capability() -> ColorCapability {
    *DETECTED_CAPABILITY.get_or_init(|| {
        let capability = detect_from_environment();
        info!(capability = ?capability, "Terminal color capability detected");
        capability
    })
}

/// Internal detection logic that reads environment variables.
///
/// This is separated from the public function to allow tracing instrumentation
/// on the cached wrapper while keeping the core logic clean.
fn detect_from_environment() -> ColorCapability {
    use std::env;

    // Check $COLORTERM for true color support
    // Modern terminals often set this to indicate 24-bit color
    if let Ok(colorterm) = env::var("COLORTERM") {
        debug!(colorterm = %colorterm, "Checking COLORTERM environment variable");
        let colorterm_lower = colorterm.to_lowercase();

        if colorterm_lower.contains("truecolor") || colorterm_lower.contains("24bit") {
            debug!("COLORTERM indicates TrueColor support");
            return ColorCapability::TrueColor;
        }

        // If COLORTERM is set but doesn't indicate true color,
        // it still suggests color support - check for specific values
        // that indicate 256 color support
        if colorterm_lower.contains("256") {
            debug!("COLORTERM indicates 256-color support");
            return ColorCapability::Ansi256;
        }
    }

    // Check $TERM for color level hints
    // This is the traditional way terminals advertise capabilities
    if let Ok(term) = env::var("TERM") {
        debug!(term = %term, "Checking TERM environment variable");
        let term_lower = term.to_lowercase();

        // Check for 256-color indicator (e.g., xterm-256color)
        if term_lower.contains("256color") {
            debug!("TERM indicates 256-color support");
            return ColorCapability::Ansi256;
        }

        // Check for basic color indicator (e.g., xterm-color)
        if term_lower.contains("color") {
            debug!("TERM indicates basic color support");
            return ColorCapability::Ansi16;
        }

        // Some terminal types imply color support even without "color" in name
        if term_lower.contains("xterm")
            || term_lower.contains("screen")
            || term_lower.contains("tmux")
            || term_lower.contains("vt100")
            || term_lower.contains("linux")
            || term_lower.contains("ansi")
        {
            debug!("TERM implies at least basic color support");
            return ColorCapability::Ansi16;
        }
    }

    // Safe fallback: Ansi256 is widely supported in modern terminals
    // We default to this rather than Monochrome because:
    // 1. Most modern terminals support at least 256 colors
    // 2. Outputting 256-color codes to a less capable terminal usually
    //    just results in degraded (but visible) output
    debug!("Using default fallback: Ansi256");
    ColorCapability::Ansi256
}

/// Detect color capability with explicit environment values (for testing).
///
/// This function allows testing the detection logic with specific environment
/// variable values without modifying the actual environment.
///
/// # Arguments
///
/// * `colorterm` - Optional value for `$COLORTERM`
/// * `term` - Optional value for `$TERM`
///
/// # Returns
///
/// The [`ColorCapability`] that would be detected with the given environment.
///
/// # Note
///
/// This function does NOT cache its result and is primarily intended for
/// testing purposes. For normal usage, use [`detect_color_capability()`].
#[must_use]
pub fn detect_with_env(colorterm: Option<&str>, term: Option<&str>) -> ColorCapability {
    // Check $COLORTERM for true color support
    if let Some(colorterm_val) = colorterm {
        let colorterm_lower = colorterm_val.to_lowercase();

        if colorterm_lower.contains("truecolor") || colorterm_lower.contains("24bit") {
            return ColorCapability::TrueColor;
        }

        if colorterm_lower.contains("256") {
            return ColorCapability::Ansi256;
        }
    }

    // Check $TERM for color level hints
    if let Some(term_val) = term {
        let term_lower = term_val.to_lowercase();

        if term_lower.contains("256color") {
            return ColorCapability::Ansi256;
        }

        if term_lower.contains("color") {
            return ColorCapability::Ansi16;
        }

        if term_lower.contains("xterm")
            || term_lower.contains("screen")
            || term_lower.contains("tmux")
            || term_lower.contains("vt100")
            || term_lower.contains("linux")
            || term_lower.contains("ansi")
        {
            return ColorCapability::Ansi16;
        }
    }

    // Safe fallback
    ColorCapability::Ansi256
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // AC1: ColorCapability Enum Tests
    // ============================================================

    #[test]
    fn test_color_capability_debug() {
        // Verify Debug derive works
        let cap = ColorCapability::TrueColor;
        let debug_str = format!("{:?}", cap);
        assert!(debug_str.contains("TrueColor"));
    }

    #[test]
    fn test_color_capability_clone() {
        // Verify Clone derive works (Copy types can also clone)
        let cap = ColorCapability::Ansi256;
        #[allow(clippy::clone_on_copy)]
        let cloned = cap.clone();
        assert_eq!(cap, cloned);
    }

    #[test]
    fn test_color_capability_copy() {
        // Verify Copy derive works (can pass by value without move)
        let cap = ColorCapability::Ansi16;
        let copied = cap; // Copy, not move
        assert_eq!(cap, copied); // Both still usable
    }

    #[test]
    fn test_color_capability_eq() {
        // Verify PartialEq and Eq derive works
        assert_eq!(ColorCapability::Monochrome, ColorCapability::Monochrome);
        assert_eq!(ColorCapability::Ansi16, ColorCapability::Ansi16);
        assert_eq!(ColorCapability::Ansi256, ColorCapability::Ansi256);
        assert_eq!(ColorCapability::TrueColor, ColorCapability::TrueColor);
        assert_ne!(ColorCapability::Monochrome, ColorCapability::TrueColor);
    }

    #[test]
    fn test_color_capability_hash() {
        // Verify Hash derive works (can be used in HashSet/HashMap)
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ColorCapability::TrueColor);
        set.insert(ColorCapability::Ansi256);
        assert!(set.contains(&ColorCapability::TrueColor));
        assert!(set.contains(&ColorCapability::Ansi256));
        assert!(!set.contains(&ColorCapability::Monochrome));
    }

    // ============================================================
    // AC1: supports_color() Tests
    // ============================================================

    #[test]
    fn test_supports_color_monochrome_returns_false() {
        assert!(!ColorCapability::Monochrome.supports_color());
    }

    #[test]
    fn test_supports_color_ansi16_returns_true() {
        assert!(ColorCapability::Ansi16.supports_color());
    }

    #[test]
    fn test_supports_color_ansi256_returns_true() {
        assert!(ColorCapability::Ansi256.supports_color());
    }

    #[test]
    fn test_supports_color_truecolor_returns_true() {
        assert!(ColorCapability::TrueColor.supports_color());
    }

    // ============================================================
    // AC1: supports_truecolor() Tests
    // ============================================================

    #[test]
    fn test_supports_truecolor_monochrome_returns_false() {
        assert!(!ColorCapability::Monochrome.supports_truecolor());
    }

    #[test]
    fn test_supports_truecolor_ansi16_returns_false() {
        assert!(!ColorCapability::Ansi16.supports_truecolor());
    }

    #[test]
    fn test_supports_truecolor_ansi256_returns_false() {
        assert!(!ColorCapability::Ansi256.supports_truecolor());
    }

    #[test]
    fn test_supports_truecolor_only_for_truecolor() {
        assert!(ColorCapability::TrueColor.supports_truecolor());
    }

    // ============================================================
    // AC1: ColorCapability::detect() alias
    // ============================================================

    #[test]
    fn test_detect_alias_returns_same_as_function() {
        // Both should return the same cached value
        let via_detect = ColorCapability::detect();
        let via_function = detect_color_capability();
        assert_eq!(via_detect, via_function);
    }

    // ============================================================
    // AC2: Environment Variable Detection Tests (using detect_with_env)
    // ============================================================

    #[test]
    fn test_detect_colorterm_truecolor() {
        let result = detect_with_env(Some("truecolor"), None);
        assert_eq!(result, ColorCapability::TrueColor);
    }

    #[test]
    fn test_detect_colorterm_truecolor_uppercase() {
        let result = detect_with_env(Some("TRUECOLOR"), None);
        assert_eq!(result, ColorCapability::TrueColor);
    }

    #[test]
    fn test_detect_colorterm_24bit() {
        let result = detect_with_env(Some("24bit"), None);
        assert_eq!(result, ColorCapability::TrueColor);
    }

    #[test]
    fn test_detect_colorterm_24bit_uppercase() {
        let result = detect_with_env(Some("24BIT"), None);
        assert_eq!(result, ColorCapability::TrueColor);
    }

    #[test]
    fn test_detect_term_256color() {
        let result = detect_with_env(None, Some("xterm-256color"));
        assert_eq!(result, ColorCapability::Ansi256);
    }

    #[test]
    fn test_detect_term_256color_uppercase() {
        let result = detect_with_env(None, Some("XTERM-256COLOR"));
        assert_eq!(result, ColorCapability::Ansi256);
    }

    #[test]
    fn test_detect_term_color() {
        let result = detect_with_env(None, Some("xterm-color"));
        assert_eq!(result, ColorCapability::Ansi16);
    }

    #[test]
    fn test_detect_term_plain_xterm() {
        // Plain xterm should still get Ansi16 as it supports basic colors
        let result = detect_with_env(None, Some("xterm"));
        assert_eq!(result, ColorCapability::Ansi16);
    }

    #[test]
    fn test_detect_term_screen() {
        let result = detect_with_env(None, Some("screen"));
        assert_eq!(result, ColorCapability::Ansi16);
    }

    #[test]
    fn test_detect_term_tmux() {
        let result = detect_with_env(None, Some("tmux-256color"));
        assert_eq!(result, ColorCapability::Ansi256);
    }

    #[test]
    fn test_detect_term_linux() {
        let result = detect_with_env(None, Some("linux"));
        assert_eq!(result, ColorCapability::Ansi16);
    }

    #[test]
    fn test_fallback_when_no_env_vars() {
        let result = detect_with_env(None, None);
        assert_eq!(result, ColorCapability::Ansi256);
    }

    #[test]
    fn test_colorterm_takes_precedence_over_term() {
        // COLORTERM=truecolor should win even if TERM suggests less
        let result = detect_with_env(Some("truecolor"), Some("xterm-color"));
        assert_eq!(result, ColorCapability::TrueColor);
    }

    #[test]
    fn test_colorterm_256_detected() {
        let result = detect_with_env(Some("256"), None);
        assert_eq!(result, ColorCapability::Ansi256);
    }

    // ============================================================
    // AC3: Caching Tests
    // ============================================================

    #[test]
    fn test_detect_returns_same_value_on_repeated_calls() {
        // OnceLock ensures the same value is returned
        let first = detect_color_capability();
        let second = detect_color_capability();
        let third = detect_color_capability();
        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    #[test]
    fn test_detect_is_deterministic() {
        // Multiple calls should always return the same result
        for _ in 0..100 {
            let result = detect_color_capability();
            assert_eq!(result, detect_color_capability());
        }
    }

    // ============================================================
    // AC6: Error Handling Tests
    // ============================================================

    #[test]
    fn test_no_panic_with_empty_colorterm() {
        // Empty string should not panic
        let result = detect_with_env(Some(""), None);
        assert_eq!(result, ColorCapability::Ansi256); // Fallback
    }

    #[test]
    fn test_no_panic_with_empty_term() {
        // Empty string should not panic
        let result = detect_with_env(None, Some(""));
        assert_eq!(result, ColorCapability::Ansi256); // Fallback
    }

    #[test]
    fn test_no_panic_with_unusual_values() {
        // Unusual but valid strings should not panic
        let result = detect_with_env(Some("some-random-value"), Some("unknown-term"));
        assert_eq!(result, ColorCapability::Ansi256); // Fallback
    }

    #[test]
    fn test_graceful_handling_of_unicode() {
        // Unicode in env vars should not panic
        let result = detect_with_env(Some("truecolor-"), Some("xterm-256color-"));
        assert_eq!(result, ColorCapability::TrueColor);
    }

    // ============================================================
    // Display and Default Tests
    // ============================================================

    #[test]
    fn test_display_monochrome() {
        let s = format!("{}", ColorCapability::Monochrome);
        assert_eq!(s, "Monochrome (no color)");
    }

    #[test]
    fn test_display_ansi16() {
        let s = format!("{}", ColorCapability::Ansi16);
        assert_eq!(s, "ANSI 16 colors");
    }

    #[test]
    fn test_display_ansi256() {
        let s = format!("{}", ColorCapability::Ansi256);
        assert_eq!(s, "ANSI 256 colors");
    }

    #[test]
    fn test_display_truecolor() {
        let s = format!("{}", ColorCapability::TrueColor);
        assert_eq!(s, "True Color (24-bit RGB)");
    }

    #[test]
    fn test_default() {
        assert_eq!(ColorCapability::default(), ColorCapability::Ansi256);
    }
}
