//! The realm host as the cockpit holds it: the Delve's controls and the
//! miniworld it plays on, behind [`angel_live::realm::RealmHost`].
//!
//! [`GameHost`] owns the game's state; a [`Session`] is that state for the
//! length of one call, with the [`HostCx`] the cockpit lent for it. The
//! controls (`control/`) are written on `Session`, so reading the cockpit's
//! state or asking it for something is `self.cx.…` and the game's own state
//! is `self.…`: the session holds each part of the game by reference, so the
//! controls borrow them apart as they would fields of one struct.

use crate::control::{DungeonView, Walk};
use crate::loop_summary::LoopSummary;
use crate::together::Together;
use crate::world_viz::{self, World};
use angel_live::realm::{Facts, HostCx, RealmHost, Visit};
use crossterm::event::KeyEvent;
use ratatui_core::layout::Rect;

/// The game's state: the delve, the walk, the miniworld and the party room.
pub struct GameHost {
    /// The miniworld: the artifacts pane's default content.
    pub world: World,
    /// Opt-in local prototype of cooperative world gameplay and game authoring.
    pub together: Together,
    /// The delve: its run, the host's server, the card screen, the forge.
    pub dungeon: DungeonView,
    /// Walk mode: the operator's own figure on the mini-viz map.
    pub walk: Walk,
    /// Stamped by the draw that laid out the overworld map: its cells and the
    /// view of the map they show.
    pub overworld_map: Option<(Rect, world_viz::overworld::View)>,
    /// The facts of the harness's loop, as the cockpit last gave them
    /// ([`GameHost::set_loop`]); the settlement reads them.
    pub loop_summary: LoopSummary,
}

impl GameHost {
    /// A host on this miniworld, with no delve open.
    pub fn new(world: World) -> Self {
        Self {
            world,
            together: Together::default(),
            dungeon: DungeonView::default(),
            walk: Walk::default(),
            overworld_map: None,
            loop_summary: LoopSummary::default(),
        }
    }

    /// The loop facts the settlement reads from now on.
    pub fn set_loop(&mut self, summary: LoopSummary) {
        self.loop_summary = summary;
    }

    /// The island's realm (the play economy), loaded beside the world's
    /// rewards on first use.
    pub fn economy(&mut self) -> &mut crate::together_realm::Realm {
        crate::control::economy(&mut self.dungeon.realm, &self.world)
    }

    /// Fold the loop's receipts into the settlement's map, once a loop has an
    /// id (the cockpit's per-frame step).
    pub fn sync_loop(&mut self, summary: &LoopSummary) {
        let player = crate::control::host_name();
        let tavern = self
            .economy()
            .home
            .level(crate::together_shooter::home::Station::Wing)
            >= 1;
        self.world.sync_settlement(summary, &player, tavern);
    }

    /// This host for one call, in the cockpit's context. The session holds
    /// each part of the game by itself, so the controls borrow them as they
    /// would fields of one struct.
    pub fn session<'a>(&'a mut self, cx: &'a mut dyn HostCx) -> Session<'a> {
        Session {
            world: &mut self.world,
            together: &mut self.together,
            dungeon: &mut self.dungeon,
            walk: &mut self.walk,
            overworld_map: &mut self.overworld_map,
            loop_summary: &self.loop_summary,
            cx,
        }
    }

    /// This host read-only: what the cockpit asks while it draws.
    pub fn view(&self) -> View<'_> {
        View {
            world: &self.world,
            together: &self.together,
            dungeon: &self.dungeon,
            walk: &self.walk,
            overworld_map: &self.overworld_map,
        }
    }
}

/// The game's state, shared: the questions it answers (`control::queries`).
#[derive(Clone, Copy)]
pub struct View<'a> {
    pub world: &'a World,
    pub together: &'a Together,
    pub dungeon: &'a DungeonView,
    pub walk: &'a Walk,
    pub overworld_map: &'a Option<(Rect, world_viz::overworld::View)>,
}

/// The game's controls for the length of one call.
pub struct Session<'a> {
    pub world: &'a mut World,
    pub together: &'a mut Together,
    pub dungeon: &'a mut DungeonView,
    pub walk: &'a mut Walk,
    pub overworld_map: &'a mut Option<(Rect, world_viz::overworld::View)>,
    pub loop_summary: &'a LoopSummary,
    pub cx: &'a mut dyn HostCx,
}

impl<'a> Session<'a> {
    /// What the cockpit's state is now.
    pub(crate) fn facts(&self) -> Facts {
        self.cx.facts()
    }

    /// The game's state, read-only.
    pub(crate) fn view(&self) -> View<'_> {
        View {
            world: self.world,
            together: self.together,
            dungeon: self.dungeon,
            walk: self.walk,
            overworld_map: self.overworld_map,
        }
    }

    /// Say a line in the transcript, as the system.
    pub(crate) fn system_msg(&mut self, text: impl Into<String>) {
        self.cx.say(text.into());
    }
}

impl RealmHost for GameHost {
    fn key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool {
        self.session(cx).dungeon_key(key)
    }

    fn walk_key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool {
        self.session(cx).walk_key(key)
    }

    fn release_key(&mut self, key: KeyEvent) {
        self.walk.release_walk_key(key);
    }

    fn walk_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool {
        self.session(cx).walk_click(x, y)
    }

    fn gate_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool {
        self.session(cx).gate_click(x, y)
    }

    fn clear_controls(&mut self, cx: &mut dyn HostCx) {
        self.session(cx).clear_dungeon_controls();
    }

    fn collapse(&mut self, cx: &mut dyn HostCx) {
        self.session(cx).collapse_dungeon();
    }

    fn open_intro(&mut self, cx: &mut dyn HostCx) {
        self.session(cx).open_intro();
    }

    fn command(&mut self, cx: &mut dyn HostCx, arg: Option<&str>) -> String {
        self.session(cx).dungeon_command(arg)
    }

    fn walk_from_command(&mut self, cx: &mut dyn HostCx) -> String {
        self.session(cx).walk_from_command()
    }

    fn visit(&mut self, cx: &mut dyn HostCx, place: Visit) -> String {
        let place = match place {
            Visit::Stables => crate::chivalry::Place::Stables,
            Visit::Lists => crate::chivalry::Place::Tournament,
        };
        self.session(cx).visit_horsemanship(place)
    }

    fn stop_walk(&mut self) {
        self.world.stop_walk();
        self.walk.on = false;
        self.walk.held.clear();
    }

    fn go_in(&mut self, cx: &mut dyn HostCx) {
        self.session(cx).go_in();
    }

    fn tick(&mut self, cx: &mut dyn HostCx) {
        let mut session = self.session(cx);
        session.advance_dungeon_guest();
        session.watch_delve_gate();
        session.tune_audio();
        session.watch_reforge();
        session.advance_joined();
    }

    fn steer_walk(&mut self, cx: &mut dyn HostCx) {
        self.session(cx).steer_walk();
    }

    fn settle_walk(&mut self, cx: &mut dyn HostCx) {
        self.session(cx).settle_walk();
    }

    fn view_active(&self, facts: &Facts) -> bool {
        self.view().view_active(facts)
    }

    fn controls_blocked(&self, facts: &Facts) -> bool {
        self.view().controls_blocked(facts)
    }

    fn keyboard_active(&self, facts: &Facts) -> bool {
        self.view().game_keyboard_active(facts)
    }

    fn wants_fast_tick(&self, facts: &Facts) -> bool {
        let view = self.view();
        view.dungeon_wants_fast_tick(facts) || view.walk_wants_fast_tick(facts)
    }

    fn step_due(&self, facts: &Facts) -> Option<std::time::Instant> {
        self.view().step_due(facts)
    }

    fn walk_map_shown(&self, facts: &Facts) -> bool {
        self.view().walk_map_shown(facts)
    }

    fn note_call(&mut self, id: &str, name: &str, args: &str) {
        self.dungeon.note_ops_call(id, name, args);
    }

    fn note_result(&mut self, id: &str, clean: bool) {
        self.dungeon.note_ops_result(id, clean);
    }

    fn note_loop(&mut self, iteration: usize) {
        self.dungeon.note_ops_loop(iteration);
    }

    fn note_turn(&mut self, cache: u64, fresh: u64, secs: u64) {
        self.dungeon.note_ops_turn(cache, fresh, secs);
    }

    fn note_end(&mut self, cx: &mut dyn HostCx, ok: bool) {
        self.session(cx).note_ops_end(ok);
    }
}

#[cfg(test)]
#[path = "../tests/host__tests.rs"]
mod tests;
