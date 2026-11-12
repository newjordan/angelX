use angel_live::realm::{Facts, HostCx, RealmHost, Visit};
use angel_realm_host::host::GameHost;
use ratatui::crossterm::event::KeyEvent;
use std::ops::{Deref, DerefMut};
use std::time::Instant;

pub mod app;
pub(crate) mod commands;
pub mod drive;
pub mod feed;
pub(crate) mod glass;
pub(crate) mod loop_art;
pub mod stage;
pub mod viewer;

pub struct Delve {
    pub(crate) host: GameHost,
    pub feed: feed::State,
    pub(crate) stage: stage::State,
}

impl Delve {
    pub(crate) fn new(host: GameHost) -> Self {
        Self {
            host,
            feed: Default::default(),
            stage: Default::default(),
        }
    }
}

impl Deref for Delve {
    type Target = GameHost;

    fn deref(&self) -> &GameHost {
        &self.host
    }
}

impl DerefMut for Delve {
    fn deref_mut(&mut self) -> &mut GameHost {
        &mut self.host
    }
}

impl RealmHost for Delve {
    fn key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool {
        RealmHost::key(&mut self.host, cx, key)
    }

    fn walk_key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool {
        RealmHost::walk_key(&mut self.host, cx, key)
    }

    fn release_key(&mut self, key: KeyEvent) {
        RealmHost::release_key(&mut self.host, key)
    }

    fn walk_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool {
        RealmHost::walk_click(&mut self.host, cx, x, y)
    }

    fn gate_click(&mut self, cx: &mut dyn HostCx, x: u16, y: u16) -> bool {
        RealmHost::gate_click(&mut self.host, cx, x, y)
    }

    fn clear_controls(&mut self, cx: &mut dyn HostCx) {
        RealmHost::clear_controls(&mut self.host, cx)
    }

    fn collapse(&mut self, cx: &mut dyn HostCx) {
        RealmHost::collapse(&mut self.host, cx)
    }

    fn open_intro(&mut self, cx: &mut dyn HostCx) {
        RealmHost::open_intro(&mut self.host, cx)
    }

    fn command(&mut self, cx: &mut dyn HostCx, arg: Option<&str>) -> String {
        RealmHost::command(&mut self.host, cx, arg)
    }

    fn walk_from_command(&mut self, cx: &mut dyn HostCx) -> String {
        RealmHost::walk_from_command(&mut self.host, cx)
    }

    fn visit(&mut self, cx: &mut dyn HostCx, place: Visit) -> String {
        RealmHost::visit(&mut self.host, cx, place)
    }

    fn stop_walk(&mut self) {
        RealmHost::stop_walk(&mut self.host)
    }

    fn go_in(&mut self, cx: &mut dyn HostCx) {
        RealmHost::go_in(&mut self.host, cx)
    }

    fn tick(&mut self, cx: &mut dyn HostCx) {
        RealmHost::tick(&mut self.host, cx)
    }

    fn steer_walk(&mut self, cx: &mut dyn HostCx) {
        RealmHost::steer_walk(&mut self.host, cx)
    }

    fn settle_walk(&mut self, cx: &mut dyn HostCx) {
        RealmHost::settle_walk(&mut self.host, cx)
    }

    fn view_active(&self, facts: &Facts) -> bool {
        RealmHost::view_active(&self.host, facts)
    }

    fn controls_blocked(&self, facts: &Facts) -> bool {
        RealmHost::controls_blocked(&self.host, facts)
    }

    fn keyboard_active(&self, facts: &Facts) -> bool {
        RealmHost::keyboard_active(&self.host, facts)
    }

    fn wants_fast_tick(&self, facts: &Facts) -> bool {
        RealmHost::wants_fast_tick(&self.host, facts)
    }

    fn step_due(&self, facts: &Facts) -> Option<Instant> {
        RealmHost::step_due(&self.host, facts)
    }

    fn walk_map_shown(&self, facts: &Facts) -> bool {
        RealmHost::walk_map_shown(&self.host, facts)
    }

    fn note_call(&mut self, id: &str, name: &str, args: &str) {
        RealmHost::note_call(&mut self.host, id, name, args)
    }

    fn note_result(&mut self, id: &str, clean: bool) {
        RealmHost::note_result(&mut self.host, id, clean)
    }

    fn note_loop(&mut self, iteration: usize) {
        RealmHost::note_loop(&mut self.host, iteration)
    }

    fn note_turn(&mut self, cache: u64, fresh: u64, secs: u64) {
        RealmHost::note_turn(&mut self.host, cache, fresh, secs)
    }

    fn note_end(&mut self, cx: &mut dyn HostCx, ok: bool) {
        RealmHost::note_end(&mut self.host, cx, ok)
    }
}
