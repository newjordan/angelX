//! The two HUD colours the world pane's chrome wears: candlelight gold for an
//! accepted banner, caution amber for deep fog. The cockpit's `ui::hud`
//! re-exports them at their old path beside the rest of its palette.

use ratatui_core::style::Color;

pub const HUD_GOLD: Color = Color::Rgb(255, 207, 92);
pub const HUD_AMBER: Color = Color::Rgb(255, 204, 63);
