//! The miniworld and its ceremonies — the gamified systems view.

pub(crate) mod knight_journey;
pub(crate) mod questmap;
pub(crate) mod raytrace;

#[derive(Clone, Copy)]
pub(crate) struct WorldDrag {
    pub(crate) last: (u16, u16),
    pub(crate) rect: ratatui::layout::Rect,
    pub(crate) surface: crate::ui::scryglass::StageSurface,
}
