use crate::screen::Console;
use angel_live::kernel::Resource;
use angel_live::{GamePolicy, RealmStyle};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct RealmGame {
    pub id: &'static str,
    pub summary: &'static str,
}

pub trait AppWorld: Sized {
    fn raise(session_disabled: bool, workspace: &Path) -> Self;

    fn install_services(
        _ambient_stage_sim_allowed: fn() -> bool,
        _processes: Box<dyn crate::services::Processes>,
        _console: &dyn Console,
    ) {
    }

    fn retirement() -> impl Sized {}

    fn read_settings() {}

    #[cfg(feature = "glass")]
    fn draw_in_window() {}

    fn thread_sim() {}

    fn restore_run(&mut self, _workspace: &Path) {}

    fn completion_ceremony(&mut self, _active: bool) {}

    fn terminal_resized(&mut self) {}

    fn map_moving(&self) -> bool {
        false
    }

    fn scenery_animating(&self) -> bool {
        false
    }

    fn joined_to_host(&self) -> bool {
        false
    }

    fn friends_seated(&self) -> bool {
        false
    }

    fn game_entry(&self) -> Option<RealmGame> {
        None
    }

    fn house_family(&self, _house: u8) -> Option<&'static str> {
        None
    }

    fn live_status(&self) -> Option<String> {
        None
    }

    fn live_style(&mut self, _style: &RealmStyle) {}

    fn live_rules(&mut self, _rules: Option<&Arc<GamePolicy>>) {}

    fn live_characters(&mut self, _resources: &BTreeMap<String, Resource>) {}
}
