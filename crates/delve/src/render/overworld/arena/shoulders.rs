//! Shoulder rides on screen (`rules::shoulders`): a tower of knights, each
//! stood on the shoulders of the one below and swaying a little more the
//! higher they are, and a knight thrown off the top tumbling along a high
//! arc over their shadow, bigger as they rise, landing in a ring of dust.

use super::*;
use crate::rules::Hero;
use crate::rules::shoulders::{SLAM_REACH, SLAM_SHOWN, THROW_PEAK};

pub(super) const RIDER_STEP: i32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Perch {
    pub lift: i32,
    pub dx: i32,
    pub level: usize,
    pub flight: Option<(f32, f32)>,
}

impl Perch {
    pub fn shadowed(&self) -> bool {
        self.level == 0 && self.flight.is_none()
    }

    pub fn depth(&self, y: f32) -> f32 {
        if self.flight.is_some_and(|(_, h)| h > 0.8) {
            y + 1000.0
        } else {
            y + self.level as f32 * 0.001
        }
    }
}

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
    let phase = tick as f32 * 0.11 - level as f32 * 0.7;
    let dx = (phase.sin() * level as f32 * 0.55).round() as i32;
    Perch { lift: level as i32 * RIDER_STEP, dx, level, flight: None }
}

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
        let y = y - (flight.height_at(tt) * UNIT).round() as i32 - 7;
        let fade = i as f32 / 11.0;
        for (dx, ink) in [(0, '3'), (1, '2')] {
            if bayer(x + dx, y) >= fade {
                cv.put(x + dx, y, ink);
            }
        }
    }
}

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
