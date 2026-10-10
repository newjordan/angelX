//! Projection-only bridge to the normal Dotmax pane. One bounded frame cache;
//! no timers, game advancement, persistence or hidden worker work here.
use super::{World, ride::frame_to_braille_graded};
use crate::drive::chivalry::{Chivalry, Place, Visit};
use crate::ui::term::art::ColoredBrailleImage;
use crate::ui::viz::lifecycle_viz::MotionMode;
use std::sync::Arc;
#[derive(PartialEq)]
struct Key {
    state: Chivalry,
    visit: Visit,
    w: usize,
    h: usize,
    view: [u32; 4],
    motion: MotionMode,
}
pub(super) struct FrameCache {
    key: Key,
    frame: Arc<ColoredBrailleImage>,
}
impl World {
    pub(crate) fn open_chivalry(&mut self, place: Place, inside: bool) {
        self.graph_destination = false;
        self.interior = None;
        self.school_room = None;
        // A deliberate destination beats the live adventure stage on every
        // normal surface, including the optional realm map.
        self.overworld_view = Some((
            55.0 * super::overworld::TILE as f32,
            41.0 * super::overworld::TILE as f32,
            place.label(),
        ));
        self.chivalry_visit = Some(Visit {
            place,
            inside,
            station: 0,
        });
        self.activity = format!(
            "{} · /dungeon {} status",
            place.label(),
            if place == Place::Stables {
                "stable"
            } else {
                "tournament"
            }
        );
    }
    pub(crate) fn close_chivalry(&mut self) {
        if self.chivalry_visit.take().is_some() {
            self.overworld_view = None;
        }
        self.clear_chivalry_frame_cache();
    }
    pub(crate) fn clear_chivalry_frame_cache(&self) {
        *self.chivalry_cache.borrow_mut() = None;
    }
    pub(crate) fn chivalry_step(&mut self, delta: i8) -> bool {
        let Some(v) = self.chivalry_visit.as_mut().filter(|v| v.inside) else {
            return false;
        };
        v.station = (i16::from(v.station) + i16::from(delta)).clamp(0, 3) as u8;
        true
    }
    // Keep viewport and camera controls explicit at this established renderer boundary.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn chivalry_braille_frame(
        &self,
        w: usize,
        h: usize,
        sway: f32,
        yaw: f32,
        pitch: f32,
        fov: f32,
        motion: MotionMode,
    ) -> Arc<ColoredBrailleImage> {
        self.chivalry_frame(
            self.chivalry_visit.expect("visible stage"),
            w,
            h,
            sway,
            yaw,
            pitch,
            fov,
            motion,
        )
    }
    // Keep viewport and camera controls explicit at this established renderer boundary.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn chivalry_frame(
        &self,
        visit: Visit,
        w: usize,
        h: usize,
        sway: f32,
        yaw: f32,
        pitch: f32,
        fov: f32,
        motion: MotionMode,
    ) -> Arc<ColoredBrailleImage> {
        let (w, h) = (w.min(256), h.min(80));
        let finite = |v: f32, lo: f32, hi: f32| {
            if v.is_finite() {
                (v.clamp(lo, hi) * 64.0).round() / 64.0
            } else {
                0.0
            }
        };
        // No idle animation even under Full. Off/Reduced cannot move authored props.
        let view = [
            if motion == MotionMode::Off {
                0.0
            } else {
                finite(sway, -1.0, 1.0)
            },
            finite(yaw, -3.2, 3.2),
            finite(pitch, -1.0, 1.0),
            finite(fov, 0.45, 1.4),
        ];
        let key = Key {
            state: self.chivalry.clone(),
            visit,
            w,
            h,
            view: view.map(f32::to_bits),
            motion,
        };
        if let Some(cache) = self
            .chivalry_cache
            .borrow()
            .as_ref()
            .filter(|c| c.key == key)
        {
            return Arc::clone(&cache.frame);
        }
        let pixels = super::world3d::chivalry::frame(&self.chivalry, visit, w * 2, h * 4, view);
        let frame = Arc::new(frame_to_braille_graded(&pixels, w, h, true));
        *self.chivalry_cache.borrow_mut() = Some(FrameCache {
            key,
            frame: Arc::clone(&frame),
        });
        frame
    }
}
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/chivalry__tests.rs"]
mod tests;
