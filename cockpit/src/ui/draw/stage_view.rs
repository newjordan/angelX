//! Canonical Realm/Scryglass Stage presentation.
//!
//! Routes and overlay state remain in `scryglass`; world semantics remain in
//! `world_viz`. This module owns their terminal-native Stage composition.

use super::*;

pub(super) fn render_artifacts(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        app.viewer.clear_still();
        return;
    }
    app.clear_expired_lifecycle_ceremony();

    app.scryglass.sync_arrival(app.world.arrived_building());
    let media_is_video = app
        .scryglass
        .active_media()
        .and_then(|index| app.media.get(index))
        .is_some_and(Media::is_video);
    let media_fault = app.scryglass.active_error().is_some();
    let quest_owns_pane = app.world.quest_owns_pane();
    let scene =
        app.scryglass
            .controller
            .resolved_scene(media_is_video, media_fault, quest_owns_pane);
    let scene = match scene {
        crate::ui::scryglass::StageSurface::Still(index)
            if app.media.get(index).is_some_and(|media| !media.is_visual()) =>
        {
            crate::ui::scryglass::StageSurface::Document(index)
        }
        scene => scene,
    };
    app.scryglass.surface = scene;
    if !matches!(scene, crate::ui::scryglass::StageSurface::Still(_)) {
        app.viewer.clear_still();
    }
    if app.pending_approval.is_some() || app.loop_dialog.is_some() || app.agent_menu.is_some() {
        app.viewer.inspector.drag = None;
    }

    match scene {
        crate::ui::scryglass::StageSurface::Research => render_research_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::Workshop => render_workshop_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::Lifecycle => render_lifecycle_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::Moa => render_moa_deck(frame, app, area),
        crate::ui::scryglass::StageSurface::Raytrace => render_raytrace_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::Loop => render_loop_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::Observatory => {
            render_observatory_stage(frame, app, area)
        }
        crate::ui::scryglass::StageSurface::Reinforce => render_rl_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::AgentGraph => {
            render_agent_graph_stage(frame, app, area)
        }
        crate::ui::scryglass::StageSurface::Quest => render_quest_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::Vault => render_vault_stage(frame, app, area),
        crate::ui::scryglass::StageSurface::WorldFirstPerson
        | crate::ui::scryglass::StageSurface::WorldMap
        | crate::ui::scryglass::StageSurface::Lesson
        | crate::ui::scryglass::StageSurface::Catalog
        | crate::ui::scryglass::StageSurface::Arrival(_)
        | crate::ui::scryglass::StageSurface::Still(_)
        | crate::ui::scryglass::StageSurface::Document(_)
        | crate::ui::scryglass::StageSurface::Video(_)
        | crate::ui::scryglass::StageSurface::Fault => {
            render_scryglass(frame, app, area, scene);
        }
        crate::ui::scryglass::StageSurface::Hidden => {
            skip_hidden_stage_compose(app);
        }
    }
    if !matches!(scene, crate::ui::scryglass::StageSurface::Hidden) {
        render_spend_coin_overlay(frame, app, area);
    }
}

fn render_spend_coin_overlay(frame: &mut Frame, app: &mut App, stage: Rect) {
    let Some(coin) = app.spend_coin.as_ref() else {
        return;
    };
    let elapsed_secs = coin.started.elapsed().as_secs_f32();
    let motion = app.spend_coin_motion();
    let Some(pose) = crate::ui::viz::spend_viz::pose(stage, elapsed_secs, motion) else {
        app.spend_coin = None;
        return;
    };

    if pose.face_visible
        && paint_world_overlay_dots(frame, pose.area, crate::ui::viz::spend_viz::asset_path())
    {
        return;
    }

    // Immediate terminal-native fallback while Kitty warms (or when image
    // protocols are unavailable). The narrow edge phase makes the static coin
    // read as a flip without changing its cached image geometry.
    let glyph = if pose.face_visible { "✦" } else { "│" };
    let glyph_area = Rect::new(
        pose.area.x,
        pose.area.y.saturating_add(pose.area.height / 2),
        pose.area.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(glyph).alignment(Alignment::Center).style(
            Style::default()
                .fg(ratatui::style::Color::Rgb(255, 198, 0))
                .add_modifier(Modifier::BOLD),
        ),
        glyph_area,
    );
}

/// Hidden / off-stage panes must not pay world compose or dancer paint.
/// Comp / lean mode is additional: default cockpit is unchanged; when armed
/// the expensive compose is skipped so the operator path stays lean.
/// Kitty plates, braille `World::render`, and the first-person ride share
/// this gate — a compose-off frame must not fall through to the braille map.
pub(crate) fn miniviz_expensive_compose_allowed(
    surface: crate::ui::scryglass::StageSurface,
) -> bool {
    if crate::drive::comp_mode::enabled() {
        return false;
    }
    !matches!(surface, crate::ui::scryglass::StageSurface::Hidden)
}

/// Dancer ticks stay on the visible Stage only — never on the model/tool path.
/// Comp / lean mode keeps the assets selectable in tests but does not paint.
pub(crate) fn miniviz_dancer_paint_allowed(stage_visible: bool, loop_active: bool) -> bool {
    stage_visible && loop_active && !crate::drive::comp_mode::enabled()
}

/// Lesson wrap, catalog listing, and still/video decode.
/// Comp / lean keeps Scryglass chrome; default visible Stage still paints.
pub(crate) fn scryglass_scene_body_allowed() -> bool {
    crate::drive::comp_mode::ambient_stage_sim_allowed()
}

/// Footer captions, Formation strip, and reveal/arrival timers.
/// Comp / lean keeps the route title; default still paints accessories.
pub(crate) fn scryglass_scene_accessories_allowed() -> bool {
    scryglass_scene_body_allowed()
}

/// Live realm/quest/ward/activity suffix on the Scryglass title.
/// Comp / lean keeps the route identity; default still paints the live suffix.
pub(crate) fn scryglass_live_world_title_allowed() -> bool {
    scryglass_scene_accessories_allowed()
}

/// Scryglass block title. `live_realm` is the trimmed `world.title()` tail
/// (without the `"Realm · "` prefix) when live titles are allowed.
pub(crate) fn scryglass_route_title(kind: &str, live_realm: Option<&str>, queued: &str) -> String {
    let _ = (kind, live_realm);
    if queued.is_empty() {
        String::new()
    } else {
        format!(" {queued} ")
    }
}

/// Live loop / reinforce / round-table / observatory status in the Stage title.
/// Comp / lean keeps the route identity; default still paints the live suffix.
pub(crate) fn stage_live_route_title_allowed() -> bool {
    scryglass_scene_accessories_allowed()
}

/// Stage chrome title. `live` is the trimmed viz title when live titles
/// are allowed.
pub(crate) fn stage_route_title(prefix: &str, live: Option<&str>, suffix: &str) -> String {
    match live.filter(|part| !part.is_empty()) {
        Some(part) => format!("{prefix}{part}{suffix}"),
        None => format!("{prefix}{suffix}"),
    }
}

/// Ceremony / live loop may steal the Stage column only while ambient sim
/// is on. Hidden / comp-mode keep operator-selected routes (artifacts
/// module, raytrace, reinforce) but do not force a column for leftover
/// ceremony or hammertime.
pub(crate) fn ambient_stage_column_steal_allowed() -> bool {
    crate::drive::comp_mode::ambient_stage_sim_allowed()
}

/// Whether the artifacts pane should be laid out this frame.
/// Default still pops the Stage for ceremony / loop. Hidden / comp only
/// keep a column the operator already selected.
pub(crate) fn artifacts_pane_active(
    artifacts_module: bool,
    raytrace: bool,
    ceremony: bool,
    loop_visible: bool,
    reinforce: bool,
) -> bool {
    artifacts_module
        || raytrace
        || reinforce
        || (ambient_stage_column_steal_allowed() && (ceremony || loop_visible))
}

/// Skip the expensive compose closure when the pane is hidden or off-stage.
pub(crate) fn maybe_run_expensive_world_compose<F, T>(allowed: bool, compose: F) -> Option<T>
where
    F: FnOnce() -> T,
{
    if allowed { Some(compose()) } else { None }
}

/// Kitty compose, braille `World::render`, and first-person ride.
/// Hidden / comp-mode skip the closure; visible default Stage still paints.
pub(crate) fn maybe_paint_world_scene<F, T>(
    surface: crate::ui::scryglass::StageSurface,
    paint: F,
) -> Option<T>
where
    F: FnOnce() -> T,
{
    maybe_run_expensive_world_compose(miniviz_expensive_compose_allowed(surface), paint)
}

fn skip_hidden_stage_compose(app: &mut App) {
    app.world_pane_visible = false;
}

fn render_workshop_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = hud_block(crate::stage::identity::STAGE_TITLE_SMITHY);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width > 0 && inner.height > 0 {
        app.world_pane_visible = true;
        render_world_map_surface(frame, app, inner);
    }
}

/// The active loop's discoveries as the intro's plotline (see
/// `research_plot`): the dotted contour and wizard above, and under it the
/// run's start and the frontier as machine-speed offsets.
fn render_research_plotline(frame: &mut Frame, app: &mut App, area: Rect) {
    use crate::ui::viz::research_plot;
    let dim = Style::new().fg(crate::ui::hud::HUD_DIM);
    let st = &app.loop_ctl;
    if st.started_ms == 0 {
        frame.render_widget(
            Paragraph::new("no research run yet: /loop starts one").style(dim),
            Rect { height: 1, ..area },
        );
        return;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // A stopped run's clock stops at its last update.
    let until = if app.loop_active() {
        now_ms
    } else {
        st.updated_ms.max(st.started_ms)
    };
    let elapsed = until.saturating_sub(st.started_ms);
    let discoveries = research_plot::discoveries(st);
    let run = st.id.clone();
    let reveal = app.research_plot.observe(
        &run,
        discoveries.len(),
        std::time::Instant::now(),
        app.visual_motion,
    );
    let art = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };
    let (columns, rows) = match app.viewer.dot_geometry(art) {
        Some(geometry) => (geometry.grid_width, geometry.grid_height),
        None => (usize::from(art.width), usize::from(art.height)),
    };
    let image = std::sync::Arc::new(research_plot::compose(
        &discoveries,
        elapsed,
        columns,
        rows,
        reveal,
    ));
    use std::hash::{Hash, Hasher};
    let mut tag = std::collections::hash_map::DefaultHasher::new();
    ("research-plotline", &run).hash(&mut tag);
    paint_dot_frame(frame, app, art, &image, tag.finish());
    let start = research_plot::offset_label(0);
    let frontier = research_plot::offset_label(elapsed);
    let gap = usize::from(area.width).saturating_sub(start.len() + frontier.len());
    frame.render_widget(
        Paragraph::new(format!("{start}{}{frontier}", " ".repeat(gap))).style(dim),
        Rect::new(area.x, art.bottom(), area.width, 1),
    );
}

fn render_research_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    use crate::drive::research_workspace::{Action, Place};
    let block = hud_block(" Research · Realm workspace ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let context = app.refresh_research();
    let map_width = if inner.width >= 96 && inner.height >= 20 {
        27
    } else {
        0
    };
    let mut body = Rect::new(
        inner.x,
        inner.y,
        inner.width.saturating_sub(map_width),
        inner.height,
    );
    // The loop's research plotline rides the top of the stage when there is
    // room for it and the workspace text below.
    if body.height >= 16 && body.width >= 30 {
        let plot_h = (body.height * 2 / 5).clamp(7, 16);
        render_research_plotline(
            frame,
            app,
            Rect {
                height: plot_h,
                ..body
            },
        );
        body.y += plot_h;
        body.height -= plot_h;
    }
    let hits =
        crate::drive::research_workspace::view::render(frame, &mut app.research, body, &context);
    app.world_buttons.extend(
        hits.into_iter()
            .map(|(rect, action)| (rect, WorldButton::Research(action))),
    );
    if map_width > 0 {
        let map = Rect::new(
            body.right() + 1,
            inner.y + 1,
            map_width - 1,
            12.min(inner.height.saturating_sub(2)),
        );
        app.world_pane_visible = true;
        let first_hit = app.world_buttons.len();
        render_world_map_surface(frame, app, map);
        for (_, button) in &mut app.world_buttons[first_hit..] {
            if let WorldButton::ScryglassLandmark(building) = *button {
                *button = WorldButton::Research(Action::Place(Place::from_building(building)));
            }
        }
        let caption = Rect::new(map.x, map.bottom(), map.width, 1);
        frame.render_widget(
            Paragraph::new("REALM / RESEARCH PLACES")
                .style(Style::new().fg(crate::ui::hud::HUD_DIM)),
            caption,
        );
        let mut y = caption.bottom();
        for place in Place::ALL {
            if y >= inner.bottom().saturating_sub(1) {
                break;
            }
            let selected = place == app.research.place;
            let active = app.research.active_places[place as usize];
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
                Paragraph::new(crate::drive::research_workspace::view::fit(
                    &label,
                    map.width as usize,
                ))
                .style(Style::new().fg(if selected || active > 0 {
                    crate::ui::hud::HUD_PHOSPHOR
                } else {
                    crate::ui::hud::HUD_TEXT
                })),
                rect,
            );
            app.world_buttons
                .push((rect, WorldButton::Research(Action::Place(place))));
            y += 1;
        }
        if y + 3 < inner.bottom() {
            y += 1;
            frame.render_widget(
                Paragraph::new("SELECTED EVIDENCE").style(Style::new().fg(crate::ui::hud::HUD_DIM)),
                Rect::new(map.x, y, map.width, 1),
            );
            if let Some(entry) = app.research.selected_entry() {
                let text = format!(
                    "{}\n{}\n{}",
                    entry.state.label(),
                    crate::drive::research_workspace::view::fit(&entry.title, map.width as usize),
                    entry.summary
                );
                frame.render_widget(
                    Paragraph::new(text)
                        .wrap(Wrap { trim: false })
                        .style(Style::new().fg(
                            crate::drive::research_workspace::view::state_color(entry.state),
                        )),
                    Rect::new(
                        map.x,
                        y + 1,
                        map.width,
                        inner.bottom().saturating_sub(y + 3).min(5),
                    ),
                );
            }
            let inspect = Rect::new(map.x, inner.bottom() - 2, map.width, 1);
            frame.render_widget(
                Paragraph::new("[Enter] Inspect selection")
                    .style(Style::new().fg(crate::ui::hud::HUD_PHOSPHOR)),
                inspect,
            );
            app.world_buttons
                .push((inspect, WorldButton::Research(Action::Inspect)));
        }
    }
}

/// MC-Hammer-time duo on the Stage floor while a loop is live.
///
/// Two dancers at once:
/// - **left** — mirrored cartoon twin (`hammertime-a-flip` / `b`), phase-shifted
/// - **right** — video strip (`hammertime2-*`) when installed, else facing A/B
///
/// Both sway/bob a couple of cells so they read as dancing *around*, not glued
/// to a single corner. World overlays stay on subdued dots, not native portraits.
fn render_world_map_surface(frame: &mut Frame, app: &mut App, area: Rect) {
    if !render_dotmax_interior(frame, app, area)
        && crate::stage::world_viz::overworld::map_enabled()
    {
        render_overworld(frame, app, area);
    } else if !app.world.ambient_interior_visible() {
        let (width, height) = world_sample_size(app, area);
        let yaw = if app.scryglass.follow_agent {
            app.world_yaw_offset
        } else {
            app.scryglass.look_yaw + app.world_yaw_offset
        };
        if let Some(Some(world)) =
            maybe_paint_world_scene(crate::ui::scryglass::StageSurface::WorldMap, || {
                app.world.scryglass_frame_paced(
                    width,
                    height,
                    app.scenery_relaxed(),
                    yaw,
                    app.scryglass.look_pitch,
                    app.scryglass.fov,
                )
            })
        {
            paint_world_frame(frame, app, area, &world);
        }
    }
    render_hammertime_mascot(frame, app, area);
}

/// The Realm route's base layer: the pixel overworld. The camera's scale is
/// locked to the terminal's cell size and the view takes the pane's shape,
/// so a larger pane (or a zoomed-out terminal) shows more of the realm
/// rather than a bigger picture of the same place. Image-capable terminals
/// get the frame through the graphics protocol; any other terminal gets it
/// in half blocks.
fn render_overworld(frame: &mut Frame, app: &mut App, area: Rect) {
    use crate::stage::world_viz::overworld;
    let paper = ratatui::style::Color::Rgb(0, 0, 0);
    let buf = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ').set_bg(paper);
            }
        }
    }
    let scene = overworld_scene(app);
    // Under an open dialog the map draws in half blocks: a terminal image
    // would repaint over the dialog each time the map moves.
    let covered = app
        .image_occluders
        .iter()
        .any(|dialog| dialog.intersects(area));
    if app.viewer.map_pixels_native() && !covered {
        let (cell_w, cell_h) = app.viewer.map_cell_pixels();
        let scale = overworld::map_scale(cell_h);
        let view_w = (u32::from(area.width) * u32::from(cell_w) / scale).max(1) as i32;
        let view_h = (u32::from(area.height) * u32::from(cell_h) / scale).max(1) as i32;
        let lazy = overworld::LazyFrame::new(scene.clone(), view_w, view_h, scale);
        let (frame_w, frame_h) = lazy.size();
        let key = scene.key() ^ (u64::from(frame_w) << 32 | u64::from(frame_h)).rotate_left(11);
        let state = app
            .viewer
            .render_map_pixels(frame, area, key, move || (lazy, frame_w, frame_h));
        if state == crate::ui::viewer::WorldPixelsState::Ready {
            return;
        }
    }
    // Ordinary terminals, and the first frame while an image encodes: two
    // map pixels per half-block pixel, the view shaped like the pane.
    let (view_w, view_h) = (
        i32::from(area.width) * HALFBLOCK_MAP_PX,
        i32::from(area.height) * 2 * HALFBLOCK_MAP_PX,
    );
    paint_halfblock_frame(
        frame,
        area,
        &overworld::frame_cached(&scene, view_w, view_h),
    );
}

/// Map pixels per half-block pixel on terminals without graphics.
const HALFBLOCK_MAP_PX: i32 = 2;

/// The live scene, plus the travel and arrival glass the stage is showing.
fn overworld_scene(app: &App) -> crate::stage::world_viz::overworld::Scene {
    let mut scene = app.world.overworld_scene();
    // Travel and arrival cues, which used to take the whole pane, become a
    // glass over the map: the ride while the knight travels, the place's
    // painting when he arrives.
    if crate::stage::world_viz::overworld::glass_enabled() {
        use crate::ui::scryglass::StageOverlay;
        scene.glass = match app.scryglass.controller.overlay() {
            Some(StageOverlay::Arrival { destination }) => {
                Some(app.world.overworld_plate_glass(*destination))
            }
            Some(StageOverlay::Journey { destination, .. }) => {
                Some(app.world.overworld_ride_glass(*destination))
            }
            _ => None,
        };
    }
    crate::stage::world_viz::overworld::pace(&mut scene, app.scenery_relaxed());
    scene
}

/// A pixel frame as `▀` half blocks (fg top, bg bottom), aspect kept and
/// letterboxed on black paper.
fn paint_halfblock_frame(
    frame: &mut Frame,
    area: Rect,
    img: &crate::stage::world_viz::overworld::Img,
) {
    if area.width == 0 || area.height == 0 || img.w <= 0 || img.h <= 0 {
        return;
    }
    let (grid_w, grid_h) = (f32::from(area.width), f32::from(area.height) * 2.0);
    let scale = (grid_w / img.w as f32).min(grid_h / img.h as f32);
    let (draw_w, draw_h) = (img.w as f32 * scale, img.h as f32 * scale);
    let (ox, oy) = ((grid_w - draw_w) / 2.0, (grid_h - draw_h) / 2.0);
    let sample = |gx: f32, gy: f32| -> ratatui::style::Color {
        let (sx, sy) = ((gx - ox) / scale, (gy - oy) / scale);
        let [r, g, b] = if sx < 0.0 || sy < 0.0 {
            [0, 0, 0]
        } else {
            img.get(sx as i32, sy as i32).unwrap_or([0, 0, 0])
        };
        ratatui::style::Color::Rgb(r, g, b)
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

fn render_hammertime_mascot(frame: &mut Frame, app: &mut App, area: Rect) {
    if !miniviz_dancer_paint_allowed(
        true,
        crate::ui::viz::loop_viz::hammertime_active(&app.loop_ctl),
    ) {
        return;
    }
    let side = crate::ui::viz::loop_viz::hammertime_plate_side(area.width, area.height);
    let single_fits = side >= crate::ui::viz::loop_viz::HAMMERTIME_MIN_SIDE
        && area.width >= side.saturating_add(1)
        && area.height >= side.saturating_add(1);
    if !single_fits {
        // Preserve the loop scene when even one authored plate would consume it.
        return;
    }
    let duo_fits = area.width >= side.saturating_mul(2).saturating_add(2);
    if !duo_fits {
        render_hammertime_one(frame, app, area, /*right_lead=*/ true);
        return;
    }
    let t = app.started.elapsed().as_secs_f32();
    let [left, right] = crate::ui::viz::loop_viz::hammertime_duo_boxes(area, t);
    // Twin cartoon always on the left so you get two bodies even while the
    // video plate is still warming its protocol cache.
    paint_static_hammer(frame, app, left, t, /*twin=*/ true);
    paint_lead_hammer(frame, app, right, t);
}

/// Single-plate fallback for narrow Stages (still shows *someone* dancing).
fn render_hammertime_one(frame: &mut Frame, app: &mut App, area: Rect, right_lead: bool) {
    let t = app.started.elapsed().as_secs_f32();
    let side = crate::ui::viz::loop_viz::hammertime_plate_side(area.width, area.height);
    let x = if right_lead {
        area.x + area.width.saturating_sub(side.saturating_add(1))
    } else {
        area.x.saturating_add(1)
    };
    let y = area.y + area.height.saturating_sub(side.saturating_add(1));
    let box_area = Rect::new(x, y, side, side);
    paint_lead_hammer(frame, app, box_area, t);
}

/// Right-side / lead dancer: prefer the installed video strip, else cartoon A/B.
fn paint_lead_hammer(frame: &mut Frame, app: &mut App, box_area: Rect, t: f32) {
    if let Some(frame_path) = crate::ui::viz::loop_viz::mascot_frame(t)
        && paint_world_overlay_dots(frame, box_area, frame_path)
    {
        return;
    }
    paint_static_hammer(frame, app, box_area, t, /*twin=*/ false);
}

fn paint_static_hammer(frame: &mut Frame, _app: &mut App, box_area: Rect, t: f32, twin: bool) {
    let asset = if twin {
        crate::ui::viz::loop_viz::hammertime_twin_asset(t)
    } else {
        crate::ui::viz::loop_viz::hammertime_asset(t)
    };
    let path = crate::platform::runtime_paths::cockpit_dir().join(asset);
    let _ = paint_world_overlay_dots(frame, box_area, &path);
}

/// World overlay source art (hammer dancers and spend coin) convert
/// through the existing colored-dot path. Agent portraits and explicit
/// product stills keep `Viewer::render_portrait`.
fn paint_world_overlay_dots(frame: &mut Frame, area: Rect, path: &std::path::Path) -> bool {
    if area.width == 0 || area.height == 0 {
        return false;
    }
    let Some(image) = crate::ui::term::art::colored_image_braille(
        path,
        area.width as usize,
        area.height as usize,
    ) else {
        return false;
    };
    crate::ui::scryglass::Scryglass::paint_image(frame, area, &image);
    true
}

/// An explicitly entered room has one synchronous Dotmax presentation. The
/// original source art never goes through the native image worker.
fn render_dotmax_interior(frame: &mut Frame, app: &mut App, area: Rect) -> bool {
    if !app.world.ambient_interior_visible() {
        return false;
    }
    let (width, height) = world_sample_size(app, area);
    maybe_paint_world_scene(crate::ui::scryglass::StageSurface::WorldFirstPerson, || {
        let Some(image) = app
            .world
            .ambient_braille_frame(width, height, app.visual_motion)
        else {
            return false;
        };
        paint_world_frame(frame, app, area, &image);
        true
    })
    .unwrap_or(false)
}

fn world_sample_size(app: &App, area: Rect) -> (usize, usize) {
    app.viewer.dot_geometry(area).map_or(
        (usize::from(area.width), usize::from(area.height)),
        |geometry| (geometry.grid_width, geometry.grid_height),
    )
}

fn paint_world_frame(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    image: &std::sync::Arc<crate::ui::term::art::ColoredBrailleImage>,
) {
    paint_dot_frame(frame, app, area, image, 0);
}

fn paint_dot_frame(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    image: &std::sync::Arc<crate::ui::term::art::ColoredBrailleImage>,
    scene_tag: u64,
) {
    if let Some(geometry) = app.viewer.dot_geometry(area) {
        use std::hash::{Hash, Hasher};
        let mut scene = std::collections::hash_map::DefaultHasher::new();
        scene_tag.hash(&mut scene);
        app.tools.current_workspace().hash(&mut scene);
        app.world
            .interior_building()
            .map(|building| building as u8)
            .hash(&mut scene);
        if app.viewer.render_world_dots(
            frame,
            area,
            scene.finish(),
            geometry,
            std::sync::Arc::clone(image),
        ) {
            return;
        }
    }
    // Clear the area with solid Dotmax background so terminal wallpaper or transparency never leaks through.
    let bg = ratatui::style::Color::Rgb(5, 8, 12);
    let buf = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(bg);
            }
        }
    }
    // The first canvas may still be encoding. Its fallback is composed dots,
    // never a source plate; sample the entire scene rather than crop a corner.
    if image.width > usize::from(area.width) || image.height > usize::from(area.height) {
        let width = usize::from(area.width);
        let height = usize::from(area.height);
        let cells = (0..height)
            .flat_map(|y| {
                (0..width).map(move |x| {
                    image
                        .cell(x * image.width / width, y * image.height / height)
                        .unwrap_or_default()
                })
            })
            .collect();
        let coarse = crate::ui::term::art::ColoredBrailleImage {
            width,
            height,
            cells,
        };
        crate::ui::scryglass::Scryglass::paint_image(frame, area, &coarse);
    } else {
        crate::ui::scryglass::Scryglass::paint_image(frame, area, image);
    }
}

fn render_lifecycle_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some((kind, label, started)) = app
        .lifecycle_ceremony
        .as_ref()
        .map(|ceremony| (ceremony.kind, ceremony.label.clone(), ceremony.started))
    else {
        return;
    };
    let block = hud_block(kind.title());
    let inner = block.inner(area);
    let (scene_area, footer_area) = visual_panel_body(inner);
    frame.render_widget(block, area);
    if scene_area.width > 0
        && scene_area.height > 0
        && crate::drive::comp_mode::ambient_stage_sim_allowed()
    {
        let mut fine = false;
        let art_area = Rect {
            height: scene_area.height.saturating_sub(2),
            ..scene_area
        };
        if let Some(geometry) = app.viewer.dot_geometry(art_area)
            && let Some(status) =
                crate::ui::viz::lifecycle_viz::journey_status_rows(kind, &label, scene_area.width)
            && let Some(image) = crate::stage::knight_journey::try_image(
                kind,
                &label,
                started.elapsed().as_secs_f32(),
                geometry.grid_width,
                geometry.grid_height,
                app.visual_motion,
            )
        {
            use std::hash::{Hash, Hasher};
            let mut tag = std::collections::hash_map::DefaultHasher::new();
            ("journey", kind as u8, &label).hash(&mut tag);
            paint_dot_frame(frame, app, art_area, &image, tag.finish());
            let status_area = Rect::new(scene_area.x, art_area.bottom(), scene_area.width, 2);
            frame.render_widget(Paragraph::new(status), status_area);
            fine = true;
        }
        if !fine {
            let scene = crate::ui::viz::lifecycle_viz::render(
                kind,
                &label,
                started.elapsed().as_secs_f32(),
                scene_area.width,
                scene_area.height,
                app.visual_motion,
            );
            frame.render_widget(Paragraph::new(scene), scene_area);
        }
        // LoopStart/etc. used to own the Stage without the hammertime overlay, so
        // a /loop that spent its first seconds on Lifecycle looked "hammer-less"
        // even though the loop was already live. Paint the corner dancer whenever
        // the loop is active, same as Workshop/Loop/WorldMap.
        render_hammertime_mascot(frame, app, scene_area);
    }
    if let Some(footer_area) = footer_area {
        render_panel_back(frame, app, footer_area);
    }
}

fn render_raytrace_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = hud_block(crate::stage::identity::STAGE_TITLE_RAYTRACE);
    let inner = block.inner(area);
    let (scene_area, footer_area) = visual_panel_body(inner);
    frame.render_widget(block, area);
    if scene_area.width > 0
        && scene_area.height > 0
        && crate::drive::comp_mode::ambient_stage_sim_allowed()
    {
        let scene = raytrace::render(
            app.started.elapsed().as_secs_f32(),
            scene_area.width,
            scene_area.height,
        );
        frame.render_widget(Paragraph::new(scene), scene_area);
    }
    if let Some(footer_area) = footer_area {
        render_panel_back(frame, app, footer_area);
    }
}

fn render_loop_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    app.world_pane_visible = true;
    let live =
        stage_live_route_title_allowed().then(|| crate::ui::viz::loop_viz::title(&app.loop_ctl));
    let title = stage_route_title(
        crate::stage::identity::STAGE_TITLE_QUINTAIN_PREFIX,
        live.as_deref().map(str::trim),
        crate::stage::identity::STAGE_TITLE_DYNAMIC_SUFFIX,
    );
    let block = hud_block(title.as_str());
    let inner = block.inner(area);
    let (scene_area, footer_area) = visual_panel_body(inner);
    frame.render_widget(block, area);
    if scene_area.width > 0
        && scene_area.height > 0
        && crate::drive::comp_mode::ambient_stage_sim_allowed()
    {
        let elapsed = app.started.elapsed().as_secs_f32();
        let trench_time = match app.visual_motion {
            crate::ui::viz::lifecycle_viz::MotionMode::Full => elapsed,
            crate::ui::viz::lifecycle_viz::MotionMode::Reduced => elapsed * 0.28,
            crate::ui::viz::lifecycle_viz::MotionMode::Off => 0.0,
        };
        let scene = crate::ui::viz::loop_viz::render(
            &app.loop_ctl,
            &app.submission_slot,
            &app.yukon_fleet,
            trench_time,
            scene_area.width,
            scene_area.height,
        );
        frame.render_widget(Paragraph::new(scene), scene_area);
        render_hammertime_mascot(frame, app, scene_area);
    }
    if let Some(footer_area) = footer_area {
        render_panel_back(frame, app, footer_area);
    }
}

fn render_rl_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    let live =
        stage_live_route_title_allowed().then(|| crate::ui::viz::rl_viz::title(&app.tools.rl()));
    let title = stage_route_title(
        crate::stage::identity::STAGE_TITLE_REINFORCE_PREFIX,
        live.as_deref().map(str::trim),
        crate::stage::identity::STAGE_TITLE_REINFORCE_SUFFIX,
    );
    let block = hud_block(title.as_str());
    let inner = block.inner(area);
    let (scene_area, footer_area) = visual_panel_body(inner);
    frame.render_widget(block, area);
    let (tabs_area, evidence_area) = if scene_area.height >= 2 {
        let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(scene_area);
        (Some(rows[0]), rows[1])
    } else {
        (None, scene_area)
    };
    if let Some(tabs_area) = tabs_area {
        render_rl_view_tabs(frame, app, tabs_area);
    }
    if evidence_area.width > 0
        && evidence_area.height > 0
        && crate::drive::comp_mode::ambient_stage_sim_allowed()
    {
        let scene = crate::ui::viz::rl_viz::render_view(
            &app.tools.rl(),
            app.rl_view,
            app.started.elapsed().as_secs_f32(),
            evidence_area.width,
            evidence_area.height,
        );
        frame.render_widget(Paragraph::new(scene).style(panel_style()), evidence_area);
    }
    if let Some(footer_area) = footer_area {
        render_scene_caption(
            frame,
            app,
            footer_area,
            Line::from(Span::styled(
                "1 branch · 2 research · 3 sankey · ←/→",
                Style::new().fg(crate::ui::hud::HUD_DIM),
            )),
            &[("Back", WorldButton::Back)],
        );
    }
}

fn render_rl_view_tabs(frame: &mut Frame, app: &mut App, area: Rect) {
    let mut spans = Vec::new();
    let mut x = area.x;
    for (index, view) in crate::ui::viz::rl_viz::RlView::ALL
        .iter()
        .copied()
        .enumerate()
    {
        let label = format!("{} {}", index + 1, view.label());
        let width = label.chars().count().saturating_add(2) as u16;
        let gap = u16::from(index > 0);
        if x.saturating_add(gap).saturating_add(width) > area.x + area.width {
            break;
        }
        if gap > 0 {
            spans.push(Span::raw(" "));
            x = x.saturating_add(1);
        }
        let selected = view == app.rl_view;
        let bracket = if selected {
            Style::new().fg(crate::ui::hud::HUD_PHOSPHOR)
        } else {
            Style::new().fg(crate::ui::hud::HUD_DIM)
        };
        let text = if selected {
            Style::new()
                .fg(crate::ui::hud::HUD_PHOSPHOR)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(crate::ui::hud::HUD_TEXT)
        };
        app.world_buttons.push((
            Rect {
                x,
                y: area.y,
                width,
                height: 1,
            },
            WorldButton::RlView(view),
        ));
        spans.push(Span::styled("[", bracket));
        spans.push(Span::styled(label, text));
        spans.push(Span::styled("]", bracket));
        x = x.saturating_add(width);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_agent_graph_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    let live = stage_live_route_title_allowed().then(crate::ui::viz::graph_viz::title);
    let title = stage_route_title(
        crate::stage::identity::STAGE_TITLE_ROUNDTABLE_PREFIX,
        live.as_deref().map(str::trim),
        crate::stage::identity::STAGE_TITLE_DYNAMIC_SUFFIX,
    );
    let block = hud_block(title.as_str());
    let inner = block.inner(area);
    let (scene_area, footer_area) = visual_panel_body(inner);
    frame.render_widget(block, area);
    if scene_area.width > 0
        && scene_area.height > 0
        && crate::drive::comp_mode::ambient_stage_sim_allowed()
    {
        let scene = crate::ui::viz::graph_viz::render(
            app.started.elapsed().as_secs_f32(),
            scene_area.width,
            scene_area.height,
        );
        frame.render_widget(Paragraph::new(scene).style(panel_style()), scene_area);
    }
    if let Some(footer_area) = footer_area {
        render_panel_back(frame, app, footer_area);
    }
}

fn render_observatory_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    let live = stage_live_route_title_allowed().then(|| app.observatory.title());
    let title = stage_route_title(
        crate::stage::identity::STAGE_TITLE_OBSERVATORY_PREFIX,
        live.as_deref().map(str::trim),
        crate::stage::identity::STAGE_TITLE_DYNAMIC_SUFFIX,
    );
    let block = hud_block(title.as_str());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // Comp / lean: keep the route chrome, skip backplane snapshot + catalog.
    if !crate::drive::comp_mode::ambient_stage_sim_allowed() {
        return;
    }
    let mut catalog_area = inner;
    if inner.height >= 10 && inner.width >= 38 {
        let rows = Layout::vertical([Constraint::Length(4), Constraint::Min(1)]).split(inner);
        let backplane = app.tools.backplane();
        let surfaces = backplane.surfaces();
        let routes = backplane.routes();
        let leases = backplane.leases();
        let clerk = app.tools.clerk().status();
        let resolved = app
            .last_turn_outcome
            .as_ref()
            .and_then(|outcome| outcome.resolved_model_revision.as_ref())
            .map(|revision| short_backplane_id(&revision.0))
            .unwrap_or_else(|| "none".to_string());
        let sources = app
            .last_turn_outcome
            .as_ref()
            .map(|outcome| {
                outcome
                    .context_source_ids
                    .iter()
                    .take(3)
                    .map(|source| short_backplane_id(source))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|sources| !sources.is_empty())
            .unwrap_or_else(|| "none".to_string());
        let review = app.atlas.status().review_count;
        let text = format!(
            "BACKPLANE  surfaces {} · routes {} · leases {}\n\
             BINDING    model {resolved} · context {sources}\n\
             GROWTH     /rl policy · /conductor adapters · review {review} · clerk {} q{}",
            surfaces.len(),
            routes.len(),
            leases.len(),
            clerk.health.label(),
            clerk.queue_depth,
        );
        frame.render_widget(
            Paragraph::new(text)
                .style(dim_panel_style())
                .block(Block::default().borders(Borders::BOTTOM)),
            rows[0],
        );
        catalog_area = rows[1];
    }
    crate::app::observatory::render(
        frame,
        &mut app.observatory,
        catalog_area,
        app.started.elapsed().as_secs_f32(),
    );
}

fn short_backplane_id(value: &str) -> String {
    let mut chars = value.chars();
    let head = chars.by_ref().take(18).collect::<String>();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn render_quest_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = hud_block(crate::stage::identity::STAGE_TITLE_QUEST_BOARD);
    let inner = block.inner(area);
    let (body, footer) = visual_panel_body(inner);
    frame.render_widget(block, area);
    if !crate::drive::comp_mode::ambient_stage_sim_allowed() {
        if let Some(footer) = footer {
            render_panel_back(frame, app, footer);
        }
        return;
    }
    let source = app.quest_stage.as_ref();
    let goal = app
        .goal
        .as_ref()
        .map(|goal| {
            goal.text
                .chars()
                .take(body.width.saturating_sub(7) as usize)
                .collect::<String>()
        })
        .unwrap_or_else(|| "none".to_string());
    let status = format!("goal · {goal}   loop · {:?}", app.loop_ctl.status);
    let content = source.map_or_else(
        || "No quest trace is open. Use /quest sample or /quest after a thinking turn.".to_string(),
        |quest| {
            let width = body.width.max(20) as usize;
            let chart = if quest.lexicon {
                crate::stage::questmap::render_lexicon(&quest.label, &quest.trace, width)
            } else {
                crate::stage::questmap::render_trace(
                    &quest.label,
                    &quest.trace,
                    quest.theme.as_deref(),
                    width,
                )
            };
            format!("{status}\n{chart}")
        },
    );
    frame.render_widget(Paragraph::new(content).style(panel_style()), body);
    if let Some(footer) = footer {
        render_panel_back(frame, app, footer);
    }
}

fn render_vault_stage(frame: &mut Frame, app: &mut App, area: Rect) {
    if !app.atlas.enabled() {
        let title = crate::stage::identity::STAGE_TITLE_VAULT;
        let block = hud_block(title);
        let inner = block.inner(area);
        let (body, footer) = visual_panel_body(inner);
        frame.render_widget(block, area);
        if !crate::drive::comp_mode::ambient_stage_sim_allowed() {
            if let Some(footer) = footer {
                render_panel_back(frame, app, footer);
            }
            return;
        }
        if app.media.is_empty() {
            frame.render_widget(
                Paragraph::new(crate::ui::views::status_view::empty_artifacts_text())
                    .style(dim_panel_style()),
                body,
            );
        } else {
            let lines = crate::ui::views::artifacts_view::artifact_lines(
                &app.media,
                app.media_scroll,
                body.height,
            );
            frame.render_widget(Paragraph::new(lines).style(panel_style()), body);
        }
        if let Some(footer) = footer {
            render_panel_back(frame, app, footer);
        }
        return;
    }

    let status = app.atlas.status();
    let title = format!(
        " Realm / Living Atlas · {} · Review {} ",
        status.health.label(),
        status.review_count
    );
    let block = hud_block(title);
    let inner = block.inner(area);
    let (body, footer) = visual_panel_body(inner);
    frame.render_widget(block, area);
    if !crate::drive::comp_mode::ambient_stage_sim_allowed() {
        if let Some(footer) = footer {
            render_panel_back(frame, app, footer);
        }
        return;
    }

    if body.width > 0 && body.height > 0 {
        let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(body);
        let tabs = crate::knowledge::atlas::AtlasLane::ALL
            .iter()
            .enumerate()
            .flat_map(|(index, lane)| {
                let selected = *lane == app.atlas_view.lane;
                let style = if selected {
                    Style::new()
                        .fg(crate::ui::hud::HUD_GOLD)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(crate::ui::hud::HUD_DIM)
                };
                let mut spans = vec![Span::styled(format!(" {} ", lane.label()), style)];
                if index + 1 < crate::knowledge::atlas::AtlasLane::ALL.len() {
                    spans.push(Span::styled("│", Style::new().fg(crate::ui::hud::HUD_DIM)));
                }
                spans
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(Line::from(tabs)), rows[0]);
        render_atlas_content(frame, app, rows[1]);
    }
    if let Some(footer) = footer {
        render_panel_back(frame, app, footer);
    }
}

#[cfg(test)]
pub(crate) fn render_vault_for_test(frame: &mut Frame, app: &mut App, area: Rect) {
    render_vault_stage(frame, app, area);
}

fn render_atlas_content(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.atlas_view.lane == crate::knowledge::atlas::AtlasLane::Artifacts {
        if app.media.is_empty() {
            frame.render_widget(
                Paragraph::new(crate::ui::views::status_view::empty_artifacts_text())
                    .style(dim_panel_style()),
                area,
            );
        } else {
            let lines = crate::ui::views::artifacts_view::artifact_lines(
                &app.media,
                app.media_scroll,
                area.height,
            );
            frame.render_widget(Paragraph::new(lines).style(panel_style()), area);
        }
        return;
    }

    let query = (!app.atlas_view.query.is_empty()).then_some(app.atlas_view.query.as_str());
    let items = app.atlas.list(app.atlas_view.lane, query);
    if items.is_empty() {
        let message = match app.atlas_view.lane {
            crate::knowledge::atlas::AtlasLane::Review => {
                "Review is clear.\nModel proposals remain inert until accepted."
            }
            crate::knowledge::atlas::AtlasLane::Shared => {
                "No explicitly promoted shared knowledge.\nSharing is never automatic."
            }
            _ if query.is_some() => "No Atlas items match this search.",
            _ => "No reviewed Atlas knowledge in this lane.",
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(dim_panel_style())
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    app.atlas_view.selected = app.atlas_view.selected.min(items.len().saturating_sub(1));
    let selected = app.atlas_view.selected;
    let list = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let marker = if index == selected { "▸" } else { " " };
            let state = format!("{:?}", item.lifecycle).to_ascii_lowercase();
            let content = truncate_control_value(
                &item.content.replace('\n', " "),
                area.width.saturating_sub(20) as usize,
            );
            Line::from(vec![
                Span::styled(
                    format!("{marker} {:<11}", item.kind.label()),
                    if index == selected {
                        Style::new()
                            .fg(crate::ui::hud::HUD_BLUE)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::new().fg(crate::ui::hud::HUD_DIM)
                    },
                ),
                Span::raw(content),
                Span::styled(
                    format!(" · {state}"),
                    Style::new().fg(crate::ui::hud::HUD_DIM),
                ),
            ])
        })
        .collect::<Vec<_>>();
    let detail = atlas_detail_lines(&items[selected], query);

    if area.height < 7 {
        frame.render_widget(Paragraph::new(list).style(panel_style()), area);
    } else if area.width >= 70 {
        let columns = Layout::horizontal([Constraint::Percentage(43), Constraint::Percentage(57)])
            .split(area);
        frame.render_widget(
            Paragraph::new(list)
                .block(hud_block(" Index "))
                .style(panel_style()),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(detail)
                .block(hud_block(" Evidence & policy "))
                .style(panel_style())
                .wrap(Wrap { trim: false }),
            columns[1],
        );
    } else {
        let list_height = (area.height / 2).max(3);
        let rows =
            Layout::vertical([Constraint::Length(list_height), Constraint::Min(0)]).split(area);
        frame.render_widget(Paragraph::new(list).style(panel_style()), rows[0]);
        frame.render_widget(
            Paragraph::new(detail)
                .style(panel_style())
                .wrap(Wrap { trim: false }),
            rows[1],
        );
    }
}

fn atlas_detail_lines(
    item: &crate::knowledge::atlas::AtlasItem,
    query: Option<&str>,
) -> Vec<Line<'static>> {
    let age_days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().saturating_sub(item.updated_ms as u128) / 86_400_000)
        .unwrap_or(0);
    let trust = format!(
        "{:?} / {:?}{}{}",
        item.authority,
        item.epistemic,
        if item.contested { " / contested" } else { "" },
        if item.stale { " / stale" } else { "" }
    )
    .to_ascii_lowercase();
    let sources = if item.sources.is_empty() {
        "none".to_string()
    } else {
        item.sources
            .iter()
            .map(|source| {
                format!(
                    "{}:{}{}",
                    source.kind,
                    source.id,
                    if source.independent {
                        ""
                    } else {
                        " (influenced/non-independent)"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let relationships = if item.links.is_empty() {
        "none".to_string()
    } else {
        item.links
            .iter()
            .map(|link| format!("{:?}→{}", link.kind, link.target_id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let why = query
        .map(|query| format!("matched search {:?}", query))
        .unwrap_or_else(|| {
            format!(
                "selected from {} lane",
                format!("{:?}", item.scope).to_ascii_lowercase()
            )
        });
    vec![
        Line::from(Span::styled(
            item.content.clone(),
            Style::new()
                .fg(crate::ui::hud::HUD_TEXT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("id       {}", item.id)),
        Line::from(format!("scope    {:?} · kind {:?}", item.scope, item.kind)),
        Line::from(format!("trust    {trust}")),
        Line::from(format!("fresh    updated {age_days}d ago")),
        Line::from(format!("source   {sources}")),
        Line::from(format!("links    {relationships}")),
        Line::from(format!(
            "inject   {:?} (confidence ranks only: {})",
            item.injection,
            item.confidence
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "n/a".to_string())
        )),
        Line::from(format!("why      {why}")),
    ]
}

/// Render the persistent native Scryglass. The module id remains `artifacts` for
/// layout compatibility, but the visible product surface is the Scryglass.
fn render_scryglass(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    resolved: crate::ui::scryglass::StageSurface,
) {
    let active_index = app.scryglass.active_media();
    let live_title = scryglass_live_world_title_allowed();

    let focused = app
        .module_host
        .focused()
        .is_some_and(|id| id.as_str() == "artifacts");
    let route_kind = match resolved {
        crate::ui::scryglass::StageSurface::Lesson => "TEACHING WINDOW",
        crate::ui::scryglass::StageSurface::Catalog => "LIVING CATALOG",
        crate::ui::scryglass::StageSurface::Arrival(_) => {
            crate::stage::identity::STAGE_TITLE_ARRIVAL
        }
        // Z5: off Castle Town the two world surfaces are one place — the
        // quest — and they say so together. The region itself is already the
        // head of `World::title()`, so the kind only has to name the mode.
        crate::ui::scryglass::StageSurface::WorldFirstPerson
        | crate::ui::scryglass::StageSurface::WorldMap
            if app.world.quest_owns_pane() =>
        {
            crate::stage::identity::STAGE_TITLE_QUEST
        }
        crate::ui::scryglass::StageSurface::WorldFirstPerson => {
            crate::stage::identity::STAGE_TITLE_EXPLORE
        }
        _ => crate::stage::identity::STAGE_TITLE_REALM,
    };
    // Live titles clone the media label and build world.title() (town / quest /
    // ward / activity / renown). Comp / lean keeps the route identity only.
    let (kind, media_label, is_video) = if active_index.is_some() {
        active_index
            .and_then(|index| {
                app.media.get(index).map(|media| {
                    (
                        if media.is_video() {
                            "REEL"
                        } else if media.is_visual() {
                            "RELIC"
                        } else {
                            "DOCUMENT"
                        },
                        Some(media.label().to_string()),
                        media.is_video(),
                    )
                })
            })
            .unwrap_or((route_kind, None, false))
    } else {
        (route_kind, None, false)
    };
    let queued = if live_title {
        match app.scryglass.pending_count() {
            0 => String::new(),
            count => format!(" · Q{count}"),
        }
    } else {
        String::new()
    };
    let memory_warning = matches!(
        resolved,
        crate::ui::scryglass::StageSurface::WorldMap
            | crate::ui::scryglass::StageSurface::WorldFirstPerson
    ) && app.world.memory_health()
        == crate::knowledge::memory::store::MemoryHealth::Degraded;
    let title = if memory_warning {
        " Memory degraded ".to_string()
    } else if let crate::ui::scryglass::StageSurface::Lesson = resolved {
        let term = app
            .scryglass
            .lesson()
            .map(crate::ui::term::lookup::QuickLookup::term)
            .unwrap_or("concept");
        format!(" {term} ")
    } else if matches!(resolved, crate::ui::scryglass::StageSurface::Catalog) {
        String::new()
    } else {
        scryglass_route_title(kind, None, &queued)
    };
    // Block titles otherwise hard-clip mid-word at the terminal edge. Preserve
    // the Scryglass identity and the differentiating route/place tail with the
    // same display-cell-aware middle ellipsis used by model controls.
    let title = truncate_control_value(&title, area.width.saturating_sub(2) as usize);
    // Realm and Explore share Dotmax; both receive quest chrome.
    let world_pane = matches!(
        resolved,
        crate::ui::scryglass::StageSurface::WorldMap
            | crate::ui::scryglass::StageSurface::WorldFirstPerson
    );
    let _media_caption = media_label.is_some();
    // The composer shares and repaints our bottom border. Keep reset on the
    // top edge, with space reserved so the place title cannot cover the button.
    let camera_title = world_pane.then(|| " [Follow] ".to_string());
    let title_budget = area.width.saturating_sub(
        camera_title
            .as_ref()
            .map_or(2, |label| label.len() as u16 + 6),
    );
    let title = if world_pane
        && unicode_width::UnicodeWidthStr::width(title.as_str()) > usize::from(title_budget)
    {
        scryglass_route_title(kind, None, &queued)
    } else {
        title
    };
    let title = truncate_control_value(&title, usize::from(title_budget));
    let mut block = hud_block(title);
    if let Some(label) = &camera_title {
        block = block.title_top(Line::from(label.clone()).alignment(Alignment::Right));
    }
    let quest_tint = world_pane.then(|| app.world.quest_border_style()).flatten();
    match (focused, quest_tint) {
        (true, Some(style)) => {
            block = block.border_style(style.add_modifier(Modifier::BOLD));
        }
        (true, None) => block = block.border_style(HUD_BLUE_BORDER_STYLE),
        (false, Some(style)) => block = block.border_style(style),
        (false, None) => {}
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.world_buttons.clear();
    if let Some(label) = &camera_title
        && area.width >= label.len() as u16 + 2
        && area.height >= 2
    {
        app.world_buttons.push((
            Rect::new(area.right() - label.len() as u16, area.y, 8, 1),
            WorldButton::ScryglassFollow,
        ));
    }

    if inner.width == 0 || inner.height == 0 {
        app.scryglass.set_stage_visibility(true, false);
        app.viewer.clear_still();
        return;
    }
    let formation_h = 0;
    let footer_h = u16::from(inner.height >= 2);
    let scene_h = inner.height.saturating_sub(formation_h + footer_h);
    let mut scene_rect = Rect::new(inner.x, inner.y, inner.width, scene_h);
    if let Some(media) = active_index.and_then(|index| app.media.get(index)) {
        let identity_height = scene_rect.height.min(2);
        let identity_rect = Rect::new(
            scene_rect.x,
            scene_rect.y,
            scene_rect.width,
            identity_height,
        );
        let source = media.target();
        let identity = vec![
            Line::from(format!(
                "{} · {}",
                media.sigil(),
                media.label().escape_debug()
            )),
            Line::from(format!(
                "Source: {}",
                truncate_control_value(
                    &source.escape_debug().to_string(),
                    scene_rect.width.saturating_sub(8) as usize
                )
            )),
        ];
        frame.render_widget(
            Paragraph::new(identity).style(Style::new().fg(crate::ui::hud::HUD_TEXT)),
            identity_rect,
        );
        scene_rect.y += identity_height;
        scene_rect.height = scene_rect.height.saturating_sub(identity_height);
    }
    let footer_rect = Rect::new(inner.x, inner.y + scene_h, inner.width, footer_h);
    let formation_rect =
        (formation_h > 0).then(|| Rect::new(inner.x, footer_rect.y + footer_h, inner.width, 1));
    let renderable =
        scene_rect.width >= 20 && scene_rect.height >= if active_index.is_some() { 3 } else { 7 };
    app.scryglass.set_stage_visibility(true, renderable);
    // This pane hosts every Scryglass surface, not only the Realm branches
    // below. Arrival and media reveals also need the visible-stage cadence;
    // leaving this false made their animation timers stall on screen.
    app.world_pane_visible = renderable;
    let mut lesson_scroll_max = 0u16;

    let mut video_ended = false;
    let mut video_paused = false;
    let mut visual_loading = false;
    let mut video_position = Duration::ZERO;
    let mut video_duration = None;
    if !renderable {
        app.viewer.clear_still();
        frame.render_widget(
            Paragraph::new("Scryglass needs 20×7 cells\nF4 opens the full stage")
                .style(dim_panel_style()),
            scene_rect,
        );
    } else if !scryglass_scene_body_allowed() && active_index.is_none() {
        // Comp / lean: keep the route chrome, skip lesson wrap, catalog
        // listing, and still/video decode. World map/ride already share
        // maybe_paint_world_scene; this gate avoids entering those bodies.
    } else if world_pane
        && !app.world.inside_interior()
        && !app.world.quest_owns_pane()
        && app.world.latest_active_work().is_none()
        && app.startup_intro.render_miniviz(
            frame,
            scene_rect,
            app.viewer.dot_geometry(scene_rect),
            app.visual_motion,
        )
    {
        // Startup owns only the world body; keep the pane's chrome/footer and
        // let explicit media, lessons and other selected surfaces take priority.
        app.world_pane_visible = false;
    } else if matches!(resolved, crate::ui::scryglass::StageSurface::Lesson) {
        app.world_pane_visible = true;
        if app.scryglass.lesson().is_some() {
            let lesson_block = Block::default()
                .borders(Borders::ALL)
                .border_style(HUD_BLUE_BORDER_STYLE)
                .title(
                    Line::from(" WORLD LESSON · STEM / COMPUTING ")
                        .style(Style::new().fg(crate::ui::hud::HUD_BLUE)),
                );
            let lesson_inner = lesson_block.inner(scene_rect);
            frame.render_widget(lesson_block, scene_rect);
            if lesson_inner.width > 0 && lesson_inner.height > 0 {
                // Wrap key: inner width, `trim: false`, visible-text len + fingerprint.
                const WRAP_TRIM: bool = false;
                let wrap_width = lesson_inner.width;
                let total_rows = {
                    let lesson_text = app
                        .scryglass
                        .lesson()
                        .map(crate::ui::term::lookup::QuickLookup::visible_text)
                        .unwrap_or("");
                    app.lesson_wrap
                        .wrapped_lines(wrap_width, WRAP_TRIM, lesson_text, || {
                            Paragraph::new(lesson_text)
                                .wrap(Wrap { trim: WRAP_TRIM })
                                .line_count(wrap_width)
                                .min(u16::MAX as usize) as u16
                        })
                };
                lesson_scroll_max = total_rows.saturating_sub(lesson_inner.height);
                let lesson_scroll = app.scryglass.clamp_lesson_scroll(lesson_scroll_max);
                if let Some(lesson_text) = app
                    .scryglass
                    .lesson()
                    .map(crate::ui::term::lookup::QuickLookup::visible_text)
                {
                    frame.render_widget(
                        Paragraph::new(lesson_text)
                            .wrap(Wrap { trim: WRAP_TRIM })
                            .style(PHOSPHOR_BOLD_STYLE)
                            .scroll((lesson_scroll, 0)),
                        lesson_inner,
                    );
                }
            }
        }
    } else if matches!(resolved, crate::ui::scryglass::StageSurface::Catalog) {
        app.world_pane_visible = true;
        let catalog_block = Block::default()
            .borders(Borders::ALL)
            .border_style(HUD_BLUE_BORDER_STYLE)
            .title(
                Line::from(" SCRIPTORIUM CATALOG · RESIDENTS / SHELVES ")
                    .style(Style::new().fg(crate::ui::hud::HUD_BLUE)),
            );
        let catalog_inner = catalog_block.inner(scene_rect);
        frame.render_widget(catalog_block, scene_rect);
        if catalog_inner.width > 0 && catalog_inner.height > 0 {
            let selected = app.scryglass.catalog_selection();
            let shelf = app.scryglass.selected_shelf();
            let tutor = crate::knowledge::library::tutor_for(shelf.discipline);
            if catalog_inner.width >= 68 && catalog_inner.height >= 8 {
                let columns =
                    Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)])
                        .split(catalog_inner);
                let list_area = columns[0];
                let detail_area = columns[1];
                let visible = list_area.height as usize;
                let start = selected.saturating_sub(visible / 2).min(
                    crate::knowledge::library::catalog_shelves()
                        .len()
                        .saturating_sub(visible),
                );
                let lines = crate::knowledge::library::catalog_shelves()
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take(visible)
                    .map(|(index, item)| {
                        let active = index == selected;
                        Line::from(vec![
                            Span::styled(
                                if active { "› " } else { "  " },
                                Style::new().fg(if active {
                                    crate::ui::hud::HUD_BLUE
                                } else {
                                    crate::ui::hud::HUD_DIM
                                }),
                            ),
                            Span::styled(
                                format!("{:02} · {}", index + 1, item.title),
                                Style::new()
                                    .fg(if active {
                                        crate::ui::hud::HUD_TEXT
                                    } else {
                                        crate::ui::hud::HUD_DIM
                                    })
                                    .add_modifier(if active {
                                        Modifier::BOLD
                                    } else {
                                        Modifier::empty()
                                    }),
                            ),
                        ])
                    })
                    .collect::<Vec<_>>();
                frame.render_widget(Paragraph::new(lines), list_area);

                let detail = vec![
                    Line::from(Span::styled(
                        shelf.title,
                        Style::new()
                            .fg(crate::ui::hud::HUD_TEXT)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        crate::knowledge::library::discipline_label(shelf.discipline),
                        Style::new().fg(crate::ui::hud::HUD_BLUE),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("RESIDENT  ", Style::new().fg(crate::ui::hud::HUD_BLUE)),
                        Span::styled(tutor.name, Style::new().fg(crate::ui::hud::HUD_TEXT)),
                    ]),
                    Line::from(Span::styled(
                        tutor.role,
                        Style::new().fg(crate::ui::hud::HUD_DIM),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("METHOD    ", Style::new().fg(crate::ui::hud::HUD_GOLD)),
                        Span::styled(tutor.method, Style::new().fg(crate::ui::hud::HUD_TEXT)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("SOURCE    ", Style::new().fg(crate::ui::hud::HUD_GOLD)),
                        Span::styled(shelf.source, Style::new().fg(crate::ui::hud::HUD_TEXT)),
                    ]),
                    Line::from(Span::styled(
                        shelf.url,
                        Style::new().fg(crate::ui::hud::HUD_DIM),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("TRAIL     ", Style::new().fg(crate::ui::hud::HUD_BLUE)),
                        Span::styled(
                            shelf.arc.join(" → "),
                            Style::new().fg(crate::ui::hud::HUD_TEXT),
                        ),
                    ]),
                ];
                frame.render_widget(
                    Paragraph::new(detail).wrap(Wrap { trim: false }),
                    detail_area,
                );
            } else {
                let detail = format!(
                    "{:02}/{:02} · {}\n{}\n\nResident · {} — {}\nMethod · {}\n\nSuggested curriculum · {}\nCourse · {}\nTrail · {}\n\n↑/↓ choose · Enter study",
                    selected + 1,
                    crate::knowledge::library::catalog_shelves().len(),
                    shelf.title,
                    crate::knowledge::library::discipline_label(shelf.discipline),
                    tutor.name,
                    tutor.role,
                    tutor.method,
                    shelf.source,
                    shelf.url,
                    shelf.arc.join(" → ")
                );
                frame.render_widget(
                    Paragraph::new(detail)
                        .wrap(Wrap { trim: false })
                        .style(PHOSPHOR_BOLD_STYLE),
                    catalog_inner,
                );
            }
        }
    } else if let crate::ui::scryglass::StageSurface::Document(index) = resolved {
        match app.scryglass.document_frame(&app.media[index]) {
            Ok(Some(document)) => {
                let receipt_rect = Rect::new(scene_rect.x, scene_rect.y, scene_rect.width, 1);
                frame.render_widget(
                    Paragraph::new(document.receipt.as_str()).style(dim_panel_style()),
                    receipt_rect,
                );
                let body = Rect::new(
                    scene_rect.x,
                    scene_rect.y + 1,
                    scene_rect.width,
                    scene_rect.height.saturating_sub(1),
                );
                let paragraph = Paragraph::new(document.text.as_str()).wrap(Wrap { trim: false });
                let scroll = app
                    .scryglass
                    .document_scroll(body.width, body.height, || paragraph.line_count(body.width));
                frame.render_widget(
                    paragraph
                        .scroll((scroll, 0))
                        .style(Style::new().fg(crate::ui::hud::HUD_TEXT)),
                    body,
                );
                app.scryglass.note_media_painted();
            }
            Ok(None) => frame.render_widget(
                Paragraph::new("Loading requested document…").style(dim_panel_style()),
                scene_rect,
            ),
            Err(error) => {
                app.scryglass.note_media_fault(error.clone());
                frame.render_widget(
                    Paragraph::new(error)
                        .wrap(Wrap { trim: false })
                        .style(Style::new().fg(crate::ui::hud::HUD_DANGER)),
                    scene_rect,
                );
            }
        }
    } else if matches!(
        resolved,
        crate::ui::scryglass::StageSurface::Still(_)
            | crate::ui::scryglass::StageSurface::Video(_)
            | crate::ui::scryglass::StageSurface::Fault
    ) {
        let Some(index) = active_index else {
            return;
        };
        if let Some(error) = app.scryglass.active_error().map(str::to_string) {
            app.scryglass.surface = crate::ui::scryglass::StageSurface::Fault;
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "REQUESTED ARTIFACT UNAVAILABLE",
                        Style::new().fg(crate::ui::hud::HUD_GOLD),
                    )),
                    Line::from(Span::styled(
                        error,
                        Style::new().fg(crate::ui::hud::HUD_DANGER),
                    )),
                ])
                .wrap(Wrap { trim: false }),
                scene_rect,
            );
        } else if is_video {
            app.scryglass.surface = crate::ui::scryglass::StageSurface::Video(index);
            let (decode_width, decode_height) = app.viewer.video_decode_viewport(scene_rect);
            let result = {
                let media = &app.media[index];
                app.scryglass.video_frame(
                    media,
                    decode_width,
                    decode_height,
                    match app.visual_motion {
                        crate::ui::viz::lifecycle_viz::MotionMode::Full => 12,
                        crate::ui::viz::lifecycle_viz::MotionMode::Reduced => 6,
                        crate::ui::viz::lifecycle_viz::MotionMode::Off => 1,
                    },
                    app.visual_motion != crate::ui::viz::lifecycle_viz::MotionMode::Off,
                )
            };
            match result {
                Ok((Some(image), position, duration, paused, ended)) => {
                    video_position = position;
                    video_duration = duration;
                    video_paused = paused;
                    video_ended = ended;
                    match app.viewer.render_video(frame, scene_rect, image) {
                        Ok(true) => app.scryglass.note_media_painted(),
                        Ok(false) => {
                            visual_loading = true;
                            crate::ui::scryglass::Scryglass::paint_loading(
                                frame,
                                scene_rect,
                                app.started.elapsed().as_millis() as u64 / 90,
                            );
                        }
                        Err(error) => app.scryglass.note_media_fault(error),
                    }
                }
                Ok((None, position, duration, paused, ended)) => {
                    visual_loading = true;
                    video_position = position;
                    video_duration = duration;
                    video_paused = paused;
                    video_ended = ended;
                    crate::ui::scryglass::Scryglass::paint_loading(
                        frame,
                        scene_rect,
                        app.started.elapsed().as_millis() as u64 / 90,
                    );
                }
                Err(error) => app.scryglass.note_media_fault(error),
            }
        } else {
            let result = app.media[index]
                .source()
                .ok_or_else(|| "requested image has no local source".to_string())
                .and_then(|source| {
                    app.viewer.render_artifact(
                        frame,
                        scene_rect,
                        source,
                        app.scryglass.media_request_id(),
                    )
                });
            match result {
                Ok(true) => {
                    app.scryglass.surface = crate::ui::scryglass::StageSurface::Still(index);
                    app.scryglass.note_media_painted();
                }
                Ok(false) => {
                    visual_loading = true;
                    app.scryglass.surface = crate::ui::scryglass::StageSurface::Still(index);
                    if app.viewer.inspector.viewport.is_none() {
                        crate::ui::scryglass::Scryglass::paint_loading(
                            frame,
                            scene_rect,
                            app.started.elapsed().as_millis() as u64 / 90,
                        );
                    }
                }
                Err(error) => {
                    app.viewer.clear_still();
                    app.scryglass.note_media_fault(error);
                }
            }
        }
    } else if app.world.ambient_interior_visible()
        && matches!(
            resolved,
            crate::ui::scryglass::StageSurface::WorldMap
                | crate::ui::scryglass::StageSurface::WorldFirstPerson
                | crate::ui::scryglass::StageSurface::Arrival(_)
        )
    {
        app.world_pane_visible = true;
        let (width, height) = world_sample_size(app, scene_rect);
        if !render_dotmax_interior(frame, app, scene_rect)
            && let Some(world) = maybe_paint_world_scene(resolved, || {
                app.world
                    .ambient_braille_frame(width, height, app.visual_motion)
            })
            .flatten()
        {
            paint_world_frame(frame, app, scene_rect, &world);
        }
    } else if let crate::ui::scryglass::StageSurface::Arrival(building) = resolved {
        app.scryglass.surface = crate::ui::scryglass::StageSurface::Arrival(building);
        if !render_dotmax_interior(frame, app, scene_rect) {
            let (width, height) = world_sample_size(app, scene_rect);
            let yaw = if app.scryglass.follow_agent {
                app.world_yaw_offset
            } else {
                app.scryglass.look_yaw + app.world_yaw_offset
            };
            if let Some(world) = maybe_paint_world_scene(resolved, || {
                app.world.scryglass_frame_paced(
                    width,
                    height,
                    app.scenery_relaxed(),
                    yaw,
                    app.scryglass.look_pitch,
                    app.scryglass.fov,
                )
            })
            .flatten()
            {
                paint_world_frame(frame, app, scene_rect, &world);
            }
        }
    } else {
        app.world_pane_visible = true;
        match resolved {
            crate::ui::scryglass::StageSurface::WorldMap => {
                render_world_map_surface(frame, app, scene_rect);
            }
            // Persistent Explore, Journey, and every interior are the live
            // world.  The backed vista is only an Arrival establishing shot;
            // using it here made camera, weather, residents, and room state
            // disappear on image-capable terminals.
            crate::ui::scryglass::StageSurface::WorldFirstPerson => {
                let (width, height) = world_sample_size(app, scene_rect);
                let yaw = if app.scryglass.follow_agent {
                    app.world_yaw_offset
                } else {
                    app.scryglass.look_yaw + app.world_yaw_offset
                };
                if let Some(world) = maybe_paint_world_scene(resolved, || {
                    app.world.scryglass_frame_paced(
                        width,
                        height,
                        app.scenery_relaxed(),
                        yaw,
                        app.scryglass.look_pitch,
                        app.scryglass.fov,
                    )
                })
                .flatten()
                {
                    paint_world_frame(frame, app, scene_rect, &world);
                }
            }
            _ => {}
        }
    }

    if footer_h > 0 && (scryglass_scene_accessories_allowed() || active_index.is_some()) {
        let (caption, controls): (Line<'static>, Vec<(&'static str, WorldButton)>) =
            if matches!(resolved, crate::ui::scryglass::StageSurface::Lesson) {
                let lesson_status = if lesson_scroll_max > 0 {
                    "↕ scroll · not model reasoning"
                } else {
                    "local · not model reasoning"
                };
                (
                    Line::from(vec![
                        Span::styled("◈ ", Style::new().fg(crate::ui::hud::HUD_BLUE)),
                        Span::styled(lesson_status, Style::new().fg(crate::ui::hud::HUD_TEXT)),
                    ]),
                    vec![
                        ("Ask Tutor", WorldButton::ScryglassAskTutor),
                        ("Source", WorldButton::ScryglassCopySource),
                        ("Back", WorldButton::Back),
                    ],
                )
            } else if matches!(resolved, crate::ui::scryglass::StageSurface::Catalog) {
                let catalog_status = format!(
                    "{:02}/{:02} · {} residents · arrows choose · Enter study",
                    app.scryglass.catalog_selection() + 1,
                    crate::knowledge::library::catalog_shelves().len(),
                    crate::knowledge::library::catalog_tutors().len()
                );
                (
                    Line::from(vec![
                        Span::styled("⌂ ", Style::new().fg(crate::ui::hud::HUD_GOLD)),
                        Span::styled(catalog_status, Style::new().fg(crate::ui::hud::HUD_TEXT)),
                    ]),
                    vec![
                        ("Study", WorldButton::ScryglassStudy),
                        ("Ask Tutor", WorldButton::ScryglassAskTutor),
                        ("Back", WorldButton::Back),
                    ],
                )
            } else if let Some(_label) = media_label {
                let video_meta = is_video.then(|| {
                    format!(
                        "  {} / {}",
                        compact_media_time(video_position),
                        video_duration
                            .map(compact_media_time)
                            .unwrap_or_else(|| "--:--".to_string())
                    )
                });
                let mut spans = vec![
                    Span::styled("▌ ", Style::new().fg(crate::ui::hud::HUD_GOLD)),
                    Span::styled(
                        if matches!(resolved, crate::ui::scryglass::StageSurface::Document(_)) {
                            "↑↓ PgUp/PgDn scroll".to_string()
                        } else if app.scryglass.active_error().is_some() {
                            "Unavailable".to_string()
                        } else if !is_video && app.viewer.inspector.viewport.is_some() {
                            app.viewer.inspector.status_label()
                        } else if visual_loading || !app.scryglass.media_ready() {
                            "Loading".to_string()
                        } else if is_video {
                            if video_ended {
                                "Ended · r restart"
                            } else if video_paused {
                                "Paused"
                            } else {
                                "Playing"
                            }
                            .to_string()
                        } else {
                            app.viewer.inspector.status_label()
                        },
                        Style::new()
                            .fg(crate::ui::hud::HUD_TEXT)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];
                if let Some(meta) = video_meta {
                    spans.push(Span::styled(meta, Style::new().fg(crate::ui::hud::HUD_DIM)));
                }
                let controls = if is_video {
                    vec![
                        ("-5", WorldButton::ScryglassVideoBack),
                        (
                            if video_ended {
                                "Replay"
                            } else if video_paused {
                                "Play"
                            } else {
                                "Pause"
                            },
                            WorldButton::ScryglassVideoToggle,
                        ),
                        ("+5", WorldButton::ScryglassVideoForward),
                        ("World", WorldButton::ScryglassWorld),
                        ("Back", WorldButton::Back),
                    ]
                } else {
                    vec![
                        (
                            "-",
                            WorldButton::Still(crate::ui::still_inspector::Action::ZoomOut),
                        ),
                        (
                            "+",
                            WorldButton::Still(crate::ui::still_inspector::Action::ZoomIn),
                        ),
                        (
                            "Fit",
                            WorldButton::Still(crate::ui::still_inspector::Action::Fit),
                        ),
                        ("Back", WorldButton::Back),
                    ]
                };
                (Line::from(spans), controls)
            } else if app.world.inside_interior() {
                let mut controls = Vec::new();
                if app.world.interior_building()
                    == Some(crate::stage::world_viz::Building::Scriptorium)
                {
                    controls.push(("Catalog", WorldButton::ScryglassCatalog));
                }
                controls.extend([
                    ("Leave", WorldButton::ScryglassLeave),
                    ("Back", WorldButton::Back),
                ]);
                (Line::from(""), controls)
            } else if app.scryglass.arrival().is_some() {
                let mut controls = Vec::new();
                if app.world.has_authored_interior() {
                    controls.push(("Enter", WorldButton::ScryglassEnter));
                }
                controls.extend([
                    ("Explore", WorldButton::ScryglassMap),
                    ("Back", WorldButton::Back),
                ]);
                (Line::from(""), controls)
            } else {
                let exploring = matches!(
                    resolved,
                    crate::ui::scryglass::StageSurface::WorldFirstPerson
                );
                let view = if exploring { "Map" } else { "Explore" };
                let mut controls = Vec::new();
                if !app.world.riding() && app.world.has_authored_interior() {
                    controls.push(("Enter", WorldButton::ScryglassEnter));
                }
                controls.extend([
                    (view, WorldButton::ScryglassMap),
                    ("Library", WorldButton::ScryglassLibrary),
                    ("Vault", WorldButton::ScryglassVault),
                    ("Back", WorldButton::Back),
                ]);
                // Compact Realm captions carry causal and memory-health
                // evidence. Keep their existing space; `r` and `/research`
                // remain available when the extra navigation verb cannot fit.
                if footer_rect.width >= 96 {
                    controls.insert(
                        controls.len() - 1,
                        (
                            "Research",
                            WorldButton::Research(crate::drive::research_workspace::Action::Place(
                                crate::drive::research_workspace::Place::Keep,
                            )),
                        ),
                    );
                }
                if fitted_scene_verbs(footer_rect.width, &controls).0.len() != controls.len() {
                    // The ordinary 144-column cockpit can be one or two cells
                    // short of the complete action rail when Enter is present.
                    // Preserve the world-changing teaching/navigation actions;
                    // camera sugar and Vault return at wider breakpoints.
                    controls.retain(|(_, button)| {
                        matches!(
                            button,
                            WorldButton::ScryglassEnter
                                | WorldButton::ScryglassMap
                                | WorldButton::ScryglassLibrary
                                | WorldButton::ScryglassFollow
                                | WorldButton::Research(_)
                                | WorldButton::Back
                        )
                    });
                }
                (Line::from(""), controls)
            };
        render_scene_caption(frame, app, footer_rect, caption, &controls);
    }
    if let Some(formation_rect) = formation_rect
        && scryglass_scene_accessories_allowed()
    {
        let line = formation_status_bar(app, formation_rect);
        frame.render_widget(Paragraph::new(line), formation_rect);
    }
    if scryglass_scene_accessories_allowed() || active_index.is_some() {
        app.scryglass
            .tick_visible(Instant::now(), is_video, video_ended);
    }
}

fn compact_media_time(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/draw__stage_view__artifact_stage_tests.rs"]
mod artifact_stage_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/draw__stage_view__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/draw__stage_view__world_presentation_regressions.rs"]
mod world_presentation_regressions;
