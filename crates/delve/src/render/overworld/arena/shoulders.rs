//! Shoulder rides on screen (`rules::shoulders`): a tower of knights, each
//! stood on the shoulders of the one below and swaying a little more the
//! higher they are, and a knight thrown off the top tumbling along a high
//! arc over their shadow, bigger as they rise, landing in a ring of dust.

use super::*;
use crate::rules::Hero;
use crate::rules::shoulders::{SLAM_REACH, SLAM_SHOWN, THROW_PEAK};

/// How far up the one below a rider stands, in native pixels: their feet on
/// the shoulders, the head below between their knees.
pub(super) const RIDER_STEP: i32 = 15;

/// How a knight in a tower or in the air stands off their place on the
/// floor: `lift` pixels up, `dx` across (the tower's sway), and, in the
/// air, how far along the throw they are and how high (units).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Perch {
    pub lift: i32,
    pub dx: i32,
    pub level: usize,
    pub flight: Option<(f32, f32)>,
}

impl Perch {
    /// Whether they cast a shadow of their own: not a rider (the carrier's
    /// is the tower's).
    pub fn shadowed(&self) -> bool {
        self.level == 0 && self.flight.is_none()
    }

    /// Where their figure is ordered among the crowd: a rider just over the
    /// one below, a knight in the air over everything on the floor.
    pub fn depth(&self, y: f32) -> f32 {
        if self.flight.is_some_and(|(_, h)| h > 0.8) {
            y + 1000.0
        } else {
            y + self.level as f32 * 0.001
        }
    }
}

/// Knight `id`'s perch, drawn at `hero`'s place on tick `tick`.
pub(super) fn perch(run: &Run, id: u32, hero: &Hero, tick: u32) -> Perch {
    let s = &run.shoulders;
    if let Some(flight) = s.flight(id) {
        let t = flight.progress_at((hero.x, hero.y));
        let h = flight.height_at(t);
        return Perch { lift: (h * UNIT).round() as i32, dx: 0, level: 0, flight: Some((t, h)) };
    }
    let level = s.level(id);
    if level == 0 {
        return Perch::default();
    }
    // The higher up, the wider the sway; each rider a beat behind the one
    // below.
    let phase = tick as f32 * 0.11 - level as f32 * 0.7;
    let dx = (phase.sin() * level as f32 * 0.55).round() as i32;
    Perch { lift: level as i32 * RIDER_STEP, dx, level, flight: None }
}

/// A knight in the air as drawn `t` of the way along, `h` units up: tucked
/// and tumbling twice over in the middle of the throw, upright at its two
/// ends, and bigger the higher they are.
pub(super) fn airborne(im: &Img, t: f32, h: f32) -> Img {
    let turned = if (0.12..0.88).contains(&t) {
        let left = 10 - ((t - 0.12) / 0.76 * 20.0) as u32 % 10;
        knights::rolled(im, left)
    } else {
        im.clone()
    };
    let k = 1.0 + 0.6 * (h / THROW_PEAK).clamp(0.0, 1.0);
    let k = (k * 8.0).round() / 8.0;
    if k > 1.0 { enlarge(&turned, k) } else { turned }
}

/// The arc behind knight `id` in the air, `t` of the way along: a trail of
/// dust motes along the way they came, fading behind them.
pub(super) fn trail(cv: &mut Img, run: &Run, id: u32, t: f32, feet: f32) {
    use super::super::ink::bayer;
    let Some(flight) = run.shoulders.flight(id) else {
        return;
    };
    for i in 1..=10 {
        let tt = t - i as f32 * 0.035;
        if tt <= 0.0 {
            break;
        }
        let gx = flight.from.0 + (flight.to.0 - flight.from.0) * tt;
        let gy = flight.from.1 + (flight.to.1 - flight.from.1) * tt;
        let (x, y) = at(gx, gy + feet);
        // The middle of the knight, not their feet: a figure's height up.
        let y = y - (flight.height_at(tt) * UNIT).round() as i32 - 7;
        let fade = i as f32 / 11.0;
        for (dx, ink) in [(0, '3'), (1, '2')] {
            if bayer(x + dx, y) >= fade {
                cv.put(x + dx, y, ink);
            }
        }
    }
}

/// The shadow under a knight in the air, `h` units up: smaller and broken
/// up the higher they are.
pub(super) fn flight_shadow(cv: &mut Img, x: i32, y: i32, h: f32) {
    use super::super::ink::bayer;
    let k = 1.0 - 0.5 * (h / THROW_PEAK).clamp(0.0, 1.0);
    let half = (5.0 * k).round().max(2.0) as i32;
    for (dy, half) in [(0, half), (1, (half - 1).max(1))] {
        for dx in -half..=half {
            let (px, py) = (x + dx, y + dy);
            if (dx.abs() == half || h > THROW_PEAK * 0.6) && bayer(px, py) < 0.5 {
                continue;
            }
            cv.put(px, py, 's');
        }
    }
}

/// A broad shadow under a tower: the carrier's, wider for the load.
pub(super) fn tower_shadow(cv: &mut Img, x: i32, y: i32, riders: usize) {
    use super::super::ink::bayer;
    let half = 5 + riders.min(5) as i32;
    for (dy, half) in [(0, half), (1, half - 1), (2, half / 2)] {
        for dx in -half..=half {
            let (px, py) = (x + dx, y + dy);
            if dx.abs() >= half - 1 && bayer(px, py) < 0.5 {
                continue;
            }
            cv.put(px, py, 's');
        }
    }
}

/// The landings' slams: a ring of dust thrown out over the slam's reach
/// and thinning as it goes, a crack of light at its first tick.
pub(super) fn slams(cv: &mut Img, run: &Run) {
    use super::super::ink::bayer;
    for slam in &run.shoulders.slams {
        let age = run.tick.saturating_sub(slam.tick) as f32;
        let k = (age + 1.0) / SLAM_SHOWN as f32;
        if k > 1.0 {
            continue;
        }
        let (cx, cy) = at(slam.x, slam.y);
        let r = SLAM_REACH * UNIT * (0.35 + 0.65 * k.sqrt());
        let thin = k * 0.85;
        let steps = (r * 5.0) as i32;
        for i in 0..steps {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            for (band, ink) in [(0.0, '3'), (-1.5, '2')] {
                let (px, py) = (
                    cx + ((r + band) * a.cos()).round() as i32,
                    cy + ((r + band) * a.sin() * 0.5).round() as i32,
                );
                if bayer(px, py) >= thin {
                    cv.put(px, py, ink);
                }
            }
        }
        if age < 2.0 {
            // The crack: a flat burst under the feet.
            for dx in -6i32..=6 {
                cv.put(cx + dx, cy, if dx.abs() < 3 { 'w' } else { '3' });
            }
            for dy in [-1, 1] {
                for dx in -3i32..=3 {
                    cv.put(cx + dx, cy + dy, '2');
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__shoulders__shots.rs"]
mod shots;
