//! The braille ride: the first-person Dotmax travel world rendered as a
//! colored braille field for the default (protocol-free) miniviz.
//!
//! Dotmax draws a moonlit mesh at dot resolution (2×4 dots per cell). The
//! structure fitter keeps silhouettes and occlusion edges, while broad planes
//! carry soft hue-shifted value in their ink without checker density or a
//! brightness floor. The realm's 2D map keeps its separate art conversion.

use super::*;
use angel_delve::render::braille::ColoredBrailleImage;
pub(super) use angel_delve::render::braille::frame_to_braille_world3d as frame_to_braille_graded;

impl World {
    pub fn riding(&self) -> bool {
        self.avatar != self.dest() || !self.avatar_vis_settled()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn scryglass_frame_paced(
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
            crate::telemetry::MotionMode::Full,
        )
    }

    pub fn ambient_braille_frame(
        &self,
        cells_w: usize,
        cells_h: usize,
        motion: crate::telemetry::MotionMode,
    ) -> Option<std::sync::Arc<ColoredBrailleImage>> {
        self.scryglass_frame_with_motion(cells_w, cells_h, false, 0.0, 0.0, 1.05, motion)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn scryglass_frame_with_motion(
        &self,
        cells_w: usize,
        cells_h: usize,
        relaxed: bool,
        yaw_offset: f32,
        pitch: f32,
        fov: f32,
        motion: crate::telemetry::MotionMode,
    ) -> Option<std::sync::Arc<ColoredBrailleImage>> {
        const RELAXED_REFRESH_TICKS: u64 = 30;
        if cells_w == 0 || cells_h == 0 {
            return None;
        }
        let pitch = pitch.clamp(-0.30, 0.30);
        let fov = fov.clamp(0.70, 1.40);
        let ambient = self.ambient_interior_visible();
        let expedition = self.expedition_playback(motion);
        let region_stage = world3d::region::stage_for(self.quest().region());
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
        #[cfg(any(test, feature = "testing"))]
        super::world_scene_tax::note_ride_compose();
        let dot_w = (cells_w * 2) as u32;
        let dot_h = (cells_h * 4) as u32;
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
        if self.interior.is_none() && !ambient && region_stage.is_none() && crawl.is_none() {
            composite_rider_overlay(&mut frame, cinematics::rider_frame_key(self));
        }
        let convert = if ambient && self.ambient_building() != Building::Observatory {
            angel_delve::render::braille::frame_to_braille_graded
        } else {
            frame_to_braille_graded
        };
        let image = std::sync::Arc::new(convert(
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

pub(super) fn composite_rider_overlay(
    frame: &mut image::RgbaImage,
    key: cinematics::RiderFrameKey,
) {
    crate::knight_cast::composite(frame, key);
}

#[cfg(test)]
#[path = "../../tests/world_viz/ride__tests.rs"]
mod tests;
