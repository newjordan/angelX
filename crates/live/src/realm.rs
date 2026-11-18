//! The contract between the cockpit and the realm host (the Delve and its
//! world): what the cockpit asks of the game, and what the game asks of the
//! cockpit it runs inside.
//!
//! Two traits and a few plain types, and nothing of either side: a leaf of the
//! contract crate, so an edit to the game and an edit to the cockpit stop
//! rebuilding each other. The cockpit drives the game through [`RealmHost`]
//! (keys, clicks, commands, the cadence of its frames) and lends it
//! [`HostCx`] for each call: the facts the game reads of the cockpit's state
//! ([`Facts`]), the few things it may change there (the composer, the
//! transcript) and the larger moves it may ask for ([`Move`]), which the
//! cockpit makes once the call is over, so the game never re-enters the
//! cockpit's own state while it holds the game's.

use crossterm::event::KeyEvent;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub trait Oracle: Send + Sync {
    fn respond(&self, prompt: &str, at_least: Duration) -> Result<String, String>;

    fn label(&self) -> &str;

    fn warm_up(&self) {}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub composer_empty: bool,
    pub terminal_focused: bool,
    pub shell_focused: bool,
    pub tutor_draft: bool,
    pub deck_open: bool,
    pub deck_owns_input: bool,
    pub approval_pending: bool,
    pub loop_dialog: bool,
    pub agent_menu: bool,
    pub turn_running: bool,
    pub turn_queued: bool,
    pub background_job: bool,
    pub artifacts_focused: bool,
    pub world_map_shown: bool,
    pub world_pane_visible: bool,
}

impl Facts {
    pub fn typing_elsewhere(&self) -> bool {
        self.shell_focused || self.tutor_draft || self.deck_owns_input
    }

    pub fn dialog_open(&self) -> bool {
        self.approval_pending || self.loop_dialog || self.agent_menu
    }

    pub fn busy(&self) -> bool {
        self.turn_running || self.turn_queued
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    Focus(&'static str),
    ShowWorld,
    Open(Panel),
    Submit { brief: String, keep_draft: bool },
    DismissIntro,
    ResetWorldYaw,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Panel {
    Formation,
    Library,
    Tower,
    ModelMenu(u8),
}

pub trait HostCx {
    fn facts(&self) -> Facts;

    fn workspace(&self) -> PathBuf;

    fn oracle(&self) -> Arc<dyn Oracle>;

    fn say(&mut self, text: String);

    fn redraw(&mut self);

    fn take_composer(&mut self) -> (String, usize);

    fn set_composer(&mut self, text: String, cursor: usize);

    fn make(&mut self, ask: Move);
}

pub trait RealmHost {
    fn key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool;

    fn walk_key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool;

    fn release_key(&mut self, key: KeyEvent);

    fn walk_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool;

    fn gate_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool;

    fn clear_controls(&mut self, cx: &mut dyn HostCx);

    fn collapse(&mut self, cx: &mut dyn HostCx);

    fn open_intro(&mut self, cx: &mut dyn HostCx);

    fn command(&mut self, cx: &mut dyn HostCx, arg: Option<&str>) -> String;

    fn walk_from_command(&mut self, cx: &mut dyn HostCx) -> String;

    fn visit(&mut self, cx: &mut dyn HostCx, place: Visit) -> String;

    fn stop_walk(&mut self);

    fn go_in(&mut self, cx: &mut dyn HostCx);

    fn tick(&mut self, cx: &mut dyn HostCx);

    fn steer_walk(&mut self, cx: &mut dyn HostCx);

    fn settle_walk(&mut self, cx: &mut dyn HostCx);

    fn view_active(&self, facts: &Facts) -> bool;

    fn controls_blocked(&self, facts: &Facts) -> bool;

    fn keyboard_active(&self, facts: &Facts) -> bool;

    fn wants_fast_tick(&self, facts: &Facts) -> bool;

    fn step_due(&self, facts: &Facts) -> Option<Instant>;

    fn walk_map_shown(&self, facts: &Facts) -> bool;

    fn note_call(&mut self, id: &str, name: &str, args: &str);

    fn note_result(&mut self, id: &str, clean: bool);

    fn note_loop(&mut self, iteration: usize);

    fn note_turn(&mut self, cache: u64, fresh: u64, secs: u64);

    fn note_end(&mut self, cx: &mut dyn HostCx, ok: bool);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visit {
    Stables,
    Lists,
}
