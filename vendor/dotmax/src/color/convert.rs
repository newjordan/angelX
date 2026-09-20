//! RGB to ANSI color conversion algorithms.
//!
//! This module implements color conversion functions that enable RGB colors
//! to render correctly across terminals with different capability levels.
//!
//! # ANSI 256 Palette Structure
//!
//! The ANSI 256-color palette is organized into three sections:
//!
//! ## Standard Colors (0-15)
//!
//! The first 16 colors are the standard ANSI colors:
//! ```text
//! 0: Black      1: Red        2: Green      3: Yellow
//! 4: Blue       5: Magenta    6: Cyan       7: White
//! 8-15: Bright variants of the above
//! ```
//!
//! ## 6×6×6 Color Cube (16-231)
//!
//! A cube of 216 colors with 6 levels per channel:
//! - Index = 16 + 36*r + 6*g + b (where r, g, b ∈ {0, 1, 2, 3, 4, 5})
//! - RGB values for each level: 0, 95, 135, 175, 215, 255
//!
//! ## Grayscale Ramp (232-255)
//!
//! 24 shades of gray from dark to light:
//! - Gray value = 8 + 10 * (index - 232)
//! - Values: 8, 18, 28, 38, ..., 238
//!
//! # ANSI 16 Color Mapping
//!
//! Basic 16-color palette uses intensity thresholding:
//! ```text
//! 0: Black       1: Dark Red      2: Dark Green    3: Dark Yellow
//! 4: Dark Blue   5: Dark Magenta  6: Dark Cyan     7: Light Gray
//! 8: Dark Gray   9: Bright Red    10: Bright Green 11: Bright Yellow
//! 12: Bright Blue 13: Bright Magenta 14: Bright Cyan 15: Bright White
//! ```
//!
//! # Escape Code Formats
//!
//! | Color Mode | Foreground | Background |
//! |------------|------------|------------|
//! | True Color | `\x1b[38;2;R;G;Bm` | `\x1b[48;2;R;G;Bm` |
//! | ANSI 256   | `\x1b[38;5;INDEXm` | `\x1b[48;5;INDEXm` |
//! | ANSI 16 (dark) | `\x1b[3Xm` | `\x1b[4Xm` |
//! | ANSI 16 (bright) | `\x1b[9Xm` | `\x1b[10Xm` |
//! | Reset | `\x1b[0m` | `\x1b[0m` |

use crate::utils::terminal_caps::ColorCapability;

// ============================================================================
// ANSI 256 Palette Definition (Task 2)
// ============================================================================

/// RGB values for each level in the 6×6×6 color cube.
///
/// These are the standard ANSI 256 color cube levels.
const COLOR_CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

// Standard 16 ANSI colors reference (not used in code, kept as documentation):
//  0: Black (0, 0, 0)           8: Dark Gray (128, 128, 128)
//  1: Dark Red (128, 0, 0)      9: Bright Red (255, 0, 0)
//  2: Dark Green (0, 128, 0)   10: Bright Green (0, 255, 0)
//  3: Dark Yellow (128, 128, 0) 11: Bright Yellow (255, 255, 0)
//  4: Dark Blue (0, 0, 128)    12: Bright Blue (0, 0, 255)
//  5: Dark Magenta (128, 0, 128) 13: Bright Magenta (255, 0, 255)
//  6: Dark Cyan (0, 128, 128)  14: Bright Cyan (0, 255, 255)
//  7: Light Gray (192, 192, 192) 15: Bright White (255, 255, 255)

// ============================================================================
// Color Distance Calculation
// ============================================================================

/// Calculate squared Euclidean distance between two RGB colors.
///
/// Uses squared distance to avoid the expensive sqrt operation.
/// This is sufficient for comparison purposes since if a² < b², then a < b.
#[inline]
const fn color_distance_squared(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> u32 {
    let dr = (r1 as i32) - (r2 as i32);
    let dg = (g1 as i32) - (g2 as i32);
    let db = (b1 as i32) - (b2 as i32);
    // Squared distances are always non-negative, so cast is safe
    #[allow(clippy::cast_sign_loss)]
    let result = (dr * dr + dg * dg + db * db) as u32;
    result
}

// ============================================================================
// RGB to ANSI 256 Conversion (Task 3)
// ============================================================================

/// Convert RGB color to the closest ANSI 256 palette index.
///
/// This function finds the nearest color in the ANSI 256 palette using
/// Euclidean distance in RGB space. It considers both the 6×6×6 color cube
/// (indices 16-231) and the grayscale ramp (indices 232-255).
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
///
/// # Returns
///
/// ANSI 256 palette index (0-255)
///
/// # Performance
///
/// This function is optimized for performance:
/// - Uses squared distance (avoids sqrt)
/// - Uses integer arithmetic only
/// - Target: <100ns per conversion
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::rgb_to_ansi256;
///
/// // Pure colors
/// assert_eq!(rgb_to_ansi256(255, 0, 0), 196);   // Bright red
/// assert_eq!(rgb_to_ansi256(0, 255, 0), 46);    // Bright green
/// assert_eq!(rgb_to_ansi256(0, 0, 255), 21);    // Bright blue
///
/// // Black and white
/// assert_eq!(rgb_to_ansi256(0, 0, 0), 16);      // Black (color cube)
/// assert_eq!(rgb_to_ansi256(255, 255, 255), 231); // White (color cube)
///
/// // Grayscale
/// assert_eq!(rgb_to_ansi256(128, 128, 128), 244); // Mid-gray
/// ```
#[inline]
#[must_use]
pub fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Strategy: Find closest match in both color cube and grayscale ramp,
    // then return whichever is closer.

    // Find closest color in 6×6×6 color cube (indices 16-231)
    let cube_r = find_closest_cube_level(r);
    let cube_g = find_closest_cube_level(g);
    let cube_b = find_closest_cube_level(b);
    let cube_index = 16 + 36 * cube_r + 6 * cube_g + cube_b;
    let cube_color = (
        COLOR_CUBE_LEVELS[cube_r as usize],
        COLOR_CUBE_LEVELS[cube_g as usize],
        COLOR_CUBE_LEVELS[cube_b as usize],
    );
    let cube_distance = color_distance_squared(r, g, b, cube_color.0, cube_color.1, cube_color.2);

    // Find closest color in grayscale ramp (indices 232-255)
    let gray_avg = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    // gray_avg is in range 0-255 since r+g+b <= 765 and 765/3 = 255
    #[allow(clippy::cast_possible_truncation)]
    let gray_avg_u8 = gray_avg as u8;
    let gray_index = find_closest_gray_index(gray_avg_u8);
    let gray_value = gray_index_to_rgb(gray_index);
    let gray_distance = color_distance_squared(r, g, b, gray_value, gray_value, gray_value);

    // Return the closer match
    if gray_distance < cube_distance {
        gray_index
    } else {
        cube_index
    }
}

/// Find the closest level (0-5) in the color cube for a given RGB value.
#[inline]
const fn find_closest_cube_level(value: u8) -> u8 {
    // Color cube levels: 0, 95, 135, 175, 215, 255
    // Find closest by checking boundaries
    match value {
        0..=47 => 0,    // 0 is closest (midpoint to 95 is 47.5)
        48..=114 => 1,  // 95 is closest (midpoint between 95 and 135 is 115)
        115..=154 => 2, // 135 is closest
        155..=194 => 3, // 175 is closest
        195..=234 => 4, // 215 is closest
        235..=255 => 5, // 255 is closest
    }
}

/// Find the closest grayscale index (232-255) for a given gray value.
#[inline]
const fn find_closest_gray_index(gray: u8) -> u8 {
    // Grayscale ramp: indices 232-255
    // Values: 8, 18, 28, ..., 238 (step of 10)
    // gray_value = 8 + 10 * (index - 232)
    // So: index = 232 + (gray - 8) / 10

    // Handle boundary cases
    if gray < 4 {
        // Very dark - closest to black (but grayscale starts at 8)
        // Return first grayscale index
        232
    } else if gray > 243 {
        // Very light - closest to white
        255
    } else {
        // Normal case: find closest
        let offset = gray.saturating_sub(8);
        let index = (offset + 5) / 10; // Round to nearest
        let capped = if index > 23 { 23 } else { index };
        232 + capped
    }
}

/// Convert grayscale index (232-255) to RGB value.
#[inline]
const fn gray_index_to_rgb(index: u8) -> u8 {
    // gray_value = 8 + 10 * (index - 232)
    8 + 10 * (index.saturating_sub(232))
}

// ============================================================================
// RGB to ANSI 16 Conversion (Task 4)
// ============================================================================

/// Convert RGB color to the closest ANSI 16-color palette index.
///
/// This function maps RGB values to the basic 16 ANSI colors using
/// a simple thresholding algorithm based on channel intensity.
///
/// # Algorithm
///
/// 1. For each channel (R, G, B), determine if it's "high" (>127) or "low" (≤127)
/// 2. Combine the three binary values to select from 8 base colors
/// 3. Use maximum channel value to determine bright vs dark variant
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
///
/// # Returns
///
/// ANSI 16-color palette index (0-15)
///
/// # Performance
///
/// This function is very fast (simple bit operations):
/// - Target: <50ns per conversion
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::rgb_to_ansi16;
///
/// // Primary colors (bright)
/// assert_eq!(rgb_to_ansi16(255, 0, 0), 9);     // Bright red
/// assert_eq!(rgb_to_ansi16(0, 255, 0), 10);    // Bright green
/// assert_eq!(rgb_to_ansi16(0, 0, 255), 12);    // Bright blue
///
/// // Black and white
/// assert_eq!(rgb_to_ansi16(0, 0, 0), 0);       // Black
/// assert_eq!(rgb_to_ansi16(255, 255, 255), 15); // Bright white
///
/// // Dark variants
/// assert_eq!(rgb_to_ansi16(128, 0, 0), 1);     // Dark red
/// ```
#[inline]
#[must_use]
pub fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> u8 {
    // Determine which channels are "high" (threshold at 128)
    let r_high = r > 127;
    let g_high = g > 127;
    let b_high = b > 127;

    // Determine brightness by max channel value
    // A saturated color like (255, 0, 0) should be "bright red", not "dark red"
    let max_channel = r.max(g).max(b);
    let is_bright = max_channel > 191; // Threshold for bright variant (3/4 of 255)

    // Map to base color (0-7)
    // ANSI color order: black, red, green, yellow, blue, magenta, cyan, white
    let base = match (r_high, g_high, b_high) {
        (false, false, false) => 0, // Black
        (true, false, false) => 1,  // Red
        (false, true, false) => 2,  // Green
        (true, true, false) => 3,   // Yellow
        (false, false, true) => 4,  // Blue
        (true, false, true) => 5,   // Magenta
        (false, true, true) => 6,   // Cyan
        (true, true, true) => 7,    // White
    };

    // Add 8 for bright variant (except for pure black which stays black)
    if is_bright && base > 0 {
        base + 8
    } else if is_bright && base == 0 {
        // Bright black is dark gray (8)
        8
    } else {
        base
    }
}

// ============================================================================
// True Color Escape Code Functions (Task 5)
// ============================================================================

/// Generate a true color (24-bit) foreground escape code.
///
/// Produces an ANSI escape sequence for setting the foreground color
/// to the specified RGB value. Only works on terminals that support
/// 24-bit color (`TrueColor` capability).
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
///
/// # Returns
///
/// ANSI escape sequence string in the format `\x1b[38;2;R;G;Bm`
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::rgb_to_truecolor_escape;
///
/// let escape = rgb_to_truecolor_escape(255, 128, 0);
/// assert_eq!(escape, "\x1b[38;2;255;128;0m");
///
/// let escape = rgb_to_truecolor_escape(0, 0, 0);
/// assert_eq!(escape, "\x1b[38;2;0;0;0m");
/// ```
#[inline]
#[must_use]
pub fn rgb_to_truecolor_escape(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// Generate a true color (24-bit) background escape code.
///
/// Produces an ANSI escape sequence for setting the background color
/// to the specified RGB value. Only works on terminals that support
/// 24-bit color (`TrueColor` capability).
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
///
/// # Returns
///
/// ANSI escape sequence string in the format `\x1b[48;2;R;G;Bm`
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::rgb_to_truecolor_bg_escape;
///
/// let escape = rgb_to_truecolor_bg_escape(255, 128, 0);
/// assert_eq!(escape, "\x1b[48;2;255;128;0m");
/// ```
#[inline]
#[must_use]
pub fn rgb_to_truecolor_bg_escape(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{r};{g};{b}m")
}

// ============================================================================
// ANSI 256 Escape Code Functions (Task 6)
// ============================================================================

/// Generate an ANSI 256-color foreground escape code.
///
/// # Arguments
///
/// * `index` - ANSI 256 palette index (0-255)
///
/// # Returns
///
/// ANSI escape sequence string in the format `\x1b[38;5;INDEXm`
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::ansi256_fg_escape;
///
/// let escape = ansi256_fg_escape(196);
/// assert_eq!(escape, "\x1b[38;5;196m");
/// ```
#[inline]
#[must_use]
pub fn ansi256_fg_escape(index: u8) -> String {
    format!("\x1b[38;5;{index}m")
}

/// Generate an ANSI 256-color background escape code.
///
/// # Arguments
///
/// * `index` - ANSI 256 palette index (0-255)
///
/// # Returns
///
/// ANSI escape sequence string in the format `\x1b[48;5;INDEXm`
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::ansi256_bg_escape;
///
/// let escape = ansi256_bg_escape(196);
/// assert_eq!(escape, "\x1b[48;5;196m");
/// ```
#[inline]
#[must_use]
pub fn ansi256_bg_escape(index: u8) -> String {
    format!("\x1b[48;5;{index}m")
}

// ============================================================================
// ANSI 16 Escape Code Functions (Task 6)
// ============================================================================

/// Generate an ANSI 16-color foreground escape code.
///
/// Uses standard ANSI escape codes:
/// - Colors 0-7 (dark): `\x1b[3Xm` where X is the color code
/// - Colors 8-15 (bright): `\x1b[9Xm` where X is (color - 8)
///
/// # Arguments
///
/// * `code` - ANSI 16 color index (0-15)
///
/// # Returns
///
/// ANSI escape sequence string
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::ansi16_fg_escape;
///
/// // Dark red (color 1)
/// let escape = ansi16_fg_escape(1);
/// assert_eq!(escape, "\x1b[31m");
///
/// // Bright red (color 9)
/// let escape = ansi16_fg_escape(9);
/// assert_eq!(escape, "\x1b[91m");
/// ```
#[inline]
#[must_use]
pub fn ansi16_fg_escape(code: u8) -> String {
    if code < 8 {
        // Dark colors: \x1b[30m through \x1b[37m
        format!("\x1b[3{code}m")
    } else {
        // Bright colors: \x1b[90m through \x1b[97m
        format!("\x1b[9{}m", code - 8)
    }
}

/// Generate an ANSI 16-color background escape code.
///
/// Uses standard ANSI escape codes:
/// - Colors 0-7 (dark): `\x1b[4Xm` where X is the color code
/// - Colors 8-15 (bright): `\x1b[10Xm` where X is (color - 8)
///
/// # Arguments
///
/// * `code` - ANSI 16 color index (0-15)
///
/// # Returns
///
/// ANSI escape sequence string
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::ansi16_bg_escape;
///
/// // Dark red (color 1)
/// let escape = ansi16_bg_escape(1);
/// assert_eq!(escape, "\x1b[41m");
///
/// // Bright red (color 9)
/// let escape = ansi16_bg_escape(9);
/// assert_eq!(escape, "\x1b[101m");
/// ```
#[inline]
#[must_use]
pub fn ansi16_bg_escape(code: u8) -> String {
    if code < 8 {
        // Dark colors: \x1b[40m through \x1b[47m
        format!("\x1b[4{code}m")
    } else {
        // Bright colors: \x1b[100m through \x1b[107m
        format!("\x1b[10{}m", code - 8)
    }
}

// ============================================================================
// Color Reset (Task 6)
// ============================================================================

/// Return the ANSI escape code to reset all colors and attributes.
///
/// This returns a static string slice (`&'static str`) to avoid allocation.
/// Use this after each colored output to reset the terminal to its default state.
///
/// # Returns
///
/// Static string `\x1b[0m` for resetting colors
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::color_reset;
///
/// let reset = color_reset();
/// assert_eq!(reset, "\x1b[0m");
///
/// // Typical usage pattern
/// print!("{}{}{}", "\x1b[38;2;255;0;0m", "Red text", color_reset());
/// ```
#[inline]
#[must_use]
pub const fn color_reset() -> &'static str {
    "\x1b[0m"
}

// ============================================================================
// Smart Conversion Function (Task 7)
// ============================================================================

/// Convert RGB color to the appropriate escape code based on terminal capability.
///
/// This function automatically selects the best conversion based on the
/// terminal's detected color capability:
///
/// - **`TrueColor`**: Returns full RGB escape code
/// - **`Ansi256`**: Converts to nearest 256-color palette entry
/// - **`Ansi16`**: Converts to nearest 16-color palette entry
/// - **`Monochrome`**: Returns empty string
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
/// * `capability` - Terminal color capability level
///
/// # Returns
///
/// ANSI escape sequence string appropriate for the terminal capability,
/// or empty string for monochrome terminals.
///
/// # Examples
///
/// ```
/// use dotmax::color::convert::rgb_to_terminal_color;
/// use dotmax::ColorCapability;
///
/// let r = 255;
/// let g = 128;
/// let b = 0;
///
/// // True color terminal
/// let escape = rgb_to_terminal_color(r, g, b, ColorCapability::TrueColor);
/// assert_eq!(escape, "\x1b[38;2;255;128;0m");
///
/// // 256-color terminal
/// let escape = rgb_to_terminal_color(r, g, b, ColorCapability::Ansi256);
/// assert!(escape.starts_with("\x1b[38;5;"));
///
/// // 16-color terminal
/// let escape = rgb_to_terminal_color(r, g, b, ColorCapability::Ansi16);
/// assert!(escape.starts_with("\x1b[") && (escape.contains("3") || escape.contains("9")));
///
/// // Monochrome terminal
/// let escape = rgb_to_terminal_color(r, g, b, ColorCapability::Monochrome);
/// assert_eq!(escape, "");
/// ```
#[inline]
#[must_use]
pub fn rgb_to_terminal_color(r: u8, g: u8, b: u8, capability: ColorCapability) -> String {
    match capability {
        ColorCapability::TrueColor => rgb_to_truecolor_escape(r, g, b),
        ColorCapability::Ansi256 => ansi256_fg_escape(rgb_to_ansi256(r, g, b)),
        ColorCapability::Ansi16 => ansi16_fg_escape(rgb_to_ansi16(r, g, b)),
        ColorCapability::Monochrome => String::new(),
    }
}

// ============================================================================
// Unit Tests (Task 8)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // AC1: RGB-to-ANSI256 Conversion Tests
    // ========================================================================

    #[test]
    fn test_rgb_to_ansi256_pure_red() {
        // Pure bright red (255, 0, 0) → ANSI 196
        // Color cube: r=5, g=0, b=0 → 16 + 36*5 + 6*0 + 0 = 196
        assert_eq!(rgb_to_ansi256(255, 0, 0), 196);
    }

    #[test]
    fn test_rgb_to_ansi256_pure_green() {
        // Pure bright green (0, 255, 0) → ANSI 46
        // Color cube: r=0, g=5, b=0 → 16 + 36*0 + 6*5 + 0 = 46
        assert_eq!(rgb_to_ansi256(0, 255, 0), 46);
    }

    #[test]
    fn test_rgb_to_ansi256_pure_blue() {
        // Pure bright blue (0, 0, 255) → ANSI 21
        // Color cube: r=0, g=0, b=5 → 16 + 36*0 + 6*0 + 5 = 21
        assert_eq!(rgb_to_ansi256(0, 0, 255), 21);
    }

    #[test]
    fn test_rgb_to_ansi256_black() {
        // Black (0, 0, 0) → ANSI 16 (color cube black)
        // Color cube: r=0, g=0, b=0 → 16 + 0 + 0 + 0 = 16
        assert_eq!(rgb_to_ansi256(0, 0, 0), 16);
    }

    #[test]
    fn test_rgb_to_ansi256_white() {
        // White (255, 255, 255) → ANSI 231 (color cube white)
        // Color cube: r=5, g=5, b=5 → 16 + 180 + 30 + 5 = 231
        assert_eq!(rgb_to_ansi256(255, 255, 255), 231);
    }

    #[test]
    fn test_rgb_to_ansi256_gray() {
        // Mid-gray (128, 128, 128) → Should be in grayscale ramp
        // Average = 128, closest grayscale is around index 244 (gray value 128)
        let result = rgb_to_ansi256(128, 128, 128);
        // Should be in grayscale range (232-255). Upper bound check omitted since u8 max is 255.
        assert!(
            result >= 232,
            "Expected grayscale index (232-255), got {result}"
        );
    }

    #[test]
    fn test_rgb_to_ansi256_gray_exact_match() {
        // Gray value 128: closest is index 244 (value 128)
        // Index 244: 8 + 10 * (244 - 232) = 8 + 10 * 12 = 128
        assert_eq!(rgb_to_ansi256(128, 128, 128), 244);
    }

    #[test]
    fn test_rgb_to_ansi256_dark_gray() {
        // Dark gray (64, 64, 64)
        let result = rgb_to_ansi256(64, 64, 64);
        // Should be in grayscale range (232-255). Upper bound check omitted since u8 max is 255.
        assert!(result >= 232, "Expected grayscale index, got {result}");
    }

    #[test]
    fn test_rgb_to_ansi256_light_gray() {
        // Light gray (192, 192, 192)
        let result = rgb_to_ansi256(192, 192, 192);
        // Should be in grayscale range (232-255). Upper bound check omitted since u8 max is 255.
        assert!(result >= 232, "Expected grayscale index, got {result}");
    }

    #[test]
    fn test_rgb_to_ansi256_yellow() {
        // Yellow (255, 255, 0) → ANSI 226
        // Color cube: r=5, g=5, b=0 → 16 + 180 + 30 + 0 = 226
        assert_eq!(rgb_to_ansi256(255, 255, 0), 226);
    }

    #[test]
    fn test_rgb_to_ansi256_cyan() {
        // Cyan (0, 255, 255) → ANSI 51
        // Color cube: r=0, g=5, b=5 → 16 + 0 + 30 + 5 = 51
        assert_eq!(rgb_to_ansi256(0, 255, 255), 51);
    }

    #[test]
    fn test_rgb_to_ansi256_magenta() {
        // Magenta (255, 0, 255) → ANSI 201
        // Color cube: r=5, g=0, b=5 → 16 + 180 + 0 + 5 = 201
        assert_eq!(rgb_to_ansi256(255, 0, 255), 201);
    }

    // ========================================================================
    // AC2: RGB-to-ANSI16 Conversion Tests
    // ========================================================================

    #[test]
    fn test_rgb_to_ansi16_bright_red() {
        // Bright red (255, 0, 0) → 9
        assert_eq!(rgb_to_ansi16(255, 0, 0), 9);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_green() {
        // Bright green (0, 255, 0) → 10
        assert_eq!(rgb_to_ansi16(0, 255, 0), 10);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_blue() {
        // Bright blue (0, 0, 255) → 12
        assert_eq!(rgb_to_ansi16(0, 0, 255), 12);
    }

    #[test]
    fn test_rgb_to_ansi16_black() {
        // Black (0, 0, 0) → 0
        assert_eq!(rgb_to_ansi16(0, 0, 0), 0);
    }

    #[test]
    fn test_rgb_to_ansi16_white() {
        // White (255, 255, 255) → 15 (bright white)
        assert_eq!(rgb_to_ansi16(255, 255, 255), 15);
    }

    #[test]
    fn test_rgb_to_ansi16_dark_red() {
        // Dark red (128, 0, 0) → 1
        // R > 127, G <= 127, B <= 127, brightness = 42 <= 127 → dark red
        assert_eq!(rgb_to_ansi16(128, 0, 0), 1);
    }

    #[test]
    fn test_rgb_to_ansi16_dark_green() {
        // Dark green (0, 128, 0) → 2
        assert_eq!(rgb_to_ansi16(0, 128, 0), 2);
    }

    #[test]
    fn test_rgb_to_ansi16_dark_blue() {
        // Dark blue (0, 0, 128) → 4
        assert_eq!(rgb_to_ansi16(0, 0, 128), 4);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_yellow() {
        // Bright yellow (255, 255, 0) → 11
        assert_eq!(rgb_to_ansi16(255, 255, 0), 11);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_cyan() {
        // Bright cyan (0, 255, 255) → 14
        assert_eq!(rgb_to_ansi16(0, 255, 255), 14);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_magenta() {
        // Bright magenta (255, 0, 255) → 13
        assert_eq!(rgb_to_ansi16(255, 0, 255), 13);
    }

    #[test]
    fn test_rgb_to_ansi16_dark_gray() {
        // Dark gray (64, 64, 64) → 0 (black) since all channels <= 127 and brightness <= 127
        assert_eq!(rgb_to_ansi16(64, 64, 64), 0);
    }

    #[test]
    fn test_rgb_to_ansi16_light_gray() {
        // Light gray (192, 192, 192) → 15 (bright white)
        assert_eq!(rgb_to_ansi16(192, 192, 192), 15);
    }

    #[test]
    fn test_rgb_to_ansi16_mid_gray() {
        // Mid gray (128, 128, 128) → 7 (light gray)
        // max_channel = 128 <= 191, so not bright
        // All channels > 127, so base = 7 (white/light gray)
        assert_eq!(rgb_to_ansi16(128, 128, 128), 7);
    }

    // ========================================================================
    // AC3: True Color Escape Code Tests
    // ========================================================================

    #[test]
    fn test_rgb_to_truecolor_escape_format() {
        let escape = rgb_to_truecolor_escape(255, 128, 0);
        assert_eq!(escape, "\x1b[38;2;255;128;0m");
    }

    #[test]
    fn test_rgb_to_truecolor_escape_black() {
        let escape = rgb_to_truecolor_escape(0, 0, 0);
        assert_eq!(escape, "\x1b[38;2;0;0;0m");
    }

    #[test]
    fn test_rgb_to_truecolor_escape_white() {
        let escape = rgb_to_truecolor_escape(255, 255, 255);
        assert_eq!(escape, "\x1b[38;2;255;255;255m");
    }

    #[test]
    fn test_rgb_to_truecolor_bg_escape_format() {
        let escape = rgb_to_truecolor_bg_escape(255, 128, 0);
        assert_eq!(escape, "\x1b[48;2;255;128;0m");
    }

    #[test]
    fn test_truecolor_fg_vs_bg_different() {
        let fg = rgb_to_truecolor_escape(255, 0, 0);
        let bg = rgb_to_truecolor_bg_escape(255, 0, 0);
        assert_ne!(fg, bg);
        assert!(fg.contains("38;2;"));
        assert!(bg.contains("48;2;"));
    }

    // ========================================================================
    // AC4: Smart Conversion Tests
    // ========================================================================

    #[test]
    fn test_rgb_to_terminal_color_truecolor() {
        let escape = rgb_to_terminal_color(255, 128, 0, ColorCapability::TrueColor);
        assert_eq!(escape, "\x1b[38;2;255;128;0m");
    }

    #[test]
    fn test_rgb_to_terminal_color_ansi256() {
        let escape = rgb_to_terminal_color(255, 128, 0, ColorCapability::Ansi256);
        assert!(escape.starts_with("\x1b[38;5;"));
    }

    #[test]
    fn test_rgb_to_terminal_color_ansi16() {
        let escape = rgb_to_terminal_color(255, 0, 0, ColorCapability::Ansi16);
        // Bright red → 9 → \x1b[91m
        assert_eq!(escape, "\x1b[91m");
    }

    #[test]
    fn test_rgb_to_terminal_color_monochrome() {
        let escape = rgb_to_terminal_color(255, 128, 0, ColorCapability::Monochrome);
        assert_eq!(escape, "");
    }

    // ========================================================================
    // AC5: Background Color Escape Tests
    // ========================================================================

    #[test]
    fn test_ansi256_bg_escape_format() {
        let escape = ansi256_bg_escape(196);
        assert_eq!(escape, "\x1b[48;5;196m");
    }

    #[test]
    fn test_ansi16_bg_escape_dark() {
        let escape = ansi16_bg_escape(1);
        assert_eq!(escape, "\x1b[41m");
    }

    #[test]
    fn test_ansi16_bg_escape_bright() {
        let escape = ansi16_bg_escape(9);
        assert_eq!(escape, "\x1b[101m");
    }

    #[test]
    fn test_fg_vs_bg_escape_different() {
        let fg256 = ansi256_fg_escape(196);
        let bg256 = ansi256_bg_escape(196);
        assert_ne!(fg256, bg256);

        let fg16 = ansi16_fg_escape(9);
        let bg16 = ansi16_bg_escape(9);
        assert_ne!(fg16, bg16);
    }

    // ========================================================================
    // AC6: Color Reset Tests
    // ========================================================================

    #[test]
    fn test_color_reset_format() {
        assert_eq!(color_reset(), "\x1b[0m");
    }

    #[test]
    fn test_color_reset_is_static() {
        // Verify it's a static string (no allocation)
        let reset1 = color_reset();
        let reset2 = color_reset();
        // Both should point to same memory (static)
        assert_eq!(reset1.as_ptr(), reset2.as_ptr());
    }

    // ========================================================================
    // AC7: Comprehensive Edge Case Tests
    // ========================================================================

    #[test]
    fn test_edge_case_rgb_000() {
        // All zeros
        let ansi256 = rgb_to_ansi256(0, 0, 0);
        let ansi16 = rgb_to_ansi16(0, 0, 0);
        assert_eq!(ansi256, 16); // Color cube black
        assert_eq!(ansi16, 0); // Black
    }

    #[test]
    fn test_edge_case_rgb_255() {
        // All 255s
        let ansi256 = rgb_to_ansi256(255, 255, 255);
        let ansi16 = rgb_to_ansi16(255, 255, 255);
        assert_eq!(ansi256, 231); // Color cube white
        assert_eq!(ansi16, 15); // Bright white
    }

    #[test]
    fn test_edge_case_rgb_128() {
        // Mid value
        let ansi256 = rgb_to_ansi256(128, 128, 128);
        let ansi16 = rgb_to_ansi16(128, 128, 128);
        assert_eq!(ansi256, 244); // Grayscale ramp
        assert_eq!(ansi16, 7); // Light gray (all channels > 127, max <= 191)
    }

    #[test]
    fn test_primary_colors_accuracy() {
        // Red
        assert_eq!(rgb_to_ansi256(255, 0, 0), 196);
        // Green
        assert_eq!(rgb_to_ansi256(0, 255, 0), 46);
        // Blue
        assert_eq!(rgb_to_ansi256(0, 0, 255), 21);
        // Cyan
        assert_eq!(rgb_to_ansi256(0, 255, 255), 51);
        // Magenta
        assert_eq!(rgb_to_ansi256(255, 0, 255), 201);
        // Yellow
        assert_eq!(rgb_to_ansi256(255, 255, 0), 226);
    }

    #[test]
    fn test_ansi16_all_base_colors() {
        // Test all 8 base color combinations at full brightness (max channel > 191)
        assert_eq!(rgb_to_ansi16(0, 0, 0), 0); // Black
        assert_eq!(rgb_to_ansi16(255, 0, 0), 9); // Bright Red
        assert_eq!(rgb_to_ansi16(0, 255, 0), 10); // Bright Green
        assert_eq!(rgb_to_ansi16(255, 255, 0), 11); // Bright Yellow
        assert_eq!(rgb_to_ansi16(0, 0, 255), 12); // Bright Blue
        assert_eq!(rgb_to_ansi16(255, 0, 255), 13); // Bright Magenta
        assert_eq!(rgb_to_ansi16(0, 255, 255), 14); // Bright Cyan
        assert_eq!(rgb_to_ansi16(255, 255, 255), 15); // Bright White

        // Test dark variants (max channel <= 191)
        assert_eq!(rgb_to_ansi16(191, 0, 0), 1); // Dark Red
        assert_eq!(rgb_to_ansi16(0, 191, 0), 2); // Dark Green
        assert_eq!(rgb_to_ansi16(0, 0, 191), 4); // Dark Blue
    }

    #[test]
    fn test_ansi16_escape_all_colors() {
        // Dark colors (0-7)
        for code in 0..8 {
            let escape = ansi16_fg_escape(code);
            assert!(escape.starts_with("\x1b[3"));
            assert!(escape.ends_with("m"));
        }

        // Bright colors (8-15)
        for code in 8..16 {
            let escape = ansi16_fg_escape(code);
            assert!(escape.starts_with("\x1b[9"));
            assert!(escape.ends_with("m"));
        }
    }

    #[test]
    fn test_grayscale_ramp_boundaries() {
        // Test grayscale ramp boundaries
        // Index 232 → gray value 8
        // Index 255 → gray value 238

        // Very dark gray should map to low index
        let dark = rgb_to_ansi256(8, 8, 8);
        assert!(dark >= 232, "Very dark gray should be in grayscale ramp");

        // Very light gray should map to high index
        let light = rgb_to_ansi256(238, 238, 238);
        assert!(light >= 232, "Very light gray should be in grayscale ramp");
    }

    #[test]
    fn test_color_cube_boundaries() {
        // Test color cube level boundaries
        // Level boundaries: 0, 95, 135, 175, 215, 255

        // Just below boundary should go to lower level
        let below_95 = rgb_to_ansi256(94, 0, 0);
        let at_95 = rgb_to_ansi256(95, 0, 0);
        // Both should be valid but might differ
        assert!(below_95 >= 16);
        assert!(at_95 >= 16);
    }

    // ========================================================================
    // Determinism Tests
    // ========================================================================

    #[test]
    fn test_conversion_deterministic() {
        // Same input should always produce same output
        for _ in 0..100 {
            assert_eq!(rgb_to_ansi256(128, 64, 192), rgb_to_ansi256(128, 64, 192));
            assert_eq!(rgb_to_ansi16(128, 64, 192), rgb_to_ansi16(128, 64, 192));
        }
    }
}
