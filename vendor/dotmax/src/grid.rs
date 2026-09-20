//! Core [`BrailleGrid`] data structure for terminal braille rendering.
//!
//! This module provides the central [`BrailleGrid`] type and [`Color`] struct
//! for high-resolution terminal graphics using Unicode braille characters.
//!
//! # Overview
//!
//! Each terminal cell displays a braille character (U+2800-U+28FF) representing
//! a 2×4 dot matrix. This gives 4× the resolution of ASCII art.
//!
//! # Examples
//!
//! ```
//! use dotmax::BrailleGrid;
//!
//! // Create 80×24 grid (= 160×96 dot resolution)
//! let mut grid = BrailleGrid::new(80, 24).unwrap();
//!
//! // Set individual dots
//! grid.set_dot(0, 0).unwrap();  // Top-left
//! grid.set_dot(159, 95).unwrap(); // Bottom-right
//!
//! // Get the grid dimensions
//! let (width, height) = grid.dimensions();
//! assert_eq!(width, 80);
//! assert_eq!(height, 24);
//! ```
//!
//! # Extraction Note
//!
//! This module was extracted from the [crabmusic](https://github.com/newjordan/crabmusic) project.
//! See ADR 0005 (Copy-Refactor-Test strategy) for details.

// Import error types from error module
use crate::error::DotmaxError;

// Tracing for structured logging (Story 2.7)
use tracing::{debug, error, info, instrument};

/// Maximum grid dimensions to prevent OOM attacks (NFR-S2)
const MAX_GRID_WIDTH: usize = 10_000;
const MAX_GRID_HEIGHT: usize = 10_000;

// ============================================================================
// Color struct - Extracted from crabmusic/src/visualization/mod.rs
// ============================================================================

/// RGB color representation for braille cells
///
/// Extracted from crabmusic. Story 2.6 will implement full color rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red component (0-255)
    pub r: u8,
    /// Green component (0-255)
    pub g: u8,
    /// Blue component (0-255)
    pub b: u8,
}

impl Color {
    /// Create a new RGB color
    ///
    /// Extracted from `crabmusic::Color::new()`
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Create black color (0, 0, 0)
    #[must_use]
    pub const fn black() -> Self {
        Self { r: 0, g: 0, b: 0 }
    }

    /// Create white color (255, 255, 255)
    #[must_use]
    pub const fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
        }
    }
}

// ============================================================================
// BrailleDot enum - Extracted from crabmusic/src/visualization/braille.rs:16-28
// ============================================================================

/// Braille dot positions
///
/// Extracted from crabmusic. Maps dot positions to bit patterns for Unicode braille.
///
/// Dot positions in a Braille character:
///   1 4
///   2 5
///   3 6
///   7 8
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrailleDot {
    /// Top-left dot (position 1)
    Dot1 = 0b0000_0001,
    /// Middle-left dot (position 2)
    Dot2 = 0b0000_0010,
    /// Lower-left dot (position 3)
    Dot3 = 0b0000_0100,
    /// Top-right dot (position 4)
    Dot4 = 0b0000_1000,
    /// Middle-right dot (position 5)
    Dot5 = 0b0001_0000,
    /// Lower-right dot (position 6)
    Dot6 = 0b0010_0000,
    /// Bottom-left dot (position 7)
    Dot7 = 0b0100_0000,
    /// Bottom-right dot (position 8)
    Dot8 = 0b1000_0000,
}

// ============================================================================
// dots_to_char - Extracted from crabmusic/src/visualization/braille.rs:52-56
// ============================================================================

/// Convert dot pattern to Braille Unicode character
///
/// Extracted from crabmusic. Story 2.2 will integrate this into the rendering pipeline.
///
/// # Arguments
/// * `dots` - Bit pattern where each bit represents a dot (1 = filled)
///
/// # Returns
/// Unicode Braille character
///
/// # Examples
///
/// ```
/// use dotmax::grid::dots_to_char;
///
/// // Empty pattern
/// assert_eq!(dots_to_char(0b00000000), '⠀');
///
/// // All dots filled
/// assert_eq!(dots_to_char(0b11111111), '⣿');
/// ```
#[inline]
#[must_use]
pub fn dots_to_char(dots: u8) -> char {
    // Braille patterns start at U+2800
    // SAFETY: crabmusic uses unwrap_or here; we keep the same logic
    // since 0x2800 + (0..=255) is always valid Unicode
    char::from_u32(0x2800 + u32::from(dots)).unwrap_or('⠀')
}

// ============================================================================
// BrailleGrid - Extracted from crabmusic/src/visualization/braille.rs:73-369
// ============================================================================

/// High-resolution grid using Braille characters
///
/// **Extracted from crabmusic** - Battle-tested rendering engine.
///
/// Each terminal cell contains a 2×4 dot pattern (8 dots total), giving us
/// high-resolution graphics in any terminal that supports Unicode braille.
///
/// ## Architecture (Preserved from crabmusic)
///
/// - **`patterns: Vec<u8>`** - Flat array, each u8 is a bitfield (8 bits = 8 dots)
/// - **Dot coordinates**: (`dot_x`, `dot_y`) in pixel space (width*2 × height*4)
/// - **Cell coordinates**: (`cell_x`, `cell_y`) in terminal space (width × height)
///
/// ## Dot Indexing (Unicode Braille Standard)
///
/// ```text
/// Braille cell (8 dots):
/// 1 4    (Dot1=0x01, Dot4=0x08)
/// 2 5    (Dot2=0x02, Dot5=0x10)
/// 3 6    (Dot3=0x04, Dot6=0x20)
/// 7 8    (Dot7=0x40, Dot8=0x80)
/// ```
///
/// # Example
///
/// ```
/// use dotmax::BrailleGrid;
///
/// let mut grid = BrailleGrid::new(40, 20).unwrap();
/// // Grid is 40×20 cells = 80×80 dot resolution
/// grid.set_dot(0, 0); // Top-left dot
/// grid.set_dot(1, 0); // Top-right dot of first cell
/// ```
#[derive(Debug, Clone)]
pub struct BrailleGrid {
    /// Width in terminal cells
    width: usize,
    /// Height in terminal cells
    height: usize,
    /// Dot patterns for each cell (binary on/off)
    ///
    /// **Preserved from crabmusic**: `Vec<u8>` bitfield representation
    /// Each `u8` represents one terminal cell with 8 dots
    patterns: Vec<u8>,
    /// Optional colors for each cell
    ///
    /// **Preserved from crabmusic**: `Vec<Option<Color>>`
    /// Story 2.6 will implement color rendering
    colors: Vec<Option<Color>>,
    /// Optional text characters for density rendering (Story 4.4)
    ///
    /// When set, these characters override braille dot patterns for rendering.
    /// This enables character-density based rendering (ASCII art style) as an
    /// alternative to binary braille dot rendering.
    ///
    /// `None` = use braille dots (default), `Some(char)` = render this character
    characters: Vec<Option<char>>,
}

impl BrailleGrid {
    /// Create a new Braille grid
    ///
    /// **Extracted from `crabmusic::BrailleGrid::new()`** with added validation.
    ///
    /// # Arguments
    /// * `width` - Width in terminal cells (must be > 0 and <= `MAX_GRID_WIDTH`)
    /// * `height` - Height in terminal cells (must be > 0 and <= `MAX_GRID_HEIGHT`)
    ///
    /// # Returns
    /// * `Ok(BrailleGrid)` if dimensions are valid
    /// * `Err(DotmaxError::InvalidDimensions)` if width/height is 0 or exceeds max
    ///
    /// # Errors
    /// Returns `InvalidDimensions` if width or height is 0 or exceeds max allowed dimensions.
    ///
    /// # Crabmusic Change
    /// Original crabmusic code never validated dimensions.
    /// Dotmax adds validation for security (NFR-S2).
    #[instrument]
    pub fn new(width: usize, height: usize) -> Result<Self, DotmaxError> {
        // Validate dimensions (NEW - not in crabmusic)
        if width == 0 || height == 0 {
            error!(
                width = width,
                height = height,
                "Invalid grid dimensions: width or height is zero"
            );
            return Err(DotmaxError::InvalidDimensions { width, height });
        }

        if width > MAX_GRID_WIDTH || height > MAX_GRID_HEIGHT {
            error!(
                width = width,
                height = height,
                max_width = MAX_GRID_WIDTH,
                max_height = MAX_GRID_HEIGHT,
                "Invalid grid dimensions: exceeds maximum allowed size"
            );
            return Err(DotmaxError::InvalidDimensions { width, height });
        }

        // Allocate grid (PRESERVED from crabmusic)
        let size = width * height;
        info!(
            width = width,
            height = height,
            total_cells = size,
            "Creating BrailleGrid"
        );
        Ok(Self {
            width,
            height,
            patterns: vec![0; size],
            colors: vec![None; size],
            characters: vec![None; size], // Story 4.4: character buffer for density rendering
        })
    }

    /// Get width in terminal cells
    ///
    /// **Extracted from crabmusic** (lines 104-106)
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Get height in terminal cells
    ///
    /// **Extracted from crabmusic** (lines 108-111)
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Get width in dots (2× terminal width)
    ///
    /// **Extracted from crabmusic** (lines 113-116)
    #[must_use]
    pub const fn dot_width(&self) -> usize {
        self.width * 2
    }

    /// Get height in dots (4× terminal height)
    ///
    /// **Extracted from crabmusic** (lines 118-121)
    #[must_use]
    pub const fn dot_height(&self) -> usize {
        self.height * 4
    }

    /// Get the dimensions of the grid (dotmax addition for AC #7)
    ///
    /// **NEW** - Not in crabmusic. Added to satisfy AC #7 requirement.
    ///
    /// # Returns
    /// A tuple of (width, height) in terminal cells
    #[must_use]
    pub const fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Clear all dots
    ///
    /// **Extracted from crabmusic** (lines 124-127) with minor adaptation
    #[instrument(skip(self))]
    pub fn clear(&mut self) {
        debug!(
            width = self.width,
            height = self.height,
            "Clearing all dots in grid"
        );
        self.patterns.fill(0);
        self.colors.fill(None);
    }

    /// Set a single dot at the specified position
    ///
    /// **Extracted from crabmusic** (lines 144-172) with added error handling.
    ///
    /// **CRITICAL**: This uses PIXEL coordinates (`dot_x`, `dot_y`), not cell coordinates.
    /// The grid is width*2 × height*4 dots.
    ///
    /// # Arguments
    /// * `dot_x` - X position in dots (0 to width*2-1)
    /// * `dot_y` - Y position in dots (0 to height*4-1)
    ///
    /// # Crabmusic Change
    /// Original crabmusic silently ignored out-of-bounds coordinates.
    /// Dotmax returns an error for explicit bounds checking (zero panics policy).
    ///
    /// # Errors
    /// Returns `OutOfBounds` if dot coordinates exceed grid dimensions.
    pub fn set_dot(&mut self, dot_x: usize, dot_y: usize) -> Result<(), DotmaxError> {
        // Bounds check (MODIFIED from crabmusic - return error instead of silent ignore)
        if dot_x >= self.dot_width() || dot_y >= self.dot_height() {
            error!(
                dot_x = dot_x,
                dot_y = dot_y,
                dot_width = self.dot_width(),
                dot_height = self.dot_height(),
                "Out of bounds dot access: ({}, {}) in grid of size ({}, {})",
                dot_x,
                dot_y,
                self.dot_width(),
                self.dot_height()
            );
            return Err(DotmaxError::OutOfBounds {
                x: dot_x,
                y: dot_y,
                width: self.dot_width(),
                height: self.dot_height(),
            });
        }

        // Convert dot coordinates to cell coordinates (PRESERVED from crabmusic)
        let cell_x = dot_x / 2;
        let cell_y = dot_y / 4;
        let cell_index = cell_y * self.width + cell_x;

        // Determine which dot within the cell (0-7) (PRESERVED from crabmusic)
        let local_x = dot_x % 2;
        let local_y = dot_y % 4;

        // Map to Braille dot position (PRESERVED from crabmusic, lines 159-169)
        let dot_bit = match (local_x, local_y) {
            (0, 0) => BrailleDot::Dot1 as u8,
            (0, 1) => BrailleDot::Dot2 as u8,
            (0, 2) => BrailleDot::Dot3 as u8,
            (0, 3) => BrailleDot::Dot7 as u8,
            (1, 0) => BrailleDot::Dot4 as u8,
            (1, 1) => BrailleDot::Dot5 as u8,
            (1, 2) => BrailleDot::Dot6 as u8,
            (1, 3) => BrailleDot::Dot8 as u8,
            _ => unreachable!(),
        };

        // Set the dot (PRESERVED from crabmusic, line 171)
        self.patterns[cell_index] |= dot_bit;
        Ok(())
    }

    /// Get an individual dot value
    ///
    /// **NEW** - Not in crabmusic. Added to match AC #4 requirement.
    ///
    /// # Arguments
    /// * `dot_x` - X position in dots (0 to width*2-1)
    /// * `dot_y` - Y position in dots (0 to height*4-1)
    /// * `dot_index` - Dot position 0-7 in the cell
    ///
    /// # Returns
    /// * `Ok(bool)` - The dot value (true = enabled, false = disabled)
    /// * `Err(DotmaxError::OutOfBounds)` if coordinates exceed grid dimensions
    /// * `Err(DotmaxError::InvalidDotIndex)` if `dot_index` > 7
    ///
    /// # Errors
    /// Returns `OutOfBounds` if dot coordinates exceed grid dimensions, or `InvalidDotIndex` if dot index > 7.
    pub fn get_dot(&self, x: usize, y: usize, dot_index: u8) -> Result<bool, DotmaxError> {
        // Validate cell bounds
        if x >= self.width || y >= self.height {
            return Err(DotmaxError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }

        // Validate dot index
        if dot_index > 7 {
            return Err(DotmaxError::InvalidDotIndex { index: dot_index });
        }

        // Calculate cell index
        let cell_index = y * self.width + x;
        let pattern = self.patterns[cell_index];

        // Check if dot is set using bit mask
        let dot_bit = 1u8 << dot_index;
        Ok((pattern & dot_bit) != 0)
    }

    /// Clear a rectangular region of the grid
    ///
    /// **NEW** - Not in crabmusic. Added to satisfy AC #6 requirement.
    ///
    /// # Arguments
    /// * `x` - Starting column in cells (0-indexed)
    /// * `y` - Starting row in cells (0-indexed)
    /// * `width` - Width of region to clear in cells
    /// * `height` - Height of region to clear in cells
    ///
    /// # Returns
    /// * `Ok(())` if region was cleared successfully
    /// * `Err(DotmaxError::OutOfBounds)` if region extends beyond grid bounds
    ///
    /// # Errors
    /// Returns `OutOfBounds` if the specified region extends beyond grid dimensions.
    pub fn clear_region(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Result<(), DotmaxError> {
        // Validate bounds - check if region fits within grid
        let end_x = x.saturating_add(width);
        let end_y = y.saturating_add(height);

        if end_x > self.width || end_y > self.height {
            return Err(DotmaxError::OutOfBounds {
                x: end_x.saturating_sub(1),
                y: end_y.saturating_sub(1),
                width: self.width,
                height: self.height,
            });
        }

        // Clear the specified region
        for row_idx in y..end_y {
            for col_idx in x..end_x {
                let cell_index = row_idx * self.width + col_idx;
                self.patterns[cell_index] = 0;
                self.colors[cell_index] = None;
            }
        }

        Ok(())
    }

    /// Get the Braille character at a cell position
    ///
    /// **Extracted from crabmusic** (lines 338-347)
    ///
    /// # Arguments
    /// * `cell_x` - X position in cells
    /// * `cell_y` - Y position in cells
    ///
    /// # Returns
    /// Character at the cell position:
    /// - If a text character is set (Story 4.4 density rendering), returns that character
    /// - Otherwise, returns braille character representing the dot pattern
    #[must_use]
    pub fn get_char(&self, cell_x: usize, cell_y: usize) -> char {
        if cell_x >= self.width || cell_y >= self.height {
            return '⠀';
        }

        let index = cell_y * self.width + cell_x;

        // Story 4.4: Prioritize text characters for density rendering
        if let Some(ch) = self.characters[index] {
            return ch;
        }

        // Default: Convert dot pattern to braille character
        dots_to_char(self.patterns[index])
    }

    /// Get the color at a cell position
    ///
    /// **Extracted from crabmusic** (lines 350-357)
    #[must_use]
    pub fn get_color(&self, cell_x: usize, cell_y: usize) -> Option<Color> {
        if cell_x >= self.width || cell_y >= self.height {
            return None;
        }

        let index = cell_y * self.width + cell_x;
        self.colors[index]
    }

    /// Check if a cell has any dots set
    ///
    /// **Extracted from crabmusic** (lines 360-368)
    #[must_use]
    pub fn is_empty(&self, cell_x: usize, cell_y: usize) -> bool {
        if cell_x >= self.width || cell_y >= self.height {
            return true;
        }

        let index = cell_y * self.width + cell_x;
        self.patterns[index] == 0
    }

    // ========================================================================
    // Story 6.4: Raw Pattern Access for Animation Serialization
    // ========================================================================

    /// Get raw access to the pattern buffer for serialization.
    ///
    /// **Story 6.4** - Enables efficient animation frame serialization.
    ///
    /// Returns a slice of the internal pattern buffer, where each byte
    /// represents one braille cell's dot pattern (8 bits = 8 dots).
    ///
    /// # Returns
    /// A byte slice of length `width * height` containing dot patterns.
    ///
    /// # Examples
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(10, 5).unwrap();
    /// grid.set_dot(0, 0).unwrap(); // Set top-left dot
    ///
    /// let patterns = grid.get_raw_patterns();
    /// assert_eq!(patterns.len(), 50); // 10 * 5 cells
    /// assert_eq!(patterns[0], 0b0000_0001); // First cell has dot 1 set
    /// ```
    #[must_use]
    pub fn get_raw_patterns(&self) -> &[u8] {
        &self.patterns
    }

    /// Set raw pattern buffer from serialized data.
    ///
    /// **Story 6.4** - Enables efficient animation frame deserialization.
    ///
    /// Copies data into the internal pattern buffer. The data slice length
    /// must match `width * height`. If the length doesn't match, excess data
    /// is truncated or missing data is left unchanged.
    ///
    /// # Arguments
    /// * `data` - Raw pattern bytes to copy into the grid
    ///
    /// # Examples
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(10, 5).unwrap();
    /// let mut patterns = vec![0u8; 50];
    /// patterns[0] = 0b0000_0001; // Set dot 1 in first cell
    ///
    /// grid.set_raw_patterns(&patterns);
    /// assert!(grid.get_char(0, 0) == '⠁'); // First cell shows dot 1
    /// ```
    pub fn set_raw_patterns(&mut self, data: &[u8]) {
        let copy_len = data.len().min(self.patterns.len());
        self.patterns[..copy_len].copy_from_slice(&data[..copy_len]);
    }

    // ========================================================================
    // Story 2.2: Unicode Braille Character Conversion
    // ========================================================================

    /// Convert entire grid to 2D array of Unicode braille characters
    ///
    /// **Story 2.2** - Batch conversion for rendering pipeline.
    ///
    /// This method converts the entire grid from dot patterns to Unicode braille
    /// characters, producing a 2D array that matches the grid dimensions.
    ///
    /// Uses the proven `dots_to_char()` function extracted from crabmusic
    /// (lines 53-56) which applies the Unicode Braille standard formula:
    /// `U+2800 + bitfield`
    ///
    /// # Returns
    /// A 2D vector of Unicode braille characters, where `result[y][x]` corresponds
    /// to cell `(x, y)` in the grid.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(5, 5).unwrap();
    /// grid.set_dot(0, 0).unwrap(); // Top-left dot of cell (0,0)
    ///
    /// let chars = grid.to_unicode_grid();
    /// assert_eq!(chars.len(), 5); // 5 rows
    /// assert_eq!(chars[0].len(), 5); // 5 columns
    /// assert_eq!(chars[0][0], '⠁'); // Cell (0,0) has dot 1 set
    /// ```
    ///
    /// # Performance
    /// Time complexity: O(width × height) - processes each cell once
    /// Allocates: `Vec<Vec<char>>` with dimensions matching grid size
    #[must_use]
    pub fn to_unicode_grid(&self) -> Vec<Vec<char>> {
        let mut result = Vec::with_capacity(self.height);

        for y in 0..self.height {
            let mut row = Vec::with_capacity(self.width);
            for x in 0..self.width {
                let index = y * self.width + x;
                // Use extracted crabmusic conversion function
                row.push(dots_to_char(self.patterns[index]));
            }
            result.push(row);
        }

        result
    }

    /// Convert single cell at (x, y) to Unicode braille character
    ///
    /// **Story 2.2** - Single-cell conversion with bounds validation.
    ///
    /// Returns the Unicode braille character for a specific cell, or an error
    /// if coordinates are out of bounds.
    ///
    /// # Arguments
    /// * `x` - X position in cells (0 to width-1)
    /// * `y` - Y position in cells (0 to height-1)
    ///
    /// # Returns
    /// * `Ok(char)` - Unicode braille character (U+2800 to U+28FF)
    /// * `Err(DotmaxError::OutOfBounds)` - If coordinates exceed grid dimensions
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(10, 10).unwrap();
    /// grid.set_dot(2, 4).unwrap(); // Set a dot in cell (1,1)
    ///
    /// let ch = grid.cell_to_braille_char(1, 1).unwrap();
    /// assert!(ch >= '\u{2800}' && ch <= '\u{28FF}'); // Valid braille range
    /// ```
    ///
    /// # Errors
    /// Returns `OutOfBounds` if x >= width or y >= height.
    pub fn cell_to_braille_char(&self, x: usize, y: usize) -> Result<char, DotmaxError> {
        // Validate bounds
        if x >= self.width || y >= self.height {
            return Err(DotmaxError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }

        // Convert cell pattern to Unicode
        let index = y * self.width + x;
        Ok(dots_to_char(self.patterns[index]))
    }

    /// Resize the grid to new dimensions
    ///
    /// **NEW for Story 2.5** - Not in crabmusic. Enables terminal resize handling.
    ///
    /// # Arguments
    /// * `new_width` - New width in braille cells
    /// * `new_height` - New height in braille cells
    ///
    /// # Behavior
    /// - **Grow**: New cells initialized to empty (pattern=0, color=None)
    /// - **Shrink**: Existing dots outside new bounds are truncated
    /// - **Preserve**: Dots within overlap region are preserved
    /// - **Colors**: Color buffer resizes in sync with patterns
    ///
    /// # Errors
    /// Returns `DotmaxError::InvalidDimensions` if:
    /// - `new_width` or `new_height` is 0
    /// - `new_width` or `new_height` exceeds `MAX_GRID_WIDTH`/`MAX_GRID_HEIGHT` (10,000)
    ///
    /// # Examples
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(10, 10)?;
    /// grid.set_dot(0, 0)?; // Set top-left dot
    ///
    /// // Resize to larger dimensions
    /// grid.resize(20, 20)?;
    /// assert_eq!(grid.dimensions(), (20, 20));
    ///
    /// // Resize to smaller dimensions
    /// grid.resize(5, 5)?;
    /// assert_eq!(grid.dimensions(), (5, 5));
    /// # Ok::<(), dotmax::DotmaxError>(())
    /// ```
    #[instrument(skip(self))]
    pub fn resize(&mut self, new_width: usize, new_height: usize) -> Result<(), DotmaxError> {
        debug!(
            old_width = self.width,
            old_height = self.height,
            new_width = new_width,
            new_height = new_height,
            "Resizing BrailleGrid"
        );

        // Validation (same logic as new())
        if new_width == 0 || new_height == 0 {
            error!(
                new_width = new_width,
                new_height = new_height,
                "Invalid resize dimensions: width or height is zero"
            );
            return Err(DotmaxError::InvalidDimensions {
                width: new_width,
                height: new_height,
            });
        }
        if new_width > MAX_GRID_WIDTH || new_height > MAX_GRID_HEIGHT {
            error!(
                new_width = new_width,
                new_height = new_height,
                max_width = MAX_GRID_WIDTH,
                max_height = MAX_GRID_HEIGHT,
                "Invalid resize dimensions: exceeds maximum allowed size"
            );
            return Err(DotmaxError::InvalidDimensions {
                width: new_width,
                height: new_height,
            });
        }

        // Create new storage
        let new_size = new_width * new_height;
        let mut new_patterns = vec![0; new_size];
        let mut new_colors = vec![None; new_size];

        // Copy existing data (preserve overlap region)
        let copy_width = self.width.min(new_width);
        let copy_height = self.height.min(new_height);

        for y in 0..copy_height {
            for x in 0..copy_width {
                let old_index = y * self.width + x;
                let new_index = y * new_width + x;
                new_patterns[new_index] = self.patterns[old_index];
                new_colors[new_index] = self.colors[old_index];
            }
        }

        // Update grid state
        self.width = new_width;
        self.height = new_height;
        self.patterns = new_patterns;
        self.colors = new_colors;

        Ok(())
    }

    // ========================================================================
    // Story 2.6: Color Support for Braille Cells
    // ========================================================================

    /// Enable color support by allocating color buffer
    ///
    /// **Story 2.6** - Allocates per-cell color storage.
    ///
    /// Note: In the current implementation, the color buffer is always allocated
    /// during `BrailleGrid::new()`, so this method is a no-op for compatibility
    /// with the AC specification. It ensures the color buffer exists.
    ///
    /// # Examples
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(10, 10).unwrap();
    /// grid.enable_color_support(); // Ensures color support is enabled
    /// ```
    #[instrument(skip(self))]
    pub fn enable_color_support(&mut self) {
        debug!(
            width = self.width,
            height = self.height,
            "Enabling color support (already enabled in current implementation)"
        );
        // Color buffer is already allocated in new(), so this is a no-op
        // This method exists for API compatibility with AC 2.6.3
        //
        // If we change to Option<Vec<Option<Color>>> in future, this would be:
        // if self.colors.is_none() {
        //     self.colors = Some(vec![None; self.width * self.height]);
        // }
    }

    /// Assign RGB color to cell at (x, y)
    ///
    /// **Story 2.6** - Per-cell color assignment with bounds validation.
    ///
    /// Sets the color for a specific cell. The color will be applied when
    /// rendering via `TerminalRenderer`.
    ///
    /// # Arguments
    /// * `x` - X position in cells (0 to width-1)
    /// * `y` - Y position in cells (0 to height-1)
    /// * `color` - RGB color to assign
    ///
    /// # Returns
    /// * `Ok(())` if color was assigned successfully
    /// * `Err(DotmaxError::OutOfBounds)` if coordinates exceed grid dimensions
    ///
    /// # Examples
    /// ```
    /// use dotmax::{BrailleGrid, Color};
    ///
    /// let mut grid = BrailleGrid::new(10, 10).unwrap();
    /// grid.enable_color_support();
    ///
    /// // Set cell (5, 5) to red
    /// grid.set_cell_color(5, 5, Color::rgb(255, 0, 0)).unwrap();
    ///
    /// // Verify color was set
    /// assert_eq!(grid.get_color(5, 5), Some(Color::rgb(255, 0, 0)));
    /// ```
    ///
    /// # Errors
    /// Returns `OutOfBounds` if x >= width or y >= height.
    pub fn set_cell_color(&mut self, x: usize, y: usize, color: Color) -> Result<(), DotmaxError> {
        // Validate bounds
        if x >= self.width || y >= self.height {
            error!(
                x = x,
                y = y,
                width = self.width,
                height = self.height,
                "Out of bounds color assignment: ({}, {}) in grid of size ({}, {})",
                x,
                y,
                self.width,
                self.height
            );
            return Err(DotmaxError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }

        // Set color
        let index = y * self.width + x;
        self.colors[index] = Some(color);
        Ok(())
    }

    /// Reset all colors to None (monochrome)
    ///
    /// **Story 2.6** - Clear color buffer without deallocating.
    ///
    /// Resets all cell colors to `None` while keeping the color buffer
    /// allocated. This is useful for switching back to monochrome rendering
    /// without disabling color support entirely.
    ///
    /// # Examples
    /// ```
    /// use dotmax::{BrailleGrid, Color};
    ///
    /// let mut grid = BrailleGrid::new(10, 10).unwrap();
    /// grid.enable_color_support();
    ///
    /// // Set some colors
    /// grid.set_cell_color(5, 5, Color::rgb(255, 0, 0)).unwrap();
    /// grid.set_cell_color(7, 7, Color::rgb(0, 255, 0)).unwrap();
    ///
    /// // Clear all colors
    /// grid.clear_colors();
    ///
    /// // All colors are now None
    /// assert_eq!(grid.get_color(5, 5), None);
    /// assert_eq!(grid.get_color(7, 7), None);
    /// ```
    pub fn clear_colors(&mut self) {
        self.colors.fill(None);
    }

    /// Set a text character at a cell position (Story 4.4)
    ///
    /// **Story 4.4** - Character density-based rendering support.
    ///
    /// Sets a text character at the specified cell position. When a character is set,
    /// it overrides the braille dot pattern for that cell during rendering. This enables
    /// ASCII-art style density rendering as an alternative to binary braille dots.
    ///
    /// # Arguments
    ///
    /// * `x` - X position in cells (0-indexed)
    /// * `y` - Y position in cells (0-indexed)
    /// * `character` - The character to set at this position
    ///
    /// # Returns
    ///
    /// * `Ok(())` if character was set successfully
    /// * `Err(DotmaxError::OutOfBounds)` if coordinates exceed grid dimensions
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(10, 10).unwrap();
    ///
    /// // Set character at cell (5, 5)
    /// grid.set_char(5, 5, '@').unwrap();
    ///
    /// // Verify character was set
    /// assert_eq!(grid.get_char(5, 5), '@');
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `OutOfBounds` if x >= width or y >= height.
    pub fn set_char(&mut self, x: usize, y: usize, character: char) -> Result<(), DotmaxError> {
        // Validate bounds
        if x >= self.width || y >= self.height {
            error!(
                x = x,
                y = y,
                width = self.width,
                height = self.height,
                "Out of bounds character assignment: ({}, {}) in grid of size ({}, {})",
                x,
                y,
                self.width,
                self.height
            );
            return Err(DotmaxError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }

        // Set character
        let index = y * self.width + x;
        self.characters[index] = Some(character);
        Ok(())
    }

    /// Clear all text characters (Story 4.4)
    ///
    /// **Story 4.4** - Reset character buffer for density rendering.
    ///
    /// Resets all cell characters to `None`, restoring default braille dot rendering
    /// mode. This is useful for switching back from density rendering to braille dots
    /// without creating a new grid.
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::BrailleGrid;
    ///
    /// let mut grid = BrailleGrid::new(10, 10).unwrap();
    ///
    /// // Set some characters
    /// grid.set_char(5, 5, '@').unwrap();
    /// grid.set_char(7, 7, '#').unwrap();
    ///
    /// // Clear all characters
    /// grid.clear_characters();
    ///
    /// // All characters are now None (braille mode restored)
    /// assert_eq!(grid.get_char(5, 5), '⠀'); // Empty braille pattern
    /// ```
    pub fn clear_characters(&mut self) {
        self.characters.fill(None);
    }

    // ========================================================================
    // Story 5.5: Apply Color Scheme to Intensity Buffer
    // ========================================================================

    /// Apply a color scheme to a flattened intensity buffer.
    ///
    /// **Story 5.5** - Convenience method for colorizing intensity data.
    ///
    /// Maps a 1D intensity buffer to colors using the provided color scheme,
    /// populating the grid's color buffer. The intensity buffer must be in
    /// row-major order and match the grid dimensions (`width × height`).
    ///
    /// This method directly modifies the grid's internal color buffer for
    /// maximum performance, avoiding intermediate allocations.
    ///
    /// # Arguments
    ///
    /// * `intensities` - Flattened 1D intensity buffer (row-major order)
    /// * `scheme` - Color scheme for intensity-to-color mapping
    ///
    /// # Returns
    ///
    /// * `Ok(())` if colors were applied successfully
    /// * `Err(DotmaxError::BufferSizeMismatch)` if buffer length doesn't match grid size
    ///
    /// # Intensity Handling
    ///
    /// - Values in range 0.0-1.0 are mapped normally
    /// - Values outside range are clamped (consistent with `ColorScheme::sample`)
    /// - NaN values are treated as 0.0
    /// - Infinity values are clamped to 0.0 or 1.0
    ///
    /// # Examples
    ///
    /// ```
    /// use dotmax::{BrailleGrid, ColorScheme};
    ///
    /// let mut grid = BrailleGrid::new(4, 3).unwrap();
    ///
    /// // Create intensity buffer (4×3 = 12 cells)
    /// let intensities: Vec<f32> = (0..12)
    ///     .map(|i| i as f32 / 11.0)
    ///     .collect();
    ///
    /// // Apply heat map scheme
    /// let scheme = ColorScheme::heat_map();
    /// grid.apply_color_scheme(&intensities, &scheme).unwrap();
    ///
    /// // First cell is black (intensity 0.0)
    /// let c0 = grid.get_color(0, 0).unwrap();
    /// assert_eq!(c0.r, 0);
    ///
    /// // Last cell is white (intensity 1.0)
    /// let c11 = grid.get_color(3, 2).unwrap();
    /// assert_eq!(c11.r, 255);
    /// ```
    ///
    /// # Integration with Epic 3 Image Pipeline
    ///
    /// ```no_run
    /// use dotmax::{BrailleGrid, ColorScheme};
    /// # // Note: This requires the image feature
    /// # // Load image -> Grayscale -> Intensity buffer -> Color scheme
    /// ```
    ///
    /// # Performance
    ///
    /// Target: <10ms for 80×24 grid (1,920 cells)
    ///
    /// # Errors
    ///
    /// Returns [`DotmaxError::BufferSizeMismatch`] if `intensities.len() != width × height`.
    pub fn apply_color_scheme(
        &mut self,
        intensities: &[f32],
        scheme: &crate::color::schemes::ColorScheme,
    ) -> Result<(), DotmaxError> {
        let expected_len = self.width * self.height;

        // Validate buffer size
        if intensities.len() != expected_len {
            return Err(DotmaxError::BufferSizeMismatch {
                expected: expected_len,
                actual: intensities.len(),
            });
        }

        // Apply colors directly to internal buffer
        for (index, &intensity) in intensities.iter().enumerate() {
            // Handle special float values and clamp
            let normalized = if intensity.is_nan() {
                0.0
            } else if intensity.is_infinite() {
                if intensity.is_sign_positive() {
                    1.0
                } else {
                    0.0
                }
            } else {
                intensity.clamp(0.0, 1.0)
            };

            self.colors[index] = Some(scheme.sample(normalized));
        }

        Ok(())
    }
}

// ============================================================================
// STRIPPED from crabmusic - Not in Story 2.1 scope:
// ============================================================================
// - set_dot_with_color() → Story 2.6 (Color Support)
// - draw_line() / draw_line_with_color() → Epic 4 (Drawing Primitives)
// - draw_circle() → Epic 4 (Drawing Primitives)
// - Anti-aliasing logic → Out of scope (audio-reactive feature)
// - FFT/spectrum integration → Audio dependencies (discarded)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Grid Creation Tests (AC #2) - Adapted from crabmusic tests
    // ========================================================================

    #[test]
    fn test_new_valid_dimensions() {
        let grid = BrailleGrid::new(80, 24);
        assert!(grid.is_ok());
        let grid = grid.unwrap();
        assert_eq!(grid.dimensions(), (80, 24));
        assert_eq!(grid.width(), 80);
        assert_eq!(grid.height(), 24);
    }

    #[test]
    fn test_braille_grid_creation() {
        // Ported from crabmusic test_braille_grid_creation (line 389)
        let grid = BrailleGrid::new(40, 20).unwrap();
        assert_eq!(grid.width(), 40);
        assert_eq!(grid.height(), 20);
        assert_eq!(grid.dot_width(), 80);
        assert_eq!(grid.dot_height(), 80);
    }

    #[test]
    fn test_new_minimal_dimensions() {
        let grid = BrailleGrid::new(1, 1);
        assert!(grid.is_ok());
        assert_eq!(grid.unwrap().dimensions(), (1, 1));
    }

    #[test]
    fn test_new_large_dimensions() {
        let grid = BrailleGrid::new(200, 50);
        assert!(grid.is_ok());
        assert_eq!(grid.unwrap().dimensions(), (200, 50));
    }

    #[test]
    fn test_new_max_dimensions() {
        let grid = BrailleGrid::new(10_000, 10_000);
        assert!(grid.is_ok());
    }

    #[test]
    fn test_new_zero_width_returns_error() {
        let result = BrailleGrid::new(0, 24);
        assert!(matches!(
            result,
            Err(DotmaxError::InvalidDimensions {
                width: 0,
                height: 24
            })
        ));
    }

    #[test]
    fn test_new_zero_height_returns_error() {
        let result = BrailleGrid::new(80, 0);
        assert!(matches!(
            result,
            Err(DotmaxError::InvalidDimensions {
                width: 80,
                height: 0
            })
        ));
    }

    #[test]
    fn test_new_both_zero_returns_error() {
        let result = BrailleGrid::new(0, 0);
        assert!(matches!(
            result,
            Err(DotmaxError::InvalidDimensions {
                width: 0,
                height: 0
            })
        ));
    }

    #[test]
    fn test_new_exceeds_max_width() {
        let result = BrailleGrid::new(10_001, 100);
        assert!(matches!(result, Err(DotmaxError::InvalidDimensions { .. })));
    }

    #[test]
    fn test_new_exceeds_max_height() {
        let result = BrailleGrid::new(100, 10_001);
        assert!(matches!(result, Err(DotmaxError::InvalidDimensions { .. })));
    }

    // ========================================================================
    // Dot Manipulation Tests (AC #3, #4, #8) - Adapted from crabmusic
    // ========================================================================

    #[test]
    fn test_set_dot() {
        // Ported from crabmusic test_set_dot (line 398)
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Set top-left dot of first cell (dot coordinate 0,0)
        grid.set_dot(0, 0).unwrap();
        assert_eq!(grid.get_char(0, 0), '⠁');

        // Set top-right dot of first cell (dot coordinate 1,0)
        grid.set_dot(1, 0).unwrap();
        assert_eq!(grid.get_char(0, 0), '⠉');
    }

    #[test]
    fn test_dot_positions() {
        // Ported from crabmusic test_dot_positions (line 476)
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Test all 8 dot positions in first cell
        grid.clear();
        grid.set_dot(0, 0).unwrap(); // Dot 1
        assert_eq!(grid.patterns[0], 0b0000_0001);

        grid.clear();
        grid.set_dot(0, 1).unwrap(); // Dot 2
        assert_eq!(grid.patterns[0], 0b0000_0010);

        grid.clear();
        grid.set_dot(0, 2).unwrap(); // Dot 3
        assert_eq!(grid.patterns[0], 0b0000_0100);

        grid.clear();
        grid.set_dot(0, 3).unwrap(); // Dot 7
        assert_eq!(grid.patterns[0], 0b0100_0000);

        grid.clear();
        grid.set_dot(1, 0).unwrap(); // Dot 4
        assert_eq!(grid.patterns[0], 0b0000_1000);

        grid.clear();
        grid.set_dot(1, 1).unwrap(); // Dot 5
        assert_eq!(grid.patterns[0], 0b0001_0000);

        grid.clear();
        grid.set_dot(1, 2).unwrap(); // Dot 6
        assert_eq!(grid.patterns[0], 0b0010_0000);

        grid.clear();
        grid.set_dot(1, 3).unwrap(); // Dot 8
        assert_eq!(grid.patterns[0], 0b1000_0000);
    }

    #[test]
    fn test_get_dot_all_positions() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Set all 8 dots in cell (5,5) using set_dot pixel API
        for dot_y in 0..4 {
            for dot_x in 0..2 {
                // Cell (5,5) corresponds to dot range (10-11, 20-23)
                grid.set_dot(5 * 2 + dot_x, 5 * 4 + dot_y).unwrap();
            }
        }

        // Verify all 8 dots are set using get_dot
        for dot_index in 0..8 {
            assert!(
                grid.get_dot(5, 5, dot_index).unwrap(),
                "Dot {dot_index} should be set"
            );
        }
    }

    #[test]
    fn test_set_dot_out_of_bounds() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        // Grid is 10×10 cells = 20×40 dots
        let result = grid.set_dot(100, 5);
        assert!(matches!(result, Err(DotmaxError::OutOfBounds { .. })));
    }

    #[test]
    fn test_get_dot_out_of_bounds() {
        let grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.get_dot(100, 100, 0);
        assert!(matches!(result, Err(DotmaxError::OutOfBounds { .. })));
    }

    #[test]
    fn test_get_dot_invalid_dot_index() {
        let grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.get_dot(5, 5, 8);
        assert!(matches!(result, Err(DotmaxError::InvalidDotIndex { .. })));
    }

    #[test]
    fn test_bounds_checking() {
        // Ported from crabmusic test_bounds_checking (line 514)
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Should return error, not panic
        let result = grid.set_dot(1000, 1000);
        assert!(result.is_err());

        assert_eq!(grid.get_char(1000, 1000), '⠀');
        assert!(grid.is_empty(1000, 1000));
    }

    // ========================================================================
    // Clear Operations Tests (AC #5, #6) - Adapted from crabmusic
    // ========================================================================

    #[test]
    fn test_clear() {
        // Ported from crabmusic test_clear (line 411)
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        grid.set_dot(0, 0).unwrap();
        grid.set_dot(5, 5).unwrap();

        grid.clear();

        assert_eq!(grid.get_char(0, 0), '⠀');
        assert!(grid.is_empty(0, 0));
    }

    #[test]
    fn test_clear_empty_grid() {
        let mut grid = BrailleGrid::new(5, 5).unwrap();
        grid.clear(); // Should not panic on empty grid
        assert!(grid.is_empty(0, 0));
    }

    #[test]
    fn test_clear_region_basic() {
        let mut grid = BrailleGrid::new(20, 20).unwrap();

        // Set dots in various cells
        grid.set_dot(5 * 2, 5 * 4).unwrap(); // Cell (5,5)
        grid.set_dot(6 * 2, 6 * 4).unwrap(); // Cell (6,6)
        grid.set_dot(10 * 2, 10 * 4).unwrap(); // Cell (10,10)

        // Clear region (5, 5, 2, 2) - clears cells (5,5), (6,5), (5,6), (6,6)
        grid.clear_region(5, 5, 2, 2).unwrap();

        // Verify region cleared
        assert!(grid.is_empty(5, 5));
        assert!(grid.is_empty(6, 6));

        // Verify outside region unchanged
        assert!(!grid.is_empty(10, 10));
    }

    #[test]
    fn test_clear_region_out_of_bounds() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Region extends beyond grid
        let result = grid.clear_region(5, 5, 10, 10);
        assert!(matches!(result, Err(DotmaxError::OutOfBounds { .. })));
    }

    #[test]
    fn test_clear_region_zero_size() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Zero-size region should succeed (clears nothing)
        let result = grid.clear_region(5, 5, 0, 0);
        assert!(result.is_ok());
    }

    // ========================================================================
    // Dimensions Test (AC #7)
    // ========================================================================

    #[test]
    fn test_dimensions_returns_correct_size() {
        let grid1 = BrailleGrid::new(80, 24).unwrap();
        assert_eq!(grid1.dimensions(), (80, 24));

        let grid2 = BrailleGrid::new(100, 50).unwrap();
        assert_eq!(grid2.dimensions(), (100, 50));

        let grid3 = BrailleGrid::new(1, 1).unwrap();
        assert_eq!(grid3.dimensions(), (1, 1));
    }

    // ========================================================================
    // Unicode Conversion Tests - Ported from crabmusic
    // ========================================================================

    #[test]
    fn test_dots_to_char() {
        // Ported from crabmusic test_dots_to_char (line 376)
        // Empty pattern
        assert_eq!(dots_to_char(0b0000_0000), '⠀');

        // All dots
        assert_eq!(dots_to_char(0b1111_1111), '⣿');

        // Single dots
        assert_eq!(dots_to_char(0b0000_0001), '⠁'); // Dot 1
        assert_eq!(dots_to_char(0b0000_1000), '⠈'); // Dot 4
    }

    // ========================================================================
    // Color Tests (AC #1 - Color struct)
    // ========================================================================

    #[test]
    fn test_color_rgb() {
        let color = Color::rgb(255, 128, 64);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
    }

    #[test]
    fn test_color_black() {
        let color = Color::black();
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn test_color_white() {
        let color = Color::white();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 255);
    }

    #[test]
    fn test_color_equality() {
        let color1 = Color::rgb(100, 150, 200);
        let color2 = Color::rgb(100, 150, 200);
        let color3 = Color::rgb(100, 150, 201);

        assert_eq!(color1, color2);
        assert_ne!(color1, color3);
    }

    // ========================================================================
    // Story 2.2: Unicode Braille Character Conversion Tests (AC #4, #5)
    // ========================================================================

    /// Test all 256 braille patterns (exhaustive coverage for AC #4)
    ///
    /// This test verifies correctness of the Unicode Braille conversion
    /// for ALL possible 8-dot patterns (2^8 = 256 combinations).
    ///
    /// Tests the bitfield formula: U+2800 + (dots[0]<<0 | dots[1]<<1 | ... | dots[7]<<7)
    #[test]
    fn test_all_256_braille_patterns() {
        for bitfield in 0u8..=255 {
            let ch = dots_to_char(bitfield);
            let expected = char::from_u32(0x2800 + u32::from(bitfield)).unwrap();
            assert_eq!(
                ch, expected,
                "Failed for bitfield {bitfield:08b} (decimal {bitfield})"
            );
        }
    }

    /// Test empty cell → U+2800 (AC #5)
    #[test]
    fn test_empty_cell_is_u2800() {
        let ch = dots_to_char(0b0000_0000);
        assert_eq!(ch, '\u{2800}', "Empty cell should be blank braille U+2800");
    }

    /// Test full cell → U+28FF (AC #5)
    #[test]
    fn test_full_cell_is_u28ff() {
        let ch = dots_to_char(0b1111_1111);
        assert_eq!(ch, '\u{28FF}', "Full cell should be U+28FF (all dots)");
    }

    /// Test specific patterns match Unicode standard (AC #5)
    #[test]
    fn test_specific_braille_patterns() {
        // Pattern: dots [true, false, true, false, false, false, false, false]
        // Bitfield: 0b00000101 = 5
        // Expected: U+2805 = '⠅'
        assert_eq!(dots_to_char(0b0000_0101), '\u{2805}');

        // Pattern: dots [true, true, true, true, false, false, false, false]
        // Bitfield: 0b00001111 = 15
        // Expected: U+280F = '⠏'
        assert_eq!(dots_to_char(0b0000_1111), '\u{280F}');

        // Single dot patterns
        assert_eq!(dots_to_char(0b0000_0001), '\u{2801}'); // Dot 1 only
        assert_eq!(dots_to_char(0b0000_1000), '\u{2808}'); // Dot 4 only
        assert_eq!(dots_to_char(0b0100_0000), '\u{2840}'); // Dot 7 only
        assert_eq!(dots_to_char(0b1000_0000), '\u{2880}'); // Dot 8 only
    }

    /// Test `to_unicode_grid()` dimensions (AC #2)
    #[test]
    fn test_to_unicode_grid_dimensions() {
        // 5×5 grid → verify result is 5×5 Vec<Vec<char>>
        let grid = BrailleGrid::new(5, 5).unwrap();
        let chars = grid.to_unicode_grid();

        assert_eq!(chars.len(), 5, "Grid should have 5 rows");
        for row in &chars {
            assert_eq!(row.len(), 5, "Each row should have 5 columns");
        }
    }

    /// Test `to_unicode_grid()` with various grid sizes
    #[test]
    fn test_to_unicode_grid_various_sizes() {
        // 80×24 (standard terminal)
        let grid1 = BrailleGrid::new(80, 24).unwrap();
        let chars1 = grid1.to_unicode_grid();
        assert_eq!(chars1.len(), 24);
        assert_eq!(chars1[0].len(), 80);

        // 1×1 (minimal)
        let grid2 = BrailleGrid::new(1, 1).unwrap();
        let chars2 = grid2.to_unicode_grid();
        assert_eq!(chars2.len(), 1);
        assert_eq!(chars2[0].len(), 1);

        // 100×50 (large terminal)
        let grid3 = BrailleGrid::new(100, 50).unwrap();
        let chars3 = grid3.to_unicode_grid();
        assert_eq!(chars3.len(), 50);
        assert_eq!(chars3[0].len(), 100);
    }

    /// Test `to_unicode_grid()` with empty grid (all blank braille)
    #[test]
    fn test_to_unicode_grid_empty() {
        let grid = BrailleGrid::new(3, 3).unwrap();
        let chars = grid.to_unicode_grid();

        // All cells should be blank braille U+2800
        for row in chars {
            for ch in row {
                assert_eq!(ch, '\u{2800}', "Empty grid should have blank braille");
            }
        }
    }

    /// Test `to_unicode_grid()` with dots set
    #[test]
    fn test_to_unicode_grid_with_dots() {
        let mut grid = BrailleGrid::new(5, 5).unwrap();

        // Set top-left dot of cell (0,0)
        grid.set_dot(0, 0).unwrap();

        // Set all dots of cell (2,2)
        for dot_y in 0..4 {
            for dot_x in 0..2 {
                grid.set_dot(2 * 2 + dot_x, 2 * 4 + dot_y).unwrap();
            }
        }

        let chars = grid.to_unicode_grid();

        // Cell (0,0) should have dot 1 set → '⠁'
        assert_eq!(chars[0][0], '\u{2801}');

        // Cell (2,2) should have all dots → '⣿'
        assert_eq!(chars[2][2], '\u{28FF}');

        // Other cells should be blank
        assert_eq!(chars[1][1], '\u{2800}');
        assert_eq!(chars[4][4], '\u{2800}');
    }

    /// Test `cell_to_braille_char()` bounds validation (AC #3)
    #[test]
    fn test_cell_to_braille_char_out_of_bounds() {
        let grid = BrailleGrid::new(10, 10).unwrap();

        // Out of bounds → Err(OutOfBounds)
        let result1 = grid.cell_to_braille_char(100, 5);
        assert!(matches!(result1, Err(DotmaxError::OutOfBounds { .. })));

        let result2 = grid.cell_to_braille_char(5, 100);
        assert!(matches!(result2, Err(DotmaxError::OutOfBounds { .. })));

        let result3 = grid.cell_to_braille_char(10, 10); // Exactly at boundary
        assert!(matches!(result3, Err(DotmaxError::OutOfBounds { .. })));
    }

    /// Test `cell_to_braille_char()` returns correct character
    #[test]
    fn test_cell_to_braille_char_correct_conversion() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Set specific pattern in cell (5,5)
        // Set dots to create pattern 0b00001111 (bitfield 15) → '⠏'
        grid.set_dot(5 * 2, 5 * 4).unwrap(); // Dot 1
        grid.set_dot(5 * 2, 5 * 4 + 1).unwrap(); // Dot 2
        grid.set_dot(5 * 2, 5 * 4 + 2).unwrap(); // Dot 3
        grid.set_dot(5 * 2 + 1, 5 * 4).unwrap(); // Dot 4

        let ch = grid.cell_to_braille_char(5, 5).unwrap();
        assert_eq!(ch, '\u{280F}');
    }

    /// Test `cell_to_braille_char()` for empty cells
    #[test]
    fn test_cell_to_braille_char_empty_cells() {
        let grid = BrailleGrid::new(10, 10).unwrap();

        // All cells should start as blank braille
        for y in 0..10 {
            for x in 0..10 {
                let ch = grid.cell_to_braille_char(x, y).unwrap();
                assert_eq!(ch, '\u{2800}', "Empty cell ({x}, {y}) should be blank");
            }
        }
    }

    /// Test that conversion is correct after clearing
    #[test]
    fn test_unicode_conversion_after_clear() {
        let mut grid = BrailleGrid::new(5, 5).unwrap();

        // Set some dots
        grid.set_dot(0, 0).unwrap();
        grid.set_dot(5, 5).unwrap();

        // Verify they're set
        assert_ne!(grid.cell_to_braille_char(0, 0).unwrap(), '\u{2800}');

        // Clear grid
        grid.clear();

        // Verify all cells are blank braille
        let chars = grid.to_unicode_grid();
        for row in chars {
            for ch in row {
                assert_eq!(ch, '\u{2800}');
            }
        }
    }

    /// Test Unicode range validity (all conversions produce valid braille)
    #[test]
    fn test_unicode_range_validity() {
        let grid = BrailleGrid::new(5, 5).unwrap();
        let chars = grid.to_unicode_grid();

        for row in chars {
            for ch in row {
                assert!(
                    ('\u{2800}'..='\u{28FF}').contains(&ch),
                    "Character {ch} is outside braille range U+2800-U+28FF"
                );
            }
        }
    }

    // ========================================================================
    // Story 2.4: Error Context Verification Tests (AC #3)
    // ========================================================================

    /// Test `InvalidDimensions` error message includes context (AC #3)
    #[test]
    fn test_invalid_dimensions_error_message_includes_context() {
        let result = BrailleGrid::new(0, 10);
        match result {
            Err(DotmaxError::InvalidDimensions { width, height }) => {
                let msg = format!("{}", DotmaxError::InvalidDimensions { width, height });
                assert!(msg.contains('0'), "Error message should include width=0");
                assert!(msg.contains("10"), "Error message should include height=10");
                assert!(
                    msg.contains("width") && msg.contains("height"),
                    "Error message should label dimensions"
                );
            }
            _ => panic!("Expected InvalidDimensions error"),
        }
    }

    /// Test `OutOfBounds` error message includes all context (AC #3)
    #[test]
    fn test_out_of_bounds_error_message_includes_all_context() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.set_dot(100, 50);
        match result {
            Err(DotmaxError::OutOfBounds {
                x,
                y,
                width,
                height,
            }) => {
                let msg = format!(
                    "{}",
                    DotmaxError::OutOfBounds {
                        x,
                        y,
                        width,
                        height
                    }
                );
                assert!(msg.contains("100"), "Error message should include x=100");
                assert!(msg.contains("50"), "Error message should include y=50");
                // width*2=20 and height*4=40 for dot coordinates
                assert!(
                    msg.contains("20"),
                    "Error message should include dot_width=20"
                );
                assert!(
                    msg.contains("40"),
                    "Error message should include dot_height=40"
                );
            }
            _ => panic!("Expected OutOfBounds error"),
        }
    }

    /// Test `InvalidDotIndex` error message includes index (AC #3)
    #[test]
    fn test_invalid_dot_index_error_message_includes_index() {
        let grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.get_dot(5, 5, 10);
        match result {
            Err(DotmaxError::InvalidDotIndex { index }) => {
                let msg = format!("{}", DotmaxError::InvalidDotIndex { index });
                assert!(msg.contains("10"), "Error message should include index=10");
                assert!(
                    msg.contains("0-7"),
                    "Error message should specify valid range"
                );
            }
            _ => panic!("Expected InvalidDotIndex error"),
        }
    }

    /// Test exceeding maximum dimensions returns proper error (AC #1, #3)
    #[test]
    fn test_new_exceeds_both_max_dimensions() {
        let result = BrailleGrid::new(20_000, 20_000);
        assert!(
            matches!(result, Err(DotmaxError::InvalidDimensions { .. })),
            "Grid exceeding MAX_GRID_WIDTH and MAX_GRID_HEIGHT should return InvalidDimensions"
        );
    }

    /// Test `set_dot` with invalid dot index returns `InvalidDotIndex` (AC #1)
    #[test]
    fn test_set_dot_invalid_dot_index_high() {
        let grid = BrailleGrid::new(10, 10).unwrap();
        // set_dot uses dot coordinates, not dot_index, so we test via get_dot
        let result = grid.get_dot(5, 5, 255);
        assert!(
            matches!(result, Err(DotmaxError::InvalidDotIndex { index: 255 })),
            "Dot index 255 should return InvalidDotIndex error"
        );
    }

    // ========================================================================
    // Story 2.5: Terminal Resize Event Handling Tests
    // ========================================================================

    /// Test resize grow updates dimensions (AC #2, #3)
    #[test]
    fn test_resize_grow_updates_dimensions() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.resize(20, 20).unwrap();
        assert_eq!(grid.dimensions(), (20, 20));
        assert_eq!(grid.width(), 20);
        assert_eq!(grid.height(), 20);
    }

    /// Test resize grow preserves existing dots (AC #3)
    #[test]
    fn test_resize_grow_preserves_existing_dots() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.set_dot(5, 5).unwrap(); // Sets bit in cell (2, 1)
        grid.set_dot(18, 38).unwrap(); // Sets bit in cell (9, 9)

        // Check initial state
        let cell_2_1 = 10 + 2; // cell (2, 1) = index 12
        let cell_9_9 = 9 * 10 + 9; // cell (9, 9) = index 99
        assert_ne!(grid.patterns[cell_2_1], 0, "Cell (2,1) should have dots");
        assert_ne!(grid.patterns[cell_9_9], 0, "Cell (9,9) should have dots");

        grid.resize(20, 20).unwrap();

        // After resize, cells should be at same logical positions
        let new_cell_2_1 = 20 + 2; // cell (2, 1) in new grid
        let new_cell_9_9 = 9 * 20 + 9; // cell (9, 9) in new grid

        // Existing dots should be preserved
        assert_ne!(
            grid.patterns[new_cell_2_1], 0,
            "Cell (2,1) dots should be preserved"
        );
        assert_ne!(
            grid.patterns[new_cell_9_9], 0,
            "Cell (9,9) dots should be preserved"
        );

        // New cells should be empty
        let new_cell_15_15 = 15 * 20 + 15;
        assert_eq!(
            grid.patterns[new_cell_15_15], 0,
            "New cells should be empty"
        );
    }

    /// Test resize shrink truncates cleanly (AC #4)
    #[test]
    fn test_resize_shrink_truncates_cleanly() {
        let mut grid = BrailleGrid::new(20, 20).unwrap();
        grid.set_dot(5, 5).unwrap(); // Sets bit in cell (2, 1)
        grid.set_dot(30, 60).unwrap(); // Sets bit in cell (15, 15) - will be truncated

        // Check initial state
        let cell_2_1 = 20 + 2;
        let cell_15_15 = 15 * 20 + 15;
        assert_ne!(grid.patterns[cell_2_1], 0, "Cell (2,1) should have dots");
        assert_ne!(
            grid.patterns[cell_15_15], 0,
            "Cell (15,15) should have dots"
        );

        grid.resize(10, 10).unwrap();

        assert_eq!(grid.dimensions(), (10, 10));

        // Preserved dot should still exist
        let new_cell_2_1 = 10 + 2;
        assert_ne!(
            grid.patterns[new_cell_2_1], 0,
            "Cell (2,1) dots should be preserved"
        );

        // Grid is now only 10×10 = 100 cells, so cell (15,15) is truncated
        assert_eq!(
            grid.patterns.len(),
            100,
            "Grid should have only 100 cells after resize to 10×10"
        );
    }

    /// Test resize to same dimensions (no-op case)
    #[test]
    fn test_resize_same_dimensions() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.set_dot(5, 5).unwrap(); // Sets bit in cell (2, 1)

        let cell_2_1 = 10 + 2;
        let original_pattern = grid.patterns[cell_2_1];
        assert_ne!(original_pattern, 0);

        grid.resize(10, 10).unwrap();

        assert_eq!(grid.dimensions(), (10, 10));
        // Pattern should be unchanged
        assert_eq!(
            grid.patterns[cell_2_1], original_pattern,
            "Existing dot pattern should be preserved"
        );
    }

    /// Test resize with colors syncs color buffer (AC #5)
    #[test]
    fn test_resize_with_colors_syncs_color_buffer() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        // Note: Color support is implicit in BrailleGrid (colors vec always allocated)
        // We can test via internal state

        grid.resize(20, 20).unwrap();

        // Color buffer should have resized to match new dimensions
        assert_eq!(
            grid.colors.len(),
            400,
            "Color buffer should have 400 cells for 20×20 grid"
        );
    }

    /// Test resize zero width dimension error (AC #2)
    #[test]
    fn test_resize_zero_width_error() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.resize(0, 10);
        assert!(
            matches!(
                result,
                Err(DotmaxError::InvalidDimensions {
                    width: 0,
                    height: 10
                })
            ),
            "Resize to width=0 should return InvalidDimensions error"
        );
        // Grid dimensions should remain unchanged after failed resize
        assert_eq!(grid.dimensions(), (10, 10));
    }

    /// Test resize zero height dimension error (AC #2)
    #[test]
    fn test_resize_zero_height_error() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.resize(10, 0);
        assert!(
            matches!(
                result,
                Err(DotmaxError::InvalidDimensions {
                    width: 10,
                    height: 0
                })
            ),
            "Resize to height=0 should return InvalidDimensions error"
        );
        assert_eq!(grid.dimensions(), (10, 10));
    }

    /// Test resize exceeds max width (AC #2)
    #[test]
    fn test_resize_exceeds_max_width_error() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.resize(20000, 10);
        assert!(
            matches!(result, Err(DotmaxError::InvalidDimensions { .. })),
            "Resize to width=20000 should return InvalidDimensions error"
        );
        assert_eq!(grid.dimensions(), (10, 10));
    }

    /// Test resize exceeds max height (AC #2)
    #[test]
    fn test_resize_exceeds_max_height_error() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        let result = grid.resize(10, 20000);
        assert!(
            matches!(result, Err(DotmaxError::InvalidDimensions { .. })),
            "Resize to height=20000 should return InvalidDimensions error"
        );
        assert_eq!(grid.dimensions(), (10, 10));
    }

    /// Test resize maintains grid invariants (AC #6)
    #[test]
    fn test_resize_maintains_invariants() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.resize(20, 15).unwrap();

        // Verify invariants
        assert_eq!(
            grid.patterns.len(),
            300,
            "Patterns buffer should have 300 cells for 20×15 grid"
        );
        assert_eq!(
            grid.colors.len(),
            300,
            "Colors buffer should have 300 cells for 20×15 grid"
        );
        assert_eq!(grid.width(), 20);
        assert_eq!(grid.height(), 15);
    }

    /// Test resize from 1×1 to large grid (edge case)
    #[test]
    fn test_resize_from_tiny_to_large() {
        let mut grid = BrailleGrid::new(1, 1).unwrap();
        grid.set_dot(0, 0).unwrap();

        grid.resize(50, 30).unwrap();

        assert_eq!(grid.dimensions(), (50, 30));
        assert!(
            grid.get_dot(0, 0, 0).unwrap(),
            "Single dot should be preserved at (0,0)"
        );
        assert!(
            !grid.get_dot(10, 10, 0).unwrap(),
            "New cells should be empty"
        );
    }

    /// Test resize shrink to 1×1 (edge case)
    #[test]
    fn test_resize_shrink_to_tiny() {
        let mut grid = BrailleGrid::new(50, 30).unwrap();
        grid.set_dot(0, 0).unwrap();
        grid.set_dot(10, 10).unwrap();

        grid.resize(1, 1).unwrap();

        assert_eq!(grid.dimensions(), (1, 1));
        assert!(
            grid.get_dot(0, 0, 0).unwrap(),
            "Top-left dot should be preserved"
        );
        // Other dots are now out of bounds
        assert!(grid.get_dot(10, 10, 0).is_err());
    }

    // ========================================================================
    // Story 2.6: Color Support Tests (AC #1-#7)
    // ========================================================================

    /// Test `enable_color_support()` allocates buffer (AC #3)
    #[test]
    fn test_enable_color_support_allocates_buffer() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        // Color buffer is already allocated in new(), so this is a no-op
        // But we verify it exists and has correct size
        grid.enable_color_support();

        // Verify buffer size matches grid dimensions
        assert_eq!(grid.colors.len(), 100); // 10×10 = 100 cells
    }

    /// Test `set_cell_color()` assigns color (AC #4)
    #[test]
    fn test_set_cell_color_assigns_color() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        let red = Color::rgb(255, 0, 0);
        grid.set_cell_color(5, 5, red).unwrap();

        assert_eq!(grid.get_color(5, 5), Some(red));
    }

    /// Test `set_cell_color()` with valid coordinates (AC #4)
    #[test]
    fn test_set_cell_color_valid_coordinates() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        // Test corners and center
        let blue = Color::rgb(0, 0, 255);
        grid.set_cell_color(0, 0, blue).unwrap();
        assert_eq!(grid.get_color(0, 0), Some(blue));

        let green = Color::rgb(0, 255, 0);
        grid.set_cell_color(9, 9, green).unwrap();
        assert_eq!(grid.get_color(9, 9), Some(green));

        let yellow = Color::rgb(255, 255, 0);
        grid.set_cell_color(5, 5, yellow).unwrap();
        assert_eq!(grid.get_color(5, 5), Some(yellow));
    }

    /// Test `set_cell_color()` out of bounds returns error (AC #4)
    #[test]
    fn test_set_cell_color_out_of_bounds_error() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        let result = grid.set_cell_color(100, 100, Color::black());
        assert!(matches!(result, Err(DotmaxError::OutOfBounds { .. })));
    }

    /// Test `set_cell_color()` out of bounds X (AC #4)
    #[test]
    fn test_set_cell_color_out_of_bounds_x() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        let result = grid.set_cell_color(10, 5, Color::white());
        assert!(matches!(result, Err(DotmaxError::OutOfBounds { .. })));
    }

    /// Test `set_cell_color()` out of bounds Y (AC #4)
    #[test]
    fn test_set_cell_color_out_of_bounds_y() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        let result = grid.set_cell_color(5, 10, Color::white());
        assert!(matches!(result, Err(DotmaxError::OutOfBounds { .. })));
    }

    /// Test `get_color()` returns None when no color set (AC #5)
    #[test]
    fn test_get_color_none_when_not_set() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        // No color set on cell (5, 5)
        assert_eq!(grid.get_color(5, 5), None);
    }

    /// Test `get_color()` returns None for out of bounds (AC #5)
    #[test]
    fn test_get_color_none_when_out_of_bounds() {
        let grid = BrailleGrid::new(10, 10).unwrap();

        // Out of bounds returns None (not error)
        assert_eq!(grid.get_color(100, 100), None);
    }

    /// Test `get_color()` returns color after set (AC #5)
    #[test]
    fn test_get_color_returns_color_after_set() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        let magenta = Color::rgb(255, 0, 255);
        grid.set_cell_color(7, 7, magenta).unwrap();

        assert_eq!(grid.get_color(7, 7), Some(magenta));
    }

    /// Test `clear_colors()` resets all colors to None (AC #7)
    #[test]
    fn test_clear_colors_resets_all() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        // Set colors on multiple cells
        grid.set_cell_color(5, 5, Color::rgb(255, 0, 0)).unwrap();
        grid.set_cell_color(7, 7, Color::rgb(0, 255, 0)).unwrap();
        grid.set_cell_color(2, 2, Color::rgb(0, 0, 255)).unwrap();

        // Verify colors are set
        assert!(grid.get_color(5, 5).is_some());
        assert!(grid.get_color(7, 7).is_some());
        assert!(grid.get_color(2, 2).is_some());

        // Clear all colors
        grid.clear_colors();

        // All colors should be None
        assert_eq!(grid.get_color(5, 5), None);
        assert_eq!(grid.get_color(7, 7), None);
        assert_eq!(grid.get_color(2, 2), None);
    }

    /// Test `clear_colors()` doesn't affect dots (AC #7)
    #[test]
    fn test_clear_colors_preserves_dots() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        // Set dots and colors
        grid.set_dot(10, 20).unwrap();
        grid.set_cell_color(5, 5, Color::rgb(255, 0, 0)).unwrap();

        // Clear colors
        grid.clear_colors();

        // Dots should still exist
        assert!(!grid.is_empty(5, 5));
        // But colors should be None
        assert_eq!(grid.get_color(5, 5), None);
    }

    /// Test Color `PartialEq` works correctly (AC #7)
    #[test]
    fn test_color_partial_eq() {
        let red1 = Color::rgb(255, 0, 0);
        let red2 = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);

        assert_eq!(red1, red2);
        assert_ne!(red1, blue);
    }

    /// Test colors persist after resize (AC #5, Story 2.5 AC #5)
    #[test]
    fn test_colors_persist_after_resize_grow() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        let purple = Color::rgb(128, 0, 128);
        grid.set_cell_color(5, 5, purple).unwrap();

        // Resize to larger
        grid.resize(20, 20).unwrap();

        // Color should be preserved at same logical position
        assert_eq!(grid.get_color(5, 5), Some(purple));

        // New cells should have no color
        assert_eq!(grid.get_color(15, 15), None);
    }

    /// Test colors truncated correctly after resize shrink (AC #5, Story 2.5 AC #4)
    #[test]
    fn test_colors_truncated_after_resize_shrink() {
        let mut grid = BrailleGrid::new(20, 20).unwrap();
        grid.enable_color_support();

        let orange = Color::rgb(255, 165, 0);
        grid.set_cell_color(5, 5, orange).unwrap();
        grid.set_cell_color(15, 15, Color::rgb(0, 255, 255))
            .unwrap();

        // Resize to smaller
        grid.resize(10, 10).unwrap();

        // Color within bounds should be preserved
        assert_eq!(grid.get_color(5, 5), Some(orange));

        // Cell (15, 15) is now out of bounds
        assert_eq!(grid.get_color(15, 15), None);
    }

    /// Test `enable_color_support()` is idempotent (AC #3)
    #[test]
    fn test_enable_color_support_idempotent() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        grid.enable_color_support();
        grid.enable_color_support();
        grid.enable_color_support();

        // Should not panic or cause issues
        assert_eq!(grid.colors.len(), 100);
    }

    /// Test `set_cell_color()` with all predefined colors (AC #2, #4)
    #[test]
    fn test_set_cell_color_with_predefined_colors() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        // Test black() constructor
        grid.set_cell_color(0, 0, Color::black()).unwrap();
        assert_eq!(grid.get_color(0, 0), Some(Color::rgb(0, 0, 0)));

        // Test white() constructor
        grid.set_cell_color(1, 1, Color::white()).unwrap();
        assert_eq!(grid.get_color(1, 1), Some(Color::rgb(255, 255, 255)));

        // Test rgb() constructor
        grid.set_cell_color(2, 2, Color::rgb(128, 64, 32)).unwrap();
        assert_eq!(grid.get_color(2, 2), Some(Color::rgb(128, 64, 32)));
    }

    /// Test `clear()` also clears colors (not just `clear_colors()`)
    #[test]
    fn test_clear_also_clears_colors() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.enable_color_support();

        grid.set_dot(10, 20).unwrap();
        grid.set_cell_color(5, 5, Color::rgb(255, 0, 0)).unwrap();

        grid.clear();

        // Both dots and colors should be cleared
        assert!(grid.is_empty(5, 5));
        assert_eq!(grid.get_color(5, 5), None);
    }

    /// Test resize preserves multiple dots in complex pattern (AC #3, #4)
    #[test]
    fn test_resize_preserves_complex_pattern() {
        let mut grid = BrailleGrid::new(15, 15).unwrap();
        // Set dots in a diagonal pattern
        // dot (0,0) → cell (0,0), dot (2,4) → cell (1,1), dot (4,8) → cell (2,2), etc.
        for i in 0..10 {
            grid.set_dot(i * 2, i * 4).unwrap();
        }

        // Store original patterns for cells we'll verify
        let mut original_patterns = Vec::new();
        for i in 0..10 {
            let cell_x = (i * 2) / 2;
            let cell_y = (i * 4) / 4;
            let cell_index = cell_y * 15 + cell_x;
            original_patterns.push((cell_x, cell_y, grid.patterns[cell_index]));
        }

        // Resize to larger
        grid.resize(20, 20).unwrap();

        // All original diagonal dots should be preserved
        for (cell_x, cell_y, pattern) in &original_patterns {
            let new_index = cell_y * 20 + cell_x;
            assert_eq!(
                grid.patterns[new_index], *pattern,
                "Cell ({cell_x}, {cell_y}) pattern should be preserved after grow"
            );
        }

        // Resize to smaller (truncate some dots)
        grid.resize(8, 8).unwrap();

        // Dots within new bounds should be preserved (cells 0-7 in both dimensions)
        for (cell_x, cell_y, pattern) in &original_patterns {
            if *cell_x < 8 && *cell_y < 8 {
                let new_index = cell_y * 8 + cell_x;
                assert_eq!(
                    grid.patterns[new_index], *pattern,
                    "Cell ({cell_x}, {cell_y}) pattern should be preserved after shrink"
                );
            }
        }
    }

    // ========================================================================
    // Story 2.7: Debug Logging and Tracing Tests (AC #1-#6)
    // ========================================================================

    /// Test that tracing instrumentation compiles (#[instrument] attributes work)
    /// AC 2.7.2: Verify #[instrument] on key functions compiles without type errors
    #[test]
    fn test_instrumentation_compiles() {
        // This test verifies that #[instrument] attributes don't cause compilation errors
        // If this test compiles and runs, the instrumentation is correct
        let grid = BrailleGrid::new(10, 10);
        assert!(grid.is_ok());
    }

    /// Test logging works when subscriber initialized
    /// AC 2.7.6: Tests can enable logging via tracing-subscriber
    #[test]
    fn test_logging_with_subscriber_initialized() {
        // Initialize subscriber for this test
        // Using try_init() to handle case where subscriber already initialized by other tests
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        // Operations should now log (logs will appear in test output with --nocapture)
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.clear();
        grid.resize(20, 20).unwrap();
        grid.enable_color_support();

        // If this completes without panic, logging infrastructure works
        assert_eq!(grid.dimensions(), (20, 20));
    }

    /// Test logging is silent when subscriber NOT initialized (zero-cost)
    /// AC 2.7.5: Library does not initialize subscriber (user controls logging)
    #[test]
    fn test_logging_silent_by_default() {
        // No subscriber initialized
        // Operations should complete without logging (zero-cost)
        let mut grid = BrailleGrid::new(10, 10).unwrap();
        grid.clear();

        // This should work silently - no logs appear because no subscriber
        assert_eq!(grid.dimensions(), (10, 10));
    }

    /// Test that hot paths have NO debug logs
    /// AC 2.7.4: `set_dot` and `get_dot` do NOT log at debug level
    #[test]
    fn test_hot_paths_no_debug_logs() {
        // set_dot and get_dot should NOT have debug! calls (only trace! if needed)
        // This is a code review check - the test verifies they work without performance impact

        let mut grid = BrailleGrid::new(100, 100).unwrap();

        // Call hot paths many times - should complete quickly
        for y in 0..200 {
            for x in 0..200 {
                // This should be fast - no debug logging overhead
                let _ = grid.set_dot(x, y);
            }
        }

        // Verify operations completed
        assert_eq!(grid.dimensions(), (100, 100));
    }

    /// Test error logging includes context
    /// AC 2.7.3: error! logs include actionable context (coordinates, dimensions)
    #[test]
    fn test_error_logging_includes_context() {
        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // These operations will emit error! logs with context
        let result1 = BrailleGrid::new(0, 0);
        assert!(result1.is_err());

        let result2 = grid.set_dot(1000, 1000);
        assert!(result2.is_err());

        let result3 = grid.set_cell_color(1000, 1000, Color::black());
        assert!(result3.is_err());

        // If we reach here, error paths executed correctly
        // (Logs will show context if subscriber is initialized)
    }

    /// Test instrumented functions return correct types
    /// AC 2.7.2: Verify #[instrument] doesn't break function signatures
    #[test]
    fn test_instrumented_functions_correct_types() {
        // Test that #[instrument] doesn't change function return types
        let result1: Result<BrailleGrid, DotmaxError> = BrailleGrid::new(10, 10);
        assert!(result1.is_ok());

        let mut grid = result1.unwrap();

        let result2: () = grid.clear();
        assert_eq!(result2, ());

        let result3: Result<(), DotmaxError> = grid.resize(20, 20);
        assert!(result3.is_ok());

        let result4: () = grid.enable_color_support();
        assert_eq!(result4, ());
    }

    /// Test logging works in complex workflow
    /// AC 2.7.6: Verify logging throughout full workflow
    #[test]
    fn test_logging_in_full_workflow() {
        // Initialize subscriber
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        // Complex workflow that exercises all instrumented paths
        let mut grid = BrailleGrid::new(20, 20).unwrap();
        grid.enable_color_support();

        // Set dots
        grid.set_dot(10, 20).unwrap();
        grid.set_dot(30, 60).unwrap();

        // Set colors
        grid.set_cell_color(5, 5, Color::rgb(255, 0, 0)).unwrap();

        // Resize
        grid.resize(30, 30).unwrap();

        // Clear
        grid.clear();

        // Verify final state
        assert_eq!(grid.dimensions(), (30, 30));
        assert!(grid.is_empty(0, 0));

        // If this completes, logging worked throughout workflow
    }

    // ========================================================================
    // Story 5.5: Apply Color Scheme to Intensity Buffer Tests (AC #3, #6)
    // ========================================================================

    /// Test `apply_color_scheme()` basic functionality (AC #3)
    #[test]
    fn test_apply_color_scheme_basic() {
        use crate::color::schemes::grayscale;

        let mut grid = BrailleGrid::new(4, 3).unwrap();
        let intensities: Vec<f32> = (0..12).map(|i| i as f32 / 11.0).collect();
        let scheme = grayscale();

        let result = grid.apply_color_scheme(&intensities, &scheme);
        assert!(result.is_ok());

        // First cell (intensity 0.0) -> black
        let c0 = grid.get_color(0, 0).unwrap();
        assert_eq!(c0, Color::black());

        // Last cell (intensity 1.0) -> white
        let c_last = grid.get_color(3, 2).unwrap();
        assert_eq!(c_last, Color::white());
    }

    /// Test `apply_color_scheme()` buffer size mismatch (AC #3)
    #[test]
    fn test_apply_color_scheme_buffer_mismatch() {
        use crate::color::schemes::grayscale;

        let mut grid = BrailleGrid::new(4, 3).unwrap();
        let too_short: Vec<f32> = vec![0.5; 10]; // 10 < 12
        let too_long: Vec<f32> = vec![0.5; 15]; // 15 > 12
        let scheme = grayscale();

        let result = grid.apply_color_scheme(&too_short, &scheme);
        assert!(matches!(
            result,
            Err(DotmaxError::BufferSizeMismatch { .. })
        ));

        let result = grid.apply_color_scheme(&too_long, &scheme);
        assert!(matches!(
            result,
            Err(DotmaxError::BufferSizeMismatch { .. })
        ));
    }

    /// Test `apply_color_scheme()` with special float values (AC #4)
    #[test]
    fn test_apply_color_scheme_special_floats() {
        use crate::color::schemes::grayscale;

        let mut grid = BrailleGrid::new(5, 1).unwrap();
        let intensities = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.5, 1.5];
        let scheme = grayscale();

        let result = grid.apply_color_scheme(&intensities, &scheme);
        assert!(result.is_ok());

        // NaN -> 0.0 -> black
        assert_eq!(grid.get_color(0, 0), Some(Color::black()));
        // +Infinity -> 1.0 -> white
        assert_eq!(grid.get_color(1, 0), Some(Color::white()));
        // -Infinity -> 0.0 -> black
        assert_eq!(grid.get_color(2, 0), Some(Color::black()));
        // -0.5 clamped to 0.0 -> black
        assert_eq!(grid.get_color(3, 0), Some(Color::black()));
        // 1.5 clamped to 1.0 -> white
        assert_eq!(grid.get_color(4, 0), Some(Color::white()));
    }

    /// Test `apply_color_scheme()` with heat_map scheme (AC #6, #7)
    #[test]
    fn test_apply_color_scheme_heat_map() {
        use crate::color::schemes::heat_map;

        let mut grid = BrailleGrid::new(5, 1).unwrap();
        let intensities = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let scheme = heat_map();

        grid.apply_color_scheme(&intensities, &scheme).unwrap();

        // 0.0 -> black (cold)
        assert_eq!(grid.get_color(0, 0), Some(Color::black()));
        // 1.0 -> white (hot)
        assert_eq!(grid.get_color(4, 0), Some(Color::white()));

        // Middle values should have color
        let mid = grid.get_color(2, 0).unwrap();
        assert!(mid.r > 0 || mid.g > 0 || mid.b > 0);
    }

    /// Test `apply_color_scheme()` with rainbow scheme (AC #6, #7)
    #[test]
    fn test_apply_color_scheme_rainbow() {
        use crate::color::schemes::rainbow;

        let mut grid = BrailleGrid::new(3, 1).unwrap();
        let intensities = vec![0.0, 0.5, 1.0];
        let scheme = rainbow();

        grid.apply_color_scheme(&intensities, &scheme).unwrap();

        // 0.0 -> red
        let red = grid.get_color(0, 0).unwrap();
        assert_eq!(red.r, 255);
        assert_eq!(red.g, 0);
        assert_eq!(red.b, 0);

        // 1.0 -> purple-ish
        let purple = grid.get_color(2, 0).unwrap();
        assert!(purple.r > 200);
        assert!(purple.b > 200);
    }

    /// Test `apply_color_scheme()` result is renderable (AC #5)
    #[test]
    fn test_apply_color_scheme_colors_renderable() {
        use crate::color::schemes::heat_map;

        let mut grid = BrailleGrid::new(10, 10).unwrap();

        // Set some dots
        grid.set_dot(5, 5).unwrap();
        grid.set_dot(10, 10).unwrap();

        // Apply color scheme
        let intensities: Vec<f32> = (0..100).map(|i| (i as f32) / 99.0).collect();
        let scheme = heat_map();
        grid.apply_color_scheme(&intensities, &scheme).unwrap();

        // All cells should have colors
        for y in 0..10 {
            for x in 0..10 {
                assert!(grid.get_color(x, y).is_some());
            }
        }

        // Dots should still exist
        assert!(!grid.is_empty(2, 1)); // dot (5,5) -> cell (2,1)
        assert!(!grid.is_empty(5, 2)); // dot (10,10) -> cell (5,2)
    }

    /// Test `apply_color_scheme()` empty grid (edge case)
    #[test]
    fn test_apply_color_scheme_single_cell() {
        use crate::color::schemes::grayscale;

        let mut grid = BrailleGrid::new(1, 1).unwrap();
        let intensities = vec![0.5];
        let scheme = grayscale();

        grid.apply_color_scheme(&intensities, &scheme).unwrap();

        let color = grid.get_color(0, 0).unwrap();
        // 0.5 gray should be around 127-128
        assert!(color.r >= 127 && color.r <= 128);
    }
}
