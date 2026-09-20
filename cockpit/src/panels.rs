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
//! analogue of [`crate::mouse::PaneRegistry`], which tracks the chrome-free
//! *content* rects for mouse hit-testing — here we track the framed boxes.
//!
//! Pure and headless-testable: it is just a tagged list of rects rebuilt each
//! frame; the layout math that fills it lives in `main.rs::ui`.

use ratatui::layout::Rect;

/// The top-level floating panels of the cockpit. Each is its own bordered box
/// separated from its neighbors by gap cells. Not every kind is present every
/// frame — `Shell`/`Image` replace the cockpit body only when active, and
/// `Transcript`/`AgentBay`/`Artifacts` collapse to a single `Transcript` box on
/// a narrow terminal.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // layout catalog — Footer is reserved for the transport deck hint
pub enum PanelKind {
    /// Top status strip (in-hand box, mode, CPU/GPU, pet).
    Header,
    /// Terminal / chat transcript.
    Transcript,
    /// Embedded full-access shell (when focused).
    Shell,
    /// Explicit foreground image viewer (portraits and compatibility paths).
    Image,
    /// Agent-status / portrait bay.
    AgentBay,
    /// Canonical Scryglass / Realm Stage.
    Artifacts,
    /// Command / message input (also the working-metrics strip).
    Input,
    /// Bottom hint / key-legend strip.
    Footer,
}

impl PanelKind {
    /// Stable lowercase tag, suitable as a scene/skin layer key (Phase B can
    /// publish e.g. `skin.<name>` / `scene.<name>` per panel).
    // Phase B reads this; the Phase A bin only fills the registry.
    #[allow(dead_code)]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Header => "header",
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

/// The outer (framed) rects of every floating panel drawn this frame, in
/// absolute buffer coordinates. Rebuilt from scratch each draw in `ui()`.
#[derive(Clone, Debug, Default)]
pub struct PanelFrames {
    frames: Vec<(PanelKind, Rect)>,
}

impl PanelFrames {
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Register a panel's outer box rect (empty rects are ignored, so a panel
    /// that collapsed to nothing on a tiny terminal simply isn't published).
    pub fn push(&mut self, kind: PanelKind, rect: Rect) {
        if rect.width > 0 && rect.height > 0 {
            self.frames.push((kind, rect));
        }
    }

    // The accessors below are the Phase B read surface (iterate the live boxes to
    // attach a skin frame + scene bg). The Phase A bin only writes the registry,
    // so they read as dead code here; the tests exercise them.

    /// Iterate the published `(kind, outer_rect)` pairs in draw order.
    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = (PanelKind, Rect)> + '_ {
        self.frames.iter().copied()
    }

    /// The outer rect of a specific panel, if it was drawn this frame.
    #[allow(dead_code)]
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
mod tests {
    use super::*;

    #[test]
    fn empty_rects_are_not_published() {
        let mut frames = PanelFrames::default();
        frames.push(PanelKind::Header, Rect::new(0, 0, 0, 3));
        frames.push(PanelKind::Footer, Rect::new(0, 0, 10, 0));
        assert!(frames.is_empty(), "zero-area panels must be dropped");
    }

    #[test]
    fn get_returns_the_last_drawn_rect() {
        let mut frames = PanelFrames::default();
        let r = Rect::new(2, 1, 40, 12);
        frames.push(PanelKind::Transcript, r);
        assert_eq!(frames.get(PanelKind::Transcript), Some(r));
        assert_eq!(frames.get(PanelKind::Shell), None);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn names_are_stable_lowercase_tags() {
        assert_eq!(PanelKind::AgentBay.name(), "agent");
        assert_eq!(PanelKind::Transcript.name(), "transcript");
        assert_eq!(PanelKind::Input.name(), "input");
    }
}
