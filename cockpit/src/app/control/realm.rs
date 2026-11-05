//! The realm host in the cockpit: the context the cockpit lends it for a call
//! (`angel_live::realm::HostCx`) and the thin `App` methods that forward to it.
//!
//! The game's state is `App::realm` (`angel_realm_host::host::GameHost`); its
//! controls are written on the game's own side. What a control reads of the
//! cockpit's state is [`Facts`], and what it changes there is the composer, a
//! line in the transcript and a few [`Move`]s the cockpit makes once the call
//! is over. The context borrows the cockpit's fields beside the game's, so the
//! game can never re-enter the state it is running in.

use super::*;
use crate::agent::club::Bag;
use crate::stage::houses;
use crate::ui::scryglass::StageSurface;
use angel_live::realm::{Facts, HostCx, Move, Oracle, Panel, RealmHost, Visit};
use angel_realm_host::chivalry::Place;
use angel_realm_host::host::GameHost;

/// The cockpit's state as the game sees it for one call.
pub(crate) struct AppCx<'a> {
    facts: Facts,
    tools: &'a crate::agent::harness::ToolRegistry,
    bag: &'a Bag,
    input: &'a mut String,
    cursor: &'a mut usize,
    messages: &'a mut Vec<Message>,
    redraw: &'a mut bool,
    moves: Vec<Move>,
}

impl HostCx for AppCx<'_> {
    fn facts(&self) -> Facts {
        self.facts
    }

    fn workspace(&self) -> PathBuf {
        self.tools.current_workspace().to_path_buf()
    }

    fn oracle(&self) -> Arc<dyn Oracle> {
        crate::drive::realm_services::oracle(self.bag.in_hand())
    }

    fn say(&mut self, text: String) {
        self.messages.push(Message::new(Role::System, text));
    }

    fn redraw(&mut self) {
        *self.redraw = true;
    }

    fn take_composer(&mut self) -> (String, usize) {
        let text = std::mem::take(self.input);
        let cursor = std::mem::replace(self.cursor, 0);
        self.facts.composer_empty = true;
        (text, cursor)
    }

    fn set_composer(&mut self, text: String, cursor: usize) {
        *self.input = text;
        *self.cursor = cursor;
        self.facts.composer_empty = self.input.is_empty();
    }

    fn make(&mut self, ask: Move) {
        self.moves.push(ask);
    }
}

impl App {
    /// What the game reads of the cockpit's state.
    pub(crate) fn realm_facts(&self) -> Facts {
        Facts {
            composer_empty: self.input.is_empty(),
            terminal_focused: self.terminal_focused,
            shell_focused: self.shell_focused,
            tutor_draft: self.tutor_draft.is_some(),
            deck_open: self.moa_deck.is_some(),
            deck_owns_input: self.moa_deck_owns_input(),
            approval_pending: self.pending_approval.is_some(),
            loop_dialog: self.loop_dialog.is_some(),
            agent_menu: self.agent_menu.is_some(),
            turn_running: self.thinking.is_some(),
            turn_queued: self.pending_turn.is_some(),
            background_job: self.bg_job.is_some(),
            artifacts_focused: self
                .module_host
                .focused()
                .is_some_and(|id| id.as_str() == "artifacts"),
            world_map_shown: self.scryglass.surface == StageSurface::WorldMap
                && self.scryglass.active_media().is_none(),
            world_pane_visible: self.world_pane_visible,
        }
    }

    /// Run `f` on the game with the cockpit's context, then make the moves it
    /// asked for.
    pub(crate) fn with_realm<R>(
        &mut self,
        f: impl FnOnce(&mut GameHost, &mut AppCx<'_>) -> R,
    ) -> R {
        #[cfg(test)]
        crate::platform::workspace_store::install_test_hooks();
        let facts = self.realm_facts();
        let App {
            realm,
            tools,
            bag,
            input,
            cursor,
            messages,
            redraw_requested,
            ..
        } = self;
        let mut cx = AppCx {
            facts,
            tools,
            bag,
            input,
            cursor,
            messages,
            redraw: redraw_requested,
            moves: Vec::new(),
        };
        let result = f(realm, &mut cx);
        let moves = std::mem::take(&mut cx.moves);
        self.make_moves(moves);
        result
    }

    /// The larger moves the game asked for, in the order asked.
    fn make_moves(&mut self, moves: Vec<Move>) {
        for ask in moves {
            match ask {
                Move::Focus(id) => self.focus_module(id),
                Move::ShowWorld => {
                    self.selection = None;
                    self.scryglass_drag = None;
                    self.scryglass_enabled = true;
                    self.scryglass.return_to_world();
                }
                Move::Open(Panel::Formation) => self.open_moa_deck(None),
                Move::Open(Panel::Library) => {
                    self.apply_world_button(crate::app::WorldButton::ScryglassLibrary);
                }
                Move::Open(Panel::Tower) => self.apply_world_button(crate::app::WorldButton::Tower),
                Move::Open(Panel::ModelMenu(house)) => {
                    let family = houses::get(houses::HouseId(house)).family;
                    if !self.open_agent_menu_filtered(
                        crate::ui::agent_panel::controls::AgentMenuKind::Model,
                        family,
                    ) {
                        self.system_msg(format!(
                            "The model menu opens for {family} once this turn is done."
                        ));
                    }
                }
                Move::Submit { brief, keep_draft } => {
                    let draft = if keep_draft {
                        Some(std::mem::take(&mut self.input))
                    } else {
                        None
                    };
                    self.input = brief;
                    self.cursor = self.input.len();
                    self.submit();
                    if let Some(draft) = draft {
                        self.input = draft;
                        self.cursor = self.input.len();
                    }
                }
                Move::DismissIntro => self
                    .startup_intro
                    .dismiss(std::time::Instant::now(), crate::telemetry::MotionMode::Off),
                Move::ResetWorldYaw => self.reset_world_yaw(),
            }
        }
    }

    // ---- the keyboard and the pointer

    pub(crate) fn dungeon_key(&mut self, key: event::KeyEvent) -> bool {
        self.with_realm(|realm, cx| RealmHost::key(realm, cx, key))
    }

    pub(crate) fn walk_key(&mut self, key: event::KeyEvent) -> bool {
        self.with_realm(|realm, cx| RealmHost::walk_key(realm, cx, key))
    }

    pub(crate) fn release_walk_key(&mut self, key: event::KeyEvent) {
        RealmHost::release_key(&mut self.realm, key);
    }

    pub(crate) fn walk_click(&mut self, x: u16, y: u16) -> bool {
        self.with_realm(|realm, cx| RealmHost::walk_click(realm, cx, x, y))
    }

    pub(crate) fn gate_click(&mut self, x: u16, y: u16) -> bool {
        self.with_realm(|realm, cx| RealmHost::gate_click(realm, cx, x, y))
    }

    pub(crate) fn clear_dungeon_controls(&mut self) {
        self.with_realm(|realm, cx| RealmHost::clear_controls(realm, cx));
    }

    pub(crate) fn collapse_dungeon(&mut self) {
        self.with_realm(|realm, cx| RealmHost::collapse(realm, cx));
    }

    pub(crate) fn open_intro(&mut self) {
        self.with_realm(|realm, cx| RealmHost::open_intro(realm, cx));
    }

    // ---- the commands

    pub(crate) fn dungeon_command(&mut self, argument: Option<&str>) -> String {
        let summary = self.loop_ctl.summary();
        self.realm.set_loop(summary);
        self.with_realm(|realm, cx| RealmHost::command(realm, cx, argument))
    }

    pub(crate) fn walk_from_command(&mut self) -> String {
        self.with_realm(|realm, cx| RealmHost::walk_from_command(realm, cx))
    }

    pub(crate) fn visit_horsemanship(&mut self, place: Place) -> String {
        let place = match place {
            Place::Stables => Visit::Stables,
            Place::Tournament => Visit::Lists,
        };
        self.with_realm(|realm, cx| RealmHost::visit(realm, cx, place))
    }

    pub(crate) fn stop_walk(&mut self) {
        RealmHost::stop_walk(&mut self.realm);
    }

    pub(crate) fn go_in(&mut self) {
        self.with_realm(|realm, cx| RealmHost::go_in(realm, cx));
    }

    // ---- the cadence

    /// The event loop's step for the game.
    pub(crate) fn tick_realm(&mut self) {
        self.with_realm(|realm, cx| RealmHost::tick(realm, cx));
    }

    pub(crate) fn steer_walk(&mut self) {
        self.with_realm(|realm, cx| RealmHost::steer_walk(realm, cx));
    }

    pub(crate) fn settle_walk(&mut self) {
        self.with_realm(|realm, cx| RealmHost::settle_walk(realm, cx));
    }

    // ---- the questions

    pub(crate) fn dungeon_view_active(&self) -> bool {
        RealmHost::view_active(&self.realm, &self.realm_facts())
    }

    pub(crate) fn dungeon_controls_blocked(&self) -> bool {
        RealmHost::controls_blocked(&self.realm, &self.realm_facts())
    }

    pub(crate) fn game_keyboard_active(&self) -> bool {
        RealmHost::keyboard_active(&self.realm, &self.realm_facts())
    }

    /// The game wants the fast cadence: the delve plays, or he walks.
    pub(crate) fn realm_wants_fast_tick(&self) -> bool {
        RealmHost::wants_fast_tick(&self.realm, &self.realm_facts())
    }

    pub(crate) fn dungeon_friends_seated(&self) -> bool {
        self.realm.view().friends_seated()
    }

    pub(crate) fn dungeon_step_due(&self) -> Option<std::time::Instant> {
        RealmHost::step_due(&self.realm, &self.realm_facts())
    }

    pub(crate) fn walk_map_shown(&self) -> bool {
        RealmHost::walk_map_shown(&self.realm, &self.realm_facts())
    }

    // ---- the harness's work, counted in the holdings

    pub(crate) fn note_ops_call(&mut self, id: &str, name: &str, args: &str) {
        RealmHost::note_call(&mut self.realm, id, name, args);
    }

    pub(crate) fn note_ops_result(&mut self, id: &str, clean: bool) {
        RealmHost::note_result(&mut self.realm, id, clean);
    }

    pub(crate) fn note_ops_loop(&mut self, iteration: usize) {
        RealmHost::note_loop(&mut self.realm, iteration);
    }

    pub(crate) fn note_ops_turn(&mut self, cache: u64, fresh: u64, secs: u64) {
        RealmHost::note_turn(&mut self.realm, cache, fresh, secs);
    }

    pub(crate) fn note_ops_end(&mut self, ok: bool) {
        self.with_realm(|realm, cx| RealmHost::note_end(realm, cx, ok));
    }

    /// The harness line under the game: what the turn is doing.
    pub(crate) fn dungeon_harness_status(&self) -> String {
        if self.pending_approval.is_some() {
            "Harness · needs you · Esc to review".into()
        } else if let Some(thinking) = &self.thinking {
            let seconds = thinking.started.elapsed().as_secs();
            if seconds >= 60 {
                format!("Harness · working {}m · Esc to read", seconds / 60)
            } else {
                format!("Harness · working {seconds}s · Esc to read")
            }
        } else if self.pending_turn.is_some() {
            "Harness · queued · Esc to read".into()
        } else if self.last_turn_outcome.is_some() {
            "Harness · done · Esc to read".into()
        } else {
            "Harness · ready · Esc to code".into()
        }
    }
}

/// The two places in the locked `drive/loop_ctl.rs` that still read
/// `self.world` reach the miniworld through the realm host this way; no other
/// cockpit code does (it says `self.realm.world`).
impl std::ops::Deref for App {
    type Target = GameHost;

    fn deref(&self) -> &GameHost {
        &self.realm
    }
}

impl std::ops::DerefMut for App {
    fn deref_mut(&mut self) -> &mut GameHost {
        &mut self.realm
    }
}
