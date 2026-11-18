//! Which modal sits in front of the cockpit.
//!
//! The modals are separate fields on `App` (`pending_approval`, `setup`,
//! `browser_picker`, `loop_dialog`, `agent_menu`, `moa_deck`), meant to be
//! used one at a time, with nothing stored that says which one is on top.
//! `App::top_overlay` derives it in one precedence order, and every input
//! channel (keys, paste, pointer), the compositor and `ui_inspect` ask it
//! instead of keeping a ladder of their own. Nothing here is stored or sent.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Overlay {
    None,
    Approval,
    Setup,
    BrowserPicker,
    LoopDialog,
    AgentMenu,
    MoaDeck,
}

impl Overlay {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Approval => "approval",
            Self::Setup => "setup",
            Self::BrowserPicker => "browser_picker",
            Self::LoopDialog => "loop_dialog",
            Self::AgentMenu => "agent_menu",
            Self::MoaDeck => "moa_deck",
        }
    }

    pub(crate) fn takes_pointer(self) -> bool {
        !matches!(self, Self::None | Self::MoaDeck)
    }
}

impl App {
    pub(crate) fn top_overlay(&self) -> Overlay {
        if self.pending_approval.is_some() {
            Overlay::Approval
        } else if self.setup.open() {
            Overlay::Setup
        } else if self.browser_picker.is_some() {
            Overlay::BrowserPicker
        } else if self.loop_dialog.is_some() {
            Overlay::LoopDialog
        } else if self.agent_menu.is_some() {
            Overlay::AgentMenu
        } else if self.moa_deck_owns_input() {
            Overlay::MoaDeck
        } else {
            Overlay::None
        }
    }
}
