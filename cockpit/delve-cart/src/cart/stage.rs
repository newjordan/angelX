use super::Delve;
use crate::cart::commands::{plug_building, world_building};
use crate::identity;
use crate::together_shooter::identity::Making;
use crate::world_viz::{Building, classify_tool_activity, overworld};
use super::viewer::{Pictures, State as Pixels};
use angel_live::host::peer_watch::Heartbeat;
use angel_live::realm::{Facts, HostCx, RealmHost};
use angel_live::telemetry::MotionMode;
use angel_plug::screen::Screen;
use angel_plug::stage::{
    RealmRow, ResearchRail, ResearchRailHits, StageArt, StageCue, StageEffect, StageFacts,
    StagePlaces, StageRules, StageTitles, StageWorld,
};
use angel_plug::viewer::Graphics;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use serde_json::json;
use std::time::Instant;

const PLACES: [Building; 8] = [
    Building::Keep,
    Building::Gatehouse,
    Building::Rookery,
    Building::Scriptorium,
    Building::Smithy,
    Building::Chapel,
    Building::RoundTable,
    Building::Observatory,
];

const TITLES: StageTitles = StageTitles {
    smithy: identity::STAGE_TITLE_SMITHY,
    raytrace: identity::STAGE_TITLE_RAYTRACE,
    quintain_prefix: identity::STAGE_TITLE_QUINTAIN_PREFIX,
    reinforce_prefix: identity::STAGE_TITLE_REINFORCE_PREFIX,
    reinforce_suffix: identity::STAGE_TITLE_REINFORCE_SUFFIX,
    quest_board: identity::STAGE_TITLE_QUEST_BOARD,
    roundtable_prefix: identity::STAGE_TITLE_ROUNDTABLE_PREFIX,
    observatory_prefix: identity::STAGE_TITLE_OBSERVATORY_PREFIX,
    dynamic_suffix: identity::STAGE_TITLE_DYNAMIC_SUFFIX,
    vault: identity::STAGE_TITLE_VAULT,
    arrival: identity::STAGE_TITLE_ARRIVAL,
    explore: identity::STAGE_TITLE_EXPLORE,
    realm: identity::STAGE_TITLE_REALM,
    quest: identity::STAGE_TITLE_QUEST,
};

fn place(id: u8) -> Building {
    PLACES
        .get(usize::from(id))
        .copied()
        .unwrap_or(Building::Keep)
}

#[derive(Default)]
pub(crate) struct State {
    wishes: Option<(Option<Making>, u32)>,
}

/// A pixel frame as `▀` half blocks (fg top, bg bottom), aspect kept and
/// letterboxed on black paper.
pub fn paint_halfblock_frame(frame: &mut Frame, area: Rect, img: &overworld::Img) {
    if area.width == 0 || area.height == 0 || img.w <= 0 || img.h <= 0 {
        return;
    }
    let (grid_w, grid_h) = (f32::from(area.width), f32::from(area.height) * 2.0);
    let scale = (grid_w / img.w as f32).min(grid_h / img.h as f32);
    let (draw_w, draw_h) = (img.w as f32 * scale, img.h as f32 * scale);
    let (ox, oy) = ((grid_w - draw_w) / 2.0, (grid_h - draw_h) / 2.0);
    // Each half-block pixel is the mean of the map pixels it covers (at most
    // 4×4), not one of them: a moving picture keeps its colours steady.
    let sample = |gx: f32, gy: f32| -> Color {
        let span = |g: f32, o: f32| {
            let from = ((g - 0.5 - o) / scale).floor() as i32;
            let to = (((g + 0.5 - o) / scale).floor() as i32).clamp(from + 1, from + 4);
            (from, to)
        };
        let ((x0, x1), (y0, y1)) = (span(gx, ox), span(gy, oy));
        let (mut sum, mut n) = ([0u32; 3], 0u32);
        for y in y0..y1 {
            for x in x0..x1 {
                let [r, g, b] = if x < 0 || y < 0 {
                    [0, 0, 0]
                } else {
                    img.get(x, y).unwrap_or([0, 0, 0])
                };
                sum = [
                    sum[0] + u32::from(r),
                    sum[1] + u32::from(g),
                    sum[2] + u32::from(b),
                ];
                n += 1;
            }
        }
        let n = n.max(1);
        Color::Rgb((sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8)
    };
    let buf = frame.buffer_mut();
    for cy in 0..area.height {
        for cx in 0..area.width {
            let (gx, gy) = (f32::from(cx) + 0.5, f32::from(cy) * 2.0 + 0.5);
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                cell.set_char('▀')
                    .set_fg(sample(gx, gy))
                    .set_bg(sample(gx, gy + 1.0));
            }
        }
    }
}

fn yaw(facts: &StageFacts) -> f32 {
    if facts.follow_agent {
        facts.yaw_offset
    } else {
        facts.look_yaw + facts.yaw_offset
    }
}

fn world_sample_size(viewer: &dyn Screen<Pixels>, area: Rect) -> (usize, usize) {
    viewer
        .dot_grid(area)
        .unwrap_or((usize::from(area.width), usize::from(area.height)))
}

/// Map pixels per half-block pixel on terminals without graphics.
const HALFBLOCK_MAP_PX: i32 = 2;

impl StageWorld for Delve {
    fn stage_titles(&self) -> &'static StageTitles {
        &TITLES
    }

    fn quest_owns_pane(&self) -> bool {
        self.world.quest_owns_pane()
    }

    fn visiting_tower(&self) -> bool {
        self.world.visiting_scrying_tower()
    }

    fn inside_interior(&self) -> bool {
        self.world.inside_interior()
    }

    fn together_on(&self) -> bool {
        self.together.enabled()
    }

    fn no_active_work(&self) -> bool {
        self.world.latest_active_work().is_none()
    }

    fn ambient_interior_visible(&self) -> bool {
        self.world.ambient_interior_visible()
    }

    fn walk_on_map(&self, facts: &Facts) -> bool {
        self.walk.on && RealmHost::walk_map_shown(self, facts)
    }

    /// The caption while the visitor walks: the door at hand and the keys, in
    /// plain words.
    fn walk_caption(&self) -> (Line<'static>, bool) {
        use crate::world_viz::overworld::DoorOpens;
        let text = Style::new().fg(angel_plug::palette::HUD_TEXT);
        let dim = Style::new().fg(angel_plug::palette::HUD_DIM);
        let name = Style::new()
            .fg(angel_plug::palette::HUD_GOLD)
            .add_modifier(ratatui::style::Modifier::BOLD);
        match self.world.visitor_door() {
            Some(door) if door.opens().is_some() => (
                Line::from(vec![
                    Span::styled("E: ", text),
                    Span::styled(door.name(), name),
                    Span::styled(", Esc leaves", dim),
                ]),
                true,
            ),
            Some(door) => (
                Line::from(vec![
                    Span::styled(door.name(), name),
                    Span::styled(". Arrows walk, Esc leaves", dim),
                ]),
                false,
            ),
            None => (
                Line::from(Span::styled("Arrows walk, E goes in, Esc leaves", text)),
                false,
            ),
        }
    }

    fn interior_is_scriptorium(&self) -> bool {
        self.world.interior_building() == Some(Building::Scriptorium)
    }

    fn interior_is_observatory(&self) -> bool {
        self.world.interior_building() == Some(Building::Observatory)
    }

    fn live_adventure_view(&self) -> bool {
        self.world.live_adventure_view()
    }

    fn loop_crawl_on(&self) -> bool {
        self.world.loop_crawl_on()
    }

    fn memory_degraded(&self) -> bool {
        self.world.memory_health() == angel_live::telemetry::MemoryHealth::Degraded
    }

    fn has_world(&self) -> bool {
        true
    }

    fn camera_label(&self) -> Option<String> {
        self.world.overworld_view_label().map(str::to_string)
    }

    fn quest_border_style(&self) -> Option<Style> {
        self.world.quest_border_style()
    }

    fn has_authored_interior(&self) -> bool {
        self.world.has_authored_interior()
    }

    fn realm_row(&self, first_person: bool, wide: bool) -> RealmRow {
        let exploring = self.world.live_adventure_view() || first_person;
        RealmRow {
            enter: !self.world.riding() && self.world.has_authored_interior(),
            view: (!self.world.visiting_school()).then_some(if exploring {
                "Map"
            } else {
                "Explore"
            }),
            tower: true,
            research: wide,
        }
    }

    fn interior_tag(&self) -> Option<u8> {
        self.world
            .interior_building()
            .map(|building| building as u8)
    }

    fn note_peers(&mut self, peers: &[Heartbeat], now_ms: u128) {
        self.world.note_guardian_peers(peers, now_ms);
    }

    fn begin_stage_frame(&mut self) {
        self.overworld_map = None;
        self.dungeon.controls_visible = false;
    }

    fn clear_map_view(&mut self) {
        self.overworld_map = None;
    }

    fn latest_work(&self) -> Option<(String, String)> {
        self.world
            .latest_active_work()
            .map(|work| (work.literal.clone(), format!("{:?}", work.landmark)))
    }

    fn world_semantics(&self, selected_building: Option<String>) -> serde_json::Value {
        let active_work = self
            .world
            .active_work()
            .map(|work| {
                json!({
                    "id": work.id.0,
                    "literal": work.literal,
                    "landmark": format!("{:?}", work.landmark),
                    "activity": format!("{:?}", work.activity),
                    "summary": work.summary,
                    "outcome": work.outcome.map(|outcome| json!({
                        "execution": outcome.execution.as_str(),
                        "verification": outcome.verification.as_str()
                    }))
                })
            })
            .collect::<Vec<_>>();
        let next_unlock = self.world.next_unlock().map(|(threshold, label)| {
            json!({ "threshold": threshold, "label": label, "remaining": threshold.saturating_sub(self.world.renown()) })
        });
        let hearth = self.world.hearth_state();
        json!({
            "town": self.world.town_name(),
            "tier": self.world.level(),
            "renown": self.world.renown(),
            "next_unlock": next_unlock,
            "riding": self.world.riding(),
            "arrived": self.world.arrived_building().map(|building| format!("{:?}", building)),
            "inside_interior": self.world.inside_interior(),
            "interior": self.world.interior_building().map(|building| format!("{:?}", building)),
            "destination": format!("{:?}", self.world.destination()),
            "selected_building": selected_building,
            "camera_mode": self.world.camera_mode_label(),
            "active_events": active_work,
            "event_diagnostics": self.world.event_diagnostics(),
            "causal_ribbon": self.world.causal_ribbon(),
            "environment": {
                "weather": self.world.weather_label(),
                "hearth": { "light": hearth.light, "warmth": hearth.warmth, "beats": hearth.beats, "prosperity": hearth.prosperity },
                "day_phase": hearth.phase().label()
            },
            "memory_health": format!("{:?}", self.world.memory_health()).to_ascii_lowercase()
        })
    }
}

impl StagePlaces for Delve {
    type Place = angel_plug::commands::Building;

    fn place_of(id: u8) -> Option<Self::Place> {
        PLACES.get(usize::from(id)).copied().map(plug_building)
    }

    fn place_id(place: Self::Place) -> u8 {
        world_building(place) as u8
    }

    fn arrived_place(&self) -> Option<Self::Place> {
        self.world.arrived_building().map(plug_building)
    }

    fn destination_place(&self) -> Option<Self::Place> {
        Some(plug_building(self.world.destination()))
    }

    fn tool_destination(name: &str, args: &str) -> Option<Self::Place> {
        Some(plug_building(classify_tool_activity(name, args).building))
    }
}

impl StageRules for Delve {
    fn press_tower(&mut self) -> StageEffect {
        self.world.visit_scrying_tower();
        StageEffect {
            explore: Some(Building::Observatory as u8),
            ..StageEffect::default()
        }
    }

    fn press_library(&mut self) -> StageEffect {
        self.world.begin_teaching_lesson("open shelves");
        StageEffect {
            explore: Some(Building::Scriptorium as u8),
            ..StageEffect::default()
        }
    }

    fn press_toggle_camera(&mut self) -> StageEffect {
        let mut effect = StageEffect {
            reset_yaw: true,
            ..StageEffect::default()
        };
        if self.world.toggle_adventure_map() {
            effect.return_to_world = true;
            effect.follow = true;
        } else if !self.world.visiting_school() && !self.world.inside_interior() {
            effect.toggle_route = Some(self.world.destination() as u8);
        }
        effect
    }

    fn press_enter(&mut self) -> StageEffect {
        let entered = self.world.enter_interior();
        StageEffect {
            reset_yaw: entered,
            back_from_arrival: entered,
            ..StageEffect::default()
        }
    }

    fn press_leave(&mut self) -> StageEffect {
        StageEffect {
            reset_yaw: self.world.leave_interior(),
            ..StageEffect::default()
        }
    }

    fn press_catalog(&mut self) -> StageEffect {
        StageEffect {
            open_catalog: self.world.interior_building() == Some(Building::Scriptorium),
            ..StageEffect::default()
        }
    }

    fn press_follow(&mut self, realm_route: bool) -> StageEffect {
        let mut effect = StageEffect::default();
        if self.walk.on {
            effect.stop_walk = true;
        } else if self.world.visiting_scrying_tower() {
            self.world.follow_overworld();
            effect.return_to_world = true;
        } else if self.world.live_adventure_view() {
            effect.reset_yaw = true;
            effect.follow = true;
        } else if realm_route {
            if self.world.overworld_view_label().is_some() {
                self.world.follow_overworld();
            } else {
                self.world.cycle_camera_zoom();
            }
        } else {
            effect.follow = true;
        }
        effect
    }

    fn press_landmark(&mut self, id: u8) -> StageEffect {
        self.world.select_landmark(place(id));
        StageEffect {
            reset_yaw: true,
            return_to_world: true,
            ..StageEffect::default()
        }
    }

    fn teach(&mut self, topic: &str) {
        self.world.begin_teaching_lesson(topic);
    }
}

impl Delve {
    fn paint_world_frame(
        &self,
        frame: &mut Frame,
        viewer: &mut dyn Screen<Pixels>,
        facts: &StageFacts,
        area: Rect,
        image: &std::sync::Arc<angel_stage::braille::ColoredBrailleImage>,
    ) {
        viewer.paint_dots(frame, area, &facts.workspace, self.interior_tag(), image);
    }

    fn scene_frame(
        &mut self,
        frame: &mut Frame,
        viewer: &mut dyn Screen<Pixels>,
        facts: &StageFacts,
        area: Rect,
    ) {
        let (width, height) = world_sample_size(viewer, area);
        let yaw = yaw(facts);
        if let Some(Some(world)) = facts.compose_allowed.then(|| {
            self.world.scryglass_frame_with_motion(
                width,
                height,
                facts.scenery_relaxed,
                yaw,
                facts.look_pitch,
                facts.fov,
                facts.motion,
            )
        }) {
            self.paint_world_frame(frame, viewer, facts, area, &world);
        }
    }

    /// A running quest's picture: the region it's out in, walked as the Delve's
    /// crawl (with two lines under its picture for what the party is doing) or
    /// as the older expedition.
    fn live_adventure(
        &mut self,
        frame: &mut Frame,
        viewer: &mut dyn Screen<Pixels>,
        facts: &StageFacts,
        area: Rect,
    ) {
        let crawl = crate::world_viz::crawl::enabled() && area.height >= 10;
        let (area, log) = if crawl {
            (
                Rect {
                    height: area.height - 2,
                    ..area
                },
                Some(Rect {
                    y: area.bottom() - 2,
                    height: 2,
                    ..area
                }),
            )
        } else {
            (area, None)
        };
        self.scene_frame(frame, viewer, facts, area);
        if let Some(log) = log {
            let said = self.world.crawl_log(facts.motion);
            let back = Style::default().bg(Color::Rgb(5, 8, 12));
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        format!(" {}", said[0]),
                        Style::default().fg(Color::Rgb(196, 178, 140)),
                    )),
                    Line::from(Span::styled(
                        format!(" {}", said[1]),
                        Style::default().fg(Color::Rgb(122, 124, 114)),
                    )),
                ])
                .style(back),
                log,
            );
        }
    }

    /// The Realm route's base layer: the pixel overworld. The camera's scale is
    /// locked to the terminal's cell size and the view takes the pane's shape,
    /// so a larger pane (or a zoomed-out terminal) shows more of the realm
    /// rather than a bigger picture of the same place. Image-capable terminals
    /// get the frame through the graphics protocol; any other terminal gets it
    /// in half blocks.
    fn overworld(
        &mut self,
        frame: &mut Frame,
        viewer: &mut dyn Screen<Pixels>,
        facts: &StageFacts,
        area: Rect,
    ) {
        let paper = Color::Rgb(0, 0, 0);
        let buf = frame.buffer_mut();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ').set_bg(paper);
                }
            }
        }
        let mut scene = self.overworld_scene(facts);
        // A school room is drawn in the map's place; only the map can be walked.
        let walkable = scene.school_room.is_none();
        // Under an open dialog the map draws in half blocks: a terminal image
        // would repaint over the dialog each time the map moves.
        let covered = facts.occluders.iter().any(|dialog| dialog.intersects(area));
        if viewer.map_pixels_native() && !covered {
            let (cell_w, cell_h) = viewer.map_cell_pixels();
            let scale = overworld::map_scale(cell_h);
            let view_w = (u32::from(area.width) * u32::from(cell_w) / scale).max(1) as i32;
            let view_h = (u32::from(area.height) * u32::from(cell_h) / scale).max(1) as i32;
            let lazy = overworld::LazyFrame::new(scene.clone(), view_w, view_h, scale);
            let (frame_w, frame_h) = lazy.size();
            let key = scene.key() ^ (u64::from(frame_w) << 32 | u64::from(frame_h)).rotate_left(11);
            // In the window the mini-viz presents these pixels itself;
            // the viewer's encode/decode round trip below is for
            // terminal graphics protocols.
            if viewer.console().picture(
                key,
                (frame_w, frame_h),
                area,
                frame.buffer_mut(),
                &mut || lazy.as_ref().to_vec(),
            ) {
                if walkable {
                    self.overworld_map =
                        Some((area, overworld::view_sized(&scene, view_w, view_h)));
                }
                return;
            }
            let state =
                viewer.render_map_pixels(frame, area, key, move || (lazy, frame_w, frame_h));
            if state == super::viewer::WorldPixelsState::Ready {
                if walkable {
                    self.overworld_map =
                        Some((area, overworld::view_sized(&scene, view_w, view_h)));
                }
                return;
            }
        }
        // Ordinary terminals, and the first frame while an image encodes: two
        // map pixels per half-block pixel, the view shaped like the pane. The
        // camera keeps to that two-pixel grid, so a moving view shifts whole
        // half-block pixels instead of resampling (and shimmering) every one.
        let snap = |c: f32| (c / HALFBLOCK_MAP_PX as f32).round() * HALFBLOCK_MAP_PX as f32;
        scene.camera = (snap(scene.camera.0), snap(scene.camera.1));
        let (view_w, view_h) = (
            i32::from(area.width) * HALFBLOCK_MAP_PX,
            i32::from(area.height) * 2 * HALFBLOCK_MAP_PX,
        );
        if walkable {
            self.overworld_map = Some((area, overworld::view_sized(&scene, view_w, view_h)));
        }
        paint_halfblock_frame(
            frame,
            area,
            &overworld::frame_cached(&scene, view_w, view_h),
        );
    }

    /// The live scene, plus the travel and arrival glass the stage is showing.
    pub fn overworld_scene(&self, facts: &StageFacts) -> overworld::Scene {
        let mut scene = self.world.overworld_scene_moving(facts.motion);
        // Travel and arrival cues, which used to take the whole pane, become a
        // glass over the map: the ride while the knight travels, the place's
        // painting when he arrives.
        if overworld::glass_enabled()
            && self.world.overworld_view_label().is_none()
            && !self.world.graph_visiting()
            && !self.walk.on
        {
            // A serving knight rides home to his own castle, not the Keep: the
            // Keep's ride and arrival glass would say otherwise.
            let home = self.world.overworld_home().is_some();
            scene.glass = match facts.cue {
                StageCue::Arrival(destination) | StageCue::Journey(destination)
                    if home && place(destination) == Building::Keep =>
                {
                    None
                }
                StageCue::Arrival(destination) => {
                    Some(self.world.overworld_plate_glass(place(destination)))
                }
                StageCue::Journey(destination) => {
                    Some(self.world.overworld_ride_glass(place(destination)))
                }
                StageCue::None => None,
            };
        }
        overworld::pace(&mut scene, facts.scenery_relaxed);
        scene
    }

    /// An explicitly entered room has one synchronous Dotmax presentation. The
    /// original source art never goes through the native image worker.
    pub fn dotmax_interior(
        &mut self,
        frame: &mut Frame,
        viewer: &mut dyn Screen<Pixels>,
        facts: &StageFacts,
        area: Rect,
    ) -> bool {
        if !self.world.ambient_interior_visible() {
            return false;
        }
        let (width, height) = world_sample_size(viewer, area);
        let explicit_still =
            self.world.visiting_scrying_tower() && facts.comp_mode;
        let paint = || {
            let Some(image) = self.world.ambient_braille_frame(
                width,
                height,
                if explicit_still {
                    MotionMode::Off
                } else {
                    facts.motion
                },
            ) else {
                return false;
            };
            let tag = self
                .world
                .interior_building()
                .map(|building| building as u8);
            viewer.paint_dots(frame, area, &facts.workspace, tag, &image);
            true
        };
        // An operator-opened tower is a cached still in competition mode. It does
        // not restart ambient simulation or its fast tick.
        (explicit_still || facts.compose_allowed)
            .then(paint)
            .unwrap_or(false)
    }
}

impl StageArt for Delve {
    fn draw_world_map(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        facts: &StageFacts,
        viewer: &mut dyn Screen<Pixels>,
    ) -> bool {
        // A running quest leaves the town map for the actual region mesh. A
        // deliberate /world visit (including the school) keeps its chosen place.
        if self.world.live_adventure_view() {
            self.live_adventure(frame, viewer, facts, area);
            return true;
        }
        if !self.dotmax_interior(frame, viewer, facts, area) && overworld::map_enabled() {
            self.overworld(frame, viewer, facts, area);
        } else if !self.world.ambient_interior_visible() {
            self.scene_frame(frame, viewer, facts, area);
        }
        true
    }

    fn draw_first_person(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        facts: &StageFacts,
        viewer: &mut dyn Screen<Pixels>,
    ) -> bool {
        self.scene_frame(frame, viewer, facts, area);
        true
    }

    fn draw_arrival(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        facts: &StageFacts,
        viewer: &mut dyn Screen<Pixels>,
    ) -> bool {
        if !self.dotmax_interior(frame, viewer, facts, area) {
            self.scene_frame(frame, viewer, facts, area);
        }
        true
    }

    fn draw_interior(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        facts: &StageFacts,
        viewer: &mut dyn Screen<Pixels>,
    ) -> bool {
        let (width, height) = world_sample_size(viewer, area);
        if !self.dotmax_interior(frame, viewer, facts, area)
            && let Some(world) = facts
                .compose_allowed
                .then(|| {
                    self.world
                        .ambient_braille_frame(width, height, facts.motion)
                })
                .flatten()
        {
            self.paint_world_frame(frame, viewer, facts, area, &world);
        }
        true
    }

    fn draw_live_adventure(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        facts: &StageFacts,
        viewer: &mut dyn Screen<Pixels>,
    ) -> bool {
        self.live_adventure(frame, viewer, facts, area);
        true
    }

    fn draw_together(&mut self, frame: &mut Frame, area: Rect) -> bool {
        crate::together_viz::render(frame, &self.together, area);
        true
    }

    fn research_rail_width(&self, inner: Rect) -> u16 {
        if inner.width >= 96 && inner.height >= 20 {
            27
        } else {
            0
        }
    }

    fn draw_research_rail(
        &mut self,
        frame: &mut Frame,
        map: Rect,
        bottom: u16,
        rail: &ResearchRail,
    ) -> ResearchRailHits {
        use angel_plug::drive::Place;
        let mut hits = ResearchRailHits::default();
        let caption = Rect::new(map.x, map.bottom(), map.width, 1);
        frame.render_widget(
            Paragraph::new("REALM / RESEARCH PLACES")
                .style(Style::new().fg(angel_plug::palette::HUD_DIM)),
            caption,
        );
        let mut y = caption.bottom();
        for place in Place::ALL {
            if y >= bottom.saturating_sub(1) {
                break;
            }
            let selected = place as usize == rail.selected;
            let active = rail.active[place as usize];
            let label = format!(
                "{} {} {}{}",
                if selected { "▸" } else { "├" },
                place.building().glyph(),
                place.label(),
                if active > 0 {
                    format!(" · {active}")
                } else {
                    String::new()
                }
            );
            let rect = Rect::new(map.x, y, map.width, 1);
            frame.render_widget(
                Paragraph::new(angel_plug::text::fit(
                    &label,
                    map.width as usize,
                ))
                .style(Style::new().fg(if selected || active > 0 {
                    angel_plug::palette::HUD_PHOSPHOR
                } else {
                    angel_plug::palette::HUD_TEXT
                })),
                rect,
            );
            hits.places.push((rect, place as usize));
            y += 1;
        }
        if y + 3 < bottom {
            y += 1;
            frame.render_widget(
                Paragraph::new("SELECTED EVIDENCE")
                    .style(Style::new().fg(angel_plug::palette::HUD_DIM)),
                Rect::new(map.x, y, map.width, 1),
            );
            if let Some(entry) = &rail.entry {
                let text = format!(
                    "{}\n{}\n{}",
                    entry.state,
                    angel_plug::text::fit(&entry.title, map.width as usize),
                    entry.summary
                );
                frame.render_widget(
                    Paragraph::new(text)
                        .wrap(Wrap { trim: false })
                        .style(Style::new().fg(entry.color)),
                    Rect::new(map.x, y + 1, map.width, bottom.saturating_sub(y + 3).min(5)),
                );
            }
            let inspect = Rect::new(map.x, bottom - 2, map.width, 1);
            frame.render_widget(
                Paragraph::new("[Enter] Inspect selection")
                    .style(Style::new().fg(angel_plug::palette::HUD_PHOSPHOR)),
                inspect,
            );
            hits.inspect = Some(inspect);
        }
        hits
    }

    fn prepare_game(&mut self, cx: &mut dyn HostCx) {
        let player = self.dungeon.forge.draft.as_ref().map(|d| d.player);
        let wishes = {
            let session = self.session(cx);
            (
                player.and_then(|id| session.wish_making(id)),
                session.wish_depth(),
            )
        };
        self.stage.wishes = Some(wishes);
    }

    fn draw_game(
        &mut self,
        frame: &mut Frame,
        root: Rect,
        facts: &Facts,
        harness: &str,
        viewer: &mut dyn Screen<Pixels>,
    ) {
        let blocked = RealmHost::controls_blocked(self, facts);
        let now = Instant::now();
        // The host's delve as its newest tick stands (the words around the
        // room read it), and the seat whose own client draws the room.
        let screen = self.view().screen();
        let deck = self
            .dungeon
            .forge
            .draft
            .as_ref()
            .map(|d| self.dungeon.wish_deck(d.player))
            .unwrap_or_default();
        let (making, depth) = self.stage.wishes.take().unwrap_or((None, 1));
        let known = crate::known_wishes(&deck, making.as_ref(), depth);
        let (upgrades, upgrade_note) = self
            .dungeon
            .forge
            .draft
            .as_ref()
            .map(|d| self.dungeon.wish_upgrades(d.player))
            .unwrap_or_default();
        // The room goes out as the terminal's own picture this frame: its
        // cells under it need not be drawn.
        let native = viewer.graphics() == Graphics::Kitty
            && viewer.map_pixels_native()
            && !blocked
            && self.dungeon.intro.is_none()
            && self.dungeon.forge.draft.is_none()
            && !self.dungeon.cards_open
            && self.dungeon.exhibit_text.is_none()
            && self.dungeon.activity_menu.is_none();
        viewer.clear_still();
        self.dungeon.controls_visible = if let Some(intro) = &self.dungeon.intro {
            crate::delve_intro_viz::render(
                frame,
                root,
                intro,
                self.dungeon.realm_shown(),
                viewer.console(),
            );
            true
        } else if let Some(seat) = self.dungeon.activity_menu {
            frame.render_widget(ratatui::widgets::Clear, root);
            let couch = self.dungeon.joined.is_none() && self.dungeon.hosting.is_none();
            let help = if couch {
                "1 / 2 selects a knight · Tab switches"
            } else {
                ""
            };
            frame.render_widget(ratatui::widgets::Paragraph::new(format!(
                "PAUSED · Knight {seat}\n\n[ LEAVE ]  Enter / Backspace\n\nEsc resumes · F2 returns to the composer\n{help}"
            )).block(ratatui::widgets::Block::bordered().title("Activity menu")), root);
            true
        } else if let Some(text) = &self.dungeon.exhibit_text {
            frame.render_widget(ratatui::widgets::Clear, root);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(text.as_str())
                    .block(
                        ratatui::widgets::Block::bordered()
                            .title("Host-local research · ↑/↓ scroll · E/Esc close"),
                    )
                    .wrap(ratatui::widgets::Wrap { trim: false })
                    .scroll((self.dungeon.exhibit_scroll, 0)),
                root,
            );
            true
        } else if let Some(joined) = &self.dungeon.joined {
            // Cells drawn from the seat's own picture, made off this thread.
            let picture = if native {
                None
            } else {
                joined.seat.request(now);
                joined.seat.picture()
            };
            crate::joined_viz::render(
                frame,
                joined,
                picture.as_ref(),
                root,
                &self.dungeon.notice,
                self.dungeon.chorus.subtitle(std::time::Instant::now()),
                self.dungeon
                    .forge
                    .draft
                    .as_ref()
                    .map(|d| crate::shooter_viz::Wish {
                        part: d.part.word(),
                        words: d.words.as_str(),
                        confirm: d.confirm,
                        known: known.clone(),
                        pick: d.pick.clone(),
                        resolved: d.known.as_ref().and_then(|i| {
                            known
                                .iter()
                                .find(|k| &k.0 == i)
                                .map(|k| (k.1.clone(), k.2.clone()))
                        }),
                        sanctuary: d.sanctuary,
                        scroll: joined.run().and_then(|run| {
                            crate::shooter_viz::scroll_of(&run, joined.actor())
                        }),
                        upgrades: upgrades.clone(),
                        upgrade_note,
                        upgrade_page: d.upgrade_page,
                    }),
                self.dungeon
                    .cards_open
                    .then_some((self.dungeon.cards_page, self.dungeon.cards_sel)),
                native,
            )
        } else if let Some((snapshot, knight)) = &screen {
            // Cells are drawn from the seat's own picture: made on its own
            // worker while the server steps on its own thread, and here when
            // the delve is stepped by hand (tests), so a test sees this frame's.
            let seat = self.dungeon.shooter.seat(*knight);
            let picture = if native {
                None
            } else if let Some(seat) = seat {
                if self.dungeon.shooter.threaded() {
                    seat.request(now);
                } else {
                    seat.frame(now);
                }
                seat.picture()
            } else {
                None
            };
            crate::shooter_viz::render_room(
                frame,
                &snapshot.run,
                crate::shooter_viz::Room::Seat(picture.as_ref()),
                root,
                &self.dungeon.notice,
                self.dungeon.key_releases,
                !facts.terminal_focused || blocked,
                self.dungeon.hosting.is_some(),
                harness,
                self.dungeon
                    .cards_open
                    .then_some((self.dungeon.cards_page, self.dungeon.cards_sel)),
                self.dungeon.realm_shown(),
                self.dungeon.chorus.subtitle(std::time::Instant::now()),
                self.dungeon
                    .forge
                    .draft
                    .as_ref()
                    .map(|d| crate::shooter_viz::Wish {
                        part: d.part.word(),
                        words: d.words.as_str(),
                        confirm: d.confirm,
                        known: known.clone(),
                        pick: d.pick.clone(),
                        resolved: d.known.as_ref().and_then(|i| {
                            known
                                .iter()
                                .find(|k| &k.0 == i)
                                .map(|k| (k.1.clone(), k.2.clone()))
                        }),
                        sanctuary: d.sanctuary,
                        scroll: crate::shooter_viz::scroll_of(&snapshot.run, d.player),
                        upgrades: upgrades.clone(),
                        upgrade_note,
                        upgrade_page: d.upgrade_page,
                    }),
                native,
            )
        } else {
            crate::together_viz::render_game(
                frame,
                &self.together,
                root,
                &self.dungeon.notice,
            )
        } && !blocked;
        // The wish scroll's choices, the wish sealed, the genie freed: the
        // seat's picture carries them when it is shown whole; in cells,
        // the scroll's own half-block version.
        if !native
            && self.dungeon.forge.draft.is_none()
            && self.dungeon.activity_menu.is_none()
            && !self.dungeon.cards_open
        {
            if let Some(joined) = &self.dungeon.joined {
                if let Some(run) = joined.run() {
                    crate::shooter_viz::offer_box(frame, root, &run, joined.actor());
                }
            } else if let Some((snapshot, knight)) = &screen {
                crate::shooter_viz::offer_box(frame, root, &snapshot.run, *knight);
            }
        }
        // Text overlays must stay above the game: fall back to cells while a
        // dialog owns input instead of placing an image over its buttons.
        if !blocked
            && self.dungeon.intro.is_none()
            && self.dungeon.forge.draft.is_none()
            && !self.dungeon.cards_open
            && self.dungeon.activity_menu.is_none()
        {
            let seat = match (&self.dungeon.joined, &screen) {
                (Some(joined), _) => Some(joined.seat.clone()),
                (None, Some((_, knight))) => self.dungeon.shooter.seat(*knight).cloned(),
                (None, None) => None,
            };
            if let Some(seat) = seat {
                crate::shooter_viz::render_seat(frame, viewer, &seat, root, true, now);
            }
        }
        // Couch friends retain the activity's camera; the departed host has
        // a small view of their own safe entrance alongside it, from the
        // host's own seat.
        if self.dungeon.hosting.is_none()
            && self.dungeon.activity_menu.is_none()
            && !self.dungeon.cards_open
            && !blocked
            && let Some((snapshot, 2)) = &screen
            && snapshot.run.exits.outside.contains_key(&1)
            && root.width >= 80
            && root.height >= 24
            && let Some(host) = self.dungeon.shooter.seat(1)
        {
            if self.dungeon.shooter.threaded() {
                host.request(now);
            } else {
                host.frame(now);
            }
            let inset = Rect::new(root.right() - 25, root.bottom() - 13, 24, 7);
            let border = ratatui::widgets::Block::bordered().title("P1 outside · E rejoin");
            let inside = border.inner(inset);
            frame.render_widget(ratatui::widgets::Clear, inset);
            if let Some(picture) = host.picture() {
                crate::shooter_viz::paint_picture(frame, inside, &picture);
            }
            frame.render_widget(border, inset);
        }
    }
}
