//! The braille ride: the first-person Dotmax travel world rendered as a
//! colored braille field for the default (protocol-free) miniviz.
//!
//! Dotmax draws one moonlit mesh frame at dot resolution (2×4
//! dots per terminal cell), a fixed ordered-Bayer screen turns luminance
//! into dots, and each cell takes its ink color from the pixels that lit.
//! Ordered dithering is deliberate: the threshold lives in screen space, so
//! a moving frame animates without the frame-to-frame crawl error diffusion
//! would produce. Depth reads as dot density — night fog starves distant
//! walls of ink — while the candlelit destination door keeps burning
//! through it.

use super::*;
use crate::ui::term::art::ColoredBrailleImage;
pub(super) use angel_delve::render::braille::frame_to_braille_graded;

impl World {
    /// True while the knight is between landmarks — the ride owns the pane.
    pub(crate) fn riding(&self) -> bool {
        self.avatar != self.dest() || !self.avatar_vis_settled()
    }

    /// Persistent Scryglass first-person frame at cell resolution. `None` only
    /// for a zero pane; every real pane renders. It remains valid after arrival
    /// and accepts a display-only camera offset. Memoized on the semantic
    /// cinematic key, so only a world state change recomposes the scene —
    /// redraws driven by transcript motion reuse the frame. Returned by shared
    /// handle: a draw only reads the frame, so a cache hit must not copy the
    /// whole cell buffer out per redraw.
    ///
    /// `relaxed` marks a live agent turn: the toy pane must not spend draw
    /// time on scenery, so a memoized frame of the right size is reused
    /// until `RELAXED_REFRESH_TICKS` world ticks have passed. The ride keeps
    /// moving at a low cadence while the agent lane keeps the cycles, and
    /// catches up to full motion the moment the turn ends.
    #[cfg(test)]
    pub(crate) fn scryglass_frame_paced(
        &self,
        cells_w: usize,
        cells_h: usize,
        relaxed: bool,
        yaw_offset: f32,
        pitch: f32,
        fov: f32,
    ) -> Option<std::sync::Arc<ColoredBrailleImage>> {
        self.scryglass_frame_with_motion(
            cells_w,
            cells_h,
            relaxed,
            yaw_offset,
            pitch,
            fov,
            crate::ui::viz::lifecycle_viz::MotionMode::Full,
        )
    }

    pub(crate) fn ambient_braille_frame(
        &self,
        cells_w: usize,
        cells_h: usize,
        motion: crate::ui::viz::lifecycle_viz::MotionMode,
    ) -> Option<std::sync::Arc<ColoredBrailleImage>> {
        self.scryglass_frame_with_motion(cells_w, cells_h, false, 0.0, 0.0, 1.05, motion)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scryglass_frame_with_motion(
        &self,
        cells_w: usize,
        cells_h: usize,
        relaxed: bool,
        yaw_offset: f32,
        pitch: f32,
        fov: f32,
        motion: crate::ui::viz::lifecycle_viz::MotionMode,
    ) -> Option<std::sync::Arc<ColoredBrailleImage>> {
        /// Minimum world ticks between outdoor recompositions during a turn.
        /// (Idle keystrokes are already protected upstream: `advance` paces
        /// `world.tick()` by wall clock, so typing can't churn the key.)
        const RELAXED_REFRESH_TICKS: u64 = 30;
        if cells_w == 0 || cells_h == 0 {
            return None;
        }
        let pitch = pitch.clamp(-0.30, 0.30);
        let fov = fov.clamp(0.70, 1.40);
        let ambient = self.ambient_interior_visible();
        let expedition = self.expedition_playback(motion);
        let region_stage = world3d::region::stage_for(self.quest().region());
        // A loop's region is walked as the Delve's crawl, unless the older
        // expedition walk is asked for.
        let crawl = self.loop_crawl_on().then(|| self.crawl_clock(motion));
        let world_key = if ambient {
            self.ambient_scene_sequence(motion)
        } else if let Some(clock) = &crawl {
            clock.key()
        } else if region_stage.is_some() {
            world3d::expedition::frame_key(self.quest(), expedition)
        } else {
            self.cinematic_key()
        };
        let view_key = RideViewKey {
            cells_w,
            cells_h,
            yaw: if ambient { 0 } else { yaw_offset.to_bits() },
            pitch: if ambient { 0 } else { pitch.to_bits() },
            fov: if ambient { 0 } else { fov.to_bits() },
        };
        if let Some(cached) = self.ride_cache.borrow().as_ref() {
            let same_view = cached.view_key == view_key;
            let fresh = cached.world_key == world_key && same_view;
            let hold = if region_stage.is_some() {
                // Five rendered frames per second while the agent works;
                // smooth camera travel without turning the toy into a busy loop.
                world3d::expedition::FRAME_TICKS * 2
            } else {
                RELAXED_REFRESH_TICKS
            };
            let good_enough = !ambient
                && relaxed
                && same_view
                && self.tick.saturating_sub(cached.rendered_at) < hold;
            if fresh || good_enough {
                return Some(std::sync::Arc::clone(&cached.image));
            }
        }
        #[cfg(test)]
        super::world_scene_tax::note_ride_compose();
        let dot_w = (cells_w * 2) as u32;
        let dot_h = (cells_h * 4) as u32;
        // Entered locations use their retained painting; outdoors always use Dotmax.
        let mut frame = if ambient {
            image::imageops::resize(
                &super::ambient::frame(self.ambient_building(), self.ambient_pose(motion)),
                dot_w,
                dot_h,
                image::imageops::FilterType::Nearest,
            )
        } else if let Some(clock) = &crawl {
            self.crawl_frame(clock, (dot_w, dot_h))
        } else if let Some(stage) = region_stage {
            world3d::expedition::render_controls(
                stage,
                self.quest(),
                yaw_offset,
                pitch,
                fov,
                expedition,
                (dot_w, dot_h),
            )
        } else {
            let (map, mut view) = self.travel_scene();
            view.heading_rad += yaw_offset;
            view.look_yaw = yaw_offset;
            view.fov_rad = fov;
            view.bob = if self.riding() { view.bob } else { 0.0 } + pitch / 0.03;
            world3d::render_ride_frame(&map, &view, dot_w, dot_h)
        };
        // Outdoor first-person: stamp the current mounted animation frame at
        // the bottom of the plate. Interiors stay on foot — no saddle overlay
        // — while Dotmax retains its established mounted overlay.
        if self.interior.is_none() && !ambient && region_stage.is_none() && crawl.is_none() {
            composite_rider_overlay(&mut frame, cinematics::rider_frame_key(self));
        }
        let image = std::sync::Arc::new(frame_to_braille_graded(
            &frame,
            cells_w,
            cells_h,
            ambient || region_stage.is_some() || crawl.is_some() || self.settled_vista_grade(),
        ));
        *self.ride_cache.borrow_mut() = Some(RideCacheEntry {
            world_key,
            view_key,
            rendered_at: self.tick,
            image: std::sync::Arc::clone(&image),
        });
        Some(image)
    }
}

/// The red companion holds the left rail, blue the right, on one shared
/// baseline. The shared sprite compositor preserves aspect and the road center.
pub(super) fn composite_rider_overlay(
    frame: &mut image::RgbaImage,
    key: cinematics::RiderFrameKey,
) {
    crate::stage::knight_cast::composite(frame, key);
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/ride__tests.rs"]
mod tests;
