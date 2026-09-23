//! Sky: the weather and the celebrations laid over the lit map.
//!
//! The sky is folded from real health (`World::weather`): cloud shadows
//! drift over the realm while errors pile up, quotas bite or the loop
//! budget runs thin, and a rainbow arcs over the field on the first green
//! after a red run. Fireworks burst over the keep while a victory is being
//! celebrated. Shadows step colours down their own banks; the rainbow and
//! the fireworks are state, so they burn in the signal bank.

use super::ink::{Img, bayer, hash, vnoise};
use super::light::step_down;
use super::map::TILE;
use super::scene::Weather;

/// Weather over a view whose top-left sits at world pixel `(ox, oy)`.
/// Wet weather is shade, not streaks: rain and lightning are left out so
/// the realm stays dark and a frame is not re-sent for every falling drop.
pub(crate) fn weather(cv: &mut Img, sky: Weather, tick: u32, (ox, oy): (i32, i32)) {
    match sky {
        Weather::Fair | Weather::Drizzle => {}
        Weather::Clouds | Weather::Rain | Weather::Storm => cloud_shadows(cv, tick, (ox, oy)),
        Weather::Rainbow => rainbow(cv),
    }
}

/// Soft shadows drifting east, a step darker under the cloud.
fn cloud_shadows(cv: &mut Img, tick: u32, (ox, oy): (i32, i32)) {
    let drift = tick as f32 * 0.9;
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (wx, wy) = (ox + x, oy + y);
            let n = vnoise((wx as f32 - drift) / 56.0, wy as f32 / 40.0, 131);
            if n > 0.56 + bayer(wx, wy) * 0.12
                && let Some(c) = cv.get(x, y)
            {
                cv.set(x, y, step_down(c, 1));
            }
        }
    }
}

/// The recovery rainbow: a dithered arc over the field.
fn rainbow(cv: &mut Img) {
    let (cx, cy) = (cv.w as f32 / 2.0, cv.h as f32 * 1.15);
    let r0 = cv.h as f32 * 0.95;
    let bands = ['7', '@', '5', '2', '1'];
    for y in 0..cv.h {
        for x in 0..cv.w {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let band = ((d - r0) / 2.0).floor();
            if (0.0..bands.len() as f32).contains(&band) && bayer(x, y) < 0.5 {
                cv.put(x, y, bands[band as usize]);
            }
        }
    }
}

/// Fireworks over the keep: four bursts at staggered phases, each a
/// flash, a ring of sparks and an inner ring, thinning as they spread.
pub(crate) fn fireworks(cv: &mut Img, tick: u32, (ox, oy): (i32, i32)) {
    const PERIOD: u32 = 12;
    let (kx, ky) = super::map::place_px(23 * TILE + 8, 12 * TILE + 14);
    let (kx, ky) = (kx - ox, ky - oy);
    let inks = ['5', '7', '2', '3', '6', '@', '1'];
    for burst in 0..4u32 {
        let t = tick + burst * 3;
        let phase = t % PERIOD;
        let h = hash(burst as i32, (t / PERIOD) as i32, 221);
        let (bx, by) = (kx + (h % 120) as i32 - 60, ky + 14 - ((h >> 8) % 30) as i32);
        let ink = inks[((h >> 16) % inks.len() as u32) as usize];
        if phase < 2 {
            // the rocket climbing
            cv.put(bx, by + 8 - phase as i32 * 4, '6');
            cv.put(bx, by + 9 - phase as i32 * 4, 'a');
            continue;
        }
        let spread = (phase - 2) as f32;
        if phase < 4 {
            for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                cv.put(bx + dx, by + dy, 'w');
            }
        }
        for (ring, scale, sparks) in [(0, 3.0f32, 20), (1, 1.8f32, 12)] {
            for spark in 0..sparks {
                if phase > 8 && (spark + phase as i32 + ring) % 3 == 0 {
                    continue;
                }
                let a = spark as f32 / sparks as f32 * std::f32::consts::TAU + ring as f32 * 0.3;
                let r = spread * scale;
                let (sx, sy) = (bx + (a.cos() * r) as i32, by + (a.sin() * r * 0.8) as i32);
                cv.put(sx, sy, if ring == 0 { ink } else { '6' });
                if phase < 7 && ring == 0 {
                    cv.put(
                        sx - (a.cos() * 2.0) as i32,
                        sy - (a.sin() * 1.6) as i32,
                        ink,
                    );
                }
            }
        }
    }
}
