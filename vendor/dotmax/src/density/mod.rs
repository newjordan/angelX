//! Character density-based rendering
//!
//! This module provides intensity-to-character mapping for creating ASCII-art style
//! visualizations. It enables rendering of smooth gradients and shading effects by
//! mapping floating-point intensity values [0.0, 1.0] to characters of varying
//! visual density.
//!
//! # Key Concepts
//!
//! **Intensity Mapping**: Convert grayscale intensity values (0.0 = black, 1.0 = white)
//! to characters that represent different visual densities:
//! - Low intensity (dark): sparse characters like space ' ', dot '.'
//! - High intensity (bright): dense characters like hash '#', at-sign '@'
//!
//! **Density Sets**: Collections of characters ordered from sparse to dense.
//! The module provides predefined sets optimized for different use cases:
//! - `ASCII_DENSITY`: Full 69-character gradient for maximum smoothness
//! - `SIMPLE_DENSITY`: Simple 10-character gradient for quick prototypes
//! - `BLOCKS_DENSITY`: Unicode block characters for modern terminals
//! - `BRAILLE_DENSITY`: Braille dot progression (unique to dotmax)
//!
//! # Examples
//!
//! ## Using Predefined Density Sets
//!
//! ```
//! use dotmax::density::DensitySet;
//!
//! // Create predefined density set
//! let density = DensitySet::ascii();
//!
//! // Map intensity values to characters
//! assert_eq!(density.map(0.0), ' ');  // Darkest (first character)
//! assert_eq!(density.map(1.0), '$');  // Brightest (last character)
//! // Middle intensity maps to a character ~34th in the 69-char sequence
//! let mid_char = density.map(0.5);
//! assert!(mid_char != ' ' && mid_char != '$');  // Somewhere in the middle
//! ```
//!
//! ## Creating Custom Density Sets
//!
//! ```
//! use dotmax::density::DensitySet;
//!
//! // Create custom density set
//! let custom = DensitySet::new(
//!     "Custom".to_string(),
//!     vec![' ', '.', 'o', 'O', '@']
//! ).unwrap();
//!
//! // Map intensities using custom set
//! assert_eq!(custom.map(0.0), ' ');   // Sparse
//! assert_eq!(custom.map(0.25), '.');  // Low density
//! assert_eq!(custom.map(0.5), 'o');   // Medium density
//! assert_eq!(custom.map(0.75), 'O');  // High density
//! assert_eq!(custom.map(1.0), '@');   // Dense
//! ```
//!
//! ## Rendering Intensity Buffers
//!
//! ```
//! use dotmax::{BrailleGrid, density::DensitySet};
//!
//! // Create grid and generate intensity buffer
//! let mut grid = BrailleGrid::new(10, 5).unwrap();
//! let intensities: Vec<f32> = (0..50)
//!     .map(|i| i as f32 / 49.0)  // Gradient from 0.0 to 1.0
//!     .collect();
//!
//! // Render using density set
//! let density = DensitySet::simple();
//! grid.render_density(&intensities, &density).unwrap();
//! ```
//!
//! # Performance
//!
//! - Intensity mapping: O(1) per cell (array index lookup)
//! - Grid rendering: O(n) where n = width × height cells
//! - Expected: ~1μs per cell = ~2ms for 80×24 terminal
//! - Target: <10ms for full terminal rendering (validated with benchmarks)
//!
//! # Predefined Density Sets
//!
//! | Constant | Characters | Use Case |
//! |----------|-----------|----------|
//! | `ASCII_DENSITY` | 69 chars | Maximum gradient smoothness |
//! | `SIMPLE_DENSITY` | 10 chars | Quick prototypes, minimal variation |
//! | `BLOCKS_DENSITY` | 5 chars | Block-based shading (Unicode) |
//! | `BRAILLE_DENSITY` | 9 chars | Braille-themed density progression |

use crate::{BrailleGrid, DotmaxError};

/// Predefined ASCII density character set (69 characters)
///
/// Characters ordered from sparse (space) to dense (dollar sign), providing
/// maximum gradient smoothness for detailed visualizations. Works on all
/// terminals with 7-bit ASCII support.
///
/// **Character progression:**
/// ```text
/// ` .'^\`",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$`
/// ```
///
/// **Source:** Classic ASCII art density ordering used in image-to-ASCII converters.
///
/// # Examples
///
/// ```
/// use dotmax::density::{DensitySet, ASCII_DENSITY};
///
/// let density = DensitySet::ascii();
/// assert_eq!(density.characters.len(), 69);
/// assert_eq!(density.characters[0], ' ');  // Sparsest
/// assert_eq!(density.characters[68], '$'); // Densest
/// ```
pub const ASCII_DENSITY: &str =
    " .'`^\",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

/// Predefined simple density character set (10 characters)
///
/// Characters ordered from sparse (space) to dense (at-sign), providing
/// coarse but clear gradient progression. Ideal for quick prototypes and
/// minimalist visualizations.
///
/// **Character progression:** ` .:-=+*#%@`
///
/// # Examples
///
/// ```
/// use dotmax::density::{DensitySet, SIMPLE_DENSITY};
///
/// let density = DensitySet::simple();
/// assert_eq!(density.characters.len(), 10);
/// assert_eq!(density.characters[0], ' ');  // Sparsest
/// assert_eq!(density.characters[9], '@'); // Densest
/// ```
pub const SIMPLE_DENSITY: &str = " .:-=+*#%@";

/// Predefined Unicode block density character set (5 characters)
///
/// Characters ordered from sparse (space) to dense (full block), using
/// Unicode block drawing characters for smooth gradients. Requires modern
/// terminal with Unicode support.
///
/// **Character progression:** ` ░▒▓█`
///
/// **Compatibility:** Requires Unicode support (most modern terminals).
/// For universal compatibility, use `ASCII_DENSITY` or `SIMPLE_DENSITY`.
///
/// # Examples
///
/// ```
/// use dotmax::density::{DensitySet, BLOCKS_DENSITY};
///
/// let density = DensitySet::blocks();
/// assert_eq!(density.characters.len(), 5);
/// assert_eq!(density.characters[0], ' ');   // Sparsest (space)
/// assert_eq!(density.characters[4], '█'); // Densest (full block)
/// ```
pub const BLOCKS_DENSITY: &str = " ░▒▓█";

/// Predefined braille density character set (9 characters)
///
/// Characters ordered from sparse (braille blank) to dense (braille full),
/// using braille Unicode characters to create density progression. Unique
/// to dotmax, combines density rendering with braille theme.
///
/// **Character progression:** `⠀⠁⠃⠇⠏⠟⠿⡿⣿`
///
/// **Compatibility:** Requires Unicode braille support (U+2800-U+28FF).
/// Most modern terminals support this range.
///
/// # Examples
///
/// ```
/// use dotmax::density::{DensitySet, BRAILLE_DENSITY};
///
/// let density = DensitySet::braille();
/// assert_eq!(density.characters.len(), 9);
/// assert_eq!(density.characters[0], '⠀'); // Sparsest (blank)
/// assert_eq!(density.characters[8], '⣿'); // Densest (full)
/// ```
pub const BRAILLE_DENSITY: &str = "⠀⠁⠃⠇⠏⠟⠿⡿⣿";

/// Character density set for intensity-based rendering
///
/// Maps intensity values [0.0, 1.0] to characters ordered from sparse (low intensity)
/// to dense (high intensity). Provides smooth gradient effects for ASCII-art style
/// visualizations.
///
/// # Algorithm
///
/// The mapping algorithm uses linear interpolation:
///
/// 1. Clamp intensity to [0.0, 1.0] range
/// 2. Calculate index: `round(intensity * (length - 1))`
/// 3. Return character at calculated index
///
/// This ensures:
/// - Intensity 0.0 always maps to first character (sparsest)
/// - Intensity 1.0 always maps to last character (densest)
/// - Intermediate values distribute linearly across character array
///
/// # Examples
///
/// ```
/// use dotmax::density::DensitySet;
///
/// // Create custom density set
/// let density = DensitySet::new(
///     "Custom".to_string(),
///     vec![' ', '.', ':', '#', '@']
/// ).unwrap();
///
/// // Map intensity values
/// assert_eq!(density.map(0.0), ' ');   // First character
/// assert_eq!(density.map(0.5), ':');   // Middle character
/// assert_eq!(density.map(1.0), '@');   // Last character
///
/// // Out-of-range intensities are clamped
/// assert_eq!(density.map(-0.5), ' ');  // Clamped to 0.0
/// assert_eq!(density.map(1.5), '@');   // Clamped to 1.0
/// ```
#[derive(Debug, Clone)]
pub struct DensitySet {
    /// Characters ordered from sparse (low intensity) to dense (high intensity)
    pub characters: Vec<char>,
    /// Descriptive name for this density set
    pub name: String,
}

impl DensitySet {
    /// Create custom density set with validation
    ///
    /// Creates a new density set from a character array. Characters must be
    /// ordered from sparse (low visual density) to dense (high visual density).
    ///
    /// # Arguments
    ///
    /// - `name`: Descriptive name for the density set
    /// - `characters`: Character array ordered sparse to dense
    ///
    /// # Validation Rules
    ///
    /// - Character list must not be empty (at least 1 character required)
    /// - Character list must not exceed 256 characters (performance limit)
    /// - No validation of character ordering (user responsibility)
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::EmptyDensitySet`] if `characters` is empty.
    ///
    /// Returns [`DotmaxError::TooManyCharacters`] if `characters.len() > 256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::density::DensitySet;
    ///
    /// // Valid: Create custom density set
    /// let density = DensitySet::new(
    ///     "Emoji".to_string(),
    ///     vec!['🌑', '🌒', '🌓', '🌔', '🌕']
    /// ).unwrap();
    /// assert_eq!(density.characters.len(), 5);
    ///
    /// // Error: Empty character list
    /// let result = DensitySet::new("Empty".to_string(), vec![]);
    /// assert!(result.is_err());
    ///
    /// // Error: Too many characters (>256)
    /// let too_many: Vec<char> = (0..300).map(|i| (i as u8) as char).collect();
    /// let result = DensitySet::new("TooMany".to_string(), too_many);
    /// assert!(result.is_err());
    /// ```
    pub fn new(name: String, characters: Vec<char>) -> Result<Self, DotmaxError> {
        // Validate: non-empty character list
        if characters.is_empty() {
            return Err(DotmaxError::EmptyDensitySet);
        }

        // Validate: maximum 256 characters (performance and memory limit)
        if characters.len() > 256 {
            return Err(DotmaxError::TooManyCharacters {
                count: characters.len(),
            });
        }

        Ok(Self { characters, name })
    }

    /// Map intensity value [0.0, 1.0] to character
    ///
    /// Maps a floating-point intensity value to a character from the density set
    /// using linear interpolation. Intensity values are clamped to [0.0, 1.0] range,
    /// so out-of-range values are handled gracefully.
    ///
    /// # Algorithm
    ///
    /// 1. Clamp intensity: `clamped = intensity.clamp(0.0, 1.0)`
    /// 2. Calculate index: `index = round(clamped * (len - 1))`
    /// 3. Return: `characters[index]`
    ///
    /// # Arguments
    ///
    /// - `intensity`: Intensity value, ideally in [0.0, 1.0] range
    ///   - 0.0 = darkest (first character)
    ///   - 1.0 = brightest (last character)
    ///   - Out-of-range values are clamped
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::density::DensitySet;
    ///
    /// let density = DensitySet::simple();
    ///
    /// // Boundary values
    /// assert_eq!(density.map(0.0), ' ');  // First character (sparsest)
    /// assert_eq!(density.map(1.0), '@');  // Last character (densest)
    ///
    /// // Middle value
    /// let mid_char = density.map(0.5);
    /// assert!(mid_char == '+' || mid_char == '*');  // Approximate middle
    ///
    /// // Out-of-range values are clamped
    /// assert_eq!(density.map(-0.5), ' '); // Clamped to 0.0 → first char
    /// assert_eq!(density.map(1.5), '@');  // Clamped to 1.0 → last char
    /// assert_eq!(density.map(f32::NAN), ' '); // NaN clamped to 0.0
    /// ```
    #[must_use]
    pub fn map(&self, intensity: f32) -> char {
        // Clamp intensity to [0.0, 1.0] range
        let clamped = intensity.clamp(0.0, 1.0);

        // Calculate index using linear interpolation
        // Allow cast_possible_truncation, cast_sign_loss, and cast_precision_loss:
        // - characters.len() <= 256, so precision loss is negligible
        // - Result will be positive and within usize range
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let index = (clamped * (self.characters.len() - 1) as f32).round() as usize;

        // Return character at calculated index
        self.characters[index]
    }

    /// Create predefined ASCII density set (69 characters)
    ///
    /// Returns a density set with the full ASCII gradient, providing maximum
    /// smoothness for detailed visualizations. Works on all terminals with
    /// 7-bit ASCII support.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::density::DensitySet;
    ///
    /// let density = DensitySet::ascii();
    /// assert_eq!(density.name, "ASCII");
    /// assert_eq!(density.characters.len(), 69);
    /// ```
    #[must_use]
    pub fn ascii() -> Self {
        Self {
            characters: ASCII_DENSITY.chars().collect(),
            name: "ASCII".to_string(),
        }
    }

    /// Create predefined simple density set (10 characters)
    ///
    /// Returns a density set with a simple 10-character gradient, ideal for
    /// quick prototypes and minimalist visualizations.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::density::DensitySet;
    ///
    /// let density = DensitySet::simple();
    /// assert_eq!(density.name, "Simple");
    /// assert_eq!(density.characters.len(), 10);
    /// ```
    #[must_use]
    pub fn simple() -> Self {
        Self {
            characters: SIMPLE_DENSITY.chars().collect(),
            name: "Simple".to_string(),
        }
    }

    /// Create predefined Unicode blocks density set (5 characters)
    ///
    /// Returns a density set using Unicode block drawing characters for smooth
    /// gradients. Requires modern terminal with Unicode support.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::density::DensitySet;
    ///
    /// let density = DensitySet::blocks();
    /// assert_eq!(density.name, "Blocks");
    /// assert_eq!(density.characters.len(), 5);
    /// ```
    #[must_use]
    pub fn blocks() -> Self {
        Self {
            characters: BLOCKS_DENSITY.chars().collect(),
            name: "Blocks".to_string(),
        }
    }

    /// Create predefined braille density set (9 characters)
    ///
    /// Returns a density set using braille Unicode characters for density
    /// progression. Unique to dotmax, combines density rendering with braille theme.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::density::DensitySet;
    ///
    /// let density = DensitySet::braille();
    /// assert_eq!(density.name, "Braille");
    /// assert_eq!(density.characters.len(), 9);
    /// ```
    #[must_use]
    pub fn braille() -> Self {
        Self {
            characters: BRAILLE_DENSITY.chars().collect(),
            name: "Braille".to_string(),
        }
    }
}

impl BrailleGrid {
    /// Render intensity buffer using character density mapping
    ///
    /// Renders a buffer of intensity values [0.0, 1.0] onto the grid using character
    /// density mapping. Each intensity value is mapped to a character via the provided
    /// density set, then rendered at the corresponding grid cell position.
    ///
    /// # Arguments
    ///
    /// - `intensity_buffer`: Row-major array of f32 intensity values [0.0, 1.0]
    ///   - Length must equal `grid.width() * grid.height()`
    ///   - Row-major order: `[row0_col0, row0_col1, ..., row1_col0, ...]`
    ///   - 0.0 = darkest (sparse character), 1.0 = brightest (dense character)
    /// - `density_set`: Character mapping for intensity → character conversion
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::BufferSizeMismatch`] if `intensity_buffer.len() !=
    /// grid.width() * grid.height()`.
    ///
    /// # Performance
    ///
    /// - Complexity: O(n) where n = width × height cells
    /// - Expected: ~1μs per cell = ~2ms for 80×24 terminal
    /// - Target: <10ms for full terminal rendering
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::{BrailleGrid, density::DensitySet};
    ///
    /// // Create grid and generate horizontal gradient
    /// let mut grid = BrailleGrid::new(10, 5).unwrap();
    /// let intensities: Vec<f32> = (0..50)
    ///     .map(|i| (i % 10) as f32 / 9.0)  // Gradient per row
    ///     .collect();
    ///
    /// // Render using ASCII density set
    /// let density = DensitySet::ascii();
    /// grid.render_density(&intensities, &density).unwrap();
    /// ```
    ///
    /// ## Error Handling
    ///
    /// ```
    /// use dotmax::{BrailleGrid, density::DensitySet};
    ///
    /// let mut grid = BrailleGrid::new(10, 5).unwrap();
    /// let wrong_size = vec![0.5_f32; 30]; // Wrong size (expected 50)
    ///
    /// let density = DensitySet::simple();
    /// let result = grid.render_density(&wrong_size, &density);
    /// assert!(result.is_err()); // BufferSizeMismatch error
    /// ```
    pub fn render_density(
        &mut self,
        intensity_buffer: &[f32],
        density_set: &DensitySet,
    ) -> Result<(), DotmaxError> {
        // Get grid dimensions
        let (width, height) = self.dimensions();
        let expected_size = width * height;

        // Validate buffer size matches grid dimensions
        if intensity_buffer.len() != expected_size {
            return Err(DotmaxError::BufferSizeMismatch {
                expected: expected_size,
                actual: intensity_buffer.len(),
            });
        }

        // Render each intensity value as a character
        // Story 4.4: We store characters in the BrailleGrid using set_char()
        // which overrides braille dot rendering when set
        for (i, &intensity) in intensity_buffer.iter().enumerate() {
            // Map intensity to character via density set
            let character = density_set.map(intensity);

            // Calculate cell position (row-major order)
            let cell_x = i % width;
            let cell_y = i / width;

            // Set character in grid
            // This will be rendered instead of braille dots when get_char() is called
            self.set_char(cell_x, cell_y, character)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // DensitySet::new() validation tests
    #[test]
    fn test_density_set_new_empty_returns_error() {
        let result = DensitySet::new("Empty".to_string(), vec![]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DotmaxError::EmptyDensitySet));
    }

    #[test]
    fn test_density_set_new_too_many_characters_returns_error() {
        let too_many: Vec<char> = (0..300).map(|_| 'x').collect();
        let result = DensitySet::new("TooMany".to_string(), too_many);
        assert!(result.is_err());
        match result.unwrap_err() {
            DotmaxError::TooManyCharacters { count } => assert_eq!(count, 300),
            _ => panic!("Expected TooManyCharacters error"),
        }
    }

    #[test]
    fn test_density_set_new_valid_range_succeeds() {
        // Test minimum (1 character)
        let result = DensitySet::new("Single".to_string(), vec!['x']);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().characters.len(), 1);

        // Test maximum (256 characters)
        let max_chars: Vec<char> = (0..256).map(|i| (i as u8) as char).collect();
        let result = DensitySet::new("Max".to_string(), max_chars);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().characters.len(), 256);
    }

    // DensitySet::map() tests
    #[test]
    fn test_density_set_map_boundary_values() {
        let density = DensitySet::new("Test".to_string(), vec![' ', '.', ':', '@']).unwrap();

        // 0.0 should map to first character
        assert_eq!(density.map(0.0), ' ');

        // 1.0 should map to last character
        assert_eq!(density.map(1.0), '@');
    }

    #[test]
    fn test_density_set_map_middle_value() {
        let density = DensitySet::new("Test".to_string(), vec![' ', '.', ':', '@']).unwrap();

        // 0.5 should map to middle character (approximately)
        let mid_char = density.map(0.5);
        assert!(mid_char == '.' || mid_char == ':');
    }

    #[test]
    fn test_density_set_map_clamping() {
        let density = DensitySet::new("Test".to_string(), vec![' ', '.', ':', '@']).unwrap();

        // Negative values should clamp to 0.0 → first character
        assert_eq!(density.map(-0.5), ' ');
        assert_eq!(density.map(-100.0), ' ');

        // Values > 1.0 should clamp to 1.0 → last character
        assert_eq!(density.map(1.5), '@');
        assert_eq!(density.map(100.0), '@');

        // NaN should clamp (to 0.0 with f32::clamp behavior)
        assert_eq!(density.map(f32::NAN), ' ');
    }

    // Predefined density sets tests
    #[test]
    fn test_predefined_ascii_density_set() {
        let density = DensitySet::ascii();
        assert_eq!(density.name, "ASCII");
        assert_eq!(density.characters.len(), 69); // Actual count from ASCII_DENSITY constant
        assert_eq!(density.characters[0], ' ');
        assert_eq!(density.characters[68], '$');
    }

    #[test]
    fn test_predefined_simple_density_set() {
        let density = DensitySet::simple();
        assert_eq!(density.name, "Simple");
        assert_eq!(density.characters.len(), 10);
        assert_eq!(density.characters[0], ' ');
        assert_eq!(density.characters[9], '@');
    }

    #[test]
    fn test_predefined_blocks_density_set() {
        let density = DensitySet::blocks();
        assert_eq!(density.name, "Blocks");
        assert_eq!(density.characters.len(), 5);
        assert_eq!(density.characters[0], ' ');
        assert_eq!(density.characters[4], '█');
    }

    #[test]
    fn test_predefined_braille_density_set() {
        let density = DensitySet::braille();
        assert_eq!(density.name, "Braille");
        assert_eq!(density.characters.len(), 9);
        assert_eq!(density.characters[0], '⠀');
        assert_eq!(density.characters[8], '⣿');
    }

    // render_density() tests
    #[test]
    fn test_render_density_buffer_size_mismatch() {
        let mut grid = BrailleGrid::new(10, 5).unwrap();
        let wrong_size = vec![0.5_f32; 30]; // Expected 50

        let density = DensitySet::simple();
        let result = grid.render_density(&wrong_size, &density);

        assert!(result.is_err());
        match result.unwrap_err() {
            DotmaxError::BufferSizeMismatch { expected, actual } => {
                assert_eq!(expected, 50);
                assert_eq!(actual, 30);
            }
            _ => panic!("Expected BufferSizeMismatch error"),
        }
    }

    #[test]
    fn test_render_density_valid_buffer_succeeds() {
        let mut grid = BrailleGrid::new(10, 5).unwrap();
        let valid_buffer = vec![0.5_f32; 50]; // Correct size

        let density = DensitySet::simple();
        let result = grid.render_density(&valid_buffer, &density);

        assert!(result.is_ok());
    }

    #[test]
    fn test_render_density_with_gradient() {
        let mut grid = BrailleGrid::new(10, 5).unwrap();
        let gradient: Vec<f32> = (0..50).map(|i| i as f32 / 49.0).collect();

        let density = DensitySet::ascii();
        let result = grid.render_density(&gradient, &density);

        assert!(result.is_ok());
    }

    #[test]
    fn test_render_density_actually_sets_characters() {
        let mut grid = BrailleGrid::new(3, 2).unwrap();
        // Simple gradient: [0.0, 0.5, 1.0, 0.0, 0.5, 1.0]
        let intensities = vec![0.0, 0.5, 1.0, 0.0, 0.5, 1.0];

        let density = DensitySet::simple(); // " .:-=+*#%@"
        let result = grid.render_density(&intensities, &density);

        assert!(result.is_ok());

        // Verify characters are set correctly
        // Intensity 0.0 → ' ' (first char)
        assert_eq!(grid.get_char(0, 0), ' ');
        // Intensity 0.5 → '+' or '*' (middle chars)
        let mid_char = grid.get_char(1, 0);
        assert!(mid_char == '+' || mid_char == '*');
        // Intensity 1.0 → '@' (last char)
        assert_eq!(grid.get_char(2, 0), '@');

        // Row 2 should have same pattern
        assert_eq!(grid.get_char(0, 1), ' ');
        assert_eq!(grid.get_char(2, 1), '@');
    }
}
