use ratatui_core::style::Color;

// Camelot-noir palette anchors.
pub const HUD_BLUE: Color = Color::Rgb(91, 190, 255);
pub const HUD_DIM: Color = Color::Rgb(58, 111, 151);
pub const HUD_TEXT: Color = Color::Rgb(184, 228, 255);
/// Hot CRT-phosphor cyan-blue: the agent's live thinking glow.
pub const HUD_PHOSPHOR: Color = Color::Rgb(86, 232, 255);
/// Warning coral: faults and destructive states only.
pub const HUD_DANGER: Color = Color::Rgb(255, 103, 126);
/// Alchemical green: verified success and recovered state only.
pub const HUD_VERIFIED: Color = Color::Rgb(99, 241, 169);
/// Regal purple: the operator's speaker label in the primary conversation pane.
pub const HUD_PURPLE: Color = Color::Rgb(200, 130, 255);
pub const HUD_GOLD: Color = Color::Rgb(255, 207, 92);
