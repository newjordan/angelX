use crate::screen::Screen;
use crate::viewer::ViewerArt;
use angel_live::host::peer_watch::Heartbeat;
use angel_live::realm::{Facts, HostCx};
use angel_live::telemetry::MotionMode;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::terminal::Frame;
use ratatui_core::text::Line;
use std::path::PathBuf;

pub struct StageTitles {
    pub smithy: &'static str,
    pub raytrace: &'static str,
    pub quintain_prefix: &'static str,
    pub reinforce_prefix: &'static str,
    pub reinforce_suffix: &'static str,
    pub quest_board: &'static str,
    pub roundtable_prefix: &'static str,
    pub observatory_prefix: &'static str,
    pub dynamic_suffix: &'static str,
    pub vault: &'static str,
    pub arrival: &'static str,
    pub explore: &'static str,
    pub realm: &'static str,
    pub quest: &'static str,
}

pub const CONSOLE_TITLES: StageTitles = StageTitles {
    smithy: " Workshop ",
    raytrace: " Tools / Raytrace ",
    quintain_prefix: " Loop · ",
    reinforce_prefix: " Reinforce · ",
    reinforce_suffix: " ",
    quest_board: " Quest Board ",
    roundtable_prefix: " Round Table · ",
    observatory_prefix: " Observatory · ",
    dynamic_suffix: " ",
    vault: " Vault ",
    arrival: "ARRIVAL",
    explore: "SHELF",
    realm: "SHELF",
    quest: "QUEST",
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StageCue {
    #[default]
    None,
    Arrival(u8),
    Journey(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RealmRow {
    pub enter: bool,
    pub view: Option<&'static str>,
    pub tower: bool,
    pub research: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StageFacts {
    pub motion: MotionMode,
    pub scenery_relaxed: bool,
    pub compose_allowed: bool,
    pub comp_mode: bool,
    pub follow_agent: bool,
    pub look_yaw: f32,
    pub look_pitch: f32,
    pub fov: f32,
    pub yaw_offset: f32,
    pub cue: StageCue,
    pub occluders: Vec<Rect>,
    pub workspace: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResearchEntry {
    pub state: &'static str,
    pub color: Color,
    pub title: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResearchRail {
    pub selected: usize,
    pub active: [usize; 5],
    pub entry: Option<ResearchEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearchRailHits {
    pub places: Vec<(Rect, usize)>,
    pub inspect: Option<Rect>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageEffect {
    pub reset_yaw: bool,
    pub stop_walk: bool,
    pub return_to_world: bool,
    pub follow: bool,
    pub back_from_arrival: bool,
    pub open_catalog: bool,
    pub explore: Option<u8>,
    pub toggle_route: Option<u8>,
}

pub trait StageWorld {
    fn stage_titles(&self) -> &'static StageTitles {
        &CONSOLE_TITLES
    }

    fn quest_owns_pane(&self) -> bool {
        false
    }

    fn visiting_tower(&self) -> bool {
        false
    }

    fn inside_interior(&self) -> bool {
        false
    }

    fn together_on(&self) -> bool {
        false
    }

    fn no_active_work(&self) -> bool {
        true
    }

    fn ambient_interior_visible(&self) -> bool {
        false
    }

    fn walk_on_map(&self, _facts: &Facts) -> bool {
        false
    }

    fn walk_caption(&self) -> (Line<'static>, bool) {
        (Line::from(""), false)
    }

    fn interior_is_scriptorium(&self) -> bool {
        false
    }

    fn interior_is_observatory(&self) -> bool {
        false
    }

    fn interior_tag(&self) -> Option<u8> {
        None
    }

    fn live_adventure_view(&self) -> bool {
        false
    }

    fn loop_crawl_on(&self) -> bool {
        false
    }

    fn memory_degraded(&self) -> bool {
        false
    }

    fn has_world(&self) -> bool {
        false
    }

    fn camera_label(&self) -> Option<String> {
        None
    }

    fn quest_border_style(&self) -> Option<Style> {
        None
    }

    fn has_authored_interior(&self) -> bool {
        false
    }

    fn realm_row(&self, _first_person: bool, _wide: bool) -> RealmRow {
        RealmRow::default()
    }

    fn note_peers(&mut self, _peers: &[Heartbeat], _now_ms: u128) {}

    fn begin_stage_frame(&mut self) {}

    fn clear_map_view(&mut self) {}

    fn latest_work(&self) -> Option<(String, String)> {
        None
    }

    fn world_semantics(&self, _selected_building: Option<String>) -> serde_json::Value {
        serde_json::Value::Null
    }
}

pub trait StagePlaces {
    type Place: Copy;

    fn place_of(_id: u8) -> Option<Self::Place> {
        None
    }

    fn place_id(_place: Self::Place) -> u8 {
        0
    }

    fn arrived_place(&self) -> Option<Self::Place> {
        None
    }

    fn destination_place(&self) -> Option<Self::Place> {
        None
    }

    fn tool_destination(_name: &str, _args: &str) -> Option<Self::Place> {
        None
    }
}

pub trait StageArt: ViewerArt {
    fn draw_world_map(
        &mut self,
        _frame: &mut Frame,
        _area: Rect,
        _facts: &StageFacts,
        _screen: &mut dyn Screen<Self::Pixels>,
    ) -> bool {
        false
    }

    fn draw_first_person(
        &mut self,
        _frame: &mut Frame,
        _area: Rect,
        _facts: &StageFacts,
        _screen: &mut dyn Screen<Self::Pixels>,
    ) -> bool {
        false
    }

    fn draw_arrival(
        &mut self,
        _frame: &mut Frame,
        _area: Rect,
        _facts: &StageFacts,
        _screen: &mut dyn Screen<Self::Pixels>,
    ) -> bool {
        false
    }

    fn draw_interior(
        &mut self,
        _frame: &mut Frame,
        _area: Rect,
        _facts: &StageFacts,
        _screen: &mut dyn Screen<Self::Pixels>,
    ) -> bool {
        false
    }

    fn draw_live_adventure(
        &mut self,
        _frame: &mut Frame,
        _area: Rect,
        _facts: &StageFacts,
        _screen: &mut dyn Screen<Self::Pixels>,
    ) -> bool {
        false
    }

    fn draw_together(&mut self, _frame: &mut Frame, _area: Rect) -> bool {
        false
    }

    fn research_rail_width(&self, _inner: Rect) -> u16 {
        0
    }

    fn draw_research_rail(
        &mut self,
        _frame: &mut Frame,
        _map: Rect,
        _bottom: u16,
        _rail: &ResearchRail,
    ) -> ResearchRailHits {
        ResearchRailHits::default()
    }

    fn prepare_game(&mut self, _cx: &mut dyn HostCx) {}

    fn draw_game(
        &mut self,
        _frame: &mut Frame,
        _root: Rect,
        _facts: &Facts,
        _harness: &str,
        _screen: &mut dyn Screen<Self::Pixels>,
    ) {
    }
}

pub trait StageRules {
    fn press_tower(&mut self) -> StageEffect {
        StageEffect::default()
    }

    fn press_library(&mut self) -> StageEffect {
        StageEffect::default()
    }

    fn press_toggle_camera(&mut self) -> StageEffect {
        StageEffect::default()
    }

    fn press_enter(&mut self) -> StageEffect {
        StageEffect::default()
    }

    fn press_leave(&mut self) -> StageEffect {
        StageEffect::default()
    }

    fn press_catalog(&mut self) -> StageEffect {
        StageEffect::default()
    }

    fn press_follow(&mut self, _realm_route: bool) -> StageEffect {
        StageEffect::default()
    }

    fn press_landmark(&mut self, _id: u8) -> StageEffect {
        StageEffect::default()
    }

    fn teach(&mut self, _topic: &str) {}
}
