//! The realm host in the cockpit: the context the cockpit lends it for a call
//! (`angel_live::realm::HostCx`) and the thin `App` methods that forward to it.
//!
//! The game's state is `App::realm`; its
//! controls are written on the game's own side. What a control reads of the
//! cockpit's state is [`Facts`], and what it changes there is the composer, a
//! line in the transcript and a few [`Move`]s the cockpit makes once the call
//! is over. The context borrows the cockpit's fields beside the game's, so the
//! game can never re-enter the state it is running in.

use super::*;
use crate::agent::club::Bag;
use angel_plug::app::AppWorld;
use angel_plug::feed::FeedWorld;
use crate::ui::scryglass::StageSurface;
use angel_live::realm::{Facts, HostCx, Move, Oracle, Panel, RealmHost};

#[cfg(feature = "delve")]
pub(crate) type GameHost = crate::delve::cart::Delve;
#[cfg(not(feature = "delve"))]
pub(crate) type GameHost = crate::console::Shelf;

/// A club as the realm's oracle: one buffered reply, allowed at least the
/// asked-for time (a longer bound the operator set stands), at the realm's
/// own reasoning effort (`realm_effort`).
struct ClubOracle(Arc<dyn crate::agent::club::Club>);

/// What a realm ask (a wish's entry, its mend, a salvage design) reasons at
/// when its club has an effort to choose: `ANGEL_WISH_EFFORT`, `medium`
/// unless set, `inherit` for the club's own. The club in hand reasons at the
/// cockpit's effort, and on 2026-11-04 that was xhigh: six wishes at once
/// took 83-172 s a call and two of six were handed back, where at medium
/// they took 9-31 s and five of six landed (`stress_real_model_wishes`).
const REALM_EFFORT: &str = "medium";

fn realm_effort() -> Option<String> {
    let wanted = std::env::var("ANGEL_WISH_EFFORT")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| REALM_EFFORT.to_string());
    (wanted != "inherit").then_some(wanted)
}

pub(crate) fn club_oracle(club: Arc<dyn crate::agent::club::Club>) -> Arc<dyn Oracle> {
    Arc::new(ClubOracle(club))
}

impl Oracle for ClubOracle {
    fn respond(&self, prompt: &str, at_least: std::time::Duration) -> Result<String, String> {
        crate::agent::club::with_read_timeout_at_least(at_least, || {
            // Only a route with an effort to choose, and only a level it has;
            // any other club is asked as it always was.
            let effort = realm_effort()
                .filter(|_| self.0.reasoning_effort().is_some())
                .and_then(|wanted| self.0.checked_launch_effort(&wanted).ok());
            let Some(effort) = effort else {
                return self.0.respond(prompt);
            };
            let messages = [crate::agent::club::ChatMsg::user(prompt)];
            match self.0.chat_with_effort(&messages, &[], Some(&effort))? {
                crate::agent::club::ClubReply::Text(text) => Ok(text),
                crate::agent::club::ClubReply::Calls(_) => {
                    Err("the model asked for a tool with no tools offered".to_string())
                }
            }
        })
    }

    fn label(&self) -> &str {
        self.0.label()
    }

    fn warm_up(&self) {
        self.0.warm_up();
    }
}

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
        club_oracle(self.bag.in_hand())
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

    pub(crate) fn with_cart<R>(
        &mut self,
        index: usize,
        f: impl FnOnce(&mut dyn angel_live::game::Cartridge, &mut AppCx<'_>) -> R,
    ) -> Option<R> {
        let facts = self.realm_facts();
        let App {
            games,
            tools,
            bag,
            input,
            cursor,
            messages,
            redraw_requested,
            ..
        } = self;
        let cart = games.carts.get_mut(index)?;
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
        let result = f(cart.as_mut(), &mut cx);
        let moves = std::mem::take(&mut cx.moves);
        self.make_moves(moves);
        Some(result)
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
                    if let Some(family) = self.realm.house_family(house) {
                        if !self.open_agent_menu_filtered(
                            crate::ui::agent_panel::controls::AgentMenuKind::Model,
                            family,
                        ) {
                            self.system_msg(format!(
                                "The model menu opens for {family} once this turn is done."
                            ));
                        }
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
                Move::DismissIntro => self.startup_intro.dismiss(
                    std::time::Instant::now(),
                    crate::telemetry::MotionMode::Off,
                ),
                Move::ResetWorldYaw => self.reset_world_yaw(),
            }
        }
    }

    // ---- the keyboard and the pointer

    pub(crate) fn dungeon_key(&mut self, key: event::KeyEvent) -> bool {
        if let Some(index) = self.cart_on_screen() {
            return self.cart_key(index, key);
        }
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
        if let Some(index) = self.cart_on_screen() {
            self.with_cart(index, |cart, cx| cart.clear_controls(cx));
        }
        self.with_realm(|realm, cx| RealmHost::clear_controls(realm, cx));
    }

    pub(crate) fn collapse_dungeon(&mut self) {
        if self.cart_on_screen().is_some() {
            self.hide_cart();
        }
        self.with_realm(|realm, cx| RealmHost::collapse(realm, cx));
    }

    // ---- the commands

    pub(crate) fn dungeon_command(&mut self, argument: Option<&str>) -> String {
        self.realm.loop_snapshot(&|| self.loop_ctl.feed_facts());
        self.with_realm(|realm, cx| RealmHost::command(realm, cx, argument))
    }

    pub(crate) fn walk_from_command(&mut self) -> String {
        self.with_realm(|realm, cx| RealmHost::walk_from_command(realm, cx))
    }

    // ---- the cadence

    /// The event loop's step for the game.
    pub(crate) fn tick_realm(&mut self) {
        self.with_realm(|realm, cx| RealmHost::tick(realm, cx));
        self.tick_carts();
    }

    pub(crate) fn steer_walk(&mut self) {
        self.with_realm(|realm, cx| RealmHost::steer_walk(realm, cx));
    }

    pub(crate) fn settle_walk(&mut self) {
        self.with_realm(|realm, cx| RealmHost::settle_walk(realm, cx));
    }

    // ---- the questions

    pub(crate) fn dungeon_view_active(&self) -> bool {
        self.cart_on_screen().is_some() || RealmHost::view_active(&self.realm, &self.realm_facts())
    }

    pub(crate) fn dungeon_controls_blocked(&self) -> bool {
        if self.cart_on_screen().is_some() {
            let facts = self.realm_facts();
            return facts.dialog_open() || facts.typing_elsewhere();
        }
        RealmHost::controls_blocked(&self.realm, &self.realm_facts())
    }

    pub(crate) fn game_keyboard_active(&self) -> bool {
        self.cart_on_screen().is_some()
            || RealmHost::keyboard_active(&self.realm, &self.realm_facts())
    }

    /// The game wants the fast cadence: the delve plays, or he walks.
    pub(crate) fn realm_wants_fast_tick(&self) -> bool {
        self.cart_on_screen().is_some()
            || RealmHost::wants_fast_tick(&self.realm, &self.realm_facts())
    }

    pub(crate) fn dungeon_friends_seated(&self) -> bool {
        self.realm.friends_seated()
    }

    pub(crate) fn dungeon_step_due(&self) -> Option<std::time::Instant> {
        RealmHost::step_due(&self.realm, &self.realm_facts())
    }

    // ---- the harness's work, counted in the holdings

    pub(crate) fn note_ops_call(&mut self, id: &str, name: &str, args: &str) {
        RealmHost::note_call(&mut self.realm, id, name, args);
        self.note_carts(|cart| cart.note_call(id, name, args));
    }

    pub(crate) fn note_ops_result(&mut self, id: &str, clean: bool) {
        RealmHost::note_result(&mut self.realm, id, clean);
        self.note_carts(|cart| cart.note_result(id, clean));
    }

    pub(crate) fn note_ops_loop(&mut self, iteration: usize) {
        RealmHost::note_loop(&mut self.realm, iteration);
        self.note_carts(|cart| cart.note_loop(iteration));
    }

    pub(crate) fn note_ops_turn(&mut self, cache: u64, fresh: u64, secs: u64) {
        RealmHost::note_turn(&mut self.realm, cache, fresh, secs);
        self.note_carts(|cart| cart.note_turn(cache, fresh, secs));
    }

    pub(crate) fn note_ops_end(&mut self, ok: bool) {
        self.with_realm(|realm, cx| RealmHost::note_end(realm, cx, ok));
        for index in 0..self.games.carts.len() {
            self.with_cart(index, |cart, cx| cart.note_end(cx, ok));
        }
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
