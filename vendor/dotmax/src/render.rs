//! Terminal rendering module
//!
//! Provides terminal rendering abstractions for braille grids using ratatui and crossterm.
//! Extracted and adapted from crabmusic/src/rendering/mod.rs
//!
//! # Examples
//!
//! ```no_run
//! use dotmax::{BrailleGrid, TerminalRenderer};
//!
//! let mut renderer = TerminalRenderer::new().expect("Failed to initialize terminal");
//! let mut grid = BrailleGrid::new(80, 24).expect("Failed to create grid");
//!
//! // Set some dots
//! grid.set_dot(10, 10).expect("Failed to set dot");
//!
//! // Render to terminal
//! renderer.render(&grid).expect("Failed to render");
//!
//! // Clean up
//! renderer.cleanup().expect("Failed to cleanup");
//! ```

use crate::error::DotmaxError;
use crate::grid::BrailleGrid;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};
use std::io::{self, Stdout};

// Tracing for structured logging (Story 2.7)
use tracing::{debug, error, info, instrument};

// ============================================================================
// Error Handling - Extends DotmaxError for terminal operations
// ============================================================================

// DotmaxError::Terminal automatically converts from std::io::Error via #[from]
// in src/grid.rs:41

// ============================================================================
// Terminal Capabilities Detection
// ============================================================================

/// Terminal type detection for viewport handling
///
/// Different terminals report their dimensions differently:
/// - Some report the visible viewport size (what the user sees)
/// - Others report the buffer size (which can be larger, enabling scrollback)
///
/// This enum categorizes terminals based on their reporting behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalType {
    /// Windows Terminal - reports viewport size correctly
    WindowsTerminal,
    /// WSL (Windows Subsystem for Linux) - may report buffer size
    Wsl,
    /// `PowerShell` / cmd - reports buffer size (not viewport)
    WindowsConsole,
    /// macOS Terminal.app
    MacOsTerminal,
    /// Ubuntu native terminal (gnome-terminal, etc.)
    LinuxNative,
    /// Unknown terminal - use conservative defaults
    Unknown,
}

impl TerminalType {
    /// Detect the terminal type from environment variables
    ///
    /// Uses environment variables to identify the terminal emulator:
    /// - `WT_SESSION`: Windows Terminal
    /// - `WSL_DISTRO_NAME`: WSL environment
    /// - `TERM_PROGRAM`: macOS Terminal.app or other
    /// - Platform detection for fallback
    #[must_use]
    pub fn detect() -> Self {
        // Check for WSL first (highest priority)
        // WSL reports buffer size even when running in Windows Terminal
        if std::env::var("WSL_DISTRO_NAME").is_ok() {
            return Self::Wsl;
        }

        // Check for Windows Terminal with PowerShell
        // PowerShell in Windows Terminal also reports buffer size
        if std::env::var("WT_SESSION").is_ok() {
            // Check if we're running in PowerShell
            #[cfg(target_os = "windows")]
            {
                if std::env::var("PSModulePath").is_ok() {
                    // This is PowerShell running in Windows Terminal
                    return Self::WindowsConsole;
                }
            }
            return Self::WindowsTerminal;
        }

        // Check for macOS Terminal.app
        if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
            if term_program == "Apple_Terminal" {
                return Self::MacOsTerminal;
            }
        }

        // Platform-based detection
        #[cfg(target_os = "windows")]
        {
            return Self::WindowsConsole;
        }

        #[cfg(target_os = "macos")]
        {
            return Self::MacOsTerminal;
        }

        #[cfg(target_os = "linux")]
        {
            // If we're on Linux but not WSL, it's native Linux
            Self::LinuxNative
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Self::Unknown
        }
    }

    /// Calculate viewport height offset based on terminal type
    ///
    /// Returns the number of rows to subtract from the reported terminal size
    /// to get the actual visible viewport. This compensates for terminals that
    /// report buffer size instead of viewport size.
    ///
    /// # Arguments
    /// * `reported_height` - The height reported by the terminal
    ///
    /// # Returns
    /// The number of rows to subtract to get the visible viewport height
    #[must_use]
    pub const fn viewport_height_offset(self, reported_height: u16) -> u16 {
        match self {
            // WSL and Windows Console both report buffer size, not viewport size
            // Empirically tested: -12 offset works correctly for both
            Self::Wsl | Self::WindowsConsole => {
                if reported_height > 20 {
                    12 // Viewport is typically 12 rows smaller than buffer
                } else {
                    0 // Small terminals, don't apply offset
                }
            }

            // All other terminals report viewport size correctly
            Self::WindowsTerminal | Self::MacOsTerminal | Self::LinuxNative | Self::Unknown => 0,
        }
    }

    /// Get a human-readable name for this terminal type
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WindowsTerminal => "Windows Terminal",
            Self::Wsl => "WSL",
            Self::WindowsConsole => "Windows Console (PowerShell/cmd)",
            Self::MacOsTerminal => "macOS Terminal.app",
            Self::LinuxNative => "Linux Native Terminal",
            Self::Unknown => "Unknown Terminal",
        }
    }
}

/// Terminal capabilities information
///
/// Provides information about what features the terminal supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    /// Whether the terminal supports basic ANSI colors (16 colors)
    pub supports_color: bool,
    /// Whether the terminal supports true color (24-bit RGB)
    pub supports_truecolor: bool,
    /// Whether the terminal supports Unicode braille characters (U+2800-U+28FF)
    pub supports_unicode: bool,
    /// The detected terminal type (for viewport detection)
    pub terminal_type: TerminalType,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        // Modern terminals typically support all these features
        // Crossterm handles platform differences automatically
        Self {
            supports_color: true,
            supports_truecolor: true,
            supports_unicode: true,
            terminal_type: TerminalType::detect(),
        }
    }
}

// ============================================================================
// Terminal Backend Trait - ADR 0004
// ============================================================================

/// Terminal backend abstraction
///
/// This trait abstracts terminal I/O operations to enable testing with mock
/// backends and reduce lock-in to specific terminal libraries.
///
/// Implements ADR 0004: Terminal Backend Abstraction via Trait
pub trait TerminalBackend {
    /// Get the current terminal size in character cells
    ///
    /// # Returns
    /// A tuple of (width, height) in characters
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if querying size fails
    fn size(&self) -> Result<(u16, u16), DotmaxError>;

    /// Render content to the terminal
    ///
    /// # Arguments
    /// * `content` - The content to render (typically braille characters)
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if rendering fails
    fn render(&mut self, content: &str) -> Result<(), DotmaxError>;

    /// Clear the terminal display
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if clearing fails
    fn clear(&mut self) -> Result<(), DotmaxError>;

    /// Get terminal capabilities
    ///
    /// # Returns
    /// Information about terminal features (color, unicode support)
    fn capabilities(&self) -> TerminalCapabilities;
}

// ============================================================================
// Terminal Renderer - Main Implementation
// ============================================================================

/// Terminal renderer for braille grids
///
/// Manages terminal state and renders `BrailleGrid` to the terminal display.
/// Uses ratatui and crossterm for cross-platform terminal manipulation.
///
/// Extracted and adapted from crabmusic/src/rendering/mod.rs:56-377
///
/// # Examples
///
/// ```no_run
/// use dotmax::{BrailleGrid, TerminalRenderer};
///
/// let mut renderer = TerminalRenderer::new().expect("Failed to initialize terminal");
/// let grid = BrailleGrid::new(80, 24).expect("Failed to create grid");
/// renderer.render(&grid).expect("Failed to render");
/// renderer.cleanup().expect("Failed to cleanup terminal");
/// ```
pub struct TerminalRenderer {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    #[allow(dead_code)] // Reserved for future resize detection (Story 2.5)
    last_size: (u16, u16),
    /// Detected terminal type for viewport handling
    terminal_type: TerminalType,
    /// Whether this is the first render (clear needed) or subsequent (skip clear for performance)
    first_render: bool,
}

impl TerminalRenderer {
    /// Initialize a new terminal renderer
    ///
    /// Sets up the terminal in raw mode and alternate screen.
    /// Extracted from crabmusic/src/rendering/mod.rs:81-117
    ///
    /// # Returns
    /// A new `TerminalRenderer` instance
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if terminal setup fails
    /// Returns `DotmaxError::TerminalBackend` if terminal is too small (min 40×12)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::TerminalRenderer;
    ///
    /// let renderer = TerminalRenderer::new().expect("Failed to initialize terminal");
    /// ```
    #[instrument]
    pub fn new() -> Result<Self, DotmaxError> {
        let mut stdout = io::stdout();

        // Check terminal size (minimum 40×12 for basic functionality)
        // Extracted from crabmusic/src/rendering/mod.rs:85-93
        let (width, height) = crossterm::terminal::size()?;

        debug!(width = width, height = height, "Detected terminal size");

        if width < 40 || height < 12 {
            error!(
                width = width,
                height = height,
                min_width = 40,
                min_height = 12,
                "Terminal too small: {}×{} (minimum 40×12 required)",
                width,
                height
            );
            return Err(DotmaxError::TerminalBackend(format!(
                "Terminal too small: {width}×{height} (minimum 40×12 required)"
            )));
        }

        // Enter raw mode
        // Extracted from crabmusic/src/rendering/mod.rs:95-96
        enable_raw_mode()?;

        // Enter alternate screen
        // Extracted from crabmusic/src/rendering/mod.rs:98-99
        execute!(stdout, EnterAlternateScreen)?;

        // Story 2.8: Fix cursor position after entering alternate screen
        // In WSL/Windows Terminal, the cursor may not start at (0,0)
        // Explicitly clear screen and move cursor to top-left
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

        // Set up panic handler to restore terminal
        // Extracted from crabmusic/src/rendering/mod.rs:101-106
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = Self::restore_terminal();
            original_hook(panic_info);
        }));

        // Create Ratatui terminal
        // Extracted from crabmusic/src/rendering/mod.rs:108-110
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Detect terminal type for viewport handling (Story 2.8)
        let terminal_type = TerminalType::detect();
        info!(
            width = width,
            height = height,
            terminal_type = terminal_type.name(),
            "Terminal renderer initialized successfully with terminal type detection"
        );

        Ok(Self {
            terminal,
            last_size: (width, height),
            terminal_type,
            first_render: true,
        })
    }

    /// Render a braille grid to the terminal
    ///
    /// Uses Ratatui's Frame API to efficiently render the grid.
    /// Ratatui handles differential rendering automatically.
    ///
    /// Extracted and adapted from crabmusic/src/rendering/mod.rs:202-254
    /// Adapted to use `BrailleGrid::to_unicode_grid()` from Story 2.2
    ///
    /// # Arguments
    /// * `grid` - The braille grid to render
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if rendering fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::{BrailleGrid, TerminalRenderer};
    ///
    /// let mut renderer = TerminalRenderer::new().expect("Failed to initialize");
    /// let grid = BrailleGrid::new(80, 24).expect("Failed to create grid");
    /// renderer.render(&grid).expect("Failed to render");
    /// ```
    #[instrument(skip(self, grid))]
    pub fn render(&mut self, grid: &BrailleGrid) -> Result<(), DotmaxError> {
        let (grid_width, grid_height) = grid.dimensions();
        debug!(
            grid_width = grid_width,
            grid_height = grid_height,
            total_cells = grid_width * grid_height,
            "Rendering BrailleGrid to terminal"
        );

        // ISSUE #2 FIX: Clear the terminal buffer on FIRST render only to ensure
        // ratatui's differential rendering has a clean baseline.
        // Skip clear on subsequent renders to avoid flashing during video playback.
        if self.first_render {
            self.terminal.clear()?;
            self.first_render = false;
        }

        // Convert grid to Unicode characters using Story 2.2 functionality
        let unicode_grid = grid.to_unicode_grid();

        self.terminal.draw(|frame| {
            let area = frame.area();

            // DEBUG: Log the actual rendering area
            debug!(
                area_width = area.width,
                area_height = area.height,
                terminal_type = self.terminal_type.name(),
                grid_height = grid.height(),
                "Rendering area vs grid size"
            );

            // CRITICAL BUG FIX: Grid may be larger than the rendering area
            // We need to render ONLY the lines that fit, starting from the BEGINNING
            let max_lines = area.height as usize;

            if unicode_grid.len() > max_lines {
                debug!(
                    grid_lines = unicode_grid.len(),
                    area_lines = max_lines,
                    overflow = unicode_grid.len() - max_lines,
                    "WARNING: Grid is larger than rendering area, truncating to fit"
                );
            }

            // Convert Unicode grid to Ratatui Lines with colors (Story 2.6)
            // Extracted from crabmusic/src/rendering/mod.rs:207-246
            // Enhanced to support per-cell colors
            // ONLY RENDER LINES THAT FIT IN THE AREA
            let lines: Vec<Line> = unicode_grid
                .iter()
                .take(max_lines) // Critical: only render what fits
                .enumerate()
                .map(|(y, row)| {
                    let spans: Vec<Span> = row
                        .iter()
                        .enumerate()
                        .map(|(x, &ch)| {
                            // Check if cell has color assigned and apply color if present
                            grid.get_color(x, y).map_or_else(
                                || Span::raw(ch.to_string()),
                                |color| {
                                    Span::styled(
                                        ch.to_string(),
                                        ratatui::style::Style::default().fg(
                                            ratatui::style::Color::Rgb(color.r, color.g, color.b),
                                        ),
                                    )
                                },
                            )
                        })
                        .collect();
                    Line::from(spans)
                })
                .collect();

            // Create paragraph widget
            // CRITICAL: Ensure paragraph starts from the TOP (scroll = 0)
            let paragraph = Paragraph::new(lines).scroll((0, 0)); // Explicitly set scroll to (0, 0) to start from top

            // Render to frame
            frame.render_widget(paragraph, area);
        })?;

        Ok(())
    }

    /// Clear the terminal display
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if clearing fails
    pub fn clear(&mut self) -> Result<(), DotmaxError> {
        self.terminal.clear()?;
        Ok(())
    }

    /// Get the current terminal dimensions
    ///
    /// Extracted from crabmusic/src/rendering/mod.rs:282-285
    ///
    /// # Returns
    /// A tuple of (width, height) in characters
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if querying terminal size fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dotmax::TerminalRenderer;
    ///
    /// let renderer = TerminalRenderer::new().expect("Failed to initialize terminal");
    /// let (width, height) = renderer.get_terminal_size().unwrap();
    /// assert!(width >= 40);
    /// assert!(height >= 12);
    /// ```
    #[instrument(skip(self))]
    pub fn get_terminal_size(&self) -> Result<(u16, u16), DotmaxError> {
        let size = self.terminal.size()?;

        // Story 2.8: Return the viewport size (not buffer size)
        // The offset is applied during rendering in render(), not here
        // This ensures that grid sizing matches the actual visible viewport
        let offset = self.terminal_type.viewport_height_offset(size.height);
        let viewport_height = size.height.saturating_sub(offset);

        debug!(
            terminal_type = self.terminal_type.name(),
            buffer_width = size.width,
            buffer_height = size.height,
            viewport_width = size.width,
            viewport_height = viewport_height,
            offset = offset,
            "Terminal size query (returning viewport dimensions for grid sizing)"
        );

        Ok((size.width, viewport_height))
    }

    /// Clean up and restore terminal state
    ///
    /// Should be called before the application exits to restore the terminal
    /// to its original state.
    ///
    /// Extracted from crabmusic/src/rendering/mod.rs:263-265
    ///
    /// # Errors
    /// Returns `DotmaxError::Terminal` if cleanup fails
    pub fn cleanup(&mut self) -> Result<(), DotmaxError> {
        Self::restore_terminal()
    }

    /// Restore terminal to original state (static for panic handler)
    ///
    /// Extracted from crabmusic/src/rendering/mod.rs:358-369
    fn restore_terminal() -> Result<(), DotmaxError> {
        let mut stdout = io::stdout();

        // Leave alternate screen
        execute!(stdout, LeaveAlternateScreen)?;

        // Disable raw mode
        disable_raw_mode()?;

        Ok(())
    }

    /// Get terminal capabilities
    ///
    /// Returns information about terminal features including detected terminal type.
    ///
    /// # Returns
    /// Terminal capabilities information
    #[must_use]
    pub fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities {
            terminal_type: self.terminal_type,
            ..TerminalCapabilities::default()
        }
    }
}

impl Drop for TerminalRenderer {
    /// Ensure terminal is cleaned up even if `cleanup()` wasn't called
    ///
    /// Extracted from crabmusic/src/rendering/mod.rs:372-377
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Story 2.8: Terminal Type Detection Tests
    // ============================================================================

    #[test]
    fn test_terminal_type_viewport_offset_windows_terminal() {
        let term_type = TerminalType::WindowsTerminal;

        // Windows Terminal reports viewport correctly - no offset needed
        assert_eq!(term_type.viewport_height_offset(10), 0);
        assert_eq!(term_type.viewport_height_offset(24), 0);
        assert_eq!(term_type.viewport_height_offset(50), 0);
        assert_eq!(term_type.viewport_height_offset(100), 0);
    }

    #[test]
    fn test_terminal_type_viewport_offset_wsl() {
        let term_type = TerminalType::Wsl;

        // WSL applies 12-row offset (empirically tested)
        assert_eq!(
            term_type.viewport_height_offset(10),
            0,
            "Small terminal no offset"
        );
        assert_eq!(
            term_type.viewport_height_offset(20),
            0,
            "Exactly 20 rows no offset"
        );
        assert_eq!(
            term_type.viewport_height_offset(21),
            12,
            "21+ rows get 12 offset"
        );
        assert_eq!(
            term_type.viewport_height_offset(53),
            12,
            "Standard WSL terminal"
        );
        assert_eq!(term_type.viewport_height_offset(100), 12);
    }

    #[test]
    fn test_terminal_type_viewport_offset_windows_console() {
        let term_type = TerminalType::WindowsConsole;

        // Windows Console - applies 12-row offset (same as WSL)
        assert_eq!(
            term_type.viewport_height_offset(10),
            0,
            "Small terminal no offset"
        );
        assert_eq!(
            term_type.viewport_height_offset(20),
            0,
            "Exactly 20 rows no offset"
        );
        assert_eq!(
            term_type.viewport_height_offset(21),
            12,
            "21+ rows get 12 offset"
        );
        assert_eq!(
            term_type.viewport_height_offset(74),
            12,
            "PowerShell terminal"
        );
        assert_eq!(term_type.viewport_height_offset(100), 12);
    }

    #[test]
    fn test_terminal_type_viewport_offset_macos() {
        let term_type = TerminalType::MacOsTerminal;

        // macOS Terminal reports viewport correctly - no offset needed
        assert_eq!(term_type.viewport_height_offset(10), 0);
        assert_eq!(term_type.viewport_height_offset(24), 0);
        assert_eq!(term_type.viewport_height_offset(50), 0);
    }

    #[test]
    fn test_terminal_type_viewport_offset_linux_native() {
        let term_type = TerminalType::LinuxNative;

        // Linux native terminals report viewport correctly - no offset needed
        assert_eq!(term_type.viewport_height_offset(10), 0);
        assert_eq!(term_type.viewport_height_offset(24), 0);
        assert_eq!(term_type.viewport_height_offset(50), 0);
    }

    #[test]
    fn test_terminal_type_viewport_offset_unknown() {
        let term_type = TerminalType::Unknown;

        // Unknown terminals use conservative approach - no offset
        assert_eq!(term_type.viewport_height_offset(10), 0);
        assert_eq!(term_type.viewport_height_offset(24), 0);
        assert_eq!(term_type.viewport_height_offset(50), 0);
    }

    #[test]
    fn test_terminal_type_name() {
        assert_eq!(TerminalType::WindowsTerminal.name(), "Windows Terminal");
        assert_eq!(TerminalType::Wsl.name(), "WSL");
        assert_eq!(
            TerminalType::WindowsConsole.name(),
            "Windows Console (PowerShell/cmd)"
        );
        assert_eq!(TerminalType::MacOsTerminal.name(), "macOS Terminal.app");
        assert_eq!(TerminalType::LinuxNative.name(), "Linux Native Terminal");
        assert_eq!(TerminalType::Unknown.name(), "Unknown Terminal");
    }

    #[test]
    fn test_terminal_type_edge_cases() {
        // Test edge cases for offset calculation
        let wsl = TerminalType::Wsl;
        let windows_console = TerminalType::WindowsConsole;

        // Boundary testing
        assert_eq!(
            wsl.viewport_height_offset(0),
            0,
            "Zero height should not panic"
        );
        assert_eq!(
            wsl.viewport_height_offset(1),
            0,
            "Single row should have no offset"
        );

        assert_eq!(windows_console.viewport_height_offset(0), 0);
        assert_eq!(windows_console.viewport_height_offset(1), 0);

        // Maximum values
        assert_eq!(
            wsl.viewport_height_offset(u16::MAX),
            12,
            "WSL gets 12 offset for large terminals"
        );
        assert_eq!(
            windows_console.viewport_height_offset(u16::MAX),
            12,
            "Windows Console gets 12 offset"
        );
    }

    #[test]
    fn test_terminal_type_saturating_sub() {
        // Verify that offset doesn't cause underflow
        let wsl = TerminalType::Wsl;
        let offset = wsl.viewport_height_offset(25);
        let adjusted = 25u16.saturating_sub(offset);
        assert_eq!(adjusted, 13, "25 - 12 = 13");

        // Test that saturating_sub prevents underflow
        let small_height = 1u16;
        let offset = wsl.viewport_height_offset(small_height);
        let adjusted = small_height.saturating_sub(offset);
        assert_eq!(adjusted, 1, "1 - 0 = 1 (no offset for small terminal)");
    }

    #[test]
    fn test_terminal_capabilities_default() {
        let caps = TerminalCapabilities::default();
        assert!(caps.supports_color);
        assert!(caps.supports_truecolor);
        assert!(caps.supports_unicode);

        // Story 2.8: Verify terminal type is detected
        // The actual type will depend on the test environment
        // Just verify it's one of the valid types
        match caps.terminal_type {
            TerminalType::WindowsTerminal
            | TerminalType::Wsl
            | TerminalType::WindowsConsole
            | TerminalType::MacOsTerminal
            | TerminalType::LinuxNative
            | TerminalType::Unknown => {
                // Valid terminal type detected
            }
        }
    }

    #[test]
    fn test_terminal_capabilities_includes_terminal_type() {
        let caps = TerminalCapabilities::default();

        // Verify terminal_type field exists and is accessible
        let _ = caps.terminal_type;
        let _ = caps.terminal_type.name();
    }

    /// Helper macro to skip tests that require a terminal when running in CI/test harness
    /// Returns early if no terminal is available (e.g., when stdout is captured)
    macro_rules! require_terminal {
        () => {
            match TerminalRenderer::new() {
                Ok(r) => r,
                Err(_) => {
                    // No terminal available (e.g., running in test harness with captured stdout)
                    // Skip this test gracefully
                    return;
                }
            }
        };
    }

    #[test]
    fn test_terminal_renderer_creation() {
        let _renderer = require_terminal!();
        // If we get here, terminal creation succeeded
    }

    #[test]
    fn test_terminal_dimensions() {
        let renderer = require_terminal!();
        let (width, height) = renderer.get_terminal_size().expect("Failed to get size");
        assert!(width >= 40, "Terminal width should be at least 40");
        assert!(height >= 12, "Terminal height should be at least 12");
    }

    #[test]
    fn test_terminal_cleanup() {
        let mut renderer = require_terminal!();
        let result = renderer.cleanup();
        assert!(result.is_ok(), "Cleanup should succeed: {:?}", result.err());
    }

    #[test]
    fn test_render_braille_grid() {
        let mut renderer = require_terminal!();
        let mut grid = BrailleGrid::new(10, 10).expect("Failed to create grid");

        // Set a test pattern
        grid.set_dot(5, 5).expect("Failed to set dot");
        grid.set_dot(6, 6).expect("Failed to set dot");

        // Render should succeed
        let result = renderer.render(&grid);
        assert!(result.is_ok(), "Render should succeed: {:?}", result.err());
    }

    #[test]
    fn test_clear_terminal() {
        let mut renderer = require_terminal!();
        let result = renderer.clear();
        assert!(result.is_ok(), "Clear should succeed: {:?}", result.err());
    }

    #[test]
    fn test_get_capabilities() {
        let renderer = require_terminal!();
        let caps = renderer.capabilities();
        // Should return default capabilities
        assert!(caps.supports_unicode);
    }
}
