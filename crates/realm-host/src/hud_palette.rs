//! The two HUD colours the world pane's chrome wears: candlelight gold for an
//! accepted banner, caution amber for deep fog. The cockpit's `ui::hud`
//! re-exports them at their old path beside the rest of its palette.

use ratatui_core::style::Color;

/// Candlelight gold: the Camelot-noir accent. One flame against the HUD
/// blues — heading sigils, inline code, the knight's tag. Use sparingly.
pub const HUD_GOLD: Color = Color::Rgb(255, 207, 92);
/// Warning amber: caution, never blame. The quest HUD's danger ladder and
/// the world pane's danger border ride it — it is not a failure colour.
pub const HUD_AMBER: Color = Color::Rgb(255, 204, 63);
