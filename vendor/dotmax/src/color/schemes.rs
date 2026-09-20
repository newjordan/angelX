//! Predefined color schemes for intensity-to-color mapping.
//!
//! This module provides a collection of beautiful, proven color schemes extracted from
//! [crabmusic](https://github.com/newjordan/crabmusic). These schemes map intensity values
//! (0.0 to 1.0) to colors, enabling vibrant visualizations without designing colors from scratch.
//!
//! # Overview
//!
//! A [`ColorScheme`] is a gradient defined by a sequence of color stops. The
//! [`ColorScheme::sample`] method maps an intensity value (0.0 = low, 1.0 = high)
//! to an interpolated color along the gradient.
//!
//! # Available Schemes
//!
//! | Scheme | Description |
//! |--------|-------------|
//! | [`rainbow`] | Red → Orange → Yellow → Green → Blue → Purple (HSV hue rotation) |
//! | [`heat_map`] | Black → Red → Orange → Yellow → White (thermal visualization) |
//! | [`blue_purple`] | Blue → Purple gradient |
//! | [`green_yellow`] | Green → Yellow gradient |
//! | [`cyan_magenta`] | Cyan → Magenta gradient |
//! | [`grayscale`] | Black → White gradient |
//! | [`monochrome`] | All-white (for uniform coloring) |
//!
//! # Examples
//!
//! ## Using a Predefined Scheme
//!
//! ```
//! use dotmax::color::schemes::{rainbow, heat_map};
//!
//! // Get colors at different intensities
//! let scheme = rainbow();
//! let low = scheme.sample(0.0);   // Red
//! let mid = scheme.sample(0.5);   // Green-ish
//! let high = scheme.sample(1.0);  // Purple
//! ```
//!
//! ## Creating a Custom Scheme
//!
//! ```
//! use dotmax::color::schemes::ColorScheme;
//! use dotmax::Color;
//!
//! let scheme = ColorScheme::new(
//!     "custom",
//!     vec![Color::rgb(0, 0, 0), Color::rgb(255, 0, 0), Color::rgb(255, 255, 255)]
//! ).unwrap();
//!
//! let color = scheme.sample(0.5);  // Interpolated red
//! ```
//!
//! ## Discovering Schemes at Runtime
//!
//! ```
//! use dotmax::color::schemes::{list_schemes, get_scheme};
//!
//! // List all available scheme names
//! let names = list_schemes();
//! assert!(names.contains(&"rainbow".to_string()));
//!
//! // Get a scheme by name (case-insensitive)
//! let scheme = get_scheme("RAINBOW").unwrap();
//! let color = scheme.sample(0.5);
//! ```
//!
//! # Performance
//!
//! The [`ColorScheme::sample`] method is optimized for high performance:
//! - No allocations in the hot path
//! - Linear interpolation with simple arithmetic
//! - Target: <100ns per sample call
//!
//! # Source Attribution
//!
//! Color algorithms extracted from [crabmusic](https://github.com/newjordan/crabmusic)
//! (`src/visualization/color_schemes.rs`).

use crate::error::DotmaxError;
use crate::grid::Color;

/// A color scheme for mapping intensity values to colors.
///
/// A `ColorScheme` defines a gradient using a sequence of color stops. The
/// [`sample`](ColorScheme::sample) method interpolates between these stops
/// based on an intensity value (0.0 to 1.0).
///
/// # Structure
///
/// - **name**: Human-readable name for the scheme
/// - **colors**: Ordered list of color stops (at least 1 required)
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::ColorScheme;
/// use dotmax::Color;
///
/// // Create a two-color gradient (red to blue)
/// let scheme = ColorScheme::new(
///     "red_blue",
///     vec![Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)]
/// ).unwrap();
///
/// // Sample at the midpoint
/// let mid_color = scheme.sample(0.5);
/// // Result is approximately purple (127, 0, 128)
/// ```
#[derive(Debug, Clone)]
pub struct ColorScheme {
    /// Human-readable name of the scheme
    name: String,
    /// Color stops for the gradient (intensity 0.0 → colors[0], 1.0 → colors[last])
    colors: Vec<Color>,
}

impl ColorScheme {
    /// Create a new color scheme with the given name and color stops.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the scheme
    /// * `colors` - Vector of color stops (at least 1 required)
    ///
    /// # Returns
    ///
    /// * `Ok(ColorScheme)` if colors is non-empty
    /// * `Err(DotmaxError::EmptyColorScheme)` if colors is empty
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    /// use dotmax::Color;
    ///
    /// // Valid scheme with multiple colors
    /// let scheme = ColorScheme::new(
    ///     "fire",
    ///     vec![Color::black(), Color::rgb(255, 0, 0), Color::rgb(255, 255, 0)]
    /// );
    /// assert!(scheme.is_ok());
    ///
    /// // Error: empty colors
    /// let err = ColorScheme::new("empty", vec![]);
    /// assert!(err.is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::EmptyColorScheme`] if `colors` is empty.
    pub fn new(name: impl Into<String>, colors: Vec<Color>) -> Result<Self, DotmaxError> {
        if colors.is_empty() {
            return Err(DotmaxError::EmptyColorScheme);
        }
        Ok(Self {
            name: name.into(),
            colors,
        })
    }

    /// Sample the color scheme at a given intensity value.
    ///
    /// Maps an intensity value (0.0 to 1.0) to an interpolated color along
    /// the gradient defined by the color stops.
    ///
    /// # Arguments
    ///
    /// * `intensity` - Value from 0.0 (low) to 1.0 (high)
    ///
    /// # Returns
    ///
    /// The interpolated color at the given intensity.
    ///
    /// # Boundary Behavior
    ///
    /// - `intensity = 0.0` → returns `colors[0]`
    /// - `intensity = 1.0` → returns `colors[last]`
    /// - `intensity = 0.5` → returns midpoint color (interpolated)
    /// - Values outside 0.0-1.0 are clamped to valid range
    ///
    /// # Interpolation
    ///
    /// Linear RGB interpolation is used between adjacent color stops.
    /// Each channel (R, G, B) is interpolated independently.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::grayscale;
    ///
    /// let scheme = grayscale();
    ///
    /// // Black at 0.0
    /// let black = scheme.sample(0.0);
    /// assert_eq!(black.r, 0);
    /// assert_eq!(black.g, 0);
    /// assert_eq!(black.b, 0);
    ///
    /// // White at 1.0
    /// let white = scheme.sample(1.0);
    /// assert_eq!(white.r, 255);
    /// assert_eq!(white.g, 255);
    /// assert_eq!(white.b, 255);
    ///
    /// // Gray at 0.5
    /// let gray = scheme.sample(0.5);
    /// assert!(gray.r > 120 && gray.r < 135);  // ~128
    /// ```
    #[inline]
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn sample(&self, intensity: f32) -> Color {
        // Clamp intensity to valid range
        let intensity = intensity.clamp(0.0, 1.0);

        let n = self.colors.len();

        // Single color: always return it
        if n == 1 {
            return self.colors[0];
        }

        // Calculate position in the gradient
        // SAFETY: n is small (typically 2-10), so precision loss is acceptable
        let scaled = intensity * (n - 1) as f32;
        // SAFETY: scaled is clamped to [0, n-1] range, so truncation and sign loss are safe
        let lower_idx = (scaled.floor() as usize).min(n - 1);
        let upper_idx = (lower_idx + 1).min(n - 1);
        let frac = scaled.fract();

        // Linear interpolation between adjacent colors
        let c1 = &self.colors[lower_idx];
        let c2 = &self.colors[upper_idx];

        Color::rgb(
            lerp_u8(c1.r, c2.r, frac),
            lerp_u8(c1.g, c2.g, frac),
            lerp_u8(c1.b, c2.b, frac),
        )
    }

    /// Get the name of this color scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::rainbow;
    ///
    /// let scheme = rainbow();
    /// assert_eq!(scheme.name(), "rainbow");
    /// ```
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the color stops of this color scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::grayscale;
    ///
    /// let scheme = grayscale();
    /// assert_eq!(scheme.colors().len(), 2);  // Black and white
    /// ```
    #[inline]
    #[must_use]
    pub fn colors(&self) -> &[Color] {
        &self.colors
    }

    /// Create a color scheme from a vector of colors with evenly-spaced intensities.
    ///
    /// This convenience constructor creates a gradient where colors are distributed
    /// evenly across the 0.0-1.0 intensity range. For example, 4 colors are placed
    /// at intensities 0.0, 0.33, 0.67, and 1.0.
    ///
    /// This is equivalent to using [`ColorSchemeBuilder`](crate::color::scheme_builder::ColorSchemeBuilder)
    /// with `add_color()` calls at calculated intensity positions.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the scheme
    /// * `colors` - Vector of colors (at least 2 required)
    ///
    /// # Returns
    ///
    /// * `Ok(ColorScheme)` if colors has at least 2 elements
    /// * `Err(DotmaxError::InvalidColorScheme)` if colors has fewer than 2 elements
    ///
    /// # Intensity Distribution
    ///
    /// For `n` colors, intensity positions are calculated as:
    /// - `colors[0]` → intensity 0.0
    /// - `colors[i]` → intensity `i / (n - 1)`
    /// - `colors[n-1]` → intensity 1.0
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    /// use dotmax::Color;
    ///
    /// // Create a 4-color gradient (evenly spaced at 0.0, 0.33, 0.67, 1.0)
    /// let scheme = ColorScheme::from_colors(
    ///     "brand",
    ///     vec![
    ///         Color::rgb(0, 51, 102),    // Dark blue
    ///         Color::rgb(0, 102, 153),   // Medium blue
    ///         Color::rgb(51, 153, 204),  // Light blue
    ///         Color::rgb(255, 255, 255), // White
    ///     ]
    /// )?;
    ///
    /// // Sample the gradient
    /// let color = scheme.sample(0.5);  // Approximately between 2nd and 3rd colors
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::InvalidColorScheme`] if `colors` contains fewer than 2 elements.
    pub fn from_colors(name: impl Into<String>, colors: Vec<Color>) -> Result<Self, DotmaxError> {
        if colors.len() < 2 {
            return Err(DotmaxError::InvalidColorScheme(
                "at least 2 colors required".into(),
            ));
        }
        // Colors are stored directly; sample() handles interpolation based on position
        Ok(Self {
            name: name.into(),
            colors,
        })
    }

    // ========================================================================
    // Predefined Schemes (Associated Functions)
    // ========================================================================

    /// Create a rainbow color scheme.
    ///
    /// Uses HSV color space for smooth hue transitions from red (0°) to purple (300°).
    ///
    /// # Gradient
    ///
    /// Red → Orange → Yellow → Green → Cyan → Blue → Purple
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    ///
    /// let scheme = ColorScheme::rainbow();
    /// let red = scheme.sample(0.0);    // ~(255, 0, 0)
    /// let purple = scheme.sample(1.0); // ~(255, 0, 255)
    /// ```
    #[must_use]
    pub fn rainbow() -> Self {
        rainbow()
    }

    /// Create a heat map color scheme.
    ///
    /// Classic thermal visualization: Black → Red → Orange → Yellow → White
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    ///
    /// let scheme = ColorScheme::heat_map();
    /// let cold = scheme.sample(0.0);  // Black
    /// let hot = scheme.sample(1.0);   // White
    /// ```
    #[must_use]
    pub fn heat_map() -> Self {
        heat_map()
    }

    /// Create a blue-purple gradient color scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    ///
    /// let scheme = ColorScheme::blue_purple();
    /// let blue = scheme.sample(0.0);   // Blue
    /// let purple = scheme.sample(1.0); // Purple
    /// ```
    #[must_use]
    pub fn blue_purple() -> Self {
        blue_purple()
    }

    /// Create a green-yellow gradient color scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    ///
    /// let scheme = ColorScheme::green_yellow();
    /// let green = scheme.sample(0.0);  // Green
    /// let yellow = scheme.sample(1.0); // Yellow
    /// ```
    #[must_use]
    pub fn green_yellow() -> Self {
        green_yellow()
    }

    /// Create a cyan-magenta gradient color scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    ///
    /// let scheme = ColorScheme::cyan_magenta();
    /// let cyan = scheme.sample(0.0);    // Cyan
    /// let magenta = scheme.sample(1.0); // Magenta
    /// ```
    #[must_use]
    pub fn cyan_magenta() -> Self {
        cyan_magenta()
    }

    /// Create a grayscale color scheme.
    ///
    /// Simple black to white gradient.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    ///
    /// let scheme = ColorScheme::grayscale();
    /// let black = scheme.sample(0.0); // Black
    /// let gray = scheme.sample(0.5);  // Gray
    /// let white = scheme.sample(1.0); // White
    /// ```
    #[must_use]
    pub fn grayscale() -> Self {
        grayscale()
    }

    /// Create a monochrome (all-white) color scheme.
    ///
    /// Returns white for all intensity values. Useful for uniform coloring
    /// when color variation is not desired but the scheme API is still needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::color::schemes::ColorScheme;
    ///
    /// let scheme = ColorScheme::monochrome();
    /// let c1 = scheme.sample(0.0);
    /// let c2 = scheme.sample(0.5);
    /// let c3 = scheme.sample(1.0);
    /// // All are white
    /// assert_eq!(c1, c2);
    /// assert_eq!(c2, c3);
    /// ```
    #[must_use]
    pub fn monochrome() -> Self {
        monochrome()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Linear interpolation for u8 values
#[inline]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a_f = f32::from(a);
    let b_f = f32::from(b);
    // SAFETY: Result is clamped to [0, 255] by lerp behavior, so truncation and sign loss are safe
    (b_f - a_f).mul_add(t, a_f).round() as u8
}

/// Convert HSV color to RGB.
///
/// Internal helper function used by rainbow scheme for smooth hue transitions.
///
/// # Arguments
///
/// * `h` - Hue in degrees (0-360)
/// * `s` - Saturation (0.0-1.0)
/// * `v` - Value/brightness (0.0-1.0)
///
/// # Algorithm
///
/// Standard HSV→RGB conversion algorithm from crabmusic.
#[inline]
#[allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn hsv_to_rgb(hue: f32, sat: f32, val: f32) -> Color {
    let chroma = val * sat;
    let h_prime = hue / 60.0;
    let secondary = chroma * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let match_val = val - chroma;

    let (red, green, blue) = if h_prime < 1.0 {
        (chroma, secondary, 0.0)
    } else if h_prime < 2.0 {
        (secondary, chroma, 0.0)
    } else if h_prime < 3.0 {
        (0.0, chroma, secondary)
    } else if h_prime < 4.0 {
        (0.0, secondary, chroma)
    } else if h_prime < 5.0 {
        (secondary, 0.0, chroma)
    } else {
        (chroma, 0.0, secondary)
    };

    // SAFETY: RGB values are clamped to [0, 255] by the HSV→RGB conversion
    Color::rgb(
        ((red + match_val) * 255.0) as u8,
        ((green + match_val) * 255.0) as u8,
        ((blue + match_val) * 255.0) as u8,
    )
}

// ============================================================================
// Predefined Color Scheme Functions
// ============================================================================

/// Create a rainbow color scheme.
///
/// Uses HSV color space for smooth hue transitions from red (0°) to purple (300°).
/// This produces a smooth rainbow gradient: Red → Orange → Yellow → Green → Cyan → Blue → Purple.
///
/// # Algorithm
///
/// Extracted from crabmusic `rainbow_gradient()`. Uses 7 color stops generated
/// from HSV hue values at 50° intervals.
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::rainbow;
///
/// let scheme = rainbow();
/// let red = scheme.sample(0.0);
/// let purple = scheme.sample(1.0);
///
/// // Red at start
/// assert!(red.r > 200);
/// // Purple at end
/// assert!(purple.r > 100 && purple.b > 100);
/// ```
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn rainbow() -> ColorScheme {
    // Generate 7 color stops using HSV (hue 0° to 300°)
    // SAFETY: i is in range [0, 6], so precision loss is negligible
    let colors: Vec<Color> = (0..=6)
        .map(|i| {
            let hue = (i as f32 / 6.0) * 300.0;
            hsv_to_rgb(hue, 1.0, 1.0)
        })
        .collect();

    // SAFETY: colors is guaranteed non-empty (7 elements)
    ColorScheme {
        name: "rainbow".to_string(),
        colors,
    }
}

/// Create a heat map color scheme.
///
/// Classic thermal visualization gradient: Black → Red → Orange → Yellow → White.
/// Perfect for visualizing intensity data like audio levels or temperature.
///
/// # Algorithm
///
/// Extracted from crabmusic `heat_map_gradient()`. Uses 5 color stops at
/// key transition points.
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::heat_map;
///
/// let scheme = heat_map();
/// let cold = scheme.sample(0.0);   // Black
/// let hot = scheme.sample(1.0);    // White
///
/// assert_eq!(cold.r, 0);
/// assert_eq!(cold.g, 0);
/// assert_eq!(cold.b, 0);
///
/// assert_eq!(hot.r, 255);
/// assert_eq!(hot.g, 255);
/// assert_eq!(hot.b, 255);
/// ```
#[must_use]
pub fn heat_map() -> ColorScheme {
    let colors = vec![
        Color::rgb(0, 0, 0),       // Black (0.0)
        Color::rgb(255, 0, 0),     // Red (0.25)
        Color::rgb(255, 165, 0),   // Orange (0.5)
        Color::rgb(255, 255, 0),   // Yellow (0.75)
        Color::rgb(255, 255, 255), // White (1.0)
    ];

    ColorScheme {
        name: "heat_map".to_string(),
        colors,
    }
}

/// Create a blue-purple gradient color scheme.
///
/// Smooth transition from blue to purple, matching crabmusic's `blue_purple_gradient()`.
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::blue_purple;
///
/// let scheme = blue_purple();
/// let blue = scheme.sample(0.0);
/// let purple = scheme.sample(1.0);
///
/// assert_eq!(blue.r, 0);
/// assert_eq!(blue.b, 255);
///
/// assert!(purple.r > 100);
/// assert!(purple.b > 100);
/// ```
#[must_use]
pub fn blue_purple() -> ColorScheme {
    let colors = vec![
        Color::rgb(0, 0, 255),   // Blue
        Color::rgb(128, 0, 127), // Purple (matching crabmusic)
    ];

    ColorScheme {
        name: "blue_purple".to_string(),
        colors,
    }
}

/// Create a green-yellow gradient color scheme.
///
/// Smooth transition from green to yellow, matching crabmusic's `green_yellow_gradient()`.
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::green_yellow;
///
/// let scheme = green_yellow();
/// let green = scheme.sample(0.0);
/// let yellow = scheme.sample(1.0);
///
/// assert_eq!(green.r, 0);
/// assert_eq!(green.g, 255);
///
/// assert_eq!(yellow.r, 255);
/// assert_eq!(yellow.g, 255);
/// ```
#[must_use]
pub fn green_yellow() -> ColorScheme {
    let colors = vec![
        Color::rgb(0, 255, 0),   // Green
        Color::rgb(255, 255, 0), // Yellow
    ];

    ColorScheme {
        name: "green_yellow".to_string(),
        colors,
    }
}

/// Create a cyan-magenta gradient color scheme.
///
/// Smooth transition from cyan to magenta, matching crabmusic's `cyan_magenta_gradient()`.
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::cyan_magenta;
///
/// let scheme = cyan_magenta();
/// let cyan = scheme.sample(0.0);
/// let magenta = scheme.sample(1.0);
///
/// assert_eq!(cyan.r, 0);
/// assert_eq!(cyan.g, 255);
/// assert_eq!(cyan.b, 255);
///
/// assert_eq!(magenta.r, 255);
/// assert_eq!(magenta.g, 0);
/// assert_eq!(magenta.b, 255);
/// ```
#[must_use]
pub fn cyan_magenta() -> ColorScheme {
    let colors = vec![
        Color::rgb(0, 255, 255), // Cyan
        Color::rgb(255, 0, 255), // Magenta
    ];

    ColorScheme {
        name: "cyan_magenta".to_string(),
        colors,
    }
}

/// Create a grayscale color scheme.
///
/// Simple black to white gradient.
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::grayscale;
///
/// let scheme = grayscale();
/// let black = scheme.sample(0.0);
/// let white = scheme.sample(1.0);
/// let gray = scheme.sample(0.5);
///
/// assert_eq!(black.r, 0);
/// assert_eq!(white.r, 255);
/// // Gray should be around 128
/// assert!(gray.r > 120 && gray.r < 135);
/// ```
#[must_use]
pub fn grayscale() -> ColorScheme {
    let colors = vec![
        Color::rgb(0, 0, 0),       // Black
        Color::rgb(255, 255, 255), // White
    ];

    ColorScheme {
        name: "grayscale".to_string(),
        colors,
    }
}

/// Create a monochrome (all-white) color scheme.
///
/// Returns white for all intensity values. This is useful when you need to use
/// the color scheme API but don't want color variation (e.g., for uniform white output).
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::monochrome;
///
/// let scheme = monochrome();
///
/// // All intensities return white
/// let c1 = scheme.sample(0.0);
/// let c2 = scheme.sample(0.5);
/// let c3 = scheme.sample(1.0);
///
/// assert_eq!(c1.r, 255);
/// assert_eq!(c1.g, 255);
/// assert_eq!(c1.b, 255);
/// assert_eq!(c1, c2);
/// assert_eq!(c2, c3);
/// ```
#[must_use]
pub fn monochrome() -> ColorScheme {
    ColorScheme {
        name: "monochrome".to_string(),
        colors: vec![Color::rgb(255, 255, 255)],
    }
}

// ============================================================================
// Discovery API
// ============================================================================

/// List all available predefined color scheme names.
///
/// Returns scheme names in a consistent order. Use [`get_scheme`] to retrieve
/// a scheme by name.
///
/// # Returns
///
/// A vector of scheme names as strings.
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::list_schemes;
///
/// let names = list_schemes();
/// assert_eq!(names.len(), 7);
/// assert!(names.contains(&"rainbow".to_string()));
/// assert!(names.contains(&"heat_map".to_string()));
/// assert!(names.contains(&"monochrome".to_string()));
/// ```
#[must_use]
pub fn list_schemes() -> Vec<String> {
    vec![
        "rainbow".to_string(),
        "heat_map".to_string(),
        "blue_purple".to_string(),
        "green_yellow".to_string(),
        "cyan_magenta".to_string(),
        "grayscale".to_string(),
        "monochrome".to_string(),
    ]
}

/// Get a predefined color scheme by name.
///
/// Name matching is case-insensitive.
///
/// # Arguments
///
/// * `name` - The name of the scheme to retrieve
///
/// # Returns
///
/// * `Some(ColorScheme)` if the name matches a predefined scheme
/// * `None` if no matching scheme is found
///
/// # Examples
///
/// ```
/// use dotmax::color::schemes::get_scheme;
///
/// // Case-insensitive matching
/// let scheme1 = get_scheme("rainbow");
/// let scheme2 = get_scheme("RAINBOW");
/// let scheme3 = get_scheme("Rainbow");
///
/// assert!(scheme1.is_some());
/// assert!(scheme2.is_some());
/// assert!(scheme3.is_some());
///
/// // Non-existent scheme returns None
/// let none = get_scheme("nonexistent");
/// assert!(none.is_none());
/// ```
#[must_use]
pub fn get_scheme(name: &str) -> Option<ColorScheme> {
    match name.to_lowercase().as_str() {
        "rainbow" => Some(rainbow()),
        "heat_map" | "heatmap" => Some(heat_map()),
        "blue_purple" | "bluepurple" => Some(blue_purple()),
        "green_yellow" | "greenyellow" => Some(green_yellow()),
        "cyan_magenta" | "cyanmagenta" => Some(cyan_magenta()),
        "grayscale" | "greyscale" => Some(grayscale()),
        "monochrome" => Some(monochrome()),
        _ => None,
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // AC1: ColorScheme Struct Tests
    // ========================================================================

    #[test]
    fn test_colorscheme_new_valid() {
        let scheme = ColorScheme::new("test", vec![Color::rgb(0, 0, 0), Color::rgb(255, 255, 255)]);
        assert!(scheme.is_ok());
        let scheme = scheme.unwrap();
        assert_eq!(scheme.name(), "test");
        assert_eq!(scheme.colors().len(), 2);
    }

    #[test]
    fn test_colorscheme_new_single_color() {
        let scheme = ColorScheme::new("single", vec![Color::rgb(128, 128, 128)]);
        assert!(scheme.is_ok());
        assert_eq!(scheme.unwrap().colors().len(), 1);
    }

    #[test]
    fn test_colorscheme_new_empty_returns_error() {
        let result = ColorScheme::new("empty", vec![]);
        assert!(matches!(result, Err(DotmaxError::EmptyColorScheme)));
    }

    #[test]
    fn test_colorscheme_clone_and_debug() {
        let scheme = rainbow();
        let cloned = scheme.clone();
        assert_eq!(scheme.name(), cloned.name());

        // Debug trait should be implemented
        let debug_str = format!("{:?}", scheme);
        assert!(debug_str.contains("ColorScheme"));
        assert!(debug_str.contains("rainbow"));
    }

    // ========================================================================
    // AC2: Intensity Sampling Tests
    // ========================================================================

    #[test]
    fn test_sample_boundary_0() {
        let scheme = grayscale();
        let color = scheme.sample(0.0);
        assert_eq!(color, Color::black());
    }

    #[test]
    fn test_sample_boundary_1() {
        let scheme = grayscale();
        let color = scheme.sample(1.0);
        assert_eq!(color, Color::white());
    }

    #[test]
    fn test_sample_midpoint() {
        let scheme = grayscale();
        let color = scheme.sample(0.5);
        // Should be approximately gray (128, 128, 128)
        assert!(color.r >= 127 && color.r <= 128);
        assert!(color.g >= 127 && color.g <= 128);
        assert!(color.b >= 127 && color.b <= 128);
    }

    #[test]
    fn test_sample_clamps_negative() {
        let scheme = grayscale();
        let color = scheme.sample(-0.5);
        // Should clamp to 0.0 → black
        assert_eq!(color, Color::black());
    }

    #[test]
    fn test_sample_clamps_above_one() {
        let scheme = grayscale();
        let color = scheme.sample(1.5);
        // Should clamp to 1.0 → white
        assert_eq!(color, Color::white());
    }

    #[test]
    fn test_sample_single_color_scheme() {
        let scheme = monochrome();
        // All intensities should return white
        assert_eq!(scheme.sample(0.0), Color::white());
        assert_eq!(scheme.sample(0.5), Color::white());
        assert_eq!(scheme.sample(1.0), Color::white());
    }

    // ========================================================================
    // AC3: Predefined Schemes Tests
    // ========================================================================

    #[test]
    fn test_rainbow_red_at_0() {
        let scheme = rainbow();
        let color = scheme.sample(0.0);
        // Red (255, 0, 0)
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn test_rainbow_purple_at_1() {
        let scheme = rainbow();
        let color = scheme.sample(1.0);
        // Should be purple-ish (high R and B, low G)
        assert!(color.r > 200);
        assert!(color.b > 200);
    }

    #[test]
    fn test_heat_map_black_at_0() {
        let scheme = heat_map();
        let color = scheme.sample(0.0);
        assert_eq!(color, Color::black());
    }

    #[test]
    fn test_heat_map_white_at_1() {
        let scheme = heat_map();
        let color = scheme.sample(1.0);
        assert_eq!(color, Color::white());
    }

    #[test]
    fn test_blue_purple_endpoints() {
        let scheme = blue_purple();
        let blue = scheme.sample(0.0);
        let purple = scheme.sample(1.0);

        assert_eq!(blue, Color::rgb(0, 0, 255));
        assert_eq!(purple, Color::rgb(128, 0, 127));
    }

    #[test]
    fn test_green_yellow_endpoints() {
        let scheme = green_yellow();
        let green = scheme.sample(0.0);
        let yellow = scheme.sample(1.0);

        assert_eq!(green, Color::rgb(0, 255, 0));
        assert_eq!(yellow, Color::rgb(255, 255, 0));
    }

    #[test]
    fn test_cyan_magenta_endpoints() {
        let scheme = cyan_magenta();
        let cyan = scheme.sample(0.0);
        let magenta = scheme.sample(1.0);

        assert_eq!(cyan, Color::rgb(0, 255, 255));
        assert_eq!(magenta, Color::rgb(255, 0, 255));
    }

    #[test]
    fn test_grayscale_endpoints() {
        let scheme = grayscale();
        assert_eq!(scheme.sample(0.0), Color::black());
        assert_eq!(scheme.sample(1.0), Color::white());
    }

    // ========================================================================
    // AC4: Scheme Discovery Tests
    // ========================================================================

    #[test]
    fn test_list_schemes_returns_7() {
        let schemes = list_schemes();
        assert_eq!(schemes.len(), 7);
    }

    #[test]
    fn test_list_schemes_contains_all() {
        let schemes = list_schemes();
        assert!(schemes.contains(&"rainbow".to_string()));
        assert!(schemes.contains(&"heat_map".to_string()));
        assert!(schemes.contains(&"blue_purple".to_string()));
        assert!(schemes.contains(&"green_yellow".to_string()));
        assert!(schemes.contains(&"cyan_magenta".to_string()));
        assert!(schemes.contains(&"grayscale".to_string()));
        assert!(schemes.contains(&"monochrome".to_string()));
    }

    #[test]
    fn test_get_scheme_valid() {
        assert!(get_scheme("rainbow").is_some());
        assert!(get_scheme("heat_map").is_some());
        assert!(get_scheme("blue_purple").is_some());
        assert!(get_scheme("green_yellow").is_some());
        assert!(get_scheme("cyan_magenta").is_some());
        assert!(get_scheme("grayscale").is_some());
        assert!(get_scheme("monochrome").is_some());
    }

    #[test]
    fn test_get_scheme_case_insensitive() {
        assert!(get_scheme("RAINBOW").is_some());
        assert!(get_scheme("Rainbow").is_some());
        assert!(get_scheme("rAiNbOw").is_some());
        assert!(get_scheme("HEAT_MAP").is_some());
        assert!(get_scheme("HeatMap").is_some());
    }

    #[test]
    fn test_get_scheme_alternate_names() {
        // Alternate naming conventions
        assert!(get_scheme("heatmap").is_some());
        assert!(get_scheme("bluepurple").is_some());
        assert!(get_scheme("greenyellow").is_some());
        assert!(get_scheme("cyanmagenta").is_some());
        assert!(get_scheme("greyscale").is_some()); // British spelling
    }

    #[test]
    fn test_get_scheme_invalid_returns_none() {
        assert!(get_scheme("nonexistent").is_none());
        assert!(get_scheme("fire").is_none());
        assert!(get_scheme("ocean").is_none());
        assert!(get_scheme("").is_none());
    }

    // ========================================================================
    // AC5: Monochrome Scheme Tests
    // ========================================================================

    #[test]
    fn test_monochrome_always_white() {
        let scheme = monochrome();
        for i in 0..=100 {
            let intensity = i as f32 / 100.0;
            let color = scheme.sample(intensity);
            assert_eq!(color, Color::white(), "Failed at intensity {}", intensity);
        }
    }

    #[test]
    fn test_monochrome_name() {
        let scheme = monochrome();
        assert_eq!(scheme.name(), "monochrome");
    }

    // ========================================================================
    // AC6: HSV to RGB Tests
    // ========================================================================

    #[test]
    fn test_hsv_to_rgb_red() {
        let color = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(color, Color::rgb(255, 0, 0));
    }

    #[test]
    fn test_hsv_to_rgb_green() {
        let color = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(color, Color::rgb(0, 255, 0));
    }

    #[test]
    fn test_hsv_to_rgb_blue() {
        let color = hsv_to_rgb(240.0, 1.0, 1.0);
        assert_eq!(color, Color::rgb(0, 0, 255));
    }

    #[test]
    fn test_hsv_to_rgb_yellow() {
        let color = hsv_to_rgb(60.0, 1.0, 1.0);
        assert_eq!(color, Color::rgb(255, 255, 0));
    }

    #[test]
    fn test_hsv_to_rgb_cyan() {
        let color = hsv_to_rgb(180.0, 1.0, 1.0);
        assert_eq!(color, Color::rgb(0, 255, 255));
    }

    #[test]
    fn test_hsv_to_rgb_magenta() {
        let color = hsv_to_rgb(300.0, 1.0, 1.0);
        assert_eq!(color, Color::rgb(255, 0, 255));
    }

    #[test]
    fn test_hsv_to_rgb_white() {
        // Saturation = 0 means white (regardless of hue)
        let color = hsv_to_rgb(0.0, 0.0, 1.0);
        assert_eq!(color, Color::white());
    }

    #[test]
    fn test_hsv_to_rgb_black() {
        // Value = 0 means black (regardless of hue/saturation)
        let color = hsv_to_rgb(0.0, 1.0, 0.0);
        assert_eq!(color, Color::black());
    }

    // ========================================================================
    // AC7: Comprehensive Coverage Tests
    // ========================================================================

    #[test]
    fn test_all_schemes_boundary_coverage() {
        let schemes = vec![
            rainbow(),
            heat_map(),
            blue_purple(),
            green_yellow(),
            cyan_magenta(),
            grayscale(),
            monochrome(),
        ];

        for scheme in schemes {
            // Test 0.0, 0.5, 1.0 for each scheme
            let c0 = scheme.sample(0.0);
            let c5 = scheme.sample(0.5);
            let c1 = scheme.sample(1.0);

            // All should return valid colors (no panics)
            // Just verify we can access the fields - u8 is always valid
            let _ = (c0.r, c0.g, c0.b);
            let _ = (c5.r, c5.g, c5.b);
            let _ = (c1.r, c1.g, c1.b);
        }
    }

    #[test]
    fn test_interpolation_smoothness() {
        let scheme = grayscale();
        let mut prev_r = 0u8;

        // Sample at 100 points and verify monotonic increase
        for i in 0..=100 {
            let intensity = i as f32 / 100.0;
            let color = scheme.sample(intensity);

            // Each step should increase or stay the same
            assert!(
                color.r >= prev_r,
                "Non-monotonic at intensity {}",
                intensity
            );
            prev_r = color.r;
        }
    }

    #[test]
    fn test_custom_scheme_creation() {
        let colors = vec![
            Color::rgb(255, 0, 0), // Red
            Color::rgb(0, 255, 0), // Green
            Color::rgb(0, 0, 255), // Blue
        ];

        let scheme = ColorScheme::new("rgb", colors).unwrap();

        // Test endpoints
        assert_eq!(scheme.sample(0.0), Color::rgb(255, 0, 0));
        assert_eq!(scheme.sample(1.0), Color::rgb(0, 0, 255));

        // Test midpoint (should be between red and green)
        let mid = scheme.sample(0.25);
        assert!(mid.r > 0);
        assert!(mid.g > 0);
    }

    #[test]
    fn test_lerp_u8_edge_cases() {
        // Test lerp helper
        assert_eq!(lerp_u8(0, 255, 0.0), 0);
        assert_eq!(lerp_u8(0, 255, 1.0), 255);
        assert_eq!(lerp_u8(0, 255, 0.5), 128);
        assert_eq!(lerp_u8(255, 0, 0.5), 128);
        assert_eq!(lerp_u8(100, 100, 0.5), 100); // Same values
    }

    // ========================================================================
    // Associated Method Tests (ColorScheme::*)
    // ========================================================================

    #[test]
    fn test_associated_methods() {
        // Verify associated methods return same results as free functions
        assert_eq!(ColorScheme::rainbow().name(), rainbow().name());
        assert_eq!(ColorScheme::heat_map().name(), heat_map().name());
        assert_eq!(ColorScheme::blue_purple().name(), blue_purple().name());
        assert_eq!(ColorScheme::green_yellow().name(), green_yellow().name());
        assert_eq!(ColorScheme::cyan_magenta().name(), cyan_magenta().name());
        assert_eq!(ColorScheme::grayscale().name(), grayscale().name());
        assert_eq!(ColorScheme::monochrome().name(), monochrome().name());
    }

    // ========================================================================
    // AC6: from_colors Convenience Constructor Tests (Story 5.4)
    // ========================================================================

    #[test]
    fn test_from_colors_creates_valid_scheme() {
        let colors = vec![Color::black(), Color::white()];
        let scheme = ColorScheme::from_colors("test", colors).unwrap();
        assert_eq!(scheme.name(), "test");
        assert_eq!(scheme.colors().len(), 2);
    }

    #[test]
    fn test_from_colors_with_four_colors() {
        let colors = vec![
            Color::rgb(0, 0, 0),
            Color::rgb(85, 85, 85),
            Color::rgb(170, 170, 170),
            Color::rgb(255, 255, 255),
        ];
        let scheme = ColorScheme::from_colors("four", colors).unwrap();
        assert_eq!(scheme.colors().len(), 4);
    }

    #[test]
    fn test_from_colors_validates_minimum_colors() {
        // Empty vector
        let result = ColorScheme::from_colors("empty", vec![]);
        assert!(matches!(result, Err(DotmaxError::InvalidColorScheme(_))));

        // Single color
        let result = ColorScheme::from_colors("single", vec![Color::white()]);
        assert!(matches!(result, Err(DotmaxError::InvalidColorScheme(_))));
    }

    #[test]
    fn test_from_colors_sample_boundaries() {
        let scheme =
            ColorScheme::from_colors("gradient", vec![Color::black(), Color::white()]).unwrap();

        let black = scheme.sample(0.0);
        let white = scheme.sample(1.0);

        assert_eq!(black, Color::black());
        assert_eq!(white, Color::white());
    }

    #[test]
    fn test_from_colors_sample_midpoint() {
        let scheme =
            ColorScheme::from_colors("gradient", vec![Color::black(), Color::white()]).unwrap();

        let mid = scheme.sample(0.5);
        // Should be approximately gray (128, 128, 128)
        assert!(mid.r >= 127 && mid.r <= 128);
    }

    #[test]
    fn test_from_colors_evenly_spaced_four_colors() {
        // 4 colors should be at 0.0, 0.33, 0.67, 1.0
        let scheme = ColorScheme::from_colors(
            "four",
            vec![
                Color::rgb(0, 0, 0),   // 0.0
                Color::rgb(85, 0, 0),  // 0.33
                Color::rgb(170, 0, 0), // 0.67
                Color::rgb(255, 0, 0), // 1.0
            ],
        )
        .unwrap();

        // Sample at exact color positions
        let c0 = scheme.sample(0.0);
        let c1 = scheme.sample(1.0);

        assert_eq!(c0.r, 0);
        assert_eq!(c1.r, 255);

        // Sample at 0.5 should interpolate between colors[1] and colors[2]
        // 0.5 is between 0.33 and 0.67, closer to 0.33
        let mid = scheme.sample(0.5);
        assert!(mid.r > 85 && mid.r < 170);
    }
}
