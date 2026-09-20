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
use crate::ui::hud::{HUD_DIM, HUD_GOLD, HUD_TEXT};
use crate::ui::term::art::{
    ColoredBrailleCell, ColoredBrailleImage, braille_char, braille_dot_bit,
};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Tone calibration for the moonlit frame → 1-bit dots. Tuned against the
/// moving and settled scene fixtures: while riding, the sky stays
/// almost empty (stars and moon only), near walls dither at roughly half, and
/// fog starves walls toward the horizon. Settled vistas deliberately use a
/// luminous-horizon painting curve instead.
const TONE_BLACK: f32 = 18.0;
const TONE_WHITE: f32 = 118.0;
const TONE_GAMMA: f32 = 0.82;
const VISTA_TONE_BLACK: f32 = 8.0;
const VISTA_TONE_WHITE: f32 = 112.0;
const VISTA_TONE_GAMMA: f32 = 0.78;
/// A cell's ink must clear this max-channel value to survive on a dark
/// terminal; depth still reads from dot density, not ink brightness.
const INK_FLOOR: f32 = 108.0;
/// Ink luminance-weighting curve exponent. 1.0 = plain luma-proportional
/// weighting (the landed default): each lit dot contributes color in
/// proportion to its brightness. Below 1.0 flattens toward a plain average
/// (dim dots matter more); above 1.0 sharpens bright-dot dominance (dim dots
/// vanish from the cell's color). One constant — the look is tunable here
/// without touching the sampler.
const INK_LUMA_GAMMA: f32 = 1.0;

/// The per-dot color weight: brightness raised to the ink gamma. Pure — the
/// tests pin the curve's properties (monotonic, black is weightless, and
/// bright-dot dominance grows with gamma) so a future tune is a judgment
/// call, never a surprise.
fn ink_weight(luma: f32, gamma: f32) -> f32 {
    luma.max(0.0).powf(gamma)
}

/// Standard 8×8 Bayer threshold matrix (0..64).
const BAYER_8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

fn bayer(x: usize, y: usize) -> f32 {
    (BAYER_8[y % 8][x % 8] as f32 + 0.5) / 64.0
}

/// The calibrated tone curve as a 256-entry luma LUT: the black/white points
/// and gamma only ever see 8-bit luma, so the per-dot `powf` (thousands per
/// re-march) collapses to one table build.
fn tone_lut(vista: bool) -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    static VISTA_LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    let (lut, black, white, gamma) = if vista {
        (
            &VISTA_LUT,
            VISTA_TONE_BLACK,
            VISTA_TONE_WHITE,
            VISTA_TONE_GAMMA,
        )
    } else {
        (&LUT, TONE_BLACK, TONE_WHITE, TONE_GAMMA)
    };
    lut.get_or_init(|| {
        let mut table = [0.0; 256];
        for (i, value) in table.iter_mut().enumerate() {
            *value = ((i as f32 - black) / (white - black))
                .clamp(0.0, 1.0)
                .powf(gamma);
        }
        table
    })
}

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
            crate::ui::viz::lifecycle_viz::MotionMode::Off,
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
    fn scryglass_frame_with_motion(
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
        let world_key = if ambient {
            self.ambient_scene_sequence(motion)
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
            let good_enough = !ambient
                && relaxed
                && same_view
                && self.tick.saturating_sub(cached.rendered_at) < RELAXED_REFRESH_TICKS;
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
        } else {
            let (map, mut view) = self.travel_scene();
            view.heading_rad += yaw_offset;
            view.look_yaw = yaw_offset;
            view.fov_rad = fov;
            // `bob` is the renderer's intentional horizon offset. Reuse it for the
            // human camera pitch; a settled world has no canter motion.
            view.bob = if self.riding() { view.bob } else { 0.0 } + pitch / 0.03;
            world3d::render_region_frame(
                self.quest(),
                &map,
                &view,
                yaw_offset,
                self.tick / world3d::region::WISP_TICKS,
                dot_w,
                dot_h,
            )
        };
        // Outdoor first-person: stamp the current mounted animation frame at
        // the bottom of the plate. Interiors stay on foot — no saddle overlay
        // — while Dotmax retains its established mounted overlay.
        if self.interior.is_none() && !ambient {
            composite_rider_overlay(&mut frame, cinematics::rider_frame_key(self));
        }
        let image = std::sync::Arc::new(frame_to_braille_graded(
            &frame,
            cells_w,
            cells_h,
            ambient || self.settled_vista_grade(),
        ));
        *self.ride_cache.borrow_mut() = Some(RideCacheEntry {
            world_key,
            view_key,
            rendered_at: self.tick,
            image: std::sync::Arc::clone(&image),
        });
        Some(image)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ride_caption(&self) -> Line<'static> {
        Line::from(vec![
            Span::styled("▌ ", Style::new().fg(HUD_GOLD)),
            Span::styled(self.knight_caption_verb(), Style::new().fg(HUD_DIM)),
            Span::styled(
                cinematics::building_name(self.target).to_string(),
                Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" — {}{}", self.activity, self.ride_ward_caption()),
                Style::new().fg(HUD_DIM),
            ),
        ])
    }
}

/// The red companion holds the left rail, blue the right, on one shared
/// baseline. The shared sprite compositor preserves aspect and the road center.
fn composite_rider_overlay(frame: &mut image::RgbaImage, key: cinematics::RiderFrameKey) {
    crate::stage::knight_cast::composite(frame, key);
}

fn frame_to_braille_graded(
    frame: &image::RgbaImage,
    cells_w: usize,
    cells_h: usize,
    vista: bool,
) -> ColoredBrailleImage {
    let mut cells = vec![ColoredBrailleCell::default(); cells_w * cells_h];
    for cell_y in 0..cells_h {
        for cell_x in 0..cells_w {
            let mut bits = 0u8;
            let mut ink = [0f32; 3];
            let mut lit = 0f32;
            for local_y in 0..4 {
                for local_x in 0..2 {
                    let x = cell_x * 2 + local_x;
                    let y = cell_y * 4 + local_y;
                    let pixel = frame.get_pixel(x as u32, y as u32).0;
                    let luma =
                        0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
                    let tone = tone_lut(vista)[(luma.round() as usize).min(255)];
                    if tone > bayer(x, y) {
                        bits |= braille_dot_bit(local_x, local_y);
                        // The cell's ink should be the color you actually SEE:
                        // each lit dot contributes proportionally to its luma,
                        // so a few bright dots dominate the dim ones instead
                        // of a flat average letting the dimmest dot muddy the
                        // cell. A lit dot always has luma ≥ 1, so `lit` can
                        // never divide by zero.
                        let weight = ink_weight(luma, INK_LUMA_GAMMA);
                        ink[0] += pixel[0] as f32 * weight;
                        ink[1] += pixel[1] as f32 * weight;
                        ink[2] += pixel[2] as f32 * weight;
                        lit += weight;
                    }
                }
            }
            if bits == 0 {
                continue;
            }
            let mut fg = [ink[0] / lit, ink[1] / lit, ink[2] / lit];
            let peak = fg[0].max(fg[1]).max(fg[2]);
            if peak > 0.0 && peak < INK_FLOOR {
                let scale = INK_FLOOR / peak;
                fg = [fg[0] * scale, fg[1] * scale, fg[2] * scale];
            }
            let quantize_channel = |val: f32| {
                let channel = val.round().clamp(0.0, 255.0) as u16;
                let level = (channel * 31 + 127) / 255;
                ((level * 255 + 15) / 31) as u8
            };
            cells[cell_y * cells_w + cell_x] = ColoredBrailleCell {
                glyph: braille_char(bits),
                fg: [
                    quantize_channel(fg[0]),
                    quantize_channel(fg[1]),
                    quantize_channel(fg[2]),
                ],
            };
        }
    }
    ColoredBrailleImage {
        width: cells_w,
        height: cells_h,
        cells,
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/ride__tests.rs"]
mod tests;
