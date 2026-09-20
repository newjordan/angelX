//! Drag ownership for the Dotmax camera; navigation stays in World.
#[derive(Clone, Copy)]
pub(crate) struct WorldDrag {
    pub(crate) last: (u16, u16),
    pub(crate) rect: ratatui::layout::Rect,
    pub(crate) surface: crate::scryglass::StageSurface,
}
