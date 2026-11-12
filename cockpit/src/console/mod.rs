pub(crate) mod app;
pub(crate) mod commands;
pub(crate) mod drive;
pub(crate) mod feed;
pub(crate) mod glass;
pub(crate) mod loop_art;
pub(crate) mod stage;
pub(crate) mod viewer;

use angel_live::realm::{Facts, HostCx, RealmHost, Visit};
use ratatui::crossterm::event::KeyEvent;
use std::time::Instant;

pub(crate) const NO_REALM: &str =
    "This console holds no Delve. /game lists the cartridges in the slot.";

pub(crate) struct Site {
    pub(crate) id: String,
}

impl Site {
    pub(crate) fn associated(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub(crate) struct World;

impl World {
    pub(crate) fn note_workshop(&mut self) {}

    pub(crate) fn settlement_site(&self) -> Result<Option<&Site>, &str> {
        Ok(None)
    }
}

#[derive(Default)]
pub(crate) struct Shelf {
    pub(crate) world: World,
}

impl RealmHost for Shelf {
    fn key(&mut self, _cx: &mut dyn HostCx, _key: KeyEvent) -> bool {
        false
    }

    fn walk_key(&mut self, _cx: &mut dyn HostCx, _key: KeyEvent) -> bool {
        false
    }

    fn release_key(&mut self, _key: KeyEvent) {}

    fn walk_click(&mut self, _cx: &mut dyn HostCx, _x: u16, _y: u16) -> bool {
        false
    }

    fn gate_click(&mut self, _cx: &mut dyn HostCx, _x: u16, _y: u16) -> bool {
        false
    }

    fn clear_controls(&mut self, _cx: &mut dyn HostCx) {}

    fn collapse(&mut self, _cx: &mut dyn HostCx) {}

    fn open_intro(&mut self, _cx: &mut dyn HostCx) {}

    fn command(&mut self, _cx: &mut dyn HostCx, _arg: Option<&str>) -> String {
        NO_REALM.to_string()
    }

    fn walk_from_command(&mut self, _cx: &mut dyn HostCx) -> String {
        NO_REALM.to_string()
    }

    fn visit(&mut self, _cx: &mut dyn HostCx, _place: Visit) -> String {
        NO_REALM.to_string()
    }

    fn stop_walk(&mut self) {}

    fn go_in(&mut self, _cx: &mut dyn HostCx) {}

    fn tick(&mut self, _cx: &mut dyn HostCx) {}

    fn steer_walk(&mut self, _cx: &mut dyn HostCx) {}

    fn settle_walk(&mut self, _cx: &mut dyn HostCx) {}

    fn view_active(&self, _facts: &Facts) -> bool {
        false
    }

    fn controls_blocked(&self, _facts: &Facts) -> bool {
        true
    }

    fn keyboard_active(&self, _facts: &Facts) -> bool {
        false
    }

    fn wants_fast_tick(&self, _facts: &Facts) -> bool {
        false
    }

    fn step_due(&self, _facts: &Facts) -> Option<Instant> {
        None
    }

    fn walk_map_shown(&self, _facts: &Facts) -> bool {
        false
    }

    fn note_call(&mut self, _id: &str, _name: &str, _args: &str) {}

    fn note_result(&mut self, _id: &str, _clean: bool) {}

    fn note_loop(&mut self, _iteration: usize) {}

    fn note_turn(&mut self, _cache: u64, _fresh: u64, _secs: u64) {}

    fn note_end(&mut self, _cx: &mut dyn HostCx, _ok: bool) {}
}
