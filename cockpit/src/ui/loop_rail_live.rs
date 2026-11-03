//! The rail's only connection to the cockpit: observe, draw, never drive.
use super::loop_rail::{Activity, Display, Snapshot, State};
use crate::{App, drive::loop_ctl::LoopStatus};
use ratatui::layout::{Margin, Rect};
use std::time::Instant;

#[cfg(feature = "glass")]
const GROUP: u32 = 0x5241_494C;
#[cfg(feature = "glass")]
const PICTURE: u32 = angel_glass::LAYER_PICTURES + 0x5241;

pub(crate) struct Live {
    pub display: Display,
    state: State,
    origin: Instant,
    visible: bool,
}

impl Default for Live {
    fn default() -> Self {
        Self {
            display: Display::default(),
            state: State::default(),
            origin: Instant::now(),
            visible: false,
        }
    }
}

impl Live {
    pub(crate) fn animating(&self, focused: bool) -> bool {
        self.visible && focused
    }
}

fn snapshot(app: &App) -> Option<Snapshot> {
    let l = &app.loop_ctl;
    let activity = match l.status {
        LoopStatus::Idle => return None,
        LoopStatus::Done | LoopStatus::Stopped | LoopStatus::Failed => Activity::Finished,
        LoopStatus::AwaitingApproval | LoopStatus::Paused => Activity::Operator,
        _ if app.pending_approval.is_some() => Activity::Operator,
        _ if app.thinking.is_some()
            || app.pending_turn.is_some()
            || app.bg_job.is_some()
            || matches!(l.status, LoopStatus::Baselining | LoopStatus::Verifying) =>
        {
            Activity::Working
        }
        _ => Activity::Waiting,
    };
    let done = l.iteration;
    Some(Snapshot {
        run: l.started_ms,
        done,
        activity,
    })
}

pub(crate) fn frame(app: &mut App, root: Rect) -> Rect {
    let allowed = app.loop_rail.display.allows(
        super::glass::active(),
        app.visual_motion == super::viz::lifecycle_viz::MotionMode::Full,
        root.width,
        root.height,
    ) && super::glass::cell_pixels().is_some_and(|(w, h)| w >= 8 && h >= 16);
    if !allowed {
        app.loop_rail.visible = false;
        app.loop_rail.state = State::default();
        return root;
    }
    #[cfg(feature = "glass")]
    let path = {
        let g = super::glass::host()
            .expect("allowed glass rail")
            .grid_size();
        let band = (2 * g.cell_width).min(g.cell_height).min(20) as f32;
        super::loop_rail::Path {
            width: g.width_px as f32,
            height: g.height_px as f32,
            inset: band * 0.4,
            radius: band * 0.8,
        }
    };
    #[cfg(feature = "glass")]
    let pace = path.pace(app.loop_rail.state.turns);
    #[cfg(not(feature = "glass"))]
    let pace = 1.0;
    let snap = snapshot(app);
    let now = app.loop_rail.origin.elapsed().as_secs_f64();
    let visible = app.loop_rail.state.step_paced(snap, now, pace);
    app.loop_rail.visible = visible;
    // Unclaimed groups are swept once by frame_done; idle frames send nothing.
    if !visible {
        return root;
    }
    #[cfg(feature = "glass")]
    if let Some(host) = super::glass::host() {
        let mut layers = Vec::new();
        if visible {
            use super::loop_rail::art;
            use angel_glass::{Layer, LayerKind, Placement};
            // Hairline only: no track backdrop, shadow, or panel of its own.
            let w = path.width;
            let h = path.height;
            for (i, place) in [
                [0.0, 0.0, w, 1.0],
                [w - 1.0, 0.0, 1.0, h],
                [0.0, h - 1.0, w, 1.0],
                [0.0, 0.0, 1.0, h],
            ]
            .into_iter()
            .enumerate()
            {
                layers.push(Layer::new(
                    GROUP as u64 + i as u64,
                    7,
                    Placement::Pixels(place),
                    LayerKind::Solid([0.39, 0.41, 0.42, 0.28]),
                ));
            }
            let activity = snap.map_or(Activity::Finished, |s| s.activity);
            let tile = art::tile(
                path,
                app.loop_rail.state.turns,
                now as f32,
                app.loop_rail.state.speed,
                activity,
            );
            let place = [
                tile.x as f32,
                tile.y as f32,
                tile.side as f32,
                tile.side as f32,
            ];
            super::glass::upload_picture(PICTURE, (tile.side, tile.side), tile.rgba, (0, 0));
            layers.push(Layer::new(
                GROUP as u64 + 4,
                8,
                Placement::Pixels(place),
                LayerKind::Picture(PICTURE),
            ));
        }
        super::glass::show_group(host, GROUP, layers);
    }
    root.inner(Margin {
        horizontal: 2,
        vertical: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_rail_live_snapshot_matches_the_tickers_iterations_and_activity() {
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        assert!(snapshot(&app).is_none());
        assert!(!app.loop_rail.animating(true));
        app.loop_ctl.started_ms = 123;
        app.loop_ctl.iteration = 7;
        app.loop_ctl.status = LoopStatus::Running;
        let waiting = snapshot(&app).unwrap();
        assert_eq!((waiting.run, waiting.done), (123, 7));
        assert_eq!(waiting.activity, Activity::Waiting);
        app.loop_ctl.status = LoopStatus::Verifying;
        assert_eq!(snapshot(&app).unwrap().activity, Activity::Working);
        app.loop_ctl.status = LoopStatus::Baselining;
        assert_eq!(snapshot(&app).unwrap().activity, Activity::Working);
        for status in [LoopStatus::Paused, LoopStatus::AwaitingApproval] {
            app.loop_ctl.status = status;
            assert_eq!(snapshot(&app).unwrap().activity, Activity::Operator);
        }
        for status in [LoopStatus::Done, LoopStatus::Stopped, LoopStatus::Failed] {
            app.loop_ctl.status = status;
            let s = snapshot(&app).unwrap();
            assert_eq!(s.activity, Activity::Finished);
            assert_eq!(s.done, 7);
        }
    }

    #[test]
    fn loop_rail_live_leaves_plain_terminal_geometry_and_authority_unchanged() {
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        let root = Rect::new(0, 0, 150, 44);
        app.loop_ctl.status = LoopStatus::Running;
        app.loop_ctl.iteration = 3;
        app.loop_ctl.started_ms = 42;
        let before = serde_json::to_value(&app.loop_ctl).unwrap();
        assert_eq!(frame(&mut app, root), root);
        assert!(!app.loop_rail.animating(true));
        assert_eq!(serde_json::to_value(&app.loop_ctl).unwrap(), before);
        app.loop_rail.visible = true;
        assert!(!app.loop_rail.animating(false));
    }
}
