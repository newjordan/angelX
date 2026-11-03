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

/// A model the realm asks, one reply to one question, no tools and no
/// conversation (the cockpit's club in hand).
pub trait Oracle: Send + Sync {
    /// One reply to `prompt`. The call may run for as long as `at_least`
    /// (a longer bound the operator set stands): a whole phrasebook entry in
    /// one reply outlasts the clubs' general bound.
    fn respond(&self, prompt: &str, at_least: Duration) -> Result<String, String>;

    /// Short label of the model asked, for the logs.
    fn label(&self) -> &str;

    /// Warm the model's address and connection ahead of its first question.
    fn warm_up(&self) {}
}

/// What the game reads of the cockpit's state, taken as the call begins (and
/// kept current by the cockpit's context as the game changes the composer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The composer holds no text.
    pub composer_empty: bool,
    /// The terminal window has the user's focus.
    pub terminal_focused: bool,
    /// A shell pane has the keyboard.
    pub shell_focused: bool,
    /// A tutor question's draft is open.
    pub tutor_draft: bool,
    /// The Brain Route deck is up (it owns the keys while it is).
    pub deck_open: bool,
    /// The deck is up and takes keys the game would take.
    pub deck_owns_input: bool,
    /// The harness waits on an approval.
    pub approval_pending: bool,
    /// The loop launcher's dialog is up.
    pub loop_dialog: bool,
    /// An agent / model menu is up.
    pub agent_menu: bool,
    /// A turn is running.
    pub turn_running: bool,
    /// A turn is queued behind the one running.
    pub turn_queued: bool,
    /// A background job holds the flight slot.
    pub background_job: bool,
    /// The artifacts module has the keyboard focus.
    pub artifacts_focused: bool,
    /// The last draw showed the overworld map itself on the Realm pane: not a
    /// room, an asset, the 3D ride or a quest.
    pub world_map_shown: bool,
    /// The last draw laid the world pane out on screen.
    pub world_pane_visible: bool,
}

impl Facts {
    /// Another surface owns the keyboard: a shell, a tutor question, the deck.
    pub fn typing_elsewhere(&self) -> bool {
        self.shell_focused || self.tutor_draft || self.deck_owns_input
    }

    /// A dialog is up over the game.
    pub fn dialog_open(&self) -> bool {
        self.approval_pending || self.loop_dialog || self.agent_menu
    }

    /// A turn is running or queued: the harness is busy.
    pub fn busy(&self) -> bool {
        self.turn_running || self.turn_queued
    }
}

/// A larger move the game asks of the cockpit. Made after the game's call
/// returns, in the order asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// Put this module in focus (`"artifacts"` for the stage, `"core"` for
    /// the transcript).
    Focus(&'static str),
    /// Show the world on the stage: the artifacts pane back on the overworld,
    /// no selection or drag held over it.
    ShowWorld,
    /// Open a panel of the cockpit's.
    Open(Panel),
    /// Submit this brief to the harness as a turn, as if typed: the brief
    /// takes the composer, goes, and (`keep_draft`) the composer's draft
    /// comes back after it.
    Submit { brief: String, keep_draft: bool },
    /// The game took the screen: the startup intro is over.
    DismissIntro,
    /// Face the world camera front again after a room change.
    ResetWorldYaw,
}

/// A panel of the cockpit the game can open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Panel {
    /// The Brain Route formation deck.
    Formation,
    /// The Scryglass library.
    Library,
    /// The Scrying Tower.
    Tower,
    /// The model menu, filtered to one house of models (the house's number on the
    /// map); when a turn is in flight the cockpit says it opens afterwards.
    ModelMenu(u8),
}

/// What the cockpit lends the realm host for the length of one call.
pub trait HostCx {
    /// The facts, as they stand now.
    fn facts(&self) -> Facts;

    /// The workspace the game plays in.
    fn workspace(&self) -> PathBuf;

    /// The model the cockpit has in hand, for wishes.
    fn oracle(&self) -> Arc<dyn Oracle>;

    /// Say a line in the transcript, as the system.
    fn say(&mut self, text: String);

    /// Ask for a redraw of the whole screen.
    fn redraw(&mut self);

    /// Take the composer's text and cursor, leaving it empty.
    fn take_composer(&mut self) -> (String, usize);

    /// Put text and a cursor in the composer.
    fn set_composer(&mut self, text: String, cursor: usize);

    /// Ask for a larger move, made after the call.
    fn make(&mut self, ask: Move);
}

/// What the cockpit asks of the realm host.
///
/// Every method that may change the cockpit takes the context the call runs
/// in; the plain questions take the [`Facts`] the cockpit holds.
pub trait RealmHost {
    // ---- the keyboard and the pointer

    /// A key while the game has the screen. True when the game took it.
    fn key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool;

    /// A key while the visitor walks the map (or `p`, which starts him
    /// walking). True when walk mode took it.
    fn walk_key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool;

    /// A key came up.
    fn release_key(&mut self, key: KeyEvent);

    /// While the visitor walks, a click chooses a road destination: true when
    /// it was one.
    fn walk_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool;

    /// A click on the Delve's gate in the map: true when it was one.
    fn gate_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool;

    /// Let go of every control the game holds (focus lost, a dialog opened).
    fn clear_controls(&mut self, cx: &mut dyn HostCx);

    /// Give the screen back to the cockpit.
    fn collapse(&mut self, cx: &mut dyn HostCx);

    /// Open the Delve's menu.
    fn open_intro(&mut self, cx: &mut dyn HostCx);

    // ---- the commands

    /// `/dungeon ...`: the reply to say.
    fn command(&mut self, cx: &mut dyn HostCx, arg: Option<&str>) -> String;

    /// `/world walk`: the reply to say.
    fn walk_from_command(&mut self, cx: &mut dyn HostCx) -> String;

    /// `/world visit stables|lists`: the reply to say.
    fn visit(&mut self, cx: &mut dyn HostCx, place: Visit) -> String;

    /// Step off the map.
    fn stop_walk(&mut self);

    /// Go in at the door at hand on the map.
    fn go_in(&mut self, cx: &mut dyn HostCx);

    // ---- the cadence

    /// The event loop's step: advance the guests, watch the gate and the
    /// reforges, tune the audio, advance a joined seat.
    fn tick(&mut self, cx: &mut dyn HostCx);

    /// Hand the held walk keys to the visitor before the world ticks.
    fn steer_walk(&mut self, cx: &mut dyn HostCx);

    /// Walk mode lasts while its map is on screen and in focus.
    fn settle_walk(&mut self, cx: &mut dyn HostCx);

    // ---- the questions

    /// The game has the screen and its keys.
    fn view_active(&self, facts: &Facts) -> bool;

    /// A dialog or an exhibit blocks the game's controls.
    fn controls_blocked(&self, facts: &Facts) -> bool;

    /// The terminal reports keys as a game's (presses and releases).
    fn keyboard_active(&self, facts: &Facts) -> bool;

    /// The game wants the fast cadence.
    fn wants_fast_tick(&self, facts: &Facts) -> bool;

    /// When the delve's next fixed step is due, while it is stepped on the
    /// cockpit's thread.
    fn step_due(&self, facts: &Facts) -> Option<Instant>;

    /// The last draw showed the overworld map itself on the Realm pane.
    fn walk_map_shown(&self, facts: &Facts) -> bool;

    // ---- the harness's work, counted in the holdings

    /// A tool call began.
    fn note_call(&mut self, id: &str, name: &str, args: &str);

    /// A tool call came back, and whether it was clean.
    fn note_result(&mut self, id: &str, clean: bool);

    /// The loop's iterations so far.
    fn note_loop(&mut self, iteration: usize);

    /// A turn's spend: cache hits, fresh tokens, seconds.
    fn note_turn(&mut self, cache: u64, fresh: u64, secs: u64);

    /// A turn ended, and whether it went well.
    fn note_end(&mut self, cx: &mut dyn HostCx, ok: bool);
}

/// A place the world's visitor can be sent to by command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visit {
    Stables,
    Lists,
}
