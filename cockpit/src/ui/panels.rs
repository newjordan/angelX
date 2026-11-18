//! Per-panel frame rects for the disconnected cockpit layout (Phase A).
//!
//! Phase A breaks the previously-contiguous cockpit into free-floating,
//! individually-bordered panels separated by visible gaps (the backdrop shows
//! through the gaps). Every draw, each panel's **outer** rect — the box
//! *including* its border, in absolute buffer coordinates — is published into a
//! [`PanelFrames`] registry on `App`.
//!
//! This is the hook Phase B needs: a per-panel steel-skin art frame + scene
//! background can be attached to each live rect by iterating this registry (the
//! frame wraps the outer rect; the scene bg fills it). It is the floating-panel
//! analogue of [`crate::ui::mouse::PaneRegistry`], which tracks the chrome-free
//! *content* rects for mouse hit-testing — here we track the framed boxes.
//!
//! Pure and headless-testable: it is just a tagged list of rects rebuilt each
//! frame; the layout math that fills it lives in `main.rs::ui`.

use ratatui::layout::Rect;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PanelKind {
    Header,
    LoopTicker,
    Transcript,
    Shell,
    Image,
    AgentBay,
    Artifacts,
    Input,
    Footer,
}

impl PanelKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::LoopTicker => "loop",
            Self::Transcript => "transcript",
            Self::Shell => "shell",
            Self::Image => "image",
            Self::AgentBay => "agent",
            Self::Artifacts => "artifacts",
            Self::Input => "input",
            Self::Footer => "footer",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PanelFrames {
    frames: Vec<(PanelKind, Rect)>,
}

impl PanelFrames {
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    pub fn push(&mut self, kind: PanelKind, rect: Rect) {
        if rect.width > 0 && rect.height > 0 {
            self.frames.push((kind, rect));
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (PanelKind, Rect)> + '_ {
        self.frames.iter().copied()
    }

    pub fn get(&self, kind: PanelKind) -> Option<Rect> {
        self.frames
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, r)| *r)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/panels__tests.rs"]
mod tests;
